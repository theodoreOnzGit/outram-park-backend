// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Helpers shared by the verification tests.
//!
//! Included with `#[path]` rather than being a crate module, so it does not
//! appear in the published API. Everything here is test scaffolding.

#![allow(dead_code)]

use farrer_park::prelude::*;

/// Deterministic pseudo-random number in `[-1, 1)` from two integer seeds.
///
/// A splitmix-style mix of the two seeds. Deterministic is the point: a patch
/// test that jitters its mesh differently on every run cannot be reproduced
/// when it fails.
pub fn jitter(i: usize, k: usize) -> f64 {
    let mut h = (i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((k as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    ((h >> 33) as f64) / 2_147_483_648.0 * 2.0 - 1.0
}

/// Conjugate gradients with ILU(0) at a chosen relative-residual tolerance.
///
/// How tight this can usefully be set depends on the conditioning of the
/// system, which grows like `1 / h^2`: on the small patch-test meshes `1e-13`
/// is reached comfortably, and on the finest manufactured-solution mesh
/// (2178 degrees of freedom) the attainable floor is nearer `1e-12`. Asking
/// for more than the arithmetic can deliver makes the solver report a genuine
/// non-convergence, which is the right behaviour and not something to suppress.
pub fn linear_settings(tolerance: f64) -> LinearSolverSettings {
    LinearSolverSettings {
        method: KrylovMethod::ConjugateGradient,
        preconditioner: PreconditionerChoice::Ilu0,
        tolerance,
        max_iter: 20_000,
        restart: 50,
    }
}

/// Newton settings for a linear-elastic verification case, with explicit
/// tolerances.
pub fn elastic_newton_with(
    linear_tolerance: f64,
    residual_tolerance: f64,
    increment_tolerance: f64,
) -> NewtonSettings {
    NewtonSettings {
        max_iterations: 8,
        residual_tolerance,
        increment_tolerance,
        n_load_steps: 1,
        max_cutbacks: 0,
        dirichlet_method: DirichletMethod::Elimination,
        linear: linear_settings(linear_tolerance),
    }
}

/// Newton settings for the small, well-conditioned patch-test systems: linear
/// solves to `1e-13`, Newton residual to `1e-11`.
pub fn elastic_newton() -> NewtonSettings {
    elastic_newton_with(1.0e-13, 1.0e-11, 1.0e-8)
}

/// Newton settings for the manufactured-solution meshes, whose conditioning
/// grows with refinement: linear solves to `1e-11`, Newton residual to `1e-9`.
///
/// Both are six or more orders of magnitude below the discretisation errors
/// being measured (`1e-4` to `1e-1`), so the linear solve cannot contaminate
/// the observed convergence order.
pub fn mms_newton() -> NewtonSettings {
    elastic_newton_with(1.0e-11, 1.0e-9, 1.0e-6)
}

/// Observed convergence order between two mesh levels:
/// `log(e_coarse / e_fine) / log(h_coarse / h_fine)`.
pub fn order(e_coarse: f64, e_fine: f64, h_ratio: f64) -> f64 {
    (e_coarse / e_fine).ln() / h_ratio.ln()
}

/// Largest relative difference between two slices, normalised by the largest
/// absolute entry of the reference.
pub fn max_rel_diff(got: &[f64], want: &[f64]) -> f64 {
    let scale = want.iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1e-300);
    got.iter()
        .zip(want)
        .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()))
        / scale
}
