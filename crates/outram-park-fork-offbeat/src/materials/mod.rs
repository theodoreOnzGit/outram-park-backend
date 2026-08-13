// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/materials/`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Material property correlations for nuclear fuel, cladding and structure.
//!
//! # What belongs in this module
//!
//! Correlations that answer "what is property P of material M in state S?" —
//! thermal conductivity, heat capacity, density, emissivity, Young's modulus,
//! Poisson's ratio, thermal expansion ([`properties`]), and the behavioural
//! models for how the material's volume and integrity evolve under irradiation
//! — swelling, densification, relocation, phase transition, failure
//! ([`behavioral`]).
//!
//! # What does not belong here
//!
//! Anything that solves a field equation. Constitutive laws that integrate
//! stress from strain live in [`crate::rheology`]; the momentum solve lives in
//! [`crate::mechanics`]. A material model here is a *pure function* of
//! [`MaterialState`] — it has no memory, no mesh and no timestep.
//!
//! # The shape every property family takes
//!
//! Each family is an **enum** whose variants are the published correlations for
//! it, dispatched by `match` rather than by a trait object. The set of
//! correlations is closed and known at compile time, so an enum gives
//! exhaustiveness (adding a correlation forces every call site to consider it)
//! and lets rust-analyzer's go-to-definition work on the variants, which it
//! does not on `dyn Trait`. See the workspace `CLAUDE.md` "No trait objects"
//! rule.
//!
//! Every variant is named for the **author or data source of the fit** and the
//! material it applies to — `MatproUo2`, `RelapZircaloy`, `SneadSiC` — because
//! that is how the fuel-performance literature identifies a correlation, and
//! because two correlations for "UO2 conductivity" can differ by 20% and a
//! reader must be able to tell which one is in use.
//!
//! # Validity ranges are enforced, not assumed
//!
//! Each correlation is a fit over a stated range of temperature, burnup or
//! composition. Evaluated outside it, a fit returns a number that looks
//! reasonable and means nothing. The `*_checked` methods return
//! [`OutOfRange`](crate::error::OffbeatError::OutOfRange) instead; the plain
//! methods clamp to the range endpoints, matching upstream behaviour, and say
//! so in their doc comments.

pub mod behavioral;
pub mod properties;
pub mod state;

pub use state::MaterialState;
