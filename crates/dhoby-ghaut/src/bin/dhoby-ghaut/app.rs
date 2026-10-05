//! The workbench window: screen flow, top bar and the three panes.
//!
//! ```text
//! ┌─ ◀ Back │ Step N: <name> │ Next ▶ ─────────────────────────────────────┐
//! │ Literature (kovan) │ Main view: the reactor        │ Settings (prefilled)│
//! └────────────────────┴───────────────────────────────┴─────────────────────┘
//! ```
//!
//! Screens: generation picker → Basic/Advanced → HTGR sub-type → the wizard.
//! The UI thread only draws; the two engine threads do the work.

use std::path::PathBuf;
use std::sync::Arc;

use dhoby_ghaut::web_demo::link::{start_native, Link};
use dhoby_ghaut::workbench::catalogue::{Generation, HtgrCore, Mode, ReactorType, Support};
use dhoby_ghaut::workbench::recipe::{now_rfc3339, Recipe};
use dhoby_ghaut::workbench::steps::WizardStep;
use egui::{Color32, RichText};
use nee_soon::htr10_rmc::core_model::AssembledCore;

use crate::engine::{AssemblyInfo, Engine, Ev, KeffOutcome, NeededTape, Req, TapeInfo};
use crate::literature::Literature;
use crate::preset;
use crate::slice_view::SliceView;

/// Every font in the workbench is this many times its base size, applied to
/// egui's text styles at start-up and, through [`fs`], to every explicit size.
/// ~~2.0 (maintainer, 2026-10-05: "all fonts twice the size")~~ **REVERTED to
/// 1.0 the same day** at the maintainer's request, pending a systematic style
/// settlement (gh:#586); this constant is the one hook for it.
pub const FONT_SCALE: f32 = 1.0;

/// An explicit font size, scaled by [`FONT_SCALE`].
pub fn fs(px: f32) -> f32 {
    px * FONT_SCALE
}

/// What the shared file picker was opened for. Every file or folder the user
/// chooses goes through it (crate HARD RULE: pickers, never typed paths).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    EndfFolder,
    KovanRoot,
    PngFolder,
    OpenRecipe,
    SaveRecipeAs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Picker,
    ModeChoice(ReactorType),
    HtgrChoice,
    Wizard,
}

/// Step 1's DEM pour: where it is, and what it produced.
pub struct DemUi {
    pub running: bool,
    pub stop: crate::dem::StopFlag,
    pub progress: Option<outram_park_fork_liggghts::htr10_fill::FillProgress>,
    /// Latest positions for drawing \[m\].
    pub preview: Vec<[f32; 3]>,
    /// The finished bed's centres \[m\], DEM frame.
    pub result: Option<Vec<[f64; 3]>>,
    /// The pour stopped by the user before it settled.
    pub stopped: bool,
    pub started_at: f64,
    pub view: crate::dem::DemView,
}

impl Default for DemUi {
    fn default() -> Self {
        Self {
            running: false,
            stop: Default::default(),
            progress: None,
            preview: Vec::new(),
            result: None,
            stopped: false,
            started_at: 0.0,
            view: crate::dem::DemView::new(),
        }
    }
}

/// Step 5's live state.
#[derive(Default)]
pub struct McState {
    /// Nuclide plan of the running load: item and seconds once done.
    pub items: Vec<(String, Option<f64>)>,
    pub current: Option<String>,
    pub loading_data: bool,
    pub running: bool,
    pub started_at: f64,
    pub planned_histories: u64,
    pub outcomes: Vec<KeffOutcome>,
    pub shown: usize,
    /// Generations of the run in progress, streamed by the physics thread.
    pub live: crate::engine::LiveGenerations,
}

pub struct App {
    pub geo: Link<Req, Ev>,
    pub phys: Link<Req, Ev>,
    /// The DEM pour's own thread (minutes of work; slices stay live).
    pub dem_link: Link<crate::dem::DemReq, crate::dem::DemEv>,
    pub dem: DemUi,
    /// Step 1's main view shows the DEM pour (else the geometry).
    pub show_dem: bool,
    pub screen: Screen,
    pub step: WizardStep,
    pub recipe: Recipe,
    pub preset: Recipe,
    // Step 0
    pub endf_dir: String,
    /// The one file picker, and what it is open for.
    pub dialog: egui_file_dialog::FileDialog,
    pub pick: Option<Pick>,
    pub scan: Option<(PathBuf, crate::engine::EndfLayout, Vec<TapeInfo>, Vec<NeededTape>)>,
    pub scanning: bool,
    // Geometry
    pub assembly: Option<AssemblyInfo>,
    pub core: Option<Arc<AssembledCore>>,
    pub assembling: bool,
    pub assembled_for: Option<(usize, usize)>,
    pub slice: SliceView,
    /// The Blender-like 3D viewport, and whether the main view shows it (else
    /// the 2D slice).
    pub view3d: crate::view3d::View3d,
    pub show_3d: bool,
    pub review_seen: Vec<&'static str>,
    // Step 5
    pub mc: McState,
    // Panes
    pub lit: Literature,
    pub lit_open: bool,
    pub settings_open: bool,
    pub results_open: bool,
    // Files
    pub recipe_path: String,
    pub out_dir: String,
    pub status: Vec<(f64, String, bool)>,
    pub t0: std::time::Instant,
    pub main_rect: egui::Rect,
    /// The step the main view and the literature pane were last set up for.
    pub shown_step: Option<WizardStep>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, recipe: Option<Recipe>) -> Self {
        cc.egui_ctx.all_styles_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= FONT_SCALE;
            }
        });
        let ctx = cc.egui_ctx.clone();
        let c2 = ctx.clone();
        let c3 = ctx.clone();
        let geo = start_native(Engine::default(), move || ctx.request_repaint());
        let phys = start_native(Engine::default(), move || c2.request_repaint());
        let dem_link = start_native(crate::dem::DemEngine, move || c3.request_repaint());
        let preset = preset::htr10();
        let loaded = recipe.is_some();
        let recipe = recipe.unwrap_or_else(|| preset.clone());
        let endf_dir = recipe.nuclear_data.endf_dir.clone();
        let mut app = Self {
            geo,
            phys,
            dem_link,
            dem: DemUi::default(),
            show_dem: false,
            screen: if loaded {
                Screen::Wizard
            } else {
                Screen::Picker
            },
            step: WizardStep::NuclearData,
            recipe,
            preset,
            endf_dir,
            dialog: egui_file_dialog::FileDialog::new()
                .add_file_filter_extensions("Recipes (kovan markdown)", vec!["md"])
                .default_file_filter("Recipes (kovan markdown)")
                .add_save_extension("Recipe (.md)", "md"),
            pick: None,
            scan: None,
            scanning: false,
            assembly: None,
            core: None,
            assembling: false,
            assembled_for: None,
            slice: SliceView::new(),
            view3d: crate::view3d::View3d::new(),
            show_3d: true,
            review_seen: Vec::new(),
            mc: McState::default(),
            lit: Literature::new(),
            lit_open: true,
            settings_open: true,
            results_open: true,
            recipe_path: "target/dhoby-ghaut_out/htr10_recipe.md".into(),
            out_dir: "target/dhoby-ghaut_out".into(),
            status: Vec::new(),
            t0: std::time::Instant::now(),
            main_rect: egui::Rect::NOTHING,
            shown_step: None,
        };
        if loaded {
            app.refresh_edited();
            app.enter_wizard();
        }
        app
    }

    pub fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    pub fn say(&mut self, msg: impl Into<String>, error: bool) {
        let t = self.now();
        self.status.push((t, msg.into(), error));
        if self.status.len() > 40 {
            self.status.remove(0);
        }
    }

    /// Start the wizard: scan the library and assemble the preset geometry
    /// in the background straight away, so neither waits for the reader.
    pub fn enter_wizard(&mut self) {
        self.screen = Screen::Wizard;
        self.step = WizardStep::NuclearData;
        self.scan_endf();
        self.assemble();
    }

    /// Open the shared picker for `what`, starting where its current value is.
    pub fn open_picker(&mut self, what: Pick) {
        let current = match what {
            Pick::EndfFolder => PathBuf::from(&self.endf_dir),
            Pick::KovanRoot => PathBuf::from(&self.lit.root),
            Pick::PngFolder => PathBuf::from(&self.out_dir),
            Pick::OpenRecipe | Pick::SaveRecipeAs => PathBuf::from(&self.recipe_path)
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default(),
        };
        let start = if current.is_dir() {
            current
        } else {
            std::env::current_dir().unwrap_or_default()
        };
        let cfg = self.dialog.config_mut();
        cfg.initial_directory = start;
        cfg.default_file_name = PathBuf::from(&self.recipe_path)
            .file_name()
            .map_or("recipe.md".into(), |f| f.to_string_lossy().into_owned());
        cfg.title = Some(
            match what {
                Pick::EndfFolder => "Pick the unzipped ENDF folder",
                Pick::KovanRoot => "Pick the kovan root (your literature library)",
                Pick::PngFolder => "Pick the folder for exported PNGs",
                Pick::OpenRecipe => "Open a recipe",
                Pick::SaveRecipeAs => "Save the recipe as",
            }
            .into(),
        );
        self.pick = Some(what);
        match what {
            Pick::EndfFolder | Pick::KovanRoot | Pick::PngFolder => self.dialog.pick_directory(),
            Pick::OpenRecipe => self.dialog.pick_file(),
            Pick::SaveRecipeAs => self.dialog.save_file(),
        }
    }

    /// Act on whatever the picker returned.
    fn picked(&mut self, path: PathBuf) {
        let Some(what) = self.pick.take() else { return };
        let text = path.display().to_string();
        match what {
            Pick::EndfFolder => {
                self.endf_dir = text.clone();
                self.recipe.nuclear_data.endf_dir = text;
                self.scan_endf();
            }
            Pick::KovanRoot => {
                self.lit.root = text;
                self.lit.clear();
            }
            Pick::PngFolder => self.out_dir = text,
            Pick::OpenRecipe => {
                self.recipe_path = text;
                self.load_recipe();
            }
            Pick::SaveRecipeAs => {
                self.recipe_path = text;
                self.save_recipe();
            }
        }
    }

    pub fn scan_endf(&mut self) {
        self.scanning = true;
        self.geo.send(Req::ScanEndf {
            dir: PathBuf::from(&self.endf_dir),
            needed: crate::engine::needed_tapes(),
        });
    }

    pub fn assemble(&mut self) {
        let key = (self.recipe.pebble_bed.rings, self.recipe.pebble_bed.layers);
        self.assembling = true;
        self.assembled_for = Some(key);
        self.core = None;
        self.slice.invalidate();
        self.view3d.invalidate();
        self.geo.send(Req::Assemble {
            rings: key.0,
            layers: key.1,
        });
    }

    pub fn geometry_stale(&self) -> bool {
        self.assembled_for != Some((self.recipe.pebble_bed.rings, self.recipe.pebble_bed.layers))
    }

    /// Mark the recipe edited if any MODEL input differs from the preset:
    /// Steps 0-4, the data temperature, rod insertion and ablations. Run
    /// statistics, threads, the tape folder path, the runs made and the review
    /// are how a model is run and checked, not what it is, so they do not
    /// count.
    pub fn refresh_edited(&mut self) {
        let (r, p) = (&self.recipe, &self.preset);
        let model_differs = r.nuclear_data.library != p.nuclear_data.library
            || r.nuclear_data.temperature_k != p.nuclear_data.temperature_k
            || r.pebble_bed != p.pebble_bed
            || r.pebble_design != p.pebble_design
            || r.reflector != p.reflector
            || r.inserts != p.inserts
            || r.monte_carlo.rod_insertion != p.monte_carlo.rod_insertion
            || r.monte_carlo.ablations != p.monte_carlo.ablations;
        self.recipe.header.edited = model_differs;
    }

    fn handle(&mut self, ctx: &egui::Context, events: Vec<Ev>) {
        for ev in events {
            if self.view3d.on_event(ctx, &ev) {
                continue;
            }
            if self.slice.on_event(ctx, &ev) {
                if let Some(p) = self.slice.at_preset {
                    if !self.review_seen.contains(&p) {
                        self.review_seen.push(p);
                    }
                }
                continue;
            }
            if self.lit.on_event(ctx, &ev) {
                continue;
            }
            match ev {
                Ev::Scan { dir, layout, tapes, needed } => {
                    self.scanning = false;
                    self.say(
                        format!("Scanned {}: {} tapes", dir.display(), tapes.len()),
                        false,
                    );
                    self.scan = Some((dir, layout, tapes, needed));
                }
                Ev::Assembled(info, core) => {
                    self.assembling = false;
                    self.say(
                        format!(
                            "Assembled HTR-10: {} cells, {} tiles in {:.1} s",
                            info.cells, info.tiles, info.seconds
                        ),
                        false,
                    );
                    self.phys.send(Req::UseCore(core.clone()));
                    self.core = Some(core);
                    self.assembly = Some(info);
                    self.slice.invalidate();
                    self.view3d.invalidate();
                    self.shown_step = None;
                }
                Ev::Exported(p) => self.say(format!("Wrote {}", p.display()), false),
                Ev::DataPlan(items) => {
                    self.mc.items = items.into_iter().map(|i| (i, None)).collect();
                    self.mc.loading_data = true;
                }
                Ev::Data(p) => match p {
                    nee_soon::htr10_rmc::data::LoadProgress::Started { item } => {
                        self.mc.current = Some(item)
                    }
                    nee_soon::htr10_rmc::data::LoadProgress::Finished { item, seconds } => {
                        if let Some(row) = self
                            .mc
                            .items
                            .iter_mut()
                            .find(|r| r.0 == item && r.1.is_none())
                        {
                            row.1 = Some(seconds);
                        }
                        self.mc.current = None;
                    }
                },
                Ev::DataReady { seconds, cached } => {
                    self.mc.loading_data = false;
                    if cached {
                        self.say(
                            "Nuclear data reused from the previous run (same temperature)",
                            false,
                        );
                    } else {
                        self.say(format!("Nuclear data processed in {seconds:.1} s"), false);
                    }
                }
                Ev::KeffStarted { planned_histories } => {
                    self.mc.started_at = self.now();
                    self.mc.planned_histories = planned_histories;
                }
                Ev::KeffDone(o) => {
                    self.mc.running = false;
                    self.say(
                        format!(
                            "{}: k = {:.5} ± {:.5} ({:.1} s)",
                            o.label, o.k, o.sigma, o.transport_s
                        ),
                        false,
                    );
                    let mc = &self.recipe.monte_carlo;
                    self.recipe
                        .monte_carlo
                        .runs
                        .push(dhoby_ghaut::workbench::recipe::RunRecord {
                            label: o.label.clone(),
                            rod_insertion: mc.rod_insertion,
                            temperature_k: self.recipe.nuclear_data.temperature_k,
                            particles: mc.particles,
                            inactive: mc.inactive,
                            active: mc.active,
                            seed: mc.seed,
                            k: o.k,
                            sigma: o.sigma,
                            transport_s: o.transport_s,
                        });
                    self.mc.outcomes.push(o);
                    self.mc.shown = self.mc.outcomes.len() - 1;
                }
                Ev::Error(e) => {
                    self.mc.running = false;
                    self.mc.loading_data = false;
                    self.lit.loading = false;
                    self.say(e, true);
                }
                Ev::Slice { .. } | Ev::Page { .. } | Ev::View3d { .. } => {}
            }
        }
    }

    /// Start a fresh DEM pour with Step 1's settings.
    pub fn start_dem(&mut self) {
        use outram_park_fork_liggghts::compute::ThreadCount;
        use outram_park_fork_liggghts::htr10_fill::Htr10FillSettings;
        use uom::si::f64::Pressure;
        use uom::si::pressure::pascal;
        let Some(d) = self.recipe.pebble_bed.dem.clone() else { return };
        let settings = Htr10FillSettings {
            n_pebbles: d.n_pebbles,
            friction: d.friction,
            rolling_friction: d.rolling_friction,
            youngs_modulus: Pressure::new::<pascal>(d.youngs_modulus_pa),
            seed: d.seed,
            threads: ThreadCount::Fixed(self.recipe.monte_carlo.threads.max(1)),
            ..Htr10FillSettings::default()
        };
        if let Ok(mut s) = self.dem.stop.write() {
            *s = false;
        }
        self.dem.running = true;
        self.dem.stopped = false;
        self.dem.result = None;
        self.dem.progress = None;
        self.dem.started_at = self.now();
        self.show_dem = true;
        self.dem_link.send(crate::dem::DemReq::Run { settings, chunk: 1000, stop: self.dem.stop.clone() });
        self.say(format!("DEM pour started: {} pebbles", d.n_pebbles), false);
    }

    fn handle_dem(&mut self, events: Vec<crate::dem::DemEv>) {
        use crate::dem::DemEv;
        for ev in events {
            match ev {
                DemEv::Progress { progress, centres } => {
                    self.dem.progress = Some(progress);
                    self.dem.preview = centres;
                }
                DemEv::Done { progress, centres, stopped } => {
                    self.dem.running = false;
                    self.dem.stopped = stopped;
                    self.dem.progress = Some(progress);
                    self.dem.preview = centres.iter().map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect();
                    if let Some(d) = self.recipe.pebble_bed.dem.as_mut() {
                        d.settled_steps = progress.settled.then_some(progress.steps);
                        d.phi_whole_core = progress.settled.then_some(progress.phi_whole_core);
                    }
                    let msg = if progress.settled {
                        format!("DEM pour settled after {} steps: phi {:.4}", progress.steps, progress.phi_whole_core)
                    } else if stopped {
                        format!("DEM pour stopped at step {} (not settled)", progress.steps)
                    } else {
                        format!("DEM pour hit its step cap at {} without settling", progress.steps)
                    };
                    self.say(msg, !progress.settled);
                    self.dem.result = Some(centres);
                }
                DemEv::Error(e) => {
                    self.dem.running = false;
                    self.say(e, true);
                }
            }
        }
        if self.dem.running {
            // The elapsed clock keeps moving between chunks.
        }
    }

    pub fn save_recipe(&mut self) {
        self.refresh_edited();
        let path = PathBuf::from(&self.recipe_path);
        let md = match self.recipe.to_markdown(&now_rfc3339()) {
            Ok(m) => m,
            Err(e) => return self.say(e.to_string(), true),
        };
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        match std::fs::write(&path, md) {
            Ok(()) => self.say(format!("Saved recipe {}", path.display()), false),
            Err(e) => self.say(format!("Could not save {}: {e}", path.display()), true),
        }
    }

    pub fn load_recipe(&mut self) {
        let path = PathBuf::from(&self.recipe_path);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return self.say(format!("Could not read {}: {e}", path.display()), true),
        };
        match Recipe::from_markdown(&text) {
            Ok(r) => {
                self.recipe = r;
                self.endf_dir = self.recipe.nuclear_data.endf_dir.clone();
                self.refresh_edited();
                self.say(format!("Loaded recipe {}", path.display()), false);
                self.enter_wizard();
            }
            Err(e) => self.say(format!("{}: {e}", path.display()), true),
        }
    }

    // ─── Screens before the wizard ──────────────────────────────────────────

    fn picker(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.heading(RichText::new("DHOBY GHAUT: what do you want to build?").size(crate::app::fs(26.0)));
            ui.label("A guided high-fidelity build. Research, education and V&V only: not for facility operation, licensing or safety decisions.");
        });
        ui.add_space(10.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for g in Generation::ALL {
                ui.label(RichText::new(g.title()).size(crate::app::fs(18.0)).strong());
                let types: Vec<ReactorType> = ReactorType::ALL
                    .into_iter()
                    .filter(|t| t.generation() == g)
                    .collect();
                let per_row = ((ui.available_width() / 290.0).floor() as usize).max(1);
                for row in types.chunks(per_row) {
                    ui.horizontal(|ui| {
                        for &t in row {
                            let s = t.support();
                            let colour = match s {
                                Support::Wizard => Color32::from_rgb(30, 120, 60),
                                Support::Partial => Color32::from_rgb(170, 110, 0),
                                Support::NotYet => Color32::GRAY,
                            };
                            let card = egui::Frame::group(ui.style()).inner_margin(10.0);
                            card.show(ui, |ui| {
                                ui.set_width(260.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(t.label())
                                            .size(crate::app::fs(17.0))
                                            .strong(),
                                    );
                                    ui.colored_label(colour, s.badge());
                                    ui.small(t.note());
                                    if ui
                                        .add_enabled(
                                            s == Support::Wizard,
                                            egui::Button::new("Build this »"),
                                        )
                                        .clicked()
                                    {
                                        self.recipe.header.reactor = t.key().into();
                                        self.screen = Screen::ModeChoice(t);
                                    }
                                });
                            });
                        }
                    });
                }
                ui.add_space(8.0);
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Or open a recipe:");
                if ui.button("Open recipe...").clicked() {
                    self.open_picker(Pick::OpenRecipe);
                }
            });
        });
    }

    fn mode_choice(&mut self, ui: &mut egui::Ui, t: ReactorType) {
        ui.heading(format!("{}: how do you want to start?", t.label()));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            egui::Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
                ui.set_width(340.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Basic").size(crate::app::fs(20.0)).strong());
                    ui.label("Start from a pre-built model and modify from there. Every value prefilled, every value cited.");
                    if ui.button("Basic »").clicked() {
                        self.recipe.header.mode = Mode::Basic;
                        self.screen = Screen::HtgrChoice;
                    }
                });
            });
            egui::Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
                ui.set_width(340.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Advanced").size(crate::app::fs(20.0)).strong());
                    ui.label("Build from scratch, more customisable.");
                    ui.colored_label(Color32::GRAY, "Not built yet: Advanced mode details are deferred (gh:#561).");
                    ui.add_enabled(false, egui::Button::new("Advanced »"));
                });
            });
        });
        if ui.button("« Back").clicked() {
            self.screen = Screen::Picker;
        }
    }

    fn htgr_choice(&mut self, ui: &mut egui::Ui) {
        ui.heading("HTGR, Basic: which core?");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for c in [HtgrCore::Prismatic, HtgrCore::PebbleBed] {
                egui::Frame::group(ui.style())
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(c.label()).size(crate::app::fs(19.0)).strong());
                            ui.small(c.note());
                            if c == HtgrCore::PebbleBed {
                                ui.add_space(4.0);
                                ui.label(RichText::new("V&V status of this preset").strong());
                                ui.colored_label(Color32::from_rgb(170, 90, 0), preset::VV_STATUS);
                            }
                            if ui
                                .add_enabled(c.available(), egui::Button::new("Start »"))
                                .clicked()
                            {
                                self.enter_wizard();
                            }
                        });
                    });
            }
        });
        if ui.button("« Back").clicked() {
            self.screen = Screen::ModeChoice(ReactorType::Htgr);
        }
    }

    // ─── The wizard frame ───────────────────────────────────────────────────

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let i = WizardStep::ALL
            .iter()
            .position(|s| *s == self.step)
            .unwrap_or(0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    i > 0,
                    egui::Button::new(RichText::new("« Back").size(crate::app::fs(16.0))),
                )
                .clicked()
            {
                self.step = WizardStep::ALL[i - 1];
            }
            ui.label(
                RichText::new(self.step.title())
                    .size(crate::app::fs(20.0))
                    .strong(),
            );
            let next = WizardStep::ALL.get(i + 1).copied();
            let blocked = self.step == WizardStep::Review && !self.recipe.review.passed();
            if let Some(n) = next {
                let b = ui.add_enabled(
                    !blocked,
                    egui::Button::new(RichText::new("Next »").size(crate::app::fs(16.0))),
                );
                let b = if blocked {
                    b.on_disabled_hover_text("Look at every review view and confirm first")
                } else {
                    b
                };
                if b.clicked() {
                    self.step = n;
                }
            }
            ui.separator();
            ui.label(RichText::new(self.step.phase()).color(Color32::GRAY));
        });
        ui.horizontal_wrapped(|ui| {
            if self.recipe.header.edited {
                ui.colored_label(Color32::from_rgb(170, 90, 0), "derived from HTR-10: edited, no V&V standing")
                    .on_hover_text("A value differs from the preset, so the preset's V&V record no longer describes this model.");
            } else {
                ui.colored_label(Color32::from_rgb(30, 100, 150), "HTR-10 preset (TENTATIVE)")
                    .on_hover_text(preset::VV_STATUS);
            }
            ui.separator();
            ui.toggle_value(&mut self.lit_open, "Literature");
            ui.toggle_value(&mut self.settings_open, "Settings");
            ui.separator();
            ui.label("Recipe");
            if ui.button("Open...").clicked() {
                self.open_picker(Pick::OpenRecipe);
            }
            if ui.button("Save").on_hover_text(&self.recipe_path).clicked() {
                self.save_recipe();
            }
            if ui.button("Save as...").clicked() {
                self.open_picker(Pick::SaveRecipeAs);
            }
            let name = PathBuf::from(&self.recipe_path)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            ui.label(RichText::new(name).color(Color32::GRAY)).on_hover_text(&self.recipe_path);
        });
        ui.horizontal_wrapped(|ui| {
            for s in WizardStep::ALL {
                let label = match s.number() {
                    Some(n) => format!("{n}"),
                    None => "gate".to_string(),
                };
                let mut text = RichText::new(format!("{label} {}", s.name()));
                if !s.implemented() {
                    text = text.color(Color32::GRAY);
                }
                let reachable = s <= WizardStep::Review || self.recipe.review.passed();
                if ui
                    .add_enabled(reachable, egui::Button::selectable(s == self.step, text))
                    .clicked()
                {
                    self.step = s;
                }
            }
        });
        ui.label(self.step.guide());
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        if let Some((_, m, err)) = self.status.last() {
            let c = if *err {
                Color32::from_rgb(190, 30, 30)
            } else {
                Color32::DARK_GRAY
            };
            ui.colored_label(c, m);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut events = self.geo.drain();
        events.extend(self.phys.drain());
        self.handle(&ctx, events);
        let dem_events = self.dem_link.drain();
        self.handle_dem(dem_events);
        self.dialog.update(&ctx);
        if let Some(path) = self.dialog.take_picked() {
            self.picked(path);
        }
        if self.mc.running || self.assembling || self.scanning {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        match self.screen {
            Screen::Picker => {
                egui::CentralPanel::default().show(ui, |ui| self.picker(ui));
            }
            Screen::ModeChoice(t) => {
                egui::CentralPanel::default().show(ui, |ui| self.mode_choice(ui, t));
            }
            Screen::HtgrChoice => {
                egui::CentralPanel::default().show(ui, |ui| self.htgr_choice(ui));
            }
            Screen::Wizard => {
                egui::Panel::top("wizard_top").show(ui, |ui| self.top_bar(ui));
                egui::Panel::bottom("status").show(ui, |ui| self.status_line(ui));
                crate::steps_ui::panes(self, ui);
            }
        }
    }
}
