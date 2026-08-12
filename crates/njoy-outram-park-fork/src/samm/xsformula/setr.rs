//! R-matrix and level-matrix assembly at a single incident energy — ported
//! from `setr` (`samm.f90:3231-3476`).
//!
//! Builds the packed complex-symmetric `R` matrix (`R_{cc'} = sum_lambda
//! beta_{lambda,c} beta_{lambda,c'} / (E_lambda - E - i*Gamma_gamma/2)`)
//! and the level matrix `Y = L^-1 - R` (`L` diagonal, carrying the
//! channel penetrability/shift/boundary term) that
//! [`crate::samm::rmatrix_invert::invert`] (Phase 4) inverts.
//!
//! Uses the same packed complex-symmetric storage and 1-indexed-flat-offset
//! convention as [`crate::samm::linpack`]/[`crate::samm::rmatrix_invert`] —
//! see those modules' doc comments for why.
//!
//! **Not ported here (deferred, see this crate's `README.md`):** background
//! R-matrix elements (`backgr`/`backgrdata` — not parsed by
//! [`crate::samm::mf2`] either, a secondary LRF=7 feature) and the
//! `Want_Angular_Dist`-gated `cscs` (cross-phase cosine/sine) bookkeeping,
//! which has no caller until angular distributions are built alongside
//! `ERRORR`.

use crate::samm::betset::{beta_triangular_index, ResonanceAmplitudes};
use crate::samm::context::ChannelKinematics;
use crate::samm::linpack::PackedComplexMatrix;
use crate::samm::mf2::{ParticlePair, SpinGroup};
use crate::samm::penetrability::{pgh, sinsix};
use crate::samm::coulomb::pghcou;
use crate::samm::xsformula::abpart::AlphaTerms;

#[inline]
fn g(v: &[f64], k: i64) -> f64 {
    v[(k - 1) as usize]
}
#[inline]
fn s(v: &mut [f64], k: i64, x: f64) {
    v[(k - 1) as usize] = x;
}
/// `v[k] += delta` -- avoids the borrow conflict of `s(v, k, g(v, k) +
/// delta)` (mutable + immutable borrow of the same buffer in one call).
#[inline]
fn add_at(v: &mut [f64], k: i64, delta: f64) {
    let x = g(v, k) + delta;
    s(v, k, x);
}
/// `v[k] *= factor`.
#[inline]
fn mul_at(v: &mut [f64], k: i64, factor: f64) {
    let x = g(v, k) * factor;
    s(v, k, x);
}
/// `v[k] = v[k]*factor - 1.0`.
#[inline]
fn mul_sub1_at(v: &mut [f64], k: i64, factor: f64) {
    let x = g(v, k) * factor - 1.0;
    s(v, k, x);
}
#[inline]
fn packed(i: i64, j: i64) -> i64 {
    // 1-indexed packed position of (i,j), i<=j, matching samm.f90's own
    // upper-triangle-by-column packing (see linpack.rs's doc comment).
    if i <= j {
        i + j * (j - 1) / 2
    } else {
        j + i * (i - 1) / 2
    }
}

/// Output of [`setr`]: the assembled `R` and `Y` matrices, per-channel
/// kinematic scratch ([`rootp`](Self::rootp) etc., needed by
/// [`super::assembly::setxqx`] and [`super::sectio::sectio`]), and the
/// possibly-reduced channel count (upstream trims trailing channels whose
/// threshold is above the current energy, `samm.f90:3248-3256`).
pub struct SetrOutput {
    pub rmat: PackedComplexMatrix,
    pub ymat: PackedComplexMatrix,
    /// `true` if the R-matrix is (numerically) all-zero for this group at
    /// this energy — `setxqx`/full inversion can be skipped in favor of
    /// [`crate::samm::rmatrix_invert::zero_triangular`] (`lrmat` upstream).
    pub lrmat_trivial: bool,
    pub rootp: Vec<f64>,
    pub elinvr: Vec<f64>,
    pub elinvi: Vec<f64>,
    pub sinsqr: Vec<f64>,
    pub sin2ph: Vec<f64>,
    /// Channel count actually used (`<=` the group's full channel count).
    pub nchan: usize,
}

/// Assemble the R-matrix and level matrix for one spin group at incident
/// (CM) energy `energy` (eV) — ported from `setr` (`samm.f90:3231-3476`).
///
/// `kinematics`/`amplitudes` must be
/// [`crate::samm::context::compute_channel_kinematics`]'s and
/// [`crate::samm::betset::compute_resonance_amplitudes`]'s output for this
/// same `group`; `alpha` must be [`super::abpart::abpart`]'s output at this
/// same `energy`.
pub fn setr(
    group: &SpinGroup,
    kinematics: &[ChannelKinematics],
    pairs: &[ParticlePair],
    amplitudes: &[ResonanceAmplitudes],
    alpha: &[AlphaTerms],
    energy: f64,
) -> SetrOutput {
    // samm.f90:3248-3256 -- trim trailing channels whose threshold is
    // above the current energy.
    let mut nchan = group.channels.len();
    if energy >= 0.0 {
        while nchan > 0 && energy <= kinematics[nchan - 1].echan {
            nchan -= 1;
        }
    }

    let mut rmat = PackedComplexMatrix::zeros(nchan);
    let mut ymat = PackedComplexMatrix::zeros(nchan);

    // samm.f90:3297-3313 -- accumulate resonance contributions
    // R_{cc'} += alphar*beta_{cc'} + i*alphai*beta_{cc'}, restricted to
    // channels both above their own threshold.
    for (res_amp, a) in amplitudes.iter().zip(alpha.iter()) {
        for k in 1..=nchan {
            for l in 1..=k {
                if energy > kinematics[k - 1].echan && energy > kinematics[l - 1].echan {
                    let beta_kl = res_amp.beta[beta_triangular_index(k - 1, l - 1)];
                    if beta_kl != 0.0 {
                        let kl = packed(l as i64, k as i64);
                        add_at(&mut rmat.re, kl, a.alphar * beta_kl);
                        add_at(&mut rmat.im, kl, a.alphai * beta_kl);
                    }
                }
            }
        }
    }

    // samm.f90:3315-3325 -- check if the R-matrix is entirely zero.
    let lrmat_trivial = rmat.re.iter().all(|&x| x == 0.0) && rmat.im.iter().all(|&x| x == 0.0);

    // samm.f90:3327-3335 -- Ymat = -Rmat initially.
    for i in 0..ymat.re.len() {
        ymat.re[i] = -rmat.re[i];
        ymat.im[i] = -rmat.im[i];
    }

    let mut rootp = vec![0.0_f64; nchan];
    let mut elinvr = vec![0.0_f64; nchan];
    let mut elinvi = vec![0.0_f64; nchan];
    let mut sinsqr = vec![0.0_f64; nchan];
    let mut sin2ph = vec![0.0_f64; nchan];
    let mut psmall = vec![0.0_f64; nchan];

    const TINY: f64 = 1.0e-8;

    for i in 1..=nchan {
        let ch = &group.channels[i - 1];
        let ipx = ch.particle_pair; // 1-indexed particle pair
        let pair = &pairs[ipx - 1];
        let ii = packed(i as i64, i as i64); // diagonal packed position

        rootp[i - 1] = 1.0;
        elinvr[i - 1] = 0.0;
        elinvi[i - 1] = -1.0;
        let mut iffy = false;

        if energy > kinematics[i - 1].echan {
            if pair.penetrability_flag <= 0 {
                // samm.f90:3352-3355 -- penetrability not calculated.
                add_at(&mut ymat.im, ii, -1.0);
            } else {
                let ex = (energy - kinematics[i - 1].echan).sqrt();
                let rho = kinematics[i - 1].zkte * ex;
                let rhof = kinematics[i - 1].zkfe * ex;

                let (hr, hi, p, dp);
                if kinematics[i - 1].zeta == 0.0 {
                    let sx = sinsix(rhof, ch.l);
                    sinsqr[i - 1] = sx.sinsqr;
                    sin2ph[i - 1] = sx.sin2ph;
                    let pg = pgh(rho, ch.l, ch.boundary, pair.shift_flag);
                    hr = pg.g;
                    hi = pg.h;
                    p = pg.p;
                    dp = pg.dp;
                    iffy = pg.iffy;
                } else {
                    // samm.f90:3378-3396 -- Coulomb penetrability/shift/
                    // phase-shift. When rho==rhof (effective and true
                    // channel radii coincide) a single pghcou call at rho
                    // gives everything; otherwise rho (penetrability/shift)
                    // and rhof (phase shift alone) need separate calls.
                    let eta = kinematics[i - 1].zeta / ex;
                    let sinphi;
                    let cosphi;
                    if rho == rhof {
                        match pghcou(rho, ch.l, ch.boundary, pair.shift_flag, eta, true) {
                            Some(pc) => {
                                hr = pc.g;
                                hi = pc.h;
                                p = pc.p;
                                dp = pc.dp;
                                iffy = pc.iffy;
                                sinphi = pc.sinphi;
                                cosphi = pc.cosphi;
                            }
                            None => {
                                hr = 0.0;
                                hi = 0.0;
                                p = 0.0;
                                dp = 0.0;
                                sinphi = 0.0;
                                cosphi = 1.0;
                            }
                        }
                    } else {
                        match pghcou(rho, ch.l, ch.boundary, pair.shift_flag, eta, false) {
                            Some(pc) => {
                                hr = pc.g;
                                hi = pc.h;
                                p = pc.p;
                                dp = pc.dp;
                                iffy = pc.iffy;
                            }
                            None => {
                                hr = 0.0;
                                hi = 0.0;
                                p = 0.0;
                                dp = 0.0;
                            }
                        }
                        match pghcou(rhof, ch.l, ch.boundary, pair.shift_flag, eta, true) {
                            Some(pcx) => {
                                sinphi = pcx.sinphi;
                                cosphi = pcx.cosphi;
                            }
                            None => {
                                sinphi = 0.0;
                                cosphi = 1.0;
                            }
                        }
                    }
                    sinsqr[i - 1] = sinphi.powi(2);
                    sin2ph[i - 1] = 2.0 * sinphi * cosphi;
                }

                rootp[i - 1] = p.sqrt();

                // samm.f90:3415-3453
                let skip_elinv = !iffy
                    && !(pair.shift_flag <= 0 && (1.0 - p * g(&ymat.im, ii) == 1.0 || p < TINY));
                if skip_elinv {
                    elinvr[i - 1] = hr;
                    elinvi[i - 1] = hi;
                    add_at(&mut ymat.re, ii, hr);
                    add_at(&mut ymat.im, ii, hi);
                } else {
                    // Penetrability is very small but non-zero.
                    psmall[i - 1] = rootp[i - 1];
                    mul_at(&mut ymat.re, ii, p);
                    mul_sub1_at(&mut ymat.im, ii, p);
                    mul_at(&mut rmat.re, ii, p);
                    mul_at(&mut rmat.im, ii, p);
                    if nchan > 1 {
                        if i > 1 {
                            for j in 1..i {
                                let ji = packed(j as i64, i as i64);
                                mul_at(&mut ymat.re, ji, rootp[i - 1]);
                                mul_at(&mut ymat.im, ji, rootp[i - 1]);
                                mul_at(&mut rmat.re, ji, rootp[i - 1]);
                                mul_at(&mut rmat.im, ji, rootp[i - 1]);
                            }
                        }
                        if i < nchan {
                            for j in (i + 1)..=nchan {
                                let ji = packed(i as i64, j as i64);
                                mul_at(&mut ymat.re, ji, rootp[i - 1]);
                                mul_at(&mut ymat.im, ji, rootp[i - 1]);
                                mul_at(&mut rmat.re, ji, rootp[i - 1]);
                                mul_at(&mut rmat.im, ji, rootp[i - 1]);
                            }
                        }
                    }
                    rootp[i - 1] = 1.0;
                    elinvr[i - 1] = 0.0;
                    elinvi[i - 1] = -1.0;
                    let _ = dp; // dP/d(rho): derivative-only, not used here
                }
            }
        } else {
            rootp[i - 1] = 0.0;
            elinvr[i - 1] = 1.0;
            elinvi[i - 1] = 0.0;
        }
    }

    // samm.f90:3464-3473 -- an irrelevant channel (Y diagonal exactly zero)
    // is set to unity so the matrix stays invertible.
    if !lrmat_trivial {
        for k in 1..=nchan {
            let kk = packed(k as i64, k as i64);
            if g(&ymat.re, kk) == 0.0 && g(&ymat.im, kk) == 0.0 {
                s(&mut ymat.re, kk, 1.0);
            }
        }
    }

    SetrOutput {
        rmat,
        ymat,
        lrmat_trivial,
        rootp,
        elinvr,
        elinvi,
        sinsqr,
        sin2ph,
        nchan,
    }
}
