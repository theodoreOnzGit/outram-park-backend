//! ENDF tape reader/writer — the data substrate every module operates on.
//!
//! NJOY communicates between modules by passing **ENDF "tapes"**: files of
//! fixed-layout records keyed by material (`MAT`), file (`MF`), and section
//! (`MT`). Upstream this lives in `endf.f90` (the in-memory tape model and
//! record I/O) and `moder.f90` (the `MODER` module, which converts tapes between
//! ASCII and NJOY's blocked-binary format).
//!
//! The Rust port models a tape as owned data (no global scratch units), parsed
//! into typed records. Large numerical arrays (e.g. TAB1 interpolation tables,
//! cross-section grids) will use `ndarray` once Phase 1 lands.
//!
//! **Status:** scaffold only. See `docs/porting-plan.md` (Phase 1).

/// Identifies a single ENDF section by material, file, and section number.
///
/// - `mat` — material number (e.g. 9228 for U-235)
/// - `mf`  — file number (3 = cross sections, 7 = thermal scattering, …)
/// - `mt`  — reaction/section number (e.g. 1 = total, 2 = elastic, 18 = fission)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndfKey {
    /// Material number (`MAT`).
    pub mat: i32,
    /// File number (`MF`).
    pub mf: i32,
    /// Section number (`MT`).
    pub mt: i32,
}
