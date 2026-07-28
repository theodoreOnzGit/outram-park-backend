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

use outram_park_digital_twin_engine::app_scaffold::{PanelSet, SharedState};

use crate::app::schematic::{draw_schematic, SchematicTracers};
use crate::app::state::{HtgrPlotData, HtgrSnapshot};

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

/// Control side-panel: user inputs (reactivity, helium flow) written straight
/// back into the shared physics state via
/// [`SharedState::update`](outram_park_digital_twin_engine::app_scaffold::SharedState::update).
pub fn draw_controls(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
    ui.heading("Controls");
    ui.separator();

    let mut reactivity = snapshot.external_reactivity_dollars;
    let mut helium_flow = snapshot.helium_flow_setpoint_kg_per_s;

    ui.label("External reactivity");
    let rho_changed = ui
        .add(egui::Slider::new(&mut reactivity, -2.0..=1.0).text("$ (rho/beta)"))
        .changed();

    ui.add_space(8.0);
    ui.label("Helium circulator flow");
    let flow_changed = ui
        .add(egui::Slider::new(&mut helium_flow, 10.0..=150.0).text("kg/s"))
        .changed();

    if rho_changed || flow_changed {
        physics.update(|s| {
            s.external_reactivity_dollars = reactivity;
            s.helium_flow_setpoint_kg_per_s = helium_flow;
        });
    }

    ui.add_space(12.0);
    ui.separator();
    ui.label(format!(
        "Reactivity margin: {:+.3} $",
        snapshot.reactivity_margin_dollars
    ));
    ui.label(format!(
        "Reactor power: {:.1} MWth",
        snapshot.reactor_power_mw
    ));
    ui.label(format!(
        "Turbine power: {:.1} MWe",
        snapshot.turbine_power_mw
    ));
    ui.label(format!("Sim time: {:.1} s", snapshot.sim_time_s));

    ui.add_space(12.0);
    ui.separator();
    ui.small(
        "Scaffold only -- placeholder HTGR correlations, delayed-neutron layer \
         stubbed pending teh_o_prke::DelayedNeutronLayer. Not validated; not for \
         any operational or safety use.",
    );
}

/// Schematic panel body.
pub fn draw_schematic_panel(ui: &mut Ui, snapshot: &HtgrSnapshot, tracers: &SchematicTracers) {
    ui.heading("HTGR (helium-cooled, graphite-moderated prismatic-block) -- demonstration model");
    ui.separator();
    draw_schematic(ui, snapshot, tracers);
}

/// Time-history plots panel body.
pub fn draw_plots_panel(ui: &mut Ui, plots: &HtgrPlotData) {
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
    ui.heading("Temperatures vs time");
    Plot::new("htgr_temperature_plot")
        .legend(Legend::default())
        .height(240.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                "Fuel temp [K]",
                PlotPoints::from(plots.fuel_temperature_k.clone()),
            ));
            plot_ui.line(Line::new(
                "Core outlet He [K]",
                PlotPoints::from(plots.core_outlet_temp_k.clone()),
            ));
        });
}

/// Numeric diagnostics panel body.
pub fn draw_diagnostics_panel(ui: &mut Ui, s: &HtgrSnapshot) {
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
            row(
                ui,
                "Fuel temperature",
                format!("{:.1} K", s.fuel_temperature_k),
            );
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
            row(
                ui,
                "Core inlet temp",
                format!("{:.1} K", s.core_inlet_temp_k),
            );
            row(
                ui,
                "Core outlet temp",
                format!("{:.1} K", s.core_outlet_temp_k),
            );
            row(
                ui,
                "Helium mass flow",
                format!("{:.1} kg/s", s.helium_mass_flow_kg_per_s),
            );
            row(ui, "IHX duty", format!("{:.2} MW", s.ihx_duty_mw));
            row(
                ui,
                "IHX helium outlet",
                format!("{:.1} K", s.ihx_outlet_temp_k),
            );
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
            row(
                ui,
                "SG steam outlet temp",
                format!("{:.1} K", s.sg_steam_outlet_temp_k),
            );
            row(ui, "Turbine power", format!("{:.2} MW", s.turbine_power_mw));
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
            row(
                ui,
                "Cooling-water outlet",
                format!("{:.1} K", s.cooling_water_outlet_temp_k),
            );
        });
}
