//! # PASIR RIS
//!
//! **P**robabilistic **A**ssessment of **S**afety **I**n **R**eactors:
//! **R**isk & Reliability **I**ntegrated **S**tudio.
//!
//! The reserved home for an integrated risk and reliability GUI, **mostly a
//! GUI** (maintainer, 2026-10-06): an egui studio with a thin library under
//! it. One studio
//! over the workspace's safety crates, at the top of their column in kovan's
//! code map. The question it answers is **"how do the safety pieces fit
//! together, on one screen?"**
//!
//! # STATUS: PLACEHOLDER. Nothing is implemented.
//!
//! Created 2026-10-06 by maintainer direction, to reserve the name and state
//! the scope. It has no behaviour. **Do not describe it as providing
//! anything.**
//!
//! It depends on every PSA-related crate, all three levels, by maintainer
//! direction the same day (*"pasir ris should depend on any PSA related crate
//! yeah... all 3 levels at lower-ish fidelity"*): it will drive their
//! lower-fidelity models, and the high-fidelity solvers stay where they are.
//! The edges are declared before code calls into them so kovan's code map
//! shows where the studio sits.
//!
//! # What it would drive
//!
//! ```text
//!                               PASIR RIS (GUI)
//!     ┌──────────┬───────────┬──────┴─────┬─────────────┬───────────┐
//!   RAFFLES    BISHAN     SEMBAWANG     CHANGI  ──►  BUANGKOK     REDHILL
//!   fault      in-plant   source term,  dispersion   dose         ground
//!   trees, UQ  building   offsite chain                           transport
//!   ─ Level 1 ─  ──── Level 2 ────────  ──────────── Level 3 ─────────────
//! ```
//!
//! # Naming fence
//!
//! "Probabilistic" in the backronym does not promote any of this work to
//! "Level 1/2/3 PSA". `docs/ecosystem-naming.md` records that that wording is
//! a separate, deliberate maintainer decision, and it has not been taken.
//!
//! # Intended use
//!
//! Research, education, capability building and V&V only
//! (`RESPONSIBLE_USE.md`). Risk and reliability analysis is
//! licensing-adjacent: when this crate acquires code it must not be presented
//! as supporting licensing, safety-critical decisions or emergency response.

#![no_std]
#![forbid(unsafe_code)]

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use pasir_ris::SCOPE;` fails loudly if the crate is ever repurposed without
/// updating its own documentation.
pub const SCOPE: &str = "integrated risk and reliability studio over the safety crates";

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is a placeholder. This asserts the crate builds and
    /// links, which is the only claim it is entitled to make.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("risk and reliability"));
    }
}
