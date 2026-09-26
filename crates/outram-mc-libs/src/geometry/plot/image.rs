// SPDX-License-Identifier: GPL-3.0
//
// Ported from OpenMC (MIT), commit d7d3284a1: src/plot.cpp output_ppm
// (857-881) and output_png (887-935); `ImageData` is plot.h:97.
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT notice in
// verification_and_validation/geometry_plotting/openmc_inputs/LICENSE.openmc.

//! **Image buffer and the PNG / PPM codecs.**
//!
//! # Why this writes its own PNG container
//!
//! Upstream calls libpng. A PNG is a signature, three chunk types and one
//! zlib stream, and the only non-trivial part — DEFLATE — is already in this
//! crate's dependency tree: `miniz_oxide` (pure Rust, MIT/Zlib/Apache-2.0) is a
//! non-optional dependency of `njoy-outram-park-fork`, which this crate depends
//! on, so naming it directly adds **no new crate to the build**, and it builds
//! for Android/Termux and `wasm32-unknown-unknown` already (it is the WMPB
//! codec there). The rest — chunk framing, CRC-32, row filters — is ~150 lines
//! below. The `image` crate was considered and rejected: it is a far larger
//! tree, and in this workspace it is only ever a dev/GUI dependency.
//!
//! # Why no JPEG
//!
//! JPEG is lossy. A geometry plot is a *classification* — every pixel says
//! which cell or material is there — and JPEG's block DCT blurs exactly the
//! boundaries a plot exists to show, and makes a pixel-for-pixel comparison
//! against OpenMC meaningless. OpenMC itself writes only PNG or PPM
//! (`PlottableInterface::write_image`, `src/plot.cpp:193-200`). PNG is also
//! smaller than JPEG for flat-colour images like these.
//!
//! The decoder ([`decode_png`]) exists so the parity tests can read OpenMC's
//! own PNGs; it handles what libpng writes for an 8-bit RGB/RGBA
//! non-interlaced image — all five row filters — and refuses anything else.

use std::io;
use std::path::Path;

use super::colour::Rgb;

/// A rectangular RGB image, row-major with row 0 at the **top** — the order
/// both PNG and PPM store rows, and the order upstream's `data(x, y)` writes
/// them (`y` = 0 is the first row written by `output_png`).
///
/// Maps to `ImageData` = `tensor::Tensor<RGBColor>` of shape `{width, height}`
/// (`include/openmc/plot.h:97`), indexed `data(x, y)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// `width * height` pixels, `pixels[y * width + x]`.
    pub pixels: Vec<Rgb>,
}

impl ImageData {
    /// An image filled with one colour — upstream constructs every image
    /// this way, filled with the background (`ImageData data({w, h}, not_found_)`).
    #[must_use]
    pub fn filled(width: usize, height: usize, colour: Rgb) -> Self {
        Self {
            width,
            height,
            pixels: vec![colour; width * height],
        }
    }

    /// Pixel at column `x`, row `y` (row 0 at the top).
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.pixels[y * self.width + x]
    }

    /// Set the pixel at column `x`, row `y`.
    pub fn set(&mut self, x: usize, y: usize, c: Rgb) {
        self.pixels[y * self.width + x] = c;
    }

    /// Encode as PNG: 8-bit RGB, non-interlaced — the same `IHDR` upstream's
    /// `output_png` asks libpng for (`src/plot.cpp:909-910`).
    #[must_use]
    pub fn to_png_bytes(&self) -> Vec<u8> {
        encode_png(self)
    }

    /// Write a PNG file. Port of `output_png` (`src/plot.cpp:887-935`).
    ///
    /// # Errors
    /// Any I/O error from creating or writing the file.
    pub fn write_png(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, self.to_png_bytes())
    }

    /// Encode as binary PPM (`P6`), byte-for-byte what `output_ppm` writes
    /// (`src/plot.cpp:857-881`), including its trailing newline.
    #[must_use]
    pub fn to_ppm_bytes(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.reserve(self.pixels.len() * 3 + 1);
        for c in &self.pixels {
            out.extend_from_slice(&[c.r, c.g, c.b]);
        }
        out.push(b'\n');
        out
    }

    /// Write a PPM file. Port of `output_ppm`.
    ///
    /// # Errors
    /// Any I/O error from creating or writing the file.
    pub fn write_ppm(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, self.to_ppm_bytes())
    }

    /// Count of pixels that differ between two images of the same size;
    /// `None` if the sizes differ.
    #[must_use]
    pub fn count_differences(&self, other: &Self) -> Option<usize> {
        if self.width != other.width || self.height != other.height {
            return None;
        }
        Some(
            self.pixels
                .iter()
                .zip(&other.pixels)
                .filter(|(a, b)| a != b)
                .count(),
        )
    }
}

// ─── CRC-32 (ISO 3309 / PNG spec §5.5), table-driven ────────────────────────

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}

static CRC_TABLE: [u32; 256] = crc_table();

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for bytes in chunks {
        for &b in *bytes {
            c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Paeth predictor, PNG spec §9.4.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let pa = (p - i16::from(a)).abs();
    let pb = (p - i16::from(b)).abs();
    let pc = (p - i16::from(c)).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Apply filter `ft` to `row` (with `prev` the unfiltered row above, zeros for
/// the first row) into `out`. `bpp` = 3 bytes per pixel.
fn filter_row(ft: u8, row: &[u8], prev: &[u8], bpp: usize, out: &mut Vec<u8>) {
    for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let b = prev[i];
        let c = if i >= bpp { prev[i - bpp] } else { 0 };
        let x = row[i];
        out.push(match ft {
            0 => x,
            1 => x.wrapping_sub(a),
            2 => x.wrapping_sub(b),
            3 => x.wrapping_sub(((u16::from(a) + u16::from(b)) / 2) as u8),
            _ => x.wrapping_sub(paeth(a, b, c)),
        });
    }
}

fn encode_png(img: &ImageData) -> Vec<u8> {
    let (w, h) = (img.width, img.height);
    let stride = w * 3;
    // Filter each row with whichever of the five filters minimises the sum of
    // |byte| (as signed) — the heuristic PNG spec §12.8 recommends and libpng
    // uses. It changes the file size only; the decoded pixels are identical.
    let mut raw = Vec::with_capacity(h * (stride + 1));
    let mut prev = vec![0u8; stride];
    let mut row = Vec::with_capacity(stride);
    let mut trial = Vec::with_capacity(stride);
    let mut best = Vec::with_capacity(stride);
    for y in 0..h {
        row.clear();
        for c in &img.pixels[y * w..(y + 1) * w] {
            row.extend_from_slice(&[c.r, c.g, c.b]);
        }
        let mut best_ft = 0u8;
        let mut best_score = u64::MAX;
        for ft in 0..5u8 {
            trial.clear();
            filter_row(ft, &row, &prev, 3, &mut trial);
            let score: u64 = trial
                .iter()
                .map(|&v| u64::from((v as i8).unsigned_abs()))
                .sum();
            if score < best_score {
                best_score = score;
                best_ft = ft;
                std::mem::swap(&mut best, &mut trial);
            }
        }
        raw.push(best_ft);
        raw.extend_from_slice(&best);
        std::mem::swap(&mut prev, &mut row);
    }

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    // bit depth 8, colour type 2 (RGB), deflate, adaptive filtering, no interlace
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);

    let mut out = Vec::with_capacity(idat.len() + 64);
    out.extend_from_slice(&PNG_SIGNATURE);
    push_chunk(&mut out, b"IHDR", &ihdr);
    push_chunk(&mut out, b"IDAT", &idat);
    push_chunk(&mut out, b"IEND", &[]);
    out
}

/// Why a PNG could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PngDecodeError {
    /// Not a PNG, or truncated.
    Malformed(&'static str),
    /// A chunk's CRC did not match.
    BadCrc,
    /// A valid PNG this minimal decoder does not handle (palette, 16-bit,
    /// greyscale, interlaced).
    Unsupported(&'static str),
    /// The zlib stream did not inflate.
    Inflate,
}

/// Decode an 8-bit RGB or RGBA non-interlaced PNG (alpha is dropped) — the
/// format libpng writes for OpenMC's plots and [`ImageData::to_png_bytes`]
/// writes here. All five row filters are supported and every chunk CRC is
/// checked.
///
/// # Errors
/// See [`PngDecodeError`].
pub fn decode_png(bytes: &[u8]) -> Result<ImageData, PngDecodeError> {
    if bytes.len() < 8 || bytes[..8] != PNG_SIGNATURE {
        return Err(PngDecodeError::Malformed("no PNG signature"));
    }
    let mut pos = 8;
    let mut w = 0usize;
    let mut h = 0usize;
    let mut channels = 0usize;
    let mut idat = Vec::new();
    while pos + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap_or([0; 4])) as usize;
        let kind = &bytes[pos + 4..pos + 8];
        let end = pos + 8 + len;
        if end + 4 > bytes.len() {
            return Err(PngDecodeError::Malformed("truncated chunk"));
        }
        let data = &bytes[pos + 8..end];
        let crc = u32::from_be_bytes(bytes[end..end + 4].try_into().unwrap_or([0; 4]));
        if crc32(&[kind, data]) != crc {
            return Err(PngDecodeError::BadCrc);
        }
        match kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err(PngDecodeError::Malformed("IHDR length"));
                }
                w = u32::from_be_bytes(data[0..4].try_into().unwrap_or([0; 4])) as usize;
                h = u32::from_be_bytes(data[4..8].try_into().unwrap_or([0; 4])) as usize;
                if data[8] != 8 {
                    return Err(PngDecodeError::Unsupported("bit depth other than 8"));
                }
                channels = match data[9] {
                    2 => 3,
                    6 => 4,
                    _ => {
                        return Err(PngDecodeError::Unsupported(
                            "colour type other than RGB/RGBA",
                        ))
                    }
                };
                if data[12] != 0 {
                    return Err(PngDecodeError::Unsupported("interlaced"));
                }
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos = end + 4;
    }
    if channels == 0 {
        return Err(PngDecodeError::Malformed("no IHDR"));
    }
    let raw =
        miniz_oxide::inflate::decompress_to_vec_zlib(&idat).map_err(|_| PngDecodeError::Inflate)?;
    let stride = w * channels;
    if raw.len() < h * (stride + 1) {
        return Err(PngDecodeError::Malformed("image data too short"));
    }
    let mut prev = vec![0u8; stride];
    let mut cur = vec![0u8; stride];
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let ft = line[0];
        for i in 0..stride {
            let a = if i >= channels { cur[i - channels] } else { 0 };
            let b = prev[i];
            let c = if i >= channels { prev[i - channels] } else { 0 };
            let x = line[1 + i];
            cur[i] = match ft {
                0 => x,
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((u16::from(a) + u16::from(b)) / 2) as u8),
                4 => x.wrapping_add(paeth(a, b, c)),
                _ => return Err(PngDecodeError::Malformed("unknown row filter")),
            };
        }
        for px in cur.chunks_exact(channels) {
            pixels.push(Rgb::new(px[0], px[1], px[2]));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Ok(ImageData {
        width: w,
        height: h,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CRC-32 of "IEND" with no data is the constant every PNG ends with.
    #[test]
    fn crc_of_iend_is_the_well_known_constant() {
        assert_eq!(crc32(&[b"IEND", &[]]), 0xAE42_6082);
        // The standard CRC-32 check value.
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
    }

    /// Encode -> decode is the identity, on an image that exercises every
    /// filter choice (flat runs, horizontal and vertical gradients, noise).
    #[test]
    fn png_round_trip_is_lossless() {
        let (w, h) = (37, 23);
        let mut img = ImageData::filled(w, h, Rgb::new(0, 0, 0));
        let mut s = 12345u64;
        for y in 0..h {
            for x in 0..w {
                s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let noise = (s >> 56) as u8;
                img.set(
                    x,
                    y,
                    Rgb::new(
                        (x * 7) as u8,
                        (y * 11) as u8,
                        if x > 20 { noise } else { 9 },
                    ),
                );
            }
        }
        let bytes = img.to_png_bytes();
        assert_eq!(&bytes[..8], &PNG_SIGNATURE);
        let back = decode_png(&bytes).expect("decodes");
        assert_eq!(back, img);
        // A flipped byte is caught by the CRC, not silently decoded.
        let mut bad = bytes.clone();
        bad[20] ^= 1;
        assert_eq!(decode_png(&bad), Err(PngDecodeError::BadCrc));
    }

    #[test]
    fn ppm_matches_upstream_layout() {
        let img = ImageData::filled(2, 1, Rgb::new(1, 2, 3));
        assert_eq!(
            img.to_ppm_bytes(),
            b"P6\n2 1\n255\n\x01\x02\x03\x01\x02\x03\n".to_vec()
        );
    }
}
