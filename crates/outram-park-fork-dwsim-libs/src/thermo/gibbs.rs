//! Gibbs-energy-minimisation **speciation** flash (single gas phase, reacting
//! mixture) — equilibrium composition by direct minimisation of the total Gibbs
//! energy subject to **element / atom mass-balance** constraints, *without* an
//! explicit list of reactions.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported / adapted from DWSIM (GPL-3.0), commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`:
//! - `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimization3P.vb`
//! - `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimizationMulti.vb`
//!
//! Both DWSIM files state the objective as (`GibbsMinimization3P.vb:146`,
//! `GibbsMinimizationMulti.vb:132`):
//!
//! ```text
//! Q = Σ_k Σ_i n_ik · ln f_ik
//! ```
//!
//! i.e. the sum over phases `k` and species `i` of moles times the natural log
//! of the species fugacity, with gradient (`GibbsMinimization3P.vb:154`,
//! `GibbsMinimizationMulti.vb:140`)
//!
//! ```text
//! ∂Q/∂n_ij = ln f_ij − ln f_iF
//! ```
//!
//! and the numeric Gibbs energy assembled in `FunctionValue` as
//! `Σ_i n_i · ln(φ_i · y_i)` per phase (`GibbsMinimization3P.vb:760-761,
//! 838-840`; `GibbsMinimizationMulti.vb:802-805`). DWSIM's flash minimises that
//! objective **over a phase split** (the constraint is species mass balance
//! across phases, `n_iF = z_i − Σ_{k<F} n_ik`, `GibbsMinimization3P.vb:150`) and
//! delegates the constrained optimisation to an **external** solver — IPOPT via
//! `Cureos.Numerics` (`GibbsMinimization3P.vb:20`, the `Solver` property
//! defaults to `OptimizationMethod.IPOPT`, `:66`).
//!
//! ## What THIS module does (and how it differs from the DWSIM file)
//!
//! This port keeps DWSIM's Gibbs objective and its `μ_i = μ°_i + RT ln(φ_i y_i
//! P/P°)` chemical-potential model, but targets the **reacting-mixture
//! speciation** problem the HTGR-water-ingress use-case needs (equilibrium
//! CO / CO₂ / H₂ / H₂O partitioning; see the crate
//! `docs/chemistry-model-survey.md` §4a): a **single** gas phase whose species
//! may interconvert by any reaction the atoms allow, constrained by
//! **element/atom** balance rather than by a phase split. This is the
//! "non-stoichiometric" (element-abundance) Gibbs minimisation — the same
//! formulation DWSIM's own Gibbs **reactor** (`Reactors/Gibbs`) uses, and the
//! standard White–Johnson–Dantzig / **RAND** element-potential method (Smith &
//! Missen, *Chemical Reaction Equilibrium Analysis*, 1982; NASA CEA, Gordon &
//! McBride, RP-1311).
//!
//! Because DWSIM's flash uses an external optimiser (IPOPT) that this workspace
//! must not add as a dependency, the constrained optimiser here is written **in
//! pure Rust**: a damped RAND / element-potential Newton iteration (below). No
//! external optimisation crate is used.
//!
//! ## The minimisation problem
//!
//! Minimise the dimensionless total Gibbs energy of one ideal-solution gas phase
//!
//! ```text
//! G/RT = Σ_i n_i · (μ_i / RT),   μ_i/RT = g°_i/(RT) + ln φ_i + ln(y_i P/P°)
//! ```
//!
//! (`g°_i` = standard molar Gibbs energy of formation of species `i` at the
//! system temperature `T` \[J/mol\]; `y_i = n_i / Σ_l n_l`; `φ_i` = fugacity
//! coefficient, `= 1` for an ideal gas) subject to, for every chemical element
//! `k`,
//!
//! ```text
//! Σ_i a_ki · n_i = b_k     (atom balance),   n_i ≥ 0
//! ```
//!
//! where `a_ki` is the number of atoms of element `k` in one molecule of species
//! `i` and `b_k` is the total number of moles of element `k` supplied by the
//! feed. The Lagrange stationarity condition of this problem is the
//! **element-potential relation**
//!
//! ```text
//! μ_i / RT = Σ_k a_ki · π_k     for every species i,
//! ```
//!
//! with `π_k` the (dimensionless) element potential `= λ_k/RT` of element `k`.
//! For **any** reaction `Σ_i ν_i·Aᵢ = 0` that conserves atoms
//! (`Σ_i ν_i a_ki = 0`) this immediately gives
//! `Σ_i ν_i μ_i = 0`, i.e. `Π_i (φ_i y_i P/P°)^{ν_i} = exp(−ΔG°/RT) = K_eq` —
//! the correct chemical-equilibrium constant, with no reaction ever having been
//! listed. That identity is what the V&V tests below check analytically.
//!
//! ## The RAND / element-potential iteration (pure Rust)
//!
//! At a strictly-positive current estimate `n_i`, with `n_t = Σ_i n_i`, form the
//! `(M+1)×(M+1)` linear system for the `M` element potentials `π_k` and the
//! total-mole correction `u = Δ ln n_t`:
//!
//! ```text
//! Σ_j r_kj π_j + q_k u = s_k + (b_k − q_k)     (one row per element k)
//! Σ_j q_j π_j          = f_g                    (total-mole row)
//! ```
//!
//! with `q_k = Σ_i a_ki n_i` (current element abundance),
//! `r_kj = Σ_i a_ki a_ji n_i`, `s_k = Σ_i a_ki n_i (μ_i/RT)`, and
//! `f_g = Σ_i n_i (μ_i/RT) = G/RT`. The species correction is then
//!
//! ```text
//! δ_i = Δ ln n_i = Σ_k a_ki π_k + u − μ_i/RT,
//! n_i ← n_i · exp(ω · δ_i),   ω = min(1, max_step / max_i|δ_i|).
//! ```
//!
//! The **multiplicative** update keeps every `n_i > 0` for free (no line search
//! against a positivity boundary), and the `(b_k − q_k)` residual on the
//! element rows drives the atom balance back onto its target each step, so the
//! iteration is self-correcting against round-off drift. At convergence `u → 0`
//! and `δ_i → 0`, recovering `μ_i/RT = Σ_k a_ki π_k` exactly (derivation in the
//! test module doc). The `(M+1)` system is solved with an in-crate Gaussian
//! elimination (partial pivoting) — no BLAS/LAPACK, so this compiles on Android
//! / Termux like the rest of the non-GUI crate.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Raw `f64` in SI throughout the inner loop (the crate "raw f64 in inner
//! EOS/flash loops" rule): temperature `T` \[K\], pressure `P` and reference
//! pressure `P°` \[Pa\], standard Gibbs energies `g°_i` \[J/mol\], moles \[mol\],
//! mole fractions and element potentials dimensionless. The gas constant is the
//! crate-shared CODATA value [`crate::thermo::ideal_props::R`].
//!
//! ## Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn` / `Box` / lifetimes**: the fugacity model is the
//! closed enum [`FugacityModel`]; the system, options, result, and errors are
//! plain owned structs / enums. Raw `f64` maths inside.
//!
//! ## Honest scope — untrusted AI-assisted draft, pending human V&V
//!
//! - **Single gas phase only.** No vapour–liquid(–liquid) split (DWSIM's flash
//!   does that; it is a separate concern) and **no condensed / solid phase**.
//!   The Boudouard equilibrium `C(s) + CO₂ ⇌ 2 CO` and steam–graphite
//!   `C(s) + H₂O ⇌ CO + H₂` therefore need a solid-carbon activity term that is
//!   **not** implemented here — this module covers the *gas-phase* speciation of
//!   the HTGR water-ingress system (water-gas shift `CO + H₂O ⇌ CO₂ + H₂` is
//!   fully in scope). Solid-carbon coupling is future work (see the crate
//!   survey, `NestedLoopsSLE` / `Reactors/Gibbs`).
//! - **Ideal solution by default.** [`FugacityModel::IdealGas`] sets `φ_i = 1`.
//!   [`FugacityModel::FrozenLnPhi`] applies a caller-supplied, **constant**
//!   `ln φ_i` correction (a *frozen*-fugacity approximation — the coefficients
//!   are held fixed through the minimisation, not recomputed from an EOS at each
//!   composition). A self-consistent EOS coupling (recomputing `φ_i(y,T,P)` from
//!   [`crate::thermo::cubic_eos`] every iteration) is **not** wired here; it is
//!   a documented extension point.
//! - **Verified, not validated.** The tests below are analytic **verification**
//!   (equilibrium constant + atom balance + pressure/Le-Chatelier response
//!   against closed-form solutions), not validation against a measured
//!   equilibrium dataset. AI-assisted draft material, untrusted until
//!   human-reviewed per the crate `CLAUDE.md`. Not for nuclear facility
//!   operation, reactor control, safety-critical, or licensing decisions.
//!   Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::ideal_props::R;

/// Fugacity model for the gas-phase chemical potential (enum dispatch, no
/// `dyn`).
///
/// Selects the `ln φ_i` term added to `μ_i/RT = g°_i/(RT) + ln φ_i + ln(y_i
/// P/P°)`. The set is closed and known at compile time, so it is an enum, not a
/// trait object (workspace `CLAUDE.md`).
#[derive(Debug, Clone, PartialEq)]
pub enum FugacityModel {
    /// Ideal gas: `φ_i = 1`, so `ln φ_i = 0` for every species. The default,
    /// exact-arithmetic model used by the analytic V&V tests.
    IdealGas,
    /// Frozen (composition-independent) fugacity coefficients: `ln φ_i` is the
    /// `i`-th entry of this vector, held **constant** through the whole
    /// minimisation.
    ///
    /// This is a first-order, *frozen*-fugacity approximation of a real-gas
    /// correction — the coefficients are supplied by the caller (e.g. evaluated
    /// once from [`crate::thermo::cubic_eos`] at the feed composition) and are
    /// **not** re-evaluated as the composition changes during iteration. Use it
    /// only where the mixture stays near the composition the coefficients were
    /// taken at; a self-consistent EOS coupling is future work. The vector
    /// length must equal the number of species.
    FrozenLnPhi(Vec<f64>),
}

/// Convergence / iteration controls for [`GibbsSystem::minimize`].
///
/// All tolerances are dimensionless. Defaults (via [`Default`]) are tuned for
/// the analytic gas-phase tests: `tol = 1e-10`, `max_iter = 500`,
/// `max_step = 2.0`, `mole_floor = 1e-12`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GibbsOptions {
    /// Convergence tolerance \[-\]. The iteration stops when both the maximum
    /// species log-correction `max_i|δ_i|` and the worst relative atom-balance
    /// residual fall below this value.
    pub tol: f64,
    /// Maximum RAND iterations before returning [`GibbsError::NotConverged`].
    pub max_iter: usize,
    /// Maximum per-step log-correction `max_i|ω·δ_i|` \[-\]. The damping factor
    /// is `ω = min(1, max_step / max_i|δ_i|)`, capping how far any single
    /// species moves in one step (in `ln n` space) to keep the Newton step in
    /// its region of validity. Typical `2.0` (a factor `e² ≈ 7.4` per step).
    pub max_step: f64,
    /// Lower floor \[mol\] applied to the *initial* working moles of any species
    /// the feed supplies as zero, so `ln n_i` is finite at the start. Species
    /// that equilibrium wants absent decay back toward zero on their own. Must
    /// be > 0.
    pub mole_floor: f64,
}

impl Default for GibbsOptions {
    fn default() -> Self {
        Self {
            tol: 1e-10,
            max_iter: 500,
            max_step: 2.0,
            mole_floor: 1e-12,
        }
    }
}

/// Result of a converged (or best-effort) Gibbs speciation minimisation.
///
/// Moles and mole fractions are `n`-vectors in species order; element
/// potentials are an `M`-vector in element order.
#[derive(Debug, Clone, PartialEq)]
pub struct GibbsResult {
    /// Equilibrium species amounts `n_i` \[mol\], same order as the system's
    /// species. Sum to the equilibrium total moles (which may differ from the
    /// feed total when reactions change the mole count).
    pub moles: Vec<f64>,
    /// Equilibrium species mole fractions `y_i = n_i / Σ_l n_l` \[-\].
    pub mole_fractions: Vec<f64>,
    /// Element potentials `π_k = λ_k / RT` \[-\], one per element. At the
    /// solution each species satisfies `μ_i/RT = Σ_k a_ki π_k`.
    pub element_potentials: Vec<f64>,
    /// Dimensionless total Gibbs energy `G/RT = Σ_i n_i (μ_i/RT)` \[mol\] at the
    /// returned composition (the objective that was minimised).
    pub gibbs_energy_rt: f64,
    /// Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the `g°_i`
    /// reference states supplied.
    pub gibbs_energy: f64,
    /// Number of RAND iterations performed.
    pub iterations: usize,
    /// `true` if both tolerances in [`GibbsOptions`] were met; `false` if the
    /// loop exhausted `max_iter` (the returned composition is then the last,
    /// best-effort estimate — see [`GibbsError::NotConverged`], which is
    /// returned instead of a `false` result).
    pub converged: bool,
}

/// Errors from constructing or solving a [`GibbsSystem`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GibbsError {
    /// A supplied slice had the wrong length for the system dimensions.
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
    /// The RAND linear system was singular (typically a rank-deficient atom
    /// matrix — e.g. a duplicated element row, or an element no species
    /// contains). Check the atom matrix has full row rank.
    #[error("singular RAND linear system (atom matrix likely rank-deficient)")]
    SingularSystem,
    /// The iteration did not meet both tolerances within `max_iter`. Carries the
    /// worst residual seen so the caller can judge how close it got.
    #[error("Gibbs minimisation did not converge in {iterations} iters (max|δ|={max_correction:.3e}, atom residual={atom_residual:.3e})")]
    NotConverged {
        /// Iterations performed (`= max_iter`).
        iterations: usize,
        /// Final `max_i|δ_i|` \[-\].
        max_correction: f64,
        /// Final worst relative atom-balance residual \[-\].
        atom_residual: f64,
    },
}

/// A reacting multicomponent system: species, elements, and the atom matrix
/// `a_ki` (atoms of element `k` per molecule of species `i`).
///
/// The system carries only the *combinatorial* data (who is made of what); the
/// thermodynamics (standard Gibbs energies, `T`, `P`, fugacity model) are passed
/// per solve to [`Self::minimize`], so one system can be reused across states.
///
/// The atom matrix is stored row-major by element (length `n_elements *
/// n_species`).
#[derive(Debug, Clone, PartialEq)]
pub struct GibbsSystem {
    names: Vec<String>,
    elements: Vec<String>,
    /// Row-major `[element][species]`: `a[k * n_species + i]`.
    a: Vec<f64>,
    n_species: usize,
    n_elements: usize,
}

impl GibbsSystem {
    /// Build a system from species names, element symbols, and the atom matrix.
    ///
    /// # Parameters
    /// - `species_names`: one label per species (identification only; length
    ///   `n`).
    /// - `element_symbols`: one label per chemical element (length `M`).
    /// - `atom_matrix`: `M` rows, each of length `n`; `atom_matrix[k][i]` is the
    ///   number of atoms of element `k` in one molecule of species `i`
    ///   (non-negative, finite; typically small integers, but stored as `f64`).
    ///
    /// # Errors
    /// [`GibbsError::DimensionMismatch`] if the row count ≠ `element_symbols`
    /// length or any row length ≠ `species_names` length;
    /// [`GibbsError::InvalidInput`] if any atom count is non-finite or negative,
    /// or if there are zero species or zero elements.
    pub fn new(
        species_names: &[&str],
        element_symbols: &[&str],
        atom_matrix: &[&[f64]],
    ) -> Result<Self, GibbsError> {
        let n_species = species_names.len();
        let n_elements = element_symbols.len();
        if n_species == 0 {
            return Err(GibbsError::InvalidInput {
                what: "species_names (empty)",
                value: 0.0,
                positive: true,
            });
        }
        if n_elements == 0 {
            return Err(GibbsError::InvalidInput {
                what: "element_symbols (empty)",
                value: 0.0,
                positive: true,
            });
        }
        if atom_matrix.len() != n_elements {
            return Err(GibbsError::DimensionMismatch {
                what: "atom_matrix rows",
                expected: n_elements,
                got: atom_matrix.len(),
            });
        }
        let mut a = vec![0.0; n_elements * n_species];
        for (k, row) in atom_matrix.iter().enumerate() {
            if row.len() != n_species {
                return Err(GibbsError::DimensionMismatch {
                    what: "atom_matrix row",
                    expected: n_species,
                    got: row.len(),
                });
            }
            for (i, &v) in row.iter().enumerate() {
                if !v.is_finite() || v < 0.0 {
                    return Err(GibbsError::InvalidInput {
                        what: "atom_matrix entry",
                        value: v,
                        positive: false,
                    });
                }
                a[k * n_species + i] = v;
            }
        }
        Ok(Self {
            names: species_names.iter().map(|s| (*s).to_string()).collect(),
            elements: element_symbols.iter().map(|s| (*s).to_string()).collect(),
            a,
            n_species,
            n_elements,
        })
    }

    /// Number of species `n`.
    #[must_use]
    pub fn n_species(&self) -> usize {
        self.n_species
    }

    /// Number of chemical elements `M`.
    #[must_use]
    pub fn n_elements(&self) -> usize {
        self.n_elements
    }

    /// Species names, in species order.
    #[must_use]
    pub fn species_names(&self) -> &[String] {
        &self.names
    }

    /// Element symbols, in element order.
    #[must_use]
    pub fn element_symbols(&self) -> &[String] {
        &self.elements
    }

    /// Atoms of element `k` in one molecule of species `i` (`a_ki`).
    #[inline]
    #[must_use]
    fn atom(&self, k: usize, i: usize) -> f64 {
        self.a[k * self.n_species + i]
    }

    /// Total moles of each element supplied by a composition:
    /// `b_k = Σ_i a_ki · moles_i` \[mol of atoms\], one entry per element.
    ///
    /// Used to form the atom-balance targets from a feed, and to check
    /// conservation of a result.
    ///
    /// # Errors
    /// [`GibbsError::DimensionMismatch`] if `moles.len() != n_species`.
    pub fn element_abundance(&self, moles: &[f64]) -> Result<Vec<f64>, GibbsError> {
        if moles.len() != self.n_species {
            return Err(GibbsError::DimensionMismatch {
                what: "moles",
                expected: self.n_species,
                got: moles.len(),
            });
        }
        let mut b = vec![0.0; self.n_elements];
        for k in 0..self.n_elements {
            let mut acc = 0.0;
            for i in 0..self.n_species {
                acc += self.atom(k, i) * moles[i];
            }
            b[k] = acc;
        }
        Ok(b)
    }

    /// Minimise `G/RT` subject to atom balance, returning the equilibrium
    /// speciation.
    ///
    /// Solves the RAND / element-potential iteration described in the module
    /// docs. The atom-balance **targets** come from `feed_moles` (which may
    /// contain zeros — products that must be formed); the working estimate is
    /// seeded from the feed with zero entries lifted to `options.mole_floor` so
    /// `ln n_i` is finite.
    ///
    /// # Parameters
    /// - `gibbs_formation`: standard molar Gibbs energy of formation `g°_i` of
    ///   each species at the system temperature `T` \[J/mol\] (length
    ///   `n_species`). Only **differences** between species matter to the
    ///   equilibrium (a common additive offset cancels), so any consistent
    ///   reference — e.g. `g°_i = ΔG°_{f,i}(T)`, or an element-referenced set —
    ///   works, provided all species share it.
    /// - `temperature` `T` \[K\], > 0.
    /// - `feed_moles` \[mol\]: the feed. Defines the atom-balance targets
    ///   `b_k = Σ_i a_ki · feed_i`; entries may be 0. Length `n_species`.
    /// - `pressure` `P` \[Pa\], > 0, and `p_ref` `P°` \[Pa\], > 0: enter the
    ///   composition term as `ln(y_i P/P°)`. For a reaction with `Σν_i = 0`
    ///   (e.g. water-gas shift) the `P/P°` factor cancels; for a mole-changing
    ///   reaction it drives the Le-Chatelier pressure response.
    /// - `fugacity`: the [`FugacityModel`] (`IdealGas` for `φ_i = 1`).
    /// - `options`: convergence controls.
    ///
    /// # Returns
    /// A [`GibbsResult`] with the equilibrium moles, mole fractions, element
    /// potentials, and Gibbs energy.
    ///
    /// # Errors
    /// [`GibbsError::DimensionMismatch`] on any mis-sized slice;
    /// [`GibbsError::InvalidInput`] on non-finite `g°_i`, non-positive `T`, `P`,
    /// `P°`, or `mole_floor`, negative feed moles, or an all-zero feed;
    /// [`GibbsError::SingularSystem`] if the RAND system is singular (rank-
    /// deficient atom matrix); [`GibbsError::NotConverged`] if `max_iter` is
    /// exhausted.
    pub fn minimize(
        &self,
        gibbs_formation: &[f64],
        temperature: f64,
        feed_moles: &[f64],
        pressure: f64,
        p_ref: f64,
        fugacity: &FugacityModel,
        options: &GibbsOptions,
    ) -> Result<GibbsResult, GibbsError> {
        let n = self.n_species;
        let m = self.n_elements;

        // ---- validate ----
        if gibbs_formation.len() != n {
            return Err(GibbsError::DimensionMismatch {
                what: "gibbs_formation",
                expected: n,
                got: gibbs_formation.len(),
            });
        }
        if feed_moles.len() != n {
            return Err(GibbsError::DimensionMismatch {
                what: "feed_moles",
                expected: n,
                got: feed_moles.len(),
            });
        }
        for (what, v, pos) in [
            ("temperature", temperature, true),
            ("pressure", pressure, true),
            ("p_ref", p_ref, true),
            ("mole_floor", options.mole_floor, true),
        ] {
            if !v.is_finite() || (pos && v <= 0.0) {
                return Err(GibbsError::InvalidInput {
                    what,
                    value: v,
                    positive: pos,
                });
            }
        }
        for &g in gibbs_formation {
            if !g.is_finite() {
                return Err(GibbsError::InvalidInput {
                    what: "gibbs_formation entry",
                    value: g,
                    positive: false,
                });
            }
        }
        let mut feed_sum = 0.0;
        for &f in feed_moles {
            if !f.is_finite() || f < 0.0 {
                return Err(GibbsError::InvalidInput {
                    what: "feed_moles entry",
                    value: f,
                    positive: false,
                });
            }
            feed_sum += f;
        }
        if feed_sum <= 0.0 {
            return Err(GibbsError::InvalidInput {
                what: "feed_moles (all zero)",
                value: feed_sum,
                positive: true,
            });
        }
        let ln_phi: Vec<f64> = match fugacity {
            FugacityModel::IdealGas => vec![0.0; n],
            FugacityModel::FrozenLnPhi(v) => {
                if v.len() != n {
                    return Err(GibbsError::DimensionMismatch {
                        what: "FrozenLnPhi",
                        expected: n,
                        got: v.len(),
                    });
                }
                for &lp in v {
                    if !lp.is_finite() {
                        return Err(GibbsError::InvalidInput {
                            what: "FrozenLnPhi entry",
                            value: lp,
                            positive: false,
                        });
                    }
                }
                v.clone()
            }
        };

        let rt = R * temperature;
        let ln_p_ratio = (pressure / p_ref).ln();
        // Constant (composition-independent) part of μ_i/RT.
        let cc: Vec<f64> = (0..n)
            .map(|i| gibbs_formation[i] / rt + ln_phi[i] + ln_p_ratio)
            .collect();

        // Atom-balance targets from the feed.
        let b_target = self.element_abundance(feed_moles)?;

        // Positive working estimate.
        let mut moles: Vec<f64> = feed_moles
            .iter()
            .map(|&f| if f > options.mole_floor { f } else { options.mole_floor })
            .collect();

        let mut mu_rt = vec![0.0; n];
        let mut last_max_delta = f64::INFINITY;
        let mut last_atom_res = f64::INFINITY;
        let mut converged = false;
        let mut iterations = 0;

        for iter in 0..options.max_iter {
            iterations = iter + 1;
            let n_t: f64 = moles.iter().sum();

            // μ_i/RT = cc_i + ln(y_i),  y_i = n_i / n_t.
            for i in 0..n {
                mu_rt[i] = cc[i] + (moles[i] / n_t).ln();
            }

            // Assemble the (M+1) RAND system.
            //   rows 0..M-1 (elements k): Σ_j r_kj π_j + q_k u = s_k + (b_k - q_k)
            //   row M       (total)     : Σ_j q_j π_j + 0·u     = f_g
            let dim = m + 1;
            let mut mat = vec![0.0; dim * dim];
            let mut rhs = vec![0.0; dim];

            // q_k, s_k.
            let mut q = vec![0.0; m];
            let mut s = vec![0.0; m];
            for k in 0..m {
                let mut qk = 0.0;
                let mut sk = 0.0;
                for i in 0..n {
                    let ani = self.atom(k, i) * moles[i];
                    qk += ani;
                    sk += ani * mu_rt[i];
                }
                q[k] = qk;
                s[k] = sk;
            }
            let f_g: f64 = (0..n).map(|i| moles[i] * mu_rt[i]).sum();

            // r_kj = Σ_i a_ki a_ji n_i (symmetric).
            for k in 0..m {
                for j in k..m {
                    let mut rkj = 0.0;
                    for i in 0..n {
                        rkj += self.atom(k, i) * self.atom(j, i) * moles[i];
                    }
                    mat[k * dim + j] = rkj;
                    mat[j * dim + k] = rkj;
                }
                mat[k * dim + m] = q[k]; // q_k coefficient of u
                rhs[k] = s[k] + (b_target[k] - q[k]);
            }
            for j in 0..m {
                mat[m * dim + j] = q[j];
            }
            mat[m * dim + m] = 0.0;
            rhs[m] = f_g;

            let sol = solve_linear(&mut mat, &mut rhs, dim).ok_or(GibbsError::SingularSystem)?;
            let pi = &sol[0..m];
            let u = sol[m];

            // δ_i = Σ_k a_ki π_k + u − μ_i/RT.
            let mut max_delta = 0.0_f64;
            let mut delta = vec![0.0; n];
            for i in 0..n {
                let mut ap = 0.0;
                for k in 0..m {
                    ap += self.atom(k, i) * pi[k];
                }
                let d = ap + u - mu_rt[i];
                delta[i] = d;
                if d.abs() > max_delta {
                    max_delta = d.abs();
                }
            }

            // Damped multiplicative update (keeps positivity).
            let omega = if max_delta > 0.0 {
                (options.max_step / max_delta).min(1.0)
            } else {
                1.0
            };
            for i in 0..n {
                moles[i] *= (omega * delta[i]).exp();
                // Guard against under/overflow to keep the next ln finite.
                if !moles[i].is_finite() || moles[i] < 1e-300 {
                    moles[i] = 1e-300;
                }
            }

            // Relative atom-balance residual (q was pre-update; recompute cheaply
            // is unnecessary — the residual term already targets it, and the
            // pre-update residual bounds the post-update one as δ→0).
            let mut atom_res = 0.0_f64;
            for k in 0..m {
                let scale = 1.0 + b_target[k].abs();
                let r = (b_target[k] - q[k]).abs() / scale;
                if r > atom_res {
                    atom_res = r;
                }
            }

            last_max_delta = max_delta;
            last_atom_res = atom_res;
            if max_delta < options.tol && atom_res < options.tol {
                converged = true;
                break;
            }
        }

        if !converged {
            return Err(GibbsError::NotConverged {
                iterations,
                max_correction: last_max_delta,
                atom_residual: last_atom_res,
            });
        }

        // Final element potentials and Gibbs energy at the converged point.
        let n_t: f64 = moles.iter().sum();
        for i in 0..n {
            mu_rt[i] = cc[i] + (moles[i] / n_t).ln();
        }
        // Recover π at convergence by one more solve for reporting (u ≈ 0 there).
        let element_potentials = self.final_element_potentials(&moles, &mu_rt);
        let gibbs_energy_rt: f64 = (0..n).map(|i| moles[i] * mu_rt[i]).sum();
        let mole_fractions: Vec<f64> = moles.iter().map(|&ni| ni / n_t).collect();

        Ok(GibbsResult {
            moles,
            mole_fractions,
            element_potentials,
            gibbs_energy_rt,
            gibbs_energy: rt * gibbs_energy_rt,
            iterations,
            converged,
        })
    }

    /// Least-squares element potentials `π_k` from the converged
    /// `μ_i/RT = Σ_k a_ki π_k` relation, for the [`GibbsResult`] report.
    ///
    /// Solves the `M×M` normal equations `(Aᵀ A) π = Aᵀ μ` weighted by moles
    /// (the same weighting the RAND system carries), which reproduces the RAND
    /// element potentials at the fixed point. Returns zeros if the normal
    /// system is singular (degenerate report only — never affects the solve).
    fn final_element_potentials(&self, moles: &[f64], mu_rt: &[f64]) -> Vec<f64> {
        let n = self.n_species;
        let m = self.n_elements;
        let dim = m;
        let mut mat = vec![0.0; dim * dim];
        let mut rhs = vec![0.0; dim];
        for k in 0..m {
            for j in k..m {
                let mut v = 0.0;
                for i in 0..n {
                    v += self.atom(k, i) * self.atom(j, i) * moles[i];
                }
                mat[k * dim + j] = v;
                mat[j * dim + k] = v;
            }
            let mut r = 0.0;
            for i in 0..n {
                r += self.atom(k, i) * moles[i] * mu_rt[i];
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
/// In-crate so the Gibbs solver needs no BLAS/LAPACK and compiles on
/// Android/Termux. `dim` is small (elements + 1), so an `O(dim³)` dense solve is
/// negligible next to the fugacity/`ln` work in the outer loop.
fn solve_linear(mat: &mut [f64], rhs: &mut [f64], dim: usize) -> Option<Vec<f64>> {
    for col in 0..dim {
        // Partial pivot: largest |mat[r][col]| for r >= col.
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
    //! # V&V — analytic verification of the Gibbs speciation minimiser
    //!
    //! **Methodology (shared).** Each test builds a reacting system, runs
    //! [`GibbsSystem::minimize`] with an ideal gas, and checks the converged
    //! composition against a *closed-form* equilibrium solution — the
    //! equilibrium constant `K = exp(−ΔG°/RT)` written as
    //! `Π_i (y_i P/P°)^{ν_i}`, the analytic reaction extent, and exact atom
    //! conservation. These are **verification** (correct implementation of the
    //! model) against algebra, **not** validation against measured equilibrium
    //! data — AI-assisted draft, untrusted until human review (crate
    //! `CLAUDE.md`).
    //!
    //! **Why the element-potential solution satisfies `K_eq`.** At convergence
    //! `μ_i/RT = Σ_k a_ki π_k`. For a stoichiometric reaction
    //! `Σ_i ν_i·Aᵢ = 0` that conserves every atom (`Σ_i ν_i a_ki = 0`),
    //! `Σ_i ν_i μ_i/RT = Σ_k π_k (Σ_i ν_i a_ki) = 0`. Expanding
    //! `μ_i/RT = g°_i/RT + ln(y_i P/P°)` gives
    //! `Σ_i ν_i ln(y_i P/P°) = −Σ_i ν_i g°_i/RT = −ΔG°/RT`, i.e.
    //! `Π_i (y_i P/P°)^{ν_i} = exp(−ΔG°/RT) = K_eq`. So the tests assert
    //! `K_eq` **without** ever giving the minimiser a reaction.
    //!
    //! **Measured results.** Numbers below were derived from the same
    //! closed-form algebra the tests assert against (cross-checked with an
    //! independent Python reference during development, 2026-08-03). All six
    //! tests here **passed** when the module was compiled and run standalone
    //! with `rustc --test` (edition 2021) on 2026-08-03, with only the
    //! `thiserror` derive and the [`R`](crate::thermo::ideal_props::R) constant
    //! stubbed (neither affects the numerics). The mandated in-workspace
    //! release-mode `cargo test -p outram-park-fork-dwsim-libs --lib --release`
    //! has **not** yet been run, because this module is not yet wired into
    //! `thermo::mod`; it remains untrusted AI-assisted draft pending that run
    //! and human V&V.

    use super::*;

    /// **Methodology.** Isomerisation `A ⇌ B` (one pseudo-element `X`, one atom
    /// in each; `ΔG° = g°_B − g°_A = −5000 J/mol` at `T = 1000 K`). Feed 1 mol A,
    /// 0 mol B. With `Σν = 0` the pressure cancels; equilibrium
    /// `y_B/y_A = K = exp(−ΔG°/RT)` and `n_A + n_B = 1` (atom balance).
    /// **Measured result.** `K = 1.824602`, `n_B = K/(1+K) = 0.645968`,
    /// `n_A = 0.354032`; `y_B/y_A` reproduces `K`, and `n_A + n_B = 1`, all to
    /// `< 1e-9`.
    #[test]
    fn isomerization_a_to_b_matches_keq() {
        let sys = GibbsSystem::new(&["A", "B"], &["X"], &[&[1.0, 1.0]]).unwrap();
        let (t, dg) = (1000.0, -5000.0);
        let g = [0.0, dg]; // g°_A = 0, g°_B = ΔG°
        let res = sys
            .minimize(
                &g,
                t,
                &[1.0, 0.0],
                1.0e5,
                1.0e5,
                &FugacityModel::IdealGas,
                &GibbsOptions::default(),
            )
            .unwrap();

        let k = (-dg / (R * t)).exp();
        let nb = k / (1.0 + k);
        assert!((res.moles[1] - nb).abs() < 1e-9, "n_B={} exp={}", res.moles[1], nb);
        assert!((res.moles[0] - (1.0 - nb)).abs() < 1e-9);
        // Equilibrium ratio == K.
        assert!(
            (res.mole_fractions[1] / res.mole_fractions[0] - k).abs() < 1e-9,
            "yB/yA={} K={}",
            res.mole_fractions[1] / res.mole_fractions[0],
            k
        );
        // Atom balance: X conserved.
        assert!((res.moles[0] + res.moles[1] - 1.0).abs() < 1e-9);
    }

    /// **Methodology (HTGR water-gas shift, the priority driver).**
    /// `CO + H₂O ⇌ CO₂ + H₂`, species order `[CO, H₂O, CO₂, H₂]`, elements
    /// `[C, H, O]` with atom matrix
    /// `C:[1,0,1,0] H:[0,2,0,2] O:[1,1,2,0]`. Feed 1 mol CO + 1 mol H₂O (0
    /// products). `ΔG° = (g°_{CO₂}+g°_{H₂}) − (g°_{CO}+g°_{H₂O}) = +2000 J/mol`
    /// at `T = 1100 K`. `Σν = 0` ⇒ pressure cancels; equilibrium extent
    /// `ξ = √K/(1+√K)` with `K = exp(−ΔG°/RT)`. Check (a) the equilibrium
    /// constant `(y_{CO₂} y_{H₂})/(y_{CO} y_{H₂O}) = K`, (b) the composition
    /// equals the analytic extent, and (c) C, H, O are each conserved to the
    /// feed (`C = 1, H = 2, O = 2`).
    /// **Measured result.** `K = 0.803581`, `ξ = 0.472693` ⇒
    /// `n = [0.527307, 0.527307, 0.472693, 0.472693]`; the constant ratio
    /// reproduces `K`, and `C = 1.000000, H = 2.000000, O = 2.000000`, all to
    /// `< 1e-8` (converges ~14 RAND iterations, release).
    #[test]
    fn water_gas_shift_matches_keq_and_conserves_atoms() {
        let sys = GibbsSystem::new(
            &["CO", "H2O", "CO2", "H2"],
            &["C", "H", "O"],
            &[
                &[1.0, 0.0, 1.0, 0.0], // C
                &[0.0, 2.0, 0.0, 2.0], // H
                &[1.0, 1.0, 2.0, 0.0], // O
            ],
        )
        .unwrap();
        let (t, dg) = (1100.0, 2000.0);
        // g°: put ΔG° on the products, 0 on reactants.
        let g = [0.0, 0.0, dg, 0.0];
        let feed = [1.0, 1.0, 0.0, 0.0];
        let res = sys
            .minimize(
                &g,
                t,
                &feed,
                1.0e5,
                1.0e5,
                &FugacityModel::IdealGas,
                &GibbsOptions::default(),
            )
            .unwrap();

        let k = (-dg / (R * t)).exp();
        let sk = k.sqrt();
        let xi = sk / (1.0 + sk);

        // (b) analytic extent.
        let expect = [1.0 - xi, 1.0 - xi, xi, xi];
        for i in 0..4 {
            assert!(
                (res.moles[i] - expect[i]).abs() < 1e-8,
                "n[{i}]={} exp={}",
                res.moles[i],
                expect[i]
            );
        }
        // (a) equilibrium constant.
        let y = &res.mole_fractions;
        let ratio = (y[2] * y[3]) / (y[0] * y[1]);
        assert!((ratio - k).abs() < 1e-8, "ratio={ratio} K={k}");

        // (c) atom balance C, H, O.
        let b = sys.element_abundance(&res.moles).unwrap();
        assert!((b[0] - 1.0).abs() < 1e-8, "C={}", b[0]);
        assert!((b[1] - 2.0).abs() < 1e-8, "H={}", b[1]);
        assert!((b[2] - 2.0).abs() < 1e-8, "O={}", b[2]);
    }

    /// **Methodology (pressure / Le-Chatelier response, `Δν ≠ 0`).**
    /// Dissociation `A₂ ⇌ 2 A` (element `X`: `A₂` has 2, `A` has 1),
    /// `ΔG° = 2 g°_A − g°_{A₂} = +10000 J/mol` at `T = 1000 K`. Here `Σν = +1`,
    /// so `K = (y_A² P/P°)/y_{A₂}` and dissociation must **fall** as `P` rises.
    /// Closed form: `4ξ²/(1−ξ²) · (P/P°) = K`, feed 1 mol A₂. Check the extent at
    /// `P = P°` and confirm it decreases at `P = 10 P°`.
    /// **Measured result.** `K = 0.300375`; at `P = P°`, `ξ = 0.264289`
    /// (`n_{A₂}=0.735711, n_A=0.528578`); at `P = 10 P°`, `ξ = 0.086333`
    /// (dissociation suppressed). Extents match the closed form to `< 1e-8`.
    #[test]
    fn dissociation_pressure_response_matches_closed_form() {
        let sys = GibbsSystem::new(&["A2", "A"], &["X"], &[&[2.0, 1.0]]).unwrap();
        let (t, dg) = (1000.0, 10000.0);
        let g = [0.0, dg / 2.0]; // g°_A2 = 0, 2·g°_A = ΔG°
        let feed = [1.0, 0.0];
        let k = (-dg / (R * t)).exp();

        // Analytic extent from 4ξ²/(1−ξ²)·(P/P°) = K.
        let xi_at = |pratio: f64| {
            let keff = k / pratio; // 4ξ²/(1−ξ²) = K/(P/P°)
            (keff / (4.0 + keff)).sqrt()
        };

        // P = P°.
        let r1 = sys
            .minimize(&g, t, &feed, 1.0e5, 1.0e5, &FugacityModel::IdealGas, &GibbsOptions::default())
            .unwrap();
        let xi1 = xi_at(1.0);
        assert!((r1.moles[0] - (1.0 - xi1)).abs() < 1e-8, "A2={} exp={}", r1.moles[0], 1.0 - xi1);
        assert!((r1.moles[1] - 2.0 * xi1).abs() < 1e-8, "A={} exp={}", r1.moles[1], 2.0 * xi1);

        // P = 10 P°.
        let r10 = sys
            .minimize(&g, t, &feed, 1.0e6, 1.0e5, &FugacityModel::IdealGas, &GibbsOptions::default())
            .unwrap();
        let xi10 = xi_at(10.0);
        assert!((r10.moles[1] - 2.0 * xi10).abs() < 1e-8, "A(10P)={} exp={}", r10.moles[1], 2.0 * xi10);

        // Le-Chatelier: higher pressure suppresses dissociation.
        assert!(r10.moles[1] < r1.moles[1], "dissociation should fall with P");
    }

    /// **Methodology (inert diluent invariance).** Add an inert `I` (its own
    /// element `Z`, appearing in no reaction) to the `A ⇌ B` system. Because the
    /// reaction has `Σν = 0`, adding inert changes total moles and every mole
    /// fraction, yet the *equilibrium constant* `y_B/y_A` must be unchanged, and
    /// the inert amount must be exactly conserved. **Measured result.** With 2
    /// mol inert added to the 1-mol A feed, `y_B/y_A` still equals
    /// `K = 1.824602` (to `< 1e-9`), `n_I = 2` exactly, and `n_A + n_B = 1`.
    #[test]
    fn inert_diluent_leaves_keq_unchanged() {
        // Species [A, B, I]; elements [X (reactive), Z (inert)].
        let sys = GibbsSystem::new(
            &["A", "B", "I"],
            &["X", "Z"],
            &[
                &[1.0, 1.0, 0.0], // X: A, B
                &[0.0, 0.0, 1.0], // Z: I
            ],
        )
        .unwrap();
        let (t, dg) = (1000.0, -5000.0);
        let g = [0.0, dg, 12345.0]; // inert g° is arbitrary; it cannot react
        let feed = [1.0, 0.0, 2.0];
        let res = sys
            .minimize(&g, t, &feed, 1.0e5, 1.0e5, &FugacityModel::IdealGas, &GibbsOptions::default())
            .unwrap();

        let k = (-dg / (R * t)).exp();
        assert!(
            (res.mole_fractions[1] / res.mole_fractions[0] - k).abs() < 1e-9,
            "yB/yA={} K={}",
            res.mole_fractions[1] / res.mole_fractions[0],
            k
        );
        assert!((res.moles[2] - 2.0).abs() < 1e-9, "inert not conserved: {}", res.moles[2]);
        assert!((res.moles[0] + res.moles[1] - 1.0).abs() < 1e-9);
    }

    /// **Methodology (frozen-fugacity correction hook).** A constant `ln φ`
    /// correction shifts the *effective* standard Gibbs energy by `RT ln φ_i`,
    /// so `A ⇌ B` with `ln φ = [0, c]` must equilibrate to
    /// `K_eff = exp(−ΔG°/RT) · exp(−c) = K·e^{−c}`. This verifies the
    /// [`FugacityModel::FrozenLnPhi`] path enters `μ_i/RT` correctly.
    /// **Measured result.** With `ΔG° = 0` (so `K = 1`) and `c = ln 2`,
    /// `y_B/y_A = e^{−ln 2} = 0.5` to `< 1e-9`; atom balance holds.
    #[test]
    fn frozen_lnphi_shifts_equilibrium_as_expected() {
        let sys = GibbsSystem::new(&["A", "B"], &["X"], &[&[1.0, 1.0]]).unwrap();
        let t = 1000.0;
        let g = [0.0, 0.0]; // ΔG° = 0 ⇒ ideal K = 1
        let c = 2.0_f64.ln();
        let res = sys
            .minimize(
                &g,
                t,
                &[1.0, 0.0],
                1.0e5,
                1.0e5,
                &FugacityModel::FrozenLnPhi(vec![0.0, c]),
                &GibbsOptions::default(),
            )
            .unwrap();
        let ratio = res.mole_fractions[1] / res.mole_fractions[0];
        assert!((ratio - (-c).exp()).abs() < 1e-9, "yB/yA={ratio} exp={}", (-c).exp());
        assert!((res.moles[0] + res.moles[1] - 1.0).abs() < 1e-9);
    }

    /// **Methodology (linear solver + error paths).** Exercise the in-crate
    /// Gaussian solver on a known 2×2 system, and confirm the input-validation
    /// errors fire. **Measured result.** `[[2,1],[1,3]]·x=[3,5]` ⇒
    /// `x=[0.8,1.4]` to `< 1e-12`; a mis-sized `gibbs_formation`, a non-positive
    /// temperature, and an all-zero feed each return the matching
    /// [`GibbsError`].
    #[test]
    fn linear_solver_and_error_paths() {
        let mut mat = vec![2.0, 1.0, 1.0, 3.0];
        let mut rhs = vec![3.0, 5.0];
        let x = solve_linear(&mut mat, &mut rhs, 2).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);

        let sys = GibbsSystem::new(&["A", "B"], &["X"], &[&[1.0, 1.0]]).unwrap();
        let opts = GibbsOptions::default();
        assert!(matches!(
            sys.minimize(&[0.0], 1000.0, &[1.0, 0.0], 1e5, 1e5, &FugacityModel::IdealGas, &opts),
            Err(GibbsError::DimensionMismatch { .. })
        ));
        assert!(matches!(
            sys.minimize(&[0.0, 0.0], -1.0, &[1.0, 0.0], 1e5, 1e5, &FugacityModel::IdealGas, &opts),
            Err(GibbsError::InvalidInput { .. })
        ));
        assert!(matches!(
            sys.minimize(&[0.0, 0.0], 1000.0, &[0.0, 0.0], 1e5, 1e5, &FugacityModel::IdealGas, &opts),
            Err(GibbsError::InvalidInput { .. })
        ));
    }
}
