//! **Watching the function under review** (GitHub #770; #740: "Kovan
//! watches the file. When it is saved, the function is re-hashed and the
//! flag changes from ⛔ needs fix to ✏ edited, ready for re-review").
//!
//! No file watcher exists in kovan (checked 2026-10-10: no `notify`
//! dependency, nothing watching files in `crates/kovan/src`), and adding a
//! dependency for one file is not worth it, so this **polls**: every
//! [`POLL_INTERVAL`] a worker reads the file and re-hashes the function's
//! own text ([`fingerprint`]: its source, doc comment included, as
//! [`super::diff::fn_source`] cuts it). When the fingerprint differs from
//! the last one seen, [`FnWatch::poll`] says so once and the review panel
//! reloads its view, which re-runs the staleness engine on the working
//! tree: an open needs-fix whose code changed is **fixed** (✏), a stamped
//! function directly stale. An edit elsewhere in the file changes nothing.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The read and the hash run on a
//! [`Worker`]; the UI thread only calls [`FnWatch::poll`], which takes a
//! finished result without blocking and starts the next read when due.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kovan_common::review::hash::sha256_tagged;

use crate::stamping::flow::Worker;

/// How often the file is read.
pub const POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// The function's fingerprint in the working tree: `sha256:` of its source
/// now; `None` when the file cannot be read or the function is not found
/// (a deletion or a rename is a change too).
pub fn fingerprint(root: &Path, file: &str, qual: &str) -> Option<String> {
    let text = std::fs::read_to_string(root.join(file)).ok()?;
    let (source, _) = super::diff::fn_source(&text, qual)?;
    Some(sha256_tagged(source.as_bytes()))
}

/// A poll of one function's fingerprint (module doc).
pub struct FnWatch {
    root: PathBuf,
    file: String,
    qual: String,
    interval: Duration,
    /// The last fingerprint seen; `None` until the first read finishes.
    last: Option<Option<String>>,
    job: Option<Worker<Option<String>>>,
    next: Instant,
}

impl FnWatch {
    /// Watch `qual` in `file` (workspace-relative) under `root`, reading
    /// every `interval`. The first read is the baseline, never a change.
    pub fn new(root: PathBuf, file: String, qual: String, interval: Duration) -> FnWatch {
        FnWatch {
            root,
            file,
            qual,
            interval,
            last: None,
            job: None,
            next: Instant::now(),
        }
    }

    /// Whether it watches `qual` in `file`.
    pub fn watches(&self, file: &str, qual: &str) -> bool {
        self.file == file && self.qual == qual
    }

    /// Whether the baseline has been read.
    pub fn started(&self) -> bool {
        self.last.is_some()
    }

    /// Record a read: `true` when it differs from the last one (the first
    /// read is the baseline, never a change).
    pub fn record(&mut self, fp: Option<String>) -> bool {
        let changed = matches!(&self.last, Some(prev) if *prev != fp);
        self.last = Some(fp);
        changed
    }

    /// Take a finished read (returning `true` once when the fingerprint
    /// changed since the last read) and start the next one when due. Never
    /// blocks.
    pub fn poll(&mut self, now: Instant) -> bool {
        let mut changed = false;
        if let Some(fp) = self.job.as_ref().and_then(Worker::try_take) {
            self.job = None;
            changed = self.record(fp);
            self.next = now + self.interval;
        }
        if self.job.is_none() && now >= self.next {
            let (root, file, qual) = (self.root.clone(), self.file.clone(), self.qual.clone());
            self.job = Some(Worker::spawn(move || fingerprint(&root, &file, &qual)));
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "/// Doubles x.\npub fn leaf(x: f64) -> f64 {\n    x * 2.0\n}\n\n/// One.\npub fn other() -> u32 {\n    1\n}\n";

    /// Methodology: the fingerprint and the change rule, then the worker
    /// loop on a temporary file with a zero interval. Pass: an edit to
    /// another function keeps `leaf`'s fingerprint, an edit to its body
    /// changes it, a missing file or function gives `None`; the first read
    /// is the baseline, a change is reported once, a disappearance is a
    /// change; polling sees a saved edit within the timeout.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn a_saved_edit_to_the_function_is_seen_once() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join("lib.rs");
        std::fs::write(&lib, SRC).unwrap();
        let base = fingerprint(d.path(), "lib.rs", "leaf").unwrap();
        std::fs::write(&lib, SRC.replace("    1\n", "    2\n")).unwrap();
        assert_eq!(
            fingerprint(d.path(), "lib.rs", "leaf").as_ref(),
            Some(&base)
        );
        std::fs::write(&lib, SRC.replace("x * 2.0", "x * 3.0")).unwrap();
        let edited = fingerprint(d.path(), "lib.rs", "leaf").unwrap();
        assert_ne!(edited, base);
        assert_eq!(fingerprint(d.path(), "nope.rs", "leaf"), None);
        assert_eq!(fingerprint(d.path(), "lib.rs", "nope"), None);

        let mut w = FnWatch::new(
            d.path().to_path_buf(),
            "lib.rs".into(),
            "leaf".into(),
            Duration::ZERO,
        );
        assert!(w.watches("lib.rs", "leaf") && !w.watches("lib.rs", "other"));
        assert!(!w.record(Some(base.clone())), "baseline");
        assert!(!w.record(Some(base.clone())));
        assert!(w.record(Some(edited.clone())));
        assert!(!w.record(Some(edited)), "once");
        assert!(w.record(None), "gone");

        // The worker loop.
        std::fs::write(&lib, SRC).unwrap();
        let mut w = FnWatch::new(
            d.path().to_path_buf(),
            "lib.rs".into(),
            "leaf".into(),
            Duration::ZERO,
        );
        let start = Instant::now();
        while !w.started() {
            assert!(!w.poll(Instant::now()), "the baseline is no change");
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(2));
        }
        std::fs::write(&lib, SRC.replace("x * 2.0", "x * 4.0")).unwrap();
        let mut seen = false;
        while !seen {
            seen = w.poll(Instant::now());
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "the edit was not seen"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
