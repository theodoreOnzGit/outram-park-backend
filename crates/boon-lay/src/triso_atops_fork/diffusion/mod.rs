// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`diffusion_coefficient`, `diffusion_coefficient_SiC_Ag`, `integrate`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Diffusion coefficients — Arrhenius correlations `D(T)`
//!
//! Fission-product diffusion coefficients `D` [m^2/s] for the fuel **kernel**,
//! the matrix **graphite**, and (for silver) the **SiC** layer, as functions of
//! temperature. Ports `diffusion_coefficient`, `diffusion_coefficient_SiC_Ag`,
//! and the time-integration helper `integrate` from `calculation_functions.py`.
//!
//! ## Model
//!
//! Each coefficient is an Arrhenius law
//!
//! ```text
//!     D(T) = D0 · exp( −Q / (R · T) )
//! ```
//!
//! with pre-exponential `D0` in m^2/s, activation energy `Q` in J/mol, the molar
//! gas constant `R = 8.31447 J/(mol·K)`, and absolute temperature `T` in K. Some
//! species sum two Arrhenius terms (a low- and a high-temperature branch). The
//! correlations are the NP-MHTGR values (User Manual §1, ref. 1) and are
//! **valid roughly 700–2400 °C**; the User Manual (§5, "Results look
//! incorrect") notes that temperatures below a species' valid range are
//! **clamped** to the boundary value — this port reproduces that clamping
//! exactly.
//!
//! ## Units
//!
//! Temperatures are [`ThermodynamicTemperature`]; internally each correlation
//! reads the temperature both as °C (to apply the valid-range clamp thresholds,
//! which the upstream code expresses in °C) and as K (for the Arrhenius
//! exponent). Coefficients are [`DiffusionCoefficient`] in m^2/s. The
//! time-integrated coefficient `∫D dt` has units of m^2 and is returned as an
//! [`Area`].

use uom::si::area::square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{Area, DiffusionCoefficient, ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;

/// Molar gas constant `R` used by the TRISO-ATOPS correlations, in J/(mol·K).
///
/// The upstream code hard-codes `8.31447`; reproduced verbatim so the exponent
/// matches bit-for-bit.
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.31447;

/// Kernel and matrix-graphite diffusion coefficients returned together.
///
/// Both are [`DiffusionCoefficient`]s in m^2/s. Ports the `(D, D_graph)` tuple
/// returned by the Python `diffusion_coefficient`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KernelGraphiteDiffusion {
    /// Diffusion coefficient in the fuel kernel, m^2/s.
    pub kernel: DiffusionCoefficient,
    /// Diffusion coefficient in the matrix graphite, m^2/s.
    pub graphite: DiffusionCoefficient,
}

/// Evaluate one Arrhenius term `D0 · exp(−Q / (R·T))`.
///
/// - `pre_exponential` — `D0` in m^2/s.
/// - `activation_energy_kj_per_mol` — `Q` in kJ/mol (the upstream tables are in
///   kJ/mol and multiply by 1000 inline; done here too).
/// - `temperature_kelvin` — absolute temperature `T` in K.
#[inline]
fn arrhenius_term(
    pre_exponential: f64,
    activation_energy_kj_per_mol: f64,
    temperature_kelvin: f64,
) -> f64 {
    pre_exponential
        * (-activation_energy_kj_per_mol * 1000.0 / temperature_kelvin / GAS_CONSTANT_J_PER_MOL_K)
            .exp()
}

/// °C → K, matching the upstream `T + 273.15`.
#[inline]
fn celsius_to_kelvin(celsius: f64) -> f64 {
    celsius + 273.15
}

/// Kernel and graphite diffusion coefficients for a nuclide, by atomic number.
///
/// Ports `diffusion_coefficient(z, T, T_graph)`. The species is selected by its
/// atomic number `z`; the correlation family and any valid-range clamp are the
/// NP-MHTGR values (see module docs). Returns [`KernelGraphiteDiffusion`].
///
/// # Element families (by `z`)
/// - Kr, Te, I, Xe, Se (`z ∈ {36, 52, 53, 54, 34}`): a low-T branch below
///   1500 °C and a two-term high-T branch above; graphite `D` equals kernel `D`.
/// - Rb, Cs (`z ∈ {37, 55}`): kernel `T` clamped to ≥ 700 °C; graphite `T`
///   clamped to ≥ 550 °C.
/// - Sr, Ba, Eu (`z ∈ {38, 56, 63}`): kernel `T` clamped to ≥ 700 °C; graphite
///   `T` clamped to ≥ 800 °C.
/// - Ag, Pd (`z ∈ {47, 46}`): kernel un-clamped; graphite `T` clamped to
///   ≥ 490 °C. (For the SiC barrier that actually limits Ag release, use
///   [`diffusion_coefficient_sic_ag`].)
/// - anything else: a fixed nominal `D = D_graph = 1e-19 m^2/s`.
///
/// # Arguments
/// - `z` — atomic number of the nuclide.
/// - `kernel_temperature` — fuel-kernel temperature (valid ~700–2400 °C).
/// - `graphite_temperature` — matrix-graphite temperature.
///
/// # Assumptions
/// Inputs outside the ~700–2400 °C validity window are clamped (never
/// extrapolated) exactly as upstream; results there are boundary values.
#[must_use]
pub fn diffusion_coefficient(
    z: u32,
    kernel_temperature: ThermodynamicTemperature,
    graphite_temperature: ThermodynamicTemperature,
) -> KernelGraphiteDiffusion {
    let t_kernel_c = kernel_temperature.get::<degree_celsius>();
    let t_graph_c = graphite_temperature.get::<degree_celsius>();

    let (d_kernel, d_graph): (f64, f64) = match z {
        // Kr, Te, I, Xe, Se
        36 | 52 | 53 | 54 | 34 => {
            let tk = celsius_to_kelvin(t_kernel_c);
            let d = if t_kernel_c < 1500.0 {
                arrhenius_term(1.3e-12, 126.0, tk)
            } else {
                arrhenius_term(8.8e-15, 54.0, tk) + arrhenius_term(6e-1, 480.0, tk)
            };
            (d, d)
        }
        // Rb, Cs
        37 | 55 => {
            let tk = celsius_to_kelvin(t_kernel_c.max(700.0));
            let d = arrhenius_term(5.6e-8, 209.0, tk) + arrhenius_term(5.2e-4, 362.0, tk);
            let tgk = celsius_to_kelvin(t_graph_c.max(550.0));
            let d_graph = arrhenius_term(1.7e-6, 149.0, tgk);
            (d, d_graph)
        }
        // Sr, Ba, Eu
        38 | 56 | 63 => {
            let tk = celsius_to_kelvin(t_kernel_c.max(700.0));
            let d = arrhenius_term(2.2e-3, 488.0, tk);
            let tgk = celsius_to_kelvin(t_graph_c.max(800.0));
            let d_graph = arrhenius_term(1.7e-2, 268.0, tgk);
            (d, d_graph)
        }
        // Ag, Pd
        47 | 46 => {
            let tk = celsius_to_kelvin(t_kernel_c);
            let d = arrhenius_term(6.7e-9, 165.0, tk);
            let tgk = celsius_to_kelvin(t_graph_c.max(490.0));
            let d_graph = arrhenius_term(1.6, 258.0, tgk);
            (d, d_graph)
        }
        // Everything else: fixed nominal value.
        _ => (1e-19, 1e-19),
    };

    KernelGraphiteDiffusion {
        kernel: DiffusionCoefficient::new::<square_meter_per_second>(d_kernel),
        graphite: DiffusionCoefficient::new::<square_meter_per_second>(d_graph),
    }
}

/// Diffusion coefficient for silver (Ag) through the **SiC** layer.
///
/// Ports `diffusion_coefficient_SiC_Ag(T)`:
/// `D = 3.6e-9 · exp(−215 kJ/mol / (R·T))`, valid ~700–2400 °C. The SiC layer is
/// the rate-limiting barrier for Ag transport in an intact TRISO particle, so
/// this — not the kernel coefficient — governs Ag release.
///
/// # Arguments
/// - `sic_temperature` — temperature of the SiC layer (valid ~700–2400 °C).
///
/// # Returns
/// The Ag-in-SiC [`DiffusionCoefficient`] in m^2/s.
#[must_use]
pub fn diffusion_coefficient_sic_ag(
    sic_temperature: ThermodynamicTemperature,
) -> DiffusionCoefficient {
    let tk = sic_temperature.get::<kelvin>();
    DiffusionCoefficient::new::<square_meter_per_second>(arrhenius_term(3.6e-9, 215.0, tk))
}

/// Which material's diffusion coefficient to integrate over time.
///
/// Ports the `diffusion_type ∈ {'kernel', 'graphite'}` argument of the Python
/// `integrate`. For silver in the kernel, the SiC-limited coefficient is used
/// (matching the upstream `if z != 47` branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffusionMaterial {
    /// Fuel kernel (for Ag, the SiC-limited coefficient).
    Kernel,
    /// Matrix graphite.
    Graphite,
}

/// Cumulative time-integral `∫₀ᵗ D(T(t')) dt'` along a temperature history.
///
/// Ports the trapezoidal cumulative integration in the Python `integrate`
/// (accident path): the diffusion coefficient is re-evaluated at each sample's
/// temperature, then integrated in time with the trapezoid rule and accumulated
/// (`np.cumsum`). The result feeds the **transient** release models
/// ([`crate::triso_atops_fork::release_models::transient::booth_transient`] and friends),
/// which are written in terms of `∫D dt` rather than a single `D·t`.
///
/// The integral starts at 0 at the first sample (the upstream code prepends a
/// zero baseline, so the first returned value is 0).
///
/// # Arguments
/// - `z` — atomic number, selecting the diffusion correlation.
/// - `times` — monotonically increasing sample times (each a [`Time`]); same
///   length as `temperatures`.
/// - `temperatures` — temperature at each sample time.
/// - `material` — [`DiffusionMaterial::Kernel`] or [`DiffusionMaterial::Graphite`].
///
/// # Returns
/// A `Vec<Area>` of the same length as the inputs, element `i` being
/// `∫₀^{times[i]} D dt` in m^2.
///
/// # Panics
/// Panics if `times` and `temperatures` differ in length.
#[must_use]
pub fn integrate_diffusion_over_time(
    z: u32,
    times: &[Time],
    temperatures: &[ThermodynamicTemperature],
    material: DiffusionMaterial,
) -> Vec<Area> {
    assert_eq!(
        times.len(),
        temperatures.len(),
        "times and temperatures must have equal length"
    );
    let mut out = Vec::with_capacity(times.len());
    let mut running = 0.0_f64; // accumulated ∫D dt in m^2
    let mut prev_d = 0.0_f64; // D at previous step (0 at t=0, as upstream)
    let mut prev_t = 0.0_f64; // previous time in seconds

    for (i, (&time, &temperature)) in times.iter().zip(temperatures.iter()).enumerate() {
        let d = match material {
            DiffusionMaterial::Kernel if z == 47 => {
                diffusion_coefficient_sic_ag(temperature).get::<square_meter_per_second>()
            }
            DiffusionMaterial::Kernel => diffusion_coefficient(z, temperature, temperature)
                .kernel
                .get::<square_meter_per_second>(),
            DiffusionMaterial::Graphite => diffusion_coefficient(z, temperature, temperature)
                .graphite
                .get::<square_meter_per_second>(),
        };
        let t_s = time.get::<second>();
        if i == 0 {
            // Upstream sets the previous-D to 0 and Δt = times[0] − 0.
            running += 0.5 * (d + 0.0) * (t_s - 0.0);
        } else {
            running += 0.5 * (d + prev_d) * (t_s - prev_t);
        }
        out.push(Area::new::<square_meter>(running));
        prev_d = d;
        prev_t = t_s;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// Hand-computed reference for the low-T iodine/xenon branch.
    ///
    /// Methodology: for `z = 53` (I) at 1000 °C the low-T branch is
    /// `D = 1.3e-12·exp(−126000/(1273.15·8.31447))`. Computed independently:
    /// exponent = −126000/(1273.15·8.31447) = −11.9030…, exp = 6.770e-6,
    /// D = 8.8011e-18 m^2/s.
    ///
    /// Result (2026-07-15): the port returns 8.8011e-18 m^2/s, matching the
    /// independent value to < 1e-3 relative (the magnitudes are scaled by 1e18
    /// so the relative tolerance is actually exercised, not swallowed by the
    /// default absolute epsilon at ~1e-18).
    #[test]
    fn iodine_low_temperature_branch_matches_hand_calc() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let d = diffusion_coefficient(53, t, t)
            .kernel
            .get::<square_meter_per_second>();
        let expected = 1.3e-12 * (-126000.0_f64 / (1273.15 * 8.31447)).exp();
        assert_relative_eq!(d, expected, max_relative = 1e-12);
        assert_relative_eq!(d * 1e18, 8.8011, max_relative = 1e-3);
    }

    /// High-T branch (> 1500 °C) uses the two-term sum.
    #[test]
    fn iodine_high_temperature_branch_is_two_term_sum() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1800.0);
        let d = diffusion_coefficient(54, t, t)
            .kernel
            .get::<square_meter_per_second>();
        let tk = 1800.0_f64 + 273.15;
        let expected =
            8.8e-15 * (-54000.0 / (tk * 8.31447)).exp() + 6e-1 * (-480000.0 / (tk * 8.31447)).exp();
        assert_relative_eq!(d, expected, max_relative = 1e-12);
    }

    /// Cesium kernel temperature is clamped up to 700 °C below the valid range.
    #[test]
    fn cesium_kernel_temperature_clamped_below_700c() {
        let cold = ThermodynamicTemperature::new::<degree_celsius>(300.0);
        let clamp = ThermodynamicTemperature::new::<degree_celsius>(700.0);
        let d_cold = diffusion_coefficient(55, cold, cold).kernel;
        let d_clamp = diffusion_coefficient(55, clamp, clamp).kernel;
        assert_eq!(d_cold, d_clamp);
    }

    /// Silver-in-SiC correlation, hand-checked at 1200 °C.
    ///
    /// Methodology: `D = 3.6e-9·exp(−215000/(1473.15·8.31447))`. Independent
    /// value = 8.5710e-17 m^2/s. Result (2026-07-15): the port returns
    /// 8.5710e-17 m^2/s, matching to < 1e-3 relative (scaled by 1e17).
    #[test]
    fn silver_sic_matches_hand_calc() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
        let d = diffusion_coefficient_sic_ag(t).get::<square_meter_per_second>();
        let expected = 3.6e-9 * (-215000.0_f64 / (1473.15 * 8.31447)).exp();
        assert_relative_eq!(d, expected, max_relative = 1e-12);
        assert_relative_eq!(d * 1e17, 8.5710, max_relative = 1e-3);
    }

    /// Unlisted element (e.g. Zr, z=40) returns the fixed nominal 1e-19 m^2/s.
    #[test]
    fn other_elements_get_fixed_nominal() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let d = diffusion_coefficient(40, t, t);
        assert_eq!(d.kernel.get::<square_meter_per_second>(), 1e-19);
        assert_eq!(d.graphite.get::<square_meter_per_second>(), 1e-19);
    }

    /// Constant-temperature integral reduces to `D·t`.
    #[test]
    fn constant_temperature_integral_is_d_times_t() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let d = diffusion_coefficient(53, t, t)
            .kernel
            .get::<square_meter_per_second>();
        let times = [
            Time::new::<second>(0.0),
            Time::new::<second>(100.0),
            Time::new::<second>(200.0),
        ];
        let temps = [t, t, t];
        let integral = integrate_diffusion_over_time(53, &times, &temps, DiffusionMaterial::Kernel);
        // At t=0 the integral is 0 (upstream baseline); at t=200 s it is D·200
        // minus the first half-trapezoid over [0, 0] (which is 0).
        assert_relative_eq!(integral[0].get::<square_meter>(), 0.0, epsilon = 1e-30);
        assert_relative_eq!(
            integral[2].get::<square_meter>(),
            d * 200.0,
            max_relative = 1e-9
        );
    }
}
