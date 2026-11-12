// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplots`, the unresolved-resonance pages (`acefc.f90:15655-16160`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! The four unresolved-resonance pages of `aplots`: total, elastic, fission
//! and capture, each at infinite dilution, 100 b and 1 b.
//!
//! Upstream writes the four blocks as four copies of one text. They differ in
//! which probability-table column carries the cross section (1..4 times
//! `nurb` words after the cumulative probabilities) and in four details that
//! are kept exactly: the total's `if (s2.gt.ymax) ymax=s1` (a typo upstream),
//! its `ie.gt.pltumx` guard, the fission page's `ymax > 0` condition, and the
//! capture block never setting `nunu` in its `lssf = 0` branch (it keeps the
//! fission block's value, which is `nure` in that branch).

use super::fmt::{hk_it, nint, AceTable, Plot};
use super::scale::ascll;
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

const PLTUMX: usize = 10000;

#[derive(Clone, Copy, PartialEq)]
enum Which {
    Total,
    Elastic,
    Fission,
    Capture,
}

struct UrState {
    ee: Vec<f64>,
    s0: Vec<f64>,
    s1: Vec<f64>,
    s2: Vec<f64>,
    nunu: usize,
}

fn terp(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, intunr: i64) -> Result<f64, NjoyError> {
    terp1(x1, y1, x2, y2, x, IntLaw::from_code(intunr as u32))
}

fn ur_block(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64, which: Which, st: &mut UrState) -> Result<(), NjoyError> {
    let it = hk_it(hk);
    let nes = a.nes;
    let nure = a.xi(a.iurpt);
    let intunr = a.xi(a.iurpt + 2);
    let nurb = a.xi(a.iurpt + 1);
    let lssf = a.xi(a.iurpt + 5);
    let kcol: i64 = match which {
        Which::Total => 1,
        Which::Elastic => 2,
        Which::Fission => 3,
        Which::Capture => 4,
    };
    let (mut xmin, mut xmax, mut ymin, mut ymax);
    if lssf == 0 {
        xmin = 1e10;
        xmax = 0.0;
        ymin = 1e10;
        ymax = 0.0;
        for ie in 1..=nure {
            let iu = ie as usize;
            st.ee[iu] = a.x(a.iurpt + 5 + ie).abs();
            if st.ee[iu] > xmax {
                xmax = st.ee[iu];
            }
            if st.ee[iu] < xmin {
                xmin = st.ee[iu];
            }
            st.s0[iu] = 0.0;
            st.s1[iu] = 0.0;
            st.s2[iu] = 0.0;
            let mut f1 = 0.0;
            let mut f2 = 0.0;
            let mut cl = 0.0;
            let ll = a.iurpt + 5 + nure + (ie - 1) * 6 * nurb;
            for ib in 1..=nurb {
                let dp = a.x(ll + ib) - cl;
                let v = a.x(ll + kcol * nurb + ib);
                let tt = a.x(ll + nurb + ib);
                st.s0[iu] += dp * v;
                st.s1[iu] += dp * v / (100.0 + tt);
                st.s2[iu] += dp * v / (1.0 + tt);
                f1 += dp / (100.0 + tt);
                f2 += dp / (1.0 + tt);
                cl = a.x(ll + ib);
            }
            st.s1[iu] /= f1;
            st.s2[iu] /= f2;
            if st.s0[iu] > ymax {
                ymax = st.s0[iu];
            }
            if st.s1[iu] > ymax {
                ymax = st.s1[iu];
            }
            if st.s2[iu] > ymax {
                ymax = if which == Which::Total { st.s1[iu] } else { st.s2[iu] };
            }
            if st.s0[iu] < ymin {
                ymin = st.s0[iu];
            }
            if st.s1[iu] < ymin {
                ymin = st.s1[iu];
            }
            if st.s2[iu] < ymin {
                ymin = st.s2[iu];
            }
        }
        st.nunu = nure as usize;
    } else {
        xmin = a.x(a.iurpt + 5 + 1).abs();
        xmax = a.x(a.iurpt + 5 + nure).abs();
        ymin = 1e10;
        ymax = 0.0;
        let (mut kf, mut iif, mut nnf, mut kc, mut iic) = (0i64, 0i64, 0i64, 0i64, 0i64);
        if which == Which::Fission {
            for i in 1..=a.ntr {
                let mt = a.xi(a.mtr - 1 + i);
                if mt == 18 || mt == 19 {
                    kf = nint(a.x(a.lsig - 1 + i) + a.sig as f64 - 1.0);
                    nnf = a.xi(kf + 1);
                    iif = a.xi(kf);
                }
            }
        }
        if which == Which::Capture {
            for i in 1..=a.ntr {
                let mt = a.xi(a.mtr - 1 + i);
                if mt == 102 {
                    kc = nint(a.x(a.lsig - 1 + i) + a.sig as f64 - 1.0);
                    iic = a.xi(kc);
                }
            }
        }
        let mut ie: usize = 0;
        for i in 1..=nes {
            let e = a.x(a.esz - 1 + i);
            if e < xmin || e >= xmax {
                continue;
            }
            ie += 1;
            if which == Which::Total && ie > PLTUMX {
                continue;
            }
            if ie >= st.ee.len() {
                return Err(NjoyError::InvalidInput(format!("aplots: need to redefine pltumx to {ie}")));
            }
            st.ee[ie] = e;
            let tot = a.x(a.esz + nes - 1 + i);
            let val = match which {
                Which::Total => tot,
                Which::Elastic => a.x(a.esz + 3 * nes - 1 + i),
                Which::Fission => {
                    let mut f = 0.0;
                    if nnf > 0 && i >= iif {
                        f = a.x(kf + 2 + i - iif);
                    }
                    f
                }
                Which::Capture => a.x(kc + 2 + i - iic),
            };
            st.s0[ie] = val;
            for ii in 1..nure {
                let e1 = a.x(a.iurpt + 5 + ii).abs();
                let e2 = a.x(a.iurpt + 5 + ii + 1).abs();
                if e < e1 || e >= e2 {
                    continue;
                }
                let kk = a.iurpt + 5 + nure + (ii - 1) * 6 * nurb;
                let ll = a.iurpt + 5 + nure + ii * 6 * nurb;
                st.s1[ie] = 0.0;
                st.s2[ie] = 0.0;
                let (mut f1, mut f2, mut c1, mut c2) = (0.0, 0.0, 0.0, 0.0);
                for j in 1..=nurb {
                    let dp = terp(e1, a.x(kk + j) - c1, e2, a.x(ll + j) - c2, e, intunr)?;
                    let pp = terp(e1, a.x(kk + nurb + j), e2, a.x(ll + nurb + j), e, intunr)?;
                    let pe = terp(e1, a.x(kk + kcol * nurb + j), e2, a.x(ll + kcol * nurb + j), e, intunr)?;
                    st.s1[ie] += dp * pe * val / (100.0 + pp * tot);
                    st.s2[ie] += dp * pe * val / (1.0 + pp * tot);
                    f1 += dp / (100.0 + pp * tot);
                    f2 += dp / (1.0 + pp * tot);
                    c1 = a.x(kk + j);
                    c2 = a.x(ll + j);
                }
                st.s1[ie] /= f1;
                st.s2[ie] /= f2;
                if st.s0[ie] > ymax {
                    ymax = st.s0[ie];
                }
                if st.s0[ie] < ymin {
                    ymin = st.s0[ie];
                }
                if st.s1[ie] > ymax {
                    ymax = st.s1[ie];
                }
                if st.s1[ie] < ymin {
                    ymin = st.s1[ie];
                }
                if st.s2[ie] > ymax {
                    ymax = st.s2[ie];
                }
                if st.s2[ie] < ymin {
                    ymin = st.s2[ie];
                }
            }
        }
        if which == Which::Total && ie > PLTUMX {
            return Err(NjoyError::InvalidInput(format!("aplots: need to redefine pltumx to {ie}")));
        }
        st.nunu = ie;
    }
    if which == Which::Fission && ymax <= 0.0 {
        return Ok(());
    }
    ymax += ymax / 10.0;
    if ymin < ymax / 1000.0 {
        ymin = ymax / 1000.0;
    }
    (xmin, xmax) = ascll(xmin, xmax);
    (ymin, ymax) = ascll(ymin, ymax);
    w.page(iwcol, hk, it);
    w.line(match which {
        Which::Total => "'UR total cross section'/",
        Which::Elastic => "'UR elastic cross section'/",
        Which::Fission => "'UR fission cross section'/",
        Which::Capture => "'UR capture cross section'/",
    });
    w.line("4 0 2 1/");
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<c>ross section (barns)'/");
    w.line("/");
    w.line("/");
    w.line("'Inf. Dil.'/");
    w.line("0/");
    for ie in 1..=st.nunu {
        w.pair(st.ee[ie], st.s0[ie]);
    }
    w.line("/");
    w.line("2/");
    w.line("/");
    w.line("0 0 0 1/");
    w.line("'100 b'/");
    w.line("0/");
    for ie in 1..=st.nunu {
        w.pair(st.ee[ie], st.s1[ie]);
    }
    w.line("/");
    w.line("3/");
    w.line("/");
    w.line("0 0 0 2/");
    w.line("'1 b'/");
    w.line("0/");
    for ie in 1..=st.nunu {
        w.pair(st.ee[ie], st.s2[ie]);
    }
    w.line("/");
    Ok(())
}

/// The UR pages (`if (iurpt.ne.0)`): total, elastic, fission, capture.
pub fn ur_pages(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) -> Result<(), NjoyError> {
    if a.iurpt == 0 {
        return Ok(());
    }
    let nure = a.xi(a.iurpt);
    if nure as usize > PLTUMX {
        return Err(NjoyError::InvalidInput(format!("aplots: need to redefine pltumx to {nure}")));
    }
    let mut st = UrState {
        ee: vec![0.0; PLTUMX + 2],
        s0: vec![0.0; PLTUMX + 2],
        s1: vec![0.0; PLTUMX + 2],
        s2: vec![0.0; PLTUMX + 2],
        nunu: 0,
    };
    for which in [Which::Total, Which::Elastic, Which::Fission, Which::Capture] {
        ur_block(a, w, hk, iwcol, which, &mut st)?;
    }
    Ok(())
}
