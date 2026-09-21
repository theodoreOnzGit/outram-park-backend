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
//! # Why the name predates the plan — history, recorded so it is not "fixed"
//!
//! **The name REDHILL was reserved before this workspace's agentic naming
//! convention existed, and before the fork-a-mature-code pattern was the
//! default.** At the time, the motivation was to write an in-house groundwater
//! code **by hand, first** — the crate was going to be an original solver, not
//! an integration layer over somebody else's. The dependency on
//! `outram-park-fork-pflotran` described above came later and was, in the
//! maintainer's own words, more of an afterthought.
//!
//! **This is why REDHILL's intended relationship to its engine differs in
//! pattern from CHANGI's, and that asymmetry is deliberate rather than an
//! oversight:**
//!
//! | | engine lives | pattern |
//! |---|---|---|
//! | `changi` | *inside* the crate, as `changi::flexpart` | contains its port |
//! | `redhill` | *outside*, as `outram-park-fork-pflotran` | depends on a separate crate |
//!
//! Two different eras, not inconsistent reasoning. The separate-crate shape is
//! also independently justified: PFLOTRAN is a general subsurface flow and
//! reactive-transport code with uses well beyond radionuclides, so it earns its
//! own crate whether or not REDHILL ever consumes it — whereas FLEXPART's
//! scalar kernels have no consumer in this workspace outside atmospheric
//! dispersion.
//!
//! **Recorded here so that a future session does not "harmonise" the two in
//! whichever direction it happens to notice first.** Neither shape is the
//! mistake.
//!
//! ## And this is not a judgement that FLEXPART is a lesser code
//!
//! It would be easy to read "CHANGI merely contains a few FLEXPART modules,
//! REDHILL depends on a whole PFLOTRAN fork" as ranking the two upstreams. It
//! does not. FLEXPART is one of the standard Lagrangian particle dispersion
//! models and is close to the reference tool for radionuclide source
//! attribution — CTBTO-style verification work, and the Fukushima and Chernobyl
//! source reconstructions, are built on it.
//!
//! What is in `changi::flexpart` is four scalar-kernel modules, which that
//! crate's own documentation already calls *"the first verified slice of a
//! port"*. **The gap is in the port, not in the upstream.**
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
