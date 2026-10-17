// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Drawing the voxel mesh the solver saw, and its temperatures (the
//! "draw what the solver sees" rule of this crate's `CLAUDE.md`).

use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

use super::ThermalProfile;
use crate::geometry::plot::{ImageData, Rgb};

/// A slice through the voxel mesh, at the voxel layer containing the given
/// coordinate (cm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlicePlane {
    /// An x-y slice at height `z`.
    Xy {
        /// Height, cm.
        z: f64,
    },
    /// An x-z slice at `y`.
    Xz {
        /// Coordinate, cm.
        y: f64,
    },
}

/// What a slice image is coloured by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceColour {
    /// Each conducting region (zone) in its own colour; the outside grey.
    Region,
    /// Temperature, blue at the coolant or fixed-surface temperature to red
    /// at the peak; the outside grey.
    Temperature,
}

const OUTSIDE: Rgb = Rgb {
    r: 220,
    g: 220,
    b: 220,
};
const ZONE_COLOURS: [Rgb; 6] = [
    Rgb {
        r: 214,
        g: 96,
        b: 77,
    },
    Rgb {
        r: 67,
        g: 147,
        b: 195,
    },
    Rgb {
        r: 120,
        g: 180,
        b: 90,
    },
    Rgb {
        r: 240,
        g: 190,
        b: 60,
    },
    Rgb {
        r: 150,
        g: 110,
        b: 190,
    },
    Rgb {
        r: 90,
        g: 90,
        b: 90,
    },
];

impl ThermalProfile {
    /// Draw one voxel layer, `scale` pixels per voxel, row 0 at the top
    /// (largest y or z). Each pixel is the voxel the solver used: zone or
    /// solved temperature.
    #[must_use]
    pub fn render_slice(&self, plane: SlicePlane, colour: SliceColour, scale: usize) -> ImageData {
        let grid = self.mesh.grid;
        let h = grid.spacing.get::<centimeter>();
        let scale = scale.max(1);
        let (axis_fixed, value, vertical) = match plane {
            SlicePlane::Xy { z } => (2, z, 1),
            SlicePlane::Xz { y } => (1, y, 2),
        };
        let layer = ((value - grid.origin[axis_fixed].get::<centimeter>()) / h).floor();
        let layer = (layer.max(0.0) as usize).min(grid.counts[axis_fixed] - 1);
        let (nx, nv) = (grid.counts[0], grid.counts[vertical]);
        let t_low = self.heat_removal.reference_temperature.get::<kelvin>();
        let t_high = self.peak_temperature.get::<kelvin>().max(t_low + 1e-9);
        let mut img = ImageData::filled(nx * scale, nv * scale, OUTSIDE);
        for iv in 0..nv {
            for ix in 0..nx {
                let mut ijk = [0usize; 3];
                ijk[0] = ix;
                ijk[vertical] = iv;
                ijk[axis_fixed] = layer;
                let Some(c) = self.mesh.voxel_cell[grid.flat(ijk)] else {
                    continue;
                };
                let rgb = match colour {
                    SliceColour::Region => {
                        ZONE_COLOURS[self.mesh.conduction.cell_zone[c] % ZONE_COLOURS.len()]
                    }
                    SliceColour::Temperature => {
                        let t = self.solution.temperature(c).get::<kelvin>();
                        heat_colour(((t - t_low) / (t_high - t_low)).clamp(0.0, 1.0))
                    }
                };
                for py in 0..scale {
                    for px in 0..scale {
                        img.set(ix * scale + px, (nv - 1 - iv) * scale + py, rgb);
                    }
                }
            }
        }
        img
    }
}

/// Blue (0) through white (0.5) to red (1).
fn heat_colour(f: f64) -> Rgb {
    let lerp = |a: f64, b: f64, t: f64| (a + (b - a) * t).round() as u8;
    if f < 0.5 {
        let t = f * 2.0;
        Rgb {
            r: lerp(49.0, 247.0, t),
            g: lerp(54.0, 247.0, t),
            b: lerp(149.0, 247.0, t),
        }
    } else {
        let t = (f - 0.5) * 2.0;
        Rgb {
            r: lerp(247.0, 165.0, t),
            g: lerp(247.0, 0.0, t),
            b: lerp(247.0, 38.0, t),
        }
    }
}
