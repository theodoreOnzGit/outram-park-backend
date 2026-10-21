// SPDX-License-Identifier: GPL-3.0-only

//! CPU mirror of `shaders/lcg.wgsl` ([`super::LCG`]), the GPU twin of
//! [`crate::rng::lcg`].
//!
//! Two layers, deliberately kept apart:
//!
//! - [`lcg_next`] is the **reference** the shader is judged against: the
//!   advance computed in native `u64` with [`crate::rng::lcg::MULT`] and
//!   [`crate::rng::lcg::INC`], plus the shader's top-24-bit `f32` uniform.
//!   `outram-mc-libs`' batched-flight and batched-event CPU mirrors call it, so
//!   there is one Rust statement of "what the shader must return".
//! - [`mul_u32_full`] and [`mul64_low`] transcribe the shader's **32-bit
//!   emulation** of the 64-bit multiply line for line, so the emulation can be
//!   checked against `u64::wrapping_mul` on any host, with no device. On a
//!   GPU the same check is `outram-mc-libs/tests/gpu_lcg_advance_directly.rs`.
//!
//! Unlike the other mirrors in this module there is no `f32` error budget:
//! the state is integer arithmetic and must be **bit-exact**, and the uniform
//! is an exact `f32` (a 24-bit integer times `2^-24`).

use crate::rng::lcg::{INC, MULT};

/// Advance the split seed `(lo, hi)` one step, as `petir_lcg_next` does.
///
/// Returns `(new_lo, new_hi, xi)` where `(new_hi, new_lo)` is
/// `MULT * seed + INC (mod 2^64)` — i.e. [`crate::rng::lcg::future_seed`]`(1,
/// seed)` — and `xi = f32(new_hi >> 8) * 2^-24`, the shader's uniform. `xi` is
/// **not** [`crate::rng::lcg::prn`]: see the shader header for why that
/// divergence is structural and why the top-24 stream is still sound.
///
/// ```
/// use petir::wgsl::mirror_lcg::lcg_next;
/// let (lo, hi, xi) = lcg_next(1, 0);
/// let seed = ((hi as u64) << 32) | lo as u64;
/// assert_eq!(seed, petir::rng::lcg::future_seed(1, 1));
/// assert!((0.0..1.0).contains(&xi));
/// ```
#[inline]
pub fn lcg_next(lo: u32, hi: u32) -> (u32, u32, f32) {
    let seed = ((hi as u64) << 32) | (lo as u64);
    let new = seed.wrapping_mul(MULT).wrapping_add(INC);
    let new_hi = (new >> 32) as u32;
    let new_lo = new as u32;
    // Top 24 bits of the 64-bit state = new_hi >> 8 = new >> 40.
    let xi = ((new >> 40) as u32 as f32) * (1.0f32 / 16_777_216.0f32);
    (new_lo, new_hi, xi)
}

/// `petir_lcg_mul_u32_full`, line for line: the full `32 x 32 -> 64` product
/// by 16-bit schoolbook decomposition. Returns `(lo, hi)`.
#[inline]
pub fn mul_u32_full(x: u32, y: u32) -> (u32, u32) {
    let x0 = x & 0xFFFF;
    let x1 = x >> 16;
    let y0 = y & 0xFFFF;
    let y1 = y >> 16;
    let t0 = x0 * y0;
    let s = x0 * y1;
    let t1 = s.wrapping_add(x1 * y0);
    let carry1 = u32::from(t1 < s);
    let t2 = x1 * y1;
    let lo_lo16 = t0 & 0xFFFF;
    let mid = (t0 >> 16) + (t1 & 0xFFFF);
    let lo = lo_lo16 | ((mid & 0xFFFF) << 16);
    let carry_mid = mid >> 16;
    let hi = t2
        .wrapping_add(t1 >> 16)
        .wrapping_add(carry_mid)
        .wrapping_add(carry1 << 16);
    (lo, hi)
}

/// `petir_lcg_mul64_low`, line for line: the low 64 bits of
/// `(a_hi:a_lo) * (b_hi:b_lo)`. Returns `(lo, hi)`.
#[inline]
pub fn mul64_low(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> (u32, u32) {
    let (p_lo, p_hi) = mul_u32_full(a_lo, b_lo);
    let cross = a_lo
        .wrapping_mul(b_hi)
        .wrapping_add(a_hi.wrapping_mul(b_lo));
    (p_lo, p_hi.wrapping_add(cross))
}

/// The shader's whole advance through the emulated multiply, line for line.
/// Must equal [`lcg_next`] bit for bit; the tests check that it does.
#[inline]
pub fn lcg_next_emulated(lo: u32, hi: u32) -> (u32, u32, f32) {
    let (m_lo, m_hi) = mul64_low(lo, hi, MULT as u32, (MULT >> 32) as u32);
    let inc_lo = INC as u32;
    let inc_hi = (INC >> 32) as u32;
    let new_lo = m_lo.wrapping_add(inc_lo);
    let carry = u32::from(new_lo < inc_lo);
    let new_hi = m_hi.wrapping_add(inc_hi).wrapping_add(carry);
    let xi = ((new_hi >> 8) as f32) * (1.0f32 / 16_777_216.0f32);
    (new_lo, new_hi, xi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::lcg::future_seed;

    /// Seeds where an emulated carry fails if it is going to: zero, one, both
    /// 32-bit boundaries, all-ones, and a deterministic scatter.
    fn seeds() -> impl Iterator<Item = u64> {
        [
            0,
            1,
            0xFFFF_FFFF,
            0x1_0000_0000,
            u64::MAX,
            0x8000_0000_0000_0000,
        ]
        .into_iter()
        .chain((0..2000u64).map(|k| future_seed(k * 7919, 0x5eed)))
    }

    /// The emulated 32-bit multiply equals `u64::wrapping_mul`, exactly.
    #[test]
    fn the_emulated_multiply_is_exact() {
        for a in seeds() {
            for b in [MULT, INC, 0, 1, u64::MAX, a.rotate_left(17)] {
                let (lo, hi) = mul64_low(a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32);
                assert_eq!(
                    ((hi as u64) << 32) | lo as u64,
                    a.wrapping_mul(b),
                    "{a:#x} * {b:#x}"
                );
            }
        }
    }

    /// The shader's emulated advance equals the `u64` reference and
    /// `future_seed(1, .)`, bit for bit, state and uniform alike — including
    /// along a 4096-step chain, where a per-step error cannot cancel.
    #[test]
    fn the_emulated_advance_is_bit_exact_against_the_cpu_lcg() {
        for s in seeds() {
            let (lo, hi) = (s as u32, (s >> 32) as u32);
            let r = lcg_next(lo, hi);
            let e = lcg_next_emulated(lo, hi);
            assert_eq!(
                (r.0, r.1, r.2.to_bits()),
                (e.0, e.1, e.2.to_bits()),
                "seed {s:#x}"
            );
            assert_eq!(((r.1 as u64) << 32) | r.0 as u64, future_seed(1, s));
            assert!((0.0..1.0).contains(&r.2));
        }
        let (mut lo, mut hi) = (1u32, 0u32);
        for _ in 0..4096 {
            let e = lcg_next_emulated(lo, hi);
            lo = e.0;
            hi = e.1;
        }
        assert_eq!(((hi as u64) << 32) | lo as u64, future_seed(4096, 1));
    }
}
