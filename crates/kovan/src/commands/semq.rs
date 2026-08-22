//! `kovan-cli def`/`refs`/`sig` — rust-analyzer-backed semantic queries
//! (GitHub issue #32's follow-up: "wire in rust analyzer capability from
//! kopitiam into kovan cli").
//!
//! Wired directly to [`kopitiam_semantic::RustAnalyzerSession`], which spawns
//! the external `rust-analyzer` binary and talks LSP over stdio — this
//! dependency is lightweight at compile time (no `ra_ap_*` crate linked) but
//! needs `rust-analyzer` on `PATH` at runtime (`rustup component add
//! rust-analyzer`) to answer anything.
//!
//! Every query here is **name-based**: a symbol's `document_symbol` tree in
//! `--file` is searched for a declaration matching `symbol`
//! ([`find_symbol`]), and that identifier's position is the query anchor —
//! the same design kopitiam's own `semq.rs` uses, and largely ported from it
//! (the pure helpers below — [`find_symbol`], [`sym_pos`],
//! [`extract_signature`] — are close ports, cited per-function; the session
//! plumbing is new).
//!
//! **Deferred, not implemented here** (see `op-l3uz`): `callers`/`callees`
//! (call-hierarchy composition over `references` + `document_symbols`) and
//! `impls` (trait `impl`-site filtering). Both are real, more involved
//! features on top of the same session — this module ships the three
//! highest-value, simplest-to-verify queries first.

use std::path::{Path, PathBuf};
use std::time::Duration;

use kopitiam_semantic::{ProgressKind, RustAnalyzerSession};
use serde_json::Value;

/// Default rust-analyzer indexing timeout — matches kopitiam's own default
/// (`DEFAULT_RA_TIMEOUT_SECS` in its `apps/cli/src/syntactic.rs`); a large
/// workspace's first index can genuinely take a couple of minutes.
const DEFAULT_RA_TIMEOUT_SECS: u64 = 180;

/// Reads `KOVAN_RA_TIMEOUT_SECS` (a positive integer number of seconds),
/// falling back to [`DEFAULT_RA_TIMEOUT_SECS`].
fn ra_timeout() -> Duration {
    let secs = std::env::var("KOVAN_RA_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(DEFAULT_RA_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Spawn rust-analyzer for `root`, announcing indexing progress on stderr so
/// a `--json` stdout stays clean and a slow index doesn't look like a hang.
fn connect(root: &Path) -> Result<RustAnalyzerSession, String> {
    let root = std::fs::canonicalize(root)
        .map_err(|e| format!("resolving workspace root {}: {e}", root.display()))?;
    let timeout = ra_timeout();
    eprintln!(
        "kovan-cli: starting rust-analyzer and waiting for it to index {} \
         (timeout {}s; set KOVAN_RA_TIMEOUT_SECS to change)...",
        root.display(),
        timeout.as_secs(),
    );
    let mut announced = false;
    RustAnalyzerSession::connect_with_observed("rust-analyzer", &[], &root, timeout, |update| {
        if !announced && matches!(update.kind, ProgressKind::Report) {
            eprintln!("kovan-cli: rust-analyzer still indexing… (large workspace)");
            announced = true;
        }
    })
    .map_err(|e| {
        format!(
            "cannot start rust-analyzer: {e} — is it installed \
             (`rustup component add rust-analyzer`) and on PATH?"
        )
    })
}

/// The identifier position of a resolved symbol — line/character only
/// (0-based, matching every `kopitiam_semantic` query). Kopitiam's own
/// `SymPos` also carries `kind`/`range_start_line`/`range_end_line` for its
/// `callers`/`callees`/`impls` composition; this module doesn't implement
/// those (see the module doc comment), so it carries only what `def`/`refs`/
/// `sig` actually use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SymPos {
    line: u32,
    character: u32,
}

/// Builds a [`SymPos`] from one `DocumentSymbol` JSON object — `selectionRange`
/// gives the identifier position (falling back to `range`, then a flat
/// `SymbolInformation`'s `location.range`, for servers that speak the legacy
/// shape). Ported from kopitiam's `semq.rs::sym_pos`, minus the fields this
/// module doesn't need.
fn sym_pos(symbol: &Value) -> SymPos {
    let sel = symbol
        .pointer("/selectionRange/start")
        .or_else(|| symbol.pointer("/range/start"))
        .or_else(|| symbol.pointer("/location/range/start"));
    let line = sel
        .and_then(|s| s.get("line"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let character = sel
        .and_then(|s| s.get("character"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    SymPos { line, character }
}

/// Depth-first search of a `document_symbols` tree for a declaration named
/// `name` — matched either exactly or by its last `::`-separated segment (so
/// `outram_park::foo` and plain `foo` both find the same declaration).
/// Ported from kopitiam's `semq.rs::find_symbol`.
fn find_symbol(symbols: &[Value], name: &str) -> Option<SymPos> {
    let target = name.rsplit("::").next().unwrap_or(name);
    for symbol in symbols {
        let this = symbol
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if this == name || this == target {
            return Some(sym_pos(symbol));
        }
        if let Some(children) = symbol.get("children").and_then(Value::as_array) {
            if let Some(found) = find_symbol(children, name) {
                return Some(found);
            }
        }
    }
    None
}

/// Resolves `name` to its identifier position in `file` via `document_symbols`.
fn resolve(session: &mut RustAnalyzerSession, file: &Path, name: &str) -> Result<SymPos, String> {
    let symbols = session
        .document_symbols(file)
        .map_err(|e| e.to_string())?;
    let pos = find_symbol(&symbols, name).ok_or_else(|| {
        format!(
            "no symbol named `{name}` found in {} (its documentSymbol tree lists no matching declaration)",
            file.display()
        )
    })?;
    Ok(refine_position(file, pos, name).unwrap_or(pos))
}

/// Repairs [`sym_pos`]'s position against the file's actual text.
///
/// **Why this is needed**: this rust-analyzer build answers
/// `textDocument/documentSymbol` with the legacy flat `SymbolInformation`
/// shape (confirmed by hand, 2026-08-22 — no `selectionRange` field at all),
/// whose `location.range` is the item's *whole* span, starting at its
/// leading doc comment/attributes — not the identifier. Hovering or asking
/// for references at that position lands on a comment, which correctly
/// returns nothing. Rather than patching `kopitiam-semantic` (out of bounds —
/// this workspace consumes it as a dependency, not a fork; likely the same
/// client-capability-negotiation class of issue as
/// <https://github.com/theodoreOnzGit/kopitiam/issues/30>, worth reporting
/// upstream separately), this scans forward from the reported line for the
/// first line that actually declares `name` (a language keyword immediately
/// followed by the identifier, at a word boundary) and returns *that*
/// position instead. Harmless when the shape was already correct: the first
/// matching line is the declaration line itself, at line-offset 0.
fn refine_position(file: &Path, pos: SymPos, name: &str) -> Option<SymPos> {
    const KEYWORDS: [&str; 8] = [
        "fn ", "struct ", "enum ", "trait ", "const ", "static ", "type ", "mod ",
    ];
    const WINDOW: u32 = 50;
    let target = name.rsplit("::").next().unwrap_or(name);
    let text = std::fs::read_to_string(file).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let end = (pos.line as usize + WINDOW as usize).min(lines.len());
    for (offset, line) in lines[pos.line as usize..end].iter().enumerate() {
        for kw in KEYWORDS {
            let Some(kw_idx) = line.find(kw) else {
                continue;
            };
            let after = &line[kw_idx + kw.len()..];
            let leading_ws = after.len() - after.trim_start().len();
            if !after.trim_start().starts_with(target) {
                continue;
            }
            let name_start = kw_idx + kw.len() + leading_ws;
            let boundary_ok = line[name_start + target.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
            if boundary_ok {
                let character = line[..name_start].chars().count() as u32;
                return Some(SymPos {
                    line: pos.line + offset as u32,
                    character,
                });
            }
        }
    }
    None
}

/// True for a hover line that reads as a declaration rather than a bare
/// module path (`crate::module`). Ported verbatim from kopitiam's
/// `semq.rs::looks_like_signature`.
fn looks_like_signature(line: &str) -> bool {
    const KEYWORDS: [&str; 9] = [
        "fn ", "struct ", "enum ", "trait ", "const ", "static ", "type ", "impl ", "macro ",
    ];
    if KEYWORDS.iter().any(|k| line.contains(k)) {
        return true;
    }
    line.contains(": ") && line.contains(char::is_whitespace)
}

/// Extracts the single most signature-like line from a hover's Markdown —
/// the first fenced-code-block line that reads as a declaration
/// ([`looks_like_signature`]), or the first non-empty content line if none
/// do. Ported verbatim from kopitiam's `semq.rs::extract_signature` (its own
/// doc: hover text for a function typically has the module path as its first
/// code block and the actual signature as its second — this skips the path).
fn extract_signature(hover: &str) -> String {
    let mut first_content: Option<&str> = None;
    for raw in hover.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") || line == "---" {
            continue;
        }
        if first_content.is_none() {
            first_content = Some(line);
        }
        if looks_like_signature(line) {
            return line.to_string();
        }
    }
    first_content.unwrap_or("").to_string()
}

/// `kovan-cli def <symbol> --file <file>` — definition location plus its
/// signature (from `hover` at the same identifier — the declaration `--file`
/// matched *is* the definition site).
pub fn run_def(symbol: String, file: PathBuf, root: PathBuf) -> Result<(), String> {
    let mut session = connect(&root)?;
    let pos = resolve(&mut session, &file, &symbol)?;
    let hover = session.hover(&file, pos.line, pos.character).ok().flatten();
    let _ = session.shutdown();

    if let Some(h) = &hover {
        println!("{}", extract_signature(&h.contents));
    }
    println!("defined at {}:{}:{}", file.display(), pos.line, pos.character);
    Ok(())
}

/// `kovan-cli sig <symbol> --file <file>` — the signature alone.
pub fn run_sig(symbol: String, file: PathBuf, root: PathBuf) -> Result<(), String> {
    let mut session = connect(&root)?;
    let pos = resolve(&mut session, &file, &symbol)?;
    let hover = session.hover(&file, pos.line, pos.character).ok().flatten();
    let _ = session.shutdown();

    match hover {
        Some(h) => println!("{}", extract_signature(&h.contents)),
        None => println!("no signature for `{symbol}`"),
    }
    Ok(())
}

/// `kovan-cli refs <symbol> --file <file>` — every reference site, as
/// `file:line:character` coordinates (0-based, matching every
/// `kopitiam_semantic` query — no +1 display conversion, same convention
/// kopitiam's own `refs` uses).
pub fn run_refs(symbol: String, file: PathBuf, root: PathBuf) -> Result<(), String> {
    let mut session = connect(&root)?;
    let pos = resolve(&mut session, &file, &symbol)?;
    let locations = session
        .references(&file, pos.line, pos.character, false)
        .map_err(|e| e.to_string())?;
    let _ = session.shutdown();

    if locations.is_empty() {
        println!("no references to `{symbol}`");
        return Ok(());
    }
    let mut coords: Vec<(String, u32, u32)> = locations
        .iter()
        .map(|loc| {
            (
                loc.path.display().to_string(),
                loc.range.start.line,
                loc.range.start.character,
            )
        })
        .collect();
    coords.sort();
    for (path, line, character) in coords {
        println!("{path}:{line}:{character}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Methodology: a nested `document_symbols`-shaped tree (a `struct` with
    /// an `impl`-block-style child `fn`), searched both by its bare name and
    /// by a `::`-qualified form. Mirrors kopitiam's own coverage for this
    /// exact function.
    ///
    /// Result (2026-08-22): both forms find the nested `fn`.
    #[test]
    fn find_symbol_matches_bare_and_qualified_names_recursively() {
        let tree = vec![json!({
            "name": "Foo",
            "kind": 23,
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
            "selectionRange": {"start": {"line": 0, "character": 7}, "end": {"line": 0, "character": 10}},
            "children": [{
                "name": "bar",
                "kind": 6,
                "range": {"start": {"line": 1, "character": 4}, "end": {"line": 3, "character": 5}},
                "selectionRange": {"start": {"line": 1, "character": 11}, "end": {"line": 1, "character": 14}},
            }]
        })];

        let by_bare = find_symbol(&tree, "bar").expect("bare name should resolve");
        assert_eq!(by_bare, SymPos { line: 1, character: 11 });

        let by_qualified = find_symbol(&tree, "Foo::bar").expect("qualified name should resolve");
        assert_eq!(by_qualified, by_bare);

        assert!(find_symbol(&tree, "nonexistent").is_none());
    }

    /// Methodology: a hover string shaped like rust-analyzer's real output —
    /// a module-path code block, then the actual signature, then a `---`
    /// separator and prose — checked that the signature (not the module
    /// path) is what's extracted. Same fixture kopitiam's own test for this
    /// function uses.
    ///
    /// Result (2026-08-22): the signature line is extracted, not the path.
    #[test]
    fn extract_signature_skips_the_leading_module_path() {
        let hover = "```rust\nkopitiam_semantic::session\n```\n\n```rust\npub fn references(&mut self, file: &Path) -> Result<Vec<Location>>\n```\n\n---\n\nFinds all references.";
        let sig = extract_signature(hover);
        assert_eq!(
            sig,
            "pub fn references(&mut self, file: &Path) -> Result<Vec<Location>>"
        );
    }

    /// Methodology: a bare module path with no declaration keyword and no
    /// `name: Type` shape must NOT be classified as a signature.
    ///
    /// Result (2026-08-22): passes.
    #[test]
    fn looks_like_signature_rejects_a_bare_module_path() {
        assert!(!looks_like_signature("kopitiam_semantic::session"));
        assert!(looks_like_signature("pub fn foo() -> Bar"));
        assert!(looks_like_signature("field: SomeType"));
    }
}
