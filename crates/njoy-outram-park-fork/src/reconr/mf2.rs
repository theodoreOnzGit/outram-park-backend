//! Parse MF=2/MT=151 resonance parameters.
//!
//! ## ENDF-6 structure
//!
//! ```text
//! CONT  ZA  AWR  0   0   NIS  0        ← NIS isotopes follow
//! per isotope:
//!   CONT  ZAI  ABN  0  LFW  NER  0     ← NER energy ranges follow
//!   per range:
//!     CONT  EL  EH  LRU  LRF  NRO  NAPS
//!     LRU=0: CONT SPI AP 0 0 NLS 0; NLS CONT records
//!     LRU=1: resolved resonance parameters (SLBW/MLBW/RM/RML) — Phase 2b
//!     LRU=2: unresolved region — future work
//! ```
//!
//! **Phase 2a** implements the LRU=0 path (potential scattering only — H-2).
//! LRU=1/2 return [`NjoyError::NotPorted`].

use crate::{
    endf::{records::SectionCursor, tape::Section},
    NjoyError,
};

/// Resolved-resonance formalism code (LRF in ENDF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResonanceFormalism {
    /// LRF=1: Single-Level Breit-Wigner.
    Slbw,
    /// LRF=2: Multi-Level Breit-Wigner.
    Mlbw,
    /// LRF=3: Reich-Moore (parsed by SAMMY in upstream).
    ReichMoore,
    /// LRF=4: Adler-Adler.
    AdlerAdler,
    /// LRF=7: R-Matrix Limited.
    RMatrixLimited,
}

/// One contiguous energy range from MF=2/MT=151.
///
/// Each isotope entry may cover multiple non-overlapping energy ranges with
/// different resonance formalisms.
#[derive(Debug, Clone)]
pub struct EnergyRange {
    /// Lower energy limit [eV].
    pub el: f64,
    /// Upper energy limit [eV].
    pub eh: f64,
    /// LRU: 0 = no resonances (potential scattering only),
    /// 1 = resolved, 2 = unresolved.
    pub lru: i32,
    /// Resonance formalism — `None` when `lru != 1`.
    pub formalism: Option<ResonanceFormalism>,
    /// Nuclear spin of the target ground state (SPI).
    pub spi: f64,
    /// Potential scattering radius [10⁻¹² cm] (AP).
    pub ap: f64,
}

/// All resonance information for one material, parsed from MF=2/MT=151.
#[derive(Debug, Clone, Default)]
pub struct ResonanceInfo {
    /// Energy ranges in file order.
    pub ranges: Vec<EnergyRange>,
}

impl ResonanceInfo {
    /// Returns `true` if every range has LRU=0 (no resonance parameters).
    ///
    /// For LRU=0 materials (e.g. H-2) RECONR skips resonance reconstruction
    /// and works only with the MF=3 background cross sections.
    pub fn has_no_resonances(&self) -> bool {
        self.ranges.iter().all(|r| r.lru == 0)
    }
}

/// Parse MF=2/MT=151 and return resonance metadata.
///
/// Returns [`NjoyError::NotPorted`] for LRU=1/2 ranges until Phase 2b.
pub fn parse_resonance_info(sec: &Section) -> Result<ResonanceInfo, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);

    // CONT: ZA, AWR, 0, 0, NIS, 0
    let head = cur.read_cont()?;
    let nis = head.n1 as usize;

    let mut ranges = Vec::new();

    for _ in 0..nis {
        // CONT: ZAI, ABN, 0, LFW, NER, 0  (NER is in N1H — field 5; see reconr.f90:730)
        let iso = cur.read_cont()?;
        let ner = iso.n1 as usize;

        for _ in 0..ner {
            // CONT: EL, EH, LRU, LRF, NRO, NAPS
            let rh = cur.read_cont()?;
            let el  = rh.c1;
            let eh  = rh.c2;
            let lru = rh.l1;
            let lrf = rh.l2;

            let (spi, ap, formalism) = match lru {
                0 => {
                    // CONT: SPI, AP, 0, 0, NLS, 0
                    let c = cur.read_cont()?;
                    let nls = c.n1 as usize;
                    // Skip NLS l-state CONT records (each is a bare CONT, no LIST)
                    for _ in 0..nls {
                        cur.read_cont()?;
                    }
                    (c.c1, c.c2, None)
                }
                1 => {
                    // Resolved resonance region — Phase 2b will parse parameters.
                    // For now we cannot correctly advance the cursor without knowing
                    // the full record layout, so we return NotPorted.
                    return Err(NjoyError::NotPorted(
                        "LRU=1 resolved resonance parsing (Phase 2b)",
                    ));
                }
                2 => {
                    return Err(NjoyError::NotPorted(
                        "LRU=2 unresolved resonance region (future work)",
                    ));
                }
                _ => {
                    return Err(NjoyError::EndfParse(format!("unknown LRU={lru}")));
                }
            };

            let formalism = if lru == 1 {
                Some(match lrf {
                    1 => ResonanceFormalism::Slbw,
                    2 => ResonanceFormalism::Mlbw,
                    3 => ResonanceFormalism::ReichMoore,
                    4 => ResonanceFormalism::AdlerAdler,
                    7 => ResonanceFormalism::RMatrixLimited,
                    _ => return Err(NjoyError::EndfParse(
                        format!("unknown resonance formalism LRF={lrf}")
                    )),
                })
            } else {
                formalism
            };

            ranges.push(EnergyRange { el, eh, lru, formalism, spi, ap });
        }
    }

    Ok(ResonanceInfo { ranges })
}
