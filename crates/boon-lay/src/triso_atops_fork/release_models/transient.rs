// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`breakthrough_model_transient`, `booth_transient`, `RF_Graph`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Transient (accident) release models
//!
//! Accident-case counterparts of the [steady-state](super::steady_state) models.
//! During a transient the temperature — and therefore the diffusion coefficient
//! — changes with time, so these models are written in terms of the
//! **time-integrated** diffusion coefficient rather than a single `D·t`:
//!
//! - `∫D' dt` (dimensionless) where `D' = D/a²`, and
//! - `∫D dt` (units of m^2, a [`uom::si::f64::Area`]).
//!
//! Both integrals are produced by
//! [`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time`]. Ported
//! from `calculation_functions.py`; see User Manual §3.3 ("Accident").
//!
//! | Function | Upstream |
//! |---|---|
//! | [`breakthrough_model_transient`] | `breakthrough_model_transient` |
//! | [`booth_transient`] | `booth_transient` |
//! | [`rf_graph`] | `RF_Graph` |

use std::f64::consts::PI;

use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Ratio};
use uom::si::length::meter;
use uom::si::ratio::ratio;

use crate::triso_atops_fork::ReleaseFraction;

use super::steady_state::{BOOTH_SERIES_TERMS, BREAKTHROUGH_SERIES_TERMS};

/// Default number of terms in the [`rf_graph`] series (upstream `num_terms=5000`).
pub const RF_GRAPH_SERIES_TERMS: usize = 5000;

/// Below this value a [`booth_transient`] release fraction is snapped to 0 (upstream `1e-6`).
pub const BOOTH_TRANSIENT_ZERO_FLOOR: f64 = 1e-6;

/// Transient breakthrough release fraction from the kernel/barrier.
///
/// Ports `breakthrough_model_transient(int_Dp, int_Dt, a, r)`. The transient
/// analogue of [`super::steady_state::breakthrough_model`], with `D·t` replaced
/// by the time-integral `∫D dt` and `D'·t` by `∫D' dt`:
///
/// ```text
///   RF = 3·(∫D dt)/(a·r) − a/(2r) − (6a/r)·Σ_{n≥1} (−1)ⁿ/(nπ)² · exp(−(nπ)²·∫D' dt)
/// ```
///
/// Clamped to `[0, 1]`; returns exactly 0 if either integral is 0 (upstream
/// guard). Used for the silver/SiC accident release.
///
/// # Arguments
/// - `integrated_d_prime` — `∫D' dt` (dimensionless [`Ratio`]), where `D' = D/a²`.
/// - `integrated_d` — `∫D dt` (an [`Area`], m^2).
/// - `layer_thickness` — barrier thickness `a`, metres.
/// - `kernel_radius` — kernel radius `r`, metres.
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`].
#[must_use]
pub fn breakthrough_model_transient(
    integrated_d_prime: Ratio,
    integrated_d: Area,
    layer_thickness: Length,
    kernel_radius: Length,
) -> ReleaseFraction {
    let int_dp = integrated_d_prime.get::<ratio>();
    let int_dt = integrated_d.get::<square_meter>();
    let a = layer_thickness.get::<meter>();
    let r = kernel_radius.get::<meter>();

    if int_dt == 0.0 || int_dp == 0.0 {
        return Ratio::new::<ratio>(0.0);
    }

    let mut sum = 0.0;
    for k in 1..BREAKTHROUGH_SERIES_TERMS {
        let n = k as f64;
        sum += (-1.0_f64).powi(k as i32) / (n * PI).powi(2) * (-(n * PI).powi(2) * int_dp).exp();
    }
    let rf = 3.0 * int_dt / a / r - a / 2.0 / r - 6.0 * a / r * sum;
    Ratio::new::<ratio>(rf.clamp(0.0, 1.0))
}

/// Transient Booth release fraction from the kernel.
///
/// Ports `booth_transient(int_Dp)`. The transient analogue of
/// [`super::steady_state::booth_longlived`]:
///
/// ```text
///   RF = 1 − 6·Σ_{i≥1} (1/(iπ)²)·exp(−(iπ)²·∫D' dt)
/// ```
///
/// Returns 0 if `∫D' dt == 0`, and snaps values below
/// [`BOOTH_TRANSIENT_ZERO_FLOOR`] (1e-6) to 0 (upstream behaviour). Used for
/// special-metal / other fission-product accident release from the kernel.
///
/// # Arguments
/// - `integrated_d_prime` — `∫D' dt` (dimensionless [`Ratio`]), where `D' = D/a²`
///   and `a` is the kernel radius.
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`].
#[must_use]
pub fn booth_transient(integrated_d_prime: Ratio) -> ReleaseFraction {
    let int_dp = integrated_d_prime.get::<ratio>();
    if int_dp == 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    let mut sum = 0.0;
    for i in 1..BOOTH_SERIES_TERMS {
        let n = i as f64;
        sum += (-(n * PI).powi(2) * int_dp).exp() / (n * PI).powi(2);
    }
    let rf = 1.0 - 6.0 * sum;
    let rf = if rf < BOOTH_TRANSIENT_ZERO_FLOOR {
        0.0
    } else {
        rf
    };
    Ratio::new::<ratio>(rf)
}

/// Transient release fraction through the matrix **graphite**.
///
/// Ports `RF_Graph(val, a)`:
///
/// ```text
///   RF = Σ_{i odd} (8/(iπ)²)·( 1 − exp(−(iπ)²·(∫D dt)/(4a²)) )
/// ```
///
/// As `∫D dt → ∞` this saturates to 1 (since `Σ_{i odd} 8/(iπ)² = 1`).
///
/// # Arguments
/// - `integrated_d` — `∫D dt` (an [`Area`], m^2) for the graphite.
/// - `graphite_thickness` — graphite region thickness `a`, metres.
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`].
#[must_use]
pub fn rf_graph(integrated_d: Area, graphite_thickness: Length) -> ReleaseFraction {
    let val = integrated_d.get::<square_meter>();
    let a = graphite_thickness.get::<meter>();
    let mut sum = 0.0;
    let mut i = 1usize;
    while i < RF_GRAPH_SERIES_TERMS {
        let n = i as f64;
        sum += 8.0 / (n * PI).powi(2) * (1.0 - (-(n * PI).powi(2) * val / 4.0 / (a * a)).exp());
        i += 2;
    }
    Ratio::new::<ratio>(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn area(v: f64) -> Area {
        Area::new::<square_meter>(v)
    }
    fn rat(v: f64) -> Ratio {
        Ratio::new::<ratio>(v)
    }
    fn len(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    /// booth_transient returns exactly 0 for a zero integral.
    #[test]
    fn booth_transient_zero_integral_is_zero() {
        assert_eq!(booth_transient(rat(0.0)).get::<ratio>(), 0.0);
    }

    /// booth_transient saturates to 1 for a large integral.
    #[test]
    fn booth_transient_large_integral_saturates() {
        assert_relative_eq!(
            booth_transient(rat(10.0)).get::<ratio>(),
            1.0,
            epsilon = 1e-9
        );
    }

    /// rf_graph saturates to 1 as ∫D dt → ∞ (Σ_odd 8/(iπ)² = 1).
    #[test]
    fn rf_graph_saturates_to_one() {
        // val/(4a²) large: a=1e-3 ⇒ 4a²=4e-6; val=1.0 ⇒ argument huge.
        let rf = rf_graph(area(1.0), len(1e-3)).get::<ratio>();
        assert_relative_eq!(rf, 1.0, max_relative = 1e-3);
    }

    /// breakthrough_model_transient guards the zero-integral case.
    #[test]
    fn breakthrough_transient_zero_guard() {
        let rf = breakthrough_model_transient(rat(0.0), area(0.0), len(3.5e-5), len(2.13e-4));
        assert_eq!(rf.get::<ratio>(), 0.0);
    }
}
