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
                let per_event = kinematic_per_event(model, e, sec.qi, two_body_factor, awr, nu, chi, multi_spec);
                h[i] += sigma * per_event;
            }
        }
        Kerma { energy, h }
    }

    /// The KERMA as HEATR's `nheat` builds it, as far as it is ported. This is
    /// the constructor to use; [`Kerma::from_reconr`] is the kinematic,
    /// `QI`-only variant.
    ///
    /// Three things differ from [`Kerma::from_reconr`] (GitHub #535):
    ///
    /// **1. Which reactions count** (`nheat`'s skip list, `heatr.f90:1073-1093`,
    /// flags from `hinit`, `:530-619`). MT=3, 4, 10, 26, 27, 101, 121-151 and
    /// 201-599 are never heated: they are sums, and the parts are. In ENDF-6
    /// files MT=103-107 are skipped when any of their discrete levels
    /// (600-849) is present, and MT=16 when 875-890 are. MT=18 is skipped when
    /// MT=19 is present with its own MF=5 spectrum; when MT=19 has none, MT=19,
    /// 20, 21 and 38 are skipped instead and MT=18 is heated. An ENDF-5 file
    /// also skips MT=719, 739, 759, 779 and 799. RECONR rebuilds MT=4 as the
    /// sum of the levels, so heating it as well counted every inelastic event
    /// twice.
    ///
    /// **2. The locally deposited Q**, `q0` (`:1176-1247`; `QI` is used only
    /// for the neutron's kinematics):
    ///
    /// | reaction | `q0` |
    /// |---|---|
    /// | has MF=6 data | `QM` (`:1245`) |
    /// | MT ≤ 15 or 51–100 (incl. 91), no MF=6 | `0`, or `QM` when `LR ≠ 0, 31` (`:1180-1181`) |
    /// | MT 16–50, no MF=6 | `QI` (`:1193`) |
    /// | MT 600–849 | `QM` (`:1226`, `:1235`) |
    /// | anything else, fission included | `QI` (`:1204`) |
    ///
    /// For a discrete level `QM = 0` while `QI = −E_level`, so `q0` keeps the
    /// excitation energy: deposited when photons stay local, and removed by
    /// [`Kerma::with_energy_balance`] when they escape.
    ///
    /// **3. Two-body mean outgoing energies** (H6b part 1). Elastic and the
    /// discrete levels without MF=6 deposit `E + q0 − yld·Ē'` with `Ē'` from
    /// `disbar` ([`super::twobody::DisbarWalker`]): the MF=4 mean cosine, and
    /// NJOY's 10 %-node interpolation. The charged-particle levels (600-849)
    /// deposit `E + q0`.
    ///
    /// The grid is the union of every reconstructed section (`nheat` walks
    /// the PENDF union grid), so `disbar`'s node chain starts where NJOY's
    /// does.
    ///
    /// Not covered: an `nqa` override (HEATR card 4), MT=458's fission-Q
    /// adjustment, and `nheat`'s own continuum and MF=6 neutron means
    /// (`conbar`, `sixbar`), for which the kinematic H5 estimate stays.
    /// For NJOY's MT=301 itself, with all of those, use
    /// [`crate::heatr::heatr_kerma`] (the whole-module translation, byte-identical
    /// to NJOY2016's HEATR); since 2026-10-05 `acer` and `interface` do.
    pub fn from_endf(
        tape: &crate::endf::tape::Tape,
        mat: i32,
        recon: &ReconrResult,
        nu: &NuBar,
        chi: &FissionSpectrum,
        emission: &[(MtReaction, EmissionSpectrum)],
    ) -> Self {
        use super::twobody::{level_yield, DisbarWalker};
        let awr = recon.material.awr;
        let numbers: Vec<i32> = recon.sections.iter().map(|s| s.mt.number()).collect();
        let any_in = |lo: i32, hi: i32| numbers.iter().any(|&m| (lo..=hi).contains(&m));
        // `hinit`'s flags (`heatr.f90:530-619`): `mt19 = 1` when the
        // evaluation has MT=19, demoted to 2 when MF=5 has no MT=19 spectrum;
        // the 600-849 and 875-890 partial flags are set only for ENDF-6.
        let iverf = tape.section(mat, 1, 451).map_or(6, |s| crate::moder::layout::iverf_from_mf1(&s.rows));
        let mt19 = match (tape.section(mat, 3, 19).is_some(), tape.section(mat, 5, 19).is_some()) {
            (false, _) => 0,
            (true, true) => 1,
            (true, false) => 2,
        };
        let partials = |lo: i32, hi: i32| iverf >= 6 && any_in(lo, hi);
        let skipped = |mt: i32| -> bool {
            matches!(mt, 3 | 4 | 10 | 26 | 27 | 101)
                || (mt == 18 && mt19 == 1)
                || (((19..=21).contains(&mt) || mt == 38) && mt19 == 2)
                || (mt == 103 && partials(600, 649))
                || (mt == 104 && partials(650, 699))
                || (mt == 105 && partials(700, 749))
                || (mt == 106 && partials(750, 799))
                || (mt == 107 && partials(800, 849))
                || (mt == 16 && partials(875, 890))
                || (iverf <= 5 && matches!(mt, 719 | 739 | 759 | 779 | 799))
                || (121..=151).contains(&mt)
                || (201..=599).contains(&mt)
        };
        let mut energy: Vec<f64> =
            recon.sections.iter().flat_map(|s| s.pairs.iter().map(|&(e, _)| e)).collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));
        // `etop` (`heatr.f90:485-508`): 2e7 eV, or the evaluation's EMAX if higher.
        let etop = tape
            .section(mat, 1, 451)
            .and_then(|s| s.rows.get(2).map(|r| r[1]))
            .filter(|&e| e > 2.0e7)
            .unwrap_or(2.0e7);

        let two_body_factor = single_neutron_factor(awr);
        let mut h = vec![0.0; energy.len()];
        for sec in &recon.sections {
            let mt = sec.mt.number();
            if skipped(mt) {
                continue;
            }
            let has_mf6 = tape.section(mat, 6, mt).is_some();
            let q0 = deposited_q(tape, mat, mt, sec.lr, sec.qi);
            if !has_mf6 && (mt == 2 || (51..=90).contains(&mt)) {
                let mut walker = DisbarWalker::new(tape, mat, mt, awr, sec.qi, etop);
                let yld = if mt == 2 { 1.0 } else { level_yield(sec.lr) };
                let thresh = nheat_threshold(&sec.pairs);
                for (i, &e) in energy.iter().enumerate() {
                    if e < thresh {
                        continue;
                    }
                    // `nheat` asks at `sigfig(e, 9, 0)` (`:1342`) and calls
                    // `disbar` whether or not sigma is zero there.
                    let ebar = walker.ebar(crate::mixr::mix::sigfig(e, 9, 0));
                    let sigma = eval_lin_lin(&sec.pairs, e);
                    if sigma != 0.0 {
                        h[i] += sigma * (e + q0 - yld * ebar);
                    }
                }
                continue;
            }
            if (600..=849).contains(&mt) {
                for (i, &e) in energy.iter().enumerate() {
                    let sigma = eval_lin_lin(&sec.pairs, e);
                    if sigma != 0.0 {
                        h[i] += sigma * (e + q0);
                    }
                }
                continue;
            }
            let model = heating_model(sec.mt);
            if model == HeatingModel::NotModeled {
                continue;
            }
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
                let per_event = kinematic_per_event(model, e, sec.qi, two_body_factor, awr, nu, chi, multi_spec);
                h[i] += sigma * (per_event + q0 - sec.qi);
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
    /// photon-producing reactions). ~~The result is clamped at 0: heating is a
    /// physical (non-negative) energy deposition, and until the **capture
    /// momentum-recoil** refinement (`disgam`) lands, a capture reaction with
    /// MF=12/13 photon data could otherwise over-subtract.~~
    ///
    /// **CHANGED 2026-10-05 (GitHub #535): no clamp at 0.** This used to
    /// return `max(0, kinematic − MT442)`. `heatr.f90` has no lower bound on
    /// MT=301 (checked: its only zero floors are `disbar`'s damage energy,
    /// `:2002`, and the MF=6 recoil distributions `h6ddx`/`h6dis`), and the
    /// clamp's stated reason, the missing capture recoil, went with H6a's
    /// `disgam`. A negative energy balance means the evaluation's photons
    /// carry more than `E + Q` minus the outgoing particles' energy: an
    /// inconsistency in the data that NJOY writes as it finds it (its
    /// `kchk` exists to expose it), and zeroing it hid exactly that signal
    /// while biasing any group or ACE average over it upwards. Pinned by
    /// `tests/heatr_mt442_vs_njoy2016.rs`,
    /// `energy_balance_subtracts_mt442_and_nothing_else`.
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
            self.h[i] -= photon.eval(e, recon);
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

/// The kinematic per-event heating \[eV\] of one reaction model at `e`, with
/// `QI = qi` as the deposited Q (the H1-H5 formulas of the module docs).
#[allow(clippy::too_many_arguments)]
fn kinematic_per_event(
    model: HeatingModel,
    e: f64,
    qi: f64,
    two_body_factor: f64,
    awr: f64,
    nu: &NuBar,
    chi: &FissionSpectrum,
    multi_spec: Option<(&EmissionSpectrum, f64)>,
) -> f64 {
    match model {
        HeatingModel::Elastic => e * two_body_factor,
        HeatingModel::Local => e + qi,
        HeatingModel::SingleNeutron => e * two_body_factor + qi / (awr + 1.0),
        HeatingModel::Fission => e + qi - nu.at(e) * chi.mean_energy(e),
        HeatingModel::MultiNeutron => {
            let (spec, yld) = multi_spec.expect("a MultiNeutron reaction is only heated with its spectrum");
            e + qi - yld * spec.mean_energy(e)
        }
        HeatingModel::NotModeled => 0.0,
    }
}

/// `nheat`'s reaction threshold (`heatr.f90:1104-1106`): `gety1`'s first
/// break of the section's MF=3, to seven figures.
fn nheat_threshold(pairs: &[(f64, f64)]) -> f64 {
    if pairs.is_empty() {
        return f64::INFINITY;
    }
    let np = pairs.len() as i32;
    let t = crate::endf::records::Tab1 {
        head: crate::endf::records::Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: np },
        interp: vec![(np as u32, 2)],
        pairs: pairs.to_vec(),
    };
    crate::mixr::mix::sigfig(crate::endf::gety1::Gety1::new(&t).get(0.0).xnext, 7, 0)
}
