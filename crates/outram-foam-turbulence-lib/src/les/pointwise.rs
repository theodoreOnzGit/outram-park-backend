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

//! Field algebra the dynamic LES models are written in (gh:#1544): pointwise maps over a
//! [`VolField`] that carry the boundary values the way upstream's `GeometricField` operators do
//! (every operator is applied to the boundary faces too, result patches `calculated`), and the
//! tensor operations of ESI v2506 `SymmTensorI.H` / `TensorI.H` in upstream's operation order
//! (so a frozen-field comparison can be bit for bit).

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::VolField;
use outram_foam_basic_lib::fv_operators::fvc::interpolate;
use outram_foam_basic_lib::fv_operators::les_filter::FilterValue;
use outram_foam_basic_lib::prelude::{FvMesh, SymmTensor, Tensor, Vector3};
use std::sync::Arc;

/// A field with `calculated` patches: cell values plus explicit boundary-face values.
pub fn calculated<T: Clone + Default>(
    name: &str,
    mesh: &Arc<FvMesh>,
    cells: Vec<T>,
    faces: Vec<Vec<T>>,
) -> VolField<T> {
    let boundary = faces
        .into_iter()
        .map(|v| {
            let vals = Field::new(v);
            PatchField {
                bc: BoundaryCondition::Calculated(vals.clone()),
                values: vals,
            }
        })
        .collect();
    VolField::new(name, mesh.clone(), Field::new(cells), boundary)
}

/// The face values of `vf` on every patch, whatever its boundary conditions (via `interpolate`).
pub fn boundary_values<T: FilterValue>(vf: &VolField<T>) -> Vec<Vec<T>> {
    interpolate(vf)
        .boundary
        .iter()
        .map(|p| p.values.iter().cloned().collect())
        .collect()
}

/// `vf` re-expressed with `calculated` patches holding its current face values.
pub fn as_calculated<T: FilterValue>(vf: &VolField<T>) -> VolField<T> {
    calculated(
        &vf.name,
        &vf.mesh,
        vf.internal.iter().cloned().collect(),
        boundary_values(vf),
    )
}

/// `vf` after upstream's `correctBoundaryConditions()` on `extrapolatedCalculated` patches: the
/// boundary faces take the owner-cell value. Upstream's filters call this on their argument, and
/// a field built from a filter output by a same-type `tmp` operator INHERITS that patch type
/// (`reuseTmp*`), so its stale boundary values are discarded at the next filter entry
/// (gh:#1544, README slice 7).
pub fn extrapolate_boundary<T: FilterValue>(vf: &VolField<T>) -> VolField<T> {
    let mesh = &vf.mesh;
    let cells: Vec<T> = vf.internal.iter().cloned().collect();
    let faces: Vec<Vec<T>> = mesh
        .patches
        .iter()
        .map(|p| {
            (0..p.size)
                .map(|fi| cells[mesh.owner[p.start + fi]].clone())
                .collect()
        })
        .collect();
    calculated(&vf.name, mesh, cells, faces)
}

/// `f(a)` on the cells and on every boundary face.
pub fn map1<A, B>(a: &VolField<A>, f: impl Fn(&A) -> B) -> VolField<B>
where
    A: Clone,
    B: Clone + Default,
{
    let cells: Vec<B> = a.internal.iter().map(&f).collect();
    let faces: Vec<Vec<B>> = a
        .boundary
        .iter()
        .map(|p| p.values.iter().map(&f).collect())
        .collect();
    calculated(&a.name, &a.mesh, cells, faces)
}

/// `f(a, b)` on the cells and on every boundary face.
pub fn map2<A, B, C>(a: &VolField<A>, b: &VolField<B>, f: impl Fn(&A, &B) -> C) -> VolField<C>
where
    A: Clone,
    B: Clone,
    C: Clone + Default,
{
    let cells: Vec<C> = (0..a.internal.len())
        .map(|c| f(&a.internal[c], &b.internal[c]))
        .collect();
    let faces: Vec<Vec<C>> = a
        .boundary
        .iter()
        .zip(b.boundary.iter())
        .map(|(pa, pb)| {
            (0..pa.values.len())
                .map(|i| f(&pa.values[i], &pb.values[i]))
                .collect()
        })
        .collect();
    calculated(&a.name, &a.mesh, cells, faces)
}

/// Upstream scalar `max(a, b)` = `(a > b) ? a : b`.
pub fn umax(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// `sqr(v)`: the symmetric outer product (`SymmTensorI.H`).
pub fn sqr_vec(v: &Vector3) -> SymmTensor {
    SymmTensor::new(v.x * v.x, v.x * v.y, v.x * v.z, v.y * v.y, v.y * v.z, v.z * v.z)
}

/// `magSqr(vector)`.
pub fn mag_sqr_vec(v: &Vector3) -> f64 {
    v.x * v.x + v.y * v.y + v.z * v.z
}

/// `symm(tensor)` (`TensorI.H`).
pub fn symm_tensor(t: &Tensor) -> SymmTensor {
    SymmTensor::new(
        t.xx,
        0.5 * (t.xy + t.yx),
        0.5 * (t.xz + t.zx),
        t.yy,
        0.5 * (t.yz + t.zy),
        t.zz,
    )
}

/// `devSymm(tensor)`: `sph = tr/3.0`, then the diagonal minus `sph` (`TensorI.H`).
pub fn dev_symm_tensor(t: &Tensor) -> SymmTensor {
    let sph = (t.xx + t.yy + t.zz) / 3.0;
    SymmTensor::new(
        t.xx - sph,
        0.5 * (t.xy + t.yx),
        0.5 * (t.xz + t.zx),
        t.yy - sph,
        0.5 * (t.yz + t.zy),
        t.zz - sph,
    )
}

/// `dev(symmTensor) = st - sph(st)`, `sph = (1.0/3.0)*tr` (`SymmTensorI.H`).
pub fn dev_symm_st(st: &SymmTensor) -> SymmTensor {
    let sph = (1.0 / 3.0) * (st.xx + st.yy + st.zz);
    SymmTensor::new(st.xx - sph, st.xy, st.xz, st.yy - sph, st.yz, st.zz - sph)
}

/// `a && b` for two symmTensors, in upstream's order (`SymmTensorI.H`).
pub fn ddot_st(a: &SymmTensor, b: &SymmTensor) -> f64 {
    a.xx * b.xx + 2.0 * a.xy * b.xy + 2.0 * a.xz * b.xz + a.yy * b.yy + 2.0 * a.yz * b.yz
        + a.zz * b.zz
}

/// `magSqr(symmTensor)` in upstream's order (`SymmTensorI.H`).
pub fn mag_sqr_st(st: &SymmTensor) -> f64 {
    st.xx * st.xx
        + 2.0 * (st.xy * st.xy)
        + 2.0 * (st.xz * st.xz)
        + st.yy * st.yy
        + 2.0 * (st.yz * st.yz)
        + st.zz * st.zz
}

/// `t && st` for a tensor and a symmTensor (`TensorI.H`).
pub fn ddot_t_st(t: &Tensor, st: &SymmTensor) -> f64 {
    t.xx * st.xx
        + t.xy * st.xy
        + t.xz * st.xz
        + t.yx * st.xy
        + t.yy * st.yy
        + t.yz * st.yz
        + t.zx * st.xz
        + t.zy * st.yz
        + t.zz * st.zz
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn mesh() -> Arc<FvMesh> {
        let vx = |x: f64| Vector3::new(x, 0.0, 0.0);
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 0, 1])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("l", 1, 1, PatchKind::Patch),
                    BoundaryPatch::new("r", 2, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5)])
                .face_area_vectors(vec![vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(0.0), vx(2.0)])
                .build()
                .unwrap(),
        )
    }

    /// Maps carry the boundary faces; `dev` removes the trace; the double contraction of a
    /// deviatoric tensor with itself equals its `magSqr`.
    #[test]
    fn maps_and_tensor_algebra_are_consistent() {
        let m = mesh();
        let a = calculated("a", &m, vec![1.0, 2.0], vec![vec![3.0], vec![4.0]]);
        let b = map2(&a, &a, |x, y| x * y);
        assert_eq!(b.internal[1], 4.0);
        assert_eq!(b.boundary[1].values[0], 16.0);
        let c = map1(&a, |x| x + 1.0);
        assert_eq!(c.boundary[0].values[0], 4.0);
        let cal = as_calculated(&c);
        assert_eq!(boundary_values(&cal)[1][0], 5.0);
        assert_eq!(umax(1.0, 2.0), 2.0);
        let ex = extrapolate_boundary(&a);
        assert_eq!(ex.boundary[0].values[0], 1.0);
        assert_eq!(ex.boundary[1].values[0], 2.0);
        let st = dev_symm_st(&sqr_vec(&Vector3::new(1.0, 2.0, 3.0)));
        assert!((st.xx + st.yy + st.zz).abs() < 1e-14);
        assert!((ddot_st(&st, &st) - mag_sqr_st(&st)).abs() < 1e-12);
        assert_eq!(mag_sqr_vec(&Vector3::new(1.0, 2.0, 2.0)), 9.0);
        let t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let s = symm_tensor(&t);
        assert_eq!(s.xy, 3.0);
        let d = dev_symm_tensor(&t);
        assert!((d.xx + d.yy + d.zz).abs() < 1e-14);
        assert!((ddot_t_st(&t, &d) - ddot_st(&symm_tensor(&t), &d)).abs() < 1e-12);
    }
}
