//! Input-deck I/O and output writers (v1) — bead op-v6s.6.
//!
//! A minimal **card-based ASCII input-deck subset** (PFLOTRAN-*style* blocks)
//! plus CSV / legacy-VTK output writers for the v1 RICHARDS vertical slice.
//! HDF5 is deferred behind a future feature gate.
//!
//! # AI-designed subset — needs human review
//!
//! The deck grammar here is a small subset **invented for this crate**. It is
//! inspired by PFLOTRAN's card/block input style but is **not** the real
//! PFLOTRAN input-deck syntax and is not compatible with it. Per the workspace
//! `RESPONSIBLE_USE.md` rule, this is untrusted AI-generated draft material: a
//! human must review it against genuine PFLOTRAN input decks before any
//! fidelity claim is made. The full grammar is documented on [`parse_deck`].
//!
//! # Decoupling
//!
//! The parser returns plain, self-contained [`spec`] structs built from
//! primitives (`usize` / `f64` / `String` / `Vec`). It does **not** depend on
//! the [`crate::grid`], [`crate::properties`], or [`crate::solver`] modules —
//! the RICHARDS driver maps a parsed [`InputDeck`] onto those. The writers take
//! primitive slices, not foreign field types, so this module is fully testable
//! without a filesystem or the physics side.
//!
//! # Units
//!
//! Lengths in metres, pressures in pascals (Pa), times in seconds (s),
//! permeability in m^2, `alpha` in 1/Pa, Darcy fluxes in m/s.

mod parse;
mod spec;
mod writers;

pub use parse::parse_deck;
pub use spec::{
    BoundaryConditionSpec, BoundaryKindSpec, BoundaryLocationSpec, CurveModel, CurveSpec, GridSpec,
    InputDeck, MaterialSpec, TimeSpec,
};
pub use writers::{write_results_csv, write_vtk_structured};
