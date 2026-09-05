//! Flowsheet results report: tabulate every object's state into rows, then
//! render them as plain text.
//!
//! # What this does
//!
//! [`build_report_rows`] walks the flowsheet in insertion order and emits one
//! [`ReportRow`] per property per object — for a material stream, the overall
//! condition, then one block per *present* phase, each followed by that phase's
//! per-compound mole fractions. [`render_text_report`] then groups those rows by
//! object and lays them out as an aligned, monospaced table.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM**'s `DWSIM.FlowsheetBase/ReportCreator.vb`
//! (365 lines), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software.
//!
//! Source regions ported here:
//!
//! - `ReportCreator.vb` lines 18-158 — `FillDataTable`, the row-assembly
//!   algorithm: the five-column row shape
//!   (tag / description / property / value / unit), the material-stream
//!   special case with its per-phase blocks each gated on
//!   `Phases(i).Properties.massflow.HasValue` (:67, :81, :95, :109, :123, :137),
//!   the per-compound mole-fraction sub-blocks (:63-66, :76-79, ...), and the
//!   "everything else just lists its properties" fallback (:151-155). Ported as
//!   [`build_report_rows`].
//! - `ReportCreator.vb` lines 200-257 (inside `CreateAndSaveODTFile`) and
//!   292-343 (inside `CreateAndSaveODSFile`) — the identical
//!   **grouping loop** both writers run over the filled table: emit a bold
//!   `"Object: <tag> (<description>)"` header, then every consecutive row
//!   sharing that tag, then a blank gap before the next object. Ported as
//!   [`render_text_report`].
//!
//! # Excluded DWSIM behavior
//!
//! - **Both document writers.** `CreateAndSaveODTFile` (:160-277) and
//!   `CreateAndSaveODSFile` (:279-363) drive the **AODL** OpenDocument library
//!   — `TextDocument`, `TableBuilder`, `ParagraphBuilder`, font names, point
//!   sizes, cell alignment, `OnDiskPackageWriter`. That is document formatting
//!   against a .NET dependency, not algorithm; only the grouping loop inside
//!   them is ported, and it emits text.
//! - **`GetPropertyValue` / `GetProperties` / `GetPropertyUnit`.** Upstream
//!   obtains every value by reflecting over the object's property grid and
//!   slicing the returned name array at hard-coded index boundaries
//!   (`r1 = 5, r2 = 12, r3 = 30, r4 = 48, r5 = 66, r6 = 84`, :34-39, and the
//!   bare `131 To 145` at :138). Property-grid reflection is excluded by the
//!   porting brief, and those magic indices are unmaintainable besides — a
//!   reordering of the property list silently reports the wrong rows. This port
//!   replaces them with two **explicit** property lists,
//!   [`OVERALL_PROPERTIES`] and [`PHASE_PROPERTIES`], read directly off the
//!   ported [`crate::flowsheet::streams::PhaseProperties`]. The row *shape* and
//!   the block *structure* are upstream's; the property selection is this
//!   port's, and is stated rather than implied.
//! - **Localisation.** `frm.GetTranslatedString(...)` wraps every label
//!   upstream (:44, :50, :63, ...); the resource-manager layer is excluded, so
//!   labels are plain English.
//! - **Display-unit conversion.** Upstream converts each value into the user's
//!   selected unit system (`su`, :20, :48) and formats it with the flowsheet's
//!   `NumberFormat` (:21). This port reports **SI-internal values** with the
//!   units [`crate::flowsheet::streams`] documents (note: enthalpy kJ/kg,
//!   entropy kJ/(kg·K), molecular weight kg/kmol, energy flow kW), and takes the
//!   decimal precision from [`ReportOptions::decimals`].
//! - **`Phases(5)` (Liquid3).** Upstream's `FillDataTable` reports phases 2, 1,
//!   3, 4, 6 and 7 and simply never looks at 5 (compare :67-150). That gap is
//!   reproduced, and named here so it is not mistaken for an oversight in the
//!   port: see [`REPORTED_PHASES`].

use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::{FlowsheetObject, ObjectData};
use crate::flowsheet::streams::{MaterialStreamData, PhaseIndex, PhaseProperties};

/// The phase slots a material-stream report covers, in upstream's order —
/// vapour, overall liquid, liquid 1, liquid 2, aqueous, solid
/// (ReportCreator.vb:67, :81, :95, :109, :123, :137).
///
/// [`PhaseIndex::Liquid3`] is **absent**, exactly as upstream: `FillDataTable`
/// never reads `Phases(5)`. That is upstream's behaviour, reproduced
/// deliberately, not a porting gap.
pub const REPORTED_PHASES: [PhaseIndex; 6] = [
    PhaseIndex::Vapor,
    PhaseIndex::OverallLiquid,
    PhaseIndex::Liquid1,
    PhaseIndex::Liquid2,
    PhaseIndex::Aqueous,
    PhaseIndex::Solid,
];

/// Overall (mixture-slot) properties reported for a material stream, as
/// `(label, unit)` pairs.
///
/// Replaces upstream's `properties(0 .. r2 - 1)` reflection slice
/// (ReportCreator.vb:47-62) with an explicit list — see the module's
/// "Excluded DWSIM behavior". Units are the SI-internal ones documented in
/// [`crate::flowsheet::streams`].
pub const OVERALL_PROPERTIES: [(&str, &str); 10] = [
    ("Temperature", "K"),
    ("Pressure", "Pa"),
    ("Mass Flow", "kg/s"),
    ("Molar Flow", "mol/s"),
    ("Volumetric Flow", "m3/s"),
    ("Mass Enthalpy", "kJ/kg"),
    ("Mass Entropy", "kJ/[kg.K]"),
    ("Molecular Weight", "kg/kmol"),
    ("Density", "kg/m3"),
    ("Vapor Phase Molar Fraction", "-"),
];

/// Per-phase properties reported for each present phase of a material stream,
/// as `(label, unit)` pairs.
///
/// Replaces upstream's `properties(r2 .. r3 - 1)` and its five sibling slices
/// (ReportCreator.vb:68-145) with an explicit list.
pub const PHASE_PROPERTIES: [(&str, &str); 13] = [
    ("Phase Molar Fraction", "-"),
    ("Phase Mass Fraction", "-"),
    ("Mass Flow", "kg/s"),
    ("Molar Flow", "mol/s"),
    ("Volumetric Flow", "m3/s"),
    ("Density", "kg/m3"),
    ("Mass Enthalpy", "kJ/kg"),
    ("Mass Entropy", "kJ/[kg.K]"),
    ("Molecular Weight", "kg/kmol"),
    ("Heat Capacity (Cp)", "kJ/[kg.K]"),
    ("Thermal Conductivity", "W/[m.K]"),
    ("Viscosity", "Pa.s"),
    ("Compressibility Factor", "-"),
];

/// One row of the report table — DWSIM's five `DataTable` columns
/// (ReportCreator.vb:23-27: `Nome`, `Tipo`, `Propriedade`, `Valor`,
/// `Unidade`).
///
/// A row whose `value` is `None` and `unit` is empty is a **section heading**,
/// exactly as upstream emits for the per-phase composition blocks
/// (ReportCreator.vb:63: `{tag, description, "FraomolarnaMistura", "", ""}`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReportRow {
    /// The object's user-visible tag (upstream column `Nome`, fed by
    /// `GraphicObject.Tag`). Rows are grouped by this.
    pub object_tag: String,
    /// The object's description (upstream column `Tipo`, fed by
    /// `GraphicObject.Description`).
    pub description: String,
    /// Property name or section heading (upstream column `Propriedade`).
    pub property: String,
    /// The value in SI-internal units, or `None` for a heading row or an
    /// uncalculated property (upstream column `Valor`).
    pub value: Option<f64>,
    /// Unit label for `value`, empty for headings (upstream column `Unidade`).
    pub unit: String,
}

impl ReportRow {
    /// Whether this row is a section heading rather than a value.
    #[must_use]
    pub fn is_heading(&self) -> bool {
        self.value.is_none() && self.unit.is_empty()
    }
}

/// Rendering options for [`render_text_report`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReportOptions {
    /// Report title. Upstream hard-codes `"DWSIM Simulation Results Report"`
    /// (ReportCreator.vb:178); this port names the fork instead and lets the
    /// caller override.
    pub title: String,
    /// Optional source-file line, upstream's `"Simulation File: " & frm.FilePath`
    /// (ReportCreator.vb:184). `None` omits the line.
    pub file_path: Option<String>,
    /// Optional creation-date line, upstream's `"Date created: " & Date.Now`
    /// (ReportCreator.vb:190). Supplied by the caller as a formatted string —
    /// this crate deliberately reads no clock, so a report is reproducible and
    /// the library stays free of a time dependency. `None` omits the line.
    pub date_created: Option<String>,
    /// Decimal places used to format every value — the stand-in for upstream's
    /// flowsheet `NumberFormat` (ReportCreator.vb:21).
    pub decimals: usize,
}

impl Default for ReportOptions {
    /// Title `"OUTRAM PARK Simulation Results Report"`, no file path, no date,
    /// six decimal places.
    fn default() -> Self {
        ReportOptions {
            title: "OUTRAM PARK Simulation Results Report".to_string(),
            file_path: None,
            date_created: None,
            decimals: 6,
        }
    }
}

/// Read one named per-phase property off a [`PhaseProperties`] bag.
///
/// The `label` must be one from [`PHASE_PROPERTIES`]; an unknown label reads as
/// `None` (reported as "not calculated") rather than panicking.
fn phase_property_value(p: &PhaseProperties, label: &str) -> Option<f64> {
    match label {
        "Phase Molar Fraction" => p.molarfraction,
        "Phase Mass Fraction" => p.massfraction,
        "Mass Flow" => p.massflow,
        "Molar Flow" => p.molarflow,
        "Volumetric Flow" => p.volumetric_flow,
        "Density" => p.density,
        "Mass Enthalpy" => p.enthalpy,
        "Mass Entropy" => p.entropy,
        "Molecular Weight" => p.molecular_weight,
        "Heat Capacity (Cp)" => p.heat_capacity_cp,
        "Thermal Conductivity" => p.thermal_conductivity,
        "Viscosity" => p.viscosity,
        "Compressibility Factor" => p.compressibility_factor,
        _ => None,
    }
}

/// Read one named overall property off a material stream.
fn overall_property_value(ms: &MaterialStreamData, label: &str) -> Option<f64> {
    let p = &ms.phase(PhaseIndex::Mixture).properties;
    match label {
        "Temperature" => p.temperature,
        "Pressure" => p.pressure,
        "Mass Flow" => p.massflow,
        "Molar Flow" => p.molarflow,
        "Volumetric Flow" => p.volumetric_flow,
        "Mass Enthalpy" => p.enthalpy,
        "Mass Entropy" => p.entropy,
        "Molecular Weight" => p.molecular_weight,
        "Density" => p.density,
        "Vapor Phase Molar Fraction" => ms.phase(PhaseIndex::Vapor).properties.molarfraction,
        _ => None,
    }
}

/// Emit the rows for one material stream: the overall block, its overall
/// composition, then one block per present phase with that phase's composition
/// — the structure of `FillDataTable`'s `If objtype = ObjectType.MaterialStream`
/// branch (ReportCreator.vb:45-150).
fn material_stream_rows(
    object: &FlowsheetObject,
    ms: &MaterialStreamData,
    out: &mut Vec<ReportRow>,
) {
    let tag = &object.tag;
    let desc = &object.description;
    let mut push = |property: String, value: Option<f64>, unit: &str| {
        out.push(ReportRow {
            object_tag: tag.clone(),
            description: desc.clone(),
            property,
            value,
            unit: unit.to_string(),
        });
    };

    for (label, unit) in OVERALL_PROPERTIES {
        push(label.to_string(), overall_property_value(ms, label), unit);
    }

    // Overall composition block (ReportCreator.vb:63-66).
    push("Molar Fraction in the Mixture".to_string(), None, "");
    for c in &ms.phase(PhaseIndex::Mixture).compounds {
        push(c.name.clone(), c.mole_fraction, "-");
    }

    // One block per present phase. Upstream's gate is
    // `Phases(i).Properties.massflow.HasValue` (:67, :81, :95, :109, :123, :137).
    for phase_index in REPORTED_PHASES {
        let phase = ms.phase(phase_index);
        if phase.properties.massflow.is_none() {
            continue;
        }
        push(
            format!("--- Phase: {}", phase_index.upstream_name()),
            None,
            "",
        );
        for (label, unit) in PHASE_PROPERTIES {
            push(
                label.to_string(),
                phase_property_value(&phase.properties, label),
                unit,
            );
        }
        push(
            format!(
                "Molar Fraction in the {} Phase",
                phase_index.upstream_name()
            ),
            None,
            "",
        );
        for c in &phase.compounds {
            push(c.name.clone(), c.mole_fraction, "-");
        }
    }
}

/// Assemble the report rows for a whole flowsheet — port of
/// `ReportCreator.FillDataTable` (ReportCreator.vb:18-158).
///
/// Objects are visited in the flowsheet's insertion order (upstream iterates
/// `frm.SimulationObjects.Values`, whose .NET dictionary is insertion-ordered),
/// so the output is deterministic.
///
/// Row shape per object type:
///
/// - **Material stream** — the block structure described on
///   [`material_stream_rows`]: overall properties, overall composition, then one
///   block per phase whose mass flow is set.
/// - **Energy stream** — a single `Energy Flow` row \[kW\].
/// - **Everything else** — the object's net power (if calculated) followed by
///   its recorded scalar results in sorted key order. This stands in for
///   upstream's `For Each prop In properties` reflection fallback
///   (ReportCreator.vb:151-155), which this port cannot use (property-grid
///   reflection is excluded).
///
/// Drawing-only objects are skipped: they have no properties to report and
/// upstream never gives them a `GetProperties` implementation worth printing.
#[must_use]
pub fn build_report_rows(flowsheet: &Flowsheet) -> Vec<ReportRow> {
    let mut rows = Vec::new();
    for object in flowsheet.iter() {
        if object.object_type.is_drawing_only() {
            continue;
        }
        match &object.data {
            ObjectData::Material(ms) => material_stream_rows(object, ms, &mut rows),
            ObjectData::Energy(es) => rows.push(ReportRow {
                object_tag: object.tag.clone(),
                description: object.description.clone(),
                property: "Energy Flow".to_string(),
                value: es.energy_flow,
                unit: "kW".to_string(),
            }),
            ObjectData::UnitOperation { power, results } => {
                rows.push(ReportRow {
                    object_tag: object.tag.clone(),
                    description: object.description.clone(),
                    property: "Power Generated/Consumed".to_string(),
                    value: *power,
                    unit: "kW".to_string(),
                });
                let mut keys: Vec<&String> = results.keys().collect();
                keys.sort();
                for k in keys {
                    rows.push(ReportRow {
                        object_tag: object.tag.clone(),
                        description: object.description.clone(),
                        property: k.clone(),
                        value: Some(results[k]),
                        unit: String::new(),
                    });
                }
            }
        }
    }
    rows
}

/// Format one value for the report: `decimals` fixed decimal places, or `"-"`
/// when the property has not been calculated (upstream prints whatever
/// `GetPropertyValue` returned, including empty strings).
fn format_value(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(v) if v.is_finite() => format!("{v:.decimals$}"),
        Some(v) => format!("{v}"), // NaN / inf, printed honestly rather than hidden
        None => "-".to_string(),
    }
}

/// Render report rows as an aligned plain-text table, grouped by object — port
/// of the grouping loop both upstream writers share
/// (ReportCreator.vb:200-257 and :292-343).
///
/// Upstream's loop walks the filled table, opens a bold
/// `"Object: <tag> (<description>)"` paragraph whenever the tag changes, writes
/// each row as three cells (property, right-aligned value, unit), and leaves two
/// blank rows between objects (`j = j + 2`, :253). This renders the same
/// structure as text: a header line, an underline, one line per row with the
/// value right-aligned in its column, and a blank line between objects.
///
/// Returns an empty-bodied report (header only) for a flowsheet with no
/// reportable objects — upstream would divide by zero on `DT.Rows(0)` in that
/// case (:208).
#[must_use]
pub fn render_rows_as_text(rows: &[ReportRow], options: &ReportOptions) -> String {
    let mut out = String::new();
    out.push_str(&options.title);
    out.push('\n');
    if let Some(path) = &options.file_path {
        out.push_str(&format!("Simulation File: {path}\n"));
    }
    if let Some(date) = &options.date_created {
        out.push_str(&format!("Date created: {date}\n"));
    }
    out.push('\n');

    if rows.is_empty() {
        out.push_str("(no objects to report)\n");
        return out;
    }

    // Column widths, computed once so every object's table lines up.
    let prop_w = rows
        .iter()
        .map(|r| r.property.chars().count())
        .max()
        .unwrap_or(0);
    let val_w = rows
        .iter()
        .map(|r| format_value(r.value, options.decimals).chars().count())
        .max()
        .unwrap_or(0);

    let mut current: Option<&str> = None;
    for row in rows {
        if current != Some(row.object_tag.as_str()) {
            if current.is_some() {
                out.push('\n');
            }
            let header = if row.description.is_empty() {
                format!("Object: {}", row.object_tag)
            } else {
                format!("Object: {} ({})", row.object_tag, row.description)
            };
            out.push_str(&header);
            out.push('\n');
            out.push_str(&"-".repeat(header.chars().count()));
            out.push('\n');
            current = Some(row.object_tag.as_str());
        }
        if row.is_heading() {
            out.push_str(&format!("{}\n", row.property));
        } else {
            out.push_str(&format!(
                "{:<prop_w$}  {:>val_w$}  {}\n",
                row.property,
                format_value(row.value, options.decimals),
                row.unit
            ));
        }
    }
    out
}

/// Build and render a flowsheet's results report in one call — the composition
/// of [`build_report_rows`] and [`render_rows_as_text`], mirroring upstream's
/// `FillDataTable()` followed by a writer (ReportCreator.vb:162, :281).
#[must_use]
pub fn render_text_report(flowsheet: &Flowsheet, options: &ReportOptions) -> String {
    render_rows_as_text(&build_report_rows(flowsheet), options)
}

#[cfg(test)]
mod tests {
    //! # Verification tests — report assembly
    //!
    //! **Methodology.** Verification that the ported row assembly reproduces
    //! `FillDataTable`'s block structure (ReportCreator.vb:45-158) — in
    //! particular the per-phase gate on `massflow.HasValue` and the composition
    //! sub-blocks — and that the text renderer reproduces the shared grouping
    //! loop (:200-257). No physics is evaluated. Results recorded 2026-08-11.

    use super::*;
    use crate::flowsheet::objects::{ObjectData, ObjectType};

    fn flowsheet_with_one_stream() -> Flowsheet {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("FEED-1"));
        let o = fs.object_mut(&id).unwrap();
        o.description = "Feed".to_string();
        let ms = o.data.as_material_mut().unwrap();
        ms.add_compound("Water", 18.015);
        ms.add_compound("Ethanol", 46.069);
        ms.equalize_overall_composition();
        ms.phase_mut(PhaseIndex::Mixture).properties.molarflow = Some(55.0);
        fs
    }

    /// **Methodology.** A stream with no phase split must produce exactly the
    /// overall block plus the overall-composition heading and its two compound
    /// rows: 10 + 1 + 2 = 13 rows (ReportCreator.vb:47-66), with no phase blocks
    /// because every `Phases(i).Properties.massflow` is `Nothing` (:67 etc.).
    /// **Result (2026-08-11):** 13 rows; the heading row is a heading;
    /// `Temperature = 298.150000 K`; no `--- Phase:` rows.
    #[test]
    fn unflashed_stream_reports_overall_block_only() {
        let fs = flowsheet_with_one_stream();
        let rows = build_report_rows(&fs);
        assert_eq!(rows.len(), 13);
        assert_eq!(rows[0].property, "Temperature");
        assert_eq!(rows[0].unit, "K");
        assert_eq!(rows[0].value, Some(298.15));
        assert_eq!(rows[0].object_tag, "FEED-1");
        assert_eq!(rows[0].description, "Feed");
        assert!(rows[10].is_heading());
        assert_eq!(rows[10].property, "Molar Fraction in the Mixture");
        assert_eq!(rows[11].property, "Water");
        assert_eq!(rows[11].value, Some(0.5));
        assert!(!rows.iter().any(|r| r.property.starts_with("--- Phase")));
    }

    /// **Methodology.** The per-phase gate (ReportCreator.vb:67, :81, ...):
    /// setting the vapour slot's mass flow must add exactly one phase block —
    /// heading + 13 properties + composition heading + 2 compounds = 17 rows —
    /// and setting `Liquid3`'s must add nothing, because upstream never reports
    /// phase 5 (see [`REPORTED_PHASES`]).
    /// **Result (2026-08-11):** 13 -> 30 rows after the vapour slot is
    /// populated; still 30 after `Liquid3` is populated.
    #[test]
    fn phase_blocks_appear_only_for_phases_with_a_mass_flow() {
        let mut fs = flowsheet_with_one_stream();
        let id = fs.object_ids()[0].clone();
        {
            let ms = fs.object_mut(&id).unwrap().data.as_material_mut().unwrap();
            ms.phase_mut(PhaseIndex::Vapor).properties.massflow = Some(0.4);
            ms.phase_mut(PhaseIndex::Vapor).properties.molarfraction = Some(0.35);
            ms.set_phase_composition(PhaseIndex::Vapor, &[0.2, 0.8])
                .unwrap();
        }
        let rows = build_report_rows(&fs);
        assert_eq!(rows.len(), 30);
        let heading = rows
            .iter()
            .position(|r| r.property == "--- Phase: Vapor")
            .unwrap();
        assert_eq!(rows[heading + 1].property, "Phase Molar Fraction");
        assert_eq!(rows[heading + 1].value, Some(0.35));
        assert_eq!(rows[heading + 3].property, "Mass Flow");
        assert_eq!(rows[heading + 3].value, Some(0.4));
        assert_eq!(
            rows[heading + 14].property,
            "Molar Fraction in the Vapor Phase"
        );
        assert_eq!(rows[heading + 16].value, Some(0.8));

        {
            let ms = fs.object_mut(&id).unwrap().data.as_material_mut().unwrap();
            ms.phase_mut(PhaseIndex::Liquid3).properties.massflow = Some(0.1);
        }
        assert_eq!(
            build_report_rows(&fs).len(),
            30,
            "Liquid3 is never reported upstream (ReportCreator.vb:67-150)"
        );
    }

    /// **Methodology.** Non-stream objects take the fallback path
    /// (ReportCreator.vb:151-155): this port emits the net power plus the
    /// recorded scalar results in sorted key order. Energy streams emit a single
    /// `Energy Flow` row.
    /// **Result (2026-08-11):** the pump yields 3 rows —
    /// `Power Generated/Consumed = -12 kW`, then `Efficiency`, then `Head` in
    /// sorted order; the energy stream yields 1 row of 12 kW.
    #[test]
    fn unit_operations_and_energy_streams_get_their_own_row_shapes() {
        let mut fs = Flowsheet::new();
        let pump = fs.add_object(ObjectType::Pump, None);
        if let ObjectData::UnitOperation { power, results } =
            &mut fs.object_mut(&pump).unwrap().data
        {
            *power = Some(-12.0);
            results.insert("Head".to_string(), 42.0);
            results.insert("Efficiency".to_string(), 0.75);
        }
        let e = fs.add_object(ObjectType::EnergyStream, None);
        fs.object_mut(&e)
            .unwrap()
            .data
            .as_energy_mut()
            .unwrap()
            .set_value_kw(12.0);

        let rows = build_report_rows(&fs);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].property, "Power Generated/Consumed");
        assert_eq!(rows[0].value, Some(-12.0));
        assert_eq!(rows[1].property, "Efficiency");
        assert_eq!(rows[2].property, "Head");
        assert_eq!(rows[3].property, "Energy Flow");
        assert_eq!(rows[3].value, Some(12.0));
        assert_eq!(rows[3].unit, "kW");
    }

    /// **Methodology.** The grouping loop (ReportCreator.vb:200-257): one
    /// `Object: <tag> (<description>)` header per object, rows underneath, blank
    /// line between objects. Uncalculated values render as `-`.
    /// **Result (2026-08-11):** the header line
    /// `Object: FEED-1 (Feed)` appears once; the pump's header appears once; a
    /// blank line separates them; `Molar Flow` shows `55.000000` and
    /// `Density` shows `-`.
    #[test]
    fn text_rendering_groups_by_object() {
        let mut fs = flowsheet_with_one_stream();
        let pump = fs.add_object(ObjectType::Pump, None);
        if let ObjectData::UnitOperation { power, .. } = &mut fs.object_mut(&pump).unwrap().data {
            *power = Some(3.5);
        }
        let opts = ReportOptions {
            title: "T".to_string(),
            file_path: Some("/tmp/case.dwxmz".to_string()),
            date_created: Some("2026-08-11".to_string()),
            decimals: 6,
        };
        let text = render_text_report(&fs, &opts);

        assert!(text.starts_with("T\nSimulation File: /tmp/case.dwxmz\nDate created: 2026-08-11\n"));
        assert_eq!(text.matches("Object: FEED-1 (Feed)").count(), 1);
        assert_eq!(text.matches("Object: PUMP-1").count(), 1);
        assert!(text.contains("55.000000"), "molar flow formatted");
        assert!(
            text.lines()
                .any(|l| l.starts_with("Density") && l.contains('-')),
            "uncalculated property renders as -"
        );
        assert!(
            text.contains("\n\nObject: PUMP-1"),
            "blank line between objects"
        );
        assert!(text.contains("Molar Fraction in the Mixture\n"));
    }

    /// **Methodology.** An empty flowsheet must render a header-only report
    /// instead of crashing — upstream indexes `DT.Rows(0)` unconditionally
    /// (ReportCreator.vb:208) and would throw.
    /// **Result (2026-08-11):** the body reads `(no objects to report)`.
    #[test]
    fn empty_flowsheet_renders_without_panicking() {
        let fs = Flowsheet::new();
        let text = render_text_report(&fs, &ReportOptions::default());
        assert!(text.contains("(no objects to report)"));
        assert!(build_report_rows(&fs).is_empty());
    }

    /// **Methodology.** Value formatting honours [`ReportOptions::decimals`] and
    /// reports non-finite values honestly rather than hiding them.
    /// **Result (2026-08-11):** 2 decimals gives `1.23`; `None` gives `-`;
    /// `NaN` prints as `NaN`.
    #[test]
    fn value_formatting_is_explicit_about_missing_and_non_finite_values() {
        assert_eq!(format_value(Some(1.23456), 2), "1.23");
        assert_eq!(format_value(Some(1.23456), 4), "1.2346");
        assert_eq!(format_value(None, 6), "-");
        assert_eq!(format_value(Some(f64::NAN), 6), "NaN");
    }
}
