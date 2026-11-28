// SPDX-License-Identifier: AGPL-3.0-only
//! **MATPRO UO2 gaseous swelling against Fig. 8.7 of the BISON Theory
//! Manual** (Hales et al. 2013, INL/EXT-13-29930, p. 22; maintainer's private
//! corpus). The figure plots the manual's own Eqs (8.21)–(8.22), the same
//! correlation as [`SwellingModel::Uo2Matpro`], so it checks this crate's
//! transcription. It is verification, not validation.
//!
//! # Methodology
//!
//! The figure's caption gives neither the density nor the burnup conversion
//! behind the curves, so its MAGNITUDES cannot be checked without choosing
//! inputs the figure does not state, and they are not. What IS free of both:
//! at fixed burnup the gaseous swelling is
//! `C(T) · g(ρ, Bu)` with `C(T) ∝ (2800 − T)^11.73 exp(−0.0162 (2800 − T))`,
//! so the ratio `gas(T) / gas(T_peak)` depends on temperature alone.
//!
//! The red "10 − Gaseous" curve was digitised from a 400 dpi render of PDF
//! page 24 (`verification_and_validation/bison_manual_fig87/digitise_fig87.py`,
//! axis calibrated from the detected tick marks: 192.75 px per 500 K,
//! 217.6 px per 0.05; marker columns excluded). One pixel is 2.3e-4 in ΔV/V.
//! The peak value 0.1454 is the vertex of a parabola through 78 points
//! between 1900 and 2260 K. The tolerance, 0.01 on the ratio (about 6 px at
//! the peak value), is the line width plus calibration error, set after the
//! numbers below had been seen; the measured differences are a third of it.
//!
//! # Results (2026-11-28)
//!
//! | T (K) | digitised ratio | this crate | difference |
//! |---|---|---|---|
//! | 1597.3 | 0.1630 | 0.1650 | −0.0020 |
//! | 1695.8 | 0.2964 | 0.2987 | −0.0023 |
//! | 1799.6 | 0.5041 | 0.5043 | −0.0002 |
//! | 2300.3 | 0.4883 | 0.4889 | −0.0006 |
//! | 2398.8 | 0.1809 | 0.1836 | −0.0027 |
//!
//! The equation peaks at `2800 − 11.73/0.0162 = 2075.9` K; the digitised
//! parabola vertex is 2066.7 K. The 9 K (3.5 px) gap is within the bias of a
//! parabola fitted to a skewed peak, so it is reported and not gated.
//! **Not checked:** the magnitudes. The 10 MWd/kgU solid plateau digitises
//! to 0.00753, which by Eq (8.21) needs `ρ·FIMA = 135 kg/m³`. That exceeds
//! UO2's theoretical density times this crate's `Bu/937.06` FIMA, so the
//! figure used a density or burnup basis it does not state.

use outram_park_fork_offbeat::materials::behavioral::swelling::SwellingModel;
use outram_park_fork_offbeat::materials::MaterialState;

/// Gaseous swelling of `model` at `t` (K) and 10 MWd/kgHM: the value minus
/// the value at 300 K, where the gaseous term is 6.6e-7 of its peak (the
/// solid term does not depend on temperature).
fn gas(model: &SwellingModel, t: f64) -> f64 {
    let mut s = MaterialState::fresh(t);
    s.burnup = 10.0;
    let mut cold = MaterialState::fresh(300.0);
    cold.burnup = 10.0;
    model.value(&s) - model.value(&cold)
}

#[test]
fn gaseous_swelling_shape_matches_bison_manual_fig_8_7() {
    let model = SwellingModel::Uo2Matpro { theoretical_density: 10960.0 };
    let t_peak = 2800.0 - 11.73 / 0.0162;
    // Digitised (T in K, ΔV/V) from Fig. 8.7, "10 - Gaseous".
    let digitised = [
        (1597.3, 0.0237),
        (1695.8, 0.0431),
        (1799.6, 0.0733),
        (2300.3, 0.0710),
        (2398.8, 0.0263),
    ];
    let digitised_peak = 0.1454;
    let g_peak = gas(&model, t_peak);
    // The equation's own maximum is where its derivative vanishes.
    assert!(gas(&model, t_peak - 1.0) < g_peak && gas(&model, t_peak + 1.0) < g_peak);
    for (t, v) in digitised {
        let ours = gas(&model, t) / g_peak;
        let fig = v / digitised_peak;
        println!("T = {t:.1} K: figure {fig:.4}, offbeat {ours:.4}, diff {:+.4}", fig - ours);
        assert!((fig - ours).abs() < 0.01, "T = {t}: figure {fig}, offbeat {ours}");
    }
}
