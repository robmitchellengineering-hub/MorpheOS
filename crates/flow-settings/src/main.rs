//! `flow-settings`: the Settings window (doc 20, D1). Its first page is the power profile;
//! Up/Down choose, Enter sets it through power-profiles-daemon, Esc closes.
//!
//!     flow-settings [--size 480x360]
//!
//! It says on stdout what it did (`flow-settings: profile <name>`).

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    match client::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            println!("flow-settings: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("flow-settings: Linux only");
    std::process::ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod client {
    use std::io::Write;
    use std::os::fd::AsFd;

    use flow_login::account::Account;
    use flow_login::login::{Action, Input, Login, Status};
    use flow_shell::Frame;
    use wayland_client::protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
    use xkbcommon::xkb;

    fn say(line: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "flow-settings: {line}");
        let _ = out.flush();
    }

    #[derive(Default)]
    struct State {
        compositor: Option<wl_compositor::WlCompositor>,
        shm: Option<wl_shm::WlShm>,
        wm_base: Option<xdg_wm_base::XdgWmBase>,
        configured: bool,
        closed: bool,
        size: Option<(u32, u32)>,
        inputs: Vec<Input>,
        pointer: (f64, f64),
        xkb: Option<(xkb::Keymap, xkb::State)>,
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
                // The compositor's own layout, so a character is what the person's keyboard
                // says it is (and Shift works).
                wl_keyboard::Event::Keymap { fd, size, .. } => {
                    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
                    // SAFETY: the fd and size are the keymap the compositor sent, as the
                    // protocol defines them; xkbcommon maps it read-only.
                    let keymap = unsafe {
                        xkb::Keymap::new_from_fd(
                            &ctx,
                            fd,
                            size as usize,
                            xkb::KEYMAP_FORMAT_TEXT_V1,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        )
                    };
                    if let Ok(Some(keymap)) = keymap {
                        let st = xkb::State::new(&keymap);
                        state.xkb = Some((keymap, st));
                    }
                }
                wl_keyboard::Event::Modifiers {
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                    ..
                } => {
                    if let Some((_, st)) = &mut state.xkb {
                        st.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                    }
                }
                wl_keyboard::Event::Key {
                    key,
                    state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                    ..
                } => {
                    if let Some(input) = state.decode(key) {
                        state.inputs.push(input);
                    }
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
                    state.inputs.push(Input::Click {
                        x: x.max(0.0) as u32,
                        y: y.max(0.0) as u32,
                    });
                }
                _ => {}
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
            // Each frame has a buffer of its own; once the compositor is done with it, it goes.
            if let wl_buffer::Event::Release = event {
                buffer.destroy();
            }
        }
    }

    delegate_noop!(State: ignore wl_compositor::WlCompositor);
    delegate_noop!(State: ignore wl_surface::WlSurface);
    delegate_noop!(State: ignore wl_shm::WlShm);
    delegate_noop!(State: ignore wl_shm_pool::WlShmPool);

    impl State {
        fn decode(&self, key: u32) -> Option<Input> {
            let (_, st) = self.xkb.as_ref()?;
            let code = xkb::Keycode::new(key + 8);
            let sym = st.key_get_one_sym(code).raw();
            use xkb::keysyms as k;
            Some(match sym {
                k::KEY_Up => Input::Up,
                k::KEY_Down => Input::Down,
                k::KEY_Return | k::KEY_KP_Enter => Input::Enter,
                k::KEY_Escape => Input::Escape,
                k::KEY_BackSpace => Input::Backspace,
                k::KEY_Tab | k::KEY_ISO_Left_Tab => Input::Tab,
                _ => {
                    let mut chars = st.key_get_utf8(code).chars().collect::<Vec<_>>();
                    match (chars.pop(), chars.is_empty()) {
                        (Some(c), true) if !c.is_control() => Input::Char(c),
                        _ => return None,
                    }
                }
            })
        }
    }

    pub fn run() -> Result<(), String> {
        let mut size: (u32, u32) = (480, 360);
        if let Some((w, h)) = std::env::args()
            .skip(1)
            .find(|a| a.starts_with("--size="))
            .and_then(|a| a.strip_prefix("--size=").map(str::to_string))
            .and_then(|s| {
                s.split_once('x')
                    .map(|(w, h)| (w.to_string(), h.to_string()))
            })
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        {
            size = (w, h);
        }

        let mut settings = Settings::new();
        say("power profiles");

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
        toplevel.set_title("Settings".into());
        toplevel.set_app_id("morpheos.settings".into());
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

            for input in std::mem::take(&mut state.inputs) {
                match settings.handle(input) {
                    Action::Set(profile) => {
                        let set = std::process::Command::new("powerprofilesctl")
                            .args(["set", &profile])
                            .output();
                        match set {
                            Ok(o) if o.status.success() => {
                                say(&format!("profile {profile}"));
                                state.closed = true;
                            }
                            _ => say(&format!("profile {profile} failed")),
                        }
                    }
                    Action::Quit => state.closed = true,
                    Action::None => {}
                }
                state.dirty = true;
            }

            if state.configured && state.dirty {
                if let Some((w, h)) = state.size {
                    size = (w, h);
                }
                let mut frame = Frame::new(size.0, size.1);
                settings.paint(&mut frame);
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
            std::path::Path::new(&dir).join(format!("flow-settings-{}-{n}", std::process::id()));
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
