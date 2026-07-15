//! Ripgrep-first symbol scanner for **Python** source.
//!
//! Detects `def` (functions and methods) and `class` definitions (see
//! `docs/kovan.md`, "# KOVAN Semantics" → "Python"; primary target OpenMC).
//! Deterministic and offline. Nesting is recovered from **indentation** — the
//! one place Python makes structure unambiguous — so methods get a dotted
//! qualified name (`Class.method`, `outer.inner`). The escalation path to
//! Pyright / Jedi lives in [`crate::adapters`].
//!
//! Known limits: indentation is measured in raw leading-whitespace width (a tab
//! counts as one). Decorators are skipped; the `def`/`class` keyword line is the
//! recorded location. Definitions produced dynamically (`type()`, `exec`) are
//! invisible — that is what the language-server escalation exists for.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{ExtractedSymbol, SymbolKind};
use crate::LanguageAdapter;

struct Scope {
    name: String,
    indent: usize,
}

struct Patterns {
    def: Regex,
    class: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        def: Regex::new(r"^(\s*)(?:async\s+)?def\s+([A-Za-z_]\w*)").unwrap(),
        class: Regex::new(r"^(\s*)class\s+([A-Za-z_]\w*)").unwrap(),
    })
}

/// Extract all Python symbols from `text`, recording `file` on each.
pub fn extract(file: &Path, text: &str) -> Vec<ExtractedSymbol> {
    let p = patterns();
    let mut out = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let lineno = (idx + 1) as u64;
        let (indent, name, kind) = if let Some(c) = p.class.captures(raw) {
            (c[1].len(), c[2].to_string(), SymbolKind::Class)
        } else if let Some(c) = p.def.captures(raw) {
            (c[1].len(), c[2].to_string(), SymbolKind::Function)
        } else {
            continue;
        };

        // Pop scopes at the same or deeper indent — we've dedented out of them.
        while let Some(top) = stack.last() {
            if indent <= top.indent {
                stack.pop();
            } else {
                break;
            }
        }
        let prefix = stack
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let qualified = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        out.push(ExtractedSymbol {
            name: name.clone(),
            qualified_name: qualified,
            kind,
            language: LanguageAdapter::Python,
            file: file.to_path_buf(),
            line: lineno,
        });
        stack.push(Scope { name, indent });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn names(src: &str) -> Vec<(String, SymbolKind, u64)> {
        extract(Path::new("t.py"), src)
            .into_iter()
            .map(|s| (s.qualified_name, s.kind, s.line))
            .collect()
    }

    #[test]
    fn top_level_def_and_class() {
        let src = "def solve():\n    pass\n\nclass Reactor:\n    pass\n";
        let got = names(src);
        assert!(got.contains(&("solve".into(), SymbolKind::Function, 1)));
        assert!(got.contains(&("Reactor".into(), SymbolKind::Class, 4)));
    }

    #[test]
    fn methods_qualify_under_class() {
        let src = "\
class Nozzle:
    def __init__(self):
        pass
    async def throat(self):
        pass
";
        let got = names(src);
        assert!(got.contains(&("Nozzle".into(), SymbolKind::Class, 1)));
        assert!(got.contains(&("Nozzle.__init__".into(), SymbolKind::Function, 2)));
        assert!(got.contains(&("Nozzle.throat".into(), SymbolKind::Function, 4)));
    }

    #[test]
    fn nested_function_qualifies_and_then_dedents() {
        let src = "\
class A:
    def m(self):
        def inner():
            pass
def top():
    pass
";
        let got = names(src);
        assert!(got.contains(&("A.m".into(), SymbolKind::Function, 2)));
        assert!(got.contains(&("A.m.inner".into(), SymbolKind::Function, 3)));
        // `top` dedented all the way out.
        assert!(got.contains(&("top".into(), SymbolKind::Function, 5)));
    }
}
