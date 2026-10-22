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

use crate::triso_atops_fork::diffusion::{diffusion_coefficient_sic_ag, diffusion_coefficient_sic_ag_for};
use crate::triso_atops_fork::variant::{
    booth_radius_m, breakthrough_release, slab_release, sphere_release, AccidentBoothRadius,
    LongLivedVolatileRelease, OtherMetalRelease, PaperReadings, SeriesEvaluation,
    TrisoAtopsVariant,
};
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::time::second;
use crate::triso_atops_fork::nuclide_model::{nuclide_database, ElementGroup};
use crate::triso_atops_fork::{DecayConstant, ReleaseFraction};

use uom::si::frequency::hertz;

/// Nominal `<R/B>_fail` assigned to "other" fission metals (upstream `1e-5`).
pub const OTHER_METAL_RB_FAIL: f64 = 1e-5;

/// Normal-operation release-to-birth `<R/B>_fail` dispatcher.
///
/// Ports `R_B_fail(z, sl, lam, temps, t, a_grain, a_SiC, r, D)`. Selects the
/// correct steady-state release model for a nuclide from its transport
/// [`ElementGroup`]. **Derivation:** step 6(i) and step 2 (crate-root
/// `TRISO_ATOPS_DERIVATION.md`) — the group dispatch, and the Booth
/// equivalent-sphere radius `a_booth = √(2·a_grain·r)`.
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

/// [`rb_fail`] under a model [`TrisoAtopsVariant`] (gh:#1015).
///
/// [`TrisoAtopsVariant::Upstream`] (readings
/// [`PaperReadings::UPSTREAM_CODE`]) calls [`rb_fail`] itself. Otherwise, per
/// Stoyer, Raichart & Petti (2026), *Nuclear Technology*,
/// doi:10.1080/00295450.2026.2671624:
///
/// - **(a)** the Booth sphere radius `a` is [`booth_radius_m`] of the
///   [`BoothRadius`] reading (paper: RMS of kernel radius and grain size,
///   pp. 7-8 and p. 26);
/// - **(e)** Eq. (6c) and Eq. (6d) are summed to convergence
///   ([`crate::triso_atops_fork::variant::sphere_release`],
///   [`crate::triso_atops_fork::variant::breakthrough_release`]);
/// - **(f)** "other" metals use Booth Eq. (6b)/(6c) on `D = 1e-19 m²/s`
///   (pp. 6-7, p. 9) instead of the fixed `1e-5`;
/// - **(h)** long-lived noble gases, halogens, Te and Se use Eq. (6c) on
///   krypton's kernel diffusivity (p. 5, p. 9); short-lived ones keep the
///   empirical Eq. (6a) (p. 6);
/// - silver keeps Eq. (6d) with the `√(λ_Ag-110m/λ)` factor (p. 8), on the
///   SiC law clamped per (d).
///
/// `kernel_diffusion_coefficient` should come from
/// [`crate::triso_atops_fork::diffusion::diffusion_coefficient_for`] with the
/// same variant.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn rb_fail_for(
    variant: TrisoAtopsVariant,
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
    let r = variant.readings();
    if r == PaperReadings::UPSTREAM_CODE {
        return rb_fail(
            z,
            short_lived,
            decay_constant,
            temperature,
            irradiation_time,
            grain_size,
            sic_thickness,
            kernel_radius,
            kernel_diffusion_coefficient,
        );
    }
    let a_m = booth_radius_m(r.booth_radius, grain_size, kernel_radius);
    let a = Length::new::<meter>(a_m);
    let t_s = irradiation_time.get::<second>();
    // Booth Eq. (6b) or (6c) for a kernel D, with the series reading.
    let booth = |d: DiffusionCoefficient| -> ReleaseFraction {
        if short_lived {
            steady_state::booth_shortlived_fast_diffuse(d, decay_constant, a)
        } else {
            match r.series {
                SeriesEvaluation::UpstreamTruncated => {
                    steady_state::booth_longlived(d, irradiation_time, a)
                }
                SeriesEvaluation::Converged => {
                    let tau = d.get::<square_meter_per_second>() * t_s / (a_m * a_m);
                    Ratio::new::<ratio>(sphere_release(tau))
                }
            }
        }
    };
    match ElementGroup::from_atomic_number_for(z, variant) {
        ElementGroup::NobleGas | ElementGroup::Halogen => {
            if short_lived
                || r.long_lived_volatiles == LongLivedVolatileRelease::UpstreamEmpirical
            {
                steady_state::rb_fail_noble_gases(z, decay_constant, temperature)
            } else {
                booth(kernel_diffusion_coefficient)
            }
        }
        ElementGroup::SpecialMetal => booth(kernel_diffusion_coefficient),
        ElementGroup::Silver => {
            let d_sic = diffusion_coefficient_sic_ag_for(variant, temperature);
            let breakthrough = match r.series {
                SeriesEvaluation::UpstreamTruncated => steady_state::breakthrough_model(
                    d_sic,
                    irradiation_time,
                    sic_thickness,
                    kernel_radius,
                )
                .get::<ratio>(),
                SeriesEvaluation::Converged => {
                    let a_sic = sic_thickness.get::<meter>();
                    let tau = d_sic.get::<square_meter_per_second>() * t_s / (a_sic * a_sic);
                    breakthrough_release(tau, a_sic, kernel_radius.get::<meter>())
                }
            };
            let lam_ag110m = nuclide_database::find_nuclide("Ag-110m")
                .expect("Ag-110m in database")
                .decay_constant()
                .get::<hertz>();
            let lam = decay_constant.get::<hertz>();
            Ratio::new::<ratio>(breakthrough * (lam_ag110m / lam).sqrt())
        }
        ElementGroup::Other => match r.other_metals {
            OtherMetalRelease::UpstreamFixedRb => Ratio::new::<ratio>(OTHER_METAL_RB_FAIL),
            OtherMetalRelease::BoothAtNominalDiffusivity => booth(kernel_diffusion_coefficient),
        },
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

/// [`release_fraction_transient`] under a model [`TrisoAtopsVariant`]
/// (gh:#1015).
///
/// Same arguments plus `grain_size` (for the paper's Booth radius). Upstream
/// readings call [`release_fraction_transient`] itself. Otherwise, per Stoyer,
/// Raichart & Petti (2026), doi:10.1080/00295450.2026.2671624:
///
/// - **(b)** the kernel Booth Eq. (19) uses `D' = D/a²` with the same `a` as
///   normal operation (pp. 8, 11-12) under
///   [`AccidentBoothRadius::SameAsNormalOperation`], instead of the kernel
///   radius `primary_thickness`;
/// - **(e)** Eqs. (18), (19) and (21) are summed to convergence, with no
///   `1e-6` floor;
/// - **(g)** palladium takes silver's breakthrough Eq. (18) (Table 1, p. 6).
///
/// `integrated_d` should come from
/// [`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time_for`]
/// with the same variant.
///
/// # Panics
/// Panics on the silver kernel path if `secondary_thickness` is `None`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn release_fraction_transient_for(
    variant: TrisoAtopsVariant,
    z: u32,
    element_group: ElementGroup,
    integrated_d: Area,
    primary_thickness: Length,
    secondary_thickness: Option<Length>,
    material: ReleaseMaterial,
    grain_size: Length,
) -> ReleaseFraction {
    let r = variant.readings();
    if r == PaperReadings::UPSTREAM_CODE {
        return release_fraction_transient(
            z,
            element_group,
            integrated_d,
            primary_thickness,
            secondary_thickness,
            material,
        );
    }
    let int_m2 = integrated_d.get::<square_meter>();
    let silver = z == 47 || (z == 46 && r.palladium_as_silver());
    match material {
        ReleaseMaterial::Kernel if silver => {
            let a_sic =
                secondary_thickness.expect("silver kernel release requires the SiC thickness");
            let a_sic_m = a_sic.get::<meter>();
            match r.series {
                SeriesEvaluation::UpstreamTruncated => transient::breakthrough_model_transient(
                    Ratio::new::<ratio>(int_m2 / (a_sic_m * a_sic_m)),
                    integrated_d,
                    a_sic,
                    primary_thickness,
                ),
                SeriesEvaluation::Converged => Ratio::new::<ratio>(breakthrough_release(
                    int_m2 / (a_sic_m * a_sic_m),
                    a_sic_m,
                    primary_thickness.get::<meter>(),
                )),
            }
        }
        ReleaseMaterial::Kernel => {
            let a = match r.accident_booth_radius {
                AccidentBoothRadius::KernelRadius => primary_thickness.get::<meter>(),
                AccidentBoothRadius::SameAsNormalOperation => {
                    booth_radius_m(r.booth_radius, grain_size, primary_thickness)
                }
            };
            let tau = int_m2 / (a * a);
            match r.series {
                SeriesEvaluation::UpstreamTruncated => {
                    transient::booth_transient(Ratio::new::<ratio>(tau))
                }
                SeriesEvaluation::Converged => Ratio::new::<ratio>(sphere_release(tau)),
            }
        }
        ReleaseMaterial::Graphite => match element_group {
            ElementGroup::NobleGas | ElementGroup::Halogen => Ratio::new::<ratio>(0.0),
            _ => match r.series {
                SeriesEvaluation::UpstreamTruncated => {
                    transient::rf_graph(integrated_d, primary_thickness)
                }
                SeriesEvaluation::Converged => {
                    let a = primary_thickness.get::<meter>();
                    Ratio::new::<ratio>(slab_release(int_m2 / (a * a)))
                }
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::mathf::RealMath;
    use std::f64::consts::PI;
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

    const G: f64 = 1.0e-5; // Table 3 grain size, m
    const R: f64 = 2.13e-4; // Table 3 kernel radius, m
    const A_SIC: f64 = 3.5e-5; // Table 3 SiC thickness, m

    fn rb(variant: TrisoAtopsVariant, z: u32, sl: bool, lam: f64, d: f64, t_s: f64) -> f64 {
        rb_fail_for(
            variant,
            z,
            sl,
            DecayConstant::new::<hertz>(lam),
            ThermodynamicTemperature::new::<degree_celsius>(1000.0),
            Time::new::<second>(t_s),
            Length::new::<meter>(G),
            Length::new::<meter>(A_SIC),
            Length::new::<meter>(R),
            super::DiffusionCoefficient::new::<square_meter_per_second>(d),
        )
        .get::<ratio>()
    }

    /// gh:#1015 (a): the paper's Booth radius against hand values of Eqs.
    /// (6b) and (6c), and the predicted ratio to upstream.
    ///
    /// Methodology: Sr-90-like long-lived (`D = 1e-20 m²/s`, `t = 9.46e7 s`)
    /// and short-lived (`λ = 1e-5 /s`, `D = 1e-16 m²/s`) special metals at
    /// Table 3's geometry. Paper `a = √((r² + a_grain²)/2) = 150.78 µm`.
    /// Hand values: Eq. (6c) short-time `6√(D't/π) − 3D't` (exact to 1e-15
    /// at `D't ≈ 4e-5`), Eq. (6b) `(3/μ)(coth μ − 1/μ)`, `μ = a√(λ/D)`.
    ///
    /// Prediction (written before the run): paper release smaller by about
    /// `a_paper / 65.27 µm = 2.31` in both forms (`√(D't) ∝ 1/a`,
    /// `3/μ ∝ 1/a`). Result (2026-10-22): ratios printed by the test; gate
    /// 2.31 ± 3 %.
    #[test]
    fn paper_booth_radius_matches_hand_values() {
        let paper = TrisoAtopsVariant::ATOPS2026PAPER;
        let a: f64 = ((R * R + G * G) / 2.0).sqrt();
        // Long-lived, Eq. (6c).
        let (d, t) = (1e-20, 9.46e7);
        let tau = d * t / (a * a);
        let hand = 6.0 * (tau / PI).sqrt() - 3.0 * tau;
        let got = rb(paper, 38, false, 1e-9, d, t);
        assert!(((got - hand) / hand).abs() < 1e-12, "{got:e} {hand:e}");
        let up = rb(TrisoAtopsVariant::Upstream, 38, false, 1e-9, d, t);
        // Short-lived, Eq. (6b).
        let (lam, d2): (f64, f64) = (1e-5, 1e-16);
        let mu = a * (lam / d2).sqrt();
        let hand_s = 3.0 / mu * (1.0 / mu.r_tanh() - 1.0 / mu);
        let got_s = rb(paper, 38, true, lam, d2, t);
        assert!(((got_s - hand_s) / hand_s).abs() < 1e-12);
        let up_s = rb(TrisoAtopsVariant::Upstream, 38, true, lam, d2, t);
        let (r_long, r_short) = (up / got, up_s / got_s);
        println!("upstream / paper: Eq. (6c) {r_long:.4}, Eq. (6b) {r_short:.4} (predicted 2.31)");
        assert!((r_short / 2.31 - 1.0).abs() < 0.03);
        // Upstream's Eq. (6c) sits on its 4999-term floor here, so only the
        // direction is gated for the long-lived form.
        assert!(r_long > 2.0);
    }

    /// gh:#1015 (f) and (h): "other" metals and long-lived volatiles.
    ///
    /// Methodology: Zr-95-like "other" metal (`λ = ln2/5.53e6 s`), paper Eq.
    /// (6b) on `D = 1e-19` at `a = 150.78 µm`, against upstream's `1e-5`. And
    /// a Kr-85-like long-lived noble gas: paper Eq. (6c) on the Kr kernel law
    /// at 1000 °C, against upstream's Eq. (6a).
    ///
    /// Prediction: Zr-95 `<R/B>_fail` rises from 1e-5 to `3/μ ≈ 1.8e-2`;
    /// Kr-85 moves from the Eq. (6a) value to the Booth value (either way;
    /// computed, not gated). Result (2026-10-22): printed by the test.
    #[test]
    fn paper_other_metals_and_long_lived_volatiles() {
        let paper = TrisoAtopsVariant::ATOPS2026PAPER;
        let a: f64 = ((R * R + G * G) / 2.0).sqrt();
        let lam = std::f64::consts::LN_2 / 5.53e6;
        let mu = a * (lam / 1e-19_f64).sqrt();
        let hand = 3.0 / mu * (1.0 / mu.r_tanh() - 1.0 / mu);
        let got = rb(paper, 40, true, lam, 1e-19, 9.46e7);
        assert!(((got - hand) / hand).abs() < 1e-12);
        assert_eq!(rb(TrisoAtopsVariant::Upstream, 40, true, lam, 1e-19, 9.46e7), 1e-5);
        println!("Zr-95 <R/B>_fail: upstream 1e-5, paper {got:.4e}");

        let t1000 = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let d_kr = crate::triso_atops_fork::diffusion::diffusion_coefficient_for(paper, 36, t1000, t1000)
            .kernel
            .get::<square_meter_per_second>();
        let lam85 = std::f64::consts::LN_2 / 3.39e8;
        let tau = d_kr * 9.46e7 / (a * a);
        let got85 = rb(paper, 36, false, lam85, d_kr, 9.46e7);
        assert!((got85 - sphere_release(tau)).abs() < 1e-15);
        let up85 = rb(TrisoAtopsVariant::Upstream, 36, false, lam85, d_kr, 9.46e7);
        println!("Kr-85 <R/B>_fail at 1000 degC: upstream Eq. (6a) {up85:.4e}, paper Eq. (6c) {got85:.4e}");
        // Short-lived volatiles are unchanged.
        let lam88 = std::f64::consts::LN_2 / 1.02e4;
        assert_eq!(
            rb(paper, 36, true, lam88, d_kr, 9.46e7),
            rb(TrisoAtopsVariant::Upstream, 36, true, lam88, d_kr, 9.46e7)
        );
    }

    /// gh:#1015 (b): the accident kernel release uses `D' = D/a²` with the
    /// paper's `a`, Eq. (19).
    ///
    /// Methodology: `∫D dt = 1e-12 m²` (Booth short-time regime at both
    /// radii). Hand value: `6√(τ/π) − 3τ`, `τ = ∫D dt / a²`.
    /// Prediction: paper / upstream ≈ `r/a = 213/150.78 = 1.41` (√-regime).
    /// Result (2026-10-22): ratio printed; gated at 1.41 ± 3 %.
    #[test]
    fn paper_accident_booth_radius() {
        let paper = TrisoAtopsVariant::ATOPS2026PAPER;
        let int_d = Area::new::<square_meter>(1e-12);
        let f = |v| {
            release_fraction_transient_for(
                v,
                55,
                ElementGroup::SpecialMetal,
                int_d,
                Length::new::<meter>(R),
                None,
                ReleaseMaterial::Kernel,
                Length::new::<meter>(G),
            )
            .get::<ratio>()
        };
        let a: f64 = ((R * R + G * G) / 2.0).sqrt();
        let tau = 1e-12 / (a * a);
        let hand = 6.0 * (tau / PI).sqrt() - 3.0 * tau;
        assert!(((f(paper) - hand) / hand).abs() < 1e-12);
        assert_eq!(
            f(TrisoAtopsVariant::Upstream),
            release_fraction_transient(
                55,
                ElementGroup::SpecialMetal,
                int_d,
                Length::new::<meter>(R),
                None,
                ReleaseMaterial::Kernel
            )
            .get::<ratio>()
        );
        let ratio_pu = f(paper) / f(TrisoAtopsVariant::Upstream);
        println!("accident Booth release paper / upstream = {ratio_pu:.4} (predicted 1.41)");
        assert!((ratio_pu / 1.41 - 1.0).abs() < 0.03);
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
