// Ported from NJOY2016 `src/samm.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine abpart`, l.2950-3006 — `upr`/`upi` and `pr`/`pii` (energy-dependent `dR/du`).
//   - `subroutine setqri`, l.6499-6556 — `qr`/`qi` = `dXXXX/dR`.
//   - `subroutine settri`, l.6558-6674 (the angle-integrated part) — `tr`/`ti` = ½ `dσ/dR`.
//   - `subroutine derres`, l.6811-6850 — `deriv` = `dσ/du` for one spin group.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The per-energy pieces of the resonance-parameter derivatives. Every
//! matrix here is indexed the way `samm.f90` packs it: the triangle of a
//! spin group's `nchan` channels runs `ij = 1..nn` in the order
//! `(1,1),(2,1),(2,2),(3,1),…` (`ij = i(i-1)/2 + j`, `j <= i`), stored
//! 0-based.

use crate::samm::linpack::PackedComplexMatrix;
use crate::samm::xsformula::abpart::AlphaTerms;

use super::DerivSetup;

/// `pr(ij, ipar)` / `pii(ij, ipar)` — `dR/du` at the current energy, per
/// parameter over its own group's packed triangle, stored flat with a
/// fixed `stride` (the largest group triangle) so one evaluation makes
/// two allocations, not `2·npar`.
#[derive(Debug, Clone)]
pub struct ParamEnergyTerms {
    pub stride: usize,
    pub pr: Vec<f64>,
    pub pi: Vec<f64>,
}

impl ParamEnergyTerms {
    #[inline]
    pub fn pr_row(&self, ipar: usize) -> &[f64] {
        &self.pr[ipar * self.stride..(ipar + 1) * self.stride]
    }
    #[inline]
    pub fn pi_row(&self, ipar: usize) -> &[f64] {
        &self.pi[ipar * self.stride..(ipar + 1) * self.stride]
    }
}

/// `abpart`'s derivative half (`samm.f90:2950-3006`): `upr`/`upi` from the
/// Breit-Wigner terms of each parameter's resonance, then `pr = br·upr`,
/// `pii = bi·upi`. `alpha[g][ires]` is [`crate::samm::xsformula::abpart::abpart`]'s
/// output for every group at this energy.
#[allow(clippy::needless_range_loop)]
pub fn abpart_derivs(ds: &DerivSetup, nchan: &[usize], alpha: &[Vec<AlphaTerms>]) -> ParamEnergyTerms {
    let stride = ds.mchan * (ds.mchan + 1) / 2;
    let mut pr = vec![0.0f64; ds.npar * stride];
    let mut pi = vec![0.0f64; ds.npar * stride];
    let mut ipar = 0usize;
    for (g, al) in alpha.iter().enumerate() {
        let n2 = nchan[g] + 2;
        for a in al {
            for m in 1..=n2 {
                let mut upr = a.alphar;
                let mut upi = a.alphai;
                if m == 1 {
                    // variable is resonance-energy
                    upi *= upr;
                    upr = a.xden - 2.0 * upr * upr;
                } else if m == 2 {
                    // variable is capture width
                    upr *= upi;
                    upi = a.xden - 2.0 * upi * upi;
                }
                let brk = &ds.br[ipar];
                let bik = &ds.bi[ipar];
                if upr != 0.0 || upi != 0.0 {
                    let base = ipar * stride;
                    for ij in 0..brk.len() {
                        if brk[ij] != 0.0 {
                            pr[base + ij] = brk[ij] * upr;
                        }
                        if bik[ij] != 0.0 {
                            pi[base + ij] = bik[ij] * upi;
                        }
                    }
                }
                ipar += 1;
            }
        }
    }
    debug_assert_eq!(ipar, ds.npar);
    ParamEnergyTerms { stride, pr, pi }
}

#[inline]
fn ijkl(m: usize, n: usize) -> usize {
    // samm.f90:6442-6454, 1-based in and out
    if m <= n {
        (n * (n - 1)) / 2 + m
    } else {
        (m * (m - 1)) / 2 + n
    }
}

/// `qr(kl, ij)` / `qi(kl, ij)` — the real and imaginary parts of
/// `dXXXX(kl)/dR(ij)` (`setqri`, `samm.f90:6499-6556`), `nn × nn` with
/// `nn = nchan(nchan+1)/2`, flattened `[kl][ij]` 0-based.
pub struct QMatrix {
    pub nn: usize,
    pub qr: Vec<f64>,
    pub qi: Vec<f64>,
}

impl QMatrix {
    #[inline]
    pub fn r(&self, kl: usize, ij: usize) -> f64 {
        self.qr[(kl - 1) * self.nn + (ij - 1)]
    }
    #[inline]
    pub fn i(&self, kl: usize, ij: usize) -> f64 {
        self.qi[(kl - 1) * self.nn + (ij - 1)]
    }
}

/// `setqri` (`samm.f90:6499-6556`): redefine `XQ = sqrt(P)/L · Y⁻¹ ·
/// psmall`, then `Q(kl, ij) = XQ(i,k) XQ(j,l) (+ XQ(j,k) XQ(i,l) for i≠j)`.
/// `rootp`/`elinvr`/`elinvi`/`psmall` are [`crate::samm::xsformula::setr::setr`]'s
/// outputs; `yinv` is the inverted level matrix.
#[allow(clippy::needless_range_loop)]
pub fn setqri(
    nchan: usize,
    rootp: &[f64],
    elinvr: &[f64],
    elinvi: &[f64],
    psmall: &[f64],
    yinv: &PackedComplexMatrix,
) -> QMatrix {
    // xq[k][i] (1-based in the Fortran, xqr(k,i,ier)); stored 0-based [k-1][i-1]
    let mut xqr = vec![vec![0.0f64; nchan]; nchan];
    let mut xqi = vec![vec![0.0f64; nchan]; nchan];
    for i in 1..=nchan {
        let plri = rootp[i - 1] * elinvr[i - 1];
        let plii = rootp[i - 1] * elinvi[i - 1];
        for k in 1..=nchan {
            let ik = ijkl(i, k);
            let yr = yinv.re[ik - 1];
            let yi = yinv.im[ik - 1];
            let mut r = plri * yr - plii * yi;
            let mut im = plri * yi + plii * yr;
            if psmall[k - 1] != 0.0 {
                r *= psmall[k - 1];
                im *= psmall[k - 1];
            }
            xqr[k - 1][i - 1] = r;
            xqi[k - 1][i - 1] = im;
        }
    }
    let nn = nchan * (nchan + 1) / 2;
    let mut qr = vec![0.0f64; nn * nn];
    let mut qi = vec![0.0f64; nn * nn];
    let mut ij = 0usize;
    for i in 1..=nchan {
        for j in 1..=i {
            ij += 1;
            let mut kl = 0usize;
            for k in 1..=nchan {
                for l in 1..=k {
                    kl += 1;
                    // xqr(i,k) in the Fortran is the [i][k] element of the array
                    // filled as xqr(k,i) above -> our xqr[i-1][k-1]
                    let (rik, iik) = (xqr[i - 1][k - 1], xqi[i - 1][k - 1]);
                    let (rjl, ijl) = (xqr[j - 1][l - 1], xqi[j - 1][l - 1]);
                    let mut vr = rik * rjl - iik * ijl;
                    let mut vi = rik * ijl + iik * rjl;
                    if i != j {
                        let (rjk, ijk) = (xqr[j - 1][k - 1], xqi[j - 1][k - 1]);
                        let (ril, iil) = (xqr[i - 1][l - 1], xqi[i - 1][l - 1]);
                        vr += rjk * ril - ijk * iil;
                        vi += rjk * iil + ijk * ril;
                    }
                    qr[(kl - 1) * nn + (ij - 1)] = vr;
                    qi[(kl - 1) * nn + (ij - 1)] = vi;
                }
            }
        }
    }
    QMatrix { nn, qr, qi }
}

/// `tr(m, ij)` / `ti(m, ij)` — ½ the real/imaginary parts of `dσ_m/dR(ij)`
/// without the `4π/E` factor (`settri`, angle-integrated part,
/// `samm.f90:6558-6674`). Rows `m = 1, 2` are elastic and absorption, rows
/// `m >= 3` are reaction channels by particle-pair number (as `sectio`).
pub struct TMatrix {
    pub nn: usize,
    /// `[m][ij]`, `npp × nn`.
    pub tr: Vec<Vec<f64>>,
    pub ti: Vec<Vec<f64>>,
}

/// `settri` (`samm.f90:6558-6674`). `zke`/`particle_pair`/`sinsqr`/`sin2ph`
/// are per channel (trimmed `nchan`); `xxxxr`/`xxxxi` the packed `XXXX` of
/// this group at this energy; `q` from [`setqri`].
#[allow(clippy::needless_range_loop, clippy::too_many_arguments)]
pub fn settri(
    npp: usize,
    nent: usize,
    nchan: usize,
    zke: &[f64],
    particle_pair: &[usize],
    sinsqr: &[f64],
    sin2ph: &[f64],
    xxxxr: &[f64],
    xxxxi: &[f64],
    q: &QMatrix,
) -> TMatrix {
    let nn = q.nn;
    let mut tr = vec![vec![0.0f64; nn]; npp];
    let mut ti = vec![vec![0.0f64; nn]; npp];

    // integrated elastic, diagonal in channel numbers (l.6583-6601)
    let mut kl = 0usize;
    for k in 1..=nent {
        let zz = zke[k - 1] * zke[k - 1];
        kl += k;
        for ij in 1..=nn {
            let (qr, qi) = (q.r(kl, ij), q.i(kl, ij));
            if qi != 0.0 || qr != 0.0 {
                let ar = qr * (-sin2ph[k - 1] * 0.5) + qi * (-sinsqr[k - 1]);
                let ai = qi * (-sin2ph[k - 1] * 0.5) - qr * (-sinsqr[k - 1]);
                tr[0][ij - 1] += ar / zz;
                ti[0][ij - 1] += ai / zz;
            }
        }
    }

    // absorption only, diagonal in channel numbers (l.6603-6618)
    let mut kl = 0usize;
    for k in 1..=nent {
        let zz = 2.0 * zke[k - 1] * zke[k - 1];
        kl += k;
        for ij in 1..=nn {
            let (qr, qi) = (q.r(kl, ij), q.i(kl, ij));
            if qi != 0.0 {
                tr[1][ij - 1] += qi / zz;
            }
            if qr != 0.0 {
                ti[1][ij - 1] -= qr / zz;
            }
        }
    }

    // not-necessarily diagonal pieces of elastic and capture (l.6620-6646)
    let mut kl = 0usize;
    for k in 1..=nent {
        let zz = zke[k - 1] * zke[k - 1];
        for l in 1..=k {
            kl += 1;
            for ij in 1..=nn {
                let (qr, qi) = (q.r(kl, ij), q.i(kl, ij));
                if qi != 0.0 || qr != 0.0 {
                    let mut ar = qr * xxxxr[kl - 1] + qi * xxxxi[kl - 1];
                    let mut ai = qi * xxxxr[kl - 1] - qr * xxxxi[kl - 1];
                    if k != l {
                        ar *= 2.0;
                        ai *= 2.0;
                    }
                    tr[0][ij - 1] += ar / zz;
                    ti[0][ij - 1] += ai / zz;
                    tr[1][ij - 1] -= ar / zz;
                    ti[1][ij - 1] -= ai / zz;
                }
            }
        }
    }

    // reactions (l.6648-6672)
    if nchan > nent {
        let mut kl = 0usize;
        for k in 1..=nchan {
            let m = particle_pair[k - 1];
            for l in 1..=k {
                kl += 1;
                if l <= nent && k > nent {
                    let zz = zke[l - 1] * zke[l - 1];
                    for ij in 1..=nn {
                        let (qr, qi) = (q.r(kl, ij), q.i(kl, ij));
                        if qi != 0.0 || qr != 0.0 {
                            tr[m - 1][ij - 1] += (qr * xxxxr[kl - 1] + qi * xxxxi[kl - 1]) / zz;
                            ti[m - 1][ij - 1] += (qi * xxxxr[kl - 1] - qr * xxxxi[kl - 1]) / zz;
                        }
                    }
                }
            }
        }
    }

    TMatrix { nn, tr, ti }
}

/// `derres` (`samm.f90:6811-6850`): add `goj · Σ_ij [pr(ij,m) tr(ip,ij) -
/// pii(ij,m) ti(ip,ij)]` to `deriv[ip][m]` for this group's parameters
/// `m = kstart .. kstart + npr` (0-based). `deriv` may be the running
/// `dsigma` itself: upstream zeroes a per-group `deriv` and then adds it
/// into `dsigma` (`samm.f90:3164-3169`), and only this group's columns
/// are touched here, so accumulating directly is the same sum.
#[allow(clippy::needless_range_loop, clippy::too_many_arguments)]
pub fn derres(
    npp: usize,
    nchan: usize,
    goj: f64,
    kstart: usize,
    npr: usize,
    terms: &ParamEnergyTerms,
    t: &TMatrix,
    deriv: &mut [Vec<f64>],
) {
    let nn = nchan * (nchan + 1) / 2;
    let mut ddddd = vec![0.0f64; npp];
    for mm in 0..npr {
        let m = kstart + mm;
        ddddd.iter_mut().for_each(|v| *v = 0.0);
        let prm = terms.pr_row(m);
        let pim = terms.pi_row(m);
        for ij in 0..nn {
            if pim[ij] != 0.0 {
                for ip in 0..npp {
                    ddddd[ip] -= pim[ij] * t.ti[ip][ij];
                }
            }
            if prm[ij] != 0.0 {
                for ip in 0..npp {
                    ddddd[ip] += prm[ij] * t.tr[ip][ij];
                }
            }
        }
        for ip in 0..npp {
            deriv[ip][m] += goj * ddddd[ip];
        }
    }
}
