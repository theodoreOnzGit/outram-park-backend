// Translated from NJOY2016.80 `src/groupr.f90`: the reaction lists `conver` builds
// (`:10920-11827`), `nextr` (`:1132-1317`), the automatic-list bookkeeping of
// `subroutine groupr` (`:640-750`) and `getyld`'s MF=6/MF=9 product yields
// (`:6521-6644`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GROUPR's automatic reaction lists** (gh:#1369): what a card 9 with no `mtd`
//! (`mfd /`) expands to.
//!
//! `conver` walks the material once and records, in tape order:
//! - `mf4`: the MF=4 MTs, consecutive MTs stored as a negative range end. MT=18 is
//!   never merged into a range, and an MT=19 straight after it **replaces** it
//!   (`:10994-11003`), so a list `18, 19, 20, 21` is visited as `19, 20, 21`.
//! - `mf6` / `mf18`: the MF=6 MTs with a neutron (`ZAP = 1`) / photon (`ZAP = 0`)
//!   subsection, duplicates dropped (`:11554-11583`).
//! - `mf12` (MT=460 dropped, `:11010`) and `mf13`.
//! - the MF=8 production entries `(LMF, MT, 10 ZAP, LFS, isomer count)`
//!   (`:11733-11766`), for MTs up to 200 and from 600 on.
//!
//! `nextr` then hands them out: `mfd = 6` takes `mf4` while `|mt| <= 91`, then
//! switches to `mfd = 8` (MF=6 neutrons, `getmf6`) over `mf6`; `mfd = 16` takes
//! `mf12`, then `mfd = 17` over `mf13`, then `mfd = 18` over `mf18`; `mfd = 10`
//! takes the MF=8 entries as `mfd = 10 000 000 * k + 10 ZAP` (`k` = 1, 2, 3, 4
//! for LMF 3, 6, 9, 10). A negative entry `-m` after `mtd` visits
//! `mtd + 1, ..., m` (`:640-646`, `:731-732`).

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// One reaction GROUPR will process: the `mfd` it is processed as, its `mtd`,
/// and for a production reaction (`mfd >= 10 000 000`) `lfs`, `isom` and the
/// `izam`/`izar` the main loop derives (`:688-711`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoReaction {
    /// `mfd` as `getff` sees it (`8` and `18` for MF=6 neutrons and photons).
    pub mfd: i32,
    /// `mtd`.
    pub mtd: i32,
    /// `lfs` (MF=8 `LFS`), 0 otherwise.
    pub lfs: i32,
    /// `isom`, the isomer count of `conver` (`mlfs8`), 0 otherwise.
    pub isom: i32,
    /// Read from card 9a (`mfd = -1`, `groupr.f90:655-676`): `izam` and the
    /// head's C2 are built from `izar` and `lfs` there ([`Self::head_c2`]).
    pub card9a: bool,
}

impl AutoReaction {
    /// A plain `(mfd, mtd)`.
    pub fn plain(mfd: i32, mtd: i32) -> Self {
        Self { mfd, mtd, lfs: 0, isom: 0, card9a: false }
    }

    /// `izam` (`:705-711`): the product written in the section head's C2;
    /// `-1` for the fission special case `mfd = 40 000 000`.
    pub fn izam(&self) -> i32 {
        if self.mfd < 10_000_000 {
            return 0;
        }
        if self.mfd == 40_000_000 {
            return -1;
        }
        let m = self.mfd % 10_000_000;
        if self.lfs < 10 {
            m + self.lfs
        } else {
            10 * m + self.lfs
        }
    }

    /// The production section head's C2 (`:871-878`): `scr(2) = izam`, then
    /// `fzam` when it is non-zero. Only card 9a sets `fzam`, to
    /// `real(izar) + real(lfs)/1000` (`:674`), the `ZZZAAA.SSS` form; every
    /// other request leaves it 0 (`:620`) and writes `izam`.
    pub fn head_c2(&self) -> f64 {
        if self.card9a {
            let fzam = f64::from(self.izar()) + f64::from(self.lfs) / 1000.0;
            if fzam != 0.0 {
                return fzam;
            }
            return f64::from(self.izar() * 1000 + self.lfs);
        }
        f64::from(self.izam())
    }

    /// `izar` (`:704-708`): the product `ZA`, `-1` for `mfd = 40 000 000`.
    pub fn izar(&self) -> i32 {
        if self.mfd == 40_000_000 {
            -1
        } else {
            (self.mfd % 10_000_000) / 10
        }
    }
}

/// One MF=8 production entry of `conver` (`mf10f, mf10s, mf10i, lfs8, mlfs8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mf8Entry {
    lmf: i32,
    mt: i32,
    izan10: i32,
    lfs: i32,
    isom: i32,
}

/// The lists `conver` leaves for `nextr`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConverLists {
    /// MF=4 MTs (range-compressed).
    pub mf4: Vec<i32>,
    /// MF=6 MTs with a neutron subsection.
    pub mf6: Vec<i32>,
    /// MF=12 MTs (MT=460 dropped).
    pub mf12: Vec<i32>,
    /// MF=13 MTs.
    pub mf13: Vec<i32>,
    /// MF=6 MTs with a photon subsection.
    pub mf18: Vec<i32>,
    mf8: Vec<Mf8Entry>,
}

/// `itest` form (`mf12`, `mf13`, `mf4r`, `:11203-11230`): `mth <= |last| + 1`
/// extends the range.
fn push_range(list: &mut Vec<i32>, mth: i32) {
    match list.last().copied() {
        Some(last) if mth <= last.abs() + 1 => {
            if last < 0 {
                list.pop();
            }
            list.push(-mth);
        }
        _ => list.push(mth),
    }
}

/// MF=6 form (`:11554-11583`): a repeat of the last MT is dropped, `|last| + 1`
/// extends the range.
fn push_range_dedup(list: &mut Vec<i32>, mth: i32) {
    match list.last().copied() {
        Some(last) if mth == last.abs() => {}
        Some(last) if mth <= last.abs() + 1 => {
            if last < 0 {
                list.pop();
            }
            list.push(-mth);
        }
        _ => list.push(mth),
    }
}

/// MF=4 form (`:10994-11003`).
fn push_mf4(list: &mut Vec<i32>, mth: i32) {
    let Some(last) = list.last().copied() else {
        list.push(mth);
        return;
    };
    if mth == 18 {
        list.push(mth);
    } else if mth == 19 && last == 18 {
        // label 311: the 18 is overwritten
        list.pop();
        list.push(mth);
    } else if mth > last.abs() + 1 {
        list.push(mth);
    } else {
        if last < 0 {
            list.pop();
        }
        list.push(-mth);
    }
}

/// `conver`'s lists for material `mat` of the evaluation `nend`.
///
/// # Errors
/// A malformed MF=6 or MF=8 section.
pub fn conver_lists(nend: &Tape, mat: i32) -> Result<ConverLists, NjoyError> {
    let mut l = ConverLists::default();
    let mut lastza = 0i32;
    for s in nend.sections().iter().filter(|s| s.key.mat == mat) {
        let (mf, mt) = (s.key.mf, s.key.mt);
        match mf {
            4 => push_mf4(&mut l.mf4, mt),
            12 if mt != 460 => push_range(&mut l.mf12, mt),
            13 => push_range(&mut l.mf13, mt),
            6 => {
                let mut cur = SectionCursor::new(&s.rows);
                let head = cur.read_cont()?;
                // `:10985-10990`: the new MT=18 multiplicity form is skipped.
                if mt == 18 && head.l1 != 0 {
                    continue;
                }
                for _ in 0..head.n1.max(0) {
                    let t = cur.read_tab1()?;
                    let jzap = t.head.c1.round() as i32;
                    crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, t.head.l2)?;
                    match jzap {
                        0 => push_range_dedup(&mut l.mf18, mt),
                        1 => push_range_dedup(&mut l.mf6, mt),
                        _ => {}
                    }
                }
            }
            8 if mt <= 200 || mt >= 600 => {
                let mut cur = SectionCursor::new(&s.rows);
                let head = cur.read_cont()?;
                for _ in 0..head.n1.max(0) {
                    let r = cur.read_list()?;
                    let izan = r.head.c1.round() as i32;
                    let lfs = r.head.l2;
                    let izan10 = if izan == -1 && mt == 18 { 0 } else { 10 * izan };
                    let isom = if izan != lastza {
                        i32::from(lfs != 0)
                    } else {
                        l.mf8.last().map_or(0, |e| e.isom) + 1
                    };
                    lastza = izan;
                    l.mf8.push(Mf8Entry { lmf: r.head.l1, mt, izan10, lfs, isom });
                }
            }
            _ => {}
        }
    }
    Ok(l)
}

/// Expand a range-compressed list from `mtd` (the last MT visited): a negative
/// entry `-m` visits `mtd + 1 ..= m` (at least once, as `:731-732` does).
fn expand(list: &[i32], mtd: &mut i32, mfd: i32, out: &mut Vec<AutoReaction>) {
    for &m in list {
        if m >= 0 {
            *mtd = m;
            out.push(AutoReaction::plain(mfd, m));
        } else {
            loop {
                *mtd += 1;
                out.push(AutoReaction::plain(mfd, *mtd));
                if *mtd + m >= 0 {
                    break;
                }
            }
        }
    }
}

/// `nextr` for an automatic card 9 of file `mfd` (`mtd` absent), every reaction
/// in turn. `mtd` is the main loop's running MT (1 before the first card).
///
/// `mfd <= 3` reads the PENDF's own MF=3 sections for this temperature, skipping
/// `MT = 201-202, 208-300` (`:1155-1161`).
///
/// # Errors
/// [`NjoyError::NotPorted`] for the charged-particle lists (`mfd = 21-26`).
pub fn nextr_all(mfd: i32, lists: &ConverLists, pend: &Tape, mat: i32, mtd: &mut i32) -> Result<Vec<AutoReaction>, NjoyError> {
    let mut out = Vec::new();
    match mfd {
        ..=3 => {
            for s in pend.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3) {
                let m = s.key.mt;
                if m <= 200 || (203..=207).contains(&m) || m > 300 {
                    *mtd = m;
                    out.push(AutoReaction::plain(3, m));
                }
            }
        }
        6 => {
            let cut = lists.mf4.iter().position(|&m| m.abs() > 91).unwrap_or(lists.mf4.len());
            expand(&lists.mf4[..cut], mtd, 6, &mut out);
            expand(&lists.mf6, mtd, 8, &mut out);
        }
        8 => expand(&lists.mf6, mtd, 8, &mut out),
        16 | 17 | 18 => {
            if mfd == 16 {
                expand(&lists.mf12, mtd, 16, &mut out);
            }
            if mfd <= 17 {
                expand(&lists.mf13, mtd, 17, &mut out);
            }
            expand(&lists.mf18, mtd, 18, &mut out);
        }
        10 => {
            for e in &lists.mf8 {
                if e.lmf == 0 {
                    break;
                }
                let base = match e.lmf {
                    3 => 10_000_000,
                    6 => 20_000_000,
                    9 => 30_000_000,
                    10 => 40_000_000,
                    _ => return Err(NjoyError::NotPorted("groupr auto mode: MF=8 LMF other than 3, 6, 9 and 10")),
                };
                *mtd = e.mt;
                out.push(AutoReaction { mfd: base + e.izan10, mtd: e.mt, lfs: e.lfs, isom: e.isom, card9a: false });
            }
        }
        _ => return Err(NjoyError::NotPorted("groupr auto mode for this mfd (charged-particle lists)")),
    }
    Ok(out)
}

/// `getyld` (`:6521-6644`) for a production reaction, `mfd` in
/// `[20 000 000, 40 000 000)`: the packed yield TAB1 of the MF=6 (`ZAP = izan`,
/// `LIP = lfs`) or MF=9 (`IZAP = izan` or 0, `LFS = lfs`) subsection, for `terpa`.
///
/// `lfs` is `isom` for MF=6 and `lfs` for MF=9, as the call sites pass them
/// (`getff` `:7085-7088`).
///
/// # Errors
/// `'unable to find nuclide'` (`:6612-6614`) or a missing section.
pub fn product_yield(nend: &Tape, mat: i32, mfd: i32, mt: i32, lfs: i32) -> Result<Vec<f64>, NjoyError> {
    let mft = if mfd >= 30_000_000 { 9 } else { 6 };
    let iza = (mfd / 10) % 1_000_000;
    let sec = nend
        .section(mat, mft, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf: mft, mt })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    for _ in 0..head.n1.max(0) {
        let t = cur.read_tab1()?;
        let (izn, lfn) = if mft == 6 { (t.head.c1.round() as i32, t.head.l1) } else { (t.head.l1, t.head.l2) };
        if (iza == izn && lfs == lfn) || (mft > 6 && izn == 0 && lfs == lfn) {
            return Ok(crate::groupr::continuum::pack(&t));
        }
        if mft == 6 {
            crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, t.head.l2)?;
        }
    }
    Err(NjoyError::InvalidInput(format!("getyld: unable to find nuclide for iza={iza} lfs={lfs}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mf4_merges_ranges_and_replaces_18_by_19() {
        let mut l = Vec::new();
        for m in [2, 16, 17, 18, 19, 20, 21, 51, 52, 53, 91] {
            push_mf4(&mut l, m);
        }
        assert_eq!(l, vec![2, 16, -17, 19, -21, 51, -53, 91]);
        let (mut mtd, mut out) = (1, Vec::new());
        expand(&l, &mut mtd, 6, &mut out);
        let mts: Vec<i32> = out.iter().map(|r| r.mtd).collect();
        assert_eq!(mts, vec![2, 16, 17, 19, 20, 21, 51, 52, 53, 91]);
    }

    #[test]
    fn mf6_list_drops_repeats() {
        let mut l = Vec::new();
        for m in [16, 16, 17, 22, 22, 28] {
            push_range_dedup(&mut l, m);
        }
        assert_eq!(l, vec![16, -17, 22, 28]);
    }

    #[test]
    fn production_izam_and_izar() {
        let r = AutoReaction { mfd: 20_000_000 + 10 * 76183, mtd: 5, lfs: 2, isom: 1, card9a: false };
        assert_eq!((r.izam(), r.izar()), (761832, 76183));
        let f = AutoReaction { mfd: 40_000_000, mtd: 18, lfs: 0, isom: 0, card9a: false };
        assert_eq!((f.izam(), f.izar()), (-1, -1));
    }
}
