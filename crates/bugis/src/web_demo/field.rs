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
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0, Color32::from_gray(140)), egui::StrokeKind::Inside);
    let font = egui::FontId::proportional(text_px);
    let y = bar.bottom() + 2.0;
    painter.text(Pos2::new(bar.left(), y), egui::Align2::LEFT_TOP, fmt(scale.min), font.clone(), Color32::LIGHT_GRAY);
    painter.text(Pos2::new(bar.right(), y), egui::Align2::RIGHT_TOP, fmt(scale.max), font.clone(), Color32::LIGHT_GRAY);
    painter.text(Pos2::new(bar.center().x, y), egui::Align2::CENTER_TOP, label, font, Color32::LIGHT_GRAY);
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
}
