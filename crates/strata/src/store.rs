//! Content-addressed object store.
//!
//! Layout under the store root:
//!
//! ```text
//! chunks/ab/cdef…   zstd-compressed file pieces, named by BLAKE3 of the plain bytes
//! files/ab/cdef…    zstd-compressed JSON list of chunk hashes, named by BLAKE3 of that list
//! ```
//!
//! A file's content is identified by its `files/` hash. Chunks are cut by
//! FastCDC, so an edit in the middle of a large file changes only the chunks
//! around the edit, and every other chunk is shared with earlier versions.
//! Every read re-hashes the data: a damaged store is reported, never trusted.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::{Error, Result, io};

pub const MIN_CHUNK: usize = 16 * 1024;
pub const AVG_CHUNK: usize = 64 * 1024;
pub const MAX_CHUNK: usize = 256 * 1024;

/// How much of the start of a file is sampled for its entropy.
pub const ENTROPY_SAMPLE: usize = 64 * 1024;

pub struct ObjectStore {
    root: PathBuf,
}

/// What was learned while storing a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    /// Hash naming the file's chunk list.
    pub content: String,
    pub size: u64,
    /// Shannon entropy of the first [`ENTROPY_SAMPLE`] bytes, in bits per byte (0–8).
    pub entropy: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Chunk,
    File,
}

impl Kind {
    fn dir(self) -> &'static str {
        match self {
            Kind::Chunk => "chunks",
            Kind::File => "files",
        }
    }
}

pub fn hash_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Shannon entropy in bits per byte. Plain text sits around 4–5, compressed or
/// encrypted data close to 8.
pub fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for b in data {
        counts[*b as usize] += 1;
    }
    let n = data.len() as f64;
    counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = *c as f64 / n;
            -p * p.log2()
        })
        .sum()
}

impl ObjectStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for kind in [Kind::Chunk, Kind::File] {
            fs::create_dir_all(root.join(kind.dir()))
                .map_err(io(format!("creating {}", root.display())))?;
        }
        Ok(ObjectStore { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, kind: Kind, hash: &str) -> PathBuf {
        self.root.join(kind.dir()).join(&hash[..2]).join(&hash[2..])
    }

    fn put(&self, kind: Kind, data: &[u8]) -> Result<String> {
        let hash = hash_hex(data);
        let path = self.path(kind, &hash);
        if path.exists() {
            return Ok(hash);
        }
        let dir = path.parent().expect("object path has a parent");
        fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
        let packed = zstd::encode_all(data, 3).map_err(io("compressing"))?;
        // Write beside, then rename: a crash leaves either nothing or the whole object.
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        let mut f = fs::File::create(&tmp).map_err(io(format!("writing {}", tmp.display())))?;
        f.write_all(&packed)
            .and_then(|_| f.sync_all())
            .map_err(io(format!("writing {}", tmp.display())))?;
        fs::rename(&tmp, &path).map_err(io(format!("storing {}", path.display())))?;
        Ok(hash)
    }

    fn get(&self, kind: Kind, hash: &str) -> Result<Vec<u8>> {
        if hash.len() < 3 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Corrupt(format!("'{hash}' is not an object hash")));
        }
        let path = self.path(kind, hash);
        let packed = fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                Error::NotFound(format!("{} object {hash}", kind.dir()))
            }
            _ => Error::Io {
                what: format!("reading {}", path.display()),
                source: e,
            },
        })?;
        let data = zstd::decode_all(packed.as_slice())
            .map_err(|e| Error::Corrupt(format!("{}: {e}", path.display())))?;
        if hash_hex(&data) != hash {
            return Err(Error::Corrupt(format!(
                "{} does not match its hash",
                path.display()
            )));
        }
        Ok(data)
    }

    /// Store everything `reader` yields as one file version.
    pub fn put_file(&self, reader: impl Read) -> Result<Stored> {
        self.file(reader, true)
    }

    /// The content hash `put_file` would give, without writing anything.
    pub fn hash_file(&self, reader: impl Read) -> Result<Stored> {
        self.file(reader, false)
    }

    fn file(&self, reader: impl Read, write: bool) -> Result<Stored> {
        let mut chunks = Vec::new();
        let mut size = 0u64;
        let mut sample = Vec::new();
        let cdc = fastcdc::v2020::StreamCDC::new(reader, MIN_CHUNK, AVG_CHUNK, MAX_CHUNK);
        for chunk in cdc {
            let chunk = chunk.map_err(|e| Error::Io {
                what: "reading file".into(),
                source: std::io::Error::other(e.to_string()),
            })?;
            if sample.len() < ENTROPY_SAMPLE {
                let take = (ENTROPY_SAMPLE - sample.len()).min(chunk.data.len());
                sample.extend_from_slice(&chunk.data[..take]);
            }
            size += chunk.data.len() as u64;
            chunks.push(if write {
                self.put(Kind::Chunk, &chunk.data)?
            } else {
                hash_hex(&chunk.data)
            });
        }
        let list = serde_json::to_vec(&chunks).expect("a list of strings serializes");
        Ok(Stored {
            content: if write {
                self.put(Kind::File, &list)?
            } else {
                hash_hex(&list)
            },
            size,
            entropy: entropy(&sample),
        })
    }

    /// The chunk hashes making up a stored file.
    pub fn chunks_of(&self, content: &str) -> Result<Vec<String>> {
        let list = self.get(Kind::File, content)?;
        serde_json::from_slice(&list)
            .map_err(|e| Error::Corrupt(format!("file object {content}: {e}")))
    }

    /// Write a stored file's bytes to `out`, verifying every chunk on the way.
    pub fn read_file(&self, content: &str, out: &mut impl Write) -> Result<u64> {
        let mut n = 0u64;
        for chunk in self.chunks_of(content)? {
            let data = self.get(Kind::Chunk, &chunk)?;
            out.write_all(&data).map_err(io("writing restored data"))?;
            n += data.len() as u64;
        }
        Ok(n)
    }

    /// Delete every object not in `keep_files` (and their chunks), skipping
    /// anything written after `grace_before_ns` so a snapshot running right now
    /// cannot lose chunks it has stored but not yet recorded.
    pub fn collect_garbage(
        &self,
        keep_files: &std::collections::HashSet<String>,
        grace_before_ns: i64,
    ) -> Result<GcReport> {
        let mut keep_chunks = std::collections::HashSet::new();
        for f in keep_files {
            keep_chunks.extend(self.chunks_of(f)?);
        }
        let mut report = GcReport::default();
        for (kind, keep) in [(Kind::File, keep_files), (Kind::Chunk, &keep_chunks)] {
            for entry in walk_objects(&self.root.join(kind.dir()))? {
                let (hash, path, mtime_ns, len) = entry;
                if keep.contains(&hash) || mtime_ns >= grace_before_ns {
                    continue;
                }
                fs::remove_file(&path).map_err(io(format!("removing {}", path.display())))?;
                report.objects_removed += 1;
                report.bytes_freed += len;
            }
        }
        Ok(report)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GcReport {
    pub objects_removed: u64,
    pub bytes_freed: u64,
}

/// Every object under `dir`: (hash, path, mtime in ns, size on disk).
fn walk_objects(dir: &Path) -> Result<Vec<(String, PathBuf, i64, u64)>> {
    let mut out = Vec::new();
    let rd = |p: &Path| fs::read_dir(p).map_err(io(format!("listing {}", p.display())));
    for shard in rd(dir)? {
        let shard = shard.map_err(io("listing store"))?;
        let prefix = shard.file_name().to_string_lossy().into_owned();
        if prefix.len() != 2 || !shard.path().is_dir() {
            continue;
        }
        for obj in rd(&shard.path())? {
            let obj = obj.map_err(io("listing store"))?;
            let rest = obj.file_name().to_string_lossy().into_owned();
            if rest.contains('.') {
                continue; // an interrupted write's temp file; never a live object
            }
            let meta = obj.metadata().map_err(io("reading store metadata"))?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(i64::MAX);
            out.push((format!("{prefix}{rest}"), obj.path(), mtime, meta.len()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("strata-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn pseudo_random(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    #[test]
    fn roundtrip_and_verify() {
        let s = ObjectStore::open(tmp("rt")).unwrap();
        let data = pseudo_random(700_000, 1);
        let stored = s.put_file(data.as_slice()).unwrap();
        assert_eq!(stored.size, data.len() as u64);
        let mut out = Vec::new();
        s.read_file(&stored.content, &mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn hashing_writes_nothing_and_agrees_with_storing() {
        let root = tmp("hash");
        let s = ObjectStore::open(&root).unwrap();
        let data = pseudo_random(200_000, 5);
        let hashed = s.hash_file(data.as_slice()).unwrap();
        assert!(walk_objects(&root.join("chunks")).unwrap().is_empty());
        assert!(walk_objects(&root.join("files")).unwrap().is_empty());
        assert_eq!(s.put_file(data.as_slice()).unwrap(), hashed);
    }

    #[test]
    fn empty_file() {
        let s = ObjectStore::open(tmp("empty")).unwrap();
        let stored = s.put_file(&b""[..]).unwrap();
        let mut out = Vec::new();
        assert_eq!(s.read_file(&stored.content, &mut out).unwrap(), 0);
    }

    #[test]
    fn a_small_edit_reuses_most_chunks() {
        let s = ObjectStore::open(tmp("dedup")).unwrap();
        let mut data = pseudo_random(2_000_000, 7);
        let v1 = s.put_file(data.as_slice()).unwrap();
        data[1_000_000] ^= 0xff;
        let v2 = s.put_file(data.as_slice()).unwrap();
        let (a, b) = (
            s.chunks_of(&v1.content).unwrap(),
            s.chunks_of(&v2.content).unwrap(),
        );
        let shared = b.iter().filter(|c| a.contains(c)).count();
        assert!(
            shared + 3 >= b.len(),
            "only {shared} of {} chunks reused",
            b.len()
        );
    }

    #[test]
    fn damage_is_detected() {
        let root = tmp("damage");
        let s = ObjectStore::open(&root).unwrap();
        let stored = s.put_file(&b"important"[..]).unwrap();
        let chunk = &s.chunks_of(&stored.content).unwrap()[0];
        let path = s.path(Kind::Chunk, chunk);
        fs::write(&path, zstd::encode_all(&b"tampered"[..], 3).unwrap()).unwrap();
        let err = s.read_file(&stored.content, &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)), "{err}");
    }

    #[test]
    fn entropy_separates_text_from_ciphertext() {
        let text = b"The quick brown fox jumps over the lazy dog. ".repeat(500);
        assert!(entropy(&text) < 5.0);
        assert!(entropy(&pseudo_random(65_536, 3)) > 7.9);
        assert_eq!(entropy(&[]), 0.0);
    }

    #[test]
    fn garbage_collection_keeps_what_is_referenced() {
        let s = ObjectStore::open(tmp("gc")).unwrap();
        let keep = s.put_file(pseudo_random(300_000, 11).as_slice()).unwrap();
        let drop = s.put_file(pseudo_random(300_000, 12).as_slice()).unwrap();
        let keep_set = [keep.content.clone()].into_iter().collect();
        // Grace in the past: nothing is protected by age.
        let r = s.collect_garbage(&keep_set, i64::MAX).unwrap();
        assert!(r.objects_removed > 0);
        assert!(s.read_file(&keep.content, &mut Vec::new()).is_ok());
        assert!(matches!(
            s.chunks_of(&drop.content),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn garbage_collection_spares_recent_objects() {
        let s = ObjectStore::open(tmp("gc-grace")).unwrap();
        let fresh = s.put_file(pseudo_random(100_000, 13).as_slice()).unwrap();
        let r = s.collect_garbage(&Default::default(), 0).unwrap();
        assert_eq!(r.objects_removed, 0);
        assert!(s.read_file(&fresh.content, &mut Vec::new()).is_ok());
    }
}
