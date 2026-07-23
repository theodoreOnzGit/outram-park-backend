//! # outram-park-fork-pflotran
//!
//! An independent, pure-Rust fork / translation of
//! [PFLOTRAN](https://www.pflotran.org) — the US-DOE national-lab subsurface
//! **flow and reactive-transport** simulator (Fortran + PETSc, massively
//! parallel) — rebuilt to OUTRAM PARK's design rules: enum dispatch (no trait
//! objects), `uom`-typed API boundaries, a pure-Rust solver (no PETSc FFI, no
//! MPI in v1), and an Android-buildable library.
//!
//! > **⚠️ VERIFICATION-ONLY — no validation, no human V&V yet.** Multiple modes
//! > are implemented and unit/verification-tested, but NONE has been validated
//! > against published PFLOTRAN reference cases (all validation is bead-tracked
//! > and deferred: op-v6s.9.x/.10.1/.11.1/.12.1/.13.1). Do not treat any output
//! > as validated. Implemented so far, each **verified** against closed-form /
//! > manufactured references only:
//! > - **RICHARDS** variably-saturated flow ([`flow::RichardsSimulation`]) — MMS 2nd-order.
//! > - **Solute transport** ([`transport`]) + **TH heat transport** ([`energy`]) — closed-form advection–diffusion/conduction.
//! > - **Aqueous geochemistry** ([`geochemistry`]) + **kinetics** ([`kinetics`]) + **reactive transport** ([`reactive_transport`]).
//! > - **Two-phase (air–water) multiphase flow** ([`multiphase`]) on the block
//! >   solver ([`solver::block`]).
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
//! | [`grid`] | `discretization` / `grid` structured FV | **real** — structured Cartesian FV, two-point flux (bead op-v6s.5) |
//! | [`solver`] | PETSc SNES/KSP replacement | **real** — scalar + block ([`solver::block`]) Newton–Krylov over foam-basic-lib `krylov` (beads op-v6s.4, op-v6s.4.1) |
//! | [`properties`] | EOS + characteristic curves | **real** — EOS, van Genuchten/Brooks–Corey/Haverkamp curves, thermal properties (beads op-v6s.7, op-v6s.10) |
//! | [`io`] | input-deck cards + output | **real (AI-designed subset)** — card-deck + CSV/VTK (bead op-v6s.6) |
//! | [`transport`] | conservative solute transport | **working (verification-only)** — advection–diffusion, coupled to a RICHARDS flow field (bead op-v6s.11) |
//! | [`energy`] | TH heat transport | **working (verification-only)** — advection–conduction of temperature, one-way coupled to flow (bead op-v6s.10) |
//! | [`geochemistry`] | aqueous speciation | **working (verification-only)** — equilibrium speciation (GIRT core; bead op-v6s.12) |
//! | [`kinetics`] | mineral kinetics | **working (verification-only)** — TST precipitation/dissolution on a foam ODE solver (bead op-v6s.12) |
//! | [`reactive_transport`] | GIRT reactive transport | **working (verification-only)** — SNIA transport↔geochemistry coupling (bead op-v6s.12) |
//! | [`multiphase`] | GENERAL multiphase flow | **working (verification-only)** — two-phase air–water on the block solver (bead op-v6s.13) |
//! | [`activity`] | aqueous activity models | **real** — Debye–Hückel / Davies coefficients (bead op-v6s.15.1) |
//! | [`sorption`] | sorption + ion exchange | **real** — Kd/Langmuir/Freundlich isotherms + Gaines–Thomas exchange (bead op-v6s.15.2) |
//! | [`decay`] | radioactive decay | **real** — Bateman decay chains + ingrowth (bead op-v6s.15.3) |
//! | [`eos_real`] | real fluid EOS | **real** — IAPWS-IF97 liquid water via `tampines-steam-tables` (bead op-v6s.15.7) |
//! | [`microbial`] | microbial reactions | **real** — Monod/dual-Monod biodegradation on a foam ODE solver (bead op-v6s.15.4) |
//! | [`wells`] | wells + advanced BCs | **real** — Peaceman well index + hydrostatic/seepage/time-varying BCs (bead op-v6s.15.12) |
//! | [`deck`] | real PFLOTRAN input deck | **real (subset)** — genuine PFLOTRAN keyword-block syntax, Fortran D-exponent floats (bead op-v6s.15.10) |
//!
//! Modules op-v6s.15.1/.2/.3/.4/.7/.10/.12 above are standalone building blocks
//! (upstream-parity gaps); the sorption/decay pieces are wired into
//! [`transport`], while the others are self-contained and not yet wired into the
//! flow/transport/geochemistry hot loops.
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

pub mod activity;
pub mod deck;
pub mod energy;
pub mod decay;
pub mod eos_real;
pub mod error;
pub mod flow;
pub mod geochemistry;
/// Optional wgpu GPU acceleration — compiled only off Android (the workspace GPU
/// rule keeps GPU deps out of the Android library build); CPU is the trusted path.
#[cfg(not(target_os = "android"))]
pub mod gpu;
pub mod grid;
pub mod io;
pub mod kinetics;
pub mod microbial;
pub mod multiphase;
pub mod properties;
pub mod reactive_transport;
pub mod solver;
pub mod sorption;
pub mod transport;
pub mod units;
pub mod wells;

pub use error::PflotranError;
pub use flow::{FlowMode, RichardsSimulation};

/// Convenience `Result` alias for the crate's fallible operations.
///
/// The error variant is always [`PflotranError`]; scaffold entry points that
/// are not implemented yet return [`PflotranError::NotImplemented`] rather than
/// panicking or returning a fabricated value.
pub type Result<T> = core::result::Result<T, PflotranError>;
