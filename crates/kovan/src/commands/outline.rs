//! `kovan-cli outline <file>` — a declarations-only skeleton of one file, so
//! an agent can decide whether it needs the whole thing before reading it
//! (GitHub issue #32's token-savings ask).
//!
//! Reuses [`kovan_semantics::LanguageAdapter::rough_definition_pattern`] and
//! [`kovan_discovery::search_file`] directly — the same ripgrep-first
//! extractor `kovan-cli symbols`/`summary` already run repository-wide, just
//! applied to one file instead of walking a tree. No new dependency: this is
//! the ripgrep tier only, matching what [`kovan_semantics`] actually has
//! today; a richer, rust-analyzer-backed outline (mirroring kopitiam's own
//! `outline`, kopitiam-semantic-backed) is a deferred follow-on (see
//! `op-l3uz`), not this command's job.

use std::path::PathBuf;

use kovan_discovery::search_file;
use kovan_semantics::LanguageAdapter;

/// Print `<line>: <declaration line>` for every rough-definition match in
/// `path`, in file order.
///
/// # Errors
///
/// A message if the file cannot be searched (missing, a directory, not valid
/// UTF-8, or an internal regex failure — see [`kovan_discovery::search_file`]).
pub fn run(path: PathBuf, lang: LanguageAdapter) -> Result<(), String> {
    let pattern = lang.rough_definition_pattern();
    let matches = search_file(&path, pattern).map_err(|e| e.to_string())?;
    if matches.is_empty() {
        println!("{}: no declarations found", path.display());
        return Ok(());
    }
    for m in matches {
        println!("{}: {}", m.line, m.text.trim());
    }
    Ok(())
}
