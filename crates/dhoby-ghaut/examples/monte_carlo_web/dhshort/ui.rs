//! The page side of the `dhshort` demo: the slice of the chosen treatment
//! (the shared [`Slicer`]) beside a chart of the recorded bias and speed of
//! every treatment ([`super::record`]).

use super::model::{keeps_and_gives_up, palette, SHORT, TREATMENTS};
use super::record::{bias, RECORDS};
use super::MSG_DESCRIBE;
use crate::raster::{Basis, Preset, RasterInfo, Slicer};
use crate::walkdemo::{split, Outgoing};
use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};

/// One colour per record, oldest first.
pub const RECORD_COLOURS: [Color32; 4] = [
    Color32::from_rgb(235, 235, 235),
    Color32::from_rgb(140, 146, 160),
    Color32::from_rgb(120, 170, 255),
    Color32::from_rgb(255, 170, 90),
];

/// What the worker said about one treatment's universe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Built {
    pub secs: f64,
    pub particles: usize,
    pub packing_fraction: f64,
    pub materials: usize,
}

pub struct ShortcutsView {
    pub slicer: Slicer,
    /// The treatment shown (index into [`TREATMENTS`]).
    pub t: usize,
    /// Redraws asked for (CLS and SCLS sample: a redraw is new flights).
    pub draws: u32,
    pub built: [Option<Built>; 7],
    /// A describe in flight, for this treatment.
    asked: Option<usize>,
}

/// The zoom ladder: the unit cell, the fuel-zone edge (where the pebble's
/// shell and the cut particles meet), and a few particles.
pub fn ladder() -> Vec<Preset> {
    let xy = |label, centre, half| Preset {
        label,
        basis: Basis::Xy,
        centre,
        depth: 0.0,
        half,
    };
    vec![
        xy("unit cell", [0.0, 0.0], 3.1),
        xy("fuel-zone edge", [1.8, 0.0], 0.35),
        xy("particles", [0.6, 0.3], 0.12),
    ]
}

impl ShortcutsView {
    pub fn new() -> Self {
        let info = RasterInfo {
            palette: palette(0),
            ladder: ladder(),
            source: "DhUniverse::pebble(fhr_unit_cell, treatment), sliced by DhUniverse::material_at in the worker",
            start: 0,
            param: 0.0,
        };
        Self {
            slicer: Slicer::new(info, 0, 0.0),
            t: 0,
            draws: 0,
            built: [None; 7],
            asked: None,
        }
    }

    /// Show treatment `t`: its palette, and a new slice (the raster parameter
    /// carries the treatment and the redraw count).
    pub fn select(&mut self, t: usize) {
        if t < TREATMENTS.len() {
            self.t = t;
            self.slicer.info.palette = palette(t);
            self.slicer.param = self.param();
        }
    }

    /// Ask for new flights through a sampled (CLS/SCLS) arm.
    pub fn redraw(&mut self) {
        self.draws += 1;
        self.slicer.param = self.param();
    }

    fn param(&self) -> f64 {
        self.t as f64 + (self.draws % 999) as f64 * 1.0e-3
    }

    /// Whether treatment `t` samples its geometry (so a redraw changes it).
    pub fn samples(t: usize) -> bool {
        matches!(t, 1 | 2 | 5 | 6)
    }

    /// The describe for the shown treatment, once, if it is not known yet.
    pub fn pump(&mut self) -> Vec<Outgoing> {
        if self.built[self.t].is_none() && self.asked.is_none() {
            self.asked = Some(self.t);
            return vec![Outgoing::Walk(vec![MSG_DESCRIBE, self.t as f64])];
        }
        Vec::new()
    }

    /// `[MSG_DESCRIBE, i, secs, particles, pf, materials]`.
    pub fn receive(&mut self, msg: &[f64]) -> Result<(), String> {
        match msg {
            [c, i, secs, n, pf, m] if *c == MSG_DESCRIBE => {
                let i = *i as usize;
                let slot = self.built.get_mut(i).ok_or("describe: no such treatment")?;
                *slot = Some(Built {
                    secs: *secs,
                    particles: *n as usize,
                    packing_fraction: *pf,
                    materials: *m as usize,
                });
                if self.asked == Some(i) {
                    self.asked = None;
                }
                Ok(())
            }
            _ => Err(format!(
                "dhshort: unexpected answer of {} values",
                msg.len()
            )),
        }
    }

    pub fn status(&self) -> String {
        let state = if self.slicer.busy() {
            "slicing"
        } else if self.slicer.settled() {
            "slice ready"
        } else {
            "waiting"
        };
        format!(
            "DH shortcuts · {} · {} · {state}",
            SHORT[self.t], self.slicer.info.ladder[self.slicer.preset].label
        )
    }

    /// One line about what the shown universe stores.
    pub fn built_line(&self) -> String {
        match self.built[self.t] {
            None => "building this treatment's universe in the worker…".into(),
            Some(b) if b.particles > 0 => format!(
                "{} particles stored, packing fraction {:.4} realised (pack_in_ball), built in {:.2} s",
                b.particles, b.packing_fraction, b.secs
            ),
            Some(b) => format!("0 particles stored; models packing fraction {:.4}; built in {:.3} s", b.packing_fraction, b.secs),
        }
    }

    /// The panel. GUI drawing: exempt from the reaching-test rule; the state it
    /// changes goes through [`Self::select`] and [`Self::redraw`], which are tested.
    pub fn panel(&mut self, ui: &mut egui::Ui) -> Vec<Outgoing> {
        ui.label("The same FHR unit cell under each treatment. The picture is what the treatment's material_at answers at each point: the call the delta-tracked power iteration makes.");
        let mut pick = None;
        ui.horizontal_wrapped(|ui| {
            for (i, s) in SHORT.iter().enumerate() {
                if ui
                    .selectable_label(i == self.t, RichText::new(*s).size(14.0))
                    .clicked()
                {
                    pick = Some(i);
                }
            }
        });
        if let Some(i) = pick {
            self.select(i);
        }
        let (keeps, gives) = keeps_and_gives_up(self.t);
        ui.label(RichText::new(TREATMENTS[self.t].name()).strong());
        ui.label(format!("Keeps: {keeps}."));
        ui.label(format!("Gives up: {gives}."));
        ui.weak(self.built_line());
        if Self::samples(self.t) && ui.button("Redraw: new flights").clicked() {
            self.redraw();
        }
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("Zoom to:");
            for (i, p) in self.slicer.info.ladder.iter().enumerate() {
                if ui
                    .selectable_label(i == self.slicer.preset, p.label)
                    .clicked()
                {
                    go = Some(i);
                }
            }
        });
        if let Some(i) = go {
            self.slicer.go(i);
        }
        ui.strong("In this slice");
        for (c, name) in self.slicer.present() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
        ui.separator();
        ui.strong("Recorded, k against delta tracking on the same record");
        for (ri, r) in RECORDS.iter().enumerate() {
            egui::CollapsingHeader::new(
                RichText::new(format!("{} · {}", r.date, r.stats)).color(RECORD_COLOURS[ri]),
            )
            .default_open(ri == 0)
            .show(ui, |ui| {
                for a in r.arms {
                    let b = bias(r, a.t).map_or("reference".to_string(), |(d, s)| {
                        format!("{d:+.0} ± {s:.0} pcm ({:.1}σ)", d.abs() / s.max(1.0))
                    });
                    let sp = a.speed.map_or(String::new(), |s| format!(", {s:.2}×"));
                    ui.label(format!(
                        "{}: {:.5} ± {:.5}, {b}{sp}",
                        SHORT[a.t], a.k, a.sigma
                    ));
                }
                ui.weak(format!("{}. {}. Source: {}.", r.host, r.status, r.source));
            });
        }
        ui.weak("Every record is re-measurement pending (#582). Quote speed as a ratio measured on one host, never seconds across hosts.");
        Vec::new()
    }

    /// The main view. GUI drawing: exempt from the reaching-test rule.
    pub fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        full: Rect,
        painter: &egui::Painter,
        resp: &egui::Response,
    ) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let (rect, chart) = split(full);
        let pic = painter.with_clip_rect(rect);
        let _ = resp; // the app's response covers the chart too; the picture gets its own
        let picture = ui.interact(
            rect,
            ui.id().with("walk-picture"),
            egui::Sense::click_and_drag(),
        );
        self.slicer.view.handle_input(ui, &picture);
        if let Some(req) = self.slicer.pump(rect) {
            out.push(Outgoing::Raster(req));
        }
        self.slicer.draw(&pic, rect);
        if self.slicer.busy() || !self.slicer.settled() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(80));
        }
        let msg = if self.slicer.busy() {
            "asking material_at for every pixel, in the worker…"
        } else {
            ""
        };
        pic.text(
            rect.left_bottom() + Vec2::new(16.0, -60.0),
            egui::Align2::LEFT_BOTTOM,
            msg,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(250, 200, 80),
        );
        // The treatment, finger-sized, on the main view (the panel is folded on a phone).
        let n = SHORT.len();
        let cols = if rect.width() < 520.0 { 4 } else { n };
        let bw = ((rect.width() - 20.0 - 4.0 * (cols as f32 - 1.0)) / cols as f32).min(118.0);
        let rows = n.div_ceil(cols);
        let strip = Rect::from_min_size(
            rect.left_top() + Vec2::new(4.0, 50.0),
            Vec2::new(cols as f32 * (bw + 4.0) + 12.0, rows as f32 * 38.0 + 8.0),
        );
        pic.rect_filled(strip, 6.0, Color32::from_rgba_unmultiplied(14, 16, 20, 215));
        let mut pick = None;
        for (i, s) in SHORT.iter().enumerate() {
            let b = Rect::from_min_size(
                strip.left_top()
                    + Vec2::new(
                        6.0 + (i % cols) as f32 * (bw + 4.0),
                        4.0 + (i / cols) as f32 * 38.0,
                    ),
                Vec2::new(bw, 34.0),
            );
            if ui
                .put(
                    b,
                    egui::Button::selectable(i == self.t, RichText::new(*s).size(12.5)),
                )
                .clicked()
            {
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.select(i);
        }
        if Self::samples(self.t) {
            let b = Rect::from_min_size(
                Pos2::new(strip.left() + 6.0, strip.bottom() + 6.0),
                Vec2::new(150.0, 34.0),
            );
            if ui
                .put(
                    b,
                    egui::Button::new(RichText::new("Redraw: new flights").size(12.5)),
                )
                .clicked()
            {
                self.redraw();
            }
        }
        pic.text(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 40.0),
            egui::Align2::LEFT_BOTTOM,
            self.built_line(),
            egui::FontId::proportional(11.5),
            Color32::from_rgb(190, 196, 210),
        );
        draw_chart(painter, chart, self.t);
        out
    }
}

/// The recorded bias of every treatment against delta tracking, one row per
/// treatment and one dot (±1σ) per record that has it, with the speed-ups
/// beside. GUI drawing: exempt from the reaching-test rule; the numbers come
/// from [`bias`], which is tested.
pub fn draw_chart(painter: &egui::Painter, rect: Rect, selected: usize) {
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(20, 23, 29));
    let text = Color32::from_rgb(210, 216, 226);
    let font = |s: f32| egui::FontId::proportional(s);
    p.text(
        rect.left_top() + Vec2::new(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        "Recorded k bias vs delta tracking, pcm ± 1σ",
        font(13.0),
        text,
    );
    p.text(
        rect.left_top() + Vec2::new(8.0, 23.0),
        egui::Align2::LEFT_TOP,
        "every record: re-measurement pending (#582)",
        font(11.0),
        Color32::from_rgb(250, 200, 80),
    );
    // Legend, one colour per record.
    let mut x = rect.left() + 8.0;
    for (ri, r) in RECORDS.iter().enumerate() {
        let g = p.layout_no_wrap(r.date.to_string(), font(11.0), RECORD_COLOURS[ri]);
        if x + g.size().x + 14.0 > rect.right() {
            break;
        }
        p.circle_filled(
            Pos2::new(x + 4.0, rect.top() + 46.0),
            4.0,
            RECORD_COLOURS[ri],
        );
        p.galley(
            Pos2::new(x + 11.0, rect.top() + 39.0),
            g,
            RECORD_COLOURS[ri],
        );
        x += 90.0;
    }
    let label_w = (rect.width() * 0.27).clamp(70.0, 120.0);
    let speed_w = (rect.width() * 0.2).clamp(54.0, 110.0);
    let plot = Rect::from_min_max(
        Pos2::new(rect.left() + label_w, rect.top() + 62.0),
        Pos2::new(rect.right() - speed_w, rect.bottom() - 26.0),
    );
    let (lo, hi) = (-5500.0_f64, 2500.0_f64);
    let px = |pcm: f64| plot.left() + ((pcm.clamp(lo, hi) - lo) / (hi - lo)) as f32 * plot.width();
    for tick in [
        -5000.0, -4000.0, -3000.0, -2000.0, -1000.0, 0.0, 1000.0, 2000.0,
    ] {
        let xt = px(tick);
        let c = if tick == 0.0 {
            Color32::from_rgb(150, 156, 170)
        } else {
            Color32::from_rgb(44, 48, 58)
        };
        p.line_segment(
            [Pos2::new(xt, plot.top()), Pos2::new(xt, plot.bottom())],
            Stroke::new(1.0, c),
        );
        if (tick as i64) % 2000 == 0 {
            p.text(
                Pos2::new(xt, plot.bottom() + 3.0),
                egui::Align2::CENTER_TOP,
                format!("{tick:+.0}"),
                font(10.0),
                Color32::from_rgb(150, 156, 170),
            );
        }
    }
    let row_h = plot.height() / TREATMENTS.len() as f32;
    for t in 0..TREATMENTS.len() {
        let yc = plot.top() + (t as f32 + 0.5) * row_h;
        if t == selected {
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left() + 2.0, yc - row_h * 0.5),
                    Pos2::new(rect.right() - 2.0, yc + row_h * 0.5),
                ),
                3.0,
                Color32::from_rgb(38, 44, 58),
            );
        }
        p.text(
            Pos2::new(rect.left() + 6.0, yc),
            egui::Align2::LEFT_CENTER,
            SHORT[t],
            font(11.5),
            text,
        );
        let mut speeds = Vec::new();
        for (ri, r) in RECORDS.iter().enumerate() {
            let dy = (ri as f32 - 1.5) * (row_h * 0.17).min(5.0);
            if t == 0 {
                if r.arms.iter().any(|a| a.t == 0) {
                    p.circle_stroke(
                        Pos2::new(px(0.0), yc + dy),
                        3.0,
                        Stroke::new(1.2, RECORD_COLOURS[ri]),
                    );
                }
                continue;
            }
            if let Some((d, s)) = bias(r, t) {
                let y = yc + dy;
                p.line_segment(
                    [Pos2::new(px(d - s), y), Pos2::new(px(d + s), y)],
                    Stroke::new(1.6, RECORD_COLOURS[ri]),
                );
                p.circle_filled(Pos2::new(px(d), y), 3.4, RECORD_COLOURS[ri]);
            }
            if let Some(sp) = r.arms.iter().find(|a| a.t == t).and_then(|a| a.speed) {
                speeds.push((ri, sp));
            }
        }
        // The newest recorded speed-up of this arm, coloured by its record.
        if let Some(&(ri, sp)) = speeds.last() {
            let s = if sp >= 0.1 {
                format!("{sp:.2}×")
            } else {
                format!("{:.0}× slower", 1.0 / sp)
            };
            p.text(
                Pos2::new(rect.right() - 6.0, yc),
                egui::Align2::RIGHT_CENTER,
                s,
                font(11.5),
                RECORD_COLOURS[ri],
            );
        }
    }
    p.text(
        Pos2::new(plot.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        "pcm vs delta; right: speed vs delta (newest record)",
        font(10.5),
        Color32::from_rgb(150, 156, 170),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Selecting a treatment swaps the palette and asks for a new slice (the
    /// raster parameter changes); a redraw of a sampled arm does too; a
    /// describe is asked once and recorded when it comes back.
    #[test]
    fn selecting_and_redrawing_change_what_is_asked() {
        let mut v = ShortcutsView::new();
        assert_eq!(v.pump(), vec![Outgoing::Walk(vec![MSG_DESCRIBE, 0.0])]);
        assert!(v.pump().is_empty());
        v.receive(&[MSG_DESCRIBE, 0.0, 1.5, 26_000.0, 0.3, 8.0])
            .unwrap();
        assert!(v.built_line().contains("26000 particles"));
        v.select(3);
        assert_eq!(v.slicer.param, 3.0);
        assert_eq!(v.slicer.info.palette.len(), 9);
        assert_eq!(v.pump(), vec![Outgoing::Walk(vec![MSG_DESCRIBE, 3.0])]);
        v.select(1);
        let before = v.slicer.param;
        v.redraw();
        assert!(v.slicer.param != before && v.slicer.param.floor() == 1.0);
        assert!(ShortcutsView::samples(1) && !ShortcutsView::samples(4));
        assert!(v.receive(&[MSG_DESCRIBE, 1.0]).is_err());
        assert!(v.status().contains("CLS"));
    }
}
