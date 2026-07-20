//! Ripgrep-first symbol scanner for **Rust** source.
//!
//! Detects `fn`, `struct`, `enum`, `trait`, `type`, `mod`, and `impl`
//! definitions (see `docs/kovan.md`, "# KOVAN Semantics" → "Rust"). Deterministic
//! and offline: a pure function of the file text. Enclosing `mod` and `impl`
//! blocks are tracked by textual brace depth so members receive a `::`-qualified
//! name (`module::Type::method`). The escalation path to `rust-analyzer` lives in
//! [`crate::adapters`].
//!
//! Known limits: braces inside string/char literals, `//`/`/* */` comments, or
//! macro bodies are counted textually, so exotic formatting can mis-nest a
//! qualified name — the bare `name` is always correct. Multi-line generics are
//! read from the definition's keyword line.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{ExtractedSymbol, SymbolKind};
use crate::LanguageAdapter;

/// An open `mod`/`impl` scope, remembered until its block closes.
struct Scope {
    /// The `::` segment this scope contributes to qualified names.
    name: String,
    /// Brace depth just before this scope's opening line — the block's body
    /// lives while depth is strictly greater, and the scope pops when depth
    /// returns to this value.
    open_depth: usize,
}

struct Patterns {
    func: Regex,
    strukt: Regex,
    enumm: Regex,
    treyt: Regex,
    alias: Regex,
    modd: Regex,
    /// `impl [<..>] [Trait for] Type` — group 1 (optional) trait, group 2 type.
    implb: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let vis = r"(?:pub(?:\s*\([^)]*\))?\s+)?";
        Patterns {
            func: Regex::new(&format!(
                r"^\s*{vis}(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+(?:\x22[^\x22]*\x22\s+)?)?fn\s+([A-Za-z_]\w*)"
            ))
            .unwrap(),
            strukt: Regex::new(&format!(r"^\s*{vis}struct\s+([A-Za-z_]\w*)")).unwrap(),
            enumm: Regex::new(&format!(r"^\s*{vis}enum\s+([A-Za-z_]\w*)")).unwrap(),
            treyt: Regex::new(&format!(r"^\s*{vis}(?:unsafe\s+)?(?:auto\s+)?trait\s+([A-Za-z_]\w*)")).unwrap(),
            alias: Regex::new(&format!(r"^\s*{vis}type\s+([A-Za-z_]\w*)")).unwrap(),
            modd: Regex::new(&format!(r"^\s*{vis}mod\s+([A-Za-z_]\w*)")).unwrap(),
            implb: Regex::new(
                r"^\s*impl(?:\s*<[^>]*>)?\s+(?:([A-Za-z_][\w:]*(?:\s*<[^>]*>)?)\s+for\s+)?([A-Za-z_][\w:]*)",
            )
            .unwrap(),
        }
    })
}

/// Extract all Rust symbols from `text`, recording `file` on each.
pub fn extract(file: &Path, text: &str) -> Vec<ExtractedSymbol> {
    let p = patterns();
    let mut out = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();
    let mut depth: usize = 0;

    for (idx, raw) in text.lines().enumerate() {
        let line = strip_line_comment(raw);
        let lineno = (idx + 1) as u64;
        let base_depth = depth;
        let prefix = qualified_prefix(&stack);

        // A line opens at most one new named scope (mod/impl); remember it and
        // push *after* emitting so the definition line is not its own parent.
        let mut opened: Option<String> = None;

        if let Some(c) = p.modd.captures(line) {
            let name = c[1].to_string();
            out.push(mk(file, lineno, &prefix, &name, SymbolKind::Module));
            opened = Some(name);
        } else if let Some(c) = p.implb.captures(line) {
            let ty = c[2].to_string();
            let qual = match c.get(1) {
                Some(tr) => format!("{} for {}", tr.as_str().trim(), ty),
                None => ty.clone(),
            };
            out.push(ExtractedSymbol {
                name: ty.clone(),
                qualified_name: join(&prefix, &qual),
                kind: SymbolKind::Impl,
                language: LanguageAdapter::Rust,
                file: file.to_path_buf(),
                line: lineno,
            });
            // Members qualify under the concrete type, not "Trait for Type".
            opened = Some(ty);
        } else if let Some(c) = p.func.captures(line) {
            out.push(mk(file, lineno, &prefix, &c[1], SymbolKind::Function));
        } else if let Some(c) = p.strukt.captures(line) {
            out.push(mk(file, lineno, &prefix, &c[1], SymbolKind::Struct));
        } else if let Some(c) = p.enumm.captures(line) {
            out.push(mk(file, lineno, &prefix, &c[1], SymbolKind::Enum));
        } else if let Some(c) = p.treyt.captures(line) {
            out.push(mk(file, lineno, &prefix, &c[1], SymbolKind::Trait));
        } else if let Some(c) = p.alias.captures(line) {
            out.push(mk(file, lineno, &prefix, &c[1], SymbolKind::Type));
        }

        // Update brace depth, then pop finished scopes, then push the new one.
        let (opens, closes) = super::brace_delta(line);
        depth = depth.saturating_add(opens).saturating_sub(closes);
        while let Some(top) = stack.last() {
            if depth <= top.open_depth {
                stack.pop();
            } else {
                break;
            }
        }
        // Only push a scope if the opener line actually began a block (`{`).
        if let Some(name) = opened {
            if opens > closes {
                stack.push(Scope {
                    name,
                    open_depth: base_depth,
                });
            }
        }
    }
    out
}

/// Trim a trailing `//` line comment (best-effort; ignores `//` inside strings).
fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn qualified_prefix(stack: &[Scope]) -> String {
    stack
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

fn mk(file: &Path, line: u64, prefix: &str, name: &str, kind: SymbolKind) -> ExtractedSymbol {
    ExtractedSymbol {
        name: name.to_string(),
        qualified_name: join(prefix, name),
        kind,
        language: LanguageAdapter::Rust,
        file: file.to_path_buf(),
        line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn names(src: &str) -> Vec<(String, SymbolKind, u64)> {
        extract(Path::new("t.rs"), src)
            .into_iter()
            .map(|s| (s.qualified_name, s.kind, s.line))
            .collect()
    }

    #[test]
    fn top_level_items() {
        let src = "\
pub fn free_fn() {}
struct Config;
pub enum Region { One, Two }
trait Solver {}
pub type Alias = u32;
mod inner {}
";
        let got = names(src);
        assert!(got.contains(&("free_fn".into(), SymbolKind::Function, 1)));
        assert!(got.contains(&("Config".into(), SymbolKind::Struct, 2)));
        assert!(got.contains(&("Region".into(), SymbolKind::Enum, 3)));
        assert!(got.contains(&("Solver".into(), SymbolKind::Trait, 4)));
        assert!(got.contains(&("Alias".into(), SymbolKind::Type, 5)));
        assert!(got.contains(&("inner".into(), SymbolKind::Module, 6)));
    }

    #[test]
    fn methods_qualify_under_impl_and_mod() {
        let src = "\
mod nozzle {
    pub struct Throat;
    impl Throat {
        pub fn mass_flux(&self) -> f64 { 0.0 }
        fn helper() {}
    }
}
";
        let got = names(src);
        assert!(got.contains(&("nozzle::Throat".into(), SymbolKind::Struct, 2)));
        assert!(got.contains(&("nozzle::Throat::mass_flux".into(), SymbolKind::Function, 4)));
        assert!(got.contains(&("nozzle::Throat::helper".into(), SymbolKind::Function, 5)));
    }

    #[test]
    fn trait_impl_records_trait_for_type_but_qualifies_members_by_type() {
        let src = "\
impl Display for Widget {
    fn fmt(&self) {}
}
";
        let got = names(src);
        assert!(got
            .iter()
            .any(|(q, k, _)| q == "Display for Widget" && *k == SymbolKind::Impl));
        assert!(got.contains(&("Widget::fmt".into(), SymbolKind::Function, 2)));
    }

    #[test]
    fn scope_closes_after_block() {
        let src = "\
impl A {
    fn m() {}
}
fn after() {}
";
        let got = names(src);
        assert!(got.contains(&("A::m".into(), SymbolKind::Function, 2)));
        // `after` is back at top level, not under A.
        assert!(got.contains(&("after".into(), SymbolKind::Function, 4)));
    }

    #[test]
    fn ignores_commented_definition() {
        let src = "// fn ghost() {}\nfn real() {}\n";
        let got = names(src);
        assert!(!got.iter().any(|(q, _, _)| q == "ghost"));
        assert!(got.contains(&("real".into(), SymbolKind::Function, 2)));
    }
}
