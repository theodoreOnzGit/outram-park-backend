// Note: //! indicates crate level documentation
//
//
//! A Library which contains useful traits and methods for thermal
//! hydraulics calculations in salt loops
//!
//!
//! This crate has heavy reliance on units of measure (uom) released under
//! Apache 2.0 license. So you'll need to get used to unit safe calculations
//! with uom as well.
//!
//!
//! This library was initially developed for
//! use in my PhD thesis under supervision
//! of Professor Per F. Peterson. It a thermal hydraulics
//! library in Rust that is released under the GNU General Public License
//! v 3.0. This is partly due to the fact that some of the libraries
//! inherit from GeN-Foam and OpenFOAM, both licensed under GNU General
//! Public License v3.0.
//!
//! As such, the entire library is released under GNU GPL v3.0. It is a strong
//! copyleft license which means you cannot use it in proprietary software.
//!
//!
//! License
//!    This is a thermal hydraulics library written
//!    in rust meant to help with the
//!    fluid mechanics and heat transfer aspects of the calculations
//!    for the Compact Integral Effects Tests (CIET) and hopefully
//!    Gen IV Reactors such as the Fluoride Salt cooled High Temperature
//!    Reactor (FHR)
//!     
//!    Copyright (C) 2022-2023  Theodore Kay Chen Ong, Singapore Nuclear
//!    Research and Safety Initiative, Per F. Peterson, University of
//!    California, Berkeley Thermal Hydraulics Laboratory
//!
//!    tuas_boussinesq_solver is free software; you can
//!    redistribute it and/or modify it
//!    under the terms of the GNU General Public License as published by the
//!    Free Software Foundation; either version 2 of the License, or (at your
//!    option) any later version.
//!
//!    tuas_boussinesq_solver is distributed in the hope
//!    that it will be useful, but WITHOUT
//!    ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
//!    FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
//!    for more details.
//!
//!    This thermal hydraulics library
//!    contains some code copied from GeN-Foam, and OpenFOAM derivative.
//!    This offering is not approved or endorsed by the OpenFOAM Foundation nor
//!    OpenCFD Limited, producer and distributor of the OpenFOAM(R)software via
//!    www.openfoam.com, and owner of the OPENFOAM(R) and OpenCFD(R) trademarks.
//!    Nor is it endorsed by the authors and owners of GeN-Foam.
//!
//!    You should have received a copy of the GNU General Public License
//!    along with this program.  If not, see <http://www.gnu.org/licenses/>.
//!
//! © All rights reserved. Theodore Kay Chen Ong,
//! Singapore Nuclear Research and Safety Initiative,
//! Per F. Peterson,
//! University of California, Berkeley Thermal Hydraulics Laboratory
//!
//! Main author of the code: Theodore Kay Chen Ong, supervised by
//! Professor Per F. Peterson
//!
//! Btw, I no affiliation with the Rust Foundation.
//!
#![warn(missing_docs)]
extern crate uom;

// /// for mostly incompressible fluids using the Boussinesq Approximation
// /// that is, density doesn't change much except for natural convection
// ///
// /// also, radiation heat transfer is NOT included in this one (yet)
// /// though to be honest, it is not too different in implementation compared
// /// to conduction
// pub mod boussinesq_solver;

/// for mostly incompressible fluids using the Boussinesq Approximation
/// that is, density doesn't change much except for natural convection
///
/// also, radiation heat transfer is NOT included in this one (yet)
/// though to be honest, it is not too different in implementation compared
/// to conduction

/// use peroxide macros
#[macro_use]
extern crate peroxide;

/// provides error types for tuas_boussinesq_solver
pub mod tuas_lib_error;

/// Pure-Rust dense LU solver (`SquareMatrix`), inlined from `outram-foam-basic-lib`
/// so this crate has no `outram-foam-basic-lib` dependency — see the module doc.
pub mod matrix;

#[warn(missing_docs)]
/// prelude, for easy importing
pub mod prelude;

#[warn(missing_docs)]
/// Module specifically for thermophysical properties
/// For liquids and solids with almost invariable density
///
pub mod boussinesq_thermophysical_properties;

#[warn(missing_docs)]
/// Module for correlations of fluid mechanics
/// suitable for tuas_boussinesq_solver (single phase, negligble density changes
/// except for buoyancy)
pub mod fluid_mechanics_correlations;

#[warn(missing_docs)]
/// Module for heat transfer correlations
/// suitable for tuas_boussinesq_solver (single phase, negligble density changes
/// except for buoyancy)
pub mod heat_transfer_correlations;

#[warn(missing_docs)]
/// specific dimensions for control volume construction
pub mod control_volume_dimensions;

#[warn(missing_docs)]
/// Module for boundary conditions
pub mod boundary_conditions;

/// Module for single control volumes (mainly for fluid control volumes,
/// but solid control volumes are set by setting flowrate to zero)
///
/// Single control volumes by default have functions which abstract away
/// the details of calculating heat transfer between different
/// single control volumes as well as between single control volumes and
/// different boundary conditions
///
/// This will abstract away some functionality of the following
/// modules, and is therefore dependent on these modules:
///
/// 1. boussinesq_thermophysical_properties
/// 2. fluid_mechanics_correlations
/// 3. heat_transfer_correlations
/// 4. control_volume_dimensions
/// 5. boundary_conditions
///
/// By itself, it will NOT contain functions on how to interact with array
/// control volumes. This is to prevent overbloated hard to read code
#[warn(missing_docs)]
pub mod single_control_vol;

/// Module for array control volumes (mainly for fluid control volumes,
/// but solid control volumes are set by setting flowrate to zero)
/// suitable for tuas_boussinesq_solver (single phase, negligble density changes
/// except for buoyancy)
///
/// also contains code to help calculate pressure drop and mass flow rate
/// amongst multiple fluid components (eg. pipes) which are usually
/// represented by array control volumes
/// This will abstract away some functionality of the following
/// modules, and is therefore dependent on these modules:
///
/// 1. boussinesq_thermophysical_properties
/// 2. fluid_mechanics_correlations
/// 3. heat_transfer_correlations
/// 4. control_volume_dimensions
/// 5. boundary_conditions
/// 6. single_control_vol
///
/// By itself, it will NOT contain functions on how to interact with array
/// control volumes. This is to prevent overbloated hard to read code
#[warn(missing_docs)]
pub mod array_fluid_collections;
/// Compatibility re-export: this module was physically renamed to
/// [`array_fluid_collections`] on 2026-08-19 (workspace file-path-length hard
/// rule, root `CLAUDE.md`) because its old directory name, stacked with its
/// own child module names, pushed several nested files past the new
/// 170-character cap on Windows. The old public path is kept working so
/// downstream crates (in this workspace and on crates.io) do not need to
/// update their `use` paths.
pub use array_fluid_collections as array_control_vol_and_fluid_component_collections;

/// Module for pre-built-components
/// suitable for tuas_boussinesq_solver (single phase, negligble density changes
/// except for buoyancy)
///
/// It's dependent on all the other modules within the tuas_boussinesq_solver
///
/// You don't want to write everything from scratch right?
#[warn(missing_docs)]
pub mod pre_built_components;

/// Test-support helpers for verification & validation (V&V) tests.
///
/// Compiled only under `cfg(test)`, so it adds nothing to the public API.
/// Its single job is to give every CSV-writing test a deterministic output
/// location under the crate's gitignored `verification_and_validation/`
/// folder, so the raw simulation traces land in one predictable place for
/// plotting regardless of the directory a test happens to run from.
#[cfg(test)]
pub(crate) mod vnv_test_support {
    use std::path::PathBuf;

    /// Deterministic path for a V&V CSV output file.
    ///
    /// Returns `<crate manifest dir>/verification_and_validation/<file_name>`,
    /// creating the `verification_and_validation/` directory first if it does
    /// not exist (so a test never fails on a missing folder). The path is built
    /// from `CARGO_MANIFEST_DIR`, so it is independent of the test process's
    /// current working directory.
    ///
    /// # Parameters
    /// - `file_name`: a bare CSV file name such as `"ciet_heater_steady_state.csv"`
    ///   (no directory components). Name it per test/dataset so the output is
    ///   easy to find and plot.
    ///
    /// The returned [`PathBuf`] is accepted directly by `csv::Writer::from_path`.
    pub fn vnv_csv_path(file_name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation");
        std::fs::create_dir_all(&dir)
            .expect("could not create verification_and_validation output directory");
        dir.join(file_name)
    }
}
