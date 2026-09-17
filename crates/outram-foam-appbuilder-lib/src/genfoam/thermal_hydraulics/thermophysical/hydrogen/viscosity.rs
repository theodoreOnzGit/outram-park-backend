// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/H2I.H
//     (`viscosity0`, `viscosity1`, `viscosityH`, `mu`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Dynamic viscosity
//!
//! Molecular H2's viscosity is Muzny et al. (2013)'s symbolic-regression
//! correlation: a zero-density term ([`zero_density_viscosity`]), a
//! first-density-correction term ([`excess_viscosity`]), and a residual
//! (critical-region) term folded directly into [`dynamic_viscosity`]. Atomic
//! H's viscosity ([`atomic_hydrogen_viscosity`]) comes from Chapman-Enskog
//! kinetic theory via the H-H [`collision_integrals::omega22_22`]. Where both
//! species are present, [`dynamic_viscosity`] combines them with the
//! Hirschfelder-Curtiss-Bird multicomponent mixture rule (the `H_11`/`H_22`/
//! `H_12` system below), not a simple mass blend — unlike every other bulk
//! property in this package.
//!
//! Reference: [5] Muzny, Huber, Kazakov, *Correlation for the viscosity of
//! normal hydrogen obtained from symbolic regression*, J. Chem. Eng. Data
//! 58(4), 2013.
//!
//! ## A faithfully-ported upstream naming quirk
//!
//! Upstream's local variable inside `mu()` is named `rhoH2` but is assigned
//! `rho(p, T)` — the **bulk mixture** density, not the pure-H2 density
//! ([`equation_of_state::density_molecular_h2`](super::equation_of_state::density_molecular_h2)).
//! [`dynamic_viscosity`] reproduces this exactly (using
//! [`equation_of_state::density`](super::equation_of_state::density)); the
//! upstream regression fixture's reference values already reflect it.

use super::collision_integrals::{a12, omega22_22};
use super::constants::{R, TC, W, W_H};
use super::diffusion::binary_diffusion_h_h2;
use super::dissociation::mole_fraction_atomic_h;
use super::equation_of_state::density;

/// Zero-density-limit viscosity of molecular H2 (Muzny et al. Eqs. 3-4,
/// upstream `H2::viscosity0`).
///
/// `T` in K; returns Pa s.
#[inline]
pub(super) fn zero_density_viscosity(t: f64) -> f64 {
    let t_star = t / 30.41;
    let ln_t_star = t_star.ln();
    let s_star = (2.09630e-1 - 4.55274e-1 * ln_t_star + 1.43602e-1 * ln_t_star.powi(2)
        - 3.35325e-2 * ln_t_star.powi(3)
        + 2.76981e-3 * ln_t_star.powi(4))
    .exp();
    0.021357 * (W * t).sqrt() / (0.297 * 0.297 * s_star) * 1e-6
}

/// First-density-correction (excess) viscosity term (Muzny et al. Eqs. 5-7,
/// upstream `H2::viscosity1`).
///
/// `T` in K; returns Pa s per (kg/m^3) — multiply by density to get a Pa s
/// contribution, as [`dynamic_viscosity`] does.
#[inline]
pub(super) fn excess_viscosity(t: f64) -> f64 {
    let t_star = t / 30.41;
    let b_star = -0.1870 + 2.4871 / t_star + 3.7151 / t_star.powi(2) - 11.0972 / t_star.powi(3)
        + 9.0965 / t_star.powi(4)
        - 3.8292 / t_star.powi(5)
        + 0.5166 / t_star.powi(6);
    b_star * 0.297f64.powi(3) * zero_density_viscosity(t)
}

/// Viscosity of pure atomic hydrogen from Chapman-Enskog kinetic theory
/// (upstream `H2::viscosityH`).
///
/// `T` in K; returns Pa s.
#[inline]
pub(super) fn atomic_hydrogen_viscosity(t: f64) -> f64 {
    2.6693e-6 * (W_H * t).sqrt() / omega22_22(t)
}

/// Bulk mixture dynamic viscosity (upstream `H2::mu`).
///
/// Below the mixing region (`x_H = 0`, see the module doc for when this
/// branch is actually reachable in double precision) this is pure molecular
/// H2's Muzny et al. correlation. Otherwise it combines the pure-species
/// viscosities and the [`binary_diffusion_h_h2`] coefficient through the
/// Hirschfelder-Curtiss-Bird mixture rule.
///
/// `p` in Pa, `T` in K; returns Pa s.
#[inline]
pub(super) fn dynamic_viscosity(p: f64, t: f64) -> f64 {
    // Faithful port of upstream naming: this is the *bulk mixture* density
    // (see module doc), not the pure-H2 density.
    let rho_mix = density(p, t);
    let rho_r = rho_mix / 90.5;
    let tr = t / TC;
    let viscosity_h2 = zero_density_viscosity(t)
        + excess_viscosity(t) * rho_mix
        + 1e-6
            * (6.43449673
                * rho_r
                * rho_r
                * (4.56334068e-2 * tr
                    + 2.32797868e-1 / tr
                    + 9.58326120e-1 * rho_r * rho_r / (1.27941189e-1 + tr)
                    + 3.63576595e-1 * rho_r.powi(6))
                .exp());

    let x_h = mole_fraction_atomic_h(p, t);
    if x_h == 0.0 {
        return viscosity_h2;
    }

    let viscosity_h = atomic_hydrogen_viscosity(t);
    let d_h_h2 = binary_diffusion_h_h2(p, t);
    let a_ = a12(t);

    let h11 = (1.0 - x_h).powi(2) / viscosity_h2
        + 2.0 * (1.0 - x_h) * x_h / (W + W_H) * R * t / (p * d_h_h2)
            * (1.0 + 3.0 * W_H * a_ / (5.0 * W));
    let h22 = x_h * x_h / viscosity_h
        + 2.0 * (1.0 - x_h) * x_h / (W + W_H) * R * t / (p * d_h_h2)
            * (1.0 + 3.0 * W * a_ / (5.0 * W_H));
    let h12 = -2.0 * (1.0 - x_h) * x_h / (W + W_H) * R * t / (p * d_h_h2) * (1.0 - 3.0 * a_ / 5.0);

    ((1.0 - x_h).powi(2) / h11 + x_h * x_h / h22 - 2.0 * (1.0 - x_h) * x_h * h12 / (h11 * h22))
        / (1.0 - h12 * h12 / (h11 * h22))
}

#[cfg(test)]
mod tests {
    //! # V&V — dynamic viscosity `mu`
    //!
    //! **Methodology.** Reproduces GeN-Foam's `test_mu` regression fixture: 112
    //! `(p, T) -> mu` points over the same 16-temperature x 7-pressure grid used
    //! by the density/enthalpy/entropy fixtures. Upstream's own `test_mu` uses a
    //! **two-tier** pass criterion (see `main()`'s call `test_mu(0.25)` and the
    //! body of `test_mu` in `Test-hydrogenThermophysicalProperties.C`): a point
    //! with symmetric relative error in `(0.25, 0.9]` prints a warning but still
    //! counts as correct; only `> 0.9` is a hard failure. This port reproduces
    //! that exact two-tier criterion rather than upstream's overall-pass
    //! threshold alone, so that a point upstream itself only *warns* about does
    //! not spuriously fail here.
    //!
    //! **Results (measured 2026-07-15, `cargo test --release`).** See the
    //! `eprintln!` diagnostics for the per-point warning-band breakdown; the
    //! hard-failure assertion (`< 0.9`) passes for all 112 points.
    use super::*;

    const PSIA: f64 = 6894.76;
    const TEMPERATURES_K: [f64; 16] = [
        83.33334, 111.11112, 222.22224, 333.33336, 444.44448, 555.5556, 833.3334, 1111.1112,
        1388.889, 1666.6668, 1944.4446, 2222.2224, 2500.0002, 2777.778, 3055.5558, 3333.3336,
    ];

    fn rel_err(value: f64, reference: f64) -> f64 {
        2.0 * (reference - value).abs() / (reference + value)
    }

    #[test]
    fn viscosity_matches_upstream_regression_fixture() {
        let rows: [(f64, [f64; 16]); 7] = [
            (
                1.0,
                [
                    3.68525e-06,
                    4.37989e-06,
                    7.24294e-06,
                    9.47167e-06,
                    1.15039e-05,
                    1.35344e-05,
                    1.78057e-05,
                    2.16495e-05,
                    2.52176e-05,
                    2.85615e-05,
                    3.17676e-05,
                    3.49392e-05,
                    3.83176e-05,
                    4.20752e-05,
                    4.51089e-05,
                    4.5626e-05,
                ],
            ),
            (
                15.0,
                [
                    3.69387e-06,
                    4.39024e-06,
                    7.24639e-06,
                    9.47339e-06,
                    1.15022e-05,
                    1.3531e-05,
                    1.77971e-05,
                    2.16392e-05,
                    2.52038e-05,
                    2.85529e-05,
                    3.17417e-05,
                    3.48237e-05,
                    3.79091e-05,
                    4.11927e-05,
                    4.48159e-05,
                    4.85081e-05,
                ],
            ),
            (
                50.0,
                [
                    3.71627e-06,
                    4.41609e-06,
                    7.25673e-06,
                    9.47512e-06,
                    1.1497e-05,
                    1.35206e-05,
                    1.77799e-05,
                    2.16133e-05,
                    2.51728e-05,
                    2.85081e-05,
                    3.169e-05,
                    3.47496e-05,
                    3.77902e-05,
                    4.09307e-05,
                    4.43522e-05,
                    4.80944e-05,
                ],
            ),
            (
                150.0,
                [
                    3.77833e-06,
                    4.48849e-06,
                    7.28603e-06,
                    9.48029e-06,
                    1.14867e-05,
                    1.34913e-05,
                    1.77264e-05,
                    2.15409e-05,
                    2.50814e-05,
                    2.84012e-05,
                    3.15676e-05,
                    3.46134e-05,
                    3.76333e-05,
                    4.07428e-05,
                    4.41144e-05,
                    4.77893e-05,
                ],
            ),
            (
                500.0,
                [
                    4.02481e-06,
                    4.74187e-06,
                    7.38773e-06,
                    9.49925e-06,
                    1.14453e-05,
                    1.33931e-05,
                    1.75385e-05,
                    2.12876e-05,
                    2.47642e-05,
                    2.80272e-05,
                    3.11419e-05,
                    3.41342e-05,
                    3.70817e-05,
                    4.00878e-05,
                    4.32818e-05,
                    4.6724e-05,
                ],
            ),
            (
                1500.0,
                [
                    5.1228e-06,
                    5.49167e-06,
                    7.67386e-06,
                    9.55096e-06,
                    1.13384e-05,
                    1.31259e-05,
                    1.703e-05,
                    2.05636e-05,
                    2.38559e-05,
                    2.69585e-05,
                    2.99232e-05,
                    3.27673e-05,
                    3.5508e-05,
                    3.82142e-05,
                    4.09031e-05,
                    4.36783e-05,
                ],
            ),
            (
                4500.0,
                [
                    8.70808e-06,
                    7.83244e-06,
                    8.51847e-06,
                    9.73884e-06,
                    1.11006e-05,
                    1.2464e-05,
                    1.57321e-05,
                    1.87365e-05,
                    2.15461e-05,
                    2.42351e-05,
                    2.67861e-05,
                    2.9251e-05,
                    3.16469e-05,
                    3.39739e-05,
                    3.62492e-05,
                    3.85417e-05,
                ],
            ),
        ];

        let mut max_rel_err = 0.0_f64;
        let mut n_warned = 0;
        for (p_psia, refs) in rows {
            let p_pa = p_psia * PSIA;
            for (t, mu_ref) in TEMPERATURES_K.into_iter().zip(refs) {
                let mu = dynamic_viscosity(p_pa, t);
                let err = rel_err(mu, mu_ref);
                max_rel_err = max_rel_err.max(err);
                if err > 0.25 {
                    n_warned += 1;
                    eprintln!(
                        "mu(p={p_psia} psia, T={t} K) = {mu} Pa.s (ref {mu_ref}), rel err {err} (upstream warning band)"
                    );
                }
                assert!(
                    err < 0.9,
                    "mu(p={p_psia} psia, T={t} K) = {mu} Pa.s (ref {mu_ref}), rel err {err}"
                );
            }
        }
        eprintln!("viscosity fixture: max_rel_err = {max_rel_err}, n_warned = {n_warned}/112");
    }
}
