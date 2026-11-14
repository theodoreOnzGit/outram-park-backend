// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Domain decomposition after ESI OpenFOAM v2506 `decomposePar` (gh:#1381): the `simple` and
//! `hierarchical` geometric methods ([`geom`]), the processor addressing, processor patches
//! and per-processor `polyMesh` ([`domain`]), and `std::sort` as libstdc++ orders ties
//! ([`std_sort`]). Method and results of the differential against the ESI binary:
//! `verification_and_validation/decompose_par_gh1381/README.md`.
//!
//! Research, education and V&V only.

pub mod domain;
pub mod geom;
pub mod std_sort;

pub use domain::{
    decompose_mesh, decompose_par, esi_cell_centres, write_processor_cases, DecomposeError, Decomposition,
    ProcessorAddressing,
};
pub use geom::{GeomDecomp, GeomMethod};
