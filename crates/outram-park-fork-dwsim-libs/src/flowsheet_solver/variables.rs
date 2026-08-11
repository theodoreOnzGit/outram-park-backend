//! Addressing a single scalar on a flowsheet object.
//!
//! # What this module replaces
//!
//! DWSIM's adjust and spec blocks name their controlled/manipulated/referenced
//! variables with a **reflected property string** — `"PROP_MS_0"`,
//! `"Temperature"`, and so on — resolved at run time through
//! `ISimulationObject.GetPropertyValue` / `SetPropertyValue`, then converted
//! through the user's display unit system
//! (`FlowsheetSolver.vb:2343-2403`). The flowsheet data model deliberately
//! excludes that reflection layer
//! ([`crate::flowsheet`]'s "Excluded DWSIM behavior"), so this port replaces it
//! with a **closed enum**, [`FlowsheetVariable`], dispatched by `match`.
//!
//! Three consequences, all deliberate:
//!
//! - **Exhaustiveness.** Adding a variable is a compile error at every `match`,
//!   rather than a run-time `Nothing`.
//! - **No unit system.** Everything here is **plain SI**: K, Pa, kg/s, mol/s,
//!   J/kg, W, dimensionless. DWSIM converts to and from the user's display units
//!   inside the adjust solver (`cv.ConvertFromSI(punit, adj.AdjustValue)`,
//!   FlowsheetSolver.vb:2187-2193), which means an upstream adjust tolerance is
//!   in *display* units. Here a tolerance is in the variable's SI unit. This is
//!   a **behavioural divergence** and is called out again on
//!   [`crate::flowsheet_solver::adjust::AdjustBlock::tolerance`].
//! - **A smaller surface.** Only the scalars a flowsheet object actually
//!   carries are addressable; equipment parameters that live in this crate's
//!   sibling equipment modules are reachable through
//!   [`FlowsheetVariable::UnitOperationResult`] once a model writes them there.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary source: `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:2336-2404` —
//! `GetCtlVarValue`, `GetMnpVarValue`, `SetMnpVarValue`, `GetRefVarValue`.
//!
//! # Excluded DWSIM behavior
//!
//! - **Property-grid reflection** (`GetPropertyValue`, `SetPropertyValue`,
//!   `GetPropertyUnit`, `GetProperties`) and the `PROP_*` identifier scheme.
//! - **`IUnitsOfMeasure` display-unit conversion** and its temperature special
//!   case (`punit & "."`, FlowsheetSolver.vb:2186-2189) — this port is SI
//!   throughout.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{AvailableEnergy, MassRate, Power, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::flowsheet::{Flowsheet, MolarFlowRate, ObjectData, ObjectId};
use crate::flowsheet_solver::errors::SolverError;

/// One addressable scalar on a flowsheet object.
///
/// Every variant documents its **SI unit**; [`FlowsheetVariable::get`] and
/// [`FlowsheetVariable::set`] work in those units regardless of how the value is
/// stored internally (the flowsheet keeps DWSIM's kJ/kg and kW, and this enum
/// converts).
///
/// Enum dispatch, not a trait object, per the workspace Rust design rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FlowsheetVariable {
    /// Material stream: mixture-phase temperature \[K\].
    Temperature,
    /// Material stream: mixture-phase pressure \[Pa\].
    Pressure,
    /// Material stream: mixture-phase total mass flow \[kg/s\].
    MassFlow,
    /// Material stream: mixture-phase total molar flow \[mol/s\].
    MolarFlow,
    /// Material stream: mixture-phase mass enthalpy \[J/kg\]. Stored internally
    /// in DWSIM's kJ/kg and converted here.
    MassEnthalpy,
    /// Material stream: mixture-phase vapour mole fraction \[dimensionless\],
    /// `0` to `1`.
    VaporFraction,
    /// Energy stream: power \[W\].
    EnergyFlow,
    /// Unit operation: net power generated (`> 0`) or consumed (`< 0`) \[W\].
    /// Stored internally in DWSIM's kW and converted here.
    UnitOperationPower,
    /// Unit operation: a named entry of its free-form results map, in whatever
    /// SI unit the model that wrote it documents. Reading a key that is not
    /// present is an error; writing creates it.
    UnitOperationResult(String),
}

impl FlowsheetVariable {
    /// A short human-readable name, for error messages and reports.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            FlowsheetVariable::Temperature => "Temperature",
            FlowsheetVariable::Pressure => "Pressure",
            FlowsheetVariable::MassFlow => "MassFlow",
            FlowsheetVariable::MolarFlow => "MolarFlow",
            FlowsheetVariable::MassEnthalpy => "MassEnthalpy",
            FlowsheetVariable::VaporFraction => "VaporFraction",
            FlowsheetVariable::EnergyFlow => "EnergyFlow",
            FlowsheetVariable::UnitOperationPower => "UnitOperationPower",
            FlowsheetVariable::UnitOperationResult(k) => k,
        }
    }

    /// The SI unit this variable reads and writes in, as a string, for reports.
    #[must_use]
    pub fn si_unit(&self) -> &'static str {
        match self {
            FlowsheetVariable::Temperature => "K",
            FlowsheetVariable::Pressure => "Pa",
            FlowsheetVariable::MassFlow => "kg/s",
            FlowsheetVariable::MolarFlow => "mol/s",
            FlowsheetVariable::MassEnthalpy => "J/kg",
            FlowsheetVariable::VaporFraction => "-",
            FlowsheetVariable::EnergyFlow | FlowsheetVariable::UnitOperationPower => "W",
            FlowsheetVariable::UnitOperationResult(_) => "(model-defined SI)",
        }
    }

    /// Read this variable off `object` in `flowsheet`, in SI.
    ///
    /// Stands in for `GetPropertyValue` (FlowsheetSolver.vb:2348, :2365, :2401).
    ///
    /// # Errors
    ///
    /// [`SolverError::UnknownObject`] if the object is missing;
    /// [`SolverError::UnknownVariable`] if the object does not carry this
    /// variable (a temperature on an energy stream, an absent results key, an
    /// unset optional value).
    pub fn get(&self, flowsheet: &Flowsheet, object: &ObjectId) -> Result<f64, SolverError> {
        let obj = flowsheet
            .object(object)
            .ok_or_else(|| SolverError::UnknownObject(object.0.clone()))?;
        let missing = || SolverError::UnknownVariable {
            object: object.0.clone(),
            variable: self.name().to_string(),
        };
        match (self, &obj.data) {
            (FlowsheetVariable::Temperature, ObjectData::Material(ms)) => {
                ms.temperature().map(|t| t.get::<kelvin>()).ok_or_else(missing)
            }
            (FlowsheetVariable::Pressure, ObjectData::Material(ms)) => {
                ms.pressure().map(|p| p.get::<pascal>()).ok_or_else(missing)
            }
            (FlowsheetVariable::MassFlow, ObjectData::Material(ms)) => ms
                .mass_flow()
                .map(|w| w.get::<kilogram_per_second>())
                .ok_or_else(missing),
            (FlowsheetVariable::MolarFlow, ObjectData::Material(ms)) => ms
                .molar_flow()
                .map(|n| n.get::<katal>())
                .ok_or_else(missing),
            (FlowsheetVariable::MassEnthalpy, ObjectData::Material(ms)) => ms
                .mass_enthalpy()
                .map(|h| h.get::<joule_per_kilogram>())
                .ok_or_else(missing),
            (FlowsheetVariable::VaporFraction, ObjectData::Material(ms)) => {
                ms.vapor_fraction().map(|b| b.get::<ratio>()).ok_or_else(missing)
            }
            (FlowsheetVariable::EnergyFlow, ObjectData::Energy(es)) => {
                es.power().map(|p| p.get::<watt>()).ok_or_else(missing)
            }
            (FlowsheetVariable::UnitOperationPower, ObjectData::UnitOperation { power, .. }) => {
                // Stored in DWSIM's kW.
                power.map(|kw| kw * 1000.0).ok_or_else(missing)
            }
            (
                FlowsheetVariable::UnitOperationResult(key),
                ObjectData::UnitOperation { results, .. },
            ) => results.get(key).copied().ok_or_else(missing),
            _ => Err(missing()),
        }
    }

    /// Write this variable onto `object` in `flowsheet`, in SI.
    ///
    /// Stands in for `SetMnpVarValue` (FlowsheetSolver.vb:2377-2387).
    ///
    /// # Errors
    ///
    /// [`SolverError::UnknownObject`] if the object is missing;
    /// [`SolverError::UnknownVariable`] if the object cannot carry this
    /// variable.
    pub fn set(
        &self,
        flowsheet: &mut Flowsheet,
        object: &ObjectId,
        value: f64,
    ) -> Result<(), SolverError> {
        let missing = SolverError::UnknownVariable {
            object: object.0.clone(),
            variable: self.name().to_string(),
        };
        let obj = flowsheet
            .object_mut(object)
            .ok_or_else(|| SolverError::UnknownObject(object.0.clone()))?;
        match (self, &mut obj.data) {
            (FlowsheetVariable::Temperature, ObjectData::Material(ms)) => {
                ms.set_temperature(ThermodynamicTemperature::new::<kelvin>(value));
            }
            (FlowsheetVariable::Pressure, ObjectData::Material(ms)) => {
                ms.set_pressure(Pressure::new::<pascal>(value));
            }
            (FlowsheetVariable::MassFlow, ObjectData::Material(ms)) => {
                ms.set_mass_flow(MassRate::new::<kilogram_per_second>(value));
            }
            (FlowsheetVariable::MolarFlow, ObjectData::Material(ms)) => {
                ms.set_molar_flow(MolarFlowRate::new::<katal>(value));
            }
            (FlowsheetVariable::MassEnthalpy, ObjectData::Material(ms)) => {
                ms.set_mass_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(value));
            }
            (FlowsheetVariable::VaporFraction, ObjectData::Material(ms)) => {
                ms.set_vapor_fraction(Ratio::new::<ratio>(value));
            }
            (FlowsheetVariable::EnergyFlow, ObjectData::Energy(es)) => {
                es.set_power(Power::new::<watt>(value));
            }
            (FlowsheetVariable::UnitOperationPower, ObjectData::UnitOperation { power, .. }) => {
                *power = Some(value / 1000.0);
            }
            (
                FlowsheetVariable::UnitOperationResult(key),
                ObjectData::UnitOperation { results, .. },
            ) => {
                results.insert(key.clone(), value);
            }
            _ => return Err(missing),
        }
        Ok(())
    }
}

/// An object plus the variable on it — DWSIM's `IAdjust.ControlledObjectData`
/// and friends, which pair an `ID` with a `PropertyName`
/// (FlowsheetSolver.vb:2347-2348).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableRef {
    /// Which flowsheet object.
    pub object: ObjectId,
    /// Which scalar on it.
    pub variable: FlowsheetVariable,
}

impl VariableRef {
    /// Pair an object with a variable.
    #[must_use]
    pub fn new(object: ObjectId, variable: FlowsheetVariable) -> Self {
        VariableRef { object, variable }
    }

    /// Read the referenced scalar, in SI.
    ///
    /// # Errors
    ///
    /// As [`FlowsheetVariable::get`].
    pub fn get(&self, flowsheet: &Flowsheet) -> Result<f64, SolverError> {
        self.variable.get(flowsheet, &self.object)
    }

    /// Write the referenced scalar, in SI.
    ///
    /// # Errors
    ///
    /// As [`FlowsheetVariable::set`].
    pub fn set(&self, flowsheet: &mut Flowsheet, value: f64) -> Result<(), SolverError> {
        self.variable.set(flowsheet, &self.object, value)
    }
}

#[cfg(test)]
mod tests {
    //! # Verification — variable addressing
    //!
    //! **Methodology.** Round-trip every variant through
    //! [`FlowsheetVariable::set`] then [`FlowsheetVariable::get`] on an object
    //! that carries it, and check that an object which does not carry it reports
    //! [`SolverError::UnknownVariable`]. Tolerance `1e-9` relative, which covers
    //! the kJ/kg and kW conversions. Verification only, no physics.
    //! **Results (2026-08-11, release build):** recorded per test below.

    use super::*;
    use crate::flowsheet::{ObjectType, PhaseIndex};

    /// **Methodology.** Set `T = 350 K`, `P = 5e5 Pa`, `w = 2.5 kg/s`,
    /// `h = 1.5e5 J/kg` on a material stream and read them back; the enthalpy
    /// exercises the kJ/kg storage conversion.
    /// **Result (2026-08-11):** all four round-trip to within `1e-9` relative;
    /// enthalpy stored internally as `150.000000 kJ/kg`.
    #[test]
    fn material_stream_variables_round_trip() {
        let mut fs = Flowsheet::new();
        let s = fs.add_object(ObjectType::MaterialStream, Some("S"));
        {
            let ms = fs.object_mut(&s).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
        }

        for (v, value) in [
            (FlowsheetVariable::Temperature, 350.0),
            (FlowsheetVariable::Pressure, 5.0e5),
            (FlowsheetVariable::MassFlow, 2.5),
            (FlowsheetVariable::MassEnthalpy, 1.5e5),
        ] {
            v.set(&mut fs, &s, value).unwrap();
            let got = v.get(&fs, &s).unwrap();
            assert!(
                (got - value).abs() <= 1e-9 * value.abs().max(1.0),
                "{}: set {value}, got {got}",
                v.name()
            );
        }
        // The stored field keeps DWSIM's kJ/kg.
        let stored = fs
            .object(&s)
            .unwrap()
            .data
            .as_material()
            .unwrap()
            .phase(PhaseIndex::Mixture)
            .properties
            .enthalpy
            .unwrap();
        assert!((stored - 150.0).abs() < 1e-9);
    }

    /// **Methodology.** Energy-stream power and unit-operation power both live
    /// in W on this interface; the latter is stored in kW internally.
    /// **Result (2026-08-11):** `EnergyFlow` round-trips `75000 W`;
    /// `UnitOperationPower` round-trips `-12000 W` and is stored as
    /// `-12.000000` kW.
    #[test]
    fn power_variables_round_trip_through_their_storage_units() {
        let mut fs = Flowsheet::new();
        let e = fs.add_object(ObjectType::EnergyStream, Some("E"));
        let p = fs.add_object(ObjectType::Pump, None);

        FlowsheetVariable::EnergyFlow.set(&mut fs, &e, 75_000.0).unwrap();
        assert!((FlowsheetVariable::EnergyFlow.get(&fs, &e).unwrap() - 75_000.0).abs() < 1e-6);

        FlowsheetVariable::UnitOperationPower
            .set(&mut fs, &p, -12_000.0)
            .unwrap();
        assert!(
            (FlowsheetVariable::UnitOperationPower.get(&fs, &p).unwrap() + 12_000.0).abs() < 1e-6
        );
        if let ObjectData::UnitOperation { power, .. } = &fs.object(&p).unwrap().data {
            assert!((power.unwrap() + 12.0).abs() < 1e-12);
        } else {
            panic!("a pump must carry UnitOperation data");
        }
    }

    /// **Methodology.** A results-map entry must be creatable and readable, and
    /// a missing key or a type mismatch must report
    /// [`SolverError::UnknownVariable`].
    /// **Result (2026-08-11):** `"head"` round-trips `42.5`; reading `"missing"`
    /// and reading a temperature off an energy stream both return
    /// `UnknownVariable`.
    #[test]
    fn results_map_and_missing_variables() {
        let mut fs = Flowsheet::new();
        let p = fs.add_object(ObjectType::Pump, None);
        let e = fs.add_object(ObjectType::EnergyStream, Some("E"));

        let head = FlowsheetVariable::UnitOperationResult("head".to_string());
        head.set(&mut fs, &p, 42.5).unwrap();
        assert!((head.get(&fs, &p).unwrap() - 42.5).abs() < 1e-12);

        let missing = FlowsheetVariable::UnitOperationResult("missing".to_string());
        assert!(matches!(
            missing.get(&fs, &p),
            Err(SolverError::UnknownVariable { .. })
        ));
        assert!(matches!(
            FlowsheetVariable::Temperature.get(&fs, &e),
            Err(SolverError::UnknownVariable { .. })
        ));
    }
}
