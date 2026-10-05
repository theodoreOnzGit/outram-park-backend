// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OpenFOAM (GeN-Foam's mesh-to-mesh mapper)
//   Upstream: https://develop.openfoam.com/Development/openfoam
//   Upstream source: src/meshTools/tetOverlapVolume/tetOverlapVolume{.C,Templates.C}
//                    (tetTetOverlap, cellCellOverlapMinDecomp),
//                    src/sampling/meshToMesh/calcMethod/cellVolumeWeight/
//                    cellVolumeWeightMethod.C (calculateAddressing, tolerance),
//                    src/sampling/meshToMesh/meshToMesh{.C,Templates.C}
//                    (normaliseWeights, mapTgtToSrc)
//   Called from: GeN-Foam src/classes/multiRegion/meshHandler/meshHandler.C
//                (meshToMesh(..., imCellVolumeWeight, pmAABB, false))
//   Upstream copyright: (C) 2012-2015 OpenFOAM Foundation, (C) 2016-2017 OpenCFD Ltd.
//   Upstream license: GPL-3.0-or-later
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
//
// This offering is not approved or endorsed by the OpenFOAM Foundation, nor
// OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Mesh-to-mesh overlap: GeN-Foam's `cellVolumeWeight` mapping
//!
//! GeN-Foam keeps neutronics, thermal-hydraulics and thermo-mechanics on
//! **separate, overlapping meshes** and moves fields between them with
//! OpenFOAM's `meshToMesh`, hard-coded to `imCellVolumeWeight`
//! (`meshHandler.C`, the `new meshToMesh(...)` call). That method weighs each
//! target cell's value by the **exact volume of its intersection** with every
//! source cell. This module ports that operator onto the neutral
//! [`UnstructuredMesh`], so it works for every pair of the three meshes,
//! finite-volume polyhedra and finite-element tetrahedra alike.
//!
//! ## What upstream does, and what this does
//!
//! | upstream | here |
//! |---|---|
//! | `tetOverlapVolume::tetTetOverlap`: tet A sliced by the four face planes of tet B, inside pieces kept as tets, volumes summed | [`tet_tet_overlap`]: the same four plane slices, the same split of a sliced tet into inside tets |
//! | `cellCellOverlapMinDecomp`: both cells decomposed into tets about the cell centre (face base-point fan), every pair intersected | [`cell_cell_overlap`]: both cells decomposed by [`UnstructuredMesh::for_each_cell_simplex`] (face-centre fan to the cell centre), every pair intersected, **with each tet's orientation sign** (below) |
//! | `cellVolumeWeightMethod::calculateAddressing`: an advancing front from a seed pair; a pair is kept when `vol / V_src > tolerance_` (`1e-6`) | candidate pairs from the source mesh's bucket grid ([`crate::unstructured::CellLocator`]); the same `1e-6` relative tolerance. The front is a search order, not a different answer |
//! | `meshToMesh::normaliseWeights` (`normalise = true` in `constructNoCuttingPatches`): each cell's weights divided by **their sum** | [`MeshOverlap::weights_onto_target`] / [`MeshOverlap::weights_onto_source`] |
//! | `mapTgtToSrc(field, plusEqOp, result)`: `result *= (1 - Σw)`, then `result += Σ w·field` on every cell that has addresses; cells with none keep their value | [`MeshOverlap::map_onto_target`] / [`MeshOverlap::map_onto_source`] |
//!
//! **One deliberate extension.** OpenFOAM's centre decomposition is only a
//! tiling when the cell is star-shaped about its centre; some cfMesh dual
//! cells are not (`UnstructuredMesh::decomposition_is_valid`; 264 of 24 751 on
//! the first cylinder drawn, see the crate `CLAUDE.md`). Here every
//! decomposition tet carries the sign of its own volume, and the overlap is
//! `Σ_i Σ_j s_i s_j |T_i ∩ T_j|`. The signed cones from any point to a closed
//! surface's oriented triangles sum to the cell's indicator function, so this
//! is exact for **any** closed cell, and identical to upstream for a
//! star-shaped one (every sign `+1`).
//!
//! **Units.** Both meshes are compared in centimetres (each is rescaled by its
//! own [`LengthUnit`]), so a metre-unit FV mesh overlaps a centimetre-unit
//! tally mesh correctly. Volumes reported are cm³.
//!
//! ## Verification (2026-10-05, `cargo test --release -p outram-blender --lib overlap`)
//!
//! - `a_tet_overlaps_itself_fully` / `disjoint_tets_do_not_overlap` /
//!   `a_tet_cut_by_a_plane_through_its_midpoints_keeps_one_eighth`:
//!   the kernel against closed-form volumes, to `1e-12` relative.
//! - `a_mesh_maps_onto_itself_as_the_identity`: every weight is `(c, 1.0)`.
//! - `two_different_meshes_of_one_box_conserve_volume_and_integrals`: the
//!   overlap volume equals the box volume to `1e-9` relative; a constant maps
//!   to the same constant; `Σ V φ` of a linear field is conserved both ways
//!   (the property GeN-Foam's power and temperature coupling rely on).

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;

use super::mesh::UnstructuredMesh;

/// Upstream `meshToMeshMethod::tolerance_`: an overlap below this fraction of
/// the source cell's volume is not an overlap.
pub const OVERLAP_TOLERANCE: f64 = 1e-6;

type P = [f64; 3];

#[inline]
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn lerp(a: P, b: P, t: f64) -> P {
    [
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
        a[2] + t * (b[2] - a[2]),
    ]
}

/// Unsigned volume of a tetrahedron.
#[inline]
fn tet_volume(t: &[P; 4]) -> f64 {
    (dot(cross(sub(t[1], t[0]), sub(t[2], t[0])), sub(t[3], t[0])) / 6.0).abs()
}

/// `(normal, point)` of a plane, `dot(normal, x - point) > 0` on the kept side.
type Plane = (P, P);

/// The three inside tets of a triangular prism with matching vertices
/// `a[k] <-> b[k]` (its quad sides planar, as every prism a plane slice makes).
fn prism_tets(a: [P; 3], b: [P; 3], out: &mut Vec<[P; 4]>) {
    out.push([a[0], a[1], a[2], b[0]]);
    out.push([a[1], a[2], b[0], b[1]]);
    out.push([a[2], b[0], b[1], b[2]]);
}

/// Port of `tetrahedron::sliceWithPlane` keeping the inside: the part of `t`
/// where the plane function is positive, as tets appended to `out`.
fn slice_inside(t: &[P; 4], pl: &Plane, out: &mut Vec<[P; 4]>) {
    let d: [f64; 4] = std::array::from_fn(|k| dot(pl.0, sub(t[k], pl.1)));
    let inside: Vec<usize> = (0..4).filter(|&k| d[k] > 0.0).collect();
    let outside: Vec<usize> = (0..4).filter(|&k| d[k] <= 0.0).collect();
    let cut = |i: usize, o: usize| lerp(t[i], t[o], d[i] / (d[i] - d[o]));
    match inside.len() {
        0 => {}
        4 => out.push(*t),
        1 => {
            let p = inside[0];
            out.push([
                t[p],
                cut(p, outside[0]),
                cut(p, outside[1]),
                cut(p, outside[2]),
            ]);
        }
        3 => {
            let q = outside[0];
            let a = [t[inside[0]], t[inside[1]], t[inside[2]]];
            let b = [cut(inside[0], q), cut(inside[1], q), cut(inside[2], q)];
            prism_tets(a, b, out);
        }
        _ => {
            let (p1, p2) = (inside[0], inside[1]);
            let (q1, q2) = (outside[0], outside[1]);
            let a = [t[p1], cut(p1, q1), cut(p1, q2)];
            let b = [t[p2], cut(p2, q1), cut(p2, q2)];
            prism_tets(a, b, out);
        }
    }
}

/// Volume of the intersection of two tetrahedra (unsigned): port of
/// `tetOverlapVolume::tetTetOverlap` — `a` sliced by the four face planes of
/// `b` in turn, the inside pieces carried as tets.
pub fn tet_tet_overlap(a: &[P; 4], b: &[P; 4]) -> f64 {
    // Upstream returns early when a face area is ~0 (a degenerate tet).
    let vb = tet_volume(b);
    if !(vb > 0.0) || !(tet_volume(a) > 0.0) {
        return 0.0;
    }
    let mut pieces = vec![*a];
    let mut next = Vec::with_capacity(8);
    for f in 0..4 {
        let (i, j, k) = [(1, 2, 3), (0, 2, 3), (0, 1, 3), (0, 1, 2)][f];
        let mut n = cross(sub(b[j], b[i]), sub(b[k], b[i]));
        if dot(n, sub(b[f], b[i])) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let pl = (n, b[i]);
        next.clear();
        for t in &pieces {
            slice_inside(t, &pl, &mut next);
        }
        if next.is_empty() {
            return 0.0;
        }
        std::mem::swap(&mut pieces, &mut next);
    }
    pieces.iter().map(tet_volume).sum()
}

fn bounds_of(t: &[P; 4]) -> [P; 2] {
    let mut lo = t[0];
    let mut hi = t[0];
    for p in &t[1..] {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    [lo, hi]
}

#[inline]
fn boxes_overlap(a: &[P; 2], b: &[P; 2]) -> bool {
    (0..3).all(|k| a[0][k] <= b[1][k] && b[0][k] <= a[1][k])
}

/// A cell's decomposition tets, each with its orientation sign and bounds,
/// scaled by `s` (into cm).
fn signed_tets(m: &UnstructuredMesh, c: usize, s: f64) -> Vec<(f64, [P; 4], [P; 2])> {
    let mut v = Vec::new();
    m.for_each_cell_simplex(c, |t| {
        let t = t.map(|p| [p[0] * s, p[1] * s, p[2] * s]);
        let signed = dot(cross(sub(t[1], t[0]), sub(t[2], t[0])), sub(t[3], t[0])) / 6.0;
        if signed != 0.0 {
            v.push((signed.signum(), t, bounds_of(&t)));
        }
    });
    v
}

/// Overlap volume \[cm³\] of two cells given as signed tets (the extension in
/// the module docs: exact for any closed cell).
fn cell_cell_overlap(a: &[(f64, [P; 4], [P; 2])], b: &[(f64, [P; 4], [P; 2])]) -> f64 {
    let mut v = 0.0;
    for (sa, ta, ba) in a {
        for (sb, tb, bb) in b {
            if boxes_overlap(ba, bb) {
                v += sa * sb * tet_tet_overlap(ta, tb);
            }
        }
    }
    v
}

/// The cell-overlap addressing between a source and a target mesh, with the
/// raw overlap volumes \[cm³\] (upstream keeps volumes until it normalises).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshOverlap {
    /// Per source cell: `(target cell, overlap volume [cm³])`.
    pub src_to_tgt: Vec<Vec<(usize, f64)>>,
    /// Per target cell: `(source cell, overlap volume [cm³])`.
    pub tgt_to_src: Vec<Vec<(usize, f64)>>,
    /// Source cell volumes \[cm³\].
    pub src_volumes: Vec<f64>,
    /// Target cell volumes \[cm³\].
    pub tgt_volumes: Vec<f64>,
}

impl MeshOverlap {
    /// Build the addressing (upstream `meshToMesh` with `imCellVolumeWeight`).
    /// Target cells are processed in parallel. Cost grows with the number of
    /// overlapping pairs times the tets per cell (polyhedral dual cells
    /// decompose into tens of tets).
    pub fn new(src: &UnstructuredMesh, tgt: &UnstructuredMesh) -> Self {
        let ss = src.unit().cm_per_unit();
        let st = tgt.unit().cm_per_unit();
        let to_src_unit = st / ss;
        let src_volumes: Vec<f64> = (0..src.n_cells())
            .map(|c| src.cell_volume(c) * ss.powi(3))
            .collect();
        let tgt_volumes: Vec<f64> = (0..tgt.n_cells())
            .map(|c| tgt.cell_volume(c) * st.powi(3))
            .collect();
        let src_tets: Vec<Vec<(f64, [P; 4], [P; 2])>> = (0..src.n_cells())
            .into_par_iter()
            .map(|c| signed_tets(src, c, ss))
            .collect();
        let tgt_to_src: Vec<Vec<(usize, f64)>> = (0..tgt.n_cells())
            .into_par_iter()
            .map(|t| {
                let [lo, hi] = tgt.cell_bounds(t);
                let lo_s = lo.map(|x| x * to_src_unit);
                let hi_s = hi.map(|x| x * to_src_unit);
                let mut cand = Vec::new();
                src.locator().candidates_in_box(lo_s, hi_s, &mut cand);
                let tb = [lo.map(|x| x * st), hi.map(|x| x * st)];
                let tt = signed_tets(tgt, t, st);
                let mut out = Vec::new();
                for s in cand {
                    let [a, b] = src.cell_bounds(s);
                    let sb = [a.map(|x| x * ss), b.map(|x| x * ss)];
                    if !boxes_overlap(&sb, &tb) {
                        continue;
                    }
                    let v = cell_cell_overlap(&src_tets[s], &tt);
                    if v / src_volumes[s] > OVERLAP_TOLERANCE {
                        out.push((s, v));
                    }
                }
                out
            })
            .collect();
        let mut src_to_tgt = vec![Vec::new(); src.n_cells()];
        for (t, row) in tgt_to_src.iter().enumerate() {
            for &(s, v) in row {
                src_to_tgt[s].push((t, v));
            }
        }
        Self {
            src_to_tgt,
            tgt_to_src,
            src_volumes,
            tgt_volumes,
        }
    }

    /// Total intersection volume \[cm³\] (upstream `V_`).
    pub fn overlap_volume(&self) -> f64 {
        self.tgt_to_src.iter().flatten().map(|&(_, v)| v).sum()
    }

    /// Fraction of the target mesh's volume that some source cell covers.
    pub fn target_coverage(&self) -> f64 {
        self.overlap_volume() / self.tgt_volumes.iter().sum::<f64>().max(f64::MIN_POSITIVE)
    }

    /// Fraction of the source mesh's volume that some target cell covers.
    pub fn source_coverage(&self) -> f64 {
        self.overlap_volume() / self.src_volumes.iter().sum::<f64>().max(f64::MIN_POSITIVE)
    }

    fn normalised(rows: &[Vec<(usize, f64)>]) -> Vec<Vec<(usize, f64)>> {
        rows.iter()
            .map(|r| {
                let s: f64 = r.iter().map(|x| x.1).sum();
                r.iter().map(|&(c, v)| (c, v / s)).collect()
            })
            .collect()
    }

    /// Per target cell, `(source cell, weight)` with the weights summing to 1
    /// (upstream `normaliseWeights`: divided by their sum, not the volume).
    pub fn weights_onto_target(&self) -> Vec<Vec<(usize, f64)>> {
        Self::normalised(&self.tgt_to_src)
    }

    /// Per source cell, `(target cell, weight)`, summing to 1.
    pub fn weights_onto_source(&self) -> Vec<Vec<(usize, f64)>> {
        Self::normalised(&self.src_to_tgt)
    }

    /// Map a source cell field onto the target: upstream `mapTgtToSrc` with
    /// `plusEqOp` and normalised weights. A target cell with any overlap gets
    /// `Σ w·φ_src`; one with none keeps its value.
    pub fn map_onto_target(&self, src_field: &[f64], tgt_field: &mut [f64]) {
        apply(&self.weights_onto_target(), src_field, tgt_field);
    }

    /// Map a target cell field onto the source (the reverse direction).
    pub fn map_onto_source(&self, tgt_field: &[f64], src_field: &mut [f64]) {
        apply(&self.weights_onto_source(), tgt_field, src_field);
    }
}

/// `result[c] = result[c] (1 - Σw) + Σ w field` on cells with addresses.
pub fn apply(weights: &[Vec<(usize, f64)>], field: &[f64], result: &mut [f64]) {
    for (c, row) in weights.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let sw: f64 = row.iter().map(|x| x.1).sum();
        let mut r = result[c] * (1.0 - sw);
        for &(s, w) in row {
            r += w * field[s];
        }
        result[c] = r;
    }
}

/// Convenience: cell volumes of `m` in cm³.
pub fn cell_volumes_cm3(m: &UnstructuredMesh) -> Vec<f64> {
    let s = m.unit().cm_per_unit().powi(3);
    (0..m.n_cells()).map(|c| m.cell_volume(c) * s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unstructured::element::{Element, ElementKind, LengthUnit};

    const T0: [P; 4] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];

    #[test]
    fn a_tet_overlaps_itself_fully() {
        let v = tet_tet_overlap(&T0, &T0);
        assert!((v - 1.0 / 6.0).abs() < 1e-12, "{v}");
    }

    #[test]
    fn disjoint_tets_do_not_overlap() {
        let b = T0.map(|p| [p[0] + 2.0, p[1], p[2]]);
        assert_eq!(tet_tet_overlap(&T0, &b), 0.0);
    }

    /// The unit corner tet and its copy scaled by 1/2 about the origin: the
    /// overlap is the small tet, 1/8 of the volume.
    #[test]
    fn a_tet_cut_by_a_plane_through_its_midpoints_keeps_one_eighth() {
        let half = T0.map(|p| [0.5 * p[0], 0.5 * p[1], 0.5 * p[2]]);
        let v = tet_tet_overlap(&T0, &half);
        assert!((v - 1.0 / 48.0).abs() < 1e-12, "{v}");
        let v2 = tet_tet_overlap(&half, &T0);
        assert!((v2 - 1.0 / 48.0).abs() < 1e-12, "{v2}");
    }

    /// A box `[0,lx]x[0,ly]x[0,lz]` of `nx x ny x nz` Hex8 cells.
    fn hex_box(l: [f64; 3], n: [usize; 3]) -> UnstructuredMesh {
        let id = |i: usize, j: usize, k: usize| i + (n[0] + 1) * (j + (n[1] + 1) * k);
        let mut pts = Vec::new();
        for k in 0..=n[2] {
            for j in 0..=n[1] {
                for i in 0..=n[0] {
                    pts.push([
                        l[0] * i as f64 / n[0] as f64,
                        l[1] * j as f64 / n[1] as f64,
                        l[2] * k as f64 / n[2] as f64,
                    ]);
                }
            }
        }
        let mut els = Vec::new();
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    els.push(Element {
                        kind: ElementKind::Hex8,
                        nodes: vec![
                            id(i, j, k),
                            id(i + 1, j, k),
                            id(i + 1, j + 1, k),
                            id(i, j + 1, k),
                            id(i, j, k + 1),
                            id(i + 1, j, k + 1),
                            id(i + 1, j + 1, k + 1),
                            id(i, j + 1, k + 1),
                        ],
                    });
                }
            }
        }
        UnstructuredMesh::from_elements(LengthUnit::Centimetre, pts, els, Vec::new(), Vec::new())
            .expect("hex box")
    }

    #[test]
    fn a_mesh_maps_onto_itself_as_the_identity() {
        let m = hex_box([2.0, 1.0, 1.0], [2, 1, 1]);
        let o = MeshOverlap::new(&m, &m);
        for (t, row) in o.weights_onto_target().iter().enumerate() {
            assert_eq!(row.len(), 1, "cell {t}: {row:?}");
            assert_eq!(row[0].0, t);
            assert!((row[0].1 - 1.0).abs() < 1e-12);
        }
        assert!((o.overlap_volume() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn two_different_meshes_of_one_box_conserve_volume_and_integrals() {
        let a = hex_box([1.0, 1.0, 1.0], [3, 2, 2]);
        // The same box in metres, cut differently: 0.01 m = 1 cm.
        let b = hex_box([1.0, 1.0, 1.0], [2, 3, 4]).with_unit(LengthUnit::Metre);
        let o = MeshOverlap::new(&a, &b);
        assert!(
            (o.overlap_volume() - 1.0).abs() < 1e-9,
            "{}",
            o.overlap_volume()
        );
        assert!((o.target_coverage() - 1.0).abs() < 1e-9);
        // A constant maps to itself.
        let ones = vec![1.0; a.n_cells()];
        let mut on_b = vec![0.0; b.n_cells()];
        o.map_onto_target(&ones, &mut on_b);
        assert!(on_b.iter().all(|v| (v - 1.0).abs() < 1e-12));
        // ∑ V φ of a linear field is conserved both ways.
        let va = cell_volumes_cm3(&a);
        let vb = cell_volumes_cm3(&b);
        let phi_a: Vec<f64> = (0..a.n_cells())
            .map(|c| {
                let x = a.cell_centre(c);
                1.0 + 2.0 * x[0] - x[1] + 0.5 * x[2]
            })
            .collect();
        o.map_onto_target(&phi_a, &mut on_b);
        let ia: f64 = va.iter().zip(&phi_a).map(|(v, p)| v * p).sum();
        let ib: f64 = vb.iter().zip(&on_b).map(|(v, p)| v * p).sum();
        assert!((ia - ib).abs() < 1e-9 * ia.abs(), "{ia} vs {ib}");
        let mut back = vec![0.0; a.n_cells()];
        o.map_onto_source(&on_b, &mut back);
        let ia2: f64 = va.iter().zip(&back).map(|(v, p)| v * p).sum();
        assert!((ia - ia2).abs() < 1e-9 * ia.abs(), "{ia} vs {ia2}");
    }
}
