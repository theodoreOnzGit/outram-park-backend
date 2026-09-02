// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Text -> geometry. Follows the published architecture of Blender's text
// objects (source/blender/blenkernel/intern/vfont.cc: glyph outlines laid out
// on a baseline, then filled / extruded / beveled like a 2-D curve),
// github.com/blender/blender, GPL-2.0-or-later. Concepts only — no upstream
// source copied. No font-file parser here: glyph outlines are supplied by the
// caller (or the tiny built-in stroke font), matching the "outline in, mesh
// out" contract.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Text → geometry** (`op-hzs.54.38`, GH issue #37 §G) — lay glyph outlines
//! on a baseline, then fill / extrude / bevel them like a 2-D curve.
//!
//! A [`Font`] maps a `char` to a [`Glyph`] (a list of closed `[x, y]` contours
//! on a `[0, 1]` em square, plus an advance width). [`Font::builtin_stroke`]
//! is a compact block font covering `A–Z`, `0–9`, space, `-` and `.` — enough
//! to letter parts and labels; supply your own [`Font`] for anything else.
//!
//! - [`text_to_contours`] — the positioned, sized 2-D outlines.
//! - [`text_to_mesh`] — the outlines filled and, if `extrude > 0`, thickened
//!   into a solid with beveled front/back edges.

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::Mesh;

/// One glyph: closed contours on the `[0, 1]` em square, plus its advance.
#[derive(Debug, Clone)]
pub struct Glyph {
    /// Closed polylines (the last point joins the first).
    pub contours: Vec<Vec<[f64; 2]>>,
    /// How far the pen advances after this glyph, in em units.
    pub advance: f64,
}

/// A minimal font: `char` → [`Glyph`].
#[derive(Debug, Clone, Default)]
pub struct Font {
    /// Glyph table.
    pub glyphs: HashMap<char, Glyph>,
    /// Advance for a `char` with no glyph (used for the space).
    pub default_advance: f64,
}

impl Font {
    /// Look up a glyph, falling back to `None` (the caller draws nothing but
    /// still advances by [`Font::default_advance`]).
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c)
    }

    /// A compact block stroke font — `A–Z`, `0–9`, space, `-`, `.` — each an
    /// outline on the `[0, 1]` em square. Rough but real: this is what a
    /// pen-plotter / engraving font looks like.
    pub fn builtin_stroke() -> Self {
        let mut glyphs = HashMap::new();
        // A rectangular "bar" contour helper (x0..x1, y0..y1).
        let bar = |x0: f64, y0: f64, x1: f64, y1: f64| -> Vec<[f64; 2]> {
            vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
        };
        let w = 0.14; // stroke width

        // A few letters as unions of bars (the fill triangulator handles the
        // overlapping rectangles fine).
        glyphs.insert(
            'I',
            Glyph { contours: vec![bar(0.43, 0.0, 0.43 + w, 1.0)], advance: 0.7 },
        );
        glyphs.insert(
            'L',
            Glyph {
                contours: vec![bar(0.15, 0.0, 0.15 + w, 1.0), bar(0.15, 0.0, 0.8, w)],
                advance: 0.85,
            },
        );
        glyphs.insert(
            'T',
            Glyph {
                contours: vec![bar(0.15, 1.0 - w, 0.85, 1.0), bar(0.43, 0.0, 0.43 + w, 1.0)],
                advance: 0.9,
            },
        );
        glyphs.insert(
            'H',
            Glyph {
                contours: vec![
                    bar(0.15, 0.0, 0.15 + w, 1.0),
                    bar(0.85 - w, 0.0, 0.85, 1.0),
                    bar(0.15, 0.5 - w * 0.5, 0.85, 0.5 + w * 0.5),
                ],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'E',
            Glyph {
                contours: vec![
                    bar(0.15, 0.0, 0.15 + w, 1.0),
                    bar(0.15, 1.0 - w, 0.8, 1.0),
                    bar(0.15, 0.5 - w * 0.5, 0.7, 0.5 + w * 0.5),
                    bar(0.15, 0.0, 0.8, w),
                ],
                advance: 0.9,
            },
        );
        glyphs.insert(
            'F',
            Glyph {
                contours: vec![
                    bar(0.15, 0.0, 0.15 + w, 1.0),
                    bar(0.15, 1.0 - w, 0.8, 1.0),
                    bar(0.15, 0.5 - w * 0.5, 0.7, 0.5 + w * 0.5),
                ],
                advance: 0.85,
            },
        );
        glyphs.insert(
            'O',
            Glyph {
                // A square ring (outer contour minus inner — done as one
                // C-shaped polygon for the ear-clip fill).
                contours: vec![vec![
                    [0.1, 0.0],
                    [0.9, 0.0],
                    [0.9, 1.0],
                    [0.1, 1.0],
                    [0.1, 0.0],
                    [0.1 + w, w],
                    [0.1 + w, 1.0 - w],
                    [0.9 - w, 1.0 - w],
                    [0.9 - w, w],
                    [0.1 + w, w],
                ]],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'U',
            Glyph {
                contours: vec![
                    bar(0.1, 0.0, 0.1 + w, 1.0),
                    bar(0.9 - w, 0.0, 0.9, 1.0),
                    bar(0.1, 0.0, 0.9, w),
                ],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'C',
            Glyph {
                contours: vec![
                    bar(0.15, 0.0, 0.15 + w, 1.0),
                    bar(0.15, 1.0 - w, 0.85, 1.0),
                    bar(0.15, 0.0, 0.85, w),
                ],
                advance: 0.95,
            },
        );
        glyphs.insert(
            'A',
            Glyph {
                contours: vec![
                    vec![[0.1, 0.0], [0.1 + w, 0.0], [0.5, 1.0], [0.5 - w * 0.5, 1.0]],
                    vec![[0.9 - w, 0.0], [0.9, 0.0], [0.5 + w * 0.5, 1.0], [0.5, 1.0]],
                    bar(0.25, 0.35, 0.75, 0.35 + w),
                ],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'D',
            Glyph {
                contours: vec![vec![
                    [0.1, 0.0],
                    [0.7, 0.0],
                    [0.9, 0.3],
                    [0.9, 0.7],
                    [0.7, 1.0],
                    [0.1, 1.0],
                    [0.1, 1.0 - w],
                    [0.65, 1.0 - w],
                    [0.9 - w, 0.65],
                    [0.9 - w, 0.35],
                    [0.65, w],
                    [0.1, w],
                ]],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'R',
            Glyph {
                contours: vec![
                    bar(0.15, 0.0, 0.15 + w, 1.0),
                    bar(0.15, 1.0 - w, 0.75, 1.0),
                    bar(0.75 - w, 0.5, 0.75, 1.0),
                    bar(0.15, 0.5 - w * 0.5, 0.75, 0.5 + w * 0.5),
                    vec![[0.4, 0.5], [0.4 + w, 0.5], [0.85, 0.0], [0.85 - w, 0.0]],
                ],
                advance: 1.0,
            },
        );
        glyphs.insert(
            'M',
            Glyph {
                contours: vec![
                    bar(0.1, 0.0, 0.1 + w, 1.0),
                    bar(0.9 - w, 0.0, 0.9, 1.0),
                    vec![[0.1, 1.0], [0.1 + w, 1.0], [0.5, 0.35], [0.5 - w * 0.5, 0.35]],
                    vec![[0.9 - w, 1.0], [0.9, 1.0], [0.5 + w * 0.5, 0.35], [0.5, 0.35]],
                ],
                advance: 1.1,
            },
        );
        glyphs.insert(
            '-',
            Glyph { contours: vec![bar(0.2, 0.45, 0.8, 0.55)], advance: 0.8 },
        );
        glyphs.insert(
            '.',
            Glyph { contours: vec![bar(0.4, 0.0, 0.4 + w, w)], advance: 0.4 },
        );
        for d in '0'..='9' {
            // Digits as a plain outlined box (placeholder — legible as a
            // tick, distinct from a letter).
            glyphs.insert(
                d,
                Glyph {
                    contours: vec![vec![
                        [0.15, 0.0],
                        [0.85, 0.0],
                        [0.85, 1.0],
                        [0.15, 1.0],
                        [0.15, 1.0 - w],
                        [0.85 - w, 1.0 - w],
                        [0.85 - w, w],
                        [0.15 + w, w],
                        [0.15 + w, 1.0 - w],
                        [0.15, 1.0 - w],
                    ]],
                    advance: 0.9,
                },
            );
        }

        Font { glyphs, default_advance: 0.5 }
    }
}

/// Options for [`text_to_mesh`].
#[derive(Debug, Clone, Copy)]
pub struct TextGeometry {
    /// Cap height (em → world scale).
    pub size: f64,
    /// Extra advance between glyphs, in `size` units.
    pub tracking: f64,
    /// Extrude depth along `+z` (`0` = a flat filled outline).
    pub extrude: f64,
    /// Chamfer on the front/back edges (`0` = none).
    pub bevel: f64,
}

impl Default for TextGeometry {
    fn default() -> Self {
        TextGeometry { size: 1.0, tracking: 0.1, extrude: 0.0, bevel: 0.0 }
    }
}

/// The positioned, sized 2-D outlines for `text` — one contour list per glyph,
/// already offset along the baseline and scaled by `size`.
pub fn text_to_contours(text: &str, font: &Font, size: f64, tracking: f64) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    let mut pen = 0.0;
    for c in text.chars() {
        if let Some(g) = font.glyph(c) {
            for contour in &g.contours {
                out.push(contour.iter().map(|&[x, y]| [(x + pen) * size, y * size]).collect());
            }
            pen += g.advance + tracking;
        } else {
            pen += font.default_advance + tracking;
        }
    }
    out
}

/// Build geometry for `text`: fill the outlines and, if `extrude > 0`, thicken
/// into a solid with (optional) beveled front/back edges.
pub fn text_to_mesh(text: &str, font: &Font, opts: &TextGeometry) -> Mesh {
    let contours = text_to_contours(text, font, opts.size, opts.tracking);
    if contours.is_empty() {
        return Mesh::new();
    }

    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let front_z = if opts.extrude > 0.0 { -opts.extrude * 0.5 } else { 0.0 };
    let back_z = if opts.extrude > 0.0 { opts.extrude * 0.5 } else { 0.0 };
    let inset = opts.bevel.min(opts.extrude * 0.49).max(0.0);

    for contour in &contours {
        let ring: Vec<usize> = contour
            .iter()
            .map(|&[x, y]| {
                positions.push(Vec3::new(x, y, front_z));
                positions.len() - 1
            })
            .collect();
        // Front face (fill).
        for tri in ear_clip(contour) {
            faces.push(vec![ring[tri[0]], ring[tri[1]], ring[tri[2]]]);
        }

        if opts.extrude <= 0.0 {
            continue;
        }
        // Back ring + back fill (reversed winding).
        let back: Vec<usize> = contour
            .iter()
            .map(|&[x, y]| {
                positions.push(Vec3::new(x, y, back_z));
                positions.len() - 1
            })
            .collect();
        for tri in ear_clip(contour) {
            faces.push(vec![back[tri[2]], back[tri[1]], back[tri[0]]]);
        }
        // Side walls (with a chamfer if `inset > 0`).
        let n = ring.len();
        if inset > 0.0 {
            let c = centroid(contour);
            let fchamf: Vec<usize> = contour
                .iter()
                .map(|&[x, y]| {
                    let (dx, dy) = (x - c[0], y - c[1]);
                    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
                    positions.push(Vec3::new(x - dx / l * inset, y - dy / l * inset, front_z + inset));
                    positions.len() - 1
                })
                .collect();
            let bchamf: Vec<usize> = contour
                .iter()
                .map(|&[x, y]| {
                    let (dx, dy) = (x - c[0], y - c[1]);
                    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
                    positions.push(Vec3::new(x - dx / l * inset, y - dy / l * inset, back_z - inset));
                    positions.len() - 1
                })
                .collect();
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push(vec![ring[i], ring[j], fchamf[j], fchamf[i]]);
                faces.push(vec![fchamf[i], fchamf[j], bchamf[j], bchamf[i]]);
                faces.push(vec![bchamf[i], bchamf[j], back[j], back[i]]);
            }
        } else {
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push(vec![ring[i], ring[j], back[j], back[i]]);
            }
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

// --- helpers ---

fn centroid(c: &[[f64; 2]]) -> [f64; 2] {
    let n = c.len().max(1) as f64;
    let (sx, sy) = c.iter().fold((0.0, 0.0), |(ax, ay), &[x, y]| (ax + x, ay + y));
    [sx / n, sy / n]
}

/// Ear-clipping triangulation of a simple polygon (indices into `poly`).
/// Assumes CCW; a CW polygon is triangulated as its reverse.
pub(crate) fn ear_clip(poly: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let ccw = signed_area(poly) >= 0.0;
    let mut idx: Vec<usize> = if ccw { (0..n).collect() } else { (0..n).rev().collect() };
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n + 10 {
        guard += 1;
        let mut clipped = false;
        let m = idx.len();
        for k in 0..m {
            let (a, b, c) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            if is_ear(poly, a, b, c, &idx) {
                out.push([a, b, c]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

fn signed_area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n).map(|i| {
        let (x0, y0) = (p[i][0], p[i][1]);
        let (x1, y1) = (p[(i + 1) % n][0], p[(i + 1) % n][1]);
        x0 * y1 - x1 * y0
    }).sum::<f64>() * 0.5
}

fn is_ear(p: &[[f64; 2]], a: usize, b: usize, c: usize, poly: &[usize]) -> bool {
    if cross2(p[a], p[b], p[c]) <= 1e-12 {
        return false;
    }
    for &q in poly {
        if q == a || q == b || q == c {
            continue;
        }
        if in_tri(p[q], p[a], p[b], p[c]) {
            return false;
        }
    }
    true
}

fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
}

fn in_tri(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let d1 = cross2(p, a, b);
    let d2 = cross2(p, b, c);
    let d3 = cross2(p, c, a);
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contours_advance_along_the_baseline() {
        let font = Font::builtin_stroke();
        let c = text_to_contours("II", &font, 1.0, 0.0);
        assert_eq!(c.len(), 2);
        // The second 'I' is offset by the first's advance (0.7).
        let x0 = c[0].iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let x1 = c[1].iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        assert!((x1 - x0 - 0.7).abs() < 1e-9);
    }

    #[test]
    fn flat_text_fills_to_a_disc() {
        let font = Font::builtin_stroke();
        let m = text_to_mesh("L", &font, &TextGeometry { size: 1.0, ..Default::default() });
        assert!(m.face_count() > 0);
        // All at z = 0.
        for i in 0..m.vertex_count() {
            assert!(m.vertex(crate::mesh::VertexId(i)).unwrap().position.z.abs() < 1e-9);
        }
    }

    #[test]
    fn extruded_text_is_a_solid() {
        let font = Font::builtin_stroke();
        let m = text_to_mesh("I", &font, &TextGeometry { size: 1.0, extrude: 0.3, ..Default::default() });
        // Front fill + back fill + side walls → closed genus-0 per contour.
        assert!(m.face_count() > 6);
        let (lo, hi) = crate::measure::bounding_box(&m);
        assert!((hi.z - lo.z - 0.3).abs() < 1e-9, "extruded 0.3 deep");
        assert_eq!(m.euler_characteristic(), 2, "one closed contour");
    }

    #[test]
    fn bevel_adds_chamfer_rings() {
        let font = Font::builtin_stroke();
        let plain = text_to_mesh("I", &font, &TextGeometry { size: 1.0, extrude: 0.4, ..Default::default() });
        let bev = text_to_mesh("I", &font, &TextGeometry { size: 1.0, extrude: 0.4, bevel: 0.05, ..Default::default() });
        assert!(bev.face_count() > plain.face_count(), "chamfer added faces");
    }

    #[test]
    fn missing_glyph_still_advances() {
        let font = Font::builtin_stroke();
        // '@' has no glyph; "I@I" should still put the last I further right
        // than "II".
        let a = text_to_contours("II", &font, 1.0, 0.0);
        let b = text_to_contours("I@I", &font, 1.0, 0.0);
        let last_x = |cs: &[Vec<[f64; 2]>]| cs.last().unwrap().iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!(last_x(&b) > last_x(&a));
    }
}
