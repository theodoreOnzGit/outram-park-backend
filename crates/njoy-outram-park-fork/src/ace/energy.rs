//! Parse ENDF secondary-neutron **energy distributions** and convert them to ACE
//! law form for the DLW block. **Phase 4d — in progress.**
//!
//! This is the energy-distribution counterpart of [`super::angular`]. It produces
//! the per-reaction laws stored in the ACE **DLW** block (located by **LDLW**),
//! which a transport code samples to pick a secondary neutron's outgoing energy.
//!
//! ## Status: energy laws wired into the DLW block
//!
//! Implemented and wired (via [`build_emissions`] + `AceTable::from_reconr_full`):
//! - [`parse_mf5_law4`] — MF=5 LF=1 tabulated secondary energy (fission
//!   χ(E→E')) → **ACE Law 4** (faithful to `acelf5`).
//! - [`parse_mf6_law1_neutron`] — MF=6 LAW=1 neutron energy pdf `f₀` → **Law 4**.
//! - [`law3_discrete_level`] — discrete two-body levels (MT51–90) → **ACE Law 3**
//!   from Q + AWR (the inline `acelod` branch).
//! - [`EnergyLaw::serialize`] lays these into the LDLW/DLW blocks with the correct
//!   law-validity header and internal locators.
//!
//! Not yet implemented:
//! - **Non-elastic angular**: producers are written isotropic. MF=6 LANG=1
//!   Legendre → **Law 61**, LANG=2 Kalbach-Mann → **Law 44** (`acelf6`).
//! - **MF=6 LAW=2** (two-body) and **LAW=6** (n-body phase space) — skipped.
//! - **Fission** (MT=18) secondaries — coupled to the ν̄ (NU) block.
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
use crate::endf::tape::Tape;
use crate::NjoyError;

/// eV → MeV.
const EMEV: f64 = 1.0e6;

/// A secondary-neutron energy distribution in ACE law form, ready for the DLW
/// block.
#[derive(Debug, Clone)]
pub enum EnergyLaw {
    /// **ACE Law 3** — discrete two-body level scattering. The outgoing energy is
    /// determined kinematically: `E'_cm = ldat2·(E − ldat1)`, with
    /// `ldat1 = (A+1)/A·|Q|` \[MeV\] and `ldat2 = (A/(A+1))²`.
    Law3 {
        /// `(A+1)/A · |Q|` \[MeV\] — the effective threshold term.
        ldat1_mev: f64,
        /// `(A/(A+1))²` — the CM outgoing-energy slope.
        ldat2: f64,
    },
    /// **ACE Law 4** — continuous tabular outgoing-energy distribution.
    Law4(Law4),
}

impl EnergyLaw {
    /// The ACE law number (`3` or `4`).
    pub fn law_number(&self) -> i32 {
        match self {
            EnergyLaw::Law3 { .. } => 3,
            EnergyLaw::Law4(_) => 4,
        }
    }

    /// Number of XSS words the law *data* occupies (excludes the 9-word
    /// law-validity header the DLW block prepends).
    pub fn data_len(&self) -> i32 {
        match self {
            EnergyLaw::Law3 { .. } => 2,
            EnergyLaw::Law4(l) => l.data_len(),
        }
    }

    /// Serialise the law data as `(value, is_integer)` words. `data_start_rel` is
    /// the DLW-relative 1-based position of the first word — needed so Law 4's
    /// internal per-incident-energy locators (`L`) point correctly.
    pub fn serialize(&self, data_start_rel: i32) -> Vec<(f64, bool)> {
        match self {
            EnergyLaw::Law3 { ldat1_mev, ldat2 } => vec![(*ldat1_mev, false), (*ldat2, false)],
            EnergyLaw::Law4(l) => l.serialize(data_start_rel),
        }
    }
}

impl Law4 {
    /// Number of `(NR, NBT/INT…)` header words for the incident-energy interp.
    fn nr_words(&self) -> i32 {
        if self.e_in_interp.is_empty() {
            1
        } else {
            1 + 2 * self.e_in_interp.len() as i32
        }
    }

    /// Number of XSS words this Law-4 data occupies.
    pub fn data_len(&self) -> i32 {
        let ne = self.incident.len() as i32;
        let dists: i32 = self.incident.iter().map(|d| 2 + 3 * d.e_out_mev.len() as i32).sum();
        self.nr_words() + 1 + ne + ne + dists
    }

    /// Serialise to `(value, is_integer)` words, with per-incident-energy `L`
    /// locators computed relative to `data_start_rel` (DLW-relative, 1-based).
    pub fn serialize(&self, data_start_rel: i32) -> Vec<(f64, bool)> {
        let mut w: Vec<(f64, bool)> = Vec::new();
        // NR + (NBT, INT) incident-energy interpolation.
        if self.e_in_interp.is_empty() {
            w.push((0.0, true));
        } else {
            w.push((self.e_in_interp.len() as f64, true));
            for &(nbt, intl) in &self.e_in_interp {
                w.push((nbt as f64, true));
                w.push((intl as f64, true));
            }
        }
        let ne = self.incident.len() as i32;
        w.push((ne as f64, true)); // NE
        for d in &self.incident {
            w.push((d.e_in_mev, false)); // E_in grid
        }
        // L(j): locator of each incident energy's distribution block.
        let dists_start = data_start_rel + self.nr_words() + 1 + ne + ne;
        let mut off = 0i32;
        for d in &self.incident {
            w.push(((dists_start + off) as f64, true));
            off += 2 + 3 * d.e_out_mev.len() as i32;
        }
        // The distribution blocks: [INTT, NP, E_out(NP), pdf(NP), cdf(NP)].
        for d in &self.incident {
            w.push((d.intt as f64, true));
            w.push((d.e_out_mev.len() as f64, true));
            for &e in &d.e_out_mev {
                w.push((e, false));
            }
            for &p in &d.pdf {
                w.push((p, false));
            }
            for &c in &d.cdf {
                w.push((c, false));
            }
        }
        w
    }
}

/// One neutron-producing reaction's secondary emission: the ACE TYR (yield with
/// frame sign) and its energy-distribution law. Assembled by [`build_emissions`]
/// and consumed by the ACE builder to fill the TYR / LDLW / DLW blocks.
#[derive(Debug, Clone)]
pub struct Emission {
    /// ENDF MT of the reaction.
    pub mt: i32,
    /// ACE TYR: neutron yield, negative in the centre-of-mass frame.
    pub tyr: i32,
    /// The secondary-neutron energy-distribution law.
    pub law: EnergyLaw,
}

/// Build an ACE Law 3 for a discrete two-body level reaction from its Q-value.
///
/// `qi_ev` is the reaction Q \[eV\] (negative for endothermic levels); `awr` the
/// atomic weight ratio. Ports the inline Law-3 branch of `acelod`.
pub fn law3_discrete_level(qi_ev: f64, awr: f64) -> EnergyLaw {
    let x = (awr + 1.0) / awr;
    let q_mev = qi_ev / EMEV;
    EnergyLaw::Law3 { ldat1_mev: x * (-q_mev), ldat2: 1.0 / (x * x) }
}

/// The base neutron multiplicity of a producing reaction, or `None` if the
/// reaction emits no neutron (capture, charged-particle absorption, …).
///
/// Fission (MT=18) is deliberately excluded: its secondaries are governed by the
/// ν̄ (NU) block, which is a separate increment.
fn neutron_yield(mt: i32) -> Option<u32> {
    match mt {
        16 => Some(2),                      // (n,2n)
        17 => Some(3),                      // (n,3n)
        37 => Some(4),                      // (n,4n)
        51..=91 => Some(1),                 // (n,n') discrete levels + continuum
        22 | 23 | 24 | 25 | 28 | 29 | 30 | 32 | 33 | 34 | 35 | 36 | 41 | 42 | 44 | 45 => Some(1),
        5 => Some(1),                       // (n,anything) — approx one neutron
        _ => None,
    }
}

/// Identify the neutron-producing reactions among `partials` and build their ACE
/// emissions (TYR + energy law).
///
/// - Discrete inelastic levels (MT=51–90) become **Law 3** from their Q-value
///   (two-body kinematics), centre-of-mass frame.
/// - Continuum / multi-neutron reactions (MT=91, 16, 17, 37, (n,n'+particle), 5)
///   become **Law 4** from their MF=6 neutron spectrum, with the TYR sign set by
///   the MF=6 frame (LCT). Reactions whose MF=6 is absent or not yet parseable
///   are skipped (they simply carry no secondary in this table).
/// - Fission (MT=18) is excluded (handled by the ν̄ block later).
///
/// `partials` are `(MT, Q [eV])` pairs, in MTR order.
pub fn build_emissions(tape: &Tape, mat: i32, awr: f64, partials: &[(i32, f64)]) -> Vec<Emission> {
    let mut out = Vec::new();
    for &(mt, qi) in partials {
        let Some(y) = neutron_yield(mt) else { continue };
        if (51..=90).contains(&mt) {
            // Two-body discrete level → Law 3 (CM frame ⇒ negative TYR).
            out.push(Emission { mt, tyr: -(y as i32), law: law3_discrete_level(qi, awr) });
        } else if let Some(sec) = tape.section(mat, 6, mt) {
            // Continuum / (n,xn) → Law 4 from the MF=6 neutron spectrum.
            if let Ok(n) = parse_mf6_law1_neutron(sec) {
                let sign = if n.lct == 2 { -1 } else { 1 };
                out.push(Emission { mt, tyr: sign * y as i32, law: EnergyLaw::Law4(n.law4) });
            }
        }
    }
    out
}

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
