//! Time trends of the headline signals against **simulated** time.
//!
//! ## Two plots, not five, and not one
//!
//! Heater power is in kW and the four temperatures are in degC. Sharing one y axis
//! would either flatten the 0-15 kW power trace against a 20-120 degC temperature
//! scale, or force a dual axis that invites the reader to compare two unrelated
//! quantities by eye. So there are two stacked plots with a shared x axis:
//! temperatures together (they are directly comparable, and the gap between BT-11
//! and BT-12 *is* the rise across the heater), and heater power on its own.
//!
//! ## The x axis is simulated time
//!
//! Not wall-clock time. The simulator can run in fast-forward or slow motion, so a
//! wall-clock abscissa would stretch and squash a transient that is perfectly
//! regular in simulated time. `SimulationTimeSeconds` is read from the server like
//! any other signal, and until it arrives nothing is plotted at all — see
//! [`ClientSharedState::append_trend_points`](crate::shared_state::ClientSharedState::append_trend_points).

use egui::{RichText, Ui};
use egui_plot::{Legend, Line, Plot, PlotPoints};

use outram_park_digital_twin_engine::ciet_opcua::node_map::CietSignal;

use crate::shared_state::{ClientSharedState, TRENDED_SIGNALS, TREND_CAPACITY};
use crate::ui::{awaiting_data_note, MUTED_TEXT};

/// Height of each stacked plot, points.
const PLOT_HEIGHT: f32 = 260.0;

/// The temperature signals drawn on the upper plot, in legend order.
///
/// Filtered out of [`TRENDED_SIGNALS`] by unit rather than hard-listed, so the two
/// plots between them always cover exactly the trended set.
pub fn temperature_trends() -> Vec<CietSignal> {
    TRENDED_SIGNALS
        .iter()
        .copied()
        .filter(|signal| signal.unit() == "degC")
        .collect()
}

/// The power signals drawn on the lower plot, in legend order.
pub fn power_trends() -> Vec<CietSignal> {
    TRENDED_SIGNALS
        .iter()
        .copied()
        .filter(|signal| signal.unit() == "kW")
        .collect()
}

/// Draw the trends panel.
///
/// # Arguments
///
/// * `ui` — the egui context to draw into.
/// * `state` — shared client state, read only. An empty trend draws no line
///   rather than a line at zero.
pub fn show(ui: &mut Ui, state: &ClientSharedState) {
    ui.heading("Trends against simulated time");

    let total_points: usize = state.trends.values().map(|trend| trend.len()).sum();
    if total_points == 0 {
        awaiting_data_note(ui, state);
        ui.label(
            RichText::new(
                "Nothing is plotted until the simulator's own simulation clock has been \
                 read, so a fast-forwarded or slow-motion run is never drawn against \
                 wall-clock time.",
            )
            .color(MUTED_TEXT)
            .small(),
        );
        return;
    }

    ui.label(
        RichText::new(format!(
            "x axis is the simulator's simulation time in seconds, not wall-clock time. \
             Up to {TREND_CAPACITY} points are kept per trend."
        ))
        .color(MUTED_TEXT)
        .small(),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        trend_plot(
            ui,
            state,
            "ciet_trend_temperatures",
            "Temperature / degC",
            &temperature_trends(),
        );

        ui.add_space(10.0);

        trend_plot(
            ui,
            state,
            "ciet_trend_power",
            "Heater power / kW",
            &power_trends(),
        );

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{total_points} points retained across {} trends.",
                state.trends.len()
            ))
            .color(MUTED_TEXT)
            .small(),
        );
    });
}

/// One plot holding one line per signal.
///
/// A signal with no points contributes no line — an empty trend is drawn as
/// absent, not as a flat line at zero, which would be a fabricated reading.
fn trend_plot(
    ui: &mut Ui,
    state: &ClientSharedState,
    plot_id: &str,
    y_axis_label: &str,
    signals: &[CietSignal],
) {
    ui.label(RichText::new(y_axis_label).strong());

    Plot::new(plot_id)
        .height(PLOT_HEIGHT)
        .legend(Legend::default())
        .x_axis_label("simulation time / s")
        .y_axis_label(y_axis_label)
        .show(ui, |plot_ui| {
            for signal in signals {
                let Some(trend) = state.trends.get(signal) else {
                    continue;
                };
                if trend.is_empty() {
                    continue;
                }
                plot_ui.line(
                    Line::new(signal.display_name(), PlotPoints::from(trend.points())).width(1.6),
                );
            }
        });

    let missing: Vec<&str> = signals
        .iter()
        .filter(|signal| {
            state
                .trends
                .get(signal)
                .map(|trend| trend.is_empty())
                .unwrap_or(true)
        })
        .map(|signal| signal.display_name())
        .collect();
    if !missing.is_empty() {
        ui.label(
            RichText::new(format!("not yet received: {}", missing.join(", ")))
                .color(MUTED_TEXT)
                .small(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the two plots between them cover every trended signal, and that
    /// the required five signals are trended.
    ///
    /// **Methodology.** The brief requires trends for heater power, BT-11, BT-12,
    /// BT-41 and BT-66. Assert each is in [`TRENDED_SIGNALS`], then assert the
    /// unit-based split into [`temperature_trends`] and [`power_trends`]
    /// partitions that set exactly — a signal in neither would silently never be
    /// drawn. Pass criterion: 5 required signals present; the two plots' signal
    /// counts sum to `TRENDED_SIGNALS.len()` with no overlap.
    ///
    /// **Results (2026-07-28).** All 5 required signals present in
    /// `TRENDED_SIGNALS` (measured length 5). Split measured as 4 temperature
    /// trends (BT-11, BT-12, BT-41, BT-66, all unit `degC`) + 1 power trend
    /// (heater power, unit `kW`) = 5, no overlap. Interpretation: every trended
    /// signal reaches a plot, and the plots do not mix kW with degC on one axis.
    #[test]
    fn the_two_plots_partition_the_trended_signals() {
        let required = [
            CietSignal::HeaterPowerKw,
            CietSignal::Bt11HeaterInletDegC,
            CietSignal::Bt12HeaterOutletDegC,
            CietSignal::Bt41CtahOutletDegC,
            CietSignal::Bt66TchxOutletDegC,
        ];
        for signal in required {
            assert!(
                TRENDED_SIGNALS.contains(&signal),
                "{signal:?} is required to be trended"
            );
        }

        let temperatures = temperature_trends();
        let powers = power_trends();
        assert_eq!(
            temperatures.len() + powers.len(),
            TRENDED_SIGNALS.len(),
            "a trended signal reaches neither plot"
        );
        for signal in &temperatures {
            assert!(!powers.contains(signal), "{signal:?} on both plots");
        }
        assert_eq!(temperatures.len(), 4);
        assert_eq!(powers.len(), 1);
    }
}
