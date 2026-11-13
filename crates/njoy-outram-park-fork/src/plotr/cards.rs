// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   the `read(nsysi,*)` statements of `subroutine plotr`, l.415-750 and l.1689-1727.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! PLOTR's cards (`plotr.f90:33-353`), read in the order upstream reads them.
//!
//! Which cards follow card 2 depends only on earlier cards: cards 3-7 for
//! `|iplot| = 1`, card 7 for `jtype /= 0`, a second card 8 for a difference or
//! ratio plot, cards 9-10 for a 2-d plot (`i3d = 0`, set by card 4) and card 11
//! for a 3-d one, cards 12-13 for `iverf = 0`. So the cards are read whole here,
//! with upstream's defaults, and [`super::run`] consumes them in order. Each
//! character card is kept as the `character(60)` variable it is read into
//! (blank-padded to 60), which is what PLOTR writes back out.

use crate::deck::cards::{CardReader, Slot};
use crate::NjoyError;

/// Default paper size and margins (`plotr.f90:418-421`): US letter, 1 in.
const XPAPER: f64 = 8.5;
const YPAPER: f64 = 11.0;
const XMARG: f64 = 1.0;
const YMARG: f64 = 1.0;
/// `dsize` (`plotr.f90:422`).
pub const DSIZE: f64 = 0.30;
/// The default view point and work box (`plotr.f90:425-430`).
pub const DVIEW: [f64; 6] = [15.0, -15.0, 15.0, 2.5, 6.5, 2.5];

/// Cards 3 to 7a (`|iplot| = 1`).
#[derive(Debug, Clone, PartialEq)]
pub struct AxesCards {
    /// Card 3, card 3a: `t1`, `t2` (60 characters each).
    pub t1: String,
    /// See `t1`.
    pub t2: String,
    /// Card 4: `itype, jtype, igrid, ileg, xtag, ytag`.
    pub itype: i32,
    /// See `itype`.
    pub jtype: i32,
    /// See `itype`.
    pub igrid: i32,
    /// See `itype`.
    pub ileg: i32,
    /// See `itype`.
    pub xtag: f64,
    /// See `itype`.
    pub ytag: f64,
    /// Card 5: `xlf, xh, xstep`.
    pub x: [f64; 3],
    /// Card 5a: `xl` and its last non-blank column `nx`.
    pub xl: String,
    /// See `xl`.
    pub nx: usize,
    /// Card 6: `yb, yt, yystep`.
    pub y: [f64; 3],
    /// Card 6a: `yl`, `ny`.
    pub yl: String,
    /// See `yl`.
    pub ny: usize,
    /// Cards 7, 7a (`jtype /= 0`): `rbot, rtop, rstep`, `rl`, `nr`.
    pub r: Option<([f64; 3], String, usize)>,
}

/// Card 8 (`iverf, nin, matd, mfd, mtd, temper, nth, ntp, nkh`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceCard {
    /// `iverf`
    pub iverf: i32,
    /// `nin`
    pub nin: i32,
    /// `matd`
    pub matd: i32,
    /// `mfd`
    pub mfd: i32,
    /// `mtd`
    pub mtd: i32,
    /// `temper`
    pub temper: f64,
    /// `nth`
    pub nth: i32,
    /// `ntp`
    pub ntp: i32,
    /// `nkh`
    pub nkh: i32,
}

/// Cards 9 to 11.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleCards {
    /// Card 9 (`icon, isym, idash, iccol, ithick, ishade`), card 10 `aleg`
    /// (`ileg /= 0`) and card 10a `xtag, ytag, xpoint` (`ileg = 2`).
    TwoD {
        /// Card 9.
        c9: [i32; 6],
        /// Card 10.
        aleg: Option<String>,
        /// Card 10a.
        tag: Option<[f64; 3]>,
    },
    /// Card 11: `xv, yv, zv, x3, y3, z3`.
    ThreeD([f64; 6]),
}

/// Everything read for one curve, from card 2 on.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveCards {
    /// Card 2: `iplot, iwcol, factx, facty, xll, yll, ww, wh, wr` (`iwcol` as a real).
    pub c2: [f64; 9],
    /// Cards 3-7a.
    pub axes: Option<AxesCards>,
    /// Card 8 (absent for `iplot = 99`).
    pub src: SourceCard,
    /// The second card 8 of a difference or ratio plot.
    pub src2: Option<SourceCard>,
    /// Cards 9-11.
    pub style: StyleCards,
    /// Card 12 `nform` and the card 13 points (`iverf = 0`).
    pub data: Option<(i32, Vec<[f64; 6]>)>,
}

/// PLOTR's cards.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotrCards {
    /// Card 0: `nplt, nplt0`.
    pub nplt: i32,
    /// See `nplt` (read, and not used by upstream).
    pub nplt0: i32,
    /// Card 1: `lori, istyle, size, ipcol`.
    pub lori: i32,
    /// See `lori`.
    pub istyle: i32,
    /// See `lori`.
    pub size: f64,
    /// See `lori`.
    pub ipcol: i32,
    /// The curves, in order; the run ends at the card 2 with `iplot = 99`.
    pub curves: Vec<CurveCards>,
}

/// A character card into a `character(60)` preset to blanks, and its last
/// non-blank column (`plotr.f90:493-500`).
fn text60(r: &mut CardReader) -> Result<(String, usize), NjoyError> {
    let mut s = [Slot::string(60, "")];
    r.read(&mut s)?;
    let t = s[0].text().to_string();
    let n = t.chars().count();
    Ok((format!("{t:<60}"), n))
}

fn source(r: &mut CardReader) -> Result<SourceCard, NjoyError> {
    let mut s = [
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Real(0.0),
        Slot::Int(1),
        Slot::Int(1),
        Slot::Int(1),
    ];
    r.read(&mut s)?;
    Ok(SourceCard {
        iverf: s[0].int(),
        nin: s[1].int(),
        matd: s[2].int(),
        mfd: s[3].int(),
        mtd: s[4].int(),
        temper: s[5].real(),
        nth: s[6].int(),
        ntp: s[7].int(),
        nkh: s[8].int(),
    })
}

/// Read PLOTR's cards (`plotr.f90:433-750`, `:1690-1726`).
///
/// # Errors
/// A malformed card, or the end of the deck before card 2 with `iplot = 99`.
pub fn read_plotr(r: &mut CardReader) -> Result<PlotrCards, NjoyError> {
    let c0 = r.read_ints(&[0, 0])?;
    let mut c1 = [Slot::Int(1), Slot::Int(2), Slot::Real(DSIZE), Slot::Int(0)];
    r.read(&mut c1)?;
    let lori = c1[0].int();
    let (xpage, ypage) = if lori == 0 {
        (XPAPER - XMARG, YPAPER - YMARG)
    } else {
        (YPAPER - XMARG, XPAPER - YMARG)
    };
    let mut curves = Vec::new();
    // Card-reading state that persists across curves, as upstream's variables do.
    let (mut i3d, mut ileg) = (0, 0);
    loop {
        let mut c2 = [
            Slot::Int(1),
            Slot::Int(0),
            Slot::Real(1.0),
            Slot::Real(1.0),
            Slot::Real(0.0),
            Slot::Real(0.0),
            Slot::Real(xpage),
            Slot::Real(ypage),
            Slot::Real(0.0),
        ];
        r.read(&mut c2)?;
        let iplot = c2[0].int();
        let mut v2 = [0.0; 9];
        v2[0] = f64::from(iplot);
        v2[1] = f64::from(c2[1].int());
        for (k, s) in c2.iter().enumerate().skip(2) {
            v2[k] = s.real();
        }
        if iplot == 99 {
            curves.push(CurveCards {
                c2: v2,
                axes: None,
                src: SourceCard {
                    iverf: 0,
                    nin: 0,
                    matd: 0,
                    mfd: 0,
                    mtd: 0,
                    temper: 0.0,
                    nth: 1,
                    ntp: 1,
                    nkh: 1,
                },
                src2: None,
                style: StyleCards::ThreeD(DVIEW),
                data: None,
            });
            break;
        }
        let axes = if iplot.abs() == 1 {
            let (t1, _) = text60(r)?;
            let (t2, _) = text60(r)?;
            let mut c4 = [
                Slot::Int(4),
                Slot::Int(0),
                Slot::Int(2),
                Slot::Int(0),
                Slot::Real(0.0),
                Slot::Real(0.0),
            ];
            r.read(&mut c4)?;
            let itype = c4[0].int();
            let jtype = c4[1].int();
            ileg = c4[3].int();
            i3d = i32::from(itype <= 0);
            let x = r.read_reals(&[0.0, 0.0, 0.0])?;
            let (xl, nx) = text60(r)?;
            let y = r.read_reals(&[0.0, 0.0, 0.0])?;
            let (yl, ny) = text60(r)?;
            let rr = if jtype != 0 {
                let rv = r.read_reals(&[0.0, 0.0, 0.0])?;
                let (rl, nr) = text60(r)?;
                Some(([rv[0], rv[1], rv[2]], rl, nr))
            } else {
                None
            };
            Some(AxesCards {
                t1,
                t2,
                itype,
                jtype,
                igrid: c4[2].int(),
                ileg,
                xtag: c4[4].real(),
                ytag: c4[5].real(),
                x: [x[0], x[1], x[2]],
                xl,
                nx,
                y: [y[0], y[1], y[2]],
                yl,
                ny,
                r: rr,
            })
        } else {
            None
        };
        let src = source(r)?;
        let src2 = if src.iverf > 0
            && ((src.iverf > 1 && src.mfd == 3 && src.ntp > 1)
                || (src.iverf == 1 && src.ntp.abs() > 20))
        {
            // `matd2=0 ... nkh2=1` (`plotr.f90:664-672`). Upstream presets no
            // `iverf2` (it keeps its last value, undefined on the first such card);
            // here it defaults to 0, and `nin2` to 0 as `:642` sets it.
            Some(source(r)?)
        } else {
            None
        };
        let style = if i3d == 0 {
            let c9 = r.read_ints(&[0, 0, 0, 0, 1, 0])?;
            let mut aleg = None;
            let mut tag = None;
            if ileg != 0 {
                aleg = Some(text60(r)?.0);
                if ileg == 2 {
                    let t = r.read_reals(&[0.0, 0.0, 0.0])?;
                    tag = Some([t[0], t[1], t[2]]);
                }
            }
            StyleCards::TwoD {
                c9: [c9[0], c9[1], c9[2], c9[3], c9[4], c9[5]],
                aleg,
                tag,
            }
        } else {
            let v = r.read_reals(&DVIEW)?;
            StyleCards::ThreeD([v[0], v[1], v[2], v[3], v[4], v[5]])
        };
        let data = if src.iverf == 0 {
            let nform = r.read_ints(&[0])?[0];
            let mut pts = Vec::new();
            loop {
                let flag = -99.0;
                let z = r.read_reals(&[flag, flag, 0.0, 0.0, 0.0, 0.0])?;
                if z[0] == flag && z[1] == flag {
                    break;
                }
                pts.push([z[0], z[1], z[2], z[3], z[4], z[5]]);
            }
            Some((nform, pts))
        } else {
            None
        };
        curves.push(CurveCards {
            c2: v2,
            axes,
            src,
            src2,
            style,
            data,
        });
    }
    Ok(PlotrCards {
        nplt: c0[0],
        nplt0: c0[1],
        lori,
        istyle: c1[1].int(),
        size: c1[2].real(),
        ipcol: c1[3].int(),
        curves,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_follow_the_curve_kind() {
        let deck =
            " 31/\n /\n 1/\n 'title'/\n /\n 4/\n 1e3 2e7/\n /\n .5 10/\n /\n 5 30 1306 3 1/\n /\n\
                    2/\n 0/\n -1 0/\n 0/\n 1.1e7 .08 .05 .05/\n /\n\
                    1/\n 't'/\n 'u'/\n -1 2/\n /\n /\n /\n /\n /\n /\n 5 30 1306 4 2/\n /\n 99/\n";
        let mut r = CardReader::new(deck);
        let c = read_plotr(&mut r).unwrap();
        assert_eq!((c.nplt, c.lori, c.istyle, c.ipcol), (31, 1, 2, 0));
        assert_eq!(c.curves.len(), 4);
        let a = c.curves[0].axes.as_ref().unwrap();
        assert_eq!(a.t1.trim_end(), "title");
        assert_eq!(a.t1.len(), 60);
        assert_eq!((a.itype, a.nx, a.x), (4, 0, [1e3, 2e7, 0.0]));
        assert_eq!(c.curves[0].c2[6], 10.0);
        assert_eq!(
            c.curves[1].data.as_ref().unwrap().1,
            vec![[1.1e7, 0.08, 0.05, 0.05, 0.0, 0.0]]
        );
        assert!(matches!(
            c.curves[1].style,
            StyleCards::TwoD {
                c9: [-1, 0, 0, 0, 1, 0],
                ..
            }
        ));
        assert!(c.curves[2].axes.as_ref().unwrap().r.is_some());
        assert_eq!(c.curves[2].style, StyleCards::ThreeD(DVIEW));
        assert_eq!(c.curves[3].c2[0], 99.0);
    }
}
