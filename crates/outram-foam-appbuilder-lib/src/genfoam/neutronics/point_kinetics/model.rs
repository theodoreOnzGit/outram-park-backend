// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/pointKinetics/pointKineticNeutronics.{C,H},
//                    include/{solvePointKinetics,correctReactivity,
//                    computeFeedbackFieldValues}.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Stefan Radman, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `pointKineticNeutronics`, the solid-fuel class (gh:#1864, round 36)
//!
//! A line-by-line translation of GeN-Foam's `solvers::pointKineticNeutronics` for
//! `liquidFuel false`: the constructor's derived quantities, the flux-squared weighted feedback
//! averages (`computeFeedbackFieldValues.H`), the reactivity assembly (`correctReactivity.H`:
//! Doppler, fuel/clad/coolant temperature, coolant density, structure, mechanical structure,
//! control-rod driveline, external, boron), the optional external neutron source with its two
//! modes (`transient`, `powerMonitoring`), the optional decay power, the rescaling of the flux
//! and power-density fields, and `maxDeltaT()` (`solvePointKinetics.H`,
//! `pointKineticNeutronics.C`). The dense solve is [`PointKineticsState::step_with`] with
//! upstream's `solve()` (see [`LinearSolveForm`]).
//!
//! All quantities are plain `f64` in SI base units (K, kg/m^3, W, s, m, `Delta k / k` for
//! reactivities); the arithmetic keeps upstream's operand order so that the result is bit-identical
//! to the real class (README, "Round 36").
//!
//! ## Refused by name
//!
//! ~~`GEMReactivityMap` ([`PkModelError::GemNotPorted`]; upstream says itself that it "does NOT work
//! in GeN-Foam 2.0")~~ **CORRECTED 2026-11-27 (gh:#1927, README round 63): ported** as
//! [`PointKineticNeutronics::with_gem`] / [`PointKineticNeutronics::compute_gem`] (against the real binary in the coupled loop, <= 7.3e-12);
//! [`PkConfig::gem_map`] stays refused by [`PointKineticNeutronics::new`] because the map needs the reference level and the fluid flux, which only
//! `with_gem` takes. The FMU inputs (`type fmi`), ~~the liquid-fuel precursor transport
//! (`solvePointKineticsLiquidFuel.H`)~~ **CORRECTED 2026-11-27 (gh:#1882, round 48): ported in
//! [`liquid_fuel`] (`LiquidFuelPk`), bit-identical to the real class with prescribed fluid fields**,
//! and the `reactorState` write-out.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::{LinearSolveForm, PointKineticsError, PointKineticsParameters, PointKineticsState};
use crate::genfoam::common::TimeProfile;
use petir::mathf::RealMath;
use uom::si::f64::{Frequency, Power, Ratio, Time};
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::time::second;

pub mod liquid_fuel;

/// OpenFOAM's `doubleScalarSMALL`.
const SMALL: f64 = 1.0e-15;
/// OpenFOAM's `doubleScalarGREAT`.
const GREAT: f64 = 1.0e15;

/// Errors of the point-kinetics class.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PkModelError {
    /// The 0-D core refused (singular system, bad parameters).
    #[error("point kinetics: {0}")]
    Core(#[from] PointKineticsError),
    /// `pTarget <= 0` (upstream: "Set pTarget > 0 !").
    #[error("pTarget must be > 0")]
    NonPositivePower,
    /// `promptGenerationTime <= 0` (upstream: "Set promptGenerationTime > 0 !").
    #[error("promptGenerationTime must be > 0")]
    NonPositivePromptGenerationTime,
    /// `nuFission`, `beamEnergy`, `energyPerFission` or `nuSource` negative.
    #[error("nuFission, beamEnergy, energyPerFission and nuSource must be positive")]
    NegativeSourceData,
    /// `GEMReactivityMap` was put in the config of [`PointKineticNeutronics::new`] (use [`PointKineticNeutronics::with_gem`], gh:#1927).
    #[error("GEMReactivityMap in PkConfig: give it through PointKineticNeutronics::with_gem (needs the reference level and the fluid flux, gh:#1927)")]
    GemNotPorted,
    /// `GEMReactivityMap` not in descending sodium level (upstream `FatalError`).
    #[error("GEMReactivityMap should be indexed by descending sodium level values")]
    GemNotDescending,
    /// A per-cell array does not have the mesh's length.
    #[error("field `{0}` has the wrong length")]
    FieldLength(&'static str),
}

/// `externalSourceMode` of `constant/<region>/externalSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSourceMode {
    /// "user modulation of the source" by `externalSourceModulationTimeProfile`.
    Transient,
    /// "source power modulation to keep subcritical power constant".
    PowerMonitoring,
}

/// The `externalSource` dictionary and the `nuclearData` keys the source needs.
#[derive(Debug, Clone)]
pub struct ExternalSourceConfig {
    /// `externalSourceMode`.
    pub mode: ExternalSourceMode,
    /// `externalSourceModulationTimeProfile` (only used by `transient`).
    pub modulation_profile: Option<TimeProfile>,
    /// `nuSource` (default 0).
    pub nu_source: f64,
    /// `beamEnergy` (default 0).
    pub beam_energy: f64,
    /// `nuclearData` `nuFission` (default 0).
    pub nu_fission: f64,
    /// `nuclearData` `energyPerFission` (default 0).
    pub energy_per_fission: f64,
    /// `modulationFactor` (default 0).
    pub modulation_factor: f64,
    /// `powerModulationDelay` (default 0).
    pub power_modulation_delay: f64,
}

/// The seven `feedbackCoeff*` keys of `nuclearData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackCoefficients {
    /// `feedbackCoeffDoppler`.
    pub doppler: f64,
    /// `feedbackCoeffTFuel` (1/K).
    pub t_fuel: f64,
    /// `feedbackCoeffTClad`.
    pub t_clad: f64,
    /// `feedbackCoeffTCool`.
    pub t_cool: f64,
    /// `feedbackCoeffRhoCool` (m^3/kg).
    pub rho_cool: f64,
    /// `feedbackCoeffTStruct`.
    pub t_struct: f64,
    /// `feedbackCoeffTStructMech`.
    pub t_struct_mech: f64,
    /// `absoluteDrivelineExpansionCoeff` (m/K).
    pub driveline_expansion: f64,
}

/// Everything `pointKineticNeutronics` reads from `nuclearData`, `reactorState`, `externalSource`
/// and `neutronicsProperties` for the solid-fuel path.
#[derive(Debug, Clone)]
pub struct PkConfig {
    /// `promptGenerationTime` (s).
    pub prompt_generation_time: f64,
    /// `Beta` (one value per delayed group).
    pub betas: Vec<f64>,
    /// `lambda` (1/s).
    pub lambdas: Vec<f64>,
    /// `fastNeutrons` (log Doppler) or thermal (square-root Doppler).
    pub fast_neutrons: bool,
    /// The feedback coefficients.
    pub coefficients: FeedbackCoefficients,
    /// `controlRodReactivityMap` as (driveline expansion, reactivity) knots; empty when absent.
    pub control_rod_map: Vec<(f64, f64)>,
    /// `GEMReactivityMap` knots; [`PointKineticNeutronics::new`] refuses any entry ([`PkModelError::GemNotPorted`]): GEM is given through
    /// [`PointKineticNeutronics::with_gem`] (gh:#1927).
    pub gem_map: Vec<(f64, f64)>,
    /// `externalReactivityTimeProfile`.
    pub external_reactivity: Option<TimeProfile>,
    /// `boronReactivityTimeProfile`.
    pub boron_reactivity: Option<TimeProfile>,
    /// `decayPowerTimeProfile` of `reactorState`.
    pub decay_power: Option<TimeProfile>,
    /// `reactorState` `pTarget` (W).
    pub p_target: f64,
    /// `reactorState` `subcriticalIndex`, when present.
    pub subcritical_index: Option<f64>,
    /// `reactorState` `ksrc`, when present.
    pub ksrc: Option<f64>,
    /// `reactorState` `precursorPowers` (a restart), when present.
    pub precursor_powers: Option<Vec<f64>>,
    /// `neutronicsProperties` `externalSourceNeutronics true` plus the source dictionaries.
    pub external_source: Option<ExternalSourceConfig>,
    /// `nuclearData` `energyGroups` (default 1).
    pub energy_groups: usize,
    /// Time of the constructor (`Time::timeOutputValue()` at start, normally 0) in s.
    pub start_time: f64,
}

/// The mesh data the class needs: cell volumes and the five 0/1 feedback cell fields.
#[derive(Debug, Clone)]
pub struct PkMeshData {
    /// `mesh.V()`.
    pub volume: Vec<f64>,
    /// `fuelFeedbackCellField_`.
    pub fuel: Vec<f64>,
    /// `coolFeedbackCellField_`.
    pub cool: Vec<f64>,
    /// `structFeedbackCellField_`.
    pub structure: Vec<f64>,
    /// `structMechFeedbackCellField_`.
    pub structure_mech: Vec<f64>,
    /// `drivelineFeedbackCellField_`.
    pub driveline: Vec<f64>,
}

/// `setFeedbackCellField`: no zones listed gives 1 in every cell, otherwise 1 in the listed zones'
/// cells and 0 elsewhere.
pub fn feedback_cell_field(n_cells: usize, zone_cells: &[Vec<usize>]) -> Vec<f64> {
    if zone_cells.is_empty() {
        return vec![1.0; n_cells];
    }
    let mut f = vec![0.0; n_cells];
    for zone in zone_cells {
        for &c in zone {
            f[c] = 1.0;
        }
    }
    f
}

/// The per-cell feedback fields `TFuel_`, `TClad_`, `TCool_`, `rhoCool_`, `TStruct_`,
/// `TStructMech_` (the thermal-hydraulic region's fields mapped onto the neutronics mesh).
#[derive(Debug, Clone)]
pub struct PkFeedbackFields {
    /// Fuel temperature (K).
    pub t_fuel: Vec<f64>,
    /// Cladding temperature (K).
    pub t_clad: Vec<f64>,
    /// Coolant temperature (K).
    pub t_cool: Vec<f64>,
    /// Coolant density (kg/m^3).
    pub rho_cool: Vec<f64>,
    /// Structure temperature (K).
    pub t_struct: Vec<f64>,
    /// Mechanical structure temperature (K).
    pub t_struct_mech: Vec<f64>,
}

/// The seven weighted averages of `computeFeedbackFieldValues.H`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackValues {
    /// `TFuelValue`.
    pub t_fuel: f64,
    /// `TCladValue`.
    pub t_clad: f64,
    /// `TCoolValue`.
    pub t_cool: f64,
    /// `rhoCoolValue`.
    pub rho_cool: f64,
    /// `TStructValue`.
    pub t_struct: f64,
    /// `TStructMechValue`.
    pub t_struct_mech: f64,
    /// `TDrivelineValue` (weights `TStruct_`, as upstream does).
    pub t_driveline: f64,
}

/// The reference values `T*Ref_`, set at the first step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackReferences {
    /// `TFuelRef_`.
    pub t_fuel: f64,
    /// `TCladRef_`.
    pub t_clad: f64,
    /// `TCoolRef_`.
    pub t_cool: f64,
    /// `rhoCoolRef_`.
    pub rho_cool: f64,
    /// `TStructRef_`.
    pub t_struct: f64,
    /// `TStructMechRef_`.
    pub t_struct_mech: f64,
    /// `TDrivelineRef_`.
    pub t_driveline: f64,
}

/// Every term of `correctReactivity.H` (`Delta k / k`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactivityTerms {
    /// `DopplerReactivity`.
    pub doppler: f64,
    /// `TFuelReactivity`.
    pub t_fuel: f64,
    /// `TCladReactivity`.
    pub t_clad: f64,
    /// `TCoolReactivity`.
    pub t_cool: f64,
    /// `rhoCoolReactivity`.
    pub rho_cool: f64,
    /// `TStructReactivity`.
    pub t_struct: f64,
    /// `TStructMechReactivity`.
    pub t_struct_mech: f64,
    /// `drivelineExpValue` (m).
    pub driveline_expansion: f64,
    /// `drivelineReactivity`.
    pub driveline: f64,
    /// `GEMReactivity` (always 0: refused).
    pub gem: f64,
    /// `externalReactivity`.
    pub external: f64,
    /// `boronReactivity`.
    pub boron: f64,
    /// `totalReactivity_`.
    pub total: f64,
}

/// `reactorState` `TFuelRef`, `TladRef` (sic), `TCoolRef`, `rhoCoolRef`, `TStructRef`, `TStructMechRef`, `TDriveLineRef` (sic): upstream's
/// `lookupOrDefault(key, firstStepAverage)` at the first step (`solvePointKinetics.H:12-33`); `None` = key absent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReadReferences {
    /// `TFuelRef`.
    pub t_fuel: Option<f64>,
    /// `TladRef` (upstream's spelling of the read key).
    pub t_clad: Option<f64>,
    /// `TCoolRef`.
    pub t_cool: Option<f64>,
    /// `rhoCoolRef`.
    pub rho_cool: Option<f64>,
    /// `TStructRef`.
    pub t_struct: Option<f64>,
    /// `TStructMechRef`.
    pub t_struct_mech: Option<f64>,
    /// `TDriveLineRef` (upstream's spelling of the read key).
    pub t_driveline: Option<f64>,
}

/// `GEMReactivityMap_` (descending sodium level) and `GEMSodiumLevelRef_` of `pointKineticNeutronics.C:861-882`, plus `intPhiRef_`.
#[derive(Debug, Clone, PartialEq)]
pub struct GemModel {
    map: Vec<(f64, f64)>,
    level_ref: f64,
    int_phi_ref: Option<f64>,
}

/// What one [`PointKineticNeutronics::step`] reports.
#[derive(Debug, Clone, PartialEq)]
pub struct PkStepReport {
    /// The weighted averages used.
    pub values: FeedbackValues,
    /// The reactivity terms used.
    pub terms: ReactivityTerms,
    /// `powerDensity_` rescale factor `power_/powerOld_`.
    pub power_density_scale: f64,
    /// Flux rescale factor `fissionPower_/fissionPowerOld_`.
    pub flux_scale: f64,
}

/// `pointKineticNeutronics` (solid fuel). See the module documentation.
#[derive(Debug, Clone)]
pub struct PointKineticNeutronics {
    cfg: PkConfig,
    params: PointKineticsParameters,
    state: PointKineticsState,
    mesh: PkMeshData,
    form: LinearSolveForm,
    power: f64,
    power_old: f64,
    p_tot_old: f64,
    fission_power: f64,
    decay_power: f64,
    beta: f64,
    subcritical_index: f64,
    ksrc: f64,
    intrinsic_gain: f64,
    external_source_modulation: f64,
    external_source_power: f64,
    external_source_power_ref: f64,
    external_source_beam_power: f64,
    external_source_beam_intensity: f64,
    power_record: Vec<(f64, f64)>,
    total_reactivity: f64,
    refs: Option<FeedbackReferences>,
    steps: usize,
    /// `GEMReactivityMap_` and `GEMSodiumLevelRef_` (gh:#1927), when the map has more than one knot.
    gem: Option<GemModel>,
    /// `GEMReactivity` of the last [`Self::compute_gem`] (0 without GEM).
    gem_reactivity: f64,
    /// `GEMSodiumLevel` of the last [`Self::compute_gem`] (0 without GEM).
    gem_level: f64,
    /// `reactorState` `T*Ref` keys, when read (gh:#1927).
    read_refs: ReadReferences,
    /// `oneGroupFlux_` (1/m^2/s) per cell.
    pub one_group_flux: Vec<f64>,
    /// `fluxes_[g]` per cell.
    pub fluxes: Vec<Vec<f64>>,
    /// `powerDensity_` (W/m^3) per cell.
    pub power_density: Vec<f64>,
    /// `secondaryPowerDensity_` (W/m^3) per cell.
    pub secondary_power_density: Vec<f64>,
    /// `prec_[i]` per cell: the precursor fields READ from the start directory, which `solvePointKinetics.H` only rescales by
    /// `precursorPowers_[i]/precursorPowersOld_[i]`; empty (nothing to rescale) unless [`Self::from_read_fields`] set them (gh:#1922).
    pub prec: Vec<Vec<f64>>,
}

impl PointKineticNeutronics {
    /// The constructor (`pointKineticNeutronics.C`, lines 62 to 905, solid-fuel branch).
    ///
    /// `initial_flux` is `oneGroupFlux_` after `initialOneGroupFluxByZone` (zero outside the
    /// listed zones); `power_density` and `secondary_power_density` are the fields read from the
    /// start-time directory (zero when absent).
    ///
    /// # Errors
    ///
    /// The `FatalError`s of the upstream constructor, and [`PkModelError::GemNotPorted`].
    pub fn new(
        cfg: PkConfig,
        mesh: PkMeshData,
        initial_flux: Vec<f64>,
        power_density: Vec<f64>,
        secondary_power_density: Vec<f64>,
        form: LinearSolveForm,
    ) -> Result<Self, PkModelError> {
        if cfg.gem_map.len() > 1 {
            return Err(PkModelError::GemNotPorted);
        }
        let n = mesh.volume.len();
        for (name, len) in [
            ("fuel", mesh.fuel.len()),
            ("cool", mesh.cool.len()),
            ("structure", mesh.structure.len()),
            ("structure_mech", mesh.structure_mech.len()),
            ("driveline", mesh.driveline.len()),
            ("initial_flux", initial_flux.len()),
            ("power_density", power_density.len()),
            ("secondary_power_density", secondary_power_density.len()),
        ] {
            if len != n {
                return Err(PkModelError::FieldLength(name));
            }
        }
        let power = cfg.p_target;
        let mut fission_power = power;
        let mut decay_power = 0.0;
        if let Some(profile) = cfg.decay_power.as_ref() {
            decay_power = profile.value(Time::new::<second>(cfg.start_time));
            fission_power = power - decay_power;
        }
        if power <= 0.0 {
            return Err(PkModelError::NonPositivePower);
        }
        if cfg.prompt_generation_time <= 0.0 {
            return Err(PkModelError::NonPositivePromptGenerationTime);
        }
        let n_groups = cfg.betas.len();
        // `if (!reactorState_.found("precursorPowers"))` the system starts from a steady state.
        let precursor_powers: Vec<f64> = match cfg.precursor_powers.as_ref() {
            Some(p) => p.clone(),
            None => (0..n_groups)
                .map(|i| {
                    (cfg.betas[i] * fission_power)
                        / (cfg.lambdas[i] * cfg.prompt_generation_time)
                })
                .collect(),
        };
        let subcritical_index_read = cfg.subcritical_index.unwrap_or(0.0);
        let ksrc_read = cfg.ksrc.unwrap_or(1.0);
        let mut subcritical_index = subcritical_index_read;
        let mut ksrc = ksrc_read;
        let mut intrinsic_gain = 0.0;
        let mut external_source_power = 0.0;
        let mut external_source_power_ref = 0.0;
        let mut external_source_beam_power = 0.0;
        let mut external_source_beam_intensity = 0.0;
        if let Some(es) = cfg.external_source.as_ref() {
            if es.nu_fission > 0.0 && es.beam_energy > 0.0 {
                intrinsic_gain =
                    es.energy_per_fission * es.nu_source / (es.nu_fission * es.beam_energy);
            } else if es.nu_fission < 0.0
                || es.beam_energy < 0.0
                || es.energy_per_fission < 0.0
                || es.nu_source < 0.0
            {
                return Err(PkModelError::NegativeSourceData);
            }
            match (cfg.ksrc.is_some(), cfg.subcritical_index.is_some()) {
                (false, true) => ksrc = 1.0 / (subcritical_index + 1.0),
                (true, false) => subcritical_index = (1.0 - ksrc) / ksrc,
                _ => {}
            }
            if subcritical_index != 0.0 {
                external_source_power_ref = subcritical_index * fission_power;
                external_source_power = external_source_power_ref;
                if intrinsic_gain > 0.0 {
                    external_source_beam_power = external_source_power / intrinsic_gain;
                    external_source_beam_intensity = external_source_beam_power / es.beam_energy;
                }
            }
        }
        let mut beta = 0.0;
        for b in &cfg.betas {
            beta += *b;
        }
        let params = PointKineticsParameters::with_subcritical_index(
            cfg.betas.iter().map(|&b| Ratio::new::<ratio>(b)).collect(),
            cfg.lambdas.iter().map(|&l| Frequency::new::<hertz>(l)).collect(),
            Time::new::<second>(cfg.prompt_generation_time),
            Ratio::new::<ratio>(subcritical_index),
        )?;
        let state = PointKineticsState::new(
            Power::new::<watt>(fission_power),
            precursor_powers.iter().map(|&c| Power::new::<watt>(c)).collect(),
        );
        // fluxes_[i] = (1.0/energyGroups_)*oneGroupFlux_
        let groups = cfg.energy_groups.max(1);
        let inv_groups = 1.0 / (groups as f64);
        let fluxes: Vec<Vec<f64>> = (0..groups)
            .map(|_| initial_flux.iter().map(|&f| inv_groups * f).collect())
            .collect();
        Ok(Self {
            power_old: power,
            p_tot_old: power,
            power,
            fission_power,
            decay_power,
            beta,
            subcritical_index,
            ksrc,
            intrinsic_gain,
            external_source_modulation: 1.0,
            external_source_power,
            external_source_power_ref,
            external_source_beam_power,
            external_source_beam_intensity,
            power_record: Vec::new(),
            total_reactivity: 0.0,
            refs: None,
            steps: 0,
            gem: None,
            gem_reactivity: 0.0,
            gem_level: 0.0,
            read_refs: ReadReferences::default(),
            one_group_flux: initial_flux,
            fluxes,
            power_density,
            secondary_power_density,
            prec: Vec::new(),
            cfg,
            params,
            state,
            mesh,
            form,
        })
    }

    /// The constructor of a RESTART (`pointKineticNeutronics.C:522-654`, the branch "pre-existing flux fields exist in the start time folder"):
    /// `fluxes_` are the `flux0`, `flux1`, ... READ from the start directory (their number is `energyGroups_`), `oneGroupFlux_` is
    /// `0 + flux0 + flux1 + ...` summed in that order (`oneGroupFlux_ *= 0.0; oneGroupFlux_ += fluxi`), and the `prec0`, ... fields read from the
    /// start directory are kept in [`Self::prec`] to be rescaled with the precursor powers (`solvePointKinetics.H`). `initialOneGroupFluxByZone`
    /// is NOT consulted (upstream: "otherwise it is just reconstructed from the sum of those").
    ///
    /// # Errors
    ///
    /// As [`Self::new`]; [`PkModelError::FieldLength`] when a flux or precursor field has the wrong length.
    pub fn from_read_fields(
        mut cfg: PkConfig,
        mesh: PkMeshData,
        fluxes: Vec<Vec<f64>>,
        power_density: Vec<f64>,
        secondary_power_density: Vec<f64>,
        prec: Vec<Vec<f64>>,
        form: LinearSolveForm,
    ) -> Result<Self, PkModelError> {
        let n = mesh.volume.len();
        if fluxes.is_empty() || fluxes.iter().any(|f| f.len() != n) {
            return Err(PkModelError::FieldLength("fluxes"));
        }
        if prec.iter().any(|p| p.len() != n) {
            return Err(PkModelError::FieldLength("prec"));
        }
        cfg.energy_groups = fluxes.len();
        let mut one_group = vec![0.0; n];
        for g in &fluxes {
            for (o, x) in one_group.iter_mut().zip(g) {
                *o += *x;
            }
        }
        let mut me = Self::new(cfg, mesh, one_group, power_density, secondary_power_density, form)?;
        me.fluxes = fluxes;
        me.prec = prec;
        Ok(me)
    }

    /// `reactorState`'s `T*Ref` keys (gh:#1927): used instead of the first-step averages where present.
    #[must_use]
    pub fn with_read_references(mut self, refs: ReadReferences) -> Self {
        self.read_refs = refs;
        self
    }

    /// The GEM branch of the constructor (`pointKineticNeutronics.C:861-882`): `GEMReactivityMap` with more than one knot switches GEM on;
    /// `GEMSodiumLevelRef` is `nuclearData`'s. [`Self::new`] refuses a `gem_map` in the config (it cannot know the reference), this takes it.
    ///
    /// # Errors
    /// [`PkModelError::GemNotDescending`] ("GEMReactivityMap should be indexed by descending sodium level values!").
    pub fn with_gem(mut self, map: Vec<(f64, f64)>, sodium_level_ref: f64) -> Result<Self, PkModelError> {
        if map.len() > 1 {
            for w in map.windows(2) {
                if w[0].0 <= w[1].0 {
                    return Err(PkModelError::GemNotDescending);
                }
            }
            self.gem = Some(GemModel { map, level_ref: sodium_level_ref, int_phi_ref: None });
        }
        Ok(self)
    }

    /// Whether GEM is on (`this->get<bool>("GEM")`).
    pub fn gem_enabled(&self) -> bool {
        self.gem.is_some()
    }

    /// `calcGEMLevelAndReactivity()` (`pointKineticNeutronics.C:1064-1156`), given the face-zone flux `sum_i phi_i |Sf_i|` of the fluid
    /// region AS OF NOW (the caller sums in zone order, as upstream's `forAll(phiFaces_, i)`). The first call also sets `intPhiRef_` (the
    /// reference is the flux of that call). Returns `(GEMSodiumLevel, GEMReactivity)`, also stored for the step's reactivity.
    pub fn compute_gem(&mut self, int_phi: f64) -> (f64, f64) {
        let Some(g) = self.gem.as_mut() else { return (0.0, 0.0) };
        let reference = *g.int_phi_ref.get_or_insert(int_phi);
        let frac = int_phi / reference;
        // `(265.0-539504/(2440.13+sqr(intPhiFrac*100)))/100 - GEMSodiumLevelRef_`
        let x = frac * 100.0;
        let level = (265.0 - 539504.0 / (2440.13 + x * x)) / 100.0 - g.level_ref;
        let n = g.map.len() - 1;
        let mut reactivity = 0.0;
        if level >= g.map[0].0 {
            reactivity = g.map[0].1;
        } else if level <= g.map[n].0 {
            reactivity = g.map[n].1;
        } else {
            for i in 0..n {
                let (x0, y0) = g.map[i];
                let (x1, y1) = g.map[i + 1];
                if level <= x0 && level > x1 {
                    let m = (y1 - y0) / (x1 - x0);
                    reactivity = m * (level - x0) + y0;
                    break;
                }
            }
        }
        self.gem_level = level;
        self.gem_reactivity = reactivity;
        (level, reactivity)
    }

    /// `GEMSodiumLevel` of the last [`Self::compute_gem`].
    pub fn gem_level(&self) -> f64 {
        self.gem_level
    }

    /// `computeFeedbackFieldValues.H`: the flux-squared weighted averages. Every integral is
    /// `gSum(V * field)` with a sequential sum; `oneGroupFluxSqr*T*feedback` is
    /// `(sqr*T)*feedback` per cell.
    ///
    /// # Errors
    ///
    /// [`PkModelError::FieldLength`] when a field has the wrong number of cells.
    pub fn feedback_values(&self, f: &PkFeedbackFields) -> Result<FeedbackValues, PkModelError> {
        let n = self.mesh.volume.len();
        for (name, len) in [
            ("t_fuel", f.t_fuel.len()),
            ("t_clad", f.t_clad.len()),
            ("t_cool", f.t_cool.len()),
            ("rho_cool", f.rho_cool.len()),
            ("t_struct", f.t_struct.len()),
            ("t_struct_mech", f.t_struct_mech.len()),
        ] {
            if len != n {
                return Err(PkModelError::FieldLength(name));
            }
        }
        let sqr: Vec<f64> = self.one_group_flux.iter().map(|&x| x * x).collect();
        let v = &self.mesh.volume;
        let integral_plain = |fb: &[f64]| -> f64 {
            let mut s = 0.0;
            for i in 0..n {
                s += v[i] * (sqr[i] * fb[i]);
            }
            s
        };
        let integral_weighted = |t: &[f64], fb: &[f64]| -> f64 {
            let mut s = 0.0;
            for i in 0..n {
                s += v[i] * ((sqr[i] * t[i]) * fb[i]);
            }
            s
        };
        let fuel = integral_plain(&self.mesh.fuel);
        let cool = integral_plain(&self.mesh.cool);
        let structure = integral_plain(&self.mesh.structure);
        let structure_mech = integral_plain(&self.mesh.structure_mech);
        let driveline = integral_plain(&self.mesh.driveline);
        Ok(FeedbackValues {
            t_fuel: integral_weighted(&f.t_fuel, &self.mesh.fuel) / fuel,
            t_clad: integral_weighted(&f.t_clad, &self.mesh.fuel) / fuel,
            t_cool: integral_weighted(&f.t_cool, &self.mesh.cool) / cool,
            rho_cool: integral_weighted(&f.rho_cool, &self.mesh.cool) / cool,
            t_struct: integral_weighted(&f.t_struct, &self.mesh.structure) / structure,
            t_struct_mech: integral_weighted(&f.t_struct_mech, &self.mesh.structure_mech)
                / structure_mech,
            t_driveline: integral_weighted(&f.t_struct, &self.mesh.driveline) / driveline,
        })
    }

    /// `calcDrivelineReactivity`: piecewise-linear lookup in `controlRodReactivityMap`, zero
    /// outside the map, with the half-open interval of each orientation.
    pub fn driveline_reactivity(&self, expansion: f64) -> f64 {
        let map = &self.cfg.control_rod_map;
        let n = map.len();
        let mut reactivity = 0.0;
        if n > 1 {
            let x_first = map[0].0;
            let x_last = map[n - 1].0;
            for i in 0..n - 1 {
                let (x0, y0) = map[i];
                let (x1, y1) = map[i + 1];
                let inside = if x_first > x_last {
                    expansion <= x0 && expansion > x1
                } else {
                    expansion > x0 && expansion <= x1
                };
                if inside {
                    let m = (y1 - y0) / (x1 - x0);
                    reactivity = m * (expansion - x0) + y0;
                    break;
                }
            }
        }
        reactivity
    }

    /// `correctReactivity.H` for the given averages, references and time (solid fuel:
    /// `precEquilibriumReactivity_` is 0).
    pub fn reactivity_terms(
        &self,
        v: &FeedbackValues,
        r: &FeedbackReferences,
        t: f64,
    ) -> ReactivityTerms {
        self.reactivity_terms_with_prec(v, r, t, 0.0)
    }

    /// `correctReactivity.H` with `precEquilibriumReactivity_` (liquid fuel, gh:#1882; `0.0` is the
    /// solid-fuel value and gives the same bits as before: `0.0 + x == x`).
    pub fn reactivity_terms_with_prec(
        &self,
        v: &FeedbackValues,
        r: &FeedbackReferences,
        t: f64,
        prec_equilibrium_reactivity: f64,
    ) -> ReactivityTerms {
        let c = &self.cfg.coefficients;
        let doppler = if self.cfg.fast_neutrons {
            -c.doppler * (v.t_fuel.max(SMALL) / r.t_fuel.max(SMALL)).r_ln()
        } else {
            c.doppler * (v.t_fuel.sqrt() - r.t_fuel.sqrt())
        };
        let t_fuel = c.t_fuel * (v.t_fuel - r.t_fuel);
        let t_clad = c.t_clad * (v.t_clad - r.t_clad);
        let t_cool = c.t_cool * (v.t_cool - r.t_cool);
        let rho_cool = c.rho_cool * (v.rho_cool - r.rho_cool);
        let t_struct = c.t_struct * (v.t_struct - r.t_struct);
        let t_struct_mech = c.t_struct_mech * (v.t_struct_mech - r.t_struct_mech);
        let driveline_expansion = c.driveline_expansion * (v.t_driveline - r.t_driveline);
        let driveline = self.driveline_reactivity(driveline_expansion);
        let gem = self.gem_reactivity;
        let time = Time::new::<second>(t);
        let external = self
            .cfg
            .external_reactivity
            .as_ref()
            .map_or(0.0, |p| p.value(time));
        let boron = self
            .cfg
            .boron_reactivity
            .as_ref()
            .map_or(0.0, |p| p.value(time));
        let total = prec_equilibrium_reactivity
            + external
            + doppler
            + t_fuel
            + t_clad
            + t_cool
            + rho_cool
            + t_struct
            + t_struct_mech
            + driveline
            + gem
            + boron;
        ReactivityTerms {
            doppler,
            t_fuel,
            t_clad,
            t_cool,
            rho_cool,
            t_struct,
            t_struct_mech,
            driveline_expansion,
            driveline,
            gem,
            external,
            boron,
            total,
        }
    }

    /// `interpolationTable<scalar>::interpolateValue(list, x)` with the default `CLAMP` bounding
    /// (ESI v2506 `interpolationTable.C`), used by `powerMonitoring`.
    pub fn interpolate_value_clamp(list: &[(f64, f64)], lookup: f64) -> f64 {
        let n = list.len();
        if n <= 1 {
            return list[0].1;
        }
        if lookup < list[0].0 {
            return list[0].1;
        }
        if lookup >= list[n - 1].0 {
            return list[n - 1].1;
        }
        let mut lo = 0;
        let mut hi = 0;
        for (i, row) in list.iter().enumerate() {
            if lookup >= row.0 {
                lo = i;
                hi = i;
            } else {
                hi = i;
                break;
            }
        }
        if lo == hi {
            return list[hi].1;
        }
        list[lo].1 + (list[hi].1 - list[lo].1) * (lookup - list[lo].0) / (list[hi].0 - list[lo].0)
    }

    /// One call of `correctPhysics()` (`solvePointKinetics.H`, solid fuel) at time `t` with step
    /// `dt` and the feedback fields of this step.
    ///
    /// # Errors
    ///
    /// Field-length mismatches and the singular-system error of the 0-D core.
    pub fn step(
        &mut self,
        t: f64,
        dt: f64,
        fields: &PkFeedbackFields,
    ) -> Result<PkStepReport, PkModelError> {
        self.p_tot_old = self.power;
        let values = self.feedback_values(fields)?;
        let initialize = self.steps == 0;
        if initialize {
            // `reactorState_.lookupOrDefault("TFuelRef", TFuelValue)` etc. (gh:#1927): the read key when present, else the first-step average
            let r = self.read_refs;
            self.refs = Some(FeedbackReferences {
                t_fuel: r.t_fuel.unwrap_or(values.t_fuel),
                t_clad: r.t_clad.unwrap_or(values.t_clad),
                t_cool: r.t_cool.unwrap_or(values.t_cool),
                rho_cool: r.rho_cool.unwrap_or(values.rho_cool),
                t_struct: r.t_struct.unwrap_or(values.t_struct),
                t_struct_mech: r.t_struct_mech.unwrap_or(values.t_struct_mech),
                t_driveline: r.t_driveline.unwrap_or(values.t_driveline),
            });
        }
        let refs = self.refs.unwrap_or(FeedbackReferences {
            t_fuel: values.t_fuel,
            t_clad: values.t_clad,
            t_cool: values.t_cool,
            rho_cool: values.rho_cool,
            t_struct: values.t_struct,
            t_struct_mech: values.t_struct_mech,
            t_driveline: values.t_driveline,
        });
        let terms = self.reactivity_terms(&values, &refs, t);
        self.total_reactivity = terms.total;

        // External source power.
        if let Some(es) = self.cfg.external_source.clone() {
            match es.mode {
                ExternalSourceMode::Transient => {
                    if let Some(p) = es.modulation_profile.as_ref() {
                        self.external_source_modulation = p.value(Time::new::<second>(t));
                    }
                }
                ExternalSourceMode::PowerMonitoring => {
                    self.power_record.push((t, self.power));
                    let power_delayed = Self::interpolate_value_clamp(
                        &self.power_record,
                        t - es.power_modulation_delay,
                    );
                    let target = self.cfg.p_target;
                    self.external_source_modulation = self.external_source_modulation
                        * (1.0 + es.modulation_factor * (target - power_delayed) / target * dt);
                }
            }
            self.external_source_power =
                self.external_source_power_ref * self.external_source_modulation;
            if self.intrinsic_gain > 0.0 {
                self.external_source_beam_power = self.external_source_power / self.intrinsic_gain;
                self.external_source_beam_intensity =
                    self.external_source_beam_power / es.beam_energy;
            }
        } else {
            self.external_source_power = 0.0;
        }

        if let Some(p) = self.cfg.decay_power.as_ref() {
            self.decay_power = p.value(Time::new::<second>(t));
        }

        // Old-time values (the time index changed).
        self.power_old = self.power;
        let fission_power_old = self.fission_power;
        let precursor_powers_old = self.precursor_powers();
        self.state.step_with(
            &self.params,
            Time::new::<second>(dt),
            Ratio::new::<ratio>(terms.total),
            Power::new::<watt>(self.external_source_power),
            self.form,
        )?;
        self.fission_power = self.state.fission_power().get::<watt>();
        self.power = self.fission_power + self.decay_power;

        let flux_scale = self.fission_power / fission_power_old;
        for g in self.fluxes.iter_mut() {
            for x in g.iter_mut() {
                *x *= flux_scale;
            }
        }
        for x in self.one_group_flux.iter_mut() {
            *x *= flux_scale;
        }
        let power_density_scale = self.power / self.power_old;
        for x in self.power_density.iter_mut() {
            *x *= power_density_scale;
        }
        for x in self.secondary_power_density.iter_mut() {
            *x *= power_density_scale;
        }
        // `prec_` fields read at a restart: `precursori *= precursorPowers_[i]/precursorPowersOld_[i]` where the old power is not 0
        let precursor_powers_new = self.precursor_powers();
        for (i, field) in self.prec.iter_mut().enumerate() {
            if precursor_powers_old[i] != 0.0 {
                let factor = precursor_powers_new[i] / precursor_powers_old[i];
                for x in field.iter_mut() {
                    *x *= factor;
                }
            }
        }
        self.steps += 1;
        Ok(PkStepReport {
            values,
            terms,
            power_density_scale,
            flux_scale,
        })
    }

    /// `pointKineticNeutronics::maxDeltaT()` with the `controlDict` keys `maxDeltaT` (default
    /// `GREAT`) and `maxPowerVariation` (default 0.025) passed in.
    pub fn max_delta_t(
        &self,
        delta_t: f64,
        control_max_delta_t: Option<f64>,
        max_power_variation: Option<f64>,
    ) -> f64 {
        let new_delta_t = control_max_delta_t.unwrap_or(GREAT);
        let max_power_variation = max_power_variation.unwrap_or(0.025);
        let p_tot = self.power;
        let power_variation = ((p_tot - self.p_tot_old) / (self.p_tot_old + SMALL)).abs();
        let max_fact = (max_power_variation / (power_variation + SMALL)).abs();
        let fact = max_fact.min(1.0 + 0.1 * max_fact).min(1.2);
        (fact * delta_t).min(new_delta_t)
    }

    /// Total power `power_` (W).
    pub fn power(&self) -> f64 {
        self.power
    }

    /// Fission power `fissionPower_` (W).
    pub fn fission_power(&self) -> f64 {
        self.fission_power
    }

    /// Decay power `decayPower_` (W).
    pub fn decay_power(&self) -> f64 {
        self.decay_power
    }

    /// Precursor powers `precursorPowers_` (W).
    pub fn precursor_powers(&self) -> Vec<f64> {
        self.state
            .precursor_powers()
            .iter()
            .map(|p| p.get::<watt>())
            .collect()
    }

    /// `totalReactivity_`.
    pub fn total_reactivity(&self) -> f64 {
        self.total_reactivity
    }

    /// Total delayed fraction `beta_` (sequential sum of `Beta`).
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// `subcriticalIndex_`.
    pub fn subcritical_index(&self) -> f64 {
        self.subcritical_index
    }

    /// `ksrc_`.
    pub fn ksrc(&self) -> f64 {
        self.ksrc
    }

    /// `intrinsicGain_`.
    pub fn intrinsic_gain(&self) -> f64 {
        self.intrinsic_gain
    }

    /// `(externalSourcePower_, externalSourceModulation_, beam power, beam intensity)`.
    pub fn external_source(&self) -> (f64, f64, f64, f64) {
        (
            self.external_source_power,
            self.external_source_modulation,
            self.external_source_beam_power,
            self.external_source_beam_intensity,
        )
    }

    /// `externalSourcePowerRef_`.
    pub fn external_source_power_ref(&self) -> f64 {
        self.external_source_power_ref
    }

    /// The references `T*Ref_` once the first step has run.
    pub fn references(&self) -> Option<FeedbackReferences> {
        self.refs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_cfg() -> PkConfig {
        PkConfig {
            prompt_generation_time: 1.0e-4,
            betas: vec![0.0065],
            lambdas: vec![0.08],
            fast_neutrons: true,
            coefficients: FeedbackCoefficients {
                doppler: 0.0,
                t_fuel: -1.0e-5,
                t_clad: 0.0,
                t_cool: 0.0,
                rho_cool: 0.0,
                t_struct: 0.0,
                t_struct_mech: 0.0,
                driveline_expansion: 0.0,
            },
            control_rod_map: vec![(0.01, -1.0e-3), (0.0, 0.0), (-0.01, 1.0e-3)],
            gem_map: vec![],
            external_reactivity: None,
            boron_reactivity: None,
            decay_power: None,
            p_target: 1.0e6,
            subcritical_index: None,
            ksrc: None,
            precursor_powers: None,
            external_source: None,
            energy_groups: 2,
            start_time: 0.0,
        }
    }

    fn tiny_mesh() -> PkMeshData {
        PkMeshData {
            volume: vec![1.0, 2.0],
            fuel: feedback_cell_field(2, &[]),
            cool: feedback_cell_field(2, &[]),
            structure: feedback_cell_field(2, &[]),
            structure_mech: feedback_cell_field(2, &[]),
            driveline: feedback_cell_field(2, &[]),
        }
    }

    fn fields(t: f64) -> PkFeedbackFields {
        PkFeedbackFields {
            t_fuel: vec![t, t],
            t_clad: vec![t, t],
            t_cool: vec![t, t],
            rho_cool: vec![3000.0, 3000.0],
            t_struct: vec![t, t],
            t_struct_mech: vec![t, t],
        }
    }

    #[test]
    fn critical_start_holds_and_hot_fuel_lowers_power() {
        let mut m = PointKineticNeutronics::new(
            tiny_cfg(),
            tiny_mesh(),
            vec![1.0, 1.0],
            vec![1.0, 1.0],
            vec![0.0, 0.0],
            LinearSolveForm::OpenFoamGaussian,
        )
        .unwrap();
        let r0 = m.step(0.1, 0.1, &fields(900.0)).unwrap();
        assert_eq!(r0.terms.total, 0.0);
        let p0 = m.power();
        assert!((p0 - 1.0e6).abs() < 1.0e-3);
        let r1 = m.step(0.2, 0.1, &fields(950.0)).unwrap();
        assert!(r1.terms.total < 0.0);
        assert!(m.power() < p0);
        assert!(m.max_delta_t(0.1, None, None) > 0.0);
    }

    #[test]
    fn driveline_map_orientations_and_clamp_lookup() {
        let m = PointKineticNeutronics::new(
            tiny_cfg(),
            tiny_mesh(),
            vec![1.0, 1.0],
            vec![0.0, 0.0],
            vec![0.0, 0.0],
            LinearSolveForm::OpenFoamGaussian,
        )
        .unwrap();
        assert_eq!(m.driveline_reactivity(0.005), -5.0e-4);
        assert_eq!(m.driveline_reactivity(0.5), 0.0);
        let tab = [(0.0, 1.0), (1.0, 3.0)];
        assert_eq!(PointKineticNeutronics::interpolate_value_clamp(&tab, -1.0), 1.0);
        assert_eq!(PointKineticNeutronics::interpolate_value_clamp(&tab, 0.5), 2.0);
        assert_eq!(PointKineticNeutronics::interpolate_value_clamp(&tab, 2.0), 3.0);
    }

    #[test]
    fn gem_level_follows_the_flow_fraction_and_the_map_interpolates_clamps_and_checks_order() {
        let pk = || {
            PointKineticNeutronics::new(tiny_cfg(), tiny_mesh(), vec![1.0, 1.0], vec![0.0, 0.0], vec![0.0, 0.0], LinearSolveForm::OpenFoamGaussian).expect("class")
        };
        let mut g = pk().with_gem(vec![(0.5, 1.0e-4), (0.0, 0.0), (-0.5, -1.0e-4)], 2.2).expect("gem");
        assert!(g.gem_enabled() && !pk().gem_enabled());
        // the first call fixes the reference flux: fraction 1, level (265 - 539504/(2440.13 + 100^2))/100 - 2.2
        let (level, rho) = g.compute_gem(3.0);
        let expected = (265.0 - 539504.0 / (2440.13 + 100.0 * 100.0)) / 100.0 - 2.2;
        assert_eq!(level, expected);
        let m = (0.0 - 1.0e-4) / (0.0 - 0.5);
        assert_eq!(rho, m * (level - 0.5) + 1.0e-4);
        assert_eq!(g.gem_level(), level);
        // half the flux: a lower level, below the map's last knot is clamped to it
        let (low, rho_low) = g.compute_gem(1.5);
        assert!(low < level);
        assert_eq!(rho_low, if low <= -0.5 { -1.0e-4 } else { rho_low });
        let mut clamp = pk().with_gem(vec![(0.01, 5.0), (0.0, 1.0)], 2.2).expect("gem");
        assert_eq!(clamp.compute_gem(1.0).1, 5.0, "the first call's level 0.0163 is above the first knot 0.01: clamped to its reactivity");
        assert_eq!(clamp.compute_gem(0.5).1, 1.0, "half the flux: level -0.64 is below the last knot 0.0: clamped to its reactivity");
        // without GEM nothing is computed
        assert_eq!(pk().compute_gem(1.0), (0.0, 0.0));
        // a map that is not descending, and a one-knot map (no GEM), as upstream
        assert!(matches!(pk().with_gem(vec![(0.0, 0.0), (0.5, 1.0)], 2.2), Err(PkModelError::GemNotDescending)));
        assert!(!pk().with_gem(vec![(0.0, 0.0)], 2.2).expect("one knot").gem_enabled());
    }

    #[test]
    fn read_references_replace_the_first_step_averages() {
        let mut pk = PointKineticNeutronics::new(tiny_cfg(), tiny_mesh(), vec![1.0, 1.0], vec![0.0, 0.0], vec![0.0, 0.0], LinearSolveForm::OpenFoamGaussian)
            .expect("class")
            .with_read_references(ReadReferences { t_fuel: Some(500.0), ..ReadReferences::default() });
        pk.step(0.0, 1e-3, &fields(600.0)).expect("step");
        let r = pk.references().expect("references");
        assert_eq!((r.t_fuel, r.t_clad), (500.0, 600.0));
    }

    #[test]
    fn gem_is_refused_by_name() {
        let mut cfg = tiny_cfg();
        cfg.gem_map = vec![(0.1, 0.0), (0.0, 1.0e-3)];
        let r = PointKineticNeutronics::new(
            cfg,
            tiny_mesh(),
            vec![1.0, 1.0],
            vec![0.0, 0.0],
            vec![0.0, 0.0],
            LinearSolveForm::OpenFoamGaussian,
        );
        assert!(matches!(r, Err(PkModelError::GemNotPorted)));
    }
}
