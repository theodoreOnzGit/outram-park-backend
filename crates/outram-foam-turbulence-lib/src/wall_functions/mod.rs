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

/// Wall function utilities for RAS turbulence models.
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/RAS/derivedFvPatchFields/`
///
/// Used when the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).

/// Dimensionless wall distance y⁺ = u_τ · y / ν.
///
/// # Arguments
/// * `y`     — wall-normal distance from wall to cell centre [m]
/// * `u_tau` — friction velocity [m/s]
/// * `nu`    — kinematic viscosity [m²/s]
pub fn y_plus(y: f64, u_tau: f64, nu: f64) -> f64 {
    u_tau * y / nu
}

/// Friction velocity u_τ from log-law iteration.
///
/// Solves  U⁺ = (1/κ) ln(E·y⁺)  for u_τ = U_wall / U⁺
/// using a Newton iteration.
///
/// # Arguments
/// * `u_wall` — tangential velocity at wall cell centre [m/s]
/// * `y`      — wall-normal distance [m]
/// * `nu`     — kinematic viscosity [m²/s]
pub fn u_tau(u_wall: f64, y: f64, nu: f64) -> f64 {
    const KAPPA: f64 = 0.41;
    const E: f64 = 9.8;
    const MAX_ITER: usize = 20;
    const TOL: f64 = 1e-8;

    let mut u_tau = (u_wall * nu / y).sqrt().max(1e-10);
    for _ in 0..MAX_ITER {
        let yp = y_plus(y, u_tau, nu);
        let u_plus = (1.0 / KAPPA) * yp.max(1.0).ln() + E.ln() / KAPPA;
        let f = u_tau * u_plus - u_wall;
        let df = u_plus + 1.0 / KAPPA;
        let du = f / df;
        u_tau -= du;
        if du.abs() < TOL * u_tau { break; }
    }
    u_tau
}

/// Turbulent kinematic viscosity at the wall cell (nutWallFunction).
///
/// Returns ν_t such that the log-law is satisfied:
///   ν_t = ν · (κ · y⁺ / ln(E · y⁺) − 1)  for y⁺ > y⁺_lam
///   ν_t = 0                                  for y⁺ ≤ y⁺_lam (viscous sublayer)
pub fn nu_t_wall(y_p: f64, nu: f64) -> f64 {
    const KAPPA: f64 = 0.41;
    const E: f64 = 9.8;
    const YP_LAM: f64 = 11.0;  // laminar/turbulent sublayer transition

    if y_p <= YP_LAM {
        0.0
    } else {
        nu * (KAPPA * y_p / (E * y_p).ln() - 1.0).max(0.0)
    }
}
