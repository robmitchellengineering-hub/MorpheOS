//! `morph-update`: put a new system into the inactive slot, and prove it landed.
//!
//! It writes; it does not boot. Booting a freshly written slot is `morph-boot trial`,
//! which refuses anything but the slot this command is allowed to write — so the two
//! tools agree by construction rather than by convention.

use std::path::PathBuf;
use std::process::ExitCode;

use morph_boot::{BackendOptions, Slot};
use morph_update::{Layout, Payload, TrustedKeys, install};

const USAGE: &str = "\
usage: morph-update [options] <command>

commands:
  keygen  --key FILE           make a signing key pair: FILE (secret, 0600) and FILE.pub
  seal    --payload DIR --release N
                               compute BLAKE3SUMS for a payload directory (the builder's half),
                               saying it is release N
  sign    --payload DIR --key FILE
                               sign a sealed payload: write BLAKE3SUMS.sig
  check   --payload DIR        verify a payload's signature and its own BLAKE3SUMS, write nothing
  install --payload DIR        write the payload into the inactive slot and verify it
          [--slot a|b]         assert which slot that should be (refused if wrong)

options:
  --trust PATH                 the public keys updates may be signed by: a directory of
                               *.pub files, or one file (default /usr/share/morpheos/update-keys)
  --root PATH                  the slot's root, a device or a file
                               (default /dev/disk/by-partlabel/root-<slot>)
  --release-file PATH          which release is running; nothing older is installed
                               (default /usr/lib/morpheos/release)

backend options (default: detected):
  --backend pi|sdboot
  --autoboot PATH              pi: autoboot.txt        (default /bootfs/autoboot.txt)
  --dt PATH                    pi: bootloader DT node  (default /proc/device-tree/chosen/bootloader)
  --esp PATH                   sdboot: EFI system partition (default /efi)
  --cmdline PATH               sdboot: kernel command line  (default /proc/cmdline)";

struct Opts {
    payload: Option<PathBuf>,
    key: Option<PathBuf>,
    trust: PathBuf,
    slot: Option<Slot>,
    root: Option<PathBuf>,
    release: Option<u64>,
    release_file: PathBuf,
    backend: BackendOptions,
    rest: Vec<String>,
}

fn parse(args: Vec<String>) -> Option<Opts> {
    let mut o = Opts {
        payload: None,
        key: None,
        trust: PathBuf::from(morph_update::TRUSTED_KEYS_DIR),
        slot: None,
        root: None,
        release: None,
        release_file: PathBuf::from(morph_update::RELEASE_FILE),
        backend: BackendOptions::default(),
        rest: Vec::new(),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--payload" => o.payload = Some(it.next()?.into()),
            "--key" => o.key = Some(it.next()?.into()),
            "--trust" => o.trust = it.next()?.into(),
            "--root" => o.root = Some(it.next()?.into()),
            "--release" => o.release = Some(it.next()?.parse().ok()?),
            "--release-file" => o.release_file = it.next()?.into(),
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

fn keygen(o: &Opts) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = o.key.as_ref().ok_or("keygen needs --key FILE")?;
    let (secret, public) = morph_update::keygen().map_err(|e| e.to_string())?;
    // Created 0600 and only if absent: a key is never overwritten by accident.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(format!("{secret}\n").as_bytes()))
        .map_err(|e| format!("writing {}: {e}", path.display()))?;
    let public_path = PathBuf::from(format!("{}.pub", path.display()));
    std::fs::write(&public_path, format!("{public}\n"))
        .map_err(|e| format!("writing {}: {e}", public_path.display()))?;
    println!("{public}");
    eprintln!(
        "secret key in {} (keep it; it can update every machine that trusts {})",
        path.display(),
        public_path.display()
    );
    Ok(())
}

fn run(o: &Opts) -> Result<(), String> {
    if o.rest.as_slice() == ["keygen"] {
        return keygen(o);
    }
    let dir = o
        .payload
        .as_ref()
        .ok_or_else(|| "no --payload DIR given".to_string())?;
    let rest: Vec<&str> = o.rest.iter().map(String::as_str).collect();

    // `seal` runs before a manifest exists, so the payload cannot be opened up here —
    // every other command opens it as its first act.
    let open = || {
        let trusted = TrustedKeys::load(&o.trust).map_err(|e| e.to_string())?;
        Payload::open(dir, &trusted).map_err(|e| e.to_string())
    };

    match rest.as_slice() {
        ["seal"] => {
            let release = o.release.ok_or("seal needs --release N")?;
            let sizes = morph_update::seal(dir, release).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&sizes).unwrap());
            eprintln!("sealed {}: {} file(s)", dir.display(), sizes.len());
            Ok(())
        }
        ["sign"] => {
            let key = o.key.as_ref().ok_or("sign needs --key FILE")?;
            let secret = std::fs::read_to_string(key)
                .map_err(|e| format!("reading {}: {e}", key.display()))?;
            morph_update::sign(dir, &secret).map_err(|e| e.to_string())?;
            eprintln!("signed {}", dir.display());
            Ok(())
        }
        ["check"] => {
            let mut payload = open()?;
            let release = payload.release();
            let sizes = payload.check().map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "payload": dir.display().to_string(),
                    "release": release,
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
            let running_release =
                morph_update::read_release(&o.release_file).map_err(|e| e.to_string())?;
            let layout = Layout {
                esp: o.backend.esp.clone(),
                root_device: root,
                running_release,
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
