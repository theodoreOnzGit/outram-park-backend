// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! The linear solver a GeN-Foam neutronics equation runs, selected the way the
//! case's `fvSolution` selects it (gh:#1361 round 40, maintainer direction
//! 2026-11-27: "Match all upstream").
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream
//!
//! Every shipped GeN-Foam neutronics `fvSolution` selects the same three
//! solvers (`Tutorials/reactorCases/*/system/neutroRegion/fvSolution`):
//!
//! | equation (regex of the dictionary) | solver | preconditioner | ported by |
//! |---|---|---|---|
//! | `flux.*`, `fluxStar.*`, `fluxStar2.*` | `PCG` | `DIC` | [`super::pcg::pcg_dic`] |
//! | `prec.*`, `precStar.*` | `PBiCG` | `DILU` | `legacy_fvm::pbicg::pbicg_dilu` |
//! | `angularFlux.*` | `PBiCGStab` | `DILU` | [`pbicgstab_dilu`] (this file) |
//!
//! The `GAMG` block shipped next to the `PCG` one is commented out in every
//! tutorial, so no neutronics equation selects it. ~~A case that selects `GAMG`
//! has no faithful port (gh:#1843): it runs `PCG`/`DIC` here, a NAMED deviation
//! (round 21 did the same for the thermal-hydraulics `GAMG` entries).~~
//! **CORRECTED 2026-11-27 (gh:#1934, README round 67):** `GAMGSolver` with
//! `faceAreaPair` and `DIC` IS ported ([`super::gamg`], bit-identical to the
//! real binary's on the shipped 2D_FFTF `p_rgh` matrix) and wired into the
//! one-phase cell-centred region (`GamgPolicy::Native`); this neutronics
//! selector still has no `GAMG` branch (none is shipped), ~~and the
//! thermo-mechanics, two-phase and face-centred regions keep their named
//! `PCG`/`DIC` or `PBiCG`/`DILU` substitutes.~~ **CORRECTED 2026-11-27
//! (gh:#1936, README round 72):** the face-centred onePhase, the twoPhase and
//! the legacy thermo-mechanics (`TStruct`: `GaussSeidel`, the asymmetric path
//! with a `PBiCGStab`/`DILU` coarsest level) regions run upstream's own
//! `GAMGSolver` by default (`GamgPolicy::Native`, `TStructGamgPolicy::Native`);
//! the substitutes (`GamgPolicy::SubstitutePcgDic`,
//! `TStructGamgPolicy::SubstitutePbicgDilu`, `with_krylov_for_p_rgh`) remain as
//! named ablations. ~~`CaseReadOptions::default().gamg` was `GamgPolicy::Refuse`~~
//! **CHANGED 2026-11-27 (gh:#1947, README round 76):** it is `GamgPolicy::Native`
//! too; `AllrunEdits::default().gamg` stays `SubstitutePcgDic` because it is the
//! PROTOCOL of the committed MSFR references (`AllrunEdits::as_shipped_gamg()` keeps
//! `GAMG`). ~~The commented-out neutronics block (`DICGaussSeidel`,
//! `nPreSweeps 1`) is ported and verified against the real binary, but not
//! wired: nothing selects it.~~ **CHANGED 2026-11-27 (gh:#1945, README round
//! 79):** a flux entry that selects `GAMG` runs upstream's own `GAMGSolver`
//! ([`NeutronSolverKind::Gamg`], any ported smoother incl. `DICGaussSeidel` and
//! `nPreSweeps`) by default; `PCG`/`DIC` in its place is the named ablation
//! (`GamgPolicy::SubstitutePcgDic` of `io::nuclear_data::resolve_neutron_fv_solution_with`).
//!
//! Each runs OpenFOAM's `normFactor`-normalised residual, `tolerance`,
//! `relTol`, `maxIter` and `minIter`, and the zero-iteration exit when the
//! initial residual is already below `tolerance`.
//!
//! The earlier form, basic-lib's `||r||/||b||` CG and ILU(0)-BiCGStab
//! (shared crate, not modified), stays callable as
//! [`LinearSolverForm::BasicLib`], deprecated: it stops at a different point
//! and so differs from the binary at round-off, and does not skip a solve the
//! binary skips.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::{
    Field, FvMatrix, KrylovOptions, LduMatrix, PatchField, SolverSettings, VolScalarField,
};

use super::gamg::{face_area_pair_weights, gamg_dic_solve, pair_agglomerate, GamgControls, GamgPerformance};
use outram_foam_basic_lib::mesh::FvMesh;
use super::pcg::pcg_dic;
use crate::genfoam::thermal_hydraulics::solver::two_phase::pbicgstab::{converged, g_sum_mag, g_sum_prod, norm_factor};
use crate::genfoam::thermo_mechanics::legacy_fvm::pbicg::{dilu_precondition, dilu_reciprocal_diag, pbicg_dilu};


/// `VSMALL` for doubles.
const VSMALL: f64 = 1e-300;

/// Which implementation runs the neutronics linear solves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinearSolverForm {
    /// OpenFOAM's solvers, as the case's `fvSolution` selects them, with the
    /// normalised residual and the `relTol` / `minIter` entries (default, the
    /// upstream form). `rel_tol` 0 and `min_iter` 0 are the tight protocol of
    /// the V&V runs (`tolerance 1e-14; relTol 0;`).
    Upstream {
        /// `relTol` of the `solvers` entry.
        rel_tol: f64,
        /// `minIter` of the `solvers` entry.
        min_iter: usize,
    },
    /// OpenFOAM's solvers with every key of the case's `fvSolution` `solvers` entries read
    /// (gh:#1887 round 42): per equation family the solver and preconditioner words, `tolerance`,
    /// `relTol`, `maxIter` and `minIter`, defaults where an entry omits a key. Built by
    /// `io::nuclear_data::read_neutron_fv_solution`; the `tolerance` / `max_iter` of
    /// [`LinearSolve::settings`] are not used in this form.
    UpstreamFvSolution(NeutronSolverEntries),
    /// Deprecated: basic-lib's `||r||/||b||` CG for the symmetric equations,
    /// ILU(0)-BiCGStab for the asymmetric ones (and Gauss-Seidel for the
    /// static SN sweep). The form before gh:#1361 round 40.
    BasicLib,
}

impl Default for LinearSolverForm {
    fn default() -> Self {
        Self::Upstream { rel_tol: 0.0, min_iter: 0 }
    }
}

/// The OpenFOAM linear solver and preconditioner pair an `fvSolution` entry selects (the pairs the port
/// translates; any other selection is refused by name when the entry is read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronSolverKind {
    /// `PCG` with `DIC` ([`super::pcg::pcg_dic`]); symmetric matrices.
    PcgDic,
    /// `PBiCG` with `DILU` (`legacy_fvm::pbicg::pbicg_dilu`); asymmetric matrices.
    PbicgDilu,
    /// `PBiCGStab` with `DILU` ([`pbicgstab_dilu`]); asymmetric matrices.
    PbicgstabDilu,
    /// `GAMG` (upstream's `GAMGSolver`, [`super::gamg`]) with the controls in [`NeutronSolverEntry::gamg`] (gh:#1945, README round 79).
    Gamg,
}

/// One `solvers` entry of `fvSolution` as OpenFOAM's `lduMatrix::solver::readControls` reads it
/// (OpenFOAM-v2506 `lduMatrixSolver.C:195-208`): `tolerance` default `lduMatrix::defaultTolerance`
/// (1e-6), `relTol` 0, `maxIter` `lduMatrix::defaultMaxIter` (1000), `minIter` 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutronSolverEntry {
    /// The solver and preconditioner.
    pub kind: NeutronSolverKind,
    /// `tolerance`.
    pub tolerance: f64,
    /// `relTol`.
    pub rel_tol: f64,
    /// `maxIter`.
    pub max_iter: usize,
    /// `minIter`.
    pub min_iter: usize,
    /// The `GAMG` keys of the entry (smoother, sweeps, `nCellsInCoarsestLevel`, ...), read only when `kind` is
    /// [`NeutronSolverKind::Gamg`]; its `tolerance`/`relTol`/`maxIter`/`minIter` equal the four fields above (gh:#1945).
    pub gamg: GamgControls,
    /// `pairGAMGAgglomeration::forward_` when the mesh's `GAMG` agglomeration was (last) built (gh:#1956, README round 82): `true` in a
    /// process whose first `GAMG` agglomeration is this one (neutronics-only and EN stages); in a multi-region run `RegionSolvers` sets it
    /// from its one process-wide value at each (re)build. Read only when `kind` is [`NeutronSolverKind::Gamg`].
    pub gamg_forward: bool,
}

impl NeutronSolverEntry {
    /// An entry of kind `kind` with every control at OpenFOAM's default.
    #[must_use]
    pub fn with_defaults(kind: NeutronSolverKind) -> Self {
        Self { kind, tolerance: 1e-6, rel_tol: 0.0, max_iter: 1000, min_iter: 0, gamg: GamgControls::default(), gamg_forward: true }
    }

    /// A `GAMG` entry with the controls `ctl` (its `tolerance`, `relTol`, `maxIter` and `minIter` become the entry's).
    #[must_use]
    pub fn gamg(ctl: GamgControls) -> Self {
        Self { kind: NeutronSolverKind::Gamg, tolerance: ctl.tolerance, rel_tol: ctl.rel_tol, max_iter: ctl.max_iter, min_iter: ctl.min_iter, gamg: ctl, gamg_forward: true }
    }
}

/// The entries the three equation families of the neutronics resolve to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutronSolverEntries {
    /// `flux.*` family (`flux<g>`, `fluxStar<g>`, `fluxStar2<g>`).
    pub flux: NeutronSolverEntry,
    /// `prec.*` family (`prec<k>`, `precStar<k>`).
    pub precursor: NeutronSolverEntry,
    /// `angularFlux.*` family (`angularFlux_<g>_<d>`).
    pub angular_flux: NeutronSolverEntry,
}

impl NeutronSolverEntries {
    /// The entry `role` solves with (the static SN sweep solves the angular-flux matrix).
    #[must_use]
    pub fn entry_for(&self, role: NeutronEquation) -> NeutronSolverEntry {
        match role {
            NeutronEquation::Flux => self.flux,
            NeutronEquation::Precursor => self.precursor,
            NeutronEquation::AngularFlux | NeutronEquation::StaticAngularFlux => self.angular_flux,
        }
    }
}

/// The kind of equation being solved, which fixes the solver the case selects
/// for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronEquation {
    /// `flux.*` family: symmetric, `PCG`/`DIC`.
    Flux,
    /// `prec.*` family: asymmetric (upwind transport), `PBiCG`/`DILU`.
    Precursor,
    /// `angularFlux.*`: asymmetric (upwind streaming), `PBiCGStab`/`DILU`.
    AngularFlux,
    /// The static SN source-iteration sweep of the port: the same matrix as
    /// [`Self::AngularFlux`]; the deprecated form solved it from zero with
    /// Gauss-Seidel.
    StaticAngularFlux,
}

/// `tolerance` and `maxIter` plus the [`LinearSolverForm`] that interprets them.
#[derive(Debug, Clone, Copy)]
pub struct LinearSolve {
    /// `tolerance` and `maxIter` of the `solvers` entry (OpenFOAM's default
    /// `maxIter` is 1000).
    pub settings: SolverSettings,
    /// The implementation.
    pub form: LinearSolverForm,
}

impl LinearSolve {
    /// A solve with the given limits in the form `form`.
    #[must_use]
    pub fn new(settings: SolverSettings, form: LinearSolverForm) -> Self {
        Self { settings, form }
    }

    /// Solve `eqn` for `name`, starting from `start` (the solver's initial guess), and return the
    /// solution on the template's mesh with zero-gradient patches, as basic-lib's solves do.
    #[must_use]
    pub fn solve(&self, role: NeutronEquation, eqn: &FvMatrix, name: &str, start: &VolScalarField) -> VolScalarField {
        self.solve_recorded(role, eqn, name, start).0
    }

    /// [`Self::solve`], and the solve's [`LinearSolveRecord`] (what upstream's `lduMatrix::solve` prints as `Solving for <name>, Initial
    /// residual = .., No Iterations ..`; gh:#1966, README round 86) for the upstream forms; `None` for basic-lib's solvers, whose residual
    /// normalisation is not OpenFOAM's. The same solve, operation for operation, as [`Self::solve`] (which calls this).
    #[must_use]
    pub fn solve_recorded(&self, role: NeutronEquation, eqn: &FvMatrix, name: &str, start: &VolScalarField) -> (VolScalarField, Option<LinearSolveRecord>) {
        match self.form {
            LinearSolverForm::BasicLib => (
                match role {
                    NeutronEquation::Flux => eqn.solve_cg_with_guess(name.to_string(), start, self.settings).0,
                    NeutronEquation::Precursor | NeutronEquation::AngularFlux => {
                        eqn.solve_bicgstab_with_guess(name.to_string(), start, KrylovOptions::default(), self.settings).0
                    }
                    NeutronEquation::StaticAngularFlux => eqn.solve(name.to_string(), self.settings).0,
                },
                None,
            ),
            LinearSolverForm::Upstream { rel_tol, min_iter } => {
                let kind = match role {
                    NeutronEquation::Flux => NeutronSolverKind::PcgDic,
                    NeutronEquation::Precursor => NeutronSolverKind::PbicgDilu,
                    NeutronEquation::AngularFlux | NeutronEquation::StaticAngularFlux => NeutronSolverKind::PbicgstabDilu,
                };
                let entry = NeutronSolverEntry {
                    kind,
                    tolerance: self.settings.tolerance,
                    rel_tol,
                    max_iter: self.settings.max_iter,
                    min_iter,
                    gamg: GamgControls::default(),
                    gamg_forward: true,
                };
                let (field, record) = run_entry(eqn, name, start, entry);
                (field, Some(record))
            }
            LinearSolverForm::UpstreamFvSolution(entries) => {
                let (field, record) = run_entry(eqn, name, start, entries.entry_for(role));
                (field, Some(record))
            }
        }
    }
}

/// One linear solve as upstream's log line `Solving for <field>, Initial residual = <r>, Final residual = .., No Iterations <n>` reports
/// it (gh:#1966, README round 86): the field name, the normalised initial residual and the iterations (V-cycles for `GAMG`).
#[derive(Debug, Clone, PartialEq)]
pub struct LinearSolveRecord {
    /// The solved field's name (`fluxStar0`, `flux1`, ...).
    pub field: String,
    /// The initial residual in OpenFOAM's normalisation.
    pub initial_residual: f64,
    /// Iterations performed.
    pub iterations: usize,
}

/// Run the OpenFOAM solver `entry` selects on `eqn`, from `start`, and return the solution on the template's mesh with
/// zero-gradient patches, as basic-lib's solves do, and the solve's [`LinearSolveRecord`] (gh:#1966, README round 86: the
/// performance each solver returns, which this function dropped before).
fn run_entry(eqn: &FvMatrix, name: &str, start: &VolScalarField, entry: NeutronSolverEntry) -> (VolScalarField, LinearSolveRecord) {
    let b: Vec<f64> = eqn.source.iter().copied().collect();
    let mut x: Vec<f64> = start.internal.as_slice().to_vec();
    let (tol, rel_tol, max_iter, min_iter) = (entry.tolerance, entry.rel_tol, entry.max_iter, entry.min_iter);
    let (initial_residual, iterations) = match entry.kind {
        NeutronSolverKind::PcgDic => {
            let p = pcg_dic(&eqn.ldu, &b, &mut x, tol, rel_tol, max_iter, min_iter);
            (p.initial_residual, p.iterations)
        }
        NeutronSolverKind::PbicgDilu => {
            let p = pbicg_dilu(&eqn.ldu, &b, &mut x, tol, rel_tol, max_iter, min_iter);
            (p.initial_residual, p.iterations)
        }
        NeutronSolverKind::PbicgstabDilu => {
            let p = pbicgstab_dilu(&eqn.ldu, &b, &mut x, tol, rel_tol, max_iter, min_iter);
            (p.initial_residual, p.iterations)
        }
        NeutronSolverKind::Gamg => {
            let ctl = GamgControls { tolerance: tol, rel_tol, max_iter, min_iter, ..entry.gamg };
            match gamg_solve_on_mesh_from(eqn, &ctl, entry.gamg_forward, &b, &mut x) {
                Ok(p) => (p.initial_residual, p.iterations),
                // upstream stops with a FatalError on these selections; the readers refuse them before any solve, so reaching this is a bug
                Err(e) => panic!("{name}: {e}"),
            }
        }
    };
    let boundary = eqn.mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect();
    (
        VolScalarField::new(name.to_string(), eqn.mesh.clone(), Field::new(x), boundary),
        LinearSolveRecord { field: name.to_string(), initial_residual, iterations },
    )
}

/// One `GAMG` solve of `eqn` (gh:#1945, README round 79): the `faceAreaPair` agglomeration of `eqn`'s mesh with `ctl`, then
/// [`gamg_dic_solve`]. `x` is the initial guess and receives the solution.
///
/// Upstream builds the agglomeration once per mesh (`GAMGAgglomeration::New`, the mesh registry) on the first `GAMG` solve, with
/// `pairGAMGAgglomeration::forward_` as the process left it, and rebuilds it only after `movePoints` (`requireUpdate_`). Here it is
/// built at every solve with `forward_ = true`: the same agglomeration, value for value, for a mesh that does not move in a process
/// whose first `GAMG` agglomeration is this mesh's (every flux solve of one region resolves to the one `flux.*` entry). ~~A run where
/// another region agglomerates first, or where the neutronics mesh moves, is refused by name by the readers that can tell
/// (`multi_region::region_solvers`), not run here differently.~~ **CHANGED 2026-11-28 (gh:#1956, README round 82):** such a run goes
/// through [`gamg_solve_on_mesh_from`] with the `forward_` start `RegionSolvers` hands in ([`NeutronSolverEntry::gamg_forward`]), fixed
/// at each (re)build: the process-wide value after the other regions' builds, and again after each `deformMesh`.
///
/// # Errors
/// An unsupported control, an asymmetric matrix with a `DIC`-type smoother, or a matrix too small for a coarse level (upstream:
/// `FatalError`).
pub fn gamg_solve_on_mesh(eqn: &FvMatrix, ctl: &GamgControls, source: &[f64], x: &mut [f64]) -> Result<GamgPerformance, String> {
    gamg_solve_on_mesh_from(eqn, ctl, true, source, x)
}

/// [`gamg_solve_on_mesh`] with the agglomeration built from `pairGAMGAgglomeration::forward_ = forward` (gh:#1956, README round 82): the
/// value the process held when upstream built (or, after `movePoints`, rebuilt) this mesh's cached agglomeration. Built per solve: the
/// same agglomeration, value for value, as upstream's cached one, as long as the mesh has not moved since that build (the caller,
/// `RegionSolvers`, hands in a new `forward` at each rebuild).
///
/// # Errors
/// As [`gamg_solve_on_mesh`].
pub fn gamg_solve_on_mesh_from(eqn: &FvMatrix, ctl: &GamgControls, forward: bool, source: &[f64], x: &mut [f64]) -> Result<GamgPerformance, String> {
    let ni = eqn.ldu.owner.len();
    let sf: Vec<[f64; 3]> = eqn.mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect();
    let w = face_area_pair_weights(&sf);
    let mut forward = forward;
    let agg = pair_agglomerate(eqn.ldu.n_cells, &eqn.ldu.owner, &eqn.ldu.neighbour, &w, ctl, &mut forward)?;
    gamg_dic_solve(&eqn.ldu, &agg, ctl, source, x)
}

/// `pairGAMGAgglomeration::forward_` after building the `faceAreaPair` agglomeration of `mesh` with `ctl` from `forward` (each level
/// attempted toggles it, `pairGAMGAgglomerate.C:333`): what a build of this mesh leaves for the next one in the process (gh:#1956,
/// README round 82). The same build as [`gamg_solve_on_mesh_from`]'s (the matrix's `owner`/`neighbour` are the mesh's internal faces).
///
/// # Errors
/// As [`pair_agglomerate`].
pub fn gamg_forward_after_build(mesh: &FvMesh, ctl: &GamgControls, forward: bool) -> Result<bool, String> {
    let ni = mesh.n_internal_faces;
    let sf: Vec<[f64; 3]> = mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect();
    let w = face_area_pair_weights(&sf);
    let mut forward = forward;
    let _ = pair_agglomerate(mesh.n_cells, &mesh.owner[..ni], &mesh.neighbour[..ni], &w, ctl, &mut forward)?;
    Ok(forward)
}

/// Result of a solve, as OpenFOAM's `solverPerformance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbicgstabPerformance {
    /// Iterations performed.
    pub iterations: usize,
    /// Normalised residual before the first iteration.
    pub initial_residual: f64,
    /// Normalised residual at exit.
    pub final_residual: f64,
}

/// `PBiCGStab::scalarSolve` with `preconditioner DILU`, translated operation for operation from
/// OpenFOAM-v2506 `PBiCGStab.C` and `DILUPreconditioner.C` (the `diagonal` form is
/// `two_phase::pbicgstab::pbicgstab_diagonal`, which this repeats with the preconditioner swapped).
/// `psi` is the initial guess and receives the solution.
pub(crate) fn pbicgstab_dilu(
    a: &LduMatrix,
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> PbicgstabPerformance {
    let n = psi.len();
    let ya0 = a.multiply(psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - ya0[c]).collect();
    let norm = norm_factor(a, psi, source, &ya0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut perf = PbicgstabPerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if min_iter == 0 && converged(perf.final_residual, initial, tolerance, rel_tol) {
        return perf;
    }

    let r_a0 = r_a.clone();
    let mut p_a = vec![0.0; n];
    let mut y_a = vec![0.0; n];
    let mut ay_a = vec![0.0; n];
    let mut s_a = vec![0.0; n];
    let mut z_a = vec![0.0; n];
    let mut r_a0r_a = 0.0_f64;
    let mut alpha = 0.0_f64;
    let mut omega = 0.0_f64;
    let r_d = dilu_reciprocal_diag(a);

    loop {
        let r_a0r_a_old = r_a0r_a;
        r_a0r_a = g_sum_prod(&r_a0, &r_a);
        if r_a0r_a.abs() < VSMALL {
            break;
        }
        if perf.iterations == 0 {
            p_a.copy_from_slice(&r_a);
        } else {
            if omega.abs() < VSMALL {
                break;
            }
            let beta = (r_a0r_a / r_a0r_a_old) * (alpha / omega);
            for c in 0..n {
                p_a[c] = r_a[c] + beta * (p_a[c] - omega * ay_a[c]);
            }
        }
        dilu_precondition(a, &r_d, &p_a, &mut y_a);
        ay_a = a.multiply(&y_a);
        let r_a0ay_a = g_sum_prod(&r_a0, &ay_a);
        alpha = r_a0r_a / r_a0ay_a;
        for c in 0..n {
            s_a[c] = r_a[c] - alpha * ay_a[c];
        }
        perf.final_residual = g_sum_mag(&s_a) / norm;
        if perf.iterations >= min_iter && converged(perf.final_residual, initial, tolerance, rel_tol) {
            for c in 0..n {
                psi[c] += alpha * y_a[c];
            }
            perf.iterations += 1;
            return perf;
        }
        dilu_precondition(a, &r_d, &s_a, &mut z_a);
        let t_a = a.multiply(&z_a);
        let t_at_a = g_sum_prod(&t_a, &t_a);
        omega = g_sum_prod(&t_a, &s_a) / t_at_a;
        for c in 0..n {
            psi[c] += alpha * y_a[c] + omega * z_a[c];
            r_a[c] = s_a[c] - omega * t_a[c];
        }
        perf.final_residual = g_sum_mag(&r_a) / norm;
        perf.iterations += 1;
        let go_on = (perf.iterations < max_iter && !converged(perf.final_residual, initial, tolerance, rel_tol))
            || perf.iterations < min_iter;
        if !go_on {
            break;
        }
    }
    perf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A non-symmetric upwind-like chain with exact solution `0.5^i`.
    fn chain(n: usize) -> (LduMatrix, Vec<f64>, Vec<f64>) {
        let nf = n - 1;
        let owner: Vec<usize> = (0..nf).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neighbour);
        a.diag = vec![3.0; n];
        a.lower = vec![-1.2; nf];
        a.upper = vec![-0.7; nf];
        let x: Vec<f64> = (0..n).map(|i| 0.5_f64.powi(i as i32)).collect();
        let b = a.multiply(&x);
        (a, b, x)
    }

    /// PBiCGStab/DILU reaches a tight tolerance, exits with zero iterations from a converged guess, and `minIter` forces iterations.
    #[test]
    fn pbicgstab_dilu_solves_exits_early_and_honours_min_iter() {
        let (a, b, x_exact) = chain(30);
        let mut x = vec![0.0; 30];
        let perf = pbicgstab_dilu(&a, &b, &mut x, 1e-13, 0.0, 1000, 0);
        assert!(perf.final_residual < 1e-13 && perf.iterations >= 1);
        for i in 0..30 {
            assert!((x[i] - x_exact[i]).abs() < 1e-10, "cell {i}");
        }
        let mut y = x_exact.clone();
        assert_eq!(pbicgstab_dilu(&a, &b, &mut y, 1e-10, 0.0, 1000, 0).iterations, 0);
        assert_eq!(y, x_exact);
        let mut z = vec![0.0; 30];
        let free = pbicgstab_dilu(&a, &b, &mut z, 1e-3, 0.0, 1000, 0);
        let mut w = vec![0.0; 30];
        let forced = pbicgstab_dilu(&a, &b, &mut w, 1e-3, 0.0, 1000, free.iterations + 2);
        assert!(forced.iterations >= free.iterations + 2);
    }

    /// `LinearSolve::solve` in both forms and every role on one system: each form converges to the same solution (to the
    /// solver tolerance), the upstream form does zero iterations from a converged start (the field is returned unchanged),
    /// and the deprecated form is selected by `LinearSolverForm::BasicLib`.
    #[test]
    fn linear_solve_forms_agree_and_the_upstream_form_skips_a_converged_start() {
        use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length};
        use uom::si::length::meter;
        let n = 30;
        let mesh = alloc::sync::Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n as i64).expect("mesh"),
        );
        let (ldu, b, x_exact) = chain(n);
        let eqn = FvMatrix { mesh: mesh.clone(), ldu, source: Field::new(b) };
        let zero = VolScalarField::uniform("x0", mesh.clone(), 0.0);
        let settings = SolverSettings { tolerance: 1e-13, max_iter: 10_000 };
        for role in [NeutronEquation::Precursor, NeutronEquation::AngularFlux, NeutronEquation::StaticAngularFlux] {
            let up = LinearSolve::new(settings, LinearSolverForm::default()).solve(role, &eqn, "x", &zero);
            let old = LinearSolve::new(settings, LinearSolverForm::BasicLib).solve(role, &eqn, "x", &zero);
            for i in 0..n {
                assert!((up.internal.as_slice()[i] - x_exact[i]).abs() < 1e-9, "{role:?} upstream cell {i}");
                assert!((old.internal.as_slice()[i] - x_exact[i]).abs() < 1e-6, "{role:?} basic-lib cell {i}");
            }
        }
        let mut exact = VolScalarField::uniform("x1", mesh.clone(), 0.0);
        exact.internal = Field::new(x_exact.clone());
        let again = LinearSolve::new(settings, LinearSolverForm::default()).solve(NeutronEquation::Precursor, &eqn, "x", &exact);
        assert_eq!(again.internal.as_slice(), x_exact.as_slice());
    }

    /// The flux role on a symmetric system, both forms.
    #[test]
    fn flux_role_solves_a_symmetric_system_in_both_forms() {
        use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length};
        use uom::si::length::meter;
        let n = 20;
        let mesh = alloc::sync::Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n as i64).expect("mesh"),
        );
        let (mut ldu, _, _) = chain(n);
        ldu.upper = vec![-1.0; n - 1];
        ldu.lower = vec![-1.0; n - 1];
        ldu.diag = vec![3.0; n];
        let x_exact: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let b = ldu.multiply(&x_exact);
        let eqn = FvMatrix { mesh: mesh.clone(), ldu, source: Field::new(b) };
        let zero = VolScalarField::uniform("x0", mesh, 0.0);
        let settings = SolverSettings { tolerance: 1e-13, max_iter: 10_000 };
        for form in [LinearSolverForm::default(), LinearSolverForm::BasicLib] {
            let x = LinearSolve::new(settings, form).solve(NeutronEquation::Flux, &eqn, "x", &zero);
            for i in 0..n {
                assert!((x.internal.as_slice()[i] - x_exact[i]).abs() < 1e-8, "{form:?} cell {i}");
            }
        }
    }

    /// gh:#1956 (README round 82): the `forward_` a build leaves is the start toggled once per level attempted, the same as the solve's own
    /// build; a start of `false` builds a different agglomeration (cells paired from the other end), and the solve from either start
    /// converges to the same solution.
    #[test]
    fn gamg_forward_after_build_matches_the_solves_build() {
        use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length};
        use uom::si::length::meter;
        let n = 41;
        let mesh = alloc::sync::Arc::new(create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n as i64).expect("mesh"));
        let (mut ldu, _, _) = chain(n);
        ldu.upper = vec![-1.0; n - 1];
        ldu.lower = vec![-1.0; n - 1];
        ldu.diag = vec![2.5; n];
        let ctl = GamgControls { n_cells_in_coarsest_level: 2, tolerance: 1e-14, ..GamgControls::default() };
        let ni = mesh.n_internal_faces;
        let sf: Vec<[f64; 3]> = mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect();
        let w = face_area_pair_weights(&sf);
        for start in [true, false] {
            let mut f = start;
            let agg = pair_agglomerate(n, &ldu.owner, &ldu.neighbour, &w, &ctl, &mut f).expect("agglomerate");
            assert!(!agg.levels.is_empty());
            assert_eq!(gamg_forward_after_build(&mesh, &ctl, start).expect("build"), f, "start {start}");
        }
        let (mut a, mut b) = (true, false);
        let (aa, bb) = (
            pair_agglomerate(n, &ldu.owner, &ldu.neighbour, &w, &ctl, &mut a).expect("t"),
            pair_agglomerate(n, &ldu.owner, &ldu.neighbour, &w, &ctl, &mut b).expect("f"),
        );
        assert_ne!(format!("{:?}", aa.levels[0]), format!("{:?}", bb.levels[0]), "forward and backward pairings differ on an odd chain");
        let x_exact: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
        let src = ldu.multiply(&x_exact);
        let eqn = FvMatrix { mesh: mesh.clone(), ldu, source: Field::new(src.clone()) };
        for start in [true, false] {
            let mut x = vec![0.0; n];
            let _ = gamg_solve_on_mesh_from(&eqn, &ctl, start, &src, &mut x).expect("solve");
            for i in 0..n {
                assert!((x[i] - x_exact[i]).abs() < 1e-10, "start {start} cell {i}");
            }
        }
    }

    /// The default form is the upstream one with the tight protocol's `relTol 0`, `minIter 0`.
    #[test]
    fn default_form_is_upstream() {
        assert_eq!(LinearSolverForm::default(), LinearSolverForm::Upstream { rel_tol: 0.0, min_iter: 0 });
    }
}
