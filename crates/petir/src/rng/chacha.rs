// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from the exact versions boon-lay's Cargo.lock pinned
// on 2026-10-20 (`rand` 0.10.3 re-exports `chacha20`'s generator as
// `rand::rngs::ChaCha8Rng`):
//
//   chacha20  0.10.2  src/lib.rs           (CONSTANTS, `ChaChaCore::
//                                            new_internal`, `quarter_round`)
//                     src/backends/soft.rs (`gen_ks_blocks`, `run_rounds`)
//                     src/rng.rs           (`ChaCha8Rng`: 4-block buffer,
//                                            `from_seed`, `set_stream`,
//                                            `set_block_pos`)
//                     src/variants.rs      (`Legacy`: 64-bit counter,
//                                            64-bit stream id)
//   rand_core 0.10.1  src/block.rs         (`BlockRng::next_word`,
//                                            `next_u64_from_u32`, `fill_bytes`,
//                                            `reset_and_skip`)
//
// chacha20 — Licence: MIT OR Apache-2.0 (used here under MIT):
//
//   Copyright (c) 2019-2026 The RustCrypto Project Developers
//
// rand_core — Licence: MIT OR Apache-2.0 (used here under MIT):
//
//   Copyright (c) 2018-2026 The Rand Project Developers
//
// The MIT permission notice, which applies to both:
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

//! **ChaCha8Rng** — the ChaCha stream cipher with 8 rounds as a random number
//! generator, bit-identical to `rand` 0.10's `rand::rngs::ChaCha8Rng`.
//!
//! The state is the 16-word ChaCha matrix: 4 constants, the 256-bit seed as
//! the key, a 64-bit block counter (words 12-13) and a 64-bit stream id
//! (words 14-15; the "Legacy" DJB layout). Each refill runs the block
//! function on 4 consecutive counter values and buffers the 64 output words;
//! [`Rng::next_u32`] hands them out in order, [`Rng::next_u64`] pairs two
//! (low word first).
//!
//! # Deviations from upstream, all output-neutral
//!
//! - **DEVIATION (backend):** only the portable `soft` backend is ported.
//!   Upstream picks AVX2/SSE2/NEON at run time; those compute the same
//!   keystream four blocks at a time, so the output is the same.
//!   `tests/rng_vs_rand.rs` compares against a reference that upstream
//!   produced on x86_64 (AVX2 backend), word for word.
//! - **DEVIATION (storage):** upstream's `BlockRng` stores its read index in
//!   `results[0]` once that word has been consumed. PETIR keeps the index in
//!   its own field. The sequence of words handed out is the same.
//! - **DEVIATION (indexing):** PETIR's `tests/no_panic_gate.rs` forbids any
//!   subscript that can fail at run time, so upstream's `state[a]`-style
//!   indexing is written with slice patterns, `get`, `zip` and by-value
//!   quarter rounds. The arithmetic and its order are upstream's.
//! - Not ported (no caller): `ChaCha12Rng` / `ChaCha20Rng`, word-position
//!   get/set, state serialisation, `PartialEq`, zeroize.

use super::traits::{Rng, SeedableRng};

/// "expand 32-byte k". **Upstream:** `chacha20` `lib.rs:47`.
const CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

/// Words in the ChaCha state and in one output block. **Upstream:**
/// `lib.rs:50` (`STATE_WORDS`), `rng.rs:28` (`BLOCK_WORDS`).
const STATE_WORDS: usize = 16;

/// Blocks generated per refill. **Upstream:** `rng.rs:31` (`BUF_BLOCKS`).
const BUF_BLOCKS: usize = 4;

/// Words buffered per refill. **Upstream:** `rng.rs:33` (`BUFFER_SIZE`).
const BUFFER_SIZE: usize = STATE_WORDS * BUF_BLOCKS;

/// Double rounds for ChaCha8. **Upstream:** `lib.rs:62-64`, `R8::COUNT = 4`.
const DOUBLE_ROUNDS: usize = 4;

/// The ChaCha quarter round on four words, returned in the same order.
///
/// **Upstream:** `chacha20` `lib.rs:321-343`, `quarter_round(a, b, c, d,
/// state)`, which updates `state[a..d]` in place. The operations and their
/// order are upstream's; only the operands are passed by value (see the
/// indexing deviation in the module docs).
#[inline(always)]
fn quarter_round(mut a: u32, mut b: u32, mut c: u32, mut d: u32) -> (u32, u32, u32, u32) {
    a = a.wrapping_add(b);
    d ^= a;
    d = d.rotate_left(16);

    c = c.wrapping_add(d);
    b ^= c;
    b = b.rotate_left(12);

    a = a.wrapping_add(b);
    d ^= a;
    d = d.rotate_left(8);

    c = c.wrapping_add(d);
    b ^= c;
    b = b.rotate_left(7);
    (a, b, c, d)
}

/// One ChaCha8 block: 4 double rounds, then the input state added back.
///
/// **Upstream:** `chacha20` `backends/soft.rs:66-87`, `run_rounds::<R8>`,
/// with the same column and diagonal word groups in the same order.
#[inline(always)]
fn run_rounds(state: &[u32; STATE_WORDS]) -> [u32; STATE_WORDS] {
    let [
        mut x0,
        mut x1,
        mut x2,
        mut x3,
        mut x4,
        mut x5,
        mut x6,
        mut x7,
        mut x8,
        mut x9,
        mut x10,
        mut x11,
        mut x12,
        mut x13,
        mut x14,
        mut x15,
    ] = *state;

    for _ in 0..DOUBLE_ROUNDS {
        // column rounds
        (x0, x4, x8, x12) = quarter_round(x0, x4, x8, x12);
        (x1, x5, x9, x13) = quarter_round(x1, x5, x9, x13);
        (x2, x6, x10, x14) = quarter_round(x2, x6, x10, x14);
        (x3, x7, x11, x15) = quarter_round(x3, x7, x11, x15);

        // diagonal rounds
        (x0, x5, x10, x15) = quarter_round(x0, x5, x10, x15);
        (x1, x6, x11, x12) = quarter_round(x1, x6, x11, x12);
        (x2, x7, x8, x13) = quarter_round(x2, x7, x8, x13);
        (x3, x4, x9, x14) = quarter_round(x3, x4, x9, x14);
    }

    let mut res = [
        x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15,
    ];
    for (s1, s0) in res.iter_mut().zip(state.iter()) {
        *s1 = s1.wrapping_add(*s0);
    }
    res
}

/// A ChaCha8 random number generator, bit-identical to `rand` 0.10.3's
/// `rand::rngs::ChaCha8Rng` (that is, `chacha20` 0.10.2's).
///
/// Seed it with [`SeedableRng::seed_from_u64`], [`SeedableRng::from_seed`]
/// or [`SeedableRng::from_rng`]; draw with [`Rng`] or
/// [`RngExt::random`](super::RngExt::random). [`ChaCha8Rng::set_stream`]
/// selects one of 2^64 independent streams per seed, which is how RustBCA
/// gives each particle its own.
#[derive(Clone, Debug)]
pub struct ChaCha8Rng {
    /// The ChaCha state (constants, key, block counter, stream id).
    state: [u32; STATE_WORDS],
    /// The 4-block output buffer.
    results: [u32; BUFFER_SIZE],
    /// Index of the next unread word of `results`; `BUFFER_SIZE` means empty.
    index: usize,
}

impl ChaCha8Rng {
    /// Fill the buffer with the next 4 blocks, advancing the counter.
    ///
    /// **Upstream:** `chacha20` `backends/soft.rs:51-63`, `gen_ks_blocks`
    /// (the 64-bit counter in words 12-13 wraps).
    fn generate(&mut self) {
        for block in self.results.chunks_exact_mut(STATE_WORDS) {
            let res = run_rounds(&self.state);
            let [.., c12, c13, _, _] = &mut self.state;
            let mut ctr = (u64::from(*c13) << 32) | u64::from(*c12);
            ctr = ctr.wrapping_add(1);
            *c12 = ctr as u32;
            *c13 = (ctr >> 32) as u32;

            for (dst, src) in block.iter_mut().zip(res) {
                *dst = src;
            }
        }
    }

    /// Set the stream id and rewind to the start of that stream.
    ///
    /// **Upstream:** `chacha20` `rng.rs:263-268` (`set_stream`), which calls
    /// `set_block_pos(0)` (`rng.rs:242-246`): `reset_and_skip(0)` empties
    /// the buffer (`rand_core` `block.rs:195-203`) and the counter is zeroed
    /// (`Legacy::set_block_pos`, `variants.rs`).
    pub fn set_stream(&mut self, stream: u64) {
        let [.., c12, c13, s14, s15] = &mut self.state;
        *s14 = stream as u32;
        *s15 = (stream >> 32) as u32;
        self.index = BUFFER_SIZE;
        *c12 = 0;
        *c13 = 0;
    }

    /// The stream id. **Upstream:** `chacha20` `rng.rs:271-278`.
    pub fn get_stream(&self) -> u64 {
        let [.., s14, s15] = self.state;
        (u64::from(s15) << 32) | u64::from(s14)
    }

    /// Refill the buffer and return its first two words; the caller sets
    /// `index`.
    #[inline(always)]
    fn refill_and_take_first_two(&mut self) -> (u32, u32) {
        self.generate();
        let [w0, w1, ..] = self.results;
        (w0, w1)
    }
}

impl SeedableRng for ChaCha8Rng {
    type Seed = [u8; 32];

    /// Key = `seed` (little-endian words), counter 0, stream 0, buffer
    /// empty.
    ///
    /// **Upstream:** `chacha20` `rng.rs:142-147` →
    /// `ChaChaCore::new_internal(&seed, &[0u8; 8])` (`lib.rs:139-180`), and
    /// `BlockRng::new` (`rand_core` `block.rs:142-147`), which marks the
    /// buffer empty.
    fn from_seed(seed: [u8; 32]) -> Self {
        let mut state = [0u32; STATE_WORDS];
        let (key, _) = seed.as_chunks::<4>();
        let words = CONSTANTS
            .into_iter()
            .chain(key.iter().map(|w| u32::from_le_bytes(*w)));
        for (dst, w) in state.iter_mut().zip(words) {
            *dst = w;
        }
        // Words 12-13 (counter) and 14-15 (the all-zero 8-byte iv) stay 0.
        ChaCha8Rng {
            state,
            results: [0u32; BUFFER_SIZE],
            index: BUFFER_SIZE,
        }
    }
}

impl Rng for ChaCha8Rng {
    /// **Upstream:** `rand_core` `block.rs:232-243`, `BlockRng::next_word`:
    /// an exhausted buffer (`index >= N`) is refilled and read from 0.
    #[inline]
    fn next_u32(&mut self) -> u32 {
        if let Some(&value) = self.results.get(self.index) {
            self.index += 1;
            value
        } else {
            let (w0, _) = self.refill_and_take_first_two();
            self.index = 1;
            w0
        }
    }

    /// Two consecutive words, the first as the low half.
    ///
    /// **Upstream:** `rand_core` `block.rs:248-272`,
    /// `BlockRng::next_u64_from_u32`, including its straddle case: with one
    /// word left, that word is the low half and the first word of the next
    /// refill the high half; with none left, the first two of a refill.
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let index = self.index;
        let (lo, hi);
        match (
            self.results.get(index).copied(),
            self.results.get(index + 1).copied(),
        ) {
            (Some(l), Some(h)) => {
                lo = l;
                hi = h;
                self.index = index + 2;
            }
            (Some(l), None) => {
                lo = l;
                (hi, _) = self.refill_and_take_first_two();
                self.index = 1;
            }
            (None, _) => {
                (lo, hi) = self.refill_and_take_first_two();
                self.index = 2;
            }
        }
        (u64::from(hi) << 32) | u64::from(lo)
    }

    /// Bytes from the word stream, little-endian; a final partial word
    /// consumes the whole word.
    ///
    /// **Upstream:** `rand_core` `block.rs:275-308`, `BlockRng::fill_bytes`.
    /// **DEVIATION (structure, output-neutral):** upstream copies runs of
    /// whole words out of the buffer between refills and then a partial
    /// word; PETIR takes one word per 4-byte chunk through
    /// [`Rng::next_u32`]. Both hand out the same words in the same order and
    /// leave the same read position (a partial last chunk consumes its whole
    /// word in both), which the `bytes` records of `tests/rng_vs_rand.rs`
    /// pin against upstream.
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(4) {
            let word = self.next_u32().to_le_bytes();
            for (d, s) in chunk.iter_mut().zip(word) {
                *d = s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ChaCha block function against RFC 7539 section 2.3.2's test vector
    /// is for 20 rounds and the IETF layout, so it cannot pin this 8-round
    /// generator. What it can pin is the quarter round: RFC 7539 section
    /// 2.1.1's vector (`a, b, c, d` = `0x11111111, 0x01020304, 0x9b8d6f43,
    /// 0x01234567`).
    #[test]
    fn quarter_round_matches_rfc7539_2_1_1() {
        assert_eq!(
            quarter_round(0x1111_1111, 0x0102_0304, 0x9b8d_6f43, 0x0123_4567),
            (0xea2a_92f4, 0xcb1c_f8ce, 0x4581_472e, 0x5881_c4bb)
        );
    }

    /// `set_stream` rewinds: the same stream twice gives the same words, a
    /// different stream different ones, and `get_stream` reads it back.
    #[test]
    fn set_stream_rewinds_and_selects() {
        let mut a = ChaCha8Rng::seed_from_u64(3);
        let first: [u32; 3] = [a.next_u32(), a.next_u32(), a.next_u32()];
        a.set_stream(0);
        assert_eq!([a.next_u32(), a.next_u32(), a.next_u32()], first);
        a.set_stream(9);
        assert_eq!(a.get_stream(), 9);
        assert_ne!(a.next_u32(), first[0]);
    }

    /// `next_u64` across the buffer edge (one word left) takes that word as
    /// the low half, as upstream's straddle branch does; and `fill_bytes`
    /// reads the same words little-endian.
    #[test]
    fn next_u64_straddle_and_fill_bytes_agree_with_words() {
        let mut words = ChaCha8Rng::seed_from_u64(11);
        let w: [u32; 130] = core::array::from_fn(|_| words.next_u32());

        let mut r = ChaCha8Rng::seed_from_u64(11);
        for _ in 0..63 {
            r.next_u32();
        }
        assert_eq!(r.next_u64(), (u64::from(w[64]) << 32) | u64::from(w[63]));

        let mut b = ChaCha8Rng::seed_from_u64(11);
        let mut bytes = [0u8; 7];
        b.fill_bytes(&mut bytes);
        assert_eq!(&bytes[..4], &w[0].to_le_bytes());
        assert_eq!(&bytes[4..], &w[1].to_le_bytes()[..3]);
        // The partial word was consumed whole.
        assert_eq!(b.next_u32(), w[2]);
    }
}
