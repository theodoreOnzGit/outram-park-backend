// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Self-shielded (Bondarenko-dilution) multigroup cross-section assembly** —
//! the flux-weighted group average `sigma_g(sigma_0)` produced by combining a
//! background-dilution weighting flux with the URR self-shielded cross sections.
//!
//! # What this computes
//!
//! For each background dilution `sigma_0` and each group `[E_lo, E_hi)`, the
//! self-shielded group cross section is the flux-weighted average
//!
//! ```text
//! sigma_g(sigma_0) = ( integral_g sigma(E; sigma_0) phi(E; sigma_0) dE )
//!                    / ( integral_g phi(E; sigma_0) dE )
//! ```
//!
//! with two dilution-dependent inputs:
//!
//! 1. the **weighting flux** `phi(E; sigma_0)` — the Bondarenko narrow-resonance
//!    flux ([`crate::groupr::unresolved::genflx_bondarenko`]) or, in the
//!    heterogeneous case, the slowing-down flux
//!    ([`crate::groupr::slowing_down::genflx_slowing_down`]);
//! 2. the **self-shielded cross section** `sigma(E; sigma_0)` — the smooth-file
//!    cross section corrected inside the unresolved range by
//!    [`crate::groupr::unresolved::UnresolvedTable::shield`] (when a URR table is
//!    present).
//!
//! This is the `panel`/`displa` group average (already ported for the vector
//! case in [`crate::groupr::panel`]) driven once per dilution with the
//! dilution-shielded flux and cross section — i.e. the `nz > 1` generalization of
//! the vector path (`groupr.f90:5858-6091` with `iz = 1..nsigz`).
//!
//! # Dilution limits (the V&V contract)
//!
//! - `sigma_0 -> inf` (infinite dilution): `phi -> C(E)`, `sigma -> sinf`
//!   (unshielded) — `sigma_g` equals the plain flux-weighted average
//!   ([`crate::groupr::panel::group_average_vector`] under the smooth weight).
//! - `sigma_0 -> 0` (fully self-shielded): `phi -> C(E) sigma_pot / sigma_t(E)`,
//!   the narrow-resonance `1/sigma_t` dip — `sigma_g` is maximally depressed
//!   inside resonances.
//! - Monotonic in between: `sigma_g` decreases as `sigma_0` decreases (more
//!   self-shielding), for a resonance-shaped cross section.
//!
//! # Scope
//!
//! [`self_shielded_group_xs`] is assembled entirely from already-ported parts —
//! [`genflx_bondarenko`] for the per-dilution flux,
//! [`UnresolvedTable::shield`] for the per-dilution shielded cross section (when
//! a table is supplied), and [`group_integral`] for the average. No new physics;
//! this is the *assembly*. Its three dilution-limit properties (infinite-dilution
//! reference, monotonicity, and the fully-shielded floor) are verified with
//! measured numbers in the module tests.

use std::sync::Arc;

use crate::groupr::panel::{group_integral, GroupFlux, PointwiseXs};
use crate::groupr::unresolved::{genflx_bondarenko, UnresolvedTable, UrrReaction};
use crate::NjoyError;

/// A self-shielded multigroup cross section: `sigma_g(sigma_0)` for one reaction
/// over a group structure and a set of background dilutions.
///
/// `sigma[is][g]` is the group cross section \[barn\] at dilution
/// `dilutions[is]` in group `g` (0-based, ascending group order — group 0 is the
/// lowest energy). The infinite-dilution column is the unshielded flux-weighted
/// average; smaller dilutions are progressively self-shielded.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfShieldedMgxs {
    /// Background dilutions `sigma_0` \[barn\], in the order supplied.
    pub dilutions: Vec<f64>,
    /// Group boundaries \[eV\], ascending (length = number of groups + 1).
    pub group_bounds: Vec<f64>,
    /// `sigma[is][g]` — group cross section \[barn\] at dilution `is`, group `g`.
    pub sigma: Vec<Vec<f64>>,
}

impl SelfShieldedMgxs {
    /// Group cross section \[barn\] at dilution index `is`, group `g`; `0.0` for
    /// out-of-range indices.
    pub fn get(&self, is: usize, g: usize) -> f64 {
        self.sigma
            .get(is)
            .and_then(|r| r.get(g))
            .copied()
            .unwrap_or(0.0)
    }

    /// Number of groups (`group_bounds.len() - 1`).
    pub fn n_groups(&self) -> usize {
        self.group_bounds.len().saturating_sub(1)
    }
}

/// Assemble the self-shielded multigroup cross section `sigma_g(sigma_0)` for one
/// reaction — the `nz > 1` (per-dilution) generalization of the ported vector
/// group average.
///
/// For each dilution `sigma_0` in `dilutions` this must:
/// 1. build the weighting flux `phi(E; sigma_0)` on the union energy grid
///    (Bondarenko: [`genflx_bondarenko`] with `sigma_t`, `weight`, `sigma_pot`);
/// 2. build the self-shielded cross section `sigma(E; sigma_0)` — apply
///    [`UnresolvedTable::shield`] to the smooth `sigma_rx` inside the URR range
///    when `urr` is `Some`, else use `sigma_rx` directly;
/// 3. group-average with [`group_integral`] per group and take the ratio.
///
/// # Parameters
/// - `sigma_t` — pointwise **total** cross section \[barn vs eV\] (drives the
///   Bondarenko flux dips).
/// - `sigma_rx` — pointwise cross section of the reaction being shielded
///   \[barn vs eV\] (equals `sigma_t` for the total reaction itself).
/// - `urr` — optional URR self-shielded table for `reaction`; `None` runs the
///   Bondarenko-flux weighting without a URR cross-section correction (still a
///   valid self-shielded average via the flux alone).
/// - `reaction` — which URR reaction column `sigma_rx` corresponds to.
/// - `weight` — the smooth asymptotic weight `C(E)` (any [`GroupFlux`]).
/// - `sigma_pot` — potential-scattering cross section \[barn\].
/// - `dilutions` — background `sigma_0` values \[barn\] (`f64::INFINITY` allowed
///   for the infinite-dilution reference).
/// - `group_bounds` — group boundaries \[eV\], **ascending**, `>= 2` entries.
/// - `energy_grid` — the fine energy grid \[eV\] the flux is tabulated on
///   (ascending; put it on the resonance structure so the flux resolves the
///   dips).
///
/// # Errors
/// [`NjoyError::EndfParse`] if `group_bounds` has fewer than 2 entries or is not
/// strictly ascending, if the energy grid is invalid (propagated from
/// [`genflx_bondarenko`]), or if a URR shielding lookup fails.
///
/// # Verification
/// The three dilution-limit properties are checked in this module's tests with
/// measured numbers (2026-07-15): the infinite-dilution column matches
/// [`crate::groupr::panel::group_average_vector`], each group is strictly
/// increasing in `sigma_0`, and the smallest dilution is the most depressed.
#[allow(clippy::too_many_arguments)]
pub fn self_shielded_group_xs(
    sigma_t: &PointwiseXs,
    sigma_rx: &PointwiseXs,
    urr: Option<&UnresolvedTable>,
    reaction: UrrReaction,
    weight: &GroupFlux,
    sigma_pot: f64,
    dilutions: &[f64],
    group_bounds: &[f64],
    energy_grid: &[f64],
) -> Result<SelfShieldedMgxs, NjoyError> {
    if group_bounds.len() < 2 {
        return Err(NjoyError::EndfParse(
            "self_shielded_group_xs: need >= 2 group bounds".into(),
        ));
    }
    for w in group_bounds.windows(2) {
        if !(w[1] > w[0]) {
            return Err(NjoyError::EndfParse(
                "self_shielded_group_xs: group bounds must be strictly ascending".into(),
            ));
        }
    }

    // (1) Build the per-dilution Bondarenko weighting fluxes on the fine grid.
    // This also validates the energy grid (strictly ascending, >= 2 points).
    // Narrow-resonance branch of `genflx` (groupr.f90:5623-5665).
    let flux_set = genflx_bondarenko(sigma_t, weight, sigma_pot, dilutions, energy_grid)?;

    let n_groups = group_bounds.len() - 1;
    let mut sigma = Vec::with_capacity(dilutions.len());

    // Per-dilution loop — the `iz = 1..nz` outer index `panel` carries
    // (groupr.f90:5858-6091), driven once per background dilution.
    for (is, &s0) in dilutions.iter().enumerate() {
        let phi = &flux_set.fluxes[is];

        // (2) Form the self-shielded cross section sigma(E; sigma_0). With a URR
        // table, correct each grid point inside the URR range via `getunr`
        // (UnresolvedTable::shield, single-dilution call). Without a table, the
        // smooth cross section is weighted by the Bondarenko flux alone.
        let sig_rx_shielded = build_shielded_xs(sigma_rx, urr, reaction, s0, energy_grid)?;

        // (3) Group-average with the panel integrator, then take the ratio — the
        // `panel` accumulate + `displa` divide (groupr.f90:6014-6031, 6093).
        let mut row = Vec::with_capacity(n_groups);
        for g in 0..n_groups {
            let e_lo = group_bounds[g];
            let e_hi = group_bounds[g + 1];
            row.push(group_integral(&sig_rx_shielded, phi, e_lo, e_hi).average());
        }
        sigma.push(row);
    }

    Ok(SelfShieldedMgxs {
        dilutions: dilutions.to_vec(),
        group_bounds: group_bounds.to_vec(),
        sigma,
    })
}

/// Build the self-shielded cross section `sigma(E; sigma_0)` sampled on
/// `energy_grid` for one background dilution.
///
/// With no URR table (`urr = None`) the smooth reaction cross section is returned
/// unchanged — the honest tape-free path where self-shielding enters only through
/// the Bondarenko weighting flux. With a table, each grid point inside the URR
/// range is corrected by [`UnresolvedTable::shield`] (a single-dilution `getunr`
/// call), leaving points outside the range at their smooth-file value.
fn build_shielded_xs(
    sigma_rx: &PointwiseXs,
    urr: Option<&UnresolvedTable>,
    reaction: UrrReaction,
    s0: f64,
    energy_grid: &[f64],
) -> Result<PointwiseXs, NjoyError> {
    match urr {
        None => Ok(sigma_rx.clone()),
        Some(table) => {
            let mut pairs = Vec::with_capacity(energy_grid.len());
            let dilution = [s0];
            for &e in energy_grid {
                let bg = [sigma_rx.value(e)];
                let out = table.shield(reaction, e, &bg, &dilution)?;
                pairs.push((e, out.sig[0]));
            }
            Ok(PointwiseXs::LinLin(Arc::new(pairs)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groupr::panel::group_average_vector;

    /// Build the triangular-resonance total cross section shared by the
    /// dilution-limit tests: 1 barn at the edges of `[0, 100] eV`, rising
    /// linearly to 101 barn at 50 eV, tabulated on the 11-point grid
    /// `E = 0, 10, ..., 100 eV` (the same shape as the `unresolved` module's
    /// `self_shielding_monotone_in_dilution` test).
    fn triangular_sigma_t() -> (PointwiseXs, Vec<f64>) {
        let mut pairs = Vec::new();
        for k in 0..=10 {
            let e = k as f64 * 10.0;
            let peak = 1.0 + 100.0 * (1.0 - ((e - 50.0) / 50.0).abs());
            pairs.push((e, peak));
        }
        let grid: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        (PointwiseXs::LinLin(Arc::new(pairs)), grid)
    }

    /// **Property 1 — infinite-dilution limit reproduces the unshielded vector
    /// average.**
    ///
    /// **Methodology.** For the triangular resonance `sigma_t` (edges 1 barn,
    /// peak 101 barn at 50 eV) with a flat weight `C = 1`, `sigma_pot = 1 barn`
    /// and no URR table (`urr = None`), assemble `sigma_g(sigma_0)` over two
    /// groups `[0, 50)` and `[50, 100)` for
    /// `sigma_0 in {inf, 1e4, 100, 10, 1, 0.1} barn`, taking `sigma_rx = sigma_t`
    /// (the total reaction shields itself). At `sigma_0 = inf` the Bondarenko
    /// flux collapses to the flat weight, so the column must equal
    /// [`group_average_vector`]`(sigma_t, Flat, bounds)` to `< 1e-6` relative.
    /// The hand value of that reference is the trapezoid mean of each triangular
    /// half: `10*(11+31+51+71+91)/50 = 51.0 barn` per group.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** infinite-dilution column =
    /// `[51.0, 51.0] barn`, matching `group_average_vector` to `< 1e-12`
    /// (both groups). The reference vector was `[51.0, 51.0]`.
    #[test]
    fn infinite_dilution_equals_vector_average() {
        let (sigma_t, grid) = triangular_sigma_t();
        let bounds = [0.0, 50.0, 100.0];
        let dilutions = [f64::INFINITY, 1.0e4, 1.0e2, 1.0e1, 1.0, 0.1];

        let mgxs = self_shielded_group_xs(
            &sigma_t,
            &sigma_t,
            None,
            UrrReaction::Total,
            &GroupFlux::Flat,
            1.0,
            &dilutions,
            &bounds,
            &grid,
        )
        .expect("assembly");

        let reference = group_average_vector(&sigma_t, &GroupFlux::Flat, &bounds);
        assert_eq!(reference.len(), 2);
        // The infinite-dilution column is index 0 (dilutions[0] = inf).
        for g in 0..mgxs.n_groups() {
            let got = mgxs.get(0, g);
            let want = reference[g];
            let rel = (got - want).abs() / want.abs().max(1e-30);
            assert!(
                rel < 1e-6,
                "group {g}: inf-dilution {got} != reference {want}"
            );
            // Hand value: 51.0 barn per triangular half.
            assert!((want - 51.0).abs() < 1e-9, "reference group {g} = {want}");
        }
    }

    /// **Property 2 — each group is strictly monotone increasing in `sigma_0`.**
    ///
    /// **Methodology.** Same triangular `sigma_t`, flat weight, `sigma_pot = 1`,
    /// `urr = None`, two groups. For the descending dilution list
    /// `{inf, 1e4, 100, 10, 1, 0.1}` the columns of `sigma_g` must be strictly
    /// *decreasing* (more self-shielding at smaller `sigma_0` depresses the flux
    /// inside the resonance peak, down-weighting the large cross section). Assert
    /// `sigma_g(is) > sigma_g(is+1) + 1e-6` for every adjacent pair, in every
    /// group.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** group 0 `[0,50)` averages \[barn\]
    /// vs `sigma_0 = {inf, 1e4, 100, 10, 1, 0.1}`:
    /// `[51.0, 50.9105, 44.7984, 29.4016, 13.1848, 8.8300]` — strictly
    /// decreasing. Group 1 `[50,100)` is identical to group 0 by the symmetry of
    /// the triangle: `[51.0, 50.9105, 44.7984, 29.4016, 13.1848, 8.8300]`.
    #[test]
    fn group_xs_strictly_increasing_in_dilution() {
        let (sigma_t, grid) = triangular_sigma_t();
        let bounds = [0.0, 50.0, 100.0];
        let dilutions = [f64::INFINITY, 1.0e4, 1.0e2, 1.0e1, 1.0, 0.1];

        let mgxs = self_shielded_group_xs(
            &sigma_t,
            &sigma_t,
            None,
            UrrReaction::Total,
            &GroupFlux::Flat,
            1.0,
            &dilutions,
            &bounds,
            &grid,
        )
        .expect("assembly");

        for g in 0..mgxs.n_groups() {
            for is in 0..dilutions.len() - 1 {
                let hi = mgxs.get(is, g);
                let lo = mgxs.get(is + 1, g);
                assert!(
                    hi > lo + 1e-6,
                    "group {g}: sigma_g not decreasing with dilution index \
                     (is={is}: {hi} vs {lo})",
                );
            }
        }
    }

    /// **Property 3 — the smallest dilution is the fully-shielded floor.**
    ///
    /// **Methodology.** Same setup. The last dilution column (`sigma_0 = 0.1`,
    /// the most self-shielded) must be the minimum over all dilutions in every
    /// group — the narrow-resonance `1/sigma_t` flux dip depresses the group
    /// average maximally there.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** the `sigma_0 = 0.1` column
    /// `[8.8300, 8.8300] barn` is the minimum in both groups; the infinite
    /// column `[51.0, 51.0] barn` is the maximum. Floor/ceiling ratio ~ 0.173.
    #[test]
    fn fully_shielded_floor_is_the_minimum() {
        let (sigma_t, grid) = triangular_sigma_t();
        let bounds = [0.0, 50.0, 100.0];
        let dilutions = [f64::INFINITY, 1.0e4, 1.0e2, 1.0e1, 1.0, 0.1];
        let last = dilutions.len() - 1;

        let mgxs = self_shielded_group_xs(
            &sigma_t,
            &sigma_t,
            None,
            UrrReaction::Total,
            &GroupFlux::Flat,
            1.0,
            &dilutions,
            &bounds,
            &grid,
        )
        .expect("assembly");

        for g in 0..mgxs.n_groups() {
            let floor = mgxs.get(last, g);
            for is in 0..last {
                assert!(
                    mgxs.get(is, g) >= floor,
                    "group {g}: dilution {is} ({}) below the floor {floor}",
                    mgxs.get(is, g),
                );
            }
        }
    }

    /// **URR path — a stored self-shielding table depresses the group average
    /// below its flux-only value.**
    ///
    /// **Methodology.** Use the additive (`lssf = 0`) URR table shape from the
    /// `unresolved` module (dilution grid `[1e10, 100, 10, 1]`, energies
    /// 10/20/30 eV, cross section growing with energy and with self-shielding).
    /// Take the smooth cross section equal to the table's infinite-dilution (first)
    /// column, `sigma_t = sigma_rx`, and assemble the total reaction over one group
    /// `[10, 30) eV` with the Bondarenko flux (`sigma_pot = 5 barn`) on the URR
    /// energy grid, at `sigma_0 = 1 barn`, once with `urr = Some(table)` and once
    /// with `urr = None`. In both cases the Bondarenko flux already self-shields
    /// via the `1/sigma_t` dip; the `urr = Some` case additionally corrects the
    /// cross section itself downward with the stored table, so its group average
    /// must fall below the flux-only value; both must be finite and positive.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** with `urr = None` (flux-only
    /// self-shielding) the group average was 17.3146 barn; adding the URR table
    /// dropped it to 6.9258 barn — the stored table depresses the cross section
    /// further on top of the flux dip, as expected.
    #[test]
    fn urr_table_shields_below_flux_only() {
        use crate::endf::interp::IntLaw;
        use crate::groupr::unresolved::{LssfFlag, UnresolvedTable, UrrEnergyPoint};

        // Smooth cross section = the infinite-dilution (first) column of the table:
        // 10 barn @10 eV, 20 @20, 30 @30 — so urr=None gives the unshielded level.
        let smooth = PointwiseXs::LinLin(Arc::new(vec![(10.0, 10.0), (20.0, 20.0), (30.0, 30.0)]));
        let sigma0 = vec![1.0e10, 100.0, 10.0, 1.0];
        let points = vec![
            UrrEnergyPoint {
                energy_ev: 10.0,
                overlap: false,
                xs: vec![vec![10.0, 9.0, 7.0, 4.0]],
            },
            UrrEnergyPoint {
                energy_ev: 20.0,
                overlap: false,
                xs: vec![vec![20.0, 18.0, 14.0, 8.0]],
            },
            UrrEnergyPoint {
                energy_ev: 30.0,
                overlap: false,
                xs: vec![vec![30.0, 27.0, 21.0, 12.0]],
            },
        ];
        let table =
            UnresolvedTable::store(300.0, LssfFlag::Additive, IntLaw::LinLin, sigma0, points)
                .expect("store");

        let grid = [10.0, 20.0, 30.0];
        let bounds = [10.0, 30.0];
        let dilutions = [1.0];

        let with_urr = self_shielded_group_xs(
            &smooth,
            &smooth,
            Some(&table),
            UrrReaction::Total,
            &GroupFlux::Flat,
            5.0,
            &dilutions,
            &bounds,
            &grid,
        )
        .expect("assembly urr");
        let without = self_shielded_group_xs(
            &smooth,
            &smooth,
            None,
            UrrReaction::Total,
            &GroupFlux::Flat,
            5.0,
            &dilutions,
            &bounds,
            &grid,
        )
        .expect("assembly none");

        let shielded = with_urr.get(0, 0);
        let flux_only = without.get(0, 0);
        assert!(shielded > 0.0 && flux_only > 0.0);
        assert!(
            shielded < flux_only,
            "URR shielding should depress the average: urr={shielded} vs none={flux_only}",
        );
    }
}
