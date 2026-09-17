//! Saturation **ancillary** correlations — fast closed-form `p_sat(T)`,
//! `ρ'(T)` (saturated liquid) and `ρ''(T)` (saturated vapour) fits that CoolProp
//! ships alongside each fluid's EOS.
//!
//! These are *correlations*, not the EOS: they reproduce the saturation curve to
//! their stated accuracy (typically ≤0.1–1 %) in one evaluation, with no
//! iteration. They serve two roles here: standalone saturation lookups, and the
//! **initial guess** for the thermodynamically-consistent Maxwell VLE solve in
//! [`crate::vle`].
//!
//! # Form (CoolProp `SaturationAncillaryFunction::evaluate`)
//!
//! With `θ = 1 − T/T_r` and `S = Σ n_i · θ^{t_i}`:
//! - **exponential** (`pS`, `ρ''`): `value = v_r · exp(k · S)`, where `k = T_r/T`
//!   if `using_tau_r` else `1`.
//! - **non-exponential** (`ρ'` / `rhoLnoexp`): `value = v_r · (1 + S)`.
//!
//! Valid only for `T ≤ T_r`; above the reducing temperature the correlation
//! returns `NaN` (as in CoolProp), so callers must guard supercritical `T`.

use crate::fluid::Fluid;

/// One saturation ancillary correlation (see the module docs for the form).
#[derive(Debug, Clone, Copy)]
pub struct SatAncillary {
    /// Reducing value `v_r` (a pressure \[Pa\] for `p_sat`, a molar density
    /// \[mol/m³\] for the densities).
    pub reducing_value: f64,
    /// Reducing temperature `T_r` \[K\] (for `θ = 1 − T/T_r`).
    pub t_r: f64,
    /// Whether the exponential prefactor uses `k = T_r/T` (`true`) or `k = 1`.
    pub using_tau_r: bool,
    /// `true` → `v_r·exp(k·S)` (pressure / vapour density);
    /// `false` → `v_r·(1 + S)` (liquid density, `rhoLnoexp`).
    pub exponential: bool,
    /// Series coefficients `n_i`.
    pub n: &'static [f64],
    /// Series exponents `t_i`.
    pub t: &'static [f64],
}

impl SatAncillary {
    /// Evaluate the correlation at temperature `t` \[K\]. Returns `NaN` for
    /// `t > t_r` (supercritical — outside the correlation's domain).
    pub fn evaluate(&self, t: f64) -> f64 {
        let theta = 1.0 - t / self.t_r;
        if theta < 0.0 {
            return f64::NAN;
        }
        let mut summer = 0.0;
        for i in 0..self.n.len() {
            summer += self.n[i] * theta.powf(self.t[i]);
        }
        if self.exponential {
            let k = if self.using_tau_r { self.t_r / t } else { 1.0 };
            self.reducing_value * (k * summer).exp()
        } else {
            self.reducing_value * (1.0 + summer)
        }
    }
}

/// The three saturation ancillaries used for VLE: pressure and the two
/// coexisting densities. Present only for fluids that ship all three (131 of
/// CoolProp's 137; the rest lack a `pS` ancillary).
#[derive(Debug, Clone, Copy)]
pub struct FluidAncillaries {
    /// Saturation pressure `p_sat(T)` \[Pa\] (CoolProp `pS`, type `pL`/`pV`).
    pub p_sat: SatAncillary,
    /// Saturated-liquid molar density `ρ'(T)` \[mol/m³\] (CoolProp `rhoL`).
    pub rho_l: SatAncillary,
    /// Saturated-vapour molar density `ρ''(T)` \[mol/m³\] (CoolProp `rhoV`).
    pub rho_v: SatAncillary,
}

/// Ancillary **saturation pressure** \[Pa\] at temperature `t` \[K\], or `None`
/// if the fluid has no ancillaries / `t` is supercritical. This is the fast
/// correlation; for the EOS-consistent value use [`crate::vle`].
pub fn saturation_pressure_ancillary(fluid: Fluid, t: f64) -> Option<f64> {
    let a = fluid.ancillaries()?;
    let p = a.p_sat.evaluate(t);
    if p.is_finite() && p > 0.0 {
        Some(p)
    } else {
        None
    }
}

/// Ancillary **saturated-liquid** mass density \[kg/m³\] at temperature `t`
/// \[K\] (converts the molar ancillary with the fluid's molar mass).
pub fn saturated_liquid_density_ancillary(fluid: Fluid, t: f64) -> Option<f64> {
    let a = fluid.ancillaries()?;
    let rho_molar = a.rho_l.evaluate(t);
    (rho_molar.is_finite() && rho_molar > 0.0).then(|| rho_molar * fluid.eos().molar_mass)
}

/// Ancillary **saturated-vapour** mass density \[kg/m³\] at temperature `t`
/// \[K\].
pub fn saturated_vapour_density_ancillary(fluid: Fluid, t: f64) -> Option<f64> {
    let a = fluid.ancillaries()?;
    let rho_molar = a.rho_v.evaluate(t);
    (rho_molar.is_finite() && rho_molar > 0.0).then(|| rho_molar * fluid.eos().molar_mass)
}
