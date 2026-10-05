//! Step 6: parameter extraction, choose a branch (gh:#571).
//!
//! (B) multiphysics, the default, routes on to Step 7. (A) the reactivity map:
//! a LOW-FIDELITY, neutronics-only surrogate, a Chebyshev fit of `ρ(T)` over
//! Step 5's saved runs (`dhoby_ghaut::workbench::reactivity_map`), with its
//! held-out error beside the Monte Carlo σ, a state-point planner that queues
//! the runs it needs on Step 5's own run machinery, and a plain-TOML export.
//!
//! The fit is a few microseconds of least squares, redone only when the runs
//! or the settings change; every Monte Carlo run goes to the physics thread.

use dhoby_ghaut::workbench::reactivity_map::{
    plan_temperatures, rho_pcm, runs_needed, sigma_rho_pcm, AxisBasis, AxisName, Provenance, ReactivityMap,
};
use dhoby_ghaut::workbench::recipe::{now_rfc3339, Branch};
use dhoby_ghaut::workbench::steps::WizardStep;
use egui::{Color32, RichText};
use egui_plot::{Line, Plot, PlotPoints, Points};

use crate::app::App;
use crate::engine::{KeffJob, Req};

const AMBER: Color32 = Color32::from_rgb(170, 90, 0);

/// Step 6's live state.
#[derive(Default)]
pub struct BranchUi {
    /// The last fit and what it was fitted to (run count, settings).
    pub fit: Option<(usize, String, Result<ReactivityMap, String>)>,
    /// Temperatures still to run in the planned sweep \[K\].
    pub queue: Vec<f64>,
    /// The temperature the recipe had before the sweep, restored after it.
    pub restore_t: Option<f64>,
    /// Where the map TOML was last saved.
    pub saved_to: Option<String>,
}

/// The map over the current runs, refitted only when they or the settings
/// changed.
pub fn current_map(app: &mut App) -> Result<ReactivityMap, String> {
    let key = format!("{:?}", app.recipe.branch.map);
    let n = app.recipe.monte_carlo.runs.len();
    let stale = app.branch.fit.as_ref().is_none_or(|(m, k, _)| *m != n || *k != key);
    if stale {
        let fit = ReactivityMap::fit(&app.recipe.monte_carlo.runs, app.recipe.branch.map).map(|mut m| {
            m.provenance = Provenance::from_recipe(&app.recipe, &now_rfc3339(), false);
            m
        });
        app.branch.fit = Some((n, key, fit.map_err(|e| e.to_string())));
    }
    match &app.branch.fit {
        Some((_, _, r)) => r.clone(),
        None => Err("no fit".into()),
    }
}

/// Send the next queued run to the physics thread, at its temperature, with
/// Step 5's settings. Rods stay at Step 5's insertion (gh:#580).
fn start_next(app: &mut App) {
    if app.mc.running {
        return;
    }
    let Some(t) = (!app.branch.queue.is_empty()).then(|| app.branch.queue.remove(0)) else {
        if let Some(t) = app.branch.restore_t.take() {
            app.recipe.nuclear_data.temperature_k = t;
            app.refresh_edited();
            app.say("State-point sweep finished; the recipe's temperature is restored", false);
        }
        return;
    };
    app.recipe.nuclear_data.temperature_k = t;
    let mc = &app.recipe.monte_carlo;
    let label = format!("Run {} (map, {t:.0} K)", mc.runs.len() + 1);
    let job = KeffJob {
        live: app.mc.live.clone(),
        label: label.clone(),
        particles: mc.particles,
        inactive: mc.inactive,
        active: mc.active,
        seed: mc.seed,
        threads: mc.threads,
        temperature_k: t,
        bins_per_decade: mc.spectrum_bins_per_decade,
        tapes: crate::engine::tape_source(std::path::Path::new(&app.recipe.nuclear_data.endf_dir)),
    };
    app.mc.running = true;
    app.mc.items.clear();
    app.phys.send(Req::RunKeff(job));
    app.say(format!("{label} started ({} more queued)", app.branch.queue.len()), false);
}

/// Called after every finished run: the sweep's next run, if one is queued.
pub fn after_run(app: &mut App) {
    if !app.branch.queue.is_empty() || app.branch.restore_t.is_some() {
        start_next(app);
    }
}

/// Called on an engine error: the sweep stops (the runs made so far stay).
pub fn on_error(app: &mut App) {
    if !app.branch.queue.is_empty() {
        app.branch.queue.clear();
    }
    if let Some(t) = app.branch.restore_t.take() {
        app.recipe.nuclear_data.temperature_k = t;
    }
}

/// Temperatures the planner proposes, skipping temperatures already run.
fn plan(app: &App) -> Vec<f64> {
    let b = &app.recipe.branch;
    let existing: Vec<f64> = app
        .recipe
        .monte_carlo
        .runs
        .iter()
        .filter(|r| r.rod_insertion == app.recipe.monte_carlo.rod_insertion)
        .map(|r| r.temperature_k)
        .collect();
    plan_temperatures(b.plan_lo_k, b.plan_hi_k, b.plan_points, b.map.temperature_basis, &existing, 1.0)
}

/// The settings panel.
pub fn settings(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new("Branch").strong());
    let b = &mut app.recipe.branch.branch;
    ui.radio_value(b, Branch::Multiphysics, "(B) Multiphysics (default)");
    ui.small("Monte Carlo -> multigroup cross sections -> coupled neutronics, thermal-hydraulics and structural solvers (Steps 7–10).");
    ui.radio_value(b, Branch::ReactivityMap, "(A) Reactivity map");
    ui.small("A low-fidelity, neutronics-only surrogate: ρ(state) fitted over Monte Carlo runs, exported as plain TOML for DOVER and the digital-twin simulators.");
    ui.separator();
    if app.recipe.branch.branch == Branch::Multiphysics {
        if ui.button("Continue to Step 7: meshing »").clicked() {
            app.step = WizardStep::Meshing;
        }
        return;
    }
    ui.colored_label(AMBER, "LOW FIDELITY: neutronics only. No thermal-hydraulics, no feedback model, no kinetics; interpolation only inside the runs' range.");
    ui.separator();
    ui.label(RichText::new("Fit (chosen before fitting)").strong());
    let m = &mut app.recipe.branch.map;
    ui.horizontal(|ui| {
        ui.label("Order (total Chebyshev degree)");
        ui.add(egui::DragValue::new(&mut m.order).range(0..=6));
    });
    egui::ComboBox::from_label("Temperature enters as")
        .selected_text(m.temperature_basis.label())
        .show_ui(ui, |ui| {
            for b in AxisBasis::ALL {
                ui.selectable_value(&mut m.temperature_basis, b, b.label());
            }
        });
    ui.small("√T is the default: to first order, Doppler-broadened resonance absorption grows like √T, so ρ linear in √T is the expected leading term.");
    ui.label(format!(
        "This order needs at least {} runs at distinct temperatures (one more than its terms, so a run can be held out).",
        runs_needed(m.order, 1)
    ));
    ui.separator();
    ui.label(RichText::new("State axes").strong());
    ui.label("Temperature [K]: isothermal, every material at the run's one data temperature.");
    ui.colored_label(AMBER, "Fuel and moderator temperatures are one axis here: separating them is gh:#590.");
    ui.colored_label(AMBER, "Rod insertion: fixed at the runs' value; the planner sweeps temperature only (all ten rods move together, gh:#580).");
    ui.separator();
    ui.label(RichText::new("State-point planner").strong());
    let b = &mut app.recipe.branch;
    ui.horizontal(|ui| {
        ui.label("From");
        ui.add(egui::DragValue::new(&mut b.plan_lo_k).range(1.0..=3000.0).suffix(" K"));
        ui.label("to");
        ui.add(egui::DragValue::new(&mut b.plan_hi_k).range(1.0..=3000.0).suffix(" K"));
    });
    ui.horizontal(|ui| {
        ui.label("Points");
        ui.add(egui::DragValue::new(&mut b.plan_points).range(2..=12));
    });
    let planned = plan(app);
    ui.small("Chebyshev–Lobatto nodes in the fitting variable, endpoints included; a temperature already run (±1 K) is skipped.");
    ui.label(if planned.is_empty() {
        "Every planned temperature has a run.".to_string()
    } else {
        format!(
            "To run: {} K",
            planned.iter().map(|t| format!("{t:.0}")).collect::<Vec<_>>().join(", ")
        )
    });
    let mc = &app.recipe.monte_carlo;
    ui.small(format!(
        "Each run uses Step 5's statistics ({} × [{} + {}], seed {}). Every new temperature reprocesses the nuclear data (minutes per temperature).",
        mc.particles, mc.inactive, mc.active, mc.seed
    ));
    let ready = app.core.is_some() && app.recipe.review.passed();
    let sweeping = !app.branch.queue.is_empty() || app.branch.restore_t.is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(ready && !sweeping && !app.mc.running && !planned.is_empty(), egui::Button::new(format!("Queue {} runs", planned.len())))
            .clicked()
        {
            app.branch.restore_t = Some(app.recipe.nuclear_data.temperature_k);
            app.branch.queue = planned.clone();
            start_next(app);
        }
        if sweeping && ui.button("Stop after this run").clicked() {
            app.branch.queue.clear();
        }
    });
    if !ready {
        ui.colored_label(AMBER, "Assemble the geometry and pass the review gate first (Steps 1–4, gate).");
    }
    ui.separator();
    ui.label(RichText::new("Export (plain TOML)").strong());
    let map = current_map(app);
    if ui.add_enabled(map.is_ok(), egui::Button::new("Save map as...")).clicked() {
        crate::step11::pick(app, crate::step11::ExportPick::MapToml);
    }
    if let Some(p) = &app.branch.saved_to {
        ui.small(format!("Saved: {p}"));
    }
    ui.small("Provenance tables: recipe hash, ENDF library, every run with k ± σ, fit order, held-out error.");
    if ui.button("Continue to Step 11: post-processing »").clicked() {
        app.step = WizardStep::PostProcessing;
    }
}

/// Write the map to `path`.
pub fn save_map(app: &mut App, path: &std::path::Path) {
    let text = current_map(app).and_then(|mut m| {
        m.provenance.created = now_rfc3339();
        m.to_toml().map_err(|e| e.to_string())
    });
    match text {
        Ok(t) => match std::fs::write(path, t) {
            Ok(()) => {
                app.branch.saved_to = Some(path.display().to_string());
                app.say(format!("Saved the reactivity map {}", path.display()), false);
            }
            Err(e) => app.say(format!("Could not save {}: {e}", path.display()), true),
        },
        Err(e) => app.say(e, true),
    }
}

/// The main view.
pub fn main_view(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::central_panel(ui.style()).inner_margin(16.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        if app.recipe.branch.branch == Branch::Multiphysics {
            ui.heading("(B) Multiphysics: the default branch");
            ui.label("Monte Carlo at state points -> multigroup cross sections per region (Step 8) on three meshes (Step 7) -> the coupled OUTRAM-Foam and farrer-park case (Steps 9–10).");
            ui.label("Choose (A) in the panel for a reactivity map instead: quicker, and low fidelity.");
            return;
        }
        ui.heading("(A) Reactivity map");
        ui.colored_label(AMBER, RichText::new("LOW FIDELITY: a neutronics-only surrogate. A Chebyshev fit of ρ over eigenvalue runs; no thermal-hydraulics, no feedback model, no kinetics, no spatial shape.").strong());
        if !app.branch.queue.is_empty() || (app.mc.running && app.branch.restore_t.is_some()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "State-point sweep: running at {:.0} K, {} more queued. Step 5 shows the console.",
                    app.recipe.nuclear_data.temperature_k,
                    app.branch.queue.len()
                ));
            });
        }
        let map = current_map(app);
        let runs = app.recipe.monte_carlo.runs.clone();
        match &map {
            Err(e) => {
                ui.colored_label(AMBER, format!("No map yet: {e}"));
                ui.label("Plan and queue the runs it needs in the panel.");
            }
            Ok(m) => {
                let f = &m.fit;
                let ho = f.held_out_rms_pcm.map_or("n/a".to_string(), |x| format!("{x:.0} pcm"));
                ui.label(
                    RichText::new(format!(
                        "Held-out error (leave-one-out RMS): {ho}   beside   Monte Carlo σ_ρ (RMS): {:.0} pcm",
                        f.mc_sigma_rms_pcm
                    ))
                    .size(crate::app::fs(17.0))
                    .strong(),
                );
                ui.small(format!(
                    "Order {}, {} terms over {} runs; largest held-out miss {}; in-sample χ²/dof {} (not the number to quote: it cannot see over-fitting). A held-out RMS near σ means the order carries the shape; well above it means it does not.",
                    f.order,
                    f.n_terms,
                    f.n_runs,
                    f.held_out_max_pcm.map_or("n/a".into(), |x| format!("{x:.0} pcm")),
                    f.chi2_per_dof.map_or("n/a".into(), |x| format!("{x:.2}"))
                ));
                if f.n_runs < f.n_terms + 3 {
                    ui.colored_label(
                        AMBER,
                        format!(
                            "Only {} runs for {} terms: the held-out error rests on very few refits and can land far from σ by chance. Plan more runs before quoting it.",
                            f.n_runs, f.n_terms
                        ),
                    );
                }
            }
        }
        let h = (ui.available_height() * 0.55).max(200.0);
        Plot::new("rho_map")
            .height(h)
            .x_axis_label("temperature [K]")
            .y_axis_label("ρ [pcm]")
            .legend(egui_plot::Legend::default())
            .show(ui, |p| {
                let pts: Vec<[f64; 2]> = runs.iter().map(|r| [r.temperature_k, rho_pcm(r.k)]).collect();
                for r in &runs {
                    let (y, s) = (rho_pcm(r.k), sigma_rho_pcm(r.k, r.sigma));
                    p.line(
                        Line::new("MC ± σ", PlotPoints::from(vec![[r.temperature_k, y - s], [r.temperature_k, y + s]]))
                            .color(Color32::from_rgb(40, 90, 160)),
                    );
                }
                p.points(Points::new("runs (Step 5)", PlotPoints::from(pts)).radius(4.0).color(Color32::from_rgb(40, 90, 160)));
                if let Ok(m) = &map {
                    if let Some(a) = m.axes.iter().find(|a| a.name == AxisName::TemperatureK) {
                        let curve: Vec<[f64; 2]> = (0..=100)
                            .filter_map(|i| {
                                let t = a.lo + (a.hi - a.lo) * f64::from(i) / 100.0;
                                m.rho_pcm_at(&[(AxisName::TemperatureK, t)]).ok().map(|v| [t, v])
                            })
                            .collect();
                        p.line(Line::new("Chebyshev map", PlotPoints::from(curve)).color(AMBER));
                    }
                    let loo: Vec<[f64; 2]> = m
                        .held_out
                        .iter()
                        .zip(&runs)
                        .filter_map(|(h, r)| h.predicted_pcm.map(|v| [r.temperature_k, v]))
                        .collect();
                    p.points(
                        Points::new("held-out prediction", PlotPoints::from(loo))
                            .radius(4.0)
                            .shape(egui_plot::MarkerShape::Cross)
                            .color(Color32::from_rgb(190, 30, 30)),
                    );
                }
            });
        if let Ok(m) = &map {
            egui::ScrollArea::vertical().id_salt("map_table").show(ui, |ui| {
                egui::Grid::new("map_runs").striped(true).show(ui, |ui| {
                    for h in ["run", "T [K]", "rods", "ρ ± σ [pcm]", "held-out prediction [pcm]", "miss [pcm]", "miss / σ"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (h, r) in m.held_out.iter().zip(&runs) {
                        ui.label(&h.label);
                        ui.label(format!("{:.1}", r.temperature_k));
                        ui.label(format!("{:.2}", r.rod_insertion));
                        ui.label(format!("{:.0} ± {:.0}", h.rho_pcm, h.sigma_rho_pcm));
                        ui.label(h.predicted_pcm.map_or("n/a".into(), |v| format!("{v:.0}")));
                        ui.label(h.error_pcm().map_or("n/a".into(), |v| format!("{v:+.0}")));
                        ui.label(h.error_pcm().map_or("n/a".into(), |v| format!("{:.1}", v.abs() / h.sigma_rho_pcm)));
                        ui.end_row();
                    }
                });
            });
        }
    });
}
