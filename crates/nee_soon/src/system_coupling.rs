// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! # Coupling a 1-D system code to point kinetics (gh:#1084)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! The RELAP-7 Theory Manual §8 (Fig 7; cited, not quoted) lays out a system
//! code exchanging node temperatures and powers with a kinetics code. This is
//! that interface for OUTRAM PARK: a 1-D loop (`tampines`, `tuas`) on one
//! side, `teh-o-prke`'s point kinetics on the other, and nothing in
//! `tampines` depending on either.
//!
//! ## What it reuses
//!
//! - **Feedback:** GeN-Foam's [`ReactivityFeedback`] (Doppler, expansion and
//!   density terms, importance-weighted) evaluated on a [`CouplingRegion`]
//!   whose cells are the loop's core nodes. The loop's node temperatures are
//!   written into the region's fields and the port computes the reactivity;
//!   nothing is re-implemented.
//! - **Kinetics:** `teh_o_prke`'s [`PointKinetics`], advanced by its backward
//!   Euler step.
//! - **Outer iteration:** GeN-Foam's `MultiPhysicsSolver` loop dispatches over
//!   a closed `RegionModel` enum of its own region kinds, so a system code
//!   cannot be added to it without that crate depending on one; the same
//!   Picard pattern (repeat the step until the inter-code residual is below a
//!   tolerance) is applied here, by [`LoopKineticsCoupling::coupled_step`].
//!
//! ## The contract
//!
//! The thermal-hydraulic side implements [`SystemCodeSide`], a compile-time
//! contract (never used as `dyn`): it reports node fields, takes node powers,
//! and advances. It must be `Clone` so a Picard pass can restart the step.
//!
//! Each coupled step, iteration `k`:
//! 1. write the side's end-of-step node fields (the start state on `k = 0`)
//!    into the region and evaluate `ρ = ρ_ext + ρ_fb`;
//! 2. advance the kinetics from the saved start by `dt` with that `ρ`
//!    (backward Euler, so `ρ` is the end-of-step reactivity);
//! 3. restore the side, give it `P f_i` per node, advance `dt`;
//! 4. stop when `|P_k − P_{k−1}|/P_k < tol`.
//!
//! With `max_iterations = 1` this is the loose (operator-split) coupling. At
//! convergence it is the fully implicit backward-Euler solution of the joint
//! system, which is what the V&V compares against a monolithic solve.

use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::multi_region::{CouplingRegion, ReactivityFeedback};
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use teh_o_prke::zero_power_prke::n_group_prke::PointKinetics;
use uom::si::f64::{Power, Ratio, Time, VolumetricNumberDensity, VolumetricNumberRate};
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

/// Errors of the system coupling.
#[derive(Debug, thiserror::Error)]
pub enum SystemCouplingError {
    /// Inconsistent inputs.
    #[error("system coupling: invalid input: {0}")]
    InvalidInput(String),
    /// The kinetics step failed.
    #[error("system coupling: kinetics: {0}")]
    Kinetics(String),
    /// The feedback evaluation failed.
    #[error("system coupling: feedback: {0}")]
    Feedback(String),
    /// The system-code side failed.
    #[error("system coupling: system code: {0}")]
    SystemCode(String),
    /// The Picard iteration did not converge.
    #[error("system coupling: no convergence in {iterations} iterations (residual {residual:e})")]
    NotConverged {
        /// Iterations taken.
        iterations: usize,
        /// Last residual.
        residual: f64,
    },
}

/// What a 1-D system code offers the coupling (a compile-time contract).
pub trait SystemCodeSide: Clone {
    /// The value of node field `name` (e.g. `"TFuel"`, K) at every core node,
    /// in the order of the coupling's nodes.
    ///
    /// # Errors
    ///
    /// When the field is unknown.
    fn node_field(&self, name: &str) -> Result<Vec<f64>, SystemCouplingError>;
    /// Advance `dt` with `node_powers` (W) deposited at the core nodes.
    ///
    /// # Errors
    ///
    /// The code's own failures.
    fn advance(&mut self, dt: Time, node_powers: &[f64]) -> Result<(), SystemCouplingError>;
}

/// A 1-D line mesh of `volumes.len()` cells, for a [`CouplingRegion`] whose
/// cells are loop nodes.
///
/// # Errors
///
/// When there are no cells or a volume is not positive.
pub fn node_mesh(volumes: &[f64]) -> Result<Arc<FvMesh>, SystemCouplingError> {
    let n = volumes.len();
    if n == 0 || volumes.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err(SystemCouplingError::InvalidInput(format!("node volumes {volumes:?}")));
    }
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let neighbour: Vec<usize> = (1..n).collect();
    let mut areas: Vec<Vector3> = (0..n - 1).map(|_| Vector3::new(1.0, 0.0, 0.0)).collect();
    areas.push(Vector3::new(-1.0, 0.0, 0.0));
    areas.push(Vector3::new(1.0, 0.0, 0.0));
    let mut centres: Vec<Vector3> = (0..n - 1).map(|i| Vector3::new(i as f64 + 0.5, 0.0, 0.0)).collect();
    centres.push(Vector3::new(-0.5, 0.0, 0.0));
    centres.push(Vector3::new(n as f64 - 0.5, 0.0, 0.0));
    FvMeshBuilder::new()
        .n_cells(n)
        .n_internal_faces(n - 1)
        .owner(owner)
        .neighbour(neighbour)
        .patches(vec![
            BoundaryPatch::new("a", n - 1, 1, PatchKind::Wall),
            BoundaryPatch::new("b", n, 1, PatchKind::Wall),
        ])
        .cell_volumes(volumes.to_vec())
        .cell_centres((0..n).map(|i| Vector3::new(i as f64, 0.0, 0.0)).collect())
        .face_area_vectors(areas)
        .face_centres(centres)
        .build()
        .map(Arc::new)
        .map_err(|e| SystemCouplingError::InvalidInput(format!("node mesh: {e:?}")))
}

/// Picard controls for [`LoopKineticsCoupling::coupled_step`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PicardControl {
    /// Iterations per step (1 = loose coupling).
    pub max_iterations: usize,
    /// Relative power change at which the step is converged.
    pub tolerance: f64,
}

/// The report of one coupled step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoupledStepReport {
    /// Picard iterations taken.
    pub iterations: usize,
    /// Last relative power change (0 after one iteration).
    pub residual: f64,
    /// End-of-step reactivity.
    pub reactivity: f64,
    /// End-of-step power, W.
    pub power: f64,
}

/// Point kinetics coupled to a system code's core nodes.
#[derive(Debug, Clone)]
pub struct LoopKineticsCoupling {
    kinetics: PointKinetics,
    generation_time: Time,
    /// Neutron density that corresponds to `p0`.
    n0: f64,
    p0: f64,
    power_fractions: Vec<f64>,
    feedback: ReactivityFeedback,
    region: CouplingRegion,
    fields: Vec<String>,
    external_reactivity: f64,
}

impl LoopKineticsCoupling {
    /// Kinetics at critical equilibrium at power `p0`, over core nodes of
    /// `node_volumes` sharing the power by `power_fractions` (normalised),
    /// with `feedback` reading node fields named in it.
    ///
    /// # Errors
    ///
    /// On length mismatches, a non-positive power or fraction sum, or a bad
    /// mesh.
    pub fn new(
        mut kinetics: PointKinetics,
        generation_time: Time,
        p0: Power,
        node_volumes: &[f64],
        power_fractions: &[f64],
        feedback: ReactivityFeedback,
    ) -> Result<Self, SystemCouplingError> {
        let p0 = p0.get::<watt>();
        let sum: f64 = power_fractions.iter().sum();
        if node_volumes.len() != power_fractions.len()
            || !(p0.is_finite() && p0 > 0.0)
            || !(sum > 0.0)
            || power_fractions.iter().any(|f| !(f.is_finite() && *f >= 0.0))
        {
            return Err(SystemCouplingError::InvalidInput(format!(
                "p0 {p0} W, {} volumes, fractions {power_fractions:?}",
                node_volumes.len()
            )));
        }
        let mesh = node_mesh(node_volumes)?;
        let mut region = CouplingRegion::new("core", mesh.clone());
        let mut fields: Vec<String> = feedback.terms().iter().map(|t| t.field_name().to_string()).collect();
        fields.dedup();
        for f in &fields {
            region.insert_scalar(VolScalarField::uniform(f.as_str(), mesh.clone(), 0.0));
        }
        let n0 = 1.0e10;
        kinetics.set_critical_equilibrium(VolumetricNumberDensity::new::<per_cubic_meter>(n0), generation_time);
        Ok(Self {
            kinetics,
            generation_time,
            n0,
            p0,
            power_fractions: power_fractions.iter().map(|f| f / sum).collect(),
            feedback,
            region,
            fields,
            external_reactivity: 0.0,
        })
    }

    /// Set the external (inserted) reactivity, absolute.
    pub fn set_external_reactivity(&mut self, rho: Ratio) {
        self.external_reactivity = rho.get::<ratio>();
    }

    /// The current power.
    pub fn power(&self) -> Power {
        Power::new::<watt>(self.p0 * self.kinetics.neutron_density().get::<per_cubic_meter>() / self.n0)
    }

    /// The current node powers, W.
    pub fn node_powers(&self) -> Vec<f64> {
        let p = self.power().get::<watt>();
        self.power_fractions.iter().map(|f| p * f).collect()
    }

    /// The reactivity for `side`'s current node fields: external plus
    /// feedback.
    ///
    /// # Errors
    ///
    /// A missing field on the side, or the feedback's errors.
    pub fn reactivity_of<S: SystemCodeSide>(&mut self, side: &S) -> Result<f64, SystemCouplingError> {
        let n = self.power_fractions.len();
        for f in &self.fields {
            let v = side.node_field(f)?;
            if v.len() != n {
                return Err(SystemCouplingError::InvalidInput(format!("field {f}: {} values for {n} nodes", v.len())));
            }
            let fld = self
                .region
                .scalar_mut(f)
                .ok_or_else(|| SystemCouplingError::Feedback(format!("no field {f}")))?;
            fld.internal.as_mut_slice().copy_from_slice(&v);
        }
        let fb = self
            .feedback
            .reactivity(&self.region)
            .map_err(|e| SystemCouplingError::Feedback(format!("{e:?}")))?;
        Ok(self.external_reactivity + fb.get::<ratio>())
    }

    /// One coupled step of `dt` (see the module docs).
    ///
    /// # Errors
    ///
    /// The side's, the kinetics' and the feedback's errors;
    /// [`SystemCouplingError::NotConverged`] if more than one iteration was
    /// allowed and the tolerance was not met.
    pub fn coupled_step<S: SystemCodeSide>(
        &mut self,
        side: &mut S,
        dt: Time,
        control: PicardControl,
    ) -> Result<CoupledStepReport, SystemCouplingError> {
        if control.max_iterations == 0 {
            return Err(SystemCouplingError::InvalidInput("max_iterations = 0".into()));
        }
        let (side0, kin0) = (side.clone(), self.kinetics.clone());
        let mut guess = side.clone();
        let (mut p_prev, mut residual, mut rho) = (f64::NAN, 0.0, 0.0);
        for k in 1..=control.max_iterations {
            rho = self.reactivity_of(&guess)?;
            self.kinetics = kin0.clone();
            self.kinetics
                .step_implicit(
                    dt,
                    Ratio::new::<ratio>(rho),
                    self.generation_time,
                    VolumetricNumberRate::new::<per_cubic_meter_second>(0.0),
                )
                .map_err(|e| SystemCouplingError::Kinetics(format!("{e:?}")))?;
            let q = self.node_powers();
            let mut next = side0.clone();
            next.advance(dt, &q)?;
            let p = self.power().get::<watt>();
            residual = if p_prev.is_nan() { 0.0 } else { ((p - p_prev) / p).abs() };
            guess = next;
            if k > 1 && residual < control.tolerance {
                *side = guess;
                return Ok(CoupledStepReport { iterations: k, residual, reactivity: rho, power: p });
            }
            p_prev = p;
        }
        if control.max_iterations > 1 {
            return Err(SystemCouplingError::NotConverged { iterations: control.max_iterations, residual });
        }
        *side = guess;
        Ok(CoupledStepReport { iterations: 1, residual, reactivity: rho, power: self.power().get::<watt>() })
    }

    /// The kinetics (for its constants and state).
    pub fn kinetics(&self) -> &PointKinetics {
        &self.kinetics
    }
}
