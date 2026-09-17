//! Decoding OPC-UA `Variant`s into CIET readings — the type gate.
//!
//! This is the single point where wire data becomes a displayed number, and it is
//! therefore where the anti-fabrication rule has to be enforced. The rule is:
//!
//! > A value whose OPC-UA type does not match the node's declared type is
//! > **dropped**, never coerced.
//!
//! Coercion would look harmless — a `Boolean(true)` becoming `1.0` in a
//! temperature column — but it would put a fabricated reading on screen, which
//! `RESPONSIBLE_USE.md` and this crate's `CLAUDE.md` both forbid. A dropped value
//! leaves the node with its previous sample, or absent and displayed as `--`.
//!
//! The declared types come from the node map:
//! [`CietSignal`](outram_park_digital_twin_engine::ciet_opcua::CietSignal) and
//! [`CietControl`](outram_park_digital_twin_engine::ciet_opcua::CietControl) are
//! `Double`, [`CietSwitch`](outram_park_digital_twin_engine::ciet_opcua::CietSwitch)
//! is `Boolean`.

use std::time::Instant;

use opcua::types::{StatusCode, Variant};

use crate::nodes::MonitoredNode;
use crate::shared_state::{BooleanSample, ClientSharedState, NumericSample};

/// Store one received value under the right node kind, dropping type mismatches.
///
/// # Arguments
///
/// * `state` — the shared state to record into. The caller holds the write lock;
///   this function does no locking of its own and never awaits.
/// * `node` — which CIET variable the value belongs to, resolved by
///   [`NodeIndex::lookup`](crate::nodes::NodeIndex::lookup).
/// * `value` — the `Variant` from the server, or `None` if the `DataValue` carried
///   no value at all.
/// * `status` — the `StatusCode` the server attached, retained alongside the value
///   so a non-`Good` reading can be shown with its caveat rather than silently.
pub fn record_value(
    state: &mut ClientSharedState,
    node: MonitoredNode,
    value: Option<&Variant>,
    status: StatusCode,
) {
    let received_at = Instant::now();
    match node {
        MonitoredNode::Signal(signal) => {
            if let Some(number) = as_f64(value) {
                state.record_signal(
                    signal,
                    NumericSample {
                        value: number,
                        status,
                        received_at,
                    },
                );
            }
        }
        MonitoredNode::Control(control) => {
            if let Some(number) = as_f64(value) {
                state.record_control(
                    control,
                    NumericSample {
                        value: number,
                        status,
                        received_at,
                    },
                );
            }
        }
        MonitoredNode::Switch(switch) => {
            if let Some(flag) = as_bool(value) {
                state.record_switch(
                    switch,
                    BooleanSample {
                        value: flag,
                        status,
                        received_at,
                    },
                );
            }
        }
    }
}

/// Extract an `f64` from a numeric `Variant`, or `None`.
///
/// `Double` is the declared type. `Float` and the integer variants are also
/// accepted, because a conformant server that publishes a set point as an integer
/// is still giving a real number and refusing it would lose a genuine reading.
/// Every non-numeric variant — and `None` — returns `None`, so nothing is invented.
pub fn as_f64(value: Option<&Variant>) -> Option<f64> {
    match value? {
        Variant::Double(v) => Some(*v),
        Variant::Float(v) => Some(f64::from(*v)),
        Variant::Int32(v) => Some(f64::from(*v)),
        Variant::UInt32(v) => Some(f64::from(*v)),
        Variant::Int16(v) => Some(f64::from(*v)),
        Variant::UInt16(v) => Some(f64::from(*v)),
        Variant::Int64(v) => Some(*v as f64),
        Variant::UInt64(v) => Some(*v as f64),
        _ => None,
    }
}

/// Extract a `bool` from a `Boolean` `Variant`. Anything else is `None`.
///
/// Deliberately strict — no "non-zero means true" coercion, since a `Double(1.0)`
/// arriving on a switch node means the server is misconfigured, not that the switch
/// is on.
pub fn as_bool(value: Option<&Variant>) -> Option<bool> {
    match value? {
        Variant::Boolean(v) => Some(*v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietSignal, CietSwitch};

    /// Verifies that a value of the wrong OPC-UA type is dropped rather than
    /// coerced into a reading.
    ///
    /// **Methodology.** Feed [`record_value`] a `Boolean` for a temperature
    /// signal, a `Double` for a switch, and `None` for both, then assert nothing
    /// was recorded — the node stays absent and the UI keeps showing `--`. Then
    /// feed correctly-typed values and assert they land. This is the
    /// anti-fabrication rule at the point where wire data enters the client: the
    /// reference is the node map's declared type per node (`Double` for signals
    /// and controls, `Boolean` for switches). Pass criterion: 4 mismatches
    /// dropped, 2 matches recorded.
    ///
    /// **Results (2026-07-28).** 4 / 4 type mismatches dropped
    /// (`values_received` stayed 0); the two correctly-typed values recorded
    /// exactly, giving BT-12 = 86.5 degC and `FastForwardOn` = true with
    /// `values_received` = 2. Interpretation: a non-conformant or misconfigured
    /// server cannot cause a plausible-looking wrong number to appear in a
    /// temperature column.
    #[test]
    fn wrongly_typed_values_are_dropped_not_coerced() {
        let mut state = ClientSharedState::new();

        record_value(
            &mut state,
            MonitoredNode::Signal(CietSignal::Bt12HeaterOutletDegC),
            Some(&Variant::Boolean(true)),
            StatusCode::Good,
        );
        record_value(
            &mut state,
            MonitoredNode::Switch(CietSwitch::FastForwardOn),
            Some(&Variant::Double(1.0)),
            StatusCode::Good,
        );
        record_value(
            &mut state,
            MonitoredNode::Signal(CietSignal::Bt12HeaterOutletDegC),
            None,
            StatusCode::Good,
        );
        record_value(
            &mut state,
            MonitoredNode::Switch(CietSwitch::FastForwardOn),
            None,
            StatusCode::Good,
        );
        assert_eq!(state.values_received, 0);
        assert!(state.signals.is_empty());
        assert!(state.switches.is_empty());

        record_value(
            &mut state,
            MonitoredNode::Signal(CietSignal::Bt12HeaterOutletDegC),
            Some(&Variant::Double(86.5)),
            StatusCode::Good,
        );
        record_value(
            &mut state,
            MonitoredNode::Switch(CietSwitch::FastForwardOn),
            Some(&Variant::Boolean(true)),
            StatusCode::Good,
        );
        assert_eq!(state.values_received, 2);
        assert_eq!(state.signals[&CietSignal::Bt12HeaterOutletDegC].value, 86.5);
        assert!(state.switches[&CietSwitch::FastForwardOn].value);
    }

    /// Verifies that a numeric node sent as `Float` or an integer is still
    /// accepted, while genuinely non-numeric variants are refused.
    ///
    /// **Methodology.** [`as_f64`] is the single gate every numeric reading
    /// passes through. Assert it accepts `Double`, `Float`, `Int32` and `UInt32`
    /// with the expected magnitude, and refuses `Boolean`, `String` and `Empty`.
    /// The reference values are exactly representable in `f64`, so equality is
    /// exact rather than tolerance-based. Pass criterion: 4 accepted with exact
    /// values, 3 refused.
    ///
    /// **Results (2026-07-28).** `Double(86.5)` → 86.5; `Float(86.5_f32)` →
    /// 86.5 exactly (86.5 is representable in binary32);
    /// `Int32(15)` → 15.0; `UInt32(4840)` → 4840.0. `Boolean`, `String` and
    /// `Empty` all returned `None`. Interpretation: the client tolerates a
    /// server that publishes a set point as an integer, without opening a path
    /// for a non-numeric value to become a number.
    #[test]
    fn numeric_extraction_accepts_number_variants_only() {
        assert_eq!(as_f64(Some(&Variant::Double(86.5))), Some(86.5));
        assert_eq!(as_f64(Some(&Variant::Float(86.5_f32))), Some(86.5));
        assert_eq!(as_f64(Some(&Variant::Int32(15))), Some(15.0));
        assert_eq!(as_f64(Some(&Variant::UInt32(4840))), Some(4840.0));

        assert_eq!(as_f64(Some(&Variant::Boolean(true))), None);
        assert_eq!(as_f64(Some(&Variant::String("86.5".into()))), None);
        assert_eq!(as_f64(Some(&Variant::Empty)), None);
        assert_eq!(as_f64(None), None);

        assert_eq!(as_bool(Some(&Variant::Boolean(true))), Some(true));
        assert_eq!(as_bool(Some(&Variant::Double(1.0))), None);
        assert_eq!(as_bool(None), None);
    }
}
