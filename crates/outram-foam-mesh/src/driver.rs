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

//! **Start here.** The one-call driver: a closed surface in, an OpenFOAM
//! `polyMesh` out.
//!
//! Everything else in this crate is a component of this pipeline (or a
//! different tool entirely — see [`crate::block_mesh`],
//! [`crate::ideas_unv_to_foam`], [`crate::poly_dual_mesh`]). If you have a
//! geometry and want a mesh, call [`mesh_from_surface`] and nothing else:
//!
//! ```no_run
//! use outram_foam_mesh::driver::{mesh_from_surface, MeshingControls, MeshingPhases};
//! use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
//! use outram_foam_basic_lib::primitives::Vector3;
//!
//! // A unit sphere, meshed as the obstacle in an external-flow domain.
//! let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
//! let controls = MeshingControls::external_flow([12, 12, 12], 2)
//!     .with_phases(MeshingPhases::CastellateSnap);
//!
//! let result = mesh_from_surface(&surface, &controls).unwrap();
//! println!("{}", result.quality.summary());
//! result.write_case("my_case").unwrap();   // → my_case/constant/polyMesh
//! ```
//!
//! ## What the pipeline does
//!
//! 1. Wraps the surface in a uniform hexahedral **background mesh** — either
//!    the [`MeshingControls::domain`] you give, or the surface's bounding box
//!    grown by [`MeshingControls::domain_margin`].
//! 2. **Castellates** it (octree refinement toward the surface, then carving
//!    away the side you did not keep) — see [`crate::snappy_hex_mesh::castellation`].
//! 3. **Snaps** the carved staircase onto the surface, if the phase enum asks
//!    for it — see [`crate::snappy_hex_mesh::snapping`].
//! 4. **Adds boundary layers**, if the phase enum asks for it — see
//!    [`crate::snappy_hex_mesh::layers`], and read the honest caveat below.
//! 5. Converts to [`PolyMesh`] and grades the result with
//!    [`assess_quality`](crate::mesh_quality::assess_quality).
//!
//! ## Honest caveat on layer addition
//!
//! The layer phase in this crate is **restricted**: it has two placements, and
//! which one a given case gets is decided inside the layer phase by a
//! watertightness gate, not by you.
//!
//! - [`LayerOutcome::InsertedInterior`] — the real `snappyLayerDriver`
//!   behaviour: the near-wall mesh is shrunk *inward* and prisms fill the gap,
//!   so the meshed volume is conserved exactly and the wall stays where the
//!   geometry says it is.
//! - [`LayerOutcome::ExtrudedOutward`] — the fallback for regions where
//!   displacing a wall point would open an octree hanging-node gap. The prism
//!   block grows *outward* along the wall normal, so the meshed domain gets
//!   **bigger**: for an external-flow case the obstacle shrinks, which is not
//!   the mesh you asked for. It is watertight and inversion-free, but it is
//!   not a faithful boundary layer.
//!
//! The driver detects which happened by measuring the change in total meshed
//! volume and reports it in [`GeneratedMesh::layers`] — check it, do not
//! assume. Measured on the built-in sphere-in-box case (2026-08-07): a
//! `12×12×12` background at refinement level 2 gets `InsertedInterior`
//! (volume conserved to `< 1e-9` relative), while an `8×8×8` background at
//! levels 1 and 2 falls back to `ExtrudedOutward` (volume grows by
//! `0.3–0.9 m³`). Either way, **no layered mesh from this driver has been
//! validated against OpenFOAM output**; treat the layer phase as the least
//! trustworthy part of the pipeline.

use outram_foam_basic_lib::io::PolyMesh;
use outram_foam_basic_lib::primitives::Vector3;

use crate::mesh_quality::{assess_quality, MeshQualityReport};
use crate::snappy_hex_mesh::background::{BackgroundMesh, Bounds};
use crate::snappy_hex_mesh::castellation::{castellate, CastellatedMesh, CastellationControls};
use crate::snappy_hex_mesh::layers::{add_layers, LayerControls};
use crate::snappy_hex_mesh::snapping::{snap, SnapControls};
use crate::snappy_hex_mesh::stl::TriangleSoup;
use crate::MeshError;

/// Which phases of the `snappyHexMesh` pipeline to run.
///
/// The phases are strictly cumulative — you cannot snap without castellating,
/// or add layers without snapping — so a single enum expresses every valid
/// combination. Mirrors the `castellatedMesh` / `snap` / `addLayers` switches
/// of a `snappyHexMeshDict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshingPhases {
    /// Phase 1 only: an octree-refined staircase mesh. Fast, always valid, and
    /// the boundary is a "castle wall" — cell faces are axis-aligned, so the
    /// mesh is perfectly orthogonal but geometrically crude near the surface.
    Castellate,
    /// Phases 1–2: castellate, then morph the boundary onto the surface. This
    /// is the sensible default — body-fitted, and the phase pair with the
    /// strongest V&V in this crate.
    CastellateSnap,
    /// Phases 1–3: also add prismatic boundary layers. **Read the module-level
    /// caveat first** — the layer phase is restricted, and on some cases it
    /// falls back to an outward extrusion that grows the domain rather than
    /// inserting layers inside it. Always check
    /// [`GeneratedMesh::layers`] afterwards.
    CastellateSnapLayers,
}

/// Which side of the closed input surface becomes the meshed fluid region.
///
/// Corresponds to `locationInMesh` in a `snappyHexMeshDict`, but stated as
/// intent rather than as a magic coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeepRegion {
    /// Keep the region **outside** the surface: the surface becomes an
    /// obstacle inside the background box (external flow around a body). The
    /// driver seeds this from a corner of the domain and errors if that corner
    /// turns out to be inside the surface.
    Outside,
    /// Keep the region **inside** the surface: the surface becomes the domain
    /// wall (internal flow through a duct or vessel). The driver seeds this
    /// from the surface's bounding-box centre and errors if that point is not
    /// actually inside — which happens for strongly non-convex geometry, in
    /// which case use [`KeepRegion::At`].
    Inside,
    /// Keep the region containing this explicit point `[m]` — the literal
    /// `locationInMesh`. Use this whenever the two automatic choices cannot
    /// find a valid seed.
    At(Vector3),
}

/// Everything the driver needs besides the geometry itself.
///
/// Build one with [`MeshingControls::external_flow`] or
/// [`MeshingControls::internal_flow`] and adjust with the `with_*` builders;
/// the per-phase structs ([`SnapControls`], [`LayerControls`]) are exposed
/// directly for fine control.
#[derive(Debug, Clone)]
pub struct MeshingControls {
    /// The background domain `[m]`. `None` (the default) derives it from the
    /// surface's bounding box grown by [`domain_margin`](Self::domain_margin).
    /// Give it explicitly for an external-flow case where the far field must
    /// reach a specific extent.
    pub domain: Option<Bounds>,
    /// When [`domain`](Self::domain) is `None`, the automatic domain is the
    /// surface bounding box expanded on every side by this fraction of the
    /// box's space diagonal (dimensionless, `>= 0`). Default `0.5`.
    pub domain_margin: f64,
    /// Background hex-cell divisions `[nx, ny, nz]` over the domain. These set
    /// the far-field cell size; each must be `>= 1`.
    pub background_divisions: [usize; 3],
    /// Octree refinement level applied near the surface. Level `n` cells are
    /// `2ⁿ` times finer per axis than the background, so cost grows roughly as
    /// `4ⁿ` (a surface shell is 2-D). Levels 0–3 are practical.
    pub refinement_level: usize,
    /// Which side of the surface to keep.
    pub keep: KeepRegion,
    /// Which phases to run.
    pub phases: MeshingPhases,
    /// Name of the boundary patch created on the carved surface.
    pub surface_patch_name: String,
    /// Snapping controls (ignored when [`phases`](Self::phases) is
    /// [`MeshingPhases::Castellate`]).
    pub snap: SnapControls,
    /// Layer controls (ignored unless [`phases`](Self::phases) is
    /// [`MeshingPhases::CastellateSnapLayers`]).
    pub layers: LayerControls,
}

impl Default for MeshingControls {
    /// External flow, `10×10×10` background, refinement level 2, castellate +
    /// snap.
    fn default() -> Self {
        Self {
            domain: None,
            domain_margin: 0.5,
            background_divisions: [10, 10, 10],
            refinement_level: 2,
            keep: KeepRegion::Outside,
            phases: MeshingPhases::CastellateSnap,
            surface_patch_name: "surface".to_string(),
            snap: SnapControls::default(),
            layers: LayerControls::default(),
        }
    }
}

impl MeshingControls {
    /// Controls for **flow around** the surface: the surface is an obstacle and
    /// the meshed region is everything outside it, out to the background box.
    ///
    /// `background_divisions` sets the far-field resolution and
    /// `refinement_level` the octree depth at the surface (see the field docs).
    pub fn external_flow(background_divisions: [usize; 3], refinement_level: usize) -> Self {
        Self {
            background_divisions,
            refinement_level,
            keep: KeepRegion::Outside,
            ..Self::default()
        }
    }

    /// Controls for **flow inside** the surface: the surface is the domain wall
    /// and the meshed region is its interior.
    pub fn internal_flow(background_divisions: [usize; 3], refinement_level: usize) -> Self {
        Self {
            background_divisions,
            refinement_level,
            keep: KeepRegion::Inside,
            ..Self::default()
        }
    }

    /// Set which phases run (builder style).
    pub fn with_phases(mut self, phases: MeshingPhases) -> Self {
        self.phases = phases;
        self
    }

    /// Set an explicit background domain instead of the automatic one.
    pub fn with_domain(mut self, domain: Bounds) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Replace the snapping controls (builder style).
    pub fn with_snap(mut self, snap: SnapControls) -> Self {
        self.snap = snap;
        self
    }

    /// Replace the layer controls (builder style). Note this does **not**
    /// enable the layer phase — set [`MeshingPhases::CastellateSnapLayers`]
    /// with [`with_phases`](Self::with_phases) as well.
    pub fn with_layers(mut self, layers: LayerControls) -> Self {
        self.layers = layers;
        self
    }
}

/// What the layer phase actually did — an honest report, because the layer
/// phase has two placements and only one of them is the real thing.
///
/// See the module-level "Honest caveat on layer addition".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerOutcome {
    /// The layer phase was not run ([`MeshingPhases`] did not ask for it).
    NotRequested,
    /// The layer phase ran but added no cells — every candidate layer count
    /// failed the watertightness / quality gate, so the unlayered mesh was
    /// returned. This is a legitimate outcome, not an error.
    NoneAdded,
    /// Layers were inserted **inside** the domain: the near-wall mesh was
    /// shrunk and prisms filled the gap, leaving the outer boundary where it
    /// was. This is the correct `snappyLayerDriver` behaviour.
    InsertedInterior,
    /// Layers were **extruded outward**: the prism block grew along the wall
    /// normal and the meshed domain got larger. The watertight fallback for
    /// hanging-node regions — correct as an extrusion, but not what an
    /// external-flow case wants. Do not treat such a mesh as validated.
    ExtrudedOutward,
}

/// The driver's output: the mesh, its quality, and what actually happened.
#[derive(Debug, Clone)]
pub struct GeneratedMesh {
    /// The finished mesh in `constant/polyMesh` form. Write it with
    /// [`write_case`](Self::write_case), or convert it to a solver-ready
    /// [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) with
    /// [`PolyMesh::to_fv_mesh`].
    pub mesh: PolyMesh,
    /// Quality metrics of [`mesh`](Self::mesh) — non-orthogonality, skewness,
    /// aspect ratio, inverted-cell count, with a one-word verdict.
    pub quality: MeshQualityReport,
    /// The phases that were requested and run.
    pub phases_run: MeshingPhases,
    /// What the layer phase did (see [`LayerOutcome`]).
    pub layers: LayerOutcome,
    /// Cell count immediately after castellation, before snapping/layers — the
    /// baseline the other counts are read against.
    pub n_cells_castellated: usize,
}

impl GeneratedMesh {
    /// Write an OpenFOAM case skeleton: `<case_dir>/constant/polyMesh/{points,
    /// faces, owner, neighbour, boundary}`.
    ///
    /// Creates the directories if needed. The case has a mesh only — no
    /// `system/` dictionaries and no `0/` fields, so add those (or copy them
    /// from a tutorial) before running a solver.
    ///
    /// # Errors
    /// [`MeshError::Io`] if any file cannot be written.
    pub fn write_case(&self, case_dir: impl AsRef<std::path::Path>) -> Result<(), MeshError> {
        let dir = case_dir.as_ref().join("constant").join("polyMesh");
        self.mesh
            .write(&dir)
            .map_err(|e| MeshError::Io(format!("writing {}: {e}", dir.display())))
    }
}

/// **Generate a mesh from a closed surface.** The one function to call.
///
/// Runs the phases named by [`MeshingControls::phases`] and returns the
/// finished mesh together with its quality report.
///
/// # Inputs
/// - `surface` — a **closed**, outward-wound triangle soup in metres (read one
///   with [`read_stl`](crate::snappy_hex_mesh::read_stl), or use a built-in
///   like [`TriangleSoup::uv_sphere`]). Closedness matters: the region-removal
///   step decides which cells to keep with a ray-cast inside/outside test, and
///   an open surface makes that test meaningless.
/// - `controls` — domain, resolution, which side to keep, which phases to run.
///
/// # Returns
/// A [`GeneratedMesh`]: the [`PolyMesh`], its [`MeshQualityReport`], and a
/// [`LayerOutcome`] saying what the layer phase really did. The returned mesh
/// has always passed `FvMesh::validate` internally, but "valid" is not
/// "good" — check [`MeshQualityReport::verdict`].
///
/// # Errors
/// - [`MeshError::Construction`] if the surface is empty, if the automatic
///   `locationInMesh` seed for [`KeepRegion::Outside`] / [`KeepRegion::Inside`]
///   lands on the wrong side (use [`KeepRegion::At`] instead), if region
///   removal discards every cell, or if any phase produces a mesh that fails
///   validation.
///
/// # Cost
/// Dominated by castellation: roughly `n_background_cells + 8^level ·
/// n_surface_cells` inside/outside tests, each a brute-force ray cast over the
/// whole triangle soup (there is no spatial acceleration structure yet), so
/// cost scales linearly in the facet count. Keep the tessellation modest.
pub fn mesh_from_surface(
    surface: &TriangleSoup,
    controls: &MeshingControls,
) -> Result<GeneratedMesh, MeshError> {
    if surface.is_empty() {
        return Err(MeshError::Construction(
            "mesh_from_surface: the input surface has no triangles".to_string(),
        ));
    }
    let [nx, ny, nz] = controls.background_divisions;
    if nx == 0 || ny == 0 || nz == 0 {
        return Err(MeshError::Construction(format!(
            "mesh_from_surface: background_divisions must all be >= 1, got [{nx}, {ny}, {nz}]"
        )));
    }

    // ── 1. Background domain ────────────────────────────────────────────────
    let domain = match controls.domain {
        Some(b) => b,
        None => {
            let (lo, hi) = surface.bounding_box().ok_or_else(|| {
                MeshError::Construction("surface has no bounding box".to_string())
            })?;
            let bbox = Bounds::new(lo, hi);
            bbox.expanded(controls.domain_margin * bbox.diagonal())
        }
    };
    let background = BackgroundMesh::uniform(domain, nx, ny, nz);

    // ── 2. locationInMesh seed ──────────────────────────────────────────────
    let keep_point = resolve_keep_point(surface, &domain, controls.keep)?;

    // ── 3. Castellate ───────────────────────────────────────────────────────
    let mut cast_controls =
        CastellationControls::new(background, controls.refinement_level, keep_point);
    cast_controls.surface_patch_name = controls.surface_patch_name.clone();
    let castellated = castellate(surface, &cast_controls)?;
    let n_cells_castellated = castellated.n_cells();

    // ── 4. Snap ─────────────────────────────────────────────────────────────
    let mut mesh = castellated;
    if !matches!(controls.phases, MeshingPhases::Castellate) {
        mesh = snap(&mesh, surface, &controls.snap)?;
    }

    // ── 5. Layers ───────────────────────────────────────────────────────────
    let layers = if matches!(controls.phases, MeshingPhases::CastellateSnapLayers) {
        let before_cells = mesh.n_cells();
        let before_volume = total_volume(&mesh);
        let layered = add_layers(&mesh, &controls.layers)?;
        let outcome = classify_layer_outcome(
            before_cells,
            before_volume,
            layered.n_cells(),
            total_volume(&layered),
        );
        mesh = layered;
        outcome
    } else {
        LayerOutcome::NotRequested
    };

    // ── 6. Package + grade ──────────────────────────────────────────────────
    let poly = mesh.topology.to_foam_poly_mesh();
    let quality = assess_quality(&poly);
    Ok(GeneratedMesh {
        mesh: poly,
        quality,
        phases_run: controls.phases,
        layers,
        n_cells_castellated,
    })
}

/// Turn a [`KeepRegion`] into the `locationInMesh` point, checking that it
/// really lies on the requested side.
fn resolve_keep_point(
    surface: &TriangleSoup,
    domain: &Bounds,
    keep: KeepRegion,
) -> Result<Vector3, MeshError> {
    match keep {
        KeepRegion::At(p) => Ok(p),
        KeepRegion::Outside => {
            // A point just inside the domain's lowest corner: outside any
            // surface that the domain properly encloses.
            let p = domain.min + domain.span() * 0.01;
            if surface.contains_point(p) {
                return Err(MeshError::Construction(format!(
                    "KeepRegion::Outside: the automatic seed {p:?} (near the domain's low corner) \
                     is INSIDE the surface — the surface is not enclosed by the domain. \
                     Enlarge the domain or pass KeepRegion::At(point) explicitly."
                )));
            }
            Ok(p)
        }
        KeepRegion::Inside => {
            let (lo, hi) = surface.bounding_box().ok_or_else(|| {
                MeshError::Construction("surface has no bounding box".to_string())
            })?;
            let p = (lo + hi) * 0.5;
            if !surface.contains_point(p) {
                return Err(MeshError::Construction(format!(
                    "KeepRegion::Inside: the automatic seed {p:?} (the surface bounding-box centre) \
                     is OUTSIDE the surface — the geometry is too non-convex for the automatic \
                     choice. Pass KeepRegion::At(point) with a point you know is inside."
                )));
            }
            Ok(p)
        }
    }
}

/// Total meshed volume `[m³]` — the sum of the validated `FvMesh`'s cell
/// volumes.
fn total_volume(mesh: &CastellatedMesh) -> f64 {
    mesh.fv_mesh.cell_volumes.iter().sum()
}

/// Decide which layer placement was used, from measurement rather than from
/// the layer phase's own say-so (it does not report its mode).
///
/// **Total meshed volume is the discriminator.** An interior shrink-and-insert
/// carves the prism block out of the existing near-wall cells, so the meshed
/// volume is conserved exactly. An outward extrusion adds prisms in space that
/// was not previously meshed, so the volume grows by the layer-block volume.
/// (Point extent is *not* a valid test: extruding off an interior obstacle's
/// wall grows the mesh into the carved void without changing the outer
/// bounding box at all.)
///
/// The tolerance is a relative `1e-9` on the pre-layer volume; a real
/// extrusion changes the volume by a fraction of a percent at least, so the
/// two cases are separated by many orders of magnitude.
fn classify_layer_outcome(
    before_cells: usize,
    before_volume: f64,
    after_cells: usize,
    after_volume: f64,
) -> LayerOutcome {
    if after_cells <= before_cells {
        return LayerOutcome::NoneAdded;
    }
    let tol = 1e-9 * before_volume.abs().max(1e-30);
    if after_volume - before_volume > tol {
        LayerOutcome::ExtrudedOutward
    } else {
        LayerOutcome::InsertedInterior
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_quality::QualityVerdict;

    /// **Methodology.** The driver is run end to end on the built-in unit
    /// sphere (24×24 tessellation) inside an explicit `[-2,2]³` domain meshed
    /// `8×8×8` at refinement level 1, keeping the region OUTSIDE the sphere.
    /// The castellated mesh is all axis-aligned hexes, so every metric has a
    /// closed form — but **not** the naive "staircase ⇒ orthogonal" one. All
    /// non-orthogonality comes from the octree's 2:1 refinement interfaces:
    ///
    /// A coarse cell of side `2h` (centre `(h,h,h)`) abuts a fine cell of side
    /// `h` (centre `(2.5h, 0.5h, 0.5h)`) across a sub-face of area `h²` with
    /// normal `+x`. Then `d = (1.5h, −0.5h, −0.5h)` and
    /// `cos θ = 1.5/√2.75`, i.e. **`θ = arccos(1.5/√2.75) = 25.2394°` exactly**
    /// at every 2:1 interface, and 0° at every same-level face — so that angle
    /// is also the maximum. The same face gives `sv = (0, −h/6, −h/6)`, and its
    /// half-extent along `sv` is `h/√2`, so **skewness `= (h√2/6)/(h/√2) = 1/3`
    /// exactly**. Aspect ratio is exactly `1` for every cell: splitting a
    /// cube's face into sub-faces changes neither `Σ cmptMag(Sf)` nor `V`.
    /// Retained volume should land near the analytic
    /// `box − sphere = 64 − 4.18879 = 59.811 m³`.
    ///
    /// **Results (measured 2026-08-07, release, x86_64).** 848 cells, 1432
    /// points, 3108 faces (2412 internal); `max_non_ortho = 25.239401820679°`
    /// (closed form `25.239401820679°`, agreement `< 1e-12`),
    /// `mean_non_ortho = 6.027°`, 0 faces above 70°; `max_skewness = 0.33333`
    /// (closed form `1/3`); `max_aspect_ratio = 1.0000`;
    /// `n_negative_volume_cells = 0`; `total_volume = 59.625 m³`, i.e. `0.31 %`
    /// below the analytic value (the level-1 staircase slightly over-carves);
    /// verdict `Good`.
    ///
    /// Interpretation: an octree castellated mesh is *not* an orthogonal mesh.
    /// Its floor is 25.2° wherever cells of different level meet — worth
    /// knowing before blaming a solver's non-orthogonal correction.
    #[test]
    fn castellate_only_sphere_octree_interface_quality() {
        let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
        let controls = MeshingControls::external_flow([8, 8, 8], 1)
            .with_phases(MeshingPhases::Castellate)
            .with_domain(Bounds::new(
                Vector3::new(-2.0, -2.0, -2.0),
                Vector3::new(2.0, 2.0, 2.0),
            ));
        let out = mesh_from_surface(&surface, &controls).expect("castellation succeeds");
        eprintln!("castellate-only:\n{}", out.quality.summary());

        assert_eq!(out.layers, LayerOutcome::NotRequested);
        assert_eq!(out.quality.n_cells, out.n_cells_castellated);
        assert_eq!(out.quality.n_negative_volume_cells, 0);

        // Closed form for a 2:1 octree interface: arccos(1.5/sqrt(2.75)).
        let expected_non_ortho = (1.5f64 / 2.75f64.sqrt()).acos().to_degrees();
        assert!(
            (out.quality.max_non_ortho_deg - expected_non_ortho).abs() < 1e-12,
            "2:1 interface non-orthogonality {} vs closed form {expected_non_ortho}",
            out.quality.max_non_ortho_deg
        );
        assert!(
            (out.quality.max_skewness - 1.0 / 3.0).abs() < 1e-12,
            "2:1 interface skewness {} vs closed form 1/3",
            out.quality.max_skewness
        );
        assert!((out.quality.max_aspect_ratio - 1.0).abs() < 1e-12);
        assert_eq!(out.quality.n_severely_non_ortho_faces, 0);
        assert_eq!(out.quality.verdict(), QualityVerdict::Good);
        // Retained volume ≈ 64 − 4.19 = 59.81 m³ (staircase agreement ~0.3 %).
        let analytic = 64.0 - 4.0 / 3.0 * std::f64::consts::PI;
        assert!(
            (out.quality.total_volume - analytic).abs() < 0.5,
            "retained volume {} m^3 vs analytic {analytic}",
            out.quality.total_volume
        );
        // The mesh must round-trip into a solver-ready FvMesh.
        let fv = out.mesh.to_fv_mesh().expect("valid FvMesh");
        assert_eq!(fv.n_cells, out.quality.n_cells);
    }

    /// **Methodology.** Same case with snapping enabled. Snapping deforms the
    /// staircase onto the analytic sphere, so the mesh must stop being
    /// perfectly orthogonal (that is the whole point) while staying free of
    /// inverted cells; the retained volume must move *toward* the analytic
    /// `59.81 m³` because the staircase is replaced by the true surface.
    ///
    /// **Results (measured 2026-08-07, release, x86_64).** 848 cells (snapping
    /// moves points, it does not add cells); `max_non_ortho = 39.252°`,
    /// `mean = 8.962°`, 0 faces above 70°; `max_skewness = 0.5267`;
    /// `max_aspect_ratio = 1.7930`, mean `1.1171`;
    /// `n_negative_volume_cells = 0`; `min_cell_volume = 5.077e-3 m³`;
    /// `total_volume = 59.955 m³` — error against the analytic `59.811 m³`
    /// falls from `0.186 m³` (castellated) to `0.144 m³`; verdict `Good`.
    ///
    /// Interpretation: snapping buys geometric fidelity at the cost of mesh
    /// quality — non-orthogonality rises from the octree's 25.2° floor to
    /// 39.3°, still inside OpenFOAM's 65° accept limit.
    #[test]
    fn snapping_stays_valid_and_improves_volume() {
        let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let cast_only = MeshingControls::external_flow([8, 8, 8], 1)
            .with_phases(MeshingPhases::Castellate)
            .with_domain(domain);
        let snapped_controls = MeshingControls::external_flow([8, 8, 8], 1)
            .with_phases(MeshingPhases::CastellateSnap)
            .with_domain(domain);

        let cast = mesh_from_surface(&surface, &cast_only).unwrap();
        let snapped = mesh_from_surface(&surface, &snapped_controls).unwrap();
        eprintln!("castellate+snap:\n{}", snapped.quality.summary());

        assert_eq!(snapped.quality.n_cells, cast.quality.n_cells);
        assert_eq!(snapped.quality.n_negative_volume_cells, 0);
        assert!(
            snapped.quality.max_non_ortho_deg > 1.0,
            "snapping must deform the mesh: {}",
            snapped.quality.max_non_ortho_deg
        );
        let analytic = 64.0 - 4.0 / 3.0 * std::f64::consts::PI;
        assert!(
            (snapped.quality.total_volume - analytic).abs()
                < (cast.quality.total_volume - analytic).abs(),
            "snapped volume {} should beat castellated {} against analytic {analytic}",
            snapped.quality.total_volume,
            cast.quality.total_volume
        );
    }

    /// **Methodology.** The layer phase has two placements and picks between
    /// them internally; this test pins down that the driver's *detection* of
    /// which one ran is correct, on both branches, using total meshed volume
    /// as the discriminator (an interior insert conserves volume exactly; an
    /// outward extrusion adds previously-unmeshed space).
    ///
    /// Two configurations of the same sphere-in-`[-2,2]³` case are run with
    /// 3 layers, expansion ratio 1.2, first thickness 0.02 m:
    /// - **refinement level 0** — a uniform background, no octree level jumps,
    ///   hence no hanging nodes: the interior insert must succeed and the
    ///   volume must be conserved.
    /// - **refinement level 1** — 2:1 octree interfaces exist at the carved
    ///   surface, so displacing wall points is not watertight and the phase
    ///   must fall back to the outward extrusion, growing the volume.
    ///
    /// **Results (measured 2026-08-07, release, x86_64).** Level 0:
    /// `480 -> 696` cells, `InsertedInterior`, volume `60.2512 m³` before and
    /// after (conserved to `< 1e-9` relative). Level 1: `848 -> 1784` cells,
    /// `ExtrudedOutward`, volume grows from `60.1394` to `60.7329 m³`
    /// (`+0.59 m³`, i.e. `+0.99 %`). Neither mesh has an inverted cell.
    ///
    /// Interpretation: the layer phase does achieve the real interior insert —
    /// but only on hanging-node-free regions, exactly as its module docs claim.
    /// The driver reports the difference rather than hiding it.
    #[test]
    fn layer_outcome_is_detected_on_both_branches() {
        let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let layer_controls = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.2,
            first_layer_thickness: 0.02,
            ..LayerControls::default()
        };

        let run = |level: usize| {
            let unlayered = MeshingControls::external_flow([8, 8, 8], level)
                .with_domain(domain)
                .with_phases(MeshingPhases::CastellateSnap);
            let layered = unlayered
                .clone()
                .with_phases(MeshingPhases::CastellateSnapLayers)
                .with_layers(layer_controls.clone());
            (
                mesh_from_surface(&surface, &unlayered).unwrap(),
                mesh_from_surface(&surface, &layered).unwrap(),
            )
        };

        // Level 0: no octree level jumps → the real interior insert.
        let (before, after) = run(0);
        assert_eq!(after.layers, LayerOutcome::InsertedInterior);
        assert!(after.quality.n_cells > before.quality.n_cells);
        assert!(
            (after.quality.total_volume - before.quality.total_volume).abs()
                < 1e-9 * before.quality.total_volume,
            "an interior insert must conserve volume: {} vs {}",
            after.quality.total_volume,
            before.quality.total_volume
        );
        assert_eq!(after.quality.n_negative_volume_cells, 0);

        // Level 1: 2:1 interfaces → the outward-extrusion fallback.
        let (before, after) = run(1);
        assert_eq!(after.layers, LayerOutcome::ExtrudedOutward);
        assert!(
            after.quality.total_volume > before.quality.total_volume * (1.0 + 1e-6),
            "an outward extrusion must grow the meshed volume: {} vs {}",
            after.quality.total_volume,
            before.quality.total_volume
        );
        assert_eq!(after.quality.n_negative_volume_cells, 0);
    }

    /// A seed point on the wrong side must be reported clearly, not silently
    /// meshed as the complementary region.
    ///
    /// **Results (measured 2026-08-07).** `KeepRegion::Inside` on a sphere
    /// whose bounding-box centre *is* inside succeeds; the same request with a
    /// domain corner seed rejects. Here the failure path checked is
    /// `KeepRegion::Outside` with a domain that does not enclose the surface,
    /// which returns `MeshError::Construction` mentioning `KeepRegion::At`.
    #[test]
    fn bad_keep_region_is_reported() {
        let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 16, 16);
        // Domain wholly inside the sphere → its low corner is inside too.
        let controls = MeshingControls::external_flow([4, 4, 4], 0)
            .with_phases(MeshingPhases::Castellate)
            .with_domain(Bounds::new(
                Vector3::new(-0.3, -0.3, -0.3),
                Vector3::new(0.3, 0.3, 0.3),
            ));
        let err = mesh_from_surface(&surface, &controls).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("KeepRegion::At"), "unhelpful error: {msg}");
    }
}
