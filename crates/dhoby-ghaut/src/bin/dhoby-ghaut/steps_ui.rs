//! What each step puts in the three panes: the literature pane (left), the
//! main view (centre) and the settings panel (right), plus Step 5's results.

use dhoby_ghaut::workbench::recipe::{Citation, ElementStatus, ModelElement};
use dhoby_ghaut::workbench::steps::WizardStep;
use egui::{Color32, RichText};
use outram_mc_libs::geometry::plot::PlotBasis;

use crate::app::App;
use crate::engine::{KeffJob, Req};
use crate::slice_view::{plane_controls, Preset};

/// The review gate's minimum set (the crate's drawing rule): the whole model
/// axially, radial slices at the heights that matter, and zooms down to one
/// pebble and one TRISO particle.
pub fn review_presets(app: &App) -> Vec<Preset> {
    app.assembly
        .as_ref()
        .map_or_else(Vec::new, review_presets_for)
}

/// [`review_presets`] for an assembled geometry.
pub fn review_presets_for(a: &crate::engine::AssemblyInfo) -> Vec<Preset> {
    let zmid = 0.5 * (a.z_range[0] + a.z_range[1]);
    let half = 0.5 * (a.z_range[1] - a.z_range[0]).max(400.0) + 5.0;
    let mut v = vec![
        Preset {
            name: "R-Z (X-Z) whole model",
            basis: PlotBasis::Xz,
            depth: 0.0,
            centre: [0.0, zmid],
            half_extent: half,
        },
        Preset {
            name: "X-Y at bed mid-height",
            basis: PlotBasis::Xy,
            depth: 0.0,
            centre: [0.0, 0.0],
            half_extent: 196.0,
        },
        Preset {
            name: "X-Y through the conus",
            basis: PlotBasis::Xy,
            depth: 0.5 * (a.conus_floor - a.bed_half_height),
            centre: [0.0, 0.0],
            half_extent: 196.0,
        },
        Preset {
            name: "X-Y through the cavity",
            basis: PlotBasis::Xy,
            depth: 0.5 * (a.bed_half_height + a.cavity_top),
            centre: [0.0, 0.0],
            half_extent: 196.0,
        },
        Preset {
            name: "X-Y at the bed wall and rod ring",
            basis: PlotBasis::Xy,
            depth: 0.0,
            centre: [100.0, 0.0],
            half_extent: 25.0,
        },
    ];
    if let (Some(p), Some(t)) = (a.pebble, a.particle) {
        v.push(Preset {
            name: "X-Y through one fuel pebble",
            basis: PlotBasis::Xy,
            depth: t[2],
            centre: [p[0], p[1]],
            half_extent: 3.6,
        });
        v.push(Preset {
            name: "X-Y round one TRISO particle",
            basis: PlotBasis::Xy,
            depth: t[2],
            centre: [t[0], t[1]],
            half_extent: 0.4,
        });
    }
    v
}

/// The view each geometry step opens on.
fn step_preset(app: &App, step: WizardStep) -> Option<Preset> {
    let p = review_presets(app);
    let pick = |name: &str| p.iter().find(|x| x.name == name).copied();
    match step {
        WizardStep::PebbleBed | WizardStep::MonteCarlo => pick("R-Z (X-Z) whole model"),
        WizardStep::PebbleDesign => pick("X-Y through one fuel pebble"),
        WizardStep::Reflector => pick("X-Y at bed mid-height"),
        WizardStep::Inserts => pick("X-Y at the bed wall and rod ring"),
        _ => None,
    }
}

fn cites_for(app: &App, step: WizardStep) -> Vec<Citation> {
    let r = &app.recipe;
    match step {
        WizardStep::NuclearData => r.nuclear_data.cites.clone(),
        WizardStep::PebbleBed => r.pebble_bed.cites.clone(),
        WizardStep::PebbleDesign => r.pebble_design.cites.clone(),
        WizardStep::Reflector => r.reflector.cites.clone(),
        WizardStep::Inserts => r.inserts.cites.clone(),
        WizardStep::MonteCarlo => r.monte_carlo.cites.clone(),
        _ => Vec::new(),
    }
}

pub fn panes(app: &mut App, ui: &mut egui::Ui) {
    let step = app.step;
    if app.shown_step.is_some_and(|s| s != step) {
        app.lit.clear();
    }
    if !matches!(
        step,
        WizardStep::PebbleBed
            | WizardStep::PebbleDesign
            | WizardStep::Reflector
            | WizardStep::Inserts
            | WizardStep::Review
            | WizardStep::MonteCarlo
    ) {
        app.shown_step = Some(step);
    }
    // Literature, left.
    let cites = cites_for(app, step);
    let mut lit_open = app.lit_open;
    egui::Panel::left("literature")
        .default_size(330.0)
        .resizable(true)
        .show_collapsible(ui, &mut lit_open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let geo = &app.geo;
                app.lit.show(ui, &cites, &mut |r| geo.send(r));
            });
        });
    app.lit_open = lit_open;
    // Settings, right.
    let mut settings_open = app.settings_open;
    egui::Panel::right("settings")
        .default_size(340.0)
        .resizable(true)
        .show_collapsible(ui, &mut settings_open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| settings(app, ui));
        });
    app.settings_open = settings_open;
    // Step 5's results, bottom.
    if step == WizardStep::MonteCarlo
        && (app.mc.running
            || !app.mc.outcomes.is_empty()
            || !app.mc.items.is_empty()
            || !app.recipe.monte_carlo.runs.is_empty())
    {
        let mut open = app.results_open;
        egui::Panel::bottom("results")
            .default_size(310.0)
            .size_range(200.0..=520.0)
            .resizable(true)
            .show_collapsible(ui, &mut open, |ui| {
                crate::results::show(app, ui);
            });
        app.results_open = open;
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| main_view(app, ui));
}

fn main_view(app: &mut App, ui: &mut egui::Ui) {
    let step = app.step;
    match step {
        WizardStep::NuclearData => {
            egui::Frame::central_panel(ui.style()).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                crate::results::tape_table(app, ui);
            });
        }
        s if !s.implemented() => {
            egui::Frame::central_panel(ui.style()).inner_margin(20.0).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.heading(s.title());
                ui.label(s.guide());
                ui.add_space(8.0);
                ui.colored_label(
                    Color32::from_rgb(170, 90, 0),
                    format!("Not built yet. This step is planned in gh:#{}; nothing here is simulated.", s.issue().unwrap_or(561)),
                );
                ui.add_space(8.0);
                ui.label("What exists to build it on, found in the workspace (2026-10-05):");
                for line in building_blocks(s) {
                    ui.label(format!("• {line}"));
                }
            });
        }
        _ => {
            let now = app.now();
            let have = app.core.is_some();
            // Each step opens on its own view, once, when it is entered and
            // the geometry exists; after that the reader is in charge.
            if have && app.shown_step != Some(step) && app.main_rect.is_positive() {
                if let Some(p) = step_preset(app, step) {
                    let rect = app.main_rect;
                    app.slice.go(&p, rect);
                }
                app.shown_step = Some(step);
            }
            let geo = &app.geo;
            let rect = app.slice.show(ui, now, have, &mut |r| geo.send(r));
            app.main_rect = rect;
        }
    }
}

fn building_blocks(s: WizardStep) -> Vec<&'static str> {
    match s {
        WizardStep::Branch => vec![
            "multiphysics: nee_soon::mgxs (MC-tallied MGXS) and nee_soon::genfoam_xs",
            "reactivity map: Chebyshev fits exist only in tampines-steam-tables; export as plain TOML (decided)",
        ],
        WizardStep::Meshing => vec![
            "outram-park-fork-cfmesh tet -> dual -> boundary-layer pipeline (mesh_studio drives it)",
            "outram-foam-appbuilder-lib/src/genfoam/multi_region: mesh_to_mesh, rbf_mapping, coupling_fields",
            "farrer-park: Tet4 FEM structural elements",
        ],
        WizardStep::Mgxs => vec!["nee_soon::mgxs, nee_soon::genfoam_xs, njoy-outram-park-fork delayed_mgxs"],
        WizardStep::Setup | WizardStep::Run => vec![
            "outram-foam-appbuilder-lib GeN-Foam port; tampines pebble_bed (friction, effective conductivity)",
            "farrer-park (FEM structural)",
        ],
        WizardStep::PostProcessing => vec!["the recipe already saves as kovan markdown (Save recipe, top bar)"],
        _ => Vec::new(),
    }
}

fn elements(ui: &mut egui::Ui, list: &[ModelElement]) {
    for e in list {
        let c = match e.status {
            ElementStatus::InModel => Color32::from_rgb(30, 120, 60),
            ElementStatus::Simplified => Color32::from_rgb(170, 110, 0),
            ElementStatus::NotInModel => Color32::from_rgb(190, 30, 30),
        };
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(c, format!("[{}]", e.status.badge()));
            ui.label(RichText::new(&e.name).strong());
        });
        ui.small(&e.note);
    }
}

fn num(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(speed).range(range))
            .changed()
    })
    .inner
}

fn int(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut usize,
    range: std::ops::RangeInclusive<usize>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).range(range)).changed()
    })
    .inner
}

fn settings(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new("Settings (prefilled)").strong());
    ui.small("Everything is prefilled from the HTR-10 preset. Change anything, or nothing.");
    ui.separator();
    let mut changed = false;
    match app.step {
        WizardStep::NuclearData => {
            ui.label("ENDF folder (unzipped tapes)");
            ui.text_edit_singleline(&mut app.endf_dir);
            ui.horizontal(|ui| {
                if ui.button("Scan").clicked() {
                    app.scan_endf();
                }
                if app.scanning {
                    ui.spinner();
                }
            });
            ui.label(format!("Library: {}", app.recipe.nuclear_data.library));
            changed |= num(
                ui,
                "Data temperature [K]",
                &mut app.recipe.nuclear_data.temperature_k,
                1.0,
                1.0..=3000.0,
            );
            ui.small("Every nuclide is reconstructed and Doppler-broadened at this temperature (RECONR + BROADR, tol 1e-3) when Step 5 runs.");
        }
        WizardStep::PebbleBed => {
            let pb = &mut app.recipe.pebble_bed;
            ui.label(RichText::new(&pb.packing).italics());
            changed |= int(ui, "Radial rings of the tiling", &mut pb.rings, 1..=20);
            changed |= int(
                ui,
                "Loaded layers N (bed height 9.798 N + 6 cm)",
                &mut pb.layers,
                1..=25,
            );
            ui.label(format!("Filling fraction {:.3}", pb.filling_fraction));
            ui.label("Pebble-type mix (fraction of pebbles)");
            changed |= num(ui, "  fuel", &mut pb.mix.fuel, 0.01, 0.0..=1.0);
            changed |= num(ui, "  moderator", &mut pb.mix.moderator, 0.01, 0.0..=1.0);
            changed |= num(ui, "  fertile", &mut pb.mix.fertile, 0.01, 0.0..=1.0);
            changed |= num(ui, "  poison", &mut pb.mix.poison, 0.01, 0.0..=1.0);
            let mix_is_preset = pb.mix == app.preset.pebble_bed.mix;
            if (pb.mix.total() - 1.0).abs() > 1e-9 {
                ui.colored_label(
                    Color32::from_rgb(190, 30, 30),
                    format!("The mix sums to {:.3}, not 1.", pb.mix.total()),
                );
            }
            if !mix_is_preset {
                ui.colored_label(
                    Color32::from_rgb(170, 90, 0),
                    "The HTR-10 geometry builder places fuel and moderator balls by its own rule; a changed mix is \
                     recorded in the recipe but not yet built (gh:#566).",
                );
            }
            if let Some(a) = &app.assembly {
                ui.separator();
                ui.label(format!(
                    "Built: {} rings × {} layers, bed {:.1} cm high, r {:.1} cm",
                    a.rings, a.layers, a.bed_height, a.bed_radius
                ));
                if let Some(b) = a.balls {
                    ui.label(format!("{b} balls in the core (Şeker count)"));
                }
                ui.label(format!(
                    "{} cells, {} universes, {} tiles; assembled in {:.1} s",
                    a.cells, a.universes, a.tiles, a.seconds
                ));
            }
        }
        WizardStep::PebbleDesign => {
            ui.label(format!(
                "Voids between pebbles: {}",
                app.recipe.pebble_design.interstitial
            ));
            for p in &app.recipe.pebble_design.pebbles {
                ui.separator();
                ui.label(RichText::new(format!("{} pebble", p.kind)).strong());
                ui.label(format!(
                    "outer radius {:.3} cm, matrix {}",
                    p.outer_radius_cm, p.matrix
                ));
                if let Some(r) = p.fuel_zone_radius_cm {
                    ui.label(format!("fuelled zone radius {r:.3} cm"));
                }
                if let Some(t) = &p.triso {
                    ui.label(format!(
                        "TRISO: {} kernel r {:.4} cm, {:.0} wt% U-235, {} per pebble",
                        t.kernel,
                        t.kernel_radius_cm,
                        100.0 * t.enrichment,
                        t.particles_per_pebble
                    ));
                    for (n, r) in &t.layers {
                        ui.label(format!("   {n}: outer r {r:.4} cm"));
                    }
                }
            }
            ui.small("Pebble designs are read-only in this build: the HTR-10 builder takes them from the paper's tables (gh:#566).");
        }
        WizardStep::Reflector => {
            let r = &app.recipe.reflector;
            ui.label(format!(
                "Core radius {:.1} cm, reflector outer radius {:.1} cm, model height {:.1} cm",
                r.core_radius_cm, r.reflector_outer_cm, r.model_height_cm
            ));
            ui.separator();
            elements(ui, &r.elements.clone());
        }
        WizardStep::Inserts => {
            let i = &app.recipe.inserts;
            ui.label(format!(
                "{} control rods: B4C ring {:.2}–{:.2} cm, {:.2} g/cm³",
                i.control_rods,
                i.b4c_inner_radius_cm,
                i.b4c_outer_radius_cm,
                i.b4c_density_g_per_cm3
            ));
            ui.separator();
            elements(ui, &i.elements.clone());
        }
        WizardStep::Review => review(app, ui),
        WizardStep::MonteCarlo => changed |= monte_carlo(app, ui),
        s => {
            ui.label(format!(
                "Nothing to set: {} is not built yet (gh:#{}).",
                s.name(),
                s.issue().unwrap_or(561)
            ));
        }
    }
    if matches!(
        app.step,
        WizardStep::PebbleBed
            | WizardStep::PebbleDesign
            | WizardStep::Reflector
            | WizardStep::Inserts
            | WizardStep::MonteCarlo
    ) {
        ui.separator();
        ui.label(RichText::new("Main view").strong());
        plane_controls(ui, &mut app.slice);
        ui.horizontal(|ui| {
            ui.label("PNG folder");
            ui.text_edit_singleline(&mut app.out_dir);
        });
        if ui.button("Export this view as PNG").clicked() {
            export_view(app);
        }
        egui::CollapsingHeader::new(format!(
            "Legend ({} materials in view)",
            app.slice.legend.len()
        ))
        .default_open(false)
        .show(ui, |ui| {
            for (c, label) in &app.slice.legend {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, *c);
                    ui.label(*label);
                });
            }
        });
    }
    if app.geometry_stale() && !app.assembling {
        ui.separator();
        ui.colored_label(
            Color32::from_rgb(170, 90, 0),
            "The bed settings changed since the geometry was built.",
        );
        if ui.button("Re-assemble the geometry").clicked() {
            app.assemble();
            app.recipe.review = Default::default();
            app.review_seen.clear();
        }
    }
    if app.assembling {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Assembling the geometry…");
        });
    }
    if changed {
        app.refresh_edited();
    }
}

fn export_view(app: &mut App) {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, app.slice.canvas);
    let (origin, width, pixels) = app.slice.window(rect, 1400.0);
    let name = app
        .slice
        .at_preset
        .unwrap_or("view")
        .to_lowercase()
        .replace([' ', '(', ')', '-'], "_");
    let n = app.status.len();
    let path = std::path::PathBuf::from(&app.out_dir).join(format!("htr10_{name}_{n}.png"));
    let title = format!(
        "HTR-10 {}X{}: {}",
        app.recipe.pebble_bed.rings,
        app.recipe.pebble_bed.layers,
        app.slice.at_preset.unwrap_or("view").to_uppercase()
    );
    app.geo.send(Req::ExportPng {
        basis: app.slice.basis,
        origin,
        width,
        pixels,
        title,
        path,
    });
}

fn review(app: &mut App, ui: &mut egui::Ui) {
    ui.label("Open each view; it is ticked once its slice has been drawn on screen. Pan and zoom freely.");
    let presets = review_presets(app);
    if presets.is_empty() {
        ui.label("Waiting for the geometry…");
        return;
    }
    for p in &presets {
        ui.horizontal(|ui| {
            let seen = app.review_seen.contains(&p.name);
            ui.label(if seen { "[x]" } else { "[ ]" });
            if ui.button(p.name).clicked() {
                let rect = app.main_rect;
                app.slice.go(p, rect);
            }
        });
    }
    if app.assembly.as_ref().is_some_and(|a| a.pebble.is_none()) {
        ui.colored_label(
            Color32::from_rgb(190, 30, 30),
            "No fuel pebble was found near the axis: the pebble and TRISO zooms are missing.",
        );
    }
    ui.separator();
    ui.label("What to check: whole pebbles (none cut at the wall, cone or tube); the reflector zones and boronated bricks where the TECDOC zone map puts them; every boring; five TRISO layers round each kernel.");
    let all = presets.iter().all(|p| app.review_seen.contains(&p.name));
    if app.recipe.review.passed() {
        ui.colored_label(
            Color32::from_rgb(30, 120, 60),
            format!(
                "Confirmed at {}",
                app.recipe.review.confirmed_at.clone().unwrap_or_default()
            ),
        );
    } else if ui
        .add_enabled(
            all,
            egui::Button::new("I have looked at every view: continue to Monte Carlo"),
        )
        .clicked()
    {
        app.recipe.review.viewed = presets.iter().map(|p| p.name.to_string()).collect();
        app.recipe.review.confirmed_at = Some(dhoby_ghaut::workbench::recipe::now_rfc3339());
        app.step = WizardStep::MonteCarlo;
    }
    if ui.button("Export every review view as PNG").clicked() {
        for p in &presets {
            let half = p.half_extent;
            let origin = crate::slice_view::SliceView::origin(p.basis, p.depth, p.centre);
            let name = p.name.to_lowercase().replace([' ', '(', ')', '-'], "_");
            let path = std::path::PathBuf::from(&app.out_dir).join(format!("review_{name}.png"));
            let title = format!(
                "HTR-10 {}X{}: {}",
                app.recipe.pebble_bed.rings,
                app.recipe.pebble_bed.layers,
                p.name.to_uppercase()
            );
            app.geo.send(Req::ExportPng {
                basis: p.basis,
                origin,
                width: [2.0 * half, 2.0 * half],
                pixels: [1000, 1000],
                title,
                path,
            });
        }
    }
}

fn monte_carlo(app: &mut App, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    let busy = app.mc.running;
    ui.label(RichText::new("State of the next run").strong());
    changed |= num(
        ui,
        "Temperature, every material [K]",
        &mut app.recipe.nuclear_data.temperature_k,
        1.0,
        1.0..=3000.0,
    );
    ui.small(
        "A new temperature reprocesses the nuclear data (minutes); the same temperature reuses it.",
    );
    ui.horizontal(|ui| {
        ui.label("Control-rod insertion");
        ui.add_enabled(
            false,
            egui::Slider::new(&mut app.recipe.monte_carlo.rod_insertion, 0.0..=1.0),
        );
    });
    ui.small("Rods are explicit but only WITHDRAWN in this model (the benchmark state); insertion is not modelled yet (gh:#580).");
    ui.separator();
    ui.label(RichText::new("Statistics").strong());
    let mc = &mut app.recipe.monte_carlo;
    ui.horizontal(|ui| {
        if ui.button("Record (QUICK) 2000 × [30 + 70]").clicked() {
            (mc.particles, mc.inactive, mc.active) = (2000, 30, 70);
        }
        if ui.button("Preview 500 × [10 + 20]").clicked() {
            (mc.particles, mc.inactive, mc.active) = (500, 10, 20);
        }
    });
    int(
        ui,
        "Neutrons per generation",
        &mut mc.particles,
        100..=100_000,
    );
    int(ui, "Inactive generations", &mut mc.inactive, 1..=500);
    int(ui, "Active generations", &mut mc.active, 2..=1000);
    int(ui, "Threads", &mut mc.threads, 1..=64);
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut mc.seed));
    });
    int(
        ui,
        "Spectrum bins per decade",
        &mut mc.spectrum_bins_per_decade,
        1..=50,
    );
    ui.small(
        "Statistics set σ, not the physics. A preview run has a larger σ; it is reported with it.",
    );
    ui.separator();
    ui.label(RichText::new("Physics").strong());
    if mc.ablations.is_empty() {
        ui.label(
            "No ablation selected: the data layout is nee_soon's correct-physics default \
             (Htr10DataConfig::default: bound thermal laws on, full rod metal). Its own modelling \
             notes (substitutions, mixed libraries) are listed with every run.",
        );
    } else {
        ui.colored_label(
            Color32::from_rgb(170, 90, 0),
            format!("Ablations: {}", mc.ablations.join(", ")),
        );
    }
    ui.separator();
    let ready = app.core.is_some() && app.recipe.review.passed();
    let label = format!("Run {}", app.recipe.monte_carlo.runs.len() + 1);
    if ui
        .add_enabled(
            ready && !busy,
            egui::Button::new(RichText::new(format!("Run: {label}")).size(18.0)),
        )
        .clicked()
    {
        let mc = &app.recipe.monte_carlo;
        let job = KeffJob {
            label,
            particles: mc.particles,
            inactive: mc.inactive,
            active: mc.active,
            seed: mc.seed,
            threads: mc.threads,
            temperature_k: app.recipe.nuclear_data.temperature_k,
            bins_per_decade: mc.spectrum_bins_per_decade,
        };
        app.mc.running = true;
        app.mc.items.clear();
        app.results_open = true;
        app.phys.send(Req::RunKeff(job));
    }
    if !app.recipe.review.passed() {
        ui.colored_label(
            Color32::from_rgb(170, 90, 0),
            "Pass the geometry review gate first.",
        );
    }
    changed
}
