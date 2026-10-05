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

    /// The kinematic-limit KERMA with each reaction's **locally deposited Q**
    /// chosen as HEATR's `nheat` chooses it, rather than always `QI`. This is
    /// the constructor to use; [`Kerma::from_reconr`] is the `QI`-only variant.
    ///
    /// `nheat` deposits `E + q0 − Ē_n` per reaction (`heatr.f90:1176-1247`),
    /// with `QI` used only for the neutron's kinematics and `q0`:
    ///
    /// | reaction | `q0` |
    /// |---|---|
    /// | has MF=6 data | `QM` (`:1245`) |
    /// | MT ≤ 15 or 51–100 (incl. 91), no MF=6 | `0`, or `QM` when `LR ≠ 0, 31` (`:1180-1181`) |
    /// | MT 16–50, no MF=6 | `QI` (`:1193`) |
    /// | MT 600–849 | `QM` (`:1226`, `:1235`) |
    /// | anything else, fission included | `QI` (`:1204`) |
    ///
    /// For a discrete inelastic level `QM = 0` while `QI = −E_level`, so `q0`
    /// keeps the level's de-excitation energy: with photons deposited locally
    /// (NJOY's `local = 1`) that energy is heating, and the energy-balance
    /// method ([`Kerma::with_energy_balance`]) then removes exactly the part
    /// the evaluation's photons carry away. `QI` alone leaves it out, which
    /// makes the kinematic limit too low above the first inelastic threshold
    /// and makes the energy balance subtract photon energy that was never
    /// deposited.
    ///
    /// Every heating model here is linear in the deposited Q with unit
    /// coefficient (H3's `Q/(A+1)` recoil term is kinematics and keeps `QI`;
    /// `E − Ē_n` with isotropic two-body `Ē_n` gives `E·2A/(A+1)² + QI/(A+1) +
    /// (q0 − QI)`), so this is [`Kerma::from_reconr`] plus
    /// `Σ_r σ_r(E)·(q0_r − QI_r)` over the modeled reactions.
    ///
    /// Not covered: an `nqa` override (HEATR card 4's user Q values), and
    /// MT=458's fission-Q adjustment.
    pub fn from_endf(
        tape: &crate::endf::tape::Tape,
        mat: i32,
        recon: &ReconrResult,
        nu: &NuBar,
        chi: &FissionSpectrum,
        emission: &[(MtReaction, EmissionSpectrum)],
    ) -> Self {
        let mut k = Kerma::from_reconr(recon, nu, chi, emission);
        for sec in &recon.sections {
            let model = heating_model(sec.mt);
            if model == HeatingModel::NotModeled
                || (model == HeatingModel::MultiNeutron && emission_spectrum(emission, sec.mt).is_none())
            {
                continue;
            }
            let mt = sec.mt.number();
            let dq = deposited_q(tape, mat, mt, sec.lr, sec.qi) - sec.qi;
            if dq == 0.0 {
                continue;
            }
            for (i, &e) in k.energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma != 0.0 {
                    k.h[i] += sigma * dq;
                }
            }
        }
        k
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

/// HEATR's deposited Q, `q0`, for reaction `mt` (see [`Kerma::from_endf`]).
fn deposited_q(tape: &crate::endf::tape::Tape, mat: i32, mt: i32, lr: i32, qi: f64) -> f64 {
    let qm = || -> f64 {
        tape.section(mat, 3, mt)
            .and_then(|s| {
                let mut c = crate::endf::records::SectionCursor::new(&s.rows);
                c.read_cont().ok()?;
                Some(c.read_tab1().ok()?.head.c1)
            })
            .unwrap_or(qi)
    };
    let fission = (18..=21).contains(&mt) || mt == 38;
    if fission {
        return qi;
    }
    if tape.section(mat, 6, mt).is_some() {
        return qm();
    }
    if mt <= 15 || (51..=100).contains(&mt) {
        return if lr != 0 && lr != 31 { qm() } else { 0.0 };
    }
    if (16..=50).contains(&mt) {
        return qi;
    }
    if (600..=849).contains(&mt) {
        return qm();
    }
    qi
}
