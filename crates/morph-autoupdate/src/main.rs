//! `morph-autoupdate`: the unattended-update timer entry point (doc 20, S3).
//!
//! The timer's unit runs this with the server's base URL (and, on the test image, the
//! throwaway CA); it decides, fetches, installs and arms the trial, and it prints and exits 0
//! when there is nothing to do — so a machine with no network, or nothing newer, is a healthy
//! boot, not a failed unit.

use std::process::ExitCode;

use morph_autoupdate::{Config, run};

const USAGE: &str = "\
usage: morph-autoupdate [options]

options:
  --base URL            the update server's base URL, e.g. https://10.0.2.2:8123 (required)
  --ca PATH             an extra CA certificate to trust (the test image's throwaway CA;
                        production passes none)
  --trust PATH          the public keys updates may be signed by (default
                        /usr/share/morpheos/update-keys)
  --release-file PATH   which release is running (default /usr/lib/morpheos/release)
  --esp PATH            the EFI system partition (default /efi)
  --cmdline PATH        the kernel command line (default /proc/cmdline)
  --state PATH          the armed-trial record (default /persist/morph-update/state.json)
  --history PATH        the boot history (default /efi/morph-boot/history.jsonl)
  --staging PATH        where a payload is downloaded (default /persist/morph-update/staging)
  --reboot-now          reboot right after arming (the test drop-in), instead of at the next
                        boot a person makes";

/// Read the three settings the unit/drop-in pass through the environment (doc 20, S3 note 4
/// and 6): the base URL, the throwaway test CA, and "reboot now". The service sets the base;
/// the test drop-in overrides it and adds the other two; the command line can still override
/// any of them for a manual run.
fn from_env(cfg: &mut Config) {
    if let Ok(base) = std::env::var("MORPH_UPDATE_BASE")
        && !base.is_empty()
    {
        cfg.base = base;
    }
    if let Ok(ca) = std::env::var("MORPH_UPDATE_CA")
        && !ca.is_empty()
    {
        cfg.ca = Some(ca.into());
    }
    if std::env::var("MORPH_UPDATE_REBOOT").as_deref() == Ok("now") {
        cfg.reboot_now = true;
    }
}

fn main() -> ExitCode {
    let mut cfg = Config::default();
    from_env(&mut cfg);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value =
            |name: &str| -> String { it.next().unwrap_or_else(|| panic!("{name} needs a value")) };
        match a.as_str() {
            "--base" => cfg.base = value("--base"),
            "--ca" => cfg.ca = Some(value("--ca").into()),
            "--trust" => cfg.trust = value("--trust").into(),
            "--release-file" => cfg.release_file = value("--release-file").into(),
            "--esp" => cfg.esp = value("--esp").into(),
            "--cmdline" => cfg.cmdline = value("--cmdline").into(),
            "--state" => cfg.state_file = value("--state").into(),
            "--history" => cfg.history_file = value("--history").into(),
            "--staging" => cfg.staging = value("--staging").into(),
            "--reboot-now" => cfg.reboot_now = true,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if cfg.base.is_empty() {
        eprintln!("morph-autoupdate: --base URL is required\n{USAGE}");
        return ExitCode::from(2);
    }
    match run(&cfg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("morph-autoupdate: {e}");
            ExitCode::FAILURE
        }
    }
}
