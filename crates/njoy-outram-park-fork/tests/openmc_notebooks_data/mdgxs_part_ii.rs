//! Notebook: **`mdgxs-part-ii`** (multi-delayed-group XS, part 2) — verification tests (op-6tz.6.4).
//!
//! Notebook provenance: openmc-notebooks `mdgxs-part-ii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part II builds an `openmc.mgxs.Library(num_delayed_groups=6)` over a 17×17
//! lattice, condenses β and χ_delayed by energy and delayed group, computes
//! precursor concentrations via tally arithmetic, and exports the library.
//!
//! The underlying delayed-neutron **data** (ENDF MF=1/455 precursor decay
//! constants + delayed ν̄_d, and MF=5/455 χ_delayed) is now parsed by njoy — see
//! `mdgxs_part_i.rs` for the live reader tests. What Part II adds on top is still
//! out of njoy's scope: a **delayed-group condensation over an energy-group MGXS
//! structure** (a GROUPR-style group collapse with a flux weight) and a
//! **transport solve** to produce the precursor-concentration tallies. Both
//! remain `#[ignore]`, with the missing capability named below.
//!
//! # Gaps (bead op-6tz.6.4, shared with mdgxs-part-i)
//!
//! - Delayed-group MGXS collapse (GROUPR-style condensation of β / χ_delayed /
//!   delayed ν·σ_f over an energy-group structure with a flux weight).
//! - Transport tallies for precursor concentrations (`outram-mc-libs`).

/// Notebook op: delayed-group condensation of β / χ_delayed over the energy +
/// delayed-group structure. The delayed data itself (MF=1/455, MF=5/455) is now
/// readable (`mdgxs_part_i.rs`); this op still needs the GROUPR-style
/// delayed-group MGXS collapse, which is not ported.
#[test]
#[ignore = "requires delayed-group MGXS collapse (GROUPR-style condensation); the MF=1/455 + MF=5/455 data is now read in mdgxs_part_i (op-6tz.6.4)"]
fn delayed_group_condensation() {
    panic!("delayed-group condensation needs a GROUPR-style group-collapse engine (data now available; collapse not ported)");
}

/// Notebook op: precursor concentration C_k,d from tally arithmetic +
/// library export. Requires transport tallies (`outram-mc-libs`) on top of the
/// now-available delayed data.
#[test]
#[ignore = "requires transport tallies (outram-mc-libs) for precursor concentrations; delayed data now available in mdgxs_part_i (op-6tz.6.4)"]
fn precursor_concentration_and_export() {
    panic!("precursor concentration needs transport tallies (outram-mc-libs); delayed data is now available");
}
