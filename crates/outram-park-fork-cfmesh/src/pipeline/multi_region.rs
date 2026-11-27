// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code, NOT a port. It introduces no
// meshing algorithm of its own: the tetrahedra come from this crate's port
// of TetGen 1.6.0 (`crate::tetgen`, AGPL-3.0-or-later, Hang Si / WIAS) run
// with region attributes (`-A`), and each region's polyhedral dual from
// `crate::dual`. What is new is the bookkeeping: splitting the tetrahedra
// by region, dualling each region on its own, and stitching the duals back
// together along the shared interface faces.

//! **Multi-region meshes with body-fitted internal boundaries.**
//!
//! The single-region pipeline ([`super::surface_to_tet_dual_mesh_multipatch`])
//! meshes one closed surface. A caller that needs regions inside it (solid
//! graphite, a fluid plenum, a porous bed) then has to give each cell the
//! region of its centroid, so an internal boundary comes out stair-stepped
//! at the cell size: no face lies on it.
//!
//! This module meshes the regions **together, with the interfaces as
//! constraints**:
//!
//! 1. [`MultiRegionSurface`] holds every outer and interface triangle, each
//!    with a patch marker, plus one seed point per connected region
//!    component.
//! 2. [`multi_region_tetgen`] runs TetGen `-pq<r>/<d>[a<V>]A` on it. TetGen
//!    recovers every triangle as a subface, so no tetrahedron crosses an
//!    interface, and `-A` gives every tetrahedron its seed's attribute.
//! 3. [`multi_region_tet_mesh`] returns the tetrahedra as one mesh. An
//!    interface subface sits between two tetrahedra, so it is an internal
//!    face, and the cell's region is its tetrahedron's attribute.
//! 4. [`multi_region_dual_mesh`] duals each region **separately**, so that a
//!    primal vertex on an interface gets one dual cell on each side instead
//!    of a single cell straddling it. Each region's dual boundary is tiled by
//!    the per-corner quads of the primal boundary triangles, and both sides
//!    of an interface have the same primal triangles. So the two duals
//!    present the same faces there, which are then **stitched** into internal
//!    faces. Their points are merged within `1e-9` of the bounding-box
//!    diagonal: face centroids are summed in a different vertex order on
//!    each side and can differ in the last bit. Any interface face without
//!    exactly one partner in another region is an error, never dropped.
//!
//! Each region's dual is re-wound from the primal geometry (`rewind_dual`):
//! the dual's own winding, from the vertex average of each cell, left a
//! cell open at a re-entrant edge of a region (the nested-cube test).
//!
//! Both routes end with [`crate::renumber_mesh::upper_triangular`], which
//! keeps the cell numbering, so `cell_region` stays valid, then
//! [`VolumeMesh::validate`] (closure) and a positive-volume check on every
//! cell (a cell wound inside out is closed too).
//!
//! # What is and is not verified
//!
//! The tests below check closure (`validate`), that the regions' volumes
//! are the exact volumes of the input regions to `1e-9`, that no cell has a
//! vertex on the far side of an interface, that every interface face is
//! stitched, and that the outer patches carry only outer faces. They do not
//! check cell quality. The duals are the existing
//! [`crate::dual::polyhedral_dual_min_faces`], with
//! [`crate::dual::polyhedral_dual`] as the fallback; only the per-corner
//! (`DualKind::MedianPerCorner`) boundary rule is used, because the stitch
//! relies on that rule depending only on the boundary triangle.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research
//! and V&V only, per the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use alloc::collections::BTreeMap;

use super::TetSource;
use crate::dual::{polyhedral_dual, polyhedral_dual_min_faces};
use crate::math::Vec3;
use crate::patches::{assign_patches_by_region_with, SurfaceRegions, SurfaceSearch};
use crate::tetgen::{tetrahedralize_plc, to_volume_mesh, PlcInput, Switches, TetgenOutput};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

/// A closed outer surface with internal interfaces, ready for
/// [`multi_region_mesh`]. Every triangle is wound consistently within its
/// own closed shell (TetGen does not need the winding; the dual and the
/// stitch do not read it).
#[derive(Debug, Clone, PartialEq)]
pub struct MultiRegionSurface {
    /// Surface points.
    pub points: Vec<Vec3>,
    /// Triangles (indices into [`Self::points`]); outer and interface ones.
    pub tris: Vec<[usize; 3]>,
    /// Patch of each triangle (index into [`Self::patch_names`]).
    pub marker: Vec<usize>,
    /// Patch names; an outer patch becomes a mesh patch, an interface
    /// patch becomes internal faces.
    pub patch_names: Vec<String>,
    /// Whether each patch is an internal interface.
    pub interface: Vec<bool>,
    /// Region names (cellZone names).
    pub region_names: Vec<String>,
    /// One point strictly inside every connected component of every
    /// region, and the region's index.
    pub seeds: Vec<(Vec3, usize)>,
    /// TetGen hole points (`holelist`, 2026-11-27): one inside every
    /// enclosed volume that belongs to no region, removed from the mesh. A
    /// single region meshed on its own with others inside it (HTR-10's solid
    /// graphite round the bed and plenums) needs one per cavity; without it
    /// the cavity is meshed with no seed and refused.
    pub holes: Vec<Vec3>,
}

impl MultiRegionSurface {
    /// Check the indices and lengths agree.
    ///
    /// # Errors
    /// The first inconsistency found.
    pub fn validate(&self) -> Result<(), String> {
        if self.marker.len() != self.tris.len() {
            return Err(format!("{} markers for {} triangles", self.marker.len(), self.tris.len()));
        }
        if self.interface.len() != self.patch_names.len() {
            return Err("one interface flag per patch name is required".into());
        }
        if self.tris.iter().flatten().any(|&i| i >= self.points.len()) {
            return Err("a triangle names a point that does not exist".into());
        }
        if self.marker.iter().any(|&m| m >= self.patch_names.len()) {
            return Err("a triangle's marker names no patch".into());
        }
        if self.seeds.iter().any(|s| s.1 >= self.region_names.len()) {
            return Err("a seed names no region".into());
        }
        for g in 0..self.region_names.len() {
            if !self.seeds.iter().any(|s| s.1 == g) {
                return Err(format!("region `{}` has no seed point", self.region_names[g]));
            }
        }
        Ok(())
    }
}

/// A mesh of several regions with body-fitted internal boundaries.
#[derive(Debug, Clone)]
pub struct MultiRegionMesh {
    /// The mesh: outer patches only; interfaces are internal faces.
    pub mesh: VolumeMesh,
    /// Region of every cell (index into the surface's `region_names`).
    pub cell_region: Vec<usize>,
    /// Per interface patch: the number of internal faces lying on it.
    pub interface_faces: Vec<(String, usize)>,
    /// What was run, for the report.
    pub notes: Vec<String>,
}

/// TetGen's switches for `source`, with `-A` (region attributes) added.
fn switches(source: TetSource) -> Result<Switches, String> {
    let TetSource::TetGen { radius_edge, min_dihedral, max_volume, wall_grade } = source else {
        return Err("multi-region meshing needs TetSource::TetGen (the carved route cannot fit internal boundaries)".into());
    };
    if wall_grade.is_some() {
        return Err("multi-region meshing does not take TetGen's -m wall grading (gh:#1825); pass wall_grade: None".into());
    }
    if !(radius_edge > 1.0 && radius_edge.is_finite()) {
        return Err(format!("TetGen radius-edge bound must be > 1 (got {radius_edge})"));
    }
    if !(0.0..90.0).contains(&min_dihedral) {
        return Err(format!("TetGen dihedral bound must be in [0, 90) degrees (got {min_dihedral})"));
    }
    let mut sw = Switches::new().quality(radius_edge, min_dihedral).region_attributes();
    if let Some(v) = max_volume {
        if !(v > 0.0 && v.is_finite()) {
            return Err(format!("TetGen volume bound must be > 0 (got {v})"));
        }
        sw = sw.max_volume(v);
    }
    Ok(sw)
}

/// Run TetGen on `s` with region attributes: attribute `g + 1` for region
/// `g` (0 would mean a tetrahedron no seed reached).
///
/// # Errors
/// A bad surface or source, TetGen refusing the surface, or a tetrahedron
/// in no seeded region (a component without a seed).
pub fn multi_region_tetgen(s: &MultiRegionSurface, source: TetSource) -> Result<TetgenOutput, String> {
    s.validate()?;
    let sw = switches(source)?;
    let pts: Vec<[f64; 3]> = s.points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let markers: Vec<i32> = s.marker.iter().map(|&m| m as i32).collect();
    let mut plc = PlcInput::from_triangle_soup(&pts, &s.tris, &markers);
    plc.regions = s
        .seeds
        .iter()
        .map(|(p, g)| [p.x, p.y, p.z, (*g + 1) as f64, -1.0])
        .collect();
    plc.holes = s.holes.iter().map(|p| [p.x, p.y, p.z]).collect();
    let r = tetrahedralize_plc(&plc, &sw.behavior()).map_err(|e| format!("TetGen refused the multi-region surface: {e:?}"))?;
    let out = r.output.ok_or("TetGen wrote no output")?;
    let attr = out.tet_attributes.as_ref().ok_or("TetGen gave no region attributes (-A)")?;
    let n = s.region_names.len();
    if let Some(a) = attr.iter().find(|&&a| !(a >= 1.0 && a <= n as f64 && a.fract() == 0.0)) {
        return Err(format!(
            "a tetrahedron has region attribute {a}: some enclosed volume has no seed point"
        ));
    }
    Ok(out)
}

fn regions_of(out: &TetgenOutput) -> Vec<usize> {
    out.tet_attributes
        .as_ref()
        .map(|a| a.iter().map(|&x| x as usize - 1).collect())
        .unwrap_or_else(|| vec![0; out.tets.len()])
}

/// The tetrahedra of `out` as one mesh with the region of every cell
/// (TetGen's `.ele` order, which [`to_volume_mesh`] keeps).
///
/// # Errors
/// When an interface subface ends up on the mesh boundary, or the mesh
/// does not validate.
pub fn multi_region_tet_mesh(s: &MultiRegionSurface, out: &TetgenOutput) -> Result<MultiRegionMesh, String> {
    let mesh = to_volume_mesh(out, Some(&s.patch_names));
    for p in &mesh.patches {
        if let Some(k) = s.patch_names.iter().position(|n| *n == p.name) {
            if s.interface[k] {
                return Err(format!("{} faces of interface `{}` are on the mesh boundary", p.n_faces, p.name));
            }
        }
    }
    mesh.validate()?;
    check_positive_volumes(&mesh, "multi-region tetrahedra")?;
    let cell_region = regions_of(out);
    let interface_faces = interface_counts_by_marker(&mesh, &cell_region, s, out);
    Ok(MultiRegionMesh {
        mesh,
        cell_region,
        interface_faces,
        notes: vec![format!(
            "TetGen tetrahedra with region attributes: {} tetrahedra on {} nodes, internal boundaries fitted",
            out.tets.len(),
            out.points.len()
        )],
    })
}

/// Per interface patch, the TetGen subfaces carrying its marker, then the
/// total of internal faces between cells of two different regions.
fn interface_counts_by_marker(mesh: &VolumeMesh, cell_region: &[usize], s: &MultiRegionSurface, out: &TetgenOutput) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = s
        .patch_names
        .iter()
        .enumerate()
        .filter(|&(k, _)| s.interface[k])
        .map(|(k, name)| (name.clone(), out.face_markers.iter().filter(|&&m| m == k as i32).count()))
        .collect();
    let total = (0..mesh.face_count())
        .filter(|&f| matches!(mesh.neighbour[f], Some(nb) if cell_region[nb] != cell_region[mesh.owner[f]]))
        .count();
    v.push(("all interfaces".into(), total));
    v
}

/// The tetrahedra of region `g` alone, points compacted, every `.face`
/// triangle on those points kept (with its marker).
fn region_subset(out: &TetgenOutput, region: &[usize], g: usize) -> TetgenOutput {
    let first = out.firstindex;
    let mut map: Vec<i32> = vec![-1; out.points.len()];
    let mut points = Vec::new();
    let mut tets = Vec::new();
    for (t, &rg) in out.tets.iter().zip(region) {
        if rg != g {
            continue;
        }
        let mut nt = [0i32; 4];
        for k in 0..4 {
            let i = (t[k] - first) as usize;
            if map[i] < 0 {
                map[i] = points.len() as i32;
                points.push(out.points[i]);
            }
            nt[k] = map[i];
        }
        tets.push(nt);
    }
    let mut faces = Vec::new();
    let mut face_markers = Vec::new();
    if let Some(fs) = &out.faces {
        for (f, &m) in fs.iter().zip(&out.face_markers) {
            let g3 = [map[(f[0] - first) as usize], map[(f[1] - first) as usize], map[(f[2] - first) as usize]];
            if g3.iter().all(|&x| x >= 0) {
                faces.push(g3);
                face_markers.push(m);
            }
        }
    }
    TetgenOutput {
        firstindex: 0,
        points,
        point_markers: None,
        tets,
        tet_attributes: None,
        faces: Some(faces),
        face_markers,
        edges: Vec::new(),
        edge_markers: Vec::new(),
        // No `-m` sizes on this route (wall grading is refused above).
        point_mtrs: None,
        point2tet: None,
    }
}

/// Re-wind the faces of a median dual of the tetrahedral mesh `prim` from
/// the primal geometry, not from cell centroids.
///
/// [`crate::volume_mesh::from_cell_faces`] winds every face outward from its
/// owner's vertex average, which on a non-convex dual cell (a primal vertex
/// on a re-entrant edge of the region, as at the corner of an enclosed
/// region) can lie on the wrong side of a face, leaving the cell open. Two
/// exact rules replace it, both for a dual whose cell `i` is primal vertex
/// `i` (as [`polyhedral_dual`] and [`polyhedral_dual_min_faces`] build it):
///
/// - an **internal** face between cells `o` and `n` is made of pieces
///   `[edge-mid, face-centroid, cell-centroid, face-centroid]`, each in the
///   plane `lambda_o = lambda_n` of its tetrahedron, so it separates the two
///   primal vertices: wound so that its area vector points from `o` to `n`;
/// - a **boundary** face `[v, edge-mid, face-centroid, edge-mid]` lies in
///   one primal boundary triangle at `v`: wound as that triangle (which
///   [`to_volume_mesh`] winds outward), the triangle being the one at `v`
///   whose plane holds the face's centroid.
///
/// On a dual that was already wound outward this changes nothing.
fn rewind_dual(dual: &VolumeMesh, prim: &VolumeMesh) -> VolumeMesh {
    let area = |pts: &[Vec3], r: &[usize]| {
        let mut a = Vec3::ZERO;
        for i in 0..r.len() {
            a = a.add(pts[r[i]].cross(pts[r[(i + 1) % r.len()]]));
        }
        a
    };
    // Primal boundary triangles at every vertex.
    let mut at: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for f in (0..prim.face_count()).filter(|&f| prim.neighbour[f].is_none()) {
        for &v in &prim.faces[f] {
            at.entry(v).or_default().push(f);
        }
    }
    let mut out = dual.clone();
    for f in 0..out.face_count() {
        let o = out.owner[f];
        let a = area(&out.points, &out.faces[f]);
        let want = match out.neighbour[f] {
            Some(n) => prim.points[n].sub(prim.points[o]),
            None => {
                let r = &out.faces[f];
                let c = r.iter().fold(Vec3::ZERO, |s, &v| s.add(out.points[v])).scale(1.0 / r.len() as f64);
                let best = at.get(&o).and_then(|ts| {
                    ts.iter().copied().min_by(|&x, &y| {
                        let off = |t: usize| {
                            let n = area(&prim.points, &prim.faces[t]);
                            (n.dot(c.sub(prim.points[prim.faces[t][0]])) / n.length().max(1e-300)).abs()
                        };
                        off(x).total_cmp(&off(y))
                    })
                });
                match best {
                    Some(t) => area(&prim.points, &prim.faces[t]),
                    None => continue,
                }
            }
        };
        if a.dot(want) < 0.0 {
            out.faces[f].reverse();
        }
    }
    out.cells = None;
    out
}

/// Region `g`'s dual, with its boundary faces in patches named after the
/// primal boundary triangle each lies on.
fn region_dual(s: &MultiRegionSurface, sub: &TetgenOutput, g: usize, notes: &mut Vec<String>) -> Result<VolumeMesh, String> {
    let prim = to_volume_mesh(sub, Some(&s.patch_names));
    if let Some(p) = prim.patches.iter().find(|p| p.name == "unmarked" || p.name.starts_with("marker_")) {
        return Err(format!("region `{}`: {} boundary faces are on no input facet (`{}`)", s.region_names[g], p.n_faces, p.name));
    }
    prim.validate().map_err(|e| format!("region `{}` tetrahedra: {e}", s.region_names[g]))?;
    let mut d = rewind_dual(&polyhedral_dual_min_faces(&prim), &prim);
    if d.validate().is_err() {
        notes.push(format!("region `{}`: face-minimal dual invalid, used the quad-fan dual", s.region_names[g]));
        d = rewind_dual(&polyhedral_dual(&prim), &prim);
        d.validate().map_err(|e| format!("region `{}` dual: {e}", s.region_names[g]))?;
    }
    // The primal boundary as a labelled surface, to name the dual's faces.
    let mut tris = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for p in &prim.patches {
        for f in p.start_face..p.start_face + p.n_faces {
            let r = &prim.faces[f];
            tris.push([r[0], r[1], r[2]]);
            labels.push(&p.name);
        }
    }
    let regions = SurfaceRegions::from_labels(&labels);
    assign_patches_by_region_with(&d, &prim.points, &tris, &regions, SurfaceSearch::Octree)
}

/// Merges points closer than `tol` through a hash grid of cell `2 tol`.
struct PointMerge {
    tol: f64,
    grid: BTreeMap<[i64; 3], Vec<usize>>,
}

impl PointMerge {
    fn key(&self, p: Vec3) -> [i64; 3] {
        let h = 2.0 * self.tol;
        [(p.x / h).floor() as i64, (p.y / h).floor() as i64, (p.z / h).floor() as i64]
    }
    /// The representative of `p` (global index `i`): an earlier point within
    /// `tol`, else `i` itself, registered.
    fn find_or_insert(&mut self, i: usize, p: Vec3, pts: &[Vec3]) -> usize {
        let k = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = self.grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        for &j in v {
                            if pts[j].sub(p).length() <= self.tol {
                                return j;
                            }
                        }
                    }
                }
            }
        }
        self.grid.entry(k).or_default().push(i);
        i
    }
}

/// Dual every region of `out` separately and stitch the duals along the
/// interfaces into one conformal mesh.
///
/// # Errors
/// A region whose tetrahedra or dual is invalid, an interface face without
/// exactly one partner in another region, or a stitched mesh that does not
/// validate.
pub fn multi_region_dual_mesh(s: &MultiRegionSurface, out: &TetgenOutput) -> Result<MultiRegionMesh, String> {
    let region = regions_of(out);
    let n_regions = s.region_names.len();
    let mut notes = Vec::new();
    let mut duals = Vec::with_capacity(n_regions);
    for g in 0..n_regions {
        let sub = region_subset(out, &region, g);
        if sub.tets.is_empty() {
            return Err(format!("region `{}` has no tetrahedra", s.region_names[g]));
        }
        duals.push(region_dual(s, &sub, g, &mut notes)?);
    }
    // Global points and cells.
    let mut pts: Vec<Vec3> = Vec::new();
    let mut p_off = Vec::with_capacity(n_regions);
    let mut c_off = Vec::with_capacity(n_regions);
    let mut cell_region = Vec::new();
    for (g, d) in duals.iter().enumerate() {
        p_off.push(pts.len());
        c_off.push(cell_region.len());
        pts.extend_from_slice(&d.points);
        cell_region.extend(core::iter::repeat(g).take(d.n_cells));
    }
    let (mut lo, mut hi) = (Vec3::new(f64::MAX, f64::MAX, f64::MAX), Vec3::new(f64::MIN, f64::MIN, f64::MIN));
    for p in &pts {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let mut merge = PointMerge { tol: 1e-9 * hi.sub(lo).length().max(1e-300), grid: BTreeMap::new() };
    let mut rep: Vec<usize> = (0..pts.len()).collect();
    let is_interface = |name: &str| s.patch_names.iter().position(|n| n == name).is_some_and(|k| s.interface[k]);
    // Merge the points of every interface face.
    for (g, d) in duals.iter().enumerate() {
        for p in d.patches.iter().filter(|p| is_interface(&p.name)) {
            for f in p.start_face..p.start_face + p.n_faces {
                for &v in &d.faces[f] {
                    let gi = p_off[g] + v;
                    if rep[gi] == gi {
                        rep[gi] = merge.find_or_insert(gi, pts[gi], &pts);
                    }
                }
            }
        }
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for (g, d) in duals.iter().enumerate() {
        for f in 0..d.face_count() {
            if let Some(nb) = d.neighbour[f] {
                faces.push(d.faces[f].iter().map(|&v| rep[p_off[g] + v]).collect());
                owner.push(c_off[g] + d.owner[f]);
                neighbour.push(Some(c_off[g] + nb));
            }
        }
    }
    // Stitch: pair interface faces by their merged point sets.
    let mut by_key: BTreeMap<Vec<usize>, Vec<(usize, usize, String)>> = BTreeMap::new();
    for (g, d) in duals.iter().enumerate() {
        for p in d.patches.iter().filter(|p| is_interface(&p.name)) {
            for f in p.start_face..p.start_face + p.n_faces {
                let mut k: Vec<usize> = d.faces[f].iter().map(|&v| rep[p_off[g] + v]).collect();
                k.sort_unstable();
                by_key.entry(k).or_default().push((g, f, p.name.clone()));
            }
        }
    }
    let mut stitched: BTreeMap<String, usize> = BTreeMap::new();
    let mut unmatched = 0usize;
    for list in by_key.values() {
        match list.as_slice() {
            [(ga, fa, name), (gb, fb, _)] if ga != gb => {
                let (da, db) = (&duals[*ga], &duals[*gb]);
                faces.push(da.faces[*fa].iter().map(|&v| rep[p_off[*ga] + v]).collect());
                owner.push(c_off[*ga] + da.owner[*fa]);
                neighbour.push(Some(c_off[*gb] + db.owner[*fb]));
                *stitched.entry(name.clone()).or_default() += 1;
            }
            _ => unmatched += list.len(),
        }
    }
    if unmatched > 0 {
        return Err(format!("{unmatched} interface faces have no partner across the interface; the duals do not conform"));
    }
    // Outer patches, in the surface's patch order.
    let mut patches = Vec::new();
    for (k, name) in s.patch_names.iter().enumerate() {
        if s.interface[k] {
            continue;
        }
        let start = faces.len();
        for (g, d) in duals.iter().enumerate() {
            for p in d.patches.iter().filter(|p| p.name == *name) {
                for f in p.start_face..p.start_face + p.n_faces {
                    faces.push(d.faces[f].iter().map(|&v| rep[p_off[g] + v]).collect());
                    owner.push(c_off[g] + d.owner[f]);
                    neighbour.push(None);
                }
            }
        }
        if faces.len() > start {
            patches.push(BoundaryPatch { name: name.clone(), start_face: start, n_faces: faces.len() - start });
        }
    }
    // Compact the points the merge left unused.
    let mut used = vec![usize::MAX; pts.len()];
    let mut points = Vec::new();
    for f in faces.iter_mut() {
        for v in f.iter_mut() {
            if used[*v] == usize::MAX {
                used[*v] = points.len();
                points.push(pts[*v]);
            }
            *v = used[*v];
        }
    }
    let n_cells = cell_region.len();
    let mesh = crate::renumber_mesh::upper_triangular(&VolumeMesh { points, faces, owner, neighbour, n_cells, patches, cells: None });
    mesh.validate().map_err(|e| format!("stitched multi-region dual: {e}"))?;
    check_positive_volumes(&mesh, "stitched multi-region dual")?;
    let mut interface_faces: Vec<(String, usize)> = s
        .patch_names
        .iter()
        .zip(&s.interface)
        .filter(|(_, &i)| i)
        .map(|(n, _)| (n.clone(), stitched.get(n).copied().unwrap_or(0)))
        .collect();
    interface_faces.push(("all interfaces".into(), stitched.values().sum()));
    notes.insert(
        0,
        format!(
            "TetGen tetrahedra with region attributes ({} tetrahedra), each region dualled separately and stitched: {} interface faces",
            out.tets.len(),
            stitched.values().sum::<usize>()
        ),
    );
    Ok(MultiRegionMesh { mesh, cell_region, interface_faces, notes })
}

/// [`multi_region_tetgen`], then [`multi_region_dual_mesh`] (`dual`) or
/// [`multi_region_tet_mesh`].
///
/// # Errors
/// As the three functions.
pub fn multi_region_mesh(s: &MultiRegionSurface, source: TetSource, dual: bool) -> Result<MultiRegionMesh, String> {
    let out = multi_region_tetgen(s, source)?;
    let mut m = if dual { multi_region_dual_mesh(s, &out)? } else { multi_region_tet_mesh(s, &out)? };
    m.notes.push(format!("TetGen -{}A", source.switches().unwrap_or_default()));
    Ok(m)
}

/// Every cell's volume (divergence theorem over its faces).
#[must_use]
pub fn cell_volumes(mesh: &VolumeMesh) -> Vec<f64> {
    let mut v = vec![0.0; mesh.n_cells];
    for f in 0..mesh.face_count() {
        let c = mesh.face_centroid(f).dot(mesh.face_area_vector(f)) / 3.0;
        v[mesh.owner[f]] += c;
        if let Some(nb) = mesh.neighbour[f] {
            v[nb] -= c;
        }
    }
    v
}

/// `Err` naming the cells whose volume is not positive: a closed cell
/// wound inside out passes [`VolumeMesh::validate`] and would hide a hole
/// of the same volume elsewhere.
fn check_positive_volumes(mesh: &VolumeMesh, what: &str) -> Result<(), String> {
    let v = cell_volumes(mesh);
    let bad: Vec<usize> = (0..v.len()).filter(|&c| !(v[c] > 0.0)).collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("{what}: {} cells have non-positive volume (first: cell {}, {:e})", bad.len(), bad[0], v[bad[0]]))
    }
}

/// Volume of every region of `mesh` (divergence theorem over the faces).
#[must_use]
pub fn region_volumes(mesh: &VolumeMesh, cell_region: &[usize], n_regions: usize) -> Vec<f64> {
    let mut v = vec![0.0; n_regions];
    for f in 0..mesh.face_count() {
        let c = mesh.face_centroid(f).dot(mesh.face_area_vector(f)) / 3.0;
        v[cell_region[mesh.owner[f]]] += c;
        if let Some(nb) = mesh.neighbour[f] {
            v[cell_region[nb]] -= c;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles per quad `[a, b, c, d]`.
    fn quad(t: &mut Vec<[usize; 3]>, m: &mut Vec<usize>, q: [usize; 4], marker: usize) {
        t.push([q[0], q[1], q[2]]);
        t.push([q[0], q[2], q[3]]);
        m.push(marker);
        m.push(marker);
    }

    /// The six faces of the box with corner indices `c[x][y][z]`.
    fn box_faces(t: &mut Vec<[usize; 3]>, m: &mut Vec<usize>, c: [[[usize; 2]; 2]; 2], marker: [usize; 6]) {
        quad(t, m, [c[0][0][0], c[0][1][0], c[0][1][1], c[0][0][1]], marker[0]); // -x
        quad(t, m, [c[1][0][0], c[1][0][1], c[1][1][1], c[1][1][0]], marker[1]); // +x
        quad(t, m, [c[0][0][0], c[0][0][1], c[1][0][1], c[1][0][0]], marker[2]); // -y
        quad(t, m, [c[0][1][0], c[1][1][0], c[1][1][1], c[0][1][1]], marker[3]); // +y
        quad(t, m, [c[0][0][0], c[1][0][0], c[1][1][0], c[0][1][0]], marker[4]); // -z
        quad(t, m, [c[0][0][1], c[0][1][1], c[1][1][1], c[1][0][1]], marker[5]); // +z
    }

    /// Two unit cubes side by side along x, sharing the face x = 1.
    fn two_cubes() -> MultiRegionSurface {
        let mut points = Vec::new();
        let mut id = [[[0usize; 2]; 2]; 3];
        for (i, x) in [0.0, 1.0, 2.0].into_iter().enumerate() {
            for j in 0..2 {
                for k in 0..2 {
                    id[i][j][k] = points.len();
                    points.push(Vec3::new(x, j as f64, k as f64));
                }
            }
        }
        let (mut tris, mut marker) = (Vec::new(), Vec::new());
        let a = [[id[0][0], id[0][1]], [id[1][0], id[1][1]]];
        let b = [[id[1][0], id[1][1]], [id[2][0], id[2][1]]];
        box_faces(&mut tris, &mut marker, a, [0, 2, 0, 0, 0, 0]);
        // cube b: its -x face is the interface, already emitted by cube a.
        let mut tb = Vec::new();
        let mut mb = Vec::new();
        box_faces(&mut tb, &mut mb, b, [2, 1, 1, 1, 1, 1]);
        for (t, m) in tb.into_iter().zip(mb).skip(2) {
            tris.push(t);
            marker.push(m);
        }
        MultiRegionSurface {
            points,
            tris,
            marker,
            patch_names: vec!["wall_a".into(), "wall_b".into(), "a_to_b".into()],
            interface: vec![false, false, true],
            region_names: vec!["a".into(), "b".into()],
            seeds: vec![(Vec3::new(0.5, 0.5, 0.5), 0), (Vec3::new(1.5, 0.5, 0.5), 1)],
            holes: Vec::new(),
        }
    }

    /// A unit cube (region 1) inside a cube of side 3 (region 0): the
    /// interface does not touch the outer boundary.
    fn nested_cubes() -> MultiRegionSurface {
        let mut points = Vec::new();
        let mut corners = |lo: f64, hi: f64| {
            let mut c = [[[0usize; 2]; 2]; 2];
            for (i, x) in [lo, hi].into_iter().enumerate() {
                for (j, y) in [lo, hi].into_iter().enumerate() {
                    for (k, z) in [lo, hi].into_iter().enumerate() {
                        c[i][j][k] = points.len();
                        points.push(Vec3::new(x, y, z));
                    }
                }
            }
            c
        };
        let outer = corners(0.0, 3.0);
        let inner = corners(1.0, 2.0);
        let (mut tris, mut marker) = (Vec::new(), Vec::new());
        box_faces(&mut tris, &mut marker, outer, [0; 6]);
        box_faces(&mut tris, &mut marker, inner, [1; 6]);
        MultiRegionSurface {
            points,
            tris,
            marker,
            patch_names: vec!["outside".into(), "shell_to_core".into()],
            interface: vec![false, true],
            region_names: vec!["shell".into(), "core".into()],
            seeds: vec![(Vec3::new(0.5, 0.5, 0.5), 0), (Vec3::new(1.5, 1.5, 1.5), 1)],
            holes: Vec::new(),
        }
    }

    fn source() -> TetSource {
        TetSource::tetgen(1.414, Some(0.01))
    }

    /// Every vertex of every cell of `region` satisfies `inside`.
    fn cells_stay_inside(m: &MultiRegionMesh, region: usize, inside: impl Fn(Vec3) -> bool) -> bool {
        let lists = m.mesh.cell_face_lists();
        (0..m.mesh.n_cells).filter(|&c| m.cell_region[c] == region).all(|c| {
            lists[c].iter().all(|&f| m.mesh.faces[f].iter().all(|&v| inside(m.mesh.points[v])))
        })
    }

    fn check(m: &MultiRegionMesh, exact: &[f64], outer: &[&str]) {
        m.mesh.validate().expect("closed");
        let v = region_volumes(&m.mesh, &m.cell_region, exact.len());
        for (g, (got, want)) in v.iter().zip(exact).enumerate() {
            assert!((got - want).abs() < 1e-9 * want, "region {g}: {got} vs {want}");
        }
        assert!((m.mesh.total_volume() - exact.iter().sum::<f64>()).abs() < 1e-9);
        let names: Vec<&str> = m.mesh.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, outer, "only the outer patches are patches");
        assert!(m.interface_faces.last().unwrap().1 > 0, "interface faces present");
    }

    /// The shell of the nested cubes meshed alone (2026-11-27): the core's
    /// faces become an outer patch, a hole point inside the core removes it,
    /// and the dual has exactly the shell's volume (27 - 1). Without the hole
    /// the core is meshed with no seed and refused.
    #[test]
    fn a_region_meshed_alone_keeps_its_cavity_as_a_hole() {
        let mut s = nested_cubes();
        s.interface = vec![false, false];
        s.region_names.truncate(1);
        s.seeds.truncate(1);
        let err = multi_region_mesh(&s, source(), true).unwrap_err();
        assert!(err.contains("no seed point"), "{err}");
        s.holes = vec![Vec3::new(1.5, 1.5, 1.5)];
        let m = multi_region_mesh(&s, source(), true).unwrap();
        m.mesh.validate().expect("closed");
        assert!((m.mesh.total_volume() - 26.0).abs() < 1e-9, "{}", m.mesh.total_volume());
        let names: Vec<&str> = m.mesh.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["outside", "shell_to_core"]);
    }

    #[test]
    fn two_cubes_tet_mesh_is_fitted_at_the_interface() {
        let m = multi_region_mesh(&two_cubes(), source(), false).unwrap();
        check(&m, &[1.0, 1.0], &["wall_a", "wall_b"]);
        assert!(cells_stay_inside(&m, 0, |p| p.x <= 1.0 + 1e-12));
        assert!(cells_stay_inside(&m, 1, |p| p.x >= 1.0 - 1e-12));
    }

    #[test]
    fn two_cubes_dual_mesh_is_fitted_and_stitched() {
        let s = two_cubes();
        let out = multi_region_tetgen(&s, source()).unwrap();
        let tet = multi_region_tet_mesh(&s, &out).unwrap();
        let m = multi_region_dual_mesh(&s, &out).unwrap();
        check(&m, &[1.0, 1.0], &["wall_a", "wall_b"]);
        assert!(cells_stay_inside(&m, 0, |p| p.x <= 1.0 + 1e-12));
        assert!(cells_stay_inside(&m, 1, |p| p.x >= 1.0 - 1e-12));
        // One dual cell per primal vertex per region: the interface vertices
        // count twice.
        let on_iface = out.points.iter().filter(|p| (p[0] - 1.0).abs() < 1e-12).count();
        assert_eq!(m.mesh.n_cells, out.points.len() + on_iface);
        // The stitched faces tile the interface: their area is the unit square.
        let area: f64 = (0..m.mesh.face_count())
            .filter(|&f| matches!(m.mesh.neighbour[f], Some(nb) if m.cell_region[nb] != m.cell_region[m.mesh.owner[f]]))
            .map(|f| m.mesh.face_area_vector(f).length())
            .sum();
        assert!((area - 1.0).abs() < 1e-9, "interface area {area}");
        assert!(tet.mesh.n_cells == out.tets.len());
    }

    #[test]
    fn nested_cubes_both_routes() {
        let inside_core = |p: Vec3| [p.x, p.y, p.z].iter().all(|&c| (1.0 - 1e-12..=2.0 + 1e-12).contains(&c));
        let outside_core = |p: Vec3| [p.x, p.y, p.z].iter().any(|&c| c <= 1.0 + 1e-12 || c >= 2.0 - 1e-12);
        for dual in [false, true] {
            let m = multi_region_mesh(&nested_cubes(), source(), dual).unwrap();
            check(&m, &[26.0, 1.0], &["outside"]);
            assert!(cells_stay_inside(&m, 1, inside_core), "dual {dual}: core cells inside the core");
            assert!(cells_stay_inside(&m, 0, outside_core), "dual {dual}: shell cells outside the core");
        }
    }

    #[test]
    fn a_missing_seed_and_the_carved_source_are_refused() {
        let mut s = nested_cubes();
        s.seeds.pop();
        assert!(multi_region_mesh(&s, source(), true).unwrap_err().contains("no seed"));
        assert!(multi_region_mesh(&nested_cubes(), TetSource::CarvedCentroid, true).is_err());
    }
}

