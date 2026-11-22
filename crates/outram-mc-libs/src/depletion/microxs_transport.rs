// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream file: openmc/deplete/microxs.py (get_microxs_and_flux,
//   reaction_rate_mode = "flux", material domains)

//! **`get_microxs_and_flux` on this crate's transport** (gh:#1625).
//!
//! As upstream in `reaction_rate_mode = "flux"` with material domains: one
//! k-eigenvalue run with a `MaterialFilter × EnergyFilter` flux tally, then
//! [`MultigroupMicroXs::from_multigroup_flux`] on each material's group
//! flux, and the flux returned **summed over groups** (upstream:
//! `fluxes = [flux.sum(keepdims=True) ...]`). The group fluxes are returned
//! too, which upstream drops.
//!
//! Not here: `reaction_rate_mode = "direct"` (reaction-rate tallies per
//! nuclide), cell / universe / mesh domains, and the statepoint and input
//! files. Cross sections come from the caller's `lookup`, as in
//! [`super::microxs`]. Flux units are this crate's tally units (per source
//! particle), like upstream's.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::microxs::{MultigroupMicroXs, NuclideReaction};
use crate::prelude::{FilterKind, Geometry, KeffSettings, Material, Nuclide, ScoreType, SourceBox, Tally, TallyBin};
use crate::tally::filter::{EnergyFilter, MaterialFilter};

/// One domain's result.
#[derive(Debug, Clone)]
pub struct DomainMicroXs {
    /// Index of the material in the material array.
    pub material: usize,
    /// Group fluxes (tally mean per source particle), `G` of them.
    pub group_flux: Vec<f64>,
    /// Their sum, which is what upstream returns.
    pub flux: f64,
    /// One-group microscopic cross sections collapsed on `group_flux`.
    pub micro: MultigroupMicroXs,
}

/// Upstream `get_microxs_and_flux(model, materials, nuclides, reactions,
/// energies, reaction_rate_mode="flux")` on a CSG model.
///
/// # Errors
/// Fewer than two group bounds, a transport error, or
/// `from_multigroup_flux`'s.
#[allow(clippy::too_many_arguments)]
pub fn get_microxs_and_flux(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: SourceBox,
    settings: &KeffSettings,
    domains: &[usize],
    nuclide_names: &[&str],
    reactions: &[&str],
    energies: &[f64],
    lookup: impl Fn(&str, i32) -> Option<NuclideReaction>,
) -> Result<Vec<DomainMicroXs>, String> {
    if energies.len() < 2 {
        return Err("need at least one energy group".into());
    }
    let groups = energies.len() - 1;
    let mut tally = Tally {
        id: 1,
        name: "MicroXS flux 0".into(),
        filters: vec![
            FilterKind::Material(MaterialFilter { material_indices: domains.to_vec() }),
            FilterKind::Energy(EnergyFilter { bins: energies.to_vec() }),
        ],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); domains.len() * groups], derivative: None,
    };
    crate::physics::transport_csg::try_run_keff_csg(geom, materials, nuclides, source, settings, Some(&mut tally))
        .map_err(|e| e.to_string())?;
    let n = settings.n_active as u64;
    let mut out = Vec::with_capacity(domains.len());
    for (d, &m) in domains.iter().enumerate() {
        // Row-major over the filters, the first outermost (`scoring.rs`).
        let group_flux: Vec<f64> = (0..groups).map(|g| tally.bins[d * groups + g].mean(n)).collect();
        let micro = MultigroupMicroXs::from_multigroup_flux(energies, &group_flux, nuclide_names, reactions, &lookup)?;
        let flux = group_flux.iter().sum();
        out.push(DomainMicroXs { material: m, group_flux, flux, micro });
    }
    Ok(out)
}
