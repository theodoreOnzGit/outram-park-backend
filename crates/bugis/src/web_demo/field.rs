//! Cell fields painted by hand with [`egui::Painter`]: polygons coloured by
//! value through a Crameri map, and a colour bar (gh:#1155; planned in
//! `docs/lessons/cfd-and-thermal-hydraulics-track-scoping.md`, gh:#891).
//!
//! The polygons are whatever the caller cuts from its mesh (a plane section
//! of each cell, already in screen coordinates). All of one field goes into
//! a single [`egui::Mesh`] of triangle fans, so tens of thousands of cells
//! cost one draw call.
//!
//! **Checked first:** `dhoby-ghaut`'s `dispersion_web` paints its field as
//! axis-aligned `rect_filled` squares of a regular grid through a private
//! ramp; that does not take polygons of an unstructured section, so it is
//! not reused, and it could move onto this module later.
//!
//! For the fixed/autoscale toggle and the tap probe that gh:#892 asks for:
//! [`ScaleMode`] (with [`scale_mode_checkbox`] for the panel) and [`pick`].

use egui::{Color32, Pos2, Rect, Stroke};

use crate::colour_maps::color32::crameri;

/// Which perceptually uniform map to use (Crameri, doi:10.5281/zenodo.1243862).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourMap {
    /// Sequential, dark blue to yellow: magnitudes (speed).
    Batlow,
    /// Diverging, blue–white–red: signed values centred on zero.
    Vik,
    /// Sequential, light: pressure.
    Lajolla,
}

/// A value range and a map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    pub min: f64,
    pub max: f64,
    pub map: ColourMap,
}

impl Scale {
    /// The range of `values` (finite ones only); a diverging map is made
    /// symmetric about zero. An empty or constant field gets a unit range.
    pub fn from_values(values: &[f64], map: ColourMap) -> Self {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in values.iter().filter(|v| v.is_finite()) {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        if !(lo < hi) {
            let c = if lo.is_finite() { lo } else { 0.0 };
            lo = c - 0.5;
            hi = c + 0.5;
        }
        if map == ColourMap::Vik {
            let m = lo.abs().max(hi.abs());
            lo = -m;
            hi = m;
        }
        Self { min: lo, max: hi, map }
    }

    /// `v` as a fraction of the range, clamped to \[0, 1\].
    pub fn t(&self, v: f64) -> f32 {
        (((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0)) as f32
    }

    /// The colour of `v`.
    pub fn colour(&self, v: f64) -> Color32 {
        let t = self.t(v);
        match self.map {
            ColourMap::Batlow => crameri::batlow(t),
            ColourMap::Vik => crameri::vik(t),
            ColourMap::Lajolla => crameri::lajolla(1.0 - t),
        }
    }
}

/// The triangle-fan mesh of `polygons` (screen points) coloured by `values`
/// through `scale`. A polygon with fewer than 3 points or a non-finite value
/// is skipped.
pub fn polygon_mesh(polygons: &[Vec<Pos2>], values: &[f64], scale: &Scale) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for (poly, v) in polygons.iter().zip(values) {
        if poly.len() < 3 || !v.is_finite() {
            continue;
        }
        let c = scale.colour(*v);
        let base = mesh.vertices.len() as u32;
        for p in poly {
            mesh.colored_vertex(*p, c);
        }
        for i in 1..(poly.len() as u32 - 1) {
            mesh.add_triangle(base, base + i, base + i + 1);
        }
    }
    mesh
}

/// Paint a field. Drawing code (the mesh it adds is built by the tested
/// [`polygon_mesh`]).
pub fn paint(painter: &egui::Painter, polygons: &[Vec<Pos2>], values: &[f64], scale: &Scale) {
    painter.add(egui::Shape::mesh(polygon_mesh(polygons, values, scale)));
}

/// Whether `p` is inside the convex or simple polygon `poly` (even–odd rule),
/// for picking the cell under a finger.
pub fn contains(poly: &[Pos2], p: Pos2) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// The index of the first polygon under `p`, for a tap probe (gh:#892).
/// Built on [`contains`]; `None` when `p` is over no cell.
pub fn pick(polygons: &[Vec<Pos2>], p: Pos2) -> Option<usize> {
    polygons.iter().position(|poly| poly.len() >= 3 && contains(poly, p))
}

/// Whether the colour range follows the field or is held fixed (gh:#892).
///
/// A fixed range keeps colours comparable between frames of a transient. An
/// automatic range shows the most contrast in each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleMode {
    /// Rescale to each field's own range ([`Scale::from_values`]).
    Auto,
    /// Hold `min..max`, whatever the field does.
    Fixed {
        /// Value painted with the bottom of the map, in the field's units.
        min: f64,
        /// Value painted with the top of the map, in the field's units.
        max: f64,
    },
}

impl ScaleMode {
    /// The [`Scale`] for `values` under this mode. A fixed range with
    /// `min >= max`, or with a bound that is not finite, falls back to the
    /// automatic one, so a bad lock never paints a blank field.
    pub fn scale(&self, values: &[f64], map: ColourMap) -> Scale {
        match *self {
            ScaleMode::Fixed { min, max } if min < max && min.is_finite() && max.is_finite() => {
                Scale { min, max, map }
            }
            _ => Scale::from_values(values, map),
        }
    }

    /// Switch modes. Locking holds the range `current` was drawn with, so the
    /// colours do not jump at the moment of locking. Unlocking returns to
    /// [`ScaleMode::Auto`].
    pub fn toggled(&self, current: &Scale) -> Self {
        match self {
            ScaleMode::Auto => ScaleMode::Fixed { min: current.min, max: current.max },
            ScaleMode::Fixed { .. } => ScaleMode::Auto,
        }
    }
}

/// A "Lock colour scale" checkbox for the side panel. Drawing code: the state
/// change it makes is the tested [`ScaleMode::toggled`].
pub fn scale_mode_checkbox(ui: &mut egui::Ui, mode: &mut ScaleMode, current: &Scale) {
    let mut locked = matches!(mode, ScaleMode::Fixed { .. });
    if ui.checkbox(&mut locked, "Lock colour scale").changed() {
        *mode = mode.toggled(current);
    }
}

/// A horizontal colour bar in `rect` with the range and `label` under it.
/// Drawing code (its colours come from the tested [`Scale::colour`]).
pub fn colour_bar(painter: &egui::Painter, rect: Rect, scale: &Scale, label: &str, text_px: f32) {
    let n = 48;
    let bar = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + (rect.height() * 0.45).max(8.0)));
    let w = bar.width() / n as f32;
    for i in 0..n {
        let v = scale.min + (scale.max - scale.min) * (i as f64 + 0.5) / n as f64;
        let r = Rect::from_min_size(Pos2::new(bar.left() + w * i as f32, bar.top()), egui::vec2(w + 0.5, bar.height()));
        painter.rect_filled(r, 0.0, scale.colour(v));
    }
    let ink = crate::Ink::of_ctx(painter.ctx());
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0, ink.border), egui::StrokeKind::Inside);
    let font = egui::FontId::proportional(text_px);
    let y = bar.bottom() + 2.0;
    painter.text(Pos2::new(bar.left(), y), egui::Align2::LEFT_TOP, fmt(scale.min), font.clone(), ink.text);
    painter.text(Pos2::new(bar.right(), y), egui::Align2::RIGHT_TOP, fmt(scale.max), font.clone(), ink.text);
    painter.text(Pos2::new(bar.center().x, y), egui::Align2::CENTER_TOP, label, font, ink.text);
}

/// Three significant figures, switching to an exponent outside 1e-3..1e4.
pub fn fmt(v: f64) -> String {
    let a = v.abs();
    if a != 0.0 && !(1e-3..1e4).contains(&a) {
        format!("{v:.2e}")
    } else {
        format!("{v:.3}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_range_and_colours() {
        let s = Scale::from_values(&[1.0, 3.0, f64::NAN, 2.0], ColourMap::Batlow);
        assert_eq!((s.min, s.max), (1.0, 3.0));
        assert_eq!(s.t(2.0), 0.5);
        assert_eq!(s.t(-9.0), 0.0);
        assert_eq!(s.colour(1.0), crameri::batlow(0.0));
        let d = Scale::from_values(&[-1.0, 4.0], ColourMap::Vik);
        assert_eq!((d.min, d.max), (-4.0, 4.0));
        assert_eq!(d.colour(0.0), crameri::vik(0.5));
        let c = Scale::from_values(&[], ColourMap::Lajolla);
        assert!(c.min < c.max);
        assert_eq!(c.colour(c.max), crameri::lajolla(0.0));
        assert_eq!(fmt(0.5), "0.500");
        assert_eq!(fmt(1.5e-5), "1.50e-5");
    }

    #[test]
    fn polygons_become_one_triangle_fan_mesh_and_pick() {
        let sq = vec![Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)];
        let tri = vec![Pos2::new(2.0, 0.0), Pos2::new(3.0, 0.0), Pos2::new(2.0, 1.0)];
        let s = Scale::from_values(&[0.0, 1.0], ColourMap::Batlow);
        let m = polygon_mesh(&[sq.clone(), tri, vec![Pos2::ZERO]], &[0.0, 1.0, 0.5], &s);
        assert_eq!(m.vertices.len(), 7);
        assert_eq!(m.indices.len(), 3 * 3);
        assert_eq!(m.vertices[4].color, crameri::batlow(1.0));
        assert!(contains(&sq, Pos2::new(0.5, 0.5)));
        assert!(!contains(&sq, Pos2::new(1.5, 0.5)));
    }
    #[test]
    fn fixed_or_auto_scale_and_tap_pick() {
        let vals = [2.0, 6.0];
        let auto = ScaleMode::Auto.scale(&vals, ColourMap::Batlow);
        assert_eq!((auto.min, auto.max), (2.0, 6.0));
        // Locking holds the range that was on screen, whatever comes next.
        let locked = ScaleMode::Auto.toggled(&auto);
        assert_eq!(locked, ScaleMode::Fixed { min: 2.0, max: 6.0 });
        let later = locked.scale(&[100.0, 200.0], ColourMap::Batlow);
        assert_eq!((later.min, later.max), (2.0, 6.0));
        assert_eq!(locked.toggled(&later), ScaleMode::Auto);
        // A degenerate lock falls back to the field's own range.
        let bad = ScaleMode::Fixed { min: 1.0, max: 1.0 }.scale(&vals, ColourMap::Batlow);
        assert_eq!((bad.min, bad.max), (2.0, 6.0));
        let nan = ScaleMode::Fixed { min: f64::NAN, max: 1.0 }.scale(&vals, ColourMap::Batlow);
        assert_eq!((nan.min, nan.max), (2.0, 6.0));

        let sq = vec![Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)];
        let tri = vec![Pos2::new(2.0, 0.0), Pos2::new(3.0, 0.0), Pos2::new(2.0, 1.0)];
        let polys = [vec![Pos2::ZERO], sq, tri];
        assert_eq!(pick(&polys, Pos2::new(0.5, 0.5)), Some(1));
        assert_eq!(pick(&polys, Pos2::new(2.2, 0.2)), Some(2));
        assert_eq!(pick(&polys, Pos2::new(5.0, 5.0)), None);
    }
}
