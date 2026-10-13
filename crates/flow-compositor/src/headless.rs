//! The headless compositor: real Wayland clients, rendered in software, written to a PNG.
//!
//! This is the first step of the express road (HANDOFF.md, E1): the Flow shell stops being a
//! picture of a desktop and starts hosting other programs' windows. It runs **headless with
//! Smithay's pixman (software) renderer** because CI runners have no GPU, and because a frame
//! a test can read back is the evidence this repository asks for (ADR 0010: "CI proves a
//! frame, not a unit"). The live service (`live.rs`: a real display, keyboard and mouse,
//! E3 and E10) reuses everything here and swaps only where the frame goes and where input
//! comes from.
//!
//! The flow: listen on a socket, accept clients, answer the protocols a window needs, and
//! once `wait_for` toplevels have each committed a buffer, either write one frame, or run a
//! **script** of input commands (`click X Y`, `key C...`, `cycle`, `close`, `frame PATH`,
//! `settle MS`). The script goes through the same input handlers real devices do, so a test
//! proves focus, clicks, keys and close without a keyboard or a mouse.
//!
//! Protocols served: `wl_compositor`, `wl_subcompositor`, `wl_shm`, `xdg_wm_base`,
//! `wl_seat`, `wl_data_device`, and `wl_output` with `xdg_output` (the screen: GTK, and so
//! Firefox, cannot size or scale a window without one). Not `linux-dmabuf`, deliberately:
//! the pixman renderer cannot import dmabufs, and without it clients draw into `wl_shm`
//! buffers, which it shows. Windows are placed by their window geometry, not their surface
//! (a toolkit's shadow is inside the surface), and asked to fit the screen ([`wm::fit`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::input::{ButtonState, KeyState};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::utils::{
    draw_render_elements, on_commit_buffer_handler, with_renderer_surface_state,
};
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Frame, Offscreen, Renderer};
use smithay::input::keyboard::{FilterResult, KeyboardHandle, Keycode, Keysym, XkbConfig};
use smithay::input::pointer::{ButtonEvent, MotionEvent, PointerHandle};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::{wl_buffer, wl_seat, wl_surface::WlSurface};
use smithay::reexports::wayland_server::{Client, Display, ListeningSocket};
use smithay::utils::{
    Buffer, Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Serial, Size, Transform,
};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};
use smithay::wayland::compositor::{
    SurfaceAttributes, TraversalAction, with_states, with_surface_tree_downward,
};
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::shell::xdg::SurfaceCachedState;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
};

use flow_shell::Frame as ShellFrame;
use flow_shell::palette;

use flow_compositor::wm::{self, Manager, Window};

/// What to run, from the command line.
pub struct Options {
    /// Socket name, created in `$XDG_RUNTIME_DIR` (what clients put in `WAYLAND_DISPLAY`).
    pub socket: String,
    pub width: u32,
    pub height: u32,
    /// Render once this many toplevels have committed a buffer.
    pub wait_for: usize,
    /// Give up, with an error, if they have not by then.
    pub timeout: Duration,
    /// Where to write the frame when there is no script.
    pub out: Option<PathBuf>,
    /// Input commands to run once the windows have drawn, one per line.
    pub script: Option<String>,
    /// Show the screen on this DRM device and run until stopped (E3). Overrides the rest.
    pub drm: Option<PathBuf>,
}

struct App {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    seat_state: SeatState<Self>,
    data_device_state: DataDeviceState,
    /// The screen, as clients see it (`wl_output`). Toolkits need one: GTK, and so Firefox,
    /// size their windows and pick their scale from it, and without it GTK logs a critical
    /// error on every frame (E4).
    _output_manager_state: OutputManagerState,
    output: Output,
    wm: Manager,
    /// Surfaces and the IDs the window manager knows them by, and where each window's
    /// visible part starts inside its surface (its window geometry; see `refresh`).
    ids: Vec<(WlSurface, u32)>,
    offsets: Vec<(u32, (i32, i32))>,
    /// The size each window was last asked to take, so a window is asked once, not on every
    /// turn while it gets round to it.
    asked: Vec<(u32, (i32, i32))>,
    /// The screen's size, which windows are kept inside.
    screen: (i32, i32),
    next_id: u32,
}

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl BufferHandler for App {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl CompositorHandler for App {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("every client is inserted with ClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        // Keeps each surface's current buffer where the renderer can import it.
        on_commit_buffer_handler::<Self>(surface);
    }
}

impl XdgShellHandler for App {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // A client may not draw before its first configure, so answer at once, and say how
        // big a window may be (clients that read `configure_bounds` open inside it).
        let (max_w, max_h) = wm::max_size(self.screen.0, self.screen.1);
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
            state.bounds = Some((max_w, max_h).into());
        });
        surface.send_configure();
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}
    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
}

impl ShmHandler for App {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for App {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }
}

impl SelectionHandler for App {
    type SelectionUserData = ();
}

impl DataDeviceHandler for App {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for App {}
impl ServerDndGrabHandler for App {}

impl OutputHandler for App {}

delegate_compositor!(App);
delegate_output!(App);
delegate_xdg_shell!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_data_device!(App);

impl App {
    /// Toplevels that have a buffer to show, in the order they were created.
    fn drawn_toplevels(&self) -> Vec<WlSurface> {
        self.xdg_shell_state
            .toplevel_surfaces()
            .iter()
            .map(|t| t.wl_surface().clone())
            .filter(|s| with_renderer_surface_state(s, |st| st.buffer().is_some()) == Some(true))
            .collect()
    }

    /// Bring the window manager in line with the windows that exist now.
    fn refresh(&mut self) {
        let drawn = self.drawn_toplevels();
        self.ids.retain(|(s, _)| drawn.contains(s));
        let mut current = Vec::new();
        for s in drawn {
            let id = match self.ids.iter().find(|(x, _)| *x == s) {
                Some((_, id)) => *id,
                None => {
                    self.next_id += 1;
                    self.ids.push((s.clone(), self.next_id));
                    self.next_id
                }
            };
            let size = with_renderer_surface_state(&s, |st| st.surface_size())
                .flatten()
                .unwrap_or_default();
            // ⚠️ THE SURFACE IS NOT THE WINDOW. A toolkit that draws its own decorations (GTK,
            // and so Firefox) draws a shadow round the window inside its surface, and says
            // where the window really is with `set_window_geometry`. Placing the surface put
            // the visible window ~40 px off its place. The window manager lays out the window
            // geometry; the surface is drawn that far up and left of it.
            let geometry = with_states(&s, |states| {
                states
                    .cached_state
                    .get::<SurfaceCachedState>()
                    .current()
                    .geometry
            })
            .unwrap_or_else(|| Rectangle::from_size(size));
            self.offsets.retain(|(i, _)| *i != id);
            self.offsets.push((id, (geometry.loc.x, geometry.loc.y)));
            // Too big for the screen: ask it to fit, once per size asked for.
            if let Some(size) = wm::fit(
                geometry.size.w,
                geometry.size.h,
                self.screen.0,
                self.screen.1,
            ) && !self.asked.contains(&(id, size))
                && let Some(toplevel) = self
                    .xdg_shell_state
                    .toplevel_surfaces()
                    .iter()
                    .find(|t| *t.wl_surface() == s)
            {
                toplevel.with_pending_state(|state| state.size = Some(size.into()));
                toplevel.send_configure();
                self.asked.retain(|(i, _)| *i != id);
                self.asked.push((id, size));
            }
            current.push(Window {
                id,
                w: geometry.size.w,
                h: geometry.size.h,
            });
        }
        self.offsets
            .retain(|(i, _)| current.iter().any(|w| w.id == *i));
        self.asked
            .retain(|(i, _)| current.iter().any(|w| w.id == *i));
        self.wm.sync(&current);
    }

    /// Where a placed window's surface goes: up and left of its window geometry by the
    /// geometry's offset inside the surface.
    fn surface_origin(&self, placed: &wm::Placed) -> (i32, i32) {
        let (dx, dy) = self
            .offsets
            .iter()
            .find(|(i, _)| *i == placed.id)
            .map_or((0, 0), |(_, o)| *o);
        (placed.x - dx, placed.y - dy)
    }

    fn surface(&self, id: u32) -> Option<WlSurface> {
        self.ids
            .iter()
            .find(|(_, i)| *i == id)
            .map(|(s, _)| s.clone())
    }
}

/// The pointer's size on screen, in pixels.
pub const CURSOR: i32 = 12;

/// The left mouse button's code (linux/input-event-codes.h `BTN_LEFT`).
pub const BTN_LEFT: u32 = 0x110;

/// Everything the event loop needs to keep clients moving.
pub struct Server {
    display: Display<App>,
    listener: ListeningSocket,
    state: App,
    keyboard: KeyboardHandle<App>,
    pointer: PointerHandle<App>,
    /// Where the pointer is on the screen, and whether anything has moved it yet.
    pointer_at: Point<f64, Logical>,
    pointer_moved: bool,
    /// The window the keyboard was last given to, so focus follows the window manager
    /// when a window goes away on its own.
    keyboard_on: Option<u32>,
    /// When the server started (frame-callback timestamps count from it), and when the
    /// visible surfaces were last told their frame was shown.
    started: Instant,
    last_frames: Instant,
    /// The screen's size, which the pointer stays inside.
    width: u32,
    height: u32,
}

impl Server {
    /// Accept, dispatch and flush once, and refresh the window list.
    pub fn turn(&mut self) -> Result<(), String> {
        if let Some(stream) = self.listener.accept().map_err(|e| format!("accept: {e}"))? {
            self.display
                .handle()
                .insert_client(stream, Arc::new(ClientState::default()))
                .map_err(|e| format!("adding a client: {e}"))?;
        }
        self.display
            .dispatch_clients(&mut self.state)
            .map_err(|e| format!("dispatching: {e}"))?;
        self.state.refresh();
        if self.state.wm.focused() != self.keyboard_on {
            self.apply_focus();
        }
        // About 60 times a second, tell every visible surface its frame was shown.
        if self.last_frames.elapsed() >= Duration::from_millis(16) {
            self.send_frames();
            self.last_frames = Instant::now();
        }
        self.display
            .flush_clients()
            .map_err(|e| format!("flushing: {e}"))?;
        Ok(())
    }

    /// ⚠️ A CLIENT DRAWS ITS NEXT FRAME ONLY WHEN TOLD THE LAST ONE WAS SHOWN. Each commit can
    /// ask for a frame callback (`wl_surface.frame`), and a toolkit waits for its `done` before
    /// drawing again. Nothing answered them, so every client froze on its first frame: GTK's
    /// widget factory looked right (one frame was all its test needed), and Firefox put up its
    /// window and never painted the page (found by `test-browser.sh`). Answer them for every
    /// surface of every window on the screen, subsurfaces included.
    fn send_frames(&mut self) {
        let time = self.started.elapsed().as_millis() as u32;
        for id in self.state.wm.layout().iter().map(|p| p.id) {
            let Some(surface) = self.state.surface(id) else {
                continue;
            };
            with_surface_tree_downward(
                &surface,
                (),
                |_, _, ()| TraversalAction::DoChildren(()),
                |_, states, ()| {
                    let callbacks: Vec<_> = states
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .current()
                        .frame_callbacks
                        .split_off(0);
                    for callback in callbacks {
                        callback.done(time);
                    }
                },
                |_, _, ()| true,
            );
        }
    }

    /// The screen's size.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// How many windows are on the screen.
    pub fn windows(&self) -> usize {
        self.state.wm.layout().len()
    }

    /// Render the screen. `cursor` draws the pointer, once something has moved it: the
    /// live display shows it, while the headless frames tests read leave it out.
    pub fn render(&self, width: u32, height: u32, cursor: bool) -> Result<ShellFrame, String> {
        let at = (cursor && self.pointer_moved)
            .then_some((self.pointer_at.x as i32, self.pointer_at.y as i32));
        render(&self.state, width, height, at)
    }

    /// Keep clients moving for a while, so they see what was just sent.
    fn settle(&mut self, d: Duration) -> Result<(), String> {
        let end = Instant::now() + d;
        while Instant::now() < end {
            self.turn()?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    // INPUT. Real devices (live.rs, through libinput) and the script's commands come in
    // through these three and nothing else, so a test of the script is a test of what a
    // person's mouse and keyboard do (AGENTS.md H3).

    /// Move the pointer to a point on the screen, kept inside it, and tell the window under
    /// it where the pointer is in its own coordinates.
    pub fn pointer_to(&mut self, x: f64, y: f64, time: u32) {
        let x = x.clamp(0.0, f64::from(self.width) - 1.0);
        let y = y.clamp(0.0, f64::from(self.height) - 1.0);
        self.pointer_at = (x, y).into();
        self.pointer_moved = true;
        let focus = self.state.wm.hit(x as i32, y as i32).and_then(|id| {
            let placed = self.state.wm.layout().into_iter().find(|p| p.id == id)?;
            let surface = self.state.surface(id)?;
            let (x, y) = self.state.surface_origin(&placed);
            Some((surface, Point::from((f64::from(x), f64::from(y)))))
        });
        let serial = SERIAL_COUNTER.next_serial();
        self.pointer.motion(
            &mut self.state,
            focus,
            &MotionEvent {
                location: self.pointer_at,
                serial,
                time,
            },
        );
        self.pointer.frame(&mut self.state);
    }

    /// Move the pointer by an amount (a mouse, as opposed to a tablet or a touchpad
    /// in absolute mode).
    pub fn pointer_by(&mut self, dx: f64, dy: f64, time: u32) {
        let (x, y) = (self.pointer_at.x + dx, self.pointer_at.y + dy);
        self.pointer_to(x, y, time);
    }

    /// A mouse button. Pressing focuses the window under the pointer (the Classic feel:
    /// click to focus), and the window gets the button either way.
    pub fn pointer_button(&mut self, button: u32, pressed: bool, time: u32) {
        if pressed {
            let at = self.pointer_at;
            self.state.wm.click(at.x as i32, at.y as i32);
            self.apply_focus();
        }
        let serial = SERIAL_COUNTER.next_serial();
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        self.pointer.button(
            &mut self.state,
            &ButtonEvent {
                serial,
                time,
                button,
                state,
            },
        );
        self.pointer.frame(&mut self.state);
    }

    /// A key, by its Linux input code (`KEY_A` is 30). The focused window gets it, except
    /// Ctrl+Alt+F1..F12, which the shell keeps: it returns the virtual terminal to switch
    /// to, so a person can always reach the root console on VT2.
    pub fn key(&mut self, code: u32, pressed: bool, time: u32) -> Option<i32> {
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let serial = SERIAL_COUNTER.next_serial();
        self.keyboard
            .input(
                &mut self.state,
                Keycode::new(code + 8),
                state,
                serial,
                time,
                |_, _, keysym| {
                    let sym = keysym.modified_sym().raw();
                    let (first, last) = (
                        Keysym::XF86_Switch_VT_1.raw(),
                        Keysym::XF86_Switch_VT_12.raw(),
                    );
                    if pressed && (first..=last).contains(&sym) {
                        FilterResult::Intercept(Some((sym - first + 1) as i32))
                    } else {
                        FilterResult::Forward
                    }
                },
            )
            .flatten()
    }

    /// Give keyboard focus to the window the window manager says has it.
    fn apply_focus(&mut self) {
        let surface = self
            .state
            .wm
            .focused()
            .and_then(|id| self.state.surface(id));
        let serial = SERIAL_COUNTER.next_serial();
        self.keyboard.set_focus(&mut self.state, surface, serial);
        self.keyboard_on = self.state.wm.focused();
    }

    fn write_frame(&self, path: &Path, width: u32, height: u32) -> Result<(), String> {
        let frame = self.render(width, height, false)?;
        // PNG to look at; PPM for a test to read without a decoder.
        let bytes = if path.extension().is_some_and(|e| e == "ppm") {
            frame.to_ppm()
        } else {
            frame.to_png()
        };
        std::fs::write(path, bytes).map_err(|e| format!("writing {}: {e}", path.display()))
    }

    /// Run one script command.
    fn command(&mut self, line: &str, opts: &Options) -> Result<(), String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> Result<i32, String> {
            words
                .get(i)
                .and_then(|w| w.parse().ok())
                .ok_or_else(|| format!("script: `{line}` needs a number at word {}", i + 1))
        };
        match words.as_slice() {
            ["click", ..] => {
                self.pointer_to(f64::from(num(1)?), f64::from(num(2)?), 0);
                self.pointer_button(BTN_LEFT, true, 0);
                self.pointer_button(BTN_LEFT, false, 0);
            }
            ["key", codes @ ..] if !codes.is_empty() => {
                // A chord: press each in order, release in reverse, the way hands do it.
                let codes = codes
                    .iter()
                    .map(|c| c.parse::<u32>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| format!("script: `{line}` needs key codes"))?;
                for &code in &codes {
                    if let Some(vt) = self.key(code, true, 0) {
                        // Headless has no virtual terminals; say what a person would get.
                        eprintln!("flow-compositor: switch to VT{vt} requested");
                    }
                }
                for &code in codes.iter().rev() {
                    self.key(code, false, 0);
                }
            }
            ["cycle"] => {
                self.state.wm.cycle();
                self.apply_focus();
            }
            ["close"] => {
                let id = self
                    .state
                    .wm
                    .focused()
                    .ok_or("script: `close` with no focused window")?;
                let surface = self
                    .state
                    .surface(id)
                    .ok_or("script: the focused window is gone")?;
                let toplevel = self
                    .state
                    .xdg_shell_state
                    .toplevel_surfaces()
                    .iter()
                    .find(|t| *t.wl_surface() == surface)
                    .cloned()
                    .ok_or("script: the focused window has no toplevel")?;
                // Ask, the way a close button does; the client decides to go.
                toplevel.send_close();
                let end = Instant::now() + opts.timeout;
                while self.state.surface(id).is_some() {
                    if Instant::now() > end {
                        return Err(format!(
                            "window {id} did not close within {:?}",
                            opts.timeout
                        ));
                    }
                    self.settle(Duration::from_millis(20))?;
                }
                self.apply_focus();
            }
            ["frame", path] => self.write_frame(Path::new(path), opts.width, opts.height)?,
            ["settle", _] => self.settle(Duration::from_millis(num(1)?.max(0) as u64))?,
            [] => {}
            _ => return Err(format!("script: unknown command `{line}`")),
        }
        // Let clients see the result before the next command.
        self.settle(Duration::from_millis(50))
    }
}

impl Server {
    /// Listen on `$XDG_RUNTIME_DIR/<socket>` with a seat that has a keyboard and a pointer,
    /// for a screen of `width` x `height`.
    pub fn new(socket: &str, width: u32, height: u32) -> Result<Self, String> {
        let display: Display<App> = Display::new().map_err(|e| format!("display: {e}"))?;
        let dh = display.handle();
        let mut seat_state = SeatState::new();
        let mut seat: Seat<App> = seat_state.new_wl_seat(&dh, "seat0");
        // The keymap comes from xkeyboard-config's data files (the `xkb-data` package).
        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25)
            .map_err(|e| format!("creating the keyboard (is xkb-data installed?): {e}"))?;
        let pointer = seat.add_pointer();
        let state = App {
            compositor_state: CompositorState::new::<App>(&dh),
            xdg_shell_state: XdgShellState::new::<App>(&dh),
            shm_state: ShmState::new::<App>(&dh, vec![]),
            seat_state,
            data_device_state: DataDeviceState::new::<App>(&dh),
            _output_manager_state: OutputManagerState::new_with_xdg_output::<App>(&dh),
            output: Output::new(
                "flow-0".into(),
                PhysicalProperties {
                    // Unknown until the live display says (E13); 0 tells clients so.
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "MorpheOS".into(),
                    model: "Flow".into(),
                },
            ),
            wm: Manager::default(),
            ids: Vec::new(),
            offsets: Vec::new(),
            asked: Vec::new(),
            screen: (width as i32, height as i32),
            next_id: 0,
        };
        state.output.create_global::<App>(&dh);
        set_mode(&state.output, width, height);
        let listener =
            ListeningSocket::bind(socket).map_err(|e| format!("binding socket {socket}: {e}"))?;
        eprintln!("flow-compositor: listening on {socket}");
        Ok(Self {
            display,
            listener,
            state,
            keyboard,
            pointer,
            // The pointer starts in the middle of the screen, as people expect.
            pointer_at: (f64::from(width) / 2.0, f64::from(height) / 2.0).into(),
            pointer_moved: false,
            keyboard_on: None,
            started: Instant::now(),
            last_frames: Instant::now(),
            width,
            height,
        })
    }

    /// The screen changed size (the live display's mode is known only once it is open).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        set_mode(&self.state.output, width, height);
        self.state.screen = (width as i32, height as i32);
        self.pointer_at = (f64::from(width) / 2.0, f64::from(height) / 2.0).into();
    }
}

/// Tell clients the screen's size: one mode, preferred and current, at the origin.
fn set_mode(output: &Output, width: u32, height: u32) {
    let mode = OutputMode {
        size: (width as i32, height as i32).into(),
        refresh: 60_000,
    };
    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
}

/// Run until `wait_for` windows are drawn, then write the frame or run the script. An
/// error means the run did not do what it was asked.
pub fn run(opts: &Options) -> Result<(), String> {
    let mut server = Server::new(&opts.socket, opts.width, opts.height)?;

    if let Some(device) = &opts.drm {
        return crate::live::run(server, device);
    }

    let deadline = Instant::now() + opts.timeout;
    let start = Instant::now();
    while server.state.drawn_toplevels().len() < opts.wait_for {
        if Instant::now() > deadline {
            return Err(format!(
                "{} of {} window(s) drew a buffer within {:?}; no frame written",
                server.state.drawn_toplevels().len(),
                opts.wait_for,
                opts.timeout
            ));
        }
        server.turn()?;
        std::thread::sleep(Duration::from_millis(5));
    }
    eprintln!(
        "flow-compositor: {} window(s) drawn after {:?}",
        opts.wait_for,
        start.elapsed()
    );
    match (&opts.script, &opts.out) {
        (Some(script), _) => {
            for line in script.lines() {
                server.command(line.trim(), opts)?;
            }
        }
        (None, Some(out)) => server.write_frame(out, opts.width, opts.height)?,
        (None, None) => return Err("nothing to do: give --out or --script".into()),
    }
    Ok(())
}

/// Render the windows where the window manager put them, over the shell's background,
/// with a border round the focused one and the pointer on top, and read the pixels back.
fn render(
    state: &App,
    width: u32,
    height: u32,
    cursor: Option<(i32, i32)>,
) -> Result<ShellFrame, String> {
    let err = |what: &'static str| {
        move |e: smithay::backend::renderer::pixman::PixmanError| format!("{what}: {e}")
    };
    let mut renderer = PixmanRenderer::new().map_err(err("starting the pixman renderer"))?;
    let buffer_size: Size<i32, Buffer> = (width as i32, height as i32).into();
    let size: Size<i32, Physical> = (width as i32, height as i32).into();
    let mut image = renderer
        .create_buffer(Fourcc::Argb8888, buffer_size)
        .map_err(err("creating the frame"))?;
    let full = Rectangle::from_size(size);

    let layout = state.wm.layout();
    let mut elements: Vec<WaylandSurfaceRenderElement<PixmanRenderer>> = Vec::new();
    // The first element in the list is drawn on top, so the last-opened window goes first.
    for placed in layout.iter().rev() {
        if let Some(surface) = state.surface(placed.id) {
            elements.extend(render_elements_from_surface_tree(
                &mut renderer,
                &surface,
                state.surface_origin(placed),
                1.0,
                1.0,
                Kind::Unspecified,
            ));
        }
    }
    let focus_border = state
        .wm
        .focused()
        .and_then(|id| layout.iter().find(|p| p.id == id))
        .map(|p| {
            Rectangle::new(
                (p.x - wm::BORDER, p.y - wm::BORDER).into(),
                (p.w + 2 * wm::BORDER, p.h + 2 * wm::BORDER).into(),
            )
        });

    {
        let mut target = renderer
            .bind(&mut image)
            .map_err(err("binding the frame"))?;
        let mut frame = renderer
            .render(&mut target, size, Transform::Normal)
            .map_err(err("starting the frame"))?;
        let (_, r, g, b) = palette::BACKGROUND.channels();
        let bg = Color32F::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        );
        frame.clear(bg, &[full]).map_err(err("clearing"))?;
        if let Some(border) = focus_border {
            let (_, r, g, b) = palette::ACCENT.channels();
            let accent = Color32F::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            );
            // ⚠️ draw_solid's damage is RELATIVE TO `dst` (smithay adds dst.loc to it), unlike
            // clear's. Passing the whole screen here filled everything below and right of
            // the border's corner. Found by tests/windows.rs.
            frame
                .draw_solid(border, &[Rectangle::from_size(border.size)], accent)
                .map_err(err("drawing the focus border"))?;
        }
        draw_render_elements(&mut frame, 1.0, &elements, &[full])
            .map_err(err("drawing the windows"))?;
        if let Some((x, y)) = cursor {
            // The pointer: a white square with a dark edge, its top-left corner on the
            // point. Plain, but a person can see where they are. A themed cursor (and
            // clients setting their own) comes with the work that needs it.
            let (_, r, g, b) = palette::TEXT.channels();
            let fill = Color32F::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            );
            for (inset, c) in [(0, Color32F::new(0.0, 0.0, 0.0, 1.0)), (1, fill)] {
                let size = CURSOR - 2 * inset;
                let rect = Rectangle::new((x + inset, y + inset).into(), (size, size).into());
                frame
                    .draw_solid(rect, &[Rectangle::from_size(rect.size)], c)
                    .map_err(err("drawing the pointer"))?;
            }
        }
        // Software rendering finishes synchronously; the sync point has nothing to wait on.
        let _sync = frame.finish().map_err(err("finishing the frame"))?;
    }

    let target = renderer.bind(&mut image).map_err(err("binding to read"))?;
    let mapping = renderer
        .copy_framebuffer(&target, Rectangle::from_size(buffer_size), Fourcc::Argb8888)
        .map_err(err("reading the frame back"))?;
    let bytes = renderer
        .map_texture(&mapping)
        .map_err(err("mapping the frame"))?;
    let stride = bytes.len() / height as usize;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in bytes.chunks_exact(stride).take(height as usize) {
        let (row_px, _) = row.as_chunks::<4>();
        for px in row_px.iter().take(width as usize) {
            // ARGB8888 is a little-endian u32: B, G, R, A in memory.
            pixels.push(u32::from_le_bytes(*px));
        }
    }
    ShellFrame::from_pixels(width, height, pixels)
        .ok_or_else(|| "the frame read back has the wrong size".to_string())
}
