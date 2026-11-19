// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream file, read line for line: openmc/deplete/helpers.py
//     (EnergyNormalizationHelper.factor, ChainFissionHelper.prepare/update,
//      SourceRateHelper.factor, ConstantFissionYieldHelper.__init__,
//      FissionYieldCutoffHelper.__init__/weighted_yields,
//      AveragedFissionYieldHelper.weighted_yields)
//
// An OUTRAM PARK translation for research, education and V&V only.

//! **The arithmetic of OpenMC's depletion helpers** (gh:#1624): how the
//! reaction rates are normalised to a power or source rate, and how a
//! nuclide's fission yields are chosen or blended from its tabulated
//! incident energies.
//!
//! Upstream's helpers also create and unpack tallies through `openmc.lib`.
//! That half belongs to this crate's transport and is not here: every
//! function below takes the tallied numbers (fission rates, thermal/fast
//! fractions, average fission energies) as arguments.
//!
//! # Departures
//!
//! 1. **No MPI.** `EnergyNormalizationHelper.factor` reduces the energy over
//!    ranks and aborts the whole run on zero; here the caller passes the
//!    total and a zero is an error.
//! 2. **Yield sets are blended by product name.** Upstream multiplies and
//!    adds `FissionYield` objects, which share one product list per parent;
//!    a product present in only one set is treated as 0 in the other.
//!
//! # Verification status (2026-11-17)
//!
//! ~~Analytic tests only. Not yet compared against OpenMC by running it.~~
//! **CORRECTED 2026-11-17:** `source_rate_factor`, `energy_normalization_factor`
//! and `chain_fission_energy` are compared against OpenMC `e4febe9` run on the
//! same inputs: the rates they normalise are bit-identical to
//! `IndependentOperator`'s in all 16 runs of
//! `tests/independent_operator_vs_openmc_run.rs`. The yield helpers are
//! still checked analytically only.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

/// Upstream `openmc.data.JOULE_PER_EV` (CODATA 2018, exact).
pub const JOULE_PER_EV: f64 = 1.602_176_634e-19;

/// One parent's fission yields at its tabulated incident energies
/// (ascending, upstream `yield_energies`), each a `(product, yield)` list.
#[derive(Debug, Clone, PartialEq)]
pub struct YieldTable {
    /// Incident energies \[eV\], ascending.
    pub energies: Vec<f64>,
    /// Yields at each energy, parallel to `energies`.
    pub sets: Vec<Vec<(String, f64)>>,
}

/// `a * wa + b * wb`, by product name (departure 2), in `a`'s order with
/// `b`'s extra products after.
fn blend(a: &[(String, f64)], wa: f64, b: &[(String, f64)], wb: f64) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = a.iter().map(|(p, y)| (p.clone(), y * wa)).collect();
    for (p, y) in b {
        match out.iter_mut().find(|(q, _)| q == p) {
            Some((_, v)) => *v += y * wb,
            None => out.push((p.clone(), y * wb)),
        }
    }
    out
}

/// Upstream's `min(energies, key=lambda e: abs(e - target))`: the first of
/// the nearest.
fn nearest(energies: &[f64], target: f64) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, e) in energies.iter().enumerate() {
        if best.is_none_or(|b| (e - target).abs() < (energies[b] - target).abs()) {
            best = Some(i);
        }
    }
    best
}

/// Upstream `EnergyNormalizationHelper.factor`: source neutrons per second
/// for a power `power_w` when `energy_ev_per_source` eV are deposited per
/// source neutron.
///
/// # Errors
/// Zero energy (upstream: "No energy reported from OpenMC tallies").
pub fn energy_normalization_factor(power_w: f64, energy_ev_per_source: f64) -> Result<f64, String> {
    let energy = energy_ev_per_source * JOULE_PER_EV;
    if energy == 0.0 {
        return Err("No energy reported from the tallies. Do the nuclear data have heating?".into());
    }
    Ok(power_w / energy)
}

/// Upstream `SourceRateHelper.factor`: the source rate itself.
pub fn source_rate_factor(source_rate: f64) -> f64 {
    source_rate
}

/// Upstream `ChainFissionHelper.prepare`: each rated nuclide's fission Q
/// \[eV\] from the chain (its first `fission` reaction), 0 where none.
/// `chain` is `(name, [(reaction type, Q)])`; `rate_index` names the slots.
pub fn fission_q_vector(chain: &[(String, Vec<(String, f64)>)], rate_index: &[String]) -> Vec<f64> {
    let mut q = vec![0.0; rate_index.len()];
    for (name, reactions) in chain {
        if let Some(i) = rate_index.iter().position(|n| n == name) {
            if let Some((_, qv)) = reactions.iter().find(|(t, _)| t == "fission") {
                q[i] = *qv;
            }
        }
    }
    q
}

/// Upstream `ChainFissionHelper.update`: `energy += fission_rates · q`.
pub fn chain_fission_energy(energy: f64, fission_rates: &[f64], q: &[f64]) -> f64 {
    energy + fission_rates.iter().zip(q).map(|(r, qv)| r * qv).sum::<f64>()
}

/// Upstream `ConstantFissionYieldHelper.__init__` for one parent: the set at
/// exactly `energy` if tabulated, else at the nearest tabulated energy.
pub fn constant_yields(table: &YieldTable, energy: f64) -> Option<Vec<(String, f64)>> {
    if let Some(i) = table.energies.iter().position(|e| *e == energy) {
        return Some(table.sets[i].clone());
    }
    nearest(&table.energies, energy).map(|i| table.sets[i].clone())
}

/// What `FissionYieldCutoffHelper` does with one parent.
#[derive(Debug, Clone, PartialEq)]
pub enum CutoffYields {
    /// The cutoff lies outside the tabulated energies: one constant set
    /// (the lowest if `cutoff <= E_0`, the highest if `cutoff >= E_last`).
    Constant(Vec<(String, f64)>),
    /// Thermal and fast sets, blended by the tallied fission fractions.
    Split {
        /// Set used below the cutoff.
        thermal: Vec<(String, f64)>,
        /// Set used above it.
        fast: Vec<(String, f64)>,
    },
}

/// Upstream `FissionYieldCutoffHelper.__init__` for one parent: the sets
/// at `thermal_energy` and `fast_energy` if tabulated; otherwise the
/// nearest tabulated energy below the cutoff (thermal) and at or above it
/// (fast), after the constant cases.
///
/// # Errors
/// Upstream's checks: `0 <= thermal < cutoff < fast`, and a non-empty table.
pub fn cutoff_yields(table: &YieldTable, cutoff: f64, thermal_energy: f64, fast_energy: f64) -> Result<CutoffYields, String> {
    if !(thermal_energy >= 0.0 && cutoff > thermal_energy && fast_energy > cutoff) {
        return Err(format!("need 0 <= thermal_energy ({thermal_energy}) < cutoff ({cutoff}) < fast_energy ({fast_energy})"));
    }
    let e = &table.energies;
    if e.is_empty() {
        return Err("a parent with no yield energies".into());
    }
    let at = |x: f64| e.iter().position(|v| *v == x).map(|i| table.sets[i].clone());
    let (mut thermal, mut fast) = (at(thermal_energy), at(fast_energy));
    if thermal.is_none() || fast.is_none() {
        if cutoff <= e[0] {
            return Ok(CutoffYields::Constant(table.sets[0].clone()));
        }
        if cutoff >= e[e.len() - 1] {
            return Ok(CutoffYields::Constant(table.sets[e.len() - 1].clone()));
        }
        // bisect.bisect_left
        let ix = e.partition_point(|v| *v < cutoff);
        if thermal.is_none() {
            thermal = nearest(&e[..ix], thermal_energy).map(|i| table.sets[i].clone());
        }
        if fast.is_none() {
            fast = nearest(&e[ix..], fast_energy).map(|i| table.sets[ix + i].clone());
        }
    }
    match (thermal, fast) {
        (Some(thermal), Some(fast)) => Ok(CutoffYields::Split { thermal, fast }),
        _ => Err("no yield set either side of the cutoff".into()),
    }
}

/// Upstream `FissionYieldCutoffHelper.weighted_yields` for one parent:
/// `thermal · f_thermal + fast · f_fast`, the fractions from the tallies.
pub fn cutoff_weighted(y: &CutoffYields, thermal_fraction: f64, fast_fraction: f64) -> Vec<(String, f64)> {
    match y {
        CutoffYields::Constant(c) => c.clone(),
        CutoffYields::Split { thermal, fast } => blend(thermal, thermal_fraction, fast, fast_fraction),
    }
}

/// Upstream `AveragedFissionYieldHelper.weighted_yields` for one parent:
/// at the tallied average fission energy, the lowest or highest set outside
/// the table, else linear interpolation between the bracketing sets.
pub fn averaged_yields(table: &YieldTable, average_energy: f64) -> Option<Vec<(String, f64)>> {
    let e = &table.energies;
    if e.is_empty() {
        return None;
    }
    if average_energy <= e[0] {
        return Some(table.sets[0].clone());
    }
    if average_energy >= e[e.len() - 1] {
        return Some(table.sets[e.len() - 1].clone());
    }
    // Upstream's linear search: the first ix with e[ix+1] > avg.
    let mut ix = 0;
    for i in 0..e.len() - 1 {
        ix = i;
        if e[i + 1] > average_energy {
            break;
        }
    }
    let (lower, upper) = (e[ix], e[ix + 1]);
    let fast_frac = (average_energy - lower) / (upper - lower);
    Some(blend(&table.sets[ix], 1.0 - fast_frac, &table.sets[ix + 1], fast_frac))
}

#[cfg(test)]
mod tests;
