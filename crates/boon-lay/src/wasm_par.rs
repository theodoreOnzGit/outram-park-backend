// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Serial stand-ins for the `rayon` surface this crate uses, for `wasm32` —
//! bead `op-okqo.1`. Mirrors the same module in `outram-mc-libs`.
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
//! # This runs the work serially, and that is exact here
//!
//! Nothing in this module is concurrent. `wasm32-unknown-unknown` is
//! single-threaded, so the honest way to keep one source tree compiling for it
//! is to let the parallel adapters degrade to their sequential equivalents
//! rather than to fake concurrency.
//!
//! For **this crate** the serial path is already the trusted reference: the CPU
//! Walk-on-Spheres ensemble is precisely what the optional GPU path is
//! validated against, and the per-walker work is independent, so running it on
//! one thread changes wall time and nothing else. If a future kernel is added
//! whose result *does* depend on the worker count, it must not use this shim
//! without saying so.
//!
//! # Scope
//!
//! Deliberately minimal: exactly the surface this crate uses today. A call site
//! reaching for a rayon API that is not here will fail the `wasm32` build,
//! which is intended — extending the shim should be a deliberate act rather
//! than something a catch-all quietly absorbs.

/// Mirror of `rayon::prelude`, so a call site can swap one import for another.
pub(crate) mod prelude {
    pub(crate) use super::{IntoParallelIterator, ParallelSlice, ParallelSliceMut};
}

/// Serial stand-in for `rayon`'s `IntoParallelIterator`.
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

/// Serial stand-in for `rayon::ThreadPoolBuilder`.
///
/// Accepts and ignores a worker count: there is one thread available and
/// asking for more cannot change that. The requested count is retained only so
/// [`ThreadPool::current_num_threads`] can report the truth (`1`) rather than
/// the request.
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
    pub(crate) fn current_num_threads(&self) -> usize {
        1
    }
}
