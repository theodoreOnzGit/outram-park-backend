// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/boundaryConditions/
//             timeFieldTable/timeFieldTableFvPatchScalarField.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (stefan.radman@epfl.ch; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `time_field_table` — tabulated time-varying per-face scalar boundary
//!
//! Port of GeN-Foam's `timeFieldTableFvPatchScalarField`: a boundary
//! condition whose value is a **whole per-face field**, tabulated against
//! time and linearly interpolated —
//!
//! ```text
//! table
//! (
//!     (t0 (v0_0 v1_0 v2_0 ...))
//!     ...
//!     (tN (v0_N v1_N v2_N ...))
//! );
//! ```
//!
//! `ti` is a tabulated time and `vj_i` the value at face `j` for that time
//! row. For `t < t0` the first row's field applies unchanged; for `t > tN`
//! the last row's field applies unchanged; in between, adjacent rows are
//! linearly interpolated **face-by-face** with a single shared interpolation
//! coefficient (all faces cross their `(t_i, t_{i+1})` bracket together, since
//! they share one time axis).
//!
//! Unlike [`super::super::super::common::TimeProfile`] (one scalar value,
//! ordinate unit fixed by the consumer) this is the **per-face** analogue —
//! GeN-Foam's `timeFieldTableFvPatchScalarField` doc notes plainly there was
//! no ready-made OpenFOAM `Table` class for a `scalarField`-valued table, so
//! it hand-rolled this one; the same gap exists here, so [`TimeFieldTable`]
//! fills it.
//!
//! ## Implementation: reuses [`ScalarInterpolateTable`]
//!
//! Rather than re-deriving the clamp/interpolate arithmetic, [`TimeFieldTable`]
//! builds one [`ScalarInterpolateTable`] per face column (all sharing the same
//! abscissa — the tabulated times) with
//! [`InterpolationMethod::Linear`] and [`OutOfBounds::Fixed`] (clamp to the
//! nearest tabulated row), which is exactly upstream's `t < t0` /
//! `t > tN` / linear-in-between behaviour. This is a straight reuse of an
//! already-verified building block (see
//! `crate::genfoam::common::interpolate_table`'s own tests), not a new
//! numerical implementation.
//!
//! ## Dimension convention
//!
//! Like [`TimeProfile`](super::super::super::common::TimeProfile), the
//! abscissa (time) is [`Time`](uom::si::f64::Time) and each face's ordinate is
//! a raw `f64` whose physical unit the consumer fixes — the same generic
//! table serves any scalar boundary field (a temperature ramp, a heat-flux
//! schedule, …).
//!
//! ## What this is (and is not)
//!
//! A **physical-value closure** over a `Vec<f64>` per query time — not a full
//! `fvPatchField`. Reading the dictionary's nested `(t (v0 v1 ...))` list
//! syntax and writing it back out is case I/O, out of scope here; this module
//! only reproduces the lookup-and-interpolate behaviour once the table has
//! been parsed into `(times, values)`.
//!
//! A table needs at least **two** time rows to interpolate between (the same
//! floor [`ScalarInterpolateTable`] enforces); a single-row table is a
//! constant field and does not need this machinery.

use uom::si::f64::Time;
use uom::si::time::second;

use crate::genfoam::common::interpolate_table::{
    InterpolateTableError, InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
};

/// Errors constructing a [`TimeFieldTable`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TimeFieldTableError {
    /// The `times` and `values` (row) vectors had different lengths.
    #[error("time-field table has mismatched lengths: {times} times vs {rows} value rows")]
    LengthMismatch {
        /// Number of tabulated time entries.
        times: usize,
        /// Number of value rows supplied.
        rows: usize,
    },

    /// A value row had a different number of faces than row 0.
    #[error(
        "time-field table row {index} has {got} face values, expected {expected} (from row 0)"
    )]
    InconsistentRowLength {
        /// Index of the offending row.
        index: usize,
        /// Face count established by row 0.
        expected: usize,
        /// Face count actually found in this row.
        got: usize,
    },

    /// The per-face interpolation table could not be built — wraps the
    /// underlying [`InterpolateTableError`] (too few time points, or the
    /// times were not strictly ascending). Since every face shares the same
    /// time axis, this only ever originates from the shared `times` column.
    #[error("time-field table time axis is invalid: {0}")]
    Table(#[from] InterpolateTableError),
}

/// A per-face scalar boundary field tabulated against time.
///
/// See the [module documentation](self) for the table layout, the reuse of
/// [`ScalarInterpolateTable`] for the interior arithmetic, and the dimension
/// convention (abscissa is [`Time`]; each face's ordinate is a raw `f64` the
/// consumer's unit). Build with [`TimeFieldTable::new`]; query with
/// [`TimeFieldTable::value`].
#[derive(Debug, Clone, PartialEq)]
pub struct TimeFieldTable {
    /// One interpolation table per face, all built over the same (shared)
    /// tabulated times.
    per_face: Vec<ScalarInterpolateTable>,
}

impl TimeFieldTable {
    /// Build a table from tabulated rows `(times[i], values[i])`, where
    /// `values[i]` is the per-face field at `times[i]` (one entry per face,
    /// same length for every row).
    ///
    /// # Parameters
    /// - `times` — strictly-ascending tabulated times, length >= 2.
    /// - `values` — one row per time, each row holding one value per face
    ///   (all rows the same length — the face count).
    ///
    /// # Errors
    /// - [`TimeFieldTableError::LengthMismatch`] if `times.len() !=
    ///   values.len()`.
    /// - [`TimeFieldTableError::InconsistentRowLength`] if any row's face
    ///   count differs from row 0's.
    /// - [`TimeFieldTableError::Table`] if `times` has fewer than two points
    ///   or is not strictly ascending.
    pub fn new(times: Vec<Time>, values: Vec<Vec<f64>>) -> Result<Self, TimeFieldTableError> {
        if times.len() != values.len() {
            return Err(TimeFieldTableError::LengthMismatch {
                times: times.len(),
                rows: values.len(),
            });
        }
        // Validate the shared time axis up front so a zero-face table (an
        // edge case with no rows to walk below) still rejects a malformed
        // axis, matching ScalarInterpolateTable's own floor.
        if times.len() < 2 {
            return Err(TimeFieldTableError::Table(
                InterpolateTableError::TooFewPoints(times.len()),
            ));
        }

        let n_faces = values[0].len();
        for (index, row) in values.iter().enumerate() {
            if row.len() != n_faces {
                return Err(TimeFieldTableError::InconsistentRowLength {
                    index,
                    expected: n_faces,
                    got: row.len(),
                });
            }
        }

        let times_s: Vec<f64> = times.iter().map(|t| t.get::<second>()).collect();

        let mut per_face = Vec::with_capacity(n_faces);
        for face in 0..n_faces {
            let ys: Vec<f64> = values.iter().map(|row| row[face]).collect();
            let table = ScalarInterpolateTable::new(
                times_s.clone(),
                ys,
                InterpolationMethod::Linear,
                OutOfBounds::Fixed,
            )?;
            per_face.push(table);
        }

        Ok(Self { per_face })
    }

    /// The number of faces this table covers.
    #[must_use]
    pub fn n_faces(&self) -> usize {
        self.per_face.len()
    }

    /// The per-face field at time `t`: linear interpolation between the
    /// bracketing tabulated rows, clamped to the first/last row outside the
    /// tabulated time range (upstream's `t < t0` / `t > tN` behaviour).
    ///
    /// Returns one value per face, in table order.
    #[must_use]
    pub fn value(&self, t: Time) -> Vec<f64> {
        let x = t.get::<second>();
        self.per_face
            .iter()
            .map(|table| {
                table
                    .interpolate(x)
                    .expect("OutOfBounds::Fixed never errors")
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    #[test]
    fn linear_interpolation_between_rows() {
        // 2 faces, 3 time rows.
        let table = TimeFieldTable::new(
            vec![s(0.0), s(1.0), s(2.0)],
            vec![vec![0.0, 10.0], vec![2.0, 12.0], vec![6.0, 20.0]],
        )
        .unwrap();

        assert_eq!(table.n_faces(), 2);
        assert_eq!(table.value(s(0.0)), vec![0.0, 10.0]);
        assert_eq!(table.value(s(0.5)), vec![1.0, 11.0]); // midpoint of row 0/1
        assert_eq!(table.value(s(1.5)), vec![4.0, 16.0]); // midpoint of row 1/2
        assert_eq!(table.value(s(2.0)), vec![6.0, 20.0]);
    }

    #[test]
    fn clamps_outside_tabulated_range() {
        let table = TimeFieldTable::new(
            vec![s(1.0), s(3.0)],
            vec![vec![10.0, -5.0], vec![30.0, -15.0]],
        )
        .unwrap();

        assert_eq!(table.value(s(-100.0)), vec![10.0, -5.0]); // before t0 -> row 0
        assert_eq!(table.value(s(100.0)), vec![30.0, -15.0]); // after tN -> last row
    }

    #[test]
    fn length_mismatch_rejected() {
        let err = TimeFieldTable::new(vec![s(0.0), s(1.0)], vec![vec![0.0]]).unwrap_err();
        assert_eq!(
            err,
            TimeFieldTableError::LengthMismatch { times: 2, rows: 1 }
        );
    }

    #[test]
    fn inconsistent_row_length_rejected() {
        let err =
            TimeFieldTable::new(vec![s(0.0), s(1.0)], vec![vec![0.0, 1.0], vec![2.0]]).unwrap_err();
        assert_eq!(
            err,
            TimeFieldTableError::InconsistentRowLength {
                index: 1,
                expected: 2,
                got: 1
            }
        );
    }

    #[test]
    fn too_few_time_points_rejected() {
        let err = TimeFieldTable::new(vec![s(0.0)], vec![vec![1.0, 2.0]]).unwrap_err();
        assert_eq!(
            err,
            TimeFieldTableError::Table(InterpolateTableError::TooFewPoints(1))
        );
    }

    #[test]
    fn zero_face_table_still_validates_time_axis() {
        // No faces (empty rows) is a degenerate but legal table; the time
        // axis must still be validated even though the per-face loop never
        // runs.
        let err = TimeFieldTable::new(vec![s(0.0)], vec![vec![]]).unwrap_err();
        assert_eq!(
            err,
            TimeFieldTableError::Table(InterpolateTableError::TooFewPoints(1))
        );

        let table = TimeFieldTable::new(vec![s(0.0), s(1.0)], vec![vec![], vec![]]).unwrap();
        assert_eq!(table.n_faces(), 0);
        assert_eq!(table.value(s(0.5)), Vec::<f64>::new());
    }
}
