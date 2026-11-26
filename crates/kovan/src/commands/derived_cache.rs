//! `kovan-cli derived-cache pack | fetch` (GitHub #1779): the derived
//! `kovan.toml` cache ([`kovan_common::code_index::derived`]) as one zip, so
//! a machine without it (a fresh clone, CI) gets `callees`, `uses`, `lines`
//! and `reached_by` without a 27-minute `rust-analyzer scip` run.
//!
//! - **`pack`** (the nightly job, `.github/workflows/kovan-nightly-tests.yml`):
//!   every file under `target/kovan/derived/` into a zip, beside a manifest
//!   `kovan-derived.json` naming the commit it was built at.
//! - **`fetch`**: download such a zip (default: the `kovan-derived` release
//!   asset of the repository's GitHub `origin`) and unpack it into
//!   `target/kovan/derived/`. Only entries under that folder are written
//!   (a crafted path cannot escape it). The request carries a fixed
//!   `kovan` user agent and nothing that identifies the user.
//!
//! **A stale bundle is safe to use.** The cache is matched function by
//! function on id **and** `hash`: an entry for code that changed since the
//! bundle's commit is ignored and that function reads "index out of date"
//! until the next `kovan-cli index` (1 to 3 s per changed crate once a
//! SCIP index exists). `fetch` says how far the bundle's commit is from
//! `HEAD`.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use kovan_common::code_index::derived::DERIVED_DIR;

/// The manifest entry in the zip.
pub const MANIFEST: &str = "kovan-derived.json";

/// The release tag and asset name `fetch` downloads by default.
pub const RELEASE_TAG: &str = "kovan-derived";
pub const ASSET: &str = "kovan-derived.zip";

/// What a bundle says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    /// The commit the cache was built at.
    pub commit: String,
    /// How many cache files the zip holds.
    pub files: usize,
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Zip `root`'s `target/kovan/derived/` into `out`, recording `commit`.
/// Entries are in path order, so the same cache gives the same file list.
pub fn pack(root: &Path, commit: &str, out: &Path) -> Result<Manifest, String> {
    let base = root.join(DERIVED_DIR);
    let mut files = Vec::new();
    files_under(&base, &mut files);
    if files.is_empty() {
        return Err(format!("{DERIVED_DIR} is empty: run `kovan-cli index` first"));
    }
    let f = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let manifest = Manifest { schema: 1, commit: commit.to_string(), files: files.len() };
    zip.start_file(MANIFEST, opts).map_err(|e| e.to_string())?;
    zip.write_all(serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?.as_bytes())
        .map_err(|e| e.to_string())?;
    for p in &files {
        let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        zip.start_file(rel, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(manifest)
}

/// Whether zip entry `name` is a plain relative path inside the cache
/// folder (no `..`, no root, no drive).
fn safe_entry(name: &str) -> bool {
    let p = Path::new(name);
    name.starts_with(&format!("{DERIVED_DIR}/"))
        && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Unpack bundle `bytes` into `root`: the manifest is returned, every
/// entry under the cache folder written, anything else refused.
pub fn unpack(root: &Path, bytes: &[u8]) -> Result<Manifest, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a kovan-derived zip: {e}"))?;
    let manifest: Manifest = {
        let mut m = zip.by_name(MANIFEST).map_err(|_| format!("no {MANIFEST} in the zip"))?;
        let mut s = String::new();
        m.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("{MANIFEST}: {e}"))?
    };
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = e.name().to_string();
        if name == MANIFEST {
            continue;
        }
        if !safe_entry(&name) {
            return Err(format!("refusing zip entry {name:?}: outside {DERIVED_DIR}"));
        }
        let dest = root.join(&name);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let mut buf = Vec::new();
        e.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, buf).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    Ok(manifest)
}

/// The default bundle URL: the `kovan-derived` release of `origin` when it
/// is a GitHub repository (`https://github.com/o/r(.git)` or
/// `git@github.com:o/r(.git)`).
pub fn default_url(origin: &str) -> Option<String> {
    let o = origin.trim().trim_end_matches('/').trim_end_matches(".git");
    let path = o
        .strip_prefix("https://github.com/")
        .or_else(|| o.strip_prefix("git@github.com:"))
        .or_else(|| o.strip_prefix("ssh://git@github.com/"))?;
    let mut parts = path.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("https://github.com/{owner}/{repo}/releases/download/{RELEASE_TAG}/{ASSET}"))
}

/// What [`migrate`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Migrated {
    /// Code-folder `kovan.toml` files found (fixture copies excluded).
    pub files: usize,
    /// Of those, rewritten to the tracked fields only.
    pub rewritten: usize,
}

/// Split every code-folder `kovan.toml` under `root` **as it stands**
/// (GitHub #1780: one migration commit, a pure field removal): no source is
/// read, no hash recomputed. Each file's split is verified before anything
/// is written: the tracked text merged with the cache must give back the
/// file's index exactly (every id, hash, doc hash, `[test_run]`, review and
/// history kept), or the whole run stops with nothing written for that file.
pub fn migrate(root: &Path) -> Result<Migrated, String> {
    use kovan_common::code_index::derived;
    use kovan_common::review::index::FolderIndex;
    let mut out = Migrated::default();
    for p in kovan_discovery::discover(root, &["toml"]) {
        if p.file_name().and_then(|f| f.to_str()) != Some(super::index::KOVAN_TOML) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(fi) = FolderIndex::parse(&text) else { continue };
        let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        if fi.dir != dir {
            continue; // a fixture copy written for another folder
        }
        out.files += 1;
        let tracked = derived::tracked_text(&fi).map_err(|e| format!("{rel}: {e}"))?;
        let back = derived::merge(FolderIndex::parse(&tracked).map_err(|e| format!("{rel}: {e}"))?, Some(fi.clone()));
        let mut want = fi.clone();
        want.normalise();
        if back != want {
            return Err(format!("{rel}: the split does not give the index back; nothing written for it"));
        }
        derived::write_cache(root, &fi)?;
        if tracked != text {
            std::fs::write(&p, &tracked).map_err(|e| format!("{rel}: {e}"))?;
            out.rewritten += 1;
        }
    }
    Ok(out)
}

/// Download `url` (a fixed `kovan` user agent; nothing about the user).
#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(300))
        .redirects(10)
        .user_agent("kovan")
        .build();
    let resp = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// `main`/CLI glue over [`unpack`] (tested) and [`download`]: no test
/// reaches the network.
#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
pub fn fetch(root: &Path, url: &str) -> Result<Manifest, String> {
    let bytes = download(url)?;
    unpack(root, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: pack a cache folder of two files, unpack it into an
    /// empty workspace, and feed `unpack` a zip with an entry escaping the
    /// cache folder. Pass: the files and manifest round-trip; the escaping
    /// entry is refused; the default URL is formed from GitHub remotes only.
    ///
    /// Result (2026-11-26): passes.
    #[test]
    fn a_bundle_round_trips_and_cannot_write_outside_the_cache() {
        let a = tempfile::tempdir().unwrap();
        for (rel, text) in [("crates/x/src/kovan.derived.toml", "one"), ("kovan.derived.toml", "two")] {
            let p = a.path().join(DERIVED_DIR).join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let zip_path = a.path().join("b.zip");
        let m = pack(a.path(), "abc123", &zip_path).unwrap();
        assert_eq!((m.files, m.commit.as_str()), (2, "abc123"));
        let b = tempfile::tempdir().unwrap();
        let back = unpack(b.path(), &std::fs::read(&zip_path).unwrap()).unwrap();
        assert_eq!(back, m);
        let read = |rel: &str| std::fs::read_to_string(b.path().join(DERIVED_DIR).join(rel)).unwrap();
        assert_eq!((read("crates/x/src/kovan.derived.toml").as_str(), read("kovan.derived.toml").as_str()), ("one", "two"));

        let mut evil = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        evil.start_file(MANIFEST, o).unwrap();
        evil.write_all(br#"{"schema":1,"commit":"x","files":1}"#).unwrap();
        evil.start_file(format!("{DERIVED_DIR}/../../escape.txt"), o).unwrap();
        evil.write_all(b"no").unwrap();
        let bytes = evil.finish().unwrap().into_inner();
        assert!(unpack(b.path(), &bytes).unwrap_err().contains("refusing"));
        assert!(!b.path().join("escape.txt").exists());
        assert!(pack(b.path().join("empty").as_path(), "x", &b.path().join("e.zip")).is_err());

        let want = "https://github.com/o/r/releases/download/kovan-derived/kovan-derived.zip";
        assert_eq!(default_url("https://github.com/o/r.git").as_deref(), Some(want));
        assert_eq!(default_url("git@github.com:o/r.git\n").as_deref(), Some(want));
        assert_eq!(default_url("http://127.0.0.1:1234/git/o/r"), None);
    }

    /// Methodology: a workspace with one `kovan.toml` in the old format
    /// (derived fields inline) and a fixture copy written for another
    /// folder; migrate twice. Pass: the folder's file loses `lines` and
    /// `callees`, its cache appears, tracked + cache loads back the very
    /// index it held; the fixture is untouched; the second run rewrites
    /// nothing.
    ///
    /// Result (2026-11-26): passes.
    #[test]
    fn migration_is_a_pure_field_removal() {
        use kovan_common::review::index::{FolderIndex, FunctionIndex, ModuleIndex};
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let mut idx = FolderIndex::new("k", "crates/k/src");
        idx.modules.insert(
            "a.rs".into(),
            ModuleIndex {
                path: "crate::a".into(),
                functions: vec![FunctionIndex {
                    id: "fn:000000000000000a".into(),
                    name: "f".into(),
                    qual: "f".into(),
                    item: Default::default(),
                    shape: None,
                    lines: [3, 9],
                    hash: format!("sha256:{}", "a".repeat(64)),
                    doc_hash: format!("sha256:{}", "b".repeat(64)),
                    callees: vec!["fn:000000000000000b".into()],
                    uses: vec![],
                    reached_by: vec!["crates/k/tests/t.rs::t".into()],
                    test: false,
                    index_out_of_date: false,
                    physical_interface: false,
                }],
            },
        );
        let old = idx.to_toml().unwrap();
        for (rel, text) in [("crates/k/src/kovan.toml", &old), ("crates/k/tests/fixtures/kovan.toml", &old)] {
            std::fs::create_dir_all(root.join(rel).parent().unwrap()).unwrap();
            std::fs::write(root.join(rel), text).unwrap();
        }
        let m = migrate(root).unwrap();
        assert_eq!(m, Migrated { files: 1, rewritten: 1 });
        let tracked = std::fs::read_to_string(root.join("crates/k/src/kovan.toml")).unwrap();
        assert!(!tracked.contains("callees =") && !tracked.contains("lines =") && tracked.contains("hash ="));
        let back = kovan_common::code_index::derived::load(root, &tracked).unwrap();
        let mut want = idx;
        want.normalise();
        assert_eq!(back, want);
        assert_eq!(std::fs::read_to_string(root.join("crates/k/tests/fixtures/kovan.toml")).unwrap(), old);
        assert_eq!(migrate(root).unwrap(), Migrated { files: 1, rewritten: 0 });
    }
}
