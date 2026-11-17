// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Domain decomposition after ESI OpenFOAM v2506 `decomposePar` (gh:#1381): the `simple` and
//! `hierarchical` geometric methods ([`geom`]), the processor addressing, processor patches
//! and per-processor `polyMesh` ([`domain`]), and `std::sort` as libstdc++ orders ties
//! ([`std_sort`]). Method and results of the differential against the ESI binary:
//! `verification_and_validation/decompose_par_gh1381/README.md`.
//!
//! Research, education and V&V only.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod domain;
pub mod geom;
pub mod std_sort;

pub use domain::{
    decompose_mesh, decompose_par, esi_cell_centres, DecomposeError, Decomposition, ProcessorAddressing,
};
#[cfg(feature = "std")]
pub use domain::write_processor_cases;
pub use geom::{GeomDecomp, GeomMethod};
