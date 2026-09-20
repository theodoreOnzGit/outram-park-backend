// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::ellint`, which ports GSL
// 2.8's specfunc/ellint.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//   Algorithms: B. C. Carlson, Numer. Math. 33, 1 (1979); the two near-k=1
//   series are Abramowitz & Stegun 17.3.34 and 17.3.36.
//
//   R_F(x,y,z)   = (1/2) int_0^inf dt / sqrt((t+x)(t+y)(t+z))
//   R_D(x,y,z)   = (3/2) int_0^inf dt / ((t+z) sqrt((t+x)(t+y)(t+z)))
//   R_J(x,y,z,p) = (3/2) int_0^inf dt / ((t+p) sqrt((t+x)(t+y)(t+z)))
//   R_C(x,y)     = R_F(x,y,y)
//
// and the Legendre forms F, E, Pi, D follow algebraically.
//
// THE SECOND TABLE-FREE SHADER HERE, after dilog.wgsl. Nothing is fitted:
// the Carlson forms converge by duplication and close with a short
// polynomial in the deviations. So there is no coefficient array, no
// order_sp decision, and nothing for the generator that produces the other
// shaders to do -- this one is written by hand alongside its mirror.
//
// ============================================================================
// TWO DESIGN DECISIONS, BOTH MEASURED
// ============================================================================
//
// 1. errtol = 0.03, WHICH IS UPSTREAM'S OWN SINGLE-PRECISION VALUE.
//
//    GSL's routines take a gsl_mode_t and read one thing from it:
//
//      GSL_PREC_DOUBLE  ->  errtol 0.001  ->  relative error ~1e-17
//      otherwise        ->  errtol 0.03   ->  relative error ~2e-08
//
//    (upstream's own table, in the comment above gsl_sf_ellint_RC_e). This
//    is the THIRD kind of single-precision parameter GSL supplies, after
//    cheb_series's order_sp and the per-width machine constants -- and the
//    first that changes the amount of WORK rather than of data. Measured in
//    f64 over 201 moduli: 8447 duplication steps against Double's 14615,
//    for a 1.309e-11 difference. Five orders below f32's epsilon, so it is
//    free here and halves the iteration count.
//
// 2. THE ITERATION CAP IS 16, NOT UPSTREAM'S 10000.
//
//    GSL guards its duplication loops with nmax = 10000. That is a
//    can't-happen bound, not a working one. Measured at errtol 0.03 over the
//    WHOLE f32-reachable domain -- x and y swept geometrically from
//    5*FLT_MIN to 0.2*FLT_MAX, 121 x 121 pairs:
//
//      R_C   9 steps at worst
//      R_F   8
//      R_D   8
//      R_J  13 in total, including the R_C it calls at every step
//      a whole Pi(phi,k,n) call: 14 in total
//
//    so 16 per loop has room and 10000 is four orders of dead ceiling. On a
//    GPU that ceiling is not free the way it is on a CPU: a uniform-control
//    -flow loop bound is what the compiler unrolls against.
//
// R_J CALLS R_C INSIDE ITS OWN LOOP, AND THAT IS FINE. WGSL forbids
// RECURSION, not nested calls, and R_C never calls back. This was recorded
// as an open question when the f64 module landed and it is not one.
//
// f32 DOMAIN BOUNDS, all recomputed from FLT_MIN/FLT_MAX rather than
// retargeted by hand:
//   R_F, R_C   lolim 5*FLT_MIN = 5.8775e-38,  uplim 0.2*FLT_MAX = 6.8056e+37
//   R_D        lolim 2/FLT_MAX^(2/3) = 4.1033e-26,
//              uplim (0.1*errtol/FLT_MIN)^(2/3) = 4.0235e+23
//   R_J        lolim (5*FLT_MIN)^(1/3) = 3.8880e-13,
//              uplim 0.3*(0.2*FLT_MAX)^(1/3) = 1.2248e+12

const PETIR_ELL_ERRTOL: f32 = 0.03;
const PETIR_ELL_NMAX: u32 = 16u;

const PETIR_ELL_RF_LOLIM: f32 = 5.8774718e-38;
const PETIR_ELL_RF_UPLIM: f32 = 6.8056469e37;
const PETIR_ELL_RD_LOLIM: f32 = 4.1033357e-26;
const PETIR_ELL_RD_UPLIM: f32 = 4.0234673e23;
const PETIR_ELL_RJ_LOLIM: f32 = 3.8880352e-13;
const PETIR_ELL_RJ_UPLIM: f32 = 1.2248354e12;
// sqrt(f32::EPSILON), where Kcomp and Ecomp switch to the A&S series.
const PETIR_ELL_SQRT_EPS: f32 = 3.4526698e-4;

fn petir_ell_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

fn petir_ell_max3(x: f32, y: f32, z: f32) -> f32 { return max(max(x, y), z); }
fn petir_ell_max4(x: f32, y: f32, z: f32, w: f32) -> f32 { return max(max(max(x, y), z), w); }

// Carlson's R_C(x,y) = R_F(x,y,y).
fn petir_ellint_rc(x: f32, y: f32) -> f32 {
    if (!(x >= 0.0) || !(y >= 0.0)) { return petir_ell_nan(); }
    if (x + y < PETIR_ELL_RF_LOLIM || max(x, y) >= PETIR_ELL_RF_UPLIM) { return petir_ell_nan(); }
    let c1 = 1.0 / 7.0;
    let c2 = 9.0 / 22.0;
    var xn = x;
    var yn = y;
    var mu = 0.0;
    var sn = 0.0;
    for (var n: u32 = 0u; n < PETIR_ELL_NMAX; n = n + 1u) {
        mu = (xn + yn + yn) / 3.0;
        sn = (yn + mu) / mu - 2.0;
        if (abs(sn) < PETIR_ELL_ERRTOL) { break; }
        let lamda = 2.0 * sqrt(xn) * sqrt(yn) + yn;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
    }
    let s = sn * sn * (0.3 + sn * (c1 + sn * (0.375 + sn * c2)));
    return (1.0 + s) / sqrt(mu);
}

// Carlson's R_D(x,y,z).
fn petir_ellint_rd(x: f32, y: f32, z: f32) -> f32 {
    if (!(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0)) { return petir_ell_nan(); }
    if (min(x + y, z) < PETIR_ELL_RD_LOLIM) { return petir_ell_nan(); }
    if (petir_ell_max3(x, y, z) >= PETIR_ELL_RD_UPLIM) { return petir_ell_nan(); }
    let c1 = 3.0 / 14.0;
    let c2 = 1.0 / 6.0;
    let c3 = 9.0 / 22.0;
    let c4 = 3.0 / 26.0;
    var xn = x;
    var yn = y;
    var zn = z;
    var sigma = 0.0;
    var power4 = 1.0;
    var mu = 0.0;
    var xndev = 0.0;
    var yndev = 0.0;
    var zndev = 0.0;
    for (var n: u32 = 0u; n < PETIR_ELL_NMAX; n = n + 1u) {
        mu = (xn + yn + 3.0 * zn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        if (petir_ell_max3(abs(xndev), abs(yndev), abs(zndev)) < PETIR_ELL_ERRTOL) { break; }
        let xr = sqrt(xn);
        let yr = sqrt(yn);
        let zr = sqrt(zn);
        let lamda = xr * (yr + zr) + yr * zr;
        sigma = sigma + power4 / (zr * (zn + lamda));
        power4 = power4 * 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
    }
    let ea = xndev * yndev;
    let eb = zndev * zndev;
    let ec = ea - eb;
    let ed = ea - 6.0 * eb;
    let ef = ed + ec + ec;
    let s1 = ed * (-c1 + 0.25 * c3 * ed - 1.5 * c4 * zndev * ef);
    let s2 = zndev * (c2 * ef + zndev * (-c3 * ec + zndev * c4 * ea));
    return 3.0 * sigma + power4 * (1.0 + s1 + s2) / (mu * sqrt(mu));
}

// Carlson's R_F(x,y,z).
fn petir_ellint_rf(x: f32, y: f32, z: f32) -> f32 {
    if (!(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0)) { return petir_ell_nan(); }
    if (x + y < PETIR_ELL_RF_LOLIM || x + z < PETIR_ELL_RF_LOLIM || y + z < PETIR_ELL_RF_LOLIM) {
        return petir_ell_nan();
    }
    if (petir_ell_max3(x, y, z) >= PETIR_ELL_RF_UPLIM) { return petir_ell_nan(); }
    let c1 = 1.0 / 24.0;
    let c2 = 3.0 / 44.0;
    let c3 = 1.0 / 14.0;
    var xn = x;
    var yn = y;
    var zn = z;
    var mu = 0.0;
    var xndev = 0.0;
    var yndev = 0.0;
    var zndev = 0.0;
    for (var n: u32 = 0u; n < PETIR_ELL_NMAX; n = n + 1u) {
        mu = (xn + yn + zn) / 3.0;
        xndev = 2.0 - (mu + xn) / mu;
        yndev = 2.0 - (mu + yn) / mu;
        zndev = 2.0 - (mu + zn) / mu;
        if (petir_ell_max3(abs(xndev), abs(yndev), abs(zndev)) < PETIR_ELL_ERRTOL) { break; }
        let xr = sqrt(xn);
        let yr = sqrt(yn);
        let zr = sqrt(zn);
        let lamda = xr * (yr + zr) + yr * zr;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
    }
    let e2 = xndev * yndev - zndev * zndev;
    let e3 = xndev * yndev * zndev;
    let s = 1.0 + (c1 * e2 - 0.1 - c2 * e3) * e2 + c3 * e3;
    return s / sqrt(mu);
}

// Carlson's R_J(x,y,z,p). Calls R_C once per duplication step -- a nested
// call, not recursion, which WGSL permits.
fn petir_ellint_rj(x: f32, y: f32, z: f32, p: f32) -> f32 {
    if (!(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0) || !(p >= 0.0)) { return petir_ell_nan(); }
    if (x + y < PETIR_ELL_RJ_LOLIM || x + z < PETIR_ELL_RJ_LOLIM
        || y + z < PETIR_ELL_RJ_LOLIM || p < PETIR_ELL_RJ_LOLIM) {
        return petir_ell_nan();
    }
    if (petir_ell_max4(x, y, z, p) >= PETIR_ELL_RJ_UPLIM) { return petir_ell_nan(); }
    let c1 = 3.0 / 14.0;
    let c2 = 1.0 / 3.0;
    let c3 = 3.0 / 22.0;
    let c4 = 3.0 / 26.0;
    var xn = x;
    var yn = y;
    var zn = z;
    var pn = p;
    var sigma = 0.0;
    var power4 = 1.0;
    var mu = 0.0;
    var xndev = 0.0;
    var yndev = 0.0;
    var zndev = 0.0;
    var pndev = 0.0;
    for (var n: u32 = 0u; n < PETIR_ELL_NMAX; n = n + 1u) {
        mu = (xn + yn + zn + pn + pn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        pndev = (mu - pn) / mu;
        if (petir_ell_max4(abs(xndev), abs(yndev), abs(zndev), abs(pndev)) < PETIR_ELL_ERRTOL) {
            break;
        }
        let xr = sqrt(xn);
        let yr = sqrt(yn);
        let zr = sqrt(zn);
        let lamda = xr * (yr + zr) + yr * zr;
        var alfa = pn * (xr + yr + zr) + xr * yr * zr;
        alfa = alfa * alfa;
        let beta = pn * (pn + lamda) * (pn + lamda);
        let rcv = petir_ellint_rc(alfa, beta);
        if (rcv != rcv) { return petir_ell_nan(); }
        sigma = sigma + power4 * rcv;
        power4 = power4 * 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
        pn = (pn + lamda) * 0.25;
    }
    let ea = xndev * (yndev + zndev) + yndev * zndev;
    let eb = xndev * yndev * zndev;
    let ec = pndev * pndev;
    let e2 = ea - 3.0 * ec;
    let e3 = eb + 2.0 * pndev * (ea - ec);
    let s1 = 1.0 + e2 * (-c1 + 0.75 * c3 * e2 - 1.5 * c4 * e3);
    let s2 = eb * (0.5 * c2 + pndev * (-c3 - c3 + pndev * c4));
    let s3 = pndev * ea * (c2 - pndev * c3) - c2 * pndev * ec;
    return 3.0 * sigma + power4 * (s1 + s2 + s3) / (mu * sqrt(mu));
}

// The complete integrals.
fn petir_ellint_kcomp(k: f32) -> f32 {
    if (!(k * k < 1.0)) { return petir_ell_nan(); }
    if (k * k >= 1.0 - PETIR_ELL_SQRT_EPS) {
        // A&S 17.3.34 -- K diverges logarithmically at k = 1 and no Carlson
        // form can represent that.
        let y = 1.0 - k * k;
        let ta = 1.38629436112 + y * (0.09666344259 + y * 0.03590092383);
        let tb = -log(y) * (0.5 + y * (0.12498593597 + y * 0.06880248576));
        return ta + tb;
    }
    return petir_ellint_rf(0.0, 1.0 - k * k, 1.0);
}

fn petir_ellint_ecomp(k: f32) -> f32 {
    if (!(k * k < 1.0)) { return petir_ell_nan(); }
    if (k * k >= 1.0 - PETIR_ELL_SQRT_EPS) {
        // A&S 17.3.36.
        let y = 1.0 - k * k;
        let ta = 1.0 + y * (0.44325141463 + y * (0.06260601220 + 0.04757383546 * y));
        let tb = -y * log(y) * (0.24998368310 + y * (0.09200180037 + 0.04069697526 * y));
        return ta + tb;
    }
    let y = 1.0 - k * k;
    return petir_ellint_rf(0.0, y, 1.0) - k * k / 3.0 * petir_ellint_rd(0.0, y, 1.0);
}

fn petir_ellint_dcomp(k: f32) -> f32 {
    if (!(k * k < 1.0)) { return petir_ell_nan(); }
    return (1.0 / 3.0) * petir_ellint_rd(0.0, 1.0 - k * k, 1.0);
}

fn petir_ellint_pcomp(k: f32, n: f32) -> f32 {
    if (!(k * k < 1.0)) { return petir_ell_nan(); }
    let y = 1.0 - k * k;
    return petir_ellint_rf(0.0, y, 1.0) - (n / 3.0) * petir_ellint_rj(0.0, y, 1.0, 1.0 + n);
}

// The incomplete integrals. phi is reduced modulo pi with the periodicity
// added back -- upstream's own reduction, which its comment calls approximate.
fn petir_ellint_f(phi_in: f32, k: f32) -> f32 {
    if (phi_in != phi_in || k != k) { return petir_ell_nan(); }
    let nc = floor(phi_in / 3.1415927 + 0.5);
    let phi = phi_in - nc * 3.1415927;
    let sp = sin(phi);
    let s2 = sp * sp;
    let v = sp * petir_ellint_rf(1.0 - s2, 1.0 - k * k * s2, 1.0);
    if (nc == 0.0) { return v; }
    return v + 2.0 * nc * petir_ellint_kcomp(k);
}

fn petir_ellint_e(phi_in: f32, k: f32) -> f32 {
    if (phi_in != phi_in || k != k) { return petir_ell_nan(); }
    let nc = floor(phi_in / 3.1415927 + 0.5);
    let phi = phi_in - nc * 3.1415927;
    let sp = sin(phi);
    let s2 = sp * sp;
    let x = 1.0 - s2;
    let y = 1.0 - k * k * s2;
    if (x < 1.1920929e-7) {
        let re = petir_ellint_ecomp(k);
        return 2.0 * nc * re + sign(sp) * re;
    }
    let s3 = s2 * sp;
    let v = sp * petir_ellint_rf(x, y, 1.0) - k * k / 3.0 * s3 * petir_ellint_rd(x, y, 1.0);
    if (nc == 0.0) { return v; }
    return v + 2.0 * nc * petir_ellint_ecomp(k);
}

fn petir_ellint_p(phi_in: f32, k: f32, n: f32) -> f32 {
    if (phi_in != phi_in || k != k || n != n) { return petir_ell_nan(); }
    let nc = floor(phi_in / 3.1415927 + 0.5);
    let phi = phi_in - nc * 3.1415927;
    let sp = sin(phi);
    let s2 = sp * sp;
    let s3 = s2 * sp;
    let x = 1.0 - s2;
    let y = 1.0 - k * k * s2;
    let v = sp * petir_ellint_rf(x, y, 1.0)
          - n / 3.0 * s3 * petir_ellint_rj(x, y, 1.0, 1.0 + n * s2);
    if (nc == 0.0) { return v; }
    return v + 2.0 * nc * petir_ellint_pcomp(k, n);
}

fn petir_ellint_d(phi_in: f32, k: f32) -> f32 {
    if (phi_in != phi_in || k != k) { return petir_ell_nan(); }
    let nc = floor(phi_in / 3.1415927 + 0.5);
    let phi = phi_in - nc * 3.1415927;
    let sp = sin(phi);
    let s2 = sp * sp;
    let s3 = s2 * sp;
    let v = s3 / 3.0 * petir_ellint_rd(1.0 - s2, 1.0 - k * k * s2, 1.0);
    if (nc == 0.0) { return v; }
    return v + 2.0 * nc * petir_ellint_dcomp(k);
}

// One dispatcher over the four complete integrals, in the order K, E, D, Pi.
// `n` is read only by Pi. Any other `which` gives NaN.
fn petir_ellint_comp(which: u32, k: f32, n: f32) -> f32 {
    switch (which) {
        case 0u: { return petir_ellint_kcomp(k); }
        case 1u: { return petir_ellint_ecomp(k); }
        case 2u: { return petir_ellint_dcomp(k); }
        case 3u: { return petir_ellint_pcomp(k, n); }
        default: { return petir_ell_nan(); }
    }
}
