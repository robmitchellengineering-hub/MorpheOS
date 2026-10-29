//! `flow-wifi`: the Wi-Fi picker (doc 20, E5). A window on the Flow shell listing the
//! networks in range; choose one, type its password, and NetworkManager joins it.
//!
//!     flow-wifi [--device wlan0] [--size 480x360]
//!     flow-wifi --fake 'Home:80:secured,Cafe:40:open' [--fake-password 'correct horse']
//!
//! Up and Down (or a click) choose, Enter joins, Esc backs out of the password and then
//! closes the picker. It says on stdout what it saw and did (`flow-wifi: networks ...`,
//! `joining`, `connected NAME ADDRESS`, `failed NAME wrong-password`), never the password,
//! so a test or the journal can follow it. `--fake` replaces NetworkManager with a fixed
//! list, for the compositor's own tests.
//!
//! NetworkManager is called from a thread of its own: a join takes seconds, and the window
//! must keep drawing (and answering the compositor's pings) while it does.

/// One picker at a time.
///
/// Two independent things start this program: `flow-wifi.service`, when the machine has no
/// network at boot, and the shell's own Wi-Fi button or Super+W. The shell only counts the
/// pickers **it** started, so both paths can run one at the same time — which is what the first
/// real-hardware boot showed: two Wi-Fi windows on screen, over the same NetworkManager
/// (MacBookPro9,1, 2026-10-28). Whichever starts second exits quietly instead.
///
/// The check reads /proc rather than taking a lock file: a lock left behind by a crash would
/// silence the picker for the rest of the boot, and no cleanup path is more trustworthy than the
/// kernel's own list of running processes.
#[cfg(target_os = "linux")]
fn another_picker_is_running() -> bool {
    let me = std::process::id();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me {
            continue;
        }
        // One condition, not two: clippy denies `collapsible_if` here (CI runs clippy with
        // -D warnings), and the let-chain is what this toolchain reads best.
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm"))
            && comm.trim() == "flow-wifi"
        {
            return true;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    // A second picker is not an error: it is the other starter winning a race. Exit 0 so the
    // unit that started it is not a failed unit — a failed unit is an unhealthy boot (doc 20,
    // E11), and a diagnostic-grade detail must never be able to roll an update back.
    // `--fake` is the compositor's own test mode, and its tests run in parallel in one process
    // space: two of them starting a picker at the same moment is not two windows over one
    // NetworkManager, it is the harness. So the rule applies to the real path only.
    let fake = std::env::args().any(|a| a == "--fake" || a.starts_with("--fake="));
    if !fake && another_picker_is_running() {
        println!("flow-wifi: a picker is already on screen; not opening a second window");
        return std::process::ExitCode::SUCCESS;
    }
    match client::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            println!("flow-wifi: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("flow-wifi: Linux only");
    std::process::ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod client {
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
    use std::time::{Duration, Instant};

    use flow_shell::Frame;
    use flow_wifi::backend::{Backend, Fake};
    use flow_wifi::nm::Nm;
    use flow_wifi::picker::{Action, Failure, Input, Network, Picker, Status};
    use wayland_client::protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
    use xkbcommon::xkb;

    fn say(line: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "flow-wifi: {line}");
        let _ = out.flush();
    }

    // ---- The worker: NetworkManager (or the fake), off the window's thread. ----

    enum Command {
        Connect {
            ssid: String,
            password: Option<String>,
        },
    }

    enum Update {
        Networks(Vec<Network>),
        Joined {
            ssid: String,
            result: Result<String, Failure>,
        },
        Unavailable(String),
    }

    fn worker(
        mut backend: Box<dyn Backend + Send>,
        commands: Receiver<Command>,
        updates: Sender<Update>,
    ) {
        let started = Instant::now();
        backend.scan();
        loop {
            match backend.networks() {
                Ok(n) => {
                    if updates.send(Update::Networks(n)).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = updates.send(Update::Unavailable(e));
                }
            }
            // Often at first, while the first scan comes in; then every few seconds.
            let wait = if started.elapsed() < Duration::from_secs(15) {
                1
            } else {
                5
            };
            match commands.recv_timeout(Duration::from_secs(wait)) {
                Ok(Command::Connect { ssid, password }) => {
                    let result = backend.connect(&ssid, password.as_deref());
                    if updates.send(Update::Joined { ssid, result }).is_err() {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => backend.scan(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
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

    struct Args {
        fake: Option<String>,
        fake_password: String,
        device: Option<String>,
        size: (u32, u32),
    }

    fn args() -> Result<Args, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let flag = |n: &str| {
            args.iter()
                .position(|a| a == n)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let size = flag("--size").unwrap_or_else(|| "480x360".into());
        let size = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or(format!("bad size {size}"))?;
        Ok(Args {
            fake: flag("--fake"),
            fake_password: flag("--fake-password").unwrap_or_else(|| "correct horse".into()),
            device: flag("--device"),
            size,
        })
    }

    /// One line naming every network, when the list changes.
    fn describe(networks: &[Network]) -> String {
        let names: Vec<String> = networks
            .iter()
            .map(|n| {
                let kind = if n.secured { "secured" } else { "open" };
                let on = if n.active { ",on" } else { "" };
                format!("{}({},{kind}{on})", n.ssid, n.strength)
            })
            .collect();
        format!("networks {}", names.join(" "))
    }

    pub fn run() -> Result<(), String> {
        let Args {
            fake,
            fake_password,
            device,
            size: (mut width, mut height),
        } = args()?;

        let mut picker = Picker::new();
        let (commands, command_rx) = mpsc::channel();
        let (update_tx, updates) = mpsc::channel();
        let backend: Result<Box<dyn Backend + Send>, String> = match &fake {
            Some(spec) => Fake::parse(spec, &fake_password).map(|f| Box::new(f) as _),
            None => Nm::open(device.as_deref()).map(|nm| {
                say(&format!("device {}", nm.interface));
                Box::new(nm) as _
            }),
        };
        match backend {
            Ok(b) => {
                std::thread::spawn(move || worker(b, command_rx, update_tx));
            }
            Err(e) => {
                say(&format!("unavailable: {e}"));
                picker.set_status(Status::Unavailable(e));
            }
        }

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
        toplevel.set_title("Wi-Fi".into());
        toplevel.set_app_id("morpheos.wifi".into());
        surface.commit();

        let mut last_networks = String::new();
        while !state.closed {
            // Wait for the compositor, or a tenth of a second, whichever is first: the
            // worker's news is read in between.
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
                        // ⚠️ READABLE IS NOT A PROMISE. The socket is non-blocking, and a
                        // read after poll said "readable" can still find nothing whole to
                        // read (EAGAIN). Taking that for a lost connection made the picker
                        // quit a second after it opened, the first time the compositor's
                        // reply to a buffer's destroy arrived on its own.
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

            while let Ok(update) = updates.try_recv() {
                match update {
                    Update::Networks(n) => {
                        picker.set_networks(n);
                        let line = describe(picker.networks());
                        if line != last_networks {
                            say(&line);
                            last_networks = line;
                        }
                    }
                    Update::Joined { ssid, result } => match result {
                        Ok(address) => {
                            say(&format!("connected {ssid} {address}"));
                            picker.set_status(Status::Connected(ssid));
                        }
                        Err(why) => {
                            match &why {
                                Failure::WrongPassword => {
                                    say(&format!("failed {ssid} wrong-password"))
                                }
                                Failure::Other(o) => say(&format!("failed {ssid}: {o}")),
                            }
                            picker.set_status(Status::Failed { ssid, why });
                        }
                    },
                    Update::Unavailable(e) => picker.set_status(Status::Unavailable(e)),
                }
                state.dirty = true;
            }
            for input in std::mem::take(&mut state.inputs) {
                match picker.handle(input, height) {
                    Action::Connect { ssid, password } => {
                        say(&format!("joining {ssid}"));
                        let _ = commands.send(Command::Connect { ssid, password });
                    }
                    Action::Quit => state.closed = true,
                    Action::None => {}
                }
                state.dirty = true;
            }

            if state.configured && state.dirty {
                if let Some((w, h)) = state.size {
                    (width, height) = (w, h);
                }
                let mut frame = Frame::new(width, height);
                picker.paint(&mut frame);
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
        let path = std::path::Path::new(&dir).join(format!("flow-wifi-{}-{n}", std::process::id()));
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
