// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/endf.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `contio` l.37-111, `listio` l.113-207, `tab1io` l.209-340, `tab2io` l.342-421,
//   `moreio` l.423-509, `tpidio` l.511-565, `tosend` l.1064-1116, `tomend` l.1172-1224,
//   `findf` l.1477-1587 (the input halves, coded and blocked-binary alike).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The ENDF tape as PLOTR reads it: one line at a time, with `npage` paging.
//!
//! PLOTR walks its input tapes with NJOY's record routines and positions them
//! with `findf`, which reads up **and down** a card at a time (`skiprz`). So the
//! tape is kept here as its lines, rebuilt from the unit's [`Tape`] with the
//! SEND, FEND, MEND and TEND cards NJOY wrote between its sections, and a read
//! position. A record that `listio`/`tab1io` page (more than `npage = 306`
//! words) leaves its later lines unread until `moreio` asks for them, as
//! upstream does.
//!
//! The record routines set `endf.f90`'s module variables (`math`, `mfh`, `mth`,
//! `c1h` ... `n2h`); PLOTR reads them after a call, so they are a [`Head`] the
//! caller keeps, shared by every tape, as upstream's are. Records go into the
//! caller's work array at a 1-based Fortran index, `a[at - 1]` being `a(at)`.

use crate::endf::parse::parse_line;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// `npage` (`endf.f90:19`): words per page of a LIST or TAB1.
pub const NPAGE: usize = 306;

/// `endf.f90`'s module variables set by each record read.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Head {
    /// `math`
    pub math: i32,
    /// `mfh`
    pub mfh: i32,
    /// `mth`
    pub mth: i32,
    /// `c1h`
    pub c1h: f64,
    /// `c2h`
    pub c2h: f64,
    /// `l1h`
    pub l1h: i32,
    /// `l2h`
    pub l2h: i32,
    /// `n1h`
    pub n1h: i32,
    /// `n2h`
    pub n2h: i32,
}

#[derive(Debug, Clone, Copy)]
struct Line {
    mat: i32,
    mf: i32,
    mt: i32,
    f: [f64; 6],
}

/// One tape's lines and its read position.
#[derive(Debug, Clone)]
pub struct EndfStream {
    lines: Vec<Line>,
    pos: usize,
}

fn eof() -> NjoyError {
    NjoyError::EndfParse("plotr: end of file on an ENDF input tape".into())
}

/// Fortran `nint`.
pub fn nint(x: f64) -> i32 {
    x.round() as i32
}

impl EndfStream {
    /// The tape's lines in the order NJOY wrote them: the TPID, each section
    /// followed by its SEND, a FEND after a file's last section, a MEND after a
    /// material (or before the next MF=1/MT=451 of the same MAT, a further
    /// temperature on a PENDF), and the TEND.
    pub fn from_tape(tape: &Tape) -> Self {
        let mut lines = Vec::new();
        let t = parse_line(&tape.tpid).ok();
        lines.push(Line {
            mat: t.as_ref().map_or(0, |r| r.mat),
            mf: 0,
            mt: 0,
            f: [0.0; 6],
        });
        let secs = tape.sections();
        for (k, s) in secs.iter().enumerate() {
            let (mat, mf, mt) = (s.key.mat, s.key.mf, s.key.mt);
            for r in &s.rows {
                lines.push(Line { mat, mf, mt, f: *r });
            }
            lines.push(Line {
                mat,
                mf,
                mt: 0,
                f: [0.0; 6],
            });
            let next = secs.get(k + 1).map(|n| n.key);
            let new_mat = next.is_none_or(|n| n.mat != mat || (n.mf == 1 && n.mt == 451));
            if new_mat || next.is_some_and(|n| n.mf != mf) {
                lines.push(Line {
                    mat,
                    mf: 0,
                    mt: 0,
                    f: [0.0; 6],
                });
            }
            if new_mat {
                lines.push(Line {
                    mat: 0,
                    mf: 0,
                    mt: 0,
                    f: [0.0; 6],
                });
            }
        }
        lines.push(Line {
            mat: -1,
            mf: 0,
            mt: 0,
            f: [0.0; 6],
        });
        Self { lines, pos: 0 }
    }

    /// The tape's lines exactly as they stand in `text`, markers included
    /// (gh:#1373, POWR). A GENDF ends its MF=1 section with a FEND and every
    /// later section with a SEND and no FEND, which [`Self::from_tape`] does
    /// not lay out; a reader that positions by cards (`skiprz`, `findf`) needs
    /// the cards upstream wrote. The first line is the label (`tpidio` reads
    /// only its MAT, MF and MT).
    ///
    /// # Errors
    /// A numeric field after the label that does not parse.
    pub fn from_text(text: &str) -> Result<Self, NjoyError> {
        let mut lines = Vec::new();
        for (k, t) in text.lines().enumerate() {
            if k == 0 {
                let col = |a: usize, b: usize| {
                    t.get(a..b.min(t.len())).and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0)
                };
                lines.push(Line { mat: col(66, 70), mf: col(70, 72), mt: col(72, 75), f: [0.0; 6] });
                continue;
            }
            let r = parse_line(t)?;
            lines.push(Line { mat: r.mat, mf: r.mf, mt: r.mt, f: r.fields });
        }
        Ok(Self { lines, pos: 0 })
    }

    /// `skiprz(nin, -n)`: back up `n` cards (gh:#1373, POWR's `gamxs`).
    pub fn skiprz_back(&mut self, n: usize) {
        self.back(n);
    }

    /// `repoz`: rewind.
    pub fn rewind(&mut self) {
        self.pos = 0;
    }

    fn next_line(&mut self) -> Result<Line, NjoyError> {
        let l = *self.lines.get(self.pos).ok_or_else(eof)?;
        self.pos += 1;
        Ok(l)
    }

    /// `skiprz(n)` for `n < 0`: back up `-n` cards.
    fn back(&mut self, n: usize) {
        self.pos = self.pos.saturating_sub(n);
    }

    /// `tpidio(nin,0,0,a,nb,nw)`: the tape label (its MAT goes to `math`).
    pub fn tpidio(&mut self, h: &mut Head) -> Result<(), NjoyError> {
        let l = self.next_line()?;
        h.math = l.mat;
        h.mfh = l.mf;
        h.mth = l.mt;
        Ok(())
    }

    /// `contio`: one control record into `a(at..at+5)`.
    pub fn contio(&mut self, h: &mut Head, a: &mut [f64], at: usize) -> Result<(), NjoyError> {
        let l = self.next_line()?;
        h.math = l.mat;
        h.mfh = l.mf;
        h.mth = l.mt;
        h.c1h = l.f[0];
        h.c2h = l.f[1];
        h.l1h = nint(l.f[2]);
        h.l2h = nint(l.f[3]);
        h.n1h = nint(l.f[4]);
        h.n2h = nint(l.f[5]);
        a[at - 1] = l.f[0];
        a[at] = l.f[1];
        a[at + 1] = f64::from(h.l1h);
        a[at + 2] = f64::from(h.l2h);
        a[at + 3] = f64::from(h.n1h);
        a[at + 4] = f64::from(h.n2h);
        Ok(())
    }

    /// `n` words from successive lines into `a(at..)`, six per line.
    fn words(&mut self, a: &mut [f64], at: usize, n: usize) -> Result<(), NjoyError> {
        let mut k = 0;
        while k < n {
            let l = self.next_line()?;
            for j in 0..6.min(n - k) {
                a[at - 1 + k] = l.f[j];
                k += 1;
            }
        }
        Ok(())
    }

    /// `listio`: the head and first page of a LIST. Returns `(nb, nw)`.
    pub fn listio(
        &mut self,
        h: &mut Head,
        a: &mut [f64],
        at: usize,
    ) -> Result<(usize, usize), NjoyError> {
        self.contio(h, a, at)?;
        let n1 = h.n1h.max(0) as usize;
        let iend = n1.min(NPAGE);
        self.words(a, at + 6, iend)?;
        let nw = 6 + iend;
        Ok((6 + n1 - nw, nw))
    }

    /// `moreio`: the next page of a LIST or TAB1 being read. Returns `(nb, nw)`.
    pub fn moreio(
        &mut self,
        a: &mut [f64],
        at: usize,
        nb: usize,
    ) -> Result<(usize, usize), NjoyError> {
        let nw = nb.min(NPAGE);
        self.words(a, at, nw)?;
        Ok((nb - nw, nw))
    }

    /// `tablio`: the `nr` (NBT, INT) pairs, three to a line, into `a(at..)`.
    fn tablio(&mut self, a: &mut [f64], at: usize, nr: usize) -> Result<(), NjoyError> {
        self.words(a, at, 2 * nr)
    }

    /// `tab1io`: head, interpolation table and first page of pairs. Returns `(nb, nw)`.
    ///
    /// # Errors
    /// "illegal TAB1" when `nr <= 0` or `nbt(nr) /= np`, as upstream.
    pub fn tab1io(
        &mut self,
        h: &mut Head,
        a: &mut [f64],
        at: usize,
    ) -> Result<(usize, usize), NjoyError> {
        self.contio(h, a, at)?;
        if h.n1h <= 0 {
            return Err(NjoyError::EndfParse(format!(
                " illegal TAB1, nr<=0 for mf/mt = {}/{}",
                h.mfh, h.mth
            )));
        }
        let nr = h.n1h as usize;
        let np = h.n2h.max(0) as usize;
        self.tablio(a, at + 6, nr)?;
        if nint(a[at + 3 + 2 * nr]) != h.n2h {
            return Err(NjoyError::EndfParse(format!(
                " illegal TAB1, nbt(nr)/=np for mf/mt = {}/{}",
                h.mfh, h.mth
            )));
        }
        let lim = (2 * np).min(NPAGE);
        self.words(a, at + 6 + 2 * nr, lim)?;
        let nw = 6 + 2 * nr + lim;
        Ok((6 + 2 * nr + 2 * np - nw, nw))
    }

    /// `tab2io`: head and interpolation table. Returns `nw`.
    pub fn tab2io(&mut self, h: &mut Head, a: &mut [f64], at: usize) -> Result<usize, NjoyError> {
        self.contio(h, a, at)?;
        let nr = h.n1h.max(0) as usize;
        self.tablio(a, at + 6, nr)?;
        Ok(6 + 2 * nr)
    }

    /// `tosend`: skip through the SEND card (any card with `mt <= 0`). Does
    /// not touch the module variables.
    pub fn tosend(&mut self) -> Result<(), NjoyError> {
        loop {
            if self.next_line()?.mt <= 0 {
                return Ok(());
            }
        }
    }

    /// `tomend`: skip through the MEND card (any card with `mat <= 0`).
    pub fn tomend(&mut self) -> Result<(), NjoyError> {
        loop {
            if self.next_line()?.mat <= 0 {
                return Ok(());
            }
        }
    }

    /// `findf(mat,mf,mt)`: position to read the card that starts section
    /// `mt` of file `mf` (`mt = 0`: the file's first section; `mf = 0`: the
    /// material's first file), searching down or up as upstream does.
    ///
    /// # Errors
    /// "mat .. mf .. mt .. not on tape" (`endf.f90:1582-1585`).
    pub fn findf(&mut self, mat: i32, mf: i32, mt: i32, itape: i32) -> Result<(), NjoyError> {
        let notfound = || {
            NjoyError::EndfParse(format!(
                " mat{mat:4} mf{mf:2} mt{mt:3} not on tape {itape:3}"
            ))
        };
        // 100
        let mut l;
        let mut igo: i32;
        'top: loop {
            l = self.next_line()?;
            if l.mat == 0 {
                continue 'top;
            }
            if l.mat == mat {
                // 120
                if mf == 0 {
                    return self.found(mat, mf, mt);
                }
                if l.mf == mf {
                    // 130
                    if mt == 0 || l.mt == mt {
                        return self.found(mat, mf, mt);
                    }
                    if l.mt == 0 {
                        self.back(2);
                        continue 'top;
                    }
                    igo = if l.mt > mt { -1 } else { 1 };
                    break 'top;
                }
                if l.mf == 0 {
                    continue 'top;
                }
                igo = if l.mf > mf { -1 } else { 1 };
                break 'top;
            }
            igo = 1;
            if l.mat == -1 || l.mat > mat {
                igo = -1;
            }
            break 'top;
        }
        // 200
        let (mut matl, mut mfl) = (0, 0);
        loop {
            if igo < 0 {
                if self.pos < 2 {
                    return Err(notfound());
                }
                self.back(2);
                matl = l.mat;
                mfl = l.mf;
            }
            l = self.next_line()?;
            if l.mat == mat {
                // 230
                if mf == 0 {
                    return self.found(mat, mf, mt);
                }
                if l.mf == mf {
                    // 240
                    if mt == 0 || l.mt == mt {
                        return self.found(mat, mf, mt);
                    }
                    continue;
                }
                if l.mf == 0 {
                    continue;
                }
                if (igo < 0 && l.mf < mf) || (igo > 0 && l.mf > mf) {
                    return Err(notfound());
                }
                continue;
            }
            if igo < 0 {
                // 220
                if l.mat < mat && l.mat != 0 {
                    return Err(notfound());
                }
                if l.mat == matl && mfl == 0 && l.mf == 0 {
                    igo = 1;
                }
                continue;
            }
            if l.mat > mat || l.mat == -1 {
                return Err(notfound());
            }
        }
    }

    /// `findf`'s 300: back up a card at a time to the card before the
    /// section's HEAD (a SEND, FEND or the label), so the HEAD is next.
    fn found(&mut self, mat: i32, mf: i32, mt: i32) -> Result<(), NjoyError> {
        loop {
            self.back(2);
            let l = self.next_line()?;
            if l.mt > 0 || (mt == 0 && l.mf > 0) || (mf == 0 && l.mat == mat) {
                if self.pos <= 1 {
                    return Ok(());
                }
                continue;
            }
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tape() -> Tape {
        let text = "\
 tape label                                                          1 0  0    0
 1.001000+3 9.991673-1          0          0          0          0 125 1451    1
 0.000000+0 0.000000+0          0          0          0          0 125 1451    2
                                                                   125 1  0    0
                                                                   125 0  0    0
 1.001000+3 9.991673-1          0          0          0          0 125 3  1    1
 0.000000+0 0.000000+0          0          0          1          2 125 3  1    2
          2          2                                             125 3  1    3
 1.000000+0 2.000000+0 3.000000+0 4.000000+0                       125 3  1    4
                                                                   125 3  0    0
 1.001000+3 9.991673-1          0          0          0          0 125 3  2    1
 0.000000+0 0.000000+0          0          0          1          2 125 3  2    2
          2          2                                             125 3  2    3
 1.000000+0 5.000000+0 3.000000+0 6.000000+0                       125 3  2    4
                                                                   125 3  0    0
                                                                   125 0  0    0
                                                                     0 0  0    0
                                                                    -1 0  0    0
";
        Tape::read(text.as_bytes()).unwrap()
    }

    #[test]
    fn records_and_positioning_follow_endf_f90() {
        let mut s = EndfStream::from_tape(&tape());
        let mut h = Head::default();
        let mut a = vec![0.0; 64];
        s.tpidio(&mut h).unwrap();
        s.contio(&mut h, &mut a, 1).unwrap();
        assert_eq!((h.math, h.mfh, h.mth), (125, 1, 451));
        s.tomend().unwrap();
        s.rewind();
        s.tpidio(&mut h).unwrap();
        s.findf(125, 3, 2, 20).unwrap();
        s.contio(&mut h, &mut a, 1).unwrap();
        assert_eq!(h.mth, 2);
        let (nb, nw) = s.tab1io(&mut h, &mut a, 1).unwrap();
        assert_eq!((nb, nw), (0, 12));
        assert_eq!(&a[8..12], &[1.0, 5.0, 3.0, 6.0]);
        s.tosend().unwrap();
        // findf searches up for a section already passed.
        s.findf(125, 3, 1, 20).unwrap();
        s.contio(&mut h, &mut a, 1).unwrap();
        assert_eq!(h.mth, 1);
        assert!(s.findf(125, 4, 2, 20).is_err());
    }

    #[test]
    fn a_long_list_is_paged() {
        let mut s = EndfStream {
            lines: Vec::new(),
            pos: 0,
        };
        s.lines.push(Line {
            mat: 1,
            mf: 6,
            mt: 2,
            f: [0.0, 0.0, 0.0, 0.0, 400.0, 0.0],
        });
        for i in 0..67 {
            let b = 6.0 * f64::from(i);
            s.lines.push(Line {
                mat: 1,
                mf: 6,
                mt: 2,
                f: [b, b + 1.0, b + 2.0, b + 3.0, b + 4.0, b + 5.0],
            });
        }
        let mut h = Head::default();
        let mut a = vec![0.0; 800];
        let (nb, nw) = s.listio(&mut h, &mut a, 1).unwrap();
        assert_eq!((nb, nw), (94, 312));
        let (nb, nw) = s.moreio(&mut a, 313, nb).unwrap();
        assert_eq!((nb, nw), (0, 94));
        assert_eq!(a[312 + 93], 399.0);
    }
}
