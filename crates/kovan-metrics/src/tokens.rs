//! Token accounting: the write side (git hooks) and the query side (history).
//!
//! # Write side — driven by the git hooks
//!
//! - [`stamp_trailer`] (`prepare-commit-msg`) appends the `API-Usage-*` trailers
//!   to a commit message. **Idempotent**: a message that already carries the key
//!   is left untouched, so amend and rebase are safe.
//! - [`record`] (`post-commit`) advances the baseline and regenerates the ledger.
//! - [`report`] regenerates `docs/token-usage.md` from the commit trailers.
//! - [`init`] stamps the baseline (used by the installer).
//! - [`show`] prints the live cumulative reading.
//!
//! # Query side — reads the durable git record
//!
//! [`query`] sums the usage **recorded in commit trailers** over a date window
//! on any branch. It never reads the live transcripts, so it works for any
//! historical window, on any branch, from any clone.
//!
//! # The non-blocking contract
//!
//! Every function here returns `()` or a value and swallows its own errors. A
//! commit must never fail because token accounting failed — the worst
//! acceptable outcome is a missing or zero trailer. Callers in the hook path
//! must preserve this.

use std::path::Path;

use crate::date::Date;
use crate::git::{self, CommitRecord};
use crate::trailer::{self, TokenCounts};
use crate::transcript;

/// The generated ledger's path, relative to the repository root.
///
/// **Generated and gitignored** — regenerable from the commit trailers at any
/// time, deliberately not tracked (committing it on many branches caused
/// recurring merge conflicts). Never `git add` this file.
pub const LEDGER_REL: &str = "docs/token-usage.md";

/// Append the `API-Usage-*` trailers to the commit message at `msgfile`.
///
/// Does nothing when the message already carries the trailer key (amend and
/// rebase safety), when the file cannot be read, or when it cannot be written.
///
/// On the very first commit in a clone there is no baseline to subtract, so the
/// delta is stamped as zero and the source is suffixed `:baseline-initialised`
/// — an honest "we started measuring here" rather than attributing the whole
/// transcript history to one commit.
pub fn stamp_trailer(msgfile: &Path) {
    let Ok(body) = std::fs::read_to_string(msgfile) else {
        return;
    };
    if body.contains(trailer::TRAILER_KEY) {
        return;
    }

    let root = git::repo_root();
    let cum = transcript::read_cumulative(&root);
    let (delta, source) = match crate::baseline::load() {
        Some(base) => (
            cum.counts.saturating_sub(&base),
            cum.source.as_str().to_string(),
        ),
        None => {
            crate::baseline::save(&cum.counts, cum.records);
            (
                TokenCounts::default(),
                format!("{}:baseline-initialised", cum.source.as_str()),
            )
        }
    };
    let text = trailer::format(&delta, &cum.counts, &source);

    // Insert above git's comment block, matching the Python byte for byte:
    // everything before the first line starting with '#' is the message head.
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let insert_at = lines
        .iter()
        .position(|l| l.starts_with('#'))
        .unwrap_or(lines.len());
    let head = lines[..insert_at].concat();
    let head = head.trim_end_matches('\n');
    let tail = lines[insert_at..].concat();

    let mut new = format!("{head}\n\n{text}\n");
    if !tail.is_empty() {
        new.push('\n');
        new.push_str(&tail);
    }
    let _ = std::fs::write(msgfile, new);
}

/// Advance the baseline to the current cumulative, then regenerate the ledger.
pub fn record() {
    let root = git::repo_root();
    let cum = transcript::read_cumulative(&root);
    crate::baseline::save(&cum.counts, cum.records);
    report();
}

/// Read every commit that carries a trailer, oldest first.
fn ledger_rows() -> Vec<(CommitRecord, trailer::ParsedTrailer)> {
    let fmt = git::record_format();
    let raw = git::git_output(&["log", "--reverse", &fmt]);
    git::parse_records(&raw)
        .into_iter()
        .filter_map(|rec| trailer::parse(&rec.body).map(|t| (rec, t)))
        .collect()
}

/// Regenerate `docs/token-usage.md` from the commit trailers.
///
/// Writing failures are swallowed — this runs from `post-commit`, after the
/// commit already exists.
pub fn report() {
    let rows = ledger_rows();
    let mut totals = TokenCounts::default();
    let mut grand = 0u64;
    for (_, t) in &rows {
        totals.add(&t.counts);
        grand += t.total;
    }

    let g = trailer::group;
    let mut out = String::new();
    out.push_str("# API token usage per commit\n\n");
    out.push_str(
        "> **Auto-generated — do not hand-edit.** Regenerated on every commit by \
         `kovan tokens record` (via the `post-commit` hook) from the \
         `API-Usage-Since-Last-Commit` commit trailers. Rebuild with \
         `kovan tokens report`; query a period with \
         `kovan tokens query --from DDMMYY --to DDMMYY`.\n\n",
    );
    out.push_str("## Methodology & caveats\n\n");
    out.push_str(
        "- **Source.** Counts come from the Claude Code session transcripts \
         (`~/.claude/projects/<slug>/*.jsonl`, the same data `ccusage` reads). Nothing is invented.\n",
    );
    out.push_str(
        "- **Attribution is temporal**, not per-diff: each row is the usage recorded \
         *between the previous commit and this one*.\n",
    );
    out.push_str(
        "- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read dominates; shown separately.\n",
    );
    out.push_str(
        "- Commits authored outside a Claude session show `0` (`source=none`) and are omitted below.\n\n",
    );
    out.push_str("## Per-commit ledger\n\n");
    out.push_str("| Date | Commit | Subject | Total | in | out | cache_read | cache_write |\n");
    out.push_str("|---|---|---|--:|--:|--:|--:|--:|\n");
    for (rec, t) in &rows {
        let mut subj = rec.subject.replace('|', "\\|");
        if subj.chars().count() > 60 {
            subj = subj.chars().take(57).collect::<String>() + "...";
        }
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} |\n",
            rec.date,
            rec.short,
            subj,
            g(t.total),
            g(t.counts.input),
            g(t.counts.output),
            g(t.counts.cache_read),
            g(t.counts.cache_write),
        ));
    }
    out.push_str(&format!(
        "| **TOTAL** | | **{} commits** | **{}** | **{}** | **{}** | **{}** | **{}** |\n",
        rows.len(),
        g(grand),
        g(totals.input),
        g(totals.output),
        g(totals.cache_read),
        g(totals.cache_write),
    ));

    let path = git::repo_root().join(LEDGER_REL);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, out);
}

/// Stamp the baseline at the current cumulative reading (installer entry point).
pub fn init() {
    let root = git::repo_root();
    let cum = transcript::read_cumulative(&root);
    crate::baseline::save(&cum.counts, cum.records);
    println!(
        "token-accounting baseline initialised at {}",
        crate::baseline::path().display()
    );
}

/// Print the live cumulative reading and the delta since the last commit.
pub fn show() {
    let root = git::repo_root();
    let cum = transcript::read_cumulative(&root);
    let delta = match crate::baseline::load() {
        Some(base) => cum.counts.saturating_sub(&base),
        None => TokenCounts::default(),
    };
    let g = trailer::group;
    println!("source:             {}", cum.source.as_str());
    println!("transcript records: {}", cum.records);
    println!("cumulative total:   {}", g(cum.counts.total()));
    println!("  cumulative input       : {:>15}", g(cum.counts.input));
    println!("  cumulative output      : {:>15}", g(cum.counts.output));
    println!(
        "  cumulative cache_write : {:>15}",
        g(cum.counts.cache_write)
    );
    println!(
        "  cumulative cache_read  : {:>15}",
        g(cum.counts.cache_read)
    );
    println!("since last commit:  {}", g(delta.total()));
}

/// One commit's contribution to a [`query`] result.
#[derive(Debug, Clone)]
pub struct QueryRow {
    /// Author date, `YYYY-MM-DD`.
    pub date: String,
    /// Abbreviated commit hash.
    pub commit: String,
    /// Commit subject.
    pub subject: String,
    /// Recorded total, or `None` when the commit carries no usable trailer.
    pub total: Option<u64>,
}

/// The outcome of a [`query`] over a window of history.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// The ref actually reported on (e.g. `origin/develop`).
    pub branch: String,
    /// Window start, if bounded.
    pub from: Option<Date>,
    /// Window end, if bounded.
    pub to: Option<Date>,
    /// Non-merge commits in the window.
    pub commits_total: usize,
    /// How many of those carried real token data.
    pub commits_with_data: usize,
    /// Summed components.
    pub totals: TokenCounts,
    /// Summed `total=` fields as recorded.
    pub grand_total: u64,
    /// Per-commit rows, oldest first.
    pub rows: Vec<QueryRow>,
}

/// Sum the token usage recorded in commit trailers over a date window.
///
/// Reads the **durable git record**, not the live transcripts, so it is valid
/// for any window on any branch. Commits with no trailer, or with
/// `source=none`, contribute zero and are reported as having no data — which is
/// correct for commits made before the hooks existed or outside a Claude
/// session.
pub fn query(from: Option<Date>, to: Option<Date>, branch: &str) -> QueryResult {
    let git_ref = git::ref_for_branch(branch);
    let fmt = git::record_format();
    let since = from.map(|d| format!("--since={} 00:00:00", d.iso()));
    let until = to.map(|d| format!("--until={} 23:59:59", d.iso()));

    let mut args: Vec<&str> = vec!["log", "--no-merges", "--reverse", &fmt];
    if let Some(s) = since.as_deref() {
        args.push(s);
    }
    if let Some(u) = until.as_deref() {
        args.push(u);
    }
    args.push(&git_ref);

    let raw = git::git_output(&args);
    let mut result = QueryResult {
        branch: git_ref.clone(),
        from,
        to,
        commits_total: 0,
        commits_with_data: 0,
        totals: TokenCounts::default(),
        grand_total: 0,
        rows: Vec::new(),
    };

    for rec in git::parse_records(&raw) {
        result.commits_total += 1;
        match trailer::parse(&rec.body).filter(|t| t.has_data()) {
            Some(t) => {
                result.commits_with_data += 1;
                result.totals.add(&t.counts);
                result.grand_total += t.total;
                result.rows.push(QueryRow {
                    date: rec.date,
                    commit: rec.short,
                    subject: rec.subject,
                    total: Some(t.total),
                });
            }
            None => result.rows.push(QueryRow {
                date: rec.date,
                commit: rec.short,
                subject: rec.subject,
                total: None,
            }),
        }
    }
    result
}

impl QueryResult {
    /// Render as JSON, optionally including the per-commit breakdown.
    pub fn to_json(&self, per_commit: bool) -> String {
        let mut v = serde_json::json!({
            "branch": self.branch,
            "from": self.from.map(|d| d.iso()),
            "to": self.to.map(|d| d.iso()),
            "commits_total": self.commits_total,
            "commits_with_token_data": self.commits_with_data,
            "total": self.grand_total,
            "input": self.totals.input,
            "output": self.totals.output,
            "cache_read": self.totals.cache_read,
            "cache_write": self.totals.cache_write,
        });
        if per_commit {
            v["per_commit"] = serde_json::Value::Array(
                self.rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "date": r.date,
                            "commit": r.commit,
                            "subject": r.subject,
                            "total": r.total,
                        })
                    })
                    .collect(),
            );
        }
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string())
    }

    /// Print the human-facing summary.
    pub fn print(&self, per_commit: bool) {
        let g = trailer::group;
        let span = match (self.from, self.to) {
            (None, None) => "full history".to_string(),
            (f, t) => format!(
                "{} .. {}",
                f.map(|d| d.iso()).unwrap_or_else(|| "—".into()),
                t.map(|d| d.iso()).unwrap_or_else(|| "—".into())
            ),
        };
        println!(
            "Token usage recorded in commits on {}  ({span})",
            self.branch
        );
        println!(
            "  commits:            {}  ({} with token trailers)",
            self.commits_total, self.commits_with_data
        );
        println!("  total:              {}", g(self.grand_total));
        println!("    input:            {}", g(self.totals.input));
        println!("    output:           {}", g(self.totals.output));
        println!("    cache_read:       {}", g(self.totals.cache_read));
        println!("    cache_write:      {}", g(self.totals.cache_write));
        if per_commit {
            println!("  per-commit:");
            for r in &self.rows {
                let tk = match r.total {
                    Some(t) => format!("{:>15}", g(t)),
                    None => format!("{:>15}", "—"),
                };
                let subj: String = r.subject.chars().take(60).collect();
                println!("    {}  {}  {}  {}", r.date, r.commit, tk, subj);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Write a commit-message file and run the trailer insertion over it.
    fn stamp(content: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("COMMIT_EDITMSG");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);
        stamp_trailer(&p);
        std::fs::read_to_string(&p).unwrap()
    }

    #[test]
    fn stamping_is_idempotent() {
        let already = format!(
            "subject\n\n{}: total=5 in=1 out=1 cache_read=3 cache_write=0 source=none\n",
            trailer::TRAILER_KEY
        );
        assert_eq!(stamp(&already), already, "must not double-stamp");
    }

    #[test]
    fn the_trailer_lands_above_gits_comment_block() {
        let out = stamp("my subject\n\nmy body\n\n# Please enter the commit message\n# lines\n");
        let key_at = out.find(trailer::TRAILER_KEY).expect("trailer written");
        let hash_at = out.find("# Please enter").expect("comments preserved");
        assert!(key_at < hash_at, "trailer must precede the comment block");
        assert!(out.starts_with("my subject"));
        assert!(out.contains("# lines"), "comment block must survive");
    }

    #[test]
    fn a_message_with_no_comment_block_gets_the_trailer_appended() {
        let out = stamp("just a subject\n");
        assert!(out.starts_with("just a subject"));
        assert!(out.contains(trailer::TRAILER_KEY));
        assert!(out.contains(trailer::CUMULATIVE_KEY));
    }

    #[test]
    fn an_unreadable_message_file_is_a_no_op_not_a_panic() {
        // The hook contract: never block a commit.
        stamp_trailer(Path::new("/definitely/not/a/real/path/COMMIT_EDITMSG"));
    }

    #[test]
    fn query_json_is_wellformed_and_carries_the_component_split() {
        let r = QueryResult {
            branch: "origin/develop".into(),
            from: Some(Date::new(2026, 8, 1).unwrap()),
            to: Some(Date::new(2026, 8, 13).unwrap()),
            commits_total: 3,
            commits_with_data: 2,
            totals: TokenCounts {
                input: 1,
                output: 2,
                cache_read: 3,
                cache_write: 4,
            },
            grand_total: 10,
            rows: vec![QueryRow {
                date: "2026-08-13".into(),
                commit: "abc1234".into(),
                subject: "a commit".into(),
                total: Some(10),
            }],
        };
        let v: serde_json::Value = serde_json::from_str(&r.to_json(true)).unwrap();
        assert_eq!(v["branch"], "origin/develop");
        assert_eq!(v["from"], "2026-08-01");
        assert_eq!(v["total"], 10);
        assert_eq!(v["cache_read"], 3);
        assert_eq!(v["commits_with_token_data"], 2);
        assert_eq!(v["per_commit"][0]["commit"], "abc1234");
    }

    #[test]
    fn query_json_omits_per_commit_unless_asked() {
        let r = QueryResult {
            branch: "develop".into(),
            from: None,
            to: None,
            commits_total: 0,
            commits_with_data: 0,
            totals: TokenCounts::default(),
            grand_total: 0,
            rows: Vec::new(),
        };
        let v: serde_json::Value = serde_json::from_str(&r.to_json(false)).unwrap();
        assert!(v.get("per_commit").is_none());
        assert!(v["from"].is_null());
    }
}
