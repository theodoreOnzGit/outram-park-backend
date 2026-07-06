//! # NEE_SOON
//!
//! **N**eutron **E**nergy-dependent **S**imulation using **O**pen-source
//! **O**bject-**O**riented **N**umerics.
//!
//! NEE_SOON is the **coupling / integration layer** of the OUTRAM PARK suite.
//! It does not implement transport, nuclear-data processing, or kinetics
//! itself — those live in dedicated crates. Instead it composes them behind a
//! single, human-navigable object-oriented API so that a user can assemble the
//! simulation pieces they want without wiring the crates together by hand.
//!
//! ## What it composes
//!
//! | Piece | Provided by | Role |
//! |---|---|---|
//! | Nuclear data / cross sections | [`njoy_outram_park_fork`] | energy-dependent σ(E), ν̄, χ, WMP |
//! | Monte Carlo transport | [`openmc_libs`] | CSG geometry, k-eigenvalue, Woodcock tracking |
//! | Point reactor kinetics | [`teh_o_prke`] | PRKE precursor/reactivity time response |
//!
//! ## Entry point
//!
//! The whole crate is reached through **one struct**, [`NeeSoon`]. It is the
//! object-oriented facade: the user constructs a `NeeSoon`, then asks it to
//! create the relevant simulation pieces (a data provider, a transport model, a
//! kinetics model, a coupled run) rather than importing each underlying crate
//! directly. This keeps the mental context load low — one type to learn, with
//! `rust-analyzer` autocompletion revealing the available pieces.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** orchestration, the object-oriented facade, cross-crate
//!   glue types, ergonomic constructors, coupling schedules, and any *new*
//!   user-facing functionality that only makes sense once the pieces are joined.
//! - **Does NOT belong here:** raw physics kernels. New cross-section code goes
//!   to `njoy-outram-park-fork`; new transport code to `openmc-libs`; new
//!   kinetics to `teh-o-prke`. NEE_SOON only *exposes and integrates* them.
//!
//! ## Status
//!
//! **Scaffold only.** The public surface below is a documented skeleton; the
//! coupling logic is not implemented yet.

#![forbid(unsafe_code)]

/// Object-oriented facade for the OUTRAM PARK neutronics + kinetics suite.
///
/// `NeeSoon` is the single entry point of the crate (the "one big struct"): a
/// user constructs one of these and then creates the relevant simulation pieces
/// through it — a nuclear-data provider ([`njoy_outram_park_fork`]), a Monte
/// Carlo transport model ([`openmc_libs`]), a point-kinetics model
/// ([`teh_o_prke`]), and, ultimately, coupled runs that thread data between
/// them.
///
/// # Physical scope
///
/// This type owns no physics of its own; it is a builder/orchestrator over the
/// composed crates. Physical quantities exchanged across its API are dimensioned
/// via [`uom`] (never bare `f64`).
///
/// # Status
///
/// Scaffold only — fields and methods are intentionally omitted. The planned
/// shape is a builder that holds:
/// - a nuclear-data provider handle (cross-section source),
/// - an optional transport model,
/// - an optional kinetics model,
/// - coupling / orchestration configuration.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct NeeSoon {}
