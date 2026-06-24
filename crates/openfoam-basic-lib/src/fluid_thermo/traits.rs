use std::sync::Arc;
use crate::mesh::fv_mesh::FvMesh;
use crate::fields::vol_field::VolScalarField;

/// Field-level fluid thermodynamic model.
///
/// Mirrors the `Foam::fluidThermo` / `Foam::psiThermo` / `Foam::rhoThermo`
/// abstract interface from `src/thermophysicalModels/basic/`.
///
/// Owns the primary thermodynamic fields (`p`, `T`, `he`, `rho`, `psi`) and
/// provides `correct()` to recompute derived quantities after `he` or `p`
/// have been updated by the solver.
///
/// Computed transport fields (`mu`, `kappa`, `alpha_h`) are returned by value
/// rather than stored, to keep the struct lean and avoid stale-field bugs.
pub trait FluidThermo {
    fn mesh(&self) -> &Arc<FvMesh>;

    /// Pressure field [Pa].
    fn p(&self) -> &VolScalarField;
    fn p_mut(&mut self) -> &mut VolScalarField;

    /// Temperature field [K].
    fn t(&self) -> &VolScalarField;

    /// Density field [kg/m³].
    fn rho(&self) -> &VolScalarField;

    /// Energy field — sensible enthalpy `hs` [J/kg] by default.
    fn he(&self) -> &VolScalarField;
    fn he_mut(&mut self) -> &mut VolScalarField;

    /// Compressibility field ψ = ∂ρ/∂p|_T [s²/m²].
    fn psi(&self) -> &VolScalarField;

    /// Dynamic viscosity field μ [Pa·s] — computed on demand.
    fn mu(&self) -> VolScalarField;

    /// Thermal conductivity field κ [W/(m·K)] — computed on demand.
    fn kappa(&self) -> VolScalarField;

    /// Thermal diffusivity αh = κ/Cp [kg/(m·s)] — computed on demand.
    fn alpha_h(&self) -> VolScalarField;

    /// Recompute `T`, `ρ`, and `ψ` from `he` + `p`.
    ///
    /// Call this after the energy equation has updated `he` and after `p` has
    /// been corrected.  The Newton iteration `t_from_hs` is applied cell-by-cell.
    fn correct(&mut self);

    /// Clamp density after the pressure equation:
    /// `ρ ← clamp(ρ + δρ, ρ_min, ρ_max)`.
    ///
    /// Corresponds to `thermo.correctRho(psi*p − ψ₀·p₀, rhoMin, rhoMax)` in
    /// OpenFOAM's rhoPimpleFoam.
    fn correct_rho(&mut self, delta_rho: &VolScalarField, rho_min: f64, rho_max: f64);
}
