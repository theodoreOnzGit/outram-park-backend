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

//! # Direct coupling: Monte Carlo transport against GeN-Foam thermal-hydraulics
//!
//! The other coupling in this crate, [`crate::coupling`], goes *through*
//! multigroup cross sections: Monte Carlo condenses them, a deterministic
//! solver consumes them. This one does not. Here `outram-mc` **is** the
//! neutronics, iterated directly against GeN-Foam's thermal model until the two
//! agree on a temperature.
//!
//! ## The loop
//!
//! ```text
//!   repeat:
//!     Monte Carlo transport at the current fuel temperature  -> k_eff, power shape
//!     write powerDensityNeutronics into the coupling region
//!     GeN-Foam LumpedThermal.correct()                       -> new TFuel
//!     feed TFuel back as the transport temperature
//!   until |dT| and |dk| are both below tolerance
//! ```
//!
//! This is a Picard (fixed-point) iteration, the same scheme GeN-Foam's own
//! `multiPhysicsSolver` outer loop uses, and the thermal half is literally
//! GeN-Foam's [`LumpedThermal`] driven through its
//! [`RegionKernel`] contract and its [`CouplingRegion`] field exchange. The
//! thermal physics is not reimplemented here.
//!
//! ## The Doppler feedback is GLOBAL, and that is a real limitation
//!
//! `outram-mc` takes **one** temperature for the whole problem
//! (`KeffSettings::temperature_k`); it is not per-material. So the feedback
//! this loop applies is a single fuel temperature applied everywhere, which is
//! coherent with a **lumped** thermal model of one node and would be wrong for
//! a spatially resolved one. A mesh-resolved coupling needs per-cell
//! temperature in the transport, which the transport does not yet accept.
//! Stated here rather than discovered later.
//!
//! ## Power normalisation
//!
//! Monte Carlo returns reaction rates per source particle, not watts. The
//! caller therefore states the reactor's thermal power, and that is what drives
//! the thermal model; the transport supplies `k_eff` and the temperature
//! feedback. This is honest for a lumped node — there is only one power density
//! to distribute — and would need the tallied `KappaFission` shape the moment
//! more than one thermal node exists.

use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::multi_region::coupling_fields::CouplingRegion;
use outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::{
    lumped_cell_mesh, LumpedThermal, RegionKernel,
};
use outram_foam_basic_lib::prelude::{BoundaryCondition, Field, PatchField, VolScalarField};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};
use uom::si::f64::{Power, ThermodynamicTemperature, Time};
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Why a coupled solve stopped without converging.
#[derive(Debug, thiserror::Error)]
pub enum DirectCouplingError {
    /// The outer loop hit its iteration cap.
    ///
    /// Carries the last residuals so the caller can see whether it was close or
    /// diverging — those need opposite responses, and a bare "did not converge"
    /// cannot distinguish them.
    #[error(
        "coupled solve did not converge in {iterations} iterations: \
         last |dT| = {last_dt_k:.4} K (tol {tol_k}), last |dk| = {last_dk:.6} (tol {tol_k_eff})"
    )]
    NotConverged {
        /// Iterations performed.
        iterations: usize,
        /// Last temperature change \[K\].
        last_dt_k: f64,
        /// Temperature tolerance \[K\].
        tol_k: f64,
        /// Last eigenvalue change.
        last_dk: f64,
        /// Eigenvalue tolerance.
        tol_k_eff: f64,
    },
    /// GeN-Foam's thermal region failed to advance.
    #[error("the GeN-Foam thermal region failed: {0}")]
    Thermal(String),
}

/// Convergence criteria for the outer Picard loop.
#[derive(Debug, Clone, Copy)]
pub struct Convergence {
    /// Stop when the fuel temperature moves less than this between outer
    /// iterations \[K\].
    pub temperature_k: f64,
    /// Stop when `k_eff` moves less than this between outer iterations.
    ///
    /// Setting this below the Monte Carlo statistical uncertainty is asking the
    /// loop to converge on noise; it will burn iterations and stop on chance.
    pub k_eff: f64,
    /// Maximum outer iterations before giving up.
    pub max_iterations: usize,
}

impl Default for Convergence {
    /// 1 K and 1e-3 in `k`, capped at 20 outer iterations.
    ///
    /// `1e-3` is chosen to sit at or above a typical Monte Carlo one-sigma at
    /// practical particle counts, for the reason given on [`Self::k_eff`].
    fn default() -> Self {
        Self {
            temperature_k: 1.0,
            k_eff: 1.0e-3,
            max_iterations: 20,
        }
    }
}

/// One outer iteration's state, kept so a caller can see the approach rather
/// than only the answer.
#[derive(Debug, Clone, Copy)]
pub struct CoupledIteration {
    /// 1-based iteration number.
    pub iteration: usize,
    /// Fuel temperature the transport ran at \[K\].
    pub temperature_k: f64,
    /// Eigenvalue at that temperature.
    pub k_eff: f64,
    /// Its one-sigma statistical uncertainty.
    pub k_std: f64,
    /// Fuel temperature after the thermal solve \[K\].
    pub temperature_after_k: f64,
}

/// A converged coupled steady state.
#[derive(Debug, Clone)]
pub struct CoupledSteadyState {
    /// Equilibrium fuel temperature \[K\].
    pub temperature_k: f64,
    /// Eigenvalue at equilibrium.
    pub k_eff: f64,
    /// Its one-sigma statistical uncertainty.
    pub k_std: f64,
    /// Every outer iteration, in order.
    pub history: Vec<CoupledIteration>,
}

impl CoupledSteadyState {
    /// Temperature reactivity feedback across the whole solve, in pcm/K.
    ///
    /// Computed from the FIRST and LAST iterations, so it is a secant slope
    /// over the traversed range and not a local derivative. Returns `None` if
    /// the temperature never moved, where the slope is undefined rather than
    /// zero.
    ///
    /// Treat it as diagnostic. It is differenced from two eigenvalues that each
    /// carry Monte Carlo noise, so its uncertainty is roughly
    /// `sqrt(2) * sigma_k / dT` — at a small `dT` that can swamp the signal
    /// entirely.
    #[must_use]
    pub fn feedback_pcm_per_k(&self) -> Option<f64> {
        let first = self.history.first()?;
        let last = self.history.last()?;
        let dt = last.temperature_k - first.temperature_k;
        if dt.abs() < f64::EPSILON {
            return None;
        }
        Some((last.k_eff - first.k_eff) / first.k_eff * 1.0e5 / dt)
    }
}

/// Monte Carlo neutronics iterated directly against GeN-Foam thermal-hydraulics.
///
/// Build with [`new`](Self::new), then [`solve_steady_state`](Self::solve_steady_state).
pub struct McGenFoamDirect {
    geometry: Geometry,
    materials: Vec<Material>,
    nuclides: Vec<Nuclide>,
    source: SourceBox,
    majorants: Vec<Majorant>,
    settings: KeffSettings,
    thermal: LumpedThermal,
    region: CouplingRegion,
    power: Power,
    dt: Time,
}

impl McGenFoamDirect {
    /// A direct coupling of `geometry` to a GeN-Foam lumped thermal region.
    ///
    /// `power` is the reactor's thermal power, which drives the thermal model;
    /// see the module note on power normalisation for why it is stated rather
    /// than derived. `thermal` is GeN-Foam's own lumped node, constructed by the
    /// caller so its heat capacity, conductance and sink temperature are
    /// explicit rather than guessed here.
    #[must_use]
    pub fn new(
        geometry: Geometry,
        materials: Vec<Material>,
        nuclides: Vec<Nuclide>,
        source: SourceBox,
        thermal: LumpedThermal,
        power: Power,
        region_volume_m3: f64,
    ) -> Self {
        let mesh = lumped_cell_mesh(region_volume_m3);
        let mut region = CouplingRegion::new("core", Arc::clone(&mesh));
        let zero = |name: &str, mesh: &Arc<outram_foam_basic_lib::prelude::FvMesh>| {
            let boundary = mesh
                .patches
                .iter()
                .map(|p| PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::uniform(p.size, 0.0),
                })
                .collect();
            VolScalarField::new(
                name,
                Arc::clone(mesh),
                Field::uniform(mesh.n_cells, 0.0),
                boundary,
            )
        };
        region.insert_scalar(zero("powerDensityNeutronics", &mesh));
        region.insert_scalar(zero("TFuel", &mesh));

        let t0 = thermal.fuel_temperature().get::<kelvin>();
        Self {
            geometry,
            materials,
            nuclides,
            source,
            majorants: Vec::new(),
            settings: KeffSettings {
                n_particles: 2_000,
                n_inactive: 20,
                n_active: 40,
                seed: 20_260_919,
                temperature_k: t0,
                ..KeffSettings::default()
            },
            thermal,
            region,
            power,
            dt: Time::new::<second>(1.0),
        }
    }

    /// Transport `n` particles per batch.
    #[must_use]
    pub fn with_particles(mut self, n: usize) -> Self {
        self.settings.n_particles = n;
        self
    }

    /// Use `inactive` source-convergence batches and `active` scoring batches.
    #[must_use]
    pub fn with_batches(mut self, inactive: usize, active: usize) -> Self {
        self.settings.n_inactive = inactive;
        self.settings.n_active = active;
        self
    }

    /// Supply delta-tracking majorants for a delta-tracked model.
    #[must_use]
    pub fn with_majorants(mut self, majorants: Vec<Majorant>) -> Self {
        self.majorants = majorants;
        self
    }

    /// The pseudo-time step handed to the thermal model each outer iteration.
    ///
    /// For a steady state this only sets how fast the lumped node relaxes
    /// toward its equilibrium; the converged answer does not depend on it, but
    /// the iteration count does. Too small and the loop crawls; too large and
    /// it can overshoot.
    #[must_use]
    pub fn with_time_step(mut self, dt: Time) -> Self {
        self.dt = dt;
        self
    }

    /// Iterate transport and thermal-hydraulics to a converged steady state.
    ///
    /// # Errors
    ///
    /// [`DirectCouplingError::NotConverged`] if the cap is reached, carrying the
    /// last residuals; [`DirectCouplingError::Thermal`] if GeN-Foam's region
    /// fails to advance.
    pub fn solve_steady_state(
        &mut self,
        criteria: Convergence,
    ) -> Result<CoupledSteadyState, DirectCouplingError> {
        let mut history: Vec<CoupledIteration> = Vec::new();
        let mut last_dt_k = f64::INFINITY;
        let mut last_dk = f64::INFINITY;

        for iteration in 1..=criteria.max_iterations {
            // 1. Transport at the current fuel temperature.
            let t_before = self.settings.temperature_k;
            let mc = if self.majorants.is_empty() {
                run_keff_csg(
                    &self.geometry,
                    &self.materials,
                    &self.nuclides,
                    self.source,
                    &self.settings,
                    None,
                )
            } else {
                run_keff_csg_hybrid(
                    &self.geometry,
                    &self.materials,
                    &self.nuclides,
                    &self.majorants,
                    None,
                    self.source,
                    &self.settings,
                    None,
                )
            };

            // 2. Hand the power density to the coupling region, in the field
            //    name GeN-Foam's thermal kernel reads.
            let volume = lumped_cell_mesh_volume(&self.region);
            let q = if volume > 0.0 {
                self.power.get::<watt>() / volume
            } else {
                0.0
            };
            if let Some(f) = self.region.scalar_mut("powerDensityNeutronics") {
                for v in f.internal.as_mut_slice() {
                    *v = q;
                }
            }

            // 3. GeN-Foam's own thermal model advances.
            self.thermal.begin_step();
            self.thermal
                .correct(&mut self.region, self.dt, false)
                .map_err(|e| DirectCouplingError::Thermal(format!("{e:?}")))?;
            let t_after = self.thermal.fuel_temperature().get::<kelvin>();

            // 4. Feed the temperature back into the transport.
            self.settings.temperature_k = t_after;

            let dk = history
                .last()
                .map(|p: &CoupledIteration| (mc.k_mean - p.k_eff).abs())
                .unwrap_or(f64::INFINITY);
            last_dt_k = (t_after - t_before).abs();
            last_dk = dk;

            history.push(CoupledIteration {
                iteration,
                temperature_k: t_before,
                k_eff: mc.k_mean,
                k_std: mc.k_std,
                temperature_after_k: t_after,
            });

            if last_dt_k < criteria.temperature_k && dk < criteria.k_eff {
                let last = *history.last().expect("just pushed");
                return Ok(CoupledSteadyState {
                    temperature_k: t_after,
                    k_eff: last.k_eff,
                    k_std: last.k_std,
                    history,
                });
            }
        }

        Err(DirectCouplingError::NotConverged {
            iterations: criteria.max_iterations,
            last_dt_k,
            tol_k: criteria.temperature_k,
            last_dk,
            tol_k_eff: criteria.k_eff,
        })
    }
}

/// Total cell volume of a coupling region's mesh \[m^3\].
fn lumped_cell_mesh_volume(region: &CouplingRegion) -> f64 {
    region.mesh().cell_volumes.iter().sum()
}
