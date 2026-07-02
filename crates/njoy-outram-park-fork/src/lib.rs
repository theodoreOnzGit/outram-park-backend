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

pub mod ace;
pub mod broadr;
pub mod common;
pub mod endf;
/// User friendly interface to access njoy library (used to be called library,
/// but I renamed as interface to avoid mixing with lib.rs) 
///
///
pub mod interface;
pub mod modules;
/// Nuclear-data **provider** surface consumed by downstream transport crates
/// (`openmc-libs`, …). All cross-section representation lives in this crate; see
/// [`nuclear_data`] and `docs/architecture.md`.
pub mod nuclear_data;
pub mod prelude;
pub mod reconr;
/// Thermal neutron scattering (the THERMR domain): read MF=7 S(α,β) evaluations
/// and, in future, compute bound-atom thermal cross sections. Distinct from
/// [`ace::thermal`], which *writes* the thermal ACE table.
pub mod thermal;
pub mod units;
/// Windowed Multipole (WMP) cross-section import — **scaffold only**. This is
/// independent **MIT CRPG** work (not NJOY/LANL); see [`wmp`] for provenance and
/// the MIT attribution requirements. Planned Phase-4 item, after thermal S(α,β).
pub mod wmp;

mod error;
pub use endf::MtReaction;
pub use error::NjoyError;
