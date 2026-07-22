//! Equilibrium aqueous speciation — the reaction-network core.
//!
//! This module implements the classic PHREEQC/PFLOTRAN component-based
//! speciation solve: given the *total* concentration of each PRIMARY (component)
//! species, find the free (uncomplexed) concentration of each primary such that
//! every SECONDARY species is in equilibrium (law of mass action) and every
//! component's mass balance is satisfied.
//!
//! # Physical model (v1)
//!
//! A system has `Nc` primary species and `Ns` secondary species. Concentrations
//! are molar, **mol/L**. Each secondary species `i` is formed from the primaries
//! by an equilibrium reaction with stoichiometry `nu[i][j]` (moles of primary
//! `j` consumed to make one mole of secondary `i`) and a base-10 equilibrium
//! constant `log10_k[i]`:
//!
//! - **Mass action** (secondary concentration):
//!   `C_sec[i] = 10^{log10_k[i]} * prod_j C_prim[j]^{nu[i][j]}`.
//! - **Mass balance** (total of component `j`):
//!   `T[j] = C_prim[j] + sum_i nu[i][j] * C_sec[i]`.
//!
//! # Simplifications (flagged for human review)
//!
//! - **Ideal activities** — activity coefficients `gamma = 1`, so activity
//!   equals concentration. Debye–Hückel / Davies corrections are **deferred**
//!   to a follow-up (they multiply each species activity and change the
//!   mass-action product; the Newton skeleton here is unchanged by adding them).
//! - **No mineral phases** — precipitation/dissolution and the associated
//!   solubility-product constraints are not modelled here (later bead).
//! - **No kinetic reactions** — only instantaneous equilibrium.
//! - **No explicit charge-balance constraint** — the caller supplies the total
//!   of every component (including any proton/charge component) directly. The
//!   solve enforces mass action + mass balance on those totals; it does not add
//!   an electroneutrality equation. A caller wanting a charge-balanced state
//!   must choose totals that are themselves charge-consistent (see the
//!   symmetric weak-acid verification case in the tests).
//! - **No water/H2O activity** — the solvent is not treated as a component.
//!
//! # Numerics
//!
//! The solve is Newton's method in `x_j = ln C_prim[j]`, which keeps every
//! primary concentration strictly positive for any real `x`. See
//! [`ReactionNetwork::speciate`] for the exact residual and Jacobian.

use crate::error::PflotranError;
use outram_foam_basic_lib::matrix::SquareMatrix;

/// Natural log of 10, used to convert `log10_k` to a natural-log offset.
const LN_10: f64 = core::f64::consts::LN_10;

/// Maximum Newton iterations before the solve is declared non-convergent.
const MAX_ITERS: usize = 200;

/// Relative residual tolerance: the solve is converged when, for every
/// component `j`, `|f_j| <= RESIDUAL_TOL * (|T[j]| + ABS_FLOOR)`.
const RESIDUAL_TOL: f64 = 1.0e-12;

/// Absolute floor added to the totals when scaling the residual, so a component
/// with a (near-)zero total still has a finite convergence threshold.
const ABS_FLOOR: f64 = 1.0e-30;

/// Largest allowed change in `x_j = ln C_prim[j]` per Newton step. The raw
/// Newton update is scaled down so that its largest component does not exceed
/// this, damping the early iterations against overshoot. `4.0` corresponds to a
/// single-step change of at most a factor of `e^4 ≈ 55` in any concentration.
const MAX_LOG_STEP: f64 = 4.0;

/// Clamp on `x_j = ln C_prim[j]` to keep `exp(x_j)` finite (avoids overflow to
/// infinity from a transient large iterate). `±690` keeps `exp` within f64 range.
const X_CLAMP: f64 = 690.0;

/// An equilibrium aqueous reaction network: `Nc` primary + `Ns` secondary species.
///
/// Concentrations are in mol/L; equilibrium constants are dimensionless (ideal
/// activities, so activity equals concentration in v1). Secondary species `i`
/// has a `log10_k` and a stoichiometry row, and its concentration follows the
/// law of mass action
/// `C_sec[i] = 10^{log10_k[i]} * prod_j C_prim[j]^{stoich[i][j]}`.
///
/// Construct with [`ReactionNetwork::new`] (which validates the shapes) and
/// solve a state with [`ReactionNetwork::speciate`].
#[derive(Debug, Clone)]
pub struct ReactionNetwork {
    /// Number of primary (component) species, `Nc >= 1`.
    n_primary: usize,
    /// Number of secondary (equilibrium) species, `Ns >= 0`.
    n_secondary: usize,
    /// Stoichiometry, `Ns` rows by `Nc` cols. `stoich[i][j]` is the moles of
    /// primary `j` in one mole of secondary `i`.
    stoich: Vec<Vec<f64>>,
    /// Base-10 equilibrium constants, length `Ns`.
    log10_k: Vec<f64>,
    /// Primary species names, length `Nc` (human-facing labels only).
    primary_names: Vec<String>,
    /// Secondary species names, length `Ns` (human-facing labels only).
    secondary_names: Vec<String>,
}

/// Result of an equilibrium speciation solve.
///
/// All concentrations are in mol/L and strictly positive. `primary[j]` is the
/// free (uncomplexed) concentration of primary species `j`; `secondary[i]` is
/// the equilibrium concentration of secondary species `i`; `iterations` is the
/// number of Newton iterations taken to reach the tolerance.
#[derive(Debug, Clone)]
pub struct Speciation {
    /// Free primary (component) concentrations, mol/L, length `Nc`.
    pub primary: Vec<f64>,
    /// Secondary species concentrations, mol/L, length `Ns`.
    pub secondary: Vec<f64>,
    /// Newton iterations taken to converge.
    pub iterations: usize,
}

impl ReactionNetwork {
    /// Build a reaction network from species names, a stoichiometry matrix, and
    /// base-10 equilibrium constants, validating the shapes.
    ///
    /// # Parameters
    ///
    /// - `primary`: names of the `Nc` primary (component) species. `Nc >= 1`.
    /// - `secondary`: names of the `Ns` secondary (equilibrium) species. `Ns`
    ///   may be `0` (an inert set of components).
    /// - `stoich`: an `Ns`-row by `Nc`-col matrix; `stoich[i][j]` is the moles
    ///   of primary `j` in one mole of secondary `i` (may be negative or
    ///   non-integer). Row count must equal `secondary.len()` and every row's
    ///   length must equal `primary.len()`.
    /// - `log10_k`: base-10 equilibrium constants, one per secondary species
    ///   (length `Ns`). Dimensionless (ideal activities).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `primary` is empty, or if any
    /// of `secondary`, `stoich`, or `log10_k` disagree on `Ns`, or if any
    /// stoichiometry row does not have exactly `Nc` entries.
    pub fn new(
        primary: Vec<String>,
        secondary: Vec<String>,
        stoich: Vec<Vec<f64>>,
        log10_k: Vec<f64>,
    ) -> Result<Self, PflotranError> {
        let n_primary = primary.len();
        let n_secondary = secondary.len();

        if n_primary == 0 {
            return Err(PflotranError::InvalidInput(
                "reaction network needs at least one primary species".to_string(),
            ));
        }
        if stoich.len() != n_secondary {
            return Err(PflotranError::InvalidInput(format!(
                "stoichiometry has {} rows but there are {} secondary species",
                stoich.len(),
                n_secondary
            )));
        }
        if log10_k.len() != n_secondary {
            return Err(PflotranError::InvalidInput(format!(
                "log10_k has length {} but there are {} secondary species",
                log10_k.len(),
                n_secondary
            )));
        }
        for (i, row) in stoich.iter().enumerate() {
            if row.len() != n_primary {
                return Err(PflotranError::InvalidInput(format!(
                    "stoichiometry row {} has {} entries but there are {} primary species",
                    i,
                    row.len(),
                    n_primary
                )));
            }
            for (j, v) in row.iter().enumerate() {
                if !v.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "stoichiometry[{i}][{j}] is not finite"
                    )));
                }
            }
        }
        for (i, k) in log10_k.iter().enumerate() {
            if !k.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "log10_k[{i}] is not finite"
                )));
            }
        }

        Ok(Self {
            n_primary,
            n_secondary,
            stoich,
            log10_k,
            primary_names: primary,
            secondary_names: secondary,
        })
    }

    /// Number of primary (component) species, `Nc`.
    pub fn n_primary(&self) -> usize {
        self.n_primary
    }

    /// Number of secondary (equilibrium) species, `Ns`.
    pub fn n_secondary(&self) -> usize {
        self.n_secondary
    }

    /// Names of the primary (component) species, in column order.
    pub fn primary_names(&self) -> &[String] {
        &self.primary_names
    }

    /// Names of the secondary (equilibrium) species, in row order.
    pub fn secondary_names(&self) -> &[String] {
        &self.secondary_names
    }

    /// Compute the secondary concentrations `C_sec[i]` from a set of log-primary
    /// concentrations `x_j = ln C_prim[j]`, using the law of mass action in log
    /// form: `ln C_sec[i] = ln(10) * log10_k[i] + sum_j stoich[i][j] * x_j`.
    ///
    /// Working in logs before exponentiating avoids repeated `powf` calls and is
    /// numerically better-behaved for small concentrations. `x` has length `Nc`;
    /// the returned vector has length `Ns`.
    fn secondary_from_log_primary(&self, x: &[f64]) -> Vec<f64> {
        let mut c_sec = vec![0.0; self.n_secondary];
        for i in 0..self.n_secondary {
            let mut ln_c = self.log10_k[i] * LN_10;
            for j in 0..self.n_primary {
                ln_c += self.stoich[i][j] * x[j];
            }
            c_sec[i] = ln_c.exp();
        }
        c_sec
    }

    /// Solve for the free primary concentrations that satisfy mass action and
    /// mass balance for the given total component concentrations.
    ///
    /// # Method — Newton on log-concentration
    ///
    /// The unknowns are `x_j = ln C_prim[j]`, so `C_prim[j] = exp(x_j) > 0` for
    /// any real `x_j` (positivity is structural, never enforced by clamping the
    /// physical answer). For each component `j` the residual is the mass-balance
    /// mismatch
    ///
    /// ```text
    /// f_j(x) = C_prim[j] + sum_i stoich[i][j] * C_sec[i] - T[j]
    /// ```
    ///
    /// with `C_sec[i] = 10^{log10_k[i]} * prod_k exp(x_k)^{stoich[i][k]}`. The
    /// analytic Jacobian uses `dC_prim[j]/dx_k = delta_jk * C_prim[j]` and
    /// `dC_sec[i]/dx_k = stoich[i][k] * C_sec[i]`:
    ///
    /// ```text
    /// J[j][k] = df_j/dx_k = delta_jk * C_prim[j]
    ///                       + sum_i stoich[i][j] * stoich[i][k] * C_sec[i]
    /// ```
    ///
    /// Each iteration solves the dense `Nc x Nc` system `J * dx = -f` with the
    /// foam-basic-lib [`SquareMatrix`] LU solver, damps `dx` so its largest
    /// component is at most `4.0` (a factor `e^4` in concentration), and updates
    /// `x += dx`. Convergence is `|f_j| <= 1e-12 * (|T[j]| + 1e-30)` for all `j`.
    ///
    /// # Parameters
    ///
    /// - `totals`: total component concentrations `T[j]`, length `Nc`, mol/L.
    ///   Each must be **strictly positive** — a non-positive component total has
    ///   no positive-concentration solution in the ideal model (the log-Newton
    ///   iterate would be `ln` of a non-positive number).
    /// - `initial`: optional starting guess for the free primary
    ///   concentrations, length `Nc`, all strictly positive. When `None`, the
    ///   solver starts from `C_prim[j] = T[j]` (exact when `Ns = 0`, and a good
    ///   guess when secondary complexation is weak).
    ///
    /// # Errors
    ///
    /// - [`PflotranError::InvalidInput`] if `totals` (or a provided `initial`)
    ///   has the wrong length, or contains a non-finite or non-positive value.
    /// - [`PflotranError::Convergence`] if Newton fails to reach the tolerance
    ///   within the iteration budget, or if a Jacobian solve fails.
    pub fn speciate(
        &self,
        totals: &[f64],
        initial: Option<&[f64]>,
    ) -> Result<Speciation, PflotranError> {
        let nc = self.n_primary;

        if totals.len() != nc {
            return Err(PflotranError::InvalidInput(format!(
                "totals has length {} but there are {nc} primary species",
                totals.len()
            )));
        }
        for (j, t) in totals.iter().enumerate() {
            if !t.is_finite() || *t <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "totals[{j}] = {t} must be finite and strictly positive"
                )));
            }
        }

        // Initial log-primary iterate x = ln(C_prim).
        let mut x = vec![0.0_f64; nc];
        match initial {
            Some(guess) => {
                if guess.len() != nc {
                    return Err(PflotranError::InvalidInput(format!(
                        "initial guess has length {} but there are {nc} primary species",
                        guess.len()
                    )));
                }
                for (j, g) in guess.iter().enumerate() {
                    if !g.is_finite() || *g <= 0.0 {
                        return Err(PflotranError::InvalidInput(format!(
                            "initial[{j}] = {g} must be finite and strictly positive"
                        )));
                    }
                    x[j] = g.ln();
                }
            }
            None => {
                for j in 0..nc {
                    x[j] = totals[j].ln();
                }
            }
        }

        let mut iterations = 0;
        for iter in 1..=MAX_ITERS {
            iterations = iter;

            let c_prim: Vec<f64> = x.iter().map(|xj| xj.exp()).collect();
            let c_sec = self.secondary_from_log_primary(&x);

            // Residual f_j = C_prim[j] + sum_i stoich[i][j] C_sec[i] - T[j].
            let mut f = vec![0.0_f64; nc];
            for j in 0..nc {
                let mut acc = c_prim[j];
                for i in 0..self.n_secondary {
                    acc += self.stoich[i][j] * c_sec[i];
                }
                f[j] = acc - totals[j];
            }

            // Convergence test on the scaled residual.
            let converged = (0..nc).all(|j| f[j].abs() <= RESIDUAL_TOL * (totals[j].abs() + ABS_FLOOR));
            if converged {
                return Ok(Speciation {
                    primary: c_prim,
                    secondary: c_sec,
                    iterations,
                });
            }

            // Jacobian J[j][k] = delta_jk C_prim[j] + sum_i stoich[i][j] stoich[i][k] C_sec[i].
            let mut jac = SquareMatrix::new(nc);
            for j in 0..nc {
                for k in 0..nc {
                    let mut v = 0.0;
                    for i in 0..self.n_secondary {
                        v += self.stoich[i][j] * self.stoich[i][k] * c_sec[i];
                    }
                    if j == k {
                        v += c_prim[j];
                    }
                    jac.set(j, k, v);
                }
            }

            // Solve J * dx = -f.
            let rhs: Vec<f64> = f.iter().map(|fj| -fj).collect();
            let mut dx = jac.solve(&rhs).map_err(|e| {
                PflotranError::Convergence(format!(
                    "speciation Jacobian solve failed at iteration {iter}: {e}"
                ))
            })?;

            // Damp the step so no |dx_k| exceeds MAX_LOG_STEP.
            let max_abs = dx.iter().fold(0.0_f64, |m, d| m.max(d.abs()));
            if max_abs > MAX_LOG_STEP {
                let scale = MAX_LOG_STEP / max_abs;
                for d in dx.iter_mut() {
                    *d *= scale;
                }
            }

            for j in 0..nc {
                x[j] = (x[j] + dx[j]).clamp(-X_CLAMP, X_CLAMP);
            }
        }

        Err(PflotranError::Convergence(format!(
            "aqueous speciation did not converge in {MAX_ITERS} Newton iterations"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Absolute/relative closeness helper.
    fn approx(a: f64, b: f64, rel: f64, abs: f64) -> bool {
        (a - b).abs() <= abs + rel * b.abs()
    }

    /// Ns = 0 (no secondary species): speciation must return the totals
    /// unchanged, since mass balance reduces to `C_prim[j] = T[j]`.
    #[test]
    fn no_secondary_returns_totals() {
        let net = ReactionNetwork::new(
            vec!["A".into(), "B".into()],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(net.n_primary(), 2);
        assert_eq!(net.n_secondary(), 0);

        let totals = [0.3, 1.7];
        let sol = net.speciate(&totals, None).unwrap();
        assert_eq!(sol.secondary.len(), 0);
        for j in 0..2 {
            assert!(
                approx(sol.primary[j], totals[j], 1e-12, 1e-15),
                "primary[{j}]={} != total {}",
                sol.primary[j],
                totals[j]
            );
        }
    }

    /// Weak monoprotic acid dissociation with an analytical closed form.
    ///
    /// # Methodology
    ///
    /// Primaries {H+, Ac-}, one secondary HAc formed by the association
    /// H+ + Ac- -> HAc, stoich = [1, 1], with association constant
    /// `K_assoc = [HAc]/([H+][Ac-]) = 1/Ka`, i.e. `log10_k = -log10(Ka) = pKa`.
    /// Choosing equal totals `T_H = T_Ac = C_total` makes the equilibrium
    /// symmetric, so at the solution `[H+] = [Ac-] = x` and
    /// `[HAc] = x^2 / Ka`. Mass balance then gives the closed-form quadratic
    ///
    ///   `x^2 / Ka + x - C_total = 0`  =>  `x = (-Ka + sqrt(Ka^2 + 4 Ka C_total)) / 2`.
    ///
    /// # Results (checked in-test)
    ///
    /// With acetic acid `Ka = 1.8e-5` (`pKa = 4.744727...`) and
    /// `C_total = 0.1 mol/L`, the quadratic gives `x = 1.3326...e-3 mol/L` and
    /// `[HAc] = C_total - x = 9.8667...e-2 mol/L`. The solver reproduces both to
    /// a relative `1e-8`, and mass balance / mass action hold to `1e-12`.
    #[test]
    fn weak_acid_dissociation_analytical() {
        let ka = 1.8e-5;
        let pka = -ka.log10();
        let c_total = 0.1;

        let net = ReactionNetwork::new(
            vec!["H+".into(), "Ac-".into()],
            vec!["HAc".into()],
            vec![vec![1.0, 1.0]],
            vec![pka], // log10 K_assoc = pKa
        )
        .unwrap();

        // Closed-form monoprotic-acid dissociation root.
        let x_analytic = (-ka + (ka * ka + 4.0 * ka * c_total).sqrt()) / 2.0;
        let hac_analytic = c_total - x_analytic; // = x^2 / Ka

        let sol = net.speciate(&[c_total, c_total], None).unwrap();

        assert!(
            approx(sol.primary[0], x_analytic, 1e-8, 0.0),
            "[H+]={} vs analytic {}",
            sol.primary[0],
            x_analytic
        );
        assert!(
            approx(sol.primary[1], x_analytic, 1e-8, 0.0),
            "[Ac-]={} vs analytic {}",
            sol.primary[1],
            x_analytic
        );
        assert!(
            approx(sol.secondary[0], hac_analytic, 1e-8, 0.0),
            "[HAc]={} vs analytic {}",
            sol.secondary[0],
            hac_analytic
        );

        // Mass balance: T[j] = C_prim[j] + sum_i nu[i][j] C_sec[i].
        for j in 0..2 {
            let bal = sol.primary[j] + 1.0 * sol.secondary[0];
            assert!(
                approx(bal, c_total, 1e-12, 1e-15),
                "mass balance component {j}: {bal} != {c_total}"
            );
        }
        // Mass action: C_sec = 10^{log10K} * prod C_prim^nu.
        let ma = 10f64.powf(pka) * sol.primary[0] * sol.primary[1];
        assert!(
            approx(sol.secondary[0], ma, 1e-12, 1e-18),
            "mass action: {} != {ma}",
            sol.secondary[0]
        );
        // All concentrations strictly positive.
        assert!(sol.primary.iter().all(|c| *c > 0.0));
        assert!(sol.secondary.iter().all(|c| *c > 0.0));
    }

    /// Asymmetric weak-acid case (T_H != T_Ac) with multiple secondaries: no
    /// closed form is asserted, but mass balance, mass action, and positivity
    /// must all hold for the converged state. Exercises the general Newton loop
    /// with a non-symmetric right-hand side and two secondary species.
    #[test]
    fn general_network_mass_balance_and_action() {
        // Primaries {H+, Ac-, CO3--}. Secondaries:
        //   HAc  = H+ + Ac-        stoich [1, 1, 0], log10K = 4.75
        //   HCO3-= H+ + CO3--      stoich [1, 0, 1], log10K = 10.33
        let net = ReactionNetwork::new(
            vec!["H+".into(), "Ac-".into(), "CO3--".into()],
            vec!["HAc".into(), "HCO3-".into()],
            vec![vec![1.0, 1.0, 0.0], vec![1.0, 0.0, 1.0]],
            vec![4.75, 10.33],
        )
        .unwrap();

        let totals = [5.0e-3, 2.0e-3, 1.0e-3];
        let sol = net.speciate(&totals, None).unwrap();

        // Mass balance for every component.
        for j in 0..3 {
            let mut bal = sol.primary[j];
            for i in 0..net.n_secondary() {
                bal += net.stoich[i][j] * sol.secondary[i];
            }
            assert!(
                approx(bal, totals[j], 1e-11, 1e-16),
                "mass balance component {j}: {bal} != {}",
                totals[j]
            );
        }
        // Mass action for every secondary.
        for i in 0..net.n_secondary() {
            let mut ma = 10f64.powf(net.log10_k[i]);
            for j in 0..3 {
                ma *= sol.primary[j].powf(net.stoich[i][j]);
            }
            assert!(
                approx(sol.secondary[i], ma, 1e-10, 1e-20),
                "mass action secondary {i}: {} != {ma}",
                sol.secondary[i]
            );
        }
        assert!(sol.primary.iter().all(|c| *c > 0.0));
        assert!(sol.secondary.iter().all(|c| *c > 0.0));
    }

    /// A user-supplied initial guess must reach the same converged answer as the
    /// default start (the fixed point is independent of the starting iterate).
    #[test]
    fn initial_guess_reaches_same_solution() {
        let net = ReactionNetwork::new(
            vec!["H+".into(), "Ac-".into()],
            vec!["HAc".into()],
            vec![vec![1.0, 1.0]],
            vec![4.75],
        )
        .unwrap();
        let totals = [0.05, 0.05];
        let a = net.speciate(&totals, None).unwrap();
        let b = net.speciate(&totals, Some(&[1e-3, 1e-3])).unwrap();
        for j in 0..2 {
            assert!(approx(a.primary[j], b.primary[j], 1e-10, 1e-18));
        }
    }

    /// Constructor and input validation: shape mismatches and bad totals are
    /// rejected with `InvalidInput` rather than panicking or producing garbage.
    #[test]
    fn validation_rejects_bad_shapes_and_totals() {
        // stoich row count != Ns
        assert!(ReactionNetwork::new(
            vec!["A".into()],
            vec!["S".into()],
            vec![], // 0 rows, need 1
            vec![0.0],
        )
        .is_err());

        // stoich row width != Nc
        assert!(ReactionNetwork::new(
            vec!["A".into(), "B".into()],
            vec!["S".into()],
            vec![vec![1.0]], // width 1, need 2
            vec![0.0],
        )
        .is_err());

        // log10_k length != Ns
        assert!(ReactionNetwork::new(
            vec!["A".into()],
            vec!["S".into()],
            vec![vec![1.0]],
            vec![], // need 1
        )
        .is_err());

        // empty primary
        assert!(ReactionNetwork::new(vec![], vec![], vec![], vec![]).is_err());

        // bad totals (non-positive / wrong length)
        let net = ReactionNetwork::new(vec!["A".into()], vec![], vec![], vec![]).unwrap();
        assert!(net.speciate(&[0.0], None).is_err());
        assert!(net.speciate(&[-1.0], None).is_err());
        assert!(net.speciate(&[1.0, 2.0], None).is_err());
        assert!(net.speciate(&[1.0], Some(&[0.0])).is_err());
    }
}
