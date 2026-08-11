//! Draining the calculation queue.
//!
//! # What this module is
//!
//! The inner loop of the solver: take the object at the head of
//! [`crate::flowsheet::Flowsheet::calculation_queue`], calculate it, record
//! success or failure on it, dequeue, repeat. It is the port of DWSIM's
//! `ProcessQueueInternal` family.
//!
//! It knows nothing about *what* calculating an object means — that is
//! [`crate::flowsheet_solver::evaluator`]'s job, reached through the `calculate`
//! closure — and nothing about ordering, which happened before the queue was
//! filled. It owns exactly three concerns: the peek/dequeue protocol, the
//! error-collection policy, and the abort checks.
//!
//! # The three upstream variants, and why one implementation covers them
//!
//! | Upstream | Lines | Difference | Here |
//! |---|---|---|---|
//! | `ProcessQueueInternal` | :513-624 | synchronous; honours `Isolated` and `FlowsheetSolverMode`; guards on `SimulationObjects.ContainsKey`; calls `CheckCalculatorStatus` after each item | this is the one ported, guards and all |
//! | `ProcessQueueInternalAsync` | :632-737 | identical control flow with a `CancellationToken` instead of `CheckCalculatorStatus`, and **no** `ContainsKey` guard (it would throw on a dangling id) | same code path; the abort flag covers the token, and the guard is kept because losing it can only turn a skip into a panic |
//! | `ProcessQueueInternalAsyncParallel` | :745-853 | runs `Parallel.ForEach` over each *level* of the ordering, after giving every object a private property-package clone (:753-761) and stripping them again afterwards (:843-849) | **not ported as parallel** — see below |
//!
//! ## The parallel variant is deliberately sequential here
//!
//! Upstream's parallel mode exists to overlap flash calculations across objects
//! that have no dependency on each other, which it identifies as the members of
//! one *level* of the ordering
//! ([`crate::flowsheet_solver::ordering::SolvingList::filtered_levels`]). Making
//! that safe requires giving each object its own property-package instance,
//! because DWSIM's packages carry a mutable `CurrentMaterialStream`. This port
//! has no property-package plumbing to clone, and the workspace's shared-state
//! rule would express the sharing as `Arc<RwLock<Flowsheet>>` — which would
//! serialise the very writes the parallelism is for.
//!
//! **So: correctness first.** [`process_queue`] executes every item on the
//! calling thread, in queue order. Level order is preserved, so the *result* is
//! identical to upstream's parallel mode for any evaluator without cross-object
//! side effects; only the wall-clock time differs. Re-introducing real
//! parallelism is proposed as a follow-up, and would go here.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources: `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:479-494`
//! (`ProcessCalculationQueue`), `:505-624`, `:626-737`, `:739-853`;
//! `DWSIM.FlowsheetSolver/FlowsheetSolver2.vb:176-310` — the newer class's copy,
//! which differs only in dropping `Settings.SolverBreakOnException` and always
//! breaking on the first error (`Exit While` unconditionally at :288, :296).
//! That behaviour is available here as
//! [`QueueOptions::break_on_exception`] = `true`.
//!
//! # Excluded DWSIM behavior
//!
//! - **`AggregateException` unwrapping** — four nested levels of it, repeated in
//!   all three variants (:562-598, :676-713, :789-827). .NET task aggregation
//!   has no analogue; errors are collected flat.
//! - **`GraphicObject.Status`** transitions (`Calculating` / `Calculated` /
//!   `ErrorCalculating`) and `fgui.UpdateInterface()` — GUI state.
//! - **`myobj.AttachedUtilities` auto-update** (:553-555, :669-671) — the
//!   utility/plug-in system.
//! - **`myobj.UpdateEditForm()` / `UpdateDynamicsEditForm()`** (:558-559).
//! - **`myobj.LastUpdated = Date.Now`** (:557) — the flowsheet data model
//!   carries no per-object timestamp.
//! - **`Inspector` items** (:654-658, :728).
//! - **The runtime log line** `"Runtime (s): ..."` (:620) — replaced by
//!   [`QueueReport::elapsed`].
//! - **Property-package cloning for parallel execution** (:753-761, :843-849) —
//!   see above.
//!
//! # Honest scope
//!
//! AI-assisted draft with **no human V&V**. The tests verify the queue protocol
//! and the error policy against the transcribed upstream loop.

use std::time::{Duration, Instant};

use crate::flowsheet::{CalculationArgs, CalculationSender, Flowsheet, ObjectId};
use crate::flowsheet_solver::errors::{AbortFlag, SolverError};

/// How [`process_queue`] treats the items it drains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueueOptions {
    /// Upstream's `Isolated` (FlowsheetSolver.vb:509, forwarded as the `OnlyMe`
    /// argument of `CalculateObject`/`CalculateMaterialStream`, :541-551).
    ///
    /// When `true`, an object must be calculated **without** following its
    /// outlet connections to pull further objects in — "only the objects in the
    /// queue must be calculated". This port passes the flag through to the
    /// `calculate` closure by way of [`QueueOptions`]; the closure is what
    /// decides whether to propagate, because propagation is model behaviour.
    pub isolated: bool,
    /// Upstream's `FlowsheetSolverMode` (FlowsheetSolver.vb:510, :538-545).
    ///
    /// When `true`, **only** queue entries whose sender is
    /// [`CalculationSender::FlowsheetSolver`] are calculated; everything else is
    /// dequeued untouched. This is how upstream stops a stale property-grid
    /// request from being replayed mid-solve.
    pub flowsheet_solver_mode: bool,
    /// Upstream's `GlobalSettings.Settings.SolverBreakOnException`
    /// (FlowsheetSolver.vb:599, :607).
    ///
    /// When `true`, the first failure stops the drain **and leaves the failing
    /// item at the head of the queue** — upstream's `Exit While` skips the
    /// `Dequeue` at :616. When `false`, the drain continues and every failure is
    /// collected.
    pub break_on_exception: bool,
}

/// What a queue drain did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueueReport {
    /// How many queue entries were dequeued.
    pub dequeued: usize,
    /// How many objects were actually handed to `calculate` — smaller than
    /// [`QueueReport::dequeued`] when entries were skipped as inactive, missing,
    /// or filtered out by [`QueueOptions::flowsheet_solver_mode`].
    pub calculated: usize,
    /// Every failure, in the order they happened.
    pub errors: Vec<SolverError>,
    /// Wall-clock time spent, standing in for upstream's `"Runtime (s)"` log
    /// line (FlowsheetSolver.vb:620).
    pub elapsed: Duration,
    /// Whether the drain stopped early because of
    /// [`QueueOptions::break_on_exception`].
    pub stopped_early: bool,
}

/// Drain the flowsheet's calculation queue, calculating each entry.
///
/// # Arguments
///
/// - `flowsheet` — the queue lives on it, and `calculate` mutates it.
/// - `options` — see [`QueueOptions`].
/// - `abort` — checked at the top of each item and again after it, matching
///   upstream's `ct.IsCancellationRequested` (:525) and `CheckCalculatorStatus`
///   (:614).
/// - `calculate` — how to calculate one object. Receives the flowsheet and the
///   queue entry. The solver supplies a closure that fires specs, handles
///   recycle blocks, and otherwise delegates to the evaluation hook.
///
/// # Protocol, reproduced exactly
///
/// Upstream **peeks**, calculates, then dequeues at the very end of the
/// iteration (:527, :616). Two consequences this port keeps:
///
/// - a failing item under [`QueueOptions::break_on_exception`] stays at the head
///   of the queue, so the caller can inspect it;
/// - `calculate` sees a queue that still contains the item it is working on,
///   which some models rely on.
///
/// # Errors
///
/// Returns `Err(`[`SolverError::Aborted`]`)` if the abort flag is raised.
/// Per-object failures are **not** returned as `Err`; they are collected into
/// [`QueueReport::errors`] and recorded on the object's
/// [`crate::flowsheet::FlowsheetObject::error_message`], which is what upstream
/// does.
pub fn process_queue<F>(
    flowsheet: &mut Flowsheet,
    options: QueueOptions,
    abort: &AbortFlag,
    calculate: &mut F,
) -> Result<QueueReport, SolverError>
where
    F: FnMut(&mut Flowsheet, &CalculationArgs, QueueOptions) -> Result<(), SolverError>,
{
    let start = Instant::now();
    let mut report = QueueReport::default();

    while !flowsheet.calculation_queue.is_empty() {
        abort.check()?;

        let Some(info) = flowsheet.calculation_queue.peek().cloned() else {
            break;
        };
        let id = ObjectId(info.name.clone());

        // Upstream's `ContainsKey` guard (:531): a dangling queue entry is
        // dequeued and ignored, not an error.
        if flowsheet.contains(&id) {
            let active = flowsheet.object(&id).is_some_and(|o| o.active);
            let sender_ok = !options.flowsheet_solver_mode
                || info.sender == CalculationSender::FlowsheetSolver;

            if let Some(obj) = flowsheet.object_mut(&id) {
                obj.error_message = None;
            }

            if active && sender_ok {
                report.calculated += 1;
                match calculate(flowsheet, &info, options) {
                    Ok(()) => {
                        if let Some(obj) = flowsheet.object_mut(&id) {
                            obj.calculated = true;
                            obj.dirty = false;
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        if let Some(obj) = flowsheet.object_mut(&id) {
                            obj.calculated = false;
                            obj.error_message = Some(message.clone());
                        }
                        report.errors.push(SolverError::for_object(
                            info.tag.clone(),
                            info.name.clone(),
                            message,
                        ));
                        if options.break_on_exception {
                            // Upstream `Exit While` (:599, :607) skips the
                            // dequeue, leaving the failure at the head.
                            report.stopped_early = true;
                            report.elapsed = start.elapsed();
                            return Ok(report);
                        }
                    }
                }
            }
        }

        // `CheckCalculatorStatus()` (:614), before the dequeue.
        abort.check()?;

        flowsheet.calculation_queue.dequeue();
        report.dequeued += 1;
    }

    report.elapsed = start.elapsed();
    Ok(report)
}

/// Enqueue every object of `order`, attributed to
/// [`CalculationSender::FlowsheetSolver`], and clear each one's
/// `at_equilibrium` flag if it is a material stream.
///
/// The port of the queue-filling block the master routine runs at the top of
/// every outer iteration (FlowsheetSolver.vb:1394-1411, repeated verbatim at
/// :1453-1470). Objects not present in the flowsheet are skipped, matching
/// upstream's `ContainsKey` guard (:1395).
pub fn enqueue_solving_order(flowsheet: &mut Flowsheet, order: &[ObjectId]) {
    for id in order {
        if !flowsheet.contains(id) {
            continue;
        }
        if let Some(obj) = flowsheet.object_mut(id) {
            if let Some(ms) = obj.data.as_material_mut() {
                ms.at_equilibrium = false;
            }
        }
        let args = {
            let obj = flowsheet.object(id).expect("checked by contains");
            CalculationArgs::for_object(obj, CalculationSender::FlowsheetSolver)
        };
        // Upstream sets `.Calculated = True` on the *args*, which is a
        // "this request is live" marker, not a statement about the object
        // (FlowsheetSolver.vb:1404).
        let mut args = args;
        args.calculated = true;
        flowsheet.calculation_queue.enqueue(args);
    }
}

/// Mark every object of `order` not-calculated before a pass.
///
/// The port of FlowsheetSolver.vb:1421-1434. Inactive objects are left alone
/// beyond the flag, since upstream only logs a warning for them (:1429-1430).
pub fn reset_calculated_flags(flowsheet: &mut Flowsheet, order: &[ObjectId]) {
    for id in order {
        if let Some(obj) = flowsheet.object_mut(id) {
            obj.calculated = false;
        }
    }
}

#[cfg(test)]
mod tests {
    //! # Verification — queue processing
    //!
    //! **Methodology.** Drive [`process_queue`] with a stub `calculate` closure
    //! that records which objects it saw and can be made to fail on demand, then
    //! check the drain order, the `calculated` flags, the error policy, and the
    //! abort behaviour against the transcribed loop at
    //! `FlowsheetSolver.vb:513-624`. Pass criterion: exact sequences and exact
    //! flag states. No physics is involved.
    //! **Results (2026-08-11, release build):** recorded per test.

    use super::*;
    use crate::flowsheet::ObjectType;

    fn rig() -> (Flowsheet, Vec<ObjectId>) {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::MaterialStream, Some("A"));
        let b = fs.add_object(ObjectType::Heater, None);
        let c = fs.add_object(ObjectType::MaterialStream, Some("C"));
        (fs, vec![a, b, c])
    }

    /// **Methodology.** Enqueue three objects in order and drain with a closure
    /// that always succeeds. The closure must see them in queue order, all three
    /// must end `calculated = true` and `dirty = false`, and the queue must be
    /// empty.
    /// **Result (2026-08-11):** closure saw `["A", "HT-1", "C"]`; all three
    /// `calculated = true`, `dirty = false`; `dequeued = 3`, `calculated = 3`,
    /// no errors, `stopped_early = false`; queue empty.
    #[test]
    fn drains_in_order_and_marks_everything_calculated() {
        let (mut fs, ids) = rig();
        enqueue_solving_order(&mut fs, &ids);
        reset_calculated_flags(&mut fs, &ids);

        let mut seen: Vec<String> = Vec::new();
        let mut calculate =
            |_fs: &mut Flowsheet, args: &CalculationArgs, _o: QueueOptions| -> Result<(), SolverError> {
                seen.push(args.tag.clone());
                Ok(())
            };
        let report = process_queue(
            &mut fs,
            QueueOptions::default(),
            &AbortFlag::new(),
            &mut calculate,
        )
        .unwrap();

        assert_eq!(seen, vec!["A", "HT-1", "C"]);
        assert_eq!(report.dequeued, 3);
        assert_eq!(report.calculated, 3);
        assert!(report.errors.is_empty());
        assert!(!report.stopped_early);
        assert!(fs.calculation_queue.is_empty());
        for id in &ids {
            let obj = fs.object(id).unwrap();
            assert!(obj.calculated, "{} should be calculated", obj.tag);
            assert!(!obj.dirty);
        }
    }

    /// **Methodology.** Fail on the middle object with
    /// `break_on_exception = false`: the drain must continue, the error must be
    /// attributed to the failing object's tag and recorded on it, and the
    /// failing object must be left `calculated = false` while its neighbours are
    /// `true`.
    /// **Result (2026-08-11):** all three dequeued; one error, message
    /// `"HT-1: boom"`; `HT-1.calculated = false` with
    /// `error_message = Some("boom")`; `A` and `C` `calculated = true`.
    #[test]
    fn continues_past_an_error_and_attributes_it() {
        let (mut fs, ids) = rig();
        enqueue_solving_order(&mut fs, &ids);

        let mut calculate =
            |_fs: &mut Flowsheet, args: &CalculationArgs, _o: QueueOptions| -> Result<(), SolverError> {
                if args.tag == "HT-1" {
                    Err(SolverError::Other("boom".to_string()))
                } else {
                    Ok(())
                }
            };
        let report = process_queue(
            &mut fs,
            QueueOptions::default(),
            &AbortFlag::new(),
            &mut calculate,
        )
        .unwrap();

        assert_eq!(report.dequeued, 3);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].to_string(), "HT-1: boom");
        assert!(!report.stopped_early);
        assert!(!fs.object(&ids[1]).unwrap().calculated);
        assert_eq!(
            fs.object(&ids[1]).unwrap().error_message.as_deref(),
            Some("boom")
        );
        assert!(fs.object(&ids[0]).unwrap().calculated);
        assert!(fs.object(&ids[2]).unwrap().calculated);
    }

    /// **Methodology.** The same failure with `break_on_exception = true` must
    /// stop the drain and leave the failing entry at the head of the queue —
    /// upstream's `Exit While` skips the dequeue (:599).
    /// **Result (2026-08-11):** `stopped_early = true`; `dequeued = 1` (only the
    /// first, successful object); the queue still holds 2 entries with `HT-1` at
    /// its head; `C` was never calculated.
    #[test]
    fn break_on_exception_leaves_the_failure_at_the_head() {
        let (mut fs, ids) = rig();
        enqueue_solving_order(&mut fs, &ids);

        let mut calculate =
            |_fs: &mut Flowsheet, args: &CalculationArgs, _o: QueueOptions| -> Result<(), SolverError> {
                if args.tag == "HT-1" {
                    Err(SolverError::Other("boom".to_string()))
                } else {
                    Ok(())
                }
            };
        let options = QueueOptions {
            break_on_exception: true,
            ..QueueOptions::default()
        };
        let report =
            process_queue(&mut fs, options, &AbortFlag::new(), &mut calculate).unwrap();

        assert!(report.stopped_early);
        assert_eq!(report.dequeued, 1);
        assert_eq!(fs.calculation_queue.len(), 2);
        assert_eq!(fs.calculation_queue.peek().unwrap().tag, "HT-1");
        assert!(!fs.object(&ids[2]).unwrap().calculated);
    }

    /// **Methodology.** Three checks of the skip rules: an **inactive** object is
    /// dequeued but not calculated (:537); `flowsheet_solver_mode` filters out a
    /// non-solver sender (:538-545); a **dangling** queue entry naming an object
    /// that is not in the flowsheet is dequeued and ignored (:531).
    /// **Result (2026-08-11):** with the heater inactive, `dequeued = 3` and
    /// `calculated = 2`; with `flowsheet_solver_mode = true` and a
    /// `PropertyGrid` entry, that entry is dequeued but not calculated; a
    /// dangling entry is dequeued with no error.
    #[test]
    fn skip_rules_dequeue_without_calculating() {
        // Inactive object.
        let (mut fs, ids) = rig();
        fs.object_mut(&ids[1]).unwrap().active = false;
        enqueue_solving_order(&mut fs, &ids);
        let mut calculate =
            |_fs: &mut Flowsheet, _a: &CalculationArgs, _o: QueueOptions| -> Result<(), SolverError> {
                Ok(())
            };
        let report = process_queue(
            &mut fs,
            QueueOptions::default(),
            &AbortFlag::new(),
            &mut calculate,
        )
        .unwrap();
        assert_eq!(report.dequeued, 3);
        assert_eq!(report.calculated, 2);

        // Sender filter.
        let (mut fs, ids) = rig();
        let obj = fs.object(&ids[0]).unwrap().clone();
        fs.calculation_queue
            .request_calculation(&obj, CalculationSender::PropertyGrid);
        let options = QueueOptions {
            flowsheet_solver_mode: true,
            ..QueueOptions::default()
        };
        let report =
            process_queue(&mut fs, options, &AbortFlag::new(), &mut calculate).unwrap();
        assert_eq!(report.dequeued, 1);
        assert_eq!(report.calculated, 0);

        // Dangling entry.
        let (mut fs, ids) = rig();
        let obj = fs.object(&ids[0]).unwrap().clone();
        fs.calculation_queue
            .request_calculation(&obj, CalculationSender::FlowsheetSolver);
        fs.remove_object(&ids[0]).unwrap();
        let report = process_queue(
            &mut fs,
            QueueOptions::default(),
            &AbortFlag::new(),
            &mut calculate,
        )
        .unwrap();
        assert_eq!(report.dequeued, 1);
        assert_eq!(report.calculated, 0);
        assert!(report.errors.is_empty());
    }

    /// **Methodology.** Raising the abort flag mid-drain must return
    /// [`SolverError::Aborted`] and leave the remaining entries queued.
    /// **Result (2026-08-11):** the closure ran once, then `Err(Aborted)` came
    /// back with 2 entries still queued.
    #[test]
    fn abort_stops_the_drain() {
        let (mut fs, ids) = rig();
        enqueue_solving_order(&mut fs, &ids);
        let abort = AbortFlag::new();
        let abort_for_closure = abort.clone();
        let mut runs = 0usize;
        let mut calculate = |_fs: &mut Flowsheet,
                             _a: &CalculationArgs,
                             _o: QueueOptions|
         -> Result<(), SolverError> {
            runs += 1;
            abort_for_closure.request_abort();
            Ok(())
        };
        let err = process_queue(
            &mut fs,
            QueueOptions::default(),
            &abort,
            &mut calculate,
        )
        .unwrap_err();
        assert_eq!(err, SolverError::Aborted);
        assert_eq!(runs, 1);
        assert_eq!(fs.calculation_queue.len(), 3);
    }
}
