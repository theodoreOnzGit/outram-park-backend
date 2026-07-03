//! `HEATR` — heating (KERMA) cross section, ENDF MT=301.
//!
//! Computes the **kinematic-limit KERMA**: the heating cross section under the
//! assumption that every escaping *neutron* carries its kinetic energy away
//! from the local region, and everything else (nuclear recoil, charged
//! particles, and — per NJOY's own documented fallback — photon energy when no
//! photon-production data is processed) deposits locally. `heatr.f90` computes
//! this exact quantity as a **check** (`kchk`) against its full photon
//! energy-balance method; here it is the primary (and, for now, only) result.
//!
//! Ported in phases (`docs/porting-plan.md` §HEATR sub-phases) — see the
//! module's own progress:
//!
//! - **H1** (done): elastic (MT=2).
//! - **H2** (done): local-deposition reactions — capture (MT=102) and
//!   charged-particle-only exits (MT=103–117), `H(E) = σ(E)·(E+Q)`.
//! - **H3** (done): single-escaping-neutron reactions — discrete inelastic
//!   levels (MT=51–90, MT=4) and the `(n,n'X)` family (MT=22, 23, 28, 29,
//!   32–36, 44, 45), `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` (see
//!   [`single_neutron_factor`] for the derivation).
//! - **H4** (done): fission (MT=18, 19–21, 38),
//!   `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, reusing [`NuBar`] and
//!   [`FissionSpectrum::mean_energy`].
//! - **H5** (done): multi-neutron-exit + continuum inelastic (MT=11, 16, 17,
//!   24, 25, 30, 37, 41, 42, 91), `H(E) = σ(E)·[E + Q − ȳ·⟨E'⟩]`, where `ȳ` is
//!   the emitted-neutron multiplicity (fixed by the MT) and `⟨E'⟩` the mean
//!   emitted-neutron energy from the reaction's own MF=5 secondary spectrum
//!   (reusing [`FissionSpectrum::mean_energy`] — the same first-moment machinery
//!   H4 uses for χ). This is the direct generalization of H3/H4's balance:
//!   `ȳ` neutrons each escape carrying the emission spectrum's mean energy, and
//!   `E + Q` minus that total deposits locally. A reaction whose emission
//!   spectrum is not supplied contributes 0 (excluded, not guessed).
//! - **H6–H7** (deferred): the full photon energy-balance method, and damage
//!   energy (MT=444).
//!
//! ## Elastic kinematics (H1)
//!
//! For isotropic scattering in the centre-of-mass frame off a target of mass
//! ratio `A` (`= AWR`, target/neutron mass) initially at rest, the *average*
//! post-collision lab-frame neutron energy is the standard two-body result
//!
//! ```text
//!   ⟨E'⟩ = E · (1 + A²) / (A + 1)²
//! ```
//!
//! so the average energy transferred to the recoiling nucleus — and thus
//! deposited locally — is
//!
//! ```text
//!   H(E) = E − ⟨E'⟩ = E · 2A / (A + 1)².
//! ```
//!
//! For `A = 1` (hydrogen) this gives `H = E/2`: an elastic collision with a
//! free proton loses on average exactly half its energy — a textbook result,
//! and the module's primary correctness check.

use crate::endf::MtReaction;
use crate::nuclear_data::secondary::{FissionSpectrum, NuBar};
use crate::reconr::{eval_lin_lin, ReconrResult};

/// Which heating model a reaction MT uses in the kinematic-limit KERMA — the
/// closed dispatch [`heating_model`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeatingModel {
    /// Elastic (MT=2): `H(E) = σ(E)·E·2A/(A+1)²` — see the module docs (H1).
    Elastic,
    /// **H2.** No escaping neutron — pure capture (MT=102) or capture plus
    /// charged particle(s) that stay local in matter (MT=103–117):
    /// `H(E) = σ(E)·(E+Q)`. All of the incident energy plus the reaction
    /// Q-value is deposited, since nothing escapes to carry energy away —
    /// this is NJOY's own documented behaviour for materials/reactions
    /// without photon-production data taken to its logical conclusion.
    Local,
    /// **H3.** Exactly one escaping neutron, with the reaction's own Q-value
    /// (discrete inelastic levels MT=51–90 and the lumped MT=4; the
    /// `(n,n'X)`-family reactions MT=22, 23, 28, 29, 32–36, 44, 45, whose
    /// charged-particle byproduct(s) stay local like H2's):
    /// `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` — the two-body-with-Q
    /// generalization of H1's elastic formula (reduces to it at `Q=0`; see
    /// the derivation in [`single_neutron_factor`]).
    SingleNeutron,
    /// **H4.** Fission (MT=18 and the chance-fission partials MT=19–21, 38):
    /// `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, where `ν̄(E)` is the total
    /// (prompt+delayed) neutron yield and `⟨E'⟩` the mean fission-neutron
    /// energy from χ(E→E') at incident energy `E` — the multi-neutron
    /// generalization of H2/H3's single-particle energy balance: `ν̄` escaping
    /// neutrons each carry away the birth spectrum's mean energy, and
    /// everything else (fragment KE, prompt/delayed γ and β) deposits locally.
    Fission,
    /// **H5.** Multi-neutron-exit (MT=11, 16, 17, 24, 25, 30, 37, 41, 42) and
    /// continuum inelastic (MT=91): `ȳ` neutrons escape, each carrying the mean
    /// energy `⟨E'⟩` of the reaction's MF=5 secondary-neutron spectrum, so
    /// `H(E) = σ(E)·[E + Q − ȳ·⟨E'⟩]`. Unlike H1–H4 the mean has no closed
    /// kinematic form, so the emission spectrum must be supplied (via the
    /// `emission` argument of [`Kerma::from_reconr`]); the multiplicity `ȳ`
    /// comes from the MT ([`neutron_multiplicity`]). If no spectrum is supplied
    /// for the reaction, it contributes 0 (falls through to [`Self::NotModeled`]
    /// behaviour rather than guessing an energy loss).
    MultiNeutron,
    /// Not modeled by the kinematic-limit KERMA — everything left to H6/H7 (the
    /// full photon energy-balance method and MT=444 damage energy), plus any
    /// H5 reaction whose emission spectrum was not supplied: contributes 0.
    NotModeled,
}

/// Dispatch an [`MtReaction`] to its [`HeatingModel`].
fn heating_model(mt: MtReaction) -> HeatingModel {
    use MtReaction::*;
    match mt {
        Mt2Elastic => HeatingModel::Elastic,
        Mt102Capture | Mt103Np | Mt104Nd | Mt105Nt | Mt106NHe3 | Mt107NAlpha | Mt108N2Alpha
        | Mt109N3Alpha | Mt111N2Proton | Mt112NProtonAlpha | Mt113NT2Alpha | Mt114ND2Alpha
        | Mt115NProtonD | Mt116NProtonT | Mt117NDAlpha => HeatingModel::Local,
        Mt4Inelastic | Mt22NnAlpha | Mt23Nn3Alpha | Mt28NnProton | Mt29Nn2Alpha | Mt32NnD
        | Mt33NnT | Mt34NnHe3 | Mt35NnD2Alpha | Mt36NnT2Alpha | Mt44Nn2Proton
        | Mt45NnProtonAlpha => HeatingModel::SingleNeutron,
        // MT=51–90: discrete inelastic levels. Matched by number, not name —
        // each level has its own named variant (`Mt51NnLevel1`, …, up to
        // `Mt90…`), too many to enumerate individually here.
        _ if (51..=90).contains(&mt.number()) => HeatingModel::SingleNeutron,
        Mt18Fission
        | Mt19FirstChanceFission
        | Mt20SecondChanceFission
        | Mt21ThirdChanceFission
        | Mt38FourthChanceFission => HeatingModel::Fission,
        Mt11N2nD | Mt16N2n | Mt17N3n | Mt24N2nAlpha | Mt25N3nAlpha | Mt30N2n2Alpha | Mt37N4n
        | Mt41N2nProton | Mt42N3nProton | Mt91NnContinuum => HeatingModel::MultiNeutron,
        _ => HeatingModel::NotModeled,
    }
}

/// Number of neutrons `ȳ` emitted by an [`HeatingModel::MultiNeutron`] reaction —
/// the multiplicity in NJOY's `H = σ·(E + Q − ȳ·⟨E'⟩)` (`heatr.f90::nheat`,
/// where it is `yld`). Fixed by the MT's exit channel: (n,2n)-type → 2,
/// (n,3n)-type → 3, (n,4n) → 4, continuum inelastic (n,n') → 1. Returns 0 for
/// any MT that is not an H5 reaction (never reached — [`heating_model`] gates it).
fn neutron_multiplicity(mt: MtReaction) -> f64 {
    use MtReaction::*;
    match mt {
        Mt91NnContinuum => 1.0,
        Mt11N2nD | Mt16N2n | Mt24N2nAlpha | Mt30N2n2Alpha | Mt41N2nProton => 2.0,
        Mt17N3n | Mt25N3nAlpha | Mt42N3nProton => 3.0,
        Mt37N4n => 4.0,
        _ => 0.0,
    }
}

/// The MF=5 emitted-neutron energy spectrum supplied for reaction `mt`, if any.
/// H5 has no closed-form kinematic mean, so [`Kerma::from_reconr`] needs the
/// actual secondary spectrum to take its first moment `⟨E'⟩`; a reaction absent
/// from `emission` contributes 0 (see [`HeatingModel::MultiNeutron`]).
fn emission_spectrum(
    emission: &[(MtReaction, FissionSpectrum)],
    mt: MtReaction,
) -> Option<&FissionSpectrum> {
    emission.iter().find(|(m, _)| *m == mt).map(|(_, s)| s)
}

/// The `2A/(A+1)²` factor from the two-body elastic recoil formula (H1),
/// shared by [`HeatingModel::Elastic`] (`Q=0`) and [`HeatingModel::SingleNeutron`]
/// (`Q≠0`, see the derivation below).
///
/// **Derivation (2-body reaction with Q-value, isotropic CM scattering).**
/// For neutron mass 1, target mass `A` at rest, incident lab energy `E`: the
/// CM-frame kinetic energy available before the reaction is `A/(A+1)·E`; after
/// the reaction it is `A/(A+1)·E + Q`. Splitting that between the outgoing
/// neutron (mass 1) and residual (mass `A`) inversely by mass, then
/// transforming back to the lab frame and averaging the neutron's lab energy
/// over an isotropic CM cosine (the cross term vanishes on average) gives
///
/// ```text
///   ⟨E'⟩ = E·(1+A²)/(A+1)² + A·Q/(A+1)
/// ```
///
/// so the energy deposited locally, `H = E + Q − ⟨E'⟩`, simplifies to
///
/// ```text
///   H(E) = E·2A/(A+1)² + Q/(A+1).
/// ```
///
/// At `Q=0` this is exactly H1's elastic formula — confirming the two are the
/// same physics with (H1) and without (H3) an excitation/reaction Q-value.
fn single_neutron_factor(awr: f64) -> f64 {
    2.0 * awr / (awr + 1.0).powi(2)
}

/// Heating (KERMA) cross section, ENDF **MT=301**, vs incident energy \[eV\].
///
/// Built once from a [`ReconrResult`] via [`Kerma::from_reconr`]; evaluated
/// with [`Kerma::eval`]. Units are \[eV·barn\] (energy × cross section — the
/// standard ENDF heating-cross-section convention), summed over every
/// reaction [`heating_model`] currently covers.
#[derive(Debug, Clone, Default)]
pub struct Kerma {
    /// Union incident-energy grid \[eV\], ascending, deduplicated.
    pub energy: Vec<f64>,
    /// Σ over covered reactions of `σ_mt(E)·H_mt(E)` \[eV·barn\], aligned with
    /// `energy`.
    pub h: Vec<f64>,
}

impl Kerma {
    /// Compute the kinematic-limit KERMA from a reconstructed evaluation.
    ///
    /// **H1–H5** are covered (elastic; capture + charged-particle-only exits;
    /// single-escaping-neutron reactions; fission; multi-neutron-exit + continuum
    /// inelastic); every other reaction contributes 0 (see the module docs).
    ///
    /// - `awr` is the target's mass ratio (`ReconrResult::material::awr`).
    /// - `nu`/`chi` are the fission neutron yield and birth spectrum (from
    ///   [`crate::nuclear_data::secondary`]) used only by the `Fission` model —
    ///   pass [`NuBar::default`]/[`FissionSpectrum::default`] for a
    ///   non-fissionable material (no MT=18 section ⇒ they are never evaluated).
    /// - `emission` maps each H5 multi-neutron / continuum-inelastic reaction
    ///   (MT=11, 16, 17, 24, 25, 30, 37, 41, 42, 91) to its MF=5 emitted-neutron
    ///   spectrum; the mean of that spectrum is subtracted `ȳ` times (once per
    ///   escaping neutron). A reaction present in `recon` but absent from
    ///   `emission` contributes 0 — pass `&[]` to skip all of H5.
    pub fn from_reconr(
        recon: &ReconrResult,
        nu: &NuBar,
        chi: &FissionSpectrum,
        emission: &[(MtReaction, FissionSpectrum)],
    ) -> Self {
        let awr = recon.material.awr;
        // A section contributes to the grid only if its heating is actually
        // modeled — for H5 that additionally requires a supplied emission spectrum.
        let contributes = |mt: MtReaction| match heating_model(mt) {
            HeatingModel::NotModeled => false,
            HeatingModel::MultiNeutron => emission_spectrum(emission, mt).is_some(),
            _ => true,
        };
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| contributes(s.mt))
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let two_body_factor = single_neutron_factor(awr);
        let mut h = vec![0.0; energy.len()];
        for sec in &recon.sections {
            let model = heating_model(sec.mt);
            if model == HeatingModel::NotModeled {
                continue;
            }
            // Resolve the H5 emission spectrum once per section; skip the whole
            // section (contributes 0) if it is a MultiNeutron reaction with none.
            let multi_spec = if model == HeatingModel::MultiNeutron {
                match emission_spectrum(emission, sec.mt) {
                    Some(spec) => Some((spec, neutron_multiplicity(sec.mt))),
                    None => continue,
                }
            } else {
                None
            };
            for (i, &e) in energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                let per_event = match model {
                    HeatingModel::Elastic => e * two_body_factor,
                    HeatingModel::Local => e + sec.qi,
                    HeatingModel::SingleNeutron => e * two_body_factor + sec.qi / (awr + 1.0),
                    HeatingModel::Fission => e + sec.qi - nu.at(e) * chi.mean_energy(e),
                    HeatingModel::MultiNeutron => {
                        let (spec, yld) = multi_spec.unwrap();
                        e + sec.qi - yld * spec.mean_energy(e)
                    }
                    HeatingModel::NotModeled => unreachable!(),
                };
                h[i] += sigma * per_event;
            }
        }
        Kerma { energy, h }
    }

    /// Evaluate the heating cross section \[eV·barn\] at incident energy `e`
    /// \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, e: f64) -> f64 {
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self
            .energy
            .iter()
            .copied()
            .zip(self.h.iter().copied())
            .collect();
        eval_lin_lin(&pairs, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::{MaterialInfo, ReconrSection};

    fn material(awr: f64) -> MaterialInfo {
        MaterialInfo {
            za: 1001.0,
            awr,
            lrp: 0,
            lfi: 0,
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: 2.0e7,
        }
    }

    fn recon(awr: f64, sections: Vec<ReconrSection>) -> ReconrResult {
        ReconrResult {
            material: material(awr),
            sections,
        }
    }

    /// Hydrogen (A=1) elastic scattering loses on average exactly half the
    /// neutron's energy per collision — the textbook check for this formula.
    #[test]
    fn hydrogen_elastic_loses_half_its_energy() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e5, 20.0), (1.0e6, 20.0), (1.0e7, 20.0)], // flat 20 b
        };
        let kerma = Kerma::from_reconr(
            &recon(1.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        for &e in &[1.0e5, 1.0e6, 1.0e7] {
            let h = kerma.eval(e);
            let expected = 20.0 * e * 0.5; // σ·E/2
            assert!(
                (h - expected).abs() / expected < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// A heavy target (A≫1) transfers only a small fraction of the incident
    /// energy per elastic collision — `H(E)/(:σE) → 2/A` as `A → ∞`.
    #[test]
    fn heavy_target_transfers_small_fraction() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 5.0), (2.0e6, 5.0)],
        };
        let awr = 238.0;
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let sigma_e = 5.0 * 1.0e6;
        let frac = h / sigma_e;
        let approx_2_over_a = 2.0 / awr;
        assert!(
            frac > 0.0 && frac < 0.01,
            "heavy-target fraction {frac} should be small"
        );
        assert!(
            (frac - approx_2_over_a).abs() / approx_2_over_a < 0.02,
            "frac={frac} ≈ 2/A={approx_2_over_a}"
        );
    }

    /// **H5, no-spectrum guard.** A multi-neutron reaction (MT=16 (n,2n)) with no
    /// emission spectrum supplied contributes nothing — H5 has no closed-form
    /// mean, so without the secondary spectrum the dispatch excludes the section
    /// entirely (empty grid) rather than guessing an energy loss. Passing the
    /// spectrum (see [`n2n_heating_subtracts_two_neutron_means`]) turns it on.
    #[test]
    fn multineutron_without_spectrum_contributes_nothing() {
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: -8.0e6,
            pairs: vec![(1.0e5, 1.0), (1.0e6, 1.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[], // no emission spectrum for MT=16
        );
        assert!(
            kerma.energy.is_empty(),
            "(n,2n) with no supplied spectrum ⇒ empty grid"
        );
        assert_eq!(kerma.eval(5.0e5), 0.0);
    }

    /// **H2.** Radiative capture (MT=102) deposits all of `E+Q` locally — no
    /// escaping neutron, so nothing is subtracted from the energy balance.
    #[test]
    fn capture_deposits_e_plus_q() {
        let q = 6.0e6; // representative (n,γ) Q-value
        let sec = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: q,
            pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        for &e in &[1.0e5, 1.0e6] {
            let h = kerma.eval(e);
            let expected = 2.0 * (e + q);
            assert!(
                (h - expected).abs() / expected < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H2.** A charged-particle-only exit, e.g. MT=107 `(n,α)`, uses the same
    /// `E+Q` local-deposition formula as capture — no escaping neutron.
    #[test]
    fn charged_particle_only_exit_deposits_e_plus_q() {
        let q = 2.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt107NAlpha,
            qi: q,
            pairs: vec![(1.0e6, 0.5)],
        };
        let kerma = Kerma::from_reconr(
            &recon(27.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let expected = 0.5 * (1.0e6 + q);
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H3, `Q=0` reduction.** A discrete "level" with `qi=0.0` (unphysical for
    /// a real level, but a clean check) must reproduce H1's pure elastic
    /// formula exactly — confirming the two-body-with-Q formula's `Q→0` limit.
    #[test]
    fn single_neutron_at_q_zero_matches_elastic_formula() {
        let awr = 12.0;
        let level = ReconrSection {
            mt: MtReaction::from_any(52),
            qi: 0.0,
            pairs: vec![(1.0e6, 3.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let expected = 3.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H3.** A discrete inelastic level with a real (negative) Q-value: the
    /// per-event heating `H(E)/σ(E) = E·2A/(A+1)² + Q/(A+1)` is *less* than the
    /// pure-elastic term because the excitation energy `|Q|` is carried away by
    /// the residual nucleus's internal state — consistent with `E+Q < E`
    /// available for redistribution.
    #[test]
    fn discrete_level_heating_is_reduced_by_negative_q() {
        let awr = 12.0;
        let q = -4.4e6; // e.g. carbon-12's first excited state
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: q,
            pairs: vec![(1.0e7, 1.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e7);
        let expected = 1.0 * (1.0e7 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
        let h_elastic_only = 1.0 * 1.0e7 * 2.0 * awr / (awr + 1.0).powi(2);
        assert!(
            h < h_elastic_only,
            "negative Q must reduce heating below the Q=0 term"
        );
    }

    /// **H3.** An `(n,n'α)`-family reaction (MT=22): single escaping neutron,
    /// the alpha stays local — same formula as a discrete level, using MT=22's
    /// own Q-value.
    #[test]
    fn nn_alpha_family_uses_single_neutron_formula() {
        let awr = 27.0;
        let q = -3.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt22NnAlpha,
            qi: q,
            pairs: vec![(8.0e6, 0.2)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(8.0e6);
        let expected = 0.2 * (8.0e6 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// H1+H2+H3 all sum additively on the shared union grid.
    #[test]
    fn all_three_phases_sum_additively() {
        let awr = 56.0;
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e6, 1.0)],
        };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -1.0e6,
            pairs: vec![(1.0e6, 0.3)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let h_elastic = 10.0 * 1.0e6 * f;
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
        let expected = h_elastic + h_capture + h_level;
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "got {h}, want {expected}"
        );
    }

    /// H1 and H2 sum additively on the same union grid — a material with both
    /// elastic and capture data gets both contributions at each energy.
    #[test]
    fn elastic_and_capture_sum_additively() {
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e6, 1.0)],
        };
        let awr = 56.0;
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let h_elastic = 10.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        assert!(
            (h - (h_elastic + h_capture)).abs() < 1.0,
            "got {h}, want {}",
            h_elastic + h_capture
        );
    }

    /// Union grid + summation across two elastic-tagged sections at different
    /// energy grids (only one physical elastic section normally exists per
    /// material, but the union-grid machinery must still be exercised).
    #[test]
    fn eval_interpolates_between_grid_points() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0), (2.0e6, 10.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(1.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h_mid = kerma.eval(1.5e6);
        // σ is flat at 10 b, so H(E) = 10*E/2 exactly (linear in E), and lin-lin
        // interpolation between the two grid points must land on that line.
        assert!((h_mid - 10.0 * 1.5e6 * 0.5).abs() < 1.0, "got {h_mid}");
    }

    /// **H4.** Fission heating matches `σ_f·[E + Q_fission − ν̄·⟨E'⟩]` exactly for
    /// a constant ν̄ and the fixed-parameter Watt χ (closed-form mean).
    #[test]
    fn fission_heating_matches_energy_balance_formula() {
        let q_fission = 200.0e6; // representative U-235 fission Q-value
        let sigma_f = 2.0; // barn, flat for simplicity
        let sec = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: q_fission,
            pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
        };
        let nu = NuBar { energy: vec![1.0e5, 1.0e6], nu_total: vec![2.4, 2.6] };
        let chi = FissionSpectrum::Watt { a: 0.988e6, b: 2.249e-6 };
        let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);

        let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        for &e in &[1.0e5, 1.0e6] {
            let h = kerma.eval(e);
            let nu_at_e = nu.at(e);
            let expected = sigma_f * (e + q_fission - nu_at_e * mean_e_prime);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H4.** Fission heating is *positive and order-190 MeV/fission* for a
    /// realistic case (Q≫ν̄·⟨E'⟩ at typical incident energies) — a basic
    /// physical sanity bound distinct from the exact-formula check above
    /// (catches a sign error that the exact check, using the same formula,
    /// structurally cannot).
    #[test]
    fn fission_heating_is_positive_and_order_200_mev() {
        let q_fission = 200.0e6;
        let sigma_f = 1.5;
        let sec = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: q_fission,
            pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
        };
        let nu = NuBar { energy: vec![1.0e5, 1.0e6], nu_total: vec![2.44, 2.44] };
        let chi = FissionSpectrum::default(); // thermal-Watt stand-in
        let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);
        let h = kerma.eval(1.0e5);
        let heating_number_ev = h / sigma_f; // H(E)/σ(E), eV per fission
        assert!(heating_number_ev > 0.0, "fission heating must be positive, got {heating_number_ev}");
        // ~200 MeV per fission minus a few MeV carried off by ~2.4 fission
        // neutrons at ~2 MeV each — should land well within [150, 200] MeV.
        assert!(
            (150.0e6..200.0e6).contains(&heating_number_ev),
            "heating number {heating_number_ev} eV should be ~190 MeV/fission"
        );
    }

    /// H1+H2+H3+H4 all sum additively on the shared union grid — the full
    /// tonight-scoped kinematic-limit KERMA.
    #[test]
    fn all_four_phases_sum_additively() {
        let awr = 235.0;
        let elastic = ReconrSection { mt: MtReaction::Mt2Elastic, qi: 0.0, pairs: vec![(1.0e6, 10.0)] };
        let capture =
            ReconrSection { mt: MtReaction::Mt102Capture, qi: 6.0e6, pairs: vec![(1.0e6, 1.0)] };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -1.0e6,
            pairs: vec![(1.0e6, 0.3)],
        };
        let fission = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: 200.0e6,
            pairs: vec![(1.0e6, 1.2)],
        };
        let nu = NuBar { energy: vec![1.0e6], nu_total: vec![2.5] };
        let chi = FissionSpectrum::Watt { a: 0.988e6, b: 2.249e-6 };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level, fission]),
            &nu,
            &chi,
            &[],
        );
        let h = kerma.eval(1.0e6);

        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let h_elastic = 10.0 * 1.0e6 * f;
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
        let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        let h_fission = 1.2 * (1.0e6 + 200.0e6 - 2.5 * mean_e_prime);
        let expected = h_elastic + h_capture + h_level + h_fission;
        assert!((h - expected).abs() / expected < 1.0e-9, "got {h}, want {expected}");
    }

    /// **H5.** An (n,2n) reaction (MT=16) subtracts the mean energy of *two*
    /// escaping neutrons from the `E + Q` energy balance: with a fixed-parameter
    /// Watt emission spectrum (closed-form mean `1.5a + ¼a²b`) the heating is
    /// `σ·(E + Q − 2·⟨E'⟩)` exactly.
    #[test]
    fn n2n_heating_subtracts_two_neutron_means() {
        let q = -6.0e6; // representative (n,2n) threshold Q
        let sigma = 0.7; // barn, flat
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: q,
            pairs: vec![(1.0e7, sigma), (1.4e7, sigma)],
        };
        // Reuse the general MF=5 machinery: a Watt-shaped emitted-neutron spectrum.
        let (a, b) = (0.5e6, 4.0e-6);
        let spectrum = FissionSpectrum::Watt { a, b };
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(MtReaction::Mt16N2n, spectrum)],
        );
        for &e in &[1.0e7, 1.4e7] {
            let h = kerma.eval(e);
            let expected = sigma * (e + q - 2.0 * mean_e_prime);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H5, multiplicity.** (n,3n) (MT=17) removes *three* neutron means, (n,4n)
    /// (MT=37) *four* — the multiplicity comes from the MT, so with the same
    /// emission spectrum the (n,3n) heating is lower than (n,2n)'s by exactly one
    /// extra `σ·⟨E'⟩`.
    #[test]
    fn n3n_and_n4n_use_higher_multiplicity() {
        let q = -12.0e6;
        let sigma = 0.4;
        let (a, b) = (0.6e6, 3.0e-6);
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let e = 2.0e7;
        for (mt, yld) in [(MtReaction::Mt17N3n, 3.0), (MtReaction::Mt37N4n, 4.0)] {
            let sec = ReconrSection { mt, qi: q, pairs: vec![(e, sigma)] };
            let kerma = Kerma::from_reconr(
                &recon(90.0, vec![sec]),
                &NuBar::default(),
                &FissionSpectrum::default(),
                &[(mt, FissionSpectrum::Watt { a, b })],
            );
            let expected = sigma * (e + q - yld * mean_e_prime);
            let h = kerma.eval(e);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "MT={:?}: H={h}, want {expected}",
                mt
            );
        }
    }

    /// **H5.** Continuum inelastic (MT=91) is the yield-1 member of the family:
    /// a single escaping neutron carrying the emission spectrum's mean, using the
    /// section's own Q — the continuum analogue of a discrete level (H3), but with
    /// the *spectrum* mean instead of the two-body kinematic mean.
    #[test]
    fn continuum_inelastic_is_single_neutron_with_spectrum_mean() {
        let q = -2.0e6;
        let sigma = 1.1;
        let (a, b) = (0.8e6, 2.0e-6);
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let e = 5.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt91NnContinuum,
            qi: q,
            pairs: vec![(e, sigma)],
        };
        let kerma = Kerma::from_reconr(
            &recon(28.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(MtReaction::Mt91NnContinuum, FissionSpectrum::Watt { a, b })],
        );
        let h = kerma.eval(e);
        let expected = sigma * (e + q - 1.0 * mean_e_prime);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H5, physical sanity.** A 14-MeV (n,2n) on a mid-mass nucleus deposits a
    /// small *positive* heating: after the ~8 MeV threshold Q and two ~1–2 MeV
    /// neutrons escape, only a few MeV of recoil + charged-particle energy is
    /// left local — distinct from the exact-formula check (catches a sign or
    /// multiplicity blunder the algebra-mirroring check structurally cannot).
    #[test]
    fn n2n_heating_is_small_and_positive_at_14_mev() {
        let q = -8.0e6;
        let sigma = 0.5;
        let e = 1.4e7;
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: q,
            pairs: vec![(e, sigma)],
        };
        // Evaporation-like emitted neutrons, ~1.5 MeV mean (θ ≈ 0.75 MeV, ⟨E'⟩=2θ).
        let spectrum = FissionSpectrum::Watt { a: 0.5e6, b: 3.0e-6 };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(MtReaction::Mt16N2n, spectrum)],
        );
        let heating_number_ev = kerma.eval(e) / sigma; // H/σ, eV per event
        assert!(
            heating_number_ev > 0.0,
            "(n,2n) heating must be positive, got {heating_number_ev}"
        );
        assert!(
            heating_number_ev < e + q,
            "escaping neutrons must reduce heating below E+Q={}",
            e + q
        );
        assert!(
            (0.0..5.0e6).contains(&heating_number_ev),
            "14-MeV (n,2n) heating {heating_number_ev} eV should be a few MeV"
        );
    }

    /// **H5, additive with H1–H4.** A section list mixing elastic, capture, a
    /// discrete level, fission, and an (n,2n) sums all five models on the shared
    /// union grid.
    #[test]
    fn h5_sums_additively_with_all_prior_phases() {
        let awr = 235.0;
        let e = 1.4e7;
        let elastic = ReconrSection { mt: MtReaction::Mt2Elastic, qi: 0.0, pairs: vec![(e, 5.0)] };
        let capture =
            ReconrSection { mt: MtReaction::Mt102Capture, qi: 6.0e6, pairs: vec![(e, 0.1)] };
        let level =
            ReconrSection { mt: MtReaction::from_any(51), qi: -1.0e6, pairs: vec![(e, 0.2)] };
        let fission =
            ReconrSection { mt: MtReaction::Mt18Fission, qi: 200.0e6, pairs: vec![(e, 1.0)] };
        let n2n = ReconrSection { mt: MtReaction::Mt16N2n, qi: -8.0e6, pairs: vec![(e, 0.6)] };
        let nu = NuBar { energy: vec![e], nu_total: vec![2.6] };
        let chi = FissionSpectrum::Watt { a: 0.988e6, b: 2.249e-6 };
        let (a, b) = (0.5e6, 4.0e-6);
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level, fission, n2n]),
            &nu,
            &chi,
            &[(MtReaction::Mt16N2n, FissionSpectrum::Watt { a, b })],
        );
        let h = kerma.eval(e);

        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let mean_chi = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        let mean_n2n = 1.5 * a + 0.25 * a * a * b;
        let expected = 5.0 * e * f
            + 0.1 * (e + 6.0e6)
            + 0.2 * (e * f + (-1.0e6) / (awr + 1.0))
            + 1.0 * (e + 200.0e6 - 2.6 * mean_chi)
            + 0.6 * (e - 8.0e6 - 2.0 * mean_n2n);
        assert!((h - expected).abs() / expected.abs() < 1.0e-9, "got {h}, want {expected}");
    }
}
