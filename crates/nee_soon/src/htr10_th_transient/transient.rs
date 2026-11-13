// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The coupled transient: loop + core + kinetics, the LOFC ATWS and CRW
//! ATWS cases and their named sensitivity cases (gh:#1151).
//!
//! # Step order (dt)
//!
//! 1. kinetics: backward-Euler PRKE with the reactivity of the core's
//!    start-of-step temperatures (explicit feedback) plus any external
//!    reactivity at the end of the step (the CRW rod withdrawal), and the
//!    external neutron source; then decay heat and xenon at the end-of-step
//!    fission rate;
//! 2. loop: speed, baffle, isolation, momentum against the core's
//!    start-of-step resistance and gravity head; SG; cold-nozzle temperature;
//! 3. core: the lumped stand-in steps every `dt`; the R-Z core
//!    (`tampines::htr10_primary_loop::RzPorousCore`) sub-cycles at its own step with the time-mean
//!    power.
//!
//! # Initial state (D4: as-run)
//!
//! 3315 kW ([J09] Tab 2), RPV inlet 212 degC ([J09] Tab 2, [C09] Tab 1),
//! 2476 kPa ([J09] Tab 2), 1.457 kg/s (Derived, sheet Sec 6). The core is
//! solved to steady state at that point, the kinetics is made critical at
//! the resulting temperatures, and the circulator head is set to hold the
//! flow.
//!
//! # CRW ATWS (input sheet Sec 8)
//!
//! The external reactivity is [T1] Tab 2.60 (0-128 s, held at 4.9762 mk
//! after). As run (D4 base, `CRW-BASE`): the protection system trips the
//! circulator, closes the baffle and isolates the secondary at **12 s**
//! ([H06] Tab 2, Sec 4.2); the baffle closure stops the loop flow at once.
//! The specification (`CRW-TRIP9`, [T1] Sec 2.4.1) stops the flow
//! instantly at 9 s.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use outram_foam_appbuilder_lib::htr10_rz::{Thermocouple, TransientCoreOptions};
use uom::si::f64::{MassRate, Power, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::kinetics::{Htr10Kinetics, KineticsParameters, ReactivityBreakdown};
use tampines::htr10_primary_loop::data::TabulatedSeries;
use tampines::htr10_primary_loop::{
    BypassSplit, CoastdownModel, CoreBoundary, CoreModel, CoreResponse, LumpedPebbleBedCore, PrimaryLoop,
    PrimaryLoopConfig, RzCoreTimeStep, RzPorousCore, SteamGeneratorModel,
};
use tampines::TampinesError;

/// The initial operating point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitialState {
    /// Total thermal power.
    pub power: Power,
    /// RPV inlet (cold nozzle) helium temperature.
    pub inlet_temperature: ThermodynamicTemperature,
    /// Primary pressure.
    pub pressure: Pressure,
    /// Loop mass flow.
    pub mass_flow: MassRate,
}

impl InitialState {
    /// As-run (D4): 3315 kW, 212 degC, 2476 kPa, 1.457 kg/s.
    pub fn as_run() -> Self {
        Self {
            power: Power::new::<watt>(3.315e6),
            inlet_temperature: ThermodynamicTemperature::new::<kelvin>(485.15),
            pressure: Pressure::new::<pascal>(2.476e6),
            mass_flow: MassRate::new::<kilogram_per_second>(1.457),
        }
    }
    /// `TR-HU`: [H06] 3000 kW, 215 degC, 2500 kPa; flow by energy balance
    /// to 650 degC (1.328 kg/s, sheet Sec 6).
    pub fn hu2006() -> Self {
        Self {
            power: Power::new::<watt>(3.0e6),
            inlet_temperature: ThermodynamicTemperature::new::<kelvin>(488.15),
            pressure: Pressure::new::<pascal>(2.5e6),
            mass_flow: MassRate::new::<kilogram_per_second>(3.0e6 / (5195.0 * 435.0)),
        }
    }
    /// `TR-SPEC`: [T1] 3 MW, 250 degC, 2.5 MPa, flow 1.444 kg/s (sheet Sec 6).
    pub fn spec() -> Self {
        Self {
            power: Power::new::<watt>(3.0e6),
            inlet_temperature: ThermodynamicTemperature::new::<kelvin>(523.15),
            pressure: Pressure::new::<pascal>(2.5e6),
            mass_flow: MassRate::new::<kilogram_per_second>(3.0e6 / (5195.0 * 400.0)),
        }
    }
}

/// Which core model a case runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoreChoice {
    /// The lumped stand-in of part A.
    LumpedStandIn,
    /// The R-Z porous core of part B. Its bypass split is replaced by the
    /// case's [`LofcCase::bypass`].
    RzPorous {
        /// The R-Z model options.
        options: TransientCoreOptions,
        /// Its own time step.
        time_step: RzCoreTimeStep,
    },
}

/// One transient case (LOFC or CRW): everything that defines a run.
#[derive(Debug, Clone, PartialEq)]
pub struct LofcCase {
    /// The case name (input sheet Sec 9.2).
    pub name: String,
    /// Initial point.
    pub initial: InitialState,
    /// Kinetics.
    pub kinetics: KineticsParameters,
    /// Bypass split.
    pub bypass: BypassSplit,
    /// Loop (coastdown, baffle, ...).
    pub loop_config: PrimaryLoopConfig,
    /// Simulated duration after the start.
    pub duration: Time,
    /// The core model.
    pub core: CoreChoice,
    /// Externally inserted reactivity, `dk/k` against time since the start
    /// (`None` in the LOFC).
    pub external_reactivity: Option<TabulatedSeries>,
}

impl LofcCase {
    /// The base case (as-run, D4) with the lumped stand-in (part A).
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn base() -> Result<Self, TampinesError> {
        let initial = InitialState::as_run();
        let mut loop_config = PrimaryLoopConfig::htr10_lofc_as_run()?;
        loop_config.pressure = initial.pressure;
        Ok(Self {
            name: "BASE".into(),
            initial,
            kinetics: KineticsParameters::htr10_base(),
            bypass: BypassSplit::tecdoc1694(),
            loop_config,
            duration: Time::new::<second>(10_800.0),
            core: CoreChoice::LumpedStandIn,
            external_reactivity: None,
        })
    }

    /// The LOFC base case with the R-Z core (part B): `RZ-BASE`.
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn rz_base() -> Result<Self, TampinesError> {
        let mut c = Self::base()?.named("RZ-BASE");
        c.core = CoreChoice::RzPorous {
            options: TransientCoreOptions::htr10_lofc(),
            time_step: RzCoreTimeStep::base(),
        };
        Ok(c)
    }

    /// The CRW ATWS base (as-run, D4; module docs): `CRW-BASE`, R-Z core,
    /// Tab 2.61 shape, Tab 2.60 reactivity, trip at 12 s.
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn crw_base() -> Result<Self, TampinesError> {
        let mut c = Self::rz_base()?.named("CRW-BASE");
        c.core = CoreChoice::RzPorous {
            options: TransientCoreOptions::htr10_crw(),
            time_step: RzCoreTimeStep::base(),
        };
        c.loop_config.trip_time = Some(Time::new::<second>(12.0));
        c.loop_config.baffle_close_after_trip = Some(Time::new::<second>(0.0));
        c.loop_config.isolation_after_trip = Some(Time::new::<second>(0.0));
        c.external_reactivity = Some(crw_reactivity()?);
        Ok(c)
    }

    /// `CRW-TRIP9`: the specification's instantaneous flow stop and
    /// isolation at 9 s ([T1] Sec 2.4.1).
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn crw_trip9() -> Result<Self, TampinesError> {
        let mut c = Self::crw_base()?.named("CRW-TRIP9").with_coastdown(CoastdownModel::Instantaneous);
        c.loop_config.trip_time = Some(Time::new::<second>(9.0));
        Ok(c)
    }

    /// Rename (builder).
    pub fn named(mut self, name: &str) -> Self {
        self.name = name.into();
        self
    }

    /// Replace the coastdown (builder).
    pub fn with_coastdown(mut self, c: CoastdownModel) -> Self {
        self.loop_config.coastdown = c;
        self
    }

    /// Replace the initial state, keeping the loop pressure consistent.
    pub fn with_initial(mut self, s: InitialState) -> Self {
        self.initial = s;
        self.loop_config.pressure = s.pressure;
        self
    }

    /// Change the R-Z core options (no-op for the lumped stand-in).
    pub fn with_rz_options(mut self, f: impl Fn(TransientCoreOptions) -> TransientCoreOptions) -> Self {
        if let CoreChoice::RzPorous { options, time_step } = self.core {
            self.core = CoreChoice::RzPorous { options: f(options), time_step };
        }
        self
    }

    /// Change the R-Z core time step (no-op for the lumped stand-in).
    pub fn with_rz_time_step(mut self, step: RzCoreTimeStep) -> Self {
        if let CoreChoice::RzPorous { options, .. } = self.core {
            self.core = CoreChoice::RzPorous { options, time_step: step };
        }
        self
    }
}

/// [T1] Tab 2.60 as `dk/k` against s (mk / 1000), held after 128 s.
///
/// # Errors
/// Embedded-table parse errors.
pub fn crw_reactivity() -> Result<TabulatedSeries, TampinesError> {
    let raw = TabulatedSeries::from_csv_columns(super::data::CRW_REACTIVITY_TAB2_60, 0, 1)?;
    TabulatedSeries::new(raw.points().iter().map(|(t, r)| (*t, r * 1.0e-3)).collect())
}

/// One sample of a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientSample {
    /// Time after the start, s.
    pub time_s: f64,
    /// Total power, W.
    pub total_power_w: f64,
    /// Immediate fission power, W.
    pub fission_power_w: f64,
    /// Decay power, W.
    pub decay_power_w: f64,
    /// Reactivity parts.
    pub reactivity: ReactivityBreakdown,
    /// Loop flow, kg/s.
    pub mass_flow_kg_s: f64,
    /// Relative circulator speed.
    pub speed: f64,
    /// Power-weighted fuel, moderator; reflector, K.
    pub fuel_k: f64,
    /// See `fuel_k`.
    pub moderator_k: f64,
    /// See `fuel_k`.
    pub reflector_k: f64,
    /// Hot-nozzle helium, K.
    pub core_outlet_k: f64,
    /// Heat to the RCCS, W.
    pub heat_to_rccs_w: f64,
    /// Heat to the coolant, W.
    pub heat_to_coolant_w: f64,
    /// In-vessel natural circulation: upward mass flow across the bed
    /// mid-height, kg/s (R-Z core only; 0 for the stand-in).
    pub bed_upflow_kg_s: f64,
}

/// Summary numbers of a LOFC run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LofcMetrics {
    /// First time the fission power is at or below 1 % of its initial value.
    pub time_to_one_percent_s: Option<f64>,
    /// Most negative total reactivity, dk/k, and its time.
    pub min_reactivity: (f64, f64),
    /// First upward zero crossing of the total reactivity after its minimum.
    pub reactivity_zero_crossing_s: Option<f64>,
    /// First time after the 1 % point that the fission power climbs back
    /// above 1 % of its initial value (the measured-trace definition).
    pub power_return_s: Option<f64>,
    /// Largest relative fission power after the return, and its time.
    pub first_peak: Option<(f64, f64)>,
    /// Mean total power over the last 1800 s, W.
    pub late_total_power_w: f64,
}

/// Summary numbers of a CRW run (input sheet Sec 8.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrwMetrics {
    /// Largest relative fission power in the first 300 s, and its time.
    pub first_peak: (f64, f64),
    /// That peak's total power, W.
    pub first_peak_total_w: f64,
    /// First time after the peak that the fission power is below the decay
    /// power ("power to decay level").
    pub decay_level_s: Option<f64>,
    /// Most negative total reactivity after the peak, dk/k, and its time.
    pub min_reactivity: (f64, f64),
    /// First upward zero crossing of the total reactivity after its
    /// minimum (re-criticality).
    pub recriticality_s: Option<f64>,
    /// Largest relative fission power after the re-criticality, and its
    /// time.
    pub recriticality_peak: Option<(f64, f64)>,
    /// Total power at 7200 s, W (or the last sample).
    pub power_at_7200_w: f64,
    /// Mean total power over the last 1800 s, W.
    pub late_total_power_w: f64,
}

/// Loop + core + kinetics.
#[derive(Debug)]
pub struct CoupledTransient {
    loop_: PrimaryLoop,
    core: CoreModel,
    kinetics: Htr10Kinetics,
    pressure: Pressure,
    last: CoreResponse,
    time: f64,
    initial_fission_w: f64,
    initial_response: CoreResponse,
    external: Option<TabulatedSeries>,
}

impl CoupledTransient {
    /// Build at the case's initial state with the case's core and the
    /// prescribed-SG boundary (see [`SteamGeneratorModel`]).
    ///
    /// # Errors
    /// Initialisation errors from the core, loop or kinetics.
    pub fn new(case: &LofcCase) -> Result<Self, TampinesError> {
        let core = match case.core {
            CoreChoice::LumpedStandIn => CoreModel::LumpedStandIn(LumpedPebbleBedCore::htr10(
                case.bypass,
                ThermodynamicTemperature::new::<kelvin>(700.0),
            )?),
            CoreChoice::RzPorous { mut options, time_step } => {
                let b = case.bypass;
                options.split = outram_foam_appbuilder_lib::htr10_rz::BypassSplit {
                    bed: Ratio::new::<ratio>(b.bed),
                    discharge_tube: Ratio::new::<ratio>(b.discharge_tube),
                    control_rods: Ratio::new::<ratio>(b.control_rods),
                    leakage: Ratio::new::<ratio>(b.leakage),
                };
                CoreModel::RzPorous(RzPorousCore::new(options, time_step)?)
            }
        };
        let sg = SteamGeneratorModel::PrescribedHeliumOutlet(case.initial.inlet_temperature);
        let mut me = Self::with_parts(case, core, sg)?;
        me.external = case.external_reactivity.clone();
        Ok(me)
    }

    /// Build with an explicit core model and steam generator.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn with_parts(case: &LofcCase, mut core: CoreModel, sg: SteamGeneratorModel) -> Result<Self, TampinesError> {
        let s = case.initial;
        let boundary = CoreBoundary {
            mass_flow: s.mass_flow,
            cold_nozzle_temperature: s.inlet_temperature,
            hot_nozzle_temperature: s.inlet_temperature,
            pressure: s.pressure,
        };
        let response = core.steady_state(s.power, &boundary)?;
        let kinetics = Htr10Kinetics::new(case.kinetics, s.power, response.feedback)?;
        let mut loop_ = PrimaryLoop::new(case.loop_config.clone(), sg, s.mass_flow);
        loop_.initialise(&core, s.inlet_temperature, response.outlet_temperature)?;
        let initial_fission_w = kinetics.fission_power().get::<watt>();
        Ok(Self {
            loop_,
            core,
            kinetics,
            pressure: s.pressure,
            last: response,
            time: 0.0,
            initial_fission_w,
            initial_response: response,
            external: case.external_reactivity.clone(),
        })
    }

    /// The core's steady-state response at the initial point.
    pub fn initial_response(&self) -> CoreResponse {
        self.initial_response
    }

    /// The loop.
    pub fn primary_loop(&self) -> &PrimaryLoop {
        &self.loop_
    }

    /// The core.
    pub fn core(&self) -> &CoreModel {
        &self.core
    }

    /// The kinetics.
    pub fn kinetics(&self) -> &Htr10Kinetics {
        &self.kinetics
    }

    /// Time since the start, s.
    pub fn time_s(&self) -> f64 {
        self.time
    }

    /// Advance by `dt`.
    ///
    /// # Errors
    /// Any sub-model error.
    pub fn step(&mut self, dt: Time) -> Result<TransientSample, TampinesError> {
        let temps = self.core.feedback_temperatures();
        let t_end = self.time + dt.get::<second>();
        let ext = self.external.as_ref().map_or(0.0, |s| s.value_at(t_end));
        let rho = self.kinetics.step_with_external(dt, &temps, ext)?;
        let hot = self.last.outlet_temperature;
        let cold = self.loop_.advance(dt, &self.core, hot)?;
        let boundary = CoreBoundary {
            mass_flow: self.loop_.mass_flow(),
            cold_nozzle_temperature: cold,
            hot_nozzle_temperature: self.loop_.sg_outlet_temperature(),
            pressure: self.pressure,
        };
        let power = self.kinetics.total_power();
        self.last = self.core.advance(dt, power, &boundary)?;
        self.time = t_end;
        Ok(self.sample(rho))
    }

    fn sample(&self, rho: ReactivityBreakdown) -> TransientSample {
        let f = self.last.feedback;
        let upflow = match &self.core {
            CoreModel::RzPorous(c) => c
                .last_report()
                .map_or(0.0, |r| r.circulation.bed_upflow.get::<kilogram_per_second>()),
            CoreModel::LumpedStandIn(_) => 0.0,
        };
        TransientSample {
            time_s: self.time,
            total_power_w: self.kinetics.total_power().get::<watt>(),
            fission_power_w: self.kinetics.fission_power().get::<watt>(),
            decay_power_w: self.kinetics.decay_power().get::<watt>(),
            reactivity: rho,
            mass_flow_kg_s: self.loop_.mass_flow().get::<kilogram_per_second>(),
            speed: self.loop_.speed(),
            fuel_k: f.fuel.get::<kelvin>(),
            moderator_k: f.moderator.get::<kelvin>(),
            reflector_k: f.reflector.get::<kelvin>(),
            core_outlet_k: self.last.outlet_temperature.get::<kelvin>(),
            heat_to_rccs_w: self.last.heat_to_rccs.get::<watt>(),
            heat_to_coolant_w: self.last.power_to_coolant.get::<watt>(),
            bed_upflow_kg_s: upflow,
        }
    }

    /// Settle a helical-coil steam generator at the hot-nozzle temperature
    /// `hot` and the current flow for `duration` (0.1 s steps); returns the
    /// helium temperature it then delivers. A no-op for the prescribed SG.
    ///
    /// # Errors
    /// Steam-generator errors.
    pub fn settle_steam_generator(
        &mut self,
        hot: ThermodynamicTemperature,
        duration: Time,
    ) -> Result<ThermodynamicTemperature, TampinesError> {
        self.loop_.settle_steam_generator(hot, duration, Time::new::<second>(0.1))?;
        Ok(self.loop_.sg_outlet_temperature())
    }

    /// Initial immediate fission power, W.
    pub fn initial_fission_power_w(&self) -> f64 {
        self.initial_fission_w
    }

    /// The R-Z core's model value at each thermocouple, deg C (`None` for
    /// the lumped stand-in, which has no positions).
    pub fn thermocouples_c(&self, tcs: &[Thermocouple]) -> Option<Vec<f64>> {
        match &self.core {
            CoreModel::RzPorous(c) => Some(tcs.iter().map(|t| c.thermocouple_c(t)).collect()),
            CoreModel::LumpedStandIn(_) => None,
        }
    }
}

/// Thermocouple readings at one time: `(t_s, deg C per thermocouple)`.
pub type ProbeRow = (f64, Vec<f64>);

/// Run a case: 0.1 s steps for the first 1000 s, 0.5 s after; one sample
/// per second (plus `t = 0`).
///
/// # Errors
/// As [`CoupledTransient::step`].
pub fn run_lofc(case: &LofcCase) -> Result<(CoupledTransient, Vec<TransientSample>), TampinesError> {
    let (sim, trace, _) = run_case(case, &[], 0.0)?;
    Ok((sim, trace))
}

/// As [`run_lofc`], also reading the R-Z core at `probes` every
/// `probe_every_s` seconds (and at `t = 0`; nothing for the stand-in or
/// for `probe_every_s <= 0`).
///
/// # Errors
/// As [`CoupledTransient::step`].
pub fn run_case(
    case: &LofcCase,
    probes: &[Thermocouple],
    probe_every_s: f64,
) -> Result<(CoupledTransient, Vec<TransientSample>, Vec<ProbeRow>), TampinesError> {
    let mut sim = CoupledTransient::new(case)?;
    let rho0 = sim.kinetics.reactivity(&sim.core.feedback_temperatures());
    let mut trace = vec![sim.sample(rho0)];
    let mut rows = Vec::new();
    let probing = probe_every_s > 0.0 && !probes.is_empty();
    if probing {
        if let Some(v) = sim.thermocouples_c(probes) {
            rows.push((0.0, v));
        }
    }
    let end = case.duration.get::<second>();
    let mut next_sample = 1.0;
    let mut next_probe = probe_every_s;
    while sim.time < end - 1e-9 {
        let dt = if sim.time < 1000.0 { 0.1 } else { 0.5 };
        let s = sim.step(Time::new::<second>(dt))?;
        if s.time_s >= next_sample - 1e-9 {
            trace.push(s);
            next_sample += 1.0;
        }
        if probing && s.time_s >= next_probe - 1e-9 {
            if let Some(v) = sim.thermocouples_c(probes) {
                rows.push((s.time_s, v));
            }
            next_probe += probe_every_s;
        }
    }
    Ok((sim, trace, rows))
}

/// The metrics of [`LofcMetrics`] from a trace.
pub fn lofc_metrics(trace: &[TransientSample]) -> LofcMetrics {
    let p0 = trace[0].fission_power_w;
    let one = trace.iter().find(|s| s.fission_power_w <= 0.01 * p0).map(|s| s.time_s);
    let (imin, smin) = trace
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.reactivity.total().total_cmp(&b.1.reactivity.total()))
        .map(|(i, s)| (i, *s))
        .unwrap_or((0, trace[0]));
    let zero = trace[imin..].iter().find(|s| s.reactivity.total() >= 0.0).map(|s| s.time_s);
    let ret = one.and_then(|t1| {
        trace
            .iter()
            .filter(|s| s.time_s > t1)
            .skip_while(|s| s.fission_power_w > 0.01 * p0)
            .find(|s| s.fission_power_w > 0.01 * p0)
            .map(|s| s.time_s)
    });
    let peak = ret.and_then(|tr| {
        trace
            .iter()
            .filter(|s| s.time_s >= tr)
            .max_by(|a, b| a.fission_power_w.total_cmp(&b.fission_power_w))
            .map(|s| (s.fission_power_w / p0, s.time_s))
    });
    LofcMetrics {
        time_to_one_percent_s: one,
        min_reactivity: (smin.reactivity.total(), smin.time_s),
        reactivity_zero_crossing_s: zero,
        power_return_s: ret,
        first_peak: peak,
        late_total_power_w: late_mean(trace),
    }
}

fn late_mean(trace: &[TransientSample]) -> f64 {
    let t_end = trace.last().map_or(0.0, |s| s.time_s);
    let late: Vec<f64> = trace.iter().filter(|s| s.time_s >= t_end - 1800.0).map(|s| s.total_power_w).collect();
    late.iter().sum::<f64>() / late.len().max(1) as f64
}

/// The metrics of [`CrwMetrics`] from a trace.
pub fn crw_metrics(trace: &[TransientSample]) -> CrwMetrics {
    let p0 = trace[0].fission_power_w;
    let early = trace.iter().filter(|s| s.time_s <= 300.0);
    let peak = early
        .max_by(|a, b| a.fission_power_w.total_cmp(&b.fission_power_w))
        .copied()
        .unwrap_or(trace[0]);
    let after: Vec<&TransientSample> = trace.iter().filter(|s| s.time_s > peak.time_s).collect();
    let decay = after.iter().find(|s| s.fission_power_w < s.decay_power_w).map(|s| s.time_s);
    let (imin, smin) = after
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.reactivity.total().total_cmp(&b.1.reactivity.total()))
        .map(|(i, s)| (i, **s))
        .unwrap_or((0, peak));
    let recrit = after.get(imin..).and_then(|v| v.iter().find(|s| s.reactivity.total() >= 0.0)).map(|s| s.time_s);
    let rpeak = recrit.and_then(|tr| {
        // The first local maximum after the re-criticality: the largest
        // value before the power falls below half of its running maximum.
        let mut best: Option<(f64, f64)> = None;
        for s in trace.iter().filter(|s| s.time_s >= tr) {
            let v = s.fission_power_w / p0;
            match best {
                Some((b, _)) if v < 0.5 * b => break,
                Some((b, _)) if v <= b => {}
                _ => best = Some((v, s.time_s)),
            }
        }
        best
    });
    let at = trace.iter().find(|s| s.time_s >= 7200.0 - 1e-9).or(trace.last()).map_or(0.0, |s| s.total_power_w);
    CrwMetrics {
        first_peak: (peak.fission_power_w / p0, peak.time_s),
        first_peak_total_w: peak.total_power_w,
        decay_level_s: decay,
        min_reactivity: (smin.reactivity.total(), smin.time_s),
        recriticality_s: recrit,
        recriticality_peak: rpeak,
        power_at_7200_w: at,
        late_total_power_w: late_mean(trace),
    }
}

/// Model positions of the 17 [C09] LOFC thermocouple traces (labels as in
/// `lofc-thermocouples-c09-figs8-11.csv`), cm, TECDOC `z` downward.
///
/// **[C09] prints no coordinates** (its Fig 3 is a schematic), so every
/// position is **Uncertain**, and was fixed before any model result:
///
/// - `SR1`-`SR12`: [T1] Tab 2.7 thermocouples 1-12 (`r` 193 / 189 / 167 /
///   133 / 117 / 93 cm at `z` 80, then the same at `z` 170). Basis: [C09]
///   Sec 4 has SR1-6 and SR7-12 as two rows "at two different heights"
///   from the carbon brick inward, Fig 3 draws SR1-6 as the upper row, and
///   in both rows the measured initial temperatures rise monotonically from
///   SR1 to SR6 and SR7 to SR12 as Tab 2.7's radii fall.
/// - `TR1_TR4`: [T1] Tab 2.7 thermocouple 13 (`r 60, z -40`), the top
///   reflector just above the core cavity ([C09] Sec 5: "located above the
///   top cavity"). `TR2_TR5` and `TR3_TR6`: read from Fig 3 as the same
///   column (`r ~ 55 cm`) about 60 cm and 95 cm higher (`z -100`, `-135`:
///   upper top reflector and the top carbon brick, which [C09] Sec 5 names
///   for TR3&TR6), +-15 cm.
/// - `BR1_BR3`, `BR2_BR4`: read from Fig 3 just under the cone at `r ~ 78`
///   and `~ 45 cm`, placed at Tab 2.7's under-core height `z 234`. **Which
///   pair is inner is not readable**: BR2&BR4 is put inner because it reads
///   the hotter at `t = 0` (748 against 643 deg C), as the inner of Tab
///   2.7's two `z 234` points (TC 14, `r 40`) is at full power.
pub fn c09_thermocouple_positions() -> Vec<Thermocouple> {
    let mut v: Vec<(String, f64, f64)> = Vec::new();
    for (k, r) in [193.0, 189.0, 167.0, 133.0, 117.0, 93.0].iter().enumerate() {
        v.push((format!("SR{}", k + 1), *r, 80.0));
    }
    for (k, r) in [193.0, 189.0, 167.0, 133.0, 117.0, 93.0].iter().enumerate() {
        v.push((format!("SR{}", k + 7), *r, 170.0));
    }
    v.push(("TR1_TR4".into(), 60.0, -40.0));
    v.push(("TR2_TR5".into(), 55.0, -100.0));
    v.push(("TR3_TR6".into(), 55.0, -135.0));
    v.push(("BR1_BR3".into(), 78.0, 234.0));
    v.push(("BR2_BR4".into(), 45.0, 234.0));
    v.into_iter()
        .enumerate()
        .map(|(i, (label, r, z))| Thermocouple {
            number: i + 1,
            location: label,
            r_cm: r,
            z_cm: z,
            measured_c: f64::NAN,
            inet_c: f64::NAN,
            participants: Vec::new(),
        })
        .collect()
}

