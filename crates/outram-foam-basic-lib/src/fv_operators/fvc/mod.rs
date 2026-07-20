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

//! Explicit (`fvc`) finite-volume operators — each returns a **new field**
//! (a `VolField` / `SurfaceField`), never a matrix.
//!
//! Mirrors `Foam::fvc::` (`src/finiteVolume/finiteVolume/fvc/`). Contents:
//! Gauss gradient (`grad`, `grad_vec`), Gauss divergence (`div`, `div_flux`,
//! `div_vec`, `div_tensor`, `div_symm_tensor`), surface-normal gradient
//! (`sn_grad`), linear face interpolation (`interpolate`) and flux assembly
//! (`flux`, `buoyancy_flux`), least-squares velocity reconstruction
//! (`reconstruct`), the Rhie–Chow time-derivative flux correction
//! (`ddt_corr`), and MUSCL / TVD limited face reconstruction
//! (`reconstruct_pos_neg`, `Limiter`). Field values carry raw
//! `f64` / `Vector3` / `Tensor` element data (no `uom`), consistent with the
//! rest of the FV operator layer.

mod ddt_corr;
mod div;
mod div_tensor;
mod flux;
mod grad;
mod grad_vec;
mod interpolate;
mod muscl;
mod reconstruct;
mod sn_grad;

pub use ddt_corr::ddt_corr;
pub use div::{div, div_flux, div_vec};
pub use div_tensor::{div_symm_tensor, div_tensor};
pub use flux::{buoyancy_flux, flux};
pub use grad::grad;
pub use grad_vec::grad_vec;
pub use interpolate::interpolate;
pub use muscl::{reconstruct_pos_neg, Limiter};
pub use reconstruct::reconstruct;
pub use sn_grad::sn_grad;
