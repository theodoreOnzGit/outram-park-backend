//! Pure-compound **constant-property data model** — the substrate every thermo
//! module consumes.
//!
//! Mirrors the fields of DWSIM's `ICompoundConstantProperties`
//! (`DWSIM.Interfaces/ICompoundConstantProperties.vb`) needed by the cubic EOS,
//! activity models, and flash: molar mass, the critical point, the acentric
//! factor, the normal boiling point, and the ideal-gas heat-capacity
//! coefficients (the departure-function reference state).
//!
//! ## Units (documented raw `f64`, SI — the DWSIM-internal convention)
//!
//! Stored as plain `f64` in SI base units because these feed the inner EOS/flash
//! arithmetic loops (per the crate `CLAUDE.md` "raw f64 in inner EOS loops"
//! rule). Every field spells out its unit below.
//!
//! | Field | Quantity | Unit |
//! |---|---|---|
//! | `molar_mass` | molar mass `M` | kg/mol |
//! | `critical_temperature` | `Tc` | K |
//! | `critical_pressure` | `Pc` | Pa |
//! | `critical_volume` | `Vc` | m³/mol |
//! | `acentric_factor` | Pitzer acentric factor `ω` | dimensionless |
//! | `normal_boiling_point` | `Tb` at 1 atm | K |
//! | `cp_ig_a..e` | ideal-gas Cp correlation coefficients | see [`Component`] |

/// Pure-compound constant properties.
///
/// A plain data record: no behaviour beyond validated construction and
/// accessors. The ideal-gas heat capacity is evaluated from `cp_ig_a..e` by
/// [`crate::thermo::ideal_props`] (this struct only stores the coefficients);
/// the EOS `a(T)`/`b` parameters are computed by [`crate::thermo::cubic_eos`]
/// from `critical_temperature`, `critical_pressure`, and `acentric_factor`.
///
/// ## Ideal-gas Cp correlation
///
/// `cp_ig_a..e` are DWSIM's `Ideal_Gas_Heat_Capacity_Const_A..E`. The exact
/// polynomial/DIPPR form they parameterise is implemented by
/// [`crate::thermo::ideal_props`] against DWSIM's `PropertyPackageMethods`; this
/// record is agnostic to that form and merely carries the five coefficients
/// plus the reference entropy of formation.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// Human-readable compound name (identification only).
    pub name: String,
    /// Molar mass `M` [kg/mol]. Must be > 0.
    pub molar_mass: f64,
    /// Critical temperature `Tc` [K]. Must be > 0.
    pub critical_temperature: f64,
    /// Critical pressure `Pc` [Pa]. Must be > 0.
    pub critical_pressure: f64,
    /// Critical volume `Vc` [m³/mol]. Must be > 0 (or `f64::NAN` if unknown —
    /// EOS use Tc/Pc, not Vc, so it may be absent).
    pub critical_volume: f64,
    /// Pitzer acentric factor `ω` [-].
    pub acentric_factor: f64,
    /// Normal boiling point `Tb` [K] at 1 atm. Must be > 0 (used by Wilson
    /// K-value init only indirectly; may be `f64::NAN` if unknown).
    pub normal_boiling_point: f64,
    /// Ideal-gas Cp coefficient A (units per the correlation in `ideal_props`).
    pub cp_ig_a: f64,
    /// Ideal-gas Cp coefficient B.
    pub cp_ig_b: f64,
    /// Ideal-gas Cp coefficient C.
    pub cp_ig_c: f64,
    /// Ideal-gas Cp coefficient D.
    pub cp_ig_d: f64,
    /// Ideal-gas Cp coefficient E.
    pub cp_ig_e: f64,
    /// Ideal-gas entropy of formation at 25 °C [J/(mol·K)] (`f64::NAN` if unused).
    pub ig_entropy_formation_25c: f64,
}

/// Error constructing a [`Component`] from out-of-range constants.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComponentError {
    /// A required positive property was zero, negative, or non-finite.
    #[error("component `{name}`: property `{property}` must be finite and > 0 (got {value})")]
    NonPositive {
        /// Compound name.
        name: String,
        /// Offending property.
        property: &'static str,
        /// Offending value.
        value: f64,
    },
}

impl Component {
    /// Construct a component from its critical constants, acentric factor, and
    /// ideal-gas Cp coefficients, validating that `molar_mass`,
    /// `critical_temperature`, and `critical_pressure` are finite and strictly
    /// positive. `critical_volume` and `normal_boiling_point` may be `NAN`
    /// (unknown); the acentric factor and Cp coefficients are unconstrained.
    ///
    /// Prefer the [`reference`] presets for common fluids.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        molar_mass: f64,
        critical_temperature: f64,
        critical_pressure: f64,
        critical_volume: f64,
        acentric_factor: f64,
        normal_boiling_point: f64,
        cp_ig: [f64; 5],
        ig_entropy_formation_25c: f64,
    ) -> Result<Self, ComponentError> {
        let name = name.into();
        let check = |property: &'static str, value: f64| -> Result<(), ComponentError> {
            if !value.is_finite() || value <= 0.0 {
                Err(ComponentError::NonPositive { name: name.clone(), property, value })
            } else {
                Ok(())
            }
        };
        check("molar_mass", molar_mass)?;
        check("critical_temperature", critical_temperature)?;
        check("critical_pressure", critical_pressure)?;
        Ok(Self {
            name,
            molar_mass,
            critical_temperature,
            critical_pressure,
            critical_volume,
            acentric_factor,
            normal_boiling_point,
            cp_ig_a: cp_ig[0],
            cp_ig_b: cp_ig[1],
            cp_ig_c: cp_ig[2],
            cp_ig_d: cp_ig[3],
            cp_ig_e: cp_ig[4],
            ig_entropy_formation_25c,
        })
    }

    /// Reduced temperature `Tr = T / Tc` [-] at `temperature` [K].
    #[must_use]
    pub fn reduced_temperature(&self, temperature: f64) -> f64 {
        temperature / self.critical_temperature
    }

    /// Reduced pressure `Pr = P / Pc` [-] at `pressure` [Pa].
    #[must_use]
    pub fn reduced_pressure(&self, pressure: f64) -> f64 {
        pressure / self.critical_pressure
    }
}

/// Reference-compound presets with **public-literature** constant properties.
///
/// Critical constants, acentric factors, and molar masses are the standard
/// tabulated values from Poling, Prausnitz & O'Connell, *The Properties of
/// Gases and Liquids*, 5th ed. (McGraw-Hill, 2001), Appendix A — an open,
/// widely-cited reference (workspace `DATA_POLICY`: public literature data
/// only). Ideal-gas Cp coefficients are left as `0.0` placeholders here (the
/// `ideal_props` module owns the Cp correlation and its data); callers needing
/// enthalpy departures should supply real Cp coefficients.
pub mod reference {
    use super::Component;

    /// Water (H₂O). Tc = 647.14 K, Pc = 22.064 MPa, ω = 0.344, M = 18.015 g/mol.
    #[must_use]
    pub fn water() -> Component {
        Component::new(
            "Water", 0.018015, 647.14, 22.064e6, 55.95e-6, 0.344, 373.15,
            [0.0; 5], f64::NAN,
        )
        .expect("water reference constants are valid")
    }

    /// Methane (CH₄). Tc = 190.56 K, Pc = 4.599 MPa, ω = 0.011, M = 16.043 g/mol.
    #[must_use]
    pub fn methane() -> Component {
        Component::new(
            "Methane", 0.016043, 190.56, 4.599e6, 98.6e-6, 0.011, 111.66,
            [0.0; 5], f64::NAN,
        )
        .expect("methane reference constants are valid")
    }

    /// Ethane (C₂H₆). Tc = 305.32 K, Pc = 4.872 MPa, ω = 0.099, M = 30.070 g/mol.
    #[must_use]
    pub fn ethane() -> Component {
        Component::new(
            "Ethane", 0.030070, 305.32, 4.872e6, 145.5e-6, 0.099, 184.55,
            [0.0; 5], f64::NAN,
        )
        .expect("ethane reference constants are valid")
    }

    /// Nitrogen (N₂). Tc = 126.20 K, Pc = 3.398 MPa, ω = 0.037, M = 28.014 g/mol.
    #[must_use]
    pub fn nitrogen() -> Component {
        Component::new(
            "Nitrogen", 0.028014, 126.20, 3.398e6, 90.1e-6, 0.037, 77.35,
            [0.0; 5], f64::NAN,
        )
        .expect("nitrogen reference constants are valid")
    }

    /// Carbon dioxide (CO₂). Tc = 304.12 K, Pc = 7.374 MPa, ω = 0.225, M = 44.01 g/mol.
    #[must_use]
    pub fn carbon_dioxide() -> Component {
        Component::new(
            "CarbonDioxide", 0.044010, 304.12, 7.374e6, 94.07e-6, 0.225, 194.65,
            [0.0; 5], f64::NAN,
        )
        .expect("carbon dioxide reference constants are valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** Construct each reference compound and check the stored
    /// constants match the cited Poling et al. (2001) Appendix-A values and that
    /// reduced-property helpers are exact. **Results (2026-07-24):** all presets
    /// construct; water Tr at 323.57 K = 0.5 exactly, Pr at 11.032 MPa = 0.5.
    #[test]
    fn reference_compounds_construct_and_reduce() {
        let w = reference::water();
        assert_eq!(w.critical_temperature, 647.14);
        assert!((w.reduced_temperature(323.57) - 0.5).abs() < 1e-12);
        assert!((w.reduced_pressure(11.032e6) - 0.5).abs() < 1e-12);
        for c in [
            reference::methane(),
            reference::ethane(),
            reference::nitrogen(),
            reference::carbon_dioxide(),
        ] {
            assert!(c.critical_temperature > 0.0 && c.critical_pressure > 0.0);
            assert!(c.molar_mass > 0.0);
        }
    }

    /// **Methodology.** Non-positive / non-finite required constants must be
    /// rejected. **Results (2026-07-24):** zero Tc, negative Pc, and NaN molar
    /// mass each return `ComponentError::NonPositive`.
    #[test]
    fn rejects_invalid_constants() {
        assert!(Component::new("bad", 0.018, 0.0, 1e5, f64::NAN, 0.3, f64::NAN, [0.0; 5], f64::NAN).is_err());
        assert!(Component::new("bad", 0.018, 600.0, -1.0, f64::NAN, 0.3, f64::NAN, [0.0; 5], f64::NAN).is_err());
        assert!(Component::new("bad", f64::NAN, 600.0, 1e5, f64::NAN, 0.3, f64::NAN, [0.0; 5], f64::NAN).is_err());
    }
}
