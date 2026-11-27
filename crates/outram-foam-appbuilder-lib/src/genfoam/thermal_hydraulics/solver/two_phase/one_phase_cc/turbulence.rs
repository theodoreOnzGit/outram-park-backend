// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/porousKEpsilon/
//     porousKEpsilon.{C,H} (`correct`, `correctNut`, constructor), the call site
//     src/classes/thermalHydraulics/solvers/onePhase/include/equations/UEqn_1p.H, and
//     src/phaseModels/fluid/fluid.C (`constructTurbulenceModel`: the model gets `rho()` = `rho0` under Boussinesq)
//   and the OpenFOAM-ESI v2506 classes it builds on: TurbulenceModels/turbulenceModels/derivedFvPatchFields/
//     wallFunctions/{epsilonWallFunctions/epsilonWallFunction, nutWallFunctions/nutkWallFunction, kqRWallFunctions,
//     wallFunction/wallFunctionCoefficients, wallFunction/wallFunctionBlenders},
//     finiteVolume/fvMesh/wallDist/nearWallDist/nearWallDist.C, meshTools/cellDist/cellDistFuncs{,Templates}.C,
//     finiteVolume/fvMatrices/fvMatrix/fvMatrix.C (`relax`, `setValuesFromList`), finiteVolume/cfdTools/general/bound/bound.C
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506 (C) OpenCFD Ltd
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `one_phase_cc::turbulence` — single-phase `porousKEpsilon::correct()` with wall functions (gh:#1838, gh:#1839)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! `UEqn_1p.H` begins with `fluid_.turbulence().correct()`: per PIMPLE outer iteration, `epsilon` then `k` are solved,
//! `nut = Cmu k^2/epsilon`, `alphat = rho0 nut/Prt`. The order inside `correct()` (read from the source, not tuned):
//!
//! 1. `divU = div(phi)`, `G = nut (dev(twoSymm(grad U)) && grad U)` on the cells;
//! 2. `epsilon.boundaryFieldRef().updateCoeffs()`: the `epsilonWallFunction` patches overwrite `G` AND `epsilon` in
//!    EVERY cell with a wall face (weights `1/nWallFaces(cell)`), with `nut` of the wall patches as they stand (the READ
//!    value `0.1` in the very first call: GeN-Foam never calls `validate()`), and set their patch value to the cell's `epsilon0`;
//! 3. the `epsilon` equation `ddt(alpha, rho, eps) + div(alphaRhoPhi, eps) - laplacian(alpha rho DepsilonEff, eps) ==
//!    C1 alpha rho G eps/k noStructure - SuSp((2/3 C1 - C3) alpha rho divU noStructure, eps) - Sp(C2 alpha rho eps/k noStructure, eps)
//!    - Sp(alphaRhoConv, eps) + alphaRhoConv equilibriumEpsilon`, `relax(0.9)`, `boundaryManipulate` (the wall cells are
//!    SET to their `epsilon` value: `fvMatrix::setValues`), `PBiCGStab`/`diagonal`, `bound`;
//! 4. the `k` equation (no manipulation), `bound`;
//! 5. `correctNut`: `nut = Cmu k^2/epsilon`, the wall patches re-evaluated by `nutkWallFunction`, `alphat = rho0 nut/Prt`.
//!
//! `noStructure = neg0(alpha.structure)`, `yesStructure = pos(alpha.structure)`, `alphaRhoConv = alpha rho |U|/convergenceLength
//! yesStructure`, `equilibriumK = 1.5 (|U| A Re^B)^2`, `equilibriumEpsilon = Cmu^0.75 equilibriumK^1.5/max(C Dh, SMALL)`.
//! `fvMatrix::relax` of the two equations and the `bound` function are the ones of the two-phase port
//! ([`super::super::turbulence`]); `porousKEpsilon2PhaseCorrected` is NOT this class (it is not derived from it).
//!
//! ## Old-time `epsilon`: the lazy `oldTime()` quirk is real but immaterial here
//!
//! OpenFOAM creates `epsilon.oldTime()` at the first `oldTime()` request, which in step 1 is `fvm::ddt`, AFTER `updateCoeffs`
//! has overwritten the wall-cell values. It makes no difference: the old-time value of a wall cell enters only that cell's row,
//! and `boundaryManipulate` replaces the row (measured: the step-1 `epsilon` is bit-identical with either old time), so
//! the old fields are simply the values at the start of the step.
//!
//! ## Ported in gh:#1842 (round 29), each differenced against the real binary on case variants
//!
//! * wall-function entries per patch: `Cmu`, `kappa`, `E` ([`WallFunctionCoeffs`]), `blending` and `n`
//!   ([`WallFunctionBlending`], [`WallBlender`]: `stepwise` default, `max`, `binomial`, `exponential`, `tanh`; `n` defaults 4 for
//!   `nutkWallFunction`, 2 for `epsilonWallFunction`), `lowReCorrection` ([`EpsilonWallSpec`]) and `U` ([`NutWallSpec`]; read by
//!   `nutkWallFunction::yPlus()` only, which `correct()` never calls: it changes nothing, measured);
//! * `DhStruct` / `laminarReStruct` nut stabilisation (`correctNut`: `alphat` from the UNstabilised `nut`, then
//!   `nut += pos(structure) magU DhStruct/laminarReStruct`, `magU` the stored `fluid_.magU()` of the last pressure corrector);
//! * `kFinal`/`epsilonFinal` solver and relaxation entries are selected only when `isFinalIteration()` is true: GeN-Foam's
//!   `customPimpleControl` declares its own `corr_`, shadowing the base class's, so `pimpleControl::finalIter()` is `converged_` only
//!   and `isFinalIteration()` is true only in the iteration `PIMPLE.residualControl` flagged (ported in gh:#1855, round 31:
//!   [`super::pimple`]); without `residualControl` the plain entries run in every outer iteration (measured, round 29: a `kFinal`
//!   with a non-existent solver does not stop the real binary). [`OnePhaseCcSolver::with_stock_final_iteration_selection`]
//!   is the stock-OpenFOAM behaviour, kept as a named CONTROL.
//!
//! ## Ported in gh:#1929 (round 65), differenced against the real binary on `w8_zg`, `w9_calc`, `w10_mapped`
//!
//! `porousKEpsilon::correct()` has no wall-function code of its own: the patch TYPES of `0/fluidRegion/{k,epsilon,nut,alphat}` decide.
//! The shipped `2D_FFTF` uses none: `k`/`epsilon`/`nut`/`alphat` `zeroGradient` on the walls (the epsilon equation then has no `updateCoeffs`
//! effect and no `boundaryManipulate`), `nut` and `alphat` `calculated` on the baffles. A `calculated` patch takes the values of the
//! assigned expression (`nut_b = Cmu k_b^2/epsilon_b`, plus `pos(structure) mag(U_b) DhStruct/laminarReStruct` with the stabilisation,
//! `alphat_b = rho nut_b/Prt`), a `zeroGradient` patch the owner value; the energy equation's `alphaEff` takes `alphat` on the patches
//! (`heThermo::alphaEff`: `alpha_ + alphat`), which the form before round 65 took as 0. `DhStruct` and `laminarReStruct` are `zeroGradient`
//! fields upstream (`porousKEpsilon.C:400-440`); `structure`'s own patch type was NOT read, its patch value is taken as the owner value.
//!
//! ## Refused by name
//!
//! `RASModel` other than `porousKEpsilon`; `k`/`epsilon` patch types other than the wall functions and `zeroGradient`; `nut` patch types
//! other than `nutkWallFunction`, `zeroGradient`, `calculated`; `alphat` other than `zeroGradient`, `calculated`; `alphat` `calculated`
//! beside a `nutkWallFunction`; any other entry on a wall-function patch (including every entry of `kqRWallFunction`, which upstream
//! ignores); a `k`/`epsilon` solver other than `PBiCGStab`/`diagonal` or `PBiCG`/`DILU`; `fvOptions`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::hash::HashMap;

use outram_foam_basic_lib::mesh::{FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use petir::mathf::RealMath;

use super::super::fv::{self, Bc, Eqn, Geometry, Scalar};
use super::super::pbicgstab::{pbicgstab_diagonal, DEFAULT_MAX_ITER};
use super::super::turbulence::{bound, convective_inflow2, production_invariant, relax_equation, velocity_gradient, SMALL};
use super::OnePhaseCcSolver;
#[cfg(feature = "std")]
use super::super::case::{read_field, scalar, word, Ctx, FieldFile};
#[cfg(feature = "std")]
use super::last_match;
#[cfg(feature = "std")]
use crate::error::AppBuilderError;
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry};

/// `ROOTVSMALL` of OpenFOAM (double precision).
const ROOT_VSMALL: f64 = 1.0e-150;

/// `wallFunctionCoefficients`: `Cmu 0.09`, `kappa 0.41`, `E 9.8`, `yPlusLam` by `calcYPlusLam`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFunctionCoeffs {
    /// `Cmu`.
    pub cmu: f64,
    /// von Karman constant.
    pub kappa: f64,
    /// Wall roughness parameter `E`.
    pub e: f64,
    /// `y+` of the intersection of the viscous and the inertial sublayer.
    pub y_plus_lam: f64,
}

impl WallFunctionCoeffs {
    /// The defaults (`wallFunctionCoefficients()`): `calcYPlusLam` iterates `ypl = ln(max(E ypl, 1))/kappa` ten times from 11.
    #[must_use]
    pub fn standard() -> Self {
        Self::new(0.09, 0.41, 9.8)
    }

    /// `wallFunctionCoefficients(dict)` with the three values already read: `calcYPlusLam(kappa, E)`
    /// (`wallFunctionCoefficients.C`). The caller applies the `kappa, E >= SMALL` check of the dictionary constructor.
    #[must_use]
    pub fn new(cmu: f64, kappa: f64, e: f64) -> Self {
        let mut ypl = 11.0_f64;
        for _ in 0..10 {
            ypl = (e * ypl).max(1.0).r_ln() / kappa;
        }
        Self { cmu, kappa, e, y_plus_lam: ypl }
    }

    /// `pow025(Cmu)`.
    #[must_use]
    pub fn cmu25(&self) -> f64 {
        self.cmu.sqrt().sqrt()
    }
}

/// `wallFunctionBlenders::blenderType` (ESI v2506 `wallFunctionBlenders.{H,C}`), the `blending` keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallFunctionBlending {
    /// `stepwise` (the default): the inertial value above `yPlusLam`, the viscous one below.
    Stepwise,
    /// `max`: the larger of the two (Popovac and Hanjalic 2007, Eq. 27).
    Max,
    /// `binomial`: `(vis^n + log^n)^(1/n)` (Menter and Esch 2001).
    Binomial,
    /// `exponential`: `vis exp(-Gamma) + log exp(-1/Gamma)` (Popovac and Hanjalic 2007).
    Exponential,
    /// `tanh`: `eta (vis + log) + (1 - eta) (vis^1.2 + log^1.2)^(1/1.2)` (Knopp et al. 2006).
    Tanh,
}

impl WallFunctionBlending {
    /// The keyword spelling (`blenderTypeNames`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Stepwise => "stepwise",
            Self::Max => "max",
            Self::Binomial => "binomial",
            Self::Exponential => "exponential",
            Self::Tanh => "tanh",
        }
    }

    /// Parse the keyword; `None` for a name `blenderTypeNames` does not hold.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Stepwise, Self::Max, Self::Binomial, Self::Exponential, Self::Tanh].into_iter().find(|b| b.name() == name)
    }
}

/// `sqr(sqr(s))`, OpenFOAM's `pow4`.
fn pow4(s: f64) -> f64 {
    let q = s * s;
    q * q
}

/// `wallFunctionBlenders`: the blender and the binomial exponent `n` of one wall patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallBlender {
    /// `blending`.
    pub blending: WallFunctionBlending,
    /// `n` (read for every blender, used by `binomial`).
    pub n: f64,
}

impl WallBlender {
    /// `nutkWallFunction`'s default: stepwise, `n = 4`.
    #[must_use]
    pub fn nut_default() -> Self {
        Self { blending: WallFunctionBlending::Stepwise, n: 4.0 }
    }

    /// `epsilonWallFunction`'s default: stepwise, `n = 2`.
    #[must_use]
    pub fn epsilon_default() -> Self {
        Self { blending: WallFunctionBlending::Stepwise, n: 2.0 }
    }

    /// `nutkWallFunction::calcNut` for one face (ESI v2506 `nutkWallFunctionFvPatchScalarField.C`): `nutw` for the viscosity `nu`
    /// of the face and `y+ = Cmu25 y sqrt(k)/nu`.
    #[must_use]
    pub fn nut_wall(&self, w: &WallFunctionCoeffs, y_plus: f64, nu: f64) -> f64 {
        let nut_log = nu * y_plus * w.kappa / (w.e * y_plus).max(1.0 + 1e-4).r_ln();
        let blended = match self.blending {
            WallFunctionBlending::Stepwise => {
                if y_plus > w.y_plus_lam {
                    nut_log
                } else {
                    nu
                }
            }
            WallFunctionBlending::Max => nu.max(nut_log),
            WallFunctionBlending::Binomial => (nu.r_powf(self.n) + nut_log.r_powf(self.n)).r_powf(1.0 / self.n),
            WallFunctionBlending::Exponential => {
                let gamma = 0.01 * pow4(y_plus) / (1.0 + 5.0 * y_plus);
                let inv_gamma = 1.0 / (gamma + ROOT_VSMALL);
                nu * (-gamma).r_exp() + nut_log * (-inv_gamma).r_exp()
            }
            WallFunctionBlending::Tanh => {
                let b1 = nu + nut_log;
                let b2 = (nu.r_powf(1.2) + nut_log.r_powf(1.2)).r_powf(1.0 / 1.2);
                let phi = pow4(0.1 * y_plus).r_tanh();
                phi * b1 + (1.0 - phi) * b2
            }
        };
        blended - nu
    }

    /// The contribution of one face to `epsilon0` before the cell weight (`epsilonWallFunction::calculate`, ESI v2506
    /// `epsilonWallFunctionFvPatchScalarField.C`): `eps_vis = 2 k nu/y^2`, `eps_log = Cmu75 k^1.5/(kappa y)`.
    #[must_use]
    pub fn epsilon_wall(&self, w: &WallFunctionCoeffs, low_re_correction: bool, y_plus: f64, eps_vis: f64, eps_log: f64) -> f64 {
        match self.blending {
            WallFunctionBlending::Stepwise => {
                if low_re_correction && y_plus < w.y_plus_lam {
                    eps_vis
                } else {
                    eps_log
                }
            }
            WallFunctionBlending::Binomial => (eps_vis.r_powf(self.n) + eps_log.r_powf(self.n)).r_powf(1.0 / self.n),
            WallFunctionBlending::Max => eps_vis.max(eps_log),
            WallFunctionBlending::Exponential => {
                let gamma = 0.001 * pow4(y_plus) / (1.0 + y_plus);
                let inv_gamma = 1.0 / (gamma + ROOT_VSMALL);
                eps_vis * (-gamma).r_exp() + eps_log * (-inv_gamma).r_exp()
            }
            WallFunctionBlending::Tanh => {
                let b1 = eps_vis + eps_log;
                let b2 = (eps_vis.r_powf(1.2) + eps_log.r_powf(1.2)).r_powf(1.0 / 1.2);
                let phi = pow4(0.1 * y_plus).r_tanh();
                phi * b1 + (1.0 - phi) * b2
            }
        }
    }
}

/// The entries of one `nutkWallFunction` patch.
#[derive(Debug, Clone, PartialEq)]
pub struct NutWallSpec {
    /// `Cmu`, `kappa`, `E`.
    pub coeffs: WallFunctionCoeffs,
    /// `blending`, `n`.
    pub blender: WallBlender,
    /// `U`: the velocity field `nutkWallFunction::yPlus()` reads (a diagnostic `correct()` never calls); `None` = the model's.
    pub u_name: Option<String>,
}

impl NutWallSpec {
    /// Every entry at its default.
    #[must_use]
    pub fn standard() -> Self {
        Self { coeffs: WallFunctionCoeffs::standard(), blender: WallBlender::nut_default(), u_name: None }
    }
}

/// The entries of one `epsilonWallFunction` patch.
#[derive(Debug, Clone, PartialEq)]
pub struct EpsilonWallSpec {
    /// `Cmu`, `kappa`, `E`.
    pub coeffs: WallFunctionCoeffs,
    /// `blending`, `n`.
    pub blender: WallBlender,
    /// `lowReCorrection` (default `false`).
    pub low_re_correction: bool,
}

impl EpsilonWallSpec {
    /// Every entry at its default.
    #[must_use]
    pub fn standard() -> Self {
        Self { coeffs: WallFunctionCoeffs::standard(), blender: WallBlender::epsilon_default(), low_re_correction: false }
    }
}

/// A linear-solver entry of `k` or `epsilon` (`PBiCGStab`, `diagonal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagonalBicgstab {
    /// `tolerance`.
    pub tolerance: f64,
    /// `relTol`.
    pub rel_tol: f64,
    /// `minIter`.
    pub min_iter: usize,
    /// `maxIter`.
    pub max_iter: usize,
    /// `solver PBiCG; preconditioner DILU;` instead of `PBiCGStab`/`diagonal` (the shipped `2D_FFTF`, gh:#1870 round 62; default false).
    pub dilu: bool,
}

impl DiagonalBicgstab {
    /// Solve `ldu x = source` from the guess in `x` with the selected solver; returns `(initial residual, iterations)`.
    pub fn solve(&self, ldu: &outram_foam_basic_lib::prelude::LduMatrix, source: &[f64], x: &mut [f64]) -> (f64, usize) {
        if self.dilu {
            let perf = crate::genfoam::thermo_mechanics::legacy_fvm::pbicg::pbicg_dilu(ldu, source, x, self.tolerance, self.rel_tol, self.max_iter, self.min_iter);
            (perf.initial_residual, perf.iterations)
        } else {
            let perf = pbicgstab_diagonal(ldu, source, x, self.tolerance, self.rel_tol, self.max_iter, self.min_iter);
            (perf.initial_residual, perf.iterations)
        }
    }
}

/// The `turbulence true` part of a read case.
#[derive(Debug, Clone)]
pub struct OnePhaseTurbulenceCase {
    /// `Cmu` (0.09).
    pub cmu: f64,
    /// `C1` (1.44).
    pub c1: f64,
    /// `C2` (1.92).
    pub c2: f64,
    /// `C3` (0).
    pub c3: f64,
    /// `sigmak` (1).
    pub sigmak: f64,
    /// `sigmaEps` (1.3).
    pub sigma_eps: f64,
    /// `Prt` (1).
    pub prt: f64,
    /// `kMin`.
    pub k_min: f64,
    /// `epsilonMin`.
    pub epsilon_min: f64,
    /// `k` at the start time (cells).
    pub k: Vec<f64>,
    /// `epsilon` at the start time (cells).
    pub epsilon: Vec<f64>,
    /// `convergenceLength` per cell (`1e69` outside the porous regions, then `max(., SMALL)`).
    pub convergence_length: Vec<f64>,
    /// `turbulenceIntensityCoeff` per cell.
    pub turbulence_intensity_coeff: Vec<f64>,
    /// `turbulenceIntensityExp` per cell.
    pub turbulence_intensity_exp: Vec<f64>,
    /// `turbulenceLengthScaleCoeff` per cell.
    pub turbulence_length_scale_coeff: Vec<f64>,
    /// Which patches carry the three wall functions.
    pub wall_patch: Vec<bool>,
    /// Which patches carry a `calculated` `nut` (gh:#1929: the patch takes the values of the assigned expression, not the owner value).
    pub nut_calculated: Vec<bool>,
    /// Which patches carry a `calculated` `alphat` (gh:#1929).
    pub alphat_calculated: Vec<bool>,
    /// `y` (`nearWallDist`) of every boundary face (`face - n_internal`), 0 off the walls.
    pub wall_y: Vec<f64>,
    /// The `nutkWallFunction` entries of every patch (defaults off the walls).
    pub nut_wall: Vec<NutWallSpec>,
    /// The `epsilonWallFunction` entries of every patch (defaults off the walls).
    pub epsilon_wall: Vec<EpsilonWallSpec>,
    /// `nutStabilization_`: some `porousKEpsilonProperties` region carries `DhStruct`.
    pub nut_stabilization: bool,
    /// `DhStructPtr_` per cell (0 outside the regions that carry `DhStruct`).
    pub dh_struct: Vec<f64>,
    /// `laminarReStructPtr_` per cell (500 outside the regions that set it).
    pub laminar_re_struct: Vec<f64>,
    /// The `k` linear solver (`k`).
    pub k_solver: DiagonalBicgstab,
    /// The `epsilon` linear solver (`epsilon`).
    pub epsilon_solver: DiagonalBicgstab,
    /// The `k` linear solver of the final PIMPLE iteration (`kFinal`).
    pub k_solver_final: DiagonalBicgstab,
    /// The `epsilon` linear solver of the final PIMPLE iteration (`epsilonFinal`).
    pub epsilon_solver_final: DiagonalBicgstab,
}

/// The mutable state of the model.
#[derive(Debug, Clone)]
pub(super) struct TurbState {
    pub case: OnePhaseTurbulenceCase,
    pub k: Scalar,
    pub epsilon: Scalar,
    pub nut: Scalar,
    pub alphat: Scalar,
    pub k_old: Vec<f64>,
    pub epsilon_old: Vec<f64>,
    /// `1/(number of wall faces of the cell)` for cells with a wall face, else 0.
    pub cell_weight: Vec<f64>,
    /// Apply the wall functions (default true; the ablation switches them off).
    pub wall_functions: bool,
    /// `G` of the last `correct()` (diagnostic).
    pub production: Vec<f64>,
    /// `bound` actions on `k` and `epsilon`.
    pub bound_count: usize,
    /// Initial residual and iterations of the `epsilon` and `k` solves of each outer iteration of the current step.
    pub step_solves: Vec<[(f64, usize); 2]>,
}

/// Minimum distance from `p` to the triangle `(a, b, c)` (Ericson, "Real-Time Collision Detection", 5.1.5).
#[must_use]
pub fn point_triangle_distance(p: Vector3, a: Vector3, b: Vector3, c: Vector3) -> f64 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (p - a).mag();
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (p - b).mag();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (p - (a + ab * v)).mag();
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (p - c).mag();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (p - (a + ac * w)).mag();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (p - (b + (c - b) * w)).mag();
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (p - (a + ab * v + ac * w)).mag()
}

/// `face::nearestPoint` distance: a triangle directly, any other face as the fan `(p_i, p_i+1, centre)`.
fn point_face_distance(p: Vector3, pts: &[Vector3], centre: Vector3) -> f64 {
    if pts.len() == 3 {
        return point_triangle_distance(p, pts[0], pts[1], pts[2]);
    }
    let mut best = f64::MAX;
    for i in 0..pts.len() {
        let d = point_triangle_distance(p, pts[i], pts[(i + 1) % pts.len()], centre);
        if d < best {
            best = d;
        }
    }
    best
}

/// ESI `nearWallDist` (`useCombinedWallPatch true`): for each wall face, the smallest distance from the centre of its cell to
/// any wall face that shares a point with it (itself included). Returns one value per boundary face, 0 off the walls.
#[must_use]
pub fn near_wall_dist(mesh: &FvMesh, points: &[[f64; 3]], faces: &[Vec<usize>]) -> Vec<f64> {
    let ni = mesh.n_internal_faces;
    let mut out = vec![0.0; mesh.n_faces - ni];
    let mut wall_faces: Vec<usize> = Vec::new();
    for p in &mesh.patches {
        if p.kind == PatchKind::Wall {
            wall_faces.extend(p.start..p.start + p.size);
        }
    }
    let mut point_faces: HashMap<usize, Vec<usize>> = HashMap::default();
    for (i, &f) in wall_faces.iter().enumerate() {
        for &pt in &faces[f] {
            point_faces.entry(pt).or_default().push(i);
        }
    }
    let to_v = |i: usize| Vector3::new(points[i][0], points[i][1], points[i][2]);
    for &f in &wall_faces {
        let cc = mesh.cell_centres[mesh.owner[f]];
        let mut neighbours: Vec<usize> = Vec::new();
        for &pt in &faces[f] {
            for &i in &point_faces[&pt] {
                if !neighbours.contains(&i) {
                    neighbours.push(i);
                }
            }
        }
        let mut best = f64::MAX;
        for &i in &neighbours {
            let nf = wall_faces[i];
            let pts: Vec<Vector3> = faces[nf].iter().map(|&q| to_v(q)).collect();
            let d = point_face_distance(cc, &pts, mesh.face_centres[nf]);
            if d < best {
                best = d;
            }
        }
        out[f - ni] = best;
    }
    out
}

impl TurbState {
    /// The state at the start time. `nut_cells`/`nut_wall` are the read `nut` (cells; wall patch values by boundary face).
    pub(super) fn new(
        g: &Geometry,
        case: &OnePhaseTurbulenceCase,
        nut_cells: &[f64],
        nut_boundary: &[f64],
        alphat: &[f64],
    ) -> Self {
        let ni = g.n_internal();
        let n = g.n_cells();
        let mesh = &g.mesh;
        let npatch = mesh.patches.len();
        let mut count = vec![0usize; n];
        for (pi, p) in mesh.patches.iter().enumerate() {
            if case.wall_patch[pi] {
                for f in p.start..p.start + p.size {
                    count[mesh.owner[f]] += 1;
                }
            }
        }
        let cell_weight: Vec<f64> = count.iter().map(|&c| if c > 0 { 1.0 / (c as f64) } else { 0.0 }).collect();
        let zg = vec![Bc::ZeroGradient; npatch];
        let k = Scalar::uniform(g, 0.0, zg.clone());
        let mut k = k;
        k.c = case.k.clone();
        k.correct_bc(g);
        // epsilonWallFunction: the dictionary constructor extrapolates the cell value onto the patch (`extrapolateInternal`)
        let mut eps_bc = zg.clone();
        for (pi, p) in mesh.patches.iter().enumerate() {
            if case.wall_patch[pi] {
                eps_bc[pi] = Bc::Fixed((0..p.size).map(|j| case.epsilon[mesh.owner[p.start + j]]).collect());
            }
        }
        let mut epsilon = Scalar::uniform(g, 0.0, eps_bc);
        epsilon.c = case.epsilon.clone();
        epsilon.correct_bc(g);
        // nutkWallFunction: a fixedValue patch carrying its READ value until a wall function evaluates it
        let mut nut_bc = zg.clone();
        for (pi, _) in mesh.patches.iter().enumerate() {
            if case.wall_patch[pi] || case.nut_calculated[pi] {
                nut_bc[pi] = Bc::Calculated;
            }
        }
        let mut nut = Scalar::uniform(g, 0.0, nut_bc);
        nut.c = nut_cells.to_vec();
        nut.b = nut_boundary.to_vec();
        let mut at_bc = zg;
        for (pi, _) in mesh.patches.iter().enumerate() {
            if case.alphat_calculated[pi] {
                at_bc[pi] = Bc::Calculated;
            }
        }
        let mut at = Scalar::uniform(g, 0.0, at_bc);
        at.c = alphat.to_vec();
        at.correct_bc(g);
        debug_assert_eq!(nut.b.len(), g.n_faces() - ni);
        Self {
            case: case.clone(),
            k_old: k.c.clone(),
            k,
            epsilon,
            nut,
            alphat: at,
            epsilon_old: case.epsilon.clone(),
            cell_weight,
            wall_functions: true,
            production: vec![0.0; n],
            bound_count: 0,
            step_solves: Vec::new(),
        }
    }
}

/// `internalCoeffs` of the boundary faces of every cell of a scalar equation `div(phi, psi) - laplacian(gamma, psi)`:
/// a `fixedValue` face has the diffusive coefficient `gamma |Sf| deltaCoeffs`, any other face the convective `phi`.
fn boundary_internal_coeffs(g: &Geometry, phi: &[f64], gamma: &[f64], psi: &Scalar) -> Vec<f64> {
    let mut out = vec![0.0; g.n_cells()];
    for f in g.n_internal()..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let o = g.mesh.owner[f];
        if psi.is_fixed(g, f) {
            out[o] += gamma[f] * g.mesh.face_areas[f] * g.delta[f];
        } else {
            out[o] += phi[f];
        }
    }
    out
}

/// `fvMatrix::setValues(cells, values)` on an assembled equation whose boundary coefficients are folded into the diagonal and the
/// source: `unfolded_diag[c]` is the diagonal without them (`diag()`). Neighbours take the coupling to a set cell into their
/// source, the couplings are cut, the set row becomes `diag value = diag value`, and the row's boundary coefficients vanish.
pub fn set_values(g: &Geometry, e: &mut Eqn, cells: &[usize], values: &[f64], unfolded_diag: &[f64]) {
    let m = &g.mesh;
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); g.n_cells()];
    for f in 0..g.n_internal() {
        incident[m.owner[f]].push(f);
        incident[m.neighbour[f]].push(f);
    }
    for (i, &c) in cells.iter().enumerate() {
        let value = values[i];
        for &f in &incident[c] {
            if c == m.owner[f] {
                e.source[m.neighbour[f]] -= e.ldu.lower[f] * value;
            } else {
                e.source[m.owner[f]] -= e.ldu.upper[f] * value;
            }
            e.ldu.upper[f] = 0.0;
            e.ldu.lower[f] = 0.0;
        }
    }
    for (i, &c) in cells.iter().enumerate() {
        e.ldu.diag[c] = unfolded_diag[c];
        e.source[c] = values[i] * unfolded_diag[c];
    }
}

impl OnePhaseCcSolver {
    /// DEPRECATED ablation (gh:#1838): no wall functions. `epsilon` has `zeroGradient` wall patches, `G` is not overwritten,
    /// no cell is set, and `nut` keeps its wall values as read (what `turbulence false` leaves). The default applies them.
    #[must_use]
    pub fn without_wall_functions(mut self) -> Self {
        if let Some(t) = self.turb.as_mut() {
            t.wall_functions = false;
            for b in t.epsilon.bc.iter_mut() {
                if matches!(b, Bc::Fixed(_)) {
                    *b = Bc::ZeroGradient;
                }
            }
            t.epsilon.correct_bc(&self.geom);
        }
        self
    }

    /// CONTROL ONLY, NOT upstream's behaviour (gh:#1842): select the `kFinal`/`epsilonFinal` solver and relaxation entries in the last
    /// PIMPLE outer iteration, as stock OpenFOAM's `pimpleControl` would. GeN-Foam's `customPimpleControl` never does (see
    /// `PimpleState`), so the default is the plain entries until `residualControl` flags an iteration (gh:#1855).
    #[must_use]
    pub fn with_stock_final_iteration_selection(mut self) -> Self {
        self.pimple = self.pimple.with_stock_final_selection();
        self
    }

    /// `k` per cell (`None` with `turbulence false`).
    #[must_use]
    pub fn turbulent_kinetic_energy(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.k.c.as_slice())
    }

    /// `epsilon` per cell.
    #[must_use]
    pub fn turbulent_dissipation(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.epsilon.c.as_slice())
    }

    /// `nut` per cell.
    #[must_use]
    pub fn turbulent_viscosity(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.nut.c.as_slice())
    }

    /// `alphat` per cell.
    #[must_use]
    pub fn turbulent_thermal_diffusivity(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.alphat.c.as_slice())
    }

    /// `nut` on the wall boundary faces (by `face - n_internal`).
    #[must_use]
    pub fn turbulent_viscosity_boundary(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.nut.b.as_slice())
    }

    /// `alphat` on the boundary faces (by `face - n_internal`; gh:#1929).
    #[must_use]
    pub fn turbulent_thermal_diffusivity_boundary(&self) -> Option<&[f64]> {
        self.turb.as_ref().map(|t| t.alphat.b.as_slice())
    }

    /// Initial residual and iterations of the `epsilon` and `k` solves, one entry per outer iteration of the current step.
    #[must_use]
    pub fn turbulence_solves(&self) -> Option<&[[(f64, usize); 2]]> {
        self.turb.as_ref().map(|t| t.step_solves.as_slice())
    }

    /// `nut` of the wall faces by `nutkWallFunction::calcNut` (STEPWISE blender): `nutw = nu (yPlus kappa/ln(max(E yPlus, 1 + 1e-4)))
    /// - nu` above `yPlusLam`, 0 below; `yPlus = Cmu25 y sqrt(k)/nu`.
    fn wall_nut(&self, ts: &TurbState, nu_b: &[f64]) -> Vec<(usize, f64)> {
        let g = &self.geom;
        let ni = g.n_internal();
        let mut out = Vec::new();
        for (pi, p) in g.mesh.patches.iter().enumerate() {
            if !ts.case.wall_patch[pi] {
                continue;
            }
            let spec = &ts.case.nut_wall[pi];
            let w = spec.coeffs;
            let cmu25 = w.cmu25();
            for f in p.start..p.start + p.size {
                let y = ts.case.wall_y[f - ni];
                let nu = nu_b[f - ni];
                let kc = ts.k.c[g.mesh.owner[f]];
                let y_plus = cmu25 * y * kc.sqrt() / nu;
                out.push((f - ni, spec.blender.nut_wall(&w, y_plus, nu)));
            }
        }
        out
    }

    /// `porousKEpsilon::correct()` (`UEqn_1p.H`: `fluid_.turbulence().correct()`).
    #[allow(clippy::too_many_lines)]
    ///
    /// `mag_u` is `fluid_.magU()` as the last pressure corrector left it (used by the `DhStruct` stabilisation). The `kFinal`/`epsilonFinal`
    /// entries are selected when `isFinalIteration()` is true: the iteration `residualControl` flagged (gh:#1855), or, with the
    /// stock-OpenFOAM control (`with_stock_final_iteration_selection`), the last outer iteration.
    pub(super) fn correct_turbulence(&mut self, fluid_phi: &[f64], mag_u: &[f64]) {
        let Some(mut ts) = self.turb.take() else { return };
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let nb = g.n_faces() - ni;
        let m = g.mesh.clone();
        let th = self.case.thermo.clone();
        // `rho0` (Boussinesq: `rho` is that constant everywhere; the reader refuses `turbulence true` for any other package)
        let rho0 = self.rho.first().copied().unwrap_or(0.0);
        // `nu() = transport_.nu()` = the thermo's mu/rho(T) (cells and, with `zeroGradient` T, the owner value on faces):
        // `PhaseCompressibleTurbulenceModel` does not override `TurbulenceModel::nu()`, it does NOT use the model's `rho_` (= `rho0`)
        let nu_c: Vec<f64> = (0..n).map(|i| { let pp = th.props(self.p[i], self.t[i]); pp.mu / pp.rho }).collect();
        let nu_b: Vec<f64> = (0..nb).map(|i| nu_c[m.owner[ni + i]]).collect();
        let rdt = 1.0 / self.dt;
        let vol = &m.cell_volumes;
        let c = ts.case.clone();
        let alpha = &self.alpha;
        let s_alpha = &self.case.alpha_structure;
        let no_structure: Vec<f64> = s_alpha.iter().map(|&s| if s <= 0.0 { 1.0 } else { 0.0 }).collect();
        let yes_structure: Vec<f64> = s_alpha.iter().map(|&s| if s > 0.0 { 1.0 } else { 0.0 }).collect();

        // divU, G
        let div_u = fv::div(&g, fluid_phi);
        let grad_u = velocity_gradient(&g, &self.u);
        let mut gprod: Vec<f64> = (0..n).map(|i| ts.nut.c[i] * production_invariant(&grad_u[i])).collect();

        // epsilon.boundaryFieldRef().updateCoeffs(): wall functions
        let mut wall_cells: Vec<usize> = Vec::new();
        if ts.wall_functions {
            let mut g0 = vec![0.0; n];
            let mut eps0 = vec![0.0; n];
            for (pi, p) in m.patches.iter().enumerate() {
                if !c.wall_patch[pi] {
                    continue;
                }
                // each patch calculates with its OWN entries (`calculateTurbulenceFields`: `epf.calculate(...)`)
                let spec = &c.epsilon_wall[pi];
                let wc = spec.coeffs;
                let cmu25 = wc.cmu25();
                let cmu75 = wc.cmu.r_powf(0.75);
                let kappa = wc.kappa;
                for f in p.start..p.start + p.size {
                    let o = m.owner[f];
                    let w = ts.cell_weight[o];
                    let y = c.wall_y[f - ni];
                    let kc = ts.k.c[o];
                    let nuw = nu_b[f - ni];
                    let y_plus = cmu25 * y * kc.sqrt() / nuw;
                    // epsilon: the blend of the viscous and the inertial-sublayer value
                    let eps_vis = 2.0 * kc * nuw / (y * y);
                    let eps_log = cmu75 * kc.r_powf(1.5) / (kappa * y);
                    eps0[o] += w * spec.blender.epsilon_wall(&wc, spec.low_re_correction, y_plus, eps_vis, eps_log);
                    // G: (nutw + nuw) |snGrad U_w| Cmu25 sqrt(k)/(kappa y), only if !lowReCorrection || yPlus > yPlusLam
                    if !spec.low_re_correction || y_plus > wc.y_plus_lam {
                        let sn = (self.u.b[f - ni] - self.u.c[o]) * g.delta[f];
                        let mag_grad = sn.mag();
                        g0[o] += w * (ts.nut.b[f - ni] + nuw) * mag_grad * cmu25 * kc.sqrt() / (kappa * y);
                    }
                    if !wall_cells.contains(&o) {
                        wall_cells.push(o);
                    }
                }
            }
            for &o in &wall_cells {
                gprod[o] = g0[o];
                ts.epsilon.c[o] = eps0[o];
            }
            for (pi, p) in m.patches.iter().enumerate() {
                if c.wall_patch[pi] {
                    let vals: Vec<f64> = (0..p.size).map(|j| eps0[m.owner[p.start + j]]).collect();
                    for (j, v) in vals.iter().enumerate() {
                        ts.epsilon.b[p.start + j - ni] = *v;
                    }
                    ts.epsilon.bc[pi] = Bc::Fixed(vals);
                }
            }
        }
        let eps_old = ts.epsilon_old.clone();

        // equilibrium k, epsilon and the structure relaxation
        let cmu34 = c.cmu.r_powf(0.75);
        let mut eq_k = vec![0.0; n];
        let mut eq_eps = vec![0.0; n];
        let mut conv = vec![0.0; n];
        for i in 0..n {
            let x = self.u.c[i].mag() * c.turbulence_intensity_coeff[i] * self.re[i].r_powf(c.turbulence_intensity_exp[i]);
            eq_k[i] = 1.5 * (x * x);
            let len = (c.turbulence_length_scale_coeff[i] * self.case.dh[i]).max(SMALL);
            eq_eps[i] = cmu34 * eq_k[i].r_powf(1.5) / len;
            conv[i] = alpha[i] * rho0 * (self.u.c[i].mag() / c.convergence_length[i]) * yes_structure[i];
        }
        // gamma = alpha rho D*Eff, linearly interpolated (boundary: the patch values)
        let gamma_of = |sigma: f64, nut: &Scalar| -> Vec<f64> {
            let mut gm = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; m.patches.len()] };
            for i in 0..n {
                gm.c[i] = alpha[i] * rho0 * (nut.c[i] / sigma + nu_c[i]);
            }
            for i in 0..nb {
                gm.b[i] = alpha[m.owner[ni + i]] * rho0 * (nut.b[i] / sigma + nu_b[i]);
            }
            fv::interp_linear_openfoam(&g, &gm)
        };
        let ddt = |e: &mut Eqn, psi_old: &[f64]| {
            for i in 0..n {
                e.ldu.diag[i] += rdt * alpha[i] * rho0 * vol[i];
                e.source[i] += rdt * alpha[i] * rho0 * psi_old[i] * vol[i];
            }
        };
        let phi = &self.alpha_rho_phi;

        // ---- epsilon equation ----
        let use_final = self.pimple.final_iteration;
        let gamma_e = gamma_of(c.sigma_eps, &ts.nut);
        let mut e = Eqn::new(&g);
        ddt(&mut e, &eps_old);
        e.div_upwind(&g, phi, &ts.epsilon);
        e.neg_laplacian(&g, &gamma_e, &ts.epsilon);
        let susp_scale = (2.0 / 3.0) * c.c1 - c.c3;
        let mut su = vec![0.0; n];
        let mut susp = vec![0.0; n];
        let mut sp = vec![0.0; n];
        let mut su_conv = vec![0.0; n];
        for i in 0..n {
            su[i] = c.c1 * alpha[i] * rho0 * gprod[i] * ts.epsilon.c[i] / ts.k.c[i] * no_structure[i];
            susp[i] = susp_scale * alpha[i] * rho0 * div_u[i] * no_structure[i];
            sp[i] = c.c2 * alpha[i] * rho0 * ts.epsilon.c[i] / ts.k.c[i] * no_structure[i];
            su_conv[i] = conv[i] * eq_eps[i];
        }
        e.su(&g, &su);
        e.su_sp(&g, &susp, &ts.epsilon.c);
        e.sp(&g, &sp);
        e.sp(&g, &conv);
        e.su(&g, &su_conv);
        let bdiag = boundary_internal_coeffs(&g, phi, &gamma_e, &ts.epsilon);
        if let Some(a) = self.case.controls.relax.equation("epsilon", use_final).filter(|a| *a > 0.0) {
            let inflow2 = convective_inflow2(&g, phi, &ts.epsilon);
            relax_equation(&g, &mut e, a, &inflow2, &ts.epsilon.c);
        }
        // boundaryManipulate: the wall cells are SET to their (overwritten) epsilon
        if ts.wall_functions {
            let unfolded: Vec<f64> = (0..n).map(|i| e.ldu.diag[i] - bdiag[i]).collect();
            let values: Vec<f64> = wall_cells.iter().map(|&o| ts.epsilon.c[o]).collect();
            set_values(&g, &mut e, &wall_cells, &values, &unfolded);
        }
        let mut x = ts.epsilon.c.clone();
        let sv = if use_final { c.epsilon_solver_final } else { c.epsilon_solver };
        let perf = sv.solve(&e.ldu, &e.source, &mut x);
        let eps_solve = (perf.0, perf.1);
        self.pimple.record("epsilon", perf.0);
        ts.epsilon.c = x;
        ts.epsilon.correct_bc(&g);
        if bound(&g, &mut ts.epsilon, c.epsilon_min) {
            ts.bound_count += 1;
        }

        // ---- k equation ----
        let gamma_k = gamma_of(c.sigmak, &ts.nut);
        let mut e = Eqn::new(&g);
        ddt(&mut e, &ts.k_old);
        e.div_upwind(&g, phi, &ts.k);
        e.neg_laplacian(&g, &gamma_k, &ts.k);
        let mut su = vec![0.0; n];
        let mut susp = vec![0.0; n];
        let mut sp = vec![0.0; n];
        let mut su_conv = vec![0.0; n];
        for i in 0..n {
            su[i] = alpha[i] * rho0 * gprod[i] * no_structure[i];
            susp[i] = (2.0 / 3.0) * alpha[i] * rho0 * div_u[i] * no_structure[i];
            sp[i] = alpha[i] * rho0 * ts.epsilon.c[i] / ts.k.c[i] * no_structure[i];
            su_conv[i] = conv[i] * eq_k[i];
        }
        e.su(&g, &su);
        e.su_sp(&g, &susp, &ts.k.c);
        e.sp(&g, &sp);
        e.sp(&g, &conv);
        e.su(&g, &su_conv);
        if let Some(a) = self.case.controls.relax.equation("k", use_final).filter(|a| *a > 0.0) {
            let inflow2 = convective_inflow2(&g, phi, &ts.k);
            relax_equation(&g, &mut e, a, &inflow2, &ts.k.c);
        }
        let mut x = ts.k.c.clone();
        let sv = if use_final { c.k_solver_final } else { c.k_solver };
        let perf = sv.solve(&e.ldu, &e.source, &mut x);
        self.pimple.record("k", perf.0);
        ts.step_solves.push([eps_solve, (perf.0, perf.1)]);
        ts.k.c = x;
        ts.k.correct_bc(&g);
        if bound(&g, &mut ts.k, c.k_min) {
            ts.bound_count += 1;
        }

        // ---- correctNut ----
        for i in 0..n {
            ts.nut.c[i] = c.cmu * (ts.k.c[i] * ts.k.c[i]) / ts.epsilon.c[i];
        }
        // alphat from the UNstabilised nut (`correctNut`: "Correct alphat before nut is stabilized")
        for i in 0..n {
            ts.alphat.c[i] = rho0 * ts.nut.c[i] / c.prt;
        }
        ts.alphat.correct_bc(&g);
        // `nut.correctBoundaryConditions()` of `correctNut`: a `calculated` patch keeps the assigned expression's patch values
        // `Cmu k_b^2/epsilon_b`, any other non-wall patch (`zeroGradient`, `wedge`) the owner value (gh:#1929)
        let mut nut_unstab_b = vec![0.0; nb];
        for (pi, p) in m.patches.iter().enumerate() {
            if c.wall_patch[pi] {
                continue;
            }
            for f in p.start..p.start + p.size {
                nut_unstab_b[f - ni] =
                    if c.nut_calculated[pi] { c.cmu * (ts.k.b[f - ni] * ts.k.b[f - ni]) / ts.epsilon.b[f - ni] } else { ts.nut.c[m.owner[f]] };
            }
        }
        // `alphat_ = rho_*nut_/Prt_` on a `calculated` patch (`alphat_.correctBoundaryConditions()` leaves it)
        for (pi, p) in m.patches.iter().enumerate() {
            if c.alphat_calculated[pi] {
                for f in p.start..p.start + p.size {
                    ts.alphat.b[f - ni] = rho0 * nut_unstab_b[f - ni] / c.prt;
                }
            }
        }
        if c.nut_stabilization {
            // nut += pos(structure) magU DhStruct/laminarReStruct
            for i in 0..n {
                let pos_structure = if s_alpha[i] > 0.0 { 1.0 } else { 0.0 };
                ts.nut.c[i] += pos_structure * mag_u[i] * c.dh_struct[i] / c.laminar_re_struct[i];
            }
        }
        for (pi, p) in m.patches.iter().enumerate() {
            if !c.wall_patch[pi] {
                for j in 0..p.size {
                    let f = p.start + j;
                    let o = m.owner[f];
                    ts.nut.b[f - ni] = if c.nut_calculated[pi] {
                        // `nut_ += pos(structure_) magU DhStruct/laminarReStruct` on the patch: `structure_`, `DhStruct`, `laminarReStruct`
                        // are `zeroGradient` (owner values), `magU` is `mag(U_b)` left by the last pressure corrector
                        let stab = if c.nut_stabilization {
                            let pos_structure = if s_alpha[o] > 0.0 { 1.0 } else { 0.0 };
                            pos_structure * self.u.b[f - ni].mag() * c.dh_struct[o] / c.laminar_re_struct[o]
                        } else {
                            0.0
                        };
                        nut_unstab_b[f - ni] + stab
                    } else {
                        ts.nut.c[o]
                    };
                }
            }
        }
        if ts.wall_functions {
            for (fb, v) in self.wall_nut(&ts, &nu_b) {
                ts.nut.b[fb] = v;
            }
        }
        ts.production = gprod;
        self.turb = Some(ts);
    }
}

/// `wallFunctionCoefficients(dict)` and `wallFunctionBlenders(dict, STEPWISE, n)` of one wall patch dictionary.
///
/// # Errors
/// [`AppBuilderError::Case`] for a `blending` word `blenderTypeNames` does not hold, `kappa` or `E` below `SMALL`, `n < 0`
/// (`getCheckOrDefault` is fatal upstream), or an entry that is not a number.
#[cfg(feature = "std")]
fn read_wall_entries(ctx: &Ctx, what: &str, d: &FoamDict, default_blender: WallBlender) -> Result<(WallFunctionCoeffs, WallBlender), AppBuilderError> {
    let num = |key: &str, default: f64| -> Result<f64, AppBuilderError> {
        match d.get(key) {
            None => Ok(default),
            Some(_) => scalar(d, key).ok_or_else(|| ctx.e(alloc::format!("{what}.{key}: not a number"))),
        }
    };
    let cmu = num("Cmu", 0.09)?;
    let kappa = num("kappa", 0.41)?;
    let e = num("E", 9.8)?;
    if kappa < SMALL || e < SMALL {
        return Err(ctx.e(alloc::format!("{what}: kappa and E must be >= SMALL (wallFunctionCoefficients)")));
    }
    let n = num("n", default_blender.n)?;
    if n < 0.0 {
        return Err(ctx.e(alloc::format!("{what}.n: must be >= 0")));
    }
    let blending = match word(d, "blending") {
        None => default_blender.blending,
        Some(w) => WallFunctionBlending::from_name(&w).ok_or_else(|| {
            ctx.e(alloc::format!("{what}.blending `{w}`: not one of stepwise, max, binomial, exponential, tanh"))
        })?,
    };
    Ok((WallFunctionCoeffs::new(cmu, kappa, e), WallBlender { blending, n }))
}

/// Read the `turbulence true` part of the case.
///
/// # Errors
/// [`AppBuilderError::Case`] for every selection that is not ported (see the module documentation).
#[cfg(feature = "std")]
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub(super) fn read_turbulence_case(
    ctx: &Ctx,
    g: &Geometry,
    tdir: &str,
    fs: &FoamDict,
    tp: &FoamDict,
    sp: &FoamDict,
    poly_mesh_dir: &std::path::Path,
    zone_cells: impl Fn(&str) -> Result<Vec<usize>, AppBuilderError>,
) -> Result<Option<OnePhaseTurbulenceCase>, AppBuilderError> {
    let n = g.n_cells();
    let ras = ctx.sub(tp, "RAS")?;
    if super::super::case::bool_entry(ras, "turbulence") != Some(true) {
        return Ok(None);
    }
    if word(ras, "RASModel").as_deref() != Some("porousKEpsilon") {
        return Err(ctx.e("RASModel other than porousKEpsilon is not ported"));
    }
    let coeffs = ras.get_dict("porousKEpsilonCoeffs").unwrap_or(ras);
    let cf = |key: &str, default: f64| scalar(coeffs, key).unwrap_or(default);
    let prop = ctx.sub(tp, "porousKEpsilonProperties")?;
    let mut convergence_length = vec![1e69; n];
    let mut ic = vec![0.0; n];
    let mut ie = vec![0.0; n];
    let mut ls = vec![0.0; n];
    let mut regions_in_dict: Vec<String> = Vec::new();
    let mut nut_stabilization = false;
    let mut dh_struct = vec![0.0; n];
    let mut laminar_re_struct = vec![500.0; n];
    // structure volume fraction by zone key
    let vf_of = |region: &str| -> f64 {
        sp.iter()
            .filter_map(|(k, e)| match e {
                FoamEntry::SubDict(zd) if k.split(':').any(|z| z == region) => scalar(zd, "volumeFraction"),
                _ => None,
            })
            .fold(0.0_f64, f64::max)
    };
    for (key, e) in prop.iter() {
        let FoamEntry::SubDict(rd) = e else { continue };
        for region in key.split(':') {
            if vf_of(region) == 0.0 {
                continue;
            }
            regions_in_dict.push(region.to_string());
            let get = |k: &str| ctx.f64(rd, k);
            let (cl, a, b, cc) =
                (get("convergenceLength")?, get("turbulenceIntensityCoeff")?, get("turbulenceIntensityExp")?, get("turbulenceLengthScaleCoeff")?);
            // `nutStabilization`: `DhStruct` (required scalar) and `laminarReStruct` (default 500) on the region's cells
            let stabilisation = if rd.get("DhStruct").is_some() {
                nut_stabilization = true;
                Some((get("DhStruct")?, scalar(rd, "laminarReStruct").unwrap_or(500.0)))
            } else {
                None
            };
            for cell in zone_cells(region)? {
                if let Some((dh, re)) = stabilisation {
                    dh_struct[cell] = dh;
                    laminar_re_struct[cell] = re;
                }
                convergence_length[cell] = cl;
                ic[cell] = a;
                ie[cell] = b;
                ls[cell] = cc;
            }
        }
    }
    for (key, e) in sp.iter() {
        if let FoamEntry::SubDict(_) = e {
            for region in key.split(':') {
                if vf_of(region) != 0.0 && !regions_in_dict.iter().any(|r| r == region) {
                    return Err(ctx.e(format!("porousKEpsilon region `{region}`: properties were not set")));
                }
            }
        }
    }
    for v in convergence_length.iter_mut() {
        *v = v.max(SMALL);
    }

    // fields
    let kf = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/k"))?;
    let ef = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/epsilon"))?;
    let nf = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/nut"))?;
    let af = read_field(ctx, &alloc::format!("{tdir}/fluidRegion/alphat"))?;
    let check = |ff: &FieldFile, wall_ty: &str, other: &[&str], extra_keys: &[&str]| -> Result<Vec<bool>, AppBuilderError> {
        let mut out = Vec::new();
        for p in &g.mesh.patches {
            let (_, ty, _) = ff
                .bcs
                .iter()
                .find(|(nm, _, _)| nm == &p.name)
                .ok_or_else(|| ctx.e(format!("{}: no BC for patch `{}`", ff.name, p.name)))?;
            let is_wall = p.kind == PatchKind::Wall && ty == wall_ty;
            // `porousKEpsilon::correct` has no wall-function code of its own: the patch TYPES of the field files decide (gh:#1929)
            let allowed = is_wall
                || (p.kind == PatchKind::Wedge && ty == "wedge")
                || (p.kind == PatchKind::Empty && ty == "empty")
                || (!matches!(p.kind, PatchKind::Wedge | PatchKind::Empty) && other.contains(&ty.as_str()));
            if !allowed {
                return Err(ctx.e(format!("{}.{}: `{ty}` on a {:?} patch is not ported (wall: {wall_ty}, {other:?}, wedge)", ff.name, p.name, p.kind)));
            }
            if is_wall {
                if let Some((_, d)) = ff.dicts.iter().find(|(nm, _)| nm == &p.name) {
                    for (k, _) in d.iter() {
                        if k != "type" && k != "value" && !extra_keys.iter().any(|e| k == *e) {
                            return Err(ctx.e(format!("{}.{}: entry `{k}` of a `{ty}` patch is not ported", ff.name, p.name)));
                        }
                    }
                }
            }
            out.push(is_wall);
        }
        Ok(out)
    };
    let wk = check(&kf, "kqRWallFunction", &["zeroGradient"], &[])?;
    let we = check(&ef, "epsilonWallFunction", &["zeroGradient"], &["Cmu", "kappa", "E", "blending", "n", "lowReCorrection"])?;
    let wn = check(&nf, "nutkWallFunction", &["zeroGradient", "calculated"], &["Cmu", "kappa", "E", "blending", "n", "U"])?;
    let patch_is = |ff: &FieldFile, name: &str, want: &str| ff.bcs.iter().any(|(nm, ty, _)| nm == name && ty == want);
    let nut_calculated: Vec<bool> = g.mesh.patches.iter().map(|p| patch_is(&nf, &p.name, "calculated")).collect();
    if wk != we || we != wn {
        return Err(ctx.e("k, epsilon and nut must carry their wall functions on the same patches"));
    }
    let mut alphat_calculated = Vec::new();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        let (_, ty, _) = af.bcs.iter().find(|(nm, _, _)| nm == &p.name).ok_or_else(|| ctx.e("alphat: no BC for a patch"))?;
        if !matches!(ty.as_str(), "zeroGradient" | "wedge" | "calculated" | "empty") {
            return Err(ctx.e(format!("alphat.{}: `{ty}` is not ported (zeroGradient, calculated, wedge)", p.name)));
        }
        if ty == "calculated" && wn[pi] {
            return Err(ctx.e(format!("alphat.{}: `calculated` beside a `nutkWallFunction` is not ported", p.name)));
        }
        alphat_calculated.push(ty == "calculated");
    }
    let k0 = kf.internal.as_scalar().ok_or_else(|| ctx.e("k internalField must be a uniform scalar"))?;
    let e0 = ef.internal.as_scalar().ok_or_else(|| ctx.e("epsilon internalField must be a uniform scalar"))?;

    // the wall-function entries of every wall patch (`wallFunctionCoefficients(dict)`, `wallFunctionBlenders(dict, STEPWISE, n)`)
    let mut nut_wall = vec![NutWallSpec::standard(); g.mesh.patches.len()];
    let mut epsilon_wall = vec![EpsilonWallSpec::standard(); g.mesh.patches.len()];
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if !wn[pi] {
            continue;
        }
        if let Some((_, d)) = nf.dicts.iter().find(|(nm, _)| nm == &p.name) {
            let (coeffs, blender) = read_wall_entries(ctx, &alloc::format!("nut.{}", p.name), d, WallBlender::nut_default())?;
            nut_wall[pi] = NutWallSpec { coeffs, blender, u_name: word(d, "U") };
        }
        if let Some((_, d)) = ef.dicts.iter().find(|(nm, _)| nm == &p.name) {
            let (coeffs, blender) = read_wall_entries(ctx, &alloc::format!("epsilon.{}", p.name), d, WallBlender::epsilon_default())?;
            let low_re_correction = super::super::case::bool_entry(d, "lowReCorrection").unwrap_or(false);
            epsilon_wall[pi] = EpsilonWallSpec { coeffs, blender, low_re_correction };
        }
    }

    // solvers and relaxation: the LAST matching pattern wins
    let solvers = ctx.sub(fs, "solvers")?;
    let solver_of = |name: &str| -> Result<DiagonalBicgstab, AppBuilderError> {
        match last_match(solvers, name) {
            Some(FoamEntry::SubDict(d)) => {
                let dilu = match (word(d, "solver").as_deref(), word(d, "preconditioner").as_deref()) {
                    (Some("PBiCGStab"), Some("diagonal")) => false,
                    (Some("PBiCG"), Some("DILU")) => true,
                    _ => return Err(ctx.e(format!("solvers for `{name}`: only PBiCGStab/diagonal and PBiCG/DILU are ported"))),
                };
                Ok(DiagonalBicgstab {
                    dilu,
                    tolerance: scalar(d, "tolerance").unwrap_or(1e-6),
                    rel_tol: scalar(d, "relTol").unwrap_or(0.0),
                    min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
                    max_iter: scalar(d, "maxIter").map_or(DEFAULT_MAX_ITER, |v| v as usize),
                })
            }
            _ => Err(ctx.e(format!("fvSolution: no solver entry for `{name}`"))),
        }
    };
    let (k_solver, epsilon_solver) = (solver_of("k")?, solver_of("epsilon")?);
    // `kFinal`, `epsilonFinal`: GeN-Foam never selects them (`isFinalIteration()` is false), so an absent or unported entry is not
    // an error; they are read for the stock-OpenFOAM control only (`with_stock_final_iteration_selection`), falling back to the plain entry
    let k_solver_final = solver_of("kFinal").unwrap_or(k_solver);
    let epsilon_solver_final = solver_of("epsilonFinal").unwrap_or(epsilon_solver);
    // y of the wall faces
    let rd = |name: &str| -> Result<String, AppBuilderError> {
        let p = poly_mesh_dir.join(name);
        std::fs::read_to_string(&p).map_err(|e| AppBuilderError::Io { path: p, source: e })
    };
    let points = crate::io::poly_mesh::parse_points(&rd("points")?, "points")?;
    let faces = crate::io::poly_mesh::parse_faces(&rd("faces")?, "faces")?;
    let wall_y = near_wall_dist(&g.mesh, &points, &faces);

    Ok(Some(OnePhaseTurbulenceCase {
        cmu: cf("Cmu", 0.09),
        c1: cf("C1", 1.44),
        c2: cf("C2", 1.92),
        c3: cf("C3", 0.0),
        sigmak: cf("sigmak", 1.0),
        sigma_eps: cf("sigmaEps", 1.3),
        prt: cf("Prt", 1.0),
        k_min: scalar(ras, "kMin").unwrap_or(SMALL),
        epsilon_min: scalar(ras, "epsilonMin").unwrap_or(SMALL),
        k: vec![k0; n],
        epsilon: vec![e0; n],
        convergence_length,
        turbulence_intensity_coeff: ic,
        turbulence_intensity_exp: ie,
        turbulence_length_scale_coeff: ls,
        wall_patch: wk,
        nut_calculated,
        alphat_calculated,
        wall_y,
        nut_wall,
        epsilon_wall,
        nut_stabilization,
        dh_struct,
        laminar_re_struct,
        k_solver,
        epsilon_solver,
        k_solver_final,
        epsilon_solver_final,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbicg_dilu_and_pbicgstab_diagonal_solve_the_same_asymmetric_system() {
        // 4 cells in a row, an asymmetric diagonally dominant matrix (`div` + `laplacian`-like)
        let mut m = outram_foam_basic_lib::prelude::LduMatrix::new(4, alloc::vec![0, 1, 2], alloc::vec![1, 2, 3]);
        m.diag = alloc::vec![4.0, 5.0, 5.0, 4.0];
        m.upper = alloc::vec![-1.0, -1.5, -1.0];
        m.lower = alloc::vec![-2.0, -0.5, -1.2];
        let source = [1.0, 2.0, 3.0, 4.0];
        let mk = |dilu: bool| DiagonalBicgstab { tolerance: 1e-14, rel_tol: 0.0, min_iter: 0, max_iter: 1000, dilu };
        let (mut a, mut b) = ([0.0; 4], [0.0; 4]);
        let (ra, ia) = mk(false).solve(&m, &source, &mut a);
        let (rb, ib) = mk(true).solve(&m, &source, &mut b);
        assert!(ra > 0.0 && rb > 0.0 && ia > 0 && ib > 0);
        for c in 0..4 {
            assert!((a[c] - b[c]).abs() < 1e-12, "{c}: {} {}", a[c], b[c]);
        }
        // the solution satisfies the system
        let res = m.multiply(&b);
        for c in 0..4 {
            assert!((res[c] - source[c]).abs() < 1e-12);
        }
    }

    #[test]
    fn y_plus_lam_is_the_fixed_point_of_the_log_law_intersection() {
        let w = WallFunctionCoeffs::standard();
        // ypl = ln(E ypl)/kappa: the well-known 11.53 for kappa 0.41, E 9.8
        assert!((w.y_plus_lam - 11.53).abs() < 0.01, "{}", w.y_plus_lam);
        assert!((w.cmu25() - 0.09_f64.r_powf(0.25)).abs() < 1e-15);
    }

    #[test]
    fn point_triangle_distance_covers_face_edge_and_vertex_regions() {
        let (a, b, c) = (Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0));
        // above the interior
        assert!((point_triangle_distance(Vector3::new(0.2, 0.2, 0.5), a, b, c) - 0.5).abs() < 1e-15);
        // beside the edge ab
        assert!((point_triangle_distance(Vector3::new(0.5, -0.3, 0.4), a, b, c) - 0.5).abs() < 1e-15);
        // beyond the vertex a
        assert!((point_triangle_distance(Vector3::new(-0.3, -0.4, 0.0), a, b, c) - 0.5).abs() < 1e-15);
        // beside the hypotenuse bc
        let d = point_triangle_distance(Vector3::new(1.0, 1.0, 0.0), a, b, c);
        assert!((d - 0.5 * 2.0_f64.sqrt()).abs() < 1e-15);
    }

    #[test]
    fn a_quad_face_distance_is_the_perpendicular_one_inside_and_the_edge_one_outside() {
        let pts = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];
        let ctr = Vector3::new(0.5, 0.5, 0.0);
        assert!((point_face_distance(Vector3::new(0.3, 0.8, 0.25), &pts, ctr) - 0.25).abs() < 1e-15);
        assert!((point_face_distance(Vector3::new(2.0, 0.5, 0.0), &pts, ctr) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn blender_names_round_trip_and_unknown_names_are_none() {
        for b in [
            WallFunctionBlending::Stepwise,
            WallFunctionBlending::Max,
            WallFunctionBlending::Binomial,
            WallFunctionBlending::Exponential,
            WallFunctionBlending::Tanh,
        ] {
            assert_eq!(WallFunctionBlending::from_name(b.name()), Some(b));
        }
        assert_eq!(WallFunctionBlending::from_name("smooth"), None);
        assert_eq!(WallBlender::nut_default().n, 4.0);
        assert_eq!(WallBlender::epsilon_default().n, 2.0);
    }

    #[test]
    fn wall_coefficients_new_recomputes_the_sublayer_intersection() {
        let w = WallFunctionCoeffs::new(0.0845, 0.4187, 9.793);
        // the fixed point of ypl = ln(E ypl)/kappa
        assert!((w.y_plus_lam - (w.e * w.y_plus_lam).r_ln() / w.kappa).abs() < 1e-6, "{}", w.y_plus_lam);
        assert!(w.y_plus_lam != WallFunctionCoeffs::standard().y_plus_lam);
    }

    /// nut: `max` equals `stepwise` away from the crossing at `yPlusLam` (prediction R3: `nutLog < nutVis` below it, `>` above),
    /// and the smooth blenders reduce to the inertial value far above it.
    #[test]
    fn nut_max_equals_stepwise_away_from_the_crossing_and_the_smooth_blenders_reach_the_log_value() {
        let w = WallFunctionCoeffs::standard();
        let nu = 1.2e-4;
        let step = WallBlender::nut_default();
        let max = WallBlender { blending: WallFunctionBlending::Max, n: 4.0 };
        let mut y = 0.2_f64;
        while y < 60.0 {
            if (y - w.y_plus_lam).abs() > 0.05 {
                assert_eq!(step.nut_wall(&w, y, nu), max.nut_wall(&w, y, nu), "y+ {y}");
            }
            y *= 1.07;
        }
        let y = 2000.0;
        let log = step.nut_wall(&w, y, nu);
        // exponential: nutLog - nu, as stepwise; tanh (eta = 1): `b1 - nu = nutLog`, i.e. stepwise + nu (the blend adds the viscous part)
        let v = WallBlender { blending: WallFunctionBlending::Exponential, n: 4.0 }.nut_wall(&w, y, nu);
        // `log` is this module's own stepwise value (full double precision), not a rounded upstream number. The 1e-9 expectation was
        // wrong: the formula itself leaves `nutLog exp(-1/Gamma)`, `Gamma = 0.01 y+^4/(1+5 y+) = 1.6e7`, so the relative deficit is
        // `(1 + nu/log)/Gamma` (about 6.2e-8) analytically; assert THAT value to 1e-3 of itself.
        let gamma = 0.01 * (y * y) * (y * y) / (1.0 + 5.0 * y);
        let predicted = (1.0 + nu / log) / gamma;
        assert!((((log - v) / log) - predicted).abs() < 1e-3 * predicted, "{v} {log} {predicted}");
        let v = WallBlender { blending: WallFunctionBlending::Tanh, n: 4.0 }.nut_wall(&w, y, nu);
        assert!(((v - (log + nu)) / log).abs() < 1e-9, "{v} {log}");
        // binomial with n = 1 is vis + log: nutw = nutLog
        let bin = WallBlender { blending: WallFunctionBlending::Binomial, n: 1.0 };
        let y = 5.0;
        let nut_log = nu * y * w.kappa / (w.e * y).r_ln();
        assert!((bin.nut_wall(&w, y, nu) - nut_log).abs() < 1e-12 * nut_log);
    }

    /// epsilon: `stepwise` with `lowReCorrection` takes the viscous value below `yPlusLam`; `max` the larger; the others as their formulas say.
    #[test]
    fn epsilon_blenders_follow_their_formulas() {
        let w = WallFunctionCoeffs::standard();
        let (vis, log) = (3.0, 2.0);
        let step = WallBlender::epsilon_default();
        assert_eq!(step.epsilon_wall(&w, false, 5.0, vis, log), log);
        assert_eq!(step.epsilon_wall(&w, true, 5.0, vis, log), vis);
        assert_eq!(step.epsilon_wall(&w, true, 50.0, vis, log), log);
        let max = WallBlender { blending: WallFunctionBlending::Max, n: 2.0 };
        assert_eq!(max.epsilon_wall(&w, false, 5.0, vis, log), vis);
        let bin = WallBlender { blending: WallFunctionBlending::Binomial, n: 2.0 };
        assert!((bin.epsilon_wall(&w, false, 5.0, vis, log) - 13.0_f64.sqrt()).abs() < 1e-14);
        // tanh at y+ = 0: eta = 0, the result is (vis^1.2 + log^1.2)^(1/1.2)
        let th = WallBlender { blending: WallFunctionBlending::Tanh, n: 2.0 };
        let b2 = (vis.r_powf(1.2) + log.r_powf(1.2)).r_powf(1.0 / 1.2);
        assert!((th.epsilon_wall(&w, false, 0.0, vis, log) - b2).abs() < 1e-14);
        // exponential: Gamma -> inf gives eps_log, Gamma -> 0 gives eps_vis
        let ex = WallBlender { blending: WallFunctionBlending::Exponential, n: 2.0 };
        assert!((ex.epsilon_wall(&w, false, 500.0, vis, log) - log).abs() < 1e-4);
        assert!((ex.epsilon_wall(&w, false, 1e-3, vis, log) - vis).abs() < 1e-9);
    }
}
