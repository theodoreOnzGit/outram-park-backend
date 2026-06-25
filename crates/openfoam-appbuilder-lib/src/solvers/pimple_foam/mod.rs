use std::sync::Arc;
use openfoam_basic_lib::prelude::{FvMesh, VolScalarField, VolVectorField};
use crate::error::AppBuilderError;
use crate::io::control_dict::ControlDict;
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Incompressible, transient solver using the PIMPLE algorithm
/// (merged PISO + SIMPLE outer correctors).
///
/// Supports turbulence via the `openfoam-turbulence-lib` trait object.
///
/// C++ solver: `applications/solvers/incompressible/pimpleFoam/`
///
/// Outer PIMPLE loop:
///   1. Momentum predictor  — solve U with explicit pressure gradient
///   2. Inner PISO correctors — solve p, correct U
///   3. Turbulence correct
pub struct PimpleFoam {
    pub mesh:    Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
}

impl PimpleFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::zeros("p", mesh.clone());
        Self { mesh, control, schemes, solution, u, p }
    }

    /// Advance the solution by one time step.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        todo!("PimpleFoam::step — PIMPLE outer loop + PISO pressure correction")
    }

    /// Run the full time loop until `endTime`.
    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        todo!("PimpleFoam::run — time loop calling step() until end condition")
    }
}
