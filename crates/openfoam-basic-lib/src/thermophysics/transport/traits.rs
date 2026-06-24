use crate::thermophysics::imports::*;
use crate::thermophysics::thermo::ThermoModel;

/// Per-species transport model — dynamic viscosity and thermal conductivity.
///
/// Mirrors the `transport` layer in
/// `src/thermophysicalModels/specie/transport/`.
///
/// `alpha_h` (thermal diffusivity = κ/Cp, units kg/(m·s) = same as DynamicViscosity)
/// has a default implementation via `kappa / cp`.
pub trait TransportModel: ThermoModel {
    /// Dynamic viscosity μ  [Pa·s = kg/(m·s)].
    fn mu(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;

    /// Thermal conductivity κ  [W/(m·K)].
    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity;

    /// Thermal diffusivity αh = κ / Cp  [kg/(m·s)]  (same dimension as DynamicViscosity).
    fn alpha_h(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        // κ / Cp:  [W/(m·K)] / [J/(kg·K)] = kg/(m·s)
        self.kappa(p, t) / self.cp(p, t)
    }
}
