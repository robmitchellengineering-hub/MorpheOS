//! The service: the shell on a real display, driven by a real keyboard and mouse (doc 20,
//! E3 and E10).
//!
//! WHY A SEAT SESSION. Showing a picture needs DRM master, and reading the keyboard needs
//! the input devices; both belong to whoever owns the seat. Without a session the shell
//! would hold them by being root and nothing would hand them back, so the keys a person
//! types into a window would ALSO reach the login prompt on the console underneath, and
//! Ctrl+Alt+F2 would never reach the root console on VT2. `libseat` (through `seatd`, or
//! logind for an ordinary user later, E11) owns the virtual terminal: it puts it in graphics
//! mode with the kernel's keyboard handling off, opens the devices for us, and pauses the
//! session when a person switches away and resumes it when they come back.
//!
//! Input comes from libinput and goes through the SAME handlers the headless script uses
//! (`Server::pointer_to`, `pointer_button`, `key`), so the script's tests cover what a
//! person's hands do (AGENTS.md H3).

use std::path::Path;
use std::time::{Duration, Instant};

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
    PointerButtonEvent, PointerMotionEvent,
};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::input::{DeviceCapability, Libinput};
use smithay::reexports::rustix::fs::OFlags;

use crate::drm_output::Output;
use crate::headless::Server;
use flow_compositor::display;

/// How long seatd has to give us the seat before the service gives up.
const SEAT_TIMEOUT: Duration = Duration::from_secs(10);

struct Live {
    server: Server,
    session: LibSeatSession,
    libinput: Libinput,
    output: Option<Output>,
    /// Whether this session's virtual terminal is the one showing.
    active: bool,
    /// An error from inside a callback, returned by the main loop.
    failed: Option<String>,
}

impl Live {
    fn input(&mut self, event: InputEvent<LibinputInputBackend>) {
        match event {
            InputEvent::DeviceAdded { device } => announce(&device),
            InputEvent::Keyboard { event } => {
                // libinput's codes are offset by 8 into xkb's; `key` takes the Linux code.
                let code = event.key_code().raw().saturating_sub(8);
                let pressed = event.state() == KeyState::Pressed;
                if let Some(vt) = self.server.key(code, pressed, event.time_msec())
                    && let Err(e) = self.session.change_vt(vt)
                {
                    eprintln!("flow-compositor: switching to VT{vt}: {e}");
                }
            }
            InputEvent::PointerMotion { event } => {
                self.server
                    .pointer_by(event.delta_x(), event.delta_y(), event.time_msec());
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let (w, h) = self.server.size();
                let at = event.position_transformed((w as i32, h as i32).into());
                self.server.pointer_to(at.x, at.y, event.time_msec());
            }
            InputEvent::PointerButton { event } => {
                let pressed = event.state() == ButtonState::Pressed;
                self.server
                    .pointer_button(event.button_code(), pressed, event.time_msec());
            }
            _ => {}
        }
    }

    fn session(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::PauseSession => {
                eprintln!("flow-compositor: session paused (another virtual terminal is showing)");
                self.libinput.suspend();
                self.active = false;
            }
            SessionEvent::ActivateSession => {
                eprintln!("flow-compositor: session active");
                if self.libinput.resume().is_err() {
                    self.failed = Some("resuming input after the session came back".into());
                }
                if let Some(output) = &self.output
                    && let Err(e) = output.restore()
                {
                    self.failed = Some(e);
                }
                self.active = true;
            }
        }
    }
}

/// Tell systemd the shell is up (`sd_notify(READY=1)`), when it started us as `Type=notify`.
/// Without `$NOTIFY_SOCKET` (a test, or a person running it by hand) there is nobody to tell.
fn notify_ready() {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return;
    };
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
    // An address starting with '@' is in the abstract namespace.
    let addr = match bytes.split_first() {
        Some((b'@', name)) => SocketAddr::from_abstract_name(name),
        _ => SocketAddr::from_pathname(&path),
    };
    let sent = addr.and_then(|addr| UnixDatagram::unbound()?.send_to_addr(b"READY=1", &addr));
    if let Err(e) = sent {
        eprintln!("flow-compositor: could not tell systemd it is ready: {e}");
    }
}

/// Say which input devices the shell is using, so a log shows what it found.
fn announce(device: &smithay::reexports::input::Device) {
    let kinds: Vec<&str> = [
        (DeviceCapability::Keyboard, "keyboard"),
        (DeviceCapability::Pointer, "pointer"),
    ]
    .into_iter()
    .filter(|(c, _)| device.has_capability(*c))
    .map(|(_, n)| n)
    .collect();
    if !kinds.is_empty() {
        eprintln!(
            "flow-compositor: input: {} ({})",
            device.name(),
            kinds.join(", ")
        );
    }
}

/// Open `device` through the seat and show a buffer on its first connected display.
fn open_output(session: &mut LibSeatSession, device: &Path) -> Result<Output, String> {
    let fd = session
        .open(
            device,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|e| format!("opening {} through the seat: {e}", device.display()))?;
    Output::open(fd, device)
}

/// `--drm auto`: the first card in `/dev/dri` with a connected display ([`display`] says
/// why not `card0`). A card passed over stays known to the seat until the shell exits; it
/// is never shown on.
fn open_any(session: &mut LibSeatSession) -> Result<Output, String> {
    let names = std::fs::read_dir("/dev/dri")
        .map_err(|e| format!("reading /dev/dri: {e}"))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok());
    let cards = display::card_order(names);
    let (card, output) = display::choose(&cards, |card| {
        open_output(session, &Path::new("/dev/dri").join(card))
    })?;
    eprintln!(
        "flow-compositor: --drm auto chose /dev/dri/{card} (of {})",
        cards.join(", ")
    );
    Ok(output)
}

/// Take the seat, show the screen on `device` (or the first card that can, for `auto`),
/// and serve until stopped.
pub fn run(server: Server, device: &Path) -> Result<(), String> {
    let (session, notifier) = LibSeatSession::new()
        .map_err(|e| format!("opening a seat session (is seatd running?): {e}"))?;
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    let mut event_loop: EventLoop<Live> =
        EventLoop::try_new().map_err(|e| format!("event loop: {e}"))?;
    let handle = event_loop.handle();
    handle
        .insert_source(notifier, |event, _, live| live.session(event))
        .map_err(|e| format!("watching the session: {e}"))?;
    handle
        .insert_source(
            LibinputInputBackend::new(libinput.clone()),
            |event, _, live| {
                live.input(event);
            },
        )
        .map_err(|e| format!("watching input: {e}"))?;
    let active = session.is_active();
    let mut live = Live {
        server,
        session,
        libinput: libinput.clone(),
        output: None,
        active,
        failed: None,
    };

    // The seat may be granted a moment after it is asked for.
    let deadline = Instant::now() + SEAT_TIMEOUT;
    while !live.active {
        if Instant::now() > deadline {
            return Err(format!("the seat was not granted within {SEAT_TIMEOUT:?}"));
        }
        event_loop
            .dispatch(Some(Duration::from_millis(50)), &mut live)
            .map_err(|e| format!("waiting for the seat: {e}"))?;
    }

    let output = if device == Path::new("auto") {
        open_any(&mut live.session)?
    } else {
        open_output(&mut live.session, device)?
    };
    live.server.resize(output.width, output.height);
    live.output = Some(output);
    let seat = live.session.seat();
    libinput
        .udev_assign_seat(&seat)
        .map_err(|()| format!("libinput could not use seat {seat}"))?;
    // ⚠️ THE DEVICES FOUND HERE WAIT FOR A KEYPRESS. Assigning the seat queues one "added"
    // event per device inside libinput without making its fd readable, and the event loop
    // only drains libinput when the fd is readable, so nothing was announced until the first
    // real key or mouse move (found by test-shell.sh, which waits for the devices). Drain the
    // queue once here. It holds only those "added" events: nobody has touched anything yet.
    libinput
        .dispatch()
        .map_err(|e| format!("reading input devices: {e}"))?;
    for event in &mut libinput {
        if let smithay::reexports::input::Event::Device(
            smithay::reexports::input::event::DeviceEvent::Added(added),
        ) = event
        {
            announce(&smithay::reexports::input::event::EventTrait::device(
                &added,
            ));
        }
    }

    let mut shown = None;
    let mut ready = false;
    loop {
        let started = Instant::now();
        event_loop
            .dispatch(Some(Duration::ZERO), &mut live)
            .map_err(|e| format!("dispatching events: {e}"))?;
        if let Some(e) = live.failed.take() {
            return Err(e);
        }
        live.server.turn()?;
        if live.active
            && let Some(output) = live.output.as_mut()
        {
            let frame = live.server.render(output.width, output.height, true)?;
            output.show(frame.pixels())?;
            // ⚠️ READY MEANS A PICTURE IS ON THE SCREEN, NOT THAT THE PROCESS STARTED (E11).
            // The health check judges a trial by its units, and a compositor that starts
            // and never draws (stuck on the display, or hung before its first frame) was a
            // running unit: the trial would be committed with a black screen. So the unit
            // is `Type=notify`, and is only started once the first frame has been shown.
            if !ready {
                notify_ready();
                eprintln!("flow-compositor: first frame on the display; ready");
                ready = true;
            }
        }
        // One line when the picture changes in kind, so a log (and a test) can follow it.
        let count = live.server.windows();
        if shown != Some(count) {
            eprintln!("flow-compositor: {count} window(s) on screen");
            shown = Some(count);
        }
        // About 30 frames a second; the software path does not need more yet.
        std::thread::sleep(Duration::from_millis(33).saturating_sub(started.elapsed()));
    }
}
