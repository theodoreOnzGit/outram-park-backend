// SPDX-License-Identifier: GPL-3.0

//! **ACE file layouts the reader did not read before the GitHub #365 audit,
//! checked against OpenMC's own reader.**
//!
//! # What was missing
//!
//! | layout | upstream reader | before |
//! |---|---|---|
//! | ACE 2.0.1 header | `openmc/data/ace.py:349-359` | parse error (version token read as the ZAID) |
//! | several tables in one file, one picked by name | `ace.py:194-241`, `get_table` | XSS-length error |
//! | NJOY's exponent-less floats (`1.2345-05`) | `ace.py:399-405` (`ENDF_FLOAT_RE`) | parse error |
//! | direct-access Type 2 (4096-byte records) | `ace.py:243-325` | mis-sniffed as text |
//! | gzipped Type 2 | — | always parsed as Type 1 |
//!
//! # Methodology
//!
//! `crates/outram-mc-libs/verification_and_validation/ace_route_physics/openmc_inputs/ace_format_variants.py`
//! builds each layout from the five-route study's NJOY2016 tables
//! (`target/five_route_keff/njoy/293.6K/`) into `target/ace_format_variants/`,
//! asserts that OpenMC 0.16.1.dev25 decodes every variant it can read
//! **identically to the original table**, and records per table the AWR, kT,
//! NXS, JXS, the XSS length, its sum and its end values in
//! `data/ace_format_variants_openmc.csv`. Two cases have no OpenMC read:
//! OpenMC cannot read NJOY's *sequential* Type 2 at all, and with NumPy 2.5.3
//! `np.fromstring` raises on `1.2345-05` before OpenMC's fallback runs. For
//! both, the reference is the original table. Its values are unchanged by the
//! rewrite.
//!
//! Each variant must decode with this crate's reader to exactly OpenMC's
//! numbers. There is no tolerance: parsing either reproduces the file's values
//! or it does not. It must also be bit-identical to this reader's decoding of
//! the original table. Skipped when the generated files are absent;
//! `acer::read`'s unit tests cover the same layouts on synthetic tables
//! without data.
//!
//! # Results (2026-09-29)
//!
//! All five variants are equal, in 5 table rows plus the gzip case (printed by
//! the test).

use njoy_outram_park_fork::acer::read::{read, read_library, read_table, RawAceTable};
use std::path::PathBuf;

fn ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn same(a: &RawAceTable, b: &RawAceTable) -> bool {
    a.nxs == b.nxs
        && a.jxs == b.jxs
        && a.header.awr.to_bits() == b.header.awr.to_bits()
        && a.header.kt_mev.to_bits() == b.header.kt_mev.to_bits()
        && a.xss.len() == b.xss.len()
        && a.xss
            .iter()
            .zip(&b.xss)
            .all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn ace_layouts_decode_as_openmc_decodes_them() {
    let dir = ws().join("target/ace_format_variants");
    let src = ws().join("target/five_route_keff/njoy/293.6K");
    if !dir.is_dir() || !src.is_dir() {
        println!("generated variants absent: skipping (see the module doc)");
        return;
    }
    let csv = ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/ace_format_variants_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference csv");
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split(',').collect();
        let (file, name) = (f[0], f[1]);
        let t = read_table(dir.join(file), name).unwrap_or_else(|e| panic!("{file} {name}: {e}"));
        assert_eq!(t.header.zaid, name);
        assert_eq!(t.header.awr, f[2].parse::<f64>().unwrap(), "{file}: awr");
        assert_eq!(t.header.kt_mev, f[3].parse::<f64>().unwrap(), "{file}: kT");
        assert_eq!(
            t.xss.len(),
            f[4].parse::<usize>().unwrap(),
            "{file}: XSS length"
        );
        for k in 0..16 {
            assert_eq!(
                t.nxs[k],
                f[5 + k].parse::<i32>().unwrap(),
                "{file}: NXS({})",
                k + 1
            );
        }
        for k in 0..32 {
            assert_eq!(
                t.jxs[k],
                f[21 + k].parse::<i32>().unwrap(),
                "{file}: JXS({})",
                k + 1
            );
        }
        let sum: f64 = t.xss.iter().sum();
        assert_eq!(sum, f[53].parse::<f64>().unwrap(), "{file}: XSS sum");
        assert_eq!(t.xss[0], f[54].parse::<f64>().unwrap());
        assert_eq!(*t.xss.last().unwrap(), f[55].parse::<f64>().unwrap());
        // And bit-identical to this reader on the original table.
        let base = match name {
            "9019.800nc" => "F19",
            "1001.00c" => "H1",
            "8016.00c" => "O16",
            other => panic!("unexpected table {other}"),
        };
        let orig = read(src.join(format!("{base}.ace"))).expect("original");
        assert!(
            same(&t, &orig),
            "{file} {name}: differs from the original table"
        );
        println!(
            "{file} {name}: equal to OpenMC and to the original ({} XSS)",
            t.xss.len()
        );
        rows += 1;
    }
    assert_eq!(rows, 5);

    // The library file is refused by the single-table `read`, by name.
    let err = read(dir.join("H1_O16_library.ace"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("2 ACE tables") && err.contains("8016.00c"),
        "{err}"
    );
    assert_eq!(
        read_library(dir.join("H1_O16_library.ace")).unwrap().len(),
        2
    );

    // Gzipped NJOY sequential Type 2: equal to the uncompressed file.
    let gz = read(dir.join("B10_seq.ace.gz")).expect("gzipped Type 2");
    let plain = read(src.join("B10.ace")).expect("B10 Type 2");
    assert!(
        same(&gz, &plain),
        "gzipped Type 2 differs from the uncompressed file"
    );
    println!("B10_seq.ace.gz: equal to the uncompressed sequential Type 2");
}
