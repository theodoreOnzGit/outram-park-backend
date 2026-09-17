// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/multiPhysicsSolver/
//                    (region solver dispatch) driving
//                    src/classes/neutronics/diffusion/ and
//                    src/classes/thermalHydraulics/ per-region correct() calls
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
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

//! # Mesh-based region models (spatial neutronics ↔ thermal-hydraulics)
//!
//! The mesh-based [`RegionModel`](super::outer_iteration::RegionModel) variants
//! that drive the *spatial* physics solvers through the multi-region Picard loop
//! — the piece the `multiRegion` module was scaffolded for. Where the 0-D
//! [`LumpedNeutronics`](super::outer_iteration::LumpedNeutronics) /
//! [`LumpedThermal`](super::outer_iteration::LumpedThermal) carry one cell, these
//! carry a full [`FvMesh`] and exchange **per-cell** fields
//! (`powerDensityNeutronics`, `TFuel`) through the
//! [`MeshHandler`](super::coupling_fields::MeshHandler) mapping.
//!
//! ## [`MeshNeutronics`] — real mesh diffusion with temperature feedback
//!
//! Wraps the ported [`DiffusionNeutronics`] multigroup-diffusion solver
//! (`op-p6p.6`). Each Picard sub-step it:
//!
//! 1. reads the mapped per-cell fuel temperature `TFuel` from its region;
//! 2. collapses it to a single importance-weighted feedback temperature
//!    (via [`ReactivityFeedback::weighted_mean`]) and **re-materialises the cross
//!    sections at that temperature**, so the group constants carry the real
//!    Doppler dependence encoded in the `nuclearData` states (`op-p6p.10`);
//! 3. re-solves the k-eigenvalue for the fundamental-mode flux/power shape;
//! 4. renormalises to a fixed target total power and writes the per-cell
//!    volumetric power density `powerDensityNeutronics` back for the mapping.
//!
//! The feedback is therefore the **physical** cross-section feedback GeN-Foam
//! uses (hot fuel → altered `nu Sigma_f` / `Sigma_r` → lower `k_eff`), not a
//! bolted-on reactivity offset. The residual it reports is the relative change in
//! `k_eff` between Picard iterates.
//!
//! ### Fidelity note — uniform vs. per-cell feedback point
//!
//! [`DiffusionXsFields::materialize`](crate::genfoam::neutronics::diffusion::DiffusionXsFields::materialize)
//! takes **one** feedback vector for the whole mesh, so this model feeds it the
//! importance-weighted mean temperature — a *global* Doppler feedback. Upstream
//! evaluates the cross sections at each cell's *local* temperature. Per-cell
//! feedback needs a `materialize` variant that accepts a per-cell parameter
//! array; that is a neutronics-subtree API addition, tracked as a follow-up on
//! `op-p6p.8.4` (see the crate handoff). The global-feedback path is real and
//! sufficient to close the coupled loop and demonstrate negative feedback.
//!
//! ## [`MeshThermalHydraulics`] — minimal per-cell energy balance (seam)
//!
//! A real but deliberately minimal mesh thermal model: a per-cell backward-Euler
//! fuel-energy balance `ρc_p dT/dt = q''' − (ρc_p/τ)(T − T_cool)` (the coolant
//! coupling is parametrised by a thermal time constant `τ` so every input is a
//! standard named `uom` alias). It reads the mapped `powerDensityNeutronics` and
//! writes `TFuel`, closing the loop across meshes.
//!
//! **This is the integration seam for the full porous / two-phase
//! [`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver
//! (`op-p6p.7`, in progress in the parallel fleet).** Its intended interface is
//! the same coupling contract — consume `powerDensityNeutronics`, produce
//! `TFuel` / `TStruct` / coolant density on its mesh — so when that solver lands
//! it drops into the same [`RegionModel`](super::outer_iteration::RegionModel)
//! seam. Until then this stand-in provides the temperature field the neutronics
//! feedback consumes. See the `TODO(op-p6p.7)` below.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use uom::si::f64::{Power, ThermodynamicTemperature, Time, VolumetricHeatCapacity};
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::xs::CrossSectionData;

use super::coupling_fields::CouplingRegion;
use super::outer_iteration::{CouplingLoopError, RegionKernel};
use super::reactivity_feedback::ReactivityFeedback;

/// Mesh-based neutron-diffusion region with cross-section temperature feedback.
///
/// Owns the multigroup cross sections (`Arc<CrossSectionData>`, re-materialised
/// every sub-step at the mapped feedback temperature), the mesh zoning and flux
/// boundary conditions, the solver settings, and the [`ReactivityFeedback`] used
/// both to collapse the temperature field to a feedback point and to report the
/// equivalent feedback reactivity for diagnostics.
#[derive(Debug, Clone)]
pub struct MeshNeutronics {
    region_name: String,
    /// The multigroup cross sections (shared, read-only; re-materialised each
    /// sub-step onto the region mesh at the current feedback temperature).
    xs: Arc<CrossSectionData>,
    /// Material-zone index of each cell (length `mesh.n_cells`).
    zone_of_cell: Vec<usize>,
    /// Flux boundary condition per mesh patch.
    flux_boundary: Vec<BoundaryCondition<f64>>,
    /// Diffusion convergence / linear-solver settings.
    settings: DiffusionSettings,
    /// Raw feedback-parameter vector in [`CrossSectionData::variable_names`]
    /// order; `feedback_var` (if any) is overwritten each sub-step.
    base_params: Vec<f64>,
    /// Index into `base_params` of the fuel-temperature feedback variable, if the
    /// cross sections are parametrised on one (`None` ⇒ feedback-free XS).
    feedback_var: Option<usize>,
    /// The reactivity-feedback assembly (its weighting also collapses `TFuel`).
    feedback: ReactivityFeedback,
    /// Fixed target total fission power [W] the flux/power is renormalised to.
    target_power: f64,
    /// Current diffusion model (last converged eigenvalue solution).
    model: DiffusionNeutronics,
    /// Last converged `k_eff` (the Picard residual base).
    last_k: f64,
    /// Input fuel-temperature field name.
    t_fuel_field: String,
    /// Output power-density field name.
    power_field: String,
}

impl MeshNeutronics {
    /// Build a mesh-diffusion region.
    ///
    /// # Parameters
    ///
    /// - `region_name` — the coupling region this model owns.
    /// - `mesh` — the neutronics mesh (must be the same `Arc` registered for this
    ///   region in the [`MeshHandler`](super::coupling_fields::MeshHandler)).
    /// - `xs` — the multigroup cross sections (shared).
    /// - `zone_of_cell` — material-zone index of each cell.
    /// - `base_params` — the raw feedback-parameter vector in
    ///   [`CrossSectionData::variable_names`] order (`&[]` for feedback-free XS).
    /// - `feedback_var` — index into `base_params` of the fuel-temperature
    ///   variable to drive from the mapped `TFuel` (`None` ⇒ no XS feedback).
    /// - `flux_boundary` — one boundary condition per mesh patch.
    /// - `settings` — diffusion solver controls.
    /// - `target_power` — fixed total fission power [W] to renormalise to.
    /// - `feedback` — the [`ReactivityFeedback`] assembly (its weighting is
    ///   reused to collapse `TFuel` to the feedback temperature).
    ///
    /// # Errors
    ///
    /// [`CouplingLoopError::Neutronics`] if the initial cross-section
    /// materialisation or the seeding eigenvalue solve fails.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        region_name: &str,
        mesh: Arc<FvMesh>,
        xs: Arc<CrossSectionData>,
        zone_of_cell: Vec<usize>,
        base_params: Vec<f64>,
        feedback_var: Option<usize>,
        flux_boundary: Vec<BoundaryCondition<f64>>,
        settings: DiffusionSettings,
        target_power: Power,
        feedback: ReactivityFeedback,
    ) -> Result<Self, CouplingLoopError> {
        let mut model = DiffusionNeutronics::new(
            mesh,
            &xs,
            &zone_of_cell,
            &base_params,
            &flux_boundary,
            settings,
        )
        .map_err(|e| CouplingLoopError::Neutronics(e.to_string()))?;
        let report = model
            .solve_eigenvalue()
            .map_err(|e| CouplingLoopError::Neutronics(e.to_string()))?;
        Ok(Self {
            region_name: region_name.to_string(),
            xs,
            zone_of_cell,
            flux_boundary,
            settings,
            base_params,
            feedback_var,
            feedback,
            target_power: target_power.get::<watt>(),
            last_k: report.k_eff,
            model,
            t_fuel_field: "TFuel".to_string(),
            power_field: "powerDensityNeutronics".to_string(),
        })
    }

    /// The current effective multiplication factor `k_eff`.
    #[must_use]
    pub fn k_eff(&self) -> f64 {
        self.model.state().k_eff_raw()
    }

    /// The fixed total fission power the model renormalises to [W].
    #[must_use]
    pub fn fission_power(&self) -> Power {
        Power::new::<watt>(self.target_power)
    }

    /// The diffusion model (flux / precursors / power density / `k_eff`).
    #[must_use]
    pub fn model(&self) -> &DiffusionNeutronics {
        &self.model
    }

    /// The equivalent feedback reactivity `Δρ` the mapped fields imply, via the
    /// configured [`ReactivityFeedback`] (a diagnostic that ties the mesh XS
    /// feedback to the point-kinetics-style reactivity of `op-p6p.10`).
    ///
    /// # Errors
    ///
    /// [`CouplingLoopError::MissingField`] if a feedback field is absent.
    pub fn feedback_reactivity(
        &self,
        region: &CouplingRegion,
    ) -> Result<uom::si::f64::Ratio, CouplingLoopError> {
        self.feedback.reactivity(region)
    }
}

impl RegionKernel for MeshNeutronics {
    fn correct(
        &mut self,
        region: &mut CouplingRegion,
        _dt: Time,
        _tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        // 1. Collapse the mapped per-cell TFuel to a feedback temperature and
        //    inject it into the XS parameter vector.
        let mut params = self.base_params.clone();
        if let Some(idx) = self.feedback_var {
            let t_mean = self.feedback.weighted_mean(region, &self.t_fuel_field)?;
            if idx < params.len() {
                params[idx] = t_mean;
            }
        }

        // 2. Re-materialise the cross sections at the feedback point and re-solve
        //    the eigenvalue for the fundamental-mode flux/power shape.
        let mut model = DiffusionNeutronics::new(
            region.mesh().clone(),
            &self.xs,
            &self.zone_of_cell,
            &params,
            &self.flux_boundary,
            self.settings,
        )
        .map_err(|e| CouplingLoopError::Neutronics(e.to_string()))?;
        let report = model
            .solve_eigenvalue()
            .map_err(|e| CouplingLoopError::Neutronics(e.to_string()))?;
        let k_new = report.k_eff;

        // 3. Renormalise to the fixed target total power and write per-cell q'''.
        let p_eigen = model.state().total_power_raw();
        let scale = if p_eigen.abs() > 0.0 {
            self.target_power / p_eigen
        } else {
            0.0
        };
        let pd = model.state().power_density().internal.as_slice().to_vec();
        let out = region.scalar_mut(&self.power_field).ok_or_else(|| {
            CouplingLoopError::MissingField {
                region: self.region_name.clone(),
                field: self.power_field.clone(),
            }
        })?;
        for (o, p) in out.internal.as_mut_slice().iter_mut().zip(pd.iter()) {
            *o = p * scale;
        }

        self.model = model;
        let residual = (k_new - self.last_k).abs() / self.last_k.abs().max(1e-30);
        self.last_k = k_new;
        Ok(residual)
    }

    fn region_name(&self) -> &str {
        &self.region_name
    }
}

/// Mesh-based thermal-hydraulics region — a per-cell fuel-energy balance.
///
/// A real but minimal spatial thermal model closing the loop across meshes:
/// each cell solves the backward-Euler balance
/// `ρc_p dT/dt = q''' − (ρc_p/τ)(T − T_cool)`, reading the mapped per-cell
/// `powerDensityNeutronics` `q'''` [W/m³] and writing `TFuel` [K]. `τ` is the
/// fuel-to-coolant thermal time constant — expressing the coolant coupling as a
/// time constant keeps every input a standard named `uom` alias (no exotic
/// volumetric-conductance type). At steady state `T = T_cool + q''' τ/ρc_p`, so
/// deposited power heats the fuel above the coolant temperature.
///
/// TODO(op-p6p.7): replace this stand-in with the full porous single-/two-phase
/// [`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver when it
/// lands. Its coupling contract is identical (consume `powerDensityNeutronics`,
/// produce `TFuel` / `TStruct` / coolant density on this mesh), so it slots into
/// the same [`RegionModel`](super::outer_iteration::RegionModel) seam.
#[derive(Debug, Clone)]
pub struct MeshThermalHydraulics {
    region_name: String,
    /// Volumetric heat capacity `ρc_p` [J/(m³·K)].
    rho_cp: VolumetricHeatCapacity,
    /// Fuel-to-coolant thermal time constant `τ` [s].
    tau: Time,
    /// Coolant (sink) temperature [K].
    t_coolant: ThermodynamicTemperature,
    /// Per-cell fuel temperature at the start of the current outer step [K].
    t_step_start: Vec<f64>,
    /// Current per-cell fuel temperature [K].
    t_cur: Vec<f64>,
    /// Input power-density field name.
    power_field: String,
    /// Output fuel-temperature field name.
    t_fuel_field: String,
}

impl MeshThermalHydraulics {
    /// Build a per-cell thermal region on a mesh of `n_cells` at a uniform
    /// initial temperature.
    ///
    /// # Parameters
    ///
    /// - `region_name` — the coupling region this model owns.
    /// - `n_cells` — number of cells on the region mesh.
    /// - `rho_cp` — volumetric heat capacity `ρc_p` [J/(m³·K)].
    /// - `tau` — fuel-to-coolant thermal time constant `τ` [s] (larger ⇒ weaker
    ///   coupling to the coolant ⇒ hotter fuel for a given power density).
    /// - `t_coolant` — coolant sink temperature.
    /// - `t_initial` — uniform initial fuel temperature.
    pub fn new(
        region_name: &str,
        n_cells: usize,
        rho_cp: VolumetricHeatCapacity,
        tau: Time,
        t_coolant: ThermodynamicTemperature,
        t_initial: ThermodynamicTemperature,
    ) -> Self {
        let t0 = t_initial.get::<kelvin>();
        Self {
            region_name: region_name.to_string(),
            rho_cp,
            tau,
            t_coolant,
            t_step_start: vec![t0; n_cells],
            t_cur: vec![t0; n_cells],
            power_field: "powerDensityNeutronics".to_string(),
            t_fuel_field: "TFuel".to_string(),
        }
    }

    /// The current per-cell fuel temperature [K].
    #[must_use]
    pub fn temperatures(&self) -> &[f64] {
        &self.t_cur
    }

    /// The volume-mean fuel temperature.
    #[must_use]
    pub fn mean_temperature(&self) -> ThermodynamicTemperature {
        let n = self.t_cur.len().max(1);
        ThermodynamicTemperature::new::<kelvin>(self.t_cur.iter().sum::<f64>() / n as f64)
    }

    /// Snapshot the temperature at the start of an outer time step.
    pub fn begin_step(&mut self) {
        self.t_step_start.copy_from_slice(&self.t_cur);
    }
}

impl RegionKernel for MeshThermalHydraulics {
    fn correct(
        &mut self,
        region: &mut CouplingRegion,
        dt: Time,
        _tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        let q = region
            .scalar(&self.power_field)
            .ok_or_else(|| CouplingLoopError::MissingField {
                region: self.region_name.clone(),
                field: self.power_field.clone(),
            })?
            .internal
            .as_slice()
            .to_vec();

        let rho_cp = self.rho_cp.get::<joule_per_cubic_meter_kelvin>();
        let inv_dt = 1.0 / dt.get::<second>();
        let inv_tau = 1.0 / self.tau.get::<second>();
        let t_cool = self.t_coolant.get::<kelvin>();

        let mut max_rel = 0.0_f64;
        for (c, &qc) in q.iter().enumerate() {
            let t_old = self.t_step_start[c];
            // dT/dt = q'''/ρc_p − (T − T_cool)/τ, backward Euler:
            // T_new (1/dt + 1/τ) = T_old/dt + q'''/ρc_p + T_cool/τ.
            let t_new = (t_old * inv_dt + qc / rho_cp + t_cool * inv_tau) / (inv_dt + inv_tau);
            let rel = (t_new - self.t_cur[c]).abs() / self.t_cur[c].abs().max(1e-30);
            max_rel = max_rel.max(rel);
            self.t_cur[c] = t_new;
        }

        let out = region.scalar_mut(&self.t_fuel_field).ok_or_else(|| {
            CouplingLoopError::MissingField {
                region: self.region_name.clone(),
                field: self.t_fuel_field.clone(),
            }
        })?;
        out.internal.as_mut_slice().copy_from_slice(&self.t_cur);
        Ok(max_rel)
    }

    fn region_name(&self) -> &str {
        &self.region_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use outram_foam_basic_lib::fields::vol_field::VolScalarField;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

    use crate::genfoam::neutronics::xs::input::{
        NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
    };
    use crate::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};

    use super::super::coupling_fields::{CouplingField, CouplingRegion, MeshHandler};
    use super::super::mesh_to_mesh::MappingMethod;
    use super::super::outer_iteration::{FeedbackCoefficient, MultiPhysicsSolver, RegionModel};
    use super::super::reactivity_feedback::{DopplerLaw, ReactivityFeedback};

    const T_REF: f64 = 900.0;
    const T_HOT: f64 = 1800.0;

    fn slab(n: i64) -> Arc<FvMesh> {
        Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
                .unwrap(),
        )
    }

    /// One-group, one-zone XS with a single Doppler variable (`Tfuel`, log law):
    /// `nu Sigma_f` falls from 0.30 (900 K) to 0.27 (1800 K) — a negative Doppler
    /// dependence, so `k_eff` drops as the fuel heats.
    fn doppler_xs() -> CrossSectionData {
        let constants = ZoneConstantsInput {
            iv: vec![1.0e-6],
            disc_factor: vec![1.0],
            integral_flux: vec![1.0],
            beta: vec![0.0065],
            lambda: vec![0.08],
            ..ZoneConstantsInput::default()
        };
        let core_ref = ZoneStateInput {
            name: "core".to_string(),
            d: vec![0.02],
            nu_sigma_eff: vec![0.30],
            sigma_pow: vec![1.0],
            sigma_removal: vec![0.10],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![vec![0.0]]],
            constants: Some(constants),
        };
        let core_hot = ZoneStateInput {
            name: "core".to_string(),
            d: vec![0.02],
            nu_sigma_eff: vec![0.27],
            sigma_pow: vec![1.0],
            sigma_removal: vec![0.10],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![vec![0.0]]],
            constants: None,
        };
        let mut ref_params = BTreeMap::new();
        ref_params.insert("Tfuel".to_string(), T_REF);
        let mut hot_params = BTreeMap::new();
        hot_params.insert("Tfuel".to_string(), T_HOT);
        let input = NuclearDataInput {
            energy_groups: 1,
            prec_groups: 1,
            legendre_moments: 1,
            poly_spline_mode: 1,
            fast_neutrons: false,
            xs_variables: vec![XsVariable::new("Tfuel", VariableLaw::Log)],
            do_not_parametrize: Vec::new(),
            states: vec![
                StateInput {
                    name: "reference".to_string(),
                    parameters: ref_params,
                    zones: vec![core_ref],
                },
                StateInput {
                    name: "Tfuel1800".to_string(),
                    parameters: hot_params,
                    zones: vec![core_hot],
                },
            ],
        };
        CrossSectionData::from_input(&input).expect("valid doppler XS")
    }

    fn reflective_bc() -> Vec<BoundaryCondition<f64>> {
        vec![
            BoundaryCondition::ZeroGradient,
            BoundaryCondition::ZeroGradient,
        ]
    }

    fn build_mesh_neutronics(mesh: Arc<FvMesh>, t_ref: f64) -> MeshNeutronics {
        let xs = Arc::new(doppler_xs());
        let feedback = ReactivityFeedback::new().with_doppler(
            "TFuel",
            FeedbackCoefficient::new::<per_kelvin>(1.0e-5),
            ThermodynamicTemperature::new::<kelvin>(t_ref),
            DopplerLaw::Linear,
        );
        MeshNeutronics::new(
            "neutro",
            mesh.clone(),
            xs,
            vec![0usize; mesh.n_cells],
            vec![t_ref],
            Some(0),
            reflective_bc(),
            DiffusionSettings::default(),
            Power::new::<watt>(1.0e6),
            feedback,
        )
        .expect("build mesh neutronics")
    }

    /// V&V — mesh cross-section (Doppler) feedback: heating the fuel lowers
    /// `k_eff`.
    ///
    /// Methodology: an infinite-medium (reflective) one-group slab whose
    /// `nu Sigma_f` falls with temperature. Materialise + solve the eigenvalue at
    /// the reference `T = 900 K` and at a hot `T = 1800 K` feedback point (the
    /// `k_inf = nu Sigma_f / Sigma_a` case, so `k` is exactly `nu Sigma_f/0.10`).
    /// The hot state must give a strictly lower `k_eff`.
    ///
    /// Result (2026-07-15): `k_eff(900 K) = 3.000000` (νΣf 0.30/Σa 0.10),
    /// `k_eff(1800 K) = 2.700000` (νΣf 0.27) — a −0.300 drop (−10 %), the encoded
    /// negative Doppler dependence. Pass — mesh XS feedback is negative.
    #[test]
    fn mesh_doppler_feedback_lowers_keff() {
        let mesh = slab(4);
        let mut n_cold = build_mesh_neutronics(mesh.clone(), T_REF);
        let mut region = CouplingRegion::new("neutro", mesh.clone());
        region.insert_scalar(VolScalarField::uniform("TFuel", mesh.clone(), T_REF));
        region.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            mesh.clone(),
            0.0,
        ));
        // Cold feedback point.
        n_cold
            .correct(&mut region, Time::new::<second>(1.0), false)
            .unwrap();
        let k_cold = n_cold.k_eff();

        // Hot feedback point: overwrite TFuel to the hot temperature and re-correct.
        for v in region.scalar_mut("TFuel").unwrap().internal.as_mut_slice() {
            *v = T_HOT;
        }
        n_cold
            .correct(&mut region, Time::new::<second>(1.0), false)
            .unwrap();
        let k_hot = n_cold.k_eff();

        eprintln!(
            "k_cold {k_cold:.6}, k_hot {k_hot:.6}, drop {:.6}",
            k_cold - k_hot
        );
        assert!((k_cold - 3.0).abs() < 1e-6, "k_cold {k_cold}");
        assert!((k_hot - 2.7).abs() < 1e-6, "k_hot {k_hot}");
        assert!(k_hot < k_cold, "Doppler feedback must lower k_eff");
    }

    /// V&V — power-density conservation across a non-conformal mesh map, in the
    /// coupled loop.
    ///
    /// Methodology: neutronics on a 20-cell slab, thermal-hydraulics on a coarser
    /// 8-cell slab. The neutronics renormalises to a fixed 1 MW total; the
    /// `CellVolumeWeight` mapping transfers `powerDensityNeutronics` onto the TH
    /// mesh conservatively. After one coupled step the volume integral
    /// `∑ V q'''` on the TH mesh must equal the 1 MW deposited on the neutronics
    /// mesh (both are `1 m³` total, so `q'''` integrates to the total power).
    ///
    /// Result (2026-07-15): neutronics ∫q''' = 1.000000e6 W, TH ∫q''' =
    /// 1.000000e6 W, relative mismatch 2.3e-16. Pass — the map conserves power.
    #[test]
    fn coupled_power_conservation_across_meshes() {
        let neutro_mesh = slab(20);
        let th_mesh = slab(8);

        let neutronics = build_mesh_neutronics(neutro_mesh.clone(), T_REF);
        let thermal = MeshThermalHydraulics::new(
            "fluid",
            th_mesh.n_cells,
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.0e5),
            Time::new::<second>(50.0),
            ThermodynamicTemperature::new::<kelvin>(T_REF),
            ThermodynamicTemperature::new::<kelvin>(T_REF),
        );

        let mut neutro = CouplingRegion::new("neutro", neutro_mesh.clone());
        neutro.insert_scalar(VolScalarField::uniform("TFuel", neutro_mesh.clone(), T_REF));
        neutro.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            neutro_mesh.clone(),
            0.0,
        ));
        let mut fluid = CouplingRegion::new("fluid", th_mesh.clone());
        fluid.insert_scalar(VolScalarField::uniform("TFuel", th_mesh.clone(), T_REF));
        fluid.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            th_mesh.clone(),
            0.0,
        ));

        let mut handler = MeshHandler::new();
        handler.add_region(neutro).unwrap();
        handler.add_region(fluid).unwrap();
        handler
            .add_mapping(
                "fluid",
                "neutro",
                MappingMethod::CellVolumeWeight,
                vec![CouplingField::scalar(
                    "powerDensityNeutronics",
                    "powerDensityNeutronics",
                )],
            )
            .unwrap();
        handler
            .add_mapping(
                "neutro",
                "fluid",
                MappingMethod::CellVolumeWeight,
                vec![CouplingField::scalar("TFuel", "TFuel")],
            )
            .unwrap();

        let mut solver = MultiPhysicsSolver::new(
            handler,
            vec![
                RegionModel::MeshNeutronics(neutronics),
                RegionModel::MeshThermalHydraulics(thermal),
            ],
            1.0e-6,
            50,
        );
        solver.solve(Time::new::<second>(1.0)).unwrap();

        let p_neutro = solver
            .handler()
            .scalar_integral("neutro", "powerDensityNeutronics")
            .unwrap();
        let p_th = solver
            .handler()
            .scalar_integral("fluid", "powerDensityNeutronics")
            .unwrap();
        let rel = (p_neutro - p_th).abs() / p_neutro;
        eprintln!("neutro ∫q''' {p_neutro:.6e} W, TH ∫q''' {p_th:.6e} W, rel {rel:.2e}");
        assert!(
            (p_neutro - 1.0e6).abs() / 1.0e6 < 1e-9,
            "neutro power {p_neutro}"
        );
        assert!(rel < 1e-9, "power not conserved across map: rel {rel}");
    }

    /// V&V — the coupled mesh loop reaches a self-consistent steady state and the
    /// Doppler feedback opposes the heating.
    ///
    /// Methodology: run the 20-cell-neutronics / 8-cell-TH coupled system to
    /// convergence over a 10 ms step. As the deposited power heats the fuel, the
    /// mean feedback temperature rises above `T_ref`, so the equivalent feedback
    /// reactivity (via [`ReactivityFeedback`]) must be negative and `k_eff` must
    /// fall below its cold reference value.
    ///
    /// Result (2026-07-15): converged within the 50-iteration budget; the coupled
    /// steady state settled at mean TFuel = 909.80 K (above `T_ref` = 900 K),
    /// equivalent feedback reactivity Δρ = −9.804e-5 (−9.8 pcm, negative), and
    /// k_eff = 2.995311 (below the cold value 3.0). Pass — the mesh Doppler
    /// feedback opposes the power-driven heating.
    #[test]
    fn coupled_steady_state_negative_feedback() {
        let neutro_mesh = slab(20);
        let th_mesh = slab(8);
        let neutronics = build_mesh_neutronics(neutro_mesh.clone(), T_REF);
        let thermal = MeshThermalHydraulics::new(
            "fluid",
            th_mesh.n_cells,
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.0e5),
            Time::new::<second>(50.0),
            ThermodynamicTemperature::new::<kelvin>(T_REF),
            ThermodynamicTemperature::new::<kelvin>(T_REF),
        );
        let mut neutro = CouplingRegion::new("neutro", neutro_mesh.clone());
        neutro.insert_scalar(VolScalarField::uniform("TFuel", neutro_mesh.clone(), T_REF));
        neutro.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            neutro_mesh.clone(),
            0.0,
        ));
        let mut fluid = CouplingRegion::new("fluid", th_mesh.clone());
        fluid.insert_scalar(VolScalarField::uniform("TFuel", th_mesh.clone(), T_REF));
        fluid.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            th_mesh.clone(),
            0.0,
        ));
        let mut handler = MeshHandler::new();
        handler.add_region(neutro).unwrap();
        handler.add_region(fluid).unwrap();
        handler
            .add_mapping(
                "fluid",
                "neutro",
                MappingMethod::CellVolumeWeight,
                vec![CouplingField::scalar(
                    "powerDensityNeutronics",
                    "powerDensityNeutronics",
                )],
            )
            .unwrap();
        handler
            .add_mapping(
                "neutro",
                "fluid",
                MappingMethod::CellVolumeWeight,
                vec![CouplingField::scalar("TFuel", "TFuel")],
            )
            .unwrap();
        let mut solver = MultiPhysicsSolver::new(
            handler,
            vec![
                RegionModel::MeshNeutronics(neutronics),
                RegionModel::MeshThermalHydraulics(thermal),
            ],
            1.0e-6,
            50,
        );
        let iters = solver.solve(Time::new::<second>(1.0)).unwrap();
        assert!(iters <= 50, "did not converge, {iters} iters");

        let RegionModel::MeshNeutronics(n) = solver.model(0).unwrap() else {
            panic!("model 0 is mesh neutronics");
        };
        let neutro_region = solver.handler().region("neutro").unwrap();
        let t_mean = n.feedback.weighted_mean(neutro_region, "TFuel").unwrap();
        let rho = n.feedback_reactivity(neutro_region).unwrap().get::<ratio>();
        eprintln!(
            "steady: k_eff {:.6}, mean TFuel {t_mean:.2} K, feedback Δρ {rho:.3e}",
            n.k_eff()
        );
        assert!(t_mean > T_REF, "fuel should heat above T_ref, got {t_mean}");
        assert!(rho < 0.0, "Doppler feedback must be negative, got {rho}");
        assert!(n.k_eff() < 3.0, "k_eff should fall below cold value 3.0");

        // Sanity: the coupled state is truly self-consistent (residual small).
        assert!(
            *solver.residual_history().last().unwrap() < 1.0e-6,
            "final residual too high"
        );
    }
}
