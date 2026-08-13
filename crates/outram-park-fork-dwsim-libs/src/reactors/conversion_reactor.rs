//! Fixed-conversion reactor — port of DWSIM `Reactors/Conversion.vb`.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Conversion.vb` (commit
//! `1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
//! per-compound mole-flow update mirrors the delta-mole-flow loop at lines
//! 622–702: `Δnᵢ = −X · νᵢ / ν_BC · n_BC`, where `X` is the specified fractional
//! conversion of the base reactant, `ν` the stoichiometric coefficients, and
//! `n_BC` the base reactant's inlet molar flow.
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
//! and `Δnᵢ = νᵢ · ζ_r`. Reactions are applied in list order (DWSIM's
//! sequential-group treatment); each reactant flow is clamped at zero so a
//! later reaction cannot drive a compound negative.
//!
//! ⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

use crate::reactions::Reaction;

use super::{ReactorError, ReactorFeed, ReactorOutcome};

/// A fixed-conversion reactor holding a list of conversion reactions applied in
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionReactor {
    /// The reactions to apply; each should be
    /// [`ReactionKind::Conversion`](crate::reactions::ReactionKind::Conversion)
    /// with its [`Reaction::conversion`] set.
    pub reactions: Vec<Reaction>,
}

impl ConversionReactor {
    /// Construct a conversion reactor from its reaction list.
    #[must_use]
    pub fn new(reactions: Vec<Reaction>) -> Self {
        Self { reactions }
    }

    /// Apply every reaction's fixed conversion to the `feed`, returning the
    /// outlet molar flows, per-reaction extents, and net heat of reaction.
    ///
    /// For each reaction the base reactant's *current* flow (after any earlier
    /// reaction in the list) sets the extent, so sequential reactions compound
    /// as in DWSIM. Conversions are clamped to `[0, 1]`; a conversion that would
    /// drive any compound below zero is capped so the mole balance stays
    /// non-negative.
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        let n = feed.molar_flows.len();
        let mut flows = feed.molar_flows.clone();
        let mut extents = Vec::with_capacity(self.reactions.len());
        let mut heat = 0.0;

        for rxn in &self.reactions {
            // Validate indices.
            for c in &rxn.components {
                if c.component_index >= n {
                    return Err(ReactorError::InvalidFeed(format!(
                        "reaction references component index {} but the feed has {} components",
                        c.component_index, n
                    )));
                }
            }

            let x = rxn.conversion.clamp(0.0, 1.0);
            let bc = rxn.base_component_index();
            let sc_bc = rxn.base_stoich_coeff();
            let n_bc = flows[bc];

            // Extent as written: Δn_BC = ν_BC · ζ = −X · n_BC  ⇒  ζ = −X·n_BC/ν_BC.
            let mut extent = if sc_bc != 0.0 { -x * n_bc / sc_bc } else { 0.0 };

            // Cap the extent so no compound goes negative (protects against an
            // over-specified conversion when a reactant is shared/limiting).
            for c in &rxn.components {
                if c.stoich_coeff < 0.0 {
                    // available / consumption-per-unit-extent
                    let max_extent = flows[c.component_index] / (-c.stoich_coeff);
                    if extent > max_extent {
                        extent = max_extent;
                    }
                }
            }
            if extent < 0.0 {
                extent = 0.0;
            }

            for c in &rxn.components {
                flows[c.component_index] += c.stoich_coeff * extent;
                if flows[c.component_index] < 0.0 {
                    flows[c.component_index] = 0.0;
                }
            }

            heat += rxn.reaction_heat * extent;
            extents.push(extent);
        }

        Ok(ReactorOutcome {
            molar_flows: flows,
            extents,
            heat_of_reaction: heat,
        })
    }
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
