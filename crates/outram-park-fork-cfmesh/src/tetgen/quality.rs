// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx` `qualitystatistics()`, the "Mesh
// quality statistics" block `-V` prints):
//   TetGen — A Quality Tetrahedral Mesh Generator and 3D Delaunay Triangulator
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0)
//   Copyright (C) 2002-2020 Hang Si, Weierstrass Institute for Applied
//   Analysis and Stochastics (WIAS), Berlin.
//   Licence: GNU Affero General Public License v3.0 or later.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! The mesh quality statistics `tetgen -V` prints (stage 6, gh:#1795):
//! [`TetgenMesh::qualitystatistics`] returns the same text, and
//! [`QualityStatistics`] the numbers, including the largest radius-edge
//! ratio upstream computes but does not print.
//!
//! **A pointer comparison is emulated.** Upstream takes a face's angles
//! from the tetrahedron at the lower **address** (`tetloop.tet <
//! neightet.tet`); the same face is counted once either way, but its vertex
//! order (and so the last bits of an angle near a bin edge) follows that
//! choice. `tet_address_below` models the addresses of upstream's pool
//! blocks on Linux / glibc (see there); on another allocator upstream itself
//! may print slightly different face-angle counts.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::mesh::{TetgenMesh, TriFace, DUMMYPOINT};
use alloc::string::String;
use core::f64::consts::PI;
use core::fmt::Write;

#[inline]
fn dot(v1: &[f64; 3], v2: &[f64; 3]) -> f64 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}

/// C's `%.<p>g` (glibc): `p` significant digits, fixed notation for
/// decimal exponents `-4 ..= p - 1`, trailing zeros removed, a two-digit
/// exponent otherwise. (`super::output::fmt_g17` is this at 17.)
pub fn fmt_g(x: f64, p: usize) -> String {
    let p = if p == 0 { 1 } else { p } as i32;
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    if !x.is_finite() {
        return super::output::fmt_g17(x);
    }
    let e = alloc::format!("{:.*e}", (p - 1) as usize, x);
    let (mant, exp) = e.split_once('e').unwrap_or((e.as_str(), "0"));
    let xexp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').into()
        } else {
            s.into()
        }
    };
    if xexp < -4 || xexp >= p {
        let sign = if xexp < 0 { '-' } else { '+' };
        alloc::format!("{}e{sign}{:02}", strip(mant), xexp.abs())
    } else {
        strip(&alloc::format!("{:.*}", (p - 1 - xexp) as usize, x))
    }
}

/// The numbers of `qualitystatistics()`.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityStatistics {
    /// Smallest and largest tetrahedron volume.
    pub volume: [f64; 2],
    /// Shortest and longest edge.
    pub edge: [f64; 2],
    /// Smallest and largest aspect ratio (longest edge over smallest
    /// height).
    pub aspect_ratio: [f64; 2],
    /// Smallest and largest radius-edge ratio (circumradius over shortest
    /// edge; upstream computes it, does not print it).
    pub radius_edge_ratio: [f64; 2],
    /// Smallest and largest face angle (degrees).
    pub face_angle: [f64; 2],
    /// Smallest and largest dihedral angle (degrees).
    pub dihedral: [f64; 2],
    /// The aspect-ratio histogram (`aspecttable[0..12]`).
    pub aspect_table: [u64; 12],
    /// The face-angle histogram (10-degree bins).
    pub face_angle_table: [u64; 18],
    /// The dihedral-angle histogram (upstream's 18 bins).
    pub dihedral_table: [u64; 18],
}

/// `aspectratiotable`.
const ASPECT: [f64; 12] = [
    1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 10.0, 15.0, 25.0, 50.0, 100.0, 0.0,
];
/// `radiusratiotable`.
const RADIUS: [f64; 12] = [
    0.707, 1.0, 1.1, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5, 3.0, 10.0, 0.0,
];

impl TetgenMesh {
    /// `a < b` for two tetrahedron **pointers** upstream (`qualitystatistics`
    /// compares them): within a pool block by slot; across blocks the later
    /// block lies lower. A block of 8188 tetrahedra is well above glibc's
    /// `mmap` threshold, so each block is a fresh `mmap`, and Linux places
    /// each new mapping below the last (checked against upstream's `-V`
    /// histograms on the 2- and 4-block Mesh Studio meshes, which differ
    /// by two face angles with plain slot order).
    fn tet_address_below(&self, a: usize, b: usize) -> bool {
        let n = self.tetpool.itemsperblock;
        let (ba, bb) = (a / n, b / n);
        if ba != bb {
            ba > bb
        } else {
            a < b
        }
    }

    /// The quality numbers of every (non-hull) tetrahedron, as
    /// `qualitystatistics()` computes them.
    pub fn quality_numbers(&mut self) -> QualityStatistics {
        let mut aspecttable = [0u64; 12];
        let mut radiustable = [0u64; 12];
        let mut dihedangletable = [0u64; 18];
        let mut faceangletable = [0u64; 18];

        let mut minaltitude = self.xmax - self.xmin + self.ymax - self.ymin + self.zmax - self.zmin;
        minaltitude *= minaltitude;
        let mut shortest = minaltitude;
        let mut longest = 0.0f64;
        let mut smallestvolume = minaltitude;
        let mut biggestvolume = 0.0f64;
        let (mut smallestratio, mut smallestradiusratio) = (1e16f64, 1e16f64);
        let (mut biggestratio, mut biggestradiusratio) = (0.0f64, 0.0f64);
        let (mut smallestdiangle, mut smallestfaangle) = (180.0f64, 180.0f64);
        let (mut biggestdiangle, mut biggestfaangle) = (0.0f64, 0.0f64);

        self.tetpool.traversalinit();
        while let Some(t) = self.tetrahedrontraverse() {
            let pv = self.tets[t].v;
            let p = [
                self.points[pv[0]].xyz,
                self.points[pv[1]].xyz,
                self.points[pv[2]].xyz,
                self.points[pv[3]].xyz,
            ];
            let tetvol = self.pred.orient3dfast(&p[1], &p[0], &p[2], &p[3]) / 6.0;
            if tetvol < smallestvolume {
                smallestvolume = tetvol;
            }
            if tetvol > biggestvolume {
                biggestvolume = tetvol;
            }
            let sub = |a: &[f64; 3], b: &[f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let v = [
                sub(&p[0], &p[3]),
                sub(&p[1], &p[3]),
                sub(&p[2], &p[3]),
                sub(&p[1], &p[0]),
                sub(&p[2], &p[1]),
                sub(&p[0], &p[2]),
            ];
            let mut edgelength = [0.0f64; 6];
            for i in 0..6 {
                edgelength[i] = dot(&v[i], &v[i]);
            }
            let (mut shortlen, mut longlen) = (0.0f64, 0.0f64);
            for (i, &e) in edgelength.iter().enumerate() {
                if i == 0 {
                    shortlen = e;
                    longlen = e;
                } else {
                    shortlen = if e < shortlen { e } else { shortlen };
                    longlen = if e > longlen { e } else { longlen };
                }
                if e > longest {
                    longest = e;
                }
                if e < shortest {
                    shortest = e;
                }
            }

            let mut a = [[0.0f64; 4]; 4];
            for j in 0..3 {
                a[j][..3].copy_from_slice(&v[j]);
            }
            let mut indx = [0usize; 4];
            let mut d = 0.0;
            let mut n = [[0.0f64; 3]; 4];
            let mut h = [0.0f64; 4];
            let cirradius;
            let mut minheightinv;
            if super::surface::lu_decmp(&mut a, 3, &mut indx, &mut d) {
                for j in 0..3 {
                    let mut rhs = [0.0f64; 4];
                    rhs[j] = 1.0;
                    super::surface::lu_solve(&a, 3, &indx, &mut rhs);
                    n[j] = [rhs[0], rhs[1], rhs[2]];
                }
                for i in 0..3 {
                    n[3][i] = -n[0][i] - n[1][i] - n[2][i];
                }
                let mut rhs = [0.0f64; 4];
                for i in 0..3 {
                    rhs[i] = 0.5 * dot(&v[i], &v[i]);
                }
                super::surface::lu_solve(&a, 3, &indx, &mut rhs);
                let r3 = [rhs[0], rhs[1], rhs[2]];
                cirradius = dot(&r3, &r3).sqrt();
                for i in 0..4 {
                    h[i] = dot(&n[i], &n[i]).sqrt();
                    for j in 0..3 {
                        n[i][j] /= h[i];
                    }
                }
                minheightinv = h[0];
                for &hi in h.iter().skip(1) {
                    if hi > minheightinv {
                        minheightinv = hi;
                    }
                }
            } else {
                // A nearly degenerated tet.
                if tetvol <= 0.0 {
                    continue; // (upstream prints a warning and skips it)
                }
                n[0] = super::surface::facenormal(&p[2], &p[1], &p[3], 1);
                n[1] = super::surface::facenormal(&p[0], &p[2], &p[3], 1);
                n[2] = super::surface::facenormal(&p[1], &p[0], &p[3], 1);
                n[3] = super::surface::facenormal(&p[0], &p[1], &p[2], 1);
                for i in 0..4 {
                    h[i] = dot(&n[i], &n[i]).sqrt();
                    for j in 0..3 {
                        n[i][j] /= h[i];
                    }
                }
                minheightinv = h[0] / tetvol;
                for &hi in h.iter().skip(1) {
                    if hi / tetvol > minheightinv {
                        minheightinv = hi / tetvol;
                    }
                }
                cirradius = 0.5 * longlen.sqrt();
            }

            // The dihedrals (degrees) at each edge.
            let mut alldihed = [0.0f64; 6];
            let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
            for (k, &(i, j)) in pairs.iter().enumerate() {
                let mut c = -dot(&n[i], &n[j]);
                if c < -1.0 {
                    c = -1.0;
                } else if c > 1.0 {
                    c = 1.0;
                }
                alldihed[k] = c.acos() / PI * 180.0;
            }
            for &ad in &alldihed {
                if ad < smallestdiangle {
                    smallestdiangle = ad;
                }
                if ad > biggestdiangle {
                    biggestdiangle = ad;
                }
                let tendegree = if ad < 5.0 {
                    0
                } else if ad < 10.0 {
                    1
                } else if (80.0..110.0).contains(&ad) {
                    9
                } else if (170.0..175.0).contains(&ad) {
                    16
                } else if ad >= 175.0 {
                    17
                } else {
                    let t = (ad / 10.0) as usize;
                    if ad < 80.0 {
                        t + 1
                    } else {
                        t - 1
                    }
                };
                dihedangletable[tendegree] += 1;
            }

            // The face angles, once per face.
            for ver in 0..4 {
                let tl = TriFace { tet: t, ver };
                let nb = self.fsym(&tl);
                if self.tets[nb.tet].v[3] == DUMMYPOINT || self.tet_address_below(t, nb.tet) {
                    let q = [
                        self.points[self.org(&tl)].xyz,
                        self.points[self.dest(&tl)].xyz,
                        self.points[self.apex(&tl)].xyz,
                    ];
                    let f0 = self.interiorangle(&q[0], &q[1], &q[2], None);
                    let f1 = self.interiorangle(&q[1], &q[2], &q[0], None);
                    let fa = [f0, f1, PI - (f0 + f1)];
                    for x in fa {
                        let x = (x * 180.0) / PI;
                        if x < smallestfaangle {
                            smallestfaangle = x;
                        }
                        if x > biggestfaangle {
                            biggestfaangle = x;
                        }
                        faceangletable[(x / 10.0) as usize] += 1;
                    }
                }
            }

            let tetradius = cirradius / shortlen.sqrt();
            if tetradius < smallestradiusratio {
                smallestradiusratio = tetradius;
            }
            if tetradius > biggestradiusratio {
                biggestradiusratio = tetradius;
            }
            let tetaspect = longlen.sqrt() * minheightinv;
            if tetaspect < smallestratio {
                smallestratio = tetaspect;
            }
            if tetaspect > biggestratio {
                biggestratio = tetaspect;
            }
            let mut ai = 0;
            while tetaspect > ASPECT[ai] && ai < 11 {
                ai += 1;
            }
            aspecttable[ai] += 1;
            let mut ri = 0;
            while tetradius > RADIUS[ri] && ri < 11 {
                ri += 1;
            }
            radiustable[ri] += 1;
        }

        QualityStatistics {
            volume: [smallestvolume, biggestvolume],
            edge: [shortest.sqrt(), longest.sqrt()],
            aspect_ratio: [smallestratio, biggestratio],
            radius_edge_ratio: [smallestradiusratio, biggestradiusratio],
            face_angle: [smallestfaangle, biggestfaangle],
            dihedral: [smallestdiangle, biggestdiangle],
            aspect_table: aspecttable,
            face_angle_table: faceangletable,
            dihedral_table: dihedangletable,
        }
    }

    /// `qualitystatistics()`: the text `tetgen -V` prints, from "Mesh
    /// quality statistics:" to the blank lines after the dihedral
    /// histogram.
    pub fn qualitystatistics(&mut self) -> String {
        let q = self.quality_numbers();
        let mut s = String::new();
        let g5 = |x: f64, w: usize| alloc::format!("{:>w$}", fmt_g(x, 5), w = w);
        let trunc8 = |x: f64| {
            let t = super::output::fmt_g17(x);
            t.chars().take(8).collect::<String>()
        };
        let _ = writeln!(s, "Mesh quality statistics:\n");
        let _ = writeln!(
            s,
            "  Smallest volume: {}   |  Largest volume: {}",
            g5(q.volume[0], 16),
            g5(q.volume[1], 16)
        );
        let _ = writeln!(
            s,
            "  Shortest edge:   {}   |  Longest edge:   {}",
            g5(q.edge[0], 16),
            g5(q.edge[1], 16)
        );
        let _ = writeln!(
            s,
            "  Smallest asp.ratio: {}   |  Largest asp.ratio: {}",
            g5(q.aspect_ratio[0], 13),
            g5(q.aspect_ratio[1], 13)
        );
        let _ = writeln!(
            s,
            "  Smallest facangle: {}   |  Largest facangle:       {}",
            g5(q.face_angle[0], 14),
            trunc8(q.face_angle[1])
        );
        let _ = writeln!(
            s,
            "  Smallest dihedral: {}   |  Largest dihedral:       {}\n",
            g5(q.dihedral[0], 14),
            trunc8(q.dihedral[1])
        );
        let g6l = |x: f64| alloc::format!("{:<6}", fmt_g(x, 6));
        let g6r = |x: f64| alloc::format!("{:>6}", fmt_g(x, 6));
        let at = &q.aspect_table;
        let _ = writeln!(s, "  Aspect ratio histogram:");
        let _ = writeln!(
            s,
            "         < {}    :  {:8}      | {} - {}     :  {:8}",
            g6l(ASPECT[0]),
            at[0],
            g6r(ASPECT[5]),
            g6l(ASPECT[6]),
            at[6]
        );
        for i in 1..5 {
            let _ = writeln!(
                s,
                "  {} - {}    :  {:8}      | {} - {}     :  {:8}",
                g6r(ASPECT[i - 1]),
                g6l(ASPECT[i]),
                at[i],
                g6r(ASPECT[i + 5]),
                g6l(ASPECT[i + 6]),
                at[i + 6]
            );
        }
        let _ = writeln!(
            s,
            "  {} - {}    :  {:8}      | {} -            :  {:8}",
            g6r(ASPECT[4]),
            g6l(ASPECT[5]),
            at[5],
            g6r(ASPECT[10]),
            at[11]
        );
        let _ = writeln!(
            s,
            "  (A tetrahedron's aspect ratio is its longest edge length divided by its"
        );
        let _ = writeln!(s, "    smallest side height)\n");
        let ft = &q.face_angle_table;
        let _ = writeln!(s, "  Face angle histogram:");
        for i in 0..9 {
            let _ = writeln!(
                s,
                "    {:3} - {:3} degrees:  {:8}      |    {:3} - {:3} degrees:  {:8}",
                i * 10,
                i * 10 + 10,
                ft[i],
                i * 10 + 90,
                i * 10 + 100,
                ft[i + 9]
            );
        }
        s.push('\n');
        let dt = &q.dihedral_table;
        let _ = writeln!(s, "  Dihedral angle histogram:");
        let row = |s: &mut String, a: i32, b: i32, x: u64, c: i32, d: i32, y: u64| {
            let _ = writeln!(
                s,
                "     {a:3} - {b:2} degrees:  {x:8}      |    {c:3} - {d:3} degrees:  {y:8}"
            );
        };
        row(&mut s, 0, 5, dt[0], 80, 110, dt[9]);
        row(&mut s, 5, 10, dt[1], 110, 120, dt[10]);
        for i in 2..7 {
            let i32_ = i as i32;
            row(
                &mut s,
                (i32_ - 1) * 10,
                (i32_ - 1) * 10 + 10,
                dt[i],
                (i32_ - 1) * 10 + 110,
                (i32_ - 1) * 10 + 120,
                dt[i + 9],
            );
        }
        row(&mut s, 60, 70, dt[7], 170, 175, dt[16]);
        row(&mut s, 70, 80, dt[8], 175, 180, dt[17]);
        if self.minfacetdihed != PI {
            let _ = writeln!(
                s,
                "  Minimum input dihedral angle is {} (degree).",
                fmt_g(self.minfacetdihed / PI * 180.0, 6)
            );
        }
        s.push_str("\n\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_g;

    /// `%g` at 5 and 6 digits, as glibc prints them.
    #[test]
    fn fmt_g_is_c_percent_g() {
        assert_eq!(fmt_g(0.00049323, 5), "0.00049323");
        assert_eq!(fmt_g(1.2366999, 5), "1.2367");
        assert_eq!(fmt_g(49.71899, 5), "49.719");
        assert_eq!(fmt_g(1.5, 6), "1.5");
        assert_eq!(fmt_g(100.0, 6), "100");
        assert_eq!(fmt_g(1.0e-5, 5), "1e-05");
        assert_eq!(fmt_g(123456.0, 5), "1.2346e+05");
    }
}
