// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Writing a ZIP archive in memory, DEFLATE over `miniz_oxide`.

use super::crc32;
use crate::NjoyError;

/// The MS-DOS date every member is stamped with: 1980-01-01, the format's
/// epoch. A fixed date makes the archive a function of its contents only, so
/// the same files zip to the same bytes on every machine and in a browser
/// (the native-against-wasm fingerprint in #935 depends on it).
pub const WRITE_DOS_DATE: u16 = (0 << 9) | (1 << 5) | 1;

fn push16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// **Zip `files` (`(path, bytes)`) into one archive, in memory.** Each member
/// is DEFLATE-compressed at `miniz_oxide`'s level 6, or stored when that would
/// not make it smaller; paths are written as given, flagged UTF-8.
///
/// No ZIP64: an archive over 4 GiB or 65 535 members is refused (the reports
/// and test fixtures this writes are far smaller). The reader reads ZIP64.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for an empty or over-long path, or a size the
/// classic format cannot hold.
pub fn write_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, NjoyError> {
    let too_big = || {
        NjoyError::InvalidInput(
            "zip writer: over 4 GiB or 65 535 members needs ZIP64, not written".into(),
        )
    };
    let count = u16::try_from(files.len()).map_err(|_| too_big())?;
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let name_len = u16::try_from(name.len())
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| {
                NjoyError::InvalidInput(format!("zip writer: path '{name}' is empty or too long"))
            })?;
        let crc = crc32(data);
        let deflated = miniz_oxide::deflate::compress_to_vec(data, 6);
        let (method, body): (u16, &[u8]) = if deflated.len() < data.len() {
            (8, &deflated)
        } else {
            (0, data)
        };
        let size = u32::try_from(data.len()).map_err(|_| too_big())?;
        let csize = u32::try_from(body.len()).map_err(|_| too_big())?;
        let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
        // Local header (APPNOTE 4.3.7).
        push32(&mut out, 0x0403_4b50);
        push16(&mut out, 20); // version needed: 2.0 (DEFLATE)
        push16(&mut out, 1 << 11); // UTF-8 names
        push16(&mut out, method);
        push16(&mut out, 0); // time 00:00:00
        push16(&mut out, WRITE_DOS_DATE);
        push32(&mut out, crc);
        push32(&mut out, csize);
        push32(&mut out, size);
        push16(&mut out, name_len);
        push16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        // Central directory header (APPNOTE 4.3.12).
        push32(&mut central, 0x0201_4b50);
        push16(&mut central, 20); // made by: 2.0, MS-DOS attributes
        push16(&mut central, 20);
        push16(&mut central, 1 << 11);
        push16(&mut central, method);
        push16(&mut central, 0);
        push16(&mut central, WRITE_DOS_DATE);
        push32(&mut central, crc);
        push32(&mut central, csize);
        push32(&mut central, size);
        push16(&mut central, name_len);
        push16(&mut central, 0); // extra
        push16(&mut central, 0); // comment
        push16(&mut central, 0); // disk
        push16(&mut central, 0); // internal attributes
        push32(&mut central, 0); // external attributes
        push32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = u32::try_from(out.len()).map_err(|_| too_big())?;
    let cd_size = u32::try_from(central.len()).map_err(|_| too_big())?;
    out.extend_from_slice(&central);
    // End of central directory (APPNOTE 4.3.16).
    push32(&mut out, 0x0605_4b50);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, count);
    push16(&mut out, count);
    push32(&mut out, cd_size);
    push32(&mut out, cd_offset);
    push16(&mut out, 0);
    Ok(out)
}
