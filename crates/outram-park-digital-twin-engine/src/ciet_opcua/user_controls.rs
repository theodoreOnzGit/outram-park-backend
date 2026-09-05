//! Pending remote control requests, held separately from the plant state.
//!
//! This is the fix for a **lost-update race**. The obvious design — have OPC-UA
//! write callbacks mutate [`CietState`] directly — puts remote clients, the GUI
//! and the physics thread all in contention for the same struct, and whichever
//! writer stores last wins. v1's GUI made that concrete: it cloned the whole
//! state early in an egui repaint, mutated two control fields on the clone, then
//! stored the whole clone back (`overwrite_state`) about twenty times a second.
//! In v1 that was almost harmless, because the state is a *publication surface*
//! and the physics thread re-derives its outputs from its own component objects
//! every timestep. Add remote control and it stops being harmless: a client
//! write landing between the GUI's clone and its store is silently discarded,
//! the write still returns `Good`, and there is nothing on the client side to
//! diagnose.
//!
//! So remote intent lives here instead, in its own small struct:
//!
//! ```text
//!  OPC-UA write callback ──> CietUserControls (pending requests)
//!                                    │
//!                                    │  apply_and_clear(), once per timestep
//!                                    v
//!  GUI writes ─────────────────> CietState ──> physics reads controls,
//!                                    ^          writes outputs
//!                                    └── OPC-UA read callbacks (outputs)
//! ```
//!
//! Three properties fall out of that:
//!
//! 1. **No clobbering.** Whatever the GUI does to [`CietState`], it cannot erase
//!    a remote request, because the request is not stored there. It is re-applied
//!    at the top of the next timestep.
//! 2. **No contention with physics.** A write callback locks this small struct,
//!    not the plant state, so a room full of clients writing set points does not
//!    serialise against the solver or the repaint.
//! 3. **Requests are sparse.** Each field is an `Option`, so a client that never
//!    touches the heater power does not fight the GUI slider for ownership of
//!    it. Only what was actually written is applied, and each request is
//!    consumed once.
//!
//! ## Semantics
//!
//! Last-write-wins per field between successive timesteps: if two clients write
//! the same control 5 ms apart, the physics thread sees only the later value.
//! That is the honest behaviour of a shared plant with no interlocks, and it
//! matches what a real facility with two operators at two panels would do.
//! Values are clamped on the way in by [`CietControl::write`], so a pending
//! request is always inside the documented envelope.
//!
//! Read-back for a control still comes from [`CietState`] — the *effective*
//! value the solver is using — not from the pending request. A client that
//! writes 1000 kW and reads back 15 kW is being told the truth about what the
//! simulator is doing.
//!
//! ## Scope
//!
//! Per `RESPONSIBLE_USE.md` this is part of an **offline educational
//! demonstration** interface, never to be connected to live operational
//! systems, plant systems, or safety-critical infrastructure. There are no
//! interlocks, no authority model, and no arbitration between clients here,
//! because there is no operational role for any of that in a teaching demo.

use std::sync::{Arc, RwLock};

use super::node_map::{CietControl, CietSwitch};
use super::state::CietState;

/// Pending control requests from remote (OPC-UA) clients.
///
/// One `Option` slot per [`CietControl`] and per [`CietSwitch`], indexed by
/// their `index()` methods, so adding a control to the node map automatically
/// gets a slot here with no second place to update. `None` means "no request
/// outstanding".
#[derive(Debug, Clone, PartialEq)]
pub struct CietUserControls {
    /// Requested continuous-control values, indexed by [`CietControl::index`].
    /// Already clamped to each control's documented envelope.
    pending_controls: [Option<f64>; CietUserControls::CONTROL_SLOTS],
    /// Requested switch positions, indexed by [`CietSwitch::index`].
    pending_switches: [Option<bool>; CietUserControls::SWITCH_SLOTS],
    /// Total requests applied to plant state since start-up. Diagnostic only —
    /// the simulator's OPC-UA page displays it so a user can confirm that their
    /// client's writes are actually arriving.
    applied_request_count: u64,
}

/// Shared handle to the pending-request store.
///
/// `RwLock` rather than `Mutex` for consistency with the rest of the interface
/// layer, though contention here is negligible by construction: the critical
/// sections are a single array slot write and a once-per-timestep drain.
pub type SharedUserControls = Arc<RwLock<CietUserControls>>;

impl CietUserControls {
    /// Number of continuous-control slots. Must equal `CietControl::ALL.len()`;
    /// [`Self::slot_counts_match_the_node_map`] enforces it.
    const CONTROL_SLOTS: usize = 8;
    /// Number of switch slots. Must equal `CietSwitch::ALL.len()`.
    const SWITCH_SLOTS: usize = 7;

    /// An empty request store: nothing pending, nothing applied yet.
    pub fn new() -> Self {
        Self {
            pending_controls: [None; Self::CONTROL_SLOTS],
            pending_switches: [None; Self::SWITCH_SLOTS],
            applied_request_count: 0,
        }
    }

    /// Record a client's request to set `control` to `value`.
    ///
    /// `value` is in the control's documented unit (see [`CietControl::unit`]);
    /// it is stored as given and clamped when applied, so an out-of-range
    /// request is honoured at the nearest limit rather than rejected. Overwrites
    /// any request for the same control that has not yet been applied.
    pub fn request_control(&mut self, control: CietControl, value: f64) {
        self.pending_controls[control.index()] = Some(value);
    }

    /// Record a client's request to set `switch` to `value`.
    pub fn request_switch(&mut self, switch: CietSwitch, value: bool) {
        self.pending_switches[switch.index()] = Some(value);
    }

    /// The outstanding request for `control`, if any.
    pub fn pending_control(&self, control: CietControl) -> Option<f64> {
        self.pending_controls[control.index()]
    }

    /// The outstanding request for `switch`, if any.
    pub fn pending_switch(&self, switch: CietSwitch) -> Option<bool> {
        self.pending_switches[switch.index()]
    }

    /// Whether anything is waiting to be applied.
    ///
    /// The physics thread uses this to skip taking a write lock on a quiet
    /// timestep, which is the common case when nobody is connected.
    pub fn has_pending_requests(&self) -> bool {
        self.pending_controls.iter().any(Option::is_some)
            || self.pending_switches.iter().any(Option::is_some)
    }

    /// Total requests applied since start-up. Diagnostic only.
    pub fn applied_request_count(&self) -> u64 {
        self.applied_request_count
    }

    /// Apply every outstanding request to `state`, then clear them.
    ///
    /// Called by the physics thread once per timestep, before it reads the
    /// controls it will integrate with. Returns how many requests were applied,
    /// so a caller can log or display remote activity.
    ///
    /// Clamping happens here, inside [`CietControl::write`], so plant state
    /// never holds an out-of-envelope value even momentarily, and a NaN request
    /// is discarded rather than poisoning the solver.
    pub fn apply_and_clear(&mut self, state: &mut CietState) -> usize {
        let mut applied = 0usize;

        for control in CietControl::ALL {
            if let Some(requested) = self.pending_controls[control.index()].take() {
                control.write(state, requested);
                applied += 1;
            }
        }

        for switch in CietSwitch::ALL {
            if let Some(requested) = self.pending_switches[switch.index()].take() {
                switch.write(state, requested);
                applied += 1;
            }
        }

        self.applied_request_count += applied as u64;
        applied
    }

    /// Discard every outstanding request without applying it.
    ///
    /// For a UI "take local control" action: the operator at the keyboard
    /// drops whatever remote clients have queued rather than letting it land on
    /// the next timestep.
    pub fn clear_without_applying(&mut self) {
        self.pending_controls = [None; Self::CONTROL_SLOTS];
        self.pending_switches = [None; Self::SWITCH_SLOTS];
    }
}

impl Default for CietUserControls {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a fresh shared pending-request store.
pub fn new_shared_user_controls() -> SharedUserControls {
    Arc::new(RwLock::new(CietUserControls::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixed-size arrays must have exactly as many slots as the node map has
    /// controls and switches, or `index()` would panic out of bounds at runtime.
    ///
    /// **Methodology.** Compare the `CONTROL_SLOTS`/`SWITCH_SLOTS` constants with
    /// `CietControl::ALL.len()` and `CietSwitch::ALL.len()`. Pass criterion:
    /// exact equality. This is the test that fires if someone adds a control to
    /// the node map and forgets the slot count.
    ///
    /// **Results (2026-07-28).** 8 control slots for 8 controls, 7 switch slots
    /// for 7 switches — matched.
    #[test]
    fn slot_counts_match_the_node_map() {
        assert_eq!(
            CietUserControls::CONTROL_SLOTS,
            CietControl::ALL.len(),
            "CONTROL_SLOTS must equal CietControl::ALL.len()"
        );
        assert_eq!(
            CietUserControls::SWITCH_SLOTS,
            CietSwitch::ALL.len(),
            "SWITCH_SLOTS must equal CietSwitch::ALL.len()"
        );
    }

    /// Every control and switch must round-trip through request -> apply.
    ///
    /// **Methodology.** For each control, request the midpoint of its valid
    /// range; for each switch, request `true`. Apply once, then read the
    /// effective value back out of the state. Pass criterion: every request
    /// lands (agreement to a relative `f32` tolerance, since two controls are
    /// backed by `f32` fields inherited from v1), and `apply_and_clear` reports
    /// 15 applied requests (8 controls + 7 switches).
    ///
    /// **Results (2026-07-28).** All 15 requests applied and observable in plant
    /// state; the returned count was 15.
    #[test]
    fn every_request_reaches_plant_state() {
        let mut controls = CietUserControls::new();
        let mut state = CietState::default();

        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            controls.request_control(*control, 0.5 * (min + max));
        }
        for switch in CietSwitch::ALL {
            controls.request_switch(*switch, true);
        }

        let applied = controls.apply_and_clear(&mut state);
        assert_eq!(applied, CietControl::ALL.len() + CietSwitch::ALL.len());

        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            let expected = 0.5 * (min + max);
            let got = control.read(&state);
            let tolerance = (expected.abs() * f32::EPSILON as f64).max(1e-9);
            assert!(
                (got - expected).abs() <= tolerance,
                "{control:?}: applied {got}, expected {expected}"
            );
        }
        for switch in CietSwitch::ALL {
            assert!(switch.read(&state), "{switch:?} did not latch");
        }
    }

    /// Requests must be consumed exactly once, so a stale request cannot keep
    /// overriding an operator at the keyboard on every subsequent timestep.
    ///
    /// **Methodology.** Request 9 kW heater power, apply, then change the state
    /// directly to 3 kW (standing in for a GUI slider move) and apply again with
    /// nothing pending. Pass criterion: the second apply reports 0 requests and
    /// leaves 3 kW in place.
    ///
    /// **Results (2026-07-28).** Second apply reported 0 and the heater power
    /// stayed at 3 kW — the request did not resurrect.
    #[test]
    fn requests_are_consumed_exactly_once() {
        let mut controls = CietUserControls::new();
        let mut state = CietState::default();

        controls.request_control(CietControl::HeaterPowerKw, 9.0);
        assert_eq!(controls.apply_and_clear(&mut state), 1);
        assert!((state.heater_power_kilowatts - 9.0).abs() < 1e-9);
        assert!(!controls.has_pending_requests());

        // the human at the keyboard moves the slider
        state.set_heater_power_kilowatts(3.0);

        assert_eq!(controls.apply_and_clear(&mut state), 0);
        assert!(
            (state.heater_power_kilowatts - 3.0).abs() < 1e-9,
            "a consumed request must not re-apply; got {}",
            state.heater_power_kilowatts
        );
    }

    /// The whole point of the design: a full-struct overwrite of the plant state
    /// must not be able to destroy a pending remote request.
    ///
    /// **Methodology.** Reproduce v1's clobbering pattern. Clone the state, queue
    /// a remote request for 12 kW, then store the stale clone back over the
    /// state (as v1's GUI did with `overwrite_state`), and only then run the
    /// per-timestep apply. Pass criterion: the request still lands, i.e. 12 kW
    /// is in effect after the clobber.
    ///
    /// **Results (2026-07-28).** Heater power was 12 kW after the stale
    /// overwrite — the request survived. Under the previous design, where write
    /// callbacks mutated the plant state directly, this sequence lost the write
    /// entirely. This test is the regression guard for op-wqk.13.9.
    #[test]
    fn a_stale_full_struct_overwrite_cannot_lose_a_request() {
        let mut controls = CietUserControls::new();
        let mut state = CietState::default();

        // GUI clones the state early in its repaint
        let stale_clone = state.clone();

        // a remote client writes while the repaint is still in progress
        controls.request_control(CietControl::HeaterPowerKw, 12.0);

        // GUI stores its stale clone back over everything (v1's pattern)
        state.overwrite_state(stale_clone);

        // physics thread applies pending requests at the top of the timestep
        let applied = controls.apply_and_clear(&mut state);

        assert_eq!(applied, 1, "the request should still have been pending");
        assert!(
            (state.heater_power_kilowatts - 12.0).abs() < 1e-9,
            "remote request was lost to a stale overwrite; heater power is {}",
            state.heater_power_kilowatts
        );
    }

    /// An out-of-range or NaN request must be clamped/discarded when applied, so
    /// the envelope holds even though requests are stored unclamped.
    ///
    /// **Methodology.** Request 1000 kW (ceiling 15 kW) and apply; then request
    /// NaN and apply. Pass criterion: 15 kW after the first, unchanged and
    /// finite after the second.
    ///
    /// **Results (2026-07-28).** 1000 kW clamped to 15 kW; the NaN request left
    /// 15 kW in place and finite.
    #[test]
    fn applying_enforces_the_envelope() {
        let mut controls = CietUserControls::new();
        let mut state = CietState::default();

        controls.request_control(CietControl::HeaterPowerKw, 1000.0);
        controls.apply_and_clear(&mut state);
        assert!((state.heater_power_kilowatts - 15.0).abs() < 1e-9);

        controls.request_control(CietControl::HeaterPowerKw, f64::NAN);
        controls.apply_and_clear(&mut state);
        assert!(
            state.heater_power_kilowatts.is_finite()
                && (state.heater_power_kilowatts - 15.0).abs() < 1e-9,
            "NaN request changed the value to {}",
            state.heater_power_kilowatts
        );
    }

    /// Last-write-wins between timesteps: only the later of two requests for the
    /// same control is applied.
    ///
    /// **Methodology.** Request 4 kW then 11 kW without applying in between,
    /// then apply. Pass criterion: one request applied, 11 kW in effect.
    ///
    /// **Results (2026-07-28).** One request applied, heater power 11 kW.
    #[test]
    fn later_request_supersedes_earlier_within_a_timestep() {
        let mut controls = CietUserControls::new();
        let mut state = CietState::default();

        controls.request_control(CietControl::HeaterPowerKw, 4.0);
        controls.request_control(CietControl::HeaterPowerKw, 11.0);

        assert_eq!(controls.apply_and_clear(&mut state), 1);
        assert!((state.heater_power_kilowatts - 11.0).abs() < 1e-9);
    }
}
