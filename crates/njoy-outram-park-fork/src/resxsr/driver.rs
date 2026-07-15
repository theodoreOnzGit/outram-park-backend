// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RESXSR top-level orchestration skeleton (`subroutine resxsr`,
//! `resxsr.f90:10-502`).
//!
//! Documents the RESXSR pipeline stage by stage and dispatches to the ported
//! kernels. The tape reader (PENDF `gety1` walk, `loada`/`finda` scratch files)
//! and the binary RESXS writer are not ported, so [`run`] returns
//! [`NjoyError::NotPorted`] rather than fabricating an output file.

use crate::resxsr::input::ResxsrInput;
use crate::NjoyError;

/// Run the RESXSR pipeline for a full input deck (`resxsr.f90:10-502`).
///
/// The card deck ([`ResxsrInput`]), the RESXS record layout
/// ([`crate::resxsr::format`]), and the per-material union-grid + thinning
/// kernels ([`crate::resxsr::assemble`]) are ported and tested; the tape reader
/// that supplies real pointwise `gety1` values and the binary RESXS writer are
/// not, so this function documents the stages and returns
/// [`NjoyError::NotPorted`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"resxsr::run"` until the PENDF
/// reader and RESXS writer land.
pub fn run(input: &ResxsrInput) -> Result<(), NjoyError> {
    // --- Stage 0: user input (resxsr.f90:236-248) ------------------------
    //   card 1 nout; card 2 nmat,maxt,nholl,efirst,elast,eps;
    //   card 3 huse,ivers; card 4 holl[nholl]; card 5 hmat/mat/unit[nmat].
    //   Modelled by `input`.
    let _ = input;

    // --- Stage 1: per material (resxsr.f90:250-433) ----------------------
    //   openz(nin); tpidio; loop temperatures (contio/hdatio, iverf detect);
    //   findf(matd,3): for each resonance MT (2/18/102) walk the grid with
    //   gety1 and merge into the union set
    //   (assemble::assemble_union_grid), then thin with eps
    //   (assemble::thin_linear).  <-- kernels ported; the gety1/loada/finda
    //   tape plumbing that feeds them is not.

    // --- Stage 2: material control + xs blocks to scratch ----------------
    //   resxsr.f90:399-430 — write material control (format::MaterialControl)
    //   then the thinned points blocked by nblok (format::xs_* helpers).

    // --- Stage 3: RESXS output file (resxsr.f90:435-501) -----------------
    //   file identification / file control / set-Hollerith / file data
    //   (format::FileIdentification/FileControl/FileData), then copy the
    //   material control + xs blocks from scratch to nout.

    Err(NjoyError::NotPorted("resxsr::run"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resxsr::input::MaterialSpec;

    /// The driver reports `NotPorted`, never a fabricated RESXS file.
    ///
    /// **Methodology.** RESXSR's tape reader and binary writer are not ported;
    /// [`run`] must surface that honestly. Call it with a populated deck and
    /// assert the documented `NotPorted` tag.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `run(&deck)` →
    /// `NotPorted("resxsr::run")`.
    #[test]
    fn run_is_not_ported() {
        let deck = ResxsrInput {
            nout: -21,
            maxt: 3,
            efirst: 4.0,
            elast: 200.0,
            eps: 1.0e-3,
            user_id: "outrampark".into(),
            ivers: 1,
            comments: vec!["resonance test".into()],
            materials: vec![MaterialSpec { hmat: "u238".into(), mat: 9237, nin: 20 }],
        };
        match run(&deck) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "resxsr::run"),
            other => panic!("expected NotPorted(\"resxsr::run\"), got {other:?}"),
        }
    }
}
