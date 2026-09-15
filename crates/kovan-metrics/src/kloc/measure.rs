//! Turning repositories into the numbers the paper reports.
//!
//! Two measurements, and the awkward bit is how they meet:
//!
//! - [`measure_baseline`] walks the pre-agentic repositories.
//! - [`measure_agentic`] walks `outram-park-backend`'s crates.
//!
//! An **extension** crate is a pre-agentic repository vendored into the backend
//! and then worked on, so only the excess over the standalone original is
//! agentic. That subtraction is why a missing baseline repository is not merely
//! an incomplete baseline: its lines move silently into the agentic total,
//! inflating it by very nearly what the baseline loses. [`BaselineTotals::missing`]
//! exists so callers can refuse to report a comparison that is not valid.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::config::{
    provenance_of, Provenance, RepoSpec, AGENTIC_SINCE, AGENTIC_UNTIL, CRATE_PROVENANCE,
    CRATE_SUBPATH_PROVENANCE, TUAS_IMPORT_REF, TUAS_KEY, TUAS_PREDECESSOR_KEY,
};
use super::repo::{active_days, count_tree, head_date, list_dir, read_blob, select_ref};
use super::source::LineCount;

/// One pre-agentic repository, as measured.
#[derive(Clone, Debug, Default)]
pub struct RepoStats {
    /// The repository's directory / GitHub name.
    pub key: String,
    /// How the manuscript names it, as LaTeX.
    pub label: String,
    /// Footnote text for the rate table's "AI?" column.
    pub note: String,
    /// Footnote marker (a, b, c) in the table.
    pub marker: String,
    /// Bib key, cited in the table row.
    pub cite: String,
    /// Absolute path to the checkout, if one was found.
    pub path: Option<PathBuf>,
    /// Line counts, **after** any net-of-predecessor adjustment.
    pub lines: LineCount,
    /// Line counts as measured, before that adjustment.
    ///
    /// Extensions must subtract the standalone original at its head, not the
    /// net figure, so both are kept.
    pub head_lines: LineCount,
    /// Distinct calendar dates carrying a commit.
    pub days: BTreeSet<String>,
    /// Earliest and latest active day, or empty when there are no commits.
    pub first: String,
    /// Latest active day.
    pub last: String,
    /// The ref lines were counted at.
    pub reference: String,
    /// No checkout was found for this repository.
    pub missing: bool,
}

impl RepoStats {
    /// Code lines per active day, or `None` when no day carried a commit.
    pub fn rate(&self) -> Option<f64> {
        if self.days.is_empty() {
            None
        } else {
            Some(self.lines.code as f64 / self.days.len() as f64)
        }
    }
}

/// The TUAS net-of-predecessor arithmetic, kept for the report and the caption.
#[derive(Clone, Copy, Debug, Default)]
pub struct TuasNet {
    /// TUAS as it stands at its head.
    pub head: LineCount,
    /// The tree TUAS imported wholesale at its second commit.
    pub imported: LineCount,
    /// Head less imported.
    pub net: LineCount,
    /// Written in the predecessor and never carried across at spin-out.
    pub abandoned_code: u64,
}

/// Totals across the pre-agentic baseline.
#[derive(Clone, Debug, Default)]
pub struct BaselineTotals {
    /// Summed line counts over the repositories that were found.
    pub lines: LineCount,
    /// Union of active dates — **not** a column sum, because these projects
    /// overlapped in time.
    pub union_days: BTreeSet<String>,
    /// Earliest active day across the baseline.
    pub first: String,
    /// Latest active day across the baseline.
    pub last: String,
    /// The TUAS adjustment, if both it and its predecessor were present.
    pub tuas_net: Option<TuasNet>,
    /// Repositories that could not be found.
    pub missing: Vec<String>,
}

impl BaselineTotals {
    /// Number of distinct active days.
    pub fn active_days(&self) -> usize {
        self.union_days.len()
    }

    /// Code lines per active day.
    pub fn rate(&self) -> Option<f64> {
        if self.union_days.is_empty() {
            None
        } else {
            Some(self.lines.code as f64 / self.union_days.len() as f64)
        }
    }

    /// The "measured &lt;date&gt;" stamp for the table caption: the newest commit
    /// date seen, **not** today's date, so re-running months later cannot
    /// silently restamp a table whose inputs have not moved.
    pub fn as_of(&self) -> &str {
        &self.last
    }
}

/// One crate in the agentic repository, as measured.
#[derive(Clone, Debug)]
pub struct CrateStats {
    /// Crate directory name, or the split-out binary's name.
    pub name: String,
    /// How the table names it.
    pub display: String,
    /// How its lines are attributed.
    pub class: Provenance,
    /// Upstream project, or the pre-agentic repository it extends.
    pub upstream: Option<String>,
    /// Total lines, blank and comment included.
    pub total_lines: u64,
    /// Agentic code lines: `raw_code_lines` less any pre-agentic original.
    pub code_lines: u64,
    /// Code lines as they stand in the backend.
    pub raw_code_lines: u64,
    /// The standalone pre-agentic original's code lines, if any.
    pub baseline_code_lines: u64,
    /// The crate's own `Cargo.toml` description, carried so the classification
    /// can be audited against what the crate says it is.
    pub description: String,
}

/// Totals across the agentic repository.
#[derive(Clone, Debug, Default)]
pub struct AgenticSummary {
    /// No checkout was found.
    pub missing: bool,
    /// Number of crate directories found.
    pub n_crates: usize,
    /// Present in the checkout but absent from the classification table, and
    /// therefore excluded from every total.
    pub unclassified: Vec<String>,
    /// Classified but not present in the checkout.
    pub stale: Vec<String>,
    /// All Rust code in `crates/`, before any pre-agentic subtraction.
    pub total_rust_code: u64,
    /// Agentic code lines per class.
    pub subtotals: BTreeMap<String, u64>,
    /// Sum of `subtotals`.
    pub agentic_total: u64,
    /// Active days inside the reported window.
    pub active_days: usize,
    /// Commit date of the measured ref.
    pub head: String,
    /// The ref measured.
    pub reference: String,
}

impl AgenticSummary {
    /// Agentic code lines per active day.
    pub fn rate(&self) -> Option<f64> {
        if self.active_days == 0 {
            None
        } else {
            Some(self.agentic_total as f64 / self.active_days as f64)
        }
    }

    /// Subtotal for one class.
    pub fn subtotal(&self, class: Provenance) -> u64 {
        self.subtotals.get(class.key()).copied().unwrap_or(0)
    }
}

/// Measure one pre-agentic repository.
///
/// Lines are counted at `measure_ref` where one is pinned; **active days are
/// always counted over the branch's whole history**, because a day the author
/// committed is a day worked whether or not that commit survives to the pinned
/// tree.
pub fn measure_repo(spec: &RepoSpec, path: Option<&Path>) -> RepoStats {
    let mut stats = RepoStats {
        key: spec.key.to_string(),
        label: spec.label.to_string(),
        note: spec.note.to_string(),
        marker: spec.marker.to_string(),
        cite: spec.cite.to_string(),
        path: path.map(Path::to_path_buf),
        ..Default::default()
    };
    let Some(path) = path else {
        stats.missing = true;
        return stats;
    };

    let branch = select_ref(path);
    stats.reference = spec.measure_ref.unwrap_or(&branch).to_string();
    stats.lines = count_tree(path, &stats.reference, "", &BTreeSet::new());
    stats.head_lines = stats.lines;
    stats.days = active_days(path, &branch, None, None);
    if let (Some(first), Some(last)) = (stats.days.iter().next(), stats.days.iter().next_back()) {
        stats.first = first.clone();
        stats.last = last.clone();
    }
    stats
}

/// Measure the whole pre-agentic baseline, applying the TUAS adjustment.
pub fn measure_baseline(
    specs: &[RepoSpec],
    paths: &BTreeMap<String, PathBuf>,
) -> (Vec<RepoStats>, BaselineTotals) {
    let mut stats: Vec<RepoStats> = specs
        .iter()
        .map(|spec| measure_repo(spec, paths.get(spec.key).map(PathBuf::as_path)))
        .collect();

    // TUAS net of what it inherited. The subtrahend is the imported tree
    // itself, not the predecessor's full extent -- see `TUAS_IMPORT_REF`.
    let mut tuas_net = None;
    let tuas_index = stats.iter().position(|s| s.key == TUAS_KEY);
    let pred_code = stats
        .iter()
        .find(|s| s.key == TUAS_PREDECESSOR_KEY && !s.missing)
        .map(|s| s.lines.code);
    if let (Some(index), Some(pred_code)) = (tuas_index, pred_code) {
        if !stats[index].missing {
            let path = stats[index].path.clone().expect("present repo has a path");
            let imported = count_tree(&path, TUAS_IMPORT_REF, "", &BTreeSet::new());
            let head = stats[index].lines;
            let net = LineCount {
                total: head.total.saturating_sub(imported.total),
                code: head.code.saturating_sub(imported.code),
                files: head.files,
            };
            tuas_net = Some(TuasNet {
                head,
                imported,
                net,
                abandoned_code: pred_code.saturating_sub(imported.code),
            });
            stats[index].lines.total = net.total;
            stats[index].lines.code = net.code;
        }
    }

    let present: Vec<&RepoStats> = stats.iter().filter(|s| !s.missing).collect();
    let mut union_days: BTreeSet<String> = BTreeSet::new();
    let mut lines = LineCount::default();
    for repo in &present {
        union_days.extend(repo.days.iter().cloned());
        lines.total += repo.lines.total;
        lines.code += repo.lines.code;
        lines.files += repo.lines.files;
    }

    let totals = BaselineTotals {
        lines,
        first: present
            .iter()
            .filter(|s| !s.first.is_empty())
            .map(|s| s.first.clone())
            .min()
            .unwrap_or_default(),
        last: present
            .iter()
            .filter(|s| !s.last.is_empty())
            .map(|s| s.last.clone())
            .max()
            .unwrap_or_default(),
        union_days,
        tuas_net,
        missing: stats
            .iter()
            .filter(|s| s.missing)
            .map(|s| s.key.clone())
            .collect(),
    };
    (stats, totals)
}

/// The `description` a crate declares in its own `Cargo.toml`.
///
/// Parsed without a TOML dependency: the field is a single quoted string, and
/// whitespace is collapsed so a description wrapped across lines reads as one.
fn crate_description(manifest: &str) -> String {
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("description") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                let value = value.trim().trim_matches('"');
                return value.split_whitespace().collect::<Vec<_>>().join(" ");
            }
        }
    }
    String::new()
}

/// Measure the agentic repository's crates.
///
/// Every crate is counted out of the **same** tree at the same ref, so no crate
/// can be measured at a different commit from its neighbours.
pub fn measure_agentic(
    path: Option<&Path>,
    measure_ref: Option<&str>,
    baseline: &BTreeMap<String, RepoStats>,
) -> (Vec<CrateStats>, AgenticSummary) {
    let Some(path) = path else {
        return (
            Vec::new(),
            AgenticSummary {
                missing: true,
                ..Default::default()
            },
        );
    };

    let branch = select_ref(path);
    let reference = measure_ref.unwrap_or(&branch).to_string();

    // Crate directories are those carrying a Cargo.toml, sorted for
    // determinism.
    let found: Vec<String> = list_dir(path, &reference, "crates")
        .into_iter()
        .filter(|name| read_blob(path, &reference, &format!("crates/{name}/Cargo.toml")).is_some())
        .collect();

    if found.is_empty() {
        return (
            Vec::new(),
            AgenticSummary {
                missing: true,
                ..Default::default()
            },
        );
    }

    let unclassified: Vec<String> = found
        .iter()
        .filter(|name| provenance_of(name).is_none())
        .cloned()
        .collect();
    let stale: Vec<String> = CRATE_PROVENANCE
        .iter()
        .map(|(name, _, _)| (*name).to_string())
        .filter(|name| !found.contains(name))
        .collect();

    let mut rows: Vec<CrateStats> = Vec::new();
    for name in &found {
        let Some((class, upstream)) = provenance_of(name) else {
            continue;
        };
        let crate_prefix = format!("crates/{name}");

        // Split out any part of the crate classified differently, and exclude
        // it from the crate's own count so nothing is counted twice.
        let mut sub_paths: BTreeSet<String> = BTreeSet::new();
        for (host, rel, sub_class, sub_name) in CRATE_SUBPATH_PROVENANCE {
            if host != name {
                continue;
            }
            let sub_prefix = format!("{crate_prefix}/{rel}");
            let sub = count_tree(path, &reference, &sub_prefix, &BTreeSet::new());
            if sub.files == 0 {
                continue;
            }
            sub_paths.insert((*rel).to_string());
            rows.push(CrateStats {
                name: (*sub_name).to_string(),
                display: format!("{sub_name} (bin in {name})"),
                class: *sub_class,
                upstream: None,
                total_lines: sub.total,
                code_lines: sub.code,
                raw_code_lines: sub.code,
                baseline_code_lines: 0,
                description: format!("feature-gated binary inside {name}"),
            });
        }

        let counted = count_tree(path, &reference, &crate_prefix, &sub_paths);

        // Compare an extension against the standalone repo AS MEASURED AT ITS
        // OWN HEAD, not the net-of-predecessor figure in the baseline table.
        let baseline_code = if class == Provenance::Extension {
            upstream
                .and_then(|key| baseline.get(key))
                .filter(|base| !base.missing)
                .map(|base| base.head_lines.code)
                .unwrap_or(0)
        } else {
            0
        };

        let description = read_blob(path, &reference, &format!("{crate_prefix}/Cargo.toml"))
            .map(|manifest| crate_description(&manifest))
            .unwrap_or_default();

        rows.push(CrateStats {
            name: name.clone(),
            display: name.clone(),
            class,
            upstream: upstream.map(str::to_string),
            total_lines: counted.total,
            code_lines: counted.code.saturating_sub(baseline_code),
            raw_code_lines: counted.code,
            baseline_code_lines: baseline_code,
            description,
        });
    }

    let mut subtotals: BTreeMap<String, u64> = BTreeMap::new();
    for class in Provenance::ORDER {
        let sum = rows
            .iter()
            .filter(|r| r.class == *class)
            .map(|r| r.code_lines)
            .sum();
        subtotals.insert(class.key().to_string(), sum);
    }

    let days = active_days(path, &reference, Some(AGENTIC_SINCE), Some(AGENTIC_UNTIL));
    let summary = AgenticSummary {
        missing: false,
        n_crates: found.len(),
        unclassified,
        stale,
        total_rust_code: rows.iter().map(|r| r.raw_code_lines).sum(),
        agentic_total: subtotals.values().sum(),
        subtotals,
        active_days: days.len(),
        head: head_date(path, &reference),
        reference,
    };
    (rows, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_is_read_and_whitespace_collapsed() {
        let manifest = "[package]\nname = \"x\"\ndescription = \"a  b\\n c\"\n";
        assert_eq!(crate_description(manifest), "a b\\n c");
        assert_eq!(crate_description("[package]\nname = \"x\"\n"), "");
        // A description under another section is not the package's.
        assert_eq!(
            crate_description("[package]\n\n[lib]\ndescription = \"no\"\n"),
            ""
        );
    }

    #[test]
    fn a_missing_repository_is_reported_not_counted_as_zero() {
        let spec = &super::super::config::BASELINE_REPOS[0];
        let stats = measure_repo(spec, None);
        assert!(stats.missing);
        assert_eq!(stats.lines, LineCount::default());
        assert!(stats.rate().is_none());
    }

    /// **Missing baseline repositories must be surfaced**, because an extension
    /// crate subtracts its standalone original: a missing one silently moves
    /// those lines into the agentic total instead of merely shrinking the
    /// baseline. A caller that does not check this reports a comparison that is
    /// not valid.
    #[test]
    fn missing_repositories_are_listed_in_the_totals() {
        let specs = super::super::config::BASELINE_REPOS;
        let (stats, totals) = measure_baseline(specs, &BTreeMap::new());
        assert_eq!(stats.len(), specs.len());
        assert_eq!(totals.missing.len(), specs.len());
        assert_eq!(totals.lines, LineCount::default());
        assert_eq!(totals.active_days(), 0);
        assert!(totals.rate().is_none());
        assert!(
            totals.tuas_net.is_none(),
            "no TUAS adjustment without both repos"
        );
    }

    #[test]
    fn an_absent_agentic_checkout_reports_missing() {
        let (rows, summary) = measure_agentic(None, None, &BTreeMap::new());
        assert!(rows.is_empty());
        assert!(summary.missing);
        assert!(summary.rate().is_none());
    }

    #[test]
    fn the_as_of_stamp_is_the_newest_commit_not_today() {
        let totals = BaselineTotals {
            last: "2026-07-23".to_string(),
            ..Default::default()
        };
        assert_eq!(totals.as_of(), "2026-07-23");
    }
}
