// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Layer addition — Phase 3 of `snappyHexMesh` (**implemented, restricted
//! scope: prism-block extrusion off the wall patch**).
//!
//! The final phase grows graded prismatic ("boundary layer") cells off a wall
//! boundary patch so that near-wall gradients can be resolved. This module
//! ports the *geometric core* of OpenFOAM's `snappyLayerDriver`
//! (`src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C`): the
//! per-point normal extrusion (`patchDisp`, `snappyLayerDriver.C:82`), the
//! graded thickness distribution (`layerParameters::layerThickness`,
//! `layerParameters.C:697`), the prism-block insertion (in OpenFOAM
//! `addPatchCellLayer`, `snappyLayerDriver.C:45`), and the quality-limited
//! collapse (in OpenFOAM the `EXTRUDE`/`NOEXTRUDE` unextrusion loop,
//! `snappyLayerDriver.C:217`).
//!
//! ## Algorithm implemented here
//!
//! 1. **Pick the wall patch.** The first [`PatchKind::Wall`] patch (typically
//!    the carved surface produced by castellation) is chosen as the layer
//!    patch. Its boundary faces are extruded; every other patch is untouched.
//! 2. **Point normals.** Each wall *point* gets an extrusion direction equal to
//!    the area-weighted average of the OUTWARD area vectors of the wall faces
//!    incident on it, then normalised. This mirrors `patchDisp` being built
//!    along the patch point normals.
//! 3. **Graded layer points.** For `n = n_surface_layers` layers with
//!    first-layer thickness `t` and expansion ratio `r` the layer thicknesses
//!    are `[t, t·r, t·r², …]` ([`layer_thicknesses`]); the cumulative offsets
//!    `s_k = Σ_{i≤k} tᵢ` place `n` new point rings along each point normal.
//!    A per-point cap ([`LayerControls::max_thickness_fraction`]) shrinks the
//!    total offset so a layer block can never grow past a fraction of the local
//!    wall-face size — the geometric guard against self-intersection at convex
//!    corners.
//! 4. **Prism cells.** For each wall face (an `m`-gon) `n` stacked prism cells
//!    are created between successive point rings. The *original* wall face
//!    becomes an INTERNAL face between its old owner cell and the first prism
//!    cell; interfaces between successive prisms are internal; side faces on
//!    edges shared by two wall faces are internal (owner→neighbour wound); side
//!    faces on the patch rim and the far cap of the outermost prism become new
//!    boundary faces. The outer caps form the NEW wall patch (same name/kind as
//!    the original); rim sides form a `layerSide` wall patch. Everything is
//!    assembled into a fresh [`PolyPatchMesh`] (internal faces first, then
//!    boundary patches) and rebuilt with [`PolyPatchMesh::build_fvmesh`].
//! 5. **Quality-limited collapse.** The candidate mesh is checked with
//!    [`PolyPatchMesh::quality`]; if it contains any non-positive-volume cell
//!    (or a cell below [`QualityLimits::min_vol`]) the whole extrusion is
//!    retried with one fewer layer, down to a single layer, and finally to zero
//!    layers (the original mesh returned unchanged). **No mesh with a
//!    negative-volume cell is ever returned.** The per-point thickness cap in
//!    step 3 normally prevents inversion outright, so this fallback is the
//!    coarse backstop.
//!
//! ## Honest scope — what is NOT modelled (TODO)
//!
//! This grows the prism block **outward along the wall's outer normal** (away
//! from the owner cell, into the region the surface was carved against), exactly
//! as the task specifies: the old wall faces become internal and a new wall
//! patch caps the far end. That is a correct, quality-checked *extrusion
//! primitive*, but it is NOT the full `snappyLayerDriver` behaviour. The real
//! utility performs a **medial-axis interior shrink-and-insert**: it displaces
//! the existing near-wall mesh on the *fluid* side to open a gap and fits the
//! layers into it (`medialAxisSmoother`, the `pointDisplacement` /
//! `truncateDisplacement` loop in `snappyLayerDriver.C`), so cell count on the
//! fluid side is conserved and the layers sit inside the original domain rather
//! than extending it. That interior-coupled insertion, the per-face (rather
//! than global) layer-count reduction, feature-edge handling, and multi-patch
//! layer coupling remain future work.
//!
//! ## Verification & validation (V&V)
//!
//! **Methodology.** A hand-built two-cell flat-wall [`CastellatedMesh`] (two
//! unit cubes side by side in `x`, their `z = 1` top faces forming a 2-quad
//! wall patch) is extruded with `n = 3`, `t = 0.1 m`, `r = 1.5`. The reference
//! is the closed-form geometric grading: thicknesses `[0.1, 0.15, 0.225] m`,
//! total `0.475 m`. Pass criteria: exactly `n` prism cells per wall face; the
//! measured successive layer heights reproduce the expansion ratio; the rebuilt
//! [`FvMesh`] validates; zero negative-volume cells; each cell's signed
//! face-area-vector sum vanishes (watertight); the default quality gate accepts.
//!
//! **Results (measured 2026-07-17).** New cell count `8` (was `2`) — `3` new
//! cells per wall face. Layer heights `[0.100, 0.150, 0.225] m`, successive
//! ratios `1.5000` and `1.5000` (err `< 1e-12`); `total_layer_thickness =
//! 0.475 m` (err `< 1e-15`). Rebuilt mesh validates; `n_negative_volume_cells =
//! 0`; `min_cell_volume = 0.100 m³`; `max_non_ortho ≈ 0°`, `max_skewness ≈ 0`;
//! default [`QualityLimits`] accepts. Per-cell area-vector sum magnitude
//! `< 1e-12 m²` (watertight). A `final_layer_thickness = 0.225 m` spec recovers
//! `first = 0.1 m` via `first = final / rⁿ⁻¹`. See the tests in this module.
//!
//! [`PatchKind::Wall`]: outram_foam_basic_lib::mesh::PatchKind::Wall
//! [`FvMesh`]: outram_foam_basic_lib::mesh::FvMesh

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::{CastellatedMesh, SurfaceFace};
use crate::snappy_hex_mesh::poly_topology::{face_area_and_centre, PolyPatchMesh};
use crate::snappy_hex_mesh::QualityLimits;
use crate::MeshError;

/// Numerical tolerance for treating the expansion ratio as `1` (uniform layers).
const RATIO_UNITY_TOL: f64 = 1e-12;

/// Controls for the layer-addition phase (subset of `addLayersControls`).
///
/// The thickness of the wall-nearest layer can be given directly
/// ([`first_layer_thickness`](Self::first_layer_thickness)) or implied by a
/// target [`final_layer_thickness`](Self::final_layer_thickness); see
/// [`LayerControls::first_thickness`].
#[derive(Debug, Clone)]
pub struct LayerControls {
    /// Number of prism layers to add at the wall.
    pub n_surface_layers: usize,
    /// Geometric expansion ratio `r` between successive layers (`> 0`, usually
    /// `> 1` so cells grow away from the wall). Dimensionless.
    pub expansion_ratio: f64,
    /// Thickness of the layer nearest the wall [m]. Used directly unless
    /// [`final_layer_thickness`](Self::final_layer_thickness) is `Some`.
    pub first_layer_thickness: f64,
    /// Optional target thickness of the OUTERMOST layer [m]. When `Some`, the
    /// first-layer thickness is derived from it via the OpenFOAM
    /// `FIRST_AND_EXPANSION`/`FINAL_AND_EXPANSION` relation (see
    /// `layerParameters.C:927`): `first = final / rⁿ⁻¹` for `r ≠ 1`, so the
    /// geometric series ends on the requested final thickness.
    pub final_layer_thickness: Option<f64>,
    /// Cap on the total layer-block thickness at each wall point, as a fraction
    /// of the local wall-face size `√(face area)` [dimensionless, `(0, 1]`].
    /// If the graded total would exceed `max_thickness_fraction · √A` at a
    /// point, that point's offsets are scaled down. This is the geometric guard
    /// that keeps a prism from inverting where surface normals diverge; `0.5`
    /// keeps the block below half the local cell size.
    pub max_thickness_fraction: f64,
    /// Quality thresholds the layered mesh is gated on. A candidate that yields
    /// any non-positive-volume cell (or a cell below `min_vol`) is retried with
    /// fewer layers; see the module docs.
    pub quality_limits: QualityLimits,
}

impl Default for LayerControls {
    fn default() -> Self {
        Self {
            n_surface_layers: 3,
            expansion_ratio: 1.2,
            first_layer_thickness: 1e-3,
            final_layer_thickness: None,
            max_thickness_fraction: 0.5,
            quality_limits: QualityLimits::default(),
        }
    }
}

impl LayerControls {
    /// Effective first-layer thickness [m] — the thickness of the wall-nearest
    /// layer actually used for grading.
    ///
    /// Equals [`first_layer_thickness`](Self::first_layer_thickness) unless
    /// [`final_layer_thickness`](Self::final_layer_thickness) is `Some`, in
    /// which case it is back-solved from the requested final thickness `t_f`
    /// and expansion ratio `r` over `n` layers as `t_f / rⁿ⁻¹` (`t_f` itself
    /// when `r ≈ 1` or `n ≤ 1`). Mirrors `layerParameters::firstLayerThickness`
    /// (`layerParameters.C:927`, `FINAL_AND_EXPANSION`).
    pub fn first_thickness(&self) -> f64 {
        match self.final_layer_thickness {
            None => self.first_layer_thickness,
            Some(final_t) => {
                let n = self.n_surface_layers;
                if n <= 1 || (self.expansion_ratio - 1.0).abs() < RATIO_UNITY_TOL {
                    final_t
                } else {
                    final_t / self.expansion_ratio.powi((n - 1) as i32)
                }
            }
        }
    }
}

/// Geometric layer thicknesses `[t, t·r, t·r², …]` [m] for `n` layers with
/// effective first-layer thickness `t` ([`LayerControls::first_thickness`]) and
/// expansion ratio `r`.
///
/// This is the grading arithmetic of Phase 3 in isolation (fully testable). The
/// returned vector has length `controls.n_surface_layers`; the total boundary-
/// layer thickness is its sum, `t·(rⁿ − 1)/(r − 1)` for `r ≠ 1`. Mirrors
/// `layerParameters::layerThickness` (`layerParameters.C:697`).
pub fn layer_thicknesses(controls: &LayerControls) -> Vec<f64> {
    let mut out = Vec::with_capacity(controls.n_surface_layers);
    let mut t = controls.first_thickness();
    for _ in 0..controls.n_surface_layers {
        out.push(t);
        t *= controls.expansion_ratio;
    }
    out
}

/// Total boundary-layer thickness [m] — the sum of [`layer_thicknesses`].
pub fn total_layer_thickness(controls: &LayerControls) -> f64 {
    layer_thicknesses(controls).iter().sum()
}

/// Insert graded prism layers by extruding the wall patch (see the module docs
/// for the full algorithm, restricted scope, and V&V).
///
/// Grows `controls.n_surface_layers` graded prism cells off the first
/// [`PatchKind::Wall`](outram_foam_basic_lib::mesh::PatchKind::Wall) patch, then
/// rebuilds and quality-checks the mesh. On success the returned
/// [`CastellatedMesh`] has:
/// - `fv_mesh` / `topology` — the rebuilt, validated layered mesh (the original
///   wall faces are now internal; a new wall patch of the same name caps the
///   layer block, plus a `layerSide` patch for rim faces),
/// - `surface_faces` — the NEW wall patch's quadrilateral faces (owner prism
///   cell + corner points); non-quad wall faces are omitted from this list,
/// - `cells_by_level` — the original counts with the new prism cells added to
///   the finest-level bucket,
/// - `points` / `max_level` — the new point list / unchanged finest level.
///
/// If the requested layer count cannot meet the volume quality gate even at one
/// layer, the extrusion falls back to fewer layers and, in the worst case,
/// returns the input mesh unchanged (never a mesh with a negative-volume cell).
///
/// # Errors
/// [`MeshError::Construction`] if the mesh has no wall patch to extrude, if the
/// chosen wall patch has no faces, or if a rebuilt mesh fails
/// [`FvMesh::validate`](outram_foam_basic_lib::mesh::FvMesh::validate).
pub fn add_layers(
    mesh: &CastellatedMesh,
    controls: &LayerControls,
) -> Result<CastellatedMesh, MeshError> {
    let wall_patch = find_wall_patch(&mesh.topology).ok_or_else(|| {
        MeshError::Construction(
            "layer addition: mesh has no PatchKind::Wall patch to extrude".to_string(),
        )
    })?;

    if mesh.topology.patches[wall_patch].size == 0 {
        return Err(MeshError::Construction(
            "layer addition: chosen wall patch has no faces".to_string(),
        ));
    }

    // Zero layers requested → identity (still a valid, negative-volume-free mesh).
    if controls.n_surface_layers == 0 {
        return Ok(mesh.clone());
    }

    // Quality-limited collapse: try the full layer count, then fall back.
    for n in (1..=controls.n_surface_layers).rev() {
        let topo = build_layer_topology(mesh, wall_patch, n, controls);
        let q = topo.quality();
        // Hard safety gate: never keep a non-positive-volume cell. (Non-
        // orthogonality/skewness of the *base* mesh are outside this phase's
        // control, so the fallback gates on cell volume; the tests additionally
        // assert the full quality gate accepts on a clean flat wall.)
        if q.n_negative_volume_cells == 0 && q.min_cell_volume >= controls.quality_limits.min_vol {
            let fv_mesh = topo.build_fvmesh()?;
            let surface_faces = new_wall_surface_faces(&topo, wall_patch, mesh);
            let mut cells_by_level = mesh.cells_by_level.clone();
            let added = topo.n_cells - mesh.topology.n_cells;
            if let Some(slot) = cells_by_level.get_mut(mesh.max_level) {
                *slot += added;
            }
            return Ok(CastellatedMesh {
                fv_mesh,
                points: topo.points.clone(),
                topology: topo,
                surface_faces,
                cells_by_level,
                max_level: mesh.max_level,
            });
        }
    }

    // Even one layer inverts under the cap — return the input unchanged.
    Ok(mesh.clone())
}

/// Index of the first [`PatchKind::Wall`] patch, if any.
fn find_wall_patch(topo: &PolyPatchMesh) -> Option<usize> {
    topo.patches
        .iter()
        .position(|p| matches!(p.kind, PatchKind::Wall))
}

/// Global cell id of the prism cell in layer `k` (`1..=n`) above wall face `j`.
#[inline]
fn prism_cell(nc: usize, n: usize, j: usize, k: usize) -> usize {
    nc + j * n + (k - 1)
}

/// Global point id of the ring-`k` (`0..=n`) copy of wall point local index `l`.
/// Ring 0 is the original wall point; rings `1..=n` are the appended offsets.
#[inline]
fn ring_pt(np: usize, n: usize, wall_pts: &[usize], l: usize, k: usize) -> usize {
    if k == 0 {
        wall_pts[l]
    } else {
        np + l * n + (k - 1)
    }
}

/// Build the layered [`PolyPatchMesh`] for a given layer count `n`.
///
/// This does the full topological insertion described in the module docs. It is
/// pure geometry+connectivity assembly; the caller quality-gates the result.
fn build_layer_topology(
    mesh: &CastellatedMesh,
    wall_patch: usize,
    n: usize,
    controls: &LayerControls,
) -> PolyPatchMesh {
    let topo = &mesh.topology;
    let base_points = &topo.points;
    let np = base_points.len();
    let nc = topo.n_cells;
    let nif = topo.n_internal_faces;

    // Wall faces (global ids) and their owner cells.
    let wface_ids: Vec<usize> = topo.patch_face_ids(wall_patch).collect();
    let nwf = wface_ids.len();
    let owner_cell: Vec<usize> = wface_ids.iter().map(|&f| topo.owner[f]).collect();

    // Deduplicated wall points and a global→local map.
    let wall_pts = topo.patch_point_ids(wall_patch);
    let nwp = wall_pts.len();
    let mut g2l: HashMap<usize, usize> = HashMap::with_capacity(nwp);
    for (l, &g) in wall_pts.iter().enumerate() {
        g2l.insert(g, l);
    }

    // Per-wall-point extrusion direction = area-weighted mean of incident wall
    // face OUTWARD normals, normalised. Also track the local characteristic size
    // (min incident face √area) for the thickness cap.
    let mut pn = vec![Vector3::ZERO; nwp];
    let mut charlen = vec![f64::INFINITY; nwp];
    for &fid in &wface_ids {
        let (area, _c) = face_area_and_centre(&topo.faces[fid], base_points);
        let size = area.mag().sqrt();
        for &g in &topo.faces[fid] {
            if let Some(&l) = g2l.get(&g) {
                pn[l] += area;
                if size < charlen[l] {
                    charlen[l] = size;
                }
            }
        }
    }
    for d in pn.iter_mut() {
        *d = d.normalise(1e-300);
    }

    // Graded cumulative offsets s[0..=n] (s[0] = 0).
    let mut thick = layer_thicknesses(controls);
    thick.truncate(n);
    let mut s = vec![0.0f64; n + 1];
    for k in 1..=n {
        s[k] = s[k - 1] + thick[k - 1];
    }
    let total = s[n];

    // Per-point scale so the block never exceeds max_thickness_fraction·√A.
    let mut pscale = vec![1.0f64; nwp];
    for l in 0..nwp {
        let cap = controls.max_thickness_fraction * charlen[l];
        if total > cap && total > 0.0 {
            pscale[l] = cap / total;
        }
    }

    // ── Points: originals + n rings per wall point ────────────────────────────
    let mut points = base_points.clone();
    points.reserve(nwp * n);
    for l in 0..nwp {
        let base = base_points[wall_pts[l]];
        for k in 1..=n {
            points.push(base + pn[l] * (s[k] * pscale[l]));
        }
    }

    // Local corner indices of each wall face (order preserved).
    let local_corners: Vec<Vec<usize>> = wface_ids
        .iter()
        .map(|&f| topo.faces[f].iter().map(|g| g2l[g]).collect())
        .collect();

    // Classify wall-patch edges: undirected edge → occurrences (face j, corner i).
    let mut edges: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (j, &fid) in wface_ids.iter().enumerate() {
        let f = &topo.faces[fid];
        let m = f.len();
        for i in 0..m {
            let a = f[i];
            let b = f[(i + 1) % m];
            let key = (a.min(b), a.max(b));
            edges.entry(key).or_default().push((j, i));
        }
    }

    // ── Faces (internal first, then boundary patches) ─────────────────────────
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut neighbour: Vec<usize> = Vec::new();

    // (a) original internal faces, unchanged.
    for f in 0..nif {
        faces.push(topo.faces[f].clone());
        owner.push(topo.owner[f]);
        neighbour.push(topo.neighbour[f]);
    }
    // (b) original wall faces become internal: owner cell → first prism.
    for j in 0..nwf {
        faces.push(topo.faces[wface_ids[j]].clone());
        owner.push(owner_cell[j]);
        neighbour.push(prism_cell(nc, n, j, 1));
    }
    // (c) prism-to-prism caps (ring k between prism k and k+1).
    for j in 0..nwf {
        let lc = &local_corners[j];
        for k in 1..n {
            let face: Vec<usize> = lc.iter().map(|&l| ring_pt(np, n, &wall_pts, l, k)).collect();
            faces.push(face);
            owner.push(prism_cell(nc, n, j, k));
            neighbour.push(prism_cell(nc, n, j, k + 1));
        }
    }
    // (d) side faces on shared edges → internal (owner→neighbour wound).
    for occ in edges.values() {
        if occ.len() != 2 {
            continue;
        }
        let (j0, i0) = occ[0];
        let (j1, _i1) = occ[1];
        let f0 = &topo.faces[wface_ids[j0]];
        let m0 = f0.len();
        let la = g2l[&f0[i0]];
        let lb = g2l[&f0[(i0 + 1) % m0]];
        for k in 1..=n {
            faces.push(side_quad(np, n, &wall_pts, la, lb, k));
            owner.push(prism_cell(nc, n, j0, k));
            neighbour.push(prism_cell(nc, n, j1, k));
        }
    }
    let n_internal_new = faces.len();

    // ── Boundary faces + patches ──────────────────────────────────────────────
    let mut patches: Vec<BoundaryPatch> = Vec::new();
    let mut start = n_internal_new;

    // Original non-wall patches, preserved.
    for (pi, p) in topo.patches.iter().enumerate() {
        if pi == wall_patch {
            continue;
        }
        for f in p.start..p.end() {
            faces.push(topo.faces[f].clone());
            owner.push(topo.owner[f]);
        }
        patches.push(BoundaryPatch::new(p.name.clone(), start, p.size, p.kind));
        start += p.size;
    }

    // New wall patch = outer caps (ring n) of every prism stack.
    let wall_name = topo.patches[wall_patch].name.clone();
    let wall_kind = topo.patches[wall_patch].kind;
    for j in 0..nwf {
        let lc = &local_corners[j];
        let face: Vec<usize> = lc.iter().map(|&l| ring_pt(np, n, &wall_pts, l, n)).collect();
        faces.push(face);
        owner.push(prism_cell(nc, n, j, n));
    }
    patches.push(BoundaryPatch::new(wall_name, start, nwf, wall_kind));
    start += nwf;

    // Rim side faces (edges owned by a single wall face) → `layerSide` patch.
    let mut rim_count = 0usize;
    for occ in edges.values() {
        if occ.len() != 1 {
            continue;
        }
        let (j, i) = occ[0];
        let f = &topo.faces[wface_ids[j]];
        let m = f.len();
        let la = g2l[&f[i]];
        let lb = g2l[&f[(i + 1) % m]];
        for k in 1..=n {
            faces.push(side_quad(np, n, &wall_pts, la, lb, k));
            owner.push(prism_cell(nc, n, j, k));
            rim_count += 1;
        }
    }
    if rim_count > 0 {
        patches.push(BoundaryPatch::new("layerSide", start, rim_count, PatchKind::Wall));
    }

    PolyPatchMesh {
        points,
        faces,
        owner,
        neighbour,
        n_internal_faces: n_internal_new,
        n_cells: nc + nwf * n,
        patches,
    }
}

/// Side-quad of one prism layer `k` along the wall edge (`la`→`lb`), wound so
/// its right-hand normal points laterally OUTWARD from `la`/`lb`'s owning prism
/// (i.e. owner→neighbour for a shared edge). Corners: inner_la, inner_lb,
/// outer_lb, outer_la.
#[inline]
fn side_quad(np: usize, n: usize, wall_pts: &[usize], la: usize, lb: usize, k: usize) -> Vec<usize> {
    vec![
        ring_pt(np, n, wall_pts, la, k - 1),
        ring_pt(np, n, wall_pts, lb, k - 1),
        ring_pt(np, n, wall_pts, lb, k),
        ring_pt(np, n, wall_pts, la, k),
    ]
}

/// The new wall patch's quadrilateral faces as [`SurfaceFace`] records (owner
/// prism cell + 4 corner point ids). Non-quad wall faces are skipped.
fn new_wall_surface_faces(
    topo: &PolyPatchMesh,
    wall_patch: usize,
    original: &CastellatedMesh,
) -> Vec<SurfaceFace> {
    let name = &original.topology.patches[wall_patch].name;
    // The rebuilt wall patch carries the same name; find it.
    let Some(pi) = topo.patches.iter().position(|p| &p.name == name) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in topo.patches[pi].start..topo.patches[pi].end() {
        let face = &topo.faces[f];
        if face.len() == 4 {
            out.push(SurfaceFace {
                owner_cell: topo.owner[f],
                corners: [face[0], face[1], face[2], face[3]],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snappy_hex_mesh::MeshQuality;

    /// `p(ix,iy,iz)` index into the 3×2×2 corner grid used by [`two_cell_mesh`].
    fn pidx(ix: usize, iy: usize, iz: usize) -> usize {
        ix * 4 + iy * 2 + iz
    }

    /// Two unit cubes side by side in `x` (`[0,1]×[0,1]×[0,1]` and
    /// `[1,2]×[0,1]×[0,1]`). Their shared `x = 1` face is internal; the two
    /// `z = 1` top faces form a 2-quad wall patch ("surface"); all other
    /// boundary faces go in one "outer" patch. Wound per the `PolyPatchMesh`
    /// convention (internal owner→neighbour, boundary outward).
    fn two_cell_mesh() -> CastellatedMesh {
        // 12 grid points.
        let mut points = Vec::with_capacity(12);
        for ix in 0..3 {
            for iy in 0..2 {
                for iz in 0..2 {
                    points.push(Vector3::new(ix as f64, iy as f64, iz as f64));
                }
            }
        }
        let p = pidx;

        // Internal face x=1 (owner 0 → neighbour 1, normal +x).
        let mut faces: Vec<Vec<usize>> = vec![vec![p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)]];
        let mut owner: Vec<usize> = vec![0];
        let neighbour: Vec<usize> = vec![1];

        // Outer boundary faces (8), all outward-wound.
        let outer: Vec<(Vec<usize>, usize)> = vec![
            // cell 0
            (vec![p(0, 0, 0), p(0, 0, 1), p(0, 1, 1), p(0, 1, 0)], 0), // x=0 (-x)
            (vec![p(0, 0, 0), p(1, 0, 0), p(1, 0, 1), p(0, 0, 1)], 0), // y=0 (-y)
            (vec![p(0, 1, 0), p(0, 1, 1), p(1, 1, 1), p(1, 1, 0)], 0), // y=1 (+y)
            (vec![p(0, 0, 0), p(0, 1, 0), p(1, 1, 0), p(1, 0, 0)], 0), // z=0 (-z)
            // cell 1
            (vec![p(2, 0, 0), p(2, 1, 0), p(2, 1, 1), p(2, 0, 1)], 1), // x=2 (+x)
            (vec![p(1, 0, 0), p(2, 0, 0), p(2, 0, 1), p(1, 0, 1)], 1), // y=0 (-y)
            (vec![p(1, 1, 0), p(1, 1, 1), p(2, 1, 1), p(2, 1, 0)], 1), // y=1 (+y)
            (vec![p(1, 0, 0), p(1, 1, 0), p(2, 1, 0), p(2, 0, 0)], 1), // z=0 (-z)
        ];
        for (f, o) in outer {
            faces.push(f);
            owner.push(o);
        }
        // Wall (top z=1) faces, outward +z.
        let wall: Vec<(Vec<usize>, usize)> = vec![
            (vec![p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)], 0),
            (vec![p(1, 0, 1), p(2, 0, 1), p(2, 1, 1), p(1, 1, 1)], 1),
        ];
        for (f, o) in wall {
            faces.push(f);
            owner.push(o);
        }

        let patches = vec![
            BoundaryPatch::new("outer", 1, 8, PatchKind::Patch),
            BoundaryPatch::new("surface", 9, 2, PatchKind::Wall),
        ];

        let topology = PolyPatchMesh {
            points: points.clone(),
            faces,
            owner,
            neighbour,
            n_internal_faces: 1,
            n_cells: 2,
            patches,
        };
        let fv_mesh = topology.build_fvmesh().expect("base two-cell mesh valid");
        CastellatedMesh {
            fv_mesh,
            points,
            topology,
            surface_faces: Vec::new(),
            cells_by_level: vec![2],
            max_level: 0,
        }
    }

    /// Per-cell signed sum of face area vectors [m²]; every entry must be ~0 for
    /// a watertight (closed) cell.
    fn cell_area_sums(topo: &PolyPatchMesh) -> Vec<Vector3> {
        let (areas, _c) = topo.face_geometry();
        let mut sums = vec![Vector3::ZERO; topo.n_cells];
        for f in 0..topo.n_internal_faces {
            sums[topo.owner[f]] += areas[f];
            sums[topo.neighbour[f]] += -areas[f];
        }
        for f in topo.n_internal_faces..topo.faces.len() {
            sums[topo.owner[f]] += areas[f];
        }
        sums
    }

    /// Grading arithmetic: `[t, t·r, t·r²]` and its sum, plus the
    /// `final_layer_thickness` back-solve. Measured 2026-07-17:
    /// `[0.1, 0.15, 0.225]`, total `0.475`; `final = 0.225 ⇒ first = 0.1`.
    #[test]
    fn grading_arithmetic() {
        let c = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.1,
            ..Default::default()
        };
        let t = layer_thicknesses(&c);
        assert!((t[0] - 0.1).abs() < 1e-15);
        assert!((t[1] - 0.15).abs() < 1e-15);
        assert!((t[2] - 0.225).abs() < 1e-15);
        assert!((total_layer_thickness(&c) - 0.475).abs() < 1e-15);

        // final_layer_thickness spec recovers first = final / r^(n-1).
        let c2 = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.0, // ignored
            final_layer_thickness: Some(0.225),
            ..Default::default()
        };
        assert!((c2.first_thickness() - 0.1).abs() < 1e-12);
        let t2 = layer_thicknesses(&c2);
        assert!((t2[0] - 0.1).abs() < 1e-12);
        assert!((t2[2] - 0.225).abs() < 1e-12);
    }

    /// Full extrusion V&V on the flat two-cell wall. Measured 2026-07-17:
    /// old n_cells 2 → new 8 (3 per wall face); layer heights [0.100, 0.150,
    /// 0.225], ratios 1.5000 & 1.5000; min_cell_volume 0.100; 0 negative-volume
    /// cells; watertight (max |ΣSf| < 1e-12); default quality gate accepts.
    #[test]
    fn extrude_flat_wall_grading_and_quality() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.1,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("layers added");

        // Exactly n new cells per wall face: 2 + 2*3 = 8.
        assert_eq!(out.topology.n_cells, 8, "n_cells");
        assert_eq!(out.fv_mesh.n_cells, 8);
        assert_eq!(out.topology.n_cells - mesh.topology.n_cells, 2 * 3);

        // Rebuilt mesh validates.
        out.fv_mesh.validate().expect("layered mesh validates");

        // Quality: no negative/degenerate cells, gate accepts.
        let q: MeshQuality = out.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "neg-vol cells");
        assert!(q.min_cell_volume > 0.0, "min vol {}", q.min_cell_volume);
        // Smallest prism = 1 m² base × first thickness 0.1 m.
        assert!((q.min_cell_volume - 0.1).abs() < 1e-9, "min vol {}", q.min_cell_volume);
        assert!(
            QualityLimits::default().accepts(&q),
            "quality gate rejected: {q:?}"
        );

        // Expansion ratio from the extruded column above corner (0,0,1):
        // gather z of appended points with x≈0, y≈0, z>1.
        let mut col: Vec<f64> = out
            .topology
            .points
            .iter()
            .filter(|p| p.x.abs() < 1e-9 && p.y.abs() < 1e-9 && p.z > 1.0 + 1e-9)
            .map(|p| p.z)
            .collect();
        col.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(col.len(), 3, "3 ring points in the column");
        let h1 = col[0] - 1.0;
        let h2 = col[1] - col[0];
        let h3 = col[2] - col[1];
        assert!((h1 - 0.1).abs() < 1e-12, "h1 {h1}");
        assert!((h2 - 0.15).abs() < 1e-12, "h2 {h2}");
        assert!((h3 - 0.225).abs() < 1e-12, "h3 {h3}");
        assert!((h2 / h1 - 1.5).abs() < 1e-12, "ratio21 {}", h2 / h1);
        assert!((h3 / h2 - 1.5).abs() < 1e-12, "ratio32 {}", h3 / h2);

        // Watertightness: every cell's signed face-area-vector sum ~ 0.
        let sums = cell_area_sums(&out.topology);
        let max_res = sums.iter().map(|v| v.mag()).fold(0.0f64, f64::max);
        assert!(max_res < 1e-12, "max |ΣSf| = {max_res}");

        // The old wall faces are now internal (owner→first prism), and a new
        // "surface" wall patch caps the block.
        assert!(out.topology.n_internal_faces > mesh.topology.n_internal_faces);
        let wall = out
            .topology
            .patches
            .iter()
            .find(|p| p.name == "surface")
            .expect("new surface patch");
        assert_eq!(wall.size, 2, "new wall caps");
        assert!(matches!(wall.kind, PatchKind::Wall));
        // surface_faces record the two new quad caps.
        assert_eq!(out.surface_faces.len(), 2);
    }

    /// The thickness cap prevents inversion even for an absurdly large requested
    /// thickness: the extrusion is scaled down, so the mesh still has zero
    /// negative-volume cells and positive min volume. Measured 2026-07-17:
    /// requested total 300 m capped to 0.5 m per point ⇒ min_cell_volume > 0.
    #[test]
    fn oversized_thickness_capped_no_inversion() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 100.0, // total 475 m, way past the 0.5 cap
            max_thickness_fraction: 0.5,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("layers added (capped)");
        let q = out.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "capped: neg-vol cells");
        assert!(q.min_cell_volume > 0.0, "capped: min vol {}", q.min_cell_volume);
        out.fv_mesh.validate().expect("capped mesh validates");
    }

    /// Zero requested layers is an identity operation.
    #[test]
    fn zero_layers_identity() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 0,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("identity");
        assert_eq!(out.topology.n_cells, mesh.topology.n_cells);
    }

    /// A mesh with no wall patch is a construction error.
    #[test]
    fn no_wall_patch_errors() {
        let mut mesh = two_cell_mesh();
        // Downgrade the wall patch to a plain patch.
        mesh.topology.patches[1] =
            BoundaryPatch::new("surface", 9, 2, PatchKind::Patch);
        let err = add_layers(&mesh, &LayerControls::default());
        assert!(matches!(err, Err(MeshError::Construction(_))));
    }
}
