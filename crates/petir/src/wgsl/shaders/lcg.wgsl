// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (u32 pairs, f32 uniform) from `petir::rng::lcg`, which
// ports OpenMC's `src/random_lcg.cpp` (state advance `prn`, lines 32-35;
// `prn_mult` / `prn_add`, lines 11-12). OpenMC is
//   Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
//   Argonne LLC, and OpenMC contributors, MIT licence (one-way into GPL-3.0).
//
// MOVED 2026-10-02 from `outram-mc-libs`, where two copies lived inline:
// `lcg_advance` in `src/gpu/shaders/batched_flight.wgsl` and `rng_next` in
// `src/gpu/shaders/batched_event.wgsl`. Both were the same arithmetic, written
// twice. This file is now the ONE copy; both kernels are composed with it at
// pipeline creation (`outram_mc_libs::gpu::batched_{flight,event}::shader_source`)
// and call `petir_lcg_next`. The arithmetic below is byte-for-byte the old
// `rng_next`/`lcg_advance` body; only the names gained a `petir_` prefix so
// that composing this file into another shader cannot collide with that
// shader's own identifiers.
//
// THE RECURRENCE
// --------------
//   seed_{n+1} = (MULT * seed_n + INC) mod 2^64
//   MULT = 6364136223846793005 = 0x5851F42D4C957F2D
//   INC  = 1442695040888963407 = 0x14057B7EF767814F
//
// WGSL has NO u64 and NO f64, so the 64-bit multiply-add is emulated with u32
// pairs (16-bit schoolbook for exact carries). The INTEGER STATE ADVANCE IS
// BIT-EXACT against the CPU `petir::rng::lcg::future_seed(1, seed)`; that is
// the property consumers rely on for reproducibility, and it is pinned on a
// device by `outram-mc-libs/tests/gpu_lcg_advance_directly.rs` and
// `gpu_sampling_helpers_directly.rs`, and off a device by
// `petir::wgsl::mirror_lcg`'s tests.
//
// THE UNIFORM IS NOT THE CPU's `prn`, AND THAT IS STRUCTURAL
// ----------------------------------------------------------
// The uniform returned here is the TOP 24 BITS of the advanced state,
//   xi = f32(state_hi >> 8) * 2^-24      in [0, 1), exactly representable,
// with NO output permutation. The CPU `prn` applies OpenMC's PCG-RXS-M-XS
// permutation and scales the permuted word by 2^-64. Those are different
// functions of the same integer and disagree in exact arithmetic (worst gap
// 9.995e-01 over 1e6 draws from seed 1). The raw top-24 stream is a sound
// uniform in its own right: over the same 1e6 draws its mean is 4.9977e-01,
// chi-square 44.67 on 63 dof over 64 buckets (5 % critical value 82.5), and
// its covariance with the CPU stream is -6.6e-05, inside one standard error of
// zero. Measured 2026-09-19 by `outram-mc-libs`'
// `the_reference_is_sound_without_a_gpu`.
//
// This file declares no binding and reads no buffer, so it composes into any
// shader. It does NOT need `petir::wgsl::BINDING_PRELUDE`.

// LCG constants split into 32-bit halves.
const PETIR_LCG_MULT_HI: u32 = 0x5851F42Du;
const PETIR_LCG_MULT_LO: u32 = 0x4C957F2Du;
const PETIR_LCG_INC_HI:  u32 = 0x14057B7Eu;
const PETIR_LCG_INC_LO:  u32 = 0xF767814Fu;

// Full 32x32 -> 64-bit unsigned product via 16-bit schoolbook decomposition, so
// all carries are exact. Returns vec2(lo32, hi32).
fn petir_lcg_mul_u32_full(x: u32, y: u32) -> vec2<u32> {
    let x0 = x & 0xFFFFu;
    let x1 = x >> 16u;
    let y0 = y & 0xFFFFu;
    let y1 = y >> 16u;
    let t0 = x0 * y0;               // < 2^32
    let s  = x0 * y1;               // < 2^32
    let t1 = s + x1 * y0;           // wraps in u32; carry captured below
    let carry1 = select(0u, 1u, t1 < s); // bit 32 of the true (s + x1*y0)
    let t2 = x1 * y1;               // < 2^32
    let lo_lo16 = t0 & 0xFFFFu;
    let mid = (t0 >> 16u) + (t1 & 0xFFFFu); // < 2^17
    let lo = lo_lo16 | ((mid & 0xFFFFu) << 16u);
    let carry_mid = mid >> 16u;     // 0 or 1
    let hi = t2 + (t1 >> 16u) + carry_mid + (carry1 << 16u);
    return vec2<u32>(lo, hi);
}

// Low 64 bits of the 64x64 product (a_hi:a_lo) * (b_hi:b_lo). Only the low 64
// bits are needed for the mod-2^64 LCG advance.
fn petir_lcg_mul64_low(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let p = petir_lcg_mul_u32_full(a_lo, b_lo);
    // (a_lo*b_hi + a_hi*b_lo) contributes only its low 32 bits (it is scaled by
    // 2^32); u32 multiply/add already wraps to those low 32 bits.
    let cross = a_lo * b_hi + a_hi * b_lo;
    return vec2<u32>(p.x, p.y + cross); // high word wraps in u32 = mod 2^32
}

// Advance the split seed (s.x = lo, s.y = hi) by ONE step. Returns the new
// state in .xy (lo, hi) and the top-24-bit f32 uniform bitcast into .z, so a
// caller writes `let r = petir_lcg_next(seed); seed = r.xy;
// let xi = bitcast<f32>(r.z);`.
fn petir_lcg_next(s: vec2<u32>) -> vec3<u32> {
    let m = petir_lcg_mul64_low(s.x, s.y, PETIR_LCG_MULT_LO, PETIR_LCG_MULT_HI);
    let new_lo = m.x + PETIR_LCG_INC_LO;
    let carry = select(0u, 1u, new_lo < PETIR_LCG_INC_LO); // carry into the high word
    let new_hi = m.y + PETIR_LCG_INC_HI + carry;
    // Uniform from the top 24 bits of the 64-bit state (state_hi >> 8) in [0,1).
    let xi = f32(new_hi >> 8u) * (1.0 / 16777216.0);
    return vec3<u32>(new_lo, new_hi, bitcast<u32>(xi));
}
