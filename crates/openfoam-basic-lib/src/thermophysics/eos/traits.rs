use uom::si::f64::{MassDensity, MolarMass, Pressure, Ratio, AvailableEnergy, SpecificHeatCapacity, ThermodynamicTemperature};

use crate::thermophysics::quantities::Compressibility;

/// Per-species equation of state — mesh-independent kernel.
///
/// Maps to the OpenFOAM `equationOfState` template layer in
/// `src/thermophysicalModels/specie/equationOfState/`.
///
/// All methods take `(p, T)` and return the corresponding property.
/// Enthalpy/entropy departure methods return the EOS *contribution* only;
/// the full quantity is assembled in `ThermoModel`.
pub trait EquationOfState {
    fn mol_weight(&self) -> MolarMass;

    /// Specific gas constant R = R_universal / W  [J/(kg·K)].
    fn r(&self) -> SpecificHeatCapacity;

    /// Density ρ = ρ(p, T)  [kg/m³].
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity;

    /// Isentropic compressibility ψ = ∂ρ/∂p|_T  [s²/m²].
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility;

    /// Compressibility factor Z = p·v / (R·T)  [-].
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio;

    /// Cp − Cv = R for ideal gas; Maxwell relation correction for real gas.
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// EOS contribution to Cp  [J/(kg·K)].  Zero for perfect/incompressible gas.
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Enthalpy departure from ideal gas  [J/kg].
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Internal-energy departure from ideal gas  [J/kg].
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// EOS contribution to specific entropy  [J/(kg·K)].
    /// For perfect gas: `−R·ln(p/p_ref)`.
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
}
