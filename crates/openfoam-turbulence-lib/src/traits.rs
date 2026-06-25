use openfoam_basic_lib::prelude::{FvVectorMatrix, VolScalarField, VolVectorField};

/// Common interface for all RAS and LES turbulence models.
///
/// Mirrors `Foam::compressible::turbulenceModel` and its incompressible
/// counterpart. Use static dispatch (generics) — not `dyn TurbulenceModel` —
/// to match C++ template zero-overhead composition.
pub trait TurbulenceModel {
    /// Assemble the turbulent deviatoric stress divergence term for the
    /// momentum equation:  ∇·(−2 μ_eff · dev(symm(∇U))).
    ///
    /// Returns an `FvVectorMatrix` whose coefficients are added to the
    /// momentum predictor before solving.
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;

    /// Recompute turbulence transport fields (k, ε/ω, ν_t/μ_t) by solving
    /// the turbulence transport equations for one time step.
    ///
    /// Called once per time step **after** the momentum and pressure correctors.
    fn correct(&mut self);

    /// Turbulent kinematic viscosity field ν_t (incompressible) or μ_t/ρ
    /// (compressible).  Length == `mesh.n_cells`.
    fn nu_t(&self) -> &VolScalarField;

    /// Effective thermal diffusivity field: α_eff = α + α_t.
    ///
    /// `alpha` is the molecular thermal diffusivity (= κ / Cp) passed in from
    /// the thermophysical model.
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField;

    /// Effective dynamic viscosity field: μ_eff = μ + μ_t.
    ///
    /// `mu` is the molecular dynamic viscosity field from the thermophysical model.
    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField;
}
