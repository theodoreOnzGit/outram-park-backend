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
//! Every query here is **name-based**: [`locate_declaration`] finds `symbol`'s
//! declaration line in `--file` by text (not `document_symbols` — see its own
//! doc comment for why), and that identifier's position is the query anchor.
//! [`extract_signature`]/[`looks_like_signature`] are close ports of
//! kopitiam's own `semq.rs` functions of the same name.
//!
//! **Keeping rust-analyzer warm across invocations** (op-fdph): each of
//! `run_def`/`run_sig`/`run_refs` tries [`super::lsp_daemon::query`] first —
//! a background daemon holding one long-lived, already-indexed session — and
//! only falls back to [`connect`]'s spawn-index-shutdown-per-call path if no
//! daemon is reachable (including on non-Unix targets, where the daemon
//! doesn't exist at all). See `commands::lsp_daemon`'s module doc for the
//! daemon design.
//!
//! **Deferred, not implemented here** (see `op-l3uz`): `callers`/`callees`
//! (call-hierarchy composition over `references` + `document_symbols`) and
//! `impls` (trait `impl`-site filtering). Both are real, more involved
//! features on top of the same session — this module ships the three
//! highest-value, simplest-to-verify queries first.

use std::path::{Path, PathBuf};
use std::time::Duration;

use kopitiam_semantic::{ProgressKind, RustAnalyzerSession};

use super::lsp_daemon;

/// Default rust-analyzer indexing timeout — matches kopitiam's own default
/// (`DEFAULT_RA_TIMEOUT_SECS` in its `apps/cli/src/syntactic.rs`); a large
/// workspace's first index can genuinely take a couple of minutes. Also used
/// by the daemon (`commands::lsp_daemon`) as its ready-wait timeout.
pub(super) const DEFAULT_RA_TIMEOUT_SECS: u64 = 180;

/// Reads `KOVAN_RA_TIMEOUT_SECS` (a positive integer number of seconds),
/// falling back to [`DEFAULT_RA_TIMEOUT_SECS`].
pub(super) fn ra_timeout() -> Duration {
    let secs = std::env::var("KOVAN_RA_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(DEFAULT_RA_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Spawn rust-analyzer for `root`, announcing indexing progress on stderr so
/// a `--json` stdout stays clean and a slow index doesn't look like a hang.
/// The fallback path when no daemon (`commands::lsp_daemon`) is reachable.
fn connect(root: &Path) -> Result<RustAnalyzerSession, String> {
    let root = std::fs::canonicalize(root)
        .map_err(|e| format!("resolving workspace root {}: {e}", root.display()))?;
    let timeout = ra_timeout();
    eprintln!(
        "kovan-cli: no warm lsp-daemon found — starting rust-analyzer and waiting for it to \
         index {} (timeout {}s; set KOVAN_RA_TIMEOUT_SECS to change)...",
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

/// The identifier position of a resolved symbol (0-based, matching every
/// `kopitiam_semantic` query).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct SymPos {
    pub(super) line: u32,
    pub(super) character: u32,
}

/// Finds `name`'s declaration in `file` by scanning its text for a language
/// keyword (`fn`/`struct`/`enum`/`trait`/`const`/`static`/`type`/`mod`)
/// immediately followed by `name` at a word boundary, and returns that
/// identifier's position.
///
/// **Why a text scan, not `document_symbols`**: two independent reasons.
/// (1) [`kopitiam_semantic::RustAnalyzerSession::document_symbols`] has no
/// counterpart on [`kopitiam_semantic::AsyncRustAnalyzerSession`] — the
/// keep-warm daemon (`commands::lsp_daemon`) can't call it at all. (2) Even
/// on the synchronous session, this workspace's rust-analyzer answers
/// `textDocument/documentSymbol` with the legacy flat `SymbolInformation`
/// shape (confirmed by hand, 2026-08-22 — no `selectionRange` field), whose
/// `location.range` starts at the item's leading doc comment/attributes, not
/// its identifier — hovering there returns nothing. A plain text scan sidesteps
/// both problems at once and needs no LSP round-trip to resolve a position,
/// which is also why it's used unconditionally rather than as an LSP-first,
/// text-scan-as-repair combination the way an earlier version of this module
/// did.
///
/// # Errors
///
/// A message if `file` cannot be read, or if no line in it declares `name`.
pub(super) fn locate_declaration(file: &Path, name: &str) -> Result<SymPos, String> {
    const KEYWORDS: [&str; 8] = [
        "fn ", "struct ", "enum ", "trait ", "const ", "static ", "type ", "mod ",
    ];
    let target = name.rsplit("::").next().unwrap_or(name);
    let text = std::fs::read_to_string(file)
        .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    for (line_no, line) in text.lines().enumerate() {
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
                return Ok(SymPos {
                    line: line_no as u32,
                    character,
                });
            }
        }
    }
    Err(format!(
        "no declaration of `{name}` found in {} (looked for fn/struct/enum/trait/const/static/type/mod {target})",
        file.display()
    ))
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
pub(super) fn extract_signature(hover: &str) -> String {
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
    if let Some(resp) = lsp_daemon::query(
        &root,
        &lsp_daemon::Request::Def {
            file: file.clone(),
            symbol: symbol.clone(),
        },
    ) {
        return print_daemon_response(resp);
    }
    let pos = locate_declaration(&file, &symbol)?;
    let mut session = connect(&root)?;
    let hover = session.hover(&file, pos.line, pos.character).ok().flatten();
    let _ = session.shutdown();

    if let Some(h) = &hover {
        println!("{}", extract_signature(&h.contents));
    }
    println!(
        "defined at {}:{}:{}",
        file.display(),
        pos.line,
        pos.character
    );
    Ok(())
}

/// `kovan-cli sig <symbol> --file <file>` — the signature alone.
pub fn run_sig(symbol: String, file: PathBuf, root: PathBuf) -> Result<(), String> {
    if let Some(resp) = lsp_daemon::query(
        &root,
        &lsp_daemon::Request::Sig {
            file: file.clone(),
            symbol: symbol.clone(),
        },
    ) {
        return print_daemon_response(resp);
    }
    let pos = locate_declaration(&file, &symbol)?;
    let mut session = connect(&root)?;
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
    if let Some(resp) = lsp_daemon::query(
        &root,
        &lsp_daemon::Request::Refs {
            file: file.clone(),
            symbol: symbol.clone(),
        },
    ) {
        return print_daemon_response(resp);
    }
    let pos = locate_declaration(&file, &symbol)?;
    let mut session = connect(&root)?;
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

/// Prints a [`lsp_daemon::Response`] the same way the fallback path prints
/// its own result, and turns a daemon-reported failure into this module's
/// `Result<(), String>` error convention.
fn print_daemon_response(resp: lsp_daemon::Response) -> Result<(), String> {
    if !resp.ok {
        return Err(resp
            .error
            .unwrap_or_else(|| "daemon request failed".to_string()));
    }
    if let Some(sig) = &resp.signature {
        println!("{sig}");
    }
    if let Some(def) = &resp.definition {
        println!("defined at {}:{}:{}", def.file, def.line, def.character);
    }
    if let Some(refs) = &resp.refs {
        if refs.is_empty() {
            println!("no references found");
        }
        for r in refs {
            println!("{}:{}:{}", r.file, r.line, r.character);
        }
    }
    if resp.signature.is_none() && resp.definition.is_none() && resp.refs.is_none() {
        println!("no signature found");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a small synthetic Rust source (a top-level `fn` preceded
    /// by a doc comment that also mentions the function's own name as a
    /// distractor — the exact shape that broke the earlier
    /// `document_symbols`-based resolver, see the module doc comment).
    /// Resolved both by bare name and by a `::`-qualified form.
    ///
    /// Result (2026-08-22): both forms land on the `fn` line, not the doc
    /// comment.
    #[test]
    fn locate_declaration_finds_the_fn_line_not_a_doc_comment_mention() {
        let dir = std::env::temp_dir();
        let file = dir.join(format!("kovan_semq_test_{}.rs", std::process::id()));
        std::fs::write(
            &file,
            "/// See `bar` below -- a distractor mention of the same name.\nfn bar(x: u32) -> u32 {\n    x\n}\n",
        )
        .unwrap();

        let by_bare = locate_declaration(&file, "bar").expect("bare name should resolve");
        assert_eq!(
            by_bare,
            SymPos {
                line: 1,
                character: 3
            }
        );

        let by_qualified =
            locate_declaration(&file, "crate::bar").expect("qualified name should resolve");
        assert_eq!(by_qualified, by_bare);

        assert!(locate_declaration(&file, "nonexistent").is_err());
        std::fs::remove_file(&file).ok();
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
