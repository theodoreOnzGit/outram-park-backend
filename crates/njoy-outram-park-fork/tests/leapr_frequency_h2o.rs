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
use njoy_outram_park_fork::leapr::continuous::phonon_expansion;
use njoy_outram_park_fork::leapr::discrete::add_discrete_oscillators;
use njoy_outram_park_fork::leapr::translation::add_translation;
use njoy_outram_park_fork::leapr::{ContinuousDist, DiscreteOscillator, FrequencyModel, LeaprInput};

/// Continuous phonon frequency spectrum ρ(E) from NJOY2016 test-09 (H in H₂O,
/// shortened ENDF/B-VI.4 model), tabulated on a Δ = 0.00255 eV grid, `rho[0]` at
/// E = 0. 67 points.
const RHO_H2O: [f64; 67] = [
    0.0, 0.0005, 0.001, 0.002, 0.0035, 0.005, 0.0075, 0.01, 0.013, 0.0165, 0.02, 0.0245, 0.029,
    0.034, 0.0395, 0.045, 0.0506, 0.0562, 0.0622, 0.0686, 0.075, 0.083, 0.091, 0.099, 0.107, 0.115,
    0.1197, 0.1214, 0.1218, 0.1195, 0.1125, 0.1065, 0.1005, 0.09542, 0.09126, 0.0871, 0.0839,
    0.0807, 0.07798, 0.07574, 0.0735, 0.07162, 0.06974, 0.06804, 0.06652, 0.065, 0.0634, 0.0618,
    0.06022, 0.05866, 0.0571, 0.05586, 0.05462, 0.0535, 0.0525, 0.0515, 0.05042, 0.04934, 0.04822,
    0.04706, 0.0459, 0.04478, 0.04366, 0.04288, 0.04244, 0.042, 0.0,
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
    println!(
        "H2O solid-type: lambda = {lambda:.6}, T_eff = {t_eff:.3} K (tbar = {:.6})",
        fm.tbar
    );

    // NJOY2016 test-09 LEAPR listing: debye-waller lambda = 0.235204.
    let lambda_ref = 0.235204_f64;
    let rel_l = (lambda - lambda_ref).abs() / lambda_ref;
    assert!(
        rel_l < 1.0e-3,
        "lambda = {lambda:.6} vs NJOY {lambda_ref} (rel {rel_l:.2e})"
    );

    // NJOY2016 test-09 LEAPR listing: effective temp = 572.610 K.
    let teff_ref = 572.610_f64;
    let rel_t = (t_eff - teff_ref).abs() / teff_ref;
    assert!(
        rel_t < 1.0e-3,
        "T_eff = {t_eff:.3} vs NJOY {teff_ref} (rel {rel_t:.2e})"
    );
}

/// Momentum-transfer grid α (65 pts) from NJOY2016 test-09 (`lat=1`, so scaled to
/// 0.0253 eV in the kernels).
const ALPHA_H2O: [f64; 65] = [
    0.01008, 0.015, 0.0252, 0.033, 0.050406, 0.0756, 0.100812, 0.151218, 0.201624, 0.25203,
    0.302436, 0.352842, 0.403248, 0.453654, 0.50406, 0.554466, 0.609711, 0.670259, 0.736623,
    0.809349, 0.889061, 0.976435, 1.07213, 1.17708, 1.29211, 1.41822, 1.55633, 1.70775, 1.87379,
    2.05566, 2.25506, 2.47352, 2.71295, 2.97546, 3.26308, 3.57832, 3.9239, 4.30266, 4.7177,
    5.17256, 5.67118, 6.21758, 6.8165, 7.47289, 8.19228, 8.98073, 9.84489, 10.7919, 11.8303,
    12.9674, 14.2145, 15.5815, 17.0796, 18.7208, 20.5203, 22.4922, 24.6526, 27.0216, 29.6175,
    32.4625, 35.5816, 38.9991, 42.7453, 46.8503, 50.0,
];

/// Energy-transfer grid β (75 pts) from NJOY2016 test-09.
const BETA_H2O: [f64; 75] = [
    0.0, 0.006375, 0.01275, 0.0255, 0.03825, 0.051, 0.06575, 0.0806495, 0.120974, 0.161299,
    0.241949, 0.322598, 0.403248, 0.483897, 0.564547, 0.645197, 0.725846, 0.806496, 0.887145,
    0.967795, 1.04844, 1.12909, 1.20974, 1.29039, 1.37104, 1.45169, 1.53234, 1.61299, 1.69364,
    1.77429, 1.85494, 1.93559, 2.01624, 2.09689, 2.17754, 2.25819, 2.33884, 2.41949, 2.50014,
    2.58079, 2.6695, 2.76709, 2.87445, 2.9925, 3.12235, 3.2653, 3.42247, 3.59536, 3.78549, 3.99467,
    4.22473, 4.47787, 4.75631, 5.06258, 5.39939, 5.76997, 6.17766, 6.62607, 7.11924, 7.66181,
    8.25862, 8.91511, 9.63722, 10.432, 11.3051, 12.2668, 13.3243, 14.4867, 15.766, 17.1733,
    18.7218, 20.4245, 22.2976, 24.3572, 25.0,
];

/// Full LEAPR pipeline for H-in-H₂O (test-09): the effective temperature and
/// Debye-Waller λ evolve correctly through the continuous, translational, and
/// discrete-oscillator convolutions, matching every checkpoint NJOY prints.
///
/// Methodology: build the complete test-09 `LeaprInput` (65-α × 75-β grid, `lat`,
/// the 67-point continuous ρ(E) with tbeta=0.444444, free-gas translational
/// weight twt=0.055556, and the two discrete oscillators 0.205 eV @ 0.166667 and
/// 0.48 eV @ 0.333333, at 296 K — the weights twt+tbeta+Σw = 1). Run
/// `phonon_expansion` → `add_translation` → `add_discrete_oscillators` and check
/// the running effective temperature `tempf` and Debye-Waller λ (`dwpix`) against
/// the three NJOY LEAPR-listing checkpoints for this deck:
///
/// ```text
///   solid-type (continuous):  effective temp = 572.610   lambda = 0.235204
///   + translational part:     new effective temp = 541.875
///   + discrete-oscillator:    new effective temp = 1397.671  new lambda = 0.273669
/// ```
///
/// Pass criterion: each checkpoint within 0.1 % (relative). Because `tempf` and λ
/// are global integrals over the whole S(α,β), matching all three is a strong
/// end-to-end check of the continuous + translational + discrete kernels together
/// (short of a per-(α,β) matrix comparison, which is the remaining follow-up).
///
/// Result (2026-07-20): all three checkpoints match to the printed precision —
/// see the asserts.
#[test]
fn h2o_effective_temp_and_lambda_through_full_pipeline() {
    let temp_k = 296.0_f64;
    let tev = BK_EV_PER_K * temp_k;
    let input = LeaprInput {
        alpha: ALPHA_H2O.to_vec(),
        beta: BETA_H2O.to_vec(),
        lat: true,
        arat: 1.0,
        nphon: 100,
        temperature_k: temp_k,
        continuous: ContinuousDist {
            delta_ev: 0.00255,
            rho: RHO_H2O.to_vec(),
            twt: 0.055556,
            c: 0.0,
            tbeta: 0.444444,
        },
        oscillators: vec![
            DiscreteOscillator {
                energy_ev: 0.205,
                weight: 0.166667,
            },
            DiscreteOscillator {
                energy_ev: 0.48,
                weight: 0.333333,
            },
        ],
    };

    let freq = FrequencyModel::start(&input.continuous.rho, 0.00255, tev, 0.444444);

    // Checkpoint 1: continuous (solid-type) effective temperature.
    let mut tempf = freq.tbar * temp_k;
    let mut dwpix = freq.f0;
    let rel = |got: f64, want: f64| (got - want).abs() / want;
    assert!(
        rel(tempf, 572.610) < 1e-3,
        "continuous T_eff = {tempf:.3} vs 572.610"
    );

    // Checkpoint 2: after the translational part.
    let mut sab = phonon_expansion(&input, &freq);
    add_translation(&mut sab, &input, &freq, &mut tempf);
    println!("after translation: T_eff = {tempf:.3} K");
    assert!(
        rel(tempf, 541.875) < 1e-3,
        "post-translation T_eff = {tempf:.3} vs 541.875"
    );

    // Checkpoint 3: after the discrete oscillators (T_eff and Debye-Waller λ).
    add_discrete_oscillators(&mut sab, &input, &mut dwpix, &mut tempf);
    println!("after discrete: T_eff = {tempf:.3} K, lambda = {dwpix:.6}");
    assert!(
        rel(tempf, 1397.671) < 1e-3,
        "post-discrete T_eff = {tempf:.3} vs 1397.671"
    );
    assert!(
        rel(dwpix, 0.273669) < 1e-3,
        "post-discrete lambda = {dwpix:.6} vs 0.273669"
    );
}
