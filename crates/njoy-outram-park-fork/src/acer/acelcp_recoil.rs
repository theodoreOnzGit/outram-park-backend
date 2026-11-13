// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine acelcp`, l.10962-11113: the heavy recoils' heating for an
//     incident charged particle (LAW = 1, l.10970-11035; LAW = 4,
//     l.11036-11091), and the assignments to `e`, `ne` and `amass` in
//     l.9242-10900 that the LAW = 4 branch reads without setting;
//   - `subroutine acelcp`, l.10318-10900: what the energy pass does with a
//     LAW = 5 subsection, for which it has no branch.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Two `acelcp` branches that no held evaluation reaches (gh:#1490), each
//! translated with its upstream quirks and checked against NJOY2016 on
//! synthetic tapes (`tests/acer_charged_particle.rs`, `synthetic_*`):
//!
//! - [`recoil_heating`]: the heating from the heavy recoils (`ZAP > 2004`) of
//!   an incident charged particle, LAW = 1 and the LAW = 4 two-body branch
//!   ([`recoil_law4_heating`]), which reads three locals it does not set
//!   ([`AcelcpStale`]);
//! - [`law5_tail_is_harmless`]: a LAW = 5 product other than the projectile,
//!   which the energy pass neither handles nor skips.

use crate::endf::interp::terpa;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::{Section, Tape};
use crate::mixr::mix::sigfig;
use crate::NjoyError;
use petir::mathf::RealMath;

use super::acelcp::Xss;

const EMEV: f64 = 1.0e6;
/// `npage` (`endf.f90:19`): the words `listio` reads in one call. A LIST
/// longer than this needs `moreio`, which the LAW = 4 branch never calls.
const NPAGE: i32 = 306;

/// `acelcp`'s locals `e`, `ne` and `amass` as the code before the heavy
/// recoils' loop leaves them (gh:#1490).
///
/// Upstream's LAW = 4 recoil heating (`acefc.f90:11036-11091`) reads all
/// three without setting them first:
/// - `ne` sizes its heating table (`scr(llht+5)`, `scr(llht+6)`), before it
///   is re-read from the first subsection's TAB2;
/// - `e` is the energy in `betasq=r1*(1+th/e)`, one value for the whole table;
/// - `amass` is in `th`, `r1` and `r2`.
///
/// They hold whatever the last assignment on the path left, so the port tracks
/// that assignment: each update in `acelcp::append_incident_mode` and in
/// [`recoil_heating`] sits at the code that mirrors it and cites its line.
/// The sources, in the order upstream runs them (for each production type:
/// the production pass, then the angular pass, then the energy pass; then the
/// heavy recoils, section by section):
///
/// | pass | branch | `ne` | `e` | `amass` |
/// |---|---|---|---|---|
/// | production, MF=6 | the item's subsection | yield `NP` (`:9506`) | `esz(nes)*emev/emev` if `iaa <= nes` (`:9477-9480`) | |
/// | angular | MT=2 | AND `NE` (`:9558`) | `esz(nes)` (`:9630`) | `awr/awi` (`:9576`) |
/// | angular | MF=4 LTT 1, 2 | TAB2 `NE` | `esz(nes)` | `awr/awi` |
/// | angular | MF=6 LAW 2, or LAW 4 from a LAW 2 first subsection | TAB2 `NE` (`:9929`) | `esz(nes)` (`:10042`) | `awr/awi` (`:9944`) |
/// | energy | MT=102 | | | `awr/awi` |
/// | energy | MF=4 | | `esz(nes)` if LTT = 0 | `awr/awi` (`:10196`) |
/// | energy | MF=6 LAW 1 | TAB2 `NE` (`:10334`) | `esz(nes)` (`:10556`) | `awp*emc2` if `NE >= 1` (`:10387`) |
/// | energy | MF=6 LAW 2 | TAB2 `NE` | | `awr/awi` (`:10529`) |
/// | energy | MF=6 LAW 3 / 4 | | `esz(nes)` for LAW 3 (`:10588`) | `awr/awi` (`:10579`) |
/// | energy | MF=6 LAW 6 | | `esz(nes)` (`:10673`) | |
/// | recoils | LAW 1, LAW 4 | TAB2 `NE` (`:10974`, `:11066`) | `esz(nes)*emev` **in eV** if `iaa <= nes`, else the last LIST's `E` (`:11025`, `:11084`) | |
///
/// `esz(nes)` is the ACE grid's top energy **in MeV**. So the LAW = 4 branch
/// divides `th` (MeV) by an `e` in MeV when the production passes set it last,
/// and by an `e` in eV when an earlier recoil did. The isotropy pre-checks'
/// `ne` (`:9430`, `:10248`) is always overwritten before it is read. `ne` is
/// initialised to 0 (`:9242`); `e` and `amass` are not initialised at all,
/// and the port refuses to read them unset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcelcpStale {
    /// `e`: `None` until an assignment above.
    pub e: Option<f64>,
    /// `ne`, 0 at entry (`:9242`).
    pub ne: i64,
    /// `amass`: `None` until an assignment above.
    pub amass: Option<f64>,
}

impl AcelcpStale {
    /// The state at `acelcp`'s entry: `ne = 0`, `e` and `amass` unset.
    pub fn new() -> Self {
        AcelcpStale { e: None, ne: 0, amass: None }
    }
}

impl Default for AcelcpStale {
    fn default() -> Self {
        Self::new()
    }
}

/// The table's ESZ/MTR/LQR/LSIG/SIG locators and counts, 1-based as
/// upstream's.
#[derive(Debug, Clone, Copy)]
pub(super) struct EszBlocks {
    pub(super) nes: usize,
    pub(super) esz: usize,
    pub(super) mtr: usize,
    pub(super) lqr: usize,
    pub(super) lsig: usize,
    pub(super) sig: usize,
    pub(super) ntr: usize,
}

/// Heating from the heavy recoils (`ZAP > 2004`) of the MF=6 reactions, for an
/// incident charged particle (`acelcp`, `acefc.f90:10936-11113`): each
/// recoil's heating `h(E)`, weighted by `sigma_r / sigma_t`, is added to ESZ's
/// heating column with `sigfig(., 7, 0)`.
///
/// - **LAW = 1** (`:10970-11035`): `h` is the yield times the mean outgoing
///   energy of each incident energy's spectrum, lin-lin.
/// - **LAW = 4** (`:11036-11091`): [`recoil_law4_heating`], which re-reads the
///   section from its first subsection and carries on from there, as upstream
///   does: the subsection count stays the original one, so a recoil that is
///   not the last subsection is followed by the section re-read from its
///   second subsection.
/// - **LAW = 0, 3** have no body; **LAW = 6, 2, 5** have one and upstream does
///   not skip it (`:11092-11103`, the message says "subsection skipped" and
///   none is): that is harmless only in the last subsection, and refused
///   otherwise. **LAW = 7** (`topfil` rewrites it to LAW = 1 on the scratch
///   tape) is refused.
///
/// A section whose MT is not in MTR is passed over (upstream's search for it,
/// `:10945-10952`, would run past the block).
///
/// # Errors
/// [`NjoyError::NotPorted`] for the cases above and from
/// [`recoil_law4_heating`]; [`NjoyError::EndfParse`] for a malformed section.
pub(super) fn recoil_heating(x: &mut Xss, tape: &Tape, mat: i32, b: &EszBlocks, awr: f64, stale: &mut AcelcpStale) -> Result<(), NjoyError> {
    let EszBlocks { mtr, lqr, lsig, sig, ntr, .. } = *b;
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6 && s.key.mt > 2) {
        let mt = sec.key.mt;
        let Some(ir) = (1..=ntr).find(|&i| x.get(mtr + i - 1).round() as i32 == mt) else { continue };
        let k = x.get(lsig + ir - 1).round() as usize + sig - 1;
        let iaa = x.get(k).round() as usize;
        let q = x.get(lqr + ir - 1);
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nk = head.n1;
        // `do ik=1,nk`: the count is fixed when the loop starts.
        for ik in 1..=nk {
            let yt = cur.read_tab1()?;
            let izap = yt.head.c1.round() as i32;
            let law = yt.head.l2;
            if izap <= 2004 {
                super::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
                continue;
            }
            match law {
                1 => law1_heating(x, &mut cur, &yt, b, k, iaa, stale)?,
                4 => {
                    let at = RecoilAt { k, iaa, q };
                    recoil_law4_heating(x, sec, b, at, awr, stale)?;
                    // `findf` rewound the section and the branch read its
                    // first subsection; the loop reads on from there.
                    cur = SectionCursor::new(&sec.rows);
                    cur.read_cont()?;
                    let law1 = cur.read_tab1()?.head.l2;
                    super::energy::mf6::skip_mf6_subsection(&mut cur, law1)?;
                }
                0 | 3 => {}
                7 => return Err(NjoyError::NotPorted("acelcp: a LAW = 7 heavy recoil (topfil's LAW = 1 rewrite is not followed here)")),
                _ if ik < nk => {
                    return Err(NjoyError::NotPorted(
                        "acelcp: a heavy recoil of LAW 2, 5 or 6 before the last subsection (upstream does not skip its body, :11092-11103)",
                    ))
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// LAW = 1 heavy recoil (`:10970-11035`): yield times the mean outgoing energy
/// `sum (E'_i - E'_{i-1}) g_{i-1} (E'_i + E'_{i-1}) / 2`, lin-lin in `E`.
fn law1_heating(x: &mut Xss, cur: &mut SectionCursor<'_>, yt: &Tab1, b: &EszBlocks, k: usize, iaa: usize, stale: &mut AcelcpStale) -> Result<(), NjoyError> {
    let (nes, esz) = (b.nes, b.esz);
    let t2 = cur.read_tab2()?;
    let ne = t2.head.n2.max(0) as usize;
    let mut heat_tab: Vec<(f64, f64)> = Vec::with_capacity(ne);
    let mut last_e = None;
    for _ in 0..ne {
        let l = cur.read_list()?;
        let e = l.head.c2;
        let np = l.head.n2.max(0) as usize;
        let (y, _, _) = terpa(&yt.interp, &yt.pairs, e);
        let (mut heat, mut epl, mut gl) = (0.0f64, 0.0f64, 0.0f64);
        for ipp in 1..=np {
            let ep = l.data.get(2 * (ipp - 1)).copied().unwrap_or(0.0);
            let g = l.data.get(2 * (ipp - 1) + 1).copied().unwrap_or(0.0);
            if ipp > 1 {
                heat += (ep - epl) * gl * (ep + epl) / 2.0;
            }
            epl = ep;
            gl = g;
        }
        heat_tab.push((e, y * heat));
        last_e = Some(e);
    }
    let hint = vec![(heat_tab.len() as u32, 2)];
    add_heating(x, &hint, &heat_tab, b, k, iaa);
    // `ne=nint(scr(ll+5))` (`:10974`); `e=c2h` per LIST, then the heating
    // loop's `e=xss(esz+ie-1)*emev` (`:11025`).
    stale.ne = ne as i64;
    stale.e = if iaa <= nes { Some(x.get(esz + nes - 1) * EMEV) } else { last_e.or(stale.e) };
    Ok(())
}

/// `do ie=iaa,nes`: `h` from the heating table at `E` in eV, then
/// `h=(h/emev)*sigma_r/sigma_t` added to ESZ's heating column with
/// `sigfig(.,7,0)` (`:11021-11033`, `:11083-11090`).
fn add_heating(x: &mut Xss, hint: &[(u32, u32)], tab: &[(f64, f64)], b: &EszBlocks, k: usize, iaa: usize) {
    let (nes, esz) = (b.nes, b.esz);
    for ie in iaa..=nes {
        let e = x.get(esz + ie - 1) * EMEV;
        let (h, _, _) = terpa(hint, tab, e);
        let h = (h / EMEV) * x.get(2 + k + ie - iaa) / x.get(esz + nes + ie - 1);
        let prev = x.get(esz + 4 * nes + ie - 1);
        x.r(esz + 4 * nes + ie - 1, sigfig(prev + h, 7, 0));
    }
}

/// The reaction's SIG entry and Q value, as the heavy recoils' loop found
/// them at the start of the section (`:10945-10952`): `k` the SIG locator,
/// `iaa` its first energy index, `q = xss(lqr+ir-1)` in MeV.
#[derive(Debug, Clone, Copy)]
pub(super) struct RecoilAt {
    pub(super) k: usize,
    pub(super) iaa: usize,
    pub(super) q: f64,
}

/// LAW = 4 heavy recoil (`acefc.f90:11036-11091`), translated with its quirks.
/// Two-body kinematics from the section's **first** subsection (the light
/// particle, normally LAW = 2): for each of its incident energies `E_i`,
///
/// `h_i = E_i r2 (gammsq + 1 - 2 ubar_i sqrt(gammsq))`,
///
/// with `th = (1+amass) q / amass`, `r1 = amass (amass+1-aprime) / aprime`,
/// `r2 = (amass+1-aprime) / (1+amass)^2`, `betasq = r1 (1 + th/e)` (0 if
/// negative), `gammsq = (aprime/(amass+1-aprime))^2 betasq`; then `h(E)`
/// lin-lin, weighted and added as in LAW = 1.
///
/// **Upstream's quirks, kept as they are (not fixed):**
/// 1. **`ne`, `e` and `amass` are stale** ([`AcelcpStale`]). The table is
///    sized by the stale `ne` (`NP = NBT = ne`) before `ne` is re-read, so only
///    the first `ne_stale` energies are used; `betasq` is evaluated **once**,
///    with the stale `e`, for every energy, and `e` is in MeV or in eV
///    depending on what set it last, while `q` is always MeV; `amass` is
///    whatever the last production item computed (`awr/awi` or `awp*emc2`).
/// 2. **`aprime = awp/awr`**: the first subsection's AWP over the target's
///    AWR (`acelcp`'s `awr`), where every other branch divides by `awi`.
/// 3. **`ubar = scr(ll+6)`** is the first word of each LIST whatever its
///    `LANG` (`a_1` for Legendre, the first cosine for a tabulation), read by
///    `listio` with no `moreio`. For a LIST with no data the word is whatever
///    `scr(ll+6)` last held: the TAB2's first `NBT`, an earlier LIST's first
///    word, or a heating-table entry written over it (the port tracks that one
///    word).
/// 4. **`findf` rewinds, but `do ik=1,nk` keeps counting**, so after this
///    branch the caller's loop reads on from the first subsection's end: a
///    recoil that is not the last subsection is followed by subsection 2, 3,
///    ... again, and meets itself again with `e` now in eV. [`recoil_heating`]
///    carries on from the first subsection's end, as upstream does.
///
/// # Errors
/// [`NjoyError::NotPorted`] where upstream reads scratch words this branch
/// did not write in the same call, or reads past a record:
/// - `e` or `amass` never set, or `ne_stale < 1`;
/// - the first subsection's TAB2 has fewer energies than `ne_stale` (the
///   table's last points would be leftovers);
/// - a first subsection that is not LAW 1, 2 or 5 (no TAB2 and LISTs to read);
/// - a LIST longer than `npage` words (`listio` leaves the rest unread);
/// - a LIST with no data and no known `scr(ll+6)`.
pub(super) fn recoil_law4_heating(
    x: &mut Xss,
    sec: &Section,
    b: &EszBlocks,
    at: RecoilAt,
    awr: f64,
    stale: &mut AcelcpStale,
) -> Result<(), NjoyError> {
    let (nes, esz) = (b.nes, b.esz);
    let RecoilAt { k, iaa, q } = at;
    let (Some(e_stale), Some(amass)) = (stale.e, stale.amass) else {
        return Err(NjoyError::NotPorted("acelcp: LAW = 4 recoil heating reads e or amass before anything set it (:11067-11070)"));
    };
    let ne_stale = stale.ne;
    if ne_stale < 1 {
        return Err(NjoyError::NotPorted("acelcp: LAW = 4 recoil heating with a stale ne < 1 (an empty heating table, :11043-11049)"));
    }
    let ne_s = ne_stale as usize;
    // `call findf(matd,6,mt,nin)`, `contio`, then the first subsection.
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    let y1 = cur.read_tab1()?;
    if !matches!(y1.head.l2, 1 | 2 | 5) {
        return Err(NjoyError::NotPorted("acelcp: LAW = 4 recoil heating whose first subsection has no TAB2 and LISTs"));
    }
    let aprime = y1.head.c2 / awr;
    let t2 = cur.read_tab2()?;
    let ne_new = t2.head.n2.max(0) as usize;
    if ne_new < ne_s {
        return Err(NjoyError::NotPorted(
            "acelcp: LAW = 4 recoil heating with fewer energies than the stale ne (the table's last points are leftover scratch words)",
        ));
    }
    let th = (1.0 + amass) * q / amass;
    let r1 = amass * (amass + 1.0 - aprime) / aprime;
    let r2 = (amass + 1.0 - aprime) / (1.0 + amass).r_powi(2);
    let mut betasq = r1 * (1.0 + th / e_stale);
    if betasq < 0.0 {
        betasq = 0.0;
    }
    let gammsq = (aprime / (amass + 1.0 - aprime)).r_powi(2) * betasq;
    // Upstream's scratch layout, 1-based with `llht = 1`: the table header in
    // scr(1..8) and entry `ie` in scr(7+2ie), scr(8+2ie); the first
    // subsection's TAB1 from `lly = 9+2*ne_stale`, its TAB2 and each LIST at
    // `ll` just after the TAB1. `cell` is `scr(ll+6)`, the word read as `ubar`.
    let lly = 9 + 2 * ne_s;
    let ll = lly + 6 + 2 * y1.interp.len() + 2 * y1.pairs.len();
    let cell = ll + 6;
    // `tab2io` puts the TAB2's first NBT there.
    let mut cell_word: Option<f64> = t2.interp.first().map(|&(nbt, _)| f64::from(nbt));
    let mut table: Vec<(f64, f64)> = Vec::with_capacity(ne_s);
    let mut last_e = None;
    for ie in 1..=ne_new {
        let l = cur.read_list()?;
        if l.head.n1 > NPAGE {
            return Err(NjoyError::NotPorted("acelcp: LAW = 4 recoil heating with a LIST longer than npage (read with no moreio)"));
        }
        if let Some(&w) = l.data.first() {
            cell_word = Some(w);
        }
        let e = l.head.c2;
        let Some(ubar) = cell_word else {
            return Err(NjoyError::NotPorted("acelcp: LAW = 4 recoil heating reads ubar from a scratch word nothing wrote"));
        };
        let h = e * r2 * (gammsq + 1.0 - 2.0 * ubar * gammsq.sqrt());
        if ie <= ne_s {
            table.push((e, h));
        }
        if 7 + 2 * ie == cell {
            cell_word = Some(e);
        }
        if 8 + 2 * ie == cell {
            cell_word = Some(h);
        }
        last_e = Some(e);
    }
    let hint = vec![(ne_s as u32, 2)];
    add_heating(x, &hint, &table, b, k, iaa);
    // `ne=nint(scr(ll+5))` (`:11066`); `e=scr(ll+1)` per LIST, then the
    // heating loop's `e=xss(esz+ie-1)*emev` (`:11084`).
    stale.ne = ne_new as i64;
    stale.e = if iaa <= nes { Some(x.get(esz + nes - 1) * EMEV) } else { last_e.or(stale.e) };
    Ok(())
}

/// Whether the heavy recoils' heating is translated for every heavy recoil of
/// the evaluation, judged from the tape alone (the stale-value refusals of
/// [`recoil_law4_heating`] depend on the production history and are found
/// only when the table is built).
///
/// False for: a LAW = 7 heavy recoil; a heavy recoil of LAW 2, 5 or 6 before
/// the last subsection; a LAW = 4 heavy recoil whose section's first
/// subsection is not LAW 1, 2 or 5, or has a LIST longer than `npage`.
pub fn recoil_heating_supported(tape: &Tape, mat: i32) -> bool {
    tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6 && s.key.mt > 2).all(|sec| section_supported(sec).unwrap_or(false))
}

fn section_supported(sec: &Section) -> Result<bool, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);
    let nk = cur.read_cont()?.n1;
    for ik in 1..=nk {
        let yt = cur.read_tab1()?;
        let (izap, law) = (yt.head.c1.round() as i32, yt.head.l2);
        if izap > 2004 {
            match law {
                7 => return Ok(false),
                2 | 5 | 6 if ik < nk => return Ok(false),
                4 => {
                    let mut c1 = SectionCursor::new(&sec.rows);
                    c1.read_cont()?;
                    let y1 = c1.read_tab1()?;
                    if !matches!(y1.head.l2, 1 | 2 | 5) {
                        return Ok(false);
                    }
                    let t2 = c1.read_tab2()?;
                    for _ in 0..t2.head.n2.max(0) {
                        if c1.read_list()?.head.n1 > NPAGE {
                            return Ok(false);
                        }
                    }
                }
                _ => {}
            }
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
    }
    Ok(true)
}

/// The records upstream's energy pass reads after a LAW = 5 subsection `l`
/// (gh:#1490): `(C1, C2, L2)` of the body's TAB2 head and of each LIST head,
/// one per binary record, in order. `None` if a LIST is longer than `npage`
/// (it would take more than one record) or the section is malformed.
fn law5_body_records(sec: &Section, l: i32) -> Option<(i32, Vec<(f64, f64, i32)>)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let nk = cur.read_cont().ok()?.n1;
    for _ in 1..l {
        let law = cur.read_tab1().ok()?.head.l2;
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    cur.read_tab1().ok()?;
    let t2 = cur.read_tab2().ok()?;
    let mut recs = vec![(t2.head.c1, t2.head.c2, t2.head.l2)];
    for _ in 0..t2.head.n2.max(0) {
        let li = cur.read_list().ok()?;
        if li.head.n1 > NPAGE {
            return None;
        }
        recs.push((li.head.c1, li.head.c2, li.head.l2));
    }
    Some((nk, recs))
}

/// Whether what upstream does after a LAW = 5 production item `l` (a product
/// other than the projectile) is what this port does: nothing.
///
/// Upstream (gh:#1490, read in `acefc.f90`):
/// - the production pass stores its yield, and the angular pass sets LANDH to
///   -1 and skips the body (`skip6a` reads LAW = 5);
/// - the energy pass (`:10318-10900`) has no LAW = 5 branch. It leaves the
///   DLWH entry `LNW = 0`, `LAW = 0` with the third word unwritten (0, from
///   `xss=0` at ACER's start, `:113`), adds no heating, does not set `idone`,
///   and **does not skip the body**. `change` then stops at that entry
///   ("Undefined law for dlwh block: 0", `:13945`), so NJOY2016 writes no
///   complete table; `acefix_change::xss_lines` stops the same way.
///
/// If `l` is the last subsection, that is all (true). Otherwise the loop's
/// next `tab1io` reads the body's records as TAB1s. On a **binary** scratch
/// tape (`text = false`, as test decks 14 and 50-71 use) each read takes one
/// record, and it is a no-op when its `L2` is no law `skip6a` reads (not 1, 2,
/// 5, 6, 7) and it does not reach the `mt = 102` branch (`ZAP = nint(C1) =
/// ip` with MT = 102): then true. A text scratch tape reads `2 NR + 2 NP`
/// numbers from the lines that follow, which is not translated (false).
pub fn law5_tail_is_harmless(sec: &Section, l: i32, ip: i32, mt: i32, text: bool) -> bool {
    let Some((nk, recs)) = law5_body_records(sec, l) else { return false };
    if l == nk {
        return true;
    }
    if text {
        return false;
    }
    let m = (nk - l) as usize;
    m <= recs.len() && recs[..m].iter().all(|&(c1, _, l2)| !matches!(l2, 1 | 2 | 5 | 6 | 7) && !(c1.round() as i32 == ip && mt == 102))
}

/// Upstream's `awp` after the energy pass over a LAW = 5 item `l`: `C2` of the
/// last TAB1 its loop read, which for a subsection before the last is a
/// record of the LAW = 5 body (see [`law5_tail_is_harmless`]). `None` when
/// that is not one of the body's records.
pub(super) fn law5_tail_last_c2(sec: &Section, l: i32) -> Option<f64> {
    let (nk, recs) = law5_body_records(sec, l)?;
    let m = (nk - l) as usize;
    if m == 0 {
        return None;
    }
    recs.get(m - 1).map(|r| r.1)
}
