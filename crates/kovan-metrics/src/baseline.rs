//! The per-commit baseline — how "since the last commit" is computed.
//!
//! The transcripts only ever grow, so a *cumulative* reading is not by itself
//! attributable to a commit. The baseline is the cumulative reading as of the
//! previous commit; the delta stamped into a trailer is `now - baseline`, and
//! the `post-commit` hook advances the baseline afterwards.
//!
//! It lives at `<git-dir>/claude-token-baseline.json` — inside `.git/`, so it
//! is per-clone, never committed, and cannot collide across worktrees.
//!
//! Attribution is therefore **temporal, not per-diff**: a commit is charged the
//! tokens spent between the previous commit and itself, whatever files those
//! tokens actually touched.

use std::path::PathBuf;

use crate::git;
use crate::trailer::TokenCounts;

/// Absolute path to the baseline file.
pub fn path() -> PathBuf {
    git::git_dir().join("claude-token-baseline.json")
}

/// Read the stored baseline, or `None` when this clone has never stamped one.
///
/// Any failure — missing file, unreadable, malformed JSON — reads as `None`,
/// which the caller treats as "first run" rather than an error.
pub fn load() -> Option<TokenCounts> {
    let text = std::fs::read_to_string(path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let n = |key: &str| v.get(key).and_then(|x| x.as_u64()).unwrap_or(0);
    Some(TokenCounts {
        input: n("input"),
        output: n("output"),
        cache_read: n("cache_read"),
        cache_write: n("cache_write"),
    })
}

/// Write `counts` as the new baseline, recording `records` for diagnostics.
///
/// Errors are swallowed: failing to advance the baseline must never abort a
/// commit. The next commit simply attributes a larger window.
pub fn save(counts: &TokenCounts, records: u64) {
    let data = serde_json::json!({
        "input": counts.input,
        "output": counts.output,
        "cache_read": counts.cache_read,
        "cache_write": counts.cache_write,
        "records": records,
    });
    if let Ok(text) = serde_json::to_string(&data) {
        let _ = std::fs::write(path(), text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_baseline_document() {
        let json = r#"{"input":1,"output":2,"cache_read":3,"cache_write":4,"records":9}"#;
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        let counts = TokenCounts {
            input: n("input"),
            output: n("output"),
            cache_read: n("cache_read"),
            cache_write: n("cache_write"),
        };
        assert_eq!(counts.total(), 10);
    }

    #[test]
    fn a_missing_field_reads_as_zero_not_an_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"input":5}"#).unwrap();
        let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        assert_eq!(n("input"), 5);
        assert_eq!(n("cache_read"), 0);
    }

    #[test]
    fn the_baseline_lives_inside_the_git_directory() {
        // Never in the working tree: it must not be committable.
        assert!(path().ends_with("claude-token-baseline.json"));
    }
}
