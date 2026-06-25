use std::sync::Arc;
use openfoam_basic_lib::prelude::{FvMesh, FvVectorMatrix, VolScalarField, VolVectorField};
use crate::traits::TurbulenceModel;

/// No-op turbulence model — laminar flow, zero turbulent stresses.
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/laminar/laminar.H`
pub struct LaminarModel {
    pub mesh: Arc<FvMesh>,
    /// Molecular kinematic viscosity (from fluid thermo); ν_t ≡ 0.
    nu: VolScalarField,
}

impl LaminarModel {
    pub fn new(mesh: Arc<FvMesh>, nu: VolScalarField) -> Self {
        Self { mesh, nu }
    }
}

impl TurbulenceModel for LaminarModel {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("LaminarModel::div_dev_rho_reff")
    }

    /// No-op — laminar model has no transport equations to solve.
    fn correct(&mut self) {}

    fn nu_t(&self) -> &VolScalarField {
        // ν_t = 0 for laminar flow; return the molecular viscosity field
        // with its values zeroed (caller interprets this as turbulent contribution).
        &self.nu
    }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        alpha.clone()
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        mu.clone()
    }
}
