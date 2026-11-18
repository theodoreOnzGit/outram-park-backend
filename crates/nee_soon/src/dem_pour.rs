//! **The DEM pour's diagnostics and drawings, for any pebble-bed k_eff
//! simulator** (gh:#1655; maintainer, 2026-11-18: "for pebble beds I want
//! to see the Dem interface plus rdf fill graph shown, this shld be reusable
//! library in nee soon").
//!
//! Moved from the HTR-10 rung of `dhoby-ghaut`'s `monte_carlo_web`
//! (`htr10/core/pour.rs` and `screen.rs`, gh:#1244), which now calls these:
//!
//! - [`PourDiagnostics`] and [`diagnostics`]: a finished pour measured by the
//!   whole-core packing fraction and `g(r)` of
//!   `outram_park_fork_liggghts::rdf::radial_distribution`, the instrument
//!   `bed_rdf_check` measures the reference bed with; [`g_at`] reads it at
//!   the second-shell points, [`REFERENCE_G`] is the reference bed's;
//! - [`rdf_plot`]: the g(r) graph against the reference points;
//! - [`draw_pour`]: the pour side-on, the pebbles cut by `y = 0`, inside a
//!   container given as its wall radius against height.
//!
//! The vessel-specific parts stay with the caller: the bulk fraction of a
//! particular vessel (`phi_bulk`) and the timing (`secs`) are passed in.
//! Behind the `dem_ui` feature (std, egui), so nee_soon's `no_std` core and
//! its Android build are unaffected (gh:#1640). Research, education and V&V
//! only.

use egui::{Color32, Pos2, Rect, Vec2};
use outram_park_fork_liggghts::htr10_fill::{surface_height_m, whole_core_fraction, PEBBLE_RADIUS_M};

/// Fewest interior centres a `g(r)` is taken over ([`diagnostics`]).
pub const RDF_MIN_CENTRES: usize = 200;

/// The reference random bed's `g(r)` at the second-shell points and its
/// contact peak (core region, `bed_rdf_check`, as `htr10_fill::seed_loose`'s
/// doc records them for `htr10_conus_presettled_mu10_mur00.csv`): g(√2 d),
/// g(√3 d), g(2 d), peak.
pub const REFERENCE_G: [f64; 4] = [0.599, 1.286, 1.190, 19.56];
/// The reference bed's whole-core φ (V&V § 4.9) and the published 0.61
/// (IAEA-TECDOC-1382, quoted, not measured).
pub const REFERENCE_PHI: f64 = 0.6047;
pub const PUBLISHED_PHI: f64 = 0.61;

/// **What a finished pour is, measured** (gh:#1244, maintainer: "post pour,
/// give diagnostics like packing fraction and rdf graph").
#[derive(Clone, Debug, PartialEq)]
pub struct PourDiagnostics {
    /// Pebbles, and those centred above the floor.
    pub n: usize,
    pub n_core: usize,
    /// Bed surface (99th percentile core centre + r) \[cm\].
    pub surface_cm: f64,
    /// Whole-core φ `N V / (π R² h)` (gh:#216's instrument, against 0.61).
    pub phi_whole_core: f64,
    /// Bulk solid fraction of the filled vessel, the caller's (NaN when it
    /// has none).
    pub phi_bulk: f64,
    /// `g(r)` over the core (`z > 0`), `r` in diameters, and the `r_max` used
    /// \[d\]; empty when the bed is too shallow ([`diagnostics`]).
    pub r_over_d: Vec<f64>,
    pub g: Vec<f64>,
    pub r_max_d: f64,
    /// Interior centres the histogram was taken over.
    pub rdf_centres: usize,
    /// Mean contact coordination (`Rdf::contact_coordination`).
    pub coordination: f64,
    /// Measurement time, s (the caller's clock).
    pub secs: f64,
}

/// `g` at `x` diameters: the mean of the three bins around it (as
/// `bed_rdf_check` reads it); NaN without a `g(r)`.
pub fn g_at(d: &PourDiagnostics, x: f64) -> f64 {
    let Some(i) = d.r_over_d.iter().position(|r| *r >= x) else { return f64::NAN };
    let lo = i.saturating_sub(1);
    let hi = (i + 1).min(d.g.len() - 1);
    d.g[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64
}

/// **The pour's diagnostics** from its centres (m, flattened): the
/// whole-core packing fraction, and `g(r)` of the pebbles above the floor by
/// `outram_park_fork_liggghts::rdf::radial_distribution` (one thread), in
/// the bed's own cylinder from its centres, as `bed_rdf_check` measures the
/// reference bed. The estimator only counts centres at least `r_max` from
/// every face, so `r_max` is the reference's 5 d whenever at least
/// [`RDF_MIN_CENTRES`] centres are that far inside; a shallower bed steps it
/// down by d/4 until they are, to 2.5 d (just past the 2 d peak); below that
/// there is no `g(r)`. `phi_bulk` and `secs` are the caller's.
pub fn diagnostics(centres_m: &[f32], phi_bulk: f64, secs: f64) -> PourDiagnostics {
    use outram_park_fork_liggghts::compute::ComputeType;
    use outram_park_fork_liggghts::particle::Vec3;
    use outram_park_fork_liggghts::rdf::{radial_distribution, RdfDomain, RdfSettings};
    let all: Vec<Vec3> = centres_m.chunks_exact(3).map(|c| Vec3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2]))).collect();
    let core: Vec<Vec3> = all.iter().copied().filter(|c| c.z > 0.0).collect();
    let d = 2.0 * PEBBLE_RADIUS_M;
    let domain = RdfDomain::from_positions(&core);
    let mut r_max_d = 5.0;
    let interior = |r_max: f64| core.iter().filter(|p| domain.is_interior(**p, r_max)).count();
    while r_max_d > 2.5 && interior(r_max_d * d) < RDF_MIN_CENTRES {
        r_max_d -= 0.25;
    }
    let (mut r_over_d, mut g, mut rdf_centres, mut coordination) = (Vec::new(), Vec::new(), 0, f64::NAN);
    if !core.is_empty() && interior(r_max_d * d) > 0 {
        let settings = RdfSettings { r_max: r_max_d * d, n_bins: (50.0 * r_max_d) as usize, domain };
        let rdf = radial_distribution(&core, settings, ComputeType::CpuSingleThread);
        r_over_d = rdf.r.iter().map(|r| r / d).collect();
        g = rdf.g.clone();
        rdf_centres = interior(r_max_d * d);
        coordination = rdf.contact_coordination().unwrap_or(f64::NAN);
    } else {
        r_max_d = 0.0;
    }
    PourDiagnostics {
        n: all.len(),
        n_core: core.len(),
        surface_cm: surface_height_m(&all).map_or(f64::NAN, |h| 100.0 * h),
        phi_whole_core: whole_core_fraction(&all),
        phi_bulk,
        r_over_d,
        g,
        r_max_d,
        rdf_centres,
        coordination,
        secs,
    }
}

/// `n` with thin-space thousands: 16 890 → "16 890".
pub fn grouped(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

impl PourDiagnostics {
    /// As numbers for the wire, and back ([`Self::from_words`]).
    pub fn to_words(&self) -> Vec<f64> {
        let mut w = vec![
            self.n as f64,
            self.n_core as f64,
            self.surface_cm,
            self.phi_whole_core,
            self.phi_bulk,
            self.r_max_d,
            self.rdf_centres as f64,
            self.coordination,
            self.secs,
            self.g.len() as f64,
        ];
        w.extend(&self.r_over_d);
        w.extend(&self.g);
        w
    }

    /// The inverse of [`Self::to_words`].
    ///
    /// # Errors
    /// A message when the words are short or the bin count does not match.
    pub fn from_words(w: &[f64]) -> Result<Self, String> {
        const HEAD: usize = 10;
        let n = *w.get(HEAD - 1).ok_or("pour diagnostics: short")? as usize;
        if w.len() != HEAD + 2 * n {
            return Err(format!("pour diagnostics: {} words for {n} bins", w.len()));
        }
        Ok(Self {
            n: w[0] as usize,
            n_core: w[1] as usize,
            surface_cm: w[2],
            phi_whole_core: w[3],
            phi_bulk: w[4],
            r_max_d: w[5],
            rdf_centres: w[6] as usize,
            coordination: w[7],
            secs: w[8],
            r_over_d: w[HEAD..HEAD + n].to_vec(),
            g: w[HEAD + n..].to_vec(),
        })
    }

    /// The page's lines: packing fractions against the reference bed and the
    /// published figure, and the g(r) points that tell random from ordered.
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![format!(
            "Packing: whole-core φ {:.4} (the reference full core {REFERENCE_PHI}; published {PUBLISHED_PHI}, quoted) · bulk φ {:.4} · surface {:.1} cm · {} pebbles, {} above the floor",
            self.phi_whole_core,
            self.phi_bulk,
            self.surface_cm,
            grouped(self.n),
            grouped(self.n_core)
        )];
        if self.g.is_empty() {
            v.push("g(r): the bed is too shallow for one (a centre must stand 2.5 pebble diameters clear of every face of the bed).".into());
        } else {
            let [r2, r3, r4, rp] = REFERENCE_G;
            let peak = self.g.iter().copied().fold(0.0, f64::max);
            v.push(format!(
                "g(r) over {} interior centres to {:.2} d: contact peak {peak:.2} (ref {rp}), g(√2 d) {:.3} (ref {r2}; an FCC crystal peaks here), g(√3 d) {:.3} (ref {r3}), g(2 d) {:.3} (ref {r4}), contact coordination {:.2}. The split √3 d / 2 d peak with a √2 d trough is a random packing.",
                self.rdf_centres,
                self.r_max_d,
                g_at(self, 2f64.sqrt()),
                g_at(self, 3f64.sqrt()),
                g_at(self, 2.0),
                self.coordination
            ));
        }
        v.push(format!("Measured in the worker in {:.2} s.", self.secs));
        v
    }
}

/// **The pour's g(r)**, with the reference random bed's points
/// ([`REFERENCE_G`]) marked, √2 d (where a crystal would peak), √3 d and 2 d
/// as guides, and g = 1. Colours from the Gruvbox theme (bugis). GUI
/// drawing, not reached by a test (the workspace's GUI exception); its
/// numbers are [`diagnostics`]', which is.
pub fn rdf_plot(ui: &mut egui::Ui, d: &PourDiagnostics) {
    let theme = bugis::Theme::current(ui.visuals());
    let fg = ui.visuals().text_color();
    let faint = ui.visuals().weak_text_color();
    let w = ui.available_width().max(160.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 170.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let plot = Rect::from_min_max(rect.min + Vec2::new(34.0, 8.0), rect.max - Vec2::new(8.0, 22.0));
    let x_max = d.r_max_d.max(1.0);
    // A linear axis clipped at 6 so the second shell reads; the contact
    // peak's height is in the line above.
    let y_max = 6.0;
    let at = |x: f64, y: f64| Pos2::new(plot.left() + (x / x_max) as f32 * plot.width(), plot.bottom() - (y.min(y_max) / y_max) as f32 * plot.height());
    for (x, label) in [(1.0, "d"), (2f64.sqrt(), "√2"), (3f64.sqrt(), "√3"), (2.0, "2d")] {
        if x <= x_max {
            p.line_segment([at(x, 0.0), at(x, y_max)], egui::Stroke::new(0.5, faint));
            p.text(at(x, 0.0) + Vec2::new(0.0, 3.0), egui::Align2::CENTER_TOP, label, egui::FontId::proportional(10.0), faint);
        }
    }
    p.line_segment([at(0.0, 1.0), at(x_max, 1.0)], egui::Stroke::new(0.5, faint));
    for y in [0.0, 1.0, 2.0, 4.0, 6.0] {
        p.text(Pos2::new(plot.left() - 4.0, at(0.0, y).y), egui::Align2::RIGHT_CENTER, format!("{y}"), egui::FontId::proportional(10.0), faint);
    }
    let pts: Vec<Pos2> = d.r_over_d.iter().zip(&d.g).filter(|(_, g)| g.is_finite()).map(|(&x, &g)| at(x, g)).collect();
    p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, theme.accent(0))));
    let [g2, g3, g4, _] = REFERENCE_G;
    for (x, g) in [(2f64.sqrt(), g2), (3f64.sqrt(), g3), (2.0, g4)] {
        if x <= x_max {
            p.circle_stroke(at(x, g), 3.5, egui::Stroke::new(1.5, theme.accent(1)));
        }
    }
    p.text(plot.right_top(), egui::Align2::RIGHT_TOP, "g(r): yours (line), reference bed (circles)", egui::FontId::proportional(10.5), fg);
    p.text(Pos2::new(plot.center().x, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM, "r / d", egui::FontId::proportional(10.0), faint);
}

/// How [`draw_pour`] draws the container: its wall as radius against
/// height (cm, both sides drawn mirrored), horizontal marks (height cm,
/// label, colour), and `shift` cm subtracted from every height (to centre
/// the bed in the view).
#[derive(Clone, Debug, PartialEq)]
pub struct PourFrame {
    pub wall: Vec<(f64, f64)>,
    pub wall_colour: Color32,
    pub marks: Vec<(f64, String, Color32)>,
    /// Half-width of the marks \[cm\].
    pub mark_half_width: f64,
    pub shift: f64,
}

/// **The pour side-on**: the pebbles cut by the plane `y = 0` (each a disc
/// of its cut radius), those below `z = 0` faint, inside the container of
/// `frame`. GUI drawing, not reached by a test (the workspace's GUI
/// exception).
pub fn draw_pour(painter: &egui::Painter, rect: Rect, view: &bugis::web_demo::view::View, centres_m: &[f32], radius_cm: f64, frame: &PourFrame, visuals: &egui::Visuals) {
    painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
    let at = |x: f64, z: f64| view.to_screen(rect, x, z - frame.shift);
    let wall = egui::Stroke::new(1.5, frame.wall_colour);
    for side in [-1.0, 1.0] {
        let pts: Vec<Pos2> = frame.wall.iter().map(|&(r, z)| at(side * r, z)).collect();
        painter.add(egui::Shape::line(pts, wall));
    }
    for (z, label, c) in &frame.marks {
        painter.line_segment([at(-frame.mark_half_width, *z), at(frame.mark_half_width, *z)], egui::Stroke::new(1.0, *c));
        if !label.is_empty() {
            painter.text(at(frame.mark_half_width + 2.0, *z), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), *c);
        }
    }
    let r = radius_cm;
    let scale = (at(1.0, 0.0).x - at(0.0, 0.0).x).abs();
    for c in centres_m.chunks_exact(3) {
        let (x, y, z) = (100.0 * f64::from(c[0]), 100.0 * f64::from(c[1]), 100.0 * f64::from(c[2]));
        if y.abs() >= r {
            continue;
        }
        let rr = (r * r - y * y).sqrt() * f64::from(scale);
        let col = if z > 0.0 { visuals.text_color() } else { visuals.weak_text_color() };
        painter.circle_filled(at(x, z), rr.max(1.0) as f32, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple cubic block of pebbles: φ and g(r) come out, the wire
    /// round-trips, and the lines name the reference.
    #[test]
    fn diagnostics_measure_a_bed_and_survive_the_wire() {
        let d = 2.0 * PEBBLE_RADIUS_M as f32 * 1.001;
        let mut c = Vec::new();
        for i in 0..14 {
            for j in 0..14 {
                for k in 0..14 {
                    c.extend([i as f32 * d - 0.4, j as f32 * d - 0.4, k as f32 * d + 0.01]);
                }
            }
        }
        let diag = diagnostics(&c, 0.42, 0.5);
        assert_eq!(diag.n, 14 * 14 * 14);
        assert_eq!(diag.n_core, diag.n);
        assert!(diag.phi_whole_core > 0.0);
        assert!(!diag.g.is_empty(), "a 14-layer block has interior centres");
        // A cubic lattice has no neighbour at √2 d·0.999.. but its first shell
        // at d: the contact peak dominates.
        assert!(diag.g.iter().copied().fold(0.0, f64::max) > 5.0);
        assert_eq!(PourDiagnostics::from_words(&diag.to_words()).unwrap(), diag);
        assert!(diag.lines()[0].contains("0.6047"));
        assert_eq!(grouped(16_890), "16 890");
        assert_eq!(grouped(999), "999");
        let shallow = diagnostics(&c[..9], f64::NAN, 0.0);
        assert!(shallow.g.is_empty() && shallow.lines()[1].contains("too shallow"));
    }
}
