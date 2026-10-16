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
//! Popups (menus, dropdowns) are placed by their positioner, kept on the screen, drawn on top
//! and given the pointer first; a click outside them closes them. Every visible surface is
//! told when its frame was shown (frame callbacks), or clients stop drawing (AGENTS.md H17).

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
use smithay::reexports::wayland_server::protocol::{
    wl_buffer, wl_output, wl_seat, wl_surface::WlSurface,
};
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
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
};

use flow_shell::Frame as ShellFrame;
use flow_shell::palette;

use flow_compositor::splash;
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
    /// The command that opens the Wi-Fi picker, from the bar's button or Super+W (E5).
    pub launch_wifi: Option<String>,
    /// The command that opens the file view, from the bar's Files button or Super+E (E20).
    pub launch_files: Option<String>,
    /// Render the boot splash (E19) at this many seconds after the compositor started,
    /// instead of the shell. `None` renders the shell; `Some(0)` is the first instant.
    pub splash: Option<f32>,
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
    /// Open popups (menus, dropdowns, tooltips), oldest first, each with its place relative
    /// to its parent's window geometry. A popup's parent is a window or another popup.
    popups: Vec<(PopupSurface, Rectangle<i32, Logical>)>,
    /// The size each window was last asked to take, so a window is asked once, not on every
    /// turn while it gets round to it.
    asked: Vec<(u32, (i32, i32))>,
    /// The screen's size, which windows are kept inside.
    screen: (i32, i32),
    /// Where the pointer is, for a client that asks to be moved from it (its title bar).
    pointer: (i32, i32),
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

    // POPUPS (menus, dropdowns, tooltips). A popup is placed by its positioner relative to its
    // parent's window geometry, kept on the screen, drawn above everything, and given the
    // pointer before any window. A click outside every popup closes them all, as every
    // desktop does; that is the whole of what `grab` asks for here (E4).
    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        let geometry = self.place_popup(&surface, positioner);
        surface.with_pending_state(|state| state.geometry = geometry);
        // A popup may not draw before its first configure.
        let _ = surface.send_configure();
        self.popups.push((surface, geometry));
    }
    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        let geometry = self.place_popup(&surface, positioner);
        surface.with_pending_state(|state| {
            state.geometry = geometry;
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
        if let Some(entry) = self.popups.iter_mut().find(|(p, _)| *p == surface) {
            entry.1 = geometry;
        }
    }
    fn popup_destroyed(&mut self, surface: PopupSurface) {
        self.popups.retain(|(p, _)| *p != surface);
    }

    // MOVE AND FULLSCREEN ASKED FOR BY THE CLIENT (E9). A toolkit that draws its own title
    // bar (GTK, and so Firefox) asks to be moved when it is dragged by it, and asks to be
    // fullscreen on F11. The window manager decides; these only carry the request to it.
    fn move_request(&mut self, surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        if let Some(id) = self.id_of(surface.wl_surface()) {
            let (x, y) = self.pointer;
            self.wm.begin_move(id, x, y);
        }
    }
    fn fullscreen_request(
        &mut self,
        surface: ToplevelSurface,
        _output: Option<wl_output::WlOutput>,
    ) {
        match self.id_of(surface.wl_surface()) {
            Some(id) if self.wm.set_fullscreen(id, true) => self.tell_fullscreen(),
            // Nothing changed, but a request must be answered with a configure.
            _ => {
                surface.send_configure();
            }
        }
    }
    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        match self.id_of(surface.wl_surface()) {
            Some(id) if self.wm.set_fullscreen(id, false) => self.tell_fullscreen(),
            _ => {
                surface.send_configure();
            }
        }
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
        self.wm.set_screen(self.screen.0, self.screen.1);
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
            // Too big for the screen: ask it to fit, once per size asked for. (A fullscreen
            // window is exactly the screen's size, and asked to be.)
            if self.wm.fullscreen() != Some(id)
                && let Some(size) = wm::fit(
                    geometry.size.w,
                    geometry.size.h,
                    self.screen.0,
                    self.screen.1,
                )
                && !self.asked.contains(&(id, size))
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
        self.popups.retain(|(p, _)| p.alive());
        self.wm.sync(&current);
    }

    /// Where a surface's window geometry starts on the screen: a window's is where the window
    /// manager placed it; a popup's is its parent's plus the popup's place in it.
    fn window_origin(&self, surface: &WlSurface) -> Option<(i32, i32)> {
        if let Some((_, id)) = self.ids.iter().find(|(s, _)| s == surface) {
            let placed = self.wm.layout().into_iter().find(|p| p.id == *id)?;
            return Some((placed.x, placed.y));
        }
        let (popup, geometry) = self
            .popups
            .iter()
            .find(|(p, _)| p.wl_surface() == surface)?;
        let parent = popup.get_parent_surface()?;
        let (px, py) = self.window_origin(&parent)?;
        Some((px + geometry.loc.x, py + geometry.loc.y))
    }

    /// Where a popup goes, inside its parent, kept on the screen where it can be.
    fn place_popup(
        &self,
        surface: &PopupSurface,
        positioner: PositionerState,
    ) -> Rectangle<i32, Logical> {
        match surface
            .get_parent_surface()
            .and_then(|p| self.window_origin(&p))
        {
            // The screen, in the parent's window-geometry coordinates.
            Some((px, py)) => positioner
                .get_unconstrained_geometry(Rectangle::new((-px, -py).into(), self.screen.into())),
            None => positioner.get_geometry(),
        }
    }

    /// Where a surface is drawn: its window geometry's origin, less where that geometry
    /// starts inside the surface (a toolkit's shadow).
    fn draw_origin(&self, surface: &WlSurface) -> Option<(i32, i32)> {
        let (x, y) = self.window_origin(surface)?;
        let (dx, dy) = with_states(surface, |states| {
            states
                .cached_state
                .get::<SurfaceCachedState>()
                .current()
                .geometry
        })
        .map_or((0, 0), |g| (g.loc.x, g.loc.y));
        Some((x - dx, y - dy))
    }

    /// The open popup under a point, topmost first, with where its surface is drawn.
    fn popup_at(&self, x: i32, y: i32) -> Option<(WlSurface, (i32, i32))> {
        self.popups.iter().rev().find_map(|(popup, geometry)| {
            let surface = popup.wl_surface();
            let (ox, oy) = self.window_origin(surface)?;
            let inside = x >= ox && y >= oy && x < ox + geometry.size.w && y < oy + geometry.size.h;
            inside.then(|| {
                (
                    surface.clone(),
                    self.draw_origin(surface).unwrap_or((ox, oy)),
                )
            })
        })
    }

    /// Close every open popup, newest first (a click landed outside them).
    fn dismiss_popups(&mut self) {
        for (popup, _) in self.popups.drain(..).rev() {
            popup.send_popup_done();
        }
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

    fn id_of(&self, surface: &WlSurface) -> Option<u32> {
        self.ids.iter().find(|(s, _)| s == surface).map(|(_, i)| *i)
    }

    /// Tell every window whose fullscreen state the window manager changed: fullscreen at
    /// the screen's size, or back to the size it had before.
    fn tell_fullscreen(&mut self) {
        let full = self.wm.fullscreen();
        for (surface, id) in self.ids.clone() {
            let Some(toplevel) = self
                .xdg_shell_state
                .toplevel_surfaces()
                .iter()
                .find(|t| *t.wl_surface() == surface)
                .cloned()
            else {
                continue;
            };
            let want = full == Some(id);
            let has = toplevel
                .with_pending_state(|state| state.states.contains(xdg_toplevel::State::Fullscreen));
            if want == has {
                continue;
            }
            let screen = self.screen;
            let before = self.wm.size_before_fullscreen(id);
            toplevel.with_pending_state(|state| {
                if want {
                    state.states.set(xdg_toplevel::State::Fullscreen);
                    state.size = Some(screen.into());
                } else {
                    state.states.unset(xdg_toplevel::State::Fullscreen);
                    state.size = before.map(Into::into);
                }
            });
            toplevel.send_configure();
        }
    }

    /// Tell one window its new size (a resize the shell started): configure it, and the
    /// client redraws at that size.
    fn tell_size(&mut self, id: u32, w: i32, h: i32) {
        let Some((surface, _)) = self.ids.iter().find(|(_, i)| *i == id).cloned() else {
            return;
        };
        let Some(toplevel) = self
            .xdg_shell_state
            .toplevel_surfaces()
            .iter()
            .find(|t| *t.wl_surface() == surface)
            .cloned()
        else {
            return;
        };
        toplevel.with_pending_state(|state| {
            state.size = Some((w, h).into());
        });
        toplevel.send_configure();
    }

    /// The window whose toplevel says it is `app_id` (`xdg_toplevel.set_app_id`).
    fn window_with_app_id(&self, app_id: &str) -> Option<u32> {
        self.xdg_shell_state
            .toplevel_surfaces()
            .iter()
            .find(|t| {
                with_states(t.wl_surface(), |states| {
                    states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .and_then(|d| d.lock().ok().and_then(|d| d.app_id.clone()))
                        .is_some_and(|a| a == app_id)
                })
            })
            .and_then(|t| self.id_of(t.wl_surface()))
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

/// A key combination the shell keeps for itself.
enum Shortcut {
    Vt(i32),
    Cycle,
    Fullscreen,
    Wifi,
    Files,
}

/// The Wi-Fi picker's `app_id`, by which the shell finds one already open.
pub const WIFI_APP_ID: &str = "morpheos.wifi";

/// The file view's `app_id`, by which the shell finds one already open.
pub const FILES_APP_ID: &str = "morpheos.files";

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
    /// A button press started a move the shell made (an edge, or Super): its release is the
    /// shell's too, not the window's.
    drag_button: Option<u32>,
    resize_button: Option<u32>,
    /// When the server started (frame-callback timestamps count from it), and when the
    /// visible surfaces were last told their frame was shown.
    started: Instant,
    last_frames: Instant,
    /// The screen's size, which the pointer stays inside.
    width: u32,
    height: u32,
    /// The socket clients connect to, and the Wi-Fi picker the bar opens (E5): its command,
    /// and the pickers this shell started, reaped as they exit.
    socket: String,
    launch_wifi: Option<Vec<String>>,
    launch_files: Option<Vec<String>>,
    launched: Vec<std::process::Child>,
}

impl Drop for Server {
    /// A picker the shell started goes with it: nothing else would close it.
    fn drop(&mut self) {
        for child in &mut self.launched {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
        // Reap the pickers that have closed, or each stays a zombie.
        self.launched
            .retain_mut(|c| matches!(c.try_wait(), Ok(None)));
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
        let windows = self
            .state
            .wm
            .layout()
            .iter()
            .filter_map(|p| self.state.surface(p.id))
            .collect::<Vec<_>>();
        let popups = self
            .state
            .popups
            .iter()
            .map(|(p, _)| p.wl_surface().clone());
        for surface in windows.into_iter().chain(popups) {
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

    /// Whether the boot splash is still showing; the live display logs when it ends.
    pub fn splash_showing(&self) -> bool {
        splash::shown_at_boot() && splash::state(self.started.elapsed()).showing
    }

    /// Render the screen now (the live display): `cursor` draws the pointer, once something
    /// has moved it, while the headless frames tests read leave it out. The splash shows
    /// for the first moments, from when the server started.
    pub fn render(&self, width: u32, height: u32, cursor: bool) -> Result<ShellFrame, String> {
        self.render_at(width, height, cursor, self.started.elapsed())
    }

    /// Render the screen as it looked `elapsed` after the compositor started: the splash
    /// while it is showing, then the shell. The headless tests render a fixed instant (the
    /// splash at zero, the shell once it is over) so the frame does not depend on how long
    /// a client took to draw.
    fn render_at(
        &self,
        width: u32,
        height: u32,
        cursor: bool,
        elapsed: Duration,
    ) -> Result<ShellFrame, String> {
        let at = (cursor && self.pointer_moved)
            .then_some((self.pointer_at.x as i32, self.pointer_at.y as i32));
        render(&self.state, width, height, at, elapsed)
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
        self.state.pointer = (x as i32, y as i32);
        if self.state.wm.dragging() {
            self.state.wm.drag_to(x as i32, y as i32);
        }
        if let Some(id) = self.state.wm.resizing_id()
            && let Some((_, _, w, h)) = self.state.wm.resize_to(x as i32, y as i32)
        {
            self.state.tell_size(id, w, h);
        }
        let on_popup = self
            .state
            .popup_at(x as i32, y as i32)
            .map(|(surface, (px, py))| (surface, Point::from((f64::from(px), f64::from(py)))));
        let focus = on_popup.or_else(|| {
            self.state.wm.hit(x as i32, y as i32).and_then(|id| {
                let placed = self.state.wm.layout().into_iter().find(|p| p.id == id)?;
                let surface = self.state.surface(id)?;
                let (x, y) = self.state.surface_origin(&placed);
                Some((surface, Point::from((f64::from(x), f64::from(y)))))
            })
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
        // A move the shell started: its buttons are the shell's, not the window's (E9).
        if !pressed && self.drag_button == Some(button) {
            self.drag_button = None;
            self.state.wm.end_drag();
            return;
        }
        if !pressed && self.resize_button == Some(button) {
            self.resize_button = None;
            self.state.wm.end_resize();
            return;
        }
        // The bar's Files button is the shell's: the press opens the file view, and neither it
        // nor its release reaches a window.
        if pressed
            && button == BTN_LEFT
            && self
                .state
                .wm
                .on_files_button(self.pointer_at.x as i32, self.pointer_at.y as i32)
        {
            self.state.dismiss_popups();
            self.drag_button = Some(button);
            self.open_files();
            return;
        }
        // The bar's Wi-Fi button is the shell's: the press opens the picker, and neither it
        // nor its release reaches a window.
        if pressed
            && button == BTN_LEFT
            && self
                .state
                .wm
                .on_wifi_button(self.pointer_at.x as i32, self.pointer_at.y as i32)
        {
            self.state.dismiss_popups();
            self.drag_button = Some(button);
            self.open_wifi();
            return;
        }
        if pressed && button == BTN_LEFT {
            let at = self.pointer_at;
            let with_super = self.keyboard.modifier_state().logo;
            if self.state.popup_at(at.x as i32, at.y as i32).is_none() {
                // A corner press resizes; an edge press moves.
                if let Some((id, corner)) = self.state.wm.corner_at(at.x as i32, at.y as i32)
                    && self.state.wm.begin_resize(id, corner)
                {
                    self.state.dismiss_popups();
                    self.resize_button = Some(button);
                    self.apply_focus();
                    return;
                }
                if self
                    .state
                    .wm
                    .begin_drag(at.x as i32, at.y as i32, with_super)
                    .is_some()
                {
                    self.state.dismiss_popups();
                    self.drag_button = Some(button);
                    self.apply_focus();
                    return;
                }
            }
        }
        // A client's own move (its title bar) ends when its button comes up; the window
        // saw the press, so it sees the release too.
        if !pressed && self.state.wm.dragging() {
            self.state.wm.end_drag();
        }
        if pressed {
            let at = self.pointer_at;
            if self.state.popup_at(at.x as i32, at.y as i32).is_none() {
                // Outside every popup: they close, and the click does what it would have.
                self.state.dismiss_popups();
                self.state.wm.click(at.x as i32, at.y as i32);
                self.apply_focus();
                // The pointer may now be over what was under a popup.
                self.pointer_to(at.x, at.y, time);
            }
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
    /// the shell's own shortcuts, which it keeps:
    ///   * Ctrl+Alt+F1..F12: returns the virtual terminal to switch to, so a person can
    ///     always reach the root console on VT2;
    ///   * Alt+Tab: the next window, in front (E9);
    ///   * Super+F: the focused window fullscreen, and back (E9);
    ///   * Super+W: the Wi-Fi picker, as the bar's button opens it (E5).
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
                |_, modifiers, keysym| {
                    let sym = keysym.modified_sym().raw();
                    let (first, last) = (
                        Keysym::XF86_Switch_VT_1.raw(),
                        Keysym::XF86_Switch_VT_12.raw(),
                    );
                    // The unmodified symbol, so Shift or a layout does not hide the letter.
                    let plain = keysym.raw_latin_sym_or_raw_current_sym().map(|k| k.raw());
                    if !pressed {
                        FilterResult::Forward
                    } else if (first..=last).contains(&sym) {
                        FilterResult::Intercept(Shortcut::Vt((sym - first + 1) as i32))
                    } else if modifiers.alt && plain == Some(Keysym::Tab.raw()) {
                        FilterResult::Intercept(Shortcut::Cycle)
                    } else if modifiers.logo && plain == Some(Keysym::f.raw()) {
                        FilterResult::Intercept(Shortcut::Fullscreen)
                    } else if modifiers.logo && plain == Some(Keysym::w.raw()) {
                        FilterResult::Intercept(Shortcut::Wifi)
                    } else if modifiers.logo && plain == Some(Keysym::e.raw()) {
                        FilterResult::Intercept(Shortcut::Files)
                    } else {
                        FilterResult::Forward
                    }
                },
            )
            .and_then(|shortcut| match shortcut {
                Shortcut::Vt(n) => Some(n),
                Shortcut::Cycle => {
                    self.state.wm.cycle();
                    self.apply_focus();
                    None
                }
                Shortcut::Fullscreen => {
                    if let Some(id) = self.state.wm.focused() {
                        let on = self.state.wm.fullscreen() != Some(id);
                        if self.state.wm.set_fullscreen(id, on) {
                            self.state.tell_fullscreen();
                        }
                    }
                    None
                }
                Shortcut::Wifi => {
                    self.open_wifi();
                    None
                }
                Shortcut::Files => {
                    self.open_files();
                    None
                }
            })
    }

    /// Open the file view (the bar's Files button, Super+E). There is only ever one: one
    /// already on the screen comes to the front, and while one this shell started is still
    /// starting, nothing more is started.
    fn open_files(&mut self) {
        if let Some(id) = self.state.window_with_app_id(FILES_APP_ID) {
            self.state.wm.activate(id);
            self.apply_focus();
            return;
        }
        let Some(cmd) = &self.launch_files else {
            eprintln!("flow-compositor: no file view to open (no --launch-files)");
            return;
        };
        if !self.launched.is_empty() {
            return;
        }
        match std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .env("WAYLAND_DISPLAY", &self.socket)
            .spawn()
        {
            Ok(child) => {
                eprintln!("flow-compositor: opening the file view");
                self.launched.push(child);
            }
            Err(e) => eprintln!(
                "flow-compositor: could not open the file view ({}): {e}",
                cmd[0]
            ),
        }
    }

    /// Open the Wi-Fi picker (the bar's button, Super+W). There is only ever one: one already
    /// on the screen comes to the front with the keyboard, and while one this shell started is
    /// still starting, nothing more is started.
    fn open_wifi(&mut self) {
        if let Some(id) = self.state.window_with_app_id(WIFI_APP_ID) {
            self.state.wm.activate(id);
            self.apply_focus();
            return;
        }
        let Some(cmd) = &self.launch_wifi else {
            eprintln!("flow-compositor: no Wi-Fi picker to open (no --launch-wifi)");
            return;
        };
        if !self.launched.is_empty() {
            return;
        }
        match std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .env("WAYLAND_DISPLAY", &self.socket)
            .spawn()
        {
            Ok(child) => {
                eprintln!("flow-compositor: opening the Wi-Fi picker");
                self.launched.push(child);
            }
            Err(e) => eprintln!(
                "flow-compositor: could not open the Wi-Fi picker ({}): {e}",
                cmd[0]
            ),
        }
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

    fn write_frame(
        &self,
        path: &Path,
        width: u32,
        height: u32,
        elapsed: Duration,
    ) -> Result<(), String> {
        let frame = self.render_at(width, height, false, elapsed)?;
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
            // A drag with the left button, in steps, as a hand moves a mouse; `super` holds
            // the Super key through it.
            ["drag", _, _, _, _, rest @ ..] => {
                let with_super = rest == ["super"];
                if !rest.is_empty() && !with_super {
                    return Err(format!("script: `{line}`: drag x0 y0 x1 y1 [super]"));
                }
                const KEY_LEFTMETA: u32 = 125;
                if with_super {
                    self.key(KEY_LEFTMETA, true, 0);
                }
                let (x0, y0, x1, y1) = (num(1)?, num(2)?, num(3)?, num(4)?);
                self.pointer_to(f64::from(x0), f64::from(y0), 0);
                self.pointer_button(BTN_LEFT, true, 0);
                for i in 1..=8 {
                    let x = x0 + (x1 - x0) * i / 8;
                    let y = y0 + (y1 - y0) * i / 8;
                    self.pointer_to(f64::from(x), f64::from(y), 0);
                    self.settle(Duration::from_millis(5))?;
                }
                self.pointer_button(BTN_LEFT, false, 0);
                if with_super {
                    self.key(KEY_LEFTMETA, false, 0);
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
            // A scripted frame is the shell: the splash is the first moments, long over by
            // the time input commands have run.
            ["frame", path] => {
                self.write_frame(Path::new(path), opts.width, opts.height, splash::total())?
            }
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
            popups: Vec::new(),
            asked: Vec::new(),
            screen: (width as i32, height as i32),
            pointer: (width as i32 / 2, height as i32 / 2),
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
            drag_button: None,
            resize_button: None,
            started: Instant::now(),
            last_frames: Instant::now(),
            width,
            height,
            socket: socket.to_string(),
            launch_wifi: None,
            launch_files: None,
            launched: Vec::new(),
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
    server.launch_wifi = opts
        .launch_wifi
        .as_deref()
        .map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .filter(|c| !c.is_empty());
    server.launch_files = opts
        .launch_files
        .as_deref()
        .map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .filter(|c| !c.is_empty());

    if let Some(device) = &opts.drm {
        return crate::live::run(server, device);
    }

    // The splash is the shell's first moments, before any window has drawn, so it is
    // rendered without waiting for one, at a fixed elapsed time. It needs an `--out` to
    // write, like the shell.
    if let Some(elapsed) = opts.splash {
        let Some(out) = &opts.out else {
            return Err("--splash needs --out".to_string());
        };
        server.write_frame(
            out,
            opts.width,
            opts.height,
            Duration::from_secs_f32(elapsed),
        )?;
        return Ok(());
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
        // The shell frame, drawn as it looks once the splash has finished: the headless
        // frame is the shell, not the boot (that is `--splash`).
        (None, Some(out)) => server.write_frame(out, opts.width, opts.height, splash::total())?,
        (None, None) => return Err("nothing to do: give --out or --script".into()),
    }
    Ok(())
}

/// Render the windows where the window manager put them, over the shell's background,
/// with a border round the focused one and the pointer on top, and read the pixels back.
/// For the first moments after the compositor started (`elapsed`), the frame is the boot
/// splash instead: the animated geomesh, before any window has drawn.
fn render(
    state: &App,
    width: u32,
    height: u32,
    cursor: Option<(i32, i32)>,
    elapsed: Duration,
) -> Result<ShellFrame, String> {
    // The splash is pure (no renderer, no windows): the mark on the brand background.
    // It is returned before the pixman path below, so the live display and the headless
    // frame both show the same splash for the same elapsed time.
    let boot = splash::state(elapsed);
    if splash::shown_at_boot() && boot.showing {
        return Ok(splash::frame(width, height, elapsed));
    }
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
    // The first element is drawn on top: the newest popup first, then the windows.
    for (popup, _) in state.popups.iter().rev() {
        let surface = popup.wl_surface();
        if let Some(origin) = state.draw_origin(surface) {
            elements.extend(render_elements_from_surface_tree(
                &mut renderer,
                surface,
                origin,
                1.0,
                1.0,
                Kind::Unspecified,
            ));
        }
    }
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
    // No border on a fullscreen window: it has the whole screen, edge to edge.
    let focus_border = state
        .wm
        .focused()
        .filter(|id| state.wm.fullscreen() != Some(*id))
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
        // The bar, over the windows (a moved one may reach under it), except when a window
        // is fullscreen: then the screen is that window's (E5).
        if state.wm.fullscreen().is_none() {
            let (sw, sh) = state.screen;
            let colour = |c: flow_shell::Colour| {
                let (_, r, g, b) = c.channels();
                Color32F::new(
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    1.0,
                )
            };
            let rect =
                |(x, y, w, h): (i32, i32, i32, i32)| Rectangle::new((x, y).into(), (w, h).into());
            let (bx, by, _, _) = wm::wifi_button(sw, sh);
            let (fx, fy, _, _) = wm::files_button(sw, sh);
            // The Wi-Fi button: a square with three rising bars. The Files button: a folder.
            let mut parts = vec![
                (rect(wm::bar(sw, sh)), colour(palette::BAR)),
                (
                    rect(wm::wifi_button(sw, sh)),
                    colour(palette::SURFACE_FOCUSED),
                ),
                (
                    rect(wm::files_button(sw, sh)),
                    colour(palette::SURFACE_FOCUSED),
                ),
                (rect((fx + 4, fy + 8, 16, 12)), colour(palette::BAR_ICON)),
                (rect((fx + 4, fy + 5, 7, 4)), colour(palette::BAR_ICON)),
            ];
            for (i, h) in [6, 11, 16].into_iter().enumerate() {
                let x = bx + 5 + 5 * i as i32;
                parts.push((rect((x, by + 20 - h, 4, h)), colour(palette::BAR_ICON)));
            }
            for (r, c) in parts {
                frame
                    .draw_solid(r, &[Rectangle::from_size(r.size)], c)
                    .map_err(err("drawing the bar"))?;
            }
        }
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
