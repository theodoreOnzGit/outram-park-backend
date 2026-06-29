//! RECONR Phase 2b integration tests: Ar-37 (TENDL-2023 evaluation).
//!
//! Fixture: `tests/resources/n-018_Ar_37-tendl2023.endf`
//! Source:  NJOY2016 test suite (TENDL-2023, MAT=1828)
//! Size:    47 257 lines.
//!
//! Ar-37 is the canonical Phase 2b test nuclide:
//! - Non-fissile, so σ_fis = 0 everywhere.
//! - LRU=1, LRF=2 (MLBW) resolved resonances from 1e-5 eV to 4268 eV.
//! - Only 7 resonances (5 s-wave + 2 d-wave); resonances are widely spaced so
//!   SLBW ≈ MLBW (interference between levels is negligible).
//! - Three physical resonances (positive energy, s-wave):
//!     E_r = 1540 eV  (J=2, Γ_n=20.36 eV, Γ_γ=1.0 eV)
//!     E_r = 2630 eV  (J=2, Γ_n=29.72 eV, Γ_γ=1.0 eV)
//!     E_r = 4268 eV  (J=1, Γ_n=0.071 eV, Γ_γ=0.536 eV)
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test reconr_ar37 --release
//! ```

use std::fs::File;
use njoy_outram_park_fork::{
    endf::tape::Tape,
    interface::NuclearDataLibrary,
    reconr::{reconr, mf1, mf2, ReconrConfig},
    MtReaction,
};
use uom::si::energy::electronvolt;
use uom::si::area::barn;

const MAT: i32 = 1828; // Ar-37

fn ar37_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/n-018_Ar_37-tendl2023.endf");
    p
}

fn load_tape() -> Tape {
    Tape::read(File::open(ar37_path()).expect("Ar-37 fixture missing"))
        .expect("Ar-37 tape parse failed")
}

fn default_config() -> ReconrConfig {
    ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 }
}

// ── MF=1 header ───────────────────────────────────────────────────────────────

#[test]
fn mf1_za_and_awr() {
    let tape = load_tape();
    let sec = tape.section(MAT, 1, 451).expect("MF=1/MT=451 missing");
    let info = mf1::parse_material_info(sec).expect("MF=1 parse");
    // ZA = 18×1000 + 37 = 18037
    assert!((info.za - 18037.0).abs() < 0.1, "ZA={}", info.za);
    // AWR ≈ 36.6 (Ar-37 / neutron mass)
    assert!(info.awr > 36.0 && info.awr < 37.5, "AWR={}", info.awr);
    // LRP=1 — has resolved resonance parameters
    assert_eq!(info.lrp, 1, "LRP");
    // LFI=0 — not fissile
    assert_eq!(info.lfi, 0, "LFI");
}

// ── MF=2 resonance parsing ────────────────────────────────────────────────────

#[test]
fn mf2_parses_without_error() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).expect("MF=2/MT=151 missing");
    mf2::parse_resonance_info(sec).expect("MF=2 parse failed");
}

#[test]
fn mf2_has_resolved_resonances() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    assert!(!info.has_no_resonances(), "Ar-37 must have resonances");
}

#[test]
fn mf2_resolved_range_energy_bounds() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();

    let resolved: Vec<_> = info.resolved_slbw_ranges().collect();
    assert!(!resolved.is_empty(), "no resolved ranges found");

    let r = &resolved[0];
    assert!((r.el - 1e-5).abs() < 1e-8, "EL={}", r.el);
    // EH ≈ 4268 eV (the last resolved resonance energy)
    assert!(r.eh > 4000.0 && r.eh < 5000.0, "EH={}", r.eh);
    assert_eq!(r.lru, 1, "LRU");
    assert_eq!(r.formalism, Some(mf2::ResonanceFormalism::Mlbw), "LRF");
}

#[test]
fn mf2_swave_has_5_resonances() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();

    let resolved: Vec<_> = info.resolved_slbw_ranges().collect();
    let swave = resolved[0].l_states.iter().find(|ls| ls.l == 0)
        .expect("no s-wave l-state found");
    // 5 s-wave resonances: ER = -3236, -681, 1540, 2630, 4268 eV
    assert_eq!(swave.resonances.len(), 5, "s-wave resonance count");
}

#[test]
fn mf2_swave_resonance_energies() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    let resolved: Vec<_> = info.resolved_slbw_ranges().collect();
    let swave = resolved[0].l_states.iter().find(|ls| ls.l == 0).unwrap();

    let energies: Vec<f64> = swave.resonances.iter().map(|r| r.er).collect();
    // Physical (positive) resonances at 1540, 2630, 4268 eV
    assert!(energies.iter().any(|&e| (e - 1540.0).abs() < 1.0), "missing 1540 eV resonance");
    assert!(energies.iter().any(|&e| (e - 2630.0).abs() < 1.0), "missing 2630 eV resonance");
    assert!(energies.iter().any(|&e| (e - 4268.0).abs() < 10.0), "missing 4268 eV resonance");
}

#[test]
fn mf2_dwave_has_2_resonances() {
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    let resolved: Vec<_> = info.resolved_slbw_ranges().collect();
    // d-wave (l=2) has 2 resonances
    let dwave = resolved[0].l_states.iter().find(|ls| ls.l == 2)
        .expect("no d-wave l-state found");
    assert_eq!(dwave.resonances.len(), 2, "d-wave resonance count");
}

#[test]
fn mf2_spi_and_ap() {
    // SPI = 3/2 (I = 1.5), AP = 0.338779 × 10⁻¹² cm
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    let info = mf2::parse_resonance_info(sec).unwrap();
    let r = &info.ranges[0];
    assert!((r.spi - 1.5).abs() < 0.01, "SPI={}", r.spi);
    assert!((r.ap - 0.338779).abs() < 1e-4, "AP={}", r.ap);
}

// ── SLBW unit physics ─────────────────────────────────────────────────────────

#[test]
fn slbw_ar37_resonance_peak_elastic() {
    use njoy_outram_park_fork::reconr::slbw::{channel_radius, eval_slbw_lstate};

    let awri = 36.64921;
    let spi  = 1.5;
    let ap   = 0.338779;
    let ra   = channel_radius(awri, 1, ap);  // NAPS=1
    // s-wave, only the 1540 eV resonance
    let res = [(1540.0_f64, 2.0, 21.36429, 20.36429, 1.0, 0.0)];

    let peak = eval_slbw_lstate(1540.0, &res, 0, spi, ap, awri, ra);
    // Peak elastic should be very large (~400–700 b range)
    assert!(peak.elastic > 100.0,
        "σ_el peak at 1540 eV should be > 100 b, got {:.1}", peak.elastic);

    // Capture should be much smaller (Γ_γ/Γ_tot ≈ 1/21 ≈ 5%)
    // σ_cap ≈ 50 b at peak (Γ_γ/Γ_tot ratio × peak elastic ≈ 1/21 × ~1000 b)
    assert!(peak.capture > 10.0 && peak.capture < 150.0,
        "σ_cap at 1540 eV peak: {:.2}", peak.capture);

    // Fission = 0 (non-fissile)
    assert_eq!(peak.fission, 0.0, "Ar-37 has no fission");
}

#[test]
fn slbw_ar37_resonance_drops_away_from_peak() {
    use njoy_outram_park_fork::reconr::slbw::{channel_radius, eval_slbw_lstate};

    let awri = 36.64921;
    let spi  = 1.5;
    let ap   = 0.338779;
    let ra   = channel_radius(awri, 1, ap);
    let res = [(1540.0_f64, 2.0, 21.36429, 20.36429, 1.0, 0.0)];

    let peak = eval_slbw_lstate(1540.0, &res, 0, spi, ap, awri, ra);
    let far  = eval_slbw_lstate(100.0,  &res, 0, spi, ap, awri, ra);

    // 100 eV is ~85 half-widths below the peak; cross section should be tiny
    assert!(far.elastic < peak.elastic / 100.0,
        "σ_el(100 eV) should be < 1% of peak, got {:.3} vs {:.1}", far.elastic, peak.elastic);
}

// ── Full RECONR run ───────────────────────────────────────────────────────────

#[test]
fn reconr_runs_without_error() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).expect("reconr failed for Ar-37");
    assert!((result.material.za - 18037.0).abs() < 0.1, "ZA={}", result.material.za);
}

#[test]
fn reconr_produces_elastic_section() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    let elastic = result.sections.iter().find(|s| s.mt == MtReaction::Mt2Elastic)
        .expect("no elastic scattering section");
    assert!(!elastic.pairs.is_empty(), "MT=2 is empty");
}

#[test]
fn reconr_all_xs_nonnegative() {
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();
    for sec in &result.sections {
        for &(e, sigma) in &sec.pairs {
            assert!(sigma >= -0.01, // small negative tolerance for floating-point
                "MT={} σ={:.4} at E={:.3} eV is unexpectedly negative", sec.mt, sigma, e);
        }
    }
}

#[test]
fn reconr_resonance_peak_visible() {
    // After RECONR, the elastic cross section at 1540 eV should show the
    // resonance peak — it should be significantly larger than the off-resonance value.
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    let at_peak = result.eval_mt(MtReaction::Mt2Elastic, 1540.0);
    let off_peak = result.eval_mt(MtReaction::Mt2Elastic, 500.0);

    assert!(at_peak > off_peak,
        "σ_el(1540 eV) = {:.2} b should exceed σ_el(500 eV) = {:.2} b",
        at_peak, off_peak);
}

#[test]
fn reconr_capture_peak_at_resonance() {
    // Capture cross section should show peaks at resonance energies.
    let tape = load_tape();
    let result = reconr(&tape, &default_config()).unwrap();

    let at_peak  = result.eval_mt(MtReaction::Mt102Capture, 1540.0);
    let off_peak = result.eval_mt(MtReaction::Mt102Capture, 500.0);

    assert!(at_peak > off_peak,
        "σ_cap(1540 eV) = {:.3} b should exceed σ_cap(500 eV) = {:.3} b",
        at_peak, off_peak);
}

// ── NuclearDataLibrary OOP API ────────────────────────────────────────────────

#[test]
fn library_from_file_and_reconstruct() {
    let lib = NuclearDataLibrary::from_file(ar37_path(), MAT)
        .expect("from_file failed")
        .reconstruct(0.001)
        .expect("reconstruct failed");

    assert_eq!(lib.mat(), MAT);
    assert!((lib.za().unwrap() - 18037.0).abs() < 0.1, "ZA={:?}", lib.za());
}

#[test]
fn library_elastic_xs_uom_typed() {
    let lib = NuclearDataLibrary::from_file(ar37_path(), MAT)
        .expect("from_file")
        .reconstruct(0.001)
        .expect("reconstruct");

    let e = uom::si::f64::Energy::new::<electronvolt>(1540.0);
    let xs = lib.elastic_xs(e);

    // At the 1540 eV resonance, elastic XS should be substantial (> 1 b)
    assert!(xs.get::<barn>() > 1.0,
        "σ_el(1540 eV) = {:.2} b, expected > 1 b", xs.get::<barn>());
}

#[test]
fn library_fission_xs_is_zero() {
    // Ar-37 is non-fissile
    let lib = NuclearDataLibrary::from_file(ar37_path(), MAT)
        .expect("from_file")
        .reconstruct(0.001)
        .expect("reconstruct");

    let e = uom::si::f64::Energy::new::<electronvolt>(1540.0);
    let xs = lib.fission_xs(e);
    assert!(xs.get::<barn>().abs() < 0.01,
        "σ_fis(Ar-37) should be ≈ 0, got {:.4} b", xs.get::<barn>());
}

#[test]
fn library_broaden_records_temperature() {
    use uom::si::thermodynamic_temperature::kelvin;

    let lib = NuclearDataLibrary::from_file(ar37_path(), MAT)
        .expect("from_file")
        .reconstruct(0.001)
        .expect("reconstruct")
        .broaden(uom::si::f64::ThermodynamicTemperature::new::<kelvin>(293.6))
        .expect("broaden");

    assert!((lib.temperature().get::<kelvin>() - 293.6).abs() < 0.01);
}

#[test]
fn library_to_continuous_energy_data() {
    let lib = NuclearDataLibrary::from_file(ar37_path(), MAT)
        .expect("from_file")
        .reconstruct(0.001)
        .expect("reconstruct");

    let ced = lib.to_continuous_energy_data().expect("to_continuous_energy_data");

    assert!((ced.za - 18037.0).abs() < 0.1);
    assert!(!ced.reactions.is_empty(), "no reactions in ContinuousEnergyData");

    let elastic = ced.reaction(MtReaction::Mt2Elastic).expect("no elastic reaction in CED");
    assert!(!elastic.data.is_empty(), "empty elastic data");
}
