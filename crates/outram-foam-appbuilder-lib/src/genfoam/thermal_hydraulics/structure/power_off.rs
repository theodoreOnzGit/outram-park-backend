// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerOffCriterionModels/{powerOffCriterionModel,timer/*,fieldValue/*}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `power_off` — criteria that scram the structure power
//!
//! Port of GeN-Foam's `powerOffCriterionModel` family. Each timestep the
//! structure evaluates its criterion; once satisfied, every power model's
//! internal source is zeroed ([`super::power_model::PowerModel::power_off`]) —
//! the software analogue of a reactor trip / SCRAM in the educational model.
//!
//! ## Criteria (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Fires when |
//! |---|---|---|
//! | [`PowerOffCriterion::Timer`] | `timer` | The simulation time reaches a set instant |
//! | [`PowerOffCriterion::FieldValue`] | `fieldValue` | A monitored field's max/min crosses a threshold (with an optional confirmation delay) |
//!
//! Both are evaluated through [`PowerOffCriterion::check`]. The `fieldValue`
//! criterion carries a small latch (`time0`) so the optional `time_delay`
//! confirmation window is measured from first crossing, faithfully to upstream.

use uom::si::f64::Time;

/// Which extremum of the monitored field the `fieldValue` criterion reduces to.
///
/// Port of GeN-Foam's `fieldValue::fieldOp` (`max` / `min`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldReduction {
    /// Use the field maximum over the domain.
    Max,
    /// Use the field minimum over the domain.
    Min,
}

/// The direction of the threshold test for the `fieldValue` criterion.
///
/// Port of GeN-Foam's `fieldValue::criterion`
/// (`valueAboveThreshold` / `valueBelowThreshold`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdDirection {
    /// Fire when the reduced value is at or above the threshold.
    Above,
    /// Fire when the reduced value is at or below the threshold.
    Below,
}

/// A run-time-selectable power-off (SCRAM) criterion — closed enum, no `dyn`.
#[derive(Debug, Clone, PartialEq)]
pub enum PowerOffCriterion {
    /// Fire once the simulation time reaches `trigger_time`
    /// (port of `powerOffCriterionModels::timer`).
    Timer {
        /// The instant at which power is turned off.
        trigger_time: Time,
    },
    /// Fire when a monitored scalar field's extremum crosses a threshold
    /// (port of `powerOffCriterionModels::fieldValue`).
    FieldValue {
        /// Whether to test the field maximum or minimum.
        reduction: FieldReduction,
        /// Whether the trip is on crossing above or below the threshold.
        direction: ThresholdDirection,
        /// The threshold value, in the monitored field's own units.
        threshold: f64,
        /// Optional confirmation delay: the criterion must stay satisfied from
        /// first crossing until `time0 + time_delay` before it fires. Zero ⇒ no
        /// delay.
        time_delay: Time,
        /// Latch: the time of first crossing, set on the first satisfied check.
        first_crossing: Option<Time>,
    },
}

impl PowerOffCriterion {
    /// A timer criterion firing at `trigger_time`.
    #[must_use]
    pub fn timer(trigger_time: Time) -> Self {
        Self::Timer { trigger_time }
    }

    /// A field-value criterion (no confirmation delay).
    #[must_use]
    pub fn field_value(
        reduction: FieldReduction,
        direction: ThresholdDirection,
        threshold: f64,
    ) -> Self {
        Self::FieldValue {
            reduction,
            direction,
            threshold,
            time_delay: Time::new::<uom::si::time::second>(0.0),
            first_crossing: None,
        }
    }

    /// Attach a confirmation delay to a [`PowerOffCriterion::FieldValue`]
    /// (no-op on a timer).
    #[must_use]
    pub fn with_time_delay(mut self, delay: Time) -> Self {
        if let Self::FieldValue { time_delay, .. } = &mut self {
            *time_delay = delay;
        }
        self
    }

    /// Evaluate the criterion at simulation time `t`.
    ///
    /// - [`PowerOffCriterion::Timer`]: fires when `t >= trigger_time`
    ///   (`field_extremum` ignored).
    /// - [`PowerOffCriterion::FieldValue`]: reduce the monitored field to its
    ///   max/min *externally* and pass it as `field_extremum` (in the field's
    ///   own units); the criterion tests it against the threshold and, if a
    ///   `time_delay` is set, only fires once the delay has elapsed since the
    ///   first crossing.
    ///
    /// Faithful port of `timer::powerOffCriterion` and
    /// `fieldValue::powerOffCriterion`.
    pub fn check(&mut self, t: Time, field_extremum: f64) -> bool {
        use uom::si::time::second;
        match self {
            Self::Timer { trigger_time } => t.get::<second>() >= trigger_time.get::<second>(),
            Self::FieldValue {
                direction,
                threshold,
                time_delay,
                first_crossing,
                ..
            } => {
                let crossed = match direction {
                    ThresholdDirection::Above => field_extremum >= *threshold,
                    ThresholdDirection::Below => field_extremum <= *threshold,
                };
                if !crossed {
                    return false;
                }
                // Latch the first crossing time, then honour the delay window.
                let t0 = first_crossing.get_or_insert(t);
                t.get::<second>() >= t0.get::<second>() + time_delay.get::<second>()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::second;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    #[test]
    fn timer_fires_at_or_after_trigger() {
        let mut c = PowerOffCriterion::timer(s(10.0));
        assert!(!c.check(s(9.99), 0.0));
        assert!(c.check(s(10.0), 0.0));
        assert!(c.check(s(11.0), 0.0));
    }

    #[test]
    fn field_value_above_threshold_no_delay() {
        let mut c =
            PowerOffCriterion::field_value(FieldReduction::Max, ThresholdDirection::Above, 1500.0);
        assert!(!c.check(s(1.0), 1400.0));
        assert!(c.check(s(2.0), 1600.0));
    }

    #[test]
    fn field_value_below_threshold() {
        let mut c =
            PowerOffCriterion::field_value(FieldReduction::Min, ThresholdDirection::Below, 300.0);
        assert!(!c.check(s(1.0), 350.0));
        assert!(c.check(s(2.0), 250.0));
    }

    /// V&V — confirmation delay measured from first crossing.
    ///
    /// Methodology: an over-temperature trip at 1500 K with a 5 s confirmation
    /// delay. The field first exceeds the threshold at t = 2 s; the trip must
    /// not fire until t = 7 s (= 2 + 5), even though the field stays above the
    /// threshold throughout.
    ///
    /// Result (2026-07-15): no fire at t = 2, 6.9 s; fires at t = 7 s. Pass.
    #[test]
    fn field_value_honours_confirmation_delay() {
        let mut c =
            PowerOffCriterion::field_value(FieldReduction::Max, ThresholdDirection::Above, 1500.0)
                .with_time_delay(s(5.0));
        assert!(!c.check(s(2.0), 1600.0)); // first crossing latched at t=2
        assert!(!c.check(s(6.9), 1600.0)); // still within the 5 s window
        assert!(c.check(s(7.0), 1600.0)); // 2 + 5 s elapsed ⇒ fire
    }
}
