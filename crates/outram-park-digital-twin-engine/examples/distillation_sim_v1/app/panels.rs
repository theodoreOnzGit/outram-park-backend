//! Panel layout: operator controls, and the schematic/plots/diagnostics tabs.
//! Mirrors `htgr_sim_v1::app::panels`'s free-function-per-panel structure.

use egui::Ui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use outram_park_digital_twin_engine::app_scaffold::{PanelSet, SharedState};

use super::state::{ColumnPlotData, ColumnSnapshot};
use crate::physics::column_config;

/// Which central-panel tab is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    /// The process schematic.
    Schematic,
    /// Time-series plots.
    Plots,
    /// Per-stage tabular readouts.
    Diagnostics,
}

impl PanelSet for Panel {
    const ALL: &'static [Self] = &[Self::Schematic, Self::Plots, Self::Diagnostics];

    fn label(&self) -> &'static str {
        match self {
            Self::Schematic => "Schematic",
            Self::Plots => "Plots",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

/// Valid reflux-ratio range offered by the slider \[-\]. Must stay `> 0` --
/// [`crate::physics::DistillationPlant::step`] panics on a non-positive
/// value, so the slider bound is the enforcement point, matching
/// `htgr_sim_v1`'s convention of pulling ranges from the physics layer
/// rather than letting a widget offer a value the model would reject.
const REFLUX_RATIO_RANGE: std::ops::RangeInclusive<f64> = 0.5..=6.0;
/// Valid reboiler-duty range offered by the slider \[W\]. Same enforcement
/// reasoning as [`REFLUX_RATIO_RANGE`].
const REBOILER_DUTY_RANGE_W: std::ops::RangeInclusive<f64> = 1_000.0..=200_000.0;

/// The right-side operator control panel: reflux ratio and reboiler duty.
/// Both write back to the shared snapshot only on `.changed()`, exactly like
/// `htgr_sim_v1::app::panels::draw_controls`'s slider pattern.
pub fn draw_controls(
    ui: &mut Ui,
    physics: &SharedState<ColumnSnapshot>,
    snapshot: &ColumnSnapshot,
) {
    ui.heading("Operator controls");
    ui.separator();

    let mut reflux_ratio = snapshot.reflux_ratio;
    let reflux_changed = ui
        .add(
            egui::Slider::new(&mut reflux_ratio, REFLUX_RATIO_RANGE)
                .text("Reflux ratio R = L0/D [-]"),
        )
        .changed();

    let mut reboiler_duty = snapshot.reboiler_duty_watts;
    let duty_changed = ui
        .add(egui::Slider::new(&mut reboiler_duty, REBOILER_DUTY_RANGE_W).text("Reboiler duty [W]"))
        .changed();

    if reflux_changed || duty_changed {
        physics.update(|s| {
            s.reflux_ratio = reflux_ratio;
            s.reboiler_duty_watts = reboiler_duty;
        });
    }

    ui.separator();
    ui.label(format!("Simulated time: {:.0} s", snapshot.sim_time_s));
    ui.label(format!(
        "Distillate: {:.4} mol/s ({:.2}% benzene)",
        snapshot.distillate_mol_s,
        100.0
            * snapshot
                .liquid_benzene_fraction
                .first()
                .copied()
                .unwrap_or(0.0)
    ));
    ui.label(format!(
        "Bottoms: {:.4} mol/s ({:.2}% benzene)",
        snapshot.bottoms_mol_s,
        100.0
            * snapshot
                .liquid_benzene_fraction
                .last()
                .copied()
                .unwrap_or(0.0)
    ));
}

/// Delegates to [`super::schematic::draw_schematic`].
pub fn draw_schematic_panel(ui: &mut Ui, snapshot: &ColumnSnapshot) {
    ui.heading("Column schematic");
    super::schematic::draw_schematic(ui, snapshot);
}

/// Time-series plots: purities, temperatures, reboiler duty.
pub fn draw_plots_panel(ui: &mut Ui, plots: &ColumnPlotData) {
    ui.heading("Time series");

    Plot::new("dist_purity_plot")
        .legend(Legend::default())
        .height(220.0)
        .include_y(0.0)
        .include_y(1.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                "Distillate benzene fraction",
                PlotPoints::from(plots.distillate_purity.clone()),
            ));
            plot_ui.line(Line::new(
                "Bottoms benzene fraction",
                PlotPoints::from(plots.bottoms_purity.clone()),
            ));
        });

    Plot::new("dist_temperature_plot")
        .legend(Legend::default())
        .height(220.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                "Condenser stage T [K]",
                PlotPoints::from(plots.condenser_temperature_k.clone()),
            ));
            plot_ui.line(Line::new(
                "Reboiler stage T [K]",
                PlotPoints::from(plots.reboiler_temperature_k.clone()),
            ));
        });

    Plot::new("dist_duty_plot")
        .legend(Legend::default())
        .height(180.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(
                "Reboiler duty [W]",
                PlotPoints::from(plots.reboiler_duty_watts.clone()),
            ));
        });
}

/// Per-stage tabular readouts -- naturally a **grid with a stage-number
/// column**, unlike `htgr_sim_v1`'s single-vessel diagnostics grid, because
/// this plant has `N` equilibrium stages rather than one lumped core.
pub fn draw_diagnostics_panel(ui: &mut Ui, snapshot: &ColumnSnapshot) {
    ui.heading("Per-stage profile");
    ui.label(format!(
        "Stage 0 = total condenser, stage {} = reboiler",
        column_config::N_STAGES - 1
    ));
    ui.separator();

    egui::Grid::new("dist_stage_grid")
        .num_columns(6)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Stage");
            ui.strong("T [K]");
            ui.strong("x benzene [-]");
            ui.strong("y benzene [-]");
            ui.strong("L [mol/s]");
            ui.strong("V [mol/s]");
            ui.end_row();

            for j in 0..snapshot.n_stages {
                ui.label(format!("{j}"));
                ui.label(format!(
                    "{:.2}",
                    snapshot.stage_temperature_k.get(j).copied().unwrap_or(0.0)
                ));
                ui.label(format!(
                    "{:.4}",
                    snapshot
                        .liquid_benzene_fraction
                        .get(j)
                        .copied()
                        .unwrap_or(0.0)
                ));
                ui.label(format!(
                    "{:.4}",
                    snapshot
                        .vapor_benzene_fraction
                        .get(j)
                        .copied()
                        .unwrap_or(0.0)
                ));
                ui.label(format!(
                    "{:.4}",
                    snapshot.liquid_flow_mol_s.get(j).copied().unwrap_or(0.0)
                ));
                ui.label(format!(
                    "{:.4}",
                    snapshot.vapor_flow_mol_s.get(j).copied().unwrap_or(0.0)
                ));
                ui.end_row();
            }
        });

    ui.separator();
    ui.label(format!("Reflux ratio: {:.3}", snapshot.reflux_ratio));
    ui.label(format!(
        "Reboiler duty: {:.1} W",
        snapshot.reboiler_duty_watts
    ));
    ui.label(format!(
        "Distillate: {:.4} mol/s",
        snapshot.distillate_mol_s
    ));
    ui.label(format!("Bottoms: {:.4} mol/s", snapshot.bottoms_mol_s));
}
