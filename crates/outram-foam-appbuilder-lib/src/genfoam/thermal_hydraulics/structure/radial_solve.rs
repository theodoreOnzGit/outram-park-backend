// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam) and OpenFOAM-ESI v2506
//   Upstream: GeN-Foam src/classes/thermalHydraulics/src/include/IOFieldField/IOFieldField.{H,C}
//             (`oldTime`, `storeOldTime`), powerModels/{heatedPin,nuclearFuelPin}.C
//             (`updateLocalAvgGlobalMinMaxT`); OpenFOAM src/OpenFOAM/matrices/scalarMatrices/
//             scalarMatricesTemplates.C (`Foam::solve(Field&, SquareMatrix, List)`)
//   Upstream commit: 652b3da (GeN-Foam); OpenFOAM-ESI v2506
//   Upstream copyright: (C) 2015-2022 EPFL (GeN-Foam); (C) OpenCFD Ltd / OpenFOAM Foundation
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL); OpenFOAM contributors
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `radial_solve` — what the four radial-conduction power models share (gh:#1230)
//!
//! Three upstream pieces, each translated line for line because bit identity with the real binary needs
//! the same operations in the same order:
//!
//! * [`foam_solve`]: `Foam::solve(Field&, SquareMatrix, List)`, **Gaussian elimination with partial
//!   pivoting and a `1e-20` singularity test**, not `LUDecompose` (the workspace's
//!   `SquareMatrix::lu_decompose` is the other routine and rounds differently; the V&V test carries it as a
//!   named control).
//! * [`OldTimeFieldField`]: `IOFieldField::oldTime()` with its first-call behaviour (the first call creates
//!   the old-time copy and does not set `timeIndex_`, so a second `correct` in the first time step gives the
//!   first cell the state it already had after its first update). The start-of-step form is kept callable
//!   ([`OldTimePolicy::StartOfStep`]) as the control that shows the quirk matters.
//! * [`update_average_and_extrema`]: `updateLocalAvgGlobalMinMaxT`, the `r dr`-weighted average with
//!   half-weight end nodes and the running min/max.
//!
//! Units are SI base units in plain `f64` (the radial model works on radii in m, `W/m^3`, `W/m^2/K`, K),
//! as `nuclear_steady_state_pebble` does.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use core::fmt;

/// `constant::mathematical::pi` of OpenFOAM (a `double`, equal to `core::f64::consts::PI`).
pub const PI: f64 = core::f64::consts::PI;

/// `SMALL` of OpenFOAM in double precision.
pub const SMALL: f64 = 1.0e-15;

/// Why a radial power model refused or failed.
#[derive(Debug, Clone, PartialEq)]
pub enum RadialError {
    /// `Foam::solve`: a pivot below `1e-20` in magnitude ("Singular Matrix").
    SingularMatrix,
    /// An input the upstream constructor would reject, or one the port refuses by name.
    Invalid(&'static str),
}

impl fmt::Display for RadialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingularMatrix => write!(f, "Singular Matrix (Foam::solve pivot below 1e-20)"),
            Self::Invalid(why) => write!(f, "{why}"),
        }
    }
}

/// `sqr(x)` of OpenFOAM.
#[must_use]
pub fn sqr(x: f64) -> f64 {
    x * x
}

/// `Foam::solve(List<Type>& psi, const scalarSquareMatrix& matrix, const List<Type>& source)` for `Type =
/// scalar`: `psi = source`, then elimination with partial pivoting on a copy of `matrix` (row-major, `n x n`),
/// then back-substitution. Returns `psi`.
///
/// # Errors
/// [`RadialError::SingularMatrix`] when a pivot is below `1e-20` in magnitude.
pub fn foam_solve(matrix: &[f64], n: usize, source: &[f64]) -> Result<Vec<f64>, RadialError> {
    let mut m: Vec<f64> = matrix.to_vec();
    let mut s: Vec<f64> = source.to_vec();
    if m.len() != n * n || s.len() != n {
        return Err(RadialError::Invalid("foam_solve: matrix or source of the wrong size"));
    }
    // Elimination
    for i in 0..n {
        let mut i_max = i;
        let mut largest = m[i_max * n + i].abs();
        // Swap elements around to find a good pivot
        for j in (i + 1)..n {
            if m[j * n + i].abs() > largest {
                i_max = j;
                largest = m[i_max * n + i].abs();
            }
        }
        if i != i_max {
            for k in i..n {
                m.swap(i * n + k, i_max * n + k);
            }
            s.swap(i, i_max);
        }
        // Check that the system of equations isn't singular
        if m[i * n + i].abs() < 1e-20 {
            return Err(RadialError::SingularMatrix);
        }
        // Reduce to upper triangular form
        for j in (i + 1)..n {
            s[j] -= s[i] * (m[j * n + i] / m[i * n + i]);
            for k in (i..n).rev() {
                m[j * n + k] -= m[i * n + k] * m[j * n + i] / m[i * n + i];
            }
        }
    }
    // Back-substitution
    for j in (0..n).rev() {
        let mut ntempvec = 0.0_f64;
        for k in (j + 1)..n {
            ntempvec += m[j * n + k] * s[k];
        }
        s[j] = (s[j] - ntempvec) / m[j * n + j];
    }
    Ok(s)
}

/// The same system solved with the workspace's `SquareMatrix::lu_decompose` (`Foam::LUDecompose` +
/// `LUBacksubstitute`): NOT what GeN-Foam calls. Kept as the control that shows the elimination order is
/// load-bearing for bit identity.
///
/// # Errors
/// [`RadialError::SingularMatrix`] for an exactly singular matrix.
pub fn lu_control_solve(matrix: &[f64], n: usize, source: &[f64]) -> Result<Vec<f64>, RadialError> {
    let mut a = outram_foam_basic_lib::prelude::SquareMatrix::new(n);
    for i in 0..n {
        for j in 0..n {
            a.set(i, j, matrix[i * n + j]);
        }
    }
    a.solve(source).map_err(|_| RadialError::SingularMatrix)
}

/// How the old-time state of a per-cell profile is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OldTimePolicy {
    /// `IOFieldField::oldTime()` as upstream (the default): the first call creates the copy without setting
    /// the stored time index, so the old time of the first cell read in a second `correct` of the first time
    /// step is its already-updated state.
    UpstreamQuirk,
    /// The old time is the state at the end of the previous time index, always. DEPRECATED control.
    StartOfStep,
}

/// `IOFieldField<Field, scalar>` with `oldTime()` (a per-cell list of node values).
#[derive(Debug, Clone, PartialEq)]
pub struct OldTimeFieldField {
    /// The current values, one `Vec` per mesh cell (empty for a cell outside the model).
    pub current: Vec<Vec<f64>>,
    field0: Option<Vec<Vec<f64>>>,
    time_index: i64,
    policy: OldTimePolicy,
    /// `StartOfStep` bookkeeping: the time index at which `field0` was last stored.
    stored_index: i64,
}

impl OldTimeFieldField {
    /// Construct at time index `time_index` (`timeIndex_(this->time().timeIndex())`).
    #[must_use]
    pub fn new(current: Vec<Vec<f64>>, time_index: i64) -> Self {
        Self { current, field0: None, time_index, policy: OldTimePolicy::UpstreamQuirk, stored_index: time_index }
    }

    /// Select the old-time policy (the control uses [`OldTimePolicy::StartOfStep`]).
    #[must_use]
    pub fn with_policy(mut self, policy: OldTimePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// `oldTime()[celli]` at the current time index `now`: the old-time values of cell `cell`.
    pub fn old_time_of(&mut self, cell: usize, now: i64) -> Vec<f64> {
        match self.policy {
            OldTimePolicy::UpstreamQuirk => {
                if self.field0.is_none() {
                    // `field0Ptr_ = new IOFieldField(*this)`: a copy, `timeIndex_` untouched
                    self.field0 = Some(self.current.clone());
                } else if self.time_index != now {
                    // `storeOldTime()`
                    self.field0 = Some(self.current.clone());
                    self.time_index = now;
                }
            }
            OldTimePolicy::StartOfStep => {
                if self.field0.is_none() || self.stored_index != now {
                    // the state when the first cell of this time index is read is the end of the last index
                    // only if no cell was updated yet; the control snapshots once per index, before any update
                    self.field0 = Some(self.current.clone());
                    self.stored_index = now;
                }
            }
        }
        match &self.field0 {
            Some(f) => f[cell].clone(),
            None => Vec::new(),
        }
    }
}

/// `updateLocalAvgGlobalMinMaxT`: the `r dr`-weighted average of `trad[start..end]` (end nodes at half
/// weight) and the running extrema. Returns `Tav`.
pub fn update_average_and_extrema(
    start: usize,
    end: usize,
    r: &[f64],
    dr: f64,
    trad: &[f64],
    t_min: &mut f64,
    t_max: &mut f64,
) -> f64 {
    let mut intr = 0.0_f64;
    let mut int_tr = 0.0_f64;
    for j in start..end {
        let t = trad[j];
        let rdr = r[j] * dr;
        // cells at the mesh ends are only half as wide
        if j == start || j == end - 1 {
            intr += rdr / 2.0;
            int_tr += t * rdr / 2.0;
        } else {
            intr += rdr;
            int_tr += t * rdr;
        }
        if t > *t_max {
            *t_max = t;
        }
        if t < *t_min {
            *t_min = t;
        }
    }
    int_tr / intr
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `foam_solve` solves a small pivoting system (the first pivot is zero) to round-off.
    #[test]
    fn foam_solve_pivots_and_solves() {
        // [0 2; 3 1] x = [4; 5]  ->  x = [1, 2]
        let x = foam_solve(&[0.0, 2.0, 3.0, 1.0], 2, &[4.0, 5.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-15 && (x[1] - 2.0).abs() < 1e-15, "{x:?}");
        let y = lu_control_solve(&[0.0, 2.0, 3.0, 1.0], 2, &[4.0, 5.0]).unwrap();
        assert!((y[0] - 1.0).abs() < 1e-14 && (y[1] - 2.0).abs() < 1e-14, "{y:?}");
    }

    /// The upstream singularity test: a zero pivot column is refused, not divided by.
    #[test]
    fn foam_solve_refuses_singular() {
        assert_eq!(foam_solve(&[1.0, 2.0, 2.0, 4.0], 2, &[1.0, 2.0]), Err(RadialError::SingularMatrix));
        assert!(foam_solve(&[1.0], 2, &[1.0]).is_err());
        assert!(format!("{}", RadialError::Invalid("x")).contains('x'));
    }

    /// `oldTime()` quirk: first call copies, second call in the SAME time index (first cell) returns the
    /// updated state; the `StartOfStep` control does not.
    #[test]
    fn old_time_quirk_versus_start_of_step() {
        for (policy, expect_second) in [(OldTimePolicy::UpstreamQuirk, 9.0), (OldTimePolicy::StartOfStep, 1.0)] {
            let mut f = OldTimeFieldField::new(vec![vec![1.0], vec![2.0]], 0).with_policy(policy);
            // time index 1, first outer iteration: cell 0 then cell 1
            assert_eq!(f.old_time_of(0, 1), vec![1.0]);
            f.current[0][0] = 9.0;
            assert_eq!(f.old_time_of(1, 1), vec![2.0]);
            // second outer iteration of the same step: cell 0 asks again
            assert_eq!(f.old_time_of(0, 1)[0], expect_second, "{policy:?}");
        }
    }

    /// Averages: half weights at both ends, extrema updated.
    #[test]
    fn average_and_extrema() {
        let (mut lo, mut hi) = (1e69, 0.0);
        let tav = update_average_and_extrema(0, 3, &[1.0, 2.0, 3.0], 0.5, &[10.0, 20.0, 30.0], &mut lo, &mut hi);
        // intr = 0.5/2 + 1.0 + 1.5/2 = 2.0 ; intTr = 10*0.5/2 + 20*1.0 + 30*1.5/2 = 45
        assert!((tav - 22.5).abs() < 1e-14, "{tav}");
        assert_eq!((lo, hi), (10.0, 30.0));
    }
}
