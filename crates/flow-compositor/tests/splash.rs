//! The boot splash's evidence (doc 20, E19): the mark is in the shell's first frame.
//!
//! The compositor renders the splash when asked with `--splash` — the shell's first
//! moments, at elapsed time zero, before any window has drawn. This test boots the real
//! binary that way, reads the PPM it wrote, and asserts the mark is actually there: the
//! frame's centre is matrix green and there is a mark's worth of green pixels. It is not
//! enough that the timing function said "showing"; the pixels must be the mark.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
/// The mark (design/README.md: matrix green `#00ff41`).
const GREEN: [u8; 3] = [0x00, 0xff, 0x41];
/// The brand background (design/README.md: near-black `#05070d`).
const BRAND_BLACK: [u8; 3] = [0x05, 0x07, 0x0d];

/// A private `XDG_RUNTIME_DIR` per test, so parallel tests cannot share a socket.
fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-splash-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

#[test]
fn the_splash_frame_shows_the_mark_on_the_brand_background() {
    let dir = runtime_dir("splash");
    let out = dir.join("splash.ppm");
    let status = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .arg("--splash")
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "the compositor did not write the splash");

    let (w, h, px) = read_ppm(&out);
    assert_eq!((w, h), (640, 480));
    // A corner the mark cannot reach is the brand background.
    assert_eq!(px[0], BRAND_BLACK, "the background should be brand black");
    // The mark is centred, so the frame's centre is matrix green.
    assert_eq!(px[240 * w + 320], GREEN, "the centre should be the mark");
    // And it is a real mark, not a couple of stray pixels.
    let green = px.iter().filter(|p| **p == GREEN).count();
    assert!(
        green > 5000,
        "the mark should be thousands of green pixels, found {green}"
    );
}
