//! QNG: non-adaptive Gauss-Kronrod-Patterson quadrature (gh:#1340), ported
//! from GSL 2.8 `integration/qng.c` (lines 29-190) and `qng.h` (the
//! coefficient tables, copied from the file by script, not typed), commit
//! `cf180cd7`, read 2026-11-13. Copyright (C) 1996-2000, 2007 Brian Gough
//! (GPL-3.0-or-later). GSL's QNG is QUADPACK's `dqng` (public domain).
//! Research, education and V&V only.
//!
//! The 10-, 21-, 43- and 87-point rules are applied in turn, re-using every
//! function value already computed, until the error estimate meets
//! `max(epsabs, epsrel |I|)`. `neval` is 21, 43 or 87.
//!
//! # Differences from upstream
//!
//! - Every array access is a checked `get` (the gate `no_panic_gate` bans
//!   subscripts); all indices are loop constants, so the fallback is
//!   unreachable.
//! - Argument errors are [`PetirError::Tolerance`], and non-finite limits
//!   [`PetirError::Domain`] (upstream does not check them).
//! - `GSL_FN_EVAL(f, c + x) + GSL_FN_EVAL(f, c - x)` evaluates the plus point
//!   first; C leaves the order unspecified, and it affects nothing but the
//!   order of calls to a stateful integrand.

#[allow(unused_imports)]
use crate::real::Real;
use crate::PetirError;
use crate::Result;

use super::rescale_error;
use crate::scalar::DBL_EPSILON;

/// x1, abscissae common to the 10-, 21-, 43- and 87-point rule
const X1: [f64; 5] = [
    0.973906528517171720077964012084452,
    0.865063366688984510732096688423493,
    0.679409568299024406234327365114874,
    0.433395394129247190799265943165784,
    0.148874338981631210884826001129720,
];
/// w10, weights of the 10-point formula
const W10: [f64; 5] = [
    0.066671344308688137593568809893332,
    0.149451349150580593145776339657697,
    0.219086362515982043995534934228163,
    0.269266719309996355091226921569469,
    0.295524224714752870173892994651338,
];
/// x2, abscissae common to the 21-, 43- and 87-point rule
const X2: [f64; 5] = [
    0.995657163025808080735527280689003,
    0.930157491355708226001207180059508,
    0.780817726586416897063717578345042,
    0.562757134668604683339000099272694,
    0.294392862701460198131126603103866,
];
/// w21a, weights of the 21-point formula for abscissae x1
const W21A: [f64; 5] = [
    0.032558162307964727478818972459390,
    0.075039674810919952767043140916190,
    0.109387158802297641899210590325805,
    0.134709217311473325928054001771707,
    0.147739104901338491374841515972068,
];
/// w21b, weights of the 21-point formula for abscissae x2
const W21B: [f64; 6] = [
    0.011694638867371874278064396062192,
    0.054755896574351996031381300244580,
    0.093125454583697605535065465083366,
    0.123491976262065851077958109831074,
    0.142775938577060080797094273138717,
    0.149445554002916905664936468389821,
];
/// x3, abscissae common to the 43- and 87-point rule
const X3: [f64; 11] = [
    0.999333360901932081394099323919911,
    0.987433402908088869795961478381209,
    0.954807934814266299257919200290473,
    0.900148695748328293625099494069092,
    0.825198314983114150847066732588520,
    0.732148388989304982612354848755461,
    0.622847970537725238641159120344323,
    0.499479574071056499952214885499755,
    0.364901661346580768043989548502644,
    0.222254919776601296498260928066212,
    0.074650617461383322043914435796506,
];
/// w43a, weights of the 43-point formula for abscissae x1, x3
const W43A: [f64; 10] = [
    0.016296734289666564924281974617663,
    0.037522876120869501461613795898115,
    0.054694902058255442147212685465005,
    0.067355414609478086075553166302174,
    0.073870199632393953432140695251367,
    0.005768556059769796184184327908655,
    0.027371890593248842081276069289151,
    0.046560826910428830743339154433824,
    0.061744995201442564496240336030883,
    0.071387267268693397768559114425516,
];
/// w43b, weights of the 43-point formula for abscissae x3
const W43B: [f64; 12] = [
    0.001844477640212414100389106552965,
    0.010798689585891651740465406741293,
    0.021895363867795428102523123075149,
    0.032597463975345689443882222526137,
    0.042163137935191811847627924327955,
    0.050741939600184577780189020092084,
    0.058379395542619248375475369330206,
    0.064746404951445885544689259517511,
    0.069566197912356484528633315038405,
    0.072824441471833208150939535192842,
    0.074507751014175118273571813842889,
    0.074722147517403005594425168280423,
];
/// x4, abscissae of the 87-point rule
const X4: [f64; 22] = [
    0.999902977262729234490529830591582,
    0.997989895986678745427496322365960,
    0.992175497860687222808523352251425,
    0.981358163572712773571916941623894,
    0.965057623858384619128284110607926,
    0.943167613133670596816416634507426,
    0.915806414685507209591826430720050,
    0.883221657771316501372117548744163,
    0.845710748462415666605902011504855,
    0.803557658035230982788739474980964,
    0.757005730685495558328942793432020,
    0.706273209787321819824094274740840,
    0.651589466501177922534422205016736,
    0.593223374057961088875273770349144,
    0.531493605970831932285268948562671,
    0.466763623042022844871966781659270,
    0.399424847859218804732101665817923,
    0.329874877106188288265053371824597,
    0.258503559202161551802280975429025,
    0.185695396568346652015917141167606,
    0.111842213179907468172398359241362,
    0.037352123394619870814998165437704,
];
/// w87a, weights of the 87-point formula for abscissae x1, x2, x3
const W87A: [f64; 21] = [
    0.008148377384149172900002878448190,
    0.018761438201562822243935059003794,
    0.027347451050052286161582829741283,
    0.033677707311637930046581056957588,
    0.036935099820427907614589586742499,
    0.002884872430211530501334156248695,
    0.013685946022712701888950035273128,
    0.023280413502888311123409291030404,
    0.030872497611713358675466394126442,
    0.035693633639418770719351355457044,
    0.000915283345202241360843392549948,
    0.005399280219300471367738743391053,
    0.010947679601118931134327826856808,
    0.016298731696787335262665703223280,
    0.021081568889203835112433060188190,
    0.025370969769253827243467999831710,
    0.029189697756475752501446154084920,
    0.032373202467202789685788194889595,
    0.034783098950365142750781997949596,
    0.036412220731351787562801163687577,
    0.037253875503047708539592001191226,
];
/// w87b, weights of the 87-point formula for abscissae x4
const W87B: [f64; 23] = [
    0.000274145563762072350016527092881,
    0.001807124155057942948341311753254,
    0.004096869282759164864458070683480,
    0.006758290051847378699816577897424,
    0.009549957672201646536053581325377,
    0.012329447652244853694626639963780,
    0.015010447346388952376697286041943,
    0.017548967986243191099665352925900,
    0.019938037786440888202278192730714,
    0.022194935961012286796332102959499,
    0.024339147126000805470360647041454,
    0.026374505414839207241503786552615,
    0.028286910788771200659968002987960,
    0.030052581128092695322521110347341,
    0.031646751371439929404586051078883,
    0.033050413419978503290785944862689,
    0.034255099704226061787082821046821,
    0.035262412660156681033782717998428,
    0.036076989622888701185500318003895,
    0.036698604498456094498018047441094,
    0.037120549269832576114119958413599,
    0.037334228751935040321235449094698,
    0.037361073762679023410321241766599,
];

/// The outcome of [`qng_with_status`]: GSL writes `*result`, `*abserr` and
/// `*neval` even when it returns an error status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QngOutcome {
    /// `*result`.
    pub value: f64,
    /// `*abserr`.
    pub abserr: f64,
    /// `*neval`: 21, 43 or 87 (0 for an argument error).
    pub neval: usize,
    /// `Ok(())` for `GSL_SUCCESS`, `Err(Tolerance)` for `GSL_EBADTOL` or
    /// `GSL_ETOL`.
    pub status: Result<()>,
    /// GSL's own integer status: 0 success, 13 `GSL_EBADTOL` (bad `epsabs`,
    /// `epsrel` pair), 14 `GSL_ETOL` (the 87-point rule missed the
    /// tolerance), 1 `GSL_EDOM` (non-finite limit, PETIR only).
    ///
    /// Present because [`PetirError`] has one `Tolerance` variant and the
    /// integration routines use it for `GSL_EBADTOL` (as `qags` does); a
    /// variant for `GSL_ETOL` would edit the kovan-reviewed enum, which is
    /// deferred to the maintainer (gh:#1340).
    pub gsl_status: i32,
}

/// Checked read; the indices below are loop constants, so `0.0` is unreachable.
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

/// Checked write.
fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// `|v - mean|`, upstream's `fabs (v - mean)`.
fn dev(v: f64, mean: f64) -> f64 {
    (v - mean).abs()
}

/// `gsl_integration_qng` (`qng.c` lines 29-190): non-adaptive 21/43/87-point
/// Gauss-Kronrod-Patterson integration of `f` over `[a, b]`.
///
/// # Errors
///
/// [`PetirError::Tolerance`] for `epsabs <= 0` with a too-small `epsrel`
/// (`GSL_EBADTOL`) and when the 87-point rule misses the tolerance
/// (`GSL_ETOL`); [`PetirError::Domain`] for a non-finite limit.
///
/// # Example
///
/// ```
/// use petir::integration::qng;
/// let r = qng(|x: f64| x * x * x, 0.0, 2.0, 0.0, 1e-10).unwrap();
/// assert!((r.value - 4.0).abs() < 1e-12);
/// assert_eq!(r.neval, 21);
/// ```
pub fn qng<F>(f: F, a: f64, b: f64, epsabs: f64, epsrel: f64) -> Result<QngOutcome>
where
    F: Fn(f64) -> f64,
{
    let o = qng_with_status(f, a, b, epsabs, epsrel);
    o.status.map(|()| o)
}

/// [`qng`], keeping GSL's `*result`, `*abserr` and `*neval` when the status is
/// an error.
pub fn qng_with_status<F>(f: F, a: f64, b: f64, epsabs: f64, epsrel: f64) -> QngOutcome
where
    F: Fn(f64) -> f64,
{
    let mut out = QngOutcome {
        value: 0.0,
        abserr: 0.0,
        neval: 0,
        status: Ok(()),
        gsl_status: 0,
    };
    if !(a.is_finite() && b.is_finite()) {
        out.status = Err(PetirError::Domain);
        out.gsl_status = 1;
        return out;
    }
    let mut fv1 = [0.0_f64; 5];
    let mut fv2 = [0.0_f64; 5];
    let mut fv3 = [0.0_f64; 5];
    let mut fv4 = [0.0_f64; 5];
    let mut savfun = [0.0_f64; 21];

    let half_length = 0.5 * (b - a);
    let abs_half_length = half_length.abs();
    let center = 0.5 * (b + a);
    let f_center = f(center);

    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        out.status = Err(PetirError::Tolerance);
        out.gsl_status = 13;
        return out;
    }

    // Compute the integral using the 10- and 21-point formula.
    let mut res10 = 0.0;
    let mut res21 = at(&W21B, 5) * f_center;
    let mut resabs = at(&W21B, 5) * f_center.abs();

    for k in 0..5 {
        let abscissa = half_length * at(&X1, k);
        let fval1 = f(center + abscissa);
        let fval2 = f(center - abscissa);
        let fval = fval1 + fval2;
        res10 += at(&W10, k) * fval;
        res21 += at(&W21A, k) * fval;
        resabs += at(&W21A, k) * (fval1.abs() + fval2.abs());
        put(&mut savfun, k, fval);
        put(&mut fv1, k, fval1);
        put(&mut fv2, k, fval2);
    }

    for k in 0..5 {
        let abscissa = half_length * at(&X2, k);
        let fval1 = f(center + abscissa);
        let fval2 = f(center - abscissa);
        let fval = fval1 + fval2;
        res21 += at(&W21B, k) * fval;
        resabs += at(&W21B, k) * (fval1.abs() + fval2.abs());
        put(&mut savfun, k + 5, fval);
        put(&mut fv3, k, fval1);
        put(&mut fv4, k, fval2);
    }

    resabs *= abs_half_length;

    let mean = 0.5 * res21;
    let mut resasc = at(&W21B, 5) * dev(f_center, mean);
    for k in 0..5 {
        resasc += at(&W21A, k) * (dev(at(&fv1, k), mean) + dev(at(&fv2, k), mean))
            + at(&W21B, k) * (dev(at(&fv3, k), mean) + dev(at(&fv4, k), mean));
    }
    resasc *= abs_half_length;

    let mut result_kronrod = res21 * half_length;
    let mut err = rescale_error((res21 - res10) * half_length, resabs, resasc);

    // Test for convergence.
    if err < epsabs || err < epsrel * result_kronrod.abs() {
        out.value = result_kronrod;
        out.abserr = err;
        out.neval = 21;
        return out;
    }

    // Compute the integral using the 43-point formula.
    let mut res43 = at(&W43B, 11) * f_center;
    for k in 0..10 {
        res43 += at(&savfun, k) * at(&W43A, k);
    }
    for k in 0..11 {
        let abscissa = half_length * at(&X3, k);
        let fval = f(center + abscissa) + f(center - abscissa);
        res43 += fval * at(&W43B, k);
        put(&mut savfun, k + 10, fval);
    }

    result_kronrod = res43 * half_length;
    err = rescale_error((res43 - res21) * half_length, resabs, resasc);

    if err < epsabs || err < epsrel * result_kronrod.abs() {
        out.value = result_kronrod;
        out.abserr = err;
        out.neval = 43;
        return out;
    }

    // Compute the integral using the 87-point formula.
    let mut res87 = at(&W87B, 22) * f_center;
    for k in 0..21 {
        res87 += at(&savfun, k) * at(&W87A, k);
    }
    for k in 0..22 {
        let abscissa = half_length * at(&X4, k);
        res87 += at(&W87B, k) * (f(center + abscissa) + f(center - abscissa));
    }

    result_kronrod = res87 * half_length;
    err = rescale_error((res87 - res43) * half_length, resabs, resasc);

    out.value = result_kronrod;
    out.abserr = err;
    out.neval = 87;
    if !(err < epsabs || err < epsrel * result_kronrod.abs()) {
        // "failed to reach tolerance with highest-order rule" (GSL_ETOL).
        out.status = Err(PetirError::Tolerance);
        out.gsl_status = 14;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_escalate_and_refuse() {
        // A cubic is exact in the 21-point rule.
        let r = qng(|x: f64| x * x * x, 0.0, 2.0, 0.0, 1e-10).unwrap();
        assert_eq!(r.neval, 21);
        let o = qng_with_status(|x: f64| 1.0 / Real::sqrt(x), 0.0, 1.0, 0.0, 1e-8);
        assert_eq!((o.neval, o.gsl_status), (87, 14));
        assert_eq!(o.status, Err(PetirError::Tolerance));
        let o = qng_with_status(|x: f64| x, 0.0, 1.0, 0.0, 0.0);
        assert_eq!((o.neval, o.status), (0, Err(PetirError::Tolerance)));
        assert_eq!(o.gsl_status, 13);
        assert_eq!(
            qng_with_status(|x: f64| x, 0.0, f64::NAN, 0.0, 1e-8).status,
            Err(PetirError::Domain)
        );
    }
}
