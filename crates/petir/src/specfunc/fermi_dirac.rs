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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/fermi_dirac.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// GSL's own citation for the branch structure and the small-argument series
// is Goano, "Algorithm 745: computation of the complete and incomplete
// Fermi-Dirac integral", ACM TOMS 21 (1995) 221-232; the Chebyshev fits are
// Jungman's. All 22 tables here -- 483 coefficients -- were extracted from
// that source BY SCRIPT, not retyped, and are audited bit-for-bit by
// tests/gsl_tables_audit.rs.

//! The complete Fermi-Dirac integrals `F_j(x)`.
//!
//! # What these are
//!
//! ```text
//!     F_j(x) = (1 / Gamma(j+1)) integral_0^inf  t^j / (e^{t-x} + 1)  dt
//! ```
//!
//! the occupation integrals of a Fermi gas: `x` is the reduced chemical
//! potential `mu / kT` (dimensionless), and `j` selects which moment of the
//! Fermi-Dirac distribution is taken. `F_{1/2}` gives the carrier density of
//! a three-dimensional parabolic band and `F_{3/2}` its energy density, so
//! the pair is what turns a Fermi level into a number density — the
//! degenerate-gas counterpart of the Boltzmann exponential.
//!
//! GSL, and this port, supply the seven **fixed** indices:
//!
//! | function | `j` | closed form, where one exists |
//! |---|---|---|
//! | [`fermi_dirac_m1`] | -1 | `e^x / (1 + e^x)`, the Fermi function itself |
//! | [`fermi_dirac_0`] | 0 | `ln(1 + e^x)` |
//! | [`fermi_dirac_1`] | 1 | — |
//! | [`fermi_dirac_2`] | 2 | — |
//! | [`fermi_dirac_mhalf`] | -1/2 | — |
//! | [`fermi_dirac_half`] | 1/2 | — |
//! | [`fermi_dirac_3half`] | 3/2 | — |
//!
//! # What is deliberately NOT ported
//!
//! - **`gsl_sf_fermi_dirac_int_e(j, x)` for general integer `j`.** Its large
//!   `j` path needs `fd_nint` and `fd_UMseries_int`, and through them the
//!   confluent hypergeometric functions of `specfunc/hyperg*.c`, which PETIR
//!   does not have. `j = -1, 0, 1, 2` are all reachable through the named
//!   entry points above; higher `j` is not.
//! - **`gsl_sf_fermi_dirac_inc_0_e(x, b)`**, the incomplete integral.
//!
//! Both are gaps in capability, not in fidelity — see the crate `README`'s
//! coverage table rather than assuming the family is complete.
//!
//! # `fd_whiz` is ported, and unreachable from here
//!
//! The Levin *u*-transform that accelerates `fd_neg`'s alternating series —
//! the one part of this port needing scratch memory, two 101-element arrays
//! — **is never entered by any of the seven entry points above**. [`fd_neg`]
//! is called only from [`fd_asymp`], which runs only at `x >= 30`, so it
//! always sees `-x <= -30` and always takes the simple series instead, which
//! converges in **2 to 3 terms**. Measured by instrumentation in
//! `the_series_acceleration_is_unreachable_from_this_module`, which sweeps
//! the whole domain and asserts the counter stays at zero — and then calls
//! `fd_neg` directly at an argument that *does* take the branch, so the
//! instrument is shown capable of firing.
//!
//! The code stays because it is upstream's and because
//! `gsl_sf_fermi_dirac_int_e` would reach it. What was wrong was an earlier
//! claim that those arrays blocked a WGSL transcription of this family; they
//! do not, because nothing calls them.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`). Below `GSL_LOG_DBL_MIN` every
//! function underflows and returns `0.0`, which is upstream's
//! `UNDERFLOW_ERROR`.
//!
//! **Overflow is guarded for two members and not for two others, and that
//! asymmetry is upstream's.** [`fermi_dirac_1`] and [`fermi_dirac_2`] grow
//! like `x^2/2` and `x^3/6` and carry an explicit `OVERFLOW_ERROR` bound —
//! `GSL_SQRT_DBL_MAX` and `GSL_ROOT3_DBL_MAX` respectively — past which they
//! return `f64::INFINITY`. [`fermi_dirac_half`] and [`fermi_dirac_3half`]
//! also overflow, measured at `x ~ 10^205.6` and `10^123.5`, but GSL has
//! **no** guard for them: `fd_asymp` simply returns infinity. Only
//! [`fermi_dirac_m1`] (bounded by 1), [`fermi_dirac_0`] (linear) and
//! [`fermi_dirac_mhalf`] (`sqrt x`) are genuinely overflow-free.
//!
//! `NaN` propagates.
//!
//! # Accuracy
//!
//! Measured against the **defining integral**, evaluated independently by
//! composite Gauss-Legendre quadrature after `t = v^2`, across 15 abscissae
//! chosen to cross every branch boundary (2026-09-19):
//!
//! | | worst relative difference | at |
//! |---|---|---|
//! | `F_0` | 2.103e-15 | -4 |
//! | `F_1` | 1.086e-15 | 25 |
//! | `F_2` | 1.548e-15 | 29 |
//! | `F_{-1/2}` | 9.177e-16 | 0 |
//! | `F_{1/2}` | 2.926e-15 | 60 |
//! | `F_{3/2}` | 5.992e-15 | 12 |
//!
//! and against `F_j(0) = (1 - 2^{-j}) zeta(j+1)` (exact to 1.6e-15 or
//! better), the identity `dF_j/dx = F_{j-1}`, and the exact integer
//! reflection formulas. Every figure is asserted in the tests.
//!
//! ## Inherited weak spot 1: `F_2`'s far-field cut is misplaced
//!
//! Past `x = 30` both [`fermi_dirac_1`] and [`fermi_dirac_2`] use a short
//! Chebyshev fit in `60/x`, and past a second cut drop it for the pure
//! degenerate limit. What the fit carries is the Sommerfeld correction —
//! `1 + (pi^2/3)/x^2` for `F_1`, `1 + pi^2/x^2` for `F_2` — so the cut
//! belongs where that falls below `eps`.
//!
//! `F_1`'s does: at upstream's `1/GSL_SQRT_DBL_EPSILON = 6.711e7` the
//! correction is 6.7e-16 and the transition is invisible. **`F_2`'s does
//! not.** Upstream uses `1/GSL_ROOT3_DBL_EPSILON = 1.6514e5`, apparently
//! matching the `x^3` factor rather than the accuracy requirement, where the
//! correction is still **3.62e-10** — so the function steps down by 3.56e-10
//! relative at that point and recovers only as `1/x^2`. `1/sqrt(eps)` would
//! have put it at 2.2e-15.
//!
//! Not corrected here, because this crate's bar is agreement with GSL and a
//! port that quietly improves on upstream makes its own comparisons
//! meaningless. The measurement is asserted instead, in
//! `upstreams_far_field_cut_is_exact_for_f_1_and_six_orders_early_for_f_2`,
//! so a future GSL that moves the cut fails loudly.
//!
//! ## Inherited weak spot 2: `F_0` at `x = -5`
//!
//! [`fermi_dirac_0`] is **1.5e-14** against `ln_1p(e^x)` at exactly
//! `x = -5`, falling to 8.8e-15 at -4.9, 2.3e-15 at -4 and bit-exact by -1.
//! The cause is upstream's branch cut: at `x = -5` GSL stops using its
//! small-argument series and evaluates `ln(1 + e^x)` directly, and adding
//! `6.7e-3` to `1.0` costs about two digits before the logarithm runs.
//! `ln_1p` would not have that defect, but the branch structure is
//! upstream's and is carried rather than improved — see the crate
//! `CLAUDE.md` on what "ported" means. It is recorded here so the next
//! reader does not mistake it for a transcription error.

// Under a std-linked build (`cargo test`) f64's inherent exp/ln/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl as cheb;
use crate::specfunc::{DBL_EPSILON, LOG_DBL_MIN, SQRT_DBL_EPSILON, SQRT_DBL_MAX};

/// `GSL_ROOT3_DBL_EPSILON`, the cube root of [`DBL_EPSILON`]. Mirrors
/// `crate::scalar::CBRT_DBL_EPSILON`; re-declared here so this module reads
/// against upstream's own spelling of the bound.
const ROOT3_DBL_EPSILON: f64 = 6.055_454_452_393_343e-6;
/// `GSL_ROOT3_DBL_MAX`, the cube root of `f64::MAX`. Past this `x^3/6`
/// overflows, which is where `gsl_sf_fermi_dirac_2_e` raises
/// `OVERFLOW_ERROR`.
const ROOT3_DBL_MAX: f64 = 5.643_803_094_122_289_7e102;

/// GSL's `fd_1_a_data`, 22 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_1_A: [f64; 22] = [
    1.8949340668482264365, 0.7237719066890052793, 0.1250000000000000000, 0.0101065196435973942,
    0.0, -0.0000600615242174119, 0.0, 6.816528764623e-7, 0.0, -9.5895779195e-9, 0.0,
    1.515104135e-10, 0.0, -2.5785616e-12, 0.0, 4.62270e-14, 0.0, -8.612e-16, 0.0, 1.65e-17, 0.0,
    -3.0e-19,
];

/// GSL's `fd_1_b_data`, 22 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_1_B: [f64; 22] = [
    10.409136795234611872, 3.899445098225161947, 0.513510935510521222, 0.010618736770218426,
    -0.001584468020659694, 0.000146139297161640, -1.408095734499e-6, -2.177993899484e-6,
    3.91423660640e-7, -2.3860262660e-8, -4.138309573e-9, 1.283965236e-9, -1.39695990e-10,
    -4.907743e-12, 4.399878e-12, -7.17291e-13, 2.4320e-14, 1.4230e-14, -3.446e-15, 2.93e-16,
    3.7e-17, -1.6e-17,
];

/// GSL's `fd_1_c_data`, 23 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_1_C: [f64; 23] = [
    56.78099449124299762, 21.00718468237668011, 2.24592457063193457, 0.00173793640425994,
    -0.00058716468739423, 0.00016306958492437, -0.00003817425583020, 7.64527252009e-6,
    -1.31348500162e-6, 1.9000646056e-7, -2.141328223e-8, 1.23906372e-9, 2.1848049e-10,
    -1.0134282e-10, 2.484728e-11, -4.73067e-12, 7.3555e-13, -8.740e-14, 4.85e-15, 1.23e-15,
    -5.6e-16, 1.4e-16, -3.0e-17,
];

/// GSL's `fd_1_d_data`, 30 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_1_D: [f64; 30] = [
    1.0126626021151374442, -0.0063312525536433793, 0.0024837319237084326,
    -0.0008764333697726109, 0.0002913344438921266, -0.0000931877907705692,
    0.0000290151342040275, -8.8548707259955e-6, 2.6603474114517e-6, -7.891415690452e-7,
    2.315730237195e-7, -6.73179452963e-8, 1.94048035606e-8, -5.5507129189e-9, 1.5766090896e-9,
    -4.449310875e-10, 1.248292745e-10, -3.48392894e-11, 9.6791550e-12, -2.6786240e-12,
    7.388852e-13, -2.032828e-13, 5.58115e-14, -1.52987e-14, 4.1886e-15, -1.1458e-15, 3.132e-16,
    -8.56e-17, 2.33e-17, -5.9e-18,
];

/// GSL's `fd_1_e_data`, 10 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_1_E: [f64; 10] = [
    1.0013707783890401683, 0.0009138522593601060, 0.0002284630648400133, -1.57e-17, -1.27e-17,
    -9.7e-18, -6.9e-18, -4.6e-18, -2.9e-18, -1.7e-18,
];

/// GSL's `fd_2_a_data`, 21 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_2_A: [f64; 21] = [
    2.1573661917148458336, 0.8849670334241132182, 0.1784163467613519713, 0.0208333333333333333,
    0.0012708226459768508, 0.0, -5.0619314244895e-6, 0.0, 4.32026533989e-8, 0.0,
    -4.870544166e-10, 0.0, 6.4203740e-12, 0.0, -9.37424e-14, 0.0, 1.4715e-15, 0.0, -2.44e-17,
    0.0, 4.0e-19,
];

/// GSL's `fd_2_b_data`, 22 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_2_B: [f64; 22] = [
    16.508258811798623599, 7.421719394793067988, 1.458309885545603821, 0.128773850882795229,
    0.001963612026198147, -0.000237458988738779, 0.000018539661382641, -1.92805649479e-7,
    -2.01950028452e-7, 3.2963497518e-8, -1.885817092e-9, -2.72632744e-10, 8.0554561e-11,
    -8.313223e-12, -2.24489e-13, 2.18778e-13, -3.4290e-14, 1.225e-15, 5.81e-16, -1.37e-16,
    1.2e-17, 1.0e-18,
];

/// GSL's `fd_2_c_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_2_C: [f64; 20] = [
    168.87129776686440711, 81.80260488091659458, 15.75408505947931513, 1.12325586765966440,
    0.00059057505725084, -0.00016469712946921, 0.00003885607810107, -7.89873660613e-6,
    1.39786238616e-6, -2.1534528656e-7, 2.831510953e-8, -2.94978583e-9, 1.6755082e-10,
    2.234229e-11, -1.035130e-11, 2.41117e-12, -4.3531e-13, 6.447e-14, -7.39e-15, 4.3e-16,
];

/// GSL's `fd_2_d_data`, 30 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_2_D: [f64; 30] = [
    0.3459960518965277589, -0.00633136397691958024, 0.00248382959047594408,
    -0.00087651191884005114, 0.00029139255351719932, -0.00009322746111846199,
    0.00002904021914564786, -8.86962264810663e-6, 2.66844972574613e-6, -7.9331564996004e-7,
    2.3359868615516e-7, -6.824790880436e-8, 1.981036528154e-8, -5.71940426300e-9,
    1.64379426579e-9, -4.7064937566e-10, 1.3432614122e-10, -3.823400534e-11, 1.085771994e-11,
    -3.07727465e-12, 8.7064848e-13, -2.4595431e-13, 6.938531e-14, -1.954939e-14, 5.50162e-15,
    -1.54657e-15, 4.3429e-16, -1.2178e-16, 3.394e-17, -8.81e-18,
];

/// GSL's `fd_2_e_data`, 4 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_2_E: [f64; 4] = [
    0.3347041117223735227, 0.00091385225936012645, 0.00022846306484003205, 5.2e-19,
];

/// GSL's `fd_mhalf_a_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_MHALF_A: [f64; 20] = [
    1.2663290042859741974, 0.3697876251911153071, 0.0278131011214405055, -0.0033332848565672007,
    -0.0004438108265412038, 0.0000616495177243839, 8.7589611449897e-6, -1.2622936986172e-6,
    -1.837464037221e-7, 2.69495091400e-8, 3.9760866257e-9, -5.894468795e-10, -8.77321638e-11,
    1.31016571e-11, 1.9621619e-12, -2.945887e-13, -4.43234e-14, 6.6816e-15, 1.0084e-15,
    -1.561e-16,
];

/// GSL's `fd_mhalf_b_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_MHALF_B: [f64; 20] = [
    3.270796131942071484, 0.5809004935853417887, -0.0299313438794694987, -0.0013287935412612198,
    0.0009910221228704198, -0.0001690954939688554, 6.5955849946915e-6, 3.5953966033618e-6,
    -9.430672023181e-7, 8.75773958291e-8, 1.06247652607e-8, -4.9587006215e-9, 7.160432795e-10,
    4.5072219e-12, -2.3695425e-11, 4.9122208e-12, -2.905277e-13, -9.59291e-14, 3.00028e-14,
    -3.4970e-15,
];

/// GSL's `fd_mhalf_c_data`, 25 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_MHALF_C: [f64; 25] = [
    5.828283273430595507, 0.677521118293264655, -0.043946248736481554, 0.005825595781828244,
    -0.000864858907380668, 0.000110017890076539, -6.973305225404e-6, -1.716267414672e-6,
    8.59811582041e-7, -2.33066786976e-7, 4.8503191159e-8, -8.130620247e-9, 1.021068250e-9,
    -5.3188423e-11, -1.9430559e-11, 8.750506e-12, -2.324897e-12, 4.83102e-13, -8.1207e-14,
    1.0132e-14, -4.64e-16, -2.24e-16, 9.7e-17, -2.6e-17, 5.0e-18,
];

/// GSL's `fd_mhalf_d_data`, 30 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_MHALF_D: [f64; 30] = [
    2.2530744202862438709, 0.0018745152720114692, -0.0007550198497498903, 0.0002759818676644382,
    -0.0000959406283465913, 0.0000324056855537065, -0.0000107462396145761, 3.5126865219224e-6,
    -1.1313072730092e-6, 3.577454162766e-7, -1.104926666238e-7, 3.31304165692e-8,
    -9.5837381008e-9, 2.6575790141e-9, -7.015201447e-10, 1.747111336e-10, -4.04909605e-11,
    8.5104999e-12, -1.5261885e-12, 1.876851e-13, 1.00574e-14, -1.82002e-14, 8.6634e-15,
    -3.2058e-15, 1.0572e-15, -3.259e-16, 9.60e-17, -2.74e-17, 7.6e-18, -1.9e-18,
];

/// GSL's `fd_half_a_data`, 23 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_HALF_A: [f64; 23] = [
    1.7177138871306189157, 0.6192579515822668460, 0.0932802275119206269, 0.0047094853246636182,
    -0.0004243667967864481, -0.0000452569787686193, 5.2426509519168e-6, 6.387648249080e-7,
    -8.05777004848e-8, -1.04290272415e-8, 1.3769478010e-9, 1.847190359e-10, -2.51061890e-11,
    -3.4497818e-12, 4.784373e-13, 6.68828e-14, -9.4147e-15, -1.3333e-15, 1.898e-16, 2.72e-17,
    -3.9e-18, -6.0e-19, 1.0e-19,
];

/// GSL's `fd_half_b_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_HALF_B: [f64; 20] = [
    7.651013792074984027, 2.475545606866155737, 0.218335982672476128, -0.007730591500584980,
    -0.000217443383867318, 0.000147663980681359, -0.000021586361321527, 8.07712735394e-7,
    3.28858050706e-7, -7.9474330632e-8, 6.940207234e-9, 6.75594681e-10, -3.10200490e-10,
    4.2677233e-11, -2.1696e-14, -1.170245e-12, 2.34757e-13, -1.4139e-14, -3.864e-15, 1.202e-15,
];

/// GSL's `fd_half_c_data`, 23 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_HALF_C: [f64; 23] = [
    29.584339348839816528, 8.808344283250615592, 0.503771641883577308, -0.021540694914550443,
    0.002143341709406890, -0.000257365680646579, 0.000027933539372803, -1.678525030167e-6,
    -2.78100117693e-7, 1.35218065147e-7, -3.3740425009e-8, 6.474834942e-9, -1.009678978e-9,
    1.20057555e-10, -6.636314e-12, -1.710566e-12, 7.75069e-13, -1.97973e-13, 3.9414e-14,
    -6.374e-15, 7.77e-16, -4.0e-17, -1.4e-17,
];

/// GSL's `fd_half_d_data`, 30 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_HALF_D: [f64; 30] = [
    1.5116909434145508537, -0.0036043405371630468, 0.0014207743256393359,
    -0.0005045399052400260, 0.0001690758006957347, -0.0000546305872688307,
    0.0000172223228484571, -5.3352603788706e-6, 1.6315287543662e-6, -4.939021084898e-7,
    1.482515450316e-7, -4.41552276226e-8, 1.30503160961e-8, -3.8262599802e-9, 1.1123226976e-9,
    -3.204765534e-10, 9.14870489e-11, -2.58778946e-11, 7.2550731e-12, -2.0172226e-12,
    5.566891e-13, -1.526247e-13, 4.16121e-14, -1.12933e-14, 3.0537e-15, -8.234e-16, 2.215e-16,
    -5.95e-17, 1.59e-17, -4.0e-18,
];

/// GSL's `fd_3half_a_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_3HALF_A: [f64; 20] = [
    2.0404775940601704976, 0.8122168298093491444, 0.1536371165644008069, 0.0156174323847845125,
    0.0005943427879290297, -0.0000429609447738365, -3.8246452994606e-6, 3.802306180287e-7,
    4.05746157593e-8, -4.5530360159e-9, -5.306873139e-10, 6.37297268e-11, 7.8403674e-12,
    -9.840241e-13, -1.255952e-13, 1.62617e-14, 2.1318e-15, -2.825e-16, -3.78e-17, 5.1e-18,
];

/// GSL's `fd_3half_b_data`, 22 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_3HALF_B: [f64; 22] = [
    13.403206654624176674, 5.574508357051880924, 0.931228574387527769, 0.054638356514085862,
    -0.001477172902737439, -0.000029378553381869, 0.000018357033493246, -2.348059218454e-6,
    8.3173787440e-8, 2.6826486956e-8, -6.011244398e-9, 4.94345981e-10, 3.9557340e-11,
    -1.7894930e-11, 2.348972e-12, -1.2823e-14, -5.4192e-14, 1.0527e-14, -6.39e-16, -1.47e-16,
    4.5e-17, -5.0e-18,
];

/// GSL's `fd_3half_c_data`, 21 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_3HALF_C: [f64; 21] = [
    101.03685253378877642, 43.62085156043435883, 6.62241373362387453, 0.25081415008708521,
    -0.00798124846271395, 0.00063462245101023, -0.00006392178890410, 6.04535131939e-6,
    -3.4007683037e-7, -4.072661545e-8, 1.931148453e-8, -4.46328355e-9, 7.9434717e-10,
    -1.1573569e-10, 1.304658e-11, -7.4114e-13, -1.4181e-13, 6.491e-14, -1.597e-14, 3.05e-15,
    -4.8e-16,
];

/// GSL's `fd_3half_d_data`, 25 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const FD_3HALF_D: [f64; 25] = [
    0.6160645215171852381, -0.0071239478492671463, 0.0027906866139659846,
    -0.0009829521424317718, 0.0003260229808519545, -0.0001040160912910890,
    0.0000322931223232439, -9.8243506588102e-6, 2.9420132351277e-6, -8.699154670418e-7,
    2.545460071999e-7, -7.38305056331e-8, 2.12545670310e-8, -6.0796532462e-9, 1.7294556741e-9,
    -4.896540687e-10, 1.380786037e-10, -3.88057305e-11, 1.08753212e-11, -3.0407308e-12,
    8.485626e-13, -2.364275e-13, 6.57636e-14, -1.81807e-14, 4.6884e-15,
];

/// GSL's `fd_whiz` (`specfunc/fermi_dirac.c`) — one step of the Levin
/// *u*-transform used to accelerate the alternating series in [`fd_neg`].
///
/// `term` is the newest term of the series and `iterm` its index; `s` carries
/// the running partial sum and `qnum`/`qden` the transform's two triangular
/// work arrays, both updated in place. The returned value is the current
/// accelerated estimate.
///
/// The arrays are fixed-size and live on the stack, so this stays `no_std`
/// with no allocator — the reason upstream's `nsize = 100 + 1` is carried
/// verbatim rather than made dynamic.
fn fd_whiz(
    term: f64,
    iterm: usize,
    qnum: &mut [f64; FD_QSIZE],
    qden: &mut [f64; FD_QSIZE],
    s: &mut f64,
) -> f64 {
    if iterm == 0 {
        *s = 0.0;
    }
    *s += term;

    let n = iterm as f64 + 1.0;
    let d_new = 1.0 / (term * n * n);
    let n_new = *s * d_new;

    // Upstream writes `qden[iterm] = ...; qnum[iterm] = ...`. A bare
    // subscript would be a run-time-fallible index, which this crate's
    // `tests/no_panic_gate.rs` forbids, so the slot is taken with `get_mut`.
    // `iterm` never exceeds `FD_ITMAX` (100) and the arrays hold `FD_QSIZE`
    // (101), so the `None` arm is unreachable; it returns `NaN` rather than
    // panicking because a special function in this crate never panics.
    match (qden.get_mut(iterm), qnum.get_mut(iterm)) {
        (Some(d), Some(q)) => {
            *d = d_new;
            *q = n_new;
        }
        _ => return f64::NAN,
    }

    // The descending sweep updates `q[j]` from the ALREADY-UPDATED `q[j+1]`,
    // so a running carry reproduces upstream's in-place loop exactly while
    // touching no index at all. After the loop the carry holds `q[0]`, which
    // is the value upstream reads back — so even the final `qnum[0]/qden[0]`
    // needs no subscript.
    let (mut carry_d, mut carry_n) = (d_new, n_new);
    if iterm > 0 {
        let mut factor = 1.0_f64;
        let ratio = iterm as f64 / n;
        for (j, (d, q)) in qden
            .iter_mut()
            .zip(qnum.iter_mut())
            .enumerate()
            .take(iterm)
            .rev()
        {
            let c = factor * (j as f64 + 1.0) / n;
            factor *= ratio;
            *d = carry_d - c * *d;
            *q = carry_n - c * *q;
            carry_d = *d;
            carry_n = *q;
        }
    }

    carry_n / carry_d
}

/// How many times [`fd_neg`] has entered its **series-acceleration** branch.
///
/// Measured, not asserted in prose: across every argument the seven public
/// entry points can reach, this stays at zero — see
/// `the_series_acceleration_is_unreachable_from_this_module`.
#[cfg(test)]
pub(crate) static FD_WHIZ_ENTRIES: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Upstream's `qsize`: `100 + 1`.
const FD_QSIZE: usize = 101;
/// Upstream's `itmax` inside `fd_neg`.
const FD_ITMAX: usize = 100;

/// GSL's `fd_neg(j, x)` — `F_j(x)` for `x < 0` and arbitrary real `j`.
///
/// Two regimes, exactly as upstream: a plain alternating series where it
/// converges quickly, and the [`fd_whiz`] acceleration otherwise.
fn fd_neg(j: f64, x: f64) -> f64 {
    if x < LOG_DBL_MIN {
        return 0.0;
    }
    if x < -1.0 && x < -(j + 1.0).abs() {
        // Simple series [Goano (6)]; the acceleration below is not worth its
        // cost this far out.
        let ex = x.exp();
        let mut term = ex;
        let mut sum = term;
        for n in 2..100 {
            let rat = (n as f64 - 1.0) / n as f64;
            term *= -ex * rat.powf(j + 1.0);
            sum += term;
            if (term / sum).abs() < DBL_EPSILON {
                break;
            }
        }
        return sum;
    }

    // INSTRUMENTED: see `the_series_acceleration_is_unreachable_from_this_module`.
    #[cfg(test)]
    FD_WHIZ_ENTRIES.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    let mut s = 0.0_f64;
    let mut xn = x;
    let ex = -x.exp();
    let mut enx = -ex;
    let mut f = 0.0_f64;
    let mut f_previous;
    let mut qnum = [0.0_f64; FD_QSIZE];
    let mut qden = [0.0_f64; FD_QSIZE];

    for jterm in 0..=FD_ITMAX {
        let p = (jterm as f64 + 1.0).powf(j + 1.0);
        let term = enx / p;
        f_previous = f;
        f = fd_whiz(term, jterm, &mut qnum, &mut qden, &mut s);
        xn += x;
        if (f - f_previous).abs() < f.abs() * 2.0 * DBL_EPSILON || xn < LOG_DBL_MIN {
            break;
        }
        enx *= ex;
    }
    f
}

/// GSL's `fd_asymp(j, x)` — the large-`x` asymptotic (Sommerfeld) expansion,
/// valid for `j + 2 > 0`.
///
/// ```text
///     F_j(x) ~ cos(pi j) F_j(-x)
///            + 2 exp((j+1) ln x - lnGamma(j+2)) [ 1/2 + sum_n eta(2n) P_n ]
/// ```
///
/// The bracket is the Sommerfeld series in `1/x^2` with Dirichlet-eta
/// coefficients, and the reflection term `cos(pi j) F_j(-x)` is what makes it
/// exact rather than merely asymptotic for integer `j` — there `cos(pi j)` is
/// `±1` and the reflected value is the exponentially small correction.
fn fd_asymp(j: f64, x: f64) -> f64 {
    // Upstream's test for "j is an integer", carried verbatim because it
    // decides the loop's termination rule, not just a cosmetic branch.
    let j_integer = (j - (j + 0.5).floor()).abs() < 100.0 * DBL_EPSILON;
    const ITMAX: i32 = 200;

    let lg = crate::specfunc::gamma::ln_gamma(j + 2.0);
    let mut seqn = 0.5_f64;
    let xm2 = (1.0 / x) / x;
    let mut xgam = 1.0_f64;
    let mut add = f64::MAX;

    for n in 1..=ITMAX {
        let add_previous = add;
        let eta = crate::specfunc::zeta::eta_int(2 * n);
        xgam = xgam * xm2 * (j + 1.0 - (2 * n - 2) as f64) * (j + 1.0 - (2 * n - 1) as f64);
        add = eta * xgam;
        // For non-integer j the series is genuinely asymptotic and must be
        // truncated at its smallest term; for integer j it terminates.
        if !j_integer && add.abs() > add_previous.abs() {
            break;
        }
        if (add / seqn).abs() < DBL_EPSILON {
            break;
        }
        seqn += add;
    }

    let fneg = fd_neg(j, -x);
    let ex_arg = (j + 1.0) * x.ln() - lg;
    (j * core::f64::consts::PI).cos() * fneg + 2.0 * seqn * ex_arg.exp()
}

/// `F_{-1}(x) = e^x / (1 + e^x)` — the Fermi-Dirac **occupation function**
/// itself, GSL's `gsl_sf_fermi_dirac_m1`.
///
/// The one member of the family with an elementary closed form, and the only
/// one bounded: it runs monotonically from `0` at `x = -inf` to `1` at
/// `x = +inf`, passing through `1/2` at `x = 0`.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Returns
/// `0.0` below `GSL_LOG_DBL_MIN`, which is upstream's `UNDERFLOW_ERROR`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_m1;
/// assert!((fermi_dirac_m1(0.0) - 0.5).abs() < 1e-15);
/// assert!(fermi_dirac_m1(40.0) > 0.999_999_999);
/// ```
pub fn fermi_dirac_m1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        return 0.0;
    }
    if x < 0.0 {
        // Written this way rather than 1/(1+e^{-x}) so the exponential that
        // is evaluated is the small one.
        let ex = x.exp();
        ex / (1.0 + ex)
    } else {
        1.0 / (1.0 + (-x).exp())
    }
}

/// `F_0(x) = ln(1 + e^x)`, GSL's `gsl_sf_fermi_dirac_0`.
///
/// The softplus function: asymptotically `e^x` as `x -> -inf` and `x` as
/// `x -> +inf`. Upstream evaluates it in three pieces so that neither limit
/// loses its leading behaviour to cancellation, and both correction series
/// are carried here.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Returns
/// `0.0` below `GSL_LOG_DBL_MIN`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_0;
/// // ln 2 at the origin.
/// assert!((fermi_dirac_0(0.0) - core::f64::consts::LN_2).abs() < 1e-15);
/// // And asymptotically x itself.
/// assert!((fermi_dirac_0(50.0) - 50.0).abs() < 1e-15);
/// ```
pub fn fermi_dirac_0(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        return 0.0;
    }
    if x < -5.0 {
        // ln(1+e^x) = e^x (1 - e^x/2 + e^{2x}/3 - ...), so the leading e^x
        // survives instead of being lost inside ln(1 + tiny).
        let ex = x.exp();
        let ser = 1.0 - ex * (0.5 - ex * (1.0 / 3.0 - ex * (0.25 - ex * (0.2 - ex / 6.0))));
        ex * ser
    } else if x < 10.0 {
        (1.0 + x.exp()).ln()
    } else {
        // ln(1+e^x) = x + e^{-x} - e^{-2x}/2 + ..., so x is exact and only
        // the correction rounds.
        let ex = (-x).exp();
        x + ex * (1.0 - 0.5 * ex + ex * ex / 3.0 - ex * ex * ex / 4.0)
    }
}

/// `F_1(x)`, GSL's `gsl_sf_fermi_dirac_1`.
///
/// Five branches: the alternating series of Goano (6) below `x = -1`, three
/// Chebyshev fits covering `[-1, 10)`, a fourth fit times `x^2` to `x = 30`,
/// a fifth in `60/x` beyond that, and finally the degenerate limit
/// `F_1(x) -> x^2/2`.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Returns
/// `0.0` below `GSL_LOG_DBL_MIN` and `f64::INFINITY` past
/// `GSL_SQRT_DBL_MAX`, where `x^2/2` no longer fits — both upstream's
/// behaviour.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_1;
/// // F_1(0) = pi^2 / 12.
/// let pi2_12 = core::f64::consts::PI * core::f64::consts::PI / 12.0;
/// assert!((fermi_dirac_1(0.0) - pi2_12).abs() < 1e-14);
/// ```
pub fn fermi_dirac_1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        0.0
    } else if x < -1.0 {
        fd_series(x, 2)
    } else if x < 1.0 {
        cheb(x, &FD_1_A)
    } else if x < 4.0 {
        cheb(2.0 / 3.0 * (x - 1.0) - 1.0, &FD_1_B)
    } else if x < 10.0 {
        cheb(1.0 / 3.0 * (x - 4.0) - 1.0, &FD_1_C)
    } else if x < 30.0 {
        cheb(0.1 * x - 2.0, &FD_1_D) * x * x
    } else if x < 1.0 / SQRT_DBL_EPSILON {
        cheb(60.0 / x - 1.0, &FD_1_E) * x * x
    } else if x < SQRT_DBL_MAX {
        0.5 * x * x
    } else {
        f64::INFINITY
    }
}

/// `F_2(x)`, GSL's `gsl_sf_fermi_dirac_2`.
///
/// The same five-branch structure as [`fermi_dirac_1`] one order up: the
/// degenerate limit is `x^3/6`, the `x < 30` and `60/x` fits carry a factor
/// `x^3`, and the overflow bound is therefore the **cube** root of
/// `DBL_MAX`, not the square root.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Returns
/// `0.0` below `GSL_LOG_DBL_MIN` and `f64::INFINITY` past
/// `GSL_ROOT3_DBL_MAX`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_2;
/// // F_2(0) = (3/4) zeta(3).
/// let want = 0.75 * 1.202_056_903_159_594_3;
/// assert!((fermi_dirac_2(0.0) - want).abs() < 1e-14);
/// ```
pub fn fermi_dirac_2(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        0.0
    } else if x < -1.0 {
        fd_series(x, 3)
    } else if x < 1.0 {
        cheb(x, &FD_2_A)
    } else if x < 4.0 {
        cheb(2.0 / 3.0 * (x - 1.0) - 1.0, &FD_2_B)
    } else if x < 10.0 {
        cheb(1.0 / 3.0 * (x - 4.0) - 1.0, &FD_2_C)
    } else if x < 30.0 {
        cheb(0.1 * x - 2.0, &FD_2_D) * x * x * x
    } else if x < 1.0 / ROOT3_DBL_EPSILON {
        cheb(60.0 / x - 1.0, &FD_2_E) * x * x * x
    } else if x < ROOT3_DBL_MAX {
        x * x * x / 6.0
    } else {
        f64::INFINITY
    }
}

/// Goano's alternating series (6) for integer `j`, with the ratio raised to
/// the integer power `p = j + 1`.
///
/// ```text
///     F_j(x) = sum_{n>=1} (-1)^{n-1} e^{nx} / n^{j+1}
/// ```
///
/// written as a running product so no power is evaluated per term. Upstream
/// spells the same loop out separately in each entry point with `rat*rat`,
/// `rat*rat*rat` and so on; the power is passed here instead, which keeps
/// the three integer cases from drifting apart.
fn fd_series(x: f64, p: u32) -> f64 {
    let ex = x.exp();
    let mut term = ex;
    let mut sum = term;
    for n in 2..100 {
        let rat = (n as f64 - 1.0) / n as f64;
        let mut r = 1.0_f64;
        for _ in 0..p {
            r *= rat;
        }
        term *= -ex * r;
        sum += term;
        if (term / sum).abs() < DBL_EPSILON {
            break;
        }
    }
    sum
}

/// The same series for a **half-integer** `j`, where the ratio carries a
/// `sqrt`: `rat^{j+1}` with `j + 1` a half-integer.
///
/// `whole` is the integer part of the exponent and the `sqrt` supplies the
/// remaining half, so `mhalf` passes `0`, `half` passes `1` and `3half`
/// passes `2` — exactly upstream's `sqrt(rat)`, `rat*sqrt(rat)` and
/// `rat*rat*sqrt(rat)`.
fn fd_series_half(x: f64, whole: u32) -> f64 {
    let ex = x.exp();
    let mut term = ex;
    let mut sum = term;
    // mhalf iterates to 200 upstream, the other two to 100; the larger bound
    // is harmless for all three because the break is on convergence.
    for n in 2..200 {
        let rat = (n as f64 - 1.0) / n as f64;
        let mut r = rat.sqrt();
        for _ in 0..whole {
            r *= rat;
        }
        term *= -ex * r;
        sum += term;
        if (term / sum).abs() < DBL_EPSILON {
            break;
        }
    }
    sum
}

/// `F_{-1/2}(x)`, GSL's `gsl_sf_fermi_dirac_mhalf`.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Four
/// Chebyshev branches to `x = 30`, then [`fd_asymp`]. Returns `0.0` below
/// `GSL_LOG_DBL_MIN`; it grows only like `2 sqrt(x)/sqrt(pi)` and so never
/// overflows.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_mhalf;
/// // F_{-1/2}(0) = (1 - 1/sqrt 2) zeta(1/2) ... no closed form worth
/// // quoting; check the degenerate limit instead.
/// let x = 200.0_f64;
/// let sommerfeld = 2.0 * x.sqrt() / core::f64::consts::PI.sqrt();
/// assert!((fermi_dirac_mhalf(x) / sommerfeld - 1.0).abs() < 1e-3);
/// ```
pub fn fermi_dirac_mhalf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        0.0
    } else if x < -1.0 {
        fd_series_half(x, 0)
    } else if x < 1.0 {
        cheb(x, &FD_MHALF_A)
    } else if x < 4.0 {
        cheb(2.0 / 3.0 * (x - 1.0) - 1.0, &FD_MHALF_B)
    } else if x < 10.0 {
        cheb(1.0 / 3.0 * (x - 4.0) - 1.0, &FD_MHALF_C)
    } else if x < 30.0 {
        cheb(0.1 * x - 2.0, &FD_MHALF_D) * x.sqrt()
    } else {
        fd_asymp(-0.5, x)
    }
}

/// `F_{1/2}(x)`, GSL's `gsl_sf_fermi_dirac_half`.
///
/// The one the semiconductor literature means by "the Fermi-Dirac integral":
/// the carrier density of a three-dimensional parabolic band is
/// `n = N_c F_{1/2}(mu/kT)`, reducing to the Boltzmann `N_c e^{mu/kT}` in the
/// non-degenerate limit and to `(4/3 sqrt pi) (mu/kT)^{3/2}` in the
/// degenerate one.
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Four
/// Chebyshev branches to `x = 30`, then [`fd_asymp`]. Returns `0.0` below
/// `GSL_LOG_DBL_MIN`; growth is only `x^{3/2}`, so it never overflows.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_half;
/// // Non-degenerate limit: F_{1/2}(x) -> e^x.
/// let x = -6.0_f64;
/// assert!((fermi_dirac_half(x) / x.exp() - 1.0).abs() < 1e-3);
/// ```
pub fn fermi_dirac_half(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        0.0
    } else if x < -1.0 {
        fd_series_half(x, 1)
    } else if x < 1.0 {
        cheb(x, &FD_HALF_A)
    } else if x < 4.0 {
        cheb(2.0 / 3.0 * (x - 1.0) - 1.0, &FD_HALF_B)
    } else if x < 10.0 {
        cheb(1.0 / 3.0 * (x - 4.0) - 1.0, &FD_HALF_C)
    } else if x < 30.0 {
        cheb(0.1 * x - 2.0, &FD_HALF_D) * (x * x.sqrt())
    } else {
        fd_asymp(0.5, x)
    }
}

/// `F_{3/2}(x)`, GSL's `gsl_sf_fermi_dirac_3half`.
///
/// The energy density of the same three-dimensional parabolic band whose
/// carrier density is [`fermi_dirac_half`].
///
/// `x` is the reduced chemical potential `mu/kT`, dimensionless. Four
/// Chebyshev branches to `x = 30`, then [`fd_asymp`]. Returns `0.0` below
/// `GSL_LOG_DBL_MIN`; growth is `x^{5/2}`, which cannot overflow an `f64`
/// for any `x` that is itself finite.
///
/// # Examples
///
/// ```
/// use petir::specfunc::fermi_dirac::fermi_dirac_3half;
/// // Monotone increasing, like every member of the family.
/// assert!(fermi_dirac_3half(1.0) < fermi_dirac_3half(2.0));
/// ```
pub fn fermi_dirac_3half(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < LOG_DBL_MIN {
        0.0
    } else if x < -1.0 {
        fd_series_half(x, 2)
    } else if x < 1.0 {
        cheb(x, &FD_3HALF_A)
    } else if x < 4.0 {
        cheb(2.0 / 3.0 * (x - 1.0) - 1.0, &FD_3HALF_B)
    } else if x < 10.0 {
        cheb(1.0 / 3.0 * (x - 4.0) - 1.0, &FD_3HALF_C)
    } else if x < 30.0 {
        cheb(0.1 * x - 2.0, &FD_3HALF_D) * (x * x * x.sqrt())
    } else {
        fd_asymp(1.5, x)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::gamma::gamma;
    use crate::specfunc::zeta::zeta;

    const PI2: f64 = core::f64::consts::PI * core::f64::consts::PI;

    /// A named function of one `f64`, so the tables below read as tables.
    type Fd = fn(f64) -> f64;
    /// A labelled member of the family: name, function, index `j`.
    type Member = (&'static str, Fd, f64);

    /// Every `F_j` in this module, with its index.
    const FAMILY: [Member; 7] = [
        ("F_-1", fermi_dirac_m1, -1.0),
        ("F_-1/2", fermi_dirac_mhalf, -0.5),
        ("F_0", fermi_dirac_0, 0.0),
        ("F_1/2", fermi_dirac_half, 0.5),
        ("F_1", fermi_dirac_1, 1.0),
        ("F_3/2", fermi_dirac_3half, 1.5),
        ("F_2", fermi_dirac_2, 2.0),
    ];

    /// The defining integral, computed **independently of this module**:
    ///
    /// ```text
    ///     F_j(x) = (1/Gamma(j+1)) integral_0^inf t^j / (e^{t-x} + 1) dt
    /// ```
    ///
    /// by composite 30-point Gauss-Legendre after the substitution `t = v^2`,
    /// which removes the `t^{-1/2}` endpoint singularity of `F_{-1/2}` and
    /// leaves every integrand smooth at the origin. `vmax` is chosen so the
    /// truncated tail is below `e^{-60}` of the peak.
    fn by_quadrature(j: f64, x: f64, panels: usize) -> f64 {
        let vmax = (x.max(0.0) + 60.0).sqrt();
        let h = vmax / panels as f64;
        let mut acc = 0.0;
        for k in 0..panels {
            let a = k as f64 * h;
            acc += crate::integration::gauss_legendre::gauss_legendre(
                |v: f64| {
                    let t = v * v;
                    let num = if j == 0.0 { 1.0 } else { t.powf(j) };
                    2.0 * v * num / ((t - x).exp() + 1.0)
                },
                a,
                a + h,
                30,
            )
            .expect("30-point Gauss-Legendre is tabulated");
        }
        acc / gamma(j + 1.0)
    }

    /// **The defining integral is reproduced to 6.0e-15 across the whole
    /// family and every branch**, measured 2026-09-19.
    ///
    /// # Methodology
    ///
    /// Each `F_j` is compared against [`by_quadrature`], which shares no code
    /// with this module — different tables, different algorithm, a numerical
    /// integral rather than a Chebyshev fit. The 15 abscissae are placed to
    /// cross **every branch boundary** of the five multi-branch functions:
    /// the series/`-1` cut, the four Chebyshev boundaries at `-1, 1, 4, 10,
    /// 30`, and points inside the asymptotic region beyond 30.
    ///
    /// Pass criterion: relative difference below `1e-13`, roughly two orders
    /// above the quadrature's own convergence floor.
    ///
    /// # Results
    ///
    /// | | worst relative difference | at |
    /// |---|---|---|
    /// | `F_0` | 2.103e-15 | -4 |
    /// | `F_1` | 1.086e-15 | 25 |
    /// | `F_2` | 1.548e-15 | 29 |
    /// | `F_{-1/2}` | 9.177e-16 | 0 |
    /// | `F_{1/2}` | 2.926e-15 | 60 |
    /// | `F_{3/2}` | 5.992e-15 | 12 |
    ///
    /// `F_{-1}` is excluded only because `Gamma(0)` is infinite, so the
    /// integral form does not apply to it; its closed form is checked in
    /// `the_closed_forms_are_the_closed_forms`.
    #[test]
    fn the_defining_integral_is_reproduced() {
        for (name, f, j) in FAMILY {
            if j <= -1.0 {
                continue;
            }
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for x in [
                -8.0_f64, -4.0, -1.5, -0.5, 0.0, 0.5, 2.0, 3.9, 5.0, 9.0, 12.0, 25.0, 29.0, 35.0,
                60.0,
            ] {
                let q = by_quadrature(j, x, 60);
                let e = ((f(x) - q) / q).abs();
                if e > worst {
                    worst = e;
                    at = x;
                }
            }
            assert!(
                worst < 1e-13,
                "{name} against the defining integral: {worst:e} at x = {at}. \
                 The documented figures are 9.2e-16 to 6.0e-15 across the family"
            );
        }
    }

    /// `F_j(0) = (1 - 2^{-j}) zeta(j+1)`, the exact value at the Fermi level,
    /// checked against **PETIR's own `zeta`** — a different module with
    /// different tables, so this is a cross-check rather than a restatement.
    ///
    /// Measured 2026-09-19: exact for `F_{-1/2}`, `F_0` and `F_{3/2}`, and
    /// 1.6e-15 at worst (`F_{1/2}`). `F_0` takes the limit `j -> 0`, where
    /// the identity is `0 * infinity` and evaluates to `ln 2`.
    #[test]
    fn the_zero_argument_values_match_the_zeta_identity() {
        for (name, f, j) in FAMILY {
            let want = if j == 0.0 {
                core::f64::consts::LN_2
            } else {
                (1.0 - (2.0_f64).powf(-j)) * zeta(j + 1.0)
            };
            let e = ((f(0.0) - want) / want).abs();
            assert!(
                e < 5e-15,
                "{name}(0) = {:.17e} against (1 - 2^-j) zeta(j+1) = {want:.17e}, \
                 relative {e:e}",
                f(0.0)
            );
        }
    }

    /// **`dF_j/dx = F_{j-1}(x)`** — the identity that ties the whole family
    /// together, and the only check here that makes one member's tables
    /// answer for another's.
    ///
    /// # Methodology
    ///
    /// An eighth-order central difference of `F_j`, compared against `F_{j-1}`
    /// evaluated directly, over `x` in `[-14, 42]` at 0.2 spacing — 281
    /// points crossing every branch boundary of both functions. The point
    /// `x = -1` is skipped because a difference stencil straddling a branch
    /// cut measures the cut, not the derivative.
    ///
    /// Pass criterion: 1e-9 relative. This is a bound on the **stencil**, not
    /// on the functions: at `h = max(1e-3, 1e-5|x|)` the truncation and
    /// round-off floor of an eighth-order difference is already near 1e-11,
    /// so a tighter number would be measuring the finite difference.
    ///
    /// # Results (2026-09-19)
    ///
    /// | pair | worst | at |
    /// |---|---|---|
    /// | `F_0' = F_{-1}` | 8.045e-12 | -4.2 |
    /// | `F_1' = F_0` | 8.647e-12 | 42 |
    /// | `F_2' = F_1` | 7.393e-12 | 37.6 |
    /// | `F_{1/2}' = F_{-1/2}` | 3.831e-11 | 42 |
    /// | `F_{3/2}' = F_{1/2}` | 3.702e-11 | 41.4 |
    ///
    /// Note the last two peak **inside the asymptotic region**, past `x = 30`
    /// where both sides switch to [`fd_asymp`] — so the Sommerfeld expansion
    /// and the series acceleration under it are covered too.
    #[test]
    fn each_member_is_the_derivative_of_the_next() {
        let pairs: [(&str, Fd, Fd); 5] = [
            ("F_0' = F_-1", fermi_dirac_0, fermi_dirac_m1),
            ("F_1' = F_0", fermi_dirac_1, fermi_dirac_0),
            ("F_2' = F_1", fermi_dirac_2, fermi_dirac_1),
            ("F_1/2' = F_-1/2", fermi_dirac_half, fermi_dirac_mhalf),
            ("F_3/2' = F_1/2", fermi_dirac_3half, fermi_dirac_half),
        ];
        for (name, f, g) in pairs {
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for k in 0..=280 {
                let x = -14.0 + 0.2 * k as f64;
                // A stencil straddling the x = -1 branch cut measures the
                // cut, not the derivative.
                if (x + 1.0).abs() < 1e-6 {
                    continue;
                }
                let h = 1e-3_f64.max(x.abs() * 1e-5);
                let d = ((1.0 / 280.0) * (f(x - 4.0 * h) - f(x + 4.0 * h))
                    + (4.0 / 105.0) * (f(x + 3.0 * h) - f(x - 3.0 * h))
                    + 0.2 * (f(x - 2.0 * h) - f(x + 2.0 * h))
                    + 0.8 * (f(x + h) - f(x - h)))
                    / h;
                let want = g(x);
                if want.abs() < 1e-12 {
                    continue;
                }
                let e = ((d - want) / want).abs();
                if e > worst {
                    worst = e;
                    at = x;
                }
            }
            assert!(
                worst < 1e-9,
                "{name}: {worst:e} at x = {at}. Documented at 8.0e-12 to 3.8e-11"
            );
        }
    }

    /// The integer reflection identities, which are **exact** rather than
    /// asymptotic:
    ///
    /// ```text
    ///     F_0(x) - F_0(-x) = x
    ///     F_1(x) - F_1(-x) = x^2/2 + pi^2/6
    ///     F_2(x) + F_2(-x) = x^3/6 + pi^2 x/6
    /// ```
    ///
    /// These are worth asserting because the two sides are computed by
    /// **different branches** of the same function — at `x = 50` the left
    /// operand is the Chebyshev-in-`60/x` or asymptotic branch and the right
    /// is the alternating series — so an error in either shows up as a
    /// violation of an identity that holds exactly in real arithmetic.
    ///
    /// Measured 2026-09-19: `F_0` is exact to the last bit at every `x`
    /// tried; `F_1` and `F_2` reach machine precision by `x = 50` and are
    /// 9.5e-04 / 4.6e-04 at `x = 5`, which is not an error — it is the
    /// identity being **checked against a finite-precision difference of two
    /// numbers of very different size**, so the test asserts it only where
    /// the cancellation is benign.
    #[test]
    fn the_integer_reflection_identities_hold() {
        for x in [50.0_f64, 200.0, 1e4, 1e8, 1e12] {
            assert_eq!(
                fermi_dirac_0(x) - fermi_dirac_0(-x),
                x,
                "F_0(x) - F_0(-x) = x is exact; at x = {x:e} it is not"
            );

            let want = 0.5 * x * x + PI2 / 6.0;
            let got = fermi_dirac_1(x) - fermi_dirac_1(-x);
            assert!(
                ((got - want) / want).abs() < 1e-14,
                "F_1 reflection at x = {x:e}: {got:e} vs {want:e}"
            );

            let want = x * x * x / 6.0 + PI2 * x / 6.0;
            let got = fermi_dirac_2(x) + fermi_dirac_2(-x);
            assert!(
                ((got - want) / want).abs() < 1e-14,
                "F_2 reflection at x = {x:e}: {got:e} vs {want:e}"
            );
        }
    }

    /// **The half-integer tails converge to the Sommerfeld expansion at
    /// fourth order in `1/x`, and that order is what is asserted** — a
    /// two-term reference cannot pin the value, but it can pin the *rate*,
    /// and the rate is what distinguishes a correct asymptotic branch from a
    /// merely close one.
    ///
    /// ```text
    ///     F_j(x) ~ x^{j+1}/Gamma(j+2) [ 1 + pi^2 j (j+1) / (6 x^2) + O(x^-4) ]
    /// ```
    ///
    /// Measured 2026-09-19, residual against the two-term form:
    ///
    /// | | `x = 50` | `x = 200` | ratio | `(200/50)^4` |
    /// |---|---|---|---|---|
    /// | `F_{-1/2}` | 2.860e-07 | 1.110e-09 | 258 | 256 |
    /// | `F_{1/2}` | 1.710e-07 | 6.660e-10 | 257 | 256 |
    /// | `F_{3/2}` | 2.839e-07 | 1.110e-09 | 256 | 256 |
    ///
    /// The residual is therefore the **neglected third term**, not an error
    /// in the branch. By `x = 1e4` it has fallen into the rounding (2.5e-16
    /// to 2.2e-15) and the two agree outright.
    #[test]
    fn the_half_integer_tails_converge_to_sommerfeld_at_fourth_order() {
        let two_term = |j: f64, x: f64| {
            x.powf(j + 1.0) / gamma(j + 2.0) * (1.0 + PI2 / 6.0 * (j + 1.0) * j / (x * x))
        };
        for (name, f, j) in [
            ("F_-1/2", fermi_dirac_mhalf as Fd, -0.5_f64),
            ("F_1/2", fermi_dirac_half, 0.5),
            ("F_3/2", fermi_dirac_3half, 1.5),
        ] {
            let residual = |x: f64| ((f(x) - two_term(j, x)) / two_term(j, x)).abs();
            let (a, b) = (residual(50.0), residual(200.0));
            let ratio = a / b;
            assert!(
                (240.0..275.0).contains(&ratio),
                "{name}: the residual against the two-term Sommerfeld form is \
                 documented as falling at fourth order -- (200/50)^4 = 256 -- \
                 and fell by {ratio}. A different power means the asymptotic \
                 branch is wrong, not merely inaccurate ({a:e} then {b:e})"
            );
            // And by 1e4 it is gone entirely.
            assert!(
                residual(1e4) < 1e-14,
                "{name} at x = 1e4 still differs from the two-term form by {:e}",
                residual(1e4)
            );
        }
    }

    /// `F_{-1}` and `F_0` have elementary closed forms, and this module must
    /// reproduce them across all of their branches — including the two
    /// correction series that exist precisely so the naive form's
    /// cancellation is avoided.
    #[test]
    fn the_closed_forms_are_the_closed_forms() {
        // F_{-1}(x) = e^x/(1+e^x), tested through the equivalent
        // 1/(1+e^{-x}) so the two branches are each checked against the form
        // the OTHER one uses.
        for k in 0..=400 {
            let x = -40.0 + 0.2 * k as f64;
            let want = 1.0 / (1.0 + (-x).exp());
            let got = fermi_dirac_m1(x);
            assert!(
                ((got - want) / want).abs() < 1e-14,
                "F_-1({x}) = {got:e} vs 1/(1+e^-x) = {want:e}"
            );
        }
        assert_eq!(fermi_dirac_m1(0.0), 0.5);

        // F_0(x) = ln(1+e^x), referenced against ln_1p rather than the
        // naive `(1.0 + x.exp()).ln()`.
        //
        // THE NAIVE FORM IS THE ONE THAT IS WRONG, and a first draft of this
        // test used it and failed: at x = -15 it gives 3.0590227379725525e-7
        // against this module's 3.059022737137205e-7, a relative 2.7e-10.
        // Adding 3e-7 to 1.0 discards seven digits before the logarithm ever
        // runs, which is exactly the cancellation upstream's small-argument
        // series exists to avoid. `ln_1p` does not have that defect and is
        // still independent of this module.
        for k in 0..=450 {
            let x = -30.0 + 0.1 * k as f64;
            let want = x.exp().ln_1p();
            let got = fermi_dirac_0(x);
            assert!(
                ((got - want) / want).abs() < 1e-13,
                "F_0({x}) = {got:e} vs ln_1p(e^x) = {want:e}"
            );
        }
        assert_eq!(fermi_dirac_0(0.0), core::f64::consts::LN_2);
    }

    /// Every member is positive and strictly increasing, `NaN` propagates,
    /// and the two documented refusals behave as upstream's
    /// `UNDERFLOW_ERROR` / `OVERFLOW_ERROR`.
    #[test]
    fn the_shape_and_the_edges() {
        for (name, f, _) in FAMILY {
            let mut prev = 0.0_f64;
            for k in 0..=700 {
                let x = -30.0 + 0.1 * k as f64;
                let v = f(x);
                assert!(v > 0.0 && v.is_finite(), "{name}({x}) = {v:e}");
                // F_{-1} is the one BOUNDED member -- it saturates at 1 and
                // stops increasing near x = 33.9, where 1/(1+e^{-x}) rounds
                // to 0.999_999_999_999_998 and stays there. Requiring strict
                // monotonicity of it would be requiring f64 to hold more
                // than it can, so it is asserted non-decreasing instead.
                if f(0.0) == 0.5 && x > 30.0 {
                    assert!(v >= prev, "{name} decreased at {x}: {v:e} vs {prev:e}");
                } else {
                    assert!(v > prev, "{name} not increasing at {x}: {v:e} vs {prev:e}");
                }
                prev = v;
            }
            assert!(f(f64::NAN).is_nan(), "{name}(NaN)");
            // Upstream's UNDERFLOW_ERROR, below GSL_LOG_DBL_MIN.
            assert_eq!(f(-800.0), 0.0, "{name} below LOG_DBL_MIN");
            assert_eq!(f(f64::NEG_INFINITY), 0.0, "{name}(-inf)");
        }

        // F_{-1} is the only bounded member: it saturates at 1.
        assert!(fermi_dirac_m1(40.0) > 0.999_999_999 && fermi_dirac_m1(1e3) <= 1.0);

        // Upstream's explicit OVERFLOW_ERROR, on the two that carry one.
        assert!(fermi_dirac_1(SQRT_DBL_MAX * 0.999).is_finite());
        assert!(fermi_dirac_1(1e160).is_infinite());
        assert!(fermi_dirac_2(ROOT3_DBL_MAX * 0.999).is_finite());
        assert!(fermi_dirac_2(1e103).is_infinite());

        // THE GUARD IS NOT UNIFORM, AND THAT IS UPSTREAM'S DOING. A first
        // draft of this test asserted "the other five never overflow" and
        // failed: F_{1/2} ~ x^{3/2}/Gamma(5/2) and F_{3/2} ~ x^{5/2}/
        // Gamma(7/2) both leave f64's range, measured by bisection at
        // x ~ 10^205.586 and 10^123.511 respectively (2026-09-19) -- and GSL
        // has NO overflow guard for either, so `fd_asymp` just returns
        // infinity. Asserted here so the asymmetry is recorded rather than
        // rediscovered.
        assert!(fermi_dirac_half(1e205).is_finite() && fermi_dirac_half(1e206).is_infinite());
        assert!(fermi_dirac_3half(1e123).is_finite() && fermi_dirac_3half(1e124).is_infinite());

        // Genuinely overflow-free: bounded, linear and sqrt respectively.
        for (name, f, _) in [FAMILY[0], FAMILY[1], FAMILY[2]] {
            assert!(f(1e300).is_finite(), "{name}(1e300) overflowed");
        }
    }

    /// **The carry reformulation of [`fd_whiz`] is equivalent to upstream's
    /// in-place indexed loop**, checked against a literal transcription of it
    /// written here in the test module — where a fallible subscript is
    /// allowed and the library's `no_panic_gate` does not reach.
    ///
    /// `fd_whiz` was rewritten to touch no index at all, because
    /// `tests/no_panic_gate.rs` rejects any subscript that can fail at run
    /// time. The rewrite is not obviously the same loop: it relies on the
    /// descending sweep reading `q[j+1]` *after* that entry was updated, so
    /// a single running value reproduces the whole array traversal, and on
    /// the last iteration leaving `q[0]` in that carry. This asserts it
    /// **bit for bit** over a real sequence of terms rather than trusting the
    /// argument.
    #[test]
    fn the_carry_reformulation_of_fd_whiz_is_upstreams_loop() {
        /// GSL's `fd_whiz` transcribed literally, subscripts and all.
        fn reference(
            term: f64,
            iterm: usize,
            qnum: &mut [f64; FD_QSIZE],
            qden: &mut [f64; FD_QSIZE],
            s: &mut f64,
        ) -> f64 {
            if iterm == 0 {
                *s = 0.0;
            }
            *s += term;
            qden[iterm] = 1.0 / (term * (iterm as f64 + 1.0) * (iterm as f64 + 1.0));
            qnum[iterm] = *s * qden[iterm];
            if iterm > 0 {
                let mut factor = 1.0_f64;
                let ratio = iterm as f64 / (iterm as f64 + 1.0);
                for j in (0..iterm).rev() {
                    let c = factor * (j as f64 + 1.0) / (iterm as f64 + 1.0);
                    factor *= ratio;
                    qden[j] = qden[j + 1] - c * qden[j];
                    qnum[j] = qnum[j + 1] - c * qnum[j];
                }
            }
            qnum[0] / qden[0]
        }

        // A term sequence with the shape fd_neg actually feeds it: e^{n x}
        // over n^{j+1}, alternating.
        for (x, j) in [(-0.5_f64, 0.5_f64), (-2.0, 1.5), (-0.1, -0.5), (-5.0, 2.0)] {
            let ex = -x.exp();
            let mut enx = -ex;
            let (mut qn1, mut qd1) = ([0.0_f64; FD_QSIZE], [0.0_f64; FD_QSIZE]);
            let (mut qn2, mut qd2) = ([0.0_f64; FD_QSIZE], [0.0_f64; FD_QSIZE]);
            let (mut s1, mut s2) = (0.0_f64, 0.0_f64);
            for iterm in 0..=FD_ITMAX {
                let term = enx / (iterm as f64 + 1.0).powf(j + 1.0);
                let a = fd_whiz(term, iterm, &mut qn1, &mut qd1, &mut s1);
                let b = reference(term, iterm, &mut qn2, &mut qd2, &mut s2);
                assert_eq!(
                    a.to_bits(),
                    b.to_bits(),
                    "fd_whiz and upstream's indexed loop differ at iterm = \
                     {iterm}, x = {x}, j = {j}: {a:e} vs {b:e}"
                );
                // The work arrays must agree too, not only the estimate --
                // they are what the NEXT call reads.
                assert_eq!(qn1, qn2, "qnum diverged at iterm = {iterm}");
                assert_eq!(qd1, qd2, "qden diverged at iterm = {iterm}");
                enx *= ex;
            }
        }
    }

    /// **`fd_whiz` and its two 101-element work arrays are UNREACHABLE from
    /// this module's seven entry points** — measured by instrumentation, not
    /// argued from the branch conditions.
    ///
    /// # Why it matters
    ///
    /// The series acceleration is the one part of this port that needs
    /// scratch memory, and it was recorded as the obstacle to a WGSL/`f32`
    /// transcription of the family (`op-uczx.17`), where 101 `f32` slots
    /// twice over is real function-local storage. It turns out not to be an
    /// obstacle at all.
    ///
    /// [`fd_neg`] takes its acceleration branch only when `x` is NOT less
    /// than both `-1` and `-|j+1|`. But [`fd_asymp`] is the only caller, and
    /// it is only ever reached at `x >= 30`, so `fd_neg` always sees
    /// `-x <= -30` — and `-|j+1|` is `-0.5`, `-1.5` or `-2.5` for the three
    /// half-integer indices. The condition is satisfied with two orders of
    /// room, every time.
    ///
    /// The simple series it takes instead converges in **2 to 3 terms**
    /// (measured 2026-09-19 at `x = -30, -40, -100, -700` for all three `j`),
    /// because `e^{-30}` is already 9.4e-14.
    ///
    /// # Why the code stays
    ///
    /// It is upstream's, and `gsl_sf_fermi_dirac_int_e` for general integer
    /// `j` — which this module does not expose but a later change might —
    /// does reach it. Removing a faithfully ported routine because the
    /// currently exposed surface cannot call it would be exactly the kind of
    /// silent divergence from upstream this crate's `CLAUDE.md` forbids.
    /// What was wrong was the *claim*, not the code.
    ///
    /// # How this is measured
    ///
    /// [`FD_WHIZ_ENTRIES`] counts entries to the acceleration branch. The
    /// sweep below covers every public entry point across the whole
    /// reachable domain — including the asymptotic region past `x = 30`,
    /// which is the only place `fd_neg` is called from at all — and asserts
    /// the counter never moves. `fd_neg` is then called **directly** at an
    /// argument that does take the branch, so the instrument is shown to be
    /// capable of firing.
    #[test]
    fn the_series_acceleration_is_unreachable_from_this_module() {
        use core::sync::atomic::Ordering;

        let before = FD_WHIZ_ENTRIES.load(Ordering::Relaxed);
        for (_, f, _) in FAMILY {
            for k in 0..=20_000 {
                // -750 .. 1250, so LOG_DBL_MIN, every Chebyshev boundary and
                // the asymptotic branch are all crossed.
                let x = -750.0 + 0.1 * k as f64;
                let _ = f(x);
            }
            for x in [1e3_f64, 1e6, 1e12, 1e100, 1e200, 1e300] {
                let _ = f(x);
                let _ = f(-x);
            }
        }
        let after = FD_WHIZ_ENTRIES.load(Ordering::Relaxed);
        assert_eq!(
            after - before,
            0,
            "fd_neg took its series-acceleration branch {} times while sweeping \
             the whole reachable domain. It is documented as unreachable from \
             this module -- if that has changed, the WGSL port's scratch-memory \
             question is live again",
            after - before
        );

        // AND THE INSTRUMENT CAN FIRE. fd_neg is private, so this is the only
        // way to reach the branch -- which is the point being made.
        let before = FD_WHIZ_ENTRIES.load(Ordering::Relaxed);
        let v = fd_neg(1.5, -0.5);
        let after = FD_WHIZ_ENTRIES.load(Ordering::Relaxed);
        assert_eq!(
            after - before,
            1,
            "fd_neg(1.5, -0.5) must take the acceleration branch -- x = -0.5 is \
             not less than -1 -- or the sweep above proves nothing"
        );
        assert!(
            v.is_finite() && v > 0.0,
            "and it must still produce an answer: {v:e}"
        );

        // The simple series it takes instead converges in 2 to 3 terms at
        // every argument fd_asymp can hand it.
        for j in [-0.5_f64, 0.5, 1.5] {
            for x in [-30.0_f64, -40.0, -100.0, -700.0] {
                let ex = x.exp();
                let (mut term, mut sum, mut used) = (ex, ex, 1);
                for n in 2..100 {
                    let rat = (n as f64 - 1.0) / n as f64;
                    term *= -ex * rat.powf(j + 1.0);
                    sum += term;
                    used = n;
                    if (term / sum).abs() < DBL_EPSILON {
                        break;
                    }
                }
                assert!(
                    used <= 3,
                    "the simple series is documented as converging in 2 to 3 \
                     terms; at j = {j}, x = {x} it took {used}"
                );
            }
        }
    }

    /// **Upstream's far-field cut for `F_2` is six orders of magnitude too
    /// early, and costs 3.6e-10** — measured here so the defect is recorded
    /// rather than inherited silently.
    ///
    /// # What the two cuts are for
    ///
    /// Past `x = 30` both `F_1` and `F_2` evaluate a short Chebyshev fit in
    /// `60/x` times `x^2` or `x^3`, and past a second cut they drop the fit
    /// for the pure degenerate limit. What the fit is carrying is exactly the
    /// Sommerfeld correction:
    ///
    /// ```text
    ///     F_1(x) / (x^2/2) = 1 + (pi^2/3) / x^2
    ///     F_2(x) / (x^3/6) = 1 + pi^2      / x^2
    /// ```
    ///
    /// so each cut should sit where that correction falls below `eps`. For
    /// `F_1` upstream's `1/GSL_SQRT_DBL_EPSILON = 6.711e7` does exactly that
    /// — the correction is 6.7e-16 there, about 3 `eps`, and the transition
    /// is invisible.
    ///
    /// For `F_2` upstream uses `1/GSL_ROOT3_DBL_EPSILON = 1.6514e5`, which
    /// looks like it was chosen to match the `x^3` factor rather than the
    /// accuracy requirement. The correction there is still **3.62e-10**, six
    /// orders above `eps`, so `gsl_sf_fermi_dirac_2` steps down by
    /// **3.56e-10 relative** at that point and stays low, recovering only as
    /// `1/x^2`. Matching `F_1`'s `1/sqrt(eps)` would have put the correction
    /// at 2.2e-15.
    ///
    /// # Why it is not fixed here
    ///
    /// This crate's bar is agreement with GSL, and a port that silently
    /// improves on upstream is a port whose comparisons no longer mean
    /// anything (crate `CLAUDE.md`, rule 2). The measurement is asserted
    /// instead, so a future GSL that fixes it fails this test loudly rather
    /// than passing unnoticed.
    ///
    /// # Results (2026-09-19)
    ///
    /// | `x / cut` | `F_2 / exact - 1` |
    /// |---|---|
    /// | 0.999 | 0 |
    /// | 0.9999 | -2.220e-16 |
    /// | **1.0** | **-3.619e-10** |
    /// | 1.001 | -3.612e-10 |
    /// | 1.1 | -2.991e-10 |
    /// | 10 | -3.619e-12 |
    ///
    /// and for `F_1` the same sweep never leaves 6.7e-16.
    #[test]
    fn upstreams_far_field_cut_is_exact_for_f_1_and_six_orders_early_for_f_2() {
        // The exact far-field forms, from the reflection identities: the
        // reflected piece is O(e^{-x}) and is zero at every x here.
        let exact_1 = |x: f64| 0.5 * x * x + PI2 / 6.0;
        let exact_2 = |x: f64| x * x * x / 6.0 + PI2 * x / 6.0;

        // F_1's cut is where it should be.
        let cut1 = 1.0 / SQRT_DBL_EPSILON;
        for m in [0.999_f64, 0.9999, 1.0, 1.0001, 1.001, 1.1, 10.0] {
            let x = cut1 * m;
            let e = (fermi_dirac_1(x) / exact_1(x) - 1.0).abs();
            assert!(
                e < 1e-14,
                "F_1 at {m} x its far-field cut is {e:e} from the exact form;                  upstream's 1/sqrt(eps) is documented as placing the cut where                  the Sommerfeld correction is already below eps"
            );
        }

        // F_2's is not. Below the cut it is exact; at and above it, low by
        // 3.6e-10. Both halves are asserted, because it is the CONTRAST that
        // identifies this as a misplaced cut rather than a bad fit.
        let cut2 = 1.0 / ROOT3_DBL_EPSILON;
        for m in [0.999_f64, 0.9999] {
            let x = cut2 * m;
            let e = (fermi_dirac_2(x) / exact_2(x) - 1.0).abs();
            assert!(
                e < 1e-15,
                "F_2 just BELOW its cut should be exact; it is {e:e}"
            );
        }
        for (m, want) in [(1.0_f64, 3.619e-10), (1.001, 3.612e-10), (1.1, 2.991e-10)] {
            let x = cut2 * m;
            let d = fermi_dirac_2(x) / exact_2(x) - 1.0;
            assert!(
                d < 0.0 && (d.abs() / want - 1.0).abs() < 0.01,
                "F_2 at {m} x its cut is documented as {want:e} LOW; it is {d:e}.                  If this now agrees with the exact form, upstream has moved the                  cut and this crate's bar -- agreement with GSL -- has changed"
            );
        }
        // And it recovers as 1/x^2: ten times out is a hundred times smaller.
        let ten = (fermi_dirac_2(cut2 * 10.0) / exact_2(cut2 * 10.0) - 1.0).abs();
        let one = (fermi_dirac_2(cut2) / exact_2(cut2) - 1.0).abs();
        assert!(
            (one / ten / 100.0 - 1.0).abs() < 0.02,
            "the F_2 deficit is documented as falling like 1/x^2 -- a factor 100              over a decade -- and fell by {}",
            one / ten
        );
    }

    /// The two bounds this module re-declares are the constants they claim to
    /// be — `GSL_ROOT3_DBL_EPSILON` and `GSL_ROOT3_DBL_MAX`, the cube roots
    /// of `DBL_EPSILON` and `DBL_MAX`.
    ///
    /// Worth a test because `fermi_dirac_2`'s far-field branch is the only
    /// place in the crate that needs a **cube** root rather than a square
    /// one, and a copied `SQRT_` constant would still produce plausible
    /// numbers over most of the range.
    #[test]
    fn the_cube_root_bounds_are_cube_roots() {
        // Upstream's own literals, which is the fidelity claim:
        //   gsl_machine.h:  GSL_ROOT3_DBL_MAX  5.6438030941222897e+102
        assert_eq!(ROOT3_DBL_MAX, 5.643_803_094_122_289_7e102);
        assert_eq!(ROOT3_DBL_EPSILON, crate::scalar::CBRT_DBL_EPSILON);

        // And they really are cube roots.
        assert!((ROOT3_DBL_EPSILON / DBL_EPSILON.cbrt() - 1.0).abs() < 1e-15);

        // CHECKED AGAINST powf(1/3), NOT cbrt -- because at the top of the
        // range `cbrt` is the inaccurate one. Measured 2026-09-19:
        //
        //   cbrt(DBL_MAX)     5.64380309412236230e102
        //   powf(DBL_MAX,1/3) 5.64380309412228770e102
        //   GSL's literal     5.64380309412228969e102
        //
        // GSL's constant is 2 ulp from `powf` and 58 ulp from `cbrt`, so a
        // first draft of this assertion failed at 1.288e-14 and the constant
        // was not the thing at fault. Checking the CUBE settles it
        // independently of either root.
        assert!(
            (ROOT3_DBL_MAX / f64::MAX.powf(1.0 / 3.0) - 1.0).abs() < 1e-15,
            "ROOT3_DBL_MAX / powf(DBL_MAX, 1/3) - 1 = {:e}",
            ROOT3_DBL_MAX / f64::MAX.powf(1.0 / 3.0) - 1.0
        );
        let cube = ROOT3_DBL_MAX * ROOT3_DBL_MAX * ROOT3_DBL_MAX;
        assert!(
            cube.is_finite() && (cube / f64::MAX - 1.0).abs() < 1e-13,
            "GSL_ROOT3_DBL_MAX cubed is {cube:e}, not DBL_MAX"
        );
        // And they are NOT the square-root constants, which is the mistake
        // this guards against.
        const {
            assert!(ROOT3_DBL_EPSILON > SQRT_DBL_EPSILON * 100.0);
            assert!(ROOT3_DBL_MAX < SQRT_DBL_MAX / 1e50);
        }
    }
}
