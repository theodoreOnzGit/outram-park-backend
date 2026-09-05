//! Clean-energy unit operations: electrolysis, PEM fuel cells, solar, wind, hydro.
//!
//! Pure-Rust port of DWSIM's `CleanEnergies` unit-operation family.
//!
//! # Attribution
//!
//! - **Upstream project:** DWSIM — Open Source Process Simulator
//! - **Source directory:** `DWSIM.UnitOperations/UnitOperations/CleanEnergies/`
//! - **Upstream commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`)
//! - **Upstream copyright:** Daniel Wagner O. de Medeiros and the DWSIM contributors
//! - **Upstream licence:** GPL-3.0
//! - **This port:** GPL-3.0-only (OUTRAM PARK fork; not the official DWSIM software)
//!
//! # What this module contains
//!
//! Five unit operations, one submodule each, all translated from the
//! `Public Overrides Sub Calculate` routine of the matching `.vb` file:
//!
//! | Submodule | DWSIM source | Physics |
//! |---|---|---|
//! | [`water_electrolyzer`] | `WaterElectrolyzer.vb` | Faraday-law water electrolysis, reversible/thermoneutral cell voltage, waste heat |
//! | [`pem_fuel_cell`] | `PEMFuelCellUnitOpBase.vb`, `PEMFC_Amphlett.vb`, `PEMFC_ChamberlineKim.vb`, `PEMFC_LarminieDicks.vb` | Three PEM polarization models + fuel-cell stoichiometry |
//! | [`solar_panel`] | `SolarPanel.vb` | Irradiance × area × efficiency |
//! | [`wind_turbine`] | `WindTurbine.vb` | Betz-limited rotor power |
//! | [`hydroelectric_turbine`] | `HydroelectricTurbine.vb` | Static + velocity head, `eta·rho·g·H·Q` |
//!
//! # Design: trait as contract, enum for dispatch
//!
//! DWSIM expresses the family through .NET inheritance —
//! `CleanEnergyUnitOpBase` (`CleanEnergyUnitOpBase.vb:6-62`) inherits
//! `UnitOpBaseClass` and implements the `IExternalUnitOperation` interface, and
//! `PEMFuelCellUnitOpBase` (`PEMFuelCellUnitOpBase.vb:63-389`) is a parallel
//! base for the three fuel cells. Per the workspace "no trait objects" rule
//! that inheritance is rendered here as:
//!
//! - the [`CleanEnergyUnitOp`] **trait**, used purely as a *compile-time
//!   contract* so the compiler checks every concrete unit exposes the same
//!   descriptive surface; and
//! - the [`CleanEnergyUnit`] **enum**, which is what any dispatch goes
//!   through. Adding a sixth unit is then a compile error at every `match`
//!   site rather than a runtime surprise. No `Box<dyn …>`, `&dyn …` or
//!   `Arc<dyn …>` appears anywhere in this module.
//!
//! # Design: flash boundary pushed to the caller
//!
//! Every DWSIM routine here finishes by writing `(P, H, w, composition)` onto
//! an outlet material stream and letting the flowsheet solver run a PH or PT
//! flash. Following the same convention as [`crate::mixer`],
//! [`crate::heater`] and [`crate::expander`], **no flash is performed in this
//! module**. Calls into DWSIM's property package (ideal-gas reaction
//! enthalpy, heat of vaporization, vapour pressure, humid-air density) become
//! plain *inputs* to the free functions here, and the outlet state each
//! routine returns is the input to a caller-side flash.
//!
//! # Design: weather is an input, not a network call
//!
//! `SolarPanel.vb:252-262` and `WindTurbine.vb:339-358` read ambient
//! conditions either from user-entered fields or from the flowsheet's
//! `CurrentWeather` object, which DWSIM populates from an online weather
//! service. The port keeps only the physics: irradiance, wind speed, air
//! temperature/pressure/humidity are ordinary function arguments. No network
//! access exists in this crate, and none is introduced. The user/global
//! selector itself survives as the [`WeatherSource`] enum so the distinction
//! DWSIM records is not lost.
//!
//! # Units
//!
//! Public APIs are `uom`-typed. Inner arithmetic is raw `f64` in SI (or in
//! the model-specific units the correlations are stated in — atm for PEM
//! partial pressures, cm² for active area, kJ/mol for reaction enthalpies),
//! documented at each call site. DWSIM carries energy flows in **kW** and
//! mass enthalpies in **kJ/kg**; that scaling is converted at the `uom`
//! boundary so users of this module always see watts and J/kg.
//!
//! # Excluded DWSIM behaviour (whole module)
//!
//! Deliberately **not** ported anywhere in this module, because none of it is
//! physics:
//!
//! - **SkiaSharp drawing and icon resources** — `Draw`, `GetIconBitmap`,
//!   `GetIconBitmapBytes` (`WaterElectrolyzer.vb:164-190, :349-359`;
//!   `PEMFuelCellUnitOpBase.vb:111-137`; `PEMFC_Amphlett.vb:58-68`;
//!   `PEMFC_ChamberlineKim.vb:32-36`; `PEMFC_LarminieDicks.vb:46-50`;
//!   `SolarPanel.vb:74-100, :201-211`; `WindTurbine.vb:96-122, :288-298`;
//!   `HydroelectricTurbine.vb:55-81, :256-266`).
//! - **Flowsheet graphic connectors** — `CreateConnectors`
//!   (`WaterElectrolyzer.vb:192-252`; `PEMFuelCellUnitOpBase.vb:139-199`;
//!   `SolarPanel.vb:102-127`; `WindTurbine.vb:124-149`;
//!   `HydroelectricTurbine.vb:83-…`).
//! - **Eto.Forms editor panels and editing forms** — `PopulateEditorPanel`,
//!   `DisplayEditForm`, `UpdateEditForm`, `CloseEditForm`
//!   (`WaterElectrolyzer.vb:254-282, :302-341`;
//!   `PEMFuelCellUnitOpBase.vb:282-321, :339`; `PEMFC_Amphlett.vb:278-296`;
//!   `SolarPanel.vb:130-173, :270-309`; `WindTurbine.vb:151-215, :241-280`;
//!   `HydroelectricTurbine.vb:…-248`).
//! - **XML/JSON serialization and cloning** — `SaveData`, `LoadData`,
//!   `CloneXML`, `CloneJSON`, `ReturnInstance`, and the
//!   `PEMFuelCellModelParameter` XML container
//!   (`PEMFuelCellUnitOpBase.vb:12-57, :225-280`;
//!   `WaterElectrolyzer.vb:343-392`; `PEMFC_Amphlett.vb:52-82`;
//!   `PEMFC_ChamberlineKim.vb:26-50`; `PEMFC_LarminieDicks.vb:40-64`;
//!   `SolarPanel.vb:195-244`; `WindTurbine.vb:282-331`;
//!   `HydroelectricTurbine.vb:250-299`).
//! - **Property-grid reflection accessors** — `GetProperties`,
//!   `GetPropertyValue`, `GetPropertyUnit`, `SetPropertyValue`
//!   (`WaterElectrolyzer.vb:76-156`; `PEMFuelCellUnitOpBase.vb:341-387`;
//!   `SolarPanel.vb:311-379`; `WindTurbine.vb:412-530`;
//!   `HydroelectricTurbine.vb:342-416`) — and the human-readable
//!   `GetReport` string builders (`WaterElectrolyzer.vb:284-300`;
//!   `PEMFC_Amphlett.vb:298-308`; `SolarPanel.vb:175-193`;
//!   `WindTurbine.vb:217-239`). The *values* those accessors expose are all
//!   fields of the ported result structs, so nothing physical is lost.
//! - **Dimension bookkeeping for the equipment-sizing GUI** —
//!   `CreateDimensionsList` / `UpdateDimensionsList`
//!   (`WaterElectrolyzer.vb:31-44`; `SolarPanel.vb:31-44`;
//!   `WindTurbine.vb:35-48`).
//! - **The solver-callback and .NET plumbing of the base class** —
//!   `CallSolverIfNeeded`, `PerformPostCalcValidation`, `MobileCompatible`,
//!   `ObjectClass`, the `IExternalUnitOperation` interface members and the
//!   two constructors (`CleanEnergyUnitOpBase.vb:26-60`;
//!   `PEMFuelCellUnitOpBase.vb:86-107, :201-223`).
//! - **The embedded CPython runtime** — `Python.Runtime`, `Py.GIL`,
//!   `InitializePythonEnvironment`, the `ToList` PyObject marshaller, the
//!   temp-file HTML/CSV/OPEM report round-trip, and the `HTMLreport` /
//!   `CSVreport` / `OPEMreport` string fields
//!   (`PEMFuelCellUnitOpBase.vb:75-79, :323-337`;
//!   `PEMFC_Amphlett.vb:86-90, :131-193, :273`). See [`pem_fuel_cell`] for
//!   how the physics behind that call is recovered.
//! - **Live weather-service lookups** — `FlowsheetOptions.CurrentWeather`
//!   (`SolarPanel.vb:258`; `WindTurbine.vb:348-351`), replaced by plain
//!   inputs as described above.

pub mod hydroelectric_turbine;
pub mod pem_fuel_cell;
pub mod solar_panel;
pub mod water_electrolyzer;
pub mod wind_turbine;

pub use hydroelectric_turbine::HydroelectricTurbine;
pub use pem_fuel_cell::{PemFuelCell, PemFuelCellModel};
pub use solar_panel::SolarPanel;
pub use water_electrolyzer::WaterElectrolyzer;
pub use wind_turbine::WindTurbine;

use uom::si::f64::Power;

/// Where a unit operation's ambient conditions come from — DWSIM's
/// `CleanEnergyUnitOpBase.UseUserDefinedWeather` boolean
/// (`CleanEnergyUnitOpBase.vb:12`), rendered as a named two-state enum rather
/// than a bare `bool` so call sites read unambiguously.
///
/// This port never fetches weather. [`WeatherSource::Global`] therefore means
/// "the caller supplies ambient conditions obtained from its own weather
/// source"; it is recorded for provenance and reporting, and changes no
/// arithmetic in this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherSource {
    /// DWSIM's `UseUserDefinedWeather = False` (the default): ambient
    /// conditions come from the flowsheet's global weather object.
    #[default]
    Global,
    /// DWSIM's `UseUserDefinedWeather = True`: ambient conditions are the
    /// unit's own user-entered fields.
    UserDefined,
}

/// Compile-time contract shared by every clean-energy unit operation — the
/// Rust stand-in for DWSIM's `CleanEnergyUnitOpBase` /
/// `PEMFuelCellUnitOpBase` inheritance (`CleanEnergyUnitOpBase.vb:6-62`,
/// `PEMFuelCellUnitOpBase.vb:63-389`).
///
/// The trait exists so the compiler verifies each concrete unit exposes the
/// same descriptive surface. It is **never** used for runtime dispatch — that
/// is [`CleanEnergyUnit`]'s job — and so is deliberately object-unsafe-free
/// of any need for `dyn`.
///
/// All clean-energy units are *sources* in DWSIM's sense
/// (`CleanEnergyUnitOpBase.vb:14`, `IsSource = True`): they inject power into
/// the flowsheet rather than merely transforming a stream.
pub trait CleanEnergyUnitOp {
    /// DWSIM's `GetDisplayName()` — the human-facing name of the unit type,
    /// e.g. `"Water Electrolyzer"`.
    fn display_name(&self) -> &'static str;

    /// DWSIM's `Prefix` property — the tag prefix auto-assigned to new
    /// instances, e.g. `"WE-"` for the electrolyzer
    /// (`WaterElectrolyzer.vb:54`).
    fn prefix(&self) -> &'static str;

    /// Electrical power this unit delivers to (positive) or draws from
    /// (negative) the flowsheet, in watts.
    ///
    /// Generators (solar, wind, hydro, fuel cell) return a positive power.
    /// The electrolyzer is a consumer and returns a negative power. Returns
    /// zero watts before the unit has been given an operating point.
    fn generated_power(&self) -> Power;
}

/// Closed set of clean-energy unit operations, for dispatch.
///
/// This is the enum that replaces DWSIM's polymorphic
/// `CleanEnergyUnitOpBase` reference. Adding a unit type here makes every
/// `match` over `CleanEnergyUnit` a compile error until it is handled, which
/// is exactly the property the workspace design rules ask for. The enum owns
/// each unit by value — no `Box`, no `Arc`, no lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub enum CleanEnergyUnit {
    /// Water electrolyzer — see [`water_electrolyzer`].
    WaterElectrolyzer(WaterElectrolyzer),
    /// PEM fuel cell (any of the three polarization models) — see
    /// [`pem_fuel_cell`].
    PemFuelCell(PemFuelCell),
    /// Photovoltaic panel array — see [`solar_panel`].
    SolarPanel(SolarPanel),
    /// Wind turbine (or wind farm of identical turbines) — see
    /// [`wind_turbine`].
    WindTurbine(WindTurbine),
    /// Hydroelectric turbine — see [`hydroelectric_turbine`].
    HydroelectricTurbine(HydroelectricTurbine),
}

impl CleanEnergyUnitOp for CleanEnergyUnit {
    fn display_name(&self) -> &'static str {
        match self {
            Self::WaterElectrolyzer(u) => u.display_name(),
            Self::PemFuelCell(u) => u.display_name(),
            Self::SolarPanel(u) => u.display_name(),
            Self::WindTurbine(u) => u.display_name(),
            Self::HydroelectricTurbine(u) => u.display_name(),
        }
    }

    fn prefix(&self) -> &'static str {
        match self {
            Self::WaterElectrolyzer(u) => u.prefix(),
            Self::PemFuelCell(u) => u.prefix(),
            Self::SolarPanel(u) => u.prefix(),
            Self::WindTurbine(u) => u.prefix(),
            Self::HydroelectricTurbine(u) => u.prefix(),
        }
    }

    fn generated_power(&self) -> Power {
        match self {
            Self::WaterElectrolyzer(u) => u.generated_power(),
            Self::PemFuelCell(u) => u.generated_power(),
            Self::SolarPanel(u) => u.generated_power(),
            Self::WindTurbine(u) => u.generated_power(),
            Self::HydroelectricTurbine(u) => u.generated_power(),
        }
    }
}

/// Faraday's constant `F` \[C/mol\], as DWSIM spells it literally in
/// `WaterElectrolyzer.vb:448-449, :463` (`96485.3365`).
///
/// The 2019 SI exact value is 96485.332 12 C/mol; DWSIM's constant differs in
/// the 8th significant figure, which is far below any modelling uncertainty
/// here. The upstream literal is kept so this port reproduces DWSIM's numbers
/// bit-for-bit.
pub const FARADAY_CONSTANT_C_PER_MOL: f64 = 96485.3365;

/// Standard gravitational acceleration `g` \[m/s²\] as DWSIM uses it in the
/// hydroelectric turbine (`HydroelectricTurbine.vb:314`, `Dim g = 9.8`).
///
/// This is **not** the CODATA standard value 9.806 65 m/s²; DWSIM rounds to
/// 9.8, a 0.07 % difference that propagates directly into the generated
/// power. The upstream literal is kept for numerical parity — see
/// [`hydroelectric_turbine`] for the consequence.
pub const GRAVITY_M_PER_S2: f64 = 9.8;

/// Errors shared across the clean-energy unit operations.
///
/// DWSIM raises these as plain `Exception`s with English messages
/// (`WaterElectrolyzer.vb:401, :411, :415-421, :469, :494, :510, :548`;
/// `PEMFC_Amphlett.vb:99, :106-108, :169, :253, :256`). They become typed
/// variants here so callers can branch on the failure rather than parse a
/// string.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum CleanEnergyError {
    /// The applied cell voltage is below the reversible (Nernst) voltage, so
    /// electrolysis cannot proceed — DWSIM's "Total Voltage too low."
    /// (`WaterElectrolyzer.vb:469`).
    #[error(
        "cell voltage {cell_voltage_v} V is below the reversible voltage {reversible_voltage_v} V; \
         electrolysis cannot proceed"
    )]
    CellVoltageBelowReversible {
        /// Applied per-cell voltage \[V\] = stack voltage / number of cells.
        cell_voltage_v: f64,
        /// Reversible (Nernst) voltage `V_rev` \[V\] at the operating
        /// temperature.
        reversible_voltage_v: f64,
    },
    /// A species' outlet molar flow came out negative — DWSIM's "Negative
    /// {0} molar flow calculated. Increase water rate in inlet stream or
    /// reduce power." (`WaterElectrolyzer.vb:510, :548`) and "Negative
    /// Hydrogen/Oxygen molar flow calculated." (`PEMFC_Amphlett.vb:253, :256`).
    #[error("negative outlet molar flow for {species}: {molar_flow_mol_per_s} mol/s")]
    NegativeMolarFlow {
        /// Name of the offending species, as DWSIM names it.
        species: &'static str,
        /// The computed (negative) molar flow \[mol/s\].
        molar_flow_mol_per_s: f64,
    },
    /// Neither of the electrolyzer's two specification branches was
    /// satisfied — DWSIM's "Specify total voltage and number of cells or set
    /// both to zero and specify efficiency between 0 and 1"
    /// (`WaterElectrolyzer.vb:494`).
    #[error(
        "electrolyzer under-specified: give a positive total voltage with a positive cell count, \
         or an efficiency in (0, 1]"
    )]
    UnderspecifiedElectrolyzer,
    /// A model input fell outside the domain where its correlation is
    /// defined (a logarithm of a non-positive number, a division by zero, a
    /// current at or above the limiting current, …). This port reports the
    /// condition; OPEM's Python original prints an error and yields `None`,
    /// and DWSIM then propagates a "Calculation error"
    /// (`PEMFC_Amphlett.vb:169`).
    #[error("{parameter} = {value} is outside the valid domain for this correlation: {reason}")]
    OutOfDomain {
        /// Name of the offending parameter.
        parameter: &'static str,
        /// Its value.
        value: f64,
        /// Why the value is unusable.
        reason: &'static str,
    },
    /// A current sweep was requested with a non-positive step, an empty
    /// range, or otherwise produced no operating points. OPEM's
    /// `filter_range` silently repairs some of these; this port reports them.
    #[error("empty or invalid current sweep: start {start_a} A, stop {stop_a} A, step {step_a} A")]
    EmptySweep {
        /// Sweep start current \[A\].
        start_a: f64,
        /// Sweep stop current \[A\].
        stop_a: f64,
        /// Sweep step \[A\].
        step_a: f64,
    },
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! Verification only — these check the enum-dispatch wiring, not physics.
    //! Measured 2026-08-11.
    use super::*;
    use uom::si::power::watt;

    /// Methodology: build one of each unit, wrap in [`CleanEnergyUnit`], and
    /// confirm the enum forwards `display_name` / `prefix` /
    /// `generated_power` to the wrapped struct (the Rust replacement for
    /// DWSIM's virtual dispatch through `CleanEnergyUnitOpBase`).
    /// Result (2026-08-11): all five names and prefixes match the upstream
    /// `GetDisplayName()` / `Prefix` literals; every freshly built unit
    /// reports 0 W.
    #[test]
    fn enum_dispatch_forwards_to_each_unit() {
        let units = [
            CleanEnergyUnit::WaterElectrolyzer(WaterElectrolyzer::default()),
            CleanEnergyUnit::PemFuelCell(PemFuelCell::default()),
            CleanEnergyUnit::SolarPanel(SolarPanel::default()),
            CleanEnergyUnit::WindTurbine(WindTurbine::default()),
            CleanEnergyUnit::HydroelectricTurbine(HydroelectricTurbine::default()),
        ];
        let expected_names = [
            "Water Electrolyzer",
            "PEM Fuel Cell (Amphlett)",
            "Solar Panel",
            "Wind Turbine",
            "Hydroelectric Turbine",
        ];
        let expected_prefixes = ["WE-", "FCA-", "SP-", "WT-", "HT-"];
        for (i, u) in units.iter().enumerate() {
            assert_eq!(u.display_name(), expected_names[i]);
            assert_eq!(u.prefix(), expected_prefixes[i]);
            assert_eq!(u.generated_power().get::<watt>(), 0.0);
        }
    }

    /// Methodology: confirm the two DWSIM literal constants are carried
    /// through unchanged (`WaterElectrolyzer.vb:448`,
    /// `HydroelectricTurbine.vb:314`).
    /// Result (2026-08-11): `F = 96485.3365 C/mol`, `g = 9.8 m/s²`, both
    /// exact.
    #[test]
    fn upstream_constants_are_preserved_verbatim() {
        assert_eq!(FARADAY_CONSTANT_C_PER_MOL, 96485.3365);
        assert_eq!(GRAVITY_M_PER_S2, 9.8);
    }

    /// Methodology: `WeatherSource` must default to `Global`, matching
    /// DWSIM's `UseUserDefinedWeather As Boolean = False`
    /// (`CleanEnergyUnitOpBase.vb:12`).
    /// Result (2026-08-11): `WeatherSource::default() == Global`.
    #[test]
    fn weather_source_defaults_to_global() {
        assert_eq!(WeatherSource::default(), WeatherSource::Global);
    }
}
