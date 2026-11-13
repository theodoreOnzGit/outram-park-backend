// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Closures shared by the 1-D solvers (RELAP-7 parity epic gh:#1056).
//!
//! - [`wall_friction`]: the RELAP5 four-region Darcy friction factor with
//!   Zigrang–Sylvester's turbulent form (gh:#1061). It lives in
//!   `tampines_steam_tables::hem::wall_friction`, beside the HEM pipe that
//!   also uses it, and is re-exported here for `multiphase_1d`.

/// The RELAP5 four-region wall friction (gh:#1061).
pub mod wall_friction {
    pub use tampines_steam_tables::hem::wall_friction::{
        darcy_friction_factor, darcy_friction_factor_with, wall_friction_acceleration,
        zigrang_sylvester, LowReynolds, RE_F_MAX, RE_LAMINAR_MAX, RE_TURBULENT_MIN,
    };
}
