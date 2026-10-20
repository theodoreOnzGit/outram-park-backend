// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, read from the published
// crates.io source (faer-0.24.4 in the cargo registry) on 2026-10-20.
//
//     https://codeberg.org/sarah-quinones/faer
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors
//     Licensed MIT (faer's Cargo.toml `license = "MIT"`; the published crate
//     ships no LICENSE file, so the standard MIT text is reproduced here).
//
// Its notice, retained as that licence requires of a redistribution in source
// form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// MIT into AGPL-3.0-only is a ONE-WAY flow: this file may not be contributed
// back to `faer` or `pulp` under their original licences without their
// authors' agreement. Maintainer direction for this port: gh #996,
// 2026-10-20 ("Whatever faer does, port into petir"). See the crate NOTICE
// and CLAUDE.md.
//
// Ported items:
//   faer 0.24.4  src/utils/simd.rs   SimdCtx::{new, new_align, indices,
//                                     batch_indices}, SimdHead/SimdBody/
//                                     SimdTail reads and writes
//   faer 0.24.4  src/lib.rs:1284     simd_iter! (which accumulator each
//                                     head, body and tail chunk feeds)
//   faer 0.24.4  src/lib.rs:1036     simd_align
//   pulp 0.22.3  src/x86/v3.rs       f64s = 4 lanes (AVX2); mul_add_f64s =
//                                     _mm256_fmadd_pd (line 892);
//                                     reduce_sum_f64s (line 1111) via
//                                     reduce_sum_f64x2 (src/x86/v1.rs:581)
//   pulp 0.22.3  copyright (c) sarah quiñones, MIT (pulp's Cargo.toml
//                `license = "MIT"`), notice text as above.

//! A **scalar model of the one SIMD instruction set faer used** when the
//! reference was recorded: `pulp`'s `V3` (x86-64 AVX2 + FMA), where an `f64`
//! vector has four lanes.
//!
//! faer's kernels are written against a vector abstraction, and which lanes a
//! value lands in decides the order of the final horizontal sum. To reproduce
//! faer's rounding, this module keeps the lanes explicitly as `[f64; 4]` and
//! reproduces:
//!
//! - **the chunking** of a contiguous column into a masked head, full-width
//!   body vectors and a masked tail (`SimdCtx::new` / `new_align`), including
//!   which lane each element sits in;
//! - **which accumulator** each chunk feeds (`simd_iter!` over
//!   `batch_indices`): the head goes to the LAST accumulator, full batches go
//!   round-robin, leftover body vectors go to accumulators `0, 1, ...`, and
//!   the tail to the next one;
//! - **masked loads** return `+0.0` in the masked-off lanes, and masked
//!   stores leave the masked-off memory alone;
//! - **`mul_add`** is one fused multiply-add per lane (`vfmadd`), done here
//!   with [`Real::mul_add`] (`libm::fma`, correctly rounded, so identical);
//! - **`reduce_sum`** is `(l0 + l2) + (l1 + l3)`: `pulp` adds the high
//!   128-bit half onto the low half, then the two remaining lanes.
//!
//! **There is no `core::arch` and no runtime CPU detection here**, by
//! design (gh #985 needs cross-platform bit identity): this code gives the
//! same bits on every target. It reproduces faer **as faer runs on an
//! AVX2+FMA x86-64 machine**. On a machine where faer itself selects a
//! different instruction set (AVX-512 under faer's `nightly` feature,
//! SSE-only x86, aarch64 NEON with two lanes, or scalar), faer's own answer
//! changes in the last bits and this module does not follow it.

use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;
use crate::{PetirError, Result};

/// Lanes in one `f64` vector under `pulp`'s `V3` (`__m256d`).
pub(crate) const LANES: usize = 4;

/// One `f64` vector, lane 0 first.
pub(crate) type Lanes = [f64; LANES];

/// The all-zero vector (`simd.zero()`).
pub(crate) const ZERO: Lanes = [0.0; LANES];

/// One chunk of a contiguous column: lanes `lo..hi` hold elements
/// `start + lo .. start + hi` of the column (`start` may be negative for an
/// aligned head, whose first lanes lie before the column and are masked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chunk {
    start: isize,
    lo: usize,
    hi: usize,
}

/// faer's `SimdCtx` for one column length: the optional masked head, the
/// full-width body chunks, and the optional masked tail, in iteration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SimdCtx {
    head: Option<Chunk>,
    body: Vec<Chunk>,
    tail: Option<Chunk>,
}

/// `simd_align(i)` (`faer/src/lib.rs:1036`): `i.wrapping_neg()`.
pub(crate) fn simd_align(i: usize) -> usize {
    i.wrapping_neg()
}

impl SimdCtx {
    /// `SimdCtx::new(simd, len)` (`faer/src/utils/simd.rs:158`): no head,
    /// `len / 4` body vectors, and a tail holding `len % 4` lanes if that is
    /// not zero.
    pub(crate) fn new(len: usize) -> Self {
        let body_end = len / LANES;
        let tail_end = len.div_ceil(LANES);
        let body = (0..body_end)
            .map(|i| Chunk {
                start: (i * LANES) as isize,
                lo: 0,
                hi: LANES,
            })
            .collect();
        let tail = if body_end == tail_end {
            None
        } else {
            Some(Chunk {
                start: (body_end * LANES) as isize,
                lo: 0,
                hi: len % LANES,
            })
        };
        Self {
            head: None,
            body,
            tail,
        }
    }

    /// `SimdCtx::new_align(simd, len, align_offset)`
    /// (`faer/src/utils/simd.rs:201`). With `align_offset % 4 == 0` this is
    /// [`SimdCtx::new`]; otherwise the column starts at lane
    /// `offset = 4 - align_offset % 4` of a masked head.
    pub(crate) fn new_align(len: usize, align_offset: usize) -> Self {
        let align_offset = align_offset % LANES;
        if align_offset == 0 {
            return Self::new(len);
        }
        let offset = LANES - align_offset;
        let full_len = offset + len;
        let neg = -(offset as isize);
        if align_offset <= len {
            // head_end = 1, body_end = full_len / 4, tail_end = ceil.
            let body_end = full_len / LANES;
            let tail_end = full_len.div_ceil(LANES);
            let body = (1..body_end)
                .map(|i| Chunk {
                    start: neg + (i * LANES) as isize,
                    lo: 0,
                    hi: LANES,
                })
                .collect();
            let tail = if body_end == tail_end {
                None
            } else {
                Some(Chunk {
                    start: neg + (body_end * LANES) as isize,
                    lo: 0,
                    hi: full_len % LANES,
                })
            };
            Self {
                head: Some(Chunk {
                    start: neg,
                    lo: offset,
                    hi: LANES,
                }),
                body,
                tail,
            }
        } else {
            // The whole column fits inside the head: lanes offset..full_len.
            Self {
                head: Some(Chunk {
                    start: neg,
                    lo: offset,
                    hi: full_len % LANES,
                }),
                body: Vec::new(),
                tail: None,
            }
        }
    }

    /// `simd_iter!(for i in [simd.indices()] { .. })`: head, then each body
    /// vector, then the tail.
    pub(crate) fn indices(&self) -> Vec<Chunk> {
        let mut out = Vec::with_capacity(self.body.len() + 2);
        out.extend(self.head);
        out.extend(self.body.iter().copied());
        out.extend(self.tail);
        out
    }

    /// `simd_iter!(for (IDX, i) in [simd.batch_indices::<B>(); B] { .. })`
    /// (`faer/src/lib.rs:1284`): every chunk paired with the accumulator it
    /// feeds, in execution order.
    ///
    /// - the head feeds accumulator `B - 1`;
    /// - the first `len / B * B` body vectors go in batches, chunk `k` of a
    ///   batch feeding accumulator `k`;
    /// - the leftover body vectors feed `0, 1, ...`;
    /// - the tail feeds the next accumulator after the leftovers.
    pub(crate) fn batch_indices(&self, batch: usize) -> Vec<(usize, Chunk)> {
        let mut out = Vec::with_capacity(self.body.len() + 2);
        if let Some(h) = self.head {
            out.push((batch.saturating_sub(1), h));
        }
        let full = self.body.len() / batch * batch;
        let (batched, rest) = self.body.split_at(full.min(self.body.len()));
        for group in batched.chunks(batch) {
            for (k, c) in group.iter().enumerate() {
                out.push((k, *c));
            }
        }
        let mut next = 0;
        for c in rest {
            out.push((next, *c));
            next += 1;
        }
        if let Some(t) = self.tail {
            out.push((next, t));
        }
        out
    }
}

/// The element of `col` held in lane `lane` of `chunk`, or `None` if that
/// lane is masked off.
fn element(chunk: Chunk, lane: usize) -> Option<usize> {
    if lane < chunk.lo || lane >= chunk.hi {
        return None;
    }
    usize::try_from(chunk.start + lane as isize).ok()
}

/// `simd.read(col, chunk)`: a (masked) load; masked-off lanes read `+0.0`.
pub(crate) fn read(col: &[f64], chunk: Chunk) -> Result<Lanes> {
    let mut v = ZERO;
    for (lane, slot) in v.iter_mut().enumerate() {
        if let Some(i) = element(chunk, lane) {
            *slot = *col.get(i).ok_or(PetirError::Invalid)?;
        }
    }
    Ok(v)
}

/// `simd.write(col, chunk, v)`: a (masked) store; masked-off memory is left
/// alone.
pub(crate) fn write(col: &mut [f64], chunk: Chunk, v: Lanes) -> Result<()> {
    for (lane, x) in v.iter().enumerate() {
        if let Some(i) = element(chunk, lane) {
            *col.get_mut(i).ok_or(PetirError::Invalid)? = *x;
        }
    }
    Ok(())
}

/// `splat_f64s`.
pub(crate) fn splat(x: f64) -> Lanes {
    [x; LANES]
}

/// `add_f64s`, lane by lane.
pub(crate) fn add(a: Lanes, b: Lanes) -> Lanes {
    let mut r = ZERO;
    for ((r, a), b) in r.iter_mut().zip(a).zip(b) {
        *r = a + b;
    }
    r
}

/// `mul_f64s`, lane by lane.
pub(crate) fn mul(a: Lanes, b: Lanes) -> Lanes {
    let mut r = ZERO;
    for ((r, a), b) in r.iter_mut().zip(a).zip(b) {
        *r = a * b;
    }
    r
}

/// `mul_add_f64s` = `_mm256_fmadd_pd`: `a * b + c` with one rounding per
/// lane.
pub(crate) fn mul_add(a: Lanes, b: Lanes, c: Lanes) -> Lanes {
    let mut r = ZERO;
    for (((r, a), b), c) in r.iter_mut().zip(a).zip(b).zip(c) {
        *r = Real::mul_add(a, b, c);
    }
    r
}

/// `reduce_sum_f64s` (`pulp/src/x86/v3.rs:1111`): `_mm_add_pd` of the low
/// and high 128-bit halves, then `_mm_add_sd` of the two lanes left, i.e.
/// `(l0 + l2) + (l1 + l3)`.
pub(crate) fn reduce_sum(v: Lanes) -> f64 {
    let [l0, l1, l2, l3] = v;
    (l0 + l2) + (l1 + l3)
}

/// The `[V; 4]` accumulator fold every faer dot-product kernel ends with:
/// `(acc0 + acc1) + (acc2 + acc3)` lane by lane, then [`reduce_sum`].
pub(crate) fn fold4(acc: [Lanes; 4]) -> f64 {
    let [a0, a1, a2, a3] = acc;
    let acc0 = add(a0, a1);
    let acc2 = add(a2, a3);
    reduce_sum(add(acc0, acc2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chunk layout of `new` and `new_align` against faer's own index
    /// arithmetic worked by hand.
    #[test]
    fn chunking_matches_faer_simd_ctx() {
        // len 6, unaligned: one body vector (0..4) and a 2-lane tail at 4.
        let c = SimdCtx::new(6);
        assert_eq!(c.head, None);
        assert_eq!(
            c.body,
            [Chunk {
                start: 0,
                lo: 0,
                hi: 4
            }]
        );
        assert_eq!(
            c.tail,
            Some(Chunk {
                start: 4,
                lo: 0,
                hi: 2
            })
        );
        // len 1 with simd_align(2): align_offset 2, offset 2, full_len 3 > len,
        // so the element sits alone in lane 2 of a head.
        let a = SimdCtx::new_align(1, simd_align(2));
        assert_eq!(
            a.head,
            Some(Chunk {
                start: -2,
                lo: 2,
                hi: 3
            })
        );
        assert!(a.body.is_empty() && a.tail.is_none());
        // len 7 with align_offset 3: offset 1, head lanes 1..4 (3 elements),
        // full_len 8 -> one body vector at 3, no tail.
        let b = SimdCtx::new_align(7, 3);
        assert_eq!(b.indices().len(), 2);
        assert_eq!(
            b.body,
            [Chunk {
                start: 3,
                lo: 0,
                hi: 4
            }]
        );
    }

    /// `simd_iter!` accumulator assignment: head -> B-1, leftovers 0.., tail
    /// after them.
    #[test]
    fn batch_assignment_follows_simd_iter() {
        // len 22 = 5 body + tail(2); batch 4: one batch of 4, one leftover
        // (acc 0), tail to acc 1.
        let c = SimdCtx::new(22);
        let idx: Vec<usize> = c.batch_indices(4).iter().map(|p| p.0).collect();
        assert_eq!(idx, [0, 1, 2, 3, 0, 1]);
        let h = SimdCtx::new_align(1, simd_align(2));
        assert_eq!(h.batch_indices(4).first().map(|p| p.0), Some(3));
    }

    #[test]
    fn reduce_sum_pairs_lanes_as_pulp_v3() {
        // Chosen so the pairing matters: (1e16 + -1e16) + (1 + 1) = 2, while
        // ((1e16 + 1) + -1e16) + 1 would give 1.
        assert_eq!(reduce_sum([1e16, 1.0, -1e16, 1.0]), 2.0);
    }
}
