# 04 · Strata: Rethinking the Filesystem

## Why replace the hierarchical filesystem?

The path `/home/me/Documents/Work/2025/Clients/Acme/contract_final_v3_REAL.docx`
combines four separate things into one string: **identity**, **location**,
**organisation** and **version**. Each of these causes everyday pain:

- If you move a file, links break.
- If you overwrite a file, the old version is gone.
- If you can't remember the folder, you can't find the file.
- Two copies of the same file use twice the space and drift apart.

Strata separates these concerns.

## Model

```
            ┌──────────── Views ────────────┐
            │ POSIX paths · Smart folders ·  │
            │ Timeline · People · Projects   │
            └───────────────┬───────────────┘
                            │ (queries)
┌──────────── Metadata Graph & Index ────────────┐
│ objects ── tags ── projects ── people ── apps  │
│ provenance (came-from, derived-from)           │
│ semantic embeddings · full-text · EXIF/ID3     │
└───────────────┬────────────────────────────────┘
                │ refers to
┌──────────── Object Store ──────────────────────┐
│ content-addressed chunks (BLAKE3) · CoW        │
│ dedup · zstd/lz4 · versions · snapshots        │
└───────────────┬────────────────────────────────┘
                │ placed on
┌──────────── Tier Manager ──────────────────────┐
│ RAM cache → NVMe → SSD/HDD → mesh peer → cloud │
└────────────────────────────────────────────────┘
```

### 1. Object store
- **Content-defined chunking** (FastCDC) plus **BLAKE3** hashing gives automatic
  deduplication across files, versions, capsules and even devices.
- **Copy-on-write everywhere.** Each write creates a new version, and old chunks are
  kept until retention policy frees them.
- **Compression** happens per chunk, with the codec chosen adaptively: lz4 for hot data, zstd for cold, and
  none for data that's already compressed (media).
- Writes are **log-structured and flash-aware**, which matters most on SD cards (Pi) and helps
  NVMe endurance.
- **Integrity** comes for free from the hashes: data is self-healing when there's a redundant copy
  (a mirror, or a mesh peer).

### 2. Metadata graph
- Every object has a stable **ObjectID** that doesn't depend on its name or location.
- Typed relationships include `in-project`, `from-person`, `attached-to`,
  `derived-from` (export.pdf ← report.docx), `opened-with` and `downloaded-from`.
- Provenance is recorded automatically: Strata knows that file came from that email from
  that person on that day.
- It is stored in an embedded, transactional store (SQLite/LMDB-class) with
  graph queries on top.

### 3. Index
- Full-text indexing and file-format extractors.
- **Local semantic embeddings**, run in the background when idle on capable hardware, or
  offloaded to a mesh peer from a Pi.
- Query latency target: < 50 ms for structured queries, < 300 ms for semantic ones.

### 4. Views
- **POSIX view:** a normal path tree for legacy apps and tools. Paths are just
  names in a view, and many paths can point at one object.
- **Smart folders:** saved queries, such as `project:acme type:contract after:2025-03`.
- **Timeline:** the whole system seen as of any point in time.
- **Session view:** only the files relevant to the current task.

## Time travel

```bash
morph strata at "yesterday 5pm" ~/report.docx     # open old version
morph strata diff report.docx@-2h report.docx      # what changed
morph strata rewind session:"Q3 report" --to 14:00 # rewind a whole task
```

Retention follows a simple default that's easy to understand, for example
*every version for 24 h, hourly for 30 days, daily for 1 year*, all tuned
automatically to the available space.

## Compatibility with existing files and filesystems

- **Read and write** existing filesystems: ext4, NTFS, exFAT, FAT32, APFS (read-only at first), btrfs and
  XFS, through kernel drivers or FUSE. External drives "just work".
- **Import** without copying: existing volumes can be indexed in place.
- **Export:** any view can be materialised as a plain directory or archive. No lock-in.
- Implementation path:
  - **v0:** a Rust FUSE layer over btrfs or bcachefs, using their CoW and snapshots, plus the
    metadata and index services. This ships early and proves the UX.
  - **v1:** a native Strata block layer once the model is proven, with FUSE replaced
    by a kernel module or passthrough for performance.

## Performance notes

- Hot-path reads go through the kernel page cache, with no added indirection after open.
- Dedup lookups use an in-memory Bloom filter plus an on-disk index. Pi Zero
  profiles turn off inline dedup and dedup during idle instead.
- Large media files are stored with large chunks and, when possible, as a single
  extent so that streaming read throughput is as good as ext4.

## Open questions
- Should the native v1 be a new on-disk format, or a bcachefs fork or extension?
- How should version retention interact with encrypted and secure-delete data?
- Semantic index privacy: per-user keys, and how it's excluded from mesh sync.
