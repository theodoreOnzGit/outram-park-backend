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
use crate::engine::{Design, Event, Exch, Meshed, Mode, Progress, Request, Section, Shape, Tag, DEFAULT_CELLS, MAX_CELLS, T_COLD_IN, T_HOT_IN, U_IN};
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

const BG: Color32 = Color32::from_rgb(14, 16, 20);
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
        if bugis::web_demo::platform::query_value(&q, "mode") == Some("transient") {
            app.mode = Mode::Transient;
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
                    self.status = if self.is_exchanger() {
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

    /// The side panel. GUI (exempt).
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.label(RichText::new("1 · Shape").strong());
        ui.horizontal_wrapped(|ui| {
            let cyl = matches!(self.design.shape, Shape::Cylinder { .. });
            let bx = matches!(self.design.shape, Shape::Box { .. });
            let ex = self.is_exchanger();
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
        if !self.is_exchanger() {
            ui.label(RichText::new("inlet: uniform 5 mm/s · outlet: p = 0 · walls: no-slip").small());
        }
        ui.separator();
        ui.label(RichText::new("3 · Mesh").strong());
        ui.horizontal_wrapped(|ui| {
            for (n, label) in [(2_000, "~2k"), (5_000, "~5k"), (10_000, "~10k"), (DEFAULT_CELLS, "20k"), (MAX_CELLS, "≤50k batch")] {
                if ui.selectable_label(self.design.target_cells == n, label).clicked() {
                    self.design.target_cells = n;
                    changed = true;
                }
            }
        });
        ui.label(
            RichText::new(match self.design.shape {
                Shape::Cylinder { .. } => "O-grid: a square core and a ring of radial cells; the wall is a polygon on the circle.",
                Shape::Box { .. } => "One hex block, near-cubic cells.",
                Shape::DoublePipe { .. } => "Three region meshes, face to face: an O-grid in the tube, a ring for the wall, a ring for the annulus (the same cells around).",
            })
            .small(),
        );
        if changed {
            self.dirty = true;
            self.cam = Self::camera_for(&self.design);
            self.send_mesh();
        }
        if let Some(m) = &self.mesh {
            ui.label(format!("{} cells · max non-orthogonality {:.1}°", m.n_cells, m.max_non_ortho_deg));
            if m.n_cells as f64 > 1.05 * DEFAULT_CELLS as f64 {
                ui.label(RichText::new("Over 20k cells: a batch run of about a minute; the progress bar shows how far.").color(Color32::from_rgb(250, 189, 47)));
            }
        }
        ui.separator();
        ui.label(RichText::new("4 · Run").strong());
        if self.is_exchanger() {
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
        if ui.add(egui::Slider::new(&mut self.probe, 0.0..=1.0).text("probe and slice, inlet to outlet")).drag_stopped() {
            self.send(Request::Probe { id: self.next_id, probe: self.probe });
        }
        ui.checkbox(&mut self.pan_mode, "one-finger drag pans (else orbits)");
        ui.separator();
        ui.label(RichText::new(&self.status).small());
        if let Some(e) = &self.link_error {
            ui.colored_label(Color32::LIGHT_RED, e);
        }
        ui.separator();
        ui.label(
            RichText::new(
                "Solver: OUTRAM PARK's port of OpenFOAM's pimpleFoam (laminar, PISO, upwind, Gauss linear corrected), \
                 in a background worker. Steady runs are pseudo-transient. The pipe case is checked against \
                 Hagen–Poiseuille and the Durst et al. (2005) development length in the V&V record laminar_pipe_gh1155.md. \
                 The exchanger adds the energy equation and a port of chtMultiRegionFoam's coupled-temperature interface; \
                 checked against the composite slab, the Graetz Nusselt numbers and ε-NTU in cht_double_pipe_gh1156.md. \
                 Research, education and V&V only: not for reactor operation, licensing or safety decisions.",
            )
            .small(),
        );
        ui.add_space(8.0);
        bugis::selector(ui, &mut self.theme);
    }

    /// The 3-D viewport. GUI drawing (exempt).
    fn viewport(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let resp = ui.interact(rect, ui.id().with("viewport"), Sense::click_and_drag());
        self.cam.handle_input(ui, &resp, self.pan_mode);
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, BG);
        let Some(me) = &self.mesh else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, &self.status, egui::FontId::proportional(16.0), Color32::GRAY);
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
                    let polys = project_section(&cam, rect, sec.axis, sec.offset, &sec.polygons);
                    let vv: Vec<f64> = v.iter().map(|x| f64::from(*x)).collect();
                    field::paint(&painter, &polys, &vv, &s);
                }
            }
            scale = Some(s);
        } else {
            // Before a run: the slices as outlines of the cells cut.
            for sec in [&me.axial, &me.cross] {
                outline(&painter, &cam, rect, sec, Color32::from_gray(70));
            }
        }
        if self.show_mesh {
            let n = me.edges.len() / 7;
            let colours = [Color32::from_rgb(142, 192, 124), Color32::from_rgb(251, 73, 52), Color32::from_rgba_unmultiplied(131, 165, 152, 90)];
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
            painter.circle_stroke(p, 6.0, Stroke::new(2.0, Color32::WHITE));
        }
        // Axes triad, bottom left.
        let o = Pos2::new(rect.left() + 40.0, rect.bottom() - 40.0);
        let (r, u, _) = cam.basis();
        for (i, (name, col)) in [("x", Color32::from_rgb(251, 73, 52)), ("y", Color32::from_rgb(184, 187, 38)), ("z", Color32::from_rgb(131, 165, 152))].iter().enumerate() {
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
        painter.text(Pos2::new(rect.left() + 80.0, rect.bottom() - 12.0), egui::Align2::LEFT_BOTTOM, legend, egui::FontId::proportional(12.0), Color32::GRAY);
        // Batch progress.
        if self.running && me.n_cells as f64 > 1.05 * DEFAULT_CELLS as f64 && (self.mode == Mode::Steady || self.is_exchanger()) {
            let b = Rect::from_min_size(Pos2::new(rect.left() + 16.0, rect.top() + 140.0), Vec2::new((rect.width() - 32.0).min(320.0), 18.0));
            painter.rect_filled(b, 4.0, Color32::from_gray(40));
            let f = Rect::from_min_size(b.min, Vec2::new(b.width() * self.batch_frac as f32, b.height()));
            painter.rect_filled(f, 4.0, Color32::from_rgb(131, 165, 152));
            painter.text(b.center(), egui::Align2::CENTER_CENTER, format!("batch run {:.0} %", 100.0 * self.batch_frac), egui::FontId::proportional(12.0), Color32::WHITE);
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
        let n = 3.0;
        let rects: Vec<Rect> = (0..3)
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
        if self.is_exchanger() {
            residual::paint(&painter, rects[0], &self.residuals, true, "step (flow to < 1e-3, then T to < 1e-7)", 11.0);
            residual::paint(&painter, rects[1], &self.probe_hist, false, "step", 11.0);
            self.exchanger_numbers(&painter, rects[2]);
        } else {
            residual::paint(&painter, rects[0], &self.residuals, true, "time [s] (steady when the blue line < 1e-3)", 11.0);
            residual::paint(&painter, rects[1], &self.probe_hist, false, "time [s]", 11.0);
            self.profile(&painter, rects[2]);
        }
    }

    /// The exchanger's terminal temperatures, heats and effectiveness against
    /// ε-NTU at the solved local UA. GUI drawing (exempt).
    fn exchanger_numbers(&self, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(20, 22, 26, 220));
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
            painter.text(rect.left_top() + Vec2::new(8.0, 8.0 + 18.0 * i as f32), egui::Align2::LEFT_TOP, l, f.clone(), Color32::from_gray(220));
        }
    }

    /// u_axial / U against r / R at the slice, with Hagen–Poiseuille's
    /// 2 (1 − r²/R²). GUI drawing (exempt).
    fn profile(&self, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(20, 22, 26, 220));
        let (Some(me), Some(p), Shape::Cylinder { radius, centre, .. }) = (&self.mesh, &self.last, self.design.shape) else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "profile: run a pipe", egui::FontId::proportional(11.0), Color32::GRAY);
            return;
        };
        let plot = Rect::from_min_max(rect.min + Vec2::new(30.0, 6.0), rect.max - Vec2::new(6.0, 26.0));
        painter.rect_stroke(plot, 0.0, Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
        let to = |r: f64, u: f64| Pos2::new(plot.left() + (r as f32) * plot.width(), plot.bottom() - (u / 2.2) as f32 * plot.height());
        let line: Vec<Pos2> = (0..=40).map(|k| k as f64 / 40.0).map(|r| to(r, 2.0 * (1.0 - r * r))).collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.5, Color32::from_rgb(254, 128, 25))));
        if me.cross.polygons.len() == p.cross_ua.len() {
            for (poly, u) in me.cross.polygons.iter().zip(&p.cross_ua) {
                let n = poly.len().max(1) as f64;
                let (cx, cy) = poly.iter().fold((0.0, 0.0), |a, q| (a.0 + f64::from(q[0]) / n, a.1 + f64::from(q[1]) / n));
                let r = ((cx - centre[0]).powi(2) + (cy - centre[1]).powi(2)).sqrt() / radius;
                painter.circle_filled(to(r.min(1.0), f64::from(*u) / U_IN), 1.6, Color32::from_rgb(131, 165, 152));
            }
        }
        let f = egui::FontId::proportional(11.0);
        painter.text(Pos2::new(plot.left() - 3.0, plot.top()), egui::Align2::RIGHT_TOP, "2.2", f.clone(), Color32::GRAY);
        painter.text(Pos2::new(plot.left() - 3.0, plot.bottom()), egui::Align2::RIGHT_BOTTOM, "0", f.clone(), Color32::GRAY);
        painter.text(
            Pos2::new(plot.center().x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            "u_axial/U against r/R; orange: Hagen–Poiseuille",
            f,
            Color32::GRAY,
        );
    }
}

/// The outline of each polygon of a section. GUI drawing (exempt).
fn outline(painter: &egui::Painter, cam: &Orbit, rect: Rect, sec: &Section, c: Color32) {
    for p in project_section(cam, rect, sec.axis, sec.offset, &sec.polygons) {
        if p.len() >= 3 {
            painter.add(egui::Shape::closed_line(p, Stroke::new(0.6, c)));
        }
    }
}

/// The probe's 3-D point: the cross-section's plane on the shape's axis.
fn probe_point(d: &Design, cross: &Section) -> [f64; 3] {
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
                ui.painter().rect_filled(body, 0.0, BG);
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
