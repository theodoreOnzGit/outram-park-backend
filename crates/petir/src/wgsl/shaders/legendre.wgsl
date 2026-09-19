// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from Bonnet's three-term recurrence as used by
// `petir::poly::dense::legendre` and by
// `petir/tests/gauss_legendre_table_audit.rs`, which evaluates P_n and P_n'
// this way precisely because it is stable at every order.
//
// Legendre polynomials are not decoration here: neutron scattering anisotropy
// is expanded in Legendre moments, so a GPU transport kernel needs P_n(mu).

// P_n(x) by Bonnet's recurrence, (k+1) P_{k+1} = (2k+1) x P_k - k P_{k-1}.
//
// Evaluated on VALUES rather than from expanded coefficients: the coefficients
// of P_n grow like 4^n and cancel catastrophically, which costs about one
// decimal digit per two degrees even in f64 (measured in
// `petir::poly::dense`'s orthogonality test). In f32 that is fatal by n ~ 8.
// The recurrence has no such problem.
fn petir_legendre_p(n: u32, x: f32) -> f32 {
    if (n == 0u) { return 1.0; }
    if (n == 1u) { return x; }

    var p0: f32 = 1.0;
    var p1: f32 = x;
    var k: u32 = 1u;
    loop {
        if (k >= n) { break; }
        let kf = f32(k);
        let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
        p0 = p1;
        p1 = p2;
        k = k + 1u;
    }
    return p1;
}

// P_n(x) and P_n'(x) together.
//
// The derivative uses `P_n' = n (x P_n - P_{n-1}) / (x^2 - 1)`, which is exact
// away from the endpoints and singular AT them. Callers evaluating at
// x = +-1 must use `P_n'(+-1) = +- n(n+1)/2` instead; that branch is left to
// the caller rather than hidden here, because a GPU kernel that silently
// returns a huge number at the endpoint is worse than one that does not
// pretend to handle it.
fn petir_legendre_p_dp(n: u32, x: f32) -> vec2<f32> {
    if (n == 0u) { return vec2<f32>(1.0, 0.0); }
    if (n == 1u) { return vec2<f32>(x, 1.0); }

    var p0: f32 = 1.0;
    var p1: f32 = x;
    var k: u32 = 1u;
    loop {
        if (k >= n) { break; }
        let kf = f32(k);
        let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
        p0 = p1;
        p1 = p2;
        k = k + 1u;
    }
    let dp = f32(n) * (x * p1 - p0) / (x * x - 1.0);
    return vec2<f32>(p1, dp);
}
