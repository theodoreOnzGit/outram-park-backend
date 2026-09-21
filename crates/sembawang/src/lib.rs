//! # SEMBAWANG
//!
//! **S**evere-accident **E**volution and **M**elt **B**ehaviour **A**nalysis
//! **W**orkbench for **A**dvanced **N**uclear **G**eometries.
//!
//! **Severe accident and source term, and orchestrator of the offsite chain**
//! (maintainer decision 2026-09-21, GitHub #235; `docs/ecosystem-naming.md`
//! decision 7).
//!
//! - **Its own physics:** the fission-product source term (implemented, on
//!   `boon-lay`'s TRISO-ATOPS fork) and severe-accident progression: melt
//!   behaviour, relocation, vessel failure, molten-core–concrete interaction,
//!   hydrogen and aerosol release (**not implemented**).
//! - **What it orchestrates:** [`changi`] for dispersion and deposition (wired
//!   in, see [`chain`]), `redhill` for ground transport (a placeholder, not
//!   wired), and `raffles` for uncertainty propagation (GitHub #238, not yet
//!   a dependency).
//!
//! The chain ends at activity released, air concentration and deposition.
//! **No dose quantity is computed**, and none of this is described as PSA.
//!
//! # STATUS: partially implemented, and the unimplemented part is the larger one
//!
//! ~~**PLACEHOLDER. Nothing is implemented.** Created 2026-09-18 to hold a
//! reserved name and a scope statement, not code. Do not cite it as the
//! location of a source-term calculation until something has actually been
//! written here.~~ **SUPERSEDED 2026-09-21** — the text is kept rather than
//! deleted because it was correct when written and because what replaced it is
//! narrower than the crate's name suggests. Read the next two paragraphs
//! before citing this crate for anything.
//!
//! **What exists: the fission-product release path for TRISO fuel**, built on
//! `boon-lay`'s TRISO-ATOPS fork, producing a
//! [`changi::activity::source::SourceTerm`]. That is a *mechanistic release
//! from intact and defective particles under a prescribed temperature
//! transient* — diffusion through the kernel and coating layers, and
//! breakthrough of the SiC.
//!
//! **What does not exist: severe-accident progression**, which is the rest of
//! the crate's name and the larger half. There is no melt behaviour, no
//! relocation, no vessel failure, no molten-core-concrete interaction, no
//! hydrogen, and no aerosol physics. The temperature transient this crate
//! consumes is **prescribed by the caller**; nothing here computes it.
//!
//! So: cite this crate for a TRISO release calculation under a temperature
//! history you supplied, and (since 2026-09-21) for carrying that release
//! through `changi` to air concentration and deposition. The line below is the
//! original, kept because it is still true as far as it goes:
//!
//! > cite this crate for a TRISO release calculation under a temperature
//! > history you supplied. Do **not** cite it for accident progression.
//!
//! The name was already reserved in `docs/ecosystem-naming.md` and
//! `GOVERNANCE.md`; this crate makes the reservation visible in the workspace
//! rather than only in prose.
//!
//! # Where it sits: the offsite chain
//!
//! ```text
//!              ┌──────────── SEMBAWANG orchestrates ────────────┐
//!   boon-lay ──► source term ──► CHANGI ──► REDHILL             │
//!   (TRISO       (what gets      (air, Bq·s/m³;  (ground         │
//!    release)     released?)      ground, Bq/m²)  transport)     │
//!              └── RAFFLES: uncertainty propagation (#238) ─────┘
//! ```
//!
//! ~~[`changi`] exists and has a FLEXPART v10.4 port under way; [`redhill`] is,
//! like this crate, a placeholder. **CHANGI currently has no upstream**: its
//! own README records that until SEMBAWANG exists, a release-rate time series
//! has to be supplied by hand.~~ **CORRECTED 2026-09-21**: this crate is no
//! longer a placeholder, and it feeds [`changi`] directly through
//! [`chain::pad_for_dispersion`] (see `examples/npmhtgr_chain.rs`). `redhill` is
//! still a placeholder.
//!
//! # Honest scope of the gap
//!
//! This is the **largest** of the reserved names by some margin, and that
//! should be stated plainly rather than discovered later. [`redhill`] can
//! build on `outram-park-fork-pflotran`, which already exists in this
//! workspace, so it is substantially an integration layer.
//!
//! ~~SEMBAWANG has **no engine at all**.~~ **CORRECTED 2026-09-21** — it has
//! one for *release*: `boon-lay`'s TRISO-ATOPS fork, which is ported,
//! `uom`-typed and code-to-code verified. That is what made the source-term
//! half tractable.
//!
//! It still has **no engine for progression**. Severe-accident progression
//! would need a MELCOR-class port, which is scoped in `docs/melcor-scoping.md`
//! and has not been begun. Treat that scoping document as a statement of
//! intent, not of progress.
//!
//! # What this crate does NOT compute, listed so it is not assumed
//!
//! - **The temperature transient.** Prescribed by the caller. An
//!   `outram-park-digital-twin-engine` trace can be dropped in, but nothing
//!   here solves for one.
//! - **The core inventory.** Prescribed. In particular, do **not** try to build
//!   one from `fission-yields-data`: it exposes *independent* fission yields,
//!   and Cs-137's independent yield is roughly two orders of magnitude below
//!   its cumulative yield because Cs-137 arrives down the A = 137 isobaric
//!   chain. Naive use gives a silently ~100x low inventory for most of the
//!   species that matter. Cumulative yields need a decay-chain walk.
//! - **Parent to daughter chaining during the accident**, and so no daughter
//!   ingrowth.
//! - **Containment transport, pool scrubbing or iodine chemistry.** What leaves
//!   the fuel is treated as what leaves the building.
//! - **Any dose quantity**, here or in [`changi`]. The chain stops at activity
//!   in air and on the ground.
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
//! [`redhill`]: https://docs.rs/redhill

// `std` is required: `boon-lay` is a `std` crate, so `no_std` could not survive
// the dependency even if this crate wanted it. The previous `#![no_std]` was
// removed on 2026-09-21 for that reason and not as a preference.
#![forbid(unsafe_code)]

pub mod accident;
pub mod chain;
pub mod error;
pub mod inventory;
pub mod scenario;
pub mod units;

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use sembawang::SCOPE;` fails loudly if the crate is ever repurposed
/// without updating its own documentation.
pub const SCOPE: &str = "severe accident progression; source term";

pub use error::{Error, Result};

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
