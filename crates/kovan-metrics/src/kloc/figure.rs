//! `fig_kloc_productivity.svg` — the productivity figure, hand-rolled as SVG.
//!
//! Two stacked horizontal bar charts:
//!
//! - **(a)** code lines per active day, one bar per pre-agentic repository,
//!   then the pre-agentic aggregate, then the agentic month.
//! - **(b)** what the agentic output is made of, one bar per class.
//!
//! # Why SVG rather than the matplotlib PNG this replaces
//!
//! Three reasons, in order of weight.
//!
//! **Determinism.** A plotting library's output moves with its version, its
//! font metrics and its rasteriser. This code emits the same bytes for the same
//! numbers on any machine, which is what a reproducibility artifact needs and
//! what a 200-dpi raster cannot promise.
//!
//! **No dependency.** No plotting crate is in the workspace's shared
//! dependencies, and a chart of two bar groups does not justify adding one.
//!
//! **It suits the consumer.** The figure goes into a LaTeX manuscript, where
//! vector art scales and prints better than a raster.
//!
//! **This visibly changes the artifact** in a submitted paper — the maintainer
//! chose it on 2026-08-14 with that stated. The layout below follows the
//! matplotlib original closely (bar order, the rule separating the summary rows,
//! the hatched aggregate bar, value labels outside the bars, no top or right
//! spine) so the change is of format rather than of content.
//!
//! # Why one bar per class in (b), rather than one stacked bar
//!
//! Inherited from the original, and the reasoning still holds: the two small
//! classes are roughly 15% and 7% of the total, far too narrow to hold a legible
//! inline label, and leader lines out of adjacent thin segments cross each
//! other. Separate bars carry the same split with no collision risk, and each
//! label states its share.

use super::config::Provenance;
use super::measure::{AgenticSummary, BaselineTotals, RepoStats};
use super::report::fmt;

/// Categorical slots 1-3 of the validated reference palette (light mode).
const C_BLUE: &str = "#2a78d6";
/// Second categorical slot.
const C_ORANGE: &str = "#eb6834";
/// Third categorical slot.
const C_AQUA: &str = "#1baf7a";
/// Body text.
const INK: &str = "#0b0b0b";
/// Axis lines and tick labels.
const INK_SOFT: &str = "#52514e";
/// Grid lines.
const GRID: &str = "#d8d7d2";

/// Overall figure width, in SVG user units (≈ px at 96 dpi).
const WIDTH: f64 = 660.0;
/// Left margin, wide enough for the repository labels.
const LEFT: f64 = 250.0;
/// Right margin, leaving room for the value label outside each bar.
const RIGHT: f64 = 24.0;
/// Height of one bar row.
const ROW: f64 = 24.0;

/// Escape the five characters XML reserves.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Format a number the way the axis labels do: rounded, comma-grouped.
fn label(value: f64) -> String {
    fmt(value.round().max(0.0) as u64)
}

/// One bar in a chart.
struct Bar {
    /// Row label, drawn to the left of the axis.
    name: String,
    /// Bar length in data units.
    value: f64,
    /// Fill colour.
    colour: &'static str,
    /// Draw with diagonal hatching — used for the aggregate row, which is the
    /// same entity as the bars above it and should not spend a hue.
    hatched: bool,
    /// Text drawn beyond the bar's end.
    annotation: String,
}

/// Render one horizontal bar chart into `out`, returning the height consumed.
fn bar_chart(
    out: &mut String,
    top: f64,
    title: &str,
    axis_label: &str,
    bars: &[Bar],
    rule_after: Option<usize>,
) -> f64 {
    let plot_width = WIDTH - LEFT - RIGHT;
    let span = bars
        .iter()
        .map(|b| b.value)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    // Matches the original's 16% headroom, so a value label never runs off.
    let scale = plot_width / (span * 1.16);

    out.push_str(&format!(
        "  <text x=\"{:.1}\" y=\"{:.1}\" class=\"title\">{}</text>\n",
        8.0,
        top,
        escape(title)
    ));

    let axis_top = top + 16.0;
    let height = bars.len() as f64 * ROW;

    // Vertical grid lines behind the bars, at four intervals across the span.
    for step in 1..=4 {
        let value = span * step as f64 / 4.0;
        let x = LEFT + value * scale;
        out.push_str(&format!(
            "  <line x1=\"{x:.1}\" y1=\"{:.1}\" x2=\"{x:.1}\" y2=\"{:.1}\" class=\"grid\"/>\n",
            axis_top,
            axis_top + height
        ));
        out.push_str(&format!(
            "  <text x=\"{x:.1}\" y=\"{:.1}\" class=\"tick\" text-anchor=\"middle\">{}</text>\n",
            axis_top + height + 14.0,
            label(value)
        ));
    }

    for (index, bar) in bars.iter().enumerate() {
        let y = axis_top + index as f64 * ROW;
        let bar_height = ROW * 0.62;
        let bar_y = y + (ROW - bar_height) / 2.0;
        let width = (bar.value * scale).max(0.0);

        out.push_str(&format!(
            "  <text x=\"{:.1}\" y=\"{:.1}\" class=\"rowlabel\" text-anchor=\"end\">{}</text>\n",
            LEFT - 8.0,
            bar_y + bar_height * 0.75,
            escape(&bar.name)
        ));
        let fill = if bar.hatched {
            "url(#hatch)".to_string()
        } else {
            bar.colour.to_string()
        };
        if bar.hatched {
            // Hatch pattern is drawn over a solid fill so the hue still reads.
            out.push_str(&format!(
                "  <rect x=\"{LEFT:.1}\" y=\"{bar_y:.1}\" width=\"{width:.1}\" \
                 height=\"{bar_height:.1}\" fill=\"{}\"/>\n",
                bar.colour
            ));
        }
        out.push_str(&format!(
            "  <rect x=\"{LEFT:.1}\" y=\"{bar_y:.1}\" width=\"{width:.1}\" \
             height=\"{bar_height:.1}\" fill=\"{fill}\"/>\n"
        ));
        out.push_str(&format!(
            "  <text x=\"{:.1}\" y=\"{:.1}\" class=\"value\">{}</text>\n",
            LEFT + width + span * scale * 0.012,
            bar_y + bar_height * 0.75,
            escape(&bar.annotation)
        ));
    }

    // Rule between the individual repositories and the summary rows, so the
    // aggregate is not misread as just another repository.
    if let Some(after) = rule_after {
        if after > 0 && after < bars.len() {
            let y = axis_top + after as f64 * ROW;
            out.push_str(&format!(
                "  <line x1=\"{LEFT:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" \
                 class=\"sep\"/>\n",
                WIDTH - RIGHT
            ));
        }
    }

    // Baseline axis. No top or right spine, matching the original.
    out.push_str(&format!(
        "  <line x1=\"{LEFT:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"axis\"/>\n",
        axis_top + height,
        WIDTH - RIGHT,
        axis_top + height
    ));
    out.push_str(&format!(
        "  <text x=\"{:.1}\" y=\"{:.1}\" class=\"axislabel\" text-anchor=\"middle\">{}</text>\n",
        LEFT + plot_width / 2.0,
        axis_top + height + 34.0,
        escape(axis_label)
    ));

    height + 60.0
}

/// Render the whole figure.
pub fn productivity_svg(
    base_stats: &[RepoStats],
    base_totals: &BaselineTotals,
    agentic: &AgenticSummary,
) -> String {
    // (a) Rate of code production. Repositories slowest-first, so the fastest
    // sits nearest the summary rows, matching the original's sort.
    let mut rows: Vec<Bar> = base_stats
        .iter()
        .filter(|s| !s.missing && !s.days.is_empty())
        .map(|s| {
            let rate = s.rate().unwrap_or(0.0);
            Bar {
                name: s.key.clone(),
                value: rate,
                colour: C_BLUE,
                hatched: false,
                annotation: label(rate),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.value
            .partial_cmp(&b.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n_repos = rows.len();

    if let Some(rate) = base_totals.rate() {
        rows.push(Bar {
            name: "Baseline, all repositories".to_string(),
            value: rate,
            colour: C_BLUE,
            hatched: true,
            annotation: label(rate),
        });
    }
    if let Some(rate) = agentic.rate() {
        rows.push(Bar {
            name: "Agentic (outram-park-backend)".to_string(),
            value: rate,
            colour: C_ORANGE,
            hatched: false,
            annotation: label(rate),
        });
    }

    // (b) Composition of the agentic output, largest class at the top.
    let total: u64 = Provenance::ORDER.iter().map(|c| agentic.subtotal(*c)).sum();
    let mut composition: Vec<Bar> = Provenance::ORDER
        .iter()
        .rev()
        .filter(|class| agentic.subtotal(**class) > 0)
        .map(|class| {
            let value = agentic.subtotal(*class);
            let share = if total == 0 {
                0.0
            } else {
                value as f64 / total as f64 * 100.0
            };
            Bar {
                name: class.label().to_string(),
                value: value as f64,
                colour: match class {
                    Provenance::Translated => C_BLUE,
                    Provenance::Original => C_ORANGE,
                    Provenance::Extension => C_AQUA,
                },
                hatched: false,
                annotation: format!("{}  ({:.0}%)", fmt(value), share),
            }
        })
        .collect();
    // `ORDER.rev()` gives extension, original, translated; the original put the
    // largest at the top, which for this data is translated.
    composition.reverse();

    let mut body = String::new();
    let mut cursor = 28.0;
    cursor += bar_chart(
        &mut body,
        cursor,
        "(a)  Rate of code production, pre-agentic vs agentic",
        "Code lines per active day",
        &rows,
        Some(n_repos),
    );
    cursor += 28.0;
    if !composition.is_empty() {
        cursor += bar_chart(
            &mut body,
            cursor,
            &format!("(b)  Composition of the {} agentic code lines", fmt(total)),
            "Agentic code lines",
            &composition,
            None,
        );
    }
    let height = cursor + 12.0;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH:.0}" height="{height:.0}" viewBox="0 0 {WIDTH:.0} {height:.0}">
  <defs>
    <pattern id="hatch" patternUnits="userSpaceOnUse" width="6" height="6" patternTransform="rotate(45)">
      <rect width="6" height="6" fill="{C_BLUE}"/>
      <line x1="0" y1="0" x2="0" y2="6" stroke="#ffffff" stroke-width="2"/>
    </pattern>
  </defs>
  <style>
    text {{ font-family: "Nimbus Roman", "Times New Roman", serif; fill: {INK}; }}
    .title {{ font-size: 13px; }}
    .rowlabel {{ font-size: 11px; }}
    .value {{ font-size: 11px; }}
    .tick {{ font-size: 10px; fill: {INK_SOFT}; }}
    .axislabel {{ font-size: 11px; fill: {INK}; }}
    .grid {{ stroke: {GRID}; stroke-width: 0.8; }}
    .sep {{ stroke: {GRID}; stroke-width: 1.2; }}
    .axis {{ stroke: {INK_SOFT}; stroke-width: 0.9; }}
  </style>
  <rect width="100%" height="100%" fill="#ffffff"/>
{body}</svg>
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn stats(key: &str, code: u64, days: &[&str]) -> RepoStats {
        RepoStats {
            key: key.to_string(),
            lines: crate::kloc::LineCount {
                total: code * 2,
                code,
                files: 1,
            },
            days: days.iter().map(|d| d.to_string()).collect::<BTreeSet<_>>(),
            first: days.first().unwrap_or(&"").to_string(),
            last: days.last().unwrap_or(&"").to_string(),
            ..Default::default()
        }
    }

    fn agentic() -> AgenticSummary {
        let mut subtotals = BTreeMap::new();
        subtotals.insert("translated".to_string(), 136_462_u64);
        subtotals.insert("original".to_string(), 27_177);
        subtotals.insert("extension".to_string(), 12_358);
        AgenticSummary {
            subtotals,
            agentic_total: 175_997,
            active_days: 28,
            ..Default::default()
        }
    }

    #[test]
    fn the_figure_is_wellformed_svg_with_both_charts() {
        let base = vec![stats("alpha", 1_000, &["2026-01-01", "2026-01-02"])];
        let totals = BaselineTotals {
            lines: crate::kloc::LineCount {
                total: 2_000,
                code: 1_000,
                files: 1,
            },
            union_days: ["2026-01-01".to_string(), "2026-01-02".to_string()]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let svg = productivity_svg(&base, &totals, &agentic());

        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("(a)  Rate of code production"));
        assert!(svg.contains("(b)  Composition of the 175,997 agentic code lines"));
        assert!(svg.contains("Code lines per active day"));
    }

    /// The same numbers must give the same bytes — the whole reason this is
    /// hand-rolled rather than delegated to a plotting library.
    #[test]
    fn the_figure_is_deterministic() {
        let base = vec![stats("alpha", 1_000, &["2026-01-01"])];
        let totals = BaselineTotals::default();
        let first = productivity_svg(&base, &totals, &agentic());
        let second = productivity_svg(&base, &totals, &agentic());
        assert_eq!(first, second);
    }

    #[test]
    fn shares_are_stated_on_each_composition_bar() {
        let svg = productivity_svg(&[], &BaselineTotals::default(), &agentic());
        // 136,462 of 175,997 is 78%.
        assert!(svg.contains("136,462  (78%)"), "got: {svg}");
        assert!(svg.contains("27,177  (15%)"));
        assert!(svg.contains("12,358  (7%)"));
    }

    #[test]
    fn xml_reserved_characters_are_escaped() {
        assert_eq!(escape("a & b < c"), "a &amp; b &lt; c");
    }

    #[test]
    fn a_missing_repository_contributes_no_bar() {
        let base = vec![RepoStats {
            key: "gone".to_string(),
            missing: true,
            ..Default::default()
        }];
        let svg = productivity_svg(&base, &BaselineTotals::default(), &agentic());
        assert!(!svg.contains("gone"));
    }
}
