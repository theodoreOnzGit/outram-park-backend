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

use crate::openfoam_algorithms::openfoam_source::vol_field::VolScalarField;
use crate::openfoam_algorithms::openfoam_source::fv_matrix::FvMatrix;

/// Implicit Euler ddt: `∂φ/∂t ≈ (φ − φ_old) / Δt`.
///
/// Assembles `V/Δt` on the diagonal and `V·φ_old/Δt` into the source vector.
pub fn ddt(phi: &VolScalarField, phi_old: &VolScalarField, dt: f64) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let coeff = mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += coeff;
        mat.source[c] += coeff * phi_old.internal[c];
    }
    mat
}

/// Field-coefficient implicit Euler ddt: `∂(coeff·φ)/∂t ≈ coeff·(φ − φ_old) / Δt`.
///
/// Assembles `coeff[c]·V[c]/Δt` on the diagonal. Covers `fvm::ddt(rho, he)`
/// (fluid energy), `fvm::ddt(rho_cp, T)` (solid energy), etc.
pub fn ddt_coeff(
    coeff: &VolScalarField,
    phi: &VolScalarField,
    phi_old: &VolScalarField,
    dt: f64,
) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let c_dt = coeff.internal[c] * mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += c_dt;
        mat.source[c] += c_dt * phi_old.internal[c];
    }
    let _ = phi;
    mat
}

/// Conservative implicit Euler ddt with **distinct new- and old-time
/// coefficients**: `∂(ρφ)/∂t ≈ (ρ_new·φ − ρ_old·φ_old) / Δt`.
///
/// Unlike [`ddt_coeff`] (which reuses one `coeff` field for both time levels),
/// this takes the new-time density `coeff_new` (e.g. the continuity density
/// `ρ_cont = ρ_old − Δt·∇·φ`) for the implicit diagonal and the old-time density
/// `coeff_old` for the explicit source. Assembling `ρ_new·V/Δt` on the diagonal
/// and `ρ_old·φ_old·V/Δt` in the source makes the discrete energy balance carry
/// the missing `φ_old·(ρ − ρ_old)/Δt = −φ_old·∇·φ` term, which cancels the
/// `φ·∇·φ` part of `∇·(φ·Φ)` exactly and recovers the material derivative
/// `ρ Dφ/Dt`. This is the conservative energy time-derivative used by
/// [`crate::OPCPFluidArray::step`]'s energy equation (mirroring the
/// `tampines-steam-tables` `TampinesSteamArray` Edwards flashing-plateau fix,
/// bead op-21g.14 / op-ek2). See that method's energy-equation block for the
/// full rationale.
pub fn ddt_coeff_old(
    coeff_new: &VolScalarField,
    coeff_old: &VolScalarField,
    phi: &VolScalarField,
    phi_old: &VolScalarField,
    dt: f64,
) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let v_dt = mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += coeff_new.internal[c] * v_dt;
        mat.source[c] += coeff_old.internal[c] * phi_old.internal[c] * v_dt;
    }
    let _ = phi;
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::openfoam_algorithms::openfoam_source::{FvMesh, SolverSettings, Vector3};
    use crate::openfoam_algorithms::openfoam_source::vol_field::VolScalarField;
    use crate::openfoam_algorithms::openfoam_source::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};

    fn unit_mesh() -> Arc<FvMesh> {
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

    #[test]
    fn ddt_diagonal_is_volume_over_dt() {
        let m = unit_mesh();
        let phi = VolScalarField::uniform("p", m.clone(), 0.0);
        let phi_old = VolScalarField::uniform("p_old", m.clone(), 2.0);
        let mat = ddt(&phi, &phi_old, 0.1);
        // diag = V/dt = 0.5/0.1 = 5
        assert!((mat.ldu.diag[0] - 5.0).abs() < 1e-12);
        // source = V/dt * phi_old = 5 * 2 = 10
        assert!((mat.source[0] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn ddt_solves_to_old_when_no_spatial_terms() {
        // ddt only: A·φ = V/dt·φ_old  →  φ = φ_old
        let m = unit_mesh();
        let phi = VolScalarField::zeros("p", m.clone());
        let phi_old = VolScalarField::uniform("p_old", m.clone(), 3.0);
        let mat = ddt(&phi, &phi_old, 1.0);
        let settings = SolverSettings::default();
        let (result, perf) = mat.solve("p", settings);
        assert!(perf.converged, "Gauss-Seidel did not converge");
        assert!((result.internal[0] - 3.0).abs() < 1e-8);
        assert!((result.internal[1] - 3.0).abs() < 1e-8);
    }

    #[test]
    fn ddt_coeff_diagonal_is_coeff_times_volume_over_dt() {
        let m = unit_mesh();
        // rho_cp = 2 everywhere, V = 0.5, dt = 0.1  →  diag = 2*0.5/0.1 = 10
        let coeff   = VolScalarField::uniform("rho_cp", m.clone(), 2.0);
        let phi     = VolScalarField::zeros("T", m.clone());
        let phi_old = VolScalarField::uniform("T_old", m.clone(), 5.0);
        let mat = ddt_coeff(&coeff, &phi, &phi_old, 0.1);
        assert!((mat.ldu.diag[0] - 10.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 10.0).abs() < 1e-12);
        // source = 10 * 5 = 50
        assert!((mat.source[0] - 50.0).abs() < 1e-12);
    }

    #[test]
    fn ddt_coeff_solves_to_old_when_no_spatial_terms() {
        let m = unit_mesh();
        let coeff   = VolScalarField::uniform("rho_cp", m.clone(), 3.0);
        let phi     = VolScalarField::zeros("T", m.clone());
        let phi_old = VolScalarField::uniform("T_old", m.clone(), 7.0);
        let mat = ddt_coeff(&coeff, &phi, &phi_old, 1.0);
        let settings = SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged);
        assert!((result.internal[0] - 7.0).abs() < 1e-8);
        assert!((result.internal[1] - 7.0).abs() < 1e-8);
    }
}
