// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! A flattening multi-way `zip`, used to write parallel-array loops without
//! indexing.
//!
//! # Lineage
//!
//! **Neither ported nor lifted — this is crate-local plumbing**, not a
//! numerical routine, so the "port, never write from scratch" rule in the
//! crate `CLAUDE.md` (which governs *numerics*) does not reach it. It computes
//! nothing; it only changes how a loop is spelled.
//!
//! # Why it exists
//!
//! The ported kernels are full of loops of the shape
//!
//! ```text
//! for (i = 0; i < dim; i++)
//!   ytmp[i] = y[i] + h * (b4[0] * k1[i] + b4[1] * k2[i] + b4[2] * k3[i]);
//! ```
//!
//! Translated literally, every one of those subscripts is a potential panic.
//! They are all provably in range — the vectors are allocated in the same
//! function at the same length — but *provably* there means by reading, not by
//! the compiler, and a panic on a microcontroller is the end of the program.
//! See `tests/no_panic_gate.rs`.
//!
//! Iterators remove the subscripts, but a chain of `zip`s yields a
//! left-nested tuple, so a five-way loop binds `((((a, b), c), d), e)`. That is
//! unreadable in exactly the code where a reviewer most needs to diff against
//! the C. The functions below flatten the tuple back out.
//!
//! # Why not `itertools::izip!`
//!
//! The crate `CLAUDE.md` dependency policy: nothing may be added that pulls
//! `std`. `itertools` is `no_std`-capable, but this is a handful of lines for
//! a single crate-internal use, and a dependency taken on for that would be a
//! poor trade. The functions below are written out one arity at a time rather
//! than recursively (and are plain functions rather than a macro, so rust-analyzer
//! and the call graph see them), so there is nothing to verify beyond reading
//! them.
//!
//! # What it does NOT change
//!
//! **Floating-point association.** The functions only supply the values; the
//! arithmetic expression in the loop body is written exactly as upstream wrote
//! it, so the rounding is identical. A rewrite that accumulated term by term
//! instead — `acc += h * b * k` in a loop over stages — would associate
//! differently and silently change the last bits of every result. That is why
//! the stage expressions below are kept verbatim rather than factored into a
//! helper.

use core::iter::Zip;

/// Zip 2 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. This is `Iterator::zip`
/// itself, returned by its concrete type so that `.rev()`, `.len()` and the
/// other double-ended / exact-size adaptors stay available to the caller.
pub(crate) fn zip2<A, B>(a: A, b: B) -> Zip<A::IntoIter, B::IntoIter>
where
    A: IntoIterator,
    B: IntoIterator,
{
    a.into_iter().zip(b)
}

/// Zip 3 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip3<A, B, C>(a: A, b: B, c: C) -> impl Iterator<Item = (A::Item, B::Item, C::Item)>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
{
    a.into_iter().zip(b).zip(c).map(|((a, b), c)| (a, b, c))
}

/// Zip 4 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip4<A, B, C, D>(
    a: A,
    b: B,
    c: C,
    d: D,
) -> impl Iterator<Item = (A::Item, B::Item, C::Item, D::Item)>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
{
    a.into_iter()
        .zip(b)
        .zip(c)
        .zip(d)
        .map(|(((a, b), c), d)| (a, b, c, d))
}

/// Zip 5 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip5<A, B, C, D, E>(
    a: A,
    b: B,
    c: C,
    d: D,
    e: E,
) -> impl Iterator<Item = (A::Item, B::Item, C::Item, D::Item, E::Item)>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
    E: IntoIterator,
{
    a.into_iter()
        .zip(b)
        .zip(c)
        .zip(d)
        .zip(e)
        .map(|((((a, b), c), d), e)| (a, b, c, d, e))
}

/// Zip 6 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip6<A, B, C, D, E, F>(
    a: A,
    b: B,
    c: C,
    d: D,
    e: E,
    f: F,
) -> impl Iterator<Item = (A::Item, B::Item, C::Item, D::Item, E::Item, F::Item)>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
    E: IntoIterator,
    F: IntoIterator,
{
    a.into_iter()
        .zip(b)
        .zip(c)
        .zip(d)
        .zip(e)
        .zip(f)
        .map(|(((((a, b), c), d), e), f)| (a, b, c, d, e, f))
}

/// Zip 7 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip7<A, B, C, D, E, F, G>(
    a: A,
    b: B,
    c: C,
    d: D,
    e: E,
    f: F,
    g: G,
) -> impl Iterator<
    Item = (
        A::Item,
        B::Item,
        C::Item,
        D::Item,
        E::Item,
        F::Item,
        G::Item,
    ),
>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
    E: IntoIterator,
    F: IntoIterator,
    G: IntoIterator,
{
    a.into_iter()
        .zip(b)
        .zip(c)
        .zip(d)
        .zip(e)
        .zip(f)
        .zip(g)
        .map(|((((((a, b), c), d), e), f), g)| (a, b, c, d, e, f, g))
}

/// Zip 8 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`]. The result is not
/// double-ended; a caller that needs `.rev()` zips with `Iterator::zip`
/// directly and binds the nested tuple.
pub(crate) fn zip8<A, B, C, D, E, F, G, H>(
    a: A,
    b: B,
    c: C,
    d: D,
    e: E,
    f: F,
    g: G,
    h: H,
) -> impl Iterator<
    Item = (
        A::Item,
        B::Item,
        C::Item,
        D::Item,
        E::Item,
        F::Item,
        G::Item,
        H::Item,
    ),
>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
    E: IntoIterator,
    F: IntoIterator,
    G: IntoIterator,
    H: IntoIterator,
{
    a.into_iter()
        .zip(b)
        .zip(c)
        .zip(d)
        .zip(e)
        .zip(f)
        .zip(g)
        .zip(h)
        .map(|(((((((a, b), c), d), e), f), g), h)| (a, b, c, d, e, f, g, h))
}

#[cfg(test)]
mod tests {
    use super::{zip2, zip3, zip4, zip5, zip6, zip7, zip8};
    use alloc::vec::Vec;

    /// Each arity flattens to the tuple the caller expects, in order.
    #[test]
    fn every_arity_yields_a_flat_tuple_in_argument_order() {
        let a = [1, 2, 3];
        let b = [10, 20, 30];
        let c = [100, 200, 300];
        let d = [1000, 2000, 3000];
        let e = [10_000, 20_000, 30_000];
        let f = [100_000, 200_000, 300_000];
        let g = [1_000_000, 2_000_000, 3_000_000];
        let h = [10_000_000, 20_000_000, 30_000_000];

        let v2: Vec<_> = zip2(a.iter(), b.iter()).collect();
        assert_eq!(v2, alloc::vec![(&1, &10), (&2, &20), (&3, &30)]);

        let s3: i64 = zip3(a.iter(), b.iter(), c.iter())
            .map(|(x, y, z)| i64::from(x + y + z))
            .sum();
        assert_eq!(s3, 666);

        let s4: i64 = zip4(a.iter(), b.iter(), c.iter(), d.iter())
            .map(|(w, x, y, z)| i64::from(w + x + y + z))
            .sum();
        assert_eq!(s4, 6666);

        let s5: i64 = zip5(a.iter(), b.iter(), c.iter(), d.iter(), e.iter())
            .map(|(v, w, x, y, z)| i64::from(v + w + x + y + z))
            .sum();
        assert_eq!(s5, 66_666);

        let s6: i64 = zip6(a.iter(), b.iter(), c.iter(), d.iter(), e.iter(), f.iter())
            .map(|(u, v, w, x, y, z)| i64::from(u + v + w + x + y + z))
            .sum();
        assert_eq!(s6, 666_666);

        let s7: i64 = zip7(
            a.iter(),
            b.iter(),
            c.iter(),
            d.iter(),
            e.iter(),
            f.iter(),
            g.iter(),
        )
        .map(|(t, u, v, w, x, y, z)| i64::from(t + u + v + w + x + y + z))
        .sum();
        assert_eq!(s7, 6_666_666);

        let s8: i64 = zip8(
            a.iter(),
            b.iter(),
            c.iter(),
            d.iter(),
            e.iter(),
            f.iter(),
            g.iter(),
            h.iter(),
        )
        .map(|(s, t, u, v, w, x, y, z)| i64::from(s + t + u + v + w + x + y + z))
        .sum();
        assert_eq!(s8, 66_666_666);
    }

    /// Like `Iterator::zip`, the shortest input decides the length -- relied on
    /// nowhere in the kernels (every slice is the same length by construction)
    /// but pinned so a future caller is not surprised.
    #[test]
    fn a_short_input_truncates_rather_than_padding() {
        let a = [1, 2, 3];
        let b = [10, 20];
        let n = zip3(a.iter(), b.iter(), a.iter()).count();
        assert_eq!(n, 2);
    }
}
