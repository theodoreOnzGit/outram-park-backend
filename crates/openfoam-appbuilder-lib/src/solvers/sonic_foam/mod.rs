use std::sync::Arc;
use openfoam_basic_lib::prelude::{FvMesh, VolScalarField, VolVectorField};
use crate::error::AppBuilderError;
use crate::io::control_dict::ControlDict;
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Transonic / supersonic compressible solver — sonicFoam.
/// Uses a density-based formulation; suitable for Mach > 1 flows.
///
/// C++ solver: `applications/solvers/compressible/sonicFoam/`
///
/// Compared to rhoPimpleFoam, sonicFoam:
///   - uses a pressure-velocity coupling based on the sonic equation
///   - is better conditioned at high Mach numbers
///   - does not support MRF or porous zones in the base solver
pub struct SonicFoam {
    pub mesh:    Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    pub u:   VolVectorField,
    pub p:   VolScalarField,
    pub rho: VolScalarField,
    pub e:   VolScalarField,
}

impl SonicFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u   = VolVectorField::zero("U",   mesh.clone());
        let p   = VolScalarField::zeros("p",   mesh.clone());
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let e   = VolScalarField::zeros("e",   mesh.clone());
        Self { mesh, control, schemes, solution, u, p, rho, e }
    }

    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        todo!("SonicFoam::step — sonic pressure-velocity coupling")
    }

    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        todo!("SonicFoam::run")
    }
}
