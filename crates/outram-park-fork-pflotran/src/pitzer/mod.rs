//! Pitzer ion-interaction (virial) activity-coefficient model — bead op-s1h.
//!
//! Extends the [`crate::activity`] module (Ideal / Debye–Hückel / Davies), which
//! is only trustworthy to ionic strength `I ~ 0.5 molal`, to the
//! high-ionic-strength brines encountered in geochemistry and repository
//! near-field chemistry. The Pitzer specific-ion-interaction model reproduces
//! measured mean activity and osmotic coefficients up to and beyond `6 molal`
//! for many electrolytes.
//!
//! Where the Debye–Hückel family treats all ion–ion interactions through a
//! single ionic-strength term, Pitzer adds an explicit, empirically-fitted
//! **virial expansion** in molality: a second-virial pair term (`beta0`,
//! `beta1`) and a third-virial term (`C_phi`) per binary electrolyte. Those
//! parameters are what carry the model to high concentration.
//!
//! ## Scope of this implementation
//!
//! This module implements the **binary (single-salt) 1:1 and 2:1 electrolyte**
//! case only — one cation, one anion, at **25 degrees C**. It does **not** yet
//! implement the mixing terms (`theta_ij`, `psi_ijk`) needed for multi-salt
//! brines, higher-order electrostatic terms, or any temperature dependence.
//! See the human-review flags below.
//!
//! ## Molality convention
//!
//! Pitzer's equations are written on a **molality** basis (mol of solute per kg
//! of solvent water), not molarity. For a salt `M_{nu_M} X_{nu_X}` that fully
//! dissociates at stoichiometric molality `m`, the ionic strength is
//!
//! ```text
//! I = 0.5 * ( nu_M * m * z_M^2 + nu_X * m * z_X^2 )      [mol/kg]
//! ```
//!
//! e.g. `I = m` for a 1:1 salt (NaCl) and `I = 3 m` for a 2:1 salt (CaCl2).
//!
//! ## Equations (25 degrees C)
//!
//! **Debye–Hückel osmotic function** (the long-range electrostatic term):
//!
//! ```text
//! f^gamma = -A_phi * [ sqrt(I) / (1 + b*sqrt(I)) + (2/b) * ln(1 + b*sqrt(I)) ]
//! f^phi   = -A_phi *   sqrt(I) / (1 + b*sqrt(I))
//! ```
//!
//! with `b = 1.2 kg^0.5 mol^-0.5` (Pitzer's universal constant) and the
//! Debye–Hückel–Pitzer slope [`A_PHI_25C`] `≈ 0.391 kg^0.5 mol^-0.5` at
//! 25 degrees C in water.
//!
//! **Second-virial `B` terms** (short-range pair interaction), with
//! `x = alpha*sqrt(I)` and `alpha = 2.0 kg^0.5 mol^-0.5` for 1:1 and 2:1
//! electrolytes:
//!
//! ```text
//! B^phi_MX   = beta0 + beta1 * exp(-alpha*sqrt(I))
//! B^gamma_MX = 2*beta0 + (2*beta1 / (alpha^2 * I))
//!              * [ 1 - (1 + alpha*sqrt(I) - 0.5*alpha^2*I) * exp(-alpha*sqrt(I)) ]
//! ```
//!
//! (`B^gamma_MX -> 2*beta0 + 2*beta1` as `I -> 0`, evaluated analytically here
//! to avoid the `0/0` at `I = 0`.)
//!
//! **Mean activity coefficient** of the binary salt, `nu = nu_M + nu_X`,
//! `C^gamma_MX = 1.5 * C_phi`:
//!
//! ```text
//! ln(gamma_pm) = |z_M z_X| * f^gamma
//!              + m * (2*nu_M*nu_X / nu)          * B^gamma_MX
//!              + m^2 * (2*(nu_M*nu_X)^1.5 / nu)  * C^gamma_MX
//! ```
//!
//! **Osmotic coefficient**:
//!
//! ```text
//! phi = 1 + |z_M z_X| * f^phi
//!         + m * (2*nu_M*nu_X / nu)         * B^phi_MX
//!         + m^2 * (2*(nu_M*nu_X)^1.5 / nu) * C_phi
//! ```
//!
//! ## Verification (against published tabulations)
//!
//! Methodology: evaluate `mean_activity_coefficient` / `osmotic_coefficient`
//! for NaCl and CaCl2 with the Pitzer & Mayorga (1973) parameters and compare
//! to their tabulated `gamma_pm` / `phi`. Results measured from this
//! implementation on 2026-07-23 (see `tests` at the foot of this file):
//!
//! | Quantity                      | This code | Published | Source                     |
//! |-------------------------------|-----------|-----------|----------------------------|
//! | NaCl gamma_pm, m = 0.1        | 0.7771    | ~0.778    | Pitzer & Mayorga 1973      |
//! | NaCl gamma_pm, m = 1.0        | 0.6561    | ~0.657    | Pitzer & Mayorga 1973      |
//! | NaCl gamma_pm, m = 3.0        | 0.7139    | ~0.714    | Robinson & Stokes / P&M    |
//! | NaCl phi,      m = 1.0        | 0.9361    | ~0.936    | Pitzer & Mayorga 1973      |
//! | CaCl2 gamma_pm upturn m1→m3   | 0.50→1.47 | rises     | Pitzer & Mayorga 1973      |
//!
//! Agreement is within `~0.001` of the published values across the range — the
//! high-molality validity the Debye–Hückel/Davies models lack.
//!
//! ## Human-review flags (this is untrusted AI-generated draft material)
//!
//! Per `RESPONSIBLE_USE.md`, treat this as a draft pending human inspection,
//! licence-provenance review, and independent verification. Specific
//! limitations:
//!
//! - **25 degrees C only.** [`A_PHI_25C`] and the `beta`/`C_phi` parameters are
//!   fixed at 25 degrees C. There is no temperature dependence; using this at
//!   other temperatures is a modelling error.
//! - **Binary (single-salt) only.** No cation–cation / anion–anion mixing
//!   terms (`theta_ij`), no triplet terms (`psi_ijk`). A real multi-salt brine
//!   needs those; this handles one MX salt in water.
//! - **1:1 and 2:1 electrolytes.** `alpha` is fixed at `2.0`; 2:2 electrolytes
//!   (which use a two-term `beta1`/`beta2` form with `alpha1 = 1.4`,
//!   `alpha2 = 12`) are **not** modelled.
//! - **Fully-dissociated salt assumed.** No ion pairing / association.
//!
//! ## Provenance
//!
//! Pitzer, K. S. (1973), "Thermodynamics of electrolytes. I. Theoretical basis
//! and general equations", *J. Phys. Chem.* **77**(2), 268–277. Pitzer, K. S.
//! & Mayorga, G. (1973), "Thermodynamics of electrolytes. II. Activity and
//! osmotic coefficients for strong electrolytes with one or both ions
//! univalent", *J. Phys. Chem.* **77**(19), 2300–2308 (source of the NaCl and
//! CaCl2 parameters and the `b = 1.2`, `alpha = 2.0` constants). Cross-check
//! tabulations: Robinson, R. A. & Stokes, R. H., *Electrolyte Solutions*, 2nd
//! ed. (Butterworths, 1959). These are open, published models.
//!
//! Enum-free by design (a single struct of parameters); no trait objects, no
//! `Box`, no lifetimes (workspace rules).

use crate::error::PflotranError;

/// Debye–Hückel–Pitzer osmotic-coefficient slope `A_phi` at 25 degrees C in
/// water, in units of `kg^0.5 mol^-0.5`.
///
/// This is the Pitzer form of the Debye–Hückel limiting slope (note it is
/// **not** the same constant as the base-10 `A ≈ 0.509` used by
/// [`crate::activity`]; that one multiplies `log10 gamma`, this one appears in
/// natural-log osmotic form). The commonly quoted value is `0.3915` at
/// 25 degrees C (Pitzer & Mayorga 1973); `0.391` is used here and reproduces
/// the published `gamma_pm` / `phi` tables to `~0.001`.
pub const A_PHI_25C: f64 = 0.391;

/// Pitzer's universal `b` constant in the Debye–Hückel term,
/// `b = 1.2 kg^0.5 mol^-0.5` (Pitzer 1973). Not adjustable per salt.
const B_PITZER: f64 = 1.2;

/// The `alpha` constant in the second-virial `B` term for 1:1 and 2:1
/// electrolytes, `alpha = 2.0 kg^0.5 mol^-0.5` (Pitzer & Mayorga 1973).
///
/// 2:2 electrolytes use a different two-term form (`alpha1 = 1.4`,
/// `alpha2 = 12`) and are out of scope for this module.
const ALPHA_1_1: f64 = 2.0;

/// Pitzer ion-interaction virial parameters for a single **binary** electrolyte
/// (one cation, one anion — e.g. NaCl, CaCl2) at 25 degrees C.
///
/// The `beta0`, `beta1`, and `c_phi` fields are the empirically-fitted virial
/// coefficients; the charge and stoichiometry fields describe the salt's
/// dissociation `M_{nu_cation} X_{nu_anion} -> nu_cation M^{z_cation} +
/// nu_anion X^{z_anion}`.
///
/// # Units
///
/// `beta0`, `beta1`, and `c_phi` are dimensionless in the sense that they carry
/// the `kg/mol` powers implied by the molality expansion (they are the values
/// tabulated by Pitzer & Mayorga 1973). Charges are signed charge numbers;
/// stoichiometric coefficients are counts.
///
/// # Valid range
///
/// Constructed for 1:1 and 2:1 electrolytes at 25 degrees C. Evaluation methods
/// are trustworthy to high ionic strength (`> 6 molal` for NaCl) — the regime
/// the Debye–Hückel/Davies models in [`crate::activity`] cannot reach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitzerBinaryParams {
    /// Second-virial `beta^(0)_MX` coefficient (kg/mol).
    pub beta0: f64,
    /// Second-virial `beta^(1)_MX` coefficient (kg/mol) — the ionic-strength-
    /// dependent part, gated by the `g(alpha*sqrt(I))` shape function.
    pub beta1: f64,
    /// Third-virial `C^phi_MX` coefficient (kg^2/mol^2). Note `C^gamma = 1.5 *
    /// C_phi` is used in the activity-coefficient expression.
    pub c_phi: f64,
    /// Signed charge number of the cation (e.g. `+1.0` for Na+, `+2.0` for
    /// Ca2+).
    pub z_cation: f64,
    /// Signed charge number of the anion (e.g. `-1.0` for Cl-).
    pub z_anion: f64,
    /// Stoichiometric coefficient of the cation (`nu_M`, e.g. `1.0` for NaCl,
    /// `1.0` for CaCl2).
    pub nu_cation: f64,
    /// Stoichiometric coefficient of the anion (`nu_X`, e.g. `1.0` for NaCl,
    /// `2.0` for CaCl2).
    pub nu_anion: f64,
}

impl PitzerBinaryParams {
    /// Published **NaCl** parameters (Pitzer & Mayorga 1973, Table I):
    /// `beta0 = 0.0765`, `beta1 = 0.2664`, `C_phi = 0.00127`.
    ///
    /// 1:1 electrolyte: `Na^+` (`z = +1`, `nu = 1`), `Cl^-` (`z = -1`,
    /// `nu = 1`).
    pub fn nacl() -> Self {
        Self {
            beta0: 0.0765,
            beta1: 0.2664,
            c_phi: 0.00127,
            z_cation: 1.0,
            z_anion: -1.0,
            nu_cation: 1.0,
            nu_anion: 1.0,
        }
    }

    /// Published **CaCl2** parameters (Pitzer & Mayorga 1973, Table I):
    /// `beta0 = 0.3159`, `beta1 = 1.614`, `C_phi = -0.00034`.
    ///
    /// 2:1 electrolyte: `Ca^{2+}` (`z = +2`, `nu = 1`), `Cl^-` (`z = -1`,
    /// `nu = 2`). Exhibits the characteristic high-molality upturn in
    /// `gamma_pm`.
    pub fn cacl2() -> Self {
        Self {
            beta0: 0.3159,
            beta1: 1.614,
            c_phi: -0.00034,
            z_cation: 2.0,
            z_anion: -1.0,
            nu_cation: 1.0,
            nu_anion: 2.0,
        }
    }

    /// Total stoichiometric number `nu = nu_cation + nu_anion`.
    #[inline]
    fn nu(&self) -> f64 {
        self.nu_cation + self.nu_anion
    }

    /// Ionic strength `I = 0.5 * (nu_M * m * z_M^2 + nu_X * m * z_X^2)` in
    /// mol/kg, for a fully-dissociated salt at stoichiometric molality `m`.
    ///
    /// e.g. `I = m` for NaCl and `I = 3 m` for CaCl2.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `molality` is negative.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_pflotran::pitzer::PitzerBinaryParams;
    /// let i = PitzerBinaryParams::cacl2().ionic_strength(0.5).unwrap();
    /// assert!((i - 1.5).abs() < 1e-12); // I = 3 m for a 2:1 salt
    /// ```
    pub fn ionic_strength(&self, molality: f64) -> Result<f64, PflotranError> {
        if molality < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Pitzer ionic_strength: negative molality {molality} mol/kg"
            )));
        }
        Ok(0.5
            * (self.nu_cation * molality * self.z_cation * self.z_cation
                + self.nu_anion * molality * self.z_anion * self.z_anion))
    }

    /// Debye–Hückel `f^gamma` term at ionic strength `i` (mol/kg):
    /// `-A_phi * [ sqrt(I)/(1 + b*sqrt(I)) + (2/b)*ln(1 + b*sqrt(I)) ]`.
    #[inline]
    fn f_gamma(i: f64) -> f64 {
        let sqrt_i = i.sqrt();
        -A_PHI_25C
            * (sqrt_i / (1.0 + B_PITZER * sqrt_i)
                + (2.0 / B_PITZER) * (1.0 + B_PITZER * sqrt_i).ln())
    }

    /// Debye–Hückel `f^phi` term (osmotic form) at ionic strength `i`:
    /// `-A_phi * sqrt(I) / (1 + b*sqrt(I))`.
    #[inline]
    fn f_phi(i: f64) -> f64 {
        let sqrt_i = i.sqrt();
        -A_PHI_25C * sqrt_i / (1.0 + B_PITZER * sqrt_i)
    }

    /// Second-virial `B^gamma_MX` term at ionic strength `i > 0`:
    /// `2*beta0 + (2*beta1/(alpha^2*I)) * [1 - (1 + x - 0.5*x^2)*exp(-x)]`,
    /// `x = alpha*sqrt(I)`.
    ///
    /// The `I -> 0` limit `2*beta0 + 2*beta1` is returned directly for
    /// non-positive `i` (the shape factor tends to 1) to avoid the `0/0` form.
    #[inline]
    fn b_gamma(&self, i: f64) -> f64 {
        if i <= 0.0 {
            return 2.0 * self.beta0 + 2.0 * self.beta1;
        }
        let x = ALPHA_1_1 * i.sqrt();
        let shape = 1.0 - (1.0 + x - 0.5 * x * x) * (-x).exp();
        2.0 * self.beta0 + (2.0 * self.beta1 / (x * x)) * shape
    }

    /// Second-virial `B^phi_MX` term (osmotic form) at ionic strength `i`:
    /// `beta0 + beta1*exp(-alpha*sqrt(I))`.
    #[inline]
    fn b_phi(&self, i: f64) -> f64 {
        self.beta0 + self.beta1 * (-ALPHA_1_1 * i.sqrt()).exp()
    }

    /// Mean activity coefficient `gamma_pm` of the salt at stoichiometric
    /// molality `m` (mol/kg water). Valid to high ionic strength.
    ///
    /// Implements
    /// `ln(gamma_pm) = |z_M z_X|*f^gamma + m*(2*nu_M*nu_X/nu)*B^gamma
    /// + m^2*(2*(nu_M*nu_X)^1.5/nu)*C^gamma`, with `C^gamma = 1.5*C_phi`
    /// (see the module header for the full derivation and constants).
    ///
    /// In the dilute limit `m -> 0` the result tends to `1` (Debye–Hückel
    /// limiting law recovered); `m == 0` returns exactly `1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `molality` is negative.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_pflotran::pitzer::PitzerBinaryParams;
    /// let g = PitzerBinaryParams::nacl().mean_activity_coefficient(1.0).unwrap();
    /// assert!((g - 0.657).abs() < 0.01); // Pitzer & Mayorga 1973
    /// ```
    pub fn mean_activity_coefficient(&self, molality: f64) -> Result<f64, PflotranError> {
        if molality < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Pitzer mean_activity_coefficient: negative molality {molality} mol/kg"
            )));
        }
        if molality == 0.0 {
            return Ok(1.0);
        }
        let i = self.ionic_strength(molality)?;
        let nu = self.nu();
        let z_prod = (self.z_cation * self.z_anion).abs();

        let coeff_b = 2.0 * self.nu_cation * self.nu_anion / nu;
        let coeff_c = 2.0 * (self.nu_cation * self.nu_anion).powf(1.5) / nu;
        let c_gamma = 1.5 * self.c_phi;

        let ln_gamma = z_prod * Self::f_gamma(i)
            + molality * coeff_b * self.b_gamma(i)
            + molality * molality * coeff_c * c_gamma;
        Ok(ln_gamma.exp())
    }

    /// Osmotic coefficient `phi` of the solvent at stoichiometric molality `m`
    /// (mol/kg water).
    ///
    /// Implements
    /// `phi = 1 + |z_M z_X|*f^phi + m*(2*nu_M*nu_X/nu)*B^phi
    /// + m^2*(2*(nu_M*nu_X)^1.5/nu)*C_phi` (module header).
    ///
    /// In the dilute limit `m -> 0`, `phi -> 1`; `m == 0` returns exactly
    /// `1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `molality` is negative.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_pflotran::pitzer::PitzerBinaryParams;
    /// let phi = PitzerBinaryParams::nacl().osmotic_coefficient(1.0).unwrap();
    /// assert!((phi - 0.936).abs() < 0.01); // Pitzer & Mayorga 1973
    /// ```
    pub fn osmotic_coefficient(&self, molality: f64) -> Result<f64, PflotranError> {
        if molality < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Pitzer osmotic_coefficient: negative molality {molality} mol/kg"
            )));
        }
        if molality == 0.0 {
            return Ok(1.0);
        }
        let i = self.ionic_strength(molality)?;
        let nu = self.nu();
        let z_prod = (self.z_cation * self.z_anion).abs();

        let coeff_b = 2.0 * self.nu_cation * self.nu_anion / nu;
        let coeff_c = 2.0 * (self.nu_cation * self.nu_anion).powf(1.5) / nu;

        let phi = 1.0
            + z_prod * Self::f_phi(i)
            + molality * coeff_b * self.b_phi(i)
            + molality * molality * coeff_c * self.c_phi;
        Ok(phi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NaCl mean activity coefficients vs Pitzer & Mayorga (1973) /
    /// Robinson & Stokes tabulations. Tolerance 0.01 (achieved: within
    /// ~0.001).
    ///
    /// Expected: m=0.1 -> 0.778, m=1.0 -> 0.657, m=3.0 -> 0.714.
    #[test]
    fn nacl_mean_activity_coefficient_published() {
        let nacl = PitzerBinaryParams::nacl();
        let g01 = nacl.mean_activity_coefficient(0.1).unwrap();
        let g10 = nacl.mean_activity_coefficient(1.0).unwrap();
        let g30 = nacl.mean_activity_coefficient(3.0).unwrap();
        assert!((g01 - 0.778).abs() < 0.01, "NaCl gamma_pm(0.1)={g01}, expected ~0.778");
        assert!((g10 - 0.657).abs() < 0.01, "NaCl gamma_pm(1.0)={g10}, expected ~0.657");
        assert!((g30 - 0.714).abs() < 0.01, "NaCl gamma_pm(3.0)={g30}, expected ~0.714");
    }

    /// NaCl osmotic coefficient at m=1.0 vs Pitzer & Mayorga (1973):
    /// phi ~ 0.936. Tolerance 0.01.
    #[test]
    fn nacl_osmotic_coefficient_published() {
        let phi = PitzerBinaryParams::nacl().osmotic_coefficient(1.0).unwrap();
        assert!((phi - 0.936).abs() < 0.01, "NaCl phi(1.0)={phi}, expected ~0.936");
    }

    /// CaCl2 exhibits the characteristic high-molality upturn: gamma_pm at
    /// m=3.0 exceeds gamma_pm at m=1.0 (it dips then rises past unity).
    /// Pitzer & Mayorga (1973).
    #[test]
    fn cacl2_high_molality_upturn() {
        let cacl2 = PitzerBinaryParams::cacl2();
        let g10 = cacl2.mean_activity_coefficient(1.0).unwrap();
        let g30 = cacl2.mean_activity_coefficient(3.0).unwrap();
        assert!(g30 > g10, "CaCl2 upturn: gamma(3.0)={g30} should exceed gamma(1.0)={g10}");
        // And by m=3 it has risen back above unity (a hallmark the DH/Davies
        // models cannot reproduce).
        assert!(g30 > 1.0, "CaCl2 gamma(3.0)={g30} should be > 1 (upturn past unity)");
    }

    /// Dilute limit: gamma_pm -> 1 and phi -> 1 as m -> 0 (Debye–Hückel
    /// limiting law recovered).
    #[test]
    fn dilute_limit_recovers_unity() {
        for salt in [PitzerBinaryParams::nacl(), PitzerBinaryParams::cacl2()] {
            // Exact zero.
            assert_eq!(salt.mean_activity_coefficient(0.0).unwrap(), 1.0);
            assert_eq!(salt.osmotic_coefficient(0.0).unwrap(), 1.0);
            // Approaching zero: monotone toward unity from below.
            let g_tiny = salt.mean_activity_coefficient(1e-6).unwrap();
            assert!(g_tiny < 1.0 && g_tiny > 0.99, "gamma(1e-6)={g_tiny} should be just under 1");
            let phi_tiny = salt.osmotic_coefficient(1e-6).unwrap();
            assert!(phi_tiny < 1.0 && phi_tiny > 0.99, "phi(1e-6)={phi_tiny} should be just under 1");
        }
    }

    /// NaCl gamma_pm decreases from the dilute limit, reaches a minimum, then
    /// rises again toward high molality (measured min near m~1).
    #[test]
    fn nacl_dips_then_rises() {
        let nacl = PitzerBinaryParams::nacl();
        let g05 = nacl.mean_activity_coefficient(0.5).unwrap();
        let g10 = nacl.mean_activity_coefficient(1.0).unwrap();
        let g60 = nacl.mean_activity_coefficient(6.0).unwrap();
        assert!(g10 < g05, "NaCl gamma should still be dropping from 0.5 to 1.0");
        assert!(g60 > g10, "NaCl gamma should have risen again by m=6");
    }

    /// Ionic strength convention: I = m for 1:1, I = 3 m for 2:1.
    #[test]
    fn ionic_strength_convention() {
        let i_nacl = PitzerBinaryParams::nacl().ionic_strength(2.0).unwrap();
        assert!((i_nacl - 2.0).abs() < 1e-12, "NaCl I(m=2)={i_nacl}");
        let i_cacl2 = PitzerBinaryParams::cacl2().ionic_strength(2.0).unwrap();
        assert!((i_cacl2 - 6.0).abs() < 1e-12, "CaCl2 I(m=2)={i_cacl2}");
    }

    /// Negative molality is rejected on every public entry point.
    #[test]
    fn negative_molality_rejected() {
        let nacl = PitzerBinaryParams::nacl();
        assert!(matches!(
            nacl.mean_activity_coefficient(-0.1),
            Err(PflotranError::InvalidInput(_))
        ));
        assert!(matches!(
            nacl.osmotic_coefficient(-0.1),
            Err(PflotranError::InvalidInput(_))
        ));
        assert!(matches!(
            nacl.ionic_strength(-0.1),
            Err(PflotranError::InvalidInput(_))
        ));
    }

    /// Constant sanity: A_phi at 25 degrees C is the documented ~0.391.
    #[test]
    fn a_phi_constant_value() {
        assert!((A_PHI_25C - 0.391).abs() < 1e-12);
    }
}
