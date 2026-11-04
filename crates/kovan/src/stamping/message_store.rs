//! **Test commit messages kept on disk** (maintainer, 2026-11-03): the
//! answers of `git log --format=%B <commit> -- <test file>`, which the
//! stamp states read to see who wrote a stamp's reaching tests (an agent
//! trailer in a message), kept in `target/kovan/test_commit_messages.json`
//! across kovan runs.
//!
//! **Why.** Every stamp asks it for each test file that reached the stamped
//! function at the stamp's commit. Measured 2026-11-03 on this workspace,
//! from a cold start: 10 216 `git log` processes, 95 s of a 143 s refresh
//! (1013 s of git summed over the threads); the second refresh in the same
//! process took 11 s. The answers were kept in memory only (#856), so each
//! launch of kovan paid it again.
//!
//! **Why it cannot go stale.** Only answers for a full commit id are kept:
//! the commits up to a fixed commit that touched a file never change, so
//! the answer is the same in every later run. A file that is missing, from
//! another format version, or unreadable is ignored (the answers are asked
//! of git again); one that cannot be written costs only the time. Either
//! way the stamp states are the same, as the test
//! `kept_messages_survive_a_restart_and_match_git` checks.
//!
//! **A corrupt file is rebuilt** (maintainer, 2026-11-04: "regenerate cache
//! if it is corrupted"). Since that day the file carries a checksum of its
//! contents (`check`); one that fails it, or does not parse, is ignored as
//! a whole, and the answers git gives again are written afresh by the same
//! refresh. A file written before the checksum existed (no `check`) is
//! read as it is and rewritten with one by that refresh.
//!
//! ```text
//!  { "format": 1, "check": "sha256:…",
//!    "messages": ["msg A", "msg B", …],          each text once
//!    "logs": [ { "rev": "<sha>", "file": "x.rs", "m": [0, 1] }, … ] }
//! ```

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Where the answers are kept, under the workspace root.
pub(crate) const STORE: &str = "target/kovan/test_commit_messages.json";
const FORMAT: u32 = 1;

#[derive(Default)]
struct Store {
    logs: HashMap<(String, String), Vec<String>>,
    /// Answers added since the file was read or written.
    dirty: bool,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    format: u32,
    /// [`check`] of the contents; absent in a file written before it
    /// existed (module doc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    check: Option<String>,
    messages: Vec<String>,
    logs: Vec<Log>,
}

/// The checksum of the contents as written (module doc).
fn check(messages: &[String], logs: &[Log]) -> Option<String> {
    serde_json::to_string(&(messages, logs)).ok().map(|t| kovan_common::review::hash::sha256_tagged(t.as_bytes()))
}

#[derive(Serialize, Deserialize)]
struct Log {
    rev: String,
    file: String,
    m: Vec<u32>,
}

fn stores() -> &'static Mutex<HashMap<PathBuf, Store>> {
    static S: OnceLock<Mutex<HashMap<PathBuf, Store>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// The kept answers of `root`'s file, and whether it must be rewritten (it
/// has no checksum yet); empty when there is none, or it is corrupt or of
/// another format (module doc).
fn read(root: &Path) -> (HashMap<(String, String), Vec<String>>, bool) {
    let Ok(text) = std::fs::read_to_string(root.join(STORE)) else { return Default::default() };
    let Ok(d) = serde_json::from_str::<OnDisk>(&text) else { return Default::default() };
    if d.format != FORMAT {
        return Default::default();
    }
    let unchecked = match &d.check {
        Some(c) if check(&d.messages, &d.logs).as_ref() == Some(c) => false,
        Some(_) => return Default::default(),
        None => true,
    };
    let logs = d.logs
        .into_iter()
        .filter_map(|l| {
            let m: Option<Vec<String>> = l.m.iter().map(|&i| d.messages.get(i as usize).cloned()).collect();
            m.map(|m| ((l.rev, l.file), m))
        })
        .collect();
    (logs, unchecked)
}

fn with<T>(root: &Path, f: impl FnOnce(&mut Store) -> T) -> Option<T> {
    let mut s = stores().lock().ok()?;
    let store = s.entry(root.to_path_buf()).or_insert_with(|| {
        let (logs, dirty) = read(root);
        Store { logs, dirty }
    });
    Some(f(store))
}

/// The kept messages of the commits up to `rev` that touched `file`.
pub(crate) fn kept(root: &Path, rev: &str, file: &str) -> Option<Vec<String>> {
    with(root, |s| s.logs.get(&(rev.to_string(), file.to_string())).cloned()).flatten()
}

/// Keep an answer (`rev` must be a full commit id; module doc).
pub(crate) fn keep(root: &Path, rev: &str, file: &str, messages: Vec<String>) {
    with(root, |s| {
        s.logs.insert((rev.to_string(), file.to_string()), messages);
        s.dirty = true;
    });
}

/// Write `root`'s answers when any were added, through a temporary file
/// renamed into place, so a reader never sees half a file.
pub(crate) fn save(root: &Path) -> Result<(), String> {
    let Some(text) = with(root, |s| {
        if !s.dirty {
            return None;
        }
        s.dirty = false;
        let mut index: BTreeMap<&str, u32> = BTreeMap::new();
        let mut messages: Vec<String> = Vec::new();
        // Numbered in (rev, file) order, so the same answers always give
        // the same file (a HashMap's order differs between runs).
        let mut sorted: Vec<_> = s.logs.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        let logs: Vec<Log> = sorted
            .into_iter()
            .map(|((rev, file), ms)| Log {
                rev: rev.clone(),
                file: file.clone(),
                m: ms
                    .iter()
                    .map(|t| {
                        *index.entry(t.as_str()).or_insert_with(|| {
                            messages.push(t.clone());
                            (messages.len() - 1) as u32
                        })
                    })
                    .collect(),
            })
            .collect();
        serde_json::to_string(&OnDisk { format: FORMAT, check: check(&messages, &logs), messages, logs }).ok()
    })
    .flatten() else {
        return Ok(());
    };
    let path = root.join(STORE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Drop what this process holds for `root`, as a new kovan run starts.
#[cfg(test)]
pub(crate) fn forget(root: &Path) {
    if let Ok(mut s) = stores().lock() {
        s.remove(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: two answers sharing a message are kept and saved, the
    /// process's copy is dropped, and they are read back from the file;
    /// the shared text is stored once. A garbage file or another format
    /// version reads as empty, never a panic.
    #[test]
    fn answers_round_trip_and_a_bad_file_reads_as_empty() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let (a, b) = ("a".repeat(40), "b".repeat(40));
        keep(root, &a, "t.rs", vec!["one".into(), "shared".into()]);
        keep(root, &b, "t.rs", vec!["shared".into()]);
        save(root).unwrap();
        forget(root);
        assert_eq!(kept(root, &a, "t.rs").unwrap(), ["one", "shared"]);
        assert_eq!(kept(root, &b, "t.rs").unwrap(), ["shared"]);
        assert_eq!(kept(root, &a, "u.rs"), None);
        let text = std::fs::read_to_string(root.join(STORE)).unwrap();
        assert_eq!(text.matches("shared").count(), 1);
        // Corrupt: a message changed under its checksum, cut short, not
        // JSON, another format. All ignored.
        let altered = text.replacen("one", "two", 1);
        for bad in [altered.as_str(), &text[..text.len() / 2], "not json", r#"{"format":99,"messages":[],"logs":[]}"#] {
            std::fs::write(root.join(STORE), bad).unwrap();
            forget(root);
            assert_eq!(kept(root, &a, "t.rs"), None, "{bad}");
        }
        assert_eq!(save(root), Ok(()), "nothing new: nothing written");
        // Asked of git again and kept: the file is rewritten whole.
        keep(root, &a, "t.rs", vec!["one".into(), "shared".into()]);
        keep(root, &b, "t.rs", vec!["shared".into()]);
        save(root).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(STORE)).unwrap(), text);
        // A file from before the checksum: read as it is, rewritten with one.
        let old = r#"{"format":1,"messages":["m"],"logs":[{"rev":"c","file":"t.rs","m":[0]}]}"#;
        std::fs::write(root.join(STORE), old).unwrap();
        forget(root);
        assert_eq!(kept(root, "c", "t.rs").unwrap(), ["m"]);
        save(root).unwrap();
        let rewritten = std::fs::read_to_string(root.join(STORE)).unwrap();
        assert!(rewritten.contains("\"check\":\"sha256:"), "{rewritten}");
        forget(root);
        assert_eq!(kept(root, "c", "t.rs").unwrap(), ["m"], "and it passes its own check");
    }
}
