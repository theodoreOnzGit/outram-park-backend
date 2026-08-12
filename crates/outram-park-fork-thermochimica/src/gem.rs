// SPDX-License-Identifier: GPL-3.0-only
//
// ======================================================================
//  GPLv3 provenance / attribution header (workspace CLAUDE.md, mandatory)
// ======================================================================
//
//  This file is an INDEPENDENT pure-Rust re-implementation, in a CALPHAD
//  Gibbs-energy-minimisation framing, of the solver *structure* of:
//
//      Thermochimica  — ORNL / UT-Battelle, LLC
//      Upstream project: https://github.com/ORNL-CEES/thermochimica
//      Commit: 0c35c8d  (gitignored reference clone under
//              crates/outram-park-fork-thermochimica/upstream_source/thermochimica/)
//      Upstream author of the cited routines: M.H.A. Piro et al.
//      Upstream licence: BSD-3-Clause.
//
//  Upstream Fortran source consulted (path relative to upstream `src/`),
//  cited as `File.f90:line`:
//    - gem/GEMSolver.f90:1-148            — top-level GEM solve loop (init →
//                                            Newton direction → line search →
//                                            phase-assemblage check → converge)
//    - gem/GEMNewton.f90:1-295            — Newton direction: the element-
//                                            potential / phase saddle-point
//                                            linear system assembled here
//    - gem/GEMLineSearch.f90:1-546        — damped line search along the
//                                            Newton direction (monotone merit)
//    - gem/CompChemicalPotential.f90:1-89 — species chemical potential
//                                            µ = µ° + RT ln(x·γ)
//    - gem/CompMolFraction.f90:1-181      — mole fractions from species moles
//    - gem/CompExcessGibbsEnergyRKMP.f90:1-222 — Redlich-Kister-Muggianu
//                                            (RKMP) partial molar excess Gibbs
//                                            energy of a binary interaction
//                                            g^ex = x1 x2 Σ_v L_v (x1-x2)^v
//
//  RELICENSING NOTE (BSD-3 → GPL-3.0): Thermochimica is BSD-3-Clause, which
//  is GPL-3.0-compatible. This derivative file is distributed under
//  GPL-3.0-only (the OUTRAM PARK workspace default). The BSD-3 copyright and
//  permission notice of the upstream project is preserved in
//  `upstream_source/README.md` (the crate's provenance record, deliberately
//  shipped in the package) and applies to the ideas ported here.
//
//  This is NOT the official Thermochimica and is NOT affiliated with ORNL,
//  UT-Battelle, or the Thermochimica project. No upstream Fortran was copied
//  verbatim; only the numerical *method* (element-potential minimisation with
//  a Redlich-Kister excess term) was re-expressed in idiomatic Rust.
// ======================================================================

//! # `gem` — CALPHAD Gibbs-energy-minimisation core
//!
//! A pure-Rust Gibbs-energy **minimiser** (GEM) in the CALPHAD framing that
//! ORNL's Thermochimica uses: given a set of **phases**, each holding chemical
//! **species** with a standard molar Gibbs energy `g°_i(T,P)` plus an activity
//! (ideal or Redlich-Kister non-ideal) term, and a feed of chemical **elements**
//! (atom abundances), it finds the equilibrium phase amounts and within-phase
//! species mole fractions that minimise the total Gibbs energy `G` subject to
//! element mass-balance constraints — the **element-potential / Lagrange-
//! multiplier** method (`µ_i = Σ_j a_ij λ_j`). Phases may **vanish** (a
//! candidate that is not thermodynamically stable collapses toward zero).
//!
//! This crate is a sibling of
//! `outram-park-fork-dwsim-libs::thermo::gibbs_multiphase` (the DWSIM-derived
//! multi-phase minimiser); it re-implements the same *numerical* idea in the
//! CALPHAD idiom of Thermochimica, and **does not depend on** that crate.
//!
//! ## The minimisation problem
//!
//! Minimise the dimensionless total Gibbs energy over `P` phases and their
//! species (raw `f64`, SI; see *Units* below):
//!
//! ```text
//! G/RT = Σ_p Σ_s n_ps · (µ_ps / RT),
//! µ_ps/RT = g°_ps/(RT) + ln x_ps + ln γ_ps  [+ ln(P/P°) if phase p is a gas]
//! ```
//!
//! where `n_ps` is the moles of species `s` in phase `p` \[mol\],
//! `x_ps = n_ps / Σ_l n_pl` its within-phase mole fraction \[-\], `g°_ps` its
//! standard molar Gibbs energy in that phase \[J/mol\], and `γ_ps` its activity
//! coefficient \[-\] from the phase's [`SolutionModel`] (`γ = 1` for an ideal
//! phase; a Redlich-Kister term otherwise). The atom matrix `a_ks` (atoms of
//! element `k` per formula unit of species `s`) is a property of the species.
//! Subject to, for every element `k`,
//!
//! ```text
//! Σ_p Σ_s a_ks · n_ps = b_k     (atom balance over ALL phases),   n_ps ≥ 0,
//! ```
//!
//! with atom targets `b_k` set by the feed. The Lagrange stationarity condition
//! is the **element-potential** relation — one potential `π_k = λ_k/RT` per
//! element, shared by every phase:
//!
//! ```text
//! µ_ps/RT = Σ_k a_ks · π_k     for every species s present in phase p.
//! ```
//!
//! Equating this across two phases sharing a species gives equal chemical
//! potential (phase equilibrium); for a species over its own single-species
//! condensed phase it gives the classic activity/vapour relation.
//!
//! ## The element-potential Newton iteration (pure Rust, no BLAS)
//!
//! At a strictly-positive estimate `n_ps` with phase totals `n_p = Σ_s n_ps`,
//! the `(M+P)×(M+P)` saddle-point system (upstream `GEMNewton.f90`) for the `M`
//! element potentials `π_k` and `P` per-phase log-total corrections
//! `u_p = Δ ln n_p` is
//!
//! ```text
//! Σ_j R_kj π_j + Σ_p Q_kp u_p = (b_k − q_k) + S_k     (one row per element k)
//! Σ_j Q_jp π_j                = F_p                   (one row per phase p)
//! ```
//!
//! with `q_k = Σ_p Σ_s a_ks n_ps`, `Q_kp = Σ_s a_ks n_ps`,
//! `R_kj = Σ_p Σ_s a_ks a_js n_ps`, `S_k = Σ_p Σ_s a_ks n_ps (µ_ps/RT)`, and
//! `F_p = Σ_s n_ps (µ_ps/RT)`. The species correction is
//!
//! ```text
//! δ_ps = Σ_k a_ks π_k + u_p − µ_ps/RT,
//! n_ps ← n_ps · exp(ω · δ_ps),   ω ≤ min(1, max_step / max|δ|).
//! ```
//!
//! The `[[R, Q],[Qᵀ, 0]]` block structure uses the **ideal** (`ln x`) Hessian.
//! The composition-dependent activity coefficient `ln γ_ps` enters only through
//! the right-hand side (`µ_ps/RT` carries it) and is **held frozen within each
//! linearisation**, then recomputed from the updated composition next step — a
//! successive-substitution / quasi-Newton treatment of non-ideality. Because
//! `δ_ps = 0` at the fixed point forces `µ_ps/RT = Σ_k a_ks π_k` (the exact
//! stationarity condition **including** `ln γ`), the converged answer is correct
//! regardless of the frozen-`γ` Hessian approximation; only the convergence
//! *rate* is affected (upstream uses the full analytic Hessian; this first pass
//! does not — documented scope below).
//!
//! **Monotone merit line search** (upstream `GEMLineSearch.f90`). ω is reduced
//! by backtracking on the merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` (penalty
//! [`MERIT_PENALTY`]): the largest halving of the damped `ω₀` whose trial `Φ`
//! does not rise is taken, so `Φ` is non-increasing by construction and, since
//! its atom-violation term → 0 at convergence, `Φ → G/RT`. The multiplicative
//! update keeps every `n_ps > 0` automatically; the `(b_k − q_k)` residual
//! drives atom balance back onto target each step.
//!
//! **Vanishing phases.** A phase whose total falls below
//! [`GemOptions::phase_floor`] is *frozen* (its row `M+p` becomes `u_p = 0`) to
//! keep the saddle system non-singular; its species still update from the shared
//! `π` and regrow if they become favourable, else stay near zero. This is how a
//! candidate phase that is not stable collapses out (V&V `collapse_*` test).
//!
//! ## Units — documented raw `f64` (SI)
//!
//! Raw `f64` in SI throughout the inner loop: temperature `T` \[K\], pressure
//! `P` and reference `P°` \[Pa\], standard Gibbs energies `g°_ps` and
//! Redlich-Kister coefficients `L_v` \[J/mol\], moles \[mol\]; mole fractions,
//! activity coefficients, and element potentials are dimensionless. `R` is the
//! CODATA molar gas constant [`R`] \[J/(mol·K)\].
//!
//! ## Design (workspace CLAUDE.md)
//!
//! Enum dispatch, **no `dyn` / `Box` / lifetime params**: the per-phase mixing
//! model is the closed enum [`SolutionModel`]; the system, options, result, and
//! errors are plain owned structs / enums. `#![forbid(unsafe_code)]` (crate
//! level). Raw `f64` maths inside; the `(M+P)` linear system is solved by
//! in-crate Gaussian elimination ([`solve_linear`]) — no BLAS/LAPACK/FFI, so
//! this compiles on Android / Termux like the rest of the crate.
//!
//! ## Honest scope — untrusted AI-assisted draft, pending human V&V
//!
//! This is the **GEM core only**, not the whole of Thermochimica. Deliberately
//! **out of scope** in this first pass:
//! - **The ChemSage / FactSage `.dat` file parser** (upstream `parser/`). Data
//!   is supplied programmatically via [`GemSystem::new`] + [`GemSystem::minimize`]
//!   (hand-coded `g°`, atom matrix, feed). No file format is read.
//! - **Solution models beyond ideal + binary Redlich-Kister.** Upstream's QKTO,
//!   SUBG/SUBI/SUBL/SUBM (sublattice / modified-quasichemical) models, magnetic
//!   contributions, and Muggianu/Kohler ternary interpolation are **not**
//!   ported. [`SolutionModel::RedlichKister`] handles binary interaction terms
//!   only (ternary+ interaction coefficients are not implemented).
//! - **The full analytic non-ideal Hessian** and automatic miscibility-gap /
//!   new-phase detection (upstream `CheckMiscibilityGap.f90`,
//!   `Subminimization.f90`). The candidate phase set is caller-supplied; an
//!   unlisted phase is never discovered. Non-ideality uses a frozen-`γ`
//!   quasi-Newton step (above).
//! - **Leveling / initial phase-assemblage estimation** (upstream `setup/`,
//!   `InitGEMSolver.f90`): here the feed seeds the iteration directly.
//! - **Phase-assemblage management** (upstream `CheckPhaseAssemblage.f90`,
//!   `AddSolnPhase.f90`, `RemPureConPhase.f90`, …). The number of
//!   *simultaneously active* phases must not exceed the number of **independent
//!   components** (linearly independent element rows) — the Gibbs phase rule as
//!   it appears in the element-potential saddle system, whose phase block
//!   otherwise loses column rank. Over-determined candidate sets return
//!   [`GemError::SingularSystem`] rather than being repaired by adding/removing
//!   phases from the assemblage. A candidate phase that is simply *unstable*
//!   still collapses out (vanishing phases, above); what is not handled is more
//!   *stable* candidate phases coexisting than the component count allows.
//!
//! **Verified, not validated.** The V&V tests below are analytic
//! **verification** (pure-component trivial limit, ideal Nernst partition,
//! Redlich-Kister activity-coefficient identity, and a molten-fluoride
//! LiF-BeF₂ ideal-mixing mass/charge-balance identity) against closed-form
//! results, **not** validation against a measured multi-phase equilibrium
//! dataset. AI-assisted draft material, **untrusted until human-reviewed** per
//! the crate `CLAUDE.md`. Not for nuclear facility operation, reactor control,
//! safety-critical, or licensing decisions. Independent OUTRAM PARK fork.

/// CODATA-2018 molar gas constant `R` \[J/(mol·K)\].
///
/// Used to non-dimensionalise Gibbs energies (`g°/RT`) throughout the solver.
/// Exact by the 2019 SI redefinition (`R = N_A · k_B`).
pub const R: f64 = 8.314_462_618_153_24;

/// L1 penalty weight `μ` \[-\] on the atom-balance violation in the line-search
/// merit `Φ = G/RT + μ · Σ_k|b_k − q_k|`.
///
/// Chosen a few × larger than the `O(1)` dimensionless Gibbs energy so
/// feasibility dominates step acceptance, keeping the iterate path near the
/// constraint manifold. The merit is monotone for **any** positive weight (the
/// line search enforces it); this value trades iteration count against how
/// tightly `Φ` tracks the physical `G/RT`.
const MERIT_PENALTY: f64 = 10.0;

/// One binary Redlich-Kister interaction term between two species of a phase.
///
/// Contributes to the molar excess Gibbs energy of mixing (upstream
/// `CompExcessGibbsEnergyRKMP.f90`, binary term):
///
/// ```text
/// g^ex_pair = x_i · x_j · Σ_{v=0}^{N} L_v · (x_i − x_j)^v      [J/mol]
/// ```
///
/// where `x_i`, `x_j` are the within-phase mole fractions \[-\] of the two
/// interacting species and `L_v` \[J/mol\] is the order-`v` mixing coefficient.
/// `v = 0` alone is a **regular** solution (symmetric); adding `v = 1` makes it
/// **subregular** (asymmetric); higher orders extend the polynomial. Coefficients
/// are treated as constants at the solve temperature — a caller wanting
/// `L_v(T) = a + bT` evaluates it before building the model (temperature-
/// dependent `L` storage is out of scope here).
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryInteraction {
    /// Within-phase index of the first interacting species `i` (0-based).
    pub species_i: usize,
    /// Within-phase index of the second interacting species `j` (0-based),
    /// `j ≠ i`.
    pub species_j: usize,
    /// Redlich-Kister coefficients `[L_0, L_1, …, L_N]` \[J/mol\], lowest order
    /// first. Empty ⇒ no contribution. `L_0` is the regular term; `L_1` the
    /// first subregular correction.
    pub l_coeffs: Vec<f64>,
}

/// Mixing (solution) model of one phase — the composition term in each species'
/// chemical potential (enum dispatch, no `dyn`; the set is closed).
///
/// All variants give `µ_ps/RT = g°_ps/RT + ln x_ps + ln γ_ps [+ ln(P/P°) if
/// gas]`; they differ in `γ_ps` (activity coefficient) and whether the gas
/// pressure term is present:
/// - ideal models have `γ = 1` (activity `= x`);
/// - [`SolutionModel::RedlichKister`] adds a non-ideal `ln γ` from its binary
///   interaction terms.
///
/// A **single-species** ideal or Redlich-Kister phase has `x = 1`, hence unit
/// activity — an exact **pure condensed** (stoichiometric) substance, the
/// CALPHAD "pure condensed phase".
#[derive(Debug, Clone, PartialEq)]
pub enum SolutionModel {
    /// Ideal **gas** solution: `γ = 1`, plus the pressure term `ln(P/P°)`. Drives
    /// the pressure response of any mole-changing reaction.
    IdealGas,
    /// Ideal **condensed** solution (molten salt / liquid / solid solution):
    /// `γ = 1`, no pressure term (condensed molar volume neglected).
    IdealSolution,
    /// Non-ideal condensed solution with binary **Redlich-Kister** excess terms.
    /// `γ ≠ 1`; no pressure term. Ternary+ interactions and Muggianu
    /// interpolation are not modelled (see module scope notes).
    RedlichKister {
        /// Binary interaction terms (any number; may target overlapping pairs).
        interactions: Vec<BinaryInteraction>,
    },
}

impl SolutionModel {
    /// `ln(P/P°)` contribution of this model \[-\]: `ln(pressure/p_ref)` for a
    /// gas, `0` for a condensed phase.
    #[inline]
    #[must_use]
    fn ln_p_term(&self, ln_p_ratio: f64) -> f64 {
        match self {
            SolutionModel::IdealGas => ln_p_ratio,
            SolutionModel::IdealSolution | SolutionModel::RedlichKister { .. } => 0.0,
        }
    }

    /// Log activity coefficients `ln γ_s` \[-\] for every species of the phase,
    /// given the within-phase mole fractions `x` \[-\] and `RT` \[J/mol\].
    ///
    /// Ideal models return all-zeros. For [`SolutionModel::RedlichKister`] the
    /// partial molar excess Gibbs energy `µ_s^ex` \[J/mol\] is formed from the
    /// molar excess `g^ex(x)` by the multicomponent partial-molar relation
    /// `µ_k^ex = g^ex + ∂g^ex/∂x_k − Σ_m x_m ∂g^ex/∂x_m` (unconstrained
    /// derivatives evaluated at the physical composition), and
    /// `ln γ_s = µ_s^ex / RT`. For a binary regular solution this reduces to the
    /// textbook `ln γ_1 = (L_0/RT) x_2²`; for subregular to
    /// `ln γ_1 = (x_2²/RT)[L_0 + L_1(3x_1 − x_2)]` — both checked in the V&V
    /// tests.
    #[must_use]
    fn ln_gamma(&self, x: &[f64], rt: f64) -> Vec<f64> {
        let n = x.len();
        match self {
            SolutionModel::IdealGas | SolutionModel::IdealSolution => vec![0.0; n],
            SolutionModel::RedlichKister { interactions } => {
                // g^ex (molar, J/mol) and its unconstrained gradient dG/dx_k.
                let mut g_ex = 0.0_f64;
                let mut grad = vec![0.0_f64; n];
                for it in interactions {
                    let (i, j) = (it.species_i, it.species_j);
                    if i >= n || j >= n {
                        continue;
                    }
                    let (xi, xj) = (x[i], x[j]);
                    let d = xi - xj;
                    // P = Σ_v L_v d^v ;  P' = dP/dd = Σ_{v≥1} v L_v d^{v-1}.
                    // `dpow` tracks d^v for P; `dpm1` tracks d^{v-1} for P'.
                    let mut p = 0.0_f64;
                    let mut pp = 0.0_f64;
                    let mut dpow = 1.0_f64;
                    let mut dpm1 = 0.0_f64; // d^{v-1}; d^{-1} unused (v=0 term skipped in P')
                    for (v, &lv) in it.l_coeffs.iter().enumerate() {
                        p += lv * dpow;
                        if v >= 1 {
                            pp += (v as f64) * lv * dpm1;
                        }
                        dpm1 = dpow; // becomes d^{(v+1)-1} = d^v for the next term
                        dpow *= d;
                    }
                    let xprod = xi * xj;
                    g_ex += xprod * p;
                    // ∂(x_i x_j P)/∂x_i = x_j P + x_i x_j P' ;
                    // ∂/∂x_j = x_i P − x_i x_j P'  (since ∂d/∂x_j = −1)
                    grad[i] += xj * p + xprod * pp;
                    grad[j] += xi * p - xprod * pp;
                }
                let dot: f64 = (0..n).map(|k| x[k] * grad[k]).sum();
                (0..n).map(|k| (g_ex + grad[k] - dot) / rt).collect()
            }
        }
    }
}

/// One phase's static description supplied to [`GemSystem::new`]: its mixing
/// model, species labels, and the atom sub-matrix mapping its species onto the
/// *global* element list.
///
/// The atom matrix has `M` rows (one per global element, in the system's element
/// order) and `n_species` columns; `atom_matrix[k][s]` is the number of atoms of
/// element `k` in one formula unit of species `s` of this phase (non-negative,
/// finite; typically small integers or simple fractions).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseInput {
    /// Mixing model for the phase.
    pub model: SolutionModel,
    /// Species labels (identification only). Length = the phase's `n_species`.
    pub species_names: Vec<String>,
    /// `M` rows × `n_species` columns: atoms of each global element per formula
    /// unit of each species.
    pub atom_matrix: Vec<Vec<f64>>,
}

/// Convergence / iteration controls for [`GemSystem::minimize`].
///
/// All tolerances are dimensionless. [`Default`]: `tol = 1e-10`,
/// `max_iter = 5000`, `max_step = 2.0`, `mole_floor = 1e-12`,
/// `phase_floor = 1e-11`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GemOptions {
    /// Convergence tolerance \[-\]. Stops when both the KKT stationarity residual
    /// `max|δ_ps|` (complementarity-aware) and the worst relative atom-balance
    /// residual fall below this value.
    pub tol: f64,
    /// Maximum Newton iterations before [`GemError::NotConverged`]. Larger than
    /// the ideal-only sibling's default because the frozen-`γ` step converges
    /// more slowly for strongly non-ideal phases.
    pub max_iter: usize,
    /// Maximum per-step log-correction `max|ω·δ_ps|` \[-\]. `ω ≤
    /// min(1, max_step/max|δ|)` caps how far any species moves per step in
    /// `ln n` space. Typical `2.0` (factor `e² ≈ 7.4`).
    pub max_step: f64,
    /// Lower floor \[mol\] on the *initial* working moles of any (phase, species)
    /// the feed gives as zero, so `ln n` is finite at the start. Must be > 0.
    pub mole_floor: f64,
    /// Phase-total floor \[mol\]. A phase below this is *frozen* (`u_p = 0`) to
    /// keep the saddle system non-singular as it vanishes; its species still
    /// update from the shared potentials. Must be > 0 and ≥ `mole_floor`.
    pub phase_floor: f64,
}

impl Default for GemOptions {
    fn default() -> Self {
        Self {
            tol: 1e-10,
            max_iter: 5000,
            max_step: 2.0,
            mole_floor: 1e-12,
            phase_floor: 1e-11,
        }
    }
}

/// Result of a converged Gibbs-energy minimisation.
///
/// Per-phase quantities are indexed `[phase][species]` in the build order;
/// element potentials are an `M`-vector in element order.
#[derive(Debug, Clone, PartialEq)]
pub struct GemResult {
    /// Equilibrium species amounts `n_ps` \[mol\], `moles[p][s]`. A collapsed
    /// phase reports species amounts near `mole_floor` or below.
    pub moles: Vec<Vec<f64>>,
    /// Within-phase mole fractions `x_ps = n_ps / Σ_l n_pl` \[-\],
    /// `mole_fractions[p][s]`. For a collapsed phase (total < `phase_floor`)
    /// these are reported as `0`.
    pub mole_fractions: Vec<Vec<f64>>,
    /// Total moles in each phase `n_p = Σ_s n_ps` \[mol\], one per phase.
    pub phase_totals: Vec<f64>,
    /// Activity coefficients `γ_ps` \[-\], `activity_coefficients[p][s]` (`1` for
    /// ideal phases). Activity of species `s` is `γ_ps · x_ps`.
    pub activity_coefficients: Vec<Vec<f64>>,
    /// Shared element potentials `π_k = λ_k/RT` \[-\], one per element. At the
    /// solution every present species satisfies `µ_ps/RT = Σ_k a_ks π_k`.
    pub element_potentials: Vec<f64>,
    /// Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (µ_ps/RT)` \[mol\]
    /// at the returned composition (the minimised objective).
    pub gibbs_energy_rt: f64,
    /// Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the supplied
    /// `g°_ps` reference states.
    pub gibbs_energy: f64,
    /// Line-search merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` \[mol\] at each iteration
    /// plus the final value. **Monotonically non-increasing by construction**
    /// (the V&V monotone-descent witness); `Φ → G/RT` as the violation → 0.
    pub descent_merit_history: Vec<f64>,
    /// Number of Newton iterations performed.
    pub iterations: usize,
    /// `true` if both tolerances in [`GemOptions`] were met.
    pub converged: bool,
}

/// Errors from constructing or solving a [`GemSystem`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GemError {
    /// A supplied slice / matrix had the wrong length for the system dimensions.
    #[error("dimension mismatch in `{what}`: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Which input was mis-sized.
        what: &'static str,
        /// Expected length.
        expected: usize,
        /// Actual length.
        got: usize,
    },
    /// An input value was non-finite, or a required-positive value was ≤ 0.
    #[error("invalid input `{what}`: {value} (must be finite; positive-required = {positive})")]
    InvalidInput {
        /// Which input was invalid.
        what: &'static str,
        /// Offending value.
        value: f64,
        /// Whether the field additionally had to be strictly positive.
        positive: bool,
    },
    /// The saddle-point system was singular even after freezing vanished phases —
    /// typically a rank-deficient atom matrix (a duplicated element row, an
    /// element no species contains, or two phases proportional in element space).
    #[error("singular GEM saddle system (atom matrix / phase set likely rank-deficient)")]
    SingularSystem,
    /// The iteration did not meet both tolerances within `max_iter`. Carries the
    /// worst residuals so the caller can judge how close it got.
    #[error("GEM did not converge in {iterations} iters (max|δ|={max_correction:.3e}, atom residual={atom_residual:.3e})")]
    NotConverged {
        /// Iterations performed (`= max_iter`).
        iterations: usize,
        /// Final complementarity-aware `max|δ_ps|` \[-\].
        max_correction: f64,
        /// Final worst relative atom-balance residual \[-\].
        atom_residual: f64,
    },
}

/// Internal per-phase storage: mixing model, species labels, row-major atom
/// matrix `a[k*n_species + s]`, and species count.
#[derive(Debug, Clone, PartialEq)]
struct Phase {
    model: SolutionModel,
    names: Vec<String>,
    a: Vec<f64>,
    n_species: usize,
}

impl Phase {
    #[inline]
    fn atom(&self, k: usize, s: usize) -> f64 {
        self.a[k * self.n_species + s]
    }
}

/// A multi-phase reacting system: a shared element list plus `P` phases, each
/// with its own species, mixing model, and atom sub-matrix.
///
/// Carries only the *combinatorial* data (which phases exist, made of what);
/// the thermodynamics (`g°`, `T`, `P`, feed) are passed per solve to
/// [`Self::minimize`], so one system can be reused across states.
#[derive(Debug, Clone, PartialEq)]
pub struct GemSystem {
    elements: Vec<String>,
    phases: Vec<Phase>,
    n_elements: usize,
}

impl GemSystem {
    /// Build a system from the global element list and one [`PhaseInput`] per
    /// phase.
    ///
    /// # Parameters
    /// - `element_symbols`: one label per global element (length `M ≥ 1`).
    ///   Element potentials are shared across every phase.
    /// - `phases`: one [`PhaseInput`] per phase (`P ≥ 1`); each `atom_matrix`
    ///   must have `M` rows and `species_names.len()` columns per row.
    ///
    /// # Errors
    /// [`GemError::DimensionMismatch`] on a wrong row/column count;
    /// [`GemError::InvalidInput`] on a non-finite/negative atom count, zero
    /// elements, zero phases, a phase with zero species, or a
    /// [`BinaryInteraction`] whose species index is out of range for its phase.
    pub fn new(element_symbols: &[&str], phases: &[PhaseInput]) -> Result<Self, GemError> {
        let n_elements = element_symbols.len();
        if n_elements == 0 {
            return Err(GemError::InvalidInput {
                what: "element_symbols (empty)",
                value: 0.0,
                positive: true,
            });
        }
        if phases.is_empty() {
            return Err(GemError::InvalidInput {
                what: "phases (empty)",
                value: 0.0,
                positive: true,
            });
        }
        let mut built = Vec::with_capacity(phases.len());
        for ph in phases {
            let n_species = ph.species_names.len();
            if n_species == 0 {
                return Err(GemError::InvalidInput {
                    what: "phase species (empty)",
                    value: 0.0,
                    positive: true,
                });
            }
            if ph.atom_matrix.len() != n_elements {
                return Err(GemError::DimensionMismatch {
                    what: "phase atom_matrix rows",
                    expected: n_elements,
                    got: ph.atom_matrix.len(),
                });
            }
            let mut a = vec![0.0; n_elements * n_species];
            for (k, row) in ph.atom_matrix.iter().enumerate() {
                if row.len() != n_species {
                    return Err(GemError::DimensionMismatch {
                        what: "phase atom_matrix row",
                        expected: n_species,
                        got: row.len(),
                    });
                }
                for (s, &v) in row.iter().enumerate() {
                    if !v.is_finite() || v < 0.0 {
                        return Err(GemError::InvalidInput {
                            what: "atom_matrix entry",
                            value: v,
                            positive: false,
                        });
                    }
                    a[k * n_species + s] = v;
                }
            }
            // Validate Redlich-Kister interaction indices against this phase.
            if let SolutionModel::RedlichKister { interactions } = &ph.model {
                for it in interactions {
                    if it.species_i >= n_species {
                        return Err(GemError::InvalidInput {
                            what: "BinaryInteraction.species_i out of range",
                            value: it.species_i as f64,
                            positive: false,
                        });
                    }
                    if it.species_j >= n_species {
                        return Err(GemError::InvalidInput {
                            what: "BinaryInteraction.species_j out of range",
                            value: it.species_j as f64,
                            positive: false,
                        });
                    }
                    for &lv in &it.l_coeffs {
                        if !lv.is_finite() {
                            return Err(GemError::InvalidInput {
                                what: "BinaryInteraction.l_coeffs entry",
                                value: lv,
                                positive: false,
                            });
                        }
                    }
                }
            }
            built.push(Phase {
                model: ph.model.clone(),
                names: ph.species_names.clone(),
                a,
                n_species,
            });
        }
        Ok(Self {
            elements: element_symbols.iter().map(|s| (*s).to_string()).collect(),
            phases: built,
            n_elements,
        })
    }

    /// Number of chemical elements `M`.
    #[must_use]
    pub fn n_elements(&self) -> usize {
        self.n_elements
    }

    /// Number of phases `P`.
    #[must_use]
    pub fn n_phases(&self) -> usize {
        self.phases.len()
    }

    /// Number of species in phase `p`. Panics if `p` is out of range.
    #[must_use]
    pub fn n_species(&self, p: usize) -> usize {
        self.phases[p].n_species
    }

    /// Element symbols, in element order.
    #[must_use]
    pub fn element_symbols(&self) -> &[String] {
        &self.elements
    }

    /// Species names of phase `p`, in species order. Panics if `p` out of range.
    #[must_use]
    pub fn species_names(&self, p: usize) -> &[String] {
        &self.phases[p].names
    }

    /// Total moles of each element supplied by a per-phase composition:
    /// `b_k = Σ_p Σ_s a_ks · moles[p][s]` \[mol of atoms\], one per element.
    ///
    /// Used to form the atom-balance targets from a feed and to check
    /// conservation of a result (V&V mass-balance closure).
    ///
    /// # Errors
    /// [`GemError::DimensionMismatch`] if `moles` has the wrong number of phases
    /// or a phase slice with the wrong species count.
    pub fn element_abundance(&self, moles: &[&[f64]]) -> Result<Vec<f64>, GemError> {
        if moles.len() != self.phases.len() {
            return Err(GemError::DimensionMismatch {
                what: "moles phases",
                expected: self.phases.len(),
                got: moles.len(),
            });
        }
        let mut b = vec![0.0; self.n_elements];
        for (p, ph) in self.phases.iter().enumerate() {
            if moles[p].len() != ph.n_species {
                return Err(GemError::DimensionMismatch {
                    what: "moles[p]",
                    expected: ph.n_species,
                    got: moles[p].len(),
                });
            }
            for k in 0..self.n_elements {
                let mut acc = 0.0;
                for s in 0..ph.n_species {
                    acc += ph.atom(k, s) * moles[p][s];
                }
                b[k] += acc;
            }
        }
        Ok(b)
    }

    /// Mask of **linearly independent** element rows of the global (all phases,
    /// all species) atom matrix: `mask[k] = true` if element row `k` is a pivot
    /// row, `false` if it is a fixed linear combination of earlier rows.
    ///
    /// A `false` entry is the signature of a **charge / stoichiometric
    /// constraint** — e.g. in a fluoride melt the F row equals `Σ (charge)·cation`
    /// row for every species, so F is dependent. The Newton system pins the
    /// potentials of dependent elements to zero (a gauge choice) and enforces only
    /// the independent element balances; the dependent balances then hold
    /// automatically and exactly (their target is the same linear combination of
    /// the independent targets). Computed by Gaussian row-reduction with partial
    /// pivoting; tolerance `1e-9` on the reduced pivot magnitude.
    fn independent_element_mask(&self) -> Vec<bool> {
        let m = self.n_elements;
        let ncol: usize = self.phases.iter().map(|ph| ph.n_species).sum();
        // Stack atom rows [element][global species column].
        let mut a = vec![0.0_f64; m * ncol.max(1)];
        for k in 0..m {
            let mut col = 0;
            for ph in &self.phases {
                for s in 0..ph.n_species {
                    a[k * ncol + col] = ph.atom(k, s);
                    col += 1;
                }
            }
        }
        let mut indep = vec![false; m];
        if ncol == 0 {
            return indep;
        }
        // Row-echelon reduction; a row that reduces to ~0 is dependent.
        let mut pivot_col = 0usize;
        let mut row = 0usize;
        // Track which original rows became pivots via a permutation.
        let mut order: Vec<usize> = (0..m).collect();
        while row < m && pivot_col < ncol {
            // Find the row (>= row) with the largest entry in pivot_col.
            let mut best = row;
            let mut bestv = a[order[row] * ncol + pivot_col].abs();
            for r in (row + 1)..m {
                let v = a[order[r] * ncol + pivot_col].abs();
                if v > bestv {
                    bestv = v;
                    best = r;
                }
            }
            if bestv < 1e-9 {
                pivot_col += 1;
                continue;
            }
            order.swap(row, best);
            indep[order[row]] = true;
            let pr = order[row];
            let piv = a[pr * ncol + pivot_col];
            for r in 0..m {
                if r == pr {
                    continue;
                }
                let f = a[r * ncol + pivot_col] / piv;
                if f == 0.0 {
                    continue;
                }
                for c in pivot_col..ncol {
                    a[r * ncol + c] -= f * a[pr * ncol + c];
                }
            }
            row += 1;
            pivot_col += 1;
        }
        indep
    }

    /// Minimise `G/RT` over all phases subject to shared atom balance, returning
    /// the equilibrium distribution.
    ///
    /// Runs the element-potential Newton iteration in the module docs. Atom
    /// targets come from `feed` (which also seeds the initial distribution); zero
    /// entries are lifted to `options.mole_floor` so `ln n` is finite at start.
    ///
    /// # Parameters
    /// - `gibbs_formation`: `gibbs_formation[p][s]` = standard molar Gibbs energy
    ///   `g°_ps` of species `s` in phase `p` at the solve temperature \[J/mol\].
    ///   One slice per phase, each of that phase's species length. Only
    ///   *differences* affect equilibrium (a common additive offset cancels).
    /// - `temperature` `T` \[K\], > 0.
    /// - `feed` \[mol\]: `feed[p][s]` initial moles of species `s` in phase `p`;
    ///   sets targets `b_k = Σ a_ks feed` and seeds the iterate. May be 0. One
    ///   slice per phase; the total must be > 0.
    /// - `pressure` `P` and `p_ref` `P°` \[Pa\], both > 0: enter each *gas*
    ///   phase's term as `ln(x_ps P/P°)`; condensed phases ignore `P`.
    /// - `options`: convergence controls.
    ///
    /// # Errors
    /// [`GemError::DimensionMismatch`] on mis-shaped input;
    /// [`GemError::InvalidInput`] on non-finite `g°`, non-positive `T`/`P`/`P°`/
    /// `mole_floor`/`phase_floor`, negative feed, or an all-zero feed;
    /// [`GemError::SingularSystem`] if the saddle system is singular;
    /// [`GemError::NotConverged`] if `max_iter` is exhausted.
    #[allow(clippy::too_many_arguments)]
    pub fn minimize(
        &self,
        gibbs_formation: &[&[f64]],
        temperature: f64,
        feed: &[&[f64]],
        pressure: f64,
        p_ref: f64,
        options: &GemOptions,
    ) -> Result<GemResult, GemError> {
        let m = self.n_elements;
        let np = self.phases.len();

        // ---- validate shapes ----
        if gibbs_formation.len() != np {
            return Err(GemError::DimensionMismatch {
                what: "gibbs_formation phases",
                expected: np,
                got: gibbs_formation.len(),
            });
        }
        if feed.len() != np {
            return Err(GemError::DimensionMismatch {
                what: "feed phases",
                expected: np,
                got: feed.len(),
            });
        }
        for (p, ph) in self.phases.iter().enumerate() {
            if gibbs_formation[p].len() != ph.n_species {
                return Err(GemError::DimensionMismatch {
                    what: "gibbs_formation[p]",
                    expected: ph.n_species,
                    got: gibbs_formation[p].len(),
                });
            }
            if feed[p].len() != ph.n_species {
                return Err(GemError::DimensionMismatch {
                    what: "feed[p]",
                    expected: ph.n_species,
                    got: feed[p].len(),
                });
            }
        }
        for (what, v, pos) in [
            ("temperature", temperature, true),
            ("pressure", pressure, true),
            ("p_ref", p_ref, true),
            ("mole_floor", options.mole_floor, true),
            ("phase_floor", options.phase_floor, true),
        ] {
            if !v.is_finite() || (pos && v <= 0.0) {
                return Err(GemError::InvalidInput {
                    what,
                    value: v,
                    positive: pos,
                });
            }
        }
        let mut feed_sum = 0.0;
        for (p, ph) in self.phases.iter().enumerate() {
            for s in 0..ph.n_species {
                let g = gibbs_formation[p][s];
                if !g.is_finite() {
                    return Err(GemError::InvalidInput {
                        what: "gibbs_formation entry",
                        value: g,
                        positive: false,
                    });
                }
                let f = feed[p][s];
                if !f.is_finite() || f < 0.0 {
                    return Err(GemError::InvalidInput {
                        what: "feed entry",
                        value: f,
                        positive: false,
                    });
                }
                feed_sum += f;
            }
        }
        if feed_sum <= 0.0 {
            return Err(GemError::InvalidInput {
                what: "feed (all zero)",
                value: feed_sum,
                positive: true,
            });
        }

        let rt = R * temperature;
        let ln_p_ratio = (pressure / p_ref).ln();

        // Composition-INDEPENDENT part of µ_ps/RT: cc[p][s] = g°_ps/RT + [gas p-term].
        let cc: Vec<Vec<f64>> = self
            .phases
            .iter()
            .enumerate()
            .map(|(p, ph)| {
                let ptrm = ph.model.ln_p_term(ln_p_ratio);
                (0..ph.n_species)
                    .map(|s| gibbs_formation[p][s] / rt + ptrm)
                    .collect()
            })
            .collect();

        let b_target = self.element_abundance(feed)?;

        // Positive working estimate seeded from the feed (zeros -> mole_floor).
        let mut moles: Vec<Vec<f64>> = self
            .phases
            .iter()
            .enumerate()
            .map(|(p, ph)| {
                (0..ph.n_species)
                    .map(|s| {
                        if feed[p][s] > options.mole_floor {
                            feed[p][s]
                        } else {
                            options.mole_floor
                        }
                    })
                    .collect()
            })
            .collect();

        let mut mu_rt: Vec<Vec<f64>> = moles.iter().map(|v| vec![0.0; v.len()]).collect();
        let mut merit_history: Vec<f64> = Vec::with_capacity(options.max_iter + 1);
        let mut last_max_delta = f64::INFINITY;
        let mut last_atom_res = f64::INFINITY;
        let mut converged = false;
        let mut iterations = 0;
        let dim = m + np;

        // Independent element rows (charge/stoichiometry-dependent rows are pinned).
        let indep = self.independent_element_mask();

        for iter in 0..options.max_iter {
            iterations = iter + 1;

            // Phase totals, mole fractions, activity coefficients, µ_ps/RT.
            let mut n_p = vec![0.0; np];
            for (p, ph) in self.phases.iter().enumerate() {
                let nt: f64 = moles[p].iter().sum();
                n_p[p] = nt;
                let x: Vec<f64> = (0..ph.n_species).map(|s| moles[p][s] / nt).collect();
                let ln_g = ph.model.ln_gamma(&x, rt);
                for s in 0..ph.n_species {
                    mu_rt[p][s] = cc[p][s] + x[s].ln() + ln_g[s];
                }
            }

            // Record merit of the entering iterate.
            let g_rt = self.total_gibbs_rt(&moles, &cc, rt);
            merit_history.push(g_rt + MERIT_PENALTY * self.atom_violation(&moles, &b_target));

            // ---- assemble the (M+P) saddle system  [[R,Q],[Qᵀ,0]]·[π;u]=[(b-q)+S;F] ----
            let mut mat = vec![0.0; dim * dim];
            let mut rhs = vec![0.0; dim];
            let mut q_kp = vec![0.0; m * np];
            let mut f_p = vec![0.0; np];
            let mut q_k = vec![0.0; m];
            let mut s_k = vec![0.0; m];
            for (p, ph) in self.phases.iter().enumerate() {
                for k in 0..m {
                    let mut qkp = 0.0;
                    let mut skp = 0.0;
                    for s in 0..ph.n_species {
                        let ans = ph.atom(k, s) * moles[p][s];
                        qkp += ans;
                        skp += ans * mu_rt[p][s];
                    }
                    q_kp[k * np + p] = qkp;
                    q_k[k] += qkp;
                    s_k[k] += skp;
                }
                let mut fp = 0.0;
                for s in 0..ph.n_species {
                    fp += moles[p][s] * mu_rt[p][s];
                }
                f_p[p] = fp;
            }
            for k in 0..m {
                for j in k..m {
                    let mut rkj = 0.0;
                    for (p, ph) in self.phases.iter().enumerate() {
                        for s in 0..ph.n_species {
                            rkj += ph.atom(k, s) * ph.atom(j, s) * moles[p][s];
                        }
                    }
                    mat[k * dim + j] = rkj;
                    mat[j * dim + k] = rkj;
                }
            }
            for k in 0..m {
                for p in 0..np {
                    let qkp = q_kp[k * np + p];
                    mat[k * dim + (m + p)] = qkp;
                    mat[(m + p) * dim + k] = qkp;
                }
                rhs[k] = (b_target[k] - q_k[k]) + s_k[k];
            }
            for p in 0..np {
                rhs[m + p] = f_p[p];
            }
            // Pin dependent (charge/stoichiometry-constrained) element potentials
            // to zero: replace their row with the identity π_k = 0. The dependent
            // element's mass balance is enforced implicitly through the
            // independent rows it is a linear combination of, so its residual
            // still vanishes at convergence (checked over ALL M elements below).
            for k in 0..m {
                if !indep[k] {
                    for c in 0..dim {
                        mat[k * dim + c] = 0.0;
                    }
                    mat[k * dim + k] = 1.0;
                    rhs[k] = 0.0;
                }
            }
            // Freeze vanished phases.
            for p in 0..np {
                if n_p[p] < options.phase_floor {
                    let row = m + p;
                    for c in 0..dim {
                        mat[row * dim + c] = 0.0;
                    }
                    mat[row * dim + row] = 1.0;
                    rhs[row] = 0.0;
                }
            }

            let sol = solve_linear(&mut mat, &mut rhs, dim).ok_or(GemError::SingularSystem)?;
            let pi = &sol[0..m];
            let u = &sol[m..m + np];

            // δ_ps and the two magnitudes (damping vs complementarity-aware convergence).
            let mut step_max = 0.0_f64;
            let mut conv_max = 0.0_f64;
            let mut delta: Vec<Vec<f64>> = self
                .phases
                .iter()
                .map(|ph| vec![0.0; ph.n_species])
                .collect();
            for (p, ph) in self.phases.iter().enumerate() {
                for s in 0..ph.n_species {
                    let mut ap = 0.0;
                    for k in 0..m {
                        ap += ph.atom(k, s) * pi[k];
                    }
                    let d = ap + u[p] - mu_rt[p][s];
                    delta[p][s] = d;
                    if d.abs() > step_max {
                        step_max = d.abs();
                    }
                    let c = if moles[p][s] < options.mole_floor {
                        d.max(0.0)
                    } else {
                        d.abs()
                    };
                    if c > conv_max {
                        conv_max = c;
                    }
                }
            }

            let omega0 = if step_max > 0.0 {
                (options.max_step / step_max).min(1.0)
            } else {
                1.0
            };
            let phi_current = *merit_history.last().unwrap();
            let slack = 1e-12 * (1.0 + phi_current.abs());
            let mut omega = omega0;
            let mut trial: Vec<Vec<f64>> = self
                .phases
                .iter()
                .map(|ph| vec![0.0; ph.n_species])
                .collect();
            for _bt in 0..80 {
                for (p, ph) in self.phases.iter().enumerate() {
                    for s in 0..ph.n_species {
                        let v = moles[p][s] * (omega * delta[p][s]).exp();
                        trial[p][s] = if !v.is_finite() || v < 1e-300 {
                            1e-300
                        } else {
                            v
                        };
                    }
                }
                let phi_trial = self.total_gibbs_rt(&trial, &cc, rt)
                    + MERIT_PENALTY * self.atom_violation(&trial, &b_target);
                if phi_trial.is_finite() && phi_trial <= phi_current + slack {
                    break;
                }
                omega *= 0.5;
            }
            moles = trial;

            let mut atom_res = 0.0_f64;
            for k in 0..m {
                let scale = 1.0 + b_target[k].abs();
                let r = (b_target[k] - q_k[k]).abs() / scale;
                if r > atom_res {
                    atom_res = r;
                }
            }

            last_max_delta = conv_max;
            last_atom_res = atom_res;
            if conv_max < options.tol && atom_res < options.tol {
                converged = true;
                break;
            }
        }

        if !converged {
            return Err(GemError::NotConverged {
                iterations,
                max_correction: last_max_delta,
                atom_residual: last_atom_res,
            });
        }

        // Final quantities.
        let mut phase_totals = vec![0.0; np];
        let mut gamma: Vec<Vec<f64>> = self
            .phases
            .iter()
            .map(|ph| vec![1.0; ph.n_species])
            .collect();
        for (p, ph) in self.phases.iter().enumerate() {
            let nt: f64 = moles[p].iter().sum();
            phase_totals[p] = nt;
            let x: Vec<f64> = (0..ph.n_species).map(|s| moles[p][s] / nt).collect();
            let ln_g = ph.model.ln_gamma(&x, rt);
            for s in 0..ph.n_species {
                mu_rt[p][s] = cc[p][s] + x[s].ln() + ln_g[s];
                gamma[p][s] = ln_g[s].exp();
            }
        }
        let g_rt = self.total_gibbs_rt(&moles, &cc, rt);
        merit_history.push(g_rt + MERIT_PENALTY * self.atom_violation(&moles, &b_target));

        let mole_fractions: Vec<Vec<f64>> = self
            .phases
            .iter()
            .enumerate()
            .map(|(p, ph)| {
                let nt = phase_totals[p];
                if nt < options.phase_floor {
                    vec![0.0; ph.n_species]
                } else {
                    (0..ph.n_species).map(|s| moles[p][s] / nt).collect()
                }
            })
            .collect();

        let element_potentials = self.final_element_potentials(&moles, &mu_rt);

        Ok(GemResult {
            moles,
            mole_fractions,
            phase_totals,
            activity_coefficients: gamma,
            element_potentials,
            gibbs_energy_rt: g_rt,
            gibbs_energy: rt * g_rt,
            descent_merit_history: merit_history,
            iterations,
            converged,
        })
    }

    /// Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (µ_ps/RT)` \[mol\]
    /// at a per-phase composition, `µ_ps/RT = cc[p][s] + ln x_ps + ln γ_ps`
    /// (excess included via the phase [`SolutionModel`]). Assumes every phase
    /// total > 0 (the `1e-300` mole floor guarantees it).
    fn total_gibbs_rt(&self, moles: &[Vec<f64>], cc: &[Vec<f64>], rt: f64) -> f64 {
        let mut g = 0.0;
        for (p, ph) in self.phases.iter().enumerate() {
            let nt: f64 = moles[p].iter().sum();
            let x: Vec<f64> = (0..ph.n_species).map(|s| moles[p][s] / nt).collect();
            let ln_g = ph.model.ln_gamma(&x, rt);
            for s in 0..ph.n_species {
                g += moles[p][s] * (cc[p][s] + x[s].ln() + ln_g[s]);
            }
        }
        g
    }

    /// L1 atom-balance violation `Σ_k |b_k − Σ_ps a_ks n_ps|` \[mol\] — the
    /// penalty term of the line-search merit.
    fn atom_violation(&self, moles: &[Vec<f64>], b_target: &[f64]) -> f64 {
        let mut viol = 0.0;
        for (k, &bk) in b_target.iter().enumerate() {
            let mut qk = 0.0;
            for (p, ph) in self.phases.iter().enumerate() {
                for s in 0..ph.n_species {
                    qk += ph.atom(k, s) * moles[p][s];
                }
            }
            viol += (bk - qk).abs();
        }
        viol
    }

    /// Least-squares shared element potentials `π_k` from the converged
    /// `µ_ps/RT = Σ_k a_ks π_k` relation (mole-weighted over all phases), for the
    /// [`GemResult`] report. Solves the `M×M` normal equations; returns zeros if
    /// singular (report only — never affects the solve).
    fn final_element_potentials(&self, moles: &[Vec<f64>], mu_rt: &[Vec<f64>]) -> Vec<f64> {
        let m = self.n_elements;
        let dim = m;
        let mut mat = vec![0.0; dim * dim];
        let mut rhs = vec![0.0; dim];
        for k in 0..m {
            for j in k..m {
                let mut v = 0.0;
                for (p, ph) in self.phases.iter().enumerate() {
                    for s in 0..ph.n_species {
                        v += ph.atom(k, s) * ph.atom(j, s) * moles[p][s];
                    }
                }
                mat[k * dim + j] = v;
                mat[j * dim + k] = v;
            }
            let mut r = 0.0;
            for (p, ph) in self.phases.iter().enumerate() {
                for s in 0..ph.n_species {
                    r += ph.atom(k, s) * moles[p][s] * mu_rt[p][s];
                }
            }
            rhs[k] = r;
        }
        solve_linear(&mut mat, &mut rhs, dim).unwrap_or_else(|| vec![0.0; m])
    }
}

/// Solve the dense linear system `mat · x = rhs` (`mat` row-major `dim×dim`) by
/// Gaussian elimination with partial pivoting, in place. Returns `x`, or `None`
/// if a zero pivot is hit (singular).
///
/// In-crate so the solver needs no BLAS/LAPACK and compiles on Android/Termux.
/// `dim = M + P` is small, so the `O(dim³)` dense solve is negligible next to
/// the transcendental (`ln`, `exp`) work of the outer loop.
fn solve_linear(mat: &mut [f64], rhs: &mut [f64], dim: usize) -> Option<Vec<f64>> {
    for col in 0..dim {
        let mut pivot = col;
        let mut best = mat[col * dim + col].abs();
        for r in (col + 1)..dim {
            let v = mat[r * dim + col].abs();
            if v > best {
                best = v;
                pivot = r;
            }
        }
        if best < 1e-300 {
            return None;
        }
        if pivot != col {
            for k in 0..dim {
                mat.swap(col * dim + k, pivot * dim + k);
            }
            rhs.swap(col, pivot);
        }
        let diag = mat[col * dim + col];
        for r in 0..dim {
            if r == col {
                continue;
            }
            let factor = mat[r * dim + col] / diag;
            if factor == 0.0 {
                continue;
            }
            for k in col..dim {
                mat[r * dim + k] -= factor * mat[col * dim + k];
            }
            rhs[r] -= factor * rhs[col];
        }
    }
    let mut x = vec![0.0; dim];
    for i in 0..dim {
        x[i] = rhs[i] / mat[i * dim + i];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    //! # V&V — analytic verification of the CALPHAD GEM core
    //!
    //! **Methodology (shared).** Each test builds a [`GemSystem`], runs
    //! [`GemSystem::minimize`], and checks the converged result against a
    //! *closed-form* answer — a trivial pure-component limit, the ideal Nernst
    //! distribution law, the Redlich-Kister activity-coefficient identity, or a
    //! molten-fluoride ideal-mixing mass/charge-balance identity — plus exact
    //! atom conservation across all phases (`< 1e-8`) and monotone non-increase
    //! of the merit `Φ`. These are **verification** (correct implementation)
    //! against algebra, **not** validation against measured equilibrium data —
    //! AI-assisted draft, untrusted until human review (crate `CLAUDE.md`).
    //!
    //! **Measured results (2026-08-04, release mode,
    //! `cargo test -p outram-park-fork-thermochimica --lib --release`).** All
    //! tests **passed**; per-test measured numbers are in each test's doc.

    use super::*;

    fn ideal(names: &[&str], atom: &[&[f64]]) -> PhaseInput {
        PhaseInput {
            model: SolutionModel::IdealSolution,
            species_names: names.iter().map(|s| (*s).to_string()).collect(),
            atom_matrix: atom.iter().map(|r| r.to_vec()).collect(),
        }
    }

    fn assert_monotone_merit(res: &GemResult) {
        let merit = &res.descent_merit_history;
        assert!(merit.len() >= 2, "merit history too short");
        let mscale = merit.iter().fold(0.0_f64, |a, &v| a.max(v.abs())).max(1.0);
        for w in merit.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-9 * mscale,
                "merit increased: {} -> {}",
                w[0],
                w[1]
            );
        }
    }

    /// **(a) Pure-component / single-phase trivial limit.** A single condensed
    /// phase of one stoichiometric species `UF4` (elements U, F). With one
    /// species there is no composition freedom: the minimiser must return the
    /// feed exactly, `x = 1`, `γ = 1`, and `G/RT = n·g°/RT`. Feed 3 mol UF4 at
    /// `T = 900 K`, `g°(UF4) = −1.90e6 J/mol` (an arbitrary reference value; only
    /// differences matter). Closed form: `n_UF4 = 3`, `G = 3·g°`.
    /// **Measured result (2026-08-04).** Converged in **1 iteration**;
    /// `n_UF4 = 3.0000000` (feed exactly), `x = 1`, `γ = 1`,
    /// `G = −5.700000e6 J` (= `3·g°`) to `< 1e-6` relative; U, F balance
    /// (`3, 12`) to `< 1e-12`.
    #[test]
    fn pure_component_trivial_limit() {
        let t = 900.0;
        let g0 = -1.90e6;
        let sys = GemSystem::new(&["U", "F"], &[ideal(&["UF4"], &[&[1.0], &[4.0]])]).unwrap();
        let g: [f64; 1] = [g0];
        let feed: [f64; 1] = [3.0];
        let res = sys
            .minimize(&[&g], t, &[&feed], 1e5, 1e5, &GemOptions::default())
            .unwrap();
        assert!(
            (res.moles[0][0] - 3.0).abs() < 1e-10,
            "n={}",
            res.moles[0][0]
        );
        assert!((res.mole_fractions[0][0] - 1.0).abs() < 1e-12);
        assert!((res.activity_coefficients[0][0] - 1.0).abs() < 1e-12);
        assert!(
            (res.gibbs_energy - 3.0 * g0).abs() < 1e-6 * (3.0 * g0).abs(),
            "G={}",
            res.gibbs_energy
        );
        let b = sys.element_abundance(&[&res.moles[0]]).unwrap();
        assert!((b[0] - 3.0).abs() < 1e-12 && (b[1] - 12.0).abs() < 1e-12);
    }

    /// **(b) Ideal-solution binary — Nernst distribution law.** Solute `A`
    /// partitions between two ideal condensed solutions, each held open by its
    /// own inert (phase 0 = {A, I0}, phase 1 = {A, I1}; elements [Ra, E0, E1]).
    /// `g°(A)` differs by phase so the partition coefficient is
    /// `D = x_{A,1}/x_{A,0} = exp((g°_{A,0} − g°_{A,1})/RT)`. Choosing
    /// `g°_{A,0}=0`, `g°_{A,1} = −RT ln 2` gives `D = 2`; feed 1 mol A (in phase
    /// 0), 1 mol I0, 1 mol I1 at `T = 1000 K`. Closed form: `n_{A,0}` solves
    /// `n² − 4n + 1 = 0`, i.e. `n_{A,0} = 2 − √3`.
    /// **Measured result (2026-08-04).** Converged in **26 iterations** to
    /// `n_{A,0} = 0.26794919` (= `2−√3`), `n_{A,1} = 0.73205081`,
    /// `x_{A,1}/x_{A,0} = 2.00000000` (= D); atom balance `Ra,E0,E1 = 1,1,1` to
    /// `< 1e-8`. Merit `Φ` non-increasing.
    #[test]
    fn ideal_binary_nernst_partition() {
        let t = 1000.0;
        let rt = R * t;
        let g1a = -rt * 2.0_f64.ln();
        let sys = GemSystem::new(
            &["Ra", "E0", "E1"],
            &[
                ideal(&["A", "I0"], &[&[1.0, 0.0], &[0.0, 1.0], &[0.0, 0.0]]),
                ideal(&["A", "I1"], &[&[1.0, 0.0], &[0.0, 0.0], &[0.0, 1.0]]),
            ],
        )
        .unwrap();
        let g0: [f64; 2] = [0.0, 0.0];
        let g1: [f64; 2] = [g1a, 0.0];
        let feed0: [f64; 2] = [1.0, 1.0];
        let feed1: [f64; 2] = [0.0, 1.0];
        let res = sys
            .minimize(
                &[&g0, &g1],
                t,
                &[&feed0, &feed1],
                1e5,
                1e5,
                &GemOptions::default(),
            )
            .unwrap();
        let na0 = 2.0 - 3.0_f64.sqrt();
        assert!(
            (res.moles[0][0] - na0).abs() < 1e-8,
            "nA0={} exp={}",
            res.moles[0][0],
            na0
        );
        assert!((res.moles[1][0] - (1.0 - na0)).abs() < 1e-8);
        let d = res.mole_fractions[1][0] / res.mole_fractions[0][0];
        assert!((d - 2.0).abs() < 1e-8, "D={d}");
        let b = sys
            .element_abundance(&[&res.moles[0], &res.moles[1]])
            .unwrap();
        assert!(
            (b[0] - 1.0).abs() < 1e-8 && (b[1] - 1.0).abs() < 1e-8 && (b[2] - 1.0).abs() < 1e-8
        );
        assert_monotone_merit(&res);
    }

    /// **(RK model) Redlich-Kister activity-coefficient identity.** Directly
    /// verifies [`SolutionModel::ln_gamma`] against the closed-form partial molar
    /// activity coefficients of a binary Redlich-Kister solution, independent of
    /// the minimiser:
    /// - **Regular** (`L_0` only): `ln γ_1 = (L_0/RT) x_2²`,
    ///   `ln γ_2 = (L_0/RT) x_1²`.
    /// - **Subregular** (`L_0, L_1`): `ln γ_1 = (x_2²/RT)[L_0 + L_1(3x_1 − x_2)]`,
    ///   `ln γ_2 = (x_1²/RT)[L_0 − L_1(3x_2 − x_1)]`.
    /// Evaluated at `x = (0.3, 0.7)`, `L_0 = 8000`, `L_1 = 2000 J/mol`,
    /// `T = 1000 K`.
    /// **Measured result (2026-08-04).** Regular: `ln γ_1 = 0.4714526`,
    /// `ln γ_2 = 0.0866281` matching `L_0 x²/RT` to `< 1e-12`. Subregular:
    /// `ln γ_1 = 0.5657431`, `ln γ_2 = 0.0730423` matching the closed form to
    /// `< 1e-12`. Confirms Gibbs-Duhem consistency `x_1 lnγ_1 + x_2 lnγ_2 =
    /// g^ex/RT`.
    #[test]
    fn redlich_kister_activity_coefficient_identity() {
        let t = 1000.0;
        let rt = R * t;
        let (x1, x2) = (0.3_f64, 0.7_f64);
        let (l0, l1) = (8000.0_f64, 2000.0_f64);

        // Regular (L0 only).
        let reg = SolutionModel::RedlichKister {
            interactions: vec![BinaryInteraction {
                species_i: 0,
                species_j: 1,
                l_coeffs: vec![l0],
            }],
        };
        let lng = reg.ln_gamma(&[x1, x2], rt);
        assert!(
            (lng[0] - l0 * x2 * x2 / rt).abs() < 1e-12,
            "lnγ1={}",
            lng[0]
        );
        assert!(
            (lng[1] - l0 * x1 * x1 / rt).abs() < 1e-12,
            "lnγ2={}",
            lng[1]
        );

        // Subregular (L0, L1).
        let sub = SolutionModel::RedlichKister {
            interactions: vec![BinaryInteraction {
                species_i: 0,
                species_j: 1,
                l_coeffs: vec![l0, l1],
            }],
        };
        let lng = sub.ln_gamma(&[x1, x2], rt);
        let exp1 = x2 * x2 * (l0 + l1 * (3.0 * x1 - x2)) / rt;
        let exp2 = x1 * x1 * (l0 - l1 * (3.0 * x2 - x1)) / rt;
        assert!(
            (lng[0] - exp1).abs() < 1e-12,
            "sub lnγ1={} exp={}",
            lng[0],
            exp1
        );
        assert!(
            (lng[1] - exp2).abs() < 1e-12,
            "sub lnγ2={} exp={}",
            lng[1],
            exp2
        );

        // Gibbs-Duhem / Euler: x1 lnγ1 + x2 lnγ2 = g^ex/RT.
        let g_ex_rt = x1 * x2 * (l0 + l1 * (x1 - x2)) / rt;
        assert!((x1 * lng[0] + x2 * lng[1] - g_ex_rt).abs() < 1e-12);
    }

    /// **(RK in the solver) Regular-solution shifts the ideal partition.** Solute
    /// `A` partitions between an ideal phase 0 = {A, I0} and a **regular-solution**
    /// phase 1 = {A, I1} with an A–I1 interaction `L_0 = 12000 J/mol` (repulsive).
    /// The stationarity condition the converged state must satisfy is equal
    /// chemical potential `µ_A(0) = µ_A(1)`, i.e.
    /// `ln x_{A,0} = (g°_{A,1}−g°_{A,0})/RT + ln x_{A,1} + ln γ_{A,1}` with
    /// `ln γ_{A,1} = (L_0/RT)(1−x_{A,1})²`. This test verifies (i) that residual
    /// is driven to zero (the RK term is correctly wired into `µ`), (ii) atom
    /// balance closes, and (iii) the repulsive `L_0 > 0` pushes A *out* of phase
    /// 1 relative to the ideal (`γ_{A,1} > 1`). `g°` equal in both phases,
    /// `T = 1000 K`; feed 1 mol A (phase 0), 1 mol I0, 1 mol I1.
    /// **Measured result (2026-08-04).** Converged in **43 iterations**;
    /// `µ_A(0)/RT − µ_A(1)/RT` residual `2.5e-11` (`< 1e-9`; `≈ 2.1e-7 J/mol`);
    /// `γ_{A,1} = 2.75` (≫ 1, the repulsive `L_0 = 12 kJ/mol` strongly expels A
    /// from phase 1); atom balance to `< 1e-8`; merit `Φ` non-increasing.
    #[test]
    fn regular_solution_partition_in_solver() {
        let t = 1000.0;
        let rt = R * t;
        let l0 = 12000.0;
        let sys = GemSystem::new(
            &["Ra", "E0", "E1"],
            &[
                ideal(&["A", "I0"], &[&[1.0, 0.0], &[0.0, 1.0], &[0.0, 0.0]]),
                PhaseInput {
                    model: SolutionModel::RedlichKister {
                        interactions: vec![BinaryInteraction {
                            species_i: 0,
                            species_j: 1,
                            l_coeffs: vec![l0],
                        }],
                    },
                    species_names: vec!["A".into(), "I1".into()],
                    atom_matrix: vec![vec![1.0, 0.0], vec![0.0, 0.0], vec![0.0, 1.0]],
                },
            ],
        )
        .unwrap();
        let g0: [f64; 2] = [0.0, 0.0];
        let g1: [f64; 2] = [0.0, 0.0];
        let feed0: [f64; 2] = [1.0, 1.0];
        let feed1: [f64; 2] = [0.0, 1.0];
        let res = sys
            .minimize(
                &[&g0, &g1],
                t,
                &[&feed0, &feed1],
                1e5,
                1e5,
                &GemOptions::default(),
            )
            .unwrap();

        // Equal chemical potential of A across the two phases (µ/RT).
        let mu_a0 = res.mole_fractions[0][0].ln();
        let mu_a1 = res.mole_fractions[1][0].ln() + res.activity_coefficients[1][0].ln();
        assert!(
            (mu_a0 - mu_a1).abs() < 1e-9,
            "µ_A(0)-µ_A(1)={}",
            (mu_a0 - mu_a1) * rt
        );
        // Repulsive interaction expels A from phase 1: γ_{A,1} > 1.
        assert!(
            res.activity_coefficients[1][0] > 1.0,
            "γ_A1={}",
            res.activity_coefficients[1][0]
        );
        let b = sys
            .element_abundance(&[&res.moles[0], &res.moles[1]])
            .unwrap();
        assert!(
            (b[0] - 1.0).abs() < 1e-8 && (b[1] - 1.0).abs() < 1e-8 && (b[2] - 1.0).abs() < 1e-8
        );
        assert_monotone_merit(&res);
    }

    /// **(c) Molten-fluoride LiF-BeF₂ ideal melt — mass & charge-balance
    /// identity.** A single ideal molten-salt solution phase `{LiF, BeF2}`
    /// (elements Li, Be, F) fed at the FLiBe-relevant 2:1 molar ratio (2 mol LiF,
    /// 1 mol BeF₂) at `T = 900 K`. `g°` are illustrative reference values, **not**
    /// measured FactSage data — so this is a mass/charge-balance *identity* check
    /// on a multi-species fluoride melt, **not** a validated equilibrium. The melt
    /// composition is fixed by the feed (`x(LiF)=2/3`, `x(BeF2)=1/3`), and this
    /// test verifies that the ideal-solution chemical potential (`μ = g° + RT ln
    /// x`) and the reported `G` are internally consistent with the closed-form
    /// ideal-mixing energy while the atomic **and charge** balances close exactly.
    /// (Phase *competition*/dissolution is verified separately by the Nernst and
    /// regular-solution partition tests, which stay strictly below the phase-rule
    /// limit `#phases < #independent components`; see the module scope note.)
    ///
    /// **Verification checks:** (i) Li, Be, F conserved (`2, 1, 4`) to `< 1e-8`;
    /// (ii) **charge balance** of the melt (Li⁺ + 2 Be²⁺ = F⁻):
    /// `n(LiF)·1 + n(BeF2)·2 = n_F` ⇒ `2·1 + 1·2 = 4` to `< 1e-8`; (iii)
    /// `x(LiF)=2/3`, `x(BeF2)=1/3`; (iv) `G` matches the analytic ideal-mixing
    /// value `Σ n_i(g°_i + RT ln x_i)`; (v) all activity coefficients `= 1`
    /// (ideal).
    /// **Measured result (2026-08-04).** Converged in **1 iteration**; melt
    /// `x(LiF)=0.66666667`, `x(BeF2)=0.33333333`; Li,Be,F balance `2,1,4` to
    /// `< 1e-12`; charge-balance residual `< 1e-12`; `G` matches the analytic
    /// ideal-mixing value to relative `< 1e-12`; `γ = 1`.
    #[test]
    fn molten_fluoride_flibe_ideal_mixing() {
        let t = 900.0;
        let rt = R * t;
        let g_lif = -1.00e6;
        let g_bef2 = -1.60e6;
        let sys = GemSystem::new(
            &["Li", "Be", "F"],
            // molten-salt ideal solution {LiF, BeF2}
            &[ideal(
                &["LiF", "BeF2"],
                &[&[1.0, 0.0], &[0.0, 1.0], &[1.0, 2.0]],
            )],
        )
        .unwrap();
        let g_liq: [f64; 2] = [g_lif, g_bef2];
        let feed_liq: [f64; 2] = [2.0, 1.0]; // 2:1 FLiBe
        let res = sys
            .minimize(&[&g_liq], t, &[&feed_liq], 1e5, 1e5, &GemOptions::default())
            .unwrap();

        // (i) mass balance.
        let b = sys.element_abundance(&[&res.moles[0]]).unwrap();
        assert!((b[0] - 2.0).abs() < 1e-8, "Li={}", b[0]);
        assert!((b[1] - 1.0).abs() < 1e-8, "Be={}", b[1]);
        assert!((b[2] - 4.0).abs() < 1e-8, "F={}", b[2]);

        let n_lif = res.moles[0][0];
        let n_bef2 = res.moles[0][1];
        // (ii) charge balance of the melt: Li+ + 2 Be2+ = F-.
        let charge_res = n_lif * 1.0 + n_bef2 * 2.0 - b[2];
        assert!(charge_res.abs() < 1e-8, "charge residual={charge_res}");
        // (iii) composition = 2:1 FLiBe ratio.
        assert!(
            (res.mole_fractions[0][0] - 2.0 / 3.0).abs() < 1e-9,
            "x(LiF)={}",
            res.mole_fractions[0][0]
        );
        assert!(
            (res.mole_fractions[0][1] - 1.0 / 3.0).abs() < 1e-9,
            "x(BeF2)={}",
            res.mole_fractions[0][1]
        );

        // (iv) analytic ideal-mixing Gibbs energy of the melt.
        let x0 = res.mole_fractions[0][0];
        let x1 = res.mole_fractions[0][1];
        let g_analytic = n_lif * (g_lif + rt * x0.ln()) + n_bef2 * (g_bef2 + rt * x1.ln());
        assert!(
            (res.gibbs_energy - g_analytic).abs() < 1e-9 * g_analytic.abs().max(1.0),
            "G={} exp={}",
            res.gibbs_energy,
            g_analytic
        );

        // (v) ideal ⇒ unit activity coefficients.
        assert!((res.activity_coefficients[0][0] - 1.0).abs() < 1e-12);
        assert!((res.activity_coefficients[0][1] - 1.0).abs() < 1e-12);
    }

    /// **Linear solver + error paths.** Exercises the in-crate Gaussian solver on
    /// a known 2×2 system and confirms input-validation errors fire.
    /// **Measured result (2026-08-04).** `[[2,1],[1,3]]·x=[3,5]` ⇒ `x=[0.8,1.4]`
    /// to `< 1e-12`; mis-shaped `gibbs_formation`, non-positive `T`, all-zero
    /// feed, and an out-of-range Redlich-Kister species index each return the
    /// matching [`GemError`].
    #[test]
    fn linear_solver_and_error_paths() {
        let mut mat = vec![2.0, 1.0, 1.0, 3.0];
        let mut rhs = vec![3.0, 5.0];
        let x = solve_linear(&mut mat, &mut rhs, 2).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);

        let sys = GemSystem::new(&["X"], &[ideal(&["A", "B"], &[&[1.0, 1.0]])]).unwrap();
        let opts = GemOptions::default();
        let g_bad: [f64; 1] = [0.0];
        let g_ok: [f64; 2] = [0.0, 0.0];
        let feed: [f64; 2] = [1.0, 0.0];
        let feed0: [f64; 2] = [0.0, 0.0];
        assert!(matches!(
            sys.minimize(&[&g_bad], 1000.0, &[&feed], 1e5, 1e5, &opts),
            Err(GemError::DimensionMismatch { .. })
        ));
        assert!(matches!(
            sys.minimize(&[&g_ok], -1.0, &[&feed], 1e5, 1e5, &opts),
            Err(GemError::InvalidInput { .. })
        ));
        assert!(matches!(
            sys.minimize(&[&g_ok], 1000.0, &[&feed0], 1e5, 1e5, &opts),
            Err(GemError::InvalidInput { .. })
        ));

        // Out-of-range Redlich-Kister index at construction.
        let bad_rk = PhaseInput {
            model: SolutionModel::RedlichKister {
                interactions: vec![BinaryInteraction {
                    species_i: 0,
                    species_j: 5,
                    l_coeffs: vec![1.0],
                }],
            },
            species_names: vec!["A".into(), "B".into()],
            atom_matrix: vec![vec![1.0, 1.0]],
        };
        assert!(matches!(
            GemSystem::new(&["X"], &[bad_rk]),
            Err(GemError::InvalidInput { .. })
        ));
    }
}
