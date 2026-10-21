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
//! the C. These macro arms flatten the tuple back out.
//!
//! # Why not `itertools::izip!`
//!
//! The crate `CLAUDE.md` dependency policy: nothing may be added that pulls
//! `std`. `itertools` is `no_std`-capable, but this is seven lines of macro for
//! a single crate-internal use, and a dependency taken on for that would be a
//! poor trade. The arms below are written out one arity at a time rather than
//! recursively, so there is nothing to verify beyond reading them.
//!
//! # What it does NOT change
//!
//! **Floating-point association.** The macro only supplies the values; the
//! arithmetic expression in the loop body is written exactly as upstream wrote
//! it, so the rounding is identical. A rewrite that accumulated term by term
//! instead — `acc += h * b * k` in a loop over stages — would associate
//! differently and silently change the last bits of every result. That is why
//! the stage expressions below are kept verbatim rather than factored into a
//! helper.

/// Zip 2 to 8 iterators, yielding a flat tuple.
///
/// Stops at the shortest input, like [`Iterator::zip`].
macro_rules! zip_flat {
    ($a:expr, $b:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a).zip($b)
    };
    ($a:expr, $b:expr, $c:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .map(|((a, b), c)| (a, b, c))
    };
    ($a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .zip($d)
            .map(|(((a, b), c), d)| (a, b, c, d))
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .zip($d)
            .zip($e)
            .map(|((((a, b), c), d), e)| (a, b, c, d, e))
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .zip($d)
            .zip($e)
            .zip($f)
            .map(|(((((a, b), c), d), e), f)| (a, b, c, d, e, f))
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .zip($d)
            .zip($e)
            .zip($f)
            .zip($g)
            .map(|((((((a, b), c), d), e), f), g)| (a, b, c, d, e, f, g))
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr $(,)?) => {
        ::core::iter::IntoIterator::into_iter($a)
            .zip($b)
            .zip($c)
            .zip($d)
            .zip($e)
            .zip($f)
            .zip($g)
            .zip($h)
            .map(|(((((((a, b), c), d), e), f), g), h)| (a, b, c, d, e, f, g, h))
    };
}

pub(crate) use zip_flat;

#[cfg(test)]
mod tests {
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

        let v2: Vec<_> = zip_flat!(a.iter(), b.iter()).collect();
        assert_eq!(v2, alloc::vec![(&1, &10), (&2, &20), (&3, &30)]);

        let s3: i64 = zip_flat!(a.iter(), b.iter(), c.iter())
            .map(|(x, y, z)| i64::from(x + y + z))
            .sum();
        assert_eq!(s3, 666);

        let s4: i64 = zip_flat!(a.iter(), b.iter(), c.iter(), d.iter())
            .map(|(w, x, y, z)| i64::from(w + x + y + z))
            .sum();
        assert_eq!(s4, 6666);

        let s5: i64 = zip_flat!(a.iter(), b.iter(), c.iter(), d.iter(), e.iter())
            .map(|(v, w, x, y, z)| i64::from(v + w + x + y + z))
            .sum();
        assert_eq!(s5, 66_666);

        let s6: i64 = zip_flat!(a.iter(), b.iter(), c.iter(), d.iter(), e.iter(), f.iter())
            .map(|(u, v, w, x, y, z)| i64::from(u + v + w + x + y + z))
            .sum();
        assert_eq!(s6, 666_666);

        let s7: i64 = zip_flat!(
            a.iter(),
            b.iter(),
            c.iter(),
            d.iter(),
            e.iter(),
            f.iter(),
            g.iter()
        )
        .map(|(t, u, v, w, x, y, z)| i64::from(t + u + v + w + x + y + z))
        .sum();
        assert_eq!(s7, 6_666_666);

        let s8: i64 = zip_flat!(
            a.iter(),
            b.iter(),
            c.iter(),
            d.iter(),
            e.iter(),
            f.iter(),
            g.iter(),
            h.iter()
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
        let n = zip_flat!(a.iter(), b.iter(), a.iter()).count();
        assert_eq!(n, 2);
    }
}
