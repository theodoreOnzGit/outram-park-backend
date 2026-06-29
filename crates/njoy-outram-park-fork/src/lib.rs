//! # njoy-outram-park-fork
//!
//! Pure-Rust port (**in progress**) of [NJOY2016], the modular nuclear-data
//! processing system that reads evaluated nuclear data in ENDF format and
//! transforms it into libraries used by transport codes. Within OUTRAM PARK its
//! purpose is to produce the **ACE** continuous-energy libraries that
//! [`openmc-libs`] consumes — NJOY is the data-preparation step that sits
//! *upstream* of an OpenMC run.
//!
//! ## License & provenance — read this first
//!
//! This crate is a **derivative work** of NJOY2016 (v2016.79), which is licensed
//! under a modified BSD 3-Clause license (the LANL/DOE variant). That license is
//! GPL-compatible, so this translation is distributed under `GPL-3.0-only` — the
//! same license as the rest of the OUTRAM PARK workspace.
//!
//! It is **not** the version available from LANL and is **not** endorsed by
//! LANL / Los Alamos / the U.S. Government. The upstream copyright notice is
//! preserved verbatim in `LICENSE.njoy`; the full provenance and modification
//! statement is in `NOTICE`. Both files must travel with any redistribution.
//!
//! ## Pipeline (what gets ported, in dependency order)
//!
//! NJOY is a sequence of independent modules that pass ENDF "tapes" to each
//! other. The path that yields an OpenMC-ready ACE file is:
//!
//! ```text
//!   MODER → RECONR → BROADR → [HEATR] → [GASPR] → [PURR] → [THERMR] → ACER
//! ```
//!
//! See `docs/porting-plan.md` for the full module list, the C-source map, the
//! phased porting order, and the verification strategy (golden-file comparison
//! against upstream Fortran NJOY in `../../../NJOY2016`).
//!
//! [NJOY2016]: https://github.com/njoy/NJOY2016
//! [`openmc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend

pub mod broadr;
pub mod common;
pub mod endf;
pub mod interface;
pub mod modules;
pub mod prelude;
pub mod reconr;
pub mod units;

mod error;
pub use endf::MtReaction;
pub use error::NjoyError;
