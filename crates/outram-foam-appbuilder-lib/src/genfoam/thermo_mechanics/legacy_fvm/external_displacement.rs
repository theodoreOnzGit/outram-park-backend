// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (commit 652b3da), see the parent module's header.

//! Two additive entry points that split [`LegacyFvmThermoMechanics::correct_physics`] at the displacement solve
//! (gh:#1782), so another discretisation's displacement can be fed to the SAME expansion-feedback code
//! (`fuelDisp`, `CRDisp`, `meshDisp`). A child module of `legacy_fvm`: it reads the private fields and calls the
//! private `solve_temperature` / `solve_expansion` without changing them. Nothing in `correct_physics` is edited.

use outram_foam_basic_lib::primitives::Vector3;

use super::{LegacyFvmThermoMechanics, PhysicsLog};

impl LegacyFvmThermoMechanics {
    /// The first stage of `correctPhysics()`: the `TStruct` equation (if `thermalSolver`), exactly as
    /// [`Self::correct_physics`] runs it. `log.residual` carries its initial residual (0 when thermal is off).
    pub fn correct_physics_temperature_stage(&mut self) -> PhysicsLog {
        let mut log = PhysicsLog::default();
        if self.settings.solve_thermo {
            log.residual = self.solve_temperature(&mut log);
        }
        self.residual = log.residual;
        log
    }

    /// The last stage of `correctPhysics()` with the displacement supplied from outside instead of solved here:
    /// sets `disp`, then runs the axial `fuelDisp` / `CRDisp` expansions and `meshDisp`, which are the code
    /// [`Self::correct_physics`] itself runs after its displacement loop.
    ///
    /// # Panics
    /// If `disp` does not have one vector per cell.
    pub fn correct_physics_expansion_stage(&mut self, disp: &[Vector3], log: &mut PhysicsLog) {
        assert_eq!(disp.len(), self.disp.len(), "one displacement per cell");
        self.disp.copy_from_slice(disp);
        self.correct_disp_boundary();
        self.solve_expansion(log);
    }
}
