//! Plug-flow reactor (PFR) — port of DWSIM `Reactors/PFR.vb`.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/PFR.vb` (commit
//! `1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
//! molar-balance ODE and the per-compound production term
//! `dNᵢ/dV = −rate · νᵢ / ν_BC` mirror the `ODEFunc` derivative assembly at lines
//! 260–500 (rate law lines 303–359; production sum lines 461–470; the returned
//! `dy = −Ri` at lines 475–481). DWSIM offers several stiff/non-stiff ODE
//! integrators (`InternalSolver`, lines 1108–1170); this port uses a fixed-step
//! classical Runge–Kutta 4 marching over reactor volume.
//!
//! ## Model
//!
//! A PFR integrates the steady-state molar balance along the reactor volume `V`
//! for each compound `i`:
//!
//! `dFᵢ/dV = Σ_r ( −rate_r(C) · νᵢᵣ / ν_BC,r )`,   `Cᵢ = Fᵢ / Q`
//!
//! from `V = 0` (the feed) to `V = volume`, with the volumetric flow `Q` held
//! constant (see [`crate::reactors`] "Honest scope"). `rate_r` is the power-law
//! rate from [`Reaction::net_rate`]. The per-reaction extent
//! `ζ_r = ∫₀^V rate_r dV` [mol/s] is accumulated with the same RK4 weights.
//!
//! ⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

use crate::reactions::Reaction;

use super::{ReactorError, ReactorFeed, ReactorOutcome};

/// A plug-flow reactor: a reaction list, a total volume, and the number of RK4
/// integration sub-steps.
#[derive(Debug, Clone, PartialEq)]
pub struct Pfr {
    /// The kinetic reactions driving the balance (typically
    /// [`ReactionKind::Kinetic`](crate::reactions::ReactionKind::Kinetic)).
    pub reactions: Vec<Reaction>,
    /// Total reactor volume `V` [m³].
    pub volume: f64,
    /// Number of fixed RK4 sub-steps over `[0, V]`. More steps = more accurate;
    /// 100–1000 is ample for the smooth balances here.
    pub n_steps: usize,
}

impl Pfr {
    /// Construct a PFR. `n_steps` is clamped to at least 1.
    #[must_use]
    pub fn new(reactions: Vec<Reaction>, volume: f64, n_steps: usize) -> Self {
        Self {
            reactions,
            volume,
            n_steps: n_steps.max(1),
        }
    }

    /// Evaluate `(dF/dV, per-reaction rate)` at the current molar flows.
    ///
    /// `dfdv[i] = Σ_r −rate_r · νᵢᵣ / ν_BC,r`; `rates[r] = rate_r(C)`,
    /// `Cᵢ = max(Fᵢ, 0) / Q`.
    fn derivatives(&self, flows: &[f64], q: f64, temperature: f64) -> (Vec<f64>, Vec<f64>) {
        let n = flows.len();
        let mut conc = vec![0.0; n];
        for (i, &f) in flows.iter().enumerate() {
            conc[i] = f.max(0.0) / q;
        }
        let mut dfdv = vec![0.0; n];
        let mut rates = Vec::with_capacity(self.reactions.len());
        for rxn in &self.reactions {
            let rate = rxn.net_rate(&conc, temperature);
            let sc_bc = rxn.base_stoich_coeff();
            for c in &rxn.components {
                // Production per unit volume: dN_i/dV = -rate * ν_i / ν_BC.
                dfdv[c.component_index] += -rate * c.stoich_coeff / sc_bc;
            }
            rates.push(rate);
        }
        (dfdv, rates)
    }

    /// Integrate the PFR balance from the `feed` to the reactor outlet.
    ///
    /// Requires `Q > 0` (concentrations are `F/Q`). Returns the outlet molar
    /// flows, per-reaction integrated extents `ζ_r = ∫ rate_r dV`, and the net
    /// heat of reaction `Σ_r ΔH°_r · ζ_r`.
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        let n = feed.molar_flows.len();
        let q = feed.volumetric_flow;
        if !(q > 0.0) {
            return Err(ReactorError::InvalidFeed(
                "PFR requires a positive volumetric flow Q (concentrations are F/Q)".into(),
            ));
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

        let t = feed.temperature;
        let h = self.volume / self.n_steps as f64;
        let mut y = feed.molar_flows.clone();
        let mut extents = vec![0.0; self.reactions.len()];

        let add_scaled = |base: &[f64], delta: &[f64], scale: f64| -> Vec<f64> {
            base.iter()
                .zip(delta.iter())
                .map(|(b, d)| b + scale * d)
                .collect()
        };

        for _ in 0..self.n_steps {
            let (k1f, k1r) = self.derivatives(&y, q, t);
            let y2 = add_scaled(&y, &k1f, h / 2.0);
            let (k2f, k2r) = self.derivatives(&y2, q, t);
            let y3 = add_scaled(&y, &k2f, h / 2.0);
            let (k3f, k3r) = self.derivatives(&y3, q, t);
            let y4 = add_scaled(&y, &k3f, h);
            let (k4f, k4r) = self.derivatives(&y4, q, t);

            for i in 0..n {
                y[i] += h / 6.0 * (k1f[i] + 2.0 * k2f[i] + 2.0 * k3f[i] + k4f[i]);
                if y[i] < 0.0 {
                    y[i] = 0.0;
                }
            }
            for r in 0..self.reactions.len() {
                extents[r] += h / 6.0 * (k1r[r] + 2.0 * k2r[r] + 2.0 * k3r[r] + k4r[r]);
            }
        }

        let heat = self
            .reactions
            .iter()
            .zip(extents.iter())
            .map(|(rxn, ext)| rxn.reaction_heat * ext)
            .sum();

        Ok(ReactorOutcome {
            molar_flows: y,
            extents,
            heat_of_reaction: heat,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactions::{ReactionBasis, ReactionComponent, ReactionKind};

    /// First-order irreversible A → B, forward-order 1 in A, no reverse.
    fn first_order_a_to_b(k: f64) -> Reaction {
        Reaction::new(
            ReactionKind::Kinetic,
            ReactionBasis::MolarConcentration,
            vec![
                ReactionComponent::new(0, -1.0, 1.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
            ],
        )
        // A_forward = k, E = 0 so the rate constant equals k at any T.
        .with_forward(k, 0.0)
    }

    /// **Methodology (ANALYTIC).** Isothermal first-order `A → B` in a PFR has
    /// the closed-form conversion `X = 1 − exp(−k·τ)`, `τ = V/Q`. With `k = 2 s⁻¹`,
    /// `V = 1 m³`, `Q = 0.5 m³/s` ⇒ `τ = 2 s`, `k·τ = 4`, so
    /// `X = 1 − e⁻⁴ = 0.981684…`. Feed `F_A0 = 1 mol/s`. RK4 with 500 steps.
    ///
    /// **Measured result (2026-08-03).** Numerical `X = 0.9816844…` vs analytic
    /// `0.9816844`; `|ΔX| < 1e−6`. Atom balance `F_A + F_B = F_A0` closes to
    /// `< 1e−9`.
    #[test]
    fn first_order_matches_analytic_conversion() {
        let k = 2.0;
        let reactor = Pfr::new(vec![first_order_a_to_b(k)], 1.0, 500);
        let feed = ReactorFeed::new(vec![1.0, 0.0], 500.0, 1.0e5, 0.5);
        let out = reactor.solve(&feed).unwrap();

        let tau = reactor.volume / feed.volumetric_flow; // 2 s
        let x_analytic = 1.0 - (-k * tau).exp();
        let x_num = out.conversion_of(&feed, 0);
        assert!(
            (x_num - x_analytic).abs() < 1e-6,
            "X_num={x_num}, X_analytic={x_analytic}"
        );

        // Mole balance A + B conserved (A -> B is mole-conserving).
        let total: f64 = out.molar_flows.iter().sum();
        assert!((total - 1.0).abs() < 1e-9);

        // Extent equals moles of A consumed.
        assert!((out.extents[0] - (1.0 - out.molar_flows[0])).abs() < 1e-9);
    }

    /// **Methodology.** Longer residence time must give higher conversion, and
    /// as `τ → ∞` conversion → 1 for an irreversible reaction.
    ///
    /// **Measured result (2026-08-03).** Doubling `V` (0.5 → 1.0 m³, same Q)
    /// raises `X` monotonically; a large `V = 20 m³` gives `X > 0.9999`.
    #[test]
    fn conversion_increases_with_residence_time() {
        let k = 2.0;
        let feed = ReactorFeed::new(vec![1.0, 0.0], 500.0, 1.0e5, 0.5);
        let x_small = Pfr::new(vec![first_order_a_to_b(k)], 0.5, 500)
            .solve(&feed)
            .unwrap()
            .conversion_of(&feed, 0);
        let x_big = Pfr::new(vec![first_order_a_to_b(k)], 1.0, 500)
            .solve(&feed)
            .unwrap()
            .conversion_of(&feed, 0);
        assert!(x_big > x_small);
        let x_huge = Pfr::new(vec![first_order_a_to_b(k)], 20.0, 2000)
            .solve(&feed)
            .unwrap()
            .conversion_of(&feed, 0);
        assert!(x_huge > 0.9999);
    }

    /// **Methodology.** `Q ≤ 0` must be rejected (concentrations are `F/Q`).
    ///
    /// **Measured result (2026-08-03).** `Q = 0` returns
    /// `ReactorError::InvalidFeed`.
    #[test]
    fn rejects_nonpositive_flow() {
        let reactor = Pfr::new(vec![first_order_a_to_b(1.0)], 1.0, 10);
        let feed = ReactorFeed::new(vec![1.0, 0.0], 500.0, 1.0e5, 0.0);
        assert!(matches!(
            reactor.solve(&feed),
            Err(ReactorError::InvalidFeed(_))
        ));
    }
}
