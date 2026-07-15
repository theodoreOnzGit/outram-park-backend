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
//! Γ_ichan)` after reading. That reorder had two off-by-one defects in
//! upstream Fortran, corrected here in [`reorder_eliminated_channel`] (bug
//! op-cjw.3); see its doc comment for the derivation and verification.

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
/// LIST record, `samm.f90:1134-1196`, after the eliminated-channel reorder
/// (see [`reorder_eliminated_channel`]).
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
/// # Eliminated-channel reorder — two upstream defects corrected (bug op-cjw.3)
///
/// Each resonance record lists `Γγ` and the explicit channels' widths in
/// **raw ENDF channel order** (the eliminated capture channel can appear at
/// *any* position among the raw channels, not necessarily first). `rdsammy`
/// reorders these into `(Γγ, Γ_1, …, Γ_ichan)` via `samm.f90:1185-1194`.
/// That reorder is delegated to [`reorder_eliminated_channel`], whose doc
/// comment carries the full derivation. The upstream Fortran has two latent
/// off-by-one defects in this block (a wrong extract index and a wrong shift
/// bound); both are corrected in the port and pinned by unit tests. This
/// module's port is therefore **not** a literal translation of those two
/// lines — it is the corrected version, cross-checked against the provisional
/// layout the immediately-preceding read establishes.
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

            // samm.f90:1185-1194 — eliminated-channel reorder. Two off-by-one
            // defects in the upstream Fortran are corrected here (see
            // [`reorder_eliminated_channel`]'s doc comment for the full
            // derivation and verification); bug op-cjw.3.
            reorder_eliminated_channel(&mut gamma_gamma, &mut channel_widths, igamma)?;

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

/// Separate the eliminated radiative-capture width `Γγ` out of a resonance's
/// raw-channel width array and leave the explicit-channel widths in
/// explicit-channel order — the corrected form of `rdsammy`'s reorder
/// (`samm.f90:1185-1194`).
///
/// # Inputs and provisional layout
///
/// On entry the widths are in the **provisional** layout the immediately
/// preceding read (`samm.f90:1181-1183`) produces from the `ichp1 = ichan + 1`
/// raw-channel width words `w_1 … w_{ichp1}` of one resonance record:
///
/// - `gamma_gamma` = `w_1` (raw channel 1's width)
/// - `channel_widths[k]` = `w_{k+2}` (raw channel `k+2`'s width), `k = 0..ichan`
///
/// i.e. provisionally `gamma_gamma` = raw₁ and `channel_widths[k]` = raw₍ₖ₊₂₎.
///
/// `igamma` is the **0-indexed** raw-channel position of the eliminated
/// (particle-pair 1, `MT=102`) channel; the corresponding 1-indexed Fortran
/// index is `igamma + 1`.
///
/// # Target layout
///
/// On return:
/// - `gamma_gamma` = the eliminated channel's width `Γγ` (raw channel `igamma`)
/// - `channel_widths[k]` = the `k`-th explicit channel's width, where the
///   explicit channels are the raw channels with the eliminated one removed,
///   preserving order.
///
/// Units: all widths are resonance partial widths in electron-volts (eV).
///
/// # Why the upstream Fortran is wrong (two off-by-one defects)
///
/// With the provisional layout above, the eliminated channel's true width
/// sits at `channel_widths[igamma - 1]`. Let `p = igamma + 1` be the 1-indexed
/// raw position of the eliminated channel. Provisionally `channel_widths[k]`
/// holds the width of raw channel `k + 2` (0-indexed `k`), so
/// `channel_widths[igamma - 1] = channel_widths[p - 2]` holds raw channel
/// `(p - 2) + 2 = p`'s width — exactly the eliminated channel. The explicit
/// channels at raw positions `1..p-1` are provisionally shifted one slot too
/// high, and the eliminated width lands exactly one slot below where upstream
/// reads it:
///
/// - Upstream reads `x = gamma(igamma+1)` (`samm.f90:1186`). Correct is
///   `gamma(igamma-1)` → 0-indexed `channel_widths[igamma - 1]`. Upstream's
///   `igamma+1` (0-indexed `channel_widths[igamma + 1]`) points **two** slots
///   past the eliminated width and, when the eliminated channel is last, reads
///   past this resonance's written data into the globally-sized backing array.
/// - Upstream shifts `do i = igamma, 2, -1` (`samm.f90:1188`). Correct upper
///   bound is `igamma-1` → 0-indexed loop over `(1..igamma).rev()`. Upstream's
///   bound of `igamma` overwrites `channel_widths[igamma]`, an explicit channel
///   that is already in its correct slot, clobbering it with a duplicate.
///
/// Fixing only the extract index (as the bug ticket's one-line summary
/// suggested) is insufficient: for a group whose eliminated channel is neither
/// first nor last, the wrong shift bound still corrupts the last explicit
/// channel. Both indices are corrected here; the unit tests
/// [`tests::reorder_eliminated_first`], [`tests::reorder_eliminated_middle`],
/// and [`tests::reorder_eliminated_last`] pin all three regimes.
///
/// # Errors
///
/// Returns [`NjoyError::EndfParse`] if the derived source index is out of range
/// (defensive — with the corrected `igamma - 1` and `igamma >= 1` this cannot
/// happen for a well-formed section, but a malformed `ichp1`/channel count is
/// reported rather than panicking).
fn reorder_eliminated_channel(
    gamma_gamma: &mut f64,
    channel_widths: &mut [f64],
    igamma: usize,
) -> Result<(), NjoyError> {
    // samm.f90:1185 `if (igamma.ne.1)` — when the eliminated channel is the
    // first raw channel (0-indexed igamma == 0), the provisional layout already
    // matches the target (gamma_gamma = raw1 = Γγ, and channel_widths is already
    // in explicit order): nothing to do.
    if igamma == 0 {
        return Ok(());
    }

    // CORRECTED extract index: eliminated width sits at channel_widths[igamma-1]
    // (upstream `gamma(igamma+1)` is wrong — see doc comment).
    let x = *channel_widths.get(igamma - 1).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "samm: eliminated-channel reorder index out of range (igamma={igamma}, nchan={})",
            channel_widths.len()
        ))
    })?;

    // CORRECTED shift bound: shift the block channel_widths[0..igamma-1] up by
    // one into [1..igamma], stopping at igamma-1 (upstream loops to igamma,
    // clobbering the already-correct explicit channel there — see doc comment).
    // Fortran `do i=igamma-1,2,-1: gamma(i)=gamma(i-1)` (1-indexed) ->
    // 0-indexed `for i in (1..igamma).rev(): channel_widths[i] = channel_widths[i-1]`.
    for i in (1..igamma).rev() {
        channel_widths[i] = channel_widths[i - 1];
    }
    channel_widths[0] = *gamma_gamma;
    *gamma_gamma = x;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The three tests below share a common construction. For an LRF=7 spin
    // group with `ichan` explicit channels plus one eliminated capture channel
    // at raw position `p` (1-indexed), the resonance record lists one width per
    // raw channel in raw order. We label the eliminated width `Γγ` and the
    // explicit widths by their raw position. `parse_rml_section`'s provisional
    // read (samm.f90:1181-1183) then loads:
    //   gamma_gamma = raw₁'s width
    //   channel_widths[k] = raw₍ₖ₊₂₎'s width   (k = 0..ichan)
    // and `reorder_eliminated_channel` must recover:
    //   gamma_gamma = Γγ (the eliminated width)
    //   channel_widths = the explicit widths in explicit-channel order.
    //
    // METHODOLOGY: build the provisional arrays by hand for a chosen raw layout,
    // run the reorder, and assert against the target layout derived directly
    // from the raw channel order (eliminated removed). PASS CRITERION: exact
    // f64 equality (the reorder only moves values, never computes on them).
    //
    // RESULTS (2026-07-15, verified against samm.f90 @ ac5adf5): all three
    // regimes pass; the pre-fix code (`igamma+1` extract, `igamma` shift bound)
    // fails `reorder_eliminated_middle` and `reorder_eliminated_last`.

    /// Eliminated channel first (raw position 1, 0-indexed igamma = 0): the
    /// no-op branch. Raw order [Γγ, A, B]; provisional already correct.
    #[test]
    fn reorder_eliminated_first() {
        // raw: [Γγ, A, B]; provisional: gamma_gamma = Γγ, widths = [A, B].
        let mut gg = 100.0; // Γγ
        let mut w = vec![10.0, 20.0]; // [A, B]
        reorder_eliminated_channel(&mut gg, &mut w, 0).unwrap();
        assert_eq!(gg, 100.0, "Γγ unchanged when eliminated channel is first");
        assert_eq!(w, vec![10.0, 20.0], "explicit widths unchanged");
    }

    /// Eliminated channel in the middle (raw position 2, igamma = 1).
    /// Raw order [A, Γγ, B]; provisional gamma_gamma = A, widths = [Γγ, B].
    /// Target: gamma_gamma = Γγ, widths = [A, B].
    #[test]
    fn reorder_eliminated_middle() {
        let mut gg = 10.0; // provisional Γγ-slot holds A (raw₁)
        let mut w = vec![100.0, 20.0]; // [Γγ, B]
        reorder_eliminated_channel(&mut gg, &mut w, 1).unwrap();
        assert_eq!(gg, 100.0, "Γγ recovered from channel_widths[igamma-1]");
        assert_eq!(w, vec![10.0, 20.0], "explicit widths [A, B] in order");
    }

    /// Eliminated channel last (raw position 4, igamma = 3) in a 3-explicit
    /// group. Raw order [A, B, C, Γγ]; provisional gamma_gamma = A,
    /// widths = [B, C, Γγ]. Target: gamma_gamma = Γγ, widths = [A, B, C].
    #[test]
    fn reorder_eliminated_last() {
        let mut gg = 10.0; // A (raw₁)
        let mut w = vec![20.0, 30.0, 100.0]; // [B, C, Γγ]
        reorder_eliminated_channel(&mut gg, &mut w, 3).unwrap();
        assert_eq!(gg, 100.0, "Γγ recovered from the last provisional slot");
        assert_eq!(w, vec![10.0, 20.0, 30.0], "explicit widths [A, B, C] in order");
    }

    /// A larger middle case (5 explicit channels, eliminated at raw position 3,
    /// igamma = 2) to exercise a non-trivial shift block.
    /// Raw order [A, B, Γγ, C, D, E]; provisional gamma_gamma = A,
    /// widths = [B, Γγ, C, D, E]. Target: gamma_gamma = Γγ, widths = [A,B,C,D,E].
    #[test]
    fn reorder_eliminated_middle_large() {
        let mut gg = 1.0; // A
        let mut w = vec![2.0, 100.0, 3.0, 4.0, 5.0]; // [B, Γγ, C, D, E]
        reorder_eliminated_channel(&mut gg, &mut w, 2).unwrap();
        assert_eq!(gg, 100.0);
        assert_eq!(w, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }
}
