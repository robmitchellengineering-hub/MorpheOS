//! Mount a removable disk where the pure `mount_point` decides (doc 20, E18).
//!
//! A udev rule runs this on block `add` events; udev passes the device in the environment
//! (`DEVNAME`, `ID_FS_LABEL`, `ID_FS_UUID`). It mounts and does nothing else — the file view
//! (E17) then lists the mount point like any other folder.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::process::Command;

    /// Say it to the journal, and to the console the harness's serial log carries (so the
    /// QEMU test sees both the mount and its failure).
    fn announce(line: &str) {
        println!("flow-mount: {line}");
        let _ = std::fs::write("/dev/console", format!("flow-mount: {line}\n"));
    }

    let device = std::env::var("DEVNAME").unwrap_or_default();
    if device.is_empty() {
        announce("no DEVNAME — run it from the udev rule");
        return std::process::ExitCode::FAILURE;
    }
    let label = std::env::var("ID_FS_LABEL").unwrap_or_default();
    let uuid = std::env::var("ID_FS_UUID").unwrap_or_default();
    let mp = flow_files::mount_point(&label, &uuid);

    if let Err(e) = std::fs::create_dir_all(&mp) {
        announce(&format!("cannot create {mp}: {e}"));
        return std::process::ExitCode::FAILURE;
    }
    // `mount(8)` auto-detects the filesystem; the mount point is the pure decision.
    match Command::new("mount").arg(&device).arg(&mp).status() {
        Ok(s) if s.success() => {
            announce(&format!("mounted {device} at {mp}"));
            std::process::ExitCode::SUCCESS
        }
        Ok(s) => {
            announce(&format!("mount {device} failed: {s}"));
            std::process::ExitCode::FAILURE
        }
        Err(e) => {
            announce(&format!("cannot run mount: {e}"));
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("flow-mount: Linux only");
    std::process::exit(2);
}
