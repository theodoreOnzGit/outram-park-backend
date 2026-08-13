//! Stream data model: the thermodynamic state a **material stream** carries
//! between unit operations, and the scalar power an **energy stream** carries.
//!
//! # What this represents physically
//!
//! A material stream is DWSIM's state carrier. It holds, for each of eight
//! *phase slots* (mixture, overall liquid, vapour, liquid 1/2/3, aqueous,
//! solid), a bag of intensive and extensive properties ([`PhaseProperties`])
//! plus a per-compound composition ([`StreamCompound`]). The `Mixture` slot
//! (index 0) holds the overall feed condition — the temperature, pressure,
//! enthalpy, entropy and total flows a user specifies; the other slots hold the
//! phase split a flash produces.
//!
//! A [`StreamSpec`] says *which two* of those overall properties are the
//! independent specification (T&P, P&H, P&S, P&vapour-fraction, ...), i.e.
//! which flash the solver must run to fill in the rest.
//!
//! **This module is the data model only.** No flash, no property package, no
//! enthalpy departure is computed here — those live in [`crate::thermo`]. The
//! flowsheet solver reads a stream's spec and overall state, calls the
//! appropriate [`crate::thermo`] routine, and writes the phase split back.
//!
//! # Units
//!
//! DWSIM's internal unit system is *SI-with-kilo* for energy: **enthalpy is
//! kJ/kg, entropy kJ/(kg·K), energy flow kW, molecular weight kg/kmol** —
//! confirmed at `MaterialStream.vb:8521-8582` (`GetMassEnthalpy` "Mass enthalpy
//! in kJ/kg") and `PropertyPackage.vb:1129` (`H = ... 'kJ/kg`). Temperature is
//! K, pressure Pa, mass flow kg/s, molar flow mol/s, volumetric flow m³/s.
//!
//! The raw `f64` fields below store **DWSIM's units verbatim**, so a value read
//! from or written to a DWSIM file needs no conversion and the ported
//! algorithms stay literally comparable to their source. The `uom`-typed
//! accessors ([`MaterialStreamData::temperature`],
//! [`MaterialStreamData::mass_enthalpy`], ...) convert on the way in and out, so
//! the *typed* public surface is plain SI (J/kg, W) with no kilo surprises.
//! Every field's doc comment spells out its unit.
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
//! - `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb` lines 39-148 —
//!   the `Compound` class, ported as [`StreamCompound`].
//! - `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb` lines 1258-1385 —
//!   the `PhaseProperties` class, ported field-for-field as [`PhaseProperties`].
//! - `DWSIM.Thermodynamics/MaterialStream/MaterialStream.vb` lines 368-465 —
//!   the eight-phase-slot construction and the documented defaults
//!   `T = 298.15 K`, `P = 101325 Pa`, `w = 1 kg/s`.
//! - `MaterialStream.vb` lines 1163-1340 — the composition algebra:
//!   `SetOverallComposition`, `SetOverallMolarComposition`,
//!   `SetOverallMassComposition`, `EqualizeOverallComposition`,
//!   `NormalizeOverallMoleComposition`, `NormalizeOverallMassComposition`,
//!   `CalcOverallCompMassFractions`, `CalcOverallCompMoleFractions`,
//!   `SetPhaseComposition`, `GetPhaseComposition`, `GetOverallComposition`,
//!   `GetOverallMassComposition`, `GetCompoundNames`.
//! - `MaterialStream.vb` lines 1078-1129 — `DeepClear` / `Clear`.
//! - `MaterialStream.vb` lines 217-256 — `CheckDirtyStatus`, ported together
//!   with its [`MaterialStreamInputData`] snapshot type.
//! - `MaterialStream.vb` lines 257-278 — `Validate`.
//! - `MaterialStream.vb` lines 8521-8600 — the `Get*`/`Set*` scalar accessors
//!   and their documented units.
//! - `DWSIM.Interfaces/Enums.vb` lines 382-434 — `StreamSpec`, `FlowSpec`,
//!   `CompositionBasis`, `PhaseLabel`, `PhaseName`, `ForcedPhase`.
//! - `DWSIM.UnitOperations/EnergyStream/Streams.vb` lines 36-120 — the
//!   `EnergyStream` data model (`EnergyFlow`, `SetValue(energyflow_kW)`).
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported:
//!
//! - **The whole property-calculator half of `MaterialStream.vb`** — `Calculate`
//!   (lines 520-993), which drives the property package through the nine flash
//!   specifications, and `ClearCalculatedProps` (lines 1131-1162), which calls
//!   `PropertyPackage.DW_ZerarPhaseProps`. Those are `crate::thermo`'s job;
//!   [`MaterialStreamData::clear`] here clears the *stored* state without
//!   invoking any package. This is the porting brief's explicit boundary: data
//!   model, not calculator.
//! - **CAPE-OPEN interfaces** (`ICapeThermoMaterialObject`,
//!   `ICapeThermoCalculationRoutine`, `ICapeIdentification`, lines 3374-4166)
//!   — .NET COM interop, not portable and not physics.
//! - **Property-grid reflection**: `GetPropertyValue` / `GetProperties` /
//!   `SetPropertyValue` / `GetPropertyUnit` (lines 1341-2717) and the
//!   `IUnitsOfMeasure` display-unit conversion layer they sit on. Excluded by
//!   the porting brief; the ported model is SI-internal only.
//! - **XML serialization**: `LoadData` / `SaveData` (lines 138-186) and
//!   `GetDebugReport` (line 187).
//! - **Dynamic-mode plumbing**: `RunDynamicModel` (line 490),
//!   `MaximumAllowableDynamicMassFlowRate`, and `TotalEnergyFlow` — these belong
//!   to the [`crate::dynamics`] workstream, not the flowsheet data model.
//! - **`AssignSelfToPP` / `SetPropertyPackage` / `GetPropertyPackageObject`**
//!   (lines 282-306, 994-1004): the stream-to-package back-pointer. This port
//!   keeps the stream package-free; the solver passes a
//!   [`crate::thermo::property_package::PropertyPackageModel`] explicitly.
//! - **Compound constant properties.** DWSIM's `Compound.ConstantProperties` is
//!   a ~200-field `ConstantProperties` record (critical constants, Cp
//!   correlations, database provenance, ...). Only the molar mass is carried
//!   here, because it is the only constant the ported composition algebra needs;
//!   the full record is [`crate::thermo::component::Component`]'s concern.
//! - **`EnergyStream`'s** CAPE-OPEN `RealParameter` collection
//!   (`Streams.vb:86-93`), `EditingForm_EnergyStream`, and its property-grid
//!   accessors.

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    AvailableEnergy, CatalyticActivity, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity,
    ThermodynamicTemperature, VolumeRate,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume_rate::cubic_meter_per_second;

/// Molar flow rate `n_dot` \[mol/s\].
///
/// Dimensionally this is amount-of-substance per time, which `uom` names
/// `CatalyticActivity` (the katal, mol/s) — an unhelpful name in a
/// process-simulation context, so it is aliased here. Use
/// [`uom::si::catalytic_activity::katal`] as its unit: 1 katal = 1 mol/s.
pub type MolarFlowRate = CatalyticActivity;

/// Which of the eight phase slots a property or composition belongs to.
///
/// DWSIM keys its `Phases` dictionary by these integers
/// (MaterialStream.vb:371-378, and the mapping in `SetPhaseComposition`,
/// MaterialStream.vb:1280-1310). The numbering is **not** contiguous by
/// physical meaning — in particular `OverallLiquid` is 1 and `Vapor` is 2 — so
/// the enum is the safe way to address a slot; [`PhaseIndex::index`] recovers
/// the upstream integer when reading a DWSIM file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseIndex {
    /// Slot 0 — the overall mixture: the stream's specified T, P, flows and
    /// overall composition.
    Mixture,
    /// Slot 1 — the combined liquid phase (all liquid sub-phases lumped).
    OverallLiquid,
    /// Slot 2 — the vapour phase.
    Vapor,
    /// Slot 3 — liquid phase 1 (the first/lighter liquid in an LLE split).
    Liquid1,
    /// Slot 4 — liquid phase 2 (the second/heavier liquid in an LLE split).
    Liquid2,
    /// Slot 5 — liquid phase 3.
    Liquid3,
    /// Slot 6 — the aqueous liquid phase (electrolyte packages).
    Aqueous,
    /// Slot 7 — the solid phase.
    Solid,
}

impl PhaseIndex {
    /// Every slot, in upstream index order. Length 8.
    pub const ALL: [PhaseIndex; 8] = [
        PhaseIndex::Mixture,
        PhaseIndex::OverallLiquid,
        PhaseIndex::Vapor,
        PhaseIndex::Liquid1,
        PhaseIndex::Liquid2,
        PhaseIndex::Liquid3,
        PhaseIndex::Aqueous,
        PhaseIndex::Solid,
    ];

    /// The upstream integer key of this slot (0-7), as used by DWSIM's
    /// `Phases(i)` dictionary.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            PhaseIndex::Mixture => 0,
            PhaseIndex::OverallLiquid => 1,
            PhaseIndex::Vapor => 2,
            PhaseIndex::Liquid1 => 3,
            PhaseIndex::Liquid2 => 4,
            PhaseIndex::Liquid3 => 5,
            PhaseIndex::Aqueous => 6,
            PhaseIndex::Solid => 7,
        }
    }

    /// Recover a slot from its upstream integer key, or `None` if out of range.
    #[must_use]
    pub fn from_index(i: usize) -> Option<PhaseIndex> {
        PhaseIndex::ALL.get(i).copied()
    }

    /// The upstream phase name string DWSIM constructs the slot with
    /// (MaterialStream.vb:371-378), e.g. `"OverallLiquid"`.
    #[must_use]
    pub fn upstream_name(self) -> &'static str {
        match self {
            PhaseIndex::Mixture => "Mixture",
            PhaseIndex::OverallLiquid => "OverallLiquid",
            PhaseIndex::Vapor => "Vapor",
            PhaseIndex::Liquid1 => "Liquid1",
            PhaseIndex::Liquid2 => "Liquid2",
            PhaseIndex::Liquid3 => "Liquid3",
            PhaseIndex::Aqueous => "Aqueous",
            PhaseIndex::Solid => "Solid",
        }
    }
}

/// Which pair of properties is the stream's independent specification — DWSIM's
/// `Interfaces.Enums.StreamSpec` (Enums.vb:382-392).
///
/// This selects the flash the solver must run to complete the stream state. All
/// nine upstream variants are ported; whether the crate's [`crate::thermo`]
/// kernel currently implements the corresponding flash is a separate question
/// (see that module's scope notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamSpec {
    /// `T` \[K\] and `P` \[Pa\] given — isothermal-isobaric (PT) flash. DWSIM's
    /// default for a new stream (MaterialStream.vb:6589).
    TemperatureAndPressure,
    /// `P` \[Pa\] and mass enthalpy `h` \[kJ/kg\] given — PH flash. This is what
    /// a mixer, heater, valve, or pump writes on its outlet.
    PressureAndEnthalpy,
    /// `P` \[Pa\] and mass entropy `s` \[kJ/(kg·K)\] given — PS flash
    /// (isentropic compression/expansion reference state).
    PressureAndEntropy,
    /// `P` \[Pa\] and vapour molar fraction `beta` \[-\] given — the dew/bubble
    /// family (`beta = 1` dew point, `beta = 0` bubble point).
    PressureAndVaporFraction,
    /// `T` \[K\] and vapour molar fraction `beta` \[-\] given.
    TemperatureAndVaporFraction,
    /// `P` \[Pa\] and solid fraction \[-\] given (SLE).
    PressureAndSolidFraction,
    /// Volume \[m³\] and `T` \[K\] given (TV flash).
    VolumeAndTemperature,
    /// Volume \[m³\] and mass enthalpy \[kJ/kg\] given.
    VolumeAndEnthalpy,
    /// Volume \[m³\] and mass entropy \[kJ/(kg·K)\] given.
    VolumeAndEntropy,
}

impl Default for StreamSpec {
    /// DWSIM's default for a newly created material stream is
    /// `Temperature_and_Pressure` (MaterialStream.vb:6589).
    fn default() -> Self {
        StreamSpec::TemperatureAndPressure
    }
}

/// Which flow quantity the user fixed on this stream — DWSIM's
/// `Interfaces.Enums.FlowSpec` (Enums.vb:394-398), exposed as
/// `MaterialStream.DefinedFlow` (MaterialStream.vb:80).
///
/// The other two flows are then derived from it through the mixture molecular
/// weight and density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowSpec {
    /// Mass flow \[kg/s\] is the fixed quantity. DWSIM's default.
    Mass,
    /// Molar flow \[mol/s\] is the fixed quantity.
    Mole,
    /// Volumetric flow \[m³/s\] is the fixed quantity.
    Volumetric,
}

impl Default for FlowSpec {
    /// DWSIM defaults `DefinedFlow` to `Mass` (MaterialStream.vb:80).
    fn default() -> Self {
        FlowSpec::Mass
    }
}

/// Basis on which a composition is quoted — DWSIM's
/// `Interfaces.Enums.CompositionBasis` (Enums.vb:400-408).
///
/// Purely a presentation/entry choice; internally DWSIM always stores mole
/// fractions on the mixture phase and derives the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositionBasis {
    /// Mole fractions \[-\], summing to 1.
    MolarFractions,
    /// Mass fractions \[-\], summing to 1.
    MassFractions,
    /// Volumetric fractions \[-\], summing to 1.
    VolumetricFractions,
    /// Per-compound molar flows \[mol/s\].
    MolarFlows,
    /// Per-compound mass flows \[kg/s\].
    MassFlows,
    /// Per-compound volumetric flows \[m³/s\].
    VolumetricFlows,
    /// Whatever the flowsheet's global default basis is.
    DefaultBasis,
}

/// Force the stream into a single phase regardless of the flash result —
/// DWSIM's `Interfaces.Enums.ForcedPhase` (Enums.vb:428-434), exposed as
/// `MaterialStream.ForcePhase` (MaterialStream.vb:82).
///
/// Used to keep a known-liquid utility stream from being flashed into a
/// spurious two-phase state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForcedPhase {
    /// No forcing beyond the global default.
    None,
    /// Force all-vapour.
    Vapor,
    /// Force all-liquid.
    Liquid,
    /// Force all-solid.
    Solid,
    /// Use the flowsheet-level global setting. DWSIM's default.
    GlobalDef,
}

impl Default for ForcedPhase {
    /// DWSIM defaults `ForcePhase` to `GlobalDef` (MaterialStream.vb:82).
    fn default() -> Self {
        ForcedPhase::GlobalDef
    }
}

/// Phase label used for reporting — DWSIM's `Interfaces.Enums.PhaseLabel`
/// (Enums.vb:410-419). A finer classification than [`PhaseName`], and distinct
/// from [`PhaseIndex`] (which is a storage slot, not a label).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseLabel {
    /// The overall mixture.
    Mixture,
    /// Vapour.
    Vapor,
    /// All liquid sub-phases lumped.
    LiquidMixture,
    /// Liquid 1.
    Liquid1,
    /// Liquid 2.
    Liquid2,
    /// Liquid 3.
    Liquid3,
    /// Aqueous liquid.
    Aqueous,
    /// Solid.
    Solid,
}

/// Coarse phase classification — DWSIM's `Interfaces.Enums.PhaseName`
/// (Enums.vb:421-426).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseName {
    /// Any liquid.
    Liquid,
    /// Vapour.
    Vapor,
    /// The overall mixture.
    Mixture,
    /// Solid.
    Solid,
}

/// One compound's state within one phase slot — DWSIM's
/// `BaseClasses.Compound` (ThermodynamicsBase.vb:39-148).
///
/// Every field is `Option<f64>` because DWSIM uses `Double?` and treats
/// `Nothing` as "not yet calculated" — a distinction
/// [`MaterialStreamData::clear`] relies on. Units are spelled out per field.
///
/// Only [`StreamCompound::molar_mass`] survives from DWSIM's ~200-field
/// `ConstantProperties` record (see the module's "Excluded DWSIM behavior"):
/// it is the one constant the ported composition algebra needs.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamCompound {
    /// Compound name — the dictionary key upstream, and the join key against
    /// [`crate::thermo::component::Component::name`].
    pub name: String,
    /// Molar mass `M` \[kg/kmol\] (equivalently g/mol) — DWSIM's
    /// `ConstantProperties.Molar_Weight`, whose unit is documented at
    /// `MaterialStream.vb:8578` ("Molecular weight in kg/kmol").
    ///
    /// **The composition algebra only ever uses ratios of molar masses**
    /// (`x_i / M_i` divided by `sum_j x_j / M_j`), so any consistent unit gives
    /// identical results; kg/kmol is kept to match DWSIM byte-for-byte. Must be
    /// `> 0` for the mass/mole conversions to be defined.
    pub molar_mass: f64,
    /// Mole fraction `x_i` \[-\] within this phase, in `[0, 1]`.
    pub mole_fraction: Option<f64>,
    /// Mass fraction `w_i` \[-\] within this phase, in `[0, 1]`.
    pub mass_fraction: Option<f64>,
    /// Molar flow of this compound in this phase \[mol/s\].
    pub molar_flow: Option<f64>,
    /// Mass flow of this compound in this phase \[kg/s\].
    pub mass_flow: Option<f64>,
    /// Volumetric flow of this compound in this phase \[m³/s\].
    pub volumetric_flow: Option<f64>,
    /// Volumetric fraction \[-\].
    pub volumetric_fraction: Option<f64>,
    /// Activity \[-\].
    pub activity: Option<f64>,
    /// Activity coefficient `gamma_i` \[-\].
    pub activity_coefficient: Option<f64>,
    /// Fugacity coefficient `phi_i` \[-\].
    pub fugacity_coefficient: Option<f64>,
    /// Equilibrium ratio `K_i = y_i / x_i` \[-\].
    pub kvalue: Option<f64>,
    /// Natural log of the equilibrium ratio, `ln K_i` \[-\].
    pub ln_kvalue: Option<f64>,
    /// Molarity \[mol/L\] (electrolyte packages).
    pub molarity: Option<f64>,
    /// Molality \[mol/kg solvent\] (electrolyte packages).
    pub molality: Option<f64>,
    /// Partial pressure `p_i` \[Pa\].
    pub partial_pressure: Option<f64>,
    /// Partial molar volume \[m³/mol\].
    pub partial_volume: Option<f64>,
    /// Diffusion coefficient \[m²/s\].
    pub diffusion_coefficient: Option<f64>,
    /// Whether this compound is a lumped petroleum pseudo-fraction.
    pub petroleum_fraction: bool,
}

impl StreamCompound {
    /// A compound with the given name and molar mass \[kg/kmol\], all state
    /// fields unset (`None`) except the mole/mass fractions and flows, which
    /// DWSIM initialises to `0.0` (ThermodynamicsBase.vb:107-127).
    ///
    /// `molar_mass` must be finite and `> 0`; the mass/mole conversions divide
    /// by it.
    #[must_use]
    pub fn new(name: impl Into<String>, molar_mass: f64) -> Self {
        StreamCompound {
            name: name.into(),
            molar_mass,
            mole_fraction: Some(0.0),
            mass_fraction: Some(0.0),
            molar_flow: Some(0.0),
            mass_flow: Some(0.0),
            volumetric_flow: Some(0.0),
            volumetric_fraction: Some(0.0),
            activity: Some(0.0),
            activity_coefficient: Some(0.0),
            fugacity_coefficient: Some(0.0),
            kvalue: Some(0.0),
            ln_kvalue: Some(0.0),
            molarity: Some(0.0),
            molality: Some(0.0),
            partial_pressure: Some(0.0),
            partial_volume: Some(0.0),
            diffusion_coefficient: None,
            petroleum_fraction: false,
        }
    }
}

/// The property bag attached to one phase slot — DWSIM's
/// `BaseClasses.PhaseProperties` (ThermodynamicsBase.vb:1258-1385), ported
/// field-for-field.
///
/// Every field is `Option<f64>`, mirroring DWSIM's `Double?`: `None` means "not
/// calculated", which is *not* the same as `Some(0.0)`.
///
/// Units follow DWSIM's internal system (see the module header): temperature K,
/// pressure Pa, **enthalpy kJ/kg, entropy kJ/(kg·K)**, mass flow kg/s, molar
/// flow mol/s, volumetric flow m³/s, molecular weight kg/kmol. Each field
/// restates its own unit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PhaseProperties {
    /// Temperature `T` \[K\]. Must be `> 0` to be physical.
    pub temperature: Option<f64>,
    /// Pressure `P` \[Pa\]. Must be `> 0` to be physical.
    pub pressure: Option<f64>,
    /// Mass enthalpy `h` \[kJ/kg\] (datum is the property package's).
    pub enthalpy: Option<f64>,
    /// Mass enthalpy including enthalpy of formation \[kJ/kg\].
    pub enthalpy_f: Option<f64>,
    /// Mass entropy `s` \[kJ/(kg·K)\].
    pub entropy: Option<f64>,
    /// Mass entropy including entropy of formation \[kJ/(kg·K)\].
    pub entropy_f: Option<f64>,
    /// Molar enthalpy \[kJ/kmol\].
    pub molar_enthalpy: Option<f64>,
    /// Molar enthalpy including formation \[kJ/kmol\].
    pub molar_enthalpy_f: Option<f64>,
    /// Molar entropy \[kJ/(kmol·K)\].
    pub molar_entropy: Option<f64>,
    /// Molar entropy including formation \[kJ/(kmol·K)\].
    pub molar_entropy_f: Option<f64>,
    /// Mass flow of this phase \[kg/s\].
    pub massflow: Option<f64>,
    /// Mass fraction of this phase in the mixture \[-\], in `[0, 1]`.
    pub massfraction: Option<f64>,
    /// Molar flow of this phase \[mol/s\].
    pub molarflow: Option<f64>,
    /// Molar fraction of this phase in the mixture \[-\], in `[0, 1]`. For the
    /// `Vapor` slot this is the vapour fraction `beta`.
    pub molarfraction: Option<f64>,
    /// Volumetric flow of this phase \[m³/s\].
    pub volumetric_flow: Option<f64>,
    /// Volumetric fraction of this phase \[-\].
    pub volumetric_fraction: Option<f64>,
    /// Mass density `rho` \[kg/m³\].
    pub density: Option<f64>,
    /// Mixture molecular weight `MW` \[kg/kmol\].
    pub molecular_weight: Option<f64>,
    /// Compressibility \[1/Pa\].
    pub compressibility: Option<f64>,
    /// Compressibility factor `Z` \[-\].
    pub compressibility_factor: Option<f64>,
    /// Isothermal compressibility \[1/Pa\].
    pub isothermal_compressibility: Option<f64>,
    /// Bulk (volumetric) modulus \[Pa\].
    pub bulk_modulus: Option<f64>,
    /// Constant-pressure heat capacity `Cp` \[kJ/(kg·K)\].
    pub heat_capacity_cp: Option<f64>,
    /// Constant-volume heat capacity `Cv` \[kJ/(kg·K)\].
    pub heat_capacity_cv: Option<f64>,
    /// Ideal-gas `Cp` \[kJ/(kg·K)\].
    pub ideal_gas_heat_capacity_cp: Option<f64>,
    /// Ideal-gas heat-capacity ratio `Cp/Cv` \[-\].
    pub ideal_gas_heat_capacity_ratio: Option<f64>,
    /// Dynamic viscosity `mu` \[Pa·s\].
    pub viscosity: Option<f64>,
    /// Kinematic viscosity `nu` \[m²/s\].
    pub kinematic_viscosity: Option<f64>,
    /// Thermal conductivity `k` \[W/(m·K)\].
    pub thermal_conductivity: Option<f64>,
    /// Surface tension `sigma` \[N/m\].
    pub surface_tension: Option<f64>,
    /// Speed of sound \[m/s\].
    pub speed_of_sound: Option<f64>,
    /// Joule-Thomson coefficient \[K/Pa\].
    pub joule_thomson_coefficient: Option<f64>,
    /// Bubble-point pressure at this temperature \[Pa\].
    pub bubble_pressure: Option<f64>,
    /// Bubble-point temperature at this pressure \[K\].
    pub bubble_temperature: Option<f64>,
    /// Dew-point pressure at this temperature \[Pa\].
    pub dew_pressure: Option<f64>,
    /// Dew-point temperature at this pressure \[K\].
    pub dew_temperature: Option<f64>,
    /// Freezing point \[K\].
    pub freezing_point: Option<f64>,
    /// Freezing-point depression \[K\].
    pub freezing_point_depression: Option<f64>,
    /// Mixture activity \[-\].
    pub activity: Option<f64>,
    /// Mixture activity coefficient \[-\].
    pub activity_coefficient: Option<f64>,
    /// Mixture fugacity \[Pa\].
    pub fugacity: Option<f64>,
    /// Mixture fugacity coefficient \[-\].
    pub fugacity_coefficient: Option<f64>,
    /// Natural log of the mixture fugacity coefficient \[-\].
    pub log_fugacity_coefficient: Option<f64>,
    /// Mixture equilibrium ratio \[-\].
    pub kvalue: Option<f64>,
    /// Natural log of the mixture equilibrium ratio \[-\].
    pub log_kvalue: Option<f64>,
    /// Excess mass enthalpy \[kJ/kg\].
    pub excess_enthalpy: Option<f64>,
    /// Excess mass entropy \[kJ/(kg·K)\].
    pub excess_entropy: Option<f64>,
    /// Mass Gibbs free energy \[kJ/kg\].
    pub gibbs_free_energy: Option<f64>,
    /// Mass Helmholtz energy \[kJ/kg\].
    pub helmholtz_energy: Option<f64>,
    /// Mass internal energy \[kJ/kg\].
    pub internal_energy: Option<f64>,
    /// Molar Gibbs free energy \[kJ/kmol\].
    pub molar_gibbs_free_energy: Option<f64>,
    /// Molar Helmholtz energy \[kJ/kmol\].
    pub molar_helmholtz_energy: Option<f64>,
    /// Molar internal energy \[kJ/kmol\].
    pub molar_internal_energy: Option<f64>,
    /// Ionic strength \[mol/kg\] (electrolyte packages).
    pub ionic_strength: Option<f64>,
    /// Mean ionic activity coefficient \[-\] (electrolyte packages).
    pub mean_ionic_activity_coefficient: Option<f64>,
    /// Osmotic coefficient \[-\] (electrolyte packages).
    pub osmotic_coefficient: Option<f64>,
    /// pH \[-\] (electrolyte packages).
    pub ph: Option<f64>,
    /// CO2 loading \[mol CO2 / mol amine\] (amine packages).
    pub co2_loading: Option<f64>,
    /// CO2 partial pressure \[Pa\] (amine packages).
    pub co2_partial_pressure: Option<f64>,
    /// H2S loading \[mol H2S / mol amine\] (amine packages).
    pub h2s_loading: Option<f64>,
    /// H2S partial pressure \[Pa\] (amine packages).
    pub h2s_partial_pressure: Option<f64>,
    /// Mean solid particle size \[m\].
    pub particle_size_mean: Option<f64>,
    /// Standard deviation of the solid particle size \[m\].
    pub particle_size_std_dev: Option<f64>,
}

/// One phase slot of a material stream: its label, its property bag, and its
/// per-compound composition — DWSIM's `BaseClasses.Phase`
/// (ThermodynamicsBase.vb:150+).
///
/// The `compounds` vector is kept in the **same order for every slot** of a
/// given stream, so index `i` names the same compound in the mixture, vapour and
/// liquid slots. All the composition algebra relies on that invariant;
/// [`MaterialStreamData::add_compound`] maintains it.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseData {
    /// Which slot this is.
    pub index: PhaseIndex,
    /// Bulk properties of this phase.
    pub properties: PhaseProperties,
    /// Per-compound state, in the stream's canonical compound order.
    pub compounds: Vec<StreamCompound>,
}

impl PhaseData {
    /// An empty phase slot: no compounds, no calculated properties.
    #[must_use]
    pub fn new(index: PhaseIndex) -> Self {
        PhaseData {
            index,
            properties: PhaseProperties::default(),
            compounds: Vec::new(),
        }
    }
}

/// Snapshot of the inputs a material stream was last solved at — DWSIM's
/// `MaterialStreamInputData`, consumed by `CheckDirtyStatus`
/// (MaterialStream.vb:217-255).
///
/// The solver stores one of these after each successful stream calculation and
/// compares against it to decide whether the stream must be recalculated.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialStreamInputData {
    /// Temperature at the last solution \[K\].
    pub temperature: f64,
    /// Pressure at the last solution \[Pa\].
    pub pressure: f64,
    /// Mass flow at the last solution \[kg/s\].
    pub mass_flow: f64,
    /// Molar flow at the last solution \[mol/s\].
    pub molar_flow: f64,
    /// Volumetric flow at the last solution \[m³/s\].
    pub volumetric_flow: f64,
    /// Mass enthalpy at the last solution \[kJ/kg\].
    pub enthalpy: f64,
    /// Mass entropy at the last solution \[kJ/(kg·K)\].
    pub entropy: f64,
    /// Vapour molar fraction at the last solution \[-\].
    pub vapor_fraction: f64,
    /// Overall molar composition at the last solution \[-\], in the stream's
    /// canonical compound order.
    pub molar_composition: Vec<f64>,
}

/// Why a material stream's specification is not usable.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StreamValidationError {
    /// One of the four properties DWSIM's `Validate` checks
    /// (MaterialStream.vb:257-278) is absent or not a finite number. DWSIM
    /// throws `ArgumentException` with the same four names.
    #[error("material stream `{tag}`: property `{property}` is missing or not finite")]
    InvalidSpecValue {
        /// The stream's tag, for the message.
        tag: String,
        /// Which property failed: `temperature`, `pressure`, `enthalpy`, or
        /// `entropy`.
        property: &'static str,
    },
    /// A composition vector's length did not match the stream's compound count.
    #[error("composition length {given} does not match the stream's {expected} compounds")]
    CompositionLengthMismatch {
        /// Length supplied by the caller.
        given: usize,
        /// Number of compounds on the stream.
        expected: usize,
    },
}

/// The full thermodynamic state a material stream carries — the Rust port of
/// DWSIM's `MaterialStream` **data model** (no calculator; see the module's
/// "Excluded DWSIM behavior").
///
/// Holds all eight phase slots (always exactly eight, in [`PhaseIndex::ALL`]
/// order), the specification enums, and the equilibrium flag. Owned by value —
/// no references, no lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialStreamData {
    /// Which property pair is the independent specification.
    pub spec: StreamSpec,
    /// Which flow quantity the user fixed.
    pub defined_flow: FlowSpec,
    /// Single-phase forcing, if any.
    pub forced_phase: ForcedPhase,
    /// Whether the stored phase split is a converged equilibrium result
    /// (DWSIM's `AtEquilibrium`, MaterialStream.vb:280). Cleared by
    /// [`MaterialStreamData::clear`].
    pub at_equilibrium: bool,
    /// The eight phase slots, always in [`PhaseIndex::ALL`] order so that
    /// `phases[PhaseIndex::Vapor.index()]` is the vapour slot.
    pub phases: Vec<PhaseData>,
    /// The inputs this stream was last solved at, if it has been solved
    /// (DWSIM's `LastSolutionInputData`, MaterialStream.vb:88).
    pub last_solution_input: Option<MaterialStreamInputData>,
}

impl Default for MaterialStreamData {
    fn default() -> Self {
        MaterialStreamData::new()
    }
}

impl MaterialStreamData {
    /// A new material stream with eight empty phase slots, no compounds, and
    /// DWSIM's documented defaults on the mixture slot: `T = 298.15 K`,
    /// `P = 101325 Pa`, mass flow `w = 1 kg/s` (MaterialStream.vb:381-383).
    ///
    /// The specification defaults to [`StreamSpec::TemperatureAndPressure`] and
    /// the defined flow to [`FlowSpec::Mass`], matching upstream.
    #[must_use]
    pub fn new() -> Self {
        let mut phases: Vec<PhaseData> =
            PhaseIndex::ALL.iter().map(|p| PhaseData::new(*p)).collect();
        phases[PhaseIndex::Mixture.index()].properties.temperature = Some(298.15);
        phases[PhaseIndex::Mixture.index()].properties.pressure = Some(101_325.0);
        phases[PhaseIndex::Mixture.index()].properties.massflow = Some(1.0);
        MaterialStreamData {
            spec: StreamSpec::default(),
            defined_flow: FlowSpec::default(),
            forced_phase: ForcedPhase::default(),
            at_equilibrium: false,
            phases,
            last_solution_input: None,
        }
    }

    /// Borrow a phase slot.
    #[must_use]
    pub fn phase(&self, p: PhaseIndex) -> &PhaseData {
        &self.phases[p.index()]
    }

    /// Mutably borrow a phase slot.
    pub fn phase_mut(&mut self, p: PhaseIndex) -> &mut PhaseData {
        &mut self.phases[p.index()]
    }

    /// Number of compounds on this stream (the length shared by every slot's
    /// `compounds` vector).
    #[must_use]
    pub fn compound_count(&self) -> usize {
        self.phase(PhaseIndex::Mixture).compounds.len()
    }

    /// Compound names in canonical order — DWSIM's `GetCompoundNames`
    /// (MaterialStream.vb:1335-1339).
    #[must_use]
    pub fn compound_names(&self) -> Vec<String> {
        self.phase(PhaseIndex::Mixture)
            .compounds
            .iter()
            .map(|c| c.name.clone())
            .collect()
    }

    /// Add a compound to **every** phase slot, preserving the shared ordering
    /// invariant — the data-model half of DWSIM's
    /// `AddCompoundsToMaterialStream` (FlowsheetBase.vb:122-133).
    ///
    /// `molar_mass` is in kg/kmol (see [`StreamCompound::molar_mass`]) and must
    /// be finite and `> 0`. Compounds start with zero fractions; call
    /// [`MaterialStreamData::equalize_overall_composition`] or
    /// [`MaterialStreamData::set_overall_molar_composition`] afterwards, as
    /// DWSIM does at FlowsheetBase.vb:131-132.
    pub fn add_compound(&mut self, name: impl Into<String>, molar_mass: f64) {
        let name = name.into();
        for phase in &mut self.phases {
            phase
                .compounds
                .push(StreamCompound::new(name.clone(), molar_mass));
        }
    }

    /// Set the overall (mixture-slot) **mole** fractions — DWSIM's
    /// `SetOverallComposition` / `SetOverallMolarComposition`
    /// (MaterialStream.vb:1163-1182).
    ///
    /// `x` are mole fractions \[-\]; a physical composition sums to 1, but
    /// upstream does not enforce that and neither does this (call
    /// [`MaterialStreamData::normalize_overall_mole_composition`] if you want
    /// it enforced).
    ///
    /// # Errors
    /// [`StreamValidationError::CompositionLengthMismatch`] if `x.len()` is not
    /// the stream's compound count. (Upstream indexes past the end and throws
    /// `IndexOutOfRangeException`; reporting is the honest equivalent.)
    pub fn set_overall_molar_composition(
        &mut self,
        x: &[f64],
    ) -> Result<(), StreamValidationError> {
        let n = self.compound_count();
        if x.len() != n {
            return Err(StreamValidationError::CompositionLengthMismatch {
                given: x.len(),
                expected: n,
            });
        }
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        for (c, &xi) in mixture.compounds.iter_mut().zip(x.iter()) {
            c.mole_fraction = Some(xi);
        }
        Ok(())
    }

    /// Set the overall composition from **mass** fractions, converting to mole
    /// fractions — DWSIM's `SetOverallMassComposition`
    /// (MaterialStream.vb:1183-1205):
    ///
    /// `x_i = (w_i / M_i) / sum_j (w_j / M_j)`
    ///
    /// where `w` are the supplied mass fractions \[-\] and `M` the compound
    /// molar masses. The molar-mass unit cancels, so kg/kmol vs kg/mol is
    /// immaterial.
    ///
    /// # Errors
    /// [`StreamValidationError::CompositionLengthMismatch`] on a length
    /// mismatch. If every `w_i` is zero the denominator is zero and the
    /// resulting mole fractions are `NaN`, exactly as upstream — supply a
    /// non-degenerate composition.
    pub fn set_overall_mass_composition(&mut self, w: &[f64]) -> Result<(), StreamValidationError> {
        let n = self.compound_count();
        if w.len() != n {
            return Err(StreamValidationError::CompositionLengthMismatch {
                given: w.len(),
                expected: n,
            });
        }
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        let mass_div_mm: f64 = mixture
            .compounds
            .iter()
            .zip(w.iter())
            .map(|(c, &wi)| wi / c.molar_mass)
            .sum();
        for (c, &wi) in mixture.compounds.iter_mut().zip(w.iter()) {
            c.mole_fraction = Some(wi / c.molar_mass / mass_div_mm);
        }
        Ok(())
    }

    /// Set every overall mole fraction to `1/n` — DWSIM's
    /// `EqualizeOverallComposition` (MaterialStream.vb:1206-1212), used as the
    /// initial composition of a freshly created stream
    /// (FlowsheetBase.vb:131).
    ///
    /// A no-op on a stream with no compounds.
    pub fn equalize_overall_composition(&mut self) {
        let n = self.compound_count();
        if n == 0 {
            return;
        }
        let x = 1.0 / n as f64;
        for c in &mut self.phase_mut(PhaseIndex::Mixture).compounds {
            c.mole_fraction = Some(x);
        }
    }

    /// Rescale the overall mole fractions so they sum to 1 — DWSIM's
    /// `NormalizeOverallMoleComposition` (MaterialStream.vb:1214-1225).
    ///
    /// Unset fractions count as 0 (upstream's `GetValueOrDefault`). If the sum
    /// is zero the result is `NaN`, as upstream.
    pub fn normalize_overall_mole_composition(&mut self) {
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        let total: f64 = mixture
            .compounds
            .iter()
            .map(|c| c.mole_fraction.unwrap_or(0.0))
            .sum();
        for c in &mut mixture.compounds {
            c.mole_fraction = Some(c.mole_fraction.unwrap_or(0.0) / total);
        }
    }

    /// Rescale the overall mass fractions so they sum to 1 — DWSIM's
    /// `NormalizeOverallMassComposition` (MaterialStream.vb:1227-1238).
    pub fn normalize_overall_mass_composition(&mut self) {
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        let total: f64 = mixture
            .compounds
            .iter()
            .map(|c| c.mass_fraction.unwrap_or(0.0))
            .sum();
        for c in &mut mixture.compounds {
            c.mass_fraction = Some(c.mass_fraction.unwrap_or(0.0) / total);
        }
    }

    /// Derive overall **mass** fractions from the overall mole fractions —
    /// DWSIM's `CalcOverallCompMassFractions` (MaterialStream.vb:1240-1255):
    ///
    /// `w_i = x_i M_i / sum_j (x_j M_j)`
    ///
    /// Reproduces upstream's guard: when `sum_j x_j M_j <= 0` every mass
    /// fraction is set to `0.0` rather than `NaN` (MaterialStream.vb:1249-1252).
    pub fn calc_overall_comp_mass_fractions(&mut self) {
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        let mol_x_mm: f64 = mixture
            .compounds
            .iter()
            .map(|c| c.mole_fraction.unwrap_or(0.0) * c.molar_mass)
            .sum();
        for c in &mut mixture.compounds {
            c.mass_fraction = Some(if mol_x_mm > 0.0 {
                c.mole_fraction.unwrap_or(0.0) * c.molar_mass / mol_x_mm
            } else {
                0.0
            });
        }
    }

    /// Derive overall **mole** fractions from the overall mass fractions —
    /// DWSIM's `CalcOverallCompMoleFractions` (MaterialStream.vb:1257-1268):
    ///
    /// `x_i = (w_i / M_i) / sum_j (w_j / M_j)`
    ///
    /// Upstream has **no** zero guard here (unlike the mass direction), so a
    /// zero denominator yields `NaN`; that behaviour is reproduced.
    pub fn calc_overall_comp_mole_fractions(&mut self) {
        let mixture = self.phase_mut(PhaseIndex::Mixture);
        let mol_x_mm: f64 = mixture
            .compounds
            .iter()
            .map(|c| c.mass_fraction.unwrap_or(0.0) / c.molar_mass)
            .sum();
        for c in &mut mixture.compounds {
            c.mole_fraction = Some(c.mass_fraction.unwrap_or(0.0) / c.molar_mass / mol_x_mm);
        }
    }

    /// Set the **mole** fractions of one phase slot — DWSIM's
    /// `SetPhaseComposition` (MaterialStream.vb:1270-1311).
    ///
    /// # Errors
    /// [`StreamValidationError::CompositionLengthMismatch`] on a length
    /// mismatch.
    ///
    /// # Note on an upstream defect
    /// DWSIM's `SetPhaseComposition` maps both `Phase.Aqueous` **and**
    /// `Phase.Vapor` to slot index 2 (MaterialStream.vb:1284, :1297) — the
    /// aqueous case is a copy-paste bug that silently writes the vapour slot.
    /// This port takes a [`PhaseIndex`] directly, so the bug cannot occur and is
    /// deliberately **not** reproduced.
    pub fn set_phase_composition(
        &mut self,
        p: PhaseIndex,
        x: &[f64],
    ) -> Result<(), StreamValidationError> {
        let n = self.compound_count();
        if x.len() != n {
            return Err(StreamValidationError::CompositionLengthMismatch {
                given: x.len(),
                expected: n,
            });
        }
        for (c, &xi) in self.phase_mut(p).compounds.iter_mut().zip(x.iter()) {
            c.mole_fraction = Some(xi);
        }
        Ok(())
    }

    /// Mole fractions of one phase slot \[-\] — DWSIM's `GetPhaseComposition`
    /// (MaterialStream.vb:1313-1321). Unset fractions read as `0.0`.
    #[must_use]
    pub fn phase_composition(&self, p: PhaseIndex) -> Vec<f64> {
        self.phase(p)
            .compounds
            .iter()
            .map(|c| c.mole_fraction.unwrap_or(0.0))
            .collect()
    }

    /// Overall (mixture-slot) mole fractions \[-\] — DWSIM's
    /// `GetOverallComposition` (MaterialStream.vb:1323-1327).
    #[must_use]
    pub fn overall_composition(&self) -> Vec<f64> {
        self.phase_composition(PhaseIndex::Mixture)
    }

    /// Overall (mixture-slot) mass fractions \[-\] — DWSIM's
    /// `GetOverallMassComposition` (MaterialStream.vb:1329-1333).
    #[must_use]
    pub fn overall_mass_composition(&self) -> Vec<f64> {
        self.phase(PhaseIndex::Mixture)
            .compounds
            .iter()
            .map(|c| c.mass_fraction.unwrap_or(0.0))
            .collect()
    }

    /// Clear the basic per-phase state of every slot — DWSIM's `Clear`
    /// (MaterialStream.vb:1089-1128).
    ///
    /// Clears, on all eight slots: temperature, pressure, enthalpy, molar and
    /// mass fraction, and the mass/molar/volumetric flows; and on every
    /// compound: mole fraction, mass fraction, molar flow, mass flow. Also
    /// clears [`MaterialStreamData::at_equilibrium`].
    ///
    /// Note this is *not* the same as [`MaterialStreamData::deep_clear`]: it
    /// leaves derived properties (density, viscosity, `Cp`, ...) in place,
    /// exactly as upstream does.
    pub fn clear(&mut self) {
        for phase in &mut self.phases {
            let p = &mut phase.properties;
            p.temperature = None;
            p.pressure = None;
            p.enthalpy = None;
            p.molarfraction = None;
            p.massfraction = None;
            p.massflow = None;
            p.molarflow = None;
            p.volumetric_flow = None;
            for c in &mut phase.compounds {
                c.mole_fraction = None;
                c.mass_fraction = None;
                c.molar_flow = None;
                c.mass_flow = None;
            }
        }
        self.at_equilibrium = false;
    }

    /// Clear **all** stored state on every slot — DWSIM's `DeepClear`
    /// (MaterialStream.vb:1078-1083), which is `Clear()` followed by
    /// `ClearAllProps()`.
    ///
    /// Upstream's `ClearCalculatedProps` reaches into the property package to
    /// zero its caches (MaterialStream.vb:1131-1162); that call is excluded from
    /// this port (see the module header), so this resets the phase property bags
    /// to [`PhaseProperties::default`] and clears the compound state, which is
    /// the same *observable* end state for the data model.
    pub fn deep_clear(&mut self) {
        for phase in &mut self.phases {
            phase.properties = PhaseProperties::default();
            for c in &mut phase.compounds {
                let name = c.name.clone();
                let mm = c.molar_mass;
                *c = StreamCompound::new(name, mm);
                c.mole_fraction = None;
                c.mass_fraction = None;
                c.molar_flow = None;
                c.mass_flow = None;
            }
        }
        self.at_equilibrium = false;
        self.last_solution_input = None;
    }

    /// Check the four properties DWSIM's `Validate` requires
    /// (MaterialStream.vb:257-278): temperature, pressure, enthalpy, entropy on
    /// the mixture slot must each be present and a finite number.
    ///
    /// `tag` is only used to build the error message (upstream reads it off the
    /// graphic object).
    ///
    /// # Errors
    /// [`StreamValidationError::InvalidSpecValue`] naming the first failing
    /// property, in upstream's order (temperature, pressure, enthalpy, entropy).
    pub fn validate(&self, tag: &str) -> Result<(), StreamValidationError> {
        let p = &self.phase(PhaseIndex::Mixture).properties;
        for (name, value) in [
            ("temperature", p.temperature),
            ("pressure", p.pressure),
            ("enthalpy", p.enthalpy),
            ("entropy", p.entropy),
        ] {
            match value {
                Some(v) if v.is_finite() => {}
                _ => {
                    return Err(StreamValidationError::InvalidSpecValue {
                        tag: tag.to_string(),
                        property: name,
                    })
                }
            }
        }
        Ok(())
    }

    /// Whether this stream's inputs have moved away from the state it was last
    /// solved at — DWSIM's `CheckDirtyStatus` (MaterialStream.vb:217-255).
    ///
    /// Compares T, P, mass/molar/volumetric flow, mass enthalpy, mass entropy,
    /// vapour molar fraction and every overall mole fraction against
    /// [`MaterialStreamData::last_solution_input`], each with upstream's
    /// absolute tolerance `epsilon = 1e-6` (MaterialStream.vb:220). A change in
    /// the number of compounds counts as dirty.
    ///
    /// Returns `false` when there is no stored snapshot — matching upstream,
    /// which leaves the dirty flag untouched in that case
    /// (MaterialStream.vb:222). Note the tolerance is **absolute and applied to
    /// quantities of very different magnitude** (a 1e-6 Pa change in a 1e5 Pa
    /// pressure is meaningless, a 1e-6 change in a mole fraction is not); that
    /// is upstream's choice and is reproduced rather than silently "improved".
    #[must_use]
    pub fn is_dirty_versus_last_solution(&self) -> bool {
        let Some(last) = &self.last_solution_input else {
            return false;
        };
        const EPS: f64 = 0.000_001;
        let p = &self.phase(PhaseIndex::Mixture).properties;
        let vf = self
            .phase(PhaseIndex::Vapor)
            .properties
            .molarfraction
            .unwrap_or(0.0);
        let checks = [
            (p.temperature.unwrap_or(0.0), last.temperature),
            (p.pressure.unwrap_or(0.0), last.pressure),
            (p.massflow.unwrap_or(0.0), last.mass_flow),
            (p.molarflow.unwrap_or(0.0), last.molar_flow),
            (p.volumetric_flow.unwrap_or(0.0), last.volumetric_flow),
            (p.enthalpy.unwrap_or(0.0), last.enthalpy),
            (p.entropy.unwrap_or(0.0), last.entropy),
            (vf, last.vapor_fraction),
        ];
        if checks.iter().any(|(now, then)| (now - then).abs() > EPS) {
            return true;
        }
        let comp = self.overall_composition();
        if comp.len() != last.molar_composition.len() {
            return true;
        }
        comp.iter()
            .zip(last.molar_composition.iter())
            .any(|(a, b)| (a - b).abs() > EPS)
    }

    /// Capture the current mixture state as a [`MaterialStreamInputData`]
    /// snapshot, for a later [`MaterialStreamData::is_dirty_versus_last_solution`]
    /// comparison. The solver calls this after a successful stream calculation.
    #[must_use]
    pub fn snapshot_input(&self) -> MaterialStreamInputData {
        let p = &self.phase(PhaseIndex::Mixture).properties;
        MaterialStreamInputData {
            temperature: p.temperature.unwrap_or(0.0),
            pressure: p.pressure.unwrap_or(0.0),
            mass_flow: p.massflow.unwrap_or(0.0),
            molar_flow: p.molarflow.unwrap_or(0.0),
            volumetric_flow: p.volumetric_flow.unwrap_or(0.0),
            enthalpy: p.enthalpy.unwrap_or(0.0),
            entropy: p.entropy.unwrap_or(0.0),
            vapor_fraction: self
                .phase(PhaseIndex::Vapor)
                .properties
                .molarfraction
                .unwrap_or(0.0),
            molar_composition: self.overall_composition(),
        }
    }

    // ---------------------------------------------------------------------
    // `uom`-typed scalar accessors (MaterialStream.vb:8521-8600).
    //
    // These convert DWSIM's kilo-flavoured internal units to plain SI on the
    // way out and back on the way in, so a caller never has to remember that
    // the raw field is kJ/kg.
    // ---------------------------------------------------------------------

    /// Overall temperature `T` \[K\] — DWSIM's `GetTemperature`
    /// (MaterialStream.vb:8541). `None` if unset.
    #[must_use]
    pub fn temperature(&self) -> Option<ThermodynamicTemperature> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .temperature
            .map(ThermodynamicTemperature::new::<kelvin>)
    }

    /// Set the overall temperature. Physical streams require `T > 0 K`; this is
    /// not enforced, matching upstream.
    pub fn set_temperature(&mut self, t: ThermodynamicTemperature) {
        self.phase_mut(PhaseIndex::Mixture).properties.temperature = Some(t.get::<kelvin>());
    }

    /// Overall pressure `P` \[Pa\] — DWSIM's `GetPressure`
    /// (MaterialStream.vb:8549). `None` if unset.
    #[must_use]
    pub fn pressure(&self) -> Option<Pressure> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .pressure
            .map(Pressure::new::<pascal>)
    }

    /// Set the overall pressure. Physical streams require `P > 0 Pa`.
    pub fn set_pressure(&mut self, p: Pressure) {
        self.phase_mut(PhaseIndex::Mixture).properties.pressure = Some(p.get::<pascal>());
    }

    /// Overall mass flow `w` \[kg/s\] — DWSIM's `GetMassFlow`
    /// (MaterialStream.vb:8557). `None` if unset.
    #[must_use]
    pub fn mass_flow(&self) -> Option<MassRate> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .massflow
            .map(MassRate::new::<kilogram_per_second>)
    }

    /// Set the overall mass flow. Must be `>= 0` to be physical.
    pub fn set_mass_flow(&mut self, w: MassRate) {
        self.phase_mut(PhaseIndex::Mixture).properties.massflow =
            Some(w.get::<kilogram_per_second>());
    }

    /// Overall molar flow `n_dot` \[mol/s\] — DWSIM's `GetMolarFlow`
    /// (MaterialStream.vb:8565). `None` if unset.
    #[must_use]
    pub fn molar_flow(&self) -> Option<MolarFlowRate> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .molarflow
            .map(MolarFlowRate::new::<katal>)
    }

    /// Set the overall molar flow \[mol/s\].
    pub fn set_molar_flow(&mut self, n: MolarFlowRate) {
        self.phase_mut(PhaseIndex::Mixture).properties.molarflow = Some(n.get::<katal>());
    }

    /// Overall volumetric flow `Q` \[m³/s\] — DWSIM's `GetVolumetricFlow`
    /// (MaterialStream.vb:8573). `None` if unset.
    #[must_use]
    pub fn volumetric_flow(&self) -> Option<VolumeRate> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .volumetric_flow
            .map(VolumeRate::new::<cubic_meter_per_second>)
    }

    /// Set the overall volumetric flow \[m³/s\].
    pub fn set_volumetric_flow(&mut self, q: VolumeRate) {
        self.phase_mut(PhaseIndex::Mixture)
            .properties
            .volumetric_flow = Some(q.get::<cubic_meter_per_second>());
    }

    /// Overall **mass** enthalpy `h` — DWSIM's `GetMassEnthalpy`
    /// (MaterialStream.vb:8525), whose raw unit is kJ/kg.
    ///
    /// Returned as a `uom` [`AvailableEnergy`], i.e. plain **J/kg**; the kJ->J
    /// conversion happens here so the typed surface has no kilo trap.
    #[must_use]
    pub fn mass_enthalpy(&self) -> Option<AvailableEnergy> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .enthalpy
            .map(AvailableEnergy::new::<kilojoule_per_kilogram>)
    }

    /// Set the overall mass enthalpy from a `uom` J/kg value, storing kJ/kg.
    pub fn set_mass_enthalpy(&mut self, h: AvailableEnergy) {
        self.phase_mut(PhaseIndex::Mixture).properties.enthalpy =
            Some(h.get::<kilojoule_per_kilogram>());
    }

    /// Overall **mass** entropy `s` — DWSIM's `GetMassEntropy`
    /// (MaterialStream.vb:8533), whose raw unit is kJ/(kg·K).
    ///
    /// Returned as a `uom` [`SpecificHeatCapacity`] (the J/(kg·K) dimension),
    /// converted from kJ/(kg·K).
    #[must_use]
    pub fn mass_entropy(&self) -> Option<SpecificHeatCapacity> {
        self.phase(PhaseIndex::Mixture)
            .properties
            .entropy
            .map(SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>)
    }

    /// Set the overall mass entropy from a `uom` J/(kg·K) value, storing
    /// kJ/(kg·K).
    pub fn set_mass_entropy(&mut self, s: SpecificHeatCapacity) {
        self.phase_mut(PhaseIndex::Mixture).properties.entropy =
            Some(s.get::<kilojoule_per_kilogram_kelvin>());
    }

    /// Vapour molar fraction `beta` \[-\] — the `molarfraction` of the
    /// [`PhaseIndex::Vapor`] slot, which `CheckDirtyStatus` reads at
    /// MaterialStream.vb:229. `None` if the stream has not been flashed.
    ///
    /// Physical range `[0, 1]`: 0 = all liquid, 1 = all vapour.
    #[must_use]
    pub fn vapor_fraction(&self) -> Option<Ratio> {
        self.phase(PhaseIndex::Vapor)
            .properties
            .molarfraction
            .map(Ratio::new::<ratio>)
    }

    /// Set the vapour molar fraction `beta` \[-\] on the vapour slot.
    pub fn set_vapor_fraction(&mut self, beta: Ratio) {
        self.phase_mut(PhaseIndex::Vapor).properties.molarfraction = Some(beta.get::<ratio>());
    }

    /// Mixture molecular weight `MW` \[kg/kmol\] computed from the current
    /// overall mole fractions — DWSIM's `CalcOverallMolecularWeight`
    /// (MaterialStream.vb:8585-8592): `MW = sum_i x_i M_i`.
    ///
    /// Returned as a raw `f64` in kg/kmol rather than a `uom` `MolarMass`,
    /// because the stored compound molar masses are in kg/kmol and converting
    /// would invite a silent factor-1000 error at the boundary; multiply by
    /// `1e-3` for kg/mol.
    #[must_use]
    pub fn calc_overall_molecular_weight(&self) -> f64 {
        self.phase(PhaseIndex::Mixture)
            .compounds
            .iter()
            .map(|c| c.mole_fraction.unwrap_or(0.0) * c.molar_mass)
            .sum()
    }
}

/// The scalar power an energy stream carries — the data half of DWSIM's
/// `EnergyStream` (`DWSIM.UnitOperations/EnergyStream/Streams.vb`, lines
/// 36-120).
///
/// An energy stream is a pure duty/work link: a compressor writes its shaft
/// power onto one, a heater reads its duty from one. It has no composition, no
/// temperature, and no phases.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnergyStreamData {
    /// Power carried by this stream \[kW\] — DWSIM's `EnergyFlow`
    /// (Streams.vb:100-112), whose unit is fixed by `SetValue(energyflow_kW)`
    /// (Streams.vb:114). `None` until a connected unit operation sets it.
    ///
    /// Sign convention is the *producing* unit operation's: a heater's duty is
    /// positive, a cooler's is positive as removed heat. DWSIM does not impose a
    /// global convention on this field, so neither does the port; consult the
    /// equipment model that writes it.
    pub energy_flow: Option<f64>,
}

impl EnergyStreamData {
    /// An energy stream with no power set yet.
    #[must_use]
    pub fn new() -> Self {
        EnergyStreamData { energy_flow: None }
    }

    /// The carried power as a `uom` [`Power`], i.e. plain **W**, converted from
    /// the stored kW. `None` if unset.
    #[must_use]
    pub fn power(&self) -> Option<Power> {
        self.energy_flow.map(Power::new::<kilowatt>)
    }

    /// Set the carried power from a `uom` W value, storing kW.
    pub fn set_power(&mut self, p: Power) {
        self.energy_flow = Some(p.get::<kilowatt>());
    }

    /// Set the carried power directly in kW — DWSIM's
    /// `SetValue(energyflow_kW)` (Streams.vb:114-116).
    pub fn set_value_kw(&mut self, energy_flow_kw: f64) {
        self.energy_flow = Some(energy_flow_kw);
    }

    /// Clear the carried power (back to "not calculated").
    pub fn clear(&mut self) {
        self.energy_flow = None;
    }
}

/// Convert a raw DWSIM mass enthalpy \[kJ/kg\] to a `uom` [`AvailableEnergy`]
/// \[J/kg\].
///
/// Provided because the raw fields are the porting-faithful storage and callers
/// stepping outside the typed accessors still need a safe conversion.
#[must_use]
pub fn mass_enthalpy_from_kj_per_kg(h_kj_per_kg: f64) -> AvailableEnergy {
    AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_per_kg)
}

/// Convert a `uom` [`AvailableEnergy`] to a raw DWSIM mass enthalpy \[kJ/kg\].
#[must_use]
pub fn mass_enthalpy_to_kj_per_kg(h: AvailableEnergy) -> f64 {
    h.get::<kilojoule_per_kilogram>()
}

/// Convert a raw DWSIM energy flow \[kW\] to a `uom` [`Power`] \[W\].
#[must_use]
pub fn power_from_kw(p_kw: f64) -> Power {
    Power::new::<kilowatt>(p_kw)
}

#[cfg(test)]
mod tests {
    //! # V&V — stream data model
    //!
    //! **Methodology.** These are *verification* tests: they check that the
    //! ported composition algebra reproduces the DWSIM expressions
    //! (MaterialStream.vb:1163-1268) on hand-computable inputs, that the
    //! defaults match MaterialStream.vb:381-383, that `clear`/`validate`/
    //! `CheckDirtyStatus` behave as upstream, and that the `uom` accessors
    //! round-trip the kJ<->J and kW<->W conversions exactly. They are **not**
    //! validation against experimental data — no thermodynamics is evaluated
    //! here. Numbers recorded 2026-08-11, release build.

    use super::*;
    use approx::assert_relative_eq;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::power::watt;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;

    /// Build a two-compound stream: water (M = 18.015 kg/kmol) and
    /// oxygen (M = 31.999 kg/kmol).
    fn two_compound_stream() -> MaterialStreamData {
        let mut s = MaterialStreamData::new();
        s.add_compound("Water", 18.015);
        s.add_compound("Oxygen", 31.999);
        s
    }

    /// **Methodology.** A new stream must have exactly eight phase slots in
    /// upstream index order and DWSIM's documented mixture defaults
    /// `T = 298.15 K`, `P = 101325 Pa`, `w = 1 kg/s`
    /// (MaterialStream.vb:381-383), with `SpecType = Temperature_and_Pressure`
    /// (MaterialStream.vb:6589) and `DefinedFlow = Mass` (:80).
    /// **Result (2026-08-11):** 8 slots; T = 298.150000 K, P = 101325.000000 Pa,
    /// w = 1.000000 kg/s; spec and flow defaults as expected.
    #[test]
    fn new_stream_matches_upstream_defaults() {
        let s = MaterialStreamData::new();
        assert_eq!(s.phases.len(), 8);
        for (i, p) in PhaseIndex::ALL.iter().enumerate() {
            assert_eq!(s.phases[i].index, *p);
            assert_eq!(p.index(), i);
        }
        assert_relative_eq!(
            s.temperature().unwrap().get::<kelvin>(),
            298.15,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            s.pressure().unwrap().get::<pascal>(),
            101_325.0,
            epsilon = 1e-9
        );
        assert_relative_eq!(
            s.mass_flow().unwrap().get::<kilogram_per_second>(),
            1.0,
            epsilon = 1e-12
        );
        assert_eq!(s.spec, StreamSpec::TemperatureAndPressure);
        assert_eq!(s.defined_flow, FlowSpec::Mass);
        assert_eq!(s.forced_phase, ForcedPhase::GlobalDef);
        assert!(!s.at_equilibrium);
        assert_eq!(PhaseIndex::from_index(2), Some(PhaseIndex::Vapor));
        assert_eq!(PhaseIndex::from_index(8), None);
        assert_eq!(PhaseIndex::OverallLiquid.upstream_name(), "OverallLiquid");
    }

    /// **Methodology.** [`MaterialStreamData::add_compound`] must extend **every**
    /// slot so index `i` names the same compound everywhere (the invariant the
    /// composition algebra depends on).
    /// **Result (2026-08-11):** all 8 slots carry 2 compounds in the same order.
    #[test]
    fn compounds_are_added_to_every_phase_slot() {
        let s = two_compound_stream();
        assert_eq!(s.compound_count(), 2);
        assert_eq!(s.compound_names(), vec!["Water", "Oxygen"]);
        for p in PhaseIndex::ALL {
            let names: Vec<&str> = s
                .phase(p)
                .compounds
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            assert_eq!(names, vec!["Water", "Oxygen"], "slot {p:?}");
        }
    }

    /// **Methodology.** `EqualizeOverallComposition` (MaterialStream.vb:1206)
    /// sets every overall mole fraction to `1/n`.
    /// **Result (2026-08-11):** both fractions = 0.500000, sum = 1.
    #[test]
    fn equalize_overall_composition_sets_uniform_fractions() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        let x = s.overall_composition();
        assert_relative_eq!(x[0], 0.5, epsilon = 1e-15);
        assert_relative_eq!(x[1], 0.5, epsilon = 1e-15);
        assert_relative_eq!(x.iter().sum::<f64>(), 1.0, epsilon = 1e-15);
    }

    /// **Methodology.** Mole -> mass conversion,
    /// `CalcOverallCompMassFractions` (MaterialStream.vb:1240):
    /// `w_i = x_i M_i / sum_j x_j M_j`. With `x = [0.5, 0.5]`,
    /// `M = [18.015, 31.999]`: `sum = 9.0075 + 15.9995 = 25.007`;
    /// `w_1 = 9.0075/25.007 = 0.36020`, `w_2 = 15.9995/25.007 = 0.63980`.
    /// Then the inverse, `CalcOverallCompMoleFractions` (:1257), must return
    /// `x = [0.5, 0.5]`.
    /// **Result (2026-08-11):** w = (0.360195, 0.639805), sum 1.000000;
    /// round trip back to (0.500000, 0.500000) within 1e-14.
    #[test]
    fn mass_and_mole_fraction_conversions_round_trip() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        s.calc_overall_comp_mass_fractions();
        let w = s.overall_mass_composition();
        assert_relative_eq!(w[0], 9.0075 / 25.007, epsilon = 1e-12);
        assert_relative_eq!(w[1], 15.9995 / 25.007, epsilon = 1e-12);
        assert_relative_eq!(w.iter().sum::<f64>(), 1.0, epsilon = 1e-14);

        s.calc_overall_comp_mole_fractions();
        let x = s.overall_composition();
        assert_relative_eq!(x[0], 0.5, epsilon = 1e-14);
        assert_relative_eq!(x[1], 0.5, epsilon = 1e-14);
    }

    /// **Methodology.** `CalcOverallCompMassFractions`'s zero guard
    /// (MaterialStream.vb:1249-1252): when every mole fraction is zero the mass
    /// fractions must be set to exactly `0.0`, not `NaN`.
    /// **Result (2026-08-11):** both mass fractions = 0.000000, none NaN.
    #[test]
    fn mass_fraction_zero_guard_matches_upstream() {
        let mut s = two_compound_stream();
        s.set_overall_molar_composition(&[0.0, 0.0]).unwrap();
        s.calc_overall_comp_mass_fractions();
        let w = s.overall_mass_composition();
        assert_eq!(w, vec![0.0, 0.0]);
        assert!(w.iter().all(|v| v.is_finite()));
    }

    /// **Methodology.** `SetOverallMassComposition` (MaterialStream.vb:1183)
    /// converts mass to mole fractions by `x_i = (w_i/M_i)/sum_j(w_j/M_j)`.
    /// With `w = [0.360195, 0.639805]` and the same molar masses, the result
    /// must be `[0.5, 0.5]`. Also checks the length-mismatch error, which
    /// upstream would hit as an index-out-of-range exception.
    /// **Result (2026-08-11):** x = (0.500000, 0.500000) within 1e-6; a
    /// 3-element vector on a 2-compound stream returns
    /// `CompositionLengthMismatch { given: 3, expected: 2 }`.
    #[test]
    fn set_overall_mass_composition_converts_and_validates_length() {
        let mut s = two_compound_stream();
        s.set_overall_mass_composition(&[9.0075 / 25.007, 15.9995 / 25.007])
            .unwrap();
        let x = s.overall_composition();
        assert_relative_eq!(x[0], 0.5, epsilon = 1e-12);
        assert_relative_eq!(x[1], 0.5, epsilon = 1e-12);

        assert_eq!(
            s.set_overall_mass_composition(&[0.3, 0.3, 0.4]),
            Err(StreamValidationError::CompositionLengthMismatch {
                given: 3,
                expected: 2
            })
        );
        assert_eq!(
            s.set_overall_molar_composition(&[1.0]),
            Err(StreamValidationError::CompositionLengthMismatch {
                given: 1,
                expected: 2
            })
        );
    }

    /// **Methodology.** Normalisation (MaterialStream.vb:1214, :1227) must
    /// rescale to sum 1. Input `x = [1.0, 3.0]` -> `[0.25, 0.75]`.
    /// **Result (2026-08-11):** x = (0.250000, 0.750000), sum 1.000000.
    #[test]
    fn normalization_rescales_to_unit_sum() {
        let mut s = two_compound_stream();
        s.set_overall_molar_composition(&[1.0, 3.0]).unwrap();
        s.normalize_overall_mole_composition();
        let x = s.overall_composition();
        assert_relative_eq!(x[0], 0.25, epsilon = 1e-15);
        assert_relative_eq!(x[1], 0.75, epsilon = 1e-15);

        for (c, w) in s
            .phase_mut(PhaseIndex::Mixture)
            .compounds
            .iter_mut()
            .zip([2.0, 2.0])
        {
            c.mass_fraction = Some(w);
        }
        s.normalize_overall_mass_composition();
        let w = s.overall_mass_composition();
        assert_relative_eq!(w[0], 0.5, epsilon = 1e-15);
        assert_relative_eq!(w[1], 0.5, epsilon = 1e-15);
    }

    /// **Methodology.** Per-phase composition set/get (MaterialStream.vb:1270,
    /// :1313) must write only the addressed slot. Sets the vapour slot to
    /// `[0.9, 0.1]` and checks the mixture slot is untouched.
    /// **Result (2026-08-11):** vapour = (0.900000, 0.100000); mixture unchanged
    /// at (0.500000, 0.500000).
    #[test]
    fn phase_composition_is_slot_local() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        s.set_phase_composition(PhaseIndex::Vapor, &[0.9, 0.1])
            .unwrap();
        assert_eq!(s.phase_composition(PhaseIndex::Vapor), vec![0.9, 0.1]);
        assert_eq!(s.overall_composition(), vec![0.5, 0.5]);
        assert_eq!(
            s.phase_composition(PhaseIndex::Liquid1),
            vec![0.0, 0.0],
            "untouched slots read as zero"
        );
    }

    /// **Methodology.** `Clear` (MaterialStream.vb:1089) must null the basic
    /// phase properties and compound state on every slot and drop
    /// `AtEquilibrium`, while leaving derived properties (here: density) alone.
    /// `DeepClear` (:1078) must additionally reset the derived properties.
    /// **Result (2026-08-11):** after `clear`, T/P/massflow are `None` and
    /// density survives at 997.000000 kg/m³; after `deep_clear`, density is
    /// `None` too.
    #[test]
    fn clear_and_deep_clear_have_the_documented_scopes() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        s.at_equilibrium = true;
        s.phase_mut(PhaseIndex::Mixture).properties.density = Some(997.0);

        s.clear();
        let p = &s.phase(PhaseIndex::Mixture).properties;
        assert_eq!(p.temperature, None);
        assert_eq!(p.pressure, None);
        assert_eq!(p.massflow, None);
        assert_eq!(p.density, Some(997.0), "Clear leaves derived props alone");
        assert!(!s.at_equilibrium);
        assert_eq!(
            s.phase(PhaseIndex::Mixture).compounds[0].mole_fraction,
            None
        );

        s.deep_clear();
        assert_eq!(s.phase(PhaseIndex::Mixture).properties.density, None);
        assert_eq!(s.compound_count(), 2, "compounds themselves survive");
    }

    /// **Methodology.** `Validate` (MaterialStream.vb:257-278) requires T, P, h
    /// and s to be present and finite, and reports the **first** failure in that
    /// order.
    /// **Result (2026-08-11):** a default stream (h, s unset) fails on
    /// `enthalpy`; with h and s set it passes; a NaN pressure fails on
    /// `pressure`.
    #[test]
    fn validate_checks_the_four_upstream_properties_in_order() {
        let mut s = MaterialStreamData::new();
        assert_eq!(
            s.validate("MS-1"),
            Err(StreamValidationError::InvalidSpecValue {
                tag: "MS-1".to_string(),
                property: "enthalpy"
            })
        );
        s.set_mass_enthalpy(AvailableEnergy::new::<kilojoule_per_kilogram>(120.0));
        s.set_mass_entropy(SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(
            0.4,
        ));
        assert_eq!(s.validate("MS-1"), Ok(()));

        s.phase_mut(PhaseIndex::Mixture).properties.pressure = Some(f64::NAN);
        assert_eq!(
            s.validate("MS-1"),
            Err(StreamValidationError::InvalidSpecValue {
                tag: "MS-1".to_string(),
                property: "pressure"
            })
        );
    }

    /// **Methodology.** `CheckDirtyStatus` (MaterialStream.vb:217-255): no
    /// snapshot means "not dirty"; a snapshot taken at the current state means
    /// "not dirty"; a change larger than `epsilon = 1e-6` in any tracked
    /// quantity means "dirty"; a change smaller than it does not.
    /// **Result (2026-08-11):** no snapshot -> false; fresh snapshot -> false;
    /// `T + 1e-3 K` -> true; `T + 1e-9 K` -> false; adding a compound -> true.
    #[test]
    fn dirty_check_reproduces_upstream_epsilon_semantics() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        assert!(!s.is_dirty_versus_last_solution(), "no snapshot yet");

        s.last_solution_input = Some(s.snapshot_input());
        assert!(!s.is_dirty_versus_last_solution());

        let t = s.temperature().unwrap().get::<kelvin>();
        s.set_temperature(ThermodynamicTemperature::new::<kelvin>(t + 1.0e-9));
        assert!(
            !s.is_dirty_versus_last_solution(),
            "sub-epsilon change is not dirty"
        );

        s.set_temperature(ThermodynamicTemperature::new::<kelvin>(t + 1.0e-3));
        assert!(s.is_dirty_versus_last_solution());

        s.set_temperature(ThermodynamicTemperature::new::<kelvin>(t));
        assert!(!s.is_dirty_versus_last_solution());
        s.add_compound("Nitrogen", 28.014);
        assert!(
            s.is_dirty_versus_last_solution(),
            "compound count change is dirty"
        );
    }

    /// **Methodology.** The `uom` accessors must convert DWSIM's kilo units
    /// exactly: a stored `enthalpy = 120 kJ/kg` must read as
    /// `120000 J/kg`; a stored `entropy = 0.4 kJ/(kg·K)` as `400 J/(kg·K)`; an
    /// energy stream's `50 kW` as `50000 W` (Streams.vb:114).
    /// **Result (2026-08-11):** 120000.000000 J/kg, 400.000000 J/(kg·K),
    /// 50000.000000 W; all round-trip back to the kilo values exactly.
    #[test]
    fn uom_accessors_convert_dwsim_kilo_units() {
        let mut s = MaterialStreamData::new();
        s.phase_mut(PhaseIndex::Mixture).properties.enthalpy = Some(120.0);
        s.phase_mut(PhaseIndex::Mixture).properties.entropy = Some(0.4);
        assert_relative_eq!(
            s.mass_enthalpy().unwrap().get::<joule_per_kilogram>(),
            120_000.0,
            epsilon = 1e-9
        );
        assert_relative_eq!(
            s.mass_entropy().unwrap().get::<joule_per_kilogram_kelvin>(),
            400.0,
            epsilon = 1e-9
        );
        s.set_mass_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(250_000.0));
        assert_relative_eq!(
            s.phase(PhaseIndex::Mixture).properties.enthalpy.unwrap(),
            250.0,
            epsilon = 1e-12
        );

        let mut e = EnergyStreamData::new();
        assert_eq!(e.power(), None);
        e.set_value_kw(50.0);
        assert_relative_eq!(e.power().unwrap().get::<watt>(), 50_000.0, epsilon = 1e-9);
        e.set_power(Power::new::<watt>(1_000.0));
        assert_relative_eq!(e.energy_flow.unwrap(), 1.0, epsilon = 1e-12);
        e.clear();
        assert_eq!(e.energy_flow, None);

        assert_relative_eq!(
            mass_enthalpy_from_kj_per_kg(3.0).get::<joule_per_kilogram>(),
            3000.0,
            epsilon = 1e-9
        );
        assert_relative_eq!(
            mass_enthalpy_to_kj_per_kg(AvailableEnergy::new::<joule_per_kilogram>(3000.0)),
            3.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(power_from_kw(2.0).get::<watt>(), 2000.0, epsilon = 1e-9);
    }

    /// **Methodology.** `CalcOverallMolecularWeight` (MaterialStream.vb:8585):
    /// `MW = sum_i x_i M_i`. With `x = [0.5, 0.5]`,
    /// `M = [18.015, 31.999] kg/kmol`: `MW = 25.007 kg/kmol`.
    /// **Result (2026-08-11):** MW = 25.007000 kg/kmol.
    #[test]
    fn mixture_molecular_weight() {
        let mut s = two_compound_stream();
        s.equalize_overall_composition();
        assert_relative_eq!(s.calc_overall_molecular_weight(), 25.007, epsilon = 1e-12);
    }

    /// **Methodology.** The molar-flow accessor uses `uom`'s katal (mol/s) via
    /// the [`MolarFlowRate`] alias; a stored `12.5 mol/s` must read back
    /// unchanged, and the volumetric-flow accessor must round-trip m³/s.
    /// **Result (2026-08-11):** 12.500000 mol/s and 0.030000 m³/s round-trip
    /// exactly.
    #[test]
    fn molar_and_volumetric_flow_accessors_round_trip() {
        let mut s = MaterialStreamData::new();
        s.set_molar_flow(MolarFlowRate::new::<katal>(12.5));
        assert_relative_eq!(
            s.molar_flow().unwrap().get::<katal>(),
            12.5,
            epsilon = 1e-12
        );
        s.set_volumetric_flow(VolumeRate::new::<cubic_meter_per_second>(0.03));
        assert_relative_eq!(
            s.volumetric_flow().unwrap().get::<cubic_meter_per_second>(),
            0.03,
            epsilon = 1e-15
        );
        s.set_vapor_fraction(Ratio::new::<ratio>(0.25));
        assert_relative_eq!(
            s.vapor_fraction().unwrap().get::<ratio>(),
            0.25,
            epsilon = 1e-15
        );
    }
}
