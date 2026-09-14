// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The Sköld approximation for intermolecular coherence (`subroutine skold`,
//! `leapr.f90:2816-2862`; card 5 `nsk = 2`).
//!
//! For each `(beta_i, alpha_j)` the coherent part is the incoherent law
//! evaluated at the shifted alpha `alpha_j / S(kappa)` and scaled by
//! `S(kappa)`, where `kappa` \[1/Å\] is the momentum transfer of `alpha_j`:
//!
//! ```text
//! kappa   = 1e-8 * sqrt(2 * awr*amassn*amu * tev*ev * alpha') / hbar,
//!           alpha' = alpha_j * sc / arat
//! S_coh_j = S(kappa) * S_inc(beta_i, alpha_j / S(kappa))     (log-log in alpha)
//! S       = (1 - cfrac) * S_inc + cfrac * S_coh
//! ```
//!
//! `S(kappa)` comes from the deck's card-18 table via [`terpk`] (which is 1
//! beyond the table). The interpolation is `terp1` law 5 between the two
//! bracketing alphas, and a bracket touching a zero law gives zero
//! (`leapr.f90:2845-2851`). Upstream applies it after `contin`/`trans`/
//! `discre` in every temperature pass, only when `ncold = 0`
//! (`leapr.f90:389-390`); the effective temperature is untouched.

use crate::leapr::coldh::terpk;
use crate::leapr::deck::PairCorrelation;
use crate::leapr::input::LeaprInput;
use crate::leapr::SabMatrix;

/// Thermal reference energy `therm` \[eV\] (`leapr.f90:2832`).
const THERM_EV: f64 = 0.0253;
/// `angst` (`leapr.f90:2831`): 1 Å in cm.
const ANGST_CM: f64 = 1.0e-8;

/// `terp1(x1,y1,x2,y2,x,y,5)` (`endf.f90:1589-1647`): log-log interpolation
/// with upstream's degenerate cases.
fn terp1_loglog(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x2 == x1 || y2 == y1 || x == x1 || y1 == 0.0 {
        return y1;
    }
    y1 * ((x / x1).ln() * (y2 / y1).ln() / (x2 / x1).ln()).exp()
}

/// Apply the Sköld correction to `ssm` in place (`leapr.f90:2830-2863`).
///
/// * `input` — the pass's job: alpha grid, `lat`, `arat`, `tev`, constants.
/// * `pc` — the temperature block's cards 17-19 (`dka`, `S(kappa)`, `cfrac`).
/// * `awr` — the principal scatterer's mass ratio (upstream uses `awr` in
///   both passes; only `arat` distinguishes the secondary).
pub fn apply_skold(ssm: &mut SabMatrix, input: &LeaprInput, pc: &PairCorrelation, awr: f64) {
    let nalpha = ssm.nalpha;
    let nbeta = ssm.nbeta;
    let alpha = &input.alpha;
    let tev = input.tev();
    let sc = if input.lat { THERM_EV / tev } else { 1.0 };
    let c = input.constants;
    let amass = awr * c.amassn_amu() * c.amu_g();

    // Per alpha: kappa, S(kappa) and the shifted alpha with its bracket
    // (`leapr.f90:2836-2844`) — independent of beta.
    let shifts: Vec<(f64, f64, usize)> = (0..nalpha)
        .map(|j| {
            let al = alpha[j] * sc / input.arat;
            let waven = ANGST_CM * (2.0 * amass * tev * c.ev_erg() * al).sqrt() / c.hbar_erg_s();
            let sk = terpk(&pc.skappa, pc.dka, waven);
            let ap = alpha[j] / sk;
            let mut kk = nalpha; // 1-based `kk`
            for (k, &a) in alpha.iter().enumerate() {
                kk = k + 1;
                if ap < a {
                    break;
                }
            }
            if kk == 1 {
                kk = 2;
            }
            (sk, ap, kk)
        })
        .collect();

    let mut scoh = vec![0.0f64; nalpha];
    for i in 0..nbeta {
        for (j, &(sk, ap, kk)) in shifts.iter().enumerate() {
            let (lo, hi) = (kk - 2, kk - 1); // 0-based kk-1, kk
            let (y1, y2) = (ssm.get(i, lo), ssm.get(i, hi));
            scoh[j] = if y1 == 0.0 || y2 == 0.0 {
                0.0
            } else {
                terp1_loglog(alpha[lo], y1, alpha[hi], y2, ap)
            } * sk;
        }
        for (j, &s) in scoh.iter().enumerate() {
            let v = (1.0 - pc.cfrac) * ssm.get(i, j) + pc.cfrac * s;
            ssm.set(i, j, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: `terp1` law 5 (`endf.f90:1627-1633`) — log-log between
    /// (1, 1) and (4, 16) at x = 2 gives 4; the degenerate cases return
    /// `y1`. Result (2026-09-10): exact.
    #[test]
    fn loglog_interpolation_matches_terp1_law5() {
        assert!((terp1_loglog(1.0, 1.0, 4.0, 16.0, 2.0) - 4.0).abs() < 1e-12);
        assert_eq!(terp1_loglog(1.0, 0.0, 4.0, 16.0, 2.0), 0.0);
        assert_eq!(terp1_loglog(1.0, 3.0, 1.0, 16.0, 2.0), 3.0);
        assert_eq!(terp1_loglog(1.0, 3.0, 4.0, 3.0, 2.0), 3.0);
    }

    /// Methodology: with `S(kappa) = 1` everywhere the shifted alpha equals
    /// alpha, so `S_coh = S_inc` and the correction is the identity for any
    /// `cfrac` (`leapr.f90:2862`). Result (2026-09-10): unchanged to 1e-14.
    #[test]
    fn unit_structure_factor_is_the_identity() {
        let alpha = vec![0.1, 0.2, 0.4, 0.8, 1.6];
        let beta = vec![0.0, 0.5, 1.0];
        let mut ssm = SabMatrix::zeros(beta.len(), alpha.len());
        for i in 0..beta.len() {
            for j in 0..alpha.len() {
                ssm.set(i, j, (1.0 + i as f64) / (1.0 + alpha[j]));
            }
        }
        let before = ssm.clone();
        let input = LeaprInput {
            alpha: alpha.clone(),
            beta,
            lat: true,
            arat: 1.0,
            nphon: 100,
            temperature_k: 293.6,
            continuous: crate::leapr::input::ContinuousDist {
                delta_ev: 0.001,
                rho: vec![0.0, 1.0, 0.0],
                twt: 0.0,
                c: 0.0,
                tbeta: 1.0,
            },
            oscillators: vec![],
            constants: Default::default(),
        };
        let pc = PairCorrelation {
            dka: 0.5,
            skappa: vec![1.0; 40],
            cfrac: 0.7,
        };
        apply_skold(&mut ssm, &input, &pc, 2.0);
        for i in 0..3 {
            for j in 0..5 {
                assert!(
                    (ssm.get(i, j) - before.get(i, j)).abs() < 1e-14,
                    "({i},{j})"
                );
            }
        }
    }
}
