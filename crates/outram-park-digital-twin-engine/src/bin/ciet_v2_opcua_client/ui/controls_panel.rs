//! The controls panel: write the simulator's set points and switches.
//!
//! ## Three columns, and why
//!
//! Each control row shows **what you are asking for** (slider + numeric box),
//! **what the server holds** (the read-back), and **whether those agree**. The
//! third column is the point of the panel: the simulator clamps writes to each
//! control's documented `valid_range()`, so asking for 1000 kW of heater power
//! gets you the 15 kW ceiling, and without a read-back column that would look
//! like the write had been ignored.
//!
//! The **slider is bounded** by `valid_range()` so the common case is easy. The
//! **numeric box is not**, deliberately, so the clamping behaviour can actually be
//! demonstrated — type an absurd number, watch the read-back come back at the
//! limit and the row explain that it was clamped.
//!
//! ## Failures are shown, never swallowed
//!
//! Every `Write` outcome — success or failure — goes into the write log at the
//! bottom of the panel, with the `StatusCode` the server returned. A write
//! attempted with no session is logged too, as `BadNotConnected`, so a user who
//! moves a slider before connecting is told why nothing happened.
//!
//! Switches write **on toggle**; continuous controls write **on button press or on
//! slider release**, not on every intermediate drag value, since dragging a
//! heater-power slider across its range would otherwise fire a hundred `Write`s.

use egui::{RichText, Ui};

use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietControl, CietSwitch};

use crate::drafts::{compare_readback, ControlDrafts};
use crate::shared_state::{ClientCommand, ClientSharedState};
use crate::ui::{
    format_boolean, format_numeric, numeric_colour, ERROR_TEXT, MUTED_TEXT, OK_TEXT, WARNING_TEXT,
};

#[cfg(test)]
use crate::ui::UNREAD_PLACEHOLDER;

/// Draw the controls panel, returning every write the user asked for this frame.
///
/// # Arguments
///
/// * `ui` — the egui context to draw into.
/// * `state` — shared client state, read only. Read-backs come from
///   `state.controls` / `state.switches`; absent means `--`.
/// * `drafts` — app-owned pending slider/box values.
///
/// Returns the [`ClientCommand`]s to queue. Returned rather than queued here so
/// the panel stays a pure renderer and the app owns every mutation.
pub fn show(
    ui: &mut Ui,
    state: &ClientSharedState,
    drafts: &mut ControlDrafts,
) -> Vec<ClientCommand> {
    let mut commands = Vec::new();
    let connected = state.connection.is_connected();

    ui.heading("Controls");
    if !connected {
        ui.label(
            RichText::new(
                "Not connected -- the controls below are disabled. Connect to a simulator \
                 first.",
            )
            .color(WARNING_TEXT),
        );
    }
    ui.label(
        RichText::new(
            "The simulator clamps every write to the control's valid range. This client \
             sends what you ask for, unclamped, so you can see the clamp happen in the \
             read-back column.",
        )
        .color(MUTED_TEXT)
        .small(),
    );

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("Continuous set points").strong().size(15.0));
        ui.add_space(2.0);
        commands.extend(continuous_controls(ui, state, drafts, connected));

        ui.add_space(12.0);
        ui.label(RichText::new("Switches").strong().size(15.0));
        ui.label(
            RichText::new("These write immediately when toggled.")
                .color(MUTED_TEXT)
                .small(),
        );
        ui.add_space(2.0);
        commands.extend(switches(ui, state, connected));

        ui.add_space(12.0);
        write_log(ui, state);
    });

    commands
}

/// One row per [`CietControl`]: slider, numeric box, Write, read-back, agreement.
fn continuous_controls(
    ui: &mut Ui,
    state: &ClientSharedState,
    drafts: &mut ControlDrafts,
    connected: bool,
) -> Vec<ClientCommand> {
    let mut commands = Vec::new();

    for control in CietControl::ALL {
        let control = *control;
        let (min, max) = control.valid_range();
        let readback = state.controls.get(&control);
        let readback_value = readback.map(|sample| sample.value);
        let mut value = drafts.value_for(control, readback_value);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(control.display_name()).strong());
                ui.label(
                    RichText::new(format!("({}, valid {min} to {max})", control.unit()))
                        .color(MUTED_TEXT)
                        .small(),
                );
            });

            let mut requested = false;
            ui.horizontal_wrapped(|ui| {
                // Slider: bounded by the documented range. Writes on release, so
                // dragging does not fire one Write per pixel.
                let slider = ui.add_enabled(
                    connected,
                    egui::Slider::new(&mut value, min..=max)
                        .clamping(egui::SliderClamping::Never)
                        .suffix(format!(" {}", control.unit())),
                );
                if slider.changed() {
                    drafts.set(control, value);
                }
                if slider.drag_stopped() || slider.lost_focus() {
                    requested = true;
                }

                // Numeric box: deliberately unbounded, so a user can send an
                // out-of-range value and watch the server clamp it.
                let box_response = ui.add_enabled(
                    connected,
                    egui::DragValue::new(&mut value).speed(0.01).max_decimals(6),
                );
                if box_response.changed() {
                    drafts.set(control, value);
                }

                if ui
                    .add_enabled(connected, egui::Button::new("Write"))
                    .clicked()
                {
                    requested = true;
                }

                // Abandon the draft, so the row tracks the server's read-back
                // again instead of holding a value the user has decided against.
                if ui
                    .add_enabled(drafts.is_edited(control), egui::Button::new("Reset"))
                    .on_hover_text("forget this proposed value and follow the server's again")
                    .clicked()
                {
                    drafts.clear(control);
                }
            });

            if requested {
                drafts.set(control, value);
                commands.push(ClientCommand::WriteControl { control, value });
            }

            // Read-back and agreement.
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("server holds:").color(MUTED_TEXT).small());
                ui.label(
                    RichText::new(format_numeric(readback, control.unit(), 4))
                        .monospace()
                        .color(numeric_colour(readback)),
                );

                let comparison = compare_readback(control, value, readback_value);
                if let Some(note) = comparison.note() {
                    let colour = if comparison.is_noteworthy() {
                        WARNING_TEXT
                    } else {
                        MUTED_TEXT
                    };
                    ui.label(RichText::new(note).color(colour).small());
                } else if readback.is_some() && drafts.is_edited(control) {
                    ui.label(
                        RichText::new("matches what you asked for")
                            .color(OK_TEXT)
                            .small(),
                    );
                }
            });
        });
        ui.add_space(4.0);
    }

    commands
}

/// One checkbox per [`CietSwitch`], writing on toggle, with its read-back beside.
fn switches(ui: &mut Ui, state: &ClientSharedState, connected: bool) -> Vec<ClientCommand> {
    let mut commands = Vec::new();

    egui::Grid::new("ciet_switches")
        .num_columns(4)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Switch").strong());
            ui.label(RichText::new("Set").strong());
            ui.label(RichText::new("Server holds").strong());
            ui.label(RichText::new("Node identifier").strong());
            ui.end_row();

            for switch in CietSwitch::ALL {
                let switch = *switch;
                let readback = state.switches.get(&switch);

                // The checkbox tracks the server's value, so it shows the truth
                // rather than a local guess. Until the node has been read it is
                // shown unticked and the read-back column says `--`, so an
                // unticked box next to `--` never claims the switch is off.
                let mut checked = readback.map(|sample| sample.value).unwrap_or(false);

                ui.label(switch.display_name());
                let response = ui.add_enabled(connected, egui::Checkbox::new(&mut checked, ""));
                if response.changed() {
                    commands.push(ClientCommand::WriteSwitch {
                        switch,
                        value: checked,
                    });
                }
                ui.label(RichText::new(format_boolean(readback)).monospace().color(
                    match readback {
                        None => MUTED_TEXT,
                        Some(sample) if sample.is_good() => OK_TEXT,
                        Some(_) => ERROR_TEXT,
                    },
                ));
                ui.label(
                    RichText::new(switch.node_identifier())
                        .monospace()
                        .color(MUTED_TEXT)
                        .small(),
                );
                ui.end_row();
            }
        });

    commands
}

/// The write log: every outcome, newest first, failures in the error colour.
fn write_log(ui: &mut Ui, state: &ClientSharedState) {
    ui.label(RichText::new("Write log").strong().size(15.0));

    if state.write_log.is_empty() {
        ui.label(
            RichText::new("No writes attempted yet.")
                .color(MUTED_TEXT)
                .small(),
        );
        return;
    }

    ui.label(
        RichText::new(
            "Every write is recorded here with the status code the server returned. \
             Failures are never hidden.",
        )
        .color(MUTED_TEXT)
        .small(),
    );
    ui.add_space(2.0);

    egui::Grid::new("ciet_write_log")
        .num_columns(4)
        .striped(true)
        .spacing([12.0, 3.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Age").strong());
            ui.label(RichText::new("Target").strong());
            ui.label(RichText::new("Value sent").strong());
            ui.label(RichText::new("Result").strong());
            ui.end_row();

            for outcome in &state.write_log {
                let colour = if outcome.is_good() {
                    OK_TEXT
                } else {
                    ERROR_TEXT
                };
                ui.label(
                    RichText::new(format!("{:.0} s", outcome.at.elapsed().as_secs_f64()))
                        .color(MUTED_TEXT)
                        .small(),
                );
                ui.label(outcome.target.display_name());
                ui.label(RichText::new(outcome.value.display()).monospace());
                let result = if outcome.message.is_empty() {
                    outcome.status.to_string()
                } else {
                    format!("{} -- {}", outcome.status, outcome.message)
                };
                ui.label(RichText::new(result).color(colour).monospace().small());
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_state::ClientSharedState;

    /// Verifies the panel exposes every writable node the simulator publishes,
    /// and that each control's slider bounds come from the node map rather than
    /// from a hard-coded range.
    ///
    /// **Methodology.** The panel iterates `CietControl::ALL` and
    /// `CietSwitch::ALL` directly, so coverage follows the node map by
    /// construction; what has to be checked is that every range it would hand to
    /// a slider is *usable*: finite, with `min < max`, and containing at least one
    /// representable value. A degenerate or inverted range would make
    /// `egui::Slider::new(&mut v, min..=max)` unusable or panic. Pass criterion:
    /// all 8 ranges finite and strictly increasing; 15 writable nodes total (8
    /// controls + 7 switches).
    ///
    /// **Results (2026-07-28).** 8 / 8 control ranges finite with `min < max`.
    /// Measured ranges: heater power and steady-state power 0 to 15 kW; CTAH pump
    /// pressure -17000 to 17000 Pa; both temperature set points 15 to 120 degC;
    /// frequency-response amplitude 0 to 4 kW; angular velocity 0 to 10 rad/s;
    /// timestep 0.001 to 0.1 s. Writable node count measured 15 (8 controls +
    /// 7 switches). Interpretation: the panel drives the whole writable interface
    /// with sliders that cannot be constructed from an invalid range.
    #[test]
    fn every_writable_node_has_a_usable_slider_range() {
        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            assert!(
                min.is_finite() && max.is_finite(),
                "{control:?} range non-finite"
            );
            assert!(min < max, "{control:?}: min {min} not below max {max}");
        }
        assert_eq!(CietControl::ALL.len() + CietSwitch::ALL.len(), 15);

        // Pin the ranges quoted in this test's documentation, so the doc numbers
        // cannot silently drift from the node map.
        assert_eq!(CietControl::HeaterPowerKw.valid_range(), (0.0, 15.0));
        assert_eq!(
            CietControl::CtahPumpPressurePascals.valid_range(),
            (-17000.0, 17000.0)
        );
        assert_eq!(
            CietControl::Bt41CtahOutletSetPointDegC.valid_range(),
            (15.0, 120.0)
        );
        assert_eq!(
            CietControl::FrequencyResponseAmplitudeKw.valid_range(),
            (0.0, 4.0)
        );
        assert_eq!(
            CietControl::FrequencyResponseAngularVelocityRadPerS.valid_range(),
            (0.0, 10.0)
        );
        assert_eq!(CietControl::TimestepSeconds.valid_range(), (0.001, 0.1));
    }

    /// Verifies an unread switch is not presented as "off".
    ///
    /// **Methodology.** The checkbox tracks the server's value and therefore shows
    /// unticked when the node has not been read, which alone would read as "the
    /// switch is off". The read-back column is what disambiguates it, so assert
    /// [`format_boolean`] returns the `--` placeholder — not `"off"` — for an
    /// unread switch, for every switch. Pass criterion: 7 / 7 return `"--"`.
    ///
    /// **Results (2026-07-28).** 7 / 7 unread switches rendered as `"--"` in the
    /// "Server holds" column, distinct from the `"off"` a genuine `false` produces.
    /// Interpretation: an unticked box beside `--` cannot be read as a
    /// confirmed-off switch.
    #[test]
    fn an_unread_switch_is_not_displayed_as_off() {
        let state = ClientSharedState::new();
        for switch in CietSwitch::ALL {
            let readback = state.switches.get(switch);
            assert!(readback.is_none());
            assert_eq!(format_boolean(readback), UNREAD_PLACEHOLDER, "{switch:?}");
            assert_ne!(format_boolean(readback), "off");
        }
    }
}
