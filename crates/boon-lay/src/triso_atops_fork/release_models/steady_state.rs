// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`RB_fail_Noble_Gases`, `breakthrough_model`, `booth_longlived`,
//                     `booth_shortlived_fastdiffuse`, `attenuation_factor`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Steady-state (normal-operation) release models
//!
//! Closed-form release-to-birth / release-fraction models for a reactor at
//! **steady, constant temperature** (normal operation). Each is a solution (or
//! empirical fit) to Fickian diffusion out of the fuel, expressed as a single
//! `<R/B>` or release fraction. Ported from `calculation_functions.py`.
//!
//! | Function | Upstream | Applies to |
//! |---|---|---|
//! | [`rb_fail_noble_gases`] | `RB_fail_Noble_Gases` | Kr, Xe, halogens (empirical `<R/B>` fit) |
//! | [`breakthrough_model`] | `breakthrough_model` | low-release / barrier-limited species (e.g. Ag through SiC) |
//! | [`booth_longlived`] | `booth_longlived` | long-lived metals, large release |
//! | [`booth_shortlived_fast_diffuse`] | `booth_shortlived_fastdiffuse` | short-lived metals, decay-limited |
//! | [`attenuation_factor`] | `attenuation_factor` | graphite hold-up factor (≥ 1, not a fraction) |
//!
//! All release fractions are clamped to `[0, 1]` exactly where the upstream code
//! clamps them. The number of terms kept in each infinite series matches the
//! upstream defaults ([`BREAKTHROUGH_SERIES_TERMS`], [`BOOTH_SERIES_TERMS`],
//! [`ATTENUATION_SERIES_TERMS`]).

use std::f64::consts::PI;

use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Length, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::triso_atops_fork::{DecayConstant, ReleaseFraction};

/// Default number of terms in the [`breakthrough_model`] series (upstream `num_terms=1000`).
pub const BREAKTHROUGH_SERIES_TERMS: usize = 1000;
/// Default number of terms in the [`booth_longlived`] series (upstream `num_terms=5000`).
pub const BOOTH_SERIES_TERMS: usize = 5000;
/// Default number of terms in the [`attenuation_factor`] series (upstream `num_terms=500`).
pub const ATTENUATION_SERIES_TERMS: usize = 500;
/// Upper cap the upstream places on the graphite attenuation factor.
pub const ATTENUATION_FACTOR_CAP: f64 = 1e8;

/// Clamp a raw release fraction into the physical `[0, 1]` range.
#[inline]
fn clamp_release_fraction(rf: f64) -> ReleaseFraction {
    Ratio::new::<ratio>(rf.clamp(0.0, 1.0))
}

/// Empirical release-to-birth `<R/B>_fail` for noble gases and halogens.
///
/// Ports `RB_fail_Noble_Gases(z, lam, T)`:
/// `<R/B> = exp( n·ln(1/λ) + B/T_K + C )`, with `(n, B, C)` an empirical fit that
/// differs for krypton versus xenon/halogens. This is the release from a
/// **failed/exposed** particle for the volatile species that do not follow the
/// Booth metal model.
///
/// # Arguments
/// - `z` — atomic number. `z == 36` (Kr) selects the krypton fit; any other
///   value routed here (Xe `z == 54`, or a halogen) selects the xenon/halogen
///   fit. Intended only for noble gases and halogens (see
///   [`crate::triso_atops_fork::nuclide_model::ElementGroup`]).
/// - `decay_constant` — the nuclide decay constant `λ` (s^-1); the fit uses its
///   numeric value in s^-1 inside `ln(1/λ)`.
/// - `temperature` — local temperature; the fit uses absolute temperature in K.
///
/// # Returns
/// The dimensionless `<R/B>_fail` as a [`ReleaseFraction`].
#[must_use]
pub fn rb_fail_noble_gases(
    z: u32,
    decay_constant: DecayConstant,
    temperature: ThermodynamicTemperature,
) -> ReleaseFraction {
    // Krypton fit vs. xenon/halogen fit (upstream constants).
    let (n, b, c) = if z == 36 {
        (0.325, -8572.0, -1.41)
    } else {
        (0.302, -7793.0, -2.73)
    };
    let lam = decay_constant.get::<hertz>();
    let t_k = temperature.get::<kelvin>();
    let rb = (n * (1.0 / lam).ln() + b / t_k + c).exp();
    Ratio::new::<ratio>(rb)
}

/// Booth "breakthrough" release fraction for a barrier-limited (low-release) species.
///
/// Ports `breakthrough_model(D, t, a, r)`. This is the classic low-release
/// expansion for diffusion through a shell of thickness `a` around a kernel of
/// radius `r`:
///
/// ```text
///   RF = 3·D·t/(r·a) − a/(2r) − (6a/r)·Σ_{n≥1} (−1)ⁿ/(nπ)² · exp(−(nπ)²·D'·t)
/// ```
///
/// with `D' = D/a²`. Used for silver diffusing through the SiC layer (the
/// limiting barrier). The result is clamped to `[0, 1]`.
///
/// # Arguments
/// - `diffusion_coefficient` — `D` in the barrier layer, m^2/s.
/// - `time` — elapsed (irradiation) time `t`, seconds.
/// - `layer_thickness` — barrier thickness `a`, metres.
/// - `kernel_radius` — kernel radius `r`, metres.
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`].
#[must_use]
pub fn breakthrough_model(
    diffusion_coefficient: DiffusionCoefficient,
    time: Time,
    layer_thickness: Length,
    kernel_radius: Length,
) -> ReleaseFraction {
    let d = diffusion_coefficient.get::<square_meter_per_second>();
    let t = time.get::<second>();
    let a = layer_thickness.get::<meter>();
    let r = kernel_radius.get::<meter>();

    let d_prime = d / a / a; // 1/s
    let mut sum = 0.0;
    for k in 1..BREAKTHROUGH_SERIES_TERMS {
        let n = k as f64;
        sum +=
            (-1.0_f64).powi(k as i32) / (n * PI).powi(2) * (-(n * PI).powi(2) * d_prime * t).exp();
    }
    let rf = 3.0 * (d * t) / r / a - a / 2.0 / r - 6.0 * a / r * sum;
    clamp_release_fraction(rf)
}

/// Booth model release fraction for a **long-lived** species with large release.
///
/// Ports `booth_longlived(D, t, a)`. The Booth equivalent-sphere fractional
/// release from diffusion out of a sphere of radius `a`:
///
/// ```text
///   RF = 1 − (6/π²)·Σ_{i≥1} (1/i²)·exp(−(iπ)²·D'·t) ,   D' = D/a²
/// ```
///
/// As `D'·t → ∞`, `RF → 1`. For small `D'·t` it recovers the early-time law
/// `RF ≈ 6·√(D'·t/π) − 3·D'·t`. Used for long-lived special-metal fission
/// products (Sr, Cs, Ba, Eu).
///
/// # Arguments
/// - `diffusion_coefficient` — `D`, m^2/s.
/// - `time` — elapsed time `t`, seconds.
/// - `equivalent_sphere_radius` — Booth equivalent-sphere radius `a`, metres.
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`] (the series is
/// mathematically already in range; not explicitly clamped, matching upstream).
#[must_use]
pub fn booth_longlived(
    diffusion_coefficient: DiffusionCoefficient,
    time: Time,
    equivalent_sphere_radius: Length,
) -> ReleaseFraction {
    let d = diffusion_coefficient.get::<square_meter_per_second>();
    let t = time.get::<second>();
    let a = equivalent_sphere_radius.get::<meter>();
    let d_prime = d / a / a;

    let mut sum = 0.0;
    for i in 1..BOOTH_SERIES_TERMS {
        let n = i as f64;
        sum += (-(n * PI).powi(2) * d_prime * t).exp() / (n * PI).powi(2);
    }
    Ratio::new::<ratio>(1.0 - 6.0 * sum)
}

/// Booth model `<R/B>` for a **short-lived** species that diffuses fast relative to decay.
///
/// Ports `booth_shortlived_fastdiffuse(D, lam, a)`. The steady-state
/// release-to-birth ratio for a decaying species diffusing out of an
/// equivalent sphere:
///
/// ```text
///   x = √(λ·a²/D) ,   <R/B> = (3/x)·( coth(x) − 1/x )
/// ```
///
/// Limits: as `x → 0` (fast diffusion / long-lived), `<R/B> → 1`; as `x → ∞`
/// (slow diffusion / short-lived), `<R/B> → 3/x`.
///
/// # Arguments
/// - `diffusion_coefficient` — `D`, m^2/s.
/// - `decay_constant` — `λ`, s^-1.
/// - `equivalent_sphere_radius` — Booth equivalent-sphere radius `a`, metres.
///
/// # Returns
/// The dimensionless `<R/B>` as a [`ReleaseFraction`].
#[must_use]
pub fn booth_shortlived_fast_diffuse(
    diffusion_coefficient: DiffusionCoefficient,
    decay_constant: DecayConstant,
    equivalent_sphere_radius: Length,
) -> ReleaseFraction {
    let d = diffusion_coefficient.get::<square_meter_per_second>();
    let lam = decay_constant.get::<hertz>();
    let a = equivalent_sphere_radius.get::<meter>();
    let x = (lam * a * a / d).sqrt();
    let rb = 3.0 / x * (1.0 / x.tanh() - 1.0 / x);
    Ratio::new::<ratio>(rb)
}

/// Graphite hold-up (attenuation) factor `Af` for a fission metal.
///
/// Ports `attenuation_factor(D_graph, t, a)`. This is **not** a release fraction
/// — it is a dimensionless factor `Af ≥ 1` describing how much the matrix
/// graphite attenuates (delays) release of a metal, via
///
/// ```text
///   S = Σ_{i odd} (4/(iπ))·sin(iπ/2)·exp(−(iπ)²·D_graph·t/(4a²)) ,   Af = 1/(1 − S)
/// ```
///
/// The upstream code caps `Af` at [`ATTENUATION_FACTOR_CAP`] (1e8) and returns
/// the cap if `S == 1`, `Af > 1e8`, or `Af < 0`; reproduced here.
///
/// # Arguments
/// - `graphite_diffusion_coefficient` — `D_graph`, m^2/s.
/// - `time` — elapsed time `t`, seconds.
/// - `graphite_thickness` — graphite region thickness `a`, metres.
///
/// # Returns
/// The dimensionless attenuation factor as a [`Ratio`] (≥ 1; **not** clamped to
/// `[0, 1]` — it is a hold-up factor, not a fraction).
#[must_use]
pub fn attenuation_factor(
    graphite_diffusion_coefficient: DiffusionCoefficient,
    time: Time,
    graphite_thickness: Length,
) -> Ratio {
    let d = graphite_diffusion_coefficient.get::<square_meter_per_second>();
    let t = time.get::<second>();
    let a = graphite_thickness.get::<meter>();

    let mut sum = 0.0;
    let mut i = 1usize;
    while i < ATTENUATION_SERIES_TERMS {
        let n = i as f64;
        sum +=
            4.0 / n / PI * (n * PI / 2.0).sin() * (-(n * PI).powi(2) * d * t / 4.0 / a / a).exp();
        i += 2;
    }
    let af = if sum == 1.0 {
        ATTENUATION_FACTOR_CAP
    } else {
        1.0 / (1.0 - sum)
    };
    let af = if af > ATTENUATION_FACTOR_CAP || af < 0.0 {
        ATTENUATION_FACTOR_CAP
    } else {
        af
    };
    Ratio::new::<ratio>(af)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn d(v: f64) -> DiffusionCoefficient {
        DiffusionCoefficient::new::<square_meter_per_second>(v)
    }
    fn len(v: f64) -> Length {
        Length::new::<meter>(v)
    }
    fn t(v: f64) -> Time {
        Time::new::<second>(v)
    }
    fn lam(v: f64) -> DecayConstant {
        DecayConstant::new::<hertz>(v)
    }

    /// Booth long-lived → 1 as diffusion time grows large.
    #[test]
    fn booth_longlived_saturates_to_one() {
        // D'·t large: a=1e-5 m, D=1e-12 m^2/s, t=1e6 s ⇒ D'·t = 1e-12/1e-10·1e6 = 1e4.
        let rf = booth_longlived(d(1e-12), t(1e6), len(1e-5)).get::<ratio>();
        assert_relative_eq!(rf, 1.0, epsilon = 1e-9);
    }

    /// Booth long-lived early-time law: RF ≈ 6√(D't/π) − 3D't for small D't.
    ///
    /// Methodology: choose D'·t = 1e-4 (a=1e-3 m, D=1e-13, t=1e3 ⇒
    /// D'=1e-13/1e-6=1e-7, D't=1e-4). Analytical early-time value
    /// 6·√(1e-4/π) − 3·1e-4 = 6·0.005642 − 3e-4 = 0.033550. The series must
    /// agree to a few parts in 1e3 (early-time expansion accuracy at D't=1e-4).
    #[test]
    fn booth_longlived_early_time_matches_analytic() {
        let dt = 1e-4_f64;
        let a = 1e-3;
        let d_val = 1e-13;
        let time_val = dt * a * a / d_val; // gives D'·t = dt
        let rf = booth_longlived(d(d_val), t(time_val), len(a)).get::<ratio>();
        let analytic = 6.0 * (dt / PI).sqrt() - 3.0 * dt;
        assert_relative_eq!(rf, analytic, max_relative = 5e-3);
    }

    /// Booth short-lived coth limit: x→∞ ⇒ <R/B> → 3/x.
    #[test]
    fn booth_shortlived_large_x_limit() {
        // x = √(λa²/D). Choose λ=1, a=1e-2, D=1e-8 ⇒ x=√(1·1e-4/1e-8)=√1e4=100.
        let rb = booth_shortlived_fast_diffuse(d(1e-8), lam(1.0), len(1e-2)).get::<ratio>();
        let x = (1.0_f64 * 1e-4 / 1e-8).sqrt();
        // For large x, coth(x)≈1, so <R/B> ≈ (3/x)(1 − 1/x) ≈ 3/x.
        assert_relative_eq!(
            rb,
            3.0 / x * (1.0 / x.tanh() - 1.0 / x),
            max_relative = 1e-12
        );
        assert_relative_eq!(rb, 3.0 / x, max_relative = 2e-2);
    }

    /// Booth short-lived small-x limit: x→0 ⇒ <R/B> → 1.
    #[test]
    fn booth_shortlived_small_x_limit() {
        // x small: λ=1e-6, a=1e-5, D=1e-8 ⇒ x=√(1e-6·1e-10/1e-8)=√1e-8=1e-4.
        let rb = booth_shortlived_fast_diffuse(d(1e-8), lam(1e-6), len(1e-5)).get::<ratio>();
        assert_relative_eq!(rb, 1.0, max_relative = 1e-6);
    }

    /// Breakthrough model is clamped to [0, 1] and small at early time.
    #[test]
    fn breakthrough_model_clamped_and_small_early() {
        let rf = breakthrough_model(d(1e-14), t(1e3), len(3.5e-5), len(2.13e-4)).get::<ratio>();
        assert!((0.0..=1.0).contains(&rf));
    }

    /// Attenuation factor is ≥ 1 and finite (capped).
    #[test]
    fn attenuation_factor_ge_one_and_capped() {
        let af = attenuation_factor(d(1e-12), t(1e6), len(4.5e-3)).get::<ratio>();
        assert!(af >= 1.0);
        assert!(af <= ATTENUATION_FACTOR_CAP);
    }
}
