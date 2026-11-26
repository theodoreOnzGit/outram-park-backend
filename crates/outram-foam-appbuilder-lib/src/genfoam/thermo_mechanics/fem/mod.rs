// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Reuses `farrer-park` (FEM structural mechanics, ported from MOOSE tensor mechanics, LGPL-2.1, relicensed per its
// NOTICE) for the element, quadrature, B matrix, CSR assembly, Dirichlet elimination and the ILU(0)-CG solve.
// The field being solved is the one of GeN-Foam `legacyThermoMechanics` (commit 652b3da; GPL-3.0,
// (C) 2015-2022 EPFL, Carlo Fiorina), discretised by finite elements instead of finite volumes.
//
// This file is part of OUTRAM PARK, distributed under AGPL-3.0. See the crate root or
// <https://www.gnu.org/licenses/> for the full license text.

//! # `FemThermoElastic`: small-strain thermo-elasticity by finite elements on GeN-Foam's `polyMesh` (gh:#1782)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The parallel, finite-element discretisation of the structural mechanics that [`super::legacy_fvm`] solves by
//! finite volumes: the same `polyMesh`, the same per-zone cards ([`ZoneCard`]: `E`, `nu`, `alpha`, `Tref`, `rho`),
//! the same per-cell `TStruct`, and the same patch conditions (`fixedValue`; `zeroGradient` becomes the natural,
//! traction-free condition, see below). It is selected beside the finite-volume solver by
//! [`super::structural_region::StructuralDiscretisation`], finite volume being the default.
//!
//! ## What is solved
//!
//! Steady small-strain thermo-elasticity, `div(sigma) = 0`, `sigma = lambda tr(eps) I + 2 mu eps - 3 K alpha (T - Tref) I`,
//! `eps = sym grad u`: weak form `K u = f_thermal + f_traction`,
//! `f_thermal = integral(B^T 3 K alpha (T - Tref) I dV)`. `T` is the cell value `TStruct` (piecewise constant on
//! each cell, as the finite-volume solver sees it). The finite-volume solver's `d2dt2` is `steadyState` in GeN-Foam's
//! shipped cases and the inertial term vanishes; transient `d2dt2` is not ported on either side (gh:#1845).
//!
//! ## Reused from `farrer-park` (searched first; the crate is a finite-element code, MMS/Lame verified)
//!
//! `mesh::Mesh`, `element::{ElementType, map_gradients}`, `quadrature::default_rule`, `assembly::BMatrix`
//! (strain-displacement operator, stiffness and internal-force accumulation), `tensor::{Tensor4, Voigt6}`,
//! `dof::{DofMap, SparsityPattern}`, `sparse::CsrMatrix` (scatter), `bc::{DirichletSet, Traction}`,
//! `solver::solve_linear` (CG + ILU(0) through `outram-foam-basic-lib`'s shared Krylov layer).
//! Checked and NOT reused: `assembly::System` (one material for the whole mesh, no thermal strain: GeN-Foam has
//! eight zones and an eigenstrain), `solver::solve_nonlinear` (load stepping and plasticity, not needed for a linear
//! problem), `outram-park-fork-offbeat` (a finite-volume `smallStrain`, the wrong discretisation), the
//! `MechanicsMeshSolver` of this module (finite volume). No `code_aster` port exists in the workspace.
//!
//! ## Element
//!
//! Trilinear `Hex8` (2x2x2 Gauss, full integration) on an all-hexahedral mesh, otherwise linear `Tet4` on the
//! conforming cone decomposition of every cell ([`topology`]). Both are displacement elements: `u` is continuous,
//! stress is discontinuous between elements and constant (`Tet4`) or bilinear-in-`xi` (`Hex8`) inside one.
//! Hex8 shear-locks in bending and is volumetrically stiff for nearly incompressible solids (`nu -> 0.5`); that is
//! `farrer-park`'s known, measured gap (its `op-uqqg`), not corrected here.
//!
//! ## Stiffness normalisation (`StiffnessNormalisation`)
//!
//! GeN-Foam's displacement equation divides the stress by `rho` (`E_ = rhoE/rho`, so `d2dt2(D) = div(sigma/rho)`).
//! With one `rho` that is the elasticity equation; with eight zones of different `rho` the interface condition is
//! continuity of `sigma/rho`, not of `sigma`. [`StiffnessNormalisation::UpstreamRhoNormalised`] (the DEFAULT, "follow
//! upstream") solves that same equation so a finite-element / finite-volume difference measures the discretisation
//! alone; [`StiffnessNormalisation::Physical`] solves `div(sigma) = 0` with `E = rhoE`. Reported stresses are
//! multiplied by `rho` in the first case, as `calculateStress.H` does.
//!
//! ## `zeroGradient` is not traction-free
//!
//! `zeroGradient` on `D` imposes `dD/dn = 0`; the natural boundary condition of the weak form is `sigma . n = 0`.
//! They coincide only where the solution has no normal-gradient/shear coupling (a 1-D state). A case with
//! `zeroGradient` patches therefore differs between the two discretisations by a model difference as well as a
//! discretisation one; the V&V record separates them (cases whose patches coincide, and the ESFR case where they do
//! not).
//!
//! ## Not here (named, filed)
//!
//! Nodal (continuous) temperature, anisotropic or temperature-dependent properties, the `GFTractionDisplacement`
//! and `gapContact` conditions (gh:#1235), transient inertia (gh:#1845), `movePoints` (gh:#1784).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;
use alloc::vec::Vec;

use farrer_park::assembly::{BMatrix, MAX_ELEM_DOFS};
use farrer_park::bc::{DirichletMethod, DirichletSet, Traction};
use farrer_park::dof::{DofMap, SparsityPattern};
use farrer_park::element::{map_gradients, ElementType, MAX_ELEM_NODES};
use farrer_park::mesh::{ElemId, Mesh, NodeId};
use farrer_park::quadrature::{default_rule, QuadraturePoint};
use farrer_park::solver::solve_linear;
use farrer_park::sparse::CsrMatrix;
use farrer_park::tensor::{Tensor4, Voigt6};
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use crate::error::AppBuilderError;
use super::legacy_fvm::ZoneCard;

pub mod topology;

pub use farrer_park::solver::LinearSolverSettings as FemLinearSolverSettings;
pub use topology::{FemElementChoice, FemMesh, FemTopology};

#[cfg(test)]
mod tests;

/// Boundary condition of the displacement on one patch.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum FemPatch {
    /// Prescribed displacement of all three components (m): GeN-Foam's `fixedValue`.
    FixedValue(Vector3),
    /// Prescribed displacement of the components that are `Some` (m); the others are free. A symmetry plane normal
    /// to an axis is `[Some(0.0), None, None]`. FEM only: the finite-volume solver has no such condition.
    FixedComponents([Option<f64>; 3]),
    /// Natural boundary: `sigma . n = 0`. What GeN-Foam's `zeroGradient` maps to (module documentation).
    TractionFree,
    /// Uniform normal pressure `p` (Pa) pushing into the solid, `t = -p n`. FEM only.
    Pressure(f64),
}

/// Which stiffness the equation is built with (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StiffnessNormalisation {
    /// `E = rhoE / rho`, `sigma/rho` continuous across zones: GeN-Foam's equation (default).
    #[default]
    UpstreamRhoNormalised,
    /// `E = rhoE`, `sigma` continuous across zones: `div(sigma) = 0`.
    Physical,
}

/// Settings of the finite-element solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FemSettings {
    /// `rheologyOption.planeStress` (GeN-Foam's modified `lambda` and `3K`, applied as the finite-volume solver does).
    pub plane_stress: bool,
    /// Stiffness normalisation.
    pub normalisation: StiffnessNormalisation,
    /// Element choice.
    pub element: FemElementChoice,
    /// The linear solve (default: CG + ILU(0), relative tolerance `1e-11`).
    pub linear: FemLinearSolverSettings,
}

impl Default for FemSettings {
    fn default() -> Self {
        Self {
            plane_stress: false,
            normalisation: StiffnessNormalisation::default(),
            element: FemElementChoice::default(),
            // farrer-park's default is 1e-12; the true relative residual of CG + ILU(0) stalls at about 1e-12 on the
            // 2e11 Pa stiffness (a 1.005e-12 > 1e-12 refusal was measured on the thick-cylinder mesh), so 1e-11.
            linear: FemLinearSolverSettings { tolerance: 1.0e-11, ..FemLinearSolverSettings::default() },
        }
    }
}

/// What one finite-element solve did.
#[derive(Debug, Clone, PartialEq)]
pub struct FemSolveLog {
    /// Degrees of freedom.
    pub n_dofs: usize,
    /// Elements.
    pub n_elements: usize,
    /// The element used.
    pub element: ElementType,
    /// Krylov iterations.
    pub n_iterations: usize,
    /// True relative residual of the returned iterate.
    pub final_residual: f64,
    /// Maximum cell `sigmaEq` (Pa), the von Mises stress of the cell-averaged stress (`rho`-scaled as `calculateStress.H`).
    pub max_sigma_eq: f64,
}

/// Per-cell constitutive data.
#[derive(Debug, Clone, Copy)]
struct CellMaterial {
    mu: f64,
    lambda: f64,
    /// `3K alpha` (Pa/K), after the same normalisation as `mu`.
    three_k_alpha: f64,
    t_ref: f64,
    /// Factor from the solved stress to the reported stress (`rho` when normalised, else 1).
    stress_scale: f64,
}

/// The per-cell material from the zone cards, with the defaults and plane-stress algebra of the finite-volume
/// constructor (`legacyThermoMechanics`): `E_ = rhoE/rho`, `mu = E/(2(1+nu))`, `lambda = nu E/((1+nu)(1-2nu))`,
/// `threeK = E/(1-2nu)`; plane stress `lambda = nu E/((1+nu)(1-nu))`, `threeK = E/(1-nu)`.
fn cell_materials(
    n_cells: usize,
    zones: &[(String, Vec<usize>)],
    cards: &[ZoneCard],
    settings: &FemSettings,
) -> Result<Vec<CellMaterial>, AppBuilderError> {
    let mut rho = vec![1.0; n_cells];
    let mut rho_e = vec![1.0; n_cells];
    let mut nu = vec![0.0; n_cells];
    let mut alpha = vec![1.0e-5; n_cells];
    let mut t_ref = vec![0.0; n_cells];
    for card in cards {
        let zone = zones.iter().find(|z| z.0 == card.name).ok_or_else(|| AppBuilderError::Case {
            case: crate::path::PathBuf::new(),
            msg: format!("thermoMechanicalProperties.materials: no cellZone `{}`", card.name),
        })?;
        for &c in &zone.1 {
            rho[c] = card.rho;
            rho_e[c] = card.e;
            nu[c] = card.nu;
            alpha[c] = card.alpha;
            t_ref[c] = card.t_ref;
        }
    }
    let normalised = settings.normalisation == StiffnessNormalisation::UpstreamRhoNormalised;
    Ok((0..n_cells)
        .map(|i| {
            let e = if normalised { rho_e[i] / rho[i] } else { rho_e[i] };
            let mu = e / (2.0 * (1.0 + nu[i]));
            let (lambda, three_k) = if settings.plane_stress {
                (nu[i] * e / ((1.0 + nu[i]) * (1.0 - nu[i])), e / (1.0 - nu[i]))
            } else {
                (nu[i] * e / ((1.0 + nu[i]) * (1.0 - 2.0 * nu[i])), e / (1.0 - 2.0 * nu[i]))
            };
            CellMaterial {
                mu,
                lambda,
                three_k_alpha: three_k * alpha[i],
                t_ref: t_ref[i],
                stress_scale: if normalised { rho[i] } else { 1.0 },
            }
        })
        .collect())
}

/// Small-strain thermo-elasticity by finite elements; see the module documentation.
#[derive(Debug, Clone)]
pub struct FemThermoElastic {
    fv_mesh: Arc<FvMesh>,
    fem: FemMesh,
    dofs: DofMap,
    rule: Vec<QuadraturePoint>,
    materials: Vec<CellMaterial>,
    patches: Vec<FemPatch>,
    settings: FemSettings,
    stiffness: CsrMatrix,
    u: Vec<f64>,
    cell_disp: Vec<Vector3>,
    cell_stress: Vec<Voigt6>,
    cell_sigma_eq: Vec<f64>,
}

impl FemThermoElastic {
    /// Build the element mesh and assemble the stiffness matrix (independent of temperature).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] for a card without a cell zone, a patch list that does not match the mesh, an
    /// `empty` patch, or any refusal of [`FemMesh::build`].
    pub fn new(
        mesh: Arc<FvMesh>,
        topology: &FemTopology,
        settings: FemSettings,
        zones: &[(String, Vec<usize>)],
        cards: &[ZoneCard],
        patches: Vec<FemPatch>,
    ) -> Result<Self, AppBuilderError> {
        if patches.len() != mesh.patches.len() {
            return Err(AppBuilderError::Case {
                case: crate::path::PathBuf::new(),
                msg: "FEM thermo-elasticity: one patch condition per patch".to_string(),
            });
        }
        let fem = FemMesh::build(&mesh, topology, settings.element)?;
        let materials = cell_materials(mesh.n_cells, zones, cards, &settings)?;
        let dofs = DofMap::displacement(&fem.mesh);
        let rule = default_rule(fem.mesh.element_type());
        let pattern = SparsityPattern::from_mesh(&fem.mesh, &dofs);
        let mut stiffness = CsrMatrix::from_pattern(&pattern);
        let et = fem.mesh.element_type();
        let nn = et.n_nodes();
        let nd = 3 * nn;
        let mut ec = vec![[0.0_f64; 3]; nn];
        let mut ed = vec![0usize; nd];
        for e in 0..fem.mesh.n_elements() {
            let m = materials[fem.element_cell[e]];
            let d = Tensor4::isotropic(m.lambda + 2.0 * m.mu / 3.0, m.mu);
            fem.mesh.element_coords(ElemId(e), &mut ec);
            let mut ke = vec![0.0_f64; nd * nd];
            for q in &rule {
                let mp = map_gradients(et, &ec, q.xi).map_err(|x| AppBuilderError::Case {
                    case: crate::path::PathBuf::new(),
                    msg: format!("FEM: element {e}: {x}"),
                })?;
                BMatrix::from_gradients(&mp.grad, nn, 3).accumulate_stiffness(&d, q.weight * mp.det_j, &mut ke);
            }
            dofs.element_dofs(&fem.mesh, ElemId(e), &mut ed);
            stiffness.scatter(&ed, &ke).map_err(|x| AppBuilderError::Case {
                case: crate::path::PathBuf::new(),
                msg: format!("FEM: assembly: {x}"),
            })?;
        }
        let n_dofs = dofs.n_dofs();
        let n_cells = mesh.n_cells;
        Ok(Self {
            fv_mesh: mesh,
            fem,
            dofs,
            rule,
            materials,
            patches,
            settings,
            stiffness,
            u: vec![0.0; n_dofs],
            cell_disp: vec![Vector3::ZERO; n_cells],
            cell_stress: vec![Voigt6::ZERO; n_cells],
            cell_sigma_eq: vec![0.0; n_cells],
        })
    }

    /// The finite-element mesh (element type, node coordinates, element-to-cell map).
    pub fn fem_mesh(&self) -> &FemMesh {
        &self.fem
    }

    /// Nodal displacement (m), node-major, three components per node.
    pub fn nodal_displacement(&self) -> &[f64] {
        &self.u
    }

    /// Cell-averaged displacement (m): `integral(u dV) / V` over the elements of each cell.
    pub fn displacement(&self) -> &[Vector3] {
        &self.cell_disp
    }

    /// Cell-averaged stress (Pa, Voigt `xx yy zz yz xz xy`), scaled as `calculateStress.H` reports it.
    pub fn stress(&self) -> &[Voigt6] {
        &self.cell_stress
    }

    /// `sigmaEq` per cell (Pa): von Mises of [`Self::stress`].
    pub fn sigma_eq(&self) -> &[f64] {
        &self.cell_sigma_eq
    }

    fn dirichlet(&self) -> DirichletSet {
        let mut set = DirichletSet::new();
        for (pi, bc) in self.patches.iter().enumerate() {
            for &n in &self.fem.patch_nodes[pi] {
                match bc {
                    FemPatch::FixedValue(v) => {
                        set.fix(&self.dofs, n, 0, v.x);
                        set.fix(&self.dofs, n, 1, v.y);
                        set.fix(&self.dofs, n, 2, v.z);
                    }
                    FemPatch::FixedComponents(c) => {
                        for (k, val) in c.iter().enumerate() {
                            if let Some(v) = val {
                                set.fix(&self.dofs, n, k, *v);
                            }
                        }
                    }
                    FemPatch::TractionFree | FemPatch::Pressure(_) => {}
                }
            }
        }
        set
    }

    /// Solve for the displacement at the cell temperatures `t_struct` (K, one per cell).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if `t_struct` has the wrong length or the linear solve does not converge.
    pub fn solve(&mut self, t_struct: &[f64]) -> Result<FemSolveLog, AppBuilderError> {
        let err = |msg: String| AppBuilderError::Case { case: crate::path::PathBuf::new(), msg };
        let n_cells = self.fv_mesh.n_cells;
        if t_struct.len() != n_cells {
            return Err(err("FEM: one TStruct per cell".to_string()));
        }
        let et = self.fem.mesh.element_type();
        let nn = et.n_nodes();
        let nd = 3 * nn;
        let mut ec = vec![[0.0_f64; 3]; nn];
        let mut ed = vec![0usize; nd];
        let n_dofs = self.dofs.n_dofs();
        // load: thermal eigenstress on the elements, pressure on the patches
        let mut f = vec![0.0_f64; n_dofs];
        for e in 0..self.fem.mesh.n_elements() {
            let c = self.fem.element_cell[e];
            let m = self.materials[c];
            let s = m.three_k_alpha * (t_struct[c] - m.t_ref);
            let sigma_th = Voigt6::new(s, s, s, 0.0, 0.0, 0.0);
            self.fem.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.fem.mesh, ElemId(e), &mut ed);
            let mut fe = [0.0_f64; MAX_ELEM_DOFS];
            for q in &self.rule {
                let mp = map_gradients(et, &ec, q.xi).map_err(|x| err(format!("FEM: element {e}: {x}")))?;
                BMatrix::from_gradients(&mp.grad, nn, 3).accumulate_internal_force(&sigma_th, q.weight * mp.det_j, &mut fe);
            }
            for (a, &dof) in ed.iter().enumerate() {
                f[dof] += fe[a];
            }
        }
        for (pi, bc) in self.patches.iter().enumerate() {
            if let FemPatch::Pressure(p) = bc {
                for pf in &self.fem.patch_facets[pi] {
                    let sf = self.fv_mesh.face_area_vectors[pf.face];
                    let mag = sf.mag();
                    let t = [-p * sf.x / mag, -p * sf.y / mag, -p * sf.z / mag];
                    Traction::uniform(vec![pf.facet.clone()], t)
                        .accumulate(&self.fem.mesh, &self.dofs, &mut f)
                        .map_err(|x| err(format!("FEM: traction: {x}")))?;
                }
            }
        }
        let mut k = self.stiffness.clone();
        self.dirichlet()
            .apply(&mut k, &mut f, &[], DirichletMethod::Elimination)
            .map_err(|x| err(format!("FEM: Dirichlet: {x}")))?;
        let (u, rep) = solve_linear(&k, &f, &self.settings.linear).map_err(|x| err(format!("FEM: linear solve: {x}")))?;
        self.u = u;
        self.recover(t_struct)?;
        let max_sigma_eq = self.cell_sigma_eq.iter().copied().fold(f64::MIN, f64::max);
        Ok(FemSolveLog {
            n_dofs,
            n_elements: self.fem.mesh.n_elements(),
            element: et,
            n_iterations: rep.n_iterations,
            final_residual: rep.final_residual,
            max_sigma_eq,
        })
    }

    /// Volume-averaged displacement and stress per cell from the nodal solution.
    fn recover(&mut self, t_struct: &[f64]) -> Result<(), AppBuilderError> {
        let et = self.fem.mesh.element_type();
        let nn = et.n_nodes();
        let n_cells = self.fv_mesh.n_cells;
        let mut vol = vec![0.0_f64; n_cells];
        let mut disp = vec![[0.0_f64; 3]; n_cells];
        let mut stress = vec![Voigt6::ZERO; n_cells];
        let mut ec = vec![[0.0_f64; 3]; nn];
        let mut ed = vec![0usize; 3 * nn];
        let mut ue = [0.0_f64; MAX_ELEM_DOFS];
        for e in 0..self.fem.mesh.n_elements() {
            let c = self.fem.element_cell[e];
            let m = self.materials[c];
            let d = Tensor4::isotropic(m.lambda + 2.0 * m.mu / 3.0, m.mu);
            let s = m.three_k_alpha * (t_struct[c] - m.t_ref);
            let sigma_th = Voigt6::new(s, s, s, 0.0, 0.0, 0.0);
            self.fem.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.fem.mesh, ElemId(e), &mut ed);
            for (a, &dof) in ed.iter().enumerate() {
                ue[a] = self.u[dof];
            }
            for q in &self.rule {
                let mp = map_gradients(et, &ec, q.xi).map_err(|x| AppBuilderError::Case {
                    case: crate::path::PathBuf::new(),
                    msg: format!("FEM: element {e}: {x}"),
                })?;
                let w = q.weight * mp.det_j;
                let shape = et.shape_functions(q.xi);
                let mut ugp = [0.0_f64; 3];
                for a in 0..nn.min(MAX_ELEM_NODES) {
                    for i in 0..3 {
                        ugp[i] += shape[a] * ue[3 * a + i];
                    }
                }
                let eps = BMatrix::from_gradients(&mp.grad, nn, 3).strain(&ue[..3 * nn]);
                let sig = d.apply(&eps).minus(&sigma_th);
                vol[c] += w;
                for i in 0..3 {
                    disp[c][i] += w * ugp[i];
                }
                stress[c] = stress[c].plus(&sig.scaled(w));
            }
        }
        for c in 0..n_cells {
            let v = vol[c];
            if v > 0.0 {
                self.cell_disp[c] = Vector3::new(disp[c][0] / v, disp[c][1] / v, disp[c][2] / v);
                self.cell_stress[c] = stress[c].scaled(self.materials[c].stress_scale / v);
            } else {
                self.cell_disp[c] = Vector3::ZERO;
                self.cell_stress[c] = Voigt6::ZERO;
            }
            self.cell_sigma_eq[c] = self.cell_stress[c].von_mises();
        }
        Ok(())
    }

    /// The finite-element mesh as `farrer-park` sees it, for callers that evaluate a field at nodes.
    pub fn mesh(&self) -> &Mesh {
        &self.fem.mesh
    }

    /// Node `n`'s displacement (m).
    pub fn node_displacement(&self, n: NodeId) -> [f64; 3] {
        [self.u[3 * n.0], self.u[3 * n.0 + 1], self.u[3 * n.0 + 2]]
    }
}
