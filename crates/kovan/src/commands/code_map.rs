//! `kovan-cli code-map`: the workspace's code map as data (GitHub #734).
//!
//! Runs `cargo metadata --format-version 1 --no-deps` in the workspace and
//! writes the [`crate::code_map::CodeMap`] as JSON (`--format json`, the
//! default) or the drawn map as a standalone SVG (`--format svg`). Both are
//! deterministic: the same `Cargo.toml`s give byte-identical files. The
//! static site (`scripts/build-pages.sh`) writes both at build time; the
//! desktop GUI's Code Map view reads the JSON or runs the same metadata
//! itself. No rust-analyzer is involved.
//!
//! Exits with an error when a tag is malformed. A placement rule broken
//! (a crate below a dependency's row, …) is printed as a warning and the
//! map is still written: the test `tests/code_map_tags.rs` is the gate.

use std::path::{Path, PathBuf};

use crate::code_map::{self, layout, svg, CodeMap};

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// The `CodeMap` (crates, tags, required edges) as pretty JSON.
    Json,
    /// The drawn map, a standalone SVG.
    Svg,
}

/// Build the map of `root` and render it in `format`.
pub fn render(root: &Path, format: Format) -> Result<String, String> {
    let json = code_map::run_cargo_metadata(root, false)?;
    let map = CodeMap::from_cargo_metadata(&json).map_err(|e| e.join("\n"))?;
    for p in map.placement_problems() {
        eprintln!("code-map: warning: {p}");
    }
    Ok(match format {
        Format::Json => {
            let mut s = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
            s.push('\n');
            s
        }
        Format::Svg => svg::render(&map, &layout::layout(&map)),
    })
}

/// Run `kovan-cli code-map`: write to `out`, or stdout when `None`.
pub fn run(root: &Path, format: Format, out: Option<PathBuf>) -> Result<(), String> {
    let text = render(root, format)?;
    match out {
        Some(path) => {
            std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
            eprintln!("code-map: wrote {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}
