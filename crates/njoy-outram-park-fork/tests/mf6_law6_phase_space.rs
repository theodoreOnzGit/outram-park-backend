//! **MF=6 LAW=6 (`n`-body phase space) against upstream's own closed form.**
//!
//! Found 2026-09-16 by the interpolation survey, which — once it was made to
//! count its own skips — reported two tapes it could not read at all: **H-2 and
//! Be-9 MT=16 use MF=6 LAW=6/LAW=7, not LAW=1**. Both had been falling back to
//! the Weisskopf evaporation stand-in with nothing recording that they were.
//!
//! H-2 is the LAW=6 case. This test covers it; **LAW=7 (Be-9) remains
//! unported** and is recorded as such rather than quietly approximated.
//!
//! # What is compared
//!
//! `ContinuumEmission::from_endf_mf6` now converts LAW=6 into the tabulated
//! form the samplers already consume, so the thing to verify is that the
//! converted table reproduces the **analytic distribution upstream evaluates**:
//! NJOY's `f6psp` (`groupr.f90:12643-12687`),
//!
//! ```text
//! f1     = (APSX - AWP) / APSX
//! f2     = AWR / (AWR + 1)
//! E'_max = f1 * (f2 * E + Q)
//! f(E')  = C_n * sqrt(E') * (E'_max - E')^(1.5*NPSX - 4)
//! ```
//!
//! with `C_n` the `NPSX`-dependent normalisation upstream tabulates (1.2732,
//! 3.2813, 5.8205 for 3, 4, 5 particles). The comparison is against that
//! formula evaluated independently here, not against our own converter's
//! intermediate values.
//!
//! # Results
//!
//! Printed by the test.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::ContinuumEmission;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// Upstream `f6psp`'s shape, evaluated independently of the converter.
fn f6psp(e_out: f64, e_max: f64, npsx: i32) -> f64 {
    if !(e_out < e_max) {
        return 0.0;
    }
    let ex = 1.5 * npsx as f64 - 4.0;
    let cn = match npsx {
        3 => 1.2732 / e_max.powi(2),
        4 => 3.2813 / e_max.powf(3.5),
        5 => 5.8205 / e_max.powi(5),
        _ => return 0.0,
    };
    cn * e_out.sqrt() * (e_max - e_out).powf(ex)
}

#[test]
fn h2_mt16_phase_space_matches_upstreams_closed_form() {
    let Some(p) = reference_endf_or_skip("n-001_H_002-ENDF8.0.endf", "H-2 (MF=6 LAW=6)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("H-2 tape parses");
    let mat = tape.materials()[0];

    let law = ContinuumEmission::from_endf_mf6(&tape, mat, 16)
        .expect("MF=6 parses")
        .expect(
            "H-2 MT=16 produced no continuum law. Its MF=6 is LAW=6 (phase space); before \
             2026-09-16 this fell back to the Weisskopf stand-in silently.",
        );
    assert_eq!(law.branches.len(), 1);
    let chi = &law.branches[0].spectrum;
    assert!(
        chi.incident.len() >= 10,
        "only {} incident rows built for H-2's phase-space law",
        chi.incident.len()
    );

    // H-2 (n,2n): NPSX = 3 (n + n + p), AWR from the tape.
    const NPSX: i32 = 3;

    // Compare the converted table's shape against the closed form at several
    // incident energies, using the row's OWN E'_max (its last grid point) so
    // the comparison tests the shape and the scaling separately from any
    // re-derivation of E'_max here.
    let mut worst = 0.0f64;
    let mut worst_at = 0.0f64;
    let mut checked = 0usize;
    for (k, &e_in) in chi.incident.iter().enumerate() {
        if k % 12 != 0 {
            continue;
        }
        let row = &chi.tables[k];
        let e_max = *row.e_out.last().expect("row");
        if e_max <= 0.0 {
            continue;
        }
        // Normalise both to unit integral before comparing shapes.
        let norm_ours: f64 = (0..row.e_out.len() - 1)
            .map(|i| {
                0.5 * (row.e_out[i + 1] - row.e_out[i]) * (row.pdf[i] + row.pdf[i + 1])
            })
            .sum();
        let norm_ref: f64 = (0..row.e_out.len() - 1)
            .map(|i| {
                0.5 * (row.e_out[i + 1] - row.e_out[i])
                    * (f6psp(row.e_out[i], e_max, NPSX) + f6psp(row.e_out[i + 1], e_max, NPSX))
            })
            .sum();
        if !(norm_ours > 0.0 && norm_ref > 0.0) {
            continue;
        }
        for i in 0..row.e_out.len() {
            let ours = row.pdf[i] / norm_ours;
            let theirs = f6psp(row.e_out[i], e_max, NPSX) / norm_ref;
            let scale = theirs.abs().max(1.0e-12 / e_max);
            let d = (ours - theirs).abs() / scale;
            if theirs > 1.0e-3 / e_max && d > worst {
                worst = d;
                worst_at = e_in;
            }
        }
        checked += 1;
    }

    assert!(checked >= 3, "only {checked} incident rows compared");
    assert!(
        worst < 0.05,
        "worst relative shape difference against upstream's f6psp is {worst:.4} (at incident \
         {worst_at:.4e} eV), above the 5 % gate. The converted phase-space table does not \
         reproduce the analytic distribution NJOY evaluates."
    );

    // E'_max must rise with incident energy -- the whole content of the law.
    let first = *chi.tables[0].e_out.last().unwrap();
    let last = *chi.tables[chi.incident.len() - 1].e_out.last().unwrap();
    assert!(
        last > first,
        "E'_max did not increase across the incident grid ({first:.4e} -> {last:.4e} eV); the \
         law's only incident-energy dependence is missing."
    );

    println!(
        "H-2 MT=16 MF=6 LAW=6 (phase space, NPSX={NPSX}): {} incident rows, E'_max \
         {first:.4e} -> {last:.4e} eV; worst relative shape difference against upstream's \
         f6psp closed form = {worst:.4} over {checked} rows",
        chi.incident.len()
    );
}

/// **Be-9 MT=16 is LAW=7 and remains unported** — asserted, so the day it is
/// implemented this test fails and gets updated rather than the gap quietly
/// persisting behind a Weisskopf fallback nobody rechecks.
#[test]
fn be9_mt16_law7_is_still_unported_and_says_so() {
    let Some(p) = reference_endf_or_skip("n-004_Be_009-ENDF8.0.endf", "Be-9 (MF=6 LAW=7)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("Be-9 tape parses");
    let mat = tape.materials()[0];
    let law = ContinuumEmission::from_endf_mf6(&tape, mat, 16).expect("MF=6 read does not fail");
    assert!(
        law.is_none(),
        "Be-9 MT=16 now yields a continuum law. Its MF=6 is LAW=7 (lab angle-energy), which \
         this port does not sample -- if that changed, this test should be replaced by a real \
         comparison rather than deleted."
    );
    println!(
        "Be-9 MT=16 is MF=6 LAW=7: still unported, still falling back to the Weisskopf \
         stand-in, and now recorded rather than silent"
    );
}
