// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/boundaryConditions/
//             velocityRundown/velocityRundownFvPatchVectorField.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `velocity_rundown` — pump/flow coastdown velocity boundary condition
//!
//! Port of GeN-Foam's `velocityRundownFvPatchVectorField`: a velocity inlet
//! whose value follows a power-law coastdown/run-up profile once a trip time
//! `t_0` has elapsed —
//!
//! ```text
//! U(t) = U0                              for t <= t0
//! U(t) = U0 * (A + B*(t - t0))^C         for t >  t0
//! ```
//!
//! `U0` is the reference (pre-trip, steady) velocity vector; `A`, `B`, `C` are
//! the upstream dictionary entries `const`, `coeff`, `exp`. This reproduces a
//! **power-law** rundown, not an exponential one — e.g. a pump coasting down
//! under `C < 0` slows towards zero as `(1 + B*(t-t0))^C` grows without bound
//! in the denominator sense; a linear ramp is `C = 1`.
//!
//! ## Reading back a restarted case (not ported)
//!
//! Upstream's dictionary constructor also *un-rundowns* a `value` entry read
//! from a restart file back to `U0` (dividing by the same factor evaluated at
//! the restart time), because OpenFOAM restart files store the boundary
//! field's last-evaluated value rather than the original reference velocity.
//! That is restart-file I/O plumbing, not physics — this port only exposes
//! the run-time evaluation `U(t)` given `U0` directly; a caller that already
//! has `U0` (from the case setup, not a restart file) does not need it.
//!
//! ## What this is (and is not)
//!
//! A **physical-value closure**: a plain struct plus a method computing the
//! instantaneous velocity vector as a function of time, not a full
//! `fvPatchField`. The reference velocity is carried as a basic-lib
//! [`Vector3`] rather than a `uom` vector type — matching the precedent in
//! [`super::super::structure::pump::Pump`], whose doc explains why: the
//! underlying `Foam::volVectorField`/`surfaceVectorField` this ultimately
//! feeds is itself raw `f64` per component, so `uom` typing stops at the
//! scalar coefficients (`A`, `C` dimensionless; `B` and `t0` carry time
//! dimensions) and the vector magnitude is scaled by the dimensionless
//! rundown factor.

use uom::si::f64::{Frequency, Ratio, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;

use outram_foam_basic_lib::prelude::Vector3;

/// A pump/flow coastdown (power-law rundown) velocity boundary condition.
///
/// Port of `velocityRundownFvPatchVectorField`. See the [module
/// documentation](self) for the `U(t) = U0 * (A + B*(t-t0))^C` profile.
/// Construct with [`VelocityRundownBc::new`]; query with
/// [`VelocityRundownBc::rundown_factor`] (the dimensionless scale factor
/// alone) or [`VelocityRundownBc::velocity`] (applied to a reference
/// velocity vector).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocityRundownBc {
    /// The dictionary `const` entry `A`, dimensionless.
    pub const_a: Ratio,
    /// The dictionary `coeff` entry `B` — **base SI: 1/s** (`Hz`). Multiplies
    /// the elapsed time `t - t0` so the sum `A + B*(t-t0)` stays
    /// dimensionless before being raised to the power `C`.
    pub coeff_b: Frequency,
    /// The dictionary `exp` entry `C`, dimensionless exponent (can be
    /// negative for a decaying rundown, e.g. `C < 0`, or positive for a
    /// ramp-up).
    pub exp_c: f64,
    /// The trip/start time `t0`. The profile is held at the reference value
    /// for `t <= t0` and evaluates the power law for `t > t0`.
    pub start_time: Time,
}

impl VelocityRundownBc {
    /// Build a rundown BC from its four dictionary entries (`const`, `coeff`,
    /// `exp`, `startTime` upstream).
    #[must_use]
    pub fn new(const_a: Ratio, coeff_b: Frequency, exp_c: f64, start_time: Time) -> Self {
        Self {
            const_a,
            coeff_b,
            exp_c,
            start_time,
        }
    }

    /// The dimensionless rundown factor `f(t)`:
    ///
    /// ```text
    /// f(t) = 1                        for t <= t0
    /// f(t) = (A + B*(t - t0))^C       for t >  t0
    /// ```
    ///
    /// Port of the `Foam::pow(A_+B_*(t-t0_), C_)` expression in
    /// `updateCoeffs()`; the `t <= t0` branch matches upstream's early return
    /// (the fixed value — already `f = 1` relative to `U0` — is left
    /// untouched).
    #[must_use]
    pub fn rundown_factor(&self, t: Time) -> Ratio {
        if t <= self.start_time {
            return Ratio::new::<ratio>(1.0);
        }
        let dt = (t - self.start_time).get::<second>();
        let base = self.const_a.get::<ratio>() + self.coeff_b.get::<hertz>() * dt;
        Ratio::new::<ratio>(base.powf(self.exp_c))
    }

    /// The instantaneous velocity `U(t) = U0 * f(t)`, given the reference
    /// (pre-trip) velocity vector `U0`.
    #[must_use]
    pub fn velocity(&self, reference_velocity: Vector3, t: Time) -> Vector3 {
        reference_velocity * self.rundown_factor(t).get::<ratio>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    fn hz(x: f64) -> Frequency {
        Frequency::new::<hertz>(x)
    }

    fn ratio(x: f64) -> Ratio {
        Ratio::new::<uom::si::ratio::ratio>(x)
    }

    /// V&V — power-law rundown reproduces the analytic exponent by hand.
    ///
    /// Methodology: `A = 1`, `B = -0.5 Hz`, `C = 2`, `t0 = 10 s`, a coastdown
    /// that (over the tested window) shrinks the linear term
    /// `(1 - 0.5*(t-t0))` and squares it. At `t = 11 s` (`dt = 1 s`):
    /// `f = (1 - 0.5*1)^2 = 0.5^2 = 0.25`. At `t = 12 s` (`dt = 2 s`):
    /// `f = (1 - 0.5*2)^2 = 0^2 = 0`. Reference velocity `U0 = (2, 0, 0)
    /// m/s`, so `U(11 s) = (0.5, 0, 0) m/s` and `U(12 s) = (0, 0, 0) m/s`.
    ///
    /// Result (2026-07-15): `f(11 s) = 0.250000`, `f(12 s) = 0.000000`,
    /// `U(11 s).x = 0.500000 m/s`, `U(12 s).x = 0.000000 m/s`,
    /// `|error| < 1e-9` throughout. Pass.
    #[test]
    fn power_law_rundown_matches_hand_calculation() {
        let bc = VelocityRundownBc::new(ratio(1.0), hz(-0.5), 2.0, s(10.0));

        let f11 = bc.rundown_factor(s(11.0));
        assert!((f11.get::<uom::si::ratio::ratio>() - 0.25).abs() < 1e-9);

        let f12 = bc.rundown_factor(s(12.0));
        assert!((f12.get::<uom::si::ratio::ratio>() - 0.0).abs() < 1e-9);

        let u0 = Vector3::new(2.0, 0.0, 0.0);
        let u11 = bc.velocity(u0, s(11.0));
        assert!((u11.x - 0.5).abs() < 1e-9);
        assert!(u11.y.abs() < 1e-12 && u11.z.abs() < 1e-12);

        let u12 = bc.velocity(u0, s(12.0));
        assert!(u12.x.abs() < 1e-9);
    }

    /// V&V — before the trip time, the profile holds the reference velocity
    /// exactly (the upstream early-return branch), and exactly at `t0` the
    /// power-law and held-value branches must agree (`dt = 0` makes
    /// `A^C = 1^C = 1` for `A = 1`).
    ///
    /// Result (2026-07-15): `U(0 s) = U0`, `U(t0) = U0`, `|error| < 1e-12`
    /// componentwise. Pass.
    #[test]
    fn holds_reference_velocity_before_and_at_trip_time() {
        let bc = VelocityRundownBc::new(ratio(1.0), hz(-0.5), 2.0, s(10.0));
        let u0 = Vector3::new(3.0, -1.0, 0.5);

        let before = bc.velocity(u0, s(0.0));
        assert!((before.x - u0.x).abs() < 1e-12);
        assert!((before.y - u0.y).abs() < 1e-12);
        assert!((before.z - u0.z).abs() < 1e-12);

        let at_trip = bc.velocity(u0, s(10.0));
        assert!((at_trip.x - u0.x).abs() < 1e-9);
        assert!((at_trip.y - u0.y).abs() < 1e-9);
        assert!((at_trip.z - u0.z).abs() < 1e-9);
    }

    #[test]
    fn linear_ramp_down_to_zero() {
        // A = 1, B = -1 Hz, C = 1: f(t) = 1 - (t - t0), a straight-line
        // ramp-down to zero at dt = 1 s.
        let bc = VelocityRundownBc::new(ratio(1.0), hz(-1.0), 1.0, s(0.0));
        let f_half = bc.rundown_factor(s(0.5));
        assert!((f_half.get::<uom::si::ratio::ratio>() - 0.5).abs() < 1e-12);
        let f_one = bc.rundown_factor(s(1.0));
        assert!(f_one.get::<uom::si::ratio::ratio>().abs() < 1e-12);
    }
}
