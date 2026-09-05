//! Equilibrium sorption models for reactive transport (bead op-v6s.15.2).
//!
//! Sorption is the reversible partitioning of a dissolved species between the
//! aqueous phase and the solid (mineral / soil) surface. In a reactive-transport
//! model it is the dominant control on how much *slower* a reactive solute moves
//! than the water carrying it — captured by the **retardation factor** `R`. This
//! module provides the two standard equilibrium families:
//!
//! 1. **Isotherms** ([`SorptionIsotherm`]) — a single-solute algebraic relation
//!    between the sorbed concentration `s` (mol per kg of dry solid) and the
//!    aqueous concentration `c` (mol per litre of solution):
//!    - **Linear (Kd):**   `s = Kd · c`                          (`Kd` in L/kg)
//!    - **Langmuir:**       `s = s_max · K·c / (1 + K·c)`         (`s_max` mol/kg, `K` L/mol)
//!    - **Freundlich:**     `s = Kf · c^n`,  `n ∈ (0, 1]`         (`Kf` mol/kg per (mol/L)^n)
//!
//! 2. **Ion exchange** ([`IonExchange`]) — multi-cation competition for a fixed
//!    number of negatively-charged exchange sites, using the **Gaines–Thomas**
//!    convention (activities of the sorbed species are their *equivalent
//!    fractions*). Given the aqueous concentrations of `N ≥ 2` competing cations
//!    it returns their equivalent fractions `beta_i` on the exchanger.
//!
//! ## Retardation
//!
//! For a linearising sorption relation the one-dimensional advection–dispersion
//! equation for the total (aqueous + sorbed) mass carries an effective velocity
//! `v / R`, with
//!
//! ```text
//!     R = 1 + (rho_b / theta_w) · ds/dc
//! ```
//!
//! where `rho_b` is the dry **bulk density** of the porous medium (kg solid per
//! litre of bulk volume) and `theta_w` is the volumetric **water content**
//! (dimensionless, litre water per litre bulk). For the linear isotherm `ds/dc`
//! is the constant `Kd`, so `R` is constant; for Langmuir/Freundlich `ds/dc`
//! depends on `c`, so `R` is concentration-dependent (a nonlinear front).
//! See [`SorptionIsotherm::retardation`].
//!
//! ## Units summary
//!
//! | Symbol   | Meaning                         | Unit            |
//! |----------|---------------------------------|-----------------|
//! | `c`      | aqueous concentration           | mol/L           |
//! | `s`      | sorbed concentration            | mol/kg solid    |
//! | `Kd`     | linear distribution coefficient | L/kg            |
//! | `s_max`  | Langmuir sorption capacity      | mol/kg solid    |
//! | `K`      | Langmuir affinity               | L/mol           |
//! | `Kf`     | Freundlich coefficient          | mol/kg /(mol/L)^n |
//! | `n`      | Freundlich exponent             | dimensionless, (0,1] |
//! | `rho_b`  | dry bulk density                | kg/L (bulk)     |
//! | `theta_w`| volumetric water content        | dimensionless   |
//! | `CEC`    | cation-exchange capacity        | eq/kg solid     |
//! | `beta_i` | exchanger equivalent fraction   | dimensionless   |
//!
//! ## Design (workspace mandate)
//!
//! Enum dispatch, no trait objects, no `Box`, no lifetime parameters. The
//! ion-exchange equilibrium is closed with a single scalar site-activity term
//! and solved by a safeguarded (bisection-bracketed) **scalar Newton** iteration
//! — no dense linear algebra is needed.
//!
//! ## Scope and human-review flags
//!
//! - **Equilibrium only.** Local instantaneous equilibrium is assumed; there is
//!   no kinetic (rate-limited) sorption here.
//! - **Ideal activities.** Aqueous *concentrations* are used directly as
//!   activities (activity coefficients `gamma = 1`), consistent with the v1
//!   [`crate::geochemistry`] speciation core. Debye–Hückel / Davies corrections
//!   are deferred.
//! - **Surface complexation is deferred** — no diffuse-layer / constant-
//!   capacitance / triple-layer electrostatic surface-complexation model is
//!   implemented in this module.
//! - **Untrusted AI-generated draft** until a human reviews it, per the workspace
//!   `RESPONSIBLE_USE.md`. Verification-only: the tests below check closed-form
//!   limits and finite-difference derivatives, not published PFLOTRAN cases.
//!
//! ## Provenance
//!
//! Standard sorption / ion-exchange theory as used in PFLOTRAN's reactive-
//! transport (GIRT) module:
//! - I. Langmuir, "The adsorption of gases on plane surfaces of glass, mica and
//!   platinum," J. Am. Chem. Soc. 40 (1918) 1361.
//! - H. Freundlich, "Über die Adsorption in Lösungen," Z. Phys. Chem. 57 (1906) 385.
//! - G. L. Gaines & H. C. Thomas, "Adsorption Studies on Clay Minerals. II. A
//!   Formulation of the Thermodynamics of Exchange Adsorption," J. Chem. Phys.
//!   21 (1953) 714.
//! - C. A. J. Appelo & D. Postma, *Geochemistry, Groundwater and Pollution*,
//!   2nd ed., Balkema (2005), ch. 6 (ion exchange, Gaines–Thomas convention).
//! - G. E. Hammond, P. C. Lichtner & R. T. Mills, "Evaluating the performance of
//!   parallel subsurface simulators: An illustrative example with PFLOTRAN,"
//!   Water Resour. Res. 50 (2014) 208.

use crate::error::PflotranError;

/// Equilibrium sorption isotherm: sorbed `s` (mol/kg solid) versus aqueous `c`
/// (mol/L).
///
/// All three forms are single-solute, closed-form relations. Construct
/// [`SorptionIsotherm::Linear`] directly, or use the validating constructors
/// [`SorptionIsotherm::new_langmuir`] / [`SorptionIsotherm::new_freundlich`] for
/// the nonlinear forms, which reject non-physical parameters.
///
/// # Assumptions
/// - Local equilibrium; concentrations are used directly as activities
///   (`gamma = 1`).
/// - `c >= 0`. Negative inputs are clamped to `0` internally so no `NaN` escapes
///   from the fractional power in [`SorptionIsotherm::Freundlich`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SorptionIsotherm {
    /// Linear (constant-`Kd`) isotherm, `s = Kd · c`.
    ///
    /// - `kd`: distribution coefficient, L/kg (i.e. mol/kg sorbed per mol/L aqueous).
    ///   Physically `kd >= 0`.
    Linear {
        /// Linear distribution coefficient `Kd`, in L/kg.
        kd: f64,
    },

    /// Langmuir isotherm, `s = s_max · K·c / (1 + K·c)`.
    ///
    /// Saturates at the monolayer capacity `s_max` as `c -> inf`. Build with
    /// [`SorptionIsotherm::new_langmuir`] to enforce `s_max > 0`, `k > 0`.
    Langmuir {
        /// Maximum (monolayer) sorption capacity `s_max`, in mol/kg solid.
        s_max: f64,
        /// Langmuir affinity constant `K`, in L/mol.
        k: f64,
    },

    /// Freundlich isotherm, `s = Kf · c^n`, with `n ∈ (0, 1]`.
    ///
    /// Empirical power law for energetically heterogeneous surfaces. `n = 1`
    /// reduces exactly to the linear isotherm with `Kd = Kf`. Build with
    /// [`SorptionIsotherm::new_freundlich`] to enforce `kf > 0` and `n ∈ (0, 1]`.
    Freundlich {
        /// Freundlich coefficient `Kf`, in mol/kg per (mol/L)^n.
        kf: f64,
        /// Freundlich exponent `n`, dimensionless, in `(0, 1]`.
        n: f64,
    },
}

impl SorptionIsotherm {
    /// Sorbed concentration `s(c)` in mol/kg for aqueous `c` in mol/L (`c >= 0`).
    ///
    /// Negative `c` is treated as `0`.
    pub fn sorbed(&self, c: f64) -> f64 {
        let c = c.max(0.0);
        match *self {
            SorptionIsotherm::Linear { kd } => kd * c,
            SorptionIsotherm::Langmuir { s_max, k } => {
                let kc = k * c;
                s_max * kc / (1.0 + kc)
            }
            SorptionIsotherm::Freundlich { kf, n } => {
                if c == 0.0 {
                    // c^n -> 0 for n > 0; defined and finite.
                    0.0
                } else {
                    kf * c.powf(n)
                }
            }
        }
    }

    /// Analytic derivative `ds/dc` (units: (mol/kg)/(mol/L) = L/kg).
    ///
    /// Formulas:
    /// - Linear:      `ds/dc = Kd` (constant).
    /// - Langmuir:    `ds/dc = s_max·K / (1 + K·c)^2` (positive, decreasing in `c`).
    /// - Freundlich:  `ds/dc = Kf·n·c^(n-1)` (for `n < 1` this diverges as
    ///   `c -> 0+`, returning `+inf` at `c == 0`).
    ///
    /// Negative `c` is treated as `0`.
    pub fn d_sorbed_d_c(&self, c: f64) -> f64 {
        let c = c.max(0.0);
        match *self {
            SorptionIsotherm::Linear { kd } => kd,
            SorptionIsotherm::Langmuir { s_max, k } => {
                let denom = 1.0 + k * c;
                s_max * k / (denom * denom)
            }
            SorptionIsotherm::Freundlich { kf, n } => {
                if c == 0.0 {
                    if n < 1.0 {
                        f64::INFINITY
                    } else {
                        // n == 1 => linear slope Kf.
                        kf * n
                    }
                } else {
                    kf * n * c.powf(n - 1.0)
                }
            }
        }
    }

    /// Retardation factor `R = 1 + (rho_b / theta_w) · ds/dc`.
    ///
    /// # Parameters
    /// - `c`: aqueous concentration, mol/L (`c >= 0`).
    /// - `bulk_density` (`rho_b`): dry bulk density, kg solid per litre of bulk
    ///   volume (kg/L). Must be `> 0` for a meaningful result.
    /// - `water_content` (`theta_w`): volumetric water content, dimensionless
    ///   (L water / L bulk), `0 < theta_w <= 1`. Must be `> 0`.
    ///
    /// Returns `R >= 1`; `R = 1` means no sorption (solute travels at the water
    /// velocity). For the linear isotherm `R` is independent of `c`.
    pub fn retardation(&self, c: f64, bulk_density: f64, water_content: f64) -> f64 {
        1.0 + (bulk_density / water_content) * self.d_sorbed_d_c(c)
    }

    /// Construct a validated [`SorptionIsotherm::Langmuir`].
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `s_max <= 0`, `k <= 0`, or either is
    /// non-finite.
    pub fn new_langmuir(s_max: f64, k: f64) -> Result<Self, PflotranError> {
        if !s_max.is_finite() || s_max <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Langmuir s_max must be a finite positive value (mol/kg), got {s_max}"
            )));
        }
        if !k.is_finite() || k <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Langmuir affinity K must be a finite positive value (L/mol), got {k}"
            )));
        }
        Ok(SorptionIsotherm::Langmuir { s_max, k })
    }

    /// Construct a validated [`SorptionIsotherm::Freundlich`].
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `kf <= 0`, `n` is outside `(0, 1]`, or
    /// either is non-finite.
    pub fn new_freundlich(kf: f64, n: f64) -> Result<Self, PflotranError> {
        if !kf.is_finite() || kf <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Freundlich Kf must be a finite positive value, got {kf}"
            )));
        }
        if !n.is_finite() || n <= 0.0 || n > 1.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Freundlich exponent n must lie in (0, 1], got {n}"
            )));
        }
        Ok(SorptionIsotherm::Freundlich { kf, n })
    }
}

/// Gaines–Thomas cation exchange on an exchanger of capacity `cec` (eq/kg).
///
/// Models `N ≥ 2` cations competing for a fixed pool of negatively-charged
/// exchange sites. The state of the exchanger is described by the **equivalent
/// fractions** `beta_i` (fraction of the total charge-equivalents held by cation
/// `i`), which are dimensionless and sum to 1.
///
/// # Mass-action model
///
/// Each cation `i` has charge `z_i` and a Gaines–Thomas half-reaction relating
/// it to the pool of exchange sites. Closing the system with a single common
/// **site-activity term** `mu` (a positive scalar Lagrange-like multiplier that
/// enforces `sum_i beta_i = 1`) gives the equivalent fractions in the compact form
///
/// ```text
///     beta_i = K_i · a_i · mu^(z_i)          with  sum_i beta_i = 1,
/// ```
///
/// where `a_i` is the aqueous concentration (mol/L, used as activity) and
/// `K_i = 10^(log10_selectivity_i)` is cation `i`'s selectivity coefficient.
/// With this closure the *pairwise* Gaines–Thomas selectivity coefficient
/// between cations `i` and `j`,
///
/// ```text
///     K_ij = (beta_i^(z_j) · a_j^(z_i)) / (beta_j^(z_i) · a_i^(z_j)) = K_i^(z_j) / K_j^(z_i),
/// ```
///
/// is a constant independent of the aqueous composition, as Gaines–Thomas
/// requires. By convention one cation (typically the reference / index 0) is
/// assigned `log10_selectivity = 0` (`K = 1`); the others are relative to it.
///
/// # Solve method
///
/// Because every term `K_i·a_i·mu^(z_i)` is positive and strictly increasing in
/// `mu > 0`, the closure `g(mu) = sum_i K_i·a_i·mu^(z_i) - 1 = 0` has a unique
/// positive root. It is found by a **safeguarded scalar Newton** iteration: the
/// root is first bracketed (`g(0) = -1 < 0`; the upper bound is found by
/// doubling `mu` until `g > 0`), then Newton steps are taken with a bisection
/// fallback whenever a step would leave the bracket. This is a robust 1-D solve
/// — no dense linear algebra is required.
///
/// # Sorbed amounts
///
/// The sorbed concentration of cation `i` in mol/kg is recovered from its
/// equivalent fraction as `q_i = beta_i · cec / z_i`.
///
/// # Scope
/// Equilibrium only; ideal activities (`gamma = 1`); no surface complexation.
#[derive(Debug, Clone, PartialEq)]
pub struct IonExchange {
    /// Cation-exchange capacity `CEC`, in eq/kg solid (`> 0`).
    cec: f64,
    /// Cation charges `z_i` (each `> 0`), length `N >= 2`.
    charges: Vec<f64>,
    /// Selectivity coefficients `K_i = 10^(log10_selectivity_i)`, length `N`.
    selectivity: Vec<f64>,
}

impl IonExchange {
    /// Build a Gaines–Thomas exchanger.
    ///
    /// # Parameters
    /// - `cec`: cation-exchange capacity, eq/kg solid (`> 0`).
    /// - `charges`: cation charges `z_i`, each strictly positive; length `N >= 2`.
    /// - `log10_selectivity`: base-10 log of each cation's selectivity `K_i`,
    ///   same length `N` as `charges`. Use `0.0` for the reference cation.
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `cec <= 0`, fewer than two cations are
    /// given, the two slices differ in length, any charge is non-positive, or
    /// any value is non-finite.
    pub fn new(
        cec: f64,
        charges: Vec<f64>,
        log10_selectivity: Vec<f64>,
    ) -> Result<Self, PflotranError> {
        if !cec.is_finite() || cec <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "CEC must be a finite positive value (eq/kg), got {cec}"
            )));
        }
        if charges.len() < 2 {
            return Err(PflotranError::InvalidInput(format!(
                "ion exchange needs at least 2 competing cations, got {}",
                charges.len()
            )));
        }
        if charges.len() != log10_selectivity.len() {
            return Err(PflotranError::InvalidInput(format!(
                "charges (len {}) and log10_selectivity (len {}) must match",
                charges.len(),
                log10_selectivity.len()
            )));
        }
        for (i, &z) in charges.iter().enumerate() {
            if !z.is_finite() || z <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "cation {i} charge must be a finite positive value, got {z}"
                )));
            }
        }
        let mut selectivity = Vec::with_capacity(log10_selectivity.len());
        for (i, &ls) in log10_selectivity.iter().enumerate() {
            if !ls.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "cation {i} log10_selectivity must be finite, got {ls}"
                )));
            }
            selectivity.push(10.0_f64.powf(ls));
        }
        Ok(IonExchange {
            cec,
            charges,
            selectivity,
        })
    }

    /// Number of competing cations `N`.
    pub fn n_cations(&self) -> usize {
        self.charges.len()
    }

    /// Cation-exchange capacity `CEC`, in eq/kg solid.
    pub fn cec(&self) -> f64 {
        self.cec
    }

    /// Equilibrium exchanger equivalent fractions `beta_i` in equilibrium with
    /// the given aqueous concentrations (mol/L).
    ///
    /// Returns a length-`N` vector of dimensionless equivalent fractions that
    /// sum to 1, each in `[0, 1]`. Solved by the safeguarded scalar Newton
    /// described on [`IonExchange`].
    ///
    /// # Errors
    /// - [`PflotranError::InvalidInput`] if `aqueous.len() != N`, any
    ///   concentration is negative or non-finite, or every concentration is
    ///   zero (no cation present to occupy the exchanger).
    /// - [`PflotranError::Convergence`] if the scalar solve fails to bracket or
    ///   converge within its iteration budget (not expected for valid input).
    pub fn exchange_fractions(&self, aqueous: &[f64]) -> Result<Vec<f64>, PflotranError> {
        let n = self.charges.len();
        if aqueous.len() != n {
            return Err(PflotranError::InvalidInput(format!(
                "aqueous slice len {} does not match {n} cations",
                aqueous.len()
            )));
        }
        // Per-cation coefficient A_i = K_i * a_i (>= 0).
        let mut coeff = Vec::with_capacity(n);
        let mut any_positive = false;
        for (i, &a) in aqueous.iter().enumerate() {
            if !a.is_finite() || a < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "aqueous concentration {i} must be finite and >= 0, got {a}"
                )));
            }
            let ci = self.selectivity[i] * a;
            if ci > 0.0 {
                any_positive = true;
            }
            coeff.push(ci);
        }
        if !any_positive {
            return Err(PflotranError::InvalidInput(
                "all aqueous concentrations are zero — no cation to occupy the exchanger"
                    .to_string(),
            ));
        }

        let mu = self.solve_site_term(&coeff)?;

        // beta_i = A_i * mu^{z_i}; renormalise to kill any residual rounding.
        let mut beta: Vec<f64> = coeff
            .iter()
            .zip(self.charges.iter())
            .map(|(&a, &z)| a * mu.powf(z))
            .collect();
        let sum: f64 = beta.iter().sum();
        if !(sum.is_finite() && sum > 0.0) {
            return Err(PflotranError::Convergence(format!(
                "ion-exchange fractions did not form a valid distribution (sum = {sum})"
            )));
        }
        for b in beta.iter_mut() {
            *b /= sum;
        }
        Ok(beta)
    }

    /// Sorbed amounts `q_i = beta_i · CEC / z_i` (mol/kg solid) in equilibrium
    /// with the given aqueous concentrations.
    ///
    /// A convenience wrapper over [`IonExchange::exchange_fractions`] that
    /// converts equivalent fractions to molar sorbed concentrations.
    ///
    /// # Errors
    /// Propagates the errors of [`IonExchange::exchange_fractions`].
    pub fn sorbed_amounts(&self, aqueous: &[f64]) -> Result<Vec<f64>, PflotranError> {
        let beta = self.exchange_fractions(aqueous)?;
        Ok(beta
            .iter()
            .zip(self.charges.iter())
            .map(|(&b, &z)| b * self.cec / z)
            .collect())
    }

    /// Solve `g(mu) = sum_i coeff_i · mu^(z_i) - 1 = 0` for the unique `mu > 0`.
    ///
    /// `coeff_i = K_i · a_i >= 0` with at least one strictly positive. `g` is
    /// strictly increasing on `(0, inf)` with `g(0+) = -1`, so the root is
    /// bracketed then refined by safeguarded Newton (bisection fallback).
    fn solve_site_term(&self, coeff: &[f64]) -> Result<f64, PflotranError> {
        let z = &self.charges;
        let g = |mu: f64| -> f64 {
            coeff
                .iter()
                .zip(z.iter())
                .map(|(&a, &zi)| a * mu.powf(zi))
                .sum::<f64>()
                - 1.0
        };
        let dg = |mu: f64| -> f64 {
            coeff
                .iter()
                .zip(z.iter())
                .map(|(&a, &zi)| a * zi * mu.powf(zi - 1.0))
                .sum::<f64>()
        };

        // Bracket: lo has g < 0, hi has g > 0.
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        let mut g_hi = g(hi);
        let mut expand = 0;
        while g_hi < 0.0 {
            lo = hi;
            hi *= 2.0;
            g_hi = g(hi);
            expand += 1;
            if expand > 200 || !hi.is_finite() {
                return Err(PflotranError::Convergence(
                    "ion-exchange site term could not be bracketed".to_string(),
                ));
            }
        }
        // If g(1) already >= 0, shrink lo toward 0 to keep a valid bracket.
        if lo == 0.0 {
            let mut lo_try = hi;
            let mut g_lo = g(lo_try);
            let mut shrink = 0;
            while g_lo > 0.0 {
                lo_try *= 0.5;
                g_lo = g(lo_try);
                shrink += 1;
                if shrink > 200 || lo_try == 0.0 {
                    // g(0+) = -1 < 0 analytically; a tiny lo is a safe lower bound.
                    lo = 0.0;
                    break;
                }
                lo = lo_try;
            }
        }

        // Safeguarded Newton on [lo, hi].
        let mut mu = 0.5 * (lo + hi);
        let tol = 1.0e-13;
        for _ in 0..100 {
            let gm = g(mu);
            if gm.abs() <= tol {
                return Ok(mu);
            }
            if gm > 0.0 {
                hi = mu;
            } else {
                lo = mu;
            }
            let d = dg(mu);
            let mut next = if d.abs() > 0.0 { mu - gm / d } else { f64::NAN };
            // Fall back to bisection if the Newton step leaves the bracket.
            if !(next.is_finite() && next > lo && next < hi) {
                next = 0.5 * (lo + hi);
            }
            if (next - mu).abs() <= tol * mu.max(1.0) {
                mu = next;
                break;
            }
            mu = next;
        }
        if g(mu).abs() <= 1.0e-8 {
            Ok(mu)
        } else {
            Err(PflotranError::Convergence(format!(
                "ion-exchange site-term Newton did not converge (residual {})",
                g(mu)
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Central finite-difference of `s(c)` for derivative verification.
    fn fd_dsdc(iso: &SorptionIsotherm, c: f64, h: f64) -> f64 {
        (iso.sorbed(c + h) - iso.sorbed(c - h)) / (2.0 * h)
    }

    #[test]
    fn linear_matches_kd_and_constant_retardation() {
        let kd = 2.5;
        let iso = SorptionIsotherm::Linear { kd };
        for &c in &[0.0, 0.1, 1.0, 10.0] {
            assert!((iso.sorbed(c) - kd * c).abs() < 1e-14);
            assert!((iso.d_sorbed_d_c(c) - kd).abs() < 1e-14);
        }
        // R = 1 + rho_b/theta * Kd, independent of c.
        let rho_b = 1.8; // kg/L
        let theta = 0.3;
        let r_expected = 1.0 + rho_b / theta * kd;
        for &c in &[0.0, 1.0, 100.0] {
            assert!((iso.retardation(c, rho_b, theta) - r_expected).abs() < 1e-12);
        }
    }

    #[test]
    fn langmuir_saturates_and_slope_positive_decreasing() {
        let iso = SorptionIsotherm::new_langmuir(3.0, 2.0).unwrap();
        // s -> s_max as c -> inf.
        assert!((iso.sorbed(1e12) - 3.0).abs() < 1e-3);
        assert!(iso.sorbed(1e12) < 3.0);
        // ds/dc > 0 and strictly decreasing in c.
        let mut prev = f64::INFINITY;
        for &c in &[0.0, 0.5, 1.0, 2.0, 5.0] {
            let d = iso.d_sorbed_d_c(c);
            assert!(d > 0.0, "slope must be positive at c={c}");
            assert!(d < prev, "slope must decrease with c at c={c}");
            prev = d;
        }
        // FD-check analytic derivative.
        for &c in &[0.3, 1.0, 4.0] {
            let fd = fd_dsdc(&iso, c, 1e-6);
            let an = iso.d_sorbed_d_c(c);
            assert!((fd - an).abs() < 1e-6, "c={c} fd={fd} an={an}");
        }
    }

    #[test]
    fn freundlich_value_and_derivative() {
        let kf = 1.7;
        let n = 0.6;
        let iso = SorptionIsotherm::new_freundlich(kf, n).unwrap();
        // s(1) = Kf * 1^n = Kf.
        assert!((iso.sorbed(1.0) - kf).abs() < 1e-14);
        // FD-check ds/dc away from the c=0 singularity.
        for &c in &[0.2, 1.0, 3.0, 10.0] {
            let fd = fd_dsdc(&iso, c, 1e-6 * c);
            let an = iso.d_sorbed_d_c(c);
            assert!(
                (fd - an).abs() < 1e-5 * an.max(1.0),
                "c={c} fd={fd} an={an}"
            );
        }
        // n < 1 => slope diverges at c = 0.
        assert!(iso.d_sorbed_d_c(0.0).is_infinite());
    }

    #[test]
    fn freundlich_n1_reduces_to_linear() {
        let kf = 4.2;
        let iso = SorptionIsotherm::new_freundlich(kf, 1.0).unwrap();
        let lin = SorptionIsotherm::Linear { kd: kf };
        for &c in &[0.0, 0.5, 2.0, 25.0] {
            assert!((iso.sorbed(c) - lin.sorbed(c)).abs() < 1e-13);
            assert!((iso.d_sorbed_d_c(c) - lin.d_sorbed_d_c(c)).abs() < 1e-13);
        }
    }

    #[test]
    fn isotherm_constructor_validation() {
        assert!(SorptionIsotherm::new_langmuir(-1.0, 2.0).is_err());
        assert!(SorptionIsotherm::new_langmuir(1.0, 0.0).is_err());
        assert!(SorptionIsotherm::new_langmuir(1.0, -3.0).is_err());
        assert!(SorptionIsotherm::new_langmuir(f64::NAN, 1.0).is_err());
        assert!(SorptionIsotherm::new_langmuir(2.0, 1.0).is_ok());

        assert!(SorptionIsotherm::new_freundlich(-1.0, 0.5).is_err()); // kf <= 0
        assert!(SorptionIsotherm::new_freundlich(1.0, 0.0).is_err()); // n <= 0
        assert!(SorptionIsotherm::new_freundlich(1.0, 1.5).is_err()); // n > 1
        assert!(SorptionIsotherm::new_freundlich(1.0, 1.0).is_ok()); // n = 1 allowed
        assert!(SorptionIsotherm::new_freundlich(1.0, 0.5).is_ok());
    }

    #[test]
    fn exchange_fractions_sum_to_one_and_bounded() {
        // Two monovalent + one divalent cation.
        let ix = IonExchange::new(1.0, vec![1.0, 1.0, 2.0], vec![0.0, 0.3, 0.7]).unwrap();
        let beta = ix.exchange_fractions(&[0.01, 0.02, 0.005]).unwrap();
        assert_eq!(beta.len(), 3);
        let sum: f64 = beta.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "sum = {sum}");
        for &b in &beta {
            assert!((0.0..=1.0).contains(&b), "beta out of [0,1]: {b}");
        }
        // Sorbed amounts convert consistently: q_i = beta_i * CEC / z_i.
        let q = ix.sorbed_amounts(&[0.01, 0.02, 0.005]).unwrap();
        for i in 0..3 {
            let z = [1.0, 1.0, 2.0][i];
            assert!((q[i] - beta[i] * 1.0 / z).abs() < 1e-12);
        }
    }

    #[test]
    fn symmetric_binary_splits_by_concentration() {
        // Two identical monovalent cations (same charge, same selectivity):
        // beta_i proportional to a_i, so beta_i = a_i / (a0 + a1).
        let ix = IonExchange::new(2.0, vec![1.0, 1.0], vec![0.0, 0.0]).unwrap();

        // Equal concentrations => exactly 50/50.
        let beta = ix.exchange_fractions(&[0.01, 0.01]).unwrap();
        assert!((beta[0] - 0.5).abs() < 1e-10);
        assert!((beta[1] - 0.5).abs() < 1e-10);

        // Unequal concentrations => proportional split.
        let a = [0.03, 0.01];
        let beta = ix.exchange_fractions(&a).unwrap();
        let expect0 = a[0] / (a[0] + a[1]);
        assert!(
            (beta[0] - expect0).abs() < 1e-10,
            "beta0={} exp={}",
            beta[0],
            expect0
        );
        assert!((beta[1] - (1.0 - expect0)).abs() < 1e-10);
    }

    #[test]
    fn raising_a_cation_raises_its_exchanger_fraction() {
        // Monovalent (z=1) vs divalent (z=2) competition.
        let ix = IonExchange::new(1.0, vec![1.0, 2.0], vec![0.0, 0.0]).unwrap();
        let base = ix.exchange_fractions(&[0.01, 0.01]).unwrap();
        // Increase the monovalent aqueous concentration tenfold.
        let more_mono = ix.exchange_fractions(&[0.1, 0.01]).unwrap();
        assert!(
            more_mono[0] > base[0],
            "raising cation-0 aqueous conc should raise its exchanger fraction: {} -> {}",
            base[0],
            more_mono[0]
        );
        // And increasing the divalent conc should raise the divalent fraction.
        let more_di = ix.exchange_fractions(&[0.01, 0.1]).unwrap();
        assert!(
            more_di[1] > base[1],
            "raising cation-1 aqueous conc should raise its exchanger fraction: {} -> {}",
            base[1],
            more_di[1]
        );
    }

    #[test]
    fn divalent_selectivity_preference() {
        // Same aqueous concentration for a monovalent and a divalent cation;
        // give the divalent a higher selectivity => it should dominate the
        // exchanger (the classic preference of clays for higher-charge cations).
        let ix = IonExchange::new(1.0, vec![1.0, 2.0], vec![0.0, 1.0]).unwrap();
        let beta = ix.exchange_fractions(&[0.01, 0.01]).unwrap();
        assert!(
            beta[1] > beta[0],
            "divalent with higher selectivity should hold more of the exchanger: {beta:?}"
        );
    }

    #[test]
    fn ion_exchange_constructor_validation() {
        assert!(IonExchange::new(0.0, vec![1.0, 1.0], vec![0.0, 0.0]).is_err()); // cec <= 0
        assert!(IonExchange::new(1.0, vec![1.0], vec![0.0]).is_err()); // < 2 cations
        assert!(IonExchange::new(1.0, vec![1.0, 1.0], vec![0.0]).is_err()); // length mismatch
        assert!(IonExchange::new(1.0, vec![1.0, -2.0], vec![0.0, 0.0]).is_err()); // bad charge
        assert!(IonExchange::new(1.0, vec![1.0, 2.0], vec![0.0, f64::NAN]).is_err()); // bad sel
        assert!(IonExchange::new(1.0, vec![1.0, 2.0], vec![0.0, 0.5]).is_ok());

        let ix = IonExchange::new(1.0, vec![1.0, 2.0], vec![0.0, 0.0]).unwrap();
        assert!(ix.exchange_fractions(&[0.1]).is_err()); // wrong length
        assert!(ix.exchange_fractions(&[-0.1, 0.1]).is_err()); // negative conc
        assert!(ix.exchange_fractions(&[0.0, 0.0]).is_err()); // all zero
    }
}
