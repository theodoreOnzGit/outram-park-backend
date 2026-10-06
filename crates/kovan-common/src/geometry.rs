//! World-space geometry shared by kovan's map views: a [`Point`] and an
//! axis-aligned [`Bounds`]. Moved 2026-10-06 from `kovan::mindmap_layout`
//! (which re-exports both) so the wasm-clean views (`mindmap_view`,
//! `code_map`, web-kovan) need nothing from the AGPL `kovan` crate.

/// A point in world space (the same space the mind-map and code-map layouts place cards in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned bounding box in world space, as produced by
/// `kovan::mindmap_layout::bounds_for`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    /// Width, floored at `1.0` so a single-point or degenerate bounds
    /// never divides by zero in a camera fit.
    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(1.0)
    }

    /// Height, floored at `1.0` — see [`width`](Self::width).
    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(1.0)
    }

    /// The bounds' centre point.
    pub fn centre(&self) -> Point {
        Point::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }
}

