// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under AGPL-3.0. See the crate root or
// <https://www.gnu.org/licenses/> for the full license text.

//! # The thermo-mechanics region with a selectable structural discretisation (gh:#1782)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Maintainer request 2026-11-26: structural mechanics "should have a parallel FEM version selectable by enum".
//! [`StructuralDiscretisation`] is that enum, [`StructuralDiscretisation::FiniteVolume`] (GeN-Foam's own, the
//! bit-identical [`LegacyFvmThermoMechanics`]) the DEFAULT. [`ThermoMechanicsRegion`] wraps the chosen solver
//! behind the interface the coupling reads: [`set_coupling_fields`](ThermoMechanicsRegion::set_coupling_fields),
//! [`correct_physics`](ThermoMechanicsRegion::correct_physics), and the feedback outputs
//! ([`mesh_displacement`](ThermoMechanicsRegion::mesh_displacement),
//! [`fuel_displacement`](ThermoMechanicsRegion::fuel_displacement),
//! [`control_rod_displacement`](ThermoMechanicsRegion::control_rod_displacement)).
//!
//! ## What the finite-element branch does, in `correctPhysics()` order
//!
//! 1. the `TStruct` equation, by the finite-volume solver's own code
//!    ([`LegacyFvmThermoMechanics::correct_physics_temperature_stage`]): the finite-element variant takes the same
//!    `TStruct`, so the temperature, and every coupling field, is shared;
//! 2. the displacement, by [`FemThermoElastic`] at that `TStruct` (replacing the displacement corrector loop);
//! 3. the expansion feedback (`fuelDisp`, `CRDisp`, `meshDisp = (disp - (disp & n) n) + (fuelDisp + CRDisp) n`),
//!    by the finite-volume solver's own code with the finite-element displacement as `disp`
//!    ([`LegacyFvmThermoMechanics::correct_physics_expansion_stage`]): identical, not re-implemented.
//!
//! `mechanicsSolver off` zeroes everything, as upstream does, in both branches.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::vec::Vec;

use outram_foam_basic_lib::primitives::Vector3;

use super::fem::{FemPatch, FemSettings, FemSolveLog, FemThermoElastic, FemTopology};
use super::legacy_fvm::{DispPatch, LegacyFvmThermoMechanics, PhysicsLog, SolveRecord};
use crate::error::AppBuilderError;

/// The discretisation of the structural (displacement) equation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructuralDiscretisation {
    /// GeN-Foam's `legacyThermoMechanics`: segregated finite volumes, compact normal stress. The default.
    #[default]
    FiniteVolume,
    /// Small-strain finite elements on the same mesh ([`FemThermoElastic`]).
    FiniteElement,
}

/// A finite-element region: the finite-volume solver supplies the temperature and the expansion feedback, the
/// finite-element solver the displacement.
#[derive(Debug, Clone)]
pub struct FemRegion {
    fvm: LegacyFvmThermoMechanics,
    fem: FemThermoElastic,
    solve_displacement: bool,
    last: Option<FemSolveLog>,
}

/// The thermo-mechanics region of the selected discretisation.
#[derive(Debug, Clone)]
pub enum ThermoMechanicsRegion {
    /// Finite volume (default).
    FiniteVolume(LegacyFvmThermoMechanics),
    /// Finite element.
    FiniteElement(FemRegion),
}

/// GeN-Foam's `fixedValue` / `zeroGradient` patches of `disp` as finite-element patches: `fixedValue` stays,
/// `zeroGradient` becomes the natural, traction-free condition (a model difference, `fem` module documentation).
///
/// # Errors
/// [`AppBuilderError::UnsupportedScheme`] for a patch type with no finite-element counterpart yet
/// (`GFTractionDisplacement`, `gapContact`): named, not substituted.
pub fn fem_patches_from_disp_patches(patches: &[DispPatch]) -> Result<Vec<FemPatch>, AppBuilderError> {
    patches
        .iter()
        .map(|p| match p {
            DispPatch::FixedValue(v) => Ok(FemPatch::FixedValue(*v)),
            DispPatch::ZeroGradient => Ok(FemPatch::TractionFree),
            #[allow(unreachable_patterns)]
            _ => Err(AppBuilderError::UnsupportedScheme {
                family: "boundaryField disp",
                scheme: format!("{p:?}"),
                reason: "no finite-element counterpart ported yet: GFTractionDisplacement is ported for the finite-volume solver only (gh:#1235); a finite-element traction patch is filed",
            }),
        })
        .collect()
}

impl ThermoMechanicsRegion {
    /// A finite-volume region (the default discretisation).
    pub fn finite_volume(fvm: LegacyFvmThermoMechanics) -> Self {
        Self::FiniteVolume(fvm)
    }

    /// A finite-element region: `fvm` (built from the same case) supplies `TStruct` and the expansion feedback,
    /// `fem` the displacement.
    pub fn finite_element(fvm: LegacyFvmThermoMechanics, fem: FemThermoElastic, solve_displacement: bool) -> Self {
        Self::FiniteElement(FemRegion { fvm, fem, solve_displacement, last: None })
    }

    /// Build the region of `kind` from a read case (`std`: the case reader is).
    ///
    /// # Errors
    /// Any refusal of the finite-volume or finite-element constructors.
    #[cfg(feature = "std")]
    pub fn from_case(
        kind: StructuralDiscretisation,
        case: &super::legacy_fvm::case::LegacyFvmCase,
        topology: &FemTopology,
        fem_settings: FemSettings,
    ) -> Result<Self, AppBuilderError> {
        let fvm = case.build()?;
        match kind {
            StructuralDiscretisation::FiniteVolume => Ok(Self::FiniteVolume(fvm)),
            StructuralDiscretisation::FiniteElement => {
                let mut s = fem_settings;
                s.plane_stress = case.settings.plane_stress;
                let fem = FemThermoElastic::new(
                    case.mesh.clone(),
                    topology,
                    s,
                    &case.zones,
                    &case.cards,
                    fem_patches_from_disp_patches(&case.disp_patches)?,
                )?;
                Ok(Self::finite_element(fvm, fem, case.settings.solve_displacement))
            }
        }
    }

    /// Which discretisation this region runs.
    pub fn discretisation(&self) -> StructuralDiscretisation {
        match self {
            Self::FiniteVolume(_) => StructuralDiscretisation::FiniteVolume,
            Self::FiniteElement(_) => StructuralDiscretisation::FiniteElement,
        }
    }

    fn fvm(&self) -> &LegacyFvmThermoMechanics {
        match self {
            Self::FiniteVolume(f) => f,
            Self::FiniteElement(r) => &r.fvm,
        }
    }

    /// `TStructFromTH`, `TFuel`, `powerDensityNeutronics` per cell (see
    /// [`LegacyFvmThermoMechanics::set_coupling_fields`]).
    pub fn set_coupling_fields(&mut self, t_struct_from_th: &[f64], t_fuel: &[f64], power_density: &[f64]) {
        match self {
            Self::FiniteVolume(f) => f.set_coupling_fields(t_struct_from_th, t_fuel, power_density),
            Self::FiniteElement(r) => r.fvm.set_coupling_fields(t_struct_from_th, t_fuel, power_density),
        }
    }

    /// The step length of the `TStruct` time derivative.
    pub fn set_delta_t(&mut self, delta_t: f64) {
        match self {
            Self::FiniteVolume(f) => f.set_delta_t(delta_t),
            Self::FiniteElement(r) => r.fvm.set_delta_t(delta_t),
        }
    }

    /// One `correctPhysics()`. The finite-element branch records its solve as the entry `dispFEM` of the log.
    ///
    /// # Errors
    /// The finite-element solve's failure (the finite-volume branch cannot fail).
    pub fn correct_physics(&mut self) -> Result<PhysicsLog, AppBuilderError> {
        match self {
            Self::FiniteVolume(f) => Ok(f.correct_physics()),
            Self::FiniteElement(r) => {
                if !r.solve_displacement {
                    return Ok(r.fvm.correct_physics());
                }
                let mut log = r.fvm.correct_physics_temperature_stage();
                let log_fem = r.fem.solve(r.fvm.t_struct())?;
                log.solves.push(SolveRecord {
                    field: "dispFEM",
                    initial_residual: 1.0,
                    final_residual: log_fem.final_residual,
                    n_iterations: log_fem.n_iterations,
                });
                log.max_sigma_eq = Some(log_fem.max_sigma_eq);
                log.n_correctors_run = 1;
                r.fvm.correct_physics_expansion_stage(r.fem.displacement(), &mut log);
                r.last = Some(log_fem);
                Ok(log)
            }
        }
    }

    /// The last finite-element solve (`None` for finite volume or before the first step).
    pub fn fem_log(&self) -> Option<&FemSolveLog> {
        match self {
            Self::FiniteVolume(_) => None,
            Self::FiniteElement(r) => r.last.as_ref(),
        }
    }

    /// The finite-element solver, for the finite-element branch.
    pub fn fem(&self) -> Option<&FemThermoElastic> {
        match self {
            Self::FiniteVolume(_) => None,
            Self::FiniteElement(r) => Some(&r.fem),
        }
    }

    /// `TStruct` per cell (K).
    pub fn t_struct(&self) -> &[f64] {
        self.fvm().t_struct()
    }

    /// Displacement per cell (m): the finite-volume cell value, or the finite-element cell average.
    pub fn displacement(&self) -> &[Vector3] {
        match self {
            Self::FiniteVolume(f) => f.displacement(),
            Self::FiniteElement(r) => f_disp(r),
        }
    }

    /// `fuelDisp` per cell (m).
    pub fn fuel_displacement(&self) -> &[f64] {
        self.fvm().fuel_displacement()
    }

    /// `CRDisp` per cell (m).
    pub fn control_rod_displacement(&self) -> &[f64] {
        self.fvm().control_rod_displacement()
    }

    /// `meshDisp` per cell (m).
    pub fn mesh_displacement(&self) -> &[Vector3] {
        self.fvm().mesh_displacement()
    }

    /// `residual_` of the last step (the temperature equation's for the finite-element branch).
    pub fn residual(&self) -> f64 {
        self.fvm().residual()
    }
}

/// The displacement of a finite-element region: what the expansion stage was given, which the inner solver holds.
fn f_disp(r: &FemRegion) -> &[Vector3] {
    // After `correct_physics` the inner finite-volume object holds exactly the finite-element displacement
    // (copied in by the expansion stage); before it, zeros.
    r.fvm.displacement()
}
