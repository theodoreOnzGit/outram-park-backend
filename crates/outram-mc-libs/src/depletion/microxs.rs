// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream files, read line for line:
//     src/reaction.cpp            (Reaction::collapse_rate, :125-197)
//     src/nuclide.cpp             (Nuclide::collapse_rate, :1045-1076)
//     include/openmc/search.h     (lower_bound_index)
//     openmc/deplete/microxs.py   (MicroXS.__init__, from_multigroup_flux,
//                                  merge, _valid_rxns)
//     openmc/data/reaction.py     (REACTION_MT, for the names used here)
//
// An OUTRAM PARK translation for research, education and V&V only.

//! **Multigroup microscopic cross sections collapsed from a known flux**
//! (gh:#1625): OpenMC's `MicroXS` and the `collapse_rate` it calls.
//!
//! `collapse_rate` integrates a pointwise reaction cross section against a
//! group flux taken **flat in energy within each group** (`flux_per_eV =
//! flux[j] / ΔE_j`), segment by segment on the nuclide's energy grid, with
//! the cross section linear on each segment. `from_multigroup_flux` first
//! normalises the flux to sum 1, so the result is a one-group cross section
//! in barns.
//!
//! What is not here: `get_microxs_and_flux`, which runs a transport
//! calculation with tallies (gh:#1625's second half, through this crate's
//! transport), and the CSV/HDF5 readers and writers.
//!
//! # Departures
//!
//! 1. **Data comes from a lookup closure**, `(nuclide, MT) -> Option<…>`,
//!    instead of `openmc.lib.load_nuclide`; `None` is upstream's "nuclide
//!    has no data" (a zero row) or "no such reaction" (`i_rx < 0`, zero).
//! 2. **A first group edge above the whole grid** returns 0. Upstream's
//!    `lower_bound_index` then gives the last index and the loop reads
//!    `grid[i_low + 1]` past the end (undefined behaviour in C++).
//! 3. **Named group structures** (`energies = "CASMO-40"` etc.) are not
//!    carried; pass the bounds.
//!
//! # Verification status (2026-11-17)
//!
//! Analytic tests only (below). Not yet compared against OpenMC by running
//! it.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

/// The reaction names `MicroXS` accepts (upstream `_valid_rxns`: the keys
/// of `chain.REACTIONS`, then `fission` and `damage-energy`), with the MT
/// `REACTION_MT` gives each.
pub const MICROXS_REACTIONS: &[(&str, i32)] = &[
    ("(n,2nd)", 11),
    ("(n,2n)", 16),
    ("(n,3n)", 17),
    ("(n,na)", 22),
    ("(n,n3a)", 23),
    ("(n,2na)", 24),
    ("(n,3na)", 25),
    ("(n,np)", 28),
    ("(n,n2a)", 29),
    ("(n,2n2a)", 30),
    ("(n,nd)", 32),
    ("(n,nt)", 33),
    ("(n,n3He)", 34),
    ("(n,nd2a)", 35),
    ("(n,nt2a)", 36),
    ("(n,4n)", 37),
    ("(n,2np)", 41),
    ("(n,3np)", 42),
    ("(n,n2p)", 44),
    ("(n,npa)", 45),
    ("(n,gamma)", 102),
    ("(n,p)", 103),
    ("(n,d)", 104),
    ("(n,t)", 105),
    ("(n,3He)", 106),
    ("(n,a)", 107),
    ("(n,2a)", 108),
    ("(n,3a)", 109),
    ("(n,2p)", 111),
    ("(n,pa)", 112),
    ("(n,t2a)", 113),
    ("(n,d2a)", 114),
    ("(n,pd)", 115),
    ("(n,pt)", 116),
    ("(n,da)", 117),
    ("(n,5n)", 152),
    ("(n,6n)", 153),
    ("(n,2nt)", 154),
    ("(n,ta)", 155),
    ("(n,4np)", 156),
    ("(n,3nd)", 157),
    ("(n,nda)", 158),
    ("(n,2npa)", 159),
    ("(n,7n)", 160),
    ("(n,8n)", 161),
    ("(n,5np)", 162),
    ("(n,6np)", 163),
    ("(n,7np)", 164),
    ("(n,4na)", 165),
    ("(n,5na)", 166),
    ("(n,6na)", 167),
    ("(n,7na)", 168),
    ("(n,4nd)", 169),
    ("(n,5nd)", 170),
    ("(n,6nd)", 171),
    ("(n,3nt)", 172),
    ("(n,4nt)", 173),
    ("(n,5nt)", 174),
    ("(n,6nt)", 175),
    ("(n,2n3He)", 176),
    ("(n,3n3He)", 177),
    ("(n,4n3He)", 178),
    ("(n,3n2p)", 179),
    ("(n,3n2a)", 180),
    ("(n,3npa)", 181),
    ("(n,dt)", 182),
    ("(n,npd)", 183),
    ("(n,npt)", 184),
    ("(n,ndt)", 185),
    ("(n,np3He)", 186),
    ("(n,nd3He)", 187),
    ("(n,nt3He)", 188),
    ("(n,nta)", 189),
    ("(n,2n2p)", 190),
    ("(n,p3He)", 191),
    ("(n,d3He)", 192),
    ("(n,3Hea)", 193),
    ("(n,4n2p)", 194),
    ("(n,4n2a)", 195),
    ("(n,4npa)", 196),
    ("(n,3p)", 197),
    ("(n,n3p)", 198),
    ("(n,3n2pa)", 199),
    ("(n,5n2p)", 200),
    ("fission", 18),
    ("damage-energy", 444),
];

/// Upstream `REACTION_MT[name]` for a name `MicroXS` accepts.
pub fn reaction_mt(name: &str) -> Option<i32> {
    MICROXS_REACTIONS.iter().find(|(n, _)| *n == name).map(|(_, mt)| *mt)
}

/// One reaction's pointwise data at one temperature: the nuclide's energy
/// grid \[eV\], the cross section \[b\] starting at grid index `threshold`
/// (upstream `xs_[i_temp].value` and `.threshold`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReactionTable {
    /// Nuclide energy grid, ascending.
    pub grid: Vec<f64>,
    /// Cross section from `grid[threshold]` on.
    pub xs: Vec<f64>,
    /// Index of the first grid point carrying this reaction.
    pub threshold: usize,
}

/// A reaction at a requested temperature: the lower table and, when the
/// temperature lies between two, the upper one with the interpolation
/// factor `f` (upstream `find_temperature`).
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideReaction {
    /// Table at the lower (or only) temperature.
    pub low: ReactionTable,
    /// Table at the upper temperature and `f`, if interpolating.
    pub high: Option<(ReactionTable, f64)>,
}

/// Upstream `lower_bound_index`: `-1` below the first point.
fn lower_bound_index(grid: &[f64], value: f64) -> isize {
    if grid[0] == value {
        return 0;
    }
    grid.partition_point(|g| *g < value) as isize - 1
}

/// Upstream `Reaction::collapse_rate`: `Σ_j flux_j/ΔE_j ∫_j σ(E) dE`.
pub fn reaction_collapse_rate(table: &ReactionTable, energy: &[f64], flux: &[f64]) -> f64 {
    let grid = &table.grid;
    let xs = &table.xs;
    if grid.len() < 2 || energy.len() != flux.len() + 1 || energy.is_empty() {
        return 0.0;
    }
    let mut i_low = lower_bound_index(grid, energy[0]);
    let mut j_start = 0usize;
    let i_threshold = table.threshold as isize;
    if i_low < i_threshold {
        i_low = i_threshold;
        while energy[j_start + 1] < grid[i_low as usize] {
            j_start += 1;
            if j_start + 1 == energy.len() {
                return 0.0;
            }
        }
    }
    let mut i_low = i_low as usize;
    if i_low + 1 >= grid.len() {
        return 0.0; // departure 2
    }
    let mut xs_flux_sum = 0.0;
    for j in j_start..flux.len() {
        let e_group_low = energy[j];
        let e_group_high = energy[j + 1];
        let flux_per_ev = flux[j] / (e_group_high - e_group_low);
        let mut i_high = i_low;
        while grid[i_high + 1] < e_group_high && i_high + 1 < grid.len() - 1 {
            i_high += 1;
        }
        while i_low <= i_high {
            let e_l = grid[i_low];
            let e_r = grid[i_low + 1];
            if e_l != e_r {
                let xs_l = xs[i_low - table.threshold];
                let xs_r = xs[i_low + 1 - table.threshold];
                let e_low = e_group_low.max(e_l);
                let e_high = e_group_high.min(e_r);
                let m = (xs_r - xs_l) / (e_r - e_l);
                let xs_low = xs_l + m * (e_low - e_l);
                let xs_high = xs_l + m * (e_high - e_l);
                let xs_avg = 0.5 * (xs_low + xs_high);
                let de = e_high - e_low;
                xs_flux_sum += flux_per_ev * xs_avg * de;
            }
            i_low += 1;
        }
        i_low = i_high;
        if i_low + 1 == grid.len() {
            break;
        }
    }
    xs_flux_sum
}

/// Upstream `Nuclide::collapse_rate`: linear in temperature between the two
/// bracketing tables.
pub fn nuclide_collapse_rate(rx: &NuclideReaction, energy: &[f64], flux: &[f64]) -> f64 {
    let rr_low = reaction_collapse_rate(&rx.low, energy, flux);
    match &rx.high {
        Some((high, f)) if *f > 0.0 => {
            let rr_high = reaction_collapse_rate(high, energy, flux);
            rr_low + f * (rr_high - rr_low)
        }
        _ => rr_low,
    }
}

/// Upstream `MicroXS`: microscopic cross sections \[b\] indexed by nuclide,
/// reaction and group.
#[derive(Debug, Clone, PartialEq)]
pub struct MultigroupMicroXs {
    data: Vec<f64>,
    groups: usize,
    nuclides: Vec<String>,
    reactions: Vec<String>,
}

impl MultigroupMicroXs {
    /// Upstream `MicroXS(data, nuclides, reactions)`; `data` is row-major
    /// `[nuclide][reaction][group]`.
    ///
    /// # Errors
    /// A size mismatch or an unknown reaction name.
    pub fn new(data: Vec<f64>, nuclides: Vec<String>, reactions: Vec<String>, groups: usize) -> Result<Self, String> {
        if data.len() != nuclides.len() * reactions.len() * groups {
            return Err(format!(
                "Nuclides list of length {} and reactions array of length {} do not match a data array of {} values over {groups} groups",
                nuclides.len(),
                reactions.len(),
                data.len()
            ));
        }
        for r in &reactions {
            if reaction_mt(r).is_none() {
                return Err(format!("reactions: {r} is not a valid reaction"));
            }
        }
        Ok(Self { data, groups, nuclides, reactions })
    }

    /// Upstream `MicroXS.from_multigroup_flux`, one group out. The flux is
    /// normalised to sum 1; a zero flux gives zero cross sections. A
    /// nuclide or reaction `lookup` returns `None` for stays zero.
    ///
    /// # Errors
    /// Bounds not strictly ascending, a flux of the wrong length, or an
    /// unknown reaction name.
    pub fn from_multigroup_flux(
        energies: &[f64],
        multigroup_flux: &[f64],
        nuclides: &[&str],
        reactions: &[&str],
        lookup: impl Fn(&str, i32) -> Option<NuclideReaction>,
    ) -> Result<Self, String> {
        if energies.windows(2).any(|w| !(w[1] - w[0] > 0.0)) {
            return Err("Energy group boundaries must be in ascending order".into());
        }
        if multigroup_flux.len() + 1 != energies.len() {
            return Err("Length of flux array should be len(energies)-1".into());
        }
        let mut mts = Vec::with_capacity(reactions.len());
        for r in reactions {
            mts.push(reaction_mt(r).ok_or_else(|| format!("reactions: {r} is not a valid reaction"))?);
        }
        let nucs: Vec<String> = nuclides.iter().map(|s| (*s).to_owned()).collect();
        let rxs: Vec<String> = reactions.iter().map(|s| (*s).to_owned()).collect();
        let mut data = vec![0.0; nucs.len() * mts.len()];
        let flux_sum: f64 = multigroup_flux.iter().sum();
        if flux_sum == 0.0 {
            return Self::new(data, nucs, rxs, 1);
        }
        let flux: Vec<f64> = multigroup_flux.iter().map(|f| f / flux_sum).collect();
        for (i, nuc) in nuclides.iter().enumerate() {
            for (k, mt) in mts.iter().enumerate() {
                if let Some(rx) = lookup(nuc, *mt) {
                    data[i * mts.len() + k] = nuclide_collapse_rate(&rx, energies, &flux);
                }
            }
        }
        Self::new(data, nucs, rxs, 1)
    }

    /// `micro[nuclide, reaction]` in group `g` \[b\].
    pub fn get(&self, nuclide: &str, reaction: &str, group: usize) -> Option<f64> {
        let i = self.nuclides.iter().position(|n| n == nuclide)?;
        let k = self.reactions.iter().position(|r| r == reaction)?;
        (group < self.groups).then(|| self.data[(i * self.reactions.len() + k) * self.groups + group])
    }

    /// Nuclides, in order.
    pub fn nuclides(&self) -> &[String] {
        &self.nuclides
    }

    /// Reactions, in order.
    pub fn reactions(&self) -> &[String] {
        &self.reactions
    }

    /// Upstream `MicroXS.merge`: the union of nuclides and reactions (this
    /// one's first, then the other's new ones). On overlap `prefer_other`
    /// takes `other`'s value, else this one's; entries in neither stay 0.
    ///
    /// # Errors
    /// Different group counts.
    pub fn merge(&self, other: &Self, prefer_other: bool) -> Result<Self, String> {
        if self.groups != other.groups {
            return Err(format!(
                "Cannot merge MicroXS with different number of energy groups: {} vs {}",
                self.groups, other.groups
            ));
        }
        let mut nucs = self.nuclides.clone();
        for n in &other.nuclides {
            if !nucs.contains(n) {
                nucs.push(n.clone());
            }
        }
        let mut rxs = self.reactions.clone();
        for r in &other.reactions {
            if !rxs.contains(r) {
                rxs.push(r.clone());
            }
        }
        let g = self.groups;
        let mut data = vec![0.0; nucs.len() * rxs.len() * g];
        let at = |i: usize, k: usize, gg: usize| (i * rxs.len() + k) * g + gg;
        for (i, _) in self.nuclides.iter().enumerate() {
            for (k, _) in self.reactions.iter().enumerate() {
                for gg in 0..g {
                    data[at(i, k, gg)] = self.data[(i * self.reactions.len() + k) * g + gg];
                }
            }
        }
        for (oi, n) in other.nuclides.iter().enumerate() {
            let i = nucs.iter().position(|x| x == n).unwrap_or(0);
            for (ok, r) in other.reactions.iter().enumerate() {
                let k = rxs.iter().position(|x| x == r).unwrap_or(0);
                let new = !self.nuclides.contains(n) || !self.reactions.contains(r);
                if prefer_other || new {
                    for gg in 0..g {
                        data[at(i, k, gg)] = other.data[(oi * other.reactions.len() + ok) * g + gg];
                    }
                }
            }
        }
        Self::new(data, nucs, rxs, g)
    }
}

#[cfg(test)]
mod tests;
