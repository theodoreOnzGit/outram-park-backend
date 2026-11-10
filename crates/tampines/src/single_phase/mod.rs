//! Lumped single-phase liquid thermal-hydraulics.
//!
//! Re-exports [`tuas_boussinesq_solver`]'s finite-volume lumped-parameter
//! single-phase pipe/component model under a TAMPINES-local name. TUAS's
//! backend fluid list ([`LiquidMaterial`]) currently covers molten-salt and
//! thermal-oil loops (TherminolVP1, DowthermA, HITEC, YD325, FLiBe, ...) and,
//! since 2026-11-10, **liquid sodium** ([`liquid_sodium`], 400-1000 K,
//! CoolProp's `LiqNa` fit; gh:#1320) -- it does **not** yet back water, air,
//! or helium. For those fluids, use
//! [`crate::compressible`] (CoolProp-backed) instead until TUAS grows
//! matching liquid coverage.
//!
//! This is a thin wiring module: it does not add behaviour beyond naming.
//! The full TAMPINES fluid-array interface (unifying this with
//! [`crate::compressible`] and the steam-tables backend behind one enum) is
//! tracked separately -- see the workspace's beads issue tracker, epic
//! `op-dt3`.

/// A single-phase liquid loop segment (1D lumped finite-volume pipe/component
/// with lateral thermal coupling).
///
/// Alias for [`tuas_boussinesq_solver`]'s `FluidArray` -- see that type's own
/// documentation for the underlying physics and constructors
/// (`new_cylinder`, `new_annular_cylinder`, `new_odd_shaped_pipe`,
/// `new_custom_component`).
pub use tuas_boussinesq_solver::array_fluid_collections::fluid_array_lateral_coupling::FluidArray as SinglePhaseFluidArray;

/// The liquid substances a [`SinglePhaseFluidArray`] can currently be backed
/// by. Re-exported from `tuas_boussinesq_solver` for convenience.
pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;

/// Liquid sodium as a [`LiquidMaterial`] (a TUAS `CustomLiquid`), 400-1000 K,
/// every property from CoolProp's `LiqNa` fit via `outram-park-fork-coolprop`.
/// Re-exported from `tuas_boussinesq_solver`; see that function for the
/// range and its V&V (gh:#1320, EBR-II groundwork #1312).
pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::liquid_sodium_coolprop::liquid_sodium;

/// Error type returned by [`SinglePhaseFluidArray`] methods. Re-exported from
/// `tuas_boussinesq_solver` for convenience.
pub use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;

#[cfg(test)]
mod tests {
    use super::*;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::density::try_get_rho;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::Material;
    use uom::si::f64::{Pressure, ThermodynamicTemperature};
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::atmosphere;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Sodium is selectable from TAMPINES: the re-exported constructor
    /// gives CoolProp `LiqNa`'s density at 700 K (846.678594 kg/m³, the cubic
    /// fit evaluated in Python, gh:#1320) and errors above 1000 K.
    #[test]
    fn liquid_sodium_is_selectable_through_single_phase() {
        let sodium = Material::Liquid(liquid_sodium());
        let p = Pressure::new::<atmosphere>(1.0);
        let rho = try_get_rho(sodium, ThermodynamicTemperature::new::<kelvin>(700.0), p)
            .unwrap()
            .get::<kilogram_per_cubic_meter>();
        assert!((rho / 846.678_594 - 1.0).abs() < 1e-8, "rho = {rho}");
        assert!(try_get_rho(sodium, ThermodynamicTemperature::new::<kelvin>(1100.0), p).is_err());
    }
}
