//! `MODER` — convert ENDF/PENDF/GENDF tapes between ASCII and NJOY binary, and
//! select materials onto a new tape.
//!
//! Ported from `moder.f90` (NJOY2016, 1714 lines — the README's ~0.6k-line
//! estimate undercounts it). In Fortran NJOY, MODER does two things:
//! (1) convert a tape between NJOY's blocked-binary mode and formatted
//! (ASCII) mode by walking every ENDF file type (`file1`..`file40`)
//! record-by-record, and (2) pull selected materials (by MAT number) off one
//! or more input tapes onto a single output tape (the card-3 `(nin, matd)`
//! loop in `moder.f90`, labels 130-205).
//!
//! In this crate's in-memory model, (1) collapses almost entirely: [`Tape`]
//! already parses an entire tape's data rows into [`Section`]s regardless of
//! "file type", with no NJOY blocked-binary concept to convert *from* (see
//! `src/moder/README.md` for why a blocked-binary reader/writer is not
//! planned). So the piece of MODER worth porting is (2), **material
//! selection**: [`select_materials`] below, faithfully mirroring the card-3
//! loop's logic (ascending-MAT-order check, "not found" is a warning that
//! skips the request rather than a fatal error — see its doc comment for the
//! exact Fortran line references).
//!
//! ASCII (de)serialisation is provided by [`crate::endf::tape::Tape::read`]
//! and [`crate::endf::tape::Tape::write`] (the latter added as part of this
//! port, since MODER is the first module that needs a tape *writer*).
//!
//! **Status:** material selection is ported (see [`select_materials`]); the
//! NJOY card-input driver (`run()`) remains [`crate::NjoyError::NotPorted`],
//! matching the convention used by every other module in this crate (e.g.
//! `crate::reconr`, `crate::broadr`) — physics/logic lives in a typed API,
//! `run()` is the deferred card-reader shim. See `README.md` in this
//! directory for theory, plan, and caveats.

use crate::endf::tape::{Section, Tape};
use crate::NjoyError;

/// One material-selection request — the Rust analogue of a single card-3
/// `(nin, matd)` pair in `moder.f90`'s selection loop (labels 130-205).
///
/// `nin` (a Fortran logical-unit number) becomes `tape_index`: an index into
/// the `inputs` slice passed to [`select_materials`], since this crate holds
/// every input tape fully parsed in memory rather than as physical units.
#[derive(Debug, Clone, Copy)]
pub struct MaterialSelection {
    /// Index into the `inputs` slice given to [`select_materials`] — the
    /// analogue of the Fortran input unit number `nin`.
    pub tape_index: usize,
    /// The material number (`MAT`) to pull from that tape — `matd` in the
    /// Fortran source.
    pub mat: i32,
}

/// Build a new tape containing the requested materials, drawn (in order)
/// from one or more input tapes — the material-selection half of MODER.
///
/// Ports the card-3 loop in `moder.f90` (labels 130-205): for each
/// `(tape_index, mat)` request, find every section on that input tape with
/// the requested `MAT` and append it (in file order) to the output. Sections
/// for one material are simply concatenated in request order; no attempt is
/// made to merge or re-sort MF/MT within a material beyond the order they
/// already appear on the source tape.
///
/// # Ascending-MAT-order check
///
/// `moder.f90:135-136` errors if a requested `matd` is not strictly greater
/// than the previous request's material (`matl`, seeded at `-2` to admit any
/// first `MAT`, ported verbatim as `LAST_MAT_SENTINEL` below):
///
/// ```text
/// if (inout.eq.1.and.matd.le.matl) call error('moder',
///   'endf materials must be in ascending order.',' ')
/// ```
///
/// That check is conditioned on `inout.eq.1` (the ENDF/PENDF tape path) in
/// Fortran; the GENDF (`inout=2`) and covariance (`inout=3`) paths do not
/// enforce it. This port does not distinguish tape "kind" (there is no
/// structural difference in the in-memory [`Section`] model), so the check is
/// applied **unconditionally** — a documented divergence from genuine NJOY
/// for GENDF/covariance-tape material selection, not a silent gap.
///
/// # "Material not found" is a warning, not a fatal error
///
/// `moder.f90:174-177` (the GENDF-path not-found case) calls `mess` — a
/// non-fatal warning that logs and continues to the next card — rather than
/// `error`:
///
/// ```text
/// write(strng,'(''mat='',i4,'' not found on gendf tape'')') matd
/// call mess('moder',strng,' ')
/// go to 130
/// ```
///
/// Per the crate's Fortran→Rust convention (`docs/porting-plan.md` §5:
/// `mess` → `log`, never a silent print), a selection whose `mat` is absent
/// from its `tape_index` tape logs a [`log::warn!`] and is skipped — it does
/// **not** abort the whole call. An out-of-range `tape_index` (no Fortran
/// analogue — this crate has no equivalent of an unopened logical unit
/// erroring at read time) is treated the same way: warned and skipped.
///
/// # Errors
///
/// Returns [`NjoyError::EndfParse`] only for the ascending-MAT-order
/// violation described above.
pub fn select_materials(
    inputs: &[Tape],
    tpid: &str,
    selections: &[MaterialSelection],
) -> Result<Tape, NjoyError> {
    // moder.f90:70 — `matl=-2`, a sentinel below any real MAT number so the
    // first selection's ascending-order check always passes.
    const LAST_MAT_SENTINEL: i32 = -2;

    let mut out_sections: Vec<Section> = Vec::new();
    let mut last_mat = LAST_MAT_SENTINEL;

    for sel in selections {
        // moder.f90:135-136 (see doc comment above for the `inout` caveat).
        if sel.mat <= last_mat {
            return Err(NjoyError::EndfParse(
                "moder: endf materials must be in ascending order".to_string(),
            ));
        }

        let Some(tape) = inputs.get(sel.tape_index) else {
            log::warn!(
                "moder: no input tape at index {} (requested mat={})",
                sel.tape_index,
                sel.mat
            );
            last_mat = sel.mat;
            continue;
        };

        // findf-equivalent: gather every section on this tape with this MAT,
        // in file order (moder.f90's `findf`/loop copies each MT of the
        // material verbatim via file1..file40).
        let mut found = false;
        for sec in tape.sections() {
            if sec.key.mat == sel.mat {
                out_sections.push(sec.clone());
                found = true;
            }
        }

        // moder.f90:174-177 — not found is a warning, not a fatal error.
        if !found {
            log::warn!(
                "moder: mat={} not found on input tape {}",
                sel.mat,
                sel.tape_index
            );
        }

        last_mat = sel.mat;
    }

    Ok(Tape::from_sections(tpid.to_string(), out_sections))
}

/// Run the MODER card-input driver. Placeholder — material selection is
/// reached directly through [`select_materials`]; ASCII (de)serialisation
/// through [`crate::endf::tape::Tape::read`] / [`crate::endf::tape::Tape::write`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "moder driver (selection: crate::moder::select_materials)",
    ))
}
