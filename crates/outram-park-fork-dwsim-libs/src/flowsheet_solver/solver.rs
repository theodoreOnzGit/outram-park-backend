//! The master solve routine — ordering, queueing, recycle convergence, adjusts.
//!
//! # What this module is
//!
//! [`FlowsheetSolver::solve_flowsheet`] is the port of DWSIM's `SolveFlowsheet`
//! (`FlowsheetSolver.vb:1111-1783`): the routine a user's "Solve" button calls.
//! It owns the outer loop, and the solver-side state the flowsheet data model
//! does not carry — the recycle blocks, the energy recycles, the adjust blocks,
//! and the spec schedule.
//!
//! ## The shape of a solve
//!
//! ```text
//!   solve_flowsheet
//!     |- ordering::solving_list                       one breadth-first walk
//!     |- (optional) custom calculation order
//!     |- collect recycle blocks, size the Broyden Hessian
//!     |- REPEAT until every recycle converges:            <-- the outer loop
//!     |    |- fire BeforeFlowsheet specs
//!     |    |- enqueue the whole order, clear `calculated`
//!     |    |- process_queue  ---> per object: fire specs, then
//!     |    |                       recycle block   (handled here), or
//!     |    |                       evaluation hook (your equipment physics)
//!     |    |- solve simultaneous adjusts   (unless already adjusting)
//!     |    |- fire AfterFlowsheet specs, then DO THE WHOLE PASS AGAIN
//!     |    |- poll every recycle's `converged`
//!     |    +- if not converged and any recycle is on GlobalBroyden:
//!     |         one global quasi-Newton step across all of them
//!     +- update the mass and energy balance
//! ```
//!
//! ## Where the unbounded work is
//!
//! Three nested loops have no iteration bound of their own upstream, which
//! matters for any real-time use:
//!
//! 1. **The outer recycle loop** (`While Not converged`, FlowsheetSolver.vb:1377)
//!    has **no iteration cap at all**. It is bounded only by each recycle
//!    block's own `MaximumIterations` (default 50, Recycle.vb:44) throwing, and
//!    by a wall-clock timeout polled on a separate thread (:1588). This port
//!    adds an optional explicit bound,
//!    [`SolveOptions::max_recycle_loops`], defaulting to `None` = faithful.
//! 2. **The simultaneous adjust solver** costs `1 + 2n` **complete flowsheet
//!    solves** per Newton iteration, capped at 25 iterations — see
//!    [`crate::flowsheet_solver::adjust`].
//! 3. **`SpecCalcMode::AfterFlowsheet`** re-runs the entire calculation pass a
//!    second time inside every outer iteration (:1440-1474), doubling its cost.
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
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:1111-1783` (`SolveFlowsheet`),
//!   `:59-254` (`CalculateObject`), `:345-416` (`CalculateMaterialStream`),
//!   `:479-494` (`ProcessCalculationQueue`), `:1785-1820` and `:1822-1887`
//!   (`CalculateObject(fobj, ObjID)` / `CalculateObjectSync`).
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver2.vb:552-1083` — the 2025 rewrite of
//!   the same routine. **Ported divergences are listed below**; everything else
//!   is a verbatim duplicate.
//!
//! ## `FlowsheetSolver2` — what is different, and what this port did with it
//!
//! `FlowsheetSolver2` is an instance class (not `Shared`) carrying its own
//! `SolverTimeoutSeconds` and `ThisCancellationToken`
//! (FlowsheetSolver2.vb:30-32). Line for line its `SolveFlowsheet` matches
//! `FlowsheetSolver`'s, minus the following. Each is accounted for:
//!
//! | Divergence | Upstream | This port |
//! |---|---|---|
//! | No `mode` parameter — always the single background-task path | FS2:559-564 vs FS:1120-1125 | [`SolverMode`] is retained (all modes run sequentially anyway) |
//! | No `GlobalSettings` at all: no `CalculatorActivated`, `CalculatorBusy`, `LockModelParameters`, `SolverBreakOnException` | FS2 throughout vs FS:1127-1199, :1753-1759 | ambient globals are excluded from the port entirely; `break_on_exception` is an explicit option |
//! | Always breaks the queue drain on the first error | FS2:288, :296 | [`QueueOptions::break_on_exception`] = `true` |
//! | Always throws on any queue error, with no `SolverBreakOnException` guard | FS2:853-855 vs FS:1478-1480 | same option |
//! | No `WaitingForUserDefinedOrder` spin-wait for the custom-order dialog | FS2:638-659 vs FS:1244-1275 | the GUI dialog is excluded; [`SolveOptions::custom_calculation_order`] is supplied by the caller |
//! | Timeout defaults to 60 s on the instance | FS2:30 | [`SolveOptions::timeout`] default, `uom`-typed |
//! | No `IFlowsheetSolveCallback` registration | FS:45-50 | excluded (see below) |
//!
//! Everything in `FlowsheetSolver2` is therefore represented; nothing was
//! discarded as redundant without being recorded here.
//!
//! # Excluded DWSIM behavior
//!
//! - **Ambient global state** — `GlobalSettings.Settings.CalculatorActivated`
//!   (:1129), `CalculatorBusy` (:1133-1137, :1179-1183, :1192, :1753),
//!   `LockModelParameters` (:1127, :1759), `SolverBreakOnException` (:1478),
//!   `SolverMode`, `EnableParallelProcessing`, `InspectorEnabled` (:1147-1150),
//!   `IsRunningOnMono` (:1598), `CAPEOPENMode`. All process-wide mutable flags;
//!   this port takes explicit options and `&mut Flowsheet`.
//! - **The remote solvers** — Azure Service Bus (`mode = 3`, :1628-1646) and the
//!   TCP network solver (`mode = 4`, :1648-1665). Both are commented out
//!   upstream and both are out of scope.
//! - **.NET task and thread plumbing** — `TaskHelper.Run`, `maintask.Wait(500)`,
//!   `Task.Status`, `maintask.Dispose`, `Thread.Sleep(500)`,
//!   `CancellationTokenSource` (:1349-1617). The port runs on the calling
//!   thread; the wall-clock timeout is checked between outer iterations rather
//!   than polled from a watchdog thread. **Consequence:** a single very long
//!   outer iteration can overrun [`SolveOptions::timeout`], where upstream would
//!   abandon it mid-flight.
//! - **Plugin events** — `UnitOpCalculationStarted/Finished`,
//!   `MaterialStreamCalculationStarted/Finished`,
//!   `FlowsheetCalculationStarted/Finished`, `CalculationError`,
//!   `CalculatingObject` (:36-43), and `RegisterCallback` /
//!   `IFlowsheetSolveCallback` (:45-50, :1139-1143).
//! - **Script hooks** — `ProcessScripts(SolverStarted / SolverRecycleLoop /
//!   SolverFinished / ObjectCalculation*)` (:1305, :1527, :1755, and every
//!   per-object site). IronPython integration.
//! - **The spreadsheet bridge** — `UpdateSpreadsheet`,
//!   `WriteSpreadsheetVariables` (:1309-1310, :1746-1747).
//! - **UI** — `ShowMessage`, `ClearLog`, `UpdateInterface`, `UpdateInformation`,
//!   `UpdateOpenEditForms`, `UpdateDisplayStatus` (:882-921), `ChangeCalculationOrder`
//!   dialog, `Inspector` items, `ExceptionProcessing.ExceptionList` GUID
//!   registry, and the `StackTrace` file/line decoration (:1724-1731).
//! - **Pre-flight validation** — `PropertyPackages.Count = 0` and
//!   `SelectedCompounds.Count = 0` (:1164-1176). Neither collection exists in
//!   the flowsheet data model; compounds live on each stream.
//! - **`FlowsheetOptions.ForceStreamPhase` warning** (:1298-1301) — the flag is
//!   on the stream (`MaterialStreamData::forced_phase`) and is the flash's
//!   business, not the solver's.
//! - **`MasterFlowsheet` / sub-flowsheet nesting** (:1133, :1192, :1196, :1681).
//!   The `FlowsheetUO` sub-flowsheet unit operation is not ported.
//! - **`AttachedUtilities`** auto-update throughout.
//!
//! # Honest scope
//!
//! AI-assisted draft with **no human V&V**. No DWSIM benchmark flowsheet has
//! been run through this solver. The tests are *verification* against the
//! transcribed upstream control flow and against synthetic cases with
//! analytically known answers. Not for nuclear facility operation, reactor
//! control, safety-critical decisions, or licensing.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uom::si::f64::Time;
use uom::si::time::second;

use crate::flowsheet::{CalculationArgs, CalculationSender, Flowsheet, ObjectId, ObjectType};
use crate::flowsheet_solver::adjust::{
    solve_simultaneous_adjusts, AdjustBlock, AdjustSolveReport,
};
use crate::flowsheet_solver::errors::{AbortFlag, SolverError, SolverMode};
use crate::flowsheet_solver::evaluator::UnitOpEvaluator;
use crate::flowsheet_solver::ordering::{self, SolvingList};
use crate::flowsheet_solver::queue_processing::{
    enqueue_solving_order, process_queue, reset_calculated_flags, QueueOptions, QueueReport,
};
use crate::flowsheet_solver::recycle::{
    broydn, AccelerationMethod, EnergyRecycleBlock, RecycleBlock, RecycleVariables,
};
use crate::flowsheet_solver::spec::{
    specs_firing_at, SpecBlock, SpecCalcMode, SpecFiringPoint,
};

/// The two mixing weights of the global Broyden update
/// (`0.3 * recvars(i) + 0.7 * recdvars(i)`, FlowsheetSolver.vb:1560).
///
/// Note that `recdvars` is the **step** `p`, not `x + p`, so the update is
/// `x_next = 0.3 x + 0.7 p` — not the textbook `x + p`. Reproduced verbatim.
/// Dimensionless.
pub const BROYDEN_MIX_CURRENT: f64 = 0.3;
/// See [`BROYDEN_MIX_CURRENT`].
pub const BROYDEN_MIX_STEP: f64 = 0.7;

/// Everything a solve needs to know that is not the flowsheet itself.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveOptions {
    /// Which execution mode is nominally requested — see [`SolverMode`]. All
    /// modes run sequentially in this port.
    pub mode: SolverMode,
    /// Upstream's `frompgrid` (FlowsheetSolver.vb:1121). `true` runs the
    /// incremental forward ordering walk from the head of the calculation queue
    /// instead of the full backward walk, and re-derives the order on every
    /// outer iteration (:1571-1579).
    pub from_property_grid: bool,
    /// Upstream's `Adjusting` (FlowsheetSolver.vb:1121). `true` means this solve
    /// is itself a function evaluation of the simultaneous adjust solver, which
    /// suppresses a nested adjust solve (:490).
    pub adjusting: bool,
    /// Stop the queue drain at the first object failure — upstream's
    /// `SolverBreakOnException` (:1478, :599). `FlowsheetSolver2` behaves as
    /// though this were always `true`.
    pub break_on_exception: bool,
    /// Forwarded to the queue as [`QueueOptions::isolated`].
    pub isolated: bool,
    /// The flowsheet-wide spec firing default
    /// (`FlowsheetOptions.SpecCalculationMode`).
    pub spec_calculation_mode: SpecCalcMode,
    /// Wall-clock budget for the whole solve. Upstream's
    /// `Settings.SolverTimeoutSeconds`, whose `FlowsheetSolver2` default is
    /// **60 s** (FlowsheetSolver2.vb:30). Checked between outer iterations —
    /// see the module's exclusion note on thread plumbing.
    pub timeout: Time,
    /// An explicit cap on the outer recycle loop. **`None` is faithful to
    /// upstream, which has no cap** (FlowsheetSolver.vb:1377). Set it for
    /// real-time or unattended use; exceeding it yields
    /// [`SolverError::Other`] naming the bound.
    pub max_recycle_loops: Option<usize>,
    /// A user-defined calculation order to merge over the computed one —
    /// upstream's `FlowsheetOptions.CustomCalculationOrder`
    /// (FlowsheetSolver.vb:1246-1271). Entries not in the computed order are
    /// dropped and entries missing from it are appended, exactly as upstream
    /// reconciles the two lists.
    pub custom_calculation_order: Option<Vec<ObjectId>>,
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions {
            mode: SolverMode::default(),
            from_property_grid: false,
            adjusting: false,
            break_on_exception: true,
            isolated: false,
            spec_calculation_mode: SpecCalcMode::default(),
            timeout: Time::new::<second>(60.0),
            max_recycle_loops: None,
            custom_calculation_order: None,
        }
    }
}

/// What a solve did.
///
/// Replaces upstream's `List(Of Exception)` return plus the several log lines it
/// writes on the way (`FSstartedsolving`, the per-loop recycle error,
/// `FSfinishedsolvingok`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SolveOutcome {
    /// Whether the solve finished with no errors — upstream's `fs.Solved`
    /// (FlowsheetSolver.vb:1694).
    pub solved: bool,
    /// Every error collected, flattened.
    pub errors: Vec<SolverError>,
    /// The calculation order that was used.
    pub order: Vec<ObjectId>,
    /// How many times the outer recycle loop ran — upstream's `icount`.
    pub recycle_loops: usize,
    /// How many objects were handed to the evaluator in total, across all outer
    /// iterations.
    pub objects_calculated: usize,
    /// The last simultaneous-adjust report, if the solver ran one.
    pub adjust: Option<AdjustSolveReport>,
    /// The average recycle error of the last non-converged iteration, in percent
    /// — upstream's `avgerr` (FlowsheetSolver.vb:1497-1511).
    pub average_recycle_error_percent: Option<f64>,
    /// Wall-clock time spent.
    pub elapsed: Duration,
}

/// The sequential-modular flowsheet solver.
///
/// # What it owns
///
/// The solver-side state the flowsheet data model deliberately does not carry:
/// the recycle blocks (whose iteration counters must survive across outer
/// iterations), the energy recycles, the adjust blocks, and the spec schedule.
/// Register them yourself, or call [`FlowsheetSolver::sync_blocks`] to create a
/// default block for every recycle/adjust/spec object present in a flowsheet.
///
/// # How to drive it
///
/// ```
/// use outram_park_fork_dwsim_libs::flowsheet::{Flowsheet, ObjectType};
/// use outram_park_fork_dwsim_libs::flowsheet_solver::{DefaultEvaluator, FlowsheetSolver};
///
/// let mut fs = Flowsheet::new();
/// let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
/// let mixer = fs.add_object(ObjectType::Mixer, None);
/// let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
/// fs.connect(&feed, &mixer, None, None).unwrap();
/// fs.connect(&mixer, &product, None, None).unwrap();
/// for id in [&feed, &product] {
///     let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
///     ms.add_compound("Water", 18.015);
///     ms.equalize_overall_composition();
/// }
///
/// let mut solver = FlowsheetSolver::new();
/// let outcome = solver.solve_flowsheet(&mut fs, &mut DefaultEvaluator);
/// assert!(outcome.solved, "{:?}", outcome.errors);
/// ```
///
/// Supply your own equipment physics by passing a closure instead of
/// [`crate::flowsheet_solver::evaluator::DefaultEvaluator`] — see
/// [`crate::flowsheet_solver::evaluator`].
#[derive(Debug, Clone, Default)]
pub struct FlowsheetSolver {
    /// Solve options. Mutable between solves.
    pub options: SolveOptions,
    /// The shared stop request. Clone it to hand a stop button to another
    /// thread.
    pub abort: AbortFlag,
    /// Material recycle blocks, keyed by their [`ObjectType::OtRecycle`] object.
    pub recycles: HashMap<ObjectId, RecycleBlock>,
    /// Energy recycle blocks, keyed by their [`ObjectType::OtEnergyRecycle`]
    /// object.
    pub energy_recycles: HashMap<ObjectId, EnergyRecycleBlock>,
    /// Adjust blocks, keyed by their [`ObjectType::OtAdjust`] object. Only
    /// blocks marked [`AdjustBlock::simultaneous_adjust`] are solved.
    pub adjusts: HashMap<ObjectId, AdjustBlock>,
    /// Spec schedule, keyed by [`ObjectType::OtSpec`] object.
    pub specs: HashMap<ObjectId, SpecBlock>,
}

impl FlowsheetSolver {
    /// A solver with default options, no blocks registered, and a cleared abort
    /// flag.
    #[must_use]
    pub fn new() -> Self {
        FlowsheetSolver::default()
    }

    /// A solver with the given options.
    #[must_use]
    pub fn with_options(options: SolveOptions) -> Self {
        FlowsheetSolver {
            options,
            ..FlowsheetSolver::default()
        }
    }

    /// Create a default block for every recycle, energy recycle and spec object
    /// in `flowsheet` that does not already have one, and forget blocks whose
    /// object has been deleted.
    ///
    /// Adjust blocks are **not** created, because an
    /// [`AdjustBlock`] has no meaningful default — it must name a manipulated
    /// and a controlled variable. Register those yourself.
    ///
    /// Called automatically at the top of [`FlowsheetSolver::solve_flowsheet`].
    pub fn sync_blocks(&mut self, flowsheet: &Flowsheet) {
        for id in flowsheet.ids_of_type(ObjectType::OtRecycle) {
            self.recycles.entry(id).or_default();
        }
        for id in flowsheet.ids_of_type(ObjectType::OtEnergyRecycle) {
            self.energy_recycles.entry(id).or_default();
        }
        for id in flowsheet.ids_of_type(ObjectType::OtSpec) {
            self.specs.entry(id).or_default();
        }
        self.recycles.retain(|id, _| flowsheet.contains(id));
        self.energy_recycles.retain(|id, _| flowsheet.contains(id));
        self.specs.retain(|id, _| flowsheet.contains(id));
        self.adjusts.retain(|id, _| flowsheet.contains(id));
    }

    /// Reset every recycle block's iteration state — DWSIM's `DeCalculate`
    /// across the flowsheet.
    ///
    /// Call this before an unrelated solve so a previous run's iteration
    /// counters and error history do not leak into it.
    pub fn reset_blocks(&mut self) {
        for block in self.recycles.values_mut() {
            *block = RecycleBlock::new();
        }
        for block in self.energy_recycles.values_mut() {
            *block = EnergyRecycleBlock::new();
        }
    }

    /// Solve the whole flowsheet — the port of `SolveFlowsheet`
    /// (FlowsheetSolver.vb:1111-1783).
    ///
    /// See the module documentation for the shape of the algorithm and for what
    /// was excluded. This never panics and never returns `Err`: every failure is
    /// collected into [`SolveOutcome::errors`], which is upstream's contract
    /// (it returns a `List(Of Exception)`).
    ///
    /// # Side effects on `flowsheet`
    ///
    /// - The calculation queue is filled and drained, and left empty (:1621).
    /// - Every object in the order has its `calculated` flag rewritten.
    /// - Streams are written by the evaluator and by the recycle blocks.
    /// - [`crate::flowsheet::Flowsheet::results`] is refreshed via
    ///   `update_mass_and_energy_balance` (:1669), and `solved` /
    ///   `error_message` are set (:1687-1741).
    pub fn solve_flowsheet<E: UnitOpEvaluator>(
        &mut self,
        flowsheet: &mut Flowsheet,
        evaluator: &mut E,
    ) -> SolveOutcome {
        let options = self.options.clone();
        self.solve_with_options(flowsheet, evaluator, options)
    }

    /// [`FlowsheetSolver::solve_flowsheet`] with an explicit option set,
    /// bypassing [`FlowsheetSolver::options`].
    ///
    /// Used internally to re-enter the solver with `adjusting = true` during a
    /// simultaneous adjust solve, which is what stops the recursion
    /// (FlowsheetSolver.vb:2175).
    pub fn solve_with_options<E: UnitOpEvaluator>(
        &mut self,
        flowsheet: &mut Flowsheet,
        evaluator: &mut E,
        options: SolveOptions,
    ) -> SolveOutcome {
        let started = Instant::now();
        let mut outcome = SolveOutcome::default();

        // `Settings.CalculatorStopRequested = False` (:1152-1154). Only the
        // top-level solve clears it; a nested adjust evaluation must not wipe a
        // stop the user just requested.
        if !options.adjusting {
            self.abort.clear();
        }
        self.sync_blocks(flowsheet);

        // ---- ordering (:1214-1237) -------------------------------------
        let list: SolvingList = match ordering::solving_list(flowsheet, options.from_property_grid)
        {
            Ok(list) => list,
            Err(error) => {
                flowsheet.solved = false;
                flowsheet.error_message = Some(error.to_string());
                outcome.errors.push(error);
                outcome.elapsed = started.elapsed();
                return outcome;
            }
        };
        let mut order = apply_custom_order(list.stack, options.custom_calculation_order.as_ref());

        // (:1285-1289) an empty order is a silent no-op upstream.
        if order.is_empty() {
            outcome.elapsed = started.elapsed();
            return outcome;
        }
        outcome.order = order.clone();

        flowsheet.solved = false;
        flowsheet.error_message = None;

        // ---- recycles and the Broyden workspace (:1314-1347) ------------
        let recycle_ids: Vec<ObjectId> = order
            .iter()
            .filter(|id| {
                flowsheet
                    .object(id)
                    .is_some_and(|o| o.object_type == ObjectType::OtRecycle)
            })
            .cloned()
            .collect();

        let mut total_variables = 0usize;
        for id in &recycle_ids {
            let uses_broyden = self
                .recycles
                .get(id)
                .is_some_and(|b| b.acceleration_method == AccelerationMethod::GlobalBroyden);
            if !uses_broyden {
                continue;
            }
            // `If rec.Values.Count = 0 Then ...Solve()` (:1329) — prime the
            // block so it has values to contribute.
            let needs_priming = self.recycles.get(id).is_some_and(|b| b.value_count() == 0);
            if needs_priming {
                if let Some(mut block) = self.recycles.remove(id) {
                    if let Err(error) = block.calculate(flowsheet, id) {
                        outcome.errors.push(error);
                    }
                    self.recycles.insert(id.clone(), block);
                }
            }
            total_variables += self
                .recycles
                .get(id)
                .map_or(0, crate::flowsheet_solver::recycle::RecycleBlock::value_count);
        }

        // Identity matrix as the first Hessian (:1341-1347). These vectors live
        // outside the loop upstream too, so the Broyden memory persists.
        let mut hessian = vec![vec![0.0_f64; total_variables]; total_variables];
        for (i, row) in hessian.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        let mut rec_vars = vec![0.0_f64; total_variables];
        let mut rec_errs = vec![0.0_f64; total_variables];
        let mut rec_dvars = vec![0.0_f64; total_variables];
        let mut rec_vars_b = vec![0.0_f64; total_variables];
        let mut rec_errs_b = vec![0.0_f64; total_variables];

        let queue_options = QueueOptions {
            isolated: options.isolated,
            flowsheet_solver_mode: false,
            break_on_exception: options.break_on_exception,
        };
        let timeout = Duration::from_secs_f64(options.timeout.get::<second>().max(0.0));

        // ---- the outer loop (:1377-1583) --------------------------------
        let mut icount = 0usize;
        loop {
            if let Err(error) = self.abort.check() {
                outcome.errors.push(error);
                break;
            }
            if started.elapsed() > timeout {
                outcome
                    .errors
                    .push(SolverError::Timeout(options.timeout.get::<second>()));
                break;
            }
            if let Some(cap) = options.max_recycle_loops {
                if icount >= cap {
                    outcome.errors.push(SolverError::Other(format!(
                        "recycle loop exceeded the configured bound of {cap} iterations"
                    )));
                    break;
                }
            }

            // Specs scheduled before the pass (:1383-1390).
            if let Err(error) =
                self.fire_specs(flowsheet, evaluator, options.spec_calculation_mode, &SpecFiringPoint::BeforeFlowsheet)
            {
                outcome.errors.push(error);
                break;
            }

            match self.run_one_pass(flowsheet, evaluator, &order, queue_options, &options) {
                Ok((report, adjust)) => {
                    outcome.objects_calculated += report.calculated;
                    outcome.errors.extend(report.errors.iter().cloned());
                    if adjust.is_some() {
                        outcome.adjust = adjust;
                    }
                    if options.break_on_exception && !report.errors.is_empty() {
                        break;
                    }
                }
                Err(error) => {
                    outcome.errors.push(error);
                    break;
                }
            }

            // `SpecCalcMode.AfterFlowsheet`: fire, then do the whole pass again
            // (:1440-1474).
            if options.spec_calculation_mode == SpecCalcMode::AfterFlowsheet {
                if let Err(error) = self.fire_specs(
                    flowsheet,
                    evaluator,
                    options.spec_calculation_mode,
                    &SpecFiringPoint::AfterFlowsheet,
                ) {
                    outcome.errors.push(error);
                    break;
                }
                match self.run_one_pass(flowsheet, evaluator, &order, queue_options, &options) {
                    Ok((report, adjust)) => {
                        outcome.objects_calculated += report.calculated;
                        outcome.errors.extend(report.errors.iter().cloned());
                        if adjust.is_some() {
                            outcome.adjust = adjust;
                        }
                        if options.break_on_exception && !report.errors.is_empty() {
                            break;
                        }
                    }
                    Err(error) => {
                        outcome.errors.push(error);
                        break;
                    }
                }
            }

            // ---- recycle convergence (:1482-1493) ------------------------
            let mut converged = true;
            for id in &recycle_ids {
                converged = self.recycles.get(id).is_some_and(|b| b.converged);
                if !converged {
                    break;
                }
            }
            // "in dynamic mode, recycles are redundant" (:1491-1493).
            if flowsheet.dynamic_mode {
                converged = true;
            }

            if !converged {
                outcome.average_recycle_error_percent =
                    Some(self.average_recycle_error_percent(&recycle_ids));
            }

            if converged {
                icount += 1;
                break;
            }

            // ---- the global Broyden step (:1537-1567) --------------------
            if total_variables > 0 {
                self.global_broyden_step(
                    flowsheet,
                    &recycle_ids,
                    icount,
                    &mut rec_vars,
                    &mut rec_errs,
                    &mut rec_dvars,
                    &mut rec_vars_b,
                    &mut rec_errs_b,
                    &mut hessian,
                    &mut outcome,
                );
            }

            // The property-grid path re-derives the order every iteration
            // (:1571-1579).
            if options.from_property_grid {
                match ordering::solving_list(flowsheet, false) {
                    Ok(list) => {
                        order = apply_custom_order(
                            list.stack,
                            options.custom_calculation_order.as_ref(),
                        );
                        outcome.order = order.clone();
                    }
                    Err(error) => {
                        outcome.errors.push(error);
                        break;
                    }
                }
            }

            icount += 1;
        }

        outcome.recycle_loops = icount;

        // ---- wrap up (:1619-1741) ---------------------------------------
        flowsheet.calculation_queue.clear();
        flowsheet.results = flowsheet.update_mass_and_energy_balance();
        if !options.adjusting {
            self.abort.clear();
        }

        outcome.solved = outcome.errors.is_empty();
        flowsheet.solved = outcome.solved;
        flowsheet.error_message = outcome.errors.first().map(std::string::ToString::to_string);
        outcome.elapsed = started.elapsed();
        outcome
    }

    /// One complete pass over the calculation order: enqueue, clear the
    /// `calculated` flags, drain the queue, then run the simultaneous adjust
    /// solver unless this solve *is* one.
    ///
    /// The port of the body of the outer loop (:1392-1436) plus
    /// `ProcessCalculationQueue` (:484-494).
    fn run_one_pass<E: UnitOpEvaluator>(
        &mut self,
        flowsheet: &mut Flowsheet,
        evaluator: &mut E,
        order: &[ObjectId],
        queue_options: QueueOptions,
        options: &SolveOptions,
    ) -> Result<(QueueReport, Option<AdjustSolveReport>), SolverError> {
        enqueue_solving_order(flowsheet, order);
        reset_calculated_flags(flowsheet, order);

        let report = {
            let recycles = &mut self.recycles;
            let energy_recycles = &mut self.energy_recycles;
            let specs = &self.specs;
            let spec_mode = options.spec_calculation_mode;
            let mut calculate = |fs: &mut Flowsheet,
                                 args: &CalculationArgs,
                                 _queue: QueueOptions|
             -> Result<(), SolverError> {
                calculate_one_object(
                    fs,
                    args,
                    recycles,
                    energy_recycles,
                    specs,
                    spec_mode,
                    evaluator,
                )
            };
            process_queue(flowsheet, queue_options, &self.abort, &mut calculate)?
        };

        // `If Not Adjusting Then SolveSimultaneousAdjustsAsync(...)` (:490).
        let adjust = if options.adjusting || self.adjusts.is_empty() {
            None
        } else {
            Some(self.run_adjust_solve(flowsheet, evaluator, options)?)
        };

        Ok((report, adjust))
    }

    /// Run the simultaneous adjust solver, re-entering this solver with
    /// `adjusting = true` for every function evaluation.
    ///
    /// The adjust map is moved out of `self` for the duration so the re-entrant
    /// closure can borrow the rest of the solver; it is restored before
    /// returning, including on the error path.
    fn run_adjust_solve<E: UnitOpEvaluator>(
        &mut self,
        flowsheet: &mut Flowsheet,
        evaluator: &mut E,
        options: &SolveOptions,
    ) -> Result<AdjustSolveReport, SolverError> {
        let adjusts = std::mem::take(&mut self.adjusts);
        let abort = self.abort.clone();
        let mut inner = options.clone();
        inner.adjusting = true;
        inner.from_property_grid = false;

        let result = {
            let mut resolve = |fs: &mut Flowsheet| -> Result<(), SolverError> {
                let nested = self.solve_with_options(fs, evaluator, inner.clone());
                match nested.errors.into_iter().next() {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            };
            solve_simultaneous_adjusts(flowsheet, &adjusts, &mut resolve, &abort)
        };
        self.adjusts = adjusts;
        result
    }

    /// Fire every spec scheduled at `point`, in registry order, by dispatching
    /// the spec object through the evaluation hook.
    ///
    /// See [`crate::flowsheet_solver::spec`] for why the spec's own arithmetic
    /// is not ported here.
    fn fire_specs<E: UnitOpEvaluator>(
        &mut self,
        flowsheet: &mut Flowsheet,
        evaluator: &mut E,
        global: SpecCalcMode,
        point: &SpecFiringPoint,
    ) -> Result<(), SolverError> {
        let firing = specs_firing_at(flowsheet, &self.specs, global, point);
        for id in firing {
            let Some(args) = spec_args(flowsheet, &id) else {
                continue;
            };
            evaluator.evaluate(flowsheet, &args)?;
        }
        Ok(())
    }

    /// The average recycle error, in percent — upstream's `avgerr`
    /// (FlowsheetSolver.vb:1497-1511), reproduced including its weighting.
    ///
    /// `avgerr = 100 / n * sum over recycles of
    /// (0.33 dT/T + 0.33 dP/P + 0.33 dW/W)`.
    ///
    /// **Two upstream hazards are reproduced, not fixed:** the weights sum to
    /// `0.99`, not `1`, and each term divides by a value that is zero on the
    /// very first iteration (before any history exists), giving `inf` or `NaN`.
    /// Upstream only logs the number, and so does this port — it never gates a
    /// decision on it.
    fn average_recycle_error_percent(&self, recycle_ids: &[ObjectId]) -> f64 {
        let mut total = 0.0_f64;
        let mut count = 0usize;
        for id in recycle_ids {
            let Some(block) = self.recycles.get(id) else {
                continue;
            };
            let h = block.convergence_history;
            total += 0.33 * h.temperature_err / h.temperature;
            total += 0.33 * h.pressure_err / h.pressure;
            total += 0.33 * h.mass_flow_err / h.mass_flow;
            count += 1;
        }
        if count == 0 {
            return 0.0;
        }
        total * 100.0 / count as f64
    }

    /// The global Broyden update across every recycle marked
    /// [`AccelerationMethod::GlobalBroyden`] (FlowsheetSolver.vb:1537-1567).
    ///
    /// Packs their values and errors into one vector, takes a
    /// [`broydn`] step, mixes it back as
    /// `0.3 * value + 0.7 * step` (see [`BROYDEN_MIX_CURRENT`]), and then calls
    /// `SetOutletStreamProperties` on **every** recycle — Broyden-marked or not,
    /// which is where upstream puts the call (:1564, outside the `If`).
    #[allow(clippy::too_many_arguments)]
    fn global_broyden_step(
        &mut self,
        flowsheet: &mut Flowsheet,
        recycle_ids: &[ObjectId],
        icount: usize,
        rec_vars: &mut [f64],
        rec_errs: &mut [f64],
        rec_dvars: &mut [f64],
        rec_vars_b: &mut [f64],
        rec_errs_b: &mut [f64],
        hessian: &mut [Vec<f64>],
        outcome: &mut SolveOutcome,
    ) {
        let n = rec_vars.len();
        let mut i = 0usize;
        for id in recycle_ids {
            let Some(block) = self.recycles.get(id) else {
                continue;
            };
            if block.acceleration_method != AccelerationMethod::GlobalBroyden {
                continue;
            }
            let values = block.values.as_pairs();
            let errors = block.errors.as_pairs();
            for k in 0..RecycleVariables::LEN {
                if i >= n {
                    break;
                }
                rec_vars[i] = values[k].1;
                rec_errs[i] = errors[k].1;
                i += 1;
            }
        }

        // `If(icount < 2, 0, 1)` (:1553): the first two calls do not update the
        // Hessian.
        let update = icount >= 2;
        let ok = broydn(
            n,
            rec_vars,
            rec_errs,
            rec_dvars,
            rec_vars_b,
            rec_errs_b,
            hessian,
            update,
        );

        let mut i = 0usize;
        for id in recycle_ids {
            if ok {
                if let Some(block) = self.recycles.get_mut(id) {
                    if block.acceleration_method == AccelerationMethod::GlobalBroyden {
                        let mut mixed = [0.0_f64; RecycleVariables::LEN];
                        for slot in mixed.iter_mut() {
                            if i >= n {
                                break;
                            }
                            *slot = BROYDEN_MIX_CURRENT * rec_vars[i]
                                + BROYDEN_MIX_STEP * rec_dvars[i];
                            i += 1;
                        }
                        block.values.set_from_slice(&mixed);
                    }
                }
            }
            // Upstream calls this for every recycle (:1564).
            if let Some(block) = self.recycles.get(id).cloned() {
                if let Err(error) = block.set_outlet_stream_properties(flowsheet, id) {
                    outcome.errors.push(error);
                }
            }
        }
    }
}

/// Calculate one queue entry — the port of `CalculateObject`
/// (FlowsheetSolver.vb:59-254) and `CalculateMaterialStream` (:345-416),
/// restricted to the `Sender = "FlowsheetSolver"` path the solver itself uses.
///
/// # Dispatch, in order
///
/// 1. Fire any spec scheduled [`SpecFiringPoint::BeforeTargetObject`] or
///    [`SpecFiringPoint::BeforeObject`] on this object (:83-94, :365-376).
/// 2. If the object is a **recycle** or **energy recycle**, run its block here —
///    the solver owns that state, so no evaluator can.
/// 3. Otherwise hand it to the evaluation hook.
/// 4. Fire any spec scheduled [`SpecFiringPoint::AfterSourceObject`] or
///    [`SpecFiringPoint::AfterObject`] (:109-120, :384-395).
///
/// # Excluded DWSIM behavior
///
/// The whole `Sender <> "FlowsheetSolver"` half of `CalculateObject`
/// (:78-129, :140-176, :212-247) — the interactive path in which editing a
/// material stream pulls its downstream unit operation and that unit
/// operation's outlet streams into the calculation, and `DeCalculate` is
/// called when the request is a de-calculation. The solver never takes that
/// path because it always enqueues with
/// [`CalculationSender::FlowsheetSolver`]; a GUI front-end would need it.
fn calculate_one_object<E: UnitOpEvaluator>(
    flowsheet: &mut Flowsheet,
    args: &CalculationArgs,
    recycles: &mut HashMap<ObjectId, RecycleBlock>,
    energy_recycles: &mut HashMap<ObjectId, EnergyRecycleBlock>,
    specs: &HashMap<ObjectId, SpecBlock>,
    spec_mode: SpecCalcMode,
    evaluator: &mut E,
) -> Result<(), SolverError> {
    let id = ObjectId(args.name.clone());

    fire_specs_around(
        flowsheet,
        specs,
        spec_mode,
        evaluator,
        &[
            SpecFiringPoint::BeforeTargetObject(id.clone()),
            SpecFiringPoint::BeforeObject(id.clone()),
        ],
    )?;

    match args.object_type {
        ObjectType::OtRecycle => {
            let mut block = recycles.remove(&id).unwrap_or_default();
            let result = if flowsheet.dynamic_mode {
                block.run_dynamic_model(flowsheet, &id)
            } else {
                block.calculate(flowsheet, &id)
            };
            recycles.insert(id.clone(), block);
            result?;
        }
        ObjectType::OtEnergyRecycle => {
            let mut block = energy_recycles.remove(&id).unwrap_or_default();
            let result = if flowsheet.dynamic_mode {
                block.run_dynamic_model();
                Ok(())
            } else {
                block.calculate(flowsheet, &id)
            };
            energy_recycles.insert(id.clone(), block);
            result?;
        }
        _ => evaluator.evaluate(flowsheet, args)?,
    }

    fire_specs_around(
        flowsheet,
        specs,
        spec_mode,
        evaluator,
        &[
            SpecFiringPoint::AfterSourceObject(id.clone()),
            SpecFiringPoint::AfterObject(id),
        ],
    )?;
    Ok(())
}

/// Fire every spec scheduled at any of `points`, in the order the points are
/// given and, within each, in registry order.
fn fire_specs_around<E: UnitOpEvaluator>(
    flowsheet: &mut Flowsheet,
    specs: &HashMap<ObjectId, SpecBlock>,
    global: SpecCalcMode,
    evaluator: &mut E,
    points: &[SpecFiringPoint],
) -> Result<(), SolverError> {
    for point in points {
        for spec_id in specs_firing_at(flowsheet, specs, global, point) {
            let Some(args) = spec_args(flowsheet, &spec_id) else {
                continue;
            };
            evaluator.evaluate(flowsheet, &args)?;
        }
    }
    Ok(())
}

/// Build the [`CalculationArgs`] used to dispatch a spec block, attributed to
/// the sender upstream writes as the literal string `"Spec"`
/// (FlowsheetSolver.vb:75).
fn spec_args(flowsheet: &Flowsheet, spec_id: &ObjectId) -> Option<CalculationArgs> {
    let obj = flowsheet.object(spec_id)?;
    Some(CalculationArgs::for_object(
        obj,
        CalculationSender::Other("Spec".to_string()),
    ))
}

/// Merge a user-defined calculation order over the computed one.
///
/// The port of FlowsheetSolver.vb:1246-1271: entries of the custom list that are
/// no longer in the computed order are dropped, entries of the computed order
/// missing from the custom list are appended in computed order, and the result
/// is the custom list. An empty or absent custom list leaves the computed order
/// alone.
fn apply_custom_order(computed: Vec<ObjectId>, custom: Option<&Vec<ObjectId>>) -> Vec<ObjectId> {
    let Some(custom) = custom else {
        return computed;
    };
    if custom.is_empty() {
        return computed;
    }
    let mut merged: Vec<ObjectId> = custom
        .iter()
        .filter(|id| computed.contains(id))
        .cloned()
        .collect();
    for id in computed {
        if !merged.contains(&id) {
            merged.push(id);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    //! # Verification — the master solve routine
    //!
    //! **Methodology.** Drive [`FlowsheetSolver::solve_flowsheet`] on small
    //! flowsheets with a stub evaluator whose "physics" is an explicit algebraic
    //! map, so every expected result is available in closed form. Checks cover:
    //! the acyclic happy path, the recycle outer loop reaching a known fixed
    //! point, error propagation and the abort flag, the outer-loop bound, and
    //! the custom-order merge. Pass criterion: exact orders, exact flags, and
    //! convergence to the analytic fixed point within the recycle block's own
    //! tolerance.
    //!
    //! These are verification tests against the transcribed upstream control
    //! flow. **No DWSIM benchmark flowsheet has been run**, so nothing here is
    //! validation.
    //!
    //! **Results (2026-08-11, release build):** recorded per test.

    use super::*;
    use crate::flowsheet::PhaseIndex;
    use crate::flowsheet_solver::evaluator::{default_evaluate, DefaultEvaluator};

    fn add_water(fs: &mut Flowsheet, id: &ObjectId) {
        let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
        ms.add_compound("Water", 18.015);
        ms.equalize_overall_composition();
    }

    fn mass_flow(fs: &Flowsheet, id: &ObjectId) -> f64 {
        fs.object(id)
            .unwrap()
            .data
            .as_material()
            .unwrap()
            .phase(PhaseIndex::Mixture)
            .properties
            .massflow
            .unwrap_or(0.0)
    }

    fn set_mass_flow(fs: &mut Flowsheet, id: &ObjectId, w: f64) {
        let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
        ms.phases[PhaseIndex::Mixture.index()].properties.massflow = Some(w);
        ms.phases[PhaseIndex::Mixture.index()].compounds[0].mass_flow = Some(w);
        ms.phases[PhaseIndex::Mixture.index()].properties.temperature = Some(300.0);
        ms.phases[PhaseIndex::Mixture.index()].properties.pressure = Some(1.0e5);
    }

    /// **Methodology.** FEED -> MIX-1 -> PROD with [`DefaultEvaluator`], which
    /// covers all three object types. The solve must succeed, run exactly one
    /// outer iteration (no recycles), report the order feeds-first, and set
    /// `flowsheet.solved`.
    /// **Result (2026-08-11, measured):** `solved = true`, no errors,
    /// `recycle_loops = 1`, order `["FEED", "MIX-1", "PROD"]`,
    /// `objects_calculated = 3`, `flowsheet.solved = true`, product mass flow
    /// `3.000000 kg/s` carried through by the mixer shortcut.
    #[test]
    fn acyclic_flowsheet_solves_in_one_pass() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        fs.connect(&feed, &mixer, None, None).unwrap();
        fs.connect(&mixer, &product, None, None).unwrap();
        add_water(&mut fs, &feed);
        add_water(&mut fs, &product);
        set_mass_flow(&mut fs, &feed, 3.0);

        let mut solver = FlowsheetSolver::new();
        let outcome = solver.solve_flowsheet(&mut fs, &mut DefaultEvaluator);

        assert!(outcome.solved, "{:?}", outcome.errors);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.recycle_loops, 1);
        assert_eq!(outcome.objects_calculated, 3);
        let tags: Vec<String> = outcome
            .order
            .iter()
            .map(|i| fs.object(i).unwrap().tag.clone())
            .collect();
        assert_eq!(tags, vec!["FEED", "MIX-1", "PROD"]);
        assert!(fs.solved);
        assert!(fs.calculation_queue.is_empty());
        // The mixer's single-active-inlet shortcut carried the feed through.
        assert!((mass_flow(&fs, &product) - 3.0).abs() < 1e-12);
    }

    /// **Methodology — the recycle outer loop against a known fixed point.**
    /// A loop `FEED + RECY -> MIX -> S1 -> SPLIT-ish -> RY-1 -> RECY`, in which
    /// the stub evaluator's "physics" for the loop is `w_out = 0.5 * w_in`, and
    /// the mixer adds the 4 kg/s feed. The tear mass flow therefore satisfies
    /// `w = 0.5 (4 + w)`, whose exact solution is `w* = 4 kg/s`, and the mixer
    /// outlet settles at `8 kg/s`. Pass criterion: the solve converges, the tear
    /// stream is within the recycle's own `0.01 kg/s` tolerance of `4 kg/s`, and
    /// more than one outer iteration was needed (proving the loop actually ran).
    /// **Result (2026-08-11, measured):** converged with `solved = true` after
    /// **9** outer recycle iterations; tear stream `w = 3.992188 kg/s`
    /// (`|w - 4| = 7.8e-03 kg/s`) and mixer outlet `w = 7.984375 kg/s`.
    /// **Interpretation:** the recycle stops on
    /// `|w_in - w_out| <= 0.01 kg/s`, and here `w_in - w_out = 0.5 (4 + w) - w`
    /// so that criterion admits `|w - 4| <= 0.02 kg/s`; the measured
    /// `7.8e-03 kg/s` is comfortably inside it, so the loop halted exactly where
    /// upstream's criterion says it should.
    #[test]
    fn recycle_loop_converges_to_the_analytic_fixed_point() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let s1 = fs.add_object(ObjectType::MaterialStream, Some("S1"));
        let unit = fs.add_object(ObjectType::Heater, Some("HALVE"));
        let s2 = fs.add_object(ObjectType::MaterialStream, Some("S2"));
        let block = fs.add_object(ObjectType::OtRecycle, Some("RY-1"));
        let tear = fs.add_object(ObjectType::MaterialStream, Some("RECY"));

        fs.connect(&feed, &mixer, None, Some(0)).unwrap();
        fs.connect(&mixer, &s1, None, None).unwrap();
        fs.connect(&s1, &unit, None, None).unwrap();
        fs.connect(&unit, &s2, None, None).unwrap();
        fs.connect(&s2, &block, None, None).unwrap();
        fs.connect(&block, &tear, None, None).unwrap();
        fs.connect(&tear, &mixer, None, Some(1)).unwrap();

        for id in [&feed, &s1, &s2, &tear] {
            add_water(&mut fs, id);
        }
        set_mass_flow(&mut fs, &feed, 4.0);
        set_mass_flow(&mut fs, &s1, 0.0);
        set_mass_flow(&mut fs, &s2, 0.0);
        set_mass_flow(&mut fs, &tear, 0.0);

        // The stub "physics": the HALVE unit halves whatever S1 carries.
        let (in_id, out_id) = (s1.clone(), s2.clone());
        let mut evaluator = move |fs: &mut Flowsheet,
                                  args: &CalculationArgs|
              -> Result<(), SolverError> {
            if args.object_type == ObjectType::Heater {
                let w = fs
                    .object(&in_id)
                    .and_then(|o| o.data.as_material())
                    .and_then(|m| m.phase(PhaseIndex::Mixture).properties.massflow)
                    .unwrap_or(0.0);
                let ms = fs
                    .object_mut(&out_id)
                    .unwrap()
                    .data
                    .as_material_mut()
                    .unwrap();
                ms.phases[PhaseIndex::Mixture.index()].properties.massflow = Some(0.5 * w);
                ms.phases[PhaseIndex::Mixture.index()].compounds[0].mass_flow = Some(0.5 * w);
                ms.phases[PhaseIndex::Mixture.index()].properties.temperature = Some(300.0);
                ms.phases[PhaseIndex::Mixture.index()].properties.pressure = Some(1.0e5);
                return Ok(());
            }
            match default_evaluate(fs, args) {
                Some(result) => result,
                None => Ok(()),
            }
        };

        let mut solver = FlowsheetSolver::with_options(SolveOptions {
            max_recycle_loops: Some(200),
            ..SolveOptions::default()
        });
        let outcome = solver.solve_flowsheet(&mut fs, &mut evaluator);

        assert!(outcome.solved, "{:?}", outcome.errors);
        assert!(
            outcome.recycle_loops > 1,
            "the outer loop must have run more than once, got {}",
            outcome.recycle_loops
        );
        let w = mass_flow(&fs, &tear);
        assert!(
            (w - 4.0).abs() <= 1e-2,
            "tear stream w = {w}, expected 4 +/- 0.01 (report: {outcome:?})"
        );
    }

    /// **Methodology.** An evaluator that always fails, with
    /// `break_on_exception = true`: the solve must stop, report the failure
    /// attributed to the object's tag, leave `flowsheet.solved = false`, and
    /// record the message on the object.
    /// **Result (2026-08-11):** `solved = false`; one error, `"HT-1: no model"`;
    /// `fs.solved = false`; `fs.error_message` set;
    /// `HT-1.error_message = Some("no model")`.
    #[test]
    fn evaluator_failure_stops_the_solve_and_is_attributed() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        fs.connect(&feed, &heater, None, None).unwrap();
        fs.connect(&heater, &product, None, None).unwrap();
        add_water(&mut fs, &feed);
        add_water(&mut fs, &product);

        let mut evaluator =
            |fs: &mut Flowsheet, args: &CalculationArgs| -> Result<(), SolverError> {
                if args.object_type == ObjectType::Heater {
                    return Err(SolverError::Other("no model".to_string()));
                }
                match default_evaluate(fs, args) {
                    Some(result) => result,
                    None => Ok(()),
                }
            };

        let mut solver = FlowsheetSolver::new();
        let outcome = solver.solve_flowsheet(&mut fs, &mut evaluator);

        assert!(!outcome.solved);
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].to_string(), "HT-1: no model");
        assert!(!fs.solved);
        assert!(fs.error_message.is_some());
        assert_eq!(
            fs.object(&heater).unwrap().error_message.as_deref(),
            Some("no model")
        );
    }

    /// **Methodology.** A recycle that can never converge (the "physics" keeps
    /// the loop diverging) with `max_recycle_loops = Some(5)` must stop at the
    /// bound rather than spinning — the port-side addition documented on
    /// [`SolveOptions::max_recycle_loops`].
    /// **Result (2026-08-11):** stopped after **5** outer iterations with an
    /// error naming the bound; `solved = false`.
    #[test]
    fn outer_loop_bound_is_enforced() {
        let mut fs = Flowsheet::new();
        let s2 = fs.add_object(ObjectType::MaterialStream, Some("S2"));
        let block = fs.add_object(ObjectType::OtRecycle, Some("RY-1"));
        let tear = fs.add_object(ObjectType::MaterialStream, Some("RECY"));
        fs.connect(&s2, &block, None, None).unwrap();
        fs.connect(&block, &tear, None, None).unwrap();
        add_water(&mut fs, &s2);
        add_water(&mut fs, &tear);
        set_mass_flow(&mut fs, &s2, 1.0);
        set_mass_flow(&mut fs, &tear, 0.0);

        // Every pass doubles S2, so the recycle error never shrinks.
        let s2_for_eval = s2.clone();
        let mut evaluator = move |fs: &mut Flowsheet,
                                  args: &CalculationArgs|
              -> Result<(), SolverError> {
            if args.name == s2_for_eval.0 {
                let w = mass_flow(fs, &s2_for_eval);
                set_mass_flow(fs, &s2_for_eval, w * 2.0);
                return Ok(());
            }
            match default_evaluate(fs, args) {
                Some(result) => result,
                None => Ok(()),
            }
        };

        let mut solver = FlowsheetSolver::with_options(SolveOptions {
            max_recycle_loops: Some(5),
            ..SolveOptions::default()
        });
        let outcome = solver.solve_flowsheet(&mut fs, &mut evaluator);

        assert!(!outcome.solved);
        assert_eq!(outcome.recycle_loops, 5);
        assert!(
            outcome
                .errors
                .iter()
                .any(|e| e.to_string().contains("bound of 5")),
            "{:?}",
            outcome.errors
        );
    }

    /// **Methodology.** Raising the abort flag before the solve must stop it
    /// immediately with [`SolverError::Aborted`].
    /// **Result (2026-08-11):** `solved = false`, one error `Aborted`,
    /// `recycle_loops = 0`, no object calculated.
    #[test]
    fn abort_flag_stops_the_solve() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        fs.connect(&feed, &mixer, None, None).unwrap();
        fs.connect(&mixer, &product, None, None).unwrap();
        add_water(&mut fs, &feed);
        add_water(&mut fs, &product);

        let mut solver = FlowsheetSolver::with_options(SolveOptions {
            adjusting: true, // keeps `solve_with_options` from clearing the flag
            ..SolveOptions::default()
        });
        solver.abort.request_abort();
        let outcome = solver.solve_flowsheet(&mut fs, &mut DefaultEvaluator);

        assert!(!outcome.solved);
        assert_eq!(outcome.errors, vec![SolverError::Aborted]);
        assert_eq!(outcome.objects_calculated, 0);
    }

    /// **Methodology.** [`apply_custom_order`] must reproduce upstream's
    /// reconciliation (FlowsheetSolver.vb:1246-1271): drop custom entries no
    /// longer in the computed order, append computed entries missing from the
    /// custom list, and leave the computed order alone when the custom list is
    /// absent or empty.
    /// **Result (2026-08-11):** `computed = [a, b, c]`,
    /// `custom = [c, z, a]` gives `[c, a, b]`; `None` and `Some(vec![])` both
    /// give `[a, b, c]`.
    #[test]
    fn custom_order_merge_matches_upstream() {
        let id = |s: &str| ObjectId(s.to_string());
        let computed = vec![id("a"), id("b"), id("c")];
        let custom = vec![id("c"), id("z"), id("a")];
        assert_eq!(
            apply_custom_order(computed.clone(), Some(&custom)),
            vec![id("c"), id("a"), id("b")]
        );
        assert_eq!(apply_custom_order(computed.clone(), None), computed);
        assert_eq!(
            apply_custom_order(computed.clone(), Some(&vec![])),
            computed
        );
    }

    /// **Methodology.** [`FlowsheetSolver::sync_blocks`] must create a default
    /// block for every recycle/energy-recycle/spec object and forget blocks
    /// whose object was deleted, while never inventing an adjust block.
    /// **Result (2026-08-11):** one recycle block, one energy-recycle block and
    /// one spec block created, no adjust block; after deleting the recycle
    /// object, its block is dropped.
    #[test]
    fn sync_blocks_tracks_the_flowsheet() {
        let mut fs = Flowsheet::new();
        let recycle = fs.add_object(ObjectType::OtRecycle, Some("RY-1"));
        let _energy = fs.add_object(ObjectType::OtEnergyRecycle, Some("ER-1"));
        let _spec = fs.add_object(ObjectType::OtSpec, Some("SP-1"));
        let _adjust = fs.add_object(ObjectType::OtAdjust, Some("ADJ-1"));

        let mut solver = FlowsheetSolver::new();
        solver.sync_blocks(&fs);
        assert_eq!(solver.recycles.len(), 1);
        assert_eq!(solver.energy_recycles.len(), 1);
        assert_eq!(solver.specs.len(), 1);
        assert!(solver.adjusts.is_empty());

        fs.remove_object(&recycle).unwrap();
        solver.sync_blocks(&fs);
        assert!(solver.recycles.is_empty());
    }
}
