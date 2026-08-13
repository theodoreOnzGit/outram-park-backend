//! The simultaneous adjust solver — Newton's method over several adjust blocks.
//!
//! # What an adjust block is
//!
//! An **adjust** (DWSIM's `IAdjust`, the flowsheet's `OT_Adjust` object) is a
//! controller written into the flowsheet: "move this **manipulated** variable
//! until that **controlled** variable equals a target". A reboiler duty adjusted
//! until a distillate purity hits spec, say.
//!
//! A single adjust can be solved by the block itself, iteration by iteration,
//! inside the ordinary calculation pass. But several adjusts on the *same*
//! flowsheet interact — moving one changes the others' controlled variables — so
//! DWSIM offers a **simultaneous** mode: mark the blocks
//! [`AdjustBlock::simultaneous_adjust`] and one Newton solve drives them all to
//! their targets at once. That solver is [`solve_simultaneous_adjusts`], and it
//! is what this module ports.
//!
//! # The method
//!
//! With `n` marked adjusts, let `x` be the vector of manipulated variables and
//!
//! `f_i(x) = target_i - controlled_i(x)`
//!
//! (plus a referenced variable when [`AdjustBlock::referenced`] is set, see
//! [`AdjustBlock::residual`]). Then, per iteration:
//!
//! 1. evaluate `f(x)` — which requires a **full flowsheet solve**;
//! 2. stop if `|f_i| < tol_i` for every `i` (FlowsheetSolver.vb:1994-2001);
//! 3. build the Jacobian `J` by central differences with a **1 % relative
//!    perturbation**, costing `2n` further full flowsheet solves
//!    (FlowsheetSolver.vb:2211-2243);
//! 4. solve `J dx = f` (FlowsheetSolver.vb:2010);
//! 5. take a **damped** step: `dfac = min(0.2 (ic+1), 1)`, halved by a further
//!    factor of ten if any component would overshoot past zero, then
//!    `x <- x - dfac * dx` (FlowsheetSolver.vb:2013-2026);
//! 6. give up at **25 iterations** (FlowsheetSolver.vb:2035), or stop early once
//!    `sum |dx| < 1e-6` (:2037).
//!
//! # Cost warning
//!
//! Each Newton iteration costs `1 + 2n` **complete flowsheet solves**, and each
//! of those may itself run a full recycle-convergence loop. With the hard cap of
//! 25 iterations and `n` adjusts, the worst case is `25 (1 + 2n)` flowsheet
//! solves — 125 for a single adjust, 525 for five. This is by far the most
//! expensive thing in the solver, and it is the first thing to disable for any
//! real-time use. See [`AdjustSolveReport::flowsheet_solves`], which counts them.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources:
//!
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:1943-2049`
//!   (`SolveSimultaneousAdjusts`) — the synchronous master routine this module
//!   follows.
//! - `:2051-2154` (`SolveSimultaneousAdjustsAsync`) — the asynchronous twin.
//!   **Identical arithmetic**; it differs only in (a) taking a
//!   `CancellationToken`, (b) moving `ic += 1` after `CheckStatus`/
//!   `UpdateInterface` rather than before (:2142 vs :2030), which cannot change
//!   the result because nothing between them reads `ic`, and (c) letting the
//!   exception escape instead of catching it and logging (:2043-2045). Ported
//!   once.
//! - `:2156-2202` (`FunctionValueSync`) and `:2204-2243`
//!   (`FunctionGradientSync`).
//! - `:2336-2404` — the four variable accessors, ported in
//!   [`crate::flowsheet_solver::variables`].
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver2.vb:1085-1351` — the newer class's
//!   copy of the same routine; see the module note in
//!   [`crate::flowsheet_solver`].
//!
//! # Excluded DWSIM behavior
//!
//! - **`FlowsheetOptions.SimultaneousAdjustSolverEnabled`**
//!   (FlowsheetSolver.vb:1955). A flowsheet-level option this port replaces with
//!   "the caller passes adjust blocks, or does not".
//! - **Display-unit conversion of the target and the controlled value**
//!   (`cv.ConvertFromSI(punit, adj.AdjustValue)`, :2184-2194) including its
//!   temperature special case. This port is SI throughout — see
//!   [`AdjustBlock::tolerance`].
//! - **`fgui.ShowMessage` iteration logging** (:2003) and
//!   `fgui.UpdateInterface` (:2033). Replaced by [`AdjustSolveReport`].
//! - **`Try/Catch` swallowing every failure into a log line** (:2043-2045). This
//!   port returns the error, following the async variant.
//! - **`il_err_ant`** (:1971, :1992). Assigned and never read upstream.
//!
//! # Honest scope
//!
//! AI-assisted draft with **no human V&V**. The tests below verify the ported
//! Newton arithmetic against analytically-solvable synthetic cases; they are not
//! validation against a DWSIM benchmark flowsheet.

use std::collections::HashMap;

use crate::flowsheet::{Flowsheet, ObjectId, ObjectType};
use crate::flowsheet_solver::errors::{AbortFlag, SolverError};
use crate::flowsheet_solver::linalg::{abs_sqr_sum, abs_sum, solve_dense};
use crate::flowsheet_solver::variables::VariableRef;

/// The hard iteration cap upstream writes as a literal
/// (`If ic >= 25 Then Throw`, FlowsheetSolver.vb:2035, :2144).
pub const MAX_ADJUST_ITERATIONS: usize = 25;

/// The relative perturbation used to build the Jacobian by central differences
/// (`Dim epsilon As Double = 0.01`, FlowsheetSolver.vb:2213, :2304).
///
/// Dimensionless: each variable is perturbed to `x (1 +/- epsilon)`, or to
/// `x + epsilon` when `x` is exactly zero.
pub const GRADIENT_EPSILON: f64 = 0.01;

/// The step-size floor below which the solver stops
/// (`If Math.Abs(AbsSum(dx)) < 0.000001 Then Exit Do`,
/// FlowsheetSolver.vb:2037, :2146).
pub const MIN_STEP_SUM: f64 = 1e-6;

/// One adjust block's specification and its convergence tolerance.
///
/// # Where this state lives
///
/// Like [`crate::flowsheet_solver::recycle::RecycleBlock`], an `AdjustBlock` is
/// owned by [`crate::flowsheet_solver::solver::FlowsheetSolver`] and keyed by
/// the [`ObjectId`] of the corresponding [`ObjectType::OtAdjust`] object, because
/// the flowsheet data model carries no equipment state.
#[derive(Debug, Clone, PartialEq)]
pub struct AdjustBlock {
    /// The variable the solver is allowed to move (`ManipulatedObjectData`,
    /// FlowsheetSolver.vb:2364). Read once to seed `x`, written every function
    /// evaluation.
    pub manipulated: VariableRef,
    /// The variable being driven to the target (`ControlledObjectData`,
    /// FlowsheetSolver.vb:2347).
    pub controlled: VariableRef,
    /// An optional variable the target is measured *relative to*
    /// (`ReferencedObjectData`, FlowsheetSolver.vb:2400). Active only when
    /// [`AdjustBlock::referenced`] is `true`.
    pub referenced_variable: Option<VariableRef>,
    /// Whether the target is relative (`IAdjust.Referenced`,
    /// FlowsheetSolver.vb:2185). See [`AdjustBlock::residual`].
    pub referenced: bool,
    /// The target value for the controlled variable, in that variable's **SI**
    /// unit (`IAdjust.AdjustValue`).
    pub adjust_value: f64,
    /// Convergence tolerance on the residual, in the controlled variable's
    /// **SI** unit (`IAdjust.Tolerance`, FlowsheetSolver.vb:1980).
    ///
    /// **Divergence from upstream:** DWSIM compares the residual in the user's
    /// *display* units (:2184-2194), so an upstream tolerance of `0.01` on a
    /// temperature means 0.01 degC or 0.01 degF depending on the selected unit
    /// system. Here it always means the SI unit named by
    /// [`crate::flowsheet_solver::variables::FlowsheetVariable::si_unit`].
    pub tolerance: f64,
    /// Whether this block participates in the simultaneous solve
    /// (`IAdjust.SimultaneousAdjust`, FlowsheetSolver.vb:1962). Blocks with this
    /// `false` are ignored by [`solve_simultaneous_adjusts`] entirely.
    pub simultaneous_adjust: bool,
}

impl AdjustBlock {
    /// A block that drives `controlled` to `adjust_value` by moving
    /// `manipulated`, with the given tolerance, marked for the simultaneous
    /// solver.
    #[must_use]
    pub fn new(
        manipulated: VariableRef,
        controlled: VariableRef,
        adjust_value: f64,
        tolerance: f64,
    ) -> Self {
        AdjustBlock {
            manipulated,
            controlled,
            referenced_variable: None,
            referenced: false,
            adjust_value,
            tolerance,
            simultaneous_adjust: true,
        }
    }

    /// The residual `f_i` for this block, in the controlled variable's SI unit.
    ///
    /// - Absolute mode (`referenced = false`, FlowsheetSolver.vb:2193-2194):
    ///   `f = adjust_value - controlled`.
    /// - Relative mode (`referenced = true`, :2191):
    ///   `f = adjust_value + referenced - controlled`, i.e. the controlled
    ///   variable is driven to `referenced + adjust_value` and `adjust_value`
    ///   reads as an *offset*.
    ///
    /// A missing referenced variable in relative mode contributes `0`, matching
    /// upstream's `GetValueOrDefault`-style tolerance of unset data.
    ///
    /// # Errors
    ///
    /// Whatever [`VariableRef::get`] reports for the controlled or referenced
    /// variable.
    pub fn residual(&self, flowsheet: &Flowsheet) -> Result<f64, SolverError> {
        let controlled = self.controlled.get(flowsheet)?;
        if self.referenced {
            let reference = match &self.referenced_variable {
                Some(r) => r.get(flowsheet)?,
                None => 0.0,
            };
            Ok(self.adjust_value + reference - controlled)
        } else {
            Ok(self.adjust_value - controlled)
        }
    }
}

/// What a simultaneous adjust solve did.
///
/// Replaces upstream's `ShowMessage` log lines (FlowsheetSolver.vb:2003).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdjustSolveReport {
    /// How many adjust blocks took part. `0` means the solve was a no-op.
    pub variables: usize,
    /// Newton iterations performed (upstream's `ic`).
    pub iterations: usize,
    /// How many **full flowsheet solves** were spent. `1 + 2 * variables` per
    /// iteration; see the module's cost warning.
    pub flowsheet_solves: usize,
    /// Whether every residual came inside its tolerance.
    pub converged: bool,
    /// The final sum of squared residuals — upstream's "NSSE"
    /// (FlowsheetSolver.vb:1993).
    pub error_norm: f64,
}

/// Solve every marked adjust block simultaneously by Newton's method.
///
/// # Arguments
///
/// - `flowsheet` — mutated throughout: the manipulated variables are written and
///   the whole flowsheet is re-solved on every function evaluation.
/// - `adjusts` — the blocks, keyed by their [`ObjectType::OtAdjust`] object id.
///   Blocks whose object is absent from the flowsheet, inactive, or not marked
///   [`AdjustBlock::simultaneous_adjust`] are skipped, exactly as upstream's
///   `Where` clause does (FlowsheetSolver.vb:1961-1963).
/// - `resolve` — re-solve the whole flowsheet. Upstream calls
///   `SolveFlowsheet(fobj, mode, Nothing, False, True)` here — note the
///   `Adjusting = True`, which is what stops the recursion
///   (FlowsheetSolver.vb:2175). The caller must pass a closure with that same
///   property.
/// - `abort` — checked once per Newton iteration.
///
/// # Ordering
///
/// Deterministic: blocks are taken in the flowsheet's **registry insertion
/// order** over [`ObjectType::OtAdjust`] objects, which is what upstream's
/// `SimulationObjects.Values.Where(...)` gives on .NET's insertion-ordered
/// dictionary.
///
/// # Errors
///
/// - [`SolverError::AdjustMaxIterations`] after [`MAX_ADJUST_ITERATIONS`].
/// - [`SolverError::AdjustNonFinite`] if the error norm becomes NaN.
/// - [`SolverError::Aborted`] if the abort flag is raised.
/// - Anything `resolve` or the variable accessors report.
pub fn solve_simultaneous_adjusts<S>(
    flowsheet: &mut Flowsheet,
    adjusts: &HashMap<ObjectId, AdjustBlock>,
    resolve: &mut S,
    abort: &AbortFlag,
) -> Result<AdjustSolveReport, SolverError>
where
    S: FnMut(&mut Flowsheet) -> Result<(), SolverError>,
{
    let active = active_adjusts(flowsheet, adjusts);
    let n = active.len();
    let mut report = AdjustSolveReport {
        variables: n,
        ..AdjustSolveReport::default()
    };
    if n == 0 {
        report.converged = true;
        return Ok(report);
    }

    // Seed x and the tolerances (FlowsheetSolver.vb:1976-1983).
    let mut x: Vec<f64> = Vec::with_capacity(n);
    let mut tolerances: Vec<f64> = Vec::with_capacity(n);
    for id in &active {
        let block = &adjusts[id];
        x.push(block.manipulated.get(flowsheet)?);
        tolerances.push(block.tolerance);
    }

    // `Dim dx(n)` is zero-initialised upstream, and the `AbsSum(dx) < 1e-6`
    // early exit reads it even on an iteration where the linear solve failed.
    let mut dx = vec![0.0_f64; n];
    let mut ic = 0usize;

    loop {
        abort.check()?;

        let fx = function_value(flowsheet, adjusts, &active, &x, resolve)?;
        report.flowsheet_solves += 1;

        let error_norm = abs_sqr_sum(&fx);
        report.error_norm = error_norm;

        // Upstream's convergence loop breaks on the first residual outside
        // tolerance (:1994-2001).
        let converged = fx
            .iter()
            .zip(tolerances.iter())
            .all(|(f, tol)| f.abs() < *tol);
        if converged {
            report.converged = true;
            report.iterations = ic;
            return Ok(report);
        }

        let jacobian = function_gradient(flowsheet, adjusts, &active, &x, resolve, &mut report)?;

        if let Some(step) = solve_dense(jacobian, &fx) {
            // Damping schedule (FlowsheetSolver.vb:2013-2021).
            let mut dfac = ((ic + 1) as f64) * 0.2;
            if dfac > 1.0 {
                dfac = 1.0;
            }
            for (i, s) in step.iter().enumerate() {
                if (-s * dfac).abs() > x[i] {
                    dfac /= 10.0;
                    break;
                }
            }
            for i in 0..n {
                dx[i] = -step[i];
                x[i] += dfac * dx[i];
            }
        }

        ic += 1;
        report.iterations = ic;

        if ic >= MAX_ADJUST_ITERATIONS {
            return Err(SolverError::AdjustMaxIterations);
        }
        if error_norm.is_nan() {
            return Err(SolverError::AdjustNonFinite);
        }
        if abs_sum(&dx).abs() < MIN_STEP_SUM {
            // Upstream exits the loop *without* declaring convergence
            // (:2037) — the step became too small to make progress.
            return Ok(report);
        }
    }
}

/// The adjust blocks that take part, in registry insertion order.
///
/// Skips blocks whose flowsheet object is missing or inactive, and blocks not
/// marked [`AdjustBlock::simultaneous_adjust`] — upstream's
/// `Where(adj.SimultaneousAdjust And GraphicObject.Active)`
/// (FlowsheetSolver.vb:1962).
#[must_use]
pub fn active_adjusts(
    flowsheet: &Flowsheet,
    adjusts: &HashMap<ObjectId, AdjustBlock>,
) -> Vec<ObjectId> {
    flowsheet
        .ids_of_type(ObjectType::OtAdjust)
        .into_iter()
        .filter(|id| {
            let active = flowsheet.object(id).is_some_and(|o| o.active);
            let marked = adjusts.get(id).is_some_and(|b| b.simultaneous_adjust);
            active && marked
        })
        .collect()
}

/// `f(x)` — write the manipulated variables, re-solve, read the residuals.
///
/// The port of `FunctionValueSync` (FlowsheetSolver.vb:2163-2202).
fn function_value<S>(
    flowsheet: &mut Flowsheet,
    adjusts: &HashMap<ObjectId, AdjustBlock>,
    active: &[ObjectId],
    x: &[f64],
    resolve: &mut S,
) -> Result<Vec<f64>, SolverError>
where
    S: FnMut(&mut Flowsheet) -> Result<(), SolverError>,
{
    for (i, id) in active.iter().enumerate() {
        adjusts[id].manipulated.set(flowsheet, x[i])?;
    }
    resolve(flowsheet)?;
    let mut fx = Vec::with_capacity(active.len());
    for id in active {
        fx.push(adjusts[id].residual(flowsheet)?);
    }
    Ok(fx)
}

/// `J = d f / d x` by central differences with a 1 % relative perturbation.
///
/// The port of `FunctionGradientSync` (FlowsheetSolver.vb:2211-2243). Returns a
/// row-major `n x n` matrix with `J[k][i] = (f2_k - f3_k) / (x2_i - x3_i)`,
/// which is upstream's `g(k, i)` indexing exactly.
///
/// Costs `2n` full flowsheet solves; each one is counted into
/// [`AdjustSolveReport::flowsheet_solves`].
fn function_gradient<S>(
    flowsheet: &mut Flowsheet,
    adjusts: &HashMap<ObjectId, AdjustBlock>,
    active: &[ObjectId],
    x: &[f64],
    resolve: &mut S,
    report: &mut AdjustSolveReport,
) -> Result<Vec<Vec<f64>>, SolverError>
where
    S: FnMut(&mut Flowsheet) -> Result<(), SolverError>,
{
    let n = x.len();
    let mut g = vec![vec![0.0_f64; n]; n];
    for i in 0..n {
        let mut x2 = x.to_vec();
        let mut x3 = x.to_vec();
        if x[i] != 0.0 {
            x2[i] = x[i] * (1.0 + GRADIENT_EPSILON);
            x3[i] = x[i] * (1.0 - GRADIENT_EPSILON);
        } else {
            // Upstream degenerates to a *forward* difference when the variable
            // is exactly zero (:2229-2230).
            x2[i] = x[i] + GRADIENT_EPSILON;
            x3[i] = x[i];
        }
        let f2 = function_value(flowsheet, adjusts, active, &x2, resolve)?;
        let f3 = function_value(flowsheet, adjusts, active, &x3, resolve)?;
        report.flowsheet_solves += 2;
        let denominator = x2[i] - x3[i];
        for k in 0..n {
            g[k][i] = (f2[k] - f3[k]) / denominator;
        }
    }
    Ok(g)
}

#[cfg(test)]
mod tests {
    //! # Verification — the simultaneous adjust solver
    //!
    //! **Methodology.** Synthetic flowsheets in which the "solve" is an explicit
    //! algebraic map, so the exact root is known in closed form. The adjust
    //! solver must find it within its own tolerance, and the ported damping,
    //! iteration cap and step-floor rules must fire where upstream's do.
    //! Verification of the ported Newton arithmetic only — no thermodynamics,
    //! and no comparison against a DWSIM benchmark case.
    //!
    //! **Results (2026-08-11, release build):** recorded per test.

    use super::*;
    use crate::flowsheet::ObjectType;
    use crate::flowsheet_solver::variables::FlowsheetVariable;

    /// A one-variable rig: a "feed" stream whose mass flow is manipulated, and a
    /// "product" stream whose temperature is the controlled variable. The
    /// `resolve` closure plays the role of the flowsheet physics.
    fn rig() -> (Flowsheet, ObjectId, ObjectId, ObjectId) {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        let adjust = fs.add_object(ObjectType::OtAdjust, Some("ADJ-1"));
        for id in [&feed, &product] {
            let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
        }
        (fs, feed, product, adjust)
    }

    /// **Methodology — one-variable Newton.** The synthetic "physics" is
    /// `T_prod = 300 + 20 * w_feed` \[K\]. The adjust drives `T_prod` to
    /// `400 K` by moving `w_feed`, so the exact answer is
    /// `w* = (400 - 300)/20 = 5 kg/s`. Tolerance `1e-6 K`, initial guess
    /// `w = 1 kg/s`. Because the map is linear, an undamped Newton step would
    /// land exactly; upstream's damping schedule
    /// (`dfac = 0.2, 0.4, 0.6, ...`) means several iterations are expected.
    /// Pass criterion: converged, and `|w - 5| < 1e-4 kg/s`.
    /// **Result (2026-08-11, measured):** converged in **5** Newton iterations
    /// at `w = 5.000000000 kg/s`, error norm `0.0 K^2` (the residual reached
    /// exactly zero), costing **16** flowsheet solves
    /// (`5 * (1 + 2*1) + 1`; the trailing `+1` is the converging evaluation,
    /// which needs no Jacobian).
    #[test]
    fn one_variable_newton_finds_the_exact_root() {
        let (mut fs, feed, product, adjust_id) = rig();
        FlowsheetVariable::MassFlow
            .set(&mut fs, &feed, 1.0)
            .unwrap();

        let mut adjusts = HashMap::new();
        adjusts.insert(
            adjust_id,
            AdjustBlock::new(
                VariableRef::new(feed.clone(), FlowsheetVariable::MassFlow),
                VariableRef::new(product.clone(), FlowsheetVariable::Temperature),
                400.0,
                1e-6,
            ),
        );

        let feed_for_solve = feed.clone();
        let product_for_solve = product.clone();
        let mut resolve = move |fs: &mut Flowsheet| -> Result<(), SolverError> {
            let w = FlowsheetVariable::MassFlow.get(fs, &feed_for_solve)?;
            FlowsheetVariable::Temperature.set(fs, &product_for_solve, 300.0 + 20.0 * w)
        };

        let report =
            solve_simultaneous_adjusts(&mut fs, &adjusts, &mut resolve, &AbortFlag::new()).unwrap();

        assert!(report.converged, "{report:?}");
        assert_eq!(report.variables, 1);
        assert_eq!(report.flowsheet_solves, 3 * report.iterations + 1);
        let w = FlowsheetVariable::MassFlow.get(&fs, &feed).unwrap();
        assert!((w - 5.0).abs() < 1e-4, "w = {w}, report = {report:?}");
        let t = FlowsheetVariable::Temperature.get(&fs, &product).unwrap();
        assert!((t - 400.0).abs() < 1e-3, "T = {t}");
    }

    /// **Methodology — two coupled variables.** Two feeds and two products with
    /// the coupled linear map `T_1 = 300 + 10 w_1 + 2 w_2`,
    /// `T_2 = 280 + 3 w_1 + 8 w_2`. Targets `T_1 = 400 K`, `T_2 = 380 K`. The
    /// exact solution of `10 w_1 + 2 w_2 = 100`, `3 w_1 + 8 w_2 = 100` is
    /// `w_1 = 600/74 = 8.108108... kg/s`, `w_2 = 700/74 = 9.459459... kg/s`.
    /// Tolerance `1e-6 K` on both. Pass criterion: converged to within
    /// `1e-3 kg/s` of the exact pair — which is the whole point of the
    /// *simultaneous* solver, since neither adjust can reach its target alone.
    /// **Result (2026-08-11, measured):** converged in **9** iterations at
    /// `w_1 = 8.108108108 kg/s` and `w_2 = 9.459459459 kg/s` — both matching
    /// `600/74` and `700/74` to nine decimals — with error norm
    /// `3.16e-24 K^2` and **46** flowsheet solves (`9 * (1 + 2*2) + 1`).
    #[test]
    fn two_coupled_adjusts_solve_simultaneously() {
        let mut fs = Flowsheet::new();
        let f1 = fs.add_object(ObjectType::MaterialStream, Some("F1"));
        let f2 = fs.add_object(ObjectType::MaterialStream, Some("F2"));
        let p1 = fs.add_object(ObjectType::MaterialStream, Some("P1"));
        let p2 = fs.add_object(ObjectType::MaterialStream, Some("P2"));
        let a1 = fs.add_object(ObjectType::OtAdjust, Some("ADJ-1"));
        let a2 = fs.add_object(ObjectType::OtAdjust, Some("ADJ-2"));
        for id in [&f1, &f2, &p1, &p2] {
            let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
        }
        FlowsheetVariable::MassFlow.set(&mut fs, &f1, 1.0).unwrap();
        FlowsheetVariable::MassFlow.set(&mut fs, &f2, 1.0).unwrap();

        let mut adjusts = HashMap::new();
        adjusts.insert(
            a1,
            AdjustBlock::new(
                VariableRef::new(f1.clone(), FlowsheetVariable::MassFlow),
                VariableRef::new(p1.clone(), FlowsheetVariable::Temperature),
                400.0,
                1e-6,
            ),
        );
        adjusts.insert(
            a2,
            AdjustBlock::new(
                VariableRef::new(f2.clone(), FlowsheetVariable::MassFlow),
                VariableRef::new(p2.clone(), FlowsheetVariable::Temperature),
                380.0,
                1e-6,
            ),
        );

        let (sf1, sf2, sp1, sp2) = (f1.clone(), f2.clone(), p1.clone(), p2.clone());
        let mut resolve = move |fs: &mut Flowsheet| -> Result<(), SolverError> {
            let w1 = FlowsheetVariable::MassFlow.get(fs, &sf1)?;
            let w2 = FlowsheetVariable::MassFlow.get(fs, &sf2)?;
            FlowsheetVariable::Temperature.set(fs, &sp1, 300.0 + 10.0 * w1 + 2.0 * w2)?;
            FlowsheetVariable::Temperature.set(fs, &sp2, 280.0 + 3.0 * w1 + 8.0 * w2)
        };

        let report =
            solve_simultaneous_adjusts(&mut fs, &adjusts, &mut resolve, &AbortFlag::new()).unwrap();

        assert!(report.converged, "{report:?}");
        assert_eq!(report.variables, 2);
        assert_eq!(report.flowsheet_solves, 5 * report.iterations + 1);
        let w1 = FlowsheetVariable::MassFlow.get(&fs, &f1).unwrap();
        let w2 = FlowsheetVariable::MassFlow.get(&fs, &f2).unwrap();
        assert!((w1 - 600.0 / 74.0).abs() < 1e-3, "w1 = {w1}");
        assert!((w2 - 700.0 / 74.0).abs() < 1e-3, "w2 = {w2}");
    }

    /// **Methodology.** An unreachable target (the controlled variable is
    /// constant, so no manipulated value can satisfy it) must exhaust the cap
    /// and return [`SolverError::AdjustMaxIterations`] rather than looping.
    /// **Result (2026-08-11, measured):** `Err(AdjustMaxIterations)`, reached
    /// after [`MAX_ADJUST_ITERATIONS`] = 25 Newton iterations.
    #[test]
    fn unreachable_target_hits_the_iteration_cap() {
        let (mut fs, feed, product, adjust_id) = rig();
        FlowsheetVariable::MassFlow
            .set(&mut fs, &feed, 1.0)
            .unwrap();

        let mut adjusts = HashMap::new();
        adjusts.insert(
            adjust_id,
            AdjustBlock::new(
                VariableRef::new(feed.clone(), FlowsheetVariable::MassFlow),
                VariableRef::new(product.clone(), FlowsheetVariable::Temperature),
                400.0,
                1e-9,
            ),
        );

        // A saturating response: `T = 300 + 50 tanh(w)` can never exceed 350 K,
        // so the 400 K target has no root. The derivative stays non-zero near
        // the start (so a step is always taken and `dx` never collapses below
        // `MIN_STEP_SUM`) but vanishes as `w` grows, so Newton chases the target
        // forever and the iteration cap is what fires.
        let (sf, sp) = (feed.clone(), product.clone());
        let mut resolve = move |fs: &mut Flowsheet| -> Result<(), SolverError> {
            let w = FlowsheetVariable::MassFlow.get(fs, &sf)?;
            FlowsheetVariable::Temperature.set(fs, &sp, 300.0 + 50.0 * w.tanh())
        };

        let err = solve_simultaneous_adjusts(&mut fs, &adjusts, &mut resolve, &AbortFlag::new())
            .unwrap_err();
        assert_eq!(err, SolverError::AdjustMaxIterations);
    }

    /// **Methodology.** No marked blocks must be a no-op: zero variables,
    /// converged, zero flowsheet solves. Upstream's `If n > 0` guard
    /// (FlowsheetSolver.vb:1965).
    /// **Result (2026-08-11):** `variables = 0`, `converged = true`,
    /// `flowsheet_solves = 0`; the `resolve` closure was never called.
    #[test]
    fn no_marked_adjusts_is_a_no_op() {
        let (mut fs, _feed, _product, _adjust) = rig();
        let adjusts: HashMap<ObjectId, AdjustBlock> = HashMap::new();
        let mut calls = 0usize;
        let mut resolve = |_fs: &mut Flowsheet| -> Result<(), SolverError> {
            calls += 1;
            Ok(())
        };
        let report =
            solve_simultaneous_adjusts(&mut fs, &adjusts, &mut resolve, &AbortFlag::new()).unwrap();
        assert_eq!(report.variables, 0);
        assert!(report.converged);
        assert_eq!(report.flowsheet_solves, 0);
        assert_eq!(calls, 0);
    }

    /// **Methodology.** The abort flag must stop the solve at the top of a
    /// Newton iteration (upstream's `CheckStatus`, FlowsheetSolver.vb:2032).
    /// **Result (2026-08-11):** `Err(Aborted)` on the first iteration, with the
    /// `resolve` closure never called.
    #[test]
    fn abort_flag_stops_the_newton_loop() {
        let (mut fs, feed, product, adjust_id) = rig();
        let mut adjusts = HashMap::new();
        adjusts.insert(
            adjust_id,
            AdjustBlock::new(
                VariableRef::new(feed, FlowsheetVariable::MassFlow),
                VariableRef::new(product, FlowsheetVariable::Temperature),
                400.0,
                1e-6,
            ),
        );
        let mut calls = 0usize;
        let mut resolve = |_fs: &mut Flowsheet| -> Result<(), SolverError> {
            calls += 1;
            Ok(())
        };
        let abort = AbortFlag::new();
        abort.request_abort();
        let err = solve_simultaneous_adjusts(&mut fs, &adjusts, &mut resolve, &abort).unwrap_err();
        assert_eq!(err, SolverError::Aborted);
        assert_eq!(calls, 0);
    }
}
