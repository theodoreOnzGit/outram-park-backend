// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! DTFR top-level orchestration skeleton (`subroutine dtfr`, `dtfr.f90:52-588`).
//!
//! Documents the DTFR pipeline stage by stage and dispatches to the ported
//! table-assembly and formatting helpers. The GENDF tape reader
//! (`contio`/`listio`/`moreio` walk of a GROUPR tape) and the PostScript/viewr
//! plotting paths are **not** ported, so [`run`] returns
//! [`NjoyError::NotPorted`] rather than fabricating a table.

use crate::dtfr::input::DtfrInput;
use crate::NjoyError;

/// Run the DTFR pipeline for a full input deck (`dtfr.f90:52-588`).
///
/// The card deck ([`DtfrInput`]), the DTF table layout / scatter-matrix packing
/// ([`crate::dtfr::table`]), and the DTF card/line formatting
/// ([`crate::dtfr::format`]) are ported and tested. The GENDF-tape reader that
/// supplies the raw multigroup values and the plotting routines
/// (`ploted`/`plotnn`/`plotnp`) are not, so this function documents the stages
/// and returns [`NjoyError::NotPorted`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"dtfr::run"` until the GENDF
/// reader lands. The plotting paths are permanently out of scope and would
/// report `"dtfr::plot"`.
pub fn run(input: &DtfrInput) -> Result<(), NjoyError> {
    // --- Stage 0: user input (ruin, dtfr.f90:590-767) --------------------
    //   card 1 nin/nout/npend/nplot; card 2 iprint/ifilm/iedit;
    //   iedit==0 -> card 3 nlmax/ng/iptotl/ipingp/itabl/ned/ntherm (+3a);
    //   cards 4/5 edit names + specs;  iedit==1 -> card 6 CLAW defaults;
    //   card 7 nptabl/ngp; the three standard edits appended.
    //   Modelled by `input` (see DtfrInput::claw_defaults / standard_edits).
    let _ = input;

    // --- Stage 1: per material/dilution/temperature (dtfr.f90:105-272) ---
    //   card 8 hisnam/matd/jsigz/dtemp; find matd on the GENDF tape, read the
    //   header (nsigz/ntw/ngn/ngg + egn/egg group bounds), copy the material to
    //   scratch; locate the matching temperature on the PENDF tape for plots.
    //   <-- GENDF/PENDF tape reader NOT ported.

    // --- Stage 2: table accumulation (dtfr.f90:274-553) ------------------
    //   loop the scratch records; per record dispatch by MF/MT:
    //     mf3/mf23 -> cross sections & edits (table::DtfTable::add, group
    //                 ordering via table::dtf_group);
    //     mf6/mf26 -> neutron transfer matrix (table::add_scatter_record —
    //                 reduced-band packing + transport correction);
    //     mt18/19.. -> fission nu*sigf and chi accumulation;
    //     mt455     -> delayed neutrons; mf16/mf17 -> photon production.

    // --- Stage 3: finish & output (dtfr.f90:559-573) ---------------------
    //   normalise chi by cnorm; dtfout(sig,...) writes the DTF tables
    //   (format::format0_header/format0_body for iedit==0, or
    //   format::pack_dtf_block for the td6/CLAW blocks).

    // --- Plotting (dtfr.f90:940-1507) — permanently out of scope ---------
    //   ploted/plotnn/plotnp emit viewr/PostScript plot streams; NOT ported.

    Err(NjoyError::NotPorted("dtfr::run"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The driver reports `NotPorted`, never a fabricated DTF table.
    ///
    /// **Methodology.** DTFR's GENDF reader is not ported; [`run`] must surface
    /// that honestly. Call it with a CLAW deck and assert the documented tag.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `run(&claw_deck)` →
    /// `NotPorted("dtfr::run")`.
    #[test]
    fn run_is_not_ported() {
        let mut deck = DtfrInput::claw_defaults(5, 30);
        deck.units.nin = -20;
        deck.units.nout = 21;
        match run(&deck) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "dtfr::run"),
            other => panic!("expected NotPorted(\"dtfr::run\"), got {other:?}"),
        }
    }
}
