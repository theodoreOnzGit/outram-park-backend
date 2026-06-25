// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! MUSCL / TVD limited reconstruction of cell fields to face values.
//!
//! Provides the owner-biased (`pos`) and neighbour-biased (`neg`) reconstructed
//! face values used by density-based central-upwind solvers (rhoCentralFoam) to
//! reach second order, à la OpenFOAM's `interpolate(field, pos)` /
//! `interpolate(field, neg)`.

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use super::{grad, interpolate};

/// Slope / flux limiter for MUSCL reconstruction.
///
/// `λ(r)` blends between first-order upwind (`λ = 0`) and central differencing
/// (`λ = 1`); TVD limiters choose `λ(r)` from the local slope ratio `r` so the
/// reconstruction stays monotone (no new extrema) near discontinuities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limiter {
    /// First-order upwind: `λ = 0`.
    Upwind,
    /// Unlimited central differencing (2nd order, may oscillate): `λ = 1`.
    Linear,
    /// van Leer TVD limiter: `λ(r) = (r + |r|) / (1 + |r|)`.
    VanLeer,
    /// minmod TVD limiter: `λ(r) = max(0, min(r, 1))`.
    Minmod,
}

impl Limiter {
    #[inline]
    fn lambda(self, r: f64) -> f64 {
        match self {
            Limiter::Upwind  => 0.0,
            Limiter::Linear  => 1.0,
            Limiter::VanLeer => (r + r.abs()) / (1.0 + r.abs()),
            Limiter::Minmod  => 0.0_f64.max(r.min(1.0)),
        }
    }
}

/// MUSCL reconstruction of `field` to owner-biased (`pos`) and neighbour-biased
/// (`neg`) face values.
///
/// For each internal face (owner `O`, neighbour `N`, `d = C_N − C_O`):
/// ```text
/// gradf  = φ_N − φ_O
/// r_pos  = 2·(d·∇φ_O)/gradf − 1      (owner is the upwind side)
/// r_neg  = 2·(d·∇φ_N)/gradf − 1      (neighbour is the upwind side)
/// φ_pos  = φ_O + λ(r_pos)·(φ_lin − φ_O)
/// φ_neg  = φ_N + λ(r_neg)·(φ_lin − φ_N)
/// ```
/// where `φ_lin` is the geometric linear (central) face interpolation. `λ = 0`
/// reduces each side to first-order upwind (the previous rhoCentralFoam
/// behaviour); `λ = 1` gives central differencing. Mirrors `NVDTVD::r` plus the
/// limited-scheme weight blend in
/// `src/finiteVolume/interpolation/surfaceInterpolation/limitedSchemes/`.
///
/// Boundary faces are reconstructed first-order (both sides take the patch face
/// value), matching OpenFOAM's treatment of `interpolate(_, pos/neg)` at
/// boundaries.
pub fn reconstruct_pos_neg(
    field: &VolScalarField,
    limiter: Limiter,
) -> (SurfaceScalarField, SurfaceScalarField) {
    let mesh   = field.mesh.clone();
    let grad_c = grad(field);       // cell gradients ∇φ
    let lin    = interpolate(field); // central face values (+ boundary)
    let phi    = field.internal.as_slice();
    let gc     = grad_c.internal.as_slice();
    const TINY: f64 = 1e-30;

    let mut pos = vec![0.0_f64; mesh.n_internal_faces];
    let mut neg = vec![0.0_f64; mesh.n_internal_faces];

    for f in 0..mesh.n_internal_faces {
        let o  = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let gradf = phi[nb] - phi[o];

        if gradf.abs() < TINY {
            // No jump across the face: limiter is irrelevant, both sides equal
            // their own cell value.
            pos[f] = phi[o];
            neg[f] = phi[nb];
            continue;
        }

        let d       = mesh.cell_centres[nb] - mesh.cell_centres[o];
        let phi_lin = lin.internal[f];
        let r_pos   = 2.0 * d.dot(gc[o])  / gradf - 1.0;
        let r_neg   = 2.0 * d.dot(gc[nb]) / gradf - 1.0;

        pos[f] = phi[o]  + limiter.lambda(r_pos) * (phi_lin - phi[o]);
        neg[f] = phi[nb] + limiter.lambda(r_neg) * (phi_lin - phi[nb]);
    }

    let boundary = || -> Vec<PatchField<f64>> {
        lin.boundary.iter()
            .map(|lb| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: lb.values.clone(),
            })
            .collect()
    };

    let pos_field = SurfaceScalarField::new(
        format!("{}_pos", field.name), mesh.clone(), Field::new(pos), boundary());
    let neg_field = SurfaceScalarField::new(
        format!("{}_neg", field.name), mesh, Field::new(neg), boundary());
    (pos_field, neg_field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

    // 3-cell 1-D mesh, dx = 1, cells centred at 0.5, 1.5, 2.5.
    fn line_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(3).n_internal_faces(2)
            .owner(vec![0, 1, 0, 2])
            .neighbour(vec![1, 2])
            .patches(vec![
                BoundaryPatch::new("left",  2, 1, PatchKind::Patch),
                BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![1.0, 1.0, 1.0])
            .cell_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(1.5, 0.0, 0.0),
                Vector3::new(2.5, 0.0, 0.0),
            ])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(3.0, 0.0, 0.0),
            ])
            .build().unwrap())
    }

    #[test]
    fn linear_field_reconstructs_to_central_value() {
        // φ = x (linear): both biased reconstructions equal the linear face
        // value (van Leer → λ = 1, no limiting). Fixed-value boundaries carry
        // the true φ at the patch faces (x = 0 → 0, x = 3 → 3) so the Gauss
        // cell gradients are exact at the boundary cells.
        let m = line_mesh();
        let mut f = VolScalarField::zeros("phi", m.clone());
        f.internal = Field::new(vec![0.5, 1.5, 2.5]); // φ = x at cell centres
        f.boundary[0].bc = BoundaryCondition::FixedValue(0.0); // left  face x = 0
        f.boundary[0].values = Field::new(vec![0.0]);
        f.boundary[1].bc = BoundaryCondition::FixedValue(3.0); // right face x = 3
        f.boundary[1].values = Field::new(vec![3.0]);
        let (pos, neg) = reconstruct_pos_neg(&f, Limiter::VanLeer);
        // Internal face 0 is at x = 1.0 → φ = 1.0
        assert!((pos.internal[0] - 1.0).abs() < 1e-9, "pos = {}", pos.internal[0]);
        assert!((neg.internal[0] - 1.0).abs() < 1e-9, "neg = {}", neg.internal[0]);
    }

    #[test]
    fn upwind_limiter_returns_cell_values() {
        // λ = 0 → pos = owner value, neg = neighbour value (first order).
        let m = line_mesh();
        let mut f = VolScalarField::zeros("phi", m.clone());
        f.internal = Field::new(vec![1.0, 5.0, 9.0]);
        let (pos, neg) = reconstruct_pos_neg(&f, Limiter::Upwind);
        assert!((pos.internal[0] - 1.0).abs() < 1e-12); // owner of face 0
        assert!((neg.internal[0] - 5.0).abs() < 1e-12); // neighbour of face 0
    }

    #[test]
    fn vanleer_clamps_at_discontinuity() {
        // Sharp jump 0,0,1: van Leer must not introduce a new extremum, i.e.
        // the reconstructed face values stay within [min, max] of the stencil.
        let m = line_mesh();
        let mut f = VolScalarField::zeros("phi", m.clone());
        f.internal = Field::new(vec![0.0, 0.0, 1.0]);
        let (pos, neg) = reconstruct_pos_neg(&f, Limiter::VanLeer);
        for f in 0..2 {
            assert!(pos.internal[f] >= -1e-12 && pos.internal[f] <= 1.0 + 1e-12);
            assert!(neg.internal[f] >= -1e-12 && neg.internal[f] <= 1.0 + 1e-12);
        }
    }
}
