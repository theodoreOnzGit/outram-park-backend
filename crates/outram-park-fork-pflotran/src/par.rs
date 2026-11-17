// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Serial stand-ins for the `rayon` surface this crate uses, for `no_std`
//! builds (gh:#1603), where there is no thread pool.
//!
//! The call sites import `rayon::prelude::*` with `std` and this module's
//! `prelude` without it. Every parallel loop in this crate is a per-cell
//! `map` + ordered `collect`, or a per-element (or per-chunk) `for_each` writing only
//! its own element, so running it in order gives the same values as the thread pool.
//!
//! Deliberately minimal: exactly the surface this crate uses. A call site
//! reaching for anything else fails the `no_std` build, so extending it is a
//! deliberate act.

/// Mirror of `rayon::prelude`.
pub(crate) mod prelude {
    pub(crate) use super::{IntoParallelIterator, ParallelSliceMut};
}

/// Serial stand-in for `rayon`'s `IntoParallelIterator` on index ranges.
pub(crate) trait IntoParallelIterator {
    /// The sequential iterator returned.
    type Iter: Iterator;
    /// Sequential stand-in for `into_par_iter`.
    fn into_par_iter(self) -> Self::Iter;
}

impl IntoParallelIterator for core::ops::Range<usize> {
    type Iter = core::ops::Range<usize>;
    fn into_par_iter(self) -> Self::Iter {
        self
    }
}

/// Serial stand-in for `rayon`'s `par_iter_mut` on slices.
pub(crate) trait ParallelSliceMut<T> {
    /// Sequential stand-in for `par_iter_mut`.
    fn par_iter_mut(&mut self) -> core::slice::IterMut<'_, T>;
    /// Sequential stand-in for `par_chunks_mut`.
    fn par_chunks_mut(&mut self, size: usize) -> core::slice::ChunksMut<'_, T>;
}

impl<T> ParallelSliceMut<T> for [T] {
    fn par_iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.iter_mut()
    }
    fn par_chunks_mut(&mut self, size: usize) -> core::slice::ChunksMut<'_, T> {
        self.chunks_mut(size)
    }
}
