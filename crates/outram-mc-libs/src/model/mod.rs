//! # The beginner API: materials, shapes, a model, k_eff
//!
//! **Epic gh #925**, children #937 (materials), #938 (geometry) and #939
//! (the model and the k_eff run). Godiva, the whole of it:
//!
//! ```no_run
//! use outram_mc_libs::model::prelude::*;
//! use uom::si::{f64::{Length, ThermodynamicTemperature}, length::centimeter, thermodynamic_temperature::kelvin};
//!
//! let mut godiva = Model::new(NuclearData::workspace_reference()?);
//! let core = godiva.component("Core");
//! let heu = Material::from_atom_densities([            // ICSBEP HEU-MET-FAST-001's own numbers
//!     (U234, atoms_per_barn_cm(4.9184e-4)),
//!     (U235, atoms_per_barn_cm(4.4994e-2)),
//!     (U238, atoms_per_barn_cm(2.4984e-3)),
//! ]);
//! godiva.add(Sphere::radius(Length::new::<centimeter>(8.7407)).filled_with(heu).in_component(core));
//! godiva.set_temperature(Temperature::Uniform(ThermodynamicTemperature::new::<kelvin>(293.6)));
//! let run = godiva.keff(KeffSettings::new(MCSettings::Auto, None))?;
//! println!("{run}");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## What belongs here, and what does not
//!
//! This is a **shell over the existing transport**, not new physics: a model
//! lowers to the CSG [`Geometry`](crate::geometry::geometry::Geometry),
//! [`Material`](crate::material::material::Material)s and
//! [`Nuclide`](crate::material::nuclide::Nuclide)s every recorded benchmark
//! in this crate is built from, and runs on the existing CSG power
//! iteration. Cells, regions, universes, surfaces with boundary conditions,
//! nuclide indices and run-size integers stay in the lower-level modules.
//!
//! Two kinds of choice, two treatments (maintainer, #925):
//!
//! - **Physics** (URR, DBRC, the thermal laws): always on and hidden here.
//!   Ablation will live behind one explicit `advanced` entry (#942, not yet
//!   built). The run's [`RunProvenance`] records what was on.
//! - **The user's statement of their input** ([`Basis`], [`PpmOf`],
//!   [`Placement`], enrichment, fuel family, temperature): required, no
//!   defaults, no warnings.
//!
//! ## Not yet here
//!
//! Diagnostics (#940; [`Diagnostics`] is a placeholder with no values, so
//! `None` is the only choice), the report zip (#941), `advanced` (#942),
//! repeated structures (#926), position-dependent temperature (#945), and
//! the njoy → outram-mc data contract, which is open on #925
//! ([`NuclearData`] wraps today's path so it can be swapped).
//!
//! ## Names shared with the lower level
//!
//! `Material`, `Sphere` and `KeffSettings` also name lower-level types in
//! `outram_mc_libs::prelude`. Use `outram_mc_libs::model::prelude::*` for
//! this API; the crate prelude re-exports the rest of it under its own
//! names.

pub mod data;
pub mod isotope;
pub mod masses;
pub mod material;
#[allow(clippy::module_inception)]
pub mod model;
pub mod resolve;
pub mod run;
pub mod shape;

pub use data::{DataError, NuclearData};
pub use isotope::{isotopes, Element, Isotope, IsotopeError, Species};
pub use masses::{AtomicMasses, MassSource, AVOGADRO_PER_MOL, BARN_CM_PER_CUBIC_CM, NEUTRON_MASS_U};
pub use material::{
    atoms_per_barn_cm, to_atoms_per_barn_cm, AtomDensity, Basis, Contaminant, Enrichment,
    FissileUranium, FuelForm, GraphiteKind, HeavyMetal, Material, Moderator, Placement, PpmOf,
    PuVector, SaltCarrier,
};
pub use model::{AssembledModel, GeometryProblem, Model, ModelError, NuclideKey, Temperature};
pub use resolve::{MaterialError, MaterialReport, NumberDensityComparison, ResolvedMaterial};
pub use run::{
    Convergence, CustomRun, Diagnostics, Keff, KeffRun, KeffSettings, MCSettings, NormalityCheck,
    NuclidePhysics, ReactivityTolerance, RunError, RunProvenance,
};
pub use shape::{Component, Shape, Sphere};

/// **Everything a beginner model needs, in one `use`**:
/// `use outram_mc_libs::model::prelude::*;` brings the types above and the
/// named nuclides (`U235`, `H1`, ...).
pub mod prelude {
    pub use super::isotopes::*;
    pub use super::{
        atoms_per_barn_cm, to_atoms_per_barn_cm, AssembledModel, AtomDensity, Basis, Component,
        Contaminant, Convergence, CustomRun, DataError, Diagnostics, Element, Enrichment,
        FissileUranium, FuelForm, GraphiteKind, HeavyMetal, Isotope, Keff, KeffRun, KeffSettings,
        MCSettings, Material, MaterialError, Model, ModelError, Moderator, NuclearData, Placement,
        PpmOf, PuVector, ReactivityTolerance, RunError, Shape, Species, Sphere, Temperature,
    };
}
