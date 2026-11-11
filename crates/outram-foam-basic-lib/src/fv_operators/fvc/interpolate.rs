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

use std::ops::{Add, Div, Mul, Sub};

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceField;
use crate::fields::vol_field::VolField;
use crate::fv_operators::fvm::DeltaCoeff;
use crate::mesh::fv_mesh::{FvMesh, PatchKind};

/// Linear interpolation weight for the **owner** cell at internal face `f`:
/// OpenFOAM's `surfaceInterpolation::makeWeights` (OpenFOAM-dev
/// `852527377c7e`,
/// `src/finiteVolume/interpolation/surfaceInterpolation/surfaceInterpolation/surfaceInterpolation.C`),
///
/// `w = |S_f·(C_N − C_f)| / (|S_f·(C_f − C_O)| + |S_f·(C_N − C_f)|)`,
///
/// the distances measured along the face normal, falling back to the
/// distance form `|C_N − C_f| / (|C_f − C_O| + |C_N − C_f|)` when the
/// projections vanish (upstream's `SfdNei/vGreat < SfdOwnNei` test), and to
/// 0.5 when both vanish.
///
/// ~~`w = |C_N − C_f| / |C_N − C_O|`~~ **CORRECTED 2026-11-04 (gh:#1202):**
/// that was the port's form, which is neither of upstream's: it differs
/// wherever the face centre is off the line between the cell centres
/// (mean |Δw| 4e-3, max 0.16 on the Foam Studio O-grid; mean 1.8e-2 on the
/// cfMesh dual). Every interpolated field moved with the fix; the re-measured
/// numbers are in `outram-foam-appbuilder-lib`'s
/// `verification_and_validation/laminar_pipe_gh1155.md`.
#[inline]
pub fn linear_weight(mesh: &FvMesh, f: usize) -> f64 {
    let co = mesh.cell_centres[mesh.owner[f]];
    let cn = mesh.cell_centres[mesh.neighbour[f]];
    let cf = mesh.face_centres[f];
    let s = mesh.face_area_vectors[f];
    let sfd_own = s.dot(cf - co).abs();
    let sfd_nei = s.dot(cn - cf).abs();
    let sum = sfd_own + sfd_nei;
    if sfd_nei / 1.0e300 < sum {
        return sfd_nei / sum;
    }
    let (d_own, d_nei) = ((cf - co).mag(), (cn - cf).mag());
    if d_own + d_nei > 0.0 {
        d_nei / (d_own + d_nei)
    } else {
        0.5
    }
}

/// Linear interpolation of a `VolField` to face centres → `SurfaceField`.
///
/// Internal faces: `φ_f = w·(φ_O − φ_N) + φ_N` (geometric weighting; upstream's form,
/// gh:#1489, equal to `w·φ_O + (1−w)·φ_N` up to an ulp).
/// Boundary faces: evaluated from the patch BC (zero-gradient → cell value,
/// fixed-value → the fixed value, etc.).
///
/// ```rust
/// use outram_foam_basic_lib::prelude::*;
/// use outram_foam_basic_lib::fv_operators::fvc;
/// use std::sync::Arc;
///
/// let mesh = Arc::new(
///     FvMeshBuilder::new()
///         .n_cells(2).n_internal_faces(1)
///         .owner(vec![0, 1, 0]).neighbour(vec![1])
///         .patches(vec![
///             BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
///             BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
///         ])
///         .cell_volumes(vec![1.0, 1.0])
///         .cell_centres(vec![
///             Vector3::new(0.25, 0.0, 0.0),
///             Vector3::new(0.75, 0.0, 0.0),
///         ])
///         .face_area_vectors(vec![
///             Vector3::new(1.0, 0.0, 0.0),
///             Vector3::new(1.0, 0.0, 0.0),
///             Vector3::new(-1.0, 0.0, 0.0),
///         ])
///         .face_centres(vec![
///             Vector3::new(0.5, 0.0, 0.0),
///             Vector3::new(1.0, 0.0, 0.0),
///             Vector3::new(0.0, 0.0, 0.0),
///         ])
///         .build().unwrap()
/// );
/// let p = VolScalarField::uniform("p", mesh.clone(), 101325.0);
/// let p_f = fvc::interpolate(&p);
/// assert!((p_f.internal[0] - 101325.0).abs() < 1e-8);
/// ```
pub fn interpolate<T>(vol: &VolField<T>) -> SurfaceField<T>
where
    T: Clone
        + Default
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<f64, Output = T>
        + Div<f64, Output = T>,
{
    let mesh = &vol.mesh;

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let w = linear_weight(mesh, f);
        // gh:#1489: upstream `surfaceInterpolationScheme::interpolate`:
        // `w*(P - N) + N`, not `w*P + (1 - w)*N` (differs by an ulp).
        let p = vol.internal[mesh.owner[f]].clone();
        let n = vol.internal[mesh.neighbour[f]].clone();
        (p - n.clone()) * w + n
    });

    let boundary = mesh
        .patches
        .iter()
        .zip(vol.boundary.iter())
        .map(|(patch, bc_patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                let owner = mesh.owner[gf];
                // Cyclic (periodic) seam: linearly interpolate across the seam to
                // the paired cell, weight from the two half-face gaps. Reduces to
                // 0.5/0.5 for an equally spaced seam.
                if patch.kind == PatchKind::Cyclic {
                    if let Some(pf) = mesh.cyclic_partner_face(gf) {
                        let paired = mesh.owner[pf];
                        let d_a = (mesh.face_centres[gf] - mesh.cell_centres[owner]).mag();
                        let d_b = (mesh.face_centres[pf] - mesh.cell_centres[paired]).mag();
                        let denom = d_a + d_b;
                        let w = if denom < 1e-300 { 0.5 } else { d_b / denom };
                        return vol.internal[owner].clone() * w
                            + vol.internal[paired].clone() * (1.0 - w);
                    }
                }
                match &bc_patch.bc {
                    // Zero-gradient family: face value = adjacent cell value.
                    // `Slip`/`Wedge` fall here for the generic (scalar) path;
                    // the vector-specific normal removal for `Slip`/`Symmetry`
                    // is applied by the momentum corrector, not this Layer-1
                    // interpolation. `InletOutlet`/`OutletInlet` have no flux in
                    // this operator, so they degrade to zero-gradient here.
                    // `Freestream`/`PressureInletOutletVelocity` carry no flux in
                    // this operator, so like `InletOutlet` they degrade to
                    // zero-gradient (face value = adjacent cell value) here.
                    BoundaryCondition::ZeroGradient
                    | BoundaryCondition::Symmetry
                    | BoundaryCondition::Slip
                    | BoundaryCondition::Wedge
                    | BoundaryCondition::InletOutlet { .. }
                    | BoundaryCondition::OutletInlet { .. }
                    | BoundaryCondition::Freestream { .. }
                    | BoundaryCondition::PressureInletOutletVelocity => vol.internal[owner].clone(),
                    BoundaryCondition::FixedValue(v) => v.clone(),
                    BoundaryCondition::FixedField(ff) => ff[fi].clone(),
                    BoundaryCondition::Calculated(ff) => ff[fi].clone(),
                    // Solver-driven Dirichlet BCs: the face value is the
                    // last-computed value the solver hook wrote into
                    // `bc_patch.values` (`p0 − 0.5ρ|U|²` for totalPressure, the
                    // flow-rate inlet velocity for flowRateInletVelocity).
                    BoundaryCondition::TotalPressure { .. }
                    | BoundaryCondition::FlowRateInletVelocity { .. } => {
                        bc_patch.values[fi].clone()
                    }
                    // No-slip wall: fixedValue of zero.
                    BoundaryCondition::NoSlip => T::default(),
                    // Neumann with prescribed gradient g: φ_face = φ_cell + g·delta.
                    // `FixedFluxPressure` is a fixedGradient whose gradient the
                    // solver set (`snGrad(p)`), so it reconstructs the same way.
                    BoundaryCondition::FixedGradient(g)
                    | BoundaryCondition::FixedFluxPressure { gradient: g } => {
                        // upstream `fixedGradientFvPatchField::evaluate`: `P + gradient/deltaCoeffs`
                        // (gh:#1362: divide by the stored coefficient, ~~`g*delta`~~)
                        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, gf);
                        vol.internal[owner].clone() + g.clone() / dc
                    }
                    // Robin/mixed blend of the fixedValue and fixedGradient parts.
                    BoundaryCondition::Mixed {
                        value_fraction,
                        ref_value,
                        ref_grad,
                    } => {
                        // upstream v2506 `mixedFvPatchField::evaluate`:
                        // `lerp(P + refGrad/deltaCoeffs, refValue, w)` = `(1-w)*(P + g/dC) + w*ref`
                        // (gh:#1362: `g/dC`, ~~`g*delta`~~). Foundation (dev) writes
                        // `w*ref + (1-w)*(...)`, the same sum.
                        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, gf);
                        let w = *value_fraction;
                        (vol.internal[owner].clone() + ref_grad.clone() / dc) * (1.0 - w)
                            + ref_value.clone() * w
                    }
                    // Per-face Robin blend; same algebra, coefficients per face.
                    BoundaryCondition::MixedField {
                        value_fraction,
                        ref_value,
                        ref_grad,
                    } => {
                        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, gf);
                        let w = value_fraction[fi];
                        (vol.internal[owner].clone() + ref_grad[fi].clone() / dc) * (1.0 - w)
                            + ref_value[fi].clone() * w
                    }
                    BoundaryCondition::Empty => T::default(),
                }
            });
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values,
            }
        })
        .collect();

    SurfaceField::new(
        crate::fv_operators::naming::derived_name("interpolate", &vol.name),
        vol.mesh.clone(),
        internal,
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn uniform_field_interpolates_exactly() {
        let m = unit_mesh();
        let p = VolScalarField::uniform("p", m, 5.0);
        let p_f = interpolate(&p);
        assert!((p_f.internal[0] - 5.0).abs() < 1e-12);
        assert!((p_f.boundary[0].values[0] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn linear_field_midpoint_weight() {
        // p[0]=0, p[1]=1 — face at midpoint should give 0.5
        let m = unit_mesh();
        let mut p = VolScalarField::zeros("p", m.clone());
        p.internal[1] = 1.0;
        let p_f = interpolate(&p);
        assert!((p_f.internal[0] - 0.5).abs() < 1e-12);
    }

    /// V&V (verification, 2026-08-04). Cyclic-patch interpolation crosses the
    /// periodic seam. Methodology: on `periodic_1d(4, 1.0, 1.0)` set the field to
    /// `[0,1,2,3]`. The left cyclic patch (owner cell 0) is paired with cell 3
    /// across the seam; equal half-gaps give weight 0.5, so the seam face value
    /// is `0.5·φ_0 + 0.5·φ_3 = 1.5`; the right patch (owner cell 3, paired cell
    /// 0) gives the same 1.5. Pass criterion: |value − 1.5| < 1e-12.
    /// Result: left = 1.500000, right = 1.500000. PASS.
    #[test]
    fn vv_cyclic_interpolates_across_seam() {
        let m = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(4, 1.0, 1.0));
        let mut p = VolScalarField::zeros("p", m.clone());
        p.internal[0] = 0.0;
        p.internal[1] = 1.0;
        p.internal[2] = 2.0;
        p.internal[3] = 3.0;
        let p_f = interpolate(&p);
        assert!(
            (p_f.boundary[0].values[0] - 1.5).abs() < 1e-12,
            "left={}",
            p_f.boundary[0].values[0]
        );
        assert!(
            (p_f.boundary[1].values[0] - 1.5).abs() < 1e-12,
            "right={}",
            p_f.boundary[1].values[0]
        );
    }

    /// V&V (verification, gh:#1202) — `linear_weight` is upstream's
    /// `makeWeights`. Hand values: cells at x = 0 and 1, face normal +x.
    /// Face centre (0.3, 0.4, 0): projections 0.3 and 0.7, so w = 0.7 (the
    /// old distance form gave |C_N − C_f| = √0.65 = 0.806). Face centre
    /// (0.25, 0, 0) on the line: 0.75 either way. Degenerate (zero normal):
    /// the distance fallback, 0.75. Results (2026-11-04): as stated, |Δ| <
    /// 1e-15.
    #[test]
    fn linear_weight_projects_on_the_face_normal_as_upstream() {
        let mesh = |cf: Vector3, s: Vector3| {
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![BoundaryPatch::new("right", 1, 1, PatchKind::Wall), BoundaryPatch::new("left", 2, 1, PatchKind::Wall)])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)])
                .face_area_vectors(vec![s, Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)])
                .face_centres(vec![cf, Vector3::new(1.5, 0.0, 0.0), Vector3::new(-0.5, 0.0, 0.0)])
                .build()
                .unwrap()
        };
        let x = Vector3::new(1.0, 0.0, 0.0);
        assert!((linear_weight(&mesh(Vector3::new(0.3, 0.4, 0.0), x), 0) - 0.7).abs() < 1e-15);
        assert!((linear_weight(&mesh(Vector3::new(0.25, 0.0, 0.0), x), 0) - 0.75).abs() < 1e-15);
        assert!((linear_weight(&mesh(Vector3::new(0.25, 0.0, 0.0), Vector3::ZERO), 0) - 0.75).abs() < 1e-15);
    }
}
