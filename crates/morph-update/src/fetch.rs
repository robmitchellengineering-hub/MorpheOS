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
use std::sync::Arc;

use crate::{Error, INITRD, KERNEL, MANIFEST, Payload, ROOT_IMAGE, Result, SIGNATURE, TrustedKeys};

/// `GET {base}/stable/{release}/{name}` into `dir/{name}`, through `agent`.
fn download(agent: &ureq::Agent, base: &str, release: u64, name: &str, dir: &Path) -> Result<()> {
    let url = format!("{base}/stable/{release}/{name}");
    let resp = agent
        .get(&url)
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

/// An HTTP(S) client that trusts the bundled Mozilla roots and, when `ca` names a file, the
/// CA certificate in it too.
///
/// The default roots are `webpki-roots`, exactly the set ureq's own default agent uses (its
/// `native-certs` feature is off, so no system store is read and no `ca-certificates` package
/// is needed — doc 20, S3 note 5). `ca` is how the autoupdate **test** image trusts its
/// throwaway CA (doc 20, S3 note 4): the production unit never names one, so a real image
/// trusts only the bundled roots. The rustls backend and its features are the same ones ureq
/// links, so there is still no system library to link against.
fn agent(ca: Option<&Path>) -> Result<ureq::Agent> {
    let mut roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    if let Some(path) = ca {
        let pem = std::fs::read(path).map_err(|e| Error::Io {
            what: format!("reading {}", path.display()),
            source: e,
        })?;
        let mut reader = std::io::BufReader::new(pem.as_slice());
        for cert in rustls_pemfile::certs(&mut reader) {
            let cert = cert.map_err(|e| Error::Io {
                what: format!("parsing {}", path.display()),
                source: e,
            })?;
            roots.add(cert).map_err(|e| {
                Error::Untrusted(format!(
                    "{} is not a usable CA certificate: {e}",
                    path.display()
                ))
            })?;
        }
    }
    let config = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
    .expect("the ring provider always configures TLS 1.2 and TLS 1.3")
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(ureq::builder().tls_config(Arc::new(config)).build())
}

/// The release the server advertises as its newest (`{base}/stable/LATEST`), a hint trusted
/// for nothing: the signed manifest is re-read and compared before anything is installed, so
/// a server that advertises one release and serves another is refused rather than installed.
pub fn latest(base: &str, ca: Option<&Path>) -> Result<u64> {
    let url = format!("{base}/stable/LATEST");
    let body = agent(ca)?
        .get(&url)
        .call()
        .map_err(|e| Error::Fetch(format!("GET {url}: {e}")))?
        .into_string()
        .map_err(|e| Error::Fetch(format!("GET {url}: {e}")))?;
    body.trim()
        .parse()
        .map_err(|_| Error::Fetch(format!("{url} says '{body}', not a release number")))
}

/// Fetch release `release` from `base` into `dir`, verify it, and return it.
///
/// `release` is the number the server advertised (its `stable/LATEST`, a hint trusted for
/// nothing): the signed manifest is re-read and its release compared, so a server that
/// serves a different release under the advertised name is refused, not installed.
pub fn fetch(base: &str, release: u64, dir: &Path, trusted: &TrustedKeys) -> Result<Payload> {
    fetch_with_ca(base, release, dir, trusted, None)
}

/// [`fetch`] with an extra CA certificate to trust (`ca`), which the autoupdate test image
/// passes so the throwaway CA that signed its server certificate is accepted (doc 20, S3
/// note 4). A production caller passes `None` and gets the bundled roots alone.
pub fn fetch_with_ca(
    base: &str,
    release: u64,
    dir: &Path,
    trusted: &TrustedKeys,
    ca: Option<&Path>,
) -> Result<Payload> {
    std::fs::create_dir_all(dir).map_err(|e| Error::Io {
        what: format!("creating {}", dir.display()),
        source: e,
    })?;
    let agent = agent(ca)?;
    download(&agent, base, release, MANIFEST, dir)?;
    download(&agent, base, release, SIGNATURE, dir)?;
    let mut payload = Payload::open(dir, trusted)?;
    if payload.release() != release {
        return Err(Error::Fetch(format!(
            "the server advertised release {release}, but the signed manifest says {}",
            payload.release()
        )));
    }
    for name in [ROOT_IMAGE, KERNEL, INITRD] {
        download(&agent, base, release, name, dir)?;
    }
    // Hash every file against the manifest; a truncated or tampered download is refused here.
    payload.check()?;
    Ok(payload)
}
