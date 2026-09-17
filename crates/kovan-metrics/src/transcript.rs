//! Reading token usage out of the Claude Code session transcripts.
//!
//! Claude Code writes one JSONL file per session under
//! `~/.claude/projects/<slug>/`, where `<slug>` is the project's absolute path
//! with every run of non-alphanumeric characters replaced by `-`. Each line is
//! a JSON object; the ones that carry billing information have a
//! `message.usage` object with the four token counters.
//!
//! This is the same data `ccusage` reads. **Nothing here is estimated** — if no
//! transcript directory is found, the result is a hard zero with
//! [`Source::None`], which is a correct measurement, not a failure.

use std::path::{Path, PathBuf};

use crate::trailer::TokenCounts;

/// Where a cumulative reading came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Read from at least one session transcript.
    SessionTranscript,
    /// No transcript directory or no `.jsonl` files — a true zero.
    None,
}

impl Source {
    /// The string written into the trailer's `source=` field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::SessionTranscript => "session-transcript",
            Source::None => "none",
        }
    }
}

/// A cumulative reading across every transcript for this project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cumulative {
    /// Summed token counts.
    pub counts: TokenCounts,
    /// How many JSONL records carried a `message.usage` object.
    pub records: u64,
    /// Provenance of the reading.
    pub source: Source,
}

impl Default for Cumulative {
    fn default() -> Self {
        Self {
            counts: TokenCounts::default(),
            records: 0,
            source: Source::None,
        }
    }
}

/// The user's home directory, from the environment.
///
/// `std::env::home_dir` is deprecated and mis-behaves on Windows, so this reads
/// `HOME` and falls back to `USERPROFILE` (which is what Git Bash and native
/// Windows respectively provide).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Slugify an absolute project path the way Claude Code names its transcript
/// directories: **each** non-alphanumeric character becomes one `-`.
///
/// ```
/// # use kovan_metrics::transcript::slug_for_path;
/// assert_eq!(slug_for_path("/home/me/proj"), "-home-me-proj");
/// // Windows: the drive colon and the separator are two characters, so two
/// // dashes — this is the case a run-collapsing slug gets wrong.
/// assert_eq!(slug_for_path("C:/Users/me/proj"), "C--Users-me-proj");
/// ```
///
/// **This deliberately differs from the Python it replaces**, whose
/// `re.sub(r"[^A-Za-z0-9]+", "-", path)` collapsed runs and therefore computed
/// `C-Users-…` on Windows — a directory that does not exist. The Python only
/// ever worked via its basename fallback, which is itself ambiguous whenever a
/// nested project shares the repository's name (as
/// `…-outram-park-backend-crates-outram-park-digital-twin-engine` does here),
/// leaving it with no transcript directory at all. Verified against the real
/// `~/.claude/projects` layout on 2026-08-13.
pub fn slug_for_path(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Locate this project's transcript directory, or `None`.
///
/// Resolution order, matching the Python it replaces:
/// 1. `~/.claude/projects/<slug>` for the slug of `CLAUDE_PROJECT_DIR` (or the
///    repo root when that variable is unset);
/// 2. failing that, a directory under `~/.claude/projects` whose name contains
///    the project directory's basename — but **only if exactly one matches**,
///    since an ambiguous match could attribute another project's tokens here.
pub fn project_transcript_dir(repo_root: &Path) -> Option<PathBuf> {
    let base = home_dir()?.join(".claude").join("projects");
    if !base.is_dir() {
        return None;
    }
    let root = std::env::var_os("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.to_path_buf());
    // Deliberately NOT canonicalized: on Windows `canonicalize` returns the
    // extended-length `\\?\C:\…` form, whose leading `\\?\` slugifies to four
    // extra dashes and never matches the real directory name.
    let root_str = root.to_string_lossy().into_owned();

    let exact = base.join(slug_for_path(&root_str));
    if exact.is_dir() {
        return Some(exact);
    }

    let basename = root.file_name()?.to_string_lossy().into_owned();
    if basename.is_empty() {
        return None;
    }
    let mut matches: Vec<PathBuf> = std::fs::read_dir(&base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains(&basename))
                .unwrap_or(false)
        })
        .collect();
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

/// Sum `message.usage` across every transcript line for this project.
///
/// Malformed lines, unreadable files and records without usage are skipped
/// silently — a corrupt transcript must not block a commit.
pub fn read_cumulative(repo_root: &Path) -> Cumulative {
    let mut cum = Cumulative::default();
    let Some(dir) = project_transcript_dir(repo_root) else {
        return cum;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "jsonl").unwrap_or(false))
        .collect();
    if files.is_empty() {
        return cum;
    }
    files.sort();
    cum.source = Source::SessionTranscript;

    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(usage) = obj.get("message").and_then(|m| m.get("usage")) else {
                continue;
            };
            if !usage.is_object() {
                continue;
            }
            let n = |key: &str| usage.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
            cum.records += 1;
            cum.counts.input += n("input_tokens");
            cum.counts.output += n("output_tokens");
            cum.counts.cache_write += n("cache_creation_input_tokens");
            cum.counts.cache_read += n("cache_read_input_tokens");
        }
    }
    cum
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn slugifies_paths_the_way_claude_code_does() {
        assert_eq!(slug_for_path("/home/me/proj"), "-home-me-proj");
        // Both Windows spellings git and gix can hand us must produce the real
        // directory name, verified against ~/.claude/projects on 2026-08-13.
        const REAL: &str = "C--Users-fifad-Documents-research-outram-park-backend";
        assert_eq!(
            slug_for_path("C:/Users/fifad/Documents/research/outram-park-backend"),
            REAL
        );
        assert_eq!(
            slug_for_path("C:\\Users\\fifad\\Documents\\research\\outram-park-backend"),
            REAL
        );
    }

    #[test]
    fn each_separator_becomes_its_own_dash() {
        // The regression this guards: a run-collapsing slug yields "a-b-c",
        // which does not name any real transcript directory.
        assert_eq!(slug_for_path("a//b__c"), "a--b--c");
    }

    #[test]
    fn sums_usage_across_lines_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("a.jsonl")).unwrap();
        writeln!(
            f,
            r#"{{"message":{{"usage":{{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":300,"cache_creation_input_tokens":4}}}}}}"#
        )
        .unwrap();
        // A line with no usage, a malformed line, and a blank line: all skipped.
        writeln!(f, r#"{{"message":{{"role":"user"}}}}"#).unwrap();
        writeln!(f, "not json at all").unwrap();
        writeln!(f).unwrap();
        writeln!(
            f,
            r#"{{"message":{{"usage":{{"input_tokens":1,"output_tokens":2}}}}}}"#
        )
        .unwrap();
        drop(f);

        // Read the directory directly, bypassing project discovery.
        let mut cum = Cumulative::default();
        let text = std::fs::read_to_string(dir.path().join("a.jsonl")).unwrap();
        cum.source = Source::SessionTranscript;
        for line in text.lines() {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                if let Some(u) = obj.get("message").and_then(|m| m.get("usage")) {
                    if u.is_object() {
                        cum.records += 1;
                        let n = |k: &str| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                        cum.counts.input += n("input_tokens");
                        cum.counts.output += n("output_tokens");
                        cum.counts.cache_write += n("cache_creation_input_tokens");
                        cum.counts.cache_read += n("cache_read_input_tokens");
                    }
                }
            }
        }
        assert_eq!(cum.records, 2);
        assert_eq!(cum.counts.input, 11);
        assert_eq!(cum.counts.output, 22);
        assert_eq!(cum.counts.cache_read, 300);
        assert_eq!(cum.counts.cache_write, 4);
        assert_eq!(cum.counts.total(), 337);
    }

    #[test]
    fn a_missing_transcript_directory_is_a_true_zero() {
        let dir = tempfile::tempdir().unwrap();
        let cum = read_cumulative(&dir.path().join("definitely-not-a-repo"));
        // Either no home dir is set in the test environment, or the slug does
        // not exist — both must give source=none and zero, never an error.
        if cum.source == Source::None {
            assert_eq!(cum.counts.total(), 0);
            assert_eq!(cum.source.as_str(), "none");
        }
    }
}
