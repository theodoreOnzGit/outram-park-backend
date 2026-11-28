// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! The `GAMG` `p_rgh` solver of a GeN-Foam fluid region: the entries of `fvSolution` that select it, and the cached agglomeration
//! (gh:#1934, README round 67). The solver itself is [`crate::genfoam::common::gamg`].
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Upstream: `GAMGAgglomeration::New(matrix, controlDict)` stores the agglomeration in the mesh registry under the name
//! `GAMGAgglomeration` the first time a `GAMG` solver is constructed on the mesh, with THAT solver's dictionary, and every later
//! `GAMG` solve on the mesh (`p_rgh` and `p_rghFinal` alike) reuses it. [`GamgSolveState`] is that cache; `forward_` (a process-wide
//! static upstream) starts `true` in each state, which is what a process whose first `GAMG` agglomeration is this mesh's has.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::fv::Geometry;
use crate::genfoam::common::gamg::{
    algebraic_pair_weights, face_area_pair_weights_at, gamg_dic_solve, gamg_dic_solve_if, gamg_dic_solve_without_coarse_interfaces, matrix_has_lower, pair_agglomerate,
    GamgAgglomeration, GamgAgglomerator, GamgControls, GamgPerformance, GamgSmoother,
};
use outram_foam_basic_lib::prelude::LduMatrix;
use crate::genfoam::common::ldu_interfaces::LduInterfaces;

/// The cached agglomeration of one mesh and the `forward_` toggle.
#[derive(Debug, Clone)]
pub struct GamgSolveState {
    agg: Option<GamgAgglomeration>,
    forward: bool,
    /// Every solve since the last [`Self::take_log`]: `(initial residual, residual after each V-cycle)`.
    log: Vec<(f64, Vec<f64>)>,
    /// `false` only for the CONTROL [`Self::without_coarse_interfaces`] (gh:#1905 round 80, Q9).
    coarse_interfaces: bool,
    /// `mesh.time().timeIndex()` of the next build (gh:#1946: `updateInterval` decides the `faceAreaPair` weights from it). Default 1,
    /// the first step of a run started at time index 0; a solver hands in its own with [`Self::set_time_index`].
    time_index: i64,
}

impl Default for GamgSolveState {
    fn default() -> Self {
        Self { agg: None, forward: true, log: Vec::new(), coarse_interfaces: true, time_index: 1 }
    }
}

impl GamgSolveState {
    /// Hand in `mesh.time().timeIndex()` before the solves of a time step (gh:#1946, README round 85): the build of the agglomeration
    /// (the first solve, or every solve under `cacheAgglomeration false`) takes its `faceAreaPair` weights from `updateInterval` and this
    /// index ([`face_area_pair_weights_at`]).
    pub fn set_time_index(&mut self, time_index: i64) {
        self.time_index = time_index;
    }

    /// The `GAMGAgglomeration` the constructor of a `GAMG` solver with `ctl` builds for `ldu` (`internal_face_areas` gives `Sf` of the
    /// internal faces, read only by `faceAreaPair`).
    fn build<F: FnOnce() -> Vec<[f64; 3]>>(&mut self, internal_face_areas: F, ldu: &LduMatrix, ctl: &GamgControls) -> Result<GamgAgglomeration, String> {
        let w = match ctl.agglomerator {
            GamgAgglomerator::FaceAreaPair => face_area_pair_weights_at(&internal_face_areas(), ctl.update_interval, self.time_index),
            GamgAgglomerator::AlgebraicPair => algebraic_pair_weights(ldu, matrix_has_lower(ldu, ctl.has_lower)),
        };
        pair_agglomerate(ldu.n_cells, &ldu.owner, &ldu.neighbour, &w, ctl, &mut self.forward)
    }

    /// `~GAMGSolver`: with `cacheAgglomeration false` the agglomeration is deleted and the next solve builds it again (gh:#1946).
    fn after_solve(&mut self, ctl: &GamgControls) {
        if !ctl.cache_agglomeration {
            self.agg = None;
        }
    }
    /// `pairGAMGAgglomeration::forward_` as this state holds it: the value its next agglomeration starts from, or (once built) the value
    /// its build left (gh:#1956, README round 82).
    #[must_use]
    pub fn forward(&self) -> bool {
        self.forward
    }

    /// Hand in the process-wide `forward_` before a region's solves ([`crate::genfoam::multi_region::region_solvers::RegionSolvers`] keeps
    /// ONE for every region, as upstream's static is one per process); read it back with [`Self::forward`] after them. A state that is
    /// already built does not use it (its agglomeration is cached), so the value then passes through unchanged.
    pub fn set_forward(&mut self, forward: bool) {
        self.forward = forward;
    }

    /// Whether the agglomeration is built (cached).
    #[must_use]
    pub fn is_built(&self) -> bool {
        self.agg.is_some()
    }

    /// Drop the cached agglomeration (a new process, or `GAMGAgglomeration::requireUpdate_` after `movePoints`): the next solve rebuilds it
    /// from the `forward_` handed in.
    pub fn invalidate(&mut self) {
        self.agg = None;
    }

    /// Number of agglomeration levels once built (0 before the first solve).
    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.agg.as_ref().map_or(0, |a| a.levels.len())
    }

    /// The solves since the last call, `(initial residual, residual after each V-cycle)` (what upstream's instrumented copy dumps).
    pub fn take_log(&mut self) -> Vec<(f64, Vec<f64>)> {
        core::mem::take(&mut self.log)
    }

    /// One `GAMG` solve of `ldu x = source` from the guess in `x`; the first call builds the `faceAreaPair` agglomeration of `g`'s
    /// mesh with `ctl` (upstream: the dictionary of the first `GAMG` solver constructed).
    ///
    /// # Panics
    /// On an asymmetric matrix or an unsupported control (the reader refuses those by name before a run).
    pub fn solve(&mut self, g: &Geometry, ldu: &LduMatrix, source: &[f64], x: &mut [f64], ctl: &GamgControls) -> GamgPerformance {
        self.solve_if(g, ldu, &LduInterfaces::none(), source, x, ctl)
    }

    /// CONTROL, NOT upstream (gh:#1905, README "Round 80", prediction Q9): later solves keep the coupled interfaces on the finest level
    /// only ([`crate::genfoam::common::gamg::gamg_dic_solve_without_coarse_interfaces`]).
    #[must_use]
    pub fn without_coarse_interfaces(mut self) -> Self {
        self.coarse_interfaces = false;
        self
    }

    /// [`Self::solve`] of a matrix with coupled (`cyclic`) interfaces `ifs` (gh:#1905 round 80): the agglomeration is that of the internal
    /// faces (as upstream's), the interfaces are carried on every level ([`gamg_dic_solve_if`]).
    ///
    /// # Panics
    /// As [`Self::solve`].
    pub fn solve_if(&mut self, g: &Geometry, ldu: &LduMatrix, ifs: &LduInterfaces, source: &[f64], x: &mut [f64], ctl: &GamgControls) -> GamgPerformance {
        if self.agg.is_none() {
            let ni = g.n_internal();
            let sf = || -> Vec<[f64; 3]> { g.mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect() };
            let agg = match self.build(sf, ldu, ctl) {
                Ok(a) => a,
                // the readers refuse every unsupported control by name before a run
                Err(e) => panic!("{e}"),
            };
            self.agg = Some(agg);
        }
        let agg = self.agg.as_ref().expect("built above");
        let perf = if self.coarse_interfaces {
            gamg_dic_solve_if(ldu, ifs, agg, ctl, source, x)
        } else {
            gamg_dic_solve_without_coarse_interfaces(ldu, ifs, agg, ctl, source, x)
        }
        .unwrap_or_else(|e| panic!("{e}"));
        self.log.push((perf.initial_residual, perf.cycles.clone()));
        self.after_solve(ctl);
        perf
    }
}

impl GamgSolveState {
    /// [`Self::solve`] for a caller that has no [`Geometry`] (the thermo-mechanics region): `internal_face_areas` supplies the `Sf` of the internal
    /// faces and is called only for the first solve (the agglomeration is then cached, as upstream's mesh registry does).
    ///
    /// # Panics
    /// As [`Self::solve`].
    pub fn solve_with_face_areas<F: FnOnce() -> Vec<[f64; 3]>>(&mut self, internal_face_areas: F, ldu: &LduMatrix, source: &[f64], x: &mut [f64], ctl: &GamgControls) -> GamgPerformance {
        if self.agg.is_none() {
            let agg = match self.build(internal_face_areas, ldu, ctl) {
                Ok(a) => a,
                // the readers refuse every unsupported control by name before a run
                Err(e) => panic!("{e}"),
            };
            self.agg = Some(agg);
        }
        let agg = self.agg.as_ref().expect("built above");
        let perf = gamg_dic_solve(ldu, agg, ctl, source, x).unwrap_or_else(|e| panic!("{e}"));
        self.log.push((perf.initial_residual, perf.cycles.clone()));
        self.after_solve(ctl);
        perf
    }
}

/// The `GAMG` controls of a `solvers` entry (`p_rgh`, `p_rghFinal`), refusing by name what the port does not run. `tolerance` falls back to
/// `fallback` when absent, as the readers of the `PCG` entries do.
///
/// # Errors
/// The first unsupported selection, named.
#[cfg(feature = "std")]
pub(crate) fn read_gamg_controls(name: &str, d: &outram_foam_basic_lib::io::FoamDict, fallback: Option<&GamgControls>) -> Result<GamgControls, String> {
    use super::case::{bool_entry, scalar, word};
    let mut c = GamgControls::default();
    if let Some(f) = fallback {
        c.tolerance = f.tolerance;
    }
    if let Some(v) = scalar(d, "tolerance") {
        c.tolerance = v;
    }
    let count = |key: &str, default: usize| scalar(d, key).map_or(default, |v| v as usize);
    c.rel_tol = scalar(d, "relTol").unwrap_or(0.0);
    c.max_iter = count("maxIter", c.max_iter);
    c.min_iter = count("minIter", c.min_iter);
    c.n_pre_sweeps = count("nPreSweeps", c.n_pre_sweeps);
    c.n_post_sweeps = count("nPostSweeps", c.n_post_sweeps);
    c.post_sweeps_level_multiplier = count("postSweepsLevelMultiplier", c.post_sweeps_level_multiplier);
    c.max_post_sweeps = count("maxPostSweeps", c.max_post_sweeps);
    c.n_finest_sweeps = count("nFinestSweeps", c.n_finest_sweeps);
    c.n_cells_in_coarsest_level = count("nCellsInCoarsestLevel", c.n_cells_in_coarsest_level);
    c.merge_levels = count("mergeLevels", c.merge_levels);
    c.scale_correction = bool_entry(d, "scaleCorrection");
    c.pre_sweeps_level_multiplier = count("preSweepsLevelMultiplier", c.pre_sweeps_level_multiplier);
    c.max_pre_sweeps = count("maxPreSweeps", c.max_pre_sweeps);
    if let Some(s) = word(d, "smoother") {
        c.smoother = match GamgSmoother::from_word(&s) {
            Some(k) => k,
            None => {
                return Err(format!(
                    "solvers.{name}: GAMG smoother `{s}` is not ported (DIC, FDIC, GaussSeidel, DICGaussSeidel, symGaussSeidel, DILU, DILUGaussSeidel, nonBlockingGaussSeidel only)"
                ))
            }
        };
    } else {
        return Err(format!("solvers.{name}: GAMG without a `smoother` entry is not ported"));
    }
    // gh:#1946 (README round 85): `algebraicPair`, `interpolateCorrection`, `directSolveCoarsest`, `cacheAgglomeration`, `updateInterval`
    if let Some(a) = word(d, "agglomerator") {
        c.agglomerator = match GamgAgglomerator::from_word(&a) {
            Some(k) => k,
            None => return Err(format!("solvers.{name}: GAMG agglomerator `{a}` is not ported (faceAreaPair, algebraicPair only)")),
        };
    }
    c.interpolate_correction = bool_entry(d, "interpolateCorrection").unwrap_or(c.interpolate_correction);
    c.direct_solve_coarsest = bool_entry(d, "directSolveCoarsest").unwrap_or(c.direct_solve_coarsest);
    c.cache_agglomeration = bool_entry(d, "cacheAgglomeration").unwrap_or(c.cache_agglomeration);
    if let Some(v) = scalar(d, "updateInterval") {
        c.update_interval = v as i64;
    }
    match c.check_supported() {
        Ok(()) => Ok(c),
        Err(e) => Err(format!("solvers.{name}: {e}")),
    }
}

/// For a region whose solver does not hand its time index to [`GamgSolveState::set_time_index`] (gh:#1946, README round 85): refuse,
/// by name, an `updateInterval` other than 1, whose `faceAreaPair` weights depend on the time index of the build.
///
/// # Errors
/// `updateInterval != 1`, named.
pub(crate) fn refuse_update_interval_without_time_index(name: &str, c: &GamgControls) -> Result<(), String> {
    if c.update_interval != 1 {
        return Err(format!("solvers.{name}: GAMG updateInterval {} is not ported in this region (its solver does not hand the time index to the agglomeration; default 1)", c.update_interval));
    }
    Ok(())
}

/// For the neutronics flux `GAMG` (`common::linear_solvers`, which builds its own `faceAreaPair` agglomeration outside
/// [`GamgSolveState`]): refuse, by name, the agglomeration controls that path does not honour (gh:#1946, README round 85).
///
/// # Errors
/// `updateInterval != 1`, `agglomerator algebraicPair`, `cacheAgglomeration false`, named.
pub(crate) fn refuse_agglomeration_controls_outside_the_state(name: &str, c: &GamgControls) -> Result<(), String> {
    if let Err(e) = refuse_update_interval_without_time_index(name, c) {
        return Err(e);
    }
    if c.agglomerator != GamgAgglomerator::FaceAreaPair {
        return Err(format!("solvers.{name}: GAMG agglomerator algebraicPair is not ported for this solve (faceAreaPair only here)"));
    }
    if !c.cache_agglomeration {
        return Err(format!("solvers.{name}: GAMG cacheAgglomeration false is not ported for this solve"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_builds_the_agglomeration_once_and_reuses_it() {
        let mut s = GamgSolveState::default();
        assert!(s.take_log().is_empty());
        assert_eq!(s.n_levels(), 0);
        assert!(s.forward);
    }

    /// The reader takes FFTF's entry, applies the v2506 defaults, and refuses by name what is not ported.
    #[cfg(feature = "std")]
    #[test]
    fn the_reader_applies_defaults_and_refuses_unported_selections_by_name() {
        use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile};
        let parse = |t: &str| -> FoamDict {
            let d = FoamFile::parse(&alloc::format!("e {{ {t} }}")).expect("parse").dict;
            match d.get("e") {
                Some(FoamEntry::SubDict(x)) => x.clone(),
                _ => panic!("no sub dict"),
            }
        };
        let c = read_gamg_controls("p_rgh", &parse("solver GAMG; smoother DIC; tolerance 1e-07; relTol 0.001;"), None).expect("FFTF entry");
        assert_eq!(c, GamgControls { tolerance: 1e-7, rel_tol: 0.001, ..GamgControls::default() });
        let c = read_gamg_controls("p_rgh", &parse("solver GAMG; smoother DIC; tolerance 1e-06; relTol 0; maxIter 25;"), None).expect("cavity entry");
        assert_eq!(c.max_iter, 25);
        let c = read_gamg_controls("TStruct", &parse("solver GAMG; smoother GaussSeidel; nCellsInCoarsestLevel 50; nPreSweeps 1;"), None).expect("TStruct entry");
        assert!(c.smoother == GamgSmoother::GaussSeidel && c.n_pre_sweeps == 1 && c.n_cells_in_coarsest_level == 50);
        // gh:#1946 (README round 85): the selections refused before this round are read
        let text = "solver GAMG; smoother DILU; agglomerator algebraicPair; interpolateCorrection true; directSolveCoarsest true; mergeLevels 2; cacheAgglomeration false; updateInterval 10;";
        let c = match read_gamg_controls("p_rgh", &parse(text), None) {
            Ok(c) => c,
            Err(e) => panic!("round 85 entry: {e}"),
        };
        assert_eq!(
            c,
            GamgControls {
                smoother: GamgSmoother::Dilu,
                agglomerator: GamgAgglomerator::AlgebraicPair,
                interpolate_correction: true,
                direct_solve_coarsest: true,
                merge_levels: 2,
                cache_agglomeration: false,
                update_interval: 10,
                ..GamgControls::default()
            }
        );
        match refuse_update_interval_without_time_index("p_rgh", &c) {
            Ok(()) => panic!("updateInterval 10 must be refused where no time index is handed in"),
            Err(e) => assert!(e.contains("updateInterval"), "{e}"),
        }
        assert!(refuse_update_interval_without_time_index("p_rgh", &GamgControls::default()).is_ok());
        for (text, needle) in [
            ("solver GAMG; smoother ILU; tolerance 1e-6;", "smoother"),
            ("solver GAMG; tolerance 1e-6;", "smoother"),
            ("solver GAMG; smoother DIC; agglomerator MGridGen;", "agglomerator"),
            ("solver GAMG; smoother DIC; mergeLevels 0;", "mergeLevels"),
        ] {
            match read_gamg_controls("p_rgh", &parse(text), None) {
                Ok(c) => panic!("{text}: accepted as {c:?}"),
                Err(e) => assert!(e.contains(needle), "{text}: {e}"),
            }
        }
    }

    /// gh:#1946: `cacheAgglomeration false` drops the agglomeration after each solve (the next one builds it again, `forward_` toggled
    /// on); `true` keeps it.
    #[test]
    fn cache_agglomeration_false_rebuilds_for_the_next_solve() {
        let n = 64;
        let mut a = LduMatrix::new(n, (0..n - 1).collect(), (1..n).collect());
        a.diag = vec![2.5; n];
        a.upper = vec![-1.0; n - 1];
        a.lower = vec![-1.0; n - 1];
        let sf = || -> Vec<[f64; 3]> { (0..n - 1).map(|i| [1.0, 0.1 * i as f64, 0.0]).collect() };
        let b = vec![1.0; n];
        for cache in [true, false] {
            let ctl = GamgControls { cache_agglomeration: cache, tolerance: 1e-10, ..GamgControls::default() };
            let mut s = GamgSolveState::default();
            let mut x = vec![0.0; n];
            let _ = s.solve_with_face_areas(sf, &a, &b, &mut x, &ctl);
            assert_eq!(s.is_built(), cache);
            let mut y = vec![0.0; n];
            let _ = s.solve_with_face_areas(sf, &a, &b, &mut y, &ctl);
            assert_eq!(s.is_built(), cache);
            assert_eq!(s.take_log().len(), 2);
        }
    }
}
