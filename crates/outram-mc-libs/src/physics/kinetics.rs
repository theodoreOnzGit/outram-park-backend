// SPDX-License-Identifier: GPL-3.0

//! **Point-kinetics parameters from a Monte Carlo run** — the effective
//! delayed fraction `β_eff` and the neutron generation time `Λ`. GitHub #262.
//!
//! These are the two numbers a point-kinetics model needs, and
//! `crates/teh-o-prke` exists in this workspace with nothing feeding it. This
//! module is that feed.
//!
//! # Two definitions, and only one of them is here
//!
//! **The `k`-ratio route** (this module): solve the eigenvalue twice, once
//! with the full yield and once with the prompt yield alone, and take
//!
//! ```text
//! beta_eff ~ 1 - k_p / k
//! ```
//!
//! It is cheap, it is the standard first answer, and it is **biased**: it is
//! the "prompt-`k`" definition, not the adjoint-weighted one. Delayed neutrons
//! are born softer than prompt ones, so in a fast system they are *less* worth
//! than average and the adjoint-weighted `β_eff` is smaller than the bare
//! delayed fraction; in a thermal system the sign of the correction flips. The
//! ratio route captures part of that through the two eigenvalue solves and
//! **not all of it**.
//!
//! **The IFP route** (`src/ifp.cpp`, not ported): adjoint-weighted, the
//! correct definition, and what makes the number defensible. It needs
//! generation lineage on the particle. `beta_eff_from_k_ratio`'s docs say
//! plainly that its answer is the biased one, because a `β_eff` quoted without
//! that qualifier is the kind of number that gets used in a safety argument.
//!
//! # `Λ`
//!
//! ```text
//! Lambda = integral(phi / v) / integral(nu Sigma_f phi)
//! ```
//!
//! — the neutron population divided by the production rate, i.e. seconds. Both
//! integrals come from the same run as flux-weighted tallies
//! ([`crate::tally::tally::ScoreType::InverseVelocity`] and
//! [`crate::tally::tally::ScoreType::NuFission`]), so this is one extra tally
//! rather than a second calculation. Like `β_eff` above it is the
//! **non-adjoint-weighted** form.

use crate::material::material::Material;
use crate::material::nuclide::Nuclide;

/// The kinetics parameters of one system, with their provenance attached.
#[derive(Debug, Clone, PartialEq)]
pub struct KineticsParameters {
    /// Effective delayed fraction, dimensionless (not pcm, not per cent).
    pub beta_eff: f64,
    /// 1σ uncertainty on [`Self::beta_eff`], propagated from the two
    /// eigenvalues.
    pub beta_eff_sigma: f64,
    /// Neutron generation time \[s\].
    pub lambda_generation: f64,
    /// The total eigenvalue `k`.
    pub k_total: f64,
    /// The prompt eigenvalue `k_p`.
    pub k_prompt: f64,
    /// **How this was obtained.** Carried on the value so it cannot be quoted
    /// without it.
    pub method: KineticsMethod,
}

/// Which definition produced a [`KineticsParameters`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KineticsMethod {
    /// `1 − k_p/k` from two eigenvalue solves. Biased; see the module docs.
    KRatio,
    /// Adjoint-weighted iterated fission probability. **Not implemented** —
    /// the variant exists so a consumer can match on it and so that an
    /// IFP result, when it lands, is distinguishable from a ratio one rather
    /// than silently replacing it.
    IteratedFissionProbability,
}

/// `β_eff` from two eigenvalues — the ratio route.
///
/// `k` is the ordinary eigenvalue and `k_p` the one obtained with every
/// nuclide in [`Nuclide::with_prompt_only_nubar`]. The two runs must be
/// otherwise identical, including the seed: the difference is a few hundred
/// pcm on a fast system and seed-to-seed scatter is comparable, so an unpaired
/// pair of runs measures noise.
///
/// The uncertainty assumes the two eigenvalues are **independent**, which
/// paired runs are not — they share a seed and a geometry, so the quoted sigma
/// is conservative. That direction is the safe one and is stated rather than
/// corrected by an assumed correlation nobody measured.
pub fn beta_eff_from_k_ratio(
    k: f64,
    k_sigma: f64,
    k_prompt: f64,
    k_prompt_sigma: f64,
) -> Result<f64, String> {
    if !(k > 0.0) {
        return Err(format!("k = {k} is not positive; there is no ratio to take"));
    }
    if k_prompt > k {
        return Err(format!(
            "k_prompt = {k_prompt} exceeds k = {k}, which would make beta_eff negative. \
             Either the prompt-only ablation was not applied, or the two runs are not \
             paired and this is seed scatter (k_sigma {k_sigma}, k_prompt_sigma \
             {k_prompt_sigma})."
        ));
    }
    Ok(1.0 - k_prompt / k)
}

/// The neutron generation time from an inverse-velocity and a ν-fission tally.
///
/// `inverse_velocity` is `∫ φ/v` \[neutrons per source neutron\] and
/// `nu_fission` is `∫ νΣ_f φ` \[neutrons per second per source neutron\], both
/// from the same run. Their ratio is seconds.
///
/// # Errors
///
/// A non-positive production integral — a system that produced no fission
/// neutrons has no generation time, and returning zero or infinity would read
/// as one.
pub fn generation_time(inverse_velocity: f64, nu_fission: f64) -> Result<f64, String> {
    if !(nu_fission > 0.0) {
        return Err(format!(
            "the nu-fission tally is {nu_fission}; a system that produces no fission \
             neutrons has no generation time"
        ));
    }
    if !(inverse_velocity >= 0.0) {
        return Err(format!("the inverse-velocity tally is {inverse_velocity}"));
    }
    Ok(inverse_velocity / nu_fission)
}

/// Assemble the parameters, refusing the cases where the answer would be
/// meaningless rather than returning it.
///
/// # Errors
///
/// - `k_p > k`, per [`beta_eff_from_k_ratio`].
/// - A zero production integral, per [`generation_time`].
/// - **A β of exactly zero**, which on this path means the evaluations carry
///   no MT=455 rather than that the system has no delayed neutrons. Use
///   [`delayed_data_is_complete`] to distinguish the two before running.
pub fn from_k_ratio(
    k: f64,
    k_sigma: f64,
    k_prompt: f64,
    k_prompt_sigma: f64,
    inverse_velocity: f64,
    nu_fission: f64,
) -> Result<KineticsParameters, String> {
    let beta_eff = beta_eff_from_k_ratio(k, k_sigma, k_prompt, k_prompt_sigma)?;
    if beta_eff == 0.0 {
        return Err(
            "beta_eff came out exactly zero. On the k-ratio path that means the prompt \
             and total eigenvalues were identical, which happens when the evaluations \
             carry no MF=1/455 delayed data at all - not when the system has no delayed \
             neutrons. Check `delayed_data_is_complete` first."
                .into(),
        );
    }
    // d(beta)/dk = k_p/k^2, d(beta)/dk_p = -1/k.
    let beta_eff_sigma = ((k_prompt / (k * k) * k_sigma).powi(2)
        + (k_prompt_sigma / k).powi(2))
    .sqrt();
    Ok(KineticsParameters {
        beta_eff,
        beta_eff_sigma,
        lambda_generation: generation_time(inverse_velocity, nu_fission)?,
        k_total: k,
        k_prompt,
        method: KineticsMethod::KRatio,
    })
}

/// Whether every fissionable nuclide in every material carries MF=1/455.
///
/// A `false` here means a kinetics result from this model is missing part of
/// its delayed production, so the β it yields is a **lower bound**, not a
/// measurement. Checked before a run rather than inferred from a small answer
/// afterwards.
pub fn delayed_data_is_complete(materials: &[Material], nuclides: &[Nuclide]) -> bool {
    materials.iter().all(|m| m.has_delayed_data(nuclides))
}

/// The nuclides of a model with the prompt-only ablation applied to all of
/// them — the second arm of the ratio route.
pub fn prompt_only(nuclides: &[Nuclide]) -> Vec<Nuclide> {
    nuclides
        .iter()
        .cloned()
        .map(Nuclide::with_prompt_only_nubar)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arithmetic, on numbers chosen so the answer is exact.
    #[test]
    fn the_k_ratio_beta_is_one_minus_the_ratio() {
        // k = 1.0, k_p = 0.9935 -> beta = 65 pcm exactly.
        let b = beta_eff_from_k_ratio(1.0, 1.0e-4, 0.9935, 1.0e-4).unwrap();
        assert!((b - 0.0065).abs() < 1e-15, "beta = {b}");
    }

    /// `k_p > k` is refused, not reported as a negative delayed fraction.
    ///
    /// This is the failure mode that matters: unpaired runs differ by
    /// seed scatter of the same order as the effect, so the ratio can come out
    /// the wrong way round, and a negative β would propagate into a kinetics
    /// model as a prompt-supercritical system.
    #[test]
    fn a_prompt_eigenvalue_above_the_total_is_refused() {
        let err = beta_eff_from_k_ratio(0.99, 1.0e-3, 1.01, 1.0e-3).unwrap_err();
        assert!(err.contains("exceeds"), "{err}");
        assert!(beta_eff_from_k_ratio(0.0, 1.0e-3, 0.0, 1.0e-3).is_err());
    }

    /// A zero β is refused on this path, because it means missing data rather
    /// than a system without delayed neutrons.
    #[test]
    fn an_exactly_zero_beta_is_refused_as_missing_data() {
        let err = from_k_ratio(1.0, 1e-4, 1.0, 1e-4, 1.0, 1.0).unwrap_err();
        assert!(err.contains("no MF=1/455"), "{err}");
    }

    /// Λ is seconds, and its inputs are checked.
    #[test]
    fn the_generation_time_is_the_population_over_the_production() {
        // 1e-7 neutrons of population per 1.0 neutron/s of production -> 100 ns.
        let l = generation_time(1.0e-7, 1.0).unwrap();
        assert!((l - 1.0e-7).abs() < 1e-20, "Lambda = {l}");
        assert!(generation_time(1.0e-7, 0.0).is_err());
        assert!(generation_time(-1.0, 1.0).is_err());
    }

    /// The uncertainty propagates both eigenvalues.
    #[test]
    fn the_beta_uncertainty_carries_both_eigenvalues() {
        let p = from_k_ratio(1.0, 2.0e-4, 0.993, 2.0e-4, 1.0e-7, 1.0).unwrap();
        assert_eq!(p.method, KineticsMethod::KRatio);
        // Both terms are ~2e-4, so sigma ~ sqrt(2) * 2e-4.
        let want = ((0.993 * 2.0e-4_f64).powi(2) + (2.0e-4_f64).powi(2)).sqrt();
        assert!(
            (p.beta_eff_sigma - want).abs() < 1e-12,
            "sigma {} vs {want}",
            p.beta_eff_sigma
        );
        assert!(p.beta_eff_sigma > 2.0e-4, "one eigenvalue alone would be smaller");
    }
}
