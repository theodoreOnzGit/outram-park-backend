//! GUI-local edit state for the controls, and the read-back comparison.
//!
//! ## Why drafts are separate from readings
//!
//! A slider needs somewhere to keep the value the user is dragging *before* it is
//! written. If it wrote straight into the shared reading, the next subscription
//! update would yank the handle back under the user's finger, and the read-back —
//! the whole point of the panel — would be overwritten by the request.
//!
//! So the *draft* (what the user has dialled up) and the *read-back* (what the
//! server says it holds) are two separate values, shown side by side. When they
//! differ after a write, that difference is information:
//! [`compare_readback`] names what kind.
//!
//! ## The clamp is the server's, and it is visible
//!
//! Every [`CietControl`] has a documented `valid_range()`, and the simulator
//! clamps writes to it rather than rejecting them — send 1000 kW and the heater
//! takes 15 kW. This client deliberately does **not** pre-clamp the value it
//! sends: it sends what the user asked for, so the clamp happens where it is
//! specified to happen and the user sees it in the read-back. The slider is
//! bounded by the range for convenience, but the numeric box is not, so the
//! clamping behaviour can actually be demonstrated.

use std::collections::HashMap;

use outram_park_digital_twin_engine::ciet_opcua::node_map::CietControl;

/// Per-control values the user has dialled up but not necessarily written.
///
/// A control with no entry has not been touched, and the UI shows the server's
/// read-back in its place. No entry is created until the user edits something, so
/// an untouched panel proposes nothing.
#[derive(Debug, Clone, Default)]
pub struct ControlDrafts {
    values: HashMap<CietControl, f64>,
}

impl ControlDrafts {
    /// No drafts.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value the slider and numeric box should show.
    ///
    /// # Arguments
    ///
    /// * `control` — which control.
    /// * `readback` — the server's current value for it, or `None` if this
    ///   client has not read it yet.
    ///
    /// Precedence: the user's draft, then the server's read-back, then the low
    /// end of the control's `valid_range()`. Falling back to the range minimum
    /// rather than to zero matters for
    /// [`CtahPumpPressurePascals`](CietControl::CtahPumpPressurePascals), whose
    /// range is symmetric about zero, and for
    /// [`TimestepSeconds`](CietControl::TimestepSeconds), whose minimum is
    /// 0.001 s — a displayed 0 there would be outside the valid range.
    ///
    /// This is a *display* value only. It is never presented as a measurement:
    /// the read-back column shows `--` independently whenever `readback` is
    /// `None`, so a fallback shown in the slider cannot be mistaken for a
    /// reading from the server.
    pub fn value_for(&self, control: CietControl, readback: Option<f64>) -> f64 {
        if let Some(draft) = self.values.get(&control) {
            return *draft;
        }
        if let Some(value) = readback {
            return value;
        }
        control.valid_range().0
    }

    /// Record an edit.
    pub fn set(&mut self, control: CietControl, value: f64) {
        self.values.insert(control, value);
    }

    /// Whether the user has edited this control since the last
    /// [`clear`](Self::clear).
    pub fn is_edited(&self, control: CietControl) -> bool {
        self.values.contains_key(&control)
    }

    /// Forget one control's draft, so it tracks the server's read-back again.
    pub fn clear(&mut self, control: CietControl) {
        self.values.remove(&control);
    }

    /// Forget every draft. Called on disconnect, so a new session does not open
    /// with the previous one's proposed set points.
    pub fn clear_all(&mut self) {
        self.values.clear();
    }
}

/// How the server's read-back relates to what the user asked for.
///
/// An enum so the UI's `match` is exhaustive and the "not read yet" case cannot
/// be rendered as agreement — which would amount to claiming a confirmation the
/// client never received.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadBackComparison {
    /// This client has not read the control back yet. Displayed as `--`, never
    /// as a match.
    NotReadYet,

    /// The server holds the requested value, to within
    /// [`READBACK_TOLERANCE_RELATIVE`].
    Matches,

    /// The request was outside the control's `valid_range()` and the server
    /// clamped it to the nearest limit — the documented, expected behaviour.
    Clamped {
        /// The value the server actually holds, i.e. the limit.
        held: f64,
        /// The limit the value was clamped to. Equal to `held` in practice;
        /// carried separately so the UI can name which end.
        limit: f64,
    },

    /// The read-back differs for some other reason — a `f32`-backed field
    /// round-tripping, or another client having written since, or advanced heater
    /// control overwriting the heater power set point every timestep.
    Differs {
        /// The value the server holds.
        held: f64,
    },
}

/// Relative tolerance for calling a read-back a match.
///
/// Not zero, because several CIET state fields are `f32`
/// (`ctah_pump_pressure_pascals`, `timestep_seconds`), so a `f64` request
/// round-trips through 24 bits of mantissa. 1e-6 relative is about 8 times the
/// `f32` epsilon of 1.19e-7 — loose enough to absorb that conversion, tight
/// enough that a genuine clamp or an overwrite by another writer is never called
/// a match.
pub const READBACK_TOLERANCE_RELATIVE: f64 = 1.0e-6;

/// Absolute tolerance floor, for requests at or near zero where a relative
/// tolerance degenerates.
pub const READBACK_TOLERANCE_ABSOLUTE: f64 = 1.0e-9;

/// Classify a control's read-back against the value the user requested.
///
/// # Arguments
///
/// * `control` — which control, used for its `valid_range()`.
/// * `requested` — the value this client wrote.
/// * `readback` — what the server reports it now holds, or `None` if unread.
///
/// # Returns
///
/// [`ReadBackComparison::NotReadYet`] when `readback` is `None`;
/// [`ReadBackComparison::Matches`] within tolerance;
/// [`ReadBackComparison::Clamped`] when the request lay outside the range and the
/// read-back sits at the nearest limit; otherwise
/// [`ReadBackComparison::Differs`].
pub fn compare_readback(
    control: CietControl,
    requested: f64,
    readback: Option<f64>,
) -> ReadBackComparison {
    let Some(held) = readback else {
        return ReadBackComparison::NotReadYet;
    };

    let tolerance =
        (READBACK_TOLERANCE_RELATIVE * requested.abs()).max(READBACK_TOLERANCE_ABSOLUTE);
    if (held - requested).abs() <= tolerance {
        return ReadBackComparison::Matches;
    }

    let (min, max) = control.valid_range();
    if requested < min
        && (held - min).abs() <= tolerance.max(READBACK_TOLERANCE_RELATIVE * min.abs())
    {
        return ReadBackComparison::Clamped { held, limit: min };
    }
    if requested > max
        && (held - max).abs() <= tolerance.max(READBACK_TOLERANCE_RELATIVE * max.abs())
    {
        return ReadBackComparison::Clamped { held, limit: max };
    }

    ReadBackComparison::Differs { held }
}

impl ReadBackComparison {
    /// Short note for the column beside the control, or `None` when the
    /// read-back agrees and there is nothing worth saying.
    pub fn note(&self) -> Option<String> {
        match self {
            Self::NotReadYet | Self::Matches => None,
            Self::Clamped { limit, .. } => {
                Some(format!("server clamped it to its limit of {limit}"))
            }
            Self::Differs { held } => Some(format!(
                "server holds {held:.4} -- something else is driving it"
            )),
        }
    }

    /// Whether the UI should draw this in a warning colour.
    pub fn is_noteworthy(&self) -> bool {
        matches!(self, Self::Clamped { .. } | Self::Differs { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the slider's displayed value falls back through draft →
    /// read-back → range minimum, and never to a bare zero outside the valid
    /// range.
    ///
    /// **Methodology.** For every [`CietControl`], with no draft and no
    /// read-back, assert `value_for` returns a value inside the control's
    /// documented `valid_range()`. Then supply a read-back and assert it is
    /// preferred; then set a draft and assert *that* is preferred. The reference
    /// is `CietControl::valid_range()` from the node map. Pass criterion: for all
    /// 8 controls the fallback is in range, and the precedence order holds.
    ///
    /// **Results (2026-07-28).** 8 / 8 controls returned an in-range fallback.
    /// The values that make the point: `TimestepSeconds` fell back to 0.001 s
    /// (its documented minimum, not 0, which is out of range), and
    /// `CtahPumpPressurePascals` fell back to its negative limit rather than to
    /// zero. Precedence held for all 8: draft beat read-back, read-back beat
    /// fallback. Interpretation: no control can open showing a value the server
    /// would refuse to accept unclamped.
    #[test]
    fn slider_values_fall_back_within_the_documented_range() {
        let mut drafts = ControlDrafts::new();

        for control in CietControl::ALL {
            let (min, max) = control.valid_range();

            let fallback = drafts.value_for(*control, None);
            assert!(
                fallback >= min && fallback <= max,
                "{control:?}: fallback {fallback} outside [{min}, {max}]"
            );

            let readback = 0.5 * (min + max);
            assert_eq!(drafts.value_for(*control, Some(readback)), readback);

            drafts.set(*control, min);
            assert_eq!(drafts.value_for(*control, Some(readback)), min);
            assert!(drafts.is_edited(*control));

            drafts.clear(*control);
            assert!(!drafts.is_edited(*control));
            assert_eq!(drafts.value_for(*control, Some(readback)), readback);
        }

        assert_eq!(
            drafts.value_for(CietControl::TimestepSeconds, None),
            CietControl::TimestepSeconds.valid_range().0
        );
    }

    /// Verifies that a server-side clamp is displayed as a clamp, an unread
    /// control as unread, and an agreeing read-back as a match — for every
    /// control.
    ///
    /// **Methodology.** For each of the 8 [`CietControl`]s, run four cases
    /// against `compare_readback`: (a) `readback = None` → `NotReadYet`;
    /// (b) requested = midpoint, readback = midpoint → `Matches`;
    /// (c) requested = `max + 1e6`, readback = `max` → `Clamped { limit: max }`;
    /// (d) requested = `min - 1e6`, readback = `min` → `Clamped { limit: min }`.
    /// The clamp reference is the node map's own documented behaviour, verified
    /// independently by `node_map::tests::every_control_clamps_out_of_range_writes`
    /// — so this test checks the *client's presentation* of a clamp the server is
    /// already known to perform. Pass criterion: 32 classifications correct.
    ///
    /// **Results (2026-07-28).** 32 / 32 correct across all 8 controls. Worked
    /// example, heater power (`valid_range()` = 0.0 to 15.0 kW): a request of
    /// 1000015.0 kW read back as 15.0 kW classified as
    /// `Clamped { held: 15.0, limit: 15.0 }`, and its note rendered as "server
    /// clamped it to its limit of 15". Interpretation: a student who types an
    /// absurd set point is shown the ceiling and told that is what happened,
    /// which is the pedagogically useful outcome.
    #[test]
    fn server_side_clamping_is_displayed_as_clamping() {
        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            let midpoint = 0.5 * (min + max);

            assert_eq!(
                compare_readback(*control, midpoint, None),
                ReadBackComparison::NotReadYet,
                "{control:?}: unread should not be a match"
            );

            assert_eq!(
                compare_readback(*control, midpoint, Some(midpoint)),
                ReadBackComparison::Matches,
                "{control:?}: identical read-back should match"
            );

            let too_high = max + 1.0e6;
            assert_eq!(
                compare_readback(*control, too_high, Some(max)),
                ReadBackComparison::Clamped {
                    held: max,
                    limit: max
                },
                "{control:?}: high clamp not recognised"
            );

            let too_low = min - 1.0e6;
            assert_eq!(
                compare_readback(*control, too_low, Some(min)),
                ReadBackComparison::Clamped {
                    held: min,
                    limit: min
                },
                "{control:?}: low clamp not recognised"
            );
        }

        // The worked example quoted in this test's documentation.
        let (_, heater_max) = CietControl::HeaterPowerKw.valid_range();
        assert_eq!(
            heater_max, 15.0,
            "heater ceiling changed; update the doc numbers"
        );
        let comparison = compare_readback(CietControl::HeaterPowerKw, 1_000_015.0, Some(15.0));
        assert_eq!(
            comparison,
            ReadBackComparison::Clamped {
                held: 15.0,
                limit: 15.0
            }
        );
        assert_eq!(
            comparison.note().unwrap(),
            "server clamped it to its limit of 15"
        );
    }

    /// Verifies an in-range read-back that does not match the request is called
    /// out as such rather than silently accepted.
    ///
    /// **Methodology.** Write a mid-range heater power and read back a different
    /// in-range value — the situation produced when the simulator's advanced
    /// heater control is switched on and overwrites the set point every timestep,
    /// which the node map documents. Pass criterion: `Differs`, flagged
    /// noteworthy, with a note naming the held value.
    ///
    /// **Results (2026-07-28).** A request of 8.0 kW read back as 5.25 kW gave
    /// `Differs { held: 5.25 }`, `is_noteworthy()` true, note "server holds
    /// 5.2500 -- something else is driving it". Interpretation: the user is told
    /// their write was accepted but is being overridden, rather than concluding
    /// the client is broken.
    #[test]
    fn an_overridden_set_point_is_flagged_rather_than_accepted() {
        let comparison = compare_readback(CietControl::HeaterPowerKw, 8.0, Some(5.25));
        assert_eq!(comparison, ReadBackComparison::Differs { held: 5.25 });
        assert!(comparison.is_noteworthy());
        assert!(comparison.note().unwrap().contains("5.2500"));

        assert!(!ReadBackComparison::Matches.is_noteworthy());
        assert!(ReadBackComparison::Matches.note().is_none());
        assert!(!ReadBackComparison::NotReadYet.is_noteworthy());
        assert!(ReadBackComparison::NotReadYet.note().is_none());
    }

    /// Verifies the read-back tolerance absorbs an `f32` round-trip without
    /// masking a real difference.
    ///
    /// **Methodology.** Two CIET state fields behind controls are `f32`
    /// (`ctah_pump_pressure_pascals`, `timestep_seconds`), so a `f64` request
    /// returns having lost mantissa bits. Write 0.037 s to
    /// `TimestepSeconds`, round-trip it through `f32` exactly as the server does,
    /// and assert `Matches`. Then assert a difference an order of magnitude
    /// larger than the `f32` error is *not* called a match. Pass criterion:
    /// `Matches` for the round-trip, `Differs` for the coarser change.
    ///
    /// **Results (2026-07-28).** 0.037 f64 → f32 → f64 measured as
    /// 0.03700000047683716, an absolute error of 4.77e-10 and a relative error of
    /// 1.29e-8 — inside the 1e-6 relative tolerance, classified `Matches`. A
    /// read-back of 0.0371 (relative error 2.7e-3) classified `Differs`.
    /// Interpretation: the tolerance is set by the storage precision of the
    /// server's own fields, and has roughly two orders of magnitude of margin
    /// before it would hide a meaningful discrepancy.
    #[test]
    fn the_readback_tolerance_absorbs_an_f32_round_trip_only() {
        let requested = 0.037_f64;
        let through_f32 = requested as f32 as f64;
        let relative_error = (through_f32 - requested).abs() / requested;
        assert!(
            relative_error < READBACK_TOLERANCE_RELATIVE,
            "f32 round-trip error {relative_error:e} exceeds the tolerance"
        );
        assert_eq!(
            compare_readback(CietControl::TimestepSeconds, requested, Some(through_f32)),
            ReadBackComparison::Matches
        );

        assert_eq!(
            compare_readback(CietControl::TimestepSeconds, requested, Some(0.0371)),
            ReadBackComparison::Differs { held: 0.0371 }
        );
    }
}
