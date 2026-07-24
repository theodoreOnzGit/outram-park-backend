// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

pub use crate::error::AppBuilderError;

// I/O
pub use crate::io::control_dict::{
    ControlDict, StartControl, StopControl, WriteControl, WriteFormat,
};
pub use crate::io::fv_schemes::{
    DdtScheme, DivScheme, FvSchemes, GradScheme, LaplacianScheme, SnGradScheme,
};
pub use crate::io::fv_solution::{FvSolution, LinearSolverConfig, LinearSolverType, PimpleControl};
pub use crate::io::output::write_scalar_field;
pub use crate::io::poly_mesh::read_poly_mesh;

// GeN-Foam neutronics — point kinetics (0-D)
pub use crate::genfoam::neutronics::point_kinetics::{
    DecayConstant, PointKineticsError, PointKineticsParameters, PointKineticsState,
    PromptGenerationTime, Reactivity,
};
// GeN-Foam neutronics — model dispatch, state, and cross-section data
pub use crate::genfoam::neutronics::xs::CrossSectionData;
pub use crate::genfoam::neutronics::{DiffusionNeutronics, NeutronicsModel, NeutronicsState};

// GeN-Foam thermo-mechanics — displacement/thermal-stress solver
pub use crate::genfoam::thermo_mechanics::{LegacyThermoMechanics, MechanicsMeshSolver};

// GeN-Foam multi-region coupling
pub use crate::genfoam::multi_region::{MeshHandler, MultiPhysicsSolver};

// GeN-Foam thermal-hydraulics — fluid phase and solid-structure state
pub use crate::genfoam::thermal_hydraulics::phase::{Fluid, StateOfMatter};
pub use crate::genfoam::thermal_hydraulics::structure::{HeatExchanger, PowerModel};

// Solvers
pub use crate::solvers::hrm_foam::{HrmFoam, HrmModelConfig};
pub use crate::solvers::pimple_foam::{PimpleFoam, PressureSolver};
pub use crate::solvers::reacting_two_phase_euler_foam::{
    build_default as build_reacting_two_phase_euler, InterfacialHeatTransfer,
    InterfacialMassTransfer, PhaseSelector, PhaseThermo, ReactingTwoPhaseEulerFoam, ReactionSource,
};
pub use crate::solvers::rho_central_foam::RhoCentralFoam;
pub use crate::solvers::rho_pimple_foam::RhoPimpleFoam;
pub use crate::solvers::sonic_foam::SonicFoam;
