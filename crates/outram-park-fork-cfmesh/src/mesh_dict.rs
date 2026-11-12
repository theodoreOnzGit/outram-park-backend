// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/checkMeshDict/checkMeshDict.{C,H}
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeCreator/
//     meshOctreeCreatorCreateOctreeBoxes.C (setRootCubeSizeAndRefParameters,
//     refineBoxesIntersectingSurfaces, refineBoxesIntersectingEdgeMeshes:
//     the dictionary reads)
//   meshLibrary/utilities/octrees/meshOctree/refinementControls/
//     objectRefinement/{objectRefinement,newObjectRefinement,boxRefinement,
//     sphereRefinement,coneRefinement,hollowConeRefinement,lineRefinement}.C
//   meshLibrary/utilities/anisotropicMeshing/coordinateModification/
//     {coordinateModifier,newCoordinateModification,boxScaling,planeScaling}.C
//   meshLibrary/utilities/boundaryLayers/refineBoundaryLayers/
//     refineBoundaryLayers.C (readSettings, set*ForPatch)
//   meshLibrary/utilities/boundaryLayers/boundaryLayers/boundaryLayers.C
//     (the height-factor reads)
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/
//     boundaryLayerOptimisation/boundaryLayerOptimisation.C (readSettings),
//     optimizeMeshFV.C (optimiseLayer, untangleLayers)
//   meshLibrary/utilities/workflowControls/workflowControls.C
//   meshLibrary/utilities/surfaceTools/renameBoundaryPatches/
//     renameBoundaryPatches.C (the dictionary reads)
//   meshLibrary/{cartesianMesh,tetMesh,voronoiMesh,cartesian2DMesh}/
//     *Generator.C (the keywords each generator reads)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! The **`meshDict` reader** (gh:#1476): an OpenFOAM `system/meshDict`, as
//! parsed by `outram_foam_basic_lib::io::dict` (the workspace's one
//! dictionary parser), read into the typed settings the four generators
//! take ([`crate::mesh_octree_creator::MeshDict`],
//! [`crate::pipeline::cartesian_mesh::CartesianMeshGeneratorOptions`],
//! [`crate::pipeline::tet_mesh::TetMeshOptions`],
//! [`crate::pipeline::cartesian_2d_mesh::Cartesian2DMeshOptions`]), and
//! [`check_mesh_dict`], `checkMeshDict`'s validation.
//!
//! # Two steps
//!
//! 1. [`read_mesh_dict`] reads the dictionary alone: every value, with
//!    patch names still as written, and a [`KeywordUse`] for **every**
//!    entry it met. Nothing is skipped silently: an entry is
//!    [`KeywordStatus::Honoured`], [`KeywordStatus::Reported`] (read, but
//!    this port does not act on it, with the reason),
//!    [`KeywordStatus::NotApplicable`] (upstream's generator does not read
//!    it either) or [`KeywordStatus::Unknown`] (no upstream code reads it).
//! 2. [`MeshDictReading::resolve`] matches the patch names against the
//!    surface's patches and loads the refinement-source surfaces, giving
//!    the generator's options ([`GeneratorOptions`]).
//!
//! # Patch names are regular expressions
//!
//! Upstream matches every patch name it reads (`localRefinement`,
//! `patchCellSize`, `patchBoundaryLayers`, `excludedPatches`,
//! `keep`/`removeCellsIntersectingPatches`, `renameBoundary`) through
//! `findPatches(name)`, `findMatchingStrings(regExp(name), patchNames())`:
//! a **full** match of the name as a POSIX extended regular expression,
//! whether or not it was quoted. The port uses the crate's existing matcher,
//! [`crate::rename_boundary_patches::regex_full_match`]. Two upstream quirks
//! are kept: `tetMesh`/`pMesh`'s `generateBoundaryLayers` matches
//! `patchBoundaryLayers` names **exactly** (`addLayerForPatch(word)`
//! compares names), and the facet-subset half of a `localRefinement` entry
//! is an exact name lookup.
//!
//! # Upstream defects kept
//!
//! - `checkMeshDict::checkEntries` does not call `checkLocalRefinementLevel`
//!   ([`check_local_refinement_level`] is ported and not called either).
//! - `checkSubsetCellSize`'s list branch reads `patchCellSize`.
//! - `checkMeshDict` reads `optimisationParameters/reCalculateNormals`, but
//!   `boundaryLayerOptimisation::readSettings` reads `recalculateNormals`:
//!   the `ship5415Octree` tutorial's `reCalculateNormals 1` has no effect
//!   upstream. Both spellings are recorded; only `recalculateNormals` sets
//!   anything.
//! - A `planeScaling` read from the dictionary keeps its `normal` as written
//!   (only the parametric constructor normalises it) and does not reset a
//!   `scalingFactor` below `SMALL`.
//!
//! # What the port does not act on (reported, not skipped)
//!
//! See [`KeywordStatus::Reported`] in a reading's table; at the time of
//! writing: `meshQualitySettings` (the `findBadFaces` thresholds are not
//! wired), `edgeMeshRefinement` (no `edgeFile` reader), `boundaryLayers/
//! optimiseLayer` and `optimisationParameters` for `cartesianMesh` only
//! (`optimizeBoundaryLayer` runs, without its buffer layer), `untangleLayers false`, `subsetCellSize` and
//! subset `localRefinement` (the generators take no facet subsets), and for
//! `tetMesh`/`pMesh` the octree-addressing keys (`nonManifoldMeshing`,
//! `checkForGluedMesh`, `keep`/`removeCellsIntersectingPatches`: their
//! templates build the addressing without the dictionary).

use crate::anisotropic::{BoxScaling, CoordinateModifier, Modification, PlaneScaling};
use crate::layers::boundary_layers::BoundaryLayersControls;
use crate::layers::refine_boundary_layers::{PatchLayerSettings, RefineBoundaryLayersSettings};
use crate::math::Vec3;
use crate::mesh_octree::TriSurf;
use crate::mesh_octree_creator::{LocalRefinement, MeshDict, PatchRefinement, SubsetLocalRefinement, SubsetRefinement, SurfaceMeshRefinement};
use crate::object_refinement::{ObjectRefinement, ObjectShape};
use crate::pipeline::cartesian_2d_mesh::Cartesian2DMeshOptions;
use crate::pipeline::cartesian_mesh::CartesianMeshGeneratorOptions;
use crate::pipeline::tet_mesh::{BoundaryLayersDict, TetMeshOptions};
use crate::rename_boundary_patches::{regex_full_match, NewPatch, RenameBoundary};
use crate::workflow_controls::{WorkflowSettings, WorkflowStep};
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamValue};
use std::sync::Arc;

/// The executable whose generator reads the dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generator {
    /// `cartesianMesh` (`cartesianMeshGenerator`).
    CartesianMesh,
    /// `tetMesh` (`tetMeshGenerator`).
    TetMesh,
    /// `pMesh` (`voronoiMeshGenerator`).
    PMesh,
    /// `cartesian2DMesh` (`cartesian2DMeshGenerator`).
    Cartesian2DMesh,
}

impl Generator {
    /// The executable's name.
    pub fn name(self) -> &'static str {
        match self {
            Generator::CartesianMesh => "cartesianMesh",
            Generator::TetMesh => "tetMesh",
            Generator::PMesh => "pMesh",
            Generator::Cartesian2DMesh => "cartesian2DMesh",
        }
    }

    fn is_tet_or_poly(self) -> bool {
        matches!(self, Generator::TetMesh | Generator::PMesh)
    }
}

/// What the port does with one dictionary entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordStatus {
    /// Read and acted on as upstream.
    Honoured,
    /// Read, but this port does not act on it; the reason.
    Reported(String),
    /// Upstream's generator does not read it either; the reason.
    NotApplicable(String),
    /// No upstream code reads an entry of this name here.
    Unknown,
}

/// One dictionary entry and what happens to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordUse {
    /// The entry's path, `/`-separated (`boundaryLayers/nLayers`).
    pub path: String,
    /// What the port does with it.
    pub status: KeywordStatus,
}

/// A `localRefinement`, `patchCellSize` or `subsetCellSize` entry with its
/// name as written.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedRefinement {
    /// The name (a pattern for patches).
    pub name: String,
    /// `additionalRefinementLevels`, read first, as upstream.
    pub additional_levels: Option<u8>,
    /// `cellSize`.
    pub cell_size: Option<f64>,
    /// `refinementThickness` (0 if absent).
    pub refinement_thickness: f64,
}

/// A `surfaceMeshRefinement` or `edgeMeshRefinement` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceEntry {
    /// The entry's name.
    pub name: String,
    /// `surfaceFile` / `edgeFile`, as written.
    pub file: String,
    /// `cellSize` (takes precedence, as upstream).
    pub cell_size: Option<f64>,
    /// `additionalRefinementLevels`.
    pub additional_levels: Option<u8>,
    /// `refinementThickness` (0 if absent).
    pub refinement_thickness: f64,
}

/// A `patchBoundaryLayers` entry, name as written.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchLayerEntry {
    /// The patch name (a pattern).
    pub name: String,
    /// `nLayers`.
    pub n_layers: Option<usize>,
    /// `thicknessRatio`.
    pub thickness_ratio: Option<f64>,
    /// `maxFirstLayerThickness`.
    pub max_first_layer_thickness: Option<f64>,
    /// `allowDiscontinuity` (default `false`).
    pub allow_discontinuity: bool,
}

/// The `boundaryLayers` dictionary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoundaryLayersEntry {
    /// `nLayers`.
    pub n_layers: Option<usize>,
    /// `thicknessRatio`.
    pub thickness_ratio: Option<f64>,
    /// `maxFirstLayerThickness`.
    pub max_first_layer_thickness: Option<f64>,
    /// `excludedPatches` (patterns).
    pub excluded_patches: Vec<String>,
    /// `patchBoundaryLayers`, in dictionary order (`toc()`).
    pub patch_layers: Vec<PatchLayerEntry>,
    /// `true` if `patchBoundaryLayers` is present.
    pub has_patch_boundary_layers: bool,
    /// `patchBoundaryLayers.toc()`: every name, dictionary or not (what
    /// `tetMesh`/`pMesh`'s `generateBoundaryLayers` walks).
    pub patch_layer_toc: Vec<String>,
    /// `boundaryLayerHeightFactor`, `boundaryLayerMinHeight`.
    pub heights: BoundaryLayersControls,
    /// `optimiseLayer`.
    pub optimise_layer: Option<bool>,
    /// `boundaryLayerOptimisation::readSettings`: `optimisationParameters`
    /// over the defaults of `boundaryLayerOptimisation(mesh, mse)` (the
    /// constructor `optimizeBoundaryLayer` uses).
    pub optimisation: crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls,
    /// `untangleLayers` (upstream default `true`).
    pub untangle_layers: Option<bool>,
}

/// Everything [`read_mesh_dict`] took from the dictionary (step 1).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshDictReading {
    /// Which generator's reading this is.
    pub generator: Generator,
    /// `surfaceFile`.
    pub surface_file: String,
    /// `maxCellSize`.
    pub max_cell_size: f64,
    /// `minCellSize`.
    pub min_cell_size: Option<f64>,
    /// `boundaryCellSize`.
    pub boundary_cell_size: Option<f64>,
    /// `boundaryCellSizeRefinementThickness` (absolute value, as upstream).
    pub boundary_cell_size_refinement_thickness: f64,
    /// `patchCellSize` (dictionary form: patterns; list form: names).
    pub patch_cell_size: Vec<NamedRefinement>,
    /// `subsetCellSize`.
    pub subset_cell_size: Vec<NamedRefinement>,
    /// `localRefinement`, dictionary order.
    pub local_refinement: Vec<NamedRefinement>,
    /// `objectRefinements`.
    pub object_refinements: Vec<ObjectRefinement>,
    /// `surfaceMeshRefinement`.
    pub surface_mesh_refinement: Vec<SourceEntry>,
    /// `edgeMeshRefinement` (read, not applied: no `edgeFile` reader).
    pub edge_mesh_refinement: Vec<SourceEntry>,
    /// `anisotropicSources`.
    pub anisotropic_sources: Option<CoordinateModifier>,
    /// `keepCellsIntersectingBoundary`.
    pub keep_cells_intersecting_boundary: bool,
    /// `checkForGluedMesh`.
    pub check_for_glued_mesh: bool,
    /// `nonManifoldMeshing`.
    pub non_manifold_meshing: bool,
    /// `keepCellsIntersectingPatches` (patterns).
    pub keep_cells_intersecting_patches: Vec<String>,
    /// `removeCellsIntersectingPatches` (patterns).
    pub remove_cells_intersecting_patches: Vec<String>,
    /// `enforceGeometryConstraints`.
    pub enforce_geometry_constraints: bool,
    /// `decomposePolyhedraIntoTetsAndPyrs`.
    pub decompose_polyhedra_into_tets_and_pyrs: bool,
    /// `boundaryLayers` (`None`: absent or not a dictionary).
    pub boundary_layers: Option<BoundaryLayersEntry>,
    /// `renameBoundary`.
    pub rename_boundary: Option<RenameBoundary>,
    /// `workflowControls`.
    pub workflow: WorkflowSettings,
    /// Every entry met, with what the port does with it.
    pub keywords: Vec<KeywordUse>,
    /// Upstream's warnings (`WarningInFunction`) for this dictionary.
    pub warnings: Vec<String>,
}

impl MeshDictReading {
    /// The entries the port does not act on, or does not know: the
    /// [`KeywordStatus::Reported`] and [`KeywordStatus::Unknown`] rows.
    pub fn unhandled(&self) -> Vec<&KeywordUse> {
        self.keywords.iter().filter(|k| matches!(k.status, KeywordStatus::Reported(_) | KeywordStatus::Unknown)).collect()
    }

    /// The [`KeywordStatus::Unknown`] rows only.
    pub fn unknown(&self) -> Vec<&KeywordUse> {
        self.keywords.iter().filter(|k| k.status == KeywordStatus::Unknown).collect()
    }

    /// Step 2: match the patch names against `surface`'s patches, load the
    /// `surfaceMeshRefinement` surfaces with `load` (called with each
    /// `surfaceFile` as written), and build the generator's options.
    /// Returns the options and notes (patterns that matched nothing, as
    /// upstream silently does; facet subsets dropped).
    ///
    /// # Errors
    /// A source surface that `load` cannot read.
    pub fn resolve<L>(&self, surface: &TriSurf, mut load: L) -> Result<(GeneratorOptions, Vec<String>), String>
    where
        L: FnMut(&str) -> Result<TriSurf, String>,
    {
        let names = surface.patch_names();
        let mut notes = Vec::new();
        let find = |pattern: &str| -> Vec<usize> { names.iter().enumerate().filter(|(_, n)| regex_full_match(pattern, n)).map(|(i, _)| i).collect() };
        let mut d = MeshDict::with_max_cell_size(self.max_cell_size);
        d.min_cell_size = self.min_cell_size;
        d.boundary_cell_size = self.boundary_cell_size;
        d.boundary_cell_size_refinement_thickness = self.boundary_cell_size_refinement_thickness;
        for e in &self.patch_cell_size {
            let m = find(&e.name);
            if m.is_empty() {
                notes.push(format!("patchCellSize {}: matches no surface patch", e.name));
            }
            for r in m {
                d.patch_cell_size.push(PatchRefinement { region: r, cell_size: e.cell_size.unwrap_or(0.0) });
            }
        }
        for e in &self.subset_cell_size {
            d.subset_cell_size.push(SubsetRefinement { subset: e.name.clone(), cell_size: e.cell_size.unwrap_or(0.0) });
        }
        for e in &self.local_refinement {
            let m = find(&e.name);
            let is_subset = surface.facet_subsets.index(&e.name).is_some();
            if m.is_empty() && !is_subset {
                notes.push(format!("localRefinement {}: matches no surface patch or facet subset", e.name));
            }
            for r in m {
                d.local_refinement.push(LocalRefinement { region: r, additional_levels: e.additional_levels, cell_size: e.cell_size, refinement_thickness: e.refinement_thickness });
            }
            if is_subset {
                d.local_refinement_subsets.push(SubsetLocalRefinement { subset: e.name.clone(), additional_levels: e.additional_levels, cell_size: e.cell_size, refinement_thickness: e.refinement_thickness });
            }
        }
        d.object_refinements = self.object_refinements.clone();
        for s in &self.surface_mesh_refinement {
            let surf = load(&s.file).map_err(|e| format!("surfaceMeshRefinement {}: {e}", s.name))?;
            d.surface_mesh_refinement.push(SurfaceMeshRefinement { name: s.name.clone(), surface: Arc::new(surf), cell_size: s.cell_size, additional_levels: s.additional_levels, refinement_thickness: s.refinement_thickness });
        }
        for s in &self.edge_mesh_refinement {
            notes.push(format!("edgeMeshRefinement {}: edgeFile {} not read (no edgeMesh reader in this port); source dropped", s.name, s.file));
        }
        d.anisotropic_sources = self.anisotropic_sources.clone();
        d.keep_cells_intersecting_boundary = self.keep_cells_intersecting_boundary;
        d.check_for_glued_mesh = self.check_for_glued_mesh;
        d.non_manifold_meshing = self.non_manifold_meshing;
        // keep/removeCellsIntersectingPatches: findPatches per name; the
        // addressing also accepts facet-subset names as given.
        let expand = |list: &[String]| -> Vec<String> {
            let mut out = Vec::new();
            for p in list {
                let m = find(p);
                if m.is_empty() {
                    out.push(p.clone());
                }
                for r in m {
                    out.push(names[r].clone());
                }
            }
            out
        };
        d.keep_cells_intersecting_patches = expand(&self.keep_cells_intersecting_patches);
        d.remove_cells_intersecting_patches = expand(&self.remove_cells_intersecting_patches);
        if !d.subset_cell_size.is_empty() || !d.local_refinement_subsets.is_empty() {
            notes.push("subsetCellSize / subset localRefinement: the generators build their surface from points, triangles and patches only, so facet subsets do not reach the octree".into());
        }
        let refine = self.boundary_layers.as_ref().map(|b| refine_settings(b, &names));
        let optimise_layer = self.boundary_layers.as_ref().filter(|b| b.optimise_layer == Some(true)).map(|b| b.optimisation.clone());
        let heights = self.boundary_layers.as_ref().map(|b| b.heights).unwrap_or_default();
        let opts = match self.generator {
            Generator::CartesianMesh => GeneratorOptions::CartesianMesh(CartesianMeshGeneratorOptions {
                dict: d,
                workflow: self.workflow,
                rename_boundary: self.rename_boundary.clone(),
                enforce_geometry_constraints: self.enforce_geometry_constraints,
                decompose_polyhedra_into_tets_and_pyrs: self.decompose_polyhedra_into_tets_and_pyrs,
                boundary_layers: refine,
                layer_heights: heights,
                optimise_layer,
                ..CartesianMeshGeneratorOptions::default()
            }),
            Generator::Cartesian2DMesh => GeneratorOptions::Cartesian2DMesh(Cartesian2DMeshOptions {
                dict: d,
                workflow: self.workflow,
                rename_boundary: self.rename_boundary.clone(),
                decompose_polyhedra_into_tets_and_pyrs: self.decompose_polyhedra_into_tets_and_pyrs,
                layer_heights: heights,
                boundary_layers: refine,
                ..Cartesian2DMeshOptions::default()
            }),
            Generator::TetMesh | Generator::PMesh => {
                let layers = match (&self.boundary_layers, refine) {
                    (Some(b), Some(settings)) => Some(BoundaryLayersDict {
                        n_layers: b.n_layers,
                        settings,
                        heights,
                        patch_layer_names: Some(b.patch_layer_toc.clone()),
                    }),
                    _ => None,
                };
                let o = TetMeshOptions {
                    dict: d,
                    workflow: self.workflow,
                    rename_boundary: self.rename_boundary.clone(),
                    enforce_geometry_constraints: self.enforce_geometry_constraints,
                    boundary_layers: layers,
                    optimise_layer,
                    ..TetMeshOptions::default()
                };
                if self.generator == Generator::TetMesh {
                    GeneratorOptions::TetMesh(o)
                } else {
                    GeneratorOptions::PMesh(o)
                }
            }
        };
        Ok((opts, notes))
    }
}

/// The options [`MeshDictReading::resolve`] builds, by generator.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorOptions {
    /// For [`crate::pipeline::cartesian_mesh::cartesian_mesh`].
    CartesianMesh(CartesianMeshGeneratorOptions),
    /// For [`crate::pipeline::tet_mesh::tet_mesh`].
    TetMesh(TetMeshOptions),
    /// For [`crate::pipeline::p_mesh::p_mesh`].
    PMesh(TetMeshOptions),
    /// For [`crate::pipeline::cartesian_2d_mesh::cartesian_2d_mesh`].
    Cartesian2DMesh(Cartesian2DMeshOptions),
}

/// `refineBoundaryLayers::readSettings` against the mesh's patch names
/// (`set*ForPatch` match by `findPatches`; a later entry overrides an
/// earlier one field by field, as upstream's per-name maps do).
fn refine_settings(b: &BoundaryLayersEntry, names: &[String]) -> RefineBoundaryLayersSettings {
    let mut s = RefineBoundaryLayersSettings::default();
    if let Some(n) = b.n_layers {
        s.n_layers = n;
    }
    if let Some(r) = b.thickness_ratio {
        s.thickness_ratio = r;
    }
    if let Some(t) = b.max_first_layer_thickness {
        s.max_first_layer_thickness = t;
    }
    let blank = PatchLayerSettings { n_layers: None, thickness_ratio: None, max_first_layer_thickness: None, allow_discontinuity: false };
    for pattern in &b.excluded_patches {
        for n in names.iter().filter(|n| regex_full_match(pattern, n)) {
            s.patches.entry(n.clone()).or_insert_with(|| blank.clone()).n_layers = Some(1);
        }
    }
    for p in &b.patch_layers {
        for n in names.iter().filter(|n| regex_full_match(&p.name, n)) {
            let e = s.patches.entry(n.clone()).or_insert_with(|| blank.clone());
            if let Some(v) = p.n_layers {
                e.n_layers = Some(v);
            }
            // setThicknessRatioForPatch returns below 1;
            // setMaxThicknessOfFirstLayerForPatch returns at <= 0
            if let Some(v) = p.thickness_ratio.filter(|&v| v >= 1.0) {
                e.thickness_ratio = Some(v);
            }
            if let Some(v) = p.max_first_layer_thickness.filter(|&v| v > 0.0) {
                e.max_first_layer_thickness = Some(v);
            }
            if p.allow_discontinuity {
                e.allow_discontinuity = true;
            }
        }
    }
    s
}

// ── Typed reads (OpenFOAM's `readIfPresent` / `lookup`) ───────────────────

fn entry_scalar(e: &FoamEntry) -> Option<f64> {
    match e {
        FoamEntry::Scalar(x) => Some(*x),
        _ => None,
    }
}

/// `readIfPresent<scalar>`: absent `None`; present but not a number is
/// OpenFOAM's fatal IO error.
fn scalar(d: &FoamDict, k: &str, at: &str) -> Result<Option<f64>, String> {
    match d.get(k) {
        None => Ok(None),
        Some(e) => entry_scalar(e).map(Some).ok_or_else(|| format!("{at}{k}: expected a scalar")),
    }
}

/// `readIfPresent<label>`.
fn label(d: &FoamDict, k: &str, at: &str) -> Result<Option<i64>, String> {
    match scalar(d, k, at)? {
        None => Ok(None),
        Some(x) if x.fract() == 0.0 && x.abs() < 9.0e15 => Ok(Some(x as i64)),
        Some(x) => Err(format!("{at}{k}: expected a label, found {x}")),
    }
}

/// `Switch` / `readBool`: `true`/`on`/`yes`/`y`/`t`/`any` and
/// `false`/`off`/`no`/`n`/`f`/`none`, or a label (non-zero is true).
fn boolean(d: &FoamDict, k: &str, at: &str) -> Result<Option<bool>, String> {
    match d.get(k) {
        None => Ok(None),
        Some(FoamEntry::Scalar(x)) => Ok(Some(*x != 0.0)),
        Some(FoamEntry::Word(w)) => match w.as_str() {
            "true" | "on" | "yes" | "y" | "t" | "any" => Ok(Some(true)),
            "false" | "off" | "no" | "n" | "f" | "none" => Ok(Some(false)),
            _ => Err(format!("{at}{k}: expected a bool, found {w}")),
        },
        Some(_) => Err(format!("{at}{k}: expected a bool")),
    }
}

/// A `word` or `fileName`: a bare word or a quoted string.
fn word(d: &FoamDict, k: &str, at: &str) -> Result<Option<String>, String> {
    match d.get(k) {
        None => Ok(None),
        Some(FoamEntry::Word(w)) | Some(FoamEntry::Str(w)) => Ok(Some(w.clone())),
        Some(_) => Err(format!("{at}{k}: expected a word")),
    }
}

/// A `vector` / `point`: `(x y z)`.
fn vector(d: &FoamDict, k: &str, at: &str) -> Result<Option<Vec3>, String> {
    match d.get(k) {
        None => Ok(None),
        Some(FoamEntry::List(v)) => match v.as_slice() {
            [FoamValue::Scalar(x), FoamValue::Scalar(y), FoamValue::Scalar(z)] => Ok(Some(Vec3::new(*x, *y, *z))),
            _ => Err(format!("{at}{k}: expected (x y z)")),
        },
        Some(_) => Err(format!("{at}{k}: expected (x y z)")),
    }
}

/// A `wordList`: `( a b c )`, words or strings.
fn word_list(e: &FoamEntry, at: &str) -> Result<Vec<String>, String> {
    let items: Vec<FoamValue> = match e {
        FoamEntry::List(v) => v.clone(),
        FoamEntry::Tokens(v) => match v.as_slice() {
            // a counted list `N ( … )`
            [FoamValue::Scalar(_), FoamValue::List(l)] => l.clone(),
            _ => return Err(format!("{at}: expected a word list")),
        },
        FoamEntry::Word(w) | FoamEntry::Str(w) => vec![FoamValue::Word(w.clone())],
        _ => return Err(format!("{at}: expected a word list")),
    };
    items
        .into_iter()
        .map(|v| match v {
            FoamValue::Word(w) | FoamValue::Str(w) => Ok(w),
            _ => Err(format!("{at}: expected words")),
        })
        .collect()
}

/// A `PtrList<entry>` in list form: `( name { … } name { … } )` (the form
/// `objectRefinements`, `newPatchNames` and `patchCellSize` may take
/// instead of a dictionary).
fn entry_list(e: &FoamEntry, at: &str) -> Result<Vec<(String, FoamDict)>, String> {
    let items: Vec<FoamValue> = match e {
        FoamEntry::List(v) => v.clone(),
        FoamEntry::Tokens(v) => match v.as_slice() {
            [FoamValue::Scalar(_), FoamValue::List(l)] => l.clone(),
            _ => return Err(format!("{at}: expected a list of `name {{ … }}` entries")),
        },
        _ => return Err(format!("{at}: expected a list of `name {{ … }}` entries")),
    };
    let mut out = Vec::new();
    let mut it = items.into_iter();
    while let Some(n) = it.next() {
        let name = match n {
            FoamValue::Word(w) | FoamValue::Str(w) => w,
            _ => return Err(format!("{at}: expected an entry name")),
        };
        match it.next() {
            Some(FoamValue::Dict(d)) => out.push((name, d)),
            _ => return Err(format!("{at}: entry {name} is not a dictionary")),
        }
    }
    Ok(out)
}

/// The `(name, dictionary)` entries of a dictionary-or-list keyword.
fn named_dicts(e: &FoamEntry, at: &str) -> Result<Vec<(String, Option<FoamDict>)>, String> {
    match e {
        FoamEntry::SubDict(d) => Ok(d
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    match v {
                        FoamEntry::SubDict(s) => Some(s.clone()),
                        _ => None,
                    },
                )
            })
            .collect()),
        _ => Ok(entry_list(e, at)?.into_iter().map(|(n, d)| (n, Some(d))).collect()),
    }
}

fn levels(x: Option<i64>, at: &str) -> Result<Option<u8>, String> {
    match x {
        None => Ok(None),
        Some(v) if (0..=255).contains(&v) => Ok(Some(v as u8)),
        Some(v) => Err(format!("{at}additionalRefinementLevels {v} is out of range")),
    }
}

/// `objectRefinement::New(name, dict)` and the type's `operator=(dict)`.
///
/// # Errors
/// An unknown `type`, a negative `cellSize` or level, or a missing geometry
/// entry (all fatal upstream).
pub fn object_refinement_from_dict(name: &str, d: &FoamDict) -> Result<ObjectRefinement, String> {
    let at = format!("objectRefinements/{name}/");
    let ty = word(d, "type", &at)?.unwrap_or_else(|| "objectRefinement".into());
    // objectRefinement(name, dict)
    let mut cell_size = -1.0;
    let mut add = 0u8;
    if let Some(cs) = scalar(d, "cellSize", &at)? {
        if cs < 0.0 {
            return Err(format!("Specified cell size for object {name} is negative"));
        }
        cell_size = cs;
    } else if let Some(l) = label(d, "additionalRefinementLevels", &at)? {
        if l < 0 {
            return Err(format!("Specified additionalRefinementLevel for object {name} is negative"));
        }
        add = levels(Some(l), &at)?.unwrap_or(0);
    }
    let thickness = scalar(d, "refinementThickness", &at)?.unwrap_or(0.0);
    // operator=: the entries may sit in a sub-dictionary named after the type
    let g = d.get_dict(&ty).unwrap_or(d);
    let need_v = |k: &str| -> Result<Vec3, String> { vector(g, k, &at)?.ok_or_else(|| format!("{at}: Entry {k} is not specified!")) };
    let need_s = |k: &str| -> Result<f64, String> { scalar(g, k, &at)?.ok_or_else(|| format!("{at}: Entry {k} is not specified!")) };
    let shape = match ty.as_str() {
        "box" => ObjectShape::Box { centre: need_v("centre")?, lengths: Vec3::new(need_s("lengthX")?, need_s("lengthY")?, need_s("lengthZ")?) },
        "sphere" => ObjectShape::Sphere { centre: need_v("centre")?, radius: need_s("radius")? },
        "cone" => ObjectShape::Cone { p0: need_v("p0")?, r0: need_s("radius0")?, p1: need_v("p1")?, r1: need_s("radius1")? },
        "hollowCone" => ObjectShape::HollowCone {
            p0: need_v("p0")?,
            r0_outer: need_s("radius0_Outer")?,
            r0_inner: need_s("radius0_Inner")?,
            p1: need_v("p1")?,
            r1_outer: need_s("radius1_Outer")?,
            r1_inner: need_s("radius1_Inner")?,
        },
        "line" => ObjectShape::Line { p0: need_v("p0")?, p1: need_v("p1")? },
        _ => return Err(format!("Unknown objectRefinement type {ty}; valid types: box cone hollowCone line sphere")),
    };
    Ok(ObjectRefinement { name: name.into(), cell_size, additional_ref_levels: add, refinement_thickness: thickness, shape })
}

/// `coordinateModification::New(name, dict)` (`box`, `plane`).
///
/// # Errors
/// An unknown `type` or a missing entry (fatal upstream).
pub fn coordinate_modification_from_dict(name: &str, d: &FoamDict, warnings: &mut Vec<String>) -> Result<Modification, String> {
    let at = format!("anisotropicSources/{name}/");
    let ty = word(d, "type", &at)?.unwrap_or_else(|| "coordinateModification".into());
    let g = d.get_dict(&ty).unwrap_or(d);
    let need_v = |k: &str| -> Result<Vec3, String> { vector(g, k, &at)?.ok_or_else(|| format!("{at}: Entry {k} is not specified!")) };
    let need_s = |k: &str| -> Result<f64, String> { scalar(g, k, &at)?.ok_or_else(|| format!("{at}: Entry {k} is not specified!")) };
    match ty.as_str() {
        "box" => Ok(Modification::Box(BoxScaling {
            name: name.into(),
            centre: need_v("centre")?,
            length: Vec3::new(need_s("lengthX")?, need_s("lengthY")?, need_s("lengthZ")?),
            scale: Vec3::new(scalar(g, "scaleX", &at)?.unwrap_or(1.0), scalar(g, "scaleY", &at)?.unwrap_or(1.0), scalar(g, "scaleZ", &at)?.unwrap_or(1.0)),
        })),
        "plane" => {
            let scaling_factor = match scalar(g, "scalingFactor", &at)? {
                Some(f) => f,
                None => {
                    warnings.push(format!("{at}: Entry scalingFactor is not specified!"));
                    1.0
                }
            };
            // the dictionary constructor keeps `normal` as written
            Ok(Modification::Plane(PlaneScaling { name: name.into(), origin: need_v("origin")?, normal: need_v("normal")?, scaling_distance: need_s("scalingDistance")?, scaling_factor }))
        }
        _ => Err(format!("Unknown coordinateModification type {ty}; valid types: box plane")),
    }
}

/// Tracks the entries of one (sub-)dictionary that were met.
struct Table {
    rows: Vec<KeywordUse>,
}

impl Table {
    fn add(&mut self, path: &str, status: KeywordStatus) {
        self.rows.push(KeywordUse { path: path.into(), status });
    }

    /// Mark every key of `d` not in `known` as unknown.
    fn unknown_keys(&mut self, d: &FoamDict, prefix: &str, known: &[&str]) {
        for (k, _) in d.iter() {
            if !known.contains(&k) {
                self.add(&format!("{prefix}{k}"), KeywordStatus::Unknown);
            }
        }
    }
}

fn reported(s: &str) -> KeywordStatus {
    KeywordStatus::Reported(s.into())
}

fn not_applicable(s: &str) -> KeywordStatus {
    KeywordStatus::NotApplicable(s.into())
}

/// The top-level `meshDict` keywords any cfMesh code reads.
pub const TOP_LEVEL_KEYWORDS: &[&str] = &[
    "surfaceFile",
    "maxCellSize",
    "minCellSize",
    "boundaryCellSize",
    "boundaryCellSizeRefinementThickness",
    "patchCellSize",
    "subsetCellSize",
    "localRefinement",
    "objectRefinements",
    "surfaceMeshRefinement",
    "edgeMeshRefinement",
    "anisotropicSources",
    "keepCellsIntersectingBoundary",
    "checkForGluedMesh",
    "nonManifoldMeshing",
    "keepCellsIntersectingPatches",
    "removeCellsIntersectingPatches",
    "enforceGeometryConstraints",
    "enforceConstraints",
    "decomposePolyhedraIntoTetsAndPyrs",
    "boundaryLayers",
    "renameBoundary",
    "workflowControls",
    "meshQualitySettings",
];

/// Read `meshDict` for `generator` (step 1, module docs). Runs no
/// validation beyond what reading needs; call [`check_mesh_dict`] first, as
/// the generators do.
///
/// # Errors
/// A missing `surfaceFile` or `maxCellSize`, an entry of the wrong type, or
/// an object / anisotropic source upstream would refuse (all fatal there).
pub fn read_mesh_dict(dict: &FoamDict, generator: Generator) -> Result<MeshDictReading, String> {
    let mut t = Table { rows: Vec::new() };
    let mut warnings = Vec::new();
    let g = generator;
    let surface_file = word(dict, "surfaceFile", "")?.ok_or("Cannot find keyword surfaceFile in meshDict")?;
    t.add("surfaceFile", KeywordStatus::Honoured);
    let max_cell_size = scalar(dict, "maxCellSize", "")?.ok_or("Cannot find keyword maxCellSize in meshDict")?;
    t.add("maxCellSize", KeywordStatus::Honoured);
    let min_cell_size = scalar(dict, "minCellSize", "")?;
    if min_cell_size.is_some() {
        t.add("minCellSize", KeywordStatus::Honoured);
    }
    let boundary_cell_size = scalar(dict, "boundaryCellSize", "")?;
    if boundary_cell_size.is_some() {
        t.add("boundaryCellSize", KeywordStatus::Honoured);
    }
    let mut boundary_cell_size_refinement_thickness = 0.0;
    if let Some(x) = scalar(dict, "boundaryCellSizeRefinementThickness", "")? {
        if boundary_cell_size.is_some() {
            boundary_cell_size_refinement_thickness = x.abs();
            t.add("boundaryCellSizeRefinementThickness", KeywordStatus::Honoured);
        } else {
            t.add("boundaryCellSizeRefinementThickness", not_applicable("read only with boundaryCellSize"));
        }
    }

    // patchCellSize
    let mut patch_cell_size = Vec::new();
    if let Some(e) = dict.get("patchCellSize") {
        for (name, sub) in named_dicts(e, "patchCellSize")? {
            let Some(sub) = sub else { continue };
            let cs = scalar(&sub, "cellSize", &format!("patchCellSize/{name}/"))?.ok_or_else(|| format!("patchCellSize/{name}: Cannot find keyword cellSize"))?;
            patch_cell_size.push(NamedRefinement { name, additional_levels: None, cell_size: Some(cs), refinement_thickness: 0.0 });
        }
        t.add("patchCellSize", KeywordStatus::Honoured);
    }
    // subsetCellSize
    let mut subset_cell_size = Vec::new();
    if let Some(e) = dict.get("subsetCellSize") {
        for (name, sub) in named_dicts(e, "subsetCellSize")? {
            let Some(sub) = sub else { continue };
            let cs = scalar(&sub, "cellSize", &format!("subsetCellSize/{name}/"))?.ok_or_else(|| format!("subsetCellSize/{name}: Cannot find keyword cellSize"))?;
            subset_cell_size.push(NamedRefinement { name, additional_levels: None, cell_size: Some(cs), refinement_thickness: 0.0 });
        }
        t.add("subsetCellSize", if subset_cell_size.is_empty() { KeywordStatus::Honoured } else { reported("the generators take no facet subsets, so these sizes do not reach the octree") });
    }
    // localRefinement
    let mut local_refinement = Vec::new();
    if let Some(e) = dict.get("localRefinement") {
        if let FoamEntry::SubDict(ld) = e {
            for (name, v) in ld.iter() {
                let FoamEntry::SubDict(sub) = v else { continue };
                let at = format!("localRefinement/{name}/");
                let additional_levels = levels(label(sub, "additionalRefinementLevels", &at)?, &at)?;
                let cell_size = if additional_levels.is_none() { scalar(sub, "cellSize", &at)? } else { None };
                let refinement_thickness = scalar(sub, "refinementThickness", &at)?.unwrap_or(0.0);
                t.unknown_keys(sub, &at, &["additionalRefinementLevels", "cellSize", "refinementThickness"]);
                local_refinement.push(NamedRefinement { name: name.to_string(), additional_levels, cell_size, refinement_thickness });
            }
            t.add("localRefinement", KeywordStatus::Honoured);
        } else {
            t.add("localRefinement", not_applicable("not a dictionary: setRootCubeSizeAndRefParameters reads only the dictionary form"));
        }
    }
    // objectRefinements
    let mut object_refinements = Vec::new();
    if let Some(e) = dict.get("objectRefinements") {
        for (name, sub) in named_dicts(e, "objectRefinements")? {
            let sub = sub.ok_or_else(|| format!("objectRefinements/{name}: not a dictionary"))?;
            object_refinements.push(object_refinement_from_dict(&name, &sub)?);
        }
        t.add("objectRefinements", KeywordStatus::Honoured);
    }
    // surfaceMeshRefinement / edgeMeshRefinement
    let read_sources = |key: &str, file_key: &str| -> Result<Vec<SourceEntry>, String> {
        let mut out = Vec::new();
        if let Some(sd) = dict.get_dict(key) {
            for (name, v) in sd.iter() {
                let FoamEntry::SubDict(sub) = v else { return Err(format!("Dictionary {name} does not exist!!")) };
                let at = format!("{key}/{name}/");
                let file = word(sub, file_key, &at)?.ok_or_else(|| format!("Missing {file_key} for entry {name}"))?;
                let cell_size = scalar(sub, "cellSize", &at)?;
                let additional_levels = if cell_size.is_none() { levels(label(sub, "additionalRefinementLevels", &at)?, &at)? } else { None };
                let refinement_thickness = scalar(sub, "refinementThickness", &at)?.unwrap_or(0.0);
                out.push(SourceEntry { name: name.to_string(), file, cell_size, additional_levels, refinement_thickness });
            }
        }
        Ok(out)
    };
    let surface_mesh_refinement = read_sources("surfaceMeshRefinement", "surfaceFile")?;
    if dict.get("surfaceMeshRefinement").is_some() {
        t.add("surfaceMeshRefinement", KeywordStatus::Honoured);
    }
    let edge_mesh_refinement = read_sources("edgeMeshRefinement", "edgeFile")?;
    if dict.get("edgeMeshRefinement").is_some() {
        t.add("edgeMeshRefinement", if edge_mesh_refinement.is_empty() { KeywordStatus::Honoured } else { reported("no edgeMesh file reader in this port: the sources are dropped") });
    }
    // anisotropicSources
    let mut anisotropic_sources = None;
    if let Some(e) = dict.get("anisotropicSources") {
        let FoamEntry::SubDict(ad) = e else { return Err("anisotropicSources is not a dictionary".into()) };
        let mut mods = Vec::new();
        for (name, v) in ad.iter() {
            let FoamEntry::SubDict(sub) = v else { return Err(format!("anisotropicSources/{name} is not a dictionary")) };
            mods.push(coordinate_modification_from_dict(name, sub, &mut warnings)?);
        }
        anisotropic_sources = Some(CoordinateModifier::new(mods).map_err(|e| format!("anisotropicSources: {e:?}"))?);
        t.add("anisotropicSources", KeywordStatus::Honoured);
    }
    // octree addressing
    let addressing_status = |t: &mut Table, k: &str| {
        let s = if g.is_tet_or_poly() { reported("the tet / Voronoi templates build meshOctreeAddressing without the dictionary in this port") } else { KeywordStatus::Honoured };
        t.add(k, s);
    };
    let keep_cells_intersecting_boundary = boolean(dict, "keepCellsIntersectingBoundary", "")?.unwrap_or(false);
    if dict.get("keepCellsIntersectingBoundary").is_some() {
        t.add("keepCellsIntersectingBoundary", KeywordStatus::Honoured);
    }
    let check_for_glued_mesh = boolean(dict, "checkForGluedMesh", "")?.unwrap_or(false);
    if dict.get("checkForGluedMesh").is_some() {
        addressing_status(&mut t, "checkForGluedMesh");
    }
    let non_manifold_meshing = boolean(dict, "nonManifoldMeshing", "")?.unwrap_or(false);
    if dict.get("nonManifoldMeshing").is_some() {
        addressing_status(&mut t, "nonManifoldMeshing");
    }
    let names_of = |k: &str| -> Result<Vec<String>, String> {
        match dict.get(k) {
            None => Ok(Vec::new()),
            Some(FoamEntry::SubDict(d)) => Ok(d.iter().map(|(n, _)| n.to_string()).collect()),
            Some(e) => word_list(e, k),
        }
    };
    let keep_cells_intersecting_patches = names_of("keepCellsIntersectingPatches")?;
    if dict.get("keepCellsIntersectingPatches").is_some() {
        addressing_status(&mut t, "keepCellsIntersectingPatches");
    }
    let remove_cells_intersecting_patches = names_of("removeCellsIntersectingPatches")?;
    if dict.get("removeCellsIntersectingPatches").is_some() {
        addressing_status(&mut t, "removeCellsIntersectingPatches");
    }
    // generator switches
    let enforce_geometry_constraints = boolean(dict, "enforceGeometryConstraints", "")?.unwrap_or(false);
    if dict.get("enforceGeometryConstraints").is_some() {
        t.add("enforceGeometryConstraints", if g == Generator::Cartesian2DMesh { not_applicable("cartesian2DMeshGenerator does not read it") } else { KeywordStatus::Honoured });
    }
    if dict.get("enforceConstraints").is_some() {
        boolean(dict, "enforceConstraints", "")?;
        t.add("enforceConstraints", not_applicable("read by checkMeshDict only; no generator acts on it"));
    }
    let decompose_polyhedra_into_tets_and_pyrs = boolean(dict, "decomposePolyhedraIntoTetsAndPyrs", "")?.unwrap_or(false);
    if dict.get("decomposePolyhedraIntoTetsAndPyrs").is_some() {
        t.add("decomposePolyhedraIntoTetsAndPyrs", if g.is_tet_or_poly() { not_applicable("only cartesianMesh and cartesian2DMesh read it") } else { KeywordStatus::Honoured });
    }
    if dict.get("meshQualitySettings").is_some() {
        t.add("meshQualitySettings", reported("the findBadFaces thresholds it sets are not wired into this port's optimiser"));
        if let Some(q) = dict.get_dict("meshQualitySettings") {
            t.unknown_keys(q, "meshQualitySettings/", &["maxNonOrthogonality", "maxSkewness", "minPyramidVolume", "faceFlatness", "minCellPartTetrahedra", "minimumFaceArea"]);
        }
    }
    // boundaryLayers
    let boundary_layers = match dict.get("boundaryLayers") {
        Some(FoamEntry::SubDict(b)) => Some(read_boundary_layers(b, g, &mut t, &mut warnings)?),
        Some(_) => {
            t.add("boundaryLayers", not_applicable("not a dictionary: no generator reads it"));
            None
        }
        None => None,
    };
    // renameBoundary
    let rename_boundary = match dict.get_dict("renameBoundary") {
        Some(r) => {
            let mut rb = RenameBoundary { default_name: word(r, "defaultName", "renameBoundary/")?, default_type: word(r, "defaultType", "renameBoundary/")?, ..RenameBoundary::default() };
            if let Some(e) = r.get("newPatchNames") {
                for (name, sub) in named_dicts(e, "renameBoundary/newPatchNames")? {
                    let np = match sub {
                        Some(s) => {
                            let at = format!("renameBoundary/newPatchNames/{name}/");
                            t.unknown_keys(&s, &at, &["newName", "type"]);
                            Some(NewPatch { new_name: word(&s, "newName", &at)?, patch_type: word(&s, "type", &at)? })
                        }
                        None => None,
                    };
                    rb.new_patch_names.push((name, np));
                }
            }
            t.unknown_keys(r, "renameBoundary/", &["newPatchNames", "defaultName", "defaultType"]);
            t.add("renameBoundary", KeywordStatus::Honoured);
            Some(rb)
        }
        None => None,
    };
    // workflowControls
    let mut workflow = WorkflowSettings::default();
    if let Some(e) = dict.get("workflowControls") {
        if let FoamEntry::SubDict(w) = e {
            if let Some(s) = word(w, "stopAfter", "workflowControls/")? {
                match WorkflowStep::from_name(&s) {
                    Some(step) => workflow.stop_after = Some(step),
                    None => warnings.push(format!("workflowControls/stopAfter {s}: not a workflow step; upstream never stops on it")),
                }
            }
            workflow.restart_from_latest_step = boolean(w, "restartFromLatestStep", "workflowControls/")?.unwrap_or(false);
            t.unknown_keys(w, "workflowControls/", &["stopAfter", "restartFromLatestStep"]);
        }
        t.add("workflowControls", KeywordStatus::Honoured);
    }
    t.unknown_keys(dict, "", TOP_LEVEL_KEYWORDS);
    Ok(MeshDictReading {
        generator,
        surface_file,
        max_cell_size,
        min_cell_size,
        boundary_cell_size,
        boundary_cell_size_refinement_thickness,
        patch_cell_size,
        subset_cell_size,
        local_refinement,
        object_refinements,
        surface_mesh_refinement,
        edge_mesh_refinement,
        anisotropic_sources,
        keep_cells_intersecting_boundary,
        check_for_glued_mesh,
        non_manifold_meshing,
        keep_cells_intersecting_patches,
        remove_cells_intersecting_patches,
        enforce_geometry_constraints,
        decompose_polyhedra_into_tets_and_pyrs,
        boundary_layers,
        rename_boundary,
        workflow,
        keywords: t.rows,
        warnings,
    })
}

/// The `boundaryLayers` dictionary: `refineBoundaryLayers::readSettings`,
/// `boundaryLayers`' height factors, `generateBoundaryLayers` (tet / poly),
/// `meshOptimizer`'s `optimiseLayer` and `untangleLayers`.
fn read_boundary_layers(b: &FoamDict, g: Generator, t: &mut Table, warnings: &mut Vec<String>) -> Result<BoundaryLayersEntry, String> {
    let at = "boundaryLayers/";
    let mut e = BoundaryLayersEntry { heights: BoundaryLayersControls::default(), ..BoundaryLayersEntry::default() };
    t.add("boundaryLayers", KeywordStatus::Honoured);
    if let Some(n) = label(b, "nLayers", at)? {
        if n < 0 {
            return Err(format!("{at}nLayers {n} is negative"));
        }
        if n < 2 {
            warnings.push(format!("The specified number of boundary layers is {n}, less than 2: no refinement of the layer"));
        }
        e.n_layers = Some(n as usize);
        t.add("boundaryLayers/nLayers", KeywordStatus::Honoured);
    }
    if let Some(r) = scalar(b, "thicknessRatio", at)? {
        e.thickness_ratio = Some(r);
        t.add("boundaryLayers/thicknessRatio", KeywordStatus::Honoured);
    }
    if let Some(x) = scalar(b, "maxFirstLayerThickness", at)? {
        e.max_first_layer_thickness = Some(x);
        t.add("boundaryLayers/maxFirstLayerThickness", KeywordStatus::Honoured);
    }
    if let Some(x) = b.get("excludedPatches") {
        e.excluded_patches = word_list(x, "boundaryLayers/excludedPatches")?;
        t.add("boundaryLayers/excludedPatches", KeywordStatus::Honoured);
    }
    if let Some(x) = scalar(b, "boundaryLayerHeightFactor", at)? {
        e.heights.height_factor = x;
        t.add("boundaryLayers/boundaryLayerHeightFactor", KeywordStatus::Honoured);
    }
    if let Some(x) = scalar(b, "boundaryLayerMinHeight", at)? {
        e.heights.min_height = x;
        t.add("boundaryLayers/boundaryLayerMinHeight", KeywordStatus::Honoured);
    }
    if let Some(p) = b.get("patchBoundaryLayers") {
        e.has_patch_boundary_layers = true;
        if let FoamEntry::SubDict(pd) = p {
            for (name, v) in pd.iter() {
                e.patch_layer_toc.push(name.to_string());
                let FoamEntry::SubDict(s) = v else {
                    warnings.push(format!("Cannot refine layer for patch {name}"));
                    continue;
                };
                let pat = format!("{at}patchBoundaryLayers/{name}/");
                let n = label(s, "nLayers", &pat)?;
                if let Some(n) = n.filter(|&n| n < 2) {
                    warnings.push(format!("The specified number of boundary layers for patch {name} is {n}, less than 2: boundary layers disabled for this patch"));
                }
                let n_layers = n.map(|n| n.max(0) as usize);
                let thickness_ratio = scalar(s, "thicknessRatio", &pat)?;
                if let Some(r) = thickness_ratio.filter(|&r| r < 1.0) {
                    warnings.push(format!("The specified thickness ratio {r} for patch {name} is less than 1.0"));
                }
                let max_first_layer_thickness = scalar(s, "maxFirstLayerThickness", &pat)?;
                let allow_discontinuity = boolean(s, "allowDiscontinuity", &pat)?.unwrap_or(false);
                t.unknown_keys(s, &pat, &["nLayers", "thicknessRatio", "maxFirstLayerThickness", "allowDiscontinuity"]);
                e.patch_layers.push(PatchLayerEntry { name: name.to_string(), n_layers, thickness_ratio, max_first_layer_thickness, allow_discontinuity });
            }
        }
        t.add("boundaryLayers/patchBoundaryLayers", KeywordStatus::Honoured);
    }
    // optimizeBoundaryLayer: tetMesh and pMesh call it with
    // addBufferLayer false; cartesianMesh with true unless anisotropic
    // sources are set, and the buffer layer
    // (refineBoundaryLayers::activateSpecialMode) is not ported.
    let optimiser = |what: &str| -> KeywordStatus {
        match g {
            Generator::Cartesian2DMesh => not_applicable("cartesian2DMeshGenerator does not call meshOptimizer::optimizeBoundaryLayer"),
            Generator::CartesianMesh => KeywordStatus::Reported(format!("{what}: optimizeBoundaryLayer runs, but without its buffer layer (addBufferLayer: refineBoundaryLayers::activateSpecialMode is not ported)")),
            Generator::TetMesh | Generator::PMesh => KeywordStatus::Honoured,
        }
    };
    if let Some(o) = boolean(b, "optimiseLayer", at)? {
        e.optimise_layer = Some(o);
        t.add("boundaryLayers/optimiseLayer", if o { optimiser("optimiseLayer on") } else { KeywordStatus::Honoured });
    }
    if let Some(op) = b.get("optimisationParameters") {
        let FoamEntry::SubDict(od) = op else { return Err(format!("{at}optimisationParameters is not a dictionary")) };
        let pat = "boundaryLayers/optimisationParameters/";
        let c = &mut e.optimisation;
        if let Some(r) = boolean(od, "recalculateNormals", pat)? {
            c.recalculate_normals = r;
        }
        boolean(od, "reCalculateNormals", pat)?;
        if let Some(n) = label(od, "nSmoothNormals", pat)? {
            if n < 0 {
                return Err("nSmoothNormals must not be negative!".into());
            }
            c.n_smooth_normals = n as usize;
        }
        for k in ["featureSizeFactor", "relThicknessTol"] {
            if let Some(x) = scalar(od, k, pat)? {
                if !(0.0..1.0).contains(&x) {
                    return Err(format!("{k} {x} is out of a valid range 0 to 1"));
                }
                if k == "featureSizeFactor" {
                    c.feature_size_factor = x;
                } else {
                    c.rel_thickness_tol = x;
                }
            }
        }
        if let Some(n) = label(od, "maxNumIterations", pat)? {
            if n < 0 {
                return Err("maxNumIterations must not be negative!".into());
            }
            c.max_num_iterations = n as usize;
        }
        if od.get("reCalculateNormals").is_some() {
            t.add("boundaryLayers/optimisationParameters/reCalculateNormals", not_applicable("checkMeshDict's spelling; boundaryLayerOptimisation::readSettings reads recalculateNormals, so upstream ignores it"));
        }
        t.unknown_keys(od, pat, &["recalculateNormals", "reCalculateNormals", "nSmoothNormals", "featureSizeFactor", "relThicknessTol", "maxNumIterations", "relFlatnessTol"]);
        let active = e.optimise_layer == Some(true);
        t.add("boundaryLayers/optimisationParameters", if active { optimiser("optimisationParameters") } else { not_applicable("read only when optimiseLayer is on") });
    }
    if let Some(u) = boolean(b, "untangleLayers", at)? {
        e.untangle_layers = Some(u);
        let s = match (g, u) {
            (Generator::Cartesian2DMesh, _) => not_applicable("cartesian2DMeshGenerator does not call meshOptimizer::untangleBoundaryLayer"),
            (_, true) => KeywordStatus::Honoured,
            (_, false) => reported("untangleLayers false (check face pyramids and stop) is not ported; the layer is untangled"),
        };
        t.add("boundaryLayers/untangleLayers", s);
    }
    t.unknown_keys(
        b,
        at,
        &[
            "nLayers",
            "thicknessRatio",
            "maxFirstLayerThickness",
            "excludedPatches",
            "patchBoundaryLayers",
            "boundaryLayerHeightFactor",
            "boundaryLayerMinHeight",
            "optimiseLayer",
            "optimisationParameters",
            "untangleLayers",
        ],
    );
    Ok(e)
}

// ── checkMeshDict ──────────────────────────────────────────────────────────

/// `checkMeshDict(meshDict)`: `checkEntries`, in upstream's order (which
/// checks neither `localRefinement` nor `edgeMeshRefinement`). Returns
/// the warnings; a fatal error is `Err`. `is_file` answers `isFile(name)`
/// for the refinement-source files (upstream checks them relative to the
/// working directory, normally the case).
///
/// # Errors
/// What upstream stops on (`FatalErrorInFunction` / a failed lookup).
pub fn check_mesh_dict<F: Fn(&str) -> bool>(dict: &FoamDict, is_file: F) -> Result<Vec<String>, String> {
    let mut w = Vec::new();
    check_basic_settings(dict, &mut w)?;
    check_patch_cell_size(dict)?;
    check_subset_cell_size(dict)?;
    check_surface_refinements(dict, &is_file, &mut w)?;
    check_keep_remove_patches(dict)?;
    check_object_refinements(dict, &mut w)?;
    check_anisotropic_sources(dict)?;
    check_boundary_layers(dict, &mut w)?;
    check_rename_boundary(dict)?;
    check_quality_settings(dict)?;
    Ok(w)
}

fn check_basic_settings(d: &FoamDict, w: &mut Vec<String>) -> Result<(), String> {
    let m = scalar(d, "maxCellSize", "")?.ok_or("Cannot find keyword maxCellSize")?;
    if m < 0.0 {
        return Err("maxCellSize is negative! Cannot generate the mesh!!".into());
    }
    if let Some(b) = scalar(d, "boundaryCellSize", "")? {
        if b < 0.0 {
            w.push("Boundary cell size is negative!!".into());
        }
        if scalar(d, "boundaryCellSizeRefinementThickness", "")?.is_some_and(|x| x < 0.0) {
            w.push("Boundary cell size refinement thickness is negative!!".into());
        }
    }
    if scalar(d, "minCellSize", "")?.is_some_and(|x| x < 0.0) {
        return Err("Minimum cell size for automatic refinement is negative!!".into());
    }
    boolean(d, "keepCellsIntersectingBoundary", "")?;
    boolean(d, "checkForGluedMesh", "")?;
    boolean(d, "enforceConstraints", "")?;
    Ok(())
}

fn check_patch_cell_size(d: &FoamDict) -> Result<(), String> {
    match d.get("patchCellSize") {
        None | Some(FoamEntry::SubDict(_)) => Ok(()),
        Some(e) => entry_list(e, "patchCellSize").map(|_| ()),
    }
}

/// Upstream's list branch reads `patchCellSize` (kept).
fn check_subset_cell_size(d: &FoamDict) -> Result<(), String> {
    match d.get("subsetCellSize") {
        None | Some(FoamEntry::SubDict(_)) => Ok(()),
        Some(_) => match d.get("patchCellSize") {
            Some(e) => entry_list(e, "patchCellSize").map(|_| ()),
            None => Err("Cannot find keyword patchCellSize (checkSubsetCellSize's list branch reads it)".into()),
        },
    }
}

/// `checkLocalRefinementLevel`. **Upstream's `checkEntries` does not call
/// it**, and neither does [`check_mesh_dict`].
///
/// # Errors
/// An entry with neither `additionalRefinementLevels` nor `cellSize`, or a
/// `localRefinement` that is not a dictionary.
pub fn check_local_refinement_level(d: &FoamDict) -> Result<Vec<String>, String> {
    let mut w = Vec::new();
    let Some(e) = d.get("localRefinement") else { return Ok(w) };
    let FoamEntry::SubDict(ld) = e else { return Err("Cannot read localRefinement".into()) };
    for (name, v) in ld.iter() {
        let FoamEntry::SubDict(s) = v else { return Err(format!("localRefinement/{name} is not a dictionary")) };
        if let Some(cs) = scalar(s, "cellSize", "")? {
            if cs < 0.0 {
                w.push(format!("Cell size for {name} is negative"));
            }
        } else if let Some(l) = label(s, "additionalRefinementLevels", "")? {
            if l < 0 {
                w.push(format!("Refinement level for {name} is negative"));
            }
        } else {
            return Err(format!("Cannot read keyword additionalRefinementLevels or cellSizefor {name}"));
        }
        if scalar(s, "refinementThickness", "")?.is_some_and(|x| x < 0.0) {
            w.push(format!("Refinement thickness for {name} is negative"));
        }
    }
    Ok(w)
}

fn check_sources<F: Fn(&str) -> bool>(d: &FoamDict, key: &str, file_key: &str, is_file: &F, w: &mut Vec<String>) -> Result<(), String> {
    let Some(e) = d.get(key) else { return Ok(()) };
    let FoamEntry::SubDict(sd) = e else { return Err(format!("{key} is not a dictionary")) };
    for (name, v) in sd.iter() {
        let FoamEntry::SubDict(s) = v else { return Err(format!("Dictionary {name} does not exist!!")) };
        match word(s, file_key, "")? {
            Some(f) if !is_file(&f) => return Err(format!("{file_key} {f} does not exist or is not readable!!")),
            Some(_) => {}
            None => return Err(format!("Missing {file_key} for entry {name}")),
        }
        if let Some(cs) = scalar(s, "cellSize", "")? {
            if cs < 1.0e-300 {
                return Err(format!("Cell size for entry {name} is extremely small or negative!!"));
            }
        } else if let Some(l) = label(s, "additionalRefinementLevels", "")? {
            if l < 0 {
                return Err(format!("Number refinement levels for entry {name} is negative!!"));
            }
        } else {
            return Err(format!("Missing cellSize or additionalRefinementLevels for entry {name}"));
        }
        if scalar(s, "refinementThickness", "")?.is_some_and(|x| x < 1.0e-300) {
            w.push(format!("Refinement thickness for entry {name} is extremely small or negative!!"));
        }
    }
    Ok(())
}

fn check_surface_refinements<F: Fn(&str) -> bool>(d: &FoamDict, is_file: &F, w: &mut Vec<String>) -> Result<(), String> {
    check_sources(d, "surfaceMeshRefinement", "surfaceFile", is_file, w)
}

fn check_keep_remove_patches(d: &FoamDict) -> Result<(), String> {
    for k in ["keepCellsIntersectingPatches", "removeCellsIntersectingPatches"] {
        match d.get(k) {
            None | Some(FoamEntry::SubDict(_)) => {}
            Some(e) => {
                word_list(e, k)?;
            }
        }
    }
    Ok(())
}

fn check_object_refinements(d: &FoamDict, w: &mut Vec<String>) -> Result<(), String> {
    let Some(e) = d.get("objectRefinements") else { return Ok(()) };
    for (name, sub) in named_dicts(e, "objectRefinements")? {
        let sub = sub.ok_or_else(|| format!("objectRefinements/{name}: not a dictionary"))?;
        let o = object_refinement_from_dict(&name, &sub)?;
        if o.refinement_thickness < 0.0 {
            w.push(format!("Refinement thickness specified for object {name} is negative!!"));
        }
    }
    Ok(())
}

fn check_anisotropic_sources(d: &FoamDict) -> Result<(), String> {
    if let Some(FoamEntry::SubDict(ad)) = d.get("anisotropicSources") {
        let mut sink = Vec::new();
        for (name, v) in ad.iter() {
            let FoamEntry::SubDict(sub) = v else { return Err(format!("anisotropicSources/{name} is not a dictionary")) };
            coordinate_modification_from_dict(name, sub, &mut sink)?;
        }
    }
    Ok(())
}

fn check_boundary_layers(d: &FoamDict, w: &mut Vec<String>) -> Result<(), String> {
    let Some(e) = d.get("boundaryLayers") else { return Ok(()) };
    let FoamEntry::SubDict(b) = e else { return Err("boundaryLayers is not a dictionary".into()) };
    let at = "boundaryLayers/";
    label(b, "nLayers", at)?;
    scalar(b, "thicknessRatio", at)?;
    scalar(b, "maxFirstLayerThickness", at)?;
    if let Some(FoamEntry::SubDict(pd)) = b.get("patchBoundaryLayers") {
        for (name, v) in pd.iter() {
            match v {
                FoamEntry::SubDict(s) => {
                    label(s, "nLayers", at)?;
                    scalar(s, "thicknessRatio", at)?;
                    scalar(s, "maxFirstLayerThickness", at)?;
                    boolean(s, "allowDiscontinuity", at)?;
                }
                _ => w.push(format!("Cannot refine layer for patch {name}")),
            }
        }
    }
    boolean(b, "optimiseLayer", at)?;
    if let Some(op) = b.get("optimisationParameters") {
        let FoamEntry::SubDict(od) = op else { return Err("optimisationParameters is not a dictionary".into()) };
        boolean(od, "reCalculateNormals", at)?;
        if label(od, "nSmoothNormals", at)?.is_some_and(|n| n < 0) {
            return Err("nSmoothNormals must not be negative!".into());
        }
        if scalar(od, "featureSizeFactor", at)?.is_some_and(|x| !(0.0..1.0).contains(&x)) {
            return Err("Feature size factor is out of a valid range 0 to 1".into());
        }
        if scalar(od, "relThicknessTol", at)?.is_some_and(|x| !(0.0..1.0).contains(&x)) {
            return Err("Relative thickness tolerance is out of a valid range 0 to 1".into());
        }
        if label(od, "maxNumIterations", at)?.is_some_and(|n| n < 0) {
            return Err("maxNumIterations must not be negative!".into());
        }
    }
    Ok(())
}

fn check_rename_boundary(d: &FoamDict) -> Result<(), String> {
    let Some(r) = d.get("renameBoundary") else { return Ok(()) };
    let FoamEntry::SubDict(r) = r else { return Err("renameBoundary is not a dictionary".into()) };
    let Some(e) = r.get("newPatchNames") else { return Ok(()) };
    for (name, sub) in named_dicts(e, "renameBoundary/newPatchNames")? {
        let sub = sub.ok_or_else(|| format!("Entry {name} is not a dictionary"))?;
        if sub.get("newName").is_none() {
            return Err(format!("Dictionary {name} does not contain a newName keyword"));
        }
    }
    Ok(())
}

fn check_quality_settings(d: &FoamDict) -> Result<(), String> {
    let Some(q) = d.get("meshQualitySettings") else { return Ok(()) };
    let FoamEntry::SubDict(q) = q else { return Err("meshQualitySettings is not a dictionary".into()) };
    for k in ["maxNonOrthogonality", "maxSkewness", "minPyramidVolume", "faceFlatness", "minCellPartTetrahedra", "minimumFaceArea"] {
        scalar(q, k, "meshQualitySettings/")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;

    fn parse(text: &str) -> FoamDict {
        FoamFile::parse(text).expect("meshDict parses").dict
    }

    fn status<'a>(r: &'a MeshDictReading, path: &str) -> &'a KeywordStatus {
        &r.keywords.iter().find(|k| k.path == path).unwrap_or_else(|| panic!("no row {path}: {:#?}", r.keywords)).status
    }

    /// Every keyword is accounted for: a typo is `Unknown`, a key the
    /// generator does not read is `NotApplicable`, a key the port does not
    /// act on is `Reported`.
    #[test]
    fn every_entry_gets_a_row() {
        let d = parse(
            "surfaceFile \"s.stl\"; maxCellSize 1; maxCelSize 2; decomposePolyhedraIntoTetsAndPyrs 1; \
             meshQualitySettings { maxSkewness 4; } boundaryLayers { nLayers 3; optimiseLayer on; untangleLayers off; foo 1; }",
        );
        let r = read_mesh_dict(&d, Generator::TetMesh).unwrap();
        assert_eq!(status(&r, "maxCelSize"), &KeywordStatus::Unknown);
        assert_eq!(status(&r, "boundaryLayers/foo"), &KeywordStatus::Unknown);
        assert!(matches!(status(&r, "decomposePolyhedraIntoTetsAndPyrs"), KeywordStatus::NotApplicable(_)));
        assert!(matches!(status(&r, "meshQualitySettings"), KeywordStatus::Reported(_)));
        assert_eq!(status(&r, "boundaryLayers/optimiseLayer"), &KeywordStatus::Honoured);
        assert!(matches!(status(&r, "boundaryLayers/untangleLayers"), KeywordStatus::Reported(_)));
        assert_eq!(r.unknown().len(), 2);
        let c = read_mesh_dict(&d, Generator::CartesianMesh).unwrap();
        assert_eq!(status(&c, "decomposePolyhedraIntoTetsAndPyrs"), &KeywordStatus::Honoured);
        assert!(matches!(status(&c, "boundaryLayers/optimiseLayer"), KeywordStatus::Reported(_)), "cartesianMesh: no buffer layer");
        assert!(c.decompose_polyhedra_into_tets_and_pyrs);
    }

    /// `optimiseLayer on` with `optimisationParameters` reaches the
    /// generators' `optimise_layer` (`boundaryLayerOptimisation::readSettings`
    /// over the `(mesh, mse)` defaults); the misspelt `reCalculateNormals`
    /// sets nothing, as upstream.
    #[test]
    fn optimise_layer_reaches_the_options() {
        let d = parse(
            "surfaceFile s.stl; maxCellSize 1; boundaryLayers { nLayers 2; optimiseLayer 1; \
             optimisationParameters { nSmoothNormals 3; maxNumIterations 2; featureSizeFactor 0.4; reCalculateNormals 0; relThicknessTol 0.1; } }",
        );
        let s = surface(&["walls"]);
        let (GeneratorOptions::PMesh(o), _) = read_mesh_dict(&d, Generator::PMesh).unwrap().resolve(&s, |_| Err(String::new())).unwrap() else { panic!() };
        let c = o.optimise_layer.unwrap();
        assert_eq!((c.n_smooth_normals, c.max_num_iterations, c.feature_size_factor, c.rel_thickness_tol, c.recalculate_normals), (3, 2, 0.4, 0.1, true));
        let off = parse("surfaceFile s.stl; maxCellSize 1; boundaryLayers { optimiseLayer 0; }");
        let (GeneratorOptions::CartesianMesh(o), _) = read_mesh_dict(&off, Generator::CartesianMesh).unwrap().resolve(&s, |_| Err(String::new())).unwrap() else { panic!() };
        assert!(o.optimise_layer.is_none());
    }

    fn surface(names: &[&str]) -> TriSurf {
        let regions = crate::patches::SurfaceRegions { names: names.iter().map(|s| s.to_string()).collect(), region_of_tri: (0..names.len()).collect(), types: vec!["patch".to_string(); names.len()] };
        let pts = (0..names.len() + 2).map(|i| Vec3::new(i as f64, (i * i) as f64, 0.0)).collect();
        let tris = (0..names.len()).map(|i| [i, i + 1, i + 2]).collect();
        TriSurf::with_surface_regions(pts, tris, &regions)
    }

    /// Patch names are matched as full regular expressions
    /// (`findPatches`), per entry, into region indices; refinement settings
    /// are expanded per matched patch with later entries overriding.
    #[test]
    fn patterns_resolve_against_the_surface() {
        let d = parse(
            "surfaceFile s.stl; maxCellSize 5; localRefinement { \"orifice0[3-6].*\" { cellSize 0.3; } inlet_S1 { additionalRefinementLevels 2; cellSize 9; } nothing { cellSize 1; } } \
             boundaryLayers { nLayers 4; excludedPatches (inlet_S1); patchBoundaryLayers { \"orifice.*\" { nLayers 3; thicknessRatio 0.5; maxFirstLayerThickness 0.2; } orifice04_S2 { allowDiscontinuity 1; } } } \
             renameBoundary { defaultName walls; newPatchNames { \"inlet.*\" { newName inlet; type patch; } } }",
        );
        let s = surface(&["inlet_S1", "orifice03_S1", "orifice04_S2", "orifice07_S3"]);
        let r = read_mesh_dict(&d, Generator::CartesianMesh).unwrap();
        assert!(r.unhandled().is_empty(), "{:?}", r.unhandled());
        let (o, notes) = r.resolve(&s, |_| Err("no files".into())).unwrap();
        let GeneratorOptions::CartesianMesh(o) = o else { panic!() };
        let lr: Vec<(usize, Option<u8>, Option<f64>)> = o.dict.local_refinement.iter().map(|l| (l.region, l.additional_levels, l.cell_size)).collect();
        assert_eq!(lr, vec![(1, None, Some(0.3)), (2, None, Some(0.3)), (0, Some(2), None)]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        let b = o.boundary_layers.unwrap();
        assert_eq!(b.n_layers, 4);
        assert_eq!(b.patches["inlet_S1"].n_layers, Some(1));
        assert_eq!(b.patches["orifice03_S1"].n_layers, Some(3));
        assert_eq!(b.patches["orifice03_S1"].thickness_ratio, None, "a ratio below 1 is not set");
        assert_eq!(b.patches["orifice04_S2"].max_first_layer_thickness, Some(0.2));
        assert!(b.patches["orifice04_S2"].allow_discontinuity);
        assert!(!b.patches["orifice03_S1"].allow_discontinuity);
        assert!(!b.patches.contains_key("orifice07_S3") || b.patches["orifice07_S3"].n_layers == Some(3));
        let rb = o.rename_boundary.unwrap();
        assert_eq!(rb.default_name.as_deref(), Some("walls"));
        assert_eq!(rb.new_patch_names[0].1.as_ref().unwrap().new_name.as_deref(), Some("inlet"));
        // tetMesh: generateBoundaryLayers uses nLayers; the exact names are kept
        let t = read_mesh_dict(&d, Generator::TetMesh).unwrap();
        let (GeneratorOptions::TetMesh(to), _) = t.resolve(&s, |_| Err("no files".into())).unwrap() else { panic!() };
        let bl = to.boundary_layers.unwrap();
        assert_eq!(bl.n_layers, Some(4));
        assert_eq!(bl.patch_layer_names, Some(vec!["orifice.*".to_string(), "orifice04_S2".to_string()]));
    }

    /// Objects, anisotropic sources and workflow controls read as upstream;
    /// a plane's normal is kept as written.
    #[test]
    fn objects_sources_and_workflow() {
        let d = parse(
            "surfaceFile s.fms; maxCellSize 1; objectRefinements { b { type box; cellSize 0.1; centre (0 0 0); lengthX 1; lengthY 2; lengthZ 3; } \
             l { type line; additionalRefinementLevels 2; refinementThickness 0.5; p0 (0 0 0); p1 (1 1 1); } } \
             anisotropicSources { P { type plane; origin (0 0 -20); normal (0 0 2); scalingDistance 45; scalingFactor 2; } } \
             workflowControls { stopAfter edgeExtraction; restartFromLatestStep 1; }",
        );
        let r = read_mesh_dict(&d, Generator::PMesh).unwrap();
        assert!(r.unhandled().is_empty(), "{:?}", r.unhandled());
        assert_eq!(r.object_refinements.len(), 2);
        assert_eq!(r.object_refinements[0].cell_size, 0.1);
        assert_eq!(r.object_refinements[1].additional_ref_levels, 2);
        assert_eq!(r.object_refinements[1].refinement_thickness, 0.5);
        let cm = r.anisotropic_sources.as_ref().unwrap();
        let Modification::Plane(p) = &cm.modifiers[0] else { panic!() };
        assert_eq!(p.normal, Vec3::new(0.0, 0.0, 2.0));
        assert_eq!(r.workflow.stop_after, Some(WorkflowStep::EdgeExtraction));
        assert!(r.workflow.restart_from_latest_step);
        assert!(read_mesh_dict(&parse("surfaceFile s; maxCellSize 1; objectRefinements { x { type cube; cellSize 1; } }"), Generator::PMesh).is_err());
        assert!(read_mesh_dict(&parse("maxCellSize 1;"), Generator::PMesh).is_err());
    }

    /// `checkMeshDict`: what upstream stops on and warns about, including
    /// the kept defects (no `checkLocalRefinementLevel`; the subset list
    /// branch reads `patchCellSize`).
    #[test]
    fn check_mesh_dict_matches_upstream() {
        assert!(check_mesh_dict(&parse("maxCellSize -1;"), |_| true).is_err());
        assert!(check_mesh_dict(&parse("surfaceFile s; maxCellSize 1; minCellSize -1;"), |_| true).is_err());
        let w = check_mesh_dict(&parse("maxCellSize 1; boundaryCellSize -1; boundaryCellSizeRefinementThickness -2;"), |_| true).unwrap();
        assert_eq!(w.len(), 2);
        // not called by checkEntries
        let bad_local = parse("maxCellSize 1; localRefinement { a { refinementThickness 1; } }");
        assert!(check_mesh_dict(&bad_local, |_| true).is_ok());
        assert!(check_local_refinement_level(&bad_local).is_err());
        assert!(check_mesh_dict(&parse("maxCellSize 1; subsetCellSize ( a { cellSize 1; } );"), |_| true).is_err());
        let src = "maxCellSize 1; surfaceMeshRefinement { hull { surfaceFile \"h.stl\"; additionalRefinementLevels 1; } }";
        assert!(check_mesh_dict(&parse(src), |f| f == "h.stl").is_ok());
        assert!(check_mesh_dict(&parse(src), |_| false).is_err());
        assert!(check_mesh_dict(&parse("maxCellSize 1; renameBoundary { newPatchNames { a { type wall; } } }"), |_| true).is_err());
        assert!(check_mesh_dict(&parse("maxCellSize 1; boundaryLayers { optimisationParameters { featureSizeFactor 1.5; } }"), |_| true).is_err());
        assert!(check_mesh_dict(&parse("maxCellSize 1; meshQualitySettings { maxSkewness big; }"), |_| true).is_err());
    }

    /// The tutorial cases gh:#1476 names (cfMesh `e944f6ba695f`, copied
    /// verbatim to `test_data/cfmesh_tutorials/`, see `References.md`) and
    /// the surface each is run on (`None`: not copied).
    pub(crate) const TUTORIALS: &[(&str, Generator, Option<&str>)] = &[
        ("cartesianMesh/asmoOctree", Generator::CartesianMesh, Some("cartesianMesh/asmoOctree/geom.stl")),
        ("cartesianMesh/elbow_90degree", Generator::CartesianMesh, Some("cartesianMesh/elbow_90degree/elbow_90degree.stl")),
        // the cartesianMesh copy is the pMesh one with CRLF line ends
        ("cartesianMesh/multipleOrifices", Generator::CartesianMesh, Some("pMesh/multipleOrifices/multipleOrifices.stl")),
        ("cartesianMesh/singleOrifice", Generator::CartesianMesh, Some("cartesianMesh/singleOrifice/singleOrifice.stl")),
        ("cartesianMesh/ship5415Octree", Generator::CartesianMesh, None),
        ("pMesh/multipleOrifices", Generator::PMesh, Some("pMesh/multipleOrifices/multipleOrifices.stl")),
        ("cartesian2DMesh/hatOctree", Generator::Cartesian2DMesh, Some("cartesian2DMesh/hatOctree/geom.fms")),
        ("tetMesh/cutCubeOctree", Generator::TetMesh, Some("tetMesh/cutCubeOctree/geom1.stl")),
    ];

    pub(crate) fn tutorial_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/cfmesh_tutorials")
    }

    /// **Pin (gh:#1476).** Every tutorial's `meshDict` passes
    /// `checkMeshDict` and reads with **no unknown keyword**; the entries
    /// the port reads but does not act on are exactly the ones listed
    /// (`optimiseLayer on` in two cases, and the ship's
    /// `optimisationParameters`). Each case's surface (where copied) is read
    /// and every patch pattern matches at least one patch.
    #[test]
    fn tutorials_read_with_no_unhandled_keyword() {
        let root = tutorial_dir();
        let mut table = Vec::new();
        for &(case, generator, surface) in TUTORIALS {
            let dir = root.join(case);
            let d = FoamFile::read(dir.join("system/meshDict")).expect("meshDict parses").dict;
            let w = check_mesh_dict(&d, |f| dir.join(f).is_file() || f.ends_with("ship5415-hull.stl")).unwrap_or_else(|e| panic!("{case}: checkMeshDict: {e}"));
            assert!(w.is_empty(), "{case}: {w:?}");
            let r = read_mesh_dict(&d, generator).unwrap_or_else(|e| panic!("{case}: {e}"));
            assert!(r.unknown().is_empty(), "{case}: unknown {:?}", r.unknown());
            let reported: Vec<&str> = r.keywords.iter().filter(|k| matches!(k.status, KeywordStatus::Reported(_))).map(|k| k.path.as_str()).collect();
            let expected: &[&str] = match case {
                "cartesianMesh/multipleOrifices" => &["boundaryLayers/optimiseLayer"],
                "cartesianMesh/ship5415Octree" => &["boundaryLayers/optimiseLayer", "boundaryLayers/optimisationParameters"],
                _ => &[],
            };
            assert_eq!(reported, expected, "{case}");
            for k in &r.keywords {
                table.push(format!("{case:32} {:16} {:58} {:?}", generator.name(), k.path, k.status));
            }
            let Some(sf) = surface else { continue };
            let s = crate::tri_surf_io::read_surface_bytes(sf, &std::fs::read(root.join(sf)).unwrap()).unwrap().surface;
            let (opts, notes) = r.resolve(&s, |_| Err("no source surfaces in this test".into())).unwrap();
            assert!(notes.is_empty(), "{case}: {notes:?}");
            let n_local = match &opts {
                GeneratorOptions::CartesianMesh(o) => o.dict.local_refinement.len(),
                GeneratorOptions::TetMesh(o) | GeneratorOptions::PMesh(o) => o.dict.local_refinement.len(),
                GeneratorOptions::Cartesian2DMesh(o) => o.dict.local_refinement.len(),
            };
            println!("{case}: {} points, {} triangles, {} patches; {n_local} localRefinement regions", s.points.len(), s.triangles.len(), s.patches.len());
        }
        println!("{}", table.join("\n"));
    }
}
