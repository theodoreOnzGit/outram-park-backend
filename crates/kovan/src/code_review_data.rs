//! The **Code Review view's data folder** (GitHub #820): what desktop kovan
//! builds, from a workspace's saved SCIP index, so the embedded Code Review
//! UI (`kovan-web`'s `CodeReview`, `app::code_review_view`) opens with no
//! file for the user to find.
//!
//! The folder has the layout `kovan-web` reads on the published site
//! (`kovan_web::data`, written there by `crates/kovan-web/web/data.sh`):
//!
//! ```text
//! <workspace>/target/kovan-code-review/data/
//!   code_map.json      the crates and their tags (cargo metadata)
//!   graph/index.json   the split call graph's index, with stamp states
//!   graph/search.json  its search index
//!   graph/<crate>.json one slice per crate
//!   build.json         commit, and which crates rust-analyzer did not index
//! ```
//!
//! It is a **disposable cache** under `target/`, never committed, and built
//! by the same functions `kovan-cli code-map` and `kovan-cli call-graph
//! --scip <file> --split-dir <dir>` call; nothing here is a second call-graph
//! builder. **rust-analyzer is not run**: the input is the index "Index
//! fresh" left at [`SCIP_FILE`].
//!
//! `build.json` is written last and is the freshness marker
//! ([`data_state`]): data older than the SCIP index is rebuilt.

use std::path::{Path, PathBuf};

use crate::code_map::CodeMap;
use crate::commands::index_control::RunControl;
use crate::commands::{call_graph, index};

/// The data folder, workspace-relative.
pub const DATA_DIR: &str = "target/kovan-code-review/data";

/// The SCIP index an index run leaves, workspace-relative.
pub const SCIP_FILE: &str = "target/kovan-scip/index.scip";

/// `<workspace>/`[`DATA_DIR`].
pub fn data_dir(workspace: &Path) -> PathBuf {
    workspace.join(DATA_DIR)
}

/// `<workspace>/`[`SCIP_FILE`].
pub fn scip_path(workspace: &Path) -> PathBuf {
    workspace.join(SCIP_FILE)
}

/// Whether the data folder can be used as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataState {
    /// No SCIP index: the workspace has not been indexed on this machine.
    NoScip,
    /// A SCIP index and no data yet.
    Missing,
    /// The SCIP index is newer than the data.
    Stale,
    /// The data was built from the SCIP index that is there.
    Current,
}

/// Compare the data's `build.json` with the SCIP index, by modification time.
pub fn data_state(workspace: &Path) -> DataState {
    let modified = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let Some(scip) = modified(scip_path(workspace)) else {
        return DataState::NoScip;
    };
    match modified(data_dir(workspace).join("build.json")) {
        None => DataState::Missing,
        Some(built) if built < scip => DataState::Stale,
        Some(_) => DataState::Current,
    }
}

/// What a build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildReport {
    pub dir: PathBuf,
    /// Workspace crates in the code map.
    pub crates: usize,
    /// Crates with source files that the SCIP index holds no document of
    /// ([`index::crates_not_in_scip`]): shown by the UI as having no call
    /// graph, never silently.
    pub not_in_scip: Vec<String>,
}

impl BuildReport {
    /// One line for the view.
    pub fn describe(&self) -> String {
        if self.not_in_scip.is_empty() {
            format!("{} crate(s); call graph for all of them", self.crates)
        } else {
            format!(
                "{} crate(s); rust-analyzer did not index {}, so they have no call graph: {}",
                self.crates,
                self.not_in_scip.len(),
                self.not_in_scip.join(", ")
            )
        }
    }
}

/// Build the data folder of `workspace` from its saved SCIP index (module
/// doc). Slow for a large workspace (reading the index and resolving every
/// call), so the app runs it on a worker thread and shows `ctl`'s progress.
pub fn build(workspace: &Path, ctl: &RunControl) -> Result<BuildReport, String> {
    let scip = scip_path(workspace);
    if !scip.is_file() {
        return Err(format!(
            "{} has no SCIP index yet ({SCIP_FILE}): run \"Index fresh\" in the Code Map view first",
            workspace.display()
        ));
    }
    let dir = data_dir(workspace);
    let graph = dir.join("graph");
    std::fs::create_dir_all(&graph).map_err(|e| format!("creating {}: {e}", graph.display()))?;
    // A build that stops half way must not look current.
    let marker = dir.join("build.json");
    if marker.exists() {
        std::fs::remove_file(&marker).map_err(|e| format!("{}: {e}", marker.display()))?;
    }

    ctl.phase("code review data: reading the crates (cargo metadata)", 0);
    let json = crate::code_map::run_cargo_metadata(workspace, false)?;
    let (map, _untagged) =
        CodeMap::from_cargo_metadata_allowing_untagged(&json).map_err(|e| e.join("\n"))?;
    let mut text = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    text.push('\n');
    write(&dir.join("code_map.json"), &text)?;
    ctl.check()?;

    ctl.phase("code review data: reading the SCIP index", 0);
    let ix = call_graph::read_scip(&scip)?;
    let rust_analyzer = format!("{} {}", ix.tool_name, ix.tool_version)
        .trim()
        .to_string();
    let members = call_graph::member_dirs(workspace)?;
    let (_, not_in_scip) = index::crates_not_in_scip(workspace, &members, &members, &ix);
    ctl.check()?;

    ctl.phase("code review data: building the call graph", 0);
    let doc = call_graph::build_from_scip(workspace, None, ix)?;
    ctl.check()?;
    ctl.phase("code review data: writing", 0);
    call_graph::write_split(workspace, &doc, &graph)?;

    let build = serde_json::json!({
        "commit": crate::index_fresh::root_file::head_or_none(workspace),
        "call_graph": {
            "backend": "scip",
            "rust_analyzer": rust_analyzer,
            "stale": [],
            "missing": not_in_scip,
        },
    });
    let mut text = serde_json::to_string_pretty(&build).map_err(|e| e.to_string())?;
    text.push('\n');
    write(&marker, &text)?;
    ctl.phase("code review data: done", 0);
    Ok(BuildReport {
        dir,
        crates: map.crates.len(),
        not_in_scip,
    })
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::call_graph::split::SplitIndex;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// Methodology: a throwaway single crate (`f` in `src/lib.rs` calls `g`
    /// in `src/a.rs`) and a hand-encoded SCIP index of it, with document
    /// paths written the Windows way. Without the index the state is
    /// `NoScip` and a build is refused; with it the state is `Missing`, the
    /// build writes every file of the layout, each parses as the type
    /// `kovan-web` reads it into, the call from `f` to `g` is in the crate's
    /// slice, and the state becomes `Current`. A crate the index does not
    /// cover is named in `build.json`. No rust-analyzer is run.
    ///
    /// Result: ~~(2026-10-10, Windows 11): see the run recorded in the
    /// commit that added this test~~ **CORRECTED 2026-10-10**: that commit
    /// (cf9346ab6d) was never compiled, so it records no run. First run:
    /// passes on Linux (Arch, release, 2026-10-10, branch `stamping-gui`).
    /// Not re-checked on Windows.
    #[test]
    fn the_data_folder_is_built_from_a_saved_index_and_knows_when_it_is_current() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        put(
            p,
            "Cargo.toml",
            "[package]\nname = \"solo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        put(
            p,
            "src/lib.rs",
            "pub mod a;\n/// Doubles.\npub fn f(x: u32) -> u32 {\n    a::g(x) * 2\n}\n",
        );
        put(p, "src/a.rs", "pub fn g(x: u32) -> u32 {\n    x + 1\n}\n");
        let ctl = RunControl::default();
        assert_eq!(data_state(p), DataState::NoScip);
        assert!(build(p, &ctl).unwrap_err().contains("Index fresh"));

        let f = "rust-analyzer cargo solo 0.1.0 f().";
        let g = "rust-analyzer cargo solo 0.1.0 a/g().";
        let lib = [(&[2u32, 7, 8][..], f, 1u64), (&[3, 7, 8], g, 0)];
        let a = [(&[0u32, 7, 8][..], g, 1u64)];
        let bytes =
            crate::scip::encode::index("1.97.1", &[("src\\lib.rs", &lib), ("src\\a.rs", &a)]);
        std::fs::create_dir_all(scip_path(p).parent().unwrap()).unwrap();
        std::fs::write(scip_path(p), bytes).unwrap();
        assert_eq!(data_state(p), DataState::Missing);

        let report = build(p, &ctl).unwrap();
        assert_eq!(report.crates, 1);
        assert!(report.not_in_scip.is_empty(), "{report:?}");
        assert!(report.describe().contains("all of them"));
        assert_eq!(data_state(p), DataState::Current);
        let dir = data_dir(p);
        let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap();
        let map: CodeMap = serde_json::from_str(&read("code_map.json")).unwrap();
        assert_eq!(map.crates.len(), 1);
        let index: SplitIndex = serde_json::from_str(&read("graph/index.json")).unwrap();
        assert_eq!(index.crates.len(), 1);
        assert_eq!(index.crates[0].name, "solo");
        let slice = read(&format!("graph/{}", index.crates[0].file));
        assert!(
            slice.contains("src/a.rs::g"),
            "the call to g is in the slice:\n{slice}"
        );
        let build_json: serde_json::Value = serde_json::from_str(&read("build.json")).unwrap();
        assert_eq!(build_json["call_graph"]["backend"], "scip");
        assert_eq!(
            build_json["call_graph"]["rust_analyzer"],
            "rust-analyzer 1.97.1"
        );
        assert_eq!(
            build_json["call_graph"]["missing"]
                .as_array()
                .unwrap()
                .len(),
            0
        );

        // An index that covers nothing of the crate: named, not silent.
        let other = [(
            &[0u32, 7, 8][..],
            "rust-analyzer cargo other 0.1.0 h().",
            1u64,
        )];
        let bytes = crate::scip::encode::index("1.97.1", &[("crates\\other\\src\\lib.rs", &other)]);
        std::fs::write(scip_path(p), bytes).unwrap();
        let report = build(p, &ctl).unwrap();
        assert_eq!(report.not_in_scip, vec!["solo".to_string()]);
        assert!(report.describe().contains("did not index 1"));
        let build_json: serde_json::Value = serde_json::from_str(&read("build.json")).unwrap();
        assert_eq!(build_json["call_graph"]["missing"][0], "solo");
    }
}
