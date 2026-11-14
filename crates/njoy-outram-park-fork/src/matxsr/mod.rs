// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90` (2512 lines).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `MATXSR` — MATXS generalized material cross-section files (for TRANSX).
//!
//! Writes the MATXS material cross-section format — a generalized CCCC-type
//! interface for neutron, photon, and charged-particle data including cross
//! sections, group-to-group matrices, temperature variation, self-shielding, and
//! time dependence — from GROUPR/GAMINR GENDF output. MATXS libraries are the
//! input to the TRANSX code, which builds effective cross sections for transport.
//!
//! **Upstream:** `matxsr.f90`. **Manual:** LA-UR-17-20093 §MATXSR.
//! ~~**Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.~~
//! **CORRECTED 2026-11-13 (gh:#1369):** translated routine by routine:
//! [`cards`] (`matxsr`'s cards and `ruinm`), [`data`] (`mtxdat`, `findg`,
//! `vector`), [`matrix`] (`matrix`, `band`, `shufl`), [`hname`] and [`output`]
//! (`cmatxs`, coded branch). NJOY2016 test decks 03 and 30 give the MATXS file
//! line for line (`tests/deck_driver_photon_vs_njoy2016.rs`).
//!
//! The container `a` is `real(k4)`: every number on the file is the `f32`
//! upstream stores, printed `1p,e12.5`.
//!
//! Refused by name: a binary MATXS (`nmatx < 0`), and `shufl` with a container
//! too small for one band (upstream's `imax` is then undefined). `lst1io`'s
//! text read of the MF=1 head is replaced by the GENDF reader's own (same
//! columns); its title word is not used.

pub mod cards;
pub(crate) mod data;
pub mod hname;
pub(crate) mod matrix;
pub(crate) mod output;

pub use cards::{read_matxsr, MatxsrCards};

use crate::errorr::gendf::GendfTape;
use crate::NjoyError;

/// Run MATXSR with no cards: the module registry entry. A deck runs it with
/// [`run_matxsr`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("matxsr without its cards (use deck::run_deck)"))
}

/// Run MATXSR (`matxsr`, `cmatxs`) on the GENDF tapes `tapes` (by unit
/// number): the coded MATXS file.
///
/// # Errors
/// A GENDF missing a section `findg` looks for, a malformed record, or a
/// refusal named in the module doc.
pub fn run_matxsr(c: &MatxsrCards, tapes: Vec<(i32, GendfTape)>) -> Result<String, NjoyError> {
    let (npart, ntype, nholl, nmat) = (c.control[0], c.control[1], c.control[2], c.control[3]);
    // `ruinm`'s pointers (`:820-893`).
    let lim = 9;
    let icont = 1;
    let mut next = icont + 6;
    let iholl = next;
    next = iholl + nholl * data::MULT * lim;
    let ifild = next;
    let mut nwds = (npart + ntype + nmat) * data::MULT + 2 * ntype + 2 * nmat + npart;
    nwds = ((nwds + 1) / 2) * 2;
    next = ifild + nwds;
    let mut m = data::Matxs {
        c: c.clone(),
        npart: npart as usize,
        ntype: ntype as usize,
        nmat: nmat as usize,
        maxw: 5000,
        nsubmx: 100,
        maxord: 5,
        next,
        length: 0,
        nsubm: Vec::new(),
        loca: Vec::new(),
        groups: Vec::new(),
        ning: 0,
        noutg: 0,
        iz: 1,
        nritev: 0,
        nritem: 0,
        scr2: Vec::new(),
        scr3: Vec::new(),
        scr6: Vec::new(),
        hvps: Vec::new(),
        tapes,
    };
    m.mtxdat()?;
    output::cmatxs(&mut m)
}

/// MATXSR on the deck's units: the GENDF tapes on `ngen1..ngen8`, the MATXS
/// file to `nmatx`.
///
/// # Errors
/// Those of [`run_matxsr`] and of the units.
pub fn run_matxsr_deck(c: &MatxsrCards, units: &mut crate::deck::tapes::TapeUnits) -> Result<(), NjoyError> {
    let mut tapes = Vec::new();
    for (k, &u) in c.units.iter().enumerate() {
        if k == 2 || u == 0 {
            continue;
        }
        let g = units.gendf(u)?.ok_or_else(|| NjoyError::InvalidInput(format!("matxsr: unit {u} holds no gendf tape")))?;
        tapes.push((u, g));
    }
    let out = run_matxsr(c, tapes)?;
    let nmatx = c.units[2];
    units.openz(nmatx, 1)?;
    units.put_text(nmatx, out);
    Ok(())
}
