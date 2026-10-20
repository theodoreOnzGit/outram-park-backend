// Ported from CORE-MATH, https://gitlab.inria.fr/core-math/core-math,
// commit 7ebab73913c5a33a25402ee51c96ec06b3a18095 (2026-10-20), read
// 2026-10-20:
//   src/binary64/hypot/hypot.c   -> `hypot`  (cr_hypot)
//   src/binary32/hypot/hypotf.c  -> `hypotf` (cr_hypotf)
//
// Upstream notice, reproduced as the licence requires (both files carry it):
//
//   Correctly-rounded Euclidean distance function (hypot) for binary64 values.
//   Copyright (c) 2022-2026 Alexei Sibidanov <sibid@uvic.ca>.
//
//   Correctly-rounded Euclidean distance function (hypot) for binary32 values.
//   Copyright (c) 2022-2025 Alexei Sibidanov.
//
//   This file is part of the CORE-MATH project
//   (https://core-math.gitlabpages.inria.fr/).
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included
//   in all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//   OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//   MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
//   NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//   USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// Copyright (C) 2026 Theodore Ong and the outram-park contributors (this
// port). MIT into AGPL-3.0-only is one-way; see NOTICE.

//! **Correctly rounded** `hypot` for `f64` and `f32`, ported from CORE-MATH.
//!
//! Maintainer direction, gh:#997 (2026-10-20), after gh:#993 found that the
//! `libm` crate's (musl's) `hypot` is off by one ulp on some inputs —
//! `hypot(1, sqrt 3)` gave `2 - 2^-52` — so PETIR's QR, SVD and eigen ports
//! matched compiled GSL only when GSL was linked with that same `hypot`.
//! glibc's `hypot` is correctly rounded; so is this one. Every PETIR route to
//! `hypot` ([`crate::real::hypot`], `Real::hypot`, `RealMath::r_hypot`, on
//! both float types) now ends here.
//!
//! # Verification
//!
//! `tests/hypot_correct_rounding.rs` replays
//! `reference-data/core-math/hypot-mpfr-reference.txt`, made by
//! `reference-data/core-math/hypot_mpfr_driver.c` from GNU MPFR (`mpfr_hypot`
//! at the target precision, round-to-nearest, with the exponent range and
//! `mpfr_subnormalize` set to binary64 / binary32). See that test's doc
//! comment for the sweep and the measured results.
//!
//! # Cost, measured
//!
//! `tests/hypot_correct_rounding.rs::speed_per_call`, 2 000 000 calls on the
//! close-exponents sweep, best of five, `--release`, x86-64 host with FMA,
//! 2026-10-20:
//!
//! ```text
//!            PETIR (CORE-MATH)   libm crate (musl)   platform (glibc 2.44)
//!   hypot        8.3 ns              6.2 ns              7.7 ns
//!   hypotf       6.6 ns              6.8 ns              2.6 ns
//! ```
//!
//! So the f64 route is ~1.3x the `libm` crate it replaced and ~1.1x glibc;
//! the f32 route costs the same as before. Without hardware FMA,
//! `libm::fma` falls back to software and the f64 cost rises (not measured).
//!
//! # How the port handles what C has and `core` does not
//!
//! - **FMA.** `__builtin_fma`/`__builtin_fmaf` become [`libm::fma`] /
//!   [`libm::fmaf`]: correctly rounded fused multiply-adds (hardware `vfmadd`
//!   selected at run time on x86-64 when the CPU has it, a software
//!   emulation otherwise). `f64::mul_add` is `std`-only. A non-fused `a*b+c`
//!   would NOT be a substitute: the error terms `fma(x, x, -x*x)` must be
//!   exact.
//! - **`__builtin_sqrt`** becomes [`libm::sqrt`], which is IEEE-correct
//!   (the `sqrtsd` instruction where SSE2 exists).
//! - **128-bit integers.** `unsigned __int128` is Rust's `u128`, which `core`
//!   has on every target.
//! - **Unsigned wrap-around.** C's `u64` arithmetic is modular, and the hard
//!   path depends on it: `bm*bm` overflows 64 bits on purpose, and only the
//!   small difference `D = m2 - rm2` is used. PETIR builds release with
//!   overflow checks on, so every such operation is spelled `wrapping_*`.
//!
//! # Deviations from upstream (each **DEVIATION:** where made)
//!
//! - **Round-to-nearest only.** Upstream detects the dynamic rounding mode
//!   (`1 + 2^-54 == 1 - 2^-54`) and has branches for directed rounding. Rust
//!   gives no access to the rounding mode and LLVM assumes round-to-nearest,
//!   so those branches are unreachable and are omitted. The nearest branch
//!   is kept exactly.
//! - **No floating-point exception flags or `errno`.** Upstream saves and
//!   restores the MXCSR flags so an exact result raises no inexact flag, and
//!   raises underflow/overflow deliberately. Rust exposes no flags; the
//!   returned VALUE is unaffected. The overflow path still returns
//!   `MAX + MAX = +inf`.

#[allow(unused_imports)]
use crate::real::Real;

/// `fasttwosum` (`hypot.c`): `s = x + y`, `e` the exact error, for `|x| >= |y|`.
#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
}

/// `as_hypot_denorm` (`hypot.c`): both `x` and `y` subnormal, `a`, `b` their
/// encodings, `2^52 > a >= b > 0`.
#[inline(never)]
fn as_hypot_denorm(mut a: u64, mut b: u64) -> f64 {
    let af = a as i64 as f64;
    let bf = b as i64 as f64;
    // af and bf are x and y multiplied by 2^1074, thus integers
    a <<= 1;
    b <<= 1;
    let mut rm = libm::sqrt(af * af + bf * bf) as u64;
    let mut tm = (rm << 1) as i64;
    // D = a^2+b^2 - tm^2 (modular, as in C)
    let mut d = a
        .wrapping_mul(a)
        .wrapping_add(b.wrapping_mul(b))
        .wrapping_sub((tm as u64).wrapping_mul(tm as u64)) as i64;
    while d > 2 * tm {
        // tm too small
        d -= 2 * tm + 1;
        tm += 1;
    }
    while d < 0 {
        // tm too large
        d += 2 * tm - 1;
        tm -= 1;
    }
    // tm = floor(sqrt(a^2+b^2)), 0 <= D < 2*tm+1. A midpoint cannot occur in
    // the subnormal range (upstream's comment proves it).
    let rb = (tm & 1) as u64; // round bit for rm
    let sb = d != 0; // sticky bit
    rm = (tm >> 1) as u64;
    if sb {
        // DEVIATION: round-to-nearest only (see module docs); upstream's
        // `rb2` and underflow bookkeeping serve the flags, not the value.
        rm += rb;
    }
    f64::from_bits(rm)
}

/// `as_hypot_hard` (`hypot.c`): the rare case where the fast path cannot
/// decide the rounding. `x^2 + y^2` is formed exactly in integers and the
/// candidate root is squared and compared.
#[inline(never)]
fn as_hypot_hard(x: f64, y: f64) -> f64 {
    let xi = x.to_bits();
    let yi = y.to_bits();
    let mut bm = (xi & (!0u64 >> 12)) | 1u64 << 52;
    let mut lm = (yi & (!0u64 >> 12)) | 1u64 << 52;
    let be = (xi >> 52) as i32;
    let le = (yi >> 52) as i32;
    let ri = libm::sqrt(x * x + y * y).to_bits();
    const BS: i32 = 2;
    let mut rm = ri & (!0u64 >> 12);
    let mut re = (ri >> 52) as i32 - 0x3ff;
    rm |= 1u64 << 52;
    for _ in 0..3 {
        if rm == 1u64 << 52 {
            rm = !0u64 >> 11;
            re -= 1;
        } else {
            rm -= 1;
        }
    }
    bm <<= BS;
    let mut m2 = bm.wrapping_mul(bm);
    let de = be - le;
    let mut ls = BS - de;
    if ls >= 0 {
        lm <<= ls;
        m2 = m2.wrapping_add(lm.wrapping_mul(lm));
    } else {
        let lm2 = (lm as u128) * (lm as u128);
        ls *= 2;
        m2 = m2.wrapping_add((lm2 >> (-ls)) as u64); // ls < 0
        m2 |= ((lm2 << (128 + ls)) != 0) as u64;
    }
    let k = BS + re;
    let mut d: i64;
    loop {
        rm += 1 + (rm >= (1u64 << 53)) as u64;
        let tm = rm << k;
        let rm2 = tm.wrapping_mul(tm);
        d = m2.wrapping_sub(rm2) as i64;
        if d <= 0 {
            break;
        }
    }
    if d != 0 {
        // DEVIATION: round-to-nearest only; upstream's `op == om` is always
        // true under Rust (see module docs).
        let tm = (rm << k) - (1u64 << (k - (rm <= (1u64 << 53)) as i32));
        d = m2.wrapping_sub(tm.wrapping_mul(tm)) as i64;
        if d != 0 {
            rm = rm.wrapping_add((d >> 63) as u64);
        } else {
            rm -= rm & (1u64 << (1 - (rm <= (1u64 << 53)) as i32));
        }
    }
    if rm >= (1u64 << 53) {
        rm >>= 1;
        re += 1;
    }
    let e = i64::from(be) - 1 + i64::from(re);
    f64::from_bits(((e << 52) as u64).wrapping_add(rm))
}

/// `as_hypot_overflow` (`hypot.c`): `hypot(x, y) >= 2^1024`.
#[inline(never)]
fn as_hypot_overflow() -> f64 {
    let z = f64::from_bits(0x7fef_ffff_ffff_ffff); // 0x1.fffffffffffffp1023
    z + z
}

/// Correctly rounded `sqrt(x^2 + y^2)` for `f64` (round-to-nearest), without
/// spurious overflow or underflow. CORE-MATH `cr_hypot`
/// (`src/binary64/hypot/hypot.c`).
///
/// `hypot(+-inf, NaN) = +inf` (IEEE 754), other NaNs propagate.
pub fn hypot(x: f64, y: f64) -> f64 {
    let xi = x.to_bits();
    let yi = y.to_bits();
    let emsk: u64 = 0x7ff << 52;
    let mut ex = xi & emsk;
    let mut ey = yi & emsk;
    let x = x.abs();
    let y = y.abs();
    if ex == emsk || ey == emsk {
        // Either x or y is NaN or Inf
        let wx = xi << 1;
        let wy = yi << 1;
        let wm = emsk << 1;
        let ninf = (wx == wm) ^ (wy == wm);
        let nqnn = ((wx >> 52) == 0xfff) ^ ((wy >> 52) == 0xfff);
        // hypot(+-Inf, qNaN) = hypot(qNaN, +-Inf) = +Inf
        if ninf && nqnn {
            return if wx == wm { x * x } else { y * y };
        }
        return x + y; // inf, nan
    }
    // now both x and y are finite
    let u = if x > y { x } else { y };
    let v = if x > y { y } else { x };
    let mut xd = u.to_bits();
    let mut yd = v.to_bits();
    ey = yd;
    if (ey >> 52) == 0 {
        // y is subnormal
        if ey == 0 {
            return f64::from_bits(xd);
        }
        ex = xd;
        if (ex >> 52) == 0 {
            // x is subnormal too (x = 0 is impossible here)
            return as_hypot_denorm(ex, ey);
        }
        let nz = ey.leading_zeros();
        ey <<= nz - 11;
        ey &= !0u64 >> 12;
        ey = ey.wrapping_sub((u64::from(nz) - 12) << 52);
        yd = ey;
    }
    let de = xd.wrapping_sub(yd);
    if de > (27u64 << 52) {
        return libm::fma(f64::from_bits(0x3e40_0000_0000_0000), v, u); // 0x1p-27
    }
    let off = ((0x3ffu64 << 52) as i64).wrapping_sub((xd & emsk) as i64);
    xd = xd.wrapping_add(off as u64);
    yd = yd.wrapping_add(off as u64);
    let x = f64::from_bits(xd);
    let y = f64::from_bits(yd);
    let x2 = x * x;
    let dx2 = libm::fma(x, x, -x2);
    let y2 = y * y;
    let dy2 = libm::fma(y, y, -y2);
    let r2 = x2 + y2;
    let ir2 = 0.5 / r2;
    let dr2 = ((x2 - r2) + y2) + (dx2 + dy2);
    let th = libm::sqrt(r2);
    let rsqrt = th * ir2;
    let dz = dr2 - libm::fma(th, th, -r2);
    let tl = rsqrt * dz;
    let (th, tl) = fasttwosum(th, tl);
    let mut thd = th.to_bits();
    let tld = tl.abs().to_bits();
    ex = thd & (0x7ffu64 << 52);
    ey = tld;
    let aidr = ey.wrapping_add(0x3feu64 << 52).wrapping_sub(ex);
    let mid = aidr.wrapping_sub(0x3c90_0000_0000_0000).wrapping_add(16) >> 5;
    let midm = aidr.wrapping_sub(0x3c80_0000_0000_0000).wrapping_add(16) >> 5;
    if mid == 0 || midm == 0 || aidr < 0x39b0_0000_0000_0000 || aidr > 0x3c9f_ffff_ffff_ff80 {
        thd = as_hypot_hard(x, y).to_bits();
    }
    thd = thd.wrapping_sub(off as u64);
    if thd >= (0x7ffu64 << 52) {
        return as_hypot_overflow();
    }
    f64::from_bits(thd)
}

/// Correctly rounded `sqrt(x^2 + y^2)` for `f32` (round-to-nearest). CORE-MATH
/// `cr_hypotf` (`src/binary32/hypot/hypotf.c`): evaluated in `f64`, with an
/// exact correction where the double result sits too close to an `f32`
/// rounding boundary.
pub fn hypotf(x: f32, y: f32) -> f32 {
    let ax = x.abs();
    let ay = y.abs();
    let tx = ax.to_bits();
    let ty = ay.to_bits();
    if tx >= (0xffu32 << 23) || ty >= (0xffu32 << 23) {
        // either x or y is Inf or NaN
        let snan_x = tx > (0xffu32 << 23) && ((tx >> 22) & 1) == 0;
        let snan_y = ty > (0xffu32 << 23) && ((ty >> 22) & 1) == 0;
        if snan_x || snan_y {
            return x + y; // will return qNaN
        }
        if tx == (0xffu32 << 23) {
            return ax;
        }
        if ty == (0xffu32 << 23) {
            return ay;
        }
        return ax + ay;
    }
    // `__builtin_fmaxf`/`fminf`: neither argument is NaN here.
    let at = ax.max(ay);
    let ay = ax.min(ay);
    let xd = f64::from(at);
    let yd = f64::from(ay);
    let x2 = xd * xd;
    let y2 = yd * yd;
    let r2 = x2 + y2;
    if yd < xd * f64::from(f32::from_bits(0x397f_ffff)) {
        // 0x1.fffffep-13
        return libm::fmaf(f32::from_bits(0x3900_0000), ay, at); // 0x1p-13f
    }
    let r = libm::sqrt(r2);
    let mut t = r.to_bits();
    let c = r as f32;
    if t > 0x47ef_ffff_e000_0000 {
        // t > 0x1.fffffep+127
        return c;
    }
    if (t.wrapping_add(1) & 0xfff_ffff) > 2 {
        return c;
    }
    let cd = f64::from(c);
    if (cd * cd - x2) - y2 == 0.0 {
        return c;
    }
    let ir2 = 0.5 / r2;
    let dr2 = (x2 - r2) + y2;
    let rs = r * ir2;
    let dz = dr2 - libm::fma(r, r, -r2);
    let dr = rs * dz;
    let rh = r + dr;
    let rl = dr + (r - rh);
    t = rh.to_bits();
    if (t & 0xfff_ffff) == 0 {
        if rl > 0.0 {
            t += 1;
        }
        if rl < 0.0 {
            t -= 1;
        }
    }
    f64::from_bits(t) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The case gh:#993 found: musl/`libm` gives `2 - 2^-52`, the correctly
    /// rounded value is exactly 2.
    #[test]
    fn hypot_one_sqrt3_is_two() {
        let s3 = libm::sqrt(3.0);
        assert_eq!(hypot(1.0, s3), 2.0);
        assert_ne!(libm::hypot(1.0, s3), 2.0);
    }

    #[test]
    fn special_values() {
        let inf = f64::INFINITY;
        let nan = f64::NAN;
        assert_eq!(hypot(inf, nan), inf);
        assert_eq!(hypot(nan, -inf), inf);
        assert!(hypot(nan, 1.0).is_nan());
        assert_eq!(hypot(-0.0, 0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(hypot(3.0, -4.0), 5.0);
        assert_eq!(hypot(f64::MAX, f64::MAX), inf);
        let tiny = f64::from_bits(1);
        assert_eq!(hypot(tiny, tiny), tiny); // sqrt(2) * 2^-1074 rounds to 2^-1074
        assert_eq!(
            hypot(f64::from_bits(3), f64::from_bits(4)),
            f64::from_bits(5)
        );
        assert_eq!(hypotf(f32::INFINITY, f32::NAN), f32::INFINITY);
        assert!(hypotf(f32::NAN, 1.0).is_nan());
        assert_eq!(hypotf(3.0, -4.0), 5.0);
        assert_eq!(hypotf(f32::MAX, f32::MAX), f32::INFINITY);
        assert_eq!(
            hypotf(f32::from_bits(3), f32::from_bits(4)),
            f32::from_bits(5)
        );
    }
}
