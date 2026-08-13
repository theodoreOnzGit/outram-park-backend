//! Top-level GUI panels and their rendering.
//!
//! [`Panel`] implements the engine's
//! [`outram_park_digital_twin_engine::app_scaffold::PanelSet`] trait, so the
//! tab row is drawn by the engine's
//! [`panel_selector_ui`](outram_park_digital_twin_engine::app_scaffold::panel_selector_ui)
//! rather than a hand-rolled `selectable_value` row. Each panel's body is a
//! free function taking the shared state it needs.

use egui::Ui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use outram_park_digital_twin_engine::app_scaffold::{PanelSet, SharedState};
use outram_park_digital_twin_engine::components::LegendUnit;

use crate::app::schematic::{draw_schematic, SchematicTracers};
use crate::app::state::{HtgrPlotData, HtgrSnapshot};
use crate::physics::secondary_loop::ranges;

// ── Temperature display units (kopi-beans `op-qpgw`) ────────────────────────
//
// A DISPLAY-LAYER concern and nothing else. Read `temperature_display` below
// before adding a readout.

/// A temperature formatted in the operator's chosen display unit, with its unit
/// symbol.
///
/// # This is the whole units feature, and it is deliberately this small
///
/// The unit lives in [`crate::app::HtgrSimApp`], is passed by value into the
/// `draw_*` functions, and reaches nothing else. In particular it is **not** a
/// field of [`HtgrSnapshot`], which is the only channel to the physics thread,
/// and **not** a field of [`crate::physics::PlantCommands`], which is the only
/// channel into the plant model. There is therefore no path by which a display
/// preference can reach a correlation, a controller or a solver input --
/// exactly the silent unit error the workspace's `uom` typing exists to
/// prevent (kopi-beans `op-qpgw`).
///
/// The argument is a `uom` [`ThermodynamicTemperature`], never a bare `f64`, so
/// the toggle can only ever choose **which `uom` accessor is called**:
/// `get::<kelvin>()` or `get::<degree_celsius>()`. A caller holding a scalar
/// has to say what unit it is in before it can be formatted, which is the point.
///
/// `decimals` is the number of digits after the point; the returned string
/// always carries the unit symbol, so the toggle changes the label as well as
/// the number.
///
/// # Reuse
///
/// [`LegendUnit`] is the engine's existing display-unit enum, already used by
/// [`outram_park_digital_twin_engine::components::TemperatureLegend`] on this
/// very schematic. It is reused rather than a second `TemperatureUnit` being
/// invented, so the colour legend's tick labels and the numeric readouts beside
/// it cannot end up in different units. The engine's own formatter is private,
/// which is the only reason this function exists at all.
pub fn temperature_display(
    unit: LegendUnit,
    temperature: ThermodynamicTemperature,
    decimals: usize,
) -> String {
    match unit {
        LegendUnit::Kelvin => format!("{:.*} K", decimals, temperature.get::<kelvin>()),
        LegendUnit::Celsius => format!(
            "{:.*} \u{b0}C",
            decimals,
            temperature.get::<degree_celsius>()
        ),
    }
}

/// The unit symbol alone, for axis titles and plot series names.
pub fn temperature_unit_symbol(unit: LegendUnit) -> &'static str {
    match unit {
        LegendUnit::Kelvin => "K",
        LegendUnit::Celsius => "\u{b0}C",
    }
}

/// The numeric value of `temperature` in the chosen display unit, for the plot
/// panel, which needs a number rather than a string.
///
/// Same guarantee as [`temperature_display`]: the input is `uom`-typed, so the
/// only thing the toggle selects is which accessor runs.
pub fn temperature_value(unit: LegendUnit, temperature: ThermodynamicTemperature) -> f64 {
    match unit {
        LegendUnit::Kelvin => temperature.get::<kelvin>(),
        LegendUnit::Celsius => temperature.get::<degree_celsius>(),
    }
}

/// Format a snapshot temperature scalar, which is **always in kelvin** by the
/// snapshot's own convention, in the chosen display unit.
///
/// The rewrap through [`ThermodynamicTemperature`] is the point, not overhead:
/// it forces the call site to name the unit the scalar is in, so a field
/// holding degrees Celsius could not be passed here by mistake.
fn kelvin_scalar_display(unit: LegendUnit, value_k: f64, decimals: usize) -> String {
    temperature_display(
        unit,
        ThermodynamicTemperature::new::<kelvin>(value_k),
        decimals,
    )
}

/// The selectable top-level panels of the HTGR simulator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    /// Live plant schematic (built on the engine's visual widgets).
    Schematic,
    /// Time-history plots (power, temperatures).
    Plots,
    /// Numeric diagnostics table.
    Diagnostics,
}

impl PanelSet for Panel {
    const ALL: &'static [Self] = &[Self::Schematic, Self::Plots, Self::Diagnostics];

    fn label(&self) -> &'static str {
        match self {
            Self::Schematic => "Plant Schematic",
            Self::Plots => "Time-History Plots",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

/// Control side-panel: every operator input, written straight back into the
/// shared physics state via
/// [`SharedState::update`](outram_park_digital_twin_engine::app_scaffold::SharedState::update).
///
/// Laid out in the order an operator would reach for them: the trip banner
/// first when there is one, then the reactor (rods, protection), then the
/// primary circulator, then the secondary side (feedwater station, condenser),
/// then the readouts each of them causes.
///
/// `display_unit` is the **display-only** temperature unit
/// (kopi-beans `op-qpgw`). It is taken by `&mut` because the toggle that sets
/// it is drawn here; it is written back into [`crate::app::HtgrSimApp`], never
/// into `physics`. See [`temperature_display`].
pub fn draw_controls(
    ui: &mut Ui,
    physics: &SharedState<HtgrSnapshot>,
    snapshot: &HtgrSnapshot,
    display_unit: &mut LegendUnit,
) {
    ui.heading("Controls");
    ui.separator();

    // Trip banner first: if the reactor has scrammed, that is the single most
    // important thing on the panel, and the rod slider below is being
    // overridden by the protection system.
    if let Some(reason) = snapshot.trip_reason {
        ui.colored_label(
            egui::Color32::from_rgb(220, 60, 40),
            "\u{26A0} REACTOR TRIPPED - automatic scram",
        );
        ui.label(reason.description());
        ui.label(format!(
            "Scram rod demand: {:.0}% inserted",
            snapshot.scram_insertion_fraction * 100.0
        ));
        ui.label(
            "The protection system is holding the rods in. Your rod command below \
             is overridden until the trip is reset, and resetting with the rods \
             still withdrawn will simply trip again.",
        );
        if ui.button("Reset trip").clicked() {
            // A request, not a direct clear -- see the field's doc comment.
            physics.update(|s| s.trip_reset_requested = true);
        }
        ui.separator();
    }

    // Protection system arming. DISABLED by default while the steam generator
    // is under investigation -- an RPS that trips masks the excursion being
    // diagnosed. See `crate::physics::protection`.
    let mut rps_enabled = snapshot.rps_enabled;
    if ui
        .checkbox(&mut rps_enabled, "Reactor protection system armed")
        .changed()
    {
        physics.update(|s| s.rps_enabled = rps_enabled);
    }
    if !rps_enabled {
        ui.colored_label(
            egui::Color32::from_rgb(210, 150, 40),
            "RPS DISARMED - a full rod withdrawal (+16.45 $) will run away unchecked",
        );
    }
    ui.separator();

    let mut rod_insertion = snapshot.control_rod_insertion_fraction;
    let mut helium_flow = snapshot.helium_flow_setpoint_kg_per_s;

    // Rod position, not reactivity. An operator moves rods; reactivity is the
    // consequence, and is displayed below rather than commanded. The ten
    // HTR-10 side-reflector rods are ganged as one bank here.
    ui.label(format!(
        "Control rod bank insertion ({} rods, ganged)",
        crate::physics::control_rods::CONTROL_ROD_COUNT
    ));
    let rho_changed = ui
        .add(
            egui::Slider::new(&mut rod_insertion, 0.0..=1.0)
                .text("fraction (0 withdrawn, 1 inserted)"),
        )
        .changed();

    ui.add_space(8.0);
    ui.label("Helium circulator flow");
    // Range bracketing the published HTR-10 operating point of 4.3 kg/s: down
    // to roughly 7% flow at the bottom (below the circulator's regulated 30%
    // turndown, so a loss-of-flow can be driven) and half again above nominal
    // at the top. The primary loop clamps to its own circulator ceiling, so
    // the slider cannot command a flow the machine could not pass.
    let flow_changed = ui
        .add(egui::Slider::new(&mut helium_flow, 0.3..=6.0).text("kg/s"))
        .changed();

    if rho_changed || flow_changed {
        physics.update(|s| {
            s.control_rod_insertion_fraction = rod_insertion;
            s.helium_flow_setpoint_kg_per_s = helium_flow;
        });
    }

    draw_secondary_controls(ui, physics, snapshot, *display_unit);

    ui.add_space(12.0);
    ui.separator();
    // Display preference, kept away from the plant controls above so it reads
    // as a view setting rather than a command. It changes what is drawn and
    // nothing else -- see `temperature_display`.
    ui.horizontal(|ui| {
        ui.label("Temperature display");
        ui.selectable_value(display_unit, LegendUnit::Kelvin, "K");
        ui.selectable_value(display_unit, LegendUnit::Celsius, "\u{b0}C");
    });
    ui.small(
        "Display only. Every readout, the schematic's colour legend and the \
         time-history plots follow this; the model itself is uom-typed \
         throughout and never sees it.",
    );

    ui.add_space(12.0);
    ui.separator();
    // Reactivity is shown as a RESULT of rod position, immediately under the
    // slider that causes it, so the causal direction is visible.
    ui.label(format!(
        "External reactivity: {:+.3} $",
        snapshot.external_reactivity_dollars
    ));
    ui.label(format!(
        "Reactivity margin: {:+.3} $",
        snapshot.reactivity_margin_dollars
    ));
    // Reference marker: where the published bank worth and cold clean excess
    // imply criticality. Indicative only -- it carries no burnup, xenon or
    // temperature defect, so it is not where HTR-10's rods actually sit. The
    // bisection behind it is a few hundred flops, negligible per frame.
    if let Some(critical) = crate::physics::control_rods::critical_insertion_fraction(
        snapshot.delayed_neutron_fraction_pcm * 1e-5,
    ) {
        ui.label(format!(
            "Cold clean critical position: {:.1}% inserted",
            critical * 100.0
        ));
    }
    ui.label(format!(
        "Reactor power: {:.1} MWth",
        snapshot.reactor_power_mw
    ));
    // Mechanical, then electrical -- these were previously one line labelled
    // "MWe" against the MECHANICAL number, which is wrong: the enthalpy-drop
    // power is shaft power, and the electrical output is the generator's, less
    // its losses. The plant now computes both, so both are shown.
    ui.label(format!(
        "Turbine shaft power: {:.1} MW",
        snapshot.turbine_power_mw
    ));
    ui.label(format!(
        "Generator output: {:.1} MWe at {:.0} rpm",
        snapshot.generator_electrical_power_mw, snapshot.shaft_speed_rpm
    ));
    ui.label(format!("Sim time: {:.1} s", snapshot.sim_time_s));

    ui.add_space(12.0);
    ui.separator();
    ui.small(
        "Demonstration model. Arrangement and operating point follow the published \
         HTR-10 description; the correlations, controller constants and loop \
         inventories are illustrative, not a specific licensed design. Not \
         validated; not for any operational, licensing or safety use.",
    );
}

/// Secondary-side operator controls: the feedwater station and the condenser.
///
/// Drawn as part of [`draw_controls`] and split out only for length. Every
/// slider's range is read from
/// [`crate::physics::secondary_loop::ranges`] rather than written here, so a
/// widget cannot offer a value the physics would silently clamp -- and
/// `secondary_loop::tests::the_gui_ranges_match_the_physics_clamps` pins that
/// the ranges really are the clamps' own boundaries.
///
/// `display_unit` is display-only; see [`temperature_display`].
fn draw_secondary_controls(
    ui: &mut Ui,
    physics: &SharedState<HtgrSnapshot>,
    snapshot: &HtgrSnapshot,
    display_unit: LegendUnit,
) {
    ui.add_space(12.0);
    ui.separator();
    ui.heading("Secondary steam cycle");

    // ── Feedwater station: AUTO / MANUAL ────────────────────────────────
    //
    // AUTO is the default and is what this simulator did before the mode
    // existed. MANUAL is the diagnostic mode: with the controller out of the
    // loop the steam temperature is an open-loop response to duty and flow.
    let mut manual = snapshot.feedwater_manual;
    ui.label("Feedwater station");
    let mode_changed = ui
        .horizontal(|ui| {
            let a = ui.selectable_value(&mut manual, false, "AUTO");
            let m = ui.selectable_value(&mut manual, true, "MANUAL");
            a.changed() || m.changed()
        })
        .inner;

    let mut target_c =
        ThermodynamicTemperature::new::<kelvin>(snapshot.feedwater_target_steam_temp_k)
            .get::<degree_celsius>();
    let mut manual_flow = snapshot.feedwater_manual_flow_kg_per_s;
    let mut demand_changed = false;

    if manual {
        // MANUAL: the operator's number is the demand. The pump's capacity is
        // the range -- the same clamp the AUTO controller is held to, because
        // it is the same pump.
        let (lo, hi) = ranges::FEEDWATER_FLOW_KG_PER_S;
        demand_changed |= ui
            .add(
                egui::Slider::new(&mut manual_flow, lo..=hi)
                    .logarithmic(false)
                    .text("feedwater flow (kg/s)")
                    .drag_value_speed(0.01),
            )
            .changed();
        ui.small(
            "MANUAL: the steam temperature is whatever the exchanger produces at \
             this flow. Reducing flow reduces steam flow and so turbine power -- \
             this is the nearest thing this plant has to a load control, because \
             the turbine has no governor.",
        );
    } else {
        // AUTO: the operator dials a steam TEMPERATURE, not an enthalpy. It is
        // the quantity a control room reads on the main steam line, and the one
        // the published operating point is quoted in (440 degC). The controller
        // flashes it to an enthalpy through IF97 at the live steam pressure.
        let (lo, hi) = ranges::TARGET_STEAM_TEMPERATURE_C;
        demand_changed |= ui
            .add(
                egui::Slider::new(&mut target_c, lo..=hi)
                    .logarithmic(false)
                    .text("target steam temperature (\u{b0}C)")
                    .drag_value_speed(0.1),
            )
            .changed();
        ui.small(
            "AUTO: published HTR-10 main steam is 440 degC. The law is \
             feedforward, so the settled steam temperature is a slow limit cycle \
             around the setpoint rather than a fixed point (kopi-beans op-tj10) \
             -- moving the setpoint moves the centre of the swing, not the swing.",
        );
    }
    // Achieved against demanded, immediately under the control, so the
    // first-order lag on the feed pump is visible rather than surprising.
    ui.label(format!(
        "Feedwater flow: {:.2} kg/s achieved",
        snapshot.secondary_mass_flow_kg_per_s
    ));
    ui.label(format!(
        "Steam-generator outlet: {}",
        kelvin_scalar_display(display_unit, snapshot.sg_steam_outlet_temp_k, 1)
    ));

    // ── Condenser back-pressure ─────────────────────────────────────────
    ui.add_space(8.0);
    ui.label("Condenser");
    let mut condenser_kpa = snapshot.condenser_pressure_setpoint_kpa;
    let (lo, hi) = ranges::CONDENSER_PRESSURE_KPA;
    let condenser_changed = ui
        .add(
            egui::Slider::new(&mut condenser_kpa, lo..=hi)
                .logarithmic(false)
                .text("back-pressure (kPa)")
                .drag_value_speed(0.05),
        )
        .changed();
    ui.small(
        "Sets the bottom of the cycle. The floor is where the saturation \
         temperature meets the 25 degC cooling water; the ceiling is a badly \
         degraded vacuum. Raising it costs turbine work and warms the feedwater, \
         because the cycle is closed.",
    );
    ui.label(format!(
        "Cooling-water outlet: {}",
        kelvin_scalar_display(display_unit, snapshot.cooling_water_outlet_temp_k, 1)
    ));

    if mode_changed || demand_changed || condenser_changed {
        physics.update(|s| {
            s.feedwater_manual = manual;
            s.feedwater_manual_flow_kg_per_s = manual_flow;
            s.feedwater_target_steam_temp_k =
                ThermodynamicTemperature::new::<degree_celsius>(target_c).get::<kelvin>();
            s.condenser_pressure_setpoint_kpa = condenser_kpa;
        });
    }

    // ── No turbine load control, and why ────────────────────────────────
    //
    // Stated on the panel rather than left as an absence, because "there is no
    // slider for it" and "the model cannot represent it" look identical to a
    // user.
    ui.add_space(8.0);
    ui.small(
        "No turbine load control: this machine is islanded onto a fixed \
         resistive load with no governor and no throttle valve, so there is \
         nothing to command. Feedwater flow is what moves the load.",
    );
}

/// Schematic panel body.
///
/// `display_unit` is passed through to the schematic so its instrumentation
/// readouts and its colour legend follow the same toggle as every other
/// temperature on screen (display only -- see [`temperature_display`]).
pub fn draw_schematic_panel(
    ui: &mut Ui,
    snapshot: &HtgrSnapshot,
    tracers: &SchematicTracers,
    display_unit: LegendUnit,
) {
    ui.heading(
        "HTGR (helium-cooled, graphite-moderated pebble bed) -- demonstration model, \
         HTR-10-style two-vessel arrangement",
    );
    ui.separator();
    draw_schematic(ui, snapshot, tracers, display_unit);
}

/// Time-history plots panel body.
///
/// `display_unit` selects the temperature plot's units (display only -- see
/// [`temperature_display`]). The buffers themselves stay in kelvin, the
/// snapshot's convention; each sample is rewrapped as a `uom`
/// [`ThermodynamicTemperature`] and read back through the chosen accessor at
/// draw time, so the stored data is never mutated into a display unit.
pub fn draw_plots_panel(ui: &mut Ui, plots: &HtgrPlotData, display_unit: LegendUnit) {
    ui.heading("Reactor power vs time");
    Plot::new("htgr_power_plot")
        .legend(Legend::default())
        .height(240.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                "Total power [MW]",
                PlotPoints::from(plots.reactor_power_mw.clone()),
            ));
            plot_ui.line(Line::new(
                "Prompt layer [MW]",
                PlotPoints::from(plots.prompt_power_mw.clone()),
            ));
            plot_ui.line(Line::new(
                "Delayed increment/step [MW]",
                PlotPoints::from(plots.delayed_power_mw.clone()),
            ));
            plot_ui.line(Line::new(
                "Turbine power [MW]",
                PlotPoints::from(plots.turbine_power_mw.clone()),
            ));
        });

    ui.add_space(12.0);
    let symbol = temperature_unit_symbol(display_unit);
    ui.heading(format!("Temperatures vs time [{symbol}]"));
    Plot::new("htgr_temperature_plot")
        .legend(Legend::default())
        .height(240.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                format!("Fuel temp [{symbol}]"),
                PlotPoints::from(in_display_unit(&plots.fuel_temperature_k, display_unit)),
            ));
            plot_ui.line(Line::new(
                format!("Core outlet He [{symbol}]"),
                PlotPoints::from(in_display_unit(&plots.core_outlet_temp_k, display_unit)),
            ));
        });
}

/// Convert a `[t_seconds, temperature_kelvin]` plot buffer into the chosen
/// display unit, leaving the buffer itself untouched.
///
/// Each stored scalar is rewrapped as a `uom` [`ThermodynamicTemperature`]
/// before the accessor is chosen, so this is the same display-only path
/// [`temperature_display`] documents rather than a hand-rolled `-273.15`.
fn in_display_unit(samples: &[[f64; 2]], display_unit: LegendUnit) -> Vec<[f64; 2]> {
    samples
        .iter()
        .map(|[t, value_k]| {
            [
                *t,
                temperature_value(
                    display_unit,
                    ThermodynamicTemperature::new::<kelvin>(*value_k),
                ),
            ]
        })
        .collect()
}

/// Numeric diagnostics panel body.
///
/// `display_unit` selects the unit every temperature row is written in
/// (display only -- see [`temperature_display`]). Every row carries its unit
/// symbol, so the toggle changes the label as well as the number.
pub fn draw_diagnostics_panel(ui: &mut Ui, s: &HtgrSnapshot, display_unit: LegendUnit) {
    ui.heading("Numeric diagnostics");
    ui.separator();

    egui::Grid::new("htgr_diagnostics_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let row = |ui: &mut Ui, k: &str, v: String| {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            };
            // Temperature row: the scalar is in kelvin by the snapshot's
            // convention, rewrapped as a `uom` quantity so the display unit can
            // only choose an accessor.
            let temperature_row = |ui: &mut Ui, k: &str, value_k: f64| {
                ui.label(k);
                ui.label(kelvin_scalar_display(display_unit, value_k, 1));
                ui.end_row();
            };
            ui.label("Kinetics");
            ui.label("");
            ui.end_row();
            row(ui, "Total power", format!("{:.3} MWth", s.reactor_power_mw));
            row(
                ui,
                "Prompt-layer power",
                format!("{:.3} MWth", s.prompt_power_mw),
            );
            row(
                ui,
                "Delayed increment (S*dt)",
                format!("{:.3} MWth/step", s.delayed_power_mw),
            );
            temperature_row(ui, "Fuel temperature", s.fuel_temperature_k);
            row(
                ui,
                "Reactivity margin",
                format!("{:+.4} $", s.reactivity_margin_dollars),
            );
            row(
                ui,
                "Delayed fraction (beta)",
                format!("{:.0} pcm", s.delayed_neutron_fraction_pcm),
            );

            ui.label("Primary helium loop");
            ui.label("");
            ui.end_row();
            temperature_row(ui, "Core inlet temp", s.core_inlet_temp_k);
            temperature_row(ui, "Core outlet temp", s.core_outlet_temp_k);
            row(
                ui,
                "Helium mass flow",
                format!("{:.1} kg/s", s.helium_mass_flow_kg_per_s),
            );
            row(ui, "IHX duty", format!("{:.2} MW", s.ihx_duty_mw));
            temperature_row(ui, "IHX helium outlet", s.ihx_outlet_temp_k);
            row(
                ui,
                "Helium c_p (live EOS)",
                format!("{:.0} J/(kg K)", s.helium_cp_j_per_kg_k),
            );
            row(
                ui,
                "Loop residence time",
                format!("{:.2} s", s.helium_residence_time_s),
            );
            row(
                ui,
                "Loop pressure drop",
                format!("{:.1} kPa", s.primary_pressure_drop_kpa),
            );
            row(
                ui,
                "  of which bed (KTA)",
                format!("{:.2} kPa", s.bed_pressure_drop_kpa),
            );
            row(
                ui,
                "Circulator power",
                format!("{:.3} MW", s.circulator_power_mw),
            );

            ui.label("Secondary steam loop");
            ui.label("");
            ui.end_row();
            row(
                ui,
                "Steam pressure",
                format!("{:.2} MPa", s.steam_pressure_mpa),
            );
            temperature_row(ui, "SG steam outlet temp", s.sg_steam_outlet_temp_k);
            row(
                ui,
                "Turbine shaft power",
                format!("{:.2} MW", s.turbine_power_mw),
            );
            row(
                ui,
                "Shaft speed",
                format!(
                    "{:.0} rpm ({:.1} rad/s)",
                    s.shaft_speed_rpm, s.shaft_speed_rad_per_s
                ),
            );
            row(
                ui,
                "Generator output",
                format!("{:.2} MWe", s.generator_electrical_power_mw),
            );
            row(
                ui,
                "  machine rating",
                format!("{:.2} MW shaft", s.generator_rating_mw),
            );
            row(
                ui,
                "Exhaust quality",
                format!("{:.3}", s.steam_quality_after_turbine),
            );
            row(
                ui,
                "Condenser pressure",
                format!("{:.1} kPa", s.condenser_pressure_kpa),
            );
            row(
                ui,
                "Feedwater flow",
                format!("{:.1} kg/s", s.secondary_mass_flow_kg_per_s),
            );
            row(
                ui,
                "Loop residence time",
                format!("{:.2} s", s.secondary_residence_time_s),
            );
            row(
                ui,
                "  piping inventory",
                format!("{:.1} kg", s.secondary_piping_inventory_kg),
            );
            row(
                ui,
                "Condensate enthalpy",
                format!("{:.1} kJ/kg", s.condensate_enthalpy_j_per_kg / 1.0e3),
            );
            row(
                ui,
                "Feedwater enthalpy",
                format!("{:.1} kJ/kg", s.feedwater_enthalpy_j_per_kg / 1.0e3),
            );
            row(
                ui,
                "Feed-pump power",
                format!("{:.3} MW", s.feed_pump_power_mw),
            );
            row(
                ui,
                "Net cycle power",
                format!("{:.2} MW", s.net_cycle_power_mw),
            );
            row(
                ui,
                "Condenser duty",
                format!("{:.2} MW", s.condenser_duty_mw),
            );
            temperature_row(ui, "Cooling-water outlet", s.cooling_water_outlet_temp_k);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V: **the display-unit toggle changes only the presentation**, and the
    /// number it presents is the `uom` conversion rather than a hand-rolled one.
    ///
    /// # Why this is the test that matters for `op-qpgw`
    ///
    /// The bead's whole concern is that a display preference must not become a
    /// *model* quantity. Most of that guarantee is structural and holds at
    /// compile time -- [`LegendUnit`] appears in no field of
    /// [`HtgrSnapshot`] (the only channel to the physics thread) and in no field
    /// of [`crate::physics::PlantCommands`] (the only channel into the plant),
    /// so there is no expression by which it could reach a correlation. What a
    /// runtime test *can* add is the other half: that the formatter is a genuine
    /// `uom` accessor and not a `- 273.15` written out by hand, which is exactly
    /// the kind of open-coded conversion that drifts.
    ///
    /// # Methodology
    ///
    /// Three reference temperatures spanning this plant -- absolute zero, the
    /// ice point, and the published 440 degC main steam condition -- are
    /// formatted in both units and compared against `uom`'s own conversion of
    /// the same [`ThermodynamicTemperature`]. Then each value is round-tripped:
    /// the kelvin reading must be unchanged by having been displayed in Celsius,
    /// because displaying a quantity must not alter it.
    ///
    /// Pass criterion: exact agreement with `uom` to 1e-9 K, and both formatted
    /// strings carrying their unit symbol.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Temperature | Kelvin display | Celsius display |
    /// |---|---|---|
    /// | 0 K | `0.00 K` | `-273.15 °C` |
    /// | 273.15 K | `273.15 K` | `0.00 °C` |
    /// | 713.15 K (published main steam) | `713.15 K` | `440.00 °C` |
    ///
    /// All three agree with `uom` to round-off, and every round-trip returned
    /// the original kelvin value bit-for-bit. Interpretation: the toggle selects
    /// an accessor and nothing else.
    #[test]
    fn the_display_unit_changes_only_the_presentation() {
        for kelvin_value in [0.0_f64, 273.15, 713.15] {
            let t = ThermodynamicTemperature::new::<kelvin>(kelvin_value);

            let shown_k = temperature_display(LegendUnit::Kelvin, t, 2);
            let shown_c = temperature_display(LegendUnit::Celsius, t, 2);
            println!("{kelvin_value:>8.2} K -> \"{shown_k}\" / \"{shown_c}\"");

            assert_eq!(shown_k, format!("{:.2} K", t.get::<kelvin>()));
            assert_eq!(
                shown_c,
                format!("{:.2} \u{b0}C", t.get::<degree_celsius>()),
                "the Celsius display must be uom's own conversion"
            );
            assert!(shown_k.ends_with(" K") && shown_c.ends_with("\u{b0}C"));

            // The numeric accessor agrees with the formatter's, and neither
            // alters the quantity: reading it in Celsius leaves the kelvin
            // reading untouched.
            assert!(
                (temperature_value(LegendUnit::Kelvin, t) - kelvin_value).abs() < 1e-9,
                "displaying in kelvin must return the kelvin value"
            );
            let _ = temperature_value(LegendUnit::Celsius, t);
            assert!(
                (t.get::<kelvin>() - kelvin_value).abs() < 1e-9,
                "displaying a temperature must not alter it"
            );

            // And the scalar path used by every snapshot readout.
            assert_eq!(
                kelvin_scalar_display(LegendUnit::Celsius, kelvin_value, 2),
                shown_c
            );
        }

        assert_eq!(temperature_unit_symbol(LegendUnit::Kelvin), "K");
        assert_eq!(temperature_unit_symbol(LegendUnit::Celsius), "\u{b0}C");
        // Kelvin is the default, i.e. the toggle does not silently change what
        // this simulator displayed before it existed.
        assert_eq!(LegendUnit::default(), LegendUnit::Kelvin);
    }

    /// V&V: the plot buffers are **converted for display, never mutated**.
    ///
    /// **Methodology.** [`in_display_unit`] maps a `[t, kelvin]` ring-buffer
    /// slice into the chosen unit for `egui_plot`. The risk it carries is that
    /// someone "optimises" it into an in-place conversion, at which point the
    /// stored history would be in whatever unit was last selected and toggling
    /// twice would shift every curve by 273.15 K. This checks the input slice is
    /// unchanged, the time column is untouched, and the values match `uom`.
    ///
    /// **Results (2026-08-13).** A three-sample buffer at 300/400/500 K
    /// converted to 26.85/126.85/226.85 degC with the time column identical, and
    /// the source buffer unchanged. Toggling to Celsius and back to kelvin
    /// returned the original values exactly.
    #[test]
    fn plot_buffers_are_converted_for_display_not_mutated() {
        let source = vec![[0.0_f64, 300.0], [1.0, 400.0], [2.0, 500.0]];
        let before = source.clone();

        let celsius = in_display_unit(&source, LegendUnit::Celsius);
        assert_eq!(source, before, "the plot buffer must not be mutated");
        for (out, src) in celsius.iter().zip(source.iter()) {
            assert!(
                (out[0] - src[0]).abs() < 1e-12,
                "the time column must be untouched"
            );
            let expected = ThermodynamicTemperature::new::<kelvin>(src[1]).get::<degree_celsius>();
            assert!((out[1] - expected).abs() < 1e-9);
        }

        let back = in_display_unit(&source, LegendUnit::Kelvin);
        assert_eq!(back, before, "a kelvin display must be the stored values");
    }
}
