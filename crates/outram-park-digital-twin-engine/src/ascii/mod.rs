//! Render 2-D schematic geometry to a character grid.
//!
//! **Why this exists.** A GUI schematic can only be checked by a human looking
//! at it, so in practice it stops being checked. `htgr_sim_v1` accumulated
//! three drawing errors that way — core flow drawn upward when the model flows
//! it downward, the steam generator drawn co-current when the physics is
//! counter-current, and the SG vessel at the wrong height — while every test
//! passed, because the tests check numbers and the picture was checked by eye.
//!
//! A character grid is a **diffable, greppable, terminal-readable** rendering
//! that an agent or a CI job can inspect. Commit one as a fixture and a layout
//! change shows up as a text diff.
//!
//! ## This is not a substitute for assertions
//!
//! Rendering shows you *what changed*; it does not tell you *whether it is
//! right*. Where a layout property is checkable — "the SG sits below the
//! reactor", "gas and water traverse the exchanger in opposite directions" —
//! **assert it directly on the geometry**, which is cheaper, sharper, and fails
//! with a useful message. Use this to see the thing; use assertions to hold it
//! in place.
//!
//! ## Coordinates
//!
//! Screen convention, matching egui: **x grows right, y grows DOWN.** A shape
//! at smaller `y` is *higher* on the screen. This trips people up constantly,
//! so every direction-dependent method documents which way it means.
//!
//! No dependency on egui or any GUI stack — the API is plain `f32` pairs, so
//! this works headless, in a test, on wasm, or anywhere else.
//!
//! ```
//! use outram_park_digital_twin_engine::ascii::AsciiCanvas;
//!
//! let mut c = AsciiCanvas::new(24, 8, (0.0, 0.0), (100.0, 100.0));
//! c.rect(10.0, 10.0, 90.0, 60.0, Some("CORE"));
//! c.arrow(50.0, 15.0, 50.0, 55.0); // downward flow
//! let art = c.render();
//! assert!(art.contains("CORE"));
//! ```

use std::fmt;

/// A fixed-size character grid with a world-to-cell mapping.
#[derive(Clone, Debug)]
pub struct AsciiCanvas {
    cols: usize,
    rows: usize,
    min: (f32, f32),
    max: (f32, f32),
    cells: Vec<char>,
}

impl AsciiCanvas {
    /// A blank canvas `cols` x `rows`, mapping world rectangle `min`..`max`
    /// onto it.
    ///
    /// `min` is the top-left in screen coordinates (smallest x, smallest y).
    /// A degenerate world extent is widened to 1.0 so the mapping cannot divide
    /// by zero.
    pub fn new(cols: usize, rows: usize, min: (f32, f32), max: (f32, f32)) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let max = (
            if (max.0 - min.0).abs() < f32::EPSILON {
                min.0 + 1.0
            } else {
                max.0
            },
            if (max.1 - min.1).abs() < f32::EPSILON {
                min.1 + 1.0
            } else {
                max.1
            },
        );
        Self {
            cols,
            rows,
            min,
            max,
            cells: vec![' '; cols * rows],
        }
    }

    /// Grid size as `(cols, rows)`.
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// World point to cell, or `None` if it falls outside the grid.
    fn cell_of(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let fx = (x - self.min.0) / (self.max.0 - self.min.0);
        let fy = (y - self.min.1) / (self.max.1 - self.min.1);
        let cx = (fx * (self.cols as f32 - 1.0)).round();
        let cy = (fy * (self.rows as f32 - 1.0)).round();
        if cx < 0.0 || cy < 0.0 || cx >= self.cols as f32 || cy >= self.rows as f32 {
            return None;
        }
        Some((cx as usize, cy as usize))
    }

    /// Write `ch` at a world point. Out-of-bounds points are dropped silently —
    /// a schematic partly outside the view should still render what fits.
    pub fn point(&mut self, x: f32, y: f32, ch: char) {
        if let Some((cx, cy)) = self.cell_of(x, y) {
            self.cells[cy * self.cols + cx] = ch;
        }
    }

    /// A straight line in `ch`, sampled densely enough to leave no gaps.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, ch: char) {
        let steps = (self.cols + self.rows) * 2;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.point(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, ch);
        }
    }

    /// A connected polyline.
    pub fn polyline(&mut self, pts: &[(f32, f32)], ch: char) {
        for w in pts.windows(2) {
            self.line(w[0].0, w[0].1, w[1].0, w[1].1, ch);
        }
    }

    /// An axis-aligned box outline, optionally labelled at its top-left inside.
    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, label: Option<&str>) {
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        self.line(xa, ya, xb, ya, '-');
        self.line(xa, yb, xb, yb, '-');
        self.line(xa, ya, xa, yb, '|');
        self.line(xb, ya, xb, yb, '|');
        for (cx, cy) in [(xa, ya), (xb, ya), (xa, yb), (xb, yb)] {
            self.point(cx, cy, '+');
        }
        if let Some(t) = label {
            let dx = (xb - xa) * 0.06;
            let dy = (yb - ya) * 0.12;
            self.text(xa + dx, ya + dy, t);
        }
    }

    /// An arrow from one world point to another, with a head showing direction.
    ///
    /// The head glyph is chosen from the dominant axis: `v` for **downward**
    /// (increasing y), `^` upward, `>` right, `<` left. Direction is the whole
    /// point — this is what makes a flow-direction bug visible.
    pub fn arrow(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let shaft = if dx.abs() > dy.abs() { '-' } else { '|' };
        self.line(x0, y0, x1, y1, shaft);
        let head = if dx.abs() > dy.abs() {
            if dx >= 0.0 {
                '>'
            } else {
                '<'
            }
        } else if dy >= 0.0 {
            'v'
        } else {
            '^'
        };
        self.point(x1, y1, head);
    }

    /// Left-aligned text starting at a world point, clipped at the right edge.
    pub fn text(&mut self, x: f32, y: f32, s: &str) {
        if let Some((cx, cy)) = self.cell_of(x, y) {
            for (i, ch) in s.chars().enumerate() {
                if cx + i >= self.cols {
                    break;
                }
                self.cells[cy * self.cols + cx + i] = ch;
            }
        }
    }

    /// The grid as text, one line per row, trailing blanks trimmed so the
    /// output diffs cleanly.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() + self.rows);
        for r in 0..self.rows {
            let row: String = self.cells[r * self.cols..(r + 1) * self.cols]
                .iter()
                .collect();
            out.push_str(row.trim_end());
            out.push('\n');
        }
        out
    }
}

impl fmt::Display for AsciiCanvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_head_encodes_direction_with_y_growing_down() {
        let mut down = AsciiCanvas::new(9, 9, (0.0, 0.0), (10.0, 10.0));
        down.arrow(5.0, 1.0, 5.0, 9.0);
        assert!(
            down.render().contains('v'),
            "downward arrow should render 'v'"
        );

        let mut up = AsciiCanvas::new(9, 9, (0.0, 0.0), (10.0, 10.0));
        up.arrow(5.0, 9.0, 5.0, 1.0);
        assert!(up.render().contains('^'), "upward arrow should render '^'");

        let mut right = AsciiCanvas::new(9, 9, (0.0, 0.0), (10.0, 10.0));
        right.arrow(1.0, 5.0, 9.0, 5.0);
        assert!(right.render().contains('>'));
    }

    #[test]
    fn out_of_bounds_is_dropped_not_panicking() {
        let mut c = AsciiCanvas::new(8, 4, (0.0, 0.0), (10.0, 10.0));
        c.point(-100.0, -100.0, 'X');
        c.point(f32::NAN, 0.0, 'X');
        c.point(1e30, 1e30, 'X');
        assert!(!c.render().contains('X'));
    }

    #[test]
    fn rect_is_labelled_and_closed() {
        let mut c = AsciiCanvas::new(20, 8, (0.0, 0.0), (100.0, 100.0));
        c.rect(10.0, 10.0, 90.0, 90.0, Some("SG"));
        let art = c.render();
        assert!(art.contains("SG"));
        assert_eq!(art.matches('+').count(), 4, "four corners expected:\n{art}");
    }

    #[test]
    fn degenerate_extent_does_not_divide_by_zero() {
        let mut c = AsciiCanvas::new(4, 4, (5.0, 5.0), (5.0, 5.0));
        c.point(5.0, 5.0, '*');
        assert!(c.render().contains('*'));
    }
}
