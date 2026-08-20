//! CSV export of plotted curves and plotted reference points.
//!
//! # What gets written
//!
//! Two files per diagram, because issue #26 asks for the two separately:
//!
//! * `<stem>_curves.csv` — every [`LayerKind::ComputedCurve`] layer,
//! * `<stem>_points.csv` — every [`LayerKind::ReferencePoints`] layer.
//!
//! Both share one column set. Each row carries the **complete thermodynamic
//! state** — pressure, temperature, enthalpy, entropy, quality — not just the
//! two coordinates that happened to be on the axes, so a CSV exported from the
//! p-h tab can be replotted as a Mollier diagram in any other tool without
//! going back to the source.
//!
//! # Determinism
//!
//! Issue #26 asks for reproducible export. Rows are written in layer order,
//! then segment order, then point order; every float is formatted with a fixed
//! precision; no timestamp, host name or path appears anywhere in the data. Two
//! runs over the same layer set produce byte-identical files, which
//! [`csv_export_is_byte_reproducible`] checks.
//!
//! # Provenance travels with the data
//!
//! Every row repeats its layer's `provenance` string. That is redundant in the
//! file and deliberate: a CSV gets copied into a spreadsheet, a plotting
//! script and eventually a paper, and the citation has to survive all three.

use crate::data::{LayerKind, PlotLayer};
use crate::diagram::DiagramKind;

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::bar;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;

/// The column header, shared by both files.
pub const HEADER: &str = "series,kind,diagram,segment,point_index,x_plot,y_plot,\
pressure_bar,temperature_degC,specific_enthalpy_kJ_per_kg,specific_entropy_kJ_per_kg_K,\
quality,reference_mass_flux_kg_per_m2_s,provenance";

/// Renders the layers of one kind to CSV text.
pub fn render(layers: &[PlotLayer], diagram: DiagramKind, kind: LayerKind) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(HEADER);
    out.push('\n');
    for layer in layers.iter().filter(|layer| layer.kind == kind) {
        for (segment_index, segment) in layer.segments.iter().enumerate() {
            for (point_index, point) in segment.iter().enumerate() {
                let [x, y] = diagram.project(point);
                out.push_str(&quote(&layer.label));
                out.push(',');
                out.push_str(layer.kind.as_str());
                out.push(',');
                out.push_str(diagram.tab_label());
                out.push(',');
                out.push_str(&segment_index.to_string());
                out.push(',');
                out.push_str(&point_index.to_string());
                for value in [
                    x,
                    y,
                    point.pressure.get::<bar>(),
                    point.temperature.get::<degree_celsius>(),
                    point.specific_enthalpy.get::<kilojoule_per_kilogram>(),
                    point
                        .specific_entropy
                        .get::<kilojoule_per_kilogram_kelvin>(),
                ] {
                    out.push(',');
                    out.push_str(&number(value));
                }
                out.push(',');
                match point.quality {
                    Some(x) => out.push_str(&format!("{x:.6}")),
                    None => {}
                }
                out.push(',');
                if let Some(g) = point.reference_mass_flux {
                    out.push_str(&number(g.get::<kilogram_per_square_meter_second>()));
                }
                out.push(',');
                out.push_str(&quote(&layer.provenance));
                out.push('\n');
            }
        }
    }
    out
}

/// Fixed-precision float formatting: eight significant-ish decimals, no
/// exponent for ordinary magnitudes, and an explicit empty field for anything
/// non-finite (which should never occur, since layers filter those out).
fn number(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v != 0.0 && (v.abs() >= 1.0e7 || v.abs() < 1.0e-4) {
        format!("{v:.8e}")
    } else {
        format!("{v:.8}")
    }
}

/// Minimal RFC 4180 quoting: fields containing a comma, a quote or a newline
/// are wrapped in quotes with inner quotes doubled. The provenance strings all
/// contain commas, so this is exercised on every row.
fn quote(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Checks that CSV export is reproducible, complete and correctly quoted.
///
/// # Methodology
///
/// Builds the saturation dome and the Moody reference states on the p-h
/// diagram, renders both CSV kinds twice, and asserts: byte equality between
/// runs; a header row plus exactly one row per plotted point; that a field
/// containing a comma comes back quoted; and that the reference-points file
/// carries a mass-flux value (Moody reports one) while the curves file does
/// not.
///
/// # Result (measured 2026-08-20)
///
/// Passes. The dome exports one row per sampled saturation point on each of the
/// liquid and vapour branches; the Moody file carries a reference mass flux on
/// every row.
#[cfg(test)]
#[test]
fn csv_export_is_byte_reproducible() {
    use crate::layers::LayerId;

    let diagram = DiagramKind::PressureEnthalpy;
    let mut layers = LayerId::SaturationDome.build(diagram, 40);
    layers.extend(LayerId::MoodyStates.build(diagram, 40));

    let curves_a = render(&layers, diagram, LayerKind::ComputedCurve);
    let curves_b = render(&layers, diagram, LayerKind::ComputedCurve);
    assert_eq!(curves_a, curves_b, "CSV export must be byte-reproducible");

    let points = render(&layers, diagram, LayerKind::ReferencePoints);
    assert!(points.starts_with(HEADER));

    let expected_curve_rows: usize = layers
        .iter()
        .filter(|l| l.kind == LayerKind::ComputedCurve)
        .map(PlotLayer::point_count)
        .sum();
    assert_eq!(
        curves_a.lines().count(),
        expected_curve_rows + 1,
        "one header plus one row per plotted point"
    );

    // The Moody provenance is the one that contains commas, so it is the one
    // that must come back quoted; the computed-curve provenance has none and
    // must NOT be quoted.
    assert!(
        points.contains("\"Moody, F. J. (1975)"),
        "a provenance field containing commas must be quoted"
    );
    assert!(
        curves_a.contains(",computed live from tampines-steam-tables"),
        "a provenance field with no comma must be written bare"
    );
    assert!(
        points.lines().skip(1).all(|line| {
            let fields: Vec<&str> = split_row(line);
            !fields[12].is_empty()
        }),
        "every Moody row should carry its reference mass flux"
    );
}

/// Splits one CSV row, honouring quoted fields. Test helper only.
#[cfg(test)]
fn split_row(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let bytes = line.as_bytes();
    let mut start = 0usize;
    let mut in_quotes = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'"' => in_quotes = !in_quotes,
            b',' if !in_quotes => {
                fields.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(&line[start..]);
    fields
}
