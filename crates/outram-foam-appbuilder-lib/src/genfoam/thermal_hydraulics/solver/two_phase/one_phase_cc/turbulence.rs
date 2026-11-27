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
//! ## Refused by name
//!
//! `RASModel` other than `porousKEpsilon`; `DhStruct` (nut stabilisation); wall-function entries other than `type`/`value`
//! (`Cmu`, `kappa`, `E`, `blending`, `lowReCorrection`, `U`); `k`/`epsilon`/`nut` wall patches other than the three above;
//! a `k`/`epsilon` solver other than `PBiCGStab`/`diagonal`; `fvOptions`.

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
        let (cmu, kappa, e) = (0.09, 0.41, 9.8);
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
    /// `y` (`nearWallDist`) of every boundary face (`face - n_internal`), 0 off the walls.
    pub wall_y: Vec<f64>,
    /// Wall-function coefficients.
    pub wall: WallFunctionCoeffs,
    /// The `k` linear solver.
    pub k_solver: DiagonalBicgstab,
    /// The `epsilon` linear solver.
    pub epsilon_solver: DiagonalBicgstab,
    /// `relaxationFactors.equations` of `k`.
    pub relax_k: Option<f64>,
    /// `relaxationFactors.equations` of `epsilon`.
    pub relax_epsilon: Option<f64>,
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
            if case.wall_patch[pi] {
                nut_bc[pi] = Bc::Calculated;
            }
        }
        let mut nut = Scalar::uniform(g, 0.0, nut_bc);
        nut.c = nut_cells.to_vec();
        nut.b = nut_boundary.to_vec();
        let mut at = Scalar::uniform(g, 0.0, zg);
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
        let w = ts.case.wall;
        let cmu25 = w.cmu25();
        let mut out = Vec::new();
        for (pi, p) in g.mesh.patches.iter().enumerate() {
            if !ts.case.wall_patch[pi] {
                continue;
            }
            for f in p.start..p.start + p.size {
                let y = ts.case.wall_y[f - ni];
                let nu = nu_b[f - ni];
                let kc = ts.k.c[g.mesh.owner[f]];
                let y_plus = cmu25 * y * kc.sqrt() / nu;
                let nutw = if y_plus > w.y_plus_lam {
                    let nut_log = nu * y_plus * w.kappa / (w.e * y_plus).max(1.0 + 1e-4).r_ln();
                    nut_log - nu
                } else {
                    nu - nu
                };
                out.push((f - ni, nutw));
            }
        }
        out
    }

    /// `porousKEpsilon::correct()` (`UEqn_1p.H`: `fluid_.turbulence().correct()`).
    #[allow(clippy::too_many_lines)]
    pub(super) fn correct_turbulence(&mut self, fluid_phi: &[f64]) {
        let Some(mut ts) = self.turb.take() else { return };
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let nb = g.n_faces() - ni;
        let m = g.mesh.clone();
        let th = self.case.thermo.clone();
        let rho0 = th.rho0;
        // `nu() = transport_.nu()` = the thermo's mu/rho(T) (cells and, with `zeroGradient` T, the owner value on faces):
        // `PhaseCompressibleTurbulenceModel` does not override `TurbulenceModel::nu()`, it does NOT use the model's `rho_` (= `rho0`)
        let nu_c: Vec<f64> = (0..n).map(|i| th.mu / th.rho(self.t[i])).collect();
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
            let cmu25 = c.wall.cmu25();
            let cmu75 = c.wall.cmu.r_powf(0.75);
            let kappa = c.wall.kappa;
            let mut g0 = vec![0.0; n];
            let mut eps0 = vec![0.0; n];
            for (pi, p) in m.patches.iter().enumerate() {
                if !c.wall_patch[pi] {
                    continue;
                }
                for f in p.start..p.start + p.size {
                    let o = m.owner[f];
                    let w = ts.cell_weight[o];
                    let y = c.wall_y[f - ni];
                    let kc = ts.k.c[o];
                    // epsilon (STEPWISE, lowReCorrection false): the inertial-sublayer value
                    eps0[o] += w * (cmu75 * kc.r_powf(1.5) / (kappa * y));
                    // G: (nutw + nuw) |snGrad U_w| Cmu25 sqrt(k)/(kappa y)
                    let sn = (self.u.b[f - ni] - self.u.c[o]) * g.delta[f];
                    let mag_grad = sn.mag();
                    g0[o] += w * (ts.nut.b[f - ni] + nu_b[f - ni]) * mag_grad * cmu25 * kc.sqrt() / (kappa * y);
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
            fv::interp_linear(&g, &gm)
        };
        let ddt = |e: &mut Eqn, psi_old: &[f64]| {
            for i in 0..n {
                e.ldu.diag[i] += rdt * alpha[i] * rho0 * vol[i];
                e.source[i] += rdt * alpha[i] * rho0 * psi_old[i] * vol[i];
            }
        };
        let phi = &self.alpha_rho_phi;

        // ---- epsilon equation ----
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
        if let Some(a) = c.relax_epsilon {
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
        let sv = c.epsilon_solver;
        let perf = pbicgstab_diagonal(&e.ldu, &e.source, &mut x, sv.tolerance, sv.rel_tol, sv.max_iter, sv.min_iter);
        let eps_solve = (perf.initial_residual, perf.iterations);
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
        if let Some(a) = c.relax_k {
            let inflow2 = convective_inflow2(&g, phi, &ts.k);
            relax_equation(&g, &mut e, a, &inflow2, &ts.k.c);
        }
        let mut x = ts.k.c.clone();
        let sv = c.k_solver;
        let perf = pbicgstab_diagonal(&e.ldu, &e.source, &mut x, sv.tolerance, sv.rel_tol, sv.max_iter, sv.min_iter);
        ts.step_solves.push([eps_solve, (perf.initial_residual, perf.iterations)]);
        ts.k.c = x;
        ts.k.correct_bc(&g);
        if bound(&g, &mut ts.k, c.k_min) {
            ts.bound_count += 1;
        }

        // ---- correctNut ----
        for i in 0..n {
            ts.nut.c[i] = c.cmu * (ts.k.c[i] * ts.k.c[i]) / ts.epsilon.c[i];
        }
        for (pi, p) in m.patches.iter().enumerate() {
            if !c.wall_patch[pi] {
                for j in 0..p.size {
                    let f = p.start + j;
                    ts.nut.b[f - ni] = ts.nut.c[m.owner[f]];
                }
            }
        }
        if ts.wall_functions {
            for (fb, v) in self.wall_nut(&ts, &nu_b) {
                ts.nut.b[fb] = v;
            }
        }
        for i in 0..n {
            ts.alphat.c[i] = rho0 * ts.nut.c[i] / c.prt;
        }
        ts.alphat.correct_bc(&g);
        ts.production = gprod;
        self.turb = Some(ts);
    }
}

/// `true` when `v` is within `tol` relative of `w` (used by the reader to compare the `Final` solver entries).
#[cfg(feature = "std")]
fn same_solver(a: &DiagonalBicgstab, b: &DiagonalBicgstab) -> bool {
    a == b
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
            if rd.get("DhStruct").is_some() {
                return Err(ctx.e(format!("porousKEpsilonProperties.{key}.DhStruct (nutStabilization) is not ported")));
            }
            regions_in_dict.push(region.to_string());
            let get = |k: &str| ctx.f64(rd, k);
            let (cl, a, b, cc) =
                (get("convergenceLength")?, get("turbulenceIntensityCoeff")?, get("turbulenceIntensityExp")?, get("turbulenceLengthScaleCoeff")?);
            for cell in zone_cells(region)? {
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
    let check = |ff: &FieldFile, wall_ty: &str, other: &[&str]| -> Result<Vec<bool>, AppBuilderError> {
        let mut out = Vec::new();
        for p in &g.mesh.patches {
            let (_, ty, _) = ff
                .bcs
                .iter()
                .find(|(nm, _, _)| nm == &p.name)
                .ok_or_else(|| ctx.e(format!("{}: no BC for patch `{}`", ff.name, p.name)))?;
            let is_wall = p.kind == PatchKind::Wall && ty == wall_ty;
            let allowed = is_wall || (p.kind == PatchKind::Wedge && ty == "wedge") || (p.kind == PatchKind::Wall && other.contains(&ty.as_str()));
            if !allowed {
                return Err(ctx.e(format!("{}.{}: `{ty}` on a {:?} patch is not ported (wall: {wall_ty}, wedge)", ff.name, p.name, p.kind)));
            }
            if is_wall {
                if let Some((_, d)) = ff.dicts.iter().find(|(nm, _)| nm == &p.name) {
                    for (k, _) in d.iter() {
                        if k != "type" && k != "value" {
                            return Err(ctx.e(format!("{}.{}: entry `{k}` of a `{ty}` patch is not ported", ff.name, p.name)));
                        }
                    }
                }
            }
            out.push(is_wall);
        }
        Ok(out)
    };
    let wk = check(&kf, "kqRWallFunction", &[])?;
    let we = check(&ef, "epsilonWallFunction", &[])?;
    let wn = check(&nf, "nutkWallFunction", &[])?;
    if wk != we || we != wn {
        return Err(ctx.e("k, epsilon and nut must carry their wall functions on the same patches"));
    }
    for p in &g.mesh.patches {
        let (_, ty, _) = af.bcs.iter().find(|(nm, _, _)| nm == &p.name).ok_or_else(|| ctx.e("alphat: no BC for a patch"))?;
        if !matches!(ty.as_str(), "zeroGradient" | "wedge") {
            return Err(ctx.e(format!("alphat.{}: `{ty}` is not ported (zeroGradient, wedge)", p.name)));
        }
    }
    let k0 = kf.internal.as_scalar().ok_or_else(|| ctx.e("k internalField must be a uniform scalar"))?;
    let e0 = ef.internal.as_scalar().ok_or_else(|| ctx.e("epsilon internalField must be a uniform scalar"))?;

    // solvers and relaxation: the LAST matching pattern wins
    let solvers = ctx.sub(fs, "solvers")?;
    let solver_of = |name: &str| -> Result<DiagonalBicgstab, AppBuilderError> {
        match last_match(solvers, name) {
            Some(FoamEntry::SubDict(d)) => {
                if word(d, "solver").as_deref() != Some("PBiCGStab") || word(d, "preconditioner").as_deref() != Some("diagonal") {
                    return Err(ctx.e(format!("solvers for `{name}`: only PBiCGStab with the diagonal preconditioner is ported")));
                }
                Ok(DiagonalBicgstab {
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
    if !same_solver(&k_solver, &solver_of("kFinal")?) || !same_solver(&epsilon_solver, &solver_of("epsilonFinal")?) {
        return Err(ctx.e("k/epsilon: a `Final` solver entry that differs from the plain one is not ported"));
    }
    let relax_of = |name: &str| -> Option<f64> {
        fs.get_dict("relaxationFactors").and_then(|r| r.get_dict("equations")).and_then(|e| match last_match(e, name) {
            Some(FoamEntry::Scalar(x)) => Some(*x),
            _ => None,
        })
    };
    let (relax_k, relax_epsilon) = (relax_of("k"), relax_of("epsilon"));
    if relax_k != relax_of("kFinal") || relax_epsilon != relax_of("epsilonFinal") {
        return Err(ctx.e("k/epsilon: a `Final` relaxation factor that differs from the plain one is not ported"));
    }

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
        wall_y,
        wall: WallFunctionCoeffs::standard(),
        k_solver,
        epsilon_solver,
        relax_k,
        relax_epsilon,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
