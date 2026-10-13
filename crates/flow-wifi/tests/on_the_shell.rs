//! The Wi-Fi picker on the Flow compositor (doc 20, E5), driven the way a person drives it.
//!
//! The real `flow-wifi` window on the real compositor (headless), with `--fake` in place of
//! NetworkManager: two networks, one secured with a known password. The compositor's script
//! presses keys through the same handler libinput's keys go through (tests/input.rs in
//! flow-compositor), and the compositor sends the client its keymap, so the picker decodes
//! the keys the way it will on a laptop. Asserted twice each step, from the picker's own
//! words and from the compositor's frame:
//!   * the list: both networks, the stronger first, the first selected;
//!   * Down and Up move the selection (the selected band moves a row);
//!   * Enter on the secured network asks for its password; a wrong one fails as a wrong
//!     password (the status line turns red), the right one joins (green);
//!   * the password is never printed, and never drawn (only one mark per character);
//!   * Esc closes the picker.
//!
//! The NetworkManager half is proven in a VM, against a simulated Wi-Fi network
//! (`images/x86-qemu/test-wifi.sh`).
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use flow_wifi::picker::{ROW_H, STATUS_H, TITLE_H, colours};

const PICKER: &str = env!("CARGO_BIN_EXE_flow-wifi");
/// Where the window manager puts the first window.
const ORIGIN: (usize, usize) = (16, 16);
const SIZE: (usize, usize) = (480, 360);
const RIGHT: &str = "correct horse";
const WRONG: &str = "wrong pass";

// Linux input codes (linux/input-event-codes.h).
const KEY_ESC: u32 = 1;
const KEY_ENTER: u32 = 28;
const KEY_UP: u32 = 103;
const KEY_DOWN: u32 = 108;

fn code(c: char) -> u32 {
    match c {
        'q' => 16,
        'w' => 17,
        'e' => 18,
        'r' => 19,
        't' => 20,
        'y' => 21,
        'u' => 22,
        'i' => 23,
        'o' => 24,
        'p' => 25,
        'a' => 30,
        's' => 31,
        'd' => 32,
        'f' => 33,
        'g' => 34,
        'h' => 35,
        'j' => 36,
        'k' => 37,
        'l' => 38,
        'z' => 44,
        'x' => 45,
        'c' => 46,
        'v' => 47,
        'b' => 48,
        'n' => 49,
        'm' => 50,
        ' ' => 57,
        _ => panic!("no key for {c:?}"),
    }
}

fn typing(s: &str) -> String {
    s.chars().map(|c| format!("key {}\n", code(c))).collect()
}

/// The compositor binary, built beside this crate's binary by `cargo test --all`.
fn compositor() -> PathBuf {
    let path = Path::new(PICKER).with_file_name("flow-compositor");
    assert!(
        path.exists(),
        "{} is not built: run `cargo build -p flow-compositor` (cargo test --all builds it)",
        path.display()
    );
    path
}

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-wifi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Ppm {
    w: usize,
    px: Vec<[u8; 3]>,
}

impl Ppm {
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

    /// The colour at a point inside the picker's window.
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.px[(ORIGIN.1 + y) * self.w + ORIGIN.0 + x]
    }
}

fn rgb(c: flow_shell::Colour) -> [u8; 3] {
    let (_, r, g, b) = c.channels();
    [r, g, b]
}

#[test]
fn a_person_chooses_a_network_types_its_password_and_joins_it() {
    let dir = runtime_dir("join");
    let f: Vec<PathBuf> = (0..5).map(|i| dir.join(format!("{i}.ppm"))).collect();
    let script = format!(
        "settle 1500\nclick 40 30\nframe {f0}\n\
         key {KEY_DOWN}\nframe {f1}\nkey {KEY_UP}\n\
         key {KEY_ENTER}\n{wrong}frame {f2}\nkey {KEY_ENTER}\nsettle 500\nframe {f3}\n\
         key {KEY_ENTER}\n{right}key {KEY_ENTER}\nsettle 500\nframe {f4}\n\
         key {KEY_ESC}\nsettle 500\n",
        f0 = f[0].display(),
        f1 = f[1].display(),
        f2 = f[2].display(),
        f3 = f[3].display(),
        f4 = f[4].display(),
        wrong = typing(WRONG),
        right = typing(RIGHT),
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let comp = Command::new(compositor())
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args(["--wait-for", "1", "--timeout", "30", "--script"])
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
    let mut picker = Command::new(PICKER)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "test")
        .args([
            "--fake",
            "Cafe:40:open,Home:80:secured",
            "--fake-password",
            RIGHT,
        ])
        .args(["--size", &format!("{}x{}", SIZE.0, SIZE.1)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = comp.wait_with_output().unwrap();
    let comp_err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "the script did not complete: {comp_err}"
    );
    let _ = picker.kill();
    let _ = picker.wait();
    let mut said = String::new();
    picker
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();
    picker
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();

    // The list, strongest first.
    assert!(
        said.contains("networks Home(80,secured) Cafe(40,open)"),
        "the picker did not list the networks: {said:?}"
    );
    let row = |n: usize| TITLE_H as usize + n * ROW_H as usize + 2;
    let selected = rgb(colours::SELECTED);
    let frame = Ppm::read(&f[0]);
    assert_eq!(
        frame.at(2, row(0)),
        selected,
        "the first network is not selected"
    );
    assert_ne!(frame.at(2, row(1)), selected);
    // Down moves the selection a row.
    let frame = Ppm::read(&f[1]);
    assert_ne!(
        frame.at(2, row(0)),
        selected,
        "Down did not move the selection"
    );
    assert_eq!(
        frame.at(2, row(1)),
        selected,
        "Down did not move the selection"
    );

    // The password field, with one mark per character typed, and no password on screen.
    let frame = Ppm::read(&f[2]);
    let field = rgb(colours::FIELD);
    assert!(
        frame.px.iter().filter(|p| **p == field).count() > 1000,
        "Enter on a secured network did not ask for its password"
    );

    // Wrong, then right.
    let status = |frame: &Ppm| frame.at(2, SIZE.1 - STATUS_H as usize / 2);
    assert!(
        said.contains("failed Home wrong-password"),
        "a wrong password was not reported as one: {said:?}"
    );
    assert_eq!(
        status(&Ppm::read(&f[3])),
        rgb(colours::FAIL),
        "no red status line"
    );
    assert!(
        said.contains("connected Home 10.0.0.2"),
        "the right password did not join: {said:?}"
    );
    assert_eq!(
        status(&Ppm::read(&f[4])),
        rgb(colours::OK),
        "no green status line"
    );
    assert_eq!(said.matches("joining Home").count(), 2, "{said:?}");

    // Never the password, in anything either side printed.
    for secret in [RIGHT, WRONG] {
        assert!(
            !said.contains(secret),
            "the picker printed a password: {said:?}"
        );
        assert!(
            !comp_err.contains(secret),
            "the compositor printed a password"
        );
    }
    // Esc closed it.
    assert!(
        said.contains("closed"),
        "Esc did not close the picker: {said:?}"
    );
}
