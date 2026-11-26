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
//! - [`MappingMethod::CellVolumeWeight`] — upstream's default (`imCellVolumeWeight`), **exact** since gh:#651
//!   (2026-11-26): the true polyhedral overlap volumes of every source/target cell pair, per-cell normalisation by the
//!   sum of the overlaps, `result *= (1 - sum w); result += w * value`, and an uncovered cell keeps its value.
//!   This is [`super::cell_overlap::CellVolumeWeightAddressing`], a line-by-line translation of OpenFOAM-ESI v2506's
//!   `cellVolumeWeightMethod` / `tetOverlapVolume` / `meshToMesh::mapTgtToSrc`, bit-identical to the real code on the
//!   3D_SmallESFR meshes (README `Round 28`). It needs polyhedral geometry, which an [`FvMesh`] does not carry, so it is
//!   built with [`MeshToMesh::cell_volume_weight`] from two [`OverlapMesh`]es.
//!   ~~Before: nearest-cell addressing plus a global integral rescale ("a documented degraded mode ... exact polyhedral
//!   overlap awaits a basic-lib supermesh operator").~~ **CORRECTED 2026-11-26 (gh:#651):** that form turned a uniform
//!   900 K source into 450 K on a target twice as long (the rescale multiplies EVERY target cell by
//!   `I_src / I_tgt`, which is wrong whenever the two meshes do not cover the same volume, and wrong for any intensive
//!   field), and the supermesh operator was never missing from the workspace (`outram-blender` has one; this port
//!   follows ESI's own order of operations instead, which `outram-blender` does not). It remains, by name, as the
//!   deprecated [`MappingMethod::NearestCellGlobalRescale`], for reproducing earlier results only.
//!
//! ## Units
//!
//! The mapper is a linear operator on raw cell values, so it is unit-agnostic —
//! it carries the field's physical meaning without interpreting it, matching how
//! [`outram_foam_basic_lib`] fields (`VolScalarField` = `f64` per cell) are
//! unit-agnostic. The physical `uom` quantities live in the coupling layer that
//! *reads* the mapped fields ([`super::outer_iteration`]).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};

use super::cell_overlap::{CellVolumeWeightAddressing, OverlapMesh};
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
    /// Upstream's `imCellVolumeWeight` (GeN-Foam's only choice): exact overlap volumes, per-cell normalisation by the sum,
    /// uncovered cells keep their value. **The default.** Needs [`OverlapMesh`] geometry: build with
    /// [`MeshToMesh::cell_volume_weight`]; [`MeshToMesh::new`] refuses it with
    /// [`MeshToMeshError::MissingOverlapGeometry`].
    CellVolumeWeight,
    /// The pre-gh:#651 form: nearest-cell addressing followed by a global rescale that makes `sum V phi` equal on both
    /// meshes. **Wrong whenever the two meshes do not cover the same volume** (a uniform 900 K field mapped onto a mesh twice
    /// as long becomes 450 K), and not what upstream does. Kept ONLY to reproduce earlier numbers and as the control of
    /// the V&V comparison; never the default.
    #[deprecated(note = "not upstream's operator and wrong on partial overlap (gh:#651); use MappingMethod::CellVolumeWeight")]
    NearestCellGlobalRescale,
}

/// Errors from building a [`MeshToMesh`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshToMeshError {
    /// [`MappingMethod::CellVolumeWeight`] was requested from [`MeshToMesh::new`], which only has [`FvMesh`]es.
    MissingOverlapGeometry,
    /// An [`OverlapMesh`] does not have the cell count of the [`FvMesh`] it is meant to describe.
    CellCountMismatch {
        /// Which mesh (`"source"` or `"target"`).
        which: &'static str,
        /// Cells of the [`FvMesh`].
        fv_cells: usize,
        /// Cells of the [`OverlapMesh`].
        overlap_cells: usize,
    },
}

impl core::fmt::Display for MeshToMeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MeshToMeshError::MissingOverlapGeometry => write!(
                f,
                "CellVolumeWeight needs polyhedral geometry (points and faces): build it with MeshToMesh::cell_volume_weight from two OverlapMesh"
            ),
            MeshToMeshError::CellCountMismatch { which, fv_cells, overlap_cells } => {
                write!(f, "the {which} OverlapMesh has {overlap_cells} cells but its FvMesh has {fv_cells}")
            }
        }
    }
}

impl core::error::Error for MeshToMeshError {}

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
    /// target cell (an interpolation, not an extrapolation). Empty for [`MappingMethod::CellVolumeWeight`].
    weights: Vec<Vec<(usize, f64)>>,
    /// The upstream `meshToMesh` object of [`MappingMethod::CellVolumeWeight`]: GeN-Foam moves a field from region `B` onto
    /// region `A` with `meshToMesh(src = A, tgt = B).mapTgtToSrc`, so upstream's SOURCE is this mapping's
    /// [`target`](Self::target) (the receiving mesh) and upstream's TARGET is this mapping's [`source`](Self::source).
    cell_volume_weight: Option<Arc<CellVolumeWeightAddressing>>,
}

impl MeshToMesh {
    /// Build the mapping addressing from `source` onto `target` with `method`.
    ///
    /// The source and target meshes must both have cell centres and (for the
    /// deprecated global rescale) cell volumes populated — every `FvMesh` built
    /// through [`outram_foam_basic_lib::mesh::FvMeshBuilder`] does. Cost is
    /// `O(n_target · n_source)` for the nearest-cell search.
    ///
    /// [`MappingMethod::CellVolumeWeight`] cannot be built here (an [`FvMesh`] has no points or faces): use
    /// [`MeshToMesh::cell_volume_weight`].
    ///
    /// # Errors
    ///
    /// [`MeshToMeshError::MissingOverlapGeometry`] for [`MappingMethod::CellVolumeWeight`].
    ///
    /// # Panics
    ///
    /// Panics if the source mesh has no cells (nothing to interpolate from) or if
    /// `InverseDistance { n_neighbours: 0 }` is requested.
    #[allow(deprecated)]
    pub fn new(source: Arc<FvMesh>, target: Arc<FvMesh>, method: MappingMethod) -> Result<Self, MeshToMeshError> {
        assert!(
            source.n_cells > 0,
            "mesh-to-mesh mapping needs a non-empty source mesh"
        );
        let weights = match method {
            MappingMethod::CellVolumeWeight => return Err(MeshToMeshError::MissingOverlapGeometry),
            MappingMethod::NearestCell | MappingMethod::NearestCellGlobalRescale => target
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
        Ok(Self {
            source,
            target,
            method,
            weights,
            cell_volume_weight: None,
        })
    }

    /// GeN-Foam's mapping: `meshToMesh(.., imCellVolumeWeight, pmAABB, false)` between two region meshes, used as
    /// `mapTgtToSrc` (`meshHandler.C:192-201`, `meshHandlerTemplates.C:85,133`).
    ///
    /// `source` / `target` are the DATA direction of this crate (fields are read on `source`, written on `target`);
    /// `source_geometry` / `target_geometry` are their polyhedral meshes. Upstream's object is built the other way round
    /// (its source is the receiving region): see the module docs of [`super::cell_overlap`].
    ///
    /// # Errors
    ///
    /// [`MeshToMeshError::CellCountMismatch`] if a geometry does not have its `FvMesh`'s cell count.
    pub fn cell_volume_weight(
        source: Arc<FvMesh>,
        target: Arc<FvMesh>,
        source_geometry: &OverlapMesh,
        target_geometry: &OverlapMesh,
    ) -> Result<Self, MeshToMeshError> {
        if source_geometry.n_cells() != source.n_cells {
            return Err(MeshToMeshError::CellCountMismatch {
                which: "source",
                fv_cells: source.n_cells,
                overlap_cells: source_geometry.n_cells(),
            });
        }
        if target_geometry.n_cells() != target.n_cells {
            return Err(MeshToMeshError::CellCountMismatch {
                which: "target",
                fv_cells: target.n_cells,
                overlap_cells: target_geometry.n_cells(),
            });
        }
        // upstream object: src = the receiving mesh, tgt = the providing mesh
        let addressing = CellVolumeWeightAddressing::new_normalised(target_geometry, source_geometry);
        Ok(Self {
            source,
            target,
            method: MappingMethod::CellVolumeWeight,
            weights: Vec::new(),
            cell_volume_weight: Some(Arc::new(addressing)),
        })
    }

    /// The exact upstream addressing and weights of a [`MappingMethod::CellVolumeWeight`] mapping (upstream's
    /// `srcToTgtCell*` are indexed by this mapping's TARGET cells, its `tgtToSrcCell*` by this mapping's SOURCE cells),
    /// `None` for the other methods.
    #[must_use]
    pub fn upstream_addressing(&self) -> Option<&CellVolumeWeightAddressing> {
        self.cell_volume_weight.as_deref()
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
    /// with [`MapCombine::Accumulate`] the mapped values are added on.
    ///
    /// **[`MappingMethod::CellVolumeWeight`] with `Replace` is upstream's `mapTgtToSrc(.., plusEqOp, ..)` exactly**: for a
    /// covered cell `target *= (1 - sum w); target += w * value` per overlap, in order (an old value survives only as
    /// `old * (1 - sum w)`, an ulp-level remainder), and a cell with NO overlap keeps its value. That is what GeN-Foam does
    /// and nothing else. With [`MapCombine::Accumulate`] the `*= (1 - sum w)` step is skipped (an OUTRAM PARK extension,
    /// not in upstream, for several sources summing into one field) and the mapped mean is added.
    /// [`MappingMethod::NearestCellGlobalRescale`] applies the old global rescale on `Replace`.
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

        if let Some(cvw) = &self.cell_volume_weight {
            match combine {
                MapCombine::Replace => cvw.map_tgt_to_src_scalar(
                    source_field.internal.as_slice(),
                    target_field.internal.as_mut_slice(),
                ),
                MapCombine::Accumulate => {
                    for (cell, value) in target_field.internal.as_mut_slice().iter_mut().enumerate() {
                        let addr = &cvw.src_to_tgt_addr[cell];
                        let wght = &cvw.src_to_tgt_wght[cell];
                        for (i, &src) in addr.iter().enumerate() {
                            *value += wght[i] * source_field.internal[src];
                        }
                    }
                }
            }
            return;
        }

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

        #[allow(deprecated)]
        let global_rescale = matches!(self.method, MappingMethod::NearestCellGlobalRescale);
        if global_rescale && matches!(combine, MapCombine::Replace) {
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
    /// The interpolation is applied componentwise. [`MappingMethod::CellVolumeWeight`] is upstream's `mapTgtToSrc` per
    /// component (see [`MeshToMesh::map_scalar`]); the deprecated global rescale is not applied to vectors.
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

        if let Some(cvw) = &self.cell_volume_weight {
            match combine {
                MapCombine::Replace => cvw.map_tgt_to_src_vector(
                    source_field.internal.as_slice(),
                    target_field.internal.as_mut_slice(),
                ),
                MapCombine::Accumulate => {
                    for (cell, value) in target_field.internal.as_mut_slice().iter_mut().enumerate() {
                        let addr = &cvw.src_to_tgt_addr[cell];
                        let wght = &cvw.src_to_tgt_wght[cell];
                        for (i, &src) in addr.iter().enumerate() {
                            let y = source_field.internal[src];
                            *value = Vector3::new(value.x + wght[i] * y.x, value.y + wght[i] * y.y, value.z + wght[i] * y.z);
                        }
                    }
                }
            }
            return;
        }

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
    use petir::mathf::RealMath;
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
        ).unwrap();

        let mut src = VolScalarField::uniform("phi", src_mesh.clone(), 0.0);
        for (i, c) in src_mesh.cell_centres.iter().enumerate() {
            src.internal[i] = c.x.r_sin();
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
    /// Result (2026-07-15, release, the old global-rescale form): source integral = 4.0, target integral =
    /// 4.0, difference 0.0. **RE-MEASURED 2026-11-26 (gh:#651), the exact operator**: the same assertion, bound 1e-12,
    /// now holds because the overlap weights themselves conserve (not because of a rescale); the value is recorded in
    /// the README "Round 28" table. The meshes are 4 and 16 cells of the block `[0,1] x [0,1] x [0,2]`
    /// (cross-section `A = 2`).
    #[test]
    fn cell_volume_weight_conserves_integral() {
        let src_mesh = column(4, 1.0, 2.0);
        let tgt_mesh = column(16, 1.0, 2.0);
        let src_geom = OverlapMesh::box_grid(4, 1, 1, [0.0; 3], [1.0, 1.0, 2.0]);
        let tgt_geom = OverlapMesh::box_grid(16, 1, 1, [0.0; 3], [1.0, 1.0, 2.0]);
        let m = MeshToMesh::cell_volume_weight(src_mesh.clone(), tgt_mesh.clone(), &src_geom, &tgt_geom).unwrap();
        assert!(m.upstream_addressing().is_some());

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
        ).unwrap();
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
        let m = MeshToMesh::new(mesh.clone(), mesh.clone(), MappingMethod::NearestCell).unwrap();
        let src = VolScalarField::uniform("s", mesh.clone(), 2.0);
        let mut tgt = VolScalarField::uniform("t", mesh.clone(), 10.0);
        m.map_scalar(&src, MapCombine::Accumulate, &mut tgt);
        for i in 0..3 {
            assert!((tgt.internal[i] - 12.0).abs() < 1e-12);
        }
    }

    /// `new` refuses `CellVolumeWeight` by name (no points and faces on an `FvMesh`) and `cell_volume_weight` checks the
    /// cell counts.
    #[test]
    fn cell_volume_weight_needs_overlap_geometry() {
        let a = column(4, 1.0, 1.0);
        let b = column(8, 1.0, 1.0);
        assert_eq!(
            MeshToMesh::new(a.clone(), b.clone(), MappingMethod::CellVolumeWeight).unwrap_err(),
            MeshToMeshError::MissingOverlapGeometry
        );
        let g4 = OverlapMesh::box_grid(4, 1, 1, [0.0; 3], [1.0; 3]);
        let g5 = OverlapMesh::box_grid(5, 1, 1, [0.0; 3], [1.0; 3]);
        let e = MeshToMesh::cell_volume_weight(a, b, &g4, &g5).unwrap_err();
        assert_eq!(e, MeshToMeshError::CellCountMismatch { which: "target", fv_cells: 8, overlap_cells: 5 });
        assert!(e.to_string().contains("5 cells"));
    }

    /// V&V (gh:#651) — the issue's partial-overlap case through the public API, with the control.
    ///
    /// Methodology: source 10 cells on `[0, 1]` m at a uniform 900 K, target 10 cells on `[0, 2]` m with a pre-existing
    /// value `123 + i`. Upstream's `mapTgtToSrc`: target cells 0-4 are covered (900 K), cells 5-9 keep their value.
    /// Control: the deprecated `NearestCellGlobalRescale`, the pre-gh:#651 form, gives 450 K on every cell (the failure the
    /// issue reports), so the control fails exactly where the new form passes.
    ///
    /// Result (2026-11-26, release): new form 900 K on cells 0-4 (|err| < 1e-12), cells 5-9 exactly 128..132 + 0 (kept);
    /// old form 450 K everywhere (|err| < 1e-9) as the issue reported.
    #[test]
    #[allow(deprecated)]
    fn partial_overlap_upstream_form_and_deprecated_control() {
        let src = column(10, 1.0, 1.0);
        let tgt = column(10, 2.0, 1.0);
        let src_geom = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [1.0, 1.0, 1.0]);
        let tgt_geom = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [2.0, 1.0, 1.0]);
        let uniform = VolScalarField::uniform("T", src.clone(), 900.0);

        let m = MeshToMesh::cell_volume_weight(src.clone(), tgt.clone(), &src_geom, &tgt_geom).unwrap();
        let mut out = VolScalarField::uniform("T", tgt.clone(), 0.0);
        for i in 0..10 {
            out.internal[i] = 123.0 + i as f64;
        }
        m.map_scalar(&uniform, MapCombine::Replace, &mut out);
        for i in 0..5 {
            assert!((out.internal[i] - 900.0).abs() < 1.0e-12, "covered cell {i}: {}", out.internal[i]);
        }
        for i in 5..10 {
            assert_eq!(out.internal[i], 123.0 + i as f64, "uncovered cell {i}");
        }

        let old = MeshToMesh::new(src.clone(), tgt.clone(), MappingMethod::NearestCellGlobalRescale).unwrap();
        let mut out_old = VolScalarField::uniform("T", tgt.clone(), 0.0);
        old.map_scalar(&uniform, MapCombine::Replace, &mut out_old);
        for i in 0..10 {
            assert!((out_old.internal[i] - 450.0).abs() < 1.0e-9, "old form cell {i}: {}", out_old.internal[i]);
        }

        // Accumulate (OUTRAM PARK extension): the mapped mean is added, the uncovered cells are untouched
        let mut acc = VolScalarField::uniform("T", tgt.clone(), 10.0);
        m.map_scalar(&uniform, MapCombine::Accumulate, &mut acc);
        assert!((acc.internal[0] - 910.0).abs() < 1.0e-12);
        assert_eq!(acc.internal[9], 10.0);
    }

    /// V&V (gh:#651) — the issue's same-extent linear case: `600 + 600 x`, 10 -> 3 cells. Upstream's operator averages the
    /// piecewise-constant source CELL VALUES: 702 / 900 / 1098 (the issue's 700 / 900 / 1100 were the continuous means, not
    /// what `mapTgtToSrc` returns); the pre-gh:#651 form gave 697.8 / 879.8 / 1122.5 (middle cell 2.2 % off).
    ///
    /// Result (2026-11-26, release): 702 / 900 / 1098 to 1e-10; the deprecated control reproduces the issue's numbers to
    /// 0.05 (697.8 / 879.8 / 1122.5). The vector path is exercised with the same field per component.
    #[test]
    #[allow(deprecated)]
    fn same_extent_linear_field_exact_and_control_reproduces_the_issue() {
        let src = column(10, 1.0, 1.0);
        let tgt = column(3, 1.0, 1.0);
        let src_geom = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [1.0; 3]);
        let tgt_geom = OverlapMesh::box_grid(3, 1, 1, [0.0; 3], [1.0; 3]);
        let mut f = VolScalarField::uniform("T", src.clone(), 0.0);
        for (i, c) in src.cell_centres.iter().enumerate() {
            f.internal[i] = 600.0 + 600.0 * c.x;
        }
        let m = MeshToMesh::cell_volume_weight(src.clone(), tgt.clone(), &src_geom, &tgt_geom).unwrap();
        let out = m.interpolate_scalar(&f, "T");
        for (got, want) in out.internal.as_slice().iter().zip([702.0, 900.0, 1098.0]) {
            assert!((got - want).abs() < 1.0e-10, "{got} != {want}");
        }
        let old = MeshToMesh::new(src.clone(), tgt.clone(), MappingMethod::NearestCellGlobalRescale).unwrap();
        let out_old = old.interpolate_scalar(&f, "T");
        for (got, want) in out_old.internal.as_slice().iter().zip([697.8, 879.8, 1122.5]) {
            assert!((got - want).abs() < 0.05, "old form {got} != {want}");
        }
        // vector: x = the same field, y = twice it, z = constant
        let mut v = VolVectorField::zero("u", src.clone());
        for i in 0..10 {
            v.internal[i] = Vector3::new(f.internal[i], 2.0 * f.internal[i], 5.0);
        }
        let mut vo = VolVectorField::zero("u", tgt.clone());
        m.map_vector(&v, MapCombine::Replace, &mut vo);
        assert!((vo.internal[1].x - 900.0).abs() < 1.0e-10);
        assert!((vo.internal[1].y - 1800.0).abs() < 1.0e-9);
        assert!((vo.internal[1].z - 5.0).abs() < 1.0e-12);
        let mut va = VolVectorField::zero("u", tgt.clone());
        m.map_vector(&v, MapCombine::Accumulate, &mut va);
        assert!((va.internal[2].x - 1098.0).abs() < 1.0e-10);
    }

    /// Vector mapping interpolates componentwise on a conformal mesh (identity).
    #[test]
    fn vector_conformal_identity() {
        let mesh = column(4, 1.0, 1.0);
        let m = MeshToMesh::new(mesh.clone(), mesh.clone(), MappingMethod::NearestCell).unwrap();
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
