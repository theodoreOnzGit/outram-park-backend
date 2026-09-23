// SPDX-License-Identifier: GPL-3.0

//! **Tally derivatives** — a sensitivity coefficient from a single run.
//! GitHub #263 scope item 3.
//!
//! Ported from `src/tallies/derivative.cpp` at OpenMC `afa7a14`.
//!
//! # What this replaces
//!
//! Every reactivity and sensitivity coefficient in this crate today is a
//! **paired-run difference**: run twice, subtract. That costs two converged
//! runs and its uncertainty is the quadrature sum of both, which is why the
//! ablations recorded in `CLAUDE.md` need 64 to 400 seeds per arm to resolve
//! effects of a few tens of pcm. A differential operator gives the same
//! quantity from one run with **correlated** statistics, so the difference's
//! variance is not the sum of two independent variances.
//!
//! # The method
//!
//! Along each history the code accumulates the **logarithmic flux
//! derivative** `(1/φ)(∂φ/∂p)`. At a collision in the perturbed material that
//! quantity gains a term that depends only on the perturbed variable:
//!
//! | variable | `(1/φ)(∂φ/∂p)` gains |
//! |---|---|
//! | density | `1/ρ` |
//! | nuclide density | `1/N_i` |
//! | temperature | `(∂σ_s/∂T) / σ_s` |
//!
//! A score `c` is then differentiated as
//! `∂c/∂p = c ((1/φ)(∂φ/∂p) + (1/c)(∂c/∂p))`, the second term being zero for a
//! flux score and `1/ρ` or `1/N_i` for a reaction rate.
//!
//! # Where this port differs from upstream, deliberately
//!
//! **The density variable is the total ATOM density, not the mass density.**
//! Upstream differentiates with respect to `material.density_gpcc_`; this
//! crate's [`Material`] carries atom densities only and no mass density at
//! all. At fixed composition the two are proportional, so the *logarithmic*
//! derivative `1/ρ` is numerically identical either way — but the units of the
//! answer differ, and [`DerivativeVariable::Density`] says so rather than
//! leaving a reader to assume g/cm³.
//!
//! **The temperature derivative is a finite difference**, not the analytic
//! windowed-multipole one. Upstream calls `multipole_->evaluate_deriv`; this
//! crate has no such routine, and inventing an analytic derivative it does not
//! have would be worse than differencing. It is therefore available **only on
//! the WMP (`Core`) tier**, where `xs_at_energy` actually depends on the
//! temperature argument; on the pointwise tier the cross sections are
//! broadened once at construction and the temperature argument is ignored, so
//! a finite difference there would return exactly zero — a plausible,
//! completely wrong answer. [`TallyDerivative::temperature`] refuses that case
//! instead of returning the zero.
//!
//! **Upstream's own approximation is inherited**, and it is worth restating:
//! the temperature derivative assumes `∂P(E'→E, u'→u)/∂T = 0`, i.e. that only
//! the magnitude of the scattering cross section moves with temperature and
//! not the transfer kernel. Upstream records this as causing 2–5 % errors on
//! PWR pin-cell eigenvalue derivatives near low-energy resonances.

use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::tally::tally::ScoreType;

/// Which material property a tally is differentiated with respect to —
/// `DerivativeVariable` (`include/openmc/tallies/derivative.h`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DerivativeVariable {
    /// The material's **total atom density** \[atoms/barn-cm\], scaling the
    /// whole composition. See the module docs: upstream uses the mass density,
    /// and the logarithmic derivative is the same number while the units of
    /// the answer are not.
    Density,
    /// One nuclide's atom density \[atoms/barn-cm\]; the index is into the
    /// global nuclide array.
    NuclideDensity { nuclide_idx: usize },
    /// Temperature \[K\]. WMP tier only — see the module docs.
    Temperature,
}

/// A differential operator attached to a tally — `TallyDerivative`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TallyDerivative {
    /// Index of the material being perturbed. A collision anywhere else
    /// contributes nothing.
    pub material_idx: usize,
    /// What is perturbed.
    pub variable: DerivativeVariable,
}

/// The running logarithmic flux derivative for one history —
/// `p.flux_derivs(idx)`.
///
/// One per (history, derivative). Reset at the start of every history: it is a
/// path integral along that history and carrying it across would sum
/// unrelated particles.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FluxDerivative {
    /// `(1/φ)(∂φ/∂p)` accumulated so far.
    pub value: f64,
}

impl TallyDerivative {
    /// The temperature step used by the finite difference \[K\].
    ///
    /// 1 K is small against the ~hundreds of K over which a resonance profile
    /// changes shape and large enough that the difference of two broadened
    /// cross sections is not dominated by floating-point cancellation. A
    /// central difference makes the leading error `O(ΔT²)`.
    pub const TEMPERATURE_STEP_K: f64 = 1.0;

    /// Accumulate this derivative's contribution at a collision —
    /// `score_collision_derivative` (`src/tallies/derivative.cpp`).
    ///
    /// `material_idx` is where the collision happened, `event_nuclide` which
    /// nuclide was struck, `e` the incident energy \[eV\].
    ///
    /// A collision in a different material, or (for a nuclide-density
    /// derivative) on a different nuclide, contributes nothing — which is
    /// correct and is why a derivative on a material the particle never enters
    /// comes out at exactly zero rather than as an error.
    ///
    /// # Errors
    ///
    /// A temperature derivative on a nuclide whose cross sections do not vary
    /// with the temperature argument. See the module docs.
    pub fn accumulate_at_collision(
        &self,
        flux_deriv: &mut FluxDerivative,
        material_idx: usize,
        material: &Material,
        nuclides: &[Nuclide],
        event_nuclide: usize,
        e: f64,
    ) -> Result<(), String> {
        if material_idx != self.material_idx {
            return Ok(());
        }
        match self.variable {
            DerivativeVariable::Density => {
                // phi ~ Sigma_s, and Sigma_s ~ rho at fixed composition, so
                // (1/phi)(d phi/d rho) = 1/rho.
                let rho: f64 = material.components.iter().map(|c| c.atom_density).sum();
                if !(rho > 0.0) {
                    return Err(format!(
                        "material {material_idx} has total atom density {rho}; a density \
                         derivative of a void is undefined, not zero"
                    ));
                }
                flux_deriv.value += 1.0 / rho;
            }
            DerivativeVariable::NuclideDensity { nuclide_idx } => {
                if event_nuclide != nuclide_idx {
                    return Ok(());
                }
                let Some(c) = material
                    .components
                    .iter()
                    .find(|c| c.nuclide_idx == nuclide_idx)
                else {
                    return Err(format!(
                        "nuclide {nuclide_idx} was struck in material {material_idx} but is \
                         not one of its components; the derivative is on the wrong material"
                    ));
                };
                if !(c.atom_density > 0.0) {
                    return Err(format!(
                        "nuclide {nuclide_idx} has atom density {} in material \
                         {material_idx}; its logarithmic derivative is undefined",
                        c.atom_density
                    ));
                }
                flux_deriv.value += 1.0 / c.atom_density;
            }
            DerivativeVariable::Temperature => {
                let nuc = &nuclides[event_nuclide];
                let t = material.temperature;
                let h = Self::TEMPERATURE_STEP_K;
                let lo = nuc.xs_at_energy(e, t - h);
                let hi = nuc.xs_at_energy(e, t + h);
                let sig_s_lo = (lo.total - lo.absorption).max(0.0);
                let sig_s_hi = (hi.total - hi.absorption).max(0.0);
                if sig_s_lo == sig_s_hi {
                    return Err(format!(
                        "the scattering cross section of {} at {e} eV does not move between \
                         {} K and {} K, so a finite-difference temperature derivative is \
                         exactly zero. That is the POINTWISE tier, where cross sections are \
                         broadened once at construction and the temperature argument is \
                         ignored - not a physical result. Use the WMP (Core) tier, or a \
                         paired run at two temperatures.",
                        nuc.name,
                        t - h,
                        t + h
                    ));
                }
                let sig_s = 0.5 * (sig_s_lo + sig_s_hi);
                if !(sig_s > 0.0) {
                    return Ok(());
                }
                flux_deriv.value += (sig_s_hi - sig_s_lo) / (2.0 * h) / sig_s;
            }
        }
        Ok(())
    }

    /// Turn a score into its derivative —
    /// `apply_derivative_to_score` (`src/tallies/derivative.cpp`).
    ///
    /// `score` is the undifferentiated value, `flux_deriv` the accumulated
    /// logarithmic flux derivative, `material_idx` where the score was made.
    ///
    /// # Errors
    ///
    /// A score upstream refuses to differentiate. Upstream calls
    /// `fatal_error` there; this returns so a caller can decide, but it does
    /// **not** fall back to `score * flux_deriv`, which would be the
    /// flux-derivative answer wearing a reaction-rate label.
    pub fn apply_to_score(
        &self,
        score: f64,
        flux_deriv: &FluxDerivative,
        material_idx: Option<usize>,
        material: Option<&Material>,
        score_type: ScoreType,
    ) -> Result<f64, String> {
        if score == 0.0 {
            return Ok(0.0);
        }
        // A flux score has no explicit dependence on the perturbed variable,
        // so only the flux derivative survives. Same for a void, and for a
        // material that is not the perturbed one.
        if score_type == ScoreType::Flux
            || material_idx != Some(self.material_idx)
            || material.is_none()
        {
            return Ok(score * flux_deriv.value);
        }
        let material = material.expect("checked above");
        let explicit = match self.variable {
            DerivativeVariable::Density => {
                let rho: f64 = material.components.iter().map(|c| c.atom_density).sum();
                match score_type {
                    ScoreType::Total
                    | ScoreType::Scatter
                    | ScoreType::NuScatter
                    | ScoreType::ScatterN
                    | ScoreType::Absorption
                    | ScoreType::Fission
                    | ScoreType::NuFission
                    | ScoreType::PromptNuFission
                    | ScoreType::DelayedNuFission => 1.0 / rho,
                    other => {
                        return Err(format!(
                            "a density derivative of {other:?} is not defined; upstream \
                             raises a fatal error here rather than guessing"
                        ))
                    }
                }
            }
            DerivativeVariable::NuclideDensity { nuclide_idx } => {
                let Some(c) = material
                    .components
                    .iter()
                    .find(|c| c.nuclide_idx == nuclide_idx)
                else {
                    return Err(format!(
                        "nuclide {nuclide_idx} is not in material {}",
                        self.material_idx
                    ));
                };
                match score_type {
                    ScoreType::Total
                    | ScoreType::Scatter
                    | ScoreType::NuScatter
                    | ScoreType::ScatterN
                    | ScoreType::Absorption
                    | ScoreType::Fission
                    | ScoreType::NuFission
                    | ScoreType::PromptNuFission
                    | ScoreType::DelayedNuFission => 1.0 / c.atom_density,
                    other => {
                        return Err(format!(
                            "a nuclide-density derivative of {other:?} is not defined"
                        ))
                    }
                }
            }
            // Upstream differentiates the reaction cross section too; this
            // port has no analytic dsigma/dT, so only the flux term is
            // carried and the SHORTFALL IS STATED rather than the answer
            // being presented as complete.
            DerivativeVariable::Temperature => {
                return Err(
                    "a temperature derivative of a REACTION RATE needs d(sigma_MT)/dT, \
                     which this port does not have (see the module docs). The FLUX \
                     derivative is available and is what `ScoreType::Flux` returns."
                        .into(),
                )
            }
        };
        Ok(score * (flux_deriv.value + explicit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;

    fn two_nuclide_material() -> Material {
        Material {
            id: 1,
            name: "fuel".into(),
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.0e-2,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 1.0e-2,
                },
            ],
            temperature: 293.6,
        }
    }

    /// A density derivative accumulates `1/ρ` at every collision in the
    /// perturbed material and **nothing** elsewhere.
    #[test]
    fn a_density_derivative_accumulates_one_over_rho_only_in_its_own_material() {
        let d = TallyDerivative {
            material_idx: 3,
            variable: DerivativeVariable::Density,
        };
        let m = two_nuclide_material();
        let rho = 5.0e-2;
        let mut f = FluxDerivative::default();

        // Three collisions in material 3, two somewhere else.
        for _ in 0..3 {
            d.accumulate_at_collision(&mut f, 3, &m, &[], 0, 1.0e6).unwrap();
        }
        for _ in 0..2 {
            d.accumulate_at_collision(&mut f, 7, &m, &[], 0, 1.0e6).unwrap();
        }
        assert!(
            (f.value - 3.0 / rho).abs() < 1e-9,
            "flux derivative {} vs {}",
            f.value,
            3.0 / rho
        );
    }

    /// A nuclide-density derivative accumulates only on **its own nuclide**.
    #[test]
    fn a_nuclide_density_derivative_ignores_the_other_nuclide() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::NuclideDensity { nuclide_idx: 1 },
        };
        let m = two_nuclide_material();
        let mut f = FluxDerivative::default();
        d.accumulate_at_collision(&mut f, 0, &m, &[], 0, 1.0e6).unwrap(); // wrong nuclide
        assert_eq!(f.value, 0.0);
        d.accumulate_at_collision(&mut f, 0, &m, &[], 1, 1.0e6).unwrap();
        assert!((f.value - 1.0 / 1.0e-2).abs() < 1e-9, "{}", f.value);
    }

    /// A void cannot be density-perturbed, and that is an error rather than a
    /// zero — a zero would read as "this material does not matter".
    #[test]
    fn a_density_derivative_of_a_void_is_an_error_not_a_zero() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Density,
        };
        let void = Material {
            id: 9,
            name: "void".into(),
            components: vec![],
            temperature: 293.6,
        };
        let mut f = FluxDerivative::default();
        let err = d
            .accumulate_at_collision(&mut f, 0, &void, &[], 0, 1.0e6)
            .unwrap_err();
        assert!(err.contains("undefined, not zero"), "{err}");
    }

    /// A flux score carries only the flux derivative; a reaction rate carries
    /// the explicit term too.
    #[test]
    fn a_flux_score_carries_only_the_flux_derivative() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Density,
        };
        let m = two_nuclide_material();
        let f = FluxDerivative { value: 20.0 };

        let flux = d
            .apply_to_score(2.0, &f, Some(0), Some(&m), ScoreType::Flux)
            .unwrap();
        assert!((flux - 40.0).abs() < 1e-12, "{flux}");

        // Reaction rate: 2 * (20 + 1/0.05) = 2 * 40 = 80.
        let rate = d
            .apply_to_score(2.0, &f, Some(0), Some(&m), ScoreType::Fission)
            .unwrap();
        assert!((rate - 80.0).abs() < 1e-9, "{rate}");
    }

    /// A score in an unperturbed material, or in a void, keeps only the flux
    /// term.
    #[test]
    fn an_unperturbed_material_keeps_only_the_flux_term() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Density,
        };
        let m = two_nuclide_material();
        let f = FluxDerivative { value: 3.0 };
        assert_eq!(
            d.apply_to_score(2.0, &f, Some(5), Some(&m), ScoreType::Fission)
                .unwrap(),
            6.0
        );
        assert_eq!(
            d.apply_to_score(2.0, &f, None, None, ScoreType::Fission)
                .unwrap(),
            6.0
        );
    }

    /// **A score upstream refuses is refused here too**, rather than falling
    /// back to the flux-derivative answer wearing a reaction-rate label.
    #[test]
    fn an_undefined_derivative_is_refused_rather_than_approximated() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Density,
        };
        let m = two_nuclide_material();
        let f = FluxDerivative { value: 1.0 };
        let err = d
            .apply_to_score(1.0, &f, Some(0), Some(&m), ScoreType::KappaFission)
            .unwrap_err();
        assert!(err.contains("not defined"), "{err}");

        // A temperature derivative of a reaction rate needs d(sigma)/dT, which
        // this port does not have.
        let t = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Temperature,
        };
        let err = t
            .apply_to_score(1.0, &f, Some(0), Some(&m), ScoreType::Fission)
            .unwrap_err();
        assert!(err.contains("does not have"), "{err}");
        // The FLUX derivative is still available.
        assert_eq!(
            t.apply_to_score(2.0, &f, Some(0), Some(&m), ScoreType::Flux)
                .unwrap(),
            2.0
        );
    }

    /// A zero score differentiates to zero without touching anything.
    #[test]
    fn a_zero_score_differentiates_to_zero() {
        let d = TallyDerivative {
            material_idx: 0,
            variable: DerivativeVariable::Density,
        };
        let f = FluxDerivative { value: 1.0e9 };
        assert_eq!(
            d.apply_to_score(0.0, &f, Some(0), None, ScoreType::Fission)
                .unwrap(),
            0.0
        );
    }
}
