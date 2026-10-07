//! The page side of the `packing` demo: the placements the worker replayed,
//! animated in order on the plane z = 0, with the packing fractions and the
//! trials per placement as placement goes. Only interpolation and drawing
//! happen here; the packing is done in the worker (the no-lag rule).

use super::{params, MSG_PLAN, MSG_REPLAY, PER_PLACEMENT};
use crate::walkdemo::{split, Outgoing};
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};

/// What happens to the particles the fuel-zone surface cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutMode {
    /// The cube as RSA packed it: whole, cut and outside particles all drawn.
    Show,
    /// Keep only whole particles and say nothing: the zone holds less fuel
    /// than asked (the defect this code had until 2026-09-14).
    Drop,
    /// `pack_in_ball`'s fix: re-pack at a rescaled request until the whole
    /// particles hit the requested fraction.
    Fix,
}

/// One attempt of `pack_in_ball`, as the worker reported it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attempt {
    pub request: f64,
    pub generated: usize,
    pub kept: usize,
    pub realised: f64,
}

/// One placement, as replayed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub trials: u32,
    pub whole: bool,
}

pub struct PackingView {
    pub view: View,
    pub plan: Option<(Vec<Attempt>, usize)>,
    pub mode: CutMode,
    /// Which attempt is loaded, and its placements in order.
    pub attempt: Option<usize>,
    pub placements: Vec<Placement>,
    /// Whole particles among the first `i` placements, for every `i`.
    kept_prefix: Vec<u32>,
    /// Indices of the placements that cross the plane z = 0.
    in_slice: Vec<u32>,
    /// Placements shown so far (fractional while animating).
    pub shown: f64,
    /// Placements per second.
    pub rate: f64,
    pub playing: bool,
    last_t: Option<f64>,
    asked_plan: bool,
    asked_replay: Option<usize>,
    pub replay_secs: f64,
    pub plan_secs: f64,
}

impl PackingView {
    pub fn new() -> Self {
        let (r_p, big_r, _, _) = params();
        Self {
            view: View::new(big_r + r_p + 0.05),
            plan: None,
            mode: CutMode::Show,
            attempt: None,
            placements: Vec::new(),
            kept_prefix: vec![0],
            in_slice: Vec::new(),
            shown: 0.0,
            rate: 1500.0,
            playing: true,
            last_t: None,
            asked_plan: false,
            asked_replay: None,
            replay_secs: 0.0,
            plan_secs: 0.0,
        }
    }

    /// The attempt the mode shows: the first (the bare request) for Show and
    /// Drop, the kept one for Fix.
    pub fn wanted_attempt(&self) -> Option<usize> {
        let (_, best) = self.plan.as_ref()?;
        Some(if self.mode == CutMode::Fix { *best } else { 0 })
    }

    pub fn set_mode(&mut self, m: CutMode) {
        self.mode = m;
    }

    /// The plan first, then the replay of the attempt the mode wants.
    pub fn pump(&mut self) -> Vec<Outgoing> {
        if self.plan.is_none() {
            if !self.asked_plan {
                self.asked_plan = true;
                return vec![Outgoing::Walk(vec![MSG_PLAN])];
            }
            return Vec::new();
        }
        match self.wanted_attempt() {
            Some(w) if self.attempt != Some(w) && self.asked_replay != Some(w) => {
                self.asked_replay = Some(w);
                vec![Outgoing::Walk(vec![MSG_REPLAY, w as f64])]
            }
            _ => Vec::new(),
        }
    }

    pub fn receive(&mut self, msg: &[f64]) -> Result<(), String> {
        match msg.first() {
            Some(&c) if c == MSG_PLAN => {
                let n = *msg.get(3).ok_or("plan: too short")? as usize;
                if msg.len() != 4 + 4 * n {
                    return Err(format!("plan: {} values for {n} attempts", msg.len()));
                }
                let attempts = msg[4..]
                    .chunks_exact(4)
                    .map(|a| Attempt {
                        request: a[0],
                        generated: a[1] as usize,
                        kept: a[2] as usize,
                        realised: a[3],
                    })
                    .collect();
                self.plan_secs = msg[1];
                self.plan = Some((attempts, msg[2] as usize));
                Ok(())
            }
            Some(&c) if c == MSG_REPLAY => {
                if msg.len() < 3 || (msg.len() - 3) % PER_PLACEMENT != 0 {
                    return Err(format!("replay: {} values", msg.len()));
                }
                let i = msg[1] as usize;
                if self.asked_replay == Some(i) {
                    self.asked_replay = None;
                }
                let (r_p, _, _, _) = params();
                self.placements = msg[3..]
                    .chunks_exact(PER_PLACEMENT)
                    .map(|c| Placement {
                        x: c[0] as f32,
                        y: c[1] as f32,
                        z: c[2] as f32,
                        trials: c[3] as u32,
                        whole: c[4] != 0.0,
                    })
                    .collect();
                self.kept_prefix = std::iter::once(0)
                    .chain(self.placements.iter().scan(0u32, |k, p| {
                        *k += p.whole as u32;
                        Some(*k)
                    }))
                    .collect();
                self.in_slice = (0..self.placements.len() as u32)
                    .filter(|&i| (self.placements[i as usize].z.abs() as f64) < r_p)
                    .collect();
                self.attempt = Some(i);
                self.replay_secs = msg[2];
                self.restart();
                Ok(())
            }
            _ => Err("packing: unexpected answer".into()),
        }
    }

    pub fn restart(&mut self) {
        self.shown = 0.0;
        self.playing = true;
        self.last_t = None;
    }

    /// Advance the animation by `dt` seconds.
    pub fn advance(&mut self, dt: f64) {
        if self.playing {
            self.shown = (self.shown + self.rate * dt).min(self.placements.len() as f64);
            if self.shown >= self.placements.len() as f64 {
                self.playing = false;
            }
        }
    }

    /// The numbers at the current point of the placement: placed, whole kept,
    /// the cube fraction, the whole-in-ball fraction, and the trials of the
    /// newest placement.
    pub fn counts(&self) -> Counts {
        let (r_p, big_r, _, _) = params();
        let n = (self.shown.floor() as usize).min(self.placements.len());
        let half = big_r + r_p;
        let v = 4.0 / 3.0 * std::f64::consts::PI * r_p.powi(3);
        Counts {
            placed: n,
            kept: self.kept_prefix.get(n).copied().unwrap_or(0) as usize,
            cube_fraction: n as f64 * v / (2.0 * half).powi(3),
            ball_fraction: self.kept_prefix.get(n).copied().unwrap_or(0) as f64
                * (r_p / big_r).powi(3),
            last_trials: n
                .checked_sub(1)
                .and_then(|i| self.placements.get(i))
                .map_or(0, |p| p.trials),
        }
    }

    pub fn status(&self) -> String {
        let c = self.counts();
        format!(
            "Random packing · {} · placed {} · whole in ball {} · pf {:.4}",
            self.mode_label(),
            c.placed,
            c.kept,
            c.ball_fraction
        )
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            CutMode::Show => "as packed",
            CutMode::Drop => "cut dropped, unsaid",
            CutMode::Fix => "pack_in_ball's fix",
        }
    }

    /// The panel. GUI drawing: exempt from the reaching-test rule; its state
    /// changes go through tested methods (`set_mode`, `restart`, `advance`).
    pub fn panel(&mut self, ui: &mut egui::Ui) -> Vec<Outgoing> {
        ui.label("Random sequential addition on the FHR pebble: draw a centre at random in the cube, reject it if it overlaps a placed particle, repeat. This is pack_spheres itself, running in the worker.");
        self.mode_buttons(ui);
        self.play_controls(ui);
        ui.add(
            egui::Slider::new(&mut self.rate, 10.0..=30_000.0)
                .logarithmic(true)
                .text("placements / s"),
        );
        if let Some((attempts, best)) = &self.plan {
            ui.separator();
            ui.strong("pack_in_ball's attempts (this seed)");
            for (i, a) in attempts.iter().enumerate() {
                let tag = if i == *best { " ← kept" } else { "" };
                ui.label(format!(
                    "{}: asked {:.4} of the cube → {} placed, {} whole in the ball → {:.4}{tag}",
                    i + 1,
                    a.request,
                    a.generated,
                    a.kept,
                    a.realised
                ));
            }
            ui.weak(format!(
                "pack_in_ball took {:.2} s in the worker; this replay {:.2} s.",
                self.plan_secs, self.replay_secs
            ));
        }
        ui.separator();
        ui.colored_label(
            Color32::from_rgb(230, 126, 40),
            "● whole: |centre| + r ≤ 1.9 cm, kept",
        );
        ui.colored_label(
            Color32::from_rgb(235, 70, 70),
            "● cut by the fuel-zone surface",
        );
        ui.colored_label(
            Color32::from_rgb(110, 116, 128),
            "● outside the fuel zone (the cube's corners)",
        );
        Vec::new()
    }

    /// GUI drawing: exempt from the reaching-test rule.
    fn mode_buttons(&mut self, ui: &mut egui::Ui) {
        let mut m = self.mode;
        ui.horizontal_wrapped(|ui| {
            ui.label("Cut particles:");
            ui.selectable_value(&mut m, CutMode::Show, "show");
            ui.selectable_value(&mut m, CutMode::Drop, "drop, say nothing");
            ui.selectable_value(&mut m, CutMode::Fix, "pack_in_ball's fix");
        });
        if m != self.mode {
            self.set_mode(m);
        }
    }

    /// GUI drawing: exempt from the reaching-test rule.
    fn play_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let label = if self.playing { "Pause" } else { "Play" };
            if ui
                .add_sized([72.0, 34.0], egui::Button::new(label))
                .clicked()
            {
                if !self.playing && self.shown >= self.placements.len() as f64 {
                    self.restart();
                } else {
                    self.playing = !self.playing;
                    self.last_t = None;
                }
            }
            if ui
                .add_sized([72.0, 34.0], egui::Button::new("Restart"))
                .clicked()
            {
                self.restart();
            }
            if ui
                .add_sized([72.0, 34.0], egui::Button::new("To end"))
                .clicked()
            {
                self.shown = self.placements.len() as f64;
                self.playing = false;
            }
        });
    }

    /// The main view. GUI drawing: exempt from the reaching-test rule.
    pub fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        full: Rect,
        painter: &egui::Painter,
        resp: &egui::Response,
    ) -> Vec<Outgoing> {
        let (rect, chart) = split(full);
        let p = painter.with_clip_rect(rect);
        let _ = resp; // the app's response covers the chart too; the picture gets its own
        let picture = ui.interact(
            rect,
            ui.id().with("walk-picture"),
            egui::Sense::click_and_drag(),
        );
        self.view.handle_input(ui, &picture);
        let t = now_s();
        if let Some(t0) = self.last_t {
            self.advance((t - t0).min(0.1));
        }
        self.last_t = Some(t);
        if self.playing {
            ui.ctx().request_repaint();
        }
        let (r_p, big_r, target, _) = params();
        let half = big_r + r_p;
        let s = self.view.scale as f32;
        let to = |x: f64, y: f64| self.view.to_screen(rect, x, y);
        // The cube RSA packs, and the fuel zone pack_in_ball keeps.
        p.rect_stroke(
            Rect::from_two_pos(to(-half, half), to(half, -half)),
            0.0,
            Stroke::new(1.0, Color32::from_rgb(90, 96, 110)),
            egui::StrokeKind::Inside,
        );
        p.circle_filled(
            to(0.0, 0.0),
            big_r as f32 * s,
            Color32::from_rgb(34, 38, 46),
        );
        p.circle_stroke(
            to(0.0, 0.0),
            big_r as f32 * s,
            Stroke::new(1.5, Color32::from_rgb(170, 176, 190)),
        );
        let n = (self.shown.floor() as usize).min(self.placements.len());
        let r2 = (r_p * r_p) as f32;
        for &i in &self.in_slice {
            let i = i as usize;
            if i >= n {
                break;
            }
            let q = self.placements[i];
            let cut = !q.whole
                && ((q.x * q.x + q.y * q.y + q.z * q.z).sqrt() - r_p as f32) < big_r as f32;
            let colour = match (q.whole, cut, self.mode) {
                (true, _, _) => Color32::from_rgb(230, 126, 40),
                (false, _, CutMode::Drop | CutMode::Fix) => continue,
                (false, true, CutMode::Show) => Color32::from_rgb(235, 70, 70),
                (false, false, CutMode::Show) => Color32::from_rgb(110, 116, 128),
            };
            let rc = (r2 - q.z * q.z).max(0.0).sqrt() * s;
            p.circle_filled(to(q.x as f64, q.y as f64), rc.max(0.8), colour);
        }
        // The newest few placements, wherever they are in z, as rings.
        for q in self.placements[n.saturating_sub(12)..n].iter() {
            p.circle_stroke(
                to(q.x as f64, q.y as f64),
                (r_p as f32 * s).max(2.5),
                Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
            );
        }
        let c = self.counts();
        let font = egui::FontId::proportional(12.5);
        let white = Color32::from_rgb(225, 230, 240);
        let mut y = rect.top() + 48.0;
        let mut line = |text: String, colour: Color32| {
            p.text(
                Pos2::new(rect.left() + 12.0, y),
                egui::Align2::LEFT_TOP,
                text,
                font.clone(),
                colour,
            );
            y += 17.0;
        };
        match &self.plan {
            None => line(
                "pack_in_ball is running in the worker…".into(),
                Color32::from_rgb(250, 200, 80),
            ),
            Some(_) if self.attempt != self.wanted_attempt() => line(
                "replaying the packer in the worker…".into(),
                Color32::from_rgb(250, 200, 80),
            ),
            Some((attempts, _)) => {
                let a = self.attempt.and_then(|i| attempts.get(i)).copied();
                let req = a.map_or(target, |a| a.request);
                line(
                    format!(
                        "placed {} of {} · asked {:.4} of the cube · now {:.4}",
                        c.placed,
                        self.placements.len(),
                        req,
                        c.cube_fraction
                    ),
                    white,
                );
                line(
                    format!(
                        "this placement took {} trial{}",
                        c.last_trials,
                        if c.last_trials == 1 { "" } else { "s" }
                    ),
                    white,
                );
                let ball = format!(
                    "whole in the 1.9 cm zone: {} → pf {:.4} (asked {target:.2})",
                    c.kept, c.ball_fraction
                );
                let colour = match (self.mode, c.placed == self.placements.len()) {
                    (CutMode::Drop, true) => Color32::from_rgb(255, 110, 110),
                    (CutMode::Fix, true) => Color32::from_rgb(130, 220, 140),
                    _ => white,
                };
                line(ball, colour);
                if c.placed == self.placements.len() {
                    let verdict = match self.mode {
                        CutMode::Show => {
                            "the zone holds cut pieces: part-particles that are not real particles"
                                .to_string()
                        }
                        CutMode::Drop => format!(
                            "{:+.1} % heavy metal against the smeared arms, unsaid",
                            100.0 * (c.ball_fraction / target - 1.0)
                        ),
                        CutMode::Fix => format!(
                            "{:+.2} % of the request: inside pack_in_ball's 0.2 %",
                            100.0 * (c.ball_fraction / target - 1.0)
                        ),
                    };
                    line(verdict, colour);
                }
            }
        }
        p.text(
            rect.left_bottom() + Vec2::new(12.0, -40.0),
            egui::Align2::LEFT_BOTTOM,
            "slice z = 0 of the real packing; counts over all particles",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(150, 156, 170),
        );
        // Mode switch, finger-sized, on the main view (the panel is folded on a phone).
        let labels = [
            (CutMode::Show, "cut: show"),
            (CutMode::Drop, "drop, unsaid"),
            (CutMode::Fix, "the fix"),
        ];
        let bw = ((rect.width() - 24.0 - 8.0) / 3.0).min(112.0);
        let row_y = rect.bottom() - 86.0;
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 4.0, row_y - 4.0),
                Vec2::new(3.0 * (bw + 4.0) + 12.0, 42.0),
            ),
            6.0,
            Color32::from_rgba_unmultiplied(14, 16, 20, 215),
        );
        for (k, (m, l)) in labels.iter().enumerate() {
            let b = Rect::from_min_size(
                Pos2::new(rect.left() + 10.0 + k as f32 * (bw + 4.0), row_y),
                Vec2::new(bw, 34.0),
            );
            if ui
                .put(
                    b,
                    egui::Button::selectable(self.mode == *m, RichText::new(*l).size(12.5)),
                )
                .clicked()
            {
                self.set_mode(*m);
            }
        }
        let pb = Rect::from_min_size(
            Pos2::new(rect.left() + 10.0, row_y - 42.0),
            Vec2::new(80.0, 34.0),
        );
        let label = if self.playing {
            "Pause"
        } else if c.placed >= self.placements.len() {
            "Replay"
        } else {
            "Play"
        };
        if ui
            .put(pb, egui::Button::new(RichText::new(label).size(12.5)))
            .clicked()
        {
            if label == "Replay" {
                self.restart();
            } else {
                self.playing = !self.playing;
                self.last_t = None;
            }
        }
        draw_chart(painter, chart, self, target);
        Vec::new()
    }
}

impl Default for PackingView {
    fn default() -> Self {
        Self::new()
    }
}

/// The numbers [`PackingView::counts`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Counts {
    pub placed: usize,
    pub kept: usize,
    pub cube_fraction: f64,
    pub ball_fraction: f64,
    pub last_trials: u32,
}

/// Packing fraction against placements (cube, and whole in the ball, with
/// the target), and the trials per placement (a running mean). GUI drawing:
/// exempt from the reaching-test rule; the numbers come from the tested
/// [`PackingView::counts`] and the replay.
fn draw_chart(painter: &egui::Painter, rect: Rect, v: &PackingView, target: f64) {
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(20, 23, 29));
    let total = v.placements.len().max(1);
    let n = (v.shown.floor() as usize).min(v.placements.len());
    let (r_p, big_r, _, _) = params();
    let vol = 4.0 / 3.0 * std::f64::consts::PI * r_p.powi(3);
    let cube = (2.0 * (big_r + r_p)).powi(3);
    let unit_ball = (r_p / big_r).powi(3);
    let grey = Color32::from_rgb(150, 156, 170);
    let font = |s: f32| egui::FontId::proportional(s);
    let (top, bottom) = rect.split_top_bottom_at_fraction(0.6);
    // Packing fractions.
    let a = Rect::from_min_max(
        top.min + Vec2::new(38.0, 24.0),
        top.max - Vec2::new(10.0, 18.0),
    );
    p.text(
        top.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        "packing fraction as placement goes",
        font(12.5),
        Color32::from_rgb(210, 216, 226),
    );
    let ymax = 0.40;
    let at = |i: usize, f: f64| {
        Pos2::new(
            a.left() + i as f32 / total as f32 * a.width(),
            a.bottom() - (f / ymax) as f32 * a.height(),
        )
    };
    for f in [0.0, 0.1, 0.2, 0.3, 0.38] {
        let yy = at(0, f).y;
        p.line_segment(
            [Pos2::new(a.left(), yy), Pos2::new(a.right(), yy)],
            Stroke::new(1.0, Color32::from_rgb(40, 44, 54)),
        );
        p.text(
            Pos2::new(a.left() - 4.0, yy),
            egui::Align2::RIGHT_CENTER,
            format!("{f:.2}"),
            font(10.0),
            grey,
        );
    }
    // Target (dashed) and RSA's limit.
    p.add(egui::Shape::dashed_line(
        &[at(0, target), at(total, target)],
        Stroke::new(1.2, Color32::from_rgb(130, 220, 140)),
        6.0,
        4.0,
    ));
    p.text(
        at(0, 0.38) + Vec2::new(4.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        "RSA jams near 0.38",
        font(10.0),
        grey,
    );
    let step = (n / 400).max(1);
    let cube_line: Vec<Pos2> = (0..=n)
        .step_by(step)
        .map(|i| at(i, i as f64 * vol / cube))
        .collect();
    let ball_line: Vec<Pos2> = (0..=n)
        .step_by(step)
        .map(|i| {
            at(
                i,
                v.kept_prefix.get(i).copied().unwrap_or(0) as f64 * unit_ball,
            )
        })
        .collect();
    p.add(egui::Shape::line(
        cube_line,
        Stroke::new(1.6, Color32::from_rgb(120, 170, 255)),
    ));
    p.add(egui::Shape::line(
        ball_line,
        Stroke::new(1.6, Color32::from_rgb(230, 126, 40)),
    ));
    p.text(
        a.right_top() + Vec2::new(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        "cube (all placed)",
        font(10.5),
        Color32::from_rgb(120, 170, 255),
    );
    p.text(
        a.right_top() + Vec2::new(-4.0, 16.0),
        egui::Align2::RIGHT_TOP,
        "whole in the ball",
        font(10.5),
        Color32::from_rgb(230, 126, 40),
    );
    p.text(
        a.right_top() + Vec2::new(-4.0, 30.0),
        egui::Align2::RIGHT_TOP,
        "asked",
        font(10.5),
        Color32::from_rgb(130, 220, 140),
    );
    // Trials per placement, running mean over a window.
    let b = Rect::from_min_max(
        bottom.min + Vec2::new(38.0, 22.0),
        bottom.max - Vec2::new(10.0, 18.0),
    );
    p.text(
        bottom.left_top() + Vec2::new(8.0, 2.0),
        egui::Align2::LEFT_TOP,
        "trials per placement (mean of 200)",
        font(12.5),
        Color32::from_rgb(210, 216, 226),
    );
    let w = 200usize;
    let mut pts = Vec::new();
    let mut tmax: f64 = 2.0;
    let mut sum = 0.0;
    let means: Vec<(usize, f64)> = (0..n)
        .filter_map(|i| {
            sum += v.placements[i].trials as f64;
            if i >= w {
                sum -= v.placements[i - w].trials as f64;
            }
            (i % step.max(w / 4) == 0 && i >= w).then(|| (i, sum / w as f64))
        })
        .collect();
    for &(_, m) in &means {
        tmax = tmax.max(m);
    }
    for &(i, m) in &means {
        pts.push(Pos2::new(
            b.left() + i as f32 / total as f32 * b.width(),
            b.bottom() - (m / tmax) as f32 * b.height(),
        ));
    }
    p.line_segment(
        [b.left_bottom(), b.right_bottom()],
        Stroke::new(1.0, Color32::from_rgb(60, 64, 74)),
    );
    p.text(
        Pos2::new(b.left() - 4.0, b.top()),
        egui::Align2::RIGHT_CENTER,
        format!("{tmax:.0}"),
        font(10.0),
        grey,
    );
    p.text(
        Pos2::new(b.left() - 4.0, b.bottom()),
        egui::Align2::RIGHT_CENTER,
        "0",
        font(10.0),
        grey,
    );
    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.6, Color32::from_rgb(250, 200, 80)),
    ));
    p.text(
        Pos2::new(b.center().x, rect.bottom() - 3.0),
        egui::Align2::CENTER_BOTTOM,
        "placements, in order →",
        font(10.5),
        grey,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_msg() -> Vec<f64> {
        vec![
            MSG_PLAN, 0.5, 1.0, 2.0, 0.30, 4.0, 3.0, 0.289, 0.31, 4.0, 3.0, 0.300,
        ]
    }

    /// The page asks for the plan, then the replay its mode wants; the Fix mode
    /// asks for the kept attempt; counts and fractions follow the animation.
    #[test]
    fn the_view_asks_for_what_its_mode_shows_and_counts_as_it_goes() {
        let mut v = PackingView::new();
        assert_eq!(v.pump(), vec![Outgoing::Walk(vec![MSG_PLAN])]);
        assert!(v.pump().is_empty());
        v.receive(&plan_msg()).unwrap();
        assert_eq!(v.pump(), vec![Outgoing::Walk(vec![MSG_REPLAY, 0.0])]);
        assert!(v.pump().is_empty());
        // Four placements: whole, cut (outside the ball in x), whole, outside.
        let mut r = vec![MSG_REPLAY, 0.0, 0.1];
        r.extend([
            0.0, 0.0, 0.0, 1.0, 1.0, 1.9, 0.0, 0.0, 3.0, 0.0, 0.5, 0.5, 0.0, 2.0, 1.0, 1.9, 1.9,
            0.0, 7.0, 0.0,
        ]);
        v.receive(&r).unwrap();
        assert_eq!(v.placements.len(), 4);
        assert!(v.pump().is_empty());
        v.advance(2.5 / v.rate);
        let c = v.counts();
        assert_eq!((c.placed, c.kept, c.last_trials), (2, 1, 3));
        v.advance(10.0);
        let c = v.counts();
        assert_eq!((c.placed, c.kept, c.last_trials), (4, 2, 7));
        assert!(!v.playing);
        let (r_p, big_r, _, _) = params();
        assert!((c.ball_fraction - 2.0 * (r_p / big_r).powi(3)).abs() < 1e-15);
        v.set_mode(CutMode::Fix);
        assert_eq!(v.wanted_attempt(), Some(1));
        assert_eq!(v.pump(), vec![Outgoing::Walk(vec![MSG_REPLAY, 1.0])]);
        assert!(v.status().contains("fix"));
        assert!(v.receive(&[MSG_PLAN, 0.0]).is_err());
        assert!(v.receive(&[MSG_REPLAY, 0.0, 0.0, 1.0]).is_err());
        v.restart();
        assert_eq!(v.counts().placed, 0);
    }
}
