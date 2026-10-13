//! The compositor's evidence (HANDOFF.md E1): real clients' windows are in its frame.
//!
//! Each test starts the real `flow-compositor` binary on a private socket, starts real
//! `solid-window` clients, and reads the frame the compositor wrote. The assertions are on
//! pixels: a window of a known colour must cover exactly its own area, where the compositor
//! placed it. The control runs with no client and must produce no frame at all, so a
//! compositor that wrote a frame regardless could not pass.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
const CLIENT: &str = env!("CARGO_BIN_EXE_solid-window");
/// Where the compositor places the first window, and the gap between windows.
const MARGIN: usize = 16;

/// A private `XDG_RUNTIME_DIR` per test, so parallel tests cannot share a socket.
fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-compositor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn compositor(dir: &Path, out: &Path, wait_for: usize, timeout: u64) -> Child {
    Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args([
            "--wait-for",
            &wait_for.to_string(),
            "--timeout",
            &timeout.to_string(),
        ])
        .arg("--out")
        .arg(out)
        .spawn()
        .unwrap()
}

fn client(dir: &Path, colour: &str, size: &str) -> Child {
    // The socket appears when the compositor binds it; wait for it rather than guessing.
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
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// A binary PPM (P6) as (width, height, RGB triples).
fn read_ppm(path: &Path) -> (usize, usize, Vec<[u8; 3]>) {
    let bytes = std::fs::read(path).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8(bytes[start..i].to_vec()).unwrap());
    }
    assert_eq!(fields[0], "P6");
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let px = bytes[i + 1..].as_chunks::<3>().0.to_vec();
    assert_eq!(px.len(), w * h);
    (w, h, px)
}

/// The bounding box and count of pixels of one colour.
fn find(
    frame: &(usize, usize, Vec<[u8; 3]>),
    rgb: [u8; 3],
) -> Option<(usize, usize, usize, usize, usize)> {
    let (w, _, px) = frame;
    let hits: Vec<(usize, usize)> = px
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == rgb)
        .map(|(i, _)| (i % w, i / w))
        .collect();
    let (x0, y0) = (
        hits.iter().map(|h| h.0).min()?,
        hits.iter().map(|h| h.1).min()?,
    );
    let (x1, y1) = (
        hits.iter().map(|h| h.0).max()?,
        hits.iter().map(|h| h.1).max()?,
    );
    Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1, hits.len()))
}

#[test]
fn a_clients_window_is_in_the_frame_exactly_where_it_was_placed() {
    let dir = runtime_dir("one");
    let out = dir.join("frame.ppm");
    let mut comp = compositor(&dir, &out, 1, 20);
    let mut win = client(&dir, "00a0ff", "200x120");
    assert!(
        comp.wait().unwrap().success(),
        "the compositor wrote no frame"
    );
    let _ = win.wait();

    let frame = read_ppm(&out);
    let (x, y, w, h, n) =
        find(&frame, [0x00, 0xa0, 0xff]).expect("the window's colour is not in the frame");
    assert_eq!(
        (x, y, w, h),
        (MARGIN, MARGIN, 200, 120),
        "the window is not where it was placed"
    );
    assert_eq!(
        n,
        200 * 120,
        "the window is not solid: something drew over it or it was clipped"
    );
}

#[test]
fn two_windows_are_both_drawn_and_do_not_overlap() {
    let dir = runtime_dir("two");
    let out = dir.join("frame.ppm");
    let mut comp = compositor(&dir, &out, 2, 20);
    let mut a = client(&dir, "ff4000", "160x100");
    // The second window must arrive after the first so the stacking order is known.
    std::thread::sleep(Duration::from_millis(300));
    let mut b = client(&dir, "40ff00", "120x80");
    assert!(
        comp.wait().unwrap().success(),
        "the compositor wrote no frame"
    );
    let (_, _) = (a.wait(), b.wait());

    let frame = read_ppm(&out);
    let first = find(&frame, [0xff, 0x40, 0x00]).expect("the first window is missing");
    let second = find(&frame, [0x40, 0xff, 0x00]).expect("the second window is missing");
    assert_eq!(first, (MARGIN, MARGIN, 160, 100, 160 * 100));
    assert_eq!(second, (MARGIN, MARGIN + 100 + MARGIN, 120, 80, 120 * 80));
}

#[test]
fn with_no_client_there_is_no_frame_and_the_exit_says_so() {
    // The control: a compositor that wrote a frame whether or not anything drew would pass
    // the tests above on background colour alone if their colours ever matched it.
    let dir = runtime_dir("none");
    let out = dir.join("frame.ppm");
    let status = compositor(&dir, &out, 1, 1).wait().unwrap();
    assert_eq!(
        status.code(),
        Some(1),
        "no client drew, so the run must fail"
    );
    assert!(!out.exists(), "a frame was written although no window drew");
}
