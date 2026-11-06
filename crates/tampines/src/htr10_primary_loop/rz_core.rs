// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The R-Z porous core behind the loop's [`CoreModel`](super::CoreModel)
//! interface (gh:#1151 part B).
//!
//! The physics is `outram-foam-appbuilder-lib`'s
//! [`Htr10RzTransientCore`] (transient solid, quasi-steady porous gas with
//! buoyancy, lagged fuel zones; its module docs). This wrapper only maps
//! the loop interface onto it:
//!
//! - **Sub-cycling.** The caller (the loop and, in `nee_soon`, the kinetics) steps at 0.1-0.5 s; the
//!   R-Z core steps at its own [`RzCoreTimeStep`] with the **time-mean**
//!   power of the interval, so the deposited energy is exact. Between core
//!   steps the feedback temperatures are held (an operator split; its size
//!   is measured in the V&V README's time-step check).
//! - **Flow resistance and gravity head for the loop momentum balance.**
//!   The loop needs them only while it flows (12 s in either test: the
//!   baffle then stops it). Both are **frozen at the initial steady state**:
//!   the resistance as `dp = K W |W|` with `K` from the R-Z gas solve's
//!   bed-stream drop plus the [G02] Tab 1 side-reflector pass (the inlet
//!   chain carries no pressure in the R-Z network), and the gravity head
//!   from [`Htr10RzTransientCore::forced_path_gravity_head`]. Named
//!   assumption; the coastdown test shows the flow follows the speed.
//! - **Inertance**: the same derived in-RPV `sum L/A` as the lumped core.
//! - **Reversed loop flow** is refused (the R-Z inlet chain is built for the
//!   forced direction).
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use petir::mathf::RealMath;
use outram_foam_appbuilder_lib::htr10_rz::{
    CoreStepReport, Htr10RzTransientCore, OperatingPoint, Thermocouple, TransientCoreOptions,
};
use uom::si::f64::{MassRate, Power, Pressure, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::core_model::{CoreBoundary, CoreResponse};
use super::core_model::FeedbackTemperatures;
use super::lumped_core::{BypassSplit, LumpedPebbleBedCore};
use crate::TampinesError;

/// [G02] Tab 1 side-reflector pass: 0.7 kPa at 3.846 kg/s (as the lumped
/// core); scaled with `W^2` here (density held at the steady state).
const DP_SIDE_REFLECTOR_PASS: (f64, f64) = (700.0, 3.846);

fn rz_err(e: impl std::fmt::Display) -> TampinesError {
    TampinesError::Numerical(format!("HTR-10 R-Z core: {e}"))
}

/// The R-Z core's own time step: `early` up to `switch_at` after the start
/// of the transient, `late` after.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RzCoreTimeStep {
    /// Step while the power moves fast.
    pub early: Time,
    /// Step for the slow heat-up and cool-down.
    pub late: Time,
    /// When to switch.
    pub switch_at: Time,
}

impl RzCoreTimeStep {
    /// 1 s to 1000 s, 5 s after (the base; see the V&V README's check).
    pub fn base() -> Self {
        Self {
            early: Time::new::<second>(1.0),
            late: Time::new::<second>(5.0),
            switch_at: Time::new::<second>(1000.0),
        }
    }
}

/// The R-Z porous core with its loop-interface bookkeeping.
#[derive(Debug, Clone)]
pub struct RzPorousCore {
    core: Htr10RzTransientCore,
    step: RzCoreTimeStep,
    pending_s: f64,
    pending_j: f64,
    elapsed_s: f64,
    last: Option<CoreResponse>,
    last_report: Option<CoreStepReport>,
    k_dp: f64,
    gravity_pa: f64,
    inertance: f64,
}

impl RzPorousCore {
    /// Build (no solve yet; [`Self::steady_state`] must follow).
    ///
    /// # Errors
    /// The R-Z model's build errors.
    pub fn new(options: TransientCoreOptions, step: RzCoreTimeStep) -> Result<Self, TampinesError> {
        let core = Htr10RzTransientCore::build(options).map_err(rz_err)?;
        let s = options.split;
        use uom::si::ratio::ratio;
        let lumped = LumpedPebbleBedCore::htr10(
            BypassSplit {
                bed: s.bed.get::<ratio>(),
                discharge_tube: s.discharge_tube.get::<ratio>(),
                control_rods: s.control_rods.get::<ratio>(),
                leakage: s.leakage.get::<ratio>(),
            },
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )?;
        Ok(Self {
            core,
            step,
            pending_s: 0.0,
            pending_j: 0.0,
            elapsed_s: 0.0,
            last: None,
            last_report: None,
            k_dp: 0.0,
            gravity_pa: 0.0,
            inertance: lumped.inertance_per_m(),
        })
    }

    /// The R-Z core.
    pub fn core(&self) -> &Htr10RzTransientCore {
        &self.core
    }

    /// The report of the last R-Z step (or of the steady state).
    pub fn last_report(&self) -> Option<&CoreStepReport> {
        self.last_report.as_ref()
    }

    /// The core's model value at a thermocouple position, deg C (the
    /// gh:#1150 instrument rule).
    pub fn thermocouple_c(&self, tc: &Thermocouple) -> f64 {
        self.core.reading(tc).model_c
    }

    fn operating_point(power: f64, b: &CoreBoundary) -> Result<OperatingPoint, TampinesError> {
        let w = b.mass_flow.get::<kilogram_per_second>();
        if w < 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "the R-Z core does not represent a reversed loop flow ({w} kg/s)"
            )));
        }
        Ok(OperatingPoint {
            power: Power::new::<watt>(power),
            inlet: b.cold_nozzle_temperature,
            pressure: b.pressure,
            mass_flow: MassRate::new::<kilogram_per_second>(w),
        })
    }

    fn response(&self, r: &CoreStepReport) -> CoreResponse {
        CoreResponse {
            outlet_temperature: r.outlet_temperature,
            power_to_coolant: r.heat_to_gas,
            heat_to_rccs: r.heat_to_rccs_and_heads,
            feedback: Self::feedback_of(r),
        }
    }

    fn feedback_of(r: &CoreStepReport) -> FeedbackTemperatures {
        FeedbackTemperatures { fuel: r.feedback.fuel, moderator: r.feedback.moderator, reflector: r.feedback.reflector }
    }

    /// Solve the steady state and freeze the loop-side resistance and
    /// gravity head (module docs).
    ///
    /// # Errors
    /// Solver errors; a reversed flow.
    pub fn steady_state(&mut self, power: Power, boundary: &CoreBoundary) -> Result<CoreResponse, TampinesError> {
        let op = Self::operating_point(power.get::<watt>(), boundary)?;
        let r = self.core.solve_steady(&op).map_err(rz_err)?;
        let w = op.mass_flow.get::<kilogram_per_second>();
        let dp_bed = self.core.gas().map_or(0.0, |g| g.core_pressure_drop);
        let dp_pass = DP_SIDE_REFLECTOR_PASS.0 * (w / DP_SIDE_REFLECTOR_PASS.1).r_powi(2);
        self.k_dp = if w > 0.0 { (dp_bed + dp_pass) / (w * w) } else { 0.0 };
        self.gravity_pa = self.core.forced_path_gravity_head(boundary.pressure).get::<pascal>();
        let resp = self.response(&r);
        self.last = Some(resp);
        self.last_report = Some(r);
        self.pending_s = 0.0;
        self.pending_j = 0.0;
        self.elapsed_s = 0.0;
        Ok(resp)
    }

    /// Accumulate `dt` and `power`; step the R-Z core when the accumulated
    /// time reaches its step (module docs).
    ///
    /// # Errors
    /// Solver errors; a reversed flow; no steady state yet.
    pub fn advance(&mut self, dt: Time, power: Power, boundary: &CoreBoundary) -> Result<CoreResponse, TampinesError> {
        let h = dt.get::<second>();
        self.pending_s += h;
        self.pending_j += power.get::<watt>() * h;
        self.elapsed_s += h;
        let target = if self.elapsed_s <= self.step.switch_at.get::<second>() + 1e-9 {
            self.step.early
        } else {
            self.step.late
        }
        .get::<second>();
        if self.pending_s + 1e-9 >= target {
            let p_mean = self.pending_j / self.pending_s;
            let op = Self::operating_point(p_mean, boundary)?;
            let r = self.core.step(Time::new::<second>(self.pending_s), &op).map_err(rz_err)?;
            self.pending_s = 0.0;
            self.pending_j = 0.0;
            let resp = self.response(&r);
            self.last = Some(resp);
            self.last_report = Some(r);
        }
        self.last.ok_or_else(|| TampinesError::InvalidInput("R-Z core advanced before its steady state".into()))
    }

    /// Feedback temperatures of the last R-Z step.
    pub fn feedback_temperatures(&self) -> FeedbackTemperatures {
        let f = self.core.feedback();
        FeedbackTemperatures { fuel: f.fuel, moderator: f.moderator, reflector: f.reflector }
    }

    /// `K W |W|` (module docs).
    pub fn pressure_drop(&self, mass_flow: MassRate) -> Pressure {
        let w = mass_flow.get::<kilogram_per_second>();
        Pressure::new::<pascal>(self.k_dp * w * w.abs())
    }

    /// The frozen forced-path gravity head (module docs).
    pub fn gravity_head(&self) -> Pressure {
        Pressure::new::<pascal>(self.gravity_pa)
    }

    /// `sum L / A`, 1/m (as the lumped core).
    pub fn inertance_per_m(&self) -> f64 {
        self.inertance
    }
}
