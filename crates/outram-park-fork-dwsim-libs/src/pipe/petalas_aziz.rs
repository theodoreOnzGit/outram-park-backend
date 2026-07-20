//! Petalas & Aziz (2000) mechanistic multiphase-flow model.
//!
//! ## Status: not implemented
//!
//! DWSIM's own `PetalasAziz.vb` is not portable -- it is a `DllImport`
//! wrapper around an external native `PetAz.dll` that is not present
//! anywhere in the DWSIM source tree (see this crate's `docs/port-scope.md`
//! and the workspace's `op-qo2.6` bead, P4/low priority). Implementing this
//! model here would mean an independent derivation from the primary
//! literature, not a translation of existing code.
//!
//! ## Literature review (2026-07-13)
//!
//! Primary reference, correctly cited (DWSIM's own in-code citation as
//! "SPE 71124" appears to be an error -- no such paper was found under that
//! number for this model):
//!
//! > Petalas, N., & Aziz, K. (2000). A mechanistic model for multiphase flow
//! > in pipes. *Journal of Canadian Petroleum Technology*, 39(6), 43-55.
//! > <https://doi.org/10.2118/00-06-04>
//!
//! Also indexed as a PETSOC Annual Technical Meeting paper (PETSOC-98-39,
//! 1998) that appears to be an earlier/companion presentation of the same
//! model. The full closed-form correlations (flow-pattern transition
//! criteria; stratified-flow liquid/wall and liquid/gas interfacial friction
//! factors; annular-mist entrained-liquid-fraction and interfacial friction;
//! the distribution coefficient for intermittent-flow holdup) are described
//! in secondary sources (review articles, theses) at a summary level only --
//! e.g. Petalas's own Stanford PhD dissertation is the fullest public
//! derivation referenced across the review literature found, but was not
//! independently retrieved or verified as part of this review. No source
//! consulted here reproduces the complete equation set with enough fidelity
//! to port responsibly (this crate's other correlations were ported only
//! once the exact source formulas were confirmed -- see `pipe::beggs_brill`,
//! `pipe::lockhart_martinelli`).
//!
//! At a high level (not sufficient to implement from), the model:
//! - determines the flow pattern (stratified, annular-mist, intermittent,
//!   bubble/dispersed) from mechanistic stability/transition criteria rather
//!   than an empirical map like Beggs & Brill's;
//! - is applicable across all pipe inclinations and geometries (unlike
//!   correlations developed for a narrower range);
//! - proposes its own interfacial-friction and holdup-distribution
//!   correlations per flow pattern, rather than reusing older ones.
//!
//! ## What a real implementation would need
//!
//! Direct access to Petalas & Aziz (2000) itself (or the underlying Stanford
//! PhD dissertation) to extract and verify the exact correlations, then the
//! same treatment given to this crate's other correlations: `uom`-typed
//! functions, doc comments citing the exact source equation, and unit tests.
//! Not attempted here -- flagged as P4/deferred per the user's direction
//! (2026-07-13): this is a low-priority literature-review placeholder, not
//! an implementation.

/// Not implemented -- see the module documentation above for the literature
/// review and why this was not ported. Calling this always panics; it exists
/// so the gap is a discoverable, documented item in the crate's API surface
/// rather than a silent absence.
///
/// # Panics
/// Always. This is an explicit "not implemented" marker, not a usable API.
pub fn petalas_aziz_pressure_drop() -> ! {
    unimplemented!(
        "Petalas & Aziz (2000) mechanistic multiphase-flow model is not ported -- \
         see this module's doc comment for the literature review and op-qo2.6 (P4) in beads"
    )
}
