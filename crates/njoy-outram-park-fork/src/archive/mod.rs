// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! # ZIP archives in memory: read and write, bytes in and bytes out
//!
//! **Epic gh #925, #935.** A whole ENDF library arrives as a zip (NNDC
//! publishes ENDF/B-VIII.0 as `ENDF-B-VIII.0_neutrons.zip`, 557 tapes and a
//! changelog in one archive), and the browser user uploads that zip. The same
//! code must read it natively and in `wasm32`, so this module touches no file:
//! [`ZipArchive::parse`] takes the archive's bytes and [`ZipArchive::read`]
//! gives a member's bytes. [`write_zip`] is the other direction (the run report
//! zip of #941, and the tests that zip a folder of tapes).
//!
//! # Why not the `zip` crate
//!
//! `zip` (2.2) is already an optional dependency, but only behind `net-fetch`
//! together with `ureq`, and the maintainer's decision on #925 (2026-10-17)
//! was "no new dependencies; compile time was the concern": write the small
//! container directly over `miniz_oxide`, which every build already has, "the
//! way the PNG writer does" (`outram_blender::csg::plot::image`, whose CRC-32
//! [`crc32`] follows). The `zip` crate stays where it was, for the download
//! path.
//!
//! # What is read (PKWARE APPNOTE 6.3.x)
//!
//! - The **end of central directory** record, found by scanning back over at
//!   most the 65 535-byte comment; then, when a field is saturated, the
//!   **ZIP64** end record through its locator (a library archive over 4 GiB,
//!   or over 65 535 members, needs it).
//! - Each **central directory** header: name, method, CRC-32, sizes, the
//!   local header's offset, and the ZIP64 extra field (id `0x0001`) for any
//!   saturated size or offset. The central directory is authoritative for the
//!   sizes, so members written with a data descriptor (flag bit 3) read the
//!   same as any other.
//! - Methods **0 (stored)** and **8 (DEFLATE)**. Anything else, an encrypted
//!   member (flag bit 0) or a multi-disk archive is refused with a message,
//!   never guessed at.
//!
//! # Malformed input is an error, never a panic
//!
//! Every offset and length read from the archive is checked before it is
//! used, every `u64` is converted with `try_from` (a `wasm32` `usize` is 32
//! bits), a member is inflated with its declared size as the **limit** (so a
//! lying header cannot grow memory past what it declared), and the CRC-32 and
//! the size are checked after. `tests` truncates a valid archive at every
//! byte and flips bytes through it, and asserts each attempt returns.

mod read;
mod write;

pub use read::{ZipArchive, ZipEntry, ZipMethod};
pub use write::{write_zip, WRITE_DOS_DATE};

/// CRC-32 (ISO-HDLC, the polynomial `0xEDB88320` reflected, as ZIP and PNG
/// use). A port of the same table-free form as
/// `outram_blender::csg::plot::image`'s PNG CRC; this crate cannot depend on
/// that one (the dependency runs the other way), and it is a dozen lines.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFF_u32;
    for &b in bytes {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    c ^ 0xFFFF_FFFF
}

/// Whether `bytes` start like a ZIP archive (a local file header, or the end
/// record of an empty archive).
#[must_use]
pub fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

#[cfg(test)]
mod tests;
