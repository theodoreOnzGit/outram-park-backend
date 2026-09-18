//! # SEMBAWANG
//!
//! **Severe accident progression** — melt behaviour, relocation, vessel
//! failure, molten-core–concrete interaction, hydrogen and aerosol release.
//!
//! The question this crate answers is **"what gets released?"**, and its
//! output is the **source term** that [`changi`] takes as input.
//!
//! # STATUS: PLACEHOLDER. Nothing is implemented.
//!
//! Created 2026-09-18 to hold a reserved name and a scope statement, not code.
//! It has no dependencies and no behaviour. **Do not describe it as providing
//! anything**, and do not cite it as the location of a source-term calculation
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
//! [`changi`] exists and has a FLEXPART v10.4 port under way; [`redhill`] is,
//! like this crate, a placeholder. **CHANGI currently has no upstream**: its
//! own README records that until SEMBAWANG exists, a release-rate time series
//! has to be supplied by hand.
//!
//! # Honest scope of the gap
//!
//! This is the **largest** of the reserved names by some margin, and that
//! should be stated plainly rather than discovered later. [`redhill`] can
//! build on `outram-park-fork-pflotran`, which already exists in this
//! workspace, so it is substantially an integration layer. SEMBAWANG has **no
//! engine at all** — severe-accident progression would need a MELCOR-class
//! port, which is scoped in `docs/melcor-scoping.md` and has not been begun.
//!
//! Treat the scoping document as a statement of intent, not of progress.
//!
//! # Intended use, and what it will never be for
//!
//! Research, education, capability building and V&V only, like the rest of
//! this workspace (`RESPONSIBLE_USE.md`). Severe-accident and source-term
//! analysis is precisely the area where an unvalidated tool is most dangerous,
//! so when this crate does acquire code it must **not** be presented as
//! supporting emergency response, emergency planning, licensing, or
//! safety-critical decisions — the same boundary `changi` already carries.
//!
//! [`changi`]: https://docs.rs/changi
//! [`redhill`]: https://docs.rs/redhill

#![no_std]
#![forbid(unsafe_code)]

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use sembawang::SCOPE;` fails loudly if the crate is ever repurposed
/// without updating its own documentation.
pub const SCOPE: &str = "severe accident progression; source term";

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is a placeholder. This asserts the crate builds and
    /// links, which is the only claim it is entitled to make.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("source term"));
    }
}
