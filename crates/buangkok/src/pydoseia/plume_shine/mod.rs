// SPDX-License-Identifier: GPL-3.0-only
//! Plume shine: the external gamma dose from the passing cloud, by pyDOSEIA's
//! finite-cloud model: the Gaussian plume concentration folded with a
//! point-kernel photon flux (exponential attenuation, linear build-up
//! `1 + k mu r`) and integrated over a box around the receptor.
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Never a
//! > dose to a real person.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.plumeshine_dose`, with its
//! nested `adgq_single_plume`, `adgq_sector_average` and
//! `get_all_integral_stab_cat_energy_wise_for_all_rad_parallel`) and
//! `raddcffunc.py` (`gamma_energy_abundaces`, `add_zero_energy_for_pure_beta`,
//! `atten_coeff` — the second definition, which is the one Python binds —
//! `get_k_mu_mua_MFP`, `zyx_lim_for_integral`,
//! `zyx_lim_for_integral_single_plume`,
//! `zyx_lim_for_integral_sector_averaged_plume`,
//! `get_limit_lists_per_rad_for_all_energies`, and the module-level
//! `point_source_dose`), upstream <https://github.com/BiswajitSadhu/pyDOSEIA>
//! at commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024
//! Dr. Biswajit Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//! Upstream cites Wang, Ling and Shi, *Nucl. Eng. Des.* 231 (2004) 211-216
//! for the mean-free-path integration limits.
//!
//! The triple integral uses [`super::quadpack::tplquad`], a port of the SciPy
//! QUADPACK routine upstream calls, so the port reproduces upstream's
//! adaptive subdivision and not just its integrand.
//!
//! # No photon data ships with this crate
//!
//! | Upstream sheet | Content | Source as upstream states | Here |
//! |---|---|---|---|
//! | `Dose_ecerman_final.xlsx` / `gamma_energy_radionuclide` | gamma energies and emission probabilities | IAEA "Update of X-ray and gamma-ray decay data standards" (2007), `www-nds.iaea.org/xgamma_standards` | **not copied** (IAEA terms of use not established); read your own table with [`GammaLineTable::from_csv`] |
//! | `Dose_ecerman_final.xlsx` / `mass_attenuation_coeff` | mass attenuation and mass energy-absorption coefficients of air | NIST (Hubbell and Seltzer), `physics.nist.gov/PhysRefData/XrayMassCoef/ComTab/air.html` | **not copied**: NIST Standard Reference Data may carry copyright under the Standard Reference Data Act (15 U.S.C. 290e), and the terms of this table were not established; read your own with [`AttenuationTable::from_csv`] |
//!
//! The code-to-code test uses **synthetic** tables in these layouts.
//!
//! # What the numbers mean (and an upstream unit ambiguity)
//!
//! Upstream multiplies each line's integral by `5e-4 * E * mu_a * yield`, sums
//! the lines, and multiplies by the release (Bq/s for a long-term release,
//! `annual / 31 536 000`; Bq for a single plume). Its comments call the
//! result **microSv/h**, while its text report heads the met-data table
//! **"microSv per year"** and the met-data branch sums the frequency table's
//! raw **hour counts** without dividing by the hours of data (defect D16 in
//! `docs/pydoseia-code-to-code.md`). The port reproduces the numbers and does
//! not assign them a unit type: they are plain `f64` "upstream plume-shine
//! values".

mod integral;
mod tables;

pub use integral::{
    integration_limits_legacy, integration_limits_sector_averaged, integration_limits_single_plume,
    kernel_sector_averaged, kernel_single_plume, line_integral, PlumeShineGeometry, PlumeShineMode,
    SECTOR_AVERAGED_EPS, SINGLE_PLUME_EPS,
};
pub use tables::{
    numpy_interp, AirPhotonCoefficients, AttenuationTable, GammaLine, GammaLineTable,
    NuclideGammaLines, AIR_DENSITY_G_PER_CM3,
};

use super::dispersion::{height_correction_factor, StabilityClass};
use super::met::{MetClimatology, SECTOR_COUNT, SPEED_CLASS_COUNT, WSPEED_K_KMPH};
use uom::si::f64::Length;

/// Upstream's plume-shine prefactor `5 * 10 ** (-4)`, evaluated as Python
/// does (`10 ** -4` is `pow(10.0, -4.0)`).
#[must_use]
pub fn prefactor() -> f64 {
    5.0 * 10f64.powf(-4.0)
}

/// The per-class integrals of one gamma line, `[A..F]` (upstream
/// `all_integral_stab_cat_energy_wise[rad][line]`). A zero-energy placeholder
/// line (pure beta emitter) is not integrated and gives zeros: upstream does
/// integrate it, and multiplies the result by the zero energy and yield.
#[must_use]
pub fn line_integrals(
    line: GammaLine,
    table: &AttenuationTable,
    geometry: PlumeShineGeometry,
) -> [f64; 6] {
    let mut out = [0.0; 6];
    if line.energy_mev == 0.0 {
        return out;
    }
    let c = table.air_coefficients(line.energy_mev);
    for s in StabilityClass::ALL {
        out[s.index()] = line_integral(s, c, geometry);
    }
    out
}

/// Plume shine per stability class for **unit release**, summed over the
/// lines (upstream's `pl_sh_sectors` before the release multiplication, for
/// the single-plume and the long-term no-met branches).
///
/// `lines` are what [`GammaLineTable::plume_shine_lines`] returns (upstream
/// `gamma_energy_abundaces` + `add_zero_energy_for_pure_beta`).
#[must_use]
pub fn per_class_unit_release(
    lines: &[GammaLine],
    table: &AttenuationTable,
    geometry: PlumeShineGeometry,
) -> [f64; 6] {
    let mut sum: Option<[f64; 6]> = None;
    for line in lines {
        let integrals = line_integrals(*line, table, geometry);
        let mu_a = table.air_coefficients(line.energy_mev).mu_a_per_m;
        let mut row = [0.0; 6];
        for i in 0..6 {
            row[i] =
                prefactor() * line.energy_mev * mu_a * integrals[i] * line.emission_probability;
        }
        sum = Some(match sum {
            None => row,
            Some(mut s) => {
                for i in 0..6 {
                    s[i] = s[i] + row[i];
                }
                s
            }
        });
    }
    sum.unwrap_or([0.0; 6])
}

/// The release multiplier upstream applies at the end: Bq/s
/// (`annual / 31 536 000`) for a long-term release, Bq for a single plume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlumeShineRelease {
    /// Long-term release: activity discharged per year, Bq/y.
    AnnualDischargeBq(f64),
    /// Instantaneous release: activity released, Bq.
    InstantaneousBq(f64),
}

impl PlumeShineRelease {
    /// The factor upstream multiplies by.
    #[must_use]
    pub fn multiplier(self) -> f64 {
        match self {
            Self::AnnualDischargeBq(q) => q / 31_536_000.0,
            Self::InstantaneousBq(q) => q,
        }
    }
}

/// Plume shine per stability class for one nuclide (single plume, or long
/// term without met data): [`per_class_unit_release`] times the release.
#[must_use]
pub fn per_class(
    lines: &[GammaLine],
    table: &AttenuationTable,
    geometry: PlumeShineGeometry,
    release: PlumeShineRelease,
) -> [f64; 6] {
    per_class_unit_release(lines, table, geometry).map(|v| v * release.multiplier())
}

/// Plume shine per 22.5-degree sector for one nuclide, long-term release
/// **with met data**, averaged over the years of `met` and multiplied by the
/// release (upstream's `have_met_data` branch).
///
/// Per year and line, the per-class integrals (times
/// `5e-4 * E * mu_a * yield`) are weighted by the missing-corrected TJFD
/// count divided by the speed-class wind speed (m/s) and the class's height
/// correction factor, and summed over the nine non-calm speed classes and the
/// six classes. Lines with zero energy or yield contribute zero.
///
/// Note (upstream behaviour, kept; defect D16): the counts are **not**
/// divided by the number of hours in the year, so the result grows with the
/// length of the met record; upstream computes `hours_without_calm` and never
/// uses it.
#[must_use]
pub fn per_sector_with_met(
    lines: &[GammaLine],
    table: &AttenuationTable,
    geometry: PlumeShineGeometry,
    met: &MetClimatology,
    measurement_height: Length,
    release: PlumeShineRelease,
) -> [f64; SECTOR_COUNT] {
    let wspeed: [f64; SPEED_CLASS_COUNT] = WSPEED_K_KMPH.map(|s| s / 3.6);
    let hf: [f64; 6] = StabilityClass::ALL
        .map(|s| height_correction_factor(s, geometry.release_height, measurement_height));
    let line_ints: Vec<[f64; 6]> = lines
        .iter()
        .map(|l| line_integrals(*l, table, geometry))
        .collect();
    let mut years_sum = [0.0; SECTOR_COUNT];
    for (yi, year) in met.years.iter().enumerate() {
        let tjfd = &year.missing_corrected;
        let mut energy_sum: Option<[f64; SECTOR_COUNT]> = None;
        for (line, ints) in lines.iter().zip(&line_ints) {
            let mut ps_dir = [0.0; SECTOR_COUNT];
            if line.energy_mev > 0.0 && line.emission_probability > 0.0 {
                let mu_a = table.air_coefficients(line.energy_mev).mu_a_per_m;
                let ps_factors = prefactor() * line.energy_mev * mu_a * line.emission_probability;
                for (l, out) in ps_dir.iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for j in 0..6 {
                        let inte = ints[j] * ps_factors;
                        for k in 1..SPEED_CLASS_COUNT {
                            let abc = tjfd[j][k][l] as f64 * (1.0 / wspeed[k]);
                            let habc = abc * (1.0 / hf[j]);
                            acc += habc * inte;
                        }
                    }
                    *out = acc;
                }
            }
            energy_sum = Some(match energy_sum {
                None => ps_dir,
                Some(mut s) => {
                    for l in 0..SECTOR_COUNT {
                        s[l] = s[l] + ps_dir[l];
                    }
                    s
                }
            });
        }
        let e = energy_sum.unwrap_or([0.0; SECTOR_COUNT]);
        for l in 0..SECTOR_COUNT {
            years_sum[l] = if yi == 0 { e[l] } else { years_sum[l] + e[l] };
        }
    }
    let n = met.years.len() as f64;
    years_sum.map(|v| v / n * release.multiplier())
}

/// Upstream's module-level `point_source_dose`, one distance: the rule-of-thumb
/// dose rate `6 C E / d^2` of a point gamma source (C in curie, E in MeV,
/// d converted from metres with `3.28034 ft/m`), summed over lines and
/// multiplied by a damage ratio.
///
/// `unit` selects upstream's two branches: [`PointSourceUnit::MilliSievertPerHour`]
/// divides by `114 * 3.28034^2`, [`PointSourceUnit::MilliRoentgenPerHour`] by
/// `3.28034^2` only. Both then multiply by `damage_ratio * 1000`, as upstream.
///
/// Upstream's defaults (`gamma_energy`, `g_yield` for Ir-192, activity 1e6 Ci,
/// damage ratio 5e-5) are in [`POINT_SOURCE_DEFAULT_IR192_KEV`] and
/// [`POINT_SOURCE_DEFAULT_IR192_YIELD`]; they are upstream's literals, not
/// data curated here. The *method* `RaddcfFunc.point_source_dose` cannot run
/// upstream (it calls the list `self.rads_list()`, appends to a dict, and
/// unpacks three of four return values: defect D15) and is not ported.
#[must_use]
pub fn point_source_dose(
    gamma_energy_kev: &[f64],
    yields: &[f64],
    activity_curie: f64,
    distance_m: f64,
    damage_ratio: f64,
    unit: PointSourceUnit,
) -> f64 {
    let mut dose = 0.0;
    for (ge, y) in gamma_energy_kev.iter().zip(yields) {
        let num = (y * 6.0 * activity_curie * (ge / 1000.0)) / (distance_m * distance_m);
        dose += match unit {
            PointSourceUnit::MilliSievertPerHour => num / (114.0 * (3.28034f64 * 3.28034)),
            PointSourceUnit::MilliRoentgenPerHour => num / (3.28034f64 * 3.28034),
        };
    }
    dose * damage_ratio * 1000.0
}

/// Output unit branch of [`point_source_dose`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointSourceUnit {
    /// Upstream `'mSv/hr'`.
    MilliSievertPerHour,
    /// Upstream `'mR/hr'`.
    MilliRoentgenPerHour,
}

/// Upstream's default gamma energies for [`point_source_dose`] (keV; upstream
/// says Ir-192). Upstream's literals, reproduced as code defaults.
pub const POINT_SOURCE_DEFAULT_IR192_KEV: [f64; 9] = [
    205.7943, 295.9565, 308.45507, 316.50618, 468.06885, 484.5751, 588.581, 604.41105, 612.46215,
];
/// Upstream's default yields for [`point_source_dose`].
pub const POINT_SOURCE_DEFAULT_IR192_YIELD: [f64; 9] = [
    0.0334, 0.2872, 0.2968, 0.8275, 0.4781, 0.03189, 0.04517, 0.082, 0.0534,
];
/// Upstream's default distances for [`point_source_dose`], m.
pub const POINT_SOURCE_DEFAULT_DISTANCES_M: [f64; 10] = [
    10.0, 20.0, 40.0, 50.0, 100.0, 200.0, 400.0, 600.0, 800.0, 1000.0,
];
