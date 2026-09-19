// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/psi.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The five coefficient tables here were extracted from that source by script,
// not retyped, and are audited bit-for-bit against it by
// `tests/gsl_tables_audit.rs`. The three macro-valued entries GSL writes as
// `-M_EULER`, `M_PI*M_PI/6.0` and so on were expanded to the exact `f64` a C
// compiler produces, and the audit performs the same expansion.
//
// NOT PORTED: `gsl_sf_complex_psi_e`. It needs gsl_complex, which this crate
// deliberately does not have, and it has no caller here. Said plainly rather
// than left as a silent gap.

//! The digamma function and its derivatives: [`psi`], [`psi_1`], [`psi_n`],
//! and the integer-argument forms [`psi_int`] and [`psi_1_int`].
//!
//! # What these are
//!
//! `psi(x)` is the logarithmic derivative of the gamma function,
//! `d/dx ln Gamma(x)`, and `psi_n(n, x)` is its `n`-th derivative — the
//! polygamma functions. `psi_1` (the trigamma function) is the `n = 1` case
//! and is common enough to have its own entry point.
//!
//! They arrive wherever a gamma function is differentiated: the derivative of
//! a maximum-likelihood fit to a gamma or Dirichlet distribution, the mean of
//! `ln X` under a gamma law, and the asymptotic expansions of the incomplete
//! gamma and beta functions.
//!
//! # Argument ranges, stated plainly
//!
//! | function | domain | outside it |
//! |---|---|---|
//! | [`psi`] | all `x` except `0, -1, -2, ...` | `NaN` at the poles |
//! | [`psi_1`] | the same | `NaN` |
//! | [`psi_n`] | `n = 0, 1` as above; `n >= 2` needs `x > 0` | `NaN` |
//! | [`psi_int`], [`psi_1_int`] | `n >= 1` | `NaN` |
//! | [`psi_1piy`] | all real `y` — this is `Re psi(1 + iy)` | — |
//!
//! Note the pole check follows GSL's and tests only `x == 0`, `-1`, `-2`
//! exactly. At `-3` and beyond the reflection formula's `1/sin(pi x)` blows
//! up on its own and the result is a large finite number rather than an
//! error; that is upstream's behaviour and this port keeps it.
//!
//! All arguments and results are dimensionless (`f64`).
//!
//! # Accuracy
//!
//! Measured against identities that share no table with the implementation —
//! the recurrence `psi(x+1) = psi(x) + 1/x`, the reflection formula, the
//! duplication formula, and the closed forms at the half-integers. Figures
//! are in each function's doc comment and were measured, not predicted.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl;
use crate::specfunc::{exp_mult, gamma::ln_factorial, zeta::hzeta, EULER, SQRT_DBL_MIN};

use core::f64::consts::PI;

/// `PSI_TABLE_NMAX` — the largest `n` for which `psi(n)` is tabulated.
const PSI_TABLE_NMAX: u32 = 100;
/// `PSI_1_TABLE_NMAX` — the same for `psi(1, n)`.
const PSI_1_TABLE_NMAX: u32 = 100;
#[rustfmt::skip]
const R1PY: [f64; 30] = [
    1.59888328244976954803168395603, 0.67905625353213463845115658455,
    -0.068485802980122530009506482524, -0.005788184183095866792008831182,
    0.008511258167108615980419855648, -0.004042656134699693434334556409,
    0.001352328406159402601778462956, -0.000311646563930660566674525382,
    0.000018507563785249135437219139, 0.000028348705427529850296492146,
    -0.000019487536014574535567541960, 8.0709788710834469408621587335e-06,
    -2.2983564321340518037060346561e-06, 3.0506629599604749843855962658e-07,
    1.3042238632418364610774284846e-07, -1.2308657181048950589464690208e-07,
    5.7710855710682427240667414345e-08, -1.8275559342450963966092636354e-08,
    3.1020471300626589420759518930e-09, 6.8989327480593812470039430640e-10,
    -8.7182290258923059852334818997e-10, 4.4069147710243611798213548777e-10,
    -1.4727311099198535963467200277e-10, 2.7589682523262644748825844248e-11,
    4.1871826756975856411554363568e-12, -6.5673460487260087541400767340e-12,
    3.4487900886723214020103638000e-12, -1.1807251417448690607973794078e-12,
    2.3798314343969589258709315574e-13, 2.1663630410818831824259465821e-15,
];
#[rustfmt::skip]
const PSI_CS: [f64; 23] = [
    -0.038057080835217922, 0.491415393029387130, -0.056815747821244730, 0.008357821225914313,
    -0.001333232857994342, 0.000220313287069308, -0.000037040238178456, 0.000006283793654854,
    -0.000001071263908506, 0.000000183128394654, -0.000000031353509361, 0.000000005372808776,
    -0.000000000921168141, 0.000000000157981265, -0.000000000027098646, 0.000000000004648722,
    -0.000000000000797527, 0.000000000000136827, -0.000000000000023475, 0.000000000000004027,
    -0.000000000000000691, 0.000000000000000118, -0.000000000000000020,
];
#[rustfmt::skip]
const APSI_CS: [f64; 16] = [
    -0.0204749044678185, -0.0101801271534859, 0.0000559718725387, -0.0000012917176570,
    0.0000000572858606, -0.0000000038213539, 0.0000000003397434, -0.0000000000374838,
    0.0000000000048990, -0.0000000000007344, 0.0000000000001233, -0.0000000000000228,
    0.0000000000000045, -0.0000000000000009, 0.0000000000000002, -0.0000000000000000,
];
#[rustfmt::skip]
const PSI_TABLE: [f64; 101] = [
    0.0, -0.57721566490153286060651209008240243104215933593992, 0.42278433509846713939348790992,
    0.92278433509846713939348790992, 1.25611766843180047272682124325,
    1.50611766843180047272682124325, 1.70611766843180047272682124325,
    1.87278433509846713939348790992, 2.01564147795560999653634505277,
    2.14064147795560999653634505277, 2.25175258906672110764745616389,
    2.35175258906672110764745616389, 2.44266167997581201673836525479,
    2.52599501330914535007169858813, 2.60291809023222227314862166505,
    2.67434666166079370172005023648, 2.74101332832746036838671690315,
    2.80351332832746036838671690315, 2.86233685773922507426906984432,
    2.91789241329478062982462539988, 2.97052399224214905087725697883,
    3.02052399224214905087725697883, 3.06814303986119666992487602645,
    3.11359758531574212447033057190, 3.15707584618530734186163491973,
    3.1987425128519740085283015864, 3.2387425128519740085283015864,
    3.2772040513135124700667631249, 3.3142410883505495071038001619,
    3.3499553740648352213895144476, 3.3844381326855248765619282407,
    3.4177714660188582098952615740, 3.4500295305349872421533260902,
    3.4812795305349872421533260902, 3.5115825608380175451836291205,
    3.5409943255438998981248055911, 3.5695657541153284695533770196,
    3.5973435318931062473311547974, 3.6243705589201332743581818244,
    3.6506863483938174848844976139, 3.6763273740348431259101386396,
    3.7013273740348431259101386396, 3.7257176179372821503003825420,
    3.7495271417468059598241920658, 3.7727829557002943319172153216,
    3.7955102284275670591899425943, 3.8177324506497892814121648166,
    3.8394715810845718901078169905, 3.8607481768292527411716467777,
    3.8815815101625860745049801110, 3.9019896734278921969539597029,
    3.9219896734278921969539597029, 3.9415975165651470989147440166,
    3.9608282857959163296839747858, 3.9796962103242182164764276160,
    3.9982147288427367349949461345, 4.0163965470245549168131279527,
    4.0342536898816977739559850956, 4.0517975495308205809735289552,
    4.0690389288411654085597358518, 4.0859880813835382899156680552,
    4.1026547480502049565823347218, 4.1190481906731557762544658694,
    4.1351772229312202923834981274, 4.1510502388042361653993711433,
    4.1666752388042361653993711433, 4.1820598541888515500147557587,
    4.1972113693403667015299072739, 4.2121367424746950597388624977,
    4.2268426248276362362094507330, 4.2413353784508246420065521823,
    4.2556210927365389277208378966, 4.2697055997787924488475984600,
    4.2835944886676813377364873489, 4.2972931188046676391063503626,
    4.3108066323181811526198638761, 4.3241399656515144859531972094,
    4.3372978603883565912163551041, 4.3502848733753695782293421171,
    4.3631053861958823987421626300, 4.3757636140439836645649474401,
    4.3882636140439836645649474401, 4.4006092930563293435772931191,
    4.4128044150075488557724150703, 4.4248526077786331931218126607,
    4.4367573696833950978837174226, 4.4485220755657480390601880108,
    4.4601499825424922251066996387, 4.4716442354160554434975042364,
    4.4830078717796918071338678728, 4.4942438268358715824147667492,
    4.5053549379469826935258778603, 4.5163439489359936825368668713,
    4.5272135141533849868846929582, 4.5379662023254279976373811303,
    4.5486045001977684231692960239, 4.5591308159872421073798223397,
    4.5695474826539087740464890064, 4.5798567610044242379640147796,
    4.5900608426370772991885045755, 4.6001618527380874001986055856,
];
#[rustfmt::skip]
const PSI_1_TABLE: [f64; 101] = [
    0.0, 1.6449340668482264, 0.644934066848226436472415, 0.394934066848226436472415,
    0.2838229557371153253613041, 0.2213229557371153253613041, 0.1813229557371153253613041,
    0.1535451779593375475835263, 0.1331370146940314251345467, 0.1175120146940314251345467,
    0.1051663356816857461222010, 0.0951663356816857461222010, 0.0869018728717683907503002,
    0.0799574284273239463058557, 0.0740402686640103368384001, 0.0689382278476838062261552,
    0.0644937834032393617817108, 0.0605875334032393617817108, 0.0571273257907826143768665,
    0.0540409060376961946237801, 0.0512708229352031198315363, 0.0487708229352031198315363,
    0.0465032492390579951149830, 0.0444371335365786562720078, 0.0425467743683366902984728,
    0.0408106632572255791873617, 0.0392106632572255791873617, 0.0377313733163971768204978,
    0.0363596312039143235969038, 0.0350841209998326909438426, 0.0338950603577399442137594,
    0.0327839492466288331026483, 0.0317433665203020901265817, 0.03076680402030209012658168,
    0.02984853037475571730748159, 0.02898347847164153045627052, 0.02816715194102928555831133,
    0.02739554700275768062003973, 0.02666508681283803124093089, 0.02597256603721476254286995,
    0.02531510384129102815759710, 0.02469010384129102815759710, 0.02409521984367056414807896,
    0.02352832641963428296894063, 0.02298749353699501850166102, 0.02247096461137518379091722,
    0.02197713745088135663042339, 0.02150454765882086513703965, 0.02105185413233829383780923,
    0.02061782635456051606003145, 0.02020133322669712580597065, 0.01980133322669712580597065,
    0.01941686571420193164987683, 0.01904704322899483105816086, 0.01869104465298913508094477,
    0.01834810912486842177504628, 0.01801753061247172756017024, 0.01769865306145131939690494,
    0.01739086605006319997554452, 0.01709360088954001329302371, 0.01680632711763538818529605,
    0.01652854933985761040751827, 0.01625980437882562975715546, 0.01599965869724394401313881,
    0.01574770606433893015574400, 0.01550356543933893015574400, 0.01526687904880638577704578,
    0.01503731063741979257227076, 0.01481454387422086185273411, 0.01459828089844231513993134,
    0.01438824099085987447620523, 0.01418415935820681325171544, 0.01398578601958352422176106,
    0.01379288478501562298719316, 0.01360523231738567365335942, 0.01342261726990576130858221,
    0.01324483949212798353080444, 0.01307170929822216635628920, 0.01290304679189732236910755,
    0.01273868124291638877278934, 0.01257845051066194236996928, 0.01242220051066194236996928,
    0.01226978472038606978956995, 0.01212106372098095378719041, 0.01197590477193174490346273,
    0.01183418141592267460867815, 0.01169577311142440471248438, 0.01156056489076458859566448,
    0.01142844704164317229232189, 0.01129931481023821361463594, 0.01117306812421372175754719,
    0.01104961133409026496742374, 0.01092885297157366069257770, 0.01081070552355853781923177,
    0.01069508522063334415522437, 0.01058191183901270133041676, 0.01047110851491297833872701,
    0.01036260157046853389428257, 0.01025632035036012704977199, 0.01015219706839427948625679,
    0.01005016666333357139524567,
];

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// `psi(x)` for real `x`. Ports `psi_x`.
///
/// Two regimes. For `|x| >= 2` the asymptotic Chebyshev series in
/// `8/x^2 - 1`, plus the reflection term `-pi cot(pi x)` when `x < 0`. For
/// `-2 < x < 2` a Chebyshev series in `2v - 1` around the nearest of
/// `x = 1, 0, -1, -2`, with the intervening poles pulled out explicitly as
/// `1/x` terms — which is what keeps the series from having to represent
/// them.
fn psi_x(x: f64) -> f64 {
    let y = x.abs();
    if x == 0.0 || x == -1.0 || x == -2.0 {
        return f64::NAN;
    }
    if y >= 2.0 {
        let t = 8.0 / (y * y) - 1.0;
        let c = eval_gsl(t, &APSI_CS);
        if x < 0.0 {
            let s = (PI * x).sin();
            let co = (PI * x).cos();
            if s.abs() < 2.0 * SQRT_DBL_MIN {
                return f64::NAN;
            }
            return y.ln() - 0.5 / x + c - PI * co / s;
        }
        return y.ln() - 0.5 / x + c;
    }
    // -2 < x < 2
    if x < -1.0 {
        // x = -2 + v
        let v = x + 2.0;
        let t1 = 1.0 / x;
        let t2 = 1.0 / (x + 1.0);
        let t3 = 1.0 / v;
        return -(t1 + t2 + t3) + eval_gsl(2.0 * v - 1.0, &PSI_CS);
    }
    if x < 0.0 {
        // x = -1 + v
        let v = x + 1.0;
        let t1 = 1.0 / x;
        let t2 = 1.0 / v;
        return -(t1 + t2) + eval_gsl(2.0 * v - 1.0, &PSI_CS);
    }
    if x < 1.0 {
        // x = v
        return -1.0 / x + eval_gsl(2.0 * x - 1.0, &PSI_CS);
    }
    // x = 1 + v
    eval_gsl(2.0 * (x - 1.0) - 1.0, &PSI_CS)
}

/// `psi_n(n, x)` for `n >= 1` and `x > 0`, via Abramowitz & Stegun 6.4.10:
/// `psi_n(n, x) = (-1)^{n+1} n! zeta(n+1, x)`. Ports `psi_n_xg0`.
fn psi_n_xg0(n: u32, x: f64) -> f64 {
    if n == 0 {
        return psi_x(x);
    }
    let ln_nf = ln_factorial(n);
    let hz = hzeta(n as f64 + 1.0, x);
    let v = exp_mult(ln_nf, hz);
    if n % 2 == 0 {
        -v
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The digamma function `psi(x) = d/dx ln Gamma(x)`.
///
/// # Domain and range
///
/// Every `x` except the poles at `0, -1, -2, ...`. Returns `NaN` at `0`,
/// `-1` and `-2`, which are the three GSL tests for exactly; at `-3` and
/// beyond the reflection term diverges on its own and a large finite number
/// comes back instead. `psi(1) = -gamma`, the Euler-Mascheroni constant.
///
/// # Accuracy
///
/// Measured against four independent identities:
///
/// | check | worst |
/// |---|---|
/// | duplication, `psi(2x) = (psi(x) + psi(x + 1/2))/2 + ln 2` | 1.321e-15 |
/// | reflection, `psi(1 - x) - psi(x) = pi cot(pi x)` | 5.760e-15 |
/// | recurrence, `psi(x + 1) - psi(x) = 1/x` | 2.057e-14 |
/// | `psi(1) = -gamma`, `psi(1/2) = -gamma - 2 ln 2` | exact to 1e-15 |
///
/// The recurrence row is the loosest and is the *test's* limit rather than
/// `psi`'s: at `x = 24.5` both terms are about 3.2 and their difference is
/// 0.04, so forming it throws away two decimal digits. Read the duplication
/// row as the accuracy statement.
///
/// # Example
///
/// ```
/// use petir::specfunc::psi;
/// // psi(1) is -gamma.
/// assert!((psi(1.0) + 0.577_215_664_901_532_9).abs() < 1e-15);
/// // and the recurrence psi(x+1) = psi(x) + 1/x.
/// assert!((psi(3.5) - psi(2.5) - 1.0 / 2.5).abs() < 1e-14);
/// ```
pub fn psi(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    psi_x(x)
}

/// `psi(n)` for positive integer `n`, from a 100-entry table.
///
/// # Domain and range
///
/// `n >= 1`; returns `NaN` at `n = 0`, which is a pole. Past `n = 100` an
/// Abramowitz & Stegun 6.3.18 asymptotic series takes over.
///
/// # Accuracy
///
/// 4.347e-16 relative against [`psi`] over `n` in `[1, 400]` — the table and
/// the Chebyshev branch are independent routes — and 5.606e-16 against the
/// harmonic-number identity `psi(n) = -gamma + sum_{k<n} 1/k`, which is
/// independent of both. Measured in this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::psi_int;
/// // psi(n) = -gamma + sum_{k=1}^{n-1} 1/k.
/// let expect = -0.577_215_664_901_532_9 + 1.0 + 0.5 + 1.0 / 3.0;
/// assert!((psi_int(4) - expect).abs() < 1e-15);
/// assert!(psi_int(0).is_nan());
/// ```
pub fn psi_int(n: u32) -> f64 {
    if n == 0 {
        return f64::NAN;
    }
    if n <= PSI_TABLE_NMAX {
        return PSI_TABLE.get(n as usize).copied().unwrap_or(f64::NAN);
    }
    // Abramowitz & Stegun 6.3.18.
    let nf = n as f64;
    let c2 = -1.0 / 12.0;
    let c3 = 1.0 / 120.0;
    let c4 = -1.0 / 252.0;
    let c5 = 1.0 / 240.0;
    let ni2 = (1.0 / nf) * (1.0 / nf);
    let ser = ni2 * (c2 + ni2 * (c3 + ni2 * (c4 + ni2 * c5)));
    nf.ln() - 0.5 / nf + ser
}

/// `Re psi(1 + iy)`, the real part of the digamma function on the line
/// `x = 1`.
///
/// # Why this is a separate entry point
///
/// The full complex digamma needs a complex type this crate deliberately does
/// not have, but the *real part* on this one line is real-valued throughout
/// and is what Fermi-Dirac and Bose-Einstein integrals actually call for. GSL
/// gives it its own routine for the same reason.
///
/// # Domain and range
///
/// Every real `y`. Even in `y`, since `psi(1 - iy)` is the conjugate of
/// `psi(1 + iy)`.
///
/// # Accuracy
///
/// 7.136e-13 relative against the series `-gamma + sum_{n>=1} y^2/(n(n^2 +
/// y^2))` over `y` in `(0, 5]`, measured in this module's tests.
///
/// **That is three orders looser than the rest of this module, and it is
/// upstream's own limit rather than the port's.** At `y = 1` the innermost
/// branch is accurate to 2.776e-17 while the Chebyshev branch just above it
/// is accurate to 1.258e-13. GSL flags exactly this branch in its source
/// with `result->err *= 5.0; /* FIXME: losing a digit somewhere... */`; the
/// port reproduces it rather than quietly improving it, and
/// `psi_1piy_is_even_and_its_branches_agree_at_the_cut` pins the measurement.
///
/// # Example
///
/// ```
/// use petir::specfunc::{psi, psi_1piy};
/// // On the real axis it reduces to psi(1).
/// assert!((psi_1piy(0.0) - psi(1.0)).abs() < 1e-15);
/// ```
pub fn psi_1piy(y: f64) -> f64 {
    if y.is_nan() {
        return f64::NAN;
    }
    let ay = y.abs();
    if ay > 1000.0 {
        // Abramowitz & Stegun 6.3.19, three terms.
        let yi2 = 1.0 / (ay * ay);
        let lny = ay.ln();
        let sum = yi2 * (1.0 / 12.0 + 1.0 / 120.0 * yi2 + 1.0 / 252.0 * yi2 * yi2);
        return lny + sum;
    }
    if ay > 10.0 {
        // The same expansion, six terms.
        let yi2 = 1.0 / (ay * ay);
        let lny = ay.ln();
        let sum = yi2
            * (1.0 / 12.0
                + yi2
                    * (1.0 / 120.0
                        + yi2
                            * (1.0 / 252.0
                                + yi2
                                    * (1.0 / 240.0
                                        + yi2 * (1.0 / 132.0 + 691.0 / 32760.0 * yi2)))));
        return lny + sum;
    }
    if ay > 1.0 {
        let y2 = ay * ay;
        let t = (2.0 * ay - 11.0) / 9.0;
        let v = y2 * (1.0 / (1.0 + y2) + 0.5 / (4.0 + y2));
        return eval_gsl(t, &R1PY) - EULER + v;
    }
    // Abramowitz & Stegun 6.3.17, summed to M = 50 with the tail as a
    // polynomial in y^2. Upstream's comment: M = 50 gives at least 15 digits.
    let y2 = y * y;
    let c0 = 0.000_196_039_994_668_798_46;
    let c2 = 3.842_665_920_511_437_7e-08;
    let c4 = 1.004_159_283_949_764_4e-11;
    let c6 = 2.951_674_376_350_019_1e-15;
    let p = c0 + y2 * (-c2 + y2 * (c4 - y2 * c6));
    let mut sum = 0.0;
    for n in 1..=50u32 {
        let nf = n as f64;
        sum += 1.0 / (nf * (nf * nf + y * y));
    }
    -EULER + y2 * (sum + p)
}

/// `psi(1, n)`, the trigamma function at a positive integer.
///
/// # Domain and range
///
/// `n >= 1`; `NaN` at `n = 0`. Past `n = 100` an Abramowitz & Stegun 6.4.12
/// asymptotic series takes over.
///
/// # Accuracy
///
/// 6.687e-16 relative against [`psi_1`] over `n` in `[1, 400]`, measured in
/// this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::psi_1_int;
/// // psi(1, 1) = pi^2 / 6.
/// let pi = core::f64::consts::PI;
/// assert!((psi_1_int(1) - pi * pi / 6.0).abs() < 1e-15);
/// ```
pub fn psi_1_int(n: u32) -> f64 {
    if n == 0 {
        return f64::NAN;
    }
    if n <= PSI_1_TABLE_NMAX {
        return PSI_1_TABLE.get(n as usize).copied().unwrap_or(f64::NAN);
    }
    // Abramowitz & Stegun 6.4.12.
    let nf = n as f64;
    let c0 = -1.0 / 30.0;
    let c1 = 1.0 / 42.0;
    let c2 = -1.0 / 30.0;
    let ni2 = (1.0 / nf) * (1.0 / nf);
    let ser = ni2 * ni2 * (c0 + ni2 * (c1 + c2 * ni2));
    (1.0 + 0.5 / nf + 1.0 / (6.0 * nf * nf) + ser) / nf
}

/// The trigamma function `psi(1, x) = d^2/dx^2 ln Gamma(x)`.
///
/// # Domain and range
///
/// Every `x` except `0`, `-1`, `-2` (`NaN`), and the negative integers
/// generally, where the reflection formula's `1/sin^2` diverges. Strictly
/// positive and decreasing for `x > 0`.
///
/// # Method
///
/// For `x > 0`, `psi(1, x) = zeta(2, x)` — a Hurwitz zeta, which is why this
/// module depends on [`crate::specfunc::zeta`]. For `-5 < x < 0` the
/// recurrence A&S 6.4.6 shifts into that range; below `-5` the reflection
/// formula A&S 6.4.7.
///
/// # Accuracy
///
/// 1.702e-14 in the recurrence `psi_1(x + 1) - psi_1(x) = -1/x^2` over `x`
/// in `(0, 30]`, and 2.471e-14 in the reflection formula
/// `psi_1(x) + psi_1(1 - x) = pi^2 / sin^2(pi x)` over `x` in `[-7, 0)` —
/// the latter being the only check that reaches the two negative branches.
/// Both are limited by cancellation in the identity itself rather than by
/// `psi_1`. Measured in this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::psi_1;
/// // psi(1, 1) = pi^2/6, and the recurrence psi_1(x+1) = psi_1(x) - 1/x^2.
/// let pi = core::f64::consts::PI;
/// assert!((psi_1(1.0) - pi * pi / 6.0).abs() < 1e-15);
/// assert!((psi_1(2.0) - (psi_1(1.0) - 1.0)).abs() < 1e-15);
/// ```
pub fn psi_1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 || x == -1.0 || x == -2.0 {
        return f64::NAN;
    }
    if x > 0.0 {
        return psi_n_xg0(1, x);
    }
    if x > -5.0 {
        // Abramowitz & Stegun 6.4.6: shift up into x > 0.
        let m = -x.floor();
        let fx = x + m;
        if fx == 0.0 {
            return f64::NAN;
        }
        let mut sum = 0.0;
        let mi = m as u32;
        for k in 0..mi {
            let t = x + k as f64;
            sum += 1.0 / (t * t);
        }
        return psi_n_xg0(1, fx) + sum;
    }
    // Abramowitz & Stegun 6.4.7.
    let sin_px = (PI * x).sin();
    let d = PI * PI / (sin_px * sin_px);
    d - psi_n_xg0(1, 1.0 - x)
}

/// The polygamma function `psi(n, x) = d^n/dx^n psi(x)`.
///
/// # Domain and range
///
/// `n = 0` and `n = 1` forward to [`psi`] and [`psi_1`] and inherit their
/// domains. For `n >= 2` the argument must satisfy `x > 0`; otherwise `NaN`.
///
/// # Method
///
/// Abramowitz & Stegun 6.4.10, `psi(n, x) = (-1)^{n+1} n! zeta(n+1, x)`, with
/// the factorial taken in log space and recombined by `exp_mult` so that
/// `n!` overflowing `f64` (at `n = 171`) does not by itself lose the answer.
///
/// # Accuracy
///
/// 3.812e-13 relative against the defining sum
/// `(-1)^{n+1} n! sum_{k>=0} (x + k)^{-(n+1)}` for `n` in `[2, 6]` and `x` in
/// `[0.5, 20]`, measured in this module's tests. The worst point is
/// `(n, x) = (2, 20)`, where the reference's own truncated tail dominates —
/// this is a bound on the comparison, not on `psi_n`.
///
/// # Example
///
/// ```
/// use petir::specfunc::{psi_1, psi_n};
/// assert_eq!(psi_n(1, 2.5), psi_1(2.5));
/// // psi(2, x) is negative for x > 0.
/// assert!(psi_n(2, 1.0) < 0.0);
/// ```
pub fn psi_n(n: u32, x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if n == 0 {
        return psi(x);
    }
    if n == 1 {
        return psi_1(x);
    }
    if x <= 0.0 {
        return f64::NAN;
    }
    let ln_nf = ln_factorial(n);
    let hz = hzeta(n as f64 + 1.0, x);
    let v = exp_mult(ln_nf, hz);
    if n % 2 == 0 {
        -v
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sweep over `(0, 30]` avoiding nothing — `psi` has no zeros there
    /// beyond `x = 1.4616...`, which the tests that need it exclude.
    fn positive_sweep() -> impl Iterator<Item = f64> {
        (1..=300).map(|k| 0.1 * k as f64)
    }

    /// `psi(x + 1) - psi(x) = 1/x` exactly, for every `x`. The two sides are
    /// computed from different branches whenever `x` crosses 1 or 2, so this
    /// exercises the joins as well as the series.
    #[test]
    fn psi_satisfies_its_recurrence() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for x in positive_sweep() {
            let lhs = psi(x + 1.0) - psi(x);
            let e = ((lhs - 1.0 / x) * x).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        // Measured 2.057e-14 at x = 24.5, and it is the TEST that loses the
        // digits: psi(25.5) and psi(24.5) are both about 3.2 and their
        // difference is 0.04, so forming it discards two decimal digits
        // before the comparison starts. `psi` itself is pinned far tighter by
        // the closed forms and the harmonic-number identity.
        assert!(worst < 1e-13, "psi recurrence: {worst:e} at x = {at}");
    }

    /// `psi(1 - x) - psi(x) = pi cot(pi x)`, the reflection formula. This is
    /// the only test that reaches the negative-argument branch of `psi_x`.
    #[test]
    fn psi_satisfies_the_reflection_formula() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 1..=200 {
            let x = 0.005 * k as f64;
            // Skip the neighbourhood of the integers, where cot has poles,
            // AND of the half-integers, where it has zeros. The second was
            // missing first and reported a relative error of exactly 1.0 at
            // x = 0.5: cot(pi/2) is 6.1e-17 rather than 0, the left-hand side
            // is exactly 0, and dividing one by the other is not a
            // measurement of anything.
            if (x - x.round()).abs() < 0.01 || (x - 0.5 - (x - 0.5).round()).abs() < 0.01 {
                continue;
            }
            let rhs = PI * (PI * x).cos() / (PI * x).sin();
            let lhs = psi(1.0 - x) - psi(x);
            let e = ((lhs - rhs) / rhs).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-13,
            "psi reflection formula: {worst:e} at x = {at}"
        );
    }

    /// `psi(2x) = (psi(x) + psi(x + 1/2))/2 + ln 2`, Legendre's duplication
    /// formula. It couples arguments across all four of `psi_x`'s branches at
    /// once.
    #[test]
    fn psi_satisfies_the_duplication_formula() {
        let mut worst = 0.0_f64;
        for k in 1..=200 {
            let x = 0.05 * k as f64;
            let lhs = psi(2.0 * x);
            let rhs = 0.5 * (psi(x) + psi(x + 0.5)) + core::f64::consts::LN_2;
            if lhs.abs() > 0.05 {
                worst = worst.max(((lhs - rhs) / lhs).abs());
            }
        }
        assert!(worst < 1e-13, "psi duplication formula: {worst:e}");
    }

    /// The closed forms: `psi(1) = -gamma` and
    /// `psi(1/2) = -gamma - 2 ln 2`, neither of which is a tabulated value.
    #[test]
    fn psi_matches_the_closed_forms() {
        assert!((psi(1.0) + EULER).abs() < 1e-15, "psi(1) = {}", psi(1.0));
        let half = -EULER - 2.0 * core::f64::consts::LN_2;
        assert!((psi(0.5) - half).abs() < 1e-15, "psi(0.5) = {}", psi(0.5));
    }

    /// `psi_int(n)` and `psi(n as f64)` are independent routes — a table
    /// below `n = 100` and an asymptotic series above it, against the
    /// Chebyshev branch.
    #[test]
    fn the_psi_integer_table_agrees_with_the_continuous_function() {
        let mut worst = 0.0_f64;
        let mut at = 0u32;
        for n in 1..=400u32 {
            let r = psi(n as f64);
            if r == 0.0 {
                continue;
            }
            let e = ((psi_int(n) - r) / r).abs();
            if e > worst {
                worst = e;
                at = n;
            }
        }
        assert!(worst < 5e-16, "psi_int vs psi: {worst:e} at n = {at}");
    }

    /// `psi_int` also satisfies the harmonic-number identity
    /// `psi(n) = -gamma + sum_{k=1}^{n-1} 1/k`, which is independent of both
    /// the table and the series.
    #[test]
    fn psi_int_matches_the_harmonic_numbers() {
        let mut h = 0.0_f64;
        let mut worst = 0.0_f64;
        for n in 1..=300u32 {
            let expect = -EULER + h;
            if expect.abs() > 0.1 {
                worst = worst.max(((psi_int(n) - expect) / expect).abs());
            }
            h += 1.0 / n as f64;
        }
        assert!(worst < 1e-14, "psi_int vs harmonic numbers: {worst:e}");
    }

    /// `psi_1(x + 1) - psi_1(x) = -1/x^2`, the trigamma recurrence.
    #[test]
    fn psi_1_satisfies_its_recurrence() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for x in positive_sweep() {
            let lhs = psi_1(x + 1.0) - psi_1(x);
            let rhs = -1.0 / (x * x);
            let e = ((lhs - rhs) / rhs).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        // Measured 1.702e-14 at x = 29.7, reference-limited for the same
        // reason as `psi_satisfies_its_recurrence`.
        assert!(worst < 1e-13, "psi_1 recurrence: {worst:e} at x = {at}");
    }

    /// `psi_1(x) + psi_1(1 - x) = pi^2 / sin^2(pi x)`, the trigamma
    /// reflection formula. This is what reaches the two negative branches of
    /// [`psi_1`] — the shift for `-5 < x < 0` and the reflection below it.
    #[test]
    fn psi_1_satisfies_the_reflection_formula() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 1..=700 {
            let x = -0.01 * k as f64;
            if (x - x.round()).abs() < 0.02 {
                continue;
            }
            let s = (PI * x).sin();
            let rhs = PI * PI / (s * s);
            let lhs = psi_1(x) + psi_1(1.0 - x);
            let e = ((lhs - rhs) / rhs).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-12,
            "psi_1 reflection formula: {worst:e} at x = {at}"
        );
    }

    /// `psi_1(1) = pi^2/6` and `psi_1(1/2) = pi^2/2`, closed forms that are
    /// not tabulated values of the general routine.
    #[test]
    fn psi_1_matches_the_closed_forms() {
        assert!((psi_1(1.0) - PI * PI / 6.0).abs() < 1e-15);
        assert!((psi_1(0.5) - PI * PI / 2.0).abs() < 1e-14);
    }

    /// `psi_1_int(n)` against [`psi_1`], and against the tail of the Basel
    /// series `psi_1(n) = sum_{k>=n} 1/k^2`.
    #[test]
    fn the_psi_1_integer_table_agrees_with_the_continuous_function() {
        let mut worst = 0.0_f64;
        let mut at = 0u32;
        for n in 1..=400u32 {
            let r = psi_1(n as f64);
            let e = ((psi_1_int(n) - r) / r).abs();
            if e > worst {
                worst = e;
                at = n;
            }
        }
        // Measured 6.687e-16 at n = 386 -- above the table, where both sides
        // are asymptotic series in 1/n but not the same one.
        assert!(worst < 3e-15, "psi_1_int vs psi_1: {worst:e} at n = {at}");

        // psi(1, n) = sum_{k >= n} 1/k^2, summed smallest-first.
        let mut worst_sum = 0.0_f64;
        for n in 1..=20u32 {
            let mut acc = 0.0_f64;
            for k in (n..200_000u32).rev() {
                acc += 1.0 / (k as f64 * k as f64);
            }
            // The neglected tail beyond 200000 is about 5e-6, so this only
            // carries five digits; it is a sanity check on the table's
            // meaning, not on its precision.
            worst_sum = worst_sum.max(((psi_1_int(n) - acc) / acc).abs());
        }
        assert!(
            worst_sum < 1e-4,
            "psi_1_int vs the Basel tail: {worst_sum:e}"
        );
    }

    /// `psi_n(n, x) = (-1)^{n+1} n! sum_{k>=0} (x + k)^{-(n+1)}` — the
    /// defining sum, computed directly. Independent of [`hzeta`]'s
    /// Euler-Maclaurin machinery.
    #[test]
    fn psi_n_matches_its_defining_sum() {
        let mut worst = 0.0_f64;
        let mut at = (0u32, 0.0_f64);
        for n in 2..=6u32 {
            // n! by repeated multiplication; n <= 6 so this is exact.
            let mut nf = 1.0_f64;
            for k in 2..=n {
                nf *= k as f64;
            }
            for j in 1..=40 {
                let x = 0.5 * j as f64;
                let mut acc = 0.0_f64;
                let kk = 100_000u32;
                for k in (0..kk).rev() {
                    acc += (x + k as f64).powf(-(n as f64 + 1.0));
                }
                // sum_{k>=K} (x+k)^{-(n+1)} ~ (x+K)^{-n}/n. Omitting this
                // left the reference 3.804e-08 short at (n, x) = (2, 20);
                // with it the comparison reaches 3.812e-13.
                acc += (x + kk as f64).powf(-(n as f64)) / n as f64;
                let sign = if n % 2 == 0 { -1.0 } else { 1.0 };
                let r = sign * nf * acc;
                let e = ((psi_n(n, x) - r) / r).abs();
                if e > worst {
                    worst = e;
                    at = (n, x);
                }
            }
        }
        // Measured 3.812e-13, which is the reference's remaining tail error
        // rather than `psi_n`'s.
        assert!(
            worst < 2e-12,
            "psi_n vs its defining sum: {worst:e} at {at:?}"
        );
    }

    /// `psi_n` forwards `n = 0` and `n = 1` exactly, bit for bit — not
    /// approximately, since they are the same call.
    #[test]
    fn psi_n_forwards_the_low_orders_exactly() {
        for x in positive_sweep() {
            assert_eq!(psi_n(0, x).to_bits(), psi(x).to_bits(), "psi_n(0, {x})");
            assert_eq!(psi_n(1, x).to_bits(), psi_1(x).to_bits(), "psi_n(1, {x})");
        }
    }

    /// `psi_1piy(y)` against the defining series
    /// `Re psi(1 + iy) = -gamma + sum_{n>=1} y^2 / (n (n^2 + y^2))`, summed
    /// far enough to converge. The implementation uses four branches and only
    /// the innermost resembles this sum.
    #[test]
    fn psi_1piy_matches_its_defining_series() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 1..=100 {
            let y = 0.05 * k as f64;
            let y2 = y * y;
            let mut acc = 0.0_f64;
            let big = 2_000_000u32;
            for n in (1..big).rev() {
                let nf = n as f64;
                acc += y2 / (nf * (nf * nf + y2));
            }
            // The neglected tail is sum_{n>=N} y^2/(n(n^2+y^2)) ~ y^2/(2N^2).
            // Without it the reference is short by 1.25e-13 * y^2, which
            // showed up as a 2.074e-12 "error" that was entirely its own.
            let tail = y2 / (2.0 * big as f64 * big as f64);
            let r = -EULER + acc + tail;
            if r.abs() > 0.05 {
                let e = ((psi_1piy(y) - r) / r).abs();
                if e > worst {
                    worst = e;
                    at = y;
                }
            }
        }
        assert!(
            worst < 1e-12,
            "psi_1piy vs its series: {worst:e} at y = {at}"
        );
    }

    /// `psi_1piy` is even in `y`, and its branches agree **at the same
    /// argument** — the one measurement that is actually about the branches.
    ///
    /// # The obvious version of this test measures the slope, not a jump
    ///
    /// Evaluating just below and just above a cut compares two different
    /// arguments. At `|y| = 1` with a gap of 2e-09 that is a difference of
    /// 1.6e-09, which looks like a branch mismatch and is `psi_1piy` simply
    /// having a nonzero derivative. Both formulas are evaluated at `y = 1`
    /// here instead.
    ///
    /// # What that shows: GSL's own FIXME is real
    ///
    /// Against a 2e7-term reference with an analytic tail, at `y = 1` the
    /// series branch is accurate to **2.776e-17** and the Chebyshev branch to
    /// **1.258e-13** — four orders worse. Upstream flags this exact branch
    /// with `result->err *= 5.0; /* FIXME: losing a digit somewhere... */`,
    /// and that is what the digits say. The port reproduces it rather than
    /// quietly improving it, because the numbers here are GSL's.
    #[test]
    fn psi_1piy_is_even_and_its_branches_agree_at_the_cut() {
        for y in [0.3, 1.0, 5.0, 10.0, 100.0, 1000.0, 5000.0] {
            assert_eq!(psi_1piy(y), psi_1piy(-y), "psi_1piy parity at {y}");
        }

        // The two formulas that meet at |y| = 1, both at y = 1.
        let y = 1.0_f64;
        let y2 = y * y;
        let series = {
            let c0 = 0.000_196_039_994_668_798_46;
            let c2 = 3.842_665_920_511_437_7e-08;
            let c4 = 1.004_159_283_949_764_4e-11;
            let c6 = 2.951_674_376_350_019_1e-15;
            let p = c0 + y2 * (-c2 + y2 * (c4 - y2 * c6));
            let mut sum = 0.0_f64;
            for n in 1..=50u32 {
                let nf = n as f64;
                sum += 1.0 / (nf * (nf * nf + y2));
            }
            -EULER + y2 * (sum + p)
        };
        let chebyshev = {
            let t = (2.0 * y - 11.0) / 9.0;
            let v = y2 * (1.0 / (1.0 + y2) + 0.5 / (4.0 + y2));
            eval_gsl(t, &R1PY) - EULER + v
        };
        let gap = (series - chebyshev).abs();
        assert!(
            gap < 1e-12,
            "the two psi_1piy branches disagree by {gap:e} at y = 1"
        );
        assert!(
            gap > 1e-15,
            "the Chebyshev branch is documented as 1.258e-13 from the series \
             branch at y = 1, matching upstream's own FIXME; measured {gap:e}. \
             Re-measure and rewrite the docs rather than deleting this."
        );

        // And it reduces to psi(1) on the real axis.
        assert!((psi_1piy(0.0) - psi(1.0)).abs() < 1e-15);
    }

    /// The documented domain errors, and `NaN` propagation through every
    /// entry point.
    #[test]
    fn the_domain_errors_are_what_the_docs_say() {
        for x in [0.0, -1.0, -2.0] {
            assert!(psi(x).is_nan(), "psi({x})");
            assert!(psi_1(x).is_nan(), "psi_1({x})");
        }
        assert!(psi_int(0).is_nan());
        assert!(psi_1_int(0).is_nan());
        // psi_n with n >= 2 needs x > 0.
        assert!(psi_n(2, 0.0).is_nan());
        assert!(psi_n(3, -1.5).is_nan());
        let n = f64::NAN;
        assert!(psi(n).is_nan());
        assert!(psi_1(n).is_nan());
        assert!(psi_n(4, n).is_nan());
        assert!(psi_1piy(n).is_nan());
    }

    /// The `x = 0, -1, -2` pole check is GSL's, and the two entry points do
    /// **not** agree about `x = -3`.
    ///
    /// [`psi`] returns a large finite number there (-8.551e+15): its
    /// reflection term is `-pi cot(pi x)`, and `sin(-3 pi)` is a small
    /// non-zero `f64` rather than exactly zero, so the guard that rejects
    /// `|sin| < 2 * SQRT_DBL_MIN` does not fire. [`psi_1`] returns `NaN`,
    /// because its `-5 < x < 0` branch shifts by `m = -floor(x)` and then
    /// tests `x + m == 0.0`, which at an exact negative integer is true.
    ///
    /// Neither is wrong so much as differently defensive, and both are
    /// upstream's. Recorded here so it is a known property rather than a
    /// surprise.
    #[test]
    fn psi_and_psi_1_disagree_about_the_pole_at_minus_three() {
        assert!(psi(-3.0).is_finite(), "psi(-3) = {}", psi(-3.0));
        assert!(
            psi(-3.0).abs() > 1e14,
            "psi(-3) is documented as large and finite; got {}",
            psi(-3.0)
        );
        assert!(
            psi_1(-3.0).is_nan(),
            "psi_1(-3) is documented as NaN; got {}",
            psi_1(-3.0)
        );
    }
}
