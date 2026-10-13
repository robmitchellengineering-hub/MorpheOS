//! Popups (doc 20, E4): menus, dropdowns and tooltips are `xdg_popup`s, and a browser is full
//! of them. Without this a person opening a menu in Firefox saw nothing.
//!
//! A real client opens a popup when clicked: 120 x 80, at (40, 40) inside its window. The
//! popup must appear in the frame exactly there, above the window; a click INSIDE it reaches
//! it and leaves it open; a click OUTSIDE every popup closes it (the client is told
//! `popup_done`) and it leaves the frame, as on every desktop.
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
const CLIENT: &str = env!("CARGO_BIN_EXE_solid-window");
const WINDOW: [u8; 3] = [0x00, 0xa0, 0xff];
const POPUP: [u8; 3] = [0xff, 0xcc, 0x00];

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-popup-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Bounding box (x, y, w, h) and count of one colour in a PPM frame.
fn find(path: &Path, rgb: [u8; 3]) -> Option<(usize, usize, usize, usize, usize)> {
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
    let hits: Vec<(usize, usize)> = bytes[text_end + 1..]
        .as_chunks::<3>()
        .0
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

#[test]
fn a_popup_opens_where_asked_takes_its_clicks_and_closes_on_a_click_outside() {
    let dir = runtime_dir("menu");
    let frames: Vec<PathBuf> = (0..4).map(|i| dir.join(format!("{i}.ppm"))).collect();
    // The window is at (16, 16), 200 x 150; its popup opens at (16+40, 16+40) = (56, 56).
    let script = format!(
        "frame {}\nclick 30 30\nsettle 400\nframe {}\nclick 100 100\nsettle 200\nframe {}\nclick 500 400\nsettle 400\nframe {}\n",
        frames[0].display(),
        frames[1].display(),
        frames[2].display(),
        frames[3].display()
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let comp = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args(["--wait-for", "1", "--timeout", "20", "--script"])
        .arg(&script_path)
        .spawn()
        .unwrap();
    for _ in 0..200 {
        if dir.join("test").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut client = Command::new(CLIENT)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "test")
        .args([
            "--colour",
            "00a0ff",
            "--size",
            "200x150",
            "--popup-colour",
            "ffcc00",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        comp.wait_with_output().unwrap().status.success(),
        "the script did not complete"
    );
    let _ = client.kill();
    let _ = client.wait();
    let mut said = String::new();
    client
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();

    assert_eq!(find(&frames[0], POPUP), None, "a popup before any click");

    // The click on the window opened the popup, exactly where the client asked, on top.
    assert!(
        said.contains("popup-shown"),
        "the client never opened its popup: {said:?}"
    );
    assert_eq!(
        find(&frames[1], POPUP),
        Some((56, 56, 120, 80, 120 * 80)),
        "the popup is not at (56, 56), 120 x 80, whole and on top"
    );
    let (_, _, _, _, window_px) = find(&frames[1], WINDOW).unwrap();
    assert_eq!(
        window_px,
        200 * 150 - 120 * 80,
        "the window is not exactly the part the popup (wholly inside it) does not cover"
    );

    // A click inside the popup reaches it, and it stays open.
    assert_eq!(
        said.matches("button 272").count(),
        2,
        "the click inside the popup did not reach the client: {said:?}"
    );
    assert_eq!(
        find(&frames[2], POPUP),
        Some((56, 56, 120, 80, 120 * 80)),
        "a click inside the popup closed it"
    );

    // A click outside every popup closes it: the client is told, and it leaves the frame.
    assert!(
        said.contains("popup-done"),
        "the client was never told its popup closed: {said:?}"
    );
    assert_eq!(
        find(&frames[3], POPUP),
        None,
        "the popup is still on screen"
    );
}
