// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from two crates of the Rand project, read from the
// exact versions boon-lay's Cargo.lock pinned on 2026-10-20:
//
//   rand_core 0.10.1  src/seedable_rng.rs  (`SeedableRng::seed_from_u64`,
//                     `from_rng`, the private `pcg32` seed expander)
//                     src/lib.rs           (the shape of the `Rng` trait)
//   rand      0.10.3  src/rng.rs           (`RngExt::random`)
//                     src/distr/float.rs   (`StandardUniform` for f64)
//                     src/distr/integer.rs (`StandardUniform` for u32, u64)
//
// Licence: MIT OR Apache-2.0 (used here under MIT). The MIT notice, as the
// licence requires it to travel with substantial portions. Copyright lines
// as each upstream states them (LICENSE-MIT and the file headers):
//
//   rand_core: Copyright (c) 2018-2026 The Rand Project Developers
//   rand:      Copyright 2018 Developers of the Rand project
//              Copyright (c) 2014 The Rust Project Developers
//              Copyright 2013-2017 The Rust Project Developers (src/rng.rs)
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
// The flow is ONE-WAY (MIT into this AGPL-3.0-only crate). See NOTICE.
// Maintainer direction: gh:#995, 2026-10-20.
// ---------------------------------------------------------------------------

//! The generator traits: [`Rng`] (raw words), [`SeedableRng`] (seeding,
//! including `rand_core`'s PCG32 seed expansion) and [`RngExt`] (`random`).
//!
//! Only what `boon-lay`'s RustBCA copy calls is ported. The names follow
//! `rand_core` 0.10 / `rand` 0.10, so the copy's call sites read as they do
//! upstream (`ChaCha8Rng::seed_from_u64(s)`, `rng.random::<f64>()`).

/// A source of random 32- and 64-bit words and bytes.
///
/// **Upstream:** `rand_core` 0.10.1's infallible `Rng` trait (`src/lib.rs`),
/// reduced to its three required operations. `rand_core` derives them from
/// `TryRng` with `Infallible`; PETIR's generators cannot fail, so the
/// fallible layer is not ported.
pub trait Rng {
    /// The next 32-bit word of the stream.
    fn next_u32(&mut self) -> u32;
    /// The next 64-bit word of the stream.
    fn next_u64(&mut self) -> u64;
    /// Fill `dest` with the next bytes of the stream.
    fn fill_bytes(&mut self, dest: &mut [u8]);
}

/// A generator that can be built from a seed.
///
/// **Upstream:** `rand_core` 0.10.1 `src/seedable_rng.rs`. The provided
/// methods are ported line for line, so a PETIR generator and its `rand`
/// counterpart reach the same state from the same `u64`.
pub trait SeedableRng: Sized {
    /// The seed type, a byte array (`[u8; 32]` for ChaCha).
    type Seed: Clone + Default + AsRef<[u8]> + AsMut<[u8]>;

    /// Build the generator from a full seed.
    fn from_seed(seed: Self::Seed) -> Self;

    /// Build the generator from a `u64`, expanding it to a full seed with
    /// PCG32.
    ///
    /// **Upstream:** `seedable_rng.rs:109-121`. Each 4-byte chunk of the
    /// seed takes one PCG32 output (little-endian); a remainder shorter than
    /// 4 bytes takes the leading bytes of one more. The expansion is a
    /// portability contract upstream, so it is reproduced exactly.
    fn seed_from_u64(mut state: u64) -> Self {
        let mut seed = Self::Seed::default();
        let mut iter = seed.as_mut().chunks_exact_mut(4);
        for chunk in &mut iter {
            chunk.copy_from_slice(&pcg32(&mut state));
        }
        let rem = iter.into_remainder();
        if !rem.is_empty() {
            // Upstream: `rem.copy_from_slice(&pcg32(&mut state)[..rem.len()])`;
            // the zip is the same copy without a subscript (no_panic_gate).
            for (dst, src) in rem.iter_mut().zip(pcg32(&mut state)) {
                *dst = src;
            }
        }

        Self::from_seed(seed)
    }

    /// Build the generator from the next `Seed`-many bytes of another
    /// generator.
    ///
    /// **Upstream:** `seedable_rng.rs:147-151` (`rng.fill_bytes(seed)`, then
    /// `from_seed`).
    fn from_rng<R: Rng + ?Sized>(rng: &mut R) -> Self {
        let mut seed = Self::Seed::default();
        rng.fill_bytes(seed.as_mut());
        Self::from_seed(seed)
    }
}

/// One PCG32 output, as 4 little-endian bytes, advancing `state` first.
///
/// **Upstream:** `seedable_rng.rs:192-206`, private `fn pcg32`. Multiplier
/// and increment as upstream; the state is advanced *before* the output
/// (upstream: "to get away from the input value, in case it has low Hamming
/// Weight"), then the PCG XSH-RR output function.
fn pcg32(state: &mut u64) -> [u8; 4] {
    const MUL: u64 = 0x5851_F42D_4C95_7F2D;
    const INC: u64 = 0xA176_54E4_6FBE_17F3;

    *state = state.wrapping_mul(MUL).wrapping_add(INC);
    let state = *state;

    let xorshifted = (((state >> 18) ^ state) >> 27) as u32;
    let rot = (state >> 59) as u32;
    let x = xorshifted.rotate_right(rot);
    x.to_le_bytes()
}

/// A type with a "standard" uniform sample: `[0, 1)` for floats, every bit
/// pattern for integers.
///
/// **Upstream:** `rand` 0.10.3's `impl Distribution<T> for StandardUniform`,
/// for the three types ported (`src/distr/float.rs`, `src/distr/integer.rs`).
#[diagnostic::on_unimplemented(
    message = "`random::<{Self}>()` is not available: petir::rng ports only `f64`, `u32` and `u64`",
    label = "no standard sample for `{Self}`",
    note = "port the type's `StandardUniform` impl from rand 0.10.3 if a caller needs it (rule 2: from the source)"
)]
pub trait StandardSample: Sized {
    /// Draw one standard sample from `rng`.
    fn sample_standard<R: Rng + ?Sized>(rng: &mut R) -> Self;
}

impl StandardSample for u32 {
    /// **Upstream:** `integer.rs:42-47`, `rng.next_u32()`.
    #[inline]
    fn sample_standard<R: Rng + ?Sized>(rng: &mut R) -> u32 {
        rng.next_u32()
    }
}

impl StandardSample for u64 {
    /// **Upstream:** `integer.rs:49-54`, `rng.next_u64()`.
    #[inline]
    fn sample_standard<R: Rng + ?Sized>(rng: &mut R) -> u64 {
        rng.next_u64()
    }
}

impl StandardSample for f64 {
    /// A uniform `f64` in `[0, 1)` from the top 53 bits of one `next_u64`.
    ///
    /// **Upstream:** `float.rs`, the `float_impls!` arm `impl
    /// Distribution<$ty> for StandardUniform` instantiated at line 157 with
    /// `(f64, u64, 52 fraction bits)`: "Multiply-based method; 53 random
    /// bits; [0, 1) interval", `value >> (64 - 53)` times `2^-53`. Exact in
    /// `f64`, so the result has no rounding to disagree about.
    #[inline]
    fn sample_standard<R: Rng + ?Sized>(rng: &mut R) -> f64 {
        let float_size = 64u64;
        let precision = 52 + 1;
        let scale = 1.0 / ((1u64 << precision) as f64);

        let value: u64 = rng.next_u64();
        let value = value >> (float_size - precision);
        scale * (value as f64)
    }
}

/// `rand`'s convenience methods, for the one that is ported: [`random`].
///
/// **Upstream:** `rand` 0.10.3 `src/rng.rs`, trait `RngExt`, blanket
/// implemented for every [`Rng`] as upstream does.
///
/// [`random`]: RngExt::random
pub trait RngExt: Rng {
    /// One standard sample of type `T` (see [`StandardSample`]).
    ///
    /// **Upstream:** `rng.rs:93-98`, `StandardUniform.sample(self)`.
    #[inline]
    fn random<T: StandardSample>(&mut self) -> T {
        T::sample_standard(self)
    }
}

impl<R: Rng + ?Sized> RngExt for R {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A counter "generator": every word is the next integer.
    struct Counter(u64);
    impl Rng for Counter {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for b in dest {
                *b = self.next_u64() as u8;
            }
        }
    }

    /// A seed type of 6 bytes exercises the remainder branch of
    /// `seed_from_u64` (4 + 2).
    #[derive(Debug, PartialEq)]
    struct Six([u8; 6]);
    impl SeedableRng for Six {
        type Seed = [u8; 6];
        fn from_seed(seed: [u8; 6]) -> Self {
            Six(seed)
        }
    }

    /// `seed_from_u64` takes one PCG32 output per 4 bytes and the leading
    /// bytes of one more for a remainder; `from_rng` fills the seed from the
    /// source generator.
    #[test]
    fn seed_expansion_chunks_and_remainder() {
        let mut state = 7u64;
        let a = pcg32(&mut state);
        let b = pcg32(&mut state);
        let six = Six::seed_from_u64(7);
        assert_eq!(&six.0[..4], &a);
        assert_eq!(&six.0[4..], &b[..2]);
        let mut c = Counter(0);
        assert_eq!(Six::from_rng(&mut c).0, [1, 2, 3, 4, 5, 6]);
    }

    /// Upstream's own edge cases for the f64 standard sample (`float.rs`
    /// `test_f64`): all-zero bits give 0, `1 << 11` gives `EPSILON / 2`,
    /// all-one bits give `1 - EPSILON / 2`.
    #[test]
    fn standard_f64_edge_cases_match_upstream() {
        struct Const(u64);
        impl Rng for Const {
            fn next_u32(&mut self) -> u32 {
                self.0 as u32
            }
            fn next_u64(&mut self) -> u64 {
                self.0
            }
            fn fill_bytes(&mut self, _: &mut [u8]) {}
        }
        assert_eq!(Const(0).random::<f64>(), 0.0);
        assert_eq!(Const(1 << 11).random::<f64>(), f64::EPSILON / 2.0);
        assert_eq!(Const(!0).random::<f64>(), 1.0 - f64::EPSILON / 2.0);
        assert_eq!(Const(!0).random::<u32>(), u32::MAX);
        assert_eq!(Const(5).random::<u64>(), 5);
    }
}
