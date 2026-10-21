//! **A small ZIP writer** over `miniz_oxide` (gh #941): DEFLATE entries,
//! no dependency beyond what every build already has (maintainer decision on
//! #925: "no new dependencies"; the optional `zip` crate is gated with
//! `ureq` and was not used).
//!
//! The container follows PKWARE's APPNOTE.TXT (6.3.10): one local file
//! header (signature `0x04034b50`) and the compressed data per entry, a
//! central directory (`0x02014b50`) and the end-of-central-directory record
//! (`0x06054b50`). Method 8 (DEFLATE, raw, `miniz_oxide::deflate::compress_to_vec`
//! at level 6) unless DEFLATE does not shrink an entry, then method 0
//! (stored). CRC-32 is outram-blender's (the PNG one). Every timestamp is
//! the DOS epoch (1980-01-01 00:00), so the same report always gives the
//! same bytes. No ZIP64: entries and the archive stay under 4 GiB, which a
//! diagnostics report does by orders of magnitude (checked; an `Err` past
//! it).

use outram_blender::csg::plot::image::crc32;

/// Why the archive could not be written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipError {
    /// An entry or the archive passed a 32-bit ZIP limit.
    #[error("the zip passes a 32-bit limit ({0}); ZIP64 is not written")]
    TooLarge(String),
}

/// `files` as `(path, bytes)` → the bytes of a ZIP archive.
pub(crate) fn zip_bytes(files: &[(String, &[u8])]) -> Result<Vec<u8>, ZipError> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let u32_of = |x: usize, what: &str| -> Result<u32, ZipError> {
        u32::try_from(x).map_err(|_| ZipError::TooLarge(what.to_string()))
    };
    for (name, data) in files {
        let crc = crc32(&[data]);
        let deflated = miniz_oxide::deflate::compress_to_vec(data, 6);
        let (method, body): (u16, &[u8]) = if deflated.len() < data.len() {
            (8, &deflated)
        } else {
            (0, data)
        };
        let offset = u32_of(out.len(), "archive offset")?;
        let csize = u32_of(body.len(), name)?;
        let usize_ = u32_of(data.len(), name)?;
        let name_b = name.as_bytes();
        let nlen = u16::try_from(name_b.len()).map_err(|_| ZipError::TooLarge(name.clone()))?;
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // flags: UTF-8 names
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0x0021u16.to_le_bytes()); // date 1980-01-01
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&csize.to_le_bytes());
        out.extend_from_slice(&usize_.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra length
        out.extend_from_slice(name_b);
        out.extend_from_slice(body);
        // Central directory entry.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x0021u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&csize.to_le_bytes());
        central.extend_from_slice(&usize_.to_le_bytes());
        central.extend_from_slice(&nlen.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        central.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_b);
    }
    let n = u16::try_from(files.len()).map_err(|_| ZipError::TooLarge("entry count".into()))?;
    let cd_offset = u32_of(out.len(), "central directory offset")?;
    let cd_size = u32_of(central.len(), "central directory size")?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

/// Read an archive written by [`zip_bytes`] back: `(path, bytes)` per entry,
/// with each entry's CRC checked. For tests and for the browser side to
/// verify a download; it reads the central directory, as APPNOTE says a
/// reader should.
pub(crate) fn unzip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let u16_at = |i: usize| -> Result<u16, String> {
        bytes
            .get(i..i + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "truncated".to_string())
    };
    let u32_at = |i: usize| -> Result<u32, String> {
        bytes
            .get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "truncated".to_string())
    };
    let eocd = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_at(i) == Ok(0x0605_4b50))
        .ok_or("no end-of-central-directory record")?;
    let n = u16_at(eocd + 10)? as usize;
    let mut p = u32_at(eocd + 16)? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if u32_at(p)? != 0x0201_4b50 {
            return Err("bad central directory entry".into());
        }
        let method = u16_at(p + 10)?;
        let crc = u32_at(p + 16)?;
        let csize = u32_at(p + 20)? as usize;
        let nlen = u16_at(p + 28)? as usize;
        let elen = u16_at(p + 30)? as usize;
        let clen = u16_at(p + 32)? as usize;
        let off = u32_at(p + 42)? as usize;
        let name = String::from_utf8(
            bytes
                .get(p + 46..p + 46 + nlen)
                .ok_or("truncated")?
                .to_vec(),
        )
        .map_err(|e| e.to_string())?;
        let lnlen = u16_at(off + 26)? as usize;
        let lelen = u16_at(off + 28)? as usize;
        let start = off + 30 + lnlen + lelen;
        let body = bytes.get(start..start + csize).ok_or("truncated entry")?;
        let data = match method {
            0 => body.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(body).map_err(|e| format!("{e:?}"))?,
            m => return Err(format!("method {m} not supported")),
        };
        if crc32(&[&data]) != crc {
            return Err(format!("CRC mismatch on {name}"));
        }
        out.push((name, data));
        p += 46 + nlen + elen + clen;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An archive round-trips: names, contents (stored and deflated) and
    /// CRCs; the bytes are the same every time.
    #[test]
    fn zip_round_trips() {
        let big = "row,1.0\n".repeat(500);
        let files = vec![
            ("report.md".to_string(), b"# title\n".as_slice()),
            ("data/big.csv".to_string(), big.as_bytes()),
            ("x".to_string(), b"".as_slice()),
        ];
        let z = zip_bytes(&files).unwrap();
        assert_eq!(z, zip_bytes(&files).unwrap(), "deterministic");
        let back = unzip(&z).unwrap();
        assert_eq!(back.len(), 3);
        for ((n, d), (m, e)) in files.iter().zip(&back) {
            assert_eq!(n, m);
            assert_eq!(*d, e.as_slice());
        }
        assert!(z.len() < big.len() / 2, "the big entry is deflated");
        assert!(unzip(&z[..10]).is_err());
    }
}
