// SPDX-License-Identifier: AGPL-3.0-only
// The MF=32 walk follows NJOY2016.80 `src/moder.f90` (`file32`, lines 1394-1560)
// and the INTG card formats of `src/endf.f90` (`intgio`, lines 721-836).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Strict reading of the two sections whose lines are not all `e11.0`
//! records (gh:#660): MF=1/MT=451 (description text, `hdatio`) and MF=32
//! (compact `LCOMP = 2` correlations, `intgio`).
//!
//! [`Tape`](super::tape::Tape) reads every other card strictly as it reads it.
//! For these two it reads leniently, remembers the cards a strict read
//! refused, and at the end of the section walks the section's structure, as
//! NJOY's MODER walks it, to decide which cards were legitimately not
//! numeric: [`check_raw_section`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::endf::parse::parse_fortran_int;
use crate::endf::EndfKey;
use crate::moder::layout::{is_group_material, iverf_from_mf1, section_layout, LineKind};
use crate::NjoyError;

/// A card a strict `e11.0` read refused: its row in the section, its line on
/// the tape, and why.
pub(crate) struct Refused {
    pub(crate) row: usize,
    pub(crate) lineno: usize,
    pub(crate) why: String,
}

/// How NJOY reads one MF=32 card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card32 {
    /// `contio` / `listio` / `moreio`: `(6e11.0,i4,i2,i3,i5)`.
    Numeric,
    /// `intgio` with this `NDIGIT`.
    Intg(i64),
}

fn ni(x: f64) -> i64 {
    x.round() as i64
}

/// Walks an MF=32 section's rows as `moder.f90`'s `file32` reads them.
struct Walk32<'a> {
    rows: &'a [[f64; 6]],
    pos: usize,
    cards: Vec<Card32>,
}

impl Walk32<'_> {
    fn cont(&mut self) -> Option<[f64; 6]> {
        let r = *self.rows.get(self.pos)?;
        self.cards.push(Card32::Numeric);
        self.pos += 1;
        Some(r)
    }

    /// `listio` and its `moreio` continuation: the head, then `NPL` words six
    /// to a card.
    fn list(&mut self) -> Option<[f64; 6]> {
        let h = self.cont()?;
        let npl = ni(h[4]).max(0);
        for _ in 0..(npl + 5) / 6 {
            self.cont()?;
        }
        Some(h)
    }

    /// The compact block's CONT (`NDIGIT` in `L1`, the card count in `N1`)
    /// and its `intgio` cards. An illegal `NDIGIT` is `None`: MODER stops there
    /// (`error('file32','illegal value of ndigit')`), and this walk cannot say
    /// how the cards are laid out.
    fn intg_block(&mut self) -> Option<()> {
        let c = self.cont()?;
        let nn = ni(c[4]).max(0);
        let ndigit = match ni(c[2]) {
            0 => 2,
            d @ 2..=6 => d,
            _ => return None,
        };
        for _ in 0..nn {
            self.rows.get(self.pos)?;
            self.cards.push(Card32::Intg(ndigit));
            self.pos += 1;
        }
        Some(())
    }

    /// `file32` (`moder.f90:1394-1560`), `h` the section HEAD.
    fn file32(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            let iso = self.cont()?;
            for _ in 0..ni(iso[4]) {
                let r = self.cont()?;
                let (lru, lrf, nro) = (ni(r[2]), ni(r[3]), ni(r[4]));
                if nro > 0 {
                    let c = self.cont()?;
                    for _ in 0..ni(c[5]) {
                        self.list()?;
                    }
                }
                let c = self.cont()?;
                let (lcomp, nls, isr) = (ni(c[3]), ni(c[4]), ni(c[5]));
                if lrf != 7 {
                    if isr != 0 {
                        if isr == 1 && lrf <= 2 {
                            self.cont()?;
                        } else if isr == 1 && lrf == 3 {
                            self.list()?;
                        } else {
                            return None;
                        }
                    }
                    if lru == 2 {
                        for _ in 0..nls {
                            self.list()?;
                        }
                        self.list()?;
                    } else if lcomp == 0 {
                        for _ in 0..nls {
                            self.list()?;
                        }
                    } else if lcomp == 1 {
                        let c = self.cont()?;
                        let (nsrs, nlrs) = (ni(c[4]), ni(c[5]));
                        for _ in 0..nsrs {
                            self.list()?;
                        }
                        if nlrs > 0 {
                            for _ in 0..nsrs {
                                self.list()?;
                            }
                        }
                    } else if lcomp == 2 {
                        self.list()?;
                        self.intg_block()?;
                    } else {
                        return None;
                    }
                } else if isr != 0 {
                    return None;
                } else if lcomp == 1 {
                    let c = self.cont()?;
                    for _ in 0..ni(c[4]) {
                        let g = self.cont()?;
                        for _ in 0..ni(g[2]) {
                            self.list()?;
                        }
                    }
                    self.list()?;
                } else if lcomp == 2 {
                    let njs = nls;
                    self.list()?;
                    for _ in 0..njs {
                        self.list()?;
                        self.list()?;
                    }
                    self.intg_block()?;
                } else {
                    return None;
                }
            }
        }
        Some(())
    }
}

/// How each row of an MF=32 section is read, or `None` when the walk does not
/// account for exactly all its rows.
fn mf32_cards(rows: &[[f64; 6]]) -> Option<Vec<Card32>> {
    let mut w = Walk32 { rows, pos: 0, cards: Vec::with_capacity(rows.len()) };
    let h = w.cont()?;
    w.file32(h)?;
    (w.pos == rows.len()).then_some(w.cards)
}

/// The integer columns `intgio` reads on an INTG card for `ndigit`
/// (`endf.f90:751-765`), as `(start, width)`, 0-based: `2i5,1x` and then
/// `18i3` / `13i4` / `11i5` / `9i6` / `8i7`.
fn intg_columns(ndigit: i64) -> Vec<(usize, usize)> {
    let (n, w) = match ndigit {
        2 => (18, 3),
        3 => (13, 4),
        4 => (11, 5),
        5 => (9, 6),
        _ => (8, 7),
    };
    let mut c = vec![(0, 5), (5, 5)];
    c.extend((0..n).map(|k| (11 + k * w, w)));
    c
}

/// Check one INTG card's integer columns as `intgio` reads them.
fn check_intg(line: &str, ndigit: i64) -> Result<(), String> {
    let chars: Vec<char> = line.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect();
    for (k, (a, w)) in intg_columns(ndigit).into_iter().enumerate() {
        let s: String = (a..a + w).map(|i| chars.get(i).copied().unwrap_or(' ')).collect();
        if parse_fortran_int(&s).is_none() {
            return Err(format!(
                "INTG (NDIGIT={ndigit}) integer {} ({s:?}, columns {}-{}) is not an integer; \
                 NJOY2016 stops here (intgio: bad value during integer read)",
                k + 1,
                a + 1,
                a + w
            ));
        }
    }
    Ok(())
}

fn refusal(key: EndfKey, lineno: usize, why: &str) -> NjoyError {
    NjoyError::EndfParse(format!("line {lineno} (MAT {} MF {} MT {}): {why}", key.mat, key.mf, key.mt))
}

/// Judge a raw-kept section (MF=1/MT=451 or MF=32) at its end (gh:#660).
///
/// - **MF=1/MT=451:** a refused card is legal only where MODER's walk
///   (`moder::layout`, `file1`) puts a text line. A GENDF/ERRORR material has
///   no text there (its title is the TPID), so every refusal stands.
/// - **MF=32:** a refused card is legal only where `file32`'s walk puts an
///   INTG card, and every INTG card's integer columns are checked as `intgio`
///   reads them; `full` holds the section's whole lines, `first_lineno` the
///   tape line of its first row.
///
/// When the walk cannot account for the section's rows, the cards cannot be
/// classified and the section is read as before, leniently (counted on the
/// held tapes in gh:#660's record: none).
pub(crate) fn check_raw_section(
    key: EndfKey,
    rows: &[[f64; 6]],
    refused: &[Refused],
    full: &[String],
    first_lineno: usize,
) -> Result<(), NjoyError> {
    if key.mf == 1 && key.mt == 451 {
        if refused.is_empty() {
            return Ok(());
        }
        if is_group_material(rows) {
            let r = &refused[0];
            return Err(refusal(key, r.lineno, &r.why));
        }
        let Some(kinds) = section_layout(1, 451, rows, iverf_from_mf1(rows)) else {
            return Ok(());
        };
        if let Some(r) = refused.iter().find(|r| kinds.get(r.row) != Some(&LineKind::Text)) {
            return Err(refusal(key, r.lineno, &r.why));
        }
        return Ok(());
    }
    if key.mf == 32 {
        let Some(cards) = mf32_cards(rows) else {
            return Ok(());
        };
        if let Some(r) = refused.iter().find(|r| cards.get(r.row) == Some(&Card32::Numeric)) {
            return Err(refusal(key, r.lineno, &r.why));
        }
        for (row, card) in cards.iter().enumerate() {
            if let Card32::Intg(nd) = *card {
                let line = full.get(row).map_or("", String::as_str);
                check_intg(line, nd).map_err(|why| refusal(key, first_lineno + row, &why))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intg_columns_follow_intgio() {
        // ndigit 2: 2i5,1x,18i3 ends at column 65; ndigit 6: 8i7 ends at 67.
        let c2 = intg_columns(2);
        assert_eq!((c2.len(), c2.last().map(|&(a, w)| a + w)), (20, Some(65)));
        let c6 = intg_columns(6);
        assert_eq!((c6.len(), c6.last().map(|&(a, w)| a + w)), (10, Some(67)));
        // Cu-65 ENDF/B-VIII.0 line 50331, a real ndigit = 2 card.
        assert!(check_intg("    5    1   1 -2    -2                                           293132151", 2).is_ok());
        assert!(check_intg("    5    1   1 -2    -x                                           293132151", 2).is_err());
        assert!(check_intg("    5    1  -                                                     293132151", 2).is_err());
    }
}
