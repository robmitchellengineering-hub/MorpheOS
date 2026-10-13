//! `solid-window`: a real Wayland client that opens one window filled with one colour.
//!
//! The compositor's test needs a client it did not write itself. A window of a single
//! known colour is the smallest thing whose appearance in the compositor's frame proves the
//! whole path: protocol handshake, `xdg_toplevel` configure, an `shm` buffer, commit,
//! import, render. No toolkit, so nothing between the protocol and the pixels can hide a
//! fault.
//!
//! It prints `keyboard-enter` and `keyboard-leave` on stdout as keyboard focus arrives and
//! goes, and `key N` and `button N` as keys and mouse buttons are pressed in it, so a test
//! can see input from the client's side. With `--key-colour` and `--click-colour` it also
//! turns that colour when a key is pressed or it is clicked, so a test that can only see
//! the screen (a VM's screendump) can see the input arrive. It exits when the compositor asks
//! it to close (`xdg_toplevel.close`), as a well-behaved app does, or when the compositor
//! goes away.
//!
//!     WAYLAND_DISPLAY=<socket> solid-window --colour 00a0ff --size 320x200 \
//!         [--key-colour 00c060] [--click-colour c000c0]

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
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

    #[derive(Default)]
    struct State {
        compositor: Option<wl_compositor::WlCompositor>,
        shm: Option<wl_shm::WlShm>,
        wm_base: Option<xdg_wm_base::XdgWmBase>,
        seat: Option<wl_seat::WlSeat>,
        configured: bool,
        closed: bool,
        /// The buffer to show next, when input asked for a change of colour.
        show: Option<Look>,
    }

    /// Which of the window's colours it shows.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Look {
        Key,
        Click,
    }

    fn say(line: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
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
                    "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
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
    impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
        fn event(
            state: &mut Self,
            _: &xdg_toplevel::XdgToplevel,
            event: xdg_toplevel::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let xdg_toplevel::Event::Close = event {
                state.closed = true;
            }
        }
    }

    impl Dispatch<wl_seat::WlSeat, ()> for State {
        fn event(
            _: &mut Self,
            seat: &wl_seat::WlSeat,
            event: wl_seat::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_seat::Event::Capabilities {
                capabilities: wayland_client::WEnum::Value(caps),
            } = event
            {
                if caps.contains(wl_seat::Capability::Keyboard) {
                    seat.get_keyboard(qh, ());
                }
                if caps.contains(wl_seat::Capability::Pointer) {
                    seat.get_pointer(qh, ());
                }
            }
        }
    }

    impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
        fn event(
            state: &mut Self,
            _: &wl_keyboard::WlKeyboard,
            event: wl_keyboard::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                wl_keyboard::Event::Enter { .. } => say("keyboard-enter"),
                wl_keyboard::Event::Leave { .. } => say("keyboard-leave"),
                wl_keyboard::Event::Key {
                    key,
                    state: wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed),
                    ..
                } => {
                    say(&format!("key {key}"));
                    state.show = Some(Look::Key);
                }
                _ => {}
            }
        }
    }

    impl Dispatch<wl_pointer::WlPointer, ()> for State {
        fn event(
            state: &mut Self,
            _: &wl_pointer::WlPointer,
            event: wl_pointer::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_pointer::Event::Button {
                button,
                state: wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } = event
            {
                say(&format!("button {button}"));
                state.show = Some(Look::Click);
            }
        }
    }

    struct Args {
        colour: u32,
        key_colour: Option<u32>,
        click_colour: Option<u32>,
        width: u32,
        height: u32,
    }

    fn args() -> Result<Args, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let flag = |n: &str| {
            args.iter()
                .position(|a| a == n)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let hex = |v: String| u32::from_str_radix(&v, 16).map_err(|_| format!("bad colour {v}"));
        let colour = hex(flag("--colour").unwrap_or_else(|| "00a0ff".into()))?;
        let key_colour = flag("--key-colour").map(hex).transpose()?;
        let click_colour = flag("--click-colour").map(hex).transpose()?;
        let size = flag("--size").unwrap_or_else(|| "320x200".into());
        let (w, h) = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or(format!("bad size {size}"))?;
        Ok(Args {
            colour,
            key_colour,
            click_colour,
            width: w,
            height: h,
        })
    }

    pub fn run() -> Result<(), String> {
        let Args {
            colour,
            key_colour,
            click_colour,
            width,
            height,
        } = args()?;
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

        // One buffer per colour, each one opaque ARGB pixel repeated, side by side in a
        // file the compositor maps. Changing colour is attaching another buffer.
        let colours = [
            colour,
            key_colour.unwrap_or(colour),
            click_colour.unwrap_or(colour),
        ];
        let stride = width * 4;
        let len = (stride * height) as usize;
        let mut file = tempfile()?;
        let mut data = Vec::with_capacity(len * colours.len());
        for c in colours {
            let pixel = (0xff00_0000 | c).to_le_bytes();
            for _ in 0..width * height {
                data.extend_from_slice(&pixel);
            }
        }
        file.write_all(&data)
            .map_err(|e| format!("filling the buffer: {e}"))?;
        let pool = shm.create_pool(file.as_fd(), data.len() as i32, &qh, ());
        let buffers: Vec<wl_buffer::WlBuffer> = (0..colours.len())
            .map(|i| {
                pool.create_buffer(
                    (i * len) as i32,
                    width as i32,
                    height as i32,
                    stride as i32,
                    wl_shm::Format::Argb8888,
                    &qh,
                    (),
                )
            })
            .collect();
        let show = |buffer: &wl_buffer::WlBuffer| {
            surface.attach(Some(buffer), 0, 0);
            surface.damage_buffer(0, 0, width as i32, height as i32);
            surface.commit();
        };
        show(&buffers[0]);

        // Stay until asked to close, or until the compositor goes away.
        while !state.closed {
            if queue.blocking_dispatch(&mut state).is_err() {
                return Ok(());
            }
            match state.show.take() {
                Some(Look::Key) if key_colour.is_some() => show(&buffers[1]),
                Some(Look::Click) if click_colour.is_some() => show(&buffers[2]),
                _ => {}
            }
        }
        toplevel.destroy();
        xdg.destroy();
        surface.destroy();
        let _ = conn.flush();
        Ok(())
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
