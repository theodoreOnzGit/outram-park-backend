//! The module tree of one Cargo target, read from source text: which `mod`
//! declarations a file makes, which file each one loads, and which line
//! ranges are test-only code.
//!
//! Pure functions over a file's lines. The caller supplies the source lines
//! as read (`raw`) and the same lines with comments, literals and attributes
//! blanked to spaces (`code`, as `commands::code_walk::source::blank_non_code`
//! produces them), so a `mod` inside a comment or a string is not a
//! declaration, while attributes (`#[cfg(test)]`, `#[path = "…"]`) are read
//! from `raw`.
//!
//! # File resolution (the Rust reference, "Modules", 2018 edition and later)
//!
//! For `mod x;` in file `F`:
//!
//! - `F` is a **mod-rs file** when it is a target's root (`lib.rs`,
//!   `main.rs`, an example's root) or is named `mod.rs`: `x` is looked up in
//!   `F`'s directory. Otherwise (`a/b.rs`) it is looked up in `a/b/`.
//! - Inside inline modules (`mod a { mod x; }`) each enclosing name adds a
//!   directory: `…/a/x.rs`.
//! - `x.rs` is preferred to `x/mod.rs`; the first that exists wins.
//! - `#[path = "p"]` on a declaration outside any inline module is relative
//!   to `F`'s own directory; inside inline modules it is relative to the
//!   directory those modules name. (The reference's finer rule for
//!   `#[path]` inside an inline module of a non-mod-rs file is not
//!   reproduced; no crate in this workspace uses it.)
//!
//! Inline modules themselves are not separate entries in the call graph:
//! their functions belong to the file's module. A `#[cfg(test)]` inline
//! module marks its line range as test code ([`test_ranges`]).

/// An out-of-line `mod name;` declaration found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDecl {
    pub name: String,
    /// Names of the inline modules the declaration sits in, outermost first.
    pub inline: Vec<String>,
    /// The `#[path = "…"]` attribute's value, if any.
    pub path_attr: Option<String>,
    /// `#[cfg(test)]` (or a `cfg` naming `test`) on the declaration or on an
    /// enclosing inline module.
    pub test: bool,
    /// 0-based line of the `mod` keyword.
    pub line: u32,
}

fn is_ident(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// The attribute and doc lines directly above 0-based `line`, plus the text
/// on `line` before column `col`: where an item's attributes are written.
fn attributes_above(raw: &[String], line: u32, col: usize) -> String {
    let mut out = String::new();
    if let Some(l) = raw.get(line as usize) {
        out.push_str(&l.chars().take(col).collect::<String>());
    }
    let mut k = line as usize;
    while k > 0 {
        k -= 1;
        let t = raw[k].trim();
        if t.starts_with("#[") || t.starts_with("///") || t.ends_with(")]") || t.ends_with("\"]") {
            out.push('\n');
            out.push_str(t);
        } else {
            break;
        }
    }
    out
}

/// Whether attribute text gates the item on `test`: `#[cfg(test)]`,
/// `#[cfg(all(test, …))]` and the like (not `cfg(not(test))`).
pub fn is_test_cfg(attrs: &str) -> bool {
    attrs.split("#[").skip(1).any(|a| {
        let a = a.replace(' ', "");
        a.starts_with("cfg(") && !a.contains("not(test") && {
            let inner = &a[4..];
            inner.split(|c: char| !is_ident(c)).any(|w| w == "test")
        }
    })
}

/// Whether attribute text marks a function as a test (`#[test]`,
/// `#[tokio::test]`, `#[rstest]`, …).
pub fn is_test_attr(attrs: &str) -> bool {
    attrs.split("#[").skip(1).any(|a| {
        let head: String = a.chars().take_while(|&c| c != ']' && c != '(').collect();
        let head = head.trim();
        head == "test" || head.ends_with("::test") || head == "rstest"
    })
}

fn path_attr(attrs: &str) -> Option<String> {
    for a in attrs.split("#[").skip(1) {
        let t = a.trim_start();
        if let Some(rest) = t.strip_prefix("path") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
    }
    None
}

/// What [`scan`] finds in one file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileModules {
    /// Out-of-line declarations, in source order.
    pub decls: Vec<ModDecl>,
    /// 0-based inclusive line ranges of `#[cfg(test)]` inline module bodies.
    pub test_ranges: Vec<(u32, u32)>,
}

/// Every `mod` declaration in a file, tracking inline modules by brace depth.
pub fn scan(raw: &[String], code: &[String]) -> FileModules {
    // (char, line, column) of every code character, newlines as ' '.
    let mut chars: Vec<(char, u32, usize)> = Vec::new();
    for (ln, l) in code.iter().enumerate() {
        for (col, ch) in l.chars().enumerate() {
            chars.push((ch, ln as u32, col));
        }
        chars.push((' ', ln as u32, l.chars().count()));
    }
    let mut out = FileModules::default();
    // Open inline modules: (name, brace depth inside it, test, open line).
    let mut stack: Vec<(String, i64, bool, u32)> = Vec::new();
    let mut depth: i64 = 0;
    let mut i = 0;
    while i < chars.len() {
        let (ch, ln, col) = chars[i];
        match ch {
            '{' => depth += 1,
            '}' => {
                if let Some((_, _, true, open)) = stack.pop_if(|top| top.1 == depth) {
                    out.test_ranges.push((open, ln));
                }
                depth -= 1;
            }
            'm' if (i == 0 || !is_ident(chars[i - 1].0))
                && chars.get(i + 1).map(|c| c.0) == Some('o')
                && chars.get(i + 2).map(|c| c.0) == Some('d')
                && chars.get(i + 3).is_some_and(|c| c.0 == ' ') =>
            {
                let mut j = i + 3;
                while j < chars.len() && chars[j].0 == ' ' {
                    j += 1;
                }
                let start = j;
                while j < chars.len() && is_ident(chars[j].0) {
                    j += 1;
                }
                if j > start {
                    let name: String = chars[start..j].iter().map(|c| c.0).collect();
                    let mut k = j;
                    while k < chars.len() && chars[k].0 == ' ' {
                        k += 1;
                    }
                    let attrs = attributes_above(raw, ln, col);
                    let outer_test = stack.iter().any(|s| s.2);
                    let test = outer_test || is_test_cfg(&attrs);
                    match chars.get(k).map(|c| c.0) {
                        Some(';') => {
                            out.decls.push(ModDecl {
                                name,
                                inline: stack.iter().map(|s| s.0.clone()).collect(),
                                path_attr: path_attr(&attrs),
                                test,
                                line: ln,
                            });
                            i = k + 1;
                            continue;
                        }
                        Some('{') => {
                            depth += 1;
                            stack.push((name, depth, test, ln));
                            i = k + 1;
                            continue;
                        }
                        _ => {}
                    }
                }
                i = j.max(i + 1);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// `a/b` of `a/b/c.rs`; empty for a bare file name.
fn dir_of(file: &str) -> &str {
    file.rfind('/').map(|i| &file[..i]).unwrap_or("")
}

fn join(dir: &str, rest: &str) -> String {
    if dir.is_empty() {
        rest.to_string()
    } else {
        format!("{dir}/{rest}")
    }
}

/// Collapses `.` and `..` segments of a `/`-separated relative path.
fn normalise(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

/// The files `decl` (found in `file`) may load, in the order Rust tries
/// them. `mod_rs` says whether `file` is a mod-rs file (a target root or a
/// `mod.rs`).
pub fn candidate_files(file: &str, mod_rs: bool, decl: &ModDecl) -> Vec<String> {
    let own_dir = dir_of(file);
    let base = if mod_rs {
        own_dir.to_string()
    } else {
        let stem = file
            .rsplit('/')
            .next()
            .unwrap_or(file)
            .trim_end_matches(".rs");
        join(own_dir, stem)
    };
    let mut dir = base;
    for m in &decl.inline {
        dir = join(&dir, m);
    }
    if let Some(p) = &decl.path_attr {
        let from = if decl.inline.is_empty() {
            own_dir.to_string()
        } else {
            dir
        };
        return vec![normalise(&join(&from, p))];
    }
    vec![
        join(&dir, &format!("{}.rs", decl.name)),
        join(&dir, &format!("{}/mod.rs", decl.name)),
    ]
}

/// Whether a file loaded by a `mod` declaration is a mod-rs file.
pub fn is_mod_rs(file: &str) -> bool {
    file == "mod.rs" || file.ends_with("/mod.rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    /// Methodology: a file with an out-of-line module, a `#[cfg(test)]`
    /// out-of-line module, an inline module holding a declaration, an inline
    /// test module, a `#[path]` module and a `mod` word inside a comment
    /// (blanked in `code`). Each must come out as written.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn scan_finds_declarations_inline_modules_and_tests() {
        let raw = lines(
            "pub mod physics;\n#[cfg(test)]\nmod checks;\nmod outer {\n    pub(crate) mod inner;\n}\n#[path = \"../shared/x.rs\"]\nmod shared;\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n// mod not_this;\n",
        );
        let mut code = raw.clone();
        code[6] = " ".repeat(raw[6].len());
        code[8] = " ".repeat(raw[8].len());
        code[1] = " ".repeat(raw[1].len());
        code[12] = " ".repeat(raw[12].len());
        let f = scan(&raw, &code);
        let names: Vec<(&str, Vec<String>, bool)> = f
            .decls
            .iter()
            .map(|d| (d.name.as_str(), d.inline.clone(), d.test))
            .collect();
        assert_eq!(
            names,
            vec![
                ("physics", vec![], false),
                ("checks", vec![], true),
                ("inner", vec!["outer".to_string()], false),
                ("shared", vec![], false),
            ]
        );
        assert_eq!(f.decls[3].path_attr.as_deref(), Some("../shared/x.rs"));
        assert_eq!(f.test_ranges, vec![(9, 11)]);
    }

    /// Methodology: the reference's lookup rules for a mod-rs root, a
    /// non-mod-rs file, an inline module and a `#[path]` attribute.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn candidate_files_follow_the_reference() {
        let d = |name: &str, inline: &[&str], p: Option<&str>| ModDecl {
            name: name.into(),
            inline: inline.iter().map(|s| s.to_string()).collect(),
            path_attr: p.map(str::to_string),
            test: false,
            line: 0,
        };
        assert_eq!(
            candidate_files("c/examples/sim/main.rs", true, &d("physics", &[], None)),
            vec!["c/examples/sim/physics.rs", "c/examples/sim/physics/mod.rs"]
        );
        assert_eq!(
            candidate_files("c/src/a.rs", false, &d("b", &[], None)),
            vec!["c/src/a/b.rs", "c/src/a/b/mod.rs"]
        );
        assert_eq!(
            candidate_files("c/src/lib.rs", true, &d("x", &["outer"], None)),
            vec!["c/src/outer/x.rs", "c/src/outer/x/mod.rs"]
        );
        assert_eq!(
            candidate_files("c/src/a.rs", false, &d("x", &[], Some("../shared/x.rs"))),
            vec!["c/shared/x.rs"]
        );
    }

    /// Methodology: attribute classification, including the negative case.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn test_attributes_are_recognised() {
        assert!(is_test_cfg("#[cfg(test)]"));
        assert!(is_test_cfg("#[cfg(all(test, feature = \"x\"))]"));
        assert!(!is_test_cfg("#[cfg(not(test))]"));
        assert!(!is_test_cfg("#[cfg(feature = \"testing\")]"));
        assert!(is_test_attr("#[test]"));
        assert!(is_test_attr("#[tokio::test]"));
        assert!(!is_test_attr("#[inline]"));
    }
}
