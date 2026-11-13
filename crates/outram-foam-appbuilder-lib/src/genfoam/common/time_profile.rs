// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/common/timeProfile/timeProfile.{C,H}
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Thomas Guilbaud <thomas.guilbaud@epfl.ch> (EPFL/Transmutex)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Time profiles — time-tabulated scalar simulation inputs
//!
//! A [`TimeProfile`] is the value of one simulation input as a function of
//! **time** — GeN-Foam's `timeProfile` (a thin wrapper over OpenFOAM's
//! `Function1<scalar>`). It is how a transient case prescribes a time-dependent
//! driver that is *not* computed from the fields, for example:
//!
//! - the **external reactivity** `ρ(t)` injected into a point-kinetics run
//!   (a control-rod ramp, a step reactivity insertion) — dimensionless `Δk/k`,
//! - an **external neutron-source power** `S(t)` (watts),
//! - a **boron concentration** or coolant-inlet boundary value ramp.
//!
//! ## Dimension convention (read this before use)
//!
//! The **abscissa is always time**, so it is typed as [`Time`] (seconds).
//!
//! The **ordinate is a raw `f64`** whose physical meaning is fixed by the
//! *consumer*, not by this type. This mirrors the upstream `Function1<scalar>`,
//! which is deliberately dimension-agnostic: the same machinery tabulates
//! reactivity (dimensionless), source power (W), and concentration (mol/m³).
//! Attaching a single `uom` unit here would be physically wrong for a shared
//! helper. Consumers must document and attach the ordinate's unit at their own
//! boundary — e.g. point-kinetics reads a reactivity profile as a
//! [`uom::si::f64::Ratio`] and a source profile as a [`uom::si::f64::Power`].
//!
//! ## Start-time offset
//!
//! Every profile carries a `start_time` offset `t_0` (default `0 s`, faithful to
//! GeN-Foam's `startTime`). A query at wall-clock time `t` is evaluated at the
//! *shifted* time `t − t_0`, so a table authored from `t = 0` can be delayed to
//! begin at `t_0` without re-tabulating it. Before `t_0` the table clamps to its
//! first value (see [`TimeProfile::value`]).
//!
//! ## What is intentionally not ported
//!
//! GeN-Foam's `timeProfile` can also be driven by an **FMI** port
//! (`type fmi`), reading the value each step from a co-simulation
//! `commDataLayer`. That is live external-system coupling — out of scope for
//! this offline library and excluded by the workspace responsible-use policy —
//! so it is **not** ported. Only the self-contained table/constant profiles are.
//!
//! ## Example
//!
//! ```
//! use outram_foam_appbuilder_lib::genfoam::common::time_profile::TimeProfile;
//! use uom::si::f64::Time;
//! use uom::si::time::second;
//!
//! // A 57.67 pcm (0.2 β) step reactivity that switches on at t = 100 s:
//! // the table is authored from t = 0, then delayed by the start-time offset.
//! let rho = TimeProfile::table(
//!     vec![Time::new::<second>(0.0), Time::new::<second>(1.0)],
//!     vec![0.0, 0.0005767131],
//! )
//! .unwrap()
//! .with_start_time(Time::new::<second>(100.0));
//!
//! assert_eq!(rho.value(Time::new::<second>(50.0)), 0.0);          // before start
//! assert_eq!(rho.value(Time::new::<second>(100.0)), 0.0);         // at start (shifted t = 0)
//! assert_eq!(rho.value(Time::new::<second>(101.0)), 0.0005767131);// shifted t = 1 s
//! ```

use uom::si::f64::Time;
use uom::si::time::second;

use outram_foam_basic_lib::interpolation::interpolate_xy;

/// Errors returned when constructing a [`TimeProfile`] from a table.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TimeProfileError {
    /// The `times` and `values` vectors had different lengths.
    #[error("time-profile table has mismatched lengths: {times} times vs {values} values")]
    LengthMismatch {
        /// Number of abscissa (time) entries supplied.
        times: usize,
        /// Number of ordinate (value) entries supplied.
        values: usize,
    },

    /// A table needs at least two points to interpolate between.
    #[error("time-profile table needs at least 2 points, got {0}")]
    TooFewPoints(usize),

    /// The abscissa (time) column was not strictly ascending. Interpolation
    /// requires a strictly increasing time axis; equal or decreasing times are
    /// rejected. The reported index `i` is the first offender (`times[i] <=
    /// times[i-1]`), in seconds.
    #[error("time-profile times not strictly ascending at index {index}: {prev} s then {next} s")]
    NonAscendingTimes {
        /// Index of the first non-ascending entry.
        index: usize,
        /// The preceding time, in seconds.
        prev: f64,
        /// The offending time, in seconds.
        next: f64,
    },
}

/// The interpolation kind backing a [`TimeProfile`]. Private: consumers dispatch
/// through [`TimeProfile::value`], never on this enum directly.
#[derive(Debug, Clone, PartialEq)]
enum ProfileKind {
    /// A single value held for all time.
    Constant(f64),
    /// A piecewise-linear table. `times` is strictly ascending (seconds);
    /// `values` are the raw ordinates. Queries clamp to the endpoints outside
    /// the tabulated range.
    Table { times: Vec<f64>, values: Vec<f64> },
}

/// A single simulation input tabulated against **time**.
///
/// See the [module documentation](self) for the dimension convention (abscissa
/// is [`Time`]; the ordinate is a raw `f64` whose unit the consumer fixes) and
/// the start-time offset semantics.
///
/// Construct one with [`TimeProfile::constant`] or [`TimeProfile::table`], then
/// optionally shift it with [`TimeProfile::with_start_time`]. Query it with
/// [`TimeProfile::value`].
#[derive(Debug, Clone, PartialEq)]
pub struct TimeProfile {
    /// Time offset `t_0` (seconds, stored as raw `f64` for arithmetic; exposed
    /// as [`Time`] through [`TimeProfile::start_time`]). Queries are evaluated
    /// at `t − t_0`.
    start_time: f64,
    kind: ProfileKind,
}

impl TimeProfile {
    /// A profile that returns `value` at every time — the direct analogue of an
    /// OpenFOAM `constant` `Function1`.
    ///
    /// `value` is a raw ordinate in the consumer's own units (dimensionless for
    /// reactivity, watts for source power, …).
    pub fn constant(value: f64) -> Self {
        Self {
            start_time: 0.0,
            kind: ProfileKind::Constant(value),
        }
    }

    /// A piecewise-linear profile through the tabulated points
    /// `(times[i], values[i])` — the analogue of an OpenFOAM `table`
    /// `Function1` with linear interpolation.
    ///
    /// # Parameters
    /// - `times` — strictly ascending abscissae as [`Time`] (seconds), length ≥ 2.
    /// - `values` — the raw ordinates, one per time (same length as `times`).
    ///
    /// # Behaviour
    /// Between adjacent points the value is linearly interpolated. Outside the
    /// tabulated range the value **clamps** to the nearest endpoint (matching
    /// [`interpolate_xy`](fn@outram_foam_basic_lib::interpolation::interpolate_xy)
    /// and OpenFOAM's default `outOfBounds clamp`).
    ///
    /// # Errors
    /// - [`TimeProfileError::LengthMismatch`] if the two vectors differ in length.
    /// - [`TimeProfileError::TooFewPoints`] if fewer than two points are given.
    /// - [`TimeProfileError::NonAscendingTimes`] if `times` is not strictly
    ///   increasing.
    pub fn table(times: Vec<Time>, values: Vec<f64>) -> Result<Self, TimeProfileError> {
        if times.len() != values.len() {
            return Err(TimeProfileError::LengthMismatch {
                times: times.len(),
                values: values.len(),
            });
        }
        if times.len() < 2 {
            return Err(TimeProfileError::TooFewPoints(times.len()));
        }

        let times: Vec<f64> = times.into_iter().map(|t| t.get::<second>()).collect();
        for i in 1..times.len() {
            if times[i] <= times[i - 1] {
                return Err(TimeProfileError::NonAscendingTimes {
                    index: i,
                    prev: times[i - 1],
                    next: times[i],
                });
            }
        }

        Ok(Self {
            start_time: 0.0,
            kind: ProfileKind::Table { times, values },
        })
    }

    /// Set the start-time offset `t_0`: subsequent [`value`](Self::value) queries
    /// at wall-clock time `t` are evaluated at the shifted time `t − t_0`.
    ///
    /// Consumes and returns `self` so it can chain off a constructor.
    #[must_use]
    pub fn with_start_time(mut self, start_time: Time) -> Self {
        self.start_time = start_time.get::<second>();
        self
    }

    /// The start-time offset `t_0` as a [`Time`].
    pub fn start_time(&self) -> Time {
        Time::new::<second>(self.start_time)
    }

    /// Whether this profile can be evaluated. Always `true` for the ported
    /// (constant/table) profiles; retained to mirror GeN-Foam's
    /// `timeProfile::valid()`, which could be `false` for an unset FMI port
    /// (not ported here).
    pub fn valid(&self) -> bool {
        true
    }

    /// The value of the profile at wall-clock time `t`, evaluated at the shifted
    /// time `t − t_0` (`t_0` = [`start_time`](Self::start_time)).
    ///
    /// Returns a raw `f64` ordinate in the consumer's own units. For a table,
    /// the shifted time is clamped to the tabulated range (values before the
    /// first point return the first value; after the last, the last value).
    ///
    /// **ESI form (gh:#1361, 2026-11-12).** A table interpolates the way
    /// OpenFOAM-ESI v2506's `Function1Types::Table` does, which is what
    /// GeN-Foam calls: weights `w0 = (t1 - x) / (t1 - t0)`, `w1 = 1 - w0`,
    /// value `w0 * y0 + w1 * y1`
    /// (`linearInterpolationWeights::valueWeights`, `TableBase::value`).
    /// Before this the table used the Foundation-lineage
    /// `y0 + t * (y1 - y0)`, which differed from a run of GeN-Foam in 242 of
    /// 667 interior values (up to 24 ulp). That form stays callable as
    /// [`value_foundation`](Self::value_foundation).
    pub fn value(&self, t: Time) -> f64 {
        let shifted = t.get::<second>() - self.start_time;
        match &self.kind {
            ProfileKind::Constant(v) => *v,
            ProfileKind::Table { times, values } => esi_table_value(shifted, times, values),
        }
    }

    /// As [`value`](Self::value) but a table interpolates with the OpenFOAM
    /// Foundation-lineage `interpolateXY` (`y0 + t * (y1 - y0)`, the
    /// basic-lib `interpolate_xy`). Kept callable, not deleted, per the
    /// maintainer's rule of 2026-11-12; it is NOT what GeN-Foam runs.
    pub fn value_foundation(&self, t: Time) -> f64 {
        let shifted = t.get::<second>() - self.start_time;
        match &self.kind {
            ProfileKind::Constant(v) => *v,
            ProfileKind::Table { times, values } => interpolate_xy(shifted, times, values),
        }
    }
}

/// OpenFOAM-ESI v2506 `Function1Types::Table` value with linear
/// interpolation and `outOfBounds clamp`: `TableBase::value` clamps outside
/// `[times[0], times[n-1]]`, then `linearInterpolationWeights::valueWeights`
/// takes `i = findLower(times, x)` (the last sample strictly below `x`, `-1`
/// when none): index `-1` uses the first value alone, otherwise
/// `w0 = (t1 - x) / (t1 - t0)`, `w1 = 1 - w0`, result `w0 * y0 + w1 * y1`.
/// `times` is strictly ascending with at least two entries (checked by
/// [`TimeProfile::table`]).
fn esi_table_value(x: f64, times: &[f64], values: &[f64]) -> f64 {
    let n = times.len();
    if x < times[0] {
        return values[0];
    }
    if x > times[n - 1] {
        return values[n - 1];
    }
    // Number of samples strictly below x; findLower returns that count - 1.
    let below = times.partition_point(|&s| s < x);
    if below == 0 {
        return 1.0 * values[0];
    }
    let i = below - 1;
    let (t0, t1) = (times[i], times[i + 1]);
    let w0 = (t1 - x) / (t1 - t0);
    let w1 = 1.0 - w0;
    w0 * values[i] + w1 * values[i + 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    #[test]
    fn constant_profile_is_flat() {
        let p = TimeProfile::constant(3.5);
        assert_eq!(p.value(s(-100.0)), 3.5);
        assert_eq!(p.value(s(0.0)), 3.5);
        assert_eq!(p.value(s(1e6)), 3.5);
        assert!(p.valid());
    }

    #[test]
    fn table_linear_interpolation() {
        let p = TimeProfile::table(vec![s(0.0), s(1.0), s(2.0)], vec![0.0, 2.0, 6.0]).unwrap();
        assert_eq!(p.value(s(0.0)), 0.0);
        assert_eq!(p.value(s(0.5)), 1.0); // midpoint of [0,2]
        assert_eq!(p.value(s(1.5)), 4.0); // midpoint of [2,6]
        assert_eq!(p.value(s(2.0)), 6.0);
    }

    #[test]
    fn table_clamps_out_of_range() {
        let p = TimeProfile::table(vec![s(1.0), s(3.0)], vec![10.0, 30.0]).unwrap();
        assert_eq!(p.value(s(-5.0)), 10.0);
        assert_eq!(p.value(s(100.0)), 30.0);
    }

    #[test]
    fn start_time_offset_shifts_query() {
        // Step from 0 to 1 over [0,1] s, delayed to begin at t = 100 s.
        let p = TimeProfile::table(vec![s(0.0), s(1.0)], vec![0.0, 1.0])
            .unwrap()
            .with_start_time(s(100.0));
        assert_eq!(p.start_time(), s(100.0));
        assert_eq!(p.value(s(50.0)), 0.0); // before start -> clamp to first
        assert_eq!(p.value(s(100.0)), 0.0); // shifted t = 0
        assert_eq!(p.value(s(100.5)), 0.5); // shifted t = 0.5
        assert_eq!(p.value(s(101.0)), 1.0); // shifted t = 1
        assert_eq!(p.value(s(200.0)), 1.0); // after -> clamp to last
    }

    #[test]
    fn esi_and_foundation_forms_agree_to_round_off_and_at_knots() {
        let times = vec![s(0.0), s(0.7), s(1.9), s(4.0)];
        let values = vec![0.001, -0.002, 0.0035, 0.0007];
        let p = TimeProfile::table(times, values.clone()).unwrap();
        // knots and clamps are exact in both forms
        for (t, v) in [0.0, 0.7, 1.9, 4.0].iter().zip(&values) {
            assert_eq!(p.value(s(*t)), *v);
            // the Foundation form `y0 + 1*(y1 - y0)` need not return y1 exactly
            assert!((p.value_foundation(s(*t)) - *v).abs() <= 1e-18);
        }
        assert_eq!(p.value(s(-1.0)), 0.001);
        assert_eq!(p.value(s(9.0)), 0.0007);
        // interior: the two forms differ only by round-off
        for k in 1..40 {
            let t = 0.1 * f64::from(k);
            let (a, b) = (p.value(s(t)), p.value_foundation(s(t)));
            assert!((a - b).abs() <= 1e-15, "t={t}: {a} vs {b}");
        }
        // hand value: midpoint of [0.7, 1.9] is the mean of -0.002 and 0.0035
        assert!((p.value(s(1.3)) - 0.00075).abs() < 1e-15);
    }

    #[test]
    fn length_mismatch_rejected() {
        let err = TimeProfile::table(vec![s(0.0), s(1.0)], vec![0.0]).unwrap_err();
        assert_eq!(
            err,
            TimeProfileError::LengthMismatch {
                times: 2,
                values: 1
            }
        );
    }

    #[test]
    fn too_few_points_rejected() {
        let err = TimeProfile::table(vec![s(0.0)], vec![1.0]).unwrap_err();
        assert_eq!(err, TimeProfileError::TooFewPoints(1));
    }

    #[test]
    fn non_ascending_times_rejected() {
        let err =
            TimeProfile::table(vec![s(0.0), s(2.0), s(2.0)], vec![0.0, 1.0, 2.0]).unwrap_err();
        assert_eq!(
            err,
            TimeProfileError::NonAscendingTimes {
                index: 2,
                prev: 2.0,
                next: 2.0
            }
        );
    }
}
