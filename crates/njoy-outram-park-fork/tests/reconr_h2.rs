//! RECONR Phase 2a integration tests: H-2 (deuterium) evaluation.
//!
//! Fixture: `tests/resources/n-001_H_002-ENDF8.0.endf`
//! Source:  NJOY2016 test suite (ENDF/B-VIII.0, MAT=128)
//! Size:    1715 lines, 17 sections.
//!
//! H-2 is the canonical LRU=0 material: no resolved resonance region, purely
//! pointwise MF=3 cross sections. RECONR for H-2 is therefore a linearisation
//! pass — all MF=3 sections are already lin-lin in this evaluation, so the
//! output should be structurally identical to the input.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test reconr_h2 --release
//! ```

use std::fs::File;
use njoy_outram_park_fork::{
    endf::tape::Tape,
    reconr::{reconr, ReconrConfig, mf1, mf2},
};

const MAT: i32 = 128; // H-2 (deuterium)

fn h2_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/n-001_H_002-ENDF8.0.endf");
    p
}

fn load_tape() -> Tape {
    Tape::read(File::open(h2_path()).expect("H-2 fixture not found"))
        .expect("H-2 tape parse failed")
}

fn default_config() -> ReconrConfig {
    ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 }
}

// ── MF=1 header ───────────────────────────────────────────────────────────────

#[test]
fn mf1_za_awr() {
    let tape = load_tape();
    let sec = tape.section(MAT, 1, 451).expect("MF=1 MT=451 missing");
    let info = mf1::parse_material_info(sec).expect("MF=1 parse failed");
    // H-2: ZA = 1×1000 + 2 = 1002
    assert!((info.za - 1002.0).abs() < 0.1, "ZA={}", info.za);
    // AWR ≈ 1.9968 (deuteron / neutron mass ratio)
    assert!((info.awr - 1.9968).abs() < 1e-4, "AWR={}", info.awr);
    // LRP=0 — no resonance parameters
    assert_eq!(info.lrp, 0, "LRP");
    // LFI=0 — not fissile
    assert_eq!(info.lfi, 0, "LFI");
}

#[test]
fn mf1_emax() {
    let tape = load_tape();
    let sec = tape.section(MAT, 1, 451).unwrap();
    let info = mf1::parse_material_info(sec).unwrap();
    // ENDF/B-VIII.0 H-2 goes to 1.5×10⁸ eV (150 MeV)
    assert!(info.emax > 1.0e7, "EMAX={} should be > 10 MeV", info.emax);
}

// ── MF=2/MT=151 resonance info ────────────────────────────────────────────────

#[test]
fn mf2_lru0_no_resonances() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).expect("MF=2 MT=151 missing");
    let info = mf2::parse_resonance_info(sec).expect("MF=2 parse failed");
    assert!(info.has_no_resonances(), "H-2 should have no resonances (LRU=0)");
}

#[test]
fn mf2_potential_scattering_radius() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    assert_eq!(info.ranges.len(), 1, "one energy range");
    let range = &info.ranges[0];
    // AP ≈ 0.51977 × 10⁻¹² cm (from ENDF file: 5.197700-1)
    assert!((range.ap - 0.51977).abs() < 1e-4, "AP={}", range.ap);
    // Energy range covers 1e-5 eV to 1e8 eV
    assert!((range.el - 1e-5).abs() < 1e-8, "EL={}", range.el);
    assert!((range.eh - 1e8).abs() < 1.0, "EH={}", range.eh);
}

// ── Full RECONR run ───────────────────────────────────────────────────────────

#[test]
fn reconr_runs_without_error() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).expect("reconr failed");
    assert!((result.material.za - 1002.0).abs() < 0.1, "ZA={}", result.material.za);
}

#[test]
fn reconr_produces_all_mf3_sections() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    // H-2 has MF=3: MT=1 (total), 2 (elastic), 3 (nonelastic), 16 (n,2n), 102 (capture)
    let mts: Vec<i32> = result.sections.iter().map(|s| s.mt).collect();
    for mt in [1, 2, 3, 16, 102] {
        assert!(mts.contains(&mt), "missing MF=3 MT={mt}");
    }
}

#[test]
fn reconr_all_xs_nonnegative() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    for sec in &result.sections {
        for &(e, sigma) in &sec.pairs {
            assert!(sigma >= 0.0, "MF=3 MT={} σ<0 at E={}", sec.mt, e);
        }
    }
}

#[test]
fn reconr_lin_lin_preserves_point_count() {
    // All H-2 MF=3 sections use INT=2 (lin-lin), so linearisation should
    // not add any points — output size equals input size.
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    // MT=1 (total): 178 tabulated points from the fixture
    let total = result.sections.iter().find(|s| s.mt == 1).unwrap();
    assert_eq!(total.pairs.len(), 178, "MT=1 should have 178 points (already lin-lin)");

    // MT=2 (elastic): also 178 points (confirmed from MF=3 MT=2 TAB1 header NP=178)
    let elastic = result.sections.iter().find(|s| s.mt == 2).unwrap();
    assert_eq!(elastic.pairs.len(), 178, "MT=2 should have 178 points (already lin-lin)");
}

#[test]
fn reconr_elastic_thermal_value() {
    // At E = 1e-5 eV, H-2 elastic XS ≈ 3.4 b (from the tabulated file value)
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    let elastic = result.sections.iter().find(|s| s.mt == 2).unwrap();
    let (e0, s0) = elastic.pairs[0];
    assert!((e0 - 1e-5).abs() < 1e-8, "first energy = {e0}");
    // From file: 1.000000-5  3.395000+0 → 3.395 b
    assert!((s0 - 3.395).abs() < 0.001, "σ_el(1e-5 eV) = {s0} b, expected ≈ 3.395 b");
}

#[test]
fn reconr_total_xs_energy_bounds() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    let total = result.sections.iter().find(|s| s.mt == 1).unwrap();
    let (e_first, _) = total.pairs[0];
    let &(e_last, _) = total.pairs.last().unwrap();
    assert!((e_first - 1e-5).abs() < 1e-8, "E_first={e_first}");
    // MT=1 spans to 30 MeV or beyond (H-2 evaluation goes to 150 MeV)
    assert!(e_last > 1e7, "E_last={e_last} should be > 10 MeV");
}

// ── SLBW unit tests (Phase 2b stub validation) ────────────────────────────────

#[test]
fn potential_scattering_h2_thermal() {
    use njoy_outram_park_fork::reconr::slbw::potential_scattering;
    // H-2: AWR=1.9968, AP=0.51977 (10⁻¹² cm)
    // At thermal energy (1e-5 eV) potential scattering should be ≈ 3-4 b
    let sigma = potential_scattering(1.0e-5, 1.9968, 0.51977);
    assert!(sigma > 2.5 && sigma < 5.0,
        "σ_pot(H-2, 1e-5 eV) = {sigma:.3} b, expected 3–4 b");
}
