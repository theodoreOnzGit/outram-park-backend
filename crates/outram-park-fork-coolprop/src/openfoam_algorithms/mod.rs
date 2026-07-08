//! Vendored pure-Rust OpenFOAM finite-volume layer + the 1-D compressible
//! `rhoPimpleFoam` solver, copied from `tampines-steam-tables` (see this
//! directory's `CLAUDE.md` for provenance). It uses only `uom` — no
//! `openfoam-basic-lib`, `ndarray`, or BLAS — so the crate stays
//! Android-buildable.
//!
//! Only the `rhoPimpleFoam` path (driving [`crate::OPCPFluidArray`]) is kept;
//! the other vendored solvers (`driftFluxFoam`, `simplefoam`,
//! `chtMultiRegionTwoPhaseEulerFoam`) were removed. Parts of the shared
//! `openfoam_source` primitive layer are still as-yet-unused here, so the
//! `dead_code` / `unused_imports` allows below silence that expected noise for
//! the vendored tree only; do not add new dead code under this cover, and prefer
//! wiring a primitive up over leaving it unused (workspace "no orphaned dead
//! code" rule).
#![allow(dead_code)]
#![allow(unused_imports)]

// rhoPimpleFoam: PIMPLE algorithm for compressible flow, driving OPCPFluidArray.
#[allow(non_snake_case)]
pub mod rhoPimpleFoam;

pub(crate) mod openfoam_source;
