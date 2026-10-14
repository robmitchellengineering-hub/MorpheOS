//! A real toolkit on the compositor (doc 20, E4): GTK3, which is what Firefox is built on.
//!
//! `solid-window` proves the protocol path with nothing in the way; this proves the
//! compositor serves what a toolkit expects. GTK3's widget factory is a large, ordinary GTK
//! window. If it cannot start here, or draws in the wrong place, Firefox will too, and GTK is
//! far quicker to run and to read than a browser. Asserted:
//!   * GTK keeps drawing after its first frame (its frame callbacks are answered);
//!   * GTK found a screen (`wl_output`) and logged no critical error (without one it logs
//!     `GDK_IS_MONITOR` criticals on every frame);
//!   * the VISIBLE window, not its shadow, starts exactly at the window manager's margin
//!     (GTK draws its own shadow inside its surface and says where the window is with
//!     `set_window_geometry`);
//!   * GTK asked for a window wider than this screen, and the compositor asked it to fit.
//!
//! It needs `gtk3-widget-factory` (the `gtk-3-examples` package, installed by
//! `scripts/setup-linux.sh`). Without it the test FAILS rather than skips: a skipped test
//! reads exactly like a passing one (AGENTS.md H9).
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const COMPOSITOR: &str = env!("CARGO_BIN_EXE_flow-compositor");
const GTK_APP: &str = "gtk3-widget-factory";
const W: usize = 1024;
const H: usize = 768;
const MARGIN: usize = 16;

fn runtime_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flow-toolkit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read_ppm(path: &Path) -> Vec<[u8; 3]> {
    let bytes = std::fs::read(path).unwrap();
    let text_end = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .nth(2)
        .unwrap()
        .0;
    bytes[text_end + 1..].as_chunks::<3>().0.to_vec()
}

/// Light means GTK's window body; dark means the shell's background or GTK's shadow over it.
fn light(px: &[[u8; 3]], x: usize, y: usize) -> bool {
    let [r, g, b] = px[y * W + x];
    (u32::from(r) + u32::from(g) + u32::from(b)) / 3 > 180
}

#[test]
fn a_gtk_window_finds_the_screen_and_sits_inside_it_where_the_window_manager_puts_it() {
    assert!(
        Command::new("sh")
            .args(["-c", &format!("command -v {GTK_APP}")])
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .success(),
        "{GTK_APP} is not installed: run scripts/setup-linux.sh (package gtk-3-examples)"
    );
    let dir = runtime_dir("gtk");
    let frame = dir.join("gtk.ppm");
    // Give GTK time to be told it is too big, redraw at the new size, and settle.
    let script = format!("settle 2500\nframe {}\n", frame.display());
    let script_path = dir.join("script");
    std::fs::write(&script_path, script).unwrap();
    let comp = Command::new(COMPOSITOR)
        .env("XDG_RUNTIME_DIR", &dir)
        .args([
            "--socket",
            "test",
            "--width",
            &W.to_string(),
            "--height",
            &H.to_string(),
        ])
        .args(["--wait-for", "1", "--timeout", "30", "--script"])
        .arg(&script_path)
        .spawn()
        .unwrap();
    for _ in 0..200 {
        if dir.join("test").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let gtk = Command::new(GTK_APP)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "test")
        .env("GDK_BACKEND", "wayland")
        .env("WAYLAND_DEBUG", "1")
        // No accessibility bus or settings daemon in a test: don't wait for them.
        .env("NO_AT_BRIDGE", "1")
        .env("GSETTINGS_BACKEND", "memory")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let comp_status = comp.wait_with_output().unwrap().status;
    // The compositor has written its frame and gone; GTK loses its display and exits.
    let out = gtk.wait_with_output().unwrap();
    let log = String::from_utf8_lossy(&out.stderr);
    assert!(
        comp_status.success(),
        "the compositor's script did not complete"
    );

    assert!(
        log.contains("\"wl_output\""),
        "GTK never bound wl_output: there is no screen for it"
    );
    // A client draws its next frame only when told the last one was shown (frame
    // callbacks). Unanswered, GTK asked once and froze on its first frame, and Firefox never
    // painted its page. Answered, GTK keeps drawing.
    let frame_requests = log
        .lines()
        .filter(|l| l.contains("-> wl_surface@") && l.contains(".frame("))
        .count();
    assert!(
        frame_requests >= 5,
        "GTK asked for {frame_requests} frame(s) in 2.5 s: its frame callbacks are not answered"
    );
    let criticals: Vec<&str> = log.lines().filter(|l| l.contains("-CRITICAL")).collect();
    assert!(
        criticals.is_empty(),
        "GTK logged critical errors: {criticals:#?}"
    );

    let px = read_ppm(&frame);
    let (row, col) = (300, 300);
    // Left and top: the window body starts exactly at the margin, the shadow before it.
    assert!(
        !light(&px, MARGIN - 1, row) && light(&px, MARGIN, row),
        "the window's left edge is not at x = {MARGIN} (the shadow was placed, not the window)"
    );
    assert!(
        !light(&px, col, MARGIN - 1) && light(&px, col, MARGIN),
        "the window's top edge is not at y = {MARGIN}"
    );
    // Too big: GTK asked for 1393 x 740 on a 1024 x 768 screen, so the compositor must ask
    // it to fit: the screen less a margin all round, and less the bar (32) at the bottom. The widget factory cannot comply (it sets a MINIMUM size of
    // 1393 x 740 itself), so this asserts the request, read from GTK's own protocol log; a
    // browser's minimum is far smaller, and E4's VM scenario sees it fit.
    assert!(
        log.contains("configure_bounds(992, 704)"),
        "GTK was not told how big a window may be"
    );
    // The exact size GTK asks for depends on the machine's fonts and theme (CI's differs from
    // a desktop's), so assert the rule, not one instance: some configure asks for a size
    // inside 992 x 704 that reaches the limit in a direction GTK overflowed.
    let (max_w, max_h) = (W as i32 - 2 * MARGIN as i32, H as i32 - 2 * MARGIN as i32);
    let fits: Vec<(i32, i32)> = log
        .lines()
        .filter(|l| l.contains("xdg_toplevel@") && l.contains(".configure("))
        .filter_map(|l| {
            let args = l.split(".configure(").nth(1)?;
            let mut it = args.split(',').map(|v| v.trim().parse::<i32>().ok());
            Some((it.next()??, it.next()??))
        })
        .filter(|&(w, h)| w > 0 && h > 0 && w <= max_w && h <= max_h && (w == max_w || h == max_h))
        .collect();
    assert!(
        !fits.is_empty(),
        "the compositor did not ask the too-big window to fit inside {max_w} x {max_h}"
    );
}
