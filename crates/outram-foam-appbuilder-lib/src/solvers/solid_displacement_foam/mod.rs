// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   applications/solvers/stressAnalysis/solidDisplacementFoam/
//     solidDisplacementFoam.C, createFields.H, readMechanicalProperties.H,
//     readThermalProperties.H, readSolidDisplacementFoamControls.H, calculateStress.H
//   src/finiteVolume/fvMatrices/fvMatrix/fvMatrixSolve.C (solveSegregated)
//   src/OpenFOAM/matrices/lduMatrix/solvers/PCG/PCG.C, lduMatrixSolver.C (normFactor)
//   src/OpenFOAM/matrices/lduMatrix/preconditioners/DICPreconditioner/DICPreconditioner.C
//   @ ESI OpenFOAM v2506
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
//
// This offering is not approved or endorsed by OpenCFD Limited, producer and
// distributor of the OpenFOAM(R) software via www.openfoam.com.

//! # `SolidDisplacementFoamEsi`: a translation of ESI `solidDisplacementFoam` (gh:#1383)
//!
//! Transient segregated finite-volume solver of linear-elastic, small-strain
//! deformation of a solid body with optional thermal diffusion and thermal
//! stresses. This is the **compactNormalStress, `steadyState` d2dt2** branch of
//! upstream's `solidDisplacementFoam.C`, transcribed line by line in upstream's
//! operation order so that it can be compared bit for bit with the real binary
//! (`verification_and_validation/upstream_solid_displacement_gh1383/README.md`).
//!
//! It is a NEW solver beside, and independent of, GeN-Foam's
//! [`MechanicsMeshSolver`](crate::genfoam::thermo_mechanics::MechanicsMeshSolver),
//! whose discretisation differs (see that module's "Known limitations").
//!
//! ## Per outer corrector (upstream)
//!
//! 1. `thermalStress`: `solve(fvm::ddt(T) == fvm::laplacian(DT, T))` (Euler).
//! 2. `DEqn: fvm::d2dt2(D) == fvm::laplacian(2 mu + lambda, D) + divSigmaExp`
//!    (`steadyState` d2dt2 is an empty matrix), plus `fvc::grad(threeKalpha*T)`
//!    when thermal; solved component by component as `fvMatrix::solveSegregated`
//!    does (diagonal += `internalCoeffs`, source += `boundaryCoeffs`).
//! 3. `gradD = fvc::grad(D)` (Gauss linear **with** upstream's patch-normal
//!    boundary correction, [`grad_vec_corrected`]); `sigmaD = mu twoSymm(gradD)
//!    + lambda tr(gradD) I`; `divSigmaExp = fvc::div(sigmaD - (2 mu + lambda) gradD)`.
//! 4. Repeat while `initialResidual > D` tolerance and `++iCorr < nCorrectors`.
//!
//! `mu`, `lambda`, `threeK`, `k` are normalised by `rho` as in
//! `readMechanicalProperties.H` (`E/rho`).
//!
//! ## Scheme selection (fixed, refused by name otherwise)
//!
//! `Gauss linear` for `grad` and `div(sigmaD)`, `Gauss linear uncorrected` for
//! `laplacian(DD,D)` and `laplacian(DT,T)` (so the matrix uses
//! `nonOrthDeltaCoeffs` on internal faces and the patch `deltaCoeffs` on
//! boundary faces, [`DeltaCoeff`]), `linear` interpolation, PCG with DIC.
//! Not implemented: `compactNormalStress no` (needs `fvMatrix::flux()`), transient
//! `d2dt2`, `tractionDisplacement`, field-valued `E`/`nu`/`rho`, `empty` patches,
//! GAMG. The first is refused with [`AppBuilderError::UnsupportedScheme`].
//!
//! Units: SI base units as raw `f64` (the FV operator layer is un-`uom`'d).

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::{
    VolScalarField, VolTensorField, VolVectorField,
};
use outram_foam_basic_lib::fv_operators::fvc::{grad, grad_vec, grad_vec_corrected, interpolate};
use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use outram_foam_basic_lib::fv_operators::les_filter::surface_integrate;
use outram_foam_basic_lib::mesh::fv_mesh::PatchKind;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SymmTensor, Tensor, Vector3};

use crate::error::AppBuilderError;

/// Boundary condition of the displacement `D` on one mesh patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplacementPatch {
    /// `fixedValue` with a uniform displacement (m).
    FixedValue(Vector3),
    /// `zeroGradient`.
    ZeroGradient,
}

/// Boundary condition of the temperature `T` on one mesh patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemperaturePatch {
    /// `fixedValue` (K).
    FixedValue(f64),
    /// `zeroGradient`.
    ZeroGradient,
}

/// `thermalProperties` with `thermalStress yes` (uniform values only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalProperties {
    /// Specific heat `C` (J/kg/K).
    pub c: f64,
    /// Conductivity `k` (W/m/K); upstream reads it as `rhoK` and divides by `rho`.
    pub k: f64,
    /// Linear thermal expansion coefficient `alpha` (1/K).
    pub alpha: f64,
}

/// `mechanicalProperties` and `thermalProperties` (uniform values only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidProperties {
    /// Density (kg/m^3).
    pub rho: f64,
    /// Young's modulus (Pa); upstream reads it as `rhoE` and normalises by `rho`.
    pub young: f64,
    /// Poisson's ratio.
    pub nu: f64,
    /// `planeStress yes`.
    pub plane_stress: bool,
    /// `Some` when `thermalStress yes`.
    pub thermal: Option<ThermalProperties>,
}

/// The `rho`-normalised coefficients of `readMechanicalProperties.H` and
/// `readThermalProperties.H`, in upstream's expression order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalisedCoefficients {
    /// `E/rho`.
    pub e: f64,
    /// `E/(2(1+nu))`.
    pub mu: f64,
    /// Lame's first coefficient (plane strain or plane stress form).
    pub lambda: f64,
    /// `E/(1-2nu)` (or `E/(1-nu)` for plane stress).
    pub three_k: f64,
    /// `2 mu + lambda`, the implicit Laplacian coefficient.
    pub two_mu_lambda: f64,
    /// `threeK*alpha` (0 when not thermal).
    pub three_k_alpha: f64,
    /// `DT = (rhoK/rho)/C` (0 when not thermal).
    pub dt: f64,
}

impl SolidProperties {
    /// Upstream's coefficient algebra, `E = rhoE/rho`, `mu = E/(2.0*(1.0 + nu))`,
    /// `lambda = nu*E/((1.0 + nu)*(1.0 - 2.0*nu))`, `threeK = E/(1.0 - 2.0*nu)`
    /// (plane stress: `lambda = nu*E/((1.0 + nu)*(1.0 - nu))`, `threeK = E/(1.0 - nu)`),
    /// `threeKalpha = threeK*alpha`, `DT = (rhoK/rho)/C`.
    pub fn normalised(&self) -> NormalisedCoefficients {
        let e = self.young / self.rho;
        let nu = self.nu;
        let mu = e / (2.0 * (1.0 + nu));
        let (lambda, three_k) = if self.plane_stress {
            (nu * e / ((1.0 + nu) * (1.0 - nu)), e / (1.0 - nu))
        } else {
            (nu * e / ((1.0 + nu) * (1.0 - 2.0 * nu)), e / (1.0 - 2.0 * nu))
        };
        let (three_k_alpha, dt) = match self.thermal {
            Some(th) => {
                let k = th.k / self.rho;
                (three_k * th.alpha, k / th.c)
            }
            None => (0.0, 0.0),
        };
        NormalisedCoefficients {
            e,
            mu,
            lambda,
            three_k,
            two_mu_lambda: 2.0 * mu + lambda,
            three_k_alpha,
            dt,
        }
    }
}

/// A `PCG` + `DIC` linear-solver entry of `fvSolution`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcgSettings {
    /// Absolute tolerance.
    pub tolerance: f64,
    /// Relative tolerance (0 disables, as upstream).
    pub rel_tol: f64,
    /// `maxIter`.
    pub max_iter: usize,
}

/// `fvSolution` `stressAnalysis` and `solvers` entries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidDisplacementControls {
    /// `compactNormalStress` (only `true` is implemented).
    pub compact_normal_stress: bool,
    /// `nCorrectors`.
    pub n_correctors: usize,
    /// `D` convergence tolerance on the initial residual.
    pub convergence_tolerance: f64,
    /// Solver entry for `D`.
    pub d_solver: PcgSettings,
    /// Solver entry for `T`.
    pub t_solver: PcgSettings,
}

/// One linear solve, as upstream logs it (`DICPCG: Solving for Dx, ...`).
#[derive(Debug, Clone, PartialEq)]
pub struct SolveLog {
    /// Field/component name, `"T"`, `"Dx"`, `"Dy"` or `"Dz"`.
    pub field: &'static str,
    /// Normalised initial residual.
    pub initial_residual: f64,
    /// Normalised final residual.
    pub final_residual: f64,
    /// PCG iterations.
    pub n_iterations: usize,
}

/// Everything one time step did: every linear solve in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StepLog {
    /// Solves in upstream's order (`T`, `Dx`, `Dy`, `Dz` per corrector).
    pub solves: Vec<SolveLog>,
}

/// Symmetric LDU system on a mesh: `diag`, `upper = lower`.
struct SymLdu {
    diag: Vec<f64>,
    upper: Vec<f64>,
}

/// `lduMatrix::Amul` for a symmetric matrix without coupled interfaces.
fn amul(m: &SymLdu, owner: &[usize], neighbour: &[usize], psi: &[f64]) -> Vec<f64> {
    let mut a: Vec<f64> = (0..psi.len()).map(|c| m.diag[c] * psi[c]).collect();
    for f in 0..m.upper.len() {
        a[neighbour[f]] += m.upper[f] * psi[owner[f]];
        a[owner[f]] += m.upper[f] * psi[neighbour[f]];
    }
    a
}

/// `lduMatrix::solver::normFactor` (`lduMatrixSolver.C`).
fn norm_factor(m: &SymLdu, owner: &[usize], neighbour: &[usize], psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    let mut sum_a = m.diag.clone();
    for f in 0..m.upper.len() {
        sum_a[neighbour[f]] += m.upper[f];
        sum_a[owner[f]] += m.upper[f];
    }
    let mut s = 0.0;
    for p in psi {
        s += *p;
    }
    let avg = s / psi.len() as f64;
    let mut total = 0.0;
    for c in 0..psi.len() {
        let t = sum_a[c] * avg;
        total += (apsi[c] - t).abs() + (source[c] - t).abs();
    }
    total + 1.0e-20
}

/// `Foam::PCG::solve` with the `DIC` preconditioner (ESI v2506), starting from
/// the current `psi`. Returns (initial residual, final residual, iterations).
fn pcg_dic(
    m: &SymLdu,
    owner: &[usize],
    neighbour: &[usize],
    source: &[f64],
    psi: &mut [f64],
    s: PcgSettings,
) -> (f64, f64, usize) {
    let n = psi.len();
    let apsi = amul(m, owner, neighbour, psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - apsi[c]).collect();
    let nf = norm_factor(m, owner, neighbour, psi, source, &apsi);
    let sum_mag = |v: &[f64]| {
        let mut t = 0.0;
        for x in v {
            t += x.abs();
        }
        t
    };
    let initial = sum_mag(&r_a) / nf;
    let mut final_res = initial;
    let mut n_iter = 0usize;
    let converged = |fin: f64| fin < s.tolerance || (s.rel_tol > 1.0e-15 && fin < s.rel_tol * initial);
    if converged(final_res) {
        return (initial, final_res, 0);
    }
    // DICPreconditioner::calcReciprocalD
    let mut r_d = m.diag.clone();
    for f in 0..m.upper.len() {
        r_d[neighbour[f]] -= m.upper[f] * m.upper[f] / r_d[owner[f]];
    }
    for d in r_d.iter_mut() {
        *d = 1.0 / *d;
    }
    let mut p_a = vec![0.0; n];
    let mut w_ar_a = 0.0;
    loop {
        let w_ar_a_old = w_ar_a;
        // DICPreconditioner::precondition
        let mut w_a: Vec<f64> = (0..n).map(|c| r_d[c] * r_a[c]).collect();
        for f in 0..m.upper.len() {
            w_a[neighbour[f]] -= r_d[neighbour[f]] * m.upper[f] * w_a[owner[f]];
        }
        for f in (0..m.upper.len()).rev() {
            w_a[owner[f]] -= r_d[owner[f]] * m.upper[f] * w_a[neighbour[f]];
        }
        w_ar_a = 0.0;
        for c in 0..n {
            w_ar_a += w_a[c] * r_a[c];
        }
        if n_iter == 0 {
            p_a.copy_from_slice(&w_a);
        } else {
            let beta = w_ar_a / w_ar_a_old;
            for c in 0..n {
                p_a[c] = w_a[c] + beta * p_a[c];
            }
        }
        let w_a2 = amul(m, owner, neighbour, &p_a);
        let mut w_ap_a = 0.0;
        for c in 0..n {
            w_ap_a += w_a2[c] * p_a[c];
        }
        if w_ap_a.abs() / nf < 1.0e-300 {
            break;
        }
        let alpha = w_ar_a / w_ap_a;
        for c in 0..n {
            psi[c] += alpha * p_a[c];
            r_a[c] -= alpha * w_a2[c];
        }
        final_res = sum_mag(&r_a) / nf;
        n_iter += 1;
        if !(n_iter < s.max_iter && !converged(final_res)) {
            break;
        }
    }
    (initial, final_res, n_iter)
}

/// Face quantities of a mesh that the operators below reuse.
struct FaceGeometry {
    mag_sf: Vec<f64>,
    /// `nonOrthDeltaCoeffs` on internal faces.
    dc_internal: Vec<f64>,
    /// Patch `deltaCoeffs` (`1/mag(fvPatch::delta())`) per boundary face, per patch.
    dc_boundary: Vec<Vec<f64>>,
}

impl FaceGeometry {
    fn new(mesh: &FvMesh) -> Self {
        let mag_sf = (0..mesh.n_faces).map(|f| mesh.face_area_vectors[f].mag()).collect();
        let dc_internal = (0..mesh.n_internal_faces)
            .map(|f| DeltaCoeff::NonOrthogonal.interior_delta_coeff(mesh, f))
            .collect();
        let dc_boundary = mesh
            .patches
            .iter()
            .map(|p| {
                (0..p.size)
                    .map(|fi| DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, p.start + fi))
                    .collect()
            })
            .collect();
        Self { mag_sf, dc_internal, dc_boundary }
    }
}

/// The solver state. Build with [`Self::new`], advance with [`Self::step`].
#[derive(Debug, Clone)]
pub struct SolidDisplacementFoamEsi {
    mesh: Arc<FvMesh>,
    coeffs: NormalisedCoefficients,
    props: SolidProperties,
    controls: SolidDisplacementControls,
    delta_t: f64,
    d_patches: Vec<DisplacementPatch>,
    t_patches: Vec<TemperaturePatch>,
    d: Vec<Vector3>,
    d_b: Vec<Vec<Vector3>>,
    t: Vec<f64>,
    t_b: Vec<Vec<f64>>,
    div_sigma_exp: Vec<Vector3>,
    sigma_d: Vec<SymmTensor>,
    zero_gradient_boundary_grad: bool,
}

impl std::fmt::Debug for FaceGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FaceGeometry({} faces)", self.mag_sf.len())
    }
}

/// `mu*twoSymm(g) + (lambda*I)*tr(g)` of `createFields.H`/`solidDisplacementFoam.C`.
fn sigma_d_of(g: &Tensor, mu: f64, lambda: f64) -> SymmTensor {
    let two = g.two_symm();
    let tr = g.xx + g.yy + g.zz;
    let li = lambda * tr;
    SymmTensor {
        xx: mu * two.xx + li,
        xy: mu * two.xy,
        xz: mu * two.xz,
        yy: mu * two.yy + li,
        yz: mu * two.yz,
        zz: mu * two.zz + li,
    }
}

/// `operator-(SymmTensor, Tensor)` of `TensorI.H` with `gamma*g` as the tensor.
fn sigma_minus_gamma_grad(s: &SymmTensor, g: &Tensor, gamma: f64) -> Tensor {
    let gg = Tensor::new(
        gamma * g.xx, gamma * g.xy, gamma * g.xz,
        gamma * g.yx, gamma * g.yy, gamma * g.yz,
        gamma * g.zx, gamma * g.zy, gamma * g.zz,
    );
    Tensor::new(
        s.xx - gg.xx, s.xy - gg.xy, s.xz - gg.xz,
        s.xy - gg.yx, s.yy - gg.yy, s.yz - gg.yz,
        s.xz - gg.zx, s.yz - gg.zy, s.zz - gg.zz,
    )
}

fn tensor_field(mesh: &Arc<FvMesh>, name: &str, cells: Vec<Tensor>, boundary: Vec<Vec<Tensor>>) -> VolTensorField {
    let bnd = boundary
        .into_iter()
        .map(|v| {
            let f = Field::new(v);
            PatchField { bc: BoundaryCondition::Calculated(f.clone()), values: f }
        })
        .collect();
    VolTensorField::new(name, mesh.clone(), Field::new(cells), bnd)
}

impl SolidDisplacementFoamEsi {
    /// Build the solver in its initial state (upstream `createFields.H`):
    /// `D = 0` in the interior (boundary per `d_patches`), `T = t0` (when thermal),
    /// and the initial `divSigmaExp`.
    ///
    /// `t_patches` must have one entry per mesh patch when `props.thermal` is
    /// `Some`, and is ignored otherwise.
    ///
    /// # Errors
    /// [`AppBuilderError::UnsupportedScheme`] for `compactNormalStress no`;
    /// [`AppBuilderError::Case`] when a patch list does not match the mesh or the
    /// mesh has an `empty` patch.
    pub fn new(
        mesh: Arc<FvMesh>,
        props: SolidProperties,
        controls: SolidDisplacementControls,
        delta_t: f64,
        d_patches: Vec<DisplacementPatch>,
        t_patches: Vec<TemperaturePatch>,
        t0: f64,
    ) -> Result<Self, AppBuilderError> {
        if !controls.compact_normal_stress {
            return Err(AppBuilderError::UnsupportedScheme {
                family: "stressAnalysis",
                scheme: "compactNormalStress no".to_string(),
                reason: "needs fvMatrix::flux() for divSigmaExp = fvc::div(DEqn.flux()); not ported",
            });
        }
        let np = mesh.patches.len();
        let bad = d_patches.len() != np
            || (props.thermal.is_some() && t_patches.len() != np)
            || mesh.patches.iter().any(|p| p.kind == PatchKind::Empty);
        if bad {
            return Err(AppBuilderError::Case {
                case: std::path::PathBuf::new(),
                msg: "solidDisplacementFoam: one D (and T) patch condition per patch, no empty patches".to_string(),
            });
        }
        let coeffs = props.normalised();
        let d_b: Vec<Vec<Vector3>> = mesh
            .patches
            .iter()
            .zip(&d_patches)
            .map(|(p, bc)| match bc {
                DisplacementPatch::FixedValue(v) => vec![*v; p.size],
                DisplacementPatch::ZeroGradient => vec![Vector3::ZERO; p.size],
            })
            .collect();
        let t_b: Vec<Vec<f64>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(i, p)| match t_patches.get(i) {
                Some(TemperaturePatch::FixedValue(v)) if props.thermal.is_some() => vec![*v; p.size],
                _ => vec![t0; p.size],
            })
            .collect();
        let n = mesh.n_cells;
        let mut s = Self {
            coeffs,
            props,
            controls,
            delta_t,
            d_patches,
            t_patches,
            d: vec![Vector3::ZERO; n],
            d_b,
            t: vec![t0; n],
            t_b,
            div_sigma_exp: vec![Vector3::ZERO; n],
            sigma_d: vec![SymmTensor::ZERO; n],
            zero_gradient_boundary_grad: false,
            mesh,
        };
        s.correct_boundary_conditions();
        s.initial_div_sigma_exp();
        Ok(s)
    }

    /// ABLATION (default off): take the boundary values of `grad(D)` from the owner
    /// cell, as GeN-Foam's `MechanicsMeshSolver` does through the basic-lib
    /// `fvc::grad_vec` (zero-gradient output boundary), instead of upstream's
    /// patch-normal correction (`gaussGrad::correctBoundaryConditions`). Used to
    /// localise the difference between the two ports (gh:#1383). Call before the
    /// first [`Self::step`]; it also refreshes the initial `divSigmaExp`.
    #[must_use]
    pub fn without_boundary_gradient_correction(mut self) -> Self {
        self.zero_gradient_boundary_grad = true;
        self.initial_div_sigma_exp();
        self
    }

    /// `fvc::grad(D)`: upstream's Gauss linear gradient with the patch-normal boundary
    /// correction, or (ablation) the uncorrected owner-cell boundary values.
    fn grad_d(&self) -> VolTensorField {
        let field = self.d_field();
        if !self.zero_gradient_boundary_grad {
            return grad_vec_corrected(&field);
        }
        let g = grad_vec(&field);
        let mesh = &self.mesh;
        let bnd = mesh
            .patches
            .iter()
            .map(|p| {
                let vals = Field::new((0..p.size).map(|fi| g.internal[mesh.owner[p.start + fi]]).collect());
                PatchField { bc: BoundaryCondition::Calculated(vals.clone()), values: vals }
            })
            .collect();
        VolTensorField::new("grad(D)", mesh.clone(), g.internal.clone(), bnd)
    }

    /// The displacement field, one vector per cell (m).
    pub fn displacement(&self) -> &[Vector3] {
        &self.d
    }

    /// The temperature field, one value per cell (K); `t0` when not thermal.
    pub fn temperature(&self) -> &[f64] {
        &self.t
    }

    /// `volField::correctBoundaryConditions` for `D` and `T`: `zeroGradient`
    /// patches take the owner-cell value, `fixedValue` patches keep theirs.
    fn correct_boundary_conditions(&mut self) {
        for (pi, p) in self.mesh.patches.iter().enumerate() {
            if self.d_patches[pi] == DisplacementPatch::ZeroGradient {
                for fi in 0..p.size {
                    self.d_b[pi][fi] = self.d[self.mesh.owner[p.start + fi]];
                }
            }
            if self.props.thermal.is_some() && self.t_patches[pi] == TemperaturePatch::ZeroGradient {
                for fi in 0..p.size {
                    self.t_b[pi][fi] = self.t[self.mesh.owner[p.start + fi]];
                }
            }
        }
    }

    fn d_field(&self) -> VolVectorField {
        let bnd = self
            .mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, _)| PatchField {
                bc: match self.d_patches[pi] {
                    DisplacementPatch::FixedValue(v) => BoundaryCondition::FixedValue(v),
                    DisplacementPatch::ZeroGradient => BoundaryCondition::ZeroGradient,
                },
                values: Field::new(self.d_b[pi].clone()),
            })
            .collect();
        VolVectorField::new("D", self.mesh.clone(), Field::new(self.d.clone()), bnd)
    }

    /// `sigmaD` on the cells and boundary faces from a corrected `grad(D)`.
    fn sigma_d_fields(&self, gd: &VolTensorField) -> (Vec<SymmTensor>, Vec<Vec<SymmTensor>>) {
        let (mu, lambda) = (self.coeffs.mu, self.coeffs.lambda);
        let cells = (0..self.mesh.n_cells).map(|c| sigma_d_of(&gd.internal[c], mu, lambda)).collect();
        let bnd = gd
            .boundary
            .iter()
            .map(|pf| pf.values.iter().map(|g| sigma_d_of(g, mu, lambda)).collect())
            .collect();
        (cells, bnd)
    }

    /// `fvc::div(vf)` with `Gauss linear` for a tensor field `vf`:
    /// `surfaceIntegrate(Sf & interpolate(vf))`.
    fn div_of_tensor_field(&self, t: &VolTensorField) -> Vec<Vector3> {
        let tf = interpolate(t);
        let mesh = &self.mesh;
        let internal: Vec<Vector3> = (0..mesh.n_internal_faces)
            .map(|f| Tensor::vec_mat(mesh.face_area_vectors[f], tf.internal[f]))
            .collect();
        let boundary: Vec<Vec<Vector3>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| Tensor::vec_mat(mesh.face_area_vectors[p.start + fi], tf.boundary[pi].values[fi]))
                    .collect()
            })
            .collect();
        surface_integrate(mesh, &internal, &boundary)
    }

    /// `fvc::laplacian(gamma, D)` with a uniform `gamma` and an `uncorrected`
    /// snGrad: `surfaceIntegrate((gamma_f * snGrad(D)) * magSf)`.
    fn laplacian_of_d(&self, gamma: f64) -> Vec<Vector3> {
        let mesh = &self.mesh;
        let geo = FaceGeometry::new(mesh);
        let gamma_v = VolScalarField::uniform("gamma", mesh.clone(), gamma);
        let gamma_f = interpolate(&gamma_v);
        let internal: Vec<Vector3> = (0..mesh.n_internal_faces)
            .map(|f| {
                let sn = (self.d[mesh.neighbour[f]] - self.d[mesh.owner[f]]) * geo.dc_internal[f];
                (sn * gamma_f.internal[f]) * geo.mag_sf[f]
            })
            .collect();
        let boundary: Vec<Vec<Vector3>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        let gf = p.start + fi;
                        let sn = match self.d_patches[pi] {
                            DisplacementPatch::ZeroGradient => Vector3::ZERO,
                            DisplacementPatch::FixedValue(_) => {
                                (self.d_b[pi][fi] - self.d[mesh.owner[gf]]) * geo.dc_boundary[pi][fi]
                            }
                        };
                        (sn * gamma_f.boundary[pi].values[fi]) * geo.mag_sf[gf]
                    })
                    .collect()
            })
            .collect();
        surface_integrate(mesh, &internal, &boundary)
    }

    /// `createFields.H`: `divSigmaExp = fvc::div(sigmaD) - fvc::laplacian(2mu+lambda, D)`.
    fn initial_div_sigma_exp(&mut self) {
        let gd = self.grad_d();
        let (sc, sb) = self.sigma_d_fields(&gd);
        let sym_bnd: Vec<Vec<Tensor>> = sb.iter().map(|v| v.iter().map(|s| Tensor::from(*s)).collect()).collect();
        let sym_cells: Vec<Tensor> = sc.iter().map(|s| Tensor::from(*s)).collect();
        let sigma_field = tensor_field(&self.mesh, "sigmaD", sym_cells, sym_bnd);
        let div = self.div_of_tensor_field(&sigma_field);
        let lap = self.laplacian_of_d(self.coeffs.two_mu_lambda);
        self.div_sigma_exp = (0..self.mesh.n_cells).map(|c| div[c] - lap[c]).collect();
        self.sigma_d = sc;
    }

    /// Assemble `fvm::laplacian(gamma, psi)` (uncorrected, Dirichlet or
    /// zero-gradient patches) as `A psi = source` of `ddt - laplacian` /
    /// `0 - laplacian` form: positive diagonal, negative off-diagonals.
    /// Returns (matrix without boundary terms, per patch (internalCoeffs,
    /// boundaryCoeffs-factor) where the factor multiplies the fixed value).
    fn assemble_laplacian(&self, gamma: f64, fixed: &[bool]) -> (SymLdu, Vec<Vec<f64>>) {
        let mesh = &self.mesh;
        let geo = FaceGeometry::new(mesh);
        let gamma_v = VolScalarField::uniform("gamma", mesh.clone(), gamma);
        let gamma_f = interpolate(&gamma_v);
        let n_if = mesh.n_internal_faces;
        let mut upper = vec![0.0; n_if];
        let mut b_diag = vec![0.0; mesh.n_cells];
        for f in 0..n_if {
            let g_mag_sf = gamma_f.internal[f] * geo.mag_sf[f];
            upper[f] = geo.dc_internal[f] * g_mag_sf;
        }
        // negSumDiag: diag[l] -= lower; diag[u] -= upper (lower = upper)
        for f in 0..n_if {
            b_diag[mesh.owner[f]] -= upper[f];
            b_diag[mesh.neighbour[f]] -= upper[f];
        }
        // ddt/steady A minus the Laplacian B: diag = 0 - B.diag, upper = 0 - B.upper
        let diag: Vec<f64> = b_diag.iter().map(|d| 0.0 - *d).collect();
        let off: Vec<f64> = upper.iter().map(|u| 0.0 - *u).collect();
        // internalCoeffs of A - B for a fixedValue patch: gammaMagSf * deltaCoeffs
        let int_coeffs: Vec<Vec<f64>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        if fixed[pi] {
                            let g_mag_sf = gamma_f.boundary[pi].values[fi] * geo.mag_sf[p.start + fi];
                            g_mag_sf * geo.dc_boundary[pi][fi]
                        } else {
                            0.0
                        }
                    })
                    .collect()
            })
            .collect();
        (SymLdu { diag, upper: off }, int_coeffs)
    }

    /// `boundaryCoeffs` of `A - B` for one component of a fixedValue patch face:
    /// `gammaMagSf * (deltaCoeffs * value)`.
    fn boundary_coeff(&self, gamma: f64, pi: usize, fi: usize, value: f64) -> f64 {
        let mesh = &self.mesh;
        let gamma_v = VolScalarField::uniform("gamma", mesh.clone(), gamma);
        let gamma_f = interpolate(&gamma_v);
        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, mesh.patches[pi].start + fi);
        let g_mag_sf = gamma_f.boundary[pi].values[fi] * mesh.face_area_vectors[mesh.patches[pi].start + fi].mag();
        0.0 - ((0.0 - g_mag_sf) * (dc * value))
    }

    /// `solve(fvm::ddt(T) == fvm::laplacian(DT, T))`.
    fn solve_temperature(&mut self, t_old: &[f64], log: &mut StepLog) {
        let mesh = self.mesh.clone();
        let fixed: Vec<bool> = self.t_patches.iter().map(|b| matches!(b, TemperaturePatch::FixedValue(_))).collect();
        let (mut m, int_coeffs) = self.assemble_laplacian(self.coeffs.dt, &fixed);
        let r_delta_t = 1.0 / self.delta_t;
        let mut source: Vec<f64> = (0..mesh.n_cells).map(|c| r_delta_t * t_old[c] * mesh.cell_volumes[c]).collect();
        // ddt.diag - B.diag, B.diag = negSumDiag of the Laplacian (all negative)
        let b_diag = self.laplacian_negative_diag(self.coeffs.dt);
        for c in 0..mesh.n_cells {
            m.diag[c] = r_delta_t * mesh.cell_volumes[c] - b_diag[c];
        }
        for (pi, p) in mesh.patches.iter().enumerate() {
            for fi in 0..p.size {
                let c = mesh.owner[p.start + fi];
                m.diag[c] += int_coeffs[pi][fi];
                if let TemperaturePatch::FixedValue(v) = self.t_patches[pi] {
                    source[c] += self.boundary_coeff(self.coeffs.dt, pi, fi, v);
                }
            }
        }
        let mut psi = self.t.clone();
        let (i, f, n) = pcg_dic(&m, &mesh.owner, &mesh.neighbour, &source, &mut psi, self.controls.t_solver);
        self.t = psi;
        log.solves.push(SolveLog { field: "T", initial_residual: i, final_residual: f, n_iterations: n });
        self.correct_boundary_conditions();
    }

    /// `B.diag` of the Laplacian matrix B (negative: `negSumDiag`).
    fn laplacian_negative_diag(&self, gamma: f64) -> Vec<f64> {
        let mesh = &self.mesh;
        let geo = FaceGeometry::new(mesh);
        let gamma_v = VolScalarField::uniform("gamma", mesh.clone(), gamma);
        let gamma_f = interpolate(&gamma_v);
        let mut d = vec![0.0; mesh.n_cells];
        for f in 0..mesh.n_internal_faces {
            let up = geo.dc_internal[f] * (gamma_f.internal[f] * geo.mag_sf[f]);
            d[mesh.owner[f]] -= up;
            d[mesh.neighbour[f]] -= up;
        }
        d
    }

    /// `threeKalpha*T` as a volScalarField and its Gauss gradient `fvc::grad`.
    fn grad_three_k_alpha_t(&self) -> Vec<Vector3> {
        let a = self.coeffs.three_k_alpha;
        let bnd = self
            .mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, _)| {
                let vals = Field::new(self.t_b[pi].iter().map(|t| a * *t).collect());
                PatchField { bc: BoundaryCondition::Calculated(vals.clone()), values: vals }
            })
            .collect();
        let f = VolScalarField::new(
            "threeKalpha*T",
            self.mesh.clone(),
            Field::new(self.t.iter().map(|t| a * *t).collect()),
            bnd,
        );
        let g = grad(&f);
        (0..self.mesh.n_cells).map(|c| g.internal[c]).collect()
    }

    /// Advance one time step (`runTime.loop()` body of `solidDisplacementFoam.C`).
    pub fn step(&mut self) -> StepLog {
        let mesh = self.mesh.clone();
        let mut log = StepLog::default();
        let thermal = self.props.thermal.is_some();
        let t_old = self.t.clone();
        let fixed: Vec<bool> = self.d_patches.iter().map(|b| matches!(b, DisplacementPatch::FixedValue(_))).collect();
        let mut i_corr = 0usize;
        loop {
            if thermal {
                self.solve_temperature(&t_old, &mut log);
            }
            // DEqn: 0 == laplacian(2mu+lambda, D) + divSigmaExp  [A - B, steadyState d2dt2]
            let (mut m, int_coeffs) = self.assemble_laplacian(self.coeffs.two_mu_lambda, &fixed);
            // B.diag = -(sum) so that A.diag - B.diag = 0 - B.diag (already in m.diag)
            let mut src: Vec<Vector3> = (0..mesh.n_cells)
                .map(|c| {
                    let vf = self.div_sigma_exp[c] * mesh.cell_volumes[c];
                    // B.source = 0 - V*divSigmaExp ; A.source - B.source = 0 - B.source
                    let b = Vector3::new(0.0 - vf.x, 0.0 - vf.y, 0.0 - vf.z);
                    Vector3::new(0.0 - b.x, 0.0 - b.y, 0.0 - b.z)
                })
                .collect();
            if thermal {
                let gt = self.grad_three_k_alpha_t();
                for c in 0..mesh.n_cells {
                    let vg = gt[c] * mesh.cell_volumes[c];
                    src[c] = Vector3::new(src[c].x - vg.x, src[c].y - vg.y, src[c].z - vg.z);
                }
            }
            for (pi, p) in mesh.patches.iter().enumerate() {
                for fi in 0..p.size {
                    let c = mesh.owner[p.start + fi];
                    m.diag[c] += int_coeffs[pi][fi];
                }
            }
            let mut max_initial = 0.0_f64;
            let names = ["Dx", "Dy", "Dz"];
            let mut comps = [
                self.d.iter().map(|v| v.x).collect::<Vec<f64>>(),
                self.d.iter().map(|v| v.y).collect::<Vec<f64>>(),
                self.d.iter().map(|v| v.z).collect::<Vec<f64>>(),
            ];
            for k in 0..3 {
                let mut source: Vec<f64> = src
                    .iter()
                    .map(|v| match k {
                        0 => v.x,
                        1 => v.y,
                        _ => v.z,
                    })
                    .collect();
                for (pi, p) in mesh.patches.iter().enumerate() {
                    if let DisplacementPatch::FixedValue(v) = self.d_patches[pi] {
                        let val = [v.x, v.y, v.z][k];
                        for fi in 0..p.size {
                            let c = mesh.owner[p.start + fi];
                            source[c] += self.boundary_coeff(self.coeffs.two_mu_lambda, pi, fi, val);
                        }
                    }
                }
                let (i, f, n) = pcg_dic(&m, &mesh.owner, &mesh.neighbour, &source, &mut comps[k], self.controls.d_solver);
                max_initial = max_initial.max(i);
                log.solves.push(SolveLog { field: names[k], initial_residual: i, final_residual: f, n_iterations: n });
            }
            for c in 0..mesh.n_cells {
                self.d[c] = Vector3::new(comps[0][c], comps[1][c], comps[2][c]);
            }
            self.correct_boundary_conditions();
            // sigmaD and the compact-normal-stress divSigmaExp
            let gd = self.grad_d();
            let (sc, sb) = self.sigma_d_fields(&gd);
            let gamma = self.coeffs.two_mu_lambda;
            let cells: Vec<Tensor> = (0..mesh.n_cells).map(|c| sigma_minus_gamma_grad(&sc[c], &gd.internal[c], gamma)).collect();
            let bnd: Vec<Vec<Tensor>> = gd
                .boundary
                .iter()
                .enumerate()
                .map(|(pi, pf)| (0..pf.values.len()).map(|fi| sigma_minus_gamma_grad(&sb[pi][fi], &pf.values[fi], gamma)).collect())
                .collect();
            let tf = tensor_field(&mesh, "sigmaD-gammagradD", cells, bnd);
            self.div_sigma_exp = self.div_of_tensor_field(&tf);
            self.sigma_d = sc;
            i_corr += 1;
            if !(max_initial > self.controls.convergence_tolerance && i_corr < self.controls.n_correctors) {
                break;
            }
        }
        log
    }

    /// `calculateStress.H`: `sigma = rho*sigmaD` (minus `I*rho*threeKalpha*T` when
    /// thermal), one symmetric tensor per cell (Pa).
    pub fn sigma(&self) -> Vec<SymmTensor> {
        let rho = self.props.rho;
        (0..self.mesh.n_cells)
            .map(|c| {
                let s = self.sigma_d[c];
                let mut o = SymmTensor {
                    xx: rho * s.xx,
                    xy: rho * s.xy,
                    xz: rho * s.xz,
                    yy: rho * s.yy,
                    yz: rho * s.yz,
                    zz: rho * s.zz,
                };
                if self.props.thermal.is_some() {
                    let ii = rho * self.coeffs.three_k_alpha * self.t[c];
                    o.xx -= ii;
                    o.yy -= ii;
                    o.zz -= ii;
                }
                o
            })
            .collect()
    }

    /// `sigmaEq = sqrt((3.0/2.0)*magSqr(dev(sigma)))` per cell (Pa).
    pub fn sigma_eq(&self) -> Vec<f64> {
        self.sigma()
            .iter()
            .map(|s| {
                let sph = (1.0 / 3.0) * (s.xx + s.yy + s.zz);
                let d = SymmTensor { xx: s.xx - sph, xy: s.xy, xz: s.xz, yy: s.yy - sph, yz: s.yz, zz: s.zz - sph };
                let mag_sqr = d.xx * d.xx + 2.0 * (d.xy * d.xy) + 2.0 * (d.xz * d.xz) + d.yy * d.yy + 2.0 * (d.yz * d.yz) + d.zz * d.zz;
                ((3.0 / 2.0) * mag_sqr).sqrt()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
