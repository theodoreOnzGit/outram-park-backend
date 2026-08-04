//! Modified UNIFAC (Dortmund) group-contribution activity-coefficient model.
//!
//! Computes liquid-phase activity coefficients `γ_i` for a mixture from the
//! *group* composition of each molecule, like classic UNIFAC (the Fredenslund
//! "solution-of-groups" idea), but with the two Dortmund modifications of
//! Weidlich & Gmehling (1987) / Gmehling et al. (1993):
//!
//! 1. **Modified combinatorial part** — the Staverman–Guggenheim volume
//!    fraction in the Flory–Huggins term is raised to the power `3/4`, and the
//!    combinatorial is written in the algebraically-compact `1 − J + ln J − …`
//!    form. See [`ln_gamma_combinatorial`].
//! 2. **Temperature-dependent group interactions** — the interaction energy is
//!    a quadratic in temperature, `a_mn(T) = a_mn⁰ + a_mn¹ T + a_mn² T²` (K),
//!    entering `Ψ_mn = exp(−a_mn(T) / T)`. Classic UNIFAC uses a constant
//!    `a_mn`. See [`ModfacParameters::interaction_a`] / [`ModfacParameters::psi`].
//!
//! The Dortmund `R_k` / `Q_k` are *fitted* adjustable parameters (Gmehling et
//! al.), **not** Bondi van-der-Waals volumes/areas, and differ numerically from
//! the classic-UNIFAC table (e.g. Dortmund CH3 and CH2 share `R = 0.6325`).
//!
//! The residual part is structurally identical to classic UNIFAC's
//! group-solution equation — only `Ψ_mn` becomes temperature-dependent — so this
//! module reuses the classic Fredenslund `ln Γ_k` residual form (mirroring the
//! sibling [`crate::thermo::unifac`] port) rather than DWSIM's compact
//! `β/θ/s` residual; the two are algebraically identical.
//!
//! # Port provenance
//!
//! Ported from DWSIM (GPL-3.0), commit `1abf72d`:
//! `DWSIM.Thermodynamics/PropertyPackages/Models/MODFAC.vb`, class `Modfac`.
//! The algebra mirrored here:
//!
//! - molecular `r_i = Σ_k ν_k^i R_k` — `RET_Ri`, `MODFAC.vb:398-410`;
//! - molecular `q_i = Σ_k ν_k^i Q_k` — `RET_Qi`, `MODFAC.vb:412-424`;
//! - group area fraction `e_ki = ν_k^i Q_k / q_i` — `RET_EKI`, `MODFAC.vb:426-438`;
//! - `τ_mk = exp(−(a_mk + b_mk T + c_mk T²) / T)` — `TAU`, `MODFAC.vb:358-396`;
//! - modified combinatorial `ln γ_i^C = 1 − J'_i + ln J'_i
//!   − 5 q_i (1 − J_i/L_i + ln(J_i/L_i))` with `J'_i = r_i^{3/4}/Σ_j x_j r_j^{3/4}`,
//!   `J_i = r_i/Σ_j x_j r_j`, `L_i = q_i/Σ_j x_j q_j` — `GAMMA_MR`,
//!   `MODFAC.vb:283-286`;
//! - residual `ln γ_i^R` — `GAMMA_MR`, `MODFAC.vb:289-296` (compact form; this
//!   port writes the equivalent classic `ln Γ_k` form, see above);
//! - `γ_i = exp(ln γ_i^C + ln γ_i^R)` — `GAMMA_MR`, `MODFAC.vb:297`.
//!
//! # Parameter-table provenance (public-literature subset)
//!
//! The bundled table [`ModfacParameters::dortmund_vle_subset`] is a **small
//! public-literature subset**, not DWSIM's full asset files. Sources (identical
//! to DWSIM's GPL-3.0 `modfac.txt` / `modfac_ip.txt` asset rows, whose own
//! literature headers cite the same papers):
//!
//! - `R_k`, `Q_k` and subgroup→main-group assignments: DWSIM `modfac.txt`
//!   (Modified UNIFAC (Dortmund) subgroup table), rows for CH3/CH2 (main 1),
//!   OH (main 5), H2O (main 7). These are the Gmehling-fitted values of
//!   Weidlich & Gmehling, *Ind. Eng. Chem. Res.* **26**, 1372 (1987) and
//!   Gmehling, Li & Schiller, *Ind. Eng. Chem. Res.* **32**, 178 (1993).
//! - `a_mn⁰, a_mn¹, a_mn²` temperature-polynomial interaction parameters:
//!   DWSIM `modfac_ip.txt`, source-tag `2` = Gmehling, Li & Schiller,
//!   *Ind. Eng. Chem. Res.* **32**, 178 (1993).
//!
//! These tables are published, openly-cited literature data and are permitted
//! under the workspace `DATA_POLICY.md`. The subset covers only alkane (CH2),
//! alcohol (OH) and water (H2O) groups — enough for the alkane / ethanol /
//! water examples and tests here.
//!
//! **Deferred data acquisition:** porting the *full* Dortmund subgroup and
//! `a/b/c` interaction matrix (DWSIM's complete `modfac.txt` /
//! `modfac_ip.txt`, ~45 main groups) is deliberately out of scope for this
//! change and tracked as a separate data-acquisition step.
//!
//! # Honest scope — untrusted AI-assisted draft, verification not validation
//!
//! **This is an untrusted AI-assisted draft pending human V&V.** It has been
//! **verified** (the code reproduces an independent second implementation of the
//! Dortmund equations from the same published parameters, and satisfies the
//! model's exact identities — see the test docs) but **not validated** against
//! experimental phase-equilibrium benchmarks. Modified UNIFAC is itself an
//! approximation. Treat outputs as unverified draft physics.
//!
//! Explicitly **excluded** from this port:
//! - Modified UNIFAC (NIST) — different parameter set (`NISTMFAC.vb`); not here.
//! - Excess-enthalpy / heat-capacity derivatives (`HEX_MIX`, `CPEX_MIX`,
//!   `DLNGAMMA_DT` in the upstream file).
//! - The full parameter matrix (see "Deferred data acquisition" above).
//! - DWSIM's `CheckParameters` missing-pair diagnostic (`MODFAC.vb:316-356`) —
//!   here a missing pair falls through to `a = 0 ⇒ Ψ = 1`, matching `TAU`.
//!
//! # Units
//!
//! All public functions take/return raw `f64` in the DWSIM-internal SI
//! convention (temperature in **K**, mole fractions dimensionless in `[0, 1]`
//! summing to 1). Activity coefficients `γ_i` are dimensionless and `> 0`. This
//! follows the crate `CLAUDE.md` rule of raw documented `f64` in inner
//! thermodynamic loops; the quantities here are plain scalars, so no `uom`
//! type-aliasing is needed (contrast the crate's EOS public surface).

use std::collections::HashMap;

/// Coordination number `z` of the UNIFAC/UNIQUAC lattice (dimensionless).
/// Fixed at 10 (as in classic UNIFAC and the Dortmund model); enters the
/// Staverman–Guggenheim combinatorial correction as `z/2 = 5` (the literal `5`
/// in `MODFAC.vb:286`).
pub const COORDINATION_NUMBER: f64 = 10.0;

/// One Modified-UNIFAC (Dortmund) subgroup's constant parameters.
///
/// A subgroup (e.g. `CH3`, `OH`, `H2O`) belongs to a *main* group; interaction
/// polynomials `a_mn(T)` are indexed by **main** group, while the fitted
/// volume/area are per **subgroup**. Mirrors DWSIM's `ModfacGroup`
/// (`MODFAC.vb:671-749`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModfacSubgroup {
    /// Subgroup id (DWSIM `Secondary_Group`, the `no.` column of `modfac.txt`).
    /// Dimensionless integer key.
    pub subgroup_id: usize,
    /// Main-group id this subgroup belongs to (DWSIM `PrimaryGroup`, the `main`
    /// column). Interaction parameters are looked up on this id.
    pub main_group_id: usize,
    /// Fitted Dortmund group volume `R_k` (dimensionless). Unlike classic
    /// UNIFAC this is an *adjustable* parameter, not a Bondi volume. Valid range
    /// roughly `0.3 … 3`.
    pub r: f64,
    /// Fitted Dortmund group surface area `Q_k` (dimensionless), likewise an
    /// adjustable parameter. Valid range roughly `0 … 3`.
    pub q: f64,
}

/// A molecule expressed as its Dortmund group counts `ν_k` (the caller-supplied
/// molecular structure).
///
/// `groups` is a list of `(subgroup_id, count)` pairs, e.g. ethanol is
/// `[(1, 1.0), (2, 1.0), (14, 1.0)]` = one CH3, one CH2, one OH. Counts are
/// `f64` (they may be fractional for pseudo-components) and must be `≥ 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModfacComponent {
    /// `(subgroup_id, ν_k)` pairs. A subgroup id must exist in the
    /// [`ModfacParameters`] table used with this component.
    pub groups: Vec<(usize, f64)>,
}

impl ModfacComponent {
    /// Construct from `(subgroup_id, count)` pairs.
    pub fn new(groups: Vec<(usize, f64)>) -> Self {
        Self { groups }
    }
}

/// One directional main-group interaction polynomial
/// `a_mn(T) = a + b·T + c·T²` (K).
///
/// Coefficients as tabulated in DWSIM `modfac_ip.txt` (`a` in K, `b` in K/K, `c`
/// in K/K²). Mirrors DWSIM's `InteracParam_aij/bij/cij` triple
/// (`MODFAC.vb:531-536`). Directional: `a_mn ≠ a_nm` in general.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModfacInteraction {
    /// Constant term `a_mn⁰` (K).
    pub a: f64,
    /// Linear coefficient `a_mn¹` (dimensionless, multiplies `T` in K).
    pub b: f64,
    /// Quadratic coefficient `a_mn²` (1/K, multiplies `T²`).
    pub c: f64,
}

/// A Modified-UNIFAC (Dortmund) parameter table: subgroup volume/area
/// parameters plus the temperature-dependent main-group interaction matrix
/// `a_mn(T)` (K).
///
/// Owned by value (`HashMap`s, no lifetimes, no `dyn`), indexed by integer
/// subgroup / main-group ids, per the workspace design rules.
#[derive(Debug, Clone, PartialEq)]
pub struct ModfacParameters {
    subgroups: HashMap<usize, ModfacSubgroup>,
    /// `a_mn(T)` polynomials keyed by `(main_m, main_n)`. Absent pair ⇒ `0.0`
    /// (⇒ `Ψ = 1`), matching DWSIM's `TAU` fall-through (`MODFAC.vb:374-392`).
    interactions: HashMap<(usize, usize), ModfacInteraction>,
}

impl ModfacParameters {
    /// Build an empty table (no subgroups, no interactions).
    pub fn new() -> Self {
        Self {
            subgroups: HashMap::new(),
            interactions: HashMap::new(),
        }
    }

    /// Register a subgroup's `R_k` / `Q_k` and its main-group assignment.
    pub fn add_subgroup(&mut self, subgroup: ModfacSubgroup) {
        self.subgroups.insert(subgroup.subgroup_id, subgroup);
    }

    /// Set the directional main-group interaction polynomial
    /// `a_mn(T) = a + b·T + c·T²` (K). Note `a_mn ≠ a_nm` in general; call twice
    /// for both directions.
    pub fn set_interaction(&mut self, main_m: usize, main_n: usize, a: f64, b: f64, c: f64) {
        self.interactions
            .insert((main_m, main_n), ModfacInteraction { a, b, c });
    }

    /// Look up a subgroup by id.
    pub fn subgroup(&self, subgroup_id: usize) -> Option<&ModfacSubgroup> {
        self.subgroups.get(&subgroup_id)
    }

    /// Directional interaction energy `a_mn(T) = a + b·T + c·T²` (K), evaluated
    /// at `temperature` (K). Returns `0.0` if `main_m == main_n` or the pair is
    /// not tabulated (⇒ `Ψ_mn = 1`), matching DWSIM `TAU` (`MODFAC.vb:366-392`).
    ///
    /// This temperature dependence is the defining residual-side difference from
    /// classic UNIFAC (where `a_mn` is a constant).
    pub fn interaction_a(&self, main_m: usize, main_n: usize, temperature: f64) -> f64 {
        if main_m == main_n {
            return 0.0;
        }
        match self.interactions.get(&(main_m, main_n)) {
            Some(ip) => ip.a + ip.b * temperature + ip.c * temperature * temperature,
            None => 0.0,
        }
    }

    /// Group interaction factor `Ψ_mn = exp(−a_mn(T) / T)` (dimensionless), with
    /// `main_m`, `main_n` main-group ids and `temperature` in K.
    /// Ported from `TAU` (`MODFAC.vb:358-396`).
    pub fn psi(&self, main_m: usize, main_n: usize, temperature: f64) -> f64 {
        (-self.interaction_a(main_m, main_n, temperature) / temperature).exp()
    }

    /// Public-literature subset of the **Modified UNIFAC (Dortmund)** table.
    ///
    /// Covers the alkane main group (CH3/CH2, main group 1), the alcohol OH
    /// group (main 5) and water (main 7) — enough for the alkane / ethanol /
    /// water examples. Values are the Gmehling-fitted Dortmund parameters and
    /// coincide with DWSIM's `modfac.txt` (R/Q) and `modfac_ip.txt`
    /// (`a/b/c`, source-tag `2` = Gmehling, Li & Schiller 1993); see the module
    /// header for the full citation.
    ///
    /// Subgroup ids: CH3 = 1, CH2 = 2, OH = 14, H2O = 16.
    /// Main-group ids: CH3/CH2 = 1, OH = 5, H2O = 7.
    pub fn dortmund_vle_subset() -> Self {
        let mut t = Self::new();

        // Subgroups: (subgroup_id, main_group_id, R_k, Q_k). DWSIM modfac.txt.
        let subs = [
            (1usize, 1usize, 0.6325, 1.0608), // CH3
            (2, 1, 0.6325, 0.7081),           // CH2
            (14, 5, 1.2302, 0.8927),          // OH
            (16, 7, 1.7334, 2.4561),          // H2O
        ];
        for (subgroup_id, main_group_id, r, q) in subs {
            t.add_subgroup(ModfacSubgroup {
                subgroup_id,
                main_group_id,
                r,
                q,
            });
        }

        // Main-group interaction polynomials a_mn(T) = a + bT + cT² (K), DWSIM
        // modfac_ip.txt (source-tag 2). Each row of modfac_ip.txt lists a pair
        // (m, n) with the m→n triple then the n→m triple.
        // Main groups: 1 = CH2, 5 = OH, 7 = H2O.
        // (m, n, a_mn, b_mn, c_mn, a_nm, b_nm, c_nm)
        let pairs = [
            (1, 5, 2777.0, -4.674, 1.551e-3, 1606.0, -4.746, 9.181e-4), // CH2 / OH
            (1, 7, 1391.3, -3.6156, 1.144e-3, -17.253, 0.8389, 9.021e-4), // CH2 / H2O
            (5, 7, -801.9, 3.824, -7.514e-3, 1460.0, -8.673, 1.641e-2),  // OH / H2O
        ];
        for (m, n, a_mn, b_mn, c_mn, a_nm, b_nm, c_nm) in pairs {
            t.set_interaction(m, n, a_mn, b_mn, c_mn);
            t.set_interaction(n, m, a_nm, b_nm, c_nm);
        }

        t
    }
}

impl Default for ModfacParameters {
    fn default() -> Self {
        Self::new()
    }
}

/// Which Modified-UNIFAC (Dortmund) parameter table to use — an enum dispatch
/// point (no `dyn`) so future variants (the full matrix, NIST) slot in as new
/// arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModfacTable {
    /// Dortmund (VLE) subset ([`ModfacParameters::dortmund_vle_subset`]).
    DortmundVle,
}

impl ModfacTable {
    /// Materialise the chosen table's parameters.
    pub fn parameters(self) -> ModfacParameters {
        match self {
            Self::DortmundVle => ModfacParameters::dortmund_vle_subset(),
        }
    }
}

/// Molecular volume `r_i = Σ_k ν_k^i R_k` and surface area `q_i = Σ_k ν_k^i Q_k`
/// (both dimensionless) for one component.
///
/// Ported from `RET_Ri` / `RET_Qi` (`MODFAC.vb:398-424`). Panics if a subgroup
/// id in `component` is missing from `params` — the caller is expected to build
/// components against the same table.
pub fn molecular_r_q(params: &ModfacParameters, component: &ModfacComponent) -> (f64, f64) {
    let mut r = 0.0;
    let mut q = 0.0;
    for &(subgroup_id, nu) in &component.groups {
        let sg = params
            .subgroup(subgroup_id)
            .unwrap_or_else(|| panic!("MODFAC subgroup id {subgroup_id} not in parameter table"));
        r += nu * sg.r;
        q += nu * sg.q;
    }
    (r, q)
}

/// Modified (Dortmund) combinatorial part `ln γ_i^C` for every component
/// (entropic, size/shape term; temperature-independent).
///
/// Dortmund form (`MODFAC.vb:283-286`), which replaces classic UNIFAC's
/// Staverman–Guggenheim combinatorial with a Flory–Huggins term whose volume
/// fraction is raised to the power `3/4`:
///
/// `ln γ_i^C = 1 − J'_i + ln J'_i − 5 q_i (1 − J_i/L_i + ln(J_i/L_i))`
///
/// with the **`3/4`-power** volume fraction `J'_i = r_i^{3/4} / Σ_j x_j r_j^{3/4}`,
/// the ordinary volume fraction `J_i = r_i / Σ_j x_j r_j`, and the surface-area
/// fraction `L_i = q_i / Σ_j x_j q_j`; the literal `5` is `z/2` with `z = 10`.
/// All three fractions are formed without dividing by `x_i`, so `x_i = 0`
/// (infinite dilution) stays finite.
///
/// Reduction check: if all components share the same `r_i` and `q_i`, then
/// `J'_i = J_i = L_i = 1` and `ln γ_i^C = 0` for every composition (see the
/// `identical_molecules_are_ideal` test). Setting the exponent to `1`
/// (`J'_i = J_i`) recovers the classic-UNIFAC combinatorial written in the
/// equivalent `1 − J + ln J − …` algebra.
///
/// `x` are mole fractions (dimensionless, sum ≈ 1); returns one `ln γ_i^C` per
/// component.
pub fn ln_gamma_combinatorial(
    params: &ModfacParameters,
    components: &[ModfacComponent],
    x: &[f64],
) -> Vec<f64> {
    let n = components.len();
    let rq: Vec<(f64, f64)> = components
        .iter()
        .map(|c| molecular_r_q(params, c))
        .collect();

    let sum_x_r: f64 = (0..n).map(|i| x[i] * rq[i].0).sum();
    let sum_x_r34: f64 = (0..n).map(|i| x[i] * rq[i].0.powf(0.75)).sum();
    let sum_x_q: f64 = (0..n).map(|i| x[i] * rq[i].1).sum();
    let z2 = COORDINATION_NUMBER / 2.0;

    (0..n)
        .map(|i| {
            let (r_i, q_i) = rq[i];
            let j_i = r_i / sum_x_r; // J_i  (ordinary volume fraction / x_i)
            let jp_i = r_i.powf(0.75) / sum_x_r34; // J'_i (3/4-power volume fraction / x_i)
            let l_i = q_i / sum_x_q; // L_i  (surface-area fraction / x_i)
            let j_over_l = j_i / l_i;
            1.0 - jp_i + jp_i.ln() - z2 * q_i * (1.0 - j_over_l + j_over_l.ln())
        })
        .collect()
}

/// Residual group-activity `ln Γ_k` for every subgroup present in `group_counts`.
///
/// The classic Fredenslund group-solution equation (unchanged in structure from
/// UNIFAC — only `Ψ` is now temperature-dependent):
///
/// `ln Γ_k = Q_k [ 1 − ln(Σ_m θ_m Ψ_mk) − Σ_m ( θ_m Ψ_km / Σ_n θ_n Ψ_nm ) ]`
///
/// with group area fraction `θ_m = Q_m X_m / Σ_n Q_n X_n` and group mole
/// fraction `X_m`. `Ψ` is on main groups. Returned map is keyed by subgroup id;
/// `temperature` is in K.
///
/// This is the classic-form equivalent of DWSIM's compact `β/θ/s` residual
/// block (`MODFAC.vb:289-296`), used for both the mixture and the
/// pure-component reference in [`ln_gamma_residual`].
pub fn group_ln_gamma(
    params: &ModfacParameters,
    group_counts: &HashMap<usize, f64>,
    temperature: f64,
) -> HashMap<usize, f64> {
    // Group mole fractions X_m.
    let total: f64 = group_counts.values().sum();
    let ids: Vec<usize> = group_counts.keys().copied().collect();

    // Group area fractions θ_m.
    let denom_theta: f64 = ids
        .iter()
        .map(|&m| {
            let q = params.subgroup(m).expect("subgroup in table").q;
            q * group_counts[&m] / total
        })
        .sum();
    let theta: HashMap<usize, f64> = ids
        .iter()
        .map(|&m| {
            let q = params.subgroup(m).expect("subgroup in table").q;
            let x_m = group_counts[&m] / total;
            (m, q * x_m / denom_theta)
        })
        .collect();

    let main = |id: usize| params.subgroup(id).expect("subgroup in table").main_group_id;

    let mut out = HashMap::with_capacity(ids.len());
    for &k in &ids {
        let q_k = params.subgroup(k).expect("subgroup in table").q;
        let mk = main(k);

        // Σ_m θ_m Ψ_mk
        let s1: f64 = ids
            .iter()
            .map(|&m| theta[&m] * params.psi(main(m), mk, temperature))
            .sum();

        // Σ_m [ θ_m Ψ_km / Σ_n θ_n Ψ_nm ]
        let s2: f64 = ids
            .iter()
            .map(|&m| {
                let mm = main(m);
                let inner: f64 = ids
                    .iter()
                    .map(|&nn| theta[&nn] * params.psi(main(nn), mm, temperature))
                    .sum();
                theta[&m] * params.psi(mk, mm, temperature) / inner
            })
            .sum();

        out.insert(k, q_k * (1.0 - s1.ln() - s2));
    }
    out
}

/// Residual part `ln γ_i^R = Σ_k ν_k^i (ln Γ_k − ln Γ_k^i)` for every component
/// (enthalpic, energetic-interaction term; temperature-dependent).
///
/// `ln Γ_k` is the group activity in the actual mixture and `ln Γ_k^i` the group
/// activity in a fluid of pure component `i` (its reference state), both from
/// [`group_ln_gamma`]. Ported from `GAMMA_MR` (`MODFAC.vb:289-296`).
/// `x` are mole fractions; `temperature` is in K.
pub fn ln_gamma_residual(
    params: &ModfacParameters,
    components: &[ModfacComponent],
    x: &[f64],
    temperature: f64,
) -> Vec<f64> {
    let n = components.len();

    // Mixture group counts: Σ_i x_i ν_k^i (any positive scaling cancels in the
    // group mole fractions inside group_ln_gamma).
    let mut mix: HashMap<usize, f64> = HashMap::new();
    for i in 0..n {
        for &(subgroup_id, nu) in &components[i].groups {
            *mix.entry(subgroup_id).or_insert(0.0) += x[i] * nu;
        }
    }
    let ln_gamma_mix = group_ln_gamma(params, &mix, temperature);

    (0..n)
        .map(|i| {
            // Pure-component reference group counts for component i.
            let mut pure: HashMap<usize, f64> = HashMap::new();
            for &(subgroup_id, nu) in &components[i].groups {
                if nu > 0.0 {
                    *pure.entry(subgroup_id).or_insert(0.0) += nu;
                }
            }
            let ln_gamma_pure = group_ln_gamma(params, &pure, temperature);

            components[i]
                .groups
                .iter()
                .filter(|&&(_, nu)| nu > 0.0)
                .map(|&(subgroup_id, nu)| {
                    nu * (ln_gamma_mix[&subgroup_id] - ln_gamma_pure[&subgroup_id])
                })
                .sum()
        })
        .collect()
}

/// Liquid-phase activity coefficients `γ_i = exp(ln γ_i^C + ln γ_i^R)` for every
/// component (dimensionless, `> 0`), from the Modified UNIFAC (Dortmund) model.
///
/// Top-level entry point, ported from `GAMMA_MR` (`MODFAC.vb:58-314`). Inputs:
/// - `params` — the parameter table (e.g. [`ModfacTable::DortmundVle`]);
/// - `components` — each molecule's group counts `ν_k^i`;
/// - `x` — mole fractions (dimensionless, should sum to ≈ 1), same order/length
///   as `components`;
/// - `temperature` — in K (Dortmund UNIFAC is calibrated ~275–425 K; the
///   `a_mn(T)` polynomials are fitted over that range).
///
/// A pure component (`x = [1.0]`) returns `γ = 1` exactly.
pub fn activity_coefficients(
    params: &ModfacParameters,
    components: &[ModfacComponent],
    x: &[f64],
    temperature: f64,
) -> Vec<f64> {
    let ln_c = ln_gamma_combinatorial(params, components, x);
    let ln_r = ln_gamma_residual(params, components, x, temperature);
    ln_c.iter()
        .zip(ln_r.iter())
        .map(|(c, r)| (c + r).exp())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    // Subgroup id aliases for readability in tests.
    const CH3: usize = 1;
    const CH2: usize = 2;
    const OH: usize = 14;
    const H2O: usize = 16;

    fn ethanol() -> ModfacComponent {
        // C2H5OH = 1 CH3 + 1 CH2 + 1 OH
        ModfacComponent::new(vec![(CH3, 1.0), (CH2, 1.0), (OH, 1.0)])
    }
    fn water() -> ModfacComponent {
        ModfacComponent::new(vec![(H2O, 1.0)])
    }

    /// **Methodology.** A pure component (single molecule, `x = 1`) must give
    /// `γ = 1` exactly: both `ln γ^C` and `ln γ^R` vanish because the mixture
    /// *is* the reference state. Checked for ethanol at 298.15 K, Dortmund
    /// subset.
    ///
    /// **Result (2026-08-03, DWSIM modfac.txt / modfac_ip.txt source-tag 2
    /// subset).** `γ = 1.000000000` (deviation `< 1e-12`). Pass.
    #[test]
    fn pure_component_gamma_is_one() {
        let p = ModfacTable::DortmundVle.parameters();
        let g = activity_coefficients(&p, &[ethanol()], &[1.0], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
    }

    /// **Methodology — reduction to a sensible limit.** A mixture of two
    /// molecules with *identical* group composition is thermodynamically ideal:
    /// `r_i`, `q_i` and all group fractions are equal, so the modified
    /// combinatorial collapses (`J'_i = J_i = L_i = 1 ⇒ ln γ^C = 0`) and the
    /// residual vanishes (mixture = reference), giving `γ_i = 1` for every
    /// component and every composition. Checked with two ethanol "copies" at
    /// `x = (0.4, 0.6)`, 298.15 K.
    ///
    /// **Result (2026-08-03).** `γ = (1.000000000, 1.000000000)` (deviation
    /// `< 1e-12`). Pass — confirms the `3/4`-power combinatorial reduces
    /// correctly.
    #[test]
    fn identical_molecules_are_ideal() {
        let p = ModfacTable::DortmundVle.parameters();
        let g = activity_coefficients(&p, &[ethanol(), ethanol()], &[0.4, 0.6], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(g[1], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** For a mixture whose molecules are built entirely from
    /// subgroups of the *same main group* (here n-hexane = 2 CH3 + 4 CH2 and
    /// n-heptane = 2 CH3 + 5 CH2, both only main group 1), every `Ψ_mn = 1`, so
    /// each `ln Γ_k = Q_k(1 − ln 1 − 1) = 0` in both mixture and reference and
    /// the residual part is **exactly zero**. Only the (small) modified
    /// combinatorial size term survives. Checked at `x = (0.5, 0.5)`, 298.15 K.
    ///
    /// **Result (2026-08-03).** `ln γ^R = (0.0, 0.0)` exactly; the surviving
    /// modified-combinatorial coefficients are `γ = (0.99976, 0.99969)` (both
    /// just below 1, as expected for a size-asymmetric athermal alkane
    /// mixture), matching the independent Python reference to `< 1e-9`. Pass.
    #[test]
    fn same_main_group_mixture_has_zero_residual() {
        let p = ModfacTable::DortmundVle.parameters();
        let hexane = ModfacComponent::new(vec![(CH3, 2.0), (CH2, 4.0)]);
        let heptane = ModfacComponent::new(vec![(CH3, 2.0), (CH2, 5.0)]);
        let comps = [hexane, heptane];
        let x = [0.5, 0.5];

        let ln_r = ln_gamma_residual(&p, &comps, &x, 298.15);
        assert_relative_eq!(ln_r[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(ln_r[1], 0.0, epsilon = 1e-12);

        let g = activity_coefficients(&p, &comps, &x, 298.15);
        assert_relative_eq!(g[0], 0.9997626256, epsilon = 1e-9);
        assert_relative_eq!(g[1], 0.9996862492, epsilon = 1e-9);
    }

    /// **Methodology.** Modified (Dortmund) combinatorial part alone, cross-
    /// checked against a by-hand `r/q` calculation for the equimolar
    /// ethanol(1)/water(2) mixture at 298.15 K, using DWSIM Dortmund `R_k`,
    /// `Q_k`.
    ///
    /// Hand values: ethanol `r₁ = 0.6325 + 0.6325 + 1.2302 = 2.4952`,
    /// `q₁ = 1.0608 + 0.7081 + 0.8927 = 2.6616`; water `r₂ = 1.7334`,
    /// `q₂ = 2.4561`. These feed the `3/4`-power volume fraction; the resulting
    /// `ln γ^C` is compared to the independent Python reference.
    ///
    /// **Result (2026-08-03).** `r/q` reproduced exactly; `ln γ₁^C = 0.1022538`
    /// and `ln γ₂^C = 0.1347588`, matching the independent Python reference to
    /// `< 1e-9`. Pass.
    #[test]
    fn combinatorial_matches_hand_rq() {
        let p = ModfacTable::DortmundVle.parameters();

        let (r1, q1) = molecular_r_q(&p, &ethanol());
        assert_relative_eq!(r1, 2.4952, epsilon = 1e-9);
        assert_relative_eq!(q1, 2.6616, epsilon = 1e-9);
        let (r2, q2) = molecular_r_q(&p, &water());
        assert_relative_eq!(r2, 1.7334, epsilon = 1e-9);
        assert_relative_eq!(q2, 2.4561, epsilon = 1e-9);

        let ln_c = ln_gamma_combinatorial(&p, &[ethanol(), water()], &[0.5, 0.5]);
        assert_relative_eq!(ln_c[0], 0.1022538201, epsilon = 1e-9);
        assert_relative_eq!(ln_c[1], 0.1347588059, epsilon = 1e-9);
    }

    /// **Methodology — full model, equimolar.** Equimolar ethanol(1)/water(2)
    /// at 298.15 K, full Dortmund model, against the independent Python
    /// reference implementation of the Dortmund equations from the same
    /// parameters (a two-implementation *verification*, not a benchmark
    /// validation against experiment).
    ///
    /// **Result (2026-08-03).** `γ = (1.25296, 1.43982)` here vs
    /// `(1.252964, 1.439816)` reference — agree `< 1e-4`. Positive deviations
    /// from ideality, as physically expected for ethanol/water. Pass.
    #[test]
    fn ethanol_water_equimolar_matches_reference() {
        let p = ModfacTable::DortmundVle.parameters();
        let g = activity_coefficients(&p, &[ethanol(), water()], &[0.5, 0.5], 298.15);
        assert_relative_eq!(g[0], 1.252964, epsilon = 1e-4);
        assert_relative_eq!(g[1], 1.439816, epsilon = 1e-4);
    }

    /// **Methodology — infinite-dilution `γ∞`, verification cross-check.**
    /// The Modified-UNIFAC (Dortmund) `γ∞` for the ethanol(1)/water(2) system at
    /// 298.15 K, computed with the DWSIM Dortmund subset in this module, is
    /// compared against an **independent Python re-implementation** of the
    /// Dortmund equations; the two agree to `< 1e-3`, so this is a genuine
    /// two-implementation *verification* (the code computes the model
    /// correctly), **not** a benchmark validation against experiment. Infinite
    /// dilution is approximated with the trace mole fraction `x_trace = 1e-8`.
    ///
    /// Model-improvement note: the Dortmund `γ∞(ethanol in water) ≈ 4.79` here
    /// is markedly closer to the experimental range (`≈ 3.7–4.4`, e.g. Kojima
    /// et al.) than the classic-UNIFAC value of `≈ 7.62` produced by the sibling
    /// [`crate::thermo::unifac`] module — a concrete illustration of the
    /// Dortmund improvement over base UNIFAC for aqueous alcohols. (This is an
    /// observation, not a validation claim; no experimental fit was performed
    /// here.)
    ///
    /// **Result (2026-08-03, DWSIM Dortmund subset, T = 298.15 K).**
    /// - `γ∞(ethanol in water) = 4.79182` (this module) vs `4.791821` (Python
    ///   reference) — agree `< 1e-4`.
    /// - `γ∞(water in ethanol) = 2.61125` vs `2.611253` — agree `< 1e-4`.
    ///
    /// Both are physically reasonable (both `> 1`, positive deviation from
    /// Raoult's law). Pass.
    #[test]
    fn ethanol_water_infinite_dilution_matches_reference() {
        let p = ModfacTable::DortmundVle.parameters();
        let t = 298.15;
        let eps = 1e-8;

        // Ethanol infinitely dilute in water.
        let g = activity_coefficients(&p, &[ethanol(), water()], &[eps, 1.0 - eps], t);
        assert_relative_eq!(g[0], 4.791821, epsilon = 1e-4);

        // Water infinitely dilute in ethanol.
        let g = activity_coefficients(&p, &[ethanol(), water()], &[1.0 - eps, eps], t);
        assert_relative_eq!(g[1], 2.611253, epsilon = 1e-4);
    }

    /// **Methodology — temperature dependence is active and directional.**
    /// The defining residual-side feature of Dortmund UNIFAC is the
    /// temperature-dependent interaction `a_mn(T) = a + bT + cT²`. This test
    /// confirms (a) the `a_mn(T)` polynomial actually varies with `T` (so the
    /// `b`, `c` coefficients are wired in, not ignored as in classic UNIFAC),
    /// and (b) it moves `γ∞(ethanol in water)` monotonically over 298.15 →
    /// 323.15 → 353.15 K, matching the independent Python reference at each
    /// temperature. This is a *verification* that the temperature term is
    /// correctly implemented, not a validation of the physical trend.
    ///
    /// **Result (2026-08-03).** `a_{CH2→OH}(T)`: 1521.3 K at 298.15 K vs
    /// 1319.8 K at 353.15 K — the polynomial varies with `T` as intended.
    /// `γ∞(ethanol in water)` = 4.79182 (298.15 K) → 5.32165 (323.15 K) →
    /// 5.76617 (353.15 K), strictly increasing and matching the Python
    /// reference to `< 1e-4` at every point. Pass.
    #[test]
    fn temperature_dependence_is_active_and_directional() {
        let p = ModfacTable::DortmundVle.parameters();
        let eps = 1e-8;

        // (a) the a_mn(T) polynomial genuinely varies with temperature.
        let a_298 = p.interaction_a(1, 5, 298.15); // CH2 -> OH
        let a_353 = p.interaction_a(1, 5, 353.15);
        assert_relative_eq!(a_298, 1521.3205982974998, epsilon = 1e-6);
        assert_relative_eq!(a_353, 1319.8097447975, epsilon = 1e-6);
        assert!(
            (a_298 - a_353).abs() > 50.0,
            "a_mn(T) must vary appreciably with T"
        );

        // (b) γ∞(ethanol in water) is monotonic in T and matches the reference.
        let g298 = activity_coefficients(&p, &[ethanol(), water()], &[eps, 1.0 - eps], 298.15)[0];
        let g323 = activity_coefficients(&p, &[ethanol(), water()], &[eps, 1.0 - eps], 323.15)[0];
        let g353 = activity_coefficients(&p, &[ethanol(), water()], &[eps, 1.0 - eps], 353.15)[0];

        assert_relative_eq!(g298, 4.791821, epsilon = 1e-4);
        assert_relative_eq!(g323, 5.321647, epsilon = 1e-4);
        assert_relative_eq!(g353, 5.766167, epsilon = 1e-4);
        assert!(g298 < g323 && g323 < g353, "γ∞ must be monotonic in T here");
    }
}
