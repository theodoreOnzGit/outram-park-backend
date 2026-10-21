// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::legendre`, which ports
// GSL 2.8's specfunc/legendre_poly.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
//   P_l^m(x) -- the associated Legendre polynomials.
//
// SEPARATE FROM legendre.wgsl, which carries the ORDINARY P_n(x) by
// Bonnet's recurrence and is reached far more often. Keeping them apart
// means a kernel that only wants P_n does not carry the seed, the guard and
// the two-index recurrence it has no use for.
//
// THE FIFTH TABLE-FREE SHADER. A closed-form seed and an upward recurrence
// in the degree; nothing is fitted.
//
// THE OVERFLOW GUARD IS RETARGETED, and it is a RANGE GUARD in the taxonomy
// of docs/wgsl-coverage.md -- derived from LOG_DBL_MIN, which f32 does not
// share. f32's is -87.33654 against f64's -708.396, so the guard bites
// eight times sooner here, and it must: P_l^m grows factorially in m and
// leaves f32's range at m = 30 -- measured -- where f64 reaches infinity at
// m = 200.
//
// UPSTREAM'S GUARD HAS A HOLE AT l == m AND IT IS CARRIED.
//
//   legendre_poly.c:304 gates the t_s term on `dif == 0.0` rather than on
//   `sum == 0.0`. At l == m that zeroes the only term that can make the
//   estimate negative, leaving 0.5*log(2l+1) > 0, so the guard never fires
//   in exactly the case where P_l^m is largest. Measured in f64:
//   P_200^200(0.5) is inf with no error reported.
//
//   In f32 the hole bites at P_30^30, nearly seven times sooner than f64's
//   P_200^200 -- measured, not estimated.
//
//   Transcribed faithfully. This port's bar is agreement with GSL, and
//   repairing the condition here would make the shader disagree with the
//   f64 module it mirrors -- which is the one comparison that can catch a
//   transcription defect. The hole is DOCUMENTED, and on f32 it arrives
//   sooner: see mirror_legendre_plm.
//
// NO ARRAY IS ALLOCATED. The recurrence carries two scalars, so unlike
// elljac there is no register array and no cap to choose -- the trip count
// is l - m - 1, a uniform function of two integer parameters.

fn petir_plm_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

// f32's log(MIN_POSITIVE). f64's is -708.396.
const PETIR_PLM_LOG_MIN: f32 = -87.33654;

// P_m^m(x) = (-1)^m (2m-1)!! (1 - x^2)^{m/2}, as a running product.
// sqrt(1-x) * sqrt(1+x) rather than sqrt(1 - x*x): upstream's own form, and
// the one that does not cancel near |x| = 1.
fn petir_legendre_pmm(m: u32, x: f32) -> f32 {
    if (m == 0u) { return 1.0; }
    var p_mm = 1.0;
    let root_factor = sqrt(1.0 - x) * sqrt(1.0 + x);
    var fact_coeff = 1.0;
    for (var i: u32 = 1u; i <= m; i = i + 1u) {
        p_mm = p_mm * (-fact_coeff * root_factor);
        fact_coeff = fact_coeff + 2.0;
    }
    return p_mm;
}

// P_l^m(x). NaN for l < m, for |x| > 1, and where the magnitude estimate
// says the answer has left f32's range.
fn petir_legendre_plm(l: u32, m: u32, x: f32) -> f32 {
    if (x != x || l < m || x < -1.0 || x > 1.0) { return petir_plm_nan(); }

    let dif = f32(l - m);
    let sum = f32(l + m);
    var t_d = 0.0;
    var t_s = 0.0;
    if (dif != 0.0) {
        t_d = 0.5 * dif * (log(dif) - 1.0);
        // NOTE the condition is on `dif`, not `sum`. Upstream's, verbatim.
        t_s = 0.5 * sum * (log(sum) - 1.0);
    }
    let exp_check = 0.5 * log(2.0 * f32(l) + 1.0) + t_d - t_s;
    if (exp_check < PETIR_PLM_LOG_MIN + 10.0) { return petir_plm_nan(); }

    let p_mm = petir_legendre_pmm(m, x);
    let p_mmp1 = x * f32(2u * m + 1u) * p_mm;
    if (l == m) { return p_mm; }
    if (l == m + 1u) { return p_mmp1; }

    var p_ellm2 = p_mm;
    var p_ellm1 = p_mmp1;
    var p_ell = 0.0;
    for (var ell: u32 = m + 2u; ell <= l; ell = ell + 1u) {
        let ellf = f32(ell);
        p_ell = (x * (2.0 * ellf - 1.0) * p_ellm1 - (ellf + f32(m) - 1.0) * p_ellm2)
              / (ellf - f32(m));
        p_ellm2 = p_ellm1;
        p_ellm1 = p_ell;
    }
    return p_ell;
}
