// SPDX-License-Identifier: GPL-3.0

//! # CHANGI — atmospheric consequences
//!
//! **C**onsequence and **H**azard **A**nalysis for **N**uclear **G**round-level
//! and atmospheric **I**mpacts.
//!
//! CHANGI answers *"what happens after release?"*: how a radionuclide plume is
//! transported through the atmosphere, how it is depleted by dry and wet
//! deposition and radioactive decay, where it reaches the ground, and what
//! consequence that implies.
//!
//! It is the middle link of the OUTRAM PARK offsite chain — SEMBAWANG (severe
//! accident: *what gets released?*) → **CHANGI** (*what happens after
//! release?*) → REDHILL (*what happens after deposition and infiltration?*).
//! Its input is a source term produced by SEMBAWANG; until that crate exists, a
//! release-rate time series must be supplied by hand.
//!
//! ## Scope
//!
//! Atmospheric dispersion; plume transport; radionuclide deposition; ground
//! contamination; radiological consequence assessment; dose assessment;
//! emergency planning support; Level 3 PSA support.
//!
//! ## Intended use — binding limit
//!
//! **CHANGI is for research, education and verification/validation only.** Its
//! outputs must **not** be treated as authoritative for operational emergency
//! response, dose assessment of real populations, regulatory or licensing
//! decisions, nuclear facility operation, or any safety-critical purpose.
//!
//! The line, drawn by `docs/ecosystem-naming.md` decision 3 (2026-08-05), is
//! between **capability and framing**: *"The capability is in scope; that
//! framing is not."* Implementing consequence-assessment, dose and Level 3 PSA
//! *methods* is in scope — the last four scope items above are exactly that.
//! Presenting the results as fit for operational use is not, per
//! `RESPONSIBLE_USE.md`. An earlier naming draft blurred the two and was
//! corrected; do not reintroduce the operational framing.
//!
//! ## Status
//!
//! **Untrusted AI-assisted draft. No human V&V.** The crate is not declared
//! mature and carries no maturity bar. What exists today is the first verified
//! slice of a FLEXPART port — see [`flexpart`] for exactly what is and is not
//! covered — and nothing here has been compared against measured atmospheric
//! dispersion data. Code-to-code agreement with FLEXPART is *verification*
//! (is it implemented as upstream specifies?), never *validation* (does it
//! represent reality well enough?).
//!
//! ## What it builds on
//!
//! - [`petir`] — the workspace's core numerics crate, for `erf` and (in later
//!   phases) interpolation, quadrature and ODE integration. FLEXPART's own
//!   `erf.f90` is deliberately not ported.
//! - `outram-mc-libs`' LCG, for the pseudo-random numbers the Langevin
//!   turbulence scheme will need. Not yet wired in — no stochastic code has
//!   landed.
//! - `outram-foam-basic-lib`, for the gridded field and interpolation layer when
//!   the concentration-grid phase arrives. Not yet a dependency: the scalar
//!   kernels ported so far need nothing from it, and adding a finite-volume CFD
//!   dependency before there is a field to put on a mesh would be premature.
//! - `boon-lay`'s nuclide database is the intended source of half-lives for
//!   [`flexpart::decay`]; this crate deliberately carries no nuclide data of its
//!   own so the two cannot drift.
//!
//! ## Licence
//!
//! GPL-3.0, containing a port of FLEXPART (GPL-3.0-or-later). See
//! `LICENSE.flexpart` and `NOTICE.flexpart` at the crate root. Independent
//! fork; not affiliated with or endorsed by NILU or the FLEXPART developers.

#![forbid(unsafe_code)]

pub mod flexpart;

/// Everything a caller normally needs, re-exported in one place.
///
/// The workspace "human interface layer" rule asks that a Rust developer be able
/// to drive a crate with rust-analyzer alone; this is the entry point for that.
pub mod prelude {
    pub use crate::flexpart::aerosol::{part0, AerosolBins};
    pub use crate::flexpart::constants;
    pub use crate::flexpart::decay::{
        decay_constant, decay_constant_exact, decayed, surviving_fraction,
    };
    pub use crate::flexpart::surface_layer::{obukhov, psih, psim, raerod, scalev, MetDataFormat};
    pub use crate::flexpart::thermo::{
        dynamic_viscosity_of_air, ew_kelvin, saturation_vapour_pressure, viscosity_kelvin,
    };
}
