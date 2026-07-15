// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `DTFR` — DTF-IV format libraries for discrete-ordinates (Sₙ) codes.
//!
//! Prepares multigroup transport tables in the DTF-IV card-image format accepted
//! by many discrete-ordinates (Sₙ) and diffusion codes, from GROUPR GENDF output.
//! DTFR was NJOY's first output module and is largely superseded by MATXS/TRANSX.
//!
//! **Upstream:** `dtfr.f90` (git commit ac5adf5). **Manual:** LA-UR-17-20093
//! §DTFR.
//!
//! ## Module map
//!
//! | Submodule | Responsibility | Fortran |
//! |-----------|----------------|---------|
//! | [`input`] | the free-format user card deck + CLAW standard tables | `dtfr.f90:75-133, 590-767` |
//! | [`table`] | DTF `sig` layout, group ordering, reduced Sₙ scatter packing | `dtfr.f90:296-427` |
//! | [`format`] | DTF card/line formatting (`dtfout`) | `dtfr.f90:769-946` |
//! | [`driver`] | orchestration skeleton | `dtfr.f90:52-588` |
//!
//! ## Ported vs. not ported
//!
//! **Ported (self-contained, tested):** the input deck ([`DtfrInput`], the
//! selector enums, and the CLAW standard edit tables); the DTF table layout and
//! the reduced-length, up-scatter-capable **triangular scatter-matrix packing**
//! with its transport (absorption) correction ([`table`]); the `dtfout` card /
//! line formatting for both the in-table (format 0) and td6/CLAW (format 1)
//! styles ([`format`]).
//!
//! **Not ported:** the GENDF-tape reader (`contio`/`listio`/`moreio` walk of a
//! GROUPR tape, `dtfr.f90:181-553`) — the numeric values are supplied to the
//! ported kernels via the in-memory [`table::DtfTable`]. The plotting paths
//! (`ploted`/`plotnn`/`plotnp`, `dtfr.f90:948-1507`) are **permanently out of
//! scope** (viewr/PostScript output). [`driver::run`] documents the pipeline and
//! returns [`NjoyError::NotPorted`].

use crate::NjoyError;

pub mod driver;
pub mod format;
pub mod input;
pub mod table;

pub use driver::run as run_with_input;
pub use format::{column, format0_body, format0_header, fortran_e, pack_dtf_block};
pub use input::{
    DtfrInput, EditOption, EditSpec, FilmOption, MaterialDesc, NeutronTables, PrintOption,
    ThermalSpec, UnitAssignments,
};
pub use table::{dtf_group, scatter_position, DtfTable};

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// DTFR needs a full input deck ([`DtfrInput`]) plus a GENDF tape to run, so
/// this no-argument form exists only so the module registry can name DTFR; it
/// reports that the end-to-end pipeline is not yet ported. For the real entry
/// point (once the GENDF reader lands) use [`driver::run`] with a [`DtfrInput`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"dtfr"`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("dtfr"))
}
