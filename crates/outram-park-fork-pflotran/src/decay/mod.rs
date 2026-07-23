//! Radioactive decay chains with daughter ingrowth (bead op-v6s.15.3).
//!
//! This module advances a set of coupled radionuclide inventories forward in
//! time under first-order radioactive decay and the ingrowth of daughter
//! nuclides. It is the decay-source term used by radionuclide transport in the
//! OUTRAM PARK nuclear focus, mirroring PFLOTRAN's radioactive-decay reaction
//! (`reaction_sandbox_radioactive_decay` / the `RADIOACTIVE_DECAY` block).
//!
//! # Physical model
//!
//! Each nuclide `i` decays with a **decay constant**
//!
//! ```text
//! lambda_i = ln(2) / half_life_i          (units: 1/s)
//! ```
//!
//! A *stable* nuclide has an infinite half-life and hence `lambda_i = 0`.
//! Nuclides are coupled by branching: a parent `p` decaying produces daughter
//! `i` with **branching fraction** `b_{p,i}` (moles of `i` created per mole of
//! `p` that decays; the fractions over all daughters of `p` sum to at most 1,
//! the remainder being decay to nuclides not tracked in this network). The
//! governing linear ODE system for the atom densities / concentrations `C_i`
//! (any consistent amount unit — atoms, mol, mol/L; the equations are linear
//! and unit-agnostic) is
//!
//! ```text
//! dC_i/dt = -lambda_i C_i + sum_{p -> i} b_{p,i} lambda_p C_p .
//! ```
//!
//! Collecting the coefficients into the **decay matrix** `A` (`A[i][i] =
//! -lambda_i`, `A[i][p] = b_{p,i} lambda_p`), this is `dC/dt = A C`, whose exact
//! solution over a step `dt` is the matrix exponential
//!
//! ```text
//! C(t + dt) = exp(A dt) C(t) .
//! ```
//!
//! ## Atom conservation
//!
//! Column `p` of `A` sums to `-lambda_p (1 - sum_i b_{p,i})`. When every parent
//! decays entirely within the tracked network (`sum_i b_{p,i} = 1`), every
//! column sums to zero, `exp(A dt)` is column-stochastic, and the total amount
//! `sum_i C_i` is conserved exactly (decay just moves atoms down the chain into
//! the stable end member). With branching loss (`sum_i b_{p,i} < 1`) the lost
//! fraction leaves the network and the total decreases accordingly — both cases
//! are handled by the same matrix exponential, so the book-keeping is automatic.
//!
//! # Numerics — general-chain integration
//!
//! [`DecayChain::decay`] advances *any* network (linear, branching, or
//! converging) by forming `exp(A dt)` with **scaling-and-squaring driven by a
//! truncated Taylor series** (the workhorse "method 3" of Moler & Van Loan,
//! *Nineteen Dubious Ways to Compute the Exponential of a Matrix*, SIAM Review
//! 45(1) 2003). The scaled matrix `M = A dt / 2^s` is chosen with `s` large
//! enough that its infinity-norm is `<= 0.5`; the Taylor series
//! `exp(M) = sum_k M^k / k!` then converges rapidly (terminated once a term is
//! negligible relative to the running sum, capped at 30 terms), and `exp(A dt)`
//! is recovered by squaring `s` times. For a norm `<= 0.5` and 30 Taylor terms
//! the truncation error is far below `f64` round-off, so the method is accurate
//! to near machine precision (`~1e-12` relative or better) across the stiff
//! range of decay constants a real chain spans; the unit tests confirm
//! agreement with the closed-form Bateman solution to `< 1e-6`.
//!
//! [`DecayChain::bateman_linear`] provides the classic **Bateman (1910)**
//! closed form for the special case of a single linear chain
//! `0 -> 1 -> ... -> N-1` with unit branching, as an independent analytical
//! cross-check of the general integrator.
//!
//! # Provenance
//!
//! - Decay-chain / ingrowth theory: H. Bateman, "The solution of a system of
//!   differential equations occurring in the theory of radioactive
//!   transformations", *Proc. Cambridge Philos. Soc.* **15** (1910) 423–427.
//! - Matrix-exponential method: C. Moler & C. Van Loan, SIAM Review **45**(1)
//!   (2003) 3–49.
//! - Modelled on PFLOTRAN's radioactive-decay reaction (US-DOE national-lab
//!   subsurface reactive-transport code; this crate is an independent fork —
//!   see the crate root and `NOTICE`).
//!
//! # Status
//!
//! **Verification-only, untrusted AI-generated draft** (per the workspace
//! `RESPONSIBLE_USE.md`). Verified against the analytical Bateman solution and
//! conservation laws in the unit tests below; **no human V&V** has been
//! performed and it has **not** been validated against a published PFLOTRAN
//! reference case. Not for nuclear facility operation, safeguards analysis, or
//! any safety-critical or licensing use — education, research, and V&V only.

use crate::error::PflotranError;

/// Natural logarithm of 2, used to convert a half-life to a decay constant.
const LN_2: f64 = std::f64::consts::LN_2;

/// One nuclide in a [`DecayChain`]: its half-life and the branchings to its
/// daughters.
///
/// The half-life is given in **seconds** and must be strictly positive; use
/// [`f64::INFINITY`] for a stable nuclide (decay constant `0`). Each entry of
/// `daughters` names a daughter by its index into the [`DecayChain`]'s nuclide
/// list together with the branching fraction (moles of daughter produced per
/// mole of this nuclide that decays). The branching fractions must lie in
/// `[0, 1]` and sum to at most `1` (any shortfall represents decay to nuclides
/// outside the tracked network).
#[derive(Debug, Clone)]
pub struct Nuclide {
    /// Human-readable nuclide label (e.g. `"U-238"`). Not interpreted by the
    /// solver; carried for reporting and debugging.
    pub name: String,
    /// Half-life in **seconds** (`> 0`; use [`f64::INFINITY`] for a stable
    /// nuclide, giving a decay constant of `0`).
    pub half_life_seconds: f64,
    /// Branchings to daughter nuclides: `(daughter index, branching fraction)`.
    /// Indices must be valid entries of the owning [`DecayChain`]; fractions
    /// must be in `[0, 1]` and sum to at most `1`.
    pub daughters: Vec<(usize, f64)>,
}

/// A decay network of `N` nuclides identified by their index (`0..N`).
///
/// Construct with [`DecayChain::new`], which validates the topology and
/// pre-computes each decay constant. Advance an inventory in place with
/// [`DecayChain::decay`], or evaluate the closed-form [`DecayChain::bateman_linear`]
/// for a linear chain.
///
/// See the [module documentation](self) for the physical model and numerics.
#[derive(Debug, Clone)]
pub struct DecayChain {
    /// The nuclides, indexed `0..N`.
    nuclides: Vec<Nuclide>,
    /// Decay constants `lambda_i = ln(2) / half_life_i` in `1/s` (`0` for a
    /// stable nuclide). Parallel to `nuclides`.
    lambda: Vec<f64>,
}

impl DecayChain {
    /// Build a decay chain from its nuclides, validating the topology.
    ///
    /// Validation (each failure returns [`PflotranError::InvalidInput`]):
    /// - every half-life is a strictly positive number or [`f64::INFINITY`]
    ///   (`NaN` and values `<= 0` are rejected);
    /// - every daughter index refers to an existing nuclide (`< N`);
    /// - every branching fraction is a finite number in `[0, 1]`;
    /// - the branching fractions of each nuclide sum to at most `1`
    ///   (a small `1e-9` tolerance absorbs round-off).
    ///
    /// Decay constants `lambda_i = ln(2) / half_life_i` are pre-computed here
    /// (a stable nuclide yields `lambda_i = 0`).
    pub fn new(nuclides: Vec<Nuclide>) -> Result<Self, PflotranError> {
        let n = nuclides.len();
        if n == 0 {
            return Err(PflotranError::InvalidInput(
                "decay chain must contain at least one nuclide".to_string(),
            ));
        }

        let mut lambda = Vec::with_capacity(n);
        for (i, nuc) in nuclides.iter().enumerate() {
            let hl = nuc.half_life_seconds;
            if hl.is_nan() || hl <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "nuclide {} ('{}') has non-positive or NaN half-life {hl} s \
                     (must be > 0; use f64::INFINITY for a stable nuclide)",
                    i, nuc.name
                )));
            }

            let mut branch_sum = 0.0;
            for &(d, b) in &nuc.daughters {
                if d >= n {
                    return Err(PflotranError::InvalidInput(format!(
                        "nuclide {} ('{}') names daughter index {d}, but the \
                         chain only has {n} nuclides (indices 0..{n})",
                        i, nuc.name
                    )));
                }
                if !b.is_finite() || !(0.0..=1.0).contains(&b) {
                    return Err(PflotranError::InvalidInput(format!(
                        "nuclide {} ('{}') has branching fraction {b} to daughter \
                         {d}; must be a finite value in [0, 1]",
                        i, nuc.name
                    )));
                }
                branch_sum += b;
            }
            if branch_sum > 1.0 + 1e-9 {
                return Err(PflotranError::InvalidInput(format!(
                    "nuclide {} ('{}') branching fractions sum to {branch_sum} > 1",
                    i, nuc.name
                )));
            }

            // f64::INFINITY -> lambda = 0 (stable); otherwise ln2 / half-life.
            let lam = if hl.is_infinite() { 0.0 } else { LN_2 / hl };
            lambda.push(lam);
        }

        Ok(Self { nuclides, lambda })
    }

    /// Number of nuclides `N` in the chain.
    pub fn n_nuclides(&self) -> usize {
        self.nuclides.len()
    }

    /// Decay constant `lambda_i = ln(2) / half_life_i` in `1/s` for nuclide `i`
    /// (`0` for a stable nuclide).
    ///
    /// # Panics
    /// Panics if `i >= n_nuclides()`.
    pub fn decay_constant(&self, i: usize) -> f64 {
        self.lambda[i]
    }

    /// Nuclide `i` (its name, half-life, and daughter branchings).
    ///
    /// # Panics
    /// Panics if `i >= n_nuclides()`.
    pub fn nuclide(&self, i: usize) -> &Nuclide {
        &self.nuclides[i]
    }

    /// Assemble the decay matrix `A` (`dC/dt = A C`) as a dense row-major
    /// `N * N` buffer: `A[i][i] = -lambda_i` and `A[i][p] = b_{p,i} lambda_p`
    /// for every parent `p` with daughter `i`.
    fn decay_matrix(&self) -> Vec<f64> {
        let n = self.nuclides.len();
        let mut a = vec![0.0_f64; n * n];
        for (p, nuc) in self.nuclides.iter().enumerate() {
            a[p * n + p] -= self.lambda[p]; // diagonal loss term
            for &(d, b) in &nuc.daughters {
                a[d * n + p] += b * self.lambda[p]; // ingrowth into daughter d
            }
        }
        a
    }

    /// Advance an inventory over `dt` seconds **in place**, applying radioactive
    /// decay and daughter ingrowth: `concentrations <- exp(A dt) concentrations`.
    ///
    /// `concentrations[i]` is the amount of nuclide `i` (atoms, mol, mol/L — any
    /// consistent amount unit; the equations are linear). Its length must equal
    /// [`DecayChain::n_nuclides`]. `dt` must be non-negative; `dt = 0` leaves the
    /// inventory unchanged. The general-chain integrator (scaling-and-squaring
    /// with a Taylor series) is described in the [module documentation](self)
    /// and is accurate to near machine precision.
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `concentrations.len() != n_nuclides()`
    /// or if `dt` is negative or non-finite.
    pub fn decay(&self, concentrations: &mut [f64], dt: f64) -> Result<(), PflotranError> {
        let n = self.nuclides.len();
        if concentrations.len() != n {
            return Err(PflotranError::InvalidInput(format!(
                "concentrations length {} does not match n_nuclides {n}",
                concentrations.len()
            )));
        }
        if !dt.is_finite() || dt < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "decay dt must be a finite non-negative time step, got {dt} s"
            )));
        }
        if dt == 0.0 {
            return Ok(()); // exp(0) = identity
        }

        // Scale the decay matrix by dt: M0 = A * dt.
        let mut m0 = self.decay_matrix();
        for v in &mut m0 {
            *v *= dt;
        }

        let e = matrix_exp(&m0, n);
        let out = mat_vec(&e, concentrations, n);
        concentrations.copy_from_slice(&out);
        Ok(())
    }

    /// Bateman closed-form solution for a **linear** chain
    /// `0 -> 1 -> ... -> N-1` (each nuclide feeding the next with full branching
    /// `1.0`), given an initial amount `head_initial` of the head nuclide (index
    /// 0) only. Returns the amount of every member at time `t` (seconds).
    ///
    /// For distinct decay constants the Bateman (1910) solution is
    ///
    /// ```text
    /// N_n(t) = N0 * (prod_{i=0}^{n-1} lambda_i)
    ///               * sum_{i=0}^{n} exp(-lambda_i t)
    ///                 / prod_{j=0, j!=i}^{n} (lambda_j - lambda_i)
    /// ```
    ///
    /// which correctly accumulates a stable end member (`lambda_{N-1} = 0`).
    /// This is provided as an independent analytical cross-check of the general
    /// [`DecayChain::decay`] integrator.
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if the chain is **not** a simple linear
    /// chain (each nuclide `0..N-1` must have exactly one daughter, its
    /// immediate successor, with branching `1.0`; the last must have none), if
    /// `t` is negative or non-finite, or if two members up to `n` share a decay
    /// constant (the distinct-`lambda` Bateman form is singular there — split
    /// such a degenerate chain or use [`DecayChain::decay`]).
    pub fn bateman_linear(&self, head_initial: f64, t: f64) -> Result<Vec<f64>, PflotranError> {
        let n = self.nuclides.len();
        if !t.is_finite() || t < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "bateman_linear t must be finite and non-negative, got {t} s"
            )));
        }

        // Verify the strict linear topology 0 -> 1 -> ... -> N-1.
        for (i, nuc) in self.nuclides.iter().enumerate() {
            if i + 1 < n {
                let ok = nuc.daughters.len() == 1
                    && nuc.daughters[0].0 == i + 1
                    && (nuc.daughters[0].1 - 1.0).abs() < 1e-12;
                if !ok {
                    return Err(PflotranError::InvalidInput(format!(
                        "bateman_linear requires a linear chain: nuclide {i} must \
                         have exactly one daughter {} with branching 1.0",
                        i + 1
                    )));
                }
            } else if !nuc.daughters.is_empty() {
                return Err(PflotranError::InvalidInput(format!(
                    "bateman_linear requires a linear chain: the last nuclide {i} \
                     must have no daughters"
                )));
            }
        }

        let lam = &self.lambda;
        let mut result = vec![0.0_f64; n];
        for member in 0..n {
            // Product of parent decay constants prod_{i=0}^{member-1} lambda_i.
            let mut prod_lambda = 1.0;
            for &l in lam.iter().take(member) {
                prod_lambda *= l;
            }

            // Sum over i = 0..=member of exp(-lambda_i t) / prod_{j!=i}(lambda_j - lambda_i).
            let mut sum = 0.0;
            for i in 0..=member {
                let mut denom = 1.0;
                for j in 0..=member {
                    if j == i {
                        continue;
                    }
                    let diff = lam[j] - lam[i];
                    if diff.abs() < 1e-30 {
                        return Err(PflotranError::InvalidInput(format!(
                            "bateman_linear: nuclides {i} and {j} share a decay \
                             constant ({}); the distinct-lambda Bateman form is \
                             singular — use decay() instead",
                            lam[i]
                        )));
                    }
                    denom *= diff;
                }
                sum += (-lam[i] * t).exp() / denom;
            }

            result[member] = head_initial * prod_lambda * sum;
        }

        Ok(result)
    }
}

/// Infinity-norm (maximum absolute row sum) of a dense row-major `n * n` matrix.
fn inf_norm(a: &[f64], n: usize) -> f64 {
    let mut max = 0.0_f64;
    for i in 0..n {
        let mut row = 0.0;
        for j in 0..n {
            row += a[i * n + j].abs();
        }
        if row > max {
            max = row;
        }
    }
    max
}

/// Dense row-major `n * n` matrix product `c = a * b`.
fn mat_mul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = vec![0.0_f64; n * n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i * n + k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..n {
                c[i * n + j] += aik * b[k * n + j];
            }
        }
    }
    c
}

/// Dense row-major `n * n` matrix times an `n`-vector: `y = a * x`.
fn mat_vec(a: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    let mut y = vec![0.0_f64; n];
    for i in 0..n {
        let mut acc = 0.0;
        for j in 0..n {
            acc += a[i * n + j] * x[j];
        }
        y[i] = acc;
    }
    y
}

/// `n * n` identity matrix, row-major.
fn identity(n: usize) -> Vec<f64> {
    let mut m = vec![0.0_f64; n * n];
    for i in 0..n {
        m[i * n + i] = 1.0;
    }
    m
}

/// Matrix exponential `exp(A)` of a dense row-major `n * n` matrix, by
/// scaling-and-squaring with a truncated Taylor series (see the module docs).
///
/// The input `A` is scaled down to `M = A / 2^s` with `inf_norm(M) <= 0.5`, the
/// series `exp(M) = sum_k M^k / k!` is summed until a term is negligible (max 30
/// terms), and `exp(A) = (exp(M))^(2^s)` is recovered by repeated squaring.
fn matrix_exp(a: &[f64], n: usize) -> Vec<f64> {
    // Choose the scaling exponent s so that ||A|| / 2^s <= 0.5.
    let norm = inf_norm(a, n);
    let mut s = 0u32;
    if norm > 0.5 {
        // s = ceil(log2(norm / 0.5)); computed by doubling to avoid log edge cases.
        let mut scaled = norm;
        while scaled > 0.5 && s < 1023 {
            scaled *= 0.5;
            s += 1;
        }
    }

    let scale = 0.5_f64.powi(s as i32);
    let mut m = a.to_vec();
    for v in &mut m {
        *v *= scale;
    }

    // Taylor series: result = sum_{k>=0} M^k / k!, term_{k} = term_{k-1} * M / k.
    let mut result = identity(n);
    let mut term = identity(n);
    for k in 1..=30u32 {
        term = mat_mul(&term, &m, n);
        let inv_k = 1.0 / k as f64;
        for v in &mut term {
            *v *= inv_k;
        }
        for (r, t) in result.iter_mut().zip(term.iter()) {
            *r += *t;
        }
        // Stop once the added term is negligible relative to the running sum.
        if inf_norm(&term, n) <= f64::EPSILON * inf_norm(&result, n) {
            break;
        }
    }

    // Square s times: exp(A) = (exp(M))^(2^s).
    for _ in 0..s {
        result = mat_mul(&result, &result, n);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: an unstable nuclide with a single full-branching daughter.
    fn nuc(name: &str, hl: f64, daughters: Vec<(usize, f64)>) -> Nuclide {
        Nuclide {
            name: name.to_string(),
            half_life_seconds: hl,
            daughters,
        }
    }

    #[test]
    fn single_nuclide_matches_exponential_law() {
        // N(t) = N0 exp(-lambda t) for a lone decaying nuclide.
        let hl = 100.0; // s
        let chain = DecayChain::new(vec![nuc("A", hl, vec![])]).unwrap();
        let lambda = LN_2 / hl;

        let n0 = 1.0e6;
        let mut c = vec![n0];
        let dt = 37.0;
        chain.decay(&mut c, dt).unwrap();

        let expected = n0 * (-lambda * dt).exp();
        assert!(
            (c[0] - expected).abs() <= 1e-6 * expected,
            "got {}, expected {}",
            c[0],
            expected
        );
    }

    #[test]
    fn stable_nuclide_is_unchanged() {
        let chain = DecayChain::new(vec![nuc("Stable", f64::INFINITY, vec![])]).unwrap();
        assert_eq!(chain.decay_constant(0), 0.0);
        let mut c = vec![42.0];
        chain.decay(&mut c, 1.0e9).unwrap();
        assert_eq!(c[0], 42.0);
    }

    #[test]
    fn two_member_chain_matches_bateman_daughter() {
        // Parent (index 0) -> Daughter (index 1), full branching.
        let hl0 = 50.0;
        let hl1 = 8.0;
        let chain = DecayChain::new(vec![
            nuc("Parent", hl0, vec![(1, 1.0)]),
            nuc("Daughter", hl1, vec![]),
        ])
        .unwrap();
        let l0 = LN_2 / hl0;
        let l1 = LN_2 / hl1;

        let n0 = 1.0e3;
        let t = 30.0;
        let mut c = vec![n0, 0.0];
        chain.decay(&mut c, t).unwrap();

        // Analytical Bateman daughter amount.
        let n1 = n0 * l0 / (l1 - l0) * ((-l0 * t).exp() - (-l1 * t).exp());
        assert!(
            (c[1] - n1).abs() <= 1e-6 * n1,
            "decay daughter {} vs analytic {}",
            c[1],
            n1
        );
        // Parent still follows simple exponential.
        let n0t = n0 * (-l0 * t).exp();
        assert!((c[0] - n0t).abs() <= 1e-6 * n0t);

        // Cross-check against bateman_linear.
        let bat = chain.bateman_linear(n0, t).unwrap();
        assert!((bat[1] - n1).abs() <= 1e-9 * n1);
        assert!((c[0] - bat[0]).abs() <= 1e-6 * bat[0]);
        assert!((c[1] - bat[1]).abs() <= 1e-6 * bat[1]);
    }

    #[test]
    fn three_member_chain_matches_bateman() {
        // 0 -> 1 -> 2 (2 stable end member).
        let chain = DecayChain::new(vec![
            nuc("A", 20.0, vec![(1, 1.0)]),
            nuc("B", 5.0, vec![(2, 1.0)]),
            nuc("C", f64::INFINITY, vec![]),
        ])
        .unwrap();

        let n0 = 500.0;
        let t = 12.0;
        let mut c = vec![n0, 0.0, 0.0];
        chain.decay(&mut c, t).unwrap();

        let bat = chain.bateman_linear(n0, t).unwrap();
        for i in 0..3 {
            let tol = 1e-6 * bat[i].max(1.0);
            assert!(
                (c[i] - bat[i]).abs() <= tol,
                "member {i}: decay {} vs bateman {}",
                c[i],
                bat[i]
            );
        }
    }

    #[test]
    fn secular_equilibrium() {
        // Long-lived parent feeding a short-lived daughter: after many daughter
        // half-lives the daughter activity lambda1*N1 approaches the parent
        // activity lambda0*N0 (secular equilibrium).
        let hl0 = 1.0e9; // very long-lived parent
        let hl1 = 10.0; // short-lived daughter
        let chain = DecayChain::new(vec![
            nuc("Parent", hl0, vec![(1, 1.0)]),
            nuc("Daughter", hl1, vec![]),
        ])
        .unwrap();
        let l0 = LN_2 / hl0;
        let l1 = LN_2 / hl1;

        let n0 = 1.0e12;
        let mut c = vec![n0, 0.0];
        // Advance ~20 daughter half-lives so ingrowth saturates.
        chain.decay(&mut c, 200.0).unwrap();

        let parent_activity = l0 * c[0];
        let daughter_activity = l1 * c[1];
        let ratio = daughter_activity / parent_activity;
        assert!(
            (ratio - 1.0).abs() < 1e-4,
            "secular equilibrium activity ratio {ratio} not ~1"
        );
    }

    #[test]
    fn atom_conservation_no_branching_loss() {
        // Full-branching chain with a stable end member conserves total atoms.
        let chain = DecayChain::new(vec![
            nuc("A", 3.0, vec![(1, 1.0)]),
            nuc("B", 7.0, vec![(2, 1.0)]),
            nuc("C", f64::INFINITY, vec![]),
        ])
        .unwrap();

        let mut c = vec![100.0, 20.0, 5.0];
        let total0: f64 = c.iter().sum();
        chain.decay(&mut c, 15.0).unwrap();
        let total1: f64 = c.iter().sum();
        assert!(
            (total1 - total0).abs() <= 1e-9 * total0,
            "total atoms {total1} drifted from {total0}"
        );
    }

    #[test]
    fn branching_loss_accounted() {
        // Parent decays 60% into a tracked daughter and 40% out of the network.
        // The tracked total should fall by exactly the escaped fraction of the
        // decayed parent atoms.
        let hl = 10.0;
        let chain = DecayChain::new(vec![
            nuc("Parent", hl, vec![(1, 0.6)]),
            nuc("Daughter", f64::INFINITY, vec![]),
        ])
        .unwrap();
        let lam = LN_2 / hl;

        let n0 = 1000.0;
        let t = 25.0;
        let mut c = vec![n0, 0.0];
        chain.decay(&mut c, t).unwrap();

        let parent = n0 * (-lam * t).exp();
        let decayed = n0 - parent;
        let daughter = 0.6 * decayed; // 60% branch stays in-network
        assert!((c[0] - parent).abs() <= 1e-6 * parent);
        assert!((c[1] - daughter).abs() <= 1e-6 * daughter.max(1.0));

        let total = c[0] + c[1];
        let expected_total = n0 - 0.4 * decayed; // 40% escaped the network
        assert!((total - expected_total).abs() <= 1e-6 * expected_total);
    }

    #[test]
    fn converging_branching_chain_general_integrator() {
        // Two parents feeding a common daughter (a non-linear topology that
        // bateman_linear cannot handle) — exercises the general integrator and
        // checks atom conservation with full branching.
        let chain = DecayChain::new(vec![
            nuc("P1", 4.0, vec![(2, 1.0)]),
            nuc("P2", 9.0, vec![(2, 1.0)]),
            nuc("D", f64::INFINITY, vec![]),
        ])
        .unwrap();
        let mut c = vec![10.0, 20.0, 0.0];
        let total0: f64 = c.iter().sum();
        chain.decay(&mut c, 6.0).unwrap();
        let total1: f64 = c.iter().sum();
        assert!((total1 - total0).abs() <= 1e-9 * total0);
        // bateman_linear must refuse this non-linear chain.
        assert!(chain.bateman_linear(1.0, 1.0).is_err());
    }

    #[test]
    fn zero_dt_is_identity() {
        let chain = DecayChain::new(vec![nuc("A", 5.0, vec![])]).unwrap();
        let mut c = vec![7.0];
        chain.decay(&mut c, 0.0).unwrap();
        assert_eq!(c[0], 7.0);
    }

    #[test]
    fn constructor_rejects_bad_daughter_index() {
        let err = DecayChain::new(vec![nuc("A", 1.0, vec![(5, 1.0)])]).unwrap_err();
        assert!(matches!(err, PflotranError::InvalidInput(_)));
    }

    #[test]
    fn constructor_rejects_nonpositive_half_life() {
        assert!(DecayChain::new(vec![nuc("A", 0.0, vec![])]).is_err());
        assert!(DecayChain::new(vec![nuc("A", -1.0, vec![])]).is_err());
        assert!(DecayChain::new(vec![nuc("A", f64::NAN, vec![])]).is_err());
    }

    #[test]
    fn constructor_rejects_branchings_over_one() {
        let err =
            DecayChain::new(vec![nuc("A", 1.0, vec![(1, 0.7), (1, 0.5)]), nuc("B", 1.0, vec![])])
                .unwrap_err();
        assert!(matches!(err, PflotranError::InvalidInput(_)));
        // Out-of-range single fraction.
        assert!(DecayChain::new(vec![nuc("A", 1.0, vec![(0, 1.5)])]).is_err());
    }

    #[test]
    fn decay_rejects_length_mismatch_and_bad_dt() {
        let chain = DecayChain::new(vec![nuc("A", 1.0, vec![])]).unwrap();
        let mut c = vec![1.0, 2.0];
        assert!(chain.decay(&mut c, 1.0).is_err());
        let mut c1 = vec![1.0];
        assert!(chain.decay(&mut c1, -1.0).is_err());
        assert!(chain.decay(&mut c1, f64::INFINITY).is_err());
    }
}
