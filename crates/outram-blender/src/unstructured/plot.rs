// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! **Drawing a neutral mesh** — the geometry-drawing HARD RULE (crate
//! `CLAUDE.md`) for FV and FE meshes: "for meshes, plot the mesh itself
//! (cells, patches, zones)".
//!
//! A slice plane cuts the mesh; every cell's cut is filled with its zone
//! colour (or its own colour), and every **face** the plane crosses is drawn
//! as a black line, so the picture shows the cells a solver integrates over,
//! not a resampling of them. A cell's cut is the plane section of its
//! **bounding triangles** ([`UnstructuredMesh::for_each_bounding_triangle`]),
//! filled by even-odd scanline, so what is filled is exactly the region point
//! location assigns to that cell, convex or not. A 2-D mesh is drawn as it is
//! (basis [`PlotBasis::Xy`]).
//!
//! Coordinates are in **cm** whatever the mesh's unit (they are scaled), so
//! the frame and ticks of [`annotate_slice`] read correctly.

use crate::csg::plot::annotate::{annotate_slice, LegendEntry};
use crate::csg::plot::colour::{default_colours, Rgb, BLACK, WHITE};
use crate::csg::plot::image::ImageData;
use crate::csg::plot::slice::{PlotBasis, SlicePlot};
use crate::csg::position::Position;

use super::mesh::UnstructuredMesh;

/// What the fill colour of a cell means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshColourBy {
    /// One colour per cell zone (cells in no zone are light grey). The legend
    /// lists the zones.
    Zone,
    /// One colour per cell, to see individual cells. No legend.
    Cell,
}

/// A slice of a neutral mesh to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSlice {
    /// Orientation (horizontal, vertical axes).
    pub basis: PlotBasis,
    /// Slice centre \[cm\]; its normal-axis component is the cut height.
    pub origin: [f64; 3],
    /// Full widths along the horizontal and vertical axes \[cm\].
    pub width: [f64; 2],
    /// Pixels across and down.
    pub pixels: [usize; 2],
    /// Fill colouring.
    pub colour_by: MeshColourBy,
}

impl MeshSlice {
    /// A slice through the middle of the mesh's bounding box, framing it with
    /// a 5 % margin, `pixels` wide (height in proportion).
    pub fn framing(mesh: &UnstructuredMesh, basis: PlotBasis, pixels: usize, colour_by: MeshColourBy) -> Self {
        let s = mesh.unit().cm_per_unit();
        let [lo, hi] = mesh.bounds();
        let (a, b) = basis.axes();
        let centre = [0.5 * (lo[0] + hi[0]) * s, 0.5 * (lo[1] + hi[1]) * s, 0.5 * (lo[2] + hi[2]) * s];
        let w = [((hi[a] - lo[a]) * s * 1.1).max(1e-9), ((hi[b] - lo[b]) * s * 1.1).max(1e-9)];
        let py = ((pixels as f64) * w[1] / w[0]).round().max(1.0) as usize;
        Self { basis, origin: centre, width: w, pixels: [pixels, py], colour_by }
    }

    /// Continuous pixel coordinates of an in-plane point `(u, v)` \[cm\]: the
    /// inverse of `SlicePlot::pixel_centre` (pixel `(x, y)` has its centre at
    /// `(x, y)` here).
    fn to_pixel(&self, u: f64, v: f64) -> (f64, f64) {
        let (a, b) = self.basis.axes();
        let lo_u = self.origin[a] - 0.5 * self.width[0];
        let hi_v = self.origin[b] + 0.5 * self.width[1];
        let du = self.width[0] / self.pixels[0] as f64;
        let dv = self.width[1] / self.pixels[1] as f64;
        ((u - lo_u) / du - 0.5, (hi_v - v) / dv - 0.5)
    }
}

/// Fill the region enclosed by the closed set of segments `segs` (pixel
/// coordinates) by pixel-centre sampling with the even-odd rule: along each
/// pixel row, sort the crossings and fill between alternate pairs. Works for
/// non-convex and multiply connected sections.
fn fill_even_odd(img: &mut ImageData, segs: &[((f64, f64), (f64, f64))], colour: Rgb) {
    if segs.is_empty() {
        return;
    }
    let (mut y0, mut y1) = (f64::MAX, f64::MIN);
    for &(a, b) in segs {
        y0 = y0.min(a.1.min(b.1));
        y1 = y1.max(a.1.max(b.1));
    }
    let ya = y0.ceil().max(0.0) as i64;
    let yb = (y1.floor() as i64).min(img.height as i64 - 1);
    let mut xs: Vec<f64> = Vec::new();
    for y in ya..=yb {
        let py = y as f64;
        xs.clear();
        for &(a, b) in segs {
            // Half-open in y so a vertex shared by two segments counts once.
            if (a.1 <= py && b.1 > py) || (b.1 <= py && a.1 > py) {
                xs.push(a.0 + (py - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.chunks_exact(2) {
            let xa = pair[0].ceil().max(0.0) as i64;
            let xb = (pair[1].floor() as i64).min(img.width as i64 - 1);
            for x in xa..=xb {
                img.set(x as usize, y as usize, colour);
            }
        }
    }
}

/// Draw a 1-pixel line (DDA) between two pixel-space points.
fn draw_line(img: &mut ImageData, a: (f64, f64), b: (f64, f64), colour: Rgb) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = (a.0 + (b.0 - a.0) * t).round();
        let y = (a.1 + (b.1 - a.1) * t).round();
        if x >= 0.0 && y >= 0.0 && (x as usize) < img.width && (y as usize) < img.height {
            img.set(x as usize, y as usize, colour);
        }
    }
}

/// Points where the plane `x[n] = s` cuts the segments between `pts`
/// (every pair), plus any point lying on it.
fn plane_cut(pts: &[[f64; 3]], n: usize, s: f64) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for i in 0..pts.len() {
        let di = pts[i][n] - s;
        if di == 0.0 {
            out.push(pts[i]);
        }
        for j in (i + 1)..pts.len() {
            let dj = pts[j][n] - s;
            if (di < 0.0 && dj > 0.0) || (di > 0.0 && dj < 0.0) {
                let t = di / (di - dj);
                out.push([
                    pts[i][0] + t * (pts[j][0] - pts[i][0]),
                    pts[i][1] + t * (pts[j][1] - pts[i][1]),
                    pts[i][2] + t * (pts[j][2] - pts[i][2]),
                ]);
            }
        }
    }
    out
}

/// Rasterise `slice` of `mesh` (no frame): cell cuts filled, crossed faces
/// drawn in black, background white. Returns the image and the zone legend.
pub fn render_mesh_slice(mesh: &UnstructuredMesh, slice: &MeshSlice) -> (ImageData, Vec<LegendEntry>) {
    let s = mesh.unit().cm_per_unit();
    let (a, b) = slice.basis.axes();
    let n = 3 - a - b;
    let cut = slice.origin[n];
    let mut img = ImageData::filled(slice.pixels[0], slice.pixels[1], WHITE);
    let mut seed = 1u64;
    let grey = Rgb::new(200, 200, 200);
    let (colours, legend) = match slice.colour_by {
        MeshColourBy::Zone => {
            let c = default_colours(mesh.zones().len(), &mut seed);
            let legend = mesh.zones().iter().zip(&c).map(|(z, &col)| LegendEntry::new(col, z.name.clone())).collect();
            (c, legend)
        }
        MeshColourBy::Cell => (default_colours(mesh.n_cells(), &mut seed), Vec::new()),
    };
    let fill_of = |c: usize| match slice.colour_by {
        MeshColourBy::Zone => mesh.zone_of(c).map_or(grey, |z| colours[z]),
        MeshColourBy::Cell => colours[c],
    };
    let px = |p: [f64; 3]| slice.to_pixel(p[a] * s, p[b] * s);

    if mesh.dim() == 2 {
        let mut segs = Vec::new();
        for c in 0..mesh.n_cells() {
            segs.clear();
            mesh.for_each_bounding_triangle(c, |t| segs.push((px(t[0]), px(t[1]))));
            fill_even_odd(&mut img, &segs, fill_of(c));
        }
        for f in 0..mesh.n_faces() {
            let v = mesh.face(f);
            draw_line(&mut img, px(mesh.points()[v[0]]), px(mesh.points()[v[1]]), BLACK);
        }
        return (img, legend);
    }

    let cut_unit = cut / s;
    let mut segs = Vec::new();
    for c in 0..mesh.n_cells() {
        let bb = mesh.cell_bounds(c);
        if bb[0][n] > cut_unit || bb[1][n] < cut_unit {
            continue;
        }
        segs.clear();
        mesh.for_each_bounding_triangle(c, |t| {
            let pts = plane_cut(&t, n, cut_unit);
            if pts.len() >= 2 {
                segs.push((px(pts[0]), px(pts[pts.len() - 1])));
            }
        });
        fill_even_odd(&mut img, &segs, fill_of(c));
    }
    for f in 0..mesh.n_faces() {
        let verts = mesh.face(f);
        let fc = mesh.face_centre(f);
        let k = verts.len();
        for i in 0..k {
            let tri = [fc, mesh.points()[verts[i]], mesh.points()[verts[(i + 1) % k]]];
            let pts = plane_cut(&tri, n, cut_unit);
            if pts.len() >= 2 {
                draw_line(&mut img, px(pts[0]), px(pts[pts.len() - 1]), BLACK);
            }
        }
    }
    (img, legend)
}

/// [`render_mesh_slice`] framed with a title, a zone legend and cm ticks
/// ([`annotate_slice`]).
pub fn render_mesh_slice_annotated(mesh: &UnstructuredMesh, slice: &MeshSlice, title: &str) -> ImageData {
    let (img, legend) = render_mesh_slice(mesh, slice);
    let o = slice.origin;
    let frame = SlicePlot::new(slice.basis, Position::new(o[0], o[1], o[2]), slice.width, slice.pixels);
    annotate_slice(&img, &frame, title, &legend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unstructured::one_d::one_d_column;
    use crate::unstructured::LengthUnit;

    /// A slice through the middle of a 1-D column fills the column's
    /// rectangle and leaves the margin white.
    #[test]
    fn column_slice_is_filled_inside_and_white_outside() {
        let m = one_d_column(LengthUnit::Centimetre, 10.0, 4.0, 5).unwrap();
        let sl = MeshSlice::framing(&m, PlotBasis::Xz, 220, MeshColourBy::Cell);
        let (img, _) = render_mesh_slice(&m, &sl);
        assert_eq!(img.get(0, 0), WHITE);
        let (cx, cy) = sl.to_pixel(1.0, 0.5);
        assert_ne!(img.get(cx.round() as usize, cy.round() as usize), WHITE);
    }
}
