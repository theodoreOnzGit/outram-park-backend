// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/workflowControls/workflowControls.{C,H}
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`workflowControls`** (gh:#1272): `stopAfter` and
//! `restartFromLatestStep` for the generators' steps.
//!
//! A generator asks [`WorkflowControls::run_current_step`] before each step.
//! The call first closes the **previous** step: it is recorded as completed
//! in the mesh's meta data, and if it is the `stopAfter` step the workflow
//! stops there (upstream writes the mesh and throws a string the generator
//! catches, so `renumberMesh` and `renameBoundaryPatches` do not run).
//!
//! **Where the port differs.** Upstream keeps the meta data in the mesh on
//! disk and, on restart, reads the mesh written after the last completed
//! step. Here the meta data is a value ([`MeshMetaData`]) returned with the
//! mesh, and a restart is given that mesh and meta data back
//! ([`StepDecision::RunAfterLoading`]). Step names are an enum, so
//! upstream's "is not a valid name" `FatalError` cannot happen.
//!
//! Upstream quirks kept: a step is appended to `completedSteps` twice when
//! the workflow does not stop after it (`stopAfterCurrentStep` and
//! `runCurrentStep` each call `setStepCompleted`); `stopAfter` the last step
//! of a generator never triggers, because no later `runCurrentStep` call
//! closes it.

/// The steps of `workflowSteps_`, with upstream's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkflowStep {
    /// `start` (0).
    Start,
    /// `templateGeneration` (1).
    TemplateGeneration,
    /// `surfaceTopology` (2).
    SurfaceTopology,
    /// `surfaceProjection` (4).
    SurfaceProjection,
    /// `patchAssignment` (8).
    PatchAssignment,
    /// `edgeExtraction` (16).
    EdgeExtraction,
    /// `meshOptimisation` (32).
    MeshOptimisation,
    /// `boundaryLayerGeneration` (64).
    BoundaryLayerGeneration,
    /// `boundaryLayerRefinement` (128).
    BoundaryLayerRefinement,
}

impl WorkflowStep {
    /// Every step, in `workflowSteps_` order.
    pub const ALL: [WorkflowStep; 9] = [
        WorkflowStep::Start,
        WorkflowStep::TemplateGeneration,
        WorkflowStep::SurfaceTopology,
        WorkflowStep::SurfaceProjection,
        WorkflowStep::PatchAssignment,
        WorkflowStep::EdgeExtraction,
        WorkflowStep::MeshOptimisation,
        WorkflowStep::BoundaryLayerGeneration,
        WorkflowStep::BoundaryLayerRefinement,
    ];

    /// The `meshDict` name.
    pub fn name(self) -> &'static str {
        match self {
            WorkflowStep::Start => "start",
            WorkflowStep::TemplateGeneration => "templateGeneration",
            WorkflowStep::SurfaceTopology => "surfaceTopology",
            WorkflowStep::SurfaceProjection => "surfaceProjection",
            WorkflowStep::PatchAssignment => "patchAssignment",
            WorkflowStep::EdgeExtraction => "edgeExtraction",
            WorkflowStep::MeshOptimisation => "meshOptimisation",
            WorkflowStep::BoundaryLayerGeneration => "boundaryLayerGeneration",
            WorkflowStep::BoundaryLayerRefinement => "boundaryLayerRefinement",
        }
    }

    /// Upstream's value in `workflowSteps_`.
    pub fn value(self) -> u32 {
        match self {
            WorkflowStep::Start => 0,
            WorkflowStep::TemplateGeneration => 1,
            WorkflowStep::SurfaceTopology => 2,
            WorkflowStep::SurfaceProjection => 4,
            WorkflowStep::PatchAssignment => 8,
            WorkflowStep::EdgeExtraction => 16,
            WorkflowStep::MeshOptimisation => 32,
            WorkflowStep::BoundaryLayerGeneration => 64,
            WorkflowStep::BoundaryLayerRefinement => 128,
        }
    }

    /// The step of a `meshDict` name, `None` for an invalid one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.name() == name)
    }
}

/// `meshDict`'s `workflowControls` sub-dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkflowSettings {
    /// `stopAfter`.
    pub stop_after: Option<WorkflowStep>,
    /// `restartFromLatestStep`.
    pub restart_from_latest_step: bool,
}

/// The workflow entries of the mesh's meta data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeshMetaData {
    /// `lastStep`.
    pub last_step: Option<WorkflowStep>,
    /// `completedSteps` (with upstream's repeats).
    pub completed_steps: Vec<WorkflowStep>,
}

/// What a generator does with the step it asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDecision {
    /// Run the step (`true`).
    Run,
    /// The step ran before the restart; skip it (`false`).
    Skip,
    /// Load the mesh saved after the previous step, then run this one
    /// (`runAfterCurrentStep` returned `true`).
    RunAfterLoading,
    /// The previous step was `stopAfter`: stop with the mesh as it is (the
    /// thrown string upstream).
    Stop,
}

/// `workflowControls`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowControls {
    /// The `meshDict` settings.
    pub settings: WorkflowSettings,
    /// The mesh's meta data.
    pub meta: MeshMetaData,
    /// `currentStep_`.
    pub current_step: WorkflowStep,
    restart_after_step: Option<WorkflowStep>,
    completed_steps_before_restart: Vec<WorkflowStep>,
    is_restarted: bool,
}

impl WorkflowControls {
    /// `workflowControls(mesh)`: with a restart requested, the restart point
    /// is the mesh's last completed step; otherwise the completed steps are
    /// cleared.
    pub fn new(settings: WorkflowSettings, meta: MeshMetaData) -> Self {
        let mut w = WorkflowControls {
            settings,
            meta,
            current_step: WorkflowStep::Start,
            restart_after_step: None,
            completed_steps_before_restart: Vec::new(),
            is_restarted: false,
        };
        if settings.restart_from_latest_step {
            w.restart_after_step = w.meta.last_step;
            w.completed_steps_before_restart = w.meta.completed_steps.clone();
        } else {
            w.meta = MeshMetaData::default();
        }
        w
    }

    /// `setStepCompleted`.
    fn set_step_completed(&mut self) {
        self.meta.last_step = Some(self.current_step);
        self.meta.completed_steps.push(self.current_step);
    }

    /// `runCurrentStep(stepName)` (module docs).
    pub fn run_current_step(&mut self, step: WorkflowStep) -> StepDecision {
        if !self.completed_steps_before_restart.is_empty()
            && self.completed_steps_before_restart.contains(&self.current_step)
            && self.settings.restart_from_latest_step
            && !self.is_restarted
        {
            // runAfterCurrentStep
            let ret = if Some(self.current_step) == self.restart_after_step {
                self.is_restarted = true;
                StepDecision::RunAfterLoading
            } else {
                StepDecision::Skip
            };
            self.set_step_completed();
            self.current_step = step;
            return ret;
        }
        // stopAfterCurrentStep
        self.set_step_completed();
        if self.settings.stop_after == Some(self.current_step) {
            return StepDecision::Stop;
        }
        self.set_step_completed();
        self.current_step = step;
        StepDecision::Run
    }

    /// `workflowCompleted`: the workflow entries are removed.
    pub fn workflow_completed(&mut self) {
        self.meta = MeshMetaData::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WorkflowStep::*;

    const TET_STEPS: [WorkflowStep; 8] =
        [TemplateGeneration, SurfaceTopology, SurfaceProjection, PatchAssignment, EdgeExtraction, BoundaryLayerGeneration, MeshOptimisation, BoundaryLayerRefinement];

    /// V&V (gh:#1272), analytic against `workflowControls.C`: the decisions
    /// for `tetMeshGenerator`'s step order with no settings, with
    /// `stopAfter`, and on a restart from the stopped meta data.
    #[test]
    fn workflow_controls_follow_upstream() {
        assert_eq!(WorkflowStep::from_name("edgeExtraction"), Some(EdgeExtraction));
        assert_eq!(WorkflowStep::from_name("nope"), None);
        assert_eq!(MeshOptimisation.value(), 32);
        // no settings: every step runs
        let mut w = WorkflowControls::new(WorkflowSettings::default(), MeshMetaData::default());
        for s in TET_STEPS {
            assert_eq!(w.run_current_step(s), StepDecision::Run);
        }
        // each closed step is recorded twice (upstream quirk)
        assert_eq!(&w.meta.completed_steps[..4], &[Start, Start, TemplateGeneration, TemplateGeneration]);
        w.workflow_completed();
        assert_eq!(w.meta, MeshMetaData::default());
        // stop after surfaceProjection: decided when patchAssignment is asked
        let settings = WorkflowSettings { stop_after: Some(SurfaceProjection), restart_from_latest_step: false };
        let mut w = WorkflowControls::new(settings, MeshMetaData::default());
        let d: Vec<_> = TET_STEPS[..4].iter().map(|&s| w.run_current_step(s)).collect();
        assert_eq!(d, vec![StepDecision::Run, StepDecision::Run, StepDecision::Run, StepDecision::Stop]);
        assert_eq!(w.meta.last_step, Some(SurfaceProjection));
        // restart: the steps up to surfaceProjection are skipped, the mesh
        // saved after it is loaded before patchAssignment
        let settings = WorkflowSettings { stop_after: None, restart_from_latest_step: true };
        let mut r = WorkflowControls::new(settings, w.meta.clone());
        let d: Vec<_> = TET_STEPS.iter().map(|&s| r.run_current_step(s)).collect();
        assert_eq!(
            d,
            vec![
                StepDecision::Skip,
                StepDecision::Skip,
                StepDecision::Skip,
                StepDecision::RunAfterLoading,
                StepDecision::Run,
                StepDecision::Run,
                StepDecision::Run,
                StepDecision::Run
            ]
        );
        // stopAfter the last step never triggers
        let settings = WorkflowSettings { stop_after: Some(BoundaryLayerRefinement), restart_from_latest_step: false };
        let mut w = WorkflowControls::new(settings, MeshMetaData::default());
        assert!(TET_STEPS.iter().all(|&s| w.run_current_step(s) == StepDecision::Run));
    }
}
