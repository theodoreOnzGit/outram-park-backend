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
//! # STATUS (2026-09-28)
//!
//! ~~PLACEHOLDER. Nothing is implemented.~~ **CHANGED 2026-09-28:** the crate
//! holds the published HTR-10 dose tables (see below). A port of pyDOSEIA
//! (Sadhu et al., *Health Physics* 130(1) (2026) 94-110,
//! doi:10.1097/HP.0000000000002014) is in progress and lands in a following
//! commit. Nothing here computes a dose yet.
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
//! ~~Two published HTR-10 dose-versus-distance tables are stored in `changi`,
//! parked there by the maintainer … do not move them unasked.~~ **MOVED
//! 2026-09-28** (maintainer: "move table 7 and 9 to buangkok"): the published
//! HTR-10 dose tables now live here, in [`published`] — normal operation
//! (Liu and Cao 2002, Table 7) and two design-basis accidents (Table 9).
//! They are stored reference data; nothing in this crate computes a dose
//! from them.
#![forbid(unsafe_code)]

/// Published dose tables (Liu and Cao 2002, Tables 7 and 9), stored as cited
/// reference data. See the module docs.
pub mod published;

/// The scope this crate reserves, as a machine-readable string.
///
/// Created when the crate was a placeholder (2026-09-28) so it had something
/// testable. It is kept so that a downstream `use buangkok::SCOPE;` fails
/// loudly if the crate is ever repurposed without updating its own
/// documentation.
pub const SCOPE: &str = "radiation dose and bioeffects for research-grade safety analysis";

#[cfg(test)]
mod tests {
    use super::*;

    /// The scope string still names dose. (Written for the placeholder;
    /// the pyDOSEIA port carries its own tests.)
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("dose"));
    }
}
