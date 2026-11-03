// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The HTR-10 primary loop outside the RPV: circulator with coastdown,
//! blower baffle (flapper), hot gas duct, steam generator and cold return,
//! closed by one loop momentum balance (gh:#1151).
//!
//! # Momentum
//!
//! One mass flow `W` around the loop (positive in the forced direction),
//! backward Euler:
//!
//! ```text
//! I (W - W_old) / dt = dP_circ(s, rho) - dP_core(W) - dP_duct(W) - dP_sg(W) + B_core + B_ext
//! dP_circ = dP_0 s^2 rho_c / rho_c0           (fan law, flat characteristic)
//! B_ext   = g H_sg (rho_mean,sg - rho_hot)     (hot leg rises H_sg to the SG)
//! ```
//!
//! solved by bisection on the monotone residual. `s` is the circulator speed
//! relative to its pre-trip value, from the [`CoastdownModel`]. `dP_0` is
//! the loop resistance at the initial state minus the gravity heads, i.e.
//! the circulator is set to deliver the as-run flow (an initialisation, not
//! a fit: the operators set the speed that delivered that flow). `I` is the
//! core's derived `sum L/A`; the loop outside the RPV adds an Unknown
//! inertance (duct and SG lengths are not published,
//! `htr10-plant-data.md` Sec 9), so `I` is a lower bound. With
//! `I W / (2 dP) ~ 1e-2 s` the coastdown (tens of seconds) is quasi-static
//! either way, which a test checks by multiplying `I` by 10 (flow moved < 1 %).
//!
//! **No circulator characteristic exists in any source** (plant-data Sec 9:
//! "head-flow characteristic curve" Unknown), so the head is the
//! fan-law-scaled initial head with no flow dependence. With quadratic
//! losses this makes `W` proportional to `s`, which is what [C09] Sec 4
//! reports ("a proportional reduction of the primary coolant mass flow").
//!
//! # Components and where their numbers come from
//!
//! - **Hot gas duct, steam generator shell:** [G02] Tab 1 computed drops at
//!   the rated point (4.1 and 15.0 kPa at 4.32 kg/s, 3.0 MPa), scaled
//!   `dP = dP_ref (W / W_ref)|W / W_ref| rho_ref / rho` (constant loss
//!   coefficient). [`crate::gas_phase::GasDuct`] was considered for the duct
//!   and rejected: the duct length is Unknown (plant-data Sec 4.1).
//! - **Circulator temperature rise:** [`crate::gas_phase::Circulator`] at
//!   the live head, isentropic efficiency 0.8 (assumption; no source gives
//!   one, plant-data Sec 9).
//! - **Steam generator heat transfer:** [`SteamGeneratorModel`]: either the
//!   measured-boundary method of [J09] Sec 2.4 (the helium leaving the SG is
//!   prescribed so the RPV inlet sits at the measured 212 degC; the default,
//!   because the secondary side at 30 % power is not published), or the
//!   workspace's helical-coil exchanger
//!   ([`crate::components::NodalisedCounterFlowSteamGenerator`], illustrative
//!   UA) with a prescribed feedwater.
//! - **Baffle and isolation:** the protection system closed the blower
//!   baffle and isolated the secondary at 12 s ([H06] Tab 1; [C09] Sec 4).
//!   A closed baffle is a closed loop: `W = 0`.
//! - **Gravity head outside the RPV:** the SG thermal centre's height
//!   above the RPV nozzles, `H_sg`, is **Unknown** (plant-data Sec 9,
//!   "Elevation differences between components"). Base 0; the
//!   natural-circulation sensitivity cases set it explicitly.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::f64::{AvailableEnergy, MassRate, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::{bar, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use outram_foam_appbuilder_lib::htgr_closures::KtaHelium;

use super::core_model::CoreModel;
use super::data::TabulatedSeries;
use crate::components::NodalisedCounterFlowSteamGenerator;
use crate::gas_phase::Circulator;
use crate::TampinesError;

const G: f64 = 9.806_65;

fn rho(t_k: f64, p: Pressure) -> Result<f64, TampinesError> {
    Ok(KtaHelium::new(
        Pressure::new::<bar>(p.get::<bar>().clamp(KtaHelium::MIN_PRESSURE_BAR, KtaHelium::MAX_PRESSURE_BAR)),
        ThermodynamicTemperature::new::<kelvin>(
            t_k.clamp(KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K),
        ),
    )?
    .density()
    .get::<kilogram_per_cubic_meter>())
}

/// How the circulator speed (relative to its pre-trip value) falls after the
/// trip at `t = 0`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoastdownModel {
    /// The measured speed, [C09] Fig 4 digitised (`lofc-blower-speed-c09-fig4.csv`).
    /// The as-run base (D4).
    Measured(TabulatedSeries),
    /// `s = 0` and `W = 0` at once: the benchmark specification
    /// ([T1] Sec 2.3.1.1), case `LOFC-SPEC`.
    Instantaneous,
    /// A free-running rotor against a fan-law load (`torque ~ s^2`):
    /// `s = 1 / (1 + t / tau)`, `tau = I_rotor omega_0^2 / P_shaft,0`. The
    /// rotor inertia is Unknown, so `tau` must be supplied and named.
    Inertial {
        /// `tau`.
        time_constant: Time,
    },
    /// The loop flow is prescribed as a fraction of its initial value (no
    /// momentum solve): [J09] Fig 6's TUNED GAMMA+ input, case `LOFC-J09-FLOW`.
    PrescribedFlowFraction(TabulatedSeries),
}

impl CoastdownModel {
    /// Relative speed at `t` (s) after the trip.
    pub fn speed(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 1.0;
        }
        match self {
            Self::Measured(s) => s.value_at(t).max(0.0),
            Self::Instantaneous => 0.0,
            Self::Inertial { time_constant } => 1.0 / (1.0 + t / time_constant.get::<second>()),
            Self::PrescribedFlowFraction(s) => s.value_at(t).max(0.0),
        }
    }

    /// The as-run base: the digitised measured speed.
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn measured_c09() -> Result<Self, TampinesError> {
        let raw = TabulatedSeries::from_csv_columns(super::data::BLOWER_SPEED_C09_FIG4, 0, 1)?;
        // Normalise to the first reading (the trace starts at 0.98 at 0.86 s
        // because the speed already fell in the first pixel column).
        let mut pts = vec![(0.0, 1.0)];
        pts.extend(raw.points().iter().copied().filter(|p| p.0 > 0.0));
        Ok(Self::Measured(TabulatedSeries::new(pts)?))
    }

    /// [J09] Fig 6's flow, normalised by its own initial 1.413 kg/s.
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn j09_flow() -> Result<Self, TampinesError> {
        let raw = TabulatedSeries::from_csv_columns(super::data::FLOW_COASTDOWN_J09_FIG6, 0, 1)?;
        let mut pts = vec![(0.0, 1.0)];
        pts.extend(raw.points().iter().filter(|p| p.0 > 0.0).map(|p| (p.0, p.1 / 1.413)));
        Ok(Self::PrescribedFlowFraction(TabulatedSeries::new(pts)?))
    }
}

/// A component loss with a constant loss coefficient, referenced to a
/// published drop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormLoss {
    /// Drop at the reference point.
    pub reference_drop: Pressure,
    /// Flow at the reference point.
    pub reference_flow: MassRate,
    /// Gas temperature at the reference point (pressure 3.0 MPa).
    pub reference_temperature: ThermodynamicTemperature,
}

impl FormLoss {
    /// Signed drop at flow `w` and gas temperature `t`.
    ///
    /// # Errors
    /// Helium property errors.
    pub fn drop(&self, w: MassRate, t: ThermodynamicTemperature, p: Pressure) -> Result<Pressure, TampinesError> {
        let x = w.get::<kilogram_per_second>() / self.reference_flow.get::<kilogram_per_second>();
        let rho_ref = rho(self.reference_temperature.get::<kelvin>(), Pressure::new::<bar>(30.0))?;
        let r = rho(t.get::<kelvin>(), p)?;
        Ok(self.reference_drop * (x * x.abs() * rho_ref / r))
    }

    /// [G02] Tab 1 hot gas duct: 4.1 kPa at 4.32 kg/s, 700 degC.
    pub fn htr10_hot_gas_duct() -> Self {
        Self {
            reference_drop: Pressure::new::<pascal>(4.1e3),
            reference_flow: MassRate::new::<kilogram_per_second>(4.32),
            reference_temperature: ThermodynamicTemperature::new::<kelvin>(973.15),
        }
    }

    /// [G02] Tab 1 steam generator: 15.0 kPa at 4.32 kg/s, referenced to the
    /// mean of 250 and 700 degC (assumption: the shell-side mean).
    pub fn htr10_steam_generator() -> Self {
        Self {
            reference_drop: Pressure::new::<pascal>(15.0e3),
            reference_flow: MassRate::new::<kilogram_per_second>(4.32),
            reference_temperature: ThermodynamicTemperature::new::<kelvin>(748.15),
        }
    }
}

/// The steam generator as the loop sees it. (The exchanger variant is
/// large; it is held by value, per the workspace's no-`Box` rule.)
#[allow(clippy::large_enum_variant)]
pub enum SteamGeneratorModel {
    /// The helium leaving the SG is held at this temperature: [J09] Sec 2.4's
    /// method of using the measured inlet temperature as the boundary.
    PrescribedHeliumOutlet(ThermodynamicTemperature),
    /// The workspace's helical-coil exchanger with a prescribed feedwater.
    HelicalCoil {
        /// The exchanger.
        exchanger: NodalisedCounterFlowSteamGenerator,
        /// Feedwater enthalpy.
        feedwater_enthalpy: AvailableEnergy,
        /// Feedwater flow before isolation.
        feedwater_flow: MassRate,
    },
}

impl std::fmt::Debug for SteamGeneratorModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrescribedHeliumOutlet(t) => write!(f, "PrescribedHeliumOutlet({} K)", t.get::<kelvin>()),
            Self::HelicalCoil { feedwater_flow, .. } => write!(
                f,
                "HelicalCoil {{ feedwater {} kg/s }}",
                feedwater_flow.get::<kilogram_per_second>()
            ),
        }
    }
}

/// Fixed configuration of the loop.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryLoopConfig {
    /// System pressure (held).
    pub pressure: Pressure,
    /// Hot gas duct loss.
    pub hot_duct: FormLoss,
    /// Steam generator shell loss.
    pub steam_generator_loss: FormLoss,
    /// Circulator isentropic efficiency (assumed 0.8).
    pub circulator_efficiency: Ratio,
    /// Speed after the trip.
    pub coastdown: CoastdownModel,
    /// Trip time (s after the start of the run); `None` = no trip.
    pub trip_time: Option<Time>,
    /// Baffle closure, s after the trip; `None` = never.
    pub baffle_close_after_trip: Option<Time>,
    /// Secondary isolation, s after the trip; `None` = never.
    pub isolation_after_trip: Option<Time>,
    /// Height of the SG thermal centre above the RPV nozzles (Unknown;
    /// base 0).
    pub sg_thermal_centre_height: uom::si::f64::Length,
    /// Multiplier on the derived inertance (1 = derived lower bound).
    pub inertance_multiplier: f64,
}

impl PrimaryLoopConfig {
    /// The as-run LOFC: 2476 kPa ([J09] Tab 2), measured coastdown, trip at
    /// `t = 0`, baffle closure and secondary isolation at 12 s.
    ///
    /// # Errors
    /// Embedded-table parse errors.
    pub fn htr10_lofc_as_run() -> Result<Self, TampinesError> {
        Ok(Self {
            pressure: Pressure::new::<pascal>(2.476e6),
            hot_duct: FormLoss::htr10_hot_gas_duct(),
            steam_generator_loss: FormLoss::htr10_steam_generator(),
            circulator_efficiency: Ratio::new::<ratio>(0.8),
            coastdown: CoastdownModel::measured_c09()?,
            trip_time: Some(Time::new::<second>(0.0)),
            baffle_close_after_trip: Some(Time::new::<second>(12.0)),
            isolation_after_trip: Some(Time::new::<second>(12.0)),
            sg_thermal_centre_height: uom::si::f64::Length::new::<uom::si::length::meter>(0.0),
            inertance_multiplier: 1.0,
        })
    }
}

/// The loop state.
#[derive(Debug)]
pub struct PrimaryLoop {
    config: PrimaryLoopConfig,
    sg: SteamGeneratorModel,
    mass_flow: f64,
    initial_flow: f64,
    speed: f64,
    head0: f64,
    rho_cold0: f64,
    time: f64,
    baffle_closed: bool,
    isolated: bool,
    sg_outlet: f64,
    circulator_rise: f64,
}

impl PrimaryLoop {
    /// Build at the initial flow; [`Self::initialise`] must follow once the
    /// core is in steady state.
    pub fn new(config: PrimaryLoopConfig, sg: SteamGeneratorModel, initial_flow: MassRate) -> Self {
        let w = initial_flow.get::<kilogram_per_second>();
        let t_sg = match &sg {
            SteamGeneratorModel::PrescribedHeliumOutlet(t) => t.get::<kelvin>(),
            SteamGeneratorModel::HelicalCoil { .. } => 485.15,
        };
        Self {
            config,
            sg,
            mass_flow: w,
            initial_flow: w,
            speed: 1.0,
            head0: 0.0,
            rho_cold0: 1.0,
            time: 0.0,
            baffle_closed: false,
            isolated: false,
            sg_outlet: t_sg,
            circulator_rise: 0.0,
        }
    }

    fn external_gravity_head(&self, t_hot: f64) -> Result<f64, TampinesError> {
        let h = self.config.sg_thermal_centre_height.get::<uom::si::length::meter>();
        if h == 0.0 {
            return Ok(0.0);
        }
        let p = self.config.pressure;
        let rho_mean = rho(0.5 * (t_hot + self.sg_outlet), p)?;
        Ok(G * h * (rho_mean - rho(t_hot, p)?))
    }

    fn resistance(&self, core: &CoreModel, w: f64, t_hot: f64) -> Result<f64, TampinesError> {
        let p = self.config.pressure;
        let wq = MassRate::new::<kilogram_per_second>(w);
        let th = ThermodynamicTemperature::new::<kelvin>(t_hot);
        let tsg = ThermodynamicTemperature::new::<kelvin>(0.5 * (t_hot + self.sg_outlet));
        Ok(core.pressure_drop(wq, p)?.get::<pascal>()
            + self.config.hot_duct.drop(wq, th, p)?.get::<pascal>()
            + self.config.steam_generator_loss.drop(wq, tsg, p)?.get::<pascal>())
    }

    /// Set the circulator head so the current flow is a steady state of the
    /// momentum balance with the core as it is now, and set the prescribed
    /// SG outlet so the cold nozzle sits at `cold_nozzle` once the
    /// circulator's temperature rise is added.
    ///
    /// # Errors
    /// Property and circulator errors.
    pub fn initialise(
        &mut self,
        core: &CoreModel,
        cold_nozzle: ThermodynamicTemperature,
        hot_nozzle: ThermodynamicTemperature,
    ) -> Result<(), TampinesError> {
        let p = self.config.pressure;
        let th = hot_nozzle.get::<kelvin>();
        let w = self.mass_flow;
        let b = core.gravity_head(p)?.get::<pascal>() + self.external_gravity_head(th)?;
        self.head0 = self.resistance(core, w, th)? - b;
        self.rho_cold0 = rho(cold_nozzle.get::<kelvin>(), p)?;
        self.circulator_rise = self.circulator_temperature_rise(cold_nozzle.get::<kelvin>(), self.head0)?;
        if let SteamGeneratorModel::PrescribedHeliumOutlet(_) = self.sg {
            self.sg_outlet = cold_nozzle.get::<kelvin>() - self.circulator_rise;
            self.sg = SteamGeneratorModel::PrescribedHeliumOutlet(ThermodynamicTemperature::new::<kelvin>(
                self.sg_outlet,
            ));
        }
        Ok(())
    }

    fn circulator_temperature_rise(&self, t_in: f64, head: f64) -> Result<f64, TampinesError> {
        if head <= 0.0 || self.mass_flow <= 1e-9 {
            return Ok(0.0);
        }
        let c = Circulator::new_fixed_pressure_rise(Pressure::new::<pascal>(head), self.config.circulator_efficiency);
        let r = c.compress_helium(
            MassRate::new::<kilogram_per_second>(self.mass_flow),
            ThermodynamicTemperature::new::<kelvin>(t_in),
            self.config.pressure,
        )?;
        Ok(r.temperature_rise_kelvin)
    }

    fn after_trip(&self, t: f64) -> Option<f64> {
        self.config.trip_time.map(|tt| t - tt.get::<second>()).filter(|x| *x >= 0.0)
    }

    /// Advance the loop by `dt`: speed, baffle, isolation, the momentum
    /// balance against `core` (at its start-of-step temperatures), and the
    /// steam generator against the hot-nozzle temperature `hot_nozzle`.
    /// Returns the cold-nozzle temperature for the core.
    ///
    /// # Errors
    /// Property, circulator and steam-generator errors; a non-bracketing
    /// momentum residual.
    pub fn advance(
        &mut self,
        dt: Time,
        core: &CoreModel,
        hot_nozzle: ThermodynamicTemperature,
    ) -> Result<ThermodynamicTemperature, TampinesError> {
        let h = dt.get::<second>();
        self.time += h;
        let p = self.config.pressure;
        let th = hot_nozzle.get::<kelvin>();
        let since = self.after_trip(self.time);
        self.speed = since.map_or(1.0, |s| self.config.coastdown.speed(s));
        if let (Some(s), Some(b)) = (since, self.config.baffle_close_after_trip) {
            if s >= b.get::<second>() {
                self.baffle_closed = true;
            }
        }
        if let (Some(s), Some(b)) = (since, self.config.isolation_after_trip) {
            if s >= b.get::<second>() {
                self.isolated = true;
            }
        }
        let instantaneous = matches!(self.config.coastdown, CoastdownModel::Instantaneous) && since.is_some();
        if self.baffle_closed || instantaneous {
            self.mass_flow = 0.0;
        } else if let (CoastdownModel::PrescribedFlowFraction(_), Some(_)) = (&self.config.coastdown, since) {
            self.mass_flow = self.speed * self.initial_flow;
        } else {
            let t_cold = self.sg_outlet + self.circulator_rise;
            let head = self.head0 * self.speed * self.speed * rho(t_cold, p)? / self.rho_cold0;
            let b = core.gravity_head(p)?.get::<pascal>() + self.external_gravity_head(th)?;
            let inertance = core.inertance_per_m() * self.config.inertance_multiplier;
            let w_old = self.mass_flow;
            let residual = |w: f64| -> Result<f64, TampinesError> {
                Ok(inertance * (w - w_old) / h - (head + b - self.resistance(core, w, th)?))
            };
            let span = 2.0 * self.initial_flow.abs().max(0.05) + 0.5;
            let (mut lo, mut hi) = (-span, span);
            let (flo, fhi) = (residual(lo)?, residual(hi)?);
            if flo > 0.0 || fhi < 0.0 {
                return Err(TampinesError::Numerical(format!(
                    "loop momentum residual does not bracket: f({lo}) = {flo}, f({hi}) = {fhi}"
                )));
            }
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if residual(mid)? > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            self.mass_flow = 0.5 * (lo + hi);
        }
        // Circulator temperature rise at the live head and flow.
        let head_now = self.head0 * self.speed * self.speed;
        self.circulator_rise = if self.mass_flow > 1e-6 {
            self.circulator_temperature_rise(self.sg_outlet, head_now)?
        } else {
            0.0
        };
        // Steam generator.
        let isolated = self.isolated;
        let w = self.mass_flow;
        if let SteamGeneratorModel::HelicalCoil { exchanger, feedwater_enthalpy, feedwater_flow } = &mut self.sg {
            if !isolated && w > 1e-6 {
                let st = exchanger
                    .advance_timestep(
                        dt,
                        hot_nozzle,
                        MassRate::new::<kilogram_per_second>(w),
                        *feedwater_enthalpy,
                        *feedwater_flow,
                    )
                    .map_err(|e| TampinesError::Numerical(format!("steam generator: {e:?}")))?;
                self.sg_outlet = st.hot_outlet_temperature.get::<kelvin>();
            }
        }
        Ok(ThermodynamicTemperature::new::<kelvin>(self.sg_outlet + self.circulator_rise))
    }

    /// Loop mass flow.
    pub fn mass_flow(&self) -> MassRate {
        MassRate::new::<kilogram_per_second>(self.mass_flow)
    }

    /// Relative circulator speed.
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Whether the baffle has closed.
    pub fn baffle_closed(&self) -> bool {
        self.baffle_closed
    }

    /// Circulator head at full (pre-trip) speed.
    pub fn initial_head(&self) -> Pressure {
        Pressure::new::<pascal>(self.head0)
    }

    /// Helium temperature leaving the SG.
    pub fn sg_outlet_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.sg_outlet)
    }

    /// Circulator temperature rise at the current state, K.
    pub fn circulator_rise_kelvin(&self) -> f64 {
        self.circulator_rise
    }

    /// Settle a helical-coil SG at fixed hot inlet and flow for `duration`;
    /// no-op for the prescribed model.
    ///
    /// # Errors
    /// Steam-generator errors.
    pub fn settle_steam_generator(
        &mut self,
        hot_nozzle: ThermodynamicTemperature,
        duration: Time,
        dt: Time,
    ) -> Result<(), TampinesError> {
        let w = self.mass_flow;
        if let SteamGeneratorModel::HelicalCoil { exchanger, feedwater_enthalpy, feedwater_flow } = &mut self.sg {
            let n = (duration.get::<second>() / dt.get::<second>()).ceil() as usize;
            for _ in 0..n {
                let st = exchanger
                    .advance_timestep(dt, hot_nozzle, MassRate::new::<kilogram_per_second>(w), *feedwater_enthalpy, *feedwater_flow)
                    .map_err(|e| TampinesError::Numerical(format!("steam generator: {e:?}")))?;
                self.sg_outlet = st.hot_outlet_temperature.get::<kelvin>();
            }
        }
        Ok(())
    }
}
