//! Animating a recorded history: the speed law, the flight along the track,
//! and drawing it. Shared by every animated rung (moved out of the TRISO
//! demo's `app.rs`, 2026-10-04, gh:#521).

use crate::history::History;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use outram_mc_libs::physics::track_output::TrackEvent;

// ─── The speed law ───────────────────────────────────────────────────────────

/// Reference energy of the speed slider \[eV\]: the slider value is the
/// animated speed of a neutron of THIS energy. One constant for every rung,
/// so a slider value means the same on each (maintainer decision 2026-10-04;
/// each rung's default is in [`Spectrum::default_speed_at_1ev`]).
pub const SPEED_REFERENCE_EV: f64 = 1.0;

/// Neutron rest energy `m_n c^2` \[eV\] (CODATA 2018: 939.56542052 MeV).
const NEUTRON_REST_ENERGY_EV: f64 = 939.565_420_52e6;
/// Speed of light \[cm/s\].
const C_CM_S: f64 = 2.997_924_58e10;

/// A neutron's real speed \[cm/s\] from its kinetic energy, classically:
/// `E = (1/2) m_n v^2`, so `v = sqrt(2E / m_n) = c sqrt(2E / (m_n c^2))`.
///
/// Non-relativistic on purpose: at 1 MeV the relativistic correction to `v`
/// is about 0.05 %, and at 20 MeV about 1 %, immaterial for an animation.
/// (1 eV gives 1.383e6 cm/s; 0.0253 eV gives the familiar 2200 m/s.)
pub fn neutron_speed_cm_s(e_ev: f64) -> f64 {
    C_CM_S * (2.0 * e_ev.max(0.0) / NEUTRON_REST_ENERGY_EV).sqrt()
}

/// **The animated speed of a neutron of energy `e_ev`** \[cm of flight per
/// second of animation\], when the slider says a [`SPEED_REFERENCE_EV`]
/// neutron moves at `slider_cm_s`:
///
/// `v_anim(E) = v_slider * v(E) / v(E_ref)` (`= v_slider * sqrt(E / E_ref)`),
///
/// with `v(E)` from [`neutron_speed_cm_s`]. So the picture slows down exactly
/// as the neutron does, in real proportion: a neutron that scatters from
/// 2 MeV to thermal slows by a factor of ~9000 on screen as it does in the
/// pebble. There is **no floor** (maintainer decision 2026-10-04): at the
/// thermal rungs' default of 7 cm/s at 1 eV a 0.0253 eV neutron moves at
/// 1.1 cm/s, and a 0.001 eV one at 0.22 cm/s; that crawl is the point.
///
/// `e_ev` is the energy of the flight segment being drawn: the energy the
/// recorded history carried after the event that started the segment.
pub fn animated_speed(e_ev: f64, slider_cm_s: f64) -> f64 {
    slider_cm_s * neutron_speed_cm_s(e_ev) / neutron_speed_cm_s(SPEED_REFERENCE_EV)
}

/// The energy range a rung's neutrons mostly live in. It sets the default of
/// the animation-speed slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spectrum {
    /// Moderated: neutrons slow down to thermal energies (TRISO; later
    /// uranium in graphite, LCT-008, HTR-10).
    Thermal,
    /// Unmoderated: neutrons stay near their ~MeV birth energies (Godiva;
    /// later Jemima).
    Fast,
}

impl Spectrum {
    /// Default of the "speed at 1 eV" slider \[cm of flight per second\].
    ///
    /// **One unit for every rung, a default per spectrum** (maintainer
    /// decision, 2026-10-04): the slider always means the animated speed of a
    /// 1 eV neutron, so rungs stay comparable, and each spectrum's default
    /// animates a TYPICAL neutron of that spectrum at about 7 cm/s:
    ///
    /// - thermal rungs: **7 cm/s at 1 eV** (a slowing-down neutron in a
    ///   moderator; a fully thermal 0.0253 eV neutron then crawls at 1.1 cm/s
    ///   and a 2 MeV birth flies at ~9900 cm/s);
    /// - fast rungs: **0.007 cm/s at 1 eV**, which is **7 cm/s at 1 MeV**,
    ///   about where a Godiva neutron spends its life.
    ///
    /// Selecting a rung resets the slider to its rung's default.
    pub fn default_speed_at_1ev(self) -> f64 {
        match self {
            Spectrum::Thermal => 7.0,
            Spectrum::Fast => 0.007,
        }
    }

    /// The characteristic energy shown beside the slider \[eV\], and its name.
    pub fn characteristic(self) -> (f64, &'static str) {
        match self {
            Spectrum::Thermal => (0.0253, "0.0253 eV (thermal)"),
            Spectrum::Fast => (1.0e6, "1 MeV (fast)"),
        }
    }
}

// ─── Flying along a recorded track ───────────────────────────────────────────

pub struct Anim {
    pub hist: History,
    /// Cumulative 3D flight distance at each state, cm.
    pub cum: Vec<f64>,
    pub shown_cm: f64,
    /// Index of the segment being flown (state `seg` to `seg + 1`).
    seg: usize,
    /// Already added to the statistics? A history is computed whole before
    /// it is animated; counting it then would show its fate while it is
    /// still in flight.
    pub counted: bool,
}

impl Anim {
    pub fn new(hist: History) -> Self {
        let st = &hist.track.states;
        let mut cum = Vec::with_capacity(st.len());
        let mut d = 0.0;
        for (i, s) in st.iter().enumerate() {
            if i > 0 {
                let q = st[i - 1].r;
                d += ((s.r.x - q.x).powi(2) + (s.r.y - q.y).powi(2) + (s.r.z - q.z).powi(2)).sqrt();
            }
            cum.push(d);
        }
        Self { hist, cum, shown_cm: 0.0, seg: 0, counted: false }
    }
    pub fn total(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }
    pub fn finished(&self) -> bool {
        self.shown_cm >= self.total()
    }
    /// Index of the segment being flown, and the fraction along it.
    pub fn head(&self) -> (usize, f64) {
        let k = self.seg.min(self.cum.len().saturating_sub(2));
        let len = self.cum.get(k + 1).map_or(0.0, |c| c - self.cum[k]);
        let f = if len > 0.0 { ((self.shown_cm - self.cum[k]) / len).clamp(0.0, 1.0) } else { 1.0 };
        (k, f)
    }
    /// The energy of the segment being flown, eV.
    pub fn energy_now(&self) -> f64 {
        let st = &self.hist.track.states;
        st.get(self.head().0).map_or(f64::NAN, |s| s.energy)
    }

    /// Fly for `dt` seconds of animation at the speed law, crossing as many
    /// segment boundaries as that takes: each segment is flown at the speed
    /// of its own energy ([`animated_speed`]).
    pub fn advance(&mut self, dt: f64, slider_cm_s: f64) {
        let st = &self.hist.track.states;
        let mut t = dt;
        while t > 0.0 && self.seg + 1 < self.cum.len() {
            let end = self.cum[self.seg + 1];
            let remaining = end - self.shown_cm;
            let v = animated_speed(st[self.seg].energy, slider_cm_s);
            if !(v > 0.0) {
                return;
            }
            let need = remaining.max(0.0) / v;
            if need <= t {
                self.shown_cm = end;
                self.seg += 1;
                t -= need;
            } else {
                self.shown_cm += v * t;
                t = 0.0;
            }
        }
        if self.seg + 1 >= self.cum.len() {
            self.shown_cm = self.total();
        }
    }
}

// ─── Drawing ─────────────────────────────────────────────────────────────────

/// Energy colour scale: 1e-3 eV (blue) to 2e7 eV (red), logarithmic.
pub const E_LO_LOG: f64 = -3.0;
pub const E_HI_LOG: f64 = 7.3;

pub fn energy_colour(e_ev: f64, alpha: u8) -> Color32 {
    const STOPS: [(f64, [u8; 3]); 6] = [
        (0.00, [60, 100, 255]),
        (0.25, [40, 200, 235]),
        (0.45, [90, 215, 100]),
        (0.65, [240, 215, 50]),
        (0.82, [250, 140, 40]),
        (1.00, [235, 50, 45]),
    ];
    let t = ((e_ev.max(1e-12).log10() - E_LO_LOG) / (E_HI_LOG - E_LO_LOG)).clamp(0.0, 1.0);
    let k = STOPS.iter().position(|s| s.0 >= t).unwrap_or(STOPS.len() - 1).max(1);
    let (a, b) = (STOPS[k - 1], STOPS[k]);
    let f = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    let mix = |i: usize| (a.1[i] as f64 + f * (b.1[i] as f64 - a.1[i] as f64)).round() as u8;
    Color32::from_rgba_unmultiplied(mix(0), mix(1), mix(2), alpha)
}

pub fn fmt_time(t_s: f64) -> String {
    match t_s {
        t if t >= 1e-3 => format!("{:.3} ms", t * 1e3),
        t if t >= 1e-6 => format!("{:.2} µs", t * 1e6),
        t => format!("{:.1} ns", t * 1e9),
    }
}

pub fn fmt_energy(e: f64) -> String {
    match e {
        e if e >= 1e6 => format!("{:.3} MeV", e / 1e6),
        e if e >= 1e3 => format!("{:.3} keV", e / 1e3),
        e => format!("{:.4} eV", e),
    }
}

/// A speed for the slider's equivalence label.
pub fn fmt_speed(v_cm_s: f64) -> String {
    match v_cm_s {
        v if v >= 100.0 => format!("{v:.0} cm/s"),
        v if v >= 1.0 => format!("{v:.1} cm/s"),
        v if v >= 0.01 => format!("{v:.3} cm/s"),
        v => format!("{v:.2e} cm/s"),
    }
}

/// Draw one track, projected on x-y. `to_screen` maps world cm to pixels.
/// `trail` is `(cumulative distances, head distance, trail length)`:
/// segments further than `trail length` behind the head fade towards a floor
/// alpha, so the neutron's recent path stays legible on top of its history.
#[allow(clippy::too_many_arguments)]
pub fn draw_track(
    painter: &egui::Painter,
    to_screen: impl Fn(f64, f64) -> Pos2,
    h: &History,
    upto: Option<(usize, f64)>,
    alpha: u8,
    dots: bool,
    trail: Option<(&[f64], f64, f64)>,
) {
    let st = &h.track.states;
    if st.len() < 2 {
        return;
    }
    let (last_seg, frac) = upto.unwrap_or((st.len() - 2, 1.0));
    let w = if alpha == 255 { 2.0 } else { 1.2 };
    for k in 0..=last_seg.min(st.len() - 2) {
        let (a, b) = (&st[k], &st[k + 1]);
        let f = if k == last_seg { frac } else { 1.0 };
        let end = [a.r.x + f * (b.r.x - a.r.x), a.r.y + f * (b.r.y - a.r.y)];
        let seg_alpha = match trail {
            Some((cum, head, len)) if len.is_finite() => {
                let behind = head - cum.get(k + 1).copied().unwrap_or(head);
                let t = (1.0 - behind / len).clamp(0.0, 1.0);
                (35.0 + (alpha as f64 - 35.0) * t) as u8
            }
            _ => alpha,
        };
        let col = energy_colour(a.energy, seg_alpha);
        painter.line_segment([to_screen(a.r.x, a.r.y), to_screen(end[0], end[1])], Stroke::new(w, col));
        if dots && k > 0 && a.event == TrackEvent::Scatter {
            painter.circle_filled(to_screen(a.r.x, a.r.y), 1.6, Color32::from_white_alpha(seg_alpha / 2));
        }
    }
    let birth = to_screen(st[0].r.x, st[0].r.y);
    painter.circle_stroke(birth, 5.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(90, 230, 120, alpha)));
    if upto.is_none() || (last_seg >= st.len() - 2 && frac >= 1.0) {
        let e = st.last().unwrap();
        let p = to_screen(e.r.x, e.r.y);
        match h.outcome {
            Some(TrackEvent::Fission) => {
                painter.circle_filled(p, 5.0, Color32::from_rgba_unmultiplied(255, 230, 80, alpha));
                painter.circle_stroke(p, 9.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 230, 80, alpha)));
            }
            Some(TrackEvent::Leak) => {
                // An arrow-ish open ring: the neutron left the system.
                painter.circle_stroke(p, 6.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(170, 120, 255, alpha)));
            }
            _ => {
                let s = Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 70, 70, alpha));
                painter.line_segment([p + Vec2::new(-5.0, -5.0), p + Vec2::new(5.0, 5.0)], s);
                painter.line_segment([p + Vec2::new(-5.0, 5.0), p + Vec2::new(5.0, -5.0)], s);
            }
        }
    } else if let Some((k, f)) = upto {
        let (a, b) = (&st[k], &st[k + 1]);
        let p = to_screen(a.r.x + f * (b.r.x - a.r.x), a.r.y + f * (b.r.y - a.r.y));
        painter.circle_filled(p, 4.0, Color32::WHITE);
    }
}

/// A small legend swatch row: birth, fission, capture, leak.
pub fn legend_markers(ui: &mut egui::Ui, with_leak: bool) {
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
        ui.painter().circle_stroke(r.center(), 5.0, Stroke::new(1.5, Color32::from_rgb(90, 230, 120)));
        ui.label("birth");
        let (r, _) = ui.allocate_exact_size(Vec2::new(22.0, 16.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 4.0, Color32::from_rgb(255, 230, 80));
        ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.2, Color32::from_rgb(255, 230, 80)));
        ui.label("fission");
        let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
        let (c, s) = (r.center(), Stroke::new(2.0, Color32::from_rgb(255, 70, 70)));
        ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], s);
        ui.painter().line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], s);
        ui.label("capture");
        if with_leak {
            let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
            ui.painter().circle_stroke(r.center(), 5.0, Stroke::new(2.0, Color32::from_rgb(170, 120, 255)));
            ui.label("leak");
        }
    });
}

/// The energy colour bar.
pub fn energy_bar(ui: &mut egui::Ui) {
    ui.strong("Neutron energy");
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(300.0), 14.0), egui::Sense::hover());
    let n = 80;
    for i in 0..n {
        let t = i as f64 / n as f64;
        let e = 10f64.powf(E_LO_LOG + t * (E_HI_LOG - E_LO_LOG));
        let x0 = rect.left() + rect.width() * i as f32 / n as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / n as f32;
        ui.painter().rect_filled(Rect::from_x_y_ranges(x0..=x1, rect.y_range()), 0.0, energy_colour(e, 255));
    }
    ui.horizontal(|ui| {
        ui.weak("1 meV · thermal");
        ui.add_space(20.0);
        ui.weak("1 keV");
        ui.add_space(20.0);
        ui.weak("fast · 20 MeV");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The speed law: the slider is the speed at 1 eV, and the speed scales
    /// as sqrt(E) (classical kinetic energy).
    #[test]
    fn the_animated_speed_follows_classical_kinetic_energy() {
        assert!((animated_speed(1.0, 7.0) - 7.0).abs() < 1e-12);
        assert!((animated_speed(1.0e6, 0.007) - 7.0).abs() < 1e-9);
        assert!((animated_speed(0.0253, 7.0) - 7.0 * 0.0253f64.sqrt()).abs() < 1e-12);
        // 0.0253 eV is 2200 m/s.
        assert!((neutron_speed_cm_s(0.0253) / 2.2e5 - 1.0).abs() < 2e-3);
    }
}
