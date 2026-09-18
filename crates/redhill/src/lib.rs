//! # REDHILL
//!
//! **R**adionuclide **E**ffluent **D**ispersion solver for **H**ydrogeological
//! **I**nfiltration and **L**eaching through **L**ayers.
//!
//! Groundwater transport, geological migration, subsurface radionuclide
//! transport, porous-media flow and long-term repository assessment.
//!
//! The question this crate answers is **"what happens after deposition and
//! infiltration?"** — the far end of the offsite chain.
//!
//! # STATUS: PLACEHOLDER. Nothing is implemented.
//!
//! Created 2026-09-18 to hold a reserved name and a scope statement, not code.
//! It has no dependencies and no behaviour. **Do not describe it as providing
//! anything**, and do not cite it as the location of a transport calculation
//! until something has actually been written here.
//!
//! The name was already reserved in `docs/ecosystem-naming.md` and
//! `GOVERNANCE.md`; this crate makes the reservation visible in the workspace
//! rather than only in prose.
//!
//! # Where it sits: the offsite chain
//!
//! ```text
//!   SEMBAWANG  ──►  CHANGI  ──►  REDHILL
//!   what gets       what happens    what happens after
//!   released?       after release?  deposition + infiltration?
//! ```
//!
//! # The engine already exists, and that is the difference
//!
//! Unlike [`sembawang`], REDHILL is **not** starting from nothing:
//! `outram-park-fork-pflotran` is already a workspace member — a pure-Rust
//! fork of PFLOTRAN for subsurface flow and reactive transport, enum-dispatched
//! and `uom`-typed, with no PETSc/FFI/MPI. So this crate is expected to be
//! substantially an **integration layer** over that, rather than a new solver.
//!
//! **That dependency is deliberately NOT declared yet.** Declaring it now would
//! assert an architecture nothing here has earned, and an unused dependency is
//! not free — it enters the lock file and every downstream build graph. Add the
//! edge when code here actually calls into it.
//!
//! Note also that `outram-park-fork-pflotran` is itself recorded as a scaffold
//! with no human V&V, so "the engine exists" means the port exists, not that it
//! is validated.
//!
//! # Intended use
//!
//! Research, education, capability building and V&V only
//! (`RESPONSIBLE_USE.md`). Repository assessment and long-term environmental
//! transport are licensing-adjacent activities; when this crate acquires code
//! it must not be presented as supporting licensing or safety-critical
//! decisions.
//!
//! [`sembawang`]: https://docs.rs/sembawang

#![no_std]
#![forbid(unsafe_code)]

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use redhill::SCOPE;` fails loudly if the crate is ever repurposed without
/// updating its own documentation.
pub const SCOPE: &str = "groundwater and geological radionuclide transport";

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is a placeholder. This asserts the crate builds and
    /// links, which is the only claim it is entitled to make.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("transport"));
    }
}
