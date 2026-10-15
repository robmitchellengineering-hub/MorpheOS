//! `flow-files`: the file view (doc 20, E17). A window on the Flow shell listing a folder's
//! files; click a folder to enter it, click a file to open it.
//!
//!     flow-files [--dir /persist/home] [--size 480x360]
//!
//! It says on stdout what it shows and does — `flow-files: entry <name> <dir|file>`, and
//! `flow-files: open xdg-open '…'` when a file is opened — so a test or the journal can
//! follow it. `--dir` is where it starts (default: the person's home).

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    match client::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            println!("flow-files: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("flow-files: Linux only");
    std::process::ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod client {
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::path::PathBuf;

    use flow_files::{Action, View, open_command, paint, row_at};
    use flow_shell::Frame;
    use wayland_client::protocol::{
        wl_buffer, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

    fn say(line: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "flow-files: {line}");
        let _ = out.flush();
    }

    // ---- The window. ----

    #[derive(Default)]
    struct State {
        compositor: Option<wl_compositor::WlCompositor>,
        shm: Option<wl_shm::WlShm>,
        wm_base: Option<xdg_wm_base::XdgWmBase>,
        configured: bool,
        closed: bool,
        /// The size the compositor asked for, when it asked for one.
        size: Option<(u32, u32)>,
        pointer: (f64, f64),
        clicks: Vec<(u32, u32)>,
        dirty: bool,
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
                    "wl_seat" => {
                        registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                    }
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
                state.dirty = true;
            }
        }
    }

    impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
        fn event(
            state: &mut Self,
            _: &xdg_toplevel::XdgToplevel,
            event: xdg_toplevel::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                xdg_toplevel::Event::Configure { width, height, .. } if width > 0 && height > 0 => {
                    state.size = Some((width as u32, height as u32));
                }
                xdg_toplevel::Event::Close => state.closed = true,
                _ => {}
            }
        }
    }

    impl Dispatch<wl_buffer::WlBuffer, ()> for State {
        fn event(
            _: &mut Self,
            buffer: &wl_buffer::WlBuffer,
            event: wl_buffer::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_buffer::Event::Release = event {
                buffer.destroy();
            }
        }
    }

    delegate_noop!(State: ignore wl_compositor::WlCompositor);
    delegate_noop!(State: ignore wl_surface::WlSurface);
    delegate_noop!(State: ignore wl_shm::WlShm);
    delegate_noop!(State: ignore wl_shm_pool::WlShmPool);

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
                capabilities: WEnum::Value(caps),
            } = event
                && caps.contains(wl_seat::Capability::Pointer)
            {
                seat.get_pointer(qh, ());
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
            match event {
                wl_pointer::Event::Enter {
                    surface_x,
                    surface_y,
                    ..
                }
                | wl_pointer::Event::Motion {
                    surface_x,
                    surface_y,
                    ..
                } => state.pointer = (surface_x, surface_y),
                wl_pointer::Event::Button {
                    state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                    ..
                } => {
                    let (x, y) = state.pointer;
                    state.clicks.push((x.max(0.0) as u32, y.max(0.0) as u32));
                }
                _ => {}
            }
        }
    }

    fn args() -> Result<(PathBuf, (u32, u32)), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let flag = |n: &str| {
            args.iter()
                .position(|a| a == n)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let dir = flag("--dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/persist/home"));
        let size = flag("--size").unwrap_or_else(|| "480x360".into());
        let size = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or(format!("bad size {size}"))?;
        Ok((dir, size))
    }

    fn describe(view: &View) {
        for e in &view.entries {
            say(&format!(
                "entry {} {}",
                e.name,
                if e.is_dir { "dir" } else { "file" }
            ));
        }
    }

    pub fn run() -> Result<(), String> {
        let (dir, (mut width, mut height)) = args()?;
        let mut view = View::show(dir).map_err(|e| format!("showing: {e}"))?;
        describe(&view);

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
        toplevel.set_title("Files".into());
        toplevel.set_app_id("morpheos.files".into());
        surface.commit();

        while !state.closed {
            queue
                .dispatch_pending(&mut state)
                .map_err(|e| format!("dispatch: {e}"))?;
            if conn.flush().is_err() {
                return Ok(());
            }
            if let Some(guard) = queue.prepare_read() {
                let fd = guard.connection_fd();
                let mut fds = [rustix::event::PollFd::new(
                    &fd,
                    rustix::event::PollFlags::IN,
                )];
                let timeout = rustix::event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 100_000_000,
                };
                let ready = rustix::event::poll(&mut fds, Some(&timeout)).unwrap_or(0);
                if ready > 0 {
                    match guard.read() {
                        Ok(_) => {}
                        Err(wayland_client::backend::WaylandError::Io(e))
                            if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            return match conn.protocol_error() {
                                Some(p) => Err(format!("the compositor ended the connection: {p}")),
                                None => {
                                    say(&format!("the compositor went away ({e})"));
                                    Ok(())
                                }
                            };
                        }
                    }
                }
            }
            queue
                .dispatch_pending(&mut state)
                .map_err(|e| format!("dispatch: {e}"))?;

            for (x, y) in std::mem::take(&mut state.clicks) {
                let Some(i) = row_at(&view, x, y) else {
                    continue;
                };
                match view.click(i) {
                    Action::Enter(_) => {
                        describe(&view);
                        state.dirty = true;
                    }
                    Action::Open(p) => say(&format!("open {}", open_command(&p))),
                    Action::None => {}
                }
            }

            if state.configured && state.dirty {
                if let Some((w, h)) = state.size {
                    (width, height) = (w, h);
                }
                let mut frame = Frame::new(width, height);
                paint(&view, &mut frame);
                draw(&shm, &surface, &qh, &frame)?;
                state.dirty = false;
            }
        }
        say("closed");
        toplevel.destroy();
        xdg.destroy();
        surface.destroy();
        let _ = conn.flush();
        Ok(())
    }

    /// Show a frame: a buffer of its own, released (and destroyed) when the compositor is done.
    fn draw(
        shm: &wl_shm::WlShm,
        surface: &wl_surface::WlSurface,
        qh: &QueueHandle<State>,
        frame: &Frame,
    ) -> Result<(), String> {
        let (w, h) = (frame.width() as i32, frame.height() as i32);
        let bytes: Vec<u8> = frame
            .pixels()
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        let mut file = tempfile()?;
        file.write_all(&bytes)
            .map_err(|e| format!("filling the buffer: {e}"))?;
        let pool = shm.create_pool(file.as_fd(), bytes.len() as i32, qh, ());
        let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, qh, ());
        pool.destroy();
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, w, h);
        surface.commit();
        Ok(())
    }

    /// An unlinked file in `$XDG_RUNTIME_DIR` to back a buffer.
    fn tempfile() -> Result<std::fs::File, String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is not set")?;
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::path::Path::new(&dir).join(format!("flow-files-{}-{n}", std::process::id()));
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
