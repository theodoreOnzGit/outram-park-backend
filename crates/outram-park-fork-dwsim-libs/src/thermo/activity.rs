//! NRTL / UNIQUAC / Ideal liquid-phase activity-coefficient models.
//!
//! Ported from DWSIM (GPL-3.0). The activity-coefficient math lives in DWSIM's
//! auxiliary model classes, not the property-package wrappers:
//!
//! - **NRTL** — `DWSIM.Thermodynamics/PropertyPackages/Models/NRTL.vb`,
//!   `NRTL.GAMMA_MR` (NRTL.vb:222-417). The package wrapper
//!   `PropertyPackages/NRTL.vb` only wires this up and estimates missing
//!   parameters via UNIFAC (not ported — see *Excluded behaviour*).
//! - **UNIQUAC** — `DWSIM.Thermodynamics/PropertyPackages/Models/UNIQUAC.vb`,
//!   `UNIQUAC.GAMMA_MR` (UNIQUAC.vb:245-448). Wrapper `PropertyPackages/UNIQUAC.vb`.
//! - **Ideal** — Raoult's law, `gamma_i = 1`; DWSIM's `RaoultPropertyPackage`
//!   returns unit activity coefficients for the liquid phase. Trivial, but a
//!   member of the model enum so the caller has one uniform dispatch surface.
//!
//! # What this computes
//!
//! Each model returns the vector of **liquid-phase activity coefficients**
//! `gamma_i` (dimensionless, `> 0`) at a given liquid mole-fraction vector `x`
//! (dimensionless, should sum to 1) and temperature `T` (kelvin). `gamma_i`
//! corrects Raoult's law for non-ideal mixing: `f_i = x_i * gamma_i * f_i^pure`.
//!
//! # Gas constant and parameter units (DWSIM convention — read carefully)
//!
//! DWSIM's NRTL/UNIQUAC kernels divide the interaction *energies* by
//! `R = 1.98721 cal/(mol.K)` (see [`R_CAL`]) — i.e. the interaction parameters
//! `a_ij` carried through this module are in **cal/mol**, exactly as stored in
//! DWSIM's `nrtl.dat` / `uniquac.dat` and its `NRTL_IPData` / `UNIQUAC_IPData`
//! records (NRTL.vb:321-322, UNIQUAC.vb:394-395). This module reproduces that
//! literally so results match DWSIM. The SI gas constant
//! `R = 8.31446261815324 J/(mol.K)` ([`R_GAS`], = `R_CAL * 4.184`) is provided
//! for callers that work in J/mol and is **not** used inside these two kernels.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch ([`ActivityModel`]), no `dyn`, no lifetimes, no channels.
//! Binary interaction parameters are **owned by value** as dense `Vec<Vec<f64>>`
//! matrices ([`NrtlParams`], [`UniquacParams`]); components are indexed by
//! `usize`. Raw `f64` (SI/DWSIM base units) in the inner arithmetic loops per
//! the crate unit policy; every public item documents its quantity/range/units.
//!
//! # Relationship to [`crate::thermo::Component`]
//!
//! [`Component`](crate::thermo::Component) carries critical constants and Cp0
//! coefficients but **not** the UNIQUAC van-der-Waals volume/surface parameters
//! `r_i` / `q_i`. Those are therefore taken as explicit inputs to
//! [`UniquacParams`]; a caller pairs each `Component` with its `(r_i, q_i)` from
//! a group-contribution table (e.g. UNIFAC `R_k`/`Q_k` sums). NRTL needs no
//! pure-component structural parameters at all.
//!
//! # Excluded DWSIM behaviour (honest scope)
//!
//! This is a **verification-grade port of the `GAMMA_MR` activity-coefficient
//! routine only**, not the whole property package. Deliberately *not* ported:
//!
//! - **Automatic parameter estimation.** DWSIM's `EstimateMissingInteraction`
//!   `Parameters` (NRTL.vb:133-320, UNIQUAC.vb:156-339) regresses missing binary
//!   parameters against Modified-UNIFAC using an IPOPT optimiser. Here **all**
//!   parameters are caller-supplied; there is no fallback estimation.
//! - **The bundled databases** (`nrtl.dat`, `uniquac.dat`, ChemSep IP data). The
//!   whole ID-lookup / symmetric-fill machinery (NRTL.vb:319-359,
//!   UNIQUAC.vb:352-385) is replaced by dense caller-supplied matrices.
//! - **Excess enthalpy / heat capacity** (`HEX_MIX`, `CPEX_MIX`, `DLNGAMMA_DT`;
//!   NRTL.vb:419-465, UNIQUAC.vb:450-496) — the `d ln gamma / dT` finite
//!   differences and their `H^E`/`Cp^E` derivatives are **not** ported.
//! - **Temperature-dependent NRTL non-randomness.** DWSIM's `NRTL_IPData` stores
//!   a single constant `alpha12`; there is no `T`-dependent `alpha` in the source
//!   to port, and none is added here (`alpha_ij` is a plain constant matrix).
//!
//! The `B`/`C` temperature coefficients of the interaction energy
//! (`tau ~ (A + B*T + C*T^2)/(R*T)`) **are** ported, defaulting to zero.
//!
//! > **⚠️ Verification, not validation.** The tests below verify that this code
//! > reproduces the DWSIM closed-form expressions and their known analytical
//! > limits (pure component, ideal mixture, infinite dilution, Gibbs-Duhem).
//! > They do **not** validate any parameter set against experimental VLE data,
//! > and nothing here is cleared for nuclear / safety-critical use.

use crate::thermo::Component;

/// DWSIM's gas constant for the NRTL/UNIQUAC `tau` reduction:
/// `R = 1.98721 cal/(mol.K)` (NRTL.vb:321, UNIQUAC.vb:394). Because DWSIM
/// divides the interaction energies by this value, the `a`/`b`/`c` parameters
/// carried by [`NrtlParams`] and [`UniquacParams`] are in **cal/mol**,
/// **cal/(mol.K)**, and **cal/(mol.K^2)** respectively.
pub const R_CAL: f64 = 1.98721;

/// SI molar gas constant `R = 8.31446261815324 J/(mol.K)` (CODATA). Provided for
/// callers working in J/mol; note `R_GAS = R_CAL * 4.184`. **Not** used inside
/// the NRTL/UNIQUAC kernels, which follow DWSIM in using [`R_CAL`] (cal units).
pub const R_GAS: f64 = 8.314_462_618_153_24;

/// Liquid-phase activity-coefficient model (enum dispatch, no `dyn`).
///
/// Given a liquid composition and temperature, [`Self::activity_coefficients`]
/// returns the vector of `gamma_i`. Variants:
///
/// - [`ActivityModel::Ideal`] — Raoult's law, every `gamma_i = 1`.
/// - [`ActivityModel::Nrtl`] — Non-Random Two-Liquid model ([`NrtlParams`]).
/// - [`ActivityModel::Uniquac`] — UNIQUAC model ([`UniquacParams`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityModel {
    /// Ideal solution (Raoult's law): `gamma_i = 1` for every component,
    /// independent of composition and temperature.
    Ideal,
    /// NRTL model with its binary interaction parameters.
    Nrtl(NrtlParams),
    /// UNIQUAC model with its pure-component structural parameters and binary
    /// interaction energies.
    Uniquac(UniquacParams),
}

impl ActivityModel {
    /// Liquid-phase activity coefficients `gamma_i` (dimensionless, `> 0`) at
    /// composition `x` (mole fractions, dimensionless) and temperature `t`
    /// (kelvin, `> 0`).
    ///
    /// `x.len()` sets the component count `n`; for the non-ideal models it must
    /// match the parameter-matrix dimension (a mismatch panics — a programming
    /// error, not a runtime input error). Compositions need not be normalised
    /// for the model math to run, but the physical interpretation assumes
    /// `sum(x) = 1`.
    ///
    /// # Panics
    /// If a parameter matrix in [`NrtlParams`]/[`UniquacParams`] is not `n x n`
    /// (or `r`/`q` are not length `n`).
    pub fn activity_coefficients(&self, x: &[f64], t: f64) -> Vec<f64> {
        match self {
            ActivityModel::Ideal => vec![1.0; x.len()],
            ActivityModel::Nrtl(p) => p.activity_coefficients(x, t),
            ActivityModel::Uniquac(p) => p.activity_coefficients(x, t),
        }
    }
}

/// Binary interaction parameters for the **NRTL** model.
///
/// All four matrices are dense and `n x n` (`n` = component count). Following
/// DWSIM (`NRTL.GAMMA_MR`, NRTL.vb:321-323) the reduced interaction parameter is
///
/// ```text
/// tau_ij = (a_ij + b_ij * T + c_ij * T^2) / (R_CAL * T)
/// G_ij   = exp(-alpha_ij * tau_ij)
/// ```
///
/// with `T` in kelvin and [`R_CAL`] in cal/(mol.K). Sign / index convention
/// (matching DWSIM's `NRTL_IPData`): `a[i][j]` is the energy of the `i-j`
/// interaction felt by molecule `i` (`a_ij != a_ji` in general); the diagonal
/// `a[i][i]` should be `0` (so `tau_ii = 0`, `G_ii = 1`).
///
/// Units: `a` in **cal/mol**, `b` in **cal/(mol.K)**, `c` in **cal/(mol.K^2)**,
/// `alpha` dimensionless. `alpha` is physically symmetric (`alpha_ij = alpha_ji`,
/// diagonal irrelevant since `tau_ii = 0`); DWSIM stores a single `alpha12` per
/// pair. Typical `alpha` is `0.2`-`0.47`.
#[derive(Debug, Clone, PartialEq)]
pub struct NrtlParams {
    /// Constant part of the interaction energy `a_ij` [cal/mol], `n x n`.
    pub a: Vec<Vec<f64>>,
    /// Linear-in-`T` coefficient `b_ij` [cal/(mol.K)], `n x n`. Zero for the
    /// common temperature-independent-energy form.
    pub b: Vec<Vec<f64>>,
    /// Quadratic-in-`T` coefficient `c_ij` [cal/(mol.K^2)], `n x n`. Usually zero.
    pub c: Vec<Vec<f64>>,
    /// Non-randomness factor `alpha_ij` [-], `n x n`, symmetric.
    pub alpha: Vec<Vec<f64>>,
}

impl NrtlParams {
    /// Full NRTL parameter set with temperature-dependent energies.
    ///
    /// Every matrix must be square and the same size; `a`, `b`, `c` in
    /// cal/mol, cal/(mol.K), cal/(mol.K^2); `alpha` dimensionless. Panics if the
    /// matrices are not all `n x n` for a common `n`.
    pub fn new(
        a: Vec<Vec<f64>>,
        b: Vec<Vec<f64>>,
        c: Vec<Vec<f64>>,
        alpha: Vec<Vec<f64>>,
    ) -> Self {
        let n = a.len();
        assert_square(&a, n, "NRTL a");
        assert_square(&b, n, "NRTL b");
        assert_square(&c, n, "NRTL c");
        assert_square(&alpha, n, "NRTL alpha");
        Self { a, b, c, alpha }
    }

    /// Convenience constructor for the common temperature-independent-energy
    /// form `tau_ij = a_ij / (R_CAL * T)` (sets `b = c = 0`). `a` in cal/mol,
    /// `alpha` dimensionless; both `n x n`.
    pub fn from_a_alpha(a: Vec<Vec<f64>>, alpha: Vec<Vec<f64>>) -> Self {
        let n = a.len();
        let zeros = zero_matrix(n);
        Self::new(a, zeros.clone(), zeros, alpha)
    }

    /// Number of components `n`.
    pub fn n(&self) -> usize {
        self.a.len()
    }

    /// NRTL activity coefficients `gamma_i` at composition `x` and `T` [K].
    ///
    /// Direct port of `NRTL.GAMMA_MR` (NRTL.vb:222-417). Builds the `tau` and
    /// `G` matrices, then for each `i`:
    ///
    /// ```text
    /// S_i = sum_k x_k G_ki
    /// C_i = sum_k x_k G_ki tau_ki
    /// ln gamma_i = C_i / S_i
    ///            + sum_j (x_j G_ij / S_j) (tau_ij - C_j / S_j)
    /// ```
    ///
    /// (NRTL.vb:387-403). Panics if `x.len() != n`.
    // Explicit `usize` indexing (not iterators) keeps the cross-indexed matrix
    // math — `g[k][i]`, `tau[k][i]`, `g[i][j]`, `s[j]` — readable and faithful
    // to the ported DWSIM loops, so `needless_range_loop` does not apply.
    #[allow(clippy::needless_range_loop)]
    pub fn activity_coefficients(&self, x: &[f64], t: f64) -> Vec<f64> {
        let n = self.n();
        assert_eq!(x.len(), n, "NRTL: x length {} != n {}", x.len(), n);

        // tau[i][j] and the two G matrices exactly as DWSIM forms them
        // (NRTL.vb:321-323, 368-369). G is asymmetric; alpha is symmetric.
        let mut tau = zero_matrix(n);
        let mut g = zero_matrix(n);
        for i in 0..n {
            for j in 0..n {
                let a = self.a[i][j] + self.b[i][j] * t + self.c[i][j] * t * t;
                tau[i][j] = a / (R_CAL * t);
            }
        }
        for i in 0..n {
            for j in 0..n {
                g[i][j] = (-self.alpha[i][j] * tau[i][j]).exp();
            }
        }

        // S_i = sum_k x_k G_ki ; C_i = sum_k x_k G_ki tau_ki  (NRTL.vb:387-388)
        let mut s = vec![0.0_f64; n];
        let mut c = vec![0.0_f64; n];
        for i in 0..n {
            let mut si = 0.0;
            let mut ci = 0.0;
            for k in 0..n {
                si += x[k] * g[k][i];
                ci += x[k] * g[k][i] * tau[k][i];
            }
            s[i] = si;
            c[i] = ci;
        }

        // ln gamma_i (NRTL.vb:395-405)
        let mut gamma = vec![0.0_f64; n];
        for i in 0..n {
            let mut ln_g = c[i] / s[i];
            for j in 0..n {
                ln_g += x[j] * g[i][j] * (tau[i][j] - c[j] / s[j]) / s[j];
            }
            gamma[i] = ln_g.exp();
        }
        gamma
    }
}

/// Parameters for the **UNIQUAC** model: pure-component structural parameters
/// plus binary interaction energies.
///
/// `r` and `q` are the van-der-Waals relative molecular **volume** and
/// **surface-area** parameters (dimensionless, `> 0`), each length `n`. They are
/// *not* carried by [`Component`] and must be supplied here (e.g. from a
/// UNIFAC-group `R_k`/`Q_k` summation). The interaction energies follow DWSIM
/// (`UNIQUAC.GAMMA_MR`, UNIQUAC.vb:394-395):
///
/// ```text
/// tau_ij = exp( (-a_ij + b_ij * T + c_ij * T^2) / (R_CAL * T) )
/// ```
///
/// where `a_ij = u_ij - u_jj` is the interaction energy [cal/mol]
/// (`a_ij != a_ji`; diagonal `a_ii = 0` so `tau_ii = 1`). Note the **leading
/// minus sign on `a`** in DWSIM's form. Units: `a` cal/mol, `b` cal/(mol.K),
/// `c` cal/(mol.K^2). The coordination number is fixed at `z = 10` (UNIQUAC.vb:333).
#[derive(Debug, Clone, PartialEq)]
pub struct UniquacParams {
    /// Van-der-Waals volume parameters `r_i` [-], length `n`, each `> 0`.
    pub r: Vec<f64>,
    /// Van-der-Waals surface-area parameters `q_i` [-], length `n`, each `> 0`.
    pub q: Vec<f64>,
    /// Interaction energy `a_ij = u_ij - u_jj` [cal/mol], `n x n`.
    pub a: Vec<Vec<f64>>,
    /// Linear-in-`T` coefficient `b_ij` [cal/(mol.K)], `n x n`. Usually zero.
    pub b: Vec<Vec<f64>>,
    /// Quadratic-in-`T` coefficient `c_ij` [cal/(mol.K^2)], `n x n`. Usually zero.
    pub c: Vec<Vec<f64>>,
}

impl UniquacParams {
    /// Fixed coordination number `z = 10` (UNIQUAC.vb:333).
    pub const Z: f64 = 10.0;

    /// Full UNIQUAC parameter set with temperature-dependent energies.
    ///
    /// `r`, `q` length `n` (`> 0`); `a`, `b`, `c` each `n x n` in
    /// cal/mol, cal/(mol.K), cal/(mol.K^2). Panics on any dimension mismatch.
    pub fn new(
        r: Vec<f64>,
        q: Vec<f64>,
        a: Vec<Vec<f64>>,
        b: Vec<Vec<f64>>,
        c: Vec<Vec<f64>>,
    ) -> Self {
        let n = r.len();
        assert_eq!(q.len(), n, "UNIQUAC: q length {} != n {}", q.len(), n);
        assert_square(&a, n, "UNIQUAC a");
        assert_square(&b, n, "UNIQUAC b");
        assert_square(&c, n, "UNIQUAC c");
        Self { r, q, a, b, c }
    }

    /// Convenience constructor for the common temperature-independent-energy
    /// form `tau_ij = exp(-a_ij / (R_CAL * T))` (sets `b = c = 0`). `a` in
    /// cal/mol, `r`/`q` dimensionless.
    pub fn from_energies(r: Vec<f64>, q: Vec<f64>, a: Vec<Vec<f64>>) -> Self {
        let n = r.len();
        let zeros = zero_matrix(n);
        Self::new(r, q, a, zeros.clone(), zeros)
    }

    /// Number of components `n`.
    pub fn n(&self) -> usize {
        self.r.len()
    }

    /// UNIQUAC activity coefficients `gamma_i` at composition `x` and `T` [K].
    ///
    /// Direct port of `UNIQUAC.GAMMA_MR` (UNIQUAC.vb:245-448). `ln gamma_i` is
    /// the sum of a combinatorial and a residual part:
    ///
    /// ```text
    /// J_i = r_i / sum_j x_j r_j          (phi_i / x_i)
    /// L_i = q_i / sum_j x_j q_j          (theta_i / x_i)
    /// ln gamma_i^C = 1 - J_i + ln J_i
    ///              - (z/2) q_i (1 - J_i/L_i + ln(J_i/L_i))
    /// s_i = sum_l theta_l tau_li
    /// ln gamma_i^R = q_i (1 - ln s_i - sum_j theta_j tau_ij / s_j)
    /// ```
    ///
    /// (UNIQUAC.vb:402-433, with `phi_i = x_i r_i / sum`, `theta_i = x_i q_i / sum`).
    /// Panics if `x.len() != n`.
    // Explicit `usize` indexing (not iterators) keeps the cross-indexed matrix
    // math — `tau[l][i]`, `tau[i][j]`, `s[j]`, `theta[l]` — readable and faithful
    // to the ported DWSIM loops, so `needless_range_loop` does not apply.
    #[allow(clippy::needless_range_loop)]
    pub fn activity_coefficients(&self, x: &[f64], t: f64) -> Vec<f64> {
        let n = self.n();
        assert_eq!(x.len(), n, "UNIQUAC: x length {} != n {}", x.len(), n);
        let z = Self::Z;

        // tau[i][j] = exp((-a_ij + b_ij T + c_ij T^2)/(R_CAL T))  (UNIQUAC.vb:394)
        let mut tau = zero_matrix(n);
        for i in 0..n {
            for j in 0..n {
                let e = -self.a[i][j] + self.b[i][j] * t + self.c[i][j] * t * t;
                tau[i][j] = (e / (R_CAL * t)).exp();
            }
        }

        // r = sum x_j R_j ; q = sum x_j Q_j  (UNIQUAC.vb:402-403)
        let mut r_mix = 0.0;
        let mut q_mix = 0.0;
        for j in 0..n {
            r_mix += x[j] * self.r[j];
            q_mix += x[j] * self.q[j];
        }

        // theta_i = x_i Q_i / q  (UNIQUAC.vb:406)
        let mut theta = vec![0.0_f64; n];
        for i in 0..n {
            theta[i] = x[i] * self.q[i] / q_mix;
        }

        // s_i = sum_l theta_l tau_li  (UNIQUAC.vb:411)
        let mut s = vec![0.0_f64; n];
        for i in 0..n {
            let mut si = 0.0;
            for l in 0..n {
                si += theta[l] * tau[l][i];
            }
            s[i] = si;
        }

        let mut gamma = vec![0.0_f64; n];
        for i in 0..n {
            // Combinatorial part (UNIQUAC.vb:431).
            let j_i = self.r[i] / r_mix; // = phi_i / x_i
            let l_i = self.q[i] / q_mix; // = theta_i / x_i
            let jl = j_i / l_i;
            let ln_gc = 1.0 - j_i + j_i.ln() - (z / 2.0) * self.q[i] * (1.0 - jl + jl.ln());

            // Residual part (UNIQUAC.vb:432): q_i (1 - ln s_i - sum_j theta_j tau_ij / s_j)
            let mut sum1 = 0.0;
            for j in 0..n {
                sum1 += theta[j] * tau[i][j] / s[j];
            }
            let ln_gr = self.q[i] * (1.0 - s[i].ln() - sum1);

            gamma[i] = (ln_gc + ln_gr).exp();
        }
        gamma
    }
}

/// Build an `n x n` matrix of zeros.
fn zero_matrix(n: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; n]; n]
}

/// Assert `m` is `n x n`, panicking with `label` on mismatch.
fn assert_square(m: &[Vec<f64>], n: usize, label: &str) {
    assert_eq!(m.len(), n, "{label}: outer dimension {} != n {}", m.len(), n);
    for (r, row) in m.iter().enumerate() {
        assert_eq!(row.len(), n, "{label}: row {r} length {} != n {}", row.len(), n);
    }
}

/// Bind a compile-time reference to [`Component`] so the module's documented
/// relationship to the shared pure-component type is checked by the compiler
/// (UNIQUAC's `r_i`/`q_i` are *not* fields of `Component` and so are taken
/// explicitly by [`UniquacParams`]). Returns the component's name.
///
/// This is a documentation/traceability helper, not part of the model math.
pub fn component_label(component: &Component) -> &str {
    &component.name
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the DWSIM `GAMMA_MR` port
    //!
    //! **Methodology.** Each test checks the Rust port against an *analytical*
    //! property of the model equations (not experimental data): the ideal /
    //! pure-component / ideal-mixture limits where `gamma = 1` exactly, a
    //! hand-computed NRTL infinite-dilution coefficient, and the binary
    //! Gibbs-Duhem consistency relation. Reference expressions are DWSIM
    //! `Models/{NRTL,UNIQUAC}.vb::GAMMA_MR`. Tolerances are absolute on `gamma`
    //! (or on the residual of an identity), noted per test.
    //!
    //! **Scope (honesty).** This is *verification* ("did we implement the DWSIM
    //! formula correctly?"), **not** *validation* against measured VLE. No
    //! parameter set here is claimed to be experimentally accurate.
    //!
    //! Numbers below were measured on 2026-07-24 (this port).

    use super::*;
    use approx::assert_relative_eq;

    fn m2(a: f64, b: f64, c: f64, d: f64) -> Vec<Vec<f64>> {
        vec![vec![a, b], vec![c, d]]
    }

    // ----- Ideal -----------------------------------------------------------

    /// **Methodology.** Ideal model must return `gamma_i = 1` for every
    /// component at any composition/temperature (Raoult's law).
    /// **Result (2026-07-24).** All three components returned exactly `1.0`.
    #[test]
    fn ideal_all_unit() {
        let model = ActivityModel::Ideal;
        let g = model.activity_coefficients(&[0.2, 0.5, 0.3], 350.0);
        assert_eq!(g, vec![1.0, 1.0, 1.0]);
    }

    // ----- NRTL ------------------------------------------------------------

    /// **Methodology.** For a pure component (`x_1 = 1`) NRTL must give
    /// `gamma_1 = 1` (self-interaction `tau_11 = 0`, `G_11 = 1`).
    /// **Result (2026-07-24).** `gamma_1 = 1.0` to `< 1e-12`.
    #[test]
    fn nrtl_pure_component_unit() {
        let p = NrtlParams::from_a_alpha(m2(0.0, 533.18, 473.94, 0.0), m2(0.0, 0.3, 0.3, 0.0));
        let g = p.activity_coefficients(&[1.0, 0.0], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Zero interaction parameters (`a = alpha = 0`) reduce
    /// NRTL to an ideal mixture: `gamma_i = 1` for all `i` at any composition.
    /// **Result (2026-07-24).** Both `gamma` values `= 1.0` to `< 1e-12`.
    #[test]
    fn nrtl_zero_interaction_is_ideal() {
        let p = NrtlParams::from_a_alpha(m2(0.0, 0.0, 0.0, 0.0), m2(0.0, 0.0, 0.0, 0.0));
        let g = p.activity_coefficients(&[0.4, 0.6], 320.0);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(g[1], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Infinite-dilution NRTL activity coefficient of a binary,
    /// against the closed form `ln gamma_1^inf = tau_21 + tau_12 * exp(-alpha * tau_12)`
    /// (independent derivation from the general `sum` routine).
    ///
    /// Parameters: a representative ethanol(1)-water(2) NRTL set in the DECHEMA
    /// physical range — `A12 = 533.18`, `A21 = 473.94 cal/mol`, `alpha = 0.30`,
    /// `T = 298.15 K` (units/convention per DWSIM `NRTL_IPData`). These are used
    /// to **verify the formula**, not as validated parameters. Hand computation:
    /// `tau_12 = 533.18/(1.98721*298.15) = 0.8999`, `tau_21 = 0.7999`,
    /// `G_12 = exp(-0.3*0.8999) = 0.76340`, so
    /// `ln gamma_1^inf = 0.79992 + 0.89990*0.76340 = 1.48690`,
    /// `gamma_1^inf = 4.42338`.
    ///
    /// **Result (2026-07-24).** Port returned `gamma_1^inf = 4.42338`, matching
    /// both the hand value and the closed-form limit to `< 1e-9` relative.
    #[test]
    fn nrtl_infinite_dilution_binary() {
        let a12 = 533.18;
        let a21 = 473.94;
        let alpha = 0.30;
        let t = 298.15;
        let p = NrtlParams::from_a_alpha(m2(0.0, a12, a21, 0.0), m2(0.0, alpha, alpha, 0.0));
        // x1 -> 0 (infinite dilution of component 1 in component 2).
        let g = p.activity_coefficients(&[0.0, 1.0], t);

        // Closed-form limit, derived independently of the general routine.
        let tau12 = a12 / (R_CAL * t);
        let tau21 = a21 / (R_CAL * t);
        let g12 = (-alpha * tau12).exp();
        let ln_g1_inf = tau21 + tau12 * g12;
        assert_relative_eq!(g[0], ln_g1_inf.exp(), epsilon = 1e-9);

        // Hand-computed number (dated 2026-07-24).
        assert_relative_eq!(g[0], 4.423_378_352_180_1, epsilon = 1e-9);

        // Majority component behaves as pure at this limit: gamma_2 -> 1.
        assert_relative_eq!(g[1], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Binary Gibbs-Duhem at constant `T`, `P`:
    /// `x_1 (d ln gamma_1 / d x_1) + x_2 (d ln gamma_2 / d x_1) = 0`
    /// (with `x_2 = 1 - x_1`), checked by central finite differences at
    /// `x_1 = 0.4`. This is the qualitative thermodynamic-consistency check the
    /// activity-coefficient expression must satisfy.
    /// **Result (2026-07-24).** Residual `= O(1e-9)`, well under the `1e-4`
    /// finite-difference tolerance.
    #[test]
    fn nrtl_gibbs_duhem_binary() {
        let p = NrtlParams::from_a_alpha(m2(0.0, 533.18, 473.94, 0.0), m2(0.0, 0.3, 0.3, 0.0));
        let t = 298.15;
        let x1 = 0.4;
        let h = 1e-6;
        let ln_at = |x1: f64| {
            let g = p.activity_coefficients(&[x1, 1.0 - x1], t);
            (g[0].ln(), g[1].ln())
        };
        let (l1p, l2p) = ln_at(x1 + h);
        let (l1m, l2m) = ln_at(x1 - h);
        let dln1 = (l1p - l1m) / (2.0 * h);
        let dln2 = (l2p - l2m) / (2.0 * h);
        let residual = x1 * dln1 + (1.0 - x1) * dln2;
        assert!(residual.abs() < 1e-4, "Gibbs-Duhem residual {residual} too large");
    }

    // ----- UNIQUAC ---------------------------------------------------------

    /// **Methodology.** Pure component (`x_1 = 1`): UNIQUAC combinatorial and
    /// residual parts both vanish, so `gamma_1 = 1`. Uses realistic
    /// ethanol/water structural parameters (`r`/`q`) with nonzero interaction
    /// energies to show the pure limit holds regardless of parameters.
    /// **Result (2026-07-24).** `gamma_1 = 1.0` to `< 1e-12`.
    #[test]
    fn uniquac_pure_component_unit() {
        // Ethanol r=2.1055 q=1.972 ; water r=0.9200 q=1.400 (DECHEMA-range).
        let p = UniquacParams::from_energies(
            vec![2.1055, 0.9200],
            vec![1.972, 1.400],
            m2(0.0, -109.6, 1332.3, 0.0),
        );
        let g = p.activity_coefficients(&[1.0, 0.0], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Ideal-mixture limit: *identical species* (same `r`, same
    /// `q`) with zero interaction energies must give `gamma_i = 1` for all `i`.
    /// (With differing `r`/`q` the combinatorial term is nonzero even at zero
    /// interaction — physically correct — so identical species is required for
    /// the strict `gamma = 1` limit.)
    /// **Result (2026-07-24).** Both `gamma = 1.0` to `< 1e-12`.
    #[test]
    fn uniquac_identical_species_is_ideal() {
        let p = UniquacParams::from_energies(
            vec![1.5, 1.5],
            vec![1.4, 1.4],
            m2(0.0, 0.0, 0.0, 0.0),
        );
        let g = p.activity_coefficients(&[0.3, 0.7], 310.0);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(g[1], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Zero interaction energies but *different* `r`/`q`: the
    /// residual part must be exactly zero (`tau_ij = 1` => `s_i = 1`,
    /// `sum theta_j / s_j = 1`), leaving a purely combinatorial (Flory-Huggins +
    /// Staverman-Guggenheim) `gamma != 1`. Verifies the residual/combinatorial
    /// split. Cross-checks **both** `gamma_i` against the closed combinatorial
    /// form (independent code path).
    /// **Result (2026-07-24).** `gamma_1 = 1.101830`, `gamma_2 = 1.223012`, each
    /// matching its hand combinatorial expression to `< 1e-12`, and both `!= 1`
    /// (pure size/shape effect).
    #[test]
    fn uniquac_zero_energy_is_combinatorial_only() {
        let r = vec![2.1055, 0.9200];
        let q = vec![1.972, 1.400];
        let x = [0.5, 0.5];
        let p = UniquacParams::from_energies(r.clone(), q.clone(), m2(0.0, 0.0, 0.0, 0.0));
        let g = p.activity_coefficients(&x, 298.15);

        // Independent combinatorial-only evaluation for each component.
        let z = UniquacParams::Z;
        let r_mix = x[0] * r[0] + x[1] * r[1];
        let q_mix = x[0] * q[0] + x[1] * q[1];
        for i in 0..2 {
            let ji = r[i] / r_mix;
            let li = q[i] / q_mix;
            let jl = ji / li;
            let ln_gc = 1.0 - ji + ji.ln() - (z / 2.0) * q[i] * (1.0 - jl + jl.ln());
            assert_relative_eq!(g[i], ln_gc.exp(), epsilon = 1e-12);
        }
        // Combinatorial term is non-trivial (r/q asymmetry) but residual is off.
        assert!((g[0] - 1.0).abs() > 1e-3 && (g[1] - 1.0).abs() > 1e-3);
    }

    /// **Methodology.** Binary Gibbs-Duhem at constant `T`, `P` for UNIQUAC,
    /// same central-difference identity as the NRTL case, at `x_1 = 0.35`, with
    /// nonzero interaction energies and asymmetric `r`/`q`.
    /// **Result (2026-07-24).** Residual `= O(1e-8)`, under the `1e-4` tolerance.
    #[test]
    fn uniquac_gibbs_duhem_binary() {
        let p = UniquacParams::from_energies(
            vec![2.1055, 0.9200],
            vec![1.972, 1.400],
            m2(0.0, -109.6, 1332.3, 0.0),
        );
        let t = 298.15;
        let x1 = 0.35;
        let h = 1e-6;
        let ln_at = |x1: f64| {
            let g = p.activity_coefficients(&[x1, 1.0 - x1], t);
            (g[0].ln(), g[1].ln())
        };
        let (l1p, l2p) = ln_at(x1 + h);
        let (l1m, l2m) = ln_at(x1 - h);
        let dln1 = (l1p - l1m) / (2.0 * h);
        let dln2 = (l2p - l2m) / (2.0 * h);
        let residual = x1 * dln1 + (1.0 - x1) * dln2;
        assert!(residual.abs() < 1e-4, "Gibbs-Duhem residual {residual} too large");
    }

    /// **Methodology.** The enum dispatch surface must delegate correctly:
    /// wrapping the same [`NrtlParams`] in [`ActivityModel::Nrtl`] gives the
    /// same result as calling the params directly.
    /// **Result (2026-07-24).** Identical to `< 1e-15`.
    #[test]
    fn enum_dispatch_matches_direct() {
        let p = NrtlParams::from_a_alpha(m2(0.0, 533.18, 473.94, 0.0), m2(0.0, 0.3, 0.3, 0.0));
        let x = [0.4, 0.6];
        let direct = p.activity_coefficients(&x, 298.15);
        let via_enum = ActivityModel::Nrtl(p).activity_coefficients(&x, 298.15);
        assert_relative_eq!(direct[0], via_enum[0], epsilon = 1e-15);
        assert_relative_eq!(direct[1], via_enum[1], epsilon = 1e-15);
    }
}
