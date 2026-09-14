// Ported from the GNU Scientific Library `specfunc/gamma_inc.c`,
// `specfunc/gamma.c`, `specfunc/log.c` and `specfunc/cheb_eval.c`, GSL 2.8 at
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
//
// Copyright (C) 2007 Brian Gough
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman   (upstream)
// Copyright (C) 2026 Theodore Ong and the outram-park contributors  (this port)
//
// GSL is GPL-3.0-or-later (verified from its per-file headers -- see NOTICE).
// This derivative work is GPL-3.0-only. The flow is ONE-WAY.

//! The regularised lower incomplete gamma `P(a, x)` and the lower incomplete
//! gamma `γ(a, x)` — a port of GSL's `gsl_sf_gamma_inc_P`.
//!
//! # Why this is here
//!
//! Together with [`expint_e1`](crate::expint_e1) this is what NJOY ACER's MF=5
//! `LF = 12` (Madland-Nix) fission spectrum needs: `acefc.f90:9155`'s `fmn`
//! evaluates the shape in terms of `e1` and SLATEC's `gami`, and
//! `gami(a, x) = Γ(a) P(a, x)` is [`gamma_inc_lower`] below. See GitHub #201.
//!
//! Like `E_1`, the machinery underneath is Chebyshev: `gammastar` and
//! `log_1plusx_mx` are Chebyshev series, and they are what the incomplete
//! gamma is built on for the arguments that matter here.
//!
//! # Ported branches, and the ones deliberately refused
//!
//! `fmn` only ever calls this at `a = 3/2`. Rather than silently assume that,
//! the branches upstream reaches for small `a` are ported and the rest return
//! [`PetirError`] with the upstream line they stand for:
//!
//! | branch | status |
//! |---|---|
//! | `gamma_inc_P_series` (`x < 20` or `x < a/2`) | ported |
//! | `gamma_inc_Q_CF` (`a <= x`, `a > x/5`) | ported |
//! | `gamma_inc_Q_large_x` (`a <= x`, `a <= x/5`) | ported |
//! | `gamma_inc_Q_asymp_unif` (`a > 1e6`) | refused — unreachable below `a = 1e6` |
//! | `gammastar_ser` (`x >= 10`) | refused |
//! | `lngamma` Padé / singular branches (`|x-1|<0.01`, `|x-2|<0.01`, `x < 0.5`) | refused |
//!
//! Refusing is the workspace's established pattern for a branch that cannot be
//! reached by the data in hand: it keeps the port honest about what has
//! actually been transcribed and verified, instead of shipping untested paths
//! that look complete.
//!
//! # The coefficient tables were extracted mechanically
//!
//! All 89 coefficients (`gstar_a`, `gstar_b`, `lopxmx`, `lanczos_7_c`) were
//! parsed out of the C sources by script rather than retyped, and checked
//! against GSL compiled and run in `tests/gsl_gamma_inc.rs`.

use crate::error::{PetirError, Result};

/// `GSL_DBL_EPSILON`.
const EPS: f64 = 2.2204460492503131e-16;
/// `LogRootTwoPi_` (`specfunc/gamma.c:37`).
const LOG_ROOT_TWO_PI: f64 = 0.9189385332046727418;
/// `M_LN2 + M_LNPI`, halved — the `c` of `gammastar`'s `x < 0.5` branch.
const HALF_LN_2PI: f64 = 0.9189385332046727418;

/// GSL `gstar_a_cs`: order 29 on `[-1, 1]`.
const GSTAR_A_DATA: [f64; 30] = [
    2.16786447866463034423060819465,
    -0.05533249018745584258035832802,
    0.01800392431460719960888319748,
    -0.00580919269468937714480019814,
    0.00186523689488400339978881560,
    -0.00059746524113955531852595159,
    0.00019125169907783353925426722,
    -0.00006124996546944685735909697,
    0.00001963889633130842586440945,
    -6.3067741254637180272515795142e-06,
    2.0288698405861392526872789863e-06,
    -6.5384896660838465981983750582e-07,
    2.1108698058908865476480734911e-07,
    -6.8260714912274941677892994580e-08,
    2.2108560875880560555583978510e-08,
    -7.1710331930255456643627187187e-09,
    2.3290892983985406754602564745e-09,
    -7.5740371598505586754890405359e-10,
    2.4658267222594334398525312084e-10,
    -8.0362243171659883803428749516e-11,
    2.6215616826341594653521346229e-11,
    -8.5596155025948750540420068109e-12,
    2.7970831499487963614315315444e-12,
    -9.1471771211886202805502562414e-13,
    2.9934720198063397094916415927e-13,
    -9.8026575909753445931073620469e-14,
    3.2116773667767153777571410671e-14,
    -1.0518035333878147029650507254e-14,
    3.4144405720185253938994854173e-15,
    -1.0115153943081187052322643819e-15,
];
const GSTAR_A_ORDER: usize = 29;

/// GSL `gstar_b_cs`: order 29 on `[-1, 1]`.
const GSTAR_B_DATA: [f64; 30] = [
    0.0057502277273114339831606096782,
    0.0004496689534965685038254147807,
    -0.0001672763153188717308905047405,
    0.0000615137014913154794776670946,
    -0.0000223726551711525016380862195,
    8.0507405356647954540694800545e-06,
    -2.8671077107583395569766746448e-06,
    1.0106727053742747568362254106e-06,
    -3.5265558477595061262310873482e-07,
    1.2179216046419401193247254591e-07,
    -4.1619640180795366971160162267e-08,
    1.4066283500795206892487241294e-08,
    -4.6982570380537099016106141654e-09,
    1.5491248664620612686423108936e-09,
    -5.0340936319394885789686867772e-10,
    1.6084448673736032249959475006e-10,
    -5.0349733196835456497619787559e-11,
    1.5357154939762136997591808461e-11,
    -4.5233809655775649997667176224e-12,
    1.2664429179254447281068538964e-12,
    -3.2648287937449326771785041692e-13,
    7.1528272726086133795579071407e-14,
    -9.4831735252566034505739531258e-15,
    -2.3124001991413207293120906691e-15,
    2.8406613277170391482590129474e-15,
    -1.7245370321618816421281770927e-15,
    8.6507923128671112154695006592e-16,
    -3.9506563665427555895391869919e-16,
    1.6779342132074761078792361165e-16,
    -6.0483153034414765129837716260e-17,
];
const GSTAR_B_ORDER: usize = 29;

/// GSL `lopxmx_cs`: order 19 on `[-1, 1]`.
const LOPXMX_DATA: [f64; 20] = [
    -1.12100231323744103373737274541,
    0.19553462773379386241549597019,
    -0.01467470453808083971825344956,
    0.00166678250474365477643629067,
    -0.00018543356147700369785746902,
    0.00002280154021771635036301071,
    -2.8031253116633521699214134172e-06,
    3.5936568872522162983669541401e-07,
    -4.6241857041062060284381167925e-08,
    6.0822637459403991012451054971e-09,
    -8.0339824424815790302621320732e-10,
    1.0751718277499375044851551587e-10,
    -1.4445310914224613448759230882e-11,
    1.9573912180610336168921438426e-12,
    -2.6614436796793061741564104510e-13,
    3.6402634315269586532158344584e-14,
    -4.9937495922755006545809120531e-15,
    6.8802890218846809524646902703e-16,
    -9.5034129794804273611403251480e-17,
    1.3170135013050997157326965813e-17,
];
const LOPXMX_ORDER: usize = 19;

/// GSL `lanczos_7_c` (`specfunc/gamma.c:644`).
const LANCZOS_7_C: [f64; 9] = [
    0.99999999999980993227684700473478,
    676.520368121885098567009190444019,
    -1259.13921672240287047156078755283,
    771.3234287776530788486528258894,
    -176.61502916214059906584551354,
    12.507343278686904814458936853,
    -0.13857109526572011689554707,
    9.984369578019570859563e-6,
    1.50563273514931155834e-7,
];
/// GSL's `cheb_eval_e` (`specfunc/cheb_eval.c`), value only.
fn cheb(c: &[f64], order: usize, a: f64, b: f64, x: f64) -> (f64, f64) {
    let (mut d, mut dd) = (0.0f64, 0.0f64);
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;
    let mut e = 0.0f64;
    for j in (1..=order).rev() {
        let temp = d;
        d = y2 * d - dd + c[j];
        e += libm::fabs(y2 * temp) + libm::fabs(dd) + libm::fabs(c[j]);
        dd = temp;
    }
    let temp = d;
    d = y * d - dd + 0.5 * c[0];
    e += libm::fabs(y * temp) + libm::fabs(dd) + 0.5 * libm::fabs(c[0]);
    (d, EPS * e + libm::fabs(c[order]))
}

/// `lngamma_lanczos` (`specfunc/gamma.c:703-723`) — the `x >= 0.5` branch of
/// `gsl_sf_lngamma_e`, which is the only branch `a = 3/2` takes.
fn lngamma_lanczos(x: f64) -> (f64, f64) {
    let x = x - 1.0; // Lanczos writes z! rather than Gamma(z)
    let mut ag = LANCZOS_7_C[0];
    for k in 1..=8 {
        ag += LANCZOS_7_C[k] / (x + k as f64);
    }
    let term1 = (x + 0.5) * libm::log((x + 7.5) / core::f64::consts::E);
    let term2 = LOG_ROOT_TWO_PI + libm::log(ag);
    let val = term1 + (term2 - 7.0);
    let err = 2.0 * EPS * (libm::fabs(term1) + libm::fabs(term2) + 7.0) + EPS * libm::fabs(val);
    (val, err)
}

/// `gsl_sf_lngamma_e` restricted to the branches this port covers.
fn lngamma(x: f64) -> Result<(f64, f64)> {
    if libm::fabs(x - 1.0) < 0.01 || libm::fabs(x - 2.0) < 0.01 {
        // lngamma_1_pade / lngamma_2_pade (specfunc/gamma.c:1118,1130).
        return Err(PetirError::Invalid);
    }
    if x >= 0.5 {
        return Ok(lngamma_lanczos(x));
    }
    // x == 0, |x| < 0.02, and the reflection/singular branches (:1137-1176).
    Err(PetirError::Invalid)
}

/// `gsl_sf_gammastar_e` (`specfunc/gamma.c:1300-1344`), branches `0.5 <= x < 10`
/// plus the Stirling tail, which need no further dependencies.
fn gammastar(x: f64) -> Result<(f64, f64)> {
    if x <= 0.0 {
        return Err(PetirError::Domain);
    }
    if x < 0.5 {
        // Needs lngamma below 0.5, which this port refuses.
        return Err(PetirError::Invalid);
    }
    if x < 2.0 {
        let t = 4.0 / 3.0 * (x - 0.5) - 1.0;
        return Ok(cheb(&GSTAR_A_DATA, GSTAR_A_ORDER, -1.0, 1.0, t));
    }
    if x < 10.0 {
        let t = 0.25 * (x - 2.0) - 1.0;
        let (c, ce) = cheb(&GSTAR_B_DATA, GSTAR_B_ORDER, -1.0, 1.0, t);
        let val = c / (x * x) + 1.0 + 1.0 / (12.0 * x);
        return Ok((val, ce / (x * x) + 2.0 * EPS * libm::fabs(val)));
    }
    // GSL_ROOT4_DBL_EPSILON; below this it uses gammastar_ser, not ported.
    if x < 1.0 / 1.2207031250000000e-04 {
        return Err(PetirError::Invalid);
    }
    if x < 1.0 / EPS {
        let xi = 1.0 / x;
        let val = 1.0
            + xi / 12.0 * (1.0 + xi / 24.0 * (1.0 - xi * (139.0 / 180.0 + 571.0 / 8640.0 * xi)));
        return Ok((val, 2.0 * EPS * libm::fabs(val)));
    }
    Ok((1.0, 1.0 / x))
}

/// `gsl_sf_log_1plusx_mx_e` (`specfunc/log.c`) — `log(1+x) - x`.
fn log_1plusx_mx(x: f64) -> Result<(f64, f64)> {
    if x <= -1.0 {
        return Err(PetirError::Domain);
    }
    // GSL guards with a small-x cutoff; sqrt(DBL_MIN) as a literal, since
    // f64::sqrt is std-only.
    if libm::fabs(x) < 1.4916681462400413e-154 {
        return Ok((-0.5 * x * x, EPS * libm::fabs(0.5 * x * x)));
    }
    if libm::fabs(x) < 0.5 {
        let t = 0.5 * (8.0 * x + 1.0) / (x + 2.0);
        let (c, ce) = cheb(&LOPXMX_DATA, LOPXMX_ORDER, -1.0, 1.0, t);
        Ok((x * x * (-0.5 + x * c), x * x * ce))
    } else {
        let lterm = libm::log(1.0 + x);
        let val = lterm - x;
        Ok((val, EPS * (libm::fabs(lterm) + libm::fabs(x))))
    }
}

/// `gamma_inc_D(a, x)` (`specfunc/gamma_inc.c`) — the prefactor
/// `x^a e^{-x} / Γ(a+1)`, computed stably.
fn gamma_inc_d(a: f64, x: f64) -> Result<(f64, f64)> {
    if a < 10.0 {
        let (lg, lg_err) = lngamma(a + 1.0)?;
        let lnr = a * libm::log(x) - x - lg;
        let val = libm::exp(lnr);
        let err = 2.0 * EPS * (libm::fabs(lnr) + 1.0) * libm::fabs(val) + libm::fabs(val) * lg_err;
        Ok((val, err))
    } else {
        let (gstar, gstar_err) = gammastar(a)?;
        let ln_term = if x < 0.5 * a {
            let u = x / a;
            let (lu, _) = log_1plusx_mx(-u)?;
            let ln_u = libm::log(u);
            ln_u - lu
        } else {
            let mu = (x - a) / a;
            let (lmu, _) = log_1plusx_mx(mu)?;
            -lmu
        };
        let term1 = libm::exp(a * ln_term) / libm::sqrt(2.0 * core::f64::consts::PI * a);
        let val = term1 / gstar;
        let err = 2.0 * EPS * (libm::fabs(a * ln_term) + 1.0) * libm::fabs(val)
            + libm::fabs(val) * gstar_err / gstar;
        let _ = HALF_LN_2PI;
        Ok((val, err))
    }
}

/// `gamma_inc_P_series` (`specfunc/gamma_inc.c`).
fn gamma_inc_p_series(a: f64, x: f64) -> Result<(f64, f64)> {
    const NMAX: usize = 10000;
    let (d, d_err) = gamma_inc_d(a, x)?;

    let mut sum = 1.0f64;
    let mut term = 1.0f64;
    let mut n = 1usize;
    while n < NMAX {
        term *= x / (a + n as f64);
        sum += term;
        if libm::fabs(term / sum) < EPS {
            break;
        }
        n += 1;
    }
    if n == NMAX {
        return Err(PetirError::MaxIterations);
    }
    let val = d * sum;
    let err = d_err * libm::fabs(sum) + 4.0 * EPS * (n as f64 + 1.0) * libm::fabs(val);
    Ok((val, err))
}

/// `gamma_inc_F_CF` (`specfunc/gamma_inc.c`) — the continued fraction used by
/// `gamma_inc_Q_CF`, in modified Lentz form as upstream writes it.
fn gamma_inc_f_cf(a: f64, x: f64) -> Result<(f64, f64)> {
    const NMAX: usize = 5000;
    const SMALL: f64 = 1.1929972946060422e-17; // gsl_pow_3(GSL_DBL_EPSILON)

    let mut hn = 1.0f64;
    let mut cn = 1.0 / SMALL;
    let mut dn = 1.0f64;

    // n == 1 has a_1, b_1, b_0 independent of a and x, so upstream does it by
    // hand -- hence the loop starting at 2 and hn/Cn/Dn seeded above.
    let mut n = 2usize;
    while n < NMAX {
        let an = if n % 2 == 1 {
            0.5 * (n as f64 - 1.0) / x
        } else {
            (0.5 * n as f64 - a) / x
        };
        dn = 1.0 + an * dn;
        if libm::fabs(dn) < SMALL {
            dn = SMALL;
        }
        cn = 1.0 + an / cn;
        if libm::fabs(cn) < SMALL {
            cn = SMALL;
        }
        dn = 1.0 / dn;
        let delta = cn * dn;
        hn *= delta;
        if libm::fabs(delta - 1.0) < EPS {
            break;
        }
        n += 1;
    }
    if n == NMAX {
        return Err(PetirError::MaxIterations);
    }
    let err = 2.0 * EPS * libm::fabs(hn) + EPS * (2.0 + 0.5 * n as f64) * libm::fabs(hn);
    Ok((hn, err))
}

/// `gamma_inc_Q_CF` (`specfunc/gamma_inc.c`).
fn gamma_inc_q_cf(a: f64, x: f64) -> Result<(f64, f64)> {
    let (d, d_err) = gamma_inc_d(a, x)?;
    let (f, f_err) = gamma_inc_f_cf(a, x)?;
    let val = d * (a / x) * f;
    let err = d_err * libm::fabs((a / x) * f) + libm::fabs(d * a / x) * f_err;
    Ok((val, err))
}

/// `gamma_inc_Q_large_x` (`specfunc/gamma_inc.c`).
fn gamma_inc_q_large_x(a: f64, x: f64) -> Result<(f64, f64)> {
    const NMAX: usize = 5000;
    let (d, d_err) = gamma_inc_d(a, x)?;

    let mut sum = 1.0f64;
    let mut term = 1.0f64;
    let mut last = 1.0f64;
    let mut n = 1usize;
    while n < NMAX {
        term *= (a - n as f64) / x;
        if libm::fabs(term / last) > 1.0 {
            break;
        }
        if libm::fabs(term / sum) < EPS {
            break;
        }
        sum += term;
        last = term;
        n += 1;
    }
    if n == NMAX {
        return Err(PetirError::MaxIterations);
    }
    let val = d * (a / x) * sum;
    let err = d_err * libm::fabs((a / x) * sum) + 4.0 * EPS * libm::fabs(val);
    Ok((val, err))
}

/// The regularised lower incomplete gamma `P(a, x)` —
/// `gsl_sf_gamma_inc_P_e` (`specfunc/gamma_inc.c`).
///
/// Returns `(value, abserr)`.
///
/// # Errors
/// [`PetirError::Domain`] for `a <= 0` or `x < 0`; [`PetirError::Invalid`] for
/// the branches this port deliberately refuses (see the module docs);
/// [`PetirError::MaxIterations`] if a series or continued fraction does not
/// converge.
pub fn gamma_inc_p(a: f64, x: f64) -> Result<(f64, f64)> {
    if a <= 0.0 || x < 0.0 {
        return Err(PetirError::Domain);
    }
    if x == 0.0 {
        return Ok((0.0, 0.0));
    }
    if x < 20.0 || x < 0.5 * a {
        return gamma_inc_p_series(a, x);
    }
    if a > 1.0e6 && (x - a) * (x - a) < a {
        // gamma_inc_Q_asymp_unif -- unreachable for the a this port serves.
        return Err(PetirError::Invalid);
    }
    if a <= x {
        let (q, q_err) = if a > 0.2 * x {
            gamma_inc_q_cf(a, x)?
        } else {
            gamma_inc_q_large_x(a, x)?
        };
        let val = 1.0 - q;
        return Ok((val, q_err + 2.0 * EPS * libm::fabs(val)));
    }
    // a > x, x >= 20: upstream uses Q_CF here too.
    let (q, q_err) = gamma_inc_q_cf(a, x)?;
    let val = 1.0 - q;
    Ok((val, q_err + 2.0 * EPS * libm::fabs(val)))
}

/// The **lower incomplete gamma** `γ(a, x) = ∫₀ˣ t^(a-1) e^(-t) dt`.
///
/// This is SLATEC's `gami(a, x)`, which is what NJOY's `fmn` calls
/// (`acefc.f90:9155`). It is `Γ(a) P(a, x)`, and `Γ(a)` comes from
/// [`lngamma`]'s Lanczos branch.
///
/// # Errors
/// As [`gamma_inc_p`].
pub fn gamma_inc_lower(a: f64, x: f64) -> Result<(f64, f64)> {
    let (p, p_err) = gamma_inc_p(a, x)?;
    let (lg, lg_err) = lngamma(a)?;
    let gamma_a = libm::exp(lg);
    let val = gamma_a * p;
    let err = gamma_a * p_err + libm::fabs(val) * lg_err;
    Ok((val, err))
}
