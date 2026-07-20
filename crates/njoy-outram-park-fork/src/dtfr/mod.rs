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
//! **Ported (minimal reader):** a self-contained GENDF record reader
//! ([`gendf`], `dtfr.f90:199-427`) sufficient to feed the ported kernels — it
//! reads the MF=1/451 group-structure header and the MF=3/MF=6 reaction LIST
//! records over an in-memory [`crate::endf::tape::Tape`], and
//! [`gendf::build_neutron_table`] assembles the P0 total + elastic-scatter DTF
//! table from them. It depends only on the generic ENDF cursor, **not** on the
//! GROUPR writer module.
//!
//! **Not ported:** the full DTFR accumulation loop (fission ν·σ_f/χ, edits,
//! thermal corrections, photons, higher Legendre orders, `dtfr.f90:430-553`), the
//! `contio`/`moreio` scratch-tape staging, and the plotting paths
//! (`ploted`/`plotnn`/`plotnp`, `dtfr.f90:948-1507`, **permanently out of
//! scope** — viewr/PostScript). [`driver::run`] documents the pipeline and
//! returns [`NjoyError::NotPorted`].

use crate::NjoyError;

pub mod driver;
pub mod format;
pub mod gendf;
pub mod input;
pub mod table;

pub use driver::run as run_with_input;
pub use format::{column, format0_body, format0_header, fortran_e, pack_dtf_block};
pub use gendf::{
    build_neutron_table, read_header, read_section_records, GendfGroupRecord, GendfHeader,
};
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
