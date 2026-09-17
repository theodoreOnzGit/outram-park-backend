// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine rpxunr`, l.4785-5013 — unresolved range (LRU=2): +1 % one-sided
//     sensitivities of the SLBW-averaged cross sections to every (L, J) parameter,
//     folded with the relative parameter covariance into `uff/ugg/uee/utt` and
//     `uef/ueg/ufg`.
//   - `subroutine ggunr1`, l.6800-6905 — unresolved SLBW cross sections with the
//     width-fluctuation integrals (`egnrl`) at one energy.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Unresolved resonance ranges of MF=32 (`LRU=2`, the `LRF=1`-style
//! parameter layout `(D, AJ, GNO, GG, GF, GX)` per `(L, J)` with one
//! relative covariance LIST of `NPAR = MPAR * ΣNJS` parameters).
//!
//! The degrees of freedom `(AMUN, AMUF, AMUX)` per `(L, J)` are *not* in
//! MF=32; upstream takes them from `amur`, which `rdumrd2` fills only when
//! the MF=2 URR is `LRF=2` (energy-dependent parameters). A material whose
//! MF=2 URR is `LRF=1` leaves `amur` uninitialised in NJOY2016; this port
//! refuses that case (`NotPorted`) rather than guess.
//!
//! The sensitivity is one-sided: each parameter `p` is scaled by `1.01`,
//! `s = 100 (σ_g(1.01 p) − σ_g(p)) / p` (i.e. `∂σ_g/∂p` for the 1 % step),
//! dropped when `|s| < 1e-10`, and multiplied by `cflx(ig)·ABN`; the
//! relative covariance is scaled by `p_i p_j` (`errorr.f90:4959-4967`).

use crate::common::phys::{AMASSN_AMU, PI};
use crate::endf::records::SectionCursor;
use crate::reconr::slbw::WAVE_K;
use crate::NjoyError;

use super::super::math::{egnrl, eunfac};
use super::super::weight::ErrorrWeight;
use super::group::{rpxgrp, SigRow};
use super::mlbw::{RC1, RC2, THIRD};
use super::{RangeParams, ResonanceCovariance};

/// Unresolved SLBW cross sections `(σ_t, σ_el, σ_f, σ_γ)` \[b\] at `e`
/// \[eV\] (`ggunr1`, `errorr.f90:6800-6905`).
///
/// `a` is the MF=32 URR block: the `(SPI, AP, 0, 0, NLS, 0)` CONT then per
/// L the LIST head `(AWRI, 0, L, 0, 6*NJS, NJS)` and `6*NJS` words
/// `(D, AJ, GNO, GG, GF, GX)`. `amur[nlru2]` = `(AMUN, AMUF, AMUX)` in
/// the same `(L, J)` order.
///
/// # Errors
/// `EndfParse` when a quadrature selector needed by `egnrl` is outside
/// `1..=4` (upstream would index past its table).
pub fn ggunr1(e: f64, a: &[f64], amur: &[[f64; 3]]) -> Result<[f64; 4], NjoyError> {
    let mut sigp = [0.0f64; 4];
    let spi = a[0];
    let ap = a[1];
    let nls = a[4].round() as usize;
    let mut nlru2 = 0usize;
    let mut inow = 6usize;
    let mut spot = 0.0;
    for _l in 0..nls {
        let awri = a[inow];
        let ll = a[inow + 2].round() as i32;
        let njs = a[inow + 5].round() as usize;
        let arat = awri / (awri + 1.0);
        let aw = awri * AMASSN_AMU;
        let ra = RC1 * aw.powf(THIRD) + RC2;
        let konst = (2.0 * PI * PI) / (WAVE_K * arat).powi(2);
        inow += 6;
        for j in 0..njs {
            let dx = a[inow];
            let aj = a[inow + 1];
            let gnox = a[inow + 2];
            let ggx = a[inow + 3];
            let gfx = a[inow + 4];
            let gxx = a[inow + 5];
            let am = amur.get(nlru2).ok_or_else(|| {
                NjoyError::EndfParse(
                    "errorr::ggunr1: fewer (L,J) degrees of freedom in MF=2 than MF=32 URR states"
                        .into(),
                )
            })?;
            nlru2 += 1;
            let amun = am[0];
            let mu = am[0].round() as i64;
            let nu = am[1].round() as i64;
            let lamda = am[2].round() as i64;
            let gj = (2.0 * aj + 1.0) / (4.0 * spi + 2.0);
            let e2 = e.sqrt();
            let k = arat * e2 * WAVE_K;
            let rho = k * ra;
            let rhoc = k * ap;
            // penetrability and phase shift
            let (vl0, ps) = eunfac(ll, rho, rhoc, amun);
            let vl = vl0 * e2;
            // potential scattering
            if j == 0 {
                spot = 4.0 * PI * (2.0 * ll as f64 + 1.0) * (ps.sin() / k).powi(2);
            }
            // cross section contributions
            let gnx = gnox * vl;
            let diff = gxx;
            let den = e * dx;
            let temp = konst * gj * gnx / den;
            let terg = temp * ggx;
            let ters = temp * gnx;
            let terf = temp * gfx;
            // fluctuation integrals: guard the selectors egnrl will index
            let sel = |v: i64, name: &str| -> Result<usize, NjoyError> {
                if (1..=4).contains(&v) {
                    Ok(v as usize)
                } else {
                    Err(NjoyError::EndfParse(format!(
                        "errorr::ggunr1: egnrl selector {name}={v} outside 1..=4"
                    )))
                }
            };
            // egnrl returns 0 without indexing unless Γn > 0, Γγ > 0, Γf >= 0
            // and not (Γf > 0 with Γx < 0); mirror that before validating.
            let indexes = gnx > 0.0 && ggx > 0.0 && gfx >= 0.0 && !(gfx > 0.0 && diff < 0.0);
            let (gs, gc, gff) = if indexes {
                let mu_ = sel(mu, "mu")?;
                let nu_ = if gfx > 0.0 { sel(nu, "nu")? } else { 1 };
                let lamda_ = if diff > 0.0 { sel(lamda, "lamda")? } else { 1 };
                (
                    egnrl(gnx, gfx, ggx, mu_, nu_, lamda_, diff, 1),
                    egnrl(gnx, gfx, ggx, mu_, nu_, lamda_, diff, 2),
                    egnrl(gnx, gfx, ggx, mu_, nu_, lamda_, diff, 3),
                )
            } else {
                (0.0, 0.0, 0.0)
            };
            let gc = gc * terg;
            let gff = gff * terf;
            let mut gs = gs * ters;
            // interference correction
            let add = konst * gj * 2.0 * gnx * ps.sin().powi(2) / (e * dx);
            gs -= add;
            sigp[1] += gs;
            sigp[2] += gff;
            sigp[3] += gc;
            inow += 6;
        }
        sigp[1] += spot;
    }
    sigp[0] = sigp[1] + sigp[2] + sigp[3];
    Ok(sigp)
}

/// Unresolved range sensitivities and covariance accumulation
/// (`rpxunr`, `errorr.f90:4785-5013`).
///
/// `cur` stands at the first L-state LIST of the MF=32 URR block; `a0`
/// must already hold the `(SPI, AP, 0, 0, NLS, 0)` CONT as six words.
///
/// # Errors
/// `EndfParse` for a covariance LIST shorter than `NPAR(NPAR+1)/2`, plus
/// whatever [`ggunr1`] and [`rpxgrp`] report.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn rpxunr(
    cur: &mut SectionCursor,
    a0: [f64; 6],
    rp: &RangeParams,
    amur: &[[f64; 3]],
    egn: &[f64],
    cflx: &[f64],
    weight: &ErrorrWeight,
    tempin: f64,
    rc: &mut ResonanceCovariance,
) -> Result<(), NjoyError> {
    let ngn = egn.len() - 1;
    let (iest, ieed) = (rp.iest, rp.ieed);
    // the parameter block b(1:l0): CONT + every L LIST (l.4830-4842)
    let mut base: Vec<f64> = a0.to_vec();
    for _ in 0..rp.nls {
        let l = cur.read_list()?;
        base.extend_from_slice(&[
            l.head.c1,
            l.head.c2,
            l.head.l1 as f64,
            l.head.l2 as f64,
            l.head.n1 as f64,
            l.head.n2 as f64,
        ]);
        base.extend_from_slice(&l.data);
    }
    let covl = cur.read_list()?;
    let mpar = covl.head.l1 as usize;
    let npar = covl.head.n2 as usize;
    if covl.data.len() < npar * (npar + 1) / 2 {
        return Err(NjoyError::EndfParse(
            "errorr::rpxunr: relative covariance LIST shorter than NPAR(NPAR+1)/2".into(),
        ));
    }
    let lfw = rp.lfw;

    // sens[i][ig] (1-based) and the saved parameter values b(l0+1+i)
    let mut sens = vec![[0.0f64; 4]; (npar + 1) * (ngn + 1)];
    let mut saved = vec![0.0f64; npar + 1];
    let mut gsigr: Vec<[f64; 4]> = Vec::new();
    let mut njs = 0usize;
    let mut inow = 5usize; // Fortran inow = 6 (1-based) -> the CONT's last word

    for loop_ in 1..=npar + 1 {
        let mut b = base.clone();
        if loop_ > 1 {
            let loopn = (loop_ - 1) % mpar;
            if loopn == 1 || mpar == 1 {
                if njs == 0 {
                    njs = b[inow + 6].round() as usize;
                } else {
                    njs -= 1;
                }
                inow += 6;
                saved[loop_ - 1] = b[inow + 1];
                b[inow + 1] *= 1.01;
            } else if loopn == 2 || (mpar == 2 && loopn == 0) {
                saved[loop_ - 1] = b[inow + 3];
                b[inow + 3] *= 1.01;
            } else if loopn == 3 || (mpar == 3 && loopn == 0) {
                saved[loop_ - 1] = b[inow + 4];
                b[inow + 4] *= 1.01;
            } else if loopn == 4 || (mpar == 4 && loopn == 0) {
                if lfw == 1 {
                    saved[loop_ - 1] = b[inow + 5];
                    b[inow + 5] *= 1.01;
                } else if lfw == 0 {
                    saved[loop_ - 1] = b[inow + 6];
                    b[inow + 6] *= 1.01;
                }
            } else if loopn == 5 || (mpar == 5 && loopn == 0) {
                saved[loop_ - 1] = b[inow + 6];
                b[inow + 6] *= 1.01;
            }
            if loopn == 0 && njs == 1 {
                inow += 6;
                njs = 0;
            }
        }
        // pointwise cross sections on the 1.5 % grid (l.4906-4933)
        let mut sig: Vec<SigRow> = Vec::new();
        let mut e1 = rp.elg;
        loop {
            if e1 > rp.ehg {
                e1 = rp.ehg;
            }
            let s = if e1 >= rp.elr && e1 <= rp.ehr {
                ggunr1(e1, &b, amur)?
            } else {
                [0.0; 4]
            };
            sig.push([s[0], s[1], s[2], s[3], e1]);
            if e1 >= rp.ehg {
                break;
            }
            e1 *= 1.015;
            let ebc = e1 / 1.015;
            if ebc < rp.elr && e1 > rp.elr {
                e1 = rp.elr;
            }
            if ebc < rp.ehr && e1 > rp.ehr {
                e1 = rp.ehr;
            }
        }
        if loop_ == 1 {
            gsigr = rpxgrp(egn, &sig, weight, tempin)?;
        } else {
            let gsigp = rpxgrp(egn, &sig, weight, tempin)?;
            let i = loop_ - 1;
            let p = saved[i];
            for ig in iest..=ieed {
                for j in 0..4 {
                    let sfac = gsigr[ig - 1][j];
                    if p != 0.0 {
                        let s = 100.0 * (gsigp[ig - 1][j] - sfac) / p;
                        if s.abs() >= 1.0e-10 {
                            sens[i * (ngn + 1) + ig][j] = s * cflx[ig - 1] * rp.abn;
                        }
                    }
                }
            }
        }
    }

    // absolute parameter covariance (l.4959-4967)
    let mut cov = vec![vec![0.0f64; npar + 1]; npar + 1];
    let mut l3 = 0usize;
    for i in 1..=npar {
        for j in i..=npar {
            let bb = saved[i] * saved[j];
            let tmp = covl.data[l3] * bb;
            l3 += 1;
            cov[i][j] = tmp;
            cov[j][i] = tmp;
        }
    }

    let s = |i: usize, ig: usize| sens[i * (ngn + 1) + ig];
    let in_range = |g: usize| g >= iest && g <= ieed;
    let mut igind = 0usize;
    for ig in 1..=ngn {
        for ig2 in ig..=ngn {
            if in_range(ig) && in_range(ig2) {
                for i in 1..=npar {
                    for j in 1..=npar {
                        let c = cov[i][j];
                        if c == 0.0 {
                            continue;
                        }
                        rc.uff[igind] += c * s(i, ig)[2] * s(j, ig2)[2];
                        rc.ugg[igind] += c * s(i, ig)[3] * s(j, ig2)[3];
                        rc.uee[igind] += c * s(i, ig)[1] * s(j, ig2)[1];
                        rc.utt[igind] += c * s(i, ig)[0] * s(j, ig2)[0];
                    }
                }
            }
            igind += 1;
            if ig > rc.nresg {
                rc.nresg = ig;
            }
        }
    }
    let mut igind = 0usize;
    for ig in 1..=ngn {
        for ig2 in 1..=ngn {
            for i in 1..=npar {
                let si = s(i, ig);
                if si == [0.0; 4] {
                    continue;
                }
                for j in 1..=npar {
                    let c = cov[i][j];
                    if c == 0.0 {
                        continue;
                    }
                    rc.uef[igind] += c * si[1] * s(j, ig2)[2];
                    rc.ueg[igind] += c * si[1] * s(j, ig2)[3];
                    rc.ufg[igind] += c * si[2] * s(j, ig2)[3];
                }
            }
            igind += 1;
        }
    }
    rc.ifunrs = true;
    Ok(())
}
