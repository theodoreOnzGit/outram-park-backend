//! # outram-park-fork-cfmesh
//!
//! A pure-Rust **fork / port of [cfMesh](https://github.com/wyldckat/cfMesh)**
//! (Creative Fields' automatic unstructured mesh generator, **GPL-3.0**) for the
//! OUTRAM PARK multiphysics suite — the *volume-mesh-generation* layer that sits
//! between the [`outram-blender`](https://crates.io/crates/outram-blender)
//! surface-authoring frontend and the OpenFOAM-style solvers.
//!
//! ```text
//!   outram-blender  ──►  outram-park-fork-cfmesh  ──►  outram-foam-basic-lib
//!   (surface Mesh)       (tet / polyhedral cells        (PolyMesh → FvMesh,
//!                         + boundary layers)              solvable)
//! ```
//!
//! ## Why this crate exists
//!
//! The workspace already has the mesh **representation** and finite-volume
//! addressing (`outram-foam-basic-lib`'s `FvMesh` / `io::poly_mesh::PolyMesh`,
//! with `read`/`write`/`to_fv_mesh`), and the surface-**authoring** frontend
//! (`outram-blender`). What was missing everywhere is unstructured mesh
//! **generation** — turning a closed surface into a solvable *volume* mesh with
//! polyhedral cells and near-wall boundary layers. Open-source, pure-Rust,
//! GPLv3-clean tooling for this is genuinely lacking, so this crate ports the
//! proven cfMesh workflows rather than reinventing them.
//!
//! ## Goal
//!
//! Consume an `outram-blender` [`Mesh`](https://docs.rs/outram-blender) (a closed
//! watertight surface), generate a **tetrahedral** then **polyhedral** volume
//! mesh (à la cfMesh `tetMesh` / `cartesianMesh` + OpenFOAM `polyDualMesh`), with
//! optional **wall boundary/prism layers**, and emit a real-cell
//! `outram_foam_basic_lib::io::poly_mesh::PolyMesh` for the CFD/TH solvers and
//! `outram-mc-libs` geometry for neutronics — the mesh substrate for coupled
//! pebble-bed / molten-salt / light-water reactor simulations.
//!
//! ## Vendored upstreams (reference only, GPLv3-clean, never shipped)
//!
//! Both live under `upstream_source/` (gitignored, dev-only — see
//! `upstream_source/README.md`):
//!
//! - **cfMesh** — <https://github.com/wyldckat/cfMesh>, **GPL-3.0-only**. Primary
//!   port target: `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian
//!   hex-dominant + tet meshing, surface tools, boundary-layer insertion).
//! - **voro++** — <https://github.com/chr1shr/voro>, modified-BSD (LBNL),
//!   GPLv3-compatible. Reference for the Voronoi / polyhedral-dual construction.
//!
//! Ported files carry the upstream provenance header block (project, source
//! file, commit, copyright, licence) per the workspace provenance rule; the
//! algorithms are re-implemented in Rust, not transcribed verbatim from C++.
//!
//! ## Status
//!
//! **Milestone 1 — the volume-mesh core + a Cartesian block mesher.** The
//! [`volume_mesh::VolumeMesh`] data structure (points + faces + owner/neighbour
//! + patches, mirroring cfMesh's `polyMeshGen` and OpenFOAM's `polyMesh`) is in
//! place, and [`cartesian::cartesian_box`] fills an axis-aligned box with a
//! regular hex grid — the un-refined background the cfMesh `cartesianMesh`
//! workflow builds on, and a complete valid `VolumeMesh` in its own right
//! (verified: exact volume, closed cells, outward boundary normals).
//!
//! The remaining roadmap lives in beads under the `op-hzs` epic: octree
//! refinement + surface carving of this Cartesian base, the polyhedral dual
//! (`op-hzs.33`, voro++ reference), boundary layers (`op-hzs.34`), and the
//! volume-`PolyMesh` bridge to `outram-foam-basic-lib` (`op-hzs.35`).
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Index-based topology (newtype indices into `Vec`, no lifetimes/pointers);
//! enums for dispatch, never trait objects; no `Box<T>` (own by value or share
//! with `Arc<T>`); pure Rust with no BLAS/C/Fortran so the crate builds on
//! Android/Termux.
//!
//! > **Not affiliated with Creative Fields or the cfMesh project**, and not the
//! > official cfMesh software — an independent GPL fork. **Untrusted
//! > AI-assisted draft** until human-reviewed, per the workspace
//! > `RESPONSIBLE_USE.md`. For education / research / V&V only; not for reactor
//! > operation, licensing, or safety-critical decisions.

pub mod cartesian;
pub mod math;
pub mod volume_mesh;
