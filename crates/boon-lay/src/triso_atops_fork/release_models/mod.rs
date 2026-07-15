// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`R_B_fail` dispatcher, `release_fraction` dispatcher)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Release models — release-to-birth / release-fraction physics
//!
//! The dimensionless heart of TRISO-ATOPS: given a species, its diffusion
//! coefficient, and the reactor state, what fraction escapes the fuel? This
//! module holds the individual analytical models plus the two **group
//! dispatchers** that pick the right model for a nuclide:
//!
//! - [`steady_state`] — normal-operation (constant-temperature) models.
//! - [`transient`] — accident (time-integrated `∫D dt`) models.
//! - [`rb_fail`] — the normal-operation dispatcher (ports `R_B_fail`).
//! - [`release_fraction_transient`] — the accident dispatcher (ports
//!   `release_fraction`).
//!
//! The dispatch is by [`ElementGroup`] with an exhaustive `match` (no trait
//! objects), so a new group is a compile error until every dispatcher handles
//! it. See User Manual §3.1–3.3.

pub mod steady_state;
pub mod transient;

use uom::si::area::square_meter;
use uom::si::f64::{Area, DiffusionCoefficient, Length, Ratio, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::ratio::ratio;

use crate::triso_atops_fork::diffusion::diffusion_coefficient_sic_ag;
use crate::triso_atops_fork::nuclide_model::{nuclide_database, ElementGroup};
use crate::triso_atops_fork::{DecayConstant, ReleaseFraction};

use uom::si::frequency::hertz;

/// Nominal `<R/B>_fail` assigned to "other" fission metals (upstream `1e-5`).
pub const OTHER_METAL_RB_FAIL: f64 = 1e-5;

/// Normal-operation release-to-birth `<R/B>_fail` dispatcher.
///
/// Ports `R_B_fail(z, sl, lam, temps, t, a_grain, a_SiC, r, D)`. Selects the
/// correct steady-state release model for a nuclide from its transport
/// [`ElementGroup`]:
///
/// - **Noble gas / halogen** → empirical [`steady_state::rb_fail_noble_gases`].
/// - **Special metal** → Booth model: [`steady_state::booth_shortlived_fast_diffuse`]
///   if short-lived, else [`steady_state::booth_longlived`], both on the Booth
///   equivalent-sphere radius `a_booth = √(2·a_grain·r)`.
/// - **Silver** → [`steady_state::breakthrough_model`] through the SiC layer
///   (using the Ag-in-SiC coefficient), scaled by `√(λ_Ag-110m / λ)` exactly as
///   upstream.
/// - **Other** → the fixed nominal [`OTHER_METAL_RB_FAIL`].
///
/// # Arguments
/// - `z` — atomic number (selects the group).
/// - `short_lived` — the run-dependent short-lived flag `sl` (see
///   [`crate::triso_atops_fork::nuclide_model`] note); only affects special metals.
/// - `decay_constant` — `λ`, s^-1.
/// - `temperature` — local temperature (used by noble-gas and silver paths).
/// - `irradiation_time` — elapsed irradiation time `t`, seconds.
/// - `grain_size` — UCO fuel grain size `a_grain`, metres.
/// - `sic_thickness` — SiC layer thickness `a_SiC`, metres.
/// - `kernel_radius` — kernel radius `r`, metres.
/// - `kernel_diffusion_coefficient` — the species' kernel `D`, m^2/s (used by the
///   special-metal Booth models).
///
/// # Returns
/// The dimensionless `<R/B>_fail` as a [`ReleaseFraction`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn rb_fail(
    z: u32,
    short_lived: bool,
    decay_constant: DecayConstant,
    temperature: ThermodynamicTemperature,
    irradiation_time: Time,
    grain_size: Length,
    sic_thickness: Length,
    kernel_radius: Length,
    kernel_diffusion_coefficient: DiffusionCoefficient,
) -> ReleaseFraction {
    // Booth equivalent-sphere radius a_booth = √(a_grain · 2 · r).
    let a_grain_m = grain_size.get::<meter>();
    let r_m = kernel_radius.get::<meter>();
    let a_booth = Length::new::<meter>((a_grain_m * 2.0 * r_m).sqrt());

    match ElementGroup::from_atomic_number(z) {
        ElementGroup::NobleGas | ElementGroup::Halogen => {
            steady_state::rb_fail_noble_gases(z, decay_constant, temperature)
        }
        ElementGroup::SpecialMetal => {
            if short_lived {
                steady_state::booth_shortlived_fast_diffuse(
                    kernel_diffusion_coefficient,
                    decay_constant,
                    a_booth,
                )
            } else {
                steady_state::booth_longlived(
                    kernel_diffusion_coefficient,
                    irradiation_time,
                    a_booth,
                )
            }
        }
        ElementGroup::Silver => {
            let d_sic = diffusion_coefficient_sic_ag(temperature);
            let breakthrough = steady_state::breakthrough_model(
                d_sic,
                irradiation_time,
                sic_thickness,
                kernel_radius,
            )
            .get::<ratio>();
            // Upstream scales by √(λ_Ag-110m / λ).
            let lam_ag110m = nuclide_database::find_nuclide("Ag-110m")
                .expect("Ag-110m in database")
                .decay_constant()
                .get::<hertz>();
            let lam = decay_constant.get::<hertz>();
            Ratio::new::<ratio>(breakthrough * (lam_ag110m / lam).sqrt())
        }
        ElementGroup::Other => Ratio::new::<ratio>(OTHER_METAL_RB_FAIL),
    }
}

/// Which region's transient release fraction to compute.
///
/// Ports the `material ∈ {'kernel', 'graphite'}` argument of the Python
/// `release_fraction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseMaterial {
    /// Release from the fuel kernel (Booth, or breakthrough for silver).
    Kernel,
    /// Release from the matrix graphite.
    Graphite,
}

/// Accident (transient) release-fraction dispatcher.
///
/// Ports `release_fraction(z, fractions, integrals, a_primary, a_secondary,
/// material)`. Given the time-integrated diffusion coefficient `∫D dt`, selects
/// the transient model:
///
/// - **Kernel, non-silver** → [`transient::booth_transient`] on `∫D dt / r²`.
/// - **Kernel, silver (`z == 47`)** → [`transient::breakthrough_model_transient`]
///   using the SiC thickness as the barrier (`∫D dt / a_SiC²`).
/// - **Graphite, noble gas / halogen** → 0 (volatiles are not held up in
///   graphite in this model).
/// - **Graphite, metal** → [`transient::rf_graph`] on `∫D dt` and the graphite
///   thickness.
///
/// The upstream `fractions` argument only ever multiplies by 1 in this function
/// (the failure fractions are applied later, in the release-*activity* step, which
/// lives in the scaffolded [`crate::triso_atops_fork::activities`] module), so it
/// is intentionally omitted here.
///
/// # Arguments
/// - `z` — atomic number.
/// - `element_group` — the nuclide's transport group (only the graphite path
///   needs it, to zero out noble gases and halogens).
/// - `integrated_d` — `∫D dt` for this region (an [`Area`], m^2), from
///   [`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time`].
/// - `primary_thickness` — kernel radius `r` (kernel path) or graphite thickness
///   `a_graph` (graphite path), metres.
/// - `secondary_thickness` — SiC thickness `a_SiC`, metres; required for the
///   silver kernel path, ignored otherwise.
/// - `material` — [`ReleaseMaterial::Kernel`] or [`ReleaseMaterial::Graphite`].
///
/// # Returns
/// Release fraction in `[0, 1]` as a [`ReleaseFraction`].
///
/// # Panics
/// Panics if `material == Kernel`, `z == 47` (silver), and `secondary_thickness`
/// is `None` (the SiC barrier thickness is mandatory for the silver path).
#[must_use]
pub fn release_fraction_transient(
    z: u32,
    element_group: ElementGroup,
    integrated_d: Area,
    primary_thickness: Length,
    secondary_thickness: Option<Length>,
    material: ReleaseMaterial,
) -> ReleaseFraction {
    let int_m2 = integrated_d.get::<square_meter>();
    match material {
        ReleaseMaterial::Kernel => {
            if z != 47 {
                let a = primary_thickness.get::<meter>();
                let int_dp = Ratio::new::<ratio>(int_m2 / (a * a));
                transient::booth_transient(int_dp)
            } else {
                let a_sic =
                    secondary_thickness.expect("silver kernel release requires the SiC thickness");
                let a_sic_m = a_sic.get::<meter>();
                let int_dp = Ratio::new::<ratio>(int_m2 / (a_sic_m * a_sic_m));
                transient::breakthrough_model_transient(
                    int_dp,
                    integrated_d,
                    a_sic,
                    primary_thickness,
                )
            }
        }
        ReleaseMaterial::Graphite => match element_group {
            ElementGroup::NobleGas | ElementGroup::Halogen => Ratio::new::<ratio>(0.0),
            _ => transient::rf_graph(integrated_d, primary_thickness),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;

    fn d(v: f64) -> DiffusionCoefficient {
        DiffusionCoefficient::new::<square_meter_per_second>(v)
    }

    /// "Other" metals get the fixed nominal <R/B> = 1e-5.
    #[test]
    fn rb_fail_other_metal_is_nominal() {
        let rb = rb_fail(
            40, // Zr → Other
            false,
            DecayConstant::new::<hertz>(1e-6),
            ThermodynamicTemperature::new::<degree_celsius>(1000.0),
            Time::new::<second>(1e7),
            Length::new::<meter>(1e-5),
            Length::new::<meter>(3.5e-5),
            Length::new::<meter>(2.13e-4),
            d(1e-15),
        );
        assert_eq!(rb.get::<ratio>(), OTHER_METAL_RB_FAIL);
    }

    /// Graphite release of a noble gas / halogen is identically zero.
    #[test]
    fn graphite_release_zero_for_volatiles() {
        let rf = release_fraction_transient(
            54, // Xe → NobleGas
            ElementGroup::NobleGas,
            Area::new::<square_meter>(1e-10),
            Length::new::<meter>(4.5e-3),
            None,
            ReleaseMaterial::Graphite,
        );
        assert_eq!(rf.get::<ratio>(), 0.0);
    }

    /// Kernel accident release for a special metal routes through booth_transient.
    #[test]
    fn kernel_release_metal_saturates_for_large_integral() {
        // ∫D dt / r² large ⇒ RF → 1.
        let rf = release_fraction_transient(
            55, // Cs
            ElementGroup::SpecialMetal,
            Area::new::<square_meter>(1e-2),
            Length::new::<meter>(2.13e-4),
            None,
            ReleaseMaterial::Kernel,
        );
        assert!(rf.get::<ratio>() > 0.99);
    }
}
