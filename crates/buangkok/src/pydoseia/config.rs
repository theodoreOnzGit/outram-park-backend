// SPDX-License-Identifier: GPL-3.0-only
//! pyDOSEIA's run configuration, as a typed Rust structure with upstream's
//! defaults and upstream's consistency checks.
//!
//! # Provenance
//!
//! Upstream configures a run with a YAML file, written by hand or by its
//! interactive input generator (`auto_input_generator.py`,
//! `auto_input_generator_funcs_class.py`, `auto_input_v18.py`), and checked in
//! `DoseFunc.__init__` / `OutputFunc.__init__` (`dosefunc.py`,
//! `outputfunc.py`). Upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at
//! commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024
//! Dr. Biswajit Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//!
//! The interactive prompts, the YAML reader/writer, logging and the `pickle_it`
//! dump are user interface and are **not** ported; a Rust caller fills
//! [`PyDoseiaConfig`] directly. Every key that changes a number is here, under
//! its upstream name in the field docs. Keys upstream reads but never uses in
//! a calculation are listed at [`PyDoseiaConfig::unused_upstream_keys`].

use crate::pydoseia::dcf::LungAbsorptionType;
use crate::pydoseia::ingestion::food_chain::{AnimalProduct, Climate, VegetationType};
use crate::pydoseia::ingestion::{DietaryIntake, IngestionParameters, SoilType};
use crate::pydoseia::plume_shine::PlumeShineIntegrator;

/// Release scenario (upstream's mutually exclusive `long_term_release` and
/// `single_plume`).
#[derive(Debug, Clone, PartialEq)]
pub enum ReleaseScenario {
    /// Continuous release; `annual_discharge_bq_rad_list`, Bq/y per nuclide.
    LongTerm {
        /// Bq/y per nuclide.
        annual_discharge_bq: Vec<f64>,
    },
    /// Instantaneous release; `instantaneous_release_bq_list`, Bq per nuclide.
    SinglePlume {
        /// Bq per nuclide.
        instantaneous_release_bq: Vec<f64>,
    },
}

/// Where the dilution factor comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum DilutionSource {
    /// Compute it from the plume model (upstream `have_dilution_factor:
    /// False`), with or without met data.
    Computed,
    /// Use the caller's maximum `chi/Q` per distance, s/m^3
    /// (`have_dilution_factor: True`, `list_max_dilution_factor`), as
    /// `(distance m, chi/Q)` pairs.
    UserSupplied(Vec<(f64, f64)>),
}

/// The meteorological settings (upstream keys used when `have_met_data`).
#[derive(Debug, Clone, PartialEq)]
pub struct MetSettings {
    /// `calm_correction`.
    pub calm_correction: bool,
    /// `start_operation_time` (hour).
    pub start_operation_time: i64,
    /// `end_operation_time` (hour).
    pub end_operation_time: i64,
    /// `num_days`, per year of data.
    pub num_days: Vec<i64>,
    /// `sampling_time`, minutes (read; its correction is never applied).
    pub sampling_time: f64,
}

/// A pyDOSEIA run configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct PyDoseiaConfig {
    /// Release scenario.
    pub release: ReleaseScenario,
    /// Dilution factor source.
    pub dilution: DilutionSource,
    /// `have_met_data` and its settings (`None` = no met data).
    pub met: Option<MetSettings>,
    /// `rads_list`.
    pub nuclides: Vec<String>,
    /// `element_list`.
    pub elements: Vec<String>,
    /// `type_rad`, per nuclide.
    pub absorption_types: Vec<LungAbsorptionType>,
    /// `release_height`, m.
    pub release_height_m: f64,
    /// `measurement_height`, m.
    pub measurement_height_m: f64,
    /// `downwind_distances`, m.
    pub downwind_distances_m: Vec<f64>,
    /// `plant_boundary`, m (appended to the distances if absent).
    pub plant_boundary_m: f64,
    /// `age_group`, years.
    pub age_group: Vec<f64>,
    /// `max_conc_plume_central_line_gl`.
    pub centreline_ground_level: bool,
    /// `Y`, m (single plume and plume shine receptor).
    pub receptor_y_m: f64,
    /// `Z`, m.
    pub receptor_z_m: f64,
    /// `like_to_scale_with_mean_speed` with `ask_mean_speed_data` (m/s,
    /// classes A-F).
    pub mean_speed_scaling: Option<[f64; 6]>,
    /// `weathering_corr` (ground shine).
    pub weathering_corr: bool,
    /// `exposure_period`, years.
    pub exposure_period_y: f64,
    /// `consider_progeny`.
    pub consider_progeny: bool,
    /// `ignore_half_life`, s.
    pub ignore_half_life_s: f64,
    /// `run_dose_computation`.
    pub run_dose_computation: bool,
    /// `run_plume_shine_dose`.
    pub run_plume_shine_dose: bool,
    /// Which quadrature the plume-shine integral runs on (not an upstream
    /// setting). Defaults to [`PlumeShineIntegrator::Petir`]; set
    /// [`PlumeShineIntegrator::ScipyQuadpackReference`] to reproduce
    /// pyDOSEIA's numbers bit for bit.
    pub plume_shine_integrator: PlumeShineIntegrator,
    /// `inges_param_dict`.
    pub ingestion_parameters: IngestionParameters,
    /// `inges_param_dict_adult`.
    pub diet_adult: DietaryIntake,
    /// `inges_param_dict_infant`.
    pub diet_infant: DietaryIntake,
    /// `soiltype`.
    pub soil: SoilType,
    /// `climate`.
    pub climate: Climate,
    /// `veg_type_list` (one type; see the ingestion docs).
    pub veg_type: VegetationType,
    /// `animal_feed_type`.
    pub animal_feed_type: VegetationType,
    /// `animal_product_list_for_tritium`.
    pub animal_products_h3: Vec<AnimalProduct>,
    /// `animal_product_list_for_C14`.
    pub animal_products_c14: Vec<AnimalProduct>,
}

/// Why a configuration is rejected (upstream's `ValueError` messages, in
/// substance).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// No nuclides while dose computation is requested.
    NoNuclides,
    /// `element_list` (or `type_rad`, or the release list) does not match
    /// `rads_list` in length.
    LengthMismatch(&'static str),
    /// No distances.
    NoDistances,
    /// No ages.
    NoAges,
    /// Release height missing or zero (upstream tests `not release_height`).
    NoReleaseHeight,
    /// Met data requested without day counts or with a zero measurement height.
    MetSettings,
    /// `H-3` requested without the tritium food-chain settings (upstream
    /// requires the animal product list).
    TritiumSettings,
}

impl PyDoseiaConfig {
    /// A configuration with the input generator's defaults for everything
    /// except the scenario-specific lists, which the caller sets.
    #[must_use]
    pub fn input_generator_defaults(release: ReleaseScenario) -> Self {
        Self {
            release,
            dilution: DilutionSource::Computed,
            met: None,
            nuclides: Vec::new(),
            elements: Vec::new(),
            absorption_types: Vec::new(),
            release_height_m: 10.0,
            measurement_height_m: 10.0,
            downwind_distances_m: Vec::new(),
            plant_boundary_m: 0.0,
            age_group: vec![1.0, 18.0],
            centreline_ground_level: true,
            receptor_y_m: 0.0,
            receptor_z_m: 0.0,
            mean_speed_scaling: None,
            weathering_corr: false,
            exposure_period_y: 30.0,
            consider_progeny: true,
            ignore_half_life_s: 1800.0,
            run_dose_computation: true,
            run_plume_shine_dose: false,
            plume_shine_integrator: PlumeShineIntegrator::Petir,
            ingestion_parameters: IngestionParameters::INPUT_GENERATOR_DEFAULT,
            diet_adult: DietaryIntake::INPUT_GENERATOR_ADULT,
            diet_infant: DietaryIntake::INPUT_GENERATOR_INFANT,
            soil: SoilType::PeatSoil,
            climate: Climate::Continental,
            veg_type: VegetationType::LeafyVegetables,
            animal_feed_type: VegetationType::LeafyVegetables,
            animal_products_h3: vec![AnimalProduct::CowMilk, AnimalProduct::GoatMeat],
            animal_products_c14: vec![AnimalProduct::CowMilk, AnimalProduct::GoatMeat],
        }
    }

    /// The distances upstream computes at: `downwind_distances` with the plant
    /// boundary appended if it is not already there (upstream mutates the
    /// config list in place).
    #[must_use]
    pub fn distances_with_boundary(&self) -> Vec<f64> {
        let mut d = self.downwind_distances_m.clone();
        if !d.contains(&self.plant_boundary_m) {
            d.push(self.plant_boundary_m);
        }
        d
    }

    /// The release list of the active scenario.
    #[must_use]
    pub fn releases(&self) -> &[f64] {
        match &self.release {
            ReleaseScenario::LongTerm {
                annual_discharge_bq,
            } => annual_discharge_bq,
            ReleaseScenario::SinglePlume {
                instantaneous_release_bq,
            } => instantaneous_release_bq,
        }
    }

    /// Upstream's `__init__` checks, plus the list-length consistency upstream
    /// assumes without checking.
    ///
    /// # Errors
    /// The first problem found.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(m) = &self.met {
            if m.num_days.is_empty() || self.measurement_height_m == 0.0 {
                return Err(ConfigError::MetSettings);
            }
        }
        if self.release_height_m == 0.0 {
            return Err(ConfigError::NoReleaseHeight);
        }
        if self.distances_with_boundary().is_empty() {
            return Err(ConfigError::NoDistances);
        }
        if self.run_dose_computation {
            let n = self.nuclides.len();
            if n == 0 {
                return Err(ConfigError::NoNuclides);
            }
            if self.nuclides.iter().any(|x| x == "H-3") && self.animal_products_h3.is_empty() {
                return Err(ConfigError::TritiumSettings);
            }
            if self.elements.len() != n {
                return Err(ConfigError::LengthMismatch("element_list"));
            }
            if self.absorption_types.len() != n {
                return Err(ConfigError::LengthMismatch("type_rad"));
            }
            if self.releases().len() != n {
                return Err(ConfigError::LengthMismatch("release list"));
            }
            if self.age_group.is_empty() {
                return Err(ConfigError::NoAges);
            }
        }
        Ok(())
    }

    /// Keys upstream reads (or its input generator writes) that no
    /// calculation uses at `dca4cdc3`, with the reason.
    #[must_use]
    pub const fn unused_upstream_keys() -> &'static [(&'static str, &'static str)] {
        &[
            (
                "consumption_time_food",
                "ingestion uses consumption_time_frac = 1 instead",
            ),
            (
                "weathering_corr_ingestion",
                "ingestion_weathering_correction is never called",
            ),
            ("n", "only the never-called zyx_lim_for_integral takes n"),
            (
                "sampling_time",
                "the sampling-time correction to sigma_y is commented out",
            ),
            (
                "scaling_dilution_factor_based_on_met_data_speed_distribution",
                "read, never used",
            ),
            ("run_ps_dose_parallel", "joblib parallelism only"),
            ("pickle_it", "output only"),
            ("plot_dilution_factor", "output only"),
            (
                "max_dilution_factor",
                "superseded by list_max_dilution_factor",
            ),
        ]
    }
}
