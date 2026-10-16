//! Mount a removable disk where the pure `mount_point` decides (doc 20, E18).
//!
//! A udev rule hands each block `add` event to a systemd service, which runs this as root WITH
//! `CAP_SYS_ADMIN`. udev's own `RUN` programs cannot mount: systemd-udevd runs them without
//! `CAP_SYS_ADMIN`, so `mount(2)` fails with EPERM even as uid 0. The service passes the device
//! as argv[1]; the label, uuid and type come from the device itself (`blkid`) when the udev
//! environment that normally carries them is absent.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::process::Command;

    /// Say it to the journal, and to the console the harness's serial log carries (so the
    /// QEMU test sees both the mount and its failure).
    fn announce(line: &str) {
        println!("flow-mount: {line}");
        let _ = std::fs::write("/dev/console", format!("flow-mount: {line}\n"));
    }

    /// Ask the device itself for one property (LABEL, UUID, TYPE) when the udev environment
    /// that normally carries it is absent — the systemd-service path.
    fn blkid_field(device: &str, key: &str) -> String {
        let Ok(out) = Command::new("blkid")
            .arg("-p")
            .arg("-o")
            .arg("export")
            .arg(device)
            .output()
        else {
            return String::new();
        };
        if !out.status.success() {
            return String::new();
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(v) = line.strip_prefix(&format!("{key}=")) {
                return v.trim().to_string();
            }
        }
        String::new()
    }

    // The device: the systemd service passes it as argv[1]; udev's RUN used DEVNAME.
    let device = std::env::args()
        .nth(1)
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("DEVNAME").ok())
        .unwrap_or_default();
    if device.is_empty() {
        announce("no device — run it as morpheos-usb-mount@<dev>.service or from the udev rule");
        return std::process::ExitCode::FAILURE;
    }

    // udev passes these in its environment; the systemd service does not, so ask the device.
    let mut label = std::env::var("ID_FS_LABEL").unwrap_or_default();
    let mut uuid = std::env::var("ID_FS_UUID").unwrap_or_default();
    let mut fs_type = std::env::var("ID_FS_TYPE").unwrap_or_default();
    if label.is_empty() {
        label = blkid_field(&device, "LABEL");
    }
    if uuid.is_empty() {
        uuid = blkid_field(&device, "UUID");
    }
    if fs_type.is_empty() {
        fs_type = blkid_field(&device, "TYPE");
    }
    let mp = flow_files::mount_point(&label, &uuid);

    if let Err(e) = std::fs::create_dir_all(&mp) {
        announce(&format!("cannot create {mp}: {e}"));
        return std::process::ExitCode::FAILURE;
    }
    // `mount(8)` with the type the device probed, so a partitionless stick still mounts; the
    // mount point is the pure decision.
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
