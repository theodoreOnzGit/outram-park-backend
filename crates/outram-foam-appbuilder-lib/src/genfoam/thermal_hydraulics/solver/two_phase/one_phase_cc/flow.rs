// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/onePhase/onePhase.C (correctFluidMechanics,
//     correctContErr), include/equations/UEqn_1p.H, pEqn_1p.H and pEqnRoutines/{updateAlphaPhiAndInitControls_1p,
//     correctBoundaryFlux_1p, calcPhiSbyA_1p, calcrAUfSetSnGradCalcComp_1p, solveP_rgh_1p, updatePP_rghRho_1p}.H,
//   src/physicsModels/phasePairs/FSPair.C (`correct`, Kd), dragModels/FSDragFactor.C (`correctField`),
//   FSDragCoefficientModels/ReynoldsPower, phaseModels/structureModels/{structure.C (momentumSource),
//   pump/pump.C}, phaseModels/fluid/fluid.C (`initAlphaPhis`, `rho`), customPimpleControl (`correct(bool)`),
//   and OpenFOAM-ESI v2506 `fvMatrix::{relax,H,A,setReference,flux}`, `gaussConvectionScheme`,
//   `gaussLaplacianScheme`, `wedgeFvPatchField`, `transformFvPatchField`, `fvc::{reconstruct,flux,snGrad}`.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `one_phase_cc::flow` — `onePhase` `solveFluidMechanics true`, `momentumMode cellCentered`, turbulence OFF
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Translation of GeN-Foam's `UEqn_1p.H` + `pEqn_1p.H` (gh:#1837, gh:#1361 round 19) for the `2D_MSFR`
//! `steadyStateTH` stage with `turbulence false`. Per PIMPLE outer iteration:
//! `correctModels(true, true)` (Reynolds number and the isotropic `ReynoldsPower` drag `Kd`), then
//!
//! 1. `UEqn = div(alphaRhoPhi, U) + SuSp(-contErr, U) - laplacian(alpha rho nuEff, U)
//!    - div(alpha rho nuEff dev2(T(grad U))) + Sp(tr(Kd)/3, U) + (dev(Kd) & U) + ddt(alpha, rho, U)`, then
//!    `UEqn.relax()` (per component, `fvMatrix::relax(0.9)`); there is NO velocity predictor.
//! 2. `pEqn`: the fluxes `alphaPhi`, `alphaRhoPhi` and `contErr` are refreshed from the CURRENT `phi`
//!    (`updateAlphaPhiAndInitControls_1p.H`) BEFORE the pressure correctors, so the energy equation of the same outer
//!    iteration (and the next `UEqn`) sees the flux of the previous corrector set, as upstream does; then up to
//!    `nCorrectors` PISO correctors, each `p_rgh = p - rho0 gh`, `phiHbyA`, the `fixedFluxPressure` gradient,
//!    `pEqn = (contErr - (ddt(alpha) + div(alphaPhi)) rho0)/rho0 + div(phiHbyA) - laplacian(rAUf, p_rgh)` with
//!    `setReference(pRefCell, pRefValue)` (the VALUE branch: a Boussinesq fluid has `psi = 0`), PCG/DIC,
//!    `phi_ = phiHbyA + pEqnIncomp.flux()`, `phi = phi_/alphaf`, `U = HbyA + reconstruct(...)`, and
//!    `p = max(p_rgh + rho0 gh, pMin)`, `p_rgh = p - rho0 gh`.
//! 3. `correctContErr()` (a no-op value-wise: `alphaRhoPhi` was last set at the start of `pEqn`).
//!
//! ## Wedge patches (gh:#1812)
//!
//! `U` carries `fv::VectorBc::Wedge`; every TEMPORARY vector field (`UEqn.H()`, `HbyA`, the pump source) takes the
//! constraint patch of the wedge mesh patch whatever type was requested ([`fv::Vector::uniform_with_constraint_patches`],
//! `fvPatchField::New`); the scalar wedge is `zeroGradient` in value and in matrix coefficients
//! (`transformFvPatchField` for an arithmetic type).
//!
//! ## What is NOT ported (refused by name when the case is read)
//!
//! ~~`turbulence true` (gh:#1838 wall functions, gh:#1839 `porousKEpsilon::correct`)~~ **CORRECTED 2026-11-26 (round 20): ported, see [`super::turbulence`]**; ~~`momentumMode` other than
//! `cellCentered`~~ **CORRECTED 2026-11-27 (gh:#1865, gh:#1870 round 62): `cellCenteredFaceReconstruction` and `porousInterfaceSharpness` are ported ([`reconstruct_u_1p`])**, ~~`nNonOrthogonalCorrectors != 0`~~ **CORRECTED 2026-11-27 (gh:#1866, round 33): ported (`n + 1` passes of the pressure solve, `p_rghFinal` in the last pass of the last corrector, `updateFirstPISOPrevPIMPLE` per pass; `corrected` Laplacian/snGrad schemes are refused by name)**, `minKdFS != 0`, ~~a
//! `momentumSourceTimeProfile`~~ **ported (round 62)**, drag models other than `ReynoldsPower`, tortuosity, `fvOptions`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::primitives::Vector3;
use petir::mathf::RealMath;
use uom::si::ratio::ratio;
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;

use super::super::fv::{self, Bc, Eqn, Geometry, Scalar, Vector, VectorBc};
use super::super::pcg::pcg_dic;
use super::super::{dev2_div, dev2_div_tortuous, tensor_laplacian, PRghPcg};
use super::OnePhaseCcSolver;
use super::super::thermo::PhaseThermo;
#[cfg(feature = "std")]
use super::super::case::{bool_entry, read_field, read_field_bcs_only, scalar, word, Ctx, FieldFile};
#[cfg(feature = "std")]
use super::last_match;
#[cfg(feature = "std")]
use crate::error::AppBuilderError;
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry, FoamValue};
use outram_foam_basic_lib::mesh::PatchKind;

/// `FSDragCoefficientModels::ReynoldsPower`: `coeff Re^exp + const` (`ReynoldsPowerFSDragCoefficient.C`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReynoldsPowerDrag {
    /// `coeff`.
    pub coeff: f64,
    /// `exp`.
    pub exp: f64,
    /// `const` (default 0).
    pub constant: f64,
}

impl ReynoldsPowerDrag {
    /// `coeff_*pow(Rei, exp_) + const_`.
    #[must_use]
    pub fn value(&self, re: f64) -> f64 {
        self.coeff * re.r_powf(self.exp) + self.constant
    }
}

/// One `FSDragCoefficientModel` of the solved flow: `ReynoldsPower`, or a wall-friction correlation of [`FsWallFriction`]
/// (`Churchill`, `Rehme`; gh:#1870 round 62).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragLaw {
    /// `ReynoldsPower`.
    ReynoldsPower(ReynoldsPowerDrag),
    /// `Churchill` or `Rehme` (the closures of `closures::fs_drag`).
    Wall(FsWallFriction),
}

impl DragLaw {
    /// The model's `value(celli)` at the cell's Reynolds number.
    #[must_use]
    pub fn value(&self, re: f64) -> f64 {
        match self {
            Self::ReynoldsPower(d) => d.value(re),
            Self::Wall(w) => w.friction_factor(ReynoldsNumber::new::<ratio>(re)).get::<ratio>(),
        }
    }
}

/// The drag model(s) of one cell (`FSDragFactor`): ONE isotropic model (`type` given: `Kd = diag(v, v, v)`) or per-axis models
/// (`localX`, `localY`, `localZ`; an absent axis keeps its `Kd` entry 0), as `FSDragFactor::correctField` (gh:#1870 round 62).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellDrag {
    /// Isotropic (`laws[0]` is the one model) or per-axis.
    pub isotropic: bool,
    /// The models of the `x`, `y`, `z` axes.
    pub laws: [Option<DragLaw>; 3],
}

/// The `solveFluidMechanics true` part of a read case.
#[derive(Debug, Clone)]
pub struct OnePhaseFlowCase {
    /// `PIMPLE.nCorrectors`.
    pub n_correctors: usize,
    /// `PIMPLE.correctUntilConvergence` (a corrector loop ends after a solve of ZERO iterations).
    pub correct_until_convergence: bool,
    /// The `p_rgh` solver entry (`PCG`/`DIC`).
    pub p_rgh: PRghPcg,
    /// The `p_rghFinal` entry, used by the last corrector of the last outer iteration.
    pub p_rgh_final: PRghPcg,
    /// `true` when `solvers.p_rgh` selected `GAMG` and `PCG`/`DIC` ran in its place (the named deviation of [`super::GamgPolicy`]).
    pub p_rgh_gamg_substituted: bool,
    /// `pMin` [Pa].
    pub p_min: f64,
    /// `pRefCell`.
    pub p_ref_cell: usize,
    /// `pRefValue` [Pa].
    pub p_ref_value: f64,
    /// `PIMPLE.nNonOrthogonalCorrectors` (default 0, gh:#1866): the pressure equation is solved `n + 1` times per corrector.
    pub n_non_orth_correctors: usize,
    /// `PIMPLE.minMagU` (default 0), the floor of `|U|` in the drag.
    pub min_mag_u: f64,
    /// `hRef` of `constant/<region>/hRef` (default 0).
    pub h_ref: f64,
    /// One velocity boundary condition per patch (`fixedValue`, `zeroGradient`, `slip`, `wedge`; `empty` is `zeroGradient` on no face).
    pub u_bc: Vec<VectorBc>,
    /// One temperature boundary condition per patch (`fixedValue`, `zeroGradient`, `wedge` as `zeroGradient`, `empty`): what the
    /// energy equation's `he` patches follow (`fixedEnergy` / `gradientEnergy`, gh:#1913 round 55).
    pub t_bc: Vec<Bc>,
    /// `p_rgh.needReference()`: no patch fixes the value of `p_rgh` (`setReference` and the closed-volume correction act only then).
    pub p_rgh_need_reference: bool,
    /// One `p_rgh` boundary condition per patch (`fixedFluxPressure` as `FixedGradient`, `wedge` as `ZeroGradient`).
    pub p_rgh_bc: Vec<Bc>,
    /// `p_rgh` at the start time (cells).
    pub p_rgh0: Vec<f64>,
    /// `nut` at the start time (cells; never updated with `turbulence false`).
    pub nut: Vec<f64>,
    /// `nut` boundary values by `face - n_internal`: a wall-function patch keeps its READ `value` (no wall function
    /// runs with `turbulence false`), a wedge patch is the owner cell value.
    pub nut_boundary: Vec<f64>,
    /// The pump `momentumSource` per cell.
    pub momentum_source: Vec<Vector3>,
    /// `momentumMode cellCenteredFaceReconstruction` (`pEqn_1p.H`: `U` from `reconstructU_1p.H`, gh:#1865, gh:#1870 round 62).
    pub reconstruct_u: bool,
    /// `porous_sharp` (default 0), the weight of `reconstruct(phi_)/alpha` in `reconstructU_1p.H`.
    pub porous_interface_sharpness: f64,
    /// `momentumSourceTimeProfile` of each pump zone (`pump::correct`, gh:#1870 round 62): the zone's cells, its `momentumSource`
    /// vector and the table; each pressure corrector sets the zone's cells to `momentumSource * profile.value(t)` at the current time.
    pub momentum_profiles: Vec<(Vec<usize>, Vector3, crate::genfoam::common::TimeProfile)>,
    /// The `ReynoldsPower` drag per cell (`None` outside the drag zones).
    pub drag: Vec<Option<CellDrag>>,
    /// `turbulence true`: `porousKEpsilon` with the wall functions (`None` with `turbulence false`).
    pub turbulence: Option<super::OnePhaseTurbulenceCase>,
}

/// One pressure solve of the PISO loop, as upstream's log prints it (`DICPCG: Solving for p_rgh`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureSolve {
    /// PIMPLE outer iteration (1-based).
    pub outer: usize,
    /// PISO corrector (1-based).
    pub corrector: usize,
    /// Non-orthogonal pass within the corrector (1-based, gh:#1866).
    pub non_orth: usize,
    /// Normalised initial residual.
    pub initial_residual: f64,
    /// Iterations.
    pub iterations: usize,
}

/// The mutable state of the solved flow.
#[derive(Debug, Clone)]
pub(super) struct FlowState {
    /// `U.oldTime()`.
    pub u_old: Vec<Vector3>,
    /// `p_rgh`: cells, boundary, `fixedFluxPressure` as `FixedGradient`.
    pub p_rgh: Scalar,
    /// `fluid_.phi()`, every face (a `fixedValue` patch keeps `Sf . U_b`).
    pub phi: Vec<f64>,
    /// The solver's `phi_` (the volumetric flux the Courant number is built from).
    pub phi_solver: Vec<f64>,
    /// `fluid_.magU()` per cell.
    pub mag_u: Vec<f64>,
    /// `Kd` per cell: the three diagonal entries (`xx`, `yy`, `zz`) of the diagonal drag tensor.
    pub kd: Vec<[f64; 3]>,
    /// `gh = g . C + |g| hRef` per cell.
    pub gh: Vec<f64>,
    /// `ghf` on every face.
    pub ghf: Vec<f64>,
    /// The pressure solves of the last [`OnePhaseCcSolver::correct_physics`] call.
    pub solves: Vec<PressureSolve>,
}

/// `fvMatrix<vector>` with `internalCoeffs`/`boundaryCoeffs` kept per boundary face, as OpenFOAM keeps them
/// (`fvMatrix::relax`, `A()` and `H()` treat them separately from the diagonal and the source).
pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) struct UEqnCc {
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) eqn: Eqn,
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) src: [Vec<f64>; 3],
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) ic: Vec<[f64; 3]>,
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) bcoef: Vec<[f64; 3]>,
}

fn cmp(v: Vector3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

impl UEqnCc {
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn new(g: &Geometry) -> Self {
        let n = g.n_cells();
        let nb = g.n_faces() - g.n_internal();
        Self {
            eqn: Eqn::new(g),
            src: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            ic: vec![[0.0; 3]; nb],
            bcoef: vec![[0.0; 3]; nb],
        }
    }

    /// The boundary part of `fvm::div(phi, U)`: `internalCoeffs = phi valueInternalCoeffs`,
    /// `boundaryCoeffs = -phi valueBoundaryCoeffs`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn boundary_convection(&mut self, g: &Geometry, phi: &[f64], u: &Vector) {
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let c = u.patch_coeffs(g, f, [&[], &[]]);
            for k in 0..3 {
                self.ic[f - ni][k] += phi[f] * c.value_internal[k];
                self.bcoef[f - ni][k] += -phi[f] * c.value_boundary[k];
            }
        }
    }

    /// The boundary part of `-fvm::laplacian(gamma, U)`: `internalCoeffs = -(pGamma gradientInternalCoeffs)`,
    /// `boundaryCoeffs = +pGamma gradientBoundaryCoeffs`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn boundary_laplacian(&mut self, g: &Geometry, gamma: &[f64], u: &Vector) {
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let c = u.patch_coeffs(g, f, [&[], &[]]);
            let pg = gamma[f] * g.mesh.face_areas[f];
            let d = g.delta[f];
            for k in 0..3 {
                self.ic[f - ni][k] += -(pg * (-d * c.gradient_internal_diag[k]));
                self.bcoef[f - ni][k] += pg * c.gradient_boundary[k];
            }
        }
    }

    /// `fvm::SuSp(coeff, U)` on the left-hand side.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn su_sp(&mut self, g: &Geometry, coeff: &[f64], u: &[Vector3]) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c];
            self.eqn.ldu.diag[c] += v * coeff[c].max(0.0);
            let e = v * coeff[c].min(0.0);
            self.src[0][c] -= e * u[c].x;
            self.src[1][c] -= e * u[c].y;
            self.src[2][c] -= e * u[c].z;
        }
    }

    /// `matrix + vector field`: `source -= V field`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn add_field(&mut self, g: &Geometry, f: &[Vector3]) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c];
            self.src[0][c] -= v * f[c].x;
            self.src[1][c] -= v * f[c].y;
            self.src[2][c] -= v * f[c].z;
        }
    }

    /// `matrix - vector field`: `source += V field`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn sub_field(&mut self, g: &Geometry, f: &[Vector3]) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c];
            self.src[0][c] += v * f[c].x;
            self.src[1][c] += v * f[c].y;
            self.src[2][c] += v * f[c].z;
        }
    }

    /// `fvMatrix::operator+=(const fvMatrix&)` (ESI v2506 `fvMatrix.C`): `lduMatrix::operator+=` (diagonal, upper, lower),
    /// `source_ +=`, `internalCoeffs_ +=` and `boundaryCoeffs_ +=`, each added as a WHOLE to the left matrix, so that a sum of
    /// terms rounds as OpenFOAM's `A + B + C` does (each term's own `negSumDiag` first, then the sum): gh:#1894 round 46,
    /// `OnePhaseFcSolver::correct_u_eqn`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn add(&mut self, o: &UEqnCc) {
        self.eqn.add(&o.eqn);
        for k in 0..3 {
            for (a, b) in self.src[k].iter_mut().zip(&o.src[k]) {
                *a += b;
            }
        }
        for (a, b) in self.ic.iter_mut().zip(&o.ic) {
            for k in 0..3 {
                a[k] += b[k];
            }
        }
        for (a, b) in self.bcoef.iter_mut().zip(&o.bcoef) {
            for k in 0..3 {
                a[k] += b[k];
            }
        }
    }

    /// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`) for a vector matrix: the boundary contribution is the
    /// maximum-magnitude component of `internalCoeffs` on the way in, its minimum component on the way out.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn relax(&mut self, g: &Geometry, alpha: f64, u: &[Vector3]) {
        if alpha <= 0.0 {
            return;
        }
        let n = g.n_cells();
        let ni = g.n_internal();
        let d0 = self.eqn.ldu.diag.clone();
        let mut d = d0.clone();
        let mut sum_off = vec![0.0; n];
        for f in 0..ni {
            sum_off[self.eqn.ldu.neighbour[f]] += self.eqn.ldu.lower[f].abs();
            sum_off[self.eqn.ldu.owner[f]] += self.eqn.ldu.upper[f].abs();
        }
        for f in ni..g.n_faces() {
            if g.active[f] {
                let ic = self.ic[f - ni];
                d[g.mesh.owner[f]] += ic[0].abs().max(ic[1].abs()).max(ic[2].abs());
            }
        }
        for c in 0..n {
            d[c] = d[c].abs().max(sum_off[c]);
        }
        for v in &mut d {
            *v /= alpha;
        }
        for f in ni..g.n_faces() {
            if g.active[f] {
                let ic = self.ic[f - ni];
                d[g.mesh.owner[f]] -= ic[0].min(ic[1]).min(ic[2]);
            }
        }
        for c in 0..n {
            let delta = d[c] - d0[c];
            self.src[0][c] += delta * u[c].x;
            self.src[1][c] += delta * u[c].y;
            self.src[2][c] += delta * u[c].z;
            self.eqn.ldu.diag[c] = d[c];
        }
    }

    /// `fvMatrix::A()` per unit volume: `(diag + cmptAv(internalCoeffs))/V`.
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn a(&self, g: &Geometry) -> Vec<f64> {
        let ni = g.n_internal();
        let mut d = self.eqn.ldu.diag.clone();
        for f in ni..g.n_faces() {
            if g.active[f] {
                let ic = self.ic[f - ni];
                d[g.mesh.owner[f]] += (ic[0] + ic[1] + ic[2]) / 3.0;
            }
        }
        (0..g.n_cells()).map(|c| d[c] / g.mesh.cell_volumes[c]).collect()
    }

    /// `fvMatrix::H()` per unit volume, cell values only (the caller evaluates the constraint patches).
    pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn h(&self, g: &Geometry, u: &[Vector3]) -> Vec<Vector3> {
        let n = g.n_cells();
        let ni = g.n_internal();
        let m = &g.mesh;
        let mut out = vec![Vector3::ZERO; n];
        for k in 0..3 {
            let x: Vec<f64> = u.iter().map(|v| cmp(*v, k)).collect();
            // boundaryDiagCmpt = -sum(internalCoeffs_k) + sum(cmptAv(internalCoeffs))
            let mut bd = vec![0.0; n];
            for f in ni..g.n_faces() {
                if g.active[f] {
                    bd[m.owner[f]] += self.ic[f - ni][k];
                }
            }
            for v in &mut bd {
                *v = -*v;
            }
            for f in ni..g.n_faces() {
                if g.active[f] {
                    let ic = self.ic[f - ni];
                    bd[m.owner[f]] += (ic[0] + ic[1] + ic[2]) / 3.0;
                }
            }
            let mut ldu_h = vec![0.0; n];
            for f in 0..ni {
                let (o, nb) = (m.owner[f], m.neighbour[f]);
                ldu_h[nb] -= self.eqn.ldu.lower[f] * x[o];
                ldu_h[o] -= self.eqn.ldu.upper[f] * x[nb];
            }
            let mut h: Vec<f64> = (0..n).map(|c| bd[c] * x[c]).collect();
            for c in 0..n {
                h[c] += ldu_h[c] + self.src[k][c];
            }
            for f in ni..g.n_faces() {
                if g.active[f] {
                    h[m.owner[f]] += self.bcoef[f - ni][k];
                }
            }
            for c in 0..n {
                h[c] /= m.cell_volumes[c];
                match k {
                    0 => out[c].x = h[c],
                    1 => out[c].y = h[c],
                    _ => out[c].z = h[c],
                }
            }
        }
        out
    }
}

/// Read the `solveFluidMechanics true` part of a case (boundary conditions, solver entries, pump and drag per cell).
///
/// # Errors
/// [`AppBuilderError::Case`] for every selection that is not ported (see the module documentation).
#[cfg(feature = "std")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn read_flow_case(
    ctx: &Ctx,
    g: &Geometry,
    tdir: &str,
    pimple: &FoamDict,
    fs: &FoamDict,
    pp: &FoamDict,
    momentum_source: Vec<Vector3>,
    drag: Vec<Option<CellDrag>>,
    gamg: super::GamgPolicy,
    laminar: bool,
) -> Result<OnePhaseFlowCase, AppBuilderError> {
    let n = g.n_cells();
    let ni = g.n_internal();
    let mode = word(pimple, "momentumMode").unwrap_or_else(|| "cellCentered".to_string());
    if mode != "cellCentered" && mode != "cellCenteredFaceReconstruction" {
        return Err(ctx.e(format!("PIMPLE.momentumMode `{mode}` is not ported (cellCentered and cellCenteredFaceReconstruction only)")));
    }
    // `thermalHydraulicsModel.C:202`: `porous_sharp`, default 0; read in every mode, used by `reconstructU_1p.H` only
    let porous_sharp = scalar(pimple, "porousInterfaceSharpness").unwrap_or(0.0);
    if let Some(pe) = word(pimple, "partialEliminationMode") {
        if pe != "implicit" {
            return Err(ctx.e(format!("PIMPLE.partialEliminationMode `{pe}` is not ported")));
        }
    }
    // `uncorrected` Laplacian and snGrad are the only schemes `fv` implements: with a `corrected` one the extra pressure passes
    // would carry an explicit non-orthogonal correction (gh:#1866 refuses it by name).
    {
        let schemes = ctx.read("system/fluidRegion/fvSchemes")?;
        for (block, want) in [("laplacianSchemes", "uncorrected"), ("snGradSchemes", "uncorrected")] {
            let d = ctx.sub(&schemes, block)?;
            let words: Vec<String> = match d.get("default") {
                Some(FoamEntry::Word(w)) => vec![w.clone()],
                Some(FoamEntry::Tokens(vs)) => vs
                    .iter()
                    .filter_map(|v| match v {
                        FoamValue::Word(w) => Some(w.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            if !words.iter().any(|w| w == want) {
                return Err(ctx.e(format!("fvSchemes.{block}.default = {words:?} is not ported (only `uncorrected`: no explicit non-orthogonal correction)")));
            }
        }
    }
    for (key, what) in [("minKdFS", "minKdFS")] {
        if scalar(pimple, key).unwrap_or(0.0) != 0.0 {
            return Err(ctx.e(format!("PIMPLE.{what} != 0 is not ported")));
        }
    }
    let solvers = ctx.sub(fs, "solvers")?;
    let substituted = core::cell::Cell::new(false);
    let pcg_entry = |name: &str, fallback: Option<PRghPcg>| -> Result<PRghPcg, AppBuilderError> {
        match last_match(solvers, name) {
            Some(FoamEntry::SubDict(d)) => {
                let tolerance = match (scalar(d, "tolerance"), fallback) {
                    (Some(t), _) => t,
                    (None, Some(f)) => f.tolerance,
                    (None, None) => return Err(ctx.e(format!("solvers.{name}: no tolerance"))),
                };
                if fallback.is_none() {
                    let is_gamg = word(d, "solver").as_deref() == Some("GAMG");
                    if is_gamg && gamg == super::GamgPolicy::SubstitutePcgDic {
                        substituted.set(true);
                    } else if word(d, "solver").as_deref() != Some("PCG") || word(d, "preconditioner").as_deref() != Some("DIC") {
                        return Err(ctx.e(format!("solvers.{name}: only PCG with the DIC preconditioner is ported (GAMG: see GamgPolicy)")));
                    }
                }
                Ok(PRghPcg {
                    tolerance,
                    rel_tol: scalar(d, "relTol").unwrap_or(fallback.map_or(0.0, |f| f.rel_tol)),
                    min_iter: scalar(d, "minIter").map_or(fallback.map_or(0, |f| f.min_iter), |v| v as usize),
                    max_iter: scalar(d, "maxIter").map_or(fallback.map_or(1000, |f| f.max_iter), |v| v as usize),
                })
            }
            _ => Err(ctx.e(format!("fvSolution: no solver entry for `{name}`"))),
        }
    };
    let p_rgh = pcg_entry("p_rgh", None)?;
    let p_rgh_final = pcg_entry("p_rghFinal", Some(p_rgh)).unwrap_or(p_rgh);
    let patch_bc = |ff: &FieldFile, pi: usize| -> Result<(String, Option<FoamValue>), AppBuilderError> {
        let name = &g.mesh.patches[pi].name;
        let (_, ty, v) = ff
            .bcs
            .iter()
            .find(|(nm, _, _)| nm == name)
            .ok_or_else(|| ctx.e(format!("{}: no BC for patch `{name}`", ff.name)))?;
        Ok((ty.clone(), v.clone()))
    };
    let uf = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/U"))?;
    let mut u_bc = Vec::new();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind == PatchKind::Empty {
            u_bc.push(VectorBc::ZeroGradient);
            continue;
        }
        let (ty, v) = patch_bc(&uf, pi)?;
        u_bc.push(match (ty.as_str(), p.kind) {
            ("wedge", PatchKind::Wedge) => fv::wedge_velocity_bc(g, pi),
            ("zeroGradient", _) => VectorBc::ZeroGradient,
            ("slip", _) => VectorBc::Slip,
            ("fixedValue", _) => {
                let v = v.as_ref().and_then(FoamValue::as_vector3).ok_or_else(|| ctx.e("U fixedValue needs a uniform vector"))?;
                VectorBc::Fixed(vec![v; p.size])
            }
            // `noSlipFvPatchVectorField` is `fixedValue` with `Zero` (the wall of the shipped 2D_FFTF vessel)
            ("noSlip", _) => VectorBc::Fixed(vec![Vector3::ZERO; p.size]),
            (other, _) => return Err(ctx.e(format!("U.{}: `{other}` is not ported for the solved flow (wedge, fixedValue, noSlip, zeroGradient, slip)", p.name))),
        });
    }
    let pf = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/p_rgh"))?;
    let p0 = pf.internal.as_scalar().ok_or_else(|| ctx.e("p_rgh internalField must be a uniform scalar"))?;
    let mut p_rgh_bc = Vec::new();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind == PatchKind::Empty {
            p_rgh_bc.push(Bc::Empty);
            continue;
        }
        let (ty, pv) = patch_bc(&pf, pi)?;
        p_rgh_bc.push(match (ty.as_str(), p.kind) {
            ("wedge", PatchKind::Wedge) => Bc::ZeroGradient,
            ("fixedFluxPressure", _) => Bc::FixedGradient(vec![0.0; p.size]),
            ("fixedValue", _) => {
                let v = pv.as_ref().and_then(FoamValue::as_scalar).ok_or_else(|| ctx.e(format!("p_rgh.{}: fixedValue needs a uniform value", p.name)))?;
                Bc::Fixed(vec![v; p.size])
            }
            (other, _) => return Err(ctx.e(format!("p_rgh.{}: `{other}` is not ported (fixedFluxPressure, fixedValue, wedge)", p.name))),
        });
    }
    let tf = read_field_bcs_only(ctx, &alloc::format!("{tdir}/fluidRegion/T"))?;
    let mut t_bc = Vec::new();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind == PatchKind::Empty {
            t_bc.push(Bc::Empty);
            continue;
        }
        let (ty, tv) = patch_bc(&tf, pi)?;
        t_bc.push(match (ty.as_str(), p.kind) {
            ("wedge", PatchKind::Wedge) | ("zeroGradient", _) => Bc::ZeroGradient,
            ("fixedValue", _) => {
                let v = tv.as_ref().and_then(FoamValue::as_scalar).ok_or_else(|| ctx.e(format!("T.{}: fixedValue needs a uniform value", p.name)))?;
                Bc::Fixed(vec![v; p.size])
            }
            _ => return Err(ctx.e(format!("T.{}: `{ty}` is not ported for the solved flow (zeroGradient, fixedValue, wedge)", p.name))),
        });
    }
    // `laminar` (`Stokes`): `nut()` is a zero field and there is no `nut` file (gh:#1913 round 55)
    let (nut0, nf) = if laminar {
        (0.0, None)
    } else {
        let nf = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/nut"))?;
        (nf.internal.as_scalar().ok_or_else(|| ctx.e("nut internalField must be a uniform scalar"))?, Some(nf))
    };
    let mut nut_boundary = vec![0.0; g.n_faces() - ni];
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        let Some(nf) = nf.as_ref() else { break };
        let (ty, v) = patch_bc(&nf, pi)?;
        let val = match (ty.as_str(), p.kind) {
            ("wedge", PatchKind::Wedge) | ("empty", PatchKind::Empty) => nut0,
            // a `zeroGradient` patch is evaluated at construction: the owner value, the uniform internal field (gh:#1929)
            ("zeroGradient", _) => nut0,
            // with `turbulence false` no wall function ever runs: the patch keeps the value it was read with
            ("nutkWallFunction" | "calculated", _) => v
                .as_ref()
                .and_then(FoamValue::as_scalar)
                .ok_or_else(|| ctx.e(format!("nut.{}: a `value uniform` is needed", p.name)))?,
            (other, _) => return Err(ctx.e(format!("nut.{}: `{other}` is not ported", p.name))),
        };
        for k in 0..p.size {
            nut_boundary[p.start + k - ni] = val;
        }
    }
    let _ = pp.get("pMin").ok_or_else(|| ctx.e("phaseProperties: pMin is missing"))?;
    Ok(OnePhaseFlowCase {
        n_correctors: scalar(pimple, "nCorrectors").unwrap_or(1.0) as usize,
        correct_until_convergence: bool_entry(pimple, "correctUntilConvergence").unwrap_or(false),
        p_rgh,
        p_rgh_final,
        p_rgh_gamg_substituted: substituted.get(),
        p_min: ctx.f64(pp, "pMin")?,
        p_ref_cell: scalar(pp, "pRefCell").unwrap_or(0.0) as usize,
        p_ref_value: scalar(pp, "pRefValue").unwrap_or(0.0),
        n_non_orth_correctors: scalar(pimple, "nNonOrthogonalCorrectors").unwrap_or(0.0) as usize,
        min_mag_u: scalar(pimple, "minMagU").unwrap_or(0.0),
        h_ref: 0.0,
        p_rgh_need_reference: !p_rgh_bc.iter().any(|b| matches!(b, Bc::Fixed(_))),
        t_bc,
        u_bc,
        p_rgh_bc,
        p_rgh0: vec![p0; n],
        nut: vec![nut0; n],
        nut_boundary,
        momentum_source,
        reconstruct_u: mode == "cellCenteredFaceReconstruction",
        porous_interface_sharpness: porous_sharp,
        momentum_profiles: Vec::new(),
        drag,
        turbulence: None,
    })
}

impl FlowState {
    /// The flow state at construction: `phi = fvc::flux(U)` (`fluid.C`), `phi_ = alphaPhi`, `gh`, `ghf`.
    pub(super) fn new(g: &Geometry, case: &super::OnePhaseCcCase, fc: &OnePhaseFlowCase, u: &Vector) -> Self {
        let n = g.n_cells();
        let nf = g.n_faces();
        let m = &g.mesh;
        let gr = case.gravity;
        let gmag = gr.mag();
        let gh: Vec<f64> = (0..n).map(|c| gr.dot(m.cell_centres[c]) + gmag * fc.h_ref).collect();
        let ghf: Vec<f64> = (0..nf).map(|f| gr.dot(m.face_centres[f]) + gmag * fc.h_ref).collect();
        let mut p_rgh = Scalar::uniform(g, 0.0, fc.p_rgh_bc.clone());
        p_rgh.c = fc.p_rgh0.clone();
        p_rgh.correct_bc(g);
        Self {
            u_old: u.c.clone(),
            p_rgh,
            phi: fv::flux_openfoam(g, u),
            phi_solver: case.alpha_phi.clone(),
            mag_u: u.c.iter().map(|v| v.mag()).collect(),
            kd: vec![[0.0; 3]; n],
            gh,
            ghf,
            solves: Vec::new(),
        }
    }
}

impl OnePhaseCcSolver {
    /// `onePhase::correctContErr`: `contErr = fvc::ddt(fluid_, rho) + fvc::div(alphaRhoPhi)` (no `fvOptions`); `ddt(alpha, rho)` is
    /// `rDeltaT (alpha rho - alpha.old rho.old)` and is exactly 0 for the constant density of both ported packages.
    pub(super) fn correct_cont_err(&mut self) {
        let rdt = 1.0 / self.dt;
        let dv = fv::div(&self.geom, &self.alpha_rho_phi);
        for c in 0..self.geom.n_cells() {
            self.cont_err[c] = rdt * (self.alpha[c] * self.rho[c] - self.alpha[c] * self.rho_old[c]) + dv[c];
        }
    }

    /// `FSPair::correct` drag part: the isotropic `ReynoldsPower` `Kd` per cell (`FSDragFactor::correctField`),
    /// from the Reynolds number just computed and the `magU` of the last pressure corrector.
    pub(super) fn correct_kd(&mut self) {
        let Some(fc) = self.case.flow.as_ref() else { return };
        let Some(fs) = self.flow.as_mut() else { return };
        // `myOps::storePrevIterIfRelax(Kd_)` ... `Kd_.relax()`: the `fields` entry `Kd` (`KdFinal` in the flagged iteration)
        let a = self.case.controls.relax.field("Kd", self.pimple.final_iteration);
        if a.is_some() {
            self.kd_prev = fs.kd.clone();
        }
        let min_mag_u = fc.min_mag_u;
        for c in 0..self.alpha.len() {
            if let Some(d) = fc.drag[c] {
                let dh = self.case.dh[c];
                let alphas = self.case.alpha_structure[c];
                let half = 0.5 / dh * (1.0 - alphas) * self.rho[c] * fs.mag_u[c].max(min_mag_u);
                if d.isotropic {
                    let law = d.laws[0].expect("an isotropic drag cell has its model");
                    let value = half * law.value(self.re[c]);
                    fs.kd[c] = [value, value, value];
                } else {
                    // `halfAlphaRhoMagUByDh_ * model->value(celli)` per valid axis; the other entries keep their value
                    for k in 0..3 {
                        if let Some(law) = d.laws[k] {
                            fs.kd[c][k] = half * law.value(self.lre[c][k]);
                        }
                    }
                }
            }
        }
        if let Some(a) = a {
            for c in 0..self.alpha.len() {
                for k in 0..3 {
                    fs.kd[c][k] = self.kd_prev[c][k] + a * (fs.kd[c][k] - self.kd_prev[c][k]);
                }
            }
        }
    }

    /// `onePhase::correctFluidMechanics()`: `UEqn_1p.H` then `pEqn_1p.H`, then `correctContErr()`.
    /// `p_rgh` is solved with `p_rghFinal` at the last corrector of the outer iteration (`finalInnerIter()`, every outer iteration
    /// unless `finalOnLastPimpleIterOnly`); `UEqn.relax()` and the turbulence use `isFinalIteration()` (gh:#1855).
    #[allow(clippy::too_many_lines)]
    pub(super) fn correct_fluid_mechanics(&mut self, outer: usize) {
        let mut fs = self.flow.take().expect("solveFluidMechanics true needs the flow state");
        let g = self.geom.clone();
        let fc = self.case.flow.clone().expect("flow case");
        let n = g.n_cells();
        let ni = g.n_internal();
        let nf = g.n_faces();
        let nb = nf - ni;
        let m = g.mesh.clone();
        let boussinesq = self.case.thermo.boussinesq_rho0();
        let rho = self.rho.clone();
        let rho_b = self.rho_b.clone();
        let th = self.case.thermo.clone();
        let rdt = 1.0 / self.dt;
        let npatch = m.patches.len();

        // ---- UEqn_1p.H ---------------------------------------------------------------------------------------
        // nuEff = nut + nu, nu = mu/rho(T) of the thermo; gamma = alpha rho nuEff (boundary: the patch values)
        // fluid_.turbulence().correct() FIRST (`porousKEpsilon::correct`), then `nuEff = nut + mu/rho0`
        self.correct_turbulence(&fs.phi, &fs.mag_u);
        let (nut_c, nut_b) = match self.turb.as_ref() {
            Some(t) => (t.nut.c.clone(), t.nut.b.clone()),
            None => (fc.nut.clone(), fc.nut_boundary.clone()),
        };
        let mut arn = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
        // the boundary temperature (`fixedValue` or the owner value) for `nu_b = mu_b/rho_b` of the thermo
        let mut t_s = Scalar { c: self.t.clone(), b: vec![0.0; nb], bc: self.t_patch_bcs() };
        t_s.correct_bc(&g);
        for c in 0..n {
            let pp = th.props(self.p[c], self.t[c]);
            let nu = pp.mu / pp.rho;
            arn.c[c] = self.alpha[c] * rho[c] * (nut_c[c] + nu);
        }
        for i in 0..nb {
            let o = m.owner[ni + i];
            let pp = th.props(self.p[o], t_s.b[i]);
            let nu = pp.mu / pp.rho;
            arn.b[i] = self.alpha[o] * rho_b[i] * (nut_b[i] + nu);
        }
        let gamma = fv::interp_linear_openfoam(&g, &arn);
        let zg = Scalar::uniform(&g, 0.0, g.default_bcs());
        let mut e = UEqnCc::new(&g);
        e.eqn.div_upwind_internal(&g, &self.alpha_rho_phi);
        e.boundary_convection(&g, &self.alpha_rho_phi, &self.u);
        let ce: Vec<f64> = self.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &self.u.c);
        let dev2 = if let Some(td) = self.case.tortuosity.as_ref().filter(|_| !self.ctl_no_tort_momentum) {
            // `UEqn_1p.H:27-40`: `- fvm::laplacian(alpha rho nuEff tortuosity, U) - fvc::div(alpha rho nuEff tortuosity & dev2(T(grad U)))`
            // (gh:#1229, `tensor_laplacian`)
            let (tc, tb) = tensor_laplacian::tortuosity_fields(&g, td).expect("validated when the case was read");
            let gc: Vec<_> = (0..n).map(|c| tensor_laplacian::scale(arn.c[c], &tc[c])).collect();
            let gb: Vec<_> = (0..nb).map(|i| tensor_laplacian::scale(arn.b[i], &tb[i])).collect();
            let gf = tensor_laplacian::interpolate(&g, &gc, &gb);
            let tg = tensor_laplacian::decompose(&g, &gf);
            tensor_laplacian::add_neg_laplacian_vector(&mut e, &g, &tg, &self.u);
            dev2_div_tortuous(&g, &self.u, &arn, (&tc, &tb))
        } else {
            e.eqn.neg_laplacian(&g, &gamma, &zg);
            e.boundary_laplacian(&g, &gamma, &self.u);
            dev2_div(&g, &self.u, &arn)
        };
        e.sub_field(&g, &dev2);
        // fvm::Sp((1/3) tr(Kd), U) + (dev(Kd) & U); Kd = diag(v, v, v)
        let mut sp = vec![0.0; n];
        let mut dev_u = vec![Vector3::ZERO; n];
        for c in 0..n {
            let [kx, ky, kz] = fs.kd[c];
            let tr = kx + ky + kz;
            sp[c] = (1.0 / 3.0) * tr;
            let third = (1.0 / 3.0) * tr;
            dev_u[c] = Vector3::new((kx - third) * self.u.c[c].x, (ky - third) * self.u.c[c].y, (kz - third) * self.u.c[c].z);
        }
        e.eqn.sp(&g, &sp);
        e.add_field(&g, &dev_u);
        // UEqn += fvm::ddt(alpha, rho, U)
        for c in 0..n {
            let v = m.cell_volumes[c];
            e.eqn.ldu.diag[c] += (rdt * self.alpha[c] * rho[c]) * v;
            let uo = fs.u_old[c];
            let ro = self.rho_old[c];
            e.src[0][c] += (rdt * self.alpha[c] * ro * uo.x) * v;
            e.src[1][c] += (rdt * self.alpha[c] * ro * uo.y) * v;
            e.src[2][c] += (rdt * self.alpha[c] * ro * uo.z) * v;
        }
        if let Some(fct) = self.case.controls.relax.equation("U", self.pimple.final_iteration).filter(|f| *f > 0.0) {
            e.relax(&g, fct, &self.u.c);
        }

        // ---- pEqn_1p.H ---------------------------------------------------------------------------------------
        // updateAlphaPhiAndInitControls_1p.H: alphaf, alphaPhi, alphaRhoPhi, contErr from the CURRENT phi
        let alpha_s = {
            let mut s = Scalar { c: self.alpha.clone(), b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
            for i in 0..nb {
                s.b[i] = self.alpha[m.owner[ni + i]];
            }
            s
        };
        let alphaf = fv::interp_linear_openfoam(&g, &alpha_s);
        // `fluid_.alphaRhoPhi() = fvc::interpolate(rho)*fluid_.alphaPhi()`: `rho0 alphaPhi` for the uniform `rho0` of a Boussinesq fluid
        let rho_s = Scalar { c: rho.clone(), b: rho_b.clone(), bc: vec![Bc::Calculated; npatch] };
        let rho_f = fv::interp_linear_openfoam(&g, &rho_s);
        for f in 0..nf {
            if g.active[f] {
                self.alpha_phi[f] = alphaf[f] * fs.phi[f];
                self.alpha_rho_phi[f] = match boussinesq {
                    Some(rho0) => rho0 * self.alpha_phi[f],
                    None => rho_f[f] * self.alpha_phi[f],
                };
            }
        }
        self.correct_cont_err();
        // rAU = 1/A, alpharAUf = interpolate(alpha rAU)
        let a_cell = e.a(&g);
        let rau: Vec<f64> = a_cell.iter().map(|a| 1.0 / a).collect();
        let mut ar = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
        for c in 0..n {
            ar.c[c] = self.alpha[c] * rau[c];
        }
        for i in 0..nb {
            let o = m.owner[ni + i];
            ar.b[i] = self.alpha[o] * rau[o];
        }
        let alpharauf = fv::interp_linear_openfoam(&g, &ar);

        let mut converged_piso = false;
        let mut corr_piso = 0usize;
        loop {
            // customPimpleControl::correct(bool interrupt)
            corr_piso += 1;
            let interrupt = fc.correct_until_convergence && converged_piso;
            if !(corr_piso <= fc.n_correctors && !interrupt) {
                break;
            }
            for c in 0..n {
                fs.p_rgh.c[c] = self.p[c] - rho[c] * fs.gh[c];
            }
            // correctBoundaryFlux_1p.H
            for f in ni..nf {
                if g.active[f] && self.u.flux_is_fixed(&g, f) {
                    fs.phi[f] = m.face_area_vectors[f].dot(self.u.b[f - ni]);
                }
            }
            // HbyA = rAU UEqn.H(); the temporary field carries the constraint patches
            let h = e.h(&g, &self.u.c);
            let mut hv = Vector::uniform_with_constraint_patches(&g, Vector3::ZERO);
            hv.c = h;
            hv.correct_bc(&g);
            let mut hbya = Vector::uniform_with_constraint_patches(&g, Vector3::ZERO);
            for c in 0..n {
                hbya.c[c] = hv.c[c] * rau[c];
            }
            for i in 0..nb {
                hbya.b[i] = hv.b[i] * rau[m.owner[ni + i]];
            }
            // calcPhiSbyA_1p.H: -alpharAUf ghf snGrad(rho(true)) magSf + alpharAUf flux(momentumSource)
            let mut rho_t = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
            // `fluid_.rho(true)`: the thermo's `rho(T)` (Boussinesq: not `rho0`); any other package: `fluid::rho()` itself
            for c in 0..n {
                rho_t.c[c] = match &th {
                    PhaseThermo::BoussinesqHConst(b) => b.rho(self.t[c]),
                    _ => rho[c],
                };
            }
            for i in 0..nb {
                rho_t.b[i] = match &th {
                    PhaseThermo::BoussinesqHConst(b) => b.rho(self.t[m.owner[ni + i]]),
                    _ => rho_b[i],
                };
            }
            let sng = fv::sn_grad(&g, &rho_t);
            let mut ms = Vector::uniform_with_constraint_patches(&g, Vector3::ZERO);
            ms.c = fc.momentum_source.clone();
            // `pump::correct`: `pumpValue_ * momentumSourceTimeProfile_.value(timeOutputValue())` on the zone's cells
            for (cells, src, profile) in &fc.momentum_profiles {
                let f = profile.value(uom::si::f64::Time::new::<uom::si::time::second>(self.time));
                let v = Vector3::new(src.x * f, src.y * f, src.z * f);
                for &c in cells {
                    ms.c[c] = v;
                }
            }
            ms.correct_bc(&g);
            let ms_flux = fv::flux_openfoam(&g, &ms);
            let mut phi_s = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_s[f] = -alpharauf[f] * fs.ghf[f] * sng[f] * m.face_areas[f];
                    phi_s[f] += alpharauf[f] * ms_flux[f];
                }
            }
            let hbya_flux = fv::flux_openfoam(&g, &hbya);
            let mut phi_hbya = vec![0.0; nf];
            let mut rauf = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_hbya[f] = alphaf[f] * (hbya_flux[f] + phi_s[f]);
                    rauf[f] = (alphaf[f] * alpharauf[f]).abs();
                }
            }
            // the fixedFluxPressure gradient from the boundary flux mismatch
            for (pi, patch) in m.patches.iter().enumerate() {
                if let Bc::FixedGradient(gr) = &mut fs.p_rgh.bc[pi] {
                    for k in 0..patch.size {
                        let f = patch.start + k;
                        gr[k] = (phi_hbya[f] - alphaf[f] * fs.phi[f]) / (m.face_areas[f] * rauf[f]);
                    }
                }
            }
            // pEqnComp + pEqnIncomp. With the `uncorrected` Laplacian (the only scheme read, see `read_flow_case`) the matrix
            // and source of every non-orthogonal pass are the same, so they are built once; only `p_rgh` differs between passes.
            let dap = fv::div(&g, &self.alpha_phi);
            let mut peqn = Eqn::new(&g);
            peqn.neg_laplacian(&g, &rauf, &fs.p_rgh);
            let dphi = fv::div(&g, &phi_hbya);
            for c in 0..n {
                let v = m.cell_volumes[c];
                let comp = (self.cont_err[c] - (0.0 + dap[c]) * rho[c]) / rho[c];
                peqn.source[c] -= v * comp;
                peqn.source[c] -= v * dphi[c];
            }
            // setReference(pRefCell, pRefValue): `source += diag value; diag += diag` (psi = 0: `compressible` is false, so the
            // value is `pRefValue`; every patch is `fixedFluxPressure` or wedge, so `needReference()` is true)
            if fc.p_rgh_need_reference {
                let rc = fc.p_ref_cell;
                peqn.source[rc] += peqn.ldu.diag[rc] * fc.p_ref_value;
                peqn.ldu.diag[rc] += peqn.ldu.diag[rc];
            }
            // pEqn_1p.H: `while (pimple_.correctNonOrthogonal())`, solveP_rgh_1p.H in each pass
            let n_pass = fc.n_non_orth_correctors + 1;
            let mut p_flux_final = Vec::new();
            for pass in 1..=n_pass {
                let solver = if self.pimple.final_inner_iter(corr_piso, pass, &self.case.controls.pimple) { fc.p_rgh_final } else { fc.p_rgh };
                let mut x = fs.p_rgh.c.clone();
                let perf =
                    pcg_dic(&peqn.ldu, &peqn.source, &mut x, solver.tolerance, solver.rel_tol, solver.max_iter, solver.min_iter);
                fs.solves.push(PressureSolve {
                    outer,
                    corrector: corr_piso,
                    non_orth: pass,
                    initial_residual: perf.initial_residual,
                    iterations: perf.iterations,
                });
                // `if (pimple_.corrNonOrtho() == 1) convergedPISO = converged`
                if pass == 1 {
                    converged_piso = perf.iterations == 0;
                }
                self.pimple.record("p_rgh", perf.initial_residual);
                if pass == 1 || !self.first_piso_first_pass_only {
                    self.pimple.update_first_piso_prev_pimple(corr_piso);
                }
                fs.p_rgh.c = x;
                fs.p_rgh.correct_bc(&g);
                if pass == n_pass {
                    // phi_ = phiHbyA + pEqnIncomp.flux(); phi = phi_/alphaf (a fixedValue flux patch ignores the assignment)
                    p_flux_final = Eqn::laplacian_flux(&g, &rauf, &fs.p_rgh);
                }
            }
            let mut p_flux = p_flux_final;
            for f in 0..nf {
                if !g.active[f] {
                    continue;
                }
                fs.phi_solver[f] = phi_hbya[f] + p_flux[f];
                if f >= ni && self.u.flux_is_fixed(&g, f) {
                    continue;
                }
                fs.phi[f] = fs.phi_solver[f] / alphaf[f];
            }
            // `p_rgh_.relax()` sits between `phi_ = phiHbyA + pEqnIncomp.flux()` (unrelaxed `p_rgh`) and `U = HbyA + reconstruct(... pEqnIncomp.flux() ...)`,
            // whose `flux()` is evaluated again, on the RELAXED `p_rgh` (`fvMatrix::flux` reads `psi_` when called). `fields` entry `p_rgh` (`p_rghFinal` when final).
            if let Some(a) = self.case.controls.relax.field("p_rgh", self.pimple.final_iteration) {
                for c in 0..n {
                    fs.p_rgh.c[c] = self.p_rgh_prev[c] + a * (fs.p_rgh.c[c] - self.p_rgh_prev[c]);
                }
                // the patch values follow the cells (`zeroGradient`/`wedge`: no flux through them; `fixedGradient` keeps its gradient)
                fs.p_rgh.correct_bc(&g);
                p_flux = Eqn::laplacian_flux(&g, &rauf, &fs.p_rgh);
            }
            // U = HbyA + reconstruct(alpharAUf pEqnIncomp.flux()/rAUf + phiSbyA)
            let mut rec_src = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    rec_src[f] = alpharauf[f] * p_flux[f] / rauf[f] + phi_s[f];
                }
            }
            if fc.reconstruct_u {
                // reconstructU_1p.H: `phi_` is `fs.phi_solver` (phiHbyA + pEqnIncomp.flux(), unrelaxed), `fluid_.phi()` is `fs.phi`
                let sharp = fc.porous_interface_sharpness;
                let u_new = reconstruct_u_1p(&g, &fs.phi_solver, &fs.phi, &self.alpha, sharp);
                for c in 0..n {
                    self.u.c[c] = u_new[c];
                }
            } else {
                let rec = fv::reconstruct(&g, &rec_src);
                for c in 0..n {
                    self.u.c[c] = hbya.c[c] + rec[c];
                }
            }
            self.u.correct_bc(&g);
            for c in 0..n {
                fs.mag_u[c] = self.u.c[c].mag();
            }
            // updatePP_rghRho_1p.H (Boussinesq: psi = 0, the closed-volume mass correction is skipped)
            for c in 0..n {
                self.p[c] = (fs.p_rgh.c[c] + rho[c] * fs.gh[c]).max(fc.p_min);
            }
            for c in 0..n {
                self.p[c] = self.p[c].max(fc.p_min);
                fs.p_rgh.c[c] = self.p[c] - rho[c] * fs.gh[c];
            }
            fs.p_rgh.correct_bc(&g);
        }
        // correctContErr(): alphaRhoPhi has not changed since the start of pEqn; `rho` has not either (`psi = 0`: the update
        // `rho += psi (p_rgh - p_rgh_0)` adds exactly 0), so contErr is unchanged
        self.flow = Some(fs);
        self.correct_cont_err();
    }
}

/// `reconstructU_1p.H` (upstream `onePhase/include/equations/pEqnRoutines/reconstructU_1p.H`, GeN-Foam 652b3da): the velocity of
/// `momentumMode cellCenteredFaceReconstruction` from the face fluxes, with `s = porous_sharp`:
/// `s != 0`: `s == 1`: `reconstruct(phi_)/alpha`; else `s*(reconstruct(phi_)/alpha) + (1 - s)*reconstruct(phi)`; `s == 0`: `reconstruct(phi)`.
/// `phi_` is the superficial flux (`alpha phi`), `phi` the interstitial one (`phi_/alphaf`). Uses the OpenFOAM-order
/// [`fv::reconstruct_openfoam`].
pub(super) fn reconstruct_u_1p(g: &Geometry, phi_super: &[f64], phi: &[f64], alpha: &[f64], sharpness: f64) -> Vec<Vector3> {
    let n = g.n_cells();
    if sharpness == 0.0 {
        return fv::reconstruct_openfoam(g, phi);
    }
    let rs = fv::reconstruct_openfoam(g, phi_super);
    let div: Vec<Vector3> = (0..n).map(|c| Vector3::new(rs[c].x / alpha[c], rs[c].y / alpha[c], rs[c].z / alpha[c])).collect();
    if sharpness == 1.0 {
        return div;
    }
    let ri = fv::reconstruct_openfoam(g, phi);
    let w = 1.0 - sharpness;
    (0..n)
        .map(|c| Vector3::new(sharpness * div[c].x + w * ri[c].x, sharpness * div[c].y + w * ri[c].y, sharpness * div[c].z + w * ri[c].z))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reynolds_power_is_a_power_law_plus_a_constant() {
        let d = ReynoldsPowerDrag { coeff: 0.5, exp: -0.25, constant: 0.1 };
        let re = 16.0;
        assert!((d.value(re) - (0.5 * 0.5 + 0.1)).abs() < 1e-15);
    }
}
