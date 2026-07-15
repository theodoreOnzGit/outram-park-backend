//! Ripgrep-first symbol scanner for **C++** source.
//!
//! Detects `namespace`, `class` / `struct` / `union`, and function
//! definitions/declarations (see `docs/kovan.md`, "# KOVAN Semantics" → "C++";
//! primary target OpenFOAM). Deterministic and offline. Enclosing `namespace`
//! and class scopes are tracked by textual brace depth so members get a
//! `::`-qualified name. The escalation path to `clangd` / `libclang` lives in
//! [`crate::adapters`].
//!
//! C++ is the least regular of the four languages, so this scanner is the most
//! conservative — it favours precision over recall:
//!
//! - Functions are recognised only when the `name(args)` header ends the line
//!   with `{` (definition) or `;` (prototype). Multi-line signatures and
//!   trailing-return-type templates may be missed. Control-flow keywords
//!   (`if`/`for`/`while`/…) are blacklisted so `if (x) {` is never a "function".
//! - A class/namespace body is tracked when its `{` is on the keyword line, or
//!   on the immediately following line (the two dominant brace styles). Other
//!   layouts fall back to an unqualified name.
//! - Braces inside strings/comments are counted textually (documented shared
//!   limit). Macros (heavy in OpenFOAM) that hide definitions are invisible —
//!   that is what the `clangd` escalation path exists for.

use std::path::Path;
use std::sync::OnceLock;

use super::{ExtractedSymbol, SymbolKind};
use crate::LanguageAdapter;
use regex::Regex;

struct Scope {
    name: String,
    open_depth: usize,
}

struct Patterns {
    namespace: Regex,
    record: Regex,
    /// A function *definition*: `[ret] name(args) [qualifiers] {` — the body's
    /// `{` opens on this line. The return type is optional (constructors,
    /// destructors), so control-flow keywords are excluded separately.
    func_def: Regex,
    /// A function *prototype*: `ret name(args) [qualifiers];`. A return type is
    /// mandatory here — that is what distinguishes a declaration from a bare
    /// call statement like `solve(x);`.
    func_proto: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        namespace: Regex::new(r"^\s*namespace\s+([A-Za-z_]\w*)").unwrap(),
        record: Regex::new(
            r"^\s*(?:template\s*<[^>]*>\s*)?(class|struct|union)\s+([A-Za-z_]\w*)",
        )
        .unwrap(),
        func_def: Regex::new(
            r"^\s*(?:[A-Za-z_][\w:<>,\*&\s]*?\s[\*&\s]*)?(~?[A-Za-z_]\w*|operator\S+)\s*\([^;{}]*\)\s*(?:const\s*)?(?:noexcept[^{;]*)?(?:override\s*)?(?:final\s*)?\{",
        )
        .unwrap(),
        func_proto: Regex::new(
            r"^\s*[A-Za-z_][\w:<>,\*&\s]*?\s[\*&\s]*(~?[A-Za-z_]\w*|operator\S+)\s*\([^;{}]*\)\s*(?:const\s*)?(?:noexcept[^;]*)?(?:override\s*)?(?:final\s*)?(?:=\s*(?:0|default|delete)\s*)?;\s*$",
        )
        .unwrap(),
    })
}

const KEYWORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "return",
    "else",
    "do",
    "sizeof",
    "decltype",
    "static_assert",
    "throw",
    "new",
    "delete",
    "case",
    "using",
    "typedef",
    "template",
    "explicit",
    "friend",
    "and",
    "or",
    "not",
];

/// Extract all C++ symbols from `text`, recording `file` on each.
pub fn extract(file: &Path, text: &str) -> Vec<ExtractedSymbol> {
    let p = patterns();
    let mut out = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();
    let mut depth: usize = 0;
    // A class/namespace whose opening `{` is expected on the next line.
    let mut pending: Option<(String, usize)> = None;

    for raw in split_lines(text) {
        let (lineno, line_owned) = raw;
        let line = strip_comment(&line_owned);
        let base_depth = depth;

        // Resolve a pending opener whose `{` is on this (the following) line.
        if let Some((name, open_depth)) = pending.take() {
            if line.trim_start().starts_with('{') {
                stack.push(Scope { name, open_depth });
            }
            // else: brace style not recognised — drop the pending scope.
        }

        let prefix = stack
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let mut opened: Option<(String, SymbolKind)> = None;

        if let Some(c) = p.namespace.captures(line) {
            let name = c[1].to_string();
            out.push(mk(file, lineno, &prefix, &name, SymbolKind::Namespace));
            opened = Some((name, SymbolKind::Namespace));
        } else if let Some(c) = p.record.captures(line) {
            // Skip forward declarations: `class Foo;` with no body.
            let is_fwd = line.trim_end().ends_with(';');
            let kind = match &c[1] {
                "class" => SymbolKind::Class,
                "union" => SymbolKind::Union,
                _ => SymbolKind::Struct,
            };
            let name = c[2].to_string();
            out.push(mk(file, lineno, &prefix, &name, kind));
            if !is_fwd {
                opened = Some((name, kind));
            }
        } else if let Some(c) = p
            .func_def
            .captures(line)
            .or_else(|| p.func_proto.captures(line))
        {
            let name = c[1].trim().to_string();
            let bare = name.trim_start_matches('~');
            if !KEYWORDS.contains(&bare) && !bare.is_empty() {
                out.push(mk(file, lineno, &prefix, &name, SymbolKind::Function));
            }
        }

        // Update depth, pop finished scopes.
        let (opens, closes) = super::brace_delta(line);
        depth = depth.saturating_add(opens).saturating_sub(closes);
        while let Some(top) = stack.last() {
            if depth <= top.open_depth {
                stack.pop();
            } else {
                break;
            }
        }

        // Push (or defer) a newly opened class/namespace scope.
        if let Some((name, _)) = opened {
            if opens > closes {
                stack.push(Scope {
                    name,
                    open_depth: base_depth,
                });
            } else if opens == closes {
                // No brace yet on this line → expect it on the next.
                pending = Some((name, base_depth));
            }
        }
    }
    out
}

fn mk(file: &Path, line: u64, prefix: &str, name: &str, kind: SymbolKind) -> ExtractedSymbol {
    let qualified = if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    };
    ExtractedSymbol {
        name: name.to_string(),
        qualified_name: qualified,
        kind,
        language: LanguageAdapter::Cpp,
        file: file.to_path_buf(),
        line,
    }
}

/// Trim a trailing `//` comment (best-effort; ignores `//` inside strings).
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn split_lines(text: &str) -> impl Iterator<Item = (u64, String)> + '_ {
    text.lines()
        .enumerate()
        .map(|(i, l)| ((i + 1) as u64, l.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn names(src: &str) -> Vec<(String, SymbolKind, u64)> {
        extract(Path::new("t.cpp"), src)
            .into_iter()
            .map(|s| (s.qualified_name, s.kind, s.line))
            .collect()
    }

    #[test]
    fn free_function_and_prototype() {
        let src = "\
int add(int a, int b) {
    return a + b;
}
void ping();
";
        let got = names(src);
        assert!(got.contains(&("add".into(), SymbolKind::Function, 1)));
        assert!(got.contains(&("ping".into(), SymbolKind::Function, 4)));
    }

    #[test]
    fn class_methods_qualify_same_line_brace() {
        let src = "\
class Nozzle {
public:
    double massFlux() const {
        return 0.0;
    }
};
";
        let got = names(src);
        assert!(got.contains(&("Nozzle".into(), SymbolKind::Class, 1)));
        assert!(got.contains(&("Nozzle::massFlux".into(), SymbolKind::Function, 3)));
    }

    #[test]
    fn namespace_next_line_brace_and_nesting() {
        let src = "\
namespace Foam
{
    class Solver
    {
        void solve() {}
    };
}
";
        let got = names(src);
        assert!(got.contains(&("Foam".into(), SymbolKind::Namespace, 1)));
        assert!(got.contains(&("Foam::Solver".into(), SymbolKind::Class, 3)));
        assert!(got.contains(&("Foam::Solver::solve".into(), SymbolKind::Function, 5)));
    }

    #[test]
    fn control_flow_is_not_a_function() {
        let src = "\
void run() {
    if (ready) {
        for (int i = 0; i < n; i++) {
        }
    }
}
";
        let got = names(src);
        assert!(got.contains(&("run".into(), SymbolKind::Function, 1)));
        assert!(!got.iter().any(|(q, _, _)| q == "if" || q == "for"));
    }

    #[test]
    fn forward_declaration_does_not_open_scope() {
        let src = "\
class Fwd;
void after() {}
";
        let got = names(src);
        assert!(got.contains(&("Fwd".into(), SymbolKind::Class, 1)));
        // `after` must be top level, not `Fwd::after`.
        assert!(got.contains(&("after".into(), SymbolKind::Function, 2)));
    }
}
