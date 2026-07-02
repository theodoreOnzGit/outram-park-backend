//! Thermal S(α,β) ACE table for **H in ZrH** — the incoherent-*elastic* path.
//!
//! Fixture: `tests/resources/tsl-HinZrH-ENDF8.0.endf` (MAT=7). Unlike Al-27
//! (coherent Bragg elastic), H(ZrH) is a hydrogenous solid whose bound protons
//! scatter *incoherently* elastically: MF=7/MT=2 with `LTHR=2` (a bound cross
//! section `σ_b` + the Debye-Waller integral `W'(T)`), plus MF=7/MT=4 incoherent
//! inelastic S(α,β). It therefore exercises the `IDPNC=3` elastic branch —
//! ITCE/ITCX/ITCA — that Al-27 never touches.
//!
//! Run with:
//! ```bash
//! crates/njoy-outram-park-fork/scripts/test.sh --test thermal_ace_zrh
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    ace::{
        thermal::{jxs, nxs, ThermalAceOptions},
        AceTable,
    },
    endf::tape::Tape,
    thermal::mf7::{parse_mf7, Mf7},
};

/// ENDF/B-VIII.0 thermal MAT number for H(ZrH).
const HZRH_MAT: i32 = 7;
const NANG: usize = 8; // equally-probable cosines (elastic NEA and inelastic nang)

fn hzrh_mf7() -> Mf7 {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/tsl-HinZrH-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).unwrap()).unwrap();
    parse_mf7(&tape, HZRH_MAT).unwrap()
}

fn hzrh_thermal() -> AceTable {
    let mf7 = hzrh_mf7();
    // Log-spaced inelastic incident-energy grid, 1e-4 → 4 eV (also the incoherent-
    // elastic energy grid).
    let ne = 40;
    let (elo, ehi) = (1.0e-4_f64, 4.0_f64);
    let grid: Vec<f64> = (0..ne)
        .map(|i| elo * (ehi / elo).powf(i as f64 / (ne - 1) as f64))
        .collect();
    let temp = mf7.incoherent_inelastic.as_ref().unwrap().temperature_k;
    let opts = ThermalAceOptions { n_outgoing: 16, n_cosines: NANG, natom: 1.0 };
    AceTable::thermal_from_mf7(&mf7, temp, "hzrh", 0, &grid, opts).unwrap()
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
fn hzrh_has_incoherent_elastic_and_inelastic_but_no_coherent() {
    let mf7 = hzrh_mf7();
    assert!(mf7.coherent_elastic.is_none(), "H(ZrH) is not crystalline-coherent");
    assert!(mf7.incoherent_inelastic.is_some(), "H(ZrH) has S(α,β)");
    let ie = mf7.incoherent_elastic.expect("H(ZrH) has incoherent elastic (LTHR=2)");
    // SB ≈ 81.98 barn; 8 temperatures 296–1200 K; W' ascends with T.
    assert!((ie.sb - 81.98).abs() < 0.1, "σ_b ≈ 81.98 barn, got {}", ie.sb);
    assert_eq!(ie.temperature_k, 296.0, "T₀ = lowest tabulated temperature");
    assert_eq!(ie.wp_of_t.len(), 8, "8 (T, W') points");
    assert!(ie.wp_of_t.windows(2).all(|w| w[1].0 > w[0].0), "T ascending");
    assert!(ie.wp_of_t.windows(2).all(|w| w[1].1 >= w[0].1), "W'(T) non-decreasing");
}

#[test]
fn incoherent_elastic_cross_section_physics() {
    let ie = hzrh_mf7().incoherent_elastic.unwrap();
    let t = 296.0;
    // σ decreases with energy (the (1−e^{−4EW'})/(2EW') factor falls).
    let lo = ie.cross_section(1.0e-3, t, 1.0);
    let hi = ie.cross_section(2.0, t, 1.0);
    assert!(lo > 0.0 && hi >= 0.0, "σ ≥ 0");
    assert!(hi < lo, "σ falls with E: lo={lo}, hi={hi}");
    // Low-energy limit → σ_b/N (Debye-Waller factor → 1).
    assert!((lo - ie.sb).abs() / ie.sb < 0.05, "σ(E→0) → σ_b: {lo} vs {}", ie.sb);
}

#[test]
fn thermal_table_uses_idpnc3_elastic_branch() {
    let ace = hzrh_thermal();
    assert!(ace.zaid.starts_with("hzrh") && ace.zaid.ends_with('t'), "ZAID {}", ace.zaid);
    assert_eq!(ace.nxs[nxs::IDPNI], 3, "S(α,β) inelastic mode");
    assert_eq!(ace.nxs[nxs::IDPNC], 3, "incoherent elastic (no coherent) ⇒ IDPNC=3");
    assert_eq!(ace.nxs[nxs::NCL], (NANG - 1) as i32, "NCL = NEA − 1");
    assert_eq!(ace.nxs[nxs::NCLI], 0, "no secondary elastic block");
    // Primary elastic slots populated; secondary (mixed-case) slots empty.
    assert!(ace.jxs[jxs::ITCE] > 0 && ace.jxs[jxs::ITCX] > 0 && ace.jxs[jxs::ITCA] > 0);
    assert_eq!(ace.jxs[jxs::ITCEI], 0);
    assert_eq!(ace.jxs[jxs::ITCXI], 0);
    assert_eq!(ace.jxs[jxs::ITCAI], 0);
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
}

#[test]
fn incoherent_elastic_block_layout_is_consistent() {
    let ace = hzrh_thermal();
    let nee = ace.xss[(ace.jxs[jxs::ITCE] - 1) as usize] as usize;
    assert_eq!(nee, 40, "NEE = incident-energy grid length");

    // ITCE energies ascending.
    let e0 = ace.jxs[jxs::ITCE] as usize; // 1-based → first energy
    let energies = &ace.xss[e0..e0 + nee];
    assert!(energies.windows(2).all(|w| w[1] >= w[0]), "elastic E ascending");

    // ITCX cross sections: non-negative and non-increasing in E.
    let x0 = (ace.jxs[jxs::ITCX] - 1) as usize;
    let xs = &ace.xss[x0..x0 + nee];
    assert!(xs.iter().all(|&s| s >= 0.0), "σ_ie ≥ 0");
    assert!(xs.iter().any(|&s| s > 0.0), "σ_ie not all zero");
    assert!(xs.windows(2).all(|w| w[1] <= w[0] + 1e-12), "σ_ie non-increasing");

    // ITCA: NEE × NEA equally-probable cosines, each in [−1, 1].
    let nea = (ace.nxs[nxs::NCL] + 1) as usize;
    assert_eq!(nea, NANG);
    let a0 = (ace.jxs[jxs::ITCA] - 1) as usize;
    let angles = &ace.xss[a0..a0 + nee * nea];
    assert!(angles.iter().all(|&m| (-1.0..=1.0).contains(&m)), "cosines ∈ [−1,1]");
    // Each energy's NEA cosines are sorted ascending (equal-probability bins).
    for row in angles.chunks(nea) {
        assert!(row.windows(2).all(|w| w[1] >= w[0]), "cosines ascending per energy");
    }
}

#[test]
fn thermal_table_roundtrips_through_file() {
    let ace = hzrh_thermal();
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
