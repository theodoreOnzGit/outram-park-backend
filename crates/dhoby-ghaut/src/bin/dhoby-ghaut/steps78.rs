//! Steps 7 (meshing and regions, gh:#572) and 8 (MGXS, gh:#573): their engine
//! thread and their panes.
//!
//! Kept in one file of its own so the other workbench steps' files need only
//! three hooks (the `App` field, the dispatch in `steps_ui::main_view` and
//! `steps_ui::settings`). The UI thread only draws: meshing, mapping, the
//! region-map raster and every Monte Carlo pass run on this module's own
//! engine thread ([`S78Engine`]), and post their progress back.

use std::path::PathBuf;
use std::sync::Arc;

use dhoby_ghaut::web_demo::link::{start_native, Link, NativeEngine};
use dhoby_ghaut::workbench::meshes::{
    not_available as mesh_gaps, CellType, MeshPlan, MeshRole, NotAvailable, RegionClass,
    RegionMode, RzDomain,
};
use dhoby_ghaut::workbench::mgxs::{
    not_available as mgxs_gaps, GroupPreset, InterpLaw, MgxsPlan, MgxsSet, StateXs,
};
use egui::{Color32, RichText, TextureHandle, TextureOptions};
use outram_blender::csg::plot::ImageData;
use outram_blender::unstructured::{LengthUnit, UnstructuredMesh};

use crate::app::App;
use crate::meshing::{BuiltMeshes, MeshProgress};
use crate::mgxs_run::{LiveMgxs, MgxsJob, MgxsProgress};

/// Requests to the Step 7/8 thread.
pub enum Req78 {
    /// Raster the R-Z region map.
    RegionMap { domain: RzDomain, plan: MeshPlan },
    /// Build the three meshes and the maps; write into `out_dir`.
    Build {
        domain: RzDomain,
        plan: MeshPlan,
        out_dir: PathBuf,
    },
    /// Run the MGXS state points.
    Mgxs(MgxsJob),
}

/// Events from it.
pub enum Ev78 {
    RegionMap(ImageData),
    MeshStage(MeshProgress),
    Built(Arc<BuiltMeshes>, Vec<PathBuf>),
    Mgxs(MgxsProgress),
    MgxsDone(MgxsSet, PathBuf),
    Error(String),
}

/// The Step 7/8 engine (one thread; requests run in order).
#[derive(Default)]
pub struct S78Engine;

impl NativeEngine for S78Engine {
    type Req = Req78;
    type Ev = Ev78;

    fn handle(&mut self, req: Req78, post: &mut impl FnMut(Ev78)) {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match req {
            Req78::RegionMap { domain, plan } => post(Ev78::RegionMap(
                crate::meshing::draw_region_map(&domain, &plan.regions, 700),
            )),
            Req78::Build {
                domain,
                plan,
                out_dir,
            } => {
                match crate::meshing::build(&domain, &plan, &out_dir, &mut |p| {
                    post(Ev78::MeshStage(p))
                }) {
                    Ok(b) => {
                        let mut imgs = b.images.clone();
                        imgs.push((
                            "regions_rz".into(),
                            crate::meshing::draw_region_map(&domain, &plan.regions, 900),
                        ));
                        match crate::meshing::write_images(&imgs, &out_dir.join("images")) {
                            Ok(paths) => {
                                let _ = std::fs::write(
                                    out_dir.join("meshes.csv"),
                                    crate::meshing::summary_csv(&b.set),
                                );
                                post(Ev78::Built(Arc::new(b), paths))
                            }
                            Err(e) => post(Ev78::Error(e)),
                        }
                    }
                    Err(e) => post(Ev78::Error(format!("meshing failed: {e}"))),
                }
            }
            Req78::Mgxs(job) => match crate::mgxs_run::run(&job, &mut |p| post(Ev78::Mgxs(p))) {
                Ok(set) => match crate::mgxs_run::write_csv(&set, &job.out_dir) {
                    Ok(p) => post(Ev78::MgxsDone(set, p)),
                    Err(e) => post(Ev78::Error(e)),
                },
                Err(e) => post(Ev78::Error(format!("MGXS failed: {e}"))),
            },
        }));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            post(Ev78::Error(format!("Step 7/8 request panicked: {msg}")));
        }
    }
}

/// Steps 7 and 8's UI state.
pub struct S78 {
    pub link: Link<Req78, Ev78>,
    pub domain: Option<RzDomain>,
    pub plan: Option<MeshPlan>,
    pub region_tex: Option<TextureHandle>,
    pub region_dirty: bool,
    pub meshing: bool,
    pub stage: String,
    pub frac: f64,
    pub built: Option<Arc<BuiltMeshes>>,
    pub images: Vec<(String, TextureHandle)>,
    pub written: Vec<PathBuf>,
    /// Main-view image: 0 = region map, then the mesh images.
    pub shown: usize,
    pub new_region: String,
    pub new_class: RegionClass,
    pub mgxs_plan: MgxsPlan,
    pub mgxs_running: bool,
    pub mgxs_started: f64,
    pub mgxs_stage: String,
    pub mgxs_data: Vec<(String, Option<f64>)>,
    pub live: LiveMgxs,
    pub partial: Vec<StateXs>,
    pub mgxs: Option<MgxsSet>,
    pub mgxs_csv: Option<PathBuf>,
    pub shown_state: usize,
    pub error: Option<String>,
    /// Build the meshes as soon as the geometry is in (`--auto-build`).
    pub auto_build: bool,
}

impl S78 {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            link: start_native(S78Engine, move || ctx.request_repaint()),
            domain: None,
            plan: None,
            region_tex: None,
            region_dirty: true,
            meshing: false,
            stage: String::new(),
            frac: 0.0,
            built: None,
            images: Vec::new(),
            written: Vec::new(),
            shown: 0,
            new_region: "my_region".into(),
            new_class: RegionClass::Solid,
            mgxs_plan: MgxsPlan::default(),
            mgxs_running: false,
            mgxs_started: 0.0,
            mgxs_stage: String::new(),
            mgxs_data: Vec::new(),
            live: Default::default(),
            partial: Vec::new(),
            mgxs: None,
            mgxs_csv: None,
            shown_state: 0,
            error: None,
            auto_build: false,
        }
    }

    pub fn busy(&self) -> bool {
        self.meshing || self.mgxs_running
    }

    /// The case folder everything is written to.
    pub fn case_dir(app: &App) -> PathBuf {
        PathBuf::from(&app.out_dir).join("genfoam_case")
    }
}

fn upload(ctx: &egui::Context, name: &str, image: &ImageData) -> TextureHandle {
    let mut bytes = Vec::with_capacity(image.pixels.len() * 3);
    for p in &image.pixels {
        bytes.extend_from_slice(&[p.r, p.g, p.b]);
    }
    let ci = egui::ColorImage::from_rgb([image.width, image.height], &bytes);
    ctx.load_texture(name, ci, TextureOptions::LINEAR)
}

/// Handle the Step 7/8 engine's events (called every frame from `App::ui`).
pub fn handle(app: &mut App, ctx: &egui::Context) {
    let events = app.s78.link.drain();
    for e in events {
        match e {
            Ev78::RegionMap(img) => app.s78.region_tex = Some(upload(ctx, "s78-regions", &img)),
            Ev78::MeshStage(MeshProgress::Stage(s, f)) => {
                app.s78.stage = s;
                app.s78.frac = f;
            }
            Ev78::Built(b, paths) => {
                app.s78.meshing = false;
                app.s78.images = b
                    .images
                    .iter()
                    .map(|(n, i)| (n.clone(), upload(ctx, n, i)))
                    .collect();
                app.s78.written = paths;
                app.s78.shown = 1;
                app.s78.mgxs = None;
                app.say(
                    format!(
                        "Step 7: three meshes built and written to {}",
                        S78::case_dir(app).display()
                    ),
                    false,
                );
                app.s78.built = Some(b);
            }
            Ev78::Mgxs(p) => match p {
                MgxsProgress::Stage { what, .. } => app.s78.mgxs_stage = what,
                MgxsProgress::Data(d) => {
                    use nee_soon::htr10_rmc::data::LoadProgress;
                    match d {
                        LoadProgress::Started { item } => app.s78.mgxs_data.push((item, None)),
                        LoadProgress::Finished { item, seconds } => {
                            if let Some(x) =
                                app.s78.mgxs_data.iter_mut().rev().find(|x| x.0 == item)
                            {
                                x.1 = Some(seconds);
                            }
                        }
                    }
                }
                MgxsProgress::StateDone(s) => app.s78.partial.push(s),
            },
            Ev78::MgxsDone(set, csv) => {
                app.s78.mgxs_running = false;
                app.say(
                    format!(
                        "Step 8: MGXS written to {}",
                        set.nuclear_data_path.clone().unwrap_or_default()
                    ),
                    false,
                );
                app.s78.mgxs = Some(set);
                app.s78.mgxs_csv = Some(csv);
            }
            Ev78::Error(m) => {
                app.s78.meshing = false;
                app.s78.mgxs_running = false;
                app.say(m.clone(), true);
                app.s78.error = Some(m);
            }
        }
    }
    if app.s78.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    // The domain follows the assembly.
    if let Some(a) = &app.assembly {
        let d = crate::meshing::domain_from(a);
        if app.s78.domain.as_ref() != Some(&d) {
            app.s78.plan = Some(MeshPlan::default_for(&d));
            app.s78.domain = Some(d);
            app.s78.region_dirty = true;
            app.s78.built = None;
            app.s78.images.clear();
        }
    }
    if app.s78.auto_build && !app.s78.busy() {
        if let (Some(d), Some(p)) = (&app.s78.domain, &app.s78.plan) {
            app.s78.auto_build = false;
            app.s78.meshing = true;
            app.s78.link.send(Req78::Build {
                domain: d.clone(),
                plan: p.clone(),
                out_dir: S78::case_dir(app),
            });
        }
    }
    if app.s78.region_dirty {
        if let (Some(d), Some(p)) = (&app.s78.domain, &app.s78.plan) {
            app.s78.link.send(Req78::RegionMap {
                domain: d.clone(),
                plan: p.clone(),
            });
            app.s78.region_dirty = false;
        }
    }
}

fn gaps(ui: &mut egui::Ui, list: &[NotAvailable]) {
    egui::CollapsingHeader::new(
        RichText::new("NOT available yet").color(Color32::from_rgb(190, 30, 30)),
    )
    .default_open(false)
    .show(ui, |ui| {
        for g in list {
            ui.colored_label(
                Color32::from_rgb(190, 30, 30),
                format!("NOT available: {} (gh:#{})", g.what, g.issue),
            );
            ui.small(g.why);
        }
    });
}

fn image_view(ui: &mut egui::Ui, tex: &TextureHandle) {
    let avail = ui.available_size();
    let s = tex.size_vec2();
    let k = (avail.x / s.x).min(avail.y / s.y).max(0.05);
    ui.centered_and_justified(|ui| {
        ui.image((tex.id(), s * k));
    });
}

/// The Step 7 / Step 8 main view: the region map, the mesh images, or the
/// MGXS results.
pub fn main_view(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::central_panel(ui.style()).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        if app.assembly.is_none() {
            ui.heading(app.step.title());
            ui.label("Assemble the geometry first (Steps 1-4): the meshes are built on the assembled reactor's R-Z map.");
            return;
        }
        let s = &mut app.s78;
        if app.step == dhoby_ghaut::workbench::steps::WizardStep::Mgxs && (s.mgxs.is_some() || !s.partial.is_empty()) {
            mgxs_table(s, ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut s.shown, 0, "Regions (R-Z map)");
            for (i, (n, _)) in s.images.iter().enumerate() {
                let label = n.trim_start_matches("mesh_").replace('_', " ");
                ui.selectable_value(&mut s.shown, i + 1, label);
            }
        });
        let tex = if s.shown == 0 { s.region_tex.as_ref() } else { s.images.get(s.shown - 1).map(|x| &x.1) };
        match tex {
            Some(t) => image_view(ui, t),
            None => {
                ui.label("Drawing...");
            }
        }
    });
}

fn mgxs_table(s: &mut S78, ui: &mut egui::Ui) {
    let states: Vec<StateXs> = s
        .mgxs
        .as_ref()
        .map(|m| m.states.clone())
        .unwrap_or_else(|| s.partial.clone());
    ui.heading("Multigroup cross sections per region (TENTATIVE)");
    ui.colored_label(
        Color32::from_rgb(170, 90, 0),
        "TENTATIVE: short Monte Carlo runs; σ is a first-order estimate (cells independent). Research, education and V&V only.",
    );
    ui.horizontal_wrapped(|ui| {
        for (i, st) in states.iter().enumerate() {
            ui.selectable_value(
                &mut s.shown_state,
                i,
                format!(
                    "{} K: k = {:.5} ± {:.5}",
                    st.temperature_k, st.k, st.k_sigma
                ),
            );
        }
    });
    let Some(st) = states.get(s.shown_state.min(states.len().saturating_sub(1))) else {
        return;
    };
    let n_g = st.regions.first().map_or(0, |r| r.total.len());
    egui::ScrollArea::both().show(ui, |ui| {
        egui::Grid::new("mgxs").striped(true).show(ui, |ui| {
            for h in [
                "region",
                "g",
                "Σt [1/cm]",
                "±%",
                "Σa",
                "±%",
                "νΣf",
                "±%",
                "χ",
                "Σs g→g",
                "D [cm]",
                "flux ±%",
            ] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for r in &st.regions {
                for g in 0..n_g {
                    ui.label(if g == 0 { r.region.as_str() } else { "" });
                    ui.label(format!("{}", g + 1));
                    ui.label(format!("{:.5e}", r.total[g]));
                    ui.label(format!("{:.2}", 100.0 * r.rel_sigma_total[g]));
                    ui.label(format!("{:.4e}", r.absorption[g]));
                    ui.label(format!("{:.2}", 100.0 * r.rel_sigma_absorption[g]));
                    ui.label(format!("{:.4e}", r.nu_fission[g]));
                    ui.label(if r.nu_fission[g] > 0.0 {
                        format!("{:.2}", 100.0 * r.rel_sigma_nu_fission[g])
                    } else {
                        "-".into()
                    });
                    ui.label(format!("{:.4}", r.chi[g]));
                    ui.label(format!("{:.4e}", r.scatter[g][g]));
                    ui.label(format!("{:.4}", r.diffusion(g)));
                    ui.label(format!("{:.2}", 100.0 * r.flux_rel_sigma[g]));
                    ui.end_row();
                }
            }
        });
    });
}

/// Step 7's settings pane.
pub fn mesh_settings(app: &mut App, ui: &mut egui::Ui) {
    let case = S78::case_dir(app);
    let busy = app.s78.busy();
    let s = &mut app.s78;
    let (Some(d), Some(plan)) = (s.domain.clone(), s.plan.as_mut()) else {
        ui.label("Assemble the geometry first (Steps 1-4).");
        return;
    };
    ui.label(RichText::new("Regions").strong());
    let mut basic = plan.regions.mode == RegionMode::Basic;
    let mut dirty = false;
    ui.horizontal(|ui| {
        if ui.radio_value(&mut basic, true, "Basic (preset)").clicked() {
            plan.regions = dhoby_ghaut::workbench::meshes::RegionMap::basic(&d);
            dirty = true;
        }
        if ui.radio_value(&mut basic, false, "Advanced").clicked() {
            plan.regions.mode = RegionMode::Advanced;
        }
    });
    ui.small("In-core regions (bed, conus, tube, cavity) are preset and never have to be managed.");
    if plan.regions.mode == RegionMode::Advanced {
        let [mut nr, mut nz] = plan.regions.bed_split;
        ui.horizontal(|ui| {
            ui.label("Bed split: rings");
            let a = ui.add(egui::DragValue::new(&mut nr).range(1..=6)).changed();
            ui.label("layers");
            let b = ui.add(egui::DragValue::new(&mut nz).range(1..=8)).changed();
            if a || b {
                plan.regions.split_bed(nr, nz);
                dirty = true;
            }
        });
        egui::CollapsingHeader::new("Zones outside the core (TECDOC-1382 Fig. 4.10)").show(
            ui,
            |ui| {
                let outside: Vec<(usize, String)> = plan
                    .regions
                    .regions
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| !r.in_core)
                    .map(|(i, r)| (i, r.label.clone()))
                    .collect();
                for (b, bx) in d.boxes.iter().enumerate() {
                    let cur = plan.regions.box_region[b];
                    let mut sel = cur;
                    ui.horizontal(|ui| {
                        ui.label(format!("zone {} r {:.1}-{:.1}", bx.zone, bx.r[0], bx.r[1]));
                        egui::ComboBox::from_id_salt(("box", b))
                            .selected_text(plan.regions.regions[cur].label.clone())
                            .show_ui(ui, |ui| {
                                for (i, l) in &outside {
                                    ui.selectable_value(&mut sel, *i, l);
                                }
                            });
                    });
                    if sel != cur && plan.regions.assign_box(b, sel).is_ok() {
                        dirty = true;
                    }
                }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.new_region).desired_width(110.0));
                    egui::ComboBox::from_id_salt("newclass")
                        .selected_text(s.new_class.label())
                        .show_ui(ui, |ui| {
                            for c in [RegionClass::Solid, RegionClass::Porous, RegionClass::Fluid] {
                                ui.selectable_value(&mut s.new_class, c, c.label());
                            }
                        });
                    let id: String = s
                        .new_region
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    let ok = !id.is_empty() && !plan.regions.regions.iter().any(|r| r.id == id);
                    if ui
                        .add_enabled(ok, egui::Button::new("Add region"))
                        .clicked()
                    {
                        plan.regions.add_region(&id, &s.new_region, s.new_class);
                    }
                });
            },
        );
    }
    for r in &plan.regions.regions {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&r.label).strong());
            ui.small(format!(
                "{} · {}{}",
                r.id,
                r.class.label(),
                if r.in_core { " · in-core" } else { "" }
            ));
        });
    }
    ui.separator();
    ui.label(RichText::new("Meshes (GeN-Foam style)").strong());
    for role in MeshRole::ALL {
        let rs = &mut plan.roles[role.index()];
        egui::CollapsingHeader::new(role.name())
            .default_open(false)
            .show(ui, |ui| {
                let cells = match role {
                    MeshRole::Neutronics => CellType::TetDual.label().to_string(),
                    MeshRole::ThermalHydraulics => format!(
                        "bed (porous): {}; cavity (fluid): {}",
                        RegionClass::PebbleBed.th_cells().label(),
                        RegionClass::Fluid.th_cells().label()
                    ),
                    MeshRole::Structural => CellType::Tet4.label().to_string(),
                };
                ui.small(cells);
                ui.horizontal(|ui| {
                    ui.label("Cell size [cm]");
                    ui.add(
                        egui::DragValue::new(&mut rs.cell_size_cm)
                            .range(5.0..=60.0)
                            .speed(0.5),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Facets around");
                    ui.add(egui::DragValue::new(&mut rs.n_seg).range(8..=128));
                    ui.label("wall refinement levels");
                    ui.add(egui::DragValue::new(&mut rs.refine).range(0..=3));
                });
                if role == MeshRole::ThermalHydraulics {
                    ui.horizontal(|ui| {
                        ui.label("Layers");
                        ui.add(egui::DragValue::new(&mut rs.layers).range(0..=8));
                        ui.label("first [cm]");
                        ui.add(
                            egui::DragValue::new(&mut rs.first_layer_cm)
                                .range(0.05..=5.0)
                                .speed(0.05),
                        );
                        ui.label("ratio");
                        ui.add(
                            egui::DragValue::new(&mut rs.expansion)
                                .range(1.0..=2.0)
                                .speed(0.01),
                        );
                    });
                }
            });
    }
    ui.small("Defaults are coarse, for a quick build; no mesh-convergence study backs them.");
    ui.horizontal_wrapped(|ui| {
        ui.label("Case folder:");
        ui.monospace(case.display().to_string());
    });
    let pick = ui.button("Output folder...").clicked();
    if dirty {
        s.region_dirty = true;
    }
    let build = ui
        .add_enabled(
            !busy,
            egui::Button::new(RichText::new("Build the three meshes").size(crate::app::fs(16.0))),
        )
        .clicked();
    if build {
        s.meshing = true;
        s.stage = "starting".into();
        s.frac = 0.0;
        s.error = None;
        s.link.send(Req78::Build {
            domain: d.clone(),
            plan: plan.clone(),
            out_dir: case.clone(),
        });
    }
    if s.meshing {
        ui.add(egui::ProgressBar::new(s.frac as f32).text(s.stage.clone()));
    }
    gaps(ui, &mesh_gaps());
    if let Some(b) = &s.built {
        results78(ui, b, &s.written);
    }
    if pick {
        app.open_picker(crate::app::Pick::PngFolder);
    }
}

fn results78(ui: &mut egui::Ui, b: &BuiltMeshes, written: &[PathBuf]) {
    ui.separator();
    ui.label(RichText::new("Built").strong());
    for m in &b.set.meshes {
        egui::CollapsingHeader::new(format!("{}: {} cells", m.role.name(), m.cells)).default_open(true).show(ui, |ui| {
            ui.small(format!("{} · {:.1} s", m.cell_type.label(), m.seconds));
            ui.small(format!(
                "kinds: {}",
                m.kinds.iter().map(|k| format!("{} {}", k.1, k.0)).collect::<Vec<_>>().join(", ")
            ));
            ui.small(format!(
                "volume {:.4e} cm³ vs exact {:.4e} ({:+.2} %); non-star cells {}; max non-orth {:.0}°",
                m.volume_cm3,
                m.exact_cm3,
                100.0 * (m.volume_cm3 / m.exact_cm3 - 1.0),
                m.non_star_cells,
                m.max_non_orthogonality_deg
            ));
            ui.small(format!("patches: {}", m.patches.iter().map(|p| format!("{} ({})", p.0, p.1)).collect::<Vec<_>>().join(", ")));
            for n in &m.notes {
                ui.colored_label(Color32::from_rgb(170, 90, 0), n);
            }
            for r in &m.regions {
                ui.small(format!(
                    "{}: {} cells, {:+.1} % vs exact",
                    b.set.plan.regions.regions[r.region].id,
                    r.cells,
                    if r.exact_cm3 > 0.0 { 100.0 * (r.mesh_cm3 / r.exact_cm3 - 1.0) } else { f64::NAN }
                ));
            }
        });
    }
    egui::CollapsingHeader::new("Mesh-to-mesh maps (cell-volume weight, as GeN-Foam)")
        .default_open(true)
        .show(ui, |ui| {
            for m in &b.set.mappings {
                ui.small(format!(
                    "{} → {}: covers {:.2} % of the target, {} target cells uncovered ({:.1} s)",
                    m.from.name(),
                    m.to.name(),
                    100.0 * m.to_coverage,
                    m.uncovered_cells,
                    m.seconds
                ));
                if !m.fields.is_empty() {
                    ui.small(format!(
                        "   fields: {}",
                        m.fields
                            .iter()
                            .map(|f| format!("{}→{}", f.0, f.1))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        });
    if !written.is_empty() {
        ui.small(format!(
            "{} review images in {}",
            written.len(),
            written[0]
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        ));
    }
}

/// Step 8's settings pane.
pub fn mgxs_settings(app: &mut App, ui: &mut egui::Ui) {
    let now = app.now();
    let case = S78::case_dir(app);
    let core = app.core.clone();
    let s = &mut app.s78;
    let p = &mut s.mgxs_plan;
    ui.label(RichText::new("State points").strong());
    let mut remove = None;
    for (i, t) in p.temperatures_k.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(if i == 0 { "T (reference) [K]" } else { "T [K]" });
            ui.add(egui::DragValue::new(t).range(250.0..=2000.0).speed(1.0));
            if i > 0 && ui.small_button("remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        p.temperatures_k.remove(i);
    }
    if ui.button("Add a state point").clicked() {
        let last = p.temperatures_k.last().copied().unwrap_or(300.0);
        p.temperatures_k.push(last + 300.0);
    }
    ui.small("Each state point reprocesses the nuclear data with every material at T (minutes).");
    egui::ComboBox::from_label("Interpolation")
        .selected_text(p.law.label())
        .show_ui(ui, |ui| {
            for l in [InterpLaw::LnT, InterpLaw::SqrtT, InterpLaw::Linear] {
                ui.selectable_value(&mut p.law, l, l.label());
            }
        });
    egui::ComboBox::from_label("Groups")
        .selected_text(p.groups.label())
        .show_ui(ui, |ui| {
            for g in [GroupPreset::Two, GroupPreset::Four, GroupPreset::Eight] {
                ui.selectable_value(&mut p.groups, g, g.label());
            }
        });
    ui.separator();
    ui.label(RichText::new("Statistics").strong());
    for (label, v, r) in [
        ("Neutrons per generation", &mut p.particles, 100..=100_000),
        ("Inactive generations", &mut p.inactive, 1..=500),
        ("Active generations", &mut p.active, 2..=1000),
        ("Threads", &mut p.threads, 1..=64),
    ] {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add(egui::DragValue::new(v).range(r));
        });
    }
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut p.seed));
    });
    ui.small("Two passes per state at the same seed (reaction rates, then the scattering matrix).");
    ui.separator();
    let ready = s.built.is_some() && core.is_some();
    if !ready {
        ui.colored_label(Color32::from_rgb(170, 90, 0), "Build the meshes in Step 7 first: the cross sections are tallied on the neutronics mesh's regions.");
    }
    if ui
        .add_enabled(
            ready && !s.busy(),
            egui::Button::new(
                RichText::new("Run the MGXS state points").size(crate::app::fs(16.0)),
            ),
        )
        .clicked()
    {
        if let (Some(b), Some(core)) = (&s.built, core) {
            let nm = &b.meshes[MeshRole::Neutronics.index()];
            let mesh: Arc<UnstructuredMesh> = Arc::new(nm.with_unit(LengthUnit::Centimetre));
            if let Ok(mut l) = s.live.write() {
                l.clear();
            }
            let job = MgxsJob {
                plan: s.mgxs_plan.clone(),
                core,
                mesh,
                cell_region: b.set.meshes[MeshRole::Neutronics.index()]
                    .cell_region
                    .clone(),
                regions: b.set.plan.regions.regions.clone(),
                out_dir: case.clone(),
                live: s.live.clone(),
            };
            s.mgxs_running = true;
            s.mgxs_started = now;
            s.mgxs_data.clear();
            s.partial.clear();
            s.mgxs = None;
            s.error = None;
            s.link.send(Req78::Mgxs(job));
        }
    }
    if s.mgxs_running {
        ui.label(format!("{} ({:.0} s)", s.mgxs_stage, now - s.mgxs_started));
        if let Some((_, None)) = s.mgxs_data.last() {
            ui.small(format!(
                "processing {}",
                s.mgxs_data.last().map(|x| x.0.as_str()).unwrap_or("")
            ));
        }
        let gens = s.live.read().map(|l| l.last().cloned()).ok().flatten();
        if let Some((st, pass, g)) = gens {
            let of = s.mgxs_plan.inactive + s.mgxs_plan.active;
            ui.add(
                egui::ProgressBar::new((g.index + 1) as f32 / of as f32).text(format!(
                    "state {} pass {}: generation {}/{} k = {:.5}",
                    st + 1,
                    pass + 1,
                    g.index + 1,
                    of,
                    g.k
                )),
            );
        }
        ui.small(format!(
            "states done: {}/{}",
            s.partial.len(),
            s.mgxs_plan.temperatures_k.len()
        ));
    }
    gaps(ui, &mgxs_gaps());
    if let Some(m) = &s.mgxs {
        ui.separator();
        ui.label(RichText::new("Hand-off").strong());
        ui.small(format!(
            "GeN-Foam nuclearData: {}",
            m.nuclear_data_path.clone().unwrap_or_default()
        ));
        if let Some(c) = &s.mgxs_csv {
            ui.small(format!("CSV: {}", c.display()));
        }
        for n in &m.notes {
            ui.small(format!("note: {n}"));
        }
    }
}
