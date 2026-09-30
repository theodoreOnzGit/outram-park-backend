//! Content fingerprints (SHA-256) of PDFs, with a disposable on-disk cache.
//!
//! What belongs here: hashing a file and remembering the hash, so the ingest
//! duplicate guard ([`crate::ingest::find_existing`]) can compare an incoming
//! PDF with every standard-corpus file and every paper's PDF without
//! re-reading tens of megabytes each time. What does not: deciding what
//! counts as a duplicate, which is the ingest module's job.
//!
//! # The cache
//!
//! `<root>/.kovan/pdf-sha256.json` ([`crate::root::STATE_DIR`]), keyed by
//! the file's canonical path and invalidated by its length and modification
//! time. It lives in the state directory because it is derived and
//! rebuildable: deleting it only costs a re-hash. A cache that cannot be read
//! or written is ignored, never an error.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// File name of the cache inside [`crate::root::KovanRoot::state_dir`].
pub const CACHE_FILE: &str = "pdf-sha256.json";

/// The SHA-256 of `path`'s bytes, lower-case hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// One remembered hash, valid while the file keeps this length and mtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    len: u64,
    mtime_ns: u128,
    sha256: String,
}

/// A path -> SHA-256 cache. [`HashCache::sha256`] consults and fills it;
/// [`HashCache::save`] writes it back when anything changed.
#[derive(Debug, Default)]
pub struct HashCache {
    file: Option<PathBuf>,
    entries: BTreeMap<String, Entry>,
    dirty: bool,
}

/// Length and modification time (ns since the epoch) of `path`.
fn stamp(path: &Path) -> Option<(u64, u128)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

impl HashCache {
    /// A cache that is never persisted.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// The cache stored in `state_dir`, or an empty one if it is absent or
    /// unreadable.
    pub fn load(state_dir: &Path) -> Self {
        let file = state_dir.join(CACHE_FILE);
        let entries = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            file: Some(file),
            entries,
            dirty: false,
        }
    }

    /// The SHA-256 of `path`, from the cache when its length and mtime still
    /// match, otherwise hashed now and remembered. `None` if it cannot be
    /// read.
    pub fn sha256(&mut self, path: &Path) -> Option<String> {
        let canonical = path.canonicalize().ok()?;
        let (len, mtime_ns) = stamp(&canonical)?;
        let key = canonical.to_string_lossy().into_owned();
        if let Some(e) = self.entries.get(&key) {
            if e.len == len && e.mtime_ns == mtime_ns {
                return Some(e.sha256.clone());
            }
        }
        let sha256 = sha256_file(&canonical).ok()?;
        self.entries.insert(
            key,
            Entry {
                len,
                mtime_ns,
                sha256: sha256.clone(),
            },
        );
        self.dirty = true;
        Some(sha256)
    }

    /// Write the cache back if it changed and has a file. Best effort: a
    /// failure only means the next check hashes again.
    pub fn save(&mut self) {
        let (Some(file), true) = (&self.file, self.dirty) else {
            return;
        };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.entries) {
            let tmp = file.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, file).is_ok() {
                self.dirty = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The known SHA-256 of "abc" (FIPS 180-2 test vector), and the cache
    /// persisting it and noticing a change.
    #[test]
    fn hashes_are_correct_cached_and_invalidated() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("a.pdf");
        std::fs::write(&f, b"abc").unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_file(&f).unwrap(), abc);

        let state = tmp.path().join(".kovan");
        let mut cache = HashCache::load(&state);
        assert_eq!(cache.sha256(&f).as_deref(), Some(abc));
        cache.save();
        assert!(state.join(CACHE_FILE).is_file());

        let mut again = HashCache::load(&state);
        assert_eq!(again.entries.len(), 1);
        assert_eq!(again.sha256(&f).as_deref(), Some(abc));
        assert!(!again.dirty, "served from the cache");

        std::fs::write(&f, b"abcd").unwrap();
        assert_ne!(again.sha256(&f).as_deref(), Some(abc), "length changed");
    }
}
