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

//! Wall-function utilities for RAS turbulence models.
//!
//! C++ source: `src/TurbulenceModels/turbulenceModels/RAS/derivedFvPatchFields/`
//!
//! Used when the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).
//! These are **standalone helper functions**, not yet wired into any model as
//! patch boundary conditions, and are untested. The log-law constants
//! (κ = 0.41, E = 9.8, y⁺_lam = 11) are hard-coded.

use petir::mathf::RealMath;
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
        let u_plus = (1.0 / KAPPA) * yp.max(1.0).r_ln() + E.r_ln() / KAPPA;
        let f = u_tau * u_plus - u_wall;
        let df = u_plus + 1.0 / KAPPA;
        let du = f / df;
        u_tau -= du;
        if du.abs() < TOL * u_tau {
            break;
        }
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
    const YP_LAM: f64 = 11.0; // laminar/turbulent sublayer transition

    if y_p <= YP_LAM {
        0.0
    } else {
        nu * (KAPPA * y_p / (E * y_p).r_ln() - 1.0).max(0.0)
    }
}

/// `wallFunctionCoefficients::calcYPlusLam` of OpenFOAM-v2506 (ESI): the
/// laminar / inertial crossover `y+` by ten fixed-point iterations of
/// `y+ = ln(max(E y+, 1)) / kappa` from 11 (gh:#1362 row 5). For the default
/// `kappa = 0.41`, `E = 9.8` it is 11.5301, not the 11.0 that [`nu_t_wall`] uses.
pub fn y_plus_lam(kappa: f64, e: f64) -> f64 {
    let mut ypl = 11.0_f64;
    for _ in 0..10 {
        ypl = (e * ypl).max(1.0).r_ln() / kappa;
    }
    ypl
}

/// Wall `nut` of `nutkWallFunction` (v2506, `blenderType::STEPWISE`, the
/// default) for one face, given its `y+`, `nu`, `kappa` and `E`, in upstream's
/// arithmetic: above `y_plus_lam` it is
/// `nu y+ kappa / ln(max(E y+, 1 + 1e-4)) - nu`, else `nu - nu = 0`.
/// [`nu_t_wall`] is the same law written `nu (kappa y+ / ln(E y+) - 1)` with a
/// clamp at zero and a fixed threshold of 11; this is the ESI form, kept beside
/// it (gh:#1362 row 5).
pub fn nu_t_wall_esi_v2506(y_p: f64, nu: f64, kappa: f64, e: f64) -> f64 {
    let nut_log = nu * y_p * kappa / (e * y_p).max(1.0 + 1e-4).r_ln();
    let nutw = if y_p > y_plus_lam(kappa, e) { nut_log } else { nu };
    nutw - nu
}

#[cfg(test)]
mod gh1362_tests {
    use super::*;

    #[test]
    fn esi_form_matches_the_port_form_above_the_crossover() {
        let (k, e) = (0.41, 9.8);
        assert!((y_plus_lam(k, e) - 11.5301).abs() < 1e-3);
        for yp in [12.0, 30.0, 300.0] {
            let a = nu_t_wall(yp, 1e-5);
            let b = nu_t_wall_esi_v2506(yp, 1e-5, k, e);
            assert!((a - b).abs() <= 1e-9 * a.abs().max(1e-30), "{yp}: {a} {b}");
        }
        assert_eq!(nu_t_wall_esi_v2506(5.0, 1e-5, k, e), 0.0);
        // between 11 and 11.5301 both are zero (the port clamps at 0)
        assert_eq!(nu_t_wall_esi_v2506(11.4, 1e-5, k, e), 0.0);
        assert_eq!(nu_t_wall(11.4, 1e-5), 0.0);
    }
}
