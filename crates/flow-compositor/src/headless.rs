//! The headless compositor: real Wayland clients, rendered in software, written to a PNG.
//!
//! This is the first step of the express road (HANDOFF.md, E1): the Flow shell stops being a
//! picture of a desktop and starts hosting other programs' windows. It runs **headless with
//! Smithay's pixman (software) renderer** because CI runners have no GPU, and because a frame
//! a test can read back is the evidence this repository asks for (ADR 0010: "CI proves a
//! frame, not a unit"). The bare-metal backend (DRM/KMS, libinput) is E3. It reuses
//! everything here and swaps only where the frame goes.
//!
//! The flow: listen on a socket, accept clients, answer the protocols a window needs, and
//! once `wait_for` toplevels have each committed a buffer, render them over the shell's
//! background and write the frame. Windows are stacked down the content area in the order
//! they appeared. Window management (focus, move, close) is E2.
//!
//! Protocols served: `wl_compositor`, `wl_subcompositor`, `wl_shm`, `xdg_wm_base`,
//! `wl_seat` and `wl_data_device`. A browser needs more (`linux-dmabuf`, `xdg-decoration`,
//! `wl_output`, `viewporter`), and those arrive with the tasks that need them (E4).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::utils::{
    draw_render_elements, on_commit_buffer_handler, with_renderer_surface_state,
};
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Frame, Offscreen, Renderer};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::{wl_buffer, wl_seat, wl_surface::WlSurface};
use smithay::reexports::wayland_server::{Client, Display, ListeningSocket};
use smithay::utils::{Buffer, Physical, Rectangle, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use flow_shell::Frame as ShellFrame;
use flow_shell::palette;

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
    pub out: PathBuf,
}

/// Gap between stacked windows and the edges, in pixels.
const MARGIN: i32 = 16;

struct App {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    seat_state: SeatState<Self>,
    data_device_state: DataDeviceState,
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
        // A client may not draw before its first configure, so answer at once.
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
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

delegate_compositor!(App);
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
}

/// Run until `wait_for` windows are drawn, write the frame, and return. An error means no
/// frame was written.
pub fn run(opts: &Options) -> Result<(), String> {
    let mut display: Display<App> = Display::new().map_err(|e| format!("display: {e}"))?;
    let dh = display.handle();
    let mut seat_state = SeatState::new();
    let _seat: Seat<App> = seat_state.new_wl_seat(&dh, "seat0");
    let mut state = App {
        compositor_state: CompositorState::new::<App>(&dh),
        xdg_shell_state: XdgShellState::new::<App>(&dh),
        shm_state: ShmState::new::<App>(&dh, vec![]),
        seat_state,
        data_device_state: DataDeviceState::new::<App>(&dh),
    };
    let listener = ListeningSocket::bind(opts.socket.as_str())
        .map_err(|e| format!("binding socket {}: {e}", opts.socket))?;
    eprintln!("flow-compositor: listening on {}", opts.socket);

    let deadline = Instant::now() + opts.timeout;
    let start = Instant::now();
    loop {
        if let Some(stream) = listener.accept().map_err(|e| format!("accept: {e}"))? {
            dh.clone()
                .insert_client(stream, Arc::new(ClientState::default()))
                .map_err(|e| format!("adding a client: {e}"))?;
        }
        display
            .dispatch_clients(&mut state)
            .map_err(|e| format!("dispatching: {e}"))?;
        display
            .flush_clients()
            .map_err(|e| format!("flushing: {e}"))?;

        let drawn = state.drawn_toplevels();
        if drawn.len() >= opts.wait_for {
            let frame = render(&drawn, opts.width, opts.height)?;
            // PNG to look at; PPM for a test to read without a decoder.
            let bytes = if opts.out.extension().is_some_and(|e| e == "ppm") {
                frame.to_ppm()
            } else {
                frame.to_png()
            };
            std::fs::write(&opts.out, bytes)
                .map_err(|e| format!("writing {}: {e}", opts.out.display()))?;
            eprintln!(
                "flow-compositor: {} window(s) drawn; frame written to {} after {:?}",
                drawn.len(),
                opts.out.display(),
                start.elapsed()
            );
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(format!(
                "{} of {} window(s) drew a buffer within {:?}; no frame written",
                drawn.len(),
                opts.wait_for,
                opts.timeout
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Render the windows over the shell's background, and read the pixels back.
fn render(surfaces: &[WlSurface], width: u32, height: u32) -> Result<ShellFrame, String> {
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

    // Stack the windows down the screen; each element list is built before the frame
    // borrows the renderer.
    let mut elements: Vec<WaylandSurfaceRenderElement<PixmanRenderer>> = Vec::new();
    let mut y = MARGIN;
    for surface in surfaces {
        let h = with_renderer_surface_state(surface, |s| s.surface_size().map(|s| s.h))
            .flatten()
            .unwrap_or(0);
        elements.extend(render_elements_from_surface_tree(
            &mut renderer,
            surface,
            (MARGIN, y),
            1.0,
            1.0,
            Kind::Unspecified,
        ));
        y += h + MARGIN;
    }

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
        draw_render_elements(&mut frame, 1.0, &elements, &[full])
            .map_err(err("drawing the windows"))?;
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
        for px in row.chunks_exact(4).take(width as usize) {
            // ARGB8888 is a little-endian u32: B, G, R, A in memory.
            pixels.push(u32::from_le_bytes([px[0], px[1], px[2], px[3]]));
        }
    }
    ShellFrame::from_pixels(width, height, pixels)
        .ok_or_else(|| "the frame read back has the wrong size".to_string())
}
