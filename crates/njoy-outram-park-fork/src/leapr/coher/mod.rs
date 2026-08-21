//! Coherent-elastic (Bragg) scattering — MF=7/MT=2, `LTHR=1`.
//!
//! Two paths produce the same [`BraggEdges`], so everything downstream
//! ([`crate::leapr::endout`]) is indifferent to which was used:
//!
//! | Path | Module | Source | Covers |
//! |---|---|---|---|
//! | **Built-in lattices** | [`builtin`] | port of NJOY2016 `leapr.f90::coher`/`formf`/`tausq` | the six lattices stock LEAPR hardcodes: graphite, Be, BeO, Al, Pb, Fe (card 5 `iel = 1..6`) |
//! | **Generalized** | [`general`] | Zhu (2014) Eqs. 3.4-3.8, new synthesis | any crystal, from its lattice vectors and atomic basis |
//!
//! ## Why a second path exists
//!
//! Stock LEAPR cannot emit a coherent-elastic section for a crystal that is not
//! one of its six. Both ENDF/B-VIII.0 silicon-carbide evaluations
//! (`tsl-CinSiC`, MAT 44; `tsl-SiinSiC`, MAT 43) are exactly that case: their
//! card 5 carries `iel = 0`, and their own comment cards say the coherent
//! elastic was produced by *modified* LEAPR source, citing Zhu and Hawari's
//! generalized formulation. Reproducing those evaluations from the distributed
//! decks therefore needs the general structure-factor sum, not another
//! hand-coded lattice. Bead `op-t33q` / `op-jw4a`, GitHub issue #24.
//!
//! ## Which one runs
//!
//! [`crate::leapr::generate::generate_tape`] uses the built-in path whenever
//! card 5 names a lattice, and falls back to the generalized path — with the
//! crystal looked up in [`crystals::GeneralCrystal::for_material`] — when
//! `iel = 0` *and* the material is one this crate has crystallographic data
//! for. A deck that is neither still yields no MT=2, which is the honest
//! outcome.

mod builtin;
pub mod crystals;
pub mod general;

pub use builtin::{coher, coher_with_constants, CoherentLattice};
pub use crystals::GeneralCrystal;
pub use general::{
    coher_general, coher_general_with_constants, coher_general_with_per_atom_debye_waller,
    BasisAtom, CrystalStructure,
};

/// The result of a coherent-elastic run: Bragg edges in ascending energy.
///
/// Each entry is `(E \[eV\], f_edge)` where `f_edge` is the (non-cumulative)
/// structure-factor contribution of that edge in \[barn eV\]. The ENDF
/// MF=7/MT=2 writer ([`crate::leapr::endout`]) forms the cumulative,
/// temperature-weighted
///
/// ```text
/// S(E, T) = sum_{E_i <= E} f_i exp(-4 W'(T) E_i)
/// ```
///
/// from these, and a transport code recovers the cross section as
/// `sigma(E) = S(E, T) / E` \[barn per principal atom\].
///
/// Forbidden reflections are retained with `f_edge = 0`, as NJOY does — the
/// energy grid is a property of the lattice, and dropping the dead entries
/// would change the grid a consumer interpolates on.
#[derive(Debug, Clone, PartialEq)]
pub struct BraggEdges {
    /// `(energy_eV, structure_factor_barn_eV)` pairs, ascending in energy, with
    /// near-degenerate edges (within `1e-6` eV) already merged.
    pub edges: Vec<(f64, f64)>,
}
