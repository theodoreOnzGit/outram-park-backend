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
//! - **H3, H4**: single-escaping-neutron reactions and fission — added
//!   incrementally; see the doc comments on [`Kerma::from_reconr`] and
//!   [`heating_model`] for what is (and is not yet) covered.
//! - **H5–H7** (deferred): multi-neutron-exit/continuum inelastic, the full
//!   photon energy-balance method, and damage energy (MT=444).
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
    /// Not yet modeled (H3–H5 reactions, until they land): contributes 0.
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
        _ => HeatingModel::NotModeled,
    }
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
    /// **H1–H2** are covered so far (elastic; capture + charged-particle-only
    /// exits); every other reaction contributes 0 until H3–H5 land (see the
    /// module docs). `awr` is the target's mass ratio
    /// (`ReconrResult::material::awr`).
    pub fn from_reconr(recon: &ReconrResult) -> Self {
        let awr = recon.material.awr;
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| heating_model(s.mt) != HeatingModel::NotModeled)
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let elastic_factor = 2.0 * awr / (awr + 1.0).powi(2);
        let mut h = vec![0.0; energy.len()];
        for sec in &recon.sections {
            let model = heating_model(sec.mt);
            if model == HeatingModel::NotModeled {
                continue;
            }
            for (i, &e) in energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                let per_event = match model {
                    HeatingModel::Elastic => e * elastic_factor,
                    HeatingModel::Local => e + sec.qi,
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
        let pairs: Vec<(f64, f64)> = self.energy.iter().copied().zip(self.h.iter().copied()).collect();
        eval_lin_lin(&pairs, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::{MaterialInfo, ReconrSection};

    fn material(awr: f64) -> MaterialInfo {
        MaterialInfo { za: 1001.0, awr, lrp: 0, lfi: 0, nlib: 0, elis: 0.0, nfor: 6, emax: 2.0e7 }
    }

    fn recon(awr: f64, sections: Vec<ReconrSection>) -> ReconrResult {
        ReconrResult { material: material(awr), sections }
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
        let kerma = Kerma::from_reconr(&recon(1.0, vec![sec]));
        for &e in &[1.0e5, 1.0e6, 1.0e7] {
            let h = kerma.eval(e);
            let expected = 20.0 * e * 0.5; // σ·E/2
            assert!((h - expected).abs() / expected < 1.0e-9, "H({e})={h}, want {expected}");
        }
    }

    /// A heavy target (A≫1) transfers only a small fraction of the incident
    /// energy per elastic collision — `H(E)/(:σE) → 2/A` as `A → ∞`.
    #[test]
    fn heavy_target_transfers_small_fraction() {
        let sec = ReconrSection { mt: MtReaction::Mt2Elastic, qi: 0.0, pairs: vec![(1.0e6, 5.0), (2.0e6, 5.0)] };
        let awr = 238.0;
        let kerma = Kerma::from_reconr(&recon(awr, vec![sec]));
        let h = kerma.eval(1.0e6);
        let sigma_e = 5.0 * 1.0e6;
        let frac = h / sigma_e;
        let approx_2_over_a = 2.0 / awr;
        assert!(frac > 0.0 && frac < 0.01, "heavy-target fraction {frac} should be small");
        assert!((frac - approx_2_over_a).abs() / approx_2_over_a < 0.02, "frac={frac} ≈ 2/A={approx_2_over_a}");
    }

    /// Non-elastic reactions (H2–H4 not yet ported) contribute nothing yet —
    /// confirms the dispatch excludes them rather than silently double-using
    /// the elastic formula.
    #[test]
    fn not_yet_modeled_reactions_contribute_nothing() {
        // MT=16 (n,2n): multi-neutron exit, H5, not yet ported.
        let sec = ReconrSection { mt: MtReaction::Mt16N2n, qi: -8.0e6, pairs: vec![(1.0e5, 1.0), (1.0e6, 1.0)] };
        let kerma = Kerma::from_reconr(&recon(56.0, vec![sec]));
        assert!(kerma.energy.is_empty(), "(n,2n) not modeled yet ⇒ empty grid");
        assert_eq!(kerma.eval(5.0e5), 0.0);
    }

    /// **H2.** Radiative capture (MT=102) deposits all of `E+Q` locally — no
    /// escaping neutron, so nothing is subtracted from the energy balance.
    #[test]
    fn capture_deposits_e_plus_q() {
        let q = 6.0e6; // representative (n,γ) Q-value
        let sec = ReconrSection { mt: MtReaction::Mt102Capture, qi: q, pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)] };
        let kerma = Kerma::from_reconr(&recon(56.0, vec![sec]));
        for &e in &[1.0e5, 1.0e6] {
            let h = kerma.eval(e);
            let expected = 2.0 * (e + q);
            assert!((h - expected).abs() / expected < 1.0e-9, "H({e})={h}, want {expected}");
        }
    }

    /// **H2.** A charged-particle-only exit, e.g. MT=107 `(n,α)`, uses the same
    /// `E+Q` local-deposition formula as capture — no escaping neutron.
    #[test]
    fn charged_particle_only_exit_deposits_e_plus_q() {
        let q = 2.0e6;
        let sec = ReconrSection { mt: MtReaction::Mt107NAlpha, qi: q, pairs: vec![(1.0e6, 0.5)] };
        let kerma = Kerma::from_reconr(&recon(27.0, vec![sec]));
        let h = kerma.eval(1.0e6);
        let expected = 0.5 * (1.0e6 + q);
        assert!((h - expected).abs() / expected < 1.0e-9, "H={h}, want {expected}");
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
        let kerma = Kerma::from_reconr(&recon(awr, vec![elastic, capture]));
        let h = kerma.eval(1.0e6);
        let h_elastic = 10.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        assert!((h - (h_elastic + h_capture)).abs() < 1.0, "got {h}, want {}", h_elastic + h_capture);
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
        let kerma = Kerma::from_reconr(&recon(1.0, vec![sec]));
        let h_mid = kerma.eval(1.5e6);
        // σ is flat at 10 b, so H(E) = 10*E/2 exactly (linear in E), and lin-lin
        // interpolation between the two grid points must land on that line.
        assert!((h_mid - 10.0 * 1.5e6 * 0.5).abs() < 1.0, "got {h_mid}");
    }
}
