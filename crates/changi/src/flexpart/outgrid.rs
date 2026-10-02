// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/outgrid_init.f90 (subroutine outgrid_init),
//                    src/outgrid_init_nest.f90 (subroutine outgrid_init_nest)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Output-grid set-up (`outgrid_init.f90`, `outgrid_init_nest.f90`): the
//! surface area, volume and wall areas of every output cell, and the mean
//! model topography under each cell.
//!
//! # What is ported, and what is not
//!
//! Both routines do three things. The port carries the **numerics** of the
//! first two and leaves the third to the caller:
//!
//! 1. **Cell geometry** ([`cell_geometry`]): `area`, `volume`, `areaeast`,
//!    `areanorth` (`outgrid_init`) or `arean`, `volumen` (`outgrid_init_nest`).
//!    The nest routine's area and volume code is the mother routine's,
//!    textually, on the `*n` variables; it computes no wall areas (the nest
//!    has no flux output). One Rust function serves both; for a nest, ignore
//!    [`CellGeometry::areaeast`] / [`CellGeometry::areanorth`].
//! 2. **Mean topography** ([`output_orography`]): `oroout` / `orooutn`, the
//!    average of 100 bilinear samples of `oro` (or of the innermost nest's
//!    `oron`) on a 10 x 10 lattice inside each output cell.
//! 3. **Allocation and zeroing** of `gridunc`, `wetgridunc`, `drygridunc`,
//!    `flux`, `init_cond`, `creceptor` and the `concoutput` work arrays. Not
//!    ported: the Rust accumulators ([`ConcentrationGrid::zeros`],
//!    [`DepositionGrid::zeros`], [`FluxGrid::zeros`]) are created zeroed.
//!
//! [`ConcentrationGrid::zeros`]: crate::flexpart::concentration::ConcentrationGrid::zeros
//! [`DepositionGrid::zeros`]: crate::flexpart::concentration::DepositionGrid::zeros
//! [`FluxGrid::zeros`]: crate::flexpart::fluxes::FluxGrid::zeros
//!
//! # The cell area
//!
//! Upstream uses the area of a spherical zone, `M = 2 pi R h dx / 360`
//! (Netz, *Formeln der Mathematik*, 5th ed. 1983, p. 90), where `h` is the
//! zone height `R |sin(lat_n) - sin(lat_s)|`. It evaluates `|sin|` as
//! `sqrt(1 - cos^2)` and picks the order of the difference from the cosines,
//! so the same expression serves both hemispheres. A cell that **straddles
//! the equator** (`lat_s < 0 < lat_n`, strictly) takes a different formula,
//! `h = dy R pi/180` — the arc length, not the zone height. That is a planar
//! approximation of the zone; for a 2.5° cell it overestimates the exact zone
//! area by 8e-5 relative.
//!
//! # Upstream quirks reproduced (or refused)
//!
//! * **`sqrt(1 - cos^2)` near the equator.** For a cell boundary at small
//!   `|lat|`, `1 - cos^2` cancels: at 0.25° it is `1.9e-5`, so `real(4)`
//!   loses about four of its seven digits there. The shipped build's areas of
//!   near-equator cells are correspondingly imprecise (the code-to-code test
//!   measures how much); the port, in `f64`, keeps about twelve digits.
//! * **Cells beyond a pole** are not guarded: `cos` changes sign past ±90°
//!   and `sqrt(1 - cos^2)` returns `|sin|`, so the area is wrong. Upstream's
//!   `readoutgrid` does not prevent such a grid; neither does the port.
//! * **`areanorth`** uses `cos` of the cell **centre** latitude, i.e. it is the
//!   area of a wall through the cell's middle, not of its northern face; and
//!   `areaeast` is the same for every cell. Both are reproduced as written
//!   (and match how [`calcfluxes`] counts crossings of the cell centre
//!   lines).
//! * **Topography samples** take the cell with `int()` (truncation toward
//!   zero) and have **no bounds check**: a sample left of or below the
//!   meteorological grid, or whose `ix + 1`/`jy + 1` corner falls off the
//!   supplied array, reads whatever memory lies there. The port refuses
//!   ([`OutgridError::OrographyOutsideGrid`]); a sample in `(-1, 0)` truncates
//!   to cell 0 with a negative weight, as upstream, and is accepted.
//! * **Nest selection** for the samples is FLEXPART's usual one: the
//!   highest-numbered nest whose rectangle contains the sample with margin
//!   `eps = nxmax/3e5` (in mother-grid units) on all four sides.
//! * **`outgrid_init` zeroes only `flux(1:5, ...)`**: the loop is
//!   `do i=1,5`, so the downward component `flux(6, ...)` starts from
//!   whatever `allocate` returned until the first `fluxoutput` resets all six.
//!   Allocation is not ported; [`FluxGrid::zeros`] zeroes all six.
//!
//! # Units and indices
//!
//! Degrees for `outlon0`, `outlat0`, `dxout`, `dyout` and the
//! meteorological grid (`xlon0`, `ylat0`, `dx`, `dy`); metres for heights,
//! m² for areas, m³ for volumes; topography in m. Output columns `ix`, rows
//! `jy` and levels `kz` are **0-based** (upstream's levels are 1-based).

use crate::flexpart::constants::{PI, PI180, R_EARTH};
use crate::flexpart::met_fields::Field2;
use crate::flexpart::particle_average::GridGeometry;
use crate::flexpart::wet_deposition::NestFrame;

#[cfg(doc)]
use crate::flexpart::fluxes::{calcfluxes, FluxGrid};

/// Placement of one output grid: `outlon0`, `outlat0`, `dxout`, `dyout`,
/// `numxgrid`, `numygrid` (or the nest's `*n` counterparts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputGrid {
    /// Longitude of the western edge of column 0, degrees.
    pub outlon0: f64,
    /// Latitude of the southern edge of row 0, degrees.
    pub outlat0: f64,
    /// Cell width, degrees.
    pub dxout: f64,
    /// Cell height, degrees.
    pub dyout: f64,
    /// Number of columns.
    pub numxgrid: usize,
    /// Number of rows.
    pub numygrid: usize,
}

/// Geometry of every output cell. 2-D arrays are `[jy*numxgrid + ix]`, 3-D
/// arrays `[(kz*numygrid + jy)*numxgrid + ix]` (`ix` fastest, `kz` 0-based).
#[derive(Debug, Clone, PartialEq)]
pub struct CellGeometry {
    /// Columns.
    pub numxgrid: usize,
    /// Rows.
    pub numygrid: usize,
    /// Levels.
    pub numzgrid: usize,
    /// Horizontal cell area, m² (`area` / `arean`).
    pub area: Vec<f64>,
    /// Cell volume, m³ (`volume` / `volumen`).
    pub volume: Vec<f64>,
    /// Area of the cell's east-facing wall, m² (`areaeast`; mother grid
    /// only upstream).
    pub areaeast: Vec<f64>,
    /// Area of the cell's north-facing wall, m² (`areanorth`; mother grid
    /// only upstream).
    pub areanorth: Vec<f64>,
}

impl CellGeometry {
    /// Index into [`CellGeometry::area`].
    #[must_use]
    pub fn idx2(&self, ix: usize, jy: usize) -> usize {
        jy * self.numxgrid + ix
    }

    /// Index into the 3-D arrays; `kz` 0-based.
    #[must_use]
    pub fn idx3(&self, ix: usize, jy: usize, kz: usize) -> usize {
        (kz * self.numygrid + jy) * self.numxgrid + ix
    }
}

/// The zone height `hzone` of output row `jy`, m: `outgrid_init.f90:51-75`.
///
/// `R |sin(lat_n) - sin(lat_s)|` evaluated as upstream does, or the arc
/// length `dyout R pi/180` for a row straddling the equator (see the module
/// doc).
#[must_use]
pub fn zone_height(outlat0: f64, dyout: f64, jy: usize) -> f64 {
    let ylat = outlat0 + (jy as f64 + 0.5) * dyout;
    let ylatp = ylat + 0.5 * dyout;
    let ylatm = ylat - 0.5 * dyout;
    if ylatm < 0.0 && ylatp > 0.0 {
        dyout * R_EARTH * PI180
    } else {
        let cosfactp = (ylatp * PI180).cos();
        let cosfactm = (ylatm * PI180).cos();
        // `cosfact**2` is an integer power: a product.
        let hzone = if cosfactp < cosfactm {
            (1.0 - cosfactp * cosfactp).sqrt() - (1.0 - cosfactm * cosfactm).sqrt()
        } else {
            (1.0 - cosfactm * cosfactm).sqrt() - (1.0 - cosfactp * cosfactp).sqrt()
        };
        hzone * R_EARTH
    }
}

/// Surface area of a cell in output row `jy`, m²:
/// `gridarea = 2*pi*r_earth*hzone*dxout/360` (`outgrid_init.f90:80`).
#[must_use]
pub fn cell_area(outlat0: f64, dyout: f64, dxout: f64, jy: usize) -> f64 {
    let hzone = zone_height(outlat0, dyout, jy);
    2.0 * PI * R_EARTH * hzone * dxout / 360.0
}

/// `outgrid_init.f90:50-102` / `outgrid_init_nest.f90:87-127`: area, volume
/// and wall areas of every cell of `grid`, with level tops `outheight`
/// (m, increasing; `numzgrid = outheight.len()`).
///
/// Level 0 spans the ground to `outheight[0]`; level `kz` spans
/// `outheight[kz-1]` to `outheight[kz]`.
#[must_use]
pub fn cell_geometry(grid: &OutputGrid, outheight: &[f64]) -> CellGeometry {
    let (nx, ny, nz) = (grid.numxgrid, grid.numygrid, outheight.len());
    let mut g = CellGeometry {
        numxgrid: nx,
        numygrid: ny,
        numzgrid: nz,
        area: vec![0.0; nx * ny],
        volume: vec![0.0; nx * ny * nz],
        areaeast: vec![0.0; nx * ny * nz],
        areanorth: vec![0.0; nx * ny * nz],
    };
    for jy in 0..ny {
        let ylat = grid.outlat0 + (jy as f64 + 0.5) * grid.dyout;
        let gridarea = cell_area(grid.outlat0, grid.dyout, grid.dxout, jy);
        for ix in 0..nx {
            let i2 = g.idx2(ix, jy);
            g.area[i2] = gridarea;
            for kz in 0..nz {
                let dh = if kz == 0 {
                    outheight[0]
                } else {
                    outheight[kz] - outheight[kz - 1]
                };
                let i3 = g.idx3(ix, jy, kz);
                g.volume[i3] = g.area[i2] * dh;
                g.areaeast[i3] = grid.dyout * R_EARTH * PI180 * dh;
                g.areanorth[i3] = (ylat * PI180).cos() * grid.dxout * R_EARTH * PI180 * dh;
            }
        }
    }
    g
}

/// One nested meteorological grid's topography: its placement and `oron`
/// (`oron(0:nxmaxn-1, 0:nymaxn-1, n)` as a [`Field2`]).
#[derive(Debug, Clone, PartialEq)]
pub struct NestOrography {
    /// Placement in mother-grid units (`xln`, `xrn`, `yln`, `yrn`) and
    /// refinement (`xresoln`, `yresoln`).
    pub frame: NestFrame,
    /// Topography on the nest grid, m.
    pub oron: Field2,
}

/// Why [`output_orography`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutgridError {
    /// A topography sample of output cell `(ix, jy)` has a bilinear corner
    /// outside the supplied `oro` / `oron` array (upstream reads out of
    /// bounds).
    OrographyOutsideGrid {
        /// Output column.
        ix: usize,
        /// Output row.
        jy: usize,
    },
}

/// `eps = nxmax/3.e5`, the nest-selection margin of `outgrid_init*.f90`
/// (and of `advance.f90`), mother-grid units.
#[must_use]
pub fn nest_margin(nxmax: usize) -> f64 {
    nxmax as f64 / 3.0e5
}

/// `outgrid_init.f90:110-180` / `outgrid_init_nest.f90:134-204`: the mean
/// topography `oroout` of every cell of `grid`, `[jy*numxgrid + ix]`, m.
///
/// `met` places the mother grid (`xlon0`, `ylat0`, `dx`, `dy`), `oro` is its
/// topography (`oro(0:nxmax-1, 0:nymax-1)`), `nests` the nested grids in
/// upstream's order (nest `j` is `nests[j-1]`), `eps` the selection margin
/// ([`nest_margin`]).
///
/// # Errors
///
/// [`OutgridError::OrographyOutsideGrid`] where upstream would read outside
/// the arrays; nothing is returned then.
pub fn output_orography(
    grid: &OutputGrid,
    met: &GridGeometry,
    oro: &Field2,
    nests: &[NestOrography],
    eps: f64,
) -> Result<Vec<f64>, OutgridError> {
    let (nx, ny) = (grid.numxgrid, grid.numygrid);
    let mut out = vec![0.0; nx * ny];
    for jjy in 0..ny {
        for iix in 0..nx {
            let refuse = OutgridError::OrographyOutsideGrid { ix: iix, jy: jjy };
            let mut oroh = 0.0;
            for j1 in 1..=10 {
                let ylat = grid.outlat0 + (jjy as f64 + j1 as f64 / 10.0 - 0.05) * grid.dyout;
                let yl = (ylat - met.ylat0) / met.dy;
                for i1 in 1..=10 {
                    let xlon = grid.outlon0 + (iix as f64 + i1 as f64 / 10.0 - 0.05) * grid.dxout;
                    let xl = (xlon - met.xlon0) / met.dx;

                    // Highest-numbered nest containing the sample, with margin.
                    let ngrid = (0..nests.len()).rev().find(|&j| {
                        let f = &nests[j].frame;
                        xl > f.xl + eps && xl < f.xr - eps && yl > f.yl + eps && yl < f.yr - eps
                    });

                    let (xs, ys, field) = match ngrid {
                        Some(j) => {
                            let f = &nests[j].frame;
                            ((xl - f.xl) * f.xresol, (yl - f.yl) * f.yresol, &nests[j].oron)
                        }
                        None => (xl, yl, oro),
                    };
                    // Fortran int(): truncation toward zero.
                    let (ixt, jyt) = (xs.trunc(), ys.trunc());
                    if !(ixt >= 0.0 && jyt >= 0.0) {
                        return Err(refuse);
                    }
                    let ix = ixt as usize;
                    let jy = jyt as usize;
                    let ddy = ys - jy as f64;
                    let ddx = xs - ix as f64;
                    let ixp = ix + 1;
                    let jyp = jy + 1;
                    if ixp >= field.nx || jyp >= field.ny {
                        return Err(refuse);
                    }
                    let rddx = 1.0 - ddx;
                    let rddy = 1.0 - ddy;
                    let p1 = rddx * rddy;
                    let p2 = ddx * rddy;
                    let p3 = rddx * ddy;
                    let p4 = ddx * ddy;
                    // `oroh = oroh + p1*o(ix,jy) + p2*o(ixp,jy) + ...`, left to
                    // right: the running sum is the first operand.
                    oroh = oroh
                        + p1 * field.at(ix, jy)
                        + p2 * field.at(ixp, jy)
                        + p3 * field.at(ix, jyp)
                        + p4 * field.at(ixp, jyp);
                }
            }
            out[jjy * nx + iix] = oroh / 100.0;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The zone formula integrates to the sphere's area over a global grid,
    /// up to the arc-length approximation of the straddling row.
    #[test]
    fn global_grid_area_is_the_sphere() {
        // Rows at multiples of 15°: none straddles the equator.
        let g = OutputGrid {
            outlon0: -180.0,
            outlat0: -90.0,
            dxout: 30.0,
            dyout: 15.0,
            numxgrid: 12,
            numygrid: 12,
        };
        let geo = cell_geometry(&g, &[100.0]);
        let total: f64 = geo.area.iter().sum();
        let sphere = 4.0 * PI * R_EARTH * R_EARTH;
        assert!((total / sphere - 1.0).abs() < 1e-12, "{total} vs {sphere}");
    }

    #[test]
    fn straddling_row_uses_the_arc_length() {
        let h = zone_height(-1.25, 2.5, 0);
        assert_eq!(h, 2.5 * R_EARTH * PI180);
        // The exact zone height is smaller by about (pi/180 * 1.25)^2 / 6.
        let exact = 2.0 * (1.25 * PI180).sin() * R_EARTH;
        assert!(h > exact && (h / exact - 1.0) < 1e-4);
    }

    #[test]
    fn sample_off_the_grid_is_refused() {
        let g = OutputGrid {
            // Samples at xl = -1.45 .. -0.55: the first truncates to -1.
            outlon0: -1.5,
            outlat0: 0.0,
            dxout: 1.0,
            dyout: 1.0,
            numxgrid: 1,
            numygrid: 1,
        };
        let met = GridGeometry {
            xlon0: 0.0,
            ylat0: 0.0,
            dx: 1.0,
            dy: 1.0,
        };
        let oro = Field2::filled(4, 4, 1.0);
        assert_eq!(
            output_orography(&g, &met, &oro, &[], 1e-3),
            Err(OutgridError::OrographyOutsideGrid { ix: 0, jy: 0 })
        );
    }
}
