//! Integration tests: parse the U-235 ENDF/B-VIII.0 evaluation.
//!
//! Fixture: `reference-data/endf/n-092_U_235-ENDF8.0.endf`
//! Source:  NJOY2016 test suite (ENDF/B-VIII.0, MAT=9228)
//! Size:    485,315 lines, 423 sections — the real-world stress test for
//!          the tape parser: resonance data (MF=2), dozens of cross-section
//!          sections (MF=3), angular + energy distributions (MF=4, MF=5),
//!          covariances (MF=31–35).
//!
//! These tests verify:
//! 1. The parser handles a large, complex neutron evaluation without error.
//! 2. Key nuclear physics values are read correctly from MF=3 cross sections.
//! 3. The fission cross section (MT=18) structure is intact.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test endf_u235 --release
//! ```

use std::fs::File;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::interp::eval_tab1;

const MAT: i32 = 9228; // U-235

fn u235_path() -> std::path::PathBuf {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    p
}

fn load_tape() -> Tape {
    Tape::read(File::open(u235_path()).expect("U-235 fixture not found"))
        .expect("U-235 parse failed")
}

// ── Section inventory ─────────────────────────────────────────────────────────

#[test]
fn tape_parses_large_file() {
    let tape = load_tape();
    // 423 physics sections (excludes SEND/FEND/MEND/TEND sentinels and TPID)
    assert_eq!(tape.len(), 423, "expected 423 sections, got {}", tape.len());
}

#[test]
fn key_sections_present() {
    let tape = load_tape();
    // MF=1 descriptive / nu
    assert!(
        tape.section(MAT, 1, 451).is_some(),
        "missing MF=1 MT=451 (general info)"
    );
    assert!(
        tape.section(MAT, 1, 452).is_some(),
        "missing MF=1 MT=452 (nubar total)"
    );
    assert!(
        tape.section(MAT, 1, 455).is_some(),
        "missing MF=1 MT=455 (nubar delayed)"
    );
    assert!(
        tape.section(MAT, 1, 456).is_some(),
        "missing MF=1 MT=456 (nubar prompt)"
    );
    // MF=2 resonance parameters
    assert!(
        tape.section(MAT, 2, 151).is_some(),
        "missing MF=2 MT=151 (resonance params)"
    );
    // MF=3 cross sections
    assert!(
        tape.section(MAT, 3, 1).is_some(),
        "missing MF=3 MT=1 (total XS)"
    );
    assert!(
        tape.section(MAT, 3, 2).is_some(),
        "missing MF=3 MT=2 (elastic XS)"
    );
    assert!(
        tape.section(MAT, 3, 18).is_some(),
        "missing MF=3 MT=18 (fission XS)"
    );
    assert!(
        tape.section(MAT, 3, 102).is_some(),
        "missing MF=3 MT=102 (capture XS)"
    );
}

// ── MF=1 MT=451: General information header ───────────────────────────────────

#[test]
fn general_info_za_awr() {
    let tape = load_tape();
    let sec = tape.section(MAT, 1, 451).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let cont = cur.read_cont().unwrap();
    // ZA = 92235 (Z=92 × 1000 + A=235)
    assert!((cont.c1 - 92235.0).abs() < 0.1, "ZA={}", cont.c1);
    // AWR ≈ 233.0248 (atomic weight relative to neutron mass)
    assert!((cont.c2 - 233.0248).abs() < 0.01, "AWR={}", cont.c2);
    // LIS=1 (ground state), LISO=1
    assert_eq!(cont.l1, 1, "LIS (isomer state flag)");
}

// ── MF=3 MT=2: Elastic cross section ─────────────────────────────────────────

#[test]
fn elastic_xs_cont_header() {
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let cont = cur.read_cont().unwrap();
    assert!((cont.c1 - 92235.0).abs() < 0.1, "ZA={}", cont.c1);
    assert!((cont.c2 - 233.0248).abs() < 0.01, "AWR={}", cont.c2);
}

#[test]
fn elastic_xs_tab1_structure() {
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    // Elastic XS has 111 points, 1 interpolation region (lin-lin = 2)
    assert_eq!(tab1.pairs.len(), 111, "NP=111 elastic points");
    assert_eq!(tab1.head.n1, 1, "NR=1 interp region");
    assert_eq!(tab1.interp[0].1, 2, "interp law lin-lin");
}

#[test]
fn elastic_xs_energy_bounds() {
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    // From file: first energy 1e-5 eV, last energy 3e7 eV (30 MeV)
    let (e_min, _) = tab1.pairs[0];
    let &(e_max, _) = tab1.pairs.last().unwrap();
    assert!((e_min - 1e-5).abs() < 1e-8, "E_min={}", e_min);
    assert!((e_max - 3.0e7).abs() < 1.0, "E_max={}", e_max);
}

#[test]
fn elastic_xs_thermal_is_zero() {
    // At 1e-5 eV (thermal), σ_el = 0 (resonance region not yet reconstructed)
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    let (_, sigma_thermal) = tab1.pairs[0];
    assert_eq!(sigma_thermal, 0.0, "σ_el(1e-5 eV) = 0 before RECONR");
}

// ── MF=3 MT=18: Fission cross section ────────────────────────────────────────

#[test]
fn fission_xs_tab1_reads_cleanly() {
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 18).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    // Fission XS table is non-empty and spans the full energy range
    assert!(
        tab1.pairs.len() > 10,
        "fission table should have many points"
    );
    let (e_first, _) = tab1.pairs[0];
    let &(e_last, _) = tab1.pairs.last().unwrap();
    assert!(e_first < 1.0, "fission starts at thermal energies");
    assert!(e_last > 1.0e6, "fission extends to fast energies");
}

#[test]
fn fission_xs_positive_throughout() {
    // All fission cross section values must be ≥ 0
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 18).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    for &(e, sigma) in &tab1.pairs {
        assert!(sigma >= 0.0, "σ_fission < 0 at E={}", e);
    }
}

#[test]
fn fission_xs_interpolation_at_thermal() {
    // Interpolate at E=0.0253 eV (room temperature / thermal).
    // U-235 fission XS at thermal is famously large (≈583 barns from RECONR;
    // the unprocessed MF=3 value here is 0 since resonances aren't reconstructed,
    // but the nearest tabulated point near thermal should give 0 or a small value).
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 18).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    let sigma = eval_tab1(0.0253, &tab1.interp, &tab1.pairs).unwrap();
    // Before reconstruction σ=0 in the resonance region is expected
    assert!(
        sigma >= 0.0,
        "σ_fission(0.0253 eV) must be non-negative, got {}",
        sigma
    );
}

// ── MF=3 MT=1: Total cross section ───────────────────────────────────────────

#[test]
fn total_xs_tab1_reads_cleanly() {
    let tape = load_tape();
    let sec = tape.section(MAT, 3, 1).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    assert!(tab1.pairs.len() > 10);
    for &(e, sigma) in &tab1.pairs {
        assert!(sigma >= 0.0, "σ_total < 0 at E={}", e);
    }
}

// ── MF=2 MT=151: Resonance parameters (just structure check) ─────────────────

#[test]
fn resonance_section_exists_and_nonempty() {
    // MF=2 MT=151 is the resonance parameter section — large and complex.
    // Phase 1 only checks it loads without error and has data rows.
    // RECONR (Phase 2) will actually parse the resonance sub-sections.
    let tape = load_tape();
    let sec = tape.section(MAT, 2, 151).unwrap();
    assert!(!sec.rows.is_empty(), "MF=2 MT=151 has no data rows");
    // U-235 has multiple resonance ranges (RRR + URR); the section is large
    assert!(
        sec.rows.len() > 100,
        "expected many resonance rows, got {}",
        sec.rows.len()
    );
}
