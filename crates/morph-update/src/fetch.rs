//! Download a signed payload over HTTP(S) and verify it before anything is installed
//! (doc 20, S3 step 3.3).
//!
//! The order mirrors the "check first, download later" rule from S2: the manifest and its
//! signature are fetched first, [`Payload::open`] checks them (and therefore the release)
//! without hashing the large files, and only then are `root.img`, `vmlinuz` and `initrd.img`
//! downloaded and hashed by [`Payload::check`]. An old or untrusted release costs two small
//! files, never a multi-hundred-megabyte download.

use std::io::Read;
use std::path::Path;

use crate::{Error, Payload, Result, TrustedKeys, INITRD, KERNEL, MANIFEST, ROOT_IMAGE, SIGNATURE};

/// `GET {base}/stable/{release}/{name}` into `dir/{name}`.
fn download(base: &str, release: u64, name: &str, dir: &Path) -> Result<()> {
    let url = format!("{base}/stable/{release}/{name}");
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| Error::Fetch(format!("GET {url}: {e}")))?;
    let mut body = Vec::new();
    resp.into_reader()
        .read_to_end(&mut body)
        .map_err(|e| Error::Fetch(format!("GET {url}: {e}")))?;
    std::fs::write(dir.join(name), body).map_err(|e| Error::Io {
        what: format!("writing {}", dir.join(name).display()),
        source: e,
    })?;
    Ok(())
}

/// Fetch release `release` from `base` into `dir`, verify it, and return it.
///
/// `release` is the number the server advertised (its `stable/LATEST`, a hint trusted for
/// nothing): the signed manifest is re-read and its release compared, so a server that
/// serves a different release under the advertised name is refused, not installed.
pub fn fetch(base: &str, release: u64, dir: &Path, trusted: &TrustedKeys) -> Result<Payload> {
    std::fs::create_dir_all(dir).map_err(|e| Error::Io {
        what: format!("creating {}", dir.display()),
        source: e,
    })?;
    download(base, release, MANIFEST, dir)?;
    download(base, release, SIGNATURE, dir)?;
    let mut payload = Payload::open(dir, trusted)?;
    if payload.release() != release {
        return Err(Error::Fetch(format!(
            "the server advertised release {release}, but the signed manifest says {}",
            payload.release()
        )));
    }
    for name in [ROOT_IMAGE, KERNEL, INITRD] {
        download(base, release, name, dir)?;
    }
    // Hash every file against the manifest; a truncated or tampered download is refused here.
    payload.check()?;
    Ok(payload)
}
