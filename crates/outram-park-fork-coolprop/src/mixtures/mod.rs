//! Multi-fluid mixture properties — the CoolProp `HelmholtzEOSMixtureBackend`
//! (GERG-2008-style multi-fluid model).
//!
//! **840 of the 888 upstream binary pairs are ported**, including all 40
//! departure-bearing pairs and all 40 departure functions (the 48 skipped
//! pairs touch a fluid outside the crate's 137 ported pure fluids). The
//! reference-verified pair is **Nitrogen–Oxygen** (`F = 0`, see
//! [`binary_pairs`]). The evaluation engine ([`Mixture::residual_derivs`],
//! [`Mixture::state_trho_molar`]) is real, not a stub. **No flash/VLE** is
//! implemented — only direct `(T, ρ_molar, x)` evaluation — and the mixture
//! path has **not** been validated against GERG-2008 reference values yet
//! (bead op-kbc.16).
//!
//! # What this models
//!
//! A mixture of `N` pure components (each a [`crate::fluid::Fluid`]) at mole
//! fractions `x` is evaluated from a **single reduced Helmholtz surface**:
//!
//! `α_r(δ, τ, x) = Σ_i x_i · α_{r,i}(δ, τ) + Δα_r(δ, τ, x)`
//!
//! `α_0(ρ, T, x) = Σ_i x_i · [α_{0,i}(δ_i, τ_i) + ln(x_i)]`
//!
//! where
//! - the **residual** part shares the mixture's own composition-dependent
//!   reduced state `δ = ρ/ρ_r(x)`, `τ = T_r(x)/T` (Kunz–Wagner reducing
//!   functions with binary `β`, `γ` parameters — see [`reducing`]) across all
//!   components ([`Mixture::residual_derivs`]),
//! - the **ideal-gas** part instead evaluates *each* component `i` at its
//!   *own* pure-fluid reduced state `δ_i = ρ/ρ_{r,i}`, `τ_i = T_{r,i}/T`
//!   (α₀ depends only on the true density/temperature, parameterised through
//!   each component's own reducing values — not the mixture's shared `δ,τ`),
//!   plus the ideal entropy-of-mixing term `Σ x_i ln(x_i)`,
//! - `Δα_r` is the **departure function**, a binary-pair excess residual term
//!   — see [`departure`] and the interaction table in [`binary_pairs`].
//!
//! # Units
//!
//! Molar SI throughout (the EOS's natural basis): `T` \[K\], `ρ_molar`
//! \[mol/m³\], `p` \[Pa\], molar energies \[J/mol\], molar entropies/heat
//! capacities \[J/(mol·K)\].

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

    /// The mixture reduced-Helmholtz residual derivatives at the **mixture's
    /// own** reduced state `(δ, τ)` — `Σ_i x_i α_{r,i}(δ, τ) + Δα_r(δ, τ, x)`.
    ///
    /// A binary pair absent from [`binary_pairs`] contributes no departure term
    /// (`F = 0`, ideal mixing) rather than erroring — see the module doc.
    pub fn residual_derivs(&self, delta: f64, tau: f64) -> HelmholtzDerivs {
        let mut acc = HelmholtzDerivs::default();
        for (i, &fluid) in self.components.iter().enumerate() {
            let xi = self.mole_fractions[i];
            let di = fluid.eos().residual_derivs(delta, tau);
            add_weighted(&mut acc, &di, xi);
        }
        for i in 0..self.components.len() {
            for j in (i + 1)..self.components.len() {
                let Some(pair) = binary_pairs::lookup(self.components[i], self.components[j]) else {
                    continue;
                };
                if pair.f_departure == 0.0 || pair.departure_terms.is_empty() {
                    continue;
                }
                let xi = self.mole_fractions[i];
                let xj = self.mole_fractions[j];
                let mut dep = HelmholtzDerivs::default();
                for term in pair.departure_terms {
                    term.accumulate(delta, tau, &mut dep);
                }
                add_weighted(&mut acc, &dep, xi * xj * pair.f_departure);
            }
        }
        acc
    }

    /// Evaluate all properties at temperature `t` \[K\] and molar density
    /// `rho_molar` \[mol/m³\] — the mixture analogue of
    /// [`crate::props::state_trho`].
    ///
    /// Uses a single universal gas constant `R = 8.314472 J/(mol·K)` for the
    /// mixture (GERG-2008 convention), rather than a mole-fraction average of
    /// each component's own EOS-specific `R` (which for several fluids is a
    /// historical fitting artefact, not a physical constant that should be
    /// mixed).
    pub fn state_trho_molar(&self, t: f64, rho_molar: f64) -> Result<MixtureState, MixtureError> {
        if self.components.is_empty() || self.mole_fractions.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err(MixtureError::MalformedComposition);
        }
        const R: f64 = 8.314472;
        let t_r = self.reducing_temperature();
        let rho_r = self.reducing_density();
        let delta = rho_molar / rho_r;
        let tau = t_r / t;

        let res = self.residual_derivs(delta, tau);

        // Ideal-gas part: each component at its OWN reduced state (delta_i,
        // tau_i), not the mixture's shared (delta, tau) -- see the module doc.
        let mut ide = HelmholtzDerivs::default();
        let mut mixing_entropy_term = 0.0; // sum x_i ln(x_i), delta/tau-independent
        for (i, &fluid) in self.components.iter().enumerate() {
            let xi = self.mole_fractions[i];
            if xi <= 0.0 {
                continue;
            }
            let eos_i = fluid.eos();
            let delta_i = rho_molar / eos_i.rho_reducing;
            let tau_i = eos_i.t_reducing / t;
            let di = eos_i.ideal_derivs(delta_i, tau_i);
            add_weighted(&mut ide, &di, xi);
            mixing_entropy_term += xi * xi.ln();
        }

        let one_plus_dad = 1.0 + delta * res.ad;
        let p = rho_molar * R * t * one_plus_dad;

        let u = R * t * tau * (ide.at + res.at);
        let h = R * t * (1.0 + tau * (ide.at + res.at) + delta * res.ad);
        let s = R * (tau * (ide.at + res.at) - (ide.a + res.a) - mixing_entropy_term);

        let cv = -R * tau * tau * (ide.att + res.att);
        let num = (1.0 + delta * res.ad - delta * tau * res.adt).powi(2);
        let den = 1.0 + 2.0 * delta * res.ad + delta * delta * res.add;
        let cp = cv + R * num / den;

        let m_bar: f64 = self.components.iter().zip(&self.mole_fractions).map(|(f, x)| x * f.eos().molar_mass).sum();
        let w2 = (R * t / m_bar) * (den - num / (tau * tau * (ide.att + res.att)));
        let speed_of_sound = if w2 > 0.0 { w2.sqrt() } else { f64::NAN };

        Ok(MixtureState {
            temperature: t,
            rho_molar,
            pressure: p,
            internal_energy: u,
            enthalpy: h,
            entropy: s,
            cv,
            cp,
            speed_of_sound,
        })
    }
}

/// Add `w * derivs` into `acc`, field by field.
fn add_weighted(acc: &mut HelmholtzDerivs, derivs: &HelmholtzDerivs, w: f64) {
    acc.a += w * derivs.a;
    acc.ad += w * derivs.ad;
    acc.at += w * derivs.at;
    acc.add += w * derivs.add;
    acc.att += w * derivs.att;
    acc.adt += w * derivs.adt;
}
