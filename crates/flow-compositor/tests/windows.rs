//! Classic windows (doc 20, E2): click to focus, close, and focus moving on.
//!
//! Real compositor, real clients, a script of input commands that goes through the same
//! window manager real input will. Every step is asserted twice: in the frame (the focus
//! border, the windows' colours) and by the clients themselves (`keyboard-enter` arrives
//! only at the window that has focus, and a closed window's client exits).
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
const CLIENT: &str = env!("CARGO_BIN_EXE_solid-window");
const ACCENT: [u8; 3] = [0x5a, 0xa8, 0xff];
const A: [u8; 3] = [0xff, 0x40, 0x00];
const B: [u8; 3] = [0x40, 0xff, 0x00];

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-wm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn client(dir: &Path, colour: &str, size: &str) -> Child {
    for _ in 0..200 {
        if dir.join("test").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Command::new(CLIENT)
        .env("XDG_RUNTIME_DIR", dir)
        .env("WAYLAND_DISPLAY", "test")
        .args(["--colour", colour, "--size", size])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn stdout_of(child: &mut Child) -> String {
    let mut s = String::new();
    child.stdout.take().unwrap().read_to_string(&mut s).unwrap();
    s
}

fn read_ppm(path: &Path) -> (usize, Vec<[u8; 3]>) {
    let bytes = std::fs::read(path).unwrap();
    let text_end = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .nth(2)
        .unwrap()
        .0;
    let header = String::from_utf8_lossy(&bytes[..text_end]).to_string();
    let w: usize = header.split_whitespace().nth(1).unwrap().parse().unwrap();
    (w, bytes[text_end + 1..].as_chunks::<3>().0.to_vec())
}

/// Bounding box (x, y, w, h) and count of one colour.
fn find(
    frame: &(usize, Vec<[u8; 3]>),
    rgb: [u8; 3],
) -> Option<(usize, usize, usize, usize, usize)> {
    let (w, px) = frame;
    let hits: Vec<(usize, usize)> = px
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == rgb)
        .map(|(i, _)| (i % w, i / w))
        .collect();
    let x0 = hits.iter().map(|h| h.0).min()?;
    let y0 = hits.iter().map(|h| h.1).min()?;
    let x1 = hits.iter().map(|h| h.0).max()?;
    let y1 = hits.iter().map(|h| h.1).max()?;
    Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1, hits.len()))
}

/// The focus border round a window at (x, y) of size w x h: a 3-pixel ring.
fn ring(x: usize, y: usize, w: usize, h: usize) -> (usize, usize, usize, usize, usize) {
    (x - 3, y - 3, w + 6, h + 6, (w + 6) * (h + 6) - w * h)
}

#[test]
fn a_click_focuses_a_window_and_close_removes_it_and_focus_moves_on() {
    let dir = runtime_dir("focus");
    let (f1, f2, f3) = (dir.join("1.ppm"), dir.join("2.ppm"), dir.join("3.ppm"));
    // A opens first at (16, 16), 160x100. B below it at (16, 132), 120x80.
    let script = format!(
        "frame {}\nclick 50 150\nframe {}\nclose\nframe {}\n",
        f1.display(),
        f2.display(),
        f3.display()
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let mut comp = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args(["--wait-for", "2", "--timeout", "20", "--script"])
        .arg(&script_path)
        .spawn()
        .unwrap();
    let mut a = client(&dir, "ff4000", "160x100");
    std::thread::sleep(Duration::from_millis(300));
    let mut b = client(&dir, "40ff00", "120x80");
    assert!(
        comp.wait().unwrap().success(),
        "the script did not complete"
    );
    let b_status = b.wait().unwrap();
    let b_out = stdout_of(&mut b);
    let _ = a.kill();
    let a_out = stdout_of(&mut a);

    // Before any click: both windows, and nothing has focus.
    let one = read_ppm(&f1);
    assert_eq!(find(&one, A), Some((16, 16, 160, 100, 160 * 100)));
    assert_eq!(find(&one, B), Some((16, 132, 120, 80, 120 * 80)));
    assert_eq!(
        find(&one, ACCENT),
        None,
        "a focus border before anything was clicked"
    );

    // The click on B: B has the border, and B's client got the keyboard.
    let two = read_ppm(&f2);
    assert_eq!(
        find(&two, ACCENT),
        Some(ring(16, 132, 120, 80)),
        "no border round B"
    );
    assert_eq!(
        find(&two, A),
        Some((16, 16, 160, 100, 160 * 100)),
        "A was disturbed"
    );
    assert!(
        b_out.contains("keyboard-enter"),
        "B never got keyboard focus: {b_out:?}"
    );

    // Close: B's client exits, B is gone from the frame, and focus moves to A.
    assert!(
        b_status.success(),
        "B did not exit cleanly when asked to close"
    );
    let three = read_ppm(&f3);
    assert_eq!(find(&three, B), None, "B is still on screen after closing");
    assert_eq!(
        find(&three, ACCENT),
        Some(ring(16, 16, 160, 100)),
        "focus did not move to A"
    );
    assert!(
        a_out.contains("keyboard-enter"),
        "A never got the keyboard after B closed: {a_out:?}"
    );
}
