// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. It introduces no meshing algorithm:
// it turns a wall size and a growth rate into the per-point sizes TetGen's
// `-m` reads (`crate::tetgen::sizing`, the port of TetGen 1.6.0's `.mtr`
// sizes, AGPL-3.0-or-later, Hang Si / WIAS).

//! **Wall grading for the TetGen tet source** (gh:#1825): TetGen's tetrahedra
//! shrink toward every wall, not only where the surface triangles are small.
//!
//! # What TetGen offers, and why sizes inside the volume are needed
//!
//! TetGen's own sizing is `-m` with a `.mtr` size per input point
//! ([`crate::tetgen::sizing`]): Delaunay refinement splits a tetrahedron
//! whose circumradius exceeds a vertex's size, and every new point takes
//! the P1 interpolation of the sizes of the tetrahedron it lands in. The
//! sizes are interpolated on the **current mesh**, and a closed surface's
//! first tetrahedralisation has only surface vertices: with sizes on the
//! wall points alone, every tetrahedron spanning wall to wall interpolates
//! the wall size, and the whole volume is refined to it. Measured with the
//! upstream binary on the Mesh Studio pipe (wall size 0.05 m, cap centres
//! 0.25 m), the mean edge was 0.064 m at the wall and 0.068 m on the axis:
//! uniform, not graded. With twelve sized points on the axis it was 0.065 m
//! at the wall and 0.21 m on the axis.
//!
//! So the PLC gets **seed points** inside the volume as well (upstream
//! accepts points that lie on no facet and keeps them as mesh vertices), on
//! a cubic lattice of spacing `interior_size` (no closer than the largest
//! size, so that `meshcoarsening`, which removes an interior vertex whose
//! shortest edge is below its size, keeps nearly all of them), each moved
//! by a fixed pseudo-random offset of up to 0.2 spacing per axis (a
//! regular lattice showed in the first images as rows of vertices), and
//! kept when at least half a spacing from every surface triangle. Every point,
//! surface and seed, gets the size [`WallGrade::size_at`] of its distance to
//! the nearest wall triangle: `wall_size + (growth - 1) d`, capped at
//! `interior_size`, the continuous form of cells growing by `growth` per
//! cell away from the wall.
//!
//! Upstream's other sizing routes were checked and not taken: the
//! background mesh (`-m` with `.b.node` / `.b.ele`) needs `-r`'s
//! `reconstructmesh` and `scout_point` ported, and `in->tetunsuitable` is a
//! library callback the unmodified binary cannot check against. A finer
//! surface triangulation at the walls alone would grade only the first
//! tetrahedra from the wall.
//!
//! **Sizes are TetGen's**, a bound on the circumradius of the tetrahedra at
//! a point: the edges come out about 1.25 times the size near a wall
//! (measured, the gh:#1825 V&V record).
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research
//! and V&V only, per the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::math::Vec3;
use alloc::vec::Vec;

/// A size field graded from the walls (gh:#1825): `wall_size` at a wall,
/// growing linearly with the distance `d` from the nearest wall by
/// `growth - 1` per metre of distance per metre of size, capped at
/// `interior_size`. Sizes are TetGen's `-m` sizes, in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallGrade {
    /// The size at a wall \[m\]. Must be > 0.
    pub wall_size: f64,
    /// The cell-to-cell growth ratio away from the wall (dimensionless,
    /// >= 1): the size grows by `growth - 1` times the distance.
    pub growth: f64,
    /// The size far from the walls \[m\]: the cap. Must be >= `wall_size`.
    pub interior_size: f64,
}

impl WallGrade {
    /// The growth ratio when none is asked for: 1.2, the common
    /// cell-to-cell expansion of a wall-resolved CFD mesh (a ratio of 1.3
    /// or more is usually called too fast).
    pub const DEFAULT_GROWTH: f64 = 1.2;

    /// The size at distance `d` \[m\] from the nearest wall.
    #[must_use]
    pub fn size_at(&self, d: f64) -> f64 {
        let s = self.wall_size + (self.growth - 1.0) * d.max(0.0);
        if s < self.interior_size {
            s
        } else {
            self.interior_size
        }
    }

    /// `Err` with the reason when a field is out of range.
    ///
    /// # Errors
    /// A non-positive or non-finite size, `growth < 1`, or an interior
    /// size below the wall size.
    pub fn check(&self) -> Result<(), String> {
        if !(self.wall_size > 0.0 && self.wall_size.is_finite()) {
            return Err(alloc::format!(
                "wall size must be > 0 (got {})",
                self.wall_size
            ));
        }
        if !(self.growth >= 1.0 && self.growth.is_finite()) {
            return Err(alloc::format!("growth must be >= 1 (got {})", self.growth));
        }
        if !(self.interior_size >= self.wall_size && self.interior_size.is_finite()) {
            return Err(alloc::format!(
                "interior size {} must be >= the wall size {}",
                self.interior_size,
                self.wall_size
            ));
        }
        Ok(())
    }
}

/// A number in `[0, 1)` from `key` (SplitMix64's finaliser): the fixed
/// jitter of the seed lattice.
fn unit_hash(key: u64) -> f64 {
    let mut z = key.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// The distance from `p` to the nearest of `tris` whose `keep` is true,
/// `f64::INFINITY` when there is none.
fn distance_to(p: Vec3, points: &[Vec3], tris: &[[usize; 3]], keep: &[bool]) -> f64 {
    let mut best = f64::INFINITY;
    for (t, &k) in tris.iter().zip(keep) {
        if !k {
            continue;
        }
        let q = crate::snap::closest_point_on_triangle(p, points[t[0]], points[t[1]], points[t[2]]);
        let d = q.sub(p).length();
        if d < best {
            best = d;
        }
    }
    best
}

/// The seed points and the sizes of a wall-graded TetGen run: `seeds` (on
/// the lattice described in the module docs, inside the closed surface
/// `points` / `tris`, in lattice order: x fastest, then y, then z) and one
/// size per point of `points` followed by one per seed.
///
/// `is_wall[i]` says whether triangle `i` is a wall. A surface point's
/// distance is to the nearest wall triangle (0 on a wall); with no wall
/// triangle every size is `interior_size`.
pub fn wall_graded_sizes(
    points: &[Vec3],
    tris: &[[usize; 3]],
    is_wall: &[bool],
    grade: &WallGrade,
) -> (Vec<Vec3>, Vec<f64>) {
    let all = alloc::vec![true; tris.len()];
    let spacing = grade.interior_size;
    let mut seeds = Vec::new();
    if !points.is_empty() && spacing > 0.0 {
        let (mut lo, mut hi) = (points[0], points[0]);
        for p in points {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        let n = |a: f64, b: f64| ((b - a) / spacing).floor() as usize + 1;
        let (nx, ny, nz) = (n(lo.x, hi.x), n(lo.y, hi.y), n(lo.z, hi.z));
        // Centre the lattice in the bounding box.
        let off = |a: f64, b: f64, k: usize| a + 0.5 * ((b - a) - (k - 1) as f64 * spacing);
        let (ox, oy, oz) = (
            off(lo.x, hi.x, nx),
            off(lo.y, hi.y, ny),
            off(lo.z, hi.z, nz),
        );
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    // Each lattice point moved by up to 0.2 spacing per axis
                    //   (a fixed hash of its index), so that the lattice
                    //   does not show in the mesh as rows of vertices.
                    let jit = |axis: u64| {
                        let key = ((k as u64) << 42) ^ ((j as u64) << 21) ^ (i as u64);
                        0.2 * spacing
                            * (2.0 * unit_hash(key.wrapping_mul(3).wrapping_add(axis)) - 1.0)
                    };
                    let p = Vec3::new(
                        ox + i as f64 * spacing + jit(0),
                        oy + j as f64 * spacing + jit(1),
                        oz + k as f64 * spacing + jit(2),
                    );
                    if crate::carve::inside(p, points, tris)
                        && distance_to(p, points, tris, &all) >= 0.5 * spacing
                    {
                        seeds.push(p);
                    }
                }
            }
        }
    }
    let sizes = points
        .iter()
        .chain(seeds.iter())
        .map(|&p| grade.size_at(distance_to(p, points, tris, is_wall)))
        .collect();
    (seeds, sizes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::box_surface;

    /// The size law: the wall size at a wall, linear growth, the cap.
    #[test]
    fn size_grows_linearly_from_the_wall_to_the_cap() {
        let g = WallGrade {
            wall_size: 0.1,
            growth: 1.2,
            interior_size: 0.3,
        };
        assert_eq!(g.size_at(0.0), 0.1);
        assert!((g.size_at(0.5) - 0.2).abs() < 1e-15);
        assert_eq!(g.size_at(5.0), 0.3);
        assert_eq!(g.size_at(-1.0), 0.1);
        assert!(g.check().is_ok());
        for bad in [
            WallGrade {
                wall_size: 0.0,
                ..g
            },
            WallGrade { growth: 0.9, ..g },
            WallGrade {
                interior_size: 0.05,
                ..g
            },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
        }
    }

    /// In a 2 m box with its bottom face the only wall: every seed is
    /// inside, at least half a spacing from every face and within the
    /// jitter (0.2 spacing per axis) of its lattice point; the box's bottom
    /// corners get the wall size and its top corners the cap; every seed's
    /// size is that of its height.
    #[test]
    fn seeds_lie_inside_and_take_the_size_of_their_wall_distance() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let is_wall: Vec<bool> = t.iter().map(|q| q.iter().all(|&v| p[v].z == 0.0)).collect();
        assert_eq!(is_wall.iter().filter(|&&w| w).count(), 2);
        let g = WallGrade {
            wall_size: 0.05,
            growth: 1.25,
            interior_size: 0.5,
        };
        let (seeds, sizes) = wall_graded_sizes(&p, &t, &is_wall, &g);
        assert_eq!(sizes.len(), p.len() + seeds.len());
        // Spacing 0.5: a 5^3 lattice from 0 to 2, of which the 3^3 at
        //   0.5..1.5 (moved by at most 0.1) are at least 0.25 from every face.
        assert_eq!(seeds.len(), 27, "{seeds:?}");
        let mut moved = 0;
        for (i, s) in seeds.iter().enumerate() {
            for c in [s.x, s.y, s.z] {
                assert!((0.4..=1.6).contains(&c), "{s:?}");
                let off = c - 0.5 * (c / 0.5).round();
                assert!(off.abs() <= 0.1 + 1e-12, "{s:?}");
                moved += usize::from(off != 0.0);
            }
            assert!((sizes[p.len() + i] - g.size_at(s.z)).abs() < 1e-12);
        }
        assert!(moved > 70, "the lattice is jittered ({moved} of 81)");
        for (i, q) in p.iter().enumerate() {
            let want = if q.z == 0.0 { 0.05 } else { 0.5 };
            assert!((sizes[i] - want).abs() < 1e-12, "{q:?} {}", sizes[i]);
        }
    }

    /// Mean edge lengths of a TetGen run of the 1 m x 6 m pipe (the Mesh
    /// Studio pipe at size 2: 32 sides, regions inlet / outlet / walls):
    /// of the tetrahedra with a face on the wall, and of the interior ones
    /// (centroid more than 0.4 m from the wall and the caps, no vertex on
    /// the boundary). Also the smallest tetrahedron volume and the total.
    fn pipe_edges(grade: Option<WallGrade>) -> (f64, f64, f64, f64, usize) {
        use crate::pipeline::tests::pipe_surface;
        use crate::tetgen::tetrahedralize_plc;
        let (p, t, r) = pipe_surface(1.0, 6.0, 32);
        // `-a`: the regular tetrahedron of the interior size (graded) or of
        //   the cell size 0.25 m (not), as Mesh Studio derives it.
        let h: f64 = grade.map_or(0.25, |g| g.interior_size);
        let v = h * h * h / (6.0 * core::f64::consts::SQRT_2);
        let src = super::super::TetSource::tetgen(1.414, Some(v)).with_wall_grade(grade);
        let run =
            super::super::tetgen_plc(&p, &t, Some(&r), "walls", &["walls".into()], src).unwrap();
        let out = tetrahedralize_plc(&run.plc, &run.behavior)
            .unwrap()
            .output
            .unwrap();
        let wall = r.names.iter().position(|n| n == "walls").unwrap() as i32;
        let f0 = out.firstindex;
        let key = |mut k: [i32; 3]| {
            k.sort_unstable();
            k
        };
        let faces = out.faces.as_ref().unwrap();
        let mut wall_faces = alloc::collections::BTreeSet::new();
        let mut bnd = alloc::collections::BTreeSet::new();
        for (f, &m) in faces.iter().zip(&out.face_markers) {
            bnd.extend(f.iter().copied());
            if m == wall {
                wall_faces.insert(key(*f));
            }
        }
        let x = |i: i32| out.points[(i - f0) as usize];
        let (mut sw, mut nw, mut si, mut ni) = (0.0, 0usize, 0.0, 0usize);
        let (mut vmin, mut vtot) = (f64::INFINITY, 0.0);
        for tt in &out.tets {
            let q = [x(tt[0]), x(tt[1]), x(tt[2]), x(tt[3])];
            let d = |a: [f64; 3], b: [f64; 3]| {
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            };
            let mut mel = 0.0;
            for i in 0..4 {
                for j in i + 1..4 {
                    mel += d(q[i], q[j]) / 6.0;
                }
            }
            let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let (a, b, c) = (sub(q[1], q[0]), sub(q[2], q[0]), sub(q[3], q[0]));
            // TetGen's [a, b, c, d] has orient3d < 0, so this is positive.
            let vol = (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
            vmin = vmin.min(vol);
            vtot += vol;
            let on_wall = (0..4).any(|k| {
                wall_faces.contains(&key([tt[(k + 1) % 4], tt[(k + 2) % 4], tt[(k + 3) % 4]]))
            });
            let cen = [
                (q[0][0] + q[1][0] + q[2][0] + q[3][0]) / 4.0,
                (q[0][1] + q[1][1] + q[2][1] + q[3][1]) / 4.0,
                (q[0][2] + q[1][2] + q[2][2] + q[3][2]) / 4.0,
            ];
            let dw = 1.0 - (cen[0] * cen[0] + cen[1] * cen[1]).sqrt();
            if on_wall {
                sw += mel;
                nw += 1;
            } else if dw > 0.4
                && cen[2] > 0.4
                && cen[2] < 5.6
                && !tt.iter().any(|v| bnd.contains(v))
            {
                si += mel;
                ni += 1;
            }
        }
        (sw / nw as f64, si / ni as f64, vmin, vtot, out.tets.len())
    }

    /// **Property (gh:#1825): the tetrahedra shrink toward the wall.** The
    /// Mesh Studio pipe (1 m radius, 6 m, 32 sides, cell size h = 0.25 m),
    /// TetGen `-pq1.414a<V>`: ungraded with V the regular tetrahedron of h
    /// (the page's default), and graded as `?grade=on` (walls h/2, growth
    /// 1.2, 2h inside, V the regular tetrahedron of 2h, `-m`).
    ///
    /// Checked, from TetGen's semantics rather than from a first run:
    /// every tetrahedron positive and the volume the same to 1e-9 (the
    /// domain is not changed); the graded wall tetrahedra's mean edge at
    /// most 2 x the wall size (a size bounds the circumradius `R`, and no
    /// edge exceeds `2R`) and at least 1.25 times smaller than ungraded;
    /// graded, the interior mean edge at least 1.5 times the wall one (the
    /// sizes ask 0.325 / 0.125 = 2.6 on the axis), ungraded within 10 %
    /// of it.
    ///
    /// **Results (2026-11-26, release):** ungraded 17 988 tetrahedra, wall
    /// mean edge 0.219 m, interior 0.217 m (interior / wall 0.99); graded
    /// 26 979 tetrahedra, wall 0.154 m (1.23 x the wall size 0.125 m),
    /// interior 0.254 m (interior / wall 1.65).
    #[test]
    fn tetgen_tets_shrink_toward_the_walls_when_graded() {
        let h = 0.25;
        let g = WallGrade {
            wall_size: 0.5 * h,
            growth: WallGrade::DEFAULT_GROWTH,
            interior_size: 2.0 * h,
        };
        let (w0, i0, vmin0, vol0, n0) = pipe_edges(None);
        let (w1, i1, vmin1, vol1, n1) = pipe_edges(Some(g));
        std::println!(
            "ungraded: {n0} tets, wall {w0:.4} interior {i0:.4} ratio {:.3}; \
             graded: {n1} tets, wall {w1:.4} interior {i1:.4} ratio {:.3}",
            i0 / w0,
            i1 / w1
        );
        assert!(vmin0 > 0.0 && vmin1 > 0.0, "{vmin0} {vmin1}");
        assert!((vol1 - vol0).abs() < 1e-9 * vol0, "{vol0} {vol1}");
        assert!(w1 <= 2.0 * g.wall_size, "{w1}");
        assert!(w0 / w1 >= 1.25, "{w0} {w1}");
        assert!(i1 / w1 >= 1.5, "{i1} {w1}");
        assert!((i0 / w0 - 1.0).abs() < 0.1, "{i0} {w0}");
    }
}
