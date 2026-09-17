//! Flowsheet object taxonomy: the `ObjectType` enum, object identity
//! ([`ObjectId`]), the per-object registry entry ([`FlowsheetObject`]), and the
//! payload enum ([`ObjectData`]) that distinguishes a material stream from an
//! energy stream from a unit operation.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software.
//!
//! Source regions ported here:
//!
//! - `DWSIM.Interfaces/Enums.vb` lines 669-753 — the `ObjectType` enumeration
//!   (the flowsheet's closed object taxonomy).
//! - `DWSIM.Interfaces/Enums.vb` lines 495-517 — the `SimulationObjectClass`
//!   enumeration (the palette grouping each object type belongs to).
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` lines 521-727 — `AddObject(typename
//!   As String, ...)`, the human-readable display-name to `ObjectType` mapping,
//!   ported as [`ObjectType::from_display_name`] / [`ObjectType::display_name`].
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` lines 727-787 —
//!   `GetAvailableFlowsheetObjectTypeNames`, ported as
//!   [`available_object_type_names`].
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` lines 1118-1830 — the default
//!   tag-prefix table embedded in `AddObjectToSurface`'s giant `Select Case`,
//!   ported as [`ObjectType::default_tag_prefix`].
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` lines 236-274 — the
//!   spec/adjust/PID "attached logical block" bookkeeping fields, ported as the
//!   [`FlowsheetObject::attached_spec`] / [`FlowsheetObject::attached_adjust`]
//!   fields (the detach behaviour itself lives in
//!   [`crate::flowsheet::graph::Flowsheet::remove_object`]).
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported (no physics, or forbidden by the workspace
//! rules):
//!
//! - Everything graphical. DWSIM's object taxonomy mixes simulation objects
//!   with pure drawing annotations (`GO_Table`, `GO_Text`, `GO_Image`,
//!   `GO_Chart`, ...). The enum **variants are kept** so a DWSIM flowsheet can
//!   round-trip its object types, but no geometry (`X`, `Y`, `Width`,
//!   `Height`), no `Draw`, no `HitTest`, no `ShapeIcon`, and no SkiaSharp
//!   anything is ported (`DWSIM.Drawing.SkiaSharp/**`). [`ObjectType::is_drawing_only`]
//!   identifies those variants so callers can skip them.
//! - `AddObjectToSurface`'s object *construction* (FlowsheetBase.vb:992-1839):
//!   it instantiates .NET graphic-object classes and calls into
//!   `DWSIM.UnitOperations`/`DWSIM.Thermodynamics` constructors by reflection.
//!   Only the algorithmic parts — the type-indexed default tag prefix and the
//!   `objindex` counting rule — are ported (see
//!   [`crate::flowsheet::graph::Flowsheet::next_tag`]).
//! - `Initialize()` (FlowsheetBase.vb:3100-3221) — .NET assembly reflection
//!   (`Assembly.Load`, `GetExportedTypes`, `Activator.CreateInstance`) plus
//!   compound-database loading from ChemSep/CoolProp/ChEDL XML. Neither is
//!   portable, and the compound databases are a `thermo`-module concern.
//! - `GetPropertyValue` / `GetProperties` / `SetPropertyValue` /
//!   `GetPropertyUnit` property-grid reflection (MaterialStream.vb:1341-2717) —
//!   excluded by the porting brief.
//! - `SaveData` / `LoadData` / `CloneXML` XML serialization.

use std::collections::HashMap;

use crate::flowsheet::connectors::{ConType, ConnectionPoint, ConnectorLayout};
use crate::flowsheet::streams::{EnergyStreamData, MaterialStreamData};

/// Stable identity of a flowsheet object — DWSIM's `ISimulationObject.Name`,
/// which upstream is a GUID string and is the key of the `SimulationObjects`
/// dictionary (FlowsheetBase.vb:513).
///
/// This is **not** the user-visible label; that is the *tag* (see
/// [`FlowsheetObject::tag`]). The distinction matters: DWSIM lets a user rename
/// a tag freely while every internal reference keeps pointing at the immutable
/// name/ID.
///
/// Modelled as an owned `String` newtype rather than a reference or index, per
/// the workspace rules (no lifetimes, no `Box`, no `dyn`). Dimensionless — an
/// identifier, not a physical quantity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub String);

impl ObjectId {
    /// Borrow the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ObjectId {
    fn from(s: &str) -> Self {
        ObjectId(s.to_string())
    }
}

impl From<String> for ObjectId {
    fn from(s: String) -> Self {
        ObjectId(s)
    }
}

impl core::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Palette grouping of a flowsheet object — DWSIM's
/// `Interfaces.Enums.SimulationObjectClass` (Enums.vb:495-517).
///
/// Purely a classification for menus, reports, and solver filtering; it carries
/// no physics. Ported in full so a round-tripped DWSIM flowsheet keeps its
/// grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationObjectClass {
    /// Material and energy streams.
    Streams,
    /// Pumps, compressors, expanders, valves, pipes — anything that changes `P`.
    PressureChangers,
    /// Flash vessels, tanks, component/solid separators, filters.
    Separators,
    /// Mixers and splitters.
    MixersSplitters,
    /// Heaters, coolers, heat exchangers, air coolers.
    Exchangers,
    /// Conversion / equilibrium / Gibbs / CSTR / PFR reactors.
    Reactors,
    /// Shortcut and rigorous distillation/absorption columns.
    Columns,
    /// Solids-handling unit operations.
    Solids,
    /// CAPE-OPEN external unit operations.
    CapeOpen,
    /// User-scripted or spreadsheet-backed unit operations.
    UserModels,
    /// Logical blocks: adjust (controller), spec, recycle, energy recycle.
    Logical,
    /// Anything not otherwise classified.
    Other,
    /// Analog / digital / level gauges (display only).
    Indicators,
    /// PID and Python controllers (dynamic mode).
    Controllers,
    /// Switch blocks (dynamic mode).
    Switches,
    /// Input boxes (dynamic mode).
    Inputs,
    /// Explicitly unclassified — DWSIM's `None`.
    None,
    /// Wind / hydro / solar power sources.
    CleanPowerSources,
    /// Water electrolyzers and fuel cells.
    Electrolyzers,
}

/// The closed set of flowsheet object types — DWSIM's
/// `Interfaces.Enums.GraphicObjects.ObjectType` (Enums.vb:669-753), ported
/// variant-for-variant.
///
/// Modelled as an enum (never a trait object) per the workspace design rules:
/// the taxonomy is closed and known at compile time, so adding a variant forces
/// every `match` to be revisited.
///
/// Variant names follow the upstream identifiers so a reader can grep the DWSIM
/// source directly; where the upstream name is opaque the doc comment gives the
/// plain-English meaning. Two variants are renamed for legibility and are
/// flagged in their doc comments: `Nenhum` -> [`ObjectType::Undefined`], and the
/// `OT_*`/`RCT_*`/`GO_*` prefixes are spelled in Rust `CamelCase`
/// (`OT_Adjust` -> `OtAdjust`, `RCT_PFR` -> `RctPfr`, `GO_Table` -> `GoTable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    /// Stream **mixer** (upstream `NodeIn`): N material inlets, one outlet.
    NodeIn,
    /// Stream **splitter** (upstream `NodeOut`): one material inlet, N outlets.
    NodeOut,
    /// Energy-stream node (upstream `NodeEn`).
    NodeEn,
    /// Centrifugal pump.
    Pump,
    /// Liquid holdup tank.
    Tank,
    /// Gas-liquid separator vessel (flash drum).
    Vessel,
    /// Material stream — the T/P/composition/flow carrier between unit ops.
    MaterialStream,
    /// Energy stream — a scalar power (duty/work) link, kW.
    EnergyStream,
    /// Compressor.
    Compressor,
    /// Expander (turbine).
    Expander,
    /// Temperature/pressure vessel (upstream `TPVessel`).
    TpVessel,
    /// Cooler (heat removed).
    Cooler,
    /// Heater (heat added).
    Heater,
    /// Pipe segment (hydraulics + heat transfer).
    Pipe,
    /// Valve (isenthalpic pressure drop).
    Valve,
    /// Undefined / no object. Upstream spells this `Nenhum` (Portuguese for
    /// "none"); renamed here for legibility.
    Undefined,
    /// Drawing-only: property table annotation (upstream `GO_Table`).
    GoTable,
    /// Drawing-only: free text annotation (upstream `GO_Text`).
    GoText,
    /// Drawing-only: embedded image (upstream `GO_Image`).
    GoImage,
    /// Drawing-only: floating property table (upstream `GO_FloatingTable`).
    GoFloatingTable,
    /// Logical block: **adjust** / controller block (upstream `OT_Adjust`).
    OtAdjust,
    /// Logical block: **specification** block (upstream `OT_Spec`).
    OtSpec,
    /// Logical block: material **recycle** / tear block (upstream `OT_Recycle`).
    OtRecycle,
    /// Conversion reactor (upstream `RCT_Conversion`).
    RctConversion,
    /// Equilibrium reactor (upstream `RCT_Equilibrium`).
    RctEquilibrium,
    /// Gibbs-minimisation reactor (upstream `RCT_Gibbs`).
    RctGibbs,
    /// Continuous stirred-tank reactor (upstream `RCT_CSTR`).
    RctCstr,
    /// Plug-flow reactor (upstream `RCT_PFR`).
    RctPfr,
    /// Two-stream heat exchanger.
    HeatExchanger,
    /// Shortcut (Fenske-Underwood-Gilliland) distillation column.
    ShortcutColumn,
    /// Rigorous MESH distillation column.
    DistillationColumn,
    /// Rigorous absorption / extraction column.
    AbsorptionColumn,
    /// Refluxed absorber (rigorous column variant).
    RefluxedAbsorber,
    /// Reboiled absorber (rigorous column variant).
    ReboiledAbsorber,
    /// Logical block: **energy recycle** / tear block (upstream
    /// `OT_EnergyRecycle`).
    OtEnergyRecycle,
    /// Drawing-only: animation annotation (upstream `GO_Animation`).
    GoAnimation,
    /// Component (compound) separator with per-compound split fractions.
    ComponentSeparator,
    /// Orifice plate (pressure drop element).
    OrificePlate,
    /// User-scripted unit operation (upstream `CustomUO`).
    CustomUo,
    /// Spreadsheet-backed unit operation (upstream `ExcelUO`).
    ExcelUo,
    /// CAPE-OPEN external unit operation (upstream `CapeOpenUO`).
    CapeOpenUo,
    /// Nested sub-flowsheet as a unit operation (upstream `FlowsheetUO`).
    FlowsheetUo,
    /// Drawing-only: master property table (upstream `GO_MasterTable`).
    GoMasterTable,
    /// Solids separator.
    SolidSeparator,
    /// Cake filter.
    Filter,
    /// Drawing-only: spreadsheet-linked table (upstream `GO_SpreadsheetTable`).
    GoSpreadsheetTable,
    /// Drawing-only: rectangle annotation (upstream `GO_Rectangle`).
    GoRectangle,
    /// Combined compressor/expander block.
    CompressorExpander,
    /// Combined heater/cooler block.
    HeaterCooler,
    /// Drawing-only: chart annotation (upstream `GO_Chart`).
    GoChart,
    /// Drawing-only: input control annotation (upstream `GO_InputControl`).
    GoInputControl,
    /// Externally registered (plug-in) unit operation.
    External,
    /// Analog gauge indicator (display only).
    AnalogGauge,
    /// Digital gauge indicator (display only).
    DigitalGauge,
    /// Level gauge indicator (display only).
    LevelGauge,
    /// PID controller (dynamic mode) (upstream `Controller_PID`).
    ControllerPid,
    /// Switch block (dynamic mode).
    Switch,
    /// Input box (dynamic mode).
    Input,
    /// Drawing-only: HTML text annotation (upstream `GO_HTMLText`).
    GoHtmlText,
    /// Drawing-only: button annotation (upstream `GO_Button`).
    GoButton,
    /// Air cooler, revision 2.
    AirCooler2,
    /// Wind turbine (clean power source).
    WindTurbine,
    /// Hydroelectric turbine (clean power source).
    HydroelectricTurbine,
    /// Solar panel (clean power source).
    SolarPanel,
    /// PEM fuel cell (Amphlett model).
    PemFuelCell,
    /// Water electrolyzer.
    WaterElectrolyzer,
    /// Gibbs reactor backed by Reaktoro (upstream `RCT_GibbsReaktoro`).
    RctGibbsReaktoro,
    /// Energy-stream mixer.
    EnergyMixer,
    /// Newer stream mixer variant (coexists with [`ObjectType::NodeIn`]).
    Mixer,
    /// Newer stream splitter variant (coexists with [`ObjectType::NodeOut`]).
    Splitter,
    /// Python-scripted controller (upstream `Controller_Python`).
    ControllerPython,
    /// Placeholder / dummy object.
    Dummy,
    /// Generic solids-handling operation (upstream `SolidOps`).
    SolidOps,
}

impl ObjectType {
    /// `true` for [`ObjectType::MaterialStream`].
    ///
    /// Used throughout the connection rules, which treat material streams
    /// specially (DesignSurface.vb:1288-1320).
    #[must_use]
    pub fn is_material_stream(self) -> bool {
        matches!(self, ObjectType::MaterialStream)
    }

    /// `true` for [`ObjectType::EnergyStream`].
    ///
    /// Mirrors DWSIM's `IGraphicObject.IsEnergyStream` flag, which the
    /// connection logic branches on (DesignSurface.vb:1321, :1386).
    #[must_use]
    pub fn is_energy_stream(self) -> bool {
        matches!(self, ObjectType::EnergyStream)
    }

    /// `true` for either stream kind — i.e. this object carries state between
    /// unit operations rather than performing a calculation on it.
    #[must_use]
    pub fn is_stream(self) -> bool {
        self.is_material_stream() || self.is_energy_stream()
    }

    /// `true` for the pure drawing annotations (`GO_*`) and
    /// [`ObjectType::Undefined`].
    ///
    /// DWSIM refuses to connect these at all (DesignSurface.vb:1272-1277) and
    /// its delete path routes them straight to the drawing surface without
    /// touching `SimulationObjects` (FlowsheetBase.vb:202-211). No geometry or
    /// rendering is ported; the classification is kept so callers can filter.
    #[must_use]
    pub fn is_drawing_only(self) -> bool {
        matches!(
            self,
            ObjectType::GoTable
                | ObjectType::GoText
                | ObjectType::GoImage
                | ObjectType::GoFloatingTable
                | ObjectType::GoAnimation
                | ObjectType::GoMasterTable
                | ObjectType::GoSpreadsheetTable
                | ObjectType::GoRectangle
                | ObjectType::GoChart
                | ObjectType::GoInputControl
                | ObjectType::GoHtmlText
                | ObjectType::GoButton
                | ObjectType::Undefined
        )
    }

    /// `true` for the logical (non-physical) blocks: adjust, spec, recycle and
    /// energy recycle.
    ///
    /// These do not perform a mass/energy balance of their own; the solver
    /// treats them as constraints and tear points.
    #[must_use]
    pub fn is_logical_block(self) -> bool {
        matches!(
            self,
            ObjectType::OtAdjust
                | ObjectType::OtSpec
                | ObjectType::OtRecycle
                | ObjectType::OtEnergyRecycle
        )
    }

    /// `true` if this object is a *unit operation* — neither a stream, nor a
    /// drawing annotation, nor an indicator/controller/input block.
    ///
    /// This is the population DWSIM's `UpdateMassAndEnergyBalance` sums duties
    /// over (FlowsheetBase.vb:5412-5421, `TypeOf o Is UnitOpBaseClass And TypeOf
    /// o IsNot IIndicator`).
    #[must_use]
    pub fn is_unit_operation(self) -> bool {
        !self.is_stream()
            && !self.is_drawing_only()
            && !matches!(
                self,
                ObjectType::AnalogGauge
                    | ObjectType::DigitalGauge
                    | ObjectType::LevelGauge
                    | ObjectType::ControllerPid
                    | ObjectType::ControllerPython
                    | ObjectType::Switch
                    | ObjectType::Input
                    | ObjectType::Dummy
            )
    }

    /// Palette grouping this object type belongs to (Enums.vb:495-517).
    ///
    /// DWSIM assigns `ObjectClass` per concrete class rather than in one table;
    /// this function collects those assignments into a single mapping. Types
    /// with no upstream assignment return [`SimulationObjectClass::Other`].
    #[must_use]
    pub fn simulation_object_class(self) -> SimulationObjectClass {
        use ObjectType as T;
        use SimulationObjectClass as C;
        match self {
            T::MaterialStream | T::EnergyStream => C::Streams,
            T::Pump
            | T::Compressor
            | T::Expander
            | T::CompressorExpander
            | T::Valve
            | T::Pipe
            | T::OrificePlate => C::PressureChangers,
            T::Vessel | T::TpVessel | T::Tank | T::ComponentSeparator | T::Filter => C::Separators,
            T::NodeIn | T::NodeOut | T::Mixer | T::Splitter | T::EnergyMixer | T::NodeEn => {
                C::MixersSplitters
            }
            T::Heater | T::Cooler | T::HeaterCooler | T::HeatExchanger | T::AirCooler2 => {
                C::Exchangers
            }
            T::RctConversion
            | T::RctEquilibrium
            | T::RctGibbs
            | T::RctGibbsReaktoro
            | T::RctCstr
            | T::RctPfr => C::Reactors,
            T::ShortcutColumn
            | T::DistillationColumn
            | T::AbsorptionColumn
            | T::RefluxedAbsorber
            | T::ReboiledAbsorber => C::Columns,
            T::SolidSeparator | T::SolidOps => C::Solids,
            T::CapeOpenUo => C::CapeOpen,
            T::CustomUo | T::ExcelUo | T::FlowsheetUo | T::External => C::UserModels,
            T::OtAdjust | T::OtSpec | T::OtRecycle | T::OtEnergyRecycle => C::Logical,
            T::AnalogGauge | T::DigitalGauge | T::LevelGauge => C::Indicators,
            T::ControllerPid | T::ControllerPython => C::Controllers,
            T::Switch => C::Switches,
            T::Input => C::Inputs,
            T::WindTurbine | T::HydroelectricTurbine | T::SolarPanel => C::CleanPowerSources,
            T::WaterElectrolyzer | T::PemFuelCell => C::Electrolyzers,
            T::Undefined | T::Dummy => C::None,
            _ => C::Other,
        }
    }

    /// Default tag prefix DWSIM stamps on a newly created object of this type,
    /// e.g. `"PUMP-"` giving tags `PUMP-1`, `PUMP-2`, ...
    ///
    /// Collected from the per-type `Tag = "..." + objindex` assignments spread
    /// through `AddObjectToSurface` (FlowsheetBase.vb:1118-1830). Two upstream
    /// quirks are reproduced faithfully:
    ///
    /// - a **material stream** gets a bare index with no prefix (`""`, giving
    ///   `1`, `2`, ...; FlowsheetBase.vb:1283);
    /// - an **energy stream** uses `"E"` with no hyphen (`E1`, `E2`;
    ///   FlowsheetBase.vb:1302).
    ///
    /// Types DWSIM does not create through `AddObjectToSurface` (drawing
    /// annotations and the newer variants) return `"OBJ-"`.
    #[must_use]
    pub fn default_tag_prefix(self) -> &'static str {
        use ObjectType as T;
        match self {
            T::MaterialStream => "",
            T::EnergyStream => "E",
            T::OtAdjust => "C-",
            T::OtSpec => "SP-",
            T::OtRecycle => "R-",
            T::OtEnergyRecycle => "ER-",
            T::NodeIn | T::Mixer => "MIX-",
            T::NodeOut | T::Splitter => "SPL-",
            T::Pump => "PUMP-",
            T::Tank => "TANK-",
            T::Vessel => "V-",
            T::Compressor => "C-",
            T::Expander => "X-",
            T::Cooler => "CL-",
            T::Heater => "HT-",
            T::Pipe => "PIPE-",
            T::Valve => "VALVE-",
            T::RctConversion => "RCONV-",
            T::RctEquilibrium => "REQ-",
            T::RctGibbs => "RGIBBS-",
            T::RctGibbsReaktoro => "RGIBBSR-",
            T::RctCstr => "CSTR-",
            T::RctPfr => "PFR-",
            T::HeatExchanger => "HX-",
            T::ShortcutColumn => "SCOL-",
            T::DistillationColumn => "DCOL-",
            T::AbsorptionColumn | T::RefluxedAbsorber | T::ReboiledAbsorber => "ABS-",
            T::ComponentSeparator => "CS-",
            T::SolidSeparator => "SS-",
            T::Filter => "FLT-",
            T::OrificePlate => "OP-",
            T::CustomUo => "CUSTOM-",
            T::ExcelUo => "SHEET-",
            T::FlowsheetUo => "FS-",
            T::CapeOpenUo => "CO-",
            T::WindTurbine => "WT-",
            T::HydroelectricTurbine => "HYT-",
            T::PemFuelCell => "PEMFC-",
            T::SolarPanel => "SP-",
            T::WaterElectrolyzer => "WELEC-",
            T::Switch => "SW-",
            T::Input => "IN-",
            T::ControllerPid => "PID-",
            T::ControllerPython => "PC-",
            T::AnalogGauge => "AG-",
            T::DigitalGauge => "DG-",
            T::LevelGauge => "LG-",
            _ => "OBJ-",
        }
    }

    /// Human-readable palette name for this type, or `None` for types DWSIM does
    /// not expose in the "add object" menu.
    ///
    /// Inverse of [`ObjectType::from_display_name`]; both come from
    /// `AddObject(typename As String, ...)` (FlowsheetBase.vb:521-727). Where
    /// upstream accepts two names for one type (`"Absorption Column"` and
    /// `"Absorption/Extraction Column"`) this returns the first.
    #[must_use]
    pub fn display_name(self) -> Option<&'static str> {
        use ObjectType as T;
        Some(match self {
            T::OtAdjust => "Controller Block",
            T::OtSpec => "Specification Block",
            T::OtRecycle => "Recycle Block",
            T::OtEnergyRecycle => "Energy Recycle Block",
            T::NodeIn => "Stream Mixer",
            T::NodeOut => "Stream Splitter",
            T::Pump => "Pump",
            T::Tank => "Tank",
            T::Vessel => "Gas-Liquid Separator",
            T::MaterialStream => "Material Stream",
            T::EnergyStream => "Energy Stream",
            T::Compressor => "Compressor",
            T::Expander => "Expander (Turbine)",
            T::Heater => "Heater",
            T::Cooler => "Cooler",
            T::Pipe => "Pipe Segment",
            T::Valve => "Valve",
            T::RctConversion => "Conversion Reactor",
            T::RctEquilibrium => "Equilibrium Reactor",
            T::RctGibbs => "Gibbs Reactor",
            T::RctPfr => "Plug-Flow Reactor (PFR)",
            T::RctCstr => "Continuous Stirred Tank Reactor (CSTR)",
            T::HeatExchanger => "Heat Exchanger",
            T::ShortcutColumn => "Shortcut Column",
            T::DistillationColumn => "Distillation Column",
            T::AbsorptionColumn => "Absorption Column",
            T::ComponentSeparator => "Compound Separator",
            T::SolidSeparator => "Solids Separator",
            T::Filter => "Filter",
            T::OrificePlate => "Orifice Plate",
            T::CustomUo => "Python Script",
            T::ExcelUo => "Spreadsheet",
            T::FlowsheetUo => "Flowsheet",
            T::CapeOpenUo => "CAPE-OPEN Unit Operation",
            T::DigitalGauge => "Digital Gauge",
            T::AnalogGauge => "Analog Gauge",
            T::LevelGauge => "Level Gauge",
            T::ControllerPid => "PID Controller",
            T::ControllerPython => "Python Controller",
            T::Input => "Input Box",
            T::Switch => "Switch",
            T::AirCooler2 => "Air Cooler 2",
            T::RctGibbsReaktoro => "Gibbs Reactor (Reaktoro)",
            T::WindTurbine => "Wind Turbine",
            T::HydroelectricTurbine => "Hydroelectric Turbine",
            T::SolarPanel => "Solar Panel",
            T::WaterElectrolyzer => "Water Electrolyzer",
            T::PemFuelCell => "PEM Fuel Cell (Amphlett)",
            _ => return None,
        })
    }

    /// Resolve a palette display name to its [`ObjectType`], or `None` if the
    /// name is unknown.
    ///
    /// Direct port of `AddObject(typename As String, ...)`'s `Select Case`
    /// (FlowsheetBase.vb:521-727), including the two accepted spellings of the
    /// absorption column. Matching is exact and case-sensitive, as upstream.
    #[must_use]
    pub fn from_display_name(name: &str) -> Option<ObjectType> {
        use ObjectType as T;
        Some(match name {
            "Controller Block" => T::OtAdjust,
            "Specification Block" => T::OtSpec,
            "Recycle Block" => T::OtRecycle,
            "Energy Recycle Block" => T::OtEnergyRecycle,
            "Stream Mixer" => T::NodeIn,
            "Stream Splitter" => T::NodeOut,
            "Pump" => T::Pump,
            "Tank" => T::Tank,
            "Gas-Liquid Separator" => T::Vessel,
            "Material Stream" => T::MaterialStream,
            "Energy Stream" => T::EnergyStream,
            "Compressor" => T::Compressor,
            "Expander (Turbine)" => T::Expander,
            "Heater" => T::Heater,
            "Cooler" => T::Cooler,
            "Pipe Segment" => T::Pipe,
            "Valve" => T::Valve,
            "Conversion Reactor" => T::RctConversion,
            "Equilibrium Reactor" => T::RctEquilibrium,
            "Gibbs Reactor" => T::RctGibbs,
            "Plug-Flow Reactor (PFR)" => T::RctPfr,
            "Continuous Stirred Tank Reactor (CSTR)" => T::RctCstr,
            "Heat Exchanger" => T::HeatExchanger,
            "Shortcut Column" => T::ShortcutColumn,
            "Distillation Column" => T::DistillationColumn,
            "Absorption Column" | "Absorption/Extraction Column" => T::AbsorptionColumn,
            "Compound Separator" => T::ComponentSeparator,
            "Solids Separator" => T::SolidSeparator,
            "Filter" => T::Filter,
            "Orifice Plate" => T::OrificePlate,
            "Python Script" => T::CustomUo,
            "Spreadsheet" => T::ExcelUo,
            "Flowsheet" => T::FlowsheetUo,
            "CAPE-OPEN Unit Operation" => T::CapeOpenUo,
            "Digital Gauge" => T::DigitalGauge,
            "Analog Gauge" => T::AnalogGauge,
            "Level Gauge" => T::LevelGauge,
            "PID Controller" => T::ControllerPid,
            "Python Controller" => T::ControllerPython,
            "Input Box" => T::Input,
            "Switch" => T::Switch,
            "Air Cooler 2" => T::AirCooler2,
            "Gibbs Reactor (Reaktoro)" => T::RctGibbsReaktoro,
            "Wind Turbine" => T::WindTurbine,
            "Hydroelectric Turbine" => T::HydroelectricTurbine,
            "Solar Panel" => T::SolarPanel,
            "Water Electrolyzer" => T::WaterElectrolyzer,
            "PEM Fuel Cell (Amphlett)" => T::PemFuelCell,
            _ => return None,
        })
    }
}

/// The sorted list of object-type display names a user may add to a flowsheet —
/// port of `GetAvailableFlowsheetObjectTypeNames` (FlowsheetBase.vb:727-787).
///
/// Includes both accepted spellings of the absorption column, and is sorted
/// ascending (upstream calls `list.Sort()`), so the result is deterministic.
/// Every returned name resolves through [`ObjectType::from_display_name`].
#[must_use]
pub fn available_object_type_names() -> Vec<&'static str> {
    let mut list = vec![
        "Controller Block",
        "Specification Block",
        "Recycle Block",
        "Energy Recycle Block",
        "Stream Mixer",
        "Stream Splitter",
        "Pump",
        "Tank",
        "Gas-Liquid Separator",
        "Material Stream",
        "Energy Stream",
        "Compressor",
        "Expander (Turbine)",
        "Heater",
        "Cooler",
        "Pipe Segment",
        "Valve",
        "Conversion Reactor",
        "Equilibrium Reactor",
        "Gibbs Reactor",
        "Plug-Flow Reactor (PFR)",
        "Continuous Stirred Tank Reactor (CSTR)",
        "Heat Exchanger",
        "Shortcut Column",
        "Distillation Column",
        "Absorption Column",
        "Absorption/Extraction Column",
        "Compound Separator",
        "Solids Separator",
        "Filter",
        "Orifice Plate",
        "Python Script",
        "Spreadsheet",
        "Flowsheet",
        "CAPE-OPEN Unit Operation",
        "Digital Gauge",
        "Analog Gauge",
        "Level Gauge",
        "PID Controller",
        "Python Controller",
        "Input Box",
        "Switch",
        "Air Cooler 2",
        "Gibbs Reactor (Reaktoro)",
        "Wind Turbine",
        "Hydroelectric Turbine",
        "Solar Panel",
        "Water Electrolyzer",
        "PEM Fuel Cell (Amphlett)",
    ];
    list.sort_unstable();
    list
}

/// Per-object payload — enum dispatch over the three kinds of thing a flowsheet
/// registry entry can hold.
///
/// DWSIM stores heterogeneous `ISimulationObject` implementations in one
/// dictionary and downcasts (`DirectCast(baseobj, IMaterialStream)`). The
/// workspace forbids trait objects, so the closed set is an enum instead: a
/// missing arm is a compile error, not a runtime cast failure.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    /// A material stream's thermodynamic state (see
    /// [`crate::flowsheet::streams::MaterialStreamData`]).
    Material(MaterialStreamData),
    /// An energy stream's scalar power (see
    /// [`crate::flowsheet::streams::EnergyStreamData`]).
    Energy(EnergyStreamData),
    /// A unit operation, logical block, indicator, or drawing annotation. This
    /// port carries no equipment state here: the equipment models live in the
    /// crate's own modules ([`crate::pump`], [`crate::heat_exchanger`], ...)
    /// and are driven by the solver, which reads and writes the connected
    /// streams. Scalar results a report or the flowsheet-level energy balance
    /// needs are recorded in [`ObjectData::UnitOperation::power`].
    UnitOperation {
        /// Net power generated (`> 0`) or consumed (`< 0`) by this unit
        /// operation \[kW\], DWSIM's `GetPowerGeneratedOrConsumed()`
        /// (FlowsheetBase.vb:5437). `None` until the object has been
        /// calculated. kW, not W, to match DWSIM's internal energy-flow unit.
        power: Option<f64>,
        /// Free-form scalar results the solver or a report may attach, keyed by
        /// property name. Values are in SI base units; the key names the
        /// property. This replaces DWSIM's property-grid reflection, which is
        /// excluded from the port.
        results: HashMap<String, f64>,
    },
}

impl ObjectData {
    /// Borrow the material-stream payload, or `None` if this is not a material
    /// stream.
    #[must_use]
    pub fn as_material(&self) -> Option<&MaterialStreamData> {
        match self {
            ObjectData::Material(m) => Some(m),
            _ => None,
        }
    }

    /// Mutably borrow the material-stream payload, or `None`.
    pub fn as_material_mut(&mut self) -> Option<&mut MaterialStreamData> {
        match self {
            ObjectData::Material(m) => Some(m),
            _ => None,
        }
    }

    /// Borrow the energy-stream payload, or `None` if this is not an energy
    /// stream.
    #[must_use]
    pub fn as_energy(&self) -> Option<&EnergyStreamData> {
        match self {
            ObjectData::Energy(e) => Some(e),
            _ => None,
        }
    }

    /// Mutably borrow the energy-stream payload, or `None`.
    pub fn as_energy_mut(&mut self) -> Option<&mut EnergyStreamData> {
        match self {
            ObjectData::Energy(e) => Some(e),
            _ => None,
        }
    }
}

/// One entry in the flowsheet's simulation-object registry — the Rust stand-in
/// for DWSIM's paired `ISimulationObject` + `IGraphicObject`
/// (`SimulationObjects` / `GraphicObjects`, FlowsheetBase.vb:513, :374).
///
/// DWSIM keeps two parallel dictionaries because the drawing surface owns the
/// connectors. This port keeps **one** registry: the connectors live on the
/// object, since they are topology (a solver concern), not geometry (a GUI
/// concern). No coordinates, sizes, colours, or icons are ported.
///
/// All fields are owned by value; connections reference peers by [`ObjectId`],
/// never by reference — no lifetimes, no `Box`, no `Arc` needed at this level.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetObject {
    /// Immutable identity (DWSIM's `Name`, a GUID). Dictionary key.
    pub id: ObjectId,
    /// User-visible label (DWSIM's `GraphicObject.Tag`), e.g. `PUMP-1`. Unique
    /// within a flowsheet by construction — see
    /// [`crate::flowsheet::graph::Flowsheet::next_tag`].
    pub tag: String,
    /// Free-form description (DWSIM's `ComponentDescription` /
    /// `GraphicObject.Description`).
    pub description: String,
    /// What kind of object this is.
    pub object_type: ObjectType,
    /// Inlet connector slots, in index order. Index `i` is DWSIM's
    /// `InputConnectors(i)`; a connection request may name it explicitly
    /// (`tidx`).
    pub inputs: Vec<ConnectionPoint>,
    /// Outlet connector slots, in index order (DWSIM's `OutputConnectors(i)`,
    /// named by `fidx`).
    pub outputs: Vec<ConnectionPoint>,
    /// The single dedicated energy connector (DWSIM's `EnergyConnector`), used
    /// when a unit operation exports a duty to an energy stream.
    pub energy_connector: ConnectionPoint,
    /// Whether the energy connector participates in connections
    /// (`EnergyConnector.Active`). Objects that route their duty through a
    /// normal input/output slot set this `false`.
    pub energy_connector_active: bool,
    /// Whether the object's last calculation succeeded (DWSIM's `Calculated`).
    /// Cleared by [`crate::flowsheet::graph::Flowsheet::reset_calculation_status`].
    pub calculated: bool,
    /// Whether the object's inputs changed since its last successful
    /// calculation (DWSIM's `SetDirtyStatus` / `CheckDirtyStatus`). A dirty
    /// object must be recalculated.
    pub dirty: bool,
    /// Whether the object participates in the solution at all (DWSIM's
    /// `GraphicObject.Active`). An inactive object is skipped by the solver.
    pub active: bool,
    /// Last error message recorded for this object, if any (DWSIM's
    /// `ErrorMessage`).
    pub error_message: Option<String>,
    /// The specification block currently attached to this object, if any
    /// (DWSIM's `IsSpecAttached` + `AttachedSpecId`, FlowsheetBase.vb:236-245).
    pub attached_spec: Option<ObjectId>,
    /// The adjust/PID block currently attached to this object, if any (DWSIM's
    /// `IsAdjustAttached` + `AttachedAdjustId`, FlowsheetBase.vb:246-273).
    pub attached_adjust: Option<ObjectId>,
    /// Type-specific payload.
    pub data: ObjectData,
}

impl FlowsheetObject {
    /// Build an object of `object_type` with `id` and `tag`, its connector slots
    /// taken from the type's default layout
    /// ([`crate::flowsheet::connectors::default_layout`]) and its payload
    /// initialised for that type.
    ///
    /// A material stream is initialised with **no compounds** and DWSIM's
    /// documented defaults `T = 298.15 K`, `P = 101325 Pa`, `w = 1 kg/s`
    /// (MaterialStream.vb:381-383); add compounds afterwards with
    /// [`crate::flowsheet::streams::MaterialStreamData::add_compound`]. An
    /// energy stream starts with an undefined power. Everything else becomes an
    /// [`ObjectData::UnitOperation`] with no results.
    ///
    /// The object starts `active`, not `calculated`, and `dirty` (it has never
    /// been solved).
    #[must_use]
    pub fn new(id: ObjectId, tag: impl Into<String>, object_type: ObjectType) -> Self {
        let layout = ConnectorLayout::default_for(object_type);
        let data = match object_type {
            ObjectType::MaterialStream => ObjectData::Material(MaterialStreamData::new()),
            ObjectType::EnergyStream => ObjectData::Energy(EnergyStreamData::new()),
            _ => ObjectData::UnitOperation {
                power: None,
                results: HashMap::new(),
            },
        };
        FlowsheetObject {
            id,
            tag: tag.into(),
            description: String::new(),
            object_type,
            inputs: layout.inputs,
            outputs: layout.outputs,
            energy_connector: ConnectionPoint::new(ConType::Energy, "Energy Stream"),
            energy_connector_active: layout.energy_connector_active,
            calculated: false,
            dirty: true,
            active: true,
            error_message: None,
            attached_spec: None,
            attached_adjust: None,
            data,
        }
    }

    /// `true` if every inlet slot is attached to something.
    #[must_use]
    pub fn all_inputs_attached(&self) -> bool {
        self.inputs.iter().all(ConnectionPoint::is_attached)
    }

    /// `true` if no connector of any kind is attached — an isolated object.
    #[must_use]
    pub fn is_isolated(&self) -> bool {
        self.inputs.iter().all(|c| !c.is_attached())
            && self.outputs.iter().all(|c| !c.is_attached())
            && !self.energy_connector.is_attached()
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests — object taxonomy
    //!
    //! **Methodology.** These are *verification* checks (does the Rust table
    //! reproduce the DWSIM source table?), not validation against any physical
    //! benchmark — no physics is involved in a type taxonomy. Each test names
    //! the upstream source lines it re-checks. Results recorded 2026-08-11.

    use super::*;

    /// **Methodology.** Every name returned by
    /// [`available_object_type_names`] (FlowsheetBase.vb:727-787) must resolve
    /// through [`ObjectType::from_display_name`] (FlowsheetBase.vb:521-727) —
    /// upstream, an unresolvable palette entry would return `Nothing` and crash
    /// the caller with a null dereference.
    /// **Result (2026-08-11):** all 49 names resolve; list length 49, sorted.
    #[test]
    fn every_palette_name_resolves_to_a_type() {
        let names = available_object_type_names();
        assert_eq!(names.len(), 49, "palette list length");
        for n in &names {
            assert!(
                ObjectType::from_display_name(n).is_some(),
                "palette name `{n}` does not resolve to an ObjectType"
            );
        }
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "palette list must be sorted");
    }

    /// **Methodology.** [`ObjectType::display_name`] must be a left inverse of
    /// [`ObjectType::from_display_name`] for every type that has a display name.
    /// **Result (2026-08-11):** round-trips for all named types, including the
    /// two-spelling absorption column, which canonicalises to
    /// `"Absorption Column"`.
    #[test]
    fn display_name_round_trips() {
        for name in available_object_type_names() {
            let t = ObjectType::from_display_name(name).unwrap();
            let back = t.display_name().unwrap();
            assert_eq!(
                ObjectType::from_display_name(back),
                Some(t),
                "round trip failed for {name}"
            );
        }
        assert_eq!(
            ObjectType::from_display_name("Absorption/Extraction Column"),
            Some(ObjectType::AbsorptionColumn)
        );
        assert_eq!(
            ObjectType::AbsorptionColumn.display_name(),
            Some("Absorption Column")
        );
        assert_eq!(ObjectType::from_display_name("Not A Real Block"), None);
    }

    /// **Methodology.** Spot-check the tag-prefix table against the upstream
    /// `Tag = "..." + objindex` assignments (FlowsheetBase.vb:1118-1830),
    /// including the two documented quirks: a material stream has an empty
    /// prefix (:1283) and an energy stream uses `"E"` with no hyphen (:1302).
    /// **Result (2026-08-11):** all spot-checks match.
    #[test]
    fn tag_prefixes_match_upstream() {
        assert_eq!(ObjectType::MaterialStream.default_tag_prefix(), "");
        assert_eq!(ObjectType::EnergyStream.default_tag_prefix(), "E");
        assert_eq!(ObjectType::Pump.default_tag_prefix(), "PUMP-");
        assert_eq!(ObjectType::NodeIn.default_tag_prefix(), "MIX-");
        assert_eq!(ObjectType::Mixer.default_tag_prefix(), "MIX-");
        assert_eq!(ObjectType::NodeOut.default_tag_prefix(), "SPL-");
        assert_eq!(ObjectType::Compressor.default_tag_prefix(), "C-");
        assert_eq!(ObjectType::OtAdjust.default_tag_prefix(), "C-");
        assert_eq!(ObjectType::Expander.default_tag_prefix(), "X-");
        assert_eq!(ObjectType::RctPfr.default_tag_prefix(), "PFR-");
        assert_eq!(ObjectType::HeatExchanger.default_tag_prefix(), "HX-");
    }

    /// **Methodology.** Check the classification predicates used by the
    /// connection rules (DesignSurface.vb:1272-1320) and the flowsheet energy
    /// balance (FlowsheetBase.vb:5412-5421).
    /// **Result (2026-08-11):** streams classify as streams and not as unit
    /// operations; `GO_*` classify as drawing-only; gauges and controllers are
    /// excluded from the unit-operation population; a pump is a unit operation.
    #[test]
    fn classification_predicates() {
        assert!(ObjectType::MaterialStream.is_material_stream());
        assert!(ObjectType::MaterialStream.is_stream());
        assert!(!ObjectType::MaterialStream.is_unit_operation());
        assert!(ObjectType::EnergyStream.is_energy_stream());
        assert!(ObjectType::GoTable.is_drawing_only());
        assert!(ObjectType::Undefined.is_drawing_only());
        assert!(!ObjectType::Pump.is_drawing_only());
        assert!(ObjectType::Pump.is_unit_operation());
        assert!(!ObjectType::AnalogGauge.is_unit_operation());
        assert!(!ObjectType::ControllerPid.is_unit_operation());
        assert!(ObjectType::OtRecycle.is_logical_block());
        assert!(ObjectType::OtEnergyRecycle.is_logical_block());
        assert_eq!(
            ObjectType::Pump.simulation_object_class(),
            SimulationObjectClass::PressureChangers
        );
        assert_eq!(
            ObjectType::MaterialStream.simulation_object_class(),
            SimulationObjectClass::Streams
        );
        assert_eq!(
            ObjectType::DistillationColumn.simulation_object_class(),
            SimulationObjectClass::Columns
        );
    }

    /// **Methodology.** A freshly constructed [`FlowsheetObject`] must take its
    /// connector slots from the type's default layout and its payload from the
    /// type, and must start un-calculated, dirty, active, and isolated.
    /// **Result (2026-08-11):** a material stream gets 1 inlet + 1 outlet and a
    /// `Material` payload; a mixer (`NodeIn`) gets 6 inlets + 1 outlet and a
    /// `UnitOperation` payload with no power.
    #[test]
    fn new_object_takes_layout_and_payload_from_type() {
        let ms = FlowsheetObject::new(ObjectId::from("id-1"), "1", ObjectType::MaterialStream);
        assert_eq!(ms.inputs.len(), 1);
        assert_eq!(ms.outputs.len(), 1);
        assert!(ms.data.as_material().is_some());
        assert!(!ms.calculated);
        assert!(ms.dirty);
        assert!(ms.active);
        assert!(ms.is_isolated());

        let mix = FlowsheetObject::new(ObjectId::from("id-2"), "MIX-1", ObjectType::NodeIn);
        assert_eq!(mix.inputs.len(), 6);
        assert_eq!(mix.outputs.len(), 1);
        assert!(matches!(
            mix.data,
            ObjectData::UnitOperation { power: None, .. }
        ));
    }
}
