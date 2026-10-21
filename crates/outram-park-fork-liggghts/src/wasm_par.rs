// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Serial stand-ins for the `rayon` surface this crate uses, for `wasm32`.
//!
//! # Why this exists
//!
//! `rayon-core` does not build for `wasm32-unknown-unknown` at all: it needs OS
//! threads and fails with `cannot find module or crate sys`. `Cargo.toml`
//! therefore declares `rayon` only under `cfg(not(target_arch = "wasm32"))`,
//! and on `wasm32` the call sites bring in this module instead:
//!
//! ```ignore
//! #[cfg(not(target_arch = "wasm32"))]
//! use rayon::prelude::*;
//! #[cfg(target_arch = "wasm32")]
//! use crate::wasm_par::prelude::*;
//! #[cfg(target_arch = "wasm32")]
//! use crate::wasm_par as rayon;   // so `rayon::ThreadPoolBuilder` resolves here
//! ```
//!
//! This is the same shim `outram-mc-libs` carries, for the same reason and with
//! the same scope discipline.
//!
//! # This runs the work serially, and that is EXACT here
//!
//! Nothing in this module is concurrent. `wasm32-unknown-unknown` is
//! single-threaded, so the honest way to keep one source tree compiling for it
//! is to let the parallel adapters degrade to their sequential equivalents
//! rather than to fake concurrency.
//!
//! For **this crate** the degradation is numerically exact in the strongest
//! sense available: the parallel DEM backend is *required* to be bit-identical
//! to the scalar one for any thread count (see [`crate::compute`]), and it
//! achieves that by accumulating in the serial contact order regardless of how
//! the work was divided. A pool of one is therefore not an approximation of the
//! parallel path — it produces the identical bits, and so does a pool of 64.
//!
//! # Scope
//!
//! Deliberately minimal: exactly the surface this crate uses today. A call site
//! reaching for a rayon API that is not here will fail the `wasm32` build,
//! which is intended — extending the shim should be a deliberate act rather
//! than something a catch-all quietly absorbs.

/// Mirror of `rayon::prelude`, so a call site can swap one import for another.
pub(crate) mod prelude {
    pub(crate) use super::{ParallelSlice, ParallelSliceMut};
}

/// Serial stand-in for `rayon`'s `ParallelSlice`.
pub(crate) trait ParallelSlice<T> {
    /// Sequential stand-in for `rayon`'s `par_chunks`.
    fn par_chunks(&self, size: usize) -> core::slice::Chunks<'_, T>;
}

impl<T> ParallelSlice<T> for [T] {
    fn par_chunks(&self, size: usize) -> core::slice::Chunks<'_, T> {
        self.chunks(size)
    }
}

/// Serial stand-in for `rayon`'s `ParallelSliceMut`.
pub(crate) trait ParallelSliceMut<T> {
    /// Sequential stand-in for `rayon`'s `par_chunks_mut`.
    fn par_chunks_mut(&mut self, size: usize) -> core::slice::ChunksMut<'_, T>;
}

impl<T> ParallelSliceMut<T> for [T] {
    fn par_chunks_mut(&mut self, size: usize) -> core::slice::ChunksMut<'_, T> {
        self.chunks_mut(size)
    }
}

/// Serial stand-in for `rayon::ThreadPoolBuilder`.
///
/// Accepts and ignores a worker count: there is one thread available and asking
/// for more cannot change that. [`ThreadPool::current_num_threads`] reports the
/// truth (`1`) rather than the request.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ThreadPoolBuilder;

impl ThreadPoolBuilder {
    /// Stand-in for `rayon::ThreadPoolBuilder::new`.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Accepts the requested worker count and ignores it — see the type docs.
    pub(crate) fn num_threads(self, _n: usize) -> Self {
        self
    }

    /// Always succeeds. `Result` is kept so the call sites' `.build().expect(…)`
    /// compiles unchanged on both targets.
    pub(crate) fn build(self) -> Result<ThreadPool, core::convert::Infallible> {
        Ok(ThreadPool)
    }
}

/// Serial stand-in for `rayon::ThreadPool`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ThreadPool;

impl ThreadPool {
    /// Runs `op` immediately on the calling thread.
    pub(crate) fn install<R>(&self, op: impl FnOnce() -> R) -> R {
        op()
    }

    /// Always `1` — the honest answer on a single-threaded target, and
    /// deliberately not the count that was requested from
    /// [`ThreadPoolBuilder::num_threads`].
    #[allow(dead_code)]
    pub(crate) fn current_num_threads(&self) -> usize {
        1
    }
}
