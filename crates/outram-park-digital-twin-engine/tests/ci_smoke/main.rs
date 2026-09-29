// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # CI smoke set, `outram-park-digital-twin-engine` half (GitHub #415, #314)
//!
//! The push CI's short test selection, chosen by the maintainer on 2026-09-29:
//! **Godiva, Edwards, the outram-foam cavity and the Sod shock tube; that's
//! all.** This crate hosts the two whose libraries it already depends on:
//!
//! - [`godiva`]: HEU-MET-FAST-001 on `outram-mc-libs`, **HIGH tier** from
//!   the repo's ENDF/B-VIII.0 tapes;
//! - [`edwards`]: Edwards–O'Brien blowdown on `tampines`' drift flux.
//!
//! `outram-foam-cli`'s `tests/ci_smoke/` hosts the other two.
//!
//! # Why these are COPIES in a TOP crate
//!
//! On CI, compiling costs far more than running tests (measured in
//! `crates/kovan/src/commands/affected.rs`). The compile gate checks only the
//! crates nothing else depends on (#414), which builds every library once. A
//! test left in `tampines` or `outram-mc-libs` would rebuild that crate a second
//! time as a test program. A copy here reuses the libraries already built.
//!
//! Each module names its source, the commit it was copied at, and what was
//! changed. **The originals stay authoritative.** `ci/smoke-tests.toml` records
//! every pairing, and `kovan-cli ci smoke` fails if a source goes missing.
//!
//! ```text
//! cargo test --release -p outram-park-digital-twin-engine --test ci_smoke
//! ```

mod edwards;
mod godiva;
