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

//! Large-Eddy Simulation (LES) sub-grid-scale closures.
//!
//! Holds [`Smagorinsky`], [`wale::Wale`], [`KEqn`], [`DynamicKEqn`] and [`DynamicLagrangian`] (gh:#1377, #1544).
//!
//! ~~Currently holds only the `Smagorinsky` model, which is a **scaffold** (its
//! trait methods `todo!()`-panic).~~ **CORRECTED 2026-11-13:** no `todo!()`
//! remains in `smagorinsky.rs` (grep), the model implements every
//! `TurbulenceModel` method and has unit tests and a code-to-code test
//! (`tests/openfoam_turbulence_code_to_code.rs`).

pub mod deltas;
pub mod dynamic_k_eqn;
pub mod dynamic_lagrangian;
pub mod k_eqn;
pub mod pointwise;
pub mod smagorinsky;
pub mod wale;
pub use dynamic_k_eqn::DynamicKEqn;
pub use dynamic_lagrangian::DynamicLagrangian;
pub use k_eqn::KEqn;
pub use smagorinsky::Smagorinsky;
