// SPDX-License-Identifier: GPL-3.0-only
//! The pyDOSEIA driver: every distance and age of a configuration through the
//! dilution factor and all five pathways, and the summary tables upstream
//! writes.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `outputfunc.py` (`OutputFunc.dose_calculation_script`,
//! `dil_fac_all_sectors_all_dist`, `agewise_dose_inh_gs_submersion`,
//! `agewise_ingestion_dose`, `all_dist_agewise_plume_shine_dose`,
//! `agewise_dcfs_inh_gs_submersion`, the totals in `output_to_txt`),
//! `metfunc.py` (`get_max_dilution_factor`) and `main.py`
//! (`reshape_ingestion_dose_data`, `reshape_dose_data`,
//! `process_plume_doses_have_met_data`, `process_plume_doses_have_no_met_data`),
//! upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//!
//! # What is not ported
//!
//! Upstream runs the (distance, age) grid through `joblib.Parallel`; the port
//! runs it in a plain loop, which gives the same numbers (each cell is
//! independent). The text report, CSV writing, pickling, logging and plots are
//! output formatting and are not ported; the **numbers** they print are, as
//! the functions below.

use uom::si::f64::{Length, Radioactivity};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;

use crate::pydoseia::config::{DilutionSource, PyDoseiaConfig, ReleaseScenario};
use crate::pydoseia::dcf::{
    external_dcf, AgeBracket, ExternalDcfPair, ExternalDcfTable, InhalationDcfTable, ProgenyChains,
    ProgenyCorrection,
};
use crate::pydoseia::dcf_screening::{compute_max_dcf, AlternateNames, ScreenedDcf, ScreeningTables};
use crate::pydoseia::dispersion::{self, CalmCorrection, MeanSpeedScaling, PlumeGeometry, Receptor};
use crate::pydoseia::dose::{self, Release, Weathering};
use crate::pydoseia::ingestion::upstream::{
    ingestion_dose_upstream, IngestionReleaseMode, UpstreamIngestionError, UpstreamIngestionInputs,
    UpstreamIngestionOutput,
};
use crate::pydoseia::ingestion::{EcoParamTable, IngestionDcfTable, Receiver};
use crate::pydoseia::met::MetClimatology;
use crate::pydoseia::plume_shine::{
    self, AttenuationTable, GammaLineTable, PlumeShineGeometry, PlumeShineMode, PlumeShineRelease,
};
use crate::pydoseia::units::DilutionFactor;

/// The coefficient tables a run reads (all caller-supplied; see each type).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AssessmentTables {
    /// Inhalation e(g).
    pub inhalation: InhalationDcfTable,
    /// Ground-surface dose-rate coefficients.
    pub surface: ExternalDcfTable,
    /// Air-submersion dose-rate coefficients.
    pub submersion: ExternalDcfTable,
    /// Decay chains for the progeny correction.
    pub chains: ProgenyChains,
    /// Ingestion e(g).
    pub ingestion: IngestionDcfTable,
    /// Element transfer factors.
    pub eco: EcoParamTable,
    /// Gamma lines (plume shine).
    pub gamma: GammaLineTable,
    /// Air attenuation (plume shine).
    pub attenuation: AttenuationTable,
}

/// Inhalation, ground-shine and submersion doses per nuclide, mSv (mSv/y for
/// a long-term release).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathwayDoses {
    /// Inhalation, per nuclide.
    pub inhalation: Vec<f64>,
    /// Ground shine, per nuclide.
    pub ground_shine: Vec<f64>,
    /// Submersion, per nuclide.
    pub submersion: Vec<f64>,
}

/// Why a (distance, age) cell has no ingestion result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionFailure {
    /// The age is neither `> 17` nor `== 1`: upstream's driver has no receiver
    /// for it and raises `UnboundLocalError` (defect D13).
    AgeHasNoReceiver,
    /// `ingestion_dose` itself raised.
    Upstream(UpstreamIngestionError),
}

/// One (distance, age) cell.
#[derive(Debug, Clone, PartialEq)]
pub struct CellResult {
    /// Distance, m.
    pub distance_m: f64,
    /// Age, years.
    pub age: f64,
    /// Inhalation, ground shine, submersion.
    pub pathways: PathwayDoses,
    /// Ingestion, as upstream's `ingestion_dose` returns it.
    pub ingestion: Result<UpstreamIngestionOutput, IngestionFailure>,
}

/// Everything `dose_calculation_script` returns.
#[derive(Debug, Clone, PartialEq)]
pub struct AssessmentResults {
    /// Distances computed, plant boundary appended.
    pub distances_m: Vec<f64>,
    /// Dilution factor per distance: 6 values (per class) without met data,
    /// 16 (per sector) with; empty for [`DilutionSource::UserSupplied`].
    pub dilution: Vec<Vec<DilutionFactor>>,
    /// The `chi/Q` every pathway uses at each distance.
    pub max_chi_over_q: Vec<DilutionFactor>,
    /// `[distance][age]`.
    pub cells: Vec<Vec<CellResult>>,
    /// Plume shine `[distance][nuclide][class or sector]`, when requested.
    pub plume_shine: Option<Vec<Vec<Vec<f64>>>>,
}

/// Why a run cannot proceed.
#[derive(Debug, Clone, PartialEq)]
pub enum AssessmentError {
    /// The configuration failed [`PyDoseiaConfig::validate`].
    Config(crate::pydoseia::config::ConfigError),
    /// A single-plume release with met data: upstream fails its own shape
    /// assertion (defect D3).
    SinglePlumeWithMetCannotRun,
    /// Met data configured but none given.
    MissingMetData,
    /// Decay constants do not match the nuclide list.
    DecayConstants,
    /// A user-supplied dilution factor has no entry for a distance.
    NoDilutionFactorFor(f64),
}

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

/// Dilution factor for one distance under the configured mode
/// (`dil_fac_all_sectors_all_dist`).
///
/// # Errors
/// D3, or missing met data.
pub fn dilution_for_distance(
    cfg: &PyDoseiaConfig,
    x_m: f64,
    met: Option<&MetClimatology>,
) -> Result<Vec<DilutionFactor>, AssessmentError> {
    let receptor = if cfg.centreline_ground_level {
        Receptor::GroundLevelCentreline
    } else {
        Receptor::Offset {
            y: m(cfg.receptor_y_m),
            z: m(cfg.receptor_z_m),
        }
    };
    let geometry = PlumeGeometry {
        release_height: m(cfg.release_height_m),
        measurement_height: m(cfg.measurement_height_m),
        receptor,
    };
    let scaling = match cfg.mean_speed_scaling {
        None => MeanSpeedScaling::UnitSpeed,
        Some(s) => MeanSpeedScaling::PerClass(
            s.map(uom::si::f64::Velocity::new::<uom::si::velocity::meter_per_second>),
        ),
    };
    match (&cfg.release, &cfg.met) {
        (ReleaseScenario::SinglePlume { .. }, None) => {
            Ok(dispersion::dilution_single_plume_no_met(m(x_m), geometry, scaling).to_vec())
        }
        (ReleaseScenario::SinglePlume { .. }, Some(_)) => {
            Err(AssessmentError::SinglePlumeWithMetCannotRun)
        }
        (ReleaseScenario::LongTerm { .. }, None) => {
            Ok(dispersion::dilution_long_term_no_met(m(x_m), geometry, scaling).to_vec())
        }
        (ReleaseScenario::LongTerm { .. }, Some(settings)) => {
            let met = met.ok_or(AssessmentError::MissingMetData)?;
            let calm = if settings.calm_correction {
                CalmCorrection::Upstream
            } else {
                CalmCorrection::Off
            };
            Ok(dispersion::dilution_long_term_with_met(m(x_m), geometry, met, calm).to_vec())
        }
    }
}

/// Inhalation, ground shine and submersion for one (distance, age), with the
/// configured progeny and weathering settings (`agewise_dose_inh_gs_submersion`).
#[must_use]
pub fn pathway_doses(
    cfg: &PyDoseiaConfig,
    tables: &AssessmentTables,
    decay_constants_per_s: &[f64],
    chi: DilutionFactor,
    age: f64,
) -> PathwayDoses {
    let mut out = PathwayDoses::default();
    let Some(bracket) = AgeBracket::from_age_years(age) else {
        return out;
    };
    let progeny = if cfg.consider_progeny {
        ProgenyCorrection::IncludeShortLived {
            ignore_half_life_s: cfg.ignore_half_life_s,
        }
    } else {
        ProgenyCorrection::ParentOnly
    };
    let weathering = if cfg.weathering_corr {
        Weathering::SoilLossRates
    } else {
        Weathering::Off
    };
    for (i, name) in cfg.nuclides.iter().enumerate() {
        let q = Radioactivity::new::<becquerel>(cfg.releases()[i]);
        let release = match cfg.release {
            ReleaseScenario::LongTerm { .. } => Release::AnnualDischarge(q),
            ReleaseScenario::SinglePlume { .. } => Release::Instantaneous(q),
        };
        let element = &cfg.elements[i];
        let dcf_inh = tables
            .inhalation
            .lookup(name, cfg.absorption_types[i], bracket);
        out.inhalation.push(
            dose::inhalation_dose(chi, release, dcf_inh, age)
                .map_or(f64::NAN, |d| d.millisieverts()),
        );
        let dcf_gs =
            external_dcf(&tables.surface, &tables.chains, name, bracket, progeny).selected(progeny);
        let t = dose::effective_buildup_time_s(
            decay_constants_per_s[i],
            element,
            weathering,
            cfg.exposure_period_y,
        );
        out.ground_shine.push(
            dose::ground_shine_dose(
                chi,
                release,
                dose::deposition_velocity_m_per_s(element),
                t,
                dcf_gs,
            )
            .millisieverts(),
        );
        let dcf_sub = external_dcf(&tables.submersion, &tables.chains, name, bracket, progeny)
            .selected(progeny);
        out.submersion
            .push(dose::submersion_dose(chi, release, dcf_sub).millisieverts());
    }
    out
}

fn ingestion_inputs(
    cfg: &PyDoseiaConfig,
    decay_constants_per_s: &[f64],
    chi: DilutionFactor,
) -> UpstreamIngestionInputs {
    UpstreamIngestionInputs {
        nuclides: cfg.nuclides.clone(),
        elements: cfg.elements.clone(),
        decay_constants_per_s: decay_constants_per_s.to_vec(),
        releases_bq: cfg.releases().to_vec(),
        mode: match cfg.release {
            ReleaseScenario::LongTerm { .. } => IngestionReleaseMode::LongTerm,
            ReleaseScenario::SinglePlume { .. } => IngestionReleaseMode::SinglePlume,
        },
        chi_over_q: chi.seconds_per_cubic_meter(),
        parameters: cfg.ingestion_parameters,
        diet_adult: cfg.diet_adult,
        diet_infant: cfg.diet_infant,
        soil: cfg.soil,
        climate: cfg.climate,
        veg_type: cfg.veg_type,
        animal_feed_type: cfg.animal_feed_type,
        animal_products_h3: cfg.animal_products_h3.clone(),
        animal_products_c14: cfg.animal_products_c14.clone(),
    }
}

/// The whole run (`dose_calculation_script`).
///
/// With [`DilutionSource::UserSupplied`] every pathway uses the caller's
/// maximum `chi/Q` for the distance, as upstream does (its `MetFunc` keeps
/// `list_max_dilution_factor` as `dict_max_dilution_factor`; checked in the
/// fixture's `user_dilution_factor` scenario). No dilution factor per class
/// or sector is computed then.
///
/// # Errors
/// See [`AssessmentError`].
pub fn run_assessment(
    cfg: &PyDoseiaConfig,
    tables: &AssessmentTables,
    decay_constants_per_s: &[f64],
    met: Option<&MetClimatology>,
) -> Result<AssessmentResults, AssessmentError> {
    cfg.validate().map_err(AssessmentError::Config)?;
    if cfg.run_dose_computation && decay_constants_per_s.len() != cfg.nuclides.len() {
        return Err(AssessmentError::DecayConstants);
    }
    let distances = cfg.distances_with_boundary();
    let mut dilution = Vec::new();
    let mut max_chi = Vec::new();
    for x in &distances {
        match &cfg.dilution {
            DilutionSource::Computed => {
                let d = dilution_for_distance(cfg, *x, met)?;
                max_chi.push(dispersion::max_dilution_factor(&d));
                dilution.push(d);
            }
            DilutionSource::UserSupplied(pairs) => {
                let v = pairs
                    .iter()
                    .find(|(d, _)| d == x)
                    .ok_or(AssessmentError::NoDilutionFactorFor(*x))?
                    .1;
                max_chi.push(DilutionFactor::new(v));
            }
        }
    }
    let mut cells = Vec::new();
    let mut plume = None;
    if cfg.run_dose_computation {
        for (xi, x) in distances.iter().enumerate() {
            let chi = max_chi[xi];
            let mut row = Vec::new();
            for age in &cfg.age_group {
                let pathways = pathway_doses(cfg, tables, decay_constants_per_s, chi, *age);
                let ingestion = match Receiver::from_driver_age(*age) {
                    None => Err(IngestionFailure::AgeHasNoReceiver),
                    Some(r) => ingestion_dose_upstream(
                        &ingestion_inputs(cfg, decay_constants_per_s, chi),
                        &tables.eco,
                        &tables.ingestion,
                        r,
                    )
                    .map_err(IngestionFailure::Upstream),
                };
                row.push(CellResult {
                    distance_m: *x,
                    age: *age,
                    pathways,
                    ingestion,
                });
            }
            cells.push(row);
        }
        if cfg.run_plume_shine_dose {
            let mut per_distance = Vec::new();
            for x in &distances {
                let mut per_nuclide = Vec::new();
                for (i, name) in cfg.nuclides.iter().enumerate() {
                    let lines = tables.gamma.plume_shine_lines(name);
                    let (mode, release) = match cfg.release {
                        ReleaseScenario::SinglePlume { .. } => (
                            PlumeShineMode::SinglePlume {
                                y: cfg.receptor_y_m,
                                z: cfg.receptor_z_m,
                            },
                            PlumeShineRelease::InstantaneousBq(cfg.releases()[i]),
                        ),
                        ReleaseScenario::LongTerm { .. } => (
                            PlumeShineMode::SectorAveraged {
                                z: cfg.receptor_z_m,
                            },
                            PlumeShineRelease::AnnualDischargeBq(cfg.releases()[i]),
                        ),
                    };
                    let g = PlumeShineGeometry {
                        distance: m(*x),
                        release_height: m(cfg.release_height_m),
                        mode,
                    };
                    let v = match (&cfg.met, met) {
                        (Some(_), Some(mc)) => plume_shine::per_sector_with_met(
                            &lines,
                            &tables.attenuation,
                            g,
                            mc,
                            m(cfg.measurement_height_m),
                            release,
                        )
                        .to_vec(),
                        _ => {
                            plume_shine::per_class(&lines, &tables.attenuation, g, release).to_vec()
                        }
                    };
                    per_nuclide.push(v);
                }
                per_distance.push(per_nuclide);
            }
            plume = Some(per_distance);
        }
    }
    Ok(AssessmentResults {
        distances_m: distances,
        dilution,
        max_chi_over_q: max_chi,
        cells,
        plume_shine: plume,
    })
}

/// Upstream's driver reshapes each cell's ingestion array to
/// `(nuclides, 3)` in C order (`INGESTION_DOSES.reshape(..., n, 3)`). For the
/// transposed layout (H-3, C-14 and others together) that **scrambles** the
/// routes across nuclides (part of D9); this reproduces it.
#[must_use]
pub fn driver_ingestion_matrix(out: &UpstreamIngestionOutput) -> Vec<[f64; 3]> {
    let flat: Vec<f64> = out.rows.iter().flatten().copied().collect();
    flat.chunks(3)
        .map(|c| {
            [
                c[0],
                c.get(1).copied().unwrap_or(f64::NAN),
                c.get(2).copied().unwrap_or(f64::NAN),
            ]
        })
        .collect()
}

/// pandas' `sum(skipna=True)` over a short row or column: NaN counts as 0 and
/// the sum is numpy's (pairwise for 8 or more values).
#[must_use]
pub fn pandas_sum(values: &[f64]) -> f64 {
    let v: Vec<f64> = values
        .iter()
        .map(|x| if x.is_nan() { 0.0 } else { *x })
        .collect();
    numpy_pairwise_sum(&v)
}

/// numpy's pairwise summation (`pairwise_sum_DOUBLE`, block size 128, eight
/// accumulators).
#[must_use]
pub fn numpy_pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [0.0; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        numpy_pairwise_sum(&a[..n2]) + numpy_pairwise_sum(&a[n2..])
    }
}

/// pandas' groupby `sum` (Kahan-compensated, NaN skipped).
#[must_use]
pub fn pandas_groupby_sum(values: &[f64]) -> f64 {
    let mut sumx = 0.0;
    let mut comp = 0.0;
    for &val in values {
        if val.is_nan() {
            continue;
        }
        let y = val - comp;
        let t = sumx + y;
        comp = t - sumx - y;
        sumx = t;
    }
    sumx
}

/// Whether the report zeroes milk and meat for elements without transfer
/// factors (`zeroing_ingestion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zeroing {
    /// pandas >= 3 (copy-on-write): upstream's chained assignment changes a
    /// copy and the table is unchanged (defect D14). What the fixture records.
    ChainedAssignmentNoOp,
    /// What the function means to do (and what pandas < 3 did).
    ZeroMilkAndMeat,
}

/// One nuclide's line of upstream's "total dose at plant boundary" table
/// (`output_to_txt`), mSv (mSv/y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryTotal {
    /// Inhalation.
    pub inhalation: f64,
    /// Ground shine.
    pub ground_shine: f64,
    /// Submersion.
    pub submersion: f64,
    /// Ingestion (sum of veg, milk and meat, NaN as 0).
    pub ingestion: f64,
    /// `Total` (NaN as 0). Plume shine is **not** included, as upstream.
    pub total: f64,
}

/// The per-nuclide totals `output_to_txt` prints for each age at the plant
/// boundary. `None` where the cell has no ingestion result.
#[must_use]
pub fn plant_boundary_totals(
    cfg: &PyDoseiaConfig,
    results: &AssessmentResults,
    zeroing: Zeroing,
    no_transfer_factor_elements: &[String],
) -> Vec<Option<Vec<BoundaryTotal>>> {
    let Some(xi) = results
        .distances_m
        .iter()
        .position(|d| *d == cfg.plant_boundary_m)
    else {
        return Vec::new();
    };
    results.cells[xi]
        .iter()
        .map(|cell| {
            let ing = cell.ingestion.as_ref().ok()?;
            let matrix = driver_ingestion_matrix(ing);
            Some(
                (0..cfg.nuclides.len())
                    .map(|i| {
                        let mut r = matrix.get(i).copied().unwrap_or([f64::NAN; 3]);
                        if zeroing == Zeroing::ZeroMilkAndMeat
                            && no_transfer_factor_elements.contains(&cfg.elements[i])
                        {
                            r = [r[0], 0.0, 0.0];
                        }
                        let ingestion = pandas_sum(&r);
                        let inh = cell.pathways.inhalation[i];
                        let gs = cell.pathways.ground_shine[i];
                        let sub = cell.pathways.submersion[i];
                        BoundaryTotal {
                            inhalation: inh,
                            ground_shine: gs,
                            submersion: sub,
                            ingestion,
                            total: pandas_sum(&[inh, gs, sub, ingestion]),
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

/// One (distance, age) row of upstream's `summary_summed_inh_gs_sub_dose.csv`
/// (`main.reshape_dose_data`), mSv (mSv/y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SummaryRow {
    /// Distance, m.
    pub distance_m: f64,
    /// Age, years.
    pub age: f64,
    /// Sum over nuclides.
    pub inhalation: f64,
    /// Sum over nuclides.
    pub ground_shine: f64,
    /// Sum over nuclides.
    pub submersion: f64,
    /// Ingestion as the summary reports it.
    pub ingestion: f64,
    /// Sum of the four.
    pub total: f64,
}

/// How the summary counts ingestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryIngestion {
    /// Upstream: the group sum runs over the per-nuclide rows **and** the
    /// `SUM` row `reshape_ingestion_dose_data` appended, so ingestion is
    /// counted **twice** (defect D20).
    UpstreamDoubleCounted,
    /// **Divergence:** the per-nuclide rows only.
    PerNuclideRowsOnly,
}

/// The rows of upstream's summed summary, one per (distance, age), in
/// upstream's final order (a stable sort by age). Cells without ingestion are
/// skipped (upstream crashes before writing the file for them).
#[must_use]
pub fn summary_rows(results: &AssessmentResults, mode: SummaryIngestion) -> Vec<SummaryRow> {
    let mut rows = Vec::new();
    for row in &results.cells {
        for cell in row {
            let Ok(ing) = cell.ingestion.as_ref() else {
                continue;
            };
            let matrix = driver_ingestion_matrix(ing);
            let per_nuclide_totals: Vec<f64> = matrix.iter().map(|r| r[0] + r[1] + r[2]).collect();
            let sum_row = pandas_sum(&per_nuclide_totals);
            let ingestion = match mode {
                SummaryIngestion::UpstreamDoubleCounted => {
                    let mut all = per_nuclide_totals.clone();
                    all.push(sum_row);
                    pandas_groupby_sum(&all)
                }
                SummaryIngestion::PerNuclideRowsOnly => pandas_groupby_sum(&per_nuclide_totals),
            };
            let inhalation = pandas_sum(&cell.pathways.inhalation);
            let ground_shine = pandas_sum(&cell.pathways.ground_shine);
            let submersion = pandas_sum(&cell.pathways.submersion);
            rows.push(SummaryRow {
                distance_m: cell.distance_m,
                age: cell.age,
                inhalation,
                ground_shine,
                submersion,
                ingestion,
                total: pandas_sum(&[inhalation, ground_shine, submersion, ingestion]),
            });
        }
    }
    rows.sort_by(|a, b| a.age.total_cmp(&b.age));
    rows
}

/// `process_plume_doses_*`: per (distance, nuclide) the maximum over classes
/// or sectors and, for sectors, its index (pandas `idxmax`, first maximum,
/// NaN skipped).
#[must_use]
pub fn plume_shine_maxima(values: &[f64]) -> (f64, Option<usize>) {
    let mut best: Option<(usize, f64)> = None;
    for (i, v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if best.is_none_or(|(_, b)| *v > b) {
            best = Some((i, *v));
        }
    }
    best.map_or((f64::NAN, None), |(i, v)| (v, Some(i)))
}

/// The coefficients upstream's report prints for one age
/// (`agewise_dcfs_inh_gs_submersion`): the screened inhalation and ingestion
/// maxima of [`compute_max_dcf`] (not the coefficients the doses use: D7),
/// and the ground-surface and submersion coefficient pairs (with and without
/// progeny, as upstream returns them). The ground-surface one
/// is looked up with progeny **always included**, whatever the configuration
/// says (upstream passes `consider_progeny=True` explicitly: defect D22).
#[must_use]
pub fn dcf_report(
    cfg: &PyDoseiaConfig,
    tables: &AssessmentTables,
    screening: &ScreeningTables,
    alternate_names: &[AlternateNames],
    age: f64,
) -> Vec<(Option<ScreenedDcf>, ExternalDcfPair, ExternalDcfPair)> {
    let Some(bracket) = AgeBracket::from_age_years(age) else {
        return Vec::new();
    };
    let forced = ProgenyCorrection::IncludeShortLived {
        ignore_half_life_s: cfg.ignore_half_life_s,
    };
    let configured = if cfg.consider_progeny {
        forced
    } else {
        ProgenyCorrection::ParentOnly
    };
    cfg.nuclides
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let ty = match cfg.absorption_types[i] {
                crate::pydoseia::dcf::LungAbsorptionType::F => "F",
                crate::pydoseia::dcf::LungAbsorptionType::M => "M",
                crate::pydoseia::dcf::LungAbsorptionType::S => "S",
                crate::pydoseia::dcf::LungAbsorptionType::V => "V",
                crate::pydoseia::dcf::LungAbsorptionType::Max => "Max",
            };
            let alt = alternate_names.get(i).cloned().unwrap_or_default();
            let screened = compute_max_dcf(screening, n, ty, &alt, age);
            let gs = external_dcf(&tables.surface, &tables.chains, n, bracket, forced);
            let sub = external_dcf(&tables.submersion, &tables.chains, n, bracket, configured);
            (screened, gs, sub)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from numpy 2.5.3 / pandas 3.0.6 (the fixture's
    /// versions), computed 2026-09-28: `np.array(a).sum()` is 13.05 where a
    /// left-to-right sum gives 13.351, and the groupby sum is 2.3 where a
    /// plain sum gives 0.30000000000000004.
    #[test]
    fn sums_follow_numpy_and_pandas() {
        let a = [
            1e16, 1.0, -1e16, 3.3, 0.1, 0.2, 0.7, 1e-3, 5.5, 2.25, 1e15, -1e15, 0.3,
        ];
        assert_eq!(numpy_pairwise_sum(&a), 13.05);
        assert_eq!(pandas_groupby_sum(&[1e16, 1.0, 1.0, -1e16, 0.1, 0.2]), 2.3);
        assert_eq!(pandas_sum(&[f64::NAN, 1.0, 2.0]), 3.0);
    }

    #[test]
    fn plume_maxima_take_the_first_largest_and_skip_nan() {
        assert_eq!(
            plume_shine_maxima(&[1.0, f64::NAN, 3.0, 3.0]),
            (3.0, Some(2))
        );
        let (v, i) = plume_shine_maxima(&[f64::NAN]);
        assert!(v.is_nan() && i.is_none());
    }
}
