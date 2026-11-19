// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see the crate's LICENSE.

//! OpenFOAM's primitive + finite-volume layer, from `outram-foam-basic-lib`.
//!
//! ~~Pure-Rust port of OpenFOAM's primitive + finite-volume layer [...]
//! copied into this crate.~~ **CHANGED 2026-11-16 (gh:#1607, maintainer:
//! "let's have both depend directly on openfoam basic lib, no duplicate
//! code"):** this was a vendored copy of `outram-foam-basic-lib` (84 files,
//! about 15k lines) that had drifted from it in 78 of 84 files. It is now
//! only re-exports of `outram-foam-basic-lib` under the same module names, so
//! every `crate::openfoam_algorithms::openfoam_source::...` path inside this
//! crate still resolves, now to the one maintained implementation. Still
//! crate-internal: none of it is part of `tampines-steam-tables`'s public API.

#![allow(unused_imports)]

pub(crate) use outram_foam_basic_lib::{
    fields, fluid_thermo, fv_operators, interface, interpolation, ldu_matrix, math, matrix, mesh, ode,
    polynomial, primitives, thermophysics,
};

pub(crate) use outram_foam_basic_lib::fields::*;
pub(crate) use outram_foam_basic_lib::fluid_thermo::*;
pub(crate) use outram_foam_basic_lib::fv_operators::*;
pub(crate) use outram_foam_basic_lib::interface::*;
pub(crate) use outram_foam_basic_lib::interpolation::*;
pub(crate) use outram_foam_basic_lib::ldu_matrix::*;
pub(crate) use outram_foam_basic_lib::math::*;
pub(crate) use outram_foam_basic_lib::matrix::*;
pub(crate) use outram_foam_basic_lib::mesh::*;
pub(crate) use outram_foam_basic_lib::ode::*;
pub(crate) use outram_foam_basic_lib::polynomial::*;
pub(crate) use outram_foam_basic_lib::primitives::*;
pub(crate) use outram_foam_basic_lib::thermophysics::*;
