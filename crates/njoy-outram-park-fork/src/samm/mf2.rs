//! ENDF File 2 LRF=7 (R-matrix limited, Reich-Moore-limited) parameter reader.
//!
//! Ported from `rdsammy`'s `mode==7` branch (`samm.f90:1046-1263`) and the
//! corresponding size-scanning pass in `s2sammy` (`samm.f90:474-596`).
//!
//! **Scope, matching what `samm.f90` itself supports:** `rdsammy` hard-errors
//! on `IFG≠0` ("reduced resonance widths are not supported") and on `KRM≠3`
//! ("LRF=7 currently only supports Reich-Moore"), `samm.f90:1054-1059`. So
//! upstream NJOY *itself* only ever exercises the Reich-Moore-limited case —
//! this port matches that restriction rather than attempting the fully
//! general R-matrix (KRM=1/2/4) that NJOY's own driver never reaches.
//!
//! In Reich-Moore, the radiative-capture channel is eliminated analytically
//! (it never needs an explicit R-matrix row/column — its effect is folded
//! into the level matrix as a simple additive term); ENDF's LRF=7 format
//! still lists it as one of the raw channels in the spin-group channel list
//! (flagged by `MT=102`/particle-pair index 1), and the resonance-parameter
//! records list its width `Γγ` alongside the explicit channels' widths in
//! raw ENDF channel order — `rdsammy` reorders these into `(Γγ, Γ_1, …,
//! Γ_ichan)` after reading. See [`parse_rml_section`]'s doc comment for a
//! caveat on that specific reorder step.

use crate::endf::records::SectionCursor;
use crate::NjoyError;

/// One particle pair (an entrance/exit channel's asymptotic two-body system)
/// — ported from the particle-pair LIST record, `samm.f90:1071-1088`.
#[derive(Debug, Clone, Copy)]
pub struct ParticlePair {
    /// Mass of particle `a` (neutron mass units, `AMU`).
    pub mass_a: f64,
    /// Mass of particle `b` (residual nucleus, `AMU`).
    pub mass_b: f64,
    /// Charge (`Z`) of particle `a`.
    pub z_a: i32,
    /// Charge (`Z`) of particle `b`.
    pub z_b: i32,
    /// Spin of particle `a`.
    pub spin_a: f64,
    /// Spin of particle `b`.
    pub spin_b: f64,
    /// Reaction Q-value \[eV\].
    pub q_value: f64,
    /// Penetrability flag (`LPENT`; nonzero enables the centrifugal-barrier
    /// penetrability factor for this pair's channels).
    pub penetrability_flag: i32,
    /// Shift-factor flag (`ISHIFT`; nonzero enables the level-shift term for
    /// this pair's channels).
    pub shift_flag: i32,
    /// Reaction type (`MT`) this particle pair represents (e.g. `102` for
    /// the eliminated capture pair, `2` for elastic).
    pub mt: i32,
    /// Parity of particle `a`.
    pub parity_a: f64,
    /// Parity of particle `b`.
    pub parity_b: f64,
}

/// One explicit (non-eliminated) channel within a spin group — ported from
/// the spin-group channel-list LIST record, `samm.f90:1098-1132`.
#[derive(Debug, Clone, Copy)]
pub struct RmlChannel {
    /// 1-indexed [`ParticlePair`] this channel belongs to.
    pub particle_pair: usize,
    /// Orbital angular momentum `l`.
    pub l: i32,
    /// Channel spin `s`.
    pub channel_spin: f64,
    /// Boundary condition `B_c`.
    pub boundary: f64,
    /// Effective channel radius \[10⁻¹² cm\] (used for the penetrability).
    pub radius_eff: f64,
    /// True channel radius \[10⁻¹² cm\] (used for the hard-sphere phase
    /// shift).
    pub radius_true: f64,
}

/// One resonance within a spin group — ported from the resonance-parameter
/// LIST record, `samm.f90:1134-1196`, after `rdsammy`'s eliminated-channel
/// reorder (see [`parse_rml_section`]'s caveat).
#[derive(Debug, Clone)]
pub struct RmlResonance {
    /// Resonance energy `E_λ` \[eV\].
    pub energy: f64,
    /// Eliminated radiative-capture width `Γγ` \[eV\].
    pub gamma_gamma: f64,
    /// Explicit-channel widths `Γ_1..Γ_ichan` \[eV\], one per
    /// [`SpinGroup::channels`] entry, same order.
    pub channel_widths: Vec<f64>,
}

/// One spin-parity group `(Jπ)` — ported from `samm.f90:1093-1257`'s
/// per-group loop.
#[derive(Debug, Clone)]
pub struct SpinGroup {
    /// Total angular momentum `J`.
    pub j: f64,
    /// Parity `π` (`+1.0` or `-1.0`).
    pub parity: f64,
    /// Explicit (non-eliminated) channels in this group.
    pub channels: Vec<RmlChannel>,
    /// Resonances in this group.
    pub resonances: Vec<RmlResonance>,
}

/// One LRF=7 (R-matrix limited) resonance-range section.
#[derive(Debug, Clone)]
pub struct RmlSection {
    /// Every particle pair declared for this section.
    pub particle_pairs: Vec<ParticlePair>,
    /// Every spin-parity group.
    pub spin_groups: Vec<SpinGroup>,
}

/// Parse one LRF=7 section — ported from `rdsammy`'s `mode==7` branch
/// (`samm.f90:1046-1263`), preceded by the shared CONT record `rdsammy`'s
/// preamble reads before branching on `mode` (`samm.f90:658-664`, providing
/// `IFG`/`KRM`/`NLS`; `NLS` is `NGROUP` for LRF=7 — `samm.f90:1048-1049`).
///
/// # Known unverified step — the eliminated-channel reorder
///
/// Each resonance record lists `Γγ` and the explicit channels' widths in
/// **raw ENDF channel order** (the eliminated capture channel can appear at
/// *any* position among the raw channels, not necessarily first). `rdsammy`
/// reorders these into `(Γγ, Γ_1, …, Γ_ichan)` via `samm.f90:1185-1194`:
///
/// ```text
/// if (igamma.ne.1) then
///    x=gamma(igamma+1,kres,ier)
///    if (igamma.gt.1) then
///       do i=igamma,2,-1
///          gamma(i,kres,ier)=gamma(i-1,kres,ier)
///       enddo
///    endif
///    gamma(1,kres,ier)=gamgam(kres,ier)
///    gamgam(kres,ier)=x
/// endif
/// ```
///
/// This is ported **literally** below (mechanically re-indexed from
/// Fortran's 1-indexed `gamma(1..ichan)` to Rust's 0-indexed array), *not*
/// independently re-derived: hand-tracing the raw-channel-to-width-array
/// correspondence twice, by two different routes, both predicted the read
/// index should be `igamma-1` (the slot the provisional loop would have
/// placed the true `Γγ` value in), not `igamma+1` as the source states —
/// and neither attempt could resolve the discrepancy from the source alone
/// (no ENDF-102 LRF=7 spec or reference output was available to check
/// against). Faithful translation of a line whose exact intent could not be
/// independently confirmed is the correct choice here — guessing a "fixed"
/// version would risk being wrong in a way that's harder to detect than an
/// honestly-flagged literal port. **This is the single highest-priority
/// thing to verify against a real LRF=7 evaluation** (e.g. ¹⁶O or ¹⁹F) before
/// trusting any cross section this module produces for a spin group where
/// the eliminated channel is not first in the raw channel list
/// (`igamma != 1`). Read access to the reordering source index is bounds
/// checked and returns [`NjoyError::EndfParse`] rather than panicking or
/// silently wrapping, in case the discrepancy above indicates a genuine
/// out-of-range access for some inputs.
pub fn parse_rml_section(cur: &mut SectionCursor<'_>) -> Result<RmlSection, NjoyError> {
    // samm.f90:658-664 (shared preamble) — C1=SPIN(unused here), C2=AP(unused
    // here, NRO=0 required), L1=IFG, L2=KRM, N1=NLS(=NGROUP for LRF=7).
    let head = cur.read_cont()?;
    let ifg = head.l1;
    let krm = head.l2;
    let ngroup = head.n1.max(0) as usize;

    if ifg != 0 {
        return Err(NjoyError::EndfParse(
            "samm: reduced resonance widths (IFG=1) are not supported".to_string(),
        ));
    }
    if krm != 3 {
        return Err(NjoyError::EndfParse(format!(
            "samm: LRF=7 currently only supports Reich-Moore (KRM=3), got KRM={krm}"
        )));
    }

    // samm.f90:1063-1088 — particle-pair list.
    let pp_list = cur.read_list()?;
    let npp = pp_list.head.l1.max(0) as usize;
    let mut particle_pairs = Vec::with_capacity(npp);
    for i in 0..npp {
        let base = i * 12;
        let get = |k: usize| pp_list.data.get(base + k).copied().unwrap_or(0.0);
        particle_pairs.push(ParticlePair {
            mass_a: get(0),
            mass_b: get(1),
            z_a: get(2).round() as i32,
            z_b: get(3).round() as i32,
            spin_a: get(4),
            spin_b: get(5),
            q_value: get(6),
            penetrability_flag: get(7).round() as i32,
            shift_flag: get(8).round() as i32,
            mt: get(9).round() as i32,
            parity_a: get(10),
            parity_b: get(11),
        });
    }

    // samm.f90:1093-1257 — loop over spin groups.
    let mut spin_groups = Vec::with_capacity(ngroup);
    for _ in 0..ngroup {
        // samm.f90:1098-1132 — spin-group channel-list header.
        let ch_list = cur.read_list()?;
        let j = ch_list.head.c1;
        let parity = ch_list.head.c2;
        let kbk = ch_list.head.l1.max(0) as usize;
        let ichp1 = ch_list.head.n2.max(0) as usize; // raw channel count, incl. the eliminated one

        let mut channels: Vec<RmlChannel> = Vec::with_capacity(ichp1.saturating_sub(1));
        let mut igamma: Option<usize> = None; // 0-indexed raw channel position of the eliminated channel
        for ich in 0..ichp1 {
            let base = ich * 6;
            let get = |k: usize| ch_list.data.get(base + k).copied().unwrap_or(0.0);
            let ippx = get(0).round() as i32;
            if ippx == 1 {
                igamma = Some(ich);
            } else {
                channels.push(RmlChannel {
                    particle_pair: ippx.max(0) as usize,
                    l: get(1).round() as i32,
                    channel_spin: get(2),
                    boundary: get(3),
                    radius_eff: get(4),
                    radius_true: get(5),
                });
            }
        }
        let ichan = channels.len();
        let igamma = igamma.ok_or_else(|| {
            NjoyError::EndfParse("samm: spin group has no eliminated (particle-pair 1) channel".to_string())
        })?;

        // samm.f90:1134-1196 — resonance-parameter list.
        let res_list = cur.read_list()?;
        let nresg = res_list.head.l2.max(0) as usize;
        // samm.f90:1146-1147 — each resonance record is padded to a 6-word
        // boundary; needs 12 words (2 sub-rows) once `2+ichan > 6`.
        let nx = if 2 + ichan <= 6 { 6 } else { 12 };

        let mut resonances = Vec::with_capacity(nresg);
        for ires in 0..nresg {
            let base = ires * nx;
            let energy = res_list.data.get(base).copied().unwrap_or(0.0);
            // samm.f90:1181-1183 — provisional (pre-reorder) assignment:
            // gamgam <- raw channel 0's width, channel_widths[k] <- raw
            // channel (k+1)'s width, for k = 0..ichan.
            let mut gamma_gamma = res_list.data.get(base + 1).copied().unwrap_or(0.0);
            let mut channel_widths: Vec<f64> =
                (0..ichan).map(|k| res_list.data.get(base + 2 + k).copied().unwrap_or(0.0)).collect();

            // samm.f90:1185-1194 — eliminated-channel reorder (see this
            // function's doc comment: ported literally, unverified).
            if igamma != 0 {
                let x = *channel_widths.get(igamma + 1).ok_or_else(|| {
                    NjoyError::EndfParse(format!(
                        "samm: eliminated-channel reorder index out of range (igamma={igamma}, ichan={ichan}) — see parse_rml_section's doc comment"
                    ))
                })?;
                // Fortran `do i=igamma,2,-1: gamma(i)=gamma(i-1)` (1-indexed)
                // -> 0-indexed `for i in (1..=igamma).rev(): channel_widths[i] = channel_widths[i-1]`.
                for i in (1..=igamma).rev() {
                    channel_widths[i] = channel_widths[i - 1];
                }
                channel_widths[0] = gamma_gamma;
                gamma_gamma = x;
            }

            resonances.push(RmlResonance { energy, gamma_gamma, channel_widths });
        }

        // samm.f90:1199-1256 — background R-matrix elements. Not ported (see
        // crate README caveats — a secondary, rarer LRF=7 feature); the
        // cursor is still advanced correctly past them so subsequent parsing
        // (the next spin group, or whatever follows this section) is not
        // corrupted.
        for _ in 0..kbk {
            let bk_head = cur.read_cont()?;
            let lch = bk_head.l1;
            let lbk = bk_head.l2;
            if lbk != 0 && lch == 1 {
                return Err(NjoyError::EndfParse(
                    "samm: background R-matrix data is not allowed for the eliminated capture channel"
                        .to_string(),
                ));
            }
            if lbk == 1 {
                cur.read_tab1()?;
                cur.read_tab1()?;
            } else if lbk == 2 || lbk == 3 {
                cur.read_list()?;
            }
        }

        spin_groups.push(SpinGroup { j, parity, channels, resonances });
    }

    Ok(RmlSection { particle_pairs, spin_groups })
}
