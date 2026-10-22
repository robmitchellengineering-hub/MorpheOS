//! Write a new system into the inactive slot, verify it, and only then trial it.
//!
//! `crates/morph-boot` decides *which* slot may be written to and never writes to one
//! itself. `morph-update` is the other half: it is what actually puts a new system into
//! that slot. Until this existed, `morph-boot trial b` trialled whatever happened to be
//! in slot B already — and on a freshly flashed image that is a **byte-identical copy of
//! slot A**, because both slots are built from one image. So the A/B flow was exercised
//! without any update having happened, which is the one thing it was supposed to prove.
//!
//! # The order of operations is the design
//!
//! 1. **Ask the policy which slot is safe** — [`morph_boot::update_target`], never a
//!    caller's opinion. Writing the running system is the one unrecoverable mistake
//!    available here, so it is not a parameter.
//! 2. **Verify the payload against its own digest manifest before writing anything.** A
//!    truncated download must be refused while the slot is still intact.
//! 3. **Mark the slot incomplete**, so an update interrupted by a power cut is detectable
//!    rather than silently bootable.
//! 4. Write the root image and the boot files.
//! 5. **Read the root map back off the device and hash it.** Not the bytes we meant to
//!    write — the bytes that are actually there, read through the block device, which is
//!    the only version that survives a lying page cache or a short write.
//! 6. Only then clear the marker.
//!
//! A failure at any point leaves the marker in place and the inactive slot untouched or
//! incomplete — never silently usable. The running system is unaffected throughout.
//!
//! # Signed: the payload must come from someone the image trusts (doc 20, S2)
//!
//! A digest manifest proves a payload is intact, not that it is *wanted*: anyone can write a
//! root image and a matching `BLAKE3SUMS`. So `BLAKE3SUMS.sig` is an ed25519 signature over
//! the manifest's exact bytes, and [`Payload::open`] takes the image's [`TrustedKeys`] and
//! checks it before it even parses the manifest. The manifest pins every file by digest, so
//! one signature covers the whole payload. There is no way to open a payload without keys:
//! an unsigned path would be the one an attacker takes.
//!
//! ⚠️ THE VERIFIER MUST BE THE IMAGE'S OWN. An update medium can carry any binary it likes,
//! including a `morph-update` that checks nothing, so the check is only worth something when
//! the machine runs the copy in its own read-only root, with the keys in its own root.
//!
//! # Why BLAKE3 and not SHA-256
//!
//! `crates/strata` already content-addresses every stored chunk by BLAKE3. One hash
//! function in the project is one thing to reason about; a second would be a second
//! thing to get wrong, for no benefit at these sizes.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use morph_boot::Slot;

/// The three files an update payload consists of, and the manifest that names them.
pub const ROOT_IMAGE: &str = "root.img";
pub const KERNEL: &str = "vmlinuz";
pub const INITRD: &str = "initrd.img";
pub const MANIFEST: &str = "BLAKE3SUMS";
/// The ed25519 signature over [`MANIFEST`]'s bytes, as 128 hex characters.
pub const SIGNATURE: &str = "BLAKE3SUMS.sig";
/// Where an image keeps the public keys it accepts updates from, one `*.pub` file each.
pub const TRUSTED_KEYS_DIR: &str = "/usr/share/morpheos/update-keys";
const REQUIRED: [&str; 3] = [ROOT_IMAGE, KERNEL, INITRD];
/// Where an image says which release it is: one decimal number, written by the image build.
/// A payload older than it is refused, so an old release with a hole the new one fixed cannot
/// be put back on the machine, however correctly it was signed.
pub const RELEASE_FILE: &str = "/usr/lib/morpheos/release";

/// The release the running system says it is, from `path` ([`RELEASE_FILE`]). An image that
/// does not say cannot tell an old payload from a new one, so this is an error, not 0.
pub fn read_release(path: &Path) -> Result<u64> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Error::Policy(format!(
            "{}: {e}; the running system does not say which release it is, so an older \
             payload could not be told from a newer one",
            path.display()
        ))
    })?;
    text.trim().parse().map_err(|_| {
        Error::Policy(format!(
            "{} says '{}', not a release number",
            path.display(),
            text.trim()
        ))
    })
}

/// Marker left in a slot's boot directory while an update to it is incomplete.
pub const INCOMPLETE: &str = ".morph-incomplete";

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    /// The payload does not match its own manifest: missing file, or a digest
    /// that disagrees. Caught before anything is written.
    BadPayload(String),
    /// The payload is not signed by a key this machine trusts: no signature, a bad one, a
    /// key not on the list, or a manifest changed after it was signed. Caught before
    /// anything is written, and before the manifest is even read.
    Untrusted(String),
    /// The payload was written and what came back off the device does not match it.
    VerifyFailed(String),
    /// Policy says this slot must not be written to.
    Refused(String),
    /// The boot policy would not answer, or refused: kept separate from `Refused`
    /// because "you asked for the wrong slot" and "the machine will not tell me which
    /// slot is safe" are different problems with different fixes.
    Policy(String),
    /// A previous update to this slot never finished.
    Incomplete(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::BadPayload(m) => write!(f, "payload is not usable: {m}"),
            Error::Untrusted(m) => write!(f, "payload is not trusted: {m}"),
            Error::VerifyFailed(m) => write!(f, "verification failed: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
            Error::Policy(m) => write!(f, "boot policy: {m}"),
            Error::Incomplete(m) => write!(f, "incomplete update: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

/// Hash a reader to completion. Used for both payload files and read-back.
fn hash_reader(mut r: impl Read, what: &str) -> Result<(String, u64)> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(io(format!("reading {what}")))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hasher.finalize().to_hex().to_string(), total))
}

/// Whether a payload file can be read at all.
///
/// ⚠️ **NOT `Path::is_file()`, WHICH IS FALSE FOR A DEVICE.** A payload's root image is
/// legitimately a **block device** when a repair reuses the running system's own image
/// rather than copying it: `morph-heal` symlinks the running root into the payload, which is
/// the difference between a repair that needs 1.2 GiB of RAM and one that needs none. The
/// first version of this used `is_file()` and rejected exactly that case with *"root.img is
/// missing"* — a file that was present and perfectly readable. **Ask what is needed — can it
/// be read — not what it is.**
fn readable(path: &Path) -> bool {
    std::fs::OpenOptions::new().read(true).open(path).is_ok()
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    let f = std::fs::File::open(path).map_err(io(format!("opening {}", path.display())))?;
    hash_reader(f, &path.display().to_string())
}

/// Hash the first `len` bytes of a device or file — the read-back half of the update.
fn hash_prefix(path: &Path, len: u64) -> Result<String> {
    let mut f = std::fs::File::open(path).map_err(io(format!("opening {}", path.display())))?;
    f.seek(SeekFrom::Start(0))
        .map_err(io(format!("seeking {}", path.display())))?;
    let (digest, read) = hash_reader(f.take(len), &path.display().to_string())?;
    if read != len {
        return Err(Error::VerifyFailed(format!(
            "{} is {read} bytes but {len} were written to it",
            path.display()
        )));
    }
    Ok(digest)
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The public keys an image accepts updates from.
#[derive(Debug, Clone)]
pub struct TrustedKeys {
    keys: Vec<(String, ed25519_dalek::VerifyingKey)>,
}

impl TrustedKeys {
    /// Every `*.pub` file in `dir` (or `dir` itself, if it is one file): a 64-hex-character
    /// ed25519 public key on the first line that is not a `#` comment. **No keys is an
    /// error**, not "trust everything": a machine with an empty list accepts nothing.
    pub fn load(path: &Path) -> Result<TrustedKeys> {
        let files: Vec<PathBuf> = if path.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(path)
                .map_err(io(format!("reading {}", path.display())))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "pub"))
                .collect();
            v.sort();
            v
        } else {
            vec![path.to_path_buf()]
        };
        let mut keys = Vec::new();
        for f in files {
            let text =
                std::fs::read_to_string(&f).map_err(io(format!("reading {}", f.display())))?;
            let line = text
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .unwrap_or("");
            let key = unhex::<32>(line)
                .and_then(|b| ed25519_dalek::VerifyingKey::from_bytes(&b).ok())
                .ok_or_else(|| {
                    Error::Untrusted(format!("{} is not an ed25519 public key", f.display()))
                })?;
            keys.push((f.display().to_string(), key));
        }
        if keys.is_empty() {
            return Err(Error::Untrusted(format!(
                "no trusted update keys in {}: this machine accepts no update",
                path.display()
            )));
        }
        Ok(TrustedKeys { keys })
    }

    /// Exactly one key, given as hex: for a caller that signs its own payload with a key it
    /// made a moment ago (`morph-heal`, staging the running system), so that it opens the
    /// payload through the same signature check as everyone else.
    pub fn from_public_hex(name: &str, public_hex: &str) -> Result<TrustedKeys> {
        let key = unhex::<32>(public_hex)
            .and_then(|b| ed25519_dalek::VerifyingKey::from_bytes(&b).ok())
            .ok_or_else(|| Error::Untrusted(format!("{name} is not an ed25519 public key")))?;
        Ok(TrustedKeys {
            keys: vec![(name.to_string(), key)],
        })
    }

    /// Check a signature over `message` against every trusted key; the name of the key
    /// that made it, if one did.
    fn verify(&self, message: &[u8], signature: &ed25519_dalek::Signature) -> Option<&str> {
        self.keys
            .iter()
            .find(|(_, k)| k.verify_strict(message, signature).is_ok())
            .map(|(name, _)| name.as_str())
    }
}

/// A new signing key pair: the secret seed and the public key, both hex. The seed comes from
/// the kernel's random source. **The secret is the whole of the trust**: whoever holds it can
/// update every machine whose image carries the public key.
pub fn keygen() -> Result<(String, String)> {
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .map_err(io("reading /dev/urandom"))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    Ok((hex(&seed), hex(key.verifying_key().as_bytes())))
}

/// Sign a sealed payload: write `BLAKE3SUMS.sig` over its `BLAKE3SUMS`, with the secret seed
/// in `secret_hex` (as [`keygen`] wrote it).
pub fn sign(dir: &Path, secret_hex: &str) -> Result<()> {
    use ed25519_dalek::Signer;
    let seed = unhex::<32>(secret_hex)
        .ok_or_else(|| Error::Untrusted("the signing key is not a 64-hex-character seed".into()))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let manifest = dir.join(MANIFEST);
    let bytes = std::fs::read(&manifest).map_err(io(format!("reading {}", manifest.display())))?;
    let sig = key.sign(&bytes);
    let out = dir.join(SIGNATURE);
    std::fs::write(&out, format!("{}\n", hex(&sig.to_bytes())))
        .map_err(io(format!("writing {}", out.display())))
}

/// A payload directory: the new system, described by its own digest manifest.
#[derive(Debug)]
pub struct Payload {
    dir: PathBuf,
    /// The release it says it is (its manifest's `release <n>` line, signed with the rest).
    release: u64,
    /// name -> BLAKE3 hex digest, as declared.
    digests: BTreeMap<String, String>,
    /// name -> size in bytes, measured when the manifest was checked.
    sizes: BTreeMap<String, u64>,
}

impl Payload {
    /// Check `<dir>/BLAKE3SUMS` is signed by one of `trusted`, then read it and check it
    /// covers exactly the three required files.
    ///
    /// The signature comes first: a manifest from someone the machine does not trust is not
    /// parsed at all. This does **not** hash the files — [`Payload::check`] does, and that
    /// is a separate call so that the cheap checks can happen anywhere.
    pub fn open(dir: &Path, trusted: &TrustedKeys) -> Result<Payload> {
        let manifest = dir.join(MANIFEST);
        let bytes = std::fs::read(&manifest)
            .map_err(|e| Error::BadPayload(format!("{}: {e}", manifest.display())))?;
        let sig_path = dir.join(SIGNATURE);
        let sig_text = std::fs::read_to_string(&sig_path).map_err(|e| {
            Error::Untrusted(format!(
                "{}: {e} (an unsigned payload is refused)",
                sig_path.display()
            ))
        })?;
        let signature = unhex::<64>(&sig_text)
            .map(|b| ed25519_dalek::Signature::from_bytes(&b))
            .ok_or_else(|| {
                Error::Untrusted(format!(
                    "{} is not an ed25519 signature",
                    sig_path.display()
                ))
            })?;
        if trusted.verify(&bytes, &signature).is_none() {
            return Err(Error::Untrusted(format!(
                "{} is not signed by a trusted key: signed by another key, or changed after signing",
                manifest.display()
            )));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::BadPayload(format!("{} is not text", manifest.display())))?;
        let mut digests = BTreeMap::new();
        let mut release = None;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // "release <n>": which release this is. Required, once; it is under the same
            // signature as the digests, so it cannot be changed without the check failing.
            if let Some(number) = line.strip_prefix("release ") {
                let number = number.trim().parse::<u64>().map_err(|_| {
                    Error::BadPayload(format!(
                        "{}:{}: '{line}' is not 'release <number>'",
                        manifest.display(),
                        n + 1
                    ))
                })?;
                if release.replace(number).is_some() {
                    return Err(Error::BadPayload(format!(
                        "{}: more than one release line",
                        manifest.display()
                    )));
                }
                continue;
            }
            // "<hex>  <name>" — the format coreutils writes, so an operator can check
            // the payload by hand without special tooling.
            let mut parts = line.split_whitespace();
            let (Some(hex), Some(name)) = (parts.next(), parts.next()) else {
                return Err(Error::BadPayload(format!(
                    "{}:{}: expected '<blake3-hex>  <name>'",
                    manifest.display(),
                    n + 1
                )));
            };
            if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::BadPayload(format!(
                    "{}:{}: '{hex}' is not a BLAKE3 digest",
                    manifest.display(),
                    n + 1
                )));
            }
            if digests
                .insert(name.to_string(), hex.to_lowercase())
                .is_some()
            {
                return Err(Error::BadPayload(format!(
                    "{}: {name} is listed twice",
                    manifest.display()
                )));
            }
        }
        let release = release.ok_or_else(|| {
            Error::BadPayload(format!(
                "{}: no 'release <number>' line, so an old payload could not be told from a new one",
                manifest.display()
            ))
        })?;
        for want in REQUIRED {
            if !digests.contains_key(want) {
                return Err(Error::BadPayload(format!(
                    "{}: no digest for {want}; a payload is exactly {REQUIRED:?}",
                    manifest.display()
                )));
            }
        }
        if let Some(extra) = digests.keys().find(|k| !REQUIRED.contains(&k.as_str())) {
            return Err(Error::BadPayload(format!(
                "{}: unexpected entry '{extra}'; a payload is exactly {REQUIRED:?}",
                manifest.display()
            )));
        }
        Ok(Payload {
            dir: dir.to_path_buf(),
            release,
            digests,
            sizes: BTreeMap::new(),
        })
    }

    /// Hash every file and compare it against the manifest. Returns the sizes.
    ///
    /// **This runs before anything is written.** A truncated or swapped payload must be
    /// refused while the inactive slot is still a usable system, or the update turns a
    /// recoverable situation into an unrecoverable one.
    pub fn check(&mut self) -> Result<&BTreeMap<String, u64>> {
        for name in REQUIRED {
            let path = self.dir.join(name);
            if !readable(&path) {
                return Err(Error::BadPayload(format!(
                    "{} cannot be read",
                    path.display()
                )));
            }
            let (got, size) = hash_file(&path)?;
            let want = &self.digests[name];
            if &got != want {
                return Err(Error::BadPayload(format!(
                    "{name} hashes to {got}, but the manifest says {want}"
                )));
            }
            self.sizes.insert(name.to_string(), size);
        }
        Ok(&self.sizes)
    }

    /// Hash the kernel and initrd against the manifest, but **not** the root image.
    ///
    /// The root image may be a mounted block device — the recovery partition — whose raw
    /// bytes are not stable: the kernel updates the superblock and journal on mount, so a
    /// full-partition hash would falsely refuse a healthy system. The boot files live on the
    /// ESP and are stable, so they are still checked.
    pub fn check_boot(&mut self) -> Result<()> {
        for name in [KERNEL, INITRD] {
            let path = self.dir.join(name);
            if !readable(&path) {
                return Err(Error::BadPayload(format!(
                    "{} cannot be read",
                    path.display()
                )));
            }
            let (got, _size) = hash_file(&path)?;
            let want = &self.digests[name];
            if &got != want {
                return Err(Error::BadPayload(format!(
                    "{name} hashes to {got}, but the manifest says {want}"
                )));
            }
        }
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The release this payload says it is.
    pub fn release(&self) -> u64 {
        self.release
    }

    pub fn size_of(&self, name: &str) -> Option<u64> {
        self.sizes.get(name).copied()
    }

    pub fn digest_of(&self, name: &str) -> Option<&str> {
        self.digests.get(name).map(String::as_str)
    }
}

/// Write `<dir>/BLAKE3SUMS` for a payload directory, and report the sizes.
///
/// The counterpart to [`Payload::open`], for whoever *builds* a payload. It lives here
/// so the manifest format has exactly one implementation: a builder and a consumer that
/// each parse it their own way is a format with two definitions and no authority.
///
/// **The tests deliberately do not use this to build their fixtures.** They compute the
/// digests independently, so a mistake shared by `seal` and `check` cannot cancel itself
/// out and pass — the failure mode of testing a verifier with its own signer.
pub fn seal(dir: &Path, release: u64) -> Result<BTreeMap<String, u64>> {
    let mut sizes = BTreeMap::new();
    let mut text = format!("release {release}\n");
    for name in REQUIRED {
        let path = dir.join(name);
        if !readable(&path) {
            return Err(Error::BadPayload(format!(
                "{} cannot be read",
                path.display()
            )));
        }
        let (digest, size) = hash_file(&path)?;
        text.push_str(&format!("{digest}  {name}\n"));
        sizes.insert(name.to_string(), size);
    }
    let manifest = dir.join(MANIFEST);
    std::fs::write(&manifest, text).map_err(io(format!("writing {}", manifest.display())))?;
    Ok(sizes)
}

/// Where the pieces of a slot live on this machine.
#[derive(Debug, Clone)]
pub struct Layout {
    /// EFI system partition: boot files are written under `morpheos/<slot>/`.
    pub esp: PathBuf,
    /// The inactive slot's root, as a block device or (in tests) a plain file.
    pub root_device: PathBuf,
    /// The release the running system is ([`read_release`]): nothing older is installed.
    pub running_release: u64,
}

impl Layout {
    /// The directory a slot's boot files belong in.
    pub fn boot_dir(&self, slot: Slot) -> PathBuf {
        self.esp.join("morpheos").join(slot.to_string())
    }

    /// The file marking an update to `slot` as unfinished.
    pub fn marker(&self, slot: Slot) -> PathBuf {
        self.boot_dir(slot).join(INCOMPLETE)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub slot: Slot,
    pub release: u64,
    pub root_bytes: u64,
    pub root_digest: String,
    pub kernel_bytes: u64,
    pub initrd_bytes: u64,
}

/// Write `contents` under `path` as atomically as a filesystem allows.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension("morph-new");
    std::fs::write(&tmp, contents).map_err(io(format!("writing {}", tmp.display())))?;
    // Flush the file's own data before the rename, so the rename cannot publish a name
    // whose contents are still only in cache.
    let f = std::fs::File::open(&tmp).map_err(io(format!("reopening {}", tmp.display())))?;
    f.sync_all()
        .map_err(io(format!("flushing {}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(io(format!("renaming into {}", path.display())))?;
    Ok(())
}

/// Copy `src` into the first `len` bytes of `dst`, without truncating a device.
fn write_image(dst: &Path, src: &Path) -> Result<u64> {
    let meta = std::fs::metadata(dst);
    let is_regular = meta.map(|m| m.is_file()).unwrap_or(false);
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(is_regular)
        .open(dst)
        .map_err(io(format!("opening {} for writing", dst.display())))?;
    let mut input = std::fs::File::open(src).map_err(io(format!("opening {}", src.display())))?;
    let written =
        std::io::copy(&mut input, &mut out).map_err(io(format!("writing {}", dst.display())))?;
    out.flush()
        .map_err(io(format!("flushing {}", dst.display())))?;
    out.sync_all()
        .map_err(io(format!("syncing {}", dst.display())))?;
    Ok(written)
}

/// Put `payload` into the inactive slot, verify it, and report what was written.
///
/// `claimed`, when given, is the caller's *assertion* about which slot it expects to
/// write. It is checked against the policy and never used in place of it: a caller that
/// has decided wrongly should be told, not obeyed.
///
/// The policy lives here and only here: which slot may be written
/// ([`morph_boot::update_target`]), that the payload is not older than the running system,
/// and that the slot is not mid-update. The mechanical write-and-verify of a specific slot
/// is [`write_slot`], which this calls once the policy has chosen a target. Recovery
/// (`morph-heal recover`) writes **both** slots and so calls [`write_slot`] directly —
/// never this, because the two slots it must write are exactly the two this refuses.
///
/// **This does not trial the slot.** Writing and booting are separate decisions on
/// purpose — `morph-boot trial <slot>` is what makes the machine boot it, and keeping
/// them apart means a verified write is still a verified write if the operator decides
/// not to boot it.
pub fn install(
    bc: &dyn morph_boot::BootControl,
    payload: &mut Payload,
    layout: &Layout,
    claimed: Option<Slot>,
) -> Result<Outcome> {
    let target = morph_boot::update_target(bc).map_err(|e| Error::Policy(e.to_string()))?;
    if let Some(want) = claimed
        && want != target
    {
        return Err(Error::Refused(format!(
            "asked to write slot {want}, but slot {target} is the one that may be updated \
             (slot {want} is either running or the only confirmed system)"
        )));
    }

    // An older release is refused before the slot is touched. The same release is allowed:
    // `morph-heal` rebuilds the spare from the running system, which is that release.
    if payload.release() < layout.running_release {
        return Err(Error::Refused(format!(
            "the payload is release {}, older than the running release {}: an old release \
             may carry a hole a newer one fixed, so it is not put back",
            payload.release(),
            layout.running_release
        )));
    }

    let marker = layout.marker(target);
    if marker.exists() {
        return Err(Error::Incomplete(format!(
            "{} exists: a previous update to slot {target} did not finish, so it must not \
             be trialled. Remove it only after checking that slot by hand",
            marker.display()
        )));
    }

    write_slot(payload, layout, target)
}

/// Write `payload` into `slot` and verify what was written. No policy.
///
/// This is the mechanical half of an update with every policy decision removed: it does
/// not ask which slot may be written, does not compare releases, and does not refuse on a
/// stale incomplete marker. It writes exactly the slot it is told to, verifies it, and
/// clears its own marker.
///
/// The split exists for recovery (`morph-heal recover`), which writes both slots — a thing
/// no boot policy would ever allow, because in the recovery system neither slot is running.
/// It is also the H7 answer: the invariant "never write the running system" stays in
/// [`install`], where a caller reaching straight for the write cannot bypass it.
pub fn write_slot(payload: &mut Payload, layout: &Layout, slot: Slot) -> Result<Outcome> {
    // 1. The payload is sound BEFORE the slot is touched.
    payload.check()?;
    let root_bytes = payload
        .size_of(ROOT_IMAGE)
        .ok_or_else(|| Error::BadPayload("root image size unknown".into()))?;
    let root_want = payload
        .digest_of(ROOT_IMAGE)
        .ok_or_else(|| Error::BadPayload("root image digest unknown".into()))?
        .to_string();

    let boot_dir = layout.boot_dir(slot);
    std::fs::create_dir_all(&boot_dir).map_err(io(format!("creating {}", boot_dir.display())))?;

    // 2. Say out loud that this slot is mid-update. From here on, an interruption is
    //    visible instead of looking like a slot that is ready to trial.
    let marker = layout.marker(slot);
    write_atomic(&marker, b"an update to this slot did not finish\n")?;

    // 3. Write.
    let written = write_image(&layout.root_device, &payload.dir().join(ROOT_IMAGE))?;
    if written != root_bytes {
        return Err(Error::VerifyFailed(format!(
            "wrote {written} bytes of the root image but the payload is {root_bytes}"
        )));
    }
    for name in [KERNEL, INITRD] {
        let bytes = std::fs::read(payload.dir().join(name))
            .map_err(io(format!("reading payload {name}")))?;
        write_atomic(&boot_dir.join(name), &bytes)?;
    }

    // 4. Verify by reading the device, not the buffer.
    let root_got = hash_prefix(&layout.root_device, root_bytes)?;
    if root_got != root_want {
        return Err(Error::VerifyFailed(format!(
            "slot {slot} root re-read hashes to {root_got}, but the payload declared {root_want}"
        )));
    }
    for name in [KERNEL, INITRD] {
        let (got, _) = hash_file(&boot_dir.join(name))?;
        if Some(got.as_str()) != payload.digest_of(name) {
            return Err(Error::VerifyFailed(format!(
                "{name} in slot {slot}'s boot directory does not match the payload"
            )));
        }
    }

    // 5. Only now is the slot allowed to look ready.
    std::fs::remove_file(&marker).map_err(io(format!("removing {}", marker.display())))?;

    Ok(Outcome {
        slot,
        release: payload.release(),
        root_bytes,
        root_digest: root_got,
        kernel_bytes: payload.size_of(KERNEL).unwrap_or(0),
        initrd_bytes: payload.size_of(INITRD).unwrap_or(0),
    })
}
