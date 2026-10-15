//! Mount a removable disk where the pure `mount_point` decides (doc 20, E18).
//!
//! A udev rule runs this on block `add` events; udev passes the device in the environment
//! (`DEVNAME`, `ID_FS_LABEL`, `ID_FS_UUID`). It mounts and does nothing else — the file view
//! (E17) then lists the mount point like any other folder.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::process::Command;

    let device = std::env::var("DEVNAME").unwrap_or_default();
    if device.is_empty() {
        eprintln!("flow-mount: DEVNAME is not set — run it from the udev rule");
        return std::process::ExitCode::FAILURE;
    }
    let label = std::env::var("ID_FS_LABEL").unwrap_or_default();
    let uuid = std::env::var("ID_FS_UUID").unwrap_or_default();
    let mp = flow_files::mount_point(&label, &uuid);

    if let Err(e) = std::fs::create_dir_all(&mp) {
        eprintln!("flow-mount: cannot create {mp}: {e}");
        return std::process::ExitCode::FAILURE;
    }
    // `mount(8)` auto-detects the filesystem; the mount point is the pure decision.
    match Command::new("mount").arg(&device).arg(&mp).status() {
        Ok(s) if s.success() => {
            println!("flow-mount: mounted {device} at {mp}");
            // A dedicated marker on the console, so the harness's serial log carries the verdict.
            let _ = std::fs::write(
                "/dev/console",
                format!("flow-mount: mounted {device} at {mp}\n"),
            );
            std::process::ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!("flow-mount: mount {device} failed: {s}");
            std::process::ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("flow-mount: cannot run mount: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("flow-mount: Linux only");
    std::process::exit(2);
}
