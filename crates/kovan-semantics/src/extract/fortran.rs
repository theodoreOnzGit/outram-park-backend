//! Ripgrep-first symbol scanner for **Fortran** source.
//!
//! Detects `module`, `subroutine`, `function`, and derived-`type` definitions
//! (see `docs/kovan.md`, "# KOVAN Semantics" → "Fortran"; primary target NJOY).
//! Deterministic, offline, case-insensitive. Nesting is recovered from matching
//! `end module` / `end subroutine` / `end function` / `end type` keywords, so a
//! module procedure gets a `::`-qualified name (`module::subroutine`). The
//! escalation path to `fortls` lives in [`crate::adapters`].
//!
//! Known limits: fixed-form column rules and `&` line continuations are not
//! modelled — the definition's keyword line is read as-is (free-form friendly).
//! A `function` header split across continuations is read from its first line.
//! `type(foo) :: x` variable declarations are correctly *not* treated as a
//! derived-type definition; `type :: foo` and `type foo` are.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{ExtractedSymbol, SymbolKind};
use crate::LanguageAdapter;

struct Scope {
    name: String,
    /// The keyword this scope will be closed by (`"module"`, `"subroutine"`,
    /// `"function"`, `"type"`).
    closer: &'static str,
}

struct Patterns {
    module: Regex,
    subroutine: Regex,
    function: Regex,
    dtype: Regex,
    end: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    // The `regex` crate has no look-around, so `module procedure` / `type(...)`
    // exclusions are handled by post-filtering the captured name in `extract`.
    P.get_or_init(|| Patterns {
        module: Regex::new(r"(?i)^\s*module\s+([A-Za-z_]\w*)").unwrap(),
        subroutine: Regex::new(
            r"(?i)^\s*(?:(?:pure|impure|elemental|recursive|module)\s+)*subroutine\s+([A-Za-z_]\w*)",
        )
        .unwrap(),
        // Any prefix (incl. a return type like `real(dp)`) then `function NAME(`.
        function: Regex::new(r"(?i)^\s*(?:[^!\n]*?\s)?function\s+([A-Za-z_]\w*)\s*\(").unwrap(),
        // Derived-type definition: `type[, attrs] [::] NAME`. A `type(kind) ::`
        // variable declaration does not match: after `type` comes `(`, which the
        // trailing `[A-Za-z_]` name atom cannot consume.
        dtype: Regex::new(r"(?i)^\s*type\b(?:\s*,[^:]*)?\s*(?:::)?\s*([A-Za-z_]\w*)").unwrap(),
        end: Regex::new(r"(?i)^\s*end\s*(module|subroutine|function|type)\b").unwrap(),
    })
}

/// Extract all Fortran symbols from `text`, recording `file` on each.
pub fn extract(file: &Path, text: &str) -> Vec<ExtractedSymbol> {
    let p = patterns();
    let mut out = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let lineno = (idx + 1) as u64;
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }

        // `end <kw>` closes the nearest matching scope.
        if let Some(c) = p.end.captures(line) {
            let kw = c[1].to_ascii_lowercase();
            if let Some(pos) = stack.iter().rposition(|s| s.closer == kw) {
                stack.truncate(pos);
            }
            continue;
        }

        let prefix = stack
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let mk = |name: &str, kind: SymbolKind| ExtractedSymbol {
            name: name.to_string(),
            qualified_name: if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}::{name}")
            },
            kind,
            language: LanguageAdapter::Fortran,
            file: file.to_path_buf(),
            line: lineno,
        };

        if let Some(c) = p.module.captures(line) {
            let name = c[1].to_string();
            // `module procedure/subroutine/function ...` are not module defs.
            let is_module_def = !matches!(
                name.to_ascii_lowercase().as_str(),
                "procedure" | "subroutine" | "function"
            );
            if is_module_def {
                out.push(mk(&name, SymbolKind::Module));
                stack.push(Scope {
                    name,
                    closer: "module",
                });
            }
        } else if let Some(c) = p.subroutine.captures(line) {
            let name = c[1].to_string();
            out.push(mk(&name, SymbolKind::Subroutine));
            stack.push(Scope {
                name,
                closer: "subroutine",
            });
        } else if let Some(c) = p.function.captures(line) {
            let name = c[1].to_string();
            out.push(mk(&name, SymbolKind::Function));
            stack.push(Scope {
                name,
                closer: "function",
            });
        } else if let Some(c) = p.dtype.captures(line) {
            // Guard against the `end type` case already handled, and against a
            // stray keyword like `type is (...)` in a select-type block.
            let name = c[1].to_string();
            if !name.eq_ignore_ascii_case("is") {
                out.push(mk(&name, SymbolKind::Type));
                stack.push(Scope {
                    name,
                    closer: "type",
                });
            }
        }
    }
    out
}

/// Trim a trailing `!` comment (best-effort; ignores `!` inside strings).
fn strip_comment(line: &str) -> &str {
    match line.find('!') {
        Some(i) => &line[..i],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn names(src: &str) -> Vec<(String, SymbolKind, u64)> {
        extract(Path::new("t.f90"), src)
            .into_iter()
            .map(|s| (s.qualified_name, s.kind, s.line))
            .collect()
    }

    #[test]
    fn module_with_contained_procedures() {
        let src = "\
module reconr
  implicit none
  type :: grid
    integer :: n
  end type grid
contains
  subroutine broadr(x)
    real :: x
  end subroutine broadr
  real function sigma(e)
    real :: e
    sigma = e
  end function sigma
end module reconr
";
        let got = names(src);
        assert!(got.contains(&("reconr".into(), SymbolKind::Module, 1)));
        assert!(got.contains(&("reconr::grid".into(), SymbolKind::Type, 3)));
        assert!(got.contains(&("reconr::broadr".into(), SymbolKind::Subroutine, 7)));
        assert!(got.contains(&("reconr::sigma".into(), SymbolKind::Function, 10)));
    }

    #[test]
    fn type_variable_declaration_is_not_a_definition() {
        let src = "\
subroutine s(a)
  type(grid) :: a
end subroutine s
";
        let got = names(src);
        assert!(got.contains(&("s".into(), SymbolKind::Subroutine, 1)));
        // `type(grid) :: a` is a declaration, not a derived-type definition.
        assert!(!got.iter().any(|(_, k, _)| *k == SymbolKind::Type));
    }

    #[test]
    fn case_insensitive_and_prefixed() {
        let src = "\
MODULE Physics
  PURE SUBROUTINE Step()
  END SUBROUTINE Step
END MODULE Physics
";
        let got = names(src);
        assert!(got.contains(&("Physics".into(), SymbolKind::Module, 1)));
        assert!(got.contains(&("Physics::Step".into(), SymbolKind::Subroutine, 2)));
    }

    #[test]
    fn comments_are_stripped() {
        let src = "! subroutine ghost()\nsubroutine real_sub()\nend subroutine\n";
        let got = names(src);
        assert!(!got.iter().any(|(q, _, _)| q == "ghost"));
        assert!(got.contains(&("real_sub".into(), SymbolKind::Subroutine, 2)));
    }
}
