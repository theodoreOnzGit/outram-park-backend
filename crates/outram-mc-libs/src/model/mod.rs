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
//!   Ablation lives behind one explicit `advanced` entry ([`advanced`], #942;
//!   ~~not yet built~~ built 2026-10-17). The run's [`RunProvenance`] records what was on.
//! - **The user's statement of their input** ([`Basis`], [`PpmOf`],
//!   [`Placement`], enrichment, fuel family, temperature): required, no
//!   defaults, no warnings.
//!
//! ## Diagnostics, the report and `advanced` (wave 2: #940, #941, #942)
//!
//! - [`Diagnostics`]: what to collect beside k (six factors, spectra,
//!   balance, EALF, leakage, kinetics by IFP, maps), passed as
//!   `KeffSettings::new(mc, Some(Diagnostics::default()))`; results by
//!   `run.six_factors()?`, `run.spectrum(Region::Component(core))?`, ...
//! - [`DiagnosticsReport`]: `run.diagnostics_report().write_zip("out/x.zip")`
//!   (native) or `.to_zip_bytes()` (anywhere, wasm included): `report.md`
//!   with embedded CSV, `data/*.csv` and PNG plots.
//! - [`advanced`]: physics ablation (`model.advanced().without_dbrc()`) and
//!   maps on your own mesh. The only place either lives.
//!
//! ## Repeated structures (#926)
//!
//! A TRISO particle is concentric spheres, one `.containing` per statement;
//! a unit cell is a [`Cube`] of matrix containing it, with
//! `.boundary(Boundary::Reflective)` for a k-infinity; a [`Lattice`]
//! repeats tiles, its row order named by its constructor; and
//! [`AssembledModel::census`] counts the copies the assembled geometry
//! places (never from constants).
//!
//! ## Not yet here
//!
//! ~~Repeated structures (#926)~~ (lattices and unit cells landed
//! 2026-10-18; still to come on #926: random packing and the
//! double-heterogeneity region `DhRegion` of tutorial 5, lattices in a
//! sphere with whole-particle rejection, hex lattices), position-dependent
//! temperature (#945), and
//! the njoy → outram-mc data contract, which is open on #925
//! ([`NuclearData`] wraps today's path so it can be swapped).
//!
//! ## Names shared with the lower level
//!
//! `Material`, `Sphere`, `Lattice` and `KeffSettings` also name lower-level types in
//! `outram_mc_libs::prelude`. Use `outram_mc_libs::model::prelude::*` for
//! this API; the crate prelude re-exports the rest of it under its own
//! names.

pub mod advanced;
pub mod census;
pub mod collect;
pub mod data;
pub mod diagnostics;
pub mod isotope;
pub mod lattice;
mod lower;
pub mod masses;
pub mod material;
#[allow(clippy::module_inception)]
pub mod model;
pub mod report;
pub mod resolve;
pub mod run;
pub mod shape;

pub use advanced::AdvancedModel;
pub use collect::{
    BalanceRow, Ealf, Kinetics, Leakage, MCDiagnosticResult, MeshMap, NeutronBalance, RunHealth,
    SixFactors, Spectrum,
};
pub use diagnostics::{
    BalanceBy, DiagnosticError, Diagnostics, EnergyGrid, MCDiagnostic, Mesh, MeshBounds, Region,
};
pub use report::{DiagnosticsReport, OutputFile};

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
    Convergence, CustomRun, Keff, KeffRun, KeffSettings, MCSettings, NormalityCheck, Progress,
    NuclidePhysics, ReactivityTolerance, RunError, RunProvenance,
};
pub use census::{Census, CensusRow};
pub use lattice::Lattice;
pub use shape::{Boundary, Component, Cube, Cuboid, Shape, Solid, Sphere};

/// **Everything a beginner model needs, in one `use`**:
/// `use outram_mc_libs::model::prelude::*;` brings the types above and the
/// named nuclides (`U235`, `H1`, ...).
pub mod prelude {
    pub use super::isotopes::*;
    pub use super::{
        atoms_per_barn_cm, to_atoms_per_barn_cm, AdvancedModel, AssembledModel, AtomDensity,
        BalanceBy, Basis, Boundary, Census, CensusRow, Component, Cube, Cuboid, Lattice, Solid, Contaminant, Convergence, CustomRun, DataError,
        DiagnosticError, Diagnostics, DiagnosticsReport, EnergyGrid, Element, Enrichment,
        FissileUranium, FuelForm, GraphiteKind, HeavyMetal, Isotope, Keff, KeffRun, KeffSettings,
        MCDiagnostic, MCSettings, Material, MaterialError, Model, ModelError, Moderator,
        NuclearData, Placement, PpmOf, PuVector, ReactivityTolerance, Region, RunError, Shape,
        Species, Sphere, Temperature,
    };
    pub use super::{
        advanced, BalanceRow, Ealf, Kinetics, Leakage, MCDiagnosticResult, Mesh, MeshBounds,
        MeshMap, NeutronBalance, OutputFile, RunHealth, SixFactors, Spectrum,
    };
}
