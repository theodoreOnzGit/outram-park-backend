//! `GASPR` — gas-production cross sections (ENDF MT=203–207).
//!
//! Computes the total production of the five light "gas" nuclides — H1
//! (proton), H2 (deuteron), H3 (triton), He3, He4 (alpha) — from a
//! [`ReconrResult`]: post-processing information used for depletion / material
//! swelling estimates, **not** transport itself. Like KERMA/heating (MT=301),
//! this is an MF=3-only informational cross section: no secondary angle/energy
//! law is needed, because it never feeds the collision estimator — it is only
//! ever *read*, not *sampled*.
//!
//! Ported from NJOY2016 `src/gaspr.f90` (~1150 lines) for the **lumped-channel**
//! representation modern ENDF/B evaluations use: ENDF MT numbers 11, 16, 17,
//! 22–45, and 102–117 are *mutually exclusive* reaction final states (a neutron
//! that reacts via MT=107 `(n,α)` cannot simultaneously have reacted via
//! MT=103 `(n,p)`), so total gas production is simply the yield-weighted sum
//!
//! ```text
//! σ_gas(E) = Σ_mt  n_particle(mt) · σ_mt(E)
//! ```
//!
//! over every reconstructed MF=3 section — no double counting, and no need for
//! NJOY's residual-nucleus bookkeeping (`izr`/`izg` in `gaspr.f90`): the yield
//! table [`gas_yield`] encodes the same standard ENDF-6 reaction definitions
//! (ENDF-102 manual) that bookkeeping computes, but as a flat lookup instead of
//! an incident/residual mass-difference derivation, because this crate's
//! [`crate::endf::MtReaction`] enum already names each reaction's emitted
//! particles.
//!
//! **Not ported**: the legacy **MT=600–849** detailed-breakup fallback
//! `gaspr.f90` uses when an evaluation omits the lumped channels above
//! (pre-ENDF/B-VI representation; rare in ENDF/B-VII/VIII, the libraries this
//! workspace targets). A nuclide whose tape uses *only* the detailed breakup
//! would be under-counted here (each unlisted MT contributes 0) rather than
//! silently wrong — flagged as a known gap, not a claimed feature.

use crate::endf::MtReaction;
use crate::reconr::{eval_lin_lin, ReconrResult};

/// One of the five gas nuclides GASPR tracks, with its ENDF MT number
/// (MT=203–207) in the ACE/PENDF gas-production convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasSpecies {
    /// ¹H (proton) — MT=203.
    H1,
    /// ²H (deuteron) — MT=204.
    H2,
    /// ³H (triton) — MT=205.
    H3,
    /// ³He — MT=206.
    He3,
    /// ⁴He (alpha) — MT=207.
    He4,
}

impl GasSpecies {
    /// The ENDF MT number this species' production cross section would occupy
    /// on a PENDF tape (MT=203…207).
    pub fn mt(self) -> i32 {
        match self {
            GasSpecies::H1 => 203,
            GasSpecies::H2 => 204,
            GasSpecies::H3 => 205,
            GasSpecies::He3 => 206,
            GasSpecies::He4 => 207,
        }
    }
}

/// How many of each gas nuclide one event of a given MT produces. All-zero for
/// reactions that emit no charged particle (elastic, inelastic, pure `(n,xn)`,
/// fission, capture, …) — [`gas_yield`] returns this default for anything not
/// in its match table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GasYield {
    p: u8,
    d: u8,
    t: u8,
    he3: u8,
    alpha: u8,
}

impl GasYield {
    fn is_zero(self) -> bool {
        self == GasYield::default()
    }
}

/// The per-reaction gas-particle yield table: standard ENDF-6 reaction
/// definitions (ENDF-102 manual) for every MT this port recognises as a
/// gas-producing exclusive channel. Anything else (elastic, inelastic, `(n,γ)`,
/// fission, pure `(n,xn)`, …) yields zero of every species.
fn gas_yield(mt: MtReaction) -> GasYield {
    use MtReaction::*;
    match mt {
        Mt11N2nD => GasYield { d: 1, ..Default::default() },
        Mt22NnAlpha => GasYield { alpha: 1, ..Default::default() },
        Mt23Nn3Alpha => GasYield { alpha: 3, ..Default::default() },
        Mt24N2nAlpha => GasYield { alpha: 1, ..Default::default() },
        Mt25N3nAlpha => GasYield { alpha: 1, ..Default::default() },
        Mt28NnProton => GasYield { p: 1, ..Default::default() },
        Mt29Nn2Alpha => GasYield { alpha: 2, ..Default::default() },
        Mt30N2n2Alpha => GasYield { alpha: 2, ..Default::default() },
        Mt32NnD => GasYield { d: 1, ..Default::default() },
        Mt33NnT => GasYield { t: 1, ..Default::default() },
        Mt34NnHe3 => GasYield { he3: 1, ..Default::default() },
        Mt35NnD2Alpha => GasYield { d: 1, alpha: 2, ..Default::default() },
        Mt36NnT2Alpha => GasYield { t: 1, alpha: 2, ..Default::default() },
        Mt41N2nProton => GasYield { p: 1, ..Default::default() },
        Mt42N3nProton => GasYield { p: 1, ..Default::default() },
        Mt44Nn2Proton => GasYield { p: 2, ..Default::default() },
        Mt45NnProtonAlpha => GasYield { p: 1, alpha: 1, ..Default::default() },
        Mt103Np => GasYield { p: 1, ..Default::default() },
        Mt104Nd => GasYield { d: 1, ..Default::default() },
        Mt105Nt => GasYield { t: 1, ..Default::default() },
        Mt106NHe3 => GasYield { he3: 1, ..Default::default() },
        Mt107NAlpha => GasYield { alpha: 1, ..Default::default() },
        Mt108N2Alpha => GasYield { alpha: 2, ..Default::default() },
        Mt109N3Alpha => GasYield { alpha: 3, ..Default::default() },
        Mt111N2Proton => GasYield { p: 2, ..Default::default() },
        Mt112NProtonAlpha => GasYield { p: 1, alpha: 1, ..Default::default() },
        Mt113NT2Alpha => GasYield { t: 1, alpha: 2, ..Default::default() },
        Mt114ND2Alpha => GasYield { d: 1, alpha: 2, ..Default::default() },
        Mt115NProtonD => GasYield { p: 1, d: 1, ..Default::default() },
        Mt116NProtonT => GasYield { p: 1, t: 1, ..Default::default() },
        Mt117NDAlpha => GasYield { d: 1, alpha: 1, ..Default::default() },
        _ => GasYield::default(),
    }
}

/// Gas-production cross sections (ENDF MT=203–207) vs incident energy \[eV\],
/// tabulated on the union energy grid of every contributing reaction.
///
/// Built once from a [`ReconrResult`] via [`GasProduction::from_reconr`];
/// evaluated per species with [`GasProduction::eval`].
#[derive(Debug, Clone, Default)]
pub struct GasProduction {
    /// Union incident-energy grid \[eV\], ascending, deduplicated.
    pub energy: Vec<f64>,
    /// σ(MT=203) \[barn\] aligned with `energy`: total proton production.
    pub h1: Vec<f64>,
    /// σ(MT=204) \[barn\]: total deuteron production.
    pub h2: Vec<f64>,
    /// σ(MT=205) \[barn\]: total triton production.
    pub h3: Vec<f64>,
    /// σ(MT=206) \[barn\]: total ³He production.
    pub he3: Vec<f64>,
    /// σ(MT=207) \[barn\]: total alpha production.
    pub he4: Vec<f64>,
}

impl GasProduction {
    /// Compute gas-production cross sections from a reconstructed evaluation.
    ///
    /// Collects the union of every gas-producing section's energy grid, then at
    /// each grid point sums `n_particle(mt) · σ_mt(E)` (lin-lin interpolated,
    /// [`crate::reconr::eval_lin_lin`]) over every contributing MT. Sections
    /// with a zero [`gas_yield`] (the overwhelming majority — elastic,
    /// inelastic, capture, fission, pure `(n,xn)`, …) are skipped entirely.
    ///
    /// Returns an all-empty [`GasProduction`] if the evaluation has no
    /// gas-producing lumped channels (non-threshold-reaction nuclides, or one
    /// using only the unported legacy MT=600–849 breakdown — see the module
    /// docs).
    pub fn from_reconr(recon: &ReconrResult) -> Self {
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| !gas_yield(s.mt).is_zero())
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let n = energy.len();
        let mut out =
            GasProduction { energy, h1: vec![0.0; n], h2: vec![0.0; n], h3: vec![0.0; n], he3: vec![0.0; n], he4: vec![0.0; n] };

        for sec in &recon.sections {
            let y = gas_yield(sec.mt);
            if y.is_zero() {
                continue;
            }
            for (i, &e) in out.energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                if y.p > 0 {
                    out.h1[i] += y.p as f64 * sigma;
                }
                if y.d > 0 {
                    out.h2[i] += y.d as f64 * sigma;
                }
                if y.t > 0 {
                    out.h3[i] += y.t as f64 * sigma;
                }
                if y.he3 > 0 {
                    out.he3[i] += y.he3 as f64 * sigma;
                }
                if y.alpha > 0 {
                    out.he4[i] += y.alpha as f64 * sigma;
                }
            }
        }
        out
    }

    /// Evaluate one species' gas-production cross section \[barn\] at incident
    /// energy `e` \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, species: GasSpecies, e: f64) -> f64 {
        let col = match species {
            GasSpecies::H1 => &self.h1,
            GasSpecies::H2 => &self.h2,
            GasSpecies::H3 => &self.h3,
            GasSpecies::He3 => &self.he3,
            GasSpecies::He4 => &self.he4,
        };
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self.energy.iter().copied().zip(col.iter().copied()).collect();
        eval_lin_lin(&pairs, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::MaterialInfo;

    fn section(mt: MtReaction, pairs: Vec<(f64, f64)>) -> crate::reconr::ReconrSection {
        crate::reconr::ReconrSection { mt, qi: 0.0, pairs }
    }

    fn recon(sections: Vec<crate::reconr::ReconrSection>) -> ReconrResult {
        let material = MaterialInfo {
            za: 26056.0,
            awr: 55.4,
            lrp: 1,
            lfi: 0,
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: 2.0e7,
        };
        ReconrResult { material, sections }
    }

    /// A single `(n,α)` (MT=107) section reconstructs directly into MT=207
    /// (He4) with yield 1, and every other species stays zero.
    #[test]
    fn np_alpha_reaction_yields_he4_only() {
        let recon = recon(vec![section(
            MtReaction::Mt107NAlpha,
            vec![(1.0e6, 0.0), (2.0e6, 0.5), (3.0e6, 1.0)],
        )]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 2.0e6) - 0.5).abs() < 1e-9);
        assert_eq!(gas.eval(GasSpecies::H1, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::H2, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::H3, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::He3, 2.0e6), 0.0);
    }

    /// Exclusive channels are additive: MT=107 `(n,α)` (yield 1 He4) and MT=22
    /// `(n,n'α)` (yield 1 He4) both contribute to the same He4 total, summed —
    /// this is the core "disjoint final states, no double counting" claim.
    #[test]
    fn disjoint_channels_sum_additively() {
        let recon = recon(vec![
            section(MtReaction::Mt107NAlpha, vec![(1.0e6, 0.4), (2.0e6, 0.4)]),
            section(MtReaction::Mt22NnAlpha, vec![(1.0e6, 0.1), (2.0e6, 0.3)]),
        ]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 1.0e6) - 0.5).abs() < 1e-9);
        assert!((gas.eval(GasSpecies::He4, 2.0e6) - 0.7).abs() < 1e-9);
    }

    /// A multi-particle-per-event channel (MT=23, `(n,n'3α)`) is weighted by
    /// its true yield of 3, not 1.
    #[test]
    fn multi_particle_yield_is_weighted() {
        let recon = recon(vec![section(MtReaction::Mt23Nn3Alpha, vec![(1.0e6, 0.2), (2.0e6, 0.2)])]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 1.5e6) - 0.6).abs() < 1e-9);
    }

    /// A channel that produces two different species (MT=45, `(n,n'pα)`)
    /// contributes to both, each with yield 1.
    #[test]
    fn two_species_channel_credits_both() {
        let recon = recon(vec![section(MtReaction::Mt45NnProtonAlpha, vec![(1.0e6, 0.8), (2.0e6, 0.8)])]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::H1, 1.5e6) - 0.8).abs() < 1e-9);
        assert!((gas.eval(GasSpecies::He4, 1.5e6) - 0.8).abs() < 1e-9);
    }

    /// Non-gas-producing reactions (elastic, capture, fission, pure `(n,2n)`)
    /// contribute nothing — the filter in `from_reconr` must exclude them from
    /// both the union energy grid and the sums.
    #[test]
    fn non_gas_reactions_are_ignored() {
        let recon = recon(vec![
            section(MtReaction::Mt2Elastic, vec![(1.0e6, 5.0), (2.0e6, 5.0)]),
            section(MtReaction::Mt102Capture, vec![(1.0e6, 0.2), (2.0e6, 0.1)]),
            section(MtReaction::Mt16N2n, vec![(1.0e6, 0.0), (2.0e6, 0.05)]),
        ]);
        let gas = GasProduction::from_reconr(&recon);
        assert!(gas.energy.is_empty(), "no gas-producing sections ⇒ empty grid");
        for species in [GasSpecies::H1, GasSpecies::H2, GasSpecies::H3, GasSpecies::He3, GasSpecies::He4] {
            assert_eq!(gas.eval(species, 1.5e6), 0.0);
        }
    }

    /// `GasSpecies::mt` matches the standard ACE/PENDF gas-production MT range.
    #[test]
    fn species_mt_numbers_are_203_to_207() {
        assert_eq!(GasSpecies::H1.mt(), 203);
        assert_eq!(GasSpecies::H2.mt(), 204);
        assert_eq!(GasSpecies::H3.mt(), 205);
        assert_eq!(GasSpecies::He3.mt(), 206);
        assert_eq!(GasSpecies::He4.mt(), 207);
    }
}
