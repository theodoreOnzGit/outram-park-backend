//! RECONR Phase 2c integration tests: U-235 (ENDF/B-VIII.0 evaluation).
//!
//! Fixture: `reference-data/endf/n-092_U_235-ENDF8.0.endf`
//! Source:  NJOY2016 test suite (ENDF/B-VIII.0, MAT=9228)
//!
//! U-235 is the canonical Phase 2c (Reich-Moore, LRF=3) test nuclide:
//! - Fissile (LFI=1) — exercises the two fission channels (GFA, GFB) and the
//!   3×3 complex R-matrix inversion path in `reconr::rm`.
//! - Resolved resonance region: LRU=1, LRF=3 from 1e-5 eV to 2250 eV, a single
//!   s-wave l-state holding 3195 resonances.
//! - Unresolved region above 2250 eV (LRU=2) — not reconstructed (Phase 2d).
//!
//! The decisive check is physical: after reconstruction, the 2200 m/s
//! (0.0253 eV) thermal cross sections must match the textbook U-235 values
//!   σ_fission ≈ 585 b, σ_capture ≈ 99 b, σ_elastic ≈ 15 b, σ_total ≈ 699 b.
//! Reproducing these from raw resonance parameters validates the entire
//! Reich-Moore evaluation + background-merge pipeline end to end.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test reconr_u235 --release
//! ```

use std::fs::File;
use njoy_outram_park_fork::{
    endf::tape::Tape,
    interface::NuclearDataLibrary,
    reconr::{reconr, mf1, mf2, ReconrConfig, ResonanceFormalism},
    MtReaction,
};
use uom::si::{energy::electronvolt, area::barn};

const MAT: i32 = 9228; // U-235

fn u235_path() -> std::path::PathBuf {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    p
}

fn load_tape() -> Tape {
    Tape::read(File::open(u235_path()).expect("U-235 fixture missing"))
        .expect("U-235 tape parse failed")
}

fn default_config() -> ReconrConfig {
    ReconrConfig {
        mat: MAT,
        tolerance: 0.001,
        temperature: 0.0,
    }
}

// ── MF=1 header ───────────────────────────────────────────────────────────────

#[test]
fn mf1_za_awr_and_fissile_flag() {
    let tape = load_tape();
    let sec = tape.section(MAT, 1, 451).expect("MF=1/MT=451 missing");
    let info = mf1::parse_material_info(sec).expect("MF=1 parse");
    // ZA = 92×1000 + 235
    assert!((info.za - 92235.0).abs() < 0.1, "ZA={}", info.za);
    // AWR ≈ 233.0248 (U-235 / neutron mass)
    assert!((info.awr - 233.0248).abs() < 0.01, "AWR={}", info.awr);
    // LRP=1 — has resolved resonance parameters
    assert_eq!(info.lrp, 1, "LRP");
    // LFI=1 — fissile (the key difference from Ar-37)
    assert_eq!(info.lfi, 1, "LFI (fissile flag)");
}

// ── MF=2 Reich-Moore parsing ───────────────────────────────────────────────────

#[test]
fn mf2_parses_reich_moore_without_error() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).expect("MF=2/MT=151 missing");
    mf2::parse_resonance_info(sec).expect("Reich-Moore (LRF=3) parse failed");
}

#[test]
fn mf2_resolved_range_is_reich_moore() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();

    let rm_ranges: Vec<_> = info.resolved_rm_ranges().collect();
    assert_eq!(rm_ranges.len(), 1, "expected exactly one resolved RM range");

    let r = rm_ranges[0];
    assert_eq!(r.lru, 1, "LRU");
    assert_eq!(r.formalism, Some(ResonanceFormalism::ReichMoore), "LRF=3");
    // RRR spans 1e-5 eV → 2250 eV
    assert!((r.el - 1e-5).abs() < 1e-8, "EL={}", r.el);
    assert!((r.eh - 2250.0).abs() < 1.0, "EH={}", r.eh);
    // SPI = 7/2, AP ≈ 0.9602 (×10⁻¹² cm), NAPS=1
    assert!((r.spi - 3.5).abs() < 0.01, "SPI={}", r.spi);
    assert!((r.ap - 0.9602).abs() < 1e-3, "AP={}", r.ap);
    assert_eq!(r.naps, 1, "NAPS");
    // The SLBW l-state list must be empty for an LRF=3 range.
    assert!(
        r.l_states.is_empty(),
        "LRF=3 range must have no SLBW l-states"
    );
}

#[test]
fn mf2_reich_moore_resonance_inventory() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    let range = info.resolved_rm_ranges().next().unwrap();

    // Single s-wave l-state holding all 3195 resonances.
    assert_eq!(range.rm_l_states.len(), 1, "expected one RM l-state");
    let ls = &range.rm_l_states[0];
    assert_eq!(ls.l, 0, "s-wave");
    assert!((ls.awri - 233.0248).abs() < 0.01, "AWRI={}", ls.awri);
    assert_eq!(ls.resonances.len(), 3195, "resonance count");

    // First resonance (negative-energy bound level) read in ER AJ GN GG GFA GFB order.
    let r0 = &ls.resonances[0];
    assert!((r0.er - (-722.2152)).abs() < 0.01, "ER₀={}", r0.er);
    assert!((r0.aj - 4.0).abs() < 0.01, "AJ₀={}", r0.aj);
    assert!((r0.gn - 4.0221e-4).abs() < 1e-7, "GN₀={}", r0.gn);
    assert!((r0.gg - 3.6104e-2).abs() < 1e-5, "GG₀={}", r0.gg);
    // Two distinct fission channels — the hallmark of LRF=3.
    assert!(
        r0.gfa.abs() > 1e-3,
        "GFA₀ should be non-zero, got {}",
        r0.gfa
    );
    assert!(
        r0.gfb.abs() > 1e-3,
        "GFB₀ should be non-zero, got {}",
        r0.gfb
    );
}

#[test]
fn mf2_lowest_positive_resonances() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    let ls = &info.resolved_rm_ranges().next().unwrap().rm_l_states[0];

    let mut pos: Vec<f64> = ls
        .resonances
        .iter()
        .map(|r| r.er)
        .filter(|&e| e > 0.0)
        .collect();
    pos.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // The famous low-lying U-235 resonances: ~0.2684 eV and ~1.134 eV.
    assert!(
        pos.iter().any(|&e| (e - 0.2684).abs() < 1e-3),
        "missing ~0.268 eV resonance"
    );
    assert!(
        pos.iter().any(|&e| (e - 1.1342).abs() < 1e-3),
        "missing ~1.134 eV resonance"
    );
}

// ── Full RECONR run ─────────────────────────────────────────────────────────────

#[test]
fn reconr_runs_without_error() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).expect("reconr failed for U-235");
    assert!(
        (result.material.za - 92235.0).abs() < 0.1,
        "ZA={}",
        result.material.za
    );
}

#[test]
fn reconr_thermal_cross_sections_match_accepted_values() {
    // The decisive Phase 2c check: reconstructed 2200 m/s (0.0253 eV) values
    // must match the accepted U-235 thermal cross sections. These are produced
    // purely from the Reich-Moore resonance parameters (the MF=3 background is
    // zero throughout the resolved region), so agreement validates the whole
    // LRF=3 evaluation + 3×3 complex inversion path.
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    let fis = result.eval_mt(MtReaction::Mt18Fission, 0.0253);
    let cap = result.eval_mt(MtReaction::Mt102Capture, 0.0253);
    let el = result.eval_mt(MtReaction::Mt2Elastic, 0.0253);
    let tot = result.eval_mt(MtReaction::Mt1Total, 0.0253);

    assert!(
        (fis - 585.0).abs() < 25.0,
        "σ_fission(0.0253 eV) = {fis:.1} b, expected ≈585"
    );
    assert!(
        (cap - 99.0).abs() < 10.0,
        "σ_capture(0.0253 eV) = {cap:.1} b, expected ≈99"
    );
    assert!(
        (el - 15.0).abs() < 3.0,
        "σ_elastic(0.0253 eV) = {el:.1} b, expected ≈15"
    );
    assert!(
        (tot - 699.0).abs() < 25.0,
        "σ_total(0.0253 eV) = {tot:.1} b, expected ≈699"
    );

    // Internal consistency: total ≈ elastic + fission + capture (+ small others).
    assert!(
        tot >= el + fis + cap - 1.0,
        "total {tot:.1} < el+fis+cap = {:.1}",
        el + fis + cap
    );
}

#[test]
fn reconr_fission_is_substantial_and_finite() {
    // U-235 is fissile: the reconstructed fission must be present, non-trivial,
    // and physically bounded (resonance peaks reach tens of thousands of barns,
    // not millions — a regression guard against the merge-accumulation bug).
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    let fsec = result
        .sections
        .iter()
        .find(|s| s.mt == MtReaction::Mt18Fission)
        .expect("no fission section");
    let max_fis = fsec.pairs.iter().map(|&(_, s)| s).fold(0.0_f64, f64::max);
    assert!(
        max_fis > 1.0e3,
        "peak fission should exceed 1000 b, got {max_fis:.1}"
    );
    assert!(
        max_fis < 1.0e5,
        "peak fission unphysically large ({max_fis:.1} b) — merge bug?"
    );
    for &(e, s) in &fsec.pairs {
        assert!(
            s.is_finite() && s >= 0.0,
            "σ_fission={s} at E={e} is invalid"
        );
    }
}

#[test]
fn reconr_all_xs_nonnegative_and_finite() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    for sec in &result.sections {
        for &(e, sigma) in &sec.pairs {
            assert!(
                sigma.is_finite() && sigma >= -0.01,
                "MT={} σ={:.4} at E={:.4} eV is invalid",
                sec.mt,
                sigma,
                e
            );
        }
    }
}

#[test]
fn reconr_resonance_peaks_above_thermal() {
    // The 1/v thermal region should sit well below the strongest resonance peaks.
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    let thermal = result.eval_mt(MtReaction::Mt18Fission, 0.0253);
    let fsec = result
        .sections
        .iter()
        .find(|s| s.mt == MtReaction::Mt18Fission)
        .unwrap();
    let peak = fsec.pairs.iter().map(|&(_, s)| s).fold(0.0_f64, f64::max);
    assert!(
        peak > 10.0 * thermal,
        "peak fission {peak:.1} should dwarf thermal {thermal:.1}"
    );
}

// ── NuclearDataLibrary OOP API ──────────────────────────────────────────────────

#[test]
fn library_reconstructs_and_reports_fissile_xs() {
    let lib = NuclearDataLibrary::from_file(u235_path(), MAT)
        .expect("from_file failed")
        .reconstruct(0.001)
        .expect("reconstruct failed");

    assert_eq!(lib.mat(), MAT);
    assert!(
        (lib.za().unwrap() - 92235.0).abs() < 0.1,
        "ZA={:?}",
        lib.za()
    );

    // Unlike Ar-37, U-235 fission must be large at thermal.
    let e = uom::si::f64::Energy::new::<electronvolt>(0.0253);
    let fis = lib.fission_xs(e).get::<barn>();
    assert!(
        fis > 500.0 && fis < 650.0,
        "σ_fis(thermal) = {fis:.1} b, expected ≈585"
    );
}
