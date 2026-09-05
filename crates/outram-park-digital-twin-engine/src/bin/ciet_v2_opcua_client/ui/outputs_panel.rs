//! The live-outputs panel: every [`CietSignal`] in a grid, grouped by kind.
//!
//! The grouping is a [`SignalGroup`] derived from the signal variant by an
//! exhaustive `match`, so a signal added to the node map cannot appear ungrouped —
//! it becomes a compile error here until it is placed. That is deliberate: a
//! silently-appended "other" bucket would let a new temperature end up filed under
//! timing diagnostics without anyone noticing.
//!
//! Groups are ordered the way an operator reads a loop: temperatures first
//! (the instrumented BT thermocouples, which is what CIET is actually
//! instrumented for), then flowrates, then the controller outputs driving the two
//! air-side heat exchangers, then the timing diagnostics that say whether the
//! simulation is keeping up with real time.

use egui::{RichText, Ui};

use outram_park_digital_twin_engine::ciet_opcua::node_map::CietSignal;

use crate::shared_state::ClientSharedState;
use crate::ui::{awaiting_data_note, format_numeric, numeric_colour, MUTED_TEXT};

/// The kinds of quantity the output grid separates.
///
/// An enum, matched exhaustively by [`group_of`], so every published signal has
/// exactly one home and a new one must be given one explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGroup {
    /// Bulk fluid temperatures at the instrumented stations, and the two mixing
    /// nodes. Unit: degC.
    Temperatures,
    /// Branch and loop mass flowrates. Unit: kg/s. Signed where the branch can
    /// reverse.
    Flowrates,
    /// Heater power and the two PID-commanded air-side heat transfer
    /// coefficients — what the controllers are asking the plant to do.
    ControllerOutputs,
    /// Simulated time, wall-clock time and per-timestep cost: the diagnostics
    /// that say whether the solver is keeping up.
    Timing,
}

impl SignalGroup {
    /// Every group, in display order.
    pub const ALL: &'static [SignalGroup] = &[
        Self::Temperatures,
        Self::Flowrates,
        Self::ControllerOutputs,
        Self::Timing,
    ];

    /// Heading shown above the group's grid.
    pub fn heading(&self) -> &'static str {
        match self {
            Self::Temperatures => "Temperatures",
            Self::Flowrates => "Flowrates",
            Self::ControllerOutputs => "Heater power and controller outputs",
            Self::Timing => "Timing diagnostics",
        }
    }

    /// One-line explanation of what the group is for.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Temperatures => {
                "Bulk fluid temperatures at the instrumented BT stations, plus the two \
                 mixing nodes. Degrees Celsius."
            }
            Self::Flowrates => {
                "Mass flowrates. Kilograms per second, signed -- a negative CTAH-branch \
                 flow is reverse flow."
            }
            Self::ControllerOutputs => {
                "What the heater and the two PID controllers are commanding. The heater \
                 power shown is the power actually applied, which differs from the set \
                 point when the over-temperature killswitch has tripped."
            }
            Self::Timing => {
                "Compare simulated time with wall-clock time to see whether the simulation \
                 is keeping up with real time."
            }
        }
    }

    /// Digits after the decimal point for values in this group.
    ///
    /// Flowrates get four because CIET's natural-circulation flows are of order
    /// 0.01-0.1 kg/s, where two decimals would quantise the reading away.
    pub fn decimals(&self) -> usize {
        match self {
            Self::Temperatures => 2,
            Self::Flowrates => 4,
            Self::ControllerOutputs => 3,
            Self::Timing => 2,
        }
    }
}

/// Which group a signal belongs to.
///
/// Exhaustive by construction — adding a [`CietSignal`] variant breaks this
/// `match` until it is placed in a group.
pub fn group_of(signal: CietSignal) -> SignalGroup {
    match signal {
        CietSignal::Bt11HeaterInletDegC
        | CietSignal::Bt12HeaterOutletDegC
        | CietSignal::Bt43CtahInletDegC
        | CietSignal::Bt41CtahOutletDegC
        | CietSignal::Bt60DhxTubeInletDegC
        | CietSignal::Bt21DhxTubeOutletDegC
        | CietSignal::Bt21DhxShellInletDegC
        | CietSignal::Bt27DhxShellOutletDegC
        | CietSignal::Bt65TchxInletDegC
        | CietSignal::Bt66TchxOutletDegC
        | CietSignal::TopMixingNodeDegC
        | CietSignal::BottomMixingNodeDegC => SignalGroup::Temperatures,

        CietSignal::Fm40CtahBranchKgPerS
        | CietSignal::Fm20DhxBranchKgPerS
        | CietSignal::Fm60DracsKgPerS => SignalGroup::Flowrates,

        CietSignal::HeaterPowerKw
        | CietSignal::CtahHtcWattPerM2K
        | CietSignal::TchxHtcWattPerM2K => SignalGroup::ControllerOutputs,

        CietSignal::SimulationTimeSeconds
        | CietSignal::ElapsedTimeSeconds
        | CietSignal::CalcTimeMs => SignalGroup::Timing,
    }
}

/// Every signal in a group, in node-map order.
pub fn signals_in(group: SignalGroup) -> Vec<CietSignal> {
    CietSignal::ALL
        .iter()
        .copied()
        .filter(|signal| group_of(*signal) == group)
        .collect()
}

/// Draw the live-outputs panel.
///
/// # Arguments
///
/// * `ui` — the egui context to draw into.
/// * `state` — shared client state, read only. Signals absent from it render as
///   `--`.
pub fn show(ui: &mut Ui, state: &ClientSharedState) {
    ui.heading("Live outputs");
    if state.signals.is_empty() {
        awaiting_data_note(ui, state);
        ui.add_space(8.0);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for group in SignalGroup::ALL {
            ui.add_space(6.0);
            ui.label(RichText::new(group.heading()).strong().size(15.0));
            ui.label(RichText::new(group.description()).color(MUTED_TEXT).small());
            ui.add_space(2.0);

            egui::Grid::new(format!("ciet_outputs_{:?}", group))
                .num_columns(4)
                .striped(true)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Quantity").strong());
                    ui.label(RichText::new("Value").strong());
                    ui.label(RichText::new("Age").strong());
                    ui.label(RichText::new("Node identifier").strong());
                    ui.end_row();

                    for signal in signals_in(*group) {
                        let sample = state.signals.get(&signal);

                        ui.label(signal.display_name());
                        ui.label(
                            RichText::new(format_numeric(sample, signal.unit(), group.decimals()))
                                .monospace()
                                .color(numeric_colour(sample)),
                        );
                        ui.label(
                            RichText::new(match sample {
                                None => crate::ui::UNREAD_PLACEHOLDER.to_string(),
                                Some(sample) => {
                                    format!("{:.1} s", sample.received_at.elapsed().as_secs_f64())
                                }
                            })
                            .color(MUTED_TEXT)
                            .small(),
                        );
                        ui.label(
                            RichText::new(signal.node_identifier())
                                .monospace()
                                .color(MUTED_TEXT)
                                .small(),
                        );
                        ui.end_row();
                    }
                });
        }

        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "{} of {} outputs read so far. \"{}\" means this client has not yet received \
                 a value for that node -- it is never a stand-in for a number.",
                state.signals.len(),
                CietSignal::ALL.len(),
                crate::ui::UNREAD_PLACEHOLDER
            ))
            .color(MUTED_TEXT)
            .small(),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies every published signal is grouped exactly once, so the grid shows
    /// the whole interface and nothing twice.
    ///
    /// **Methodology.** Partition `CietSignal::ALL` with [`signals_in`] over
    /// [`SignalGroup::ALL`] and check the parts sum to the whole with no overlap.
    /// The reference is the node map's own signal list, so a signal added there
    /// and left ungrouped fails this test as well as breaking [`group_of`]'s
    /// `match`. Pass criterion: total across groups equals
    /// `CietSignal::ALL.len()`, and no signal appears in two groups.
    ///
    /// **Results (2026-07-28).** 21 signals partitioned as 12 temperatures +
    /// 3 flowrates + 3 controller outputs + 3 timing = 21, with no duplicates.
    /// Interpretation: a user reading the grid is seeing every output the
    /// simulator publishes.
    #[test]
    fn every_signal_is_grouped_exactly_once() {
        let mut total = 0;
        let mut seen: Vec<CietSignal> = Vec::new();
        for group in SignalGroup::ALL {
            let members = signals_in(*group);
            total += members.len();
            for signal in members {
                assert!(!seen.contains(&signal), "{signal:?} in two groups");
                seen.push(signal);
            }
        }
        assert_eq!(total, CietSignal::ALL.len());
        assert_eq!(seen.len(), CietSignal::ALL.len());

        assert_eq!(signals_in(SignalGroup::Temperatures).len(), 12);
        assert_eq!(signals_in(SignalGroup::Flowrates).len(), 3);
        assert_eq!(signals_in(SignalGroup::ControllerOutputs).len(), 3);
        assert_eq!(signals_in(SignalGroup::Timing).len(), 3);
    }

    /// Verifies each group's unit is consistent across its members, and that the
    /// displayed precision suits the magnitudes involved.
    ///
    /// **Methodology.** Every temperature in the grid must carry the unit `degC`
    /// and every flowrate `kg/s`, taken from the node map's own `unit()` — a group
    /// that mixed units would make its column unreadable. Then check the
    /// flowrate group's precision: CIET natural-circulation flows are of order
    /// 0.01-0.1 kg/s, so the group needs at least 3 decimals to resolve them.
    /// Pass criterion: uniform units within the temperature and flowrate groups,
    /// and `Flowrates::decimals() >= 3`.
    ///
    /// **Results (2026-07-28).** All 12 temperature signals reported unit
    /// `degC`; all 3 flowrate signals reported `kg/s`. `Flowrates::decimals()`
    /// measured 4, resolving 1e-4 kg/s, i.e. about 0.1-1 % of a typical CIET
    /// natural-circulation flowrate. Interpretation: a natural-circulation flow
    /// is legible rather than quantised to two decimals.
    #[test]
    fn group_units_are_uniform_and_precision_suits_the_magnitudes() {
        for signal in signals_in(SignalGroup::Temperatures) {
            assert_eq!(signal.unit(), "degC", "{signal:?}");
        }
        for signal in signals_in(SignalGroup::Flowrates) {
            assert_eq!(signal.unit(), "kg/s", "{signal:?}");
        }
        assert!(SignalGroup::Flowrates.decimals() >= 3);
    }

    /// Verifies the group headings and descriptions are non-empty and distinct,
    /// so the four sections cannot be confused with each other.
    ///
    /// **Methodology.** Collect each group's heading; assert non-empty and
    /// pairwise distinct; assert each description is non-empty. Pass criterion: 4
    /// distinct non-empty headings, 4 non-empty descriptions.
    ///
    /// **Results (2026-07-28).** 4 / 4 headings non-empty and distinct
    /// ("Temperatures", "Flowrates", "Heater power and controller outputs",
    /// "Timing diagnostics"); 4 / 4 descriptions non-empty.
    #[test]
    fn group_headings_are_present_and_distinct() {
        let mut headings: Vec<&str> = Vec::new();
        for group in SignalGroup::ALL {
            let heading = group.heading();
            assert!(!heading.is_empty());
            assert!(!headings.contains(&heading), "duplicate heading {heading}");
            headings.push(heading);
            assert!(!group.description().is_empty());
        }
        assert_eq!(headings.len(), 4);
    }
}
