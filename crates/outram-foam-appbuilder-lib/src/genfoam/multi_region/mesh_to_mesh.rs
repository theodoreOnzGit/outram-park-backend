// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/meshHandler/
//                    meshHandler.{C,H}, meshHandlerTemplates.C
//                    (the meshToMesh addressing + `map`/`mapTgtToSrc` field
//                     transfer between region meshes)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
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
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Volumetric mesh-to-mesh field mapping
//!
//! Rust port of the *volumetric* cell-to-cell field transfer at the heart of
//! GeN-Foam's `meshHandler` (`meshToMesh` addressing + `map` / `mapTgtToSrc`).
//! Where the neutronics, thermal-hydraulics and thermo-mechanics regions live on
//! **separate, spatially-overlapping meshes** covering the same physical volume,
//! this maps a cell field (temperature, density, power density, mesh
//! displacement) from a *source* region mesh onto a *target* region mesh.
//!
//! This is distinct from [`outram_foam_basic_lib::mesh::RegionInterface`], which
//! couples two regions across a **shared boundary patch** (the
//! `chtMultiRegionFoam` conjugate-heat-transfer pattern). GeN-Foam's multi-region
//! coupling is **volume-overlap**, not face-shared: the whole neutronics mesh
//! overlaps the whole TH mesh, and every cell value is transferred.
//!
//! ## Methods ([`MappingMethod`])
//!
//! For each target cell the mapper precomputes a short list of `(source cell,
//! weight)` contributions (the "addressing", built once at construction, exactly
//! as upstream builds a `meshToMesh` once and reuses it every step):
//!
//! - [`MappingMethod::NearestCell`] — the value of the source cell whose centre
//!   is nearest the target cell centre. Exact for co-located (conformal) meshes,
//!   where the nearest source cell *is* the coincident cell.
//! - [`MappingMethod::InverseDistance`] — inverse-distance-squared blend of the
//!   `n` nearest source cells. Smooths across a refinement change.
//! - [`MappingMethod::CellVolumeWeight`] — upstream's default
//!   (`imCellVolumeWeight`). The *exact* upstream operator integrates the true
//!   polyhedral cell-overlap volumes (a supermesh intersection); basic-lib does
//!   not yet expose that geometry, so this port uses nearest-cell addressing
//!   plus a **global integral-conservation rescale** — a documented degraded
//!   mode that preserves the volume integral `∑ V φ` exactly (the property most
//!   coupling terms rely on) but not the local overlap distribution. Exact
//!   polyhedral overlap is tracked as a scaffolding bead (see the crate
//!   `multi_region` module docs).
//!
//! ## Units
//!
//! The mapper is a linear operator on raw cell values, so it is unit-agnostic —
//! it carries the field's physical meaning without interpreting it, matching how
//! [`outram_foam_basic_lib`] fields (`VolScalarField` = `f64` per cell) are
//! unit-agnostic. The physical `uom` quantities live in the coupling layer that
//! *reads* the mapped fields ([`super::outer_iteration`]).

use std::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

/// How a target cell draws its value from the source mesh.
///
/// Mirrors the interpolation-method choice GeN-Foam's `meshToMesh` constructor
/// takes (upstream hard-codes `imCellVolumeWeight`). See the module docs for the
/// per-variant fidelity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingMethod {
    /// Value of the single nearest source cell (by centre distance).
    NearestCell,
    /// Inverse-distance-squared blend of the `n_neighbours` nearest source cells.
    InverseDistance {
        /// Number of nearest source cells blended (must be `>= 1`).
        n_neighbours: usize,
    },
    /// Cell-volume-weighted conservative transfer (upstream default). This port
    /// approximates it as nearest-cell addressing plus a global integral rescale;
    /// see the module docs.
    CellVolumeWeight,
}

/// How mapped values combine with the existing target-field contents.
///
/// Upstream `meshHandler::map` uses `plusEqOp` — it *accumulates* onto the
/// target (so several source regions can sum into one feedback field). A single
/// clean transfer uses [`MapCombine::Replace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapCombine {
    /// Overwrite the target cell with the mapped value.
    Replace,
    /// Add the mapped value onto the target cell (upstream `plusEqOp`).
    Accumulate,
}

/// A precomputed cell-to-cell mapping from a source mesh onto a target mesh.
///
/// Build once with [`MeshToMesh::new`]; apply every coupling step with
/// [`MeshToMesh::map_scalar`] / [`MeshToMesh::map_vector`]. The addressing
/// (`weights`) is independent of the field, so it is cached — exactly as
/// GeN-Foam caches a `meshToMesh` object across time steps.
#[derive(Debug, Clone)]
pub struct MeshToMesh {
    source: Arc<FvMesh>,
    target: Arc<FvMesh>,
    method: MappingMethod,
    /// `weights[tgt_cell] = [(src_cell, w), ...]`, with `∑ w = 1` for every
    /// target cell (an interpolation, not an extrapolation).
    weights: Vec<Vec<(usize, f64)>>,
}

impl MeshToMesh {
    /// Build the mapping addressing from `source` onto `target` with `method`.
    ///
    /// The source and target meshes must both have cell centres and (for the
    /// conservative rescale) cell volumes populated — every `FvMesh` built
    /// through [`outram_foam_basic_lib::mesh::FvMeshBuilder`] does. Cost is
    /// `O(n_target · n_source)` for the nearest-cell search; for the reactor
    /// meshes this couples that is acceptable, but a spatial acceleration
    /// structure (upstream uses an AABB octree, `pmAABB`) is the scale-up path —
    /// see the `multi_region` module scaffolding beads.
    ///
    /// # Panics
    ///
    /// Panics if the source mesh has no cells (nothing to interpolate from) or if
    /// `InverseDistance { n_neighbours: 0 }` is requested.
    #[must_use]
    pub fn new(source: Arc<FvMesh>, target: Arc<FvMesh>, method: MappingMethod) -> Self {
        assert!(
            source.n_cells > 0,
            "mesh-to-mesh mapping needs a non-empty source mesh"
        );
        let weights = match method {
            MappingMethod::NearestCell | MappingMethod::CellVolumeWeight => target
                .cell_centres
                .iter()
                .map(|&c| vec![(nearest_cell(&source, c), 1.0)])
                .collect(),
            MappingMethod::InverseDistance { n_neighbours } => {
                assert!(
                    n_neighbours >= 1,
                    "inverse-distance mapping needs at least one neighbour"
                );
                target
                    .cell_centres
                    .iter()
                    .map(|&c| inverse_distance_weights(&source, c, n_neighbours))
                    .collect()
            }
        };
        Self {
            source,
            target,
            method,
            weights,
        }
    }

    /// The source mesh this mapping reads from.
    #[must_use]
    pub fn source(&self) -> &Arc<FvMesh> {
        &self.source
    }

    /// The target mesh this mapping writes onto.
    #[must_use]
    pub fn target(&self) -> &Arc<FvMesh> {
        &self.target
    }

    /// The interpolation method in use.
    #[must_use]
    pub fn method(&self) -> MappingMethod {
        self.method
    }

    /// Map a scalar cell field from the source mesh onto `target_field` (which
    /// must live on this mapping's target mesh).
    ///
    /// With [`MapCombine::Replace`] the target internal field is overwritten;
    /// with [`MapCombine::Accumulate`] the mapped values are added on (upstream
    /// `plusEqOp`). When the method is [`MappingMethod::CellVolumeWeight`] and the
    /// combine mode is `Replace`, a global rescale enforces exact conservation of
    /// the volume integral `∑ V φ` (see [`MeshToMesh::integral`]).
    ///
    /// # Panics
    ///
    /// Panics if `source_field`/`target_field` are not sized to this mapping's
    /// source/target meshes.
    pub fn map_scalar(
        &self,
        source_field: &VolScalarField,
        combine: MapCombine,
        target_field: &mut VolScalarField,
    ) {
        assert_eq!(
            source_field.internal.len(),
            self.source.n_cells,
            "source field does not match the mapping's source mesh"
        );
        assert_eq!(
            target_field.internal.len(),
            self.target.n_cells,
            "target field does not match the mapping's target mesh"
        );

        let mut mapped: Vec<f64> = self
            .weights
            .iter()
            .map(|contribs| {
                contribs
                    .iter()
                    .map(|&(src, w)| w * source_field.internal[src])
                    .sum()
            })
            .collect();

        if matches!(self.method, MappingMethod::CellVolumeWeight)
            && matches!(combine, MapCombine::Replace)
        {
            self.conserve_integral(source_field, &mut mapped);
        }

        for (cell, value) in mapped.into_iter().enumerate() {
            match combine {
                MapCombine::Replace => target_field.internal[cell] = value,
                MapCombine::Accumulate => target_field.internal[cell] += value,
            }
        }
    }

    /// Map a vector cell field (e.g. `meshDisp`, velocity) onto `target_field`.
    ///
    /// The interpolation is applied componentwise. The conservative rescale of
    /// [`MappingMethod::CellVolumeWeight`] is **not** applied to vectors (there is
    /// no single scalar integral to conserve) — vectors are interpolated with the
    /// same nearest/inverse-distance addressing.
    ///
    /// # Panics
    ///
    /// Panics if the fields are not sized to this mapping's meshes.
    pub fn map_vector(
        &self,
        source_field: &VolVectorField,
        combine: MapCombine,
        target_field: &mut VolVectorField,
    ) {
        assert_eq!(
            source_field.internal.len(),
            self.source.n_cells,
            "source field does not match the mapping's source mesh"
        );
        assert_eq!(
            target_field.internal.len(),
            self.target.n_cells,
            "target field does not match the mapping's target mesh"
        );

        for (cell, contribs) in self.weights.iter().enumerate() {
            let mut acc = Vector3::new(0.0, 0.0, 0.0);
            for &(src, w) in contribs {
                acc += source_field.internal[src] * w;
            }
            match combine {
                MapCombine::Replace => target_field.internal[cell] = acc,
                MapCombine::Accumulate => target_field.internal[cell] += acc,
            }
        }
    }

    /// Interpolate a scalar field, returning a fresh field on the target mesh.
    ///
    /// Convenience wrapper over [`MeshToMesh::map_scalar`] with
    /// [`MapCombine::Replace`]; `name` labels the produced field.
    #[must_use]
    pub fn interpolate_scalar(&self, source_field: &VolScalarField, name: &str) -> VolScalarField {
        let mut out = VolScalarField::uniform(name, self.target.clone(), 0.0);
        self.map_scalar(source_field, MapCombine::Replace, &mut out);
        out
    }

    /// The volume integral `∑_c V_c φ_c` of a scalar field over a mesh.
    ///
    /// This is the quantity a conservative mapping preserves; the V&V tests
    /// assert `integral(target) == integral(source)` after a
    /// [`MappingMethod::CellVolumeWeight`] transfer.
    #[must_use]
    pub fn integral(mesh: &FvMesh, field: &VolScalarField) -> f64 {
        mesh.cell_volumes
            .iter()
            .zip(field.internal.iter())
            .map(|(&v, &phi)| v * phi)
            .sum()
    }

    /// Rescale `mapped` (values already on the target mesh) so its volume
    /// integral equals the source field's — the global conservation correction
    /// standing in for exact polyhedral overlap.
    fn conserve_integral(&self, source_field: &VolScalarField, mapped: &mut [f64]) {
        let i_src: f64 = self
            .source
            .cell_volumes
            .iter()
            .zip(source_field.internal.iter())
            .map(|(&v, &phi)| v * phi)
            .sum();
        let i_tgt: f64 = self
            .target
            .cell_volumes
            .iter()
            .zip(mapped.iter())
            .map(|(&v, &phi)| v * phi)
            .sum();
        // Only rescale when the mapped integral is non-trivial; a zero target
        // integral (e.g. a uniformly-zero field) is already conservative.
        if i_tgt.abs() > f64::EPSILON {
            let s = i_src / i_tgt;
            for m in mapped.iter_mut() {
                *m *= s;
            }
        }
    }
}

/// Index of the source cell whose centre is nearest `point`.
fn nearest_cell(source: &FvMesh, point: Vector3) -> usize {
    source
        .cell_centres
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (**a - point)
                .mag_sqr()
                .partial_cmp(&(**b - point).mag_sqr())
                .unwrap()
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Inverse-distance-squared weights over the `n` nearest source cells,
/// normalised to sum to 1. A coincident cell (distance 0) captures the whole
/// weight (exact reproduction on conformal meshes).
fn inverse_distance_weights(source: &FvMesh, point: Vector3, n: usize) -> Vec<(usize, f64)> {
    let mut dists: Vec<(usize, f64)> = source
        .cell_centres
        .iter()
        .enumerate()
        .map(|(i, &c)| (i, (c - point).mag_sqr()))
        .collect();
    dists.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    let n = n.min(dists.len());

    // Exact hit: a coincident source cell takes the whole weight.
    if let Some(&(cell, d2)) = dists.first() {
        if d2 <= f64::EPSILON {
            return vec![(cell, 1.0)];
        }
    }

    let raw: Vec<(usize, f64)> = dists[..n].iter().map(|&(i, d2)| (i, 1.0 / d2)).collect();
    let total: f64 = raw.iter().map(|&(_, w)| w).sum();
    raw.into_iter().map(|(i, w)| (i, w / total)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

    /// Build a 1-D column of `n` equal cells spanning `[0, length]` with
    /// cross-sectional area `area`, centred on the x-axis. Cell `i` occupies
    /// `[i·h, (i+1)·h]`; volume `= h·area`.
    fn column(n: usize, length: f64, area: f64) -> Arc<FvMesh> {
        let h = length / n as f64;
        let n_internal = n - 1;
        // internal faces between cell i and i+1, then 2 boundary faces (bottom, top)
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        for i in 0..n_internal {
            owner.push(i);
            neighbour.push(i + 1);
        }
        owner.push(0); // bottom boundary face, owner cell 0
        owner.push(n - 1); // top boundary face, owner cell n-1
        let cell_centres: Vec<Vector3> = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * h, 0.0, 0.0))
            .collect();
        let cell_volumes = vec![h * area; n];
        // face area vectors: internal faces point +x, boundaries outward.
        let mut fav = vec![Vector3::new(area, 0.0, 0.0); n_internal];
        fav.push(Vector3::new(-area, 0.0, 0.0)); // bottom outward -x
        fav.push(Vector3::new(area, 0.0, 0.0)); // top outward +x
        let mut fc: Vec<Vector3> = (1..n)
            .map(|i| Vector3::new(i as f64 * h, 0.0, 0.0))
            .collect();
        fc.push(Vector3::new(0.0, 0.0, 0.0));
        fc.push(Vector3::new(length, 0.0, 0.0));
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_internal)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("bottom", n_internal, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", n_internal + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(cell_volumes)
                .cell_centres(cell_centres)
                .face_area_vectors(fav)
                .face_centres(fc)
                .build()
                .unwrap(),
        )
    }

    /// V&V — conformal identity mapping.
    ///
    /// Methodology: two identical 10-cell columns; map a non-uniform field
    /// `φ_i = sin(x_i)` from source to target with `NearestCell`. On coincident
    /// meshes the nearest source cell is the coincident cell, so the transfer
    /// must reproduce every value bit-for-bit and preserve the integral.
    ///
    /// Result (2026-07-15): max |φ_tgt − φ_src| = 0.0 over all 10 cells; integral
    /// preserved to 0.0 absolute. Pass.
    #[test]
    fn conformal_identity_reproduces_field() {
        let src_mesh = column(10, 1.0, 2.0);
        let tgt_mesh = column(10, 1.0, 2.0);
        let m = MeshToMesh::new(
            src_mesh.clone(),
            tgt_mesh.clone(),
            MappingMethod::NearestCell,
        );

        let mut src = VolScalarField::uniform("phi", src_mesh.clone(), 0.0);
        for (i, c) in src_mesh.cell_centres.iter().enumerate() {
            src.internal[i] = c.x.sin();
        }
        let tgt = m.interpolate_scalar(&src, "phi_mapped");

        let max_err = (0..10)
            .map(|i| (tgt.internal[i] - src.internal[i]).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_err == 0.0,
            "conformal identity must be exact, got {max_err}"
        );

        let i_src = MeshToMesh::integral(&src_mesh, &src);
        let i_tgt = MeshToMesh::integral(&tgt_mesh, &tgt);
        assert!((i_src - i_tgt).abs() == 0.0);
    }

    /// V&V — conservation of the volume integral under a non-conformal transfer.
    ///
    /// Methodology: source is a coarse 4-cell column, target a fine 16-cell
    /// column over the same `[0,1]` domain (same total volume). Map a linear
    /// field `φ = 1 + 2x` with `CellVolumeWeight`. The exact volume integral is
    /// `∫₀¹ (1+2x)·A dx = (1+1)·A = 2A = 4.0` for `A = 2`. A conservative mapper
    /// must reproduce this integral on the target mesh regardless of the local
    /// refinement mismatch.
    ///
    /// Result (2026-07-15, release): source integral = 4.0, target integral =
    /// 4.0, difference 0.0 (bit-exact; assertion bound 1e-12). Pass. (Nearest-cell
    /// alone would drift; the global rescale restores conservation — see module
    /// docs on the degraded CellVolumeWeight.)
    #[test]
    fn cell_volume_weight_conserves_integral() {
        let src_mesh = column(4, 1.0, 2.0);
        let tgt_mesh = column(16, 1.0, 2.0);
        let m = MeshToMesh::new(
            src_mesh.clone(),
            tgt_mesh.clone(),
            MappingMethod::CellVolumeWeight,
        );

        let mut src = VolScalarField::uniform("phi", src_mesh.clone(), 0.0);
        for (i, c) in src_mesh.cell_centres.iter().enumerate() {
            src.internal[i] = 1.0 + 2.0 * c.x;
        }
        let tgt = m.interpolate_scalar(&src, "phi_mapped");

        let i_src = MeshToMesh::integral(&src_mesh, &src);
        let i_tgt = MeshToMesh::integral(&tgt_mesh, &tgt);
        assert!(
            (i_src - i_tgt).abs() < 1e-12,
            "integral not conserved: src={i_src} tgt={i_tgt}"
        );
    }

    /// V&V — inverse-distance blend tracks a linear field's trend.
    ///
    /// Methodology: coarse 5-cell source (centres at 0.1..0.9), fine 20-cell
    /// target, `φ = 3x` linear. Inverse-distance weighting does **not** have exact
    /// linear precision (unlike the RBF `Linear` and nearest-cell-on-conformal
    /// paths) — near data points it flattens, and target cells *beyond the
    /// outermost source centre* extrapolate poorly. So this only asserts the
    /// blend tracks the linear trend within a coarse-spacing-scaled bound.
    ///
    /// Result (2026-07-15): max nodal error 0.267 (< 0.30 tolerance), worst at the
    /// boundary cell x≈0.975 (beyond the last source centre 0.9). Interior cells
    /// are far better. This documents IDW's accuracy, not a conservation property;
    /// use `CellVolumeWeight` for conservation or the RBF path for linear
    /// precision.
    #[test]
    fn inverse_distance_tracks_linear() {
        let src_mesh = column(5, 1.0, 1.0);
        let tgt_mesh = column(20, 1.0, 1.0);
        let m = MeshToMesh::new(
            src_mesh.clone(),
            tgt_mesh.clone(),
            MappingMethod::InverseDistance { n_neighbours: 2 },
        );
        let mut src = VolScalarField::uniform("phi", src_mesh.clone(), 0.0);
        for (i, c) in src_mesh.cell_centres.iter().enumerate() {
            src.internal[i] = 3.0 * c.x;
        }
        let tgt = m.interpolate_scalar(&src, "phi_mapped");
        let max_err = (0..tgt_mesh.n_cells)
            .map(|i| (tgt.internal[i] - 3.0 * tgt_mesh.cell_centres[i].x).abs())
            .fold(0.0_f64, f64::max);
        assert!(max_err < 0.30, "linear-trend tracking error {max_err}");
    }

    /// Accumulate mode adds onto the target (upstream `plusEqOp`).
    #[test]
    fn accumulate_adds_onto_target() {
        let mesh = column(3, 1.0, 1.0);
        let m = MeshToMesh::new(mesh.clone(), mesh.clone(), MappingMethod::NearestCell);
        let src = VolScalarField::uniform("s", mesh.clone(), 2.0);
        let mut tgt = VolScalarField::uniform("t", mesh.clone(), 10.0);
        m.map_scalar(&src, MapCombine::Accumulate, &mut tgt);
        for i in 0..3 {
            assert!((tgt.internal[i] - 12.0).abs() < 1e-12);
        }
    }

    /// Vector mapping interpolates componentwise on a conformal mesh (identity).
    #[test]
    fn vector_conformal_identity() {
        let mesh = column(4, 1.0, 1.0);
        let m = MeshToMesh::new(mesh.clone(), mesh.clone(), MappingMethod::NearestCell);
        let mut src = VolVectorField::zero("disp", mesh.clone());
        for i in 0..4 {
            src.internal[i] = Vector3::new(i as f64, -(i as f64), 0.5);
        }
        let mut tgt = VolVectorField::zero("disp_mapped", mesh.clone());
        m.map_vector(&src, MapCombine::Replace, &mut tgt);
        for i in 0..4 {
            assert!((tgt.internal[i] - src.internal[i]).mag() < 1e-12);
        }
    }
}
