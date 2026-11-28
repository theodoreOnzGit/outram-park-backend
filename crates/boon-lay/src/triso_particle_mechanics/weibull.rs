// SPDX-License-Identifier: AGPL-3.0-only
//! Weakest-link (Weibull) failure probability of a layer, integrated over
//! its volume.
//!
//! The BISON Theory Manual (2013) gives **no** failure model, so this is the
//! generic volume form of the weakest-link theory (Weibull, W. (1939), "A
//! statistical theory of the strength of materials", Ingeniörsvetenskaps-
//! akademiens Handlingar 151; not in the corpus, not read, cited for the
//! standard form only):
//!
//! ```text
//! P_f = 1 − exp[ −(1/V_c) ∫_V (σ⁺/σ_c)^m dV ]
//! ```
//!
//! with `σ_c` the characteristic strength of a specimen of volume `V_c` and
//! `m` the Weibull modulus, all inputs. `σ⁺ = max(σ, 0)`: compression does
//! not fail the layer. Two stress measures are offered and neither is the
//! default, because the choice is the caller's to justify.
//!
//! Note the scale: here `σ_c` is the **characteristic** strength
//! (`P_f = 1 − e⁻¹` at `σ = σ_c`, `V = V_c`). `crate::fuel_failure`'s
//! PANAMA-I Eq (1) uses the **median** strength (`ln 2` form); the two are
//! not interchangeable.
//!
//! The integral is evaluated with the mechanics' own Gauss points, so it
//! converges with the mesh (`tests::weibull_integral_converges_to_the_closed_form`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

use uom::si::f64::{Pressure, Volume};
use uom::si::pressure::pascal;
use uom::si::volume::cubic_meter;

use super::mechanics::ParticleSolver;
use super::ParticleMechanicsError;

/// Weibull parameters of one layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeibullParameters {
    /// Weibull modulus `m`, dimensionless, > 0.
    pub modulus: f64,
    /// Characteristic strength `σ_c`, Pa, > 0.
    pub characteristic_strength_pa: f64,
    /// Reference volume `V_c`, m³, > 0.
    pub reference_volume_m3: f64,
}

impl WeibullParameters {
    /// From `uom` quantities.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidMaterial`] unless all three are
    /// positive and finite.
    pub fn new(modulus: f64, characteristic_strength: Pressure, reference_volume: Volume) -> Result<Self, ParticleMechanicsError> {
        let s = characteristic_strength.get::<pascal>();
        let v = reference_volume.get::<cubic_meter>();
        for (name, x) in [("modulus", modulus), ("characteristic strength", s), ("reference volume", v)] {
            if !(x.is_finite() && x > 0.0) {
                return Err(ParticleMechanicsError::InvalidMaterial(format!("Weibull {name} must be positive, got {x}")));
            }
        }
        Ok(Self {
            modulus,
            characteristic_strength_pa: s,
            reference_volume_m3: v,
        })
    }
}

/// Which stress enters the integrand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeibullStressMeasure {
    /// The largest principal stress, `max(σ_r, σ_θ)`.
    MaximumPrincipal,
    /// Principle of independent action: `(σ_r⁺/σ_c)^m + 2(σ_θ⁺/σ_c)^m`, each
    /// principal stress contributing separately (the 2 counts `σ_θ = σ_φ`).
    IndependentAction,
}

/// `P_f` of layer `layer` in the solver's current state.
///
/// # Errors
///
/// [`ParticleMechanicsError::NoSuchLayer`] for an index past the last layer.
pub fn layer_failure_probability(
    solver: &ParticleSolver,
    layer: usize,
    params: &WeibullParameters,
    measure: WeibullStressMeasure,
) -> Result<f64, ParticleMechanicsError> {
    if layer >= solver.mesh().layers().len() {
        return Err(ParticleMechanicsError::NoSuchLayer(layer));
    }
    let m = params.modulus;
    let sc = params.characteristic_strength_pa;
    let mut integral = 0.0;
    for p in solver.points().iter().filter(|p| p.layer == layer) {
        let sr = p.stress_pa[0].max(0.0) / sc;
        let st = p.stress_pa[1].max(0.0) / sc;
        let g = match measure {
            WeibullStressMeasure::MaximumPrincipal => {
                let s = sr.max(st);
                if s > 0.0 { s.r_powf(m) } else { 0.0 }
            }
            WeibullStressMeasure::IndependentAction => {
                let a = if sr > 0.0 { sr.r_powf(m) } else { 0.0 };
                let b = if st > 0.0 { st.r_powf(m) } else { 0.0 };
                a + 2.0 * b
            }
        };
        integral += g * p.volume_m3;
    }
    Ok(1.0 - (-integral / params.reference_volume_m3).r_exp())
}
