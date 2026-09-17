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

//! Explicit finite-volume operators (`fvc`) — each returns a *new* field
//! evaluated from the current field values: gradient, divergence, face
//! interpolation, surface-normal gradient, flux, reconstruction, MUSCL
//! limiting and ddt correction. Mirrors `Foam::fvc::` from
//! `src/finiteVolume/finiteVolume/fvc/`.

mod ddt_corr;
mod div;
mod flux;
mod grad;
mod interpolate;
mod muscl;
mod reconstruct;
mod sn_grad;

pub use ddt_corr::ddt_corr;
pub use div::{div, div_flux, div_limited, div_vec};
pub use flux::{flux, buoyancy_flux};
pub use grad::grad;
pub use interpolate::interpolate;
pub use muscl::{reconstruct_pos_neg, Limiter};
pub use reconstruct::reconstruct;
pub use sn_grad::sn_grad;
