// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::psi` and
// `petir::specfunc::zeta`, which port GSL 2.8's specfunc/psi.c and
// specfunc/zeta.c.
//   Copyright (C) 1996-2000 Gerard Jungman. GPL-3.0-or-later.
//
// REQUIRES `gamma.wgsl` to be concatenated ahead of it: the zeta reflection
// branch calls `petir_gamma`. Nothing here duplicates the Lanczos table.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 modules -- 136
// literals across six series -- and the same script emits
// `wgsl::mirror_psi_zeta`, so the shader and its CPU mirror cannot disagree
// about a constant.
//
// WHAT IS NOT HERE, AND WHY -- three deliberate omissions, stated rather than
// left to be discovered:
//
// 1. THE INTEGER-ARGUMENT ENTRY POINTS. `psi_int`, `psi_1_int`, `zeta_int`,
//    `zetam1_int` and `eta_int` are table lookups over 101-entry arrays. WGSL
//    has no module-scope storage for them, so each call would construct a
//    101-element `var array<f32, 101>` on the stack before indexing it once.
//    A lookup is not a GPU workload; the continuous functions are.
//
// 2. `zeta(s)` FOR s <= -34, WHICH RETURNS NaN. The reflection branch needs
//    Gamma(1 - s), and Gamma overflows f32 at about 35. The ANSWER is often
//    still representable -- zeta(-35) is about 8e14 -- but this ROUTE is not,
//    and 13 of GSL's 18 `twopi_pow` entries are themselves past f32::MAX. A
//    NaN says so; silently returning the 0 or inf that falls out would not.
//    The f64 module has no such limit.
//
// 3. `psi_n` FOR n >= 2. It is (-1)^(n+1) n! zeta(n+1, x), and n! leaves f32
//    at n = 34 while the polygamma order is a per-invocation integer rather
//    than something a kernel maps over. `petir_psi` and `petir_psi_1` cover
//    what a shader plausibly wants; the f64 module covers the rest.
//
// WHAT f32 COSTS HERE. Every branch calls log, exp, sin or pow, all of which
// WGSL specifies to an ULP BOUND rather than correct rounding, so none of
// these kernels can be held to bit-identity with the mirror. `pow` is the
// worst offender: WGSL defines pow(x, y) as exp2(y * log2(x)), so hzeta's
// `pow(k + q, -s)` at large s loses digits in a way the f64 path does not.
// The measured figures are in `wgsl::mirror_psi_zeta`.

// GSL's `psi_cs`, 23 coefficients, Clenshaw in GSL's convention.
fn petir_cheb_psi_cs(x: f32) -> f32 {
    var c = array<f32, 23>(
        -0.0380570808, 0.491415393, -0.0568157478, 0.00835782123, -0.00133323286,
        0.000220313287, -3.70402382e-05, 6.28379365e-06, -1.07126391e-06, 1.83128395e-07,
        -3.13535094e-08, 5.37280878e-09, -9.21168141e-10, 1.57981265e-10, -2.7098646e-11,
        4.648722e-12, -7.97527e-13, 1.36827e-13, -2.3475e-14, 4.027e-15, -6.91e-16, 1.18e-16,
        -2e-17
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `apsi_cs`, 16 coefficients, Clenshaw in GSL's convention.
fn petir_cheb_apsi_cs(x: f32) -> f32 {
    var c = array<f32, 16>(
        -0.0204749045, -0.0101801272, 5.59718725e-05, -1.29171766e-06, 5.72858606e-08,
        -3.8213539e-09, 3.397434e-10, -3.74838e-11, 4.899e-12, -7.344e-13, 1.233e-13,
        -2.28e-14, 4.5e-15, -9e-16, 2e-16, -0.0
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 15; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `r1py`, 30 coefficients, Clenshaw in GSL's convention.
fn petir_cheb_r1py(x: f32) -> f32 {
    var c = array<f32, 30>(
        1.59888328, 0.679056254, -0.068485803, -0.00578818418, 0.00851125817, -0.00404265613,
        0.00135232841, -0.000311646564, 1.85075638e-05, 2.83487054e-05, -1.9487536e-05,
        8.07097887e-06, -2.29835643e-06, 3.05066296e-07, 1.30422386e-07, -1.23086572e-07,
        5.77108557e-08, -1.82755593e-08, 3.10204713e-09, 6.89893275e-10, -8.71822903e-10,
        4.40691477e-10, -1.47273111e-10, 2.75896825e-11, 4.18718268e-12, -6.56734605e-12,
        3.44879009e-12, -1.18072514e-12, 2.37983143e-13, 2.16636304e-15
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `zeta_xlt1`, 14 coefficients, Clenshaw in GSL's convention.
fn petir_cheb_zeta_xlt1(x: f32) -> f32 {
    var c = array<f32, 14>(
        1.48018677, 0.250120625, 0.00991137502, -0.000120847597, -4.75858664e-06,
        2.22299467e-07, -2.22374965e-09, -1.01732265e-10, 4.37566435e-12, -6.22296326e-14,
        -6.6116201e-16, 4.94772795e-17, -1.04298191e-18, 6.99252162e-21
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 13; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `zeta_xgt1`, 30 coefficients, Clenshaw in GSL's convention.
fn petir_cheb_zeta_xgt1(x: f32) -> f32 {
    var c = array<f32, 30>(
        19.3918516, 9.15253297, 0.242789766, -0.133900069, 0.0577827064, -0.0187625984,
        0.00394030143, -5.81508273e-05, -0.000375614891, 0.000189253055, -5.490322e-05,
        8.7086484e-06, 6.46094779e-07, -9.67497739e-07, 3.65854008e-07, -8.45925164e-08,
        9.99567861e-09, 1.42600364e-09, -1.17619688e-09, 3.71145759e-10, -7.47568552e-11,
        7.85369342e-12, 9.98271823e-13, -7.5276687e-13, 2.19550264e-13, -4.19348599e-14,
        4.63411496e-15, 2.37424885e-16, -2.72765164e-16, 7.84735701e-17
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `zetam1_inter`, 23 coefficients, Clenshaw in GSL's convention.
// ONLY 23 OF 24 COEFFICIENTS. GSL's cheb_series declares order 22, so
// cheb_eval_e reads c[0 ..= 22]. Taking the whole array changes the answer
// at 943 of 1001 points on [5, 15] -- see the f64 module.
fn petir_cheb_zetam1_inter(x: f32) -> f32 {
    var c = array<f32, 23>(
        -21.7509436, -5.63036878, 0.0528041359, -0.0156381809, 0.00408218474, -0.00102648673,
        0.00026046988, -6.76175847e-05, 1.79284473e-05, -4.83238651e-06, 1.31913789e-06,
        -3.63760501e-07, 1.01146848e-07, -2.83215225e-08, 7.9773371e-09, -2.25850169e-09,
        6.42269393e-10, -1.83363862e-10, 5.25309764e-11, -1.50958687e-11, 4.34997546e-12,
        -1.25597783e-12, 3.6128074e-13
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `hzeta_c`, the Euler-Maclaurin Bernoulli coefficients.
fn petir_hzeta_c(k: u32) -> f32 {
    var c = array<f32, 15>(
        1.0, 0.0833333333, -0.00138888889, 3.30687831e-05, -8.26719577e-07, 2.0876757e-08,
        -5.28419014e-10, 1.33825365e-11, -3.3896803e-13, 8.58606206e-15, -2.1748687e-16,
        5.50900283e-18, -1.39544647e-19, 3.53470704e-21, -8.95351743e-23
    );
    return c[k];
}

// (2 pi)^(10 n) for n = 0 ..= 3, GSL's `twopi_pow` truncated to what f32 can
// hold. Entries 5 and beyond are past f32::MAX; entry 4 is 8.46e31 and would
// fit, but `petir_zeta` refuses s <= -34 before it could be reached, so it is
// left out rather than carried unreachable.
fn petir_twopi_pow(n: u32) -> f32 {
    var c = array<f32, 4>(1.0, 9.589560061550901e7, 9.195966217409213e15, 8.81852703658387e23);
    return c[n];
}

const PETIR_PSI_PI: f32 = 3.141592653589793;
const PETIR_PSI_LN2: f32 = 0.6931471805599453;
const PETIR_PSI_EULER: f32 = 0.5772156649015329;
const PETIR_PSI_ROOT5_EPS: f32 = 7.4009597974140505e-4;
// f32's smallest normal, standing in for GSL's 2 * GSL_DBL_MIN guard on
// |sin(pi x)|. GSL's own value flushes to zero in f32 and would never fire.
const PETIR_PSI_SQRT_MIN: f32 = 1.1754944e-38;

// psi(x), the digamma function. Ports `psi_x`. NaN at x = 0, -1, -2.
fn petir_psi(x: f32) -> f32 {
    let y = abs(x);
    if (x == 0.0 || x == -1.0 || x == -2.0) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (y >= 2.0) {
        let c = petir_cheb_apsi_cs(8.0 / (y * y) - 1.0);
        if (x < 0.0) {
            let s = sin(PETIR_PSI_PI * x);
            let co = cos(PETIR_PSI_PI * x);
            if (abs(s) < 2.0 * PETIR_PSI_SQRT_MIN) {
                return bitcast<f32>(0x7fc00000u);
            }
            return log(y) - 0.5 / x + c - PETIR_PSI_PI * co / s;
        }
        return log(y) - 0.5 / x + c;
    }
    if (x < -1.0) {
        let v = x + 2.0;
        return -(1.0 / x + 1.0 / (x + 1.0) + 1.0 / v) + petir_cheb_psi_cs(2.0 * v - 1.0);
    }
    if (x < 0.0) {
        let v = x + 1.0;
        return -(1.0 / x + 1.0 / v) + petir_cheb_psi_cs(2.0 * v - 1.0);
    }
    if (x < 1.0) {
        return -1.0 / x + petir_cheb_psi_cs(2.0 * x - 1.0);
    }
    return petir_cheb_psi_cs(2.0 * (x - 1.0) - 1.0);
}

// Re psi(1 + i y). Ports `psi_1piy`. Even in y.
fn petir_psi_1piy(y: f32) -> f32 {
    let ay = abs(y);
    if (ay > 1000.0) {
        let yi2 = 1.0 / (ay * ay);
        return log(ay) + yi2 * (1.0 / 12.0 + 1.0 / 120.0 * yi2 + 1.0 / 252.0 * yi2 * yi2);
    }
    if (ay > 10.0) {
        let yi2 = 1.0 / (ay * ay);
        let sum = yi2 * (1.0 / 12.0
            + yi2 * (1.0 / 120.0
            + yi2 * (1.0 / 252.0
            + yi2 * (1.0 / 240.0
            + yi2 * (1.0 / 132.0 + 691.0 / 32760.0 * yi2)))));
        return log(ay) + sum;
    }
    if (ay > 1.0) {
        let y2 = ay * ay;
        let v = y2 * (1.0 / (1.0 + y2) + 0.5 / (4.0 + y2));
        return petir_cheb_r1py((2.0 * ay - 11.0) / 9.0) - PETIR_PSI_EULER + v;
    }
    let y2 = y * y;
    let p = 1.9603999466879846e-4
        + y2 * (-3.8426659205114377e-8 + y2 * (1.0041592839497644e-11 - y2 * 2.951674376350019e-15));
    var sum = 0.0;
    for (var n: u32 = 1u; n <= 50u; n = n + 1u) {
        let nf = f32(n);
        sum = sum + 1.0 / (nf * (nf * nf + y2));
    }
    return -PETIR_PSI_EULER + y2 * (sum + p);
}

// The Hurwitz zeta zeta(s, q), s > 1 and q > 0. Ports `hzeta`.
//
// The f64 port's underflow and overflow guards are kept at their GSL values;
// in f32 they simply never fire, and exp()/pow() produce 0 or inf of their
// own accord at the corresponding f32 bounds.
fn petir_hzeta(s: f32, q: f32) -> f32 {
    if (!(s > 1.0) || !(q > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    let max_bits = 54.0;
    if ((s > max_bits && q < 1.0) || (s > 0.5 * max_bits && q < 0.25)) {
        return pow(q, -s);
    }
    if (s > 0.5 * max_bits && q < 1.0) {
        return pow(q, -s) * (1.0 + pow(q / (1.0 + q), s) + pow(q / (2.0 + q), s));
    }
    let kmax = 10.0;
    let pmax = pow(kmax + q, -s);
    var scp = s;
    var pcp = pmax / (kmax + q);
    var ans = pmax * ((kmax + q) / (s - 1.0) + 0.5);
    for (var k: u32 = 0u; k < 10u; k = k + 1u) {
        ans = ans + pow(f32(k) + q, -s);
    }
    for (var j: u32 = 0u; j <= 12u; j = j + 1u) {
        let delta = petir_hzeta_c(j + 1u) * scp * pcp;
        ans = ans + delta;
        if (abs(delta / ans) < 0.5 * 1.1920929e-7) {
            break;
        }
        let jf = f32(j);
        scp = scp * (s + 2.0 * jf + 1.0) * (s + 2.0 * jf + 2.0);
        pcp = pcp / ((kmax + q) * (kmax + q));
    }
    return ans;
}

// zeta(s) for s >= 0, s != 1. Ports `riemann_zeta_sgt0`.
fn petir_zeta_sgt0(s: f32) -> f32 {
    if (s < 1.0) {
        return petir_cheb_zeta_xlt1(2.0 * s - 1.0) / (s - 1.0);
    }
    if (s <= 20.0) {
        return petir_cheb_zeta_xgt1((2.0 * s - 21.0) / 19.0) / (s - 1.0);
    }
    let f2 = 1.0 - pow(2.0, -s);
    let f3 = 1.0 - pow(3.0, -s);
    let f5 = 1.0 - pow(5.0, -s);
    let f7 = 1.0 - pow(7.0, -s);
    return 1.0 / (f2 * f3 * f5 * f7);
}

// zeta(1 - s) for s < 0. Ports `riemann_zeta_1ms_slt0`.
fn petir_zeta_1ms_slt0(s: f32) -> f32 {
    if (s > -19.0) {
        return petir_cheb_zeta_xgt1((-19.0 - 2.0 * s) / 19.0) / (-s);
    }
    let f2 = 1.0 - pow(2.0, -(1.0 - s));
    let f3 = 1.0 - pow(3.0, -(1.0 - s));
    let f5 = 1.0 - pow(5.0, -(1.0 - s));
    let f7 = 1.0 - pow(7.0, -(1.0 - s));
    return 1.0 / (f2 * f3 * f5 * f7);
}

// The Riemann zeta function. Ports `zeta`. NaN at s = 1 and for s <= -34.
//
// REQUIRES gamma.wgsl for `petir_gamma`.
fn petir_zeta(s: f32) -> f32 {
    if (s == 1.0) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (s >= 0.0) {
        return petir_zeta_sgt0(s);
    }
    let m = s % 2.0;
    var sin_term = 0.0;
    if (m != 0.0) {
        sin_term = sin(0.5 * PETIR_PSI_PI * (s % 4.0)) / PETIR_PSI_PI;
    }
    if (sin_term == 0.0) {
        return 0.0;  // the trivial zeros at the negative even integers
    }
    // f32 cannot carry the reflection branch below here -- see the header.
    if (s <= -34.0) {
        return bitcast<f32>(0x7fc00000u);
    }
    let n = u32(floor((-s) / 10.0));
    let fs = s + 10.0 * f32(n);
    let p = pow(2.0 * PETIR_PSI_PI, fs) / petir_twopi_pow(n);
    return p * petir_gamma(1.0 - s) * sin_term * petir_zeta_1ms_slt0(s);
}

// zeta(s) - 1 without forming the difference. Ports `zetam1`.
fn petir_zetam1(s: f32) -> f32 {
    if (s <= 5.0) {
        return petir_zeta(s) - 1.0;
    }
    if (s < 15.0) {
        return exp(petir_cheb_zetam1_inter((s - 10.0) / 5.0)) + pow(2.0, -s);
    }
    let a = pow(2.0, -s);
    let b = pow(3.0, -s);
    let c = pow(5.0, -s);
    let d = pow(7.0, -s);
    let e = pow(11.0, -s);
    let f = pow(13.0, -s);
    let t1 = a + b + c + d + e + f;
    let t2 = a * (b + c + d + e + f) + b * (c + d + e + f) + c * (d + e + f)
           + d * (e + f) + e * f;
    let zeta = 1.0 / ((1.0 - a) * (1.0 - b) * (1.0 - c) * (1.0 - d) * (1.0 - e) * (1.0 - f));
    return (t1 - t2) * zeta;
}

// The Dirichlet eta function. Ports `eta`. Finite at s = 1, where zeta is not.
fn petir_eta(s: f32) -> f32 {
    if (s > 100.0) {
        return 1.0;
    }
    if (abs(s - 1.0) < 10.0 * PETIR_PSI_ROOT5_EPS) {
        let del = s - 1.0;
        let c1 = PETIR_PSI_LN2 * (PETIR_PSI_EULER - 0.5 * PETIR_PSI_LN2);
        return PETIR_PSI_LN2 + del * (c1 + del * (-0.0326862962794493
             + del * (0.001568991705415515 + del * 0.0007498724211204753)));
    }
    return (1.0 - exp((1.0 - s) * PETIR_PSI_LN2)) * petir_zeta(s);
}

// psi(1, x), the trigamma function. Ports `psi_1`.
//
// For x > 0 this is exactly zeta(2, x): GSL routes it through psi_n_xg0 with
// n = 1, whose ln(1!) is zero, so the exp_mult collapses to the Hurwitz zeta.
fn petir_psi_1(x: f32) -> f32 {
    if (x == 0.0 || x == -1.0 || x == -2.0) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x > 0.0) {
        return petir_hzeta(2.0, x);
    }
    if (x > -5.0) {
        let m = -floor(x);
        let fx = x + m;
        if (fx == 0.0) {
            return bitcast<f32>(0x7fc00000u);
        }
        var sum = 0.0;
        let mi = u32(m);
        for (var k: u32 = 0u; k < mi; k = k + 1u) {
            let t = x + f32(k);
            sum = sum + 1.0 / (t * t);
        }
        return petir_hzeta(2.0, fx) + sum;
    }
    let sp = sin(PETIR_PSI_PI * x);
    return PETIR_PSI_PI * PETIR_PSI_PI / (sp * sp) - petir_hzeta(2.0, 1.0 - x);
}
