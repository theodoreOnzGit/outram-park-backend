// Ported from ARM optimized-routines `math/pow.c`, `math/pow_common.h` and
// `math/pow_log_data.c`, commit f2e4faf58c6c671154f472a76eaaa977bb36c870
// (2026-09-09), read 2026-09-14.
//
// Copyright (c) 2018-2026, Arm Limited.                      (upstream)
// SPDX-License-Identifier: MIT OR Apache-2.0 WITH LLVM-exception
// Copyright (C) 2026 Theodore Ong and the outram-park contributors (this port)
//
// ARM optimized-routines is MIT OR Apache-2.0 WITH LLVM-exception -- both
// permissive into GPL-3.0. This derivative work is GPL-3.0-only.

//! A **fast**, deterministic `x^y` — ported from ARM's optimized-routines, the
//! implementation glibc itself ships.
//!
//! The third of the trio with [`crate::fast_exp`] and [`crate::fast_log`], and
//! the one with the most to gain: `pow` is where the musl-derived `libm` route
//! is slowest by a wide margin. Measured from Rust over 2 000 000 calls
//! (`tests/fast_math_speed.rs`, five runs): a **2.09-2.18x** speed-up, the
//! largest of the three. It is [`crate::real::powf`]'s default route.
//!
//! It still sits about 1.5x behind the platform, and the reason is the
//! deliberate one: `HAVE_FAST_FMA == 0` replaces three fused multiply-adds
//! with explicit hi/lo splits in `log_inline` and in the `y * log(x)` product,
//! which is real extra work that glibc's FMA build does not do. Read
//! [`crate::fast_exp`]'s module docs for why tracking the portable form is the
//! choice anyway.
//!
//! # Fidelity
//!
//! `pow` (`math/pow.c:246-347`) transcribed, along with the two static
//! helpers it is built from — `log_inline` (`pow.c:30-96`) and `exp_inline`
//! (`pow.c:165-243`) — and `checkint` / `zeroinfnan` from `pow_common.h`.
//! Resolved configuration, read off the *compiled* object rather than chosen
//! by eye:
//!
//! ```text
//!   POW_LOG_TABLE_BITS   7      (N = 128)
//!   POW_LOG_POLY_ORDER   8
//!   EXP_TABLE_BITS       7
//!   EXP_POLY_ORDER       5
//!   HAVE_FAST_FMA        0
//!   EXP_USE_TOINT_NARROW 0
//!   TOINT_INTRINSICS     0      (aarch64 only)
//! ```
//!
//! # Why `exp_inline` is duplicated here rather than shared with `fast_exp`
//!
//! Because upstream duplicates it, and the two copies are **not the same
//! function**. `pow.c`'s version takes a `sign_bias` and an `xtail`, and both
//! it and its `specialcase` differ from `exp.c`'s in ways that matter:
//!
//! - `exp.c`'s large-|x| branch still has to handle infinity and NaN;
//!   `pow.c`'s is reached only after `pow` has already excluded them, so it
//!   goes straight to overflow/underflow (`pow.c:183-189`).
//! - `exp.c`'s `specialcase` is the unsigned variant — it tests `y < 1.0` and
//!   rounds against the literal `1.0`. `pow.c`'s is the **signed** variant:
//!   `fabs(y) < 1.0`, `one = ±1.0`, and it restores the sign of a zero result
//!   from `sbits` (`pow.c:138-158`).
//!
//! Sharing one implementation between them would mean choosing one of those
//! behaviours for both, which is exactly the kind of "tidying" that silently
//! changes results. Only the *data* is shared, from [`crate::fast_exp`].
//!
//! The 512 `pow_log_data.tab` entries and 7 polynomial coefficients were
//! extracted mechanically — the compiled object linked into a dumper that
//! printed the resolved `__pow_log_data` fields as bit patterns — not parsed
//! out of the C.

use crate::error::{PetirError, Result};
use crate::fast_exp::{
    top12, INV_LN2N, N as EXP_N, NEG_LN2_HI_N, NEG_LN2_LO_N, POLY as EXP_POLY, SHIFT,
    TAB as EXP_TAB, TABLE_BITS as EXP_TABLE_BITS,
};

/// `POW_LOG_TABLE_BITS`.
const LOG_TABLE_BITS: u32 = 7;
/// `N = 1 << POW_LOG_TABLE_BITS`.
const LOG_N: u64 = 1 << LOG_TABLE_BITS;
/// `OFF` (`pow.c:25`) — the interval split point, different from `log.c`'s.
const LOG_OFF: u64 = 0x3fe6_9555_0000_0000;
/// `SIGN_BIAS = 0x800 << EXP_TABLE_BITS` (`pow.c:161`).
const SIGN_BIAS: u32 = 0x800 << EXP_TABLE_BITS;

/// `__pow_log_data.ln2hi` / `ln2lo` — `ln 2` split for accuracy.
const POW_LN2_HI: f64 = f64::from_bits(0x3fe62e42fefa3800);
const POW_LN2_LO: f64 = f64::from_bits(0x3d2ef35793c76730);

/// `__pow_log_data.poly[0..7]` — `POW_LOG_POLY_ORDER == 8`; the first
/// coefficient of the series is an implicit 1, so `POW_POLY[0] == -0.5`.
const POW_POLY: [f64; 7] = [
    f64::from_bits(0xbfe0000000000000),
    f64::from_bits(0xbfe5555555555560),
    f64::from_bits(0x3fe0000000000006),
    f64::from_bits(0x3fe999999959554e),
    f64::from_bits(0xbfe555555529a47a),
    f64::from_bits(0xbff2495b9b4845e9),
    f64::from_bits(0x3ff0002b8b263fc3),
];

/// `__pow_log_data.tab` — N entries of `(invc, logc, logctail)` as bit
/// patterns. Upstream's struct also carries an unused `pad` field purely to
/// make its indexing a power of two; it is dropped here.
const POW_TAB: [u64; 384] = [
    0x3ff6a00000000000,
    0xbfd62c82f2b9c800,
    0x3cfab42428375680,
    0x3ff6800000000000,
    0xbfd5d1bdbf580800,
    0xbd1ca508d8e0f720,
    0x3ff6600000000000,
    0xbfd5767717455800,
    0xbd2362a4d5b6506d,
    0x3ff6400000000000,
    0xbfd51aad872df800,
    0xbce684e49eb067d5,
    0x3ff6200000000000,
    0xbfd4be5f95777800,
    0xbd041b6993293ee0,
    0x3ff6000000000000,
    0xbfd4618bc21c6000,
    0x3d13d82f484c84cc,
    0x3ff5e00000000000,
    0xbfd404308686a800,
    0x3cdc42f3ed820b3a,
    0x3ff5c00000000000,
    0xbfd3a64c55694800,
    0x3d20b1c686519460,
    0x3ff5a00000000000,
    0xbfd347dd9a988000,
    0x3d25594dd4c58092,
    0x3ff5800000000000,
    0xbfd2e8e2bae12000,
    0x3d267b1e99b72bd8,
    0x3ff5600000000000,
    0xbfd2895a13de8800,
    0x3d15ca14b6cfb03f,
    0x3ff5600000000000,
    0xbfd2895a13de8800,
    0x3d15ca14b6cfb03f,
    0x3ff5400000000000,
    0xbfd22941fbcf7800,
    0xbd165a242853da76,
    0x3ff5200000000000,
    0xbfd1c898c1699800,
    0xbd1fafbc68e75404,
    0x3ff5000000000000,
    0xbfd1675cababa800,
    0x3d1f1fc63382a8f0,
    0x3ff4e00000000000,
    0xbfd1058bf9ae4800,
    0xbd26a8c4fd055a66,
    0x3ff4c00000000000,
    0xbfd0a324e2739000,
    0xbd0c6bee7ef4030e,
    0x3ff4a00000000000,
    0xbfd0402594b4d000,
    0xbcf036b89ef42d7f,
    0x3ff4a00000000000,
    0xbfd0402594b4d000,
    0xbcf036b89ef42d7f,
    0x3ff4800000000000,
    0xbfcfb9186d5e4000,
    0x3d0d572aab993c87,
    0x3ff4600000000000,
    0xbfcef0adcbdc6000,
    0x3d2b26b79c86af24,
    0x3ff4400000000000,
    0xbfce27076e2af000,
    0xbd172f4f543fff10,
    0x3ff4200000000000,
    0xbfcd5c216b4fc000,
    0x3d21ba91bbca681b,
    0x3ff4000000000000,
    0xbfcc8ff7c79aa000,
    0x3d27794f689f8434,
    0x3ff4000000000000,
    0xbfcc8ff7c79aa000,
    0x3d27794f689f8434,
    0x3ff3e00000000000,
    0xbfcbc286742d9000,
    0x3d194eb0318bb78f,
    0x3ff3c00000000000,
    0xbfcaf3c94e80c000,
    0x3cba4e633fcd9066,
    0x3ff3a00000000000,
    0xbfca23bc1fe2b000,
    0xbd258c64dc46c1ea,
    0x3ff3a00000000000,
    0xbfca23bc1fe2b000,
    0xbd258c64dc46c1ea,
    0x3ff3800000000000,
    0xbfc9525a9cf45000,
    0xbd2ad1d904c1d4e3,
    0x3ff3600000000000,
    0xbfc87fa06520d000,
    0x3d2bbdbf7fdbfa09,
    0x3ff3400000000000,
    0xbfc7ab890210e000,
    0x3d2bdb9072534a58,
    0x3ff3400000000000,
    0xbfc7ab890210e000,
    0x3d2bdb9072534a58,
    0x3ff3200000000000,
    0xbfc6d60fe719d000,
    0xbd10e46aa3b2e266,
    0x3ff3000000000000,
    0xbfc5ff3070a79000,
    0xbd1e9e439f105039,
    0x3ff3000000000000,
    0xbfc5ff3070a79000,
    0xbd1e9e439f105039,
    0x3ff2e00000000000,
    0xbfc526e5e3a1b000,
    0xbd20de8b90075b8f,
    0x3ff2c00000000000,
    0xbfc44d2b6ccb8000,
    0x3d170cc16135783c,
    0x3ff2c00000000000,
    0xbfc44d2b6ccb8000,
    0x3d170cc16135783c,
    0x3ff2a00000000000,
    0xbfc371fc201e9000,
    0x3cf178864d27543a,
    0x3ff2800000000000,
    0xbfc29552f81ff000,
    0xbd248d301771c408,
    0x3ff2600000000000,
    0xbfc1b72ad52f6000,
    0xbd2e80a41811a396,
    0x3ff2600000000000,
    0xbfc1b72ad52f6000,
    0xbd2e80a41811a396,
    0x3ff2400000000000,
    0xbfc0d77e7cd09000,
    0x3d0a699688e85bf4,
    0x3ff2400000000000,
    0xbfc0d77e7cd09000,
    0x3d0a699688e85bf4,
    0x3ff2200000000000,
    0xbfbfec9131dbe000,
    0xbd2575545ca333f2,
    0x3ff2000000000000,
    0xbfbe27076e2b0000,
    0x3d2a342c2af0003c,
    0x3ff2000000000000,
    0xbfbe27076e2b0000,
    0x3d2a342c2af0003c,
    0x3ff1e00000000000,
    0xbfbc5e548f5bc000,
    0xbd1d0c57585fbe06,
    0x3ff1c00000000000,
    0xbfba926d3a4ae000,
    0x3d253935e85baac8,
    0x3ff1c00000000000,
    0xbfba926d3a4ae000,
    0x3d253935e85baac8,
    0x3ff1a00000000000,
    0xbfb8c345d631a000,
    0x3d137c294d2f5668,
    0x3ff1a00000000000,
    0xbfb8c345d631a000,
    0x3d137c294d2f5668,
    0x3ff1800000000000,
    0xbfb6f0d28ae56000,
    0xbd269737c93373da,
    0x3ff1600000000000,
    0xbfb51b073f062000,
    0x3d1f025b61c65e57,
    0x3ff1600000000000,
    0xbfb51b073f062000,
    0x3d1f025b61c65e57,
    0x3ff1400000000000,
    0xbfb341d7961be000,
    0x3d2c5edaccf913df,
    0x3ff1400000000000,
    0xbfb341d7961be000,
    0x3d2c5edaccf913df,
    0x3ff1200000000000,
    0xbfb16536eea38000,
    0x3d147c5e768fa309,
    0x3ff1000000000000,
    0xbfaf0a30c0118000,
    0x3d2d599e83368e91,
    0x3ff1000000000000,
    0xbfaf0a30c0118000,
    0x3d2d599e83368e91,
    0x3ff0e00000000000,
    0xbfab42dd71198000,
    0x3d1c827ae5d6704c,
    0x3ff0e00000000000,
    0xbfab42dd71198000,
    0x3d1c827ae5d6704c,
    0x3ff0c00000000000,
    0xbfa77458f632c000,
    0xbd2cfc4634f2a1ee,
    0x3ff0c00000000000,
    0xbfa77458f632c000,
    0xbd2cfc4634f2a1ee,
    0x3ff0a00000000000,
    0xbfa39e87b9fec000,
    0x3cf502b7f526feaa,
    0x3ff0a00000000000,
    0xbfa39e87b9fec000,
    0x3cf502b7f526feaa,
    0x3ff0800000000000,
    0xbf9f829b0e780000,
    0xbd2980267c7e09e4,
    0x3ff0800000000000,
    0xbf9f829b0e780000,
    0xbd2980267c7e09e4,
    0x3ff0600000000000,
    0xbf97b91b07d58000,
    0xbd288d5493faa639,
    0x3ff0400000000000,
    0xbf8fc0a8b0fc0000,
    0xbcdf1e7cf6d3a69c,
    0x3ff0400000000000,
    0xbf8fc0a8b0fc0000,
    0xbcdf1e7cf6d3a69c,
    0x3ff0200000000000,
    0xbf7fe02a6b100000,
    0xbd19e23f0dda40e4,
    0x3ff0200000000000,
    0xbf7fe02a6b100000,
    0xbd19e23f0dda40e4,
    0x3ff0000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x3ff0000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x3fefc00000000000,
    0x3f80101575890000,
    0xbd10c76b999d2be8,
    0x3fef800000000000,
    0x3f90205658938000,
    0xbd23dc5b06e2f7d2,
    0x3fef400000000000,
    0x3f98492528c90000,
    0xbd2aa0ba325a0c34,
    0x3fef000000000000,
    0x3fa0415d89e74000,
    0x3d0111c05cf1d753,
    0x3feec00000000000,
    0x3fa466aed42e0000,
    0xbd2c167375bdfd28,
    0x3fee800000000000,
    0x3fa894aa149fc000,
    0xbd197995d05a267d,
    0x3fee400000000000,
    0x3faccb73cdddc000,
    0xbd1a68f247d82807,
    0x3fee200000000000,
    0x3faeea31c006c000,
    0xbd0e113e4fc93b7b,
    0x3fede00000000000,
    0x3fb1973bd1466000,
    0xbd25325d560d9e9b,
    0x3feda00000000000,
    0x3fb3bdf5a7d1e000,
    0x3d2cc85ea5db4ed7,
    0x3fed600000000000,
    0x3fb5e95a4d97a000,
    0xbd2c69063c5d1d1e,
    0x3fed400000000000,
    0x3fb700d30aeac000,
    0x3cec1e8da99ded32,
    0x3fed000000000000,
    0x3fb9335e5d594000,
    0x3d23115c3abd47da,
    0x3fecc00000000000,
    0x3fbb6ac88dad6000,
    0xbd1390802bf768e5,
    0x3feca00000000000,
    0x3fbc885801bc4000,
    0x3d2646d1c65aacd3,
    0x3fec600000000000,
    0x3fbec739830a2000,
    0xbd2dc068afe645e0,
    0x3fec400000000000,
    0x3fbfe89139dbe000,
    0xbd2534d64fa10afd,
    0x3fec000000000000,
    0x3fc1178e8227e000,
    0x3d21ef78ce2d07f2,
    0x3febe00000000000,
    0x3fc1aa2b7e23f000,
    0x3d2ca78e44389934,
    0x3feba00000000000,
    0x3fc2d1610c868000,
    0x3d039d6ccb81b4a1,
    0x3feb800000000000,
    0x3fc365fcb0159000,
    0x3cc62fa8234b7289,
    0x3feb400000000000,
    0x3fc4913d8333b000,
    0x3d25837954fdb678,
    0x3feb200000000000,
    0x3fc527e5e4a1b000,
    0x3d2633e8e5697dc7,
    0x3feae00000000000,
    0x3fc6574ebe8c1000,
    0x3d19cf8b2c3c2e78,
    0x3feac00000000000,
    0x3fc6f0128b757000,
    0xbd25118de59c21e1,
    0x3feaa00000000000,
    0x3fc7898d85445000,
    0xbd1c661070914305,
    0x3fea600000000000,
    0x3fc8beafeb390000,
    0xbd073d54aae92cd1,
    0x3fea400000000000,
    0x3fc95a5adcf70000,
    0x3d07f22858a0ff6f,
    0x3fea000000000000,
    0x3fca93ed3c8ae000,
    0xbd28724350562169,
    0x3fe9e00000000000,
    0x3fcb31d8575bd000,
    0xbd0c358d4eace1aa,
    0x3fe9c00000000000,
    0x3fcbd087383be000,
    0xbd2d4bc4595412b6,
    0x3fe9a00000000000,
    0x3fcc6ffbc6f01000,
    0xbcf1ec72c5962bd2,
    0x3fe9600000000000,
    0x3fcdb13db0d49000,
    0xbd2aff2af715b035,
    0x3fe9400000000000,
    0x3fce530effe71000,
    0x3cc212276041f430,
    0x3fe9200000000000,
    0x3fcef5ade4dd0000,
    0xbcca211565bb8e11,
    0x3fe9000000000000,
    0x3fcf991c6cb3b000,
    0x3d1bcbecca0cdf30,
    0x3fe8c00000000000,
    0x3fd07138604d5800,
    0x3cf89cdb16ed4e91,
    0x3fe8a00000000000,
    0x3fd0c42d67616000,
    0x3d27188b163ceae9,
    0x3fe8800000000000,
    0x3fd1178e8227e800,
    0xbd2c210e63a5f01c,
    0x3fe8600000000000,
    0x3fd16b5ccbacf800,
    0x3d2b9acdf7a51681,
    0x3fe8400000000000,
    0x3fd1bf99635a6800,
    0x3d2ca6ed5147bdb7,
    0x3fe8200000000000,
    0x3fd214456d0eb800,
    0x3d0a87deba46baea,
    0x3fe7e00000000000,
    0x3fd2bef07cdc9000,
    0x3d2a9cfa4a5004f4,
    0x3fe7c00000000000,
    0x3fd314f1e1d36000,
    0xbd28e27ad3213cb8,
    0x3fe7a00000000000,
    0x3fd36b6776be1000,
    0x3d116ecdb0f177c8,
    0x3fe7800000000000,
    0x3fd3c25277333000,
    0x3d183b54b606bd5c,
    0x3fe7600000000000,
    0x3fd419b423d5e800,
    0x3d08e436ec90e09d,
    0x3fe7400000000000,
    0x3fd4718dc271c800,
    0xbd2f27ce0967d675,
    0x3fe7200000000000,
    0x3fd4c9e09e173000,
    0xbd2e20891b0ad8a4,
    0x3fe7000000000000,
    0x3fd522ae0738a000,
    0x3d2ebe708164c759,
    0x3fe6e00000000000,
    0x3fd57bf753c8d000,
    0x3d1fadedee5d40ef,
    0x3fe6c00000000000,
    0x3fd5d5bddf596000,
    0xbd0a0b2a08a465dc,
];

/// `log_inline` (`pow.c:30-96`) — computes `y + tail = log(x)` to about 15
/// bits more than double precision, which is what makes `pow` accurate to
/// 0.54 ulp across the whole range.
///
/// `ix` is the bit representation of `x`, already normalised out of the
/// subnormal range by the caller.
///
/// Returns `(y, tail)`; upstream writes `tail` through an out-parameter.
fn log_inline(ix: u64) -> (f64, f64) {
    // x = 2^k z, with z in [OFF, 2*OFF) and exact. The range is split into N
    // subintervals; the i-th contains z, and c is near its centre.
    let tmp = ix.wrapping_sub(LOG_OFF);
    let i = ((tmp >> (52 - LOG_TABLE_BITS)) % LOG_N) as usize;
    let k = (tmp as i64) >> 52; // arithmetic shift
    let iz = ix.wrapping_sub(tmp & (0xfffu64 << 52));
    let z = f64::from_bits(iz);
    let kd = k as f64;

    // `i = (...) % LOG_N` and `POW_TAB` holds `3 * LOG_N` entries, so all
    // three reads are in range -- by arithmetic the compiler cannot follow.
    // `log_inline` has no error channel (upstream's `log_inline` returns a
    // double-double), so the unreachable arm yields NaN, which propagates
    // loudly through `pow` rather than ending the program.
    let (Some(&t_invc), Some(&t_logc), Some(&t_tail)) = (
        POW_TAB.get(3 * i),
        POW_TAB.get(3 * i + 1),
        POW_TAB.get(3 * i + 2),
    ) else {
        return (f64::NAN, f64::NAN);
    };
    let invc = f64::from_bits(t_invc);
    let logc = f64::from_bits(t_logc);
    let logctail = f64::from_bits(t_tail);

    // 1/c is j/N or j/N/2 for an integer j in [N, 2N), and |z/c - 1| < 1/N, so
    // r = z/c - 1 is exactly representable.
    //
    // HAVE_FAST_FMA == 0, so the split form (`pow.c:57-62`): split z so that
    // rhi, rlo and rhi*rhi are exact and |rlo| <= |r|.
    let zhi = f64::from_bits((iz + (1u64 << 31)) & (u64::MAX << 32));
    let zlo = z - zhi;
    let rhi = zhi * invc - 1.0;
    let rlo = zlo * invc;
    let r = rhi + rlo;

    // k*Ln2 + log(c) + r.
    let t1 = kd * POW_LN2_HI + logc;
    let t2 = t1 + r;
    let lo1 = kd * POW_LN2_LO + logctail;
    let lo2 = t1 - t2 + r;

    // Evaluation is ordered assuming superscalar pipelined execution.
    let ar = POW_POLY[0] * r; // POW_POLY[0] == -0.5.
    let ar2 = r * ar;
    let ar3 = r * ar2;

    // k*Ln2 + log(c) + r + POW_POLY[0]*r*r, again in the non-FMA form.
    let arhi = POW_POLY[0] * rhi;
    let arhi2 = rhi * arhi;
    let hi = t2 + arhi2;
    let lo3 = rlo * (ar + arhi);
    let lo4 = t2 - hi + arhi2;

    // p = log1p(r) - r - POW_POLY[0]*r*r. POW_LOG_POLY_ORDER == 8.
    let p = ar3
        * (POW_POLY[1]
            + r * POW_POLY[2]
            + ar2 * (POW_POLY[3] + r * POW_POLY[4] + ar2 * (POW_POLY[5] + r * POW_POLY[6])));

    let lo = lo1 + lo2 + lo3 + lo4 + p;
    let y = hi + lo;
    (y, hi - y + lo)
}

/// `specialcase` (`pow.c:120-159`) — the **signed** large-|x| branch, applying
/// the scale in two steps so the exponent cannot overflow and so a subnormal
/// result is rounded once rather than twice.
///
/// This is *not* [`crate::fast_exp`]'s `specialcase`: see the module docs for
/// the three differences, all of which are sign handling.
fn pow_specialcase(tmp: f64, sbits: u64, ki: u64) -> core::result::Result<f64, (PetirError, f64)> {
    /// `0x1p1009` — 2^1009, the counterpart to the `1009 << 52` bias removal.
    const P1009: f64 = f64::from_bits(0x7F00000000000000);
    /// `0x1p-1022` — the smallest positive normal.
    const P_M1022: f64 = f64::MIN_POSITIVE;

    if ki & 0x8000_0000 == 0 {
        // k > 0: the exponent of scale might have overflowed by <= 460.
        let sbits = sbits.wrapping_sub(1009u64 << 52);
        let scale = f64::from_bits(sbits);
        let y = scale + scale * tmp;
        // Upstream guards `pow(0x1.fffffffffffffp+1023, 1.0)` here, but only
        // under a non-default rounding mode: the guard is `y == 0x1p15 &&
        // (1.0 + 0x1p-60) != 1.0`, and the second conjunct is false under
        // round-to-nearest. Rust has no rounding-mode control and always
        // evaluates round-to-nearest, so the branch is unreachable and is
        // deliberately not transcribed.
        let y = y * P1009;
        return if y.is_infinite() {
            Err((PetirError::Overflow, y)) // check_oflow
        } else {
            Ok(y)
        };
    }

    // k < 0: needs care in the subnormal range.
    let sbits = sbits.wrapping_add(1022u64 << 52);
    // Note: sbits is a *signed* scale here.
    let scale = f64::from_bits(sbits);
    let mut y = scale + scale * tmp;
    if y.abs() < 1.0 {
        // Round y to the right precision before scaling it into the subnormal
        // range, to avoid a double rounding that costs 0.5 + E/2 ulp.
        let one = if y < 0.0 { -1.0 } else { 1.0 };
        let lo0 = scale - y + scale * tmp;
        let hi = one + y;
        let lo = one - hi + y + lo0;
        y = (hi + lo) - one;
        // Fix the sign of zero.
        if y == 0.0 {
            y = f64::from_bits(sbits & 0x8000_0000_0000_0000);
        }
    }
    let y = P_M1022 * y;
    if y == 0.0 {
        Err((PetirError::Underflow, y)) // check_uflow
    } else {
        Ok(y)
    }
}

/// `exp_inline` (`pow.c:165-243`) — `sign * exp(x + xtail)`, where
/// `|xtail| < 2^-8/N` and `|xtail| <= |x|`, and `sign_bias` is
/// [`SIGN_BIAS`] or 0 to set the sign to -1 or 1.
///
/// See the module docs for why this is not [`crate::fast_exp::exp`]: infinity
/// and NaN are already excluded by the caller, so the large-|x| branch here
/// goes straight to overflow/underflow.
fn exp_inline(x: f64, xtail: f64, sign_bias: u32) -> core::result::Result<f64, (PetirError, f64)> {
    let mut abstop = top12(x) & 0x7ff;

    // The unsigned-wraparound trick catching both tiny and huge |x| in one
    // compare (`pow.c:172`). `0x1p-54` is `f64::from_bits(0x3c90…)`.
    const TOP12_P_M54: u32 = 0x3c9;
    if abstop.wrapping_sub(TOP12_P_M54) >= top12(512.0).wrapping_sub(TOP12_P_M54) {
        if abstop.wrapping_sub(TOP12_P_M54) >= 0x8000_0000 {
            // Tiny x: avoid spurious underflow. 0 is a common input.
            let one = 1.0 + x;
            return Ok(if sign_bias != 0 { -one } else { one });
        }
        if abstop >= top12(1024.0) {
            // Note: inf and nan are already handled by `powf`'s prologue.
            // `__math_uflow(sign)` returns ±0 and `__math_oflow(sign)` ±inf,
            // the sign taken from `sign_bias` being non-zero.
            let neg = sign_bias != 0;
            return if x.to_bits() >> 63 != 0 {
                Err((PetirError::Underflow, if neg { -0.0 } else { 0.0 }))
            } else {
                Err((
                    PetirError::Overflow,
                    if neg {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    },
                ))
            };
        }
        // Large x is special-cased below.
        abstop = 0;
    }

    // exp(x) = 2^(k/N) * exp(r), r in [-ln2/2N, ln2/2N].
    let z = INV_LN2N * x;
    // TOINT_INTRINSICS and EXP_USE_TOINT_NARROW are both 0, so the portable
    // `#else` arm (`pow.c:212-215`).
    let mut kd = z + SHIFT;
    let ki = kd.to_bits();
    kd -= SHIFT;

    let mut r = x + kd * NEG_LN2_HI_N + kd * NEG_LN2_LO_N;
    // The code assumes 2^-200 < |xtail| < 2^-8/N.
    r += xtail;

    let idx = (2 * (ki % EXP_N)) as usize;
    let top = (ki.wrapping_add(u64::from(sign_bias))) << (52 - EXP_TABLE_BITS);
    // `idx = 2 * (ki % EXP_N)` and `EXP_TAB` holds `2 * EXP_N` entries, so
    // both reads are in range for every input -- by arithmetic the compiler
    // cannot follow. `get` keeps the values, and therefore the bit-identity
    // with upstream pinned by tests/fast_pow_vs_arm_optimized_routines.rs,
    // exactly as they were.
    let (Some(&tab_lo), Some(&tab_hi)) = (EXP_TAB.get(idx), EXP_TAB.get(idx + 1)) else {
        return Err((PetirError::Range, f64::NAN));
    };
    let tail = f64::from_bits(tab_lo);
    // Valid as a scale only for -1023*N < k < 1024*N.
    let sbits = tab_hi.wrapping_add(top);

    let r2 = r * r;
    // EXP_POLY_ORDER == 5.
    let tmp =
        tail + r + r2 * (EXP_POLY[0] + r * EXP_POLY[1]) + r2 * r2 * (EXP_POLY[2] + r * EXP_POLY[3]);

    if abstop == 0 {
        return pow_specialcase(tmp, sbits, ki);
    }
    let scale = f64::from_bits(sbits);
    Ok(scale + scale * tmp)
}

/// `checkint` (`pow_common.h:23-34`) — 0 if `y` is not an integer, 1 if it is
/// an odd integer, 2 if an even one. The argument is the bit representation of
/// a non-zero finite value.
fn checkint(iy: u64) -> i32 {
    let e = (iy >> 52 & 0x7ff) as i32;
    if e < 0x3ff {
        return 0;
    }
    if e > 0x3ff + 52 {
        return 2;
    }
    let shift = 0x3ff + 52 - e;
    if iy & ((1u64 << shift) - 1) != 0 {
        return 0;
    }
    if iy & (1u64 << shift) != 0 {
        return 1;
    }
    2
}

/// `issignaling_inline` (`math_config.h:263-271`) with
/// `IEEE_754_2008_SNAN == 1`, upstream's default.
///
/// Rust has no way to *produce* a signalling NaN through safe arithmetic (an
/// sNaN that reaches a Rust operation is quieted), so in practice this only
/// fires for a bit pattern the caller constructed with `f64::from_bits`. It is
/// transcribed anyway, because dropping it would silently change `pow`'s
/// answer for exactly those inputs.
fn issignaling(x: f64) -> bool {
    let ix = x.to_bits();
    2u64.wrapping_mul(ix ^ 0x0008_0000_0000_0000) > 2u64.wrapping_mul(0x7ff8_0000_0000_0000)
}

/// `zeroinfnan` (`pow_common.h:37-41`) — true for the bit representation of
/// zero, an infinity, or a NaN.
fn zeroinfnan(i: u64) -> bool {
    i.wrapping_mul(2).wrapping_sub(1) >= f64::INFINITY.to_bits().wrapping_mul(2).wrapping_sub(1)
}

/// The transcription itself — `pow` (`math/pow.c:246`). Worst-case error
/// upstream states as 0.54 ulp.
///
/// Returns `Err((error, ieee_value))` for the special cases, carrying both
/// PETIR's error and the value upstream's C returns, so that [`powf`] and
/// [`powf_ieee`] are two views of one implementation rather than two
/// implementations.
///
/// # Verification
///
/// Checked against upstream's own compiled C over 43 435 probes crossing every
/// branch — an ordinary base/exponent spread, bases within 3e-2 of 1.0 against
/// exponents up to 1e6 (the regime `log_inline`'s extra precision exists for),
/// negative bases against every integer exponent in `[-25, 25]`, the full
/// binade range of the base, and the IEEE special-case cross product:
/// **42 176 compared, 42 176 bit-identical (100.000 %)**, 1 259 refused with
/// upstream returning a NaN, infinity or zero at each
/// (`tests/fast_pow_vs_arm_optimized_routines.rs`, 2026-09-14).
fn powf_inner(x: f64, y: f64) -> core::result::Result<f64, (PetirError, f64)> {
    let mut sign_bias = 0u32;
    let mut ix = x.to_bits();
    let iy = y.to_bits();
    let mut topx = top12(x);
    let topy = top12(y);

    if topx.wrapping_sub(0x001) >= 0x7ffu32.wrapping_sub(0x001)
        || (topy & 0x7ff).wrapping_sub(0x3be) >= 0x43eu32.wrapping_sub(0x3be)
    {
        // If |y| > 1075*ln2*2^53 ~= 0x1.749p62 then x^y is inf or 0, and if
        // |y| < 2^-54/1075 ~= 0x1.e7b6p-65 then x^y is ±1. So: (x < 0x1p-126
        // or inf or nan) or (|y| < 0x1p-65 or |y| >= 0x1p63 or nan).
        if zeroinfnan(iy) {
            if 2u64.wrapping_mul(iy) == 0 {
                // y == ±0: x^0 == 1 for every x, a quiet NaN included; a
                // signalling one is quieted through the addition instead.
                return Ok(if issignaling(x) { x + y } else { 1.0 });
            }
            if ix == 1.0f64.to_bits() {
                // 1^y == 1, with the same signalling-NaN caveat.
                return Ok(if issignaling(y) { x + y } else { 1.0 });
            }
            if 2u64.wrapping_mul(ix) > 2u64.wrapping_mul(f64::INFINITY.to_bits())
                || 2u64.wrapping_mul(iy) > 2u64.wrapping_mul(f64::INFINITY.to_bits())
            {
                return Ok(x + y); // NaN propagation
            }
            if 2u64.wrapping_mul(ix) == 2u64.wrapping_mul(1.0f64.to_bits()) {
                return Ok(1.0); // (-1)^(±inf) == 1
            }
            if (2u64.wrapping_mul(ix) < 2u64.wrapping_mul(1.0f64.to_bits())) == (iy >> 63 == 0) {
                return Ok(0.0); // |x|<1 && y==inf, or |x|>1 && y==-inf
            }
            return Ok(y * y);
        }
        if zeroinfnan(ix) {
            let mut x2 = x * x;
            if ix >> 63 != 0 && checkint(iy) == 1 {
                x2 = -x2;
                sign_bias = 1;
            }
            if 2u64.wrapping_mul(ix) == 0 && iy >> 63 != 0 {
                // 0^negative. Upstream guards this branch with `WANT_ERRNO`,
                // which is **0** in the baseline build, so its compiled code
                // falls through to `1/x2` and returns ±inf without setting
                // errno. PETIR reports an infinite result as an error instead,
                // as `fast_exp` and `fast_log` do, so the divergence is in the
                // reporting convention, not the value. `sign_bias` is set to 1
                // just above only to choose the sign of that infinity.
                return Err((
                    PetirError::ZeroDivide, // __math_divzero(sign_bias)
                    if sign_bias != 0 {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    },
                ));
            }
            return Ok(if iy >> 63 != 0 { 1.0 / x2 } else { x2 });
        }

        // Here x and y are non-zero and finite.
        if ix >> 63 != 0 {
            // Finite x < 0.
            let yint = checkint(iy);
            if yint == 0 {
                // `__math_invalid(x)` returns `(x - x) / (x - x)`, raising
                // invalid-operand. The NaN's payload and sign are
                // architecture-dependent (0xfff8… on x86-64 SSE, 0x7ff8… on
                // aarch64); this port returns the positive quiet NaN on every
                // target. A deliberate divergence in payload only — see
                // `crate::fast_log`'s corresponding comment for the reasoning.
                return Err((PetirError::Domain, f64::NAN));
            }
            if yint == 1 {
                sign_bias = SIGN_BIAS;
            }
            ix &= 0x7fff_ffff_ffff_ffff;
            topx &= 0x7ff;
        }
        if (topy & 0x7ff).wrapping_sub(0x3be) >= 0x43eu32.wrapping_sub(0x3be) {
            // sign_bias == 0 here, because y is not odd.
            if ix == 1.0f64.to_bits() {
                return Ok(1.0);
            }
            if topy & 0x7ff < 0x3be {
                // |y| < 2^-65, so x^y ~= 1 + y*log(x).
                return Ok(if ix > 1.0f64.to_bits() {
                    1.0 + y
                } else {
                    1.0 - y
                });
            }
            return if (ix > 1.0f64.to_bits()) == (topy < 0x800) {
                Err((PetirError::Overflow, f64::INFINITY)) // __math_oflow(0)
            } else {
                Err((PetirError::Underflow, 0.0)) // __math_uflow(0)
            };
        }
        if topx == 0 {
            // Normalise subnormal x so its exponent becomes negative.
            ix = (x * f64::from_bits(0x4330000000000000)).to_bits(); // 0x1p52
            ix &= 0x7fff_ffff_ffff_ffff;
            ix -= 52u64 << 52;
        }
    }

    let (hi, lo) = log_inline(ix);
    // HAVE_FAST_FMA == 0, so the split form (`pow.c:337-343`).
    let yhi = f64::from_bits(iy & (u64::MAX << 27));
    let ylo = y - yhi;
    let lhi = f64::from_bits(hi.to_bits() & (u64::MAX << 27));
    let llo = hi - lhi + lo;
    let ehi = yhi * lhi;
    let elo = ylo * lhi + y * llo; // |elo| < |ehi| * 2^-25
    exp_inline(ehi, elo, sign_bias)
}

/// Fast `x^y`, refusing the cases with no finite real answer.
///
/// This is [`powf_ieee`] with upstream's special returns turned into errors,
/// which is PETIR's convention elsewhere.
///
/// # Errors
///
/// [`PetirError::Overflow`] where upstream returns `__math_oflow` (the
/// result exceeds the double range), [`PetirError::ZeroDivide`] where it
/// returns `__math_divzero` (`0^negative`), and [`PetirError::Underflow`]
/// where it returns `__math_uflow`. The first two were a single variant
/// until 2026-09-20; they are different conditions and are now reported as
/// such.
/// [`PetirError::Domain`] for a negative base raised to a non-integer
/// exponent, where upstream returns `__math_invalid` (a NaN, with the invalid
/// flag). The IEEE special cases that have a finite or infinite answer —
/// `x^0 == 1`, `1^y == 1`, `(±0)^y`, `(±inf)^y`, NaN propagation — come back
/// as `Ok`, exactly as upstream returns them.
#[inline]
pub fn powf(x: f64, y: f64) -> Result<f64> {
    powf_inner(x, y).map_err(|(e, _)| e)
}

/// Fast `x^y`, returning exactly what upstream's C returns — `±inf`, `±0` or a
/// NaN — rather than an error.
///
/// The faithful surface, and the one to prefer; see
/// [`crate::fast_exp::exp_ieee`] for why both exist.
///
/// One case deliberately does not round-trip: for `0^negative` upstream guards
/// its `__math_divzero` branch with `WANT_ERRNO`, which is 0 in the baseline
/// build, so its compiled code falls through to `1/x2` and returns `±inf`
/// without setting `errno`. This returns the same `±inf`; only [`powf`]'s
/// error report differs, and the module docs say so.
#[inline]
pub fn powf_ieee(x: f64, y: f64) -> f64 {
    powf_inner(x, y).unwrap_or_else(|(_, v)| v)
}
