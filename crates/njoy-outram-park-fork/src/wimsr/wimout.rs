// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine wimout`, l.1971-2150 — the WIMS-D/WIMS-E library text.
//   - `subroutine rsiout`, l.703-722 — the resonance-table record layout it copies.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `wimout` — the library text, record for record, in the Fortran edit
//! descriptors upstream uses: `(1p,5e15.8)`, `(i15)`, `(2i15)`,
//! `(3(1p,e15.8,i6))`, `(i6,1p,e15.8,5i6)`, `(1p,e15.8,2i6)` and the
//! literal `      999999999` separators of the WIMS-D format.

use crate::common::phys::AMASSN_AMU;

use super::input::WimsrInput;
use super::p1scat::P1Temp;
use super::resint::ResintResult;
use super::xsecs::XsecsResult;

/// Fortran `1PE15.8`: one digit before the point, eight after, a signed
/// two-digit exponent, right-aligned in 15 columns.
pub fn e15_8(x: f64) -> String {
    let s = format!("{:.8E}", x);
    let (mant, exp) = s.split_once('E').unwrap_or((&s, "0"));
    let e: i32 = exp.parse().unwrap_or(0);
    let sign = if e < 0 { '-' } else { '+' };
    format!("{:>15}", format!("{mant}E{sign}{:02}", e.abs()))
}

fn i15(v: i64) -> String {
    format!("{v:>15}")
}

fn i6(v: i64) -> String {
    format!("{v:>6}")
}

/// `(1p,5e15.8)`: five values per line.
fn lines_5e(vals: &[f64], out: &mut Vec<String>) {
    for chunk in vals.chunks(5) {
        out.push(chunk.iter().map(|&v| e15_8(v)).collect::<String>());
    }
}

/// The 15-character `999999999` separator line (`wimsr.f90:2119`).
const SEPARATOR: &str = "      999999999";

/// Write the library (`wimout`, `wimsr.f90:1971-2150`) as its lines.
#[allow(clippy::needless_range_loop)] // Fortran index loops kept verbatim
pub fn wimout(
    inp: &WimsrInput,
    awr: f64,
    iznum: i32,
    xs: &XsecsResult,
    ri: Option<&ResintResult>,
    p1: Option<&[P1Temp]>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let nfg = inp.nfg;
    let nrg = inp.nrg;
    let _ngr1 = nfg + nrg;
    let ntemp = xs.ntemp;
    let ifis = xs.ifiss;
    let nrestb = if ifis == 0 || ifis == 4 { 0 } else { 1 };
    let mut jfis = ifis;
    let ifprod = inp.ifprod_eff();
    if ifprod == 1 {
        jfis = -1;
    }
    if ifprod == 2 {
        jfis = -2;
    }
    let nfid = i64::from(inp.nfid());
    let ident = nfid;

    // identifiers (l.2048-2052)
    if inp.iverw != 4 {
        out.push(i15(nfid));
        out.push(format!("{}{}", i15(ident), i15(1)));
    }
    // burnup data (l.2055-2064)
    if inp.iburn >= 0 {
        if inp.iburn == 0 && inp.iverw != 4 {
            out.push(i15(0));
        }
        let (mut table, jcc) = inp.burn_table();
        if jcc >= 8 && table.len() >= 4 {
            table[3].1 = jfis;
        }
        out.push(format!("{}{}", i15(999_999_999), i15(3)));
        out.push(i15(jcc as i64));
        for chunk in table.chunks(3) {
            out.push(
                chunk
                    .iter()
                    .map(|&(y, id)| format!("{}{}", e15_8(y), i6(i64::from(id))))
                    .collect::<String>(),
            );
        }
        out.push(format!("{}{}", i15(999_999_999), i15(4)));
    }
    // material identifier (l.2067-2069)
    let awt = awr * AMASSN_AMU;
    out.push(format!(
        "{}{}{}{}{}{}{}",
        i6(ident),
        e15_8(awt),
        i6(i64::from(iznum)),
        i6(i64::from(ifis)),
        i6(ntemp as i64),
        i6(nrestb),
        i6(i64::from(inp.isof))
    ));
    // temperature-independent data (l.2072-2088)
    let ti = &xs.temp_independent;
    lines_5e(&ti.record, &mut out);
    if ifis > 1 {
        if let Some(f) = &ti.fission {
            lines_5e(f, &mut out);
        }
    }
    let ndat = ti.scatter.len();
    out.push(i15(ndat as i64));
    if ndat > 0 {
        lines_5e(&ti.scatter, &mut out);
    }
    // temperature-dependent data (l.2091-2114)
    lines_5e(&xs.tempr[..ntemp], &mut out);
    for td in xs.per_temp.iter().take(ntemp) {
        lines_5e(&td.record, &mut out);
        if ifis > 1 {
            if let Some(f) = &td.fission {
                lines_5e(f, &mut out);
            }
        }
        let ndat = td.scatter.len();
        out.push(i15(ndat as i64));
        if ndat != 0 {
            lines_5e(&td.scatter, &mut out);
        }
    }
    if inp.iverw == 4 {
        out.push(SEPARATOR.to_string());
    }
    // resonance data (l.2117-2143)
    if let Some(ri) = ri {
        let jres = ri.temps.len();
        let jsigz = ri.sigz.len();
        let ntnp = jres * jsigz;
        let rid = inp.rdfid;
        let header = format!("{}{}{}", e15_8(rid), i6(jres as i64), i6(jsigz as i64));
        let record = |table: &[Vec<f64>], g: &super::resint::ResonanceGroup| -> Vec<f64> {
            let mut v: Vec<f64> = ri.temps.clone();
            v.extend(g.sigb.iter().copied());
            for it in 0..jres {
                v.extend(table[it].iter().copied());
            }
            v
        };
        for g in &ri.groups {
            if inp.iverw == 4 {
                out.push(i15(ntnp as i64));
            }
            out.push(header.clone());
            lines_5e(&record(&g.absorption, g), &mut out);
            if ifis == 3 {
                if inp.iverw == 4 {
                    out.push(i15(ntnp as i64));
                }
                out.push(header.clone());
                let nu = g.nu_fission.clone().unwrap_or_else(|| {
                    vec![vec![0.0; jsigz]; jres]
                });
                lines_5e(&record(&nu, g), &mut out);
            }
            if inp.iverw == 4 {
                out.push(SEPARATOR.to_string());
            }
        }
        if inp.iverw != 4 {
            for g in &ri.groups {
                out.push(header.clone());
                lines_5e(&record(&g.elastic, g), &mut out);
            }
        }
    }
    // fission spectrum (l.2146-2148)
    if inp.isof != 0 {
        lines_5e(&xs.uff[..xs.nfiss], &mut out);
    }
    // p1 data (l.2151-2167)
    if let Some(p1) = p1 {
        for t in p1.iter().take(ntemp) {
            let mut vals: Vec<f64> = Vec::new();
            for (ig, lone, ltwo, v) in &t.rows {
                let nb = *ltwo as i64 - *lone as i64 + 1;
                vals.push((*ig as i64 - *lone as i64 + 1) as f64);
                vals.push(nb as f64);
                vals.extend(v.iter().copied());
            }
            out.push(i15(vals.len() as i64));
            lines_5e(&vals, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Fortran `1PE15.8` edit for the values the oracle library carries.
    #[test]
    fn e15_8_matches_fortran() {
        assert_eq!(e15_8(9.335_788_5), " 9.33578850E+00");
        assert_eq!(e15_8(-1.711_086e-4), "-1.71108600E-04");
        assert_eq!(e15_8(0.0), " 0.00000000E+00");
        assert_eq!(e15_8(1.0e10), " 1.00000000E+10");
        assert_eq!(e15_8(2.936e2), " 2.93600000E+02");
        assert_eq!(i15(999_999_999), "      999999999");
    }
}
