// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # CI smoke set, `outram-foam-cli` half (GitHub #415, #314)
//!
//! The push CI's short test selection, chosen by the maintainer on 2026-09-29:
//! **Godiva, Edwards, the outram-foam cavity and the Sod shock tube; that's
//! all.** This crate hosts the two outram-foam cases, because it already
//! depends on `outram-foam-appbuilder-lib`:
//!
//! - [`cavity`]: lid-driven cavity. `pimpleFoam` port vs OpenFOAM icoFoam
//!   (Re = 10), and vs Ghia 1982 (Re = 100) on the coarse and fine meshes;
//! - [`sod`]: Sod shock tube. The exact Riemann solver and the `rhoCentralFoam`
//!   port vs Sod 1978 Table II, plus the timed series.
//!
//! `outram-park-digital-twin-engine`'s `tests/ci_smoke/` hosts Godiva and
//! Edwards.
//!
//! # Why these are COPIES in a TOP crate
//!
//! On CI, compiling costs far more than running tests. The compile gate checks
//! only the crates nothing else depends on (#414), which builds every library
//! once. A test left in `outram-foam-appbuilder-lib` would rebuild that crate a
//! second time as a test program. A copy here reuses the library already
//! built. Each module's header names its source and every edit. **The
//! originals stay authoritative.**
//!
//! ```text
//! cargo test --release -p outram-foam-cli --test ci_smoke
//! ```

mod cavity;
mod sod;
