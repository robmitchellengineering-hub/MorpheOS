//! A minimal PNG writer: no dependencies, and no image tool needed on the machine that
//! wants to look at the frame.
//!
//! **Why this exists rather than an image crate or ImageMagick.** The shell's frame has to
//! be *looked at* ([ADR 0011]), and the machines it gets looked at on are a CI runner, a
//! laptop and a VM — none of which can be assumed to have an image library. The first
//! version piped a NetPBM file through `convert`, and the runner answered
//! **`convert: command not found`**; a PPM is not viewable in a browser either. So the
//! frame is encoded here instead, and every machine that can run the binary can show it.
//!
//! **The deflate stream uses STORED blocks**, so the file is larger than a compressor would
//! make it and no compressor is needed. That is the right trade for a development frame:
//! correctness with no dependency at all, and a few megabytes beside the 459 MB image build
//! this repository already uploads.
//!
//! [ADR 0011]: ../../docs/adr/0011-the-humans-mark.md

use crate::{Colour, Frame};

/// The eight bytes every PNG starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The largest payload a single stored deflate block can carry.
const MAX_STORED: usize = 65_535;

impl Frame {
    /// The frame as a PNG: 8-bit truecolour, no interlacing, stored deflate blocks.
    ///
    /// Any decoder can read this — the tests check the header and the trailer, and the
    /// encoder is proved by a real decoder accepting its output rather than by the tests
    /// alone.
    #[must_use]
    pub fn to_png(&self) -> Vec<u8> {
        let mut raw =
            Vec::with_capacity(self.pixels().len().saturating_mul(3) + self.height() as usize);
        for y in 0..self.height() {
            raw.push(0); // filter: None. The frame is flat colour, so filtering buys nothing.
            for x in 0..self.width() {
                let (_, r, g, b) = self.pixel(x, y).unwrap_or(Colour::rgb(0, 0, 0)).channels();
                raw.extend_from_slice(&[r, g, b]);
            }
        }

        let mut out = Vec::with_capacity(raw.len() + raw.len() / MAX_STORED * 5 + 128);
        out.extend_from_slice(&SIGNATURE);

        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width().to_be_bytes());
        ihdr.extend_from_slice(&self.height().to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(2); // colour type: truecolour RGB
        ihdr.push(0); // compression: deflate
        ihdr.push(0); // filter method: adaptive
        ihdr.push(0); // interlace: none
        write_chunk(&mut out, b"IHDR", &ihdr);
        write_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
        write_chunk(&mut out, b"IEND", &[]);
        out
    }
}

/// One PNG chunk: length, type, data, and the CRC over type and data.
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

/// A zlib stream whose deflate payload is stored rather than compressed.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let blocks = raw.len().div_ceil(MAX_STORED).max(1);
    let mut out = Vec::with_capacity(raw.len() + blocks * 5 + 6);
    out.push(0x78); // CMF: deflate, 32K window
    out.push(0x01); // FLG: no preset dictionary, fastest
    if raw.is_empty() {
        // An empty stream still needs one final stored block.
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    } else {
        let chunks: Vec<&[u8]> = raw.chunks(MAX_STORED).collect();
        let last_index = chunks.len() - 1;
        for (index, chunk) in chunks.iter().enumerate() {
            out.push(u8::from(index == last_index)); // BFINAL, BTYPE=00 (stored)
            let len = chunk.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(chunk);
        }
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// The zlib checksum.
fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in bytes {
        a = (a + u32::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

/// The PNG checksum: CRC-32/IEEE, reflected, which is what the format specifies.
struct Crc32 {
    value: u32,
}

impl Crc32 {
    const fn new() -> Self {
        Self { value: 0xffff_ffff }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.value ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (self.value & 1).wrapping_neg();
                self.value = (self.value >> 1) ^ (0xedb8_8320 & mask);
            }
        }
    }

    const fn finish(self) -> u32 {
        !self.value
    }
}
