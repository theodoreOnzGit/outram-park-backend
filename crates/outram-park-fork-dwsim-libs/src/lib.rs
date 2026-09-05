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

#![forbid(unsafe_code)]

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
pub mod pump;
pub mod reactions;
pub mod reactors;
pub mod separator;
pub mod splitter;
pub mod thermo;
pub mod valve;
