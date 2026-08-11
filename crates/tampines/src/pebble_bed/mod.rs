//! # Pebble-bed thermal physics for high-temperature gas-cooled reactors
//!
//! The nested conduction scales of a pebble-bed core (a *doubly
//! heterogeneous* medium — TRISO particles inside pebbles inside a packed
//! bed), being built as one coherent stack under the `op-jyyp` HTR-10 epic.
//!
//! ## Present
//!
//! - [`zbs`] — Zehner-Bauer-Schlunder packed-bed effective thermal
//!   conductivity: stagnant-gas, solid, particle-contact and thermal
//!   radiation contributions, with a near-wall porosity hook. The
//!   formulation is verified against the open Pronghorn Theory Manual
//!   (INL/EXT-18-44453-Rev001) equation set; see the module docs for the
//!   documented finding that the VTB generic-pbr 18-point reference table
//!   is *not* reproducible by ZBS with helium in the pores.
//!
//! ## Planned (tracked in beads, not yet present)
//!
//! - `triso` — coated-particle layer stack conduction (kernel, buffer,
//!   IPyC, SiC, OPyC) with fluence-dependent layer conductivities from the
//!   VTB HTR-PM pebble model; geometry reuse from `boon-lay`'s `TrisoCell`
//!   (maintainer-approved dependency edge, already declared in
//!   `Cargo.toml`).
//! - `pebble` — two-zone pebble radial conduction (fuelled zone with
//!   Chiew-Glandt TRISO dispersion + unfuelled graphite shell).
//! - `cht` — Wakao particle-to-fluid Nusselt coupling. **Warning recorded
//!   in beads:** the TUAS `WakaoData` implementation has the Re and Pr
//!   exponents swapped relative to the published correlation — do not
//!   cross-wire it until that bead is resolved.
//! - `kta` — KTA packed-bed pressure drop (friction side of the bed).
//! - `feedback` — separate graphite/moderator reactivity channel.
//!
//! ## Status
//!
//! **NOT VALIDATED.** Every correlation carries its citation and access
//! tier; nothing here has been compared against HTR-10 measurements.
//! AI-assisted draft pending human review per `RESPONSIBLE_USE.md`.

pub mod zbs;

pub use zbs::ZbsBed;
