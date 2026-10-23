//! The rewind timeline on the Flow compositor (doc 20, S6), driven the way a person drives it.
//!
//! The real `flow-timeline` window on the real compositor (headless), with `--fake` in place
//! of Strata: two moments, a dry run whose counts are known (24 come back, 25 go to
//! quarantine), and a rewind that takes long enough to see its progress. Asserted twice each
//! step, from the timeline's own words and from the compositor's frame:
//!   * the list: both folders, the first selected;
//!   * Down and Up move the selection (the selected band moves a row);
//!   * Enter runs the dry run and shows its counts in the confirm band;
//!   * a second Enter confirms and runs the rewind, whose progress bar keeps advancing while
//!     the window keeps drawing (H17) — two frames during the rewind differ;
//!   * the result line and the green status band once it is done;
//!   * Esc closes the timeline.
//!
//! The Strata half is proven in a VM (`images/x86-qemu/test-rewind.sh`, PR 2 of S6).
#![cfg(target_os = "linux")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use flow_timeline::timeline::{ROW_H, STATUS_H, TITLE_H, colours};

const TIMELINE: &str = env!("CARGO_BIN_EXE_flow-timeline");
/// Where the window manager puts the first window.
const ORIGIN: (usize, usize) = (16, 16);
const SIZE: (usize, usize) = (480, 360);

// Linux input codes (linux/input-event-codes.h).
const KEY_ESC: u32 = 1;
const KEY_ENTER: u32 = 28;
const KEY_UP: u32 = 103;
const KEY_DOWN: u32 = 108;

/// The compositor binary, built beside this crate's binary.
fn compositor() -> PathBuf {
    let path = Path::new(TIMELINE).with_file_name("flow-compositor");
    assert!(
        path.exists(),
        "{} is not built beside flow-timeline (build the whole workspace)",
        path.display()
    );
    path
}

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-timeline-{name}-{}", std::process::id()));
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

    /// The colour at a point inside the timeline's window.
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.px[(ORIGIN.1 + y) * self.w + ORIGIN.0 + x]
    }

    /// How many pixels in the whole screen are this colour.
    fn count(&self, c: [u8; 3]) -> usize {
        self.px.iter().filter(|p| **p == c).count()
    }
}

fn rgb(c: flow_shell::Colour) -> [u8; 3] {
    let (_, r, g, b) = c.channels();
    [r, g, b]
}

#[test]
fn a_person_chooses_a_moment_confirms_the_dry_run_and_rewinds_it() {
    let dir = runtime_dir("rewind");
    let f: Vec<PathBuf> = (0..6).map(|i| dir.join(format!("{i}.ppm"))).collect();
    let script = format!(
        "settle 300\nframe {f0}\n\
         key {KEY_DOWN}\nframe {f1}\nkey {KEY_UP}\n\
         key {KEY_ENTER}\nsettle 300\nframe {f2}\n\
         key {KEY_ENTER}\nsettle 120\nframe {f3}\n\
         settle 300\nframe {f4}\n\
         settle 1500\nframe {f5}\n\
         key {KEY_ESC}\nsettle 300\n",
        f0 = f[0].display(),
        f1 = f[1].display(),
        f2 = f[2].display(),
        f3 = f[3].display(),
        f4 = f[4].display(),
        f5 = f[5].display(),
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
    let mut timeline = Command::new(TIMELINE)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "test")
        .args([
            "--fake",
            "/home/morph/Documents:24,/home/morph/Documents/clients:8",
        ])
        .args(["--fake-counts", "24,25"])
        .args(["--fake-delay", "900"])
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
    let _ = timeline.kill();
    let _ = timeline.wait();
    let mut said = String::new();
    timeline
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();
    timeline
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut said)
        .unwrap();

    // The list, and the ready line a test waits for before driving it (H14).
    assert!(
        said.contains("points /home/morph/Documents(24) /home/morph/Documents/clients(8)"),
        "the timeline did not list the moments: {said:?}"
    );
    assert!(said.contains("ready"), "no ready line: {said:?}");

    let row = |n: usize| TITLE_H as usize + n * ROW_H as usize + 2;
    let selected = rgb(colours::SELECTED);
    let frame = Ppm::read(&f[0]);
    assert_eq!(
        frame.at(2, row(0)),
        selected,
        "the first moment is not selected"
    );
    assert_ne!(frame.at(2, row(1)), selected);
    // Down moves the selection a row, and back.
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

    // The dry run: the confirm band, and the counts in the timeline's own words.
    let status = |frame: &Ppm| frame.at(2, SIZE.1 - STATUS_H as usize / 2);
    assert_eq!(
        status(&Ppm::read(&f[2])),
        rgb(colours::CONFIRM),
        "no confirm band after the dry run"
    );
    assert!(
        said.contains(
            "would rewind /home/morph/Documents: 24 files will come back, 25 will go to quarantine"
        ),
        "the dry run did not report its counts: {said:?}"
    );

    // The rewind runs long enough to see two frames of progress, and the window keeps
    // drawing: the busy band is there, and the progress bar advances between the two (H17).
    let busy = rgb(colours::BUSY);
    let during = Ppm::read(&f[3]);
    let later = Ppm::read(&f[4]);
    assert_eq!(status(&during), busy, "no busy band while rewinding");
    assert_eq!(status(&later), busy, "no busy band while rewinding");
    assert!(
        later.count(busy) > during.count(busy),
        "the progress bar did not advance ({} then {} busy px)",
        during.count(busy),
        later.count(busy)
    );

    // Done: the green band and the result line.
    assert_eq!(
        status(&Ppm::read(&f[5])),
        rgb(colours::OK),
        "no green band once done"
    );
    assert!(
        said.contains(
            "rewound /home/morph/Documents: 24 files will come back, 25 will go to quarantine"
        ),
        "the rewind did not report its result: {said:?}"
    );
    assert!(
        said.contains("closed"),
        "Esc did not close the timeline: {said:?}"
    );
}
