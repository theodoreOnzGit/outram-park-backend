// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `RESXSR` — pointwise resonance cross-section (RESXS) files for near-epithermal
//! codes.
//!
//! Prepares pointwise resonance cross-section libraries for the near-epithermal
//! range (≈4–200 eV) where the simple Bondarenko narrow-resonance model used by
//! TRANSX is inadequate because many resonances are *not* narrow with respect to
//! the scattering energy loss. RESXSR supplies the pointwise data needed for
//! intermediate-resonance / detailed self-shielding treatments.
//!
//! **Upstream:** `resxsr.f90` (git commit ac5adf5). **Manual:** LA-UR-17-20093
//! §RESXSR.
//!
//! ## Module map
//!
//! | Submodule | Responsibility | Fortran |
//! |-----------|----------------|---------|
//! | [`input`] | the free-format user card deck | `resxsr.f90:16-40, 236-248` |
//! | [`format`] | RESXS record layout, header/field values, word counts | `resxsr.f90:43-189, 399-501` |
//! | [`assemble`] | union energy grid + adaptive linear thinning | `resxsr.f90:306-397` |
//!
//! ## Ported vs. not ported
//!
//! **Ported (self-contained, tested):** the input deck ([`ResxsrInput`]); the
//! RESXS record layout and its word-count arithmetic ([`format`]); the union-grid
//! assembly and thinning kernels ([`assemble`]) operating on already-parsed
//! reactions.
//!
//! **Not ported (file-level I/O):** reading a real PENDF tape end-to-end
//! (`tpidio`/`contio`/`findf`/`gety1`, `resxsr.f90:250-352`), the
//! `loada`/`finda` scratch-file buffering, and writing the binary RESXS output
//! records (`resxsr.f90:435-501`). [`run`] documents this pipeline and returns
//! [`NjoyError::NotPorted`] rather than fabricating an output file.

use crate::NjoyError;

pub mod assemble;
pub mod driver;
pub mod format;
pub mod input;

pub use assemble::{assemble_union_grid, thin_linear, PointwiseReaction, UnionRow};
pub use driver::run as run_with_input;
pub use format::{
    FileControl, FileData, FileIdentification, MaterialControl, FILE_NAME, MULT, NBLOK, NBUF, NX,
};
pub use input::{MaterialSpec, ResxsrInput};

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// RESXSR needs a full input deck ([`ResxsrInput`]) plus a PENDF tape to run, so
/// this no-argument form exists only so the module registry can name RESXSR; it
/// reports that the end-to-end pipeline is not yet ported. For the real entry
/// point (once the tape reader / RESXS writer land) use [`driver::run`] with a
/// [`ResxsrInput`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"resxsr"`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("resxsr"))
}
