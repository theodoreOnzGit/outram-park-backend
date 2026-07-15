//! Notebook: **`mgxs-part-i`** (multigroup XS generation) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mgxs-part-i.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! The OpenMC notebook defines a 2-group structure `[0, 0.625, 20e6]` eV, builds
//! `mgxs.TotalXS`/`AbsorptionXS`/`ScatterXS` tally objects, runs Monte-Carlo
//! transport, and extracts **flux-weighted, self-shielded** multigroup constants
//! from the tallies. njoy does not have a transport solver or tallies (those are
//! `outram-mc-libs`), so the full flux-solved MGXS is scaffolded `#[ignore]`.
//!
//! What njoy *does* own is the underlying **group-collapse primitive**:
//! averaging a pointwise σ(E) over a group under an assumed weighting spectrum,
//! σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE (`Mgxs::collapse`). This is a deliberately
//! low-fidelity, fixed-spectrum collapse (no self-shielding / no Boltzmann
//! solve), so it is a genuine *partial* of the notebook's MGXS — verified live
//! here on the notebook's own 2-group structure.
//!
//! # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
//!
//! - Collapsing reconstructed U-235 σ to `[0, 0.625, 20e6]` eV yields a thermal
//!   group (0–0.625 eV) whose total cross section far exceeds the fast group's —
//!   the expected 1/v absorber behaviour — with all group constants finite and
//!   non-negative.
//!
//! # Gaps (bead under op-6tz.6)
//!
//! - Flux-solved / self-shielded MGXS (GROUPR numeric engine and/or tally-based
//!   MGXS), including the group-to-group scatter matrix and group Chi.

use std::fs::File;
use std::path::PathBuf;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    nuclear_data::{secondary::NuBar, Mgxs, WeightingSpectrum},
    reconr::{reconr, ReconrConfig},
    MtReaction,
};

const U235_MAT: i32 = 9228;

fn u235_tape() -> Tape {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/n-092_U_235-ENDF8.0.endf");
    Tape::read(File::open(p).expect("open U-235")).expect("parse U-235 tape")
}

/// Live partial: the group-collapse primitive on the notebook's 2-group
/// structure `[0, 0.625, 20e6]` eV. Not the notebook's flux-solved MGXS — a
/// fixed-spectrum (1/E) collapse of the reconstructed cross section.
#[test]
fn collapse_to_notebook_two_group_structure() {
    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452")
        .expect("U-235 ν̄");

    // Fine energy grid (log-spaced, 1e-3 eV .. 20 MeV) + sampled σ columns.
    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let sample = |mt: MtReaction| -> Vec<f64> { grid.iter().map(|&e| result.eval_mt(mt, e)).collect() };
    let total = sample(MtReaction::Mt1Total);
    let elastic = sample(MtReaction::Mt2Elastic);
    let fission = sample(MtReaction::Mt18Fission);
    let capture = sample(MtReaction::Mt102Capture);
    let nu_fission: Vec<f64> = grid.iter().zip(&fission).map(|(&e, &sf)| sf * nu.at(e)).collect();
    let mubar = vec![0.0; grid.len()];

    // The notebook's 2-group structure.
    let bounds = [0.0_f64, 0.625, 2.0e7];
    let mg = Mgxs::collapse(
        "U235", &grid, &total, &elastic, &fission, &capture, &nu_fission, &mubar, &bounds,
        &WeightingSpectrum::OneOverE,
    );

    assert_eq!(mg.n_groups(), 2, "2-group structure");
    println!(
        "U-235 collapsed: thermal σ_t={:.1} b, fast σ_t={:.2} b",
        mg.total[0], mg.total[1]
    );
    for g in 0..2 {
        for (label, col) in [
            ("total", &mg.total),
            ("elastic", &mg.elastic),
            ("fission", &mg.fission),
            ("capture", &mg.capture),
            ("nu_fission", &mg.nu_fission),
        ] {
            assert!(col[g].is_finite() && col[g] >= 0.0, "group {g} {label} = {}", col[g]);
        }
    }
    // 1/v absorber: the thermal group total dwarfs the fast group total.
    assert!(
        mg.total[0] > 5.0 * mg.total[1],
        "thermal σ_t {:.1} should dominate fast σ_t {:.1}",
        mg.total[0], mg.total[1]
    );
    // Fission production present in the thermal group.
    assert!(mg.nu_fission[0] > 0.0, "thermal ν·σ_f = {}", mg.nu_fission[0]);
}

/// Notebook op: `mgxs.TotalXS(...)` + tallies + `run()` → self-shielded group
/// constants from the flux solution.
///
/// Requires a **transport solver with tally-based MGXS** (or the GROUPR numeric
/// group-averaging engine, which is `NotPorted`). op-6tz.6 "flux-solved MGXS" bead.
#[test]
#[ignore = "requires flux-solved / self-shielded MGXS (transport tallies or GROUPR engine) (op-6tz.6)"]
fn flux_weighted_self_shielded_mgxs() {
    panic!("njoy has no transport tallies or ported GROUPR group-averaging engine — flux-solved MGXS unavailable");
}
