//! # outram-park-fork-dwsim-libs
//!
//! Pure-Rust translation of selected [DWSIM](https://dwsim.org) chemical-
//! process equipment models and correlations -- an independent OUTRAM PARK
//! fork, not the official DWSIM software (see `TRADEMARKS.md`). See
//! `CLAUDE.md` for build/test instructions and `docs/port-scope.md` for the
//! prioritised porting scope, C# source map, and porting order.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** equipment-model correlations translated from DWSIM's
//!   `UnitOperations` (pipe pressure drop, valve sizing, heat-exchanger
//!   rating, pump/expander thermodynamics, [`reactors`] built on the
//!   [`reactions`] model) with `uom`-typed public APIs, plus the [`thermo`]
//!   thermodynamics kernel (EOS, activity models, flash algorithms, property
//!   packages) those equipment models draw on. **Since 2026-08 the flowsheet
//!   and dynamics layer belongs here too** (explicit maintainer scope change):
//!   the [`flowsheet`] data model (material/energy streams, object graph,
//!   connections, from `DWSIM.FlowsheetBase`), the [`flowsheet_solver`]
//!   sequential-modular execution engine (calculation ordering, cycle
//!   detection, tear streams, recycle convergence, dynamic-mode branch, from
//!   `DWSIM.FlowsheetSolver`), the [`dynamics`] transient-simulation manager
//!   (integrator state, schedules, events, monitored variables,
//!   cause-and-effect matrix, from `DWSIM.DynamicsManager`), and the
//!   [`columns`] rigorous MESH distillation column and its solvers (from
//!   `DWSIM.UnitOperations` `RigorousColumn.vb` + `RigorousColumnSolvers/`),
//!   the [`petroleum`] crude-assay / pseudo-component characterization layer
//!   (from `DWSIM.Thermodynamics/PetroleumCharacterization` +
//!   `DWSIM.SharedClasses` `AssayClass.vb`), and the [`clean_energies`]
//!   unit operations (water electrolyzer, PEM fuel cells, solar/wind/hydro,
//!   from `DWSIM.UnitOperations/UnitOperations/CleanEnergies/`).
//! - **Does NOT belong here:** DWSIM's GUI, XML/JSON serialization,
//!   property-grid reflection, .NET designer files, or Windows-Forms
//!   plumbing -- none of that is physics, and none of it is ported (see each
//!   module's doc comment for what was deliberately excluded from its source
//!   file).
//!
//! ## Where to start
//!
//! `use outram_park_fork_dwsim_libs::prelude::*;` — the [`prelude`] is the
//! curated map of entry points (crude characterisation, the rigorous column,
//! property packages, the struct-based unit operations, and the `uom` unit
//! markers the API takes). Every module path below is still reachable for the
//! long tail; the prelude's own docs say what it leaves out and why.

#![forbid(unsafe_code)]

// `no_std` + `alloc` without the default `std` feature (gh:#1591, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;
extern crate ndarray_nostd as ndarray;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}

/// Hash maps and sets: std's in a `std` build (and in tests), so iteration
/// order and every result are exactly as before; `hashbrown` without std, the
/// workspace rule (gh:#1591). `HashSet` is used only by the `std` importer.
#[allow(unused_imports)]
pub(crate) mod hash {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::collections::{HashMap, HashSet};
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use hashbrown::{HashMap, HashSet};
}

/// The wall clock behind solver and runner timeouts (gh:#1591). With `std`
/// this is `std::time::Instant`. Without it there is no clock: `now()` is a
/// fixed instant and `elapsed()` is always zero, so a wall-clock timeout never
/// fires and the iteration limits alone bound a solve.
pub mod time {
    #[cfg(any(feature = "std", test))]
    pub use std::time::Instant;

    /// Stand-in for `std::time::Instant` without std: no clock.
    #[cfg(not(any(feature = "std", test)))]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Instant;

    #[cfg(not(any(feature = "std", test)))]
    impl Instant {
        /// A fixed instant: there is no clock to read.
        pub fn now() -> Self {
            Instant
        }
        /// Always zero without a clock.
        pub fn elapsed(&self) -> core::time::Duration {
            core::time::Duration::ZERO
        }
    }
}

pub mod clean_energies;
pub mod columns;
pub mod compressor;
pub mod cooler;
pub mod dynamics;
pub mod flowsheet;
pub mod flowsheet_solver;
pub mod expander;
pub mod heat_exchanger;
pub mod heater;
pub mod interpolation;
pub mod mixer;
pub mod petroleum;
pub mod pipe;
pub mod prelude;
pub mod pump;
pub mod reactions;
pub mod reactors;
pub mod separator;
pub mod splitter;
pub mod thermo;
pub mod valve;
