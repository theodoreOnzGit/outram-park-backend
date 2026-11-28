// SPDX-License-Identifier: AGPL-3.0-only
//! The irradiation-history driver: walk a piecewise-linear history of
//! fluence, pressure and temperature, and record the stresses and failure
//! probabilities.
//!
//! The gas pressure is an input of the history, not computed here. For an
//! ideal gas in the buffer void, Eq (13.15) of the BISON Theory Manual,
//! `P = nRT/V`, is the law `crate::fuel_failure::pressure::internal_gas_pressure`
//! already implements (from PANAMA-I's Eq (3), the same ideal-gas law), and
//! it is the intended source of the pressure column. A coupled release and
//! CO model is not part of this module.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::mechanics::{LoadStep, ParticleSolver, StepTemperature};
use super::weibull::{layer_failure_probability, WeibullParameters, WeibullStressMeasure};
use super::ParticleMechanicsError;

/// One corner of a piecewise-linear history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistoryPoint {
    /// Cumulative fast fluence, n/m².
    pub fast_fluence_m2: f64,
    /// Inner-surface pressure, Pa.
    pub inner_pressure_pa: f64,
    /// Outer-surface pressure, Pa.
    pub outer_pressure_pa: f64,
    /// Uniform temperature, K.
    pub temperature_k: f64,
}

/// A layer whose failure probability is tracked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedLayer {
    /// Layer index.
    pub layer: usize,
    /// Its Weibull parameters.
    pub weibull: WeibullParameters,
    /// The stress measure.
    pub measure: WeibullStressMeasure,
}

/// The record after one step.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRecord {
    /// Fluence at the end of the step, n/m².
    pub fast_fluence_m2: f64,
    /// Inner pressure applied, Pa.
    pub inner_pressure_pa: f64,
    /// `[min, max]` tangential stress of every layer, Pa, in layer order.
    pub tangential_stress_range_pa: Vec<[f64; 2]>,
    /// `P_f` of each tracked layer, in the order given.
    pub failure_probability: Vec<f64>,
}

/// Run `history` from the solver's current state, with `steps_per_segment`
/// equal fluence steps between consecutive points (pressure and temperature
/// interpolated linearly in fluence). The first point is applied as one
/// step from the current state.
///
/// # Errors
///
/// [`ParticleMechanicsError::InvalidLoad`] for an empty history, zero steps
/// per segment, or a decreasing fluence; any error of
/// [`ParticleSolver::step`] or [`layer_failure_probability`].
pub fn run_history(
    solver: &mut ParticleSolver,
    history: &[HistoryPoint],
    steps_per_segment: usize,
    tracked: &[TrackedLayer],
) -> Result<Vec<HistoryRecord>, ParticleMechanicsError> {
    if history.is_empty() || steps_per_segment == 0 {
        return Err(ParticleMechanicsError::InvalidLoad(String::from(
            "need at least one history point and one step per segment",
        )));
    }
    let mut steps = vec![history[0]];
    for w in history.windows(2) {
        for k in 1..=steps_per_segment {
            let s = k as f64 / steps_per_segment as f64;
            let lerp = |a: f64, b: f64| a + s * (b - a);
            steps.push(HistoryPoint {
                fast_fluence_m2: lerp(w[0].fast_fluence_m2, w[1].fast_fluence_m2),
                inner_pressure_pa: lerp(w[0].inner_pressure_pa, w[1].inner_pressure_pa),
                outer_pressure_pa: lerp(w[0].outer_pressure_pa, w[1].outer_pressure_pa),
                temperature_k: lerp(w[0].temperature_k, w[1].temperature_k),
            });
        }
    }
    let n_layers = solver.mesh().layers().len();
    let mut out = Vec::with_capacity(steps.len());
    for p in steps {
        let step = LoadStep {
            fast_fluence_m2: p.fast_fluence_m2,
            inner_pressure_pa: p.inner_pressure_pa,
            outer_pressure_pa: p.outer_pressure_pa,
            temperature: StepTemperature::Uniform(p.temperature_k),
        };
        match solver.step(&step) {
            Ok(()) => {}
            Err(err) => return Err(err),
        }
        let mut ranges = Vec::with_capacity(n_layers);
        for l in 0..n_layers {
            match solver.layer_stress_range(l) {
                Ok(r) => ranges.push([r.tangential_min_pa, r.tangential_max_pa]),
                Err(err) => return Err(err),
            }
        }
        let mut pf = Vec::with_capacity(tracked.len());
        for t in tracked {
            match layer_failure_probability(solver, t.layer, &t.weibull, t.measure) {
                Ok(v) => pf.push(v),
                Err(err) => return Err(err),
            }
        }
        out.push(HistoryRecord {
            fast_fluence_m2: p.fast_fluence_m2,
            inner_pressure_pa: p.inner_pressure_pa,
            tangential_stress_range_pa: ranges,
            failure_probability: pf,
        });
    }
    Ok(out)
}
