//! The helical-coil steam generator demo's UI: the coil drawn along its
//! length and coloured by the solved water temperature, wall temperature or
//! quality; the axial profiles (temperatures, quality, pressure) drawn by
//! hand (egui_plot is not used, as in `dispersion_web`); and the side panel's
//! sliders. The UI thread only draws; every march runs in [`crate::engine`].

use crate::engine::{Event, Params, Request, Snapshot, TERMS};
use bugis::colour_maps::color32::{hot_to_cold_colour_mark_1, steam_quality_colour_mark_1};
use bugis::colour_maps::hotness;
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::set_title;
use dhoby_ghaut::web_demo::view::{apply_zoom, scale_bar, zoom_buttons, View};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use petir::mathf::RealMath;

type HLink = Link<Request, Event>;

/// The resolved single-phase coil section in Foam Studio (gh:#1157), on the
/// same site (`demos/foam-studio/`).
pub const COIL_CFD_URL: &str = "../foam-studio/?shape=coil";
/// Where the V&V record lives, for the panel's link.
pub const VV_URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/crates/tampines/verification_and_validation/helical_coil_otsg_gh1157.md";

/// Plot and label colours from the theme (`bugis::Ink`): water, hot side,
/// inner wall, quality, pressure, q''/CHF. (The coil itself is coloured by
/// the physics colour maps in [`node_colour`], which stay as they are.)
fn hues(ink: &bugis::Ink) -> [Color32; 6] {
    [ink.accent(0), ink.accent(1), ink.accent(4), ink.accent(2), ink.accent(3), ink.accent(6)]
}

/// What the coil is coloured by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    WaterTemperature,
    WallTemperature,
    Quality,
}

impl Colour {
    pub fn label(self) -> &'static str {
        match self {
            Colour::WaterTemperature => "water temperature",
            Colour::WallTemperature => "inner wall temperature",
            Colour::Quality => "equilibrium quality",
        }
    }
}

/// The colour of node value `v` under `mode`, given the inlet temperatures
/// bounding the temperature scale \[K\].
pub fn node_colour(mode: Colour, v: f64, t_cold: f64, t_hot: f64) -> Color32 {
    match mode {
        Colour::WaterTemperature | Colour::WallTemperature => {
            hot_to_cold_colour_mark_1(hotness(v, t_cold, t_hot))
        }
        Colour::Quality => steam_quality_colour_mark_1(v.clamp(0.0, 1.0) as f32),
    }
}

/// Linear interpolation of a uniformly spaced profile column at tube length
/// `s` (clamped to the tube).
pub fn sample(s_nodes: &[f64], values: &[f64], s: f64) -> f64 {
    let n = s_nodes.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 || s <= s_nodes[0] {
        return values[0];
    }
    let ds = s_nodes[1] - s_nodes[0];
    let f = ((s - s_nodes[0]) / ds).clamp(0.0, (n - 1) as f64);
    let i = (f.floor() as usize).min(n - 2);
    let w = f - i as f64;
    values[i] * (1.0 - w) + values[i + 1] * w
}

/// One drawn segment of the coil: oblique-projected world points \[cm\],
/// whether it lies behind the axis, and its colour.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub back: bool,
    pub colour: Color32,
}

/// Depth factor of the oblique view: a point `y` cm behind the axis is drawn
/// `DEPTH · y` cm higher, so each turn reads as an ellipse.
pub const DEPTH: f64 = 0.35;

/// The coil as segments along its length, from the model's own centreline
/// (`HelicalCoilGeometry::centreline_point`), coloured from the profile.
/// Computed when the result or the colouring changes, never per frame.
pub fn coil_segments(p: &Params, snap: &Snapshot, mode: Colour) -> Vec<Segment> {
    let g = p.geometry();
    let per_turn = if g.turns > 150.0 { 16.0 } else { 24.0 };
    let n = ((g.turns * per_turn).ceil() as usize).clamp(8, 20_000);
    let phi_max = 2.0 * std::f64::consts::PI * g.turns;
    let l = g.tube_length().get::<uom::si::length::meter>();
    let pr = &snap.profile;
    let column: &[f64] = match mode {
        Colour::WaterTemperature => &pr.t_water,
        Colour::WallTemperature => &pr.t_wall,
        Colour::Quality => &pr.quality,
    };
    let proj = |phi: f64| {
        let c = g.centreline_point(phi);
        [c[0] * 100.0, c[2] * 100.0 + DEPTH * c[1] * 100.0]
    };
    (0..n)
        .map(|i| {
            let (f0, f1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
            let phi_mid = 0.5 * (f0 + f1) * phi_max;
            let v = sample(&pr.s, column, 0.5 * (f0 + f1) * l);
            Segment {
                a: proj(f0 * phi_max),
                b: proj(f1 * phi_max),
                back: phi_mid.r_sin() > 0.0,
                colour: node_colour(mode, v, p.feed_temperature_k, p.hot_inlet_k),
            }
        })
        .collect()
}

/// A solve the UI is driving.
#[derive(Clone, Debug)]
pub struct Solve {
    pub id: u32,
    pub params: Params,
    pub in_flight: bool,
    pub last: Option<Snapshot>,
    pub error: Option<String>,
}

impl Solve {
    pub fn new(id: u32, params: Params) -> Self {
        Self {
            id,
            params,
            in_flight: true,
            last: None,
            error: None,
        }
    }
    /// Take in an event; returns whether it was this solve's.
    pub fn receive(&mut self, ev: Event) -> bool {
        match ev {
            Event::Progress(s) if s.id == self.id => {
                self.in_flight = false;
                self.last = Some(s);
                true
            }
            Event::Error { id, message } if id == self.id || id == 0 => {
                self.in_flight = false;
                self.error = Some(message);
                true
            }
            _ => false,
        }
    }
    pub fn converged(&self) -> bool {
        self.last.as_ref().is_some_and(|s| s.converged)
    }
    /// The next march to ask for, if any.
    pub fn next_request(&mut self) -> Option<Request> {
        if self.in_flight || self.error.is_some() || self.converged() || self.last.is_none() {
            return None;
        }
        self.in_flight = true;
        Some(Request::Step { id: self.id })
    }
}

pub struct HelicalApp {
    theme: bugis::Theme,
    link: Option<HLink>,
    link_error: Option<String>,
    panel: Panel,
    view: View,
    params: Params,
    next_id: u32,
    solve: Option<Solve>,
    /// The last converged result and the inputs it belongs to: drawn while a
    /// new solve runs, so the picture never blanks.
    shown: Option<(Params, Snapshot)>,
    colour: Colour,
    segments: Vec<Segment>,
    segments_key: Option<(u32, usize, Colour)>,
    title: String,
}

impl HelicalApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        bugis::Theme::default().apply(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<HLink, String> = {
            let ctx = cc.egui_ctx.clone();
            Ok(dhoby_ghaut::web_demo::link::start_native(
                crate::engine::Engine::default(),
                move || ctx.request_repaint(),
            ))
        };
        #[cfg(target_arch = "wasm32")]
        let link = dhoby_ghaut::web_demo::link::start_web::<Request, Event>(
            cc.egui_ctx.clone(),
            "./worker.js",
            |m| Event::Error { id: 0, message: m },
        );
        let (link, link_error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        let params = Params::default();
        let mut app = Self {
            theme: bugis::Theme::default(),
            link,
            link_error,
            panel: Panel::default(),
            view: Self::view_for(&params),
            params,
            next_id: 1,
            solve: None,
            shown: None,
            colour: Colour::WaterTemperature,
            segments: Vec::new(),
            segments_key: None,
            title: String::new(),
        };
        app.start_solve();
        app
    }

    /// A view that fits the coil: its height and diameter in cm.
    fn view_for(p: &Params) -> View {
        let g = p.geometry();
        let h = g.coil_height().get::<uom::si::length::meter>() * 100.0;
        let d = p.coil_diameter_m * 100.0;
        // 0.7 of the height: room above and below for the labels and the
        // on-screen buttons on a phone.
        View::new(0.7 * h.max(d * (1.0 + DEPTH)) + 2.0).with_home([0.0, 0.5 * h])
    }

    fn start_solve(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.solve = Some(Solve::new(id, self.params));
        if let Some(l) = &self.link {
            l.send(Request::Solve {
                id,
                params: self.params,
            });
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            if let Some(s) = &mut self.solve {
                s.receive(e);
            }
        }
        if let Some(s) = &mut self.solve {
            if s.converged() {
                if let Some(last) = &s.last {
                    let geometry_changed =
                        self.shown.as_ref().is_none_or(|(p, _)| p.geometry() != s.params.geometry());
                    self.shown = Some((s.params, last.clone()));
                    if geometry_changed {
                        // A new coil: Reset fits the new shape.
                        self.view = Self::view_for(&s.params);
                    }
                }
            }
            if let (Some(r), Some(l)) = (s.next_request(), &self.link) {
                l.send(r);
            }
        }
    }

    /// One slider row; returns whether the value moved. GUI drawing (exempt
    /// from the reaching-test rule; the values feed the tested `Params`).
    fn slider(ui: &mut egui::Ui, label: &str, v: &mut f64, lo: f64, hi: f64, scale: f64, unit: &str) -> bool {
        let mut shown = *v * scale;
        let r = ui.add(
            egui::Slider::new(&mut shown, lo..=hi)
                .text(format!("{label} [{unit}]"))
                .clamping(egui::SliderClamping::Always),
        );
        if r.changed() {
            *v = shown / scale;
            true
        } else {
            false
        }
    }

    /// The side panel. GUI drawing (exempt).
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("One tube of a helical-coil once-through steam generator: water climbs the coil from subcooled to superheated, a hot gas or liquid falls past it in counterflow. Solved in this tab's worker by tampines' 1-D march (IF97 water/steam).");
        ui.separator();
        ui.label(RichText::new("Colour the coil by").strong());
        ui.horizontal_wrapped(|ui| {
            for c in [Colour::WaterTemperature, Colour::WallTemperature, Colour::Quality] {
                ui.selectable_value(&mut self.colour, c, c.label());
            }
        });
        let p = &mut self.params;
        let mut changed = false;
        ui.separator();
        ui.label(RichText::new("Coil geometry").strong());
        changed |= Self::slider(ui, "helix diameter", &mut p.coil_diameter_m, 4.0, 200.0, 100.0, "cm");
        changed |= Self::slider(ui, "pitch", &mut p.pitch_m, 1.0, 20.0, 100.0, "cm");
        changed |= Self::slider(ui, "turns", &mut p.turns, 2.0, 400.0, 1.0, "-");
        changed |= Self::slider(ui, "tube bore", &mut p.tube_inner_diameter_m, 5.0, 40.0, 1000.0, "mm");
        changed |= Self::slider(ui, "wall thickness", &mut p.wall_thickness_m, 0.5, 6.0, 1000.0, "mm");
        changed |= Self::slider(ui, "wall conductivity", &mut p.wall_conductivity, 10.0, 60.0, 1.0, "W/(m K)");
        changed |= Self::slider(ui, "parallel tubes", &mut p.tubes, 1.0, 400.0, 1.0, "-");
        let g = p.geometry();
        ui.label(format!(
            "tube length {:.2} m · coil height {:.2} m · d/D_c {:.4}",
            g.tube_length().get::<uom::si::length::meter>(),
            g.coil_height().get::<uom::si::length::meter>(),
            g.curvature_ratio().get::<uom::si::ratio::ratio>()
        ));
        ui.separator();
        ui.label(RichText::new("Water side (all tubes)").strong());
        changed |= Self::slider(ui, "water flow", &mut p.water_flow, 0.1, 20.0, 1.0, "kg/s");
        let mut feed_c = p.feed_temperature_k - 273.15;
        if ui
            .add(egui::Slider::new(&mut feed_c, 20.0..=330.0).text("feed temperature [°C]"))
            .changed()
        {
            p.feed_temperature_k = feed_c + 273.15;
            changed = true;
        }
        changed |= Self::slider(ui, "inlet pressure", &mut p.inlet_pressure_pa, 0.5, 20.0, 1e-6, "MPa");
        ui.separator();
        ui.label(RichText::new("Hot side (constant properties)").strong());
        changed |= Self::slider(ui, "hot flow", &mut p.hot_flow, 0.5, 30.0, 1.0, "kg/s");
        let mut hot_c = p.hot_inlet_k - 273.15;
        if ui
            .add(egui::Slider::new(&mut hot_c, 150.0..=800.0).text("hot inlet [°C]"))
            .changed()
        {
            p.hot_inlet_k = hot_c + 273.15;
            changed = true;
        }
        changed |= Self::slider(ui, "hot c_p", &mut p.hot_cp, 1000.0, 6000.0, 1.0, "J/(kg K)");
        changed |= Self::slider(ui, "shell-side h_o", &mut p.shell_htc, 50.0, 5000.0, 1.0, "W/(m² K)");
        ui.separator();
        ui.label(RichText::new("Numerics and physics terms").strong());
        changed |= Self::slider(ui, "axial cells", &mut p.nodes, 20.0, 400.0, 1.0, "-");
        ui.horizontal_wrapped(|ui| {
            for (i, name) in TERMS.iter().enumerate() {
                let mut on = p.term(i);
                if ui.checkbox(&mut on, *name).changed() {
                    p.terms ^= 1 << i;
                    changed = true;
                }
            }
        });
        ui.label("All terms are on by default; unticking one is an ablation (the library's without_* switch).");
        if ui
            .add_sized([220.0, 36.0], egui::Button::new("Reset to the illustrative case"))
            .clicked()
        {
            *p = Params::default();
            changed = true;
        }
        if changed {
            self.start_solve();
        }
        ui.separator();
        self.results(ui);
        ui.separator();
        ui.label(RichText::new("What this is").strong());
        for line in NOTES {
            ui.label(format!("• {line}"));
        }
        ui.add(egui::Hyperlink::from_label_and_url("V&V record: method, predictions, results", VV_URL).open_in_new_tab(true));
        ui.add(egui::Hyperlink::from_label_and_url("Resolved 3-D CFD of a coil section (the Dean vortices, friction against Mishra & Gupta) in Foam Studio", COIL_CFD_URL).open_in_new_tab(true));
    }

    /// The results grid. GUI drawing (exempt).
    fn results(&self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Result").strong());
        let Some((_, s)) = &self.shown else {
            ui.label("solving…");
            return;
        };
        let pr = &s.profile;
        let n = pr.len() - 1;
        egui::Grid::new("res").num_columns(2).show(ui, |ui| {
            let mut row = |a: &str, b: String| {
                ui.label(a);
                ui.label(b);
                ui.end_row();
            };
            row("duty (water)", format!("{:.3} MW", s.duty_water_w / 1e6));
            row("energy closure", format!("{:.1e}", s.closure));
            row("steam outlet", format!("{:.1} °C, x_eq {:.3}", pr.t_water[n] - 273.15, pr.quality[n]));
            row("hot outlet", format!("{:.1} °C", s.hot_outlet_k - 273.15));
            row("pressure drop", format!("{:.1} kPa", s.dp_total() / 1e3));
            row("  friction · gravity · accel.", format!("{:.1} · {:.1} · {:.1}", s.dp_friction / 1e3, s.dp_gravity / 1e3, s.dp_acceleration / 1e3));
            row("dryout at", if s.dryout_s.is_finite() { format!("{:.2} m of tube", s.dryout_s) } else { "none".into() });
            row("peak inner wall", format!("{:.1} °C", pr.t_wall.iter().fold(f64::MIN, |a, &b| a.max(b)) - 273.15));
            row("marches (shooting)", format!("{} · {:.0} ms the last", s.iteration, s.ms));
        });
    }

    /// The main view. GUI drawing (exempt).
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let full = resp.rect;
        let ink = bugis::Ink::of_ui(ui);
        painter.rect_filled(full, 0.0, ink.bg);
        let wide = full.width() > full.height() * 1.1;
        let (coil, plots) = if wide {
            let split = full.left() + full.width() * 0.42;
            (
                Rect::from_min_max(full.min, Pos2::new(split, full.bottom())),
                Rect::from_min_max(Pos2::new(split + 6.0, full.top() + 52.0), full.max - Vec2::splat(8.0)),
            )
        } else {
            let split = full.top() + full.height() * 0.48;
            (
                Rect::from_min_max(full.min, Pos2::new(full.right(), split)),
                Rect::from_min_max(Pos2::new(full.left() + 8.0, split + 4.0), full.max - Vec2::new(8.0, 44.0)),
            )
        };
        let cresp = ui.interact(coil, ui.id().with("coil"), Sense::click_and_drag());
        self.view.handle_input(ui, &cresp);
        let p = painter.with_clip_rect(coil);
        if let Some((params, snap)) = &self.shown {
            let key = (snap.id, snap.profile.len(), self.colour);
            if self.segments_key != Some(key) {
                self.segments = coil_segments(params, snap, self.colour);
                self.segments_key = Some(key);
            }
            let width = (params.geometry().tube_outer_diameter.get::<uom::si::length::meter>() * 100.0 * self.view.scale)
                .clamp(1.2, 40.0) as f32;
            for pass_back in [true, false] {
                for seg in self.segments.iter().filter(|s| s.back == pass_back) {
                    let c = if seg.back { seg.colour.gamma_multiply(0.45) } else { seg.colour };
                    p.line_segment(
                        [self.view.to_screen(coil, seg.a[0], seg.a[1]), self.view.to_screen(coil, seg.b[0], seg.b[1])],
                        Stroke::new(width, c),
                    );
                }
            }
            draw_flow_labels(&p, coil, &self.view, params);
            draw_legend(&p, coil, self.colour, params);
        }
        scale_bar(&p, coil, &self.view);
        // Status where the reader sees it with the panel folded.
        let font = egui::FontId::proportional(13.0);
        let mut y = coil.top() + 52.0;
        let mut line = |text: String, c: Color32| {
            painter.text(Pos2::new(coil.left() + 10.0, y), egui::Align2::LEFT_TOP, text, font.clone(), c);
            y += 18.0;
        };
        match &self.solve {
            Some(s) if s.error.is_some() => line(format!("refused: {}", s.error.as_deref().unwrap_or("")), ink.error),
            Some(s) if !s.converged() => line(
                format!("solving… shooting march {}", s.last.as_ref().map_or(0, |l| l.iteration)),
                ink.warn,
            ),
            _ => {}
        }
        if let Some(e) = &self.link_error {
            line(e.clone(), ink.error);
        }
        if let Some((_, s)) = &self.shown {
            let n = s.profile.len() - 1;
            line(
                format!(
                    "{:.2} MW · steam {:.0} °C · Δp {:.0} kPa",
                    s.duty_water_w / 1e6,
                    s.profile.t_water[n] - 273.15,
                    s.dp_total() / 1e3
                ),
                ink.text,
            );
        }
        line("research & education only".into(), ink.muted);
        draw_plots(&painter, plots, self.shown.as_ref());
        if let Some(z) = zoom_buttons(ui, coil) {
            apply_zoom(&mut self.view, coil, z);
        }
        self.panel.reopen_button(ui, full);
    }
}

/// Notes for the side panel.
pub const NOTES: [&str; 6] = [
    "Physics: tampines::components::helical_coil_otsg (gh:#1157). Coil friction and heat transfer from Mishra & Gupta (1979), Schmidt (1967) and Gnielinski (1986); flow boiling from Chen (1966) with Forster & Zuber (1955); dryout at the Biasi et al. (1967) CHF; post-dryout from Dougall & Rohsenow (1963); two-phase friction from Lockhart & Martinelli (1949) / Chisholm (1967). Nothing is re-modelled in the demo.",
    "Each march integrates the tube once; shooting on the hot outlet temperature repeats it until the hot side returns its inlet temperature. Each march is one message to the worker, so the page never waits.",
    "Simplified: thermodynamic equilibrium (no vapour superheat beside droplets), no transition boiling, CHF of a straight tube (curvature not credited), a constant shell-side h_o you set, constant hot-side c_p.",
    "The default case is HTR-10-scale but INVENTED where the sources are silent (tube size, pitch, helix diameter, tube count, h_o). Its steam outlet (~554 °C) is not HTR-10's 440 °C, and that disagreement is recorded, not tuned away.",
    "Not validated against helical-coil boiling data: see the V&V record for what was checked (energy closure, straight-tube limits, grid convergence) and what was not.",
    "Education and research only. Not for reactor operation, licensing or safety decisions.",
];

/// Inlet and outlet labels on the coil. GUI drawing (exempt).
fn draw_flow_labels(p: &egui::Painter, rect: Rect, view: &View, params: &Params) {
    let g = params.geometry();
    let h = g.coil_height().get::<uom::si::length::meter>() * 100.0;
    let r = params.coil_diameter_m * 50.0;
    let font = egui::FontId::proportional(12.0);
    let at = |x: f64, y: f64| view.to_screen(rect, x, y);
    let [c_water, c_hot, ..] = hues(&bugis::Ink::of_ctx(p.ctx()));
    p.text(at(r, 0.0) + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, format!("water in (rises) {:.0} °C", params.feed_temperature_k - 273.15), font.clone(), c_water);
    p.text(at(r, h) + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, "steam out", font.clone(), c_water);
    p.text(at(-r, h) + Vec2::new(-10.0, 0.0), egui::Align2::RIGHT_CENTER, format!("hot in (falls) {:.0} °C", params.hot_inlet_k - 273.15), font.clone(), c_hot);
    p.text(at(-r, 0.0) + Vec2::new(-10.0, 0.0), egui::Align2::RIGHT_CENTER, "hot out", font, c_hot);
}

/// The colour scale. GUI drawing (exempt).
fn draw_legend(p: &egui::Painter, rect: Rect, mode: Colour, params: &Params) {
    let (w, h) = (16.0, (rect.height() * 0.35).clamp(60.0, 220.0));
    let x = rect.left() + 12.0;
    let y1 = rect.bottom() - 40.0;
    let steps = 40;
    for i in 0..steps {
        let f = i as f64 / steps as f64;
        let v = match mode {
            Colour::Quality => f,
            _ => params.feed_temperature_k + f * (params.hot_inlet_k - params.feed_temperature_k),
        };
        let yy = y1 - (f as f32) * h;
        p.rect_filled(
            Rect::from_min_size(Pos2::new(x, yy - h / steps as f32), Vec2::new(w, h / steps as f32 + 0.5)),
            0.0,
            node_colour(mode, v, params.feed_temperature_k, params.hot_inlet_k),
        );
    }
    let font = egui::FontId::proportional(11.0);
    let muted = bugis::Ink::of_ctx(p.ctx()).muted;
    let (lo, hi) = match mode {
        Colour::Quality => ("x ≤ 0".to_string(), "x ≥ 1".to_string()),
        _ => (
            format!("{:.0} °C", params.feed_temperature_k - 273.15),
            format!("{:.0} °C", params.hot_inlet_k - 273.15),
        ),
    };
    p.text(Pos2::new(x + w + 4.0, y1), egui::Align2::LEFT_BOTTOM, lo, font.clone(), muted);
    p.text(Pos2::new(x + w + 4.0, y1 - h), egui::Align2::LEFT_TOP, hi, font.clone(), muted);
    p.text(Pos2::new(x, y1 - h - 4.0), egui::Align2::LEFT_BOTTOM, mode.label(), font, muted);
}

/// One hand-drawn line plot. GUI drawing (exempt).
#[allow(clippy::too_many_arguments)]
fn plot(p: &egui::Painter, r: Rect, title: &str, xs: &[f64], series: &[(&[f64], Color32, &str)], lo: f64, hi: f64, unit: &str, marker: Option<f64>) {
    let ink = bugis::Ink::of_ctx(p.ctx());
    p.rect_filled(r, 4.0, ink.card);
    p.rect_stroke(r, 4.0, Stroke::new(1.0, ink.border), StrokeKind::Inside);
    let font = egui::FontId::proportional(11.0);
    p.text(r.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(12.0), ink.text);
    let inner = Rect::from_min_max(r.min + Vec2::new(44.0, 34.0), r.max - Vec2::new(8.0, 16.0));
    if xs.len() < 2 || !(hi > lo) || inner.height() < 10.0 {
        return;
    }
    let x_max = xs[xs.len() - 1].max(1e-9);
    let at = |x: f64, y: f64| {
        Pos2::new(
            inner.left() + (x / x_max) as f32 * inner.width(),
            inner.bottom() - ((y - lo) / (hi - lo)).clamp(-0.05, 1.05) as f32 * inner.height(),
        )
    };
    p.text(Pos2::new(inner.left() - 4.0, inner.top()), egui::Align2::RIGHT_TOP, format!("{hi:.4}").trim_end_matches('0').trim_end_matches('.').to_string(), font.clone(), ink.muted);
    p.text(Pos2::new(inner.left() - 4.0, inner.bottom()), egui::Align2::RIGHT_BOTTOM, format!("{lo:.4}").trim_end_matches('0').trim_end_matches('.').to_string(), font.clone(), ink.muted);
    p.text(Pos2::new(inner.left() - 4.0, inner.center().y), egui::Align2::RIGHT_CENTER, unit, font.clone(), ink.muted);
    p.text(Pos2::new(inner.right(), r.bottom() - 2.0), egui::Align2::RIGHT_BOTTOM, format!("tube length {x_max:.1} m"), font.clone(), ink.muted);
    p.text(Pos2::new(inner.left(), r.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, "0", font.clone(), ink.muted);
    if let Some(m) = marker {
        if m.is_finite() {
            let (a, b) = (at(m, lo), at(m, hi));
            p.line_segment([a, b], Stroke::new(1.0, ink.error));
            p.text(b + Vec2::new(3.0, 2.0), egui::Align2::LEFT_TOP, "dryout", font.clone(), ink.error);
        }
    }
    let mut lx = inner.left() + 4.0;
    for (ys, c, name) in series {
        let pts: Vec<Pos2> = xs.iter().zip(ys.iter()).filter(|(_, y)| y.is_finite()).map(|(&x, &y)| at(x, y)).collect();
        if pts.len() > 1 {
            p.add(egui::Shape::line(pts, Stroke::new(2.0, *c)));
        }
        let galley = p.layout_no_wrap((*name).to_string(), font.clone(), *c);
        let w = galley.size().x;
        p.galley(Pos2::new(lx, r.top() + 18.0), galley, *c);
        lx += w + 12.0;
    }
}

/// The three profile plots. GUI drawing (exempt; the numbers come from the
/// tested engine).
fn draw_plots(p: &egui::Painter, rect: Rect, shown: Option<&(Params, Snapshot)>) {
    if rect.height() < 90.0 || rect.width() < 140.0 {
        return;
    }
    let Some((params, s)) = shown else {
        return;
    };
    let pr = &s.profile;
    let [c_water, c_hot, c_wall, c_qual, c_pres, c_chf] = hues(&bugis::Ink::of_ctx(p.ctx()));
    let gap = 6.0;
    let h = (rect.height() - 2.0 * gap) / 3.0;
    let row = |k: f32| Rect::from_min_size(rect.min + Vec2::new(0.0, k * (h + gap)), Vec2::new(rect.width(), h));
    let c = |v: &[f64]| v.iter().map(|t| t - 273.15).collect::<Vec<f64>>();
    let (tw, th, twall) = (c(&pr.t_water), c(&pr.t_hot), c(&pr.t_wall));
    let t_lo = params.feed_temperature_k - 273.15;
    let t_hi = th.iter().chain(&twall).fold(params.hot_inlet_k - 273.15, |a, &b| a.max(b));
    plot(p, row(0.0), "temperatures along the tube", &pr.s, &[(&th, c_hot, "hot side"), (&twall, c_wall, "inner wall"), (&tw, c_water, "water")], t_lo.floor(), t_hi.ceil(), "°C", Some(s.dryout_s));
    let x_lo = pr.quality.iter().fold(0.0f64, |a, &b| a.min(b)).floor().max(-1.0);
    let x_hi = pr.quality.iter().fold(1.0f64, |a, &b| a.max(b)).ceil();
    let chf: Vec<f64> = pr.chf_ratio.clone();
    plot(p, row(1.0), "equilibrium quality, and q''/q''_CHF while boiling", &pr.s, &[(&pr.quality, c_qual, "x_eq"), (&chf, c_chf, "q''/CHF")], x_lo, x_hi, "-", Some(s.dryout_s));
    let pmpa: Vec<f64> = pr.pressure.iter().map(|v| v / 1e6).collect();
    let (plo, phi) = pmpa.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    let pad = ((phi - plo) * 0.08).max(1e-4);
    plot(
        p,
        row(2.0),
        &format!("pressure: Δp {:.1} kPa (friction {:.1}, gravity {:.1}, accel. {:.1})", s.dp_total() / 1e3, s.dp_friction / 1e3, s.dp_gravity / 1e3, s.dp_acceleration / 1e3),
        &pr.s,
        &[(&pmpa, c_pres, "p")],
        plo - pad,
        phi + pad,
        "MPa",
        None,
    );
}

impl eframe::App for HelicalApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        let title = match &self.shown {
            Some((_, s)) => format!(
                "Helical SG · {:.2} MW · steam {:.0} °C{}",
                s.duty_water_w / 1e6,
                s.profile.t_water[s.profile.len() - 1] - 273.15,
                if self.solve.as_ref().is_some_and(|x| x.converged()) { "" } else { " · solving" }
            ),
            None => "Helical SG · solving".to_string(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Helical steam generator", |ui| {
            bugis::selector(ui, &mut self.theme);
            self.side_panel(ui);
        });
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_and_colours() {
        let s = [0.0, 1.0, 2.0];
        let v = [10.0, 20.0, 40.0];
        assert_eq!(sample(&s, &v, 0.5), 15.0);
        assert_eq!(sample(&s, &v, 1.5), 30.0);
        assert_eq!(sample(&s, &v, -1.0), 10.0);
        assert_eq!(sample(&s, &v, 9.0), 40.0);
        assert!(sample(&[], &[], 1.0).is_nan());
        assert_ne!(node_colour(Colour::WaterTemperature, 300.0, 300.0, 900.0), node_colour(Colour::WaterTemperature, 900.0, 300.0, 900.0));
        assert_eq!(node_colour(Colour::Quality, -0.3, 0.0, 1.0), node_colour(Colour::Quality, 0.0, 0.0, 1.0));
        assert_eq!(Colour::Quality.label(), "equilibrium quality");
    }

    /// The UI pulls one march at a time, ignores another solve's events, and
    /// stops at convergence or an error; the coil is drawn from the model's
    /// own centreline with every segment inside the helix envelope.
    #[test]
    fn a_solve_is_pulled_one_march_at_a_time_and_drawn_from_the_model() {
        let params = Params {
            nodes: 40.0,
            ..Params::default()
        };
        let mut solve = Solve::new(5, params);
        assert_eq!(solve.next_request(), None, "nothing until the first report");
        let mut engine = crate::engine::Engine::default();
        let mut got = Vec::new();
        engine.serve(Request::Solve { id: 5, params }, &mut |e| got.push(e));
        assert!(!solve.receive(Event::Error { id: 4, message: "stale".into() }));
        for e in got.drain(..) {
            assert!(solve.receive(e));
        }
        let mut marches = 1;
        while let Some(r) = solve.next_request() {
            assert_eq!(solve.next_request(), None, "one in flight");
            engine.serve(r, &mut |e| got.push(e));
            for e in got.drain(..) {
                solve.receive(e);
            }
            marches += 1;
            assert!(marches < 100);
        }
        assert!(solve.converged());
        let snap = solve.last.clone().expect("a result");
        let segs = coil_segments(&params, &snap, Colour::Quality);
        let g = params.geometry();
        let r_cm = params.coil_diameter_m * 50.0;
        let h_cm = g.coil_height().get::<uom::si::length::meter>() * 100.0;
        assert!(segs.len() >= (g.turns * 24.0) as usize);
        for s in &segs {
            for q in [s.a, s.b] {
                assert!(q[0].abs() <= r_cm + 1e-9);
                assert!(q[1] >= -DEPTH * r_cm - 1e-9 && q[1] <= h_cm + DEPTH * r_cm + 1e-9);
            }
        }
        // Coloured from the profile: subcooled inlet blue-ish, superheated
        // outlet white (quality map), so the ends differ.
        assert_ne!(segs[0].colour, segs[segs.len() - 1].colour);
        let mut errs = Solve::new(6, params);
        errs.receive(Event::Error { id: 6, message: "bad".into() });
        assert_eq!(errs.next_request(), None);
    }
}
