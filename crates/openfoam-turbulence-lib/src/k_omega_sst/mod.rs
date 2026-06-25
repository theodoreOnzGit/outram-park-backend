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

//! Menter (1994) k-ω SST RAS turbulence model.
//!
//! Mirrors `src/TurbulenceModels/.../RAS/kOmegaSST/`. Blends k-ω (inner
//! boundary layer, F1 = 1) with transformed k-ε (free stream, F1 = 0) and
//! applies the Bradshaw stress limiter via F2.
//!
//! ## State the model needs beyond the trait
//!
//! `TurbulenceModel::correct(&mut self)` takes no arguments, but a real model
//! needs the live velocity `U`, face flux `phi`, molecular viscosity `ν`, the
//! wall-distance field, and the time step. As in OpenFOAM (where the model holds
//! references to those fields), this struct stores them: set `u`, `phi`, `nu`,
//! `dt` each time step before calling `correct()`. The wall-distance field `y`
//! is computed once from the `Wall` patches at construction.
//!
//! ## Scope / validation
//!
//! The constitutive relations (νt stress limiter, F1/F2 blending, production)
//! and the k/ω transport assembly are implemented and unit-tested. End-to-end
//! CFD validation (the NACA0012 aerofoil) additionally needs a working
//! `RhoPimpleFoam` and turbulence wall-function boundary conditions, both
//! tracked in `openfoam-appbuilder-lib/TODO.md`.

use std::sync::Arc;
use openfoam_basic_lib::prelude::{
    FvMesh, FvVectorMatrix, VolScalarField, VolVectorField, SurfaceScalarField,
    Field, PatchField, Vector3, Tensor, PatchKind, SolverSettings,
};
use openfoam_basic_lib::fv_operators::{fvc, fvm};
use crate::traits::TurbulenceModel;

/// Outer (dyadic) product a ⊗ b → Tensor. (`Vector3 * Vector3` is the dyad.)
#[inline]
fn outer(a: Vector3, b: Vector3) -> Tensor { a * b }

/// Tensor·vector contraction (T·v)_i = Σ_j T_ij v_j.
#[inline]
fn tensor_dot_vec(t: Tensor, v: Vector3) -> Vector3 {
    Vector3::new(
        t.xx * v.x + t.xy * v.y + t.xz * v.z,
        t.yx * v.x + t.yy * v.y + t.yz * v.z,
        t.zx * v.x + t.zy * v.y + t.zz * v.z,
    )
}

/// Menter k-ω SST turbulence model (1994).
pub struct KOmegaSST {
    pub mesh: Arc<FvMesh>,
    pub k:     VolScalarField,
    pub omega: VolScalarField,
    pub nu_t:  VolScalarField,
    /// Blending function F1 — 1 in inner layer, 0 in free stream.
    f1: VolScalarField,
    /// Blending function F2 — activates the stress limiter near walls.
    f2: VolScalarField,
    /// Velocity field — set by the solver each step before `correct()`.
    pub u:   VolVectorField,
    /// Face volumetric flux φ = U·Sf — set by the solver each step.
    pub phi: SurfaceScalarField,
    /// Molecular kinematic viscosity ν.
    pub nu:  VolScalarField,
    /// Distance to the nearest wall, per cell (computed once at construction).
    y: Vec<f64>,
    /// Time step [s].
    pub dt: f64,
    /// Turbulent Prandtl number for `alpha_eff`.
    pub prt: f64,
}

// ── Menter (1994) SST coefficients ───────────────────────────────────────────
pub const SIGMA_K1:  f64 = 0.85;
pub const SIGMA_K2:  f64 = 1.00;
pub const SIGMA_W1:  f64 = 0.50;
pub const SIGMA_W2:  f64 = 0.856;
pub const BETA1:     f64 = 0.075;
pub const BETA2:     f64 = 0.0828;
pub const BETA_STAR: f64 = 0.09;
pub const KAPPA:     f64 = 0.41;   // von Kármán constant
pub const A1:        f64 = 0.31;   // stress-limiter coefficient

/// Lower clamps to keep divisions finite.
const SMALL: f64 = 1e-14;

impl KOmegaSST {
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let k     = VolScalarField::uniform("k",     mesh.clone(), 1e-4);
        let omega = VolScalarField::uniform("omega", mesh.clone(), 1.0);
        let nu_t  = VolScalarField::zeros("nut",  mesh.clone());
        let f1    = VolScalarField::uniform("F1", mesh.clone(), 1.0);
        let f2    = VolScalarField::zeros("F2", mesh.clone());
        let u     = VolVectorField::zero("U",  mesh.clone());
        let phi   = SurfaceScalarField::zeros("phi", mesh.clone());
        let nu    = VolScalarField::uniform("nu", mesh.clone(), 1e-5);
        let y     = compute_wall_distance(&mesh);
        Self { mesh, k, omega, nu_t, f1, f2, u, phi, nu, y, dt: 1e-3, prt: 0.85 }
    }

    /// γ (a.k.a. α) blending coefficient for the ω production term:
    /// `γ = β/β* − σ_ω·κ²/√β*`, evaluated for each k-ω / k-ε set.
    fn gamma1() -> f64 { BETA1 / BETA_STAR - SIGMA_W1 * KAPPA * KAPPA / BETA_STAR.sqrt() }
    fn gamma2() -> f64 { BETA2 / BETA_STAR - SIGMA_W2 * KAPPA * KAPPA / BETA_STAR.sqrt() }

    /// Cell-centred velocity gradient ∇U (Gauss), one `Tensor` per cell.
    fn grad_u(&self) -> Vec<Tensor> {
        let mesh = &self.mesh;
        let u_f  = fvc::interpolate(&self.u);
        let mut g = vec![Tensor::ZERO; mesh.n_cells];
        for f in 0..mesh.n_internal_faces {
            let c = outer(u_f.internal[f], mesh.face_area_vectors[f]);
            g[mesh.owner[f]]     = g[mesh.owner[f]] + c;
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

    /// Recompute νt from the current k, ω and strain magnitude with the
    /// Bradshaw stress limiter: `νt = a1·k / max(a1·ω, |S|·F2)`.
    fn update_nut(&mut self, mag_s: &[f64]) {
        let n = self.mesh.n_cells;
        let k  = self.k.internal.as_slice();
        let w  = self.omega.internal.as_slice();
        let f2 = self.f2.internal.as_slice();
        let mut nut = vec![0.0_f64; n];
        for c in 0..n {
            let denom = (A1 * w[c]).max(mag_s[c] * f2[c]).max(SMALL);
            nut[c] = (A1 * k[c].max(0.0) / denom).max(0.0);
        }
        self.nu_t.internal = Field::new(nut);
    }
}

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries.
fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh.patches.iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Per-cell distance to the nearest `Wall`-patch face centre (brute force).
/// Returns a large value everywhere if the mesh has no wall patches.
fn compute_wall_distance(mesh: &FvMesh) -> Vec<f64> {
    const BIG: f64 = 1.0e10;
    let mut wall_faces: Vec<Vector3> = Vec::new();
    for patch in mesh.patches.iter() {
        if patch.kind == PatchKind::Wall {
            for fi in 0..patch.size {
                wall_faces.push(mesh.face_centres[patch.start + fi]);
            }
        }
    }
    (0..mesh.n_cells).map(|c| {
        if wall_faces.is_empty() { return BIG; }
        let cc = mesh.cell_centres[c];
        wall_faces.iter().map(|wf| (*wf - cc).mag()).fold(BIG, f64::min)
    }).collect()
}

impl TurbulenceModel for KOmegaSST {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix {
        // Effective viscosity ν_eff = ν + νt.
        let n = self.mesh.n_cells;
        let nu  = self.nu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let nu_eff_cell: Vec<f64> = (0..n).map(|c| nu[c] + nut[c]).collect();
        let nu_eff = scalar_field("nuEff", self.mesh.clone(), nu_eff_cell);

        // Implicit, dominant term: −∇·(ν_eff ∇U). `fvm::laplacian_vec` is
        // positive-definite (= −∇·(Γ∇)), so it is added directly (see the
        // pimpleFoam momentum note). This alone is OpenFOAM's
        // `-fvm::laplacian(nuEff, U)` part of `divDevReff`.
        let mut eqn = fvm::laplacian_vec(&nu_eff, u, self.mesh.clone());

        // Explicit transpose correction: −∇·(ν_eff·dev2(∇Uᵀ)). Small for nearly
        // divergence-free flows; included for fidelity to OpenFOAM's
        // `- fvc::div(nuEff*dev2(T(grad U)))`.
        let grad_u = self.grad_u();
        let mut b = vec![Tensor::ZERO; n];
        for c in 0..n {
            b[c] = grad_u[c].transpose().dev2() * (nu[c] + nut[c]);
        }
        // Gauss divergence of the tensor field B → vector per cell.
        let mesh = &self.mesh;
        let mut div_b = vec![Vector3::ZERO; n];
        for f in 0..mesh.n_internal_faces {
            // linear face tensor (owner/neighbour average) · Sf
            let bf = (b[mesh.owner[f]] + b[mesh.neighbour[f]]) * 0.5;
            let flux = tensor_dot_vec(bf, mesh.face_area_vectors[f]);
            div_b[mesh.owner[f]]     = div_b[mesh.owner[f]] + flux;
            div_b[mesh.neighbour[f]] = div_b[mesh.neighbour[f]] - flux;
        }
        for c in 0..n {
            let dvb = div_b[c] * (1.0 / mesh.cell_volumes[c]); // ∇·B
            // −∇·B on the LHS → move to the matrix source (RHS) with +V·∇·B.
            eqn.source[c] = eqn.source[c] + dvb * mesh.cell_volumes[c];
        }
        eqn
    }

    fn correct(&mut self) {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.dt;
        let settings = SolverSettings::default();

        // ── Strain rate |S| = √(2 S:S), S = symm(∇U) ─────────────────────────
        let grad_u = self.grad_u();
        let mut mag_s = vec![0.0_f64; n]; // |S|
        let mut s2    = vec![0.0_f64; n]; // |S|² = 2·S:S  (= production/νt)
        for c in 0..n {
            let s  = grad_u[c].symm();
            let ss = s.double_inner(s);
            s2[c]    = 2.0 * ss;
            mag_s[c] = s2[c].sqrt();
        }

        // ── Blending functions F1, F2 and cross-diffusion CDkω ───────────────
        let grad_k = fvc::grad(&self.k);
        let grad_w = fvc::grad(&self.omega);
        // Copy into owned vecs so they don't hold a borrow of `self` across the
        // later `self.omega`/`self.k` reassignments.
        let nu: Vec<f64> = self.nu.internal.as_slice().to_vec();
        let kk: Vec<f64> = self.k.internal.as_slice().to_vec();
        let ww: Vec<f64> = self.omega.internal.as_slice().to_vec();
        let gk  = grad_k.internal.as_slice();
        let gw  = grad_w.internal.as_slice();

        let mut f1 = vec![0.0_f64; n];
        let mut f2 = vec![0.0_f64; n];
        let mut cdkw = vec![0.0_f64; n];
        for c in 0..n {
            let y  = self.y[c].max(SMALL);
            let k  = kk[c].max(SMALL);
            let w  = ww[c].max(SMALL);
            let nc = nu[c].max(SMALL);
            let cd = (2.0 * SIGMA_W2 / w * gk[c].dot(gw[c])).max(1.0e-10);
            cdkw[c] = cd;
            let arg1 = (k.sqrt() / (BETA_STAR * w * y))
                .max(500.0 * nc / (y * y * w))
                .min(4.0 * SIGMA_W2 * k / (cd * y * y));
            f1[c] = arg1.powi(4).tanh();
            let arg2 = (2.0 * k.sqrt() / (BETA_STAR * w * y))
                .max(500.0 * nc / (y * y * w));
            f2[c] = (arg2 * arg2).tanh();
        }
        self.f1.internal = Field::new(f1.clone());
        self.f2.internal = Field::new(f2.clone());

        // νt from the current state (uses the freshly-updated F2).
        self.update_nut(&mag_s);
        let nut: Vec<f64> = self.nu_t.internal.as_slice().to_vec();

        // ── ω transport ──────────────────────────────────────────────────────
        //   ∂ω/∂t + ∇·(φω) − ∇·(D_ω ∇ω) = γ|S|² − βω² + (1−F1)·CDkω
        let g1 = Self::gamma1();
        let g2 = Self::gamma2();
        let dw_cell: Vec<f64> = (0..n)
            .map(|c| nu[c] + (f1[c] * SIGMA_W1 + (1.0 - f1[c]) * SIGMA_W2) * nut[c])
            .collect();
        let dw_f = fvc::interpolate(&scalar_field("Dw", mesh.clone(), dw_cell));

        let omega_old = self.omega.clone();
        let mut w_eqn = fvm::ddt(&self.omega, &omega_old, dt)
            + fvm::div(&self.phi, &self.omega)
            + fvm::laplacian(&dw_f, &self.omega);
        for c in 0..n {
            let v = mesh.cell_volumes[c];
            let gamma = f1[c] * g1 + (1.0 - f1[c]) * g2;
            let beta  = f1[c] * BETA1 + (1.0 - f1[c]) * BETA2;
            // explicit production + cross-diffusion sources (× V):
            w_eqn.source[c] = w_eqn.source[c]
                + v * (gamma * s2[c] + (1.0 - f1[c]) * cdkw[c]);
            // implicit destruction βω² → Sp(βω, ω): add βω·V to the diagonal.
            w_eqn.ldu.diag[c] += v * beta * ww[c].max(SMALL);
        }
        let (omega_new, _) = w_eqn.solve("omega", settings);
        self.omega = omega_new;
        for v in self.omega.internal.as_mut_slice() {
            if *v < SMALL { *v = SMALL; }
        }

        // ── k transport ──────────────────────────────────────────────────────
        //   ∂k/∂t + ∇·(φk) − ∇·(D_k ∇k) = G − β*kω,   G = νt|S|² (limited)
        let dk_cell: Vec<f64> = (0..n)
            .map(|c| nu[c] + (f1[c] * SIGMA_K1 + (1.0 - f1[c]) * SIGMA_K2) * nut[c])
            .collect();
        let dk_f = fvc::interpolate(&scalar_field("Dk", mesh.clone(), dk_cell));

        let k_old = self.k.clone();
        let mut k_eqn = fvm::ddt(&self.k, &k_old, dt)
            + fvm::div(&self.phi, &self.k)
            + fvm::laplacian(&dk_f, &self.k);
        let w_now = self.omega.internal.as_slice();
        for c in 0..n {
            let v = mesh.cell_volumes[c];
            // production, limited to 10·β*·k·ω as in OpenFOAM
            let g = (nut[c] * s2[c]).min(10.0 * BETA_STAR * kk[c].max(0.0) * w_now[c]);
            k_eqn.source[c] = k_eqn.source[c] + v * g;
            // implicit destruction β*kω → Sp(β*ω, k)
            k_eqn.ldu.diag[c] += v * BETA_STAR * w_now[c];
        }
        let (k_new, _) = k_eqn.solve("k", settings);
        self.k = k_new;
        for v in self.k.internal.as_mut_slice() {
            if *v < SMALL { *v = SMALL; }
        }

        // Final νt with the updated k, ω.
        self.update_nut(&mag_s);
    }

    fn nu_t(&self) -> &VolScalarField { &self.nu_t }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        // α_eff = α + α_t,  α_t = νt / Prt
        let n = self.mesh.n_cells;
        let a   = alpha.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let vals: Vec<f64> = (0..n).map(|c| a[c] + nut[c] / self.prt).collect();
        scalar_field("alphaEff", self.mesh.clone(), vals)
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        // μ_eff = μ + μ_t  (incompressible: ν + νt)
        let n = self.mesh.n_cells;
        let m   = mu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let vals: Vec<f64> = (0..n).map(|c| m[c] + nut[c]).collect();
        scalar_field("muEff", self.mesh.clone(), vals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openfoam_basic_lib::prelude::{FvMeshBuilder, BoundaryPatch};

    fn vx(x: f64) -> Vector3 { Vector3::new(x, 0.0, 0.0) }

    // 3 cells along x (centres 0.5, 1.5, 2.5); the left patch (x = 0) is a Wall.
    fn line_mesh_with_wall() -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(3).n_internal_faces(2)
            .owner(vec![0, 1, 0, 2]).neighbour(vec![1, 2])
            .patches(vec![
                BoundaryPatch::new("wall", 2, 1, PatchKind::Wall),
                BoundaryPatch::new("top",  3, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![1.0, 1.0, 1.0])
            .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
            .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
            .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
            .build().unwrap())
    }

    #[test]
    fn wall_distance_is_nearest_wall_face() {
        let m = line_mesh_with_wall();
        let y = compute_wall_distance(&m);
        // wall face is at x = 0, cell centres at 0.5, 1.5, 2.5
        assert!((y[0] - 0.5).abs() < 1e-12);
        assert!((y[1] - 1.5).abs() < 1e-12);
        assert!((y[2] - 2.5).abs() < 1e-12);
    }

    #[test]
    fn nut_stress_limiter_formula() {
        // νt = a1·k / max(a1·ω, |S|·F2)
        let m = line_mesh_with_wall();
        let mut model = KOmegaSST::new(m);
        model.k.internal     = Field::new(vec![0.2, 0.2, 0.2]);
        model.omega.internal = Field::new(vec![10.0, 10.0, 10.0]);
        model.f2.internal    = Field::new(vec![1.0, 1.0, 1.0]);
        // Cell 0: |S| large → limiter active (|S|·F2 > a1·ω)
        // Cell 2: |S| small → a1·ω branch
        let mag_s = vec![100.0, 0.0, 0.0];
        model.update_nut(&mag_s);
        let nut = model.nu_t.internal.as_slice();
        // cell 0: denom = max(0.31·10, 100·1) = 100 → νt = 0.31·0.2/100
        assert!((nut[0] - (A1 * 0.2 / 100.0)).abs() < 1e-12);
        // cell 2: denom = max(0.31·10, 0) = 3.1 → νt = 0.31·0.2/3.1
        assert!((nut[2] - (A1 * 0.2 / (A1 * 10.0))).abs() < 1e-12);
    }

    #[test]
    fn correct_keeps_k_omega_positive_and_finite() {
        // Shear-like velocity (u_x varies along x) drives production; one
        // correct() step must keep k, ω > 0 and νt finite and non-negative.
        let m = line_mesh_with_wall();
        let mut model = KOmegaSST::new(m.clone());
        model.u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]); // ∂u_x/∂x = 1
        model.nu = VolScalarField::uniform("nu", m.clone(), 1e-3);
        model.dt = 1e-3;
        for _ in 0..5 {
            model.correct();
        }
        for c in 0..3 {
            assert!(model.k.internal[c] > 0.0, "k must stay positive");
            assert!(model.omega.internal[c] > 0.0, "omega must stay positive");
            let nut = model.nu_t.internal[c];
            assert!(nut.is_finite() && nut >= 0.0, "nut must be finite and >= 0");
            assert!((0.0..=1.0).contains(&model.f1.internal[c]), "F1 in [0,1]");
        }
    }
}
