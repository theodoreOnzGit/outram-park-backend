// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   applications/solvers/basic/laplacianFoam/laplacianFoam.C (TEqn, steady)
//     Copyright (C) 2011-2017 OpenFOAM Foundation
//     Copyright (C) 2019 OpenCFD Ltd.
//   src/thermoTools/derivedFvPatchFields/externalWallHeatFluxTemperature/
//     externalWallHeatFluxTemperatureFvPatchScalarField.C (convective mixed
//     condition: valueFraction = hp/(hp + kappa deltaCoeffs), refValue = Ta)
//   src/finiteVolume/interpolation/surfaceInterpolation/schemes/harmonic/
//     harmonic.H (face conductivity 1/reverseLinear(1/k))
//   Upstream licence: GPL-3.0-or-later.
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

//! The steady conduction solve and its result (see the module docs of
//! [`super`]).

use std::sync::Arc;

use uom::si::f64::{
    HeatTransfer, Power, TemperatureInterval, ThermalConductivity, ThermodynamicTemperature,
    VolumetricPowerDensity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::watt;
use uom::si::temperature_interval::kelvin as delta_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::mesh::ConductionMesh;
use super::SteadyConductionError;
use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::fv_operators::fvm;
use crate::fv_operators::fvm::DeltaCoeff;
use crate::ldu_matrix::fv_matrix::{SolverPerformance, SolverSettings};
use crate::mesh::fv_mesh::FvMesh;

/// The thermal condition on one boundary patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalBoundary {
    /// No heat crosses the patch (a symmetry plane, or perfect insulation).
    Adiabatic,
    /// The surface is held at this temperature.
    FixedTemperature(ThermodynamicTemperature),
    /// The surface loses heat to a fluid: `-k dT/dn = h (T_surface - T_ambient)`.
    Convective {
        /// Heat-transfer coefficient `h`.
        coefficient: HeatTransfer,
        /// Bulk fluid (ambient) temperature `T_ambient`.
        ambient: ThermodynamicTemperature,
    },
}

/// A [`ThermalBoundary`] applied to one mesh patch, with an optional per-face
/// film-area fraction.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchCondition {
    /// The condition.
    pub boundary: ThermalBoundary,
    /// For [`ThermalBoundary::Convective`] only: the fraction of each face's
    /// meshed area that is true wetted surface (one value per face, in
    /// `[0, 1]`). `None` means 1 everywhere. On a voxel staircase this is
    /// `|n_axis . n_true|`, which makes the film's total area converge to the
    /// true surface area (see [`super::mesh::voxel_mesh`]). Ignored by the
    /// other conditions.
    pub film_area_fraction: Option<Vec<f64>>,
}

impl PatchCondition {
    /// The condition with no film-area correction.
    #[must_use]
    pub fn uniform(boundary: ThermalBoundary) -> Self {
        Self {
            boundary,
            film_area_fraction: None,
        }
    }

    /// The condition with a per-face film-area fraction (see the field doc).
    #[must_use]
    pub fn with_film_area_fraction(boundary: ThermalBoundary, fraction: Vec<f64>) -> Self {
        Self {
            boundary,
            film_area_fraction: Some(fraction),
        }
    }
}

/// Iteration controls. The defaults converge the energy balance to about
/// `1e-10` of the generated power on the meshes in this module's tests.
#[derive(Debug, Clone, Copy)]
pub struct SteadyConductionSettings {
    /// Stop the Picard passes on `k(T)` when no cell moves more than this.
    pub picard_tolerance: TemperatureInterval,
    /// Give up after this many passes.
    pub max_picard_passes: usize,
    /// Conjugate-gradient settings for each linear solve.
    pub linear: SolverSettings,
}

impl Default for SteadyConductionSettings {
    fn default() -> Self {
        Self {
            picard_tolerance: TemperatureInterval::new::<delta_kelvin>(1e-6),
            max_picard_passes: 200,
            linear: SolverSettings {
                tolerance: 1e-13,
                max_iter: 100_000,
            },
        }
    }
}

/// Heat generated in the body against heat leaving through its boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyBalance {
    /// `sum(q''' V)` over the cells.
    pub generated: Power,
    /// Heat leaving through all patches, each face's flux recomputed from its
    /// own boundary condition and the solved cell temperature.
    pub removed: Power,
    /// [`Self::removed`] split by patch (mesh patch order).
    pub removed_per_patch: Vec<Power>,
}

impl EnergyBalance {
    /// `generated - removed`.
    #[must_use]
    pub fn imbalance(&self) -> Power {
        self.generated - self.removed
    }

    /// `|generated - removed| / generated` (0 when nothing is generated and
    /// nothing leaves).
    #[must_use]
    pub fn relative_imbalance(&self) -> f64 {
        let g = self.generated.get::<watt>().abs();
        let d = self.imbalance().get::<watt>().abs();
        if g > 0.0 {
            d / g
        } else {
            d
        }
    }
}

/// A steady conduction problem: a mesh, the heat source of each zone and the
/// condition on each patch.
#[derive(Debug, Clone)]
pub struct SteadyConductionProblem {
    mesh: ConductionMesh,
    /// W/m^3 per zone.
    zone_source: Vec<f64>,
    patches: Vec<PatchCondition>,
}

impl SteadyConductionProblem {
    /// Set up a problem.
    ///
    /// `zone_heat_source` has one entry per zone (`mesh.n_zones`);
    /// `patches` one entry per mesh patch, in patch order.
    ///
    /// # Errors
    /// [`SteadyConductionError::InvalidInput`] for wrong lengths, a negative or
    /// non-finite source, a non-positive `h`, a film-area fraction of the
    /// wrong length or outside `[0, 1]`, or a heated body with no patch that
    /// removes heat (no steady state exists).
    pub fn new(
        mesh: ConductionMesh,
        zone_heat_source: Vec<VolumetricPowerDensity>,
        patches: Vec<PatchCondition>,
    ) -> Result<Self, SteadyConductionError> {
        if zone_heat_source.len() != mesh.n_zones {
            return Err(SteadyConductionError::InvalidInput(format!(
                "{} zone heat sources for {} zones",
                zone_heat_source.len(),
                mesh.n_zones
            )));
        }
        if patches.len() != mesh.mesh.patches.len() {
            return Err(SteadyConductionError::InvalidInput(format!(
                "{} patch conditions for {} mesh patches",
                patches.len(),
                mesh.mesh.patches.len()
            )));
        }
        let zone_source: Vec<f64> = zone_heat_source
            .iter()
            .map(|q| q.get::<watt_per_cubic_meter>())
            .collect();
        if let Some(bad) = zone_source.iter().find(|q| !(**q >= 0.0) || !q.is_finite()) {
            return Err(SteadyConductionError::InvalidInput(format!(
                "heat sources must be finite and non-negative, got {bad} W/m^3"
            )));
        }
        let mut removes_heat = false;
        for (pi, (cond, patch)) in patches.iter().zip(mesh.mesh.patches.iter()).enumerate() {
            match cond.boundary {
                ThermalBoundary::Adiabatic => {}
                ThermalBoundary::FixedTemperature(_) => removes_heat |= patch.size > 0,
                ThermalBoundary::Convective { coefficient, .. } => {
                    let h = coefficient.get::<watt_per_square_meter_kelvin>();
                    if !(h > 0.0) || !h.is_finite() {
                        return Err(SteadyConductionError::InvalidInput(format!(
                            "patch {pi}: h must be positive and finite, got {h} W/(m^2 K)"
                        )));
                    }
                    removes_heat |= patch.size > 0;
                }
            }
            if let Some(frac) = &cond.film_area_fraction {
                if frac.len() != patch.size {
                    return Err(SteadyConductionError::InvalidInput(format!(
                        "patch {pi}: {} film-area fractions for {} faces",
                        frac.len(),
                        patch.size
                    )));
                }
                if let Some(f) = frac.iter().find(|f| !(0.0..=1.0).contains(*f)) {
                    return Err(SteadyConductionError::InvalidInput(format!(
                        "patch {pi}: film-area fraction {f} is outside [0, 1]"
                    )));
                }
            }
        }
        let heated = mesh
            .cell_zone
            .iter()
            .any(|z| zone_source.get(*z).is_some_and(|q| *q > 0.0));
        if heated && !removes_heat {
            return Err(SteadyConductionError::InvalidInput(
                "heat is generated but no patch removes it: no steady state exists".into(),
            ));
        }
        Ok(Self {
            mesh,
            zone_source,
            patches,
        })
    }

    /// The mesh the problem is posed on.
    #[must_use]
    pub fn mesh(&self) -> &ConductionMesh {
        &self.mesh
    }

    /// Solve for the steady temperature.
    ///
    /// `conductivity(zone, T)` gives `k` of a zone at temperature `T`; it is
    /// re-evaluated at every cell's temperature between passes (Picard) until
    /// no cell moves more than `settings.picard_tolerance`. A constant `k`
    /// converges on the second pass.
    ///
    /// # Errors
    /// [`SteadyConductionError::BadConductivity`] for a non-positive or
    /// non-finite `k`; [`SteadyConductionError::LinearSolveFailed`];
    /// [`SteadyConductionError::PicardNotConverged`].
    pub fn solve<K>(
        &self,
        conductivity: K,
        settings: &SteadyConductionSettings,
    ) -> Result<SteadyConductionSolution, SteadyConductionError>
    where
        K: Fn(usize, ThermodynamicTemperature) -> ThermalConductivity,
    {
        let mesh = self.mesh.mesh.clone();
        let n = mesh.n_cells;
        let tol = settings.picard_tolerance.get::<delta_kelvin>();
        let mut t = vec![self.initial_guess_kelvin(); n];
        let mut k = vec![0.0; n];
        let q_field = VolScalarField::new(
            "q",
            mesh.clone(),
            Field::new(
                self.mesh
                    .cell_zone
                    .iter()
                    .map(|z| self.zone_source[*z])
                    .collect(),
            ),
            mesh.patches
                .iter()
                .map(|p| PatchField::zero_gradient(p.size))
                .collect(),
        );

        let mut passes = 0;
        let mut change = f64::INFINITY;
        let mut linear = SolverPerformance {
            n_iterations: 0,
            final_residual: 0.0,
            converged: true,
        };
        while change >= tol {
            if passes == settings.max_picard_passes {
                return Err(SteadyConductionError::PicardNotConverged {
                    last_change_kelvin: change,
                    iterations: passes,
                });
            }
            passes += 1;
            for c in 0..n {
                let zone = self.mesh.cell_zone[c];
                let kc = conductivity(zone, ThermodynamicTemperature::new::<kelvin>(t[c]))
                    .get::<watt_per_meter_kelvin>();
                if !(kc > 0.0) || !kc.is_finite() {
                    return Err(SteadyConductionError::BadConductivity {
                        zone,
                        temperature_kelvin: t[c],
                        value: kc,
                    });
                }
                k[c] = kc;
            }
            let gamma = face_conductivity(&mesh, &k);
            let t_field = VolScalarField::new(
                "T",
                mesh.clone(),
                Field::new(t.clone()),
                self.boundary_fields(&mesh, &k),
            );
            let eqn = fvm::laplacian(&gamma, &t_field) - fvm::su(&q_field, &t_field);
            let (solved, perf) = eqn.solve_cg_with_guess("T", &t_field, settings.linear);
            if !perf.converged {
                return Err(SteadyConductionError::LinearSolveFailed {
                    residual: perf.final_residual,
                    iterations: perf.n_iterations,
                });
            }
            linear = perf;
            change = solved
                .internal
                .iter()
                .zip(t.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            t.copy_from_slice(solved.internal.as_slice());
        }

        let (balance, face_t, face_weight) = self.energy_balance(&mesh, &k, &t);
        Ok(SteadyConductionSolution {
            mesh,
            cell_zone: self.mesh.cell_zone.clone(),
            cell_temperature: t,
            cell_conductivity: k,
            boundary_face_temperature: face_t,
            boundary_face_weight: face_weight,
            energy_balance: balance,
            picard_passes: passes,
            last_picard_change: TemperatureInterval::new::<delta_kelvin>(change),
            linear,
        })
    }

    /// The starting temperature: the first fixed or ambient temperature found.
    fn initial_guess_kelvin(&self) -> f64 {
        self.patches
            .iter()
            .find_map(|p| match p.boundary {
                ThermalBoundary::FixedTemperature(t) => Some(t.get::<kelvin>()),
                ThermalBoundary::Convective { ambient, .. } => Some(ambient.get::<kelvin>()),
                ThermalBoundary::Adiabatic => None,
            })
            .unwrap_or(300.0)
    }

    /// Film coefficient times film-area fraction on one boundary face, W/(m^2 K).
    fn face_h(&self, pi: usize, local: usize) -> Option<(f64, f64)> {
        let cond = &self.patches[pi];
        match cond.boundary {
            ThermalBoundary::Convective {
                coefficient,
                ambient,
            } => {
                let frac = cond.film_area_fraction.as_ref().map_or(1.0, |f| f[local]);
                Some((
                    coefficient.get::<watt_per_square_meter_kelvin>() * frac,
                    ambient.get::<kelvin>(),
                ))
            }
            _ => None,
        }
    }

    /// The temperature field's boundary conditions for the current `k`.
    fn boundary_fields(&self, mesh: &FvMesh, k: &[f64]) -> Vec<PatchField<f64>> {
        mesh.patches
            .iter()
            .enumerate()
            .map(|(pi, patch)| {
                let bc = match self.patches[pi].boundary {
                    ThermalBoundary::Adiabatic => BoundaryCondition::ZeroGradient,
                    ThermalBoundary::FixedTemperature(tf) => {
                        BoundaryCondition::FixedValue(tf.get::<kelvin>())
                    }
                    ThermalBoundary::Convective { .. } => {
                        let mut w = Vec::with_capacity(patch.size);
                        let mut t_ref = Vec::with_capacity(patch.size);
                        for local in 0..patch.size {
                            let gf = patch.start + local;
                            let d = DeltaCoeff::Orthogonal.boundary_delta(mesh, gf);
                            let (h, ta) = self.face_h(pi, local).unwrap_or((0.0, 0.0));
                            // externalWallHeatFluxTemperature: hp/(hp + k deltaCoeffs).
                            w.push(h * d / (h * d + k[mesh.owner[gf]]));
                            t_ref.push(ta);
                        }
                        BoundaryCondition::MixedField {
                            value_fraction: Field::new(w),
                            ref_value: Field::new(t_ref),
                            ref_grad: Field::zeros(patch.size),
                        }
                    }
                };
                PatchField {
                    bc,
                    values: Field::zeros(patch.size),
                }
            })
            .collect()
    }

    /// Generated and removed heat, the boundary-face temperatures, and the
    /// true-area weight of each boundary face (area times film fraction).
    fn energy_balance(
        &self,
        mesh: &FvMesh,
        k: &[f64],
        t: &[f64],
    ) -> (EnergyBalance, Vec<f64>, Vec<f64>) {
        let generated: f64 = (0..mesh.n_cells)
            .map(|c| self.zone_source[self.mesh.cell_zone[c]] * mesh.cell_volumes[c])
            .sum();
        let n_b = mesh.n_faces - mesh.n_internal_faces;
        let mut face_t = vec![0.0; n_b];
        let mut face_w = vec![0.0; n_b];
        let mut per_patch = Vec::with_capacity(mesh.patches.len());
        for (pi, patch) in mesh.patches.iter().enumerate() {
            let mut removed = 0.0;
            for local in 0..patch.size {
                let gf = patch.start + local;
                let c = mesh.owner[gf];
                let area = mesh.face_areas[gf];
                let d = DeltaCoeff::Orthogonal.boundary_delta(mesh, gf);
                let (flux, tf, weight) = match self.patches[pi].boundary {
                    ThermalBoundary::Adiabatic => (0.0, t[c], area),
                    ThermalBoundary::FixedTemperature(ts) => {
                        let ts = ts.get::<kelvin>();
                        (k[c] * area / d * (t[c] - ts), ts, area)
                    }
                    ThermalBoundary::Convective { .. } => {
                        let (h, ta) = self.face_h(pi, local).unwrap_or((0.0, 0.0));
                        if h > 0.0 {
                            let flux = area * (t[c] - ta) / (1.0 / h + d / k[c]);
                            // Surface temperature from the film: q'' = h (T_s - T_a).
                            (flux, ta + flux / (h * area), area * h / self.raw_h(pi))
                        } else {
                            (0.0, t[c], 0.0)
                        }
                    }
                };
                removed += flux;
                face_t[gf - mesh.n_internal_faces] = tf;
                face_w[gf - mesh.n_internal_faces] = weight;
            }
            per_patch.push(Power::new::<watt>(removed));
        }
        let removed = per_patch.iter().map(|p| p.get::<watt>()).sum();
        (
            EnergyBalance {
                generated: Power::new::<watt>(generated),
                removed: Power::new::<watt>(removed),
                removed_per_patch: per_patch,
            },
            face_t,
            face_w,
        )
    }

    /// The patch's convective `h` without film fraction, W/(m^2 K) (1 if not convective).
    fn raw_h(&self, pi: usize) -> f64 {
        match self.patches[pi].boundary {
            ThermalBoundary::Convective { coefficient, .. } => {
                coefficient.get::<watt_per_square_meter_kelvin>()
            }
            _ => 1.0,
        }
    }
}

/// Face conductivity for `fvm::laplacian` so that each internal face carries
/// the series conductance `A / (d_P/k_P + d_N/k_N)` (OpenFOAM `harmonic`).
///
/// `fvm::laplacian` multiplies the face value by `A/|d|`, so the face value is
/// `|d| / (d_P/k_P + d_N/k_N)`. Boundary faces carry the owner cell's `k`.
fn face_conductivity(mesh: &Arc<FvMesh>, k: &[f64]) -> SurfaceScalarField {
    let internal: Vec<f64> = (0..mesh.n_internal_faces)
        .map(|f| {
            let o = mesh.owner[f];
            let nb = mesh.neighbour[f];
            let d = DeltaCoeff::Orthogonal.interior_delta(mesh, f);
            let d_p = (mesh.face_centres[f] - mesh.cell_centres[o]).mag();
            let d_n = (mesh.cell_centres[nb] - mesh.face_centres[f]).mag();
            d / (d_p / k[o] + d_n / k[nb])
        })
        .collect();
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::new((0..p.size).map(|i| k[mesh.owner[p.start + i]]).collect()),
        })
        .collect();
    SurfaceScalarField::new("k", mesh.clone(), Field::new(internal), boundary)
}

/// The solved steady temperature and its energy balance.
#[derive(Debug, Clone)]
pub struct SteadyConductionSolution {
    mesh: Arc<FvMesh>,
    cell_zone: Vec<usize>,
    /// K per cell.
    cell_temperature: Vec<f64>,
    /// W/(m K) per cell, as used in the final solve.
    cell_conductivity: Vec<f64>,
    /// K per boundary face (global face index minus `n_internal_faces`).
    boundary_face_temperature: Vec<f64>,
    /// Area weight of each boundary face for surface means, m^2.
    boundary_face_weight: Vec<f64>,
    /// Heat generated against heat removed.
    pub energy_balance: EnergyBalance,
    /// Picard passes performed (a constant `k` takes 2).
    pub picard_passes: usize,
    /// Largest cell change of the final pass.
    pub last_picard_change: TemperatureInterval,
    /// The last linear solve's record.
    pub linear: SolverPerformance,
}

impl SteadyConductionSolution {
    /// Number of cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.cell_temperature.len()
    }

    /// Temperature of cell `c` (panics if `c` is out of range, like indexing).
    #[must_use]
    pub fn temperature(&self, c: usize) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.cell_temperature[c])
    }

    /// Conductivity of cell `c` as used in the final solve.
    #[must_use]
    pub fn conductivity(&self, c: usize) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(self.cell_conductivity[c])
    }

    /// The hottest cell and its temperature (a cell-centre value; the true
    /// peak of a smooth profile lies slightly above it on a coarse mesh).
    #[must_use]
    pub fn peak_temperature(&self) -> (usize, ThermodynamicTemperature) {
        let (c, t) =
            self.cell_temperature
                .iter()
                .enumerate()
                .fold((0, f64::NEG_INFINITY), |acc, (c, t)| {
                    if *t > acc.1 {
                        (c, *t)
                    } else {
                        acc
                    }
                });
        (c, ThermodynamicTemperature::new::<kelvin>(t))
    }

    /// Volume-weighted mean temperature over the cells of `zones` (all cells
    /// when `zones` is empty). `None` if no cell matches.
    #[must_use]
    pub fn volume_mean_temperature(&self, zones: &[usize]) -> Option<ThermodynamicTemperature> {
        let (mut sum, mut vol) = (0.0, 0.0);
        for c in 0..self.n_cells() {
            if zones.is_empty() || zones.contains(&self.cell_zone[c]) {
                sum += self.cell_temperature[c] * self.mesh.cell_volumes[c];
                vol += self.mesh.cell_volumes[c];
            }
        }
        (vol > 0.0).then(|| ThermodynamicTemperature::new::<kelvin>(sum / vol))
    }

    /// Temperature of boundary face `face` (a global face index).
    ///
    /// Fixed: the imposed value. Convective: `T_a + q''/h`, from the face's
    /// own film. Adiabatic: the owner cell's temperature. `None` for an
    /// internal face.
    #[must_use]
    pub fn boundary_face_temperature(&self, face: usize) -> Option<ThermodynamicTemperature> {
        face.checked_sub(self.mesh.n_internal_faces)
            .and_then(|i| self.boundary_face_temperature.get(i))
            .map(|t| ThermodynamicTemperature::new::<kelvin>(*t))
    }

    /// Area-weighted mean surface temperature of patch `patch`.
    ///
    /// On a convective patch the weight of each face is its area times its
    /// film-area fraction, i.e. the true wetted area it stands for. `None`
    /// for an empty patch or one with zero total weight.
    #[must_use]
    pub fn surface_mean_temperature(&self, patch: usize) -> Option<ThermodynamicTemperature> {
        let p = self.mesh.patches.get(patch)?;
        let (mut sum, mut w) = (0.0, 0.0);
        for local in 0..p.size {
            let i = p.start + local - self.mesh.n_internal_faces;
            sum += self.boundary_face_temperature[i] * self.boundary_face_weight[i];
            w += self.boundary_face_weight[i];
        }
        (w > 0.0).then(|| ThermodynamicTemperature::new::<kelvin>(sum / w))
    }
}
