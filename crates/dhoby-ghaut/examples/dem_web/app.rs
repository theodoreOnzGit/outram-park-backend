//! The DEM pour demo's UI: the vessel cut away with the pebbles of the live
//! pour (or the baked full-size bed), the packing fraction and the kinetic
//! energy against time, and the side panel. The UI thread only draws; the
//! pour runs in [`crate::engine`] one short chunk per request.

use crate::engine::{Event, Request, Snapshot};
use crate::htr10_beds::{self, draw, Beds};
use dhoby_ghaut::web_demo::lesson::SITE;
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{query_pairs, query_value, set_query, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, scale_bar, zoom_buttons, View};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

type DLink = Link<Request, Event>;

/// The pour's seed (`htr10_fill`'s default).
pub const SEED: u64 = 0x5EED_0010;
/// Pebble counts the reader can pour. The full core holds 27 554.
pub const COUNTS: [usize; 4] = [1000, 2000, 4000, 8000];
/// Opening count.
pub const DEFAULT_N: usize = 4000;
/// Target compute per worker request, ms: short enough that a new pour or a
/// pause is served at once, long enough that messaging is not the cost.
pub const CHUNK_MS: f64 = 120.0;
/// Depth of the cut-away behind the plane y = 0 \[m\]: two diameters.
pub const DEPTH_M: f32 = 0.12;
/// The full-size bake's whole-core φ (gh:#216, re-run 2026-10-07 for gh:#787)
/// and the published figure it is compared with (quoted, IAEA-TECDOC-1382).
pub const FULL_PHI: f64 = 0.6047;
pub const PUBLISHED_PHI: f64 = 0.61;
/// [`crate::engine::vessel_bulk_fraction`] of the same full-size bed
/// (measured 2026-10-07, `--headless`).
pub const FULL_BULK_PHI: f64 = 0.6061;
/// The lesson page for this demo, relative to [`SITE`].
pub const LESSON: &str = "tutorials/monte-carlo/dem.html";

const BG: Color32 = Color32::from_rgb(14, 16, 20);
const PHI_COLOUR: Color32 = Color32::from_rgb(120, 200, 255);
const KE_COLOUR: Color32 = Color32::from_rgb(255, 180, 90);

/// What the main view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// The live pour.
    Live,
    /// The baked full-size gh:#216 bed.
    Baked,
}

impl Show {
    pub fn name(self) -> &'static str {
        match self {
            Show::Live => "live",
            Show::Baked => "baked",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [Show::Live, Show::Baked]
            .into_iter()
            .find(|v| v.name() == s)
    }
}

/// One point of the pour's history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub phi: f64,
    pub bulk: f64,
    pub ke: f64,
}

/// A pour the UI is driving.
pub struct Pour {
    pub id: u32,
    pub last: Option<Snapshot>,
    pub history: Vec<Sample>,
    pub in_flight: bool,
    pub paused: bool,
    /// Steps per request, adapted to [`CHUNK_MS`].
    pub chunk: usize,
    /// Compute spent on this pour so far, ms.
    pub compute_ms: f64,
}

impl Pour {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            last: None,
            history: Vec::new(),
            in_flight: true,
            paused: false,
            chunk: 20,
            compute_ms: 0.0,
        }
    }
    /// Take in a report of this pour (a report for another pour is ignored).
    /// Returns whether it was taken.
    pub fn receive(&mut self, s: Snapshot) -> bool {
        if s.id != self.id {
            return false;
        }
        self.in_flight = false;
        if s.ms_per_step > 0.0 {
            let steps_done = s
                .steps
                .saturating_sub(self.last.as_ref().map_or(0, |l| l.steps));
            self.compute_ms += s.ms_per_step * steps_done as f64;
            self.chunk = ((CHUNK_MS / s.ms_per_step) as usize).clamp(2, 4000);
        }
        if s.n_core > 0 && s.phi.is_finite() {
            self.history.push(Sample {
                t: s.time_s,
                phi: s.phi,
                bulk: s.phi_bulk,
                ke: s.ke_ratio,
            });
        }
        self.last = Some(s);
        true
    }
    /// Over: settled, or out of steps.
    pub fn finished(&self) -> bool {
        self.last.as_ref().is_some_and(|s| s.settled || s.gave_up)
    }
    /// The next request, if one should go now.
    pub fn next_request(&mut self) -> Option<Request> {
        if self.in_flight || self.paused || self.finished() || self.last.is_none() {
            return None;
        }
        self.in_flight = true;
        Some(Request::Step {
            id: self.id,
            steps: self.chunk,
        })
    }
}

pub struct DemApp {
    link: Option<DLink>,
    error: Option<String>,
    panel: Panel,
    view: View,
    n_choice: usize,
    next_id: u32,
    pour: Option<Pour>,
    show: Show,
    baked: Option<Result<Beds, String>>,
    title: String,
}

/// Opening state from the URL (`?n=4000&view=live`), clamped to the choices.
pub fn from_query(q: &[(String, String)]) -> (usize, Show) {
    let n = query_value(q, "n")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| COUNTS.contains(n))
        .unwrap_or(DEFAULT_N);
    let show = query_value(q, "view")
        .and_then(Show::parse)
        .unwrap_or(Show::Live);
    (n, show)
}

impl DemApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<DLink, String> = {
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
            Event::Error,
        );
        let (link, error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        let (n_choice, show) = from_query(&query_pairs());
        // World units are cm; the vessel is 180 cm wide and runs from the
        // valve (−62 cm) to above the full bed (~190 cm).
        let view = View::new(130.0).with_home([0.0, 64.0]);
        let mut app = Self {
            link,
            error,
            panel: Panel::default(),
            view,
            n_choice,
            next_id: 1,
            pour: None,
            show,
            baked: None,
            title: String::new(),
        };
        app.start_pour();
        app
    }

    fn start_pour(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.pour = Some(Pour::new(id));
        if let Some(link) = &self.link {
            link.send(Request::Start {
                id,
                n: self.n_choice,
                seed: SEED,
            });
        }
        self.set_url();
    }

    fn set_url(&self) {
        set_query(&[
            ("n", &self.n_choice.to_string()),
            ("view", self.show.name()),
        ]);
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match e {
                Event::Error(m) => self.error = Some(m),
                Event::Progress(s) => {
                    if let Some(p) = &mut self.pour {
                        p.receive(s);
                    }
                }
            }
        }
        if let (Some(p), Some(link)) = (&mut self.pour, &self.link) {
            if let Some(r) = p.next_request() {
                link.send(r);
            }
        }
    }

    /// The side panel. GUI drawing (exempt from the reaching-test rule; its
    /// state changes go through tested [`Pour`] methods).
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("HTR-10 pebbles poured into the vessel and settled by the LIGGGHTS port's GranularSystem, running in this tab's worker.");
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("Show:");
            let before = self.show;
            ui.selectable_value(&mut self.show, Show::Live, "live pour");
            ui.selectable_value(&mut self.show, Show::Baked, "full-size bed (baked)");
            if self.show != before {
                self.set_url();
            }
        });
        ui.separator();
        ui.label(RichText::new("Pebbles to pour").strong());
        ui.horizontal_wrapped(|ui| {
            for n in COUNTS {
                ui.selectable_value(&mut self.n_choice, n, format!("{n}"));
            }
        });
        ui.label("The full core holds 27 554. A browser has one thread, so it pours a reduced count; the full-size bed is the baked view.");
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_sized([110.0, 36.0], egui::Button::new("Pour again"))
                .clicked()
            {
                self.start_pour();
            }
            if let Some(p) = &mut self.pour {
                let label = if p.paused { "Resume" } else { "Pause" };
                if ui
                    .add_sized([90.0, 36.0], egui::Button::new(label))
                    .clicked()
                {
                    p.paused = !p.paused;
                }
            }
        });
        if let Some(p) = &self.pour {
            if let Some(s) = &p.last {
                egui::Grid::new("pour").num_columns(2).show(ui, |ui| {
                    ui.label("steps (35 µs each)");
                    ui.label(format!("{}", s.steps));
                    ui.end_row();
                    ui.label("simulated time");
                    ui.label(format!("{:.3} s", s.time_s));
                    ui.end_row();
                    ui.label("pebbles above the floor");
                    ui.label(format!("{} of {}", s.n_core, s.n));
                    ui.end_row();
                    ui.label("bulk φ of the filled vessel");
                    ui.label(if s.phi_bulk.is_finite() {
                        format!("{:.4}", s.phi_bulk)
                    } else {
                        "— (bed too shallow)".into()
                    });
                    ui.end_row();
                    ui.label("whole-core φ (gh:#216)");
                    ui.label(if s.n_core > 0 {
                        format!("{:.4}", s.phi)
                    } else {
                        "—".into()
                    });
                    ui.end_row();
                    ui.label("KE / one-radius drop");
                    ui.label(if s.ke_ratio.is_finite() {
                        format!("{:.2e}", s.ke_ratio)
                    } else {
                        "—".into()
                    });
                    ui.end_row();
                    ui.label("cost here");
                    ui.label(format!("{:.2} ms/step", s.ms_per_step));
                    ui.end_row();
                });
            }
        }
        ui.separator();
        ui.label(RichText::new("What this is").strong());
        for line in NOTES {
            ui.label(format!("• {line}"));
        }
        ui.separator();
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "What's happening here? (the lesson)",
                format!("{SITE}{LESSON}"),
            )
            .open_in_new_tab(true),
        );
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "The lattice bed beside this random bed (HTR-10 rung)",
                format!("{SITE}demos/monte-carlo/?rung=htr10&mode=watch&view=beds"),
            )
            .open_in_new_tab(true),
        );
    }

    /// The main view. GUI drawing (exempt).
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let full = resp.rect;
        painter.rect_filled(full, 0.0, BG);
        // Plots beside the vessel on a wide screen, under it on a phone.
        let wide = full.width() > full.height() * 1.1;
        let (vessel, plots) = if wide {
            let split = full.left() + full.width() * 0.6;
            (
                Rect::from_min_max(full.min, Pos2::new(split, full.bottom())),
                Rect::from_min_max(
                    Pos2::new(split, full.top() + 52.0),
                    full.max - Vec2::splat(8.0),
                ),
            )
        } else {
            let split = full.top() + full.height() * 0.64;
            (
                Rect::from_min_max(full.min, Pos2::new(full.right(), split)),
                Rect::from_min_max(
                    Pos2::new(full.left() + 8.0, split),
                    full.max - Vec2::splat(8.0),
                ),
            )
        };
        let vresp = ui.interact(vessel, ui.id().with("vessel"), Sense::click_and_drag());
        self.view.handle_input(ui, &vresp);
        let p = painter.with_clip_rect(vessel);
        let (kept, top_m, label) = match self.show {
            Show::Live => {
                let s = self.pour.as_ref().and_then(|p| p.last.as_ref());
                let c: Vec<[f32; 3]> = s
                    .map(|s| {
                        s.centres
                            .chunks_exact(3)
                            .map(|c| [c[0], c[1], c[2]])
                            .collect()
                    })
                    .unwrap_or_default();
                let top = c.iter().map(|c| f64::from(c[2])).fold(0.4f64, f64::max) + 0.05;
                (
                    htr10_beds::cut_away(&c, DEPTH_M),
                    top.min(2.4),
                    format!(
                        "Live pour: {} pebbles (reduced; the full core holds 27 554)",
                        self.n_choice
                    ),
                )
            }
            Show::Baked => match self
                .baked
                .get_or_insert_with(|| htr10_beds::decode(htr10_beds::BAKED))
            {
                Ok(b) => (
                    htr10_beds::cut_away(&b.dem.centres, DEPTH_M),
                    2.0,
                    "Full-size bed: 27 554 pebbles, GranularSystem, gh:#216 (baked)".to_string(),
                ),
                Err(e) => (Vec::new(), 2.0, format!("baked bed unreadable: {e}")),
            },
        };
        draw::vessel_side(&p, vessel, &self.view, 0.0, top_m);
        draw::cut_away(&p, vessel, &self.view, &kept, DEPTH_M, draw::PEBBLE);
        scale_bar(&p, vessel, &self.view);
        let text = egui::FontId::proportional(13.0);
        let mut y = vessel.top() + 52.0;
        for line in [
            label.as_str(),
            "cut away: pebbles centred up to 12 cm behind the plane y = 0",
            "no k_eff is computed for any bed in this demo",
        ] {
            painter.text(
                Pos2::new(vessel.left() + 10.0, y),
                egui::Align2::LEFT_TOP,
                line,
                text.clone(),
                Color32::from_rgb(210, 214, 222),
            );
            y += 18.0;
        }
        if let Some(e) = &self.error {
            painter.text(
                Pos2::new(vessel.left() + 10.0, y + 6.0),
                egui::Align2::LEFT_TOP,
                e,
                text.clone(),
                Color32::from_rgb(255, 120, 120),
            );
        }
        draw_plots(&painter, plots, self.pour.as_ref(), self.show);
        if let Some(z) = zoom_buttons(ui, vessel) {
            apply_zoom(&mut self.view, vessel, z);
        }
        self.panel.reopen_button(ui, full);
    }
}

/// Notes for the side panel, one bullet each.
pub const NOTES: [&str; 6] = [
    "Physics: outram-park-fork-liggghts' htr10_fill, the GranularSystem engine (Hertz contacts with shear history), verified against upstream LIGGGHTS on the HTR-10 core. Nothing is re-modelled here.",
    "Settings: the gh:#216 study's µ = 0.1 (graphite on graphite, from the literature, not fitted), µ_r = 0, E = 5e8 Pa, ν = 0.2, e = 0.5, dt = 35 µs; the published conus as a mesh wall.",
    "The pebbles start at random in a loose column and fall; the pour has settled when their kinetic energy falls below 1e-3 of a one-radius drop.",
    "Two measures of φ. The bulk φ of the filled vessel (thick line) leaves out 4 radii at the valve and at the surface, as the LIGGGHTS port's bulk-slab measure does; on the full-size bed it reads 0.606. The whole-core φ (thin line), N V / (π R² h) above the conus, is gh:#216's like-for-like measure against the published 0.61; a reduced pour stands only ~15 cm above the conus, so its rough top dominates and this φ stays far lower.",
    "The full-size view is the 27 554-pebble bed of the gh:#216 run at the same settings, re-run on 2026-10-07 (byte-identical): whole-core φ 0.6047 against the published 0.61.",
    "Education and research only. Not for reactor operation, licensing or safety decisions.",
];

/// The φ and kinetic-energy plots. GUI drawing (exempt; the samples come
/// from tested [`Pour::receive`]).
fn draw_plots(p: &egui::Painter, rect: Rect, pour: Option<&Pour>, show: Show) {
    if rect.height() < 60.0 || rect.width() < 120.0 {
        return;
    }
    let gap = 8.0;
    let h = (rect.height() - gap) / 2.0;
    let r_phi = Rect::from_min_size(rect.min, Vec2::new(rect.width(), h));
    let r_ke = Rect::from_min_size(
        rect.min + Vec2::new(0.0, h + gap),
        Vec2::new(rect.width(), h),
    );
    let hist: &[Sample] = pour.map_or(&[], |p| &p.history);
    let t_max = hist.last().map_or(0.1, |s| s.t).max(0.05);
    let font = egui::FontId::proportional(11.0);
    let frame = |r: Rect, title: &str| {
        p.rect_filled(r, 4.0, Color32::from_rgb(20, 23, 30));
        p.rect_stroke(
            r,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(60, 66, 80)),
            StrokeKind::Inside,
        );
        p.text(
            r.left_top() + Vec2::new(6.0, 3.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    };
    // φ against time: this pour's bulk φ (solid) and whole-core φ (thin),
    // with the full-size bed's values and the published figure as lines.
    let (lo, hi) = (0.30, 0.65);
    let at = |r: Rect, t: f64, v: f64, lo: f64, hi: f64| {
        // The top 36 px hold the title and the legend.
        Pos2::new(
            r.left() + (t / t_max) as f32 * r.width(),
            r.bottom() - ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 * (r.height() - 36.0),
        )
    };
    frame(
        r_phi,
        if show == Show::Live {
            "this pour's φ against simulated time"
        } else {
            "φ of the live pour (switch to it to watch)"
        },
    );
    let refs = [
        (
            FULL_BULK_PHI,
            format!("full-size bed: {FULL_BULK_PHI:.3} bulk, {FULL_PHI:.4} whole-core"),
            Color32::from_rgb(160, 160, 170),
            egui::Align2::RIGHT_TOP,
        ),
        (
            PUBLISHED_PHI,
            format!("{PUBLISHED_PHI} published whole-core (quoted)"),
            Color32::from_rgb(235, 235, 235),
            egui::Align2::RIGHT_BOTTOM,
        ),
    ];
    for (v, label, c, align) in refs {
        let (a, b) = (at(r_phi, 0.0, v, lo, hi), at(r_phi, t_max, v, lo, hi));
        p.line_segment([a, b], Stroke::new(1.0, c));
        p.text(
            b + Vec2::new(
                -4.0,
                if align == egui::Align2::RIGHT_TOP {
                    1.0
                } else {
                    -1.0
                },
            ),
            align,
            label,
            font.clone(),
            c,
        );
    }
    p.text(
        r_phi.right_bottom() + Vec2::new(-4.0, -3.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{lo}  ·  t = {t_max:.2} s"),
        font.clone(),
        Color32::GRAY,
    );
    let whole: Vec<Pos2> = hist.iter().map(|s| at(r_phi, s.t, s.phi, lo, hi)).collect();
    if whole.len() > 1 {
        p.add(egui::Shape::line(
            whole,
            Stroke::new(1.0, Color32::from_rgb(140, 150, 175)),
        ));
    }
    let bulk: Vec<Pos2> = hist
        .iter()
        .filter(|s| s.bulk.is_finite())
        .map(|s| at(r_phi, s.t, s.bulk, lo, hi))
        .collect();
    if bulk.len() > 1 {
        p.add(egui::Shape::line(bulk, Stroke::new(2.5, PHI_COLOUR)));
    }
    p.text(
        r_phi.left_top() + Vec2::new(6.0, 18.0),
        egui::Align2::LEFT_TOP,
        "thick: bulk φ of the filled vessel · thin: whole-core φ",
        font.clone(),
        PHI_COLOUR,
    );
    // KE on a log scale, 1e-4 .. 1e2, with the settle target.
    frame(r_ke, "kinetic energy per pebble / one-radius drop (log)");
    let lg = |v: f64| v.max(1e-6).log10();
    let target = at(r_ke, 0.0, lg(1e-3), -4.0, 2.0);
    p.line_segment(
        [target, Pos2::new(r_ke.right(), target.y)],
        Stroke::new(1.0, Color32::from_rgb(160, 200, 160)),
    );
    p.text(
        target + Vec2::new(4.0, 1.0),
        egui::Align2::LEFT_TOP,
        "settled below 1e-3",
        font.clone(),
        Color32::from_rgb(160, 200, 160),
    );
    let line: Vec<Pos2> = hist
        .iter()
        .filter(|s| s.ke.is_finite())
        .map(|s| at(r_ke, s.t, lg(s.ke), -4.0, 2.0))
        .collect();
    if line.len() > 1 {
        p.add(egui::Shape::line(line, Stroke::new(2.0, KE_COLOUR)));
    }
    if let Some(s) = pour.and_then(|p| p.last.as_ref()) {
        let status = if s.settled {
            format!("settled at step {}", s.steps)
        } else if s.gave_up {
            format!("stopped at the step limit ({}) before settling", s.steps)
        } else if s.n_core == 0 {
            "falling: no pebble above the floor yet".to_string()
        } else {
            "settling…".to_string()
        };
        p.text(
            r_ke.right_top() + Vec2::new(-6.0, 3.0),
            egui::Align2::RIGHT_TOP,
            status,
            font,
            Color32::from_rgb(220, 220, 160),
        );
    }
}

impl eframe::App for DemApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        // No polling: the link repaints when the worker's next chunk arrives,
        // so the page paints at the pour's pace (about 8 times a second).
        // The title carries the pour's progress (a reader's tab, and the
        // headless browser check, can read it).
        let title = match self.pour.as_ref().and_then(|p| p.last.as_ref()) {
            Some(s) => format!(
                "DEM pour · {} pebbles · step {} · {:.1} ms/step{}",
                s.n,
                s.steps,
                s.ms_per_step,
                if s.settled { " · settled" } else { "" }
            ),
            None => format!("DEM pour · {} pebbles", self.n_choice),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "DEM pour", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: u32, steps: usize, ms: f64, settled: bool) -> Snapshot {
        Snapshot {
            id,
            n: 10,
            steps,
            time_s: steps as f64 * 3.5e-5,
            ke_ratio: 1e-2,
            phi: 0.55,
            phi_bulk: 0.58,
            surface_m: 0.2,
            n_core: 5,
            settled,
            gave_up: false,
            ms_per_step: ms,
            centres: Vec::new(),
        }
    }

    /// The pour asks for one chunk at a time, sizes it to the measured cost,
    /// ignores another pour's report, and stops asking once settled or paused.
    #[test]
    fn a_pour_is_pulled_one_chunk_at_a_time() {
        let mut p = Pour::new(4);
        assert_eq!(p.next_request(), None, "nothing until the start report");
        assert!(p.receive(snap(4, 0, 0.0, false)));
        assert_eq!(p.next_request(), Some(Request::Step { id: 4, steps: 20 }));
        assert_eq!(p.next_request(), None, "one request in flight at a time");
        assert!(
            !p.receive(snap(3, 99, 1.0, false)),
            "a replaced pour's report is ignored"
        );
        assert!(p.receive(snap(4, 20, 2.0, false)));
        assert_eq!(
            p.next_request(),
            Some(Request::Step { id: 4, steps: 60 }),
            "120 ms / 2 ms per step"
        );
        assert!((p.compute_ms - 40.0).abs() < 1e-9);
        p.receive(snap(4, 80, 2.0, false));
        p.paused = true;
        assert_eq!(p.next_request(), None);
        p.paused = false;
        p.receive(snap(4, 140, 2.0, true));
        assert!(p.finished());
        assert_eq!(p.next_request(), None);
        assert_eq!(p.history.len(), 4);
    }

    /// The full-size figures the plot draws are those of the baked bed.
    #[test]
    fn the_plotted_full_size_figures_are_the_baked_beds() {
        let b = htr10_beds::decode(htr10_beds::BAKED).expect("baked");
        let c: Vec<[f64; 3]> = b
            .dem
            .centres
            .iter()
            .map(|c| [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])])
            .collect();
        assert!((crate::engine::vessel_bulk_fraction(&c) - FULL_BULK_PHI).abs() < 5e-5);
        assert!((b.dem.stats().phi_whole_core - FULL_PHI).abs() < 5e-5);
    }

    #[test]
    fn the_url_picks_the_count_and_the_view() {
        let q = |s: &[(&str, &str)]| {
            s.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            from_query(&q(&[("n", "2000"), ("view", "baked")])),
            (2000, Show::Baked)
        );
        assert_eq!(
            from_query(&q(&[("n", "123")])),
            (DEFAULT_N, Show::Live),
            "only the offered counts"
        );
        assert_eq!(Show::parse(Show::Baked.name()), Some(Show::Baked));
    }
}
