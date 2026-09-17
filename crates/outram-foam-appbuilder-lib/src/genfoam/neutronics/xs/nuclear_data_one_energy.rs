// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/nuclearDataOneEnergy/
//                    nuclearDataOneEnergy.{C,H}
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `NuclearDataOneEnergy` — one interpolated cross-section value
//!
//! The atom of GeN-Foam's cross-section store: it holds one scalar nuclear-data
//! value (e.g. `nu Sigma_f` for a single energy group in a single material
//! zone) sampled across every perturbed reactor state, and interpolates it to
//! arbitrary feedback conditions with a polyharmonic-spline RBF
//! ([`crate::genfoam::common::rbf`]).
//!
//! A [`super::zone_data::ZoneNuclearData`] owns a grid of these — one per
//! energy group per quantity, plus the full scattering matrix.
//!
//! ## Reduced-parameter handling
//!
//! Only parameters that actually **vary** across the supplied states can be
//! interpolated over; a parameter that is identical in every state would make
//! the RBF saddle-point matrix singular. [`NuclearDataOneEnergy::build`]
//! therefore drops constant parameters, keeping a reduced coordinate table.
//! If fewer than two data points exist, or no parameter varies, the value is
//! treated as constant and [`NuclearDataOneEnergy::get`] returns the reference
//! (first) value — faithfully mirroring upstream `nuclearDataOneEnergy`.

use crate::genfoam::common::rbf;
use crate::genfoam::neutronics::xs::XsError;

/// Interpolator for a single scalar nuclear-data value across perturbed states.
///
/// All values are bare `f64` in the quantity's own base-SI units; physical
/// dimensions are re-attached at the [`super::group_constants`] boundary. The
/// parameter vectors passed to [`Self::add_data`] and [`Self::get`] are already
/// **transformed** (each variable's [`super::variables::VariableLaw`] applied)
/// and aligned to the owning [`super::CrossSectionData`]'s variable order.
#[derive(Debug, Clone)]
pub struct NuclearDataOneEnergy {
    /// Polyharmonic-spline `mode` (basis-function order) for this value.
    poly_spline_mode: usize,
    /// Number of feedback parameters (== number of `xsVariables`).
    n_parameters: usize,
    /// Transposed coordinate table: `parameter_values[param][data_point]`.
    parameter_values: Vec<Vec<f64>>,
    /// The stored value at each data point, in insertion (state) order. The
    /// first entry is the reference value.
    data: Vec<f64>,
    /// Indices (into the full parameter list) of the parameters that vary and
    /// are therefore interpolated over. Populated by [`Self::build`].
    reduced_param_idx: Vec<usize>,
    /// Reduced coordinate table over the varying parameters only, matching the
    /// layout [`rbf::polyharmonic_spline`] expects. Populated by [`Self::build`].
    point_list: Vec<Vec<f64>>,
    /// Polyharmonic-spline weights `[w | v]`. Populated by [`Self::build`].
    weights: Vec<f64>,
}

impl NuclearDataOneEnergy {
    /// Create an empty interpolator for `n_parameters` feedback variables using
    /// spline `mode`.
    ///
    /// Data points are then supplied with [`Self::add_data`] (one per reactor
    /// state, reference first) and the interpolant assembled with
    /// [`Self::build`].
    #[must_use]
    pub fn new(poly_spline_mode: usize, n_parameters: usize) -> Self {
        Self {
            poly_spline_mode,
            n_parameters,
            parameter_values: vec![Vec::new(); n_parameters],
            data: Vec::new(),
            reduced_param_idx: Vec::new(),
            point_list: Vec::new(),
            weights: Vec::new(),
        }
    }

    /// Append one data point: the value `value` observed at the (already
    /// transformed) parameter coordinates `parameters`.
    ///
    /// The first call registers the **reference** value returned by
    /// [`Self::get_ref`]. `parameters.len()` must equal the `n_parameters` this
    /// interpolator was created with.
    ///
    /// # Panics
    ///
    /// Panics if `parameters.len()` does not match `n_parameters`.
    pub fn add_data(&mut self, value: f64, parameters: &[f64]) {
        assert_eq!(
            parameters.len(),
            self.n_parameters,
            "parameter vector length must match the number of xsVariables"
        );
        self.data.push(value);
        for (param_i, &p) in parameters.iter().enumerate() {
            self.parameter_values[param_i].push(p);
        }
    }

    /// Assemble the interpolant from the accumulated data points.
    ///
    /// Detects which parameters vary, builds the reduced polyharmonic-spline
    /// system, and solves for the weights. If fewer than two data points exist
    /// or no parameter varies, the value is treated as constant and no system
    /// is solved.
    ///
    /// # Errors
    ///
    /// Propagates [`XsError::SingularRbfMatrix`] if the saddle-point solve fails
    /// (e.g. two states share identical parameter coordinates). The failure is
    /// never silently defaulted.
    pub fn build(&mut self) -> Result<(), XsError> {
        // Identify the parameters that actually vary across the data points.
        self.reduced_param_idx.clear();
        for (param_i, values) in self.parameter_values.iter().enumerate() {
            if !all_same(values) {
                self.reduced_param_idx.push(param_i);
            }
        }

        self.point_list.clear();
        self.weights.clear();

        if self.data.len() >= 2 && !self.reduced_param_idx.is_empty() {
            // Reduced coordinate table over varying parameters only, otherwise
            // the RBF matrix is singular.
            for &param_i in &self.reduced_param_idx {
                self.point_list.push(self.parameter_values[param_i].clone());
            }
            self.weights =
                rbf::solve_polyharmonic_spline(&self.point_list, &self.data, self.poly_spline_mode)
                    .map_err(|e| XsError::SingularRbfMatrix {
                        detail: e.to_string(),
                    })?;
        }
        Ok(())
    }

    /// The reference (first-registered) value, ignoring all feedback.
    ///
    /// GeN-Foam uses this for quantities it treats as feedback-independent in a
    /// given pass (e.g. the constants pass), and as the fallback whenever no
    /// interpolation is active.
    ///
    /// # Panics
    ///
    /// Panics if no data point has been added; a fully built
    /// [`super::CrossSectionData`] always has at least the reference state.
    #[must_use]
    pub fn get_ref(&self) -> f64 {
        self.data[0]
    }

    /// Interpolate the value at the (already transformed) parameter point
    /// `parameters`.
    ///
    /// `parameters` is the full feedback vector aligned to the variable order;
    /// the reduced set is selected internally. If `is_parametrize` is `false`,
    /// or no parameter varies, the reference value is returned unchanged (this
    /// is how GeN-Foam's `doNotParametrize` group list suppresses feedback for
    /// selected energy groups).
    ///
    /// # Panics
    ///
    /// Panics if `parameters.len()` is smaller than the largest reduced-parameter
    /// index; callers pass the full variable vector, which is always long enough.
    #[must_use]
    pub fn get(&self, parameters: &[f64], is_parametrize: bool) -> f64 {
        if self.reduced_param_idx.is_empty() || !is_parametrize {
            return self.data[0];
        }
        let reduced_input: Vec<f64> = self
            .reduced_param_idx
            .iter()
            .map(|&i| parameters[i])
            .collect();
        rbf::polyharmonic_spline(
            &self.weights,
            &self.point_list,
            &reduced_input,
            self.poly_spline_mode,
        )
    }

    /// Number of data points (perturbed states) registered.
    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.len()
    }
}

/// True if every element of `list` equals the first (or the list is empty).
fn all_same(list: &[f64]) -> bool {
    match list.first() {
        None => true,
        Some(&first) => list.iter().all(|&x| x == first),
    }
}
