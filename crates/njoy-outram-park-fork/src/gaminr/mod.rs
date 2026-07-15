// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `GAMINR` — multigroup photon-interaction (photoatomic) cross sections and
//! matrices.
//!
//! Produces multigroup **photoatomic** data from ENDF photon-interaction
//! evaluations: total, coherent, incoherent, pair-production, photoelectric, and
//! heating cross sections averaged over a photon group structure and weight,
//! plus Legendre group-to-group coherent/incoherent scattering matrices built
//! from atomic form factors (MF=27) and incoherent scattering functions. The
//! initial-energy quadrature is identical to GROUPR's; the secondary
//! energy-angle quadrature uses Gaussian integration (`gaminr.f90:30-36`).
//!
//! **Upstream:** `gaminr.f90` (1517 lines, NJOY2016 commit
//! `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`). **Manual:** LA-UR-17-20093
//! §GAMINR.
//!
//! # Port status — PARTIAL (AI draft, untrusted until reviewed)
//!
//! This module is a work-in-progress port. What exists so far:
//!
//! - [`input`] — the user-input **card deck** ([`GaminrInput`] and its selector
//!   enums), the standard "process all" reaction set, and deck validation.
//! - [`photon_groups`] — the built-in **photon group structures** (`genggp`,
//!   `igg` selector, [`PhotonGroupStructure`]): boundary tables in eV.
//! - [`weights`] — the built-in photon **weighting functions** (`gnwtf`/`gtflx`,
//!   [`WeightOption`] / [`PhotonWeight`]): constant and 1/E-with-roll-offs.
//! - [`gpanel`] — the **vector (`mfh = 23`) photon-interaction group-averaging
//!   engine**: the `gpanel` panel integrator and the `gtsig`/`gtflx` feeders,
//!   physically shared with GROUPR and re-exported here ([`group_average_vector`],
//!   [`PointwiseXs`], [`GroupFlux`]), plus the GENDF vector writer
//!   ([`GendfSection`], [`GendfTape`]).
//! - [`run`] / [`run_with_input`] — the top-level driver skeleton documenting
//!   the full pipeline; the **end-to-end tape driver returns
//!   [`crate::NjoyError::NotPorted`]** rather than fabricating results.
//!
//! # NOT yet ported (honest gap list)
//!
//! The photon-interaction **matrix** engine and the tape I/O control flow:
//!
//! - **`gtsig` PENDF reader** (`gaminr.f90:1133-1160`) — the `findf`/`gety1`
//!   tape retrieval of the MF=23 cross section that feeds a real `sigma(E)` grid
//!   into [`PointwiseXs::LinLin`]. The panel average of a supplied grid is
//!   ported; wiring it to a PENDF tape is not.
//! - **`gtff`** (`gaminr.f90:1162-1514`) — the matrix feed-function engine: the
//!   coherent (Rayleigh) form-factor Legendre integral (`mtd=502`), the
//!   incoherent (Compton/Klein–Nishina × S(q,Z)) energy-angle integral
//!   (`mtd=504`), and the pair-production matrix (`mtd=516`). See
//!   [`gpanel::gtff_matrix`] (a documented `NotPorted` stub).
//! - **`dspla` matrix branch** (`gaminr.f90:1083-1128`, `mfd=26`) — the
//!   group-to-group display/division; the vector branch (`mfd=23`) is ported as
//!   [`GroupIntegral::average`].
//! - The **GAM-out tape control flow** of the main `gaminr` subroutine
//!   (`gaminr.f90:133-536`) — the vector GENDF *records* round-trip, but the
//!   full ENDF tape sequencing does not.
//! - The read-in weight TAB1 (`iwt=1`) and read-in group grid (`igg=1`)
//!   *evaluation* paths beyond deck capture.
//!
//! See `README.md` in this directory for theory, the ported-vs-NotPorted table,
//! and test status.

pub mod gpanel;
pub mod input;
pub mod photon_groups;
pub mod weights;

pub use gpanel::{
    group_average_vector, group_integral, gtff_matrix, GendfGroupRecord, GendfSection, GendfTape,
    GroupFlux, GroupIntegral, PointwiseXs,
};
pub use input::{
    standard_reactions, EndfFormat, GaminrInput, GaminrReaction, GroupGrid, PrintOption,
    ReactionSelection, UnitAssignments,
};
pub use photon_groups::{photon_group_structure, PhotonGroupStructure};
pub use weights::{PhotonWeight, WeightOption, WeightTab1};

use crate::NjoyError;

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// GAMINR needs a full input deck ([`GaminrInput`]) to run, so this
/// no-argument form exists only so the module registry can name GAMINR; it
/// reports that the end-to-end pipeline is not yet ported. For the real entry
/// point (once the numeric kernels land) use [`run_with_input`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("gaminr"))
}

/// Top-level GAMINR driver skeleton for a validated input deck.
///
/// This documents and partially executes GAMINR's pipeline
/// (`gaminr.f90:133-536`):
///
/// 1. **Read user input** (`ruing`, `gaminr.f90:538-583`) — captured in
///    [`GaminrInput`]; validated here via [`GaminrInput::validate`]. *(ported)*
/// 2. **Build the photon group structure** (`genggp`, `gaminr.f90:585-776`) —
///    [`PhotonGroupStructure::boundaries`]. *(ported for built-in `igg`)*
/// 3. **Build the weight function** (`gnwtf`, `gaminr.f90:778-823`) —
///    [`PhotonWeight::from_option`]. *(ported for `iwt=2,3`)*
/// 4. **Determine the ENDF format** from the tape header (`gaminr.f90:153-164`)
///    and the reaction list ([`standard_reactions`] when `mfd=-1`). *(deck-level
///    only; tape reading NotPorted)*
/// 5. **Group-average each reaction** (`gtsig`/`gtff`/`gpanel`/`dspla`) and
///    **write the GAM-out tape**. **NotPorted** — see the module gap list.
///
/// Because step 5 is not ported, this returns [`crate::NjoyError::NotPorted`]
/// after performing and validating steps 1–3, so a caller can already exercise
/// the deck / group-structure / weight construction without a fabricated
/// numeric result.
///
/// # Errors
///
/// - Propagates deck-validation errors from [`GaminrInput::validate`].
/// - Propagates group-structure / weight construction errors (e.g. the
///   read-in `igg=1` / `iwt=1` paths that are themselves NotPorted).
/// - Always returns [`crate::NjoyError::NotPorted`] for the numeric engine even
///   when steps 1–3 succeed.
pub fn run_with_input(deck: &GaminrInput) -> Result<(), NjoyError> {
    // Step 1: validate the captured deck (ruing constraints).
    deck.validate()?;

    // Step 2: build the photon group structure (genggp). For a read-in grid
    // (igg=1) use the deck-supplied boundaries; otherwise the built-in table.
    let _egg: Vec<f64> = match &deck.group_grid {
        Some(grid) => grid.boundaries_ev.clone(),
        None => deck.group_structure.boundaries()?,
    };

    // Step 3: build the weight function (gnwtf).
    let _weight = PhotonWeight::from_option(deck.weight)?;

    // Steps 4-5: ENDF-format detection, reaction group-averaging, and GENDF
    // output are not yet ported.
    Err(NjoyError::NotPorted(
        "gaminr numeric engine (gtsig/gtff/gpanel/dspla + GENDF writer)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the registry entry point reports GAMINR is not end-to-end
    /// ported (`gaminr.f90` numeric engine absent).
    /// Result (2026-07-15): `run()` returns `NotPorted("gaminr")`.
    #[test]
    fn run_reports_not_ported() {
        assert!(matches!(run(), Err(NjoyError::NotPorted("gaminr"))));
    }

    /// Methodology: `run_with_input` executes the ported steps 1-3 (validate
    /// deck, build group structure, build weight) then reports the numeric
    /// engine NotPorted. A well-formed deck must reach that final NotPorted
    /// (not fail earlier); a malformed deck must fail validation first.
    ///
    /// Result (2026-07-15): valid VITAMIN-J / constant-weight deck reaches the
    /// engine NotPorted; an igg==1 deck without a grid fails validation.
    #[test]
    fn run_with_input_reaches_engine_gap() {
        let deck = GaminrInput {
            units: UnitAssignments {
                nendf: 20,
                npend: 21,
                ngam1: 0,
                ngam2: 22,
            },
            matb: 600,
            group_structure: PhotonGroupStructure::VitaminJ42,
            weight: WeightOption::Constant,
            lord: 3,
            iprint: PrintOption::Maximum,
            title: "carbon photoatomic".into(),
            group_grid: None,
            reactions: ReactionSelection::ProcessAll,
            next_matd: 0,
        };
        let err = run_with_input(&deck).unwrap_err();
        assert!(
            matches!(err, NjoyError::NotPorted(m) if m.contains("numeric engine")),
            "expected numeric-engine NotPorted, got {err:?}"
        );

        // Malformed deck: igg==1 requires a grid -> fails validation (step 1).
        let mut bad = deck.clone();
        bad.group_structure = PhotonGroupStructure::ArbitraryRead;
        assert!(matches!(run_with_input(&bad), Err(NjoyError::EndfParse(_))));
    }
}
