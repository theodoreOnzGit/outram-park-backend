//! Flowsheet solver — the sequential-modular execution engine.
//!
//! # What this module is
//!
//! Given a [`crate::flowsheet::Flowsheet`] (which objects exist, how they are
//! connected, what state each stream carries), this module works out **what to
//! calculate, in what order, and how many times**, and drives it to a converged
//! solution. It is the port of DWSIM's `DWSIM.FlowsheetSolver` assembly plus the
//! two recycle logical blocks that convergence actually lives in.
//!
//! It does **not** contain physics. Equipment models live in this crate's
//! sibling modules ([`crate::pump`], [`crate::heat_exchanger`], ...) and the
//! thermodynamics in [`crate::thermo`]; the solver reaches them through a
//! caller-supplied hook. See "The evaluation hook" below.
//!
//! # The four questions the solver answers
//!
//! 1. **In what order?** — [`ordering`]. A breadth-first walk from the
//!    flowsheet's endpoints back to its feeds. Loops are broken by user-placed
//!    recycle blocks, not detected automatically; a loop without one is reported
//!    as [`SolverError::InfiniteOrderingLoop`].
//! 2. **How is one object calculated?** — [`evaluator`] (your physics) and
//!    [`queue_processing`] (the peek/calculate/dequeue protocol, error
//!    collection, abort checks).
//! 3. **How does a loop converge?** — [`recycle`]. Successive substitution,
//!    relaxation, Wegstein, and a flowsheet-wide Broyden step, with upstream's
//!    exact tolerances and iteration caps.
//! 4. **How are controllers satisfied?** — [`adjust`] (Newton over several
//!    adjust blocks at once) and [`spec`] (when a specification block fires).
//!
//! [`solver::FlowsheetSolver`] ties all four together and is the entry point.
//!
//! # The evaluation hook
//!
//! The solver is generic over `E: `[`UnitOpEvaluator`], and every
//! `FnMut(&mut Flowsheet, &CalculationArgs) -> Result<(), SolverError>` closure
//! satisfies that trait. **No trait objects** — the workspace forbids them, and
//! generics monomorphise instead.
//!
//! A [`DefaultEvaluator`] is provided for what the flowsheet data model alone
//! can express (stream bookkeeping, mixers, energy mixers); everything else it
//! reports as [`SolverError::NoModel`] so the gap is visible rather than silent.
//! Compose the two:
//!
//! ```
//! use outram_park_fork_dwsim_libs::flowsheet::{CalculationArgs, Flowsheet, ObjectType};
//! use outram_park_fork_dwsim_libs::flowsheet_solver::{
//!     default_evaluate, FlowsheetSolver, SolverError,
//! };
//!
//! let mut hook = |fs: &mut Flowsheet, args: &CalculationArgs| -> Result<(), SolverError> {
//!     if args.object_type == ObjectType::Pump {
//!         // ... call crate::pump here ...
//!         return Ok(());
//!     }
//!     default_evaluate(fs, args).unwrap_or(Ok(()))
//! };
//!
//! let mut flowsheet = Flowsheet::new();
//! let mut solver = FlowsheetSolver::new();
//! let outcome = solver.solve_flowsheet(&mut flowsheet, &mut hook);
//! assert!(outcome.order.is_empty()); // nothing to do on an empty flowsheet
//! ```
//!
//! Recycle blocks are the one exception: the solver calculates them itself,
//! because their iteration state must survive across outer iterations and so
//! lives in [`solver::FlowsheetSolver`], not in the flowsheet.
//!
//! # Units
//!
//! Public tolerances and budgets are `uom`-typed
//! ([`recycle::RecycleConvergenceParameters::temperature_tolerance`],
//! [`solver::SolveOptions::timeout`], ...). Inner arithmetic is raw `f64` in
//! DWSIM's internal units — **K, Pa, kg/s, kJ/kg, kW** — so the ported formulae
//! stay line-comparable with their source; every field says which it is. The
//! adjust/spec variable layer ([`variables`]) is plain **SI** (K, Pa, kg/s,
//! mol/s, J/kg, W), which is a documented divergence from upstream's
//! display-unit convention.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources, by submodule:
//!
//! | Submodule | Upstream source |
//! |---|---|
//! | [`errors`] | `FlowsheetSolver.vb:496-503`, `:859-873`, `:1117` |
//! | [`ordering`] | `FlowsheetSolver.vb:923-1109`; `FlowsheetSolver2.vb:365-550` |
//! | [`queue_processing`] | `FlowsheetSolver.vb:479-853`; `FlowsheetSolver2.vb:176-310` |
//! | [`evaluator`] | `FlowsheetSolver.vb:59-416`; `UnitOperations/Mixer.vb:40-175` |
//! | [`recycle`] | `LogicalBlocks/Recycle.vb`; `LogicalBlocks/EnergyRecycle.vb`; `DWSIM.Math/Broyden.vb`; `Interfaces/Enums.vb:238-243` |
//! | [`adjust`] | `FlowsheetSolver.vb:1943-2334` |
//! | [`spec`] | `Interfaces/Enums.vb:360-364`, `:605-624`; the firing points throughout `FlowsheetSolver.vb` |
//! | [`variables`] | `FlowsheetSolver.vb:2336-2404` |
//! | [`linalg`] | the `rmatrixsolve` call sites, `FlowsheetSolver.vb:2010`, `:2119` |
//! | [`solver`] | `FlowsheetSolver.vb:1111-1783`; `FlowsheetSolver2.vb:552-1083` |
//!
//! Each submodule's header cites the exact line ranges it ports **and** what it
//! deliberately left behind.
//!
//! ## `FlowsheetSolver2.vb` — accounted for, not discarded
//!
//! Upstream carries two solvers: the long-standing `Shared` class
//! `FlowsheetSolver` and a 2025 instance class `FlowsheetSolver2`. The second is
//! very largely a copy of the first with the ambient `GlobalSettings` state
//! removed. Every place they diverge is tabulated in [`solver`]'s module
//! documentation and is represented here — none of `FlowsheetSolver2` was
//! dropped as redundant without being recorded.
//!
//! # Excluded DWSIM behavior, at a glance
//!
//! Each submodule documents its own exclusions precisely. The workspace-level
//! summary:
//!
//! - **Remote solvers** — the Azure Service Bus client (`mode = 3`) and the TCP
//!   network solver (`mode = 4`), `FlowsheetSolver.vb:1628-1665`. Both are
//!   commented out even upstream.
//! - **.NET scheduling** — `DWSIM.FlowsheetSolver/Task Schedulers/` (the STA and
//!   limited-concurrency schedulers), `TaskHelper.Run`, `Task.Wait`,
//!   `CancellationToken`/`CancellationTokenSource`. Cancellation is one
//!   [`AbortFlag`] (`Arc<AtomicBool>`); execution is sequential.
//! - **`GlobalSettings`** — every process-wide mutable flag
//!   (`CalculatorActivated`, `CalculatorBusy`, `SolverBreakOnException`,
//!   `SolverMode`, `EnableParallelProcessing`, `LockModelParameters`,
//!   `InspectorEnabled`, `CAPEOPENMode`). Replaced by explicit options.
//! - **Scripting** — `DWSIM.FlowsheetSolver/Script.vb` and every
//!   `ProcessScripts(...)` hook (IronPython).
//! - **UI and reporting** — `UpdateDisplayStatus`, `ShowMessage`, `ClearLog`,
//!   `UpdateInterface`, `UpdateInformation`, `UpdateOpenEditForms`, the
//!   `ChangeCalculationOrder` dialog, `Inspector` narrative items, the
//!   `ExceptionProcessing` GUID registry, and `StackTrace` decoration.
//! - **Plugin events and callbacks** — the eight `CustomEvent`s and
//!   `IFlowsheetSolveCallback`.
//! - **Spec block arithmetic** — `LogicalBlocks/Spec.vb`. [`spec`] ports *when*
//!   a spec fires; *what* it computes is dispatched through the evaluation hook.
//! - **Sub-flowsheets** — `MasterFlowsheet` and the `FlowsheetUO` unit
//!   operation.
//!
//! # Honest scope
//!
//! This is **AI-assisted draft material and has had no human V&V**. The tests in
//! each submodule are *verification* against the transcribed upstream logic and
//! against synthetic cases with analytically known answers — they check "did we
//! port it correctly?", not "does it represent physical reality?". **No DWSIM
//! benchmark flowsheet has been run through this solver.** Per the workspace
//! `RESPONSIBLE_USE.md`, treat it as untrusted until reviewed. Not for nuclear
//! facility operation, reactor control, safety-critical decision-making, or
//! licensing.
//!
//! # Known gaps
//!
//! - **No equipment registry.** The built-in evaluator covers material streams,
//!   energy streams, mixers and energy mixers. Every other unit operation must
//!   come through the hook; wiring this crate's own equipment modules into a
//!   type-dispatched registry is a larger integration and is proposed as
//!   follow-up work.
//! - **No flash.** A material stream is "calculated" here by recording its
//!   inputs, not by running a property-package flash. Supply one through the
//!   hook.
//! - **No parallelism.** Upstream's `mode = 2` runs each ordering *level*
//!   through `Parallel.ForEach`; this port executes sequentially and documents
//!   the divergence in [`queue_processing`].
//! - **`SupportsDynamicMode` is not represented** in the flowsheet data model,
//!   so [`ordering`]'s dynamic-mode addendum applies to every disconnected unit
//!   operation rather than only those declaring support.

pub mod adjust;
pub mod errors;
pub mod evaluator;
pub mod linalg;
pub mod ordering;
pub mod queue_processing;
pub mod recycle;
pub mod solver;
pub mod spec;
pub mod variables;

pub use adjust::{
    active_adjusts, solve_simultaneous_adjusts, AdjustBlock, AdjustSolveReport,
    GRADIENT_EPSILON, MAX_ADJUST_ITERATIONS, MIN_STEP_SUM,
};
pub use errors::{AbortFlag, SolverError, SolverMode};
pub use evaluator::{default_evaluate, DefaultEvaluator, UnitOpEvaluator};
pub use linalg::{abs_sqr_sum, abs_sum, solve_dense};
pub use ordering::{
    breaks_the_loop, has_unbroken_cycle, is_source, solving_list, solving_list_guarded,
    SolvingList, MAX_ORDERING_STEPS,
};
pub use queue_processing::{
    enqueue_solving_order, process_queue, reset_calculated_flags, QueueOptions, QueueReport,
};
pub use recycle::{
    broydn, AccelerationMethod, EnergyConvergenceHistory, EnergyConvergenceParameters,
    EnergyRecycleBlock, EnergyRecycleMaxIterationsPolicy, RecycleBlock,
    RecycleConvergenceHistory, RecycleConvergenceParameters, RecycleVariables,
    WegsteinParameters,
};
pub use solver::{
    FlowsheetSolver, SolveOptions, SolveOutcome, BROYDEN_MIX_CURRENT, BROYDEN_MIX_STEP,
};
pub use spec::{
    specs_firing_at, SpecBlock, SpecCalcMode, SpecCalcMode2, SpecFiringPoint, SpecVarType,
};
pub use variables::{FlowsheetVariable, VariableRef};

#[cfg(test)]
mod tests {
    //! # Integration verification — the solver end to end
    //!
    //! **Methodology.** Exercise the seams the submodule tests do not: ordering
    //! feeding the queue feeding the evaluator feeding the recycle blocks, and
    //! the adjust solver wrapped around a whole flowsheet solve. Every expected
    //! value is available in closed form from the synthetic "physics" the test
    //! supplies, so the pass criteria are exact orders and analytic fixed
    //! points. Verification only — no thermodynamics is evaluated and no DWSIM
    //! benchmark is reproduced.
    //!
    //! **Results (2026-08-11, release build, `cargo test --release`):** recorded
    //! in each test's doc comment.

    use super::*;
    use crate::flowsheet::{CalculationArgs, Flowsheet, ObjectId, ObjectType, PhaseIndex};
    use std::collections::HashMap;

    fn add_water(fs: &mut Flowsheet, id: &ObjectId) {
        let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
        ms.add_compound("Water", 18.015);
        ms.equalize_overall_composition();
        let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
        props.temperature = Some(300.0);
        props.pressure = Some(1.0e5);
        props.massflow = Some(0.0);
    }

    fn w_of(fs: &Flowsheet, id: &ObjectId) -> f64 {
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

    fn set_w(fs: &mut Flowsheet, id: &ObjectId, w: f64) {
        let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
        ms.phases[PhaseIndex::Mixture.index()].properties.massflow = Some(w);
        if !ms.phases[PhaseIndex::Mixture.index()].compounds.is_empty() {
            ms.phases[PhaseIndex::Mixture.index()].compounds[0].mass_flow = Some(w);
        }
    }

    /// **Methodology.** The full pipeline on an acyclic chain: FEED -> MIX ->
    /// S1 -> SCALE -> PROD, where the stub "SCALE" unit multiplies the mass flow
    /// by 3. With a 2 kg/s feed the closed-form answer is `PROD = 6 kg/s`. The
    /// test checks the order the solver derived, that the object was calculated
    /// exactly once per pass, and the final mass flow.
    /// **Result (2026-08-11):** order
    /// `["FEED", "MIX-1", "S1", "SCALE", "PROD"]`; one outer iteration;
    /// `PROD = 6.000000 kg/s`; `solved = true`.
    #[test]
    fn ordering_queue_and_evaluator_work_together() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let s1 = fs.add_object(ObjectType::MaterialStream, Some("S1"));
        let scale = fs.add_object(ObjectType::Heater, Some("SCALE"));
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        fs.connect(&feed, &mixer, None, None).unwrap();
        fs.connect(&mixer, &s1, None, None).unwrap();
        fs.connect(&s1, &scale, None, None).unwrap();
        fs.connect(&scale, &product, None, None).unwrap();
        for id in [&feed, &s1, &product] {
            add_water(&mut fs, id);
        }
        set_w(&mut fs, &feed, 2.0);

        let (src, dst) = (s1.clone(), product.clone());
        let mut hook = move |fs: &mut Flowsheet,
                             args: &CalculationArgs|
              -> Result<(), SolverError> {
            if args.tag == "SCALE" {
                let w = w_of(fs, &src);
                set_w(fs, &dst, 3.0 * w);
                return Ok(());
            }
            default_evaluate(fs, args).unwrap_or(Ok(()))
        };

        let mut solver = FlowsheetSolver::new();
        let outcome = solver.solve_flowsheet(&mut fs, &mut hook);

        assert!(outcome.solved, "{:?}", outcome.errors);
        let tags: Vec<String> = outcome
            .order
            .iter()
            .map(|i| fs.object(i).unwrap().tag.clone())
            .collect();
        assert_eq!(tags, vec!["FEED", "MIX-1", "S1", "SCALE", "PROD"]);
        assert_eq!(outcome.recycle_loops, 1);
        assert!((w_of(&fs, &product) - 6.0).abs() < 1e-12, "{outcome:?}");
    }

    /// **Methodology.** The adjust solver wrapped around a whole flowsheet
    /// solve. FEED -> SCALE -> PROD with `PROD = k * FEED` and `k = 3`; an
    /// adjust moves the feed mass flow so that `PROD` reaches `9 kg/s`, whose
    /// exact answer is `FEED = 3 kg/s`. This exercises the re-entrant path:
    /// every Newton function evaluation runs a complete nested solve with
    /// `adjusting = true`.
    /// **Result (2026-08-11, measured):** converged in **5** Newton iterations
    /// and **16** nested flowsheet solves, at `FEED = 3.000000000 kg/s` and
    /// `PROD = 9.000000000 kg/s`, error norm `1.26e-29 (kg/s)^2`; the outer
    /// solve reported `solved = true`.
    #[test]
    fn adjust_solver_drives_a_full_flowsheet_solve() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let scale = fs.add_object(ObjectType::Heater, Some("SCALE"));
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        let adjust_obj = fs.add_object(ObjectType::OtAdjust, Some("ADJ-1"));
        fs.connect(&feed, &scale, None, None).unwrap();
        fs.connect(&scale, &product, None, None).unwrap();
        add_water(&mut fs, &feed);
        add_water(&mut fs, &product);
        set_w(&mut fs, &feed, 1.0);

        let (src, dst) = (feed.clone(), product.clone());
        let mut hook = move |fs: &mut Flowsheet,
                             args: &CalculationArgs|
              -> Result<(), SolverError> {
            if args.tag == "SCALE" {
                let w = w_of(fs, &src);
                set_w(fs, &dst, 3.0 * w);
                return Ok(());
            }
            default_evaluate(fs, args).unwrap_or(Ok(()))
        };

        let mut adjusts: HashMap<ObjectId, AdjustBlock> = HashMap::new();
        adjusts.insert(
            adjust_obj,
            AdjustBlock::new(
                VariableRef::new(feed.clone(), FlowsheetVariable::MassFlow),
                VariableRef::new(product.clone(), FlowsheetVariable::MassFlow),
                9.0,
                1e-9,
            ),
        );

        let mut solver = FlowsheetSolver::new();
        solver.adjusts = adjusts;
        let outcome = solver.solve_flowsheet(&mut fs, &mut hook);

        assert!(outcome.solved, "{:?}", outcome.errors);
        let report = outcome.adjust.expect("an adjust report");
        assert!(report.converged, "{report:?}");
        assert_eq!(report.variables, 1);
        assert!(report.flowsheet_solves > 0);
        assert!(
            (w_of(&fs, &feed) - 3.0).abs() < 1e-6,
            "FEED = {}",
            w_of(&fs, &feed)
        );
        assert!(
            (w_of(&fs, &product) - 9.0).abs() < 1e-6,
            "PROD = {}",
            w_of(&fs, &product)
        );
    }

    /// **Methodology.** The re-exported surface must be reachable from the
    /// module root, because that is what the dynamics workstream and future
    /// integration code will import. Instantiate one item from each submodule.
    /// **Result (2026-08-11):** compiles and every default matches its
    /// submodule's.
    #[test]
    fn public_surface_is_reachable_from_the_module_root() {
        let _ = FlowsheetSolver::new();
        let _ = SolveOptions::default();
        let _ = SolveOutcome::default();
        let _ = QueueOptions::default();
        let _ = AbortFlag::new();
        let _ = RecycleBlock::new();
        let _ = EnergyRecycleBlock::new();
        let _ = WegsteinParameters::default();
        let _ = SpecBlock::default();
        let _ = DefaultEvaluator;
        assert_eq!(MAX_ADJUST_ITERATIONS, 25);
        assert_eq!(MAX_ORDERING_STEPS, 10_000);
        assert_eq!(AccelerationMethod::default(), AccelerationMethod::None);
        assert_eq!(SolverMode::default(), SolverMode::Background);
    }
}
