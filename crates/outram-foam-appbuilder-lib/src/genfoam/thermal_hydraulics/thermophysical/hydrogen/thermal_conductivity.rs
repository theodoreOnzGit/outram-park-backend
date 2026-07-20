// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/H2I.H
//     (`thermalConductivityH2`, `kappa`, `alphah`, `Pr`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Thermal conductivity, thermal diffusivity, Prandtl number
//!
//! Molecular H2's thermal conductivity ([`molecular_h2_thermal_conductivity`])
//! is Assael et al. (2011)'s correlation: a dilute-gas term, a density-excess
//! term, and a critical-enhancement term. The bulk mixture conductivity
//! ([`thermal_conductivity`]) adds two more physical effects on top: a
//! Mason-Saxena-style frozen-mixture conductivity `lambda_f` (built from the
//! same Hirschfelder-Curtiss-Bird machinery as
//! [`viscosity::dynamic_viscosity`](super::viscosity::dynamic_viscosity)) and a
//! **reactive** conductivity term `lambda_r` that accounts for the extra heat
//! transported by the H2 <-> 2H dissociation reaction itself diffusing down a
//! concentration gradient (this is why [`thermal_conductivity`] needs
//! [`diffusion::binary_diffusion_h_h2`](super::diffusion::binary_diffusion_h_h2)
//! and the atomic/molecular enthalpies from
//! [`thermodynamics`](super::thermodynamics), not just the two pure-species
//! conductivities).
//!
//! [`thermal_diffusivity`] and [`prandtl_number`] are the standard derived
//! quantities `kappa / Cp` and `Cp mu / kappa`.
//!
//! Reference: [8] Assael, Assael, Huber, Perkins, Takata, *Correlation of the
//! Thermal Conductivity of Normal and Parahydrogen from the Triple Point to
//! 1000 K and up to 100 MPa*, J. Phys. Chem. Ref. Data 40(3), 2011.
//!
//! ## Not covered by upstream's own regression fixture
//!
//! Unlike `x_H`, `rho`, `Cp`, `Cv`, `mu`, `Ha`, `D`, and `S`, none of
//! `kappa`/`alphah`/`Pr` appear in
//! `tests/hydrogenThermophysicalProperties/Test-hydrogenThermophysicalProperties.C`
//! — there is no upstream-provided reference value to check this port against.
//! The functions below are still a line-for-line transcription of `H2I.H`
//! (same discipline as everything else in this package), but their test
//! module only checks physical sanity (positivity, right order of magnitude),
//! not numerical agreement with an external reference — seeing that
//! distinction matters for how much confidence to place in `kappa` versus,
//! say, `mu`.
//!
//! ## The same mixture-density naming quirk as `mu`
//!
//! [`molecular_h2_thermal_conductivity`] uses the bulk mixture density (via
//! [`equation_of_state::density`](super::equation_of_state::density)), not
//! the pure-H2 density, exactly like
//! [`viscosity::dynamic_viscosity`](super::viscosity::dynamic_viscosity) — see
//! that module's doc for the upstream naming quirk this reproduces.

use super::collision_integrals::{a12, b12, omega22_12};
use super::constants::{R, W, W_H};
use super::diffusion::binary_diffusion_h_h2;
use super::dissociation::mole_fraction_atomic_h;
use super::equation_of_state::density;
use super::heat_capacity::specific_heat_capacity;
use super::thermodynamics::{specific_enthalpy_atomic_h, specific_enthalpy_molecular_h2};
use super::viscosity::{atomic_hydrogen_viscosity, dynamic_viscosity};

/// Thermal conductivity of pure molecular H2, Assael et al. (2011)
/// dilute-gas, density-excess, and critical-enhancement correlation (upstream
/// `H2::thermalConductivityH2`). The upstream comment notes the critical term
/// is an empirical fit with a -8% to +15% deviation from the Roder and Diller
/// data.
///
/// `p` in Pa, `T` in K; returns W/(m K).
#[inline]
pub(super) fn molecular_h2_thermal_conductivity(p: f64, t: f64) -> f64 {
    let t_ = t / 33.45;

    let lambda_0_num = -3.40976e-1 + 4.58820 * t_ - 1.45080 * t_ * t_
        + 3.26394e-1 * t_.powi(3)
        + 3.16939e-3 * t_.powi(4)
        + 1.90592e-4 * t_.powi(5)
        - 1.139e-6 * t_.powi(6);
    let lambda_0_denom = 1.38497e2 - 2.21878e1 * t_ + 4.57151 * t_ * t_ + t_.powi(3);
    let lambda_0 = lambda_0_num / lambda_0_denom;

    // Faithful port of upstream naming: this is the *bulk mixture* density
    // (see module doc), not the pure-H2 density.
    let rho_mix = density(p, t);
    let rho_c = 31.262;
    let rho_r = rho_mix / rho_c;
    let lambda_excess = (3.63081e-2 + 1.83370e-3 * t_) * rho_r
        + (-2.07629e-2 - 8.86716e-3 * t_) * rho_r.powi(2)
        + (3.14810e-2 + 1.58260e-2 * t_) * rho_r.powi(3)
        + (-1.43097e-2 - 1.06283e-2 * t_) * rho_r.powi(4)
        + (1.74980e-3 + 2.80673e-3 * t_) * rho_r.powi(5);

    let lambda_critical =
        6.24e-4 / (-2.58e-7 + t_ - 1.0) * (-(0.837 * (rho_r - 1.0)).powi(2)).exp();

    lambda_0 + lambda_excess + lambda_critical
}

/// Bulk mixture thermal conductivity (upstream `H2::kappa`): a Mason-Saxena
/// frozen-mixture conductivity plus a reactive-conductivity correction for
/// heat carried by dissociation-driven diffusion.
///
/// `p` in Pa, `T` in K; returns W/(m K).
#[inline]
pub(super) fn thermal_conductivity(p: f64, t: f64) -> f64 {
    let x_h = mole_fraction_atomic_h(p, t);
    let w_h_h2 = x_h * W_H + (1.0 - x_h) * W;
    let a_ = a12(t);
    let b_ = b12(t);
    let m12 = 2.0 * W_H * W / (W_H + W);
    let lambda_h2 = molecular_h2_thermal_conductivity(p, t);
    let lambda_h = 15.0 / 4.0 * R * atomic_hydrogen_viscosity(t) / W_H;
    let lambda_12 = 15.0 / 4.0 / w_h_h2 * R * 2.6693e-4 * (m12 * 1e-3 * t).sqrt() / omega22_12(t);

    let u_y = 4.0 / 15.0 * a_ * W_H * W / (m12 * m12) * (lambda_12 * lambda_12)
        / (lambda_h * lambda_h2)
        - (b_ / 5.0 + 1.0 / 12.0)
        - 1.0 / a_ * (3.0 * b_ / 8.0 - 25.0 / 32.0) * (W_H - W).powi(2) / (W_H * W);
    let u_z = 4.0 / 15.0
        * a_
        * (lambda_12 * (1.0 / lambda_h + 1.0 / lambda_h2) * W_H * W / (m12 * m12) - 1.0)
        - (b_ / 5.0 + 1.0 / 12.0);
    let u_1 =
        4.0 / 15.0 * a_ - (b_ / 5.0 + 1.0 / 12.0) * W_H / W + (W_H - W).powi(2) / (2.0 * W_H * W);
    let u_2 =
        4.0 / 15.0 * a_ - (b_ / 5.0 + 1.0 / 12.0) * W / W_H + (W - W_H).powi(2) / (2.0 * W_H * W);

    let x_lambda = x_h * x_h / lambda_h
        + 2.0 * x_h * (1.0 - x_h) / lambda_12
        + (1.0 - x_h).powi(2) / lambda_h2;
    let y_lambda = x_h * x_h / lambda_h * u_1
        + 2.0 * x_h * (1.0 - x_h) / lambda_12 * u_y
        + (1.0 - x_h).powi(2) / lambda_h2 * u_2;
    let z_lambda = x_h * x_h * u_1 + 2.0 * x_h * (1.0 - x_h) * u_z + (1.0 - x_h).powi(2) * u_2;

    let lambda_f = (1.0 + z_lambda) / (x_lambda + y_lambda);

    let delta_h =
        2.0 * W_H * specific_enthalpy_atomic_h(t) - W * specific_enthalpy_molecular_h2(p, t);
    let lambda_r =
        p * binary_diffusion_h_h2(p, t) / t * (delta_h / (R * t)).powi(2) * x_h * (1.0 - x_h)
            / (2.0 - x_h).powi(2);

    lambda_f + lambda_r
}

/// Thermal diffusivity `alpha = kappa / Cp` (upstream `H2::alphah`).
///
/// `p` in Pa, `T` in K; returns kg/(m s) (equivalently m^2/s times density,
/// matching upstream's `kappa/Cp` — this is *not* the usual `kappa/(rho*Cp)`
/// thermal diffusivity, ported exactly as upstream defines it).
#[inline]
pub(super) fn thermal_diffusivity(p: f64, t: f64) -> f64 {
    thermal_conductivity(p, t) / specific_heat_capacity(p, t)
}

/// Prandtl number `Pr = Cp mu / kappa` (upstream `H2::Pr`).
///
/// `p` in Pa, `T` in K; dimensionless.
#[inline]
pub(super) fn prandtl_number(p: f64, t: f64) -> f64 {
    specific_heat_capacity(p, t) * dynamic_viscosity(p, t) / thermal_conductivity(p, t)
}

#[cfg(test)]
mod tests {
    //! # Sanity check — thermal conductivity `kappa` (no upstream reference)
    //!
    //! **Methodology.** As the module doc explains, `kappa` has no reference
    //! value in upstream's regression fixture, so this is *not* a validation —
    //! it only checks physical sanity: `kappa > 0` and within an
    //! order-of-magnitude bracket of literature H2 gas conductivity
    //! (~0.1-1 W/(m K) over 100-3000 K at moderate pressure; real hydrogen gas
    //! conductivity at 300 K, 1 atm is ~0.18 W/(m K), rising with `T`), at a
    //! handful of points spanning the same grid as the other fixtures.
    //!
    //! **Results (measured 2026-07-15, `cargo test --release`).** See
    //! `eprintln!` output for the measured `kappa` at each sampled point.
    use super::*;

    const PSIA: f64 = 6894.76;

    #[test]
    fn thermal_conductivity_is_positive_and_order_of_magnitude_sane() {
        let cases: [(f64, f64); 6] = [
            (15.0, 111.11112),
            (15.0, 833.3334),
            (15.0, 2500.0002),
            (150.0, 333.33336),
            (150.0, 1666.6668),
            (500.0, 3333.3336),
        ];
        for (p_psia, t) in cases {
            let p = p_psia * PSIA;
            let kappa = thermal_conductivity(p, t);
            eprintln!("kappa(p={p_psia} psia, T={t} K) = {kappa} W/(m K)");
            assert!(
                kappa.is_finite() && kappa > 0.0,
                "kappa(p={p_psia} psia, T={t} K) = {kappa} is not a positive finite value"
            );
            assert!(
                (0.01..50.0).contains(&kappa),
                "kappa(p={p_psia} psia, T={t} K) = {kappa} W/(m K) is outside a generous sanity bracket"
            );
        }
    }
}
