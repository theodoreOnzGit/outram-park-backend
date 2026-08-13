#![forbid(unsafe_code)]
//! Multi-phase (N-phase) Gibbs-energy-**minimisation** flash — equilibrium
//! distribution of chemical species across several coexisting solution phases
//! (gas / molten-salt / condensed solution) by direct minimisation of the total
//! Gibbs energy, subject to **element / atom mass-balance** constraints shared
//! across all phases.
//!
//! This generalises the single-phase reacting-speciation minimiser
//! [`crate::thermo::gibbs`] (`GibbsSystem::minimize`) from one phase to `P`
//! coexisting phases. It is aimed at the molten-salt / fission-product
//! speciation problem, where the same set of chemical elements partitions
//! between a gas phase, a molten-salt solution, and condensed solutions
//! simultaneously.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported / adapted from DWSIM (GPL-3.0), commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`:
//! - `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimizationMulti.vb`
//! - `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimization3P.vb`
//!
//! DWSIM states the multi-phase objective as (`GibbsMinimizationMulti.vb:132`,
//! `GibbsMinimization3P.vb:146`):
//!
//! ```text
//! Q = Σ_k Σ_i n_ik · ln f_ik
//! ```
//!
//! the sum over phases `k` and species `i` of moles times `ln` of the species
//! fugacity, with gradient (`GibbsMinimizationMulti.vb:140`,
//! `GibbsMinimization3P.vb:154`)
//!
//! ```text
//! ∂Q/∂n_ij = ln f_ij − ln f_iF
//! ```
//!
//! and the numeric per-phase Gibbs energy assembled in `FunctionValue` as
//! `Σ_i x_i · n_phase · ln(φ_i x_i)` per phase
//! (`GibbsMinimizationMulti.vb:712,797-806`; `GibbsMinimization3P.vb:760-761,
//! 838-840`). DWSIM minimises that objective **over a phase split** (species
//! conserved individually, `n_iF = z_i − Σ_{k<F} n_ik`,
//! `GibbsMinimizationMulti.vb:136`) and delegates the constrained optimisation
//! to an **external** solver — IPOPT via `Cureos.Numerics`
//! (`GibbsMinimizationMulti.vb:20`; the `Solver` property defaults to
//! `OptimizationMethod.IPOPT`, `:73`).
//!
//! ## What THIS module does (and how it differs from the DWSIM file)
//!
//! It keeps DWSIM's Gibbs objective and its ideal-solution chemical-potential
//! model `μ_ik = g°_ik + RT ln(φ_ik x_ik) [+ RT ln(P/P°) for a gas]`, but:
//!
//! 1. **Element/atom balance replaces the species phase-split constraint.**
//!    This is the *non-stoichiometric* (element-abundance) formulation — the
//!    White–Johnson–Dantzig / **RAND** element-potential method (Smith & Missen,
//!    *Chemical Reaction Equilibrium Analysis*, 1982; NASA CEA, Gordon &
//!    McBride, RP-1311), extended to multiple phases with a **shared** element
//!    potential. It reduces to a species phase-split when every species is
//!    atomically distinct (see the Nernst-partition V&V test), and it also
//!    admits inter-species reactions where the atoms allow (see the water-gas-
//!    shift collapse test) — a strict superset of DWSIM's non-reacting flash.
//! 2. **The constrained optimiser is pure Rust** — a damped multi-phase RAND /
//!    element-potential Newton iteration (below). DWSIM's IPOPT dependency is
//!    not available in this workspace, so none is used; the `(M + P)` linear
//!    system is solved with an in-crate Gaussian elimination (no BLAS/LAPACK,
//!    so this compiles on Android / Termux like the rest of the non-GUI crate).
//!
//! ## The minimisation problem
//!
//! Minimise the dimensionless total Gibbs energy over `P` phases:
//!
//! ```text
//! G/RT = Σ_p Σ_s n_ps · (μ_ps / RT),
//! μ_ps/RT = g°_ps/(RT) + ln φ_ps + ln x_ps [+ ln(P/P°) if phase p is a gas]
//! ```
//!
//! where `n_ps` is the moles of species `s` in phase `p` \[mol\],
//! `x_ps = n_ps / Σ_l n_pl` its mole fraction within phase `p`, and `g°_ps` its
//! standard molar Gibbs energy in that phase \[J/mol\] (a species may take a
//! different `g°` in different phases, e.g. `H₂O(g)` vs `H₂O(l)`). The atom
//! matrix `a_ks` (atoms of element `k` per molecule of species `s`) is a
//! property of the species, identical in every phase. Subject to, for every
//! element `k`,
//!
//! ```text
//! Σ_p Σ_s a_ks · n_ps = b_k     (atom balance over ALL phases),   n_ps ≥ 0.
//! ```
//!
//! The Lagrange stationarity condition is the **shared element-potential**
//! relation — one `π_k` per element, common to *every* phase:
//!
//! ```text
//! μ_ps / RT = Σ_k a_ks · π_k     for every species s present in every phase p.
//! ```
//!
//! Equating this across two phases containing the same species `s` gives
//! `μ_s(phase p)= μ_s(phase q)`: equal chemical potential — the phase-
//! equilibrium condition. For a single species distributing between two ideal
//! solutions it becomes the **Nernst distribution law**
//! `x_sq / x_sp = exp((g°_sp − g°_sq)/RT)`; for a species over its own pure
//! condensed phase it becomes the **vapour-pressure** relation
//! `x_s(P/P°) = exp((g°_cond − g°_gas)/RT)`. Both are checked in closed form in
//! the V&V tests.
//!
//! ## The multi-phase RAND / element-potential iteration (pure Rust)
//!
//! At a strictly-positive current estimate `n_ps`, with phase totals
//! `n_p = Σ_s n_ps`, form the `(M+P)×(M+P)` saddle-point system for the `M`
//! shared element potentials `π_k` and the `P` per-phase total-mole corrections
//! `u_p = Δ ln n_p`:
//!
//! ```text
//! Σ_j R_kj π_j + Σ_p Q_kp u_p = (b_k − q_k) + S_k     (one row per element k)
//! Σ_j Q_jp π_j                = F_p                    (one row per phase p)
//! ```
//!
//! with (sums over species `s`, and over phases `p` where written)
//! `q_k = Σ_p Σ_s a_ks n_ps` (current element abundance),
//! `Q_kp = Σ_s a_ks n_ps` (element `k` abundance *in phase p*),
//! `R_kj = Σ_p Σ_s a_ks a_js n_ps`, `S_k = Σ_p Σ_s a_ks n_ps (μ_ps/RT)`, and
//! `F_p = Σ_s n_ps (μ_ps/RT) = G_p/RT` (phase Gibbs energy). The species
//! correction is then
//!
//! ```text
//! δ_ps = Δ ln n_ps = Σ_k a_ks π_k + u_p − μ_ps/RT,
//! n_ps ← n_ps · exp(ω · δ_ps),   ω = min(1, max_step / max|δ|).
//! ```
//!
//! The block structure `[[R, Q],[Qᵀ, 0]]` is exactly the single-phase system of
//! [`crate::thermo::gibbs`] with the scalar total-mole row generalised to `P`
//! rows/columns; for `P = 1` it is identical. The **multiplicative** update
//! keeps every `n_ps > 0` for free, and the `(b_k − q_k)` residual drives the
//! atom balance back onto target each step, so the iteration is self-correcting
//! against round-off drift.
//!
//! **Merit line search (monotone descent).** ω is further reduced by a
//! backtracking line search on the merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` (penalty
//! [`MERIT_PENALTY`]): the largest halving of the damped ω₀ whose trial `Φ` does
//! not rise is taken. `Φ` is thus monotonically non-increasing *by construction*,
//! and since its atom-violation term → 0 at convergence, `Φ → G/RT`. This keeps
//! the iterate path near the constraint manifold; without it the projection-free
//! multiplicative step would dip through infeasible points whose `G/RT` lies
//! *below* the constrained minimum before rebounding (non-monotone `G/RT`). The
//! monotone witness returned is [`MultiPhaseResult::descent_merit_history`].
//!
//! **Vanishing phases.** When a phase's total `n_p` falls below
//! [`MultiPhaseOptions::phase_floor`], its `Q_·p` column vanishes and the saddle
//! system would go singular. Such a phase is *frozen*: its row `M+p` is replaced
//! by the identity `u_p = 0`, so the active phases still solve a well-posed
//! system while the frozen phase's species keep updating from the shared `π`
//! (they regrow if `μ_ps < Σ_k a_ks π_k` becomes favourable, else stay near
//! zero). This is how a two-phase run **collapses onto the single-phase result**
//! when only one phase is thermodynamically stable (V&V test below).
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Raw `f64` in SI throughout the inner loop (the crate "raw f64 in inner
//! EOS/flash loops" rule): temperature `T` \[K\], pressure `P` and reference
//! pressure `P°` \[Pa\], standard Gibbs energies `g°_ps` \[J/mol\], moles
//! \[mol\], mole fractions and element potentials dimensionless. The gas
//! constant is the crate-shared CODATA value [`crate::thermo::ideal_props::R`].
//!
//! ## Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn` / `Box` / lifetimes**: the per-phase mixing model
//! is the closed enum [`PhaseModel`]; the system, options, result, and errors
//! are plain owned structs / enums. `#![forbid(unsafe_code)]`. Raw `f64` maths
//! inside.
//!
//! ## Honest scope — untrusted AI-assisted draft, pending human V&V
//!
//! - **Ideal-solution phases only.** Each phase mixes ideally
//!   ([`PhaseModel::IdealGas`] / [`PhaseModel::IdealSolution`], `φ = 1`, activity
//!   `= x`). A single-species phase therefore has activity `1` and models a
//!   **pure condensed** substance exactly (used by the vapour-pressure test). A
//!   caller-supplied *constant* `ln φ`/`ln γ` correction (frozen-fugacity) and a
//!   self-consistent EOS/activity coupling that recomputes `φ_ps(x,T,P)` /
//!   `γ_ps(x,T)` each iteration are **not** implemented — documented extension
//!   points, matching the same limitation in [`crate::thermo::gibbs`].
//! - **No automatic phase creation / detection.** The set of candidate phases is
//!   supplied by the caller. A candidate that is unstable collapses to ~0 (see
//!   vanishing-phases above), but the algorithm does **not** discover a phase the
//!   caller did not list. Stability analysis (adding a trial phase from a TPD
//!   test, cf. [`crate::thermo::stability`]) is future work.
//! - **Verified, not validated.** The tests below are analytic **verification**
//!   (Nernst partition, vapour-pressure, water-gas-shift equilibrium constant,
//!   atom balance, cross-check against the single-phase [`crate::thermo::gibbs`])
//!   against closed-form solutions, **not** validation against a measured
//!   multi-phase equilibrium dataset. AI-assisted draft material, untrusted
//!   until human-reviewed per the crate `CLAUDE.md`. Not for nuclear facility
//!   operation, reactor control, safety-critical, or licensing decisions.
//!   Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::ideal_props::R;

/// L1 penalty weight `μ` on the atom-balance violation in the line-search merit
/// function `Φ = G/RT + μ · Σ_k |b_k − q_k|` \[-\]. Chosen a few × larger than the
/// `O(1)` dimensionless Gibbs energy so feasibility dominates step acceptance:
/// this keeps the iterate path near the constraint manifold and makes
/// [`MultiPhaseResult::descent_merit_history`] a tight upper envelope of the
/// physical `G/RT`. The merit is monotone for **any** positive weight (the line
/// search enforces it); this value trades a modest iteration count against how
/// closely the merit tracks `G/RT`. Larger weights track `G/RT` more tightly but
/// converge more slowly (measured on the Nernst case, 2026-08-03: μ=30 → 69
/// iters, worst physical-`G/RT` step-rise 1.0e-2; μ=100 → 242 iters, 2.6e-3;
/// μ=1000 → 2319 iters, 1.4e-4).
const MERIT_PENALTY: f64 = 10.0;

/// Ideal mixing model for one phase (enum dispatch, no `dyn`).
///
/// Selects the composition term in the species chemical potential
/// `μ_ps/RT = g°_ps/(RT) + ln x_ps + [ln(P/P°) if gas]`. The set is closed and
/// known at compile time, so it is an enum, not a trait object (workspace
/// `CLAUDE.md`). Both variants use ideal mixing (activity `= x`, `φ = 1`); they
/// differ only by whether the pressure term `ln(P/P°)` is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseModel {
    /// Ideal **gas** solution: `μ_ps/RT = g°_ps/(RT) + ln x_ps + ln(P/P°)`. The
    /// `ln(P/P°)` term drives the Le-Chatelier pressure response of any
    /// mole-changing reaction and the vapour-pressure partition of a species
    /// over a condensed phase.
    IdealGas,
    /// Ideal **condensed** solution (molten salt, liquid, or solid solution):
    /// `μ_ps/RT = g°_ps/(RT) + ln x_ps`, with **no** pressure term (condensed-
    /// phase molar volume neglected). A single-species `IdealSolution` phase has
    /// `x = 1`, hence unit activity — an exact **pure condensed** substance.
    IdealSolution,
}

impl PhaseModel {
    /// `ln(P/P°)` contribution of this phase model \[-\]: `ln(pressure/p_ref)`
    /// for a gas, `0` for a condensed solution.
    #[inline]
    #[must_use]
    fn ln_p_term(self, ln_p_ratio: f64) -> f64 {
        match self {
            PhaseModel::IdealGas => ln_p_ratio,
            PhaseModel::IdealSolution => 0.0,
        }
    }
}

/// One phase's static (combinatorial) description supplied to
/// [`MultiPhaseGibbsSystem::new`]: its mixing model, its species labels, and the
/// atom sub-matrix mapping each of its species onto the *global* element list.
///
/// The atom matrix has `M` rows (one per global element, in the system's element
/// order) and `n_species` columns; `atom_matrix[k][s]` is the number of atoms of
/// element `k` in one molecule of species `s` of this phase (non-negative,
/// finite; typically small integers).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseInput {
    /// Ideal mixing model for the phase.
    pub model: PhaseModel,
    /// Species labels for this phase (identification only). Length =
    /// `n_species` of the phase; a species may also appear (by the same atoms,
    /// possibly different `g°`) in other phases.
    pub species_names: Vec<String>,
    /// `M` rows × `n_species` columns: atoms of each global element per molecule
    /// of each species in this phase.
    pub atom_matrix: Vec<Vec<f64>>,
}

/// Convergence / iteration controls for [`MultiPhaseGibbsSystem::minimize`].
///
/// All tolerances are dimensionless. Defaults (via [`Default`]) mirror the
/// single-phase [`crate::thermo::gibbs::GibbsOptions`], with an added
/// `phase_floor`: `tol = 1e-10`, `max_iter = 3000`, `max_step = 2.0`,
/// `mole_floor = 1e-12`, `phase_floor = 1e-11`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiPhaseOptions {
    /// Convergence tolerance \[-\]. The iteration stops when both the maximum
    /// species log-correction `max|δ_ps|` and the worst relative atom-balance
    /// residual fall below this value.
    pub tol: f64,
    /// Maximum RAND iterations before returning [`MultiPhaseError::NotConverged`].
    pub max_iter: usize,
    /// Maximum per-step log-correction `max|ω·δ_ps|` \[-\]. Damping factor is
    /// `ω = min(1, max_step / max|δ|)`, capping how far any species moves in one
    /// step (in `ln n` space). Typical `2.0` (a factor `e² ≈ 7.4` per step).
    pub max_step: f64,
    /// Lower floor \[mol\] applied to the *initial* working moles of any (phase,
    /// species) the feed supplies as zero, so `ln n` is finite at the start.
    /// Must be > 0.
    pub mole_floor: f64,
    /// Phase-total floor \[mol\]. A phase whose total moles fall below this is
    /// *frozen* (its `u_p` correction is pinned to `0`) to keep the saddle-point
    /// system non-singular as the phase vanishes; its species still update from
    /// the shared element potentials. Must be > 0 and ≥ `mole_floor`.
    pub phase_floor: f64,
}

impl Default for MultiPhaseOptions {
    fn default() -> Self {
        Self {
            tol: 1e-10,
            max_iter: 3000,
            max_step: 2.0,
            mole_floor: 1e-12,
            phase_floor: 1e-11,
        }
    }
}

/// Result of a converged multi-phase Gibbs minimisation.
///
/// Per-phase quantities are indexed `[phase][species]` in the phase / species
/// order the system was built with; element potentials are an `M`-vector in
/// element order.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiPhaseResult {
    /// Equilibrium species amounts `n_ps` \[mol\], `moles[p][s]`. A phase that
    /// collapsed reports species amounts near `mole_floor` or below.
    pub moles: Vec<Vec<f64>>,
    /// Equilibrium within-phase mole fractions `x_ps = n_ps / Σ_l n_pl` \[-\],
    /// `mole_fractions[p][s]`. For a collapsed phase (total < `phase_floor`)
    /// these are reported as `0`.
    pub mole_fractions: Vec<Vec<f64>>,
    /// Total moles in each phase `n_p = Σ_s n_ps` \[mol\], one entry per phase.
    pub phase_totals: Vec<f64>,
    /// Shared element potentials `π_k = λ_k / RT` \[-\], one per element. At the
    /// solution every present species satisfies `μ_ps/RT = Σ_k a_ks π_k` in
    /// *every* phase — the phase-equilibrium condition.
    pub element_potentials: Vec<f64>,
    /// Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (μ_ps/RT)` \[mol\]
    /// at the returned composition (the objective that was minimised).
    pub gibbs_energy_rt: f64,
    /// Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the `g°_ps`
    /// reference states supplied.
    pub gibbs_energy: f64,
    /// Trajectory of the physical `G/RT` \[mol\] evaluated at the iterate
    /// *entering* each RAND iteration, followed by the final converged value. It
    /// descends overall from the feed point to the constrained minimum
    /// (`gibbs_energy_rt_history[0]` ≥ `.last()`), but is not guaranteed
    /// non-increasing at *every* step: the multiplicative update visits slightly
    /// off-manifold points whose `G/RT` can wobble by the step's second-order
    /// atom-balance error. The rigorously monotone quantity is
    /// [`Self::descent_merit_history`].
    pub gibbs_energy_rt_history: Vec<f64>,
    /// Trajectory of the **line-search merit** `Φ = G/RT + μ·Σ_k|b_k − q_k|`
    /// \[mol\] (dimensionless-times-mol; `μ` the internal penalty weight), one
    /// entry per iteration plus the final value. This is **monotonically
    /// non-increasing by construction** — each step's backtracking accepts only a
    /// `Φ` that does not rise (to float slack). Because the atom-violation term
    /// `→ 0` at convergence, `Φ → G/RT`; so a monotone `Φ` descending to the
    /// converged `G/RT` is the V&V monotonicity witness for the Gibbs-energy
    /// minimisation (the exact-penalty / augmented-objective sense).
    pub descent_merit_history: Vec<f64>,
    /// Number of RAND iterations performed.
    pub iterations: usize,
    /// `true` if both tolerances in [`MultiPhaseOptions`] were met.
    pub converged: bool,
}

/// Errors from constructing or solving a [`MultiPhaseGibbsSystem`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MultiPhaseError {
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
    /// The RAND saddle-point system was singular even after freezing vanished
    /// phases — typically a rank-deficient atom matrix (a duplicated element
    /// row, an element no species contains, or two phases with proportional
    /// element content). Check the atom matrices have full row rank and the
    /// phases are distinct in element space.
    #[error("singular multi-phase RAND system (atom matrix / phase set likely rank-deficient)")]
    SingularSystem,
    /// The iteration did not meet both tolerances within `max_iter`. Carries the
    /// worst residuals seen so the caller can judge how close it got.
    #[error("multi-phase Gibbs minimisation did not converge in {iterations} iters (max|δ|={max_correction:.3e}, atom residual={atom_residual:.3e})")]
    NotConverged {
        /// Iterations performed (`= max_iter`).
        iterations: usize,
        /// Final `max|δ_ps|` \[-\].
        max_correction: f64,
        /// Final worst relative atom-balance residual \[-\].
        atom_residual: f64,
    },
}

/// Internal per-phase storage: mixing model, atom matrix (row-major
/// `[element][species]`, length `n_elements * n_species`), and species count.
#[derive(Debug, Clone, PartialEq)]
struct Phase {
    model: PhaseModel,
    names: Vec<String>,
    /// Row-major `a[k * n_species + s]`.
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
/// with its own species and atom sub-matrix.
///
/// The system carries only the *combinatorial* data (which phases exist, who is
/// made of what); the thermodynamics (standard Gibbs energies, `T`, `P`, and the
/// feed) are passed per solve to [`Self::minimize`], so one system can be reused
/// across states.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiPhaseGibbsSystem {
    elements: Vec<String>,
    phases: Vec<Phase>,
    n_elements: usize,
}

impl MultiPhaseGibbsSystem {
    /// Build a multi-phase system from the global element list and one
    /// [`PhaseInput`] per phase.
    ///
    /// # Parameters
    /// - `element_symbols`: one label per global chemical element (length `M`,
    ///   `M ≥ 1`). Element potentials `π_k` are shared across every phase.
    /// - `phases`: one [`PhaseInput`] per phase (`P ≥ 1`). Each phase's
    ///   `atom_matrix` must have exactly `M` rows and, in every row,
    ///   `species_names.len()` columns.
    ///
    /// # Errors
    /// [`MultiPhaseError::DimensionMismatch`] if a phase's atom-matrix row count
    /// ≠ `M`, or any row length ≠ that phase's species count;
    /// [`MultiPhaseError::InvalidInput`] if any atom count is non-finite or
    /// negative, or if there are zero elements, zero phases, or a phase with
    /// zero species.
    pub fn new(element_symbols: &[&str], phases: &[PhaseInput]) -> Result<Self, MultiPhaseError> {
        let n_elements = element_symbols.len();
        if n_elements == 0 {
            return Err(MultiPhaseError::InvalidInput {
                what: "element_symbols (empty)",
                value: 0.0,
                positive: true,
            });
        }
        if phases.is_empty() {
            return Err(MultiPhaseError::InvalidInput {
                what: "phases (empty)",
                value: 0.0,
                positive: true,
            });
        }
        let mut built = Vec::with_capacity(phases.len());
        for ph in phases {
            let n_species = ph.species_names.len();
            if n_species == 0 {
                return Err(MultiPhaseError::InvalidInput {
                    what: "phase species (empty)",
                    value: 0.0,
                    positive: true,
                });
            }
            if ph.atom_matrix.len() != n_elements {
                return Err(MultiPhaseError::DimensionMismatch {
                    what: "phase atom_matrix rows",
                    expected: n_elements,
                    got: ph.atom_matrix.len(),
                });
            }
            let mut a = vec![0.0; n_elements * n_species];
            for (k, row) in ph.atom_matrix.iter().enumerate() {
                if row.len() != n_species {
                    return Err(MultiPhaseError::DimensionMismatch {
                        what: "phase atom_matrix row",
                        expected: n_species,
                        got: row.len(),
                    });
                }
                for (s, &v) in row.iter().enumerate() {
                    if !v.is_finite() || v < 0.0 {
                        return Err(MultiPhaseError::InvalidInput {
                            what: "atom_matrix entry",
                            value: v,
                            positive: false,
                        });
                    }
                    a[k * n_species + s] = v;
                }
            }
            built.push(Phase {
                model: ph.model,
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

    /// Species names of phase `p`, in species order. Panics if `p` is out of
    /// range.
    #[must_use]
    pub fn species_names(&self, p: usize) -> &[String] {
        &self.phases[p].names
    }

    /// Total moles of each element supplied by a per-phase composition:
    /// `b_k = Σ_p Σ_s a_ks · moles[p][s]` \[mol of atoms\], one entry per
    /// element.
    ///
    /// Used to form the atom-balance targets from a feed and to check
    /// conservation of a result.
    ///
    /// # Errors
    /// [`MultiPhaseError::DimensionMismatch`] if `moles` does not have one inner
    /// slice per phase, or a slice length ≠ that phase's species count.
    pub fn element_abundance(&self, moles: &[&[f64]]) -> Result<Vec<f64>, MultiPhaseError> {
        if moles.len() != self.phases.len() {
            return Err(MultiPhaseError::DimensionMismatch {
                what: "moles phases",
                expected: self.phases.len(),
                got: moles.len(),
            });
        }
        let mut b = vec![0.0; self.n_elements];
        for (p, ph) in self.phases.iter().enumerate() {
            if moles[p].len() != ph.n_species {
                return Err(MultiPhaseError::DimensionMismatch {
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

    /// Minimise `G/RT` over all phases subject to shared atom balance, returning
    /// the equilibrium multi-phase distribution.
    ///
    /// Solves the multi-phase RAND / element-potential iteration described in the
    /// module docs. Atom-balance **targets** come from `feed` (which sets both
    /// the targets `b_k` and the initial distribution); zero entries are lifted
    /// to `options.mole_floor` so `ln n` is finite at the start.
    ///
    /// # Parameters
    /// - `gibbs_formation`: `gibbs_formation[p][s]` = standard molar Gibbs
    ///   energy of formation `g°_ps` of species `s` in phase `p` at the system
    ///   temperature `T` \[J/mol\]. One inner slice per phase, each of that
    ///   phase's species length. Only **differences** matter to the equilibrium
    ///   (a common additive offset cancels), provided all phases/species share
    ///   one consistent reference.
    /// - `temperature` `T` \[K\], > 0.
    /// - `feed` \[mol\]: `feed[p][s]` = feed of species `s` initially placed in
    ///   phase `p`. Defines the atom-balance targets `b_k = Σ_p Σ_s a_ks feed`
    ///   and seeds the iteration. Entries may be 0. One slice per phase.
    /// - `pressure` `P` \[Pa\], > 0, and `p_ref` `P°` \[Pa\], > 0: enter each
    ///   *gas* phase's composition term as `ln(x_ps P/P°)`; condensed phases
    ///   ignore `P`.
    /// - `options`: convergence controls.
    ///
    /// # Returns
    /// A [`MultiPhaseResult`] with the per-phase equilibrium moles and mole
    /// fractions, phase totals, shared element potentials, the Gibbs energy, and
    /// the `G/RT` trajectory.
    ///
    /// # Errors
    /// [`MultiPhaseError::DimensionMismatch`] on any mis-shaped input;
    /// [`MultiPhaseError::InvalidInput`] on non-finite `g°`, non-positive `T`,
    /// `P`, `P°`, `mole_floor`, or `phase_floor`, negative feed, or an all-zero
    /// feed; [`MultiPhaseError::SingularSystem`] if the RAND system is singular;
    /// [`MultiPhaseError::NotConverged`] if `max_iter` is exhausted.
    #[allow(clippy::too_many_arguments)]
    pub fn minimize(
        &self,
        gibbs_formation: &[&[f64]],
        temperature: f64,
        feed: &[&[f64]],
        pressure: f64,
        p_ref: f64,
        options: &MultiPhaseOptions,
    ) -> Result<MultiPhaseResult, MultiPhaseError> {
        let m = self.n_elements;
        let np = self.phases.len();

        // ---- validate shapes ----
        if gibbs_formation.len() != np {
            return Err(MultiPhaseError::DimensionMismatch {
                what: "gibbs_formation phases",
                expected: np,
                got: gibbs_formation.len(),
            });
        }
        if feed.len() != np {
            return Err(MultiPhaseError::DimensionMismatch {
                what: "feed phases",
                expected: np,
                got: feed.len(),
            });
        }
        for (p, ph) in self.phases.iter().enumerate() {
            if gibbs_formation[p].len() != ph.n_species {
                return Err(MultiPhaseError::DimensionMismatch {
                    what: "gibbs_formation[p]",
                    expected: ph.n_species,
                    got: gibbs_formation[p].len(),
                });
            }
            if feed[p].len() != ph.n_species {
                return Err(MultiPhaseError::DimensionMismatch {
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
                return Err(MultiPhaseError::InvalidInput {
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
                    return Err(MultiPhaseError::InvalidInput {
                        what: "gibbs_formation entry",
                        value: g,
                        positive: false,
                    });
                }
                let f = feed[p][s];
                if !f.is_finite() || f < 0.0 {
                    return Err(MultiPhaseError::InvalidInput {
                        what: "feed entry",
                        value: f,
                        positive: false,
                    });
                }
                feed_sum += f;
            }
        }
        if feed_sum <= 0.0 {
            return Err(MultiPhaseError::InvalidInput {
                what: "feed (all zero)",
                value: feed_sum,
                positive: true,
            });
        }

        let rt = R * temperature;
        let ln_p_ratio = (pressure / p_ref).ln();

        // Constant (composition-independent) part of μ_ps/RT:
        // cc[p][s] = g°_ps/RT + [ln(P/P°) if gas].
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

        // Atom-balance targets from the feed.
        let b_target = self.element_abundance(feed)?;

        // Positive working estimate, seeded from the feed (zeros -> mole_floor).
        let mut moles: Vec<Vec<f64>> = self
            .phases
            .iter()
            .enumerate()
            .map(|(p, ph)| {
                (0..ph.n_species)
                    .map(|s| {
                        let f = feed[p][s];
                        if f > options.mole_floor {
                            f
                        } else {
                            options.mole_floor
                        }
                    })
                    .collect()
            })
            .collect();

        // μ_ps/RT scratch, mirrors `moles` shape.
        let mut mu_rt: Vec<Vec<f64>> = moles.iter().map(|v| vec![0.0; v.len()]).collect();

        let mut history: Vec<f64> = Vec::with_capacity(options.max_iter + 1);
        let mut merit_history: Vec<f64> = Vec::with_capacity(options.max_iter + 1);
        let mut last_max_delta = f64::INFINITY;
        let mut last_atom_res = f64::INFINITY;
        let mut converged = false;
        let mut iterations = 0;

        let dim = m + np;

        for iter in 0..options.max_iter {
            iterations = iter + 1;

            // Phase totals and μ_ps/RT.
            let mut n_p = vec![0.0; np];
            for (p, ph) in self.phases.iter().enumerate() {
                let nt: f64 = moles[p].iter().sum();
                n_p[p] = nt;
                for s in 0..ph.n_species {
                    mu_rt[p][s] = cc[p][s] + (moles[p][s] / nt).ln();
                }
            }

            // Record G/RT of the entering iterate for the monotonicity trajectory.
            let mut g_rt = 0.0;
            for p in 0..np {
                for s in 0..self.phases[p].n_species {
                    g_rt += moles[p][s] * mu_rt[p][s];
                }
            }
            history.push(g_rt);
            merit_history.push(g_rt + MERIT_PENALTY * self.atom_violation(&moles, &b_target));

            // ---- assemble the (M+P) saddle system ----
            //   [[R, Q],[Qᵀ, 0]] · [π; u] = [ (b-q)+S ; F ]
            let mut mat = vec![0.0; dim * dim];
            let mut rhs = vec![0.0; dim];

            // Q_kp (element abundance per phase), F_p (phase Gibbs), q_k, S_k.
            let mut q_kp = vec![0.0; m * np]; // row-major [k*np + p]
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

            // R_kj = Σ_p Σ_s a_ks a_js n_ps (symmetric M×M).
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
            // Top-right Q and bottom-left Qᵀ blocks; element-row RHS.
            for k in 0..m {
                for p in 0..np {
                    let qkp = q_kp[k * np + p];
                    mat[k * dim + (m + p)] = qkp; // Q
                    mat[(m + p) * dim + k] = qkp; // Qᵀ
                }
                rhs[k] = (b_target[k] - q_k[k]) + s_k[k];
            }
            // Phase rows RHS = F_p; bottom-right block stays 0.
            for p in 0..np {
                rhs[m + p] = f_p[p];
            }

            // Freeze vanished phases: replace row (M+p) with identity u_p = 0.
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

            let sol =
                solve_linear(&mut mat, &mut rhs, dim).ok_or(MultiPhaseError::SingularSystem)?;
            let pi = &sol[0..m];
            let u = &sol[m..m + np];

            // δ_ps = Σ_k a_ks π_k + u_p − μ_ps/RT.
            //
            // Two magnitudes are tracked:
            //   `step_max` = max|δ| over all species — sets the damping factor.
            //   `conv_max` = the KKT stationarity residual — |δ| for a species
            //     present above `mole_floor`, but only max(δ, 0) for a species
            //     pinned at the floor (a species that equilibrium wants absent
            //     satisfies complementarity with δ ≤ 0, so a negative δ there is
            //     *converged*, not an error; without this an unstable/vanished
            //     phase's species would keep a huge negative δ and block the
            //     convergence test forever).
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
                let phi_trial = self.total_gibbs_rt(&trial, &cc)
                    + MERIT_PENALTY * self.atom_violation(&trial, &b_target);
                if phi_trial.is_finite() && phi_trial <= phi_current + slack {
                    break;
                }
                omega *= 0.5;
            }
            moles = trial;

            // Relative atom-balance residual (pre-update q bounds post-update as δ→0).
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
            return Err(MultiPhaseError::NotConverged {
                iterations,
                max_correction: last_max_delta,
                atom_residual: last_atom_res,
            });
        }

        // Final quantities at the converged point.
        let mut phase_totals = vec![0.0; np];
        for (p, ph) in self.phases.iter().enumerate() {
            let nt: f64 = moles[p].iter().sum();
            phase_totals[p] = nt;
            for s in 0..ph.n_species {
                mu_rt[p][s] = cc[p][s] + (moles[p][s] / nt).ln();
            }
        }
        let mut g_rt = 0.0;
        for p in 0..np {
            for s in 0..self.phases[p].n_species {
                g_rt += moles[p][s] * mu_rt[p][s];
            }
        }
        history.push(g_rt);
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

        Ok(MultiPhaseResult {
            moles,
            mole_fractions,
            phase_totals,
            element_potentials,
            gibbs_energy_rt: g_rt,
            gibbs_energy: rt * g_rt,
            gibbs_energy_rt_history: history,
            descent_merit_history: merit_history,
            iterations,
            converged,
        })
    }

    /// Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (μ_ps/RT)` \[mol\]
    /// at a given per-phase composition, with `cc[p][s]` the composition-
    /// independent part of `μ_ps/RT` (`g°_ps/RT` plus any gas `ln(P/P°)`). Used
    /// by the line search to test descent. Assumes every phase total is > 0
    /// (guaranteed by the `1e-300` mole floor).
    fn total_gibbs_rt(&self, moles: &[Vec<f64>], cc: &[Vec<f64>]) -> f64 {
        let mut g = 0.0;
        for (p, ph) in self.phases.iter().enumerate() {
            let nt: f64 = moles[p].iter().sum();
            for s in 0..ph.n_species {
                let mu = cc[p][s] + (moles[p][s] / nt).ln();
                g += moles[p][s] * mu;
            }
        }
        g
    }

    /// L1 atom-balance violation `Σ_k |b_k − Σ_ps a_ks n_ps|` \[mol\] of a
    /// per-phase composition against the element targets `b`. The penalty term of
    /// the line-search merit function.
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
    /// `μ_ps/RT = Σ_k a_ks π_k` relation (mole-weighted, summed over all phases),
    /// for the [`MultiPhaseResult`] report.
    ///
    /// Solves the `M×M` normal equations `(Σ_p Aᵀ N A) π = Σ_p Aᵀ N μ`, which
    /// reproduce the RAND element potentials at the fixed point. Returns zeros if
    /// the normal system is singular (degenerate report only — never affects the
    /// solve).
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
/// In-crate so the multi-phase Gibbs solver needs no BLAS/LAPACK and compiles on
/// Android/Termux. `dim = M + P` is small, so the `O(dim³)` dense solve is
/// negligible next to the `ln` work in the outer loop. (Independent copy of the
/// same routine in [`crate::thermo::gibbs`]; kept local to respect that module's
/// file ownership.)
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
    //! # V&V — analytic verification of the multi-phase Gibbs minimiser
    //!
    //! **Methodology (shared).** Each test builds a multi-phase system, runs
    //! [`MultiPhaseGibbsSystem::minimize`] with ideal phases, and checks the
    //! converged distribution against a *closed-form* multi-phase equilibrium —
    //! the Nernst distribution law, the vapour-pressure relation, or the
    //! reacting equilibrium constant — plus exact atom conservation across all
    //! phases and monotonic non-increase of `G/RT`. These are **verification**
    //! (correct implementation) against algebra, **not** validation against
    //! measured multi-phase equilibrium data — AI-assisted draft, untrusted
    //! until human review (crate `CLAUDE.md`).
    //!
    //! **Monotone-descent witness.** The multiplicative RAND update visits
    //! slightly off-manifold points, so the *physical* `G/RT` can wobble by the
    //! step's second-order atom-balance error (measured worst single-step rise on
    //! the Nernst case: `+5.4e-2`). The rigorously monotone quantity is the
    //! line-search merit `Φ = G/RT + μ·(atom violation)`
    //! ([`MultiPhaseResult::descent_merit_history`]), which the backtracking never
    //! lets rise (measured worst rise ≤ `2.8e-12`, i.e. float noise) and which
    //! `→ G/RT` as the violation `→ 0`. The tests therefore assert (i) `Φ`
    //! strictly non-increasing and (ii) `G/RT` descends overall from the feed
    //! point to the constrained minimum — see [`assert_monotone`].
    //!
    //! **Measured results (2026-08-03, release, this module temporarily wired
    //! into `thermo::mod`, then reverted per the coordinator's file-ownership
    //! rule).** All four tests **passed** under
    //! `cargo test -p outram-park-fork-dwsim-libs --lib --release`. Per-test
    //! measured numbers are in each test's doc. The module stays untrusted
    //! AI-assisted draft pending human V&V.

    use super::*;

    fn ph(model: PhaseModel, names: &[&str], atom: &[&[f64]]) -> PhaseInput {
        PhaseInput {
            model,
            species_names: names.iter().map(|s| (*s).to_string()).collect(),
            atom_matrix: atom.iter().map(|r| r.to_vec()).collect(),
        }
    }

    /// **Methodology (Nernst distribution law — two coexisting solution phases).**
    /// Species `A` distributes between two ideal condensed solutions, each held
    /// open by its own inert. Phase 0 = {A, I0}, phase 1 = {A, I1}; elements
    /// `[Ra, E0, E1]` with `A → Ra`, `I0 → E0`, `I1 → E1`. `g°(A)` differs by
    /// phase so the partition coefficient is
    /// `D = x_{A,1}/x_{A,0} = exp((g°_{A,0} − g°_{A,1})/RT)`. Choosing
    /// `g°_{A,0}=0`, `g°_{A,1} = −RT·ln 2` gives `D = 2`. Feed 1 mol A (all in
    /// phase 0), 1 mol I0, 1 mol I1 at `T = 1000 K`. Closed form: the phase-0 A
    /// amount solves `n²−4n+1=0`, i.e. `n_{A,0} = 2−√3`.
    /// **Measured result (2026-08-03).** Converged in **26 iterations** to
    /// `n_{A,0} = 0.2679492` (= `2−√3`), `n_{A,1} = 0.7320508`;
    /// `x_{A,1}/x_{A,0} = 2.0000000` (= D); `G/RT = −2.341066`; atom balance
    /// `Ra=1, E0=1, E1=1`, all to `< 1e-8`. Merit `Φ` strictly non-increasing
    /// (worst rise `0`); physical `G/RT` descends overall (worst step-wobble
    /// `+5.4e-2`, below the constrained min then back — see the module note).
    #[test]
    fn nernst_partition_two_solution_phases() {
        let t = 1000.0;
        let rt = R * t;
        let g1a = -rt * 2.0_f64.ln(); // gives D = 2
        let sys = MultiPhaseGibbsSystem::new(
            &["Ra", "E0", "E1"],
            &[
                ph(
                    PhaseModel::IdealSolution,
                    &["A", "I0"],
                    &[&[1.0, 0.0], &[0.0, 1.0], &[0.0, 0.0]],
                ),
                ph(
                    PhaseModel::IdealSolution,
                    &["A", "I1"],
                    &[&[1.0, 0.0], &[0.0, 0.0], &[0.0, 1.0]],
                ),
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
                1.0e5,
                1.0e5,
                &MultiPhaseOptions::default(),
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
        assert!((d - 2.0).abs() < 1e-8, "D={d} exp=2");

        let b = sys
            .element_abundance(&[&res.moles[0], &res.moles[1]])
            .unwrap();
        assert!(
            (b[0] - 1.0).abs() < 1e-8 && (b[1] - 1.0).abs() < 1e-8 && (b[2] - 1.0).abs() < 1e-8
        );

        assert_monotone(&res);
    }

    /// **Methodology (vapour pressure over a pure condensed phase).** Species `A`
    /// coexists as an ideal gas (with inert `B`) and as a single-species
    /// condensed phase (`x = 1`, unit activity ⇒ pure solid `A`). Elements
    /// `[Ea, Eb]`. Equilibrium `μ_{A,gas}=μ_{A,cond}` ⇒
    /// `y_A (P/P°) = exp((g°_{A,cond} − g°_{A,gas})/RT) = K_vp`. Choose
    /// `g°_{A,gas}=0`, `g°_{A,cond}=RT·ln 0.3` ⇒ `K_vp = 0.3`, `P = P°`. Feed
    /// 2 mol A (all gas), 1 mol B. With excess A the condensed phase persists;
    /// closed form `y_A = 0.3` ⇒ `n_{A,gas}=0.3/0.7=0.4285714`,
    /// `n_{A,cond}=2−0.4285714=1.5714286`.
    /// **Measured result (2026-08-03).** Converged in **216 iterations** to
    /// `y_A = 0.3000000`, `n_{A,gas}=0.4285714`, `n_{A,cond}=1.5714286`;
    /// `G/RT = −2.764621`; atom balance `Ea=2, Eb=1`, all to `< 1e-8`. Merit `Φ`
    /// non-increasing to `2.8e-12` (float noise); `G/RT` descends overall.
    #[test]
    fn vapor_pressure_over_pure_condensed() {
        let t = 1000.0;
        let rt = R * t;
        let g_cond = rt * 0.3_f64.ln(); // K_vp = 0.3
        let sys = MultiPhaseGibbsSystem::new(
            &["Ea", "Eb"],
            &[
                ph(
                    PhaseModel::IdealGas,
                    &["A", "B"],
                    &[&[1.0, 0.0], &[0.0, 1.0]],
                ),
                ph(PhaseModel::IdealSolution, &["Ac"], &[&[1.0], &[0.0]]),
            ],
        )
        .unwrap();

        let g_gas: [f64; 2] = [0.0, 0.0];
        let g_c: [f64; 1] = [g_cond];
        let feed_gas: [f64; 2] = [2.0, 1.0];
        let feed_c: [f64; 1] = [0.0];
        let res = sys
            .minimize(
                &[&g_gas, &g_c],
                t,
                &[&feed_gas, &feed_c],
                1.0e5,
                1.0e5,
                &MultiPhaseOptions::default(),
            )
            .unwrap();

        assert!(
            (res.mole_fractions[0][0] - 0.3).abs() < 1e-8,
            "yA={}",
            res.mole_fractions[0][0]
        );
        assert!(
            (res.moles[0][0] - 0.3 / 0.7).abs() < 1e-8,
            "nAgas={}",
            res.moles[0][0]
        );
        assert!(
            (res.moles[1][0] - (2.0 - 0.3 / 0.7)).abs() < 1e-8,
            "nAcond={}",
            res.moles[1][0]
        );

        let b = sys
            .element_abundance(&[&res.moles[0], &res.moles[1]])
            .unwrap();
        assert!((b[0] - 2.0).abs() < 1e-8 && (b[1] - 1.0).abs() < 1e-8);

        assert_monotone(&res);
    }

    /// **Methodology (collapse to the single-phase result + reacting gas).** The
    /// water-gas shift `CO + H₂O ⇌ CO₂ + H₂` runs in a gas phase, with a second
    /// candidate condensed-`CO₂` phase whose `g°` is set high (`+3e4 J/mol`) so
    /// that it is unstable and must vanish. The converged gas composition must
    /// then match the single-phase [`crate::thermo::gibbs::GibbsSystem`] solving
    /// the identical shift, confirming the multi-phase minimiser reduces exactly
    /// to the validated single-phase one when only one phase is stable.
    /// `ΔG° = +2000 J/mol`, `T = 1100 K`.
    /// **Measured result (2026-08-03).** Converged in **86 iterations**;
    /// condensed-`CO₂` phase total `≈ 2.9e-53 mol` (vanished, `< 1e-6`); gas moles
    /// `[CO, H₂O, CO₂, H₂] = [0.527307, 0.527307, 0.472693, 0.472693]`, matching
    /// the single-phase `GibbsSystem` result to `1.7e-11` (`< 1e-6`); C, H, O
    /// conserved (`1, 2, 2`) to `< 1e-7`. Merit `Φ` strictly non-increasing
    /// (worst rise `0`).
    #[test]
    fn collapse_to_single_phase_water_gas_shift() {
        use crate::thermo::gibbs::{FugacityModel, GibbsOptions, GibbsSystem};

        let (t, dg) = (1100.0, 2000.0);

        // Single-phase reference.
        let sp = GibbsSystem::new(
            &["CO", "H2O", "CO2", "H2"],
            &["C", "H", "O"],
            &[
                &[1.0, 0.0, 1.0, 0.0],
                &[0.0, 2.0, 0.0, 2.0],
                &[1.0, 1.0, 2.0, 0.0],
            ],
        )
        .unwrap();
        let sp_res = sp
            .minimize(
                &[0.0, 0.0, dg, 0.0],
                t,
                &[1.0, 1.0, 0.0, 0.0],
                1.0e5,
                1.0e5,
                &FugacityModel::IdealGas,
                &GibbsOptions::default(),
            )
            .unwrap();

        // Multi-phase: same gas + unfavourable condensed-CO2 phase.
        let sys = MultiPhaseGibbsSystem::new(
            &["C", "H", "O"],
            &[
                ph(
                    PhaseModel::IdealGas,
                    &["CO", "H2O", "CO2", "H2"],
                    &[
                        &[1.0, 0.0, 1.0, 0.0],
                        &[0.0, 2.0, 0.0, 2.0],
                        &[1.0, 1.0, 2.0, 0.0],
                    ],
                ),
                // condensed CO2 (C:1, O:2), g° huge -> vanishes.
                ph(
                    PhaseModel::IdealSolution,
                    &["CO2c"],
                    &[&[1.0], &[0.0], &[2.0]],
                ),
            ],
        )
        .unwrap();

        let g_gas: [f64; 4] = [0.0, 0.0, dg, 0.0];
        let g_c: [f64; 1] = [3.0e4];
        let feed_gas: [f64; 4] = [1.0, 1.0, 0.0, 0.0];
        let feed_c: [f64; 1] = [0.0];
        let res = sys
            .minimize(
                &[&g_gas, &g_c],
                t,
                &[&feed_gas, &feed_c],
                1.0e5,
                1.0e5,
                &MultiPhaseOptions::default(),
            )
            .unwrap();

        assert!(
            res.phase_totals[1] < 1e-6,
            "condensed phase should vanish: {}",
            res.phase_totals[1]
        );
        for i in 0..4 {
            assert!(
                (res.moles[0][i] - sp_res.moles[i]).abs() < 1e-6,
                "gas[{i}]={} single-phase={}",
                res.moles[0][i],
                sp_res.moles[i]
            );
        }
        let b = sys
            .element_abundance(&[&res.moles[0], &res.moles[1]])
            .unwrap();
        assert!(
            (b[0] - 1.0).abs() < 1e-7 && (b[1] - 2.0).abs() < 1e-7 && (b[2] - 2.0).abs() < 1e-7
        );

        assert_monotone(&res);
    }

    /// **Methodology (linear solver + error paths).** Exercise the in-crate
    /// Gaussian solver on a known 2×2 system, and confirm input-validation errors
    /// fire. **Measured result (2026-08-03).** `[[2,1],[1,3]]·x=[3,5]` ⇒
    /// `x=[0.8,1.4]` to `< 1e-12`; a mis-shaped `gibbs_formation`, a non-positive
    /// temperature, and an all-zero feed each return the matching
    /// [`MultiPhaseError`].
    #[test]
    fn linear_solver_and_error_paths() {
        let mut mat = vec![2.0, 1.0, 1.0, 3.0];
        let mut rhs = vec![3.0, 5.0];
        let x = solve_linear(&mut mat, &mut rhs, 2).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);

        let sys = MultiPhaseGibbsSystem::new(
            &["X"],
            &[ph(PhaseModel::IdealGas, &["A", "B"], &[&[1.0, 1.0]])],
        )
        .unwrap();
        let opts = MultiPhaseOptions::default();
        let g_bad: [f64; 1] = [0.0];
        let g_ok: [f64; 2] = [0.0, 0.0];
        let feed: [f64; 2] = [1.0, 0.0];
        let feed0: [f64; 2] = [0.0, 0.0];
        // mis-shaped gibbs_formation
        assert!(matches!(
            sys.minimize(&[&g_bad], 1000.0, &[&feed], 1e5, 1e5, &opts),
            Err(MultiPhaseError::DimensionMismatch { .. })
        ));
        // non-positive temperature
        assert!(matches!(
            sys.minimize(&[&g_ok], -1.0, &[&feed], 1e5, 1e5, &opts),
            Err(MultiPhaseError::InvalidInput { .. })
        ));
        // all-zero feed
        assert!(matches!(
            sys.minimize(&[&g_ok], 1000.0, &[&feed0], 1e5, 1e5, &opts),
            Err(MultiPhaseError::InvalidInput { .. })
        ));
    }

    /// Monotone-descent V&V check shared by the physical tests.
    ///
    /// Asserts two things about a converged [`MultiPhaseResult`]:
    /// 1. **The line-search merit `Φ` is monotonically non-increasing** to a
    ///    tight relative slack (`1e-9`). `Φ = G/RT + μ·(atom violation)` is the
    ///    quantity the backtracking guarantees never rises, and since the atom
    ///    violation `→ 0` it converges to the physical `G/RT`. This is the
    ///    rigorous monotone-Gibbs-descent witness.
    /// 2. **The physical `G/RT` descends overall** from the feed point to the
    ///    constrained minimum (`history[0] ≥ history.last()`), i.e. minimisation
    ///    actually lowered the Gibbs energy.
    fn assert_monotone(res: &MultiPhaseResult) {
        let merit = &res.descent_merit_history;
        assert!(merit.len() >= 2, "merit history too short");
        let mscale = merit.iter().fold(0.0_f64, |a, &v| a.max(v.abs())).max(1.0);
        for w in merit.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-9 * mscale,
                "merit increased: {} -> {} (slack {})",
                w[0],
                w[1],
                1e-9 * mscale
            );
        }
        let g = &res.gibbs_energy_rt_history;
        assert!(
            *g.last().unwrap() <= g[0] + 1e-9 * mscale,
            "G/RT did not descend overall: {} -> {}",
            g[0],
            g.last().unwrap()
        );
    }
}
