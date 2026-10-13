//! Keyboard and mouse (doc 20, E10), through the handlers real devices use.
//!
//! The script's `click` and `key` go through `Server::pointer_to`, `pointer_button` and
//! `key`, the same three functions libinput's events go through in the live service. So
//! this proves, with a real client, what a person's mouse and keyboard do: a click reaches
//! the window under it (and focuses it), keys reach the focused window, and Ctrl+Alt+F2 is
//! kept by the shell (it is the way to the root console) instead of reaching the window.
//! Each step is asserted twice: by the client (`button 272`, `key 30`) and in the frame
//! (the client changes colour when the input arrives).
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
const CLIENT: &str = env!("CARGO_BIN_EXE_solid-window");
const IDLE: [u8; 3] = [0x00, 0xa0, 0xff];
const CLICKED: [u8; 3] = [0xc0, 0x00, 0xc0];
const KEYED: [u8; 3] = [0x00, 0xc0, 0x60];
// Linux input codes (linux/input-event-codes.h).
const KEY_A: u32 = 30;
const KEY_LEFTCTRL: u32 = 29;
const KEY_LEFTALT: u32 = 56;
const KEY_F2: u32 = 60;
const BTN_LEFT: u32 = 0x110;
/// Where the window opens and how big it is, as `find` reports it.
const WINDOW: Option<(usize, usize, usize, usize, usize)> = Some((16, 16, 160, 100, 160 * 100));

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-input-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The window's bounding box and pixel count in one colour, from a PPM frame.
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
fn a_click_and_a_key_reach_the_window_and_the_console_chord_does_not() {
    let dir = runtime_dir("click-key");
    let frames: Vec<PathBuf> = (0..4).map(|i| dir.join(format!("{i}.ppm"))).collect();
    // The window opens at (16, 16), 160x100. Click inside it, type `a`, then the chord.
    let script = format!(
        "frame {}\nclick 60 50\nframe {}\nkey {KEY_A}\nframe {}\nkey {KEY_LEFTCTRL} {KEY_LEFTALT} {KEY_F2}\nframe {}\n",
        frames[0].display(),
        frames[1].display(),
        frames[2].display(),
        frames[3].display()
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let comp = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "320", "--height", "240"])
        .args(["--wait-for", "1", "--timeout", "20", "--script"])
        .arg(&script_path)
        .stderr(Stdio::piped())
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
        .args(["--colour", "00a0ff", "--size", "160x100"])
        .args(["--click-colour", "c000c0", "--key-colour", "00c060"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let out = comp.wait_with_output().unwrap();
    let comp_err = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the script did not complete: {comp_err}"
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

    assert_eq!(find(&frames[0], IDLE), WINDOW, "the window before input");

    // The click: the client got the left button, and turned its click colour.
    assert!(
        said.contains(&format!("button {BTN_LEFT}")),
        "the click never reached the window: {said:?}"
    );
    assert_eq!(find(&frames[1], CLICKED), WINDOW, "no click in the frame");

    // The key: the click gave it the keyboard, so `a` reached it.
    assert!(
        said.contains("keyboard-enter"),
        "the click did not focus the window: {said:?}"
    );
    assert!(
        said.contains(&format!("key {KEY_A}")),
        "the key never reached the window: {said:?}"
    );
    assert_eq!(find(&frames[2], KEYED), WINDOW, "no key in the frame");

    // Ctrl+Alt+F2: the shell keeps F2 (it is the way to the root console on VT2).
    assert!(
        comp_err.contains("switch to VT2 requested"),
        "the shell did not take Ctrl+Alt+F2: {comp_err}"
    );
    assert!(
        !said.contains(&format!("key {KEY_F2}")),
        "Ctrl+Alt+F2 leaked into the window: {said:?}"
    );
}
