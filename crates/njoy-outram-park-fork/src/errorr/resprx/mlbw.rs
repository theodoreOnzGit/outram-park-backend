// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine ggmlbw`, l.6527-6650 — multilevel Breit-Wigner cross sections at one
//     energy from the `b` work array `rpxlc12` builds (MF=2 records + shift/penetration).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `ggmlbw` — zero-temperature MLBW cross sections for one energy range,
//! evaluated on the resonance work array the ERRORJ sensitivity loop
//! perturbs in place (see [`super::resolved`]).
//!
//! The work array `b` (Fortran 1-based `b(i)` is `b[i-1]` here) holds, in
//! this order: the MF=2 range CONT `(EL, EH, LRU, LRF, NRO, NAPS)`, the
//! CONT `(SPI, AP, 0, 0, NLS, 0)`, then per L-state the LIST head
//! `(AWRI, QX, L, LRX, 6*NRS, NRS)`, the `6*NRS` resonance parameters
//! `(ER, AJ, GT, GN, GG, GF)` and — appended by `rpxlc12` — `3*NRS` words
//! `(S_l(|ER|), P_l(|ER|), 0)` per resonance.
//!
//! Upstream idioms kept on purpose: the channel radius uses the truncated
//! `third = 0.333333333`, `ra = ap` only when `NAPS = 1`, and the neutron
//! width at energy `E` is `Γn(E) = Γn P_l(E)/P_l(|ER|)` with the stored
//! `P_l(|ER|)`, which the caller re-evaluates whenever it perturbs `ER` or
//! the scattering radius.

use crate::common::phys::{AMASSN_AMU, PI};
use crate::reconr::slbw::WAVE_K;

use super::super::math::{efacphi, efacts};

/// `rc1`, `rc2`, `third` — the ENDF channel-radius formula
/// `ra = 0.123 (m_n AWRI)^{1/3} + 0.08` as upstream evaluates it.
pub const RC1: f64 = 0.123;
pub const RC2: f64 = 0.08;
pub const THIRD: f64 = 0.333_333_333;

/// `(σ_t, σ_el, σ_f, σ_γ)` \[b\] at `e` \[eV\] for the work array `b`
/// (`ggmlbw`, `errorr.f90:6527-6650`).
///
/// Also returns `arat = AWRI/(AWRI+1)` from the MF=2 `AWRI`, because
/// upstream stores it in a module global that the caller's later
/// `rho` evaluations read (see `resolved.rs`).
pub fn ggmlbw(e: f64, b: &[f64]) -> ([f64; 4], f64) {
    let mut sigp = [0.0f64; 4];
    // retrieve nuclide information (errorr.f90:6551-6560)
    let naps = b[5].round() as i32;
    let awri = b[12];
    let ap = b[7];
    let aw = AMASSN_AMU * awri;
    let mut ra = RC1 * aw.powf(THIRD) + RC2;
    if naps == 1 {
        ra = ap;
    }
    let spi = b[6];
    let den = 4.0 * spi + 2.0;
    let nls = b[10].round() as usize;

    // wave number, rho and rho-cap at e (errorr.f90:6563-6567)
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let rho = k * ra;
    let rhoc = k * ap;
    let mut inow = 12usize;

    let mut sigj = [[0.0f64; 10]; 2];
    let mut gj = [0.0f64; 10];
    for _l in 0..nls {
        let nrs = b[inow + 5].round() as usize;
        let ll = b[inow + 2].round() as i32;
        let qx = b[inow + 1];
        let lrx = b[inow + 3].round() as i32;
        let (se, pe) = efacts(ll, rho);
        let mut pec = 0.0;
        if lrx != 0 {
            let rhop = WAVE_K * arat * (e + qx).abs().sqrt() * ra;
            let (_sec, p) = efacts(ll, rhop);
            pec = p;
        }
        let phi = efacphi(ll, rhoc);
        let cos2p = 1.0 - (2.0 * phi).cos();
        let sin2p = (2.0 * phi).sin();
        let mut sum = 0.0;
        let fl = ll as f64;
        let ajmin = ((spi - fl).abs() - 0.5).abs();
        let ajmax = spi + fl + 0.5;
        let nj = (ajmax - ajmin + 1.0).round() as usize;
        let mut aj = ajmin;
        for g in gj.iter_mut().take(nj) {
            *g = (2.0 * aj + 1.0) / den;
            aj += 1.0;
            sum += *g;
        }
        let diff = 2.0 * fl + 1.0 - sum;
        for row in sigj.iter_mut() {
            for v in row.iter_mut().take(nj) {
                *v = 0.0;
            }
        }
        inow += 6;
        let mut in_ = inow + nrs * 6;

        // loop over all resonances (errorr.f90:6604-6626)
        for _i in 0..nrs {
            let er = b[inow];
            let j = (b[inow + 1] - ajmin + 1.0).round() as usize - 1;
            let gn = b[inow + 3];
            let gg = b[inow + 4];
            let gf = b[inow + 5];
            let ser = b[in_];
            let per = b[in_ + 1];
            let rper = 1.0 / per;
            let gc = b[in_ + 2];
            in_ += 3;
            inow += 6;
            let erp = er + gn * (ser - se) * rper / 2.0;
            let edelt = e - erp;
            let gne = gn * pe * rper;
            let gx = gg + gf;
            let mut gtt = gne + gx;
            gtt += gc * pec;
            let x = 2.0 * edelt / gtt;
            let mut comfac = 2.0 * gne / gtt / (1.0 + x * x);
            sigj[0][j] += comfac;
            sigj[1][j] += comfac * x;
            comfac = comfac * gj[j] / gtt;
            sigp[2] += comfac * gf;
            sigp[3] += comfac * gg;
        }
        for j in 0..nj {
            let add = gj[j] * ((cos2p - sigj[0][j]).powi(2) + (sin2p + sigj[1][j]).powi(2));
            sigp[1] += add;
        }
        sigp[1] += 2.0 * diff * cos2p;
        inow = in_;
    }

    // construct the final cross sections (errorr.f90:6636-6639)
    sigp[1] *= pifac;
    sigp[2] *= 2.0 * pifac;
    sigp[3] *= 2.0 * pifac;
    sigp[0] = sigp[1] + sigp[2] + sigp[3];
    (sigp, arat)
}
