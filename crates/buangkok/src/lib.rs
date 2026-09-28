//! # BUANGKOK
//!
//! **B**ioeffects, **U**ncertainty and **A**LARA for **N**uclear
//! **G**uidance, **K**eeping **O**perational **K**nowledge.
//!
//! The reserved home for **radiation dose and its biological effects**, for
//! research-grade safety analysis: turning air concentrations and ground
//! deposition into dose (cloudshine, groundshine, inhalation, ingestion),
//! dose coefficients, dose uncertainty, and ALARA reasoning. The question it
//! answers is **"what dose follows from what was released and where it
//! went?"**
//!
//! # STATUS: PLACEHOLDER. Nothing is implemented.
//!
//! Created 2026-09-28 by maintainer direction ("buangkok for dose") to
//! reserve the name and state the scope, not to hold code. It has no
//! dependencies and no behaviour. **Do not describe it as providing
//! anything**, and do not cite it as the location of any dose calculation.
//!
//! # Why dose has its own crate
//!
//! Dose is a biological quantity. Keeping it apart from the dispersion
//! physics (CHANGI) and the source term (SEMBAWANG, BISHAN) means neither of
//! those is mistaken for a health-assessment capability, and it keeps the
//! "no dose" boundary those crates already state easy to hold. The workspace
//! uses dose for **safety analysis in the research sense only**: never for
//! medical, occupational-exposure, public-health, emergency-response,
//! licensing or regulatory decisions (`RESPONSIBLE_USE.md`).
//!
//! # Where it would sit
//!
//! ```text
//!   source term ──► CHANGI ──────────────► BUANGKOK
//!   (SEMBAWANG,     air concentration,      dose, pathways,
//!    BISHAN)        deposition              uncertainty (+ RAFFLES)
//! ```
//!
//! # Already in the workspace, not yet here
//!
//! Two published HTR-10 dose-versus-distance tables are stored in `changi`,
//! parked there by the maintainer: normal operation (Liu and Cao 2002,
//! Table 7, `changi::activity::published_dose_by_distance`) and two
//! design-basis accidents (Table 9,
//! `changi::activity::published_accident_dose_by_distance`, added
//! 2026-09-28). Whether and when they move here is the maintainer's call; do
//! not move them unasked.
#![forbid(unsafe_code)]

/// The scope this crate reserves, as a machine-readable string.
///
/// Exists so the placeholder has *something* testable and so a downstream
/// `use buangkok::SCOPE;` fails loudly if the crate is ever repurposed without
/// updating its own documentation.
pub const SCOPE: &str = "radiation dose and bioeffects for research-grade safety analysis";

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is a placeholder. This asserts the crate builds and
    /// links, which is the only claim it is entitled to make.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("dose"));
    }
}
