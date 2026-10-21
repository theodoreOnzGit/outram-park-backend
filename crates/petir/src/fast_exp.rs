// Ported from ARM optimized-routines `math/exp.c` and `math/exp_data.c`,
// commit f2e4faf58c6c671154f472a76eaaa977bb36c870 (2026-09-09), read
// 2026-09-14.
//
// Copyright (c) 2018-2025, Arm Limited.                      (upstream)
// SPDX-License-Identifier: MIT OR Apache-2.0 WITH LLVM-exception
// Copyright (C) 2026 Theodore Ong and the outram-park contributors (this port)
//
// ARM optimized-routines is MIT OR Apache-2.0 WITH LLVM-exception -- both
// permissive into GPL-3.0. This derivative work is GPL-3.0-only.

//! A **fast**, deterministic `exp` — ported from ARM's optimized-routines.
//!
//! # Why this exists alongside [`crate::real::exp`]
//!
//! [`crate::real`] routes through the `libm` crate (a port of musl's), which
//! is portable and correct but **slower than the platform libm**: measured
//! 1.58x on `exp`, 1.41x on `log`, 3.51x on `powf`. That cost was the whole
//! reason `outram-mc-libs` routes its transcendentals behind an opt-in
//! feature rather than unconditionally.
//!
//! It turns out the cost is avoidable, because **the fast implementation glibc
//! itself ships is open source and permissively licensed**. glibc's
//! `exp`/`log`/`pow` are ARM's optimized-routines, contributed upstream.
//!
//! Measured **from Rust**, which is the only measurement that says anything
//! about this crate — 2 000 000 calls per route, `--release`, this host, via
//! `tests/fast_math_speed.rs`:
//!
//! ```text
//!            old libm route / ARM route      ARM route / platform
//!   exp            1.75 - 1.86x                   0.70 - 0.75x
//!   ln             1.09 - 1.18x                   1.04 - 1.25x
//!   powf           2.09 - 2.18x                   1.41 - 1.57x
//! ```
//!
//! Against the old `libm` route the fast path is **~1.8x** on `exp` and
//! **~2.1x** on `powf` — and only about **1.1x** on `ln`, which is worth
//! saying plainly rather than rounding up: the `libm` crate's `log` is already
//! good, and the portable (non-FMA) branch this port must take costs `log` and
//! `pow` real work that glibc's FMA build avoids. That is why `exp` here
//! actually beats the platform (it inlines, with no call through a dynamic
//! symbol) while `powf` sits about 1.5x behind it.
//!
//! **These three are `petir::real`'s default route**, not an opt-in:
//! [`crate::real::exp`], [`crate::real::ln`] and [`crate::real::powf`] call
//! straight into them.
//!
//! and they are close enough to glibc that the whole remaining difference is a
//! single rounding. Over 40 001 points spanning `[-700, 700]`, ARM's C built
//! **without** FMA differs from this host's glibc on **22 inputs (0.055 %),
//! each by exactly 1 ulp** — and differs nowhere when built **with** it.
//!
//! That 1 ulp is worth naming precisely, because the obvious reading of it is
//! wrong. It is not a defect in either implementation, nor a difference in
//! algorithm or constants: it is compiler contraction. glibc's ifunc dispatch
//! selects an FMA-compiled build of this same source, so `r2 * (C2 + r * C3)`
//! becomes one fused multiply-add instead of a multiply and an add. Rebuilding
//! ARM's C three ways isolates it, the third row being the control — hardware
//! present, contraction off, bit pattern back to the portable one:
//!
//! ```text
//!   gcc -O2                                   22 / 40001 differ, each 1 ulp
//!   gcc -O2 -mfma -mavx2 -ffp-contract=fast    0 / 40001
//!   gcc -O2 -mfma -mavx2 -ffp-contract=off    22 / 40001  (= the no-FMA build)
//! ```
//!
//! **This port tracks the no-FMA row, deliberately.** Rust never contracts
//! floating-point expressions — there is no `-ffp-contract=fast` equivalent
//! and `mul_add` has to be written out — so the portable form is the one a
//! Rust transcription can hold *on every target*, which is the property this
//! module exists for. Chasing glibc's FMA bit pattern instead would mean
//! `mul_add` throughout, which lowers to a libcall (correctly rounded, but
//! very slow) anywhere without hardware FMA, trading away the speed that is
//! the entire point.
//!
//! # What that buys, concretely
//!
//! Three things at once, which the `libm` route could only give two of:
//!
//! 1. **Speed** — glibc-class, because it *is* glibc's implementation.
//! 2. **Determinism** — one fixed implementation with no compiler-dependent
//!    contraction, so results are bit-identical on every platform, which is
//!    what `outram-mc-libs` wants for thread-count-independent k-eff and
//!    committed fixtures.
//! 3. **Near-zero re-baselining on Linux** — adopting it moves 0.055 % of
//!    `exp` calls by 1 ulp, where the `libm` route moved 9.64 % of them.
//!
//! # Fidelity
//!
//! The algorithm is `exp_inline` (`math/exp.c:60-134`) transcribed: the
//! table-driven `exp(x) = 2^(k/N) * exp(r)` decomposition with `N = 128`
//! (`EXP_TABLE_BITS = 7`) and the degree-5 polynomial branch
//! (`EXP_POLY_ORDER == 5`).
//!
//! The `TOINT_INTRINSICS` path is **not** taken: that is the aarch64
//! `roundtoint`/`converttoint` variant, and this port uses upstream's portable
//! `#else` branch (`exp.c:96-100`), which is what x86-64 builds use. Both
//! produce the same result; the intrinsic one is simply faster where the
//! hardware has it.
//!
//! The 256 table entries and 4 polynomial coefficients were **not** parsed out
//! of the C source: `exp_data.c` selects between `N == 64` and `N == 128`
//! variants with preprocessor conditionals, and hand-resolving those is how
//! transcription errors get in. Instead the compiled object was linked into a
//! dumper that printed the resolved `__exp_data` fields, and the Rust tables
//! were generated from that output.

use crate::error::{PetirError, Result};

/// `__exp_data.invln2N` — `1/ln2 * N`.
pub(crate) const INV_LN2N: f64 = 184.66496523378731;
/// `__exp_data.shift` — the round-to-int shift constant (`0x1.8p52`).
pub(crate) const SHIFT: f64 = 6755399441055744.0;
/// `__exp_data.negln2hiN` / `negln2loN` — `-ln2/N` split for accuracy.
pub(crate) const NEG_LN2_HI_N: f64 = -0.0054152123481117087;
pub(crate) const NEG_LN2_LO_N: f64 = -1.2864023111638346e-14;
/// `__exp_data.poly[0..4]` — C2..C5 for `EXP_POLY_ORDER == 5`.
pub(crate) const POLY: [f64; 4] = [
    0.49999999999996786,
    0.16666666666665886,
    0.041666680841067401,
    0.0083333358530595491,
];
/// `__exp_data.tab` — 2*N entries: `(tail_bits, scale_bits)` per index.
pub(crate) const TAB: [u64; 256] = [
    0x0000000000000000,
    0x3ff0000000000000,
    0x3c9b3b4f1a88bf6e,
    0x3feff63da9fb3335,
    0xbc7160139cd8dc5d,
    0x3fefec9a3e778061,
    0xbc905e7a108766d1,
    0x3fefe315e86e7f85,
    0x3c8cd2523567f613,
    0x3fefd9b0d3158574,
    0xbc8bce8023f98efa,
    0x3fefd06b29ddf6de,
    0x3c60f74e61e6c861,
    0x3fefc74518759bc8,
    0x3c90a3e45b33d399,
    0x3fefbe3ecac6f383,
    0x3c979aa65d837b6d,
    0x3fefb5586cf9890f,
    0x3c8eb51a92fdeffc,
    0x3fefac922b7247f7,
    0x3c3ebe3d702f9cd1,
    0x3fefa3ec32d3d1a2,
    0xbc6a033489906e0b,
    0x3fef9b66affed31b,
    0xbc9556522a2fbd0e,
    0x3fef9301d0125b51,
    0xbc5080ef8c4eea55,
    0x3fef8abdc06c31cc,
    0xbc91c923b9d5f416,
    0x3fef829aaea92de0,
    0x3c80d3e3e95c55af,
    0x3fef7a98c8a58e51,
    0xbc801b15eaa59348,
    0x3fef72b83c7d517b,
    0xbc8f1ff055de323d,
    0x3fef6af9388c8dea,
    0x3c8b898c3f1353bf,
    0x3fef635beb6fcb75,
    0xbc96d99c7611eb26,
    0x3fef5be084045cd4,
    0x3c9aecf73e3a2f60,
    0x3fef54873168b9aa,
    0xbc8fe782cb86389d,
    0x3fef4d5022fcd91d,
    0x3c8a6f4144a6c38d,
    0x3fef463b88628cd6,
    0x3c807a05b0e4047d,
    0x3fef3f49917ddc96,
    0x3c968efde3a8a894,
    0x3fef387a6e756238,
    0x3c875e18f274487d,
    0x3fef31ce4fb2a63f,
    0x3c80472b981fe7f2,
    0x3fef2b4565e27cdd,
    0xbc96b87b3f71085e,
    0x3fef24dfe1f56381,
    0x3c82f7e16d09ab31,
    0x3fef1e9df51fdee1,
    0xbc3d219b1a6fbffa,
    0x3fef187fd0dad990,
    0x3c8b3782720c0ab4,
    0x3fef1285a6e4030b,
    0x3c6e149289cecb8f,
    0x3fef0cafa93e2f56,
    0x3c834d754db0abb6,
    0x3fef06fe0a31b715,
    0x3c864201e2ac744c,
    0x3fef0170fc4cd831,
    0x3c8fdd395dd3f84a,
    0x3feefc08b26416ff,
    0xbc86a3803b8e5b04,
    0x3feef6c55f929ff1,
    0xbc924aedcc4b5068,
    0x3feef1a7373aa9cb,
    0xbc9907f81b512d8e,
    0x3feeecae6d05d866,
    0xbc71d1e83e9436d2,
    0x3feee7db34e59ff7,
    0xbc991919b3ce1b15,
    0x3feee32dc313a8e5,
    0x3c859f48a72a4c6d,
    0x3feedea64c123422,
    0xbc9312607a28698a,
    0x3feeda4504ac801c,
    0xbc58a78f4817895b,
    0x3feed60a21f72e2a,
    0xbc7c2c9b67499a1b,
    0x3feed1f5d950a897,
    0x3c4363ed60c2ac11,
    0x3feece086061892d,
    0x3c9666093b0664ef,
    0x3feeca41ed1d0057,
    0x3c6ecce1daa10379,
    0x3feec6a2b5c13cd0,
    0x3c93ff8e3f0f1230,
    0x3feec32af0d7d3de,
    0x3c7690cebb7aafb0,
    0x3feebfdad5362a27,
    0x3c931dbdeb54e077,
    0x3feebcb299fddd0d,
    0xbc8f94340071a38e,
    0x3feeb9b2769d2ca7,
    0xbc87deccdc93a349,
    0x3feeb6daa2cf6642,
    0xbc78dec6bd0f385f,
    0x3feeb42b569d4f82,
    0xbc861246ec7b5cf6,
    0x3feeb1a4ca5d920f,
    0x3c93350518fdd78e,
    0x3feeaf4736b527da,
    0x3c7b98b72f8a9b05,
    0x3feead12d497c7fd,
    0x3c9063e1e21c5409,
    0x3feeab07dd485429,
    0x3c34c7855019c6ea,
    0x3feea9268a5946b7,
    0x3c9432e62b64c035,
    0x3feea76f15ad2148,
    0xbc8ce44a6199769f,
    0x3feea5e1b976dc09,
    0xbc8c33c53bef4da8,
    0x3feea47eb03a5585,
    0xbc845378892be9ae,
    0x3feea34634ccc320,
    0xbc93cedd78565858,
    0x3feea23882552225,
    0x3c5710aa807e1964,
    0x3feea155d44ca973,
    0xbc93b3efbf5e2228,
    0x3feea09e667f3bcd,
    0xbc6a12ad8734b982,
    0x3feea012750bdabf,
    0xbc6367efb86da9ee,
    0x3fee9fb23c651a2f,
    0xbc80dc3d54e08851,
    0x3fee9f7df9519484,
    0xbc781f647e5a3ecf,
    0x3fee9f75e8ec5f74,
    0xbc86ee4ac08b7db0,
    0x3fee9f9a48a58174,
    0xbc8619321e55e68a,
    0x3fee9feb564267c9,
    0x3c909ccb5e09d4d3,
    0x3feea0694fde5d3f,
    0xbc7b32dcb94da51d,
    0x3feea11473eb0187,
    0x3c94ecfd5467c06b,
    0x3feea1ed0130c132,
    0x3c65ebe1abd66c55,
    0x3feea2f336cf4e62,
    0xbc88a1c52fb3cf42,
    0x3feea427543e1a12,
    0xbc9369b6f13b3734,
    0x3feea589994cce13,
    0xbc805e843a19ff1e,
    0x3feea71a4623c7ad,
    0xbc94d450d872576e,
    0x3feea8d99b4492ed,
    0x3c90ad675b0e8a00,
    0x3feeaac7d98a6699,
    0x3c8db72fc1f0eab4,
    0x3feeace5422aa0db,
    0xbc65b6609cc5e7ff,
    0x3feeaf3216b5448c,
    0x3c7bf68359f35f44,
    0x3feeb1ae99157736,
    0xbc93091fa71e3d83,
    0x3feeb45b0b91ffc6,
    0xbc5da9b88b6c1e29,
    0x3feeb737b0cdc5e5,
    0xbc6c23f97c90b959,
    0x3feeba44cbc8520f,
    0xbc92434322f4f9aa,
    0x3feebd829fde4e50,
    0xbc85ca6cd7668e4b,
    0x3feec0f170ca07ba,
    0x3c71affc2b91ce27,
    0x3feec49182a3f090,
    0x3c6dd235e10a73bb,
    0x3feec86319e32323,
    0xbc87c50422622263,
    0x3feecc667b5de565,
    0x3c8b1c86e3e231d5,
    0x3feed09bec4a2d33,
    0xbc91bbd1d3bcbb15,
    0x3feed503b23e255d,
    0x3c90cc319cee31d2,
    0x3feed99e1330b358,
    0x3c8469846e735ab3,
    0x3feede6b5579fdbf,
    0xbc82dfcd978e9db4,
    0x3feee36bbfd3f37a,
    0x3c8c1a7792cb3387,
    0x3feee89f995ad3ad,
    0xbc907b8f4ad1d9fa,
    0x3feeee07298db666,
    0xbc55c3d956dcaeba,
    0x3feef3a2b84f15fb,
    0xbc90a40e3da6f640,
    0x3feef9728de5593a,
    0xbc68d6f438ad9334,
    0x3feeff76f2fb5e47,
    0xbc91eee26b588a35,
    0x3fef05b030a1064a,
    0x3c74ffd70a5fddcd,
    0x3fef0c1e904bc1d2,
    0xbc91bdfbfa9298ac,
    0x3fef12c25bd71e09,
    0x3c736eae30af0cb3,
    0x3fef199bdd85529c,
    0x3c8ee3325c9ffd94,
    0x3fef20ab5fffd07a,
    0x3c84e08fd10959ac,
    0x3fef27f12e57d14b,
    0x3c63cdaf384e1a67,
    0x3fef2f6d9406e7b5,
    0x3c676b2c6c921968,
    0x3fef3720dcef9069,
    0xbc808a1883ccb5d2,
    0x3fef3f0b555dc3fa,
    0xbc8fad5d3ffffa6f,
    0x3fef472d4a07897c,
    0xbc900dae3875a949,
    0x3fef4f87080d89f2,
    0x3c74a385a63d07a7,
    0x3fef5818dcfba487,
    0xbc82919e2040220f,
    0x3fef60e316c98398,
    0x3c8e5a50d5c192ac,
    0x3fef69e603db3285,
    0x3c843a59ac016b4b,
    0x3fef7321f301b460,
    0xbc82d52107b43e1f,
    0x3fef7c97337b9b5f,
    0xbc892ab93b470dc9,
    0x3fef864614f5a129,
    0x3c74b604603a88d3,
    0x3fef902ee78b3ff6,
    0x3c83c5ec519d7271,
    0x3fef9a51fbc74c83,
    0xbc8ff7128fd391f0,
    0x3fefa4afa2a490da,
    0xbc8dae98e223747d,
    0x3fefaf482d8e67f1,
    0x3c8ec3bc41aa2008,
    0x3fefba1bee615a27,
    0x3c842b94c3a9eb32,
    0x3fefc52b376bba97,
    0x3c8a64a931d185ee,
    0x3fefd0765b6e4540,
    0xbc8e37bae43be3ed,
    0x3fefdbfdad9cbe14,
    0x3c77893b4d91cd9d,
    0x3fefe7c1819e90d8,
    0x3c5305c14160cc89,
    0x3feff3c22b8f71f1,
];
/// `EXP_TABLE_BITS`.
pub(crate) const TABLE_BITS: u32 = 7;
/// `N = 1 << EXP_TABLE_BITS`.
pub(crate) const N: u64 = 1 << TABLE_BITS;

/// `top12(x)` (`math_config.h`) — the top 12 bits of the double's encoding.
#[inline]
pub(crate) fn top12(x: f64) -> u32 {
    (x.to_bits() >> 52) as u32
}

/// The transcription itself — `exp_inline(x, 0)` (`math/exp.c:60`), which is
/// what upstream's `exp()` calls.
///
/// Returns `Err((error, ieee_value))` on overflow or underflow, carrying both
/// PETIR's error and the value upstream's C returns, so that [`exp`] and
/// [`exp_ieee`] are two views of one implementation rather than two
/// implementations.
///
/// # Verification
///
/// Checked against upstream's own compiled C over 8 345 probes crossing every
/// branch — the table path over `[-700, 700]`, the tiny-`x` early return, and
/// both sides of `specialcase` at the overflow/underflow boundaries:
/// **8 290 compared, 8 290 bit-identical (100.000 %)**, 55 refused at the
/// range edges with upstream returning an infinity or a zero at each
/// (`tests/fast_exp_vs_arm_optimized_routines.rs`, 2026-09-14). Against the
/// platform `exp` on this glibc host, 99.964 % bit-identical with a worst
/// relative difference of 1.390e-16 — one FMA contraction, see the module
/// docs.
///
/// # Errors
/// [`PetirError::Overflow`] where upstream returns `__math_oflow` (the result
/// exceeds the double range), and [`PetirError::Underflow`] where it returns
/// `__math_uflow` (the result is below the smallest normal). Upstream signals
/// these through errno and a returned infinity/zero; this port returns them,
/// consistent with the rest of PETIR.
fn exp_inner(x: f64) -> core::result::Result<f64, (PetirError, f64)> {
    let mut abstop = top12(x) & 0x7ff;

    // `abstop - top12(0x1p-54) >= top12(512.0) - top12(0x1p-54)` -- the
    // unsigned-wraparound trick upstream uses to catch both tiny and huge |x|
    // in one compare (`exp.c:68`).
    if abstop.wrapping_sub(top12(f64::from_bits(0x3c90000000000000)))
        >= top12(512.0).wrapping_sub(top12(f64::from_bits(0x3c90000000000000)))
    {
        if abstop.wrapping_sub(top12(f64::from_bits(0x3c90000000000000))) >= 0x80000000 {
            // Tiny x: avoid spurious underflow. 0 is a common input.
            return Ok(1.0 + x);
        }
        if abstop >= top12(1024.0) {
            if x.to_bits() == f64::NEG_INFINITY.to_bits() {
                return Ok(0.0);
            }
            if abstop >= top12(f64::INFINITY) {
                return Ok(1.0 + x);
            }
            // `__math_uflow(0)` returns +0.0 and `__math_oflow(0)` returns
            // +inf; both also raise the corresponding IEEE flag, which Rust
            // has no portable way to do.
            return if x.to_bits() >> 63 != 0 {
                Err((PetirError::Underflow, 0.0))
            } else {
                Err((PetirError::Overflow, f64::INFINITY))
            };
        }
        // Large x is special-cased below.
        abstop = 0;
    }

    // exp(x) = 2^(k/N) * exp(r), r in [-ln2/2N, ln2/2N].
    let z = INV_LN2N * x;
    // Portable rounding (`exp.c:96-100`); the TOINT_INTRINSICS path is aarch64.
    let mut kd = z + SHIFT;
    let ki = kd.to_bits();
    kd -= SHIFT;

    let r = x + kd * NEG_LN2_HI_N + kd * NEG_LN2_LO_N;
    let idx = (2 * (ki % N)) as usize;
    let top = ki << (52 - TABLE_BITS);
    // `idx = 2 * (ki % N)` and `TAB` holds `2 * N` entries, so both reads are
    // in range for every input -- but by arithmetic the compiler cannot
    // follow. Fetching through `get` keeps the values (and therefore the
    // bit-identity with upstream, pinned by
    // tests/fast_exp_vs_arm_optimized_routines.rs) exactly as they were,
    // while removing the panic the subscripts carried.
    let (Some(&tab_lo), Some(&tab_hi)) = (TAB.get(idx), TAB.get(idx + 1)) else {
        return Err((PetirError::Range, f64::NAN));
    };
    let tail = f64::from_bits(tab_lo);
    // Valid as a scale only for -1023*N < k < 1024*N.
    let sbits = tab_hi.wrapping_add(top);

    let r2 = r * r;
    // EXP_POLY_ORDER == 5.
    let tmp = tail + r + r2 * (POLY[0] + r * POLY[1]) + r2 * r2 * (POLY[2] + r * POLY[3]);

    if abstop == 0 {
        return specialcase(tmp, sbits, ki);
    }
    let scale = f64::from_bits(sbits);
    // tmp == 0 or |tmp| > 2^-200 and scale > 2^-739, so no spurious underflow.
    Ok(scale + scale * tmp)
}

/// `specialcase` (`math/exp.c:33-58`) — the large-|x| branch, where the scale
/// must be applied in two steps to avoid overflowing the exponent or losing
/// precision in the subnormal range.
///
/// Transcribed line for line. An earlier draft of this function was
/// *reconstructed* rather than read, and got four things wrong at once: the
/// sign test is on `ki & 0x8000_0000` (the low 32 bits), not the 64-bit sign
/// bit; the up-scale is `0x1p1009`, not `0x1p9`; the subnormal guard is
/// `y < 1.0`, not `|y| < 1.0`; and the rounding step uses the literal `1.0`
/// rather than a signed `±1.0`. The test against glibc caught it at 72.8 %
/// bit-identical with an infinite relative error at `x = -700`.
fn specialcase(tmp: f64, sbits: u64, ki: u64) -> core::result::Result<f64, (PetirError, f64)> {
    // `0x1p1009` — 2^1009, the counterpart to the `1009 << 52` bias removal.
    const P1009: f64 = f64::from_bits(0x7F00000000000000);

    if ki & 0x8000_0000 == 0 {
        // k > 0: the exponent of scale might have overflowed by <= 460.
        let sbits = sbits.wrapping_sub(1009u64 << 52);
        let scale = f64::from_bits(sbits);
        let y = P1009 * (scale + scale * tmp);
        // check_oflow
        return if y.is_infinite() {
            Err((PetirError::Overflow, y))
        } else {
            Ok(y)
        };
    }

    // k < 0: needs care in the subnormal range.
    let sbits = sbits.wrapping_add(1022u64 << 52);
    let scale = f64::from_bits(sbits);
    let mut y = scale + scale * tmp;
    if y < 1.0 {
        // Round y to the right precision before scaling it into the subnormal
        // range, to avoid a double rounding that costs 0.5 + E/2 ulp.
        let lo0 = scale - y + scale * tmp;
        let hi = 1.0 + y;
        let lo = 1.0 - hi + y + lo0;
        y = (hi + lo) - 1.0;
        // Avoid -0.0 with downward rounding.
        if y == 0.0 {
            y = 0.0;
        }
    }
    let y = f64::MIN_POSITIVE * y; // 0x1p-1022
                                   // check_uflow
    if y == 0.0 {
        Err((PetirError::Underflow, y))
    } else {
        Ok(y)
    }
}

/// Fast `e^x`, refusing results outside the double range.
///
/// This is [`exp_ieee`] with upstream's overflow and underflow returns turned
/// into errors, which is PETIR's convention elsewhere.
///
/// # Errors
///
/// [`PetirError::Overflow`] where upstream returns `__math_oflow` (the
/// result exceeds the double range; upstream returns `+inf`), and
/// [`PetirError::Underflow`] where it returns `__math_uflow` (the result is
/// below the smallest subnormal; upstream returns `+0.0`).
#[inline]
pub fn exp(x: f64) -> Result<f64> {
    exp_inner(x).map_err(|(e, _)| e)
}

/// Fast `e^x`, returning exactly what upstream's C returns — `+inf` on
/// overflow, `+0.0` on underflow — rather than an error.
///
/// This is the faithful surface, and the one to prefer. ARM's `exp` is an
/// `f64 -> f64` function that signals overflow and underflow through the IEEE
/// flags, not through its return value; Rust has no portable access to those
/// flags, so [`exp`] reports them as errors instead. A caller that just wants
/// the IEEE-754 answer — [`crate::real::exp`], or `outram-mc-libs`'
/// `RealMath`, whose methods return `f64` — should use this and skip the
/// round-trip through `Result`.
#[inline]
pub fn exp_ieee(x: f64) -> f64 {
    exp_inner(x).unwrap_or_else(|(_, v)| v)
}
