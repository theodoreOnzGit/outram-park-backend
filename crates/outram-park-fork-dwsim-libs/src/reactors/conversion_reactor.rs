//! Fixed-conversion reactor — port of DWSIM `Reactors/Conversion.vb`.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Conversion.vb` (commit
//! `1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
//! per-compound mole-flow update mirrors the delta-mole-flow loop at lines
//! 622–702: `Δnᵢ = −X · νᵢ / ν_BC · n_BC`, where `X` is the specified fractional
//! conversion of the base reactant, `ν` the stoichiometric coefficients, and
//! `n_BC` the base reactant's inlet molar flow. Rank grouping follows `InitVars`
//! (lines 81–117) and the parallel-group objective lines 532–652 (pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`).
//!
//! ## Model
//!
//! Each [`ReactionKind::Conversion`](crate::reactions::ReactionKind::Conversion)
//! reaction imposes a fixed fractional conversion `X ∈ [0, 1]` of its base
//! reactant. There is no rate and no equilibrium — the extent follows directly
//! from `X`:
//!
//! `ζ_r = −X_r · n_BC / ν_BC`   (extent as written, `[mol/s]`)
//!
//! and `Δnᵢ = νᵢ · ζ_r`.
//!
//! ~~Reactions are applied in list order (DWSIM's sequential-group treatment);
//! each reactant flow is clamped at zero so a later reaction cannot drive a
//! compound negative.~~ **CORRECTED 2026-10-02** — upstream has no single
//! "sequential" treatment: it groups reactions by **rank**, and equal ranks run
//! in *parallel*. Measured against compiled upstream
//! (`tests/upstream_conversion_parity.rs`), the list-order version gave methane
//! outflow 0.35 mol/s where upstream gives 0.20 for two same-rank reactions. The
//! ranks are now ported:
//!
//! ### Ranks: parallel groups and sequential groups (upstream semantics)
//!
//! Upstream groups the reactions of a reaction set by **rank**
//! (`Conversion.vb:81-117`, `InitVars`). Groups run in ascending rank, each on
//! the previous group's outlet. Reactions that share a rank form a **parallel
//! group**, in which every reaction's `n_BC` is the base reactant's flow at the
//! *group inlet* (`Conversion.vb:596-621`), not after its siblings have run.
//! This port mirrors that through [`ConversionReactor::ranks`]:
//! [`ConversionReactor::new`] gives each reaction its own rank in list order
//! (fully sequential, the behaviour this module always had), and
//! [`ConversionReactor::with_ranks`] sets them as in a DWSIM reaction set.
//!
//! **Over-specified groups.** If a group's specified conversions would drive a
//! compound negative, upstream minimises `Σ_r (X_r,spec − X_r)²` over
//! `0 ≤ X_r ≤ X_r,spec` with a `10¹⁰`-weighted penalty on negative flows, by
//! Nelder–Mead simplex (`Conversion.vb:532-652`). This port solves the *same*
//! objective exactly: the Euclidean projection of the specified conversions
//! onto `{0 ≤ X ≤ X_spec} ∩ {nᵢ + Δnᵢ(X) ≥ 0 ∀ i}`, by Dykstra's alternating
//! projections. Two differences from upstream follow, both measured in
//! `tests/upstream_conversion_parity.rs`:
//!
//! - upstream's simplex stops short of that optimum (the penalty makes the
//!   objective non-smooth at the constraint): `0.600712 / 0.399288` where the
//!   optimum is `0.6 / 0.4`;
//! - upstream checks negativity only for the compounds of the **last** reaction
//!   in the group (`nif` is reset per reaction, `Conversion.vb:599-635`), so a
//!   reactant appearing only in an earlier reaction can go negative. It is then
//!   clamped (`Conversion.vb:715`) and the outlet rebuilt from the conserved
//!   mass flow, which loses 25 % of the carbon on the test's case. This port
//!   constrains every compound.
//!
//! For a single-reaction group the projection reduces to capping `X` at the
//! limiting reactant, which is what this module did before ranks were added.
//!
//! **Not ported:** upstream takes `n_BC` from the reaction's *phase*
//! (`ReactionPhase` liquid / vapour / mixture / solid) of the flashed inlet.
//! [`ReactorFeed`] carries no phase split, so this port always uses the overall
//! flow — upstream's `Mixture` case. A liquid-phase conversion reaction in a
//! two-phase feed therefore converts more here than upstream (+67 % isobutane
//! on the test's n-butane case).
//!
//! ⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

use crate::reactions::Reaction;

use super::{ReactorError, ReactorFeed, ReactorOutcome};

/// A fixed-conversion reactor: conversion reactions applied in rank groups
/// (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionReactor {
    /// The reactions to apply; each should be
    /// [`ReactionKind::Conversion`](crate::reactions::ReactionKind::Conversion)
    /// with its [`Reaction::conversion`] set.
    pub reactions: Vec<Reaction>,
    /// Rank of each reaction, one per entry of [`reactions`](Self::reactions)
    /// (DWSIM `ReactionSetBase.Rank`). Groups run in ascending rank; equal ranks
    /// form a parallel group. [`ConversionReactor::new`] sets `0, 1, 2, …`.
    pub ranks: Vec<usize>,
}

/// Sweep cap for the Dykstra projection of an over-specified group.
const PROJECTION_MAX_SWEEPS: usize = 100_000;
/// Stop when no conversion moves by more than this in a sweep.
const PROJECTION_TOL: f64 = 1e-15;

impl ConversionReactor {
    /// Construct a conversion reactor whose reactions run **sequentially** in
    /// list order (reaction `i` has rank `i`).
    #[must_use]
    pub fn new(reactions: Vec<Reaction>) -> Self {
        let ranks = (0..reactions.len()).collect();
        Self { reactions, ranks }
    }

    /// Set the rank of each reaction, as in a DWSIM reaction set: reactions of
    /// equal rank run in parallel on the same group-inlet flows. `ranks` must
    /// have one entry per reaction, or [`solve`](Self::solve) returns
    /// [`ReactorError::InvalidFeed`].
    #[must_use]
    pub fn with_ranks(mut self, ranks: Vec<usize>) -> Self {
        self.ranks = ranks;
        self
    }

    /// Apply every reaction's fixed conversion to the `feed`, group by group,
    /// returning the outlet molar flows, per-reaction extents (in
    /// [`reactions`](Self::reactions) order), and net heat of reaction.
    ///
    /// Conversions are clamped to `[0, 1]`. A group whose specified conversions
    /// would drive any compound negative is reduced to the nearest feasible
    /// conversions in the least-squares sense (module docs).
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        let n = feed.molar_flows.len();
        if self.ranks.len() != self.reactions.len() {
            return Err(ReactorError::InvalidFeed(format!(
                "{} ranks given for {} reactions",
                self.ranks.len(),
                self.reactions.len()
            )));
        }
        for rxn in &self.reactions {
            for c in &rxn.components {
                if c.component_index >= n {
                    return Err(ReactorError::InvalidFeed(format!(
                        "reaction references component index {} but the feed has {} components",
                        c.component_index, n
                    )));
                }
            }
        }

        let mut flows = feed.molar_flows.clone();
        let mut extents = vec![0.0; self.reactions.len()];
        let mut heat = 0.0;

        let mut distinct = self.ranks.clone();
        distinct.sort_unstable();
        distinct.dedup();

        for rank in distinct {
            let group: Vec<usize> = (0..self.reactions.len())
                .filter(|&r| self.ranks[r] == rank)
                .collect();

            // Per reaction: the specified conversion, the extent per unit
            // conversion `−n_BC/ν_BC` at the group inlet, and `∂nᵢ/∂X_r`
            // (Conversion.vb:627: Δnᵢ = −X · νᵢ / ν_BC · n_BC).
            let x_spec: Vec<f64> = group
                .iter()
                .map(|&r| self.reactions[r].conversion.clamp(0.0, 1.0))
                .collect();
            let mut per_x = vec![0.0; group.len()];
            let mut dn_dx = vec![vec![0.0; n]; group.len()];
            for (g, &r) in group.iter().enumerate() {
                let rxn = &self.reactions[r];
                let sc_bc = rxn.base_stoich_coeff();
                if sc_bc != 0.0 {
                    per_x[g] = -flows[rxn.base_component_index()] / sc_bc;
                    for c in &rxn.components {
                        dn_dx[g][c.component_index] += c.stoich_coeff * per_x[g];
                    }
                }
            }

            let x = feasible_conversions(&flows, &dn_dx, &x_spec);

            for (g, &r) in group.iter().enumerate() {
                let extent = x[g] * per_x[g];
                extents[r] = extent;
                heat += self.reactions[r].reaction_heat * extent;
            }
            for (i, flow) in flows.iter_mut().enumerate() {
                let delta: f64 = (0..group.len()).map(|g| dn_dx[g][i] * x[g]).sum();
                *flow = (*flow + delta).max(0.0);
            }
        }

        Ok(ReactorOutcome {
            molar_flows: flows,
            extents,
            heat_of_reaction: heat,
        })
    }
}

/// The conversions closest in the least-squares sense to `x_spec` within
/// `0 ≤ x ≤ x_spec` that keep every `flows[i] + Σ_g dn_dx[g][i]·x[g] ≥ 0`.
///
/// Returns `x_spec` unchanged when it is feasible, the common case and the one
/// in which upstream's simplex also returns the specification. Otherwise runs
/// Dykstra's alternating projections onto the box and one half-space per
/// consumed compound, which converges to the exact Euclidean projection onto
/// their intersection (Boyle & Dykstra, 1986). That is upstream's objective
/// (`Conversion.vb:648`) without its penalty approximation.
fn feasible_conversions(flows: &[f64], dn_dx: &[Vec<f64>], x_spec: &[f64]) -> Vec<f64> {
    let m = x_spec.len();
    let n = flows.len();
    let is_feasible = (0..n).all(|i| {
        let d: f64 = (0..m).map(|g| dn_dx[g][i] * x_spec[g]).sum();
        flows[i] + d >= -1e-12 * flows[i].abs()
    });
    if is_feasible {
        return x_spec.to_vec();
    }

    // Half-spaces a·x ≥ b, one per compound that some reaction consumes.
    let halfspaces: Vec<(Vec<f64>, f64)> = (0..n)
        .filter(|&i| (0..m).any(|g| dn_dx[g][i] < 0.0))
        .map(|i| ((0..m).map(|g| dn_dx[g][i]).collect(), -flows[i]))
        .collect();
    let n_sets = 1 + halfspaces.len();
    let mut x = x_spec.to_vec();
    let mut increments = vec![vec![0.0; m]; n_sets];
    for _ in 0..PROJECTION_MAX_SWEEPS {
        let mut max_change = 0.0_f64;
        for (s, incr) in increments.iter_mut().enumerate() {
            let y: Vec<f64> = (0..m).map(|g| x[g] + incr[g]).collect();
            let p: Vec<f64> = if s == 0 {
                (0..m).map(|g| y[g].clamp(0.0, x_spec[g])).collect()
            } else {
                let (a, b) = &halfspaces[s - 1];
                let ay: f64 = (0..m).map(|g| a[g] * y[g]).sum();
                let aa: f64 = a.iter().map(|v| v * v).sum();
                if ay >= *b || aa == 0.0 {
                    y.clone()
                } else {
                    (0..m).map(|g| y[g] + (b - ay) / aa * a[g]).collect()
                }
            };
            for g in 0..m {
                incr[g] = y[g] - p[g];
                max_change = max_change.max((p[g] - x[g]).abs());
            }
            x = p;
        }
        if max_change < PROJECTION_TOL {
            break;
        }
    }
    // The last projection was onto a half-space; make the box hold exactly.
    for g in 0..m {
        x[g] = x[g].clamp(0.0, x_spec[g]);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactions::{ReactionBasis, ReactionComponent, ReactionKind};

    fn a_to_b(conversion: f64) -> Reaction {
        // A (idx 0) -> B (idx 1), base reactant A, ν = (-1, +1).
        Reaction::new(
            ReactionKind::Conversion,
            ReactionBasis::MolarConcentration,
            vec![
                ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
            ],
        )
        .with_conversion(conversion)
    }

    /// **Methodology (ANALYTIC).** A conversion reactor must honour the set
    /// conversion `X` of the base reactant *exactly*: with feed `F_A0 = 10`,
    /// `F_B0 = 0` mol/s and `X = 0.6`, the outlet must be `F_A = 4`, `F_B = 6`,
    /// and `conversion_of(A) = 0.6`. Atom balance: total moles conserved for the
    /// mole-conserving `A → B` (`10 → 10`).
    ///
    /// **Measured result (2026-08-03).** `F_A = 4.000000`, `F_B = 6.000000`,
    /// `X_A = 0.600000`, total out = 10.000000 — all to `< 1e−12`.
    #[test]
    fn honours_set_conversion_exactly() {
        let reactor = ConversionReactor::new(vec![a_to_b(0.6)]);
        let feed = ReactorFeed::new(vec![10.0, 0.0], 500.0, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();
        assert!((out.molar_flows[0] - 4.0).abs() < 1e-12);
        assert!((out.molar_flows[1] - 6.0).abs() < 1e-12);
        assert!((out.conversion_of(&feed, 0) - 0.6).abs() < 1e-12);
        // Mole (and, for A->B with equal molar mass basis, mass) balance closes.
        let total_in: f64 = feed.molar_flows.iter().sum();
        let total_out: f64 = out.molar_flows.iter().sum();
        assert!((total_in - total_out).abs() < 1e-12);
    }

    /// **Methodology (mass/atom balance for a mole-changing reaction).**
    /// `A → 2 B` (ν = −1, +2). With `F_A0 = 5`, `X = 1.0`, all A converts:
    /// `F_A = 0`, `F_B = 10`. Atom balance on the "A-atom" tracked by A+½B:
    /// `5 + 0 = 0 + ½·10`.
    ///
    /// **Measured result (2026-08-03).** `F_A = 0`, `F_B = 10.000000`; atom
    /// balance residual `< 1e−12`.
    #[test]
    fn mole_changing_reaction_atom_balance() {
        let rxn = Reaction::new(
            ReactionKind::Conversion,
            ReactionBasis::MolarConcentration,
            vec![
                ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
                ReactionComponent::new(1, 2.0, 0.0, 0.0, false),
            ],
        )
        .with_conversion(1.0);
        let reactor = ConversionReactor::new(vec![rxn]);
        let feed = ReactorFeed::new(vec![5.0, 0.0], 500.0, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();
        assert!(out.molar_flows[0].abs() < 1e-12);
        assert!((out.molar_flows[1] - 10.0).abs() < 1e-12);
        let atom_in = feed.molar_flows[0] + 0.5 * feed.molar_flows[1];
        let atom_out = out.molar_flows[0] + 0.5 * out.molar_flows[1];
        assert!((atom_in - atom_out).abs() < 1e-12);
    }

    /// **Methodology.** An over-specified conversion must not create negative
    /// flows: with only `F_A0 = 2` fed but a second reaction also consuming A,
    /// the extent is capped at the available amount.
    ///
    /// **Measured result (2026-08-03).** Two sequential `X = 1.0` reactions on
    /// A leave `F_A = 0` (not negative); no panic.
    #[test]
    fn extent_capped_at_availability() {
        let reactor = ConversionReactor::new(vec![a_to_b(1.0), a_to_b(1.0)]);
        let feed = ReactorFeed::new(vec![2.0, 0.0], 500.0, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();
        assert!(out.molar_flows[0] >= 0.0);
        assert!((out.molar_flows[0]).abs() < 1e-12);
    }
}
