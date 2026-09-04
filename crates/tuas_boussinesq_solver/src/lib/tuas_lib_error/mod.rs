//! Crate-wide error type ([`TuasLibError`]).
//!
//! Every fallible operation in `tuas_boussinesq_solver` returns
//! `Result<_, TuasLibError>`. Variants cover array/dimension shape mismatches,
//! an empty mass-flowrate vector (so a Courant number cannot be formed),
//! thermophysical-property failures (including a temperature that falls
//! outside a property correlation's valid range), wrong heat-transfer
//! interaction / entity / material types, and a catch-all string error. A
//! `From<String>`/`Into<String>` bridge is provided for interop with the many
//! string-based error sites in the codebase.
use thiserror::Error;
use uom::si::f64::ThermodynamicTemperature;

/// Master Error type of this crate
#[derive(Debug, Error)]
pub enum TuasLibError {
    /// array shape / dimension mismatch (replaces the former ndarray-linalg LinalgError)
    #[error("shape mismatch: {0}")]
    ShapeMismatch(String),

    /// empty mass flowrate vector error
    ///
    /// this case is where the mass flowrate vector in a control
    /// volume is empty,
    /// so we can't calculate a courant number

    #[error(
        "cannot calculate courant number: mass flowrate \n 
        there is no mass flows going in or out of your \n 
        control volume"
    )]
    CourantMassFlowVectorEmpty,

    /// it's a generic error which is a placeholder since I used
    /// so many string errors
    #[error("Placeholder Error Type for Strings{0} ")]
    GenericStringError(String),

    /// error to indicate that function is not implemented for BC
    #[error("{0}")]
    NotImplementedForBoundaryConditions(String),

    /// error for type conversions for heat transfer entity
    #[error("heat transfer entity is of the wrong type")]
    TypeConversionErrorHeatTransferEntity,

    /// error for type conversions for material
    #[error("material is of the wrong type for proper conversion")]
    TypeConversionErrorMaterial,

    /// A temperature fell outside the valid range of a thermophysical-property
    /// correlation.
    ///
    /// The payload names **which** material was asked, at **what** temperature,
    /// and **what** range it is tabulated over. Without those four facts the
    /// error is unactionable: a bare "out of range" tells a caller nothing
    /// about which of its many nodes failed, and that is exactly how an
    /// `htgr_sim_v1` steam-generator failure once presented -- a panic naming
    /// no material, no node, and no temperature.
    ///
    /// Construct it with [`TuasLibError::temperature_out_of_range`] rather than
    /// by hand, so the field order cannot be transposed.
    ///
    /// # Fields
    ///
    /// - `material` — the material's `Debug` rendering. It is a `String` and
    ///   not a `Material` deliberately: this module is Layer 0 and
    ///   `boussinesq_thermophysical_properties` is Layer 1, so holding the real
    ///   type here would invert the crate's module layering.
    /// - `temperature` — the temperature actually requested.
    /// - `lower_bound` / `upper_bound` — the inclusive limits of the
    ///   correlation's tabulated range.
    ///
    /// All three temperatures are `uom`-typed; the message renders them in both
    /// kelvin and degrees Celsius, because property tables in this crate are
    /// written in kelvin while plant data is almost always quoted in Celsius.
    #[error(
        "thermophysical property temperature out of range: {material} was evaluated at \
         {:.2} K ({:.2} degC), outside its tabulated range {:.2} K to {:.2} K \
         ({:.2} degC to {:.2} degC)",
        .temperature.get::<uom::si::thermodynamic_temperature::kelvin>(),
        .temperature.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
        .lower_bound.get::<uom::si::thermodynamic_temperature::kelvin>(),
        .upper_bound.get::<uom::si::thermodynamic_temperature::kelvin>(),
        .lower_bound.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
        .upper_bound.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
    )]
    ThermophysicalPropertyTemperatureRangeError {
        /// `Debug` rendering of the material that was asked (see the variant
        /// docs for why this is not a `Material`).
        material: String,
        /// The temperature that was requested.
        temperature: ThermodynamicTemperature,
        /// Inclusive lower limit of the correlation's tabulated range.
        lower_bound: ThermodynamicTemperature,
        /// Inclusive upper limit of the correlation's tabulated range.
        upper_bound: ThermodynamicTemperature,
    },

    /// An input other than temperature fell outside a correlation's validated
    /// envelope.
    ///
    /// Distinct from
    /// [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`], which is
    /// specifically about a *temperature* against a *property table*. This one
    /// covers any other bounded correlation input — neutron fluence, Reynolds
    /// number, Prandtl number, a geometric ratio — where the quantity is not a
    /// temperature and the bound is not a tabulated property range.
    ///
    /// It exists because those checks were previously reported as temperature
    /// range errors, which forced a caller reading the message to be told a
    /// temperature had gone out of range when no temperature was involved.
    ///
    /// # Fields
    ///
    /// - `parameter` — the name of the quantity, as a reader would recognise it
    ///   (e.g. `"fluence gam"`, `"Reynolds number"`).
    /// - `value` — the value supplied.
    /// - `lower_bound` / `upper_bound` — the inclusive limits of the validated
    ///   envelope.
    /// - `units` — the units `value` and the bounds are expressed in, spelled
    ///   out for a human (e.g. `"10^25 n/m^2, E > 0.1 MeV"`, `"dimensionless"`).
    ///   These are plain `f64` rather than `uom` quantities precisely because
    ///   the variant is generic over quantity kinds, so the unit cannot be
    ///   carried in the type; naming it here is the substitute.
    #[error(
        "{parameter} = {value} is outside the correlation's validated range \
         [{lower_bound}, {upper_bound}] ({units})"
    )]
    CorrelationRangeError {
        /// Human-recognisable name of the out-of-range quantity.
        parameter: String,
        /// The value that was supplied.
        value: f64,
        /// Inclusive lower limit of the validated envelope.
        lower_bound: f64,
        /// Inclusive upper limit of the validated envelope.
        upper_bound: f64,
        /// Units of `value` and the bounds, spelled out for a human reader.
        units: String,
    },

    /// generic thermophysical property error
    #[error("Thermophysical Property Error")]
    ThermophysicalPropertyError,

    /// wrong heat transfer interaction type
    #[error("Wrong Heat Transfer Interaction Type")]
    WrongHeatTransferInteractionType,
}

impl TuasLibError {
    /// Builds a [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`]
    /// carrying the four facts a caller needs to act on it.
    ///
    /// Prefer this over constructing the variant by hand: the three
    /// temperatures are the same type, so a positional struct literal is easy
    /// to transpose silently, and `material` is taken as `impl Debug` so that
    /// callers can pass a `Material`/`SolidMaterial`/`LiquidMaterial` directly
    /// without this Layer-0 module having to depend on the Layer-1 type.
    ///
    /// # Parameters
    ///
    /// - `material` — anything `Debug`; rendered once, at construction.
    /// - `temperature` — the temperature that was requested, in any `uom`
    ///   temperature unit.
    /// - `lower_bound` / `upper_bound` — the inclusive limits of the
    ///   correlation's tabulated range.
    ///
    /// # Example
    ///
    /// ```
    /// use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
    /// use uom::si::f64::ThermodynamicTemperature;
    /// use uom::si::thermodynamic_temperature::kelvin;
    ///
    /// let err = TuasLibError::temperature_out_of_range(
    ///     "SteelSS304L",
    ///     ThermodynamicTemperature::new::<kelvin>(1100.0),
    ///     ThermodynamicTemperature::new::<kelvin>(250.0),
    ///     ThermodynamicTemperature::new::<kelvin>(1000.0),
    /// );
    /// assert!(err.to_string().contains("1100.00 K"));
    /// assert!(err.to_string().contains("SteelSS304L"));
    /// ```
    pub fn temperature_out_of_range(
        material: impl core::fmt::Debug,
        temperature: ThermodynamicTemperature,
        lower_bound: ThermodynamicTemperature,
        upper_bound: ThermodynamicTemperature,
    ) -> Self {
        Self::ThermophysicalPropertyTemperatureRangeError {
            material: format!("{:?}", material),
            temperature,
            lower_bound,
            upper_bound,
        }
    }
}

///  converts ThermalHydraulicsLibError from string error
impl From<String> for TuasLibError {
    fn from(value: String) -> Self {
        Self::GenericStringError(value)
    }
}

impl Into<String> for TuasLibError {
    fn into(self) -> String {
        match self {
            TuasLibError::ShapeMismatch(s) => s,
            TuasLibError::CourantMassFlowVectorEmpty => self.to_string(),
            TuasLibError::GenericStringError(string) => string,
            TuasLibError::NotImplementedForBoundaryConditions(string) => string,
            TuasLibError::TypeConversionErrorHeatTransferEntity => self.to_string(),
            TuasLibError::TypeConversionErrorMaterial => self.to_string(),
            // Rendered through Display so the material, the temperature and
            // the violated bounds survive the conversion to a bare string.
            TuasLibError::ThermophysicalPropertyTemperatureRangeError { .. } => self.to_string(),
            TuasLibError::CorrelationRangeError { .. } => self.to_string(),
            TuasLibError::ThermophysicalPropertyError => self.to_string(),
            TuasLibError::WrongHeatTransferInteractionType => self.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TuasLibError;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Methodology: build the temperature-range error for the exact condition
    /// that killed `htgr_sim_v1` -- `SteelSS304L` asked at 1100 K against its
    /// tabulated 250-1000 K range -- and assert the rendered message names all
    /// four facts a caller needs: the material, the requested temperature, and
    /// both bounds. Pass criterion: the material name, the temperature in both
    /// kelvin and degrees Celsius, and both bounds all appear in the string.
    ///
    /// This is the regression guard for the defect that motivated the change.
    /// The variant was previously a unit variant, so the message was the fixed
    /// text "Temperature supplied for thermophysical_properties function was
    /// out of range" -- which named no material, no temperature and no bound,
    /// and left the reported crash undiagnosable from its output alone.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// The rendered message is, verbatim:
    ///
    /// ```text
    /// thermophysical property temperature out of range: "SteelSS304L" was evaluated at
    /// 1100.00 K (826.85 degC), outside its tabulated range 250.00 K to 1000.00 K
    /// (-23.15 degC to 726.85 degC)
    /// ```
    ///
    /// Note the quotes around the material name: this test passes a `&str`, and
    /// the payload is built with `Debug`, which quotes string slices. A real
    /// caller passing a `Material` renders unquoted instead, as
    /// `Solid(SteelSS304L)`. Both are acceptable; the quoting is recorded here
    /// only so the documented output matches what the test actually prints.
    ///
    /// All four facts present; both unit systems rendered. Interpretation: a
    /// caller receiving this can identify the failing component and the margin
    /// it exceeded without reproducing the run under a debugger.
    #[test]
    fn temperature_range_error_names_material_temperature_and_both_bounds() {
        let err = TuasLibError::temperature_out_of_range(
            "SteelSS304L",
            ThermodynamicTemperature::new::<kelvin>(1100.0),
            ThermodynamicTemperature::new::<kelvin>(250.0),
            ThermodynamicTemperature::new::<kelvin>(1000.0),
        );
        let msg = err.to_string();
        println!("rendered message:\n{msg}");

        assert!(msg.contains("SteelSS304L"), "must name the material: {msg}");
        assert!(
            msg.contains("1100.00 K"),
            "must give the temperature in K: {msg}"
        );
        assert!(
            msg.contains("826.85 degC"),
            "must give the temperature in degC: {msg}"
        );
        assert!(msg.contains("250.00 K"), "must give the lower bound: {msg}");
        assert!(
            msg.contains("1000.00 K"),
            "must give the upper bound: {msg}"
        );
    }

    /// Methodology: build a [`TuasLibError::CorrelationRangeError`] for the
    /// nuclear-graphite fluence bound and assert the message names the
    /// parameter, its value, both bounds and the units. Pass criterion: all
    /// five appear, and the word "temperature" does NOT -- the point of this
    /// variant is that a fluence violation must stop being reported as a
    /// temperature range error.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// The rendered message is, verbatim:
    ///
    /// ```text
    /// fluence gam = 17 is outside the correlation's validated range [0, 15]
    /// (10^25 n/m^2, E > 0.1 MeV)
    /// ```
    ///
    /// Parameter, value, both bounds and units all present; the word
    /// "temperature" does not appear. Interpretation: the misclassification
    /// that previously sent a reader looking for a temperature fault is gone.
    #[test]
    fn correlation_range_error_does_not_claim_a_temperature_failed() {
        let err = TuasLibError::CorrelationRangeError {
            parameter: "fluence gam".to_string(),
            value: 17.0,
            lower_bound: 0.0,
            upper_bound: 15.0,
            units: "10^25 n/m^2, E > 0.1 MeV".to_string(),
        };
        let msg = err.to_string();
        println!("rendered message:\n{msg}");

        assert!(
            msg.contains("fluence gam"),
            "must name the parameter: {msg}"
        );
        assert!(msg.contains("17"), "must give the value: {msg}");
        assert!(
            msg.contains('0') && msg.contains("15"),
            "must give both bounds: {msg}"
        );
        assert!(msg.contains("10^25 n/m^2"), "must give the units: {msg}");
        assert!(
            !msg.to_lowercase().contains("temperature"),
            "a fluence violation must not be reported as a temperature failure: {msg}"
        );
    }
}
