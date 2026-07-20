//! `kovan search` — regex search over a single file or a whole repository.
//!
//! Two modes, chosen by which arguments are given:
//!
//! - `--path <file> --pattern <re>` — single-file search
//!   ([`kovan_discovery::search_file`]).
//! - `--root <dir> [--kind <k>] --pattern <re>` (root defaults to `.`, kind
//!   defaults to `source`) — repository-wide search
//!   ([`kovan_discovery::search_repository`]).
//!
//! `--path` wins if both are given. Both modes print ripgrep-style
//! `path:line:column: text` lines (1-based line and column).

use std::path::PathBuf;

use super::KindArg;
use kovan_discovery::FileKind;

/// Run the search in whichever mode `path`/`root` select. See the module docs
/// for the dispatch rule.
pub fn run(
    path: Option<PathBuf>,
    root: Option<PathBuf>,
    kind: Option<KindArg>,
    pattern: &str,
) -> Result<(), String> {
    if let Some(path) = path {
        let hits = kovan_discovery::search_file(&path, pattern).map_err(|e| e.to_string())?;
        for h in hits {
            println!("{}:{}:{}: {}", path.display(), h.line, h.column, h.text);
        }
        return Ok(());
    }

    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let kind: FileKind = kind.map(FileKind::from).unwrap_or(FileKind::Source);
    let hits =
        kovan_discovery::search_repository(&root, kind, pattern).map_err(|e| e.to_string())?;
    for (file, h) in hits {
        println!("{}:{}:{}: {}", file.display(), h.line, h.column, h.text);
    }
    Ok(())
}
