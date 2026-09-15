//! Specification blocks — *when* a spec is solved, not *how*.
//!
//! # What a spec block is
//!
//! A **spec** (DWSIM's `ISpec`, the flowsheet's `OT_Spec` object) copies a value
//! from a source object onto a target object, optionally through a correlation.
//! Unlike an adjust it does not iterate; it fires once, at a chosen point in the
//! calculation pass.
//!
//! # What this module ports
//!
//! **The scheduling only.** DWSIM sprinkles spec-solving calls through the
//! solver at seven distinct moments, selected by two enums — the flowsheet-wide
//! [`SpecCalcMode`] and the per-block [`SpecCalcMode2`] that can override it.
//! Getting those seven moments right is a solver concern, and it is what
//! [`specs_firing_at`] and [`SpecBlock`] encode.
//!
//! **The spec's own arithmetic is not ported here.** `Spec.Calculate`
//! (`DWSIM.UnitOperations/LogicalBlocks/Spec.vb`) is a logical-block *model*,
//! not solver machinery, and it depends on the same property-reflection layer
//! this port replaces. When the solver reaches a spec's firing point it
//! dispatches the spec object through the ordinary evaluation hook
//! ([`crate::flowsheet_solver::evaluator::UnitOpEvaluator`]) with the sender
//! `"Spec"`, exactly as an ordinary unit operation would be dispatched. Supply a
//! model through the hook if you need one; the built-in evaluator reports
//! [`crate::flowsheet_solver::errors::SolverError::NoModel`].
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
//! - `DWSIM.Interfaces/Enums.vb:360-364` (`SpecVarType`), `:605-612`
//!   (`SpecCalcMode`), `:614-624` (`SpecCalcMode2`).
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb` — the firing points:
//!   `:83-94` and `:109-120` (before/after a unit operation reached through a
//!   material stream), `:144-148` and `:164-168` (the energy-stream path),
//!   `:186-190` and `:206-210` (the general unit-operation path), `:365-376` and
//!   `:384-395` (`CalculateMaterialStream`), `:1383-1390`
//!   (`SpecCalcMode.BeforeFlowsheet`) and `:1440-1447`
//!   (`SpecCalcMode.AfterFlowsheet`).
//!
//! # Excluded DWSIM behavior
//!
//! - **`Spec.Calculate`** and the whole `LogicalBlocks/Spec.vb` model — see
//!   above.
//! - **`FlowsheetOptions.SpecCalculationMode`** as a *stored flowsheet option*.
//!   It is passed in as [`SpecCalcMode`] instead of read off the flowsheet.

use crate::flowsheet::{Flowsheet, ObjectId, ObjectType};

/// Where a spec sits relative to the object it is attached to — DWSIM's
/// `SpecVarType` (Enums.vb:360-364).
///
/// A spec attached as a [`SpecVarType::Source`] reads from its object, so it
/// fires *after* that object is calculated. Attached as a
/// [`SpecVarType::Target`] it writes to its object, so it fires *before*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpecVarType {
    /// `Source = 0` — the spec reads this object.
    Source,
    /// `Target = 1` — the spec writes this object.
    Target,
    /// `None = 2` — not attached in either direction.
    #[default]
    None,
}

/// The flowsheet-wide default firing point — DWSIM's `SpecCalcMode`
/// (Enums.vb:605-612), stored on `FlowsheetOptions.SpecCalculationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpecCalcMode {
    /// `AfterSourceObject = 0` — fire right after the object the spec reads.
    /// Upstream's default value for the enum.
    #[default]
    AfterSourceObject,
    /// `BeforeTargetObject = 1` — fire right before the object the spec writes.
    BeforeTargetObject,
    /// `BeforeFlowsheet = 2` — fire once, before each pass over the calculation
    /// order (FlowsheetSolver.vb:1383-1390).
    BeforeFlowsheet,
    /// `AfterFlowsheet = 3` — fire once after the pass, then **re-run the whole
    /// pass** (FlowsheetSolver.vb:1440-1474). This doubles the cost of every
    /// outer iteration; see the real-time note in
    /// [`crate::flowsheet_solver::solver`].
    AfterFlowsheet,
}

/// A spec's own firing point, which may override the flowsheet-wide one —
/// DWSIM's `SpecCalcMode2` (Enums.vb:614-624).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpecCalcMode2 {
    /// `GlobalSetting = 0` — defer to the flowsheet's [`SpecCalcMode`].
    #[default]
    GlobalSetting,
    /// `AfterSourceObject = 1`.
    AfterSourceObject,
    /// `BeforeTargetObject = 2`.
    BeforeTargetObject,
    /// `BeforeFlowsheet = 3`.
    BeforeFlowsheet,
    /// `AfterFlowsheet = 4`.
    AfterFlowsheet,
    /// `AfterObject = 5` — fire after [`SpecBlock::reference_object`],
    /// whichever object that is (FlowsheetSolver.vb:115-120).
    AfterObject,
    /// `BeforeObject = 6` — fire before [`SpecBlock::reference_object`]
    /// (FlowsheetSolver.vb:89-94).
    BeforeObject,
}

/// The seven distinct moments at which the solver may fire a spec.
///
/// This is the union of [`SpecCalcMode`] and [`SpecCalcMode2`] resolved against
/// each other, so the solver can ask one question — "which specs fire *here*?" —
/// rather than re-deriving the precedence at every call site.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecFiringPoint {
    /// Once, before the whole calculation pass.
    BeforeFlowsheet,
    /// Once, after the whole calculation pass (and before it is repeated).
    AfterFlowsheet,
    /// Immediately before the given object is calculated, because the spec
    /// writes it.
    BeforeTargetObject(ObjectId),
    /// Immediately after the given object is calculated, because the spec reads
    /// it.
    AfterSourceObject(ObjectId),
    /// Immediately before the given object, by explicit
    /// [`SpecCalcMode2::BeforeObject`].
    BeforeObject(ObjectId),
    /// Immediately after the given object, by explicit
    /// [`SpecCalcMode2::AfterObject`].
    AfterObject(ObjectId),
}

/// A specification block's scheduling data.
///
/// # Where this state lives
///
/// Owned by [`crate::flowsheet_solver::solver::FlowsheetSolver`] and keyed by
/// the [`ObjectId`] of the [`ObjectType::OtSpec`] object, for the same reason
/// [`crate::flowsheet_solver::adjust::AdjustBlock`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecBlock {
    /// This block's own firing point (`ISpec.SpecCalculationMode`). Default
    /// [`SpecCalcMode2::GlobalSetting`] defers to the flowsheet.
    pub calculation_mode: SpecCalcMode2,
    /// The object this spec is scheduled relative to when the mode is
    /// [`SpecCalcMode2::BeforeObject`] or [`SpecCalcMode2::AfterObject`]
    /// (`ISpec.ReferenceObjectID`, FlowsheetSolver.vb:91).
    pub reference_object: Option<ObjectId>,
    /// The object this spec is *attached* to, together with whether it reads or
    /// writes it — the pair DWSIM stores as
    /// `ISimulationObject.AttachedSpecId` + `SpecVarType`
    /// (FlowsheetSolver.vb:83-87). Used by the
    /// [`SpecCalcMode::AfterSourceObject`] / [`SpecCalcMode::BeforeTargetObject`]
    /// global modes.
    pub attached_to: Option<(ObjectId, SpecVarType)>,
}

impl Default for SpecBlock {
    fn default() -> Self {
        SpecBlock {
            calculation_mode: SpecCalcMode2::GlobalSetting,
            reference_object: None,
            attached_to: None,
        }
    }
}

impl SpecBlock {
    /// Resolve this block's firing point against the flowsheet-wide default.
    ///
    /// Precedence, matching upstream's call sites exactly: a per-block mode
    /// other than [`SpecCalcMode2::GlobalSetting`] wins; otherwise the
    /// flowsheet's [`SpecCalcMode`] applies, and the object it applies to comes
    /// from [`SpecBlock::attached_to`].
    ///
    /// Returns `None` when the block names no object for a mode that needs one —
    /// upstream simply never matches such a spec.
    #[must_use]
    pub fn firing_point(&self, global: SpecCalcMode) -> Option<SpecFiringPoint> {
        let reference = self.reference_object.clone();
        let attached = self.attached_to.clone();
        match self.calculation_mode {
            SpecCalcMode2::BeforeFlowsheet => Some(SpecFiringPoint::BeforeFlowsheet),
            SpecCalcMode2::AfterFlowsheet => Some(SpecFiringPoint::AfterFlowsheet),
            SpecCalcMode2::BeforeObject => reference.map(SpecFiringPoint::BeforeObject),
            SpecCalcMode2::AfterObject => reference.map(SpecFiringPoint::AfterObject),
            SpecCalcMode2::BeforeTargetObject => attached
                .filter(|(_, v)| *v == SpecVarType::Target)
                .map(|(o, _)| SpecFiringPoint::BeforeTargetObject(o)),
            SpecCalcMode2::AfterSourceObject => attached
                .filter(|(_, v)| *v == SpecVarType::Source)
                .map(|(o, _)| SpecFiringPoint::AfterSourceObject(o)),
            SpecCalcMode2::GlobalSetting => match global {
                SpecCalcMode::BeforeFlowsheet => Some(SpecFiringPoint::BeforeFlowsheet),
                SpecCalcMode::AfterFlowsheet => Some(SpecFiringPoint::AfterFlowsheet),
                SpecCalcMode::BeforeTargetObject => attached
                    .filter(|(_, v)| *v == SpecVarType::Target)
                    .map(|(o, _)| SpecFiringPoint::BeforeTargetObject(o)),
                SpecCalcMode::AfterSourceObject => attached
                    .filter(|(_, v)| *v == SpecVarType::Source)
                    .map(|(o, _)| SpecFiringPoint::AfterSourceObject(o)),
            },
        }
    }
}

/// The spec objects that fire at `point`, in registry insertion order.
///
/// Ordering is deterministic and matches upstream's
/// `SimulationObjects.Values.Where(TypeOf o Is ISpec)` iteration over .NET's
/// insertion-ordered dictionary.
#[must_use]
pub fn specs_firing_at(
    flowsheet: &Flowsheet,
    specs: &std::collections::HashMap<ObjectId, SpecBlock>,
    global: SpecCalcMode,
    point: &SpecFiringPoint,
) -> Vec<ObjectId> {
    flowsheet
        .ids_of_type(ObjectType::OtSpec)
        .into_iter()
        .filter(|id| flowsheet.object(id).is_some_and(|o| o.active))
        .filter(|id| {
            specs
                .get(id)
                .and_then(|b| b.firing_point(global))
                .is_some_and(|p| p == *point)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! # Verification — spec scheduling
    //!
    //! **Methodology.** Build spec blocks in each mode and check
    //! [`SpecBlock::firing_point`] resolves to the moment the cited upstream
    //! call site fires them, then check [`specs_firing_at`] filters and orders
    //! correctly. Pass criterion: exact enum equality and exact id sequence.
    //! Verification only, no physics.
    //! **Results (2026-08-11, release build):** recorded per test.

    use super::*;
    use std::collections::HashMap;

    /// **Methodology.** Each per-block [`SpecCalcMode2`] must win over the
    /// flowsheet-wide default, and [`SpecCalcMode2::GlobalSetting`] must defer.
    /// **Result (2026-08-11):** `BeforeObject` and `AfterObject` resolve to
    /// their reference object; `GlobalSetting` under
    /// `SpecCalcMode::BeforeFlowsheet` resolves to `BeforeFlowsheet`; a
    /// `GlobalSetting` block attached as `Source` under
    /// `SpecCalcMode::AfterSourceObject` resolves to `AfterSourceObject(obj)`;
    /// the same block under `BeforeTargetObject` resolves to `None`, because it
    /// is not attached as a target.
    #[test]
    fn firing_point_precedence() {
        let obj = ObjectId("obj-7".to_string());

        let mut block = SpecBlock {
            calculation_mode: SpecCalcMode2::BeforeObject,
            reference_object: Some(obj.clone()),
            attached_to: None,
        };
        assert_eq!(
            block.firing_point(SpecCalcMode::AfterFlowsheet),
            Some(SpecFiringPoint::BeforeObject(obj.clone()))
        );

        block.calculation_mode = SpecCalcMode2::AfterObject;
        assert_eq!(
            block.firing_point(SpecCalcMode::AfterFlowsheet),
            Some(SpecFiringPoint::AfterObject(obj.clone()))
        );

        block.calculation_mode = SpecCalcMode2::GlobalSetting;
        assert_eq!(
            block.firing_point(SpecCalcMode::BeforeFlowsheet),
            Some(SpecFiringPoint::BeforeFlowsheet)
        );

        block.attached_to = Some((obj.clone(), SpecVarType::Source));
        assert_eq!(
            block.firing_point(SpecCalcMode::AfterSourceObject),
            Some(SpecFiringPoint::AfterSourceObject(obj.clone()))
        );
        assert_eq!(block.firing_point(SpecCalcMode::BeforeTargetObject), None);
    }

    /// **Methodology.** Two specs, one firing before the flowsheet and one
    /// after; [`specs_firing_at`] must return each at its own point, in registry
    /// order, and must skip an inactive spec.
    /// **Result (2026-08-11):** `BeforeFlowsheet` returns exactly the first
    /// spec, `AfterFlowsheet` exactly the second; deactivating the first makes
    /// `BeforeFlowsheet` return an empty list.
    #[test]
    fn specs_firing_at_filters_and_orders() {
        let mut fs = Flowsheet::new();
        let s1 = fs.add_object(ObjectType::OtSpec, Some("SP-1"));
        let s2 = fs.add_object(ObjectType::OtSpec, Some("SP-2"));

        let mut specs: HashMap<ObjectId, SpecBlock> = HashMap::new();
        specs.insert(
            s1.clone(),
            SpecBlock {
                calculation_mode: SpecCalcMode2::BeforeFlowsheet,
                ..SpecBlock::default()
            },
        );
        specs.insert(
            s2.clone(),
            SpecBlock {
                calculation_mode: SpecCalcMode2::AfterFlowsheet,
                ..SpecBlock::default()
            },
        );

        let global = SpecCalcMode::default();
        assert_eq!(
            specs_firing_at(&fs, &specs, global, &SpecFiringPoint::BeforeFlowsheet),
            vec![s1.clone()]
        );
        assert_eq!(
            specs_firing_at(&fs, &specs, global, &SpecFiringPoint::AfterFlowsheet),
            vec![s2.clone()]
        );

        fs.object_mut(&s1).unwrap().active = false;
        assert!(specs_firing_at(&fs, &specs, global, &SpecFiringPoint::BeforeFlowsheet).is_empty());
    }

    /// **Methodology.** The two enums' defaults must be upstream's zero values:
    /// `SpecCalcMode.AfterSourceObject = 0` (Enums.vb:607),
    /// `SpecCalcMode2.GlobalSetting = 0` (Enums.vb:616),
    /// `SpecVarType.None = 2` is *not* zero upstream, so this port defaults to
    /// `None` on the grounds that an unattached spec is the safe default.
    /// **Result (2026-08-11):** as stated.
    #[test]
    fn enum_defaults() {
        assert_eq!(SpecCalcMode::default(), SpecCalcMode::AfterSourceObject);
        assert_eq!(SpecCalcMode2::default(), SpecCalcMode2::GlobalSetting);
        assert_eq!(SpecVarType::default(), SpecVarType::None);
    }
}
