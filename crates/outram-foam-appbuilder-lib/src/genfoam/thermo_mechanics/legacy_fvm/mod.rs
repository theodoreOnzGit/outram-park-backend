// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/thermoMechanics.{C,H},
//                    src/classes/thermoMechanics/legacyThermoMechanics/legacyThermoMechanics.{C,H},
//                    legacyThermoMechanics/include/solveThermalMechanics.H, include/calculateStress.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0 (the sources say GPL-2.0-or-later)
//   The displacement/stress algorithm is derived from OpenFOAM's solidDisplacementFoam,
//   (C) 2011 OpenFOAM Foundation; the operators follow ESI OpenFOAM v2506
//   (finiteVolume: gaussLaplacianSchemes.C, correctedSnGrad.C, leastSquaresGrad.C, fvMatrix.C,
//   EulerDdtScheme.C, gaussConvectionScheme.C; OpenFOAM: lduMatrixOperations.C).
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate root or
// <https://www.gnu.org/licenses/> for the full license text.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation, nor OpenCFD Limited,
// producer and distributor of the OpenFOAM(R) software.

//! # `LegacyFvmThermoMechanics`: GeN-Foam's `legacyThermoMechanics`, finite-volume form (gh:#1781, round 23)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! A line-by-line translation of `solvers::legacyThermoMechanics::correctPhysics()`
//! (`legacyThermoMechanics/include/solveThermalMechanics.H` and `calculateStress.H`) with the base class
//! `thermoMechanics`'s axial fuel / control-rod expansion fields, in upstream's operation order, so that it can
//! be compared bit for bit with the real class (`verification_and_validation/genfoam_vs_upstream_gh1361/README.md`,
//! "Round 23").
//!
//! ## What one call of [`LegacyFvmThermoMechanics::correct_physics`] does (upstream order)
//!
//! 1. `TEqn` (if `thermalSolver`): `isPorous = TStructFromTH/max(TStructFromTH, SMALL)`;
//!    `(1 - isPorous)*(fvm::ddt(rho C, T) - fvm::laplacian(rhoK, T) - powerDensityNeutronics) ==
//!    fvm::Sp(isPorous, T) - TStructFromTH*isPorous`, solved asymmetrically (the row scaling breaks the
//!    symmetry; see [`pbicg`]).
//! 2. The displacement loop (if `mechanicsSolver`):
//!    `DEqn: fvm::d2dt2(D) == fvm::laplacian(2 mu + lambda, D, "laplacian(DD,D)") + divSigmaExp`,
//!    `DEqn += fvc::grad(threeKalpha (T - TRef))`, solved component by component, then
//!    `gradD`, `sigmaD = mu twoSymm(gradD) + lambda I tr(gradD)`, and the explicit term for the next corrector:
//!    `compactNormalStress yes`: `divSigmaExp = fvc::div(sigmaD - (2 mu + lambda) gradD, "div(sigmaD)")`;
//!    `compactNormalStress no` (gh:#1845, round 27): `divSigmaExp = fvc::div(DEqn.flux())` straight after the solve and
//!    `divSigmaExp += fvc::div(sigmaD)` after `gradD` (see [`LegacyFvmThermoMechanics::flux_divergence`]);
//!    repeated while `initialResidual > D && ++iCorr < nCorrectors`. `d2dt2` is `steadyState` (an empty matrix) or,
//!    since round 27, `Euler` ([`D2dt2Scheme`], [`LegacyFvmThermoMechanics::with_d2dt2`]).
//! 3. `calculateStress.H` (`sigma`, `sigmaEq`, the logged maximum), then the axial expansions
//!    `fvm::div(fuelOrientation & Sf, fuelDisp, "div(fuelDisp)") == alphaFuel (TFuel - TFuelRef)` and
//!    `fvm::div(-fuelOrientation & Sf, CRDisp, "div(CRDisp)") == -alphaCR (TStruct - TCRRef)`, and
//!    `meshDisp = (disp - (disp & n) n) + (fuelDisp + CRDisp) n`: the expansion feedback the other regions read.
//!
//! ## Schemes (selectable, refused by name otherwise)
//!
//! `gradSchemes`: `Gauss linear` or `leastSquares` (the shipped ESFR case: `leastSquares`).
//! `laplacianSchemes`: `Gauss linear corrected` (the shipped case; non-orthogonal correction, explicit, from the
//! selected gradient of each component), `uncorrected` or `orthogonal`. `Gauss upwind` for `div(fuelDisp)` and
//! `div(CRDisp)`, `Gauss linear` for `div(sigmaD)`, `Euler` `ddt`, `steadyState` `d2dt2`.
//!
//! ## Deliberate omissions, each refused by name (children of gh:#1781)
//!
//! ~~`compactNormalStress no` (needs `fvMatrix::flux()`), transient `d2dt2`~~ **CORRECTED 2026-11-26 (gh:#1845, round 27)**:
//! both ported (`d2dt2` `Euler`; ESI v2506 has no other transient `d2dt2` scheme, so `backward` is refused by name in
//! [`case::LegacyFvmCase`]), ~~the `GFTractionDisplacement` and
//! `gapContact` patch types~~ **CORRECTED 2026-11-26 (gh:#1235)**: `GFTractionDisplacement` is ported ([`traction`], default form of
//! `disp` patches, bit-identical to the real class); `gapContact` (needs the `regionCoupled` coupling, gh:#1846) is refused by
//! name in [`case::LegacyFvmCase`] and only its arithmetic is ported; `Gauss linear` gradients with a traction patch,
//! `meshDisp` -> `movePoints` (gh:#1784), ESI's `tractionDisplacement`, non-zeroGradient
//! `TStruct` patches, `GAMG` (named substitute below), `planeStress` needs nothing (a flag).
//!
//! ## `GAMG` is substituted, by name
//!
//! The shipped `TStruct` entry is `GAMG`, and the port has no faithful `GAMG` (README round 21). OpenFOAM refuses
//! `PCG` for the asymmetric `TStruct` matrix, so the substitute is `PBiCG`/`DILU` ([`TStructSolver::GamgSubstitutedByPbicgDilu`]),
//! at the entry's own tolerances. The effect is measured in the README.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolTensorField, VolVectorField};
use outram_foam_basic_lib::fv_operators::fvc::{
    grad, grad_least_squares, grad_vec_corrected, interpolate, linear_weight, sn_grad_correction_with_grad,
};
use outram_foam_basic_lib::fv_operators::fvm::{DeltaCoeff, NonOrthoScheme};
use outram_foam_basic_lib::fv_operators::les_filter::surface_integrate;
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::mesh::fv_mesh::PatchKind;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SymmTensor, Tensor, Vector3};

use crate::error::AppBuilderError;
use crate::genfoam::thermal_hydraulics::solver::two_phase::pcg::pcg_dic;
use crate::io::fv_schemes::GradScheme;
use crate::solvers::non_orthogonal::LaplacianCorrection;

pub mod pbicg;
use pbicg::pbicg_dilu;
pub mod traction;
use traction::{base_sn_grad, evaluate_value, GfTractionDisplacement, TractionInputs};

#[cfg(feature = "std")]
pub mod case;

mod external_displacement;
mod written;

#[cfg(test)]
mod tests;

/// `SMALL` for double precision (OpenFOAM `doubleScalarSmall`).
const SMALL: f64 = 1.0e-15;

/// One entry of `thermoMechanicalProperties.materials`: a material card for a cell zone (upstream reads each as
/// `dict.getOrDefault<dimensionedScalar>(key, default)`; the defaults below are upstream's).
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneCard {
    /// Zone name (a `cellZones` entry).
    pub name: String,
    /// `rho`, kg/m^3 (default 1).
    pub rho: f64,
    /// `Cp`, J/kg/K (default 1).
    pub cp: f64,
    /// `k`, W/m/K (default 1); upstream's field `rhoK`.
    pub k: f64,
    /// `E`, Pa (default 1); upstream's field `rhoE`.
    pub e: f64,
    /// `nu` (default 1).
    pub nu: f64,
    /// `alpha`, 1/K (default 1).
    pub alpha: f64,
    /// `Tref`, K (default 0).
    pub t_ref: f64,
    /// `TFuelRef`, K (default 0).
    pub t_fuel_ref: f64,
    /// `alphaFuel`, 1/K (default 0).
    pub alpha_fuel: f64,
    /// `TCRRef`, K (default 0).
    pub t_cr_ref: f64,
    /// `alphaCR`, 1/K (default 0).
    pub alpha_cr: f64,
}

impl ZoneCard {
    /// A card with upstream's defaults for every key.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            rho: 1.0,
            cp: 1.0,
            k: 1.0,
            e: 1.0,
            nu: 1.0,
            alpha: 1.0,
            t_ref: 0.0,
            t_fuel_ref: 0.0,
            alpha_fuel: 0.0,
            t_cr_ref: 0.0,
            alpha_cr: 0.0,
        }
    }
}

/// Boundary condition of the displacement `disp` on one patch.
///
/// `gapContact` (gh:#1235) is NOT here: it needs the `regionCoupled` two-region coupling (gh:#1846); its arithmetic is
/// [`traction::GapContact`]. The matches over this enum are in this module only.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DispPatch {
    /// `fixedValue`, uniform (m).
    FixedValue(Vector3),
    /// `zeroGradient`.
    ZeroGradient,
    /// GeN-Foam's `GFTractionDisplacement` (a `fixedGradient` patch with a recomputed gradient), see [`traction`].
    GfTractionDisplacement(GfTractionDisplacement),
}

/// Boundary condition of a scalar field (`fuelDisp`, `CRDisp`) on one patch.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ScalarPatch {
    /// `fixedValue`, uniform.
    FixedValue(f64),
    /// `zeroGradient`.
    ZeroGradient,
}

/// `solvers` entry of a linear solve (`tolerance`, `relTol`, `maxIter`, `minIter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearSolverSettings {
    /// Absolute tolerance.
    pub tolerance: f64,
    /// Relative tolerance (0 disables).
    pub rel_tol: f64,
    /// `maxIter` (upstream default 1000).
    pub max_iter: usize,
    /// `minIter` (upstream default 0).
    pub min_iter: usize,
}

impl LinearSolverSettings {
    /// The tight protocol of the differential tests: `tolerance`, `relTol 0`.
    pub fn tight(tolerance: f64) -> Self {
        Self { tolerance, rel_tol: 0.0, max_iter: 1000, min_iter: 0 }
    }
}

/// Which solver runs the asymmetric `TStruct` equation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStructSolver {
    /// The case selects `PBiCG` + `DILU` itself: upstream's own form.
    PbicgDilu,
    /// The case selects `GAMG`; the port has no faithful `GAMG` and runs `PBiCG`/`DILU` at the entry's tolerances. A
    /// NAMED deviation, reported by [`LegacyFvmThermoMechanics::t_struct_gamg_substituted`].
    GamgSubstitutedByPbicgDilu,
}

/// `fvSchemes.d2dt2Schemes.default` (gh:#1845, round 27). ESI v2506 has exactly these two (`d2dt2Schemes/`:
/// `steadyStateD2dt2Scheme`, `EulerD2dt2Scheme`); there is NO `backward` d2dt2 scheme to port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum D2dt2Scheme {
    /// `steadyStateD2dt2Scheme`: `fvm::d2dt2` is an empty matrix and never touches `oldTime()`.
    #[default]
    SteadyState,
    /// `EulerD2dt2Scheme` (non-moving mesh): `coefft = (dt+dt0)/(2 dt)`, `coefft00 = (dt+dt0)/(2 dt0)`,
    /// `diag = (coefft rDeltaT2) V`, `source = rDeltaT2 V (coefft0 old - coefft00 oldold)`.
    Euler,
}

/// The `fvSchemes`, `fvSolution` and `thermoMechanicalProperties` switches `legacyThermoMechanics` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyFvmSettings {
    /// `rheologyOption.planeStress`.
    pub plane_stress: bool,
    /// `mechanicsSolver` (default on).
    pub solve_displacement: bool,
    /// `thermalSolver` (default on).
    pub solve_thermo: bool,
    /// `globalOptions.pinDirection` (the fuel orientation; also the axis of the expansion fields).
    pub pin_direction: Vector3,
    /// `stressAnalysis.nCorrectors` (default 1).
    pub n_correctors: usize,
    /// `stressAnalysis.D`: the convergence tolerance on the maximum initial residual.
    pub convergence_tolerance: f64,
    /// `stressAnalysis.compactNormalStress`: `true` is the compact form, `false` the `fvMatrix::flux` form (round 27).
    pub compact_normal_stress: bool,
    /// `gradSchemes` for `grad(disp)`, `grad(TStruct)` and the gradients inside the Laplacian corrections.
    pub grad: GradScheme,
    /// `laplacianSchemes` for `laplacian(DD,D)` and `laplacian(rhoK,TStruct)`.
    pub laplacian: LaplacianCorrection,
    /// `solvers.disp` (`PCG`/`DIC`).
    pub d_solver: LinearSolverSettings,
    /// `solvers.fuelDisp` (`PBiCG`/`DILU`).
    pub fuel_solver: LinearSolverSettings,
    /// `solvers.CRDisp` (`PBiCG`/`DILU`).
    pub cr_solver: LinearSolverSettings,
    /// `solvers.TStruct`.
    pub t_solver: LinearSolverSettings,
    /// Which solver `TStruct` runs.
    pub t_solver_kind: TStructSolver,
    /// `false` (the default, and upstream's behaviour): the `GFTractionDisplacement` patches are plane strain whatever
    /// `planeStress` says, because upstream's lookup of the flag is misspelled (see [`traction`]). `true` is a named
    /// CONTROL (the intended behaviour), never upstream's.
    pub traction_bc_honours_plane_stress: bool,
}

/// One linear solve, as upstream logs it (`DICPCG: Solving for dispx, Initial residual = ...`).
#[derive(Debug, Clone, PartialEq)]
pub struct SolveRecord {
    /// Field (and component) name, as upstream prints it: `TStruct`, `dispx`, `dispy`, `dispz`, `fuelDisp`, `CRDisp`.
    pub field: &'static str,
    /// Normalised initial residual.
    pub initial_residual: f64,
    /// Normalised final residual.
    pub final_residual: f64,
    /// Iterations.
    pub n_iterations: usize,
}

/// Everything one `correctPhysics()` did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhysicsLog {
    /// Linear solves in upstream's order.
    pub solves: Vec<SolveRecord>,
    /// The `residual_` of upstream (`max` of the `TStruct` and displacement initial residuals).
    pub residual: f64,
    /// `Max sigmaEq` as `calculateStress.H` prints it (cells and boundary faces), when computed.
    pub max_sigma_eq: Option<f64>,
    /// Displacement correctors executed.
    pub n_correctors_run: usize,
}

/// Precomputed face geometry the operators reuse.
#[derive(Debug, Clone)]
struct FaceGeometry {
    mag_sf: Vec<f64>,
    /// `deltaCoeffs` (internal) of the selected laplacian scheme's `snGradScheme`.
    dc_internal: Vec<f64>,
    /// `fvPatch::deltaCoeffs()` per boundary face (`1/mag(patch delta)`), used by every patch coefficient.
    dc_boundary: Vec<Vec<f64>>,
}

impl FaceGeometry {
    fn new(mesh: &FvMesh, lap: LaplacianCorrection) -> Self {
        let mag_sf = (0..mesh.n_faces).map(|f| mesh.face_area_vectors[f].mag()).collect();
        let mode = lap.delta_coeff();
        let dc_internal = (0..mesh.n_internal_faces).map(|f| mode.interior_delta_coeff(mesh, f)).collect();
        let dc_boundary = mesh
            .patches
            .iter()
            .map(|p| (0..p.size).map(|fi| DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, p.start + fi)).collect())
            .collect();
        Self { mag_sf, dc_internal, dc_boundary }
    }
}

/// `scalar*vector` as `Foam` evaluates it.
fn vscale(s: f64, v: Vector3) -> Vector3 {
    Vector3::new(s * v.x, s * v.y, s * v.z)
}

fn cmpt(v: Vector3, k: usize) -> f64 {
    match k {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// `mu*twoSymm(g) + (lambda*I)*tr(g)` (`sigmaD`).
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

/// `gamma*gradD`, a `scalar*tensor` (each component multiplied).
fn gamma_times_grad(g: &Tensor, gamma: f64) -> Tensor {
    Tensor::new(gamma * g.xx, gamma * g.xy, gamma * g.xz, gamma * g.yx, gamma * g.yy, gamma * g.yz, gamma * g.zx, gamma * g.zy, gamma * g.zz)
}

/// `sigmaD - gamma*gradD`, a `symmTensor - tensor`.
fn sigma_minus_gamma_grad(s: &SymmTensor, g: &Tensor, gamma: f64) -> Tensor {
    Tensor::new(
        s.xx - gamma * g.xx,
        s.xy - gamma * g.xy,
        s.xz - gamma * g.xz,
        s.xy - gamma * g.yx,
        s.yy - gamma * g.yy,
        s.yz - gamma * g.yz,
        s.xz - gamma * g.zx,
        s.yz - gamma * g.zy,
        s.zz - gamma * g.zz,
    )
}

fn calculated_scalar(mesh: &Arc<FvMesh>, name: &str, cells: Vec<f64>, boundary: Vec<Vec<f64>>) -> VolScalarField {
    let bnd = boundary
        .into_iter()
        .map(|v| {
            let f = Field::new(v);
            PatchField { bc: BoundaryCondition::Calculated(f.clone()), values: f }
        })
        .collect();
    VolScalarField::new(name, mesh.clone(), Field::new(cells), bnd)
}

fn calculated_tensor(mesh: &Arc<FvMesh>, name: &str, cells: Vec<Tensor>, boundary: Vec<Vec<Tensor>>) -> VolTensorField {
    let bnd = boundary
        .into_iter()
        .map(|v| {
            let f = Field::new(v);
            PatchField { bc: BoundaryCondition::Calculated(f.clone()), values: f }
        })
        .collect();
    VolTensorField::new(name, mesh.clone(), Field::new(cells), bnd)
}

/// A cell-valued property field: the boundary value of a `zeroGradient` `volScalarField` after
/// `correctBoundaryConditions()` is the owner-cell value.
fn owner_boundary(mesh: &FvMesh, cells: &[f64]) -> Vec<Vec<f64>> {
    mesh.patches.iter().map(|p| (0..p.size).map(|fi| cells[mesh.owner[p.start + fi]]).collect()).collect()
}

/// The `fvc::div(vf, "div(sigmaD)")` of a tensor field with `Gauss linear`: `surfaceIntegrate(Sf & interpolate(vf))`.
fn div_tensor_field(mesh: &FvMesh, t: &VolTensorField) -> Vec<Vector3> {
    let tf = interpolate(t);
    let internal: Vec<Vector3> =
        (0..mesh.n_internal_faces).map(|f| Tensor::vec_mat(mesh.face_area_vectors[f], tf.internal[f])).collect();
    let boundary: Vec<Vec<Vector3>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size).map(|fi| Tensor::vec_mat(mesh.face_area_vectors[p.start + fi], tf.boundary[pi].values[fi])).collect()
        })
        .collect();
    surface_integrate(mesh, &internal, &boundary)
}

/// `gaussLaplacianScheme::fvmLaplacianUncorrected` for a symmetric system: the pieces of `B`.
#[derive(Debug, Clone)]
struct LaplacianParts {
    /// `deltaCoeffs*gammaMagSf` on the internal faces (`B.upper() = B.lower()`).
    upper: Vec<f64>,
    /// `B.diag()` after `negSumDiag()`.
    diag: Vec<f64>,
    /// `gammaMagSf` on boundary faces (`gamma_b*magSf_b`).
    gamma_mag_sf_boundary: Vec<Vec<f64>>,
    /// `gammaMagSf` on internal faces.
    gamma_mag_sf_internal: Vec<f64>,
}

/// `legacyThermoMechanics` on a finite-volume mesh; see the module documentation.
///
/// This type is what the later finite-element version (gh:#1782) is selected beside: the caller sees
/// [`Self::correct_physics`], the coupling inputs ([`Self::set_coupling_fields`]) and the feedback outputs
/// ([`Self::mesh_displacement`], [`Self::fuel_displacement`], [`Self::control_rod_displacement`]).
#[derive(Debug, Clone)]
pub struct LegacyFvmThermoMechanics {
    mesh: Arc<FvMesh>,
    settings: LegacyFvmSettings,
    geo: FaceGeometry,
    delta_t: f64,
    /// `Time::deltaT0()`: the previous step's `deltaT` (the first step's is the initial `deltaT`).
    delta_t0: f64,
    /// `Time::deltaTSave_`: what the next step's `deltaT0` will be.
    delta_t_saved: f64,
    d2dt2: D2dt2Scheme,
    /// `disp.oldTime()` and `disp.oldTime().oldTime()`, once an `Euler` `fvm::d2dt2` has created them.
    disp_old: Vec<Vector3>,
    disp_old_old: Vec<Vector3>,
    old_times_exist: bool,
    gamg_substituted: bool,
    /// `TStruct` boundary conditions, one per patch (`zeroGradient` unless the case says `fixedValue`).
    t_patches: Vec<ScalarPatch>,
    /// `polyMesh::solutionD()`: `false` for the direction normal to the `empty` patches (2-D case): that displacement component is
    /// not solved (`fvMatrix::solveSegregated` skips it) and not logged.
    solution_d: [bool; 3],
    // per-cell material fields (`zeroGradient` volScalarFields)
    rho: Vec<f64>,
    rho_e: Vec<f64>,
    nu: Vec<f64>,
    c: Vec<f64>,
    rho_k: Vec<f64>,
    t_struct_ref: Vec<f64>,
    alpha_fuel: Vec<f64>,
    t_fuel_ref: Vec<f64>,
    t_cr_ref: Vec<f64>,
    alpha_cr: Vec<f64>,
    mu: Vec<f64>,
    lambda: Vec<f64>,
    two_mu_lambda: Vec<f64>,
    three_k_alpha: Vec<f64>,
    /// `alpha_` of `legacyThermoMechanics` (the card's `alpha`, per cell; written as `alpha`, gh:#1935).
    alpha: Vec<f64>,
    // coupling inputs
    t_struct_from_th: Vec<f64>,
    t_fuel: Vec<f64>,
    power_density: Vec<f64>,
    // solution fields
    disp_patches: Vec<DispPatch>,
    fuel_patches: Vec<ScalarPatch>,
    cr_patches: Vec<ScalarPatch>,
    disp: Vec<Vector3>,
    disp_b: Vec<Vec<Vector3>>,
    t_struct: Vec<f64>,
    is_porous: Vec<f64>,
    fuel_disp: Vec<f64>,
    cr_disp: Vec<f64>,
    mesh_disp: Vec<Vector3>,
    sigma_d: Vec<SymmTensor>,
    sigma_d_b: Vec<Vec<SymmTensor>>,
    div_sigma_exp: Vec<Vector3>,
    residual: f64,
}

impl LegacyFvmThermoMechanics {
    /// Construction (`legacyThermoMechanics::legacyThermoMechanics`, `thermoMechanics::thermoMechanics`).
    ///
    /// `zones` are the `cellZones` of the mesh as `(name, cells)`; `cards` the `materials` entries in dictionary
    /// order (a cell in two zones takes the later card, as upstream's loop does). `t_struct0` is the initial
    /// `TStruct` per cell (`READ_IF_PRESENT`; upstream's default is 0).
    ///
    /// # Errors
    /// [`AppBuilderError::UnsupportedScheme`] for a `Limited` laplacian, `fourth` or `cellLimited` gradient; [`AppBuilderError::Case`] for a card without a `cellZone`, a patch list that does
    /// not match the mesh, or an `empty` patch.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mesh: Arc<FvMesh>,
        settings: LegacyFvmSettings,
        zones: &[(String, Vec<usize>)],
        cards: &[ZoneCard],
        disp_patches: Vec<DispPatch>,
        fuel_patches: Vec<ScalarPatch>,
        cr_patches: Vec<ScalarPatch>,
        t_struct0: Vec<f64>,
        delta_t: f64,
    ) -> Result<Self, AppBuilderError> {
        match settings.grad {
            GradScheme::GaussLinear | GradScheme::LeastSquares => {}
            _ => {
                return Err(AppBuilderError::UnsupportedScheme {
                    family: "gradSchemes",
                    scheme: "cellLimited / fourth".to_string(),
                    reason: "legacyThermoMechanics is ported with Gauss linear and leastSquares gradients",
                })
            }
        }
        if let LaplacianCorrection::Limited(_) = settings.laplacian {
            return Err(AppBuilderError::UnsupportedScheme {
                family: "laplacianSchemes",
                scheme: "limited corrected".to_string(),
                reason: "not ported for the displacement/temperature Laplacians",
            });
        }
        let np = mesh.patches.len();
        let n = mesh.n_cells;
        let has_traction = disp_patches.iter().any(|b| matches!(b, DispPatch::GfTractionDisplacement(_)));
        if has_traction && settings.grad == GradScheme::GaussLinear {
            return Err(AppBuilderError::UnsupportedScheme {
                family: "gradSchemes",
                scheme: "Gauss linear with GFTractionDisplacement".to_string(),
                reason: "grad(disp)'s boundary correction needs the patch snGrad() (= gradient_, exactly); the shared Gauss route computes deltaCoeffs*(b - P); use leastSquares",
            });
        }
        if disp_patches.iter().zip(mesh.patches.iter()).any(|(b, p)| match b {
            DispPatch::GfTractionDisplacement(t) => t.len() != p.size,
            _ => false,
        }) {
            return Err(AppBuilderError::Case {
                case: crate::path::PathBuf::new(),
                msg: "GFTractionDisplacement: traction and pressure need one entry per patch face".to_string(),
            });
        }
        if disp_patches.len() != np
            || fuel_patches.len() != np
            || cr_patches.len() != np
            || t_struct0.len() != n
            || mesh.patches.iter().any(|p| p.kind == PatchKind::Empty)
        {
            return Err(AppBuilderError::Case {
                case: crate::path::PathBuf::new(),
                msg: "legacyThermoMechanics: one patch condition per patch for disp, fuelDisp and CRDisp, one TStruct per cell, no empty patches"
                    .to_string(),
            });
        }
        // `volScalarField(..., dimensionedScalar(default), zeroGradient)` defaults of the constructor
        let mut rho = vec![1.0; n];
        let mut rho_e = vec![1.0; n];
        let mut nu = vec![0.0; n];
        let mut c = vec![1.0; n];
        let mut rho_k = vec![0.0; n];
        let mut alpha = vec![1.0e-5; n];
        let mut t_struct_ref = vec![0.0; n];
        let mut alpha_fuel = vec![0.0; n];
        let mut t_fuel_ref = vec![0.0; n];
        let mut t_cr_ref = vec![0.0; n];
        let mut alpha_cr = vec![0.0; n];
        for card in cards {
            let zone = zones
                .iter()
                .find(|z| z.0 == card.name)
                .ok_or_else(|| AppBuilderError::Case {
                    case: crate::path::PathBuf::new(),
                    msg: format!("thermoMechanicalProperties.materials: no cellZone `{}`", card.name),
                })?;
            for &cell in &zone.1 {
                t_struct_ref[cell] = card.t_ref;
                rho[cell] = card.rho;
                rho_e[cell] = card.e;
                nu[cell] = card.nu;
                c[cell] = card.cp;
                rho_k[cell] = card.k;
                alpha[cell] = card.alpha;
                t_fuel_ref[cell] = card.t_fuel_ref;
                alpha_fuel[cell] = card.alpha_fuel;
                t_cr_ref[cell] = card.t_cr_ref;
                alpha_cr[cell] = card.alpha_cr;
            }
        }
        // E_ = rhoE_/rho_; mu_ = E_/(2.0*(1.0 + nu_)); lambda_ = nu_*E_/((1.0 + nu_)*(1.0 - 2.0*nu_));
        // threeK_ = E_/(1.0 - 2.0*nu_); plane stress: lambda_ = nu_*E_/((1.0 + nu_)*(1.0 - nu_)), threeK_ = E_/(1.0 - nu_)
        let mut mu = vec![0.0; n];
        let mut lambda = vec![0.0; n];
        let mut three_k_alpha = vec![0.0; n];
        for i in 0..n {
            let e = rho_e[i] / rho[i];
            mu[i] = e / (2.0 * (1.0 + nu[i]));
            let (l, tk) = if settings.plane_stress {
                (nu[i] * e / ((1.0 + nu[i]) * (1.0 - nu[i])), e / (1.0 - nu[i]))
            } else {
                (nu[i] * e / ((1.0 + nu[i]) * (1.0 - 2.0 * nu[i])), e / (1.0 - 2.0 * nu[i]))
            };
            lambda[i] = l;
            three_k_alpha[i] = tk * alpha[i];
        }
        let two_mu_lambda: Vec<f64> = (0..n).map(|i| 2.0 * mu[i] + lambda[i]).collect();
        let geo = FaceGeometry::new(&mesh, settings.laplacian);
        let disp_b: Vec<Vec<Vector3>> = mesh
            .patches
            .iter()
            .zip(&disp_patches)
            .map(|(p, bc)| match bc {
                DispPatch::FixedValue(v) => vec![*v; p.size],
                // `fvPatchVectorField::operator=(patchInternalField())` of the constructor, `disp` starting at 0
                DispPatch::ZeroGradient | DispPatch::GfTractionDisplacement(_) => vec![Vector3::ZERO; p.size],
            })
            .collect();
        let gamg_substituted = settings.t_solver_kind == TStructSolver::GamgSubstitutedByPbicgDilu;
        let mut s = Self {
            geo,
            delta_t,
            delta_t0: delta_t,
            delta_t_saved: delta_t,
            d2dt2: D2dt2Scheme::SteadyState,
            disp_old: Vec::new(),
            disp_old_old: Vec::new(),
            old_times_exist: false,
            gamg_substituted,
            solution_d: [true; 3],
            t_patches: vec![ScalarPatch::ZeroGradient; np],
            rho,
            rho_e,
            nu,
            c,
            rho_k,
            t_struct_ref,
            alpha_fuel,
            t_fuel_ref,
            t_cr_ref,
            alpha_cr,
            mu,
            lambda,
            two_mu_lambda,
            three_k_alpha,
            alpha,
            // TStructFromTH_ and TFuel_ are constructed as uniform 1.0, powerDensityNeutronics_ as 0
            t_struct_from_th: vec![1.0; n],
            t_fuel: vec![1.0; n],
            power_density: vec![0.0; n],
            disp_patches,
            fuel_patches,
            cr_patches,
            disp: vec![Vector3::ZERO; n],
            disp_b,
            t_struct: t_struct0,
            is_porous: vec![0.0; n],
            fuel_disp: vec![0.0; n],
            cr_disp: vec![0.0; n],
            mesh_disp: vec![Vector3::ZERO; n],
            sigma_d: vec![SymmTensor::ZERO; n],
            sigma_d_b: Vec::new(),
            div_sigma_exp: vec![Vector3::ZERO; n],
            residual: 0.0,
            settings,
            mesh,
        };
        s.extrapolate_disp_boundary();
        s.initial_div_sigma_exp();
        Ok(s)
    }

    /// Whether the `TStruct` equation runs the named `GAMG` substitute (`PBiCG`/`DILU`).
    pub fn solution_directions(&self) -> [bool; 3] {
        self.solution_d
    }

    /// The `TStruct` patch conditions (README round 57): `fixedValue` patches enter the temperature equation's boundary
    /// coefficients, the face values of every gradient of `TStruct`, and the boundary `sigma`.
    ///
    /// # Panics
    /// If the list is not one entry per patch.
    #[must_use]
    pub fn with_t_struct_patches(mut self, patches: Vec<ScalarPatch>) -> Self {
        assert_eq!(patches.len(), self.mesh.patches.len(), "one TStruct patch condition per patch");
        self.t_patches = patches;
        self
    }

    /// `TStruct` on the boundary faces: the fixed value, or the owner-cell value (`zeroGradient`).
    fn t_struct_boundary(&self) -> Vec<Vec<f64>> {
        let mesh = &self.mesh;
        mesh.patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| match self.t_patches[pi] {
                        ScalarPatch::FixedValue(v) => v,
                        ScalarPatch::ZeroGradient => self.t_struct[mesh.owner[p.start + fi]],
                    })
                    .collect()
            })
            .collect()
    }

    /// `polyMesh::solutionD()`: the directions that are solved (README round 57). The default is all three.
    #[must_use]
    pub fn with_solution_directions(mut self, solution_d: [bool; 3]) -> Self {
        self.solution_d = solution_d;
        self
    }

    /// Whether `TStruct`'s linear solve is the named `GAMG` substitute.
    pub fn t_struct_gamg_substituted(&self) -> bool {
        self.gamg_substituted
    }

    /// The boundary values of `disp`, per patch and face (for `GFTractionDisplacement`: `internal + gradient/deltaCoeffs`).
    pub fn displacement_boundary(&self) -> &[Vec<Vector3>] {
        &self.disp_b
    }

    /// The condition of patch `pi` of `disp` (for `GFTractionDisplacement` it carries the current gradient).
    pub fn disp_patch(&self, pi: usize) -> &DispPatch {
        &self.disp_patches[pi]
    }

    /// The displacement field, one vector per cell (m).
    pub fn displacement(&self) -> &[Vector3] {
        &self.disp
    }

    /// `TStruct` per cell (K).
    pub fn t_struct(&self) -> &[f64] {
        &self.t_struct
    }

    /// `fuelDisp` per cell (m): the axial fuel-expansion feedback.
    pub fn fuel_displacement(&self) -> &[f64] {
        &self.fuel_disp
    }

    /// `CRDisp` per cell (m): the axial control-rod-expansion feedback.
    pub fn control_rod_displacement(&self) -> &[f64] {
        &self.cr_disp
    }

    /// `meshDisp` per cell (m): the displacement the other regions' meshes are moved by (gh:#1784).
    pub fn mesh_displacement(&self) -> &[Vector3] {
        &self.mesh_disp
    }

    /// `sigmaD` per cell (the `rho`-normalised stress without the thermal part).
    pub fn sigma_d(&self) -> &[SymmTensor] {
        &self.sigma_d
    }

    /// `residual_` of the last [`Self::correct_physics`].
    pub fn residual(&self) -> f64 {
        self.residual
    }

    /// Set the three fields the other regions supply through `meshToMesh` in a coupled run: `TStructFromTH`,
    /// `TFuel` and `powerDensityNeutronics`, one value per cell (zeroGradient boundaries follow the owner cell).
    ///
    /// # Panics
    /// If a slice length differs from the number of cells.
    pub fn set_coupling_fields(&mut self, t_struct_from_th: &[f64], t_fuel: &[f64], power_density: &[f64]) {
        let n = self.mesh.n_cells;
        assert!(t_struct_from_th.len() == n && t_fuel.len() == n && power_density.len() == n, "one value per cell");
        self.t_struct_from_th.copy_from_slice(t_struct_from_th);
        self.t_fuel.copy_from_slice(t_fuel);
        self.power_density.copy_from_slice(power_density);
    }

    /// Set the step length `deltaT` of the `Euler` `ddt` of `TStruct`.
    pub fn set_delta_t(&mut self, delta_t: f64) {
        self.delta_t = delta_t;
    }

    /// Select the `d2dt2` scheme (gh:#1845, round 27); the default is [`D2dt2Scheme::SteadyState`], upstream's
    /// shipped selection. Builder style so that [`LegacyFvmSettings`] and its struct literals are untouched.
    #[must_use]
    pub fn with_d2dt2(mut self, scheme: D2dt2Scheme) -> Self {
        self.d2dt2 = scheme;
        self
    }

    /// The constructor of a run that STARTS at a written time (gh:#1926, README round 66): `disp` and `fuelDisp`, `CRDisp` (`MUST_READ`,
    /// `legacyThermoMechanics.C:259`, `thermoMechanics.C:98,110`) and `meshDisp` (`READ_IF_PRESENT`, `thermoMechanics.C:84`) are the READ
    /// fields, and the constructor tail (`legacyThermoMechanics.C:504-518`) computes `sigmaD_` and `divSigmaExp_` from the read `disp`; the
    /// `GFTractionDisplacement` patches were constructed from the dictionary (`gradient() = Zero`, the file's `value` not read back). This is
    /// what [`Self::restart_at`] reproduces on a solver that ran; here nothing ran, so nothing is reset.
    ///
    /// # Panics
    /// If a slice is not one value per cell.
    #[must_use]
    pub fn with_read_fields(mut self, disp: &[Vector3], fuel_disp: &[f64], cr_disp: &[f64], mesh_disp: &[Vector3]) -> Self {
        let n = self.mesh.n_cells;
        assert!(disp.len() == n && fuel_disp.len() == n && cr_disp.len() == n && mesh_disp.len() == n, "one value per cell");
        self.disp.copy_from_slice(disp);
        self.fuel_disp.copy_from_slice(fuel_disp);
        self.cr_disp.copy_from_slice(cr_disp);
        self.mesh_disp.copy_from_slice(mesh_disp);
        self.extrapolate_disp_boundary();
        self.initial_div_sigma_exp();
        self
    }

    /// The selected `d2dt2` scheme.
    pub fn d2dt2_scheme(&self) -> D2dt2Scheme {
        self.d2dt2
    }

    /// `Time::deltaT0()` as the next step will see it: the previous step's `deltaT` (initially the `deltaT` the solver
    /// was built with).
    pub fn delta_t0(&self) -> f64 {
        self.delta_t_saved
    }

    /// `Time::operator++`: `deltaT0 = deltaTSave; deltaTSave = deltaT`. Called once at the start of every
    /// [`Self::correct_physics`], with the current `deltaT` ([`Self::set_delta_t`] before it, as a run sets the step
    /// before `runTime++`).
    fn advance_time_levels(&mut self) {
        self.delta_t0 = self.delta_t_saved;
        self.delta_t_saved = self.delta_t;
    }

    /// `GeometricField::storeOldTimes()` for `disp`, as the first `fvm::d2dt2` of a time step triggers it.
    ///
    /// `vf.oldTime().oldTime()` is created lazily by the FIRST `Euler` `fvm::d2dt2` and both levels are then COPIES OF
    /// THE CURRENT FIELD, so at the first step `old = oldold = disp`; afterwards the levels shift once per time step
    /// (`oldold <- old <- disp`), NOT once per corrector (`timeIndex_` equals the time index from the second
    /// corrector on). The `steadyState` scheme never calls `oldTime()`, so nothing is stored for it.
    fn store_old_displacements(&mut self) {
        if self.d2dt2 != D2dt2Scheme::Euler {
            return;
        }
        if self.old_times_exist {
            self.disp_old_old = core::mem::take(&mut self.disp_old);
            self.disp_old = self.disp.clone();
        } else {
            self.disp_old = self.disp.clone();
            self.disp_old_old = self.disp.clone();
            self.old_times_exist = true;
        }
    }

    /// `fvm::d2dt2(disp)`: the diagonal and the vector source of the `d2dt2` matrix (`EulerD2dt2Scheme::fvmD2dt2`,
    /// `mesh().moving()` false: this mesh is static). For `steadyState` both are zero (an empty matrix; `0.0 - x` is then
    /// exactly what `A - B` of an empty `A` gives, so one code path serves both).
    fn d2dt2_terms(&self) -> (Vec<f64>, Vec<Vector3>) {
        let n = self.mesh.n_cells;
        if self.d2dt2 == D2dt2Scheme::SteadyState {
            return (vec![0.0; n], vec![Vector3::ZERO; n]);
        }
        let (dt, dt0) = (self.delta_t, self.delta_t0);
        // scalar coefft = (deltaT + deltaT0)/(2*deltaT); coefft00 = (deltaT + deltaT0)/(2*deltaT0); coefft0 = coefft + coefft00;
        // scalar rDeltaT2 = 4.0/sqr(deltaT + deltaT0);
        let coefft = (dt + dt0) / (2.0 * dt);
        let coefft00 = (dt + dt0) / (2.0 * dt0);
        let coefft0 = coefft + coefft00;
        let sum = dt + dt0;
        let r_delta_t2 = 4.0 / (sum * sum);
        let v = &self.mesh.cell_volumes;
        // fvm.diag() = (coefft*rDeltaT2)*mesh().V();
        let diag: Vec<f64> = (0..n).map(|c| (coefft * r_delta_t2) * v[c]).collect();
        // fvm.source() = rDeltaT2*mesh().V()*(coefft0*old - coefft00*oldold);
        let source: Vec<Vector3> = (0..n)
            .map(|c| {
                let (o, oo) = (self.disp_old[c], self.disp_old_old[c]);
                let rv = r_delta_t2 * v[c];
                Vector3::new(
                    rv * (coefft0 * o.x - coefft00 * oo.x),
                    rv * (coefft0 * o.y - coefft00 * oo.y),
                    rv * (coefft0 * o.z - coefft00 * oo.z),
                )
            })
            .collect();
        (diag, source)
    }

    // ---------------------------------------------------------------------------------------------------------
    // fields and gradients

    /// The constructor state: `zeroGradient` and `GFTractionDisplacement` patches hold the owner-cell value
    /// (`fvPatchVectorField::operator=(patchInternalField())`); NO `updateCoeffs()` has run, the gradient is zero.
    fn extrapolate_disp_boundary(&mut self) {
        for (pi, p) in self.mesh.patches.iter().enumerate() {
            if matches!(self.disp_patches[pi], DispPatch::ZeroGradient | DispPatch::GfTractionDisplacement(_)) {
                for fi in 0..p.size {
                    self.disp_b[pi][fi] = self.disp[self.mesh.owner[p.start + fi]];
                }
            }
        }
    }

    /// The registry lookups of `GFTractionDisplacementFvPatchVectorField::updateCoeffs()` for patch `pi`
    /// (boundary values of zeroGradient fields are the owner-cell values).
    fn traction_inputs(&self, pi: usize) -> TractionInputs {
        let mesh = &self.mesh;
        let p = &mesh.patches[pi];
        let owners: Vec<usize> = (0..p.size).map(|fi| mesh.owner[p.start + fi]).collect();
        let pick = |v: &[f64]| -> Vec<f64> { owners.iter().map(|&o| v[o]).collect() };
        TractionInputs {
            rho: pick(&self.rho),
            rho_e: pick(&self.rho_e),
            nu: pick(&self.nu),
            n: (0..p.size)
                .map(|fi| {
                    let sf = mesh.face_area_vectors[p.start + fi];
                    let mag = sf.mag();
                    Vector3::new(sf.x / mag, sf.y / mag, sf.z / mag)
                })
                .collect(),
            sn_grad: (0..p.size)
                .map(|fi| base_sn_grad(self.geo.dc_boundary[pi][fi], self.disp_b[pi][fi], self.disp[owners[fi]]))
                .collect(),
            sigma_d: self.sigma_d_b[pi].clone(),
            three_k_alpha: pick(&self.three_k_alpha),
            t_struct: pick(&self.t_struct),
            t_struct_ref: pick(&self.t_struct_ref),
            honour_plane_stress: self.settings.traction_bc_honours_plane_stress,
            plane_stress: self.settings.plane_stress,
        }
    }

    /// `disp_.boundaryFieldRef().updateCoeffs()` (what constructing an `fvMatrix` of `disp` does): every
    /// `GFTractionDisplacement` patch not yet updated recomputes its gradient.
    fn update_disp_coeffs(&mut self) {
        for pi in 0..self.disp_patches.len() {
            if matches!(self.disp_patches[pi], DispPatch::GfTractionDisplacement(_)) {
                let inputs = self.traction_inputs(pi);
                if let DispPatch::GfTractionDisplacement(t) = &mut self.disp_patches[pi] {
                    t.update_gradient(&inputs);
                }
            }
        }
    }

    /// `disp_.correctBoundaryConditions()`: `zeroGradient` patches take the owner-cell value; a
    /// `GFTractionDisplacement` patch `evaluate()`s: `updateCoeffs()` first if it has not run since the last
    /// `evaluate()`, then `value = internal + gradient/deltaCoeffs`, then `updated()` is cleared.
    fn correct_disp_boundary(&mut self) {
        for pi in 0..self.mesh.patches.len() {
            let (start, size) = (self.mesh.patches[pi].start, self.mesh.patches[pi].size);
            match &self.disp_patches[pi] {
                DispPatch::ZeroGradient => {
                    for fi in 0..size {
                        self.disp_b[pi][fi] = self.disp[self.mesh.owner[start + fi]];
                    }
                }
                DispPatch::GfTractionDisplacement(_) => {
                    let inputs = self.traction_inputs(pi);
                    if let DispPatch::GfTractionDisplacement(t) = &mut self.disp_patches[pi] {
                        t.update_gradient(&inputs);
                        for fi in 0..size {
                            self.disp_b[pi][fi] =
                                evaluate_value(self.disp[self.mesh.owner[start + fi]], t.gradient()[fi], self.geo.dc_boundary[pi][fi]);
                        }
                        t.clear_updated();
                    }
                }
                DispPatch::FixedValue(_) => {}
            }
        }
    }

    /// `snGrad()` of the `disp` patch field on face `fi` of patch `pi`.
    fn disp_patch_sn_grad(&self, pi: usize, fi: usize) -> Vector3 {
        let own = self.mesh.owner[self.mesh.patches[pi].start + fi];
        match &self.disp_patches[pi] {
            DispPatch::ZeroGradient => Vector3::ZERO,
            DispPatch::FixedValue(_) => vscale(self.geo.dc_boundary[pi][fi], self.disp_b[pi][fi] - self.disp[own]),
            DispPatch::GfTractionDisplacement(t) => t.gradient()[fi],
        }
    }

    fn disp_field(&self) -> VolVectorField {
        let bnd = self
            .mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, _)| PatchField {
                bc: match &self.disp_patches[pi] {
                    DispPatch::FixedValue(v) => BoundaryCondition::FixedValue(*v),
                    DispPatch::ZeroGradient => BoundaryCondition::ZeroGradient,
                    // refused with `Gauss linear` in `new`; `leastSquares` reads only the face values
                    DispPatch::GfTractionDisplacement(_) => BoundaryCondition::Calculated(Field::new(self.disp_b[pi].clone())),
                },
                values: Field::new(self.disp_b[pi].clone()),
            })
            .collect();
        VolVectorField::new("disp", self.mesh.clone(), Field::new(self.disp.clone()), bnd)
    }

    /// Component `k` of `disp` as a scalar field with its boundary values (`vf.component(cmpt)`).
    fn disp_component(&self, k: usize) -> VolScalarField {
        let cells: Vec<f64> = self.disp.iter().map(|v| cmpt(*v, k)).collect();
        let bnd: Vec<Vec<f64>> = self.disp_b.iter().map(|b| b.iter().map(|v| cmpt(*v, k)).collect()).collect();
        calculated_scalar(&self.mesh, "disp.component", cells, bnd)
    }

    /// `gradScheme<scalar>::grad` of the selected scheme (cell values).
    fn grad_scalar(&self, f: &VolScalarField) -> Vec<Vector3> {
        let g = match self.settings.grad {
            GradScheme::LeastSquares => grad_least_squares(f),
            _ => grad(f),
        };
        (0..self.mesh.n_cells).map(|c| g.internal[c]).collect()
    }

    /// `fvc::grad(disp_)`: cells from the selected scheme and, on boundary faces, `gaussGrad::correctBoundaryConditions`
    /// (the patch-normal part replaced by the patch `snGrad`).
    fn grad_disp(&self) -> VolTensorField {
        let field = self.disp_field();
        if self.settings.grad == GradScheme::GaussLinear {
            return grad_vec_corrected(&field);
        }
        // leastSquaresGrad<vector>::calcGrad: `lsGrad[own] += ownLs*deltaVsf` with `ownLs` a vector and `deltaVsf` a
        // vector is the outer product, the per-component arithmetic of the scalar gradient of each component
        let g: Vec<VolVectorField> = (0..3).map(|k| grad_least_squares(&self.disp_component(k))).collect();
        let mesh = &self.mesh;
        let cells: Vec<Tensor> = (0..mesh.n_cells)
            .map(|c| {
                let (a, b, d) = (g[0].internal[c], g[1].internal[c], g[2].internal[c]);
                Tensor::new(a.x, b.x, d.x, a.y, b.y, d.y, a.z, b.z, d.z)
            })
            .collect();
        let boundary: Vec<Vec<Tensor>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        let gf = p.start + fi;
                        let own = mesh.owner[gf];
                        let gg = cells[own];
                        let sf = mesh.face_area_vectors[gf];
                        let mag = sf.mag();
                        let n = Vector3::new(sf.x / mag, sf.y / mag, sf.z / mag);
                        let ng = Vector3::new(
                            n.x * gg.xx + n.y * gg.yx + n.z * gg.zx,
                            n.x * gg.xy + n.y * gg.yy + n.z * gg.zy,
                            n.x * gg.xz + n.y * gg.yz + n.z * gg.zz,
                        );
                        let sn = self.disp_patch_sn_grad(pi, fi);
                        let r = sn - ng;
                        Tensor::new(
                            gg.xx + n.x * r.x,
                            gg.xy + n.x * r.y,
                            gg.xz + n.x * r.z,
                            gg.yx + n.y * r.x,
                            gg.yy + n.y * r.y,
                            gg.yz + n.y * r.z,
                            gg.zx + n.z * r.x,
                            gg.zy + n.z * r.y,
                            gg.zz + n.z * r.z,
                        )
                    })
                    .collect()
            })
            .collect();
        calculated_tensor(mesh, "grad(disp)", cells, boundary)
    }

    /// `sigmaD` on cells and boundary faces from `grad(disp)`; boundary properties are the owner-cell ones.
    fn sigma_d_fields(&self, gd: &VolTensorField) -> (Vec<SymmTensor>, Vec<Vec<SymmTensor>>) {
        let mesh = &self.mesh;
        let cells = (0..mesh.n_cells).map(|c| sigma_d_of(&gd.internal[c], self.mu[c], self.lambda[c])).collect();
        let bnd = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        let o = mesh.owner[p.start + fi];
                        sigma_d_of(&gd.boundary[pi].values[fi], self.mu[o], self.lambda[o])
                    })
                    .collect()
            })
            .collect();
        (cells, bnd)
    }

    /// `fvc::div(sigmaD, "div(sigmaD)")` of a symmetric-tensor field given on cells and boundary faces.
    fn div_sigma_d(&self, cells: &[SymmTensor], bnd: &[Vec<SymmTensor>]) -> Vec<Vector3> {
        let c: Vec<Tensor> = cells.iter().map(|s| Tensor::from(*s)).collect();
        let b: Vec<Vec<Tensor>> = bnd.iter().map(|v| v.iter().map(|s| Tensor::from(*s)).collect()).collect();
        div_tensor_field(&self.mesh, &calculated_tensor(&self.mesh, "sigmaD", c, b))
    }

    // ---------------------------------------------------------------------------------------------------------
    // laplacian

    /// The linear interpolate of a cell field to the faces: internal `w*(P - N) + N`, boundary the owner value.
    fn interpolate_cells(&self, cells: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
        let mesh = &self.mesh;
        let internal = (0..mesh.n_internal_faces)
            .map(|f| {
                let w = linear_weight(mesh, f);
                (cells[mesh.owner[f]] - cells[mesh.neighbour[f]]) * w + cells[mesh.neighbour[f]]
            })
            .collect();
        (internal, owner_boundary(mesh, cells))
    }

    /// `fvmLaplacianUncorrected(gammaMagSf, deltaCoeffs, vf)` for `gamma` given per cell.
    fn laplacian_parts(&self, gamma: &[f64]) -> LaplacianParts {
        let mesh = &self.mesh;
        let (gi, gb) = self.interpolate_cells(gamma);
        let gamma_mag_sf_internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| gi[f] * self.geo.mag_sf[f]).collect();
        let gamma_mag_sf_boundary: Vec<Vec<f64>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| (0..p.size).map(|fi| gb[pi][fi] * self.geo.mag_sf[p.start + fi]).collect())
            .collect();
        let upper: Vec<f64> = (0..mesh.n_internal_faces).map(|f| self.geo.dc_internal[f] * gamma_mag_sf_internal[f]).collect();
        let mut diag = vec![0.0; mesh.n_cells];
        for f in 0..mesh.n_internal_faces {
            diag[mesh.owner[f]] -= upper[f];
            diag[mesh.neighbour[f]] -= upper[f];
        }
        LaplacianParts { upper, diag, gamma_mag_sf_boundary, gamma_mag_sf_internal }
    }

    /// `correctedSnGrad<scalar>::correction(vf)` on the internal faces for a cell scalar field (zero when the
    /// scheme is not `corrected`).
    fn scalar_correction(&self, f: &VolScalarField) -> Option<Vec<f64>> {
        if self.settings.laplacian != LaplacianCorrection::Corrected {
            return None;
        }
        let g = match self.settings.grad {
            GradScheme::LeastSquares => grad_least_squares(f),
            _ => grad(f),
        };
        Some(sn_grad_correction_with_grad(f, &g, NonOrthoScheme::Corrected))
    }

    /// `correctedSnGrad<vector>::correction(disp)` on the internal faces: component by component.
    fn vector_correction(&self) -> Option<Vec<Vector3>> {
        if self.settings.laplacian != LaplacianCorrection::Corrected {
            return None;
        }
        let comps: Vec<Vec<f64>> =
            (0..3).map(|k| self.scalar_correction(&self.disp_component(k)).unwrap_or_default()).collect();
        Some((0..self.mesh.n_internal_faces).map(|f| Vector3::new(comps[0][f], comps[1][f], comps[2][f])).collect())
    }

    // ---------------------------------------------------------------------------------------------------------
    // constructor tail

    /// `divSigmaExp_ = fvc::div(sigmaD_) - fvc::laplacian(2*mu_ + lambda_, disp_, "laplacian(DD,D)")` (compact),
    /// with `sigmaD_ = mu_*twoSymm(grad(disp_)) + lambda_*(I*tr(grad(disp_)))`.
    fn initial_div_sigma_exp(&mut self) {
        let gd = self.grad_disp();
        let (sc, sb) = self.sigma_d_fields(&gd);
        let div = self.div_sigma_d(&sc, &sb);
        if self.settings.compact_normal_stress {
            let lap = self.fvc_laplacian_disp();
            self.div_sigma_exp = (0..self.mesh.n_cells).map(|c| div[c] - lap[c]).collect();
        } else {
            // `divSigmaExp_ -= fvc::div((2*mu_ + lambda_)*fvc::grad(disp_), "div(sigmaD)")`
            let n = self.mesh.n_cells;
            let cells: Vec<Tensor> = (0..n).map(|c| gamma_times_grad(&gd.internal[c], self.two_mu_lambda[c])).collect();
            let bnd: Vec<Vec<Tensor>> = self
                .mesh
                .patches
                .iter()
                .enumerate()
                .map(|(pi, p)| {
                    (0..p.size)
                        .map(|fi| gamma_times_grad(&gd.boundary[pi].values[fi], self.two_mu_lambda[self.mesh.owner[p.start + fi]]))
                        .collect()
                })
                .collect();
            let d2 = div_tensor_field(&self.mesh, &calculated_tensor(&self.mesh, "gamma*grad(disp)", cells, bnd));
            self.div_sigma_exp = (0..n).map(|c| div[c] - d2[c]).collect();
        }
        self.sigma_d = sc;
        self.sigma_d_b = sb;
    }

    /// What a RESTART rebuilds (gh:#1919, README round 60): `disp` is READ (`MUST_READ`) and the constructor tail computes `sigmaD_` and
    /// `divSigmaExp_` from it (`legacyThermoMechanics.C:504-518`); the running process holds the `divSigmaExp_` its last corrector left
    /// (`fvc::div(DEqn.flux())` for `compactNormalStress no`), a different explicit source, so the first corrector of a restarted step starts from
    /// a residual that is NOT the converged one.
    pub fn restart_at(&mut self) {
        // `GFTractionDisplacement(p, iF, dict)`: `operator=(patchInternalField()); gradient() = Zero;` (the file's `value` is not read back)
        for pi in 0..self.disp_patches.len() {
            let (start, size) = (self.mesh.patches[pi].start, self.mesh.patches[pi].size);
            if let DispPatch::GfTractionDisplacement(t) = &mut self.disp_patches[pi] {
                t.reset_after_read();
                for fi in 0..size {
                    self.disp_b[pi][fi] = self.disp[self.mesh.owner[start + fi]];
                }
            }
        }
        self.initial_div_sigma_exp();
    }

    /// `fvc::laplacian(2*mu_ + lambda_, disp_, "laplacian(DD,D)")`: `fvc::div(gamma*snGrad(disp)*magSf)`.
    fn fvc_laplacian_disp(&self) -> Vec<Vector3> {
        let mesh = &self.mesh;
        let (gi, gb) = self.interpolate_cells(&self.two_mu_lambda);
        let corr = self.vector_correction();
        let internal: Vec<Vector3> = (0..mesh.n_internal_faces)
            .map(|f| {
                let mut sn = vscale(self.geo.dc_internal[f], self.disp[mesh.neighbour[f]] - self.disp[mesh.owner[f]]);
                if let Some(c) = &corr {
                    sn = sn + c[f];
                }
                vscale(self.geo.mag_sf[f], vscale(gi[f], sn))
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
                        let sn = self.disp_patch_sn_grad(pi, fi);
                        vscale(self.geo.mag_sf[gf], vscale(gb[pi][fi], sn))
                    })
                    .collect()
            })
            .collect();
        surface_integrate(mesh, &internal, &boundary)
    }

    // ---------------------------------------------------------------------------------------------------------
    // TEqn

    /// `TEqn.solve()` of `solveThermalMechanics.H`; returns its initial residual.
    fn solve_temperature(&mut self, log: &mut PhysicsLog) -> f64 {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let n_if = mesh.n_internal_faces;
        // isPorous_[celli] = TStructFromTH_[celli]/max(TStructFromTH_[celli], SMALL)
        for c in 0..n {
            let t = self.t_struct_from_th[c];
            self.is_porous[c] = t / if t > SMALL { t } else { SMALL };
        }
        let rho_c: Vec<f64> = (0..n).map(|c| self.rho[c] * self.c[c]).collect();
        // A = fvm::ddt(rhoC, T): diag = rDeltaT*rho*V, source = rDeltaT*rhoOld*Told*V (rhoC.oldTime() is rhoC)
        let r_delta_t = 1.0 / self.delta_t;
        let t_old = self.t_struct.clone();
        let diag_a: Vec<f64> = (0..n).map(|c| r_delta_t * rho_c[c] * mesh.cell_volumes[c]).collect();
        let src_a: Vec<f64> = (0..n).map(|c| r_delta_t * rho_c[c] * t_old[c] * mesh.cell_volumes[c]).collect();
        // B = fvm::laplacian(rhoK, TStruct)
        let lap = self.laplacian_parts(&self.rho_k);
        let t_field = calculated_scalar(&mesh, "TStruct", t_old.clone(), self.t_struct_boundary());
        let mut b_source = vec![0.0; n];
        if let Some(corr) = self.scalar_correction(&t_field) {
            let flux: Vec<f64> = (0..n_if).map(|f| lap.gamma_mag_sf_internal[f] * corr[f]).collect();
            let zero_b: Vec<Vec<f64>> = mesh.patches.iter().map(|p| vec![0.0; p.size]).collect();
            let div = surface_integrate(&mesh, &flux, &zero_b);
            for c in 0..n {
                b_source[c] -= mesh.cell_volumes[c] * div[c];
            }
        }
        // C = A - B (`diagonal() -= B`: upper = -B.upper), C - powerDensityNeutronics (source += V*q)
        let mut diag: Vec<f64> = (0..n).map(|c| diag_a[c] - lap.diag[c]).collect();
        let mut upper: Vec<f64> = lap.upper.iter().map(|u| -*u).collect();
        let mut lower: Vec<f64> = upper.clone();
        let mut source: Vec<f64> = (0..n).map(|c| src_a[c] - b_source[c]).collect();
        for c in 0..n {
            source[c] += mesh.cell_volumes[c] * self.power_density[c];
        }
        // (1.0 - isPorous_)*C : fvMatrix::operator*=(volScalarField::Internal) -> lduMatrix::operator*=(scalarField)
        let scale: Vec<f64> = (0..n).map(|c| 1.0 - self.is_porous[c]).collect();
        for c in 0..n {
            diag[c] *= scale[c];
        }
        for f in 0..n_if {
            upper[f] *= scale[mesh.owner[f]];
            lower[f] *= scale[mesh.neighbour[f]];
        }
        for c in 0..n {
            source[c] *= scale[c];
        }
        // == fvm::Sp(isPorous*1, T) - TStructFromTH*(isPorous*1): A - B, B.diag = V*sp, B.source = V*(TH*sp)
        let sp: Vec<f64> = (0..n).map(|c| self.is_porous[c] * 1.0).collect();
        for c in 0..n {
            let b_diag = mesh.cell_volumes[c] * sp[c];
            let b_src = mesh.cell_volumes[c] * (self.t_struct_from_th[c] * sp[c]);
            diag[c] -= b_diag;
            source[c] -= b_src;
        }
        // `fixedValue` patches: `C = A - B` has `internalCoeffs = 0 - (-gammaMagSf*deltaCoeffs)` and `boundaryCoeffs = 0 - (-gammaMagSf*(deltaCoeffs*value))`
        // (`gaussLaplacianScheme::fvmLaplacianUncorrected`), scaled by `(1 - isPorous)` of the owner (`fvMatrix::operator*=`), and added to the
        // diagonal and the source when the system is solved (`addBoundaryDiag`, `addBoundarySource`)
        for (pi, p) in mesh.patches.iter().enumerate() {
            if let ScalarPatch::FixedValue(v) = self.t_patches[pi] {
                for fi in 0..p.size {
                    let o = mesh.owner[p.start + fi];
                    let g_mag_sf = lap.gamma_mag_sf_boundary[pi][fi];
                    let dc = self.geo.dc_boundary[pi][fi];
                    diag[o] += scale[o] * (g_mag_sf * dc);
                    source[o] += scale[o] * (g_mag_sf * (dc * v));
                }
            }
        }
        let mut a = LduMatrix::new(n, mesh.owner[..n_if].to_vec(), mesh.neighbour[..n_if].to_vec());
        a.diag = diag;
        a.upper = upper;
        a.lower = lower;
        let mut psi = self.t_struct.clone();
        let s = self.settings.t_solver;
        let perf = pbicg_dilu(&a, &source, &mut psi, s.tolerance, s.rel_tol, s.max_iter, s.min_iter);
        self.t_struct = psi;
        log.solves.push(SolveRecord {
            field: "TStruct",
            initial_residual: perf.initial_residual,
            final_residual: perf.final_residual,
            n_iterations: perf.iterations,
        });
        perf.initial_residual
    }

    // ---------------------------------------------------------------------------------------------------------
    // DEqn loop

    /// `fvc::grad(threeKalpha_*(TStruct_ - TStructRef_))` (cell values).
    fn grad_thermal_load(&self) -> Vec<Vector3> {
        let n = self.mesh.n_cells;
        let cells: Vec<f64> = (0..n).map(|c| self.three_k_alpha[c] * (self.t_struct[c] - self.t_struct_ref[c])).collect();
        let tb = self.t_struct_boundary();
        let bnd: Vec<Vec<f64>> = self
            .mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        let o = self.mesh.owner[p.start + fi];
                        self.three_k_alpha[o] * (tb[pi][fi] - self.t_struct_ref[o])
                    })
                    .collect()
            })
            .collect();
        let f = calculated_scalar(&self.mesh, "threeKalpha*(TStruct-TStructRef)", cells, bnd);
        self.grad_scalar(&f)
    }

    /// One pass of the `do { ... } while` of `solveThermalMechanics.H`; returns the maximum initial residual.
    fn displacement_corrector(&mut self, log: &mut PhysicsLog) -> f64 {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let n_if = mesh.n_internal_faces;
        // `fvm::d2dt2(disp_)` constructs an fvMatrix of `disp`: `updateCoeffs()` of every patch
        self.update_disp_coeffs();
        let lap = self.laplacian_parts(&self.two_mu_lambda);
        // `fvm::d2dt2(disp_)` (`Euler`: diag and source from the old and old-old displacements; `steadyState`: zeros)
        let (d2_diag, d2_src) = self.d2dt2_terms();
        // B = fvm::laplacian(...) + divSigmaExp_ : source = (0 - V div(gammaMagSf corr)) - V divSigmaExp
        let mut b_source = vec![Vector3::ZERO; n];
        // `gammaMagSf*correction(disp)`: also stored as the matrix's `faceFluxCorrection` when `fluxRequired(disp)`
        let corr_opt = self.vector_correction();
        if let Some(corr) = &corr_opt {
            let flux: Vec<Vector3> = (0..n_if).map(|f| vscale(lap.gamma_mag_sf_internal[f], corr[f])).collect();
            let zero_b: Vec<Vec<Vector3>> = mesh.patches.iter().map(|p| vec![Vector3::ZERO; p.size]).collect();
            let div = surface_integrate(&mesh, &flux, &zero_b);
            for c in 0..n {
                let vd = vscale(mesh.cell_volumes[c], div[c]);
                b_source[c] = Vector3::new(b_source[c].x - vd.x, b_source[c].y - vd.y, b_source[c].z - vd.z);
            }
        }
        for c in 0..n {
            let vd = vscale(mesh.cell_volumes[c], self.div_sigma_exp[c]);
            b_source[c] = Vector3::new(b_source[c].x - vd.x, b_source[c].y - vd.y, b_source[c].z - vd.z);
        }
        // A == B with A the d2dt2 matrix (empty for steadyState): diag = A.diag - B.diag, upper = lower = 0 - B.upper,
        // source = A.source - B.source
        let mut a = LduMatrix::new(n, mesh.owner[..n_if].to_vec(), mesh.neighbour[..n_if].to_vec());
        for c in 0..n {
            a.diag[c] = d2_diag[c] - lap.diag[c];
        }
        for f in 0..n_if {
            a.upper[f] = 0.0 - lap.upper[f];
            a.lower[f] = 0.0 - lap.upper[f];
        }
        let mut src: Vec<Vector3> =
            (0..n).map(|c| Vector3::new(d2_src[c].x - b_source[c].x, d2_src[c].y - b_source[c].y, d2_src[c].z - b_source[c].z)).collect();
        // DEqn += fvc::grad(threeKalpha*(T - TRef)): source -= V*grad
        let gt = self.grad_thermal_load();
        for c in 0..n {
            let vg = vscale(mesh.cell_volumes[c], gt[c]);
            src[c] = Vector3::new(src[c].x - vg.x, src[c].y - vg.y, src[c].z - vg.z);
        }
        // fvMatrix::solveSegregated: diag += internalCoeffs, source += boundaryCoeffs (patch by patch)
        let mut bnd_src = src.clone();
        // `internalCoeffs`/`boundaryCoeffs` of `DEqn` per boundary face, as `fvMatrix::flux()` reads them
        let mut flux_int: Vec<Vec<f64>> = mesh.patches.iter().map(|p| vec![0.0; p.size]).collect();
        let mut flux_bnd: Vec<Vec<Vector3>> = mesh.patches.iter().map(|p| vec![Vector3::ZERO; p.size]).collect();
        for (pi, p) in mesh.patches.iter().enumerate() {
            match &self.disp_patches[pi] {
                DispPatch::FixedValue(v) => {
                    for fi in 0..p.size {
                        let gf = p.start + fi;
                        let cell = mesh.owner[gf];
                        let g_mag_sf = lap.gamma_mag_sf_boundary[pi][fi];
                        let dc = self.geo.dc_boundary[pi][fi];
                        a.diag[cell] += g_mag_sf * dc;
                        bnd_src[cell] = Vector3::new(
                            bnd_src[cell].x + g_mag_sf * (dc * v.x),
                            bnd_src[cell].y + g_mag_sf * (dc * v.y),
                            bnd_src[cell].z + g_mag_sf * (dc * v.z),
                        );
                        flux_int[pi][fi] = g_mag_sf * dc;
                        flux_bnd[pi][fi] = Vector3::new(g_mag_sf * (dc * v.x), g_mag_sf * (dc * v.y), g_mag_sf * (dc * v.z));
                    }
                }
                // fixedGradient: internalCoeffs 0 (nothing added to the diagonal), boundaryCoeffs of `A == B`:
                // `0 - (-pGamma*gradient)`, added to the source of the face's cell, patch by patch, face by face
                DispPatch::GfTractionDisplacement(t) => {
                    for fi in 0..p.size {
                        let cell = mesh.owner[p.start + fi];
                        let g_mag_sf = lap.gamma_mag_sf_boundary[pi][fi];
                        let g = t.gradient()[fi];
                        bnd_src[cell] = Vector3::new(
                            bnd_src[cell].x + g_mag_sf * g.x,
                            bnd_src[cell].y + g_mag_sf * g.y,
                            bnd_src[cell].z + g_mag_sf * g.z,
                        );
                        // internalCoeffs 0 (gradientInternalCoeffs of a fixedGradient patch), boundaryCoeffs `0 - (-gMagSf*gradient)`
                        flux_bnd[pi][fi] = Vector3::new(g_mag_sf * g.x, g_mag_sf * g.y, g_mag_sf * g.z);
                    }
                }
                DispPatch::ZeroGradient => {}
            }
        }
        let names = ["dispx", "dispy", "dispz"];
        let s = self.settings.d_solver;
        let mut max_initial = 0.0_f64;
        let mut comps: Vec<Vec<f64>> = (0..3).map(|k| self.disp.iter().map(|v| cmpt(*v, k)).collect()).collect();
        for k in 0..3 {
            if !self.solution_d[k] {
                // `fvMatrix<vector>::solveSegregated`: a component with `solutionD < 0` (the `empty` direction) is not solved
                continue;
            }
            let source: Vec<f64> = bnd_src.iter().map(|v| cmpt(*v, k)).collect();
            let perf = pcg_dic(&a, &source, &mut comps[k], s.tolerance, s.rel_tol, s.max_iter, s.min_iter);
            if perf.initial_residual > max_initial {
                max_initial = perf.initial_residual;
            }
            log.solves.push(SolveRecord {
                field: names[k],
                initial_residual: perf.initial_residual,
                final_residual: perf.final_residual,
                n_iterations: perf.iterations,
            });
        }
        for c in 0..n {
            self.disp[c] = Vector3::new(comps[0][c], comps[1][c], comps[2][c]);
        }
        // `if (!compactNormalStress_) divSigmaExp_ = fvc::div(DEqn.flux());` straight after the solve, with the
        // coefficients stored when `DEqn` was built (before `correctBoundaryConditions()` recomputes a traction gradient)
        let flux_div = if self.settings.compact_normal_stress {
            None
        } else {
            Some(self.flux_divergence(&lap, corr_opt.as_deref(), &flux_int, &flux_bnd))
        };
        self.correct_disp_boundary();
        // volTensorField gradD(fvc::grad(disp_)); sigmaD_ = ...; divSigmaExp_ = fvc::div(sigmaD_ - (2 mu + lambda) gradD, "div(sigmaD)")
        let gd = self.grad_disp();
        let (sc, sb) = self.sigma_d_fields(&gd);
        if let Some(fd) = flux_div {
            // `divSigmaExp_ += fvc::div(sigmaD_)`
            let ds = self.div_sigma_d(&sc, &sb);
            self.div_sigma_exp = (0..n).map(|c| fd[c] + ds[c]).collect();
        } else {
            let cells: Vec<Tensor> =
                (0..n).map(|c| sigma_minus_gamma_grad(&sc[c], &gd.internal[c], self.two_mu_lambda[c])).collect();
            let bnd: Vec<Vec<Tensor>> = mesh
                .patches
                .iter()
                .enumerate()
                .map(|(pi, p)| {
                    (0..p.size)
                        .map(|fi| {
                            let o = mesh.owner[p.start + fi];
                            sigma_minus_gamma_grad(&sb[pi][fi], &gd.boundary[pi].values[fi], self.two_mu_lambda[o])
                        })
                        .collect()
                })
                .collect();
            self.div_sigma_exp = div_tensor_field(&mesh, &calculated_tensor(&mesh, "sigmaD-gammagradD", cells, bnd));
        }
        self.sigma_d = sc;
        self.sigma_d_b = sb;
        max_initial
    }

    /// `fvc::div(DEqn.flux())` of the displacement equation just solved (`compactNormalStress no`), gh:#1845.
    ///
    /// `fvMatrix::flux()` (ESI v2506 `fvMatrix.C:1422`): internal faces `lduMatrix::faceH(psi)` =
    /// `Upper[f]*psi[u] - Lower[f]*psi[l]` (`Upper = Lower = 0 - B.upper`: the matrix is `A - B`, `A` the d2dt2 matrix,
    /// diagonal only); boundary faces `cmptMultiply(internalCoeffs, psi[owner]) - boundaryCoeffs`; then
    /// `+= *faceFluxCorrectionPtr_`, which for `A - B` is `-(gammaMagSf*correction(psi))` (`fvMatrix::operator-=` negates
    /// `B`'s), stored when `DEqn` was built, so computed from the displacement BEFORE the solve (`corr`). `psi` here is the
    /// displacement after the solve. The sum over a cell's faces and the division by `V` are `fvc::surfaceIntegrate`.
    fn flux_divergence(
        &self,
        lap: &LaplacianParts,
        corr: Option<&[Vector3]>,
        flux_int: &[Vec<f64>],
        flux_bnd: &[Vec<Vector3>],
    ) -> Vec<Vector3> {
        let mesh = &self.mesh;
        let internal: Vec<Vector3> = (0..mesh.n_internal_faces)
            .map(|f| {
                let u = 0.0 - lap.upper[f];
                let (own, nb) = (mesh.owner[f], mesh.neighbour[f]);
                let h = vscale(u, self.disp[nb]) - vscale(u, self.disp[own]);
                match corr {
                    Some(c) => {
                        let g = vscale(lap.gamma_mag_sf_internal[f], c[f]);
                        h + Vector3::new(-g.x, -g.y, -g.z)
                    }
                    None => h,
                }
            })
            .collect();
        let boundary: Vec<Vec<Vector3>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| vscale(flux_int[pi][fi], self.disp[mesh.owner[p.start + fi]]) - flux_bnd[pi][fi])
                    .collect()
            })
            .collect();
        surface_integrate(mesh, &internal, &boundary)
    }

    /// `calculateStress.H`: the maximum of `sigmaEq = sqrt((3/2) magSqr(dev(sigma)))` over cells and boundary faces,
    /// `sigma = rho*sigmaD - I*(rho*threeKalpha*(TStruct - TStructRef))`.
    fn max_sigma_eq(&self) -> f64 {
        let mesh = &self.mesh;
        let eq = |s: &SymmTensor, o: usize, t: f64| -> f64 {
            let rho = self.rho[o];
            let ii = rho * self.three_k_alpha[o] * (t - self.t_struct_ref[o]);
            let sg = SymmTensor {
                xx: rho * s.xx - ii,
                xy: rho * s.xy,
                xz: rho * s.xz,
                yy: rho * s.yy - ii,
                yz: rho * s.yz,
                zz: rho * s.zz - ii,
            };
            let sph = (1.0 / 3.0) * (sg.xx + sg.yy + sg.zz);
            let d = SymmTensor { xx: sg.xx - sph, xy: sg.xy, xz: sg.xz, yy: sg.yy - sph, yz: sg.yz, zz: sg.zz - sph };
            let mag_sqr =
                d.xx * d.xx + 2.0 * (d.xy * d.xy) + 2.0 * (d.xz * d.xz) + d.yy * d.yy + 2.0 * (d.yz * d.yz) + d.zz * d.zz;
            ((3.0 / 2.0) * mag_sqr).sqrt()
        };
        let tb = self.t_struct_boundary();
        let mut m = f64::MIN;
        for c in 0..mesh.n_cells {
            let v = eq(&self.sigma_d[c], c, self.t_struct[c]);
            if v > m {
                m = v;
            }
        }
        for (pi, p) in mesh.patches.iter().enumerate() {
            for fi in 0..p.size {
                let v = eq(&self.sigma_d_b[pi][fi], mesh.owner[p.start + fi], tb[pi][fi]);
                if v > m {
                    m = v;
                }
            }
        }
        m
    }

    // ---------------------------------------------------------------------------------------------------------
    // expansion equations

    /// `fvm::div(faceFlux, psi, "div(...)") == rhs` with `Gauss upwind`, solved with `PBiCG`/`DILU`.
    fn solve_upwind_expansion(
        &self,
        name: &'static str,
        orientation: Vector3,
        patches: &[ScalarPatch],
        psi: &mut Vec<f64>,
        rhs: &[f64],
        s: LinearSolverSettings,
        log: &mut PhysicsLog,
    ) {
        let mesh = &self.mesh;
        let n = mesh.n_cells;
        let n_if = mesh.n_internal_faces;
        let flux = |sf: Vector3| orientation.x * sf.x + orientation.y * sf.y + orientation.z * sf.z;
        let mut a = LduMatrix::new(n, mesh.owner[..n_if].to_vec(), mesh.neighbour[..n_if].to_vec());
        // fvm.lower() = -weights*flux, fvm.upper() = fvm.lower() + flux; negSumDiag; weights = pos0(flux)
        for f in 0..n_if {
            let fl = flux(mesh.face_area_vectors[f]);
            let w = if fl >= 0.0 { 1.0 } else { 0.0 };
            a.lower[f] = -w * fl;
            a.upper[f] = a.lower[f] + fl;
        }
        for f in 0..n_if {
            a.diag[mesh.owner[f]] -= a.lower[f];
            a.diag[mesh.neighbour[f]] -= a.upper[f];
        }
        // `== rhs`: source += V*rhs
        let mut source: Vec<f64> = (0..n).map(|c| mesh.cell_volumes[c] * rhs[c]).collect();
        // boundary: internalCoeffs = patchFlux*valueInternalCoeffs, boundaryCoeffs = -patchFlux*valueBoundaryCoeffs
        for (pi, p) in mesh.patches.iter().enumerate() {
            for fi in 0..p.size {
                let gf = p.start + fi;
                let cell = mesh.owner[gf];
                let pf = flux(mesh.face_area_vectors[gf]);
                match patches[pi] {
                    ScalarPatch::FixedValue(v) => {
                        // valueInternalCoeffs = 0, valueBoundaryCoeffs = value
                        a.diag[cell] += pf * 0.0;
                        source[cell] += -pf * v;
                    }
                    ScalarPatch::ZeroGradient => {
                        // valueInternalCoeffs = 1, valueBoundaryCoeffs = 0
                        a.diag[cell] += pf * 1.0;
                        source[cell] += -pf * 0.0;
                    }
                }
            }
        }
        let perf = pbicg_dilu(&a, &source, psi, s.tolerance, s.rel_tol, s.max_iter, s.min_iter);
        log.solves.push(SolveRecord {
            field: name,
            initial_residual: perf.initial_residual,
            final_residual: perf.final_residual,
            n_iterations: perf.iterations,
        });
        // correctBoundaryConditions: boundary values are not stored (zeroGradient follows the owner, fixedValue is fixed)
    }

    /// The tail of `solveThermalMechanics.H`: `fuelDispEqn`, `CRDispEqn`, `fuelDispVector_`, `meshDisp_`.
    fn solve_expansion(&mut self, log: &mut PhysicsLog) {
        let n = self.mesh.n_cells;
        let orient = self.settings.pin_direction;
        let fuel_rhs: Vec<f64> = (0..n).map(|c| self.alpha_fuel[c] * (self.t_fuel[c] - self.t_fuel_ref[c])).collect();
        // CRDispEqn: `-alphaCR_*(TStruct_ - TCRRef_)` is (-alphaCR)*(T - TCRRef); CROrientation = -fuelOrientation_
        let cr_rhs: Vec<f64> = (0..n).map(|c| (-self.alpha_cr[c]) * (self.t_struct[c] - self.t_cr_ref[c])).collect();
        let cr_orient = Vector3::new(-orient.x, -orient.y, -orient.z);
        let fuel_patches = self.fuel_patches.clone();
        let cr_patches = self.cr_patches.clone();
        let mut fuel = self.fuel_disp.clone();
        let mut cr = self.cr_disp.clone();
        let (fs, cs) = (self.settings.fuel_solver, self.settings.cr_solver);
        self.solve_upwind_expansion("fuelDisp", orient, &fuel_patches, &mut fuel, &fuel_rhs, fs, log);
        self.solve_upwind_expansion("CRDisp", cr_orient, &cr_patches, &mut cr, &cr_rhs, cs, log);
        self.fuel_disp = fuel;
        self.cr_disp = cr;
        // fuelDispVector_ = (fuelDisp_ + CRDisp_)*fuelOrientation_;
        // meshDisp_ = (disp_ - ((disp_ & fuelOrientation_)*fuelOrientation_)) + fuelDispVector_
        for c in 0..n {
            let s = self.fuel_disp[c] + self.cr_disp[c];
            let fdv = vscale(s, orient);
            let d = self.disp[c];
            let dn = d.x * orient.x + d.y * orient.y + d.z * orient.z;
            let axial = vscale(dn, orient);
            let rem = Vector3::new(d.x - axial.x, d.y - axial.y, d.z - axial.z);
            self.mesh_disp[c] = Vector3::new(rem.x + fdv.x, rem.y + fdv.y, rem.z + fdv.z);
        }
    }

    // ---------------------------------------------------------------------------------------------------------

    /// `legacyThermoMechanics::correctPhysics()`: one pass of `solveThermalMechanics.H` (`GeN-Foam.C` calls it once
    /// per time step), then `calculateStress.H`'s maximum `sigmaEq`.
    pub fn correct_physics(&mut self) -> PhysicsLog {
        let mut log = PhysicsLog::default();
        let mut residual = 0.0_f64;
        self.advance_time_levels();
        if self.settings.solve_thermo {
            let r = self.solve_temperature(&mut log);
            residual = residual.max(r);
        }
        if self.settings.solve_displacement {
            self.store_old_displacements();
            let mut i_corr = 0usize;
            let mut initial;
            loop {
                initial = self.displacement_corrector(&mut log);
                i_corr += 1;
                // `while (initialResidual > convergenceTolerance_ && ++iCorr < nCorr_)`
                if !(initial > self.settings.convergence_tolerance && i_corr < self.settings.n_correctors) {
                    break;
                }
            }
            log.n_correctors_run = i_corr;
            residual = residual.max(initial);
            log.max_sigma_eq = Some(self.max_sigma_eq());
            self.solve_expansion(&mut log);
        } else {
            // disp_ *= 0.0, fuelDisp_ *= 0.0, CRDisp_ *= 0.0, meshDisp_ *= 0.0
            for d in self.disp.iter_mut() {
                *d = Vector3::ZERO;
            }
            // `fvPatchField::operator*=(0.0)` is live on a fixedGradient patch (fixedValue/zeroGradient ignore it)
            for pi in 0..self.disp_patches.len() {
                if matches!(self.disp_patches[pi], DispPatch::GfTractionDisplacement(_)) {
                    for v in self.disp_b[pi].iter_mut() {
                        *v = Vector3::new(v.x * 0.0, v.y * 0.0, v.z * 0.0);
                    }
                }
            }
            self.correct_disp_boundary();
            for v in self.fuel_disp.iter_mut().chain(self.cr_disp.iter_mut()) {
                *v *= 0.0;
            }
            for m in self.mesh_disp.iter_mut() {
                *m = Vector3::ZERO;
            }
        }
        self.residual = residual;
        log.residual = residual;
        log
    }
}
