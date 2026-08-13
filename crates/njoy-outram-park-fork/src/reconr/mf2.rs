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
    samm::mf2::RmlSection,
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
    /// LRF=4: Adler-Adler. Parsed and reconstructed via `crate::reconr::aa`
    /// (zero-temperature only, matching this crate's SLBW/Reich-Moore
    /// reconstruction — Doppler broadening is BROADR's job).
    AdlerAdler,
    /// LRF=7: R-Matrix Limited. Parsed and reconstructed via
    /// `crate::samm` (Reich-Moore-limited, `KRM=3`/`IFG=0` only, matching
    /// what `samm.f90` itself supports — see `crate::samm`'s module doc).
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

// ── Adler-Adler (LRF=4) resonance types ───────────────────────────────────────

/// One resonance in an Adler-Adler (LRF=4) evaluation: three independent
/// `(DE, DW, GR, GI)` Adler-Adler amplitude parameter sets — total,
/// fission, capture, in file order. There is no elastic set; elastic is
/// derived from unitarity (`total - fission - capture`) at evaluation
/// time — see [`super::aa::eval_aa_range`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AaResonance {
    pub de_total: f64,
    pub dw_total: f64,
    pub gr_total: f64,
    pub gi_total: f64,
    pub de_fission: f64,
    pub dw_fission: f64,
    pub gr_fission: f64,
    pub gi_fission: f64,
    pub de_capture: f64,
    pub dw_capture: f64,
    pub gr_capture: f64,
    pub gi_capture: f64,
}

/// One orbital angular momentum l-state in an Adler-Adler resolved
/// resonance region. Upstream (`rdf2aa`/`csaa` in `reconr.f90`) nests
/// resonances one level deeper by spin (J) group, but the Adler-Adler
/// cross-section formula never actually uses J — only L (for the shared
/// hard-sphere phase term) and each resonance's own `DE`/`DW`/`GR`/`GI` —
/// so every J-group's resonances are flattened into one `Vec` here rather
/// than modeling the (unused) J nesting.
#[derive(Debug, Clone)]
pub struct AaLState {
    pub l: u32,
    pub resonances: Vec<AaResonance>,
}

/// Adler-Adler background polynomial coefficients for one reaction type:
/// `background(E) = (c[0] + c[1]/E + c[2]/E^2 + c[3]/E^3 + c[4]*E + c[5]*E^2)`
/// (further scaled by a shared energy/mass factor at evaluation time — see
/// [`super::aa::eval_aa_range`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct AaBackground {
    pub total: [f64; 6],
    pub fission: [f64; 6],
    pub capture: [f64; 6],
}

/// One LRF=4 (Adler-Adler) resonance range.
#[derive(Debug, Clone)]
pub struct AaRange {
    /// Atomic weight ratio of the target (range-level, unlike SLBW/Reich-
    /// Moore's per-l-state `awri` — Adler-Adler's ENDF layout puts it on
    /// the shared background record instead).
    pub awri: f64,
    /// `LI` flag from the background record: `LI != 6` includes the total
    /// background polynomial; `LI != 5` includes the fission one; capture
    /// is always included. Upstream requires `LI >= 5`
    /// ([`crate::reconr::mf2::parse_adler_adler`] returns
    /// [`NjoyError::EndfParse`] otherwise, matching `csaa`'s own
    /// `call error('csaa','bad li value',' ')`).
    pub li: i32,
    pub background: AaBackground,
    pub l_states: Vec<AaLState>,
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
    /// R-Matrix Limited (LRF=7) section and its material AWR. `None` for
    /// every other formalism.
    pub rml: Option<RmlRange>,
    /// Adler-Adler (LRF=4) data. `None` for every other formalism.
    pub aa: Option<AaRange>,
}

/// An LRF=7 (R-Matrix Limited) resonance range, paired with the material
/// AWR its channel kinematics need (`crate::samm::context::compute_channel_kinematics`'s
/// doc comment explains why AWR is threaded in separately rather than
/// stored on [`RmlSection`] itself — it's read one level up, from MF=2's
/// own material-level `CONT`, not from the LRF=7 range header).
#[derive(Debug, Clone)]
pub struct RmlRange {
    pub section: RmlSection,
    pub awr: f64,
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
        self.ranges
            .iter()
            .filter(|r| r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::ReichMoore)))
    }

    /// Returns all R-Matrix Limited resolved resonance ranges (LRU=1, LRF=7).
    pub fn resolved_rml_ranges(&self) -> impl Iterator<Item = &EnergyRange> {
        self.ranges.iter().filter(|r| {
            r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::RMatrixLimited))
        })
    }

    /// Returns all Adler-Adler resolved resonance ranges (LRU=1, LRF=4).
    pub fn resolved_aa_ranges(&self) -> impl Iterator<Item = &EnergyRange> {
        self.ranges
            .iter()
            .filter(|r| r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::AdlerAdler)))
    }
}

// ── Parser ─────────────────────────────────────────────────────────────────────

/// Parse MF=2/MT=151 and return resonance metadata with parameters.
///
/// Supports:
/// - **LRU=0** — potential scattering only (H-2); full parse.
/// - **LRU=1, LRF=1 (SLBW) or LRF=2 (MLBW)** — resolved resonances; full parse.
/// - **LRU=1, LRF=3 (Reich-Moore)** — resolved resonances; full parse.
/// - **LRU=1, LRF=7 (R-Matrix Limited)** — resolved resonances; full parse
///   via `crate::samm::mf2::parse_rml_section` (Reich-Moore-limited only,
///   matching what `samm.f90` itself supports).
/// - **LRU=1, LRF=4 (Adler-Adler)** — resolved resonances; full parse, zero-
///   temperature only (see `crate::reconr::aa`).
/// - **LRU=2** — unresolved region; header only (parameters skipped).
pub fn parse_resonance_info(sec: &Section) -> Result<ResonanceInfo, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);

    // CONT: ZA, AWR, 0, 0, NIS, 0
    let head = cur.read_cont()?;
    let nis = head.n1 as usize;
    let awr = head.c2;

    let mut ranges = Vec::new();

    for _ in 0..nis {
        // CONT: ZAI, ABN, 0, LFW, NER, 0
        let iso = cur.read_cont()?;
        let ner = iso.n1 as usize;

        for _ in 0..ner {
            // CONT: EL, EH, LRU, LRF, NRO, NAPS
            let rh = cur.read_cont()?;
            let el = rh.c1;
            let eh = rh.c2;
            let lru = rh.l1;
            let lrf = rh.l2;
            let naps = rh.n2;

            let range = match lru {
                0 => parse_lru0(&mut cur, el, eh, naps)?,
                1 => parse_lru1(&mut cur, el, eh, lrf, naps, awr)?,
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

fn parse_lru0(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    naps: i32,
) -> Result<EnergyRange, NjoyError> {
    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap = c.c2;
    let nls = c.n1 as usize;
    for _ in 0..nls {
        cur.read_cont()?;
    }
    Ok(EnergyRange {
        el,
        eh,
        lru: 0,
        formalism: None,
        spi,
        ap,
        naps,
        l_states: Vec::new(),
        rm_l_states: Vec::new(),
        rml: None,
        aa: None,
    })
}

fn parse_lru1(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    lrf: i32,
    naps: i32,
    awr: f64,
) -> Result<EnergyRange, NjoyError> {
    match lrf {
        1 | 2 => parse_slbw_mlbw(cur, el, eh, lrf, naps),
        3 => parse_reich_moore(cur, el, eh, naps),
        4 => parse_adler_adler(cur, el, eh, naps),
        7 => parse_rml(cur, el, eh, naps, awr),
        other => Err(NjoyError::EndfParse(format!(
            "unknown resolved-resonance LRF={other}"
        ))),
    }
}

/// LRU=1, LRF=4: Adler-Adler resolved resonances.
///
/// Record layout, reconstructed by tracing `rdf2aa` (reader) and `csaa`
/// (evaluator) in `reconr.f90` — no ENDF-102 manual excerpt was available
/// locally, so this was derived purely from the Fortran's own flat-array
/// offset arithmetic, cross-checked internally (e.g. `jen=12` in `rdf2aa`
/// matching the 3 reaction groups × 4 values `csaa` reads per resonance;
/// `NPL=18` matching 3 × 6 background coefficients):
///
/// ```text
/// CONT: SPI, AP, 0, 0, NLS, 0                      (shared LRU=1 header)
/// LIST: AWRI, ?, LI, ?, 18, ?                        (background record)
///       + 18 data words: [6 total][6 fission][6 capture] polynomial coeffs
/// For l = 1..NLS:
///   CONT: 0.0, 0.0, L, 0, NJS, 0                     (l-state header)
///   For j = 1..NJS:
///     LIST: 0.0, 0.0, ?, ?, ?, NLJ                   (resonances for this l,j)
///       + NLJ × 12 data words, 3 groups of 4 per resonance:
///         [DE,DW,GR,GI]_total, [DE,DW,GR,GI]_fission, [DE,DW,GR,GI]_capture
/// ```
fn parse_adler_adler(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    naps: i32,
) -> Result<EnergyRange, NjoyError> {
    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap = c.c2;
    let nls = c.n1 as usize;

    let bg_list = cur.read_list()?;
    let awri = bg_list.head.c1;
    let li = bg_list.head.l1;
    if li < 5 {
        return Err(NjoyError::EndfParse(format!(
            "Adler-Adler: bad LI={li} (must be >=5, matching csaa's own validation)"
        )));
    }
    if bg_list.data.len() < 18 {
        return Err(NjoyError::EndfParse(format!(
            "Adler-Adler: background LIST has {} words, need >=18",
            bg_list.data.len()
        )));
    }
    let mut total = [0.0; 6];
    let mut fission = [0.0; 6];
    let mut capture = [0.0; 6];
    total.copy_from_slice(&bg_list.data[0..6]);
    fission.copy_from_slice(&bg_list.data[6..12]);
    capture.copy_from_slice(&bg_list.data[12..18]);

    let mut l_states = Vec::with_capacity(nls);
    for _ in 0..nls {
        let lc = cur.read_cont()?;
        let l = lc.l1 as u32;
        let njs = lc.n1 as usize;

        let mut resonances = Vec::new();
        for _ in 0..njs {
            let lst = cur.read_list()?;
            let nlj = lst.head.n2 as usize;
            for i in 0..nlj {
                let base = i * 12;
                resonances.push(AaResonance {
                    de_total: lst.data[base],
                    dw_total: lst.data[base + 1],
                    gr_total: lst.data[base + 2],
                    gi_total: lst.data[base + 3],
                    de_fission: lst.data[base + 4],
                    dw_fission: lst.data[base + 5],
                    gr_fission: lst.data[base + 6],
                    gi_fission: lst.data[base + 7],
                    de_capture: lst.data[base + 8],
                    dw_capture: lst.data[base + 9],
                    gr_capture: lst.data[base + 10],
                    gi_capture: lst.data[base + 11],
                });
            }
        }
        l_states.push(AaLState { l, resonances });
    }

    Ok(EnergyRange {
        el,
        eh,
        lru: 1,
        formalism: Some(ResonanceFormalism::AdlerAdler),
        spi,
        ap,
        naps,
        l_states: Vec::new(),
        rm_l_states: Vec::new(),
        rml: None,
        aa: Some(AaRange {
            awri,
            li,
            background: AaBackground {
                total,
                fission,
                capture,
            },
            l_states,
        }),
    })
}

/// LRU=1, LRF=7: R-Matrix Limited resolved resonances.
///
/// The range header's usual `CONT(SPI, AP, 0, LAD/0, NLS, 0)` +
/// per-l-state records are replaced entirely by
/// `crate::samm::mf2::parse_rml_section`'s own reading (its first read is
/// the structurally-analogous `CONT` carrying `IFG`/`KRM`/`NGROUP` instead
/// of `0`/`LAD`/`NLS` — see that function's doc comment) — so this is a
/// thin wrapper, not a from-scratch parse.
fn parse_rml(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    naps: i32,
    awr: f64,
) -> Result<EnergyRange, NjoyError> {
    let section = crate::samm::mf2::parse_rml_section(cur)?;
    Ok(EnergyRange {
        el,
        eh,
        lru: 1,
        formalism: Some(ResonanceFormalism::RMatrixLimited),
        spi: 0.0,
        ap: 0.0,
        naps,
        l_states: Vec::new(),
        rm_l_states: Vec::new(),
        rml: Some(RmlRange { section, awr }),
        aa: None,
    })
}

/// LRU=1, LRF=1/2: SLBW or MLBW resolved resonances.
///
/// Header: `CONT(SPI, AP, 0, 0, NLS, 0)`.
/// Each l-state LIST: `(AWRI, QX, L, LRX, 6*NRS, NRS)` + `NRS×6` floats:
/// `ER AJ GT GN GG GF` per resonance.
fn parse_slbw_mlbw(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    lrf: i32,
    naps: i32,
) -> Result<EnergyRange, NjoyError> {
    let formalism = if lrf == 1 {
        ResonanceFormalism::Slbw
    } else {
        ResonanceFormalism::Mlbw
    };

    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap = c.c2;
    let nls = c.n1 as usize;

    let mut l_states = Vec::with_capacity(nls);

    for _ in 0..nls {
        let lst = cur.read_list()?;
        let awri = lst.head.c1;
        let l = lst.head.l1 as u32;
        let nrs = lst.head.n2 as usize;

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

        l_states.push(LState {
            awri,
            l,
            resonances,
        });
    }

    Ok(EnergyRange {
        el,
        eh,
        lru: 1,
        formalism: Some(formalism),
        spi,
        ap,
        naps,
        l_states,
        rm_l_states: Vec::new(),
        rml: None,
        aa: None,
    })
}

/// LRU=1, LRF=3: Reich-Moore resolved resonances.
///
/// Header: `CONT(SPI, AP, 0, LAD, NLS, 0)`.
/// Each l-state LIST: `(AWRI, APL, L, 0, 6*NRS, NRS)` + `NRS×6` floats:
/// `ER AJ GN GG GFA GFB` per resonance (note: no GT; GN before GG).
fn parse_reich_moore(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    naps: i32,
) -> Result<EnergyRange, NjoyError> {
    // CONT: SPI, AP, 0, LAD, NLS, 0
    let c = cur.read_cont()?;
    let spi = c.c1;
    let ap = c.c2;
    let nls = c.n1 as usize;

    let mut rm_l_states = Vec::with_capacity(nls);

    for _ in 0..nls {
        let lst = cur.read_list()?;
        // LIST head: AWRI, APL, L, 0, 6*NRS, NRS
        let awri = lst.head.c1;
        let apl = lst.head.c2; // per-l scattering radius; 0.0 = use range AP
        let l = lst.head.l1 as u32;
        let nrs = lst.head.n2 as usize;

        let mut resonances = Vec::with_capacity(nrs);
        for i in 0..nrs {
            let base = i * 6;
            resonances.push(RmResonance {
                er: lst.data[base],
                aj: lst.data[base + 1],
                gn: lst.data[base + 2],
                gg: lst.data[base + 3],
                gfa: lst.data[base + 4],
                gfb: lst.data[base + 5],
            });
        }

        rm_l_states.push(RmLState {
            awri,
            apl,
            l,
            resonances,
        });
    }

    Ok(EnergyRange {
        el,
        eh,
        lru: 1,
        formalism: Some(ResonanceFormalism::ReichMoore),
        spi,
        ap,
        naps,
        l_states: Vec::new(),
        rm_l_states,
        rml: None,
        aa: None,
    })
}

/// LRU=2: unresolved resonance region — header parse only.
///
/// RECONR does not use unresolved parameters (that is PURR's job). We read the
/// SPI/AP header CONT so the cursor advances past the range header, then return
/// a placeholder.
fn parse_lru2_header(
    cur: &mut SectionCursor<'_>,
    el: f64,
    eh: f64,
    naps: i32,
) -> Result<EnergyRange, NjoyError> {
    let c = cur.read_cont()?;
    Ok(EnergyRange {
        el,
        eh,
        lru: 2,
        formalism: None,
        spi: c.c1,
        ap: c.c2,
        naps,
        l_states: Vec::new(),
        rm_l_states: Vec::new(),
        rml: None,
        aa: None,
    })
}
