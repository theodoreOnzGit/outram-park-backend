//! # outram-park-fork-pflotran
//!
//! An independent, pure-Rust fork / translation of
//! [PFLOTRAN](https://www.pflotran.org) — the US-DOE national-lab subsurface
//! **flow and reactive-transport** simulator (Fortran + PETSc, massively
//! parallel) — rebuilt to OUTRAM PARK's design rules: enum dispatch (no trait
//! objects), `uom`-typed API boundaries, a pure-Rust solver (no PETSc FFI, no
//! MPI in v1), and an Android-buildable library.
//!
//! > **⚠️ Early v1 — RICHARDS solves, but is VERIFICATION-ONLY.** The RICHARDS
//! > flow mode ([`flow::RichardsSimulation`]) runs end-to-end: a structured
//! > Cartesian two-point-flux discretisation, backward-Euler in time, solved by
//! > the pure-Rust Newton–Krylov [`solver`] over the foam-basic-lib `krylov`
//! > backend, driven from a parsed input deck ([`io`]). It has been **verified**
//! > (closed-form saturated steady state; method of manufactured solutions) but
//! > **not validated** against published PFLOTRAN reference cases (bead
//! > op-v6s.9). Do not treat any output as validated. Later flow modes (TH,
//! > GENERAL multiphase, transport, geochemistry) are not implemented and their
//! > entry points return [`error::PflotranError::NotImplemented`].
//! >
//! > **Independent fork, not the official PFLOTRAN.** "PFLOTRAN" names only the
//! > upstream work this crate derives from; nothing here is endorsed by or
//! > affiliated with the PFLOTRAN development team or the national laboratories
//! > (LANL, PNNL, ORNL, LBNL, SNL). See `NOTICE` and the workspace
//! > `TRADEMARKS.md`.
//! >
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. No human V&V has been performed. Not for
//! > nuclear facility operation, reactor control, safety-critical analysis, or
//! > licensing decisions — this is for education, research, and V&V only.
//!
//! ## v1 scope — the vertical slice (bead op-v6s.2)
//!
//! The first end-to-end target is deliberately narrow, so a real physics result
//! can be validated before breadth is added:
//!
//! - **Flow mode:** RICHARDS — variably-saturated single-phase groundwater flow.
//! - **Grid:** structured Cartesian finite volume, two-point flux.
//! - **Solver:** serial pure-Rust Newton-Krylov (no PETSc, no MPI).
//! - **I/O:** a minimal card-based ASCII input-deck subset; CSV/VTK output.
//!
//! Explicitly **out of v1**: unstructured grids, MPI / distributed solves,
//! HDF5, multiphase (GENERAL) flow, energy transport (TH), solute transport,
//! and reactive geochemistry (GIRT). Those are later beads (op-v6s.10..op-v6s.14).
//!
//! ## Module map — what belongs where
//!
//! | Module | PFLOTRAN analogue | Status |
//! |---|---|---|
//! | [`units`] | dimensional quantities used throughout | **real** — named `uom` type aliases (a human hovers `Pressure`, not a raw `Quantity`) |
//! | [`error`] | error handling | **real** — the crate [`error::PflotranError`] enum |
//! | [`flow`] | `pm_*` process-model / flow-mode polymorphism | **working (verification-only)** — [`flow::FlowMode`] + [`flow::RichardsSimulation`]: RICHARDS residual/Jacobian + adaptive timestep (bead op-v6s.8) |
//! | [`grid`] | `discretization` / `grid` structured FV | **scaffold** — structured Cartesian FV (bead op-v6s.5) |
//! | [`solver`] | PETSc SNES/KSP replacement | **scaffold** — Newton–Krylov over foam-basic-lib `krylov` (bead op-v6s.4, KEYSTONE) |
//! | [`properties`] | EOS + characteristic curves | **scaffold** — EOS + retention/rel-perm curves (bead op-v6s.7) |
//! | [`io`] | input-deck cards + output | **scaffold** — card-deck subset + CSV/VTK (bead op-v6s.6) |
//!
//! ## Design rules (workspace mandate)
//!
//! - **Enum dispatch, no trait objects.** Flow modes, EOS forms, and solver
//!   kinds are enums matched exhaustively — see [`flow::FlowMode`]. A trait may
//!   still act as a compiler-checked contract on each concrete mode, but never
//!   as `Box<dyn _>` runtime dispatch.
//! - **`uom` at API boundaries.** Every physical quantity crossing a public
//!   boundary is a [`units`] alias, so units are checked at compile time.
//! - **Pure Rust, Android-safe.** No PETSc, no MPI, no system BLAS, no C/Fortran
//!   toolchain in the library build.

pub mod error;
pub mod flow;
pub mod grid;
pub mod io;
pub mod properties;
pub mod solver;
pub mod units;

pub use error::PflotranError;
pub use flow::FlowMode;

/// Convenience `Result` alias for the crate's fallible operations.
///
/// The error variant is always [`PflotranError`]; scaffold entry points that
/// are not implemented yet return [`PflotranError::NotImplemented`] rather than
/// panicking or returning a fabricated value.
pub type Result<T> = core::result::Result<T, PflotranError>;
