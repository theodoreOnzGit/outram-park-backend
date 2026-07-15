// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::boundary_conditions` — GeN-Foam TH boundary conditions
//!
//! This module will hold the GeN-Foam-specific thermal-hydraulics boundary
//! conditions layered on top of basic-lib's generic BC set: the
//! NusseltThermalBaffle1D coupled-wall condition, blackBodyRadiation,
//! velocityRundown (pump/flow coastdown), and timeFieldTable (tabulated
//! time-varying field BC). Generic OpenFOAM BC machinery (fixedValue,
//! zeroGradient, and the rest of basic-lib's BC set) does NOT belong here —
//! only the GeN-Foam-specific closures above it.
//!
//! Ports upstream `src/classes/thermalHydraulics/src/boundaryConditions/**`.
//!
//! Scaffold-only — no boundary conditions are implemented yet. See bead
//! op-p6p.7.13.

// TODO(genfoam): port NusseltThermalBaffle1D, blackBodyRadiation, velocityRundown, timeFieldTable (bead op-p6p.7.13).
