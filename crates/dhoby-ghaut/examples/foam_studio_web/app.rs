//! The page: the editor (primitive, size, position, patch tags, cells), the
//! run controls, and the 3-D viewport with the mesh and the fields on two
//! slices; a bottom dock with the residual history, the probe and the
//! velocity profile (for the double-pipe exchanger, gh:#1156: temperature on
//! the slices and the exchanger's numbers against ε-NTU). The UI thread only draws; meshing and the solve run in
//! the engine (a thread natively, the Web Worker in the browser).
//!
//! Drawing and widget code here is exempt from the reaching-test rule (GUI
//! drawing); its geometry comes from the tested `render` and
//! `bugis::web_demo::{field, residual}`, and its messages from the tested
//! `engine`.
use crate::engine::{CoilNo, Design, Event, Exch, Meshed, Mode, PipeMesh, Progress, Request, Section, Shape, Tag, TetDualSpec, CELL_PER_R_MAX, CELL_PER_R_MIN, DEFAULT_CELLS, MAX_CELLS, MAX_LAYERS, T_COLD_IN, T_HOT_IN, U_IN};
use crate::render::{plane_to_3d, project_section, Orbit};
use bugis::web_demo::dock::{Dock, Edge};
use bugis::web_demo::field::{self, ColourMap, Scale};
use bugis::web_demo::link::Link;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::platform::set_title;
use bugis::web_demo::residual::{self, History};
use bugis::web_demo::view::Zoom;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

type FLink = Link<Request, Event>;

/// Steps per request, at most: a request stays well under a second.
const MAX_CHUNK: usize = 8;

/// Which field the slices show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    Speed,
    Pressure,
    Temperature,
}

pub struct FoamApp {
    theme: bugis::Theme,
    link: Option<FLink>,
    link_error: Option<String>,
    panel: Panel,
    dock: Dock,
    cam: Orbit,
    pan_mode: bool,
    design: Design,
    probe: f64,
    reynolds: f64,
    mode: Mode,
    shown: Shown,
    show_mesh: bool,
    next_id: u32,
    /// The id of the mesh on screen, and whether the engine owes a reply.
    mesh: Option<Meshed>,
    meshing: bool,
    dirty: bool,
    running: bool,
    in_flight: bool,
    last: Option<Progress>,
    /// Steady-run progress: the largest fraction of the way, in decades, from
    /// the first change reported to the steady criterion.
    batch_frac: f64,
    first_change: f64,
    residuals: History,
    probe_hist: History,
    status: String,
    title: String,
    /// `?autostart`: run as soon as the first mesh arrives (browser checks).
    autostart: bool,
    /// The unstructured mesher's settings, kept while the structured mesh is
    /// chosen (gh:#1202).
    unstructured: TetDualSpec,
}

impl FoamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        bugis::Theme::default().apply(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<FLink, String> = {
            let ctx = cc.egui_ctx.clone();
            Ok(bugis::web_demo::link::start_native(crate::engine::Engine::default(), move || ctx.request_repaint()))
        };
        #[cfg(target_arch = "wasm32")]
        let link = bugis::web_demo::link::start_web::<Request, Event>(cc.egui_ctx.clone(), "./worker.js", Event::Error);
        let (link, link_error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        let design = Design::default();
        let mut app = Self {
            theme: bugis::Theme::default(),
            link,
            link_error,
            panel: Panel::default(),
            dock: Dock::new(Edge::Bottom, "Convergence"),
            cam: Self::camera_for(&design),
            pan_mode: false,
            design,
            probe: 0.75,
            reynolds: 100.0,
            mode: Mode::Steady,
            shown: Shown::Speed,
            show_mesh: true,
            next_id: 1,
            mesh: None,
            meshing: false,
            dirty: true,
            running: false,
            in_flight: false,
            last: None,
            batch_frac: 0.0,
            first_change: f64::NAN,
            residuals: History::new(&["max |ΔU|/U per L/U", "continuity", "max |ΔT| per pass / ΔT_in"], 600),
            probe_hist: History::new(&["probe |U| / U"], 600),
            status: String::new(),
            title: String::new(),
            autostart: bugis::web_demo::platform::autostart(),
            unstructured: TetDualSpec::default_for(0.01),
        };
        let q = bugis::web_demo::platform::query_pairs();
        if let Some(n) = bugis::web_demo::platform::query_value(&q, "cells").and_then(|v| v.parse::<usize>().ok()) {
            app.design.target_cells = n.clamp(1, MAX_CELLS);
        }
        if bugis::web_demo::platform::query_value(&q, "shape") == Some("exchanger") {
            app.design = Design { target_cells: app.design.target_cells, ..Design::exchanger() };
            app.shown = Shown::Temperature;
            app.reynolds = 30.0;
            app.cam = Self::camera_for(&app.design);
        }
        if bugis::web_demo::platform::query_value(&q, "shape") == Some("coil") {
            app.design = Design { target_cells: app.design.target_cells, ..Design::coil() };
            app.reynolds = 200.0;
            app.probe = 0.9;
            app.cam = Self::camera_for(&app.design);
        }
        if bugis::web_demo::platform::query_value(&q, "mode") == Some("transient") {
            app.mode = Mode::Transient;
        }
        // `?mesh=unstructured` (gh:#1202): the cylinder on the cfMesh mesh, for
        // links and browser checks; `&h=` cell size and `&layers=` in mm / count
        // (the default h 5 mm, 3 layers since gh:#1298; `h=4&layers=0` the
        // bare case that was the default before).
        if bugis::web_demo::platform::query_value(&q, "mesh") == Some("unstructured") {
            app.unstructured = app.unstructured.with_size_query(
                bugis::web_demo::platform::query_value(&q, "h"),
                bugis::web_demo::platform::query_value(&q, "layers"),
            );
            // `&dual=point|centre`, `&fp=corners`, `&layerq=cfmesh|snappy`
            // (gh:#1240 browser controls).
            app.unstructured = app.unstructured.with_query(
                bugis::web_demo::platform::query_value(&q, "dual"),
                bugis::web_demo::platform::query_value(&q, "fp"),
                bugis::web_demo::platform::query_value(&q, "layerq"),
            );
            // `&route=legacy|upstream` (gh:#1298): the pre-gh:#1298 cfMesh
            // route (the ablation) or upstream's sequence (the default).
            app.unstructured = app.unstructured.with_route_query(bugis::web_demo::platform::query_value(&q, "route"));
            if matches!(app.design.shape, Shape::Cylinder { .. }) {
                app.design.mesh = PipeMesh::Unstructured(app.unstructured);
            }
        }
        app.send_mesh();
        app
    }

    fn camera_for(d: &Design) -> Orbit {
        let [lo, hi] = d.shape.bounds();
        let c = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
        let r = 0.5 * ((0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f64>()).sqrt();
        Orbit::new(c, r)
    }

    fn send(&self, r: Request) {
        if let Some(l) = &self.link {
            l.send(r);
        }
    }

    fn send_mesh(&mut self) {
        if let Err(e) = crate::engine::check(&self.design) {
            self.status = e;
            return;
        }
        self.next_id += 1;
        self.running = false;
        self.in_flight = false;
        self.meshing = true;
        self.dirty = false;
        self.last = None;
        self.residuals.clear();
        self.probe_hist.clear();
        self.status = "meshing…".into();
        self.send(Request::Mesh { id: self.next_id, design: self.design, probe: self.probe });
    }

    fn start(&mut self) {
        if self.dirty || self.mesh.is_none() {
            self.send_mesh();
        }
        self.residuals.clear();
        self.probe_hist.clear();
        self.batch_frac = 0.0;
        self.first_change = f64::NAN;
        self.send(Request::Start { id: self.next_id, reynolds: self.reynolds, mode: self.mode });
        self.running = true;
        self.in_flight = true;
        self.status = "starting…".into();
    }

    fn chunk(&self) -> usize {
        match &self.last {
            Some(p) if p.ms_per_step > 0.0 => ((150.0 / p.ms_per_step).floor() as usize).clamp(1, MAX_CHUNK),
            _ => 1,
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for ev in events {
            match ev {
                Event::Meshed(me) if me.id == self.next_id => {
                    self.meshing = false;
                    self.status = format!("meshed: {} cells, max non-orthogonality {:.1}°", me.n_cells, me.max_non_ortho_deg);
                    self.mesh = Some(me);
                    if std::mem::take(&mut self.autostart) {
                        self.start();
                    }
                }
                Event::Cross { id, cross } if id == self.next_id => {
                    if let Some(m) = &mut self.mesh {
                        m.cross = cross;
                    }
                }
                Event::Progress(p) if p.id == self.next_id => {
                    self.in_flight = false;
                    if p.change.is_finite() {
                        if !self.first_change.is_finite() {
                            self.first_change = p.change;
                        }
                        let span = (self.first_change / crate::engine::STEADY).log10().max(1e-9);
                        let got = (self.first_change / p.change).log10() / span;
                        self.batch_frac = self.batch_frac.max(got.clamp(0.0, 1.0));
                        let x = if self.is_exchanger() { p.steps as f64 } else { p.time_s };
                        if p.phase == 1 {
                            self.residuals.push(x, &[f64::NAN, p.continuity, p.change]);
                        } else {
                            self.residuals.push(x, &[p.change, p.continuity]);
                        }
                        self.probe_hist.push(x, &[p.probe_u / U_IN]);
                    }
                    if p.steady && self.mode == Mode::Steady {
                        self.running = false;
                        self.batch_frac = 1.0;
                    }
                    self.status = if self.is_coil() && p.coil[CoilNo::F_RE_64].is_finite() {
                        let c = &p.coil;
                        format!(
                            "{} steps · t = {:.0} s · De {:.1} · f/f_straight {:.3} on the last turn ({:.3} the turn before) · Mishra & Gupta {:.3} · {:.0} ms/step{}",
                            p.steps,
                            p.time_s,
                            c[CoilNo::DEAN],
                            c[CoilNo::F_RE_64],
                            c[CoilNo::F_RE_64_BEFORE],
                            c[CoilNo::MISHRA_GUPTA],
                            p.ms_per_step,
                            if p.steady { " · steady" } else { "" }
                        )
                    } else if self.is_exchanger() {
                        let x = &p.exch;
                        if p.phase == 0 {
                            format!("flow: {} steps · out/in {:.9} · {:.0} ms/step (then the flow is frozen and the temperatures iterated)", p.steps, p.outlet_over_inlet, p.ms_per_step)
                        } else {
                            format!(
                                "energy: step {} · T_hot,out {:.2} K · T_cold,out {:.2} K · ε {:.4} (ε-NTU {:.4}) · Q {:.3} W{}",
                                p.steps,
                                x[Exch::T_HOT_OUT],
                                x[Exch::T_COLD_OUT],
                                x[Exch::EPS],
                                x[Exch::EPS_NTU],
                                x[Exch::Q_HOT],
                                if p.steady { " · steady" } else { "" }
                            )
                        }
                    } else {
                        format!(
                            "{} steps · t = {:.1} s · Δp = {:.3e} m²/s² · out/in {:.9} · {:.0} ms/step{}",
                            p.steps,
                            p.time_s,
                            p.dp,
                            p.outlet_over_inlet,
                            p.ms_per_step,
                            if p.steady { " · steady" } else { "" }
                        )
                    };
                    self.last = Some(p);
                }
                Event::Error(m) => {
                    self.status = m;
                    self.running = false;
                    self.in_flight = false;
                    self.meshing = false;
                }
                _ => {}
            }
        }
        if self.running && !self.in_flight {
            let keep = match (&self.last, self.mode) {
                (Some(p), _) if self.is_exchanger() => !p.steady,
                (Some(p), Mode::Steady) => !p.steady,
                _ => true,
            };
            if keep {
                self.send(Request::Step { id: self.next_id, steps: self.chunk() });
                self.in_flight = true;
            } else {
                self.running = false;
            }
        }
    }

    fn is_exchanger(&self) -> bool {
        matches!(self.design.shape, Shape::DoublePipe { .. })
    }

    fn is_coil(&self) -> bool {
        matches!(self.design.shape, Shape::Coil { .. })
    }

    /// The side panel. GUI (exempt).
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        // Under the heading, where every other web demo puts it (2026-11-05).
        bugis::selector(ui, &mut self.theme);
        ui.separator();
        let mut changed = false;
        ui.label(RichText::new("1 · Shape").strong());
        ui.horizontal_wrapped(|ui| {
            let cyl = matches!(self.design.shape, Shape::Cylinder { .. });
            let bx = matches!(self.design.shape, Shape::Box { .. });
            let ex = self.is_exchanger();
            let co = self.is_coil();
            if ui.selectable_label(cyl, "Cylinder (pipe)").clicked() && !cyl {
                self.design = Design { target_cells: self.design.target_cells, ..Design::default() };
                self.shown = Shown::Speed;
                self.reynolds = 100.0;
                changed = true;
            }
            if ui.selectable_label(bx, "Box (duct)").clicked() && !bx {
                self.design.shape = Shape::Box { size: [0.02, 0.02, 0.2], centre: [0.0, 0.0, 0.1] };
                self.design.tags = [Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Inlet, Tag::Outlet];
                self.shown = Shown::Speed;
                changed = true;
            }
            if ui.selectable_label(ex, "Double pipe (heat exchanger)").clicked() && !ex {
                self.design = Design { target_cells: self.design.target_cells, ..Design::exchanger() };
                self.shown = Shown::Temperature;
                self.reynolds = 30.0;
                changed = true;
            }
            if ui.selectable_label(co, "Helical coil section").clicked() && !co {
                self.design = Design { target_cells: self.design.target_cells, ..Design::coil() };
                self.shown = Shown::Speed;
                self.reynolds = 200.0;
                self.probe = 0.9;
                changed = true;
            }
        });
        let mm = |ui: &mut egui::Ui, v: &mut f64, lo: f64, hi: f64, label: &str| {
            let mut x = *v * 1e3;
            let r = ui.add(egui::Slider::new(&mut x, lo..=hi).text(label).suffix(" mm"));
            *v = x * 1e-3;
            r.changed()
        };
        match &mut self.design.shape {
            Shape::Cylinder { radius, length, centre } => {
                ui.label("Scale");
                changed |= mm(ui, radius, 2.0, 30.0, "radius");
                changed |= mm(ui, length, 10.0, 400.0, "length");
                ui.label("Move (centre)");
                for (i, n) in ["x", "y", "z"].iter().enumerate() {
                    changed |= mm(ui, &mut centre[i], -200.0, 400.0, n);
                }
            }
            Shape::Box { size, centre } => {
                ui.label("Scale");
                for (i, n) in ["size x", "size y", "size z"].iter().enumerate() {
                    changed |= mm(ui, &mut size[i], 2.0, 400.0, n);
                }
                ui.label("Move (centre)");
                for (i, n) in ["x", "y", "z"].iter().enumerate() {
                    changed |= mm(ui, &mut centre[i], -200.0, 400.0, n);
                }
            }
            Shape::Coil { tube_radius, coil_diameter, pitch, turns, centre: _ } => {
                ui.label("Coil");
                changed |= mm(ui, tube_radius, 3.0, 8.0, "tube radius");
                changed |= mm(ui, coil_diameter, 40.0, 300.0, "coil diameter D_w");
                changed |= mm(ui, pitch, 0.0, 60.0, "pitch (rise per turn)");
                changed |= ui.add(egui::Slider::new(turns, 0.25..=4.0).step_by(0.25).text("turns")).changed();
                let d = 2.0 * *tube_radius;
                let dc = *coil_diameter * (1.0 + (*pitch / (std::f64::consts::PI * *coil_diameter)).powi(2));
                ui.label(RichText::new(format!("d/D_c = {:.4} · De = Re √(d/D_c) = {:.1}", d / dc, self.reynolds * (d / dc).sqrt())).small());
            }
            Shape::DoublePipe { r_tube, wall, r_shell, length, centre } => {
                ui.label("Scale");
                changed |= mm(ui, r_tube, 2.0, 20.0, "tube radius");
                changed |= mm(ui, wall, 0.2, 5.0, "tube wall");
                changed |= mm(ui, r_shell, 3.0, 40.0, "shell radius");
                changed |= mm(ui, length, 20.0, 400.0, "length");
                ui.label("Move (centre)");
                for (i, n) in ["x", "y", "z"].iter().enumerate() {
                    changed |= mm(ui, &mut centre[i], -200.0, 400.0, n);
                }
            }
        }
        ui.separator();
        ui.label(RichText::new("2 · Patches").strong());
        if self.is_exchanger() {
            ui.label(
                RichText::new(format!(
                    "Fixed, counterflow: hot fluid in the tube enters at z− at {T_HOT_IN:.0} K; cold fluid in the annulus enters at z+ at {T_COLD_IN:.0} K; \
                     both outlets p = 0; the tube wall (steel-like, k = 16 W/(m K)) conducts and couples the two; the shell is adiabatic. \
                     Water-like fluids (Pr 6.7), equal flow rates."
                ))
                .small(),
            );
        }
        let names = self.design.shape.face_names();
        egui::Grid::new("tags").num_columns(2).show(ui, |ui| {
            for (i, n) in names.iter().enumerate() {
                ui.label(*n);
                let t = &mut self.design.tags[i];
                let before = *t;
                egui::ComboBox::from_id_salt(("tag", i)).selected_text(t.name()).show_ui(ui, |ui| {
                    ui.selectable_value(t, Tag::Inlet, "inlet");
                    ui.selectable_value(t, Tag::Outlet, "outlet");
                    ui.selectable_value(t, Tag::Wall, "walls");
                });
                changed |= *t != before;
                ui.end_row();
            }
        });
        if self.is_coil() {
            ui.label(RichText::new("Fixed: inlet at the bottom end (uniform 5 mm/s), outlet at the top (p = 0), no-slip wall. The flow develops from the inlet; the friction factor is measured on the last turn.").small());
        } else if !self.is_exchanger() {
            ui.label(RichText::new("inlet: uniform 5 mm/s · outlet: p = 0 · walls: no-slip").small());
        }
        ui.separator();
        ui.label(RichText::new("3 · Mesh").strong());
        // gh:#1202: the cylinder can be meshed structured (the O-grid) or
        // unstructured (cfMesh tet → polyhedral dual → wall prism layers).
        // Unstructured changes are applied with the Generate button: a mesh
        // takes seconds, and every slider step would queue one.
        let mut regenerate = false;
        if let Shape::Cylinder { radius, length, .. } = self.design.shape {
            ui.horizontal_wrapped(|ui| {
                let st = self.design.mesh == PipeMesh::Structured;
                if ui.selectable_label(st, RichText::new("Structured (O-grid)").size(16.0)).clicked() && !st {
                    self.design.mesh = PipeMesh::Structured;
                    changed = true;
                }
                if ui.selectable_label(!st, RichText::new("Unstructured (polyhedral + layers)").size(16.0)).clicked() && st {
                    self.design.mesh = PipeMesh::Unstructured(self.unstructured);
                    changed = true;
                }
            });
            if let PipeMesh::Unstructured(_) = self.design.mesh {
                let s = &mut self.unstructured;
                let mut touched = false;
                let mut v = s.cell_size * 1e3;
                touched |= ui.add(egui::Slider::new(&mut v, (radius / CELL_PER_R_MAX * 1e3)..=(radius / CELL_PER_R_MIN * 1e3)).text("cell size").suffix(" mm")).changed();
                s.cell_size = v * 1e-3;
                touched |= ui.add(egui::Slider::new(&mut s.n_layers, 0..=MAX_LAYERS).text("wall layers")).changed();
                let mut f = s.first_layer * 1e3;
                touched |= ui.add(egui::Slider::new(&mut f, (1e-3 * radius * 1e3)..=(0.1 * radius * 1e3)).text("first layer").suffix(" mm").logarithmic(true)).changed();
                s.first_layer = f * 1e-3;
                touched |= ui.add(egui::Slider::new(&mut s.expansion, 1.0..=1.5).text("expansion")).changed();
                // gh:#1240 / #1215 mesher choices, as on the native knobs.
                {
                    use outram_blender::foam_mesh::LayerQualityStrategy as L;
                    use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
                    ui.label(RichText::new("wall faces of the dual").small());
                    ui.horizontal_wrapped(|ui| {
                        for (k, label) in [(D::MedianPerCorner, "per corner"), (D::MedianPerPoint, "per point"), (D::CellCentre, "cell-centre")] {
                            touched |= ui.selectable_value(&mut s.dual, k, label).changed();
                        }
                    });
                    if s.dual != D::MedianPerCorner {
                        let mut corners = s.feature_points == F::Corners;
                        if ui.checkbox(&mut corners, "keep corners only (polyDualMesh class rule)").changed() {
                            s.feature_points = if corners { F::Corners } else { F::OnFeatureEdge };
                            touched = true;
                        }
                    }
                    ui.label(RichText::new("bad layer cells").small());
                    ui.horizontal_wrapped(|ui| {
                        for (k, label) in [(L::BackOffOnly, "back off"), (L::CfMeshOptimise, "cfMesh optimise"), (L::SnappyCollapse, "snappy collapse")] {
                            touched |= ui.selectable_value(&mut s.layer_quality, k, label).changed();
                        }
                    });
                    // gh:#1298: upstream's sequence around the dual (the
                    // default) or the route before it (the ablation).
                    let mut upstream = s.route == crate::engine::CfMeshRoute::UpstreamNoLaplace;
                    if ui.checkbox(&mut upstream, "cfMesh surface + final optimisation (upstream sequence)").changed() {
                        s.route = if upstream { crate::engine::CfMeshRoute::UpstreamNoLaplace } else { crate::engine::CfMeshRoute::PreGh1298 };
                        touched = true;
                    }
                }
                let c = s.clamped(radius, length);
                if touched {
                    self.design.mesh = PipeMesh::Unstructured(*s);
                    self.dirty = true;
                }
                if let Some(w) = layer_warning(&c) {
                    ui.label(RichText::new(w).small().color(ui.visuals().warn_fg_color));
                }
                let est = c.estimated_cells(radius, length);
                ui.label(RichText::new(format!("≈ {:.0}k cells estimated{}", est / 1e3, if c.cell_size > s.cell_size * 1.001 { " (cell size raised to stay under the cap)" } else { "" })).small());
                regenerate = ui.add_enabled(self.dirty && !self.meshing, egui::Button::new(RichText::new("Generate mesh").size(16.0))).clicked();
            } else {
                ui.horizontal_wrapped(|ui| {
                    for (n, label) in [(2_000, "~2k"), (5_000, "~5k"), (10_000, "~10k"), (DEFAULT_CELLS, "20k"), (MAX_CELLS, "≤50k batch")] {
                        if ui.selectable_label(self.design.target_cells == n, label).clicked() {
                            self.design.target_cells = n;
                            changed = true;
                        }
                    }
                });
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                for (n, label) in [(2_000, "~2k"), (5_000, "~5k"), (10_000, "~10k"), (DEFAULT_CELLS, "20k"), (MAX_CELLS, "≤50k batch")] {
                    if ui.selectable_label(self.design.target_cells == n, label).clicked() {
                        self.design.target_cells = n;
                        changed = true;
                    }
                }
            });
        }
        let unstructured = matches!((self.design.shape, self.design.mesh), (Shape::Cylinder { .. }, PipeMesh::Unstructured(_)));
        ui.label(
            RichText::new(match self.design.shape {
                Shape::Cylinder { .. } if unstructured => {
                    "cfMesh port: a Cartesian carve snapped to the cylinder, cut into tetrahedra, turned into polyhedra (one cell per tet vertex), \
                     its boundary mapped and untangled onto the surface and the mesh optimised as cfMesh's tetMesh does, \
                     then prism layers grown on the wall only (the inlet and outlet stay layer-free). Meshing runs in the worker; it takes seconds."
                }
                Shape::Cylinder { .. } => "O-grid: a square core and a ring of radial cells; the wall is a polygon on the circle.",
                Shape::Box { .. } => "One hex block, near-cubic cells.",
                Shape::DoublePipe { .. } => "Three region meshes, face to face: an O-grid in the tube, a ring for the wall, a ring for the annulus (the same cells around).",
                Shape::Coil { .. } => "The pipe O-grid swept along the helix on an untwisted (rotation-minimising) frame; no layer is longer than the tube diameter.",
            })
            .small(),
        );
        if changed {
            self.dirty = true;
            self.cam = Self::camera_for(&self.design);
        }
        // An unstructured mesh is generated on request (the button above, or
        // Run); every other change re-meshes at once, as before.
        if (changed && !unstructured) || regenerate {
            self.send_mesh();
        } else if changed && unstructured {
            self.status = "unstructured settings changed: press Generate mesh (or Run)".into();
        }
        if let Some(m) = &self.mesh {
            ui.label(format!("{} cells · max non-orthogonality {:.1}° · max skewness {:.2}", m.n_cells, m.max_non_ortho_deg, m.max_skewness));
            ui.label(RichText::new(format!("meshed in {:.2} s{}", m.mesh_s, if self.dirty { " · settings changed since" } else { "" })).small());
            if !m.notes.is_empty() {
                ui.label(RichText::new(format!("mesher: {}", m.notes)).small().color(ui.visuals().warn_fg_color));
            }
            if m.n_cells as f64 > 1.05 * DEFAULT_CELLS as f64 {
                ui.label(RichText::new("Over 20k cells: a batch run of about a minute; the progress bar shows how far.").color(ui.visuals().warn_fg_color));
            }
        }
        ui.separator();
        ui.label(RichText::new("4 · Run").strong());
        if self.is_coil() {
            ui.add(egui::Slider::new(&mut self.reynolds, 50.0..=800.0).text("Re = U d / ν").logarithmic(true));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Steady, "Steady (Co 1)");
                ui.selectable_value(&mut self.mode, Mode::Transient, "Watch it develop (Co 1)");
            });
            ui.label(RichText::new("Convection: Gauss linear (second order, fully implicit as in OpenFOAM), as in the V&V record: upwind's numerical viscosity would damp the Dean vortices. The march runs at Co 1 (the implicit central scheme diverges at Co 2 and 4), so it takes about four times the steps of a Co 4 march.").small());
        } else if self.is_exchanger() {
            ui.add(egui::Slider::new(&mut self.reynolds, 10.0..=100.0).text("Re in the tube").logarithmic(true));
            ui.label(RichText::new("Steady: the flows are marched to steady, frozen, then the three temperatures are iterated to steady (chtMultiRegionFoam's loop).").small());
        } else {
            ui.add(egui::Slider::new(&mut self.reynolds, 20.0..=200.0).text("Re = U D_h / ν").logarithmic(true));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Steady, "Steady (Co ≤ 4)");
                ui.selectable_value(&mut self.mode, Mode::Transient, "Watch it develop (Co ≈ 1)");
            });
        }
        ui.horizontal(|ui| {
            let can = self.link.is_some() && !self.meshing;
            if ui.add_enabled(can && !self.running, egui::Button::new(RichText::new("▶ Run").size(18.0))).clicked() {
                self.start();
            }
            if ui.add_enabled(self.running, egui::Button::new(RichText::new("■ Stop").size(18.0))).clicked() {
                self.running = false;
            }
            if ui.add_enabled(can && self.last.is_some() && !self.running, egui::Button::new("Continue")).clicked() {
                self.running = true;
            }
        });
        ui.separator();
        ui.label(RichText::new("5 · Show").strong());
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.shown, Shown::Speed, "|U|");
            ui.selectable_value(&mut self.shown, Shown::Pressure, "p (kinematic)");
            if self.is_exchanger() {
                ui.selectable_value(&mut self.shown, Shown::Temperature, "T");
            }
            ui.checkbox(&mut self.show_mesh, "mesh");
        });
        let probe_label = if self.is_coil() { "cross-section, inlet to outlet (along the tube)" } else { "probe and slice, inlet to outlet" };
        if ui.add(egui::Slider::new(&mut self.probe, 0.0..=1.0).text(probe_label)).drag_stopped() {
            self.send(Request::Probe { id: self.next_id, probe: self.probe });
        }
        ui.checkbox(&mut self.pan_mode, "one-finger drag pans (else orbits)");
        ui.separator();
        ui.label(RichText::new(&self.status).small());
        if let Some(e) = &self.link_error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.separator();
        ui.label(
            RichText::new(
                "Solver: OUTRAM PARK's port of OpenFOAM's pimpleFoam (laminar, PISO, upwind, Gauss linear corrected), \
                 in a background worker. Steady runs are pseudo-transient. The pipe case is checked against \
                 Hagen–Poiseuille and the Durst et al. (2005) development length in the V&V record laminar_pipe_gh1155.md. \
                 The unstructured (cfMesh polyhedral) mesh runs Gauss linear convection with 2 non-orthogonal correctors: upwind there smeared \
                 momentum across oblique faces and flattened the profile. It still reads about 6 % low at the default (centreline and pressure gradient), 3 % at the finest cell; see the record's gh:#1202 section. \
                 The exchanger adds the energy equation and a port of chtMultiRegionFoam's coupled-temperature interface; \
                 checked against the composite slab, the Graetz Nusselt numbers and ε-NTU in cht_double_pipe_gh1156.md. \
                 The coil section is checked against Mishra & Gupta (1979)'s laminar coil friction factor, as a function of the \
                 Dean number, in helical_coil_dean_gh1157.md (the correlation is called from tampines, as in the helical steam-generator demo). \
                 Research, education and V&V only: not for reactor operation, licensing or safety decisions.",
            )
            .small(),
        );
    }

    /// The 3-D viewport. GUI drawing (exempt).
    fn viewport(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let resp = ui.interact(rect, ui.id().with("viewport"), Sense::click_and_drag());
        self.cam.handle_input(ui, &resp, self.pan_mode);
        let painter = ui.painter().with_clip_rect(rect);
        let ink = bugis::Ink::of_ui(ui);
        painter.rect_filled(rect, 0.0, ink.bg);
        let Some(me) = &self.mesh else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, &self.status, egui::FontId::proportional(16.0), ink.muted);
            return;
        };
        let cam = self.cam;
        // The fields on the two slices.
        let mut scale = None;
        if let Some(p) = &self.last {
            let (ax, cr) = match self.shown {
                Shown::Speed => (&p.axial_u, &p.cross_u),
                Shown::Pressure => (&p.axial_p, &p.cross_p),
                Shown::Temperature => (&p.axial_t, &p.cross_t),
            };
            let vals: Vec<f64> = ax.iter().chain(cr.iter()).map(|v| f64::from(*v)).collect();
            let s = match self.shown {
                Shown::Speed => Scale::from_values(&vals, ColourMap::Batlow),
                Shown::Pressure => Scale::from_values(&vals, ColourMap::Lajolla),
                // T on a fixed range, cold inlet to hot inlet.
                Shown::Temperature => Scale::from_values(&[T_COLD_IN, T_HOT_IN], ColourMap::Batlow),
            };
            for (sec, v) in [(&me.axial, ax), (&me.cross, cr)] {
                if sec.polygons.len() == v.len() {
                    let polys = project_section(&cam, rect, sec);
                    let vv: Vec<f64> = v.iter().map(|x| f64::from(*x)).collect();
                    field::paint(&painter, &polys, &vv, &s);
                }
            }
            scale = Some(s);
        } else {
            // Before a run: the slices as outlines of the cells cut.
            for sec in [&me.axial, &me.cross] {
                outline(&painter, &cam, rect, sec, ink.border);
            }
        }
        if self.show_mesh {
            let n = me.edges.len() / 7;
            let colours = [ink.accent(5), ink.accent(6), ink.accent(0).gamma_multiply(0.35)];
            for e in 0..n {
                let k = &me.edges[7 * e..7 * e + 7];
                let a = cam.project(rect, [k[0], k[1], k[2]].map(f64::from));
                let b = cam.project(rect, [k[3], k[4], k[5]].map(f64::from));
                if let (Some(a), Some(b)) = (a, b) {
                    painter.line_segment([a, b], Stroke::new(1.0, colours[(k[6] as usize).min(2)]));
                }
            }
        }
        // The probe.
        let pc = probe_point(&self.design, &me.cross);
        if let Some(p) = cam.project(rect, pc) {
            painter.circle_stroke(p, 6.0, Stroke::new(2.0, ink.text));
        }
        // Axes triad, bottom left.
        let o = Pos2::new(rect.left() + 40.0, rect.bottom() - 40.0);
        let (r, u, _) = cam.basis();
        for (i, (name, col)) in [("x", ink.accent(6)), ("y", ink.accent(5)), ("z", ink.accent(0))].iter().enumerate() {
            let tip = o + Vec2::new(r[i] as f32, -(u[i] as f32)) * 26.0;
            painter.line_segment([o, tip], Stroke::new(2.0, *col));
            painter.text(tip, egui::Align2::CENTER_CENTER, *name, egui::FontId::proportional(12.0), *col);
        }
        if let Some(s) = scale {
            let bar = Rect::from_min_size(Pos2::new(rect.right() - 230.0, rect.top() + 100.0), Vec2::new(210.0, 30.0));
            let label = match self.shown {
                Shown::Speed => "|U| [m/s]",
                Shown::Pressure => "p [m²/s²]",
                Shown::Temperature => "T [K]",
            };
            field::colour_bar(&painter, bar, &s, label, 12.0);
        }
        let legend = if self.is_exchanger() { "green inlets · red outlets · blue walls and interfaces" } else { "green inlet · red outlet · blue walls" };
        painter.text(Pos2::new(rect.left() + 80.0, rect.bottom() - 12.0), egui::Align2::LEFT_BOTTOM, legend, egui::FontId::proportional(12.0), ink.muted);
        // Batch progress.
        if self.running && me.n_cells as f64 > 1.05 * DEFAULT_CELLS as f64 && (self.mode == Mode::Steady || self.is_exchanger()) {
            let b = Rect::from_min_size(Pos2::new(rect.left() + 16.0, rect.top() + 140.0), Vec2::new((rect.width() - 32.0).min(320.0), 18.0));
            painter.rect_filled(b, 4.0, ink.grid);
            let f = Rect::from_min_size(b.min, Vec2::new(b.width() * self.batch_frac as f32, b.height()));
            painter.rect_filled(f, 4.0, ink.accent(0));
            painter.text(b.center(), egui::Align2::CENTER_CENTER, format!("batch run {:.0} %", 100.0 * self.batch_frac), egui::FontId::proportional(12.0), ink.text);
        }
        // Meshing in the worker (an unstructured mesh takes seconds): say so on
        // the main view, where it shows with the panel folded.
        if self.meshing {
            let b = Rect::from_min_size(Pos2::new(rect.left() + 16.0, rect.top() + 110.0), Vec2::new((rect.width() - 32.0).min(320.0), 24.0));
            painter.rect_filled(b, 4.0, ink.card);
            painter.text(b.center(), egui::Align2::CENTER_CENTER, "meshing in the background…", egui::FontId::proportional(13.0), ink.text);
        }
        if let Some(z) = bugis::web_demo::view::zoom_buttons(ui, rect) {
            match z {
                Zoom::In => self.cam.zoom(1.5),
                Zoom::Out => self.cam.zoom(1.0 / 1.5),
                Zoom::Reset => self.cam.reset(),
            }
        }
        // A pan / orbit toggle under the zoom buttons, for one finger.
        let t = Rect::from_min_size(Pos2::new(rect.right() - 160.0, rect.top() + 52.0), Vec2::new(152.0, 36.0));
        let label = if self.pan_mode { "Drag: pan" } else { "Drag: orbit" };
        if ui.put(t, egui::Button::new(RichText::new(label).size(16.0))).clicked() {
            self.pan_mode = !self.pan_mode;
        }
    }

    /// The dock: residuals, probe, profile. GUI drawing (exempt).
    fn dock_body(&self, ui: &egui::Ui, body: Rect) {
        let painter = ui.painter().with_clip_rect(body);
        let wide = body.width() > 700.0;
        // The pipe gets a fourth panel: the mesh picture (gh:#1202).
        let pipe = !self.is_coil() && !self.is_exchanger();
        let k = if pipe { 4 } else { 3 };
        let n = k as f32;
        let rects: Vec<Rect> = (0..k)
            .map(|i| {
                if wide {
                    let w = body.width() / n;
                    Rect::from_min_size(Pos2::new(body.left() + w * i as f32, body.top()), Vec2::new(w - 6.0, body.height() - 40.0))
                } else {
                    let h = (body.height() - 40.0) / n;
                    Rect::from_min_size(Pos2::new(body.left(), body.top() + h * i as f32), Vec2::new(body.width(), h - 4.0))
                }
            })
            .collect();
        if self.is_coil() {
            let r2: Vec<Rect> = if wide {
                let w = body.width() / 3.0;
                vec![
                    Rect::from_min_size(body.min, Vec2::new(w - 6.0, body.height() - 40.0)),
                    Rect::from_min_size(Pos2::new(body.left() + w, body.top()), Vec2::new(2.0 * w, body.height() - 40.0)),
                ]
            } else {
                let h = body.height() - 40.0;
                vec![
                    Rect::from_min_size(body.min, Vec2::new(body.width(), 0.32 * h - 4.0)),
                    Rect::from_min_size(Pos2::new(body.left(), body.top() + 0.32 * h), Vec2::new(body.width(), 0.68 * h)),
                ]
            };
            residual::paint(&painter, r2[0], &self.residuals, true, "time [s] (steady when the blue line < 1e-3)", 11.0);
            self.dean_view(&painter, r2[1]);
        } else if self.is_exchanger() {
            residual::paint(&painter, rects[0], &self.residuals, true, "step (flow to < 1e-3, then T to < 1e-7)", 11.0);
            residual::paint(&painter, rects[1], &self.probe_hist, false, "step", 11.0);
            self.exchanger_numbers(&painter, rects[2]);
        } else {
            residual::paint(&painter, rects[0], &self.residuals, true, "time [s] (steady when the blue line < 1e-3)", 11.0);
            residual::paint(&painter, rects[1], &self.probe_hist, false, "time [s]", 11.0);
            self.profile(&painter, rects[2]);
            self.mesh_picture(&painter, rects[3]);
        }
    }

    /// The mesh the solver gets, flat (gh:#1202): the cross-section at the
    /// probe on the left, the axial slice's first part (inlet end, two
    /// diameters long) on the right, cell outlines from the engine's
    /// sections. GUI drawing (exempt).
    fn mesh_picture(&self, painter: &egui::Painter, rect: Rect) {
        let ink = bugis::Ink::of_ctx(painter.ctx());
        painter.rect_filled(rect, 4.0, ink.backing);
        let f = egui::FontId::proportional(11.0);
        let (Some(me), Shape::Cylinder { radius, centre, length }) = (&self.mesh, self.design.shape) else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "mesh: mesh a cylinder", f, ink.muted);
            return;
        };
        let inner = rect.shrink(6.0);
        let half = Rect::from_min_size(inner.min, Vec2::new(0.5 * inner.width() - 3.0, inner.height() - 14.0));
        let right = Rect::from_min_size(Pos2::new(inner.center().x + 3.0, inner.top()), half.size());
        let stroke = Stroke::new(0.6, ink.muted);
        // Cross-section: in-plane (x, y) about the pipe centre.
        let k = 0.5 * half.width().min(half.height()) / radius as f32;
        for p in &me.cross.polygons {
            let pts: Vec<Pos2> = p.iter().map(|q| Pos2::new(half.center().x + (q[0] - centre[0] as f32) * k, half.center().y - (q[1] - centre[1] as f32) * k)).collect();
            if pts.len() >= 3 {
                painter.add(egui::Shape::closed_line(pts, stroke));
            }
        }
        // Axial slice (x, z) from the inlet end, 4 R long, x across.
        let z0 = centre[2] - 0.5 * length;
        let span = (4.0 * radius).min(length);
        let ka = (right.height() / span as f32).min(right.width() / (2.0 * radius) as f32);
        let clip = painter.with_clip_rect(right);
        for p in &me.axial.polygons {
            if p.iter().all(|q| f64::from(q[1]) > z0 + span) {
                continue;
            }
            let pts: Vec<Pos2> = p.iter().map(|q| Pos2::new(right.center().x + (q[0] - centre[0] as f32) * ka, right.top() + (q[1] - z0 as f32) * ka)).collect();
            if pts.len() >= 3 {
                clip.add(egui::Shape::closed_line(pts, stroke));
            }
        }
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!("cross-section · axial slice from the inlet (down), {:.0} mm · {} cells", span * 1e3, me.n_cells),
            f,
            ink.muted,
        );
    }

    /// The exchanger's terminal temperatures, heats and effectiveness against
    /// ε-NTU at the solved local UA. GUI drawing (exempt).
    fn exchanger_numbers(&self, painter: &egui::Painter, rect: Rect) {
        let ink = bugis::Ink::of_ctx(painter.ctx());
        painter.rect_filled(rect, 4.0, ink.backing);
        let f = egui::FontId::proportional(12.0);
        let lines: Vec<String> = match &self.last {
            Some(p) if p.phase == 1 => {
                let x = &p.exch;
                vec![
                    format!("hot: {T_HOT_IN:.1} to {:.2} K   cold: {T_COLD_IN:.1} to {:.2} K", x[Exch::T_HOT_OUT], x[Exch::T_COLD_OUT]),
                    format!("Q hot {:.4} W · Q cold {:.4} W", x[Exch::Q_HOT], x[Exch::Q_COLD]),
                    format!("UA (local) {:.4} W/K · NTU {:.3}", x[Exch::UA], x[Exch::NTU]),
                    format!("ε solved {:.4} · ε-NTU {:.4} ({:+.2} %)", x[Exch::EPS], x[Exch::EPS_NTU], 100.0 * (x[Exch::EPS] / x[Exch::EPS_NTU] - 1.0)),
                    "ε-NTU: counterflow, C_r = 1 (dwsim-libs ntu_effectiveness)".into(),
                ]
            }
            Some(_) => vec!["flow being marched to steady…".into(), "then: temperatures, heats, ε against ε-NTU".into()],
            None => vec!["exchanger numbers: run it".into()],
        };
        // Clip to the rect (the dock already keeps the References strip
        // clear below it); the last lines go first on a short phone dock.
        let rows = (((rect.height() - 8.0) / 18.0).floor().max(1.0)) as usize;
        for (i, l) in lines.iter().take(rows).enumerate() {
            painter.text(rect.left_top() + Vec2::new(8.0, 8.0 + 18.0 * i as f32), egui::Align2::LEFT_TOP, l, f.clone(), ink.text);
        }
    }

    /// The coil's cross-section as Dean drew it: the coil axis to the left,
    /// the outer wall to the right, B up; colour u_t/U, arrows the secondary
    /// velocity (u_n, u_b), and the friction factor against Mishra & Gupta.
    /// GUI drawing (exempt); the numbers come from the tested engine.
    fn dean_view(&self, painter: &egui::Painter, rect: Rect) {
        let ink = bugis::Ink::of_ctx(painter.ctx());
        painter.rect_filled(rect, 4.0, ink.backing);
        let f = egui::FontId::proportional(11.0);
        let (Some(me), Some(p), Shape::Coil { tube_radius, .. }) = (&self.mesh, &self.last, self.design.shape) else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Dean cross-section: run the coil", f, ink.muted);
            return;
        };
        let sec = &me.cross;
        if sec.polygons.len() != p.cross_ua.len() || p.cross_sec.len() != 2 * p.cross_ua.len() {
            return;
        }
        // Text beside the picture when there is room for ~22 characters.
        let text_w = if rect.width() > 1.4 * rect.height() || rect.width() > 330.0 { (0.45 * rect.width()).min(190.0) } else { 0.0 };
        let pic = Rect::from_min_max(rect.min + Vec2::new(8.0, 18.0), Pos2::new(rect.max.x - 8.0 - text_w, rect.max.y - 30.0));
        let k = 0.5 * pic.width().min(pic.height()) / tube_radius as f32;
        let c = pic.center();
        // (n, b) to screen: outer (−n) to the right, b up.
        let to = |n: f32, b: f32| Pos2::new(c.x - n * k, c.y - b * k);
        let ut: Vec<f64> = p.cross_ua.iter().map(|v| f64::from(*v) / U_IN).collect();
        let scale = Scale::from_values(&ut, ColourMap::Batlow);
        let polys: Vec<Vec<Pos2>> = sec.polygons.iter().map(|q| q.iter().map(|v| to(v[0], v[1])).collect()).collect();
        field::paint(painter, &polys, &ut, &scale);
        let umax = (0..ut.len()).map(|i| (p.cross_sec[2 * i].powi(2) + p.cross_sec[2 * i + 1].powi(2)).sqrt()).fold(0.0_f32, f32::max).max(1e-12);
        let cell = (pic.width().min(pic.height()) / (sec.polygons.len() as f32).sqrt()).max(4.0);
        for (i, q) in sec.polygons.iter().enumerate() {
            let nq = q.len().max(1) as f32;
            let (cn, cb) = q.iter().fold((0.0, 0.0), |a, v| (a.0 + v[0] / nq, a.1 + v[1] / nq));
            let (un, ub) = (p.cross_sec[2 * i], p.cross_sec[2 * i + 1]);
            let a = to(cn, cb);
            let d = Vec2::new(-un, -ub) / umax * 1.2 * cell;
            if d.length() < 1.0 {
                continue;
            }
            let b = a + d;
            // On the Batlow field: fixed white, as the caption says.
            let st = Stroke::new(1.2, Color32::WHITE);
            painter.line_segment([a, b], st);
            let h = d.normalized() * 3.5;
            painter.line_segment([b, b - h + Vec2::new(-h.y, h.x) * 0.6], st);
            painter.line_segment([b, b - h - Vec2::new(-h.y, h.x) * 0.6], st);
        }
        painter.text(Pos2::new(pic.left(), rect.top() + 4.0), egui::Align2::LEFT_TOP, "< coil axis (inner side)", f.clone(), ink.muted);
        painter.text(Pos2::new(pic.right(), rect.top() + 4.0), egui::Align2::RIGHT_TOP, "outer wall >", f.clone(), ink.muted);
        let bar = Rect::from_min_size(Pos2::new(rect.left() + 8.0, rect.bottom() - 26.0), Vec2::new((pic.width()).min(200.0), 14.0));
        field::colour_bar(painter, bar, &scale, "u_axial/U", 10.0);
        let cn = &p.coil;
        let lines = [
            format!("De {:.1}", cn[CoilNo::DEAN]),
            format!("f/f_s {:.3} last turn", cn[CoilNo::F_RE_64]),
            format!("f/f_s {:.3} turn before", cn[CoilNo::F_RE_64_BEFORE]),
            format!("Mishra-Gupta {:.3}", cn[CoilNo::MISHRA_GUPTA]),
            format!("axial max n/R {:+.2}", cn[CoilNo::PEAK_N]),
            format!("secondary {:.2} U", cn[CoilNo::MAX_SECONDARY]),
            format!("core u_n {:+.3} U", cn[CoilNo::CORE_UN]),
            "white arrows: secondary flow".to_string(),
            "n/R < 0: towards outer wall".to_string(),
        ];
        let (x0, y0) = if text_w > 0.0 { (pic.right() + 12.0, rect.top() + 22.0) } else { (rect.left() + 8.0, rect.top() + 18.0) };
        if text_w > 0.0 || rect.height() > 260.0 {
            for (i, l) in lines.iter().enumerate() {
                let y = if text_w > 0.0 { y0 + 16.0 * i as f32 } else { rect.top() + 18.0 + 14.0 * i as f32 };
                if text_w == 0.0 && i > 3 || y > rect.bottom() - 12.0 {
                    break;
                }
                painter.text(Pos2::new(x0, y), egui::Align2::LEFT_TOP, l, f.clone(), ink.text);
            }
        }
    }

    /// u_axial / U against r / R at the slice, with Hagen–Poiseuille's
    /// 2 (1 − r²/R²). GUI drawing (exempt).
    fn profile(&self, painter: &egui::Painter, rect: Rect) {
        let ink = bugis::Ink::of_ctx(painter.ctx());
        painter.rect_filled(rect, 4.0, ink.backing);
        let (Some(me), Some(p), Shape::Cylinder { radius, centre, .. }) = (&self.mesh, &self.last, self.design.shape) else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "profile: run a pipe", egui::FontId::proportional(11.0), ink.muted);
            return;
        };
        let plot = Rect::from_min_max(rect.min + Vec2::new(30.0, 6.0), rect.max - Vec2::new(6.0, 26.0));
        painter.rect_stroke(plot, 0.0, Stroke::new(1.0, ink.border), egui::StrokeKind::Inside);
        let to = |r: f64, u: f64| Pos2::new(plot.left() + (r as f32) * plot.width(), plot.bottom() - (u / 2.2) as f32 * plot.height());
        let line: Vec<Pos2> = (0..=40).map(|k| k as f64 / 40.0).map(|r| to(r, 2.0 * (1.0 - r * r))).collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.5, ink.accent(1))));
        if me.cross.polygons.len() == p.cross_ua.len() {
            for (poly, u) in me.cross.polygons.iter().zip(&p.cross_ua) {
                let n = poly.len().max(1) as f64;
                let (cx, cy) = poly.iter().fold((0.0, 0.0), |a, q| (a.0 + f64::from(q[0]) / n, a.1 + f64::from(q[1]) / n));
                let r = ((cx - centre[0]).powi(2) + (cy - centre[1]).powi(2)).sqrt() / radius;
                painter.circle_filled(to(r.min(1.0), f64::from(*u) / U_IN), 1.6, ink.accent(0));
            }
        }
        // The binned profile (gh:#1202): cell centres near the slice, binned
        // in r (`laminar_pipe::radial_profile_binned`); works on either mesh.
        let bins: Vec<Pos2> = p.profile.chunks(2).map(|q| to(f64::from(q[0]).min(1.0), f64::from(q[1]))).collect();
        if bins.len() >= 2 {
            painter.add(egui::Shape::line(bins.clone(), Stroke::new(1.5, ink.text)));
        }
        for b in bins {
            painter.circle_filled(b, 3.0, ink.text);
        }
        let f = egui::FontId::proportional(11.0);
        painter.text(Pos2::new(plot.left() - 3.0, plot.top()), egui::Align2::RIGHT_TOP, "2.2", f.clone(), ink.muted);
        painter.text(Pos2::new(plot.left() - 3.0, plot.bottom()), egui::Align2::RIGHT_BOTTOM, "0", f.clone(), ink.muted);
        painter.text(
            Pos2::new(plot.center().x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            "u_axial/U vs r/R · dots: cells cut by the slice · line: cell centres binned in r · orange: Hagen–Poiseuille",
            f,
            ink.muted,
        );
    }
}

/// The outline of each polygon of a section. GUI drawing (exempt).
fn outline(painter: &egui::Painter, cam: &Orbit, rect: Rect, sec: &Section, c: Color32) {
    for p in project_section(cam, rect, sec) {
        if p.len() >= 3 {
            painter.add(egui::Shape::closed_line(p, Stroke::new(0.6, c)));
        }
    }
}

/// The probe's 3-D point: the cross-section's plane on the shape's axis
/// (the frame's origin, on the centreline, for the coil).
fn probe_point(d: &Design, cross: &Section) -> [f64; 3] {
    if let Some(f) = &cross.frame {
        return [f[0], f[1], f[2]];
    }
    let [lo, hi] = d.shape.bounds();
    let c = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
    let (a, b) = match cross.axis {
        0 => (c[1], c[2]),
        1 => (c[0], c[2]),
        _ => (c[0], c[1]),
    };
    plane_to_3d(cross.axis, cross.offset, a, b)
}

impl eframe::App for FoamApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        let title = match &self.mesh {
            Some(m) => match &self.last {
                Some(p) => format!(
                    "Foam Studio · {} cells · step {} · {:.0} ms/step{}",
                    m.n_cells,
                    p.steps,
                    p.ms_per_step,
                    if self.running { " · solving" } else if p.steady { " · steady" } else { "" }
                ),
                None => format!("Foam Studio · {} cells", m.n_cells),
            },
            None => "Foam Studio".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Foam Studio", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let full = ui.max_rect();
            let split = self.dock.split(full);
            self.viewport(ui, split.main);
            if let Some(body) = split.body {
                ui.painter().rect_filled(body, 0.0, ui.visuals().extreme_bg_color);
                self.dock_body(ui, body);
            }
            let _ = self.dock.toggle(ui, &split);
            self.panel.reopen_button(ui, split.main);
        });
        if self.running || self.meshing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// The warning the mesh panel shows under the layer controls: what the V&V
/// record (`laminar_pipe_gh1155.md`, gh:#1202, #1215, #1240) measured for
/// this many layers with these mesher choices, at the demo pipe. `None` when
/// there is nothing to warn about.
fn layer_warning(c: &TetDualSpec) -> Option<&'static str> {
    use outram_blender::foam_mesh::LayerQualityStrategy as L;
    use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
    let polydual_corners = c.dual != D::MedianPerCorner && c.feature_points == F::Corners;
    let upstream = c.route == crate::engine::CfMeshRoute::UpstreamNoLaplace;
    match (c.n_layers, c.layer_quality) {
        (0, _) => None,
        (_, L::CfMeshOptimise) if upstream => Some(
            "cfMesh's layer optimiser with upstream's sequence (the default): about 6 s natively at 5 mm with 3 layers, about 22 s in a browser (gh:#1298). On the cell-centre dual, corners only, it ran at u_c/2U 0.939, dp/dz 1.006 x Hagen-Poiseuille, with 0 bad faces; the layers come out about 0.9 mm for 0.98 mm asked.",
        ),
        (_, L::CfMeshOptimise) => Some(
            "cfMesh's layer optimiser takes minutes (140-370 s natively at 5 mm, 3 layers; slower in a browser) and makes the layers thicker than asked (its thickness control is not ported). With the cell-centre dual, corners only, 3 layers ran at 5 mm: u_c/2U 0.931, dp/dz 1.005 x Hagen-Poiseuille (gh:#1240).",
        ),
        (_, L::SnappyCollapse) => Some("snappy's collapse removes every layer at 2 or more on these meshes (and most at 1): you get a bare mesh (gh:#1215, #1240)."),
        (_, L::BackOffOnly) if c.dual != D::MedianPerCorner && c.feature_points == F::OnFeatureEdge => {
            Some("with this dual and every edge point kept, the back-off grows no layer on the pipe (collinear rim points cap the thickness at 0): tick \"keep corners only\" (gh:#1240).")
        }
        (n, L::BackOffOnly) if n >= 3 => Some(if polydual_corners {
            "3 layers with the back-off diverge in the solve on these duals too (gh:#1240); use cfMesh optimise for 3, or 1-2 layers here."
        } else {
            "3 layers: the solve diverges on the meshes tried (one dual cell over the layer stack, gh:#1202). 0-2 layers run."
        }),
        (2, L::BackOffOnly) => Some("2 layers run; on this mesh they do not bring the pipe closer to Hagen–Poiseuille than a finer bare mesh (gh:#1202)."),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::layer_warning;
    use crate::engine::TetDualSpec;

    /// The mesh panel warns according to the recorded results (gh:#1202,
    /// #1215, #1240, #1283): nothing for a bare mesh, the minutes for cfMesh
    /// on the pre-gh:#1298 route and its seconds on upstream's sequence,
    /// the zero-thickness cap for a polyDualMesh dual with every edge point
    /// kept, and the 3-layer back-off divergence. The cases name every
    /// choice (gh:#1298 changed the defaults); `None` in the URL helpers
    /// below means the pre-gh:#1298 choice.
    #[test]
    fn layer_warning_matches_the_record() {
        let spec = |n: usize, dual: Option<&str>, fp: Option<&str>, lq: Option<&str>| {
            TetDualSpec { n_layers: n, ..TetDualSpec::default_for(0.01) }
                .with_query(Some(dual.unwrap_or("corner")), Some(fp.unwrap_or("edges")), Some(lq.unwrap_or("backoff")))
                .with_route_query(Some("legacy"))
        };
        let default_with = |n: usize| TetDualSpec { n_layers: n, ..TetDualSpec::default_for(0.01) };
        assert!(layer_warning(&default_with(0)).is_none());
        // gh:#1298: the default itself (h 5 mm, 3 layers) shows the
        // upstream-sequence note, with the gh:#1298 record's numbers.
        let note = layer_warning(&TetDualSpec::default_for(0.01).clamped(0.01, 0.2)).unwrap();
        assert!(note.contains("upstream's sequence (the default)") && note.contains("about 6 s") && note.contains("0.939") && note.contains("1.006"), "{note}");
        assert_eq!(layer_warning(&default_with(3)), Some(note));
        assert!(layer_warning(&spec(0, None, None, Some("cfmesh"))).is_none());
        assert!(layer_warning(&spec(1, None, None, None)).is_none());
        assert!(layer_warning(&spec(2, None, None, None)).unwrap().contains("2 layers run"));
        assert!(layer_warning(&spec(3, None, None, None)).unwrap().contains("diverges"));
        assert!(layer_warning(&spec(3, Some("centre"), Some("corners"), Some("cfmesh"))).unwrap().contains("minutes"));
        assert!(layer_warning(&spec(1, Some("point"), None, None)).unwrap().contains("keep corners only"));
        assert!(layer_warning(&spec(3, Some("centre"), Some("corners"), None)).unwrap().contains("cfMesh optimise for 3"));
        assert!(layer_warning(&spec(2, None, None, Some("snappy"))).unwrap().contains("removes every layer"));
    }
}
