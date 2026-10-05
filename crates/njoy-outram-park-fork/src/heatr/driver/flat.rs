// Ported from NJOY2016 `src/endf.f90` (`terp1`, `terpa`) and the record
// layout `heatr.f90` keeps in its work arrays.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! NJOY's in-memory ENDF record layout, and the two interpolation routines
//! HEATR calls on it.
//!
//! HEATR keeps every record it reads in a flat `real(kr)` array exactly as
//! `contio`/`listio`/`tab1io`/`tab2io` leave it: six header words
//! (`C1, C2, L1, L2, N1, N2`), then for a LIST the `N1` values, for a TAB1
//! the `NR` (`NBT`, `INT`) pairs followed by the `NP` (`x`, `y`) pairs, and for
//! a TAB2 the `NR` pairs alone. Most of HEATR indexes those arrays directly
//! (`c(lnow+6+2*nr+2*np)`, `a(ibase+ncyc*(i-1)+1)`), so this port builds the
//! same arrays and keeps the indexing. A Rust slice `s` stands for a Fortran
//! array `a` with `s[k - 1] == a(k)`.

use crate::endf::records::{Cont, List, Tab1, Tab2};

/// Fortran `nint`: round half away from zero.
pub(super) fn nint(x: f64) -> i64 {
    x.round() as i64
}

/// The six header words of a record.
pub(super) fn cont_flat(c: &Cont) -> Vec<f64> {
    vec![
        c.c1,
        c.c2,
        f64::from(c.l1),
        f64::from(c.l2),
        f64::from(c.n1),
        f64::from(c.n2),
    ]
}

/// A TAB1 as `tab1io` stores it.
pub(super) fn tab1_flat(t: &Tab1) -> Vec<f64> {
    let mut v = cont_flat(&t.head);
    v[4] = t.interp.len() as f64;
    v[5] = t.pairs.len() as f64;
    v.reserve(2 * t.interp.len() + 2 * t.pairs.len());
    for &(nbt, int) in &t.interp {
        v.push(f64::from(nbt));
        v.push(f64::from(int));
    }
    for &(x, y) in &t.pairs {
        v.push(x);
        v.push(y);
    }
    v
}

/// A LIST as `listio` stores it.
pub(super) fn list_flat(l: &List) -> Vec<f64> {
    let mut v = cont_flat(&l.head);
    v[4] = l.data.len() as f64;
    v.extend_from_slice(&l.data);
    v
}

/// A TAB2 as `tab2io` stores it.
pub(super) fn tab2_flat(t: &Tab2) -> Vec<f64> {
    let mut v = cont_flat(&t.head);
    v[4] = t.interp.len() as f64;
    for &(nbt, int) in &t.interp {
        v.push(f64::from(nbt));
        v.push(f64::from(int));
    }
    v
}

/// Append `vals` to `rows`, six to a line, the last line zero-padded.
fn pack(rows: &mut Vec<[f64; 6]>, vals: &[f64]) {
    for chunk in vals.chunks(6) {
        let mut r = [0.0; 6];
        r[..chunk.len()].copy_from_slice(chunk);
        rows.push(r);
    }
}

/// Section rows of a flat CONT, LIST or TAB2: the head, then the words
/// after it six to a line (a CONT has none).
pub(super) fn rows_of_list(flat: &[f64]) -> Vec<[f64; 6]> {
    let mut rows = vec![[flat[0], flat[1], flat[2], flat[3], flat[4], flat[5]]];
    pack(&mut rows, &flat[6..]);
    rows
}

/// Section rows of a flat TAB1: the head, the interpolation pairs, then the
/// data pairs, each block packed six to a line.
pub(super) fn rows_of_tab1(flat: &[f64]) -> Vec<[f64; 6]> {
    let nr = nint(flat[4]) as usize;
    let mut rows = vec![[flat[0], flat[1], flat[2], flat[3], flat[4], flat[5]]];
    pack(&mut rows, &flat[6..6 + 2 * nr]);
    pack(&mut rows, &flat[6 + 2 * nr..]);
    rows
}

/// `terp1` (`endf.f90`), with upstream's arithmetic and no error path: a log
/// law on a non-positive argument gives what Fortran's `log` gives (a NaN),
/// as it would upstream. Laws other than 1-5 leave `y = y1`.
pub(super) fn terp1(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, i: i64) -> f64 {
    if x2 == x1 {
        return y1;
    }
    if i == 1 || y2 == y1 || x == x1 {
        return y1;
    }
    match i {
        2 => y1 + (x - x1) * (y2 - y1) / (x2 - x1),
        3 => y1 + (x / x1).ln() * (y2 - y1) / (x2 / x1).ln(),
        4 => y1 * ((x - x1) * (y2 / y1).ln() / (x2 - x1)).exp(),
        5 => {
            if y1 == 0.0 {
                y1
            } else {
                y1 * ((x / x1).ln() * (y2 / y1).ln() / (x2 / x1).ln()).exp()
            }
        }
        _ => y1,
    }
}

/// `terpa` (`endf.f90`) on a flat TAB1, with upstream's `ip`/`ir` search
/// state. Returns `(y, xnext, idis)`.
pub(super) fn terpa(a: &[f64], x: f64, ip: &mut usize, ir: &mut usize) -> (f64, f64, bool) {
    const SHADE: f64 = 1.00001;
    const XBIG: f64 = 1.0e12;
    let at = |k: usize| a[k - 1];
    let nr = nint(at(5)) as usize;
    let np = nint(at(6)) as usize;
    if *ir > nr {
        *ir = nr;
    }
    if *ip > np {
        *ip = np;
    }
    let mut jr = 5 + 2 * *ir;
    let mut jp = 5 + 2 * nr + 2 * *ip;
    let mut idis = false;
    loop {
        // 110
        if x < at(jp) {
            // 120
            if x > at(jp - 2) {
                // 130
                let int = nint(at(jr + 1));
                let y = terp1(at(jp - 2), at(jp - 1), at(jp), at(jp + 1), x, int);
                let xnext = at(jp);
                if int == 1 {
                    idis = true;
                }
                if *ip != np && at(jp + 2) == xnext {
                    idis = true;
                }
                return (y, xnext, idis);
            }
            if x == at(jp - 2) {
                // 140
                let y = at(jp - 1);
                let int = nint(at(jr + 1));
                let xnext = at(jp);
                if int == 1 {
                    idis = true;
                }
                if *ip != np && at(jp + 2) == xnext {
                    idis = true;
                }
                return (y, xnext, idis);
            }
            if *ip == 2 {
                // 170: below the first point.
                return (0.0, at(jp - 2), true);
            }
            jp -= 2;
            *ip -= 1;
            if *ir == 1 {
                continue;
            }
            let it = nint(at(jr - 2)) as usize;
            if *ip > it {
                continue;
            }
            jr -= 2;
            *ir -= 1;
            continue;
        }
        if *ip == np {
            // 150: the last point and above.
            if x < SHADE * at(jp) {
                let y = at(jp + 1);
                let xnext = if y > 0.0 {
                    SHADE * SHADE * at(jp)
                } else {
                    XBIG
                };
                return (y, xnext, false);
            }
            return (0.0, XBIG, false);
        }
        jp += 2;
        *ip += 1;
        let it = nint(at(jr)) as usize;
        if *ip <= it {
            continue;
        }
        jr += 2;
        *ir += 1;
    }
}

/// `terpa` from `ip = 2, ir = 1`, as most HEATR call sites start it.
pub(super) fn terpa0(a: &[f64], x: f64) -> (f64, f64, bool) {
    let (mut ip, mut ir) = (2, 1);
    terpa(a, x, &mut ip, &mut ir)
}

/// The 64-point Gauss-Legendre abscissae of `disbar`, `getsix` and `hgetco`,
/// as upstream writes them.
pub(super) const QP64: [f64; 64] = [
    -9.99305042E-01,
    -9.96340117E-01,
    -9.91013371E-01,
    -9.83336254E-01,
    -9.73326828E-01,
    -9.61008800E-01,
    -9.46411375E-01,
    -9.29569172E-01,
    -9.10522137E-01,
    -8.89315446E-01,
    -8.65999398E-01,
    -8.40629296E-01,
    -8.13265315E-01,
    -7.83972359E-01,
    -7.52819907E-01,
    -7.19881850E-01,
    -6.85236313E-01,
    -6.48965471E-01,
    -6.11155355E-01,
    -5.71895646E-01,
    -5.31279464E-01,
    -4.89403146E-01,
    -4.46366017E-01,
    -4.02270158E-01,
    -3.57220158E-01,
    -3.11322872E-01,
    -2.64687162E-01,
    -2.17423644E-01,
    -1.69644420E-01,
    -1.21462819E-01,
    -7.29931218E-02,
    -2.43502927E-02,
    2.43502927E-02,
    7.29931218E-02,
    1.21462819E-01,
    1.69644420E-01,
    2.17423644E-01,
    2.64687162E-01,
    3.11322872E-01,
    3.57220158E-01,
    4.02270158E-01,
    4.46366017E-01,
    4.89403146E-01,
    5.31279464E-01,
    5.71895646E-01,
    6.11155355E-01,
    6.48965471E-01,
    6.85236313E-01,
    7.19881850E-01,
    7.52819907E-01,
    7.83972359E-01,
    8.13265315E-01,
    8.40629296E-01,
    8.65999398E-01,
    8.89315446E-01,
    9.10522137E-01,
    9.29569172E-01,
    9.46411375E-01,
    9.61008800E-01,
    9.73326828E-01,
    9.83336254E-01,
    9.91013371E-01,
    9.96340117E-01,
    9.99305042E-01,
];

/// The weights paired with [`QP64`].
pub(super) const QW64: [f64; 64] = [
    1.78328072E-03,
    4.14703326E-03,
    6.50445797E-03,
    8.84675983E-03,
    1.11681395E-02,
    1.34630479E-02,
    1.57260305E-02,
    1.79517158E-02,
    2.01348232E-02,
    2.22701738E-02,
    2.43527026E-02,
    2.63774697E-02,
    2.83396726E-02,
    3.02346571E-02,
    3.20579284E-02,
    3.38051618E-02,
    3.54722133E-02,
    3.70551285E-02,
    3.85501532E-02,
    3.99537411E-02,
    4.12625632E-02,
    4.24735151E-02,
    4.35837245E-02,
    4.45905582E-02,
    4.54916279E-02,
    4.62847966E-02,
    4.69681828E-02,
    4.75401657E-02,
    4.79993886E-02,
    4.83447622E-02,
    4.85754674E-02,
    4.86909570E-02,
    4.86909570E-02,
    4.85754674E-02,
    4.83447622E-02,
    4.79993886E-02,
    4.75401657E-02,
    4.69681828E-02,
    4.62847966E-02,
    4.54916279E-02,
    4.45905582E-02,
    4.35837245E-02,
    4.24735151E-02,
    4.12625632E-02,
    3.99537411E-02,
    3.85501532E-02,
    3.70551285E-02,
    3.54722133E-02,
    3.38051618E-02,
    3.20579284E-02,
    3.02346571E-02,
    2.83396726E-02,
    2.63774697E-02,
    2.43527026E-02,
    2.22701738E-02,
    2.01348232E-02,
    1.79517158E-02,
    1.57260305E-02,
    1.34630479E-02,
    1.11681395E-02,
    8.84675983E-03,
    6.50445797E-03,
    4.14703326E-03,
    1.78328072E-03,
];

/// The four-point rule of `capdam`, `anadam`, `tabdam`, `tabsq6`, `tabsqr`.
pub(super) const QP4: [f64; 4] = [-0.86114, -0.33998, 0.33998, 0.86114];
/// The weights paired with [`QP4`].
pub(super) const QW4: [f64; 4] = [0.34785, 0.65215, 0.65215, 0.34785];

/// `legndr` (`mathm.f90`) in upstream's 1-based form: the returned vector
/// has `p[l] = P_{l-1}(x)` for `l = 1 ..= np + 1`, and `p[0]` unused.
pub(super) fn legndr1(x: f64, np: usize) -> Vec<f64> {
    let p = crate::groupr::kinematics::shared::legndr(x, np);
    let mut out = Vec::with_capacity(np + 2);
    out.push(0.0);
    out.extend_from_slice(&p);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(pairs: &[(f64, f64)], interp: &[(u32, u32)]) -> Vec<f64> {
        let t = Tab1 {
            head: Cont {
                c1: 0.0,
                c2: 0.0,
                l1: 0,
                l2: 0,
                n1: interp.len() as i32,
                n2: pairs.len() as i32,
            },
            interp: interp.to_vec(),
            pairs: pairs.to_vec(),
        };
        tab1_flat(&t)
    }

    /// The stateful search must give the stateless `endf::interp::terpa`'s
    /// answer from any starting state, including at a duplicated abscissa.
    #[test]
    fn stateful_terpa_agrees_with_the_stateless_one() {
        let pairs = [(1.0, 1.0), (2.0, 4.0), (2.0, 5.0), (3.0, 9.0), (5.0, 2.0)];
        let interp = [(3u32, 2u32), (5, 1)];
        let a = tab(&pairs, &interp);
        for &x in &[0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 5.00001, 6.0] {
            let want = crate::endf::interp::terpa(&interp, &pairs, x);
            for start in 2..=5 {
                let (mut ip, mut ir) = (start, if start > 3 { 2 } else { 1 });
                let got = terpa(&a, x, &mut ip, &mut ir);
                assert_eq!(got.0, want.0, "y at {x} from ip={start}");
                assert_eq!(got.1, want.1, "xnext at {x} from ip={start}");
                assert_eq!(got.2, want.2, "idis at {x} from ip={start}");
            }
        }
    }
}
