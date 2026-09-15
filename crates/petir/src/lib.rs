// PETIR — Polynomials, Equations, Transforms, Integration, Roots.
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
// GPL-3.0-only. Contains work derived from the GNU Scientific Library
// (GPL-3.0-or-later); see NOTICE for provenance and per-file attribution.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! **PETIR** — **P**olynomials, **E**quations, **T**ransforms, **I**ntegration
//! and **R**oots: the workspace's core numerics library.
//!
//! <!-- vv-unverified-banner -->
//! > ⚠️ **Unverified until validated.** All code in this workspace is
//! > **unverified and untrusted** unless a specific verification & validation
//! > (V&V) case demonstrates otherwise. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.
//!
//! # What this crate is, and what it refuses to be
//!
//! Every numerical routine here is **ported from an upstream library that has
//! its own test suite**, never written from a textbook formula. That is the
//! governing constraint of the crate (`bn:op-chyp` decision 2), and it is a
//! provenance argument rather than a stylistic one: a kernel written from a
//! formula has no lineage and no reference suite, so a reviewer has nothing to
//! diff it against. A port can be opened next to its upstream file and read
//! line for line, and it inherits that upstream's V&V.
//!
//! The primary source is the **GNU Scientific Library** (GPL-3.0-or-later,
//! verified from its per-file headers — see `NOTICE`). SLATEC (public domain)
//! is used where GSL lacks a routine.
//!
//! # `no_std`
//!
//! This is the workspace's first `no_std` crate, for portability: embedded,
//! wasm, Android. Consequences that are load-bearing rather than incidental:
//!
//! - **`alloc` is required.** GSL's `gsl_cheb_alloc` sizes its coefficient
//!   array at runtime, and adaptive degree selection is meaningless without a
//!   heap. A const-generic fixed-degree variant may appear later behind a
//!   feature, but is not the primary API.
//! - **`libm` supplies the transcendentals**, which are `std`-only. One fixed
//!   implementation also means results are **bit-identical across platforms**,
//!   where `std` dispatches to whatever the system C library ships and those
//!   differ in the last ulp.
//! - **Errors are returned, never fatal.** GSL's `gsl_error` calls an
//!   installed handler that aborts the process by default; see [`PetirError`].
//! - **Callbacks are generic `F: Fn(f64) -> f64`**, not `dyn Fn` — GSL's
//!   `gsl_function` is a C function pointer plus a `void*` params block, and
//!   the generic bound is both the natural Rust equivalent and what the
//!   workspace's no-trait-objects rule wants.

extern crate alloc;

// Only for the `platform-libm` feature, whose whole purpose is to call the
// platform's `f64::exp`/`ln`/`powf` -- inherent methods that live in `std`,
// not `core`. The crate stays `no_std` in every other configuration.
#[cfg(feature = "platform-libm")]
extern crate std;

pub mod cheb;
pub mod cheb_slice;
pub mod error;
pub mod expint;
pub mod fast_exp;
pub mod fast_log;
pub mod fast_pow;
pub mod gamma_inc;
pub mod mathf;
pub mod real;

pub use cheb::ChebSeries;
pub use cheb_slice::{basis, eval2_dense, eval2_sparse, eval_gsl, eval_plain, scale};
pub use error::{PetirError, Result};
pub use expint::{expint_e1, expint_e1_scaled};
pub use gamma_inc::{gamma_inc_lower, gamma_inc_p};
pub use real::{erf, erfc, lgamma, tgamma};
