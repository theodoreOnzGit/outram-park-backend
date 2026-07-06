//! PURR's own complex probability integral evaluator (`uw2`).
//!
//! Ported from `uw2` (`purr.f90:2606-2781`) — algorithmically identical to
//! [`crate::unresr::wfun::uw`] (same break-point regions, same asymptotic
//! continued-fraction / Taylor series, same `WRecurrence` step), reusing that
//! module's [`crate::unresr::wfun::WRecurrence`] rather than re-deriving the
//! delicate continued-fraction recurrence a second time. The one genuine
//! difference is documented on [`uw2`] itself.
//!
//! `unrest`'s own two-table (coarse/fine, `uwtab2` + inline biquadratic
//! lookup) fast-evaluation scheme for `w(z)` is **not ported** — it exists
//! purely to avoid calling the exact evaluator at every Monte Carlo sample
//! point, and `unrest` itself is deferred (see the crate/module docs). Any
//! future `unrest` port needing that lookup table can build it directly on
//! [`uw2`], the same way [`crate::unresr::wfun::WTable::new`] builds on `uw`.

use crate::unresr::wfun::WRecurrence;

const EPS: f64 = 1.0e-7;

/// `w(z) = e^{-z²}·erfc(-iz)`, the complex probability integral (Faddeeva
/// function), ported from `uw2` (`purr.f90:2606-2781`).
///
/// **The one difference from [`crate::unresr::wfun::uw`]:** once the real
/// part `Re(w)` has converged, if `Re(z) == 0` exactly, the imaginary part is
/// forced to `0.0` and returned immediately rather than also iterating its
/// own convergence check (`purr.f90:2693`, `2736`) — an exactness shortcut
/// for evaluating on the purely-imaginary axis (where `Im(w)` is analytically
/// zero) that `uw` does not have. Ported as the same shortcut, not unified
/// away, since it is a genuine (if small) behavioural difference between the
/// two upstream routines.
pub fn uw2(rez: f64, aim1: f64) -> (f64, f64) {
    let aimz = aim1.abs();
    let abrez = rez.abs();
    if abrez + aimz == 0.0 {
        return (1.0, 0.0);
    }

    let r2 = rez * rez;
    let ai2 = aimz * aimz;

    const BRK1: f64 = 1.25;
    const BRK2: f64 = 5.0;
    const BRK3: f64 = 1.863_636;
    const BRK4: f64 = 4.1;
    const BRK5: f64 = 1.71;
    const BRK6: f64 = 2.89;
    const BRK7: f64 = 1.18;
    const BRK8: f64 = 5.76;
    const BRK9: f64 = 1.5;

    // purr.f90:2648-2652 — region selection (kw=1 asymptotic, kw=2 taylor),
    // identical to uw's.
    let use_taylor = if abrez + BRK1 * aimz - BRK2 > 0.0 {
        false
    } else if abrez + BRK3 * aimz - BRK4 > 0.0 {
        false
    } else if r2 + BRK5 * ai2 - BRK6 < 0.0 {
        true
    } else if r2 + BRK7 * ai2 - BRK8 >= 0.0 {
        true
    } else {
        aimz - BRK9 >= 0.0
    };

    if use_taylor {
        w_taylor2(rez, aim1, r2, ai2)
    } else if aim1 >= 0.0 {
        w_asymptotic2(rez, aimz, r2, ai2)
    } else {
        w_taylor2(rez, aim1, r2, ai2)
    }
}

/// Asymptotic-series branch with the `rez==0` shortcut — `purr.f90:2664-2695`
/// (labels 370-390).
fn w_asymptotic2(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    let (mut state, ak, rv) = WRecurrence::init_asymptotic(rez, aimz, r2, ai2);

    let mut aak: f64 = 1.0;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let ajtemp = 2.0 * aak;
        let temp4 = (1.0 - ajtemp) * ajtemp;
        let ajp = rv - (4.0 * aak + 1.0);
        state.step(ajp, temp4, ak);
        aak += 1.0;

        let (pr, pim) = (rew, aimw);
        let (new_rew, new_aimw) = state.ratio();
        rew = new_rew;
        aimw = new_aimw;
        if (rew - pr).abs() < EPS {
            // purr.f90:2693 — exactness shortcut, absent in `uw`.
            if rez == 0.0 {
                return (rew, 0.0);
            }
            if (aimw - pim).abs() < EPS {
                return (rew, aimw);
            }
        }
    }
}

/// Taylor-series branch with the `rez==0` shortcut — `purr.f90:2697-2741`
/// (labels 420-440).
fn w_taylor2(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    const C2: f64 = 1.5;
    let rpi = std::f64::consts::PI.sqrt();

    let temp1 = r2 + ai2;
    let temp2 = 2.0 * temp1 * temp1;
    let aj0 = -(r2 - ai2) / temp2;
    let ak = 2.0 * rez * aimz / temp2;

    let mut state = WRecurrence::init_taylor();

    let expon = (temp2 * aj0).exp();
    let expc = expon * (temp2 * ak).cos();
    let exps = -expon * (temp2 * ak).sin();

    let mut ajsig: f64 = 0.0;
    let mut sig2p: f64 = 2.0 * C2;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let aj4sig = 4.0 * ajsig;
        let aj4sm1 = aj4sig - 1.0;
        let temp3 = 1.0 / (aj4sm1 * (aj4sig + 3.0));
        let tt4 = sig2p * (2.0 * ajsig - 1.0);
        let temp4 = tt4 / (aj4sm1 * (aj4sig + 1.0) * (aj4sig - 3.0) * aj4sm1);
        let ajp = aj0 + temp3;
        state.step(ajp, temp4, ak);

        ajsig += 1.0;
        let temp7 = rpi * (state.am_el_mag_squared());
        let (c, d, am, el) = state.raw_cd_am_el();
        let ref_ = (aimz * (c * am + d * el) - rez * (am * d - c * el)) / temp7 / temp1;
        let aimf = (aimz * (am * d - c * el) + rez * (c * am + d * el)) / temp7 / temp1;

        let (pr, pim) = (rew, aimw);
        rew = expc - ref_;
        aimw = exps - aimf;
        if (rew - pr).abs() < EPS {
            // purr.f90:2736 — exactness shortcut, absent in `uw`.
            if rez == 0.0 {
                return (rew, 0.0);
            }
            if (aimw - pim).abs() < EPS {
                return (rew, aimw);
            }
        }
        sig2p = 2.0 * ajsig;
    }
}
