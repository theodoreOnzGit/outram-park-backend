//! Parse MF=2/MT=151 resonance parameters.
//!
//! ## ENDF-6 record structure
//!
//! ```text
//! CONT  ZA  AWR  0   0  NIS  0          ← NIS isotopes
//! per isotope:
//!   CONT  ZAI  ABN  0  LFW  NER  0      ← NER energy ranges
//!   per range:
//!     CONT  EL  EH  LRU  LRF  NRO  NAPS
//!     LRU=0: CONT SPI AP 0 0 NLS 0; NLS CONT l-state records
//!     LRU=1, LRF=1/2 (SLBW/MLBW):
//!       CONT SPI AP 0 0 NLS 0;
//!       NLS LIST records (AWRI QX L LRX 6*NRS NRS; then NRS×6: ER AJ GT GN GG GF)
//!     LRU=1, LRF=3 (Reich-Moore):
//!       CONT SPI AP 0 LAD NLS 0;
//!       NLS LIST records (AWRI APL L 0 6*NRS NRS; then NRS×6: ER AJ GN GG GFA GFB)
//!     LRU=2: partial parse (SPI/AP header only)
//! ```

use crate::{
    endf::{records::SectionCursor, tape::Section},
    NjoyError,
};

// ── Resonance formalism ────────────────────────────────────────────────────────

/// Resolved-resonance formalism code (LRF in ENDF-6, MF=2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResonanceFormalism {
    /// LRF=1: Single-Level Breit-Wigner.
    Slbw,
    /// LRF=2: Multi-Level Breit-Wigner.
    Mlbw,
    /// LRF=3: Reich-Moore.
    ReichMoore,
    /// LRF=4: Adler-Adler (not yet ported).
    AdlerAdler,
    /// LRF=7: R-Matrix Limited (not yet ported).
    RMatrixLimited,
}

// ── SLBW/MLBW resonance types ─────────────────────────────────────────────────

/// One resonance in a SLBW or MLBW evaluation.
///
/// Fields correspond to the 6 values per resonance in the ENDF-6 LIST record
/// for LRF=1/2: `ER AJ GT GN GG GF`.
#[derive(Debug, Clone)]
pub struct SlbwResonance {
    /// Resonance energy \[eV\]. May be negative (sub-threshold).
    pub er: f64,
    /// Spin assignment J.
    pub aj: f64,
    /// Total width Γ_tot \[eV\] at E_r. Used for building the energy grid.
    pub gt: f64,
    /// Neutron width Γ_n \[eV\] at E_r.
    pub gn: f64,
    /// Radiative capture width Γ_γ \[eV\].
    pub gg: f64,
    /// Fission width Γ_f \[eV\]. Zero for non-fissile nuclides.
    pub gf: f64,
}

impl SlbwResonance {
    /// Convert to the `(ER, AJ, GT, GN, GG, GF)` tuple expected by [`eval_slbw_lstate`].
    ///
    /// [`eval_slbw_lstate`]: super::slbw::eval_slbw_lstate
    pub fn as_tuple(&self) -> (f64, f64, f64, f64, f64, f64) {
        (self.er, self.aj, self.gt, self.gn, self.gg, self.gf)
    }
}

/// One orbital angular momentum l-state in a SLBW/MLBW resolved resonance region.
#[derive(Debug, Clone)]
pub struct LState {
    /// Atomic weight ratio of the target.
    pub awri: f64,
    /// Orbital angular momentum quantum number l.
    pub l: u32,
    /// All resonances in this l-state, in file order.
    pub resonances: Vec<SlbwResonance>,
}

// ── Reich-Moore (LRF=3) resonance types ───────────────────────────────────────

/// One resonance in a Reich-Moore (LRF=3) evaluation.
///
/// Fields correspond to the 6 values per resonance in the ENDF-6 LIST record
/// for LRF=3: `ER AJ GN GG GFA GFB`. Note the field order differs from SLBW
/// (no GT; two fission widths instead of one; GN before GG).
#[derive(Debug, Clone)]
pub struct RmResonance {
    /// Resonance energy \[eV\]. May be negative.
    pub er: f64,
    /// Spin J (signed): positive → channel-spin group A, negative → group B.
    pub aj: f64,
    /// Neutron partial width Γ_n \[eV\] at E_r.
    pub gn: f64,
    /// Radiative capture width Γ_γ \[eV\].
    pub gg: f64,
    /// First fission partial width Γ_fA \[eV\], signed.
    pub gfa: f64,
    /// Second fission partial width Γ_fB \[eV\], signed. Zero for most nuclides.
    pub gfb: f64,
}

/// One orbital angular momentum l-state in a Reich-Moore resolved resonance region.
#[derive(Debug, Clone)]
pub struct RmLState {
    /// Atomic weight ratio of the target.
    pub awri: f64,
    /// Per-l scattering radius \[10⁻¹² cm\]. `0.0` → use the range-level `AP`.
    pub apl: f64,
    /// Orbital angular momentum quantum number l.
    pub l: u32,
    /// All resonances in this l-state, in file order.
    pub resonances: Vec<RmResonance>,
}

// ── Energy range ───────────────────────────────────────────────────────────────

/// One energy range entry from MF=2/MT=151.
///
/// A single isotope can have multiple non-overlapping energy ranges, each with a
/// different resonance formalism. For example, Ar-37 has LRU=1/LRF=2 up to 4268 eV,
/// then LRU=2 (unresolved). U-235 has LRU=1/LRF=3 (Reich-Moore).
#[derive(Debug, Clone)]
pub struct EnergyRange {
    /// Lower energy boundary \[eV\].
    pub el: f64,
    /// Upper energy boundary \[eV\].
    pub eh: f64,
    /// LRU flag: 0 = no resonances, 1 = resolved, 2 = unresolved.
    pub lru: i32,
    /// Resonance formalism — `None` when `lru != 1`.
    pub formalism: Option<ResonanceFormalism>,
    /// Nuclear spin I of the target ground state.
    pub spi: f64,
    /// Potential scattering radius \[10⁻¹² cm\].
    pub ap: f64,
    /// NAPS flag: 0 = use formula for channel radius; 1 = use `ap` as channel radius.
    pub naps: i32,
    /// l-states with SLBW/MLBW parameters (LRU=1, LRF=1 or 2). Empty for LRF=3.
    pub l_states: Vec<LState>,
    /// l-states with Reich-Moore parameters (LRU=1, LRF=3). Empty for LRF≠3.
    pub rm_l_states: Vec<RmLState>,
}

// ── Top-level result ───────────────────────────────────────────────────────────

/// All resonance information for one material, from MF=2/MT=151.
#[derive(Debug, Clone, Default)]
pub struct ResonanceInfo {
    /// Energy ranges in file order.
    pub ranges: Vec<EnergyRange>,
}

impl ResonanceInfo {
    /// Returns `true` if every range has LRU=0 (potential scattering only).
    ///
    /// Materials like H-2 have no resonance parameters; RECONR skips the
    /// resonance reconstruction step and uses only the MF=3 background.
    pub fn has_no_resonances(&self) -> bool {
        self.ranges.iter().all(|r| r.lru == 0)
    }

    /// Returns all SLBW/MLBW resolved resonance ranges (LRU=1, LRF=1 or 2).
    pub fn resolved_slbw_ranges(&self) -> impl Iterator<Item = &EnergyRange> {
        self.ranges.iter().filter(|r| {
            r.lru == 1
                && matches!(
                    r.formalism,
                    Some(ResonanceFormalism::Slbw) | Some(ResonanceFormalism::Mlbw)
                )
        })
    }

    /// Returns all Reich-Moore resolved resonance ranges (LRU=1, LRF=3).
    pub fn resolved_rm_ranges(&self) -> impl Iterator<Item = &EnergyRange> {
        self.ranges.iter().filter(|r| {
            r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::ReichMoore))
        })
    }
}

// ── Parser ─────────────────────────────────────────────────────────────────────

/// Parse MF=2/MT=151 and return resonance metadata with parameters.
///
/// Supports:
/// - **LRU=0** — potential scattering only (H-2); full parse.
/// - **LRU=1, LRF=1 (SLBW) or LRF=2 (MLBW)** — resolved resonances; full parse.
/// - **LRU=1, LRF=3 (Reich-Moore)** — resolved resonances; full parse.
/// - **LRU=2** — unresolved region; header only (parameters skipped).
///
/// Returns [`NjoyError::NotPorted`] for LRF=4 (Adler-Adler) and LRF=7 (R-Matrix Limited).
pub fn parse_resonance_info(sec: &Section) -> Result<ResonanceInfo, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);

    // CONT: ZA, AWR, 0, 0, NIS, 0
    let head = cur.read_cont()?;
    let nis = head.n1 as usize;

    let mut ranges = Vec::new();

    for _ in 0..nis {
        // CONT: ZAI, ABN, 0, LFW, NER, 0
        let iso = cur.read_cont()?;
        let ner = iso.n1 as usize;

        for _ in 0..ner {
            // CONT: EL, EH, LRU, LRF, NRO, NAPS
            let rh = cur.read_cont()?;
            let el   = rh.c1;
            let eh   = rh.c2;
            let lru  = rh.l1;
            let lrf  = rh.l2;
            let naps = rh.n2;

            let range = match lru {
                0 => parse_lru0(&mut cur, el, eh, naps)?,
                1 => parse_lru1(&mut cur, el, eh, lrf, naps)?,
                2 => parse_lru2_header(&mut cur, el, eh, naps)?,
                other => {
                    return Err(NjoyError::EndfParse(format!("unknown LRU={other}")));
                }
            };

            ranges.push(range);
        }
    }

    Ok(ResonanceInfo { ranges })
}

// ── Per-LRU parsers ────────────────────────────────────────────────────────────

fn parse_lru0(cur: &mut SectionCursor<'_>, el: f64, eh: f64, naps: i32)
    -> Result<EnergyRange, NjoyError>
{
    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap  = c.c2;
    let nls = c.n1 as usize;
    for _ in 0..nls {
        cur.read_cont()?;
    }
    Ok(EnergyRange {
        el, eh, lru: 0, formalism: None, spi, ap, naps,
        l_states: Vec::new(), rm_l_states: Vec::new(),
    })
}

fn parse_lru1(cur: &mut SectionCursor<'_>, el: f64, eh: f64, lrf: i32, naps: i32)
    -> Result<EnergyRange, NjoyError>
{
    match lrf {
        1 | 2 => parse_slbw_mlbw(cur, el, eh, lrf, naps),
        3     => parse_reich_moore(cur, el, eh, naps),
        _     => Err(NjoyError::NotPorted(
            "LRF ≥ 4 resonance formalism (Adler-Adler/R-Matrix) — not yet ported",
        )),
    }
}

/// LRU=1, LRF=1/2: SLBW or MLBW resolved resonances.
///
/// Header: `CONT(SPI, AP, 0, 0, NLS, 0)`.
/// Each l-state LIST: `(AWRI, QX, L, LRX, 6*NRS, NRS)` + `NRS×6` floats:
/// `ER AJ GT GN GG GF` per resonance.
fn parse_slbw_mlbw(cur: &mut SectionCursor<'_>, el: f64, eh: f64, lrf: i32, naps: i32)
    -> Result<EnergyRange, NjoyError>
{
    let formalism = if lrf == 1 { ResonanceFormalism::Slbw } else { ResonanceFormalism::Mlbw };

    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap  = c.c2;
    let nls = c.n1 as usize;

    let mut l_states = Vec::with_capacity(nls);

    for _ in 0..nls {
        let lst = cur.read_list()?;
        let awri = lst.head.c1;
        let l    = lst.head.l1 as u32;
        let nrs  = lst.head.n2 as usize;

        let mut resonances = Vec::with_capacity(nrs);
        for i in 0..nrs {
            let base = i * 6;
            resonances.push(SlbwResonance {
                er: lst.data[base],
                aj: lst.data[base + 1],
                gt: lst.data[base + 2],
                gn: lst.data[base + 3],
                gg: lst.data[base + 4],
                gf: lst.data[base + 5],
            });
        }

        l_states.push(LState { awri, l, resonances });
    }

    Ok(EnergyRange {
        el, eh, lru: 1, formalism: Some(formalism), spi, ap, naps,
        l_states, rm_l_states: Vec::new(),
    })
}

/// LRU=1, LRF=3: Reich-Moore resolved resonances.
///
/// Header: `CONT(SPI, AP, 0, LAD, NLS, 0)`.
/// Each l-state LIST: `(AWRI, APL, L, 0, 6*NRS, NRS)` + `NRS×6` floats:
/// `ER AJ GN GG GFA GFB` per resonance (note: no GT; GN before GG).
fn parse_reich_moore(cur: &mut SectionCursor<'_>, el: f64, eh: f64, naps: i32)
    -> Result<EnergyRange, NjoyError>
{
    // CONT: SPI, AP, 0, LAD, NLS, 0
    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap  = c.c2;
    let nls = c.n1 as usize;

    let mut rm_l_states = Vec::with_capacity(nls);

    for _ in 0..nls {
        let lst = cur.read_list()?;
        // LIST head: AWRI, APL, L, 0, 6*NRS, NRS
        let awri = lst.head.c1;
        let apl  = lst.head.c2;   // per-l scattering radius; 0.0 = use range AP
        let l    = lst.head.l1 as u32;
        let nrs  = lst.head.n2 as usize;

        let mut resonances = Vec::with_capacity(nrs);
        for i in 0..nrs {
            let base = i * 6;
            resonances.push(RmResonance {
                er:  lst.data[base],
                aj:  lst.data[base + 1],
                gn:  lst.data[base + 2],
                gg:  lst.data[base + 3],
                gfa: lst.data[base + 4],
                gfb: lst.data[base + 5],
            });
        }

        rm_l_states.push(RmLState { awri, apl, l, resonances });
    }

    Ok(EnergyRange {
        el, eh, lru: 1, formalism: Some(ResonanceFormalism::ReichMoore),
        spi, ap, naps,
        l_states: Vec::new(), rm_l_states,
    })
}

/// LRU=2: unresolved resonance region — header parse only.
///
/// RECONR does not use unresolved parameters (that is PURR's job). We read the
/// SPI/AP header CONT so the cursor advances past the range header, then return
/// a placeholder.
fn parse_lru2_header(cur: &mut SectionCursor<'_>, el: f64, eh: f64, naps: i32)
    -> Result<EnergyRange, NjoyError>
{
    let c = cur.read_cont()?;
    Ok(EnergyRange {
        el, eh, lru: 2, formalism: None,
        spi: c.c1, ap: c.c2, naps,
        l_states: Vec::new(), rm_l_states: Vec::new(),
    })
}
