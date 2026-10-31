//! `morph-attest` — state what this machine is, and sign it.
//!
//!   morph-attest show                     print the statement (unsigned) as JSON
//!   morph-attest sign --key PATH          print the statement, signed
//!   morph-attest verify FILE [--trusted-key HEX]
//!   morph-attest keygen --out PATH        make an attestation key (refuses to overwrite)
//!
//! Exit codes are the contract: 0 said-and-true, 1 refused or stale, 2 misuse.
//!
//! DESIGN NOTE, deliberately repeated here because it is the product's honesty rule: without measured boot
//! (IMA + a TPM, the next phase) the attestation key lives on the machine, so a machine with root can sign a
//! lie. What this gives you today is that a statement cannot be edited after it was signed, that it names
//! *this boot* and *this machine*, and that a statement which no longer matches the machine is reported as
//! stale rather than silently accepted. See docs/ for the threat model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ed25519_dalek::{SigningKey, VerifyingKey};
use morph_attest::{
    canonical, evaluate, from_json, hex, sign, unhex, Statement, SignedStatement, Verdict, FORMAT,
};

/// Where the facts come from. A base path so a test (or an image build) can point at a fixture tree instead
/// of the running system — the collector must be testable, or its output is a claim nobody has ever checked
/// against known input.
pub struct Facts {
    pub root: PathBuf,
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// One field of a `key=value` or `key value` file, for os-release style files.
fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k == key).then(|| v.trim_matches('"').to_string())
    })
}

impl Facts {
    pub fn collect(&self) -> Result<Statement, String> {
        let root = &self.root;
        let os_release = read(&root.join("etc/os-release"))
            .ok_or_else(|| format!("no {}/etc/os-release: this is not a MorpheOS root", root.display()))?;
        let system = field(&os_release, "PRETTY_NAME")
            .or_else(|| field(&os_release, "NAME"))
            .ok_or("os-release names no system")?;

        // The kernel's own answers. /proc/sys/kernel/ostype is the sysname uname(2) returns: "Linux" on a
        // stock kernel, "MorpheOS" on ours. That difference is the whole point of asking.
        let kernel_release = read(&root.join("proc/sys/kernel/osrelease")).unwrap_or_else(|| "unknown".into());
        let kernel_sysname = read(&root.join("proc/sys/kernel/ostype")).unwrap_or_else(|| "unknown".into());
        let cmdline = read(&root.join("proc/cmdline")).unwrap_or_default();
        let slot = cmdline
            .split_whitespace()
            .find_map(|w| w.strip_prefix("morph.slot="))
            .unwrap_or("unknown")
            .to_string();

        let boot_id = read(&root.join("proc/sys/kernel/random/boot_id")).unwrap_or_else(|| "unknown".into());
        let machine_id = read(&root.join("etc/machine-id")).unwrap_or_else(|| "unknown".into());

        let lsm = read(&root.join("sys/kernel/security/lsm"))
            .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();

        let mut posture = BTreeMap::new();
        posture.insert(
            "console".into(),
            if root.join("etc/systemd/system/getty@tty1.service.d/autologin.conf").exists() {
                "autologin".to_string()
            } else {
                "credential-required".to_string()
            },
        );
        posture.insert(
            "lockdown".into(),
            read(&root.join("sys/kernel/security/lockdown")).unwrap_or_else(|| "absent".into()),
        );
        posture.insert(
            "root".into(),
            // The root filesystem's own mount options: MorpheOS promises a read-only root.
            read(&root.join("proc/mounts"))
                .and_then(|m| {
                    m.lines()
                        .find(|l| l.split_whitespace().nth(1) == Some("/"))
                        .and_then(|l| l.split_whitespace().nth(3))
                        .map(|opts| if opts.split(',').any(|o| o == "ro") { "read-only" } else { "writable" })
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "unknown".into()),
        );
        posture.insert(
            "secure_boot".into(),
            // EFI variables are per-vendor-named and the payload is a 4-byte attributes header + value, so
            // read what we can and say "unknown" rather than guess.
            std::fs::read_dir(root.join("sys/firmware/efi/efivars"))
                .ok()
                .and_then(|d| {
                    d.flatten().find(|e| {
                        e.file_name().to_string_lossy().starts_with("SecureBoot-")
                    })
                })
                .and_then(|e| std::fs::read(e.path()).ok())
                .and_then(|b| b.get(4).copied())
                .map(|v| if v == 1 { "on" } else { "off" }.to_string())
                .unwrap_or_else(|| "unknown".into()),
        );

        Ok(Statement {
            format: FORMAT,
            system,
            kernel_release,
            kernel_sysname,
            slot,
            boot_id,
            machine_id,
            lsm,
            posture,
        })
    }
}

fn key_from_file(path: &Path) -> Result<SigningKey, String> {
    let raw = std::fs::read(path).map_err(|e| format!("no attestation key at {}: {e}", path.display()))?;
    let bytes = if raw.len() == 32 {
        raw
    } else {
        // A hex key file is friendlier to provision than 32 binary bytes; accept both, and say which.
        let text = String::from_utf8(raw).map_err(|_| "key file is neither 32 bytes nor hex".to_string())?;
        unhex(text.trim()).ok_or("key file is neither 32 bytes nor hex")?
    };
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "key is not 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&arr))
}

fn parse_flag(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

fn keygen(path: &Path) -> Result<(), String> {
    if path.exists() {
        // Generating a key over an existing one would silently invalidate every statement ever signed.
        return Err(format!("{} already exists; refusing to replace an attestation key", path.display()));
    }
    let mut seed = [0u8; 32];
    // getrandom is part of libc, which the image has; a key from a weak source is not a key.
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut seed))
        .is_ok();
    if !filled {
        return Err("could not read /dev/urandom: refusing to invent a key".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, hex(&seed)).map_err(|e| format!("{}: {e}", path.display()))?;
    // 0600: a key anybody can read attests to nothing.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    let key = SigningKey::from_bytes(&seed);
    println!(
        "attestation key written to {}\nverifying key (give this to whoever must check statements):\n{}",
        path.display(),
        hex(key.verifying_key().as_bytes())
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("show");
    let root = parse_flag(&args, "--root").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));

    match cmd {
        "show" => {
            let facts = Facts { root };
            match facts.collect() {
                Ok(statement) => {
                    println!("{}", String::from_utf8_lossy(&canonical(&statement)));
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("morph-attest: {why}");
                    ExitCode::from(2)
                }
            }
        }
        "sign" => {
            let key_path = match parse_flag(&args, "--key").map(PathBuf::from) {
                Some(p) => p,
                None => {
                    eprintln!("morph-attest sign --key PATH  (no unsigned statement is ever printed)");
                    return ExitCode::from(2);
                }
            };
            let key = match key_from_file(&key_path) {
                Ok(k) => k,
                Err(why) => {
                    // The important refusal: no key means no claim. Printing an unsigned statement and
                    // letting a reader assume it was verified is the failure this whole crate exists to
                    // prevent.
                    eprintln!("morph-attest: {why}");
                    return ExitCode::from(1);
                }
            };
            let facts = Facts { root };
            match facts.collect() {
                Ok(statement) => {
                    let signed = sign(statement, &key);
                    match serde_json::to_string_pretty(&signed) {
                        Ok(text) => {
                            println!("{text}");
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("morph-attest: {e}");
                            ExitCode::from(2)
                        }
                    }
                }
                Err(why) => {
                    eprintln!("morph-attest: {why}");
                    ExitCode::from(2)
                }
            }
        }
        "verify" => {
            let path = match args.iter().skip(1).find(|a| !a.starts_with("--")) {
                Some(p) => PathBuf::from(p),
                None => {
                    eprintln!("morph-attest verify FILE [--trusted-key HEX]");
                    return ExitCode::from(2);
                }
            };
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("morph-attest: {}: {e}", path.display());
                    return ExitCode::from(2);
                }
            };
            let signed: SignedStatement = match from_json(&text) {
                Ok(s) => s,
                Err(why) => {
                    eprintln!("morph-attest: {why}");
                    return ExitCode::from(2);
                }
            };
            let trusted: Option<VerifyingKey> = parse_flag(&args, "--trusted-key")
                .and_then(|h| unhex(&h))
                .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
                .map(|b| VerifyingKey::from_bytes(&b).expect("validated by the length check"));

            // Without --root this compares against THIS machine; with it, against a fixture tree.
            let now = Facts { root }.collect();
            let verdict = match &now {
                Ok(now) => evaluate(&signed, trusted.as_ref(), now),
                Err(_) => morph_attest::verify_signature(&signed, trusted.as_ref()),
            };
            match verdict {
                Verdict::Trusted => {
                    println!(
                        "trusted: signed by {}, format {}, and it matches this machine",
                        signed.key,
                        signed.statement.format
                    );
                    ExitCode::SUCCESS
                }
                Verdict::Stale(changed) => {
                    println!("stale: the signature is good but the machine has moved on");
                    for c in changed {
                        println!("  {c}");
                    }
                    ExitCode::from(1)
                }
                Verdict::Refused(why) => {
                    eprintln!("refused: {why}");
                    ExitCode::from(1)
                }
            }
        }
        "keygen" => {
            let out = match parse_flag(&args, "--out").map(PathBuf::from) {
                Some(p) => p,
                None => {
                    eprintln!("morph-attest keygen --out PATH");
                    return ExitCode::from(2);
                }
            };
            match keygen(&out) {
                Ok(()) => ExitCode::SUCCESS,
                Err(why) => {
                    eprintln!("morph-attest: {why}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("morph-attest: unknown command '{other}' (show|sign|verify|keygen)");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A fixture tree with known contents, so the collector is checked against input we chose rather than
    /// against whatever this machine happens to be.
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("morph-attest-fixture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (path, text) in [
            ("etc/os-release", "NAME=\"MorpheOS\"\nPRETTY_NAME=\"MorpheOS 1.0\"\nID=morpheos\n"),
            ("etc/machine-id", "0123456789abcdef0123456789abcdef\n"),
            ("proc/sys/kernel/osrelease", "6.8.0-morpheos\n"),
            ("proc/sys/kernel/ostype", "MorpheOS\n"),
            ("proc/cmdline", "root=PARTLABEL=root-b ro morph.slot=b console=ttyS0\n"),
            ("proc/sys/kernel/random/boot_id", "0f0e0d0c-0b0a-4908-8706-050403020100\n"),
            ("sys/kernel/security/lsm", "lockdown,bpf,yama\n"),
            ("sys/kernel/security/lockdown", "[integrity] none\n"),
            ("proc/mounts", "/dev/vda2 / ext4 ro,relatime 0 0\n"),
        ] {
            let p = root.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        root
    }

    #[test]
    fn the_collector_reads_a_known_tree_faithfully() {
        let facts = Facts { root: fixture() };
        let s = facts.collect().unwrap();
        assert_eq!(s.system, "MorpheOS 1.0");
        assert_eq!(s.kernel_release, "6.8.0-morpheos");
        assert_eq!(s.kernel_sysname, "MorpheOS", "the sysname is the field that separates us from Linux");
        assert_eq!(s.slot, "b", "the slot comes from the kernel command line");
        assert_eq!(s.lsm, vec!["lockdown", "bpf", "yama"]);
        assert_eq!(s.posture["root"], "read-only");
        assert_eq!(s.posture["console"], "credential-required");
        assert_eq!(s.posture["lockdown"], "[integrity] none");
    }

    #[test]
    fn a_tree_that_is_not_a_morpheos_root_is_an_error_not_a_guess() {
        let empty = std::env::temp_dir().join(format!("morph-attest-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&empty);
        fs::create_dir_all(&empty).unwrap();
        let err = Facts { root: empty.clone() }.collect().unwrap_err();
        assert!(err.contains("not a MorpheOS root"), "unexpected: {err}");
        let _ = fs::remove_dir_all(&empty);
    }

    #[test]
    fn an_autologin_console_is_reported_as_such() {
        let root = fixture();
        let p = root.join("etc/systemd/system/getty@tty1.service.d/autologin.conf");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "[Service]\nExecStart=\n").unwrap();
        let s = Facts { root: root.clone() }.collect().unwrap();
        assert_eq!(s.posture["console"], "autologin");
        let _ = fs::remove_dir_all(&root);
    }
}
