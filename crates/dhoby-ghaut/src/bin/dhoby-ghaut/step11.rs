//! Step 11: post-processing and exports (gh:#574's Step 11 part).
//!
//! CSV (runs, spectra, the reactivity map on a grid), a kovan-markdown report
//! (`dhoby_ghaut::workbench::exports`, through `kovan::artifact`), the recipe,
//! and a round-trip check: write the recipe and read it back, or load a recipe
//! file and compare it, section by section, with the session's. Every file and
//! folder goes through the shared file picker (crate HARD RULE).

use std::path::PathBuf;

use dhoby_ghaut::workbench::exports::{differing_sections, report_markdown, verify_round_trip, write_all, Spectrum};
use dhoby_ghaut::workbench::recipe::{now_rfc3339, Branch, Recipe};
use egui::{Color32, RichText};

use crate::app::{App, Pick};

const AMBER: Color32 = Color32::from_rgb(170, 90, 0);
const GREEN: Color32 = Color32::from_rgb(30, 120, 60);

/// What a Step 6 / Step 11 file pick is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportPick {
    /// Save the reactivity map's TOML (Step 6).
    MapToml,
    /// The folder every export goes to.
    Folder,
    /// Save the report alone.
    Report,
    /// A recipe to load and compare with the session's.
    CompareRecipe,
}

/// Step 11's live state.
#[derive(Default)]
pub struct PostUi {
    /// The export folder (chosen with the picker; prefilled).
    pub dir: Option<String>,
    /// The files the last export wrote, or its error.
    pub written: Option<Result<Vec<PathBuf>, String>>,
    /// The last round-trip or comparison result.
    pub check: Option<Result<String, String>>,
}

/// Open the shared picker for `what`, where its current value is.
pub fn pick(app: &mut App, what: ExportPick) {
    let dir = PathBuf::from(export_dir(app));
    let start = if dir.is_dir() { dir } else { std::env::current_dir().unwrap_or_default() };
    let cfg = app.dialog.config_mut();
    cfg.initial_directory = start;
    cfg.default_file_name = match what {
        ExportPick::MapToml => "reactivity_map.toml".into(),
        ExportPick::Report => "report.md".into(),
        ExportPick::Folder | ExportPick::CompareRecipe => String::new(),
    };
    cfg.title = Some(
        match what {
            ExportPick::MapToml => "Save the reactivity map (plain TOML) as",
            ExportPick::Folder => "Pick the export folder",
            ExportPick::Report => "Save the report (kovan markdown) as",
            ExportPick::CompareRecipe => "Pick a recipe to compare with this session's",
        }
        .into(),
    );
    app.pick = Some(Pick::Export(what));
    match what {
        ExportPick::Folder => app.dialog.pick_directory(),
        ExportPick::CompareRecipe => app.dialog.pick_file(),
        ExportPick::MapToml | ExportPick::Report => app.dialog.save_file(),
    }
}

/// Act on what the picker returned.
pub fn picked(app: &mut App, what: ExportPick, path: PathBuf) {
    match what {
        ExportPick::MapToml => crate::step6::save_map(app, &path),
        ExportPick::Folder => app.post.dir = Some(path.display().to_string()),
        ExportPick::Report => {
            let map = map_for_export(app);
            let md = report_markdown(&app.recipe, map.as_ref(), &spectra(app), &[], &now_rfc3339());
            match md.map_err(|e| e.to_string()).and_then(|m| std::fs::write(&path, m).map_err(|e| e.to_string())) {
                Ok(()) => app.say(format!("Saved the report {}", path.display()), false),
                Err(e) => app.say(format!("Could not save {}: {e}", path.display()), true),
            }
        }
        ExportPick::CompareRecipe => {
            let r = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Recipe::from_markdown(&t).map_err(|e| e.to_string()));
            app.post.check = Some(match r {
                Err(e) => Err(format!("{}: {e}", path.display())),
                Ok(other) => {
                    let d = differing_sections(&app.recipe, &other);
                    if d.is_empty() {
                        Ok(format!("{} loads and equals this session's recipe, every section.", path.display()))
                    } else {
                        Err(format!("{} loads, but differs in: {}", path.display(), d.join(", ")))
                    }
                }
            });
        }
    }
}

fn export_dir(app: &App) -> String {
    app.post.dir.clone().unwrap_or_else(|| format!("{}/exports", app.out_dir))
}

/// The session's spectra (a spectrum is kept for runs made in this session).
fn spectra(app: &App) -> Vec<Spectrum> {
    app.mc
        .outcomes
        .iter()
        .map(|o| Spectrum {
            label: o.label.clone(),
            edges: o.edges.clone(),
            phi_per_lethargy: o.phi_per_lethargy.clone(),
        })
        .collect()
}

/// The reactivity map, when Step 6 chose it and it fits.
fn map_for_export(app: &mut App) -> Option<dhoby_ghaut::workbench::reactivity_map::ReactivityMap> {
    (app.recipe.branch.branch == Branch::ReactivityMap)
        .then(|| crate::step6::current_map(app).ok())
        .flatten()
}

/// The settings panel.
pub fn settings(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new("Export everything").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("Folder");
        ui.label(RichText::new(export_dir(app)).color(Color32::GRAY));
        if ui.button("Choose...").clicked() {
            pick(app, ExportPick::Folder);
        }
    });
    ui.small("runs.csv, spectra.csv, reactivity_map.toml + reactivity_map_grid.csv (branch A), recipe.md and report.md.");
    if ui.button("Export to this folder").clicked() {
        let map = map_for_export(app);
        app.refresh_edited();
        let r = write_all(&PathBuf::from(export_dir(app)), &app.recipe, map.as_ref(), &spectra(app), &now_rfc3339());
        match &r {
            Ok(f) => app.say(format!("Exported {} files to {}", f.len(), export_dir(app)), false),
            Err(e) => app.say(e.clone(), true),
        }
        app.post.written = Some(r);
    }
    ui.separator();
    ui.label(RichText::new("Report (kovan markdown)").strong());
    if ui.button("Save report as...").clicked() {
        pick(app, ExportPick::Report);
    }
    ui.separator();
    ui.label(RichText::new("Recipe").strong());
    ui.horizontal(|ui| {
        if ui.button("Save recipe as...").clicked() {
            app.open_picker(Pick::SaveRecipeAs);
        }
        ui.label(RichText::new(&app.recipe_path).color(Color32::GRAY));
    });
    if ui.button("Check the round trip (write, read back, compare)").clicked() {
        app.post.check = Some(
            verify_round_trip(&app.recipe).map(|()| "Written and read back through kovan: equal in every section.".to_string()),
        );
    }
    if ui.button("Load a recipe file and compare...").clicked() {
        pick(app, ExportPick::CompareRecipe);
    }
    ui.small("Comparing does not replace this session's recipe; use Open... in the top bar for that.");
}

/// The main view: what will be exported, the last results and the report.
pub fn main_view(app: &mut App, ui: &mut egui::Ui) {
    let map = map_for_export(app);
    let sp = spectra(app);
    egui::Frame::central_panel(ui.style()).inner_margin(16.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.heading("Post-processing and exports");
        ui.label("Research, education and V&V only: every exported file says so.");
        let runs = app.recipe.monte_carlo.runs.len();
        ui.label(format!(
            "{runs} Monte Carlo runs; spectra for {} of them (kept for runs made in this session only).",
            sp.len()
        ));
        match (&map, app.recipe.branch.branch) {
            (Some(m), _) => ui.label(format!(
                "Reactivity map (LOW FIDELITY): order {}, held-out RMS {} beside MC σ_ρ {:.0} pcm.",
                m.fit.order,
                m.fit.held_out_rms_pcm.map_or("n/a".into(), |x| format!("{x:.0} pcm")),
                m.fit.mc_sigma_rms_pcm
            )),
            (None, Branch::ReactivityMap) => ui.colored_label(AMBER, "Branch A is chosen but there is no map yet (Step 6 says why)."),
            (None, Branch::Multiphysics) => ui.label("Branch B (multiphysics): no reactivity map is exported."),
        };
        if let Some(w) = &app.post.written {
            ui.separator();
            match w {
                Ok(files) => {
                    ui.colored_label(GREEN, format!("Exported {} files:", files.len()));
                    for f in files {
                        ui.label(RichText::new(f.display().to_string()).monospace());
                    }
                }
                Err(e) => {
                    ui.colored_label(Color32::from_rgb(190, 30, 30), e);
                }
            }
        }
        if let Some(c) = &app.post.check {
            ui.separator();
            match c {
                Ok(m) => ui.colored_label(GREEN, format!("Round trip: {m}")),
                Err(m) => ui.colored_label(Color32::from_rgb(190, 30, 30), format!("Round trip: {m}")),
            };
        }
        ui.separator();
        ui.label(RichText::new("Report preview (kovan markdown)").strong());
        let md = report_markdown(&app.recipe, map.as_ref(), &sp, &[], &now_rfc3339()).unwrap_or_else(|e| e.to_string());
        egui::ScrollArea::vertical().id_salt("report_preview").show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(md).monospace().size(crate::app::fs(11.0))).wrap());
        });
    });
}
