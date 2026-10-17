// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

use std::sync::Arc;

use super::*;

fn sample() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "lib/neutrons/n-001_H_001.endf".into(),
            b"tape one\n".repeat(400),
        ),
        ("lib/README.txt".into(), b"x".to_vec()),
        ("lib/empty".into(), Vec::new()),
    ]
}

/// The CRC-32 check value of the standard (the CRC of "123456789").
#[test]
fn crc32_matches_the_standard_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

/// What is written is read back byte for byte; DEFLATE where it helps,
/// stored where it does not; the prefix read inflates only what it needs.
#[test]
fn written_archives_read_back_exactly() {
    let files = sample();
    let bytes = write_zip(&files).unwrap();
    assert!(looks_like_zip(&bytes));
    let z = ZipArchive::parse(Arc::new(bytes.clone())).unwrap();
    assert_eq!(z.entries().len(), 3);
    assert_eq!(z.bytes(), bytes.as_slice());
    for (i, (name, data)) in files.iter().enumerate() {
        let e = &z.entries()[i];
        assert_eq!(&e.name, name);
        assert!(!e.is_dir());
        assert_eq!(&z.read(i).unwrap(), data);
        assert_eq!(z.position(name), Some(i));
    }
    assert_eq!(z.entries()[0].method, ZipMethod::Deflate);
    assert_eq!(z.entries()[1].method, ZipMethod::Stored);
    assert_eq!(z.read_prefix(0, 12).unwrap(), b"tape one\ntap");
    assert_eq!(z.read_prefix(1, 12).unwrap(), b"x");
    assert!(z.read(9).is_err());
    assert_eq!(z.position("nope"), None);
    // Deterministic: the same files give the same archive.
    assert_eq!(write_zip(&files).unwrap(), bytes);
    assert!(write_zip(&[(String::new(), vec![1])]).is_err());
}

/// **Malformed input is an error, never a panic**: every truncation of a
/// valid archive, and a byte flipped at every position, return (Ok or Err)
/// from both parse and read.
#[test]
fn truncated_or_corrupted_archives_never_panic() {
    let bytes = write_zip(&sample()).unwrap();
    for cut in 0..bytes.len() {
        if let Ok(z) = ZipArchive::parse(Arc::new(bytes[..cut].to_vec())) {
            for i in 0..z.entries().len() {
                let _ = z.read(i);
                let _ = z.read_prefix(i, 64);
            }
        }
    }
    let mut corrupt_reads = 0;
    for pos in 0..bytes.len() {
        let mut b = bytes.clone();
        b[pos] ^= 0x5A;
        if let Ok(z) = ZipArchive::parse(Arc::new(b)) {
            for i in 0..z.entries().len() {
                if z.read(i).is_err() {
                    corrupt_reads += 1;
                }
                let _ = z.read_prefix(i, 64);
            }
        }
    }
    assert!(
        corrupt_reads > 0,
        "flipped data bytes must fail the CRC or the inflate"
    );
    assert!(ZipArchive::parse(Arc::new(b"not a zip at all, just text".to_vec())).is_err());
    assert!(ZipArchive::parse(Arc::new(Vec::new())).is_err());
}

/// A ZIP64 end record (as a large library archive carries) is followed, and
/// a ZIP64 extra field replaces the saturated sizes. Built by hand from a
/// classic archive: the end record's fields are saturated and the real values
/// moved into the ZIP64 records.
#[test]
fn zip64_end_record_and_extra_field_are_read() {
    let data = b"zip64 member".to_vec();
    // One stored member with a ZIP64 extra field carrying both sizes.
    let mut out = Vec::new();
    let crc = crc32(&data);
    let name = b"a.endf";
    out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    out.extend_from_slice(&[45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&u32::MAX.to_le_bytes());
    out.extend_from_slice(&u32::MAX.to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(&data);
    let cd_off = out.len() as u64;
    let mut cd = Vec::new();
    cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    cd.extend_from_slice(&[45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    cd.extend_from_slice(&crc.to_le_bytes());
    cd.extend_from_slice(&u32::MAX.to_le_bytes());
    cd.extend_from_slice(&u32::MAX.to_le_bytes());
    cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
    cd.extend_from_slice(&20u16.to_le_bytes());
    cd.extend_from_slice(&[0; 10]); // comment, disk, internal, external attributes
    cd.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    cd.extend_from_slice(name);
    cd.extend_from_slice(&1u16.to_le_bytes());
    cd.extend_from_slice(&16u16.to_le_bytes());
    cd.extend_from_slice(&(data.len() as u64).to_le_bytes());
    cd.extend_from_slice(&(data.len() as u64).to_le_bytes());
    out.extend_from_slice(&cd);
    let z64_off = out.len() as u64;
    out.extend_from_slice(&0x0606_4b50u32.to_le_bytes());
    out.extend_from_slice(&44u64.to_le_bytes());
    out.extend_from_slice(&[45, 0, 45, 0]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&(cd.len() as u64).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0x0706_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&z64_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    out.extend_from_slice(&u32::MAX.to_le_bytes());
    out.extend_from_slice(&u32::MAX.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    let z = ZipArchive::parse(Arc::new(out)).unwrap();
    assert_eq!(z.entries().len(), 1);
    assert_eq!(z.entries()[0].size, data.len() as u64);
    assert_eq!(z.read(0).unwrap(), data);
}
