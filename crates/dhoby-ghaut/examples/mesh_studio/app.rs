//! Mesh Studio's page: one app for desktop and browser (gh:#1767), on
//! `bugis::web_demo` (via `dhoby_ghaut::web_demo`).
//!
//! - **Main view**: a cut through the mesh, one polygon per cut cell,
//!   coloured by boundary-layer index, non-orthogonality or skewness, with
//!   on-screen zoom buttons and a colour bar.
//! - **Side panel** (folds on a phone): the shape and the mesher's controls,
//!   which rebuild; the **stage stepper** and the cut, which only re-cut; and
//!   the **layer-growth slider**, which draws only on the page.
//! - **Bottom dock**: the build's report, the cells per layer and the
//!   pipeline's stage notes.
//!
//! The UI thread only draws: meshing and cutting run in the engine
//! (`crate::engine`), a thread natively and a Web Worker in the browser.
//!
//! URL query (or `--key value` natively): `shape=box|sphere|cylinder|subdivided-cube|pipe|fuel-channel|pebble`,
//! `stage=<label>`, `layers=<k>` (growth slider), `colour=layer|nonorth|skew`,
//! `axis=x|y|z`, `h=<cell size, m>`, `n=<layers>`, `first=<first layer, m>`,
//! `ratio=<layer growth ratio>`,
//! `strategy=backoff|cfmesh|snappy`, `dual=corner|point|centre`,
//! `mesh=tet|poly` (the mesh type; `n=0` for no boundary layers),
//! `tets=tetgen|carved` (the tet source, gh:#1796: TetGen's unstructured
//! tetrahedra, the page's default, or the carved structured grid),
//! `q=<TetGen radius-edge ratio>` (default 1.414), `vol=<TetGen max tet
//! volume, m³>` (default: a regular tetrahedron of the cell size,
//! `h³/(6√2)`), `at=<x,y cm>&zoom=<f>` (a view, applied once).

use bugis::web_demo::dock::{Dock, Edge};
use bugis::web_demo::field::{self, ColourMap, Scale};
use bugis::web_demo::link::Link;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::platform::{query_pairs, set_title};
use bugis::web_demo::view::{apply_zoom, scale_bar, zoom_buttons, View};
use egui::{Color32, Pos2, Rect, Stroke};
use outram_park_fork_cfmesh::pipeline::staged::PipelineStage;

use crate::engine::{Cut, Design, Event, Request, Summary, DUALS, STRATEGIES};
use crate::model::{Cleanup, GeomKind};
use crate::section::Colouring;

type MLink = Link<Request, Event>;

/// The colour drawn for a layer cell the growth slider has not reached yet:
/// it exists in the mesh, but the slider is showing the layers growing.
/// The narrowest a cell may be on screen \[px\] and still get an outline.
const MIN_OUTLINED_PX: f32 = 5.0;
const NOT_YET: Color32 = Color32::from_gray(72);

/// Narrowest central area \[px\] the report dock's text is drawn in.
const MIN_REPORT_WIDTH: f32 = 260.0;

pub struct MeshApp {
    ctx: egui::Context,
    link: Option<MLink>,
    panel: Panel,
    dock: Dock,
    view: View,
    design: Design,
    /// The design of the last build asked for (to tell when to rebuild).
    built_design: Option<Design>,
    next_id: u64,
    building: bool,
    build_started: f64,
    summary: Option<Summary>,
    cut: Option<Cut>,
    /// Index into `summary.stages`.
    stage: usize,
    axis: u8,
    /// Cut position as a fraction of the mesh's extent along `axis`.
    offset_frac: f64,
    colouring: Colouring,
    /// Growth slider: layer cells deeper than this are drawn as not yet grown.
    grow_upto: u32,
    /// `?at=x,y&zoom=f` from the URL: the in-plane point \[cm\] to centre
    /// on and the zoom over the fit, applied once after the first build.
    /// Used by the review images to show the thin wall layers.
    wanted_zoom: Option<([f64; 2], f64)>,
    growing: bool,
    last_grow_tick: f64,
    error: Option<String>,
    title: String,
    /// The stage label asked for by the URL, applied once a build lands.
    wanted_stage: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    export: Export,
}

/// Native polyMesh export through a folder picker (the crate's file-picker
/// rule: never a typed path). The export writes the build it is asked for,
/// meshed again on a thread, so the UI never waits.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Export {
    dialog: Option<egui_file_dialog::FileDialog>,
    folder: Option<std::path::PathBuf>,
    status: std::sync::Arc<std::sync::RwLock<String>>,
}

impl MeshApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        bugis::Theme::default().apply(&cc.egui_ctx);
        let q = query_pairs();
        let get = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone());
        let design = Design::from_query(&q);
        let colouring = get("colour")
            .and_then(|s| Colouring::from_slug(&s))
            .unwrap_or(Colouring::Layer);
        let axis = match get("axis").as_deref() {
            Some("x") => 0,
            Some("y") => 1,
            _ => 2,
        };
        let grow_upto = get("layers")
            .and_then(|s| s.parse().ok())
            .unwrap_or(u32::MAX);
        let wanted_zoom = parse_zoom(get("at").as_deref(), get("zoom").as_deref());

        // A worker that cannot start is shown on the page, not swallowed.
        let (link, link_error) = match start_link(&cc.egui_ctx) {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };

        let mut app = MeshApp {
            ctx: cc.egui_ctx.clone(),
            link,
            panel: Panel::default(),
            dock: Dock::new(Edge::Bottom, "Report"),
            view: View::new(100.0),
            design,
            built_design: None,
            next_id: 0,
            building: false,
            build_started: 0.0,
            summary: None,
            cut: None,
            stage: 0,
            axis,
            offset_frac: 0.55,
            colouring,
            grow_upto,
            wanted_zoom,
            growing: false,
            last_grow_tick: 0.0,
            error: link_error,
            title: String::new(),
            wanted_stage: get("stage"),
            #[cfg(not(target_arch = "wasm32"))]
            export: Export::default(),
        };
        app.rebuild();
        app
    }

    /// Abandon the build in progress: the worker is stopped (in the browser
    /// at once; natively its thread finishes the build unseen and exits) and
    /// a fresh one takes its place.
    fn cancel(&mut self) {
        if let Some(l) = self.link.take() {
            l.terminate();
        }
        self.building = false;
        self.built_design = None;
        match start_link(&self.ctx) {
            Ok(l) => {
                self.link = Some(l);
                self.error = Some("build cancelled".into());
            }
            Err(e) => self.error = Some(e),
        }
        self.summary = None;
        self.cut = None;
    }

    fn send(&self, req: Request) {
        if let Some(l) = &self.link {
            l.send(req);
        }
    }

    fn rebuild(&mut self) {
        if self.link.is_none() {
            return; // the worker never started: the page already says so
        }
        self.design = self.design.clone().clamped();
        self.next_id += 1;
        self.building = true;
        self.build_started = bugis::web_demo::platform::now_s();
        self.error = None;
        self.built_design = Some(self.design.clone());
        self.send(Request::Build {
            id: self.next_id,
            design: self.design.clone(),
        });
    }

    fn recut(&self) {
        let Some(s) = &self.summary else { return };
        let Some(st) = s.stages.get(self.stage) else {
            return;
        };
        let a = self.axis as usize;
        let offset = s.lo[a] + self.offset_frac * (s.hi[a] - s.lo[a]);
        self.send(Request::Section {
            id: s.id,
            stage: st.stage,
            axis: self.axis,
            offset,
            colouring: self.colouring,
        });
    }

    fn handle(&mut self, events: Vec<Event>) {
        for ev in events {
            match ev {
                Event::Built(s) if s.id == self.next_id => {
                    self.building = false;
                    self.stage = s.stages.len().saturating_sub(1);
                    if let Some(w) = self.wanted_stage.take() {
                        if let Some(i) = s
                            .stages
                            .iter()
                            .position(|x| x.stage.label() == w || stage_slug(x.stage) == w)
                        {
                            self.stage = i;
                        }
                    }
                    self.view = fitted_view(&s, self.axis);
                    self.summary = Some(s);
                    self.recut();
                }
                Event::Built(_) => {} // superseded
                Event::Section(c) if self.summary.as_ref().is_some_and(|s| s.id == c.id) => {
                    self.cut = Some(c)
                }
                Event::Section(_) => {}
                Event::Error(m) => {
                    self.building = false;
                    self.error = Some(m);
                }
            }
        }
    }

    fn max_layer(&self) -> u32 {
        self.summary
            .as_ref()
            .map_or(0, |s| s.layer_counts.len().saturating_sub(1) as u32)
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Volume-mesh a closed surface through cfMesh's pipeline (tetrahedra, then their polyhedral dual, then prism boundary layers), and look inside: every stage, every layer.");
        ui.add_space(6.0);

        // ── Shape and mesher: these rebuild ─────────────────────────────
        ui.strong("Shape");
        let mut d = self.design.clone();
        let before = d.kind;
        egui::ComboBox::from_id_salt("shape")
            .selected_text(d.kind.label())
            .show_ui(ui, |ui| {
                for k in GeomKind::ALL {
                    ui.selectable_value(&mut d.kind, k, k.label());
                }
            });
        if d.kind != before {
            d.cell_size = d.kind.default_cell_size(d.size);
            d.dual = d.kind.default_dual();
        }
        if d.kind.is_reactor() {
            ui.label(egui::RichText::new("Illustrative proportions, not a particular reactor's. Layers grow on the walls only; inlet and outlet get none.").small().weak());
        }
        egui::ComboBox::from_id_salt("cleanup")
            .selected_text(d.cleanup.label())
            .show_ui(ui, |ui| {
                for c in Cleanup::ALL {
                    ui.selectable_value(&mut d.cleanup, c, c.label());
                }
            });
        ui.add(egui::Slider::new(&mut d.size, 0.5..=5.0).text("size [m]"));
        let span = d.span();
        ui.add(
            egui::Slider::new(
                &mut d.cell_size,
                span / Design::MIN_CELLS_ACROSS..=span / 2.0,
            )
            .text("cell size [m]"),
        );

        ui.add_space(6.0);
        ui.strong("Mesh type");
        ui.radio_value(&mut d.polyhedral, false, "tetrahedra");
        ui.radio_value(
            &mut d.polyhedral,
            true,
            "polyhedra (dual of the tetrahedra)",
        );

        ui.add_space(6.0);
        ui.strong("Tetrahedra");
        ui.radio_value(&mut d.tetgen, true, "TetGen (unstructured)");
        ui.radio_value(&mut d.tetgen, false, "carved grid (structured)");
        if d.tetgen {
            ui.add(
                egui::Slider::new(&mut d.radius_edge, 1.1..=4.0).text("quality: radius-edge ratio"),
            );
            let mut derived = d.max_volume.is_none();
            ui.checkbox(&mut derived, "max tet volume from the cell size");
            let v_lo = Design::regular_tet_volume(span / Design::MIN_CELLS_ACROSS);
            let v_hi = Design::regular_tet_volume(span);
            if derived {
                d.max_volume = None;
            } else {
                let mut v = d.max_volume.unwrap_or_else(|| d.tet_volume());
                ui.add(
                    egui::Slider::new(&mut v, v_lo..=v_hi)
                        .logarithmic(true)
                        .text("max tet volume [m³]"),
                );
                d.max_volume = Some(v);
            }
            ui.label(
                egui::RichText::new(format!(
                    "TetGen -pq{:.3}a{:.2e}: carve, snap and the Delaunay flips are not run.",
                    d.radius_edge,
                    d.tet_volume()
                ))
                .small()
                .weak(),
            );
        }

        ui.add_space(6.0);
        ui.strong("Boundary layers");
        ui.add(
            egui::Slider::new(&mut d.n_layers, 0..=Design::MAX_LAYERS).text("layers (0 = none)"),
        );
        ui.add_enabled_ui(d.n_layers > 0, |ui| {
            ui.add(
                egui::Slider::new(&mut d.first_layer, 1e-4 * span..=0.1 * span)
                    .logarithmic(true)
                    .text("first layer [m]"),
            );
            ui.add(egui::Slider::new(&mut d.expansion, 1.0..=2.0).text("growth ratio"));
            egui::ComboBox::from_id_salt("strategy")
                .selected_text(strategy_label(d.strategy))
                .show_ui(ui, |ui| {
                    for s in STRATEGIES {
                        ui.selectable_value(&mut d.strategy, s, strategy_label(s));
                    }
                });
        });
        ui.add_enabled_ui(d.polyhedral, |ui| {
            egui::ComboBox::from_id_salt("dual")
                .selected_text(dual_label(d.dual))
                .show_ui(ui, |ui| {
                    for k in DUALS {
                        ui.selectable_value(&mut d.dual, k, dual_label(k));
                    }
                });
        });
        if let Some(w) = strategy_note(&d) {
            ui.label(egui::RichText::new(w).small().weak());
        }
        self.design = d;
        let stale = self.built_design.as_ref() != Some(&self.design.clone().clamped());
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.building && stale, egui::Button::new("Mesh it"))
                .clicked()
            {
                self.rebuild();
            }
            if self.building {
                ui.spinner();
                ui.label(format!(
                    "meshing… {:.0} s",
                    bugis::web_demo::platform::now_s() - self.build_started
                ));
                if ui.button("Cancel").clicked() {
                    self.cancel();
                }
            } else if stale {
                ui.label("controls changed");
            }
        });

        // ── Stages and the cut: these only re-cut ────────────────────────
        let Some(s) = self.summary.clone() else {
            return;
        };
        ui.separator();
        ui.strong("Pipeline stage");
        let mut stage = self.stage;
        for (i, st) in s.stages.iter().enumerate() {
            let text = format!(
                "{}  {}{}",
                i + 1,
                st.stage.label(),
                if st.applied { "" } else { "  (skipped)" }
            );
            ui.radio_value(&mut stage, i, text)
                .on_hover_text(format!("{} cells after this stage", st.n_cells));
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(stage > 0, egui::Button::new("◀ prev"))
                .clicked()
            {
                stage -= 1;
            }
            if ui
                .add_enabled(stage + 1 < s.stages.len(), egui::Button::new("next ▶"))
                .clicked()
            {
                stage += 1;
            }
        });

        ui.add_space(6.0);
        ui.strong("Cut");
        let mut axis = self.axis;
        ui.horizontal(|ui| {
            for (k, name) in ["x", "y", "z"].into_iter().enumerate() {
                ui.radio_value(&mut axis, k as u8, format!("{name} = const"));
            }
        });
        let mut frac = self.offset_frac;
        ui.add(
            egui::Slider::new(&mut frac, 0.02..=0.98)
                .text("position")
                .show_value(false),
        );
        let mut colouring = self.colouring;
        egui::ComboBox::from_id_salt("colour")
            .selected_text(colouring.label())
            .show_ui(ui, |ui| {
                for c in Colouring::ALL {
                    ui.selectable_value(&mut colouring, c, c.label());
                }
            });
        if (stage, axis, colouring) != (self.stage, self.axis, self.colouring)
            || (frac - self.offset_frac).abs() > 1e-9
        {
            if axis != self.axis {
                // A new plane has a different in-plane centre: refit on it.
                self.view = fitted_view(&s, axis);
            }
            self.stage = stage;
            self.axis = axis;
            self.colouring = colouring;
            self.offset_frac = frac;
            self.recut();
        }

        // ── Layer growth: drawing only ───────────────────────────────────
        let max = self.max_layer();
        if max > 0
            && self.colouring == Colouring::Layer
            && s.stages.get(self.stage).is_some_and(|x| x.has_layers)
        {
            ui.add_space(6.0);
            ui.strong("Layer growth");
            let mut k = self.grow_upto.min(max);
            ui.add(egui::Slider::new(&mut k, 0..=max).text("layers grown"));
            self.grow_upto = k;
            if ui
                .button(if self.growing {
                    "stop"
                } else {
                    "▶ grow from the wall"
                })
                .clicked()
            {
                self.growing = !self.growing;
                if self.growing {
                    self.grow_upto = 0;
                    self.last_grow_tick = bugis::web_demo::platform::now_s();
                }
            }
            ui.label(
                egui::RichText::new("The layers are built in one pass; this replays them from the wall inward. A grey cell is a layer cell not yet shown.")
                    .small()
                    .weak(),
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.export_ui(ui);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn export_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Export (desktop)");
        if ui.button("Choose a polyMesh folder…").clicked() {
            let mut d = egui_file_dialog::FileDialog::new();
            d.pick_directory();
            self.export.dialog = Some(d);
        }
        if let Some(d) = self.export.dialog.as_mut() {
            d.update(ui.ctx());
            if let Some(p) = d.take_picked() {
                self.export.folder = Some(p.to_path_buf());
                self.export.dialog = None;
            }
        }
        if let Some(folder) = self.export.folder.clone() {
            ui.label(
                egui::RichText::new(folder.display().to_string())
                    .monospace()
                    .small(),
            );
            if ui
                .add_enabled(self.summary.is_some(), egui::Button::new("Write polyMesh"))
                .clicked()
            {
                let params = self.design.params();
                let status = self.export.status.clone();
                if let Ok(mut s) = status.write() {
                    *s = "writing…".into();
                }
                std::thread::spawn(move || {
                    let msg = match crate::model::build(&params) {
                        Ok(b) => match outram_blender::foam_mesh::export_polymesh(&b.mesh, &folder)
                        {
                            Ok(()) => format!(
                                "wrote {} cells to {}",
                                b.report.cell_count,
                                folder.display()
                            ),
                            Err(e) => format!("export failed: {e}"),
                        },
                        Err(e) => format!("meshing failed: {e}"),
                    };
                    if let Ok(mut s) = status.write() {
                        *s = msg;
                    }
                });
            }
        }
        if let Ok(s) = self.export.status.read() {
            if !s.is_empty() {
                ui.label(egui::RichText::new(s.as_str()).small());
            }
        }
    }

    fn main_view(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let resp = ui.interact(
            rect,
            ui.id().with("mesh_view"),
            egui::Sense::click_and_drag(),
        );
        self.view.handle_input(ui, &resp);
        if !self.view.fitted {
            self.view.fit(rect);
        }
        if self.cut.is_some() {
            if let Some((at, f)) = self.wanted_zoom.take() {
                self.view.centre = at;
                self.view.scale = self.view.fit_scale(rect) * f.clamp(0.5, 600.0);
                self.view.touched = true;
            }
        }

        if let Some(e) = &self.error {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                e,
                egui::FontId::proportional(15.0),
                Color32::LIGHT_RED,
            );
        }
        let not_run = self
            .summary
            .as_ref()
            .and_then(|s| s.stages.get(self.stage))
            .is_some_and(|st| st.n_cells == 0);
        if not_run && self.cut.is_some() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "This stage was not run: TetGen's tetrahedra replace it.",
                egui::FontId::proportional(16.0),
                ui.visuals().weak_text_color(),
            );
            return;
        }
        let Some(c) = &self.cut else {
            let msg = if self.building {
                "meshing…"
            } else {
                "no mesh yet"
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                egui::FontId::proportional(18.0),
                ui.visuals().weak_text_color(),
            );
            return;
        };

        let polys: Vec<Vec<Pos2>> = c
            .polygons
            .iter()
            .map(|p| {
                p.iter()
                    .map(|q| self.view.to_screen(rect, q[0] * 100.0, q[1] * 100.0))
                    .collect()
            })
            .collect();
        let map = match c.colouring {
            Colouring::Layer => ColourMap::Batlow,
            _ => ColourMap::Lajolla,
        };
        let scale = Scale {
            min: c.min,
            max: if c.max > c.min { c.max } else { c.min + 1.0 },
            map,
        };
        let showing_growth = c.colouring == Colouring::Layer && c.max > 0.0;
        let upto = f64::from(self.grow_upto);
        let mut mesh = egui::Mesh::default();
        for (poly, &v) in polys.iter().zip(&c.values) {
            if poly.len() < 3 {
                continue;
            }
            let colour = if showing_growth && v > upto {
                NOT_YET
            } else {
                scale.colour(v)
            };
            let base = mesh.vertices.len() as u32;
            for &p in poly {
                mesh.colored_vertex(p, colour);
            }
            for k in 1..poly.len() as u32 - 1 {
                mesh.add_triangle(base, base + k, base + k + 1);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
        // Outline only the cells that are wide enough on screen for an
        // outline to leave their colour visible. A boundary-layer cell is a
        // thin sliver against the wall; at the fitted zoom it is 1-2 px
        // across, and a 0.6 px outline on both sides would hide it (seen in
        // the first review images, gh:#1771). Zoom in and the outlines
        // return.
        let edge = Stroke::new(0.6, Color32::from_black_alpha(140));
        for poly in &polys {
            if poly.len() >= 3 && min_screen_width(poly) >= MIN_OUTLINED_PX {
                painter.add(egui::Shape::closed_line(poly.clone(), edge));
            }
        }

        let bar = Rect::from_min_size(
            rect.left_bottom() + egui::vec2(12.0, -58.0),
            egui::vec2(220.0, 14.0),
        );
        field::colour_bar(&painter, bar, &scale, c.colouring.label(), 12.0);
        scale_bar(&painter, rect, &self.view);
        if let Some(z) = zoom_buttons(ui, rect) {
            apply_zoom(&mut self.view, rect, z);
        }
    }

    fn dock_body(&self, ui: &mut egui::Ui, body: Rect) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body.shrink(8.0)));
        egui::ScrollArea::vertical().id_salt("report").show(&mut child, |ui| {
            let Some(s) = &self.summary else {
                ui.label("No mesh yet.");
                return;
            };
            ui.label(format!(
                "{} cells · {:.4} m³ · meshed in {:.2} s · max non-orthogonality {:.1}° · max skewness {:.2} · {} negative cells",
                s.cells, s.volume, s.build_s, s.max_non_orth_deg, s.max_skewness, s.negative_cells
            ));
            if s.layer_counts.len() > 1 {
                let per: Vec<String> = s.layer_counts.iter().enumerate().skip(1).map(|(k, n)| format!("layer {k}: {n}")).collect();
                ui.label(format!("core {} · {}", s.layer_counts[0], per.join(" · ")));
                let [asked, lo, med, hi] = s.layer_thickness;
                ui.label(format!(
                    "Wall layers total {:.2} / {:.2} / {:.2} mm (least / median / most per column); the design asks for {:.2} mm.",
                    lo * 1e3, med * 1e3, hi * 1e3, asked * 1e3
                ));
            } else {
                ui.label("No boundary-layer cells in this mesh.");
            }
            if let Some(st) = s.stages.get(self.stage) {
                ui.label(format!("Showing stage {} ({}), {} cells{}.", self.stage + 1, st.stage.label(), st.n_cells, if st.applied { "" } else { ", skipped by the pipeline" }));
            }
            if !s.notes.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&s.notes).small().monospace());
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Education, research and V&V only. Untrusted AI-assisted draft pending human review.").small().weak());
        });
    }
}

/// Start the engine: a thread natively, a Web Worker in the browser.
/// Start-up glue (exempt from the reaching-test rule).
fn start_link(ctx: &egui::Context) -> Result<MLink, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let c = ctx.clone();
        Ok(bugis::web_demo::link::start_native(
            crate::engine::Engine::default(),
            move || c.request_repaint(),
        ))
    }
    #[cfg(target_arch = "wasm32")]
    {
        bugis::web_demo::link::start_web::<Request, Event>(ctx.clone(), "./worker.js", Event::Error)
            .map_err(|e| format!("could not start the meshing worker: {e}"))
    }
}

fn stage_slug(s: PipelineStage) -> String {
    s.label().replace(' ', "-").to_lowercase()
}

fn strategy_label(s: outram_park_fork_cfmesh::layers::LayerQualityStrategy) -> &'static str {
    use outram_park_fork_cfmesh::layers::LayerQualityStrategy as L;
    match s {
        L::BackOffOnly => "back off (fast)",
        L::CfMeshOptimise => "cfMesh layer optimiser",
        L::SnappyCollapse => "snappy collapse",
    }
}

fn dual_label(d: outram_park_fork_cfmesh::dual::DualKind) -> &'static str {
    use outram_park_fork_cfmesh::dual::DualKind as D;
    match d {
        D::MedianPerCorner => "median dual, per corner",
        D::MedianPerPoint => "median dual, per point",
        D::CellCentre => "cell-centre dual (polyDualMesh)",
    }
}

/// What a reader should know before picking a layer strategy: measured
/// behaviour recorded in the cfMesh port (gh:#1215, #1240, #1765, #1796).
fn strategy_note(d: &Design) -> Option<&'static str> {
    use outram_park_fork_cfmesh::layers::LayerQualityStrategy as L;
    if d.n_layers == 0 {
        return None;
    }
    if d.tetgen && !d.polyhedral {
        // gh:#1796: measured on the pipe, fuel channel and pebble.
        return Some("No layer grows on TetGen's tetrahedra with back-off or cfMesh's optimiser (measured on all three reactor shapes; the report says so). Use the polyhedra for layers.");
    }
    match d.strategy {
        L::CfMeshOptimise if d.tetgen => Some(
            "cfMesh's layer optimiser on TetGen's polyhedra: measured natively (2026-11-26) about 20 s on the pipe, 60 s on the fuel channel, 90 s on the pebble, several times that in a browser. Cancel stops it. It is the only strategy here that grows layers on the pebble, and the one that grows a 20-layer stack.",
        ),
        L::CfMeshOptimise => Some(
            "cfMesh's layer optimiser is slow: measured natively (2026-11-26) 31 s on the pipe, 77 s on the pebble, 145 s on the fuel channel, 326 s on the cylinder at the finest cell size with 5 layers, and several times that in a browser. Cancel stops it. On the pebble it is the only strategy here that grows layers.",
        ),
        L::SnappyCollapse => Some("snappy's collapse removes most layers on these meshes: on a 1 cm pipe it kept 44 single-layer cells of 4260 asked (gh:#1765)."),
        L::BackOffOnly if d.kind == GeomKind::Pebble => {
            Some("back-off grows no layer on the pebble (every thickness tried left an invalid mesh); cfMesh's layer optimiser grows all of them, slowly.")
        }
        L::BackOffOnly if d.n_layers > 5 => Some("back-off shrinks every column alike and grew none of 20 layers on the pipe's TetGen polyhedra (it grows 3); cfMesh's layer optimiser grows all 20."),
        L::BackOffOnly => None,
    }
}

impl eframe::App for MeshApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        if self.growing {
            let now = bugis::web_demo::platform::now_s();
            if now - self.last_grow_tick > 0.7 {
                self.last_grow_tick = now;
                self.grow_upto += 1;
                if self.grow_upto >= self.max_layer() {
                    self.growing = false;
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let title = match &self.summary {
            Some(s) => format!(
                "Mesh Studio · {} · {} · {} cells",
                self.design.kind.slug(),
                if self.design.tetgen {
                    "TetGen"
                } else {
                    "carved"
                },
                s.cells
            ),
            None => "Mesh Studio".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Mesh Studio", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let full = ui.max_rect();
                let split = self.dock.split(full);
                self.main_view(ui, split.main);
                // With the side panel open on a phone the central area is a
                // sliver; a report squeezed into it wraps one word per line, so
                // it is left out until the panel folds.
                if let Some(body) = split.body.filter(|_| full.width() >= MIN_REPORT_WIDTH) {
                    ui.painter()
                        .rect_filled(body, 0.0, ui.visuals().extreme_bg_color);
                    self.dock_body(ui, body);
                }
                let _ = self.dock.toggle(ui, &split);
                self.panel.reopen_button(ui, split.main);
            });
        if self.building {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

/// The narrowest width of a convex polygon on screen \[px\]: the least, over
/// its edges, of the greatest distance of a vertex from that edge's line.
fn min_screen_width(poly: &[Pos2]) -> f32 {
    let n = poly.len();
    let mut best = f32::INFINITY;
    for i in 0..n {
        let a = poly[i];
        let d = poly[(i + 1) % n] - a;
        let len = d.length();
        if len <= f32::EPSILON {
            continue;
        }
        let far = poly
            .iter()
            .map(|p| ((p.x - a.x) * d.y - (p.y - a.y) * d.x).abs() / len)
            .fold(0.0, f32::max);
        best = best.min(far);
    }
    if best.is_finite() {
        best
    } else {
        0.0
    }
}

/// The in-plane centre \[cm\] of the box `lo..hi` \[m\] seen in the plane
/// normal to `axis` (in-plane axes as `outram_foam_mesh::section::Axis`).
fn in_plane_centre(lo: [f64; 3], hi: [f64; 3], axis: u8) -> [f64; 2] {
    let (a, b) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    [50.0 * (lo[a] + hi[a]), 50.0 * (lo[b] + hi[b])]
}

/// A view that fits the whole mesh, centred on it, for a cut normal to
/// `axis`: Reset returns here too.
fn fitted_view(s: &Summary, axis: u8) -> View {
    let span = (0..3).map(|k| s.hi[k] - s.lo[k]).fold(0.0, f64::max);
    View::new(0.55 * span * 100.0).with_home(in_plane_centre(s.lo, s.hi, axis))
}

/// `at=x,y` (cm) and `zoom=f` from the URL, when both are given and parse.
fn parse_zoom(at: Option<&str>, zoom: Option<&str>) -> Option<([f64; 2], f64)> {
    let mut it = at?.split(',').map(|v| v.trim().parse::<f64>());
    let (x, y) = (it.next()?.ok()?, it.next()?.ok()?);
    let f: f64 = zoom?.parse().ok()?;
    (f.is_finite() && f > 0.0).then_some(([x, y], f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thin_sliver_is_narrow_and_a_square_is_not() {
        let sq = [
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            Pos2::new(10.0, 10.0),
            Pos2::new(0.0, 10.0),
        ];
        assert!((min_screen_width(&sq) - 10.0).abs() < 1e-5);
        let sliver = [
            Pos2::new(0.0, 0.0),
            Pos2::new(40.0, 0.0),
            Pos2::new(40.0, 1.5),
            Pos2::new(0.0, 1.5),
        ];
        assert!((min_screen_width(&sliver) - 1.5).abs() < 1e-5);
        assert!(min_screen_width(&sliver) < MIN_OUTLINED_PX);
    }

    #[test]
    fn the_view_centres_on_the_mesh_in_each_plane() {
        let (lo, hi) = ([-1.0, -1.0, 0.0], [1.0, 1.0, 4.0]);
        assert_eq!(in_plane_centre(lo, hi, 2), [0.0, 0.0]);
        assert_eq!(in_plane_centre(lo, hi, 0), [0.0, 200.0]);
        assert_eq!(in_plane_centre(lo, hi, 1), [0.0, 200.0]);
    }

    #[test]
    fn the_zoom_query_needs_both_parts() {
        assert_eq!(
            parse_zoom(Some("3.5,-2"), Some("8")),
            Some(([3.5, -2.0], 8.0))
        );
        assert_eq!(parse_zoom(Some("3.5"), Some("8")), None);
        assert_eq!(parse_zoom(None, Some("8")), None);
        assert_eq!(parse_zoom(Some("1,2"), Some("-1")), None);
    }
}
