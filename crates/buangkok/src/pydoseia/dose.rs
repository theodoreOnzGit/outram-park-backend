// SPDX-License-Identifier: GPL-3.0-only
//! The three pyDOSEIA dose pathways ported in the first tranche: **inhalation**,
//! **ground shine** (external dose from deposited activity) and
//! **submersion** (external dose from the cloud, semi-infinite-cloud
//! coefficients), with the deposition velocities and weathering correction
//! they use.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.inhalation_dose`,
//! `ground_shine_dose`, `submersion_dose`) and `raddcffunc.py`
//! (`RaddcfFunc.deposition_velocity_of_rad`, `apply_weathering_correction_gs`),
//! upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream takes
//! its screening deposition velocity (1000 m/d) and soil loss rates from IAEA
//! Safety Reports Series No. 19 (2001), which it cites; the few scalar values
//! used here are quoted in the code with that citation.
//!
//! # What goes in and what comes out
//!
//! Every pathway takes a dilution factor `chi/Q` (from
//! [`super::dispersion`], or from anywhere else — e.g. a `changi` calculation
//! — since it is just s/m^3), the release per nuclide, and the coefficients.
//! The pathway functions do **not** compute dispersion themselves, which is
//! how upstream's driver uses them too (it passes the per-distance maximum
//! `chi/Q` in).
//!
//! Upstream's result is mSv for an instantaneous release and **mSv per year**
//! for a long-term release (discharge in Bq/year); [`EffectiveDose`] carries
//! the mSv value either way. Research-grade only (`RESPONSIBLE_USE.md`).

use uom::si::f64::Radioactivity;
use uom::si::radioactivity::becquerel;

use super::dcf::AgeBracket;
use super::units::{DilutionFactor, EffectiveDose};

/// Seconds in upstream's year for the dose pathways: `365 * 24 * 3600`.
pub const UPSTREAM_YEAR_S: f64 = 31_536_000.0;

/// How much of a nuclide was released.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Release {
    /// An instantaneous (single-plume) release: total activity released, Bq
    /// (upstream `instantaneous_release_bq_list`). Doses are mSv.
    Instantaneous(Radioactivity),
    /// A long-term release: activity discharged **per year**, Bq/y, carried as
    /// a [`Radioactivity`] whose Bq value is the annual amount (upstream
    /// `annual_discharge_bq_rad_list`). Doses are mSv per year.
    AnnualDischarge(Radioactivity),
}

impl Release {
    /// The Bq value upstream multiplies by (total Bq, or Bq per year).
    #[must_use]
    pub fn becquerels(self) -> f64 {
        match self {
            Self::Instantaneous(a) | Self::AnnualDischarge(a) => a.get::<becquerel>(),
        }
    }
}

/// Upstream's breathing rate, m^3/s: 8400 m^3/y for `age > 1`, 1400 m^3/y for
/// `age <= 1`. `None` for a NaN age (upstream raises).
///
/// Note (upstream simplification, kept): every age above 1 year uses the
/// adult rate, so a child's inhalation dose is computed with an adult's
/// breathing rate and a child's coefficient.
#[must_use]
pub fn breathing_rate_m3_per_s(age_years: f64) -> Option<f64> {
    if age_years > 1.0 {
        Some(8400.0 / UPSTREAM_YEAR_S)
    } else if age_years <= 1.0 {
        Some(1400.0 / UPSTREAM_YEAR_S)
    } else {
        None
    }
}

/// Inhalation dose for one nuclide:
/// `chi/Q * Q * DCF_inh * breathing rate * 1000` (mSv, or mSv/y).
///
/// `dcf_sv_per_bq` comes from [`super::dcf::InhalationDcfTable::lookup`] (NaN
/// propagates, as upstream). Returns `None` only for a NaN age.
#[must_use]
pub fn inhalation_dose(
    chi_over_q: DilutionFactor,
    release: Release,
    dcf_sv_per_bq: f64,
    age_years: f64,
) -> Option<EffectiveDose> {
    // Refactored 2026-09-29 (not an upstream change): the coefficient product
    // is `inhalation_committed_dose_rate_msv_per_s`, fed the time-integrated
    // concentration `chi/Q * Q` [Bq s m^-3]. The multiplication order is
    // upstream's, `(((chi/Q * Q) * DCF) * br) * 1000`, so the result is
    // bit-identical (checked by the code-to-code fixture).
    let time_integrated = chi_over_q.seconds_per_cubic_meter() * release.becquerels();
    Some(EffectiveDose::from_millisieverts(
        inhalation_committed_dose_rate_msv_per_s(time_integrated, dcf_sv_per_bq, age_years)?,
    ))
}

/// The inhalation pathway's coefficient product,
/// `C * DCF_inh * breathing rate * 1000`, which [`inhalation_dose`] is built on.
///
/// - Given an **instantaneous** air concentration `C` \[Bq/m^3\], the result is
///   the **committed** effective dose per second of breathing \[mSv/s\]: the
///   dose committed by one second's intake, not a dose received in that
///   second. It is what an "inhalation dose rate" means on a map, and it must
///   be labelled that way.
/// - Given a **time-integrated** concentration \[Bq s/m^3\] it is the
///   committed dose \[mSv\], which is how [`inhalation_dose`] uses it.
///
/// Returns `None` only for a NaN age. Not an upstream function: a 2026-09-29
/// refactor so a dose-rate caller (`htgr_sim_v1`'s map) and the ported dose
/// share one formula. Research-grade only (`RESPONSIBLE_USE.md`).
#[must_use]
pub fn inhalation_committed_dose_rate_msv_per_s(
    air_concentration_bq_per_m3: f64,
    dcf_sv_per_bq: f64,
    age_years: f64,
) -> Option<f64> {
    let br = breathing_rate_m3_per_s(age_years)?;
    Some(air_concentration_bq_per_m3 * dcf_sv_per_bq * br * 1000.0)
}

/// Upstream's total (dry + wet) deposition velocity by **element symbol**,
/// m/s (`deposition_velocity_of_rad`, citing IAEA SRS 19 p. 27):
///
/// - `0` for H, C and the noble gases He, Ne, Ar, Kr, Xe, Rn;
/// - `0.1` for F, Cl, Br;
/// - `1000 m/d = 1000 / 86400 m/s` for everything else (SRS 19's screening
///   value for aerosols and reactive gases).
///
/// Note (upstream behaviour, kept): iodine gets the aerosol value, not the
/// reactive-halogen 0.1 m/s that F, Cl and Br get.
#[must_use]
pub fn deposition_velocity_m_per_s(element: &str) -> f64 {
    match element {
        "H" | "C" | "He" | "Ne" | "Ar" | "Kr" | "Xe" | "Rn" => 0.0,
        "F" | "Cl" | "Br" => 0.1,
        _ => 1000.0 / 86400.0,
    }
}

/// Whether to add the environmental (soil) loss rate to radioactive decay in
/// the ground-shine build-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weathering {
    /// Radioactive decay only (upstream `weathering_corr: False`).
    Off,
    /// Add upstream's soil loss rates (`weathering_corr: True`): 0.0014 /d for
    /// Tc, Cl, I; 0.00014 /d for Cs, Sr; 0 otherwise (SRS 19 Table X, as
    /// upstream cites it).
    SoilLossRates,
}

/// Upstream's effective build-up time on the ground, s:
/// `(1 - exp(-lambda_e T)) / lambda_e`, with `lambda_e` the decay constant
/// plus the weathering rate and `T` the exposure period (years * 365 d).
///
/// Multiplying a deposition rate (Bq m^-2 s^-1) by this gives the ground
/// concentration at the end of `T` (Bq/m^2). A stable nuclide (`lambda = 0`,
/// weathering off) gives NaN, as upstream.
#[must_use]
pub fn effective_buildup_time_s(
    decay_constant_per_s: f64,
    element: &str,
    weathering: Weathering,
    exposure_period_years: f64,
) -> f64 {
    let exposure_period = exposure_period_years * 365.0 * 24.0 * 3600.0;
    let lr_effective = match weathering {
        Weathering::Off => decay_constant_per_s,
        Weathering::SoilLossRates => {
            let w = match element {
                "He" | "Ne" | "Ar" | "Kr" | "Xe" | "Rn" => 0.0,
                "Tc" | "Cl" | "I" => 0.0014 / (24.0 * 3600.0),
                "Cs" | "Sr" => 0.00014 / (24.0 * 3600.0),
                _ => 0.0,
            };
            decay_constant_per_s + w
        }
    };
    (1.0 - (-lr_effective * exposure_period).exp()) / lr_effective
}

/// Ground-shine dose for one nuclide, upstream's arithmetic:
///
/// ```text
/// deposition rate = chi/Q * (Q / year_s) * v_d          [Bq m^-2 s^-1]
/// ground conc.    = deposition rate * build-up time      [Bq m^-2]
/// dose            = ground conc. * DCF_gs * 1000 * year_s  [mSv (per year)]
/// ```
///
/// `dcf_gs` (Sv m^2 Bq^-1 s^-1) from [`super::dcf::external_dcf`] with
/// [`super::dcf::ExternalDcfPair::selected`].
///
/// Note (upstream behaviour, kept): the same formula is used for an
/// instantaneous release, where `Q` is total Bq; it then amounts to a
/// constant deposition rate `Q / year` held for the exposure period and an
/// exposure of one year at the resulting concentration.
#[must_use]
pub fn ground_shine_dose(
    chi_over_q: DilutionFactor,
    release: Release,
    deposition_velocity_m_per_s: f64,
    effective_buildup_time_s: f64,
    dcf_gs: f64,
) -> EffectiveDose {
    let deposition_rate = chi_over_q.seconds_per_cubic_meter()
        * (release.becquerels() / UPSTREAM_YEAR_S)
        * deposition_velocity_m_per_s;
    let conc_rad_ground = deposition_rate * effective_buildup_time_s;
    // Refactored 2026-09-29 (not an upstream change): `conc * DCF * 1000` is
    // `ground_shine_dose_rate_msv_per_s`, in upstream's order, so the result
    // is bit-identical.
    let gs_dose = ground_shine_dose_rate_msv_per_s(conc_rad_ground, dcf_gs);
    EffectiveDose::from_millisieverts(gs_dose * UPSTREAM_YEAR_S)
}

/// The ground-shine coefficient product `A * DCF_gs * 1000`: the effective
/// dose **rate** \[mSv/s\] from a ground-surface concentration `A`
/// \[Bq/m^2\] and a ground-surface dose-rate coefficient
/// \[Sv m^2 Bq^-1 s^-1\] (FGR-15 Table 4-1 is one).
///
/// [`ground_shine_dose`] is built on it (its `gs_dose` step). Not an upstream
/// function: a 2026-09-29 refactor so a dose-rate caller and the ported dose
/// share one formula. Research-grade only (`RESPONSIBLE_USE.md`).
#[must_use]
pub fn ground_shine_dose_rate_msv_per_s(ground_bq_per_m2: f64, dcf_gs: f64) -> f64 {
    ground_bq_per_m2 * dcf_gs * 1000.0
}

/// Submersion dose for one nuclide: `chi/Q * Q * DCF_sub * 1000` (mSv, or
/// mSv/y). `dcf_sub` (Sv m^3 Bq^-1 s^-1) from [`super::dcf::external_dcf`].
#[must_use]
pub fn submersion_dose(
    chi_over_q: DilutionFactor,
    release: Release,
    dcf_sub: f64,
) -> EffectiveDose {
    // Refactored 2026-09-29 (not an upstream change): the coefficient product
    // is `submersion_dose_rate_msv_per_s`, fed the time-integrated
    // concentration, in upstream's multiplication order -- bit-identical.
    EffectiveDose::from_millisieverts(submersion_dose_rate_msv_per_s(
        chi_over_q.seconds_per_cubic_meter() * release.becquerels(),
        dcf_sub,
    ))
}

/// The submersion coefficient product `C * DCF_sub * 1000`: given an
/// **instantaneous** air concentration `C` \[Bq/m^3\] and an air-submersion
/// dose-rate coefficient \[Sv m^3 Bq^-1 s^-1\] (FGR-15 Table 4-6 is one), the
/// effective dose **rate** \[mSv/s\]; given a time-integrated concentration
/// \[Bq s/m^3\], the dose \[mSv\], which is how [`submersion_dose`] uses it.
///
/// **Semi-infinite cloud.** The coefficient assumes the receptor stands in a
/// uniform cloud of concentration `C` extending far beyond a photon mean free
/// path (~100 m in air at 1 MeV). For a narrow plume that over-states the dose
/// on the centreline; beneath an elevated plume that has not yet reached the
/// ground it under-states it. The finite-cloud alternative is
/// [`super::plume_shine`].
///
/// Not an upstream function: a 2026-09-29 refactor so a dose-rate caller and
/// the ported dose share one formula. Research-grade only.
#[must_use]
pub fn submersion_dose_rate_msv_per_s(air_concentration_bq_per_m3: f64, dcf_sub: f64) -> f64 {
    air_concentration_bq_per_m3 * dcf_sub * 1000.0
}

/// The age bracket a pathway would use for its coefficient, re-exported here
/// for callers that assemble doses by hand.
#[must_use]
pub fn age_bracket(age_years: f64) -> Option<AgeBracket> {
    AgeBracket::from_age_years(age_years)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noble_gases_give_no_ground_shine() {
        let d = ground_shine_dose(
            DilutionFactor::new(1e-5),
            Release::AnnualDischarge(Radioactivity::new::<becquerel>(1e12)),
            deposition_velocity_m_per_s("Kr"),
            1e7,
            1e-16,
        );
        assert_eq!(d.millisieverts(), 0.0);
    }

    #[test]
    fn infant_breathing_rate_is_lower() {
        assert!(breathing_rate_m3_per_s(1.0).unwrap() < breathing_rate_m3_per_s(1.01).unwrap());
    }
}
