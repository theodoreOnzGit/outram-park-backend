//! Photon (γ) production — the escaping-photon energy HEATR's **energy-balance
//! method** subtracts from the kinematic-limit KERMA.
//!
//! The kinematic-limit KERMA ([`crate::heatr`], H1–H5) deposits *all* photon
//! energy locally. That over-counts heating wherever the evaluation carries
//! photon-production data: those photons actually escape the local region.
//! `heatr.f90`'s energy-balance method corrects this by subtracting the photon
//! energy each reaction releases,
//!
//! ```text
//!   H_balance(E) = H_kinematic(E) − Σ_reactions E_γ,prod(E),
//! ```
//!
//! where the **photon energy production** `E_γ,prod(E)` \[eV·barn\] of a reaction
//! is its production rate times the mean photon energy, summed over the reaction's
//! photon lines/continua. This module parses that photon data from ENDF and
//! evaluates `Σ E_γ,prod(E)` — which is also ENDF **MT=442** (total photon
//! eV-barns). See [`PhotonProduction`].
//!
//! ## What is ported
//!
//! - **MF=13** — photon-production *cross sections* `σ_γ,k(E)` \[barn\]: the
//!   production rate is `σ_γ,k` directly, so `E_γ,prod = Σ_k σ_γ,k(E)·Ē_γ,k`.
//! - **MF=12, LO=1** — photon *multiplicities* (yields) `y_k(E)`: the production
//!   rate is `σ_MF3,mt(E)·y_k(E)` (the reaction cross section times the yield),
//!   so `E_γ,prod = σ_mt(E)·Σ_k y_k(E)·Ē_γ,k`.
//! - **Discrete** photons (LF=2): `Ē_γ,k = EG_k`, or for a *primary* photon
//!   (LP=2) the incident-energy-dependent `EG_k + E·A/(A+1)` (`heatr.f90`'s
//!   `egk = egkr + e·afact`).
//! - **Continuum** photons (LF=1): `Ē_γ,k(E)` is the first moment of the ENDF
//!   **MF=15** normalized photon spectrum `g(E→E_γ)` at incident energy `E`
//!   (`heatr.f90`'s `gambar`/`tabbar`), interpolated over the incident grid.
//!
//! ## What is not (yet) ported
//!
//! - **MF=12, LO=2** — transition-probability arrays (discrete-level γ cascades).
//!   `heatr.f90` converts these to LO=1 yields in `hconvr` before the balance;
//!   that cascade conversion is a follow-up. A reaction whose only photon data is
//!   LO=2 contributes 0 here (its photon energy stays deposited locally, i.e. the
//!   kinematic limit — an over-count, not a silent error).
//! - **MF=6** photon (ZAP=0) subsections, and the **capture** momentum-recoil
//!   refinement (`disgam`) — both follow-ups.

use crate::endf::interp::eval_tab1;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::reconr::{eval_lin_lin, ReconrResult};

/// eV per MeV (ENDF energies are in eV; the ACE heating number is MeV).
const EMEV: f64 = 1.0e6;

/// The mean-photon-energy model of one photon subsection.
#[derive(Debug, Clone)]
enum PhotonEnergy {
    /// Discrete line: mean energy is `eg` \[eV\], plus `E·A/(A+1)` when `primary`
    /// (LP=2, an incident-energy-dependent primary-capture photon).
    Discrete { eg: f64, primary: bool },
    /// Continuum (LF=1): the mean photon energy `Ē_γ(E)` \[eV\] as a tabulated
    /// `(E_in, mean)` curve, the first moment of the MF=15 spectrum at each
    /// incident energy. Interpolated lin-lin, clamped at the ends.
    Continuum(Vec<(f64, f64)>),
}

impl PhotonEnergy {
    /// Mean photon energy `Ē_γ` \[eV\] for an incident energy `e` \[eV\] and mass
    /// ratio `awr`.
    fn mean(&self, e: f64, awr: f64) -> f64 {
        match self {
            PhotonEnergy::Discrete { eg, primary } => {
                if *primary {
                    eg + e * awr / (awr + 1.0)
                } else {
                    *eg
                }
            }
            PhotonEnergy::Continuum(curve) => eval_lin_lin(curve, e),
        }
    }
}

/// One photon subsection of a reaction: its per-event production `tab`
/// (a yield `y_k(E)` for MF=12, or a cross section `σ_γ,k(E)` \[barn\] for MF=13)
/// and its mean photon energy model.
#[derive(Debug, Clone)]
struct PhotonSub {
    /// Yield (MF=12) or production cross section (MF=13) vs incident energy.
    tab: Tab1,
    /// Mean-photon-energy model (discrete line or MF=15 continuum).
    energy: PhotonEnergy,
}

impl PhotonSub {
    fn production(&self, e: f64) -> f64 {
        eval_tab1(e, &self.tab.interp, &self.tab.pairs).unwrap_or(0.0)
    }
}

/// The photon production of one reaction MT.
#[derive(Debug, Clone)]
struct ReactionPhotons {
    /// The reaction's MT (its MF=3 cross section is the production rate for
    /// MF=12).
    mt: i32,
    /// `true` for MF=12 (yields — multiply by the MF=3 reaction cross section);
    /// `false` for MF=13 (the subsections already *are* cross sections).
    is_yield: bool,
    /// The photon lines/continua.
    subs: Vec<PhotonSub>,
}

/// Total photon energy production of a material — ENDF **MT=442** \[eV·barn\] —
/// and the term HEATR's energy-balance method subtracts from the kinematic-limit
/// KERMA.
///
/// Build with [`PhotonProduction::from_endf`]; evaluate the total
/// `Σ_reactions E_γ,prod(E)` \[eV·barn\] at an incident energy with
/// [`PhotonProduction::eval`]. See the module docs for the covered ENDF forms.
#[derive(Debug, Clone, Default)]
pub struct PhotonProduction {
    awr: f64,
    reactions: Vec<ReactionPhotons>,
}

impl PhotonProduction {
    /// Parse the photon-production data (MF=12 LO=1 and MF=13, with MF=15 for the
    /// continuum spectra) for material `mat`, using `recon` for the MF=3 reaction
    /// cross sections the MF=12 yields multiply.
    ///
    /// Reactions whose only photon data is MF=12 **LO=2** (transition
    /// probabilities) or an unported form are skipped (contribute 0). Never
    /// fails: malformed or unported sections are silently omitted, so the caller
    /// gets whatever photon energy could be read (the rest stays deposited
    /// locally, the kinematic limit).
    pub fn from_endf(tape: &Tape, mat: i32, recon: &ReconrResult) -> Self {
        let awr = recon.material.awr;
        let mut reactions = Vec::new();
        for sec in tape.sections() {
            let (mf, mt) = (sec.key.mf, sec.key.mt);
            if sec.key.mat != mat || (mf != 12 && mf != 13) {
                continue;
            }
            if mt == 460 {
                continue; // delayed fission photons: not deposited via this path
            }
            if let Some(rxn) = parse_reaction(tape, mat, mf, mt, awr) {
                if !rxn.subs.is_empty() {
                    reactions.push(rxn);
                }
            }
        }
        PhotonProduction { awr, reactions }
    }

    /// Total photon energy production `Σ_reactions E_γ,prod(E)` \[eV·barn\] at
    /// incident energy `e` \[eV\] — ENDF MT=442. Returns 0 for an evaluation
    /// with no (ported) photon-production data.
    ///
    /// `recon` supplies the MF=3 reaction cross sections that the MF=12 yields
    /// multiply (pass the same [`ReconrResult`] used to build this).
    pub fn eval(&self, e: f64, recon: &ReconrResult) -> f64 {
        let mut total = 0.0;
        for rxn in &self.reactions {
            let e_gamma: f64 = rxn
                .subs
                .iter()
                .map(|s| s.production(e) * s.energy.mean(e, self.awr))
                .sum();
            if e_gamma == 0.0 {
                continue;
            }
            if rxn.is_yield {
                // MF=12: multiply Σ y·Ē_γ by the reaction's MF=3 cross section.
                let sigma = recon
                    .sections
                    .iter()
                    .find(|s| i32::from(s.mt) == rxn.mt)
                    .map_or(0.0, |s| eval_lin_lin(&s.pairs, e));
                total += sigma * e_gamma;
            } else {
                total += e_gamma; // MF=13: already σ_γ·Ē_γ.
            }
        }
        total
    }

    /// Whether any photon-production data was found (i.e. the energy-balance
    /// correction is non-trivial).
    pub fn is_empty(&self) -> bool {
        self.reactions.is_empty()
    }
}

/// Parse one MF=12/MF=13 section into a [`ReactionPhotons`]. Returns `None` for
/// LO=2 (transition probabilities — the `hconvr` cascade is a follow-up) or a
/// malformed section.
fn parse_reaction(tape: &Tape, mat: i32, mf: i32, mt: i32, awr: f64) -> Option<ReactionPhotons> {
    let sec = tape.section(mat, mf, mt)?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    let nk = head.n1;
    if mf == 12 {
        let lo = head.l1;
        if lo != 1 {
            return None; // LO=2 transition probabilities — not ported yet.
        }
    }
    // For NK>1 a total (yield or xs) TAB1 leads the subsections; skip it.
    if nk > 1 {
        cur.read_tab1().ok()?;
    }
    let mut subs = Vec::with_capacity(nk.max(1) as usize);
    for _ in 0..nk.max(1) {
        let tab = cur.read_tab1().ok()?;
        // Subsection head: C1=EG, C2=ES, L1=LP, L2=LF.
        let eg = tab.head.c1;
        let lp = tab.head.l1;
        let lf = tab.head.l2;
        let energy = if lf == 2 || eg > 0.0 {
            PhotonEnergy::Discrete { eg, primary: lp == 2 }
        } else {
            // LF=1 continuum: mean from MF=15.
            match mf15_mean_curve(tape, mat, mt, awr) {
                Some(curve) => PhotonEnergy::Continuum(curve),
                None => continue, // no MF=15 ⇒ cannot value this continuum
            }
        };
        subs.push(PhotonSub { tab, energy });
    }
    Some(ReactionPhotons {
        mt,
        is_yield: mf == 12,
        subs,
    })
}

/// Build the `(E_in, Ē_γ)` mean-photon-energy curve from ENDF **MF=15** for
/// reaction `mt` — `heatr.f90`'s `gambar`. MF=15 stores, per incident energy, a
/// normalized outgoing-photon spectrum `g(E→E_γ)`; the mean is its first moment
/// `∫ E_γ·g dE_γ`. Multiple partial distributions (NC>1) are weighted by their
/// probability `p_c(E)`. Returns `None` if the material has no MF=15/`mt`.
fn mf15_mean_curve(tape: &Tape, mat: i32, mt: i32, _awr: f64) -> Option<Vec<(f64, f64)>> {
    let sec = tape.section(mat, 15, mt)?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    let nc = head.n1;

    // Accumulate p_c(E_in)-weighted means on a shared incident grid. For the
    // common NC=1 case this is just the single distribution's mean curve.
    let mut acc: Vec<(f64, f64, f64)> = Vec::new(); // (E_in, Σ p·mean, Σ p)
    for _ in 0..nc.max(1) {
        let p = cur.read_tab1().ok()?; // p_c(E): head L2=LF (expect 1)
        if p.head.l2 != 1 {
            return None; // only LF=1 (arbitrary tabulated) is ported
        }
        let tab2 = cur.read_tab2().ok()?;
        let ne = tab2.head.n2;
        for _ in 0..ne {
            let g = cur.read_tab1().ok()?; // head C2 = E_in; pairs = (E_γ, g)
            let e_in = g.head.c2;
            let mean = first_moment(&g);
            let pc = eval_tab1(e_in, &p.interp, &p.pairs).unwrap_or(1.0);
            match acc.iter_mut().find(|(e, _, _)| (*e - e_in).abs() < 1.0e-6 * e_in.max(1.0)) {
                Some(entry) => {
                    entry.1 += pc * mean;
                    entry.2 += pc;
                }
                None => acc.push((e_in, pc * mean, pc)),
            }
        }
    }
    if acc.is_empty() {
        return None;
    }
    acc.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Some(
        acc.into_iter()
            .map(|(e, wm, w)| (e, if w > 0.0 { wm / w } else { 0.0 }))
            .collect(),
    )
}

/// First moment `∫ E_γ·g(E_γ) dE_γ` \[eV\] of a normalized MF=15 outgoing-photon
/// spectrum (a TAB1 of `(E_γ, g)`), trapezoidal for lin-lin (INT=2) and
/// histogram for INT=1. The spectrum integrates to 1, so this is the mean.
fn first_moment(g: &Tab1) -> f64 {
    let p = &g.pairs;
    if p.len() < 2 {
        return p.first().map_or(0.0, |&(e, _)| e);
    }
    // Interpolation law: use the first region's INT (MF=15 spectra are single
    // region in practice). INT=1 histogram, else lin-lin.
    let histogram = g.interp.first().map(|&(_, i)| i == 1).unwrap_or(false);
    let mut m = 0.0;
    for w in p.windows(2) {
        let (x0, f0) = w[0];
        let (x1, f1) = w[1];
        if histogram {
            m += f0 * (x1 * x1 - x0 * x0) / 2.0;
        } else {
            m += (x1 - x0) * (x1 * f1 + x0 * f0) / 2.0;
        }
    }
    m
}

/// Convenience: the total photon energy production as an `[eV·barn]` heating
/// number in **MeV** at incident energy `e`, i.e. `E_γ,prod(E)/E_total` — not
/// used internally (KERMA subtracts in eV·barn) but handy for diagnostics.
pub fn photon_heating_number_mev(prod_ev_barn: f64, total_barn: f64) -> f64 {
    if total_barn <= 0.0 {
        0.0
    } else {
        prod_ev_barn / EMEV / total_barn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::{MaterialInfo, ReconrResult, ReconrSection};
    use crate::endf::MtReaction;

    fn material(awr: f64) -> MaterialInfo {
        MaterialInfo { za: 92235.0, awr, lrp: 0, lfi: 0, nlib: 0, elis: 0.0, nfor: 6, emax: 2.0e7 }
    }

    /// The first moment of a *normalized* flat spectrum on `[0, 2 MeV]`
    /// (`g = 1/2e6` so `∫g dE_γ = 1`) is the midpoint, 1 MeV — the
    /// mean-photon-energy kernel used for MF=15 continua.
    #[test]
    fn first_moment_of_flat_spectrum_is_midpoint() {
        use crate::endf::records::Cont;
        let g = Tab1 {
            head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 1, n1: 1, n2: 2 },
            interp: vec![(2, 2)],
            pairs: vec![(0.0, 5.0e-7), (2.0e6, 5.0e-7)], // normalized: 5e-7·2e6 = 1
        };
        assert!((first_moment(&g) - 1.0e6).abs() < 1.0, "got {}", first_moment(&g));
    }

    /// A discrete photon deposits its line energy; a primary (LP=2) photon adds
    /// the incident-energy-dependent `E·A/(A+1)` term.
    #[test]
    fn discrete_and_primary_photon_energy() {
        let awr = 55.0;
        let line = PhotonEnergy::Discrete { eg: 2.0e6, primary: false };
        assert_eq!(line.mean(1.0e6, awr), 2.0e6);
        let primary = PhotonEnergy::Discrete { eg: 2.0e6, primary: true };
        let expected = 2.0e6 + 1.0e6 * awr / (awr + 1.0);
        assert!((primary.mean(1.0e6, awr) - expected).abs() < 1.0);
    }

    /// An empty material (no MF=12/13) yields zero photon energy production.
    #[test]
    fn empty_without_photon_files() {
        let recon = ReconrResult {
            material: material(233.0),
            sections: vec![ReconrSection {
                mt: MtReaction::Mt2Elastic,
                qi: 0.0,
                pairs: vec![(1.0e6, 10.0)],
            }],
        };
        let pp = PhotonProduction::default();
        assert!(pp.is_empty());
        assert_eq!(pp.eval(1.0e6, &recon), 0.0);
    }
}
