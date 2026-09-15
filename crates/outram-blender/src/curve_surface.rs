// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Curve -> surface geometry. Follows the published behaviour of Blender's curve
// evaluation (source/blender/blenkernel/intern/curve_to_mesh.cc and
// displist.cc: bevel depth / bevel object / taper object / 2-D fill),
// github.com/blender/blender, GPL-2.0-or-later: sweep a cross-section along a
// spline, or fill a closed 2-D spline. Concepts only — no upstream source
// copied.
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

//! **Curve → surface geometry** (`op-hzs.54.35`, GH issue #37 §G). Depends on
//! [`crate::curve`].
//!
//! [`curve_to_mesh`] turns a [`Spline`] into geometry per [`CurveGeometry`]:
//!
//! - `bevel` — [`Bevel::Round`] sweeps a circle of `depth`, [`Bevel::Profile`]
//!   sweeps a custom 2-D cross-section, [`Bevel::None`] leaves a wire / fill.
//! - `taper` — an optional [`Spline`] whose height at parameter `t` scales the
//!   cross-section (a lathe taper object).
//! - `fill` — when there is no bevel and the spline is cyclic,
//!   [`FillMode::Full`] triangulates the outline (a flat cap).
//! - `caps` — close the ends of a swept open spline.

use crate::curve::Spline;
use crate::math::Vec3;
use crate::mesh::Mesh;

/// The cross-section swept along the spline.
#[derive(Debug, Clone)]
pub enum Bevel {
    /// No cross-section (wire, or a fill for a cyclic 2-D spline).
    None,
    /// A circle of the given radius, `segments` sides.
    Round { depth: f64, segments: usize },
    /// A custom cross-section, as `[x, y]` points in the spline's frame plane.
    Profile {
        section: Vec<[f64; 2]>,
        closed: bool,
    },
}

/// How a bevel-free cyclic spline is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    /// Not filled — leave a wire.
    None,
    /// Fill the outline once (a flat face).
    Full,
}

/// Options for [`curve_to_mesh`].
#[derive(Debug, Clone)]
pub struct CurveGeometry {
    /// The swept cross-section.
    pub bevel: Bevel,
    /// Optional taper spline — its `y` at parameter `t` scales the section.
    pub taper: Option<Spline>,
    /// Fill for a bevel-free cyclic spline.
    pub fill: FillMode,
    /// Cap the ends of a swept open spline.
    pub caps: bool,
}

impl Default for CurveGeometry {
    fn default() -> Self {
        CurveGeometry {
            bevel: Bevel::None,
            taper: None,
            fill: FillMode::None,
            caps: true,
        }
    }
}

/// Evaluate `spline` into a [`Mesh`] per `opts`.
pub fn curve_to_mesh(spline: &Spline, opts: &CurveGeometry) -> Mesh {
    let frames = spline.sample_with_frames();
    if frames.len() < 2 {
        return Mesh::new();
    }

    // Cross-section as [x, y] points (unit scale — the radius/taper multiply).
    let section: Vec<[f64; 2]> = match &opts.bevel {
        Bevel::None => Vec::new(),
        Bevel::Round { segments, .. } => {
            let n = (*segments).max(3);
            (0..n)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / n as f64;
                    [a.cos(), a.sin()]
                })
                .collect()
        }
        Bevel::Profile { section, .. } => section.clone(),
    };

    if section.is_empty() {
        return match opts.fill {
            FillMode::Full if spline.cyclic => {
                fill_outline(&frames.iter().map(|f| f.position).collect::<Vec<_>>())
            }
            _ => wire_mesh(
                &frames.iter().map(|f| f.position).collect::<Vec<_>>(),
                spline.cyclic,
            ),
        };
    }

    let section_scale = match &opts.bevel {
        Bevel::Round { depth, .. } => *depth,
        _ => 1.0,
    };
    let section_closed = match &opts.bevel {
        Bevel::Round { .. } => true,
        Bevel::Profile { closed, .. } => *closed,
        Bevel::None => false,
    };

    // Sweep: one ring of section points per frame.
    let mut positions: Vec<Vec3> = Vec::new();
    let m = frames.len();
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(m);
    for (fi, f) in frames.iter().enumerate() {
        let taper = opts
            .taper
            .as_ref()
            .map(|t| taper_scale(t, fi as f64 / (m - 1) as f64))
            .unwrap_or(1.0);
        let scale = section_scale * f.radius * taper;
        let binormal = f.tangent.cross(f.normal).normalize();
        let ring: Vec<usize> = section
            .iter()
            .map(|&[sx, sy]| {
                let p = f
                    .position
                    .add(f.normal.scale(sx * scale))
                    .add(binormal.scale(sy * scale));
                positions.push(p);
                positions.len() - 1
            })
            .collect();
        rings.push(ring);
    }

    let sn = section.len();
    let sides = if section_closed {
        sn
    } else {
        sn.saturating_sub(1)
    };
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let seg_rings: Vec<&Vec<usize>> = if spline.cyclic {
        rings.iter().chain(std::iter::once(&rings[0])).collect()
    } else {
        rings.iter().collect()
    };
    for w in seg_rings.windows(2) {
        let (r0, r1) = (w[0], w[1]);
        for i in 0..sides {
            let j = (i + 1) % sn;
            faces.push(vec![r0[i], r0[j], r1[j], r1[i]]);
        }
    }

    if opts.caps && !spline.cyclic && section_closed {
        // Fan-cap both ends.
        let c0 = ring_centroid(&rings[0], &positions);
        positions.push(c0);
        let ci0 = positions.len() - 1;
        for i in 0..sn {
            faces.push(vec![ci0, rings[0][(i + 1) % sn], rings[0][i]]);
        }
        let c1 = ring_centroid(&rings[m - 1], &positions);
        positions.push(c1);
        let ci1 = positions.len() - 1;
        for i in 0..sn {
            faces.push(vec![ci1, rings[m - 1][i], rings[m - 1][(i + 1) % sn]]);
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

// --- helpers ---

/// The taper spline's height (`y`) at parameter `t ∈ [0, 1]`.
fn taper_scale(taper: &Spline, t: f64) -> f64 {
    let pts = taper.sample();
    if pts.is_empty() {
        return 1.0;
    }
    let x = t * (pts.len() - 1) as f64;
    let a = x.floor() as usize;
    let b = (a + 1).min(pts.len() - 1);
    let f = x - a as f64;
    (pts[a].y + (pts[b].y - pts[a].y) * f).abs()
}

fn ring_centroid(ring: &[usize], positions: &[Vec3]) -> Vec3 {
    ring.iter()
        .fold(Vec3::ZERO, |acc, &i| acc.add(positions[i]))
        .scale(1.0 / ring.len().max(1) as f64)
}

/// A degenerate "mesh" recording the polyline as sliver triangles (so the
/// polygon-soup model keeps every vertex and edge).
fn wire_mesh(pts: &[Vec3], cyclic: bool) -> Mesh {
    let mut faces = Vec::new();
    let n = pts.len();
    let last = if cyclic { n } else { n - 1 };
    for i in 0..last {
        faces.push(vec![i, (i + 1) % n, i]);
    }
    Mesh::from_polygons(pts, &faces)
}

/// Triangulate a closed outline (ear clipping in the best-fit plane).
fn fill_outline(pts: &[Vec3]) -> Mesh {
    let n = pts.len();
    if n < 3 {
        return Mesh::new();
    }
    // Best-fit plane normal (Newell).
    let c = pts
        .iter()
        .fold(Vec3::ZERO, |a, &p| a.add(p))
        .scale(1.0 / n as f64);
    let mut nrm = Vec3::ZERO;
    for i in 0..n {
        nrm = nrm.add(pts[i].sub(c).cross(pts[(i + 1) % n].sub(c)));
    }
    if nrm.length() < 1e-12 {
        return Mesh::new();
    }
    let nrm = nrm.normalize();
    // 2-D coords.
    let up = if nrm.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let ex = up.cross(nrm).normalize();
    let ey = nrm.cross(ex);
    let p2: Vec<[f64; 2]> = pts
        .iter()
        .map(|&p| [p.sub(c).dot(ex), p.sub(c).dot(ey)])
        .collect();

    let mut idx: Vec<usize> = (0..n).collect();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let mut clipped = false;
        for k in 0..idx.len() {
            let (ia, ib, ic) = (
                idx[(k + idx.len() - 1) % idx.len()],
                idx[k],
                idx[(k + 1) % idx.len()],
            );
            if is_ear(&p2, ia, ib, ic, &idx) {
                faces.push(vec![ia, ib, ic]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break; // degenerate; bail with what we have + a fan
        }
    }
    if idx.len() == 3 {
        faces.push(vec![idx[0], idx[1], idx[2]]);
    }
    Mesh::from_polygons(pts, &faces)
}

fn is_ear(p: &[[f64; 2]], a: usize, b: usize, c: usize, poly: &[usize]) -> bool {
    let area = tri_area2(p[a], p[b], p[c]);
    if area <= 1e-12 {
        return false; // reflex or degenerate (assumes CCW winding)
    }
    for &q in poly {
        if q == a || q == b || q == c {
            continue;
        }
        if point_in_tri(p[q], p[a], p[b], p[c]) {
            return false;
        }
    }
    true
}

fn tri_area2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
}

fn point_in_tri(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let d1 = tri_area2(p, a, b);
    let d2 = tri_area2(p, b, c);
    let d3 = tri_area2(p, c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::SplineType;

    #[test]
    fn round_bevel_makes_a_tube() {
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0)]);
        let m = curve_to_mesh(
            &s,
            &CurveGeometry {
                bevel: Bevel::Round {
                    depth: 0.5,
                    segments: 8,
                },
                caps: true,
                ..Default::default()
            },
        );
        // 1 segment × 8 sides + 2 caps (8 tris each).
        assert!(m.face_count() >= 8 + 16);
        // Radius ≈ 0.5 around the axis.
        for i in 0..m.vertex_count() {
            let p = m.vertex(crate::mesh::VertexId(i)).unwrap().position;
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!(r <= 0.51);
        }
    }

    #[test]
    fn taper_shrinks_the_tube_along_its_length() {
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 6.0)]);
        let taper = Spline::poly(&[Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.1, 0.0)]);
        let m = curve_to_mesh(
            &s,
            &CurveGeometry {
                bevel: Bevel::Round {
                    depth: 0.5,
                    segments: 8,
                },
                taper: Some(taper),
                caps: false,
                ..Default::default()
            },
        );
        let r_at = |want_z: f64| {
            (0..m.vertex_count())
                .filter(|&i| {
                    (m.vertex(crate::mesh::VertexId(i)).unwrap().position.z - want_z).abs() < 0.1
                })
                .map(|i| {
                    let p = m.vertex(crate::mesh::VertexId(i)).unwrap().position;
                    (p.x * p.x + p.y * p.y).sqrt()
                })
                .fold(0.0_f64, f64::max)
        };
        assert!(r_at(0.0) > 0.4);
        assert!(r_at(6.0) < 0.1);
    }

    #[test]
    fn custom_profile_sweep() {
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0)]);
        // An L-shaped open profile.
        let section = vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.3],
            [0.3, 0.3],
            [0.3, 1.0],
            [0.0, 1.0],
        ];
        let m = curve_to_mesh(
            &s,
            &CurveGeometry {
                bevel: Bevel::Profile {
                    section: section.clone(),
                    closed: true,
                },
                caps: false,
                ..Default::default()
            },
        );
        assert_eq!(
            m.face_count(),
            section.len(),
            "one strip quad per profile side"
        );
    }

    #[test]
    fn fill_a_closed_2d_curve() {
        let mut s = Spline::poly(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ]);
        s.set_type(SplineType::Poly);
        s.cyclic = true;
        let m = curve_to_mesh(
            &s,
            &CurveGeometry {
                fill: FillMode::Full,
                ..Default::default()
            },
        );
        // A quad → 2 ear-clipped triangles.
        assert_eq!(m.face_count(), 2);
        assert_eq!(m.euler_characteristic(), 1);
    }

    #[test]
    fn no_bevel_no_fill_is_a_wire() {
        let s = Spline::poly(&[
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
        ]);
        let m = curve_to_mesh(&s, &CurveGeometry::default());
        assert_eq!(m.vertex_count(), 3);
    }
}
