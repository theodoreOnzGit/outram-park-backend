// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::multi_region` — multi-mesh coupling
//!
//! Rust port of `GeN-Foam/src/classes/multiRegion` (~2.4k LOC): the layer that
//! couples the separate neutronics / thermal-hydraulics / thermo-mechanics
//! meshes — mapping fields between them (`meshToMesh` / radial-basis-function
//! interpolation), assembling the coupled feedback (fuel/coolant temperature,
//! density, power density, mesh displacement), and driving the outer coupling
//! iteration.
//!
//! Generic FV building blocks come from [`outram_foam_basic_lib`]; the physics
//! models being coupled live in [`super::neutronics`],
//! [`super::thermo_mechanics`], and the thermal-hydraulics module.
//!
//! ## Module map — read this first
//!
//! | Submodule | Ports (upstream) | Role |
//! |---|---|---|
//! | [`mesh_to_mesh`] | `meshHandler` `meshToMesh` addressing + `map`/`mapTgtToSrc` | Volumetric cell-to-cell field transfer ([`MeshToMesh`]): nearest-cell, inverse-distance, and (approximate) conservative cell-volume-weighted mapping between overlapping region meshes. |
//! | [`rbf_mapping`] | `meshHandler::interpolateAndMapFields` (polyharmonic-spline path) | Radial-basis-function mapping ([`RbfFieldMap`]) for **non-conformal** meshes that do not volume-overlap — fits a polyharmonic spline to scattered samples and evaluates it on the target mesh. |
//! | [`coupling_fields`] | `meshHandler` region registry + `mappings` / `mapAllFields` | The [`MeshHandler`]: region meshes, their named coupling fields, and the pairwise mappings; [`MeshHandler::interpolate_coupling_fields`] is `interpolateCouplingFields`. |
//! | [`outer_iteration`] | `multiPhysicsSolver::correctPhysics` | The tightly-coupled Picard loop ([`MultiPhysicsSolver`]) advancing the regions and exchanging feedback to convergence, enum-dispatched over [`RegionModel`] (no `dyn`). |
//! | [`mesh_region`] | region-solver dispatch → `diffusionNeutronics` / `thermalHydraulics` | The **mesh-based** [`RegionModel`] variants: [`MeshNeutronics`] drives the real multigroup-diffusion solver with cross-section temperature feedback; [`MeshThermalHydraulics`] is the per-cell energy-balance seam for the full TH solver. |
//! | [`reactivity_feedback`] | `pointKineticNeutronics` feedback assembly | The [`ReactivityFeedback`] layer: turns the mesh temperature / density feedback fields into a scalar reactivity `Δρ` (Doppler + expansion + coolant density), generalising the lumped `α(T−T_ref)` to spatial fields. |
//!
//! **Start with an example**, not the API: the loop wired end-to-end (0-D
//! neutronics ↔ lumped thermal-hydraulics, using the already-ported
//! point-kinetics) lives in [`outer_iteration`]'s `tests` — it constructs a
//! [`MeshHandler`], registers two regions and their mappings, and drives
//! [`MultiPhysicsSolver::solve`] over a transient.
//!
//! ## Coupling fields exchanged
//!
//! `TFuel`, `TStruct`, coolant density, `powerDensityNeutronics`, and `meshDisp`
//! — pulled onto the meshes that consume them; see [`coupling_fields`].
//!
//! ## RBF kernel reuse
//!
//! [`rbf_mapping`] reuses the shared polyharmonic-spline kernel at
//! [`crate::genfoam::common::rbf`] rather than adding a second copy — the same
//! kernel also backs cross-section parametrisation in
//! [`crate::genfoam::neutronics::xs`]. See [`rbf_mapping`]'s docs.
//!
//! ## Scaffolded gaps (missing basic-lib mesh machinery)
//!
//! Two pieces are intentionally scaffolded (documented interface + a degraded or
//! deferred implementation) because [`outram_foam_basic_lib`] does not yet expose
//! the geometry they need — tracked as sub-beads of `op-p6p.8`:
//!
//! - **Exact conservative `meshToMesh` (`imCellVolumeWeight`).** Upstream
//!   integrates true polyhedral cell-overlap volumes (a supermesh intersection);
//!   basic-lib has cell centres/volumes but no mesh-intersection/clipping. The
//!   port uses nearest-cell addressing plus a global integral rescale — globally
//!   conservative, but not the exact local overlap distribution. Exact overlap
//!   awaits a basic-lib supermesh operator.
//! - **`deformMesh` / `movePoints`.** Applying `meshDisp` to actually move mesh
//!   points (upstream `deformMesh`) needs mutable mesh-point geometry on
//!   `FvMesh`, which basic-lib does not expose. The loop plumbs the displacement
//!   field through the coupling but does not yet move points.
//!
//! Neither hack was applied to work around these — the mapping is honest about
//! its fidelity and the loop is honest about not moving points.
//!
//! **Port status:** field mapping (volumetric + RBF), coupled-feedback assembly,
//! and the outer Picard loop are ported and verified end-to-end against both the
//! 0-D neutronics ↔ lumped-TH coupling *and* the **mesh-based** path: the
//! [`MeshNeutronics`] variant drives the real multigroup-diffusion solver
//! ([`crate::genfoam::neutronics::DiffusionNeutronics`]) with cross-section
//! (Doppler) temperature feedback through the [`reactivity_feedback`] layer, and
//! [`MeshThermalHydraulics`] closes the loop across non-conformal meshes (V&V:
//! power-density conservation across the `CellVolumeWeight` map and negative
//! Doppler feedback lowering `k_eff` — see [`mesh_region`]'s tests). The full
//! porous/two-phase TH solver (`op-p6p.7`) drops into the same
//! [`MeshThermalHydraulics`] seam when it lands; per-cell (rather than mean)
//! cross-section feedback awaits a neutronics-subtree API addition (tracked on
//! `op-p6p.8.4`). See `docs/genfoam-port-plan.md`.

pub mod coupling_fields;
pub mod mesh_region;
pub mod mesh_to_mesh;
pub mod outer_iteration;
pub mod rbf_mapping;
pub mod reactivity_feedback;

// Curated re-exports (human interface layer): the handful of types a user needs
// to build a coupled multi-region run without hunting through submodules.
pub use coupling_fields::{CouplingError, CouplingField, CouplingRegion, FieldKind, MeshHandler};
pub use mesh_region::{MeshNeutronics, MeshThermalHydraulics};
pub use mesh_to_mesh::{MapCombine, MappingMethod, MeshToMesh};
pub use outer_iteration::{
    lumped_cell_mesh, CouplingLoopError, FeedbackCoefficient, LumpedNeutronics, LumpedThermal,
    MultiPhysicsSolver, PrescribedRegion, RegionKernel, RegionModel,
};
pub use rbf_mapping::{PolyharmonicMode, RbfFieldMap, RbfMapError};
pub use reactivity_feedback::{DopplerLaw, FeedbackTerm, ImportanceWeight, ReactivityFeedback};
