// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Multigroup cross sections (MGXS) condensed from a Monte Carlo run
//!
//! This is the bridge from **stochastic transport to deterministic transport**:
//! it runs [`outram_mc_libs`] over a geometry, tallies flux-weighted reaction
//! rates on a user group structure, and divides them into macroscopic
//! multigroup cross sections that a deterministic solver — GeN-Foam's
//! diffusion / SP3 / SN neutronics — can consume.
//!
//! It implements no physics. Every number here is a ratio of two Monte Carlo
//! tallies produced by `outram-mc-libs`, which is exactly the composition role
//! [`crate`] exists for.
//!
//! ## The definition being applied
//!
//! For zone `z`, group `g`, reaction `x`, the flux-weighted group constant is
//!
//! ```text
//!   Sigma_x,g = ( integral over group g of Sigma_x(E) phi(E) dE ) / ( integral over group g of phi(E) dE )
//! ```
//!
//! The numerator is a track-length reaction-rate tally and the denominator a
//! track-length flux tally, both over the same spatial and energy filters, so
//! the ratio is the standard MC-condensed group constant. The scattering matrix
//! needs one extra axis:
//!
//! ```text
//!   Sigma_s,g->g' = ( scatter events from g into g' ) / ( integral over group g of phi(E) dE )
//! ```
//!
//! and that numerator is the analog estimator
//! [`outram_mc_libs::tally::scoring::score_scatter_matrix`], reachable only
//! because a tally can now carry an outgoing-energy filter.
//!
//! ## Two passes, one seed
//!
//! A [`outram_mc_libs::tally::tally::Tally`] carries one filter set, and the
//! scattering matrix needs an outgoing-energy axis the scalar reaction rates
//! must not have — a scalar tally binned by outgoing energy would be wrong, and
//! the matrix tally deliberately refuses to score flux (see
//! `score_scatter_matrix`). So the generator makes **two** k-eigenvalue passes
//! over the same model at the **same seed**, which transport byte-identical
//! histories:
//!
//! | pass | filters | scores |
//! |---|---|---|
//! | scalar | `[Material, Energy]` | `Flux, Total, Absorption, NuFission, KappaFission` |
//! | matrix | `[Material, Energy, EnergyOut]` | `ScatterN` |
//!
//! Because the histories are identical, the flux denominator from the scalar
//! pass is the correct denominator for the matrix pass; they are not two
//! independent estimates that happen to be close.
//!
//! ## Group ordering — read this before indexing
//!
//! Energy filter bins ascend in energy, so **group index 0 is the LOWEST
//! energy** here. Reactor convention is the opposite: group 0 is usually the
//! fastest. This module keeps the filter's own ascending order throughout,
//! because silently reversing it is how an MGXS set ends up transposed, and
//! offers [`MgxsLibrary::in_descending_energy`] for the conventional view.
//! Nothing is reversed implicitly.
//!
//! ## What this does NOT do
//!
//! - **No Legendre scattering moments.** Only the isotropic `P0` matrix is
//!   tallied. GeN-Foam's `ZoneNuclearData` stores `scattering[moment][..][..]`
//!   and SP3 wants `P1`; that needs a scattering-cosine axis on the matrix
//!   tally and is not done here.
//! - **No delayed-neutron data.** `beta`, `lambda` and the delayed spectrum are
//!   not condensed; a deterministic transient needs them and they must come
//!   from elsewhere.
//! - **No feedback parametrisation.** `ZoneNuclearData` interpolates cross
//!   sections over reactor state (fuel temperature, density). One run gives one
//!   state point, the reference state.
//! - **No validation.** Condensing correctly is not the same as the group
//!   constants reproducing a reference eigenvalue. Nothing here is validated.

use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Errors from condensing a Monte Carlo run into group constants.
#[derive(Debug, thiserror::Error)]
pub enum MgxsError {
    /// A group structure needs at least two edges to define one group.
    #[error("group structure needs at least 2 edges, got {n}")]
    TooFewEdges {
        /// Number of edges supplied.
        n: usize,
    },
    /// Group edges must ascend strictly; a flat or descending pair makes the
    /// bin mapping ambiguous.
    #[error(
        "group edges must strictly ascend: edge {i} = {lo} eV is not below edge {j} = {hi} eV"
    )]
    EdgesNotAscending {
        /// Index of the lower edge.
        i: usize,
        /// Index of the upper edge.
        j: usize,
        /// Lower edge value \[eV\].
        lo: f64,
        /// Upper edge value \[eV\].
        hi: f64,
    },
    /// A tally's bin count does not match the filter structure it was declared
    /// with, so the flat index arithmetic would read the wrong bin.
    #[error("tally '{name}' has {got} bins but {want} were expected for {zones} zones x {groups} groups (x {out_groups} outgoing)")]
    BinCountMismatch {
        /// Tally name.
        name: String,
        /// Bins actually present.
        got: usize,
        /// Bins the filter structure implies.
        want: usize,
        /// Zone count used in the calculation.
        zones: usize,
        /// Group count used in the calculation.
        groups: usize,
        /// Outgoing-group count (1 for a scalar tally).
        out_groups: usize,
    },
}

/// An energy group structure, as ascending bin edges in eV.
///
/// `n + 1` edges define `n` groups, and **group 0 is the lowest-energy group**
/// — see the module note on ordering.
#[derive(Debug, Clone)]
pub struct GroupStructure {
    edges: Vec<f64>,
}

impl GroupStructure {
    /// Build a structure from ascending edges in eV.
    ///
    /// # Errors
    ///
    /// [`MgxsError::TooFewEdges`] with fewer than two edges, or
    /// [`MgxsError::EdgesNotAscending`] if any adjacent pair does not strictly
    /// increase.
    pub fn new(edges: Vec<f64>) -> Result<Self, MgxsError> {
        if edges.len() < 2 {
            return Err(MgxsError::TooFewEdges { n: edges.len() });
        }
        for i in 0..edges.len() - 1 {
            if !(edges[i] < edges[i + 1]) {
                return Err(MgxsError::EdgesNotAscending {
                    i,
                    j: i + 1,
                    lo: edges[i],
                    hi: edges[i + 1],
                });
            }
        }
        Ok(Self { edges })
    }

    /// The conventional **two-group** structure: thermal below `split_ev`,
    /// fast above, spanning 1e-5 eV to 20 MeV.
    ///
    /// `split_ev = 2.38` is the cadmium cut-off, the usual thermal/fast
    /// boundary. A coarse structure like this is often the RIGHT choice for a
    /// Monte Carlo condensation: every group must carry flux, and a fine log
    /// grid leaves groups empty in a spectrum that does not span it.
    ///
    /// # Errors
    ///
    /// As [`Self::new`], if `split_ev` is not strictly inside the range.
    pub fn two_group(split_ev: f64) -> Result<Self, MgxsError> {
        Self::new(vec![1.0e-5, split_ev, 2.0e7])
    }

    /// `n` log-spaced groups between `e_lo` and `e_hi` \[eV\].
    ///
    /// # Errors
    ///
    /// As [`Self::new`]; also if `e_lo` is not positive, since a log grid
    /// cannot start at or below zero.
    pub fn log_spaced(e_lo: f64, e_hi: f64, n: usize) -> Result<Self, MgxsError> {
        if n == 0 || !(e_lo > 0.0) || !(e_hi > e_lo) {
            return Err(MgxsError::TooFewEdges { n: n + 1 });
        }
        let (l0, l1) = (e_lo.ln(), e_hi.ln());
        let edges = (0..=n)
            .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
            .collect();
        Self::new(edges)
    }

    /// Number of groups (`edges.len() - 1`).
    #[must_use]
    pub fn n_groups(&self) -> usize {
        self.edges.len() - 1
    }

    /// The ascending edges in eV.
    #[must_use]
    pub fn edges(&self) -> &[f64] {
        &self.edges
    }
}

/// One zone's condensed group constants.
///
/// Every vector is indexed by group in the structure's own **ascending-energy**
/// order. Cross sections are macroscopic, in cm^-1; `kappa_fission` is a power
/// density per unit flux (J cm^-1).
#[derive(Debug, Clone)]
pub struct ZoneMgxs {
    /// Human-readable zone name, carried through from the material.
    pub name: String,
    /// Group scalar flux \[arbitrary, per source particle\]. This is the
    /// weighting denominator, kept because a zero here is what makes every
    /// other entry in the group meaningless, and callers must be able to see it.
    pub flux: Vec<f64>,
    /// Total macroscopic cross section \[cm^-1\].
    pub total: Vec<f64>,
    /// Absorption (capture + fission) \[cm^-1\].
    pub absorption: Vec<f64>,
    /// Neutron production `nu * Sigma_f` \[cm^-1\].
    pub nu_fission: Vec<f64>,
    /// Fission energy deposition `kappa * Sigma_f` \[J cm^-1\].
    pub kappa_fission: Vec<f64>,
    /// Isotropic (`P0`) scattering matrix, `scatter[g_in][g_out]` \[cm^-1\].
    pub scatter: Vec<Vec<f64>>,
    /// Fission spectrum `chi_g`, normalised to sum to 1 over groups.
    ///
    /// **Measured, not assumed**: condensed from the energies fission neutrons
    /// were actually born with during the run, via
    /// `outram_mc_libs::tally::scoring::score_fission_birth`. A zone that
    /// fissioned not at all carries all zeros, and the sum is then 0 rather
    /// than 1 -- check before using it as a source distribution.
    pub chi: Vec<f64>,
}

impl ZoneMgxs {
    /// Number of groups.
    #[must_use]
    pub fn n_groups(&self) -> usize {
        self.flux.len()
    }

    /// Removal cross section for group `g`: everything that takes a neutron out
    /// of the group, `Sigma_t,g - Sigma_s,g->g`.
    ///
    /// This is the quantity GeN-Foam's `ZoneNuclearData` stores as
    /// `sigma_removal`. Within-group scattering is subtracted because it does
    /// not remove the neutron from the group.
    ///
    /// Returns `None` if `g` is out of range.
    #[must_use]
    pub fn removal(&self, g: usize) -> Option<f64> {
        let total = *self.total.get(g)?;
        let within = *self.scatter.get(g)?.get(g)?;
        Some(total - within)
    }

    /// Diffusion coefficient `D_g = 1 / (3 Sigma_tr,g)` \[cm\].
    ///
    /// **Uses the TOTAL cross section, not a transport-corrected one.** With
    /// only a `P0` scattering matrix tallied there is no `mu-bar` available, so
    /// `Sigma_tr` cannot be formed and this is the `Sigma_t` approximation. It
    /// is adequate for a scoping diffusion solve and **not** adequate where
    /// anisotropic scattering matters, notably in a graphite reflector. A `P1`
    /// matrix would fix it; see the module's "what this does NOT do".
    ///
    /// Returns `None` if `g` is out of range or the total is non-positive.
    #[must_use]
    pub fn diffusion_coefficient(&self, g: usize) -> Option<f64> {
        let total = *self.total.get(g)?;
        if total > 0.0 {
            Some(1.0 / (3.0 * total))
        } else {
            None
        }
    }

    /// Scattering production out of group `g`, summed over outgoing groups.
    ///
    /// Returns `None` if `g` is out of range.
    #[must_use]
    pub fn scatter_out(&self, g: usize) -> Option<f64> {
        Some(self.scatter.get(g)?.iter().sum())
    }
}

/// A full MGXS set: one group structure, one entry per zone.
#[derive(Debug, Clone)]
pub struct MgxsLibrary {
    /// The group structure every zone is condensed onto.
    pub groups: GroupStructure,
    /// Per-zone group constants, in the order the zones were supplied.
    pub zones: Vec<ZoneMgxs>,
}

impl MgxsLibrary {
    /// The same library with every group axis reversed, so index 0 is the
    /// **highest**-energy group — the usual reactor-physics convention.
    ///
    /// Provided explicitly rather than applied implicitly: a silently
    /// transposed MGXS set is one of the easier ways to get a plausible but
    /// wrong eigenvalue, so the caller states which convention it wants.
    #[must_use]
    pub fn in_descending_energy(&self) -> Self {
        let n = self.groups.n_groups();
        let rev = |v: &Vec<f64>| -> Vec<f64> { v.iter().rev().copied().collect() };
        let zones = self
            .zones
            .iter()
            .map(|z| {
                // Both axes of the matrix reverse together: element
                // (g_in, g_out) moves to (n-1-g_in, n-1-g_out).
                let mut scatter = vec![vec![0.0; n]; n];
                for gi in 0..n {
                    for go in 0..n {
                        scatter[n - 1 - gi][n - 1 - go] = z.scatter[gi][go];
                    }
                }
                ZoneMgxs {
                    name: z.name.clone(),
                    flux: rev(&z.flux),
                    total: rev(&z.total),
                    absorption: rev(&z.absorption),
                    nu_fission: rev(&z.nu_fission),
                    kappa_fission: rev(&z.kappa_fission),
                    scatter,
                    chi: rev(&z.chi),
                }
            })
            .collect();
        let mut edges = self.groups.edges.clone();
        edges.reverse();
        // Re-ascend the edge list so `GroupStructure`'s invariant still holds;
        // the edges themselves are symmetric under the group reversal.
        edges.reverse();
        Self {
            groups: GroupStructure { edges },
            zones,
        }
    }
}

/// The score order the scalar pass must declare, and that [`condense`] assumes.
///
/// Exposed so a caller building the tally cannot silently disagree with the
/// reader about which score sits at which offset.
/// Scores carried by the matrix pass, in order: the scattering matrix and the
/// fission-birth spectrum. Both need the same `[Material, Energy, EnergyOut]`
/// filters, so they share one tally and one transport pass.
pub const MATRIX_SCORES: usize = 2;

pub const SCALAR_SCORES: [ScoreType; 5] = [
    ScoreType::Flux,
    ScoreType::Total,
    ScoreType::Absorption,
    ScoreType::NuFission,
    ScoreType::KappaFission,
];

/// Condense a completed pair of Monte Carlo tallies into group constants.
///
/// # Parameters
/// - `groups` — the structure both tallies were binned on.
/// - `zone_names` — one name per zone, in **material-index order**, matching
///   the `MaterialFilter` the tallies carried.
/// - `scalar` — a tally with filters `[Material, Energy]` and scores exactly
///   [`SCALAR_SCORES`], already run.
/// - `matrix` — a tally with filters `[Material, Energy, EnergyOut]` and score
///   `[ScatterN]`, already run at the same seed.
/// - `n_realizations` — active batch count, for [`TallyBin::mean`].
///
/// # Errors
///
/// [`MgxsError::BinCountMismatch`] if either tally's bin count disagrees with
/// `zone_names.len()` and `groups.n_groups()`. That check exists because the
/// flat index arithmetic below would otherwise read a neighbouring zone's bin
/// and produce plausible nonsense.
///
/// # Zero-flux groups
///
/// A group a zone never saw has zero flux and no defined cross section. Rather
/// than emit a NaN or an infinity that would propagate into a solver, every
/// cross section in such a group is set to `0.0` and the zero flux is left
/// visible in [`ZoneMgxs::flux`] so a caller can detect it. This is a real
/// situation in a thermal reactor: a fast group in a deep reflector can go
/// unvisited at modest particle counts.
pub fn condense(
    groups: &GroupStructure,
    zone_names: &[String],
    scalar: &Tally,
    matrix: &Tally,
    n_realizations: u64,
) -> Result<MgxsLibrary, MgxsError> {
    let n_z = zone_names.len();
    let n_g = groups.n_groups();
    let n_s = SCALAR_SCORES.len();

    let want_scalar = n_z * n_g * n_s;
    if scalar.bins.len() != want_scalar {
        return Err(MgxsError::BinCountMismatch {
            name: scalar.name.clone(),
            got: scalar.bins.len(),
            want: want_scalar,
            zones: n_z,
            groups: n_g,
            out_groups: 1,
        });
    }
    let want_matrix = n_z * n_g * n_g * MATRIX_SCORES;
    if matrix.bins.len() != want_matrix {
        return Err(MgxsError::BinCountMismatch {
            name: matrix.name.clone(),
            got: matrix.bins.len(),
            want: want_matrix,
            zones: n_z,
            groups: n_g,
            out_groups: n_g,
        });
    }

    // Flat index for [Material, Energy] x scores: bin = z*n_g + g, then
    // bins[bin*n_s + s]. Mirrors `filter_bin`'s row-major,
    // first-filter-slowest-varying layout.
    let scal = |z: usize, g: usize, s: usize| -> f64 {
        scalar.bins[(z * n_g + g) * n_s + s].mean(n_realizations)
    };
    // [Material, Energy, EnergyOut] with MATRIX_SCORES scores: score 0 is the
    // scattering matrix, score 1 the fission-birth count.
    let mat = |z: usize, gi: usize, go: usize, s: usize| -> f64 {
        matrix.bins[((z * n_g + gi) * n_g + go) * MATRIX_SCORES + s].mean(n_realizations)
    };

    let mut zones = Vec::with_capacity(n_z);
    for (z, name) in zone_names.iter().enumerate() {
        let mut flux = vec![0.0; n_g];
        let mut total = vec![0.0; n_g];
        let mut absorption = vec![0.0; n_g];
        let mut nu_fission = vec![0.0; n_g];
        let mut kappa_fission = vec![0.0; n_g];
        let mut scatter = vec![vec![0.0; n_g]; n_g];

        for g in 0..n_g {
            let phi = scal(z, g, 0);
            flux[g] = phi;
            if !(phi > 0.0) {
                // No flux: leave every cross section at zero rather than
                // dividing by zero. The zero flux above is the signal.
                continue;
            }
            total[g] = scal(z, g, 1) / phi;
            absorption[g] = scal(z, g, 2) / phi;
            nu_fission[g] = scal(z, g, 3) / phi;
            kappa_fission[g] = scal(z, g, 4) / phi;
            for go in 0..n_g {
                scatter[g][go] = mat(z, g, go, 0) / phi;
            }
        }

        // chi: sum the birth counts over the CAUSING group, then normalise to
        // unity. Normalising is what makes it a spectrum rather than a rate, and
        // it is deliberately done after summing so a group that caused few
        // fissions cannot dominate through a small flux denominator -- chi is
        // not flux-weighted, it is a distribution over births.
        let mut chi = vec![0.0; n_g];
        for gb in 0..n_g {
            chi[gb] = (0..n_g).map(|gi| mat(z, gi, gb, 1)).sum();
        }
        let born: f64 = chi.iter().sum();
        if born > 0.0 {
            for c in chi.iter_mut() {
                *c /= born;
            }
        }

        zones.push(ZoneMgxs {
            name: name.clone(),
            flux,
            total,
            absorption,
            nu_fission,
            kappa_fission,
            scatter,
            chi,
        });
    }

    Ok(MgxsLibrary {
        groups: groups.clone(),
        zones,
    })
}

/// Build the empty scalar-pass tally for `n_zones` materials on `groups`.
///
/// The caller runs this through a k-eigenvalue driver and hands the result to
/// [`condense`]. Provided so the filter order and score order cannot drift from
/// what `condense` indexes.
#[must_use]
pub fn scalar_tally(id: i32, groups: &GroupStructure, material_indices: Vec<usize>) -> Tally {
    use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind, MaterialFilter};
    let n = material_indices.len() * groups.n_groups() * SCALAR_SCORES.len();
    Tally {
        id,
        name: "mgxs scalar".into(),
        filters: vec![
            FilterKind::Material(MaterialFilter { material_indices }),
            FilterKind::Energy(EnergyFilter {
                bins: groups.edges().to_vec(),
            }),
        ],
        scores: SCALAR_SCORES.to_vec(),
        bins: vec![TallyBin::default(); n],
    }
}

/// Build the empty scattering-matrix tally for `n_zones` materials on `groups`.
///
/// Carries an outgoing-energy filter, which is what switches
/// `outram-mc-libs`'s analog scatter estimator on; see that crate's
/// `score_scatter_matrix`.
#[must_use]
pub fn matrix_tally(id: i32, groups: &GroupStructure, material_indices: Vec<usize>) -> Tally {
    use outram_mc_libs::tally::filter::{EnergyFilter, EnergyOutFilter, FilterKind, MaterialFilter};
    let n_g = groups.n_groups();
    let n = material_indices.len() * n_g * n_g;
    Tally {
        id,
        name: "mgxs scatter matrix".into(),
        filters: vec![
            FilterKind::Material(MaterialFilter { material_indices }),
            FilterKind::Energy(EnergyFilter {
                bins: groups.edges().to_vec(),
            }),
            FilterKind::EnergyOut(EnergyOutFilter {
                bins: groups.edges().to_vec(),
            }),
        ],
        // Two scores on one filter set: ScatterN is filled by the analog
        // scatter estimator and NuFission by the fission-birth estimator. They
        // cannot contaminate each other (each scoring function writes only its
        // own score), and a track-length event cannot reach this tally at all
        // because it carries no outgoing energy and the EnergyOutFilter
        // rejects it. That is what lets chi ride along without a third pass.
        scores: vec![ScoreType::ScatterN, ScoreType::NuFission],
        bins: vec![TallyBin::default(); n * MATRIX_SCORES],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a scalar tally whose bins are set to known values, so `condense`'s
    /// flat indexing can be checked against hand arithmetic.
    ///
    /// `set(z, g, s)` returns the value to place; each bin is scored once so
    /// `mean(1)` returns it unchanged.
    fn scalar_with(n_z: usize, n_g: usize, set: impl Fn(usize, usize, usize) -> f64) -> Tally {
        let n_s = SCALAR_SCORES.len();
        let mut bins = vec![TallyBin::default(); n_z * n_g * n_s];
        for z in 0..n_z {
            for g in 0..n_g {
                for s in 0..n_s {
                    bins[(z * n_g + g) * n_s + s].score(set(z, g, s));
                }
            }
        }
        Tally {
            id: 1,
            name: "scalar".into(),
            filters: vec![],
            scores: SCALAR_SCORES.to_vec(),
            bins,
        }
    }

    fn matrix_with(n_z: usize, n_g: usize, set: impl Fn(usize, usize, usize) -> f64) -> Tally {
        let mut bins = vec![TallyBin::default(); n_z * n_g * n_g * MATRIX_SCORES];
        for z in 0..n_z {
            for gi in 0..n_g {
                for go in 0..n_g {
                    // score 0 = scatter matrix; score 1 (fission births) is left
                    // at zero unless a test sets it through `matrix_with_chi`.
                    bins[((z * n_g + gi) * n_g + go) * MATRIX_SCORES].score(set(z, gi, go));
                }
            }
        }
        Tally {
            id: 2,
            name: "matrix".into(),
            filters: vec![],
            scores: vec![ScoreType::ScatterN, ScoreType::NuFission],
            bins,
        }
    }

    fn two_group() -> GroupStructure {
        GroupStructure::new(vec![1.0e-3, 1.0, 2.0e7]).unwrap()
    }

    /// Edges must strictly ascend, and the error names the offending pair.
    #[test]
    fn group_structure_rejects_non_ascending_edges() {
        assert!(matches!(
            GroupStructure::new(vec![1.0]),
            Err(MgxsError::TooFewEdges { n: 1 })
        ));
        assert!(matches!(
            GroupStructure::new(vec![1.0, 1.0]),
            Err(MgxsError::EdgesNotAscending { i: 0, j: 1, .. })
        ));
        assert!(matches!(
            GroupStructure::new(vec![10.0, 1.0]),
            Err(MgxsError::EdgesNotAscending { .. })
        ));
        assert_eq!(
            GroupStructure::log_spaced(1.0e-3, 2.0e7, 8)
                .unwrap()
                .n_groups(),
            8
        );
    }

    /// A cross section is the reaction rate divided by the flux, per zone and
    /// group -- and the right bin is read for each.
    ///
    /// Every score is given a value that encodes its own (z, g, s), so a
    /// transposed or offset read produces a different number rather than a
    /// coincidentally equal one.
    #[test]
    fn condense_divides_the_right_rate_by_the_right_flux() {
        let g = two_group();
        let names = vec!["fuel".to_string(), "moderator".to_string()];
        // flux = 10^(z+1) * (g+1); rate_s = flux * (s as the "cross section")
        let scalar = scalar_with(2, 2, |z, gg, s| {
            let phi = 10f64.powi(z as i32 + 1) * (gg as f64 + 1.0);
            if s == 0 {
                phi
            } else {
                phi * (s as f64)
            }
        });
        // scatter[z][gi][go] rate = flux_in * (gi + 1) * (go + 2)
        let matrix = matrix_with(2, 2, |z, gi, go| {
            let phi = 10f64.powi(z as i32 + 1) * (gi as f64 + 1.0);
            phi * (gi as f64 + 1.0) * (go as f64 + 2.0)
        });

        let lib = condense(&g, &names, &scalar, &matrix, 1).unwrap();
        assert_eq!(lib.zones.len(), 2);

        for (z, zone) in lib.zones.iter().enumerate() {
            assert_eq!(zone.name, names[z]);
            for gg in 0..2 {
                let phi = 10f64.powi(z as i32 + 1) * (gg as f64 + 1.0);
                assert!((zone.flux[gg] - phi).abs() < 1e-12, "flux z{z} g{gg}");
                // score index 1 = Total, 2 = Absorption, 3 = NuFission, 4 = Kappa
                assert!((zone.total[gg] - 1.0).abs() < 1e-12, "total z{z} g{gg}");
                assert!((zone.absorption[gg] - 2.0).abs() < 1e-12);
                assert!((zone.nu_fission[gg] - 3.0).abs() < 1e-12);
                assert!((zone.kappa_fission[gg] - 4.0).abs() < 1e-12);
                for go in 0..2 {
                    let want = (gg as f64 + 1.0) * (go as f64 + 2.0);
                    assert!(
                        (zone.scatter[gg][go] - want).abs() < 1e-12,
                        "scatter z{z} {gg}->{go}: got {} want {want}",
                        zone.scatter[gg][go]
                    );
                }
            }
        }
    }

    /// The scattering matrix is NOT transposed: `scatter[g_in][g_out]`.
    ///
    /// Built with a single non-zero element at (0 -> 1). If `condense` read the
    /// matrix transposed it would appear at (1 -> 0) and this fails. A
    /// transposed scattering matrix is one of the easier ways to get a
    /// plausible-looking but wrong eigenvalue, so it gets its own test.
    #[test]
    fn scatter_matrix_is_not_transposed() {
        let g = two_group();
        let names = vec!["only".to_string()];
        let scalar = scalar_with(1, 2, |_z, _g, s| if s == 0 { 1.0 } else { 0.0 });
        let matrix = matrix_with(
            1,
            2,
            |_z, gi, go| if (gi, go) == (0, 1) { 7.0 } else { 0.0 },
        );
        let lib = condense(&g, &names, &scalar, &matrix, 1).unwrap();
        let s = &lib.zones[0].scatter;
        assert_eq!(s[0][1], 7.0, "the 0->1 element must land at [0][1]");
        assert_eq!(
            s[1][0], 0.0,
            "nothing scattered 1->0; a transpose would put 7 here"
        );
        assert_eq!(s[0][0], 0.0);
        assert_eq!(s[1][1], 0.0);
    }

    /// Removal subtracts within-group scattering; the diffusion coefficient is
    /// 1/(3 Sigma_t).
    #[test]
    fn removal_and_diffusion_follow_their_definitions() {
        let z = ZoneMgxs {
            name: "z".into(),
            flux: vec![1.0, 1.0],
            total: vec![2.0, 4.0],
            absorption: vec![0.5, 0.5],
            nu_fission: vec![0.0, 0.0],
            kappa_fission: vec![0.0, 0.0],
            scatter: vec![vec![0.75, 0.25], vec![0.1, 1.5]],
            chi: vec![0.6, 0.4],
        };
        // group 0: total 2.0, within-group 0.75 -> removal 1.25
        assert!((z.removal(0).unwrap() - 1.25).abs() < 1e-12);
        // group 1: total 4.0, within-group 1.5 -> removal 2.5
        assert!((z.removal(1).unwrap() - 2.5).abs() < 1e-12);
        assert!((z.diffusion_coefficient(0).unwrap() - 1.0 / 6.0).abs() < 1e-12);
        assert!((z.scatter_out(0).unwrap() - 1.0).abs() < 1e-12);
        assert!(z.removal(9).is_none(), "out-of-range group must be None");
    }

    /// A group the zone never saw yields zero cross sections, not NaN.
    ///
    /// Dividing by a zero flux would put a NaN or an infinity into a
    /// deterministic solver, where it propagates silently. The zero flux stays
    /// visible so a caller can tell "unvisited" from "genuinely zero".
    #[test]
    fn zero_flux_group_yields_zeros_not_nan() {
        let g = two_group();
        let names = vec!["z".to_string()];
        // group 1 has zero flux but a non-zero recorded rate.
        let scalar = scalar_with(1, 2, |_z, gg, s| {
            if gg == 1 {
                if s == 0 {
                    0.0
                } else {
                    5.0
                }
            } else if s == 0 {
                1.0
            } else {
                3.0
            }
        });
        let matrix = matrix_with(1, 2, |_z, _gi, _go| 1.0);
        let lib = condense(&g, &names, &scalar, &matrix, 1).unwrap();
        let z = &lib.zones[0];
        assert_eq!(z.flux[1], 0.0, "the zero flux must remain visible");
        for v in [
            z.total[1],
            z.absorption[1],
            z.nu_fission[1],
            z.kappa_fission[1],
        ] {
            assert_eq!(v, 0.0, "unvisited group must be zero, not NaN/inf");
            assert!(v.is_finite());
        }
        assert!(z.scatter[1].iter().all(|x| *x == 0.0));
        // The visited group is unaffected.
        assert!((z.total[0] - 3.0).abs() < 1e-12);
    }

    /// A bin-count disagreement is an error, not a silent misread.
    #[test]
    fn bin_count_mismatch_is_rejected() {
        let g = two_group();
        let names = vec!["a".to_string(), "b".to_string()];
        let scalar = scalar_with(1, 2, |_, _, _| 1.0); // only 1 zone, 2 declared
        let matrix = matrix_with(2, 2, |_, _, _| 1.0);
        assert!(matches!(
            condense(&g, &names, &scalar, &matrix, 1),
            Err(MgxsError::BinCountMismatch {
                zones: 2,
                groups: 2,
                ..
            })
        ));
    }

    /// Reversing to descending-energy order moves both matrix axes together.
    #[test]
    fn descending_energy_view_reverses_both_matrix_axes() {
        let g = two_group();
        let names = vec!["z".to_string()];
        let scalar = scalar_with(1, 2, |_z, gg, s| if s == 0 { 1.0 } else { gg as f64 });
        // single element at (0 -> 1)
        let matrix = matrix_with(
            1,
            2,
            |_z, gi, go| if (gi, go) == (0, 1) { 9.0 } else { 0.0 },
        );
        let lib = condense(&g, &names, &scalar, &matrix, 1).unwrap();
        let rev = lib.in_descending_energy();
        // ascending total was [0.0, 1.0] -> descending [1.0, 0.0]
        assert_eq!(rev.zones[0].total, vec![1.0, 0.0]);
        // (0,1) in a 2-group set maps to (1,0)
        assert_eq!(rev.zones[0].scatter[1][0], 9.0);
        assert_eq!(rev.zones[0].scatter[0][1], 0.0);
    }
}
