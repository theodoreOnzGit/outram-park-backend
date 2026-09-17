//! # Xin Wang SP3 multiphysics workflow (Figure-4.29 reproduction)
//!
//! Scaffold of the four-stage reactor-multiphysics pipeline that reproduces
//! **Figure 4.29** — the maximum fuel temperature during a control-rod-removal
//! transient — of Xin Wang's 2018 UC Berkeley PhD dissertation *"Coupled
//! neutronics and thermal-hydraulics modeling for pebble-bed FHR"*
//! (<https://escholarship.org/uc/item/40q3985m>, open literature). The extracted
//! methodology and case data live in the crate's `docs/xin-wang-thesis/`.
//!
//! Wang used **Serpent** (Monte Carlo) + **COMSOL** (SP3 via user-defined PDEs).
//! OUTRAM PARK re-implements that on **njoy → openmc → genfoam**. This module is
//! the coupling driver; it composes the public APIs of the data / transport /
//! neutronics crates — it does not re-implement any physics kernel.
//!
//! ## Module map
//!
//! | Item | Role |
//! |---|---|
//! | [`case`] | typed Mk1 case data: 8-group structure, transient definition, digitised Fig. 4.29 curve |
//! | [`mgxs::MgxsGenerationStage`] | **Stage 1** — 8-group MGXS from ENDF via `njoy` (bead op-fr2.2.2) |
//! | [`mesh_mc::MeshMonteCarloStage`] | **Stage 2** — Mk1 mesh + Monte Carlo model + MGXS/power tallies via `outram-mc` (bead op-fr2.2.3) |
//! | [`sp3_multiphysics::Sp3MultiphysicsStage`] | **Stage 3** — GeN-Foam SP3 neutronics + porous-media TH transient (bead op-fr2.2.4) |
//! | [`validation::Fig429ValidationStage`] | **Stage 4** — compare vs the Fig. 4.29 reference (bead op-fr2.2.5) |
//! | [`XinWangSp3Workflow`] | the driver that owns all four stages in pipeline order |
//!
//! ## Status — scaffold only
//!
//! Every stage's `run()` is a documented **placeholder** that returns
//! [`WorkflowError::NotYetImplemented`] naming its tracking bead. No MGXS is
//! generated, no MC model built, no SP3 transient run, and Fig. 4.29 is **not**
//! reproduced. The scaffold exists so the coupling surface compiles and each
//! stage is a navigable, beaded Rust type. The dependency ordering is
//! **MGXS → mesh → SP3 → validation**; several stages are blocked on capabilities
//! still being built in `njoy-outram-park-fork`, `outram-mc-libs`, and the
//! in-progress GeN-Foam SP3 port (see each stage's bead references).

pub mod case;
pub mod mesh_mc;
pub mod mgxs;
pub mod sp3_multiphysics;
pub mod validation;

use mesh_mc::MeshMonteCarloStage;
use mgxs::MgxsGenerationStage;
use sp3_multiphysics::Sp3MultiphysicsStage;
use validation::Fig429ValidationStage;

/// Which stage of the workflow an error or status refers to. Pipeline order is
/// the declaration order below (MGXS → mesh → SP3 → validation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStage {
    /// Stage 1 — multigroup cross-section generation (njoy).
    MgxsGeneration,
    /// Stage 2 — mesh + Monte Carlo model (openmc / outram-mc).
    MeshAndMonteCarlo,
    /// Stage 3 — SP3 multiphysics transient (genfoam).
    Sp3Multiphysics,
    /// Stage 4 — Figure-4.29 comparison (validation target).
    Fig429Validation,
}

/// Error type for the Xin Wang SP3 workflow scaffold.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    /// A workflow stage is scaffolded but not yet implemented. Carries the
    /// tracking bead so the caller knows where the work is queued.
    #[error("workflow stage {stage:?} is not yet implemented — tracked by bead {bead}")]
    NotYetImplemented {
        /// The stage that is not yet implemented.
        stage: WorkflowStage,
        /// The beads issue id tracking that stage's implementation.
        bead: &'static str,
    },
}

/// The four-stage Xin Wang SP3 multiphysics workflow driver.
///
/// Owns one instance of each stage in pipeline order. Constructing it wires the
/// Mk1 case data into every stage; running the pipeline is future work (each
/// stage's `run()` is a placeholder — see the module-level status note).
///
/// # Example
///
/// ```
/// use nee_soon::xin_wang_sp3_workflow::{XinWangSp3Workflow, WorkflowStage};
///
/// let workflow = XinWangSp3Workflow::new();
///
/// // The case data is real and available now:
/// assert_eq!(workflow.mgxs().group_lower_bounds().len(), 8);
///
/// // Running any stage is a documented placeholder for now:
/// let err = workflow.mgxs().run().unwrap_err();
/// assert!(matches!(
///     err,
///     nee_soon::xin_wang_sp3_workflow::WorkflowError::NotYetImplemented {
///         stage: WorkflowStage::MgxsGeneration,
///         ..
///     }
/// ));
/// ```
#[derive(Debug, Clone, Default)]
pub struct XinWangSp3Workflow {
    mgxs: MgxsGenerationStage,
    mesh_mc: MeshMonteCarloStage,
    sp3: Sp3MultiphysicsStage,
    validation: Fig429ValidationStage,
}

impl XinWangSp3Workflow {
    /// Creates the workflow with all four stages wired to the Mk1 case data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage 1 — MGXS generation (njoy).
    pub fn mgxs(&self) -> &MgxsGenerationStage {
        &self.mgxs
    }

    /// Stage 2 — mesh + Monte Carlo (openmc / outram-mc).
    pub fn mesh_mc(&self) -> &MeshMonteCarloStage {
        &self.mesh_mc
    }

    /// Stage 3 — SP3 multiphysics (genfoam).
    pub fn sp3_multiphysics(&self) -> &Sp3MultiphysicsStage {
        &self.sp3
    }

    /// Stage 4 — Figure-4.29 validation.
    pub fn validation(&self) -> &Fig429ValidationStage {
        &self.validation
    }

    /// Run the full pipeline in order (MGXS → mesh → SP3 → validation).
    ///
    /// # Placeholder
    ///
    /// Returns the first stage's [`WorkflowError::NotYetImplemented`]. Once the
    /// stages are implemented this will thread each stage's output into the next.
    // TODO(op-fr2.2): thread MGXS -> mesh -> SP3 -> validation outputs once the
    // per-stage runs are implemented (op-fr2.2.2 .. op-fr2.2.5).
    pub fn run(&self) -> Result<(), WorkflowError> {
        self.mgxs.run()?;
        self.mesh_mc.run()?;
        self.sp3.run()?;
        self.validation.run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_exposes_eight_group_structure() {
        let workflow = XinWangSp3Workflow::new();
        assert_eq!(
            workflow.mgxs().group_lower_bounds().len(),
            case::NUM_ENERGY_GROUPS
        );
        assert_eq!(case::NUM_ENERGY_GROUPS, 8);
    }

    #[test]
    fn every_stage_run_is_a_beaded_placeholder() {
        let w = XinWangSp3Workflow::new();
        for err in [
            w.mgxs().run().unwrap_err(),
            w.mesh_mc().run().unwrap_err(),
            w.sp3_multiphysics().run().unwrap_err(),
            w.validation().run().unwrap_err(),
        ] {
            let WorkflowError::NotYetImplemented { bead, .. } = err;
            assert!(bead.starts_with("op-"), "each placeholder names its bead");
        }
    }

    #[test]
    fn stage_three_targets_sp3_neutronics() {
        use outram_foam_appbuilder_lib::genfoam::neutronics::NeutronicsModelKind;
        let w = XinWangSp3Workflow::new();
        assert_eq!(
            w.sp3_multiphysics().target_neutronics_kind(),
            NeutronicsModelKind::Sp3
        );
    }

    #[test]
    fn fig_4_29_reference_curve_is_loaded_and_below_safety_limit() {
        use uom::si::thermodynamic_temperature::degree_celsius;
        let w = XinWangSp3Workflow::new();
        let curve = w.validation().reference_curve();
        assert!(!curve.is_empty());
        let limit = case::fuel_safety_limit().get::<degree_celsius>();
        for p in curve {
            assert!(
                p.max_fuel_temperature.get::<degree_celsius>() < limit,
                "Fig 4.29 max fuel temp stays below the 1600 C safety limit"
            );
        }
    }
}
