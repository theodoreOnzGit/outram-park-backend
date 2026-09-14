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
//! is used where GSL lacks a routine. Two in-workspace upstreams also feed it:
//! `outram-foam-basic-lib` (whose dense-matrix, polynomial and
//! special-function kernels are lifted **verbatim**, so they still diff clean
//! against their source) and
//! `chem-eng-real-time-process-control-simulator` (the transfer-function
//! blocks, themselves ported from the GNU Octave control package).
//!
//! ## Every routine declares its lineage
//!
//! Because the constraint above is only meaningful if it is checkable, each
//! module says which of three categories it falls in, and a reader should know
//! the difference:
//!
//! | Lineage | Meaning | Example |
//! |---|---|---|
//! | **Ported** | Read from a vendored upstream source at a recorded commit, with the upstream file and line named | [`specfunc::ln_gamma`] from GSL `specfunc/gamma.c` |
//! | **Lifted verbatim** | Copied byte-for-byte from an in-workspace crate, with only mechanical `std` → `core`/`alloc` edits, each one listed in the file's PROVENANCE block | [`linalg::SquareMatrix`], all of [`poly`]'s closed-form root finders |
//! | **Delegated** | Forwarded to a dependency that already implements it to a known standard | [`specfunc::erf`], which forwards to `libm`'s fdlibm |
//!
//! A routine in none of those categories does not belong in this crate.
//!
//! # `no_std`
//!
//! This is the workspace's first `no_std` crate, for portability: embedded,
//! wasm, Android. It is `#![no_std]` unconditionally — there is no `std`
//! feature to turn on, because a numerical kernel that only sometimes builds
//! for a microcontroller is one refactor away from not building for one at
//! all. Verified by building for `thumbv7em-none-eabihf` (bare-metal
//! Cortex-M4F) and `wasm32-unknown-unknown`, neither of which has a `std` to
//! fall back on.
//!
//! Consequences that are load-bearing rather than incidental:
//!
//! - **`alloc` is required.** GSL's `gsl_cheb_alloc` sizes its coefficient
//!   array at runtime, and adaptive degree selection is meaningless without a
//!   heap. A const-generic fixed-degree variant may appear later behind a
//!   feature, but is not the primary API.
//! - **`libm` supplies the transcendentals**, which are `std`-only — see
//!   [`real::Real`] for the shim that restores method syntax so ported bodies
//!   need no rewriting. One fixed implementation also means results are
//!   **bit-identical across platforms**, where `std` dispatches to whatever the
//!   system C library ships and those differ in the last ulp.
//! - **Errors are returned, never fatal.** GSL's `gsl_error` calls an
//!   installed handler that aborts the process by default; see [`PetirError`].
//! - **Callbacks are generic `F: Fn(f64) -> f64`**, not `dyn Fn` — GSL's
//!   `gsl_function` is a C function pointer plus a `void*` params block, and
//!   the generic bound is both the natural Rust equivalent and what the
//!   workspace's no-trait-objects rule wants.
//!
//! # Units: bare `f64`, with one deliberate exception
//!
//! The numerics are **dimensionless**. [`linalg`], [`poly`], [`specfunc`] and
//! everything that follows them take and return bare `f64`, per the epic's
//! settled decision — `uom` belongs at the physics crates' API boundaries, not
//! inside a Chebyshev evaluator.
//!
//! The exception is [`transfer_fn`], where units are genuinely load-bearing: a
//! sample time *is* a [`Time`](uom::si::f64::Time) and a dimensionless signal
//! *is* a [`Ratio`](uom::si::f64::Ratio) in the `chem-eng` blocks being ported,
//! and stripping them would be a regression against the source rather than a
//! simplification. It sits behind the default-on `transfer-fn` feature, so a
//! caller who wants only the numerics — `njoy-outram-park-fork` and `raffles`
//! are the motivating cases, both explicitly dependency-lean — gets no `uom` in
//! their dependency graph at all.
//!
//! Note that even there, **polynomial coefficients stay bare `f64`**. The
//! coefficient of `s^k` carries units of `s^k`, so they differ term by term and
//! no single `uom` quantity can type a coefficient vector; forcing one would
//! misstate the physics rather than protect it.
//!
//! # Two upstreams, two conventions — read this before mixing them
//!
//! GSL and Octave disagree about polynomial coefficient order: GSL writes them
//! **ascending** (`c[0]` is the constant term), Octave **descending** (`c[0]`
//! multiplies the highest power). PETIR keeps *both*, each in its own
//! namespace, rather than picking a winner — a ported GSL routine and a ported
//! `.m` routine should each read like its source, or the port stops being
//! checkable. Every function that takes coefficients says which order it means.
//! Reversing them silently reverses the polynomial, so this is the one place in
//! the crate where a careless call compiles and is wrong.
//!
//! # What is here today
//!
//! | Module | Lineage | Covers |
//! |---|---|---|
//! | [`cheb`] | ported | Chebyshev series fitting at Chebyshev nodes, evaluation with error estimate, exact derivative and integral |
//! | [`cheb_slice`] | ported | Adapter between GSL's and `tampines`' Chebyshev coefficient conventions, plus 2-D tensor-product evaluators |
//! | [`expint`] | ported | Exponential integral `E_1` and its scaled form |
//! | [`gamma_inc`] | ported | Regularised lower incomplete gamma, GSL's branch structure (see also the OpenFOAM-lifted `specfunc::inc_gamma`) |
//! | [`linalg`] | lifted + ported | Dense `n x n` Crout LU, determinant, log-determinant, inverse, level-1 BLAS |
//! | [`poly`] | lifted + ported | Horner evaluation and derivatives, Newton divided differences, exact linear/quadratic/cubic roots |
//! | [`specfunc`] | ported + lifted + delegated | Error-function family incl. the scaled `erfcx`, gamma family with GSL's Padé branches, incomplete gamma and its inverse |
//! | [`transfer_fn`] | ported | Continuous and discrete SISO transfer functions, `c2d`/`d2c` (GNU Octave control package, via `chem-eng`) |
//! | [`real`] | — | The `no_std` float-math shim |
//! | [`scalar`] | lifted | Guard constants and machine epsilons |
//!
//! Quadrature, one-dimensional root finding, minimisation, numerical
//! differentiation, ODE integration and interpolation are the epic's remaining
//! scope and are **not** here yet. They are tracked as beads rather than
//! stubbed, because an empty module that looks like an API is worse than an
//! absent one.
//!
//! Chebyshev **least-squares regression at arbitrary points** (as opposed to
//! interpolation at the Chebyshev nodes, which [`cheb`] does) and **adaptive
//! degree selection by tail-chopping** are likewise not done -- `bn:op-bcy5`
//! and `bn:op-0sl9`. The first needs a QR solve this crate does not yet carry.
//!
//! # Example
//!
//! ```
//! use petir::linalg::SquareMatrix;
//!
//! // Solve [[2, 1], [1, 3]] x = [5, 10]  ->  x = [1, 3]
//! let mut a = SquareMatrix::new(2);
//! a.set(0, 0, 2.0); a.set(0, 1, 1.0);
//! a.set(1, 0, 1.0); a.set(1, 1, 3.0);
//! let x = a.solve(&[5.0, 10.0]).unwrap();
//! assert!((x[0] - 1.0).abs() < 1e-12);
//! assert!((x[1] - 3.0).abs() < 1e-12);
//! ```

extern crate alloc;

// The test harness needs `std`; the library itself never does. This is the
// only place `std` is named, and it is gated out of every non-test build.
#[cfg(test)]
extern crate std;

pub mod cheb;
pub mod cheb_slice;
pub mod error;
pub mod expint;
pub mod gamma_inc;
pub mod linalg;
pub mod poly;
pub mod real;
pub mod scalar;
pub mod specfunc;
#[cfg(feature = "transfer-fn")]
pub mod transfer_fn;

pub use cheb::ChebSeries;
pub use cheb_slice::{basis, eval2_dense, eval2_sparse, eval_gsl, eval_plain, scale};
pub use expint::{expint_e1, expint_e1_scaled};
pub use gamma_inc::{gamma_inc_lower, gamma_inc_p};
pub use error::{PetirError, Result};
pub use real::{erf, erfc, lgamma, tgamma, Real};
