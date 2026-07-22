//! Output writers for the v1 RICHARDS slice: cell-centred CSV and legacy-ASCII
//! VTK `STRUCTURED_POINTS`.
//!
//! Both writers take **primitive slices** (`&[f64]`) rather than any grid or
//! field type from the other modules, so they stay decoupled from the physics
//! side and are trivially testable without a filesystem. Each returns a
//! `String`; the driver is responsible for writing it to disk.
//!
//! # Cell ordering
//!
//! Cells are indexed **x-fastest**: for logical indices `(i, j, k)` the flat
//! index is `index = i + nx * (j + ny * k)`. Both `pressure` and `saturation`
//! must be laid out this way and have length `nx * ny * nz`.
//!
//! # Units
//!
//! Lengths (`dx`/`dy`/`dz` and emitted coordinates) are in metres, `pressure`
//! in pascals (Pa), and `saturation` is dimensionless (0..1).

use core::fmt::Write as _;

use crate::error::PflotranError;

/// Validate the common preconditions of both writers.
///
/// Ensures the grid counts are all `> 0` and both value slices have length
/// exactly `nx * ny * nz`.
fn check_dims(
    nx: usize,
    ny: usize,
    nz: usize,
    pressure: &[f64],
    saturation: &[f64],
) -> Result<usize, PflotranError> {
    if nx == 0 || ny == 0 || nz == 0 {
        return Err(PflotranError::InvalidInput(format!(
            "grid dimensions must all be > 0, got {}x{}x{}",
            nx, ny, nz
        )));
    }
    let n = nx * ny * nz;
    if pressure.len() != n {
        return Err(PflotranError::InvalidInput(format!(
            "pressure slice has {} entries, expected nx*ny*nz = {}",
            pressure.len(),
            n
        )));
    }
    if saturation.len() != n {
        return Err(PflotranError::InvalidInput(format!(
            "saturation slice has {} entries, expected nx*ny*nz = {}",
            saturation.len(),
            n
        )));
    }
    Ok(n)
}

/// Write cell-centred results as CSV with the header
/// `x,y,z,pressure,saturation`.
///
/// One data row per cell in x-fastest order (`index = i + nx*(j + ny*k)`). The
/// emitted `x`/`y`/`z` are **cell-centre** coordinates in metres:
/// `x = (i + 0.5) * dx`, and likewise for `y`, `z`. `pressure` is in pascals,
/// `saturation` is dimensionless. The output has `nx*ny*nz + 1` lines (one
/// header + one per cell) and a trailing newline.
///
/// # Errors
///
/// Returns [`PflotranError::InvalidInput`] if any grid count is `0` or either
/// slice length differs from `nx*ny*nz`.
pub fn write_results_csv(
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    dy: f64,
    dz: f64,
    pressure: &[f64],
    saturation: &[f64],
) -> Result<String, PflotranError> {
    check_dims(nx, ny, nz, pressure, saturation)?;

    let mut out = String::new();
    out.push_str("x,y,z,pressure,saturation\n");
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = i + nx * (j + ny * k);
                let x = (i as f64 + 0.5) * dx;
                let y = (j as f64 + 0.5) * dy;
                let z = (k as f64 + 0.5) * dz;
                // `write!` to a String is infallible; unwrap is safe.
                writeln!(
                    out,
                    "{},{},{},{},{}",
                    x, y, z, pressure[idx], saturation[idx]
                )
                .expect("writing to a String cannot fail");
            }
        }
    }
    Ok(out)
}

/// Write a legacy-ASCII VTK `STRUCTURED_POINTS` dataset with `pressure` and
/// `saturation` attached as **cell data**.
///
/// The grid holds `nx*ny*nz` cells, so `DIMENSIONS` (a point count) is
/// `(nx+1, ny+1, nz+1)`, `SPACING` is `dx dy dz` (metres), and `ORIGIN` is
/// `0 0 0`. Cell values are listed x-fastest, matching
/// `index = i + nx*(j + ny*k)`, under `CELL_DATA nx*ny*nz`. Two scalar fields
/// are emitted: `pressure` (Pa) and `saturation` (dimensionless), each with a
/// `LOOKUP_TABLE default`.
///
/// # Errors
///
/// Returns [`PflotranError::InvalidInput`] if any grid count is `0` or either
/// slice length differs from `nx*ny*nz`.
pub fn write_vtk_structured(
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    dy: f64,
    dz: f64,
    pressure: &[f64],
    saturation: &[f64],
) -> Result<String, PflotranError> {
    let n = check_dims(nx, ny, nz, pressure, saturation)?;

    let mut out = String::new();
    out.push_str("# vtk DataFile Version 3.0\n");
    out.push_str("outram-park-fork-pflotran v1 RICHARDS results\n");
    out.push_str("ASCII\n");
    out.push_str("DATASET STRUCTURED_POINTS\n");
    // DIMENSIONS counts points = cells + 1 in each direction.
    writeln!(out, "DIMENSIONS {} {} {}", nx + 1, ny + 1, nz + 1)
        .expect("writing to a String cannot fail");
    writeln!(out, "SPACING {} {} {}", dx, dy, dz).expect("writing to a String cannot fail");
    out.push_str("ORIGIN 0 0 0\n");
    writeln!(out, "CELL_DATA {}", n).expect("writing to a String cannot fail");

    out.push_str("SCALARS pressure double 1\n");
    out.push_str("LOOKUP_TABLE default\n");
    for &p in pressure {
        writeln!(out, "{}", p).expect("writing to a String cannot fail");
    }

    out.push_str("SCALARS saturation double 1\n");
    out.push_str("LOOKUP_TABLE default\n");
    for &s in saturation {
        writeln!(out, "{}", s).expect("writing to a String cannot fail");
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_has_header_plus_one_row_per_cell() {
        let nx = 3;
        let ny = 2;
        let nz = 2;
        let n = nx * ny * nz;
        let pressure: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let saturation: Vec<f64> = (0..n).map(|i| 0.1 * i as f64).collect();

        let csv = write_results_csv(nx, ny, nz, 0.5, 1.0, 2.0, &pressure, &saturation).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), n + 1);
        assert_eq!(lines[0], "x,y,z,pressure,saturation");
    }

    #[test]
    fn csv_cell_centre_coordinates_are_half_spacing() {
        // dx = 0.5 keeps centres exactly representable in f64 (0.25, 0.75).
        let csv = write_results_csv(2, 1, 1, 0.5, 1.0, 1.0, &[1.0, 2.0], &[0.3, 0.4]).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        // i=0 -> x = 0.25, y = 0.5, z = 0.5; pressure 1, saturation 0.3
        assert_eq!(lines[1], "0.25,0.5,0.5,1,0.3");
        // i=1 -> x = 0.75
        assert_eq!(lines[2], "0.75,0.5,0.5,2,0.4");
    }

    #[test]
    fn csv_uses_x_fastest_ordering() {
        // 2x2x1 grid: index = i + 2*j. Encode index into pressure to check order.
        let nx = 2;
        let ny = 2;
        let nz = 1;
        let pressure = vec![0.0, 1.0, 2.0, 3.0]; // idx 0..3
        let saturation = vec![0.0; 4];
        let csv = write_results_csv(nx, ny, nz, 1.0, 1.0, 1.0, &pressure, &saturation).unwrap();
        let rows: Vec<&str> = csv.lines().skip(1).collect();
        // Row order: (i,j) = (0,0),(1,0),(0,1),(1,1) -> pressures 0,1,2,3
        assert!(rows[0].contains(",0,"));
        // centre of cell (0,0): x=0.5,y=0.5 ; pressure 0
        assert_eq!(rows[0], "0.5,0.5,0.5,0,0");
        // cell (0,1): y = 1.5, pressure index 2
        assert_eq!(rows[2], "0.5,1.5,0.5,2,0");
    }

    #[test]
    fn vtk_header_is_well_formed() {
        let nx = 4;
        let ny = 3;
        let nz = 2;
        let n = nx * ny * nz;
        let pressure = vec![101325.0; n];
        let saturation = vec![0.8; n];
        let vtk = write_vtk_structured(nx, ny, nz, 0.25, 0.5, 1.0, &pressure, &saturation).unwrap();

        assert!(vtk.starts_with("# vtk DataFile Version 3.0\n"));
        assert!(vtk.contains("ASCII\n"));
        assert!(vtk.contains("DATASET STRUCTURED_POINTS\n"));
        assert!(vtk.contains("DIMENSIONS 5 4 3\n")); // (nx+1, ny+1, nz+1)
        assert!(vtk.contains("SPACING 0.25 0.5 1\n"));
        assert!(vtk.contains("ORIGIN 0 0 0\n"));
        assert!(vtk.contains(&format!("CELL_DATA {}\n", n)));
        assert!(vtk.contains("SCALARS pressure double 1\n"));
        assert!(vtk.contains("SCALARS saturation double 1\n"));
        assert!(vtk.contains("LOOKUP_TABLE default\n"));
    }

    #[test]
    fn vtk_emits_one_value_line_per_cell_per_field() {
        let n = 6;
        let pressure = vec![1.0; n];
        let saturation = vec![0.5; n];
        let vtk = write_vtk_structured(3, 2, 1, 1.0, 1.0, 1.0, &pressure, &saturation).unwrap();
        let ones = vtk.lines().filter(|l| *l == "1").count();
        let halves = vtk.lines().filter(|l| *l == "0.5").count();
        assert_eq!(ones, n);
        assert_eq!(halves, n);
    }

    #[test]
    fn writers_reject_mismatched_slice_length() {
        let e = write_results_csv(2, 2, 1, 1.0, 1.0, 1.0, &[1.0, 2.0], &[0.0; 4]).unwrap_err();
        assert!(format!("{}", e).contains("pressure slice"));

        let e = write_vtk_structured(2, 2, 1, 1.0, 1.0, 1.0, &[0.0; 4], &[0.0; 3]).unwrap_err();
        assert!(format!("{}", e).contains("saturation slice"));
    }

    #[test]
    fn writers_reject_zero_dimension() {
        let e = write_results_csv(0, 1, 1, 1.0, 1.0, 1.0, &[], &[]).unwrap_err();
        assert!(format!("{}", e).contains("must all be > 0"));
    }
}
