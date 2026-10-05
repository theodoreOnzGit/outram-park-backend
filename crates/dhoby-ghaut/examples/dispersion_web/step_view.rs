//! Drawing the step animations ([`crate::steps`], gh:#548), and the one piece
//! of state they keep on the UI thread: the tracer particles.
//!
//! The tracers are advanced here, not in the worker, because each frame's
//! work is a few hundred additions and square roots (the no-lag rule allows
//! interpolating precomputed data on the UI thread). Their spread comes from
//! buangkok's `sigma(x)` tables the worker computed: each step's increment
//! has variance `sigma^2(x2) - sigma^2(x1)`, so a crowd at distance `x` has
//! the plume's spread. The `z` walk reflects at the ground (the image
//! source). Illustration only: tracers carry no concentration.

use crate::steps::{Step, StepFrame, REACH_M, RISE_REACH_M};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Particles alive at once, about.
const POPULATION: f64 = 500.0;

/// Tracer particles: `[x, y, z_offset]` in metres (`z_offset` is the height
/// above the centreline for `rise`, the height above ground for `tracers`).
#[derive(Default)]
pub struct Tracers {
    pts: Vec<[f64; 3]>,
    seed: u64,
    owed: f64,
}

fn lerp_table(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let i = xs.partition_point(|v| *v < x).clamp(1, xs.len() - 1);
    let (x0, x1, y0, y1) = (xs[i - 1], xs[i], ys[i - 1], ys[i]);
    if x1 <= x0 {
        return y0;
    }
    y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
}

impl Tracers {
    pub fn clear(&mut self) {
        self.pts.clear();
        self.owed = 0.0;
    }

    /// A standard normal draw (xorshift and Box-Muller: demo bookkeeping).
    fn normal(&mut self) -> f64 {
        let mut next = || {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            ((self.seed >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let (a, b) = (next(), next());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }

    /// Advance by `dt` simulated seconds on `f`'s tables (`tracers` or
    /// `rise`), emitting at the source so about [`POPULATION`] are alive.
    pub fn advance(&mut self, f: &StepFrame, dt: f64) {
        if self.seed == 0 {
            self.seed = 0x9E37_79B9_7F4A_7C15;
        }
        let (u, h) = (f.scalars[0].max(0.1), f.scalars[1]);
        let (sy, sz) = (&f.curves[0], &f.curves[1]);
        let centre = f.curves.get(2);
        let c_at = |x: f64| centre.map_or(0.0, |c| lerp_table(&c.1, &c.2, x));
        let var = |t: &(String, Vec<f64>, Vec<f64>), x: f64| lerp_table(&t.1, &t.2, x).powi(2);
        let mut pts = std::mem::take(&mut self.pts);
        for p in &mut pts {
            let (x1, x2) = (p[0], p[0] + u * dt);
            let dy = (var(sy, x2) - var(sy, x1)).max(0.0).sqrt();
            let dz = (var(sz, x2) - var(sz, x1)).max(0.0).sqrt();
            p[0] = x2;
            p[1] += dy * self.normal();
            p[2] += dz * self.normal();
            // The ground reflects (the image source): height = centre + offset.
            let height = c_at(x2) + p[2];
            if height < 0.0 {
                p[2] = -height - c_at(x2);
            }
        }
        // The table's last distance is the view's reach.
        let reach = sy.1.last().copied().unwrap_or(REACH_M);
        pts.retain(|p| p[0] <= reach);
        self.owed += dt * POPULATION * u / reach;
        while self.owed >= 1.0 {
            self.owed -= 1.0;
            // Born at the source: on the centreline for `rise` (its offset
            // starts at zero), at the release height for `tracers`.
            pts.push([0.0, 0.0, if centre.is_some() { 0.0 } else { h }]);
        }
        self.pts = pts;
    }

    fn height(&self, f: &StepFrame, p: &[f64; 3]) -> f64 {
        match f.curves.get(2) {
            Some(c) => lerp_table(&c.1, &c.2, p[0]) + p[2],
            None => p[2],
        }
    }
}

/// A panel with axes: world `[x0, x1] x [y0, y1]` mapped into `rect`.
struct Axes {
    rect: Rect,
    w: [f64; 4],
}

impl Axes {
    fn to(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(
            self.rect.left() + ((x - self.w[0]) / (self.w[1] - self.w[0])) as f32 * self.rect.width(),
            self.rect.bottom() - ((y - self.w[2]) / (self.w[3] - self.w[2])) as f32 * self.rect.height(),
        )
    }
    fn frame(&self, painter: &egui::Painter, title: &str, xlabel: &str, ylabel: &str, font: f32) {
        let small = egui::FontId::proportional(font * 0.85);
        painter.rect_stroke(self.rect, 0.0, Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
        painter.text(self.rect.left_top() - Vec2::new(0.0, 4.0), egui::Align2::LEFT_BOTTOM, title, egui::FontId::proportional(font), Color32::WHITE);
        painter.text(self.rect.right_bottom() + Vec2::new(0.0, 3.0), egui::Align2::RIGHT_TOP, xlabel, small.clone(), Color32::GRAY);
        painter.text(self.rect.left_bottom() + Vec2::new(0.0, 3.0), egui::Align2::LEFT_TOP, format!("{:.0}", self.w[0]), small.clone(), Color32::GRAY);
        painter.text(self.rect.left_top() + Vec2::new(4.0, 2.0), egui::Align2::LEFT_TOP, format!("{ylabel} {:.0}", self.w[3]), small.clone(), Color32::GRAY);
        painter.text(self.rect.left_bottom() + Vec2::new(4.0, -2.0), egui::Align2::LEFT_BOTTOM, format!("{:.0}", self.w[2]), small, Color32::GRAY);
    }
}

/// Dark to yellow-white, for `t` in (0, 1] (the map's ramp).
fn ramp(t: f64) -> Color32 {
    let t = t.clamp(0.0, 1.0) as f32;
    Color32::from_rgba_unmultiplied(
        (40.0 + 215.0 * t.powf(0.8)) as u8,
        (20.0 + 220.0 * t.powf(1.6)) as u8,
        (90.0 + 80.0 * (1.0 - t) - 40.0 * t) as u8,
        (90.0 + 165.0 * t) as u8,
    )
}

/// A grid of `nx x ny` cells (row 0 at the top) over the axes' world box.
fn grid(painter: &egui::Painter, a: &Axes, nx: u32, ny: u32, colour: impl Fn(usize) -> Option<Color32>) {
    let (dx, dy) = ((a.w[1] - a.w[0]) / nx as f64, (a.w[3] - a.w[2]) / ny as f64);
    for row in 0..ny as usize {
        for col in 0..nx as usize {
            if let Some(c) = colour(row * nx as usize + col) {
                let x0 = a.w[0] + col as f64 * dx;
                let y1 = a.w[3] - row as f64 * dy;
                painter.rect_filled(Rect::from_two_pos(a.to(x0, y1), a.to(x0 + dx, y1 - dy)).expand(0.3), 0.0, c);
            }
        }
    }
}

/// Log colour over four decades below `max`.
fn log_colour(v: f64, max: f64) -> Option<Color32> {
    if !(v > 0.0 && max > 0.0) {
        return None;
    }
    let t = ((v / max).log10() + 4.0) / 4.0;
    (t > 0.0).then(|| ramp(t.min(1.0)))
}

const CLASSES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];

fn class_name(primary: f64, second: f64) -> String {
    let a = CLASSES[(primary as usize).min(5)];
    if second >= 0.0 {
        format!("{a} (or {}; changi's table gives two, the first is used)", CLASSES[(second as usize).min(5)])
    } else {
        a.to_string()
    }
}

/// Draw one step into `rect` (below the button row).
pub fn draw(painter: &egui::Painter, rect: Rect, step: Step, f: &StepFrame, tracers: &Tracers, font: f32) {
    let font = font.max(9.0);
    let pad = Vec2::new(16.0, font * 2.2);
    match step {
        Step::Tracers => {
            let h = (rect.height() - 3.0 * pad.y) / 2.0;
            let top = Axes {
                rect: Rect::from_min_size(rect.min + Vec2::new(pad.x, pad.y), Vec2::new(rect.width() - 2.0 * pad.x, h * 0.55)),
                w: [0.0, REACH_M, -REACH_M * 0.12, REACH_M * 0.12],
            };
            let zmax = (f.scalars[1] + 3.0 * f.curves[1].2.last().copied().unwrap_or(100.0)).max(60.0);
            let side = Axes {
                rect: Rect::from_min_size(Pos2::new(rect.left() + pad.x, top.rect.bottom() + 2.0 * pad.y), Vec2::new(rect.width() - 2.0 * pad.x, h * 1.45 - pad.y)),
                w: [0.0, REACH_M, 0.0, zmax],
            };
            top.frame(painter, "From above (wind blows to the right)", "x, m", "y, m", font);
            side.frame(painter, "From the side (ground at the bottom)", "x, m", "z, m", font);
            // +/- 2 sigma envelopes from buangkok's tables.
            let env = |a: &Axes, c: &(String, Vec<f64>, Vec<f64>), centre: f64, sign: f64| -> Vec<Pos2> {
                c.1.iter().zip(&c.2).map(|(x, s)| a.to(*x, (centre + sign * 2.0 * s).clamp(a.w[2], a.w[3]))).collect()
            };
            let stroke = Stroke::new(1.0, Color32::from_rgb(110, 170, 255));
            for sign in [-1.0, 1.0] {
                painter.extend(egui::Shape::dashed_line(&env(&top, &f.curves[0], 0.0, sign), stroke, 6.0, 4.0));
                painter.extend(egui::Shape::dashed_line(&env(&side, &f.curves[1], f.scalars[1], sign), stroke, 6.0, 4.0));
            }
            let clip_top = painter.with_clip_rect(top.rect);
            let clip_side = painter.with_clip_rect(side.rect);
            for p in &tracers.pts {
                clip_top.circle_filled(top.to(p[0], p[1]), 1.8, Color32::from_rgb(255, 230, 120));
                clip_side.circle_filled(side.to(p[0], tracers.height(f, p)), 1.8, Color32::from_rgb(255, 230, 120));
            }
            painter.text(
                Pos2::new(rect.left() + pad.x, rect.bottom() - 6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Dashed: ±2 sigma (buangkok). Wind at the release height {:.2} m/s. Dots: tracers, an illustration.", f.scalars[0]),
                egui::FontId::proportional(font * 0.85),
                Color32::LIGHT_GRAY,
            );
        }
        Step::Rise => {
            let zmax = f.curves[2].2.iter().copied().fold(0.0, f64::max) + 2.5 * f.curves[1].2.last().copied().unwrap_or(50.0);
            let a = Axes {
                rect: Rect::from_min_max(rect.min + Vec2::new(pad.x, pad.y), rect.max - Vec2::new(pad.x, 2.0 * pad.y)),
                w: [-RISE_REACH_M * 0.03, RISE_REACH_M, 0.0, zmax.max(f.scalars[2] * 1.3)],
            };
            a.frame(painter, "From the side: stack, rise, bend-over", "x, m", "z, m", font);
            let c = &f.curves[2];
            let clip = painter.with_clip_rect(a.rect);
            for sign in [-1.0, 1.0] {
                let env: Vec<Pos2> = c.1.iter().zip(&c.2).zip(&f.curves[1].2).map(|((x, h), s)| a.to(*x, (h + sign * 2.0 * s).max(0.0))).collect();
                clip.extend(egui::Shape::dashed_line(&env, Stroke::new(1.0, Color32::from_rgb(110, 170, 255)), 6.0, 4.0));
            }
            let line: Vec<Pos2> = c.1.iter().zip(&c.2).map(|(x, h)| a.to(*x, *h)).collect();
            clip.add(egui::Shape::line(line, Stroke::new(2.0, Color32::from_rgb(255, 170, 80))));
            for p in &tracers.pts {
                clip.circle_filled(a.to(p[0], tracers.height(f, p)), 1.8, Color32::from_rgb(255, 230, 120));
            }
            // The stack and H_e.
            let base = a.to(-RISE_REACH_M * 0.012, 0.0);
            let tip = a.to(RISE_REACH_M * 0.012, f.scalars[1]);
            clip.rect_filled(Rect::from_two_pos(base, tip), 0.0, Color32::from_gray(150));
            let y_he = a.to(0.0, f.scalars[2]).y;
            clip.extend(egui::Shape::dashed_line(&[Pos2::new(a.rect.left(), y_he), Pos2::new(a.rect.right(), y_he)], Stroke::new(1.0, Color32::WHITE), 4.0, 4.0));
            painter.text(Pos2::new(a.rect.right() - 4.0, y_he - 2.0), egui::Align2::RIGHT_BOTTOM, format!("H_e = {:.0} m (stack {:.0} m + rise)", f.scalars[2], f.scalars[1]), egui::FontId::proportional(font * 0.9), Color32::WHITE);
            if f.scalars[3] > 1.5 && f.curves[2].2.first() == f.curves[2].2.last() {
                let g = painter.layout(
                    format!("W0/U = {:.1} > 1.5: the neutral formula's downwash term -3(1.5 - W0/U) D_e turns positive and ADDS height, so the cap 3 D_i W0/U binds from the stack. Whether the reference applies that term above 1.5 U is the open question of #542. Lower W0 below {:.1} m/s to see the x^(1/3) rise.", f.scalars[3], 1.5 * f.scalars[0]),
                    egui::FontId::proportional(font * 0.85),
                    Color32::from_rgb(255, 190, 120),
                    a.rect.width() - 16.0,
                );
                painter.galley(a.rect.left_top() + Vec2::new(8.0, font * 1.6), g, Color32::WHITE);
            }
            painter.text(
                Pos2::new(rect.left() + pad.x, rect.bottom() - 6.0),
                egui::Align2::LEFT_BOTTOM,
                "Orange: centreline, stack + pyDOSEIA rise (unchecked, #542). Dashed: ±2 sigma_z.",
                egui::FontId::proportional(font * 0.85),
                Color32::LIGHT_GRAY,
            );
        }
        Step::Slice => {
            let a = Axes {
                rect: Rect::from_min_max(rect.min + Vec2::new(pad.x, pad.y * 1.5), Pos2::new(rect.right() - pad.x, rect.top() + pad.y * 1.5 + (rect.height() * 0.5).min(rect.width() * 0.75))),
                w: f.extent,
            };
            let scale = f.scalars[4];
            grid(painter, &a, f.nx, f.ny, |i| {
                let t = (f.values[i] / scale).min(1.0);
                (t > 1e-3).then(|| ramp(t.sqrt()))
            });
            a.frame(painter, &format!("Crosswind slice at x = {:.0} m (looking downwind)", f.scalars[0]), "y, m", "z, m", font);
            let lines = [
                format!("sigma_y = {:.1} m, sigma_z = {:.1} m (buangkok)", f.scalars[1], f.scalars[2]),
                format!("u × (integral of chi/Q over the slice) = {:.12}: the whole release, at every x", f.scalars[3]),
                "Colour: chi/Q against the slice's peak at 200 m, so it fades as the plume widens.".to_string(),
            ];
            let mut y = a.rect.bottom() + pad.y;
            for l in lines {
                let g = painter.layout(l, egui::FontId::proportional(font * 0.9), Color32::LIGHT_GRAY, rect.width() - 2.0 * pad.x);
                let gh = g.size().y;
                painter.galley(Pos2::new(rect.left() + pad.x, y), g, Color32::LIGHT_GRAY);
                y += gh + 4.0;
            }
        }
        Step::Mirror => {
            let a = Axes {
                rect: Rect::from_min_max(rect.min + Vec2::new(pad.x, pad.y), rect.max - Vec2::new(pad.x, 3.0 * pad.y)),
                w: f.extent,
            };
            let max = f.values.iter().copied().fold(0.0, f64::max);
            grid(painter, &a, f.nx, f.ny, |i| {
                if f.values[i] > 0.0 {
                    log_colour(f.values[i], max)
                } else {
                    log_colour(f.values2[i], max).map(|c| Color32::from_rgba_unmultiplied(c.b(), c.g(), c.r(), c.a() / 2))
                }
            });
            a.frame(painter, "From the side: the plume and the ground", "x, m", "z, m", font);
            let g = a.to(0.0, 0.0).y;
            painter.line_segment([Pos2::new(a.rect.left(), g), Pos2::new(a.rect.right(), g)], Stroke::new(2.0, Color32::from_rgb(140, 110, 70)));
            let s = a.to(0.0, f.scalars[0]);
            let m = a.to(0.0, -f.scalars[0]);
            painter.circle_filled(s, 4.0, Color32::from_rgb(255, 90, 90));
            painter.circle_stroke(m, 4.0, Stroke::new(1.5, Color32::from_rgb(120, 160, 255)));
            painter.text(s + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, "source, +H", egui::FontId::proportional(font * 0.85), Color32::WHITE);
            painter.text(m + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, "image, −H", egui::FontId::proportional(font * 0.85), Color32::WHITE);
            let g = painter.layout(
                "Above ground: buangkok's chi/Q, which adds the image source's plume. Below ground (blue): the part of the plume without the image that would have gone underground; the image puts exactly this back above ground, mirrored. Log colour, 4 decades.".to_string(),
                egui::FontId::proportional(font * 0.85),
                Color32::LIGHT_GRAY,
                rect.width() - 2.0 * pad.x,
            );
            painter.galley(Pos2::new(rect.left() + pad.x, a.rect.bottom() + pad.y * 0.6), g, Color32::LIGHT_GRAY);
        }
        Step::DayNight => {
            let max = f.values.iter().chain(&f.values2).copied().fold(0.0, f64::max);
            let wide = rect.width() > rect.height();
            let gap = pad.y * 1.6;
            let side = if wide {
                ((rect.width() - 3.0 * pad.x) / 2.0).min(rect.height() - 2.0 * gap)
            } else {
                ((rect.height() - 3.0 * gap) / 2.0).min(rect.width() - 2.0 * pad.x)
            };
            for (k, (values, label)) in [
                (&f.values, format!("14:00: class {}", class_name(f.scalars[0], f.scalars[1]))),
                (&f.values2, format!("02:00: class {}", class_name(f.scalars[2], f.scalars[3]))),
            ]
            .into_iter()
            .enumerate()
            {
                let origin = if wide {
                    rect.min + Vec2::new(pad.x + k as f32 * (side + pad.x), gap)
                } else {
                    rect.min + Vec2::new(pad.x, gap + k as f32 * (side + gap))
                };
                let a = Axes { rect: Rect::from_min_size(origin, Vec2::splat(side)), w: f.extent };
                grid(painter, &a, f.nx, f.ny, |i| log_colour(values[i], max));
                a.frame(painter, &label, "m", "m", font);
                painter.circle_filled(a.to(0.0, 0.0), 3.0, Color32::from_rgb(255, 90, 90));
            }
            painter.text(
                Pos2::new(rect.left() + pad.x, rect.bottom() - 6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Same wind, same stack; one colour scale (log, 4 decades below {max:.2e} s/m3). Wind blows to the right."),
                egui::FontId::proportional(font * 0.85),
                Color32::LIGHT_GRAY,
            );
        }
        Step::SinglePuff => {}
    }
}
