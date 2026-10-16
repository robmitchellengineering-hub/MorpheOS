//! `flow-compositor` — the Flow shell as a Wayland compositor (headless, for now).
//!
//! See `headless.rs` for what it does and why it starts headless. Linux only: Wayland
//! compositors are a Linux thing, and the dependency is scoped to Linux so the workspace
//! still builds and tests on a Mac (AGENTS.md H9).

#[cfg(target_os = "linux")]
mod drm_output;
#[cfg(target_os = "linux")]
mod headless;
#[cfg(target_os = "linux")]
mod live;

const USAGE: &str = "\
usage: flow-compositor --socket NAME (--out FRAME.png | --script FILE | --drm DEVICE)
                       [--wait-for N] [--timeout SECS] [--width N] [--height N]
                       [--launch-wifi CMD] [--launch-files CMD] [--splash [SECS]]

--drm /dev/dri/card0 shows the screen on that display and runs until stopped (the service);
--drm auto takes the first card in /dev/dri with a connected display.
--launch-wifi CMD is the command the bar's Wi-Fi button (and Super+W) runs to open the picker.
--launch-files CMD is the command the bar's Files button (and Super+E) runs to open the file view.
--splash [SECS] writes the boot splash (the shell's first moments) at SECS seconds after
the compositor started (default 0), instead of the shell; it needs --out and does not wait
for a window. The tests use it to render the splash at a couple of instants.
Otherwise it is headless:

Listen on $XDG_RUNTIME_DIR/NAME, wait until N windows (default 1) have drawn, write one
frame of the screen (.png, or .ppm for tests), and exit 0. Exit 1 if they have not drawn within the timeout
(default 30 s), with no frame written.

--script runs input commands once the windows have drawn, one per line:
  click X Y   move the pointer there and click (focus the window under it)
  key C...    press keys by Linux code (KEY_A is 30), released in reverse
  cycle       focus the next window
  close       ask the focused window to close, and wait until it has
  frame PATH  write the screen (.png or .ppm)
  settle MS   keep clients moving for MS milliseconds";

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::process::ExitCode;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let num = |name: &str, default: u64| -> Result<u64, String> {
        flag(name).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("bad {name} '{v}'"))
        })
    };
    let parsed = (|| -> Result<headless::Options, String> {
        Ok(headless::Options {
            socket: flag("--socket").ok_or("--socket is required")?,
            out: flag("--out").map(Into::into),
            script: match flag("--script") {
                Some(p) => Some(
                    std::fs::read_to_string(&p)
                        .map_err(|e| format!("reading --script {p}: {e}"))?,
                ),
                None => None,
            },
            drm: flag("--drm").map(Into::into),
            launch_wifi: flag("--launch-wifi"),
            launch_files: flag("--launch-files"),
            wait_for: num("--wait-for", 1)? as usize,
            timeout: std::time::Duration::from_secs(num("--timeout", 30)?),
            width: num("--width", 800)? as u32,
            height: num("--height", 600)? as u32,
            // --splash renders the splash at a fixed elapsed time; an optional number
            // follows it (--splash 5.72), defaulting to the first instant.
            splash: args.iter().any(|a| a == "--splash").then(|| {
                flag("--splash")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0)
            }),
        })
    })();
    let opts = match parsed {
        Ok(o) => o,
        Err(e) => {
            eprintln!("flow-compositor: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match headless::run(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("flow-compositor: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("flow-compositor: Wayland compositors run on Linux only\n\n{USAGE}");
    std::process::ExitCode::from(2)
}
