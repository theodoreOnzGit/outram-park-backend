//! Shared infrastructure used by every NJOY module.
//!
//! In upstream NJOY2016 this material is spread across `util.f90` (general
//! utilities, the `error`/`mess` reporters, interpolation helpers), `mainio.f90`
//! (logical unit numbers for the standard input/output/scratch files),
//! `locale.f90` (locale-dependent formatting), `phys.f90` (physical constants),
//! and `mathm.f90` (special-function / linear-algebra helpers).
//!
//! In the Rust port these become ordinary modules with `uom` constants and
//! pure functions — no global mutable I/O unit table, no fixed `common` blocks.
//!
//! **Status:** scaffold only. See `docs/porting-plan.md` (Phase 1).

/// Physical constants from CODATA 2014 / ENDF-102 Appendix H.
/// Ported from NJOY2016 `phys.f90`.
pub mod phys;

/// Special-function and small-matrix helpers (from `mathm.f90`).
pub mod mathm {
    //! Math helpers (from NJOY2016 `mathm.f90`). Not yet ported.
}
