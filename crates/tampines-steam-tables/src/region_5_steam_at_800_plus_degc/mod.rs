//! IAPWS-IF97 Region 5 (ultra-high-temperature steam): valid for
//! 1073.15 K <= T <= 2273.15 K (roughly 800 °C and above) at pressures up to
//! 50 MPa. Results above ~2273 K are extrapolations beyond the IF97
//! specification, not validated IF97 output (callers default to returning
//! `OutOfRange` there). Properties are derived from the dimensionless
//! specific Gibbs free energy, split into an ideal-gas part
//! ([`gamma_ideal_gas_plus_derivatives`]) and a residual part
//! ([`gamma_residual_plus_derivatives`]) with their `pi`/`tau` derivatives.
//! IAPWS-IF97 publishes only forward `(p,T)` equations for Region 5 — there
//! are no official backward equations. This crate carries experimental,
//! non-IAPWS `T(p,h)` and `T(p,s)` correlations fitted against the forward
//! equations below, in
//! [`crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps`]; they are
//! an accelerator for what would otherwise be an iterative solve, not IAPWS
//! values.

/// IAPWS-IF97 Region 5 Table 39 coefficients (I_i, J_i, n_i) for the residual
/// part of the dimensionless Gibbs free energy, `gamma_5_res(pi, tau)`.
const REGION_5_COEFFS_RES: [[f64; 3]; 6] = [
    [1.0, 1.0, 0.15736404855259e-2],
    [1.0, 2.0, 0.90153761673944e-3],
    [1.0, 3.0, -0.50270077677648e-2],
    [2.0, 3.0, 0.22440037409485e-5],
    [2.0, 9.0, -0.41163275453471e-5],
    [3.0, 7.0, 0.37919454822955e-7],
];

/// IAPWS-IF97 Region 5 Table 38 coefficients (J_i, n_i) for the ideal-gas
/// part of the dimensionless Gibbs free energy, `gamma_5_ideal(pi, tau)`.
const REGION_5_COEFFS_IDEAL: [[f64; 2]; 6] = [
    [0.0, -0.13179983674201e2],
    [1.0, 0.68540841634434e1],
    [-3.0, -0.24805148933466e-1],
    [-2.0, 0.36901534980333],
    [-1.0, -0.31161318213925e1],
    [2.0, -0.32961626538917],
];

/// dimensionless reduced pressure `pi` and reduced temperature `tau` for
/// Region 5
pub mod dimensionless_tau_and_pi;
pub use dimensionless_tau_and_pi::*;

/// ideal-gas part of the dimensionless Gibbs free energy and its `pi`/`tau`
/// derivatives, for Region 5
pub mod gamma_ideal_gas_plus_derivatives;
pub use gamma_ideal_gas_plus_derivatives::*;

/// residual part of the dimensionless Gibbs free energy and its `pi`/`tau`
/// derivatives, for Region 5
pub mod gamma_residual_plus_derivatives;
pub use gamma_residual_plus_derivatives::*;

/// intensive properties (specific volume, enthalpy, entropy, cp, cv, speed
/// of sound, etc.) computed from the Region 5 gamma derivatives
pub mod intensive_properties;
pub use intensive_properties::*;

#[cfg(test)]
mod tests;
