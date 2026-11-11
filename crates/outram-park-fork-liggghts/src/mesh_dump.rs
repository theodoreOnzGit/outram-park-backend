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

//! `dump mesh/stl`: write a mesh as an STL file (gh:#1416). Translated from
//! `dump_mesh_stl.cpp` of LIGGGHTS-PUBLIC `3d5c00f2` (`DumpMeshSTL::pack`,
//! `write_header_*`, `write_data_ascii`, `write_data_binary`, the `region`
//! keyword of `count`/`pack`).
//!
//! Each facet is written with the mesh's CACHED unit normal (`surfaceNorm`,
//! which for a rotating mesh is the incrementally rotated one, not one
//! recomputed from the nodes) and its three current nodes. The ASCII format
//! prints every number with C's `%g` (six significant digits), reproduced by
//! [`format_g`]; the binary format is an 80-byte header, a `u32` facet count
//! and, per facet, twelve little-endian `f32` and a zero `u16`.
//!
//! **Not translated, and why.** `dump mesh/vtk`, `mesh/vtm` and
//! `mesh/gran/vtk` are compiled only with `LAMMPS_VTK` (the VTK library) and
//! write VTK data files; the compiled upstream used for the comparison has
//! none of them (`lmp_serial -h` lists `mesh/stl` only), so they cannot be
//! compared and are not ported. `dump mesh/stl`'s `ghost`/`all` selectors
//! (parallel ghost facets) and tet-mesh regions have no serial counterpart.

use crate::mesh_wall::MeshWall;

/// One facet as `dump mesh/stl` writes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StlFacet {
    /// Cached unit surface normal.
    pub normal: [f64; 3],
    /// The three nodes `[m]`.
    pub nodes: [[f64; 3]; 3],
}

/// Header line of the ASCII format.
const ASCII_HEADER: &str = "solid LIGGGHTS_STL_EXPORT\n";
/// Footer line of the ASCII format.
const ASCII_FOOTER: &str = "endsolid LIGGGHTS_STL_EXPORT\n";

/// C's `printf("%g", x)`: six significant digits, the shorter of fixed and
/// scientific form, trailing zeros removed, exponent of at least two digits.
#[must_use]
pub fn format_g(x: f64) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    const P: i32 = 6;
    // decimal exponent after rounding to P significant digits
    let sci = format!("{:.*e}", (P - 1) as usize, x);
    let (mant, exp) = sci.split_once('e').expect("scientific form has an exponent");
    let exp: i32 = exp.parse().expect("integer exponent");
    if exp < -4 || exp >= P {
        let mant = strip_zeros(mant);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{mant}e{sign}{:02}", exp.abs())
    } else {
        let decimals = (P - 1 - exp).max(0) as usize;
        strip_zeros(&format!("{x:.decimals$}"))
    }
}

fn strip_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// The facets of `mesh` in index order, for the ones whose centre satisfies
/// `keep` (`dump mesh/stl ... region`; pass `|_| true` for no region). The
/// centre is the mean of the nodes, summed node by node as `center_` is.
#[must_use]
pub fn mesh_facets<F: Fn([f64; 3]) -> bool>(mesh: &MeshWall, keep: F) -> Vec<StlFacet> {
    let nodes = mesh.nodes();
    let mut out = Vec::new();
    for (i, nd) in nodes.iter().enumerate() {
        let mut c = [0.0_f64; 3];
        for node in nd {
            for k in 0..3 {
                c[k] += node[k];
            }
        }
        for v in &mut c {
            *v /= 3.0;
        }
        if keep(c) {
            out.push(StlFacet {
                normal: mesh.state.surface_norm(i),
                nodes: *nd,
            });
        }
    }
    out
}

/// The ASCII STL text of `facets` (`write_header_ascii`, `write_data_ascii`,
/// footer).
#[must_use]
pub fn stl_ascii(facets: &[StlFacet]) -> String {
    let mut s = String::from(ASCII_HEADER);
    for f in facets {
        let g = |v: [f64; 3]| format!("{} {} {}", format_g(v[0]), format_g(v[1]), format_g(v[2]));
        s.push_str(&format!("  facet normal {}\n", g(f.normal)));
        s.push_str("    outer loop\n");
        for n in &f.nodes {
            s.push_str(&format!("      vertex {}\n", g(*n)));
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str(ASCII_FOOTER);
    s
}

/// The binary STL bytes of `facets` (`write_header_binary`,
/// `write_data_binary`): 80 header bytes (`"STL File written by LIGGGHTS"`
/// padded with spaces), the facet count, then per facet twelve `f32`
/// (normal, three nodes) and a zero `u16`.
#[must_use]
pub fn stl_binary(facets: &[StlFacet]) -> Vec<u8> {
    let mut b = Vec::with_capacity(84 + 50 * facets.len());
    let mut header = b"STL File written by LIGGGHTS".to_vec();
    header.resize(80, b' ');
    b.extend_from_slice(&header);
    b.extend_from_slice(&(facets.len() as u32).to_le_bytes());
    for f in facets {
        let mut vals = Vec::with_capacity(12);
        vals.extend_from_slice(&f.normal);
        for n in &f.nodes {
            vals.extend_from_slice(n);
        }
        for v in vals {
            b.extend_from_slice(&(v as f32).to_le_bytes());
        }
        b.extend_from_slice(&0u16.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** `format_g` against values whose `%g` form is known
    /// from C: trailing-zero removal, the fixed/scientific switch at
    /// exponents -5 and 6, rounding up across a decade (999999.5 -> 1e+06),
    /// two-digit exponents, signed zero. **Result:** all equal.
    #[test]
    fn format_g_matches_c() {
        let cases: [(f64, &str); 14] = [
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (-0.678801, "-0.678801"),
            (0.0404508, "0.0404508"),
            (-1.41394e-17, "-1.41394e-17"),
            (123456.0, "123456"),
            (1234567.0, "1.23457e+06"),
            (999999.5, "1e+06"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (0.1 + 0.2, "0.3"),
            (100.0, "100"),
            (1.5e100, "1.5e+100"),
        ];
        for (x, want) in cases {
            assert_eq!(format_g(x), want, "{x:e}");
        }
        assert_eq!(format_g(f64::INFINITY), "inf");
        assert_eq!(format_g(f64::NAN), "nan");
    }

    fn facet() -> StlFacet {
        StlFacet {
            normal: [0.0, 0.0, 1.0],
            nodes: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.5, 0.0]],
        }
    }

    /// **Methodology.** One facet written as ASCII and binary: the ASCII text
    /// is the upstream layout line for line; the binary is 84 + 50 bytes with
    /// the padded header, count 1, `f32` payload and a zero attribute word.
    #[test]
    fn ascii_and_binary_layout() {
        let t = stl_ascii(&[facet()]);
        assert_eq!(
            t,
            "solid LIGGGHTS_STL_EXPORT\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 0.5 0\n    endloop\n  endfacet\nendsolid LIGGGHTS_STL_EXPORT\n"
        );
        let b = stl_binary(&[facet()]);
        assert_eq!(b.len(), 134);
        assert_eq!(&b[..28], b"STL File written by LIGGGHTS");
        assert!(b[28..80].iter().all(|&c| c == b' '));
        assert_eq!(&b[80..84], &1u32.to_le_bytes());
        assert_eq!(&b[84 + 8..84 + 12], &1.0f32.to_le_bytes());
        assert_eq!(&b[132..134], &[0, 0]);
    }

    /// **Methodology.** `mesh_facets` keeps the facets whose centre passes
    /// the region predicate, in index order, with the cached normal.
    #[test]
    fn region_filter_keeps_matching_centres() {
        let stl = "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nfacet normal 0 0 1\nouter loop\nvertex 5 5 0\nvertex 6 5 0\nvertex 5 6 0\nendloop\nendfacet\nendsolid t\n";
        let mesh = MeshWall::from_ascii_stl(stl).expect("valid stl");
        assert_eq!(mesh_facets(&mesh, |_| true).len(), 2);
        let far = mesh_facets(&mesh, |c| c[0] > 2.0);
        assert_eq!(far.len(), 1);
        assert_eq!(far[0].nodes[0], [5.0, 5.0, 0.0]);
        assert_eq!(far[0].normal, [0.0, 0.0, 1.0]);
    }
}
