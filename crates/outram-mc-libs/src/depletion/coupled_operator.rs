// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream files: openmc/deplete/coupled_operator.py (CoupledOperator.__call__,
//     reaction_rate_mode = "flux"), openmc/deplete/openmc_operator.py
//     (_calculate_reaction_rates), openmc/deplete/helpers.py
//     (FluxCollapseHelper.get_material_rates, ChainFissionHelper,
//     SourceRateHelper)
//
// An OUTRAM PARK translation for research, education and V&V only.

//! **OpenMC's `CoupledOperator` in flux-collapse mode, on this crate's
//! transport** (gh:#1626 arm 2).
//!
//! Each evaluation writes the depletable materials' atoms into the transport
//! materials, runs one k-eigenvalue calculation with a `Material × Energy`
//! flux tally (upstream `FluxCollapseHelper.generate_tallies`), and for every
//! chain nuclide with data collapses each reaction on that group flux
//! (`Nuclide.collapse_rate`, [`super::microxs::nuclide_collapse_rate`]).
//! The rates are then normalised as `_calculate_reaction_rates` does:
//! divided by `1e24 V`, times the helper's factor (`fission-q`: power over
//! the chain-Q fission energy; `source-rate`: the source rate).
//!
//! It implements PETIR's [`TransportOperator`], so every integrator drives
//! it, as upstream's integrators drive `CoupledOperator`.
//!
//! # Departures
//!
//! 1. **Flux mode only.** Upstream's default `reaction_rate_mode="direct"`
//!    tallies each reaction per nuclide; this collapses the group flux, which
//!    is upstream's `"flux"` mode. Run OpenMC in `"flux"` mode with the same
//!    group bounds to compare like with like.
//! 2. **Reactions:** `(n,gamma)`, `fission` and `(n,2n)` (MT 102, 18, 16),
//!    the three channels [`MicroRate`] carries. A chain reaction outside
//!    these is not tallied.
//! 3. **Seeds:** each transport call uses `settings.seed + call index`, so
//!    successive evaluations are independent draws. Upstream reuses its
//!    settings seed; that changes no expectation.
//! 4. **Nuclides without data** keep their chain atoms and get no rates, as
//!    upstream; the caller says which transport nuclide carries each chain
//!    nuclide.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use alloc::sync::Arc;

use super::chain::DepletionChain;
use super::helpers::{chain_fission_energy, energy_normalization_factor, source_rate_factor};
use super::integrators::{AverageRates, Keff, OperatorResult, TransportOperator};
use super::matrix::DepletionMatrix;
use super::microxs::{nuclide_collapse_rate, NuclideReaction};
use super::{MicroRate, ReactionRates};
use crate::prelude::{FilterKind, Geometry, KeffSettings, Material, NuclideComponent, Nuclide, ScoreType, SourceBox, Tally, TallyBin};
use crate::tally::filter::{EnergyFilter, MaterialFilter};

/// Upstream's normalisation helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoupledNormalization {
    /// `ChainFissionHelper`: the source rate is a power \[W\], fission
    /// energy from the chain's fission Q.
    FissionQ,
    /// `SourceRateHelper`: the source rate is neutrons per second.
    SourceRate,
}

/// One chain nuclide's data as the operator needs it.
#[derive(Debug, Clone, Default)]
pub struct ChainNuclideData {
    /// Index of the transport nuclide carrying it, or `None` (no data).
    pub transport: Option<usize>,
    /// `(n,gamma)` (MT 102), fission (MT 18), `(n,2n)` (MT 16) tables, each
    /// `None` where the nuclide has no such reaction (rate 0, as upstream).
    pub gamma: Option<NuclideReaction>,
    /// Fission (MT 18).
    pub fission: Option<NuclideReaction>,
    /// `(n,2n)` (MT 16).
    pub n2n: Option<NuclideReaction>,
    /// Chain fission Q \[eV\] (`ChainFissionHelper.prepare`), 0 if none.
    pub fission_q: f64,
}

/// The rates of every depletable material, in depletable order.
#[derive(Debug, Clone, Default)]
pub struct MaterialRates(pub Vec<ReactionRates>);

impl AverageRates for MaterialRates {
    fn weighted_sum(a: f64, x: &Self, b: f64, y: &Self) -> Self {
        MaterialRates(x.0.iter().zip(&y.0).map(|(p, q)| <ReactionRates as AverageRates>::weighted_sum(a, p, b, q)).collect())
    }
}

/// OpenMC's `CoupledOperator` (flux mode) on a CSG model.
#[derive(Debug, Clone)]
pub struct CoupledOperator {
    geom: Arc<Geometry>,
    nuclides: Arc<Vec<Nuclide>>,
    /// The transport materials; the depletable ones are rewritten each call.
    pub materials: Vec<Material>,
    source: SourceBox,
    settings: KeffSettings,
    /// Indices of the depletable materials in [`Self::materials`].
    depletable: Vec<usize>,
    /// Their volumes \[cm^3\] (an area for a 2-D model, as upstream).
    volumes: Vec<f64>,
    chain: DepletionChain,
    names: Vec<String>,
    data: Vec<ChainNuclideData>,
    energies: Vec<f64>,
    normalization: CoupledNormalization,
    /// Transport calls made so far.
    pub calls: u64,
    /// Group flux of each depletable material from the last call.
    pub last_group_flux: Vec<Vec<f64>>,
}

impl CoupledOperator {
    /// An operator over `chain` (whose nuclides `data` describes, in chain
    /// order) on the transport model, with flux-collapse group bounds
    /// `energies` \[eV\].
    ///
    /// # Errors
    /// Mismatched lengths, fewer than two group bounds, or a depletable index
    /// out of range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        geom: Arc<Geometry>,
        nuclides: Arc<Vec<Nuclide>>,
        materials: Vec<Material>,
        source: SourceBox,
        settings: KeffSettings,
        depletable: Vec<usize>,
        volumes: Vec<f64>,
        chain: DepletionChain,
        data: Vec<ChainNuclideData>,
        energies: Vec<f64>,
        normalization: CoupledNormalization,
    ) -> Result<Self, String> {
        let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();
        if data.len() != names.len() {
            return Err(format!("{} chain nuclides but {} data entries", names.len(), data.len()));
        }
        if depletable.len() != volumes.len() {
            return Err(format!("{} depletable materials but {} volumes", depletable.len(), volumes.len()));
        }
        if energies.len() < 2 {
            return Err("need at least one energy group".into());
        }
        if let Some(&m) = depletable.iter().find(|&&m| m >= materials.len()) {
            return Err(format!("depletable material {m} out of range"));
        }
        let groups = energies.len() - 1;
        Ok(Self {
            last_group_flux: vec![vec![0.0; groups]; depletable.len()],
            geom,
            nuclides,
            materials,
            source,
            settings,
            depletable,
            volumes,
            chain,
            names,
            data,
            energies,
            normalization,
            calls: 0,
        })
    }

    /// Chain nuclide names, the order of every atom vector.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Write `atoms` of depletable material `d` into its transport material:
    /// each chain nuclide with data at `atoms / (1e24 V)` atoms/(b cm)
    /// (upstream `_update_materials`), other components unchanged.
    fn write_atoms(&mut self, d: usize, atoms: &[f64]) {
        let m = self.depletable[d];
        let v = self.volumes[d];
        for (j, nd) in self.data.iter().enumerate() {
            let Some(t) = nd.transport else { continue };
            let density = atoms[j] / v * 1.0e-24;
            let comps = &mut self.materials[m].components;
            match comps.iter_mut().find(|c| c.nuclide_idx == t) {
                Some(c) => c.atom_density = density,
                None => comps.push(NuclideComponent { nuclide_idx: t, atom_density: density }),
            }
        }
    }
}

impl TransportOperator for CoupledOperator {
    type Rates = MaterialRates;
    type Error = String;

    fn evaluate(&mut self, n: &[Vec<f64>], source_rate: f64) -> Result<OperatorResult<MaterialRates>, String> {
        if n.len() != self.depletable.len() {
            return Err(format!("{} atom vectors for {} depletable materials", n.len(), self.depletable.len()));
        }
        // CoupledOperator.__call__: zero source, zero rates, no transport.
        if source_rate == 0.0 {
            return Ok(OperatorResult {
                k: Keff { value: 0.0, std_dev: 0.0 },
                rates: MaterialRates(vec![ReactionRates::zero(); n.len()]),
            });
        }
        for (d, atoms) in n.iter().enumerate() {
            self.write_atoms(d, atoms);
        }
        let groups = self.energies.len() - 1;
        let mut tally = Tally {
            id: 1,
            name: "depletion flux".into(),
            filters: vec![
                FilterKind::Material(MaterialFilter { material_indices: self.depletable.clone() }),
                FilterKind::Energy(EnergyFilter { bins: self.energies.clone() }),
            ],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); self.depletable.len() * groups],
        };
        let mut settings = self.settings.clone();
        settings.seed = self.settings.seed.wrapping_add(self.calls);
        self.calls += 1;
        let res = crate::physics::transport_csg::try_run_keff_csg(&self.geom, &self.materials, &self.nuclides, self.source, &settings, Some(&mut tally))
            .map_err(|e| e.to_string())?;
        let n_active = settings.n_active as u64;

        // Per material: tally rates [reactions/src * b-cm/atom], fission
        // energy, then rates / (1e24 V) and the factor.
        let mut energy = 0.0;
        let mut tallied: Vec<Vec<(f64, f64, f64)>> = Vec::with_capacity(n.len());
        for (d, atoms) in n.iter().enumerate() {
            let flux: Vec<f64> = (0..groups).map(|g| tally.bins[d * groups + g].mean(n_active)).collect();
            let collapse = |rx: &Option<NuclideReaction>| rx.as_ref().map_or(0.0, |r| nuclide_collapse_rate(r, &self.energies, &flux));
            let rates: Vec<(f64, f64, f64)> = self
                .data
                .iter()
                .map(|nd| if nd.transport.is_some() { (collapse(&nd.gamma), collapse(&nd.fission), collapse(&nd.n2n)) } else { (0.0, 0.0, 0.0) })
                .collect();
            let volume_b_cm = 1e24 * self.volumes[d];
            let fission_rates: Vec<f64> = rates.iter().zip(atoms).map(|(r, a)| r.1 * (a / volume_b_cm)).collect();
            let q: Vec<f64> = self.data.iter().map(|nd| nd.fission_q).collect();
            energy = chain_fission_energy(energy, &fission_rates, &q);
            self.last_group_flux[d] = flux;
            tallied.push(rates);
        }
        let factor = match self.normalization {
            CoupledNormalization::SourceRate => source_rate_factor(source_rate),
            CoupledNormalization::FissionQ => energy_normalization_factor(source_rate, energy)?,
        };
        let mut out = Vec::with_capacity(n.len());
        for (d, rates) in tallied.iter().enumerate() {
            let volume_b_cm = 1e24 * self.volumes[d];
            let mut rr = ReactionRates::zero();
            for (name, r) in self.names.iter().zip(rates) {
                rr.set(name, MicroRate { gamma: r.0 / volume_b_cm * factor, fission: r.1 / volume_b_cm * factor, n2n: r.2 / volume_b_cm * factor });
            }
            out.push(rr);
        }
        Ok(OperatorResult { k: Keff { value: res.k_mean, std_dev: res.k_std }, rates: MaterialRates(out) })
    }

    fn form_matrix(&self, rates: &MaterialRates, material: usize) -> Result<DepletionMatrix, String> {
        rates.0.get(material).map(|r| self.chain.build_matrix(r)).ok_or_else(|| format!("no rates for depletable material {material}"))
    }
}

/// One reaction of a continuous-energy ACE table as upstream holds it in
/// memory after `IncidentNeutron.from_ace`: the nuclide's union grid, the
/// SIG block from its threshold index on (`Reaction.xs`, `threshold_idx`).
/// `None` when the table has no such MT: upstream's `collapse_rate` then
/// returns 0 (`reaction_index_[MT] < 0`, `nuclide.cpp:1052-1054`).
pub fn reaction_from_ace(ace: &njoy_outram_park_fork::acer::ce_decode::CeNeutronAce, mt: i32) -> Option<NuclideReaction> {
    let rx = ace.reactions.iter().find(|r| r.mt == mt)?;
    Some(NuclideReaction {
        low: super::microxs::ReactionTable { grid: ace.energy.clone(), xs: rx.xs.clone(), threshold: rx.threshold_index },
        high: None,
    })
}
