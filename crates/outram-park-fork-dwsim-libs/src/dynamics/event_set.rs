//! Named collections of scheduled events.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/EventSet.vb` (whole file,
//! lines 21-56), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! A [`crate::dynamics::schedule::Schedule`] names exactly one event set
//! (`CurrentEventList`); the run loop replays that set every step when
//! `UsesEventList` is on (FormDynamicsIntegratorControl.vb:556-558).
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData` (EventSet.vb:31-53),
//!   including the nested `<Events>` element and its
//!   `Events.Add(ev.ID, ev)` reload.
//!
//! # Divergence: map type
//!
//! Upstream is a `Dictionary(Of String, IDynamicsEvent)` (EventSet.vb:29), whose
//! iteration order is unspecified in principle and insertion-ordered in
//! practice. This port uses a [`BTreeMap`] keyed by the same event ID, so
//! iteration is deterministic (ID order). This matters where upstream sorts
//! events by timestamp (Manager.vb:217) — a stable sort leaves equal timestamps
//! in map order, so a deterministic map makes tie-breaking reproducible.

use std::collections::BTreeMap;

use crate::dynamics::event::DynamicEvent;
use crate::dynamics::sim_time::SimInstant;

/// A named set of [`DynamicEvent`]s — upstream's `EventSet` (EventSet.vb:21-56).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventSet {
    /// Unique identifier, and the key the manager stores this set under
    /// (EventSet.vb:25).
    pub id: String,
    /// Human-readable label. **This is what
    /// [`crate::dynamics::manager::DynamicsManager::event_set_by_description`]
    /// looks up by** — upstream's `GetEventSet` matches on `Description`, not on
    /// `ID` (Manager.vb:201-203). (EventSet.vb:27.)
    pub description: String,
    /// The events, keyed by [`DynamicEvent::id`] (EventSet.vb:29).
    pub events: BTreeMap<String, DynamicEvent>,
}

impl EventSet {
    /// An empty set with the given ID and description.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        EventSet {
            id: id.into(),
            description: description.into(),
            events: BTreeMap::new(),
        }
    }

    /// Insert (or replace) an event, keyed by its own ID — the shape upstream's
    /// `LoadData` reload uses (`Events.Add(ev.ID, ev)`, EventSet.vb:49).
    pub fn insert(&mut self, event: DynamicEvent) -> Option<DynamicEvent> {
        self.events.insert(event.id.clone(), event)
    }

    /// All events sorted by [`DynamicEvent::timestamp`], oldest first — the
    /// ordering `GetPropertyValuesFromEvents` works in (Manager.vb:217,
    /// `eventset.Events.Values.OrderBy(Function(e) e.TimeStamp)`).
    ///
    /// The sort is stable, so events with the same timestamp keep event-ID
    /// order.
    #[must_use]
    pub fn events_by_time(&self) -> Vec<&DynamicEvent> {
        let mut events: Vec<&DynamicEvent> = self.events.values().collect();
        events.sort_by_key(|e| e.timestamp);
        events
    }

    /// The events whose timestamp lies in the half-open window
    /// `[final_time - interval, final_time)`.
    ///
    /// Ports the selection at FormDynamicsIntegratorControl.vb:160-164:
    ///
    /// ```text
    /// initialtime = currentposition - interval
    /// finaltime   = currentposition
    /// events = eventset.Events.Values.Where(x.TimeStamp >= initialtime And x.TimeStamp < finaltime)
    /// ```
    ///
    /// Note the asymmetry — inclusive at the lower bound, exclusive at the upper
    /// — which is what stops an event firing twice on consecutive steps. The
    /// result is timestamp-ordered.
    #[must_use]
    pub fn events_in_window(
        &self,
        final_time: SimInstant,
        interval_seconds: f64,
    ) -> Vec<&DynamicEvent> {
        let initial = final_time.add_seconds(-interval_seconds);
        self.events_by_time()
            .into_iter()
            .filter(|e| e.timestamp >= initial && e.timestamp < final_time)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::property::DynamicProperty;
    use crate::flowsheet::objects::ObjectId;

    fn event_at(id: &str, seconds: f64) -> DynamicEvent {
        DynamicEvent::change_property(
            id,
            SimInstant::from_seconds(seconds),
            ObjectId::from("S-1"),
            DynamicProperty::Temperature,
            300.0,
            "K",
        )
    }

    #[test]
    fn insert_keys_by_event_id() {
        let mut set = EventSet::new("es-1", "Startup");
        set.insert(event_at("b", 2.0));
        set.insert(event_at("a", 1.0));
        assert_eq!(set.events.len(), 2);
        assert!(set.events.contains_key("a"));
    }

    #[test]
    fn events_are_sorted_by_timestamp_not_by_id() {
        let mut set = EventSet::new("es-1", "Startup");
        set.insert(event_at("a", 30.0));
        set.insert(event_at("b", 10.0));
        set.insert(event_at("c", 20.0));
        let ids: Vec<&str> = set.events_by_time().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn window_is_inclusive_below_and_exclusive_above() {
        let mut set = EventSet::new("es-1", "Startup");
        set.insert(event_at("at-lower", 10.0));
        set.insert(event_at("inside", 12.5));
        set.insert(event_at("at-upper", 15.0));
        // window [10, 15)
        let ids: Vec<&str> = set
            .events_in_window(SimInstant::from_seconds(15.0), 5.0)
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, vec!["at-lower", "inside"]);
    }

    #[test]
    fn consecutive_windows_fire_each_event_exactly_once() {
        let mut set = EventSet::new("es-1", "Startup");
        set.insert(event_at("e", 10.0));
        let first = set.events_in_window(SimInstant::from_seconds(10.0), 5.0);
        let second = set.events_in_window(SimInstant::from_seconds(15.0), 5.0);
        assert!(
            first.is_empty(),
            "10 is the exclusive upper bound of [5,10)"
        );
        assert_eq!(
            second.len(),
            1,
            "10 is the inclusive lower bound of [10,15)"
        );
    }
}
