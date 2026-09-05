//! Cause-and-effect matrix — "when indicator X raises alarm A, force property P
//! of object O to value v".
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM**
//! `DWSIM.DynamicsManager/CauseAndEffectItem.vb` (whole file, lines 22-55),
//! `DWSIM.DynamicsManager/CauseAndEffectMatrix.vb` (whole file, lines 21-55),
//! the alarm enum in `DWSIM.Interfaces/Enums.vb:33-38`, the indicator contract in
//! `DWSIM.Interfaces/IIndicator.vb` (whole file), the alarm evaluation in
//! `DWSIM.UnitOperations/Indicators/AnalogGauge.vb:103-127` (identical in
//! `DigitalGauge.vb:103-127` and `LevelGauge.vb:103-127`), and the matrix
//! processing in
//! `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:187-215`
//! (`ProcessCEMatrix`, `DoAlarmEffect`). Upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # How it runs
//!
//! Once per integrator step, and **only outside real-time mode**
//! (FormDynamicsIntegratorControl.vb:555-562), the run loop walks every enabled
//! matrix item, asks the item's indicator whether the named alarm is active, and
//! if so applies the item's effect — a single `SetPropertyValue`
//! (`DoAlarmEffect`, :209-215). There is no latching, no de-bounce, no
//! acknowledgement and no "return to normal" action: the effect is re-applied on
//! **every** step the alarm stays active, and nothing is undone when it clears.
//!
//! # Divergence: where indicator state lives
//!
//! Upstream indicators are flowsheet objects implementing `IIndicator`, and the
//! flowsheet solver recomputes their alarm flags inside `SolveFlowsheet` (each
//! gauge's `Calculate`, AnalogGauge.vb:103-127). This crate's flowsheet data
//! model carries no indicator payload, and the per-step solve is a
//! **caller-supplied hook** that knows nothing about indicators. So this port
//! keeps [`IndicatorState`] on the dynamics manager and re-evaluates it inside
//! [`process_ce_matrix`], immediately before testing the alarm. The arithmetic
//! is upstream's, verbatim; only the ownership moved.
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData`
//!   (CauseAndEffectItem.vb:46-53, CauseAndEffectMatrix.vb:31-53).
//! - **Script effects.** `CauseAndEffectItem.ScriptID`
//!   (CauseAndEffectItem.vb:44) is stored, but `DoAlarmEffect` (:209-215) never
//!   reads it — an alarm effect is always a property write upstream too.
//! - **Gauge rendering** — `DWSIM.Drawing.SkiaSharp/GraphicObjects/Indicators/*`
//!   and the `MinimumValue`/`MaximumValue`/`DecimalDigits`/`IntegralDigits`/
//!   `ShowAlarms`/`DisplayInPercent` display fields of `IIndicator` are not
//!   ported: they affect only the drawn dial.
//! - **Silent exception swallowing.** AnalogGauge.vb:121-123 wraps the whole
//!   evaluation in an empty `Catch`, so a mis-configured indicator silently
//!   keeps its previous alarm flags. [`IndicatorState::update`] returns the
//!   error instead.

use std::collections::BTreeMap;

use crate::dynamics::errors::DynamicsError;
use crate::dynamics::property::{
    convert_from_internal, property_value, set_property_value, PropertyRef,
};
use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::ObjectId;

/// Which of an indicator's four alarms an item watches — DWSIM's
/// `Enums.Dynamics.DynamicsAlarmType` (Enums.vb:33-38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DynamicsAlarmType {
    /// Very low (upstream `0`).
    #[default]
    VeryLow,
    /// Low (upstream `1`).
    Low,
    /// High (upstream `2`).
    High,
    /// Very high (upstream `3`).
    VeryHigh,
}

impl DynamicsAlarmType {
    /// The integer this variant is stored as in a DWSIM file (Enums.vb:33-38):
    /// `LL = 0`, `L = 1`, `H = 2`, `HH = 3`.
    #[must_use]
    pub fn upstream_value(self) -> i32 {
        match self {
            DynamicsAlarmType::VeryLow => 0,
            DynamicsAlarmType::Low => 1,
            DynamicsAlarmType::High => 2,
            DynamicsAlarmType::VeryHigh => 3,
        }
    }
}

/// The alarm state of one indicator — the subset of DWSIM's `IIndicator`
/// (IIndicator.vb) that the cause-and-effect matrix reads, plus the setpoints
/// needed to compute it.
///
/// Units: [`IndicatorState::current_value`] and the four alarm setpoints are all
/// in [`IndicatorState::units`] — the indicator's **display** units, not SI.
/// That is upstream's convention: `AnalogGauge.vb:111` converts the SI property
/// value *out* to the display unit before comparing it with the setpoints, so a
/// gauge reading in °C alarms on °C thresholds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndicatorState {
    /// The flowsheet property this indicator reads (`SelectedObjectID` +
    /// `SelectedProperty`, IIndicator.vb).
    pub source: PropertyRef,
    /// Display units for the reading and the setpoints
    /// (`SelectedPropertyUnits`).
    pub units: String,
    /// Last reading, in [`IndicatorState::units`] (`CurrentValue`).
    pub current_value: f64,
    /// Whether the very-low alarm is armed (`VeryLowAlarmEnabled`).
    pub very_low_alarm_enabled: bool,
    /// Whether the low alarm is armed (`LowAlarmEnabled`).
    pub low_alarm_enabled: bool,
    /// Whether the high alarm is armed (`HighAlarmEnabled`).
    pub high_alarm_enabled: bool,
    /// Whether the very-high alarm is armed (`VeryHighAlarmEnabled`).
    pub very_high_alarm_enabled: bool,
    /// Very-low setpoint (`VeryLowAlarmValue`).
    pub very_low_alarm_value: f64,
    /// Low setpoint (`LowAlarmValue`).
    pub low_alarm_value: f64,
    /// High setpoint (`HighAlarmValue`).
    pub high_alarm_value: f64,
    /// Very-high setpoint (`VeryHighAlarmValue`).
    pub very_high_alarm_value: f64,
    /// Whether the very-low alarm is currently active (`VeryLowAlarmActive`).
    pub very_low_alarm_active: bool,
    /// Whether the low alarm is currently active (`LowAlarmActive`).
    pub low_alarm_active: bool,
    /// Whether the high alarm is currently active (`HighAlarmActive`).
    pub high_alarm_active: bool,
    /// Whether the very-high alarm is currently active (`VeryHighAlarmActive`).
    pub very_high_alarm_active: bool,
}

impl IndicatorState {
    /// An indicator reading `source` in `units`, with all four alarms disarmed.
    #[must_use]
    pub fn new(source: PropertyRef, units: impl Into<String>) -> Self {
        IndicatorState {
            source,
            units: units.into(),
            ..IndicatorState::default()
        }
    }

    /// Arm the low pair (`L` and `LL`) at the given setpoints, in
    /// [`IndicatorState::units`].
    #[must_use]
    pub fn with_low_alarms(mut self, low: f64, very_low: f64) -> Self {
        self.low_alarm_enabled = true;
        self.low_alarm_value = low;
        self.very_low_alarm_enabled = true;
        self.very_low_alarm_value = very_low;
        self
    }

    /// Arm the high pair (`H` and `HH`) at the given setpoints, in
    /// [`IndicatorState::units`].
    #[must_use]
    pub fn with_high_alarms(mut self, high: f64, very_high: f64) -> Self {
        self.high_alarm_enabled = true;
        self.high_alarm_value = high;
        self.very_high_alarm_enabled = true;
        self.very_high_alarm_value = very_high;
        self
    }

    /// Re-read the source property and recompute the four alarm flags.
    ///
    /// Ports `AnalogGauge.Calculate` (AnalogGauge.vb:103-127) exactly:
    ///
    /// ```text
    /// currentvalue          = ConvertFromSI(SelectedPropertyUnits, GetPropertyValue(SelectedProperty))
    /// VeryLowAlarmActive    = currentvalue <= VeryLowAlarmValue  And VeryLowAlarmEnabled
    /// LowAlarmActive        = currentvalue <= LowAlarmValue      And LowAlarmEnabled
    /// HighAlarmActive       = currentvalue >= HighAlarmValue     And HighAlarmEnabled
    /// VeryHighAlarmActive   = currentvalue >= VeryHighAlarmValue And VeryHighAlarmEnabled
    /// ```
    ///
    /// The comparisons are **non-strict** (`<=` / `>=`), so a value sitting
    /// exactly on a setpoint alarms; and the low pair is independent of the high
    /// pair, so a badly-ordered setpoint pair can leave both active at once —
    /// upstream behaves the same way.
    ///
    /// # Errors
    ///
    /// Propagates [`property_value`]'s errors instead of swallowing them the way
    /// AnalogGauge.vb:121-123 does.
    pub fn update(&mut self, flowsheet: &Flowsheet) -> Result<(), DynamicsError> {
        let internal = property_value(flowsheet, &self.source)?;
        self.current_value = convert_from_internal(&self.units, internal);
        self.very_low_alarm_active =
            self.current_value <= self.very_low_alarm_value && self.very_low_alarm_enabled;
        self.low_alarm_active =
            self.current_value <= self.low_alarm_value && self.low_alarm_enabled;
        self.high_alarm_active =
            self.current_value >= self.high_alarm_value && self.high_alarm_enabled;
        self.very_high_alarm_active =
            self.current_value >= self.very_high_alarm_value && self.very_high_alarm_enabled;
        Ok(())
    }

    /// Whether the named alarm is currently active — the `Select Case` at
    /// FormDynamicsIntegratorControl.vb:95-104.
    #[must_use]
    pub fn is_active(&self, alarm: DynamicsAlarmType) -> bool {
        match alarm {
            DynamicsAlarmType::VeryLow => self.very_low_alarm_active,
            DynamicsAlarmType::Low => self.low_alarm_active,
            DynamicsAlarmType::High => self.high_alarm_active,
            DynamicsAlarmType::VeryHigh => self.very_high_alarm_active,
        }
    }
}

/// One row of the matrix: a cause (indicator + alarm) and an effect (property
/// write) — upstream's `CauseAndEffectItem` (CauseAndEffectItem.vb:22-55).
///
/// Units: [`CauseAndEffectItem::property_value`] is in
/// [`CauseAndEffectItem::property_units`], as upstream stores it
/// (CauseAndEffectItem.vb:40-42).
#[derive(Debug, Clone, PartialEq)]
pub struct CauseAndEffectItem {
    /// Unique identifier, and the key the matrix stores it under
    /// (CauseAndEffectItem.vb:26).
    pub id: String,
    /// Human-readable label (CauseAndEffectItem.vb:28).
    pub description: String,
    /// Whether this row participates (CauseAndEffectItem.vb:30, tested at
    /// FormDynamicsIntegratorControl.vb:93).
    pub enabled: bool,
    /// The indicator object whose alarm is the cause
    /// (CauseAndEffectItem.vb:32).
    pub associated_indicator: ObjectId,
    /// Which of that indicator's four alarms (CauseAndEffectItem.vb:34).
    pub associated_indicator_alarm: DynamicsAlarmType,
    /// The property the effect writes (CauseAndEffectItem.vb:36-38).
    pub target: PropertyRef,
    /// The value to write, in [`CauseAndEffectItem::property_units`]
    /// (CauseAndEffectItem.vb:40).
    pub property_value: f64,
    /// Units of `property_value` (CauseAndEffectItem.vb:42).
    pub property_units: String,
    /// Script to run — stored but never executed, as upstream's `DoAlarmEffect`
    /// ignores it (CauseAndEffectItem.vb:44).
    pub script_id: String,
}

impl Default for CauseAndEffectItem {
    /// Upstream's field initialisers (CauseAndEffectItem.vb:26-44): empty
    /// strings everywhere, `Enabled = False` (a `Boolean` field with no
    /// initialiser), and the very-low alarm (`DynamicsAlarmType = 0`).
    fn default() -> Self {
        CauseAndEffectItem {
            id: String::new(),
            description: String::new(),
            enabled: false,
            associated_indicator: ObjectId(String::new()),
            associated_indicator_alarm: DynamicsAlarmType::VeryLow,
            target: PropertyRef::default(),
            property_value: 0.0,
            property_units: String::new(),
            script_id: String::new(),
        }
    }
}

impl CauseAndEffectItem {
    /// A row that, when `alarm` on `indicator` is active, writes `value`
    /// (in `units`) to `target`.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        indicator: ObjectId,
        alarm: DynamicsAlarmType,
        target: PropertyRef,
        value: f64,
        units: impl Into<String>,
    ) -> Self {
        CauseAndEffectItem {
            id: id.into(),
            description: String::new(),
            enabled: true,
            associated_indicator: indicator,
            associated_indicator_alarm: alarm,
            target,
            property_value: value,
            property_units: units.into(),
            script_id: String::new(),
        }
    }

    /// The effect value converted into DWSIM-internal units — upstream's
    /// `ConvertToSI(ceitem.SimulationObjectPropertyUnits, ceitem.SimulationObjectPropertyValue)`
    /// (FormDynamicsIntegratorControl.vb:213).
    #[must_use]
    pub fn value_in_internal_units(&self) -> f64 {
        crate::dynamics::property::convert_to_internal(&self.property_units, self.property_value)
    }
}

/// A named set of cause-and-effect rows — upstream's `CauseAndEffectMatrix`
/// (CauseAndEffectMatrix.vb:21-55).
///
/// **Divergence:** upstream's `Items` is a `Dictionary(Of String, …)`
/// (CauseAndEffectMatrix.vb:29); this port uses a [`BTreeMap`] so that the order
/// effects are applied in is deterministic (item-ID order). With overlapping
/// effects on the same property, order decides the winner.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CauseAndEffectMatrix {
    /// Unique identifier, and the key the manager stores this matrix under
    /// (CauseAndEffectMatrix.vb:25).
    pub id: String,
    /// Human-readable label. **This is what
    /// [`crate::dynamics::manager::DynamicsManager::cause_and_effect_matrix_by_description`]
    /// looks up by** — upstream's `GetCauseAndEffectMatrix` matches on
    /// `Description` (Manager.vb:205-209). (CauseAndEffectMatrix.vb:27.)
    pub description: String,
    /// The rows, keyed by [`CauseAndEffectItem::id`]
    /// (CauseAndEffectMatrix.vb:29).
    pub items: BTreeMap<String, CauseAndEffectItem>,
}

impl CauseAndEffectMatrix {
    /// An empty matrix with the given ID and description.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        CauseAndEffectMatrix {
            id: id.into(),
            description: description.into(),
            items: BTreeMap::new(),
        }
    }

    /// Insert (or replace) a row, keyed by its own ID — the shape upstream's
    /// `LoadData` reload uses (`Items.Add(cei.ID, cei)`,
    /// CauseAndEffectMatrix.vb:49).
    pub fn insert(&mut self, item: CauseAndEffectItem) -> Option<CauseAndEffectItem> {
        self.items.insert(item.id.clone(), item)
    }
}

/// Apply one row's effect: write its value onto the flowsheet.
///
/// Ports `DoAlarmEffect` (FormDynamicsIntegratorControl.vb:209-215) exactly —
/// look the object up, convert the value out of its display units, write it.
/// There is no guard against writing the same value repeatedly; the run loop
/// calls this on every step the alarm is active.
///
/// # Errors
///
/// [`DynamicsError::ObjectNotFound`] or
/// [`DynamicsError::PropertyNotAvailable`] where upstream would throw
/// `KeyNotFoundException` or fail inside `SetPropertyValue`.
pub fn do_alarm_effect(
    item: &CauseAndEffectItem,
    flowsheet: &mut Flowsheet,
) -> Result<(), DynamicsError> {
    set_property_value(flowsheet, &item.target, item.value_in_internal_units())
}

/// Walk a whole matrix, firing the effect of every enabled row whose alarm is
/// active. Returns how many effects were applied.
///
/// Ports `ProcessCEMatrix` (FormDynamicsIntegratorControl.vb:187-207), with the
/// indicator re-evaluation folded in (see the module header's divergence note).
/// Rows are visited in item-ID order.
///
/// # Errors
///
/// - [`DynamicsError::IndicatorNotFound`] if a row names an indicator with no
///   registered state — upstream's `DirectCast(..., IIndicator)` would throw
///   `InvalidCastException` (:94).
/// - Whatever [`IndicatorState::update`] or [`do_alarm_effect`] returns.
pub fn process_ce_matrix(
    matrix: &CauseAndEffectMatrix,
    indicators: &mut BTreeMap<ObjectId, IndicatorState>,
    flowsheet: &mut Flowsheet,
) -> Result<usize, DynamicsError> {
    let mut applied = 0usize;
    for item in matrix.items.values() {
        if !item.enabled {
            continue;
        }
        let active = {
            let indicator = indicators
                .get_mut(&item.associated_indicator)
                .ok_or_else(|| {
                    DynamicsError::IndicatorNotFound(item.associated_indicator.0.clone())
                })?;
            indicator.update(flowsheet)?;
            indicator.is_active(item.associated_indicator_alarm)
        };
        if active {
            do_alarm_effect(item, flowsheet)?;
            applied += 1;
        }
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::property::DynamicProperty;
    use crate::flowsheet::objects::ObjectType;

    fn flowsheet_with_stream() -> (Flowsheet, ObjectId) {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        (fs, id)
    }

    #[test]
    fn alarm_enum_discriminants_match_upstream() {
        assert_eq!(DynamicsAlarmType::VeryLow.upstream_value(), 0);
        assert_eq!(DynamicsAlarmType::Low.upstream_value(), 1);
        assert_eq!(DynamicsAlarmType::High.upstream_value(), 2);
        assert_eq!(DynamicsAlarmType::VeryHigh.upstream_value(), 3);
    }

    #[test]
    fn indicator_alarms_use_display_units_and_non_strict_comparisons() {
        let (mut fs, id) = flowsheet_with_stream();
        // 298.15 K == 25 degC; alarm setpoints are in degC.
        let mut ind = IndicatorState::new(
            PropertyRef::new(id.clone(), DynamicProperty::Temperature),
            "C",
        )
        .with_high_alarms(25.0, 80.0);
        ind.update(&fs).unwrap();
        assert!((ind.current_value - 25.0).abs() < 1e-9);
        assert!(
            ind.high_alarm_active,
            "value sitting on the setpoint alarms"
        );
        assert!(!ind.very_high_alarm_active);

        // Push it over the very-high setpoint.
        set_property_value(
            &mut fs,
            &PropertyRef::new(id, DynamicProperty::Temperature),
            373.15,
        )
        .unwrap();
        ind.update(&fs).unwrap();
        assert!(ind.high_alarm_active && ind.very_high_alarm_active);
    }

    #[test]
    fn disabled_alarms_never_activate() {
        let (fs, id) = flowsheet_with_stream();
        let mut ind = IndicatorState::new(PropertyRef::new(id, DynamicProperty::Temperature), "K");
        ind.high_alarm_value = 0.0; // would trip if it were armed
        ind.update(&fs).unwrap();
        assert!(!ind.high_alarm_active);
    }

    #[test]
    fn matrix_fires_the_effect_only_while_the_alarm_is_active() {
        let (mut fs, stream) = flowsheet_with_stream();
        let indicator_id = ObjectId::from("IND-1");

        let mut indicators = BTreeMap::new();
        indicators.insert(
            indicator_id.clone(),
            IndicatorState::new(
                PropertyRef::new(stream.clone(), DynamicProperty::Temperature),
                "K",
            )
            .with_high_alarms(350.0, 400.0),
        );

        let mut matrix = CauseAndEffectMatrix::new("cem-1", "Trips");
        matrix.insert(CauseAndEffectItem::new(
            "trip-1",
            indicator_id,
            DynamicsAlarmType::High,
            PropertyRef::new(stream.clone(), DynamicProperty::MassFlow),
            0.0,
            "kg/s",
        ));

        // Below the setpoint: nothing fires, flow untouched.
        let fired = process_ce_matrix(&matrix, &mut indicators, &mut fs).unwrap();
        assert_eq!(fired, 0);
        let flow = property_value(
            &fs,
            &PropertyRef::new(stream.clone(), DynamicProperty::MassFlow),
        )
        .unwrap();
        assert!((flow - 1.0).abs() < 1e-12);

        // Cross the setpoint: the effect trips the flow to zero.
        set_property_value(
            &mut fs,
            &PropertyRef::new(stream.clone(), DynamicProperty::Temperature),
            360.0,
        )
        .unwrap();
        let fired = process_ce_matrix(&matrix, &mut indicators, &mut fs).unwrap();
        assert_eq!(fired, 1);
        let flow =
            property_value(&fs, &PropertyRef::new(stream, DynamicProperty::MassFlow)).unwrap();
        assert!((flow - 0.0).abs() < 1e-12);
    }

    #[test]
    fn disabled_rows_are_skipped() {
        let (mut fs, stream) = flowsheet_with_stream();
        let indicator_id = ObjectId::from("IND-1");
        let mut indicators = BTreeMap::new();
        indicators.insert(
            indicator_id.clone(),
            IndicatorState::new(
                PropertyRef::new(stream.clone(), DynamicProperty::Temperature),
                "K",
            )
            .with_high_alarms(0.0, 0.0),
        );
        let mut matrix = CauseAndEffectMatrix::new("cem-1", "Trips");
        let mut item = CauseAndEffectItem::new(
            "trip-1",
            indicator_id,
            DynamicsAlarmType::High,
            PropertyRef::new(stream, DynamicProperty::MassFlow),
            0.0,
            "kg/s",
        );
        item.enabled = false;
        matrix.insert(item);
        assert_eq!(
            process_ce_matrix(&matrix, &mut indicators, &mut fs).unwrap(),
            0
        );
    }

    #[test]
    fn a_row_naming_an_unregistered_indicator_is_an_error() {
        let (mut fs, stream) = flowsheet_with_stream();
        let mut indicators = BTreeMap::new();
        let mut matrix = CauseAndEffectMatrix::new("cem-1", "Trips");
        matrix.insert(CauseAndEffectItem::new(
            "trip-1",
            ObjectId::from("missing"),
            DynamicsAlarmType::High,
            PropertyRef::new(stream, DynamicProperty::MassFlow),
            0.0,
            "kg/s",
        ));
        assert!(matches!(
            process_ce_matrix(&matrix, &mut indicators, &mut fs),
            Err(DynamicsError::IndicatorNotFound(_))
        ));
    }
}
