//! The two Coulomb entry points other modules actually call — `pspcou`
//! (raw phase-shift/penetrability/shift-factor evaluation) and `pghcou`
//! (the Coulomb counterpart of [`super::super::penetrability::pgh`],
//! wrapping `pspcou`'s output into the `G+iH` form
//! [`super::super::betset`] needs) — ported from `samm.f90:3823-4001`.

use super::dispatch::{coulx, CoulombPsp};
use super::steed::coulfg;

/// Compute Coulomb `P` (penetrability), `S` (shift factor), and `phi`
/// (hard-sphere-analogue phase shift), for channel angular momentum `lll`
/// — ported from `pspcou` (`samm.f90:3906-4001`). Dispatches to [`coulx`]
/// for `rho<1.02` (small-`rho` branch, matching upstream, using its own
/// separate approximation strategy) or [`coulfg`] otherwise, retrying at
/// `coulx` if `coulfg` fails (`ifail!=0`) exactly as upstream does. If a
/// shift-factor derivative is requested, both branches additionally
/// evaluate at `rho*1.01` to finite-difference `dshiftcoul`.
///
/// `ishift`: `>0` to compute the shift factor. Returns `None` only for the
/// upstream `lll<0` guard (`ifail=10` there); all other upstream failure
/// paths (`coulfg`/`bigeta` convergence failures) are logged and fall
/// through to a best-effort `coulx` retry, matching upstream's own
/// fallback behavior.
pub fn pspcou(rho: f64, lll: i32, eta: f64, ishift: i32, jdopha: bool, jdoder: bool) -> Option<CoulombPsp> {
    if lll < 0 {
        return None;
    }

    let llmax = lll + 2;

    let mut result = if rho < 1.02 {
        coulx(eta, rho, lll, jdopha, jdoder, ishift).unwrap_or(CoulombPsp { p: 0.0, s: 0.0, dp: 0.0, sinphi: 0.0, cosphi: 1.0, dphi: 0.0, dshift: 0.0 })
    } else {
        let cf = coulfg(rho, eta, lll, llmax, jdopha, jdoder, ishift);
        if cf.ifail != 0 {
            log::warn!("samm pspcou: coulfg failed (ifail={}), falling back to coulx (rho={rho}, eta={eta}, lll={lll})", cf.ifail);
            coulx(eta, rho, lll, jdopha, jdoder, ishift).unwrap_or(CoulombPsp { p: 0.0, s: 0.0, dp: 0.0, sinphi: 0.0, cosphi: 1.0, dphi: 0.0, dshift: 0.0 })
        } else {
            CoulombPsp { p: cf.pcoul, s: cf.scoul, dp: cf.dpcoul, sinphi: cf.sinphi, cosphi: cf.cosphi, dphi: cf.dphi, dshift: 0.0 }
        }
    };

    if ishift > 0 && jdoder {
        let rho2 = 1.01 * rho;
        let eta2 = eta * rho / rho2;
        let s2 = if rho2 < 1.02 {
            coulx(eta2, rho2, lll, jdopha, jdoder, ishift).map(|r| r.s)
        } else {
            let cf2 = coulfg(rho2, eta2, lll, llmax, jdopha, jdoder, ishift);
            if cf2.ifail == 0 {
                Some(cf2.scoul)
            } else {
                coulx(eta2, rho2, lll, jdopha, jdoder, ishift).map(|r| r.s)
            }
        };
        if let Some(s2) = s2 {
            result.dshift = (s2 - result.s) / (rho2 - rho);
        }
    }

    Some(result)
}

/// `G+iH` (the real/imaginary parts of `1/(S-B+iP)`), the penetrability
/// `P`, and their derivatives, for a **charged**-particle channel — the
/// Coulomb counterpart of [`super::super::penetrability::pgh`] — ported
/// from `pghcou` (`samm.f90:3823-3904`).
///
/// If `S-B+iP == 0` (the `iffy` condition upstream), `g=h=p=dp=ds=0` is
/// returned along with `iffy=true` rather than dividing by zero, matching
/// [`super::super::penetrability::Pgh`].
pub struct Pghcou {
    pub g: f64,
    pub h: f64,
    pub p: f64,
    pub dp: f64,
    pub ds: f64,
    pub iffy: bool,
}

pub fn pghcou(rho: f64, l: i32, bound: f64, ishift: i32, eta: f64, jdopha: bool) -> Option<Pghcou> {
    let jdoder = true;
    let psp = pspcou(rho, l, eta, ishift, jdopha, jdoder)?;

    let hh_raw = psp.p;
    let dp = psp.dp;
    let ds = if ishift > 0 { psp.dshift } else { 0.0 };
    let gg = if ishift > 0 { psp.s - bound } else { 0.0 };

    let hh = if hh_raw <= 1.0e-35 { 0.0 } else { hh_raw };

    if gg == 0.0 && hh == 0.0 {
        return Some(Pghcou { g: 0.0, h: 0.0, p: 0.0, dp: 0.0, ds: 0.0, iffy: true });
    }
    if hh == 0.0 {
        return Some(Pghcou { g: 1.0 / gg, h: 0.0, p: 0.0, dp, ds, iffy: false });
    }
    let p = hh;
    if gg == 0.0 {
        return Some(Pghcou { g: 0.0, h: -1.0 / hh, p, dp, ds, iffy: false });
    }
    if hh + gg == hh {
        return Some(Pghcou { g: (gg / hh) / hh, h: -1.0 / hh, p, dp, ds, iffy: false });
    }
    if hh + gg == gg {
        return Some(Pghcou { g: 1.0 / gg, h: -(hh / gg) / gg, p, dp, ds, iffy: false });
    }
    let d = hh * hh + gg * gg;
    Some(Pghcou { g: gg / d, h: -hh / d, p, dp, ds, iffy: false })
}
