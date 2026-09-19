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
// PORTED from the GNU Scientific Library (GSL) 2.8
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
// Copyright (C) 2010 Brian Gough  (the K0/K1 polynomial branches)
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Upstream files read and translated, all under specfunc/:
//   bessel_J0.c  bessel_J1.c  bessel_Y0.c  bessel_Y1.c
//   bessel_I0.c  bessel_I1.c  bessel_K0.c  bessel_K1.c
//   bessel_amp_phase.c (the four asymptotic amplitude/phase tables)
//   bessel.c           (gsl_sf_bessel_cos_pi4_e / gsl_sf_bessel_sin_pi4_e)
//   cheb_eval.c        (the Clenshaw body, already in crate::cheb_slice)
//   exp.c              (gsl_sf_exp_mult_err_e, value part only)
//
// The Chebyshev tables in this file were extracted from those sources by a
// script rather than retyped, and every literal was checked to round-trip
// through `f64` before being written out. `tests/bessel_tables_audit.rs`
// re-derives them from the vendored C at test time, so a mistyped digit
// cannot survive.

//! Cylindrical Bessel functions of orders 0 and 1: [`bessel_j0`],
//! [`bessel_j1`], [`bessel_y0`], [`bessel_y1`], [`bessel_i0`], [`bessel_i1`],
//! [`bessel_k0`], [`bessel_k1`], and the exponentially scaled forms of the
//! modified functions.
//!
//! # What these are
//!
//! `J_n` and `Y_n` are the two independent solutions of Bessel's equation
//!
//! ```text
//!     x^2 y'' + x y' + (x^2 - n^2) y = 0
//! ```
//!
//! — oscillatory, decaying as `x^{-1/2}`. `I_n` and `K_n` solve the modified
//! equation (`x^2 - n^2` becomes `-(x^2 + n^2)`) and are monotone: `I_n` grows
//! like `e^x / sqrt(2 pi x)`, `K_n` decays like `sqrt(pi / 2x) e^{-x}`.
//!
//! They are the radial eigenfunctions of the Laplacian in cylindrical
//! coordinates, which is why they turn up throughout this workspace: the
//! fundamental mode of a bare cylindrical reactor is `J_0(2.405 r / R)`, and
//! `I_0`/`K_0` are the radial temperature profiles in an annular fuel pin.
//!
//! # Argument ranges, stated plainly
//!
//! | function | domain | outside it |
//! |---|---|---|
//! | [`bessel_j0`], [`bessel_j1`] | all finite `x` | — |
//! | [`bessel_y0`], [`bessel_y1`] | `x > 0` | `NaN` |
//! | [`bessel_i0`], [`bessel_i1`] | all finite `x` | `+inf` past `x ~ 709` |
//! | [`bessel_k0`], [`bessel_k1`] | `x > 0` | `NaN` |
//!
//! All arguments and results are dimensionless (`f64`). `I_n` overflows `f64`
//! near `x = 709`; use [`bessel_i0_scaled`] / [`bessel_i1_scaled`], which
//! return `e^{-|x|} I_n(x)` and stay bounded. Symmetrically
//! [`bessel_k0_scaled`] / [`bessel_k1_scaled`] return `e^{x} K_n(x)`, which
//! avoids the underflow to zero past `x ~ 705`.
//!
//! # Accuracy
//!
//! GSL's own targets, which this translation inherits: close to machine
//! precision, with the amplitude/phase route used past `x = 4` (for `J`/`Y`)
//! and `x = 8` (for `I`/`K`) carrying the argument-reduction error of the
//! platform `sin`/`cos`.
//!
//! **What was actually measured**, over 28 points spanning every branch from
//! `x = 1e-7` to `x = 50`:
//!
//! | check | worst residual |
//! |---|---|
//! | `J_0 Y_1 - J_1 Y_0 = -2/(pi x)` | 5.297e-16 relative |
//! | `I_0 K_1 + I_1 K_0 = 1/x` | 3.469e-16 relative |
//! | `J_0`, `J_1` vs their integral representations | 2.220e-15, 2.609e-15 absolute |
//! | `I_0`, `I_1` vs their integral representations | 8.226e-15, 5.616e-15 relative |
//! | `K_0`, `K_1` vs their integral representations | 3.713e-14, 3.470e-14 relative |
//! | `Y_0` vs the Neumann series | 2.276e-15 absolute |
//! | scaled vs unscaled forms | 2.682e-16 relative |
//!
//! The two Wronskians are the tight measurement and the quadrature
//! comparisons are the loose one — the references themselves run out of `f64`
//! before the implementations do, which is why `K_0` reads two orders worse
//! than an identity that constrains it far harder. Read the first two rows as
//! the accuracy statement and the rest as independent corroboration that the
//! coefficient tables are the ones upstream has.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl;
use crate::specfunc::{
    exp_mult, DBL_EPSILON, DBL_MIN, LOG_DBL_MAX, ROOT5_DBL_EPSILON, SQRT_DBL_EPSILON,
};

/// `M_SQRT2`.
const SQRT2: f64 = core::f64::consts::SQRT_2;
/// `2 * M_SQRT2` — upstream's `ROOT_EIGHT`, the `J1`/`I1` small-argument cut.
const ROOT_EIGHT: f64 = 2.0 * SQRT2;
/// `2 / M_PI`, the factor in front of the logarithmic term of `Y_n`.
const TWO_OVER_PI: f64 = 2.0 / core::f64::consts::PI;
/// `M_LN2`.
const LN2: f64 = core::f64::consts::LN_2;

// ---------------------------------------------------------------------------
// Coefficient tables, extracted from the vendored GSL sources by script.
// Each array's length equals its `cheb_series`'s `order + 1`, so the whole
// slice is always the series -- `tests/bessel_tables_audit.rs` checks that.
// ---------------------------------------------------------------------------
/// GSL `bessel_I0.c/bi0_data` (12 values).
const BI0: [f64; 12] = [
    -0.07660547252839144951,
    1.92733795399380827000,
    0.22826445869203013390,
    0.01304891466707290428,
    0.00043442709008164874,
    0.00000942265768600193,
    0.00000014340062895106,
    0.00000000161384906966,
    0.00000000001396650044,
    0.00000000000009579451,
    0.00000000000000053339,
    0.00000000000000000245,
];

/// GSL `bessel_I0.c/ai0_data` (21 values).
const AI0: [f64; 21] = [
    0.07575994494023796,
    0.00759138081082334,
    0.00041531313389237,
    0.00001070076463439,
    -0.00000790117997921,
    -0.00000078261435014,
    0.00000027838499429,
    0.00000000825247260,
    -0.00000001204463945,
    0.00000000155964859,
    0.00000000022925563,
    -0.00000000011916228,
    0.00000000001757854,
    0.00000000000112822,
    -0.00000000000114684,
    0.00000000000027155,
    -0.00000000000002415,
    -0.00000000000000608,
    0.00000000000000314,
    -0.00000000000000071,
    0.00000000000000007,
];

/// GSL `bessel_I0.c/ai02_data` (22 values).
const AI02: [f64; 22] = [
    0.05449041101410882,
    0.00336911647825569,
    0.00006889758346918,
    0.00000289137052082,
    0.00000020489185893,
    0.00000002266668991,
    0.00000000339623203,
    0.00000000049406022,
    0.00000000001188914,
    -0.00000000003149915,
    -0.00000000001321580,
    -0.00000000000179419,
    0.00000000000071801,
    0.00000000000038529,
    0.00000000000001539,
    -0.00000000000004151,
    -0.00000000000000954,
    0.00000000000000382,
    0.00000000000000176,
    -0.00000000000000034,
    -0.00000000000000027,
    0.00000000000000003,
];

/// GSL `bessel_I1.c/bi1_data` (11 values).
const BI1: [f64; 11] = [
    -0.001971713261099859,
    0.407348876675464810,
    0.034838994299959456,
    0.001545394556300123,
    0.000041888521098377,
    0.000000764902676483,
    0.000000010042493924,
    0.000000000099322077,
    0.000000000000766380,
    0.000000000000004741,
    0.000000000000000024,
];

/// GSL `bessel_I1.c/ai1_data` (21 values).
const AI1: [f64; 21] = [
    -0.02846744181881479,
    -0.01922953231443221,
    -0.00061151858579437,
    -0.00002069971253350,
    0.00000858561914581,
    0.00000104949824671,
    -0.00000029183389184,
    -0.00000001559378146,
    0.00000001318012367,
    -0.00000000144842341,
    -0.00000000029085122,
    0.00000000012663889,
    -0.00000000001664947,
    -0.00000000000166665,
    0.00000000000124260,
    -0.00000000000027315,
    0.00000000000002023,
    0.00000000000000730,
    -0.00000000000000333,
    0.00000000000000071,
    -0.00000000000000006,
];

/// GSL `bessel_I1.c/ai12_data` (22 values).
const AI12: [f64; 22] = [
    0.02857623501828014,
    -0.00976109749136147,
    -0.00011058893876263,
    -0.00000388256480887,
    -0.00000025122362377,
    -0.00000002631468847,
    -0.00000000383538039,
    -0.00000000055897433,
    -0.00000000001897495,
    0.00000000003252602,
    0.00000000001412580,
    0.00000000000203564,
    -0.00000000000071985,
    -0.00000000000040836,
    -0.00000000000002101,
    0.00000000000004273,
    0.00000000000001041,
    -0.00000000000000382,
    -0.00000000000000186,
    0.00000000000000033,
    0.00000000000000028,
    -0.00000000000000003,
];

/// GSL `bessel_J0.c/bj0_data` (13 values).
const BJ0: [f64; 13] = [
    0.100254161968939137,
    -0.665223007764405132,
    0.248983703498281314,
    -0.0332527231700357697,
    0.0023114179304694015,
    -0.0000991127741995080,
    0.0000028916708643998,
    -0.0000000612108586630,
    0.0000000009838650793,
    -0.0000000000124235515,
    0.0000000000001265433,
    -0.0000000000000010619,
    0.0000000000000000074,
];

/// GSL `bessel_J1.c/bj1_data` (12 values).
const BJ1: [f64; 12] = [
    -0.11726141513332787,
    -0.25361521830790640,
    0.050127080984469569,
    -0.004631514809625081,
    0.000247996229415914,
    -0.000008678948686278,
    0.000000214293917143,
    -0.000000003936093079,
    0.000000000055911823,
    -0.000000000000632761,
    0.000000000000005840,
    -0.000000000000000044,
];

/// GSL `bessel_Y0.c/by0_data` (13 values).
const BY0: [f64; 13] = [
    -0.011277839392865573,
    -0.128345237560420350,
    -0.104378847997942490,
    0.023662749183969695,
    -0.002090391647700486,
    0.000103975453939057,
    -0.000003369747162423,
    0.000000077293842676,
    -0.000000001324976772,
    0.000000000017648232,
    -0.000000000000188105,
    0.000000000000001641,
    -0.000000000000000011,
];

/// GSL `bessel_Y1.c/by1_data` (14 values).
const BY1: [f64; 14] = [
    0.03208047100611908629,
    1.262707897433500450,
    0.00649996189992317500,
    -0.08936164528860504117,
    0.01325088122175709545,
    -0.00089790591196483523,
    0.00003647361487958306,
    -0.00000100137438166600,
    0.00000001994539657390,
    -0.00000000030230656018,
    0.00000000000360987815,
    -0.00000000000003487488,
    0.00000000000000027838,
    -0.00000000000000000186,
];

/// GSL `bessel_K0.c/k0_poly` (8 values).
const K0_POLY: [f64; 8] = [
    1.1593151565841244842077226e-01,
    2.7898287891460317300886539e-01,
    2.5248929932161220559969776e-02,
    8.4603509072136578707676406e-04,
    1.4914719243067801775856150e-05,
    1.6271068931224552553548933e-07,
    1.2082660336282566759313543e-09,
    6.6117104672254184399933971e-12,
];

/// GSL `bessel_K0.c/i0_poly` (7 values).
const I0_POLY: [f64; 7] = [
    1.0000000000000000044974165e+00,
    2.4999999999999822316775454e-01,
    2.7777777777892149148858521e-02,
    1.7361111083544590676709592e-03,
    6.9444476047072424198677755e-05,
    1.9288265756466775034067979e-06,
    3.9908220583262192851839992e-08,
];

/// GSL `bessel_K0.c/ak0_data` (24 values).
const AK0: [f64; 24] = [
    -3.28737867094650101e-02,
    -4.49369057710236880e-02,
    2.98149992004308095e-03,
    -3.03693649396187920e-04,
    3.91085569307646836e-05,
    -5.86872422399215952e-06,
    9.82873709937322009e-07,
    -1.78978645055651171e-07,
    3.48332306845240957e-08,
    -7.15909210462546599e-09,
    1.54019930048919494e-09,
    -3.44555485579194210e-10,
    7.97356101783753023e-11,
    -1.90090968913069735e-11,
    4.65295609304114621e-12,
    -1.16614287433470780e-12,
    2.98554375218596891e-13,
    -7.79276979512292169e-14,
    2.07027467168948402e-14,
    -5.58987860393825313e-15,
    1.53202965950646914e-15,
    -4.25737536712188186e-16,
    1.19840238501357389e-16,
    -3.41407346762502397e-17,
];

/// GSL `bessel_K0.c/ak02_data` (14 values).
const AK02: [f64; 14] = [
    -0.1201869826307592240E-1,
    -0.9174852691025695311E-2,
    0.1444550931775005821E-3,
    -0.4013614175435709729E-5,
    0.1567831810852310673E-6,
    -0.7770110438521737710E-8,
    0.4611182576179717883E-9,
    -0.3158592997860565771E-10,
    0.2435018039365041128E-11,
    -0.2074331387398347898E-12,
    0.1925787280589917085E-13,
    -0.1927554805838956104E-14,
    0.2062198029197818278E-15,
    -0.2341685117579242403E-16,
];

/// GSL `bessel_K1.c/k1_poly` (9 values).
const K1_POLY: [f64; 9] = [
    -3.0796575782920622440538935e-01,
    -8.5370719728650778045782736e-02,
    -4.6421827664715603298154971e-03,
    -1.1253607036630425931072996e-04,
    -1.5592887702110907110292728e-06,
    -1.4030163679125934402498239e-08,
    -8.8718998640336832196558868e-11,
    -4.1614323580221539328960335e-13,
    -1.5261293392975541707230366e-15,
];

/// GSL `bessel_K1.c/i1_poly` (6 values).
const I1_POLY: [f64; 6] = [
    // declared [7], 6 initialisers; upstream leaves the rest zero
    8.3333333333333325191635191e-02,
    6.9444444444467956461838830e-03,
    3.4722222211230452695165215e-04,
    1.1574075952009842696580084e-05,
    2.7555870002088181016676934e-07,
    4.9724386164128529514040614e-09,
];

/// GSL `bessel_K1.c/ak1_data` (25 values).
const AK1: [f64; 25] = [
    2.07996868001418246e-01,
    1.62581565017881476e-01,
    -5.87070423518863640e-03,
    4.95021520115789501e-04,
    -5.78958347598556986e-05,
    8.18614610209334726e-06,
    -1.31604832009487277e-06,
    2.32546031520101213e-07,
    -4.42206518311557987e-08,
    8.92163994883100361e-09,
    -1.89046270526983427e-09,
    4.17568808108504702e-10,
    -9.55912361791375794e-11,
    2.25769353153867758e-11,
    -5.48128000211158482e-12,
    1.36386122546441926e-12,
    -3.46936690565986409e-13,
    9.00354564415705942e-14,
    -2.37950577776254432e-14,
    6.39447503964025336e-15,
    -1.74498363492322044e-15,
    4.82994547989290473e-16,
    -1.35460927805445606e-16,
    3.84604274446777234e-17,
    -1.10456856122581316e-17,
];

/// GSL `bessel_K1.c/ak12_data` (14 values).
const AK12: [f64; 14] = [
    0.637930834373900104E-1,
    0.283288781304972094E-1,
    -0.247537067390525035E-3,
    0.577197245160724882E-5,
    -0.206893921953654830E-6,
    0.973998344138180418E-8,
    -0.558533614038062498E-9,
    0.373299663404618524E-10,
    -0.282505196102322545E-11,
    0.237201900248414417E-12,
    -0.217667738799175398E-13,
    0.215791416161603245E-14,
    -0.229019693071826928E-15,
    0.258288572982327496E-16,
];

/// GSL `bessel_amp_phase.c/bm0_data` (21 values).
const BM0: [f64; 21] = [
    0.09284961637381644,
    -0.00142987707403484,
    0.00002830579271257,
    -0.00000143300611424,
    0.00000012028628046,
    -0.00000001397113013,
    0.00000000204076188,
    -0.00000000035399669,
    0.00000000007024759,
    -0.00000000001554107,
    0.00000000000376226,
    -0.00000000000098282,
    0.00000000000027408,
    -0.00000000000008091,
    0.00000000000002511,
    -0.00000000000000814,
    0.00000000000000275,
    -0.00000000000000096,
    0.00000000000000034,
    -0.00000000000000012,
    0.00000000000000004,
];

/// GSL `bessel_amp_phase.c/bth0_data` (24 values).
const BTH0: [f64; 24] = [
    -0.24639163774300119,
    0.001737098307508963,
    -0.000062183633402968,
    0.000004368050165742,
    -0.000000456093019869,
    0.000000062197400101,
    -0.000000010300442889,
    0.000000001979526776,
    -0.000000000428198396,
    0.000000000102035840,
    -0.000000000026363898,
    0.000000000007297935,
    -0.000000000002144188,
    0.000000000000663693,
    -0.000000000000215126,
    0.000000000000072659,
    -0.000000000000025465,
    0.000000000000009229,
    -0.000000000000003448,
    0.000000000000001325,
    -0.000000000000000522,
    0.000000000000000210,
    -0.000000000000000087,
    0.000000000000000036,
];

/// GSL `bessel_amp_phase.c/bm1_data` (21 values).
const BM1: [f64; 21] = [
    0.1047362510931285,
    0.00442443893702345,
    -0.00005661639504035,
    0.00000231349417339,
    -0.00000017377182007,
    0.00000001893209930,
    -0.00000000265416023,
    0.00000000044740209,
    -0.00000000008691795,
    0.00000000001891492,
    -0.00000000000451884,
    0.00000000000116765,
    -0.00000000000032265,
    0.00000000000009450,
    -0.00000000000002913,
    0.00000000000000939,
    -0.00000000000000315,
    0.00000000000000109,
    -0.00000000000000039,
    0.00000000000000014,
    -0.00000000000000005,
];

/// GSL `bessel_amp_phase.c/bth1_data` (24 values).
const BTH1: [f64; 24] = [
    0.74060141026313850,
    -0.004571755659637690,
    0.000119818510964326,
    -0.000006964561891648,
    0.000000655495621447,
    -0.000000084066228945,
    0.000000013376886564,
    -0.000000002499565654,
    0.000000000529495100,
    -0.000000000124135944,
    0.000000000031656485,
    -0.000000000008668640,
    0.000000000002523758,
    -0.000000000000775085,
    0.000000000000249527,
    -0.000000000000083773,
    0.000000000000029205,
    -0.000000000000010534,
    0.000000000000003919,
    -0.000000000000001500,
    0.000000000000000589,
    -0.000000000000000237,
    0.000000000000000097,
    -0.000000000000000040,
];

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Horner evaluation of `c[0] + c[1] x + ... + c[n-1] x^{n-1}` — GSL's
/// `gsl_poly_eval` (`poly/eval.c`), over a borrowed slice.
///
/// Written as a reverse fold rather than a subscripted loop so that no
/// subscript in this file can fail at run time (see `tests/no_panic_gate.rs`).
/// The accumulation order is upstream's, so the rounding is unchanged.
#[inline]
fn poly_eval(c: &[f64], x: f64) -> f64 {
    let Some((&last, head)) = c.split_last() else {
        return 0.0;
    };
    let mut ans = last;
    for &ci in head.iter().rev() {
        ans = ci + x * ans;
    }
    ans
}

/// `sin(eps)` and `cos(eps)` for the small phase correction, by GSL's own
/// branch: a two-term series below `GSL_ROOT5_DBL_EPSILON`, the library
/// functions above it.
///
/// Returns `(sin eps, cos eps)`. `bessel.c:786-795`, shared verbatim by
/// `cos_pi4` and `sin_pi4`.
#[inline]
fn sin_cos_eps(eps: f64) -> (f64, f64) {
    if eps.abs() < ROOT5_DBL_EPSILON {
        let e2 = eps * eps;
        (
            eps * (1.0 - e2 / 6.0 * (1.0 - e2 / 20.0)),
            1.0 - e2 / 2.0 * (1.0 - e2 / 12.0),
        )
    } else {
        (eps.sin(), eps.cos())
    }
}

/// `cos(y - pi/4 + eps)`, evaluated as GSL's `gsl_sf_bessel_cos_pi4_e`
/// (`specfunc/bessel.c:778`) does — by expanding the `pi/4` shift into
/// `sin y + cos y` and `sin y - cos y` rather than adding `pi/4` to the
/// argument, which would lose the low bits of a large `y`.
#[inline]
fn cos_pi4(y: f64, eps: f64) -> f64 {
    let sy = y.sin();
    let cy = y.cos();
    let s = sy + cy;
    let d = sy - cy;
    let (seps, ceps) = sin_cos_eps(eps);
    (ceps * s - seps * d) / SQRT2
}

/// `sin(y - pi/4 + eps)` — GSL's `gsl_sf_bessel_sin_pi4_e`
/// (`specfunc/bessel.c:816`), the companion to [`cos_pi4`].
#[inline]
fn sin_pi4(y: f64, eps: f64) -> f64 {
    let sy = y.sin();
    let cy = y.cos();
    let s = sy + cy;
    let d = sy - cy;
    let (seps, ceps) = sin_cos_eps(eps);
    (ceps * d + seps * s) / SQRT2
}

// ---------------------------------------------------------------------------
// J0, J1
// ---------------------------------------------------------------------------

/// Cylindrical Bessel function of the first kind, order zero, `J_0(x)`.
///
/// # Domain and range
///
/// Defined for every finite `x`, even in `x`, with `J_0(0) = 1` and
/// `|J_0(x)| <= 1` everywhere. Its first zero is at `x = 2.404826...`, the
/// eigenvalue that sets the fundamental radial mode of a bare cylindrical
/// reactor.
///
/// # Method — GSL's, in two branches
///
/// Below `|x| = 4` a 13-term Chebyshev expansion in `x^2/8 - 1` (SLATEC
/// `besj0`). Above it the asymptotic amplitude/phase form
/// `J_0 = A(x) cos(x - pi/4 + theta(x)/x)`, with `A` and `theta` themselves
/// Chebyshev series in `32/x^2 - 1`. The phase is never formed by adding
/// `pi/4` to `x`; see [`cos_pi4`].
///
/// # Accuracy
///
/// Absolute error below 2.220e-15 against the integral representation over
/// `x` in `(0, 50]`, and 5.297e-16 relative in the `J`/`Y` Wronskian. The
/// *absolute* bound is the meaningful one: `J_0` has zeros, and the relative
/// error at one is unbounded for any implementation.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_j0;
/// assert!((bessel_j0(0.0) - 1.0).abs() < 1e-15);
/// // The first zero of J_0, the cylindrical-reactor buckling eigenvalue.
/// assert!(bessel_j0(2.404_825_557_695_773).abs() < 1e-15);
/// ```
pub fn bessel_j0(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0;
    }
    if y <= 4.0 {
        return eval_gsl(0.125 * y * y - 1.0, &BJ0);
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = eval_gsl(z, &BM0);
    let ct = eval_gsl(z, &BTH0);
    let cp = cos_pi4(y, ct / y);
    let ampl = (0.75 + ca) / y.sqrt();
    ampl * cp
}

/// Cylindrical Bessel function of the first kind, order one, `J_1(x)`.
///
/// # Domain and range
///
/// Defined for every finite `x`, odd in `x`, with `J_1(0) = 0` and
/// `J_1(x) ~ x/2` near the origin. `|J_1| <= 0.5819` everywhere.
///
/// # Method
///
/// The same two-branch structure as [`bessel_j0`]: a 12-term Chebyshev series
/// below `|x| = 4` (SLATEC `besj1`), and the amplitude/phase form above it,
/// there with `sin` rather than `cos` because `J_1`'s asymptotic phase is
/// shifted by a further `pi/2`.
///
/// # Accuracy
///
/// Absolute error below 2.609e-15 against the integral representation over
/// `x` in `(0, 50]`, on the same footing as [`bessel_j0`].
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_j1;
/// assert_eq!(bessel_j1(0.0), 0.0);
/// // J_1 is the derivative of -J_0, so it vanishes where J_0 is stationary.
/// assert!((bessel_j1(1e-8) - 0.5e-8).abs() < 1e-23);
/// ```
pub fn bessel_j1(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    if y == 0.0 {
        return 0.0;
    }
    if y < 2.0 * DBL_MIN {
        // Upstream reports an underflow here; the value it hands back is zero.
        return 0.0;
    }
    if y < ROOT_EIGHT * SQRT_DBL_EPSILON {
        return 0.5 * x;
    }
    if y < 4.0 {
        let c = eval_gsl(0.125 * y * y - 1.0, &BJ1);
        return x * (0.25 + c);
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = eval_gsl(z, &BM1);
    let ct = eval_gsl(z, &BTH1);
    let sp = sin_pi4(y, ct / y);
    let ampl = (0.75 + ca) / y.sqrt();
    (if x < 0.0 { -ampl } else { ampl }) * sp
}

// ---------------------------------------------------------------------------
// Y0, Y1
// ---------------------------------------------------------------------------

/// Cylindrical Bessel function of the second kind, order zero, `Y_0(x)`.
///
/// # Domain and range
///
/// `x > 0` only — `Y_0` has a logarithmic singularity at the origin
/// (`Y_0(x) -> -inf` as `x -> 0+`). Returns `NaN` for `x <= 0`, and `0.0`
/// past `x = 1/eps ~ 4.5e15`, where upstream reports an underflow.
///
/// # Method
///
/// Below `x = 4`, `Y_0 = (2/pi) ln(x/2) J_0(x) + 3/8 + C(x^2/8 - 1)` with `C`
/// a 13-term Chebyshev series — the logarithmic term carries the singularity
/// and the series carries the rest. Above `x = 4` the same amplitude/phase
/// tables as [`bessel_j0`], with `sin` in place of `cos`.
///
/// # Accuracy
///
/// Absolute error below 2.276e-15 against the Neumann series over `x` in
/// `[0.1, 20]`, and 5.297e-16 relative in the `J_0 Y_1 - J_1 Y_0 = -2/(pi x)`
/// Wronskian over `x` in `(0, 50]`. Absolute rather than relative for the same
/// reason as [`bessel_j0`]: `Y_0` has zeros.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_y0;
/// assert!(bessel_y0(1.0).is_finite());
/// assert!(bessel_y0(0.0).is_nan());
/// ```
pub fn bessel_y0(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let xmax = 1.0 / DBL_EPSILON;
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 4.0 {
        let j0 = bessel_j0(x);
        let c = eval_gsl(0.125 * x * x - 1.0, &BY0);
        return TWO_OVER_PI * (-LN2 + x.ln()) * j0 + 0.375 + c;
    }
    if x < xmax {
        let z = 32.0 / (x * x) - 1.0;
        let c1 = eval_gsl(z, &BM0);
        let c2 = eval_gsl(z, &BTH0);
        let sp = sin_pi4(x, c2 / x);
        let ampl = (0.75 + c1) / x.sqrt();
        return ampl * sp;
    }
    // Upstream reports an underflow here; the value it hands back is zero.
    0.0
}

/// Cylindrical Bessel function of the second kind, order one, `Y_1(x)`.
///
/// # Domain and range
///
/// `x > 0` only — `Y_1(x) ~ -2/(pi x)` as `x -> 0+`, so it diverges like
/// `1/x`. Returns `NaN` for `x <= 0`, `-inf` below `1.571 * DBL_MIN` where
/// that `1/x` overflows, and `0.0` past `x = 1/eps`.
///
/// # Method
///
/// Below `x = 4`, `Y_1 = (2/pi) ln(x/2) J_1(x) + (1/2 + C(x^2/8 - 1))/x`, the
/// explicit `1/x` carrying the pole. Above `x = 4` the amplitude/phase tables
/// of [`bessel_j1`], with `cos` and an overall sign change.
///
/// # Accuracy
///
/// 5.297e-16 relative in the `J`/`Y` Wronskian over `x` in `(0, 50]`, which
/// is the tightest constraint available on it; `Y_1` has zeros, so there is no
/// meaningful relative bound at every point.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_y1;
/// // Y_1 diverges like -2/(pi x) at the origin.
/// let x = 1e-6;
/// let leading = -2.0 / (core::f64::consts::PI * x);
/// assert!(((bessel_y1(x) - leading) / leading).abs() < 1e-10);
/// ```
pub fn bessel_y1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let xmin = 1.571 * DBL_MIN;
    let x_small = 2.0 * SQRT_DBL_EPSILON;
    let xmax = 1.0 / DBL_EPSILON;
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < xmin {
        // Upstream reports an overflow; Y_1 is large and negative here.
        return f64::NEG_INFINITY;
    }
    if x < x_small {
        let lnterm = (0.5 * x).ln();
        let j1 = bessel_j1(x);
        // Upstream evaluates the series at the left endpoint of its interval
        // rather than at 0.125 x^2 - 1, which for x this small differs from
        // -1.0 by less than an ulp but is written out explicitly there.
        let c = eval_gsl(-1.0, &BY1);
        return TWO_OVER_PI * lnterm * j1 + (0.5 + c) / x;
    }
    if x < 4.0 {
        let lnterm = (0.5 * x).ln();
        let c = eval_gsl(0.125 * x * x - 1.0, &BY1);
        let j1 = bessel_j1(x);
        return TWO_OVER_PI * lnterm * j1 + (0.5 + c) / x;
    }
    if x < xmax {
        let z = 32.0 / (x * x) - 1.0;
        let ca = eval_gsl(z, &BM1);
        let ct = eval_gsl(z, &BTH1);
        let cp = cos_pi4(x, ct / x);
        let ampl = (0.75 + ca) / x.sqrt();
        return -ampl * cp;
    }
    // Upstream reports an underflow here; the value it hands back is zero.
    0.0
}

// ---------------------------------------------------------------------------
// I0, I1 and their scaled forms
// ---------------------------------------------------------------------------

/// Exponentially scaled modified Bessel function `e^{-|x|} I_0(x)`.
///
/// # Why scaled
///
/// `I_0(709)` overflows `f64`; `e^{-709} I_0(709)` is about 0.015. Any ratio
/// or product of modified Bessel functions at large argument should be
/// assembled from the scaled forms and re-exponentiated once, if at all.
///
/// # Domain and range
///
/// Every finite `x`. Even in `x`, equal to 1 at the origin, decreasing
/// monotonically towards `1/sqrt(2 pi |x|)`.
///
/// # Method — GSL's three branches (SLATEC `besi0e`)
///
/// `|x| <= 3`: `e^{-|x|}(11/4 + C(x^2/4.5 - 1))`.
/// `3 < |x| <= 8`: `(3/8 + C((48/|x| - 11)/5)) / sqrt(|x|)`.
/// `|x| > 8`: the same shape with `C(16/|x| - 1)`.
///
/// # Accuracy
///
/// 3.469e-16 relative in the `I_0 K_1 + I_1 K_0 = 1/x` Wronskian over `x` in
/// `(0, 50]`, and 2.682e-16 relative against `e^{-x} I_0(x)` formed from the
/// unscaled function where both are representable.
pub fn bessel_i0_scaled(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0 - y;
    }
    if y <= 3.0 {
        let ey = (-y).exp();
        let c = eval_gsl(y * y / 4.5 - 1.0, &BI0);
        return ey * (2.75 + c);
    }
    if y <= 8.0 {
        let c = eval_gsl((48.0 / y - 11.0) / 5.0, &AI0);
        return (0.375 + c) / y.sqrt();
    }
    let c = eval_gsl(16.0 / y - 1.0, &AI02);
    (0.375 + c) / y.sqrt()
}

/// Modified Bessel function of the first kind, order zero, `I_0(x)`.
///
/// # Domain and range
///
/// Every finite `x`, even, with `I_0(0) = 1` and growth like
/// `e^{|x|}/sqrt(2 pi |x|)`. Returns `+inf` past `|x| ~ 709`, where the result
/// leaves `f64`; use [`bessel_i0_scaled`] there.
///
/// # Accuracy
///
/// 8.226e-15 relative against `(1/pi) int_0^pi e^{x cos t} dt` over `x` in
/// `[0.1, 20]` — a bound set by the reference, which loses significance as
/// `e^{x cos t}` spans `e^{2x}`. The `I`/`K` Wronskian constrains it to
/// 3.469e-16.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_i0;
/// assert!((bessel_i0(0.0) - 1.0).abs() < 1e-15);
/// assert!(bessel_i0(1000.0).is_infinite());
/// ```
pub fn bessel_i0(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0;
    }
    if y <= 3.0 {
        let c = eval_gsl(y * y / 4.5 - 1.0, &BI0);
        return 2.75 + c;
    }
    if y < LOG_DBL_MAX - 1.0 {
        return y.exp() * bessel_i0_scaled(x);
    }
    f64::INFINITY
}

/// Exponentially scaled modified Bessel function `e^{-|x|} I_1(x)`.
///
/// # Domain and range
///
/// Every finite `x`. Odd in `x`, zero at the origin, rising to a maximum near
/// `|x| = 1.7` and decaying towards `1/sqrt(2 pi |x|)`.
///
/// # Method
///
/// The three-branch structure of [`bessel_i0_scaled`] (SLATEC `besi1e`), with
/// the odd symmetry carried by an explicit sign on the two outer branches
/// rather than by the series.
///
/// # Accuracy
///
/// 3.469e-16 relative in the `I`/`K` Wronskian over `x` in `(0, 50]`, and
/// 2.682e-16 relative against `e^{-x} I_1(x)` where both are representable.
pub fn bessel_i1_scaled(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    let x_small = ROOT_EIGHT * SQRT_DBL_EPSILON;
    if y == 0.0 {
        return 0.0;
    }
    if y < 2.0 * DBL_MIN {
        // Upstream reports an underflow here; the value it hands back is zero.
        return 0.0;
    }
    if y < x_small {
        return 0.5 * x;
    }
    if y <= 3.0 {
        let ey = (-y).exp();
        let c = eval_gsl(y * y / 4.5 - 1.0, &BI1);
        return x * ey * (0.875 + c);
    }
    let c = if y <= 8.0 {
        eval_gsl((48.0 / y - 11.0) / 5.0, &AI1)
    } else {
        eval_gsl(16.0 / y - 1.0, &AI12)
    };
    let b = (0.375 + c) / y.sqrt();
    let s = if x > 0.0 { 1.0 } else { -1.0 };
    s * b
}

/// Modified Bessel function of the first kind, order one, `I_1(x)`.
///
/// # Domain and range
///
/// Every finite `x`, odd, with `I_1(x) ~ x/2` near the origin. Returns
/// `±inf` past `|x| ~ 709`; use [`bessel_i1_scaled`] there.
///
/// # Accuracy
///
/// 5.616e-15 relative against the integral representation over `x` in
/// `[0.1, 20]`, with the same caveat about the reference as [`bessel_i0`];
/// the `I`/`K` Wronskian constrains it to 3.469e-16.
///
/// # One deliberate divergence from upstream
///
/// GSL's overflow branch is the generic `OVERFLOW_ERROR` macro, which sets
/// `+inf` whatever the sign of the argument — so upstream reports
/// `I_1(-1000) = +inf` for a function that is odd. This translation returns
/// `-inf` there. The divergence is in the sign of an already-overflowed
/// result, affects nothing representable, and
/// `i1_overflows_with_the_sign_of_its_argument` pins it so it is not
/// mistaken later for a transcription slip.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_i1;
/// assert_eq!(bessel_i1(0.0), 0.0);
/// assert!((bessel_i1(-1.0) + bessel_i1(1.0)).abs() < 1e-15);
/// ```
pub fn bessel_i1(x: f64) -> f64 {
    let y = x.abs();
    if y.is_nan() {
        return f64::NAN;
    }
    let x_small = ROOT_EIGHT * SQRT_DBL_EPSILON;
    if y == 0.0 {
        return 0.0;
    }
    if y < 2.0 * DBL_MIN {
        // Upstream reports an underflow here; the value it hands back is zero.
        return 0.0;
    }
    if y < x_small {
        return 0.5 * x;
    }
    if y <= 3.0 {
        let c = eval_gsl(y * y / 4.5 - 1.0, &BI1);
        return x * (0.875 + c);
    }
    if y < LOG_DBL_MAX {
        return y.exp() * bessel_i1_scaled(x);
    }
    if x < 0.0 {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    }
}

// ---------------------------------------------------------------------------
// K0, K1 and their scaled forms
// ---------------------------------------------------------------------------

/// Exponentially scaled modified Bessel function `e^{x} K_0(x)`.
///
/// # Why scaled
///
/// `K_0(x)` decays like `e^{-x}` and underflows `f64` to zero past `x ~ 705`,
/// while `e^{x} K_0(x)` tends to `sqrt(pi / 2x)` and stays representable.
///
/// # Domain and range
///
/// `x > 0` only; returns `NaN` otherwise. Logarithmically divergent at the
/// origin, decreasing monotonically thereafter.
///
/// # Method
///
/// `x < 1`: `e^{x}(P(x^2) - ln(x)(1 + x^2 I(x^2/4)/4))`, with `P` and `I`
/// short power series (Brian Gough's 2010 rewrite of the SLATEC branch).
/// `1 <= x <= 8` and `x > 8`: Chebyshev expansions from Pavel Holoborodko,
/// peak relative error 1.28 eps.
///
/// # Accuracy
///
/// 3.469e-16 relative in the `I_0 K_1 + I_1 K_0 = 1/x` Wronskian over `x` in
/// `(0, 50]`, and 2.682e-16 relative against `e^{x} K_0(x)` formed from the
/// unscaled function.
pub fn bessel_k0_scaled(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 1.0 {
        let lx = x.ln();
        let ex = x.exp();
        let x2 = x * x;
        return ex
            * (poly_eval(&K0_POLY, x2) - lx * (1.0 + 0.25 * x2 * poly_eval(&I0_POLY, 0.25 * x2)));
    }
    if x <= 8.0 {
        let c = eval_gsl((16.0 / x - 9.0) / 7.0, &AK0);
        // 1.203125 = 77/64, upstream's own note.
        return (1.203125 + c) / x.sqrt();
    }
    let c = eval_gsl(16.0 / x - 1.0, &AK02);
    (1.25 + c) / x.sqrt()
}

/// Modified Bessel function of the second kind, order zero, `K_0(x)`.
///
/// # Domain and range
///
/// `x > 0` only; returns `NaN` otherwise. Diverges logarithmically at the
/// origin and underflows to zero past `x ~ 705` — use [`bessel_k0_scaled`]
/// there.
///
/// # Accuracy
///
/// 3.713e-14 relative against `int_0^inf e^{-x cosh t} dt` over `x` in
/// `[0.01, 30]` — again a bound on the reference rather than on `K_0`, which
/// the `I`/`K` Wronskian pins to 3.469e-16.
///
/// # Example
///
/// ```
/// use petir::specfunc::{bessel_i0, bessel_i1, bessel_k0, bessel_k1};
/// // The Wronskian identity I_0(x) K_1(x) + I_1(x) K_0(x) = 1/x.
/// let x = 2.5;
/// let w = bessel_i0(x) * bessel_k1(x) + bessel_i1(x) * bessel_k0(x);
/// assert!((w - 1.0 / x).abs() < 1e-15);
/// ```
pub fn bessel_k0(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 1.0 {
        let lx = x.ln();
        let x2 = x * x;
        return poly_eval(&K0_POLY, x2) - lx * (1.0 + 0.25 * x2 * poly_eval(&I0_POLY, 0.25 * x2));
    }
    exp_mult(-x, bessel_k0_scaled(x))
}

/// Exponentially scaled modified Bessel function `e^{x} K_1(x)`.
///
/// # Domain and range
///
/// `x > 0` only; returns `NaN` otherwise, and `+inf` below `2 * DBL_MIN` where
/// the `1/x` pole overflows.
///
/// # Method
///
/// The branch structure of [`bessel_k0_scaled`], with the `x < 1` branch
/// carrying an explicit `1/x` for the pole and an inline `I_1` series.
///
/// # Accuracy
///
/// 3.469e-16 relative in the `I`/`K` Wronskian over `x` in `(0, 50]`, and
/// 2.682e-16 relative against `e^{x} K_1(x)` where both are representable.
pub fn bessel_k1_scaled(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 2.0 * DBL_MIN {
        return f64::INFINITY;
    }
    if x < 1.0 {
        let lx = x.ln();
        let ex = x.exp();
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1 = 0.5 * x * (1.0 + t * (0.5 + t * poly_eval(&I1_POLY, t)));
        return ex * (x2 * poly_eval(&K1_POLY, x2) + x * lx * i1 + 1.0) / x;
    }
    if x <= 8.0 {
        let c = eval_gsl((16.0 / x - 9.0) / 7.0, &AK1);
        // 1.375 = 11/8, upstream's own note.
        return (1.375 + c) / x.sqrt();
    }
    let c = eval_gsl(16.0 / x - 1.0, &AK12);
    (1.25 + c) / x.sqrt()
}

/// Modified Bessel function of the second kind, order one, `K_1(x)`.
///
/// # Domain and range
///
/// `x > 0` only; returns `NaN` otherwise, `+inf` below `2 * DBL_MIN`. Diverges
/// like `1/x` at the origin and underflows to zero past `x ~ 705` — use
/// [`bessel_k1_scaled`] there.
///
/// # Accuracy
///
/// 3.470e-14 relative against `int_0^inf e^{-x cosh t} cosh t dt` over `x` in
/// `[0.01, 30]`, with the same caveat as [`bessel_k0`]; the `I`/`K` Wronskian
/// pins it to 3.469e-16.
///
/// # Example
///
/// ```
/// use petir::specfunc::bessel_k1;
/// // K_1 diverges like 1/x at the origin.
/// let x = 1e-6;
/// assert!((bessel_k1(x) * x - 1.0).abs() < 1e-10);
/// ```
pub fn bessel_k1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 2.0 * DBL_MIN {
        return f64::INFINITY;
    }
    if x < 1.0 {
        let lx = x.ln();
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1 = 0.5 * x * (1.0 + t * (0.5 + t * poly_eval(&I1_POLY, t)));
        return (x2 * poly_eval(&K1_POLY, x2) + x * lx * i1 + 1.0) / x;
    }
    exp_mult(-x, bessel_k1_scaled(x))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    /// Euler-Mascheroni constant, used only by the `Y_0` integral
    /// representation in these tests.
    const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

    /// `J_0(x) = (1/pi) int_0^pi cos(x sin t) dt`, by the trapezoid rule.
    ///
    /// The integrand is smooth and periodic on `[0, 2pi]`, so the trapezoid
    /// rule converges geometrically — with 4096 points it is at the `f64`
    /// floor, and it shares no coefficient, branch or table with the
    /// implementation under test.
    fn j0_by_quadrature(x: f64) -> f64 {
        let n = 4096;
        let mut s = 0.0;
        for k in 0..n {
            let t = PI * (k as f64 + 0.5) / n as f64;
            s += (x * t.sin()).cos();
        }
        s / n as f64
    }

    /// `J_1(x) = (1/pi) int_0^pi cos(t - x sin t) dt`, same quadrature.
    fn j1_by_quadrature(x: f64) -> f64 {
        let n = 4096;
        let mut s = 0.0;
        for k in 0..n {
            let t = PI * (k as f64 + 0.5) / n as f64;
            s += (t - x * t.sin()).cos();
        }
        s / n as f64
    }

    /// `I_n(x) = (1/pi) int_0^pi exp(x cos t) cos(n t) dt`, same quadrature.
    fn i_by_quadrature(n_order: u32, x: f64) -> f64 {
        let n = 4096;
        let mut s = 0.0;
        for k in 0..n {
            let t = PI * (k as f64 + 0.5) / n as f64;
            s += (x * t.cos()).exp() * (n_order as f64 * t).cos();
        }
        s / n as f64
    }

    /// `K_n(x) = int_0^inf exp(-x cosh t) cosh(n t) dt`, trapezoid in `t`.
    ///
    /// The integrand decays doubly exponentially once `cosh t` exceeds `1/x`,
    /// so a fine trapezoid on a generous interval is at the `f64` floor.
    fn k_by_quadrature(n_order: u32, x: f64) -> f64 {
        let n = 400_000;
        let tmax = 40.0;
        let h = tmax / n as f64;
        let mut s = 0.0;
        for k in 0..n {
            let t = h * (k as f64 + 0.5);
            let arg = -x * t.cosh();
            if arg < -745.0 {
                break;
            }
            s += arg.exp() * (n_order as f64 * t).cosh();
        }
        s * h
    }

    /// `Y_0(x) = (2/pi)(ln(x/2) + gamma) J_0(x)
    ///           + (4/pi) sum_{k>=1} (-1)^{k+1} J_{2k}(x) / k`,
    /// with the `J_{2k}` themselves taken from the quadrature above.
    ///
    /// Independent of everything in the implementation except `J_0`, which is
    /// itself checked against quadrature in the same suite.
    fn y0_by_series(x: f64) -> f64 {
        fn jn_by_quadrature(order: u32, x: f64) -> f64 {
            let n = 4096;
            let mut s = 0.0;
            for k in 0..n {
                let t = PI * (k as f64 + 0.5) / n as f64;
                s += (order as f64 * t - x * t.sin()).cos();
            }
            s / n as f64
        }
        let mut acc = 0.0;
        for k in 1..40u32 {
            let term = jn_by_quadrature(2 * k, x) / k as f64;
            acc += if k % 2 == 1 { term } else { -term };
        }
        2.0 / PI * ((0.5 * x).ln() + EULER_GAMMA) * j0_by_quadrature(x) + 4.0 / PI * acc
    }

    /// A geometric sweep of test points spanning every branch cut in the file:
    /// the small-argument cuts, the `3`/`4`/`8` Chebyshev boundaries, and the
    /// asymptotic tails.
    fn sweep() -> impl Iterator<Item = f64> {
        [
            1e-7, 1e-4, 0.01, 0.1, 0.5, 0.9, 0.999, 1.0, 1.001, 1.5, 2.0, 2.9, 3.0, 3.1, 3.9, 4.0,
            4.1, 5.0, 6.0, 7.9, 8.0, 8.1, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0,
        ]
        .into_iter()
    }

    /// The `J` and `Y` pair satisfies `J_0 Y_1 - J_1 Y_0 = -2/(pi x)` exactly.
    /// Nothing in the implementation enforces this: `J` and `Y` come from
    /// different Chebyshev tables below `x = 4`, and above it from different
    /// trigonometric combinations of the same amplitude and phase. A mistyped
    /// coefficient in any of the six tables breaks it.
    #[test]
    fn the_j_y_wronskian_holds() {
        let mut worst = 0.0_f64;
        for x in sweep() {
            let w = bessel_j0(x) * bessel_y1(x) - bessel_j1(x) * bessel_y0(x);
            let exact = -2.0 / (PI * x);
            worst = worst.max(((w - exact) / exact).abs());
        }
        // Measured 5.297e-16; the bound is a few times that so an ulp of
        // platform `sin`/`cos` drift does not turn this red.
        assert!(
            worst < 2e-15,
            "J/Y Wronskian relative error {worst:e} exceeds 2e-15"
        );
    }

    /// The `I` and `K` pair satisfies `I_0 K_1 + I_1 K_0 = 1/x` exactly, and
    /// the same argument applies: four independent branch structures.
    #[test]
    fn the_i_k_wronskian_holds() {
        let mut worst = 0.0_f64;
        for x in sweep() {
            let w = bessel_i0(x) * bessel_k1(x) + bessel_i1(x) * bessel_k0(x);
            let exact = 1.0 / x;
            worst = worst.max(((w - exact) / exact).abs());
        }
        // Measured 3.469e-16.
        assert!(
            worst < 2e-15,
            "I/K Wronskian relative error {worst:e} exceeds 2e-15"
        );
    }

    /// `J_0` and `J_1` against their integral representations.
    ///
    /// Bounded in *absolute* terms rather than relative: `J_n` has zeros, and
    /// the quadrature reference has its own `f64` floor of about 1e-16, so a
    /// relative bound near a zero would be testing the reference rather than
    /// the implementation.
    #[test]
    fn j0_and_j1_match_their_integral_representations() {
        let (mut w0, mut w1) = (0.0_f64, 0.0_f64);
        for x in sweep() {
            w0 = w0.max((bessel_j0(x) - j0_by_quadrature(x)).abs());
            w1 = w1.max((bessel_j1(x) - j1_by_quadrature(x)).abs());
        }
        assert!(w0 < 1e-14, "J_0 absolute error {w0:e}");
        assert!(w1 < 1e-14, "J_1 absolute error {w1:e}");
    }

    /// `I_0` and `I_1` against their integral representations, relatively —
    /// neither has a zero on the positive axis, so a relative bound is
    /// meaningful throughout that window.
    ///
    /// # Why the window is `[0.01, 20]` and not the whole sweep
    ///
    /// Both ends are limits of the *reference*, not of the implementation,
    /// and both were found by measurement rather than assumed. Above `x = 20`
    /// the `exp(x cos t)` integrand spans `e^{40}` and the quadrature loses
    /// its own significance. Below `x = 0.01` the `I_1` integrand is `O(1)`
    /// against a `cos t` that integrates to `x/2` — at `x = 1e-7` the
    /// cancellation leaves the reference with about eight significant
    /// figures, which showed up as a 1.019e-8 "failure" that was entirely the
    /// reference's. `x = 0.01` is still contaminated at 1.702e-13 by the same
    /// mechanism while every point from `x = 0.1` up sits below 8.3e-15, so
    /// the cut is drawn there. The small-argument branches are covered
    /// instead by
    /// `the_small_argument_branches_give_the_leading_terms` and by the
    /// Wronskian, which holds down to `x = 1e-7`.
    #[test]
    fn i0_and_i1_match_their_integral_representations() {
        let (mut w0, mut w1) = (0.0_f64, 0.0_f64);
        for x in sweep().filter(|&x| (0.1..=20.0).contains(&x)) {
            let (r0, r1) = (i_by_quadrature(0, x), i_by_quadrature(1, x));
            w0 = w0.max(((bessel_i0(x) - r0) / r0).abs());
            if r1 > 0.0 {
                w1 = w1.max(((bessel_i1(x) - r1) / r1).abs());
            }
        }
        assert!(w0 < 1e-14, "I_0 relative error {w0:e}");
        assert!(w1 < 1e-14, "I_1 relative error {w1:e}");
    }

    /// `K_0` and `K_1` against `int_0^inf exp(-x cosh t) cosh(n t) dt`.
    ///
    /// Restricted to `x >= 0.01`: below that the integrand's plateau extends
    /// to `t ~ ln(2/x)` and the fixed trapezoid stops being the more accurate
    /// of the two.
    #[test]
    fn k0_and_k1_match_their_integral_representations() {
        let (mut w0, mut w1) = (0.0_f64, 0.0_f64);
        for x in sweep().filter(|&x| (0.01..=30.0).contains(&x)) {
            let (r0, r1) = (k_by_quadrature(0, x), k_by_quadrature(1, x));
            w0 = w0.max(((bessel_k0(x) - r0) / r0).abs());
            w1 = w1.max(((bessel_k1(x) - r1) / r1).abs());
        }
        assert!(w0 < 1e-13, "K_0 relative error {w0:e}");
        assert!(w1 < 1e-13, "K_1 relative error {w1:e}");
    }

    /// `Y_0` against the Neumann series, which shares no table with it.
    ///
    /// The series truncates at 40 terms, so it is only trustworthy while
    /// `J_{80}(x)` is negligible — comfortably true up to `x = 20`.
    #[test]
    fn y0_matches_the_neumann_series() {
        let mut worst = 0.0_f64;
        for x in sweep().filter(|&x| (0.1..=20.0).contains(&x)) {
            worst = worst.max((bessel_y0(x) - y0_by_series(x)).abs());
        }
        assert!(worst < 1e-13, "Y_0 absolute error {worst:e}");
    }

    /// The scaled forms must equal their unscaled counterparts times the
    /// exponential, wherever both are representable. This is what makes the
    /// scaled entry points usable as a drop-in at moderate argument.
    #[test]
    fn the_scaled_forms_agree_with_the_unscaled_ones() {
        let mut worst = 0.0_f64;
        for x in sweep().filter(|&x| x <= 50.0) {
            for (scaled, plain, factor) in [
                (bessel_i0_scaled(x), bessel_i0(x), (-x).exp()),
                (bessel_i1_scaled(x), bessel_i1(x), (-x).exp()),
                (bessel_k0_scaled(x), bessel_k0(x), x.exp()),
                (bessel_k1_scaled(x), bessel_k1(x), x.exp()),
            ] {
                let expect = plain * factor;
                if expect != 0.0 && expect.is_finite() {
                    worst = worst.max(((scaled - expect) / expect).abs());
                }
            }
        }
        assert!(worst < 1e-14, "scaled/unscaled disagreement {worst:e}");
    }

    /// Beyond `f64`'s reach the scaled forms must still work, which is the
    /// entire reason they exist.
    #[test]
    fn the_scaled_forms_survive_where_the_plain_ones_cannot() {
        assert!(bessel_i0(1000.0).is_infinite());
        assert_eq!(bessel_k0(1000.0), 0.0);
        // e^{-x} I_0(x) -> 1/sqrt(2 pi x) and e^{x} K_0(x) -> sqrt(pi/2x).
        let x = 1000.0;
        let i0s = bessel_i0_scaled(x);
        let k0s = bessel_k0_scaled(x);
        assert!(((i0s - 1.0 / (2.0 * PI * x).sqrt()) / i0s).abs() < 1e-3);
        assert!(((k0s - (PI / (2.0 * x)).sqrt()) / k0s).abs() < 1e-3);
        // Their product tends to 1/(2x) -- the Wronskian's leading term.
        assert!((i0s * k0s * 2.0 * x - 1.0).abs() < 1e-3);
    }

    /// Parity: `J_0`, `I_0` are even; `J_1`, `I_1` are odd. The sign is
    /// carried by explicit branches in [`bessel_j1`] and [`bessel_i1_scaled`],
    /// not by the series, so it is worth pinning.
    #[test]
    fn the_parities_hold_exactly() {
        for x in sweep() {
            assert_eq!(bessel_j0(x), bessel_j0(-x), "J_0 parity at {x}");
            assert_eq!(bessel_i0(x), bessel_i0(-x), "I_0 parity at {x}");
            assert_eq!(bessel_j1(x), -bessel_j1(-x), "J_1 parity at {x}");
            assert_eq!(bessel_i1(x), -bessel_i1(-x), "I_1 parity at {x}");
        }
    }

    /// `Y_0`, `Y_1`, `K_0`, `K_1` are undefined for `x <= 0` and say so with
    /// `NaN` rather than returning a plausible-looking number.
    #[test]
    fn the_second_kind_functions_reject_non_positive_arguments() {
        for x in [-1.0, -1e-300, 0.0] {
            assert!(bessel_y0(x).is_nan(), "Y_0({x})");
            assert!(bessel_y1(x).is_nan(), "Y_1({x})");
            assert!(bessel_k0(x).is_nan(), "K_0({x})");
            assert!(bessel_k1(x).is_nan(), "K_1({x})");
        }
    }

    /// Every entry point propagates `NaN` rather than reading a table at a
    /// nonsense index or falling through to a plausible branch.
    #[test]
    fn nan_propagates_through_every_entry_point() {
        let n = f64::NAN;
        for v in [
            bessel_j0(n),
            bessel_j1(n),
            bessel_y0(n),
            bessel_y1(n),
            bessel_i0(n),
            bessel_i1(n),
            bessel_k0(n),
            bessel_k1(n),
            bessel_i0_scaled(n),
            bessel_i1_scaled(n),
            bessel_k0_scaled(n),
            bessel_k1_scaled(n),
        ] {
            assert!(v.is_nan());
        }
    }

    /// The documented divergence from GSL: upstream's generic overflow macro
    /// returns `+inf` for `I_1` whatever the sign of the argument; this
    /// translation returns `-inf` for a negative one. See [`bessel_i1`].
    #[test]
    fn i1_overflows_with_the_sign_of_its_argument() {
        assert_eq!(bessel_i1(1000.0), f64::INFINITY);
        assert_eq!(bessel_i1(-1000.0), f64::NEG_INFINITY);
    }

    /// The small-argument limits, which are separate early-return branches and
    /// therefore not exercised by any of the sweeps above.
    #[test]
    fn the_small_argument_branches_give_the_leading_terms() {
        let tiny = 1e-12;
        assert_eq!(bessel_j0(tiny), 1.0);
        assert_eq!(bessel_i0(tiny), 1.0);
        assert_eq!(bessel_j1(tiny), 0.5 * tiny);
        assert_eq!(bessel_i1(tiny), 0.5 * tiny);
        assert_eq!(bessel_i0_scaled(tiny), 1.0 - tiny);
        // K_1 ~ 1/x and Y_1 ~ -2/(pi x).
        assert!((bessel_k1(tiny) * tiny - 1.0).abs() < 1e-20);
        assert!((bessel_y1(tiny) * tiny + 2.0 / PI).abs() < 1e-11);
    }

    /// `poly_eval` is upstream's `gsl_poly_eval` written as a fold; check it
    /// against the obvious expansion on a case with no special structure.
    #[test]
    fn poly_eval_is_horner() {
        let c = [2.0, -3.0, 0.5, 7.0];
        let x = 1.75;
        let expect = 2.0 - 3.0 * x + 0.5 * x * x + 7.0 * x * x * x;
        assert!((poly_eval(&c, x) - expect).abs() < 1e-14);
        assert_eq!(poly_eval(&[], x), 0.0);
    }

    /// `cos_pi4`/`sin_pi4` compute `cos(y - pi/4 + eps)` and
    /// `sin(y - pi/4 + eps)` without ever forming that sum, which is the whole
    /// point of them. Check against the naive form where `y` is small enough
    /// that the naive form is still accurate.
    #[test]
    fn the_pi4_helpers_agree_with_the_naive_form_at_small_argument() {
        for y in [0.5, 1.0, 4.0, 10.0, 100.0] {
            for eps in [0.0, 1e-9, 1e-4, 0.01, 0.3] {
                let arg = y - PI / 4.0 + eps;
                assert!(
                    (cos_pi4(y, eps) - arg.cos()).abs() < 1e-13,
                    "cos_pi4({y}, {eps})"
                );
                assert!(
                    (sin_pi4(y, eps) - arg.sin()).abs() < 1e-13,
                    "sin_pi4({y}, {eps})"
                );
            }
        }
    }
}
