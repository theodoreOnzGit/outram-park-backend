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
//! against upstream Fortran NJOY in `upstream_source/NJOY2016`).
//!
//! [NJOY2016]: https://github.com/njoy/NJOY2016
//! [`openmc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend

pub mod acer;
/// HIGH-fidelity data acquisition — download raw ENDF tapes from a pinned
/// upstream and reconstruct pointwise cross sections on device. Behind the
/// **`net-fetch`** feature (opt-in; the default build ships the offline LOW tier
/// only). See [`acquire`] and `docs/data-acquisition.md`.
#[cfg(feature = "net-fetch")]
pub mod acquire;
pub mod broadr;
pub mod common;
pub mod endf;
/// `GASPR` — gas-production cross sections (ENDF MT=203–207: H1/H2/H3/He3/He4)
/// from a reconstructed evaluation. Informational (depletion/swelling), not a
/// transport-path quantity — see [`gaspr`] module docs.
pub mod gaspr;
/// `HEATR` — heating (KERMA) cross section, ENDF MT=301. Kinematic-limit
/// subset (H1–H4, `docs/porting-plan.md`); the full photon energy-balance
/// method and damage energy (MT=444) are deferred — see [`heatr`] module docs.
pub mod heatr;
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
pub mod photon;
pub mod prelude;
pub mod reconr;
/// Thermal neutron scattering (the THERMR domain): read MF=7 S(α,β) evaluations
/// and, in future, compute bound-atom thermal cross sections. Distinct from
/// [`acer::thermal`], which *writes* the thermal ACE table.
pub mod thermr;
pub mod units;
/// Windowed Multipole (WMP) cross-section import — **scaffold only**. This is
/// independent **MIT CRPG** work (not NJOY/LANL); see [`wmp`] for provenance and
/// the MIT attribution requirements. Planned Phase-4 item, after thermal S(α,β).
pub mod wmp;

// --- NJOY processing modules — one home per module under `src/<name>/`, each
// with its Rust source (physics or `NotPorted` stub) and co-located `README.md`.
// The ported ones (reconr, broadr, heatr, gaspr, thermr, acer) are declared
// above next to their long-form docs; the remainder are scaffolds. Dispatch is
// via the `NjoyModule` enum in `modules.rs`.
pub mod moder;
pub mod unresr;
pub mod purr;
pub mod groupr;
pub mod gaminr;
pub mod errorr;
pub mod covr;
pub mod leapr;
pub mod samm;
pub mod dtfr;
pub mod ccccr;
pub mod matxsr;
pub mod resxsr;
pub mod powr;
pub mod plotr;
pub mod viewr;
pub mod mixr;
pub mod wimsr;

mod error;
pub use endf::MtReaction;
pub use error::NjoyError;
