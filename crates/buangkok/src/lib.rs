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
//! # STATUS: pyDOSEIA ported (2026-09-28)
//!
//! ~~PLACEHOLDER. Nothing is implemented.~~ **CHANGED 2026-09-28** (maintainer:
//! "work on translating pyDOSEIA into buangkok under a module", then "port all
//! of pyDOSEIA into buangkok"). The crate holds [`pydoseia`], a faithful port
//! of the MIT-licensed pyDOSEIA code (Sadhu et al., *Health Physics* 130(1)
//! (2026) 94-110, doi:10.1097/HP.0000000000002014): met processing,
//! Gaussian-plume dilution factors, the inhalation, ground-shine, submersion,
//! ingestion and plume-shine pathways, multi-source DCF screening, plume rise,
//! the run configuration and the driver with its summary tables. ~~Ingestion
//! and plume shine are **not ported**~~ (**ported 2026-09-28**, second
//! tranche). Everything is **code-to-code verified against upstream** on
//! synthetic inputs (`tests/pydoseia_code_to_code.rs`: 1 899 cases, 41 of 43
//! groups bit-exact). The agreement is with pyDOSEIA, not with experiment,
//! and there is no validation of any kind; 26 upstream defects are recorded
//! (`docs/pydoseia-code-to-code.md`). What is not ported is I/O and UI (see
//! `docs/pydoseia-port-scoping.md`). No dose-coefficient data ships with the
//! crate. Human V&V review is still outstanding (see README).
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

/// Port of pyDOSEIA (met processing, Gaussian-plume dilution, inhalation,
/// ground-shine, submersion, ingestion and plume-shine doses, the driver),
/// code-to-code verified against upstream. See the module docs for
/// provenance and scope.
pub mod pydoseia;

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
