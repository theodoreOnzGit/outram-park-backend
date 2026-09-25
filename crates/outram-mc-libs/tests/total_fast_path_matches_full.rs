// SPDX-License-Identifier: GPL-3.0

//! **`total_at_energy` must return exactly what `xs_at_energy(..).total`
//! returns** — it is an optimisation, not an approximation.
//!
//! # Why this test exists
//!
//! `Nuclide::total_at_energy` was added on 2026-09-24 after the LCT-008
//! timing sweep traced an 8.4x transport gap against OpenMC to eager
//! evaluation: `xs_at_energy` computes ~46 grid lookups per nuclide (MT=1,
//! MT=2, MT=18, all 40 inelastic levels, MT=16/17/5, MT=27) where the
//! flight-distance sample reads only `.total`, which MT=1 already carries.
//! Taking MT=1 alone cut transport from 64.2 s to 20.1 s with `k` unchanged.
//!
//! "With `k` unchanged" was measured on **one case, one seed**. That is
//! evidence, not a guarantee: `k` is an integral over the whole problem and a
//! small disagreement in a corner of the energy range can hide inside its
//! statistics. This asserts the equality directly, per nuclide, per energy.
//!
//! # The branch that could go wrong quietly
//!
//! Below the S(alpha,beta) cutoff the total is **not** MT=1. The bound-atom
//! law replaces the elastic channel and the total is rebuilt as
//! `absorption + inelastic + n2n + sigma_sab`. `total_at_energy` detects that
//! and defers to the full evaluation. H-1 with `c_H_in_H2O` is included here
//! precisely to exercise that path — on a thermal lattice, getting it wrong
//! shifts `k` in the way that is hardest to notice.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

/// Energies spanning thermal to fast, log-spaced, plus the boundaries that
/// matter: the S(alpha,beta) cutoff region and the resonance range.
fn probe_energies() -> Vec<f64> {
    let mut es = Vec::new();
    // 1e-5 eV to 20 MeV, 400 points, logarithmic.
    for i in 0..=400 {
        let f = i as f64 / 400.0;
        es.push(1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f));
    }
    // The thermal region densely: this is where the bound-atom override lives.
    for i in 0..=100 {
        es.push(1.0e-4 + i as f64 * (5.0 - 1.0e-4) / 100.0);
    }
    es
}

fn check(nuclide: &Nuclide, label: &str) {
    let mut worst_rel = 0.0_f64;
    let mut worst_at = 0.0_f64;
    for e in probe_energies() {
        let full = nuclide.xs_at_energy(e, TEMP_K).total;
        let fast = nuclide.total_at_energy(e, TEMP_K);
        // Exact equality is the contract: both paths evaluate the SAME
        // expression, so any difference is a logic error rather than a
        // floating-point one. A relative tolerance is kept only so a failure
        // reports how far off it is instead of just "not equal".
        if full != fast {
            let rel = if full != 0.0 {
                ((full - fast) / full).abs()
            } else {
                (full - fast).abs()
            };
            if rel > worst_rel {
                worst_rel = rel;
                worst_at = e;
            }
        }
    }
    assert!(
        worst_rel == 0.0,
        "{label}: total_at_energy disagrees with xs_at_energy(..).total by \
         {worst_rel:e} relative at {worst_at:e} eV. The fast path is an \
         optimisation and must be exact; a difference here is a logic error, \
         most likely in the S(alpha,beta) branch."
    );
}

/// A fissile actinide with a full inelastic level structure — the case the
/// fast path exists for, where `xs_at_energy` does ~46 lookups.
#[test]
fn u235_fast_total_matches_the_full_evaluation() {
    let Some(p) = reference_endf("n-092_U_235-ENDF8.0.endf") else {
        eprintln!("SKIP u235_fast_total: reference tape absent");
        return;
    };
    let n = Nuclide::from_endf_file(&p, "U235", TEMP_K, 1.0e-3).expect("U235");
    check(&n, "U235");
}

/// U-238: 40 inelastic levels and the deepest resonance structure here.
#[test]
fn u238_fast_total_matches_the_full_evaluation() {
    let Some(p) = reference_endf("n-092_U_238.endf") else {
        eprintln!("SKIP u238_fast_total: reference tape absent");
        return;
    };
    let n = Nuclide::from_endf_file(&p, "U238", TEMP_K, 1.0e-3).expect("U238");
    check(&n, "U238");
}

/// **The branch that matters.** H-1 carrying `c_H_in_H2O`: below the cutoff
/// the total is rebuilt from the bound-atom law, so the fast path must NOT
/// return MT=1 there.
#[test]
fn h1_with_thermal_scattering_matches_through_the_sab_cutoff() {
    let Some(p) = reference_endf("n-001_H_001-ENDF8.0-Beta6.endf") else {
        eprintln!("SKIP h1_fast_total: reference tape absent");
        return;
    };
    let plain = Nuclide::from_endf_file(&p, "H1", TEMP_K, 1.0e-3).expect("H1");
    check(&plain, "H1 (free gas)");

    // And again with the bound-atom law attached, which is how LCT-008 and
    // every other thermal case in this workspace actually runs it.
    let Some(sab_tape) = reference_endf("tsl-HinH2O.endf") else {
        eprintln!("SKIP h1 bound: tsl-HinH2O tape absent (free-gas arm still checked)");
        return;
    };
    let sab = outram_mc_libs::material::thermal::ThermalScattering::from_endf_file(
        sab_tape.to_str().expect("path"),
        1, // MAT 1 — H in H2O, ENDF/B-VIII.0
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("c_H_in_H2O");
    let bound = plain.with_thermal_scattering(sab);
    check(&bound, "H1 (c_H_in_H2O)");
}
