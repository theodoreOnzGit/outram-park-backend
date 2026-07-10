//! Multi-fluid mixture properties — the CoolProp `HelmholtzEOSMixtureBackend`
//! (GERG-2008-style multi-fluid model).
//!
//! **Scaffold only (bead op-kbc.16).** Types and signatures are laid out; the
//! bodies are `todo!()`. Nothing here is wired into the crate's public API yet.
//!
//! # What this models
//!
//! A mixture of `N` pure components (each a [`crate::fluid::Fluid`]) at mole
//! fractions `x` is evaluated from a **single reduced Helmholtz surface**:
//!
//! `α(δ, τ, x) = α⁰(δ, τ, x) + Σ_i x_i · α_i^r(δ, τ) + Δα^r(δ, τ, x)`
//!
//! where
//! - the reduced state uses **composition-dependent reducing functions**
//!   `T_r(x)`, `ρ_r(x)` (Lorentz–Berthelot with binary `β`, `γ` parameters) —
//!   see [`reducing`],
//! - `Σ_i x_i α_i^r` reuses each pure component's own residual Helmholtz
//!   ([`crate::eos::FluidEos::residual_derivs`]),
//! - `Δα^r` is the **departure function**, a binary-pair excess term — see
//!   [`departure`] and the interaction table in [`binary_pairs`].
//!
//! All the same `(δ, τ)` derivatives as the pure EOS are needed (for `p`, `c_v`,
//! `c_p`, sound speed), plus composition derivatives for phase equilibrium.
//!
//! # Units
//!
//! Molar internally (the EOS's natural basis); the mass-based `(T, ρ)` state and
//! `uom` wrapper are follow-ups once the surface evaluates.
#![allow(dead_code, unused_variables, unused_imports)] // TODO(op-kbc.16): drop once implemented

pub mod binary_pairs;
pub mod departure;
pub mod reducing;

use crate::eos::HelmholtzDerivs;
use crate::fluid::Fluid;

/// A multi-fluid mixture: components plus their mole fractions (parallel
/// arrays, summing to 1).
///
/// Kept small and `Copy`-friendly via fixed capacity would constrain `N`; for
/// the scaffold we own `Vec`s (the mixture is built once, then queried).
#[derive(Debug, Clone, PartialEq)]
pub struct Mixture {
    /// Pure components.
    pub components: Vec<Fluid>,
    /// Mole fractions `x_i` \[-\], `Σ x_i = 1`, parallel to `components`.
    pub mole_fractions: Vec<f64>,
}

/// A fully-evaluated mixture state at `(T, ρ, x)` — molar SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixtureState {
    /// Temperature \[K\].
    pub temperature: f64,
    /// Molar density \[mol/m³\].
    pub rho_molar: f64,
    /// Pressure \[Pa\].
    pub pressure: f64,
    /// Molar internal energy \[J/mol\].
    pub internal_energy: f64,
    /// Molar enthalpy \[J/mol\].
    pub enthalpy: f64,
    /// Molar entropy \[J/(mol·K)\].
    pub entropy: f64,
    /// Molar isochoric heat capacity \[J/(mol·K)\].
    pub cv: f64,
    /// Molar isobaric heat capacity \[J/(mol·K)\].
    pub cp: f64,
    /// Speed of sound \[m/s\].
    pub speed_of_sound: f64,
}

/// Failure modes of a mixture query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixtureError {
    /// `components.len() != mole_fractions.len()`, or fewer than two components.
    MalformedComposition,
    /// A required binary interaction pair is not in [`binary_pairs`].
    MissingBinaryPair,
    /// A flash / VLE iteration did not converge.
    NonConvergent,
}

impl Mixture {
    /// Build a mixture, validating the composition (lengths match, ≥ 2
    /// components, fractions sum to ~1).
    pub fn new(components: Vec<Fluid>, mole_fractions: Vec<f64>) -> Result<Self, MixtureError> {
        if components.len() < 2 || components.len() != mole_fractions.len() {
            return Err(MixtureError::MalformedComposition);
        }
        // TODO(op-kbc.16): assert Σx ≈ 1 within tolerance; normalise if desired.
        Ok(Self { components, mole_fractions })
    }

    /// Composition-dependent reducing temperature `T_r(x)` \[K\].
    pub fn reducing_temperature(&self) -> f64 {
        reducing::reducing_temperature(self)
    }

    /// Composition-dependent reducing molar density `ρ_r(x)` \[mol/m³\].
    pub fn reducing_density(&self) -> f64 {
        reducing::reducing_density(self)
    }

    /// The mixture reduced-Helmholtz residual derivatives at reduced state
    /// `(δ, τ)` — `Σ_i x_i α_i^r + Δα^r`.
    pub fn residual_derivs(&self, delta: f64, tau: f64) -> HelmholtzDerivs {
        // TODO(op-kbc.16): sum pure-component residuals (weighted by x_i) and add
        // the departure contribution `departure::accumulate` for each binary pair.
        todo!("op-kbc.16: mixture residual Helmholtz derivatives")
    }

    /// Evaluate all properties at temperature `t` \[K\] and molar density
    /// `rho_molar` \[mol/m³\] — the mixture analogue of
    /// [`crate::props::state_trho`].
    pub fn state_trho_molar(&self, t: f64, rho_molar: f64) -> Result<MixtureState, MixtureError> {
        // TODO(op-kbc.16): δ = ρ/ρ_r(x), τ = T_r(x)/T; combine ideal + residual
        // derivs into p, u, h, s, c_v, c_p, w via the Span–Wagner relations.
        todo!("op-kbc.16: mixture (T, ρ) state")
    }
}
