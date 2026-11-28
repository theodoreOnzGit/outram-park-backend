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
use crate::genfoam::common::gamg::{GamgSmoother, face_area_pair_weights, gamg_dic_solve, gamg_dic_solve_if, gamg_dic_solve_without_coarse_interfaces, pair_agglomerate, GamgAgglomeration, GamgControls, GamgPerformance};
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
}

impl Default for GamgSolveState {
    fn default() -> Self {
        Self { agg: None, forward: true, log: Vec::new(), coarse_interfaces: true }
    }
}

impl GamgSolveState {
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
            let sf: Vec<[f64; 3]> = g.mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect();
            let w = face_area_pair_weights(&sf);
            let agg = pair_agglomerate(ldu.n_cells, &ldu.owner, &ldu.neighbour, &w, ctl, &mut self.forward).unwrap_or_else(|e| panic!("{e}"));
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
            let w = face_area_pair_weights(&internal_face_areas());
            let agg = pair_agglomerate(ldu.n_cells, &ldu.owner, &ldu.neighbour, &w, ctl, &mut self.forward).unwrap_or_else(|e| panic!("{e}"));
            self.agg = Some(agg);
        }
        let agg = self.agg.as_ref().expect("built above");
        let perf = gamg_dic_solve(ldu, agg, ctl, source, x).unwrap_or_else(|e| panic!("{e}"));
        self.log.push((perf.initial_residual, perf.cycles.clone()));
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
        c.smoother = GamgSmoother::from_word(&s)
            .ok_or_else(|| format!("solvers.{name}: GAMG smoother `{s}` is not ported (DIC, GaussSeidel, DICGaussSeidel, symGaussSeidel only)"))?;
    } else {
        return Err(format!("solvers.{name}: GAMG without a `smoother` entry is not ported"));
    }
    if let Some(a) = word(d, "agglomerator") {
        if a != "faceAreaPair" {
            return Err(format!("solvers.{name}: GAMG agglomerator `{a}` is not ported (faceAreaPair only)"));
        }
    }
    for (key, refused_value) in [("interpolateCorrection", true), ("directSolveCoarsest", true), ("cacheAgglomeration", false)] {
        if bool_entry(d, key) == Some(refused_value) {
            return Err(format!("solvers.{name}: GAMG {key} {refused_value} is not ported"));
        }
    }
    if scalar(d, "updateInterval").is_some() {
        return Err(format!("solvers.{name}: GAMG updateInterval is not ported (default 1)"));
    }
    c.check_supported().map_err(|e| format!("solvers.{name}: {e}"))?;
    Ok(c)
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
        for (text, needle) in [
            ("solver GAMG; smoother DILU; tolerance 1e-6;", "smoother"),
            ("solver GAMG; tolerance 1e-6;", "smoother"),
            ("solver GAMG; smoother DIC; agglomerator algebraicPair;", "agglomerator"),
            ("solver GAMG; smoother DIC; interpolateCorrection true;", "interpolateCorrection"),
            ("solver GAMG; smoother DIC; mergeLevels 2;", "mergeLevels"),
            ("solver GAMG; smoother DIC; cacheAgglomeration false;", "cacheAgglomeration"),
        ] {
            let e = read_gamg_controls("p_rgh", &parse(text), None).unwrap_err();
            assert!(e.contains(needle), "{text}: {e}");
        }
    }
}
