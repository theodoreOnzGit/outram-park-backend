// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/
//                    (thermoMechanics.{C,H}, legacyThermoMechanics/*)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `genfoam::thermo_mechanics` — thermal-expansion / thermo-mechanics feedback
//!
//! Rust port of `GeN-Foam/src/classes/thermoMechanics`: the linear-elastic,
//! small-strain thermal-expansion model that feeds geometry/density changes back
//! into the neutronics and thermal-hydraulics regions. Upstream it is a
//! transient segregated finite-volume solver derived from OpenFOAM's
//! `solidDisplacementFoam`, generalised to multiple material zones
//! (`legacyThermoMechanics`).
//!
//! ## What this module contains
//!
//! The self-contained, analytically-verifiable physics core of the model:
//!
//! - [`material`] — the per-zone isotropic linear-elastic material card
//!   ([`ElasticMaterial`]): `E`, `ν`, `α`, `ρ`, `c_p`, `k`, and the derived Lamé
//!   parameters `μ`, `λ`, bulk term `3K`, and thermal diffusivity `DT`.
//! - [`stress`] — the Hooke's-law thermal-stress constitutive relation:
//!   `σ = 2μ ε + λ tr(ε) I − 3K α ΔT I`, plus the von Mises equivalent stress.
//! - [`feedback`] — the geometry feedback the model *produces*: the axial
//!   fuel/control-rod thermal expansion (`u = ∫ α ΔT dz`) and the assembly of
//!   the coupled mesh-displacement vector `meshDisp` mapped onto the other
//!   meshes.
//!
//! These are wired together by [`LegacyThermoMechanics`] and dispatched through
//! the [`ThermoMechanicsModel`] enum (no `dyn`, per workspace rules).
//!
//! - [`mesh_solve`] — the full finite-volume field solve on the mechanics
//!   `FvMesh`: the segregated displacement-momentum equation (`DEqn`) and the
//!   structure heat-diffusion equation (`TEqn`) from
//!   `include/solveThermalMechanics.H`, plus the coupled `meshDisp` assembly.
//!   The entry point is [`MechanicsMeshSolver`]. It consumes the constitutive
//!   kernel above cell-by-cell.
//!
//! ## Field solve — now implemented on the mechanics mesh
//!
//! The full displacement *field* solve on an `fvMesh`
//! (`include/solveThermalMechanics.H`) and the structure heat-diffusion solve
//! live in [`mesh_solve`], built on the finite-volume operators
//! [`outram_foam_basic_lib`] now exposes:
//!
//! - vector-field gradient `∇u → VolTensorField` (`fvc::grad_vec`) and
//!   tensor-field divergence `∇·σ → VolVectorField` (`fvc::div_tensor`),
//! - `fvm::d2dt2` (implicit second time derivative) for the inertial term,
//! - the scalar `fvm::laplacian` / vector `fvm::laplacian_vec` diffusion
//!   operators and `fvc::grad` for the explicit thermal load.
//!
//! The constitutive law and feedback relations ported in [`stress`]/[`feedback`]
//! are exactly the physics the FV assembly evaluates pointwise, and also stand
//! alone against analytical benchmarks (see the `#[test]` V&V cases in each
//! submodule, and the coupled field-solve V&V in [`mesh_solve`]).
//!
//! ## Still deferred
//!
//! An anisotropic stiffness (needs a tensor-coefficient Laplacian), a traction
//! (`σ·n = 0`) boundary condition on `D`, non-orthogonal gradient correction,
//! and the multi-region mapping of the 1-D axial fuel/CR expansion onto a shared
//! mesh — see the limitations note in [`mesh_solve`].
//!
//! ## Interface expected from `genfoam::common` (ported in parallel)
//!
//! This module currently needs nothing from `genfoam::common` at compile time.
//! The mesh solve ([`mesh_solve`]) and the multi-region coupling will need,
//! from `common` / the wider port:
//!
//! - a material-zone map (cell → [`ElasticMaterial`]) built from the
//!   `thermoMechanicalProperties`/`materials` dictionary (upstream reads it via
//!   `cellZones`),
//! - the `meshToMesh` field-mapping used by `interpolateCouplingFields`
//!   (`multiRegion`) to pull `TFuel`, `TStruct`, and `powerDensityNeutronics`
//!   onto the mechanics mesh and push `meshDisp` back, and
//! - `movePoints`-style mesh deformation on `FvMesh` (upstream `deformMesh`).
//!
//! See `docs/genfoam-port-plan.md` for the module map and translation order.

pub mod feedback;
pub mod material;
pub mod mesh_solve;
pub mod stress;

// Curated re-exports (human interface layer): the types a user needs to build
// and query a thermo-mechanics model without hunting through submodules.
pub use feedback::{assemble_mesh_displacement, axial_expansion_profile, free_axial_expansion};
pub use material::{
    BulkModulusTerm, Displacement, ElasticMaterial, LameLambda, MaterialError, RheologyOption,
    ShearModulus, Stress, ThermalDiffusivity, ThermalExpansionCoeff, YoungsModulus,
};
pub use mesh_solve::{DisplacementReport, MechanicsMeshSolver};
pub use stress::{
    deviatoric_plus_volumetric_stress, hydrostatic_stress, thermal_stress, von_mises_stress,
};

use outram_foam_basic_lib::primitives::{SymmTensor, Vector3};
use uom::si::f64::{Length, TemperatureInterval};

/// The linear-elastic thermo-mechanics model (`legacyThermoMechanics`) for a
/// single material zone: an [`ElasticMaterial`] plus its rheology idealisation.
///
/// This bundles the constitutive law and feedback relations of [`stress`] and
/// [`feedback`] against one material, so a caller supplies only kinematic /
/// thermal state (strain, temperature rise, geometry) at each query. It is the
/// per-zone constitutive kernel that [`mesh_solve`] evaluates cell-by-cell.
///
/// # Example
///
/// ```
/// use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
///     ElasticMaterial, LegacyThermoMechanics, RheologyOption, YoungsModulus, ThermalExpansionCoeff,
/// };
/// use uom::si::f64::{
///     Length, MassDensity, Ratio, SpecificHeatCapacity, TemperatureInterval, ThermalConductivity,
/// };
/// use uom::si::{
///     length::meter, mass_density::kilogram_per_cubic_meter, pressure::gigapascal, ratio::ratio,
///     specific_heat_capacity::joule_per_kilogram_kelvin, temperature_coefficient::per_kelvin,
///     temperature_interval::kelvin, thermal_conductivity::watt_per_meter_kelvin,
/// };
///
/// let steel = ElasticMaterial::new(
///     YoungsModulus::new::<gigapascal>(200.0),
///     Ratio::new::<ratio>(0.3),
///     ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
///     MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
///     SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
///     ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
/// )
/// .unwrap();
/// let model = LegacyThermoMechanics::new(steel, RheologyOption::PlaneStrain);
///
/// // A 2 m fuel rod heated 400 K above reference grows axially:
/// let dz = model.free_axial_expansion(
///     TemperatureInterval::new::<kelvin>(400.0),
///     Length::new::<meter>(2.0),
/// );
/// use uom::si::length::millimeter;
/// assert!((dz.get::<millimeter>() - 9.6).abs() < 1e-9); // α ΔT L = 1.2e-5·400·2 = 9.6 mm
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyThermoMechanics {
    material: ElasticMaterial,
    rheology: RheologyOption,
}

impl LegacyThermoMechanics {
    /// Build the model from a material and a rheology idealisation.
    pub fn new(material: ElasticMaterial, rheology: RheologyOption) -> Self {
        Self { material, rheology }
    }

    /// The underlying material card.
    pub fn material(&self) -> ElasticMaterial {
        self.material
    }

    /// The rheology idealisation (plane strain / plane stress).
    pub fn rheology(&self) -> RheologyOption {
        self.rheology
    }

    /// Full Cauchy stress tensor `σ` (pascals) for a small-strain state `ε` at a
    /// temperature rise `ΔT` above the stress-free reference. See
    /// [`stress::thermal_stress`].
    pub fn stress(&self, strain: SymmTensor, delta_t: TemperatureInterval) -> SymmTensor {
        thermal_stress(
            strain,
            self.material.shear_modulus(),
            self.material.lame_lambda(self.rheology),
            self.material.bulk_modulus_three_k(self.rheology),
            self.material.thermal_expansion(),
            delta_t,
        )
    }

    /// von Mises equivalent stress `σ_eq` (Pa) for a small-strain state `ε` at a
    /// temperature rise `ΔT`.
    pub fn von_mises(&self, strain: SymmTensor, delta_t: TemperatureInterval) -> Stress {
        von_mises_stress(self.stress(strain, delta_t))
    }

    /// Free axial thermal expansion `α ΔT L` of a uniformly-heated bar of length
    /// `L`. See [`feedback::free_axial_expansion`].
    pub fn free_axial_expansion(
        &self,
        delta_t: TemperatureInterval,
        length: Length,
    ) -> Displacement {
        free_axial_expansion(self.material.thermal_expansion(), delta_t, length)
    }

    /// Cumulative axial-displacement profile of a stack of cells at per-cell
    /// temperature rises `delta_t` and heights `cell_height`. See
    /// [`feedback::axial_expansion_profile`].
    pub fn axial_expansion_profile(
        &self,
        delta_t: &[TemperatureInterval],
        cell_height: &[Length],
    ) -> Vec<Displacement> {
        axial_expansion_profile(self.material.thermal_expansion(), delta_t, cell_height)
    }

    /// Assemble the coupled mesh-displacement vector `meshDisp` (metres) from an
    /// elastic displacement, an axial expansion, and the pin direction. See
    /// [`feedback::assemble_mesh_displacement`].
    pub fn mesh_displacement(
        &self,
        disp: Vector3,
        axial: Displacement,
        pin_direction: Vector3,
    ) -> Vector3 {
        assemble_mesh_displacement(disp, axial, pin_direction)
    }
}

/// The set of thermo-mechanics models, dispatched by `enum` (no trait objects,
/// per workspace rules).
///
/// GeN-Foam selects between `legacyThermoMechanics` (the linear-elastic model
/// ported here) and `extendedThermoMechanics` (an OFFBEAT-based fuel-performance
/// model) at run time. Only the legacy model is ported; the extended model is a
/// separate, much larger subsystem and is **deferred**. The enum exists so that
/// adding it later forces every dispatch site to handle it (exhaustiveness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermoMechanicsModel {
    /// Linear-elastic multi-zone model (`legacyThermoMechanics`).
    Legacy(LegacyThermoMechanics),
}

impl ThermoMechanicsModel {
    /// Full Cauchy stress tensor `σ` (Pa) for the given strain and temperature
    /// rise, dispatched to the active model.
    pub fn stress(&self, strain: SymmTensor, delta_t: TemperatureInterval) -> SymmTensor {
        match self {
            Self::Legacy(m) => m.stress(strain, delta_t),
        }
    }

    /// von Mises equivalent stress `σ_eq` (Pa), dispatched to the active model.
    pub fn von_mises(&self, strain: SymmTensor, delta_t: TemperatureInterval) -> Stress {
        match self {
            Self::Legacy(m) => m.von_mises(strain, delta_t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity};
    use uom::si::length::{meter, millimeter};
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::{gigapascal, megapascal};
    use uom::si::ratio::ratio;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::temperature_interval::kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    fn model() -> ThermoMechanicsModel {
        let steel = ElasticMaterial::new(
            YoungsModulus::new::<gigapascal>(200.0),
            Ratio::new::<ratio>(0.3),
            ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
            MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
        )
        .unwrap();
        ThermoMechanicsModel::Legacy(LegacyThermoMechanics::new(
            steel,
            RheologyOption::PlaneStrain,
        ))
    }

    /// V&V — enum dispatch reproduces the fully-constrained-bar analytical stress.
    /// Methodology: ε = 0, ΔT = 100 K ⇒ σ = −3K α ΔT I with σ_m = −600 MPa,
    /// σ_eq = 0 for steel plane-strain. Result (2026-07-15): σ_m = −600.000 MPa,
    /// σ_eq = 0.000 MPa. Pass.
    #[test]
    fn dispatch_constrained_bar() {
        let m = model();
        let dt = TemperatureInterval::new::<kelvin>(100.0);
        let sigma = m.stress(SymmTensor::ZERO, dt);
        assert!((hydrostatic_stress(sigma).get::<megapascal>() + 600.0).abs() < 1e-6);
        assert!(m.von_mises(SymmTensor::ZERO, dt).get::<megapascal>().abs() < 1e-6);
    }

    #[test]
    fn model_axial_expansion_wired() {
        let ThermoMechanicsModel::Legacy(legacy) = model();
        let dz = legacy.free_axial_expansion(
            TemperatureInterval::new::<kelvin>(400.0),
            Length::new::<meter>(2.0),
        );
        assert!((dz.get::<millimeter>() - 9.6).abs() < 1e-9);
    }
}
