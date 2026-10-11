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

options:
  --esp PATH                 EFI system partition (default /efi); history and the repair
                             report are derived from it unless overridden
  --history PATH             boot history (default <esp>/morph-boot/history.jsonl)
  --report PATH              where repairs are recorded (default <esp>/morph-boot/repairs.json)
  --stage PATH               scratch space for the image being assembled (default /run/morph-heal)
  --root-running PATH        the running slot's root (default /dev/disk/by-partlabel/root-<slot>)
  --root-spare PATH          the failed slot's root (default /dev/disk/by-partlabel/root-<slot>)

backend options (default: detected):
  --backend pi|sdboot
  --autoboot PATH            pi: autoboot.txt        (default /bootfs/autoboot.txt)
  --dt PATH                  pi: bootloader DT node  (default /proc/device-tree/chosen/bootloader)
  --cmdline PATH             sdboot: kernel command line  (default /proc/cmdline)";

struct Opts {
    esp: PathBuf,
    history: Option<PathBuf>,
    report: Option<PathBuf>,
    stage: Option<PathBuf>,
    root_running: Option<PathBuf>,
    root_spare: Option<PathBuf>,
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
            stage: None,
            root_running: None,
            root_spare: None,
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
            "--stage" => o.stage = Some(it.next()?.into()),
            "--root-running" => o.root_running = Some(it.next()?.into()),
            "--root-spare" => o.root_spare = Some(it.next()?.into()),
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

fn run(o: &Opts) -> Result<(), String> {
    let mut heal = Heal::new(o.esp.clone());
    if let Some(h) = &o.history {
        heal.history = h.clone();
    }
    if let Some(r) = &o.report {
        heal.report = r.clone();
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
