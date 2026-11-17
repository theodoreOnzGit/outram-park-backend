// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Spectrum-tracking depletion** (gh:#1630, phase 3 of #1623). New work,
//! not a port: OpenMC re-tallies every step and has no criterion.
//!
//! # The criterion
//!
//! A burnup matrix holds per-atom rates `r_i = σ̄_i φ`. Within a segment the
//! one-group cross sections `σ̄_i` are frozen; the composition `N` is
//! evolved exactly and `φ` is renormalised to power every step, so neither
//! of those is an error to watch. [`refresh_criterion`] therefore compares,
//! **at the same composition**, the rates the frozen segment would use with
//! the rates a fresh spectrum gives:
//!
//! `c = Σ_i Σ_ch N_i |r_new − r_frozen| / Σ_i Σ_ch N_i r_frozen`,
//!
//! over the fission and capture channels, each `φ` normalised to the same
//! power. Weighting by `N_i r_i` is weighting by each entry's share of the
//! total reaction rate, as #1630 asks: a large relative change in a nuclide
//! that does little moves `c` little. A change in `N` alone, with the
//! spectrum unchanged, gives `c = 0` exactly (pinned by a test).
//!
//! # The driver
//!
//! [`deplete_tracking`] is [`super::operator::deplete_coupled`]'s loop with
//! one change: the transport result at the start of each step is **adopted**
//! only when `c > tolerance`. The transport call count is the same; what the
//! criterion saves is the regeneration of what depends on the spectrum (the
//! one-group data here; chain branching ratios and yields when they are
//! wired, gh:#1628). Tolerance 0 reproduces `deplete_coupled` bit for bit;
//! an infinite tolerance freezes the beginning-of-life spectrum.
//!
//! # Verification status (2026-11-17)
//!
//! Tests below, on a synthetic spectrum callback: the two limits above, the
//! composition-only control, and convergence of the end-of-life inventory
//! as the tolerance tightens. **Not yet** on transport spectra, not with
//! against OpenMC. The removal pair ([`deplete_tracking_pair`]) is checked
//! for the sign of its effect on xenon sparging (tests).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::chain::DepletionChain;
use super::cram::cram16;
use super::external_rates::{deplete_with_external_rates, TransferRates};
use super::operator::{
    flux_for_power, k_inf, one_group_cross_sections, reaction_rates, BurnupResult, BurnupSettings, BurnupStep,
    OneGroupWeighting, OneGroupXs, DEFAULT_GROUP_SUB_POINTS,
};

const SECONDS_PER_DAY: f64 = 86_400.0;

/// The criterion's value and which nuclide moved it most.
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshCriterion {
    /// `c` above.
    pub value: f64,
    /// Nuclide with the largest `N_i |Δr_i|`, if any rate moved.
    pub dominant: Option<String>,
}

/// `c` at composition `densities`, between the frozen one-group data and a
/// fresh one, each with its own power-normalised flux.
pub fn refresh_criterion(
    names: &[&str],
    densities: &[f64],
    frozen: &[OneGroupXs],
    fresh: &[OneGroupXs],
    settings: &BurnupSettings,
) -> RefreshCriterion {
    let phi_old = flux_for_power(names, densities, frozen, settings);
    let phi_new = flux_for_power(names, densities, fresh, settings);
    let mut num = 0.0;
    let mut den = 0.0;
    let mut best: Option<(usize, f64)> = None;
    for i in 0..names.len() {
        let n = densities[i];
        let old = [frozen[i].fission * phi_old, frozen[i].gamma * phi_old];
        let new = [fresh[i].fission * phi_new, fresh[i].gamma * phi_new];
        let mut contrib = 0.0;
        for c in 0..2 {
            contrib += n * (new[c] - old[c]).abs();
            den += n * old[c];
        }
        num += contrib;
        if contrib > 0.0 && best.is_none_or(|(_, b)| contrib > b) {
            best = Some((i, contrib));
        }
    }
    let value = if den > 0.0 { num / den } else { 0.0 };
    RefreshCriterion { value, dominant: best.map(|(i, _)| names[i].to_owned()) }
}

/// One refresh (or a decision not to).
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshRecord {
    /// Step whose start it was evaluated at.
    pub step: usize,
    /// The criterion then.
    pub criterion: RefreshCriterion,
    /// Whether the fresh spectrum was adopted.
    pub refreshed: bool,
    /// `k_inf` at that point with the frozen data, recorded as a
    /// cross-check, never a trigger (#1630, verification 4).
    pub k_inf_frozen: f64,
}

/// A tracked run: the history plus every criterion evaluation.
#[derive(Debug, Clone)]
pub struct TrackingResult {
    /// Inventory history, as `deplete_coupled` records it.
    pub result: BurnupResult,
    /// One record per transport result after beginning of life.
    pub records: Vec<RefreshRecord>,
    /// Transport (spectrum) calls made.
    pub transport_calls: usize,
    /// `(step, chain)` for every chain used: the initial one at step 0,
    /// then each regenerated one at the step it took effect.
    pub chains: Vec<(usize, DepletionChain)>,
}

impl TrackingResult {
    /// Number of adopted refreshes.
    pub fn refreshes(&self) -> usize {
        self.records.iter().filter(|r| r.refreshed).count()
    }
}

/// [`super::operator::deplete_coupled`] with a refresh criterion: see the
/// module docs. `tolerance = 0.0` adopts every changed spectrum.
pub fn deplete_tracking(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    tolerance: f64,
    spectrum: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
) -> TrackingResult {
    run_tracking(chain, initial, settings, tolerance, None, spectrum, no_regeneration).expect("no transfer rates: nothing can fail")
}

/// The with-removal and without-removal histories from one starting state
/// (#1630's removal ablation). Each arm has its own spectrum callback and
/// its own refreshes, since removal changes the spectrum.
#[derive(Debug, Clone)]
pub struct RemovalPair {
    /// With `removal` applied every step.
    pub with_removal: TrackingResult,
    /// The same run without it.
    pub without_removal: TrackingResult,
}

/// Run both arms. `removal` is a one-material [`TransferRates`] whose
/// timestep `k` applies to burnup step `k + 1`, solved exactly by
/// [`deplete_with_external_rates`] with CRAM-16 (removal and any feed).
///
/// # Errors
/// From the transfer terms (e.g. a removal set on a material other than
/// the one `removal` names).
pub fn deplete_tracking_pair(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    tolerance: f64,
    removal: &TransferRates,
    spectrum_with: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
    spectrum_without: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
) -> Result<RemovalPair, String> {
    Ok(RemovalPair {
        with_removal: run_tracking(chain, initial, settings, tolerance, Some(removal), spectrum_with, no_regeneration)?,
        without_removal: run_tracking(chain, initial, settings, tolerance, None, spectrum_without, no_regeneration)?,
    })
}

fn no_regeneration(_: &[f64], _: &[f64]) -> Result<Option<DepletionChain>, String> {
    Ok(None)
}

/// [`deplete_tracking`] that also **regenerates the chain** at every
/// adopted refresh (#1630: "start a new segment with a regenerated chain").
/// `regenerate(group_edges, group_flux)` returns the chain for the new
/// spectrum (for example branching ratios from
/// `njoy_outram_park_fork::isomeric_branching::apply_to_chain` and
/// `DepletionChain::from_chain_xml`), or `None` to keep the current one.
/// The chain at beginning of life is `chain`; a regenerated chain must list
/// the same nuclides in the same order, since the inventory vector is
/// carried across. [`TrackingResult::chains`] records every chain used.
///
/// # Errors
/// A regenerated chain with a different nuclide list, or `regenerate`'s own.
pub fn deplete_tracking_regenerating(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    tolerance: f64,
    spectrum: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
    regenerate: impl FnMut(&[f64], &[f64]) -> Result<Option<DepletionChain>, String>,
) -> Result<TrackingResult, String> {
    run_tracking(chain, initial, settings, tolerance, None, spectrum, regenerate)
}

fn run_tracking(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    tolerance: f64,
    transfer: Option<&TransferRates>,
    mut spectrum: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
    mut regenerate: impl FnMut(&[f64], &[f64]) -> Result<Option<DepletionChain>, String>,
) -> Result<TrackingResult, String> {
    let base = chain;
    let mut current = base.clone();
    let mut chains: Vec<(usize, DepletionChain)> = vec![(0, base.clone())];
    let chain = &mut current;
    let names_owned: Vec<String> = chain.nuclide_names().iter().map(|n| (*n).to_owned()).collect();
    let names: Vec<&str> = names_owned.iter().map(String::as_str).collect();
    let mut densities = vec![0.0_f64; names.len()];
    for (name, dens) in initial {
        if let Some(idx) = chain.index_of(name) {
            densities[idx] = *dens;
        }
    }
    let inventory = |d: &[f64]| -> Vec<(String, f64)> { names.iter().zip(d).map(|(n, x)| (n.to_string(), *x)).collect() };
    let record = |step: usize, time_days: f64, flux: f64, d: &[f64], xs: &[OneGroupXs]| BurnupStep {
        step,
        time_days,
        flux,
        k_inf: k_inf(d, xs),
        densities: names.iter().zip(d).map(|(n, x)| (n.to_string(), *x)).collect(),
    };
    let with_spectrum = |e_grid_ev: Vec<f64>, phi: Vec<f64>| {
        let mut s = settings.clone();
        s.weighting = OneGroupWeighting::TabulatedFlux { e_grid_ev, phi, sub_points: DEFAULT_GROUP_SUB_POINTS };
        s
    };

    let mut transport_calls = 0usize;
    let mut xs = one_group_cross_sections(chain, settings);
    transport_calls += 1;
    if let Some((e, phi)) = spectrum(&inventory(&densities)) {
        xs = one_group_cross_sections(chain, &with_spectrum(e, phi));
    }
    let mut steps = Vec::with_capacity(settings.n_steps + 1);
    steps.push(record(0, 0.0, 0.0, &densities, &xs));
    let mut records = Vec::new();

    let dt_seconds = settings.step_days * SECONDS_PER_DAY;
    for step in 1..=settings.n_steps {
        if step > 1 {
            transport_calls += 1;
            if let Some((e, phi)) = spectrum(&inventory(&densities)) {
                let fresh = one_group_cross_sections(chain, &with_spectrum(e.clone(), phi.clone()));
                let criterion = refresh_criterion(&names, &densities, &xs, &fresh, settings);
                let refreshed = criterion.value > tolerance;
                records.push(RefreshRecord { step, criterion, refreshed, k_inf_frozen: k_inf(&densities, &xs) });
                if refreshed {
                    if let Some(new_chain) = regenerate(&e, &phi)? {
                        if new_chain.nuclide_names() != names {
                            return Err(format!("step {step}: the regenerated chain lists different nuclides"));
                        }
                        *chain = new_chain;
                        chains.push((step, chain.clone()));
                        xs = one_group_cross_sections(chain, &with_spectrum(e, phi));
                    } else {
                        xs = fresh;
                    }
                }
            }
        }
        let flux = flux_for_power(&names, &densities, &xs, settings);
        let rates = reaction_rates(&names, flux, &xs);
        let matrix = chain.build_matrix(&rates);
        densities = match transfer {
            None => cram16(&matrix, &densities, dt_seconds),
            Some(tr) => {
                deplete_with_external_rates(cram16, &names_owned, vec![matrix], vec![densities], dt_seconds, step - 1, Some(tr), None)?
                    .pop()
                    .ok_or("no material returned")?
            }
        };
        for d in &mut densities {
            if *d < 0.0 {
                *d = 0.0;
            }
        }
        steps.push(record(step, step as f64 * settings.step_days, flux, &densities, &xs));
    }
    Ok(TrackingResult { result: BurnupResult { steps }, records, transport_calls, chains })
}

#[cfg(test)]
mod tests;
