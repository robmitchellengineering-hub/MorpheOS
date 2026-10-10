//! `morph-update`: put a new system into the inactive slot, and prove it landed.
//!
//! It writes; it does not boot. Booting a freshly written slot is `morph-boot trial`,
//! which refuses anything but the slot this command is allowed to write — so the two
//! tools agree by construction rather than by convention.

use std::path::PathBuf;
use std::process::ExitCode;

use morph_boot::{BackendOptions, Slot};
use morph_update::{Layout, Payload, install};

const USAGE: &str = "\
usage: morph-update [options] <command>

commands:
  seal    --payload DIR        compute BLAKE3SUMS for a payload directory (the builder's half)
  check   --payload DIR        verify a payload against its own BLAKE3SUMS, write nothing
  install --payload DIR        write the payload into the inactive slot and verify it
          [--slot a|b]         assert which slot that should be (refused if wrong)

options:
  --root PATH                  the slot's root, a device or a file
                               (default /dev/disk/by-partlabel/root-<slot>)

backend options (default: detected):
  --backend pi|sdboot
  --autoboot PATH              pi: autoboot.txt        (default /bootfs/autoboot.txt)
  --dt PATH                    pi: bootloader DT node  (default /proc/device-tree/chosen/bootloader)
  --esp PATH                   sdboot: EFI system partition (default /efi)
  --cmdline PATH               sdboot: kernel command line  (default /proc/cmdline)";

struct Opts {
    payload: Option<PathBuf>,
    slot: Option<Slot>,
    root: Option<PathBuf>,
    backend: BackendOptions,
    rest: Vec<String>,
}

fn parse(args: Vec<String>) -> Option<Opts> {
    let mut o = Opts {
        payload: None,
        slot: None,
        root: None,
        backend: BackendOptions::default(),
        rest: Vec::new(),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--payload" => o.payload = Some(it.next()?.into()),
            "--root" => o.root = Some(it.next()?.into()),
            "--slot" => o.slot = Slot::parse(&it.next()?),
            "--backend" => o.backend.backend = Some(it.next()?),
            "--autoboot" => o.backend.autoboot = it.next()?.into(),
            "--dt" => o.backend.dt = it.next()?.into(),
            "--esp" => o.backend.esp = it.next()?.into(),
            "--cmdline" => o.backend.cmdline = it.next()?.into(),
            _ => o.rest.push(a),
        }
    }
    Some(o)
}

fn run(o: &Opts) -> Result<(), String> {
    let dir = o
        .payload
        .as_ref()
        .ok_or_else(|| "no --payload DIR given".to_string())?;
    let rest: Vec<&str> = o.rest.iter().map(String::as_str).collect();

    // `seal` runs before a manifest exists, so the payload cannot be opened up here —
    // every other command opens it as its first act.
    let open = || Payload::open(dir).map_err(|e| e.to_string());

    match rest.as_slice() {
        ["seal"] => {
            let sizes = morph_update::seal(dir).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&sizes).unwrap());
            eprintln!("sealed {}: {} file(s)", dir.display(), sizes.len());
            Ok(())
        }
        ["check"] => {
            let mut payload = open()?;
            let sizes = payload.check().map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "payload": dir.display().to_string(),
                    "files": sizes,
                    "root_image": payload.digest_of(morph_update::ROOT_IMAGE),
                })
            );
            Ok(())
        }
        ["install"] => {
            let mut payload = open()?;
            let bc = morph_boot::backend(&o.backend);
            // Resolve the policy FIRST, because the default device path depends on which
            // slot the policy names — and because the caller's `--slot` is an assertion
            // to be checked, not an instruction to be followed.
            let target = morph_boot::update_target(bc.as_ref()).map_err(|e| e.to_string())?;
            let root = o.root.clone().unwrap_or_else(|| {
                PathBuf::from(format!("/dev/disk/by-partlabel/root-{}", target.as_str()))
            });
            let layout = Layout {
                esp: o.backend.esp.clone(),
                root_device: root,
            };
            let outcome =
                install(bc.as_ref(), &mut payload, &layout, o.slot).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&outcome).unwrap());
            eprintln!(
                "slot {} holds the new system; boot it once with: morph-boot trial {} --reboot",
                outcome.slot, outcome.slot
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
            eprintln!("morph-update: {e}");
            ExitCode::FAILURE
        }
    }
}
