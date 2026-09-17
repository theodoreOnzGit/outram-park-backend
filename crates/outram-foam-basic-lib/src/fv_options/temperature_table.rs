// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2017-2026 OpenFOAM Foundation
// Upstream project: OpenFOAM-dev (OpenFOAM Foundation), vendored read-only at
//   crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM
//   (README.org dated 14th July 2026; etc/bashrc WM_PROJECT_VERSION=dev).
// Upstream source: src/OpenFOAM/primitives/functions/Function1/ — the `table`
//   entry of `Foam::Function1<scalar>`, as used by
//   `porosityModels::solidification` (`D`) and `fv::VoFSolidificationMelting`
//   (`alphaSolidT`).
// Upstream licence: GPL-3.0-or-later.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! A tabulated function of temperature — the `Function1` `table` entry the
//! solidification models take their coefficients from.
//!
//! # What belongs here, and what does not
//!
//! Only the table lookup: ascending temperature knots, linear interpolation
//! between them, clamping outside them. It carries no unit and no physical
//! meaning of its own — the model that owns one says what its values are.
//!
//! This is *not* a general port of `Foam::Function1`. Upstream's `Function1`
//! also covers constants, polynomials, sine waves, CSV files and coded
//! expressions, dispatched at runtime; none of that is here, and adding it
//! would belong in its own module rather than this one.

use crate::interpolation::interpolate_xy;
use std::sync::Arc;

/// A monotone-in-temperature lookup table with linear interpolation and
/// end-clamping.
///
/// Upstream's `Function1<scalar>` `table` entry. Two knots are the usual case
/// for a phase-change coefficient — one value below the freezing point, another
/// above it — which makes the function ramp linearly across the interval
/// between them instead of stepping discontinuously.
///
/// # Units
///
/// `temperatures` are kelvin \[K\]. **`values` have whatever unit the owning
/// model gives them** — a rate \[1/s\] for
/// [`SolidificationPorosity`](super::SolidificationPorosity)'s drag `D(T)`, a
/// dimensionless fraction \[-\] for
/// [`VofSolidificationMelting`](super::VofSolidificationMelting)'s
/// `alphaSolid(T)`. The table itself does not know and does not check.
///
/// # Valid ranges
///
/// `temperatures` must be **strictly ascending**; `values` need not be monotone
/// in either direction (the drag table descends, and a fraction table may do
/// either). A query outside the tabulated range returns the nearest end value
/// rather than extrapolating, which is `Function1`'s `table` behaviour and is
/// what lets a two-knot table act as a smoothed step.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureTable {
    temperatures: Arc<Vec<f64>>,
    values: Arc<Vec<f64>>,
}

impl TemperatureTable {
    /// Build a table from ascending temperatures \[K\] and their values.
    ///
    /// # Panics
    ///
    /// Panics if the two vectors differ in length, if either is empty, or if
    /// the temperatures are not strictly ascending. All three are case-setup
    /// errors that would otherwise yield a silently wrong coefficient, so they
    /// fail loudly at construction rather than at the first lookup.
    #[must_use]
    pub fn new(temperatures: Vec<f64>, values: Vec<f64>) -> Self {
        assert_eq!(
            temperatures.len(),
            values.len(),
            "temperature table: the temperature and value columns must have equal length"
        );
        assert!(
            !temperatures.is_empty(),
            "temperature table: at least one knot is required"
        );
        assert!(
            temperatures.windows(2).all(|w| w[1] > w[0]),
            "temperature table: temperatures must be strictly ascending"
        );
        Self {
            temperatures: Arc::new(temperatures),
            values: Arc::new(values),
        }
    }

    /// A two-knot table: `cold_value` at and below `t_cold`, `hot_value` at and
    /// above `t_hot`, linear in between.
    ///
    /// This is the shape both upstream examples use — `(330 10000) (330.5 0)`
    /// for the drag table, `(330 1) (335 0)` for the solid-fraction table.
    ///
    /// # Panics
    ///
    /// Panics if `t_hot <= t_cold`.
    #[must_use]
    pub fn two_knot(t_cold: f64, t_hot: f64, cold_value: f64, hot_value: f64) -> Self {
        Self::new(vec![t_cold, t_hot], vec![cold_value, hot_value])
    }

    /// The tabulated value at temperature `t` \[K\], clamped at both ends.
    #[must_use]
    pub fn value(&self, t: f64) -> f64 {
        interpolate_xy(t, &self.temperatures, &self.values)
    }

    /// The tabulated temperatures \[K\], ascending.
    #[must_use]
    pub fn temperatures(&self) -> &[f64] {
        &self.temperatures
    }

    /// The tabulated values, in the unit the owning model assigns them.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}
