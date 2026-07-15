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

//! # `genfoam::thermal_hydraulics::function_objects` — TH post-processing diagnostics
//!
//! This module will hold run-time post-processing and diagnostic function
//! objects: massFlow, pressureDrop, TBulk, fieldDiffExtents,
//! patchScalarFieldValue, stopIfMaxFieldDiff, and fieldIntegralToFMU. These
//! are pure post-processing/monitoring hooks over solver fields — no physics
//! is computed here, so no solver or closure logic belongs in this module.
//!
//! Ports upstream `src/classes/thermalHydraulics/src/functionObjects/**`.
//!
//! Scaffold-only — no function objects are implemented yet, and this module
//! ports last among the three. See bead op-p6p.7.14.

// TODO(genfoam): port massFlow, pressureDrop, TBulk, fieldDiffExtents, patchScalarFieldValue, stopIfMaxFieldDiff, fieldIntegralToFMU (bead op-p6p.7.14).
