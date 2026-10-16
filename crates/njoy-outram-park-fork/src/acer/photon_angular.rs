// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine ptinit`, l.2593-2639;
//   - `subroutine ptleg`, l.2641-2866;
//   - `subroutine pttab`, l.2977-3281 (with `summer`, l.3316-3349);
//   - `convr`'s MF=14 pass, l.4617-4657.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Anisotropic photon angular distributions as ACER writes them: 32
//! equally probable cosine bins (33 bin edges) per incident energy.
//!
//! `convr` converts every anisotropic MF=14 subsection before `acelpp` reads
//! it: `ptleg` for Legendre coefficients (`LTT = 1`), `pttab` for a tabulated
//! law (`LTT = 2`). A distribution found isotropic comes out as a 2-point
//! table, for which `acelpp` writes locator 0. [`ptleg`] and [`pttab`] return
//! `None` in that case and the 33 bin edges otherwise, rounded by
//! `sigfig(., 7)` as upstream writes them.
//!
//! The arithmetic follows upstream operation for operation, including its
//! exact floating-point comparisons (`pro(k) == sum`, `ap == 0`), because
//! the bin edges are stored words.

use std::sync::OnceLock;

use crate::endf::interp::gral;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `nbina = 32` coarse bins (`acefc.f90:191`), so `npt = 33` edges.
pub const NBINS: usize = 32;
/// Bin edges per incident energy.
pub const NPT: usize = NBINS + 1;
/// `ni`: the highest Legendre order `ptinit` prepares for.
const NI: usize = 64;

/// `ptinit`'s `xat`: the coefficients of the series for the integral of
/// `P_n` (`acefc.f90:2593-2639`). 1-based as upstream: index
/// `(k-1)*ni + n`.
fn xat() -> &'static [f64] {
    static XAT: OnceLock<Vec<f64>> = OnceLock::new();
    XAT.get_or_init(|| {
        let nwords = NI * (1 + NI / 2);
        let mut xat = vec![0.0f64; nwords + 1];
        let mut afac = 1.0f64;
        let asign = -1.0f64;
        for n in 1..=NI {
            if n > 1 {
                afac = afac * (2 * n - 1) as f64 / n as f64;
            }
            let mut cf = 1.0f64;
            let no = 1 + n / 2;
            let mut ist = 0i64;
            let mut ifi = 1i64;
            let mut jfi = 1i64;
            let mut ivar = 2i64;
            let aa = (2 * n + 1) as f64 / 2.0;
            xat[n] = cf * afac * aa / (n as i64 + jfi) as f64;
            if no >= 2 {
                let nn = n as i64;
                let kxat = nn - NI as i64;
                for k in 2..=no as i64 {
                    jfi -= 2;
                    let b = ((nn - ist) * (nn - ifi)) as f64;
                    let c = (ivar * (2 * nn - ifi)) as f64;
                    let d = (nn + jfi) as f64;
                    cf = cf * asign * b / c;
                    xat[(NI as i64 * k + kxat) as usize] = cf * afac * aa / d;
                    ist += 2;
                    ifi += 2;
                    ivar += 2;
                }
            }
        }
        xat
    })
}

/// `ptleg`: 33 equiprobable cosine-bin edges from Legendre coefficients
/// `fl = a_1 .. a_NL` (`acefc.f90:2641-2866`); `None` when every coefficient
/// is zero (isotropic).
pub fn ptleg(fl: &[f64]) -> Option<[f64; NPT]> {
    let nord = fl.len();
    if fl.iter().all(|&f| f == 0.0) {
        return None;
    }
    let xat = xat();
    let mut mmfne = 100usize;
    let mfine = 1000usize;
    let mut amu = vec![0.0f64; mfine + 2];
    let mut afnc = vec![0.0f64; mfine + 2];
    let mut acum = vec![0.0f64; mfine + 2];
    let mut expm = vec![0.0f64; nord + 2];
    let delmuf = 0.02f64;
    let mut mm = mmfne + 1;
    amu[1] = -1.0;
    for j in 2..=mm {
        amu[j] = amu[j - 1] + delmuf;
    }
    let nexp = nord + 1;
    let mut sum = 0.0f64;
    let mut iloop = 1;
    while iloop <= 2 {
        for j in 1..=mm {
            let varmu = amu[j];
            expm[1] = varmu;
            for l in 2..=nexp {
                expm[l] = expm[l - 1] * varmu;
                if expm[l].abs() < 1.0e-50 {
                    expm[l] = 0.0;
                }
            }
            let mut afunc = varmu / 2.0;
            for n in 1..=nord {
                let no = 1 + n / 2;
                let mut index = n + 1;
                let temp = fl[n - 1];
                for k in 1..=no {
                    afunc += xat[(k - 1) * NI + n] * expm[index] * temp;
                    index = index.saturating_sub(2);
                }
            }
            afnc[j] = afunc;
        }
        sum = 0.0;
        let mut idone = false;
        let mut j = 0usize;
        while j < mmfne && !idone {
            j += 1;
            let prob = afnc[j + 1] - afnc[j];
            sum += prob;
            if iloop <= 1 && sum > 0.5 {
                idone = true;
            }
            acum[j] = sum;
        }
        if iloop == 2 {
            iloop = 3;
        } else {
            let amumid = amu[j];
            let mfmid = mfine / 2;
            let delmu1 = (amumid + 1.0) / mfmid as f64;
            amu[1] = -1.0;
            let mut mvar = mfmid + 1;
            for jj in 2..=mvar {
                amu[jj] = amu[jj - 1] + delmu1;
            }
            let delmu2 = (1.0 - amumid) / mfmid as f64;
            mvar += 1;
            mm = mfine + 1;
            for jj in mvar..=mm {
                amu[jj] = amu[jj - 1] + delmu2;
            }
            iloop = 2;
            mmfne = mfine;
        }
    }
    let dprob = sum / NBINS as f64;
    let mut acmu = [0.0f64; NPT + 1];
    let mut j = 1usize;
    let mut k = 1usize;
    let mut varmu;
    loop {
        let pro = acum[j];
        varmu = amu[j + 1];
        if pro == sum {
            break;
        }
        let ap = pro - dprob * k as f64;
        if ap == 0.0 {
            acmu[k] = varmu;
            j += 1;
            k += 1;
        } else if ap > 0.0 {
            let b = dprob * k as f64;
            let c = amu[j + 1] - amu[j];
            let d = b - acum[j - 1];
            let g = acum[j] - acum[j - 1];
            varmu = amu[j] + (d * c / g);
            acmu[k] = varmu;
            k += 1;
        } else {
            j += 1;
        }
        if k > NPT || j > mfine {
            break;
        }
    }
    acmu[k.min(NPT)] = varmu;
    let mut out = [0.0f64; NPT];
    out[0] = sigfig(-1.0, 7, 0);
    for l in 1..=NBINS {
        out[l] = sigfig(acmu[l], 7, 0);
    }
    Some(out)
}

/// `summer`'s area: the integral of the table under law `int`
/// (`acefc.f90:3316-3349`).
fn summer(x: &[f64], y: &[f64], int: u32) -> Result<f64, NjoyError> {
    let mut area = 0.0;
    for j in 1..x.len() {
        area += gral(x[j - 1], y[j - 1], x[j], y[j], x[j - 1], x[j], int)?;
    }
    Ok(area)
}

/// `pttab`: 33 equiprobable cosine-bin edges from a tabulated `(mu, p)` law
/// with one interpolation region (`acefc.f90:2977-3281`); `None` for the
/// two-point isotropic table.
///
/// # Errors
/// As upstream: more than one interpolation region, or `INT = 3, 5`.
pub fn pttab(interp: &[(u32, u32)], pairs: &[(f64, f64)]) -> Result<Option<[f64; NPT]>, NjoyError> {
    if interp.len() > 1 {
        return Err(NjoyError::NotPorted("pttab: a tabulated photon angular law with more than one interpolation region"));
    }
    let int = interp.first().map_or(2, |&(_, i)| i);
    if int == 3 || int == 5 {
        return Err(NjoyError::EndfParse("pttab: tab ang dist not allowed for (INT = 3 or 5)".into()));
    }
    let np = pairs.len();
    let mut amu: Vec<f64> = pairs.iter().map(|p| p.0).collect();
    let mut p: Vec<f64> = pairs.iter().map(|p| p.1).collect();
    let sum = summer(&amu, &p, int)?;
    if np == 2 && p[0] == p[1] {
        return Ok(None);
    }
    let dprob = sum / NBINS as f64;
    const ABACK: f64 = 1.0e-4;
    let mut tbmu = [0.0f64; NPT + 1];
    // 1-based views
    let a = |v: &Vec<f64>, i: usize| v[i - 1];
    let mut l = 2usize;
    let mut j = 2usize;
    let mut k = 1usize;
    let mut area1 = 0.0f64;
    let mut aneed = dprob;
    tbmu[1] = amu[0];
    loop {
        let area = match int {
            1 => a(&p, k) * (a(&amu, j) - a(&amu, k)),
            2 => (a(&p, j) + a(&p, k)) * (a(&amu, j) - a(&amu, k)) / 2.0,
            _ => {
                let b = (a(&p, j) / a(&p, k)).ln() / (a(&amu, j) - a(&amu, k));
                if b.abs() < 1.0 / 10000.0 {
                    a(&p, k) * (a(&amu, j) - a(&amu, k))
                } else {
                    a(&p, k) * ((b * (a(&amu, j) - a(&amu, k))).exp() - 1.0) / b
                }
            }
        };
        if area > aneed && l < NPT {
            match int {
                1 => {
                    if a(&p, k) <= 0.0 {
                        p[k - 1] = a(&p, k - 1) / 1000.0;
                    }
                    tbmu[l] = (aneed / a(&p, k)) + a(&amu, k);
                    amu[k - 1] = tbmu[l];
                }
                2 => {
                    let test = 1.0 / 10000.0;
                    let mut slpe = 0.0;
                    if aneed < test {
                        tbmu[l] = a(&amu, k) + aneed / a(&p, k);
                    } else {
                        slpe = (a(&p, j) - a(&p, k)) / (a(&amu, j) - a(&amu, k));
                        if slpe.abs() < test {
                            if a(&p, k) <= 0.0 {
                                p[k - 1] = a(&p, k - 1) / 1000.0;
                            }
                            tbmu[l] = aneed / a(&p, k) + a(&amu, k);
                        } else {
                            let aa = slpe / 2.0;
                            let b = a(&p, k) - slpe * a(&amu, k);
                            let c = (slpe * a(&amu, k) / 2.0 - a(&p, k)) * a(&amu, k) - aneed;
                            let w = b * b - 4.0 * aa * c;
                            let d = w.sqrt();
                            let x1 = (-b + d) / (2.0 * aa);
                            let x2 = (-b - d) / (2.0 * aa);
                            if a(&amu, k) < x1 && x1 <= a(&amu, j) {
                                tbmu[l] = x1;
                            }
                            if a(&amu, k) < x2 && x2 <= a(&amu, j) {
                                tbmu[l] = x2;
                            }
                        }
                    }
                    p[k - 1] = a(&p, k) + slpe * (tbmu[l] - a(&amu, k));
                    amu[k - 1] = tbmu[l];
                }
                _ => {
                    let b = (a(&p, j) / a(&p, k)).ln() / (a(&amu, j) - a(&amu, k));
                    if b.abs() < 1.0 / 10000.0 {
                        tbmu[l] = aneed / a(&p, k) + a(&amu, k);
                    } else {
                        tbmu[l] = (1.0 + b * aneed / a(&p, k)).ln() / b + a(&amu, k);
                    }
                    p[k - 1] = a(&p, k) * (b * (tbmu[l] - a(&amu, k))).exp();
                    amu[k - 1] = tbmu[l];
                }
            }
            area1 = 0.0;
            aneed = dprob;
            l += 1;
        } else {
            area1 += area;
            aneed -= area;
            if l == 2 && area1 < ABACK {
                tbmu[1] = a(&amu, j);
            }
            if j >= np {
                break;
            }
            k += 1;
            j += 1;
        }
    }
    tbmu[l.min(NPT)] = a(&amu, np);
    let mut out = [0.0f64; NPT];
    for i in 1..=NPT {
        out[i - 1] = sigfig(tbmu[i], 7, 0);
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `xat(n)` for `k = 1` is `(2n+1)/2 * afac / (n+1)`; spot-check n = 1, 2.
    #[test]
    fn xat_first_terms() {
        let x = xat();
        assert!((x[1] - 1.5 / 2.0).abs() < 1e-15); // n = 1: 3/2 / 2
        assert!((x[2] - 2.5 * 1.5 / 3.0).abs() < 1e-15); // n = 2: 5/2 * (3/2) / 3
    }

    /// Isotropic in, `None` out, for both conversions.
    #[test]
    fn isotropic_is_none() {
        assert!(ptleg(&[0.0, 0.0]).is_none());
        assert!(pttab(&[(2, 2)], &[(-1.0, 0.5), (1.0, 0.5)]).unwrap().is_none());
    }

    /// A linear law `f(mu) = (1 + 3 a mu)/2` gives monotone edges from -1 to 1
    /// whose bins each carry 1/32 of the probability.
    #[test]
    fn ptleg_bins_are_equiprobable() {
        let a1 = 0.2;
        let e = ptleg(&[a1]).expect("anisotropic");
        assert_eq!(e[0], sigfig(-1.0, 7, 0));
        assert!((e[NPT - 1] - 1.0).abs() < 1e-6);
        let cdf = |m: f64| (m + 1.0) / 2.0 + 0.75 * a1 * (m * m - 1.0);
        for w in e.windows(2) {
            assert!(w[1] > w[0]);
            assert!((cdf(w[1]) - cdf(w[0]) - 1.0 / 32.0).abs() < 2e-4, "{w:?}");
        }
    }
}
