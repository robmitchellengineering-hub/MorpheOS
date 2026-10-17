//! `morph-heal`: rebuild the spare slot from the running system after a failed trial.
//!
//! It decides from the boot history and writes through `morph-update`, so the policy that
//! says which slot may be written has exactly one implementation, and the guarantee that
//! what was written is what is on the device has exactly one implementation too.

use std::path::PathBuf;
use std::process::ExitCode;

use morph_boot::{BackendOptions, Slot};
use morph_heal::Heal;

const USAGE: &str = "\
usage: morph-heal [options] <command>

commands:
  status                     what the boot history says, and whether a repair is owed (JSON)
  repair [--dry-run]         rebuild the failed spare slot from the running system
  recover [--dry-run]        rebuild BOTH slots from the recovery image (recovery system only)

options:
  --esp PATH                 EFI system partition (default /efi); history and the repair
                             report are derived from it unless overridden
  --history PATH             boot history (default <esp>/morph-boot/history.jsonl)
  --report PATH              where repairs are recorded (default <esp>/morph-boot/repairs.json)
  --recovery-record PATH     where a recovery is recorded (default <esp>/morph-boot/recovery.json)
  --recovery-dir PATH        the recovery payload on the ESP (default <esp>/morpheos/recovery)
  --trusted-keys PATH        keys that sign the recovery image (default /usr/share/morpheos/update-keys)
  --stage PATH               scratch space for the image being assembled (default /run/morph-heal)
  --release-file PATH        which release is running (default /usr/lib/morpheos/release)
  --root-running PATH        the running slot's root (default /dev/disk/by-partlabel/root-<slot>)
  --root-spare PATH          the failed slot's root (default /dev/disk/by-partlabel/root-<slot>)
  --recovery-root PATH       the recovery partition (default /dev/disk/by-partlabel/recovery)
  --root-a PATH              slot A's root to rebuild (default /dev/disk/by-partlabel/root-a)
  --root-b PATH              slot B's root to rebuild (default /dev/disk/by-partlabel/root-b)

backend options (default: detected):
  --backend pi|sdboot
  --autoboot PATH            pi: autoboot.txt        (default /bootfs/autoboot.txt)
  --dt PATH                  pi: bootloader DT node  (default /proc/device-tree/chosen/bootloader)
  --cmdline PATH             sdboot: kernel command line  (default /proc/cmdline)";

struct Opts {
    esp: PathBuf,
    history: Option<PathBuf>,
    report: Option<PathBuf>,
    recovery_record: Option<PathBuf>,
    recovery_dir: Option<PathBuf>,
    trusted_keys: Option<PathBuf>,
    stage: Option<PathBuf>,
    release_file: Option<PathBuf>,
    root_running: Option<PathBuf>,
    root_spare: Option<PathBuf>,
    recovery_root: Option<PathBuf>,
    root_a: Option<PathBuf>,
    root_b: Option<PathBuf>,
    backend: BackendOptions,
    dry_run: bool,
    rest: Vec<String>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            esp: PathBuf::from("/efi"),
            history: None,
            report: None,
            recovery_record: None,
            recovery_dir: None,
            trusted_keys: None,
            stage: None,
            release_file: None,
            root_running: None,
            root_spare: None,
            recovery_root: None,
            root_a: None,
            root_b: None,
            backend: BackendOptions::default(),
            dry_run: false,
            rest: Vec::new(),
        }
    }
}

fn parse(args: Vec<String>) -> Option<Opts> {
    let mut o = Opts::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--esp" => {
                let p = PathBuf::from(it.next()?);
                o.backend.esp = p.clone();
                o.esp = p;
            }
            "--history" => o.history = Some(it.next()?.into()),
            "--report" => o.report = Some(it.next()?.into()),
            "--recovery-record" => o.recovery_record = Some(it.next()?.into()),
            "--recovery-dir" => o.recovery_dir = Some(it.next()?.into()),
            "--trusted-keys" => o.trusted_keys = Some(it.next()?.into()),
            "--stage" => o.stage = Some(it.next()?.into()),
            "--release-file" => o.release_file = Some(it.next()?.into()),
            "--root-running" => o.root_running = Some(it.next()?.into()),
            "--root-spare" => o.root_spare = Some(it.next()?.into()),
            "--recovery-root" => o.recovery_root = Some(it.next()?.into()),
            "--root-a" => o.root_a = Some(it.next()?.into()),
            "--root-b" => o.root_b = Some(it.next()?.into()),
            "--dry-run" => o.dry_run = true,
            "--backend" => o.backend.backend = Some(it.next()?),
            "--autoboot" => o.backend.autoboot = it.next()?.into(),
            "--dt" => o.backend.dt = it.next()?.into(),
            "--cmdline" => o.backend.cmdline = it.next()?.into(),
            _ => o.rest.push(a),
        }
    }
    Some(o)
}

fn by_partlabel(slot: Slot) -> PathBuf {
    PathBuf::from(format!("/dev/disk/by-partlabel/root-{}", slot.as_str()))
}

fn by_recovery_label() -> PathBuf {
    PathBuf::from("/dev/disk/by-partlabel/recovery")
}

fn run(o: &Opts) -> Result<(), String> {
    let mut heal = Heal::new(o.esp.clone());
    if let Some(h) = &o.history {
        heal.history = h.clone();
    }
    if let Some(r) = &o.report {
        heal.report = r.clone();
    }
    if let Some(r) = &o.recovery_record {
        heal.recovery_record = r.clone();
    }
    if let Some(r) = &o.recovery_dir {
        heal.recovery_dir = r.clone();
    }
    if let Some(t) = &o.trusted_keys {
        heal.trusted_keys = t.clone();
    }
    if let Some(r) = &o.release_file {
        heal.release_file = r.clone();
    }
    if let Some(s) = &o.stage {
        heal.stage = s.clone();
    }

    let rest: Vec<&str> = o.rest.iter().map(String::as_str).collect();
    match rest.as_slice() {
        ["status"] => {
            let failure = morph_heal::last_failure(&heal.history).map_err(|e| e.to_string())?;
            let owed = heal.needed().map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "history": heal.history.display().to_string(),
                    "last_failure": failure,
                    "repair_owed": owed,
                })
            );
            Ok(())
        }
        ["repair"] => {
            let bc = morph_boot::backend(&o.backend);
            let running = morph_boot::status(bc.as_ref())
                .map_err(|e| e.to_string())?
                .booted;
            let owed = heal.needed().map_err(|e| e.to_string())?;
            let Some(failure) = owed else {
                println!(
                    "{}",
                    serde_json::json!({"action": "none", "why": "no unrepaired failed trial"})
                );
                return Ok(());
            };
            if o.dry_run {
                println!(
                    "{}",
                    serde_json::json!({"action": "would-repair", "failure": failure})
                );
                return Ok(());
            }
            let running_root = o
                .root_running
                .clone()
                .unwrap_or_else(|| by_partlabel(running));
            let spare_root = o
                .root_spare
                .clone()
                .unwrap_or_else(|| by_partlabel(failure.slot));
            let (failure, outcome) = heal
                .repair(bc.as_ref(), running, &running_root, &spare_root)
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "action": "repaired",
                    "failure": failure,
                    "from": running,
                    "outcome": outcome,
                })
            );
            eprintln!(
                "slot {} was rebuilt from slot {} and verified; trial it with: morph-boot trial {}",
                failure.slot, running, failure.slot
            );
            Ok(())
        }
        ["recover"] => {
            let cmdline = std::fs::read_to_string(&o.backend.cmdline)
                .map_err(|e| format!("reading {}: {e}", o.backend.cmdline.display()))?;
            let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
                .map_err(|e| format!("reading /proc/self/mountinfo: {e}"))?;
            if o.dry_run {
                let recovering = morph_boot::is_recovery(&cmdline, &mountinfo);
                println!(
                    "{}",
                    serde_json::json!({
                        "action": if recovering { "would-recover" } else { "refused" },
                        "recovering": recovering,
                    })
                );
                return Ok(());
            }
            let recovery_root = o.recovery_root.clone().unwrap_or_else(by_recovery_label);
            let root_a = o.root_a.clone().unwrap_or_else(|| by_partlabel(Slot::A));
            let root_b = o.root_b.clone().unwrap_or_else(|| by_partlabel(Slot::B));
            let mut bc = morph_boot::backend(&o.backend);
            let recovered = heal
                .recover(
                    bc.as_mut(),
                    &cmdline,
                    &mountinfo,
                    &recovery_root,
                    &root_a,
                    &root_b,
                )
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "action": "recovered",
                    "from": "recovery",
                    "release": recovered.release,
                    "default": recovered.default,
                    "a": recovered.a,
                    "b": recovered.b,
                })
            );
            eprintln!(
                "rebuilt slots a and b from the recovery image (release {}); default is slot {}",
                recovered.release, recovered.default
            );
            Ok(())
        }
        _ => Err(format!("unknown command\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    let Some(o) = parse(std::env::args().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(&o) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("morph-heal: {e}");
            ExitCode::FAILURE
        }
    }
}
