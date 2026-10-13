//! `flow-compositor` — the Flow shell as a Wayland compositor (headless, for now).
//!
//! See `headless.rs` for what it does and why it starts headless. Linux only: Wayland
//! compositors are a Linux thing, and the dependency is scoped to Linux so the workspace
//! still builds and tests on a Mac (AGENTS.md H9).

#[cfg(target_os = "linux")]
mod headless;

const USAGE: &str = "\
usage: flow-compositor --socket NAME --out FRAME.png [--wait-for N] [--timeout SECS]
                       [--width N] [--height N]

Listen on $XDG_RUNTIME_DIR/NAME, wait until N windows (default 1) have drawn, write one
frame of the screen (.png, or .ppm for tests), and exit 0. Exit 1 if they have not drawn within the timeout
(default 30 s), with no frame written.";

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
            out: flag("--out").ok_or("--out is required")?.into(),
            wait_for: num("--wait-for", 1)? as usize,
            timeout: std::time::Duration::from_secs(num("--timeout", 30)?),
            width: num("--width", 800)? as u32,
            height: num("--height", 600)? as u32,
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
