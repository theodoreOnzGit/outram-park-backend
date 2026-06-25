use std::sync::Arc;
use openfoam_basic_lib::prelude::{FvMesh, VolScalarField, VolVectorField};
use crate::error::AppBuilderError;
use crate::io::control_dict::ControlDict;
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Density-based central-upwind compressible solver — rhoCentralFoam.
/// Implements the Kurganov-Noelle-Petrova (KNP) scheme.
/// Well-suited for high-speed flows with shocks; explicit time-stepping only.
///
/// C++ solver: `applications/solvers/compressible/rhoCentralFoam/`
///
/// The KNP flux uses min/max eigenvalue wave-speed estimates to split the
/// numerical flux into left/right contributions, giving second-order accuracy
/// in smooth regions and TVD behaviour across discontinuities.
pub struct RhoCentralFoam {
    pub mesh:    Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    pub u:   VolVectorField,
    pub p:   VolScalarField,
    pub rho: VolScalarField,
    pub e:   VolScalarField,
    /// Co-volume limiter coefficient (between 0 and 1).
    pub psi_limit: f64,
}

impl RhoCentralFoam {
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
        Self { mesh, control, schemes, solution, u, p, rho, e, psi_limit: 1.0 }
    }

    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        todo!("RhoCentralFoam::step — KNP central-upwind flux, explicit RK time advance")
    }

    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        todo!("RhoCentralFoam::run")
    }
}
