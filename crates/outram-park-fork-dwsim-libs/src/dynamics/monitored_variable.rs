//! Monitored variables — the properties an integrator samples once per step to
//! build a time series.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/MonitoredVariable.vb`
//! (whole file, lines 22-57) and the sampling routine
//! `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:132-153`
//! (`StoreVariableValues`), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # How sampling works
//!
//! A [`MonitoredVariable`] on
//! [`crate::dynamics::integrator::Integrator::monitored_variables`] is a
//! *template*: it names an object, a property and a display unit. Once per step
//! the run loop **clones** each template, fills in the reading and the
//! timestamp, and files the clone under the current simulated time
//! (`StoreVariableValues`, :132-153, and its call site :498). The samples
//! therefore keep the same order as the template list, which is what lets
//! upstream's chart builder zip them into per-series vectors
//! (Manager.vb:173-179).
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData` / `Clone`
//!   (MonitoredVariable.vb:44-56; note `Clone` is implemented *as* a
//!   save-then-load round trip, which this port replaces with a derived
//!   [`Clone`]).
//! - **Chart axis limits.** `MinimumChartAxisValue` / `MaximumChartAxisValue`
//!   (MonitoredVariable.vb:40-42) are ported as plain fields because they are
//!   part of the persisted data model, but nothing here reads them — they feed
//!   OxyPlot axis setup (Manager.vb:154-157), which is excluded.
//!
//! # Divergence: the value is an `f64`, not a string
//!
//! Upstream stores `PropertyValue As String` (MonitoredVariable.vb:36) and
//! round-trips it through `ToString(InvariantCulture)` on write
//! (FormDynamicsIntegratorControl.vb:145) and `ToDoubleFromInvariant` on read
//! (Manager.vb:176). This port keeps an `f64`, which removes a lossy formatting
//! step; nothing else about the sample changes.

use crate::dynamics::errors::DynamicsError;
use crate::dynamics::property::{
    convert_from_internal, property_value, DynamicProperty, PropertyRef,
};
use crate::dynamics::sim_time::SimInstant;
use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::ObjectId;

/// One monitored property, used both as a sampling template and as a recorded
/// sample — upstream's `MonitoredVariable` (MonitoredVariable.vb:22-57).
///
/// Units: [`MonitoredVariable::property_value`] is expressed in
/// [`MonitoredVariable::property_units`] (the **display** unit), because
/// upstream converts out of internal units when it records the sample
/// (`ConvertFromSI(vnew.PropertyUnits, …)`, FormDynamicsIntegratorControl.vb:145).
/// An empty or unrecognised unit string leaves the value in DWSIM-internal units.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonitoredVariable {
    /// Unique identifier (MonitoredVariable.vb:26). Upstream uses it as the
    /// chart series/axis key.
    pub id: String,
    /// Human-readable label — the chart series name (MonitoredVariable.vb:28,
    /// used at Manager.vb:150 and :167).
    pub description: String,
    /// The simulated time this sample was taken at (MonitoredVariable.vb:30,
    /// written at FormDynamicsIntegratorControl.vb:146). Meaningless on a
    /// template.
    pub timestamp: SimInstant,
    /// Which object and property to sample (MonitoredVariable.vb:32-34).
    pub source: PropertyRef,
    /// The reading, in [`MonitoredVariable::property_units`]
    /// (MonitoredVariable.vb:36). Zero on a template.
    pub property_value: f64,
    /// Display units for the reading (MonitoredVariable.vb:38).
    pub property_units: String,
    /// Lower chart-axis bound; unused by this port (MonitoredVariable.vb:40).
    pub minimum_chart_axis_value: f64,
    /// Upper chart-axis bound; unused by this port (MonitoredVariable.vb:42).
    pub maximum_chart_axis_value: f64,
}

impl MonitoredVariable {
    /// A sampling template for `property` of `object`, recorded in `units`.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        object: ObjectId,
        property: DynamicProperty,
        units: impl Into<String>,
    ) -> Self {
        let id = id.into();
        MonitoredVariable {
            description: id.clone(),
            id,
            timestamp: SimInstant::ZERO,
            source: PropertyRef::new(object, property),
            property_value: 0.0,
            property_units: units.into(),
            minimum_chart_axis_value: 0.0,
            maximum_chart_axis_value: 0.0,
        }
    }

    /// Clone this template and fill in the reading taken from `flowsheet` at
    /// `timestamp`.
    ///
    /// Ports the body of `StoreVariableValues`'s loop
    /// (FormDynamicsIntegratorControl.vb:139-148): clone, read, convert out of
    /// internal units, stamp the time.
    ///
    /// # Errors
    ///
    /// [`DynamicsError::ObjectNotFound`] if the object is not on the flowsheet —
    /// upstream raises "At least one of the monitored variables is not
    /// configured correctly, please check." (:141-143) — or
    /// [`DynamicsError::PropertyNotAvailable`] if the property cannot be read.
    pub fn sample(
        &self,
        flowsheet: &Flowsheet,
        timestamp: SimInstant,
    ) -> Result<MonitoredVariable, DynamicsError> {
        let internal = property_value(flowsheet, &self.source)?;
        let mut sample = self.clone();
        sample.property_value = convert_from_internal(&self.property_units, internal);
        sample.timestamp = timestamp;
        Ok(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowsheet::objects::ObjectType;

    #[test]
    fn sampling_converts_into_display_units_and_stamps_the_time() {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        let template = MonitoredVariable::new("T", id, DynamicProperty::Temperature, "C");

        let sample = template
            .sample(&fs, SimInstant::from_seconds(30.0))
            .unwrap();
        assert!(
            (sample.property_value - 25.0).abs() < 1e-9,
            "298.15 K is 25 C"
        );
        assert_eq!(sample.timestamp, SimInstant::from_seconds(30.0));
        // The template itself is untouched, exactly as upstream clones before writing.
        assert!((template.property_value - 0.0).abs() < 1e-12);
        assert_eq!(template.timestamp, SimInstant::ZERO);
    }

    #[test]
    fn a_misconfigured_variable_reports_the_missing_object() {
        let fs = Flowsheet::new();
        let template = MonitoredVariable::new(
            "T",
            ObjectId::from("does-not-exist"),
            DynamicProperty::Temperature,
            "K",
        );
        assert!(matches!(
            template.sample(&fs, SimInstant::ZERO),
            Err(DynamicsError::ObjectNotFound(_))
        ));
    }
}
