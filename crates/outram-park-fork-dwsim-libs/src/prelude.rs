// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! The one import a new user needs: `use outram_park_fork_dwsim_libs::prelude::*;`
//!
//! This crate has 21 top-level modules and, until 2026-09-10, no re-exports at
//! the root, so every call site had to know a full module path
//! (`columns::initial_estimates::RigorousColumn`,
//! `petroleum::crude_distillation::solve_crude_column`, …). The prelude is the
//! curated answer to that — added for GitHub issues #70 and #75, after the #58
//! dogfooding round found a missing prelude to be the single most predictive
//! API defect across the seven crates tried.
//!
//! **This is a map, not an index.** It carries the *entry points* — the types
//! you construct and the functions you call first — and deliberately leaves
//! the long tail where it is, with a pointer. A prelude that re-exports
//! everything is as unhelpful as none.
//!
//! # What is in it
//!
//! | Group | Names | Start here |
//! |---|---|---|
//! | **Crude oil → cut slate** | [`BlackOilCrude`], [`CrudeColumnConfig`], [`solve_crude_column`], [`CrudeColumnResult`], [`CutResult`], [`CrudeCut`], [`CrudeColumnError`], [`crude_column_setup`], [`CrudeColumnSetup`], [`CrudePlant`], [`CrudeCommands`], [`CrudeSnapshot`] | `solve_crude_column(&BlackOilCrude::light_sweet(), &CrudeColumnConfig::atmospheric_default(), 12)` |
//! | **Assay characterisation** | [`Assay`], [`BulkAssay`], [`CurveAssay`], [`characterize`], [`PseudoComponent`], [`CharacterizationError`] | `characterize(&Assay::Bulk(..), cut_count)` |
//! | **Thermodynamics** | [`Component`], [`ComponentError`], [`mod@reference`] (preset compounds), [`PropertyPackageModel`], [`PropertyPackage`], [`FlashResult`], [`FlashError`], [`bubble_temperature`], [`dew_temperature`], [`SaturationState`], [`SaturationError`] | `PropertyPackageModel::PengRobinson1978.flash_pt(&components, &z, t, p)` |
//! | **Name → component lookup** | [`component_by_name`], [`ReferenceCompound`], [`known_component_names`], [`ComponentLookupError`]; and for a whole stream slate [`StreamCompound`], [`resolve_components`], [`resolve_components_checked`], [`molar_mass_discrepancies`], [`MolarMassDiscrepancy`] | `component_by_name("benzene")?` — but note **only seven compounds have data**; see [`crate::thermo::registry`] |
//! | **Rigorous MESH column** | [`RigorousColumn`] (four constructors: `distillation`, `absorption`, `reboiled_absorber`, `refluxed_absorber`), [`Stage`], [`ColumnSpec`], [`SpecType`], [`SpecBasis`], [`ColumnType`], [`CondenserType`], [`InitialEstimates`], [`ColumnSolverInput`], [`ColumnSolverOutput`], [`ColumnError`], [`ColumnSolverMethod`], [`WangHenkeSolver`], [`ModifiedWangHenkeSolver`], [`SumRatesSolver`], [`NaphtaliSandholmSolver`], [`ColumnThermo`], and the `uom` aliases [`StagePressure`], [`StageTemperature`], [`MolarFlowRate`], [`MolarEnthalpy`], [`StageHeatDuty`], [`StageEfficiency`] | `RigorousColumn::distillation(..).solver_input()?` then `ColumnSolverMethod::default().solve(&input)?` |
//! | **Shortcut column (FUG)** | [`ShortcutColumn`], [`ShortcutFeed`], [`ShortcutCondenserType`], [`ShortcutColumnResult`], [`ShortcutColumnError`], [`UnderwoodMode`], [`ShortcutHeatDuty`] | `ShortcutColumn::new(..)` |
//! | **Unit operations with a struct entry point** | [`Separator`], [`SeparatorFeed`], [`SeparatorMode`], [`SeparatorResult`], [`SeparatorError`], [`PhaseOutlet`]; [`InletStream`], [`PressureBehavior`], [`MixerOutlet`], [`MixerError`]; [`SplitSpec`], [`SplitResult`], [`SplitError`], [`OutletStream`], [`IntensiveState`]; [`PumpInlet`], [`PumpSpecification`], [`PumpResult`]; [`PipeFlowInputs`], [`PipeFlowCorrelation`], [`PipeFlowResult`] | the struct's own docs |
//! | **Reactions & reactors** | [`Reaction`], [`ReactionComponent`], [`ReactionKind`], [`ReactionBasis`], [`ReactorModel`], [`ReactorFeed`], [`ReactorOutcome`], [`ReactorError`], [`ConversionReactor`], [`Cstr`], [`EquilibriumReactor`], [`GibbsReactor`], [`Pfr`] | `ReactorModel` |
//! | **Clean energies** | [`CleanEnergyUnit`], [`CleanEnergyError`], [`WaterElectrolyzer`], [`PemFuelCell`], [`SolarPanel`], [`WindTurbine`], [`HydroelectricTurbine`] | `CleanEnergyUnit` |
//! | **Units** | [`units`] — the `uom` unit markers this crate's public API takes (`katal` = mol/s, `pascal`, `kelvin`, `joule_per_mole`, `watt`, `ratio`, `gram_per_mole`, …), and [`uom`] itself | `MolarFlowRate::new::<units::katal>(1.0)` |
//!
//! # Units, spelled out
//!
//! Every public quantity is `uom`-typed at the boundary. The one that trips
//! everybody: **molar flow is `MolarFlowRate::new::<katal>(mol_per_second)`** —
//! `uom` 0.38 has no molar-flow-rate quantity, and the katal (catalytic
//! activity) is dimensionally exactly mol/s. Pressure is `pascal`, temperature
//! `kelvin`, molar enthalpy `joule_per_mole`, heat duty `watt` (see
//! [`StageHeatDuty`] for the sign convention), efficiency and mole fraction
//! `ratio`. The markers live in [`units`] so you do not need `uom` in your own
//! `Cargo.toml`; [`uom`] is re-exported for anything else.
//!
//! # What is deliberately NOT in it, and where it is
//!
//! - **The flowsheet, solver and dynamics layers** ([`crate::flowsheet`],
//!   [`crate::flowsheet_solver`], [`crate::dynamics`]). Each is a subsystem with
//!   fifty-plus public names and its own `pub use` map at its module root;
//!   importing it here would drown the physics entry points. Start from
//!   [`crate::flowsheet::Flowsheet`] and
//!   [`crate::flowsheet_solver::FlowsheetSolver`]. **One deliberate
//!   exception**, added 2026-09-11: [`crate::flowsheet::component_basis`]'s
//!   name → component resolution and the [`StreamCompound`] it takes. That
//!   function is a thermodynamics entry point that merely lives in `flowsheet`
//!   (it is the only way to get from a stream's names to the `&[Component]`
//!   every flash needs), and its argument type has to come with it to be
//!   callable.
//! - **Function-only unit-op modules** — [`crate::heater`], [`crate::cooler`],
//!   [`crate::compressor`], [`crate::expander`], [`crate::valve`],
//!   [`crate::heat_exchanger`]. They expose correlations as free functions with
//!   generic names (`outlet_pressure`, `duty_constant_cp`, `consumed_power`),
//!   which would collide with each other and with your own code under a glob
//!   import. Call them by module path. For the same reason the mixer, splitter
//!   and pump *functions* (`mixer::mix`, `splitter::split`,
//!   `pump::modes::evaluate`) stay out while their *types* are in.
//! - **The thermo kernel's internals** — cubic EOS roots, activity models
//!   (UNIFAC, NRTL…), the electrolyte tier, the individual flash algorithms
//!   (`flash_vlle`, `flash_sle`, inside-out…), stability analysis, transport
//!   properties. [`PropertyPackageModel`] is the door to all of them; go to
//!   [`crate::thermo`] when you need to open a specific one.
//! - **Petroleum characterisation internals** — the individual property
//!   correlations ([`crate::petroleum::riazi`], `property_methods`, `gl`,
//!   `fitting`, `quality_check`). [`characterize`] and [`BlackOilCrude`] call
//!   them; reach for [`crate::petroleum`]'s own re-exports to tune them.
//! - **Solver-internal types** — [`crate::columns::StageProfile`], the
//!   tridiagonal and root-finding kernels, [`crate::interpolation`]. Not user
//!   entry points.
//! - **Every `MolarFlowRate` but one.** [`crate::columns`], [`crate::separator`],
//!   [`crate::splitter`] and [`crate::flowsheet::streams`] each alias the same
//!   `uom` type under that name; re-exporting all four would be a name clash.
//!   The prelude's [`MolarFlowRate`] is the columns one, and it is the same
//!   type as the others.
//!
//! # Example — the whole crude workflow from this one import
//!
//! ```
//! use outram_park_fork_dwsim_libs::prelude::*;
//!
//! // 38 °API. The 22 °API `BlackOilCrude::heavy()` no longer characterises at
//! // 12 cuts: its heaviest cut has a negative critical volume, which the
//! // characterisation refuses instead of emitting (GitHub #170).
//! let crude = BlackOilCrude::light_sweet();
//! let config = CrudeColumnConfig::atmospheric_default(); // PengRobinson1978
//! let result = solve_crude_column(&crude, &config, 12).expect("column converges");
//! for cut in &result.cuts {
//!     println!("{:>8}  {:.5} mol/s at {:.1} K", cut.cut.label(), cut.flow_mol_s, cut.temperature_k);
//! }
//! assert!((result.total_product_mol_s() - config.feed_flow_mol_s).abs() < 1e-9);
//! ```
//!
//! `examples/crude_column_from_prelude.rs` is the long form of that, readable
//! top to bottom, and `tests/prelude_workflow.rs` proves it needs no other
//! import.

// ── Crude oil → cut slate ────────────────────────────────────────────────────
pub use crate::petroleum::crude_distillation::{
    crude_column_setup, solve_crude_column, BlackOilCrude, CrudeColumnConfig, CrudeColumnError,
    CrudeColumnResult, CrudeColumnSetup, CrudeCut, CutResult,
};
pub use crate::petroleum::crude_plant::{CrudeCommands, CrudePlant, CrudeSnapshot};

// ── Assay characterisation ───────────────────────────────────────────────────
pub use crate::petroleum::{
    characterize, Assay, BulkAssay, CharacterizationError, CurveAssay, PseudoComponent,
};

// ── Thermodynamics ───────────────────────────────────────────────────────────
pub use crate::thermo::component::{reference, Component, ComponentError};
pub use crate::thermo::registry::{
    component_by_name, known_component_names, ComponentLookupError, ReferenceCompound,
};

// ── Stream slate → component slate ───────────────────────────────────────────
// The one deliberate exception to "the flowsheet layer stays out of the
// prelude": resolving a stream's compound names into the `Component` slate the
// thermo kernel takes is a *thermodynamics* entry point that happens to live in
// `flowsheet`, and `StreamCompound` comes with it because it is the argument
// type. See the module docs above.
pub use crate::flowsheet::component_basis::{
    molar_mass_discrepancies, resolve_components, resolve_components_checked, MolarMassDiscrepancy,
};
pub use crate::flowsheet::streams::StreamCompound;
pub use crate::thermo::flash::{FlashError, FlashResult};
pub use crate::thermo::property_package::{PropertyPackage, PropertyPackageModel};
pub use crate::thermo::saturation::{
    bubble_temperature, dew_temperature, SaturationError, SaturationState,
};

// ── Rigorous MESH column ─────────────────────────────────────────────────────
pub use crate::columns::bubble_point::WangHenkeSolver;
pub use crate::columns::bubble_point2::ModifiedWangHenkeSolver;
pub use crate::columns::initial_estimates::RigorousColumn;
pub use crate::columns::newton_raphson::NaphtaliSandholmSolver;
pub use crate::columns::sum_rates::SumRatesSolver;
pub use crate::columns::thermo_bridge::ColumnThermo;
pub use crate::columns::{
    ColumnError, ColumnSolverInput, ColumnSolverMethod, ColumnSolverOutput, ColumnSpec, ColumnType,
    CondenserType, InitialEstimates, MolarEnthalpy, MolarFlowRate, SpecBasis, SpecType, Stage,
    StageEfficiency, StageHeatDuty, StagePressure, StageTemperature,
};

// ── Shortcut (Fenske-Underwood-Gilliland) column ─────────────────────────────
pub use crate::columns::{
    ShortcutColumn, ShortcutColumnError, ShortcutColumnResult, ShortcutCondenserType, ShortcutFeed,
    ShortcutHeatDuty, UnderwoodMode,
};

// ── Unit operations with a struct entry point ────────────────────────────────
pub use crate::mixer::{InletStream, MixerError, MixerOutlet, PressureBehavior};
pub use crate::pipe::{PipeFlowCorrelation, PipeFlowInputs, PipeFlowResult};
pub use crate::pump::modes::{PumpInlet, PumpResult, PumpSpecification};
pub use crate::separator::{
    PhaseOutlet, Separator, SeparatorError, SeparatorFeed, SeparatorMode, SeparatorResult,
};
pub use crate::splitter::{IntensiveState, OutletStream, SplitError, SplitResult, SplitSpec};

// ── Reactions & reactors ─────────────────────────────────────────────────────
pub use crate::reactions::{Reaction, ReactionBasis, ReactionComponent, ReactionKind};
pub use crate::reactors::{
    ConversionReactor, Cstr, EquilibriumReactor, GibbsReactor, Pfr, ReactorError, ReactorFeed,
    ReactorModel, ReactorOutcome,
};

// ── Clean energies ───────────────────────────────────────────────────────────
pub use crate::clean_energies::{
    CleanEnergyError, CleanEnergyUnit, HydroelectricTurbine, PemFuelCell, SolarPanel,
    WaterElectrolyzer, WindTurbine,
};

// ── Units ────────────────────────────────────────────────────────────────────
/// The `uom` crate this library is built against, re-exported so a caller does
/// not have to add it (at exactly the same version) to their own `Cargo.toml`
/// to construct the quantities the public API takes.
pub use uom;

/// The `uom` unit markers this crate's public API takes, in one place.
///
/// Use as `MolarFlowRate::new::<units::katal>(1.0)` (1 mol/s),
/// `StagePressure::new::<units::pascal>(101_325.0)`,
/// `StageTemperature::new::<units::kelvin>(353.15)`,
/// `MolarEnthalpy::new::<units::joule_per_mole>(-25_000.0)`,
/// `StageHeatDuty::new::<units::watt>(0.0)`,
/// `StageEfficiency::new::<units::ratio>(1.0)`.
///
/// | Marker | Quantity | SI unit |
/// |---|---|---|
/// | [`katal`](units::katal) | [`MolarFlowRate`] (molar flow — the katal *is* mol/s) | mol/s |
/// | [`pascal`](units::pascal), [`bar`](units::bar) | [`StagePressure`], `uom::si::f64::Pressure` | Pa |
/// | [`kelvin`](units::kelvin), [`degree_celsius`](units::degree_celsius) | [`StageTemperature`], `uom::si::f64::ThermodynamicTemperature` | K |
/// | [`joule_per_mole`](units::joule_per_mole) | [`MolarEnthalpy`], `uom::si::f64::MolarEnergy` | J/mol |
/// | [`watt`](units::watt) | [`StageHeatDuty`], `uom::si::f64::Power` | W |
/// | [`ratio`](units::ratio) | [`StageEfficiency`], mole fractions, specific gravity — `uom::si::f64::Ratio` | - |
/// | [`gram_per_mole`](units::gram_per_mole), [`kilogram_per_mole`](units::kilogram_per_mole) | [`BulkAssay::molar_mass`], `uom::si::f64::MolarMass` | kg/mol |
/// | [`kilogram_per_second`](units::kilogram_per_second) | `uom::si::f64::MassRate` (mixer, splitter, pump) | kg/s |
/// | [`joule_per_kilogram`](units::joule_per_kilogram) | `uom::si::f64::AvailableEnergy` (specific enthalpy) | J/kg |
/// | [`kilogram_per_cubic_meter`](units::kilogram_per_cubic_meter) | `uom::si::f64::MassDensity` | kg/m³ |
/// | [`meter`](units::meter) | `uom::si::f64::Length` | m |
/// | [`second`](units::second) | `uom::si::f64::Time` | s |
pub mod units {
    pub use uom::si::available_energy::joule_per_kilogram;
    pub use uom::si::catalytic_activity::katal;
    pub use uom::si::length::meter;
    pub use uom::si::mass_density::kilogram_per_cubic_meter;
    pub use uom::si::mass_rate::kilogram_per_second;
    pub use uom::si::molar_energy::joule_per_mole;
    pub use uom::si::molar_mass::{gram_per_mole, kilogram_per_mole};
    pub use uom::si::power::watt;
    pub use uom::si::pressure::{bar, pascal};
    pub use uom::si::ratio::ratio;
    pub use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
    pub use uom::si::time::second;
}
