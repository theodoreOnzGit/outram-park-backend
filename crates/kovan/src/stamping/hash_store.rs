//! **Function hashes at the test-run commit, kept by blob** (GitHub #1198).
//!
//! A refresh compares each function's hash now with its hash at the commit
//! of the folder's last test run (`hashes_at_test_run`, and the targeted
//! run's). That meant reading every indexed file at that commit and parsing
//! it. Measured 2026-11-04 on this workspace (68 671 functions, 4 634
//! files): 7.3 s of every refresh, 6.4 s of it parsing on one thread, and
//! nothing was kept, so the second refresh paid it again.
//!
//! Now each file's hashes are kept under its **git blob id** (the id of the
//! file's content), in `target/kovan/test_run_hashes.json`, and the files
//! not kept are parsed on worker threads (two cores left free). Most files
//! are the same blob from one test run to the next, so a new test run
//! re-parses only what changed.
//!
//! **Why it cannot go stale.** The hashes of a text depend on the text and
//! on the hashing code. The text is fixed by the blob id. The hashing code
//! is fixed by the kovan program: the file records which program wrote it
//! ([`hasher`]: its version, size and modification time), and a file from
//! another build is ignored, so a rebuilt kovan parses again once.
//!
//! **A corrupt file is rebuilt** (maintainer, 2026-11-04: "regenerate cache
//! if it is corrupted"). The file carries a checksum of its own contents
//! (`check`, sha256 of the `blobs` as written). A file that does not parse,
//! fails its checksum (a bit flip, a hand edit, a truncated write), or
//! comes from another format or program is ignored as a whole: every hash
//! is computed again and the file is written afresh by the same refresh.
//! One that cannot be written costs only time. The test `kept_test_run_hashes_match_a_fresh_parse`
//! checks the answers are those of [`super::states::fn_at`].
//!
//! Only blobs a refresh asked about are written, so the file holds the
//! current test runs' files and does not grow with history.
//!
//! ```text
//!  { "format": 1, "hasher": "<version> <size> <mtime>", "check": "sha256:…",
//!    "blobs": { "<blob id>": [["Fractions::sum", "sha256:…"], …]   one file's items, in order
//!               "<blob id>": null, … } }                            not Rust that parses
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use kovan_common::review::hash::hash_all;
use serde::{Deserialize, Serialize};

use super::git_batch::GitBatch;

/// Where the hashes are kept, under the workspace root.
pub(crate) const STORE: &str = "target/kovan/test_run_hashes.json";
const FORMAT: u32 = 1;

/// A file's items in order: (qualified name, `hash`). `None`: the text
/// does not parse.
pub(crate) type Items = Option<Arc<Vec<(String, String)>>>;

#[derive(Default)]
struct Store {
    blobs: HashMap<String, Items>,
    /// Blob ids asked about since the file was read (what is written).
    used: BTreeSet<String>,
    /// Hashes computed since the file was read or written.
    dirty: bool,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    format: u32,
    hasher: String,
    /// [`check`] of `blobs` (module doc).
    check: String,
    blobs: Blobs,
}

type Blobs = BTreeMap<String, Option<Vec<(String, String)>>>;

/// The checksum of `blobs` as written (module doc).
fn check(blobs: &Blobs) -> Option<String> {
    serde_json::to_string(blobs).ok().map(|t| kovan_common::review::hash::sha256_tagged(t.as_bytes()))
}

/// The program that hashes (module doc): `None` when it cannot be told,
/// and then nothing is read or written.
fn hasher() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let m = std::fs::metadata(exe).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(format!("{} {} {}", env!("CARGO_PKG_VERSION"), m.len(), t.as_nanos()))
}

fn stores() -> &'static Mutex<HashMap<PathBuf, Store>> {
    static S: OnceLock<Mutex<HashMap<PathBuf, Store>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// `root`'s kept hashes; empty when there is no file, or it is corrupt or
/// from another format or program (module doc).
fn read(root: &Path) -> HashMap<String, Items> {
    let Some(me) = hasher() else { return HashMap::new() };
    let Ok(text) = std::fs::read_to_string(root.join(STORE)) else { return HashMap::new() };
    let Ok(d) = serde_json::from_str::<OnDisk>(&text) else { return HashMap::new() };
    if d.format != FORMAT || d.hasher != me || check(&d.blobs).as_deref() != Some(d.check.as_str()) {
        return HashMap::new();
    }
    d.blobs.into_iter().map(|(id, v)| (id, v.map(Arc::new))).collect()
}

fn with<T>(root: &Path, f: impl FnOnce(&mut Store) -> T) -> Option<T> {
    let mut s = stores().lock().ok()?;
    let store = s.entry(root.to_path_buf()).or_insert_with(|| Store {
        blobs: read(root),
        ..Store::default()
    });
    Some(f(store))
}

/// The items of `file` at `rev`, for each wanted pair: kept ones by blob
/// id, the rest parsed on worker threads (module doc). A pair missing from
/// the answer is a file absent at `rev`.
pub(crate) fn items_at(git: &mut GitBatch, wanted: &BTreeSet<(String, String)>) -> HashMap<(String, String), Items> {
    let root = git.root().to_path_buf();
    let mut out = HashMap::new();
    // (pair, blob id) to parse; the texts are read here, one `cat-file`.
    let mut todo: Vec<((String, String), Option<String>, String)> = Vec::new();
    for (rev, file) in wanted {
        let Some((id, text)) = git.show_with_id(rev, file) else { continue };
        let kept = id.as_ref().and_then(|id| {
            with(&root, |s| {
                s.used.insert(id.clone());
                s.blobs.get(id).cloned()
            })
            .flatten()
        });
        match kept {
            Some(items) => {
                out.insert((rev.clone(), file.clone()), items);
            }
            None => todo.push(((rev.clone(), file.clone()), id, text)),
        }
    }
    if todo.is_empty() {
        return out;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(2)).max(1);
    let per = todo.len().div_ceil(threads).max(1);
    let parsed: Vec<(usize, Items)> = std::thread::scope(|scope| {
        let handles: Vec<_> = todo
            .chunks(per)
            .enumerate()
            .map(|(c, part)| {
                scope.spawn(move || {
                    part.iter()
                        .enumerate()
                        .map(|(i, (_, _, text))| {
                            let items = hash_all(text)
                                .ok()
                                .map(|fns| Arc::new(fns.into_iter().map(|h| (h.entry.qualname(), h.hashes.hash)).collect()));
                            (c * per + i, items)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("hashing does not panic")).collect()
    });
    for (i, items) in parsed {
        let (pair, id, _) = &todo[i];
        if let Some(id) = id {
            with(&root, |s| {
                s.blobs.insert(id.clone(), items.clone());
                s.dirty = true;
            });
        }
        out.insert(pair.clone(), items);
    }
    out
}

/// The `hash` of `qual` among `items` ([`super::states::pick`]).
pub(crate) fn hash_of(items: &Items, qual: &str) -> Option<String> {
    let items = items.as_ref()?;
    super::states::pick(items.iter().map(|(n, _)| n.clone()), qual).map(|i| items[i].1.clone())
}

/// Write `root`'s hashes when any were computed, only the blobs asked
/// about, through a temporary file renamed into place.
pub(crate) fn save(root: &Path) -> Result<(), String> {
    let Some(me) = hasher() else { return Ok(()) };
    let Some(text) = with(root, |s| {
        if !s.dirty {
            return None;
        }
        s.dirty = false;
        let blobs: Blobs = s
            .used
            .iter()
            .filter_map(|id| Some((id.clone(), s.blobs.get(id)?.as_ref().map(|v| v.as_ref().clone()))))
            .collect();
        serde_json::to_string(&OnDisk { format: FORMAT, hasher: me, check: check(&blobs)?, blobs }).ok()
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
    use super::super::git;
    use super::super::states::fn_at;
    use super::*;

    /// Methodology (#1198): a throwaway repository with a file holding two
    /// same-named functions (`#1`, `#2`), a method, a type and a constant,
    /// and a file that is not Rust. Each name is looked up through the
    /// store and through [`fn_at`] (a fresh parse). Pass: the hashes agree
    /// for every name, a name not in the file is absent from both, the bad
    /// file reads as "does not parse", and an absent file is missing. The
    /// hashes are then saved, the process's copy dropped (a new kovan run)
    /// and asked again: same answers, with nothing parsed. A file written by
    /// another program is ignored.
    #[test]
    fn kept_test_run_hashes_match_a_fresh_parse() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let run = |a: &[&str]| git::git(p, a).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.org"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "commit.gpgsign", "false"]);
        let lib = "#[cfg(unix)]\nfn f() -> u8 { 1 }\n#[cfg(not(unix))]\nfn f() -> u8 { 2 }\n\
                   /// A sum.\npub struct S;\nimpl S {\n    fn sum(&self) -> u8 { f() + 1 }\n}\nconst N: u8 = 3;\n";
        std::fs::write(p.join("lib.rs"), lib).unwrap();
        std::fs::write(p.join("bad.rs"), "fn (").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "add"]);
        let head = run(&["rev-parse", "HEAD"]).trim().to_string();
        let wanted: BTreeSet<(String, String)> =
            ["lib.rs", "bad.rs", "gone.rs"].iter().map(|f| (head.clone(), f.to_string())).collect();
        let quals = ["f#1", "f#2", "f", "S::sum", "S", "N", "missing"];
        let check = |items: &HashMap<(String, String), Items>| {
            let lib_items = &items[&(head.clone(), "lib.rs".to_string())];
            let mut found = 0;
            for q in quals {
                let fresh = fn_at(&mut GitBatch::new(p), &head, "lib.rs", q).map(|h| h.hashes.hash);
                assert_eq!(hash_of(lib_items, q), fresh, "{q}");
                found += usize::from(fresh.is_some());
            }
            assert_eq!(found, 5, "f#1, f#2, S::sum, S and N; not a bare f, nor missing");
            assert!(items[&(head.clone(), "bad.rs".to_string())].is_none());
            assert!(!items.contains_key(&(head.clone(), "gone.rs".to_string())));
        };
        forget(p);
        let first = items_at(&mut GitBatch::new(p), &wanted);
        check(&first);
        save(p).unwrap();
        forget(p);
        let again = items_at(&mut GitBatch::new(p), &wanted);
        check(&again);
        assert_eq!(with(p, |s| s.dirty), Some(false), "read from the kept file, nothing parsed");
        // Corrupt (it still parses but a hash was changed, or it does not
        // parse, or it is cut short) or written by another build of kovan:
        // ignored, every hash computed again, and the file rewritten whole.
        let good = std::fs::read_to_string(p.join(STORE)).unwrap();
        let a_hash = good.split("\"sha256:").nth(1).unwrap()[..64].to_string();
        let bad = [
            good.replacen(&a_hash, &"0".repeat(64), 1),
            good.replace(&hasher().unwrap(), "0.0.0 1 2"),
            good[..good.len() / 2].to_string(),
            "not json".to_string(),
        ];
        for (i, text) in bad.iter().enumerate() {
            std::fs::write(p.join(STORE), text).unwrap();
            forget(p);
            check(&items_at(&mut GitBatch::new(p), &wanted));
            assert_eq!(with(p, |s| s.dirty), Some(true), "case {i}: parsed again");
            save(p).unwrap();
            assert_eq!(std::fs::read_to_string(p.join(STORE)).unwrap(), good, "case {i}: rewritten as it was");
        }
        forget(p);
    }
}
