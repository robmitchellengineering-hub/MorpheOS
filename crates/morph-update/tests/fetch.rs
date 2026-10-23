//! S3 step 3.3: `fetch` downloads a signed payload from a server and verifies it.
//!
//! The server here is a plain-HTTP one-shot listener that maps `/stable/<release>/<name>` to
//! a file in a directory — enough to prove the fetch order (manifest + signature first, then
//! the large files) and the checks, without pretending to be the CI host's HTTPS setup.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use morph_update::{TrustedKeys, fetch};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("morph-fetch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Serve the files in `root` over plain HTTP on an ephemeral port, one thread per request,
/// until the listener is dropped. Returns the base URL.
fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let root = Arc::new(root);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = Arc::clone(&root);
            thread::spawn(move || respond(stream, &root));
        }
    });
    format!("http://{addr}")
}

fn respond(mut stream: std::net::TcpStream, root: &Path) {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let request = String::from_utf8_lossy(&buf[..n]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let name = path.rsplit('/').next().unwrap_or("");
    let file = root.join(name);
    let (status, body) = match std::fs::read(&file) {
        Ok(bytes) => ("200 OK", bytes),
        Err(_) => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// A signed payload, served over HTTP, is fetched and verified end to end — the same shape as
/// `a_key_made_by_keygen_signs_a_payload_its_public_half_accepts`, but through the network.
#[test]
fn fetch_downloads_and_verifies_a_payload() {
    let (secret, public) = morph_update::keygen().unwrap();
    let keys = scratch("keys");
    std::fs::write(keys.join("release.pub"), format!("{public}\n")).unwrap();
    let trusted = TrustedKeys::load(&keys).unwrap();

    let payload = scratch("payload");
    for (name, bytes) in [
        ("root.img", b"root image bytes".as_slice()),
        ("vmlinuz", b"k"),
        ("initrd.img", b"i"),
    ] {
        std::fs::write(payload.join(name), bytes).unwrap();
    }
    let release = 2;
    morph_update::seal(&payload, release).unwrap();
    morph_update::sign(&payload, &secret).unwrap();

    let base_url = serve(payload);
    let dest = scratch("dest");
    let got = fetch(&base_url, release, &dest, &trusted).unwrap();

    assert_eq!(got.release(), release);
    for name in [
        "BLAKE3SUMS",
        "BLAKE3SUMS.sig",
        "root.img",
        "vmlinuz",
        "initrd.img",
    ] {
        assert!(dest.join(name).is_file(), "{name} was not fetched");
    }
}

/// A server advertising release 2 that actually serves release 3's signed manifest is refused:
/// the server's LATEST is a hint trusted for nothing; the signed manifest is the truth.
#[test]
fn server_misadvertising_a_release_is_refused() {
    let (secret, public) = morph_update::keygen().unwrap();
    let keys = scratch("keys");
    std::fs::write(keys.join("release.pub"), format!("{public}\n")).unwrap();
    let trusted = TrustedKeys::load(&keys).unwrap();

    let payload = scratch("payload");
    for (name, bytes) in [
        ("root.img", b"root image bytes".as_slice()),
        ("vmlinuz", b"k"),
        ("initrd.img", b"i"),
    ] {
        std::fs::write(payload.join(name), bytes).unwrap();
    }
    morph_update::seal(&payload, 3).unwrap();
    morph_update::sign(&payload, &secret).unwrap();

    let base_url = serve(payload);
    let dest = scratch("dest");
    let err = fetch(&base_url, 2, &dest, &trusted).unwrap_err();
    assert!(
        err.to_string().contains("advertised release 2"),
        "unexpected error: {err}"
    );
}
