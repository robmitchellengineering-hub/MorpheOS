//! The file view on the Flow compositor (doc 20, E17), driven the way a person drives it.
//!
//! The real `flow-files` window on the real compositor (headless), listing a folder with a
//! folder and a file in it. The compositor's script presses the pointer through the same
//! handler libinput's pointer goes through (tests/input.rs in flow-compositor), so the file
//! view's click is exactly the click a person makes. Asserted from the file view's own words
//! and from the compositor's frame:
//!   * the listing: folders first, then files, each named;
//!   * the window is on the shell (a folder row and a file row, in their own colours);
//!   * a click on the file's row runs the desktop's default handler for it.
//!
//! The listing is a pure function and is unit-tested in `lib.rs`; here it is the real window.
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use flow_files::{ROW_H, TITLE_H, colours, open_command};

const FILES: &str = env!("CARGO_BIN_EXE_flow-files");
/// Where the window manager puts the first window.
const ORIGIN: (usize, usize) = (16, 16);
const SIZE: (usize, usize) = (480, 360);

/// The compositor binary, built beside this crate's binary by `cargo test --all`.
fn compositor() -> PathBuf {
    let path = Path::new(FILES).with_file_name("flow-compositor");
    assert!(
        path.exists(),
        "{} is not built: run `cargo build -p flow-compositor` (cargo test --all builds it)",
        path.display()
    );
    path
}

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Ppm {
    px: Vec<[u8; 3]>,
}

impl Ppm {
    fn read(path: &Path) -> Self {
        let bytes = std::fs::read(path).unwrap();
        // Skip the PPM header: three lines ("P6", "W H", "255"), then the pixels.
        let text_end = bytes
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .nth(2)
            .unwrap()
            .0;
        Self {
            px: bytes[text_end + 1..].as_chunks::<3>().0.to_vec(),
        }
    }

    fn count(&self, c: [u8; 3]) -> usize {
        self.px.iter().filter(|p| **p == c).count()
    }
}

fn rgb(c: flow_shell::Colour) -> [u8; 3] {
    let (_, r, g, b) = c.channels();
    [r, g, b]
}

#[test]
fn a_person_lists_a_folder_and_opens_a_file() {
    let dir = runtime_dir("open");
    let home = dir.join("home");
    std::fs::create_dir_all(home.join("notes")).unwrap();
    std::fs::write(home.join("notes").join("inner.txt"), "x").unwrap();
    std::fs::write(home.join("hello.txt"), "hello").unwrap();
    let frame = dir.join("frame.ppm");

    // The first row is the folder, the second the file. Click the file: a row is ROW_H tall,
    // so its centre is TITLE_H + ROW_H + ROW_H/2 below the window's top, and the window's
    // top is ORIGIN.1 below the screen's.
    let click_y = ORIGIN.1 + TITLE_H as usize + ROW_H as usize + ROW_H as usize / 2;
    let click_x = ORIGIN.0 + 12 + 5;
    let script = format!(
        "settle 1500\nframe {f}\nclick {click_x} {click_y}\nsettle 500\n",
        f = frame.display(),
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
    let mut files = Command::new(FILES)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "test")
        .args(["--dir"])
        .arg(&home)
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
    let _ = files.kill();
    let _ = files.wait();
    let mut said = String::new();
    files
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();
    files
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();

    // The listing, folders first, then files.
    assert!(
        said.contains("entry notes dir") && said.contains("entry hello.txt file"),
        "the file view did not list the folder: {said:?}"
    );
    // The window is on the shell: a folder row and a file row, in their own colours.
    let frame = Ppm::read(&frame);
    assert!(
        frame.count(rgb(colours::FOLDER)) > 5000,
        "no folder row on the shell"
    );
    assert!(
        frame.count(rgb(colours::FILE)) > 5000,
        "no file row on the shell"
    );
    // The click opened the file with the desktop's default handler.
    let expected = format!("open {}", open_command(&home.join("hello.txt")));
    assert!(
        said.contains(&expected),
        "the file view did not open the file: {said:?}"
    );
}

/// The bar's Files button and Super+E open the file view (E20): the shell starts it itself, so
/// a person reaches their files. There is only ever one; a second press brings the open one to
/// the front with the keyboard.
#[test]
fn the_bars_button_and_super_e_open_one_file_view() {
    let dir = runtime_dir("bar");
    std::fs::write(dir.join("hello.txt"), "hello").unwrap();
    let f: Vec<PathBuf> = (0..5).map(|i| dir.join(format!("{i}.ppm"))).collect();
    // On a 640x480 screen the Files button is the 24x24 square at (576, 452) (wm::files_button).
    const KEY_ESC: u32 = 1;
    const KEY_LEFTMETA: u32 = 125;
    const KEY_E: u32 = 18;
    let script = format!(
        "settle 500\nframe {f0}\n\
         click 588 464\nsettle 2500\nframe {f1}\n\
         click 588 464\nsettle 1000\nframe {f2}\n\
         key {KEY_ESC}\nsettle 1000\nframe {f3}\n\
         key {KEY_LEFTMETA} {KEY_E}\nsettle 2500\nframe {f4}\n",
        f0 = f[0].display(),
        f1 = f[1].display(),
        f2 = f[2].display(),
        f3 = f[3].display(),
        f4 = f[4].display(),
    );
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let launch = format!(
        "{FILES} --dir {} --size {}x{}",
        dir.display(),
        SIZE.0,
        SIZE.1
    );
    let out = Command::new(compositor())
        .env("XDG_RUNTIME_DIR", &dir)
        .args(["--socket", "test", "--width", "640", "--height", "480"])
        .args(["--wait-for", "0", "--timeout", "30", "--launch-files"])
        .arg(&launch)
        .arg("--script")
        .arg(&script_path)
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "the script did not complete: {said}");

    let title = rgb(colours::TITLE);
    let titles = |i: usize| Ppm::read(&f[i]).count(title);
    let one = TITLE_H as usize * SIZE.0 * 8 / 10;
    assert_eq!(
        titles(0),
        0,
        "a file view was open before anything asked for one"
    );
    let first = titles(1);
    assert!(
        first >= one,
        "the Files button did not open the file view ({first} px): {said}"
    );
    let again = titles(2);
    assert!(
        again >= one && again < first * 3 / 2,
        "a second press opened a second file view ({first} px, then {again})"
    );
    assert_eq!(said.matches("opening the file view").count(), 2, "{said}");
    assert!(
        said.contains("closed"),
        "Esc did not close the file view: {said}"
    );
    assert_eq!(
        titles(3),
        0,
        "the file view is still on the screen after Esc"
    );
    assert!(
        titles(4) >= one,
        "Super+E did not open the file view: {said}"
    );
}
