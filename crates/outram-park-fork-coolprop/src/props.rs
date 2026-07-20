//! Thermodynamic properties from the Helmholtz EOS, given temperature and
//! density inputs `(T, ρ)`.
//!
//! `(T, ρ)` are the EOS's *natural* inputs, so these are exact single
//! evaluations (no iteration). The `(T, p)` / `(p, h)` / `(p, s)` flashes
//! CoolProp's `PropsSI` also offers require an iterative density (and/or
//! temperature) solve and live in [`crate::flash`].
//!
//! All public functions take/return **mass-based SI**: temperature in kelvin
//! \[K\], density in kg/m³, pressure in Pa, specific energies in J/kg, specific
//! heats in J/(kg·K), speed of sound in m/s. The molar EOS quantities are
//! converted at this boundary using the fluid's molar mass.
//!
//! Internally the EOS is raw `f64` SI; the `uom`-typed public wrapper matching
//! the workspace's units convention is [`crate::single_cv::OPCPFluidSingleCV`].

use crate::eos::FluidEos;
use crate::fluid::Fluid;

/// A fully-evaluated single-phase thermodynamic state. Mass-based SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidState {
    /// Temperature \[K\].
    pub temperature: f64,
    /// Mass density \[kg/m³\].
    pub density: f64,
    /// Pressure \[Pa\].
    pub pressure: f64,
    /// Specific internal energy \[J/kg\].
    pub internal_energy: f64,
    /// Specific enthalpy \[J/kg\].
    pub enthalpy: f64,
    /// Specific entropy \[J/(kg·K)\].
    pub entropy: f64,
    /// Isochoric specific heat `c_v` \[J/(kg·K)\].
    pub cv: f64,
    /// Isobaric specific heat `c_p` \[J/(kg·K)\].
    pub cp: f64,
    /// Speed of sound \[m/s\].
    pub speed_of_sound: f64,
}

/// Evaluate all properties of `fluid` at temperature `t` \[K\] and mass density
/// `rho` \[kg/m³\].
///
/// This is a direct, non-iterative EOS evaluation (the `(T, ρ)` inputs are the
/// EOS's natural variables). Returns single-phase properties for whatever
/// region `(T, ρ)` lands in; it does **not** perform a phase-stability check or
/// a two-phase flash (that is a follow-up).
pub fn state_trho(fluid: Fluid, t: f64, rho: f64) -> FluidState {
    let eos: &FluidEos = fluid.eos();
    let r = eos.gas_constant; // J/(mol·K)
    let m = eos.molar_mass; //   kg/mol
    let rho_molar = rho / m; //  mol/m³

    let delta = rho_molar / eos.rho_reducing;
    let tau = eos.t_reducing / t;

    let res = eos.residual_derivs(delta, tau);
    let ide = eos.ideal_derivs(delta, tau);

    // Standard Span–Wagner reduced-Helmholtz property relations (molar).
    let one_plus_dad = 1.0 + delta * res.ad;
    let p = rho_molar * r * t * one_plus_dad;

    let u_molar = r * t * tau * (ide.at + res.at);
    let h_molar = r * t * (1.0 + tau * (ide.at + res.at) + delta * res.ad);
    let s_molar = r * (tau * (ide.at + res.at) - (ide.a + res.a));

    let cv_molar = -r * tau * tau * (ide.att + res.att);
    let num = (1.0 + delta * res.ad - delta * tau * res.adt).powi(2);
    let den = 1.0 + 2.0 * delta * res.ad + delta * delta * res.add;
    let cp_molar = cv_molar + r * num / den;

    // w² = (R T / M) · [ 1 + 2δα_δ + δ²α_δδ − num/(τ²(α0_ττ+αr_ττ)) ]
    let w2 = (r * t / m) * (den - num / (tau * tau * (ide.att + res.att)));
    let speed_of_sound = if w2 > 0.0 { w2.sqrt() } else { f64::NAN };

    FluidState {
        temperature: t,
        density: rho,
        pressure: p,
        internal_energy: u_molar / m,
        enthalpy: h_molar / m,
        entropy: s_molar / m,
        cv: cv_molar / m,
        cp: cp_molar / m,
        speed_of_sound,
    }
}

/// Pressure \[Pa\] of `fluid` at temperature `t` \[K\] and mass density `rho`
/// \[kg/m³\]. Convenience wrapper over [`state_trho`] (only the residual
/// `∂α/∂δ` is needed, but it evaluates the full state for simplicity).
pub fn pressure_trho(fluid: Fluid, t: f64, rho: f64) -> f64 {
    state_trho(fluid, t, rho).pressure
}
