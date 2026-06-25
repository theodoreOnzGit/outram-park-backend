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

pub type Scalar = f64;
pub type Label = i64;

pub const SMALL: Scalar = 1e-15;
pub const VSMALL: Scalar = 1e-300;
pub const ROOT_SMALL: Scalar = 3.162_277_660_168_379_5e-8; // sqrt(1e-15)
pub const ROOT_VSMALL: Scalar = 1e-150; // sqrt(1e-300)
pub const GREAT: Scalar = 1e15;
pub const VGREAT: Scalar = 1e300;
pub const ROOT_GREAT: Scalar = 3.162_277_660_168_379_5e7; // sqrt(1e15)
