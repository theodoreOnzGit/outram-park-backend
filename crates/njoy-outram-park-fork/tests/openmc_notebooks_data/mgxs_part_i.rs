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
//! The GROUPR **vector** group-average engine is now ported (op-cjw.15) and is
//! exercised live here by `groupr_engine_vector_group_average`, which cross-checks
//! it against `Mgxs::collapse` (they agree to <0.03% on U-235). The engine is a
//! fixed-spectrum group-average, still not the notebook's self-shielded solve.
//!
//! # Gaps (bead under op-6tz.6)
//!
//! - Flux-solved / self-shielded (dilution/Bondarenko) MGXS, the group-to-group
//!   **scatter matrix**, and group **Chi** — these need the GROUPR *matrix* path
//!   (`cm2lab` kinematics + File-6 feeders, `NotPorted` in op-cjw.15) and/or
//!   transport-tally MGXS from `outram-mc-libs`.

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    groupr::panel::{group_average_vector, GroupFlux, PointwiseXs},
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

/// Live partial (**GROUPR numeric engine path**, op-cjw.15): group-average the
/// reconstructed U-235 total cross section over the notebook's 2-group structure
/// with the ported GROUPR panel-quadrature engine
/// ([`group_average_vector`]), and cross-check it against the lightweight
/// [`Mgxs::collapse`] primitive.
///
/// # Methodology
///
/// This is *not* the notebook's flux-solved / self-shielded MGXS. It is the
/// GROUPR **vector** group-average `σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE` computed by the
/// newly-ported `groupr::panel` engine (`groupr.f90` `panel`/`getsig`/`getflx`
/// reduction), under a fixed 1/E weighting spectrum — the same fixed-spectrum
/// assumption `Mgxs::collapse` makes, but via the GROUPR quadrature rather than
/// the lightweight collapse. Two independent group-average implementations over
/// the same reconstructed σ(E) must agree; the test asserts that plus the
/// physical 1/v-absorber property.
///
/// Inputs: U-235 ENDF/B-VIII.0 (MAT 9228), RECONR at 0.001 tol / 0 K, a
/// 3000-point log grid over `[1e-3, 2e7]` eV, group edges `[1e-3, 0.625, 2e7]`
/// eV (the thermal edge is the reconstructed-grid floor, since the 1/E weight and
/// the cross-section data are undefined at exactly 0 eV). Pass criterion: both
/// methods finite/non-negative, thermal group total ≥ 5× the fast group total,
/// and the two methods agree to the tolerance recorded below.
///
/// # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
///
/// - GROUPR panel engine: σ_t = `[1071.152, 35.1865]` barn (thermal, fast).
/// - `Mgxs::collapse`:     σ_t = `[1071.448, 35.1924]` barn.
/// - Relative difference: `2.8e-4` (thermal), `1.7e-4` (fast) — the two
///   independent group-average implementations agree to under 0.03%. Both are
///   trapezoid reductions of the same linearly-interpolated σ over the same union
///   grid; the residual is only the two engines' internal panel-refinement. The
///   test asserts `< MAX_REL_DIFF` with margin.
/// - Thermal group total (1071 b) dwarfs the fast group total (35 b): the 1/v
///   capture/fission signature.
///
/// # Gap that remains (still `#[ignore]` below)
///
/// True self-shielded (dilution/Bondarenko) MGXS, the group-to-group **scatter
/// matrix**, and group **Chi** need the GROUPR *matrix* path (`cm2lab` kinematics
/// + File-6 feeders), which op-cjw.15 leaves `NotPorted`, and/or transport-tally
/// MGXS from `outram-mc-libs`.
#[test]
fn groupr_engine_vector_group_average() {
    /// Measured agreement between the GROUPR panel engine and `Mgxs::collapse`
    /// (see the doc comment's Results section): the 2026-07-15 run showed
    /// `2.8e-4` / `1.7e-4` relative difference. This bound (0.5%) is that result
    /// with ~20x margin for grid/refinement drift — not an invented number.
    const MAX_REL_DIFF: f64 = 0.005;

    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452")
        .expect("U-235 ν̄");

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

    // Group edges: thermal edge is the reconstructed-grid floor (1e-3 eV) — the
    // 1/E weight and the σ table are undefined at exactly 0 eV.
    let bounds = [1.0e-3_f64, 0.625, 2.0e7];

    // --- GROUPR panel-quadrature engine (op-cjw.15) ---
    let sigma_pw = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(total.iter().copied()).collect::<Vec<_>>(),
    ));
    let flux = GroupFlux::spectrum(WeightingSpectrum::OneOverE);
    let sig_g_engine = group_average_vector(&sigma_pw, &flux, &bounds);

    // --- Lightweight collapse primitive, same weight/bounds ---
    let mg = Mgxs::collapse(
        "U235", &grid, &total, &elastic, &fission, &capture, &nu_fission, &mubar, &bounds,
        &WeightingSpectrum::OneOverE,
    );

    assert_eq!(sig_g_engine.len(), 2, "2-group vector");
    assert_eq!(mg.n_groups(), 2, "2-group collapse");
    println!(
        "GROUPR engine σ_t = [{:.3}, {:.4}] b ; collapse σ_t = [{:.3}, {:.4}] b",
        sig_g_engine[0], sig_g_engine[1], mg.total[0], mg.total[1]
    );

    for g in 0..2 {
        assert!(
            sig_g_engine[g].is_finite() && sig_g_engine[g] >= 0.0,
            "engine group {g} σ_t = {}",
            sig_g_engine[g]
        );
        let rel = (sig_g_engine[g] - mg.total[g]).abs() / mg.total[g].max(1e-30);
        assert!(
            rel < MAX_REL_DIFF,
            "group {g}: GROUPR engine σ_t {:.4} vs collapse {:.4}, rel diff {:.4} ≥ {}",
            sig_g_engine[g], mg.total[g], rel, MAX_REL_DIFF
        );
    }
    // 1/v absorber: thermal group total dwarfs the fast group total.
    assert!(
        sig_g_engine[0] > 5.0 * sig_g_engine[1],
        "thermal σ_t {:.1} should dominate fast σ_t {:.4}",
        sig_g_engine[0], sig_g_engine[1]
    );
}

/// Notebook op: `mgxs.TotalXS(...)` + tallies + `run()` → self-shielded group
/// constants from the flux solution, plus the group-to-group scatter matrix and
/// group Chi.
///
/// The GROUPR **vector** group-average engine is now ported (op-cjw.15, exercised
/// by [`groupr_engine_vector_group_average`]), but the notebook's *self-shielded*
/// MGXS and the **scatter matrix / Chi** need the GROUPR **matrix** path
/// (`cm2lab` kinematics + File-6 feeders, `NotPorted`) and/or transport-tally
/// MGXS from `outram-mc-libs`. op-6tz.6 "flux-solved MGXS" bead.
#[test]
#[ignore = "vector group-average ported (op-cjw.15); self-shielded MGXS + scatter matrix + Chi still need the GROUPR matrix path or transport tallies (op-6tz.6)"]
fn flux_weighted_self_shielded_mgxs() {
    panic!("GROUPR matrix path (scatter matrix / Chi / self-shielding) NotPorted and no transport tallies — flux-solved MGXS unavailable");
}
