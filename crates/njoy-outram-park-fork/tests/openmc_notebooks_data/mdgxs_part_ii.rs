//! Notebook: **`mdgxs-part-ii`** (multi-delayed-group XS, part 2) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mdgxs-part-ii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part II builds an `openmc.mgxs.Library(num_delayed_groups=6)` over a 17×17
//! lattice, condenses β and χ_delayed by energy and delayed group, computes
//! precursor concentrations via tally arithmetic, and exports the library. It
//! depends on the same delayed-neutron data njoy has not ported (MF=1/455 and
//! MF=5/455) *plus* a transport solve for the tallies. Every operation is
//! scaffolded `#[ignore]`, sharing the delayed-data gap with `mdgxs-part-i`.
//!
//! # Gaps (bead under op-6tz.6, shared with mdgxs-part-i)
//!
//! - ENDF MF=1/455 + MF=5/455 delayed-neutron data.
//! - Delayed-group condensation over an MGXS structure.
//! - Transport tallies for precursor concentrations.

/// Notebook op: delayed-group condensation of β / χ_delayed over the energy +
/// delayed-group structure. Requires the delayed-neutron data (MF=1/455,
/// MF=5/455) and a delayed-group MGXS collapse.
#[test]
#[ignore = "requires delayed-neutron data (MF=1/455, MF=5/455) + delayed-group condensation (op-6tz.6)"]
fn delayed_group_condensation() {
    panic!("njoy has no delayed-neutron data (MF=1/455, MF=5/455) to condense");
}

/// Notebook op: precursor concentration C_k,d from tally arithmetic +
/// library export. Requires transport tallies plus the delayed data above.
#[test]
#[ignore = "requires transport tallies + delayed-neutron data for precursor concentrations (op-6tz.6)"]
fn precursor_concentration_and_export() {
    panic!("precursor concentration needs transport tallies (outram-mc-libs) + delayed data (MF=1/455)");
}
