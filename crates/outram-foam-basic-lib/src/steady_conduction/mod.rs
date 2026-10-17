// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   applications/solvers/basic/laplacianFoam/laplacianFoam.C
//     Copyright (C) 2011-2017 OpenFOAM Foundation
//     Copyright (C) 2019 OpenCFD Ltd.
//   src/thermoTools/derivedFvPatchFields/externalWallHeatFluxTemperature/
//     externalWallHeatFluxTemperatureFvPatchScalarField.C (convective arm,
//     lines 424-447 of the vendored copy: valueFraction = hp/(hp + kappa
//     deltaCoeffs), refValue = Ta)
//   src/finiteVolume/interpolation/surfaceInterpolation/schemes/harmonic/
//     harmonic.H (face conductivity 1/reverseLinear(1/k))
//   Upstream licence: GPL-3.0-or-later.
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

//! # Steady heat conduction with a volumetric source (gh:#949)
//!
//! A small steady conduction solver: the steady form of OpenFOAM's
//! `laplacianFoam`,
//!
//! ```text
//!   -div( k(T) grad T ) = q'''          in the solid
//! ```
//!
//! with a per-zone uniform heat source `q'''` (W/m^3), a per-zone
//! conductivity `k(T)` (W/(m K)) supplied by the caller, and one thermal
//! condition per boundary patch: adiabatic, fixed temperature, or convective
//! (`-k dT/dn = h (T - T_ambient)`).
//!
//! It exists for the **beginner thermal profile** of `outram-mc-libs`
//! (gh:#949, under #945): a coarse temperature field for any shape, with an
//! **energy balance** (heat generated against heat leaving through the
//! boundary) reported beside it. Serious thermal-hydraulics is `nee_soon`'s.
//!
//! ## What is ported, and what is new
//!
//! | piece | source |
//! |---|---|
//! | the equation, `fvm::laplacian(k, T) == source` | `laplacianFoam.C` (`TEqn`), run steady (`ddt` dropped, as with `ddtSchemes steadyState`) |
//! | the convective boundary as a mixed (Robin) condition, `valueFraction = h/(h + k/d)`, `refValue = T_a` | `externalWallHeatFluxTemperatureFvPatchScalarField.C` (`hp` without radiation or solid layers) |
//! | `k(T)` re-evaluated between outer passes (Picard) | the outer `simple.loop()` of `laplacianFoam`, here with `k` updated each pass (upstream's `DT` is constant) |
//! | **face conductivity as series resistance**, `A / (d_P/k_P + d_N/k_N)` | OpenFOAM's `harmonic` interpolation (`schemes/harmonic/harmonic.H`: `1/reverseLinear(1/k)`), chosen over the default `Gauss linear`, which averages `k` linearly and is wrong at a material interface |
//! | **energy balance** from the boundary condition definitions, not from the matrix | new here |
//! | radial (slab / cylinder / sphere) and voxel meshers | new here ([`mesh`]) |
//!
//! ## The energy balance, and what it can catch
//!
//! The heat leaving each boundary face is recomputed from that face's own
//! condition (`h`, `T_a`, `k` of its cell, the face distance) and the solved
//! cell temperature, independently of the assembled matrix. In a correct
//! finite-volume solve the interior fluxes cancel pairwise, so generated minus
//! removed equals the sum of the linear-solver residuals: it is small by
//! construction. Its job is to catch an assembly that drops or doubles a
//! boundary contribution (a zero boundary conductivity was such a defect
//! elsewhere in this workspace; see `outram-foam-appbuilder-lib`'s
//! `thermo_mechanics::mesh_solve`) and an unconverged solve. It does **not**
//! measure discretisation error; the closed-form tests in this module do.
//!
//! ## Verification
//!
//! [`closed_form`] is the exact solution for uniform `q'''` in nested slab,
//! cylinder and sphere shells with a fixed-temperature or convective surface.
//! The tests in `tests.rs` compare the solve against it and measure the
//! convergence order; the methodology and results are in their doc comments.
//!
//! Status: untrusted AI-generated draft (2026-10-17) per `RESPONSIBLE_USE.md`.
//! For research, education and V&V only.

pub mod closed_form;
pub mod mesh;
mod solve;

#[cfg(test)]
mod tests;

pub use closed_form::{RadialClosedForm, RadialLayer};
pub use mesh::{
    radial_mesh, voxel_mesh, ConductionMesh, RadialGeometry, RadialShell, VoxelGrid, VoxelMesh,
};
pub use solve::{
    EnergyBalance, PatchCondition, SteadyConductionProblem, SteadyConductionSettings,
    SteadyConductionSolution, ThermalBoundary,
};

use thiserror::Error;

/// What can go wrong setting up or solving a steady conduction problem.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SteadyConductionError {
    /// An input was out of range or inconsistent (the message says which).
    #[error("invalid steady-conduction input: {0}")]
    InvalidInput(String),
    /// The mesh could not be assembled.
    #[error("mesh assembly failed: {0}")]
    Mesh(String),
    /// The linear solve missed its tolerance.
    #[error("linear solve did not converge: residual {residual:e} after {iterations} iterations")]
    LinearSolveFailed {
        /// Normalised residual at exit.
        residual: f64,
        /// Iterations performed.
        iterations: usize,
    },
    /// The outer (Picard) iteration on `k(T)` did not settle.
    #[error("Picard iteration on k(T) did not converge: last change {last_change_kelvin:e} K after {iterations} passes")]
    PicardNotConverged {
        /// Largest cell temperature change of the last pass, K.
        last_change_kelvin: f64,
        /// Passes performed.
        iterations: usize,
    },
    /// The caller's conductivity returned a non-positive or non-finite value.
    #[error("conductivity for zone {zone} at {temperature_kelvin} K is {value} W/(m K); it must be positive and finite")]
    BadConductivity {
        /// Zone index.
        zone: usize,
        /// Temperature it was asked at, K.
        temperature_kelvin: f64,
        /// What it returned, W/(m K).
        value: f64,
    },
}
