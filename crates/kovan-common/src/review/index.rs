//! The per-folder code-review **`kovan.toml`** (maintainer, #739 decisions
//! 2, 8, 10, 11, 13 and "kovan.toml is a disposable cache", 2026-10-07).
//!
//! One per folder, covering every `.rs` file in it. Machine-owned: generated
//! deterministically from the source, `review.md` and git, never
//! hand-edited, and **fully rebuildable**. A missing, malformed,
//! hand-edited or conflicted one is regenerated silently by the caller; this
//! module only reads and writes it and reports, with [`IndexError`], why a
//! file could not be read.
//!
//! It is a **new kind** of `kovan.toml` (`kind = "code_folder"`), so the
//! literature entity `kovan.toml` files (`kind = "paper"`, …) are untouched,
//! and [`FolderIndex::parse`] refuses one of those with
//! [`IndexError::NotACodeFolder`] rather than misreading it.
//!
//! ```toml
//! schema_version = 1
//! kind = "code_folder"
//! crate = "tampines"
//! dir = "crates/tampines/src"
//! commit = "<sha>"                  # the commit it was generated at
//!
//! [module."steam.rs"]
//! path = "crate::steam"
//!
//! [[module."steam.rs".function]]
//! id = "crates/tampines/src/steam.rs::SteamTable::flash"   # stable join key
//! name = "flash"                    # display only
//! qual = "SteamTable::flash"        # the code-walk path today
//! lines = [120, 158]
//! hash = "sha256:…"
//! doc_hash = "sha256:…"
//! callees = ["crates/tampines/src/steam.rs::saturation"]
//! reached_by = ["crates/tampines/tests/flash.rs::flash_matches_iapws"]
//!
//! [test_run]                        # last `kovan-cli test` evidence
//! commit = "<sha>"
//! cargo_lock = "sha256:…"
//! suite = "full"
//! passed = ["…"]
//! failed = []
//! edited = []                       # tests edited in the change: never count
//!
//! [upstream]                        # cached from review.md
//! …
//!
//! [[review]]                        # cached list of review.md's reviews
//! function = "…"
//! by = "github:…"
//! artifact = "review-…"
//!
//! [[deleted_folder]]                # crate root folder only (#739 D6)
//! dir = "crates/tampines/src/old"
//! [[deleted_folder.function]]
//! …
//! ```
//!
//! # Stable ids
//!
//! A function's `id` is the call graph's key ([`crate::call_graph`]
//! `function_ids`: `file.rs::name` or `file.rs::Type::name`, `#k` when not
//! unique) **at the time the function was first indexed**, and it is kept
//! when the function is renamed or moved: the indexer matches ids back from
//! `review.md` (the staleness engine, #765, reports the matches), so
//! the review's join key keeps working. `qual` and the folder/file say where
//! it is now.
//!
//! # Determinism
//!
//! Modules are a `BTreeMap` keyed by file name; [`FolderIndex::to_toml`]
//! sorts functions by first line then id, and every list it owns.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::review_md::{DeletedFunction, UpstreamTable};
use super::types::{check_commit, check_hash, FieldError};

/// The `schema_version` this module writes. Additive changes never bump it
/// (decision 11); readers accept any version up to this one.
pub const INDEX_SCHEMA_VERSION: u32 = 1;

/// `kind = "code_folder"`.
pub const CODE_FOLDER_KIND: &str = "code_folder";

/// What kind of item an entry is. Functions are hashed today; the
/// maintainer ruled (#739, 2026-10-06) that nested functions, constants and
/// types are stamped too, which later indexers add without a schema change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    #[default]
    Fn,
    Const,
    Static,
    Type,
}

/// One function (item) of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionIndex {
    /// The stable join key (module doc).
    pub id: String,
    /// Display only.
    pub name: String,
    /// The code-walk path within the file (`name` or `Type::name`).
    pub qual: String,
    #[serde(default, skip_serializing_if = "is_fn")]
    pub item: ItemKind,
    /// 1-based inclusive line range, doc comment included.
    pub lines: [u32; 2],
    pub hash: String,
    pub doc_hash: String,
    /// Resolved workspace callees, by stable id (std, external and
    /// unresolved calls are not listed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callees: Vec<String>,
    /// Every test that reaches the function (by test id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reached_by: Vec<String>,
    /// A test function, or inside `#[cfg(test)]`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
}

fn is_fn(k: &ItemKind) -> bool {
    *k == ItemKind::Fn
}

/// `[module."<file>.rs"]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleIndex {
    /// The module path, `crate::steam`.
    pub path: String,
    #[serde(default, rename = "function", skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<FunctionIndex>,
}

/// Which suite produced the evidence (#739 decision 12: only the full suite
/// counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Suite {
    /// `cargo test --workspace --lib --tests --release`, long tests included.
    Full,
    /// `cargo quick-test`: recorded, never counted.
    Quick,
}

/// `[test_run]`: the evidence `kovan-cli test` recorded (D4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub commit: String,
    /// Hash of `Cargo.lock` at the run.
    pub cargo_lock: String,
    pub suite: Suite,
    #[serde(default)]
    pub passed: Vec<String>,
    #[serde(default)]
    pub failed: Vec<String>,
    /// Tests edited in the change under test: never counted as evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edited: Vec<String>,
}

/// `[[review]]`: the cached list of `review.md`'s reviews, so a deleted
/// `review.md` is noticed (kovan then asks before restoring it from git).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CachedReview {
    pub function: String,
    pub by: String,
    pub artifact: String,
}

/// `[[deleted_folder]]`: a deleted folder's review history, kept in the
/// crate root folder's `kovan.toml` (#739 D6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedFolder {
    pub dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_commit: Option<String>,
    #[serde(default, rename = "function", skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<DeletedFunction>,
}

/// One folder's `kovan.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderIndex {
    pub schema_version: u32,
    pub kind: String,
    #[serde(rename = "crate")]
    pub krate: String,
    /// The folder, workspace-relative, `/`-separated.
    pub dir: String,
    /// Whether this is the crate's root folder (the one holding `lib.rs` or
    /// `main.rs`); only that one keeps `deleted_folder` history.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crate_root: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, rename = "module", skip_serializing_if = "BTreeMap::is_empty")]
    pub modules: BTreeMap<String, ModuleIndex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_run: Option<TestRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<UpstreamTable>,
    #[serde(default, rename = "review", skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<CachedReview>,
    #[serde(default, rename = "deleted_folder", skip_serializing_if = "Vec::is_empty")]
    pub deleted_folders: Vec<DeletedFolder>,
}

/// Why a `kovan.toml` could not be read as a code-review folder index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    /// Not TOML, or not this schema (the caller regenerates).
    Toml(String),
    /// A `kovan.toml` of another kind (a literature entity): not ours.
    NotACodeFolder { kind: String },
    /// Written by a newer kovan than this one.
    NewerSchema(u32),
    /// A hash or commit field is malformed.
    Field(FieldError),
    /// Two functions share an id.
    DuplicateId(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "kovan.toml: {e}"),
            Self::NotACodeFolder { kind } => {
                write!(f, "kovan.toml has kind {kind:?}, not {CODE_FOLDER_KIND:?}")
            }
            Self::NewerSchema(v) => write!(
                f,
                "kovan.toml schema_version {v} is newer than {INDEX_SCHEMA_VERSION}"
            ),
            Self::Field(e) => write!(f, "kovan.toml: {e}"),
            Self::DuplicateId(id) => write!(f, "kovan.toml: function id {id} appears twice"),
        }
    }
}

impl std::error::Error for IndexError {}

impl FolderIndex {
    /// An empty index of `dir` in `krate`.
    pub fn new(krate: impl Into<String>, dir: impl Into<String>) -> FolderIndex {
        FolderIndex {
            schema_version: INDEX_SCHEMA_VERSION,
            kind: CODE_FOLDER_KIND.to_string(),
            krate: krate.into(),
            dir: dir.into(),
            crate_root: false,
            commit: None,
            modules: BTreeMap::new(),
            test_run: None,
            upstream: None,
            reviews: Vec::new(),
            deleted_folders: Vec::new(),
        }
    }

    /// Read a `kovan.toml`, checking that it is a code-folder index and that
    /// its hashes and ids are well formed.
    pub fn parse(text: &str) -> Result<FolderIndex, IndexError> {
        let v: toml::Value = toml::from_str(text).map_err(|e| IndexError::Toml(e.to_string()))?;
        let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind != CODE_FOLDER_KIND {
            return Err(IndexError::NotACodeFolder {
                kind: kind.to_string(),
            });
        }
        let idx: FolderIndex = toml::from_str(text).map_err(|e| IndexError::Toml(e.to_string()))?;
        if idx.schema_version > INDEX_SCHEMA_VERSION {
            return Err(IndexError::NewerSchema(idx.schema_version));
        }
        let mut seen = std::collections::BTreeSet::new();
        for f in idx.functions() {
            check_hash("hash", &f.1.hash).map_err(IndexError::Field)?;
            check_hash("doc_hash", &f.1.doc_hash).map_err(IndexError::Field)?;
            if !seen.insert(f.1.id.clone()) {
                return Err(IndexError::DuplicateId(f.1.id.clone()));
            }
        }
        if let Some(r) = &idx.test_run {
            check_commit("test_run.commit", &r.commit).map_err(IndexError::Field)?;
            check_hash("test_run.cargo_lock", &r.cargo_lock).map_err(IndexError::Field)?;
        }
        Ok(idx)
    }

    /// Every function with its file name, in file then line order.
    pub fn functions(&self) -> impl Iterator<Item = (&str, &FunctionIndex)> {
        self.modules
            .iter()
            .flat_map(|(file, m)| m.functions.iter().map(move |f| (file.as_str(), f)))
    }

    /// The workspace-relative path of `file` in this folder.
    pub fn file_path(&self, file: &str) -> String {
        if self.dir.is_empty() {
            file.to_string()
        } else {
            format!("{}/{file}", self.dir)
        }
    }

    /// Sort everything this module owns (module doc, Determinism).
    pub fn normalise(&mut self) {
        for m in self.modules.values_mut() {
            m.functions
                .sort_by(|a, b| (a.lines[0], &a.id).cmp(&(b.lines[0], &b.id)));
            for f in &mut m.functions {
                f.callees.sort();
                f.callees.dedup();
                f.reached_by.sort();
                f.reached_by.dedup();
            }
        }
        if let Some(r) = &mut self.test_run {
            for v in [&mut r.passed, &mut r.failed, &mut r.edited] {
                v.sort();
                v.dedup();
            }
        }
        self.reviews.sort();
        self.reviews.dedup();
        self.deleted_folders.sort_by(|a, b| a.dir.cmp(&b.dir));
        for d in &mut self.deleted_folders {
            d.functions.sort();
        }
    }

    /// The `kovan.toml` text, normalised first, so the same data gives the
    /// same bytes.
    pub fn to_toml(&self) -> Result<String, IndexError> {
        let mut c = self.clone();
        c.normalise();
        toml::to_string_pretty(&c).map_err(|e| IndexError::Toml(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(c: char) -> String {
        format!("sha256:{}", c.to_string().repeat(64))
    }

    pub(crate) fn sample() -> FolderIndex {
        let mut idx = FolderIndex::new("tampines", "crates/tampines/src");
        idx.crate_root = true;
        idx.commit = Some("0123456789abcdef0123456789abcdef01234567".into());
        idx.modules.insert(
            "steam.rs".into(),
            ModuleIndex {
                path: "crate::steam".into(),
                functions: vec![FunctionIndex {
                    id: "crates/tampines/src/steam.rs::flash".into(),
                    name: "flash".into(),
                    qual: "flash".into(),
                    item: ItemKind::Fn,
                    lines: [10, 20],
                    hash: h('a'),
                    doc_hash: h('b'),
                    callees: vec!["crates/tampines/src/steam.rs::sat".into()],
                    reached_by: vec!["crates/tampines/tests/t.rs::t".into()],
                    test: false,
                }],
            },
        );
        idx.test_run = Some(TestRun {
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            cargo_lock: h('c'),
            suite: Suite::Full,
            passed: vec!["crates/tampines/tests/t.rs::t".into()],
            failed: vec![],
            edited: vec![],
        });
        idx
    }

    /// Methodology: an index with every section round-trips through
    /// [`FolderIndex::to_toml`] and [`FolderIndex::parse`] byte for byte;
    /// a literature `kovan.toml` (kind `paper`) is refused as not ours; a
    /// newer schema, a bad hash and a duplicate id are typed errors.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn index_round_trips_and_refuses_what_is_not_ours() {
        let idx = sample();
        let text = idx.to_toml().unwrap();
        let back = FolderIndex::parse(&text).unwrap();
        assert_eq!(back, idx);
        assert_eq!(back.to_toml().unwrap(), text);
        assert!(text.contains("[module.\"steam.rs\"]"));
        let lit = "schema_version = 1\nid = \"smith2020\"\nkind = \"paper\"\n";
        assert_eq!(
            FolderIndex::parse(lit),
            Err(IndexError::NotACodeFolder { kind: "paper".into() })
        );
        let newer = text.replace("schema_version = 1", "schema_version = 99");
        assert_eq!(FolderIndex::parse(&newer), Err(IndexError::NewerSchema(99)));
        let bad = text.replacen(&h('a'), "sha256:xyz", 1);
        assert!(matches!(FolderIndex::parse(&bad), Err(IndexError::Field(_))));
        let mut dup = idx.clone();
        let f = dup.modules["steam.rs"].functions[0].clone();
        dup.modules.get_mut("steam.rs").unwrap().functions.push(f);
        let dup_text = toml::to_string_pretty(&dup).unwrap();
        assert!(matches!(FolderIndex::parse(&dup_text), Err(IndexError::DuplicateId(_))));
    }

    /// Methodology: additive schema (decision 11). A minimal `kovan.toml`
    /// written with only the required keys (as a first-version writer
    /// would), and one carrying a key this version does not know, both read.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn older_and_newer_additive_files_load() {
        let minimal = "schema_version = 1\nkind = \"code_folder\"\ncrate = \"x\"\ndir = \"crates/x/src\"\n";
        let idx = FolderIndex::parse(minimal).unwrap();
        assert!(idx.modules.is_empty() && idx.test_run.is_none());
        let extra = format!("{minimal}future_field = 3\n[module.\"a.rs\"]\npath = \"crate::a\"\nnew_key = true\n");
        assert!(FolderIndex::parse(&extra).is_ok());
    }
}
