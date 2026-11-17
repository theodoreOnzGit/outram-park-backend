// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **The simple depletion API** (gh:#1629, phase 2 of #1623): a transported
//! spectrum, a chain built from the evaluations, and the compositions after
//! a number of one-group steps on that spectrum.
//!
//! [`SpectrumDepletion::run`] does, in order, each step reusing a phase-1
//! piece:
//!
//! 1. **Chain.** The caller's chain (normally
//!    `njoy_outram_park_fork::hdf5::depletion_chain_from_endf::from_endf`,
//!    gh:#1621) is **reduced** to what the starting nuclides reach
//!    (`depletion_chain_ops::reduce`). Each fissionable nuclide's yields
//!    are chosen as OpenMC's `ConstantFissionYieldHelper` does
//!    ([`super::helpers::constant_yields`], gh:#1624): the set at the
//!    requested energy, else the nearest tabulated one.
//! 2. **Spectrum.** One k-eigenvalue transport run tallies the multigroup
//!    flux in the depletable material
//!    ([`super::microxs_transport::get_microxs_and_flux`], gh:#1625).
//! 2b. **Branching (default, gh:#1628).** For each evaluated parent with
//!    MF=8/9/10 data on (n,γ), (n,2n), (n,p) or (n,a), the ratios to each
//!    product state are collapsed on the tallied flux and written into the
//!    chain through `set_branch_ratios`. A reaction without such data keeps
//!    the chain's ratio and is recorded as [`BranchingOutcome::NoData`];
//!    [`SpectrumDepletion::with_preset_branch_ratios`] is the ablation.
//! 3. **One-group data.** For each chain nuclide with an evaluation given,
//!    RECONR reconstructs it (0 K) and its (n,γ), fission and (n,2n) are
//!    collapsed on the tallied flux ([`super::microxs::nuclide_collapse_rate`]).
//!    A nuclide with no evaluation has no reactions and is **named** in
//!    [`DepletionOutcome::no_data`].
//! 4. **Depletion.** At each step the flux magnitude is set to the power
//!    with the chain's own fission Q values ([`super::helpers::fission_q_vector`]),
//!    the matrix is built, and CRAM-16 advances it. Optional transfer rates
//!    (gh:#1622) move nuclides into out-of-flux holding materials (decay
//!    only), solved exactly as one block system. The spectrum is **held
//!    fixed**: that is what this level is. The tracking driver (gh:#1630)
//!    decides when to refresh it.
//!
//! # Not yet
//!
//! - Input is a CSG geometry with its materials and nuclides, not the
//!   beginner `model::Model`.
//! - RECONR at 0 K: no Doppler broadening of the collapsed data.
//! - The integrator is predictor (CRAM-16 per step); PETIR's others are not
//!   wired here.
//! - Not compared with OpenMC running (gh:#1626).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::hdf5::depletion_chain_ops::reduce;
use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;

use super::chain::DepletionChain;
use super::cram::cram16;
use super::external_rates::{deplete_with_external_rates, TransferRates};
use super::helpers::{constant_yields, fission_q_vector, YieldTable};
use super::microxs::{nuclide_collapse_rate, NuclideReaction, ReactionTable};
use super::microxs_transport::get_microxs_and_flux;
use super::operator::OneGroupXs;
use super::{MicroRate, ReactionRates};
use crate::prelude::{Geometry, KeffSettings, Material, Nuclide, SourceBox};

const BARN_CM2: f64 = 1.0e-24;
const EV_TO_J: f64 = super::helpers::JOULE_PER_EV;

/// A study: what to deplete, from what, for how long.
#[derive(Debug, Clone)]
pub struct SpectrumDepletion {
    depletable: usize,
    starting: Vec<(String, f64)>,
    chain_xml: DepletionChainXml,
    reduce_level: Option<usize>,
    reduce_from: Option<Vec<String>>,
    yield_energy_ev: f64,
    evaluations: Vec<(String, Tape)>,
    group_edges: Vec<f64>,
    power_w: f64,
    volume_cm3: f64,
    steps_s: Vec<f64>,
    transfer: Option<TransferRates>,
    preset_branch_ratios: bool,
}

/// One reaction's branching outcome (gh:#1628).
#[derive(Debug, Clone, PartialEq)]
pub struct BranchingRecord {
    /// Parent nuclide.
    pub parent: String,
    /// Reaction, e.g. `"(n,gamma)"`.
    pub reaction: String,
    /// What was done.
    pub outcome: BranchingOutcome,
}

/// What was done with one reaction's branching.
#[derive(Debug, Clone, PartialEq)]
pub enum BranchingOutcome {
    /// Ratios collapsed on the spectrum and written to the chain.
    Applied {
        /// `(product, ratio)` written.
        set: Vec<(String, f64)>,
        /// States collapsed but not in the chain.
        not_in_chain: Vec<(String, f64)>,
    },
    /// The evaluation has no MF=8/9/10 for this reaction: the chain's
    /// ratio stands, and this record says so.
    NoData,
    /// Data exists but could not be applied (reason); the chain's ratio
    /// stands.
    Refused(String),
}

/// What a run returns. Nothing hidden: `chain` is the exact chain the
/// matrices were built from.
#[derive(Debug, Clone)]
pub struct DepletionOutcome {
    /// Group bounds \[eV\].
    pub group_edges: Vec<f64>,
    /// Tallied group flux in the depletable material (per source particle).
    pub group_flux: Vec<f64>,
    /// The reduced chain, as XML records (export with `export_to_xml`).
    pub chain_xml: DepletionChainXml,
    /// The same chain as the solver uses it.
    pub chain: DepletionChain,
    /// One-group cross sections per chain nuclide, chain order.
    pub one_group: Vec<(String, OneGroupXs)>,
    /// Chain nuclides with no evaluation given (no reactions).
    pub no_data: Vec<String>,
    /// Starting nuclides the reduced chain does not hold.
    pub dropped: Vec<String>,
    /// What happened to each evaluated reaction's branching (gh:#1628);
    /// empty under [`SpectrumDepletion::with_preset_branch_ratios`].
    pub branching: Vec<BranchingRecord>,
    /// `(time [s], flux [n/cm²/s] over the step just taken, densities per
    /// material in chain order)`; the first entry is the start.
    pub history: Vec<(f64, f64, Vec<Vec<f64>>)>,
}

impl DepletionOutcome {
    /// Density \[atoms/(b·cm)\] of `nuclide` in material `m` at history
    /// entry `k`.
    pub fn density(&self, k: usize, m: usize, nuclide: &str) -> f64 {
        self.chain.index_of(nuclide).map_or(0.0, |i| self.history[k].2[m][i])
    }
}

impl SpectrumDepletion {
    /// A study of material `depletable` (index into the transport
    /// materials) starting from `starting` (`(nuclide, atoms/(b·cm))`) on
    /// `chain_xml`. Defaults: reduce with no level limit, yields at
    /// 0.0253 eV, 1 W in 1 cm³, one 1-day step, 40 log groups from 1e-5 eV
    /// to 20 MeV, no transfer.
    pub fn new(depletable: usize, starting: Vec<(String, f64)>, chain_xml: DepletionChainXml) -> Self {
        let n = 40usize;
        let (lo, hi) = (petir::mathf::RealMath::r_ln(1.0e-5_f64), petir::mathf::RealMath::r_ln(2.0e7_f64));
        let group_edges = (0..=n).map(|i| petir::mathf::RealMath::r_exp(lo + (hi - lo) * i as f64 / n as f64)).collect();
        Self {
            depletable,
            starting,
            chain_xml,
            reduce_level: None,
            reduce_from: None,
            yield_energy_ev: 0.0253,
            evaluations: Vec::new(),
            group_edges,
            power_w: 1.0,
            volume_cm3: 1.0,
            steps_s: vec![86_400.0],
            transfer: None,
            preset_branch_ratios: false,
        }
    }

    /// The nuclides the chain is reduced from (default: the starting ones).
    /// A starting nuclide the reduction does not reach is dropped and
    /// named in [`DepletionOutcome::dropped`].
    pub fn with_reduce_from(mut self, from: &[&str]) -> Self {
        self.reduce_from = Some(from.iter().map(|s| (*s).to_owned()).collect());
        self
    }

    /// The reduction depth (`None`: everything reachable).
    pub fn with_reduce_level(mut self, level: Option<usize>) -> Self {
        self.reduce_level = level;
        self
    }

    /// The incident energy whose fission yields are used (nearest set).
    pub fn with_yield_energy(mut self, e_ev: f64) -> Self {
        self.yield_energy_ev = e_ev;
        self
    }

    /// The evaluation (an incident-neutron ENDF tape) for `nuclide`.
    pub fn with_evaluation(mut self, nuclide: &str, tape: Tape) -> Self {
        self.evaluations.push((nuclide.to_owned(), tape));
        self
    }

    /// The tally's group bounds \[eV\], ascending.
    pub fn with_group_edges(mut self, edges: Vec<f64>) -> Self {
        self.group_edges = edges;
        self
    }

    /// Fission power \[W\] in `volume_cm3` of the depletable material.
    pub fn with_power(mut self, power_w: f64, volume_cm3: f64) -> Self {
        self.power_w = power_w;
        self.volume_cm3 = volume_cm3;
        self
    }

    /// Step lengths \[s\].
    pub fn with_steps(mut self, steps_s: Vec<f64>) -> Self {
        self.steps_s = steps_s;
        self
    }

    /// Transfer rates (gh:#1622). Its first burnable material is the
    /// irradiated one; any others are out-of-flux holding materials.
    pub fn with_transfer_rates(mut self, rates: TransferRates) -> Self {
        self.transfer = Some(rates);
        self
    }

    /// **Ablation:** keep the chain's own branching ratios instead of
    /// deriving them from the spectrum (gh:#1628).
    pub fn with_preset_branch_ratios(mut self) -> Self {
        self.preset_branch_ratios = true;
        self
    }

    /// Run the study (module docs, steps 1-4).
    ///
    /// # Errors
    /// A reduction, chain-import, transport or transfer error.
    pub fn run(
        &self,
        geom: &Geometry,
        materials: &[Material],
        nuclides: &[Nuclide],
        source: SourceBox,
        keff: &KeffSettings,
    ) -> Result<DepletionOutcome, String> {
        // 1. Chain: reduce, then fix one yield set per parent.
        let start: Vec<&str> = match &self.reduce_from {
            Some(v) => v.iter().map(String::as_str).collect(),
            None => self.starting.iter().map(|(n, _)| n.as_str()).collect(),
        };
        let mut xml = reduce(&self.chain_xml, &start, self.reduce_level).map_err(|e| e.to_string())?;
        for n in &mut xml.nuclides {
            if n.fission_yields.is_empty() {
                continue;
            }
            let table = YieldTable {
                energies: n.fission_yields.iter().map(|y| y.energy_ev).collect(),
                sets: n.fission_yields.iter().map(|y| y.yields.clone()).collect(),
            };
            let chosen = constant_yields(&table, self.yield_energy_ev).unwrap_or_default();
            n.fission_yields = vec![njoy_outram_park_fork::hdf5::depletion_chain_xml::FissionYieldSet { energy_ev: self.yield_energy_ev, yields: chosen }];
            n.fission_yield_parent = None;
        }
        // 2. Spectrum: one transport run.
        let out = get_microxs_and_flux(geom, materials, nuclides, source, keff, &[self.depletable], &[], &[], &self.group_edges, |_, _| None)?;
        let group_flux = out[0].group_flux.clone();
        let total: f64 = group_flux.iter().sum();
        if !(total > 0.0) {
            return Err("no flux tallied in the depletable material".into());
        }
        let norm: Vec<f64> = group_flux.iter().map(|f| f / total).collect();

        // Reconstruct each evaluated chain nuclide once (RECONR, 0 K).
        let mut recs: Vec<(String, njoy_outram_park_fork::reconr::ReconrResult)> = Vec::new();
        for n in &xml.nuclides {
            if let Some((_, tape)) = self.evaluations.iter().find(|(m, _)| *m == n.name) {
                let rec = njoy_outram_park_fork::process::reconstruct(tape, &njoy_outram_park_fork::process::CrossSectionSettings::default())
                    .map_err(|e| format!("{}: {e}", n.name))?;
                recs.push((n.name.clone(), rec.reconr));
            }
        }
        let pairs_of = |name: &str, mt: i32| -> Option<Vec<(f64, f64)>> {
            let m = njoy_outram_park_fork::endf::mt::MtReaction::from_any(mt);
            recs.iter().find(|(n, _)| n == name)?.1.sections.iter().find(|s| s.mt == m).map(|s| s.pairs.clone())
        };

        // Branching ratios from the spectrum (gh:#1628), the default.
        let mut branching = Vec::new();
        if !self.preset_branch_ratios {
            for (name, _) in &recs {
                let Some((_, tape)) = self.evaluations.iter().find(|(m, _)| m == name) else { continue };
                let mat = tape.materials().first().copied().unwrap_or(0);
                for (kind, mt) in [("(n,gamma)", 102), ("(n,2n)", 16), ("(n,p)", 103), ("(n,a)", 107)] {
                    let has = xml.get(name).is_some_and(|n| n.reactions.iter().any(|r| r.kind == kind));
                    if !has {
                        continue;
                    }
                    let (states, _) = njoy_outram_park_fork::isomeric_branching::production_states(tape, mat, mt).map_err(|e| e.to_string())?;
                    let rec = |outcome: BranchingOutcome| BranchingRecord { parent: name.clone(), reaction: kind.to_owned(), outcome };
                    if states.is_empty() {
                        branching.push(rec(BranchingOutcome::NoData));
                        continue;
                    }
                    let Some(sigma) = pairs_of(name, mt) else {
                        branching.push(rec(BranchingOutcome::Refused(format!("no MF=3/MT={mt} after RECONR"))));
                        continue;
                    };
                    match njoy_outram_park_fork::isomeric_branching::apply_to_chain(&mut xml, name, kind, &states, &sigma, &self.group_edges, &group_flux) {
                        Ok(a) => branching.push(rec(BranchingOutcome::Applied { set: a.set, not_in_chain: a.not_in_chain })),
                        Err(e) => branching.push(rec(BranchingOutcome::Refused(e.to_string()))),
                    }
                }
            }
        }

        let chain = DepletionChain::from_chain_xml_with_branches(&xml, self.yield_energy_ev).map_err(|e| format!("{e:?}"))?;
        let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();

        // 3. One-group data from each evaluation.
        let mut one_group = Vec::with_capacity(names.len());
        let mut n2n = vec![0.0; names.len()];
        let mut no_data = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if !recs.iter().any(|(n, _)| n == name) {
                no_data.push(name.clone());
                one_group.push((name.clone(), OneGroupXs::default()));
                continue;
            }
            let collapse = |mt: i32| -> f64 {
                pairs_of(name, mt).map_or(0.0, |p| {
                    nuclide_collapse_rate(&NuclideReaction { low: ReactionTable::from_pointwise(&p), high: None }, &self.group_edges, &norm)
                })
            };
            let gamma = collapse(102);
            let fission = collapse(18);
            n2n[i] = collapse(16);
            one_group.push((name.clone(), OneGroupXs { fission, gamma, nu_fission: 0.0, absorption: gamma + fission }));
        }

        // 4. Deplete on the fixed spectrum.
        let chain_records: Vec<(String, Vec<(String, f64)>)> =
            xml.nuclides.iter().map(|n| (n.name.clone(), n.reactions.iter().map(|r| (r.kind.clone(), r.q_ev)).collect())).collect();
        let q = fission_q_vector(&chain_records, &names);
        let n_mats = self.transfer.as_ref().map_or(1, |t| t.rates().burnable_mats().len());
        let mut dens = vec![vec![0.0; names.len()]; n_mats];
        let mut dropped = Vec::new();
        for (nuc, d) in &self.starting {
            match chain.index_of(nuc) {
                Some(i) => dens[0][i] = *d,
                None => dropped.push(nuc.clone()),
            }
        }
        let mut history = vec![(0.0, 0.0, dens.clone())];
        let mut t = 0.0;
        for (k, dt) in self.steps_s.iter().enumerate() {
            let fission_power_per_flux: f64 =
                (0..names.len()).map(|i| dens[0][i] * one_group[i].1.fission * q[i] * EV_TO_J).sum::<f64>() * self.volume_cm3;
            let flux = if fission_power_per_flux > 0.0 { self.power_w / fission_power_per_flux } else { 0.0 };
            let rates_at = |phi: f64| {
                let mut r = ReactionRates { flux: phi, ..ReactionRates::zero() };
                for (i, n) in name_refs.iter().enumerate() {
                    let x = &one_group[i].1;
                    r.set(n, MicroRate { gamma: phi * x.gamma * BARN_CM2, fission: phi * x.fission * BARN_CM2, n2n: phi * n2n[i] * BARN_CM2 });
                }
                r
            };
            let irradiated = chain.build_matrix(&rates_at(flux));
            dens = match &self.transfer {
                None => vec![cram16(&irradiated, &dens[0], *dt)],
                Some(tr) => {
                    let decay_only = chain.build_matrix(&rates_at(0.0));
                    let mut ms = vec![irradiated];
                    ms.extend((1..n_mats).map(|_| decay_only.clone()));
                    deplete_with_external_rates(cram16, &names, ms, dens.clone(), *dt, k, Some(tr), None)?
                }
            };
            for m in &mut dens {
                for d in m.iter_mut() {
                    if *d < 0.0 {
                        *d = 0.0;
                    }
                }
            }
            t += dt;
            history.push((t, flux, dens.clone()));
        }
        Ok(DepletionOutcome { group_edges: self.group_edges.clone(), group_flux, chain_xml: xml, chain, one_group, no_data, dropped, branching, history })
    }
}

#[cfg(test)]
mod tests;
