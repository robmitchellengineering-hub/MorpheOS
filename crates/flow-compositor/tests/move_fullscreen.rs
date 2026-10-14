//! Move, fullscreen and the switch shortcut (doc 20, E9), through the handlers real input uses.
//!
//! The script's `drag` and `key` go through `Server::pointer_to`, `pointer_button` and `key`,
//! the same functions libinput's events go through on a laptop. Two real clients:
//!   * a window dragged by its edge moves, and stays where it was put; the window under the
//!     edge never sees the button (the shell kept it);
//!   * Super+drag moves a window by its middle, again without the window seeing the button,
//!     and the window moved comes to the front;
//!   * Alt+Tab switches to the other window and brings it to the front (the overlap is
//!     its colour now), and neither window sees the Tab;
//!   * Super+F makes the focused window fullscreen (at the screen's origin, told the
//!     screen's size, no border) and back (told its old size, where it was);
//!   * a client asking for fullscreen itself (its F key, as Firefox's F11) gets it, and gets
//!     out of it the same way;
//!   * and after all of it a plain click still reaches the window under it, once: a move
//!     that left the pointer grabbed (a press the window saw, a release it did not) would
//!     leave every window deaf to the mouse, and look like nothing in a screenshot.
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
// Linux input codes (linux/input-event-codes.h).
const KEY_TAB: u32 = 15;
const KEY_F: u32 = 33;
const KEY_LEFTALT: u32 = 56;
const KEY_LEFTMETA: u32 = 125;
const BTN_LEFT: u32 = 0x110;

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-e9-{name}-{}", std::process::id()));
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
        .args(["--colour", colour, "--size", size, "--fullscreen-key"])
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

struct Frame {
    w: usize,
    px: Vec<[u8; 3]>,
}

impl Frame {
    fn read(path: &Path) -> Self {
        let bytes = std::fs::read(path).unwrap();
        let text_end = bytes
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .nth(2)
            .unwrap()
            .0;
        let header = String::from_utf8_lossy(&bytes[..text_end]).to_string();
        let w = header.split_whitespace().nth(1).unwrap().parse().unwrap();
        Self {
            w,
            px: bytes[text_end + 1..].as_chunks::<3>().0.to_vec(),
        }
    }

    /// Bounding box (x, y, w, h) of one colour.
    fn find(&self, rgb: [u8; 3]) -> Option<(usize, usize, usize, usize)> {
        let hits: Vec<(usize, usize)> = self
            .px
            .iter()
            .enumerate()
            .filter(|(_, p)| **p == rgb)
            .map(|(i, _)| (i % self.w, i / self.w))
            .collect();
        let x0 = hits.iter().map(|h| h.0).min()?;
        let y0 = hits.iter().map(|h| h.1).min()?;
        let x1 = hits.iter().map(|h| h.0).max()?;
        let y1 = hits.iter().map(|h| h.1).max()?;
        Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }

    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.px[y * self.w + x]
    }
}

#[test]
fn windows_move_switch_and_go_fullscreen_on_real_input() {
    let dir = runtime_dir("move");
    let f: Vec<PathBuf> = (0..8).map(|i| dir.join(format!("{i}.ppm"))).collect();
    // A opens at (16, 16), 160 x 100; B below it at (16, 132), 120 x 80.
    let script = format!(
        "frame {f0}\n\
         drag 96 12 296 212\nframe {f1}\n\
         drag 40 40 240 290 super\nframe {f2}\n\
         key {KEY_LEFTALT} {KEY_TAB}\nframe {f3}\n\
         key {KEY_LEFTMETA} {KEY_F}\nsettle 300\nframe {f4}\n\
         key {KEY_LEFTMETA} {KEY_F}\nsettle 300\nframe {f5}\n\
         key {KEY_F}\nsettle 300\nframe {f6}\n\
         key {KEY_F}\nsettle 300\nframe {f7}\n\
         click 300 330\n",
        f0 = f[0].display(),
        f1 = f[1].display(),
        f2 = f[2].display(),
        f3 = f[3].display(),
        f4 = f[4].display(),
        f5 = f[5].display(),
        f6 = f[6].display(),
        f7 = f[7].display(),
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let mut comp = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args(["--wait-for", "2", "--timeout", "30", "--script"])
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
    let _ = a.kill();
    let _ = b.kill();
    let (a_out, b_out) = (stdout_of(&mut a), stdout_of(&mut b));
    let frames: Vec<Frame> = f.iter().map(|p| Frame::read(p)).collect();

    assert_eq!(frames[0].find(A), Some((16, 16, 160, 100)));
    assert_eq!(frames[0].find(B), Some((16, 132, 120, 80)));

    // By the edge: the strip above A's top. A moved by (200, 200); B stacked up into the
    // space it left. Neither window saw the button: the shell kept it.
    assert_eq!(
        frames[1].find(A),
        Some((216, 216, 160, 100)),
        "A did not move"
    );
    assert_eq!(
        frames[1].find(B),
        Some((16, 16, 120, 80)),
        "B did not take A's place"
    );
    assert_eq!(
        frames[1].find(ACCENT),
        Some((213, 213, 166, 106)),
        "the moved window is not the focused one"
    );

    // Super+drag on B's middle: B moved by (200, 250), over A's lower part, and is in
    // front, so the overlap is B's colour.
    assert_eq!(
        frames[2].find(B),
        Some((216, 266, 120, 80)),
        "Super+drag did not move B"
    );
    assert_eq!(
        frames[2].at(250, 290),
        B,
        "the window moved is not in front"
    );
    let button = format!("button {BTN_LEFT}\n");

    // Alt+Tab: A is focused and in front again, and no window saw the Tab.
    assert_eq!(
        frames[3].at(250, 290),
        A,
        "Alt+Tab did not bring A to the front"
    );
    assert_eq!(frames[3].find(ACCENT).map(|r| (r.0, r.1)), Some((213, 213)));
    let tab = format!("key {KEY_TAB}\n");
    assert!(
        !a_out.contains(&tab) && !b_out.contains(&tab),
        "a window saw the Tab of Alt+Tab"
    );

    // Super+F: A fullscreen. Placed at the origin, told the screen's size, no border.
    // (solid-window keeps its own buffer size, so it covers its 160 x 100 of the screen.)
    assert_eq!(
        frames[4].find(A),
        Some((0, 0, 160, 100)),
        "Super+F: A not at the origin"
    );
    assert_eq!(
        frames[4].find(ACCENT),
        None,
        "a border round a fullscreen window"
    );
    assert!(
        a_out.contains("configure 640x480 fullscreen"),
        "A was not told it is fullscreen: {a_out:?}"
    );
    // And back: where it was, told its old size.
    assert_eq!(
        frames[5].find(A),
        Some((216, 216, 160, 100)),
        "Super+F again: A not back"
    );
    assert!(
        a_out.contains("configure 160x100\n"),
        "A was not asked back to its own size: {a_out:?}"
    );
    let f_key = format!("key {KEY_F}\n");
    assert_eq!(
        a_out.matches(&f_key).count(),
        2,
        "A should see only the two plain F presses, not Super+F's: {a_out:?}"
    );

    // The client asks for itself (its F key), as a browser's F11 does.
    assert!(a_out.contains("asking for fullscreen"), "{a_out:?}");
    assert_eq!(
        frames[6].find(A),
        Some((0, 0, 160, 100)),
        "a client's own request was ignored"
    );
    assert!(a_out.contains("asking to leave fullscreen"), "{a_out:?}");
    assert_eq!(
        frames[7].find(A),
        Some((216, 216, 160, 100)),
        "a client could not leave fullscreen"
    );

    // A plain click on B (where A does not cover it) reaches B, once: the only button any
    // window saw.
    assert_eq!(
        b_out.matches(&button).count(),
        1,
        "the click on B: {b_out:?}"
    );
    assert_eq!(
        a_out.matches(&button).count(),
        0,
        "A saw a button: {a_out:?}"
    );
}
