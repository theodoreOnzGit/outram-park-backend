// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2011-2016 OpenFOAM Foundation
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

use super::interpolate;
use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::{VolTensorField, VolVectorField};
use crate::fv_operators::les_filter::{sn_grad_uncorrected, surface_integrate};
use crate::mesh::fv_mesh::PatchKind;
use crate::primitives::{Tensor, Vector3};

/// Gauss `linear` gradient of a vector field WITH upstream's boundary correction: a port of
/// `Foam::fv::gaussGrad<vector>::calcGrad` (ESI v2506
/// `finiteVolume/gradSchemes/gaussGrad/gaussGrad.C`, `gradf` and `correctBoundaryConditions`),
/// gh:#1544.
///
/// Cells: `sum_f Sf (x) u_f / V` (owner `+=`, neighbour `-=`, boundary faces `+=`, then a
/// division by `V`), `u_f` the linear face value. Boundary faces: first the owner-cell value
/// (`extrapolatedCalculated`), then for every non-coupled patch
/// `g_b += n (x) (snGrad(u) - n . g_b)`, `n = Sf/|Sf|`, which replaces the wall-normal component
/// by the patch's own normal gradient. [`grad_vec`](super::grad_vec) leaves the boundary at the
/// first step; this function is the whole of upstream's.
///
/// Compared bit for bit with the real `fvc::grad(U)` cells and boundary faces on the 360-cell duct
/// (`outram-foam-turbulence-lib`, `upstream_turb_gh1362/README.md`, slice 7).
pub fn grad_vec_corrected(vol: &VolVectorField) -> VolTensorField {
    let mesh = &vol.mesh;
    let u_f = interpolate(vol);
    let flux_i: Vec<Tensor> = (0..mesh.n_internal_faces)
        .map(|f| mesh.face_area_vectors[f] * u_f.internal[f])
        .collect();
    let flux_b: Vec<Vec<Tensor>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| mesh.face_area_vectors[p.start + fi] * u_f.boundary[pi].values[fi])
                .collect()
        })
        .collect();
    let cells = surface_integrate(mesh, &flux_i, &flux_b);
    let (_, sn_b) = sn_grad_uncorrected(vol);
    let boundary = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            let vals: Vec<Tensor> = (0..p.size)
                .map(|fi| {
                    let gf = p.start + fi;
                    let g = cells[mesh.owner[gf]];
                    let coupled = matches!(p.kind, PatchKind::Cyclic | PatchKind::CyclicAmi | PatchKind::Processor);
                    if coupled || p.kind == PatchKind::Empty {
                        return g;
                    }
                    let sf = mesh.face_area_vectors[gf];
                    let mag = sf.mag();
                    let n = Vector3::new(sf.x / mag, sf.y / mag, sf.z / mag);
                    let ng = Vector3::new(
                        n.x * g.xx + n.y * g.yx + n.z * g.zx,
                        n.x * g.xy + n.y * g.yy + n.z * g.zy,
                        n.x * g.xz + n.y * g.yz + n.z * g.zz,
                    );
                    g + n * (sn_b[pi][fi] - ng)
                })
                .collect();
            let vals = Field::new(vals);
            PatchField {
                bc: BoundaryCondition::Calculated(vals.clone()),
                values: vals,
            }
        })
        .collect();
    VolTensorField::new(
        crate::fv_operators::naming::derived_name("grad", &vol.name),
        vol.mesh.clone(),
        Field::new(cells),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder};
    use std::sync::Arc;

    fn mesh() -> Arc<FvMesh> {
        let vx = |x: f64| Vector3::new(x, 0.0, 0.0);
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 0, 1])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("l", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("r", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![vx(0.25), vx(0.75)])
                .face_area_vectors(vec![vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(0.5), vx(0.0), vx(1.0)])
                .build()
                .unwrap(),
        )
    }

    /// A linear field with Dirichlet walls has the exact gradient in the cells AND on the
    /// boundary faces: the normal correction replaces the (one-sided) cell value by the patch
    /// snGrad, which for a linear field is the exact slope.
    #[test]
    fn linear_field_has_exact_gradient_on_the_boundary_faces() {
        let m = mesh();
        let slope = Vector3::new(2.0, -3.0, 5.0);
        let at = |x: f64| slope * x;
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(at(0.0)),
                values: Field::new(vec![at(0.0)]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(at(1.0)),
                values: Field::new(vec![at(1.0)]),
            },
        ];
        let u = VolVectorField::new("U", m.clone(), Field::new(vec![at(0.25), at(0.75)]), boundary);
        let g = grad_vec_corrected(&u);
        for c in 0..2 {
            assert!((g.internal[c].xy + 3.0).abs() < 1e-12, "cell {c}");
        }
        for p in &g.boundary {
            assert!((p.values[0].xx - 2.0).abs() < 1e-12);
            assert!((p.values[0].xz - 5.0).abs() < 1e-12);
        }
    }
}
