//! `flow-timeline`: the rewind timeline (doc 20, S6). A window on the Flow shell listing the
//! moments a person can go back to; choose one, confirm the dry run's counts, and Strata
//! rewinds that folder.
//!
//!     flow-timeline [--store /var/lib/strata] [--size 480x360]
//!     flow-timeline --fake '/home/morph/Documents:24' [--fake-counts 24,25] [--fake-delay 900]
//!
//! Up and Down choose, Enter runs a dry run and shows what it would do, a second Enter
//! confirms it, Esc backs out and then closes. It says on stdout what it saw and did
//! (`flow-timeline: points ...`, `ready`, `would rewind ...`, `rewound ...`), so a test or the
//! journal can follow it. `--fake` replaces Strata with a fixed list, for the compositor's
//! tests.
//!
//! Strata is called from a thread of its own: a rewind takes seconds, and the window must
//! keep drawing (and answering the compositor's pings and frame callbacks) while it runs
//! (AGENTS.md H17).

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    match client::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            println!("flow-timeline: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("flow-timeline: Linux only");
    std::process::ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod client {
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::{Duration, Instant};

    use flow_shell::Frame;
    use flow_timeline::backend::{Backend, Fake, StrataBackend};
    use flow_timeline::timeline::{Action, Input, Phase, Rewind, RewindPoint, Timeline};
    use wayland_client::protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    };
    use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
    use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
    use xkbcommon::xkb;

    /// The progress bar fills over this long, so the person sees the rewind working.
    const REWIND_PROGRESS_MS: u128 = 1_000;

    fn say(line: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "flow-timeline: {line}");
        let _ = out.flush();
    }

    // ---- The worker: Strata (or the fake), off the window's thread. ----

    enum Command {
        Preview { point: RewindPoint },
        Rewind { point: RewindPoint },
    }

    enum Update {
        Alerts(Vec<RewindPoint>),
        Previewed {
            point: RewindPoint,
            result: Result<Rewind, String>,
        },
        Rewound {
            point: RewindPoint,
            result: Result<Rewind, String>,
        },
        Unavailable(String),
    }

    fn worker(
        mut backend: Box<dyn Backend + Send>,
        commands: Receiver<Command>,
        updates: Sender<Update>,
    ) {
        // A burst re-issues its alert as it reaches folders the earlier alert had not seen
        // yet (doc 20, S6 "several folders"), and the window can open on the first issue —
        // before those folders are in the store. So the list is re-read while the window
        // waits, and the window is told only when it changed: `set_alerts` swaps the list
        // and keeps the selection, and it never disturbs a rewind already in flight.
        let mut known: Vec<RewindPoint> = Vec::new();
        loop {
            match backend.alerts() {
                Ok(list) => {
                    if list != known {
                        known = list.clone();
                        if updates.send(Update::Alerts(list)).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = updates.send(Update::Unavailable(e));
                    return;
                }
            }
            match commands.recv_timeout(Duration::from_millis(500)) {
                Ok(command) => match command {
                    Command::Preview { point } => {
                        let result = backend.rewind(&point, true);
                        if updates.send(Update::Previewed { point, result }).is_err() {
                            return;
                        }
                    }
                    Command::Rewind { point } => {
                        let result = backend.rewind(&point, false);
                        if updates.send(Update::Rewound { point, result }).is_err() {
                            return;
                        }
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
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
                // The timeline has no text field: every other key is ignored.
                _ => return None,
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
        fake_counts: (usize, usize),
        fake_delay: u64,
        store: String,
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
        let counts = flag("--fake-counts").unwrap_or_else(|| "24,25".into());
        let fake_counts = counts
            .split_once(',')
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
            .ok_or(format!("bad --fake-counts {counts}: come_back,quarantine"))?;
        let fake_delay = flag("--fake-delay")
            .map(|v| v.parse())
            .transpose()
            .map_err(|_| "bad --fake-delay".to_string())?
            .unwrap_or(900);
        Ok(Args {
            fake: flag("--fake"),
            fake_counts,
            fake_delay,
            store: flag("--store").unwrap_or_else(|| "/var/lib/strata".into()),
            size,
        })
    }

    /// One line naming every moment, when the list loads.
    fn describe(points: &[RewindPoint]) -> String {
        let parts: Vec<String> = points
            .iter()
            .map(|p| format!("{}({})", p.folder, p.files))
            .collect();
        format!("points {}", parts.join(" "))
    }

    pub fn run() -> Result<(), String> {
        let Args {
            fake,
            fake_counts: (come_back, quarantine),
            fake_delay,
            store,
            size: (mut width, mut height),
        } = args()?;

        let mut timeline = Timeline::new();
        let (commands, command_rx) = mpsc::channel();
        let (update_tx, updates) = mpsc::channel();
        let backend: Result<Box<dyn Backend + Send>, String> = match &fake {
            Some(spec) => {
                Fake::parse(spec, come_back, quarantine, fake_delay).map(|f| Box::new(f) as _)
            }
            None => StrataBackend::open(&store).map(|b| Box::new(b) as _),
        };
        // `loaded` gates the first paint: it becomes true once the list arrives from the
        // worker, or once Strata refuses, so the window never sits blank waiting.
        let mut loaded = false;
        match backend {
            Ok(b) => {
                std::thread::spawn(move || worker(b, command_rx, update_tx));
            }
            Err(e) => {
                say(&format!("unavailable: {e}"));
                timeline.set_unavailable(e);
                loaded = true;
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
        toplevel.set_title("Rewind".into());
        toplevel.set_app_id("morpheos.timeline".into());
        surface.commit();

        // The window does not draw until the list is loaded (or Strata refused), so the first
        // frame the compositor sees already shows the moments — the "ready" signal a test
        // waits for before sending keys (AGENTS.md H14).
        let mut ready_said = false;
        let mut rewind_started: Option<Instant> = None;
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
                        // READABLE IS NOT A PROMISE (see flow-wifi): a read after poll said
                        // "readable" can still find nothing whole (EAGAIN). Taking that for a
                        // lost connection made the window quit a second after it opened.
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
                    Update::Alerts(list) => {
                        say(&describe(&list));
                        timeline.set_alerts(list);
                        loaded = true;
                    }
                    Update::Previewed { point, result } => match result {
                        Ok(counts) => {
                            say(&format!(
                                "would rewind {}: {}",
                                point.folder,
                                counts.result_line()
                            ));
                            timeline.previewed(counts);
                        }
                        Err(e) => {
                            say(&format!("failed {}: {e}", point.folder));
                            timeline.failed(e);
                        }
                    },
                    Update::Rewound { point, result } => match result {
                        Ok(counts) => {
                            say(&format!(
                                "rewound {}: {}",
                                point.folder,
                                counts.result_line()
                            ));
                            timeline.finished(counts);
                        }
                        Err(e) => {
                            say(&format!("failed {}: {e}", point.folder));
                            timeline.failed(e);
                        }
                    },
                    Update::Unavailable(e) => {
                        say(&format!("unavailable: {e}"));
                        timeline.set_unavailable(e);
                        loaded = true;
                    }
                }
                state.dirty = true;
            }
            for input in std::mem::take(&mut state.inputs) {
                match timeline.handle(input, height) {
                    Action::Preview { point } => {
                        say(&format!("checking {}", point.folder));
                        let _ = commands.send(Command::Preview { point });
                    }
                    Action::Rewind { point } => {
                        say(&format!("rewinding {}", point.folder));
                        rewind_started = Some(Instant::now());
                        timeline.set_progress(0);
                        let _ = commands.send(Command::Rewind { point });
                    }
                    Action::Quit => state.closed = true,
                    Action::None => {}
                }
                state.dirty = true;
            }

            // While the rewind runs, advance its progress bar and repaint, so the window keeps
            // drawing (and answering pings) instead of freezing on the confirm frame (H17).
            if matches!(timeline.phase(), Phase::Rewinding { .. }) {
                if let Some(started) = rewind_started {
                    let progress =
                        (started.elapsed().as_millis() * 100 / REWIND_PROGRESS_MS).min(100) as u32;
                    timeline.set_progress(progress);
                }
                state.dirty = true;
            }

            if state.configured && loaded && state.dirty {
                if let Some((w, h)) = state.size {
                    (width, height) = (w, h);
                }
                let mut frame = Frame::new(width, height);
                timeline.paint(&mut frame);
                draw(&shm, &surface, &qh, &frame)?;
                state.dirty = false;
                if !ready_said {
                    say("ready");
                    ready_said = true;
                }
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
            std::path::Path::new(&dir).join(format!("flow-timeline-{}-{n}", std::process::id()));
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
