// Translated from NJOY2016.80 `src/acefc.f90`: `gamout` (:4552-4899), the
// multigroup photon branch (`ngend /= 0`: sum of the GENDF MF=16/17 matrices,
// equally probable photon bins, MF=15/MT=1) and `acelod`'s read-back of it into
// the old-style 30 x 20 photon matrix after GPD (:6276-6300).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Old-style gamma production matrix** (`gamout`): ACER's `ngend` GENDF, the
//! GROUPR photon-production matrices (MF=16/17) of the material summed over the
//! sink groups, converted into `nbin = 20` equally probable photon energies per
//! neutron group. The 30 x 20 words follow GPD in the ACE table (`acelod` insists
//! on 30 neutron groups, `:6290`).

use crate::endf::parse::{format_endf_float, parse_endf_float};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const NBIN: usize = 20;
const EMEV: f64 = 1.0e6;

fn field(line: &str, k: usize) -> f64 {
    line.get(11 * k..(11 * (k + 1)).min(66)).map(|s| parse_endf_float(s).unwrap_or(0.0)).unwrap_or(0.0)
}

fn ifield(line: &str, k: usize) -> i64 {
    field(line, k).round() as i64
}

/// `gamout` on the GENDF text `gendf` for material `mat`, then `acelod`'s read-back:
/// the `30 * 20` words `sigfig(eb / emev, 7, 0)` (`eb` after the ENDF-text round trip
/// of `listio`).
///
/// # Errors
/// [`NjoyError::InvalidInput`] without the material, without photon groups on the
/// tape (`'no gamma groups on ngend.'`) or when `ngn != 30`.
pub fn photon_spectra(gendf: &str, mat: i32) -> Result<Vec<f64>, NjoyError> {
    let lines: Vec<&str> = gendf.lines().collect();
    let key = |l: &str| -> (i32, i32, i32) {
        let n = |a: usize, b: usize| l.get(a..b).and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
        (n(66, 70), n(70, 72), n(72, 75))
    };
    let (mut ngn, mut ngg) = (0usize, 0usize);
    let mut egg = Vec::<f64>::new();
    let mut sig: Vec<f64> = Vec::new();
    let mut found = false;
    let mut i = 1; // skip the tape label
    while i < lines.len() {
        let l = lines[i];
        let (m, mf, mt) = key(l);
        if m != mat || mf == 0 || mt == 0 {
            i += 1;
            continue;
        }
        found = true;
        // section CONT: nl = L1, nz = L2, ng(or ntw) = N2
        let (nl, nz, nw1) = (ifield(l, 2) as usize, ifield(l, 3) as usize, ifield(l, 5) as usize);
        i += 1;
        if mf == 1 && mt == 451 {
            // LIST: temp 0 ngn ngg np 0 ; data = title, sigz(nz), egn(ngn+1), egg(ngg+1)
            let h = lines[i];
            let np = ifield(h, 4) as usize;
            ngn = ifield(h, 2) as usize;
            ngg = ifield(h, 3) as usize;
            if ngg == 0 {
                return Err(NjoyError::InvalidInput("gamout: no gamma groups on ngend.".into()));
            }
            let mut data = Vec::with_capacity(np);
            let nlines = np.div_ceil(6);
            for k in 0..nlines {
                let dl = lines[i + 1 + k];
                for c in 0..6 {
                    if data.len() < np {
                        data.push(field(dl, c));
                    }
                }
            }
            let base = nw1 + nz;
            egg = data[base + ngn + 1..base + ngn + 2 + ngg].to_vec();
            sig = vec![0.0; ngn * ngg];
            i += 1 + nlines;
            continue;
        }
        if mf == 16 || mf == 17 {
            // LISTs until the SEND (mt of the line = 0).
            while i < lines.len() && key(lines[i]).2 != 0 {
                let h = lines[i];
                let (ng2, ig2lo, np, ig) = (ifield(h, 2), ifield(h, 3), ifield(h, 4) as usize, ifield(h, 5));
                let nlines = np.div_ceil(6);
                let mut data = Vec::with_capacity(np);
                for k in 0..nlines {
                    let dl = lines[i + 1 + k];
                    for c in 0..6 {
                        if data.len() < np {
                            data.push(field(dl, c));
                        }
                    }
                }
                let jg = ig - 1;
                if jg > 0 {
                    for ii in 2..=ng2 {
                        let jpos = ig2lo + ii - 2;
                        if jpos >= 1 && jpos <= ngg as i64 {
                            let loca = nl * nz * (ii as usize - 1);
                            let locs = (jpos + ngg as i64 * jg) as usize;
                            sig[locs - 1] += data[loca];
                        }
                    }
                }
                i += 1 + nlines;
            }
            continue;
        }
    }
    if !found {
        return Err(NjoyError::InvalidInput("gamout: mat not found.".into()));
    }
    if ngn != 30 {
        return Err(NjoyError::InvalidInput("acelod: 30 groups are required for 30x20 photon matrix".into()));
    }
    // equal probability bins (`:4780-4830`)
    let mut out = Vec::with_capacity(ngn * NBIN);
    for ign in 0..ngn {
        let jpos = ngg * ign;
        let mut sum = 0.0f64;
        let mut jgghi = 1usize;
        for jgg in 1..=ngg {
            let s = sig[jgg - 1 + jpos];
            sum += s;
            if s != 0.0 {
                jgghi = jgg;
            }
        }
        let mut eb = [0.0f64; NBIN];
        if sum != 0.0 {
            let sumn = sum / NBIN as f64;
            let rsumn = 1.0 / sumn;
            let mut jgg = 1usize;
            let mut elast = egg[0];
            for jb in 2..=NBIN + 1 {
                let r0 = sumn;
                let mut r = r0;
                let mut s = 0.0f64;
                loop {
                    let dist = sig[jgg - 1 + jpos] / (egg[jgg] - egg[jgg - 1]);
                    if jgg == jgghi || dist * (egg[jgg] - elast) >= r {
                        let e = elast + r / dist;
                        eb[jb - 2] = rsumn * (s + dist * (e * e - elast * elast) / 2.0);
                        elast = e;
                        break;
                    }
                    r -= dist * (egg[jgg] - elast);
                    s += dist * (egg[jgg] * egg[jgg] - elast * elast) / 2.0;
                    elast = egg[jgg];
                    jgg += 1;
                }
            }
            for e in &mut eb {
                *e = sigfig(*e, 7, 0);
            }
        }
        // `listio` writes ENDF text, `acelod` reads it back, then `sigfig(x/emev, 7, 0)`.
        for e in eb {
            let rt = parse_endf_float(&format_endf_float(e)).unwrap_or(e);
            out.push(sigfig(rt / EMEV, 7, 0));
        }
    }
    Ok(out)
}
