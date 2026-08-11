//! Calculation ordering — which flowsheet objects are solved, and in what order.
//!
//! # What this module computes
//!
//! A **solving list**: the sequence in which a sequential-modular solver must
//! visit the objects of a flowsheet so that every object is calculated after the
//! objects feeding it. Dimensionless — this is graph bookkeeping, not physics.
//!
//! DWSIM does not run a textbook topological sort. It does a **breadth-first
//! level walk**, and it does it *backwards*: it seeds the walk with the
//! flowsheet's endpoints (product streams with nothing downstream, every recycle
//! block, and every "source" unit operation), then repeatedly steps one edge
//! upstream, recording a new level each time. Reading the levels back from the
//! deepest to the shallowest, keeping only the first appearance of each object,
//! yields an order in which feeds come first and products last. That is
//! [`solving_list`], and this port reproduces it edge for edge — including its
//! duplicate-elimination rule, its two 10 000-step guards, and its dynamic-mode
//! addendum.
//!
//! **Cycles are broken by recycle blocks, not detected.** Every edge *leaving* a
//! [`ObjectType::OtRecycle`] or [`ObjectType::OtEnergyRecycle`] is ignored by
//! the walk (FlowsheetSolver.vb:1052-1053), so a loop containing a recycle block
//! becomes a tree and the recycle's outlet stream is the **tear stream**. A loop
//! *without* a recycle block never terminates the walk, and the 10 000-step
//! guard turns it into [`SolverError::InfiniteOrderingLoop`] with upstream's
//! own advice — "insert recycle blocks where needed". This is the entire cycle
//! story in DWSIM: the user chooses the tear, the solver does not.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary source: `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:923-1109`
//! (`GetSolvingList`). `DWSIM.FlowsheetSolver/FlowsheetSolver2.vb:365-550` holds
//! a **byte-for-byte duplicate** of the same routine except that it drops the
//! two `GlobalSettings.Settings.CalculatorBusy = False` side effects
//! (FlowsheetSolver.vb:984, absent at FlowsheetSolver2.vb:425); since
//! `CalculatorBusy` is excluded from this port anyway, one implementation covers
//! both.
//!
//! # Excluded DWSIM behavior
//!
//! - **`GlobalSettings.Settings.CalculatorBusy`** (FlowsheetSolver.vb:984). An
//!   ambient process-wide mutex flag; this port takes `&mut Flowsheet` instead,
//!   so exclusive access is enforced by the borrow checker.
//! - **`GraphicObject` geometry** — the walk reads only `Active`, `ObjectType`,
//!   and the connector attachments, all of which the flowsheet data model
//!   carries. Nothing screen-related is consulted.
//! - **`ISimulationObject.SupportsDynamicMode`** (FlowsheetSolver.vb:1093). A
//!   per-model boolean override that the flowsheet data model does not carry;
//!   see [`solving_list`]'s "Known divergence" note.
//!
//! # Honest scope
//!
//! AI-assisted draft with **no human V&V**. The tests below are *verification*
//! against the transcribed upstream logic and hand-traced orderings — they check
//! "did we port it correctly?", not "does it represent physical reality?".

use crate::flowsheet::{Flowsheet, ObjectId, ObjectType};
use crate::flowsheet_solver::errors::SolverError;

/// The guard DWSIM uses on both walks before declaring an unbroken cycle
/// (FlowsheetSolver.vb:982, :1056).
///
/// Dimensionless count. On the backward walk it counts *edges followed*; on the
/// forward walk it counts *levels created*. Both are upstream's exact meanings.
pub const MAX_ORDERING_STEPS: usize = 10_000;

/// The result of [`solving_list`] — DWSIM's `Object() {objstack, lists,
/// filteredlist}` (FlowsheetSolver.vb:1107), with the three anonymous array
/// slots given names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SolvingList {
    /// The ordered list of objects to calculate, **feeds first, products last**
    /// — upstream's `objstack`. This is what the solver enqueues.
    pub stack: Vec<ObjectId>,
    /// Every level of the breadth-first walk, in walk order, **including
    /// duplicates** — upstream's `lists`. Level 0 is the seed level. The last
    /// level is always empty (it is what terminates the walk).
    pub levels: Vec<Vec<ObjectId>>,
    /// The levels re-indexed into calculation order and stripped of objects
    /// already claimed by a deeper level — upstream's `filteredlist`. Objects
    /// within one entry have no dependency on each other, which is what
    /// upstream's parallel mode (`mode = 2`) parallelises over
    /// (FlowsheetSolver.vb:763-768).
    pub filtered_levels: Vec<Vec<ObjectId>>,
}

/// Whether an object type is a DWSIM "source" — `ISimulationObject.IsSource`.
///
/// Upstream defaults this `False` for every object
/// (SimulationObjectBaseClasses.vb:1669) and overrides it `True` only on the
/// clean-energy unit operations: `CleanEnergyUnitOpBase` sets it for the wind
/// turbine, solar panel, hydroelectric turbine and water electrolyzer
/// (CleanEnergyUnitOpBase.vb:14), and `PEMFuelCellUnitOpBase` overrides it
/// `True` as well (PEMFuelCellUnitOpBase.vb:86-90).
///
/// A source is seeded into level 0 of the backward walk
/// (FlowsheetSolver.vb:1030-1032) so that it is calculated last, like a product
/// endpoint — these blocks generate their own inlet conditions rather than
/// receiving them.
#[must_use]
pub fn is_source(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::WindTurbine
            | ObjectType::SolarPanel
            | ObjectType::HydroelectricTurbine
            | ObjectType::WaterElectrolyzer
            | ObjectType::PemFuelCell
    )
}

/// Whether the walk must refuse to follow edges *out of* this object type.
///
/// True for the two recycle blocks (FlowsheetSolver.vb:970 forward,
/// :1052-1053 backward). This single predicate is what turns a cyclic flowsheet
/// into an acyclic one: the recycle's outlet is the tear.
#[must_use]
pub fn breaks_the_loop(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::OtRecycle | ObjectType::OtEnergyRecycle
    )
}

/// Compute the calculation order for `flowsheet`.
///
/// # Arguments
///
/// - `flowsheet` — taken by `&mut` because the `from_property_grid` branch
///   **consumes** the head of [`Flowsheet::calculation_queue`] and then clears
///   it, exactly as upstream does (FlowsheetSolver.vb:951-952).
/// - `from_property_grid` — upstream's `frompgrid`. `false` runs the **backward
///   walk** over the whole flowsheet (the normal "solve everything" path).
///   `true` runs the **forward walk** starting from whatever single object is at
///   the head of the calculation queue — the incremental path DWSIM uses when a
///   user edits one property and only the downstream consequences need
///   recalculating.
///
/// # Order guarantee
///
/// Deterministic. Both walks iterate the flowsheet registry and each object's
/// connector slots in insertion order, and this port's registry preserves
/// insertion order, so repeated calls on an unchanged flowsheet return an
/// identical [`SolvingList`].
///
/// # Known divergence — `SupportsDynamicMode`
///
/// In dynamic mode DWSIM appends every *fully disconnected* unit operation that
/// declares `SupportsDynamicMode` (FlowsheetSolver.vb:1091-1103) so batch
/// equipment still gets stepped. `SupportsDynamicMode` is a per-model override
/// on the equipment class, which the flowsheet data model does not carry, so
/// this port applies the rule to **every** disconnected unit operation
/// ([`ObjectType::is_unit_operation`]). That is a superset of upstream's
/// behaviour: it can append an object DWSIM would skip, never omit one DWSIM
/// would append.
///
/// # Errors
///
/// - [`SolverError::InfiniteOrderingLoop`] if either walk exceeds
///   [`MAX_ORDERING_STEPS`], which upstream reads as a cycle with no recycle
///   block in it.
/// - [`SolverError::UnknownObject`] if the forward walk reaches an object id the
///   registry does not contain (upstream throws `KeyNotFoundException` here).
pub fn solving_list(
    flowsheet: &mut Flowsheet,
    from_property_grid: bool,
) -> Result<SolvingList, SolverError> {
    if from_property_grid {
        forward_walk(flowsheet)
    } else {
        backward_walk(flowsheet)
    }
}

/// The `frompgrid = True` branch (FlowsheetSolver.vb:947-1009): walk *forward*
/// from the single object at the head of the calculation queue.
fn forward_walk(flowsheet: &mut Flowsheet) -> Result<SolvingList, SolverError> {
    let Some(on_queue) = flowsheet.calculation_queue.dequeue() else {
        // Upstream leaves every output empty when the queue is empty
        // (the `If fqueue.CalculationQueue.Count > 0` guard, :949).
        return Ok(SolvingList::default());
    };
    flowsheet.calculation_queue.clear();

    let mut levels: Vec<Vec<ObjectId>> = vec![vec![ObjectId(on_queue.name)]];
    let mut max_idx = 0usize;
    let mut list_idx = 0usize;

    loop {
        list_idx += 1;
        if levels[list_idx - 1].is_empty() {
            break;
        }
        levels.push(Vec::new());
        max_idx = list_idx;

        // Snapshot the previous level: upstream reads it while appending to the
        // new one, which cannot alias in VB either.
        let previous = levels[list_idx - 1].clone();
        for id in previous {
            let obj = flowsheet
                .object(&id)
                .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
            if !obj.active {
                continue;
            }
            let object_type = obj.object_type;
            let mut next: Vec<ObjectId> = Vec::new();
            for slot in &obj.outputs {
                if let Some(att) = &slot.attachment {
                    // Upstream tests the *current* object's type inside the
                    // connector loop and `Exit For`s (:970), so a recycle block
                    // contributes nothing downstream at all.
                    if breaks_the_loop(object_type) {
                        next.clear();
                        break;
                    }
                    next.push(att.peer.clone());
                }
            }
            // The dedicated energy connector, skipping a self-loop (:974-976).
            if let Some(att) = &obj.energy_connector.attachment {
                if att.peer != obj.id {
                    next.push(att.peer.clone());
                }
            }
            levels[list_idx].extend(next);
        }

        if levels.len() > MAX_ORDERING_STEPS {
            return Err(SolverError::InfiniteOrderingLoop);
        }
    }

    // Concatenate levels 0..=max_idx in walk order (:991-1003).
    let mut filtered_levels: Vec<Vec<ObjectId>> = Vec::with_capacity(max_idx + 1);
    let mut stack: Vec<ObjectId> = Vec::new();
    for level in levels.iter().take(max_idx + 1) {
        filtered_levels.push(level.clone());
        stack.extend(level.iter().cloned());
    }

    // `objstack.Reverse(); Distinct(); Reverse()` (:1005-1007) keeps the LAST
    // occurrence of each object, preserving the original relative order — so an
    // object fed from two branches is calculated after both.
    stack = keep_last_occurrence(stack);

    Ok(SolvingList {
        stack,
        levels,
        filtered_levels,
    })
}

/// The `frompgrid = False` branch (FlowsheetSolver.vb:1011-1104): seed with the
/// flowsheet's endpoints and walk *backward* to its feeds.
fn backward_walk(flowsheet: &Flowsheet) -> Result<SolvingList, SolverError> {
    // Level 0 — endpoints, recycles, and sources (:1015-1033).
    let mut seeds: Vec<ObjectId> = Vec::new();
    for obj in flowsheet.iter() {
        match obj.object_type {
            ObjectType::MaterialStream | ObjectType::EnergyStream => {
                // Upstream indexes `OutputConnectors(0)` directly; a stream
                // whose outlet slot is missing is treated as unattached.
                let attached = obj.outputs.first().is_some_and(|c| c.is_attached());
                if !attached {
                    seeds.push(obj.id.clone());
                }
            }
            ObjectType::OtRecycle | ObjectType::OtEnergyRecycle => seeds.push(obj.id.clone()),
            other if is_source(other) => seeds.push(obj.id.clone()),
            _ => {}
        }
    }

    let mut levels: Vec<Vec<ObjectId>> = vec![seeds];
    let mut max_idx = 0usize;
    let mut list_idx = 0usize;
    let mut total_objects = 0usize;

    loop {
        list_idx += 1;
        if levels[list_idx - 1].is_empty() {
            break;
        }
        levels.push(Vec::new());
        max_idx = list_idx;

        let previous = levels[list_idx - 1].clone();
        for id in previous {
            let Some(obj) = flowsheet.object(&id) else {
                // Upstream's `If fbag.SimulationObjects.ContainsKey(o)` guard
                // (:1045) silently skips a dangling id.
                continue;
            };
            let mut upstreams: Vec<ObjectId> = Vec::new();
            for slot in &obj.inputs {
                let Some(att) = &slot.attachment else { continue };
                let from_type = flowsheet
                    .object(&att.peer)
                    .map_or(ObjectType::Undefined, |o| o.object_type);
                if breaks_the_loop(from_type) {
                    // The tear: do not step past a recycle block (:1052-1053).
                    continue;
                }
                upstreams.push(att.peer.clone());
                total_objects += 1;
                if total_objects > MAX_ORDERING_STEPS {
                    // Upstream throws right here, mid-walk (:1056-1058).
                    return Err(SolverError::InfiniteOrderingLoop);
                }
            }
            levels[list_idx].extend(upstreams);
        }
    }

    // Read the levels back from deepest to shallowest, first appearance wins
    // (:1071-1087). This is what puts feeds first.
    let mut stack: Vec<ObjectId> = Vec::new();
    let mut filtered_by_walk_index: Vec<Vec<ObjectId>> = vec![Vec::new(); max_idx + 1];
    for idx in (0..=max_idx).rev() {
        let level = levels[idx].clone();
        let mut filtered = level.clone();
        for o in level {
            if !stack.contains(&o) {
                stack.push(o);
            } else if let Some(pos) = filtered.iter().position(|x| *x == o) {
                filtered.remove(pos);
            }
        }
        filtered_by_walk_index[max_idx - idx] = filtered;
    }

    // Dynamic mode: append fully disconnected batch unit operations
    // (:1091-1103). See "Known divergence" on `solving_list`.
    if flowsheet.dynamic_mode {
        for obj in flowsheet.iter() {
            if !obj.object_type.is_unit_operation() {
                continue;
            }
            let inlets_attached = obj.inputs.iter().filter(|c| c.is_attached()).count();
            let outlets_attached = obj.outputs.iter().filter(|c| c.is_attached()).count();
            let slots = obj.inputs.len() + obj.outputs.len();
            if inlets_attached == 0 && outlets_attached == 0 && slots > 0 {
                stack.push(obj.id.clone());
            }
        }
    }

    Ok(SolvingList {
        stack,
        levels,
        filtered_levels: filtered_by_walk_index,
    })
}

/// Reverse, deduplicate keeping the first survivor, reverse again — LINQ's
/// `Reverse().Distinct().Reverse()` (FlowsheetSolver.vb:1005-1007).
///
/// Net effect: keep the **last** occurrence of each element, preserving the
/// original relative order of the survivors.
fn keep_last_occurrence(items: Vec<ObjectId>) -> Vec<ObjectId> {
    let mut seen: Vec<ObjectId> = Vec::with_capacity(items.len());
    for item in items.into_iter().rev() {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen.reverse();
    seen
}

/// [`solving_list`] preceded by an explicit cycle check.
///
/// [`solving_list`] reproduces upstream faithfully, which means a flowsheet with
/// an unbroken cycle is detected only after following
/// [`MAX_ORDERING_STEPS`] = 10 000 edges. This wrapper runs
/// [`has_unbroken_cycle`] first, so the same [`SolverError::InfiniteOrderingLoop`]
/// comes back in linear time. Prefer it when the caller controls the flowsheet;
/// use [`solving_list`] when byte-for-byte upstream behaviour matters.
///
/// # Errors
///
/// [`SolverError::InfiniteOrderingLoop`] when the flowsheet contains a directed
/// cycle with no recycle block on it. Otherwise identical to [`solving_list`].
pub fn solving_list_guarded(
    flowsheet: &mut Flowsheet,
    from_property_grid: bool,
) -> Result<SolvingList, SolverError> {
    if !from_property_grid && has_unbroken_cycle(flowsheet) {
        return Err(SolverError::InfiniteOrderingLoop);
    }
    solving_list(flowsheet, from_property_grid)
}

/// Whether the flowsheet contains a directed cycle that no recycle block breaks.
///
/// Not an upstream routine — upstream discovers this only by exhausting
/// [`MAX_ORDERING_STEPS`]. This is an explicit depth-first check over the same
/// edge set the walk uses (stream/unit-op edges, with edges *out of* a recycle
/// block removed), so it reports exactly the flowsheets on which upstream's
/// guard would eventually fire, in linear time instead of 10 000 steps.
///
/// Dimensionless. Deterministic: nodes and edges are visited in registry
/// insertion order.
#[must_use]
pub fn has_unbroken_cycle(flowsheet: &Flowsheet) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unvisited,
        InProgress,
        Done,
    }

    let ids: Vec<ObjectId> = flowsheet.object_ids().to_vec();
    let mut marks: Vec<Mark> = vec![Mark::Unvisited; ids.len()];

    fn successors(flowsheet: &Flowsheet, id: &ObjectId) -> Vec<ObjectId> {
        let Some(obj) = flowsheet.object(id) else {
            return Vec::new();
        };
        if breaks_the_loop(obj.object_type) {
            return Vec::new();
        }
        let mut out: Vec<ObjectId> = obj
            .outputs
            .iter()
            .filter_map(|c| c.attachment.as_ref().map(|a| a.peer.clone()))
            .collect();
        if let Some(att) = &obj.energy_connector.attachment {
            if att.peer != obj.id {
                out.push(att.peer.clone());
            }
        }
        out
    }

    // Iterative DFS so a deep flowsheet cannot overflow the stack.
    for start in 0..ids.len() {
        if marks[start] != Mark::Unvisited {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        marks[start] = Mark::InProgress;
        while let Some((node, edge)) = stack.pop() {
            let succ = successors(flowsheet, &ids[node]);
            if edge >= succ.len() {
                marks[node] = Mark::Done;
                continue;
            }
            stack.push((node, edge + 1));
            let Some(next) = ids.iter().position(|x| *x == succ[edge]) else {
                continue;
            };
            match marks[next] {
                Mark::InProgress => return true,
                Mark::Done => {}
                Mark::Unvisited => {
                    marks[next] = Mark::InProgress;
                    stack.push((next, 0));
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    //! # Verification — calculation ordering
    //!
    //! **Methodology.** Build small flowsheets through the public
    //! [`crate::flowsheet`] API, run [`solving_list`], and compare the returned
    //! order against the order hand-traced from
    //! `FlowsheetSolver.vb:930-1109`. The pass criterion is exact sequence
    //! equality on tags (not a set comparison — order is the whole point).
    //! No thermodynamics is evaluated; these are verification tests against the
    //! transcribed upstream algorithm, not validation against physical reality.
    //!
    //! **Results (2026-08-11, release build, `cargo test --release`):** all five
    //! tests pass. Observed orders are recorded in each test's doc comment.

    use super::*;
    use crate::flowsheet::{CalculationSender, ObjectType};

    fn tags(flowsheet: &Flowsheet, ids: &[ObjectId]) -> Vec<String> {
        ids.iter()
            .map(|i| flowsheet.object(i).unwrap().tag.clone())
            .collect()
    }

    /// **Methodology.** Feed -> mixer -> heater -> product, a purely acyclic
    /// chain, solved with `from_property_grid = false`. Upstream seeds level 0
    /// with the only endpoint (PROD, whose outlet is unattached) and steps
    /// upstream one edge at a time, so the deepest level holds FEED. Reading
    /// back deepest-first must give FEED, MIX-1, HT-1, PROD.
    /// **Result (2026-08-11, measured):** order
    /// `["FEED", "MIX-1", "S1", "HT-1", "PROD"]`; **6** levels, the last of
    /// which is empty as upstream requires; every object appears exactly once.
    #[test]
    fn acyclic_chain_orders_feed_first_and_product_last() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let s1 = fs.add_object(ObjectType::MaterialStream, Some("S1"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));

        fs.connect(&feed, &mixer, None, None).unwrap();
        fs.connect(&mixer, &s1, None, None).unwrap();
        fs.connect(&s1, &heater, None, None).unwrap();
        fs.connect(&heater, &product, None, None).unwrap();

        let list = solving_list(&mut fs, false).unwrap();
        assert_eq!(
            tags(&fs, &list.stack),
            vec!["FEED", "MIX-1", "S1", "HT-1", "PROD"]
        );
        assert!(
            list.levels.last().unwrap().is_empty(),
            "the walk terminates on an empty level"
        );
        // Every object appears exactly once.
        let mut sorted = list.stack.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), list.stack.len());
    }

    /// **Methodology.** A recycle loop: FEED -> MIX -> S1 -> HT -> S2 -> RY-1
    /// -> RECY (the tear stream) -> back into MIX. `has_unbroken_cycle` must
    /// report `false` (the recycle breaks it), the walk must terminate, RY-1
    /// must be seeded into level 0 as an endpoint, and the tear stream RECY must
    /// be ordered *before* the mixer that consumes it — because the backward
    /// walk refuses to step from MIX back through RECY into RY-1
    /// (FlowsheetSolver.vb:1052-1053), leaving RECY unreachable from the walk
    /// and therefore placed by the deepest level that does reach it.
    /// **Result (2026-08-11, measured):** no error, no infinite loop; order
    /// `["FEED", "RECY", "MIX-1", "S1", "HT-1", "S2", "RY-1"]`;
    /// `has_unbroken_cycle` = `false`; RY-1 present in level 0. The tear stream
    /// RECY lands at index 1, ahead of the mixer that consumes it, exactly as
    /// the tear rule predicts.
    #[test]
    fn recycle_loop_selects_the_tear_and_terminates() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let s1 = fs.add_object(ObjectType::MaterialStream, Some("S1"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let s2 = fs.add_object(ObjectType::MaterialStream, Some("S2"));
        let recycle = fs.add_object(ObjectType::OtRecycle, Some("RY-1"));
        let tear = fs.add_object(ObjectType::MaterialStream, Some("RECY"));

        fs.connect(&feed, &mixer, None, None).unwrap();
        fs.connect(&mixer, &s1, None, None).unwrap();
        fs.connect(&s1, &heater, None, None).unwrap();
        fs.connect(&heater, &s2, None, None).unwrap();
        fs.connect(&s2, &recycle, None, None).unwrap();
        fs.connect(&recycle, &tear, None, None).unwrap();
        fs.connect(&tear, &mixer, None, Some(1)).unwrap();

        assert!(
            !has_unbroken_cycle(&fs),
            "the recycle block must break the loop"
        );

        let list = solving_list(&mut fs, false).unwrap();
        let order = tags(&fs, &list.stack);
        assert!(order.contains(&"RY-1".to_string()));
        assert!(
            tags(&fs, &list.levels[0]).contains(&"RY-1".to_string()),
            "a recycle block is seeded into level 0"
        );
        let pos = |t: &str| order.iter().position(|x| x == t).unwrap();
        assert!(pos("FEED") < pos("MIX-1"), "{order:?}");
        assert!(pos("MIX-1") < pos("S1"), "{order:?}");
        assert!(pos("S1") < pos("HT-1"), "{order:?}");
        assert!(pos("HT-1") < pos("S2"), "{order:?}");
        assert!(pos("S2") < pos("RY-1"), "{order:?}");
    }

    /// **Methodology.** The same loop with the recycle block **removed** — a
    /// genuine unbroken cycle. `has_unbroken_cycle` must report it, and
    /// [`solving_list_guarded`] must return upstream's
    /// [`SolverError::InfiniteOrderingLoop`] rather than spinning.
    /// **Result (2026-08-11):** `has_unbroken_cycle` = `true`;
    /// `solving_list_guarded` returns `Err(InfiniteOrderingLoop)` whose message
    /// is upstream's verbatim "Infinite loop detected ... insert recycle blocks
    /// where needed."
    #[test]
    fn cycle_without_a_recycle_block_is_reported() {
        let mut fs = Flowsheet::new();
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let s1 = fs.add_object(ObjectType::MaterialStream, Some("S1"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let s2 = fs.add_object(ObjectType::MaterialStream, Some("S2"));

        fs.connect(&mixer, &s1, None, None).unwrap();
        fs.connect(&s1, &heater, None, None).unwrap();
        fs.connect(&heater, &s2, None, None).unwrap();
        fs.connect(&s2, &mixer, None, Some(1)).unwrap();

        assert!(has_unbroken_cycle(&fs));
        let err = solving_list_guarded(&mut fs, false).unwrap_err();
        assert_eq!(err, SolverError::InfiniteOrderingLoop);
        assert!(err.to_string().contains("insert recycle blocks"));
    }

    /// **Methodology.** The forward (`from_property_grid = true`) branch: queue
    /// a calculation request for the heater of the acyclic chain, then order.
    /// Upstream starts at the queued object and walks *downstream only*, so the
    /// result must be exactly HT-1 then PROD — the feed and mixer upstream of it
    /// are untouched.
    /// **Result (2026-08-11):** `["HT-1", "PROD"]`; the calculation queue is
    /// empty afterwards (upstream dequeues the head and clears the rest).
    #[test]
    fn property_grid_walk_goes_downstream_only() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        fs.connect(&feed, &heater, None, None).unwrap();
        fs.connect(&heater, &product, None, None).unwrap();

        let obj = fs.object(&heater).unwrap().clone();
        fs.calculation_queue
            .request_calculation(&obj, CalculationSender::PropertyGrid);

        let list = solving_list(&mut fs, true).unwrap();
        assert_eq!(tags(&fs, &list.stack), vec!["HT-1", "PROD"]);
        assert!(fs.calculation_queue.is_empty());
    }

    /// **Methodology.** Two feeds joining one mixer: FEED-A and FEED-B both
    /// enter MIX-1, which produces PROD. The backward walk reaches MIX-1 once
    /// and then both feeds at the same level, so both must precede the mixer and
    /// the mixer must precede the product. Also checks the source rule: a
    /// disconnected [`ObjectType::WindTurbine`] is seeded into level 0 by
    /// `IsSource` and therefore appears in the order.
    /// **Result (2026-08-11, measured):** order
    /// `["FEED-A", "FEED-B", "MIX-1", "PROD", "WIND-1"]` — both feeds before the
    /// mixer, WIND-1 present via the `IsSource` seed.
    #[test]
    fn two_feeds_and_a_source_block() {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::MaterialStream, Some("FEED-A"));
        let b = fs.add_object(ObjectType::MaterialStream, Some("FEED-B"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        let wind = fs.add_object(ObjectType::WindTurbine, Some("WIND-1"));

        fs.connect(&a, &mixer, None, Some(0)).unwrap();
        fs.connect(&b, &mixer, None, Some(1)).unwrap();
        fs.connect(&mixer, &product, None, None).unwrap();

        assert!(is_source(ObjectType::WindTurbine));
        assert!(!is_source(ObjectType::Heater));

        let list = solving_list(&mut fs, false).unwrap();
        let order = tags(&fs, &list.stack);
        let pos = |t: &str| order.iter().position(|x| x == t).unwrap();
        assert!(pos("FEED-A") < pos("MIX-1"), "{order:?}");
        assert!(pos("FEED-B") < pos("MIX-1"), "{order:?}");
        assert!(pos("MIX-1") < pos("PROD"), "{order:?}");
        assert!(order.contains(&"WIND-1".to_string()), "{order:?}");
        assert_eq!(fs.object(&wind).unwrap().object_type, ObjectType::WindTurbine);
    }

    /// **Methodology.** `keep_last_occurrence` must reproduce LINQ's
    /// `Reverse().Distinct().Reverse()` (FlowsheetSolver.vb:1005-1007) — keep
    /// the last occurrence, preserve relative order.
    /// **Result (2026-08-11):** `[a, b, a, c, b] -> [a, c, b]`.
    #[test]
    fn distinct_keeps_the_last_occurrence() {
        let id = |s: &str| ObjectId(s.to_string());
        let got = keep_last_occurrence(vec![id("a"), id("b"), id("a"), id("c"), id("b")]);
        assert_eq!(got, vec![id("a"), id("c"), id("b")]);
    }
}
