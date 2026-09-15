// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Serial stand-ins for the handful of `rayon` adapters this crate uses, for
//! `wasm32` — bead `op-okqo.1`.
//!
//! # Why this exists
//!
//! `rayon-core` does not build for `wasm32-unknown-unknown` at all: it needs OS
//! threads and fails with `cannot find module or crate sys`. The crate's
//! `Cargo.toml` therefore declares `rayon` only under
//! `cfg(not(target_arch = "wasm32"))`, and on `wasm32` the call sites import
//! this module instead of `rayon::prelude`.
//!
//! # What it is NOT
//!
//! It is **not** a parallelism implementation, and it does not pretend to be
//! one. Every method here runs the work **serially** on the calling thread.
//! `wasm32-unknown-unknown` is single-threaded; the honest way to keep the same
//! source compiling for it is to let the parallel adapters degrade to their
//! sequential equivalents, not to fake concurrency.
//!
//! The arithmetic is unaffected. Every use of these adapters in this crate maps
//! independent work over an index range or a slice and collects the results in
//! order, so the serial path produces bit-identical output — it is only slower.
//! That is the same reasoning as `outram-foam-basic-lib`'s
//! `ComputeBackend::Serial`, which is likewise the reference implementation
//! rather than a fallback of lesser accuracy.
//!
//! # Scope
//!
//! Deliberately minimal — it covers exactly the three adapters this crate uses
//! (`into_par_iter`, `par_iter`, `par_iter_mut`) and nothing else. If a new
//! call site needs a rayon adapter that is not here, the compiler will say so
//! on the `wasm32` build, which is the intended behaviour: adding a method
//! should be a deliberate act, not something a blanket shim hides.

/// Serial stand-in for `rayon`'s `IntoParallelIterator`.
///
/// Blanket-implemented for every [`IntoIterator`], so `(0..n).into_par_iter()`
/// and `vec.into_par_iter()` both resolve to the ordinary sequential iterator.
pub(crate) trait IntoParallelIterator: IntoIterator + Sized {
    /// Sequential stand-in for `rayon`'s `into_par_iter`.
    fn into_par_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.into_iter()
    }
}

impl<T: IntoIterator> IntoParallelIterator for T {}

/// Serial stand-in for `rayon`'s `ParallelSlice`.
pub(crate) trait ParallelSlice<T> {
    /// Sequential stand-in for `rayon`'s `par_iter`.
    fn par_iter(&self) -> core::slice::Iter<'_, T>;
}

impl<T> ParallelSlice<T> for [T] {
    fn par_iter(&self) -> core::slice::Iter<'_, T> {
        self.iter()
    }
}

/// Serial stand-in for `rayon`'s `ParallelSliceMut`.
pub(crate) trait ParallelSliceMut<T> {
    /// Sequential stand-in for `rayon`'s `par_iter_mut`.
    fn par_iter_mut(&mut self) -> core::slice::IterMut<'_, T>;
}

impl<T> ParallelSliceMut<T> for [T] {
    fn par_iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.iter_mut()
    }
}
