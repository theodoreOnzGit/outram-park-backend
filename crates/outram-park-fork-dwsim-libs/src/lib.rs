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
//!   packages) those equipment models draw on.
//! - **Does NOT belong here:** DWSIM's GUI, XML/JSON serialization,
//!   property-grid reflection, or flowsheet-solver plumbing -- none of that
//!   is physics, and none of it is ported (see each module's doc comment for
//!   what was deliberately excluded from its source file).

#![forbid(unsafe_code)]

pub mod compressor;
pub mod cooler;
pub mod expander;
pub mod heat_exchanger;
pub mod heater;
pub mod interpolation;
pub mod mixer;
pub mod pipe;
pub mod pump;
pub mod reactions;
pub mod reactors;
pub mod separator;
pub mod splitter;
pub mod thermo;
pub mod valve;
