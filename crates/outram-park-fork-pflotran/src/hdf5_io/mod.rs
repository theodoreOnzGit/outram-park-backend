//! HDF5 snapshot I/O for structured-grid solution fields (bead op-v6s.15.11).
//!
//! PFLOTRAN writes its solution to **HDF5** — the portable, self-describing binary
//! format visualisation tools (VisIt, ParaView) read via XDMF. This module writes
//! and reads a structured-grid snapshot — the grid geometry plus one or more named
//! cell fields at a given time — using the workspace's **pure-Rust `hdf5-pure`**
//! backend (no system `libhdf5`, no C toolchain), so pflotran stays
//! Android-buildable.
//!
//! # On-disk layout (AI-designed, PFLOTRAN-*inspired*)
//!
//! A snapshot file contains, at the root:
//!
//! - `grid_dimensions` — `i32[3]` = `[nx, ny, nz]`
//! - `x_coords` / `y_coords` / `z_coords` — `f64` cell-centre coordinates per axis
//! - `time` — `f64[1]`, seconds
//! - one `f64` dataset `field_<k>` per cell field (length `nx*ny*nz`, in the
//!   grid's x-fastest cell order), with the field's name carried in the root
//!   string attribute `field_name_<k>` and the count in `n_fields`
//!
//! This is a clean, round-trippable layout, **not** a byte-for-byte reproduction
//! of PFLOTRAN's exact HDF5 schema (its `Coordinates` / `Time:` group naming and
//! the companion `.xmf` descriptor). Reading a real PFLOTRAN output file, and
//! emitting the matching XDMF, are follow-ups — flagged here and in the crate
//! docs.
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft (workspace `RESPONSIBLE_USE.md`). Native-
//! endian `f64`/`i32`; structured grids only; single time level per file. Verified
//! by a write→read round-trip (see the module tests), not against PFLOTRAN gold
//! files.

use hdf5_pure::{AttrValue, File, FileBuilder};

use crate::error::PflotranError;
use crate::grid::CartesianGrid;

/// Map an `hdf5-pure` error into the crate error type.
fn h5<E: core::fmt::Display>(context: &str, e: E) -> PflotranError {
    PflotranError::Io(format!("HDF5 {context}: {e}"))
}

/// A single structured-grid solution snapshot: the grid geometry, a time stamp,
/// and one or more named cell fields.
///
/// Field values are stored per cell in the grid's x-fastest order
/// (`index = i + nx*(j + ny*k)`), matching [`CartesianGrid`]. Every field has
/// length `nx*ny*nz`.
#[derive(Debug, Clone, PartialEq)]
pub struct Hdf5Snapshot {
    /// Grid cell counts `[nx, ny, nz]`.
    pub dims: [usize; 3],
    /// Cell-centre x coordinates, length `nx` (metres).
    pub x: Vec<f64>,
    /// Cell-centre y coordinates, length `ny` (metres).
    pub y: Vec<f64>,
    /// Cell-centre z coordinates, length `nz` (metres).
    pub z: Vec<f64>,
    /// Snapshot time (seconds).
    pub time: f64,
    /// Named cell fields, each of length `nx*ny*nz` (e.g. `("Pressure", …)`).
    pub fields: Vec<(String, Vec<f64>)>,
}

impl Hdf5Snapshot {
    /// Assemble a snapshot from a [`CartesianGrid`], a time, and named cell fields.
    ///
    /// Per-axis cell-centre coordinates are read off the grid. Each field must
    /// have exactly `grid.n_cells()` values.
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if any field length differs from the cell
    /// count.
    pub fn from_grid_fields(
        grid: &CartesianGrid,
        time: f64,
        fields: Vec<(String, Vec<f64>)>,
    ) -> Result<Self, PflotranError> {
        let (nx, ny, nz) = grid.dims();
        let n_cells = grid.n_cells();
        for (name, vals) in &fields {
            if vals.len() != n_cells {
                return Err(PflotranError::InvalidInput(format!(
                    "field '{name}' has {} values but the grid has {n_cells} cells",
                    vals.len()
                )));
            }
        }
        // Per-axis cell-centre coordinates (structured grid: separable per axis).
        let x = (0..nx)
            .map(|i| grid.cell_center(grid.cell_index(i, 0, 0))[0])
            .collect();
        let y = (0..ny)
            .map(|j| grid.cell_center(grid.cell_index(0, j, 0))[1])
            .collect();
        let z = (0..nz)
            .map(|k| grid.cell_center(grid.cell_index(0, 0, k))[2])
            .collect();
        Ok(Hdf5Snapshot {
            dims: [nx, ny, nz],
            x,
            y,
            z,
            time,
            fields,
        })
    }

    /// Serialise this snapshot to in-memory HDF5 bytes.
    ///
    /// # Errors
    /// [`PflotranError::Io`] if the HDF5 writer fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PflotranError> {
        let mut fb = FileBuilder::new();
        let dims_i32 = [
            self.dims[0] as i32,
            self.dims[1] as i32,
            self.dims[2] as i32,
        ];
        fb.create_dataset("grid_dimensions")
            .with_i32_data(&dims_i32)
            .with_shape(&[3]);
        fb.create_dataset("x_coords")
            .with_f64_data(&self.x)
            .with_shape(&[self.x.len() as u64]);
        fb.create_dataset("y_coords")
            .with_f64_data(&self.y)
            .with_shape(&[self.y.len() as u64]);
        fb.create_dataset("z_coords")
            .with_f64_data(&self.z)
            .with_shape(&[self.z.len() as u64]);
        fb.create_dataset("time")
            .with_f64_data(&[self.time])
            .with_shape(&[1]);
        // Fields as flat `field_<k>` datasets; their names travel as root string
        // attributes `field_name_<k>`, with `n_fields` recording the count. This
        // avoids relying on group nesting / vlen-string datasets and preserves
        // field order across the round-trip.
        for (k, (name, vals)) in self.fields.iter().enumerate() {
            fb.create_dataset(&format!("field_{k}"))
                .with_f64_data(vals)
                .with_shape(&[vals.len() as u64]);
            fb.set_attr(&format!("field_name_{k}"), AttrValue::String(name.clone()));
        }
        fb.set_attr("n_fields", AttrValue::I32(self.fields.len() as i32));
        fb.finish().map_err(|e| h5("write", e))
    }

    /// Write this snapshot to an HDF5 file at `path`.
    ///
    /// # Errors
    /// [`PflotranError::Io`] if the file cannot be written.
    pub fn write<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), PflotranError> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes).map_err(|e| h5("file write", e))
    }

    /// Parse a snapshot from in-memory HDF5 bytes.
    ///
    /// # Errors
    /// [`PflotranError::Io`] if the bytes are not a readable snapshot (missing
    /// dataset, wrong shape, …).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PflotranError> {
        let file = File::from_bytes(bytes).map_err(|e| h5("open", e))?;

        let dims_i32 = file
            .dataset("grid_dimensions")
            .and_then(|d| d.read_i32())
            .map_err(|e| h5("grid_dimensions", e))?;
        if dims_i32.len() != 3 {
            return Err(h5("grid_dimensions", "expected 3 entries"));
        }
        let dims = [
            dims_i32[0] as usize,
            dims_i32[1] as usize,
            dims_i32[2] as usize,
        ];

        let read_axis = |name: &str| -> Result<Vec<f64>, PflotranError> {
            file.dataset(name)
                .and_then(|d| d.read_f64())
                .map_err(|e| h5(name, e))
        };
        let x = read_axis("x_coords")?;
        let y = read_axis("y_coords")?;
        let z = read_axis("z_coords")?;

        let time = file
            .dataset("time")
            .and_then(|d| d.read_f64())
            .map_err(|e| h5("time", e))?
            .first()
            .copied()
            .ok_or_else(|| h5("time", "empty"))?;

        // Fields: count + names from root attributes, data from `field_<k>`.
        let attrs = file.root().attrs().map_err(|e| h5("root attrs", e))?;
        // Scalar int attributes may be read back widened (I64); accept either.
        let n_fields = match attrs.get("n_fields") {
            Some(AttrValue::I32(n)) if *n >= 0 => *n as usize,
            Some(AttrValue::I64(n)) if *n >= 0 => *n as usize,
            _ => return Err(h5("n_fields", "missing or not a non-negative integer")),
        };
        let mut fields = Vec::with_capacity(n_fields);
        for k in 0..n_fields {
            let name = match attrs.get(&format!("field_name_{k}")) {
                Some(AttrValue::String(s)) | Some(AttrValue::AsciiString(s)) => s.clone(),
                _ => return Err(h5(&format!("field_name_{k}"), "missing or not a string")),
            };
            let vals = file
                .dataset(&format!("field_{k}"))
                .and_then(|d| d.read_f64())
                .map_err(|e| h5(&format!("field_{k} ('{name}')"), e))?;
            fields.push((name, vals));
        }

        Ok(Hdf5Snapshot {
            dims,
            x,
            y,
            z,
            time,
            fields,
        })
    }

    /// Read a snapshot from an HDF5 file at `path`.
    ///
    /// # Errors
    /// [`PflotranError::Io`] if the file cannot be read or parsed.
    pub fn read<P: AsRef<std::path::Path>>(path: P) -> Result<Self, PflotranError> {
        let bytes = std::fs::read(path).map_err(|e| h5("file read", e))?;
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    fn sample() -> Hdf5Snapshot {
        Hdf5Snapshot {
            dims: [3, 2, 1],
            x: vec![0.5, 1.5, 2.5],
            y: vec![0.25, 0.75],
            z: vec![0.5],
            time: 86_400.0,
            fields: vec![
                (
                    "Pressure".into(),
                    vec![1e5, 1.1e5, 1.2e5, 1.3e5, 1.4e5, 1.5e5],
                ),
                (
                    "Temperature".into(),
                    vec![300.0, 301.0, 302.0, 303.0, 304.0, 305.0],
                ),
            ],
        }
    }

    #[test]
    fn snapshot_round_trips_through_hdf5_bytes() {
        let snap = sample();
        let bytes = snap.to_bytes().unwrap();
        // Real HDF5 files begin with the signature \x89HDF\r\n\x1a\n.
        assert_eq!(&bytes[0..8], b"\x89HDF\r\n\x1a\n");
        let back = Hdf5Snapshot::from_bytes(bytes).unwrap();
        assert_eq!(back.dims, snap.dims);
        assert_eq!(back.x, snap.x);
        assert_eq!(back.y, snap.y);
        assert_eq!(back.z, snap.z);
        assert_eq!(back.time, snap.time);
        // Fields compared order-independently (HDF5 link order is not guaranteed).
        for (name, vals) in &snap.fields {
            let got = back
                .fields
                .iter()
                .find(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("field {name} missing after round-trip"));
            assert_eq!(&got.1, vals);
        }
    }

    #[test]
    fn round_trips_through_a_real_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("outram_pflotran_hdf5_test.h5");
        let snap = sample();
        snap.write(&path).unwrap();
        let back = Hdf5Snapshot::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back.dims, snap.dims);
        assert_eq!(back.time, snap.time);
        assert_eq!(back.fields.len(), 2);
    }

    #[test]
    fn from_grid_fields_extracts_axes_and_validates_lengths() {
        // 4x2x1 uniform grid, 1 m x 1 m x 1 m cells.
        let grid = CartesianGrid::uniform(4, 2, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let n = grid.n_cells();
        let pressure: Vec<f64> = (0..n).map(|c| c as f64).collect();
        let snap = Hdf5Snapshot::from_grid_fields(&grid, 10.0, vec![("Pressure".into(), pressure)])
            .unwrap();
        assert_eq!(snap.dims, [4, 2, 1]);
        assert_eq!(snap.x.len(), 4);
        assert_eq!(snap.y.len(), 2);
        // Cell centres of a unit grid are at 0.5, 1.5, ...
        assert!((snap.x[0] - 0.5).abs() < 1e-12);
        assert!((snap.x[3] - 3.5).abs() < 1e-12);

        // A wrong-length field is rejected.
        let bad = Hdf5Snapshot::from_grid_fields(&grid, 0.0, vec![("T".into(), vec![1.0, 2.0])]);
        assert!(matches!(bad, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn round_trip_of_a_grid_snapshot_preserves_fields() {
        let grid = CartesianGrid::uniform(5, 1, 1, m(2.0), m(1.0), m(1.0)).unwrap();
        let n = grid.n_cells();
        let t: Vec<f64> = (0..n).map(|c| 300.0 + c as f64).collect();
        let snap =
            Hdf5Snapshot::from_grid_fields(&grid, 3600.0, vec![("Temperature".into(), t.clone())])
                .unwrap();
        let back = Hdf5Snapshot::from_bytes(snap.to_bytes().unwrap()).unwrap();
        assert_eq!(back.fields[0].0, "Temperature");
        assert_eq!(back.fields[0].1, t);
    }
}
