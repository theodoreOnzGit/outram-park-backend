//! Loading the reference fixtures captured from Yan Ren's MATLAB.
//!
//! # What a fixture is, and what it is not
//!
//! These files record what Yan Ren's implementation produced when it was run
//! under GNU Octave — see `tests/fixtures/<case>/PROVENANCE.md` for the
//! interpreter, the shims applied, and the capture date. They pin the
//! *reference*, not the *truth*: agreement with a fixture shows the Rust
//! translation is faithful, which is a different claim from being correct.
//! Comparison against the published IAEA-3D benchmark values is a separate
//! check (`docs/bedok-port-scoping.md` §4).
//!
//! # Two tiers of fixture, and why
//!
//! **Reduced, committed** — under `tests/fixtures/<case>/`, about 20 kB. These
//! are the quantities the IAEA-3D benchmark itself reports, and they are what a
//! routine parity gate compares:
//!
//! | File | Shape | Loader |
//! |---|---|---|
//! | `k_eff.csv` | scalar | [`load_scalar`] |
//! | `final_residuals.csv` | 1 × 2 | [`load_row`] |
//! | `radial_power_map.csv` | 17 × 17 | [`load_matrix`] |
//! | `axial_power_profile.csv` | 19 × 1 | [`load_matrix`] |
//!
//! The reduced files are **plain matrices with no index columns** — 17 rows of
//! 17 comma-separated values, and 19 rows of one value respectively.
//!
//! **Full node-level, not committed** — about 1.4 MB of text under
//! `collaboration/bedok-full-fixtures/<case>/` (gitignored, regenerable in
//! ~77 s with the command in [`REGENERATE_FULL_FIXTURES`]). These are
//! `power_density.csv`, `fission_source.csv` and `scalar_flux.csv`, and they
//! only matter once a parity failure has to be pinned to a specific node.
//! A fresh clone will not have them, so anything that reads them must check
//! [`full_fixtures_available`] first and skip with a message rather than fail.
//!
//! # Indexed-field format
//!
//! The full fields have no header row and are laid out as
//!
//! ```text
//! g,ix,iy,iz,value[,value...]
//! ```
//!
//! with **1-based** MATLAB indices in the first four columns. Values are
//! written at `%.17g`, so every entry round-trips through an IEEE double
//! exactly — a fixture comparison is therefore never limited by the file
//! format.
//!
//! The explicit index columns exist so the port never infers the flattening
//! order. **Every loader here routes those indices through
//! [`Grid::index_from_matlab`]**, the single place the 1-based → 0-based
//! conversion is allowed to happen. Nothing in this module subtracts one by
//! hand, and neither should any caller: a silent off-by-one in the index
//! convention permutes the reactor without crashing anything, which is the
//! failure mode the [`grid`](crate::reference::grid) module exists to prevent.
//!
//! Row order within a file is *not* trusted. Each row is placed at the flat
//! index its own coordinates dictate, and the loader reports any slot written
//! twice or left empty.
//!
//! # Repo-local paths
//!
//! [`fixture_dir`] and [`full_fixture_dir`] resolve against
//! `CARGO_MANIFEST_DIR`, so they only work in a checkout of this repository.
//! The crate is `publish = false` and the fixtures live under `tests/`, so
//! this is deliberate rather than a limitation to work around.

use std::path::{Path, PathBuf};

use crate::error::{BedokError, Result};
use crate::reference::grid::Grid;

/// Fixture directory name for the IAEA-3D steady-state case.
pub const IAEA3D: &str = "iaea3d";

/// `k_eff` recorded in `tests/fixtures/iaea3d/k_eff.csv`.
///
/// Yan Ren's converged eigenvalue for IAEA-3D — `1.0290842762` to the ten
/// figures quoted in `PROVENANCE.md`, carried here at the full `%.17g`
/// precision of the file. This is the *reference* value, not the published
/// benchmark value; see the module docs on the difference.
pub const IAEA3D_K_EFF: f64 = 1.0290842761799579;

/// Number of value columns in the full `scalar_flux.csv`.
///
/// Column 1 is the converged flux; columns 2–5 are retained iterates consumed
/// by the MATLAB's fission-source extrapolation path.
pub const SCALAR_FLUX_COLUMNS: usize = 5;

/// Shell command that regenerates the uncommitted full node-level fixtures.
///
/// Quoted verbatim from `tests/fixtures/iaea3d/PROVENANCE.md` so that a test
/// which skips for want of those files can name the exact fix.
pub const REGENERATE_FULL_FIXTURES: &str = concat!(
    "cd collaboration/BEDOKfiles && ",
    "octave --no-gui --quiet --eval \"addpath('../octave-shims'); addpath('.'); capture_iaea3ds\""
);

/// Environment variable that overrides where the full fixtures are looked for.
///
/// Unset in normal use. Exists so the full fields can be regenerated somewhere
/// other than the default gitignored directory without editing code.
pub const FULL_FIXTURE_DIR_ENV: &str = "BEDOK_FULL_FIXTURES";

/// The node grid the IAEA-3D fixtures were captured on.
///
/// 17 × 17 × 19 nodes in 2 energy groups: 5,491 nodes and 10,982 state
/// entries. Note `nz = 19`, not the 18 nodes the case input requests — the
/// MATLAB case constructor appends an axial reflector plane.
///
/// # Errors
///
/// Cannot fail in practice; the signature is fallible only because
/// [`Grid::new`] is.
pub fn iaea3d_grid() -> Result<Grid> {
    Grid::new(17, 17, 19, 2)
}

/// Absolute path of the **committed reduced** fixture directory for `case`,
/// e.g. [`IAEA3D`].
///
/// Resolves to `<crate root>/tests/fixtures/<case>`. Repo-local; see the
/// module docs.
#[must_use]
pub fn fixture_dir(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

/// Absolute path of the **uncommitted full node-level** fixture directory for
/// `case`.
///
/// Resolves to `<workspace root>/collaboration/bedok-full-fixtures/<case>`,
/// unless [`FULL_FIXTURE_DIR_ENV`] is set, in which case that path is used as
/// the parent directory instead. The directory is gitignored and may well not
/// exist — check [`full_fixtures_available`] before reading from it.
#[must_use]
pub fn full_fixture_dir(case: &str) -> PathBuf {
    match std::env::var_os(FULL_FIXTURE_DIR_ENV) {
        Some(root) => PathBuf::from(root).join(case),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("collaboration")
            .join("bedok-full-fixtures")
            .join(case),
    }
}

/// Whether the full node-level fixtures for `case` are present on this
/// machine.
///
/// Checks for the directory and for every file [`Iaea3dFullFields`] reads, so
/// a partial regeneration counts as absent rather than failing halfway through
/// a comparison.
#[must_use]
pub fn full_fixtures_available(case: &str) -> bool {
    let dir = full_fixture_dir(case);
    ["power_density.csv", "fission_source.csv", "scalar_flux.csv"]
        .iter()
        .all(|f| dir.join(f).is_file())
}

/// Reads a fixture holding exactly one number on one line, such as
/// `k_eff.csv`.
///
/// # Errors
///
/// [`BedokError::Fixture`] if the file is missing, empty, holds more than one
/// row or column, or the value does not parse as a float.
pub fn load_scalar<P: AsRef<Path>>(path: P) -> Result<f64> {
    let path = path.as_ref();
    let row = load_single_row(path)?;
    if row.len() != 1 {
        return Err(fixture_err(
            path,
            format!("expected 1 value, found {}", row.len()),
        ));
    }
    Ok(row[0])
}

/// Reads a fixture holding one row of numbers, such as `final_residuals.csv`.
///
/// # Errors
///
/// [`BedokError::Fixture`] if the file is missing, empty, holds more than one
/// row, or any value fails to parse.
pub fn load_row<P: AsRef<Path>>(path: P) -> Result<Vec<f64>> {
    load_single_row(path.as_ref())
}

/// Reads a plain matrix CSV — no index columns — into a **row-major** flat
/// vector of length `rows * cols`.
///
/// This is the shape of the committed reduced fixtures:
/// `radial_power_map.csv` is `load_matrix(path, 17, 17)` and
/// `axial_power_profile.csv` is `load_matrix(path, 19, 1)`.
///
/// Row-major means entry `(r, c)` sits at `r * cols + c`. For the radial map
/// that is `ix * ny + iy` — see [`Iaea3dReduced::radial_power_map`] for the
/// orientation convention and its one unresolved ambiguity.
///
/// # Errors
///
/// [`BedokError::Fixture`] if the row count differs from `rows`, any row has a
/// column count differing from `cols`, or any entry fails to parse.
pub fn load_matrix<P: AsRef<Path>>(path: P, rows: usize, cols: usize) -> Result<Vec<f64>> {
    let path = path.as_ref();
    let mut out = Vec::with_capacity(rows * cols);
    let mut reader = open(path)?;

    let mut seen = 0usize;
    for (row_no, record) in reader.records().enumerate() {
        let record = record.map_err(|e| fixture_err(path, format!("row {}: {e}", row_no + 1)))?;
        if record.len() != cols {
            return Err(fixture_err(
                path,
                format!(
                    "row {}: expected {cols} columns, found {}",
                    row_no + 1,
                    record.len()
                ),
            ));
        }
        for (col, field) in record.iter().enumerate() {
            out.push(parse_value(path, row_no, col, field)?);
        }
        seen += 1;
    }

    if seen != rows {
        return Err(fixture_err(
            path,
            format!("expected {rows} rows, found {seen}"),
        ));
    }
    Ok(out)
}

/// Reads a single-valued indexed field CSV (`g,ix,iy,iz,value`) into a flat
/// vector in this crate's 0-based ordering.
///
/// The returned vector has length [`Grid::state_len`], with entry `i` holding
/// the value whose MATLAB coordinates map to flat index `i` under
/// [`Grid::index_from_matlab`].
///
/// # Errors
///
/// [`BedokError::Fixture`] if the row count differs from `grid.state_len()`,
/// a row has the wrong number of columns, an index is out of range for `grid`,
/// two rows claim the same node, or any field fails to parse.
pub fn load_field<P: AsRef<Path>>(path: P, grid: &Grid) -> Result<Vec<f64>> {
    let mut columns = load_field_columns(path, grid, 1)?;
    Ok(columns.remove(0))
}

/// Reads a multi-column indexed field CSV (`g,ix,iy,iz,v1,…,vN`) into
/// `columns` flat vectors, each in this crate's 0-based ordering.
///
/// `scalar_flux.csv` is the case in point: pass
/// [`SCALAR_FLUX_COLUMNS`] and take element 0 for the converged flux,
/// elements 1–4 for the retained iterates.
///
/// # Errors
///
/// As [`load_field`], plus [`BedokError::Fixture`] if `columns` is zero or a
/// row does not carry exactly `4 + columns` fields.
pub fn load_field_columns<P: AsRef<Path>>(
    path: P,
    grid: &Grid,
    columns: usize,
) -> Result<Vec<Vec<f64>>> {
    let path = path.as_ref();
    if columns == 0 {
        return Err(fixture_err(
            path,
            "asked for zero value columns".to_string(),
        ));
    }
    let expected_fields = 4 + columns;
    let len = grid.state_len();

    let mut out = vec![vec![f64::NAN; len]; columns];
    let mut filled = vec![false; len];
    let mut rows = 0usize;
    let mut reader = open(path)?;

    for (row_no, record) in reader.records().enumerate() {
        let record = record.map_err(|e| fixture_err(path, format!("row {}: {e}", row_no + 1)))?;
        if record.len() != expected_fields {
            return Err(fixture_err(
                path,
                format!(
                    "row {}: expected {expected_fields} fields, found {}",
                    row_no + 1,
                    record.len()
                ),
            ));
        }

        let g = parse_index(path, row_no, "g", &record[0])?;
        let ix = parse_index(path, row_no, "ix", &record[1])?;
        let iy = parse_index(path, row_no, "iy", &record[2])?;
        let iz = parse_index(path, row_no, "iz", &record[3])?;

        // The one and only 1-based -> 0-based conversion in this module.
        let idx = grid.index_from_matlab(g, ix, iy, iz).map_err(|e| {
            fixture_err(
                path,
                format!("row {}: ({g},{ix},{iy},{iz}) rejected: {e}", row_no + 1),
            )
        })?;

        if filled[idx] {
            return Err(fixture_err(
                path,
                format!(
                    "row {}: node ({g},{ix},{iy},{iz}) written twice",
                    row_no + 1
                ),
            ));
        }
        filled[idx] = true;

        for (col, slot) in out.iter_mut().enumerate() {
            slot[idx] = parse_value(path, row_no, col, &record[4 + col])?;
        }
        rows += 1;
    }

    if rows != len {
        return Err(fixture_err(
            path,
            format!("expected {len} rows for the grid, found {rows}"),
        ));
    }
    if let Some(missing) = filled.iter().position(|f| !f) {
        // Unreachable while `rows == len` and duplicates are rejected, but a
        // silently unwritten node is exactly the failure this module guards
        // against, so it is checked rather than argued about.
        return Err(fixture_err(
            path,
            format!("no row supplied a value for flat index {missing}"),
        ));
    }

    Ok(out)
}

/// The committed reduced IAEA-3D reference quantities.
///
/// This is what a routine parity gate compares against: the eigenvalue, the
/// residuals the reference stopped at, and the two power shapes the IAEA-3D
/// benchmark itself reports. Present in every clone.
#[derive(Debug, Clone)]
pub struct Iaea3dReduced {
    /// The grid the fixtures were captured on. See [`iaea3d_grid`].
    pub grid: Grid,
    /// Converged multiplication factor \[-\]. Matches [`IAEA3D_K_EFF`].
    pub k_eff: f64,
    /// Final fission-source residual the MATLAB stopped at \[-\].
    ///
    /// Column 1 of `final_residuals.csv`; 9.611040e-07 as captured. The fields
    /// are therefore only determined to about this level — a tolerance tighter
    /// than the reference's own convergence criterion is not meaningful.
    pub fission_source_residual: f64,
    /// Final `k_eff` residual the MATLAB stopped at \[-\].
    ///
    /// Column 2 of `final_residuals.csv`; 9.272337e-10 as captured. This sets
    /// the floor on how tightly *any* faithful translation can be expected to
    /// reproduce `k_eff`: the reference itself is converged only this far.
    pub k_eff_residual: f64,
    /// Radial power map \[-\], `nx * ny` entries, **row-major with `ix` as the
    /// row index**: entry `(ix, iy)` sits at `ix * ny + iy`.
    ///
    /// Power summed over `z` and over both energy groups, then normalised so
    /// the mean over *powered* (non-zero) nodes is exactly 1. The 112 unpowered
    /// reflector positions of the 289 are exact zeros.
    ///
    /// # Orientation caveat
    ///
    /// The captured map is symmetric under transposition — IAEA-3D is
    /// quadrant-symmetric — so the data cannot distinguish "row = `ix`" from
    /// "row = `iy`". Row-is-`ix` is assumed here because that is what MATLAB's
    /// `writematrix` of a `[maxix × maxiy]` array produces. Any future
    /// asymmetric case must re-check this rather than inherit the assumption.
    pub radial_power_map: Vec<f64>,
    /// Axial power profile \[-\], `nz` entries indexed by `iz`.
    ///
    /// Power summed over `x` and `y` and over both groups, normalised the same
    /// way: mean 1 over the 17 powered planes, with the two reflector planes
    /// exactly zero.
    pub axial_power_profile: Vec<f64>,
}

impl Iaea3dReduced {
    /// Loads the reduced fixtures from the in-repo directory returned by
    /// [`fixture_dir`]`(`[`IAEA3D`]`)`.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] if any file is missing, malformed, or does not
    /// match the IAEA-3D grid shape.
    pub fn load() -> Result<Self> {
        Self::load_from(fixture_dir(IAEA3D))
    }

    /// Loads the reduced fixtures from an explicit directory.
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn load_from<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        let grid = iaea3d_grid()?;

        let k_eff = load_scalar(dir.join("k_eff.csv"))?;

        let residuals_path = dir.join("final_residuals.csv");
        let residuals = load_row(&residuals_path)?;
        if residuals.len() != 2 {
            return Err(fixture_err(
                &residuals_path,
                format!(
                    "expected 2 residuals (fission source, k_eff), found {}",
                    residuals.len()
                ),
            ));
        }

        let radial_power_map = load_matrix(dir.join("radial_power_map.csv"), grid.nx, grid.ny)?;
        let axial_power_profile = load_matrix(dir.join("axial_power_profile.csv"), grid.nz, 1)?;

        Ok(Self {
            grid,
            k_eff,
            fission_source_residual: residuals[0],
            k_eff_residual: residuals[1],
            radial_power_map,
            axial_power_profile,
        })
    }
}

/// The uncommitted full node-level IAEA-3D reference fields.
///
/// Field vectors are all `grid.state_len()` long (10,982 entries: 5,491 nodes
/// × 2 groups) and share the 0-based flat ordering documented on
/// [`Grid::index`]. Loading these is opt-in: they are regenerable, gitignored,
/// and only needed to localise a parity failure to a node.
#[derive(Debug, Clone)]
pub struct Iaea3dFullFields {
    /// The grid the fixtures were captured on.
    pub grid: Grid,
    /// Nodal power density, MATLAB `pwrdens`.
    ///
    /// Units follow the MATLAB, which carries the benchmark's own
    /// normalisation rather than an SI-typed quantity; the reference path
    /// deliberately does not use `uom`, so that its arithmetic stays
    /// line-for-line comparable with the original (see
    /// [`Geometry`](crate::reference::grid::Geometry)).
    pub power_density: Vec<f64>,
    /// Nodal fission source \[-\], MATLAB `fissionsource`.
    pub fission_source: Vec<f64>,
    /// Converged scalar flux \[-\], column 1 of `scalar_flux.csv`.
    pub scalar_flux: Vec<f64>,
    /// The four retained flux iterates, columns 2–5 of `scalar_flux.csv`.
    ///
    /// Kept because the MATLAB's fission-source extrapolation path consumes
    /// them, so a translation of that path can be checked iterate by iterate
    /// rather than only at convergence.
    pub scalar_flux_iterates: Vec<Vec<f64>>,
}

impl Iaea3dFullFields {
    /// Loads the full fields if they are present, returning `Ok(None)` if they
    /// are not.
    ///
    /// `Ok(None)` is the normal outcome in a fresh clone. Callers should report
    /// a skip naming [`REGENERATE_FULL_FIXTURES`] rather than failing.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] only if the files exist but are malformed.
    pub fn try_load() -> Result<Option<Self>> {
        if !full_fixtures_available(IAEA3D) {
            return Ok(None);
        }
        Self::load_from(full_fixture_dir(IAEA3D)).map(Some)
    }

    /// Loads the full fields from an explicit directory, failing if absent.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] if any file is missing, malformed, or does not
    /// match the IAEA-3D grid shape.
    pub fn load_from<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        let grid = iaea3d_grid()?;

        let power_density = load_field(dir.join("power_density.csv"), &grid)?;
        let fission_source = load_field(dir.join("fission_source.csv"), &grid)?;
        let mut flux_columns =
            load_field_columns(dir.join("scalar_flux.csv"), &grid, SCALAR_FLUX_COLUMNS)?;
        let scalar_flux = flux_columns.remove(0);

        Ok(Self {
            grid,
            power_density,
            fission_source,
            scalar_flux,
            scalar_flux_iterates: flux_columns,
        })
    }

    /// Reduces [`Self::power_density`] to the radial map shape of
    /// [`Iaea3dReduced::radial_power_map`].
    ///
    /// # Errors
    ///
    /// As [`radial_power_map`].
    pub fn radial_power_map(&self) -> Result<Vec<f64>> {
        radial_power_map(&self.power_density, &self.grid)
    }

    /// Reduces [`Self::power_density`] to the axial profile shape of
    /// [`Iaea3dReduced::axial_power_profile`].
    ///
    /// # Errors
    ///
    /// As [`axial_power_profile`].
    pub fn axial_power_profile(&self) -> Result<Vec<f64>> {
        axial_power_profile(&self.power_density, &self.grid)
    }
}

/// Collapses a full node-level power field to the committed radial map.
///
/// Sums over `z` and over all energy groups, then normalises so the mean over
/// non-zero entries is exactly 1. Returns `nx * ny` entries, row-major with
/// `ix` as the row index.
///
/// # Why an unweighted sum
///
/// No node-volume weighting is applied, because the capture script does not
/// apply any. That is verified rather than assumed: reducing the captured
/// `power_density.csv` this way reproduces the committed
/// `radial_power_map.csv` to 0.0 absolute difference — bit-exact — and the
/// axial profile to 1.3e-15. The IAEA-3D mesh is uniform, so the distinction
/// would not show up here anyway; a non-uniform case must re-derive it.
///
/// # Errors
///
/// [`BedokError::Fixture`] if `power` is not `grid.state_len()` long, or if
/// every entry is zero (nothing to normalise against).
pub fn radial_power_map(power: &[f64], grid: &Grid) -> Result<Vec<f64>> {
    check_state_len(power, grid, "radial power map")?;
    let mut map = vec![0.0; grid.nx * grid.ny];
    for g in 0..grid.ngroups {
        for ix in 0..grid.nx {
            for iy in 0..grid.ny {
                for iz in 0..grid.nz {
                    map[ix * grid.ny + iy] += power[grid.index(g, ix, iy, iz)];
                }
            }
        }
    }
    normalise_to_unit_mean(map, "radial power map")
}

/// Collapses a full node-level power field to the committed axial profile.
///
/// Sums over `x`, `y` and all energy groups, then normalises so the mean over
/// non-zero entries is exactly 1. Returns `nz` entries indexed by `iz`. See
/// [`radial_power_map`] on the absence of volume weighting.
///
/// # Errors
///
/// [`BedokError::Fixture`] if `power` is not `grid.state_len()` long, or if
/// every entry is zero.
pub fn axial_power_profile(power: &[f64], grid: &Grid) -> Result<Vec<f64>> {
    check_state_len(power, grid, "axial power profile")?;
    let mut profile = vec![0.0; grid.nz];
    for g in 0..grid.ngroups {
        for ix in 0..grid.nx {
            for iy in 0..grid.ny {
                for iz in 0..grid.nz {
                    profile[iz] += power[grid.index(g, ix, iy, iz)];
                }
            }
        }
    }
    normalise_to_unit_mean(profile, "axial power profile")
}

/// Scales `values` so the mean over its non-zero entries is exactly 1.
fn normalise_to_unit_mean(mut values: Vec<f64>, what: &str) -> Result<Vec<f64>> {
    let powered: Vec<f64> = values.iter().copied().filter(|v| *v != 0.0).collect();
    if powered.is_empty() {
        return Err(BedokError::Fixture {
            path: what.to_string(),
            reason: "every entry is zero, nothing to normalise against".to_string(),
        });
    }
    let mean = powered.iter().sum::<f64>() / powered.len() as f64;
    for v in &mut values {
        *v /= mean;
    }
    Ok(values)
}

/// Rejects a field whose length does not match the grid's state vector.
fn check_state_len(values: &[f64], grid: &Grid, what: &str) -> Result<()> {
    if values.len() != grid.state_len() {
        return Err(BedokError::Fixture {
            path: what.to_string(),
            reason: format!(
                "expected a state vector of {} entries, got {}",
                grid.state_len(),
                values.len()
            ),
        });
    }
    Ok(())
}

/// Opens a headerless CSV reader, mapping failures onto [`BedokError::Fixture`].
fn open(path: &Path) -> Result<csv::Reader<std::fs::File>> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .map_err(|e| fixture_err(path, format!("could not open: {e}")))
}

/// Reads a fixture that must hold exactly one row, returning its fields.
fn load_single_row(path: &Path) -> Result<Vec<f64>> {
    let mut reader = open(path)?;
    let mut rows = reader.records();
    let record = match rows.next() {
        Some(record) => record.map_err(|e| fixture_err(path, format!("row 1: {e}")))?,
        None => return Err(fixture_err(path, "file is empty".to_string())),
    };
    if rows.next().is_some() {
        return Err(fixture_err(path, "expected exactly one row".to_string()));
    }

    record
        .iter()
        .enumerate()
        .map(|(col, field)| parse_value(path, 0, col, field))
        .collect()
}

/// Parses a 1-based MATLAB index column.
fn parse_index(path: &Path, row_no: usize, name: &str, field: &str) -> Result<usize> {
    field.trim().parse::<usize>().map_err(|e| {
        fixture_err(
            path,
            format!(
                "row {}: {name} = {field:?} is not an index ({e})",
                row_no + 1
            ),
        )
    })
}

/// Parses a value column.
fn parse_value(path: &Path, row_no: usize, col: usize, field: &str) -> Result<f64> {
    field.trim().parse::<f64>().map_err(|e| {
        fixture_err(
            path,
            format!(
                "row {}, column {}: {field:?} is not a float ({e})",
                row_no + 1,
                col + 1
            ),
        )
    })
}

/// Builds a [`BedokError::Fixture`] carrying the offending path.
fn fixture_err(path: &Path, reason: String) -> BedokError {
    BedokError::Fixture {
        path: path.display().to_string(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_dir_points_inside_the_crate() {
        let dir = fixture_dir(IAEA3D);
        assert!(
            dir.ends_with("tests/fixtures/iaea3d"),
            "got {}",
            dir.display()
        );
    }

    #[test]
    fn iaea3d_grid_has_the_captured_shape() {
        let grid = iaea3d_grid().expect("valid grid");
        assert_eq!(grid.nodes(), 5_491);
        assert_eq!(grid.state_len(), 10_982);
    }

    #[test]
    fn scalar_load_rejects_a_matrix_fixture() {
        // The radial map has 17 columns, not one — the scalar loader must say
        // so rather than silently taking the first field.
        assert!(load_scalar(fixture_dir(IAEA3D).join("radial_power_map.csv")).is_err());
    }

    #[test]
    fn matrix_load_rejects_a_wrong_shape() {
        let path = fixture_dir(IAEA3D).join("radial_power_map.csv");
        assert!(load_matrix(&path, 17, 16).is_err(), "wrong column count");
        assert!(load_matrix(&path, 16, 17).is_err(), "wrong row count");
    }

    #[test]
    fn normalisation_puts_the_mean_of_powered_entries_at_one() {
        let out = normalise_to_unit_mean(vec![0.0, 1.0, 3.0], "test").expect("normalises");
        assert_eq!(out, vec![0.0, 0.5, 1.5]);
    }

    #[test]
    fn reduction_rejects_a_field_of_the_wrong_length() {
        let grid = iaea3d_grid().expect("valid grid");
        assert!(radial_power_map(&[0.0; 10], &grid).is_err());
        assert!(axial_power_profile(&[0.0; 10], &grid).is_err());
    }
}
