//! Name tables translating DWSIM's saved identifier strings into this crate's
//! enums.
//!
//! A DWSIM file names a thing three different ways, and this module has one
//! function per way:
//!
//! | Source in the file | Example | Function |
//! |---|---|---|
//! | `<SimulationObject><Type>` — the .NET class name | `DWSIM.UnitOperations.UnitOperations.Heater` | [`object_type_from_class_name`] |
//! | `<GraphicObject><ObjectType>` — the `ObjectType` enum member | `Heater` | [`object_type_from_enum_name`] |
//! | `<GraphicObject><TipoObjeto>` — the same enum, legacy element name | `MaterialStream` | [`object_type_from_enum_name`] |
//!
//! The class name is preferred by the importer because it is the more specific
//! of the two: DWSIM's clean-power sources all serialise their graphic object as
//! the generic `External`, while their class names distinguish
//! `WindTurbine` from `SolarPanel` from `HydroelectricTurbine`.
//!
//! # Format documentation
//!
//! The tables were built from **DWSIM** at commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`, GPL-3.0),
//! reading `DWSIM.Interfaces/Enums.vb` (`ObjectType`, :669-753; `StreamSpec`,
//! `FlowSpec`, `CompositionBasis`, `ForcedPhase`) and the class names emitted by
//! each unit operation's `SaveData`. They were then checked for completeness
//! against a census of every `<Type>`, `<ObjectType>`, `<SpecType>`,
//! `<DefinedFlow>` and `<ForcePhase>` string appearing in the 175 reference
//! flowsheets under `PlatformFiles/Common/{samples,tests}` at that commit, so
//! every string the reference corpus actually uses is covered. This is original
//! code documenting that format — not a port of DWSIM's loader.

use crate::flowsheet::objects::ObjectType;
use crate::flowsheet::streams::{CompositionBasis, FlowSpec, ForcedPhase, StreamSpec};
use crate::thermo::property_package::PropertyPackageModel;

/// Map a DWSIM `<SimulationObject><Type>` .NET class name onto an
/// [`ObjectType`], or `None` if this crate has no matching variant.
///
/// Only the final dotted segment is significant, so the modern
/// (`DWSIM.UnitOperations.UnitOperations.Pump`) and legacy
/// (`DWSIM.DWSIM.SimulationObjects.Streams.MaterialStream`) namespaces both
/// resolve.
///
/// Three class names map to a *differently named* variant, because DWSIM's
/// class and its `ObjectType` member disagree:
///
/// - `Mixer` -> [`ObjectType::NodeIn`] (the mixer's graphic object is `NodeIn`)
/// - `Splitter` -> [`ObjectType::NodeOut`]
/// - `Flowsheet` -> [`ObjectType::FlowsheetUo`] (the nested-flowsheet unit op)
#[must_use]
pub fn object_type_from_class_name(dwsim_type: &str) -> Option<ObjectType> {
    let leaf = dwsim_type.rsplit('.').next()?;
    use ObjectType as T;
    Some(match leaf {
        // Streams.
        "MaterialStream" => T::MaterialStream,
        "EnergyStream" => T::EnergyStream,
        // Mixing / splitting. DWSIM's class names and enum members differ here.
        "Mixer" => T::NodeIn,
        "Splitter" => T::NodeOut,
        "EnergyMixer" => T::EnergyMixer,
        // Pressure-change and heat-transfer equipment.
        "Pump" => T::Pump,
        "Compressor" => T::Compressor,
        "Expander" => T::Expander,
        "CompressorExpander" => T::CompressorExpander,
        "Valve" => T::Valve,
        "OrificePlate" => T::OrificePlate,
        "Pipe" => T::Pipe,
        "Heater" => T::Heater,
        "Cooler" => T::Cooler,
        "HeaterCooler" => T::HeaterCooler,
        "HeatExchanger" => T::HeatExchanger,
        "AirCooler2" => T::AirCooler2,
        // Vessels and separation.
        "Vessel" => T::Vessel,
        "TPVessel" => T::TpVessel,
        "Tank" => T::Tank,
        "ComponentSeparator" => T::ComponentSeparator,
        "SolidsSeparator" | "SolidSeparator" => T::SolidSeparator,
        "Filter" => T::Filter,
        "SolidOps" => T::SolidOps,
        // Columns.
        "ShortcutColumn" => T::ShortcutColumn,
        "DistillationColumn" => T::DistillationColumn,
        "AbsorptionColumn" => T::AbsorptionColumn,
        "RefluxedAbsorber" => T::RefluxedAbsorber,
        "ReboiledAbsorber" => T::ReboiledAbsorber,
        // Reactors.
        "Reactor_Conversion" => T::RctConversion,
        "Reactor_Equilibrium" => T::RctEquilibrium,
        "Reactor_Gibbs" => T::RctGibbs,
        "Reactor_ReaktoroGibbs" => T::RctGibbsReaktoro,
        "Reactor_CSTR" => T::RctCstr,
        "Reactor_PFR" => T::RctPfr,
        // Logical blocks.
        "Recycle" => T::OtRecycle,
        "EnergyRecycle" => T::OtEnergyRecycle,
        "Spec" => T::OtSpec,
        "Adjust" => T::OtAdjust,
        "PIDController" => T::ControllerPid,
        "PythonController" => T::ControllerPython,
        "Switch" => T::Switch,
        "Input" => T::Input,
        // Indicators.
        "AnalogGauge" => T::AnalogGauge,
        "DigitalGauge" => T::DigitalGauge,
        "LevelGauge" => T::LevelGauge,
        // Scripted / external / nested.
        "CustomUO" => T::CustomUo,
        "ExcelUO" => T::ExcelUo,
        "CapeOpenUO" => T::CapeOpenUo,
        "Flowsheet" | "FlowsheetUO" => T::FlowsheetUo,
        "External" | "ExternalUnitOperation" => T::External,
        // Clean-power sources (all serialise their graphic object as `External`).
        "WindTurbine" => T::WindTurbine,
        "HydroelectricTurbine" => T::HydroelectricTurbine,
        "SolarPanel" => T::SolarPanel,
        "PEMFC_Amphlett" | "PEMFuelCell" => T::PemFuelCell,
        "WaterElectrolyzer" => T::WaterElectrolyzer,
        "Dummy" => T::Dummy,
        _ => return None,
    })
}

/// Map a DWSIM `ObjectType` **enum member name** — as written into
/// `<GraphicObject><ObjectType>` (or, in pre-5.x files, `<TipoObjeto>`) — onto
/// an [`ObjectType`].
///
/// This is a name-for-name translation of `Enums.vb:669-753`; the only spelling
/// changes are the ones this crate's enum already documents (`Nenhum` ->
/// [`ObjectType::Undefined`], `OT_`/`RCT_`/`GO_` prefixes rendered in
/// `CamelCase`).
#[must_use]
pub fn object_type_from_enum_name(name: &str) -> Option<ObjectType> {
    use ObjectType as T;
    Some(match name {
        "NodeIn" => T::NodeIn,
        "NodeOut" => T::NodeOut,
        "NodeEn" => T::NodeEn,
        "Pump" => T::Pump,
        "Tank" => T::Tank,
        "Vessel" => T::Vessel,
        "MaterialStream" => T::MaterialStream,
        "EnergyStream" => T::EnergyStream,
        "Compressor" => T::Compressor,
        "Expander" => T::Expander,
        "TPVessel" => T::TpVessel,
        "Cooler" => T::Cooler,
        "Heater" => T::Heater,
        "Pipe" => T::Pipe,
        "Valve" => T::Valve,
        "Nenhum" => T::Undefined,
        "GO_Table" => T::GoTable,
        "GO_Text" => T::GoText,
        "GO_Image" => T::GoImage,
        "GO_FloatingTable" => T::GoFloatingTable,
        "OT_Adjust" => T::OtAdjust,
        "OT_Spec" => T::OtSpec,
        "OT_Recycle" => T::OtRecycle,
        "RCT_Conversion" => T::RctConversion,
        "RCT_Equilibrium" => T::RctEquilibrium,
        "RCT_Gibbs" => T::RctGibbs,
        "RCT_CSTR" => T::RctCstr,
        "RCT_PFR" => T::RctPfr,
        "HeatExchanger" => T::HeatExchanger,
        "ShortcutColumn" => T::ShortcutColumn,
        "DistillationColumn" => T::DistillationColumn,
        "AbsorptionColumn" => T::AbsorptionColumn,
        "RefluxedAbsorber" => T::RefluxedAbsorber,
        "ReboiledAbsorber" => T::ReboiledAbsorber,
        "OT_EnergyRecycle" => T::OtEnergyRecycle,
        "GO_Animation" => T::GoAnimation,
        "ComponentSeparator" => T::ComponentSeparator,
        "OrificePlate" => T::OrificePlate,
        "CustomUO" => T::CustomUo,
        "ExcelUO" => T::ExcelUo,
        "CapeOpenUO" => T::CapeOpenUo,
        "FlowsheetUO" => T::FlowsheetUo,
        "GO_MasterTable" => T::GoMasterTable,
        "SolidSeparator" => T::SolidSeparator,
        "Filter" => T::Filter,
        "GO_SpreadsheetTable" => T::GoSpreadsheetTable,
        "GO_Rectangle" => T::GoRectangle,
        "CompressorExpander" => T::CompressorExpander,
        "HeaterCooler" => T::HeaterCooler,
        "GO_Chart" => T::GoChart,
        "GO_InputControl" => T::GoInputControl,
        "External" => T::External,
        "AnalogGauge" => T::AnalogGauge,
        "DigitalGauge" => T::DigitalGauge,
        "LevelGauge" => T::LevelGauge,
        "Controller_PID" => T::ControllerPid,
        "Switch" => T::Switch,
        "Input" => T::Input,
        "GO_HTMLText" => T::GoHtmlText,
        "GO_Button" => T::GoButton,
        "AirCooler2" => T::AirCooler2,
        "WindTurbine" => T::WindTurbine,
        "HydroelectricTurbine" => T::HydroelectricTurbine,
        "SolarPanel" => T::SolarPanel,
        "PEMFuelCell" => T::PemFuelCell,
        "WaterElectrolyzer" => T::WaterElectrolyzer,
        "RCT_GibbsReaktoro" => T::RctGibbsReaktoro,
        "EnergyMixer" => T::EnergyMixer,
        "Mixer" => T::Mixer,
        "Splitter" => T::Splitter,
        "Controller_Python" => T::ControllerPython,
        "Dummy" => T::Dummy,
        "SolidOps" => T::SolidOps,
        _ => return None,
    })
}

/// Map a `<SpecType>` value onto a [`StreamSpec`] (`Enums.vb:382-392`).
///
/// The five spellings the reference corpus uses are
/// `Temperature_and_Pressure` (1525 streams), `Pressure_and_Enthalpy` (1029),
/// `Pressure_and_VaporFraction` (10), `Temperature_and_VaporFraction` (3) and
/// `Pressure_and_Entropy` (1); the remaining four are covered for completeness.
#[must_use]
pub fn stream_spec_from_name(name: &str) -> Option<StreamSpec> {
    Some(match name {
        "Temperature_and_Pressure" => StreamSpec::TemperatureAndPressure,
        "Pressure_and_Enthalpy" => StreamSpec::PressureAndEnthalpy,
        "Pressure_and_Entropy" => StreamSpec::PressureAndEntropy,
        "Pressure_and_VaporFraction" => StreamSpec::PressureAndVaporFraction,
        "Temperature_and_VaporFraction" => StreamSpec::TemperatureAndVaporFraction,
        "Pressure_and_SolidFraction" => StreamSpec::PressureAndSolidFraction,
        "Volume_and_Temperature" => StreamSpec::VolumeAndTemperature,
        "Volume_and_Enthalpy" => StreamSpec::VolumeAndEnthalpy,
        "Volume_and_Entropy" => StreamSpec::VolumeAndEntropy,
        _ => return None,
    })
}

/// Map a `<DefinedFlow>` value onto a [`FlowSpec`] (`Enums.vb:394-398`).
#[must_use]
pub fn flow_spec_from_name(name: &str) -> Option<FlowSpec> {
    Some(match name {
        "Mass" => FlowSpec::Mass,
        "Mole" => FlowSpec::Mole,
        "Volumetric" => FlowSpec::Volumetric,
        _ => return None,
    })
}

/// Map a `<ForcePhase>` value onto a [`ForcedPhase`] (`Enums.vb:428-434`).
#[must_use]
pub fn forced_phase_from_name(name: &str) -> Option<ForcedPhase> {
    Some(match name {
        "None" => ForcedPhase::None,
        "Vapor" => ForcedPhase::Vapor,
        "Liquid" => ForcedPhase::Liquid,
        "Solid" => ForcedPhase::Solid,
        "GlobalDef" => ForcedPhase::GlobalDef,
        _ => return None,
    })
}

/// Map a `<CompositionBasis>` value onto a [`CompositionBasis`]
/// (`Enums.vb:400-408`).
///
/// The flowsheet data model stores no composition basis (it always keeps mole
/// fractions on the mixture slot), so the importer uses this only to decide
/// whether the field was understood — an unrecognised value becomes an
/// [`crate::flowsheet::import::ImportGap::UnparsedField`].
#[must_use]
pub fn composition_basis_from_name(name: &str) -> Option<CompositionBasis> {
    Some(match name {
        "Molar_Fractions" => CompositionBasis::MolarFractions,
        "Mass_Fractions" => CompositionBasis::MassFractions,
        "Volumetric_Fractions" => CompositionBasis::VolumetricFractions,
        "Molar_Flows" => CompositionBasis::MolarFlows,
        "Mass_Flows" => CompositionBasis::MassFlows,
        "Volumetric_Flows" => CompositionBasis::VolumetricFlows,
        "DefaultBasis" => CompositionBasis::DefaultBasis,
        _ => return None,
    })
}

/// Map a `<PropertyPackage><Type>` class name onto this crate's
/// [`PropertyPackageModel`], or `None` when the package is not implemented here.
///
/// This crate implements three of DWSIM's twenty-odd packages
/// ([`PropertyPackageModel`] has exactly the variants `Ideal`, `PengRobinson`,
/// `Srk`), so `None` is the common answer and is reported as an
/// [`crate::flowsheet::import::ImportGap::UnsupportedPropertyPackage`]. A
/// missing package does **not** stop the import: the flowsheet's topology and
/// stored stream states are read regardless, because nothing in the importer
/// evaluates thermodynamics.
///
/// `RaoultPropertyPackage` maps to [`PropertyPackageModel::Ideal`] because
/// DWSIM's Raoult package *is* its reference ideal-gas/ideal-solution package.
#[must_use]
pub fn property_package_from_class_name(dwsim_type: &str) -> Option<PropertyPackageModel> {
    let leaf = dwsim_type.rsplit('.').next()?;
    Some(match leaf {
        "RaoultPropertyPackage" => PropertyPackageModel::Ideal,
        "PengRobinsonPropertyPackage" => PropertyPackageModel::PengRobinson,
        "SRKPropertyPackage" => PropertyPackageModel::Srk,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    //! # Verification — the name tables against the reference corpus census
    //!
    //! **Methodology.** The tests below assert the exact strings counted in the
    //! 175 reference flowsheets shipped with DWSIM at commit
    //! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Every distinct `<Type>` in
    //! those files' `<SimulationObjects>` sections is listed and must resolve;
    //! that is the completeness criterion. Verification only — no physics.
    //! **Results (2026-08-11, release build):** all four tests pass; all 50
    //! distinct class names and all 42 distinct graphic enum names in the
    //! corpus resolve.

    use super::*;

    /// **Methodology.** The 50 distinct `<SimulationObject><Type>` strings found
    /// across the reference corpus, each of which must map.
    /// **Result (2026-08-11):** 50/50 resolve; no `None`.
    #[test]
    fn every_class_name_in_the_reference_corpus_resolves() {
        const CORPUS: [&str; 50] = [
            "DWSIM.Thermodynamics.Streams.MaterialStream",
            "DWSIM.UnitOperations.Streams.EnergyStream",
            "DWSIM.UnitOperations.UnitOperations.Mixer",
            "DWSIM.UnitOperations.UnitOperations.Cooler",
            "DWSIM.UnitOperations.UnitOperations.CapeOpenUO",
            "DWSIM.UnitOperations.SpecialOps.Recycle",
            "DWSIM.UnitOperations.UnitOperations.Heater",
            "DWSIM.UnitOperations.UnitOperations.Valve",
            "DWSIM.UnitOperations.UnitOperations.Vessel",
            "DWSIM.UnitOperations.UnitOperations.Compressor",
            "DWSIM.UnitOperations.UnitOperations.Pump",
            "DWSIM.UnitOperations.UnitOperations.HeatExchanger",
            "DWSIM.UnitOperations.UnitOperations.DistillationColumn",
            "DWSIM.UnitOperations.Reactors.Reactor_Conversion",
            "DWSIM.UnitOperations.UnitOperations.Splitter",
            "DWSIM.UnitOperations.UnitOperations.ComponentSeparator",
            "DWSIM.UnitOperations.Reactors.Reactor_PFR",
            "DWSIM.UnitOperations.UnitOperations.Expander",
            "DWSIM.UnitOperations.UnitOperations.ShortcutColumn",
            "DWSIM.UnitOperations.Reactors.Reactor_Equilibrium",
            "DWSIM.DWSIM.SimulationObjects.Streams.MaterialStream",
            "DWSIM.UnitOperations.SpecialOps.EnergyRecycle",
            "DWSIM.UnitOperations.UnitOperations.AbsorptionColumn",
            "DWSIM.UnitOperations.UnitOperations.CustomUO",
            "DWSIM.UnitOperations.SpecialOps.Spec",
            "DWSIM.UnitOperations.Reactors.Reactor_CSTR",
            "DWSIM.UnitOperations.UnitOperations.Tank",
            "DWSIM.UnitOperations.UnitOperations.Input",
            "DWSIM.DWSIM.SimulationObjects.Streams.EnergyStream",
            "DWSIM.UnitOperations.SpecialOps.Adjust",
            "DWSIM.UnitOperations.SpecialOps.PIDController",
            "DWSIM.UnitOperations.UnitOperations.AnalogGauge",
            "DWSIM.UnitOperations.UnitOperations.LevelGauge",
            "DWSIM.UnitOperations.Reactors.Reactor_Gibbs",
            "DWSIM.UnitOperations.UnitOperations.SolarPanel",
            "DWSIM.UnitOperations.UnitOperations.SolidsSeparator",
            "DWSIM.UnitOperations.SpecialOps.PythonController",
            "DWSIM.UnitOperations.UnitOperations.ExcelUO",
            "DWSIM.UnitOperations.UnitOperations.Flowsheet",
            "DWSIM.UnitOperations.Reactors.Reactor_ReaktoroGibbs",
            "DWSIM.UnitOperations.UnitOperations.HydroelectricTurbine",
            "DWSIM.UnitOperations.UnitOperations.Pipe",
            "DWSIM.UnitOperations.UnitOperations.PEMFC_Amphlett",
            "DWSIM.UnitOperations.UnitOperations.WaterElectrolyzer",
            "DWSIM.UnitOperations.UnitOperations.WindTurbine",
            "DWSIM.DWSIM.SimulationObjects.Reactors.Reactor_PFR",
            "DWSIM.DWSIM.SimulationObjects.Reactors.Reactor_Equilibrium",
            "DWSIM.DWSIM.SimulationObjects.Reactors.Reactor_Conversion",
            "DWSIM.DWSIM.SimulationObjects.Reactors.Reactor_CSTR",
            "DWSIM.DWSIM.SimulationObjects.Reactors.Reactor_Gibbs",
        ];
        for name in CORPUS {
            assert!(
                object_type_from_class_name(name).is_some(),
                "class name not mapped: {name}"
            );
        }
        assert_eq!(
            object_type_from_class_name("DWSIM.UnitOperations.UnitOperations.Mixer"),
            Some(ObjectType::NodeIn),
            "DWSIM's Mixer class serialises its graphic object as NodeIn"
        );
        assert_eq!(
            object_type_from_class_name("Made.Up.Thing"),
            None,
            "an unknown class must be reported, not guessed"
        );
    }

    /// **Methodology.** The 42 distinct `<GraphicObject><ObjectType>` strings in
    /// the corpus (the same enum is written to `<TipoObjeto>` in pre-5.x files)
    /// must all map.
    /// **Result (2026-08-11):** 42/42 resolve; `Mixer` maps to the newer
    /// [`ObjectType::Mixer`] variant here (unlike the class-name table).
    #[test]
    fn every_graphic_enum_name_in_the_reference_corpus_resolves() {
        const CORPUS: [&str; 42] = [
            "MaterialStream",
            "EnergyStream",
            "NodeIn",
            "Cooler",
            "CapeOpenUO",
            "GO_MasterTable",
            "OT_Recycle",
            "GO_Text",
            "Valve",
            "Heater",
            "Vessel",
            "Compressor",
            "Pump",
            "HeatExchanger",
            "DistillationColumn",
            "GO_Table",
            "RCT_Conversion",
            "NodeOut",
            "ComponentSeparator",
            "RCT_PFR",
            "Expander",
            "GO_Rectangle",
            "ShortcutColumn",
            "RCT_Equilibrium",
            "OT_EnergyRecycle",
            "AbsorptionColumn",
            "CustomUO",
            "RCT_CSTR",
            "GO_Chart",
            "Tank",
            "External",
            "Input",
            "AnalogGauge",
            "LevelGauge",
            "GO_Image",
            "RCT_Gibbs",
            "GO_SpreadsheetTable",
            "SolidSeparator",
            "OT_Adjust",
            "ExcelUO",
            "FlowsheetUO",
            "Pipe",
        ];
        for name in CORPUS {
            assert!(
                object_type_from_enum_name(name).is_some(),
                "enum name not mapped: {name}"
            );
        }
        assert_eq!(
            object_type_from_enum_name("Nenhum"),
            Some(ObjectType::Undefined)
        );
        assert_eq!(object_type_from_enum_name("Mixer"), Some(ObjectType::Mixer));
        assert_eq!(object_type_from_enum_name("NotAThing"), None);
    }

    /// **Methodology.** The five `<SpecType>` values, three `<DefinedFlow>`
    /// values and the one `<ForcePhase>` value observed in the corpus.
    /// **Result (2026-08-11):** all resolve to the expected variants.
    #[test]
    fn stream_enumeration_names_resolve() {
        assert_eq!(
            stream_spec_from_name("Temperature_and_Pressure"),
            Some(StreamSpec::TemperatureAndPressure)
        );
        assert_eq!(
            stream_spec_from_name("Pressure_and_Enthalpy"),
            Some(StreamSpec::PressureAndEnthalpy)
        );
        assert_eq!(
            stream_spec_from_name("Pressure_and_Entropy"),
            Some(StreamSpec::PressureAndEntropy)
        );
        assert_eq!(
            stream_spec_from_name("Pressure_and_VaporFraction"),
            Some(StreamSpec::PressureAndVaporFraction)
        );
        assert_eq!(
            stream_spec_from_name("Temperature_and_VaporFraction"),
            Some(StreamSpec::TemperatureAndVaporFraction)
        );
        assert_eq!(stream_spec_from_name("Nope"), None);

        assert_eq!(flow_spec_from_name("Mass"), Some(FlowSpec::Mass));
        assert_eq!(flow_spec_from_name("Mole"), Some(FlowSpec::Mole));
        assert_eq!(
            flow_spec_from_name("Volumetric"),
            Some(FlowSpec::Volumetric)
        );
        assert_eq!(
            forced_phase_from_name("GlobalDef"),
            Some(ForcedPhase::GlobalDef)
        );
        assert_eq!(
            composition_basis_from_name("Molar_Fractions"),
            Some(CompositionBasis::MolarFractions)
        );
    }

    /// **Methodology.** The three packages this crate implements must map; a
    /// sample of the packages it does not (NRTL, UNIQUAC, UNIFAC, Steam Tables,
    /// CoolProp, PRSV2, Lee-Kesler-Plocker, Chao-Seader, Grayson-Streed) must
    /// return `None` rather than being silently coerced onto a cubic.
    /// **Result (2026-08-11):** 3 map, 9 correctly return `None`.
    #[test]
    fn property_packages_map_only_where_implemented() {
        assert_eq!(
            property_package_from_class_name(
                "DWSIM.Thermodynamics.PropertyPackages.RaoultPropertyPackage"
            ),
            Some(PropertyPackageModel::Ideal)
        );
        assert_eq!(
            property_package_from_class_name(
                "DWSIM.Thermodynamics.PropertyPackages.PengRobinsonPropertyPackage"
            ),
            Some(PropertyPackageModel::PengRobinson)
        );
        assert_eq!(
            property_package_from_class_name(
                "DWSIM.Thermodynamics.PropertyPackages.SRKPropertyPackage"
            ),
            Some(PropertyPackageModel::Srk)
        );
        for unsupported in [
            "DWSIM.Thermodynamics.PropertyPackages.NRTLPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.UNIQUACPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.UNIFACPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.SteamTablesPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.CoolPropPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.PRSV2PropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.LKPPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.ChaoSeaderPropertyPackage",
            "DWSIM.Thermodynamics.PropertyPackages.GraysonStreedPropertyPackage",
        ] {
            assert_eq!(
                property_package_from_class_name(unsupported),
                None,
                "{unsupported} must not be silently coerced onto an implemented package"
            );
        }
    }
}
