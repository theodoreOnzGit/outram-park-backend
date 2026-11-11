//! MELCOR-class severe-accident equations, written clean-room from the
//! public MELCOR manuals and the open literature (gh:#1335).
//!
//! **Clean room (HARD RULE, `crates/sembawang/AGENTS.md`):** every model
//! here is built from public manuals (MELCOR 2.2 Vol. 1 SAND2017-0455 O,
//! Vol. 2 SAND2023-10994 O, SPARC-90) and the open literature only, cited
//! by section and equation in each module. Nobody working on this module
//! may read MELCOR, MAAP, ASTEC, RELAP5, SCDAP or FRAPCON source, "including
//! just to check an equation" (`docs/melcor-scoping.md` §1, §8). This is
//! not a port: there is no upstream source and no attribution header beyond
//! the citations; see this crate's `NOTICE`.
//!
//! **Moved here from `sembawang::bwr_mark1` on 2026-11-11 (gh:#1335,
//! maintainer direction 2026-11-10: "the MELCOR equations clean room should
//! live as an offbeat module rather than in sembawang").** The code moved
//! unchanged (`git mv`); the Fukushima Daiichi plant assembly that calls it
//! stays in `sembawang::bwr_mark1`, whose old paths
//! (`sembawang::bwr_mark1::aerosol` and the rest) are deprecated
//! re-exports of these modules for one release. The BSAF comparison was
//! re-run before and after the move and is bit-identical.
//!
//! What moved, and why the rest of `bwr_mark1` did not, is recorded on
//! gh:#1335. In short, a module stays in `sembawang` when it needs a crate
//! this one must not depend on (`tampines`, `teh-o-prke`, the ENDF decay
//! libraries, `tuas_boussinesq_solver`) or when it is plant data:
//!
//! | module | content |
//! |---|---|
//! | [`aerosol`] | sectional aerosol: MAEROS kernels, deposition (RN §2.4) |
//! | [`vapour`] | fission-product vapour pressures and condensation (RN §2.5) |
//! | [`rn_volume`] | one control volume's aerosol and vapour classes, stepped in MELCOR's order |
//! | [`iodine`] | SPARC-90 pool iodine equilibrium and partition limit (RN §2.7.1, App. D, F) |
//! | [`wire`] | the exact-`f64` wire format the volumes cross worker boundaries in |
//! | [`params`] | the generic physical constants these modules read |
//!
//! > Research, education and V&V only. AI draft, unreviewed, not
//! > validated: never for operation, licensing, safety decisions or
//! > emergency response (`RESPONSIBLE_USE.md`).

pub mod aerosol;
pub mod iodine;
pub mod params;
pub mod rn_volume;
pub mod vapour;
pub mod wire;
