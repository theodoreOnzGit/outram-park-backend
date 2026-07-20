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

//! Smagorinsky (1963) LES sub-grid-scale (SGS) eddy-viscosity model.
//!
//! Pure-Rust port of OpenFOAM's `LESModels::Smagorinsky`
//! (`src/MomentumTransportModels/momentumTransportModels/LES/Smagorinsky/`,
//! `Smagorinsky.C:39-67`, `Smagorinsky.H:38-59`) on top of
//! `LESModels::LESeddyViscosity` (`LESeddyViscosity.C:60-61` for the default
//! coefficients).
//!
//! ## What this computes
//!
//! Unlike the textbook algebraic form `ν_sgs = (Cs·Δ)²·|S|`, OpenFOAM's
//! Smagorinsky is derived from the one-equation `kEqn` SGS model **assuming
//! local equilibrium** (production = dissipation). That gives a closed
//! algebraic estimate of the SGS kinetic energy `k` (m²/s²) from the resolved
//! strain, and then the eddy viscosity from `k` (`Smagorinsky.H:38-50`):
//!
//! ```text
//!   D  = symm(grad U)                              (resolved strain-rate tensor, 1/s)
//!   a  = Ce / Δ                                    (1/(m·s) after ×k^(3/2))
//!   b  = (2/3)·tr(D)                               (1/s; = 0 for divergence-free flow)
//!   c  = 2·Ck·Δ·(dev(D) : D)                       (m·(1/s²) = m/s²)
//!   √k = (-b + √(b² + 4·a·c)) / (2·a)              (m/s)  → k = (√k)²   (m²/s²)
//!   ν_sgs = Ck · Δ · √k                            (m²/s)
//! ```
//!
//! This is the exact quadratic root solved in `Smagorinsky.C:45-55` for the
//! `k(gradU)` protected member, followed by `correctNut()`
//! (`Smagorinsky.C:59-67`): `nut_ = Ck·delta·sqrt(k)`.
//!
//! ## Filter width Δ (the `cubeRootVol` delta)
//!
//! The LES filter width is the local grid scale. This port uses the
//! `cubeRootVol` choice — `Δ = V^(1/3)` per cell, `V` the cell volume (m³), so
//! `Δ` has units of metres. OpenFOAM offers other `delta` models
//! (`smooth`, `maxDeltaxyz`, `vanDriest`, …) selected at runtime; see the
//! "Honest scope" note.
//!
//! ## Default coefficients (`LESeddyViscosity.C:60-61`)
//!
//! * `Ck = 0.094` — SGS eddy-viscosity coefficient (dimensionless).
//! * `Ce = 1.048` — SGS dissipation coefficient (dimensionless).
//!
//! These correspond to an effective Smagorinsky constant
//! `Cs = (Ck·√(Ck/Ce))^(1/2) ≈ 0.17` for isotropic turbulence, which is why
//! the model is still called "Smagorinsky".
//!
//! ## State the model needs beyond the trait
//!
//! [`TurbulenceModel::correct`] takes no arguments, but the algebraic update
//! needs the live velocity field `U` and the molecular viscosity `ν`. As in
//! OpenFOAM (where the model holds references to those fields), this struct
//! stores them: set `u` and `nu` each time step before calling `correct()`.
//! Δ is a function of the (fixed) mesh and is precomputed once at construction.
//!
//! Because the model is **algebraic**, `correct()` solves no transport PDE —
//! it simply recomputes `k_sgs` and `ν_sgs` from the current strain field. It
//! is therefore markedly simpler than the RAS transport models (k-ω SST etc.).
//!
//! ## Honest scope — what is NOT modelled
//!
//! * **Only the `cubeRootVol` delta** (`Δ = V^(1/3)`) is implemented. No
//!   `maxDeltaxyz`, no `smooth`, and in particular **no van Driest near-wall
//!   damping** — so the raw model over-predicts `ν_sgs` in the viscous
//!   sublayer, exactly as unmodified OpenFOAM Smagorinsky does. Wall damping
//!   would require a wall-distance field and the `vanDriest` delta wrapper.
//! * The compressible / two-phase `alpha`, `rho`, `alphaRhoPhi` weighting from
//!   the templated base class is not carried; this is the incompressible
//!   (`ν`-based) specialisation, matching the rest of this crate.
//! * `read()` (runtime dictionary re-read of `Ck`/`Ce`) has no analogue —
//!   coefficients are named struct fields set in Rust.
//! * No end-to-end LES validation (decaying isotropic turbulence, channel
//!   flow) is performed here; the unit tests below verify the algebra and the
//!   Δ definition only. See the `## Bookkeeping status` gate in the crate
//!   README — the V&V axis is not human-cleared.

use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
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

/// SGS eddy-viscosity coefficient Ck (dimensionless). Default from
/// `LESeddyViscosity.C:60`. Appears in `ν_sgs = Ck·Δ·√k` and in the
/// `k`-quadratic coefficient `c = 2·Ck·Δ·(dev(D):D)`.
pub const CK: f64 = 0.094;

/// SGS dissipation coefficient Ce (dimensionless). Default from
/// `LESeddyViscosity.C:61`. Appears in the `k`-quadratic coefficient
/// `a = Ce/Δ` (from `ε = Ce·k^(3/2)/Δ` under local equilibrium).
pub const CE: f64 = 1.048;

/// Lower clamp to keep divisions/roots finite.
const SMALL: f64 = 1e-14;

/// Smagorinsky (1963) LES sub-grid-scale eddy-viscosity model.
///
/// Algebraic model: `correct()` recomputes the SGS kinetic energy `k_sgs`
/// (m²/s²) and kinematic eddy viscosity `ν_sgs` (m²/s) from the resolved
/// strain `D = symm(∇U)` (1/s) and the per-cell filter width `Δ = V^(1/3)`
/// (m). No transport equation is solved.
///
/// Set [`Smagorinsky::u`] (velocity, m/s) and [`Smagorinsky::nu`] (molecular
/// kinematic viscosity, m²/s) before each `correct()`.
pub struct Smagorinsky {
    pub mesh: Arc<FvMesh>,
    /// Sub-grid-scale kinematic viscosity ν_sgs [m²/s], = Ck·Δ·√k_sgs, ≥ 0.
    pub nu_sgs: VolScalarField,
    /// Sub-grid-scale kinetic energy k_sgs [m²/s²], the algebraic equilibrium
    /// root of `a·k + b·√k − c = 0`, ≥ 0.
    pub k_sgs: VolScalarField,
    /// Velocity field U [m/s] — set by the solver each step before `correct()`.
    pub u: VolVectorField,
    /// Molecular kinematic viscosity ν [m²/s].
    pub nu: VolScalarField,
    /// Turbulent Prandtl number Prt (dimensionless) for `alpha_eff` (default 0.85).
    pub prt: f64,
    /// SGS eddy-viscosity coefficient Ck (default [`CK`]).
    ck: f64,
    /// SGS dissipation coefficient Ce (default [`CE`]).
    ce: f64,
    /// LES filter width Δ [m] per cell — the `cubeRootVol` delta, `V^(1/3)`.
    /// Depends only on the (fixed) mesh, so it is precomputed at construction.
    delta: Vec<f64>,
}

impl Smagorinsky {
    /// Construct a Smagorinsky model over `mesh` with the OpenFOAM default
    /// coefficients (`Ck = 0.094`, `Ce = 1.048`), `ν_sgs`/`k_sgs` initialised
    /// to zero, `U = 0`, `ν = 1e-5 m²/s`, `Prt = 0.85`.
    ///
    /// The `cubeRootVol` filter width `Δ = V^(1/3)` is computed once here from
    /// the mesh cell volumes. Set `u` and `nu` each time step before calling
    /// [`TurbulenceModel::correct`].
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let nu_sgs = VolScalarField::zeros("nuSgs", mesh.clone());
        let k_sgs = VolScalarField::zeros("k", mesh.clone());
        let u = VolVectorField::zero("U", mesh.clone());
        let nu = VolScalarField::uniform("nu", mesh.clone(), 1e-5);
        let delta = cube_root_vol_delta(&mesh);
        Self {
            mesh,
            nu_sgs,
            k_sgs,
            u,
            nu,
            prt: 0.85,
            ck: CK,
            ce: CE,
            delta,
        }
    }

    /// Builder override for the SGS eddy-viscosity coefficient Ck (dimensionless).
    pub fn with_ck(mut self, ck: f64) -> Self {
        self.ck = ck;
        self
    }

    /// Builder override for the SGS dissipation coefficient Ce (dimensionless).
    pub fn with_ce(mut self, ce: f64) -> Self {
        self.ce = ce;
        self
    }

    /// The per-cell `cubeRootVol` filter width Δ = V^(1/3) [m] (read-only).
    pub fn delta(&self) -> &[f64] {
        &self.delta
    }

    /// Cell-centred velocity gradient ∇U (Gauss), one `Tensor` per cell [1/s].
    ///
    /// Identical Gauss-divergence construction to the RAS models: sum the
    /// face-interpolated dyad `U_f ⊗ Sf` over owner/neighbour, then divide by
    /// the cell volume.
    fn grad_u(&self) -> Vec<Tensor> {
        let mesh = &self.mesh;
        let u_f = fvc::interpolate(&self.u);
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
}

/// Per-cell `cubeRootVol` LES filter width Δ = V^(1/3) [m].
///
/// `V` is the cell volume (m³, always positive for a valid mesh), so Δ is a
/// real positive length. This is OpenFOAM's default `delta cubeRootVol` model.
fn cube_root_vol_delta(mesh: &FvMesh) -> Vec<f64> {
    (0..mesh.n_cells)
        .map(|c| mesh.cell_volumes[c].max(SMALL).cbrt())
        .collect()
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

impl TurbulenceModel for Smagorinsky {
    /// Turbulent deviatoric-stress divergence `∇·(−2 ν_eff dev(symm(∇U)))`.
    ///
    /// Model-independent: identical structure to the RAS models — only `ν_eff`
    /// = `ν + ν_sgs` differs by carrying the SGS eddy viscosity from the
    /// algebraic update. Returns an `FvVectorMatrix` to add to the momentum
    /// equation. See the k-ω SST implementation for the derivation of the
    /// implicit Laplacian + explicit transpose-correction split.
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix {
        // Effective viscosity ν_eff = ν + ν_sgs [m²/s].
        let n = self.mesh.n_cells;
        let nu = self.nu.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let nu_eff_cell: Vec<f64> = (0..n).map(|c| nu[c] + nut[c]).collect();
        let nu_eff = scalar_field("nuEff", self.mesh.clone(), nu_eff_cell);

        // Implicit dominant term: −∇·(ν_eff ∇U). `fvm::laplacian_vec` is
        // positive-definite (= −∇·(Γ∇)), added directly (OpenFOAM
        // `-fvm::laplacian(nuEff, U)`).
        let mut eqn = fvm::laplacian_vec(&nu_eff, u, self.mesh.clone());

        // Explicit transpose correction: −∇·(ν_eff·dev2(∇Uᵀ)), OpenFOAM
        // `- fvc::div(nuEff*dev2(T(grad U)))`.
        let grad_u = self.grad_u();
        let mut b = vec![Tensor::ZERO; n];
        for c in 0..n {
            b[c] = grad_u[c].transpose().dev2() * (nu[c] + nut[c]);
        }
        let mesh = &self.mesh;
        let mut div_b = vec![Vector3::ZERO; n];
        for f in 0..mesh.n_internal_faces {
            let bf = (b[mesh.owner[f]] + b[mesh.neighbour[f]]) * 0.5;
            let flux = tensor_dot_vec(bf, mesh.face_area_vectors[f]);
            div_b[mesh.owner[f]] = div_b[mesh.owner[f]] + flux;
            div_b[mesh.neighbour[f]] = div_b[mesh.neighbour[f]] - flux;
        }
        for c in 0..n {
            let dvb = div_b[c] * (1.0 / mesh.cell_volumes[c]); // ∇·B
            eqn.source[c] = eqn.source[c] + dvb * mesh.cell_volumes[c];
        }
        eqn
    }

    /// Algebraic SGS update (no PDE solve). Ports `Smagorinsky.C:45-67`.
    ///
    /// For each cell, from `D = symm(∇U)` (1/s) and `Δ = V^(1/3)` (m):
    /// solves `k = ((-b + √(b² + 4ac))/(2a))²` with `a = Ce/Δ`,
    /// `b = (2/3)·tr(D)`, `c = 2·Ck·Δ·(dev(D):D)`, then sets
    /// `ν_sgs = Ck·Δ·√k`. Both `k_sgs` and `ν_sgs` are guaranteed ≥ 0 because
    /// `a > 0`, `c = |dev(D)|² ≥ 0` ⇒ `√(b²+4ac) ≥ |b|` ⇒ numerator ≥ 0.
    fn correct(&mut self) {
        let n = self.mesh.n_cells;
        let grad_u = self.grad_u();
        let mut k_vals = vec![0.0_f64; n];
        let mut nut_vals = vec![0.0_f64; n];
        for c in 0..n {
            let d = grad_u[c].symm(); // SymmTensor D = symm(∇U) [1/s]
            let delta = self.delta[c].max(SMALL); // Δ [m]
            let a = self.ce / delta; // a = Ce/Δ
            let b = (2.0 / 3.0) * d.tr(); // b = (2/3)·tr(D)
            let c_coef = 2.0 * self.ck * delta * d.dev().double_inner(d); // c = 2·Ck·Δ·(dev(D):D)
            let sqrt_k = (-b + (b * b + 4.0 * a * c_coef).max(0.0).sqrt()) / (2.0 * a);
            let sqrt_k = sqrt_k.max(0.0);
            k_vals[c] = sqrt_k * sqrt_k; // k = (√k)²
            nut_vals[c] = (self.ck * delta * sqrt_k).max(0.0); // ν_sgs = Ck·Δ·√k
        }
        self.k_sgs.internal = Field::new(k_vals);
        self.nu_sgs.internal = Field::new(nut_vals);
    }

    fn nu_t(&self) -> &VolScalarField {
        &self.nu_sgs
    }

    /// Effective thermal diffusivity α_eff = α + α_t, α_t = ν_sgs / Prt [m²/s].
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        let n = self.mesh.n_cells;
        let a = alpha.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let vals: Vec<f64> = (0..n).map(|c| a[c] + nut[c] / self.prt).collect();
        scalar_field("alphaEff", self.mesh.clone(), vals)
    }

    /// Effective viscosity μ_eff = μ + μ_t (incompressible: ν + ν_sgs) [m²/s].
    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        let n = self.mesh.n_cells;
        let m = mu.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let vals: Vec<f64> = (0..n).map(|c| m[c] + nut[c]).collect();
        scalar_field("muEff", self.mesh.clone(), vals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// 3 cells along x, centres 0.5/1.5/2.5, unit volumes, unit x-faces.
    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// V&V — filter-width definition (`cubeRootVol` delta).
    ///
    /// **Methodology.** Reference: OpenFOAM `cubeRootVol` delta, `Δ = V^(1/3)`.
    /// Build a mesh whose cells have known volumes (1, 8, 27 m³) and assert the
    /// precomputed `delta` equals their cube roots (1, 2, 3 m) to < 1e-12.
    /// Pass criterion: exact analytical match.
    ///
    /// **Results (run 2026-07-20).** Measured `delta = [1.0, 2.0, 3.0]`,
    /// max |error| = 0.0 (< 1e-12). Confirms Δ is the cube-root of cell volume.
    #[test]
    fn delta_is_cube_root_of_volume() {
        let mesh = Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 8.0, 27.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        );
        let model = Smagorinsky::new(mesh);
        let d = model.delta();
        assert!((d[0] - 1.0).abs() < 1e-12);
        assert!((d[1] - 2.0).abs() < 1e-12);
        assert!((d[2] - 3.0).abs() < 1e-12);
    }

    /// V&V — algebraic `correct()` produces finite, non-negative ν_sgs that
    /// increases monotonically with strain magnitude.
    ///
    /// **Methodology.** Reference: the OpenFOAM equilibrium relation
    /// `ν_sgs = Ck·Δ·√k`, `k` the positive root of the strain-driven quadratic
    /// (`Smagorinsky.C:45-64`). Because `c = 2·Ck·Δ·|dev(D)|² ∝ |S|²` and, for
    /// divergence-free flow, `b = 0`, we have `√k ∝ |S|` so `ν_sgs ∝ |S|`.
    /// Drive the model with a linear x-velocity `u_x = s·x` (so `∂u_x/∂x = s`,
    /// a pure strain) for two shear rates `s = 1` and `s = 2` on a unit mesh,
    /// call `correct()`, and check: (i) every ν_sgs is finite and ≥ 0, (ii) the
    /// stronger strain gives strictly larger ν_sgs. Pass criterion: both hold.
    ///
    /// **Results (run 2026-07-20, interior cell 1, Δ = 1 m, Ck = 0.094,
    /// Ce = 1.048).** For `s = 1 s⁻¹`: k_sgs = 2.3038e-2 m²/s²,
    /// ν_sgs = 1.4268e-2 m²/s. For `s = 2 s⁻¹`: k_sgs = 9.2153e-2 m²/s²,
    /// ν_sgs = 2.8535e-2 m²/s. Ratio ν_sgs(2)/ν_sgs(1) = 2.000000, matching
    /// the expected linear `ν_sgs ∝ |S|` scaling (both `b ∝ s` and `c ∝ s²`,
    /// so `√k ∝ s`). All values finite and > 0. Interpretation: the algebraic
    /// quadratic root, the `dev(D):D` strain contraction, and the `Ck·Δ·√k`
    /// closure are wired correctly.
    #[test]
    fn correct_gives_finite_nonneg_nut_scaling_with_strain() {
        // shear rate s: u_x = s·x sampled at cell centres 0.5, 1.5, 2.5
        let run = |s: f64| -> Vec<f64> {
            let m = line_mesh();
            let mut model = Smagorinsky::new(m);
            model.u.internal = Field::new(vec![vx(0.5 * s), vx(1.5 * s), vx(2.5 * s)]);
            model.correct();
            model.nu_sgs.internal.as_slice().to_vec()
        };
        let nut1 = run(1.0);
        let nut2 = run(2.0);
        for c in 0..3 {
            assert!(nut1[c].is_finite() && nut1[c] >= 0.0, "nu_sgs finite & >= 0");
            assert!(nut2[c].is_finite() && nut2[c] >= 0.0, "nu_sgs finite & >= 0");
        }
        // interior cell 1 has a symmetric stencil → clean central-difference
        // gradient; stronger strain must give strictly larger eddy viscosity.
        assert!(nut1[1] > 0.0, "strain should produce non-zero nu_sgs");
        assert!(nut2[1] > nut1[1], "nu_sgs must grow with strain magnitude");
        // linear scaling ν_sgs ∝ |S|: doubling the shear doubles ν_sgs.
        assert!(
            (nut2[1] / nut1[1] - 2.0).abs() < 1e-9,
            "nu_sgs should scale linearly with strain (ratio {})",
            nut2[1] / nut1[1]
        );
    }
}
