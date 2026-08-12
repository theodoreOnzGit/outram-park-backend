//! Chemical-equilibrium reactor — port of DWSIM `Reactors/Equilibrium.vb`.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Equilibrium.vb` (commit
//! `1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
//! reaction-extent formulation `nᵢ = nᵢ,0 + Σ_r Eᵢᵣ ζ_r` and the per-reaction
//! residual `f_r = ln(∏ᵢ basisᵢ^νᵢ) − ln K_r` mirror `FunctionValue2N`
//! (lines 234–381: mole update lines 244–253, basis product lines 332–363,
//! `f(i) = ln(prod/kr)` line 379, with `kr = EvaluateK(T + Approach)`).
//!
//! ## Model
//!
//! An equilibrium reactor finds the reaction extents `ζ` such that every
//! reaction simultaneously satisfies its equilibrium constant:
//!
//! `∏ᵢ (basisᵢ)^νᵢ = K_r(T)`   ⇔   `f_r(ζ) = ln(∏ᵢ basisᵢ^νᵢ) − ln K_r = 0`,
//!
//! with the mole amounts parameterised by the extents, `nᵢ = nᵢ,0 + Σ_r νᵢᵣ ζ_r`
//! (guaranteeing the atom balance by construction). The nonlinear system is
//! solved by a damped Newton iteration (finite-difference Jacobian, Gaussian
//! elimination) that keeps all `nᵢ ≥ 0`.
//!
//! The activity basis is evaluated **ideally** (activity/fugacity coefficients
//! = 1): [`ReactionBasis::MolarFraction`] uses `xᵢ`,
//! [`ReactionBasis::PartialPressure`] uses `xᵢ·P` [Pa], and
//! [`ReactionBasis::Activity`] is treated as `xᵢ`. This is an honest
//! simplification — DWSIM calls the property package for real fugacity
//! coefficients.
//!
//! ⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

use crate::reactions::{Reaction, ReactionBasis};

use super::{solve_linear, ReactorError, ReactorFeed, ReactorOutcome};

/// A chemical-equilibrium reactor: a list of equilibrium reactions solved
/// simultaneously for their extents.
#[derive(Debug, Clone, PartialEq)]
pub struct EquilibriumReactor {
    /// The equilibrium reactions (each carries its
    /// [`k_eq`](Reaction::k_eq) model).
    pub reactions: Vec<Reaction>,
    /// Maximum Newton iterations (default via [`EquilibriumReactor::new`]: 300).
    pub max_iter: usize,
    /// Convergence tolerance on the `ln`-residual norm (default: `1e−10`).
    pub tol: f64,
}

/// Lower floor on a mole amount when forming the activity basis, so `ln` stays
/// finite as a species is driven toward zero. Scaled by the total feed below.
const AMOUNT_FLOOR_FRACTION: f64 = 1e-14;

impl EquilibriumReactor {
    /// Construct an equilibrium reactor with default solver settings.
    #[must_use]
    pub fn new(reactions: Vec<Reaction>) -> Self {
        Self {
            reactions,
            max_iter: 300,
            tol: 1e-10,
        }
    }

    /// Mole amounts implied by extents: `nᵢ = nᵢ,0 + Σ_r νᵢᵣ ζ_r`.
    fn amounts(&self, feed: &ReactorFeed, extents: &[f64]) -> Vec<f64> {
        let mut n = feed.molar_flows.clone();
        for (r, rxn) in self.reactions.iter().enumerate() {
            for c in &rxn.components {
                n[c.component_index] += c.stoich_coeff * extents[r];
            }
        }
        n
    }

    /// Activity-basis value of one compound given its (floored) amount, the
    /// total, and the pressure.
    fn basis_value(basis: ReactionBasis, n_i: f64, n_tot: f64, pressure: f64) -> f64 {
        let x = n_i / n_tot;
        match basis {
            ReactionBasis::PartialPressure => x * pressure,
            // MolarFraction, Activity (ideal γ=1), and anything else fall back
            // to the mole fraction (documented simplification).
            _ => x,
        }
    }

    /// Per-reaction residual `f_r = ln(∏ basisᵢ^νᵢ) − ln K_r`. Returns `None`
    /// if any amount is non-positive (infeasible extent set).
    fn residual(&self, feed: &ReactorFeed, extents: &[f64], floor: f64) -> Option<Vec<f64>> {
        let n = self.amounts(feed, extents);
        if n.iter().any(|&v| v < 0.0) {
            return None;
        }
        let n_floored: Vec<f64> = n.iter().map(|&v| v.max(floor)).collect();
        let n_tot: f64 = n_floored.iter().sum();
        let t = feed.temperature;
        let mut f = Vec::with_capacity(self.reactions.len());
        for rxn in &self.reactions {
            let mut ln_prod = 0.0;
            for c in &rxn.components {
                let b = Self::basis_value(
                    rxn.basis,
                    n_floored[c.component_index],
                    n_tot,
                    feed.pressure,
                );
                ln_prod += c.stoich_coeff * b.ln();
            }
            let k = rxn.equilibrium_constant(t).max(1e-300);
            f.push(ln_prod - k.ln());
        }
        Some(f)
    }

    /// Heuristic initial extents: convert ~50% of each reaction's limiting
    /// reactant, so products start non-zero and `ln` is finite.
    fn initial_extents(&self, feed: &ReactorFeed) -> Vec<f64> {
        self.reactions
            .iter()
            .map(|rxn| {
                let mut limiting = f64::INFINITY;
                for c in &rxn.components {
                    if c.stoich_coeff < 0.0 {
                        let cap = feed.molar_flows[c.component_index] / (-c.stoich_coeff);
                        if cap < limiting {
                            limiting = cap;
                        }
                    }
                }
                if limiting.is_finite() {
                    0.5 * limiting
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Solve the equilibrium reactor for the `feed`.
    ///
    /// Returns the outlet mole amounts (as molar flows, same units as the feed),
    /// per-reaction extents `ζ_r`, and the net heat of reaction
    /// `Σ_r ΔH°_r · ζ_r`. Returns [`ReactorError::NonConvergence`] if the Newton
    /// iteration fails.
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        let n = feed.molar_flows.len();
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
        let r = self.reactions.len();
        if r == 0 {
            return Ok(ReactorOutcome {
                molar_flows: feed.molar_flows.clone(),
                extents: vec![],
                heat_of_reaction: 0.0,
            });
        }

        let floor = feed.total_molar_flow().max(1.0) * AMOUNT_FLOOR_FRACTION;
        let mut extents = self.initial_extents(feed);
        let mut last_residual = f64::INFINITY;

        for iter in 0..self.max_iter {
            let g = match self.residual(feed, &extents, floor) {
                Some(g) => g,
                None => {
                    // Infeasible start (shouldn't happen with the 50% seed);
                    // pull extents back toward zero and retry.
                    for e in &mut extents {
                        *e *= 0.5;
                    }
                    continue;
                }
            };
            let norm = g.iter().map(|v| v * v).sum::<f64>().sqrt();
            last_residual = norm;
            if norm < self.tol {
                break;
            }

            // Finite-difference Jacobian J[r][s] = ∂f_r/∂ζ_s.
            let mut jac = vec![vec![0.0; r]; r];
            let mut singular = false;
            for s in 0..r {
                let h = 1e-8 * extents[s].abs().max(1.0);
                let mut ep = extents.clone();
                ep[s] += h;
                match self.residual(feed, &ep, floor) {
                    Some(gp) => {
                        for row in 0..r {
                            jac[row][s] = (gp[row] - g[row]) / h;
                        }
                    }
                    None => {
                        // step outside feasible region: use a backward difference
                        let mut em = extents.clone();
                        em[s] -= h;
                        if let Some(gm) = self.residual(feed, &em, floor) {
                            for row in 0..r {
                                jac[row][s] = (g[row] - gm[row]) / h;
                            }
                        } else {
                            singular = true;
                        }
                    }
                }
            }
            if singular {
                return Err(ReactorError::NonConvergence {
                    iterations: iter,
                    residual: norm,
                });
            }

            let neg_g: Vec<f64> = g.iter().map(|v| -v).collect();
            let delta = match solve_linear(jac, neg_g) {
                Some(d) => d,
                None => {
                    return Err(ReactorError::NonConvergence {
                        iterations: iter,
                        residual: norm,
                    })
                }
            };

            // Damped, feasibility-preserving line search.
            let mut lambda = 1.0;
            let mut accepted = false;
            for _ in 0..40 {
                let trial: Vec<f64> = extents
                    .iter()
                    .zip(delta.iter())
                    .map(|(e, d)| e + lambda * d)
                    .collect();
                if let Some(gt) = self.residual(feed, &trial, floor) {
                    let nt = gt.iter().map(|v| v * v).sum::<f64>().sqrt();
                    if nt < norm {
                        extents = trial;
                        accepted = true;
                        break;
                    }
                }
                lambda *= 0.5;
            }
            if !accepted {
                // Nudge by the smallest feasible fraction of the step.
                let trial: Vec<f64> = extents
                    .iter()
                    .zip(delta.iter())
                    .map(|(e, d)| e + 1e-3 * d)
                    .collect();
                if self.residual(feed, &trial, floor).is_some() {
                    extents = trial;
                }
            }
        }

        if last_residual > self.tol.max(1e-6) {
            return Err(ReactorError::NonConvergence {
                iterations: self.max_iter,
                residual: last_residual,
            });
        }

        let amounts = self.amounts(feed, &extents);
        let amounts: Vec<f64> = amounts.into_iter().map(|v| v.max(0.0)).collect();
        let heat = self
            .reactions
            .iter()
            .zip(extents.iter())
            .map(|(rxn, ext)| rxn.reaction_heat * ext)
            .sum();

        Ok(ReactorOutcome {
            molar_flows: amounts,
            extents,
            heat_of_reaction: heat,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactions::{EquilibriumConstant, ReactionComponent, ReactionKind};

    /// A ⇌ B with a constant equilibrium constant, mole-fraction basis.
    fn a_eq_b(k: f64) -> Reaction {
        Reaction::new(
            ReactionKind::Equilibrium,
            ReactionBasis::MolarFraction,
            vec![
                ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
            ],
        )
        .with_k_eq(EquilibriumConstant::Constant(k))
    }

    /// **Methodology (ANALYTIC).** For `A ⇌ B` on a mole-fraction basis the
    /// equilibrium condition is `K = x_B / x_A`. Since `A → B` conserves total
    /// moles, `x_B/x_A = n_B/n_A`, and with feed `n_A0 = 1`, `n_B0 = 0` the
    /// extent solves `ζ/(1−ζ) = K`, i.e. `ζ = K/(1+K)`. For `K = 3`,
    /// `ζ = 0.75`, `n_A = 0.25`, `n_B = 0.75`, and `x_B/x_A = 3` exactly.
    ///
    /// **Measured result (2026-08-03).** Newton `n_A = 0.250000`,
    /// `n_B = 0.750000`; `x_B/x_A = 3.0000000` to `< 1e−8`. Atom balance
    /// `n_A + n_B = 1` to `< 1e−10`.
    #[test]
    fn a_eq_b_reaches_keq() {
        let k = 3.0;
        let reactor = EquilibriumReactor::new(vec![a_eq_b(k)]);
        let feed = ReactorFeed::new(vec![1.0, 0.0], 500.0, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();

        let n_a = out.molar_flows[0];
        let n_b = out.molar_flows[1];
        let ratio = n_b / n_a; // = x_B/x_A since total moles cancel
        assert!((ratio - k).abs() < 1e-8, "x_B/x_A={ratio}, K={k}");
        assert!((n_a - 0.25).abs() < 1e-8);
        assert!((n_b - 0.75).abs() < 1e-8);
        // Atom (total mole) balance closes.
        assert!((n_a + n_b - 1.0).abs() < 1e-10);
    }

    /// **Methodology.** A larger `K` must shift equilibrium further toward the
    /// product. Compare `K = 3` and `K = 9`: `ζ = K/(1+K)` gives `0.75` vs
    /// `0.9`.
    ///
    /// **Measured result (2026-08-03).** `n_B(K=9) = 0.900000 > n_B(K=3) =
    /// 0.750000`; both match `K/(1+K)` to `< 1e−8`.
    #[test]
    fn larger_k_shifts_toward_product() {
        let feed = ReactorFeed::new(vec![1.0, 0.0], 500.0, 1.0e5, 0.0);
        let nb3 = EquilibriumReactor::new(vec![a_eq_b(3.0)])
            .solve(&feed)
            .unwrap()
            .molar_flows[1];
        let nb9 = EquilibriumReactor::new(vec![a_eq_b(9.0)])
            .solve(&feed)
            .unwrap()
            .molar_flows[1];
        assert!(nb9 > nb3);
        assert!((nb9 - 0.9).abs() < 1e-8);
    }

    /// **Methodology (V&V: K_eq from van 't Hoff).** Use a temperature-dependent
    /// `K` via [`EquilibriumConstant::GibbsVantHoff`] and confirm the solved
    /// composition satisfies `x_B/x_A = K(T)` at the feed temperature.
    ///
    /// **Measured result (2026-08-03).** With `ΔH = −20 000 J/mol`, `ΔS = 0` at
    /// `T = 600 K`, `K = exp(20000/(8.314·600)) = 55.0…`; solved `x_B/x_A`
    /// matches `K(600)` to `< 1e−6`.
    #[test]
    fn temperature_dependent_k_satisfied() {
        let rxn = Reaction::new(
            ReactionKind::Equilibrium,
            ReactionBasis::MolarFraction,
            vec![
                ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
            ],
        )
        .with_k_eq(EquilibriumConstant::GibbsVantHoff {
            delta_h: -20_000.0,
            delta_s: 0.0,
        });
        let t = 600.0;
        let feed = ReactorFeed::new(vec![1.0, 0.0], t, 1.0e5, 0.0);
        let out = EquilibriumReactor::new(vec![rxn.clone()])
            .solve(&feed)
            .unwrap();
        let ratio = out.molar_flows[1] / out.molar_flows[0];
        let k_expected = rxn.equilibrium_constant(t);
        assert!((ratio - k_expected).abs() / k_expected < 1e-6);
    }
}
