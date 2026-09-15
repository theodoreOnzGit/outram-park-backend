//! The historian — a bounded, time-keyed record of past flowsheet states.
//!
//! # Attribution
//!
//! Pure-Rust port of the historian behaviour in **DWSIM**
//! `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb`
//! (`Historian` field :30-ish and its use at `:398`, `:490-496`, `:638`,
//! `:245-263` `RestoreHistorianState`, `:845-850` clear) and the bound
//! `MaxHistorianItems` / `EnableHistorian` on
//! `DWSIM.DynamicsManager/Manager.vb:48-50`. Upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # What it is for
//!
//! Two things, both inside a dynamic run:
//!
//! 1. **Event ramps.** A non-step transition needs the value a property had at
//!    some earlier reference instant; the manager restores that historian entry
//!    onto a scratch flowsheet and reads the value off it
//!    (Manager.vb:233-285).
//! 2. **Stepping backwards.** The GUI's "step back" button restores the entry at
//!    `CurrentTime - 2·interval` (FormDynamicsIntegratorControl.vb:317-320,
//!    :245-263).
//!
//! # Divergence: typed snapshots, not compressed XML
//!
//! Upstream stores `Dictionary(Of Date, String)` where each value is
//! `Flowsheet.GetSnapshot(SnapshotType.ObjectData).ToString().Compress()` — a
//! gzip-compressed XML document (`:491-492`). This port stores a **typed clone**
//! of the [`Flowsheet`] instead ([`FlowsheetSnapshot`]). Consequences:
//!
//! - No XML, no compression, and no serialization layer — the crate excludes all
//!   three (see the [`crate::flowsheet`] module header).
//! - Restoring is a struct assignment, not a parse; it cannot fail, so
//!   upstream's `Try`/`Catch`-with-message-box (`:257-261`) has no equivalent.
//! - **Memory cost is higher and is not measured.** Upstream's UI reports the
//!   historian size in MB (`UpdateHistorianDisplaySize`, excluded); a typed
//!   snapshot is an uncompressed deep copy of the whole flowsheet data model, so
//!   `max_items` entries cost roughly `max_items ×` the size of one flowsheet.
//!   With upstream's default bound of 1000 entries (Manager.vb:50) that is a
//!   real cost — see [`Historian::insert_bounded`].
//! - Upstream snapshots only `SnapshotType.ObjectData`, i.e. object state
//!   without drawing geometry. This port has no geometry at all, so a full
//!   flowsheet clone *is* the object data.
//!
//! # Excluded DWSIM behavior
//!
//! - `String.Compress()` / `Decompress()` and `XDocument` parsing.
//! - `UpdateHistorianDisplaySize` and the historian list view.
//! - `SnapshotType` selection — there is only one kind of snapshot here.

use std::collections::BTreeMap;

use crate::dynamics::sim_time::SimInstant;
use crate::flowsheet::graph::Flowsheet;

/// A complete capture of a flowsheet's state at one instant.
///
/// Replaces upstream's compressed-XML string (see the module header). Cloning a
/// flowsheet is a **deep** copy of every object, connector and stream, so
/// capturing is O(flowsheet size) — the dominant per-step cost of enabling the
/// historian.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetSnapshot {
    flowsheet: Flowsheet,
}

impl FlowsheetSnapshot {
    /// Capture the flowsheet as it stands — upstream's
    /// `Flowsheet.GetSnapshot(SnapshotType.ObjectData)`
    /// (FormDynamicsIntegratorControl.vb:491).
    #[must_use]
    pub fn capture(flowsheet: &Flowsheet) -> Self {
        FlowsheetSnapshot {
            flowsheet: flowsheet.clone(),
        }
    }

    /// Overwrite `target` with this snapshot — upstream's
    /// `Flowsheet.RestoreSnapshot(state, SnapshotType.ObjectData)`
    /// (Manager.vb:283, FormDynamicsIntegratorControl.vb:253).
    ///
    /// Unlike upstream this cannot fail: there is nothing to parse.
    pub fn restore_into(&self, target: &mut Flowsheet) {
        target.clone_from(&self.flowsheet);
    }

    /// Borrow the captured flowsheet, for reading a value out of a past state
    /// without restoring it anywhere.
    #[must_use]
    pub fn flowsheet(&self) -> &Flowsheet {
        &self.flowsheet
    }
}

/// A bounded, time-ordered store of past flowsheet states — upstream's
/// `Historian` dictionary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Historian {
    entries: BTreeMap<SimInstant, FlowsheetSnapshot>,
}

impl Historian {
    /// An empty historian — upstream's
    /// `Historian = New Dictionary(Of Date, String)`
    /// (FormDynamicsIntegratorControl.vb:398).
    #[must_use]
    pub fn new() -> Self {
        Historian::default()
    }

    /// How many states are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the historian holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop every entry — upstream's `Historian.Clear()`
    /// (FormDynamicsIntegratorControl.vb:638, :847).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Whether a state is already recorded at exactly this instant — upstream's
    /// `Historian.ContainsKey(integrator.CurrentTime)`
    /// (FormDynamicsIntegratorControl.vb:490).
    #[must_use]
    pub fn contains(&self, at: SimInstant) -> bool {
        self.entries.contains_key(&at)
    }

    /// Record a state, then evict the oldest entries until at most `max_items`
    /// remain.
    ///
    /// Ports FormDynamicsIntegratorControl.vb:490-496 exactly:
    ///
    /// ```text
    /// If EnableHistorian And Not Historian.ContainsKey(CurrentTime) Then
    ///     Historian.Add(CurrentTime, xdoc.ToString().Compress())
    ///     If Historian.Count > MaxHistorianItems Then
    ///         Historian.Remove(Historian.Keys.First())
    ///     End If
    /// End If
    /// ```
    ///
    /// Two details worth stating:
    ///
    /// - The bound is `> max_items`, so the historian settles at exactly
    ///   `max_items` entries, not `max_items - 1`.
    /// - `Historian.Keys.First()` on a .NET `Dictionary` is the **first-inserted**
    ///   key. Under a forward-running clock that is also the oldest, which is
    ///   what this port's `BTreeMap`-first-key eviction reproduces. They differ
    ///   only if entries are inserted out of time order — which the run loop
    ///   never does, since it records under a monotonically advancing clock (and
    ///   skips recording entirely on a backwards step,
    ///   FormDynamicsIntegratorControl.vb:488).
    ///
    /// Returns `false` (recording nothing) if an entry already exists at `at`,
    /// mirroring upstream's `ContainsKey` guard.
    pub fn insert_bounded(
        &mut self,
        at: SimInstant,
        snapshot: FlowsheetSnapshot,
        max_items: usize,
    ) -> bool {
        if self.entries.contains_key(&at) {
            return false;
        }
        self.entries.insert(at, snapshot);
        while self.entries.len() > max_items {
            let oldest = *self
                .entries
                .keys()
                .next()
                .expect("non-empty: len() is greater than max_items");
            self.entries.remove(&oldest);
        }
        true
    }

    /// The state recorded at exactly `at`, if any — upstream's `Historian(htime)`
    /// (`RestoreHistorianState`, FormDynamicsIntegratorControl.vb:249). Upstream
    /// throws `KeyNotFoundException` on a miss and swallows it in a `Catch`;
    /// this returns `None`.
    #[must_use]
    pub fn get_exact(&self, at: SimInstant) -> Option<&FlowsheetSnapshot> {
        self.entries.get(&at)
    }

    /// The oldest recorded state — upstream's `history.Values.First()`
    /// (Manager.vb:243, :251), used as the "initial state" reference for an
    /// event ramp.
    #[must_use]
    pub fn oldest(&self) -> Option<&FlowsheetSnapshot> {
        self.entries.values().next()
    }

    /// The newest state recorded at or before `at` — upstream's
    /// `history.Where(h.Key <= t).OrderByDescending(h.Key).FirstOrDefault()`
    /// (Manager.vb:261, :275).
    #[must_use]
    pub fn newest_at_or_before(&self, at: SimInstant) -> Option<&FlowsheetSnapshot> {
        self.entries.range(..=at).next_back().map(|(_, v)| v)
    }

    /// The instants held, oldest first.
    #[must_use]
    pub fn instants(&self) -> Vec<SimInstant> {
        self.entries.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::property::{property_value, set_property_value, DynamicProperty, PropertyRef};
    use crate::flowsheet::objects::ObjectType;

    fn flowsheet_at(temperature: f64) -> (Flowsheet, PropertyRef) {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        let r = PropertyRef::new(id, DynamicProperty::Temperature);
        set_property_value(&mut fs, &r, temperature).unwrap();
        (fs, r)
    }

    #[test]
    fn snapshot_restores_a_past_state() {
        let (mut fs, r) = flowsheet_at(300.0);
        let snap = FlowsheetSnapshot::capture(&fs);
        set_property_value(&mut fs, &r, 400.0).unwrap();
        assert!((property_value(&fs, &r).unwrap() - 400.0).abs() < 1e-9);
        snap.restore_into(&mut fs);
        assert!((property_value(&fs, &r).unwrap() - 300.0).abs() < 1e-9);
        // Reading without restoring works too.
        assert!((property_value(snap.flowsheet(), &r).unwrap() - 300.0).abs() < 1e-9);
    }

    #[test]
    fn max_items_evicts_the_oldest_and_settles_at_the_bound() {
        let (fs, _) = flowsheet_at(300.0);
        let mut historian = Historian::new();
        for step in 0..10 {
            historian.insert_bounded(
                SimInstant::from_seconds(f64::from(step)),
                FlowsheetSnapshot::capture(&fs),
                3,
            );
        }
        assert_eq!(historian.len(), 3, "bound is '> max', so it settles at max");
        assert_eq!(
            historian.instants(),
            vec![
                SimInstant::from_seconds(7.0),
                SimInstant::from_seconds(8.0),
                SimInstant::from_seconds(9.0)
            ],
            "the three newest survive; the oldest are evicted first"
        );
    }

    #[test]
    fn an_existing_instant_is_never_overwritten() {
        let (fs_a, r) = flowsheet_at(300.0);
        let (fs_b, _) = flowsheet_at(400.0);
        let mut historian = Historian::new();
        assert!(historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs_a), 10));
        assert!(!historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs_b), 10));
        let held = historian.get_exact(SimInstant::ZERO).unwrap();
        assert!((property_value(held.flowsheet(), &r).unwrap() - 300.0).abs() < 1e-9);
    }

    #[test]
    fn lookups_find_the_oldest_and_the_newest_at_or_before() {
        let (fs, _) = flowsheet_at(300.0);
        let mut historian = Historian::new();
        for step in [10.0, 20.0, 30.0] {
            historian.insert_bounded(
                SimInstant::from_seconds(step),
                FlowsheetSnapshot::capture(&fs),
                100,
            );
        }
        assert!(historian.oldest().is_some());
        assert!(historian
            .newest_at_or_before(SimInstant::from_seconds(5.0))
            .is_none());
        assert!(historian
            .get_exact(SimInstant::from_seconds(25.0))
            .is_none());
        assert!(historian
            .newest_at_or_before(SimInstant::from_seconds(25.0))
            .is_some());
        assert!(!historian.is_empty());
        historian.clear();
        assert!(historian.is_empty());
    }
}
