// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Thermo-mechanical property correlations.
//!
//! Seven families, one module each, mirroring upstream's
//! `thermoMechanicalPropertiesModels/` directory layout:
//!
//! | Module | Property | SI unit |
//! |---|---|---|
//! | [`conductivity`] | thermal conductivity | W/(m K) |
//! | [`heat_capacity`] | specific heat capacity | J/(kg K) |
//! | [`density`] | density | kg/m^3 |
//! | [`emissivity`] | surface emissivity | - |
//! | [`young_modulus`] | Young's modulus | Pa |
//! | [`poisson_ratio`] | Poisson's ratio | - |
//! | [`thermal_expansion`] | thermal expansion strain / coefficient | - and 1/K |
//!
//! Every family is an enum over the published correlations for it, evaluated
//! against a [`MaterialState`](crate::materials::MaterialState). See the
//! [module-level documentation](crate::materials) for why enums rather than
//! trait objects, and for the validity-range convention.

pub mod conductivity;
pub mod density;
pub mod emissivity;
pub mod heat_capacity;
pub mod poisson_ratio;
pub mod thermal_expansion;
pub mod young_modulus;
