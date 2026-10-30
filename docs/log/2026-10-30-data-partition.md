# 2026-10-30 · DeepSeek · the stick's data partition was too small to work in

Measured on the first real hardware node: `/persist` is the data partition, the root is read-only, and
that partition was **256 MiB** (268 MB as flashed). A Rust toolchain is ~1.5 GB and a work tree is not
small either, so "build MorpheOS from inside the machine" was impossible before it started.

## What changes

`build.sh` sizes the data partition at **4096 MiB for the person-facing images** (`HARDWARE=1`) only.
The self-test scenarios keep their small disks, so CI is unaffected; `MORPH_DATA_MB` overrides it for a
particular build.

The image grows by ~3.8 GB, but the added space is an **empty filesystem**: it is sparse on disk and adds
almost nothing to the compressed artefact. The Porsche stick has 15.8 GB, so the result still fits with
room to spare.

## Why this is safe

The test drivers do not hardcode partition offsets — `test-full-disk.sh` and `test-config-rollback.sh`
read `start=` and `size=` out of the image's own partition table (`sfdisk -d`) and pass them to `dd`.
The GPT is written from `sfdisk` in `build.sh`, so both the table and the tests move together.

## Not verified

That the resulting image still fits a real 8 GB stick — it does not, and that is intended: the
person-facing image is now sized for a 16 GB stick, and the README's stick guidance should say so.
