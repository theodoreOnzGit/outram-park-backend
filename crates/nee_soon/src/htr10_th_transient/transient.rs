// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The coupled transient: loop + core + kinetics, and the LOFC ATWS case
//! with its named sensitivity cases (gh:#1151).
//!
//! # Step order (dt)
//!
//! 1. kinetics: backward-Euler PRKE with the reactivity of the core's
//!    start-of-step temperatures (explicit feedback), then decay heat and
//!    xenon at the end-of-step fission rate;
//! 2. loop: speed, baffle, isolation, momentum against the core's
//!    start-of-step resistance and gravity head; SG; cold-nozzle temperature;
//! 3. core: backward Euler of all solid nodes with the end-of-step power
//!    and the new flow.
//!
//! # Initial state (D4: as-run)
//!
//! 3315 kW ([J09] Tab 2), RPV inlet 212 degC ([J09] Tab 2, [C09] Tab 1),
//! 2476 kPa ([J09] Tab 2), 1.457 kg/s (Derived, sheet Sec 6). The core is
//! solved to steady state at that point, the kinetics is made critical at
//! the resulting temperatures, and the circulator head is set to hold the
//! flow.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::f64::{MassRate, Power, Pressure, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use tampines::htr10_primary_loop::{
    BypassSplit, CoastdownModel, CoreBoundary, CoreModel, CoreResponse, LumpedPebbleBedCore, PrimaryLoop,
    PrimaryLoopConfig, SteamGeneratorModel,
};
use tampines::TampinesError;
use super::kinetics::{Htr10Kinetics, KineticsParameters, ReactivityBreakdown};

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

/// One LOFC case: everything that defines a run.
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
    /// Simulated duration after the trip.
    pub duration: Time,
}

impl LofcCase {
    /// The base case (as-run, D4).
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
        })
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
}

/// One sample of a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientSample {
    /// Time after the trip, s.
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
}

impl CoupledTransient {
    /// Build at the case's initial state with the lumped stand-in core and
    /// the prescribed-SG boundary (see [`SteamGeneratorModel`]).
    ///
    /// # Errors
    /// Initialisation errors from the core, loop or kinetics.
    pub fn new(case: &LofcCase) -> Result<Self, TampinesError> {
        let core = CoreModel::LumpedStandIn(LumpedPebbleBedCore::htr10(
            case.bypass,
            ThermodynamicTemperature::new::<kelvin>(700.0),
        )?);
        let sg = SteamGeneratorModel::PrescribedHeliumOutlet(case.initial.inlet_temperature);
        Self::with_parts(case, core, sg)
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

    /// Advance by `dt`.
    ///
    /// # Errors
    /// Any sub-model error.
    pub fn step(&mut self, dt: Time) -> Result<TransientSample, TampinesError> {
        let temps = self.core.feedback_temperatures();
        let rho = self.kinetics.step(dt, &temps)?;
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
        self.time += dt.get::<second>();
        Ok(self.sample(rho))
    }

    fn sample(&self, rho: ReactivityBreakdown) -> TransientSample {
        let f = self.last.feedback;
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
}

/// Run a LOFC case: 0.1 s steps for the first 1000 s, 0.5 s after; one
/// sample per second (plus `t = 0`).
///
/// # Errors
/// As [`CoupledTransient::step`].
pub fn run_lofc(case: &LofcCase) -> Result<(CoupledTransient, Vec<TransientSample>), TampinesError> {
    let mut sim = CoupledTransient::new(case)?;
    let mut trace = vec![sim.sample(sim.kinetics.reactivity(&sim.core.feedback_temperatures()))];
    let end = case.duration.get::<second>();
    let mut next_sample = 1.0;
    while sim.time < end - 1e-9 {
        let dt = if sim.time < 1000.0 { 0.1 } else { 0.5 };
        let s = sim.step(Time::new::<second>(dt))?;
        if s.time_s >= next_sample - 1e-9 {
            trace.push(s);
            next_sample += 1.0;
        }
    }
    Ok((sim, trace))
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
    let t_end = trace.last().map_or(0.0, |s| s.time_s);
    let late: Vec<f64> = trace.iter().filter(|s| s.time_s >= t_end - 1800.0).map(|s| s.total_power_w).collect();
    LofcMetrics {
        time_to_one_percent_s: one,
        min_reactivity: (smin.reactivity.total(), smin.time_s),
        reactivity_zero_crossing_s: zero,
        power_return_s: ret,
        first_peak: peak,
        late_total_power_w: late.iter().sum::<f64>() / late.len().max(1) as f64,
    }
}
