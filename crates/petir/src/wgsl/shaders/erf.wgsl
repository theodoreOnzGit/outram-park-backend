// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from GSL's `specfunc/erfc.c`
//   Copyright (C) 1996, 1997, 1998, 1999, 2000, 2007 Jason H. Stover,
//   Gerard Jungman, Brian Gough. GPL-3.0-or-later.
//
// Ported entities, each named where it is used below:
//   erfseries            gsl_sf_erf_e's |x| < 1 branch
//   erfc_xlt1_cs         Chebyshev fit for erfc((t+1)/2), -1 < t < 1
//   erfc_x15_cs          Chebyshev fit for erfc(x) exp(x^2), 1 < x < 5
//   erfc_x510_cs         Chebyshev fit for erfc(x) x exp(x^2), 5 < x < 10
//   erfc8_sum / erfc8    Hart et al. index 5725 rational, 8 < x < 100
//   gsl_sf_erfc_e        the four-branch dispatch
//   gsl_sf_erf_e         erfseries below 1, 1 - erfc above
//
// EVERY COEFFICIENT WAS EXTRACTED FROM THE C SOURCE BY SCRIPT, not typed.
// Hand-transcribing 65 constants is how a table acquires a single wrong digit
// that no integration test notices -- this project has already found exactly
// that defect in another upstream's Gauss-Legendre nodes.
//
// THE TRUNCATION ORDERS ARE GSL'S OWN. Each `cheb_series` in erfc.c carries a
// fifth field, `order_sp`, which is the order GSL itself considers sufficient
// in SINGLE precision: 12, 16 and 12 against full orders of 19, 24 and 19.
// This port uses `order_sp`, so it is not an approximation of GSL -- it is the
// single-precision path GSL provides. The remaining coefficients are retained
// in the arrays so a caller can raise the order if they ever want to.

// erfc(x) for f32, following GSL's four-branch structure exactly.
//
// Branches, with `ax = |x|`:
//   ax <= 1   Chebyshev erfc_xlt1 at t = 2*ax - 1
//   ax <= 5   exp(-x^2) * Chebyshev erfc_x15 at t = (ax - 3)/2
//   ax < 10   exp(-x^2)/ax * Chebyshev erfc_x510 at t = (2*ax - 15)/5
//   else      erfc8: Hart rational * exp(-x^2)
// then the reflection erfc(-x) = 2 - erfc(x).
//
// Note the branch thresholds are on |x| but `exp(-x*x)` uses x, as upstream
// has it; they agree because the square is even, and it is left in upstream's
// form rather than "tidied" to ax.
fn petir_erfc(x: f32) -> f32 {
    let ax = abs(x);
    var e_val: f32;

    if (ax <= 1.0) {
        let t = 2.0 * ax - 1.0;
        e_val = petir_cheb_erfc_xlt1(t);
    } else if (ax <= 5.0) {
        let ex2 = exp(-x * x);
        let t = 0.5 * (ax - 3.0);
        e_val = ex2 * petir_cheb_erfc_x15(t);
    } else if (ax < 10.0) {
        let exterm = exp(-x * x) / ax;
        let t = (2.0 * ax - 15.0) / 5.0;
        e_val = exterm * petir_cheb_erfc_x510(t);
    } else {
        e_val = petir_erfc8(ax);
    }

    if (x < 0.0) {
        return 2.0 - e_val;
    }
    return e_val;
}

// erf(x) for f32. Ports `gsl_sf_erf_e`.
//
// Below |x| = 1 the Taylor series is used directly rather than `1 - erfc`,
// because near zero erfc is close to 1 and the subtraction would cancel away
// the answer. Above it, erf is taken as `1 - erfc` -- which is safe in that
// direction because erfc is the small quantity there.
fn petir_erf(x: f32) -> f32 {
    if (abs(x) < 1.0) {
        return petir_erfseries(x);
    }
    return 1.0 - petir_erfc(x);
}

// The Maclaurin series for erf. Ports `erfseries`.
//
// Upstream runs k = 1..29. That is an f64 term count; in f32 the terms fall
// below epsilon far sooner, but the loop bound is kept at upstream's value so
// this is a transcription rather than a retuning -- the extra terms add
// nothing and cost nothing measurable.
fn petir_erfseries(x: f32) -> f32 {
    let two_over_sqrtpi: f32 = 1.12837916709551257390;
    var coef = x;
    var e = coef;
    for (var k: i32 = 1; k < 30; k = k + 1) {
        coef = coef * (-x * x / f32(k));
        e = e + coef / (2.0 * f32(k) + 1.0);
    }
    return two_over_sqrtpi * e;
}

// erfc for x > 10. Ports `erfc8` and `erfc8_sum` -- Hart et al. index 5725,
// a degree-5 over degree-6 rational in x, times exp(-x^2).
fn petir_erfc8(x: f32) -> f32 {
    var p = array<f32, 6>(
        2.97886562639399288862, 7.409740605964741794425, 6.1602098531096305440906,
        5.019049726784267463450058, 1.275366644729965952479585264,
        0.5641895835477550741253201704
    );
    var q = array<f32, 7>(
        3.3690752069827527677, 9.608965327192787870698, 17.08144074746600431571095,
        12.0489519278551290360340491, 9.396034016235054150430579648,
        2.260528520767326969591866945, 1.0
    );

    var num: f32 = p[5];
    for (var i: i32 = 4; i >= 0; i = i - 1) {
        num = x * num + p[i];
    }
    var den: f32 = q[6];
    for (var i: i32 = 5; i >= 0; i = i - 1) {
        den = x * den + q[i];
    }
    return (num / den) * exp(-x * x);
}

// Chebyshev fit for erfc((t+1)/2), -1 < t < 1.
//
// Clenshaw at GSL's single-precision order 12 (full order 19).
fn petir_cheb_erfc_xlt1(t: f32) -> f32 {
    var c = array<f32, 20>(
        1.06073416421769980345174155056, -0.42582445804381043569204735291,
        0.04955262679620434040357683080, 0.00449293488768382749558001242,
        -0.00129194104658496953494224761, -0.00001836389292149396270416979,
        0.00002211114704099526291538556, -5.23337485234257134673693179020e-7,
        -2.78184788833537885382530989578e-7, 1.41158092748813114560316684249e-8,
        2.72571296330561699984539141865e-9, -2.06343904872070629406401492476e-10,
        -2.14273991996785367924201401812e-11, 2.22990255539358204580285098119e-12,
        1.36250074650698280575807934155e-13, -1.95144010922293091898995913038e-14,
        -6.85627169231704599442806370690e-16, 1.44506492869699938239521607493e-16,
        2.45935306460536488037576200030e-18, -9.29599561220523396007359328540e-19
    );
    var d1: f32 = 0.0;
    var d2: f32 = 0.0;
    let y2 = 2.0 * t;
    for (var i: i32 = 12; i >= 1; i = i - 1) {
        let temp = d1;
        d1 = y2 * d1 - d2 + c[i];
        d2 = temp;
    }
    return t * d1 - d2 + 0.5 * c[0];
}

// Chebyshev fit for erfc(x) exp(x^2), 1 < x < 5, x = 2t + 3.
//
// Clenshaw at GSL's single-precision order 16 (full order 24).
fn petir_cheb_erfc_x15(t: f32) -> f32 {
    var c = array<f32, 25>(
        0.44045832024338111077637466616, -0.143958836762168335790826895326,
        0.044786499817939267247056666937, -0.013343124200271211203618353102,
        0.003824682739750469767692372556, -0.001058699227195126547306482530,
        0.000283859419210073742736310108, -0.000073906170662206760483959432,
        0.000018725312521489179015872934, -4.62530981164919445131297264430e-6,
        1.11558657244432857487884006422e-6, -2.63098662650834130067808832725e-7,
        6.07462122724551777372119408710e-8, -1.37460865539865444777251011793e-8,
        3.05157051905475145520096717210e-9, -6.65174789720310713757307724790e-10,
        1.42483346273207784489792999706e-10, -3.00141127395323902092018744545e-11,
        6.22171792645348091472914001250e-12, -1.26994639225668496876152836555e-12,
        2.55385883033257575402681845385e-13, -5.06258237507038698392265499770e-14,
        9.89705409478327321641264227110e-15, -1.90685978789192181051961024995e-15,
        3.50826648032737849245113757340e-16
    );
    var d1: f32 = 0.0;
    var d2: f32 = 0.0;
    let y2 = 2.0 * t;
    for (var i: i32 = 16; i >= 1; i = i - 1) {
        let temp = d1;
        d1 = y2 * d1 - d2 + c[i];
        d2 = temp;
    }
    return t * d1 - d2 + 0.5 * c[0];
}

// Chebyshev fit for erfc(x) x exp(x^2), 5 < x < 10, x = (5t + 15)/2.
//
// Clenshaw at GSL's single-precision order 12 (full order 19).
fn petir_cheb_erfc_x510(t: f32) -> f32 {
    var c = array<f32, 20>(
        1.11684990123545698684297865808, 0.003736240359381998520654927536,
        -0.000916623948045470238763619870, 0.000199094325044940833965078819,
        -0.000040276384918650072591781859, 7.76515264697061049477127605790e-6,
        -1.44464794206689070402099225301e-6, 2.61311930343463958393485241947e-7,
        -4.61833026634844152345304095560e-8, 8.00253111512943601598732144340e-9,
        -1.36291114862793031395712122089e-9, 2.28570483090160869607683087722e-10,
        -3.78022521563251805044056974560e-11, 6.17253683874528285729910462130e-12,
        -9.96019290955316888445830597430e-13, 1.58953143706980770269506726000e-13,
        -2.51045971047162509999527428316e-14, 3.92607828989125810013581287560e-15,
        -6.07970619384160374392535453420e-16, 9.12600607264794717315507477670e-17
    );
    var d1: f32 = 0.0;
    var d2: f32 = 0.0;
    let y2 = 2.0 * t;
    for (var i: i32 = 12; i >= 1; i = i - 1) {
        let temp = d1;
        d1 = y2 * d1 - d2 + c[i];
        d2 = temp;
    }
    return t * d1 - d2 + 0.5 * c[0];
}
