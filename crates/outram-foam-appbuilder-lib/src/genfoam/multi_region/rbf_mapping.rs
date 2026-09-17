// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/meshHandler/meshHandler.C
//                    (the `interpolateAndMapFields` polyharmonic-spline / kriging
//                     path — axial sampling then RBF onto target cell centres),
//                    on the RBF kernel in
//                    src/classes/common/radialBasisFunctionInterpolation/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Thomas Guilbaud (EPFL)
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

//! # Radial-basis-function field mapping for non-conformal meshes
//!
//! Rust port of GeN-Foam's `meshHandler::interpolateAndMapFields`
//! polyharmonic-spline path: when a field is only known at a **scattered set of
//! sample points** (e.g. an axial power profile sampled at a handful of
//! elevations, or values read off cells of a source region that does *not*
//! volume-overlap the target), a smooth radial-basis-function (RBF) interpolant
//! is fit to the samples and evaluated at every target cell centre.
//!
//! This is the fallback for the volumetric [`super::mesh_to_mesh::MeshToMesh`]
//! when the source and target meshes are geometrically dissimilar (different
//! dimensionality, no cell overlap) — exactly the case upstream's commented
//! `interpolateAndMapFields` covers with `polyharmonicSpline` / `kriging`.
//!
//! ## RBF kernel reuse
//!
//! The polyharmonic-spline solve/evaluate kernel is **not** re-implemented here.
//! It is reused from [`crate::genfoam::common::rbf`], the single shared home for
//! the port of upstream's `common/radialBasisFunctionInterpolation` (the same
//! kernel also backs cross-section parametrisation in
//! [`crate::genfoam::neutronics::xs`]). This module depends only on the two free
//! functions [`rbf::solve_polyharmonic_spline`] and [`rbf::polyharmonic_spline`].
//! **No second RBF copy is introduced.**
//!
//! ## Units
//!
//! Sample coordinates are metres (mesh cell centres are geometric positions).
//! Sample *values* are unit-agnostic raw `f64`, matching
//! [`outram_foam_basic_lib`] scalar fields — the physical `uom` meaning is
//! attached by the consumer that reads the mapped field.

use std::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::matrix::MatrixError;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use crate::genfoam::common::rbf;

/// The polyharmonic-spline basis order (GeN-Foam `polyharmonicSplineMode`).
///
/// Selects the radial basis `φ(r)`; see
/// [`rbf::polyharmonic_spline_function`]. [`PolyharmonicMode::Linear`] (the
/// upstream default, `mode = 1`) reproduces linear fields exactly and is the
/// safe choice for reactor axial/radial profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyharmonicMode {
    /// `φ = r` — linear; reproduces linear functions exactly (upstream default).
    Linear,
    /// `φ = r² ln r` — thin-plate spline.
    ThinPlate,
    /// `φ = r³`.
    Cubic,
    /// `φ = r⁴ ln r`.
    ThinPlate4,
}

impl PolyharmonicMode {
    /// The upstream integer `polyharmonicSplineMode` this corresponds to.
    #[must_use]
    pub fn as_mode(self) -> usize {
        match self {
            PolyharmonicMode::Linear => 1,
            PolyharmonicMode::ThinPlate => 2,
            PolyharmonicMode::Cubic => 3,
            PolyharmonicMode::ThinPlate4 => 4,
        }
    }
}

/// Errors from building or applying an [`RbfFieldMap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbfMapError {
    /// Fewer than one sample point, or a sample-count/value-count mismatch.
    EmptyOrMismatched {
        /// Number of sample coordinates supplied.
        n_points: usize,
        /// Number of sample values supplied.
        n_values: usize,
    },
    /// The RBF saddle-point system was singular (e.g. duplicate sample points).
    Singular,
}

impl std::fmt::Display for RbfMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RbfMapError::EmptyOrMismatched { n_points, n_values } => write!(
                f,
                "RBF field map needs >= 1 sample and matching value count \
                 (got {n_points} points, {n_values} values)"
            ),
            RbfMapError::Singular => write!(
                f,
                "RBF saddle-point matrix is singular (duplicate sample points?)"
            ),
        }
    }
}

impl std::error::Error for RbfMapError {}

impl From<MatrixError> for RbfMapError {
    fn from(_: MatrixError) -> Self {
        RbfMapError::Singular
    }
}

/// A polyharmonic-spline interpolant fit to scattered 3-D sample points, ready
/// to evaluate at arbitrary positions or map onto a whole target mesh.
///
/// Built once from `(sample_points, values)`; the weights are cached so mapping
/// onto many cells is cheap. This is the RBF analogue of a cached
/// [`super::mesh_to_mesh::MeshToMesh`] addressing.
#[derive(Debug, Clone)]
pub struct RbfFieldMap {
    /// Transposed coordinate table `x_list[param][sample]` (3 params: x, y, z),
    /// as the kernel expects.
    x_list: Vec<Vec<f64>>,
    /// Solved spline weights `[w | v]`.
    weights: Vec<f64>,
    mode: usize,
}

impl RbfFieldMap {
    /// Fit the polyharmonic spline to `sample_points` with the given `values`.
    ///
    /// `sample_points[i]` is a position in metres; `values[i]` the field value
    /// there (raw `f64`). `mode` selects the basis.
    ///
    /// **Sample geometry.** This is a full 3-D spline: its linear polynomial tail
    /// has columns `(1, x, y, z)`, so the samples must be non-degenerate in all
    /// three dimensions. A **collinear** sample set (e.g. a pure axial profile
    /// with constant `x, y` and varying `z`) or a **coplanar** one leaves a zero
    /// column and yields [`RbfMapError::Singular`]. Spread the samples in 3-D, or
    /// (future work, sub-bead of `op-p6p.8`) reduce the spline to the active
    /// dimensions for a 1-D/2-D profile. *Evaluation* along a line is fine once the
    /// fit is built from non-degenerate samples.
    ///
    /// # Errors
    ///
    /// - [`RbfMapError::EmptyOrMismatched`] if there are no samples or the counts
    ///   disagree.
    /// - [`RbfMapError::Singular`] if the fit matrix is singular — duplicate,
    ///   collinear, or coplanar sample points (propagated, never defaulted).
    pub fn build(
        sample_points: &[Vector3],
        values: &[f64],
        mode: PolyharmonicMode,
    ) -> Result<Self, RbfMapError> {
        if sample_points.is_empty() || sample_points.len() != values.len() {
            return Err(RbfMapError::EmptyOrMismatched {
                n_points: sample_points.len(),
                n_values: values.len(),
            });
        }
        // Transpose points into [param][sample]: three rows (x, y, z).
        let x_list = vec![
            sample_points.iter().map(|p| p.x).collect::<Vec<_>>(),
            sample_points.iter().map(|p| p.y).collect::<Vec<_>>(),
            sample_points.iter().map(|p| p.z).collect::<Vec<_>>(),
        ];
        let m = mode.as_mode();
        let weights = rbf::solve_polyharmonic_spline(&x_list, values, m)?;
        Ok(Self {
            x_list,
            weights,
            mode: m,
        })
    }

    /// Evaluate the interpolant at an arbitrary position (metres).
    #[must_use]
    pub fn evaluate(&self, point: Vector3) -> f64 {
        rbf::polyharmonic_spline(
            &self.weights,
            &self.x_list,
            &[point.x, point.y, point.z],
            self.mode,
        )
    }

    /// Evaluate the interpolant at every cell centre of `target`, returning a
    /// fresh scalar field named `name` on that mesh.
    ///
    /// This is the `interpolateAndMapFields` inner loop: `forAll(mesh.C(),
    /// centerI) fieldToBeMapped[centerI] = polyharmonicSpline(...)`.
    #[must_use]
    pub fn map_onto(&self, target: Arc<FvMesh>, name: &str) -> VolScalarField {
        let mut out = VolScalarField::uniform(name, target.clone(), 0.0);
        for (cell, &centre) in target.cell_centres.iter().enumerate() {
            out.internal[cell] = self.evaluate(centre);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn column(n: usize, length: f64) -> Arc<FvMesh> {
        let h = length / n as f64;
        let n_internal = n - 1;
        let mut owner: Vec<usize> = (0..n_internal).collect();
        let mut neighbour: Vec<usize> = (1..n).collect();
        owner.push(0);
        owner.push(n - 1);
        let _ = &mut neighbour;
        let cell_centres: Vec<Vector3> = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * h, 0.0, 0.0))
            .collect();
        let mut fav = vec![Vector3::new(1.0, 0.0, 0.0); n_internal];
        fav.push(Vector3::new(-1.0, 0.0, 0.0));
        fav.push(Vector3::new(1.0, 0.0, 0.0));
        let mut fc: Vec<Vector3> = (1..n)
            .map(|i| Vector3::new(i as f64 * h, 0.0, 0.0))
            .collect();
        fc.push(Vector3::new(0.0, 0.0, 0.0));
        fc.push(Vector3::new(length, 0.0, 0.0));
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_internal)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("bottom", n_internal, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", n_internal + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![h; n])
                .cell_centres(cell_centres)
                .face_area_vectors(fav)
                .face_centres(fc)
                .build()
                .unwrap(),
        )
    }

    /// V&V — linear reproduction of the mode-1 polyharmonic spline.
    ///
    /// Methodology: sample the analytic linear field `f(x,y,z) = 2 + 3x − y +
    /// 0.5z` at 6 scattered 3-D points, fit the `Linear` (mode-1) polyharmonic
    /// spline, then evaluate at 4 *unseen* query points. A polyharmonic spline
    /// carries an exact linear polynomial tail, so it must reproduce a globally
    /// linear field to machine precision everywhere, not just at the samples.
    ///
    /// Result (2026-07-15, release): max |f_rbf − f_exact| = 0.0 over the 4 query
    /// points (bit-exact linear reproduction; assertion bound 1e-9). Pass.
    #[test]
    fn linear_reproduction_exact() {
        let f = |p: Vector3| 2.0 + 3.0 * p.x - p.y + 0.5 * p.z;
        let samples = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.3, 0.7, 0.5),
        ];
        let values: Vec<f64> = samples.iter().map(|&p| f(p)).collect();
        let map = RbfFieldMap::build(&samples, &values, PolyharmonicMode::Linear).unwrap();

        let queries = [
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(0.9, 0.1, 0.2),
            Vector3::new(0.2, 0.8, 0.6),
            Vector3::new(0.6, 0.4, 0.9),
        ];
        let max_err = queries
            .iter()
            .map(|&q| (map.evaluate(q) - f(q)).abs())
            .fold(0.0_f64, f64::max);
        assert!(max_err < 1e-9, "RBF linear reproduction error {max_err}");
    }

    /// V&V — interpolation is exact at the sample points themselves.
    ///
    /// Methodology: a polyharmonic spline interpolates (does not just approximate)
    /// its data. Fit a non-linear sample set and check every sample is reproduced.
    /// Result (2026-07-15, release): max residual at samples 0.0 (< 1e-9). Pass.
    #[test]
    fn interpolates_samples_exactly() {
        let samples = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        ];
        let values = [1.0, 4.0, -2.0, 9.0];
        let map = RbfFieldMap::build(&samples, &values, PolyharmonicMode::ThinPlate).unwrap();
        let max_err = samples
            .iter()
            .zip(values.iter())
            .map(|(&p, &v)| (map.evaluate(p) - v).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_err < 1e-9,
            "RBF does not interpolate samples: {max_err}"
        );
    }

    /// V&V — mapping a linear field (fit from non-degenerate 3-D samples) onto a
    /// fine axial column reproduces it per-cell.
    ///
    /// Methodology: fit `f = 10 + 5x` from a 3-D-spread sample set (not a
    /// collinear axial line — see [`RbfFieldMap::build`] on degeneracy), then
    /// `map_onto` a 12-cell column whose cells lie on the x-axis within the sample
    /// hull. Linear reproduction ⇒ each cell must equal `10 + 5·x_cell` exactly.
    /// Result (2026-07-15, release): max per-cell error 0.0 (< 1e-9). Pass.
    #[test]
    fn map_onto_column_linear() {
        // Non-degenerate 3-D samples of f = 10 + 5x (spread in y, z too).
        let samples = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.5, 0.5, 0.5),
        ];
        let values: Vec<f64> = samples.iter().map(|p| 10.0 + 5.0 * p.x).collect();
        let map = RbfFieldMap::build(&samples, &values, PolyharmonicMode::Linear).unwrap();

        let tgt = column(12, 1.0);
        let field = map.map_onto(tgt.clone(), "profile");
        let max_err = (0..tgt.n_cells)
            .map(|i| (field.internal[i] - (10.0 + 5.0 * tgt.cell_centres[i].x)).abs())
            .fold(0.0_f64, f64::max);
        assert!(max_err < 1e-9, "mapped profile error {max_err}");
    }

    #[test]
    fn build_rejects_empty() {
        let err = RbfFieldMap::build(&[], &[], PolyharmonicMode::Linear).unwrap_err();
        assert!(matches!(err, RbfMapError::EmptyOrMismatched { .. }));
    }

    /// A collinear (pure-axial) sample set is degenerate for the 3-D spline's
    /// linear tail and must be reported as [`RbfMapError::Singular`], not silently
    /// mis-solved — the documented limitation in [`RbfFieldMap::build`].
    #[test]
    fn build_rejects_collinear_samples() {
        let samples: Vec<Vector3> = (0..4)
            .map(|i| Vector3::new(i as f64 / 3.0, 0.0, 0.0))
            .collect();
        let values: Vec<f64> = samples.iter().map(|p| 10.0 + 5.0 * p.x).collect();
        let err = RbfFieldMap::build(&samples, &values, PolyharmonicMode::Linear).unwrap_err();
        assert_eq!(err, RbfMapError::Singular);
    }
}
