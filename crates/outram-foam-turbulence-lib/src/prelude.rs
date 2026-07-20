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

//! Convenience re-exports: `use outram_foam_turbulence_lib::prelude::*;` brings
//! the trait, the error type, every model struct, and the wall-function helpers
//! into scope. Note that only [`KOmegaSST`] is a working model — the other
//! structs are scaffolds whose trait methods `todo!()`-panic (see the
//! crate-level status table).

pub use crate::error::TurbulenceError;
pub use crate::traits::TurbulenceModel;

pub use crate::k_epsilon::KEpsilon;
pub use crate::k_omega::KOmega;
pub use crate::k_omega_sst::KOmegaSST;
pub use crate::laminar::LaminarModel;
pub use crate::les::Smagorinsky;
pub use crate::spalart_allmaras::SpalartAllmaras;
pub use crate::wall_functions::{nu_t_wall, y_plus};
