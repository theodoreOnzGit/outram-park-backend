//! Addressing and reading/writing a flowsheet object's properties by name, and
//! the display-unit ↔ internal-unit conversion the dynamics layer applies to
//! every value it stores.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! Sources:
//!
//! - `DWSIM.DynamicsManager/MonitoredVariable.vb:32-38` — `ObjectID`,
//!   `PropertyID`, `PropertyValue`, `PropertyUnits`: upstream addresses a
//!   property by **object ID + property-name string** and stores the value as a
//!   string with a separate unit string.
//! - `DWSIM.DynamicsManager/Event.vb:34-40` and
//!   `DWSIM.DynamicsManager/CauseAndEffectItem.vb:36-42` — the same
//!   `SimulationObjectID` / `SimulationObjectProperty` /
//!   `SimulationObjectPropertyValue` / `SimulationObjectPropertyUnits` quartet.
//! - `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:145`
//!   (`ConvertFromSI(units, sobj.GetPropertyValue(id))`), `:79-80` and `:113-114`
//!   (`ConvertToSI(units, value)` then `obj.SetPropertyValue(...)`).
//! - `DWSIM.SharedClasses/UnitsOfMeasure/SystemsOfUnits.vb:1335-1919`
//!   (`ConvertToSI`) and `:1925-2500+` (`ConvertFromSI`).
//!
//! # The two ends of the conversion
//!
//! Upstream calls its internal unit set "SI", but it is **SI-with-kilo** for
//! energy: enthalpy is kJ/kg, entropy kJ/(kg·K) and power kW (see
//! `SystemsOfUnits.vb:1567`, `Case "w": Return value / 1000`). The crate's
//! [`crate::flowsheet`] module stores exactly those internal units in its raw
//! `f64` fields and exposes strict-SI `uom` accessors on top. This module
//! therefore works in **DWSIM-internal units** — [`convert_to_internal`] is
//! upstream's `ConvertToSI`, [`convert_from_internal`] its `ConvertFromSI` — and
//! converts to and from `uom` at the flowsheet boundary in
//! [`property_value`]/[`set_property_value`].
//!
//! Internal units per property are listed by
//! [`DynamicProperty::internal_units`].
//!
//! # Excluded DWSIM behavior
//!
//! - **Property-grid reflection.** `ISimulationObject.GetPropertyValue` /
//!   `SetPropertyValue` / `GetProperties` resolve an arbitrary
//!   `"PROP_MS_0"`-style string against a reflected property list. The workspace
//!   forbids that style of stringly-typed dispatch, so the closed set of
//!   properties the dynamics layer can address is the [`DynamicProperty`] enum —
//!   a missing arm is a compile error rather than a silent `Nothing`. The one
//!   open-ended case, a named unit-operation result, is kept as
//!   [`DynamicProperty::UnitOperationResult`].
//! - **The full unit table.** `ConvertToSI` handles ~400 unit strings across
//!   every property DWSIM knows (viscosity, thermal conductivity, surface
//!   tension, molar volume, fouling factor, …). This port implements the subset
//!   reachable from the properties [`DynamicProperty`] exposes; every other
//!   string falls through to upstream's own `Case Else: Return value` identity
//!   (`SystemsOfUnits.vb:1916-1917`), so an unsupported unit is passed through
//!   unchanged exactly as upstream would pass through an unrecognised one.
//!   **This is a real limitation:** feeding `"cP"` to a mass-flow event silently
//!   does nothing, in this port and in DWSIM alike.
//! - **`IUnitsOfMeasure` display-system selection** (`FlowsheetOptions.SelectedUnitSystem`)
//!   — the dynamics objects carry their own per-value unit string, which is what
//!   this port keeps.
//! - **Dirty-flag propagation.** Upstream's `SetPropertyValue` may mark an object
//!   for recalculation. This port does not touch
//!   [`crate::flowsheet::objects::FlowsheetObject::dirty`]: the run loop solves
//!   the **whole** flowsheet every step (FormDynamicsIntegratorControl.vb:476-480),
//!   so nothing depends on the flag.

use std::fmt;

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    AvailableEnergy, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity,
    ThermodynamicTemperature, VolumeRate,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume_rate::cubic_meter_per_second;

use crate::dynamics::errors::DynamicsError;
use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::{ObjectData, ObjectId};
use crate::flowsheet::streams::MolarFlowRate;

/// A flowsheet property the dynamics layer can read or write.
///
/// This is the typed replacement for upstream's `SimulationObjectProperty`
/// string (Event.vb:36, CauseAndEffectItem.vb:38, MonitoredVariable.vb:34). The
/// set is closed and dispatched by `match`, per the workspace's no-trait-object
/// rule.
///
/// Every variant names a physical quantity; the units each one is stored and
/// exchanged in are given by [`DynamicProperty::internal_units`] and are
/// **DWSIM-internal** units (SI-with-kilo for energy).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DynamicProperty {
    /// Material-stream temperature \[K\].
    Temperature,
    /// Material-stream pressure \[Pa\].
    Pressure,
    /// Material-stream total mass flow \[kg/s\].
    MassFlow,
    /// Material-stream total molar flow \[mol/s\].
    MolarFlow,
    /// Material-stream total volumetric flow \[m³/s\].
    VolumetricFlow,
    /// Material-stream overall specific enthalpy \[kJ/kg\] (DWSIM-internal
    /// units; the `uom` accessor is strict SI J/kg).
    MassEnthalpy,
    /// Material-stream overall specific entropy \[kJ/(kg·K)\] (DWSIM-internal
    /// units).
    MassEntropy,
    /// Material-stream vapour (molar) fraction \[-\], 0 to 1.
    VaporFraction,
    /// Energy flow \[kW\] — an energy stream's power, or a unit operation's net
    /// power generated (`> 0`) / consumed (`< 0`).
    EnergyFlow,
    /// Overall mole fraction \[-\] of one compound. Needs a compound index.
    OverallMoleFraction,
    /// Overall mass fraction \[-\] of one compound. Needs a compound index.
    OverallMassFraction,
    /// A named scalar result attached to a unit operation
    /// ([`ObjectData::UnitOperation`]'s `results` map), in SI base units. This is
    /// the one open-ended case kept from upstream's stringly-typed property
    /// access, because equipment results are genuinely open-ended.
    UnitOperationResult(String),
}

impl fmt::Display for DynamicProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynamicProperty::Temperature => write!(f, "Temperature"),
            DynamicProperty::Pressure => write!(f, "Pressure"),
            DynamicProperty::MassFlow => write!(f, "MassFlow"),
            DynamicProperty::MolarFlow => write!(f, "MolarFlow"),
            DynamicProperty::VolumetricFlow => write!(f, "VolumetricFlow"),
            DynamicProperty::MassEnthalpy => write!(f, "MassEnthalpy"),
            DynamicProperty::MassEntropy => write!(f, "MassEntropy"),
            DynamicProperty::VaporFraction => write!(f, "VaporFraction"),
            DynamicProperty::EnergyFlow => write!(f, "EnergyFlow"),
            DynamicProperty::OverallMoleFraction => write!(f, "OverallMoleFraction"),
            DynamicProperty::OverallMassFraction => write!(f, "OverallMassFraction"),
            DynamicProperty::UnitOperationResult(name) => write!(f, "Result[{name}]"),
        }
    }
}

impl DynamicProperty {
    /// The unit string this property is stored in internally — DWSIM's "SI",
    /// which is SI-with-kilo for energy quantities.
    ///
    /// Feeding this string to [`convert_to_internal`] is always the identity.
    #[must_use]
    pub fn internal_units(&self) -> &'static str {
        match self {
            DynamicProperty::Temperature => "K",
            DynamicProperty::Pressure => "Pa",
            DynamicProperty::MassFlow => "kg/s",
            DynamicProperty::MolarFlow => "mol/s",
            DynamicProperty::VolumetricFlow => "m3/s",
            DynamicProperty::MassEnthalpy => "kJ/kg",
            DynamicProperty::MassEntropy => "kJ/[kg.K]",
            DynamicProperty::EnergyFlow => "kW",
            DynamicProperty::VaporFraction
            | DynamicProperty::OverallMoleFraction
            | DynamicProperty::OverallMassFraction
            | DynamicProperty::UnitOperationResult(_) => "",
        }
    }

    /// `true` if this property addresses one compound of a mixture and therefore
    /// needs [`PropertyRef::compound_index`].
    #[must_use]
    pub fn needs_compound_index(&self) -> bool {
        matches!(
            self,
            DynamicProperty::OverallMoleFraction | DynamicProperty::OverallMassFraction
        )
    }
}

/// A fully-qualified reference to one scalar on the flowsheet: which object,
/// which property, and — for a composition — which compound.
///
/// Ports upstream's `(SimulationObjectID, SimulationObjectProperty)` pair
/// (Event.vb:34-36, CauseAndEffectItem.vb:36-38, MonitoredVariable.vb:32-34),
/// with the compound index broken out instead of being encoded in the property
/// string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PropertyRef {
    /// The flowsheet object that owns the property (upstream's
    /// `SimulationObjectID` / `ObjectID` — the immutable ID, not the tag).
    pub object: ObjectId,
    /// Which property of that object.
    pub property: DynamicProperty,
    /// For a composition property, which compound (index into the stream's
    /// compound list). `None` for scalar properties.
    pub compound_index: Option<usize>,
}

impl Default for PropertyRef {
    /// An **unbound** reference: empty object ID,
    /// [`DynamicProperty::Temperature`], no compound index.
    ///
    /// This mirrors upstream, whose `SimulationObjectID` and
    /// `SimulationObjectProperty` both default to `""` (Event.vb:34-36,
    /// CauseAndEffectItem.vb:36-38, MonitoredVariable.vb:32-34) — a
    /// freshly-constructed event or matrix row points at nothing until the user
    /// configures it. Reading through a default reference always fails with
    /// [`DynamicsError::ObjectNotFound`].
    fn default() -> Self {
        PropertyRef {
            object: ObjectId(String::new()),
            property: DynamicProperty::Temperature,
            compound_index: None,
        }
    }
}

impl PropertyRef {
    /// Reference a scalar property of an object.
    #[must_use]
    pub fn new(object: ObjectId, property: DynamicProperty) -> Self {
        PropertyRef {
            object,
            property,
            compound_index: None,
        }
    }

    /// Reference one compound's fraction in a stream's overall composition.
    #[must_use]
    pub fn with_compound(object: ObjectId, property: DynamicProperty, index: usize) -> Self {
        PropertyRef {
            object,
            property,
            compound_index: Some(index),
        }
    }

    fn compound_index_within(&self, count: usize) -> Result<usize, DynamicsError> {
        match self.compound_index {
            Some(i) if i < count => Ok(i),
            given => Err(DynamicsError::CompoundIndex {
                object: self.object.0.clone(),
                property: self.property.to_string(),
                given,
                count,
            }),
        }
    }

    fn unavailable(&self) -> DynamicsError {
        DynamicsError::PropertyNotAvailable {
            object: self.object.0.clone(),
            property: self.property.to_string(),
        }
    }
}

/// Read a property off the flowsheet, in **DWSIM-internal units**
/// ([`DynamicProperty::internal_units`]).
///
/// This is the port of `sobj.GetPropertyValue(PropertyID)` as the dynamics layer
/// uses it (FormDynamicsIntegratorControl.vb:145, Manager.vb:285,
/// AnalogGauge.vb:111).
///
/// # Errors
///
/// - [`DynamicsError::ObjectNotFound`] if the object is not on the flowsheet
///   (upstream: `KeyNotFoundException`).
/// - [`DynamicsError::PropertyNotAvailable`] if the object does not carry that
///   property, or carries it but has never had it set (upstream returns
///   `Nothing` and the caller's `Convert.ToDouble` throws).
/// - [`DynamicsError::CompoundIndex`] if a composition property is addressed
///   without a valid compound index.
pub fn property_value(
    flowsheet: &Flowsheet,
    reference: &PropertyRef,
) -> Result<f64, DynamicsError> {
    let object = flowsheet
        .object(&reference.object)
        .ok_or_else(|| DynamicsError::ObjectNotFound(reference.object.0.clone()))?;

    match &object.data {
        ObjectData::Material(stream) => match &reference.property {
            DynamicProperty::Temperature => stream
                .temperature()
                .map(|t| t.get::<kelvin>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::Pressure => stream
                .pressure()
                .map(|p| p.get::<pascal>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::MassFlow => stream
                .mass_flow()
                .map(|w| w.get::<kilogram_per_second>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::MolarFlow => stream
                .molar_flow()
                .map(|n| n.get::<katal>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::VolumetricFlow => stream
                .volumetric_flow()
                .map(|q| q.get::<cubic_meter_per_second>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::MassEnthalpy => stream
                .mass_enthalpy()
                .map(|h| h.get::<kilojoule_per_kilogram>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::MassEntropy => stream
                .mass_entropy()
                .map(|s| s.get::<kilojoule_per_kilogram_kelvin>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::VaporFraction => stream
                .vapor_fraction()
                .map(|b| b.get::<ratio>())
                .ok_or_else(|| reference.unavailable()),
            DynamicProperty::OverallMoleFraction => {
                let x = stream.overall_composition();
                Ok(x[reference.compound_index_within(x.len())?])
            }
            DynamicProperty::OverallMassFraction => {
                let w = stream.overall_mass_composition();
                Ok(w[reference.compound_index_within(w.len())?])
            }
            DynamicProperty::EnergyFlow | DynamicProperty::UnitOperationResult(_) => {
                Err(reference.unavailable())
            }
        },
        ObjectData::Energy(stream) => match &reference.property {
            DynamicProperty::EnergyFlow => stream
                .power()
                .map(|p| p.get::<kilowatt>())
                .ok_or_else(|| reference.unavailable()),
            _ => Err(reference.unavailable()),
        },
        ObjectData::UnitOperation { power, results } => match &reference.property {
            DynamicProperty::EnergyFlow => power.ok_or_else(|| reference.unavailable()),
            DynamicProperty::UnitOperationResult(name) => results
                .get(name)
                .copied()
                .ok_or_else(|| reference.unavailable()),
            _ => Err(reference.unavailable()),
        },
    }
}

/// Write a property onto the flowsheet, taking `value` in **DWSIM-internal
/// units** ([`DynamicProperty::internal_units`]).
///
/// This is the port of `obj.SetPropertyValue(property, value)` as the dynamics
/// layer uses it (FormDynamicsIntegratorControl.vb:71, :80, :114).
///
/// # Errors
///
/// Same set as [`property_value`]. Writing a composition fraction rewrites the
/// whole overall-composition vector with that one element replaced; the stream's
/// other fractions are left as they were, so the result is **not renormalised**
/// — exactly like assigning one array element upstream.
pub fn set_property_value(
    flowsheet: &mut Flowsheet,
    reference: &PropertyRef,
    value: f64,
) -> Result<(), DynamicsError> {
    // Resolve the compound index against the current composition before taking a
    // mutable borrow, so the error carries the true compound count.
    let object = flowsheet
        .object(&reference.object)
        .ok_or_else(|| DynamicsError::ObjectNotFound(reference.object.0.clone()))?;
    let compound_slot = match (&object.data, &reference.property) {
        (
            ObjectData::Material(stream),
            DynamicProperty::OverallMoleFraction | DynamicProperty::OverallMassFraction,
        ) => Some(reference.compound_index_within(stream.compound_count())?),
        _ => None,
    };

    let object = flowsheet
        .object_mut(&reference.object)
        .ok_or_else(|| DynamicsError::ObjectNotFound(reference.object.0.clone()))?;

    match &mut object.data {
        ObjectData::Material(stream) => match &reference.property {
            DynamicProperty::Temperature => {
                stream.set_temperature(ThermodynamicTemperature::new::<kelvin>(value));
                Ok(())
            }
            DynamicProperty::Pressure => {
                stream.set_pressure(Pressure::new::<pascal>(value));
                Ok(())
            }
            DynamicProperty::MassFlow => {
                stream.set_mass_flow(MassRate::new::<kilogram_per_second>(value));
                Ok(())
            }
            DynamicProperty::MolarFlow => {
                stream.set_molar_flow(MolarFlowRate::new::<katal>(value));
                Ok(())
            }
            DynamicProperty::VolumetricFlow => {
                stream.set_volumetric_flow(VolumeRate::new::<cubic_meter_per_second>(value));
                Ok(())
            }
            DynamicProperty::MassEnthalpy => {
                stream.set_mass_enthalpy(AvailableEnergy::new::<kilojoule_per_kilogram>(value));
                Ok(())
            }
            DynamicProperty::MassEntropy => {
                stream.set_mass_entropy(
                    SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(value),
                );
                Ok(())
            }
            DynamicProperty::VaporFraction => {
                stream.set_vapor_fraction(Ratio::new::<ratio>(value));
                Ok(())
            }
            DynamicProperty::OverallMoleFraction => {
                let mut x = stream.overall_composition();
                x[compound_slot.expect("resolved above for a composition property")] = value;
                stream
                    .set_overall_molar_composition(&x)
                    .map_err(|_| reference.unavailable())
            }
            DynamicProperty::OverallMassFraction => {
                let mut w = stream.overall_mass_composition();
                w[compound_slot.expect("resolved above for a composition property")] = value;
                stream
                    .set_overall_mass_composition(&w)
                    .map_err(|_| reference.unavailable())
            }
            DynamicProperty::EnergyFlow | DynamicProperty::UnitOperationResult(_) => {
                Err(reference.unavailable())
            }
        },
        ObjectData::Energy(stream) => match &reference.property {
            DynamicProperty::EnergyFlow => {
                stream.set_power(Power::new::<kilowatt>(value));
                Ok(())
            }
            _ => Err(reference.unavailable()),
        },
        ObjectData::UnitOperation { power, results } => match &reference.property {
            DynamicProperty::EnergyFlow => {
                *power = Some(value);
                Ok(())
            }
            DynamicProperty::UnitOperationResult(name) => {
                results.insert(name.clone(), value);
                Ok(())
            }
            _ => Err(reference.unavailable()),
        },
    }
}

/// Convert a value expressed in the display unit `units` into DWSIM's internal
/// units.
///
/// Port of `SystemsOfUnits.Converter.ConvertToSI` (SystemsOfUnits.vb:1335-1919),
/// restricted to the units reachable from [`DynamicProperty`]. Matching is
/// case-insensitive, exactly as upstream's `Select Case units.ToLower()`
/// (:1337), and **any unrecognised string returns the value unchanged**, which
/// is upstream's own `Case Else` (:1916-1917).
///
/// Each arm below cites the upstream line it reproduces.
#[must_use]
pub fn convert_to_internal(units: &str, value: f64) -> f64 {
    match units.to_lowercase().as_str() {
        // Temperature → K (SystemsOfUnits.vb:1752-1753, :1797-1798, :1870-1875)
        "k" => value,
        "c" | "°c" => value + 273.15,
        "f" | "°f" => (value - 32.0) * 5.0 / 9.0 + 273.15,
        "r" => value / 1.8,

        // Pressure → Pa (:1391-1420, :1757-1758, :1831-1832)
        "pa" => value,
        "kpa" => value / 0.001,
        "mpa" => value / 0.000_001,
        "bar" => value * 100_000.0,
        "mbar" => value / 0.01,
        "atm" => value * 101_325.0,
        "psi" | "psia" => value / 0.000_145_038,
        "mmhg" => value / 0.007_500_64,
        "kgf/cm2" => value * 101_325.0 / 1.033,

        // Mass flow → kg/s (:1424-1437, :1759-1760, :1803-1804, :1835-1838)
        "kg/s" => value,
        "kg/h" => value / 3600.0,
        "kg/d" => value / 3600.0 / 24.0,
        "kg/min" => value / 60.0,
        "g/s" => value / 1000.0,
        "lb/h" | "lbm/h" | "lb/hr" | "lbm/hr" => value / 7936.64,
        "lb/s" => value / 2.204_62,
        "t/h" => value * 1000.0 / 60.0 / 60.0,

        // Molar flow → mol/s (:1490-1499, :1761-1764, :1805-1806)
        "mol/s" => value,
        "mol/h" => value / 3600.0,
        "mol/d" => value / 3600.0 / 24.0,
        "kmol/s" => value * 1000.0,
        "kmol/h" => value * 1000.0 / 3600.0,
        "kmol/d" => value * 1000.0 / 3600.0 / 24.0,
        "lbmol/h" => value * 453.592_37 / 3600.0,

        // Volumetric flow → m3/s (:1520-1546, :1839-1842)
        "m3/s" => value,
        "m3/h" => value / 3600.0,
        "m3/d" => value / 3600.0 / 24.0,
        "l/s" => value / 1000.0,
        "l/min" => value / 60_000.0,
        "l/h" => value / 3_600_000.0,
        "ft3/s" => value / 35.3147,
        "ft3/min" => value / 35.3147 / 60.0,
        "ft3/h" => value / 35.3147 / 60.0 / 60.0,

        // Specific enthalpy → kJ/kg (:1589-1594)
        "kj/kg" => value,
        "cal/g" | "kcal/kg" => value / 0.238_846,
        "btu/lb" | "btu/lbm" => value / 0.429_923,

        // Specific entropy → kJ/(kg.K) (:1817-1818, mirrored for the mass basis)
        "kj/[kg.k]" | "kj/kg.k" => value,
        "cal/[g.c]" | "cal/[g.°c]" => value / 0.238_846,
        "btu/[lbm.r]" => value / 0.238_846,

        // Power / energy flow → kW (:1549-1572)
        "kw" => value,
        "w" => value / 1000.0,
        "mw" => value / 0.001,
        "btu/h" | "btu/hr" => value / 3412.14,
        "btu/s" => value / 0.947_817,
        "cal/s" => value / 238.846,
        "kcal/h" | "kcal/hr" => value / 859.845,
        "kj/h" => value / 3600.0,
        "hp" => value / 1.359_62,

        // SystemsOfUnits.vb:1916-1917 — Case Else: Return value
        _ => value,
    }
}

/// Convert a value in DWSIM's internal units into the display unit `units`.
///
/// Port of `SystemsOfUnits.Converter.ConvertFromSI` (SystemsOfUnits.vb:1925
/// onwards), restricted to the same subset as [`convert_to_internal`] and, like
/// it, case-insensitive with an identity fallback.
///
/// **Provenance of the constants.** The arms for `c`, `f`, `r`, `atm`, `kpa`,
/// `bar`, `psi`, `kg/h`, `kg/d`, `m3/h`, `mol/h`, `kmol/h`, `w`, `mw` and
/// `cal/g` were read directly from upstream's `ConvertFromSI` (`:1979-1982`,
/// `:2001-2004`, `:2015`, `:2116-2117`, `:2122-2123`, `:2158-2159`, `:2352-2353`,
/// `:2390-2391`, `:2424-2429`, `:2459-2461`) and match exactly. The remaining
/// arms are the exact algebraic inverses of [`convert_to_internal`]; they were
/// **not** individually diffed against upstream, so treat them as verified by
/// round-trip ([`convert_to_internal`] ∘ `convert_from_internal` = identity),
/// not by transcription.
#[must_use]
pub fn convert_from_internal(units: &str, value: f64) -> f64 {
    match units.to_lowercase().as_str() {
        // Temperature
        "k" => value,
        "c" | "°c" => value - 273.15,
        "f" | "°f" => (value - 273.15) * 9.0 / 5.0 + 32.0,
        "r" => value * 1.8,

        // Pressure
        "pa" => value,
        "kpa" => value * 0.001,
        "mpa" => value * 0.000_001,
        "bar" => value / 100_000.0,
        "mbar" => value * 0.01,
        "atm" => value / 101_325.0,
        "psi" | "psia" => value * 0.000_145_038,
        "mmhg" => value * 0.007_500_64,
        "kgf/cm2" => value * 1.033 / 101_325.0,

        // Mass flow
        "kg/s" => value,
        "kg/h" => value * 3600.0,
        "kg/d" => value * 3600.0 * 24.0,
        "kg/min" => value * 60.0,
        "g/s" => value * 1000.0,
        "lb/h" | "lbm/h" | "lb/hr" | "lbm/hr" => value * 7936.64,
        "lb/s" => value * 2.204_62,
        "t/h" => value / 1000.0 * 60.0 * 60.0,

        // Molar flow
        "mol/s" => value,
        "mol/h" => value * 3600.0,
        "mol/d" => value * 3600.0 * 24.0,
        "kmol/s" => value / 1000.0,
        "kmol/h" => value / 1000.0 * 3600.0,
        "kmol/d" => value / 1000.0 * 3600.0 * 24.0,
        "lbmol/h" => value / 453.592_37 * 3600.0,

        // Volumetric flow
        "m3/s" => value,
        "m3/h" => value * 3600.0,
        "m3/d" => value * 3600.0 * 24.0,
        "l/s" => value * 1000.0,
        "l/min" => value * 60_000.0,
        "l/h" => value * 3_600_000.0,
        "ft3/s" => value * 35.3147,
        "ft3/min" => value * 35.3147 * 60.0,
        "ft3/h" => value * 35.3147 * 60.0 * 60.0,

        // Specific enthalpy
        "kj/kg" => value,
        "cal/g" | "kcal/kg" => value * 0.238_846,
        "btu/lb" | "btu/lbm" => value * 0.429_923,

        // Specific entropy
        "kj/[kg.k]" | "kj/kg.k" => value,
        "cal/[g.c]" | "cal/[g.°c]" => value * 0.238_846,
        "btu/[lbm.r]" => value * 0.238_846,

        // Power / energy flow
        "kw" => value,
        "w" => value * 1000.0,
        "mw" => value * 0.001,
        "btu/h" | "btu/hr" => value * 3412.14,
        "btu/s" => value * 0.947_817,
        "cal/s" => value * 238.846,
        "kcal/h" | "kcal/hr" => value * 859.845,
        "kj/h" => value * 3600.0,
        "hp" => value * 1.359_62,

        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowsheet::objects::ObjectType;

    fn stream_flowsheet() -> (Flowsheet, ObjectId) {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        (fs, id)
    }

    #[test]
    fn reads_the_material_stream_defaults_in_internal_units() {
        let (fs, id) = stream_flowsheet();
        let t = property_value(
            &fs,
            &PropertyRef::new(id.clone(), DynamicProperty::Temperature),
        )
        .unwrap();
        let p = property_value(
            &fs,
            &PropertyRef::new(id.clone(), DynamicProperty::Pressure),
        )
        .unwrap();
        let w = property_value(&fs, &PropertyRef::new(id, DynamicProperty::MassFlow)).unwrap();
        assert!((t - 298.15).abs() < 1e-9);
        assert!((p - 101_325.0).abs() < 1e-6);
        assert!((w - 1.0).abs() < 1e-12);
    }

    #[test]
    fn writes_then_reads_back_a_scalar_property() {
        let (mut fs, id) = stream_flowsheet();
        let r = PropertyRef::new(id, DynamicProperty::Pressure);
        set_property_value(&mut fs, &r, 2.5e5).unwrap();
        assert!((property_value(&fs, &r).unwrap() - 2.5e5).abs() < 1e-6);
    }

    #[test]
    fn composition_needs_a_valid_compound_index() {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        if let Some(stream) = fs.object_mut(&id).unwrap().data.as_material_mut() {
            stream.add_compound("water", 18.015);
            stream.add_compound("ethanol", 46.07);
            stream.set_overall_molar_composition(&[0.6, 0.4]).unwrap();
        }
        let ok = PropertyRef::with_compound(id.clone(), DynamicProperty::OverallMoleFraction, 1);
        assert!((property_value(&fs, &ok).unwrap() - 0.4).abs() < 1e-12);

        let missing = PropertyRef::new(id.clone(), DynamicProperty::OverallMoleFraction);
        assert!(matches!(
            property_value(&fs, &missing),
            Err(DynamicsError::CompoundIndex { .. })
        ));
        let past_end = PropertyRef::with_compound(id, DynamicProperty::OverallMoleFraction, 7);
        assert!(matches!(
            property_value(&fs, &past_end),
            Err(DynamicsError::CompoundIndex { count: 2, .. })
        ));
    }

    #[test]
    fn energy_stream_power_is_addressed_in_kilowatts() {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::EnergyStream, Some("E-1"));
        let r = PropertyRef::new(id, DynamicProperty::EnergyFlow);
        // Unset upstream reads as Nothing; here it is an explicit error.
        assert!(matches!(
            property_value(&fs, &r),
            Err(DynamicsError::PropertyNotAvailable { .. })
        ));
        set_property_value(&mut fs, &r, 250.0).unwrap();
        assert!((property_value(&fs, &r).unwrap() - 250.0).abs() < 1e-9);
    }

    #[test]
    fn unit_operation_results_are_addressable_by_name() {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::Pump, Some("P-1"));
        let r = PropertyRef::new(
            id,
            DynamicProperty::UnitOperationResult("Efficiency".to_string()),
        );
        set_property_value(&mut fs, &r, 0.72).unwrap();
        assert!((property_value(&fs, &r).unwrap() - 0.72).abs() < 1e-12);
    }

    #[test]
    fn missing_object_is_reported_not_panicked() {
        let fs = Flowsheet::new();
        let r = PropertyRef::new(ObjectId::from("nope"), DynamicProperty::Temperature);
        assert!(matches!(
            property_value(&fs, &r),
            Err(DynamicsError::ObjectNotFound(_))
        ));
    }

    #[test]
    fn unit_conversion_matches_upstream_constants() {
        // SystemsOfUnits.vb:1752-1753 / :1870-1875 / :1797-1798
        assert!((convert_to_internal("C", 25.0) - 298.15).abs() < 1e-9);
        assert!((convert_to_internal("F", 212.0) - 373.15).abs() < 1e-9);
        assert!((convert_to_internal("R", 491.67) - 273.15).abs() < 1e-6);
        // :1413-1414 bar, :1757-1758 atm, :1391-1392 kPa
        assert!((convert_to_internal("bar", 2.0) - 200_000.0).abs() < 1e-6);
        assert!((convert_to_internal("atm", 1.0) - 101_325.0).abs() < 1e-6);
        assert!((convert_to_internal("kPa", 100.0) - 100_000.0).abs() < 1e-6);
        // :1835-1836 kg/h, :1496-1497 kmol/h, :1567-1568 W
        assert!((convert_to_internal("kg/h", 3600.0) - 1.0).abs() < 1e-12);
        assert!((convert_to_internal("kmol/h", 3.6) - 1.0).abs() < 1e-12);
        assert!((convert_to_internal("W", 1000.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn unknown_units_pass_through_exactly_as_upstreams_case_else() {
        // SystemsOfUnits.vb:1916-1917
        assert!((convert_to_internal("furlongs/fortnight", 3.5) - 3.5).abs() < 1e-12);
        assert!((convert_to_internal("", 3.5) - 3.5).abs() < 1e-12);
        assert!((convert_from_internal("furlongs/fortnight", 3.5) - 3.5).abs() < 1e-12);
    }

    #[test]
    fn conversions_round_trip() {
        for unit in [
            "C", "F", "R", "bar", "atm", "psi", "kgf/cm2", "kg/h", "lb/h", "kmol/h", "m3/h", "l/s",
            "cal/g", "btu/lb", "W", "MW", "hp", "BTU/h",
        ] {
            let internal = 1234.5_f64;
            let display = convert_from_internal(unit, internal);
            let back = convert_to_internal(unit, display);
            assert!(
                (back - internal).abs() < 1e-6 * internal.abs().max(1.0),
                "round trip failed for {unit}: {internal} -> {display} -> {back}"
            );
        }
    }

    #[test]
    fn internal_units_strings_are_conversion_identities() {
        for property in [
            DynamicProperty::Temperature,
            DynamicProperty::Pressure,
            DynamicProperty::MassFlow,
            DynamicProperty::MolarFlow,
            DynamicProperty::VolumetricFlow,
            DynamicProperty::MassEnthalpy,
            DynamicProperty::MassEntropy,
            DynamicProperty::EnergyFlow,
            DynamicProperty::VaporFraction,
        ] {
            let u = property.internal_units();
            assert!(
                (convert_to_internal(u, 42.0) - 42.0).abs() < 1e-12,
                "{property} internal unit '{u}' is not an identity"
            );
        }
    }
}
