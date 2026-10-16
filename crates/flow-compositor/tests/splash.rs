//! The boot splash's evidence (doc 20, E19): the animated geomesh is in the shell's first
//! moments, and it lands on matrix green.
//!
//! The compositor renders the splash when asked with `--splash [SECS]` — a fixed elapsed
//! time, before any window has drawn. This test boots the real binary at two instants and
//! reads the PPMs it wrote. It is not enough that the timing function said "showing"; the
//! pixels must be the scene: the first instant draws the data rings on the brand
//! background, and once the bar is over the mesh is matrix green.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
/// The brand background (design/README.md: near-black `#05070d`).
const BRAND_BLACK: [u8; 3] = [0x05, 0x07, 0x0d];
/// Ring 1 (design/README.md: neon pink `#ff2fd6`) — drawn from the very first frame.
const RING: [u8; 3] = [0xff, 0x2f, 0xd6];
/// Matrix green `#00ff41` — where the mesh lands.
const GREEN: [u8; 3] = [0x00, 0xff, 0x41];

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

/// Boot the compositor to render the splash at `elapsed` seconds, and read the PPM.
fn splash(dir: &Path, elapsed: Option<&str>) -> (usize, usize, Vec<[u8; 3]>) {
    let out = dir.join("splash.ppm");
    let mut cmd = Command::new(COMPOSITOR);
    cmd.env("XDG_RUNTIME_DIR", dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .arg("--splash");
    if let Some(secs) = elapsed {
        cmd.arg(secs);
    }
    cmd.arg("--out").arg(&out);
    let status = cmd.status().unwrap();
    assert!(status.success(), "the compositor did not write the splash");
    read_ppm(&out)
}

#[test]
fn the_first_instant_is_brand_black_and_draws_the_rings() {
    let dir = runtime_dir("first");
    let (w, h, px) = splash(&dir, None);
    assert_eq!((w, h), (640, 480));
    // A corner is the brand background.
    assert_eq!(px[0], BRAND_BLACK, "the background should be brand black");
    // And the scene is already drawing: the magenta data ring is there.
    let ring = px.iter().filter(|p| **p == RING).count();
    assert!(ring > 100, "the data rings should be drawn, found {ring}");
}

#[test]
fn the_mesh_lands_on_matrix_green() {
    let dir = runtime_dir("green");
    let (w, h, px) = splash(&dir, Some("5.72"));
    assert_eq!((w, h), (640, 480));
    // A corner is still the brand background.
    assert_eq!(px[0], BRAND_BLACK, "the background should be brand black");
    // The mesh has drawn itself in and landed on matrix green.
    let green = px.iter().filter(|p| **p == GREEN).count();
    assert!(
        green > 500,
        "the landed mesh should be green, found {green}"
    );
}
