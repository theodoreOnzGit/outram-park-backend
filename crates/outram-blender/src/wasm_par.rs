// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.

//! Serial stand-in for the one `rayon` adapter this crate uses
//! (`into_par_iter`, in the geometry plotter), for `wasm32`.
//!
//! `rayon-core` does not build for `wasm32-unknown-unknown` (it needs OS
//! threads), so `rayon` is a dependency only off wasm and the plotter imports
//! this prelude instead there. It runs the work **serially**, which is exact
//! for the plotter: every pixel is computed independently and collected in
//! order, so the image is identical for any thread count.
//!
//! Copied, cut down to the surface used here, from `outram-mc-libs`'
//! `src/wasm_par.rs` when the plotter moved (2026-10-02, GitHub #486). A call
//! site reaching for anything else fails the wasm build, which is intended.

/// Mirror of `rayon::prelude`, so a call site can swap one import for another.
pub(crate) mod prelude {
    pub(crate) use super::IntoParallelIterator;
}

/// `into_par_iter()` as a plain `into_iter()`.
pub(crate) trait IntoParallelIterator: IntoIterator + Sized {
    fn into_par_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.into_iter()
    }
}

impl<T: IntoIterator> IntoParallelIterator for T {}
