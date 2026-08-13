//! The historian report — pre-merge-to-`main` accounting.
//!
//! Before `develop` is merged into `main`, the workspace generates a report
//! accounting for the **API tokens spent** and the **lines / KLOC written**
//! across the window of history being released.
//!
//! # Sources, not estimates
//!
//! - **Lines** come from `git log --numstat --no-merges` over the range.
//! - **Tokens** come from the `API-Usage-Since-Last-Commit` commit trailers.
//!
//! Commits predating the token hooks legitimately carry *no token data*. They
//! are counted in the line totals and shown with `—` in the token column. That
//! is correct output, not a gap to be filled in.
//!
//! # Default window
//!
//! With no `--from`, the window is "everything on `<branch>` not yet on
//! `<base>`" (i.e. `base..branch`), which is exactly what the pending merge
//! would deliver.

use std::path::{Path, PathBuf};

use crate::date::Date;
use crate::git::{self, NumStat};
use crate::trailer::{self, TokenCounts};

/// Where generated reports live, relative to the repository root.
pub const REPORT_DIR_REL: &str = "docs/historian";

/// Everything one commit contributes to the report.
struct Row {
    date: String,
    short: String,
    subject: String,
    stat: NumStat,
    tokens: TokenCounts,
    recorded_total: u64,
    has_token_data: bool,
}

/// Collect the commits in the window, newest-last, with their statistics.
fn collect(
    branch_ref: &str,
    base_ref: Option<&str>,
    from: Option<Date>,
    to: Option<Date>,
) -> Vec<Row> {
    let fmt = git::record_format();
    let since = from.map(|d| format!("--since={} 00:00:00", d.iso()));
    let until = to.map(|d| format!("--until={} 23:59:59", d.iso()));
    let range = match base_ref {
        Some(base) => format!("{base}..{branch_ref}"),
        None => branch_ref.to_string(),
    };

    let mut args: Vec<&str> = vec!["log", "--no-merges", "--reverse", &fmt];
    if let Some(s) = since.as_deref() {
        args.push(s);
    }
    if let Some(u) = until.as_deref() {
        args.push(u);
    }
    args.push(&range);

    git::parse_records(&git::git_output(&args))
        .into_iter()
        .map(|rec| {
            let parsed = trailer::parse(&rec.body);
            let (tokens, recorded_total, has_data) = match parsed {
                Some(t) => (t.counts, t.total, t.has_data()),
                None => (TokenCounts::default(), 0, false),
            };
            Row {
                stat: git::numstat(&rec.short),
                date: rec.date,
                short: rec.short,
                subject: rec.subject,
                tokens,
                recorded_total,
                has_token_data: has_data,
            }
        })
        .collect()
}

/// Render the report markdown.
fn render(
    from: Option<Date>,
    to: Option<Date>,
    branch_ref: &str,
    base_ref: Option<&str>,
    rows: &[Row],
) -> String {
    let g = trailer::group;
    let mut tok = TokenCounts::default();
    let mut tok_total = 0u64;
    let mut tok_covered = 0usize;
    let mut added = 0u64;
    let mut removed = 0u64;
    let mut rs_added = 0u64;
    let mut rs_removed = 0u64;
    let mut per_crate: Vec<(String, u64)> = Vec::new();

    for r in rows {
        tok.add(&r.tokens);
        tok_total += r.recorded_total;
        if r.has_token_data {
            tok_covered += 1;
        }
        added += r.stat.added;
        removed += r.stat.removed;
        rs_added += r.stat.rs_added;
        rs_removed += r.stat.rs_removed;
        for (name, n) in &r.stat.per_crate_added {
            match per_crate.iter_mut().find(|(k, _)| k == name) {
                Some((_, v)) => *v += n,
                None => per_crate.push((name.clone(), *n)),
            }
        }
    }

    let span = match (from, to, base_ref) {
        (Some(f), Some(t), _) => format!("{} → {}", f.human(), t.human()),
        (_, _, Some(base)) => format!("all of `{branch_ref}` not in `{base}`"),
        _ => format!("full history of `{branch_ref}`"),
    };
    let kloc = |n: u64| format!("{:.1}", n as f64 / 1000.0);
    let net = |a: u64, r: u64| a as i64 - r as i64;
    /// Thousands-group a possibly-negative net figure (a shrinking window is a
    /// legitimate result, so the sign must survive).
    fn group_i64(n: i64) -> String {
        let s = trailer::group(n.unsigned_abs());
        if n < 0 {
            format!("-{s}")
        } else {
            s
        }
    }
    let net_kloc = |a: u64, r: u64| format!("{:.1}", net(a, r) as f64 / 1000.0);

    let mut o = String::new();
    o.push_str(&format!("# OUTRAM PARK — historian report ({span})\n\n"));
    o.push_str(
        "> Pre-merge-to-`main` accounting of the API tokens spent and the lines / \
         KLOC written across this window of `develop` history. **Auto-generated** \
         by `kovan historian`; regenerate with \
         `kovan historian --from DDMMYY --to DDMMYY`.\n\n",
    );
    o.push_str("## Scope\n\n");
    o.push_str(&format!(
        "- **Branch:** `{branch_ref}`{}\n",
        base_ref.map(|b| format!(" (vs base `{b}`)")).unwrap_or_default()
    ));
    o.push_str(&format!("- **Window:** {span}\n"));
    o.push_str(&format!("- **Commits (non-merge):** {}\n", rows.len()));
    o.push_str(&format!(
        "- **Token coverage:** {tok_covered}/{} commits carry an \
         `API-Usage-Since-Last-Commit` trailer. Commits before the token-accounting \
         hooks existed (or made outside a Claude session) contribute 0 and are \
         counted here as *no token data* — that is correct, not missing data.\n\n",
        rows.len()
    ));

    o.push_str("## Totals\n\n");
    o.push_str("### Lines written (git numstat, merges excluded)\n\n");
    o.push_str("| Metric | Lines | KLOC |\n|---|--:|--:|\n");
    o.push_str(&format!("| Added (all files) | {} | {} |\n", g(added), kloc(added)));
    o.push_str(&format!("| Removed (all files) | {} | {} |\n", g(removed), kloc(removed)));
    o.push_str(&format!(
        "| **Net (all files)** | **{}** | **{}** |\n",
        group_i64(net(added, removed)),
        net_kloc(added, removed)
    ));
    o.push_str(&format!("| Added (Rust `.rs`) | {} | {} |\n", g(rs_added), kloc(rs_added)));
    o.push_str(&format!(
        "| Net (Rust `.rs`) | {} | {} |\n",
        group_i64(net(rs_added, rs_removed)),
        net_kloc(rs_added, rs_removed)
    ));

    o.push_str("\n### API tokens spent\n\n");
    o.push_str("| Component | Tokens |\n|---|--:|\n");
    o.push_str(&format!("| input | {} |\n", g(tok.input)));
    o.push_str(&format!("| output | {} |\n", g(tok.output)));
    o.push_str(&format!("| cache_read | {} |\n", g(tok.cache_read)));
    o.push_str(&format!("| cache_write | {} |\n", g(tok.cache_write)));
    o.push_str(&format!("| **total** | **{}** |\n", g(tok_total)));
    o.push_str(
        "\n_`total` = input + output + cache_read + cache_write. Cache-read \
         (prompt-cache re-reads of the growing context) usually dominates; the \
         output figure is the closest proxy for net generated content._\n\n",
    );

    if !per_crate.is_empty() {
        per_crate.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        o.push_str("## Lines added, by crate (top 20)\n\n");
        o.push_str("| Crate | Lines added |\n|---|--:|\n");
        for (name, n) in per_crate.iter().take(20) {
            o.push_str(&format!("| `{name}` | {} |\n", g(*n)));
        }
        o.push('\n');
    }

    o.push_str("## Per-commit ledger\n\n");
    o.push_str("| Date | Commit | Subject | +lines | -lines | Tokens |\n");
    o.push_str("|---|---|---|--:|--:|--:|\n");
    for r in rows {
        let mut subj = r.subject.replace('|', "\\|");
        if subj.chars().count() > 64 {
            subj = subj.chars().take(61).collect::<String>() + "...";
        }
        let tk = if r.has_token_data {
            g(r.recorded_total)
        } else {
            "—".to_string()
        };
        o.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            r.date,
            r.short,
            subj,
            g(r.stat.added),
            g(r.stat.removed),
            tk
        ));
    }
    o
}

/// Generate a historian report and write it to disk.
///
/// Returns the path written and the number of non-merge commits covered.
///
/// With `from` unset the window defaults to `base..branch` — everything on
/// `branch` not yet on `base` — and `to` defaults to today only when `from` was
/// given, matching the Python this replaces.
pub fn generate(
    from: Option<Date>,
    to: Option<Date>,
    branch: &str,
    base: &str,
    outfile: Option<PathBuf>,
) -> Result<(PathBuf, usize), String> {
    let branch_ref = git::ref_for_branch(branch);
    let to = match (from, to) {
        (Some(_), None) => Some(Date::today()),
        (_, t) => t,
    };
    let base_ref = if from.is_none() {
        Some(git::ref_for_branch(base))
    } else {
        None
    };

    let rows = collect(&branch_ref, base_ref.as_deref(), from, to);
    let md = render(from, to, &branch_ref, base_ref.as_deref(), &rows);

    let root = git::repo_root();
    let path = match outfile {
        Some(p) => p,
        None => {
            let tag = match (from, to) {
                (Some(f), Some(t)) => format!("{}_to_{}", f.ddmmyy(), t.ddmmyy()),
                _ => format!("since_{}_to_{}", base, Date::today().ddmmyy()),
            };
            root.join(REPORT_DIR_REL).join(format!("historian_{tag}.md"))
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, md).map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    let shown = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
    println!("wrote {}  ({} commits)", shown.display(), rows.len());
    Ok((path, rows.len()))
}

/// Resolve the default output path for a window, without writing anything.
pub fn default_output_path(root: &Path, from: Option<Date>, to: Option<Date>, base: &str) -> PathBuf {
    let tag = match (from, to) {
        (Some(f), Some(t)) => format!("{}_to_{}", f.ddmmyy(), t.ddmmyy()),
        _ => format!("since_{}_to_{}", base, Date::today().ddmmyy()),
    };
    root.join(REPORT_DIR_REL).join(format!("historian_{tag}.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str, subject: &str, added: u64, removed: u64, tokens: Option<u64>) -> Row {
        Row {
            date: date.into(),
            short: "abc1234".into(),
            subject: subject.into(),
            stat: NumStat {
                added,
                removed,
                rs_added: added,
                rs_removed: removed,
                per_crate_added: vec![("tampines".into(), added)],
            },
            tokens: TokenCounts {
                input: tokens.unwrap_or(0),
                ..Default::default()
            },
            recorded_total: tokens.unwrap_or(0),
            has_token_data: tokens.is_some(),
        }
    }

    #[test]
    fn renders_totals_and_a_ledger() {
        let rows = vec![
            row("2026-08-12", "first", 100, 10, Some(5_000)),
            row("2026-08-13", "second", 250, 50, Some(7_500)),
        ];
        let md = render(
            Date::new(2026, 8, 1).ok(),
            Date::new(2026, 8, 13).ok(),
            "origin/develop",
            None,
            &rows,
        );
        assert!(md.contains("# OUTRAM PARK — historian report"));
        assert!(md.contains("01 Aug 2026 → 13 Aug 2026"));
        assert!(md.contains("**Commits (non-merge):** 2"));
        // 350/1000 formats as "0.3": 0.35 is not exactly representable and
        // lands just below the midpoint. Python's `f"{:.1f}"` agrees.
        assert!(md.contains("| Added (all files) | 350 | 0.3 |"));
        assert!(md.contains("**Net (all files)** | **290**"));
        assert!(md.contains("| Removed (all files) | 60 | 0.1 |"));
        assert!(md.contains("| **total** | **12,500** |"));
        assert!(md.contains("| `tampines` | 350 |"));
        assert!(md.contains("**Token coverage:** 2/2"));
    }

    #[test]
    fn commits_without_trailers_show_a_dash_not_a_zero() {
        // The honesty rule: absent data must be visibly absent.
        let rows = vec![row("2026-07-01", "pre-hooks commit", 40, 0, None)];
        let md = render(None, None, "origin/develop", Some("origin/main"), &rows);
        assert!(md.contains("| 40 | 0 | — |"), "missing token data must render as an em dash");
        assert!(md.contains("**Token coverage:** 0/1"));
        assert!(md.contains("all of `origin/develop` not in `origin/main`"));
    }

    #[test]
    fn a_pipe_in_a_subject_is_escaped_so_the_table_survives() {
        let rows = vec![row("2026-08-13", "fix a|b parsing", 1, 0, Some(1))];
        let md = render(None, None, "develop", None, &rows);
        assert!(md.contains("fix a\\|b parsing"));
    }

    #[test]
    fn long_subjects_are_truncated() {
        let long = "x".repeat(200);
        let rows = vec![row("2026-08-13", &long, 1, 0, Some(1))];
        let md = render(None, None, "develop", None, &rows);
        assert!(md.contains(&format!("{}...", "x".repeat(61))));
    }

    #[test]
    fn default_output_path_uses_the_ddmmyy_tag() {
        let p = default_output_path(
            Path::new("/repo"),
            Date::new(2026, 8, 1).ok(),
            Date::new(2026, 8, 13).ok(),
            "main",
        );
        assert!(p.ends_with("historian_010826_to_130826.md"));
    }
}
