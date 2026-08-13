//! Advanced heater control: steady power, sinusoidal frequency-response
//! perturbation, and a one-shot step response.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/frequency_response_and_transients.rs`),
//! GPL-3.0, same licence. The sliders, their ranges and the "Start Step
//! Response" behaviour are v1's.
//!
//! **What v2 changed here.** In v1 this page owned the settings in a GUI-local
//! `FreqResponseAndTransientSettings` struct, and `eframe::App::ui` evaluated
//! the signal and pushed it into the heater power **once per repaint**. That
//! cannot work headless (there are no repaints on Termux) and cannot be driven
//! remotely. In v2:
//!
//! - the settings themselves live in shared state, as
//!   [`HeaterControlSettings`] on
//!   [`CietState::heater_control`](outram_park_digital_twin_engine::ciet_opcua::state::CietState::heater_control),
//!   so the GUI and an OPC-UA client write the *same* fields;
//! - the signal is evaluated by the **physics thread**, once per timestep, in
//!   `full_simulation::educational_ciet_loop_version_4`;
//! - [`FreqResponseAndTransientSettings`] shrank to the two genuinely GUI-local
//!   fields that arm the step-response button.
//!
//! Frequency-response testing of CIET's heater is the experimental technique of
//! De Wet and Poresky (Bode plots of heater-outlet temperature against heater
//! power). This page reproduces the *input* side of that experiment. **No
//! validation against their published data has been performed**, in v1 or v2.

use egui::Ui;
use outram_park_digital_twin_engine::ciet_opcua::state::HeaterControlSettings;

use crate::ciet_simulator_v2::CIETApp;

impl CIETApp {
    /// The "Frequency Response and Transients" page.
    ///
    /// Reads [`HeaterControlSettings`] out of shared state, runs the widgets
    /// against a local copy, and writes back only if something changed. Because
    /// the settings live in shared state, the sliders double as a read-back of
    /// whatever an OPC-UA client has written.
    pub fn ciet_sim_transients_and_freq_response_page(&mut self, ui: &mut Ui) {
        // Read the shared settings. `HeaterControlSettings` is `Copy`, so this
        // is a cheap snapshot and the read lock is released immediately.
        let settings_before: HeaterControlSettings = self.ciet_state.read().unwrap().heater_control;
        let mut settings = settings_before;

        ui.checkbox(
            &mut settings.advanced_heater_control_switched_on,
            "Turn on Advanced Heater Control",
        );

        if settings.advanced_heater_control_switched_on {
            ui.heading("Advanced Heater Controls");
            ui.separator();

            ui.label("Steady State Average Power (kW)");
            let heater_set_pt_slider_kw =
                egui::Slider::new(&mut settings.steady_state_power_kw, 0.0..=15.0)
                    .text("Heater Power (kW)")
                    .drag_value_speed(0.001);

            ui.add(heater_set_pt_slider_kw);

            ui.heading("");
            ui.checkbox(
                &mut settings.frequency_response_switched_on,
                "Frequency Response Control",
            );
            ui.label(settings.sine_wave_label());

            ui.label("Sine Wave Amplitude (kW)");
            let total_amplitude_slider_kw =
                egui::Slider::new(&mut settings.total_amplitude_kw, 0.0..=4.0)
                    .text("Total Frequency Response Amplitude(kW)")
                    .drag_value_speed(0.001);

            ui.add(total_amplitude_slider_kw);

            ui.label("Angular Velocity (rad/s)");
            let angular_velocity_slider =
                egui::Slider::new(&mut settings.angular_velocity_rad_per_s, 0.0..=10.0)
                    .text("Angular Velocity Settings")
                    .logarithmic(true)
                    .drag_value_speed(0.001);

            ui.add(angular_velocity_slider);

            ui.heading("");
            ui.checkbox(
                &mut self.frequency_response_settings.step_response_switched_on,
                "Step Response Control",
            );

            let step_response_slider_kw = egui::Slider::new(
                &mut self
                    .frequency_response_settings
                    .user_set_step_response_power_kw,
                -10.0..=10.0,
            )
            .text("Desired Step Response Power (kW)")
            .logarithmic(false)
            .drag_value_speed(0.01);

            ui.add(step_response_slider_kw);

            if ui.add(egui::Button::new("Start Step Response")).clicked() {
                // only change step response if the step response is switched on
                if self.frequency_response_settings.step_response_switched_on {
                    let step_response_power_kw: f64 = self
                        .frequency_response_settings
                        .user_set_step_response_power_kw;

                    // reset the step size so the step is a one-shot
                    self.frequency_response_settings
                        .user_set_step_response_power_kw = 0.0;

                    // the step IS the change in steady-state power -- v1
                    // behaviour, kept exactly
                    settings.steady_state_power_kw += step_response_power_kw;
                }
            }

            ui.label(
                "The signal is applied by the physics thread once per timestep, \
                 so it keeps running with the GUI on any page -- and headless.",
            );
        }

        // Write back only on change, so the GUI does not stamp on an OPC-UA
        // client that is driving the same fields.
        if settings != settings_before {
            self.ciet_state.write().unwrap().heater_control = settings;
        }

        ui.separator();

        self.ciet_sim_heater_page_graph(ui);
    }
}

/// The GUI-local remainder of v1's advanced-heater-control settings.
///
/// Everything a remote client could sensibly drive (the master switch, the
/// steady power in kW, the perturbation amplitude in kW and its angular
/// frequency in rad/s) moved into shared state as
/// [`HeaterControlSettings`]. What is left here is the *button state* of the
/// step-response control, which is a piece of GUI interaction and has no
/// meaning to the physics thread or to an OPC-UA client: pressing the button
/// simply adds the step to the shared `steady_state_power_kw`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreqResponseAndTransientSettings {
    /// Arms the "Start Step Response" button. When `false` the button does
    /// nothing, which is v1's guard against an accidental click.
    pub step_response_switched_on: bool,
    /// Size of the step to add to the steady heater power when the button is
    /// pressed, kW. Valid range -10..=10; reset to zero once applied so the
    /// step is a one-shot rather than a repeated increment.
    pub user_set_step_response_power_kw: f64,
}

impl Default for FreqResponseAndTransientSettings {
    /// Step response disarmed, zero step size.
    fn default() -> Self {
        Self {
            step_response_switched_on: false,
            user_set_step_response_power_kw: 0.0,
        }
    }
}
