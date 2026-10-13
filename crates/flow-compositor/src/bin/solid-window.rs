//! `solid-window`: a real Wayland client that opens one window filled with one colour.
//!
//! The compositor's test needs a client it did not write itself. A window of a single
//! known colour is the smallest thing whose appearance in the compositor's frame proves the
//! whole path: protocol handshake, `xdg_toplevel` configure, an `shm` buffer, commit,
//! import, render. No toolkit, so nothing between the protocol and the pixels can hide a
//! fault. It waits for the compositor to close the connection, which it does after writing
//! its frame.
//!
//!     WAYLAND_DISPLAY=<socket> solid-window --colour 00a0ff --size 320x200

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    match client::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("solid-window: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("solid-window: Linux only");
    std::process::ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod client {
    use std::io::Write;
    use std::os::fd::AsFd;

    use wayland_client::protocol::{
        wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

    #[derive(Default)]
    struct State {
        compositor: Option<wl_compositor::WlCompositor>,
        shm: Option<wl_shm::WlShm>,
        wm_base: Option<xdg_wm_base::XdgWmBase>,
        configured: bool,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name, interface, ..
            } = event
            {
                match interface.as_str() {
                    "wl_compositor" => state.compositor = Some(registry.bind(name, 4, qh, ())),
                    "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                    "xdg_wm_base" => state.wm_base = Some(registry.bind(name, 1, qh, ())),
                    _ => {}
                }
            }
        }
    }

    impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
        fn event(
            _: &mut Self,
            wm_base: &xdg_wm_base::XdgWmBase,
            event: xdg_wm_base::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm_base.pong(serial);
            }
        }
    }

    impl Dispatch<xdg_surface::XdgSurface, ()> for State {
        fn event(
            state: &mut Self,
            surface: &xdg_surface::XdgSurface,
            event: xdg_surface::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let xdg_surface::Event::Configure { serial } = event {
                surface.ack_configure(serial);
                state.configured = true;
            }
        }
    }

    delegate_noop!(State: ignore wl_compositor::WlCompositor);
    delegate_noop!(State: ignore wl_surface::WlSurface);
    delegate_noop!(State: ignore wl_shm::WlShm);
    delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
    delegate_noop!(State: ignore wl_buffer::WlBuffer);
    delegate_noop!(State: ignore xdg_toplevel::XdgToplevel);

    fn args() -> Result<(u32, u32, u32), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let flag = |n: &str| {
            args.iter()
                .position(|a| a == n)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let colour = flag("--colour").unwrap_or_else(|| "00a0ff".into());
        let colour =
            u32::from_str_radix(&colour, 16).map_err(|_| format!("bad colour {colour}"))?;
        let size = flag("--size").unwrap_or_else(|| "320x200".into());
        let (w, h) = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or(format!("bad size {size}"))?;
        Ok((colour, w, h))
    }

    pub fn run() -> Result<(), String> {
        let (colour, width, height) = args()?;
        let conn = Connection::connect_to_env().map_err(|e| format!("connecting: {e}"))?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());
        let mut state = State::default();
        queue
            .roundtrip(&mut state)
            .map_err(|e| format!("registry: {e}"))?;

        let compositor = state.compositor.clone().ok_or("no wl_compositor")?;
        let shm = state.shm.clone().ok_or("no wl_shm")?;
        let wm_base = state.wm_base.clone().ok_or("no xdg_wm_base")?;

        let surface = compositor.create_surface(&qh, ());
        let xdg = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg.get_toplevel(&qh, ());
        toplevel.set_title("solid-window".into());
        surface.commit();
        while !state.configured {
            queue
                .blocking_dispatch(&mut state)
                .map_err(|e| format!("waiting for configure: {e}"))?;
        }

        // One opaque ARGB pixel, repeated, in a file the compositor maps.
        let stride = width * 4;
        let len = (stride * height) as usize;
        let pixel = (0xff00_0000 | colour).to_le_bytes();
        let mut file = tempfile()?;
        let mut data = Vec::with_capacity(len);
        for _ in 0..width * height {
            data.extend_from_slice(&pixel);
        }
        file.write_all(&data)
            .map_err(|e| format!("filling the buffer: {e}"))?;
        let pool = shm.create_pool(file.as_fd(), len as i32, &qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();

        // Stay connected until the compositor has written its frame and gone away.
        loop {
            if queue.blocking_dispatch(&mut state).is_err() {
                return Ok(());
            }
        }
    }

    /// An unlinked file in `$XDG_RUNTIME_DIR` to back the buffer.
    fn tempfile() -> Result<std::fs::File, String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is not set")?;
        let path = std::path::Path::new(&dir).join(format!("solid-window-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("creating {}: {e}", path.display()))?;
        std::fs::remove_file(&path).map_err(|e| format!("unlinking {}: {e}", path.display()))?;
        Ok(file)
    }
}
