//! Parse ENDF secondary-neutron **energy distributions** and convert them to ACE
//! law form for the DLW block. **Phase 4d — in progress.**
//!
//! This is the energy-distribution counterpart of [`super::angular`]. It produces
//! the per-reaction laws stored in the ACE **DLW** block (located by **LDLW**),
//! which a transport code samples to pick a secondary neutron's outgoing energy.
//!
//! ## Status: MF=5 LF=1 → ACE Law 4 (continuous tabular) done; MF=6 next
//!
//! Implemented here: [`parse_mf5_law4`] — the uncorrelated tabulated secondary
//! energy distribution (ENDF MF=5, LF=1), the canonical **fission χ(E→E')**
//! representation, converted to **ACE Law 4**. This is faithful to `acelf5` in
//! `acefc.f90`: per incident energy, store an outgoing-energy pdf and its CDF.
//!
//! Not yet implemented (the bulk of real evaluations, which use MF=6):
//! - **MF=6 LAW=1 LANG=2** (Kalbach-Mann) → **ACE Law 44** — `acelf6`.
//! - **MF=6 LAW=1 LANG=1/11-15** (Legendre / tabulated angle) → **ACE Law 61**.
//! - **MF=6 LAW=2** (two-body) and discrete-level **ACE Law 3** (from Q + AWR).
//!
//! ## How the DLW block is assembled (the wiring this feeds — `acelod`)
//!
//! Structural facts reverse-engineered from `acelod`/`change`, recorded here so
//! the wiring step is unambiguous:
//!
//! - **NXS(5) = NR** is the count of reactions that **produce secondary neutrons**
//!   (a *subset* of the NTR reactions in MTR), e.g. (n,2n) MT16, (n,n') levels
//!   MT51–90, continuum MT91, (n,anything) MT5, fission MT18. Elastic is separate.
//! - **TYR** (one entry per MTR reaction) is `0` for non-producers, else the
//!   neutron yield with sign = frame: `> 0` lab, `< 0` centre-of-mass, `|TYR| >
//!   100` ⇒ energy-dependent yield given in a yield table.
//! - **LDLW** has NR entries (1-based locators into DLW, in MTR order, filtered
//!   to producers). **LAND** has NR+1 entries (elastic + the NR producers).
//! - A **DLW** entry is `[LNW, LAW, IDAT, NR_app, (NBT,INT)·NR_app, NE, E(NE),
//!   P(NE)]` — the law-applicability header (LNW = next-law locator or 0; P = the
//!   probability this law applies vs incident energy) — followed at **IDAT** by
//!   the law-specific data below.
//!
//! **Consequence:** every neutron producer needs a valid DLW law, so a *partial*
//! DLW cannot yield a loadable table. Wiring Law 4 in therefore waits until the
//! MF=6 laws (44/61) cover the remaining producers for a given nuclide.
//!
//! ## ACE Law 4 data layout (what [`Law4`] serialises to, per `acelf5`)
//!
//! ```text
//! NR_in, (NBT,INT)·NR_in,        ! incident-energy interpolation
//! NE, E_in(1..NE),               ! incident energies [MeV]
//! L(1..NE),                      ! locator to each E_in's distribution
//! for each E_in:
//!   INTT, NP, E_out(1..NP), pdf(1..NP), cdf(1..NP)
//! ```
//! `INTT` is the outgoing interpolation (1 histogram, 2 lin-lin); the pdf is per
//! MeV and the cdf runs 0 → 1.

use crate::endf::{records::SectionCursor, tape::Section};
use crate::NjoyError;

/// eV → MeV.
const EMEV: f64 = 1.0e6;

/// One incident-energy outgoing-energy distribution (one row of an ACE Law 4).
#[derive(Debug, Clone)]
pub struct OutgoingEnergy {
    /// Incident neutron energy \[MeV\].
    pub e_in_mev: f64,
    /// Outgoing-energy interpolation: `1` = histogram, `2` = lin-lin.
    pub intt: u32,
    /// Outgoing-energy grid \[MeV\], ascending.
    pub e_out_mev: Vec<f64>,
    /// Probability density on `e_out_mev` \[1/MeV\], normalised so the integral
    /// (per `intt`) is 1.
    pub pdf: Vec<f64>,
    /// Cumulative distribution: `cdf[0] = 0`, `cdf[last] = 1`.
    pub cdf: Vec<f64>,
}

/// An ACE **Law 4** (continuous tabular) secondary energy distribution.
///
/// Built by [`parse_mf5_law4`]. Holds, per incident energy, the outgoing-energy
/// pdf/cdf the transport code samples.
#[derive(Debug, Clone)]
pub struct Law4 {
    /// Incident-energy interpolation regions `(NBT, INT)` (1-based NBT). Empty ⇒
    /// a single lin-lin region (the common case; ACE stores NR=0).
    pub e_in_interp: Vec<(u32, u32)>,
    /// Per-incident-energy outgoing distributions, ascending in `e_in_mev`.
    pub incident: Vec<OutgoingEnergy>,
}

/// Parse an MF=5 section with a single LF=1 subsection into an ACE [`Law4`].
///
/// This is the uncorrelated tabulated secondary-energy form — the fission
/// χ(E→E') of MT=18, and the prompt spectra of MT=455, etc. Faithful to the
/// LF=1 branch of `acelf5`.
///
/// # Limitations
/// Handles `NK = 1` subsection with `LF = 1`. Multi-subsection spectra and the
/// analytic laws (LF=5/7/9/11) are not yet ported.
///
/// # Errors
/// - [`NjoyError::NotPorted`] if `NK ≠ 1` or `LF ≠ 1`.
/// - [`NjoyError::EndfParse`] if the record structure is malformed.
pub fn parse_mf5_law4(section: &Section) -> Result<Law4, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, 0, NK, 0
    let nk = head.n1;
    if nk != 1 {
        return Err(NjoyError::NotPorted(
            "MF=5 with multiple subsections (NK>1) — Phase 4d follow-up",
        ));
    }

    // Subsection: TAB1 of p_k(E) (the law-applicability probability); its header
    // carries LF (= L2). For the fission χ this is p ≡ 1 across the range.
    let prob = cur.read_tab1()?;
    let lf = prob.head.l2;
    if lf != 1 {
        return Err(NjoyError::NotPorted(
            "MF=5 analytic spectra (LF=5/7/9/11) — Phase 4d follow-up",
        ));
    }

    // LF=1 body: TAB2 over incident energies, each a TAB1 of g(E→E') vs E'.
    let tab2 = cur.read_tab2()?;
    let ne = tab2.head.n2;
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        let g = cur.read_tab1()?; // head.c2 = E_in [eV]; pairs = (E_out, f)
        let e_in_ev = g.head.c2;
        // Outgoing interpolation (clamp to histogram/lin-lin as ACE allows).
        let intt = collapse_intt(&g.interp);
        incident.push(build_outgoing(e_in_ev, intt, &g.pairs));
    }

    Ok(Law4 { e_in_interp, incident })
}

/// The neutron emission of an MF=6 LAW=1 reaction, reduced to an ACE Law 4
/// energy distribution.
///
/// Built by [`parse_mf6_law1_neutron`]. Carries the multiplicity (yield) and
/// reference frame so the caller can fill the ACE TYR entry, plus the energy
/// distribution [`law4`](Self::law4). The **angular** dependence present in the
/// ENDF data (Legendre coefficients when LANG=1, Kalbach `r`/`a` when LANG=2) is
/// **not** carried here — extracting it into ACE Law 61/44 is the follow-up; this
/// captures the energy spectrum (`f₀`) as Law 4 (isotropic emission).
#[derive(Debug, Clone)]
pub struct Mf6Neutron {
    /// Reference frame of the distribution: `1` = laboratory, `2` = centre-of-mass
    /// (LCT from the MF=6 HEAD). Determines the sign of the ACE TYR entry.
    pub lct: i32,
    /// Neutron multiplicity (yield) vs incident energy `(E [eV], y)` — the
    /// subsection's TAB1. A constant `y` (e.g. 2 for (n,2n)) gives `TYR = ±y`.
    pub yield_pairs: Vec<(f64, f64)>,
    /// The outgoing-energy distribution as an ACE Law 4.
    pub law4: Law4,
}

/// Parse the **neutron** product of an MF=6 LAW=1 section into an ACE Law 4
/// energy distribution.
///
/// MF=6 LAW=1 stores, per incident energy, a LIST of `[E'_out, f₀, f₁ … f_NA]`
/// rows (`NA` angular coefficients after the energy pdf `f₀`). This extracts the
/// energy pdf `f₀` — faithful to `acelf6` for the isotropic (`NA=0`) case, and
/// the energy-only reduction of the anisotropic case. The neutron product is the
/// `ZAP=1` subsection (the first subsection for (n,xn)/(n,n') reactions).
///
/// # Limitations
/// Requires the **first** subsection to be the neutron (`ZAP=1`) with `LAW=1` and
/// no discrete lines (`ND=0`). Photon/recoil subsections, `LAW≠1`, and the
/// angular (Law 61/44) conversion are follow-ups.
///
/// # Errors
/// [`NjoyError::NotPorted`] for the unhandled cases above; [`NjoyError::EndfParse`]
/// on malformed records.
pub fn parse_mf6_law1_neutron(section: &Section) -> Result<Mf6Neutron, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, JP, LCT, NK, 0
    let lct = head.l2;

    // First subsection: TAB1 yield; its head carries ZAP (C1) and LAW (L2).
    let ymult = cur.read_tab1()?;
    let zap = ymult.head.c1.round() as i32;
    let law = ymult.head.l2;
    if zap != 1 {
        return Err(NjoyError::NotPorted(
            "MF=6 first subsection is not the neutron (ZAP≠1) — Phase 4d follow-up",
        ));
    }
    if law != 1 {
        return Err(NjoyError::NotPorted(
            "MF=6 LAW≠1 (two-body/phase-space/etc.) — Phase 4d follow-up",
        ));
    }

    // LAW=1 body: TAB2 (LANG, LEP, NE) then one LIST per incident energy.
    let tab2 = cur.read_tab2()?;
    let lep = tab2.head.l2; // secondary-energy interpolation
    let ne = tab2.head.n2;
    let intt = if lep >= 2 { 2 } else { 1 };
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        let list = cur.read_list()?;
        let e_in_ev = list.head.c2;
        let nd = list.head.l1; // number of discrete lines
        let na = list.head.l2; // number of angular coefficients per E_out
        let nep = list.head.n2; // number of secondary-energy points
        if nd != 0 {
            return Err(NjoyError::NotPorted(
                "MF=6 LAW=1 with discrete lines (ND>0) — Phase 4d follow-up",
            ));
        }
        // Each row is [E'_out, f0, f1 … f_NA]; stride = NA + 2. Extract (E', f0).
        let stride = (na + 2) as usize;
        let mut pairs: Vec<(f64, f64)> = Vec::with_capacity(nep as usize);
        for r in 0..nep as usize {
            let base = r * stride;
            let e_out = list.data[base];
            let f0 = list.data[base + 1];
            pairs.push((e_out, f0));
        }
        incident.push(build_outgoing(e_in_ev, intt, &pairs));
    }

    Ok(Mf6Neutron { lct, yield_pairs: ymult.pairs, law4: Law4 { e_in_interp, incident } })
}

/// Build one outgoing-energy distribution: convert eV→MeV, scale the pdf to /MeV,
/// accumulate the CDF (per `intt`), and renormalise to unit total. Mirrors the
/// per-incident-energy loop in `acelf5`.
fn build_outgoing(e_in_ev: f64, intt: u32, pairs: &[(f64, f64)]) -> OutgoingEnergy {
    let n = pairs.len();
    let e_out_mev: Vec<f64> = pairs.iter().map(|&(e, _)| e / EMEV).collect();
    // ENDF f is per eV; ACE wants per MeV → ×1e6.
    let mut pdf: Vec<f64> = pairs.iter().map(|&(_, f)| (f * EMEV).max(0.0)).collect();

    // CDF in E_out (still in MeV); histogram (intt=1) vs trapezoid (intt=2).
    let mut cdf = vec![0.0f64; n];
    for i in 1..n {
        let de = e_out_mev[i] - e_out_mev[i - 1];
        cdf[i] = cdf[i - 1]
            + if intt == 1 { pdf[i - 1] * de } else { 0.5 * (pdf[i] + pdf[i - 1]) * de };
    }
    // Renormalise so the distribution integrates to 1.
    if let Some(&total) = cdf.last() {
        if total > 0.0 {
            for v in &mut pdf {
                *v /= total;
            }
            for v in &mut cdf {
                *v /= total;
            }
        }
    }

    OutgoingEnergy { e_in_mev: e_in_ev / EMEV, intt, e_out_mev, pdf, cdf }
}

/// Reduce an ENDF interpolation table to the ACE convention: a single lin-lin
/// region (the dominant case) collapses to an empty list (ACE stores NR=0).
fn collapse_interp(interp: &[(u32, u32)]) -> Vec<(u32, u32)> {
    if interp.len() == 1 && interp[0].1 == 2 {
        Vec::new()
    } else {
        interp.to_vec()
    }
}

/// Pick the outgoing-energy interpolation flag, clamped to {1 histogram, 2 lin-lin}
/// as ACE Law 4 requires (ENDF higher laws degrade to lin-lin, per `acelf5`).
fn collapse_intt(interp: &[(u32, u32)]) -> u32 {
    let raw = interp.first().map(|&(_, i)| i % 10).unwrap_or(2);
    if raw > 2 {
        2
    } else {
        raw.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::Tape;
    use std::fs::File;

    fn u235_fission_chi() -> Law4 {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/n-092_U_235-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        let sec = tape.section(9228, 5, 18).expect("U-235 MF=5/MT=18");
        parse_mf5_law4(sec).unwrap()
    }

    #[test]
    fn fission_chi_parses_with_many_incident_energies() {
        let law = u235_fission_chi();
        assert!(law.incident.len() >= 10, "expected the χ E_in grid");
        // Incident energies ascending, in MeV (top ~30 MeV).
        let e: Vec<f64> = law.incident.iter().map(|d| d.e_in_mev).collect();
        assert!(e.windows(2).all(|w| w[1] >= w[0]), "E_in ascending");
        assert!(*e.last().unwrap() < 1000.0, "E_in in MeV not eV");
    }

    #[test]
    fn fission_chi_distributions_are_valid() {
        let law = u235_fission_chi();
        for d in &law.incident {
            assert!(d.e_out_mev.windows(2).all(|w| w[1] >= w[0]), "E_out ascending");
            assert!(d.pdf.iter().all(|&p| p >= 0.0), "pdf non-negative");
            assert!((d.cdf[0]).abs() < 1e-9, "cdf starts at 0");
            assert!((d.cdf.last().unwrap() - 1.0).abs() < 1e-6, "cdf ends at 1");
            assert!(d.cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9), "cdf monotone");
        }
    }

    fn u235_mf6(mt: i32) -> Mf6Neutron {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/n-092_U_235-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        let sec = tape.section(9228, 6, mt).expect("U-235 MF=6");
        parse_mf6_law1_neutron(sec).unwrap()
    }

    #[test]
    fn mf6_n2n_yield_and_frame() {
        // MT=16 (n,2n): neutron multiplicity 2, centre-of-mass frame (LCT=2).
        let n2n = u235_mf6(16);
        assert_eq!(n2n.lct, 2, "LCT=2 (CM)");
        assert!(n2n.yield_pairs.iter().all(|&(_, y)| (y - 2.0).abs() < 1e-6),
            "(n,2n) yield is 2");
        assert!(n2n.law4.incident.len() > 5, "MT16 has an E_in grid");
    }

    #[test]
    fn mf6_energy_distributions_are_valid() {
        // Both MT16 (n,2n) and MT91 (continuum inelastic) energy spectra.
        for mt in [16, 91] {
            let d = u235_mf6(mt);
            assert!(d.yield_pairs.iter().all(|&(_, y)| y >= 1.0), "yield ≥ 1");
            for dist in &d.law4.incident {
                assert!(dist.e_out_mev.windows(2).all(|w| w[1] >= w[0]), "E_out ascending");
                assert!(dist.pdf.iter().all(|&p| p >= 0.0), "pdf ≥ 0");
                assert!((dist.cdf[0]).abs() < 1e-9, "cdf starts at 0");
                assert!((dist.cdf.last().unwrap() - 1.0).abs() < 1e-6, "cdf ends at 1");
                assert!(dist.cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9), "cdf monotone");
            }
        }
    }

    #[test]
    fn fission_spectrum_peaks_near_1_mev() {
        // The U-235 prompt fission spectrum peaks around 0.7–1 MeV. Take a
        // thermal incident energy and find the mode of the outgoing pdf.
        let law = u235_fission_chi();
        let d = &law.incident[0]; // lowest incident energy
        let (mut mode_e, mut mode_p) = (0.0, 0.0);
        for (&e, &p) in d.e_out_mev.iter().zip(&d.pdf) {
            if p > mode_p {
                mode_p = p;
                mode_e = e;
            }
        }
        assert!(
            (0.2..=2.0).contains(&mode_e),
            "fission χ should peak near ~1 MeV, got {mode_e} MeV"
        );
    }
}
