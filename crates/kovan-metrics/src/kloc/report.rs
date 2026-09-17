//! The console report, also written to `summary.txt`.
//!
//! # Byte-for-byte parity is the requirement here
//!
//! This text is one of the artifacts gated by `docs/kloc-parity-baseline/`, so
//! the column widths, rule lengths, comma grouping and wording are **not** free
//! to improve. A tidier table that differs by one space is a failed port. Every
//! `{:<46}` and `"=".repeat(78)` below is load-bearing.
//!
//! Ported from `report()` in the retired `scripts/kloc_accounting.py`.

use std::collections::BTreeMap;

use super::config::{
    Provenance, ASSISTANCE_GROUPS, AGENTIC_SINCE, AGENTIC_UNTIL, MANUSCRIPT, TUAS_IMPORT_REF,
};
use super::measure::{AgenticSummary, BaselineTotals, CrateStats, RepoStats};

/// Format an integer with comma thousands separators, matching Python's
/// `f"{n:,}"`.
pub fn fmt(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Format a signed integer with comma separators and an explicit sign,
/// matching Python's `f"{n:+,}"`.
fn fmt_signed(n: i64) -> String {
    let sign = if n < 0 { '-' } else { '+' };
    format!("{sign}{}", fmt(n.unsigned_abs()))
}

/// Round to the nearest integer and group, matching Python's `f"{x:,.0f}"`.
///
/// Both languages round half to even here, so a rate landing exactly on `.5`
/// formats identically.
fn fmt_rate(x: f64) -> String {
    let rounded = format!("{x:.0}");
    let n: u64 = rounded.parse().unwrap_or(0);
    fmt(n)
}

/// Pad `text` on the right to `width` columns.
fn left(text: &str, width: usize) -> String {
    format!("{text:<width$}")
}

/// Pad `text` on the left to `width` columns.
fn right(text: &str, width: usize) -> String {
    format!("{text:>width$}")
}

/// Render the full report.
///
/// `check` adds the drift comparison against the manuscript's published
/// figures.
pub fn report(
    base_stats: &[RepoStats],
    base_totals: &BaselineTotals,
    crate_rows: &[CrateStats],
    agentic: &AgenticSummary,
    check: bool,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    let rule_eq = "=".repeat(78);
    let rule_dash = "-".repeat(78);

    lines.push(rule_eq.clone());
    lines.push("PRE-AGENTIC BASELINE".to_string());
    lines.push(rule_eq.clone());
    lines.push(format!(
        "{}{}{}{}",
        left("repository", 46),
        right("total", 10),
        right("code", 10),
        right("days", 7)
    ));
    lines.push(rule_dash.clone());

    for stats in base_stats {
        if stats.missing {
            lines.push(format!(
                "{}{}",
                left(&stats.key, 46),
                right("MISSING -- run with --clone", 27)
            ));
            continue;
        }
        let span = if stats.first.is_empty() {
            "no commits".to_string()
        } else {
            format!("{} .. {}", stats.first, stats.last)
        };
        lines.push(format!(
            "{}{}{}{}",
            left(&stats.key, 46),
            right(&fmt(stats.lines.total), 10),
            right(&fmt(stats.lines.code), 10),
            right(&stats.days.len().to_string(), 7)
        ));
        lines.push(format!("    {span}  [{}]", stats.reference));
    }

    lines.push(rule_dash.clone());
    lines.push(format!(
        "{}{}{}{}",
        left("BASELINE TOTAL", 46),
        right(&fmt(base_totals.lines.total), 10),
        right(&fmt(base_totals.lines.code), 10),
        right(&base_totals.active_days().to_string(), 7)
    ));
    lines.push("  (days are a union of distinct dates, not a column sum)".to_string());

    if let Some(net) = base_totals.tuas_net {
        lines.push(String::new());
        lines.push(format!(
            "  TUAS at head       : {} total, {} code",
            fmt(net.head.total),
            fmt(net.head.code)
        ));
        lines.push(format!(
            "  less imported      : {} total, {} code ({} files, at {})",
            fmt(net.imported.total),
            fmt(net.imported.code),
            net.imported.files,
            &TUAS_IMPORT_REF[..7]
        ));
        lines.push(format!(
            "  TUAS net-new       : {} total, {} code",
            fmt(net.net.total),
            fmt(net.net.code)
        ));
        lines.push(format!(
            "  predecessor kept {} code lines that were never imported;",
            fmt(net.abandoned_code)
        ));
        lines.push(
            "  they stay in the baseline under thermal_hydraulics_rs, and are the reason"
                .to_string(),
        );
        lines.push(
            "  the baseline total exceeds the pre-agentic code vendored into the backend."
                .to_string(),
        );
    }

    if !base_totals.missing.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "  ! missing repositories, totals are incomplete: {}",
            base_totals.missing.join(", ")
        ));
    }

    if let Some(rate) = base_totals.rate() {
        lines.push(String::new());
        lines.push(format!(
            "  baseline rate: {} code lines per active day",
            fmt_rate(rate)
        ));
    }

    let by_key: BTreeMap<&str, &RepoStats> =
        base_stats.iter().map(|s| (s.key.as_str(), s)).collect();
    lines.push(String::new());
    lines.push("  Split by how much non-agentic AI help each repository had:".to_string());
    for (group, keys) in ASSISTANCE_GROUPS {
        let members: Vec<&&RepoStats> = keys
            .iter()
            .filter_map(|k| by_key.get(k))
            .filter(|s| !s.missing)
            .collect();
        if members.is_empty() {
            continue;
        }
        let code: u64 = members.iter().map(|m| m.lines.code).sum();
        let mut group_days = std::collections::BTreeSet::new();
        for member in &members {
            group_days.extend(member.days.iter().cloned());
        }
        let rate = if group_days.is_empty() {
            0.0
        } else {
            code as f64 / group_days.len() as f64
        };
        lines.push(format!(
            "    {}{} code, {} days, {} lines/day",
            left(group, 36),
            right(&fmt(code), 9),
            right(&group_days.len().to_string(), 4),
            fmt_rate(rate)
        ));
    }
    lines.push(
        "    (this comparison is not clean -- the author also grew more fluent in".to_string(),
    );
    lines.push("     Rust over the same period, and the crates differ in nature)".to_string());

    lines.push(String::new());
    lines.push(rule_eq.clone());
    lines.push("AGENTIC OUTPUT -- outram-park-backend".to_string());
    lines.push(rule_eq.clone());
    if agentic.missing {
        lines.push("  MISSING -- run with --clone".to_string());
        return lines.join("\n");
    }

    lines.push(format!(
        "  branch {} @ {}, {} crates",
        agentic.reference, agentic.head, agentic.n_crates
    ));
    lines.push(format!(
        "  window {AGENTIC_SINCE} .. {AGENTIC_UNTIL}: {} active days",
        agentic.active_days
    ));
    lines.push(String::new());

    for class in Provenance::ORDER {
        let label = class.label();
        lines.push(format!("-- {label} {}", "-".repeat(60 - label.len())));
        let mut members: Vec<&CrateStats> =
            crate_rows.iter().filter(|r| r.class == *class).collect();
        // Descending by agentic lines; Python's sort is stable, so ties keep
        // the order the crates were measured in.
        members.sort_by(|a, b| b.code_lines.cmp(&a.code_lines));
        for row in members {
            let upstream = row.upstream.clone().unwrap_or_else(|| "---".to_string());
            let extra = if *class == Provenance::Extension {
                format!(
                    "  ({} - {})",
                    fmt(row.raw_code_lines),
                    fmt(row.baseline_code_lines)
                )
            } else {
                String::new()
            };
            lines.push(format!(
                "  {}{}{}{extra}",
                left(&row.display, 46),
                left(&upstream, 22),
                right(&fmt(row.code_lines), 9)
            ));
        }
        lines.push(format!(
            "  {}{}{}",
            left("SUBTOTAL", 46),
            left("", 22),
            right(&fmt(agentic.subtotal(*class)), 9)
        ));
        lines.push(String::new());
    }

    lines.push(format!(
        "  {}{}{}",
        left("TOTAL AGENTIC", 46),
        left("", 22),
        right(&fmt(agentic.agentic_total), 9)
    ));
    lines.push(format!(
        "  {}{}{}",
        left("total Rust code in crates/", 46),
        left("", 22),
        right(&fmt(agentic.total_rust_code), 9)
    ));

    if let Some(rate) = agentic.rate() {
        lines.push(String::new());
        lines.push(format!(
            "  agentic rate: {} code lines per active day",
            fmt_rate(rate)
        ));
    }

    if !agentic.unclassified.is_empty() {
        lines.push(String::new());
        lines.push(
            "  ! crates present but not classified in CRATE_PROVENANCE (excluded from all totals):"
                .to_string(),
        );
        for name in &agentic.unclassified {
            lines.push(format!("      {name}"));
        }
    }
    if !agentic.stale.is_empty() {
        lines.push(String::new());
        lines.push("  ! classified but not present in the checkout:".to_string());
        for name in &agentic.stale {
            lines.push(format!("      {name}"));
        }
    }

    if check {
        lines.push(String::new());
        lines.push(rule_eq.clone());
        lines.push("COMPARISON WITH THE MANUSCRIPT (drift check)".to_string());
        lines.push(rule_eq.clone());

        let measured: Vec<(&str, i64)> = vec![
            ("baseline_total_lines", base_totals.lines.total as i64),
            ("baseline_code_lines", base_totals.lines.code as i64),
            ("baseline_active_days", base_totals.active_days() as i64),
            ("agentic_code_lines", agentic.agentic_total as i64),
            (
                "subtotal_translated",
                agentic.subtotal(Provenance::Translated) as i64,
            ),
            (
                "subtotal_original",
                agentic.subtotal(Provenance::Original) as i64,
            ),
            (
                "subtotal_extension",
                agentic.subtotal(Provenance::Extension) as i64,
            ),
            ("n_crates", agentic.n_crates as i64),
        ];

        // A missing repository does not just shrink the baseline: because
        // extensions subtract a vendored crate's pre-agentic original, a missing
        // baseline repo silently MOVES those lines into the agentic total. Say
        // so before printing deltas that would otherwise look like real drift.
        if !base_totals.missing.is_empty() {
            lines.push("  !! THIS COMPARISON IS NOT VALID.".to_string());
            lines.push(format!("  !! Missing: {}", base_totals.missing.join(", ")));
            lines.push("  !! Their lines are absent from the baseline and are instead".to_string());
            lines
                .push("  !! counted as agentic extensions, which inflates the agentic".to_string());
            lines.push("  !! total by exactly the amount the baseline loses.".to_string());
            lines.push(
                "  !! Re-run with --from-github to fetch every repository first.".to_string(),
            );
            lines.push(String::new());
        }

        lines.push(format!(
            "{}{}{}{}",
            left("quantity", 28),
            right("manuscript", 12),
            right("measured", 12),
            right("delta", 12)
        ));
        lines.push(rule_dash.clone());
        for (key, got) in &measured {
            let Some((_, want)) = MANUSCRIPT.iter().find(|(k, _)| k == key) else {
                continue;
            };
            let delta = got - want;
            let flag = if delta == 0 { "" } else { "   <-- differs" };
            lines.push(format!(
                "{}{}{}{}{flag}",
                left(key, 28),
                right(&fmt(want.unsigned_abs()), 12),
                right(&fmt(got.unsigned_abs()), 12),
                right(&fmt_signed(delta), 12)
            ));
        }
        lines.push(String::new());
        if !base_totals.missing.is_empty() {
            lines.push("  Deltas above are meaningless until the missing repositories".to_string());
            lines.push("  listed at the top of this section are present.".to_string());
        } else {
            lines.push(
                "  A non-zero delta is not automatically an error: the repositories".to_string(),
            );
            lines.push(
                "  keep moving. It means the manuscript figure needs re-stating from".to_string(),
            );
            lines.push(
                "  this run, or the run needs pinning to the commit the table used.".to_string(),
            );
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separators_match_pythons_format() {
        assert_eq!(fmt(0), "0");
        assert_eq!(fmt(999), "999");
        assert_eq!(fmt(1_000), "1,000");
        assert_eq!(fmt(303_463), "303,463");
        assert_eq!(fmt(1_234_567), "1,234,567");
    }

    #[test]
    fn signed_deltas_always_carry_a_sign() {
        assert_eq!(fmt_signed(0), "+0");
        assert_eq!(fmt_signed(462), "+462");
        assert_eq!(fmt_signed(-1_234), "-1,234");
    }

    #[test]
    fn rates_round_and_group() {
        assert_eq!(fmt_rate(848.6), "849");
        assert_eq!(fmt_rate(6_245.7), "6,246");
        assert_eq!(fmt_rate(0.0), "0");
    }

    /// The report is one of the byte-compared artifacts, so its rules and
    /// column widths are pinned rather than left to taste.
    #[test]
    fn the_header_rules_are_seventy_eight_columns() {
        let text = report(
            &[],
            &BaselineTotals::default(),
            &[],
            &AgenticSummary {
                missing: true,
                ..Default::default()
            },
            false,
        );
        let mut found = false;
        for line in text.lines() {
            if line.starts_with("===") {
                assert_eq!(line.len(), 78, "rule must be 78 columns: {line:?}");
                found = true;
            }
        }
        assert!(found, "the report must carry its rules");
    }

    #[test]
    fn an_absent_agentic_repository_stops_the_report_early() {
        let text = report(
            &[],
            &BaselineTotals::default(),
            &[],
            &AgenticSummary {
                missing: true,
                ..Default::default()
            },
            false,
        );
        assert!(text.contains("MISSING -- run with --clone"));
        assert!(!text.contains("TOTAL AGENTIC"));
    }
}
