//! ACER integration tests: build a continuous-energy ACE table from
//! reconstructed cross sections and verify its structure round-trips.
//!
//! Covers the first ACER increment (ESZ + MTR/LQR/TYR/LSIG/SIG blocks):
//! - U-235 (fissile, MAT=9228) — exercises fission + capture partials.
//! - H-2  (light, MAT=128)     — minimal reaction set.
//!
//! The decisive checks are structural and self-consistent:
//! - NXS/JXS agree with the XSS layout (lengths, locators, END word);
//! - the ESZ total equals elastic + Σ partials at every grid point;
//! - the Type-1 ASCII file the writer emits parses back to the same arrays.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test acer --release
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    ace::{jxs, nxs, AceTable},
    endf::tape::Tape,
    reconr::{reconr, ReconrConfig},
};

fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources");
    p.push(name);
    p
}

fn build(name: &str, mat: i32) -> AceTable {
    let tape = Tape::read(File::open(fixture(name)).unwrap()).unwrap();
    let cfg = ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 };
    let res = reconr(&tape, &cfg).unwrap();
    AceTable::from_reconr(&res, 0.0, 0)
}

/// A minimal parsed Type-1 ACE table: just the arrays we need to validate.
struct ParsedAce {
    nxs: Vec<i32>,
    jxs: Vec<i32>,
    xss: Vec<f64>,
}

/// Parse a Type-1 ASCII ACE table written by [`AceTable::write_to`].
///
/// Layout: 2 header lines, 4 IZAW lines, 6 lines of NXS(16)+JXS(32) (8 ints
/// each), then the XSS data (4 values per line). Numeric fields are
/// whitespace-separable because every field is right-justified in its column.
fn parse_type1(text: &str) -> ParsedAce {
    let lines: Vec<&str> = text.lines().collect();
    // Skip 2 header + 4 IZAW lines.
    let mut ints: Vec<i32> = Vec::new();
    for line in &lines[6..12] {
        for tok in line.split_whitespace() {
            ints.push(tok.parse().unwrap());
        }
    }
    assert_eq!(ints.len(), 48, "expected 16 NXS + 32 JXS integers");
    let nxs = ints[..16].to_vec();
    let jxs = ints[16..].to_vec();

    let mut xss: Vec<f64> = Vec::new();
    for line in &lines[12..] {
        for tok in line.split_whitespace() {
            xss.push(tok.parse().unwrap());
        }
    }
    ParsedAce { nxs, jxs, xss }
}

#[test]
fn u235_ace_header_and_dimensions() {
    let ace = build("n-092_U_235-ENDF8.0.endf", 9228);

    assert_eq!(ace.nxs[nxs::ZA], 92235, "ZA");
    assert_eq!(ace.nxs[nxs::Z], 92, "Z");
    assert_eq!(ace.nxs[nxs::A], 235, "A");
    assert!(ace.zaid.starts_with("92235.00c") || ace.zaid.trim() == "92235.00c");
    assert!(ace.nxs[nxs::NES] > 1000, "U-235 union grid should be large");
    assert!(ace.nxs[nxs::NTR] >= 2, "at least fission + capture partials");

    // XSS length self-consistency.
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
    assert_eq!(ace.jxs[jxs::ESZ], 1, "ESZ starts at word 1");
    assert_eq!(ace.jxs[jxs::END] as usize, ace.xss.len(), "END = last word");
    assert_eq!(ace.xss_is_int.len(), ace.xss.len(), "type mask covers XSS");
}

#[test]
fn esz_total_equals_elastic_plus_partials() {
    let ace = build("n-092_U_235-ENDF8.0.endf", 9228);
    let nes = ace.nxs[nxs::NES] as usize;
    let ntr = ace.nxs[nxs::NTR] as usize;

    // ESZ columns (0-based offsets within the block at word jxs::ESZ=1).
    let energy = &ace.xss[0..nes];
    let total = &ace.xss[nes..2 * nes];
    let disap = &ace.xss[2 * nes..3 * nes];
    let elastic = &ace.xss[3 * nes..4 * nes];

    // Grid is ascending and expressed in MeV (top below the 20 MeV ceiling).
    assert!(energy.windows(2).all(|w| w[1] >= w[0]), "energy grid ascending");
    // In MeV the top is ~20–30; in eV it would be ~2–3e7. The loose bound just
    // confirms the unit conversion happened.
    assert!(*energy.last().unwrap() < 1000.0, "energies are in MeV, not eV");
    assert!(energy[0] > 0.0);

    // Re-sum the SIG partials at each grid point and compare to the ESZ total.
    let sig0 = ace.jxs[jxs::SIG] as usize; // 1-based
    for j in 0..nes {
        let mut sum = elastic[j];
        for i in 0..ntr {
            // Each SIG entry is [IE, NE, σ(1..NE)] laid contiguously; IE=1, NE=nes.
            let base = (sig0 - 1) + i * (2 + nes);
            let ie = ace.xss[base] as usize;
            let ne = ace.xss[base + 1] as usize;
            assert_eq!(ie, 1);
            assert_eq!(ne, nes);
            sum += ace.xss[base + 2 + j];
        }
        let t = total[j];
        assert!(
            (t - sum).abs() <= 1e-5 * t.abs().max(1.0),
            "total[{j}]={t} != elastic+Σpartials={sum}"
        );
        assert!(disap[j] <= t + 1e-9, "disappearance ≤ total");
        assert!(elastic[j] <= t + 1e-9, "elastic ≤ total");
        assert!(total[j] >= 0.0 && disap[j] >= 0.0 && elastic[j] >= 0.0);
    }
}

#[test]
fn type1_file_roundtrips() {
    let ace = build("n-001_H_002-ENDF8.0.endf", 128);

    let mut buf: Vec<u8> = Vec::new();
    ace.write_to(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let parsed = parse_type1(&text);

    // Integer arrays must match exactly.
    assert_eq!(parsed.nxs, ace.nxs.to_vec(), "NXS round-trip");
    assert_eq!(parsed.jxs, ace.jxs.to_vec(), "JXS round-trip");

    // XSS length matches; values match (reals to 11 significant figures, ints
    // exactly since they were written as i20).
    assert_eq!(parsed.xss.len(), ace.xss.len(), "XSS length round-trip");
    for (i, (&got, &want)) in parsed.xss.iter().zip(ace.xss.iter()).enumerate() {
        let tol = if ace.xss_is_int[i] { 0.0 } else { 1e-9 * want.abs().max(1.0) };
        assert!(
            (got - want).abs() <= tol,
            "xss[{i}] round-trip: got {got}, want {want}"
        );
    }
}

#[test]
fn h2_minimal_table_is_valid() {
    let ace = build("n-001_H_002-ENDF8.0.endf", 128);
    assert_eq!(ace.nxs[nxs::ZA], 1002, "H-2 ZA");
    assert!(ace.nxs[nxs::NES] > 1, "has an energy grid");
    // H-2 has elastic + capture at least → at least one MTR partial (capture).
    assert!(ace.nxs[nxs::NTR] >= 1, "at least one non-elastic reaction");
    assert_eq!(ace.jxs[jxs::MTR] != 0, ace.nxs[nxs::NTR] > 0);
}
