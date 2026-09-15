// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D pipe both [`super::drift_flux`] and [`super::two_fluid`] march on.
//!
//! # What belongs here
//!
//! Geometry and geometry alone: length, flow area, hydraulic diameter, incline,
//! and the uniform axial cell layout derived from them. No state, no fluid
//! properties, no time.
//!
//! # What does not
//!
//! Anything that changes during a transient. Both solvers own their own field
//! state; this type is constructed once and read thereafter.

use uom::si::area::square_meter;
use uom::si::f64::{Angle, Area, Length};
use uom::si::length::meter;

use crate::TampinesError;

/// A straight pipe of constant cross-section, discretised into uniform axial
/// cells.
///
/// # The mesh
///
/// `n_cells` cells of equal length `Δx = L / n_cells`, indexed `0 … n_cells−1`
/// from the `x = 0` end. Cell `i` spans `[i Δx, (i+1) Δx]` and its centre is at
/// `(i + ½) Δx`. Faces are indexed `0 … n_cells`, face `i` sitting at `x = i Δx`
/// between cells `i−1` and `i`; faces `0` and `n_cells` are the two ends.
///
/// This deliberately matches the layout
/// [`outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh`]
/// produces, so a case written against the HEM [`crate::hem`] array maps onto
/// these solvers cell-for-cell.
///
/// # Units
///
/// Constructed and read through `uom`. The stored fields are raw `f64` in
/// strict SI — metre, square metre, radian — because the marching loops read
/// them per cell per step.
///
/// # Valid ranges
///
/// `length > 0`, `flow_area > 0`, `n_cells ≥ 1`, `hydraulic_diameter > 0`.
/// The incline is unrestricted: `+π/2` is vertically upward flow in the `+x`
/// direction, `−π/2` vertically downward, `0` horizontal.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipe1d {
    /// Total pipe length `L` \[m\].
    length_m: f64,
    /// Cross-sectional flow area `A` \[m²\], constant along the pipe.
    flow_area_m2: f64,
    /// Hydraulic diameter `D_h = 4A/P` \[m\], used by the wall-friction
    /// closures and by the drag correlations' length scale.
    hydraulic_diameter_m: f64,
    /// Incline from horizontal \[rad\]; `+` means `+x` points upward.
    incline_rad: f64,
    /// Number of uniform axial cells.
    n_cells: usize,
}

impl Pipe1d {
    /// Build a pipe from its dimensions.
    ///
    /// # Arguments
    ///
    /// - `length` — total length `L` \[m\], strictly positive.
    /// - `flow_area` — cross-sectional flow area `A` \[m²\], strictly positive.
    /// - `hydraulic_diameter` — `D_h = 4A/P` \[m\], strictly positive. For a
    ///   circular pipe running full this is the inside diameter; it is taken
    ///   separately rather than derived from `A` so that non-circular and
    ///   rod-bundle geometries work without a special case.
    /// - `incline` — angle from horizontal \[rad\], `+` for `+x` upward.
    /// - `n_cells` — number of uniform axial cells, at least 1.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] naming the offending dimension.
    pub fn new(
        length: Length,
        flow_area: Area,
        hydraulic_diameter: Length,
        incline: Angle,
        n_cells: usize,
    ) -> Result<Self, TampinesError> {
        let length_m = length.get::<meter>();
        let flow_area_m2 = flow_area.get::<square_meter>();
        let hydraulic_diameter_m = hydraulic_diameter.get::<meter>();
        if !(length_m > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "pipe length must be > 0 m (got {length_m})"
            )));
        }
        if !(flow_area_m2 > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "pipe flow area must be > 0 m^2 (got {flow_area_m2})"
            )));
        }
        if !(hydraulic_diameter_m > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "hydraulic diameter must be > 0 m (got {hydraulic_diameter_m})"
            )));
        }
        if n_cells < 1 {
            return Err(TampinesError::InvalidInput(
                "pipe must have at least 1 cell".to_string(),
            ));
        }
        Ok(Self {
            length_m,
            flow_area_m2,
            hydraulic_diameter_m,
            incline_rad: incline.get::<uom::si::angle::radian>(),
            n_cells,
        })
    }

    /// A circular pipe, whose hydraulic diameter *is* its inside diameter and
    /// whose flow area follows from it as `A = πD²/4`.
    ///
    /// The common case, and the one the Edwards–O'Brien blowdown uses.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] as [`new`](Self::new).
    pub fn circular(
        length: Length,
        inside_diameter: Length,
        incline: Angle,
        n_cells: usize,
    ) -> Result<Self, TampinesError> {
        let d = inside_diameter.get::<meter>();
        let area = Area::new::<square_meter>(std::f64::consts::PI * d * d / 4.0);
        Self::new(length, area, inside_diameter, incline, n_cells)
    }

    /// Total pipe length `L` \[m\].
    #[must_use]
    pub fn length(&self) -> Length {
        Length::new::<meter>(self.length_m)
    }

    /// Cross-sectional flow area `A` \[m²\].
    #[must_use]
    pub fn flow_area(&self) -> Area {
        Area::new::<square_meter>(self.flow_area_m2)
    }

    /// Hydraulic diameter `D_h` \[m\].
    #[must_use]
    pub fn hydraulic_diameter(&self) -> Length {
        Length::new::<meter>(self.hydraulic_diameter_m)
    }

    /// Incline from horizontal \[rad\], `+` for `+x` upward.
    #[must_use]
    pub fn incline(&self) -> Angle {
        Angle::new::<uom::si::angle::radian>(self.incline_rad)
    }

    /// Number of uniform axial cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }

    /// Cell length `Δx = L / n_cells` \[m\], raw SI.
    #[must_use]
    pub fn dx(&self) -> f64 {
        self.length_m / self.n_cells as f64
    }

    /// Cell volume `V = A Δx` \[m³\], raw SI. Uniform, so one value serves
    /// every cell.
    #[must_use]
    pub fn cell_volume(&self) -> f64 {
        self.flow_area_m2 * self.dx()
    }

    /// Flow area \[m²\] as raw SI, for the marching loops.
    #[must_use]
    pub fn area_si(&self) -> f64 {
        self.flow_area_m2
    }

    /// Hydraulic diameter \[m\] as raw SI, for the marching loops.
    #[must_use]
    pub fn hydraulic_diameter_si(&self) -> f64 {
        self.hydraulic_diameter_m
    }

    /// The gravitational acceleration **along the pipe axis**,
    /// `g_x = −g sin(θ)` \[m/s²\], raw SI.
    ///
    /// Sign convention: `θ > 0` means `+x` points upward, so gravity opposes
    /// `+x` motion and `g_x` is negative. A horizontal pipe returns exactly
    /// zero, which is what the Edwards case needs.
    #[must_use]
    pub fn axial_gravity(&self) -> f64 {
        -9.806_65 * self.incline_rad.sin()
    }

    /// Centre position of cell `i` \[m\], raw SI: `(i + ½) Δx`.
    ///
    /// # Panics
    ///
    /// If `i >= n_cells`.
    #[must_use]
    pub fn cell_centre(&self, i: usize) -> f64 {
        assert!(
            i < self.n_cells,
            "cell {i} is out of range for a {}-cell pipe",
            self.n_cells
        );
        (i as f64 + 0.5) * self.dx()
    }

    /// The index of the cell whose centre lies closest to axial position `x`
    /// \[m\].
    ///
    /// Used to map an experiment's instrument stations onto the mesh. Positions
    /// outside `[0, L]` clamp to the first or last cell rather than erroring —
    /// a gauge nominally at the closed end is legitimately at `x = 0`, which is
    /// half a cell outside the first cell centre.
    #[must_use]
    pub fn nearest_cell(&self, x: Length) -> usize {
        let x_m = x.get::<meter>();
        let raw = (x_m / self.dx() - 0.5).round();
        if raw < 0.0 {
            0
        } else if raw as usize >= self.n_cells {
            self.n_cells - 1
        } else {
            raw as usize
        }
    }
}
