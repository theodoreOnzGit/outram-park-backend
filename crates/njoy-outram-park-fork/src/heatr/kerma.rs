//! MT=301 kinematic-limit heating (KERMA) cross section, phases H1–H5.
//!
//! Split out of `heatr/mod.rs` (see the module doc there for the physics).

use super::spectra::{
    emission_spectrum, heating_model, neutron_multiplicity, single_neutron_factor,
    EmissionSpectrum, HeatingModel,
};
use crate::endf::MtReaction;
use crate::nuclear_data::secondary::{FissionSpectrum, NuBar};
use crate::reconr::{eval_lin_lin, ReconrResult};

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
    ///   (MT=11, 16, 17, 24, 25, 30, 37, 41, 42, 91) to its emitted-neutron
    ///   spectrum ([`EmissionSpectrum`], from ENDF MF=5 *or* MF=6); the mean of
    ///   that spectrum is subtracted `ȳ` times (once per escaping neutron). A
    ///   reaction present in `recon` but absent from `emission` contributes 0 —
    ///   pass `&[]` to skip all of H5.
    pub fn from_reconr(
        recon: &ReconrResult,
        nu: &NuBar,
        chi: &FissionSpectrum,
        emission: &[(MtReaction, EmissionSpectrum)],
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

    /// Apply HEATR's **energy-balance correction**: subtract the escaping-photon
    /// energy production ([`PhotonProduction`], ENDF MT=442) from this
    /// kinematic-limit KERMA, giving the energy-balance MT=301 heating.
    ///
    /// The kinematic limit deposits *all* photon energy locally; wherever the
    /// evaluation carries photon-production data those photons actually escape,
    /// so their energy `Σ E_γ,prod(E)` must be removed (`heatr.f90`'s `gheat`).
    /// The subtraction happens on this KERMA's own energy grid (dense — the union
    /// of the modeled reactions' grids, which already includes the
    /// photon-producing reactions). The result is clamped at 0: heating is a
    /// physical (non-negative) energy deposition, and until the **capture
    /// momentum-recoil** refinement (`disgam`) lands, a capture reaction with
    /// MF=12/13 photon data could otherwise over-subtract (its photons carry
    /// nearly all of `E+Q`, leaving only the small recoil the clamp preserves as
    /// 0 rather than a spurious negative).
    ///
    /// A no-op when `photon` is empty (no photon files ⇒ the kinematic limit is
    /// already the energy-balance answer, per NJOY's documented fallback).
    pub fn with_energy_balance(
        mut self,
        photon: &crate::photon::PhotonProduction,
        recon: &ReconrResult,
    ) -> Self {
        if photon.is_empty() {
            return self;
        }
        for (i, &e) in self.energy.iter().enumerate() {
            self.h[i] = (self.h[i] - photon.eval(e, recon)).max(0.0);
        }
        self
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
