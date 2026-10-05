//! **The σ(E) panel** (gh:#549, lesson philosophy §7 "link the views"): a
//! log–log graph of a few microscopic cross sections beside the geometry,
//! with a marker riding the current neutron's energy, so a neutron landing
//! in a U-238 resonance is seen to be captured there.
//!
//! The curves are the cross sections the worker already processed for
//! transport (RECONR + BROADR at the rung's tier and temperature, S(α,β)
//! below its cutoff for a bound moderator), read through
//! `Nuclide::xs_at_energy`, the call the collision physics makes. They are
//! computed ONCE per load in the worker and sent decimated: on every
//! reconstructed breakpoint (`Nuclide::native_energy_grid`) plus a log
//! backbone (the S(α,β) law has structure, Bragg edges, between the
//! breakpoints), then kept as the minimum and maximum of each of [`BINS`]
//! logarithmic bins, so resonance peaks and troughs survive at a few
//! thousand points per curve.

// The wire format serves the browser build (and the tests).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use outram_mc_libs::material::nuclide::Nuclide;

/// Logarithmic bins over [`E_MIN`, `E_MAX`] eV; each keeps its min and max.
pub const BINS: usize = 900;
pub const E_MIN: f64 = 1.0e-5;
pub const E_MAX: f64 = 2.0e7;
/// Log-backbone points per decade, added to the native grid.
const BACKBONE_PER_DECADE: usize = 400;

/// Which reaction a curve shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// σ_a − σ_f: radiative capture plus charged-particle absorption (for
    /// U-238 below a few MeV this is (n,γ)).
    Capture,
    Fission,
    /// Elastic scattering, or the bound S(α,β) scattering below its cutoff.
    Scatter,
}

impl Channel {
    pub fn code(self) -> f64 {
        match self {
            Channel::Capture => 0.0,
            Channel::Fission => 1.0,
            Channel::Scatter => 2.0,
        }
    }
    pub fn from_code(c: f64) -> Channel {
        match c as i64 {
            0 => Channel::Capture,
            1 => Channel::Fission,
            _ => Channel::Scatter,
        }
    }
}

/// One decimated curve, `(E eV, σ barn)` ascending in E.
#[derive(Clone, Debug, PartialEq)]
pub struct XsCurve {
    pub label: String,
    pub channel: Channel,
    pub points: Vec<[f32; 2]>,
}

/// What a rung shows: `(label, nuclide index, channel)`.
pub type Pick = (&'static str, usize, Channel);

fn value(n: &Nuclide, e: f64, temp_k: f64, c: Channel) -> f64 {
    let x = n.xs_at_energy(e, temp_k);
    match c {
        Channel::Capture => x.absorption - x.fission,
        Channel::Fission => x.fission,
        Channel::Scatter => x.elastic,
    }
}

/// Build the curves for `picks` from the loaded nuclides (worker side).
pub fn curves(nuclides: &[Nuclide], temp_k: f64, picks: &[Pick]) -> Vec<XsCurve> {
    let (l0, l1) = (E_MIN.log10(), E_MAX.log10());
    let n_back = ((l1 - l0) * BACKBONE_PER_DECADE as f64) as usize;
    picks
        .iter()
        .filter_map(|&(label, i, channel)| {
            let n = nuclides.get(i)?;
            let mut grid = n.native_energy_grid(E_MIN, E_MAX);
            grid.extend((0..=n_back).map(|k| 10f64.powf(l0 + (l1 - l0) * k as f64 / n_back as f64)));
            grid.sort_by(|a, b| a.total_cmp(b));
            grid.dedup();
            // Per log bin: (min point, max point), kept in energy order.
            let mut lo: Vec<Option<(f64, f64)>> = vec![None; BINS];
            let mut hi: Vec<Option<(f64, f64)>> = vec![None; BINS];
            for &e in &grid {
                let s = value(n, e, temp_k, channel);
                if !(s.is_finite() && s > 0.0) {
                    continue;
                }
                let b = (((e.log10() - l0) / (l1 - l0)) * BINS as f64) as usize;
                let b = b.min(BINS - 1);
                if lo[b].is_none_or(|(_, v)| s < v) {
                    lo[b] = Some((e, s));
                }
                if hi[b].is_none_or(|(_, v)| s > v) {
                    hi[b] = Some((e, s));
                }
            }
            let mut points = Vec::with_capacity(2 * BINS);
            for b in 0..BINS {
                let mut two: Vec<(f64, f64)> = lo[b].into_iter().chain(hi[b]).collect();
                two.sort_by(|a, b| a.0.total_cmp(&b.0));
                two.dedup();
                points.extend(two.into_iter().map(|(e, s)| [e as f32, s as f32]));
            }
            Some(XsCurve { label: label.to_string(), channel, points })
        })
        .collect()
}

/// σ at `e` on a decimated curve, log–log interpolated (for the marker).
pub fn at(c: &XsCurve, e: f64) -> Option<f64> {
    let p = &c.points;
    let i = p.partition_point(|q| (q[0] as f64) < e);
    if i == 0 || i >= p.len() {
        return None;
    }
    let (a, b) = (p[i - 1], p[i]);
    let (x0, x1, y0, y1) = ((a[0] as f64).ln(), (b[0] as f64).ln(), (a[1] as f64).ln(), (b[1] as f64).ln());
    let t = if x1 > x0 { (e.ln() - x0) / (x1 - x0) } else { 0.0 };
    Some((y0 + t * (y1 - y0)).exp())
}

/// Flatten for the worker boundary: `[n_curves, (channel, n_points, e, s, e, s, …)…]`;
/// the labels travel separately, joined by newlines.
pub fn encode(cs: &[XsCurve]) -> (Vec<f64>, String) {
    let mut v = vec![cs.len() as f64];
    for c in cs {
        v.push(c.channel.code());
        v.push(c.points.len() as f64);
        v.extend(c.points.iter().flat_map(|p| [p[0] as f64, p[1] as f64]));
    }
    (v, cs.iter().map(|c| c.label.as_str()).collect::<Vec<_>>().join("\n"))
}

pub fn decode(v: &[f64], labels: &str) -> Result<Vec<XsCurve>, String> {
    let labels: Vec<&str> = if labels.is_empty() { Vec::new() } else { labels.split('\n').collect() };
    let n = *v.first().ok_or("empty cross-section message")? as usize;
    let mut out = Vec::with_capacity(n);
    let mut i = 1;
    for k in 0..n {
        let (ch, m) = (*v.get(i).ok_or("short")?, *v.get(i + 1).ok_or("short")? as usize);
        let pts = v.get(i + 2..i + 2 + 2 * m).ok_or("cross-section message too short")?;
        out.push(XsCurve {
            label: labels.get(k).unwrap_or(&"?").to_string(),
            channel: Channel::from_code(ch),
            points: pts.chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]).collect(),
        });
        i += 2 + 2 * m;
    }
    if i != v.len() {
        return Err("cross-section message has trailing values".into());
    }
    Ok(out)
}

// ─── Drawing (UI side) ───────────────────────────────────────────────────────

use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub fn colour(c: Channel) -> Color32 {
    match c {
        Channel::Capture => Color32::from_rgb(255, 120, 90),
        Channel::Fission => Color32::from_rgb(255, 205, 80),
        Channel::Scatter => Color32::from_rgb(110, 200, 255),
    }
}

/// The neutron the marker rides: its energy now, and how it ended if it has.
pub struct Marker {
    pub energy_ev: f64,
    /// `Some("captured")` / `Some("fission")` once the history has ended
    /// in the material; the marker is then drawn as a cross, not a dot.
    pub ended: Option<&'static str>,
}

/// The panel: frame, decades, every shown curve, and the marker.
pub fn draw(painter: &egui::Painter, rect: Rect, curves: &[XsCurve], shown: &[bool], marker: Option<Marker>, text: f32) {
    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 21, 28));
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 66, 80)), StrokeKind::Inside);
    let f = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.85).max(8.0));
    let grey = Color32::from_rgb(170, 176, 190);
    painter.text(rect.left_top() + Vec2::new(8.0, 5.0), egui::Align2::LEFT_TOP, "σ(E), barns (log–log)", f.clone(), Color32::WHITE);
    let visible: Vec<&XsCurve> = curves.iter().zip(shown).filter(|(_, s)| **s).map(|(c, _)| c).collect();
    // y range from the shown curves, whole decades, at most 9 of them.
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in &visible {
        for p in &c.points {
            let l = (p[1] as f64).log10();
            y0 = y0.min(l);
            y1 = y1.max(l);
        }
    }
    if !y0.is_finite() {
        (y0, y1) = (-2.0, 4.0);
    }
    let (y0, y1) = (y0.floor().max(y1.ceil() - 9.0), y1.ceil().max(y0.floor() + 1.0));
    let legend_h = (visible.len().max(1) as f32) * (text * 1.3) + 6.0;
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(text * 3.0, text * 1.8 + 6.0), rect.right_bottom() - Vec2::new(10.0, text * 1.6 + legend_h));
    if plot.width() < 40.0 || plot.height() < 30.0 {
        return;
    }
    let (x0, x1) = (E_MIN.log10(), E_MAX.log10());
    let p = |e: f64, s: f64| {
        let fx = ((e.log10() - x0) / (x1 - x0)) as f32;
        let fy = ((s.log10() - y0) / (y1 - y0)) as f32;
        Pos2::new(plot.left() + fx * plot.width(), plot.bottom() - fy.clamp(-0.02, 1.02) * plot.height())
    };
    let grid = Color32::from_rgb(40, 45, 56);
    let step = if plot.width() < 260.0 { 4 } else { 2 };
    for d in (x0.ceil() as i32..=x1.floor() as i32).filter(|d| d.rem_euclid(step) == 0) {
        let a = p(10f64.powi(d), 10f64.powf(y0));
        painter.line_segment([a, Pos2::new(a.x, plot.top())], Stroke::new(1.0, grid));
        let lab = match d {
            0 => "1 eV".to_string(),
            3 => "1 keV".to_string(),
            6 => "1 MeV".to_string(),
            _ => format!("1e{d}"),
        };
        painter.text(Pos2::new(a.x, plot.bottom() + 2.0), egui::Align2::CENTER_TOP, lab, small.clone(), grey);
    }
    for d in y0 as i32..=y1 as i32 {
        let a = p(E_MIN, 10f64.powi(d));
        painter.line_segment([a, Pos2::new(plot.right(), a.y)], Stroke::new(1.0, grid));
        painter.text(Pos2::new(plot.left() - 3.0, a.y), egui::Align2::RIGHT_CENTER, format!("1e{d}"), small.clone(), grey);
    }
    let clip = painter.with_clip_rect(plot.expand(1.0));
    for c in &visible {
        let pts: Vec<Pos2> = c.points.iter().map(|q| p(q[0] as f64, q[1] as f64)).collect();
        if pts.len() > 1 {
            clip.add(egui::Shape::line(pts, Stroke::new(1.3, colour(c.channel))));
        }
    }
    // The neutron: a vertical line at its energy and a dot on each curve.
    if let Some(m) = marker {
        if m.energy_ev > E_MIN && m.energy_ev < E_MAX {
            let e = m.energy_ev;
            let top = p(e, 10f64.powf(y1));
            let col = crate::anim::energy_colour(e, 255);
            clip.line_segment([Pos2::new(top.x, plot.top()), Pos2::new(top.x, plot.bottom())], Stroke::new(1.0, col.gamma_multiply(0.6)));
            for c in &visible {
                if let Some(s) = at(c, e) {
                    let q = p(e, s);
                    match m.ended {
                        None => {
                            clip.circle_filled(q, 5.0, col);
                            clip.circle_stroke(q, 5.0, Stroke::new(1.5, colour(c.channel)));
                        }
                        Some(_) => {
                            let d = 5.0;
                            let st = Stroke::new(2.0, Color32::from_rgb(255, 90, 90));
                            clip.line_segment([q - Vec2::splat(d), q + Vec2::splat(d)], st);
                            clip.line_segment([q + Vec2::new(-d, d), q + Vec2::new(d, -d)], st);
                        }
                    }
                }
            }
            let what = match m.ended {
                None => format!("neutron at {}", crate::anim::fmt_energy(e)),
                Some(w) => format!("{w} at {}", crate::anim::fmt_energy(e)),
            };
            painter.text(Pos2::new(plot.right(), rect.top() + 5.0), egui::Align2::RIGHT_TOP, what, small.clone(), col);
        }
    }
    painter.text(Pos2::new(plot.right(), plot.bottom() + text * 1.2 + 2.0), egui::Align2::RIGHT_TOP, "E (eV)", small.clone(), grey);
    // Legend, under the axis.
    let mut y = plot.bottom() + text * 1.6 + 6.0;
    for c in &visible {
        let a = Pos2::new(rect.left() + 10.0, y + text * 0.55);
        painter.line_segment([a, a + Vec2::new(18.0, 0.0)], Stroke::new(2.0, colour(c.channel)));
        painter.text(a + Vec2::new(24.0, 0.0), egui::Align2::LEFT_CENTER, &c.label, small.clone(), Color32::from_rgb(210, 216, 226));
        y += text * 1.3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_cross_the_worker_boundary_and_interpolate() {
        let c = XsCurve { label: "U-238 capture".into(), channel: Channel::Capture, points: vec![[1.0, 10.0], [10.0, 1000.0], [100.0, 10.0]] };
        let d = XsCurve { label: "C".into(), channel: Channel::Scatter, points: vec![[1.0e-3, 4.7], [1.0e6, 2.5]] };
        let (v, l) = encode(&[c.clone(), d.clone()]);
        assert_eq!(decode(&v, &l).unwrap(), vec![c.clone(), d]);
        // log-log: halfway (in ln E) between 1 and 10 eV is sqrt(10) eV, sigma sqrt(10 * 1000) = 100.
        assert!((at(&c, 10f64.sqrt()).unwrap() - 100.0).abs() < 1e-3);
        assert!(at(&c, 0.5).is_none() && at(&c, 200.0).is_none());
    }
}
