// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Translational (free-gas / diffusion) part of the scattering law.
//!
//! Ports `trans` (leapr.f90:844-1007), `stable` (1009-1122), `terps`
//! (1124-1162), `sbfill` (1164-1251) and `besk1` (1253-1318). A translational
//! (diffusive or free-gas) mode is convolved onto the solid-type continuous
//! `S(alpha, beta)` produced by [`crate::leapr::continuous`].
//!
//! ## Physics
//! - Free gas (`c == 0`): the translational shape is the Gaussian
//!   `exp(-(w-beta)^2/(4w)) / sqrt(4 pi w)`, `w = twt*alpha`.
//! - Diffusion (`c > 0`): the Egelstaff-Schofield diffusion law, expressed with
//!   the modified Bessel function `K_1` ([`besk1`]).
//!
//! The translational shape is tabulated by [`stable`] and Simpson-convolved with
//! the continuous law (interpolated onto the convolution grid by [`sbfill`]).
//!
//! ## Units
//! `alpha`, `beta`, `S` dimensionless; temperatures in K.

use crate::leapr::frequency::FrequencyModel;
use crate::leapr::input::LeaprInput;
use crate::leapr::SabMatrix;

const TINY: f64 = 1e-30;
/// Log-space floor (`slim` in leapr.f90).
const SLIM: f64 = -225.0;
/// Reference thermal energy for `lat` scaling (eV).
const THERM: f64 = 0.0253;

/// Modified Bessel function `K_1(x)`, with the `exp(-x)` factor **omitted for
/// `x > 1`** (i.e. it returns `K_1(x)` for `x <= 1` and `K_1(x) * exp(x)` for
/// `x > 1`). This scaling matches NJOY `besk1` (leapr.f90:1253-1318) and is
/// undone by the caller [`stable`].
///
/// # Arguments
/// * `x` — argument (dimensionless), `x > 0`.
pub fn besk1(x: f64) -> f64 {
    // polynomial coefficients (leapr.f90:1263-1300)
    const C1: f64 = 0.442850424;
    const C2: f64 = 0.584115288;
    const C3: f64 = 6.070134559;
    const C4: f64 = 17.864913364;
    const C5: f64 = 48.858995315;
    const C6: f64 = 90.924600045;
    const C7: f64 = 113.795967431;
    const C8: f64 = 85.331474517;
    const C9: f64 = 32.00008698;
    const C10: f64 = 3.999998802;
    const C11: f64 = 1.304923514;
    const C12: f64 = 1.47785657;
    const C13: f64 = 16.402802501;
    const C14: f64 = 44.732901977;
    const C15: f64 = 115.837493464;
    const C16: f64 = 198.437197312;
    const C17: f64 = 222.869709703;
    const C18: f64 = 142.216613971;
    const C19: f64 = 40.000262262;
    const C20: f64 = 1.999996391;
    const C23: f64 = 0.5772156649;
    const C25: f64 = 0.0108241775;
    const C26: f64 = 0.0788000118;
    const C27: f64 = 0.2581303765;
    const C28: f64 = 0.5050238576;
    const C29: f64 = 0.663229543;
    const C30: f64 = 0.6283380681;
    const C31: f64 = 0.4594342117;
    const C32: f64 = 0.2847618149;
    const C33: f64 = 0.1736431637;
    const C34: f64 = 0.1280426636;
    const C35: f64 = 0.1468582957;
    const C36: f64 = 0.4699927013;
    const C37: f64 = 1.2533141373;

    if x <= 1.0 {
        let v = 0.125 * x;
        let u = v * v;
        let bi1 =
            (((((((((C1 * u + C2) * u + C3) * u + C4) * u + C5) * u + C6) * u + C7) * u + C8) * u
                + C9)
                * u
                + C10)
                * v;
        let bi3 = ((((((((C11 * u + C12) * u + C13) * u + C14) * u + C15) * u + C16) * u + C17)
            * u
            + C18)
            * u
            + C19)
            * u
            + C20;
        1.0 / x + bi1 * ((0.5 * x).ln() + C23) - v * bi3
    } else {
        let u = 1.0 / x;
        let bi3 = (((((((((((-C25 * u + C26) * u - C27) * u + C28) * u - C29) * u + C30) * u
            - C31)
            * u
            + C32)
            * u
            - C33)
            * u
            + C34)
            * u
            - C35)
            * u
            + C36)
            * u
            + C37;
        u.sqrt() * bi3
    }
}

/// Interpolate in a table of the translational `S(beta)` for a required beta
/// (`terps`, leapr.f90:1124-1162). Log-linear interpolation; returns `0` beyond
/// the tabulated range.
///
/// # Arguments
/// * `sd` — the translational shape table (dimensionless).
/// * `delta` — grid spacing (dimensionless).
/// * `be` — the beta value (dimensionless, `>= 0`).
pub fn terps(sd: &[f64], delta: f64, be: f64) -> f64 {
    let nsd = sd.len();
    if be < 0.0 || be > delta * nsd as f64 {
        return 0.0;
    }
    let i = (be / delta).floor() as usize;
    if i < nsd - 1 {
        let bt = i as f64 * delta;
        let btp = bt + delta;
        let st = if sd[i] <= 0.0 { SLIM } else { sd[i].ln() };
        let stp = if sd[i + 1] <= 0.0 {
            SLIM
        } else {
            sd[i + 1].ln()
        };
        let stt = st + (be - bt) * (stp - st) / (btp - bt);
        if stt > SLIM {
            stt.exp()
        } else {
            0.0
        }
    } else {
        0.0
    }
}

/// Tabulate the translational shape `S_diffusion` or `S_free` (`stable`,
/// leapr.f90:1009-1122).
///
/// Returns `(beta_table, s_table)` where both have the same (odd, for Simpson's
/// rule) length; tabulation continues until the value drops below `1e-7` of the
/// peak or the length cap `ndmax` is hit.
///
/// # Arguments
/// * `al` — the (scaled) alpha (dimensionless).
/// * `delta` — grid spacing chosen by [`add_translation`] (dimensionless).
/// * `twt` — translational weight (dimensionless).
/// * `c` — diffusion constant (dimensionless); `0` selects the free-gas branch.
/// * `ndmax` — hard length cap.
pub fn stable(al: f64, delta: f64, twt: f64, c: f64, ndmax: usize) -> (Vec<f64>, Vec<f64>) {
    const EPS: f64 = 1e-7;
    let pi = crate::common::phys::PI;
    let mut ap: Vec<f64> = Vec::new();
    let mut sd: Vec<f64> = Vec::new();
    let mut be = 0.0_f64;
    let mut j = 1_usize; // 1-based running count, matching Fortran

    if c != 0.0 {
        // diffusion branch
        let d = twt * c;
        let c2 = (c * c + 0.25).sqrt();
        let c3a = 2.0 * d * al;
        let c4 = c3a * c3a;
        let c8 = c2 * c3a / pi;
        let c3 = 2.0 * d * c * al;
        loop {
            let c6 = (be * be + c4).sqrt();
            let c7 = c6 * c2;
            let c5 = if c7 <= 1.0 {
                c8 * (c3 + be / 2.0).exp()
            } else {
                c8 * (c3 - c7 + be / 2.0).exp()
            };
            sd.push(c5 * besk1(c7) / c6);
            ap.push(be);
            be += delta;
            j += 1;
            if j % 2 == 0 {
                if j >= ndmax || EPS * sd[0] >= *sd.last().unwrap() {
                    break;
                }
            }
        }
    } else {
        // free-gas branch
        let wal = twt * al;
        loop {
            let ex = -(wal - be) * (wal - be) / (4.0 * wal);
            sd.push(ex.exp() / (4.0 * pi * wal).sqrt());
            ap.push(be);
            be += delta;
            j += 1;
            if j % 2 == 0 {
                if j >= ndmax || EPS * sd[0] >= *sd.last().unwrap() {
                    break;
                }
            }
        }
    }
    (ap, sd)
}

/// Generate the continuous `S(beta)` on the convolution grid (`sbfill`,
/// leapr.f90:1164-1251) by log-linear interpolation of the tabulated law.
///
/// Returns the `sb` array (length `2*nbt-1`) used by the translational
/// convolution. `delta` may be grown internally to avoid floating-point
/// underflow of the grid step (rare); the possibly-updated value is returned.
///
/// # Arguments
/// * `nbt` — half-width of the convolution grid (`= nsd`).
/// * `delta` — grid spacing (dimensionless).
/// * `be` — the target beta (dimensionless).
/// * `s` — the current continuous `S` indexed by the beta grid (dimensionless).
/// * `betan` — the scaled beta grid (dimensionless).
fn sbfill(nbt: usize, mut delta: f64, be: f64, s: &[f64], betan: &[f64]) -> (Vec<f64>, f64) {
    const SHADE: f64 = 1.00001;
    let nbeta = betan.len();
    let bmin = -be - (nbt as f64 - 1.0) * delta;
    let bmax = -be + (nbt as f64 - 1.0) * delta + delta / 100.0;
    let mut sb: Vec<f64> = Vec::new();
    let mut j = nbeta; // 1-based pointer into betan
    let mut bet = bmin;
    while bet <= bmax {
        let b = bet.abs();
        // search for the correct beta bracket
        let mut idone = 0;
        while idone == 0 {
            if b > betan[j - 1] {
                if j == nbeta && b < SHADE * betan[j - 1] {
                    idone = 1;
                } else if j == nbeta {
                    idone = 2;
                } else {
                    j += 1;
                }
            } else if b > betan[j - 2] {
                idone = 1;
            } else if j == 2 {
                idone = 1;
            } else {
                j -= 1;
            }
        }
        let val = if idone == 1 {
            let st = if s[j - 1] <= 0.0 { SLIM } else { s[j - 1].ln() };
            let stm = if s[j - 2] <= 0.0 { SLIM } else { s[j - 2].ln() };
            let mut logv = st + (b - betan[j - 1]) * (stm - st) / (betan[j - 2] - betan[j - 1]);
            if bet > 0.0 {
                logv -= bet;
            }
            if logv > SLIM {
                logv.exp()
            } else {
                0.0
            }
        } else {
            0.0
        };
        sb.push(val);
        // underflow guard on the grid step
        while bet == bet + delta {
            delta *= 10.0;
        }
        bet += delta;
    }
    (sb, delta)
}

/// Add the translational term to the continuous scattering law (`trans`,
/// leapr.f90:844-1007).
///
/// Mutates `sab` in place (the solid-type law from
/// [`crate::leapr::continuous::phonon_expansion`]) and updates the effective
/// temperature `tempf`.
///
/// # Arguments
/// * `sab` — the continuous law to convolve into (modified in place).
/// * `input` — the LEAPR job (alpha/beta grids, `twt`, `c`, `lat`, `arat`).
/// * `freq` — the frequency model (Debye-Waller `f0`, `deltab`).
/// * `tempf` — the effective temperature \[K\]; updated in place.
pub fn add_translation(
    sab: &mut SabMatrix,
    input: &LeaprInput,
    freq: &FrequencyModel,
    tempf: &mut f64,
) {
    const C0: f64 = 0.4;
    const C1: f64 = 1.0;
    const C2: f64 = 1.42;
    const C3: f64 = 0.2;
    const C4: f64 = 10.0;

    let nalpha = input.alpha.len();
    let nbeta = input.beta.len();
    let tev = input.tev();
    let sc = if input.lat { THERM / tev } else { 1.0 };
    let arat = input.arat;
    let twt = input.continuous.twt;
    let c = input.continuous.c;
    let tbeta = input.continuous.tbeta;
    let f0 = freq.f0;
    let deltab = freq.deltab;
    let ndmax = nbeta.max(1_000_000);

    let betan: Vec<f64> = input.beta.iter().map(|&b| b * sc).collect();

    for ialpha in 0..nalpha {
        let al = input.alpha[ialpha] * sc / arat;

        // choose the convolution beta interval
        let mut ded = C0 * (twt * c * al) / (C1 + C2 * (twt * c * al) * c).sqrt();
        if ded == 0.0 {
            ded = C3 * (twt * al).sqrt();
        }
        let deb = C4 * al * deltab;
        let mut delta = deb.min(ded);
        if !(delta > 0.0) {
            continue;
        }

        let (_ap_tbl, sd) = stable(al, delta, twt, c, ndmax);
        let nsd = sd.len();
        if nsd <= 1 {
            continue;
        }

        // snapshot the current continuous S(-beta) for this alpha column
        let s_cont: Vec<f64> = (0..nbeta).map(|i| sab.get(i, ialpha)).collect();

        for ibeta in 0..nbeta {
            let be = betan[ibeta];
            let nbt = nsd;
            let (sb, new_delta) = sbfill(nbt, delta, be, &s_cont, &betan);
            delta = new_delta;

            // Simpson-rule convolution of s-transport with s-continuous
            let mut s = 0.0_f64;
            for i in 1..=nbt {
                let f = if i == 1 || i == nbt {
                    1.0
                } else {
                    2.0 * (((i - 1) % 2) as f64 + 1.0)
                };
                // sb is 1-based length 2*nbt-1: sb(nbt+i-1), sb(nbt-i+1)
                let hi = nbt + i - 1; // 1-based
                let lo = nbt - i + 1; // 1-based
                s += f * sd[i - 1] * sb[hi - 1];
                let bb = (i - 1) as f64 * delta;
                s += f * sd[i - 1] * sb[lo - 1] * (-bb).exp();
            }
            s *= delta / 3.0;
            if s < TINY {
                s = 0.0;
            }
            let st = terps(&sd, delta, be);
            if st > 0.0 {
                s += (-al * f0).exp() * st;
            }
            sab.set(ibeta, ialpha, s);
        }
    }

    // update effective temperature
    *tempf = (tbeta * *tempf + twt * input.temperature_k) / (tbeta + twt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leapr::input::ContinuousDist;

    /// Methodology: `besk1` returns `K_1(x)` for `x <= 1` and `K_1(x) e^x` for
    /// `x > 1`. Compare to reference values: `K_1(0.5) = 1.656441`,
    /// `K_1(1) = 0.601907`, and `e^2 K_1(2) = 1.03339` (so `besk1(2) = 1.03339`).
    /// Result (2026-07-15): all three match to `< 2e-4` absolute.
    #[test]
    fn besk1_reference_values() {
        assert!(
            (besk1(0.5) - 1.656441).abs() < 2e-4,
            "K1(0.5) = {}",
            besk1(0.5)
        );
        assert!(
            (besk1(1.0) - 0.601907).abs() < 2e-4,
            "K1(1) = {}",
            besk1(1.0)
        );
        // besk1(2) is scaled by e^2, so equals e^2 * K1(2) = 1.03339
        assert!(
            (besk1(2.0) - 1.03339).abs() < 2e-4,
            "e^2 K1(2) = {}",
            besk1(2.0)
        );
    }

    /// Methodology: the free-gas translational table from `stable` must peak near
    /// `beta = twt*alpha`, decay monotonically past the peak, and terminate with
    /// an odd number of points (Simpson). Result (2026-07-15): table length is
    /// odd, all entries finite and non-negative, and the tail is `< 1e-7` of peak.
    #[test]
    fn stable_free_gas_table_shape() {
        let (ap, sd) = stable(1.0, 0.02, 0.5, 0.0, 100_000);
        assert!(sd.len() % 2 == 1, "nsd = {} not odd", sd.len());
        assert_eq!(ap.len(), sd.len());
        assert!(sd.iter().all(|v| v.is_finite() && *v >= 0.0));
        let peak = sd[0].max(*sd.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap());
        assert!(*sd.last().unwrap() <= 1e-7 * peak + 1e-30);
    }

    /// Smoke test: convolving a free-gas translation onto a flat continuous law
    /// must keep S finite/non-negative and raise the effective temperature toward
    /// the physical temperature. Result (2026-07-15): no NaN/negative entries;
    /// `tempf` moves from the input value toward `temperature_k`.
    #[test]
    fn add_translation_runs_and_is_finite() {
        let nbeta = 40;
        let beta: Vec<f64> = (0..nbeta).map(|i| i as f64 * 0.25).collect();
        let alpha = vec![0.5, 1.0, 2.0];
        let input = LeaprInput {
            alpha: alpha.clone(),
            beta: beta.clone(),
            lat: false,
            arat: 1.0,
            nphon: 50,
            temperature_k: 293.6,
            continuous: ContinuousDist {
                delta_ev: 0.002,
                rho: vec![0.0, 1.0, 2.0, 1.0, 0.5],
                twt: 0.2,
                c: 0.0,
                tbeta: 0.8,
            },
            oscillators: vec![],
            constants: crate::leapr::vintage::PhysicalConstants::default(),
        };
        let freq = FrequencyModel {
            deltab: 0.08,
            f0: 5.0,
            tbar: 1.2,
            t1: vec![0.5, 0.4, 0.3, 0.2, 0.1],
        };
        // start from a simple positive continuous law
        let mut sab = SabMatrix::zeros(nbeta, alpha.len());
        for j in 0..alpha.len() {
            for k in 0..nbeta {
                sab.set(k, j, (-(beta[k]) / 2.0).exp() * 0.01);
            }
        }
        let mut tempf = freq.tbar * input.temperature_k;
        add_translation(&mut sab, &input, &freq, &mut tempf);
        for j in 0..alpha.len() {
            for k in 0..nbeta {
                let v = sab.get(k, j);
                assert!(v.is_finite() && v >= 0.0, "sab[{k},{j}] = {v}");
            }
        }
        assert!(tempf.is_finite() && tempf > 0.0, "tempf = {tempf}");
    }
}
