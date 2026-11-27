// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/customPimpleControl/customPimpleControl.{H,C},
//     customPimpleControlI.H (`loop`, `criteriaSatisfied`, `correct(bool)`, `updateFirstPISOPrevPIMPLE`),
//     src/classes/thermalHydraulics/src/include/myOps.C (`relax`, `relaxationFactor`)
//   and OpenFOAM-ESI v2506 `pimpleControl.{C,H,I.H}` (`finalIter`, `finalInnerIter`, `storeInitialResiduals`),
//     `solutionControl.C` (`read`, `applyToField`, `maxResidual`), `meshState` (`setSolverPerformance`),
//     `solution.C` (`relaxField`, `relaxEquation`, the `default` entry), `fvMatrix::relax()`, `GeometricField::relax()`.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506 (C) OpenCFD Ltd
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `one_phase_cc::pimple` — `customPimpleControl` and `relaxationFactors` of the cell-centred `onePhase` region (gh:#1855, round 31)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! What GeN-Foam's `onePhase::correctPhysics` does with `while (pimple_.loop())`, call by call, including what makes it differ from
//! stock OpenFOAM:
//!
//! * `customPimpleControl` declares its own `corr_`; the inline base functions `finalIter()` (`converged_ || corr_ == nCorrPIMPLE_`)
//!   and `storeInitialResiduals()` read the BASE `corr_`, which stays 0. So `finalIter()` is `converged_` only and
//!   `storeInitialResiduals()` is always false: `residualControl_[f].initialResidual` stays 0 and `relTol` is inert (the relative
//!   quantity is `last/(0 + SMALL)`).
//! * `isFinalIteration()` (the `Final` solver and relaxation entries) is set only in the iteration whose start found the criteria
//!   satisfied. That iteration runs, the next `loop()` ends the step; flagged at `corr == nOuterCorrectors` the next `loop()` takes
//!   the `corr == nCorrPIMPLE + 1` branch first and leaves `converged_` set, so the NEXT step's first `loop()` ends at once (zero
//!   outer iterations): the stale flag of round 4.
//! * `p_rgh` selects its solver by `finalInnerIter()` (last corrector, and `finalIter()` only with `finalOnLastPimpleIterOnly`),
//!   so `p_rghFinal` runs at the last corrector of EVERY outer iteration by default.
//! * the performance dictionary holds, per field and time step, the (first, last) initial residual of its solves.
//!
//! The stock-OpenFOAM selection (`finalIter()` true at `corr == nOuterCorrectors`) is kept as a named CONTROL
//! ([`PimpleState::with_stock_final_selection`]).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::super::case::key_matches;

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1e-15;

/// One `relaxationFactors` sub-dictionary (`fields` or `equations`): keys (literal or `.*` patterns) with scalar factors, in file order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelaxationDict {
    entries: Vec<(String, f64)>,
}

impl RelaxationDict {
    /// From `(key, factor)` pairs in file order.
    #[must_use]
    pub fn new(entries: Vec<(String, f64)>) -> Self {
        Self { entries }
    }

    /// `dictionary` lookup of `name`: the exact keyword, else the LAST pattern that matches.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<f64> {
        if let Some((_, v)) = self.entries.iter().find(|(k, _)| k == name) {
            return Some(*v);
        }
        self.entries.iter().rev().find(|(k, _)| key_matches(k, name)).map(|(_, v)| *v)
    }

    /// `solution::relaxField(name, factor)` / `relaxEquation(name, factor)`: the entry that `name` finds, else the `default` entry
    /// (itself looked up as a keyword, so a pattern such as `".*"` can be it), else `None` (no relaxation).
    #[must_use]
    pub fn factor(&self, name: &str, use_default: bool) -> Option<f64> {
        self.lookup(name).or_else(|| if use_default { self.lookup("default") } else { None })
    }
}

/// `fvSolution.relaxationFactors`: the `fields` and `equations` dictionaries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelaxationFactors {
    /// `fields` (`GeometricField::relax()`).
    pub fields: RelaxationDict,
    /// `equations` (`fvMatrix::relax()`).
    pub equations: RelaxationDict,
    /// ABLATION (default false): ignore the `default` entries, as the port did before gh:#1855.
    pub ignore_default: bool,
}

impl RelaxationFactors {
    /// `fvMatrix::relax()` for the equation of `field`: the name is `field` or, in the final iteration, `<field>Final`.
    #[must_use]
    pub fn equation(&self, field: &str, final_iteration: bool) -> Option<f64> {
        self.equations.factor(&select(field, final_iteration), !self.ignore_default)
    }

    /// `GeometricField::relax()` (and `myOps::relax`) for `field`.
    #[must_use]
    pub fn field(&self, field: &str, final_iteration: bool) -> Option<f64> {
        self.fields.factor(&select(field, final_iteration), !self.ignore_default)
    }

    /// `solution::relaxField(name)` for the PLAIN name (what `solutionControl::storePrevIter` asks at each `loop()`).
    #[must_use]
    pub fn relaxes_field(&self, field: &str) -> bool {
        self.fields.factor(field, !self.ignore_default).is_some()
    }
}

/// `psi.select(final)`.
fn select(name: &str, final_iteration: bool) -> String {
    if final_iteration {
        let mut s = name.to_string();
        s.push_str("Final");
        s
    } else {
        name.to_string()
    }
}

/// One `PIMPLE.residualControl` entry (`solutionControl::fieldData` plus the customPimpleControl flags).
#[derive(Debug, Clone, PartialEq)]
pub struct ResidualSpec {
    /// The field name (a literal or a `.*` pattern).
    pub name: String,
    /// `tolerance`.
    pub abs_tol: f64,
    /// `relTol` (inert: see the module documentation).
    pub rel_tol: f64,
    /// `useFirstPISOInitialResidual` (default false).
    pub use_first_piso: bool,
}

/// The `PIMPLE` entries the loop reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PimpleControls {
    /// `nOuterCorrectors` (default 1).
    pub n_outer: usize,
    /// `minNOuterCorrectors` (default 1).
    pub min_n_outer: usize,
    /// `residualControl`, in file order.
    pub residual_control: Vec<ResidualSpec>,
    /// `finalOnLastPimpleIterOnly` (default false).
    pub final_on_last_pimple_iter_only: bool,
    /// `nCorrectors` (default 1): the corrector `finalInnerIter()` fires at.
    pub n_correctors: usize,
}

/// What the loop reports to the solver about one `loop()` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStep {
    /// Run outer iteration `corr`; the previous-iteration fields were stored.
    Run,
    /// The step's outer loop is over.
    End,
}

/// The state of `customPimpleControl` and of the region's `solverPerformanceDict` and `finalIteration` flag.
#[derive(Debug, Clone, PartialEq)]
pub struct PimpleState {
    /// `customPimpleControl::corr_` (the derived one).
    pub corr: usize,
    /// `pimpleControl::converged_`; stays set after a flag at `corr == nOuterCorrectors` (the stale flag).
    pub converged: bool,
    /// `stopLoop_`.
    stop_loop: bool,
    /// `mesh.data().isFinalIteration()`.
    pub final_iteration: bool,
    /// `previousPIMPLEFirstPISORes_`.
    previous_first_piso: [f64; 2],
    /// `solverPerformanceDict`: (field, first, last initial residual) in creation order, cleared at each time step.
    perf: Vec<(String, f64, f64)>,
    /// CONTROL: stock OpenFOAM's `finalIter()` (`corr == nOuterCorrectors` also counts).
    stock_final: bool,
    /// ABLATION: no `residualControl` (the port before gh:#1855).
    ignore_residual_control: bool,
    /// ABLATION: `p_rghFinal` only at the last corrector of the last outer iteration (the port before gh:#1855).
    legacy_p_final: bool,
}

impl Default for PimpleState {
    fn default() -> Self {
        Self::new()
    }
}

impl PimpleState {
    /// `customPimpleControl` at construction (`corr_ = 0`, `converged_ = false`, residual pair (0, 0)).
    #[must_use]
    pub fn new() -> Self {
        Self {
            corr: 0,
            converged: false,
            stop_loop: false,
            final_iteration: false,
            previous_first_piso: [0.0, 0.0],
            perf: Vec::new(),
            stock_final: false,
            ignore_residual_control: false,
            legacy_p_final: false,
        }
    }

    /// CONTROL, NOT upstream: stock OpenFOAM's `finalIter()` also true at `corr == nOuterCorrectors`, so the `Final` entries are
    /// selected in the last outer iteration. GeN-Foam's shadowed `corr_` never does that.
    #[must_use]
    pub fn with_stock_final_selection(mut self) -> Self {
        self.stock_final = true;
        self
    }

    /// ABLATION: `residualControl` ignored (`residualControl_.empty()`), as the port behaved before gh:#1855.
    #[must_use]
    pub fn without_residual_control(mut self) -> Self {
        self.ignore_residual_control = true;
        self
    }

    /// ABLATION: `p_rghFinal` only at the last corrector of the LAST outer iteration, as the port did before gh:#1855 (upstream: the last
    /// corrector of every outer iteration, `finalInnerIter()`).
    #[must_use]
    pub fn with_legacy_p_final_on_last_outer_only(mut self) -> Self {
        self.legacy_p_final = true;
        self
    }

    /// A new time step: the first `setSolverPerformance` of a new time index clears the dictionary.
    pub fn begin_time_step(&mut self) {
        self.perf.clear();
    }

    /// `meshState::setSolverPerformance(name, sp)` for a scalar field: (first, last) initial residual.
    pub fn record(&mut self, name: &str, initial_residual: f64) {
        if let Some(e) = self.perf.iter_mut().find(|(n, _, _)| n == name) {
            e.2 = initial_residual;
        } else {
            self.perf.push((name.to_string(), initial_residual, initial_residual));
        }
    }

    /// The `last` initial residual recorded for `name` in this step.
    #[must_use]
    pub fn last_residual(&self, name: &str) -> Option<f64> {
        self.perf.iter().find(|(n, _, _)| n == name).map(|e| e.2)
    }

    /// `customPimpleControl::updateFirstPISOPrevPIMPLE()` after a `p_rgh` solve (`corrPISO_` is the 1-based corrector).
    pub fn update_first_piso_prev_pimple(&mut self, corr_piso: usize) {
        if corr_piso == 1 {
            self.previous_first_piso[0] = self.previous_first_piso[1];
            self.previous_first_piso[1] = self.last_residual("p_rgh").map_or(0.0, f64::abs);
        }
    }

    /// `pimpleControl::finalIter()`: `converged_`; with the stock control also the last outer iteration.
    #[must_use]
    pub fn final_iter(&self, n_outer: usize) -> bool {
        self.converged || (self.stock_final && self.corr == n_outer)
    }

    /// `pimpleControl::finalInnerIter()` for `nNonOrthogonalCorrectors 0`: the last corrector, and `finalIter()` with
    /// `finalOnLastPimpleIterOnly`.
    #[must_use]
    pub fn final_inner_iter(&self, corr_piso: usize, ctl: &PimpleControls) -> bool {
        let inner = corr_piso == ctl.n_correctors;
        if self.legacy_p_final {
            inner && self.corr == ctl.n_outer
        } else if ctl.final_on_last_pimple_iter_only {
            inner && self.final_iter(ctl.n_outer)
        } else {
            inner
        }
    }

    /// `solutionControl::applyToField`: the first `residualControl` entry that matches the field.
    fn apply_to_field<'a>(ctl: &'a PimpleControls, name: &str) -> Option<&'a ResidualSpec> {
        ctl.residual_control.iter().find(|s| s.name == name || key_matches(&s.name, name))
    }

    /// `customPimpleControl::criteriaSatisfied()`.
    fn criteria_satisfied(&self, ctl: &PimpleControls) -> bool {
        if self.corr == 1
            || ctl.residual_control.is_empty()
            || self.ignore_residual_control
            || self.final_iter(ctl.n_outer)
            || self.corr < ctl.min_n_outer
        {
            return false;
        }
        let mut achieved = true;
        let mut checked = false;
        for (name, _first, last) in &self.perf {
            let Some(spec) = Self::apply_to_field(ctl, name) else { continue };
            let r_last = if spec.use_first_piso { self.previous_first_piso[1] } else { *last };
            checked = true;
            let abs_check = r_last < spec.abs_tol;
            // `storeInitialResiduals()` is false (base `corr_`), so the stored initial residual stays 0
            let relative = r_last / (0.0 + SMALL);
            let rel_check = relative < spec.rel_tol;
            achieved = achieved && (abs_check || rel_check || self.stop_loop);
        }
        checked && achieved
    }

    /// `customPimpleControl::loop()`: `true` to run another outer iteration (`corr` then names it).
    pub fn loop_step(&mut self, ctl: &PimpleControls) -> LoopStep {
        self.corr += 1;
        if self.corr == ctl.n_outer + 1 {
            // `converged_` is NOT cleared here (upstream's defect)
            self.corr = 0;
            self.stop_loop = false;
            self.final_iteration = false;
            return LoopStep::End;
        }
        let mut completed = false;
        if self.converged || self.criteria_satisfied(ctl) {
            if self.converged {
                self.final_iteration = false;
                self.corr = 0;
                self.stop_loop = false;
                self.converged = false;
                completed = true;
            } else {
                self.final_iteration = true;
                self.converged = true;
            }
        } else {
            if self.final_iter(ctl.n_outer) {
                self.final_iteration = true;
            }
        }
        if completed {
            LoopStep::End
        } else {
            LoopStep::Run
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctl(n_outer: usize, rc: Vec<ResidualSpec>) -> PimpleControls {
        PimpleControls { n_outer, min_n_outer: 1, residual_control: rc, final_on_last_pimple_iter_only: false, n_correctors: 6 }
    }

    fn spec(name: &str, abs: f64, rel: f64, first: bool) -> ResidualSpec {
        ResidualSpec { name: name.to_string(), abs_tol: abs, rel_tol: rel, use_first_piso: first }
    }

    /// Run one step with `res[c-1]` the residual of field `h` of outer iteration `c`; returns (iterations run, finals).
    fn step(st: &mut PimpleState, c: &PimpleControls, res: &[f64]) -> (usize, Vec<bool>) {
        st.begin_time_step();
        let mut ran = 0;
        let mut finals = Vec::new();
        while st.loop_step(c) == LoopStep::Run {
            ran += 1;
            finals.push(st.final_iteration);
            st.record("h", res[st.corr - 1]);
        }
        (ran, finals)
    }

    #[test]
    fn without_residual_control_runs_every_outer_iteration_and_never_selects_final() {
        let mut st = PimpleState::new();
        let c = ctl(5, Vec::new());
        let (ran, finals) = step(&mut st, &c, &[1.0; 5]);
        assert_eq!(ran, 5);
        assert!(finals.iter().all(|f| !f));
        assert!(!st.converged);
    }

    #[test]
    fn flag_at_iteration_four_runs_four_and_finals_only_the_last() {
        let mut st = PimpleState::new();
        let c = ctl(5, vec![spec("h", 0.25, 0.0, false)]);
        let (ran, finals) = step(&mut st, &c, &[1.0, 1.0, 0.218, 0.09, 0.04]);
        assert_eq!(ran, 4);
        assert_eq!(finals, vec![false, false, false, true]);
        assert!(!st.converged && !st.final_iteration);
    }

    #[test]
    fn flag_at_the_last_iteration_leaves_the_stale_flag_and_the_next_step_runs_nothing() {
        let mut st = PimpleState::new();
        let c = ctl(5, vec![spec("h", 0.1, 0.0, false)]);
        let (ran, finals) = step(&mut st, &c, &[1.0, 1.0, 0.218, 0.0895, 0.04]);
        assert_eq!(ran, 5);
        assert!(finals[4] && !finals[3]);
        assert!(st.converged);
        let (ran2, _) = step(&mut st, &c, &[1.0; 5]);
        assert_eq!(ran2, 0);
        assert!(!st.converged);
        let (ran3, _) = step(&mut st, &c, &[1.0; 5]);
        assert_eq!(ran3, 5);
    }

    #[test]
    fn rel_tol_is_inert_and_the_control_ablation_matches_no_control() {
        let mut st = PimpleState::new();
        let c = ctl(5, vec![spec("h", 1e-30, 0.99, false)]);
        assert_eq!(step(&mut st, &c, &[0.5; 5]).0, 5);
        let mut off = PimpleState::new().without_residual_control();
        let c2 = ctl(5, vec![spec("h", 10.0, 0.0, false)]);
        assert_eq!(step(&mut off, &c2, &[0.5; 5]).0, 5);
        let mut on = PimpleState::new();
        assert_eq!(step(&mut on, &c2, &[0.5; 5]).0, 2);
    }

    #[test]
    fn stock_control_selects_final_in_the_last_iteration() {
        let mut st = PimpleState::new().with_stock_final_selection();
        let c = ctl(3, Vec::new());
        let (ran, finals) = step(&mut st, &c, &[1.0; 3]);
        assert_eq!(ran, 3);
        assert_eq!(finals, vec![false, false, true]);
    }

    #[test]
    fn first_piso_residual_pair_and_final_inner_iter() {
        let mut st = PimpleState::new();
        let c = ctl(5, vec![spec("p_rgh", 0.004, 0.0, true)]);
        st.begin_time_step();
        assert_eq!(st.loop_step(&c), LoopStep::Run);
        st.record("p_rgh", 0.9);
        st.update_first_piso_prev_pimple(1);
        st.record("p_rgh", 0.1);
        st.update_first_piso_prev_pimple(2);
        assert_eq!(st.previous_first_piso, [0.0, 0.9]);
        assert!(st.final_inner_iter(6, &c));
        assert!(!st.final_inner_iter(5, &c));
        let mut c2 = c.clone();
        c2.final_on_last_pimple_iter_only = true;
        assert!(!st.final_inner_iter(6, &c2));
        assert_eq!(st.last_residual("p_rgh"), Some(0.1));
    }

    #[test]
    fn relaxation_lookup_exact_then_pattern_then_default() {
        let d = RelaxationDict::new(vec![
            ("default".to_string(), 0.8),
            ("h".to_string(), 0.6),
            ("U.*".to_string(), 0.7),
        ]);
        assert_eq!(d.lookup("h"), Some(0.6));
        assert_eq!(d.lookup("UFinal"), Some(0.7));
        assert_eq!(d.lookup("k"), None);
        assert_eq!(d.factor("k", true), Some(0.8));
        assert_eq!(d.factor("k", false), None);
        let r = RelaxationFactors { fields: RelaxationDict::default(), equations: d, ignore_default: false };
        assert_eq!(r.equation("h", false), Some(0.6));
        // the final iteration asks for `hFinal`: not a key, so `default`, NOT h's 0.6
        assert_eq!(r.equation("h", true), Some(0.8));
        assert_eq!(r.equation("U", true), Some(0.7));
        assert!(!r.relaxes_field("h"));
        let ab = RelaxationFactors { ignore_default: true, ..r.clone() };
        assert_eq!(ab.equation("k", false), None);
        assert_eq!(ab.equation("h", true), None);
    }
}
