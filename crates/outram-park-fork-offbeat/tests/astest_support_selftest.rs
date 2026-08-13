// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Self-tests for the shared `astest` driver.
//!
//! The driver in [`astest_support`] is test-support code, so it has no test
//! binary of its own — Rust compiles `tests/<name>/mod.rs` only when some test
//! file declares it. This file exists to declare it, so the driver's own unit
//! tests run even when no deck-specific test does.
//!
//! Those unit tests verify the two things a deck reproduction depends on and
//! that nothing else checks: that the interpolators reproduce code_aster's
//! `DEFI_FONCTION`/`DEFI_NAPPE` semantics including both extrapolation modes,
//! and that the mixed strain/stress solve recovers uniaxial elasticity exactly
//! against its closed form.

mod astest_support;
