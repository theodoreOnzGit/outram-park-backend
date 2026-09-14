//! Shared energy-law types: **ACE Law 3/4**, the [`Emission`] record the ACE
//! builder consumes, and the outgoing-energy pdf/cdf normalisation both MF=5
//! and MF=6 parsers share.
//!
//! Split out of the original flat `energy.rs` (file-size cap, see the crate's
//! `CLAUDE.md`); [`super::mf5`] and [`super::mf6`] hold the ENDF-file-specific
//! parsers that build these types.

use crate::endf::tape::Tape;
use crate::acer::angular::{parse_elastic_angular, ElasticAngular};

/// eV → MeV.
pub(super) const EMEV: f64 = 1.0e6;

/// Smallest positive pdf value ACE will store; matches `acelf5`'s/`acelf6`'s
/// `rmin`/`small` guard (`1.e-30_kr`) against a rounded-to-zero density.
pub(super) const SMALL: f64 = 1.0e-30;

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
        let dists: i32 = self
            .incident
            .iter()
            .map(|d| 2 + 3 * d.e_out_mev.len() as i32)
            .sum();
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
    /// The secondary-neutron **angular** distribution for the ACE AND block, when
    /// it is given *separately* from the energy (MF=4, two-body discrete levels).
    /// `None` ⇒ isotropic in the emission frame (continuum MF=6 reactions, whose
    /// correlated angle is a future Law 61/44 upgrade, use this).
    pub angular: Option<ElasticAngular>,
}

/// Build an ACE Law 3 for a discrete two-body level reaction from its Q-value.
///
/// `qi_ev` is the reaction Q \[eV\] (negative for endothermic levels); `awr` the
/// atomic weight ratio. Ports the inline Law-3 branch of `acelod`.
pub fn law3_discrete_level(qi_ev: f64, awr: f64) -> EnergyLaw {
    let x = (awr + 1.0) / awr;
    let q_mev = qi_ev / EMEV;
    EnergyLaw::Law3 {
        ldat1_mev: x * (-q_mev),
        ldat2: 1.0 / (x * x),
    }
}

/// The base neutron multiplicity of a producing reaction, or `None` if the
/// reaction emits no neutron (capture, charged-particle absorption, …).
///
/// Fission (MT=18) is deliberately excluded: its secondaries are governed by the
/// ν̄ (NU) block, which is a separate increment.
fn neutron_yield(mt: i32) -> Option<u32> {
    match mt {
        16 => Some(2),      // (n,2n)
        17 => Some(3),      // (n,3n)
        37 => Some(4),      // (n,4n)
        51..=91 => Some(1), // (n,n') discrete levels + continuum
        22 | 23 | 24 | 25 | 28 | 29 | 30 | 32 | 33 | 34 | 35 | 36 | 41 | 42 | 44 | 45 => Some(1),
        5 => Some(1), // (n,anything) — approx one neutron
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
            // Two-body discrete level → Law 3 (CM frame ⇒ negative TYR). Its
            // angular distribution is given separately in MF=4 (→ AND block).
            let angular = tape
                .section(mat, 4, mt)
                .and_then(|s| parse_elastic_angular(s).ok())
                .filter(|a| !a.is_all_isotropic());
            out.push(Emission {
                mt,
                tyr: -(y as i32),
                law: law3_discrete_level(qi, awr),
                angular,
            });
        } else if let Some(sec) = tape.section(mat, 6, mt) {
            // Continuum / (n,xn) → Law 4 from the MF=6 neutron spectrum. The angle
            // is correlated (in MF=6); left isotropic pending a Law 61/44 upgrade.
            if let Ok(n) = super::mf6::parse_mf6_law1_neutron(sec) {
                // A negative ACE TYR marks a centre-of-mass distribution.
                // `>= 2` because ENDF-6's LCT = 3 (first two particles in the
                // CM) also puts the emitted neutron in the CM: NJOY collapses it
                // the same way at `acefc.f90:7187` (`if (lct.gt.2) lct=2`) and
                // signs on `lct.ge.2` at `acefc.f90:5869`. **MT=18 is forced to
                // the laboratory** regardless, per `acefc.f90:7277`
                // (`if (mth.eq.18) lct=1`) — fission neutrons are emitted from a
                // moving, fragmenting system and ACE stores them lab-frame.
                let cm = n.lct >= 2 && mt != 18;
                let sign = if cm { -1 } else { 1 };
                out.push(Emission {
                    mt,
                    tyr: sign * y as i32,
                    law: EnergyLaw::Law4(n.law4),
                    angular: None,
                });
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
    /// Outgoing-energy interpolation, ACE-encoded: normally `1` (histogram) or
    /// `2` (lin-lin). When the source LIST carried leading **discrete lines**
    /// (`ND>0`, MF=6 LAW=1), this instead carries the composite ACE convention
    /// `INTT = LEP + 10·ND` (`acelf6`'s `xss(nexd)=lep+10*nd`) — use
    /// [`Self::lep`] / [`Self::nd`] rather than comparing `intt` directly when a
    /// table might have discrete lines.
    pub intt: u32,
    /// Outgoing-energy grid \[MeV\], ascending. When [`Self::nd`] `> 0` the first
    /// `nd` entries are discrete-line energies.
    pub e_out_mev: Vec<f64>,
    /// Probability on `e_out_mev`, normalised so the table integrates to 1. For
    /// `i < nd` this is a plain (dimensionless) line probability; for `i >= nd`
    /// it is a density \[1/MeV\].
    pub pdf: Vec<f64>,
    /// Cumulative distribution: `cdf[0] = 0` (implicitly, via the discrete-line
    /// sum) up to `cdf[last] = 1`.
    pub cdf: Vec<f64>,
}

impl OutgoingEnergy {
    /// The outgoing-energy interpolation law (`1` histogram, `2` lin-lin),
    /// decoded from the composite ACE `INTT = LEP + 10·ND` convention.
    pub fn lep(&self) -> u32 {
        self.intt % 10
    }

    /// Number of leading discrete lines in [`Self::e_out_mev`]/[`Self::pdf`],
    /// decoded from the composite ACE `INTT = LEP + 10·ND` convention. `0` for
    /// an ordinary continuum table.
    pub fn nd(&self) -> u32 {
        self.intt / 10
    }
}

/// An ACE **Law 4** (continuous tabular) secondary energy distribution.
///
/// Built by [`super::mf5::parse_mf5_law4`]. Holds, per incident energy, the
/// outgoing-energy pdf/cdf the transport code samples.
#[derive(Debug, Clone)]
pub struct Law4 {
    /// Incident-energy interpolation regions `(NBT, INT)` (1-based NBT). Empty ⇒
    /// a single lin-lin region (the common case; ACE stores NR=0).
    pub e_in_interp: Vec<(u32, u32)>,
    /// Per-incident-energy outgoing distributions, ascending in `e_in_mev`.
    pub incident: Vec<OutgoingEnergy>,
}

/// Build one outgoing-energy distribution from a plain (no discrete lines)
/// `(E_out, f)` LIST/TAB1: convert eV→MeV, scale the pdf to /MeV, accumulate the
/// CDF (per `intt`), and renormalise to unit total. Mirrors the per-incident-
/// energy loop in `acelf5`/`acelf6`'s `ND=0` case.
pub(super) fn build_outgoing(e_in_ev: f64, intt: u32, pairs: &[(f64, f64)]) -> OutgoingEnergy {
    let (e_out_mev, pdf, cdf) = normalize_pdf_cdf(intt, pairs);
    OutgoingEnergy {
        e_in_mev: e_in_ev / EMEV,
        intt,
        e_out_mev,
        pdf,
        cdf,
    }
}

/// Core of [`build_outgoing`], shared with `acelf6`'s LAW=7 per-cosine energy
/// tables ([`super::mf6::Law7MuTable`]): eV→MeV, pdf scaled to /MeV, CDF built
/// per `intt` (`1` histogram, `2` lin-lin), then renormalised to integrate to 1.
pub(super) fn normalize_pdf_cdf(intt: u32, pairs: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = pairs.len();
    let e_out_mev: Vec<f64> = pairs.iter().map(|&(e, _)| e / EMEV).collect();
    // ENDF f is per eV; ACE wants per MeV → ×1e6.
    let mut pdf: Vec<f64> = pairs.iter().map(|&(_, f)| (f * EMEV).max(0.0)).collect();

    // CDF in E_out (still in MeV); histogram (intt=1) vs trapezoid (intt=2).
    let mut cdf = vec![0.0f64; n];
    for i in 1..n {
        let de = e_out_mev[i] - e_out_mev[i - 1];
        cdf[i] = cdf[i - 1]
            + if intt == 1 {
                pdf[i - 1] * de
            } else {
                0.5 * (pdf[i] + pdf[i - 1]) * de
            };
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
    (e_out_mev, pdf, cdf)
}

/// Build one outgoing-energy row that mixes `nd` leading **discrete lines**
/// with a continuum tail (MF=6 LAW=1, `ND>0`) — faithful to `acelf6`'s per-`ki`
/// loop (acefc.f90 ~7674–7702) that this port previously refused
/// (`NjoyError::NotPorted("MF=6 LAW=1 with discrete lines (ND>0)")`).
///
/// `lep` is the raw (unclamped) ENDF secondary-energy interpolation law from the
/// subsection's TAB2 (`1` histogram, `2` lin-lin — `acelf6` does not handle
/// `lep∈{3,4,5}` either, in which case the continuum segments silently
/// contribute no cumulative-probability increment, exactly as upstream does).
/// `pairs` is the full `NEP`-row `(E_out [eV], f [1/eV or dimensionless])` list,
/// discrete lines first.
///
/// The discrete-line probabilities are stored as-is (no `/MeV` scaling, since
/// they are plain probabilities, not a density) and their CDF contribution is a
/// straight sum; the continuum tail is scaled to `/MeV` and integrated exactly
/// as [`normalize_pdf_cdf`] does, with **zero** width contribution across the
/// discrete→continuum boundary (`ki=nd+1` in the 1-based Fortran, `i=nd` here) —
/// matching `acelf6`'s `if (nd.gt.0.and.ki.eq.nd+1) xss(...)=xss(...)` carry-over.
/// The resulting [`OutgoingEnergy::intt`] carries the composite `LEP + 10·ND`
/// ACE encoding (see [`OutgoingEnergy::lep`] / [`OutgoingEnergy::nd`]).
pub(super) fn build_outgoing_with_discrete(
    e_in_ev: f64,
    lep: i32,
    nd: usize,
    pairs: &[(f64, f64)],
) -> OutgoingEnergy {
    let n = pairs.len();
    // Same clamp already used for the plain continuum path (acelf6 itself only
    // special-cases lep∈{1,2} in the ki-loop below; higher-order interpolation
    // laws are vanishingly rare here and this keeps ND>0 consistent with ND=0).
    let intt_continuum = if lep >= 2 { 2 } else { 1 };

    let e_out_mev: Vec<f64> = pairs.iter().map(|&(e, _)| e / EMEV).collect();
    let mut pdf = vec![0.0f64; n];
    for (i, slot) in pdf.iter_mut().enumerate() {
        let f0 = pairs[i].1;
        *slot = if i < nd { f0 } else { f0 * EMEV };
        if *slot > 0.0 && *slot < SMALL {
            *slot = SMALL;
        }
    }

    let mut cdf = vec![0.0f64; n];
    for i in 0..n {
        let prev = if i == 0 { 0.0 } else { cdf[i - 1] };
        cdf[i] = if i < nd {
            // Discrete line: a plain probability weight, summed directly.
            prev + pairs[i].1
        } else if i == nd {
            // First continuum point: no width yet, so no increment.
            prev
        } else {
            let de_ev = pairs[i].0 - pairs[i - 1].0;
            prev + if intt_continuum == 1 {
                pairs[i - 1].1 * de_ev
            } else {
                0.5 * (pairs[i - 1].1 + pairs[i].1) * de_ev
            }
        };
    }
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

    OutgoingEnergy {
        e_in_mev: e_in_ev / EMEV,
        intt: (lep + 10 * nd as i32) as u32,
        e_out_mev,
        pdf,
        cdf,
    }
}

/// Reduce an ENDF interpolation table to the ACE convention: a single lin-lin
/// region (the dominant case) collapses to an empty list (ACE stores NR=0).
pub(super) fn collapse_interp(interp: &[(u32, u32)]) -> Vec<(u32, u32)> {
    if interp.len() == 1 && interp[0].1 == 2 {
        Vec::new()
    } else {
        interp.to_vec()
    }
}

/// Pick the outgoing-energy interpolation flag, clamped to {1 histogram, 2 lin-lin}
/// as ACE Law 4 requires (ENDF higher laws degrade to lin-lin, per `acelf5`).
pub(super) fn collapse_intt(interp: &[(u32, u32)]) -> u32 {
    let raw = interp.first().map(|&(_, i)| i % 10).unwrap_or(2);
    if raw > 2 {
        2
    } else {
        raw.max(1)
    }
}
