//! # BISHAN
//!
//! **B**uilding **I**nternal **S**ource-term and **H**azard **A**nalysis
//! **N**etwork.
//!
//! Level 2 PSA: severe-accident progression inside the plant, containment and
//! reactor-building response, in-building aerosol transport and pool
//! scrubbing, and release categories. The question it answers is **"what
//! leaves the building, how, and how often?"**
//!
//! # STATUS: one component implemented; the rest of the scope is not
//!
//! ~~PLACEHOLDER. Nothing is implemented.~~ **CHANGED 2026-09-29 (gh:#400,
//! maintainer's source-term plan):** [`building`] is a lumped HTR-10
//! reactor-building (vented confinement) control volume -- inflow from the
//! primary circuit, decay, deposition, filtered exhaust to the stack -- from
//! published figures (Jiang et al. 2002; Liu & Cao 2002). Everything else in
//! the scope below (severe-accident progression, aerosol transport, pool
//! scrubbing, release categories, RAFFLES) is **not implemented**; do not cite
//! this crate for it.
//!
//! Created 2026-09-28 by maintainer direction to reserve the name and state
//! the scope.
//!
//! The name and backronym come from the workspace roadmap slides
//! (`slides/outram-park.tex`, 2026-09-14): "Level 2 PSA. Severe-accident
//! progression, containment response, in-building aerosol transport and pool
//! scrubbing, release categories. Depends on RAFFLES for the probabilistic
//! machinery."
//!
//! # Where it would sit
//!
//! ```text
//!   plant transient ──► fuel release ──► BISHAN ──────────────► CHANGI ──► REDHILL
//!   (htgr_sim_v1)       (boon-lay)       building, containment,   dispersion  ground
//!                                        filters, release          deposition  transport
//!                                        categories (+ RAFFLES)
//! ```
//!
//! It fills the gap the 2026-09-28 audit of the `htgr_sim_v1` accident chain
//! found between the primary circuit and the environment: today nothing
//! models building retention, filtration, depressurisation release paths or
//! release timing (see GitHub #226).
//!
//! # Open question for the maintainer: the boundary with SEMBAWANG
//!
//! `docs/ecosystem-naming.md` gives SEMBAWANG "severe accident and source
//! term, and orchestrator of the offsite chain", including severe-accident
//! progression (melt, relocation, vessel failure, MCCI, hydrogen, aerosols).
//! The roadmap gives BISHAN severe-accident progression too. Which crate owns
//! in-plant progression, and whether BISHAN is the in-building half of
//! SEMBAWANG's scope, **has not been decided**. Settle it before either crate
//! grows code in that area.
//!
//! # Not dose
//!
//! BISHAN ends at what is released: activity, timing, release category. Dose
//! is out of its scope, as it is for SEMBAWANG and (today) CHANGI.
#![forbid(unsafe_code)]

pub mod building;

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use bishan::SCOPE;` fails loudly if the crate is ever repurposed without
/// updating its own documentation.
pub const SCOPE: &str = "Level 2 PSA: in-building source term and release categories";

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is a placeholder. This asserts the crate builds and
    /// links, which is the only claim it is entitled to make.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("Level 2 PSA"));
    }
}
