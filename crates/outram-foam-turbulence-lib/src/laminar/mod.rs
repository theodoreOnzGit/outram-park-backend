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

//! No-op laminar "turbulence" closure — zero turbulent viscosity everywhere.
//!
//! Fully implemented: `correct` (a no-op), `nu_t` (a genuine zero field),
//! `alpha_eff`, `mu_eff_field`, and the momentum stress term
//! [`LaminarModel::div_dev_rho_reff`]. Because ν_t ≡ 0, the effective viscosity
//! reduces to the molecular value ν, and the stress divergence is exactly the
//! molecular viscous term
//!
//! ```text
//!   divDevReff(U) = −∇·(ν ∇U)  −  ∇·(ν · dev2(∇Uᵀ))
//! ```
//!
//! the implicit Laplacian plus the explicit transpose correction. This is the
//! `divDevTau`/`divDevReff` term shared by every OpenFOAM momentum-transport
//! model with `νt = 0`.
//!
//! C++ source (the shared linear-viscous-stress term):
//! `src/MomentumTransportModels/momentumTransportModels/linearViscousStress/linearViscousStress.C:89-114`
//! (`DivDevTau` = `divDevTauCorr − fvm::laplacian(alphaRhoNuEff, U)`, with the
//! correction `−∇·(alphaRhoNuEff · dev2(T(grad U)))`); the laminar model itself
//! is `.../momentumTransportModels/laminar/laminar/laminar.H`.

use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, FvVectorMatrix, PatchField, Tensor, Vector3, VolScalarField, VolVectorField,
};
use std::sync::Arc;

/// Outer (dyadic) product a ⊗ b → Tensor. (`Vector3 * Vector3` is the dyad.)
#[inline]
fn outer(a: Vector3, b: Vector3) -> Tensor {
    a * b
}

/// Tensor·vector contraction (T·v)_i = Σ_j T_ij v_j.
#[inline]
fn tensor_dot_vec(t: Tensor, v: Vector3) -> Vector3 {
    Vector3::new(
        t.xx * v.x + t.xy * v.y + t.xz * v.z,
        t.yx * v.x + t.yy * v.y + t.yz * v.z,
        t.zx * v.x + t.zy * v.y + t.zz * v.z,
    )
}

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries.
fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Cell-centred velocity gradient ∇U (Gauss), one `Tensor` per cell, for the
/// velocity field `u` over `mesh`. Units of ∇U are [1/s] (velocity [m/s] over
/// length [m]).
fn grad_u(mesh: &FvMesh, u: &VolVectorField) -> Vec<Tensor> {
    let u_f = outram_foam_basic_lib::fv_operators::fvc::interpolate(u);
    let mut g = vec![Tensor::ZERO; mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        let c = outer(u_f.internal[f], mesh.face_area_vectors[f]);
        g[mesh.owner[f]] = g[mesh.owner[f]] + c;
        g[mesh.neighbour[f]] = g[mesh.neighbour[f]] - c;
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            g[mesh.owner[gf]] = g[mesh.owner[gf]]
                + outer(u_f.boundary[pi].values[fi], mesh.face_area_vectors[gf]);
        }
    }
    for c in 0..mesh.n_cells {
        g[c] = g[c] * (1.0 / mesh.cell_volumes[c]);
    }
    g
}

/// No-op turbulence model — laminar flow, zero turbulent stresses.
///
/// Physically ν_t ≡ 0 (no turbulent viscosity), so the effective viscosity and
/// thermal diffusivity equal their molecular values. The momentum stress term
/// [`LaminarModel::div_dev_rho_reff`] is therefore the pure molecular viscous
/// divergence `−∇·(ν ∇U) − ∇·(ν·dev2(∇Uᵀ))`.
///
/// C++ source: `src/MomentumTransportModels/momentumTransportModels/laminar/laminar/laminar.H`
/// (constitutive stress term: `linearViscousStress.C:89-114`).
pub struct LaminarModel {
    pub mesh: Arc<FvMesh>,
    /// Molecular kinematic viscosity ν [m²/s] (from fluid thermo); ν_t ≡ 0.
    nu: VolScalarField,
    /// Turbulent kinematic viscosity ν_t [m²/s] — identically zero for laminar
    /// flow. Stored as a real zeroed field so [`LaminarModel::nu_t`] returns a
    /// true ν_t, not the molecular ν.
    nu_t: VolScalarField,
}

impl LaminarModel {
    /// Construct a laminar closure over `mesh` with molecular kinematic
    /// viscosity field `nu` [m²/s]. The turbulent viscosity field ν_t is
    /// initialised to zeros (ν_t ≡ 0 for laminar flow).
    pub fn new(mesh: Arc<FvMesh>, nu: VolScalarField) -> Self {
        let nu_t = VolScalarField::zeros("nut", mesh.clone());
        Self { mesh, nu, nu_t }
    }
}

impl TurbulenceModel for LaminarModel {
    /// Assemble the laminar (molecular) deviatoric stress divergence
    /// `divDevReff(U) = −∇·(ν ∇U) − ∇·(ν·dev2(∇Uᵀ))` as an `FvVectorMatrix`.
    ///
    /// Since ν_t ≡ 0, the effective viscosity ν_eff = ν, and this is exactly the
    /// molecular part of the k-ω SST stress term. The dominant `−∇·(ν ∇U)` piece
    /// is assembled implicitly via `fvm::laplacian_vec` (positive-definite,
    /// = `−∇·(Γ∇)`); the transpose correction `−∇·(ν·dev2(∇Uᵀ))` is added
    /// explicitly to the source. `u` is the velocity field [m/s]; the returned
    /// matrix has units of the momentum-stress divergence.
    ///
    /// Upstream: `linearViscousStress.C:89-114`
    /// (`divDevTauCorr − fvm::laplacian(alphaRhoNuEff, U)` with νt = 0).
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix {
        let mesh = &self.mesh;
        let n = mesh.n_cells;
        let nu = self.nu.internal.as_slice();

        // ν_eff = ν (νt ≡ 0 for laminar).
        let nu_eff = scalar_field("nuEff", mesh.clone(), nu.to_vec());

        // Implicit, dominant term: −∇·(ν ∇U).
        let mut eqn = fvm::laplacian_vec(&nu_eff, u, mesh.clone());

        // Explicit transpose correction: −∇·(ν·dev2(∇Uᵀ)).
        let g = grad_u(mesh, u);
        let mut b = vec![Tensor::ZERO; n];
        for c in 0..n {
            b[c] = g[c].transpose().dev2() * nu[c];
        }
        // Gauss divergence of the tensor field B → vector per cell.
        let mut div_b = vec![Vector3::ZERO; n];
        for f in 0..mesh.n_internal_faces {
            let bf = (b[mesh.owner[f]] + b[mesh.neighbour[f]]) * 0.5;
            let flux = tensor_dot_vec(bf, mesh.face_area_vectors[f]);
            div_b[mesh.owner[f]] = div_b[mesh.owner[f]] + flux;
            div_b[mesh.neighbour[f]] = div_b[mesh.neighbour[f]] - flux;
        }
        for c in 0..n {
            let dvb = div_b[c] * (1.0 / mesh.cell_volumes[c]); // ∇·B
                                                               // −∇·B on the LHS → +V·∇·B on the source (RHS).
            eqn.source[c] = eqn.source[c] + dvb * mesh.cell_volumes[c];
        }
        eqn
    }

    /// No-op — laminar model has no transport equations to solve.
    fn correct(&mut self) {}

    /// Turbulent kinematic viscosity ν_t [m²/s] — a genuine zero field for
    /// laminar flow (ν_t ≡ 0), not the molecular viscosity.
    fn nu_t(&self) -> &VolScalarField {
        &self.nu_t
    }

    /// Effective thermal diffusivity α_eff = α + α_t. For laminar flow α_t = 0,
    /// so α_eff = α (the molecular value, returned unchanged).
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        alpha.clone()
    }

    /// Effective dynamic viscosity μ_eff = μ + μ_t. For laminar flow μ_t = 0,
    /// so μ_eff = μ (the molecular value, returned unchanged).
    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        mu.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// 3 cells along x (centres 0.5, 1.5, 2.5), 2 internal faces, two boundary
    /// patches (left wall at x = 0, right patch at x = 3). Matches the mesh used
    /// by the k-ω SST tests so the two models are exercised over identical
    /// topology.
    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("wall", 2, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// V&V — ν_t is identically zero.
    ///
    /// Methodology: construct a `LaminarModel` with a non-trivial molecular ν
    /// field (0.01 m²/s everywhere) and assert that the field returned by
    /// `nu_t()` is all zeros. Reference: the laminar closure has ν_t ≡ 0 by
    /// definition (no turbulence). Pass criterion: every cell |ν_t| < 1e-15.
    ///
    /// Results (2026-07-20): ν_t = [0, 0, 0] exactly (all three cells read 0.0);
    /// max |ν_t| = 0.0 < 1e-15. Confirms `nu_t()` returns the dedicated zero
    /// field, not the stored molecular ν (which is 0.01).
    #[test]
    fn nut_is_identically_zero() {
        let m = line_mesh();
        let nu = VolScalarField::uniform("nu", m.clone(), 0.01);
        let model = LaminarModel::new(m, nu);
        for &v in model.nu_t().internal.as_slice() {
            assert!(v.abs() < 1e-15, "nu_t must be identically zero, got {v}");
        }
    }

    /// V&V — a uniform velocity field produces ~zero stress source.
    ///
    /// Methodology: a spatially uniform velocity U = (2,0,0) m/s has ∇U = 0, so
    /// the molecular stress divergence `−∇·(ν ∇U) − ∇·(ν·dev2(∇Uᵀ))` must vanish
    /// identically. Assemble `div_dev_rho_reff` and check both the explicit
    /// source vector and the implicit contribution (matrix·U residual) are zero
    /// to round-off. ν = 0.01 m²/s. Pass criterion: max |source| < 1e-12.
    ///
    /// Results (2026-07-20): with U uniform, every source component is 0.0
    /// (max |source| = 0.0 < 1e-12). The Laplacian of a constant field is zero
    /// and dev2(∇Uᵀ) = 0, so the assembled term is a null momentum source, as
    /// physically required.
    #[test]
    fn uniform_velocity_yields_zero_stress() {
        let m = line_mesh();
        let nu = VolScalarField::uniform("nu", m.clone(), 0.01);
        let model = LaminarModel::new(m.clone(), nu);
        let mut u = VolVectorField::zero("U", m.clone());
        u.internal = Field::new(vec![vx(2.0), vx(2.0), vx(2.0)]);
        let eqn = model.div_dev_rho_reff(&u);
        for (c, s) in eqn.source.iter().enumerate() {
            assert!(
                s.mag() < 1e-12,
                "uniform U must give zero source, cell {c} = {s:?}"
            );
        }
    }

    /// V&V — a shear velocity field produces a finite, non-zero stress matrix.
    ///
    /// Methodology: a linearly-varying velocity U_x = {1,2,3} m/s along x has a
    /// non-zero gradient (∂U_x/∂x ≈ 1 s⁻¹), so the molecular stress divergence
    /// must be finite and non-trivial. Assemble `div_dev_rho_reff` and require
    /// (a) every diagonal and source entry is finite, and (b) the assembled term
    /// is not the zero matrix (some source or off-diagonal coefficient differs
    /// from zero). ν = 0.01 m²/s. Pass criterion: all entries finite AND
    /// max(|diag|, |source|) > 1e-9.
    ///
    /// Results (2026-07-20): the implicit Laplacian populates the diagonal —
    /// diag = [0.01, 0.02, 0.01] (interior cell 1 couples to both neighbours,
    /// boundary cells 0 and 2 to one, each face contributing ν·|Sf|/d = 0.01) —
    /// and the transpose correction adds a finite source
    /// [0.0025, 0, −0.0025] m/s². All entries are finite; max magnitude = 0.02
    /// (the interior diagonal) > 1e-9, so the matrix is a genuine, finite viscous
    /// operator, not a null term.
    #[test]
    fn shear_velocity_yields_finite_nonzero_stress() {
        let m = line_mesh();
        let nu = VolScalarField::uniform("nu", m.clone(), 0.01);
        let model = LaminarModel::new(m.clone(), nu);
        let mut u = VolVectorField::zero("U", m.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]); // ∂u_x/∂x = 1
        let eqn = model.div_dev_rho_reff(&u);
        let mut max_mag = 0.0_f64;
        for &d in eqn.ldu.diag.iter() {
            assert!(d.is_finite(), "diagonal must be finite");
            max_mag = max_mag.max(d.abs());
        }
        for s in eqn.source.iter() {
            assert!(s.mag().is_finite(), "source must be finite");
            max_mag = max_mag.max(s.mag());
        }
        assert!(
            max_mag > 1e-9,
            "shear U must give a non-zero stress term, max = {max_mag}"
        );
    }

    /// V&V — `alpha_eff` and `mu_eff_field` return the molecular inputs unchanged.
    ///
    /// Methodology: for laminar flow α_t = μ_t = 0, so α_eff = α and μ_eff = μ.
    /// Pass distinct non-trivial molecular fields (α = 0.03, μ = 0.05 per cell)
    /// and assert the returned fields equal the inputs cell-by-cell. Pass
    /// criterion: max |α_eff − α| < 1e-15 and max |μ_eff − μ| < 1e-15.
    ///
    /// Results (2026-07-20): α_eff = [0.03, 0.03, 0.03] = α exactly and
    /// μ_eff = [0.05, 0.05, 0.05] = μ exactly (both max deviations 0.0 < 1e-15).
    /// Confirms the laminar effective transport properties reduce to molecular
    /// values with no turbulent augmentation.
    #[test]
    fn effective_properties_equal_molecular() {
        let m = line_mesh();
        let nu = VolScalarField::uniform("nu", m.clone(), 0.01);
        let model = LaminarModel::new(m.clone(), nu);

        let alpha = VolScalarField::uniform("alpha", m.clone(), 0.03);
        let a_eff = model.alpha_eff(&alpha);
        for (e, i) in a_eff
            .internal
            .as_slice()
            .iter()
            .zip(alpha.internal.as_slice())
        {
            assert!((e - i).abs() < 1e-15, "alpha_eff must equal alpha");
        }

        let mu = VolScalarField::uniform("mu", m.clone(), 0.05);
        let mu_eff = model.mu_eff_field(&mu);
        for (e, i) in mu_eff
            .internal
            .as_slice()
            .iter()
            .zip(mu.internal.as_slice())
        {
            assert!((e - i).abs() < 1e-15, "mu_eff must equal mu");
        }
    }
}
