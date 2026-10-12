//! `flow-shell` — paint the shell's frame, write it as a PPM, and say whether it is
//! evidence.
//!
//! This is the *capturable* half of the Flow shell's first slice ([ADR 0010]): it produces
//! a real frame on disk and reports whether it has content. On a machine with
//! no GPU and no display server — the Legion VM, a CI runner — this is how the shell's
//! output can be looked at, and it exits non-zero when the frame is blank so it can be
//! used as a check rather than only as a tool.
//!
//! [ADR 0010]: ../../docs/adr/0010-flow-shell-first.md

use std::process::ExitCode;

use flow_shell::{DEFAULT_HEIGHT, DEFAULT_WIDTH, Openable, Shell, frame_evidence};

const USAGE: &str = "\
flow-shell - paint the shell's frame and write it as a PPM

usage: flow-shell [--out FILE.ppm] [--width N] [--height N] [--sessions N]

  --out       where to write the frame        (default flow-shell.png; .png or .ppm)
  --width     frame width in pixels           (default 1280)
  --height    frame height in pixels          (default 800)
  --sessions  how many sessions to lay out    (default 1)

Exits 0 when the frame has content, 1 when it does not.";

fn main() -> ExitCode {
    let mut width: u32 = DEFAULT_WIDTH;
    let mut height: u32 = DEFAULT_HEIGHT;
    let mut sessions: u32 = 1;
    let mut out = String::from("flow-shell.png");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].clone();
        let value = args.get(index + 1).cloned();
        match (flag.as_str(), value) {
            ("--out", Some(v)) => {
                out = v;
                index += 2;
            }
            ("--width", Some(v)) => {
                let Ok(n) = v.parse() else {
                    return refuse(&format!("--width wants a number, got {v:?}"));
                };
                width = n;
                index += 2;
            }
            ("--height", Some(v)) => {
                let Ok(n) = v.parse() else {
                    return refuse(&format!("--height wants a number, got {v:?}"));
                };
                height = n;
                index += 2;
            }
            ("--sessions", Some(v)) => {
                let Ok(n) = v.parse() else {
                    return refuse(&format!("--sessions wants a number, got {v:?}"));
                };
                sessions = n;
                index += 2;
            }
            ("--help" | "-h", _) => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => return refuse(&format!("unrecognised argument {flag:?}")),
        }
    }

    if width == 0 || height == 0 {
        return refuse("a frame with no pixels cannot show anything");
    }

    let mut shell = Shell::new(width, height);
    for n in 1..=sessions {
        // A session described by what it opens, so the Session + shelf feel has a shelf
        // to show: its tools, prewarmed around the task (ADR 0013). `add_session` would
        // leave the shelf empty, which proves nothing about the feel.
        shell.add_session_opening(
            &format!("session {n}"),
            vec![
                Openable::tool(format!("editor {n}")),
                Openable::tool(format!("terminal {n}")),
            ],
        );
    }
    let frame = shell.paint();
    let evidence = frame_evidence(&frame);

    // The extension picks the encoding. PNG is the default because the whole point is that
    // a person looks at the frame, and a PPM cannot be opened by a browser or a phone.
    let lower = out.to_ascii_lowercase();
    let (bytes, kind) = if lower.ends_with(".png") {
        (frame.to_png(), "PNG")
    } else if lower.ends_with(".ppm") {
        (frame.to_ppm(), "PPM")
    } else {
        return refuse("--out must end in .png (viewable anywhere) or .ppm");
    };

    if let Err(err) = std::fs::write(&out, bytes) {
        eprintln!("could not write {out}: {err}");
        return ExitCode::FAILURE;
    }

    println!("wrote {out} ({}x{}, {kind})", frame.width(), frame.height());
    println!("sessions laid out: {}", shell.sessions().len());
    println!("{}", evidence.verdict());

    if evidence.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn refuse(why: &str) -> ExitCode {
    eprintln!("flow-shell: {why}\n\n{USAGE}");
    ExitCode::FAILURE
}
