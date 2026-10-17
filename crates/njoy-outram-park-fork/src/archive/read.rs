// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Reading a ZIP archive held in memory. See the module docs of
//! [`crate::archive`] for the format subset and the error policy.

use std::sync::Arc;

use super::crc32;
use crate::NjoyError;

const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD64_LOCATOR_SIG: u32 = 0x0706_4b50;
const EOCD64_SIG: u32 = 0x0606_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;

/// How a member's bytes are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipMethod {
    /// Method 0: the bytes as they are.
    Stored,
    /// Method 8: raw DEFLATE (RFC 1951).
    Deflate,
    /// Any other method number; listed, refused when read.
    Unsupported(u16),
}

/// One member of a [`ZipArchive`], as its central directory header gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// The member's path inside the archive, `/`-separated
    /// (`ENDF-B-VIII.0_neutrons/n-092_U_235.endf`).
    pub name: String,
    /// How it is stored.
    pub method: ZipMethod,
    /// CRC-32 of the uncompressed bytes.
    pub crc32: u32,
    /// Stored (compressed) size in bytes.
    pub compressed_size: u64,
    /// Uncompressed size in bytes.
    pub size: u64,
    /// Encrypted (general-purpose flag bit 0): listed, refused when read.
    pub encrypted: bool,
    local_header_offset: u64,
}

impl ZipEntry {
    /// A directory entry (its name ends in `/`).
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// **A ZIP archive in memory**: its bytes, shared, and its member list. Bytes
/// in, bytes out; nothing here touches a file, so it runs the same natively and
/// in a browser.
#[derive(Debug, Clone)]
pub struct ZipArchive {
    bytes: Arc<Vec<u8>>,
    entries: Vec<ZipEntry>,
}

fn bad(msg: impl Into<String>) -> NjoyError {
    NjoyError::InvalidInput(format!("zip archive: {}", msg.into()))
}

fn slice(b: &[u8], off: u64, len: u64) -> Result<&[u8], NjoyError> {
    let start =
        usize::try_from(off).map_err(|_| bad(format!("offset {off} does not fit in memory")))?;
    let n =
        usize::try_from(len).map_err(|_| bad(format!("length {len} does not fit in memory")))?;
    let end = start
        .checked_add(n)
        .ok_or_else(|| bad("offset plus length overflows"))?;
    b.get(start..end).ok_or_else(|| {
        bad(format!(
            "bytes {start}..{end} lie outside the {}-byte archive",
            b.len()
        ))
    })
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_at(b: &[u8], i: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(a)
}

/// Where the central directory is, and how many members it lists.
struct Directory {
    offset: u64,
    size: u64,
    count: u64,
}

fn find_directory(b: &[u8]) -> Result<Directory, NjoyError> {
    if b.len() < 22 {
        return Err(bad(format!(
            "{} bytes is too short to hold an end record",
            b.len()
        )));
    }
    // The end record is 22 bytes plus a comment of at most 65 535 bytes.
    let lowest = b.len().saturating_sub(22 + 65_535);
    let mut at = None;
    let mut i = b.len() - 22;
    loop {
        if u32_at(b, i) == EOCD_SIG && i + 22 + usize::from(u16_at(b, i + 20)) == b.len() {
            at = Some(i);
            break;
        }
        if i == lowest {
            break;
        }
        i -= 1;
    }
    let e =
        at.ok_or_else(|| bad("no end-of-central-directory record (not a zip, or truncated)"))?;
    let (disk, cd_disk) = (u16_at(b, e + 4), u16_at(b, e + 6));
    let mut dir = Directory {
        count: u64::from(u16_at(b, e + 10)),
        size: u64::from(u32_at(b, e + 12)),
        offset: u64::from(u32_at(b, e + 16)),
    };
    let saturated = dir.count == 0xFFFF || dir.size == 0xFFFF_FFFF || dir.offset == 0xFFFF_FFFF;
    if saturated && e >= 20 && u32_at(b, e - 20) == EOCD64_LOCATOR_SIG {
        let z64 = u64_at(b, e - 20 + 8);
        let rec = slice(b, z64, 56)?;
        if u32_at(rec, 0) != EOCD64_SIG {
            return Err(bad("the ZIP64 locator points at no ZIP64 end record"));
        }
        if u32_at(rec, 16) != 0 || u32_at(rec, 20) != 0 {
            return Err(bad("multi-disk archives are not supported"));
        }
        dir = Directory {
            count: u64_at(rec, 32),
            size: u64_at(rec, 40),
            offset: u64_at(rec, 48),
        };
    } else if disk != 0 || cd_disk != 0 {
        return Err(bad("multi-disk archives are not supported"));
    }
    Ok(dir)
}

/// The ZIP64 extended-information extra field (id 0x0001): replaces each
/// saturated field, in the order size, compressed size, offset.
fn apply_zip64(
    extra: &[u8],
    size: &mut u64,
    csize: &mut u64,
    offset: &mut u64,
) -> Result<(), NjoyError> {
    let mut i = 0usize;
    while i + 4 <= extra.len() {
        let (id, len) = (u16_at(extra, i), usize::from(u16_at(extra, i + 2)));
        let body = extra
            .get(i + 4..i + 4 + len)
            .ok_or_else(|| bad("an extra field runs past its header"))?;
        if id == 0x0001 {
            let mut j = 0usize;
            for field in [size, csize, offset] {
                if *field == 0xFFFF_FFFF {
                    if j + 8 > body.len() {
                        return Err(bad("the ZIP64 extra field is too short"));
                    }
                    *field = u64_at(body, j);
                    j += 8;
                }
            }
            return Ok(());
        }
        i += 4 + len;
    }
    if [*size, *csize, *offset].contains(&0xFFFF_FFFF) {
        return Err(bad("a saturated size or offset has no ZIP64 extra field"));
    }
    Ok(())
}

impl ZipArchive {
    /// **Read an archive's member list** from its bytes.
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] naming what is wrong: no end record, a
    /// central directory outside the bytes, a truncated header, multi-disk.
    pub fn parse(bytes: Arc<Vec<u8>>) -> Result<Self, NjoyError> {
        let b = bytes.as_slice();
        let dir = find_directory(b)?;
        let cd = slice(b, dir.offset, dir.size)?;
        // Each header is at least 46 bytes; a count the directory cannot hold
        // is a corrupt record, not a reason to reserve memory for it.
        let cap = usize::try_from(dir.count.min((cd.len() / 46) as u64)).unwrap_or(0);
        let mut entries = Vec::with_capacity(cap);
        let mut i = 0usize;
        for k in 0..dir.count {
            let h = cd
                .get(i..i + 46)
                .ok_or_else(|| bad(format!("central header {k} runs past the directory")))?;
            if u32_at(h, 0) != CENTRAL_SIG {
                return Err(bad(format!("central header {k} has a bad signature")));
            }
            let flags = u16_at(h, 8);
            let method = match u16_at(h, 10) {
                0 => ZipMethod::Stored,
                8 => ZipMethod::Deflate,
                m => ZipMethod::Unsupported(m),
            };
            let (n_name, n_extra, n_comment) = (
                usize::from(u16_at(h, 28)),
                usize::from(u16_at(h, 30)),
                usize::from(u16_at(h, 32)),
            );
            let name_bytes = cd
                .get(i + 46..i + 46 + n_name)
                .ok_or_else(|| bad(format!("central header {k}'s name runs past the directory")))?;
            let extra = cd
                .get(i + 46 + n_name..i + 46 + n_name + n_extra)
                .ok_or_else(|| {
                    bad(format!(
                        "central header {k}'s extra field runs past the directory"
                    ))
                })?;
            let mut size = u64::from(u32_at(h, 24));
            let mut csize = u64::from(u32_at(h, 20));
            let mut offset = u64::from(u32_at(h, 42));
            apply_zip64(extra, &mut size, &mut csize, &mut offset)?;
            entries.push(ZipEntry {
                name: String::from_utf8_lossy(name_bytes).replace('\\', "/"),
                method,
                crc32: u32_at(h, 16),
                compressed_size: csize,
                size,
                encrypted: flags & 1 != 0,
                local_header_offset: offset,
            });
            i += 46 + n_name + n_extra + n_comment;
        }
        Ok(Self { bytes, entries })
    }

    /// The members, in central-directory order.
    #[must_use]
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// The index of the member called `name`, if there is one.
    #[must_use]
    pub fn position(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    /// The stored bytes of member `i` (still compressed).
    fn stored(&self, i: usize) -> Result<(&ZipEntry, &[u8]), NjoyError> {
        let e = self.entries.get(i).ok_or_else(|| {
            bad(format!(
                "no member {i} (the archive has {})",
                self.entries.len()
            ))
        })?;
        if e.encrypted {
            return Err(bad(format!("{} is encrypted", e.name)));
        }
        let b = self.bytes.as_slice();
        let h = slice(b, e.local_header_offset, 30)?;
        if u32_at(h, 0) != LOCAL_SIG {
            return Err(bad(format!(
                "{}'s local header has a bad signature",
                e.name
            )));
        }
        let skip = 30 + u64::from(u16_at(h, 26)) + u64::from(u16_at(h, 28));
        let start = e
            .local_header_offset
            .checked_add(skip)
            .ok_or_else(|| bad("data offset overflows"))?;
        Ok((e, slice(b, start, e.compressed_size)?))
    }

    /// **Member `i`'s bytes**, inflated, with its size and CRC-32 checked.
    ///
    /// # Errors
    /// No such member; encrypted; an unsupported method; data outside the
    /// archive; a DEFLATE stream that is corrupt, truncated or longer than
    /// declared; a size or CRC-32 that does not match the header.
    pub fn read(&self, i: usize) -> Result<Vec<u8>, NjoyError> {
        let (e, data) = self.stored(i)?;
        let limit = usize::try_from(e.size)
            .map_err(|_| bad(format!("{} is too large for memory", e.name)))?;
        let out = match e.method {
            ZipMethod::Stored => data.to_vec(),
            ZipMethod::Deflate => {
                miniz_oxide::inflate::decompress_to_vec_with_limit(data, limit)
                    .map_err(|err| bad(format!("{}: DEFLATE stream: {err}", e.name)))?
            }
            ZipMethod::Unsupported(m) => {
                return Err(bad(format!(
                    "{} uses compression method {m}; only 0 and 8 are read",
                    e.name
                )))
            }
        };
        if out.len() as u64 != e.size {
            return Err(bad(format!(
                "{} inflated to {} bytes, its header says {}",
                e.name,
                out.len(),
                e.size
            )));
        }
        if crc32(&out) != e.crc32 {
            return Err(bad(format!("{} fails its CRC-32 check", e.name)));
        }
        Ok(out)
    }

    /// **The first `max` bytes of member `i`** (fewer when it is shorter),
    /// inflating no more than that: enough to read an ENDF tape's header
    /// without inflating a 35 MB tape. Unchecked against the CRC, which needs
    /// the whole member; [`Self::read`] checks it.
    ///
    /// # Errors
    /// As [`Self::read`], except the size and CRC checks.
    pub fn read_prefix(&self, i: usize, max: usize) -> Result<Vec<u8>, NjoyError> {
        let (e, data) = self.stored(i)?;
        match e.method {
            ZipMethod::Stored => Ok(data[..data.len().min(max)].to_vec()),
            ZipMethod::Deflate => {
                let want = max.min(usize::try_from(e.size).unwrap_or(usize::MAX));
                match miniz_oxide::inflate::decompress_to_vec_with_limit(data, want) {
                    Ok(v) => Ok(v),
                    Err(err) if err.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput => {
                        Ok(err.output)
                    }
                    Err(err) => Err(bad(format!("{}: DEFLATE stream: {err}", e.name))),
                }
            }
            ZipMethod::Unsupported(m) => Err(bad(format!(
                "{} uses compression method {m}; only 0 and 8 are read",
                e.name
            ))),
        }
    }

    /// The archive's own bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
