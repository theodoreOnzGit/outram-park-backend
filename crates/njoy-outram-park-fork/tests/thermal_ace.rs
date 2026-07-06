//! Thermal S(α,β) ACE table (Phase 4f): build a `…t` table from MF=7 and check
//! its structure round-trips through the Type-1 writer.
//!
//! Fixture: `tests/resources/tsl-013_Al_027-ENDF8.0.endf` (MAT=53) — Al-27, which
//! has both coherent-elastic (Bragg) and incoherent-inelastic S(α,β) data.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test thermal_ace --release
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    acer::{
        thermal::{jxs, nxs, ThermalAceOptions},
        AceTable,
    },
    endf::tape::Tape,
    thermr::mf7::parse_mf7,
};

const AL_MAT: i32 = 53;

fn al27_thermal() -> AceTable {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/tsl-013_Al_027-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).unwrap()).unwrap();
    let mf7 = parse_mf7(&tape, AL_MAT).unwrap();

    // Log-spaced inelastic incident-energy grid, 1e-4 → 4 eV.
    let ne = 40;
    let (elo, ehi) = (1.0e-4_f64, 4.0_f64);
    let grid: Vec<f64> = (0..ne)
        .map(|i| elo * (ehi / elo).powf(i as f64 / (ne - 1) as f64))
        .collect();

    let temp = mf7.incoherent_inelastic.as_ref().unwrap().temperature_k;
    let opts = ThermalAceOptions { n_outgoing: 16, n_cosines: 8, natom: 1.0 };
    AceTable::thermal_from_mf7(&mf7, temp, "al27", 0, &grid, opts).unwrap()
}

/// Minimal Type-1 parser: skip the 6 header/IZAW lines, read NXS(16)+JXS(32)
/// (6 lines of 8 ints), then the XSS values.
fn parse_type1(text: &str) -> (Vec<i32>, Vec<i32>, Vec<f64>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut ints: Vec<i32> = Vec::new();
    for line in &lines[6..12] {
        for tok in line.split_whitespace() {
            ints.push(tok.parse().unwrap());
        }
    }
    let mut xss: Vec<f64> = Vec::new();
    for line in &lines[12..] {
        for tok in line.split_whitespace() {
            xss.push(tok.parse().unwrap());
        }
    }
    (ints[..16].to_vec(), ints[16..].to_vec(), xss)
}

#[test]
fn thermal_table_header_and_dimensions() {
    let ace = al27_thermal();
    assert!(ace.zaid.starts_with("al27") && ace.zaid.ends_with('t'), "ZAID {}", ace.zaid);
    assert_eq!(ace.nxs[nxs::IDPNI], 3, "S(α,β) inelastic mode");
    assert_eq!(ace.nxs[nxs::NIL], 7, "NIL = nang − 1 = 7");
    assert_eq!(ace.nxs[nxs::NIEB], 16, "16 outgoing energies");
    assert_eq!(ace.nxs[nxs::IFENG], 0, "equiprobable form");
    assert_eq!(ace.nxs[nxs::IDPNC], 4, "Al has coherent elastic");
    assert_eq!(ace.nxs[nxs::NCL], -1);
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
    assert_eq!(ace.jxs[jxs::ITIE], 1);
    assert!(ace.jxs[jxs::ITCE] > 0 && ace.jxs[jxs::ITCX] > 0, "Bragg blocks present");
}

#[test]
fn inelastic_blocks_are_consistent() {
    let ace = al27_thermal();
    let nei = ace.xss[(ace.jxs[jxs::ITIE] - 1) as usize] as usize;
    assert_eq!(nei, 40, "NEI incident energies");

    // ITIE energies ascending; ITIX cross sections non-negative.
    let e0 = ace.jxs[jxs::ITIE] as usize; // 1-based
    let energies = &ace.xss[e0..e0 + nei];
    assert!(energies.windows(2).all(|w| w[1] >= w[0]), "E_inc ascending");

    let x0 = (ace.jxs[jxs::ITIX] - 1) as usize;
    let xs = &ace.xss[x0..x0 + nei];
    assert!(xs.iter().all(|&s| s >= 0.0), "σ_inel ≥ 0");
    assert!(xs.iter().any(|&s| s > 0.0), "σ_inel not all zero");

    // ITXE: NEI × NIEB × (1 + nang) words of E'/cosines. Cosines ∈ [−1, 1].
    let nieb = ace.nxs[nxs::NIEB] as usize;
    let nang = (ace.nxs[nxs::NIL] + 1) as usize;
    let itxe = (ace.jxs[jxs::ITXE] - 1) as usize;
    let expect = nei * nieb * (1 + nang);
    assert!(itxe + expect <= ace.xss.len(), "ITXE fits");
    for i in 0..expect {
        let v = ace.xss[itxe + i];
        let pos_in_bin = i % (1 + nang);
        if pos_in_bin != 0 {
            assert!((-1.0..=1.0).contains(&v), "cosine ∈ [−1,1], got {v}");
        } else {
            assert!(v >= 0.0, "E' ≥ 0");
        }
    }
}

#[test]
fn coherent_elastic_bragg_block_is_monotone() {
    let ace = al27_thermal();
    let itce = (ace.jxs[jxs::ITCE] - 1) as usize;
    let nee = ace.xss[itce] as usize;
    assert!(nee > 50, "Al has many Bragg edges");
    let e = &ace.xss[itce + 1..itce + 1 + nee];
    let itcx = (ace.jxs[jxs::ITCX] - 1) as usize;
    let p = &ace.xss[itcx..itcx + nee];
    assert!(e.windows(2).all(|w| w[1] >= w[0]), "Bragg energies ascending");
    assert!(p.windows(2).all(|w| w[1] >= w[0] - 1e-12), "cumulative S non-decreasing");
}

#[test]
fn thermal_table_roundtrips_through_file() {
    let ace = al27_thermal();
    let mut buf: Vec<u8> = Vec::new();
    ace.write_to(&mut buf).unwrap();
    let (rnxs, rjxs, rxss) = parse_type1(&String::from_utf8(buf).unwrap());
    assert_eq!(rnxs, ace.nxs.to_vec(), "NXS round-trip");
    assert_eq!(rjxs, ace.jxs.to_vec(), "JXS round-trip");
    assert_eq!(rxss.len(), ace.xss.len(), "XSS length");
    for (i, (&got, &want)) in rxss.iter().zip(ace.xss.iter()).enumerate() {
        let tol = if ace.xss_is_int[i] { 0.0 } else { 1e-8 * want.abs().max(1.0) };
        assert!((got - want).abs() <= tol, "xss[{i}]: got {got}, want {want}");
    }
}
