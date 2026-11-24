// Translated from NJOY2016.80 `src/acefc.f90`, `subroutine tabize` (`:813-1059`),
// the branch that linearises a tabulated (`LNU = 2`) MF=1 nubar section.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **`tabize`'s linearise branch** (gh:#1372): ACER's `first` (`acefc.f90:510-525`)
//! passes every MF=1 MT=452/455/456 section through `tabize` on its way to the
//! scratch tape `acelod` reads. A polynomial (`LNU = 1`) and a tabulated section
//! whose ranges are all `INT <= 2` are copied; a tabulated one with any range of
//! `INT > 2` is **linearised** to a single lin-lin range, and `acelod` then writes
//! the NU/DNU block with `NR = 0`.
//!
//! [`tabize_tape`] does that to the evaluation before the NU and DNU blocks read
//! it, so [`crate::acer::nu`] and [`crate::acer::delayed_blocks`] are unchanged.
//!
//! Upstream's own behaviour is kept, including what reads as odd:
//!
//! - a midpoint that fails the test is pushed with its **lin-lin** value `ym`,
//!   not the interpolated `yy` (`y(i-1)=ym`), so later subdivisions interpolate
//!   from that point;
//! - `test = err*ym` uses the `ym` last computed even when the midpoint was not
//!   strictly inside the panel (then `dy = 0` and the panel converges anyway);
//! - the stack holds `ndim = 100` points, and a full stack converges;
//! - the walk stops at the first point equal to `xmax = 2e7` eV;
//! - the new TAB1's `C1, C2, L1, L2` are written as 0 (`scr2(1:4) = 0`);
//! - the linearised points go to ACER's scratch tape `mscr`, which is text when
//!   `npend > 0` (`acefc.f90:178-180`), so `acelod` reads them back at `a11`'s
//!   precision; on a binary `npend` they keep every bit.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::NjoyError;

/// `first`'s `err` (`acefc.f90:274`, `err=.01`), the fractional tolerance of
/// `tabize`'s linearisation.
pub const TABIZE_ERR: f64 = 0.01;
/// `tabize`'s `xmax` (20 MeV): the walk ends at a point equal to it.
const XMAX: f64 = 2.0e7;
/// `tabize`'s `errlim`: the absolute floor of the test.
const ERRLIM: f64 = 1.0e-5;
/// `tabize`'s `ndim`: the stack depth.
const NDIM: usize = 100;

/// `tabize`'s linearisation (`acefc.f90:931-1012`) of a TAB1 with ranges
/// `interp` (`(NBT, INT)`) and points `pairs`. `None` when every range has
/// `INT <= 2` (`nonlin = 0`: upstream copies the section as it is).
///
/// # Errors
/// [`terp1`]'s, for a log law on a non-positive value.
pub fn tabize_linearise(interp: &[(u32, u32)], pairs: &[(f64, f64)], err: f64) -> Result<Option<Vec<(f64, f64)>>, NjoyError> {
    if !interp.iter().any(|&(_, int)| int > 2) || pairs.is_empty() || interp.is_empty() {
        return Ok(None);
    }
    // 1-based stacks as upstream's `x(ndim)`, `y(ndim)`; index 0 unused.
    let mut x = [0.0f64; NDIM + 1];
    let mut y = [0.0f64; NDIM + 1];
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut ir = 0usize;
    let mut inn = 1usize;
    let mut nbt = interp[ir].0 as usize;
    let mut int = interp[ir].1;
    x[2] = pairs[0].0;
    y[2] = pairs[0].1;
    // `ym` is a Fortran local that keeps its last value between panels.
    let mut ym = 0.0f64;
    loop {
        let mut i = 2usize;
        // `ia=ia+2; if (ia.gt.nw.and.nb.eq.0)`: no point left.
        if inn >= pairs.len() {
            break;
        }
        inn += 1;
        if inn > nbt && ir + 1 < interp.len() {
            ir += 1;
            nbt = interp[ir].0 as usize;
            int = interp[ir].1;
        }
        x[1] = pairs[inn - 1].0;
        y[1] = pairs[inn - 1].1;
        while i > 1 {
            let mut dy = 0.0;
            let xm = crate::mixr::mix::sigfig((x[i] + x[i - 1]) / 2.0, 7, 0);
            if xm > x[i] && xm < x[i - 1] {
                let yy = terp1(x[i], y[i], x[i - 1], y[i - 1], xm, IntLaw::from_code(int))?;
                ym = terp1(x[i], y[i], x[i - 1], y[i - 1], xm, IntLaw::LinLin)?;
                dy = (yy - ym).abs();
            }
            if i == NDIM {
                dy = 0.0;
            }
            let mut test = err * ym;
            if test < ERRLIM {
                test = ERRLIM;
            }
            if dy > test {
                // Not converged: the midpoint goes on the stack.
                i += 1;
                x[i] = x[i - 1];
                y[i] = y[i - 1];
                x[i - 1] = xm;
                y[i - 1] = ym;
            } else {
                // Converged: the top point comes off the stack.
                out.push((x[i], y[i]));
                i -= 1;
            }
        }
        let last = x[1] == XMAX;
        x[2] = x[1];
        y[2] = y[1];
        if last {
            break;
        }
    }
    out.push((x[1], y[1]));
    Ok(Some(out))
}

/// Six words to a row, the last row padded with zeros.
fn rows_of_words(words: &[f64]) -> Vec<[f64; 6]> {
    words
        .chunks(6)
        .map(|c| {
            let mut r = [0.0; 6];
            r[..c.len()].copy_from_slice(c);
            r
        })
        .collect()
}

/// The evaluation as `first`'s `tabize` leaves it for `acelod`: each MF=1
/// MT=452/455/456 section of `mat` with a tabulated `nu` that has a range of
/// `INT > 2` rewritten as one lin-lin TAB1 (C1 = C2 = L1 = L2 = 0, `NR = 1`,
/// `NBT = NP`, `INT = 2`), its points read back through `a11` when `text` (a
/// coded `npend`). `None` when no section needs it, so the caller reads the
/// evaluation unchanged.
///
/// # Errors
/// A section whose records do not parse; [`tabize_linearise`]'s.
pub fn tabize_tape(tape: &Tape, mat: i32, err: f64, text: bool) -> Result<Option<Tape>, NjoyError> {
    let mut changed = false;
    let mut sections: Vec<Section> = Vec::with_capacity(tape.sections().len());
    for s in tape.sections() {
        let k = s.key;
        if k.mat != mat || k.mf != 1 || !matches!(k.mt, 452 | 455 | 456) {
            sections.push(s.clone());
            continue;
        }
        let mut cur = SectionCursor::new(&s.rows);
        let head = cur.read_cont()?;
        if head.l2 != 2 {
            sections.push(s.clone());
            continue;
        }
        if k.mt == 455 {
            cur.read_list()?;
        }
        let keep = cur.position();
        let tab = cur.read_tab1()?;
        let Some(lin) = tabize_linearise(&tab.interp, &tab.pairs, err)? else {
            sections.push(s.clone());
            continue;
        };
        let back = |v: f64| if text { crate::endf::parse::parse_endf_float(&crate::endf::parse::format_endf_float(v)) } else { Ok(v) };
        let np = lin.len() as f64;
        let mut rows: Vec<[f64; 6]> = s.rows[..keep].to_vec();
        // `scr2(4)` holds L2 = 0 and `b(4)=2` the HEAD's LNU, as upstream writes them.
        rows.push([0.0, 0.0, 0.0, 0.0, 1.0, np]);
        rows.push([np, 2.0, 0.0, 0.0, 0.0, 0.0]);
        let mut words = Vec::with_capacity(2 * lin.len());
        for (e, v) in lin {
            words.push(back(e)?);
            words.push(back(v)?);
        }
        rows.extend(rows_of_words(&words));
        sections.push(Section { key: k, rows });
        changed = true;
    }
    if !changed {
        return Ok(None);
    }
    let mut out = Tape::from_sections(tape.tpid.clone(), sections);
    out.copy_raw_mf32_from(tape);
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An all-linear table is copied (`nonlin = 0`).
    #[test]
    fn a_lin_lin_table_is_left_alone() {
        let lin = tabize_linearise(&[(3, 2)], &[(1.0, 2.0), (2.0, 2.5), (3.0, 3.0)], TABIZE_ERR).unwrap();
        assert!(lin.is_none());
        let hist = tabize_linearise(&[(3, 1)], &[(1.0, 2.0), (2.0, 2.5), (3.0, 3.0)], TABIZE_ERR).unwrap();
        assert!(hist.is_none(), "INT = 1 is not linearised upstream either");
    }

    /// A log-log panel gains points between its ends, which are kept.
    #[test]
    fn a_log_log_panel_is_subdivided() {
        let pairs = [(1.0e-5, 2.4), (2.0e7, 5.0)];
        let lin = tabize_linearise(&[(2, 5)], &pairs, TABIZE_ERR).unwrap().unwrap();
        assert!(lin.len() > 2);
        assert_eq!(lin.first(), Some(&pairs[0]));
        assert_eq!(lin.last(), Some(&pairs[1]));
        assert!(lin.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
