//! V&V: LEAPR continuous (solid-type) phonon integrals for H-in-H₂O, validated
//! against NJOY2016 (bead op-cjw.16).
//!
//! # Methodology
//!
//! The ported [`FrequencyModel::start`](njoy_outram_park_fork::leapr::FrequencyModel::start)
//! (leapr.f90:647-724) builds, from a phonon frequency spectrum ρ(E), the
//! incoherent-Gaussian Debye-Waller λ and the effective-temperature factor
//! `tbar = T_eff / T`. This test drives it with the **exact continuous-spectrum
//! input of NJOY2016 regression test 09** — the "H in H₂O, shortened ENDF model"
//! LEAPR deck (`vendor/njoy2016/tests/09/input`): the 67-point ρ(E) tabulated on a
//! Δ = 0.00255 eV grid, continuum normalization tbeta = 0.444444, at T = 296 K —
//! and compares the two solid-type scalars against the values NJOY's own LEAPR
//! prints for that deck.
//!
//! Reference (oracle): NJOY2016 (built from source at commit `2c64dfb`,
//! `njoy 2016.79`) was run on test-09's `moder → reconr → broadr → leapr → thermr`
//! deck; its LEAPR listing reports, for the "solid-type contributions to
//! scattering law" block with "p(beta) scaled to 0.44444, for p-bound only":
//!
//! ```text
//!             effective temp =    572.610
//!        debye-waller lambda=  0.235204
//! ```
//!
//! (Cross-check that the oracle build is faithful: on the same deck NJOY produced
//! a LEAPR MF=7 tape byte-identical to the committed `tests/09/referenceTape24`.)
//!
//! The ρ(E) data are open (an NJOY2016 regression input, modified BSD 3-Clause /
//! GPL-compatible; NJOY2016 is public LANL software). Only the numeric spectrum
//! and the two printed scalars are used here — no NJOY code is reproduced.
//!
//! Pass criterion: our λ and T_eff each within 0.1 % (relative) of the NJOY
//! values — tight enough to catch a wrong integral or moment, loose enough to
//! absorb the k_B-constant and trapezoid-rounding differences between the codes.
//!
//! # Results (2026-07-20)
//!
//! Measured: λ = 0.235204, T_eff = tbar·296 = 572.61 K — matching the NJOY
//! reference to the printed precision (see the asserts). This validates the
//! phonon-spectrum integrals (`start`/`fsum`: F₀ = λ, F₂ → T_eff) of the LEAPR
//! continuous path against the oracle. The full S(α,β) matrix comparison (adding
//! the translational + discrete-oscillator convolutions and the `endout` MF=7
//! writer) remains to be wired; this closes the frequency-integral piece.

use njoy_outram_park_fork::common::phys::BK_EV_PER_K;
use njoy_outram_park_fork::leapr::FrequencyModel;

/// Continuous phonon frequency spectrum ρ(E) from NJOY2016 test-09 (H in H₂O,
/// shortened ENDF/B-VI.4 model), tabulated on a Δ = 0.00255 eV grid, `rho[0]` at
/// E = 0. 67 points.
const RHO_H2O: [f64; 67] = [
    0.0, 0.0005, 0.001, 0.002, 0.0035, 0.005, 0.0075, 0.01, 0.013, 0.0165, 0.02, 0.0245,
    0.029, 0.034, 0.0395, 0.045, 0.0506, 0.0562, 0.0622, 0.0686, 0.075, 0.083, 0.091,
    0.099, 0.107, 0.115, 0.1197, 0.1214, 0.1218, 0.1195, 0.1125, 0.1065, 0.1005, 0.09542,
    0.09126, 0.0871, 0.0839, 0.0807, 0.07798, 0.07574, 0.0735, 0.07162, 0.06974,
    0.06804, 0.06652, 0.065, 0.0634, 0.0618, 0.06022, 0.05866, 0.0571, 0.05586,
    0.05462, 0.0535, 0.0525, 0.0515, 0.05042, 0.04934, 0.04822, 0.04706, 0.0459,
    0.04478, 0.04366, 0.04288, 0.04244, 0.042, 0.0,
];

/// LEAPR solid-type Debye-Waller λ and effective temperature for H-in-H₂O match
/// the NJOY2016 reference (methodology + reference values in the module doc).
#[test]
fn h2o_solid_type_lambda_and_effective_temp() {
    let delta_ev = 0.00255_f64;
    let tbeta = 0.444444_f64;
    let temp_k = 296.0_f64;
    let tev = BK_EV_PER_K * temp_k;

    let fm = FrequencyModel::start(&RHO_H2O, delta_ev, tev, tbeta);

    let lambda = fm.f0;
    let t_eff = fm.tbar * temp_k;
    println!("H2O solid-type: lambda = {lambda:.6}, T_eff = {t_eff:.3} K (tbar = {:.6})", fm.tbar);

    // NJOY2016 test-09 LEAPR listing: debye-waller lambda = 0.235204.
    let lambda_ref = 0.235204_f64;
    let rel_l = (lambda - lambda_ref).abs() / lambda_ref;
    assert!(rel_l < 1.0e-3, "lambda = {lambda:.6} vs NJOY {lambda_ref} (rel {rel_l:.2e})");

    // NJOY2016 test-09 LEAPR listing: effective temp = 572.610 K.
    let teff_ref = 572.610_f64;
    let rel_t = (t_eff - teff_ref).abs() / teff_ref;
    assert!(rel_t < 1.0e-3, "T_eff = {t_eff:.3} vs NJOY {teff_ref} (rel {rel_t:.2e})");
}
