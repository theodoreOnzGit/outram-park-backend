// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `GROUPR` — multigroup neutron/photon cross sections and matrices.
//!
//! Produces self-shielded multigroup cross sections, group-to-group scattering
//! and photon-production matrices, and ratio quantities (mubar, nubar, photon
//! yield) from a PENDF/URR-processed evaluation, by flux-weighted averaging over
//! a chosen group structure and weighting function. Fission is a full
//! group-to-group matrix. Output is a GENDF tape consumed by the formatter
//! modules.
//!
//! **Upstream:** `groupr.f90` (~12.7k lines, NJOY2016 commit
//! `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`). **Manual:** LA-UR-17-20093
//! §GROUPR.
//!
//! # Port status — PARTIAL (AI draft, untrusted until reviewed)
//!
//! This module is a work-in-progress port. What exists so far, all
//! self-contained and testable without a PENDF tape or flux integration:
//!
//! - [`input`] — the user-input **card deck** ([`GrouprInput`] and its selector
//!   enums) plus a free-format [`GrouprInput::parse`] reader. Mirrors `ruinb`
//!   and the driver card reads (`groupr.f90:1044-1130`, `:628`, `:993`).
//! - [`photon_groups`] — the built-in **gamma group structures** (`gengpg`,
//!   `igg` selector, `groupr.f90:4651-4863`): [`PhotonGroupStructure`] and its
//!   boundary tables in eV.
//! - [`weights`] — the built-in **analytic flux weighting functions** that
//!   `genwtf`/`getwtf` compute in closed form ([`AnalyticWeight`],
//!   `iwt = 2,3,4,6,7,11,12`; `groupr.f90:4865-5307`), plus the preserved
//!   built-in tabulated point tables (`iwt = 5,8,9`).
//!
//! The **neutron** group structures (`gengpn`, `ign` selector) are shared with
//! ERRORR and live in [`crate::errorr::groups`] — they are re-exported here as
//! [`neutron_group_structure`] and [`NeutronGroupStructure`] rather than
//! duplicated. Map a GROUPR `ign` to the enum with
//! [`input::neutron_group_from_ign`].
//!
//! # The numeric group-averaging engine — PARTIAL (vector path ported)
//!
//! The **1-D vector** flux-weighted averaging path is now ported and testable:
//!
//! - [`panel`] — the panel group-integration quadrature ([`group_integral`],
//!   [`group_average_vector`]) — the vector reduction of `panel`/`gpanel`
//!   (`groupr.f90:5858-6091`), with the [`PointwiseXs`] (`getsig`) and
//!   [`GroupFlux`] (`getflx`) feeders.
//! - [`gendf`] — the GENDF group-record writer/reader for a 1-D cross-section
//!   vector ([`GendfSection`], [`GendfTape`]), mirroring the GROUPR output LIST
//!   layout (`groupr.f90:882-946`).
//!
//! The **matrix** and self-shielding kernels remain **not** ported (they need a
//! PENDF tape, a self-shielding flux, and secondary MF4/5/6 distributions):
//!
//! - `getmf6`/`getff`/`getyld`/`getdis` — reaction feed functions and yields
//!   (the `ff(il,ig)` the vector path fixes to 1);
//! - `genflx`/`getunr`/`stounr` — the infinite-medium flux calculator and
//!   unresolved-resonance self-shielding;
//! - `cm2lab`/`f6cm`/`f6lab`/`ll2lab`/`getaed`/`gam102` — CM→lab kinematics and
//!   thermal/photon-production matrix machinery ([`kinematics::cm2lab`] is a
//!   documented `NotPorted` stub naming each missing routine + line range);
//! - the full ENDF/PENDF tape control flow that feeds real `sigma(E)` grids into
//!   [`PointwiseXs::LinLin`].
//!
//! [`run`] therefore documents the full pipeline and returns
//! [`crate::NjoyError::NotPorted`] rather than fabricating a GENDF result. See
//! `README.md` in this directory for the full theory summary and gap list.

pub mod fission_matrix;
pub mod gaminr_matrix;
pub mod gendf;
pub mod input;
pub mod kinematics;
pub mod matrix;
pub mod overlap;
pub mod panel;
pub mod pendf_feed;
pub mod photon_groups;
pub mod self_shielded;
pub mod slowing_down;
pub mod unresolved;
pub mod urr_pendf;
pub mod weights;

// --- Public surface -------------------------------------------------------

pub use gendf::{GendfGroupRecord, GendfSection, GendfTape};
pub use input::{
    neutron_group_from_ign, FluxCalcParams, GrouprInput, PrintOption, ReactionRequest,
    SmoothingOption, UnitAssignments, WeightOption, WeightSelection,
};
pub use panel::{
    group_average_vector, group_integral, GroupFlux, GroupIntegral, PointwiseXs, DEFAULT_FLUX_STEP,
    NO_NEXT_BREAK_EV,
};
pub use photon_groups::{photon_group_structure, PhotonGroupStructure};
pub use weights::{AnalyticWeight, ThermalFissionParams, BOLTZMANN_EV_PER_K};

// Self-shielded (Bondarenko-dilution) MGXS surface (op-bsz): the URR PENDF
// feeder, the slowing-down flux branch, the resolved/unresolved overlap
// correction, the per-dilution group-XS assembly, and the fission Chi / matrix.
pub use fission_matrix::{fission_group_chi, separable_fission_matrix, FissionMatrix, GroupChi};
pub use overlap::{shield_with_overlap, OverlapState};
pub use self_shielded::{self_shielded_group_xs, SelfShieldedMgxs};
pub use slowing_down::{genflx_slowing_down, SlowingDownParams};
pub use unresolved::{
    genflx_bondarenko, LssfFlag, OverlapContext, SelfShieldedFluxSet, UnresolvedTable, UnrShielded,
    UrrEnergyPoint, UrrReaction,
};
pub use pendf_feed::{classify_mtd, read_pendf_cross_section, MtdClass, PendfCrossSection};
pub use urr_pendf::{lssf_from_flag, read_urr_from_tape, read_urr_table};

// Re-export the shared neutron group structures (owned by ERRORR's `gengpn`
// port) so a GROUPR user finds them without knowing they live in ERRORR.
pub use crate::errorr::groups::{neutron_group_structure, NeutronGroupStructure};

use crate::NjoyError;

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// GROUPR needs a full input deck ([`GrouprInput`]) and a PENDF tape to run, so
/// this no-argument form exists only so the module registry can name GROUPR; it
/// reports that the end-to-end group-averaging pipeline is not yet ported. The
/// self-contained pieces that *are* ported (group structures, analytic weights,
/// card deck) are reachable directly through this module's public items.
///
/// # Pipeline (documented, from `subroutine groupr`, `groupr.f90:97-1042`)
///
/// 1. **Read input** — `ruinb` reads cards 1-5 and sets up the group structures
///    (`gengpn`/`gengpg`) and weight (`genwtf`) (`groupr.f90:1044-1130`).
///    Ported: [`GrouprInput::parse`], [`photon_group_structure`], [`weights`].
/// 2. **Locate MAT/temperature** on the ENDF and PENDF tapes; store URR cross
///    sections via `stounr` (`groupr.f90:470-540`). *Not ported.*
/// 3. **Compute the weighting flux** on `nflx` when `nsigz > 1` (`genflx`,
///    `groupr.f90:542-548`). *Not ported.*
/// 4. **For each reaction** (card 9): retrieve the data (`getmf6`/`getff`),
///    build the feed function, and integrate `feed*xsec*flux` over initial
///    energy with the `panel` quadrature (`groupr.f90:560-950`). *Not ported.*
/// 5. **Write the GENDF** section for each reaction/temperature and advance to
///    the next material (card 10) (`groupr.f90:950-1000`). *Not ported.*
///
/// Returns [`NjoyError::NotPorted`] until the numeric kernels of stages 2-5 are
/// ported.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry entry point reports `NotPorted`, never a fabricated GENDF
    /// result.
    ///
    /// **Methodology.** Call [`run`] and assert it returns
    /// [`NjoyError::NotPorted`] with the `"groupr"` tag. Guards the "no
    /// fabrication" rule (CLAUDE.md) until the averaging engine lands.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `run()` -> `NotPorted("groupr")`.
    #[test]
    fn run_is_not_ported() {
        match run() {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "groupr"),
            other => panic!("expected NotPorted(\"groupr\"), got {other:?}"),
        }
    }

    /// The re-exported neutron structure and the ported photon structure both
    /// resolve, demonstrating the neutron/photon split.
    ///
    /// **Methodology.** Fetch VITAMIN-J 175 neutron boundaries (via the
    /// re-exported [`neutron_group_structure`]) and VITAMIN-J 42 photon
    /// boundaries (via [`photon_group_structure`]); assert both are non-empty,
    /// ascending, and in eV. Confirms the module surface wires the shared
    /// neutron tables and the new photon tables together.
    ///
    /// **Result (2026-07-15).** neutron ign=17 -> 176 boundaries; photon igg=10
    /// -> 43 boundaries; both ascending.
    #[test]
    fn neutron_and_photon_surfaces_resolve() {
        let neutron = neutron_group_structure(17).expect("vitamin-j 175");
        assert_eq!(neutron.len(), 176);
        let photon = photon_group_structure(10).expect("vitamin-j 42 gamma");
        assert_eq!(photon.len(), 43);
        for grid in [&neutron, &photon] {
            assert!(grid.windows(2).all(|w| w[1] > w[0]));
        }
    }
}
