//! Mount a removable disk where the pure `mount_point` decides (doc 20, E18).
//!
//! A udev rule runs this on block `add` events; udev passes the device in the environment
//! (`DEVNAME`, `ID_FS_LABEL`, `ID_FS_UUID`, `ID_FS_TYPE`). It mounts and does nothing else — the
//! file view (E17) then lists the mount point like any other folder.

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
    let fs_type = std::env::var("ID_FS_TYPE").unwrap_or_default();
    let mp = flow_files::mount_point(&label, &uuid);

    if let Err(e) = std::fs::create_dir_all(&mp) {
        announce(&format!("cannot create {mp}: {e}"));
        return std::process::ExitCode::FAILURE;
    }
    // `mount(8)` with the type udev probed, so a partitionless stick still mounts; the mount
    // point is the pure decision.
    let mut cmd = Command::new("mount");
    if !fs_type.is_empty() {
        cmd.arg("-t").arg(&fs_type);
    }
    cmd.arg(&device).arg(&mp);
    match cmd.output() {
        Ok(out) if out.status.success() => {
            announce(&format!("mounted {device} at {mp}"));
            std::process::ExitCode::SUCCESS
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            announce(&format!("mount {device} at {mp} failed: {err}"));
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
