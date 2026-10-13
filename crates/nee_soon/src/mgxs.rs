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
//! | scalar | `[Material, Energy]` | `Flux, Total, Absorption, NuFission, KappaFission, Fission` |
//! | matrix | `[Material, Energy, EnergyOut]` | `ScatterN, NuFission` (births, for `chi`) |
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
//! ## Collapsing to a coarser structure
//!
//! [`MgxsLibrary::collapse`] merges groups of a library onto any structure
//! whose edges are a subset of its own. Every constant here is a ratio of
//! rates to flux, and both sum over the merged groups, so collapsing the
//! means is **exact** (reaction rates are conserved to rounding), not an
//! approximation. A run tallied once on a fine structure therefore yields
//! every nested coarse structure without another transport pass
//! ([`crate::htr10_mgxs`] relies on this). What it does NOT carry over is the
//! statistical uncertainty: the tally keeps per-bin moments only, not the
//! covariance between bins of one batch, so the uncertainty of a merged group
//! is not recoverable from the fine one.
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
use serde::{Deserialize, Serialize};

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
    /// A collapse target has an edge that is not an edge of the library being
    /// collapsed, so a fine group would have to be split between two coarse
    /// ones. Splitting needs the spectrum inside the group, which the library
    /// does not carry; refusing is the only exact answer.
    #[error("cannot collapse: coarse edge {edge_ev} eV is not an edge of the fine structure")]
    NotNested {
        /// The offending coarse edge \[eV\].
        edge_ev: f64,
    },
    /// [`MgxsLibrary::to_physics_mg`]: a group's scattering row disagrees with
    /// `Sigma_t - Sigma_a` by more than its statistics allow. The two come
    /// from separate estimators, so this is a condensation defect (or a
    /// library assembled wrongly), not noise.
    #[error(
        "zone '{zone}' group {group} (ascending): Sigma_t - Sigma_a = {expected:.6e} but the \
         scattering row sums to {row:.6e}; the difference {residual:.3e} exceeds the \
         allowed {allowed:.3e} cm^-1"
    )]
    Inconsistent {
        /// Zone name.
        zone: String,
        /// Group index in the library's own ascending order.
        group: usize,
        /// `Sigma_t - Sigma_a` \[cm^-1\].
        expected: f64,
        /// The scattering row sum \[cm^-1\].
        row: f64,
        /// `row - expected` \[cm^-1\].
        residual: f64,
        /// The bound it was held to \[cm^-1\].
        allowed: f64,
    },
    /// [`MgxsLibrary::to_physics_mg`]: the library carries no standard errors
    /// ([`ZoneMgxs::sigma`] is `None`) and is not exactly balanced, so a
    /// statistical disagreement cannot be told from a defect.
    #[error(
        "zone '{zone}' has no standard errors and its balance is off by {relative:.3e} \
         (relative) in group {group}: cannot tell noise from a defect. Condense with \
         `condense` (which estimates them) or convert `rebalanced()` as an explicit ablation"
    )]
    NoUncertainty {
        /// Zone name.
        zone: String,
        /// The first group found off balance (ascending order).
        group: usize,
        /// Its relative residual.
        relative: f64,
    },
    /// [`MgxsLibrary::to_physics_mg`]: an ordering the multigroup kernel
    /// requires does not hold (`Sigma_f <= Sigma_a <= Sigma_t`, a fissile zone
    /// with a birth spectrum, matching group counts).
    #[error("zone '{zone}': {what}")]
    NotTransportable {
        /// Zone name.
        zone: String,
        /// What failed.
        what: String,
    },
}

/// An energy group structure, as ascending bin edges in eV.
///
/// `n + 1` edges define `n` groups, and **group 0 is the lowest-energy group**
/// — see the module note on ordering.
///
/// Serialises as the plain edge list; deserialising re-runs [`Self::new`], so
/// a file cannot smuggle in edges that do not ascend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<f64>", into = "Vec<f64>")]
pub struct GroupStructure {
    edges: Vec<f64>,
}

impl TryFrom<Vec<f64>> for GroupStructure {
    type Error = MgxsError;

    fn try_from(edges: Vec<f64>) -> Result<Self, MgxsError> {
        Self::new(edges)
    }
}

impl From<GroupStructure> for Vec<f64> {
    fn from(g: GroupStructure) -> Self {
        g.edges
    }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Fission `Sigma_f` \[cm^-1\] (added 2026-10-12, gh:#848). A multigroup
    /// transport code needs it beside `nu * Sigma_f`: it is what `nu-bar`
    /// and the capture/fission split at a collision are formed from
    /// (`outram_mc_libs::physics::physics_mg::Mgxs::new`).
    pub fission: Vec<f64>,
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
    /// Standard errors of the constants above, per group, as [`condense`]
    /// estimated them (added 2026-10-13, gh:#848). `None` when they are not
    /// known: a library built by hand, one condensed from fewer than two
    /// active batches, one derived by [`MgxsLibrary::collapse`],
    /// [`MgxsLibrary::homogenised`] or [`MgxsLibrary::rebalanced`] (none of
    /// which can carry them, see [`ZoneMgxsSigma`]), and every record written
    /// before the field existed, which deserialise with `None`.
    #[serde(default)]
    pub sigma: Option<ZoneMgxsSigma>,
}

/// One standard error per condensed constant of a [`ZoneMgxs`], with the
/// same shapes and the same ascending group order.
///
/// # How they are estimated, and what that misses
///
/// Every constant is a ratio `R = X / phi` of two tally means over the same
/// batches. [`condense`] takes each mean's standard error from its own bin
/// ([`TallyBin::rel_std_dev`], the standard error of the batch mean) and
/// combines the two in quadrature:
///
/// ```text
///   sigma_R = |R| * sqrt( (sigma_X / X)^2 + (sigma_phi / phi)^2 )
/// ```
///
/// That drops the covariance term `-2 rho (sigma_X/X)(sigma_phi/phi)`. The
/// numerator and the flux are scored on the same tracks in the same batches,
/// so `rho > 0` and the dropped term would make `sigma_R` **smaller**: these
/// are upper estimates. The tally keeps per-bin moments only, not the
/// covariance between bins, which is also why no `chi` uncertainty is given
/// (each `chi_g` is a sum of bins of one batch) and why a group merged by
/// [`MgxsLibrary::collapse`] has none.
///
/// `flux` is the flux's own absolute standard error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneMgxsSigma {
    /// Standard error of [`ZoneMgxs::flux`].
    pub flux: Vec<f64>,
    /// Standard error of [`ZoneMgxs::total`] \[cm^-1\].
    pub total: Vec<f64>,
    /// Standard error of [`ZoneMgxs::absorption`] \[cm^-1\].
    pub absorption: Vec<f64>,
    /// Standard error of [`ZoneMgxs::fission`] \[cm^-1\].
    pub fission: Vec<f64>,
    /// Standard error of [`ZoneMgxs::nu_fission`] \[cm^-1\].
    pub nu_fission: Vec<f64>,
    /// Standard error of [`ZoneMgxs::kappa_fission`] \[J cm^-1\].
    pub kappa_fission: Vec<f64>,
    /// Standard error of each [`ZoneMgxs::scatter`] element \[cm^-1\].
    pub scatter: Vec<Vec<f64>>,
}

impl ZoneMgxsSigma {
    /// An upper bound on the standard error of row `g`'s sum,
    /// `sum_g' Sigma_s,g->g'`: the plain sum of the element errors. The
    /// elements of a row come from one batch, so they are correlated and the
    /// quadrature sum is not safe; the plain sum is the worst case
    /// (correlation 1), whatever the correlation is.
    #[must_use]
    pub fn scatter_row_bound(&self, g: usize) -> Option<f64> {
        Some(self.scatter.get(g)?.iter().sum())
    }

    fn reversed(&self) -> Self {
        let rev = |v: &Vec<f64>| -> Vec<f64> { v.iter().rev().copied().collect() };
        let n = self.flux.len();
        let mut scatter = vec![vec![0.0; n]; n];
        for gi in 0..n {
            for go in 0..n {
                scatter[n - 1 - gi][n - 1 - go] = self.scatter[gi][go];
            }
        }
        Self {
            flux: rev(&self.flux),
            total: rev(&self.total),
            absorption: rev(&self.absorption),
            fission: rev(&self.fission),
            nu_fission: rev(&self.nu_fission),
            kappa_fission: rev(&self.kappa_fission),
            scatter,
        }
    }
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

    /// Diffusion length `L = sqrt(D / Sigma_a)` \[cm\] for group `g`.
    ///
    /// The distance a neutron of that group diffuses before absorption — the
    /// length that governs how thick a reflector has to be before extra
    /// thickness stops mattering. Returns `None` for a group with no
    /// absorption, where `L` is unbounded.
    #[must_use]
    pub fn diffusion_length(&self, g: usize) -> Option<f64> {
        let d = self.diffusion_coefficient(g)?;
        let a = *self.absorption.get(g)?;
        if a > 0.0 {
            Some((d / a).sqrt())
        } else {
            None
        }
    }

    /// Absorption as the **GeN-Foam bridge infers it**, `Sigma_t,g - sum_g'
    /// Sigma_s,g->g'` \[cm^-1\] — NOT the tallied [`Self::absorption`].
    ///
    /// # Why this exists
    ///
    /// [`crate::genfoam_xs`] never reads the tallied `absorption` field. It
    /// passes `sigma_removal = Sigma_t - Sigma_s,g->g` plus the full scattering
    /// matrix, so whatever absorption the solver sees is whatever is left over
    /// after the out-scatter is subtracted — this quantity.
    ///
    /// **This is a difference of two large, independently tallied numbers.** In
    /// graphite `Sigma_t ~ 0.4 cm^-1` and the scatter row sum is also
    /// `~0.4 cm^-1`, while the true absorption is `~3e-4 cm^-1`. A balance
    /// residual of only 0.4 % between them is therefore about **five times the
    /// absorption itself**, and it lands entirely in the solver's absorption
    /// term. A condensation can look well-balanced and still be useless here.
    ///
    /// Compare against `absorption[g]` to see what the solver is actually
    /// absorbing with; [`MgxsLibrary::k_inf_inferred_absorption`] turns that
    /// comparison into an eigenvalue.
    ///
    /// Returns `None` if `g` is out of range.
    #[must_use]
    pub fn inferred_absorption(&self, g: usize) -> Option<f64> {
        let total = *self.total.get(g)?;
        let row: f64 = self.scatter.get(g)?.iter().sum();
        Some(total - row)
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MgxsLibrary {
    /// The group structure every zone is condensed onto.
    pub groups: GroupStructure,
    /// Per-zone group constants, in the order the zones were supplied.
    pub zones: Vec<ZoneMgxs>,
}

impl MgxsLibrary {
    /// Collapse every zone into ONE flux-weighted zone — the whole system as a
    /// single homogeneous medium.
    ///
    /// # Why this exists
    ///
    /// [`condense`] produces one zone per *material*, because that is what a
    /// `MaterialFilter` tallies. A **whole-system** quantity such as `k_inf`
    /// is not recoverable from any single one of those zones: the kernel's
    /// `k_inf` is not the core's. To compare a deterministic `k_inf` against
    /// the Monte Carlo system `k_inf` (production over absorption, summed over
    /// everything), the zones must first be homogenised into the same single
    /// medium the Monte Carlo number describes.
    ///
    /// # The weighting, and why chi is different
    ///
    /// Reaction cross sections are **flux-weighted per group**, which is the
    /// definition that conserves reaction rate:
    ///
    /// ```text
    ///   phi_g      = sum_z phi_g,z
    ///   Sigma_x,g  = sum_z ( Sigma_x,g,z * phi_g,z ) / phi_g
    /// ```
    ///
    /// The scattering matrix is weighted the same way on its **incoming**
    /// group, since `scatter[g][g']` multiplies `phi_g`.
    ///
    /// **`chi` is not a cross section and must not be flux-weighted.** It is a
    /// probability distribution over emission energy, so it is weighted by the
    /// fission SOURCE each zone actually produces,
    /// `S_z = sum_g nu_Sigma_f,g,z * phi_g,z`, and the result is renormalised
    /// to sum to one. Flux-weighting it would let a high-flux zone that
    /// produces no fission dictate the spectrum of one that does.
    ///
    /// # Zero flux
    ///
    /// A group no neutron visited has `phi_g = 0` in every zone. Its cross
    /// sections are left at zero rather than divided by zero — the caller is
    /// expected to reject such a library (the GeN-Foam bridge does), because a
    /// group with no measured cross section cannot be solved on.
    #[must_use]
    pub fn homogenised(&self, name: impl Into<String>) -> MgxsLibrary {
        let all: Vec<usize> = (0..self.zones.len()).collect();
        self.homogenised_subset(&all, name)
    }

    /// Homogenise only the zones named by `which`, leaving the rest out.
    ///
    /// # Why a subset, and not just everything
    ///
    /// [`Self::homogenised`] collapses the WHOLE library into one medium, which
    /// is wrong for a reflected core: it smears the reflector graphite and the
    /// boronated carbon bricks into the fuel, where they become a pure
    /// parasitic absorber with no fission and no spatial separation. In the
    /// real core those regions are separate, and neutrons that leak into them
    /// come BACK. Measured on the HTR-10 at 2 groups, collapsing everything
    /// into one zone put the deterministic eigenvalue about 27 000 pcm below
    /// the Monte Carlo one, after leakage (450 pcm), condensation balance
    /// (0.4 %) and the solver (SP3 = diffusion to 0 pcm) had each been excluded
    /// as the cause.
    ///
    /// Building one homogenised medium per PHYSICAL REGION — bed, reflector —
    /// and giving each its own cells on a mesh is what lets a deterministic
    /// model represent reflection at all.
    ///
    /// Indices out of range are ignored rather than panicking; a subset that
    /// selects nothing yields a zone of zeros, which the bridge will reject.
    #[must_use]
    pub fn homogenised_subset(&self, which: &[usize], name: impl Into<String>) -> MgxsLibrary {
        let g = self.zones.first().map_or(0, |z| z.flux.len());
        let mut out = ZoneMgxs {
            name: name.into(),
            flux: vec![0.0; g],
            total: vec![0.0; g],
            absorption: vec![0.0; g],
            fission: vec![0.0; g],
            nu_fission: vec![0.0; g],
            kappa_fission: vec![0.0; g],
            scatter: vec![vec![0.0; g]; g],
            chi: vec![0.0; g],
            // Not carried: a flux-weighted sum over zones needs the
            // covariance between them (see `ZoneMgxsSigma`).
            sigma: None,
        };

        let picked = || which.iter().filter_map(|&i| self.zones.get(i));

        // Flux-weighted sums: accumulate RATES, divide by total flux at the end.
        for z in picked() {
            for gi in 0..g {
                let w = z.flux[gi];
                out.flux[gi] += w;
                out.total[gi] += z.total[gi] * w;
                out.absorption[gi] += z.absorption[gi] * w;
                out.fission[gi] += z.fission[gi] * w;
                out.nu_fission[gi] += z.nu_fission[gi] * w;
                out.kappa_fission[gi] += z.kappa_fission[gi] * w;
                for gj in 0..g {
                    out.scatter[gi][gj] += z.scatter[gi][gj] * w;
                }
            }
        }
        for gi in 0..g {
            let phi = out.flux[gi];
            if phi > 0.0 {
                out.total[gi] /= phi;
                out.absorption[gi] /= phi;
                out.fission[gi] /= phi;
                out.nu_fission[gi] /= phi;
                out.kappa_fission[gi] /= phi;
                for gj in 0..g {
                    out.scatter[gi][gj] /= phi;
                }
            }
        }

        // chi: weighted by each zone's own fission SOURCE, then renormalised.
        let mut src_total = 0.0;
        for z in picked() {
            let s: f64 = (0..g).map(|gi| z.nu_fission[gi] * z.flux[gi]).sum();
            if s <= 0.0 {
                continue;
            }
            src_total += s;
            for gi in 0..g {
                out.chi[gi] += z.chi[gi] * s;
            }
        }
        if src_total > 0.0 {
            let norm: f64 = out.chi.iter().sum();
            if norm > 0.0 {
                for c in &mut out.chi {
                    *c /= norm;
                }
            }
        }

        MgxsLibrary {
            groups: self.groups.clone(),
            zones: vec![out],
        }
    }

    /// Add a **measured** buckling leakage `D_g * B^2` to every group's
    /// absorption, so a zero-gradient deterministic solve carries the same
    /// leakage the Monte Carlo run measured.
    ///
    /// # Why this is necessary, and why it is not a fit
    ///
    /// Group constants condensed from a full-core Monte Carlo run carry a
    /// **leakage-hardened** spectrum. Solving them with zero leakage asks the
    /// solver to re-derive an *infinite-medium* spectrum instead, which is a
    /// different problem — measured at `-25578 pcm` on this model, and it is a
    /// mismatch of questions rather than a defect in either code.
    ///
    /// Adding `D_g B^2` as an extra removal term makes the deterministic
    /// spectrum leakage-hardened too, so it matches the spectrum the constants
    /// were condensed under, and `k_eff` becomes comparable to `k_eff`.
    ///
    /// **`B^2` is MEASURED, not tuned.** The caller obtains it from the Monte
    /// Carlo run's own leakage via [`Self::buckling_from_leakage`]; it is never
    /// searched for a value that makes the eigenvalues agree. Searching it
    /// would convert the comparison into a fit and destroy the check — the same
    /// rule the workspace applies to any calibrated parameter.
    ///
    /// `D_g = 1 / (3 Sigma_t,g)`, the standard diffusion coefficient. This uses
    /// the TOTAL cross section rather than the transport cross section because
    /// the condensation is `P0` and carries no `P1` scattering moment, so
    /// `Sigma_tr` is not available. That approximation is `mu_bar = 0`; for
    /// graphite `mu_bar = 2/(3A) = 0.056`, so `D` is understated by about 6 %
    /// and the leakage with it. Stated rather than hidden, and it is a
    /// candidate term in the ablation.
    /// # Which field this modifies, and why it is NOT `absorption`
    ///
    /// The GeN-Foam bridge builds `sigma_removal` as `Sigma_t - Sigma_s,g->g`
    /// and the diffusion coefficient from `Sigma_t`. **It never reads the
    /// `absorption` field.** An earlier version of this method added the
    /// leakage to `absorption` and the deterministic eigenvalue came back
    /// BIT-IDENTICAL — which is how the mistake was caught, and is the same
    /// signature as a boundary condition that changes nothing.
    ///
    /// The leakage is therefore added to `total`, which is what reaches the
    /// solver through `sigma_removal`. `absorption` is updated by the same
    /// amount so [`Self::k_inf`] stays consistent with what the solver sees;
    /// the two must not drift apart, since comparing them is the whole
    /// fault-isolation scheme.
    ///
    /// `D` is evaluated from the ORIGINAL `Sigma_t`, so the added removal is
    /// exactly `D_0 B^2`. The resulting small change in `D` itself is second
    /// order and is not compensated.
    #[must_use]
    pub fn with_buckling(&self, b2: f64) -> MgxsLibrary {
        let mut out = self.clone();
        for z in &mut out.zones {
            for g in 0..z.flux.len() {
                let sigma_t = z.total[g];
                if sigma_t > 0.0 {
                    let leak = b2 / (3.0 * sigma_t);
                    z.total[g] += leak;
                    z.absorption[g] += leak;
                }
            }
        }
        out
    }

    /// Per-group check that `Sigma_t` agrees with `Sigma_a + sum_g' Sigma_s,g->g'`.
    ///
    /// # Why this matters more than it looks
    ///
    /// The GeN-Foam bridge builds removal as `Sigma_t - Sigma_s,g->g`, which is
    /// only the correct removal if the total and the scattering matrix are
    /// mutually consistent — i.e. if `Sigma_t = Sigma_a + Sigma_s,total`. Those
    /// two come from SEPARATE Monte Carlo tallies (a scalar pass and a matrix
    /// pass), so nothing enforces the identity.
    ///
    /// If the matrix pass undercounts scattering, removal is computed too large
    /// and the eigenvalue comes out too LOW, while `Sigma_a` and hence
    /// [`Self::k_inf`] are unaffected — producing exactly the signature of a
    /// solver that disagrees with the algebraic `k_inf` of its own input.
    ///
    /// Returns, per zone and group, `(Sigma_t, Sigma_a + Sigma_s,row, relative
    /// discrepancy)`. A relative discrepancy far from zero is a CONDENSATION
    /// defect, not a solver one.
    #[must_use]
    pub fn balance_check(&self) -> Vec<(String, Vec<(f64, f64, f64)>)> {
        self.zones
            .iter()
            .map(|z| {
                let rows = (0..z.flux.len())
                    .map(|g| {
                        let s_row: f64 = z.scatter[g].iter().sum();
                        let rebuilt = z.absorption[g] + s_row;
                        let rel = if z.total[g] > 0.0 {
                            (rebuilt - z.total[g]) / z.total[g]
                        } else {
                            0.0
                        };
                        (z.total[g], rebuilt, rel)
                    })
                    .collect();
                (z.name.clone(), rows)
            })
            .collect()
    }

    /// Solve for the buckling that reproduces a **measured** leakage fraction.
    ///
    /// `leak_fraction` is neutrons lost per neutron produced, taken from the
    /// Monte Carlo run (`leak_vacuum / histories`, or the six-factor leakage
    /// sum). The buckling that reproduces it satisfies
    ///
    /// ```text
    ///   leak_rate = sum_g D_g B^2 phi_g  =  B^2 * sum_g phi_g / (3 Sigma_t,g)
    /// ```
    ///
    /// so `B^2 = leak_rate / sum_g ( phi_g / (3 Sigma_t,g) )`, with `leak_rate`
    /// put on the same per-absorption footing as the tallies by scaling with
    /// the total absorption rate.
    ///
    /// Returns `None` when the flux is empty or the denominator vanishes,
    /// rather than returning a buckling of zero that would silently mean "no
    /// leakage".
    #[must_use]
    pub fn buckling_from_leakage(&self, leak_fraction: f64) -> Option<f64> {
        if !(leak_fraction > 0.0) {
            return None;
        }
        let mut absn = 0.0;
        let mut denom = 0.0;
        for z in &self.zones {
            for g in 0..z.flux.len() {
                absn += z.absorption[g] * z.flux[g];
                if z.total[g] > 0.0 {
                    denom += z.flux[g] / (3.0 * z.total[g]);
                }
            }
        }
        if denom <= 0.0 || absn <= 0.0 {
            return None;
        }
        // leak_fraction is per absorption+leakage; convert to a rate on the
        // same scale as `absn`, then solve for B^2.
        let leak_rate = absn * leak_fraction / (1.0 - leak_fraction).max(1.0e-12);
        Some(leak_rate / denom)
    }

    /// Infinite-multiplication factor implied by these constants, zero leakage.
    ///
    /// `k_inf = sum_g nu_Sigma_f,g phi_g / sum_g Sigma_a,g phi_g`, over every
    /// zone. This is the algebraic answer the group constants encode, and is
    /// the thing a zero-leakage deterministic solve must reproduce — so a
    /// disagreement between this and the diffusion solver is a SOLVER defect,
    /// while a disagreement between this and the Monte Carlo system `k_inf` is
    /// a CONDENSATION defect. Separating those two is the point.
    #[must_use]
    pub fn k_inf(&self) -> f64 {
        let mut prod = 0.0;
        let mut absn = 0.0;
        for z in &self.zones {
            for gi in 0..z.flux.len() {
                prod += z.nu_fission[gi] * z.flux[gi];
                absn += z.absorption[gi] * z.flux[gi];
            }
        }
        if absn > 0.0 {
            prod / absn
        } else {
            0.0
        }
    }

    /// The same library with every group's total rebalanced to
    /// `Sigma_t,g = Sigma_a,g + sum_g' Sigma_s,g->g'`.
    ///
    /// # Why this is the right repair, and why on the total
    ///
    /// [`crate::genfoam_xs`] hands the solver `sigma_removal` and the
    /// scattering matrix, never the tallied absorption, so the solver's
    /// effective absorption is `Sigma_t - Sigma_s,row`
    /// ([`ZoneMgxs::inferred_absorption`]). A condensed Monte Carlo tally never
    /// balances exactly, and that residual therefore lands **entirely** in the
    /// absorption term — the single most eigenvalue-sensitive number in the set,
    /// and in a graphite-reflected core one that is ~1000x smaller than the two
    /// quantities being differenced.
    ///
    /// Enforcing the balance by moving `Sigma_t` puts the residual where it does
    /// almost nothing instead. `Sigma_t` enters only
    /// [`ZoneMgxs::diffusion_coefficient`] (`D = 1/(3 Sigma_t)`) and the
    /// removal, both of which shift by the residual's own relative size — a few
    /// tenths of a percent — while the absorption becomes exactly the tallied
    /// value.
    ///
    /// **This is a rebalancing, not a fit.** No parameter is tuned against any
    /// reference: the tallied `Sigma_a` and the tallied scattering matrix are
    /// both kept exactly as measured, and `Sigma_t` is set to the sum they imply.
    /// The quantity discarded is the inconsistency between the three, which has
    /// no physical content — a true cross section set satisfies this identity by
    /// definition.
    ///
    /// Compare [`Self::k_inf`] against [`Self::k_inf_inferred_absorption`] on
    /// the rebalanced library: they are equal by construction, which is the
    /// check that this worked.
    #[must_use]
    pub fn rebalanced(&self) -> Self {
        let zones = self
            .zones
            .iter()
            .map(|z| {
                let mut out = z.clone();
                for g in 0..out.total.len() {
                    let row: f64 = out.scatter.get(g).map_or(0.0, |r| r.iter().sum());
                    let abs = out.absorption.get(g).copied().unwrap_or(0.0);
                    out.total[g] = abs + row;
                }
                // The rebalanced total is a sum of correlated constants, so
                // the tallied standard errors no longer describe it.
                out.sigma = None;
                out
            })
            .collect();
        Self {
            zones,
            ..self.clone()
        }
    }

    /// Reflector savings `delta` \[cm\] from one-group diffusion theory.
    ///
    /// ```text
    /// delta = (D_core / D_refl) * L_refl * tanh(t_refl / L_refl)
    /// ```
    ///
    /// # What it is for
    ///
    /// A reflected core leaks far less than its bare dimensions imply. The
    /// saving is the distance by which the reflector effectively extends the
    /// core, so a transverse buckling can be written
    /// `B^2 = (2.405 / (R + delta))^2` for a cylinder without meshing the
    /// reflector in that direction.
    ///
    /// **Every input comes from the cross sections themselves** — `D` and `L`
    /// of the two zones, and the reflector thickness from geometry. Nothing is
    /// taken from the reference eigenvalue, which is what separates this from
    /// [`Self::buckling_from_leakage`]: that one reads the Monte Carlo run's
    /// own measured leakage and therefore cannot be used to *test* the leakage
    /// model, only to bypass it.
    ///
    /// # Limits
    ///
    /// One-group, so it uses the thermal group's `D` and `L` and ignores fast
    /// leakage entirely. `tanh` saturates: beyond about `2 L` of reflector the
    /// saving stops growing, which is the same physics that made a 310 cm
    /// graphite reflector worth only ~86 pcm more than a 100 cm one.
    ///
    /// Returns `None` if either zone index is out of range, or the reflector
    /// group has no finite diffusion length.
    #[must_use]
    pub fn reflector_savings(
        &self,
        core_zone: usize,
        refl_zone: usize,
        thermal_group: usize,
        refl_thickness_cm: f64,
    ) -> Option<f64> {
        let core = self.zones.get(core_zone)?;
        let refl = self.zones.get(refl_zone)?;
        let d_core = core.diffusion_coefficient(thermal_group)?;
        let d_refl = refl.diffusion_coefficient(thermal_group)?;
        let l_refl = refl.diffusion_length(thermal_group)?;
        if d_refl <= 0.0 || l_refl <= 0.0 {
            return None;
        }
        Some((d_core / d_refl) * l_refl * (refl_thickness_cm / l_refl).tanh())
    }

    /// `k_inf` recomputed with the absorption the **bridge infers** rather than
    /// the absorption that was tallied — see [`ZoneMgxs::inferred_absorption`].
    ///
    /// # Interpreting the two
    ///
    /// [`Self::k_inf`] is the algebraic eigenvalue of the tallied data and is
    /// what a zero-leakage solve *should* reproduce exactly. This function is
    /// what a zero-leakage solve will *actually* reproduce, because it is the
    /// absorption the GeN-Foam bridge hands the solver.
    ///
    /// **A large split between them is a condensation defect, not a solver
    /// defect**, and it localises the fault precisely: the transport solve is
    /// doing the right arithmetic on the wrong absorption.
    #[must_use]
    pub fn k_inf_inferred_absorption(&self) -> f64 {
        let mut prod = 0.0;
        let mut absn = 0.0;
        for z in &self.zones {
            for gi in 0..z.flux.len() {
                prod += z.nu_fission[gi] * z.flux[gi];
                absn += z.inferred_absorption(gi).unwrap_or(0.0) * z.flux[gi];
            }
        }
        if absn > 0.0 {
            prod / absn
        } else {
            0.0
        }
    }

    /// The same library on the coarser structure `coarse`, whose edges must be
    /// a subset of this library's.
    ///
    /// # Why it is exact
    ///
    /// Every constant is a group reaction rate over the group flux, and both
    /// are sums over the energy range, so for a coarse group `G` made of fine
    /// groups `g`:
    ///
    /// ```text
    ///   phi_G          = sum_g phi_g
    ///   Sigma_x,G      = sum_g Sigma_x,g phi_g / phi_G
    ///   Sigma_s,G->G'  = sum_{g in G} sum_{g' in G'} Sigma_s,g->g' phi_g / phi_G
    ///   chi_G'         = sum_{g' in G'} chi_g'
    /// ```
    ///
    /// which is exactly what tallying on `G` directly would have produced,
    /// history for history. `chi` is summed, not flux-weighted, because it is
    /// a distribution over birth energy. A coarse group whose fine groups all
    /// carry zero flux stays at zero, as in [`condense`].
    ///
    /// Reaction rates per zone are therefore conserved to rounding, and with
    /// them [`Self::k_inf`]: a collapse that changes `k_inf` beyond rounding
    /// is a defect in the collapse.
    ///
    /// # Errors
    ///
    /// [`MgxsError::NotNested`] if a coarse edge is not a fine edge (matched to
    /// a relative 1e-9), or if `coarse` does not span the same range.
    pub fn collapse(&self, coarse: &GroupStructure) -> Result<MgxsLibrary, MgxsError> {
        let fine = self.groups.edges();
        let same = |a: f64, b: f64| (a - b).abs() <= 1.0e-9 * a.abs().max(b.abs());
        // Fine index of every coarse edge.
        let mut at = Vec::with_capacity(coarse.edges().len());
        for &e in coarse.edges() {
            match fine.iter().position(|&f| same(f, e)) {
                Some(i) => at.push(i),
                None => return Err(MgxsError::NotNested { edge_ev: e }),
            }
        }
        if at[0] != 0 {
            return Err(MgxsError::NotNested { edge_ev: coarse.edges()[0] });
        }
        if at[at.len() - 1] != fine.len() - 1 {
            return Err(MgxsError::NotNested {
                edge_ev: coarse.edges()[coarse.edges().len() - 1],
            });
        }
        let n_c = coarse.n_groups();
        // Coarse group of each fine group.
        let mut of = vec![0usize; self.groups.n_groups()];
        for c in 0..n_c {
            for slot in of.iter_mut().take(at[c + 1]).skip(at[c]) {
                *slot = c;
            }
        }
        let zones = self
            .zones
            .iter()
            .map(|z| {
                let mut out = ZoneMgxs {
                    name: z.name.clone(),
                    flux: vec![0.0; n_c],
                    total: vec![0.0; n_c],
                    absorption: vec![0.0; n_c],
                    fission: vec![0.0; n_c],
                    nu_fission: vec![0.0; n_c],
                    kappa_fission: vec![0.0; n_c],
                    scatter: vec![vec![0.0; n_c]; n_c],
                    chi: vec![0.0; n_c],
                    // Not recoverable: merging groups needs the covariance
                    // between the fine bins (module docs, `ZoneMgxsSigma`).
                    sigma: None,
                };
                // Accumulate rates, then divide by the merged flux.
                for (g, &c) in of.iter().enumerate() {
                    let w = z.flux[g];
                    out.flux[c] += w;
                    out.total[c] += z.total[g] * w;
                    out.absorption[c] += z.absorption[g] * w;
                    out.fission[c] += z.fission[g] * w;
                    out.nu_fission[c] += z.nu_fission[g] * w;
                    out.kappa_fission[c] += z.kappa_fission[g] * w;
                    out.chi[c] += z.chi[g];
                    for (go, &co) in of.iter().enumerate() {
                        out.scatter[c][co] += z.scatter[g][go] * w;
                    }
                }
                for c in 0..n_c {
                    let phi = out.flux[c];
                    if phi > 0.0 {
                        out.total[c] /= phi;
                        out.absorption[c] /= phi;
                        out.fission[c] /= phi;
                        out.nu_fission[c] /= phi;
                        out.kappa_fission[c] /= phi;
                        for v in &mut out.scatter[c] {
                            *v /= phi;
                        }
                    } else {
                        out.total[c] = 0.0;
                        out.absorption[c] = 0.0;
                        out.fission[c] = 0.0;
                        out.nu_fission[c] = 0.0;
                        out.kappa_fission[c] = 0.0;
                        out.scatter[c] = vec![0.0; n_c];
                    }
                }
                out
            })
            .collect();
        Ok(MgxsLibrary {
            groups: coarse.clone(),
            zones,
        })
    }

    /// Convert to the multigroup transport kernel's library,
    /// [`outram_mc_libs::physics::physics_mg::MgxsLibrary`], refusing a library
    /// the kernel would transport wrongly.
    ///
    /// # What changes on the way
    ///
    /// - **Group order is reversed**: `physics_mg` has group 0 fastest, this
    ///   module group 0 slowest ([`Self::in_descending_energy`]).
    /// - **The scattering matrix is flattened row-major**,
    ///   `scatter[g * G + g'] = Sigma_s,g->g'` in the reversed order, which is
    ///   `physics_mg::Mgxs`'s layout.
    /// - `Sigma_t`, `Sigma_a`, `Sigma_f`, `nu Sigma_f` and `chi` pass through
    ///   unchanged; nothing is rebalanced. A non-fissile zone keeps its
    ///   all-zero `chi` (allowed by `Mgxs::new` since gh:#848).
    ///
    /// **Zone `i` becomes material index `i`.** The kernel looks a material
    /// up by the index its geometry carries, so the library must hold one zone
    /// per material, in material-index order, which is what [`condense`]
    /// produces when its tallies list every material in order.
    ///
    /// # The consistency it requires
    ///
    /// The kernel scatters with probability `(Sigma_t - Sigma_a)/Sigma_t` and
    /// takes only the SHAPE of the scattering row, so a row that does not sum
    /// to `Sigma_t - Sigma_a` is not used as tallied. The two come from
    /// separate estimators (track length and analog events), so they agree
    /// only statistically. In every group with flux, the residual
    /// `row - (Sigma_t - Sigma_a)` must lie within [`CONSISTENCY_Z`] times
    /// the bound `sigma_t + sigma_a + sum_g' sigma_s,g->g'` built from
    /// [`ZoneMgxs::sigma`] (an upper bound: see [`ZoneMgxsSigma`]), or within
    /// [`CONSISTENCY_REL_FLOOR`] of `Sigma_t`.
    ///
    /// The matrix [`condense`] produces is a **nu**-scatter matrix (every
    /// neutron an (n,2n) emits is scored), so its row may exceed
    /// `Sigma_t - Sigma_a` by the (n,xn) gain. This converter does not yet
    /// carry that gain into the kernel (gh:#682), so here it counts as an
    /// inconsistency like any other.
    ///
    /// A library with no standard errors (`sigma: None`: one built by hand,
    /// collapsed, homogenised, or recorded before 2026-10-13) is converted
    /// only if every group balances to [`CONSISTENCY_REL_FLOOR`]. Real tallies
    /// never do, so such a library is refused; [`Self::rebalanced`] is the
    /// explicit, visible ablation that makes it convertible.
    ///
    /// # Errors
    ///
    /// [`MgxsError::Inconsistent`], [`MgxsError::NoUncertainty`], or
    /// [`MgxsError::NotTransportable`] (`Sigma_f > Sigma_a`,
    /// `Sigma_a > Sigma_t`, a fissile zone with no birth spectrum, or an
    /// empty library).
    pub fn to_physics_mg(
        &self,
    ) -> Result<outram_mc_libs::physics::physics_mg::MgxsLibrary, MgxsError> {
        use outram_mc_libs::physics::physics_mg as mg;
        if self.zones.is_empty() {
            return Err(MgxsError::NotTransportable {
                zone: String::new(),
                what: "the library has no zones".into(),
            });
        }
        let n = self.groups.n_groups();
        for z in &self.zones {
            check_transportable(z, n)?;
        }
        let desc = self.in_descending_energy();
        let materials = desc
            .zones
            .iter()
            .map(|z| {
                let flat: Vec<f64> = z.scatter.iter().flat_map(|row| row.iter().copied()).collect();
                mg::Mgxs::new(
                    z.name.clone(),
                    z.total.clone(),
                    z.absorption.clone(),
                    z.fission.clone(),
                    z.nu_fission.clone(),
                    z.chi.clone(),
                    flat,
                )
            })
            .collect();
        Ok(mg::MgxsLibrary::new(materials))
    }

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
                    fission: rev(&z.fission),
                    nu_fission: rev(&z.nu_fission),
                    kappa_fission: rev(&z.kappa_fission),
                    scatter,
                    chi: rev(&z.chi),
                    sigma: z.sigma.as_ref().map(ZoneMgxsSigma::reversed),
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

/// How many standard errors a scattering row may differ from
/// `Sigma_t - Sigma_a` before [`MgxsLibrary::to_physics_mg`] refuses it.
///
/// Fixed at 4 before any library was converted. The bound it multiplies is
/// itself an upper bound on the standard error (see [`ZoneMgxsSigma`]), so a
/// genuine 4-sigma excursion is rarer than the nominal 6e-5 per group.
pub const CONSISTENCY_Z: f64 = 4.0;

/// The relative balance a library must hold in every group when it carries
/// no standard errors, and the floor under the statistical bound when it
/// does: rounding, not statistics.
pub const CONSISTENCY_REL_FLOOR: f64 = 1.0e-9;

/// The checks [`MgxsLibrary::to_physics_mg`] makes on one zone, in the
/// library's own ascending order.
fn check_transportable(z: &ZoneMgxs, n: usize) -> Result<(), MgxsError> {
    let bad = |what: String| MgxsError::NotTransportable {
        zone: z.name.clone(),
        what,
    };
    let lens = [
        z.flux.len(),
        z.total.len(),
        z.absorption.len(),
        z.fission.len(),
        z.nu_fission.len(),
        z.chi.len(),
        z.scatter.len(),
    ];
    if lens.iter().any(|&l| l != n) || z.scatter.iter().any(|r| r.len() != n) {
        return Err(bad(format!("vectors do not all have {n} groups")));
    }
    // The kernel's own ordering invariants, checked here so a bad library is
    // an error rather than a panic in `physics_mg::Mgxs::new` (whose slack is
    // an absolute 1e-12).
    for g in 0..n {
        if z.fission[g] > z.absorption[g] + 1e-12 {
            return Err(bad(format!(
                "group {g}: Sigma_f {:.6e} exceeds Sigma_a {:.6e}",
                z.fission[g], z.absorption[g]
            )));
        }
        if z.absorption[g] > z.total[g] + 1e-12 {
            return Err(bad(format!(
                "group {g}: Sigma_a {:.6e} exceeds Sigma_t {:.6e}",
                z.absorption[g], z.total[g]
            )));
        }
    }
    if z.nu_fission.iter().any(|&v| v > 0.0) && !(z.chi.iter().sum::<f64>() > 0.0) {
        return Err(bad("fissile, but no fission neutron was born in it (chi = 0)".into()));
    }
    for g in 0..n {
        if !(z.flux[g] > 0.0) {
            continue;
        }
        let expected = z.total[g] - z.absorption[g];
        let row: f64 = z.scatter[g].iter().sum();
        let residual = row - expected;
        let floor = CONSISTENCY_REL_FLOOR * z.total[g].abs();
        match &z.sigma {
            Some(s) => {
                let bound = s.total[g] + s.absorption[g] + s.scatter_row_bound(g).unwrap_or(0.0);
                let allowed = (CONSISTENCY_Z * bound).max(floor);
                if residual.abs() > allowed {
                    return Err(MgxsError::Inconsistent {
                        zone: z.name.clone(),
                        group: g,
                        expected,
                        row,
                        residual,
                        allowed,
                    });
                }
            }
            None => {
                if residual.abs() > floor {
                    return Err(MgxsError::NoUncertainty {
                        zone: z.name.clone(),
                        group: g,
                        relative: residual.abs() / z.total[g].abs().max(f64::MIN_POSITIVE),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Scores carried by the matrix pass, in order: the scattering matrix and the
/// fission-birth spectrum. Both need the same `[Material, Energy, EnergyOut]`
/// filters, so they share one tally and one transport pass.
pub const MATRIX_SCORES: usize = 2;

/// The score order the scalar pass must declare, and that [`condense`] assumes.
///
/// Exposed so a caller building the tally cannot silently disagree with the
/// reader about which score sits at which offset. `Fission` was appended last
/// (2026-10-12, gh:#848) so the offsets of the first five, which
/// `dhoby-ghaut`'s mesh tally also reads, did not move.
pub const SCALAR_SCORES: [ScoreType; 6] = [
    ScoreType::Flux,
    ScoreType::Total,
    ScoreType::Absorption,
    ScoreType::NuFission,
    ScoreType::KappaFission,
    ScoreType::Fission,
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
    // Relative standard error of a bin's mean, 0 for a bin never scored
    // (a zero mean has no relative error to speak of, and its ratio is 0).
    let rel = |bin: &TallyBin| -> f64 {
        if bin.sum == 0.0 {
            0.0
        } else {
            bin.rel_std_dev(n_realizations)
        }
    };
    let scal_rel = |z: usize, g: usize, s: usize| rel(&scalar.bins[(z * n_g + g) * n_s + s]);
    let mat_rel = |z: usize, gi: usize, go: usize, s: usize| {
        rel(&matrix.bins[((z * n_g + gi) * n_g + go) * MATRIX_SCORES + s])
    };
    // Standard errors need at least two batches; with fewer, none are given
    // rather than infinities (which a JSON record cannot hold).
    let with_sigma = n_realizations >= 2;
    // sigma of R = X/phi, X and phi uncorrelated (an upper estimate, see
    // `ZoneMgxsSigma`).
    let ratio_sigma = |r: f64, rel_x: f64, rel_phi: f64| -> f64 {
        r.abs() * (rel_x * rel_x + rel_phi * rel_phi).sqrt()
    };

    let mut zones = Vec::with_capacity(n_z);
    for (z, name) in zone_names.iter().enumerate() {
        let mut flux = vec![0.0; n_g];
        let mut total = vec![0.0; n_g];
        let mut absorption = vec![0.0; n_g];
        let mut fission = vec![0.0; n_g];
        let mut nu_fission = vec![0.0; n_g];
        let mut kappa_fission = vec![0.0; n_g];
        let mut scatter = vec![vec![0.0; n_g]; n_g];
        let mut sig = ZoneMgxsSigma {
            flux: vec![0.0; n_g],
            total: vec![0.0; n_g],
            absorption: vec![0.0; n_g],
            fission: vec![0.0; n_g],
            nu_fission: vec![0.0; n_g],
            kappa_fission: vec![0.0; n_g],
            scatter: vec![vec![0.0; n_g]; n_g],
        };

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
            fission[g] = scal(z, g, 5) / phi;
            for go in 0..n_g {
                scatter[g][go] = mat(z, g, go, 0) / phi;
            }
            if with_sigma {
                let rp = scal_rel(z, g, 0);
                sig.flux[g] = phi * rp;
                sig.total[g] = ratio_sigma(total[g], scal_rel(z, g, 1), rp);
                sig.absorption[g] = ratio_sigma(absorption[g], scal_rel(z, g, 2), rp);
                sig.nu_fission[g] = ratio_sigma(nu_fission[g], scal_rel(z, g, 3), rp);
                sig.kappa_fission[g] = ratio_sigma(kappa_fission[g], scal_rel(z, g, 4), rp);
                sig.fission[g] = ratio_sigma(fission[g], scal_rel(z, g, 5), rp);
                for go in 0..n_g {
                    sig.scatter[g][go] = ratio_sigma(scatter[g][go], mat_rel(z, g, go, 0), rp);
                }
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
            fission,
            nu_fission,
            kappa_fission,
            scatter,
            chi,
            sigma: with_sigma.then_some(sig),
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
                // score index 5 = Fission, appended 2026-10-12 (gh:#848).
                assert!((zone.fission[gg] - 5.0).abs() < 1e-12);
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

    /// Rebalancing makes the solver's absorption equal the tallied absorption.
    ///
    /// # Methodology
    ///
    /// A two-group zone is built deliberately out of balance: the totals are
    /// 3 % above what `Sigma_a + Sigma_s,row` implies, so the absorption
    /// [`crate::genfoam_xs`] would infer is far from the tallied one. The
    /// library is then passed through [`MgxsLibrary::rebalanced`].
    ///
    /// # Result
    ///
    /// After rebalancing, `inferred_absorption(g)` equals `absorption[g]` to
    /// 1e-12 in every group, and the two eigenvalues
    /// [`MgxsLibrary::k_inf`] and [`MgxsLibrary::k_inf_inferred_absorption`]
    /// agree to 1e-12 — they are equal by construction, which is the property
    /// that makes a zero-leakage solve able to reproduce the exact answer.
    ///
    /// The tallied absorption and the full scattering matrix are asserted
    /// UNCHANGED, because the repair must move `Sigma_t` and nothing else; a
    /// version that quietly adjusted `Sigma_a` to force the balance would pass
    /// the eigenvalue check while destroying the measurement.
    #[test]
    fn rebalancing_moves_the_residual_off_the_absorption() {
        let lib = MgxsLibrary {
            groups: GroupStructure::new(vec![1.0e-5, 1.0, 2.0e7]).expect("edges ascend"),
            zones: vec![ZoneMgxs {
                name: "out-of-balance".into(),
                flux: vec![1.0, 2.0],
                // 3 % high against absorption + scatter row.
                total: vec![(0.01 + 0.50) * 1.03, (0.02 + 0.80) * 1.03],
                absorption: vec![0.01, 0.02],
                fission: vec![0.006, 0.012],
                nu_fission: vec![0.015, 0.030],
                kappa_fission: vec![0.0, 0.0],
                scatter: vec![vec![0.40, 0.10], vec![0.05, 0.75]],
                chi: vec![1.0, 0.0],
                sigma: None,
            }],
        };

        let before = lib.zones[0].inferred_absorption(0).unwrap();
        assert!(
            (before - 0.01).abs() > 1.0e-3,
            "the fixture must start genuinely out of balance, got {before:e}"
        );

        let fixed = lib.rebalanced();
        for g in 0..2 {
            let inferred = fixed.zones[0].inferred_absorption(g).unwrap();
            let tallied = fixed.zones[0].absorption[g];
            assert!(
                (inferred - tallied).abs() < 1.0e-12,
                "group {g}: inferred {inferred:e} != tallied {tallied:e}"
            );
        }

        assert!(
            (fixed.k_inf() - fixed.k_inf_inferred_absorption()).abs() < 1.0e-12,
            "after rebalancing the two eigenvalues must coincide: {} vs {}",
            fixed.k_inf(),
            fixed.k_inf_inferred_absorption()
        );

        // The measurement itself must be untouched.
        assert_eq!(fixed.zones[0].absorption, lib.zones[0].absorption);
        assert_eq!(fixed.zones[0].scatter, lib.zones[0].scatter);
        assert_eq!(fixed.zones[0].nu_fission, lib.zones[0].nu_fission);
    }

    /// A 0.4 % condensation imbalance becomes a ~5x absorption error.
    ///
    /// # Methodology
    ///
    /// Two graphite-like groups are built with the scattering-dominated
    /// numbers a thermal reflector actually has: `Sigma_t = 0.4 cm^-1`, true
    /// `Sigma_a = 3.0e-4 cm^-1`. The scatter row is then given a deliberate
    /// **0.4 % deficit** against the total — the size of residual a condensed
    /// Monte Carlo tally routinely carries, and the size this example's own
    /// balance check reported and I wrongly dismissed as negligible.
    ///
    /// # Result
    ///
    /// The absorption [`crate::genfoam_xs`] infers is `1.6e-3 cm^-1` against a
    /// true `3.0e-4 cm^-1` — **5.33x too high**, because it is a difference of
    /// two numbers each ~1300x larger than the answer. The pass criterion is
    /// that the inferred value exceed the true one by more than 4x, i.e. that
    /// a residual everyone would call small does NOT stay small here.
    ///
    /// This is why a balance check quoted as a *relative* residual cannot
    /// clear the condensation: 0.4 % of `Sigma_t` is 533 % of `Sigma_a`.
    #[test]
    fn a_small_balance_residual_is_a_large_absorption_error() {
        let sigma_t = 0.4_f64;
        let true_abs = 3.0e-4_f64;
        // Scatter row 0.4 % short of the total.
        let row = sigma_t * 0.996;
        let z = ZoneMgxs {
            name: "graphite-like".into(),
            flux: vec![1.0],
            total: vec![sigma_t],
            absorption: vec![true_abs],
            fission: vec![0.0],
            nu_fission: vec![0.0],
            kappa_fission: vec![0.0],
            scatter: vec![vec![row]],
            chi: vec![0.0],
            sigma: None,
        };

        let inferred = z.inferred_absorption(0).expect("group 0 exists");
        assert!(
            (inferred - 1.6e-3).abs() < 1.0e-9,
            "inferred absorption should be the 0.4 % deficit, got {inferred:e}"
        );
        assert!(
            inferred > 4.0 * true_abs,
            "a 0.4 % balance residual must show up as a LARGE absorption error; \
             inferred {inferred:e} vs true {true_abs:e}"
        );
    }

    /// Removal subtracts within-group scattering; the diffusion coefficient is
    /// 1/(3 Sigma_t).
    ///
    /// This test carried no `#[test]` until 2026-10-12 (its attribute had
    /// slid onto the rebalancing test above, which then had two), so it had
    /// never run. Restored, it passes unchanged.
    #[test]
    fn removal_and_diffusion_follow_their_definitions() {
        let z = ZoneMgxs {
            name: "z".into(),
            flux: vec![1.0, 1.0],
            total: vec![2.0, 4.0],
            absorption: vec![0.5, 0.5],
            fission: vec![0.0, 0.0],
            nu_fission: vec![0.0, 0.0],
            kappa_fission: vec![0.0, 0.0],
            scatter: vec![vec![0.75, 0.25], vec![0.1, 1.5]],
            chi: vec![0.6, 0.4],
            sigma: None,
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

    /// A three-group fixture with distinct numbers everywhere, so a collapse
    /// that reads the wrong group shows up as a different number.
    fn three_group_lib() -> MgxsLibrary {
        let zone = |name: &str, k: f64| ZoneMgxs {
            name: name.into(),
            flux: vec![2.0 * k, 3.0 * k, 5.0],
            total: vec![0.9, 0.7, 0.5],
            absorption: vec![0.30, 0.05, 0.02],
            fission: vec![0.10, 0.01, 0.004],
            nu_fission: vec![0.25, 0.025, 0.011],
            kappa_fission: vec![3.0e-12, 3.1e-13, 1.3e-13],
            scatter: vec![
                vec![0.40, 0.15, 0.05],
                vec![0.02, 0.50, 0.13],
                vec![0.0, 0.01, 0.47],
            ],
            chi: vec![0.0, 0.2, 0.8],
            sigma: None,
        };
        MgxsLibrary {
            groups: GroupStructure::new(vec![1.0e-5, 0.625, 1.0e3, 2.0e7]).expect("ascending"),
            zones: vec![zone("fuel", 1.0), zone("graphite", 7.0)],
        }
    }

    /// Per-zone reaction rates `sum_g Sigma_x,g phi_g` for every reaction.
    fn rates(z: &ZoneMgxs) -> [f64; 7] {
        let r = |x: &[f64]| x.iter().zip(&z.flux).map(|(a, b)| a * b).sum::<f64>();
        let s: f64 = (0..z.flux.len())
            .map(|g| z.scatter[g].iter().sum::<f64>() * z.flux[g])
            .sum();
        [
            z.flux.iter().sum(),
            r(&z.total),
            r(&z.absorption),
            r(&z.fission),
            r(&z.nu_fission),
            r(&z.kappa_fission),
            s,
        ]
    }

    /// Collapsing is exact: every zone's reaction rates and `k_inf` survive
    /// to rounding, and the merged constants are the flux-weighted means.
    ///
    /// # Methodology
    ///
    /// A two-zone, three-group library (edges 1e-5, 0.625, 1e3, 2e7 eV) with
    /// distinct numbers in every slot is collapsed onto two groups (the
    /// 1e3 eV edge removed) and onto one. For each target: the seven
    /// reaction-rate totals of each zone (flux, total, absorption, fission,
    /// nu-fission, kappa-fission, scatter) must match the fine library to a
    /// relative 1e-12, `k_inf` must match to 1e-12, and one merged constant
    /// is checked against hand arithmetic.
    ///
    /// # Result (2026-10-12)
    ///
    /// Passes. The hand-checked value: fuel, merged fast group of fine groups
    /// 1 and 2 (flux 3 and 5), `Sigma_t = (0.7*3 + 0.5*5)/8 = 0.575`, and the
    /// fast-to-fast scatter `(0.50+0.13)*3 + (0.01+0.47)*5) / 8 = 0.53625`.
    #[test]
    fn collapse_conserves_rates_and_k_inf() {
        let fine = three_group_lib();
        for edges in [vec![1.0e-5, 0.625, 2.0e7], vec![1.0e-5, 2.0e7]] {
            let coarse = fine
                .collapse(&GroupStructure::new(edges.clone()).expect("ascending"))
                .expect("nested");
            for (zf, zc) in fine.zones.iter().zip(&coarse.zones) {
                for (a, b) in rates(zf).iter().zip(rates(zc).iter()) {
                    assert!((a - b).abs() <= 1e-12 * a.abs().max(1.0), "{edges:?}: {a} vs {b}");
                }
                let chi: f64 = zc.chi.iter().sum();
                assert!((chi - 1.0).abs() < 1e-12, "chi must still sum to 1");
            }
            assert!((fine.k_inf() - coarse.k_inf()).abs() < 1e-12);
        }
        let two = fine
            .collapse(&GroupStructure::new(vec![1.0e-5, 0.625, 2.0e7]).expect("ascending"))
            .expect("nested");
        let f = &two.zones[0];
        assert!((f.total[1] - 0.575).abs() < 1e-12);
        assert!((f.scatter[1][1] - 0.53625).abs() < 1e-12);
        assert!((f.scatter[0][1] - 0.20).abs() < 1e-12, "group 0 is unmerged");
        assert!((f.chi[1] - 1.0).abs() < 1e-12);
    }

    /// A coarse edge that is not a fine edge, or a range that does not match,
    /// is refused rather than approximated.
    #[test]
    fn collapse_refuses_a_structure_that_does_not_nest() {
        let fine = three_group_lib();
        for edges in [
            vec![1.0e-5, 1.0, 2.0e7],
            vec![1.0e-4, 0.625, 2.0e7],
            vec![1.0e-5, 0.625, 1.0e7],
        ] {
            let coarse = GroupStructure::new(edges).expect("ascending");
            assert!(matches!(
                fine.collapse(&coarse),
                Err(MgxsError::NotNested { .. })
            ));
        }
    }

    /// A library survives a JSON round trip unchanged, and a file whose edges
    /// do not ascend is refused on the way in.
    #[test]
    fn library_round_trips_through_json_and_validates_edges() {
        let lib = three_group_lib();
        let text = serde_json::to_string(&lib).expect("serialise");
        let back: MgxsLibrary = serde_json::from_str(&text).expect("deserialise");
        assert_eq!(back, lib);
        let mut v: serde_json::Value = serde_json::from_str(&text).expect("json");
        v["groups"] = serde_json::json!([1.0e-5, 1.0e3, 0.625, 2.0e7]);
        assert!(serde_json::from_value::<MgxsLibrary>(v).is_err());
    }

    /// `to_physics_mg` reverses the group order and flattens the matrix in
    /// the reversed order, with nothing else changed.
    ///
    /// # Methodology
    ///
    /// The three-group fixture (balanced in every group, distinct numbers in
    /// every slot) is converted, and each `physics_mg` vector is compared
    /// with the fixture read backwards; the flattened matrix is compared
    /// element by element against `scatter[G-1-g][G-1-g']`, which a
    /// transposed or unreversed layout fails (the matrix is not symmetric).
    ///
    /// # Result (2026-10-13)
    ///
    /// Passes exactly (no tolerance).
    #[test]
    fn to_physics_mg_reverses_groups_and_flattens_row_major() {
        let lib = three_group_lib();
        let mg = lib.to_physics_mg().expect("balanced fixture converts");
        let n = 3;
        assert_eq!(mg.n_groups(), n);
        for (i, z) in lib.zones.iter().enumerate() {
            let m = mg.material(i);
            assert_eq!(m.name, z.name, "zone i is material i");
            let back = |v: &Vec<f64>| v.iter().rev().copied().collect::<Vec<_>>();
            assert_eq!(m.total, back(&z.total));
            assert_eq!(m.absorption, back(&z.absorption));
            assert_eq!(m.fission, back(&z.fission));
            assert_eq!(m.nu_fission, back(&z.nu_fission));
            assert_eq!(m.chi, back(&z.chi));
            for g in 0..n {
                for gp in 0..n {
                    assert_eq!(
                        m.scatter[g * n + gp],
                        z.scatter[n - 1 - g][n - 1 - gp],
                        "transfer {g}->{gp} (descending)"
                    );
                }
            }
        }
        // The fastest group is 0 in physics_mg: fuel's 20 MeV group has the
        // fixture's smallest total.
        assert_eq!(mg.material(0).total[0], 0.5);
    }

    /// A library with no standard errors converts only if it balances, so a
    /// real tally condensed before `sigma` existed is refused, and
    /// `rebalanced()` is the visible way through.
    #[test]
    fn a_library_without_sigma_must_balance_exactly() {
        let mut lib = three_group_lib();
        lib.zones[1].total[2] += 1.0e-3;
        match lib.to_physics_mg() {
            Err(MgxsError::NoUncertainty { zone, group, .. }) => {
                assert_eq!((zone.as_str(), group), ("graphite", 2));
            }
            other => panic!("expected NoUncertainty, got {other:?}"),
        }
        assert!(lib.rebalanced().to_physics_mg().is_ok());
    }

    /// With standard errors the residual is judged against them: inside
    /// `CONSISTENCY_Z` times the bound it converts, outside it is refused.
    ///
    /// The bound for fuel group 2 is `sigma_t + sigma_a + sum sigma_s`
    /// `= 1e-4 + 1e-4 + 3 * 1e-4 = 5e-4`, so 4 sigma allows `2e-3`. A
    /// residual of `1.9e-3` converts; `2.1e-3` does not.
    #[test]
    fn residual_is_judged_against_the_standard_errors() {
        let with = |extra: f64| {
            let mut lib = three_group_lib();
            for z in &mut lib.zones {
                z.sigma = Some(ZoneMgxsSigma {
                    flux: vec![0.0; 3],
                    total: vec![1.0e-4; 3],
                    absorption: vec![1.0e-4; 3],
                    fission: vec![0.0; 3],
                    nu_fission: vec![0.0; 3],
                    kappa_fission: vec![0.0; 3],
                    scatter: vec![vec![1.0e-4; 3]; 3],
                });
            }
            lib.zones[0].scatter[2][2] += extra;
            lib
        };
        assert!(with(1.9e-3).to_physics_mg().is_ok());
        match with(2.1e-3).to_physics_mg() {
            Err(MgxsError::Inconsistent {
                zone,
                group,
                residual,
                allowed,
                ..
            }) => {
                assert_eq!((zone.as_str(), group), ("fuel", 2));
                assert!((residual - 2.1e-3).abs() < 1e-12);
                assert!((allowed - 2.0e-3).abs() < 1e-12);
            }
            other => panic!("expected Inconsistent, got {other:?}"),
        }
    }

    /// A non-fissile zone with an all-zero chi converts (graphite, helium);
    /// a fissile one with no birth spectrum is refused rather than handed to
    /// a kernel that would panic.
    #[test]
    fn non_fissile_zero_chi_converts_but_fissile_zero_chi_does_not() {
        let mut lib = three_group_lib();
        let g = &mut lib.zones[1];
        g.fission = vec![0.0; 3];
        g.nu_fission = vec![0.0; 3];
        g.kappa_fission = vec![0.0; 3];
        g.chi = vec![0.0; 3];
        let mg = lib.to_physics_mg().expect("non-fissile graphite converts");
        assert!(!mg.material(1).is_fissile());
        lib.zones[0].chi = vec![0.0; 3];
        assert!(matches!(
            lib.to_physics_mg(),
            Err(MgxsError::NotTransportable { .. })
        ));
    }

    /// `condense` estimates each constant's standard error from its bins.
    ///
    /// # Methodology
    ///
    /// One zone, two groups, two realizations. Every flux bin is scored 1
    /// then 3 (mean 2, sample variance 2, standard error of the mean 1, so
    /// relative 0.5). Every other scalar bin and every matrix bin is scored
    /// 1 then 1 (relative error 0). So each ratio `R = X / phi = 0.5` must
    /// carry `sigma_R = 0.5 * sqrt(0 + 0.5^2) = 0.25`, and the flux its own
    /// `sigma = 1`. A record without the field deserialises with `None`.
    ///
    /// # Result (2026-10-13)
    ///
    /// Passes to 1e-15.
    #[test]
    fn condense_estimates_standard_errors_from_the_bins() {
        let (n_z, n_g) = (1, 2);
        let n_s = SCALAR_SCORES.len();
        let mut sb = vec![TallyBin::default(); n_z * n_g * n_s];
        for g in 0..n_g {
            for s_i in 0..n_s {
                let (a, b) = if s_i == 0 { (1.0, 3.0) } else { (1.0, 1.0) };
                sb[g * n_s + s_i].score(a);
                sb[g * n_s + s_i].score(b);
            }
        }
        let mut mb = vec![TallyBin::default(); n_z * n_g * n_g * MATRIX_SCORES];
        for gi in 0..n_g {
            for go in 0..n_g {
                mb[(gi * n_g + go) * MATRIX_SCORES].score(1.0);
                mb[(gi * n_g + go) * MATRIX_SCORES].score(1.0);
            }
        }
        let scalar = Tally {
            id: 1,
            name: "scalar".into(),
            filters: vec![],
            scores: SCALAR_SCORES.to_vec(),
            bins: sb,
        };
        let matrix = Tally {
            id: 2,
            name: "matrix".into(),
            filters: vec![],
            scores: vec![ScoreType::ScatterN, ScoreType::NuFission],
            bins: mb,
        };
        let lib = condense(&two_group(), &["z".into()], &scalar, &matrix, 2).expect("condense");
        let z = &lib.zones[0];
        let sig = z.sigma.as_ref().expect("two realizations give standard errors");
        for g in 0..n_g {
            assert!((z.total[g] - 0.5).abs() < 1e-15);
            assert!((sig.flux[g] - 1.0).abs() < 1e-15);
            for v in [sig.total[g], sig.absorption[g], sig.fission[g], sig.nu_fission[g]] {
                assert!((v - 0.25).abs() < 1e-15, "{v}");
            }
            for go in 0..n_g {
                assert!((sig.scatter[g][go] - 0.25).abs() < 1e-15);
            }
        }
        // One realization: no standard errors, not infinities.
        let one = condense(&two_group(), &["z".into()], &scalar, &matrix, 1).expect("condense");
        assert!(one.zones[0].sigma.is_none());
        // A record written before the field existed still loads.
        let mut v = serde_json::to_value(&lib).expect("json");
        v["zones"][0].as_object_mut().expect("zone").remove("sigma");
        let old: MgxsLibrary = serde_json::from_value(v).expect("old record loads");
        assert!(old.zones[0].sigma.is_none());
    }
}
