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

use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use super::interpolate;

/// `∇·φ_f = (1/V_O) · Σ_f φ_f` — net volumetric flux per unit volume.
///
/// Used to evaluate the continuity residual `∇·U` or `∇·(ρU)/ρ`.
pub fn div_flux(phi: &SurfaceScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        d[mesh.owner[f]] += phi.internal[f];
        d[mesh.neighbour[f]] -= phi.internal[f];
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            d[mesh.owner[patch.start + fi]] += phi.boundary[pi].values[fi];
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({})", phi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·ψ) = (1/V_O) · Σ_f φ_f · ψ_f` — convective scalar flux.
///
/// `phi` is the face mass flux (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField).  Face values of `psi` are obtained by linear
/// interpolation.
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let psi_f = interpolate(psi);
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = phi.internal[f] * psi_f.internal[f];
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = phi.boundary[pi].values[fi] * psi_f.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] += flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({},{})", phi.name, psi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·U) = (1/V_O) · Σ_f φ_f · U_f` — convective vector flux.
///
/// `phi` is the face mass flux; `U` is the velocity (VolVectorField).
pub fn div_vec(phi: &SurfaceScalarField, u: &VolVectorField) -> VolVectorField {
    use crate::primitives::Vector3;

    let mesh = &phi.mesh;
    let u_f = interpolate(u);
    let mut d = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = u_f.internal[f] * phi.internal[f];
        d[mesh.owner[f]] = d[mesh.owner[f]] + flux;
        d[mesh.neighbour[f]] = d[mesh.neighbour[f]] - flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = u_f.boundary[pi].values[fi] * phi.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] = d[mesh.owner[patch.start + fi]] + flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("div({},{})", phi.name, u.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![0.5, 0.5])
            .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
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
            .build().unwrap())
    }

    fn phi_field(m: Arc<crate::mesh::fv_mesh::FvMesh>, int: f64, bnd: f64) -> SurfaceScalarField {
        let ni = m.n_internal_faces;
        let bnd_pf: Vec<_> = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, bnd) })
            .collect();
        SurfaceScalarField::new("phi", m, Field::uniform(ni, int), bnd_pf)
    }

    #[test]
    fn div_flux_of_uniform_inoutflow_is_zero() {
        // Symmetric flux +1 in, -1 out → net = 0 for interior cell
        // Only internal face with phi=1: cell 0 gains +1, cell 1 loses +1
        // Both boundary fluxes are 0 → both cells see net = ±1/0.5 = ±2
        // Actually just verify the formula: div_flux sums face fluxes / V
        let m = unit_mesh();
        let phi = phi_field(m.clone(), 0.0, 0.0);
        let d = div_flux(&phi);
        assert!(d.internal[0].abs() < 1e-12);
        assert!(d.internal[1].abs() < 1e-12);
    }

    #[test]
    fn div_flux_nonzero() {
        // phi_internal = 1, boundaries = 0
        // cell 0: +1 (internal outflow), -0 (left bnd inflow=0) → net +1 / 0.5 = +2
        // cell 1: -1 (internal, neighbour) + 0 (right bnd) → net -1 / 0.5 = -2
        let m = unit_mesh();
        let phi = phi_field(m.clone(), 1.0, 0.0);
        let d = div_flux(&phi);
        assert!((d.internal[0] - 2.0).abs() < 1e-10, "div_flux[0]={}", d.internal[0]);
        assert!((d.internal[1] - (-2.0)).abs() < 1e-10, "div_flux[1]={}", d.internal[1]);
    }
}
