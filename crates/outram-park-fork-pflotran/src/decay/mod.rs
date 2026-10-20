//! Radioactive decay chains with daughter ingrowth (bead op-v6s.15.3).
//!
//! This module advances a set of coupled radionuclide inventories forward in
//! time under first-order radioactive decay and the ingrowth of daughter
//! nuclides. It is the decay-source term used by radionuclide transport in the
//! OUTRAM PARK nuclear focus, mirroring PFLOTRAN's radioactive-decay reaction
//! ~~(`reaction_sandbox_radioactive_decay` / the `RADIOACTIVE_DECAY` block).~~
//! **CORRECTED 2026-10-20 (gh:#999)** — upstream has no
//! `reaction_sandbox_radioactive_decay`; its decay lives in the
//! `CHEMISTRY` `RADIOACTIVE_DECAY_REACTION` block (`RRadioactiveDecay`,
//! `src/pflotran/reaction.F90`) and the `UFD_DECAY` process model
//! (`src/pflotran/pm_ufd_decay.F90`). What each does is recorded under
//! "What upstream PFLOTRAN does" below.
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
//! # What upstream PFLOTRAN does (read 2026-10-20, gh:#999)
//!
//! Read at upstream commit `2af572b8759c34aacc2216f56a906637f90af03e`
//! (bitbucket `pflotran/pflotran`, `master`, fetched 2026-10-20; the crate's
//! recorded reference clone is `669fbce2`, `upstream_source/README.md`).
//! **Upstream never forms a matrix exponential.** It has three decay paths:
//!
//! 1. **`CHEMISTRY` / `RADIOACTIVE_DECAY_REACTION`** (`reaction.F90`,
//!    `RRadioactiveDecay`, ~line 5323): decay is a kinetic rate
//!    `rate = kf * (total moles in aqueous + gas + equilibrium-sorbed)` added
//!    to the reaction residual with its analytical Jacobian, and solved inside
//!    PFLOTRAN's Newton iteration — i.e. **backward Euler** (first order in
//!    `dt`), coupled with transport or operator-split from it depending on the
//!    reactive-transport mode.
//! 2. **`UFD_DECAY`, default** (`pm_ufd_decay.F90`, `PMUFDDecaySolveISPDIAtCell`,
//!    ~line 1868): a **3-generation analytical (Bateman) solution**, explicit in
//!    time, for multiple parents and grandparents with non-zero initial
//!    daughters (Mariner et al. 2016, SAND2016-9610R, §3.2.3). Exact for
//!    chains of depth <= 3 from a cell's current inventory; it truncates
//!    ingrowth through great-grandparents within one step.
//! 3. **`UFD_DECAY` with `IMPLICIT_SOLUTION`** (same routine, ~line 1931):
//!    **backward Euler** for any number of generations, solved by Newton in a
//!    log formulation (`solution = solution * exp(-rhs)`, update clipped to
//!    10), stopping on a relative-change tolerance, then floored at `1e-90`.
//!
//! So this module's exponential is **not** upstream's algorithm, and it was
//! not before this change either (the hand-written Taylor `matrix_exp` was not
//! upstream's). It is the exact solution of the same ODE, which upstream's
//! backward Euler approaches as `dt -> 0` and its 3-generation form matches
//! for shallow chains. **Upstream-fidelity options (backward Euler, the
//! 3-generation form) are not ported**; [`DecaySolver`] lists only what
//! exists, and the follow-up is gh:#1000. Do not
//! quote a result from here as "PFLOTRAN's" decay.
//!
//! # Numerics — general-chain integration
//!
//! ~~[`DecayChain::decay`] advances *any* network (linear, branching, or
//! converging) by forming `exp(A dt)` with **scaling-and-squaring driven by a
//! truncated Taylor series** (the workhorse "method 3" of Moler & Van Loan,
//! *Nineteen Dubious Ways to Compute the Exponential of a Matrix*, SIAM Review
//! 45(1) 2003). ... the unit tests confirm agreement with the closed-form
//! Bateman solution to `< 1e-6`.~~ **CORRECTED 2026-10-20 (gh:#999):** the
//! hand-written Taylor + scaling-and-squaring `matrix_exp` is removed. Its
//! "`~1e-12` relative or better ... across the stiff range" claim was never
//! measured, and it was false: measured on its last day against the two
//! comparison tests below, it agreed with the exact answer to 6.6e-15 and
//! 5.7e-14 of the inventory on the radon chain (2 h, 10 d) but only **3.2e-6**
//! on the stiff thorium-C branch (Po-212, `||A dt|| ~ 4.6e11`). CRAM48 gives
//! 5.7e-16 there. So the default's results moved, by up to ~3e-6 of the
//! inventory on stiff steps, when CRAM48 replaced it.
//!
//! [`DecayChain::decay`] advances *any* network (linear, branching, or
//! converging) with **PETIR's order-48 IPF CRAM**
//! ([`petir::depletion::cram48`], OpenMC's `CRAM48`), the default
//! [`DecaySolver::Cram48`]. CRAM is accurate to round-off for any step on a
//! decay matrix (eigenvalues on the negative real axis), however stiff; there
//! is no step-length or norm limit. Order 48 rather than 16: it is OpenMC's
//! default and the more accurate of the two PETIR carries; upstream PFLOTRAN
//! has no CRAM, so no order is "upstream's".
//!
//! [`DecaySolver::Expm`] is the explicit, independent cross-check:
//! [`petir::linalg::expm`], a port of GNU Octave's `expm.m` (degree-8 Padé,
//! LAPACK `DGEBAL` balancing, scaling and squaring). It is kept for the
//! comparison tests, not as a fallback; on very stiff steps its accuracy is
//! bounded only by `~2^s u` (`s` the number of squarings), which the tests
//! measure.
//!
//! [`DecayChain::bateman_linear`] provides the classic **Bateman (1910)**
//! closed form for the special case of a single linear chain
//! `0 -> 1 -> ... -> N-1` with unit branching, as the exact reference both
//! methods are judged against.
//!
//! # Verification (gh:#999)
//!
//! Two comparison tests, CRAM48 vs `expm` vs the exact answer, each with its
//! methodology, tolerance (fixed from the methods' known error before
//! running) and measured result in its doc comment:
//! `cram48_expm_and_bateman_agree_on_the_radon_chain` (linear, 5 members,
//! half-lives 3.1 min to 22.2 a) and
//! `cram48_expm_and_exact_agree_on_the_stiff_thorium_c_branch` (branching,
//! Po-212's 0.3 us against Pb-212's 10.6 h, `||A dt|| ~ 2e11`).
//!
//! # Provenance
//!
//! - Decay-chain / ingrowth theory: H. Bateman, "The solution of a system of
//!   differential equations occurring in the theory of radioactive
//!   transformations", *Proc. Cambridge Philos. Soc.* **15** (1910) 423–427.
//! - ~~Matrix-exponential method: C. Moler & C. Van Loan, SIAM Review **45**(1)
//!   (2003) 3–49.~~ (removed with `matrix_exp`, 2026-10-20.) CRAM: PETIR's
//!   port of OpenMC `openmc/deplete/cram.py` (see `petir::depletion`);
//!   `expm`: PETIR's port of GNU Octave `expm.m`.
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

use petir::mathf::RealMath;
use crate::error::PflotranError;

pub mod upstream;

/// Which method [`DecayChain::decay_with`] uses for `exp(A dt) C`.
///
/// **The default is [`DecaySolver::Cram48`]** (gh:#999). Neither is upstream
/// PFLOTRAN's algorithm; see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecaySolver {
    /// PETIR's order-48 IPF CRAM ([`petir::depletion::cram48`]), one substep.
    #[default]
    Cram48,
    /// PETIR's port of Octave's `expm` ([`petir::linalg::expm`]), applied to
    /// the vector. The independent cross-check; less accurate on very stiff
    /// steps (see the module docs).
    Expm,
    /// Upstream PFLOTRAN's `UFD_DECAY` default: the 3-generation explicit
    /// analytical Bateman solution (`pm_ufd_decay.F90`,
    /// `PMUFDDecaySolveISPDIAtCell`, lines 1868-1928; Mariner et al. 2016,
    /// SAND2016-9610R, §3.2.3), then the `1e-90` floor. Bit-identical to the
    /// compiled upstream routine. Exact for chains of depth <= 3; **ignores
    /// branching fractions** (as upstream does) and drops great-grandparent
    /// ingrowth within a step. Use to reproduce a `UFD_DECAY` run.
    UfdThreeGeneration,
    /// Upstream PFLOTRAN's `UFD_DECAY` with `IMPLICIT_SOLUTION`: backward
    /// Euler by a log-formulation Newton (relative change `< 1e-6`, update
    /// clipped to 10), then the `1e-90` floor (`pm_ufd_decay.F90` lines
    /// 1929-1994). Bit-identical to the compiled upstream routine built with
    /// scalar `exp`, within 1 ulp of a vectorised `-O2` build. Fails with
    /// [`PflotranError::Convergence`] where upstream would hang or abort (an
    /// exactly zero inventory). Use to reproduce a `UFD_DECAY
    /// IMPLICIT_SOLUTION` run.
    UfdImplicit,
}

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

    /// Each nuclide's `(daughter, branching)` list, in the order given: the
    /// `isotope_daughters` / `isotope_daughter_stoich` of upstream's UFD_DECAY.
    fn daughter_lists(&self) -> Vec<Vec<(usize, f64)>> {
        self.nuclides.iter().map(|n| n.daughters.clone()).collect()
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
    /// inventory unchanged. ~~The general-chain integrator (scaling-and-squaring
    /// with a Taylor series) is described in the [module documentation](self)
    /// and is accurate to near machine precision.~~ **Since 2026-10-20
    /// (gh:#999)** this is [`DecayChain::decay_with`] with the default
    /// [`DecaySolver::Cram48`]; see the [module documentation](self).
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `concentrations.len() != n_nuclides()`
    /// or if `dt` is negative or non-finite.
    pub fn decay(&self, concentrations: &mut [f64], dt: f64) -> Result<(), PflotranError> {
        self.decay_with(DecaySolver::default(), concentrations, dt)
    }

    /// [`DecayChain::decay`] with the method chosen explicitly.
    ///
    /// # Errors
    /// As [`DecayChain::decay`], plus [`PflotranError::Convergence`] if a CRAM
    /// pole solve or the `expm` Padé denominator is singular (not reachable for
    /// a valid decay matrix, but reported rather than panicked).
    pub fn decay_with(
        &self,
        solver: DecaySolver,
        concentrations: &mut [f64],
        dt: f64,
    ) -> Result<(), PflotranError> {
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

        let a = self.decay_matrix();
        let out = match solver {
            DecaySolver::Cram48 => {
                let mut d = petir::depletion::DepletionMatrix::zeros(n);
                d.as_mut_slice().copy_from_slice(&a);
                petir::depletion::cram48(&d, concentrations, dt).map_err(|e| {
                    PflotranError::Convergence(format!("CRAM48 decay solve failed: {e:?}"))
                })?
            }
            DecaySolver::Expm => {
                // Scale the decay matrix by dt: M0 = A * dt.
                let m0: Vec<f64> = a.iter().map(|v| v * dt).collect();
                let m = petir::linalg::Matrix::from_row_major(n, n, m0).map_err(|e| {
                    PflotranError::InvalidInput(format!("decay matrix shape: {e:?}"))
                })?;
                let e = petir::linalg::expm(&m).map_err(|e| {
                    PflotranError::Convergence(format!("expm decay solve failed: {e:?}"))
                })?;
                (0..n)
                    .map(|i| (0..n).map(|j| e.get(i, j) * concentrations[j]).sum())
                    .collect()
            }
            DecaySolver::UfdThreeGeneration => upstream::ufd_three_generation(
                &self.lambda,
                &upstream::parent_lists(&self.daughter_lists()),
                concentrations,
                dt,
            )?,
            DecaySolver::UfdImplicit => {
                upstream::ufd_implicit(&self.lambda, &self.daughter_lists(), concentrations, dt)?.0
            }
        };
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
                sum += (-lam[i] * t).r_exp() / denom;
            }

            result[member] = head_initial * prod_lambda * sum;
        }

        Ok(result)
    }
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

        let expected = n0 * (-lambda * dt).r_exp();
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
        let n1 = n0 * l0 / (l1 - l0) * ((-l0 * t).r_exp() - (-l1 * t).r_exp());
        assert!(
            (c[1] - n1).abs() <= 1e-6 * n1,
            "decay daughter {} vs analytic {}",
            c[1],
            n1
        );
        // Parent still follows simple exponential.
        let n0t = n0 * (-l0 * t).r_exp();
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

        let parent = n0 * (-lam * t).r_exp();
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
        let err = DecayChain::new(vec![
            nuc("A", 1.0, vec![(1, 0.7), (1, 0.5)]),
            nuc("B", 1.0, vec![]),
        ])
        .unwrap_err();
        assert!(matches!(err, PflotranError::InvalidInput(_)));
        // Out-of-range single fraction.
        assert!(DecayChain::new(vec![nuc("A", 1.0, vec![(0, 1.5)])]).is_err());
    }

    /// `[~, e] = log2(norm(A dt, Inf))`, `s = max(0, e)`: the number of
    /// squarings Octave's `expm` takes, for the a-priori `2^s u` bound.
    fn expm_squarings(chain: &DecayChain, dt: f64) -> i32 {
        let n = chain.n_nuclides();
        let a = chain.decay_matrix();
        let norm = inf_norm_any(&a, n) * dt;
        if norm > 0.0 {
            ((norm.log2().floor() as i32) + 1).clamp(0, 1023)
        } else {
            0
        }
    }

    fn inf_norm_any(a: &[f64], n: usize) -> f64 {
        (0..n)
            .map(|i| (0..n).map(|j| a[i * n + j].abs()).sum::<f64>())
            .fold(0.0, f64::max)
    }

    /// The a-priori expm tolerance, normwise: `max(1e-12, min(1, 10 * 2^s * u))`.
    fn expm_bound(s: i32) -> f64 {
        (10.0 * 2f64.powi(s) * (f64::EPSILON / 2.0))
            .min(1.0)
            .max(1e-12)
    }

    /// `max_i |x_i - y_i| / total`, the normwise error in inventory units.
    fn normwise(x: &[f64], y: &[f64], total: f64) -> f64 {
        x.iter()
            .zip(y)
            .map(|(a, b)| (a - b).abs() / total)
            .fold(0.0, f64::max)
    }

    const MIN: f64 = 60.0;
    const HOUR: f64 = 3600.0;
    const DAY: f64 = 86_400.0;
    const YEAR: f64 = 365.25 * DAY;

    /// **Comparison 1 (gh:#999): CRAM48 vs `expm` vs Bateman on the radon
    /// chain.**
    ///
    /// Methodology: Rn-222 (3.8235 d) -> Po-218 (3.098 min) -> Pb-214
    /// (26.8 min) -> Bi-214 (19.9 min) -> Pb-210 (22.2 a), unit branching, from
    /// 1000 units of Rn-222, at `t = 2 h` (transient: every member changing)
    /// and `t = 10 d` (Po-218 to Bi-214 in transient equilibrium with
    /// Rn-222). **Simplifications:** Po-214 (164 us) is folded into an
    /// instantaneous Bi-214 -> Pb-210 step, and Pb-210's own daughters
    /// leave the network; both change the chain, not the comparison, which is
    /// between methods on the same matrix. Reference: the exact
    /// [`DecayChain::bateman_linear`] (no near-equal decay constants: the
    /// closest pair, Pb-214 and Bi-214, differ by 35 %).
    ///
    /// Tolerances, fixed before running, normwise (`|diff| / 1000`):
    /// CRAM48 vs Bateman 1e-13 (CRAM48's rational error is below round-off,
    /// so this is round-off in 24 complex 5x5 solves plus Bateman's own);
    /// `expm` vs Bateman `max(1e-12, 10 * 2^s * u)`, the scaling-and-squaring
    /// bound, `s` computed per step (5 at 2 h, 12 at 10 d).
    ///
    /// Measured 2026-10-20, normwise against Bateman:
    ///
    /// | `t` | CRAM48 | `expm` (`s`, bound) | old Taylor `matrix_exp` (removed) |
    /// |---|---|---|---|
    /// | 2 h | 6.82e-16 | 5.68e-16 (5, 1.0e-12) | 6.59e-15 |
    /// | 10 d | 2.27e-16 | 5.43e-14 (12, 4.5e-12) | 5.67e-14 |
    ///
    /// The old column was measured in this same change (gh:#999), with the
    /// same chain and times, just before `matrix_exp` was deleted.
    #[test]
    fn cram48_expm_and_bateman_agree_on_the_radon_chain() {
        let chain = DecayChain::new(vec![
            nuc("Rn-222", 3.8235 * DAY, vec![(1, 1.0)]),
            nuc("Po-218", 3.098 * MIN, vec![(2, 1.0)]),
            nuc("Pb-214", 26.8 * MIN, vec![(3, 1.0)]),
            nuc("Bi-214", 19.9 * MIN, vec![(4, 1.0)]),
            nuc("Pb-210", 22.2 * YEAR, vec![]),
        ])
        .unwrap();
        let n0 = 1000.0;
        for t in [2.0 * HOUR, 10.0 * DAY] {
            let bat = chain.bateman_linear(n0, t).unwrap();
            let mut c = vec![n0, 0.0, 0.0, 0.0, 0.0];
            chain.decay_with(DecaySolver::Cram48, &mut c, t).unwrap();
            let mut x = vec![n0, 0.0, 0.0, 0.0, 0.0];
            chain.decay_with(DecaySolver::Expm, &mut x, t).unwrap();
            let s = expm_squarings(&chain, t);
            let (ec, ex) = (normwise(&c, &bat, n0), normwise(&x, &bat, n0));
            println!(
                "radon t = {t} s: CRAM48-Bateman {ec:.2e}, expm-Bateman {ex:.2e} (s = {s}, \
                 bound {:.1e})",
                expm_bound(s)
            );
            assert!(ec < 1e-13, "CRAM48 vs Bateman at {t} s: {ec}");
            assert!(ex <= expm_bound(s), "expm vs Bateman at {t} s: {ex}");
        }
    }

    /// **Comparison 2 (gh:#999): CRAM48 vs `expm` vs the exact answer on a
    /// stiff branching chain**, thorium C: Pb-212 (10.64 h) -> Bi-212
    /// (60.55 min) -> {Po-212 (0.299 us, 64.06 %), Tl-208 (3.053 min,
    /// 35.94 %)} -> Pb-208 (stable), from 1000 units of Pb-212, `t = 1 d`.
    ///
    /// Methodology: Po-212 keeps its real half-life, so
    /// `||A dt||_inf ~ 4.6e11` and `expm` needs ~39 squarings: the stiffness a
    /// real decay chain brings. The exact answer is by superposition: Pb-212
    /// and Bi-212 from the two-member Bateman; Po-212 and Tl-208 each the
    /// third member of a linear chain Pb-212 -> Bi-212 -> X
    /// ([`DecayChain::bateman_linear`]) times its branching fraction; Pb-208
    /// by conservation. No near-equal decay constants (closest pair differ
    /// 10x), so the closed form is well conditioned.
    ///
    /// Tolerances, fixed before running, normwise: CRAM48 vs exact 1e-13;
    /// `expm` vs exact `max(1e-12, 10 * 2^s * u)` (~3e-4 at `s` ~ 38; the
    /// a-priori bound is weak, the measurement is the point); atom balance
    /// of CRAM48 1e-13.
    ///
    /// Measured 2026-10-20, normwise against the exact answer: **CRAM48
    /// 5.68e-16** (atom balance 5.68e-16); **`expm` 4.53e-6** (`s = 38`,
    /// a-priori bound 3.1e-4: inside the bound, ten orders worse than CRAM);
    /// the removed Taylor `matrix_exp` **3.16e-6**. So on a stiff chain the
    /// default's answer moved by ~3e-6 of the inventory when CRAM48 replaced
    /// `matrix_exp`, towards the exact answer. This is the finding that
    /// justifies CRAM as the default: scaling and squaring, whichever
    /// approximant it squares, loses accuracy on a step that is 2^38 times
    /// the stiffest decay time.
    #[test]
    fn cram48_expm_and_exact_agree_on_the_stiff_thorium_c_branch() {
        let (b_po, b_tl) = (0.6406, 0.3594);
        let chain = DecayChain::new(vec![
            nuc("Pb-212", 10.64 * HOUR, vec![(1, 1.0)]),
            nuc("Bi-212", 60.55 * MIN, vec![(2, b_po), (3, b_tl)]),
            nuc("Po-212", 0.299e-6, vec![(4, 1.0)]),
            nuc("Tl-208", 3.053 * MIN, vec![(4, 1.0)]),
            nuc("Pb-208", f64::INFINITY, vec![]),
        ])
        .unwrap();
        let via = |hl: f64| {
            DecayChain::new(vec![
                nuc("Pb-212", 10.64 * HOUR, vec![(1, 1.0)]),
                nuc("Bi-212", 60.55 * MIN, vec![(2, 1.0)]),
                nuc("X", hl, vec![]),
            ])
            .unwrap()
        };
        let n0 = 1000.0;
        let t = DAY;
        let po = via(0.299e-6).bateman_linear(n0, t).unwrap();
        let tl = via(3.053 * MIN).bateman_linear(n0, t).unwrap();
        let (pb212, bi212) = (po[0], po[1]);
        let (po212, tl208) = (b_po * po[2], b_tl * tl[2]);
        let exact = [
            pb212,
            bi212,
            po212,
            tl208,
            n0 - pb212 - bi212 - po212 - tl208,
        ];

        let mut c = vec![n0, 0.0, 0.0, 0.0, 0.0];
        chain.decay_with(DecaySolver::Cram48, &mut c, t).unwrap();
        let mut x = vec![n0, 0.0, 0.0, 0.0, 0.0];
        chain.decay_with(DecaySolver::Expm, &mut x, t).unwrap();
        let s = expm_squarings(&chain, t);
        let (ec, ex) = (normwise(&c, &exact, n0), normwise(&x, &exact, n0));
        let bal = (c.iter().sum::<f64>() - n0).abs() / n0;
        println!(
            "thorium C: CRAM48-exact {ec:.2e} (balance {bal:.2e}), expm-exact {ex:.2e} \
             (s = {s}, bound {:.1e})",
            expm_bound(s)
        );
        assert!(tl208 / n0 > 1e-4, "Tl-208 share must be resolvable");
        assert!(ec < 1e-13, "CRAM48 vs exact {ec}");
        assert!(bal < 1e-13, "CRAM48 balance {bal}");
        assert!(ex <= expm_bound(s), "expm vs exact {ex}");
    }

    /// The default is CRAM48, pinned (root CLAUDE.md, "correct physics is the
    /// default" — here, "the accurate method is the default"): `decay` is bit
    /// for bit `decay_with(DecaySolver::Cram48, ..)`.
    #[test]
    fn cram48_is_the_default_solver() {
        assert_eq!(DecaySolver::default(), DecaySolver::Cram48);
        let chain =
            DecayChain::new(vec![nuc("A", 20.0, vec![(1, 1.0)]), nuc("B", 5.0, vec![])]).unwrap();
        let mut a = vec![3.0, 1.0];
        let mut b = a.clone();
        chain.decay(&mut a, 7.0).unwrap();
        chain.decay_with(DecaySolver::Cram48, &mut b, 7.0).unwrap();
        assert_eq!(a, b);
        let mut z = vec![3.0, 1.0];
        chain.decay_with(DecaySolver::Expm, &mut z, 0.0).unwrap();
        assert_eq!(z, vec![3.0, 1.0]);
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
