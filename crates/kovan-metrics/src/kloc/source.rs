//! Counting lines of Rust source the way the productivity accounting defines
//! them: **code lines exclude blank and comment-only lines**.
//!
//! # Why a comment stripper rather than a line-prefix test
//!
//! "Does this line start with `//`?" is wrong in both directions. It misses the
//! body of a block comment, and it fires on a `//` that is inside a string
//! literal. Since Rust doc comments carry executable doctests, a project that
//! documents heavily would have that work counted or discarded almost at
//! random depending on which mistake dominated.
//!
//! So the source is stripped properly — nested block comments, line comments,
//! ordinary and raw strings, byte strings and character literals — and a line
//! counts as code if anything survives. **Newlines inside removed comments are
//! preserved**, so the stripped text stays line-aligned with the original and
//! the blank/comment-only test lands on the right lines.
//!
//! # The lifetime problem
//!
//! `'a` in `&'a str` opens no character literal, and a naive `'`-scanner
//! swallows the rest of the file looking for a closing quote. A character
//! literal is therefore matched as a *shape* — `'x'`, `'\n'`, `'\x41'`,
//! `'\u{1F600}'` — and anything not matching that shape is treated as ordinary
//! code, which is exactly what a lifetime is.
//!
//! # Parity
//!
//! This is a direct port of `strip_rust_comments` from the retired
//! `scripts/kloc_accounting.py`, and its output must match that script's
//! byte-for-byte on the same inputs — see `docs/kloc-parity-baseline/`. Do not
//! "improve" the classification; a better stripper that disagrees with the
//! published figures is a regression here.

use std::collections::BTreeSet;
use std::path::Path;

/// Directories never descended into when counting source.
///
/// Build output, dependency caches and vendored third-party trees are not the
/// work being measured. `.git` is excluded because a packfile is not source.
pub const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "build",
    "__pycache__",
    ".cargo",
    "third_party",
];

/// Whether a repository-relative path lies inside a skipped directory.
///
/// Applied to paths listed out of a git tree, so it takes the *path* rather
/// than walking a filesystem: the measurement reads committed state, never a
/// working directory.
pub fn is_skipped(path: &str) -> bool {
    path.split('/')
        .any(|component| SKIP_DIRS.contains(&component))
}

/// Whether a repository-relative path is Rust source to be counted.
pub fn is_rust_source(path: &str) -> bool {
    path.ends_with(".rs") && !is_skipped(path)
}

/// Remove Rust comments from `src`, preserving line structure.
///
/// Handles nested block comments, line comments, ordinary and raw strings,
/// byte strings and character literals. Newlines inside removed comments are
/// kept, so line numbering — and therefore the caller's blank/comment-only
/// test — stays aligned with the original file.
pub fn strip_rust_comments(src: &str) -> String {
    let bytes: Vec<char> = src.chars().collect();
    let n = bytes.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0_usize;
    let mut depth = 0_usize;

    while i < n {
        if depth > 0 {
            if starts_with(&bytes, i, "/*") {
                depth += 1;
                i += 2;
                continue;
            }
            if starts_with(&bytes, i, "*/") {
                depth -= 1;
                i += 2;
                continue;
            }
            // Keep the newline so the stripped text stays line-aligned.
            if bytes[i] == '\n' {
                out.push('\n');
            }
            i += 1;
            continue;
        }

        if starts_with(&bytes, i, "//") {
            while i < n && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if starts_with(&bytes, i, "/*") {
            depth = 1;
            i += 2;
            continue;
        }

        let ch = bytes[i];

        // Raw string, but only at a token boundary -- otherwise the `r` in
        // `for` or the `b` in `verb` would open one.
        if ch == 'r' || ch == 'b' {
            let at_boundary = i == 0 || !is_ident_char(bytes[i - 1]);
            if at_boundary {
                if let Some((body_start, hashes)) = raw_string_opener(&bytes, i) {
                    let mut terminator = String::from("\"");
                    terminator.push_str(&"#".repeat(hashes));
                    let end = find_from(&bytes, body_start, &terminator)
                        .map(|p| p + terminator.chars().count())
                        .unwrap_or(n);
                    for &c in &bytes[i..end] {
                        out.push(c);
                    }
                    i = end;
                    continue;
                }
            }
        }

        if ch == '"' {
            let mut j = i + 1;
            while j < n {
                if bytes[j] == '\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == '"' {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let end = j.min(n);
            for &c in &bytes[i..end] {
                out.push(c);
            }
            i = end;
            continue;
        }

        if ch == '\'' {
            if let Some(end) = char_literal_end(&bytes, i) {
                for &c in &bytes[i..end] {
                    out.push(c);
                }
                i = end;
                continue;
            }
            // Otherwise a lifetime -- ordinary code, fall through.
        }

        out.push(ch);
        i += 1;
    }

    out
}

/// Does `bytes[at..]` begin with `needle`?
fn starts_with(bytes: &[char], at: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(offset, c)| bytes.get(at + offset) == Some(&c))
}

/// Index of the first occurrence of `needle` at or after `from`.
fn find_from(bytes: &[char], from: usize, needle: &str) -> Option<usize> {
    let len = needle.chars().count();
    if len == 0 || from > bytes.len() {
        return None;
    }
    (from..=bytes.len().saturating_sub(len)).find(|&i| starts_with(bytes, i, needle))
}

/// Rust identifier character, for the raw-string token-boundary test.
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// If a raw-string opener (`r"`, `r#"`, `br"`, `br#"`, …) starts at `i`,
/// return the index just past the opening quote and the number of `#`.
fn raw_string_opener(bytes: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if bytes.get(j) == Some(&'b') {
        j += 1;
    }
    if bytes.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0_usize;
    while bytes.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if bytes.get(j) != Some(&'"') {
        return None;
    }
    Some((j + 1, hashes))
}

/// If a character literal starts at `i`, return the index just past it.
///
/// Matches the shapes `'x'`, `'\n'`, `'\x41'`, `'\u{1F600}'` and nothing else,
/// so a lifetime (`'a`) yields `None` and is treated as ordinary code.
fn char_literal_end(bytes: &[char], i: usize) -> Option<usize> {
    let mut j = i + 1;
    let first = *bytes.get(j)?;

    if first == '\\' {
        j += 1;
        let escape = *bytes.get(j)?;
        match escape {
            'x' => {
                // \xNN -- exactly two hex digits.
                for _ in 0..2 {
                    j += 1;
                    if !bytes.get(j)?.is_ascii_hexdigit() {
                        return None;
                    }
                }
                j += 1;
            }
            'u' => {
                // \u{N..} -- one to six hex digits in braces.
                j += 1;
                if bytes.get(j) != Some(&'{') {
                    return None;
                }
                let mut digits = 0_usize;
                loop {
                    j += 1;
                    let c = *bytes.get(j)?;
                    if c == '}' {
                        break;
                    }
                    if !c.is_ascii_hexdigit() || digits == 6 {
                        return None;
                    }
                    digits += 1;
                }
                if digits == 0 {
                    return None;
                }
                j += 1;
            }
            // Any other single-character escape: \n, \t, \\, \', \0, …
            _ => j += 1,
        }
    } else {
        if first == '\\' || first == '\n' || first == '\'' {
            return None;
        }
        j += 1;
    }

    if bytes.get(j) == Some(&'\'') {
        Some(j + 1)
    } else {
        None
    }
}

/// Line counts for one body of Rust source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineCount {
    /// Every line, blank and comment-only included. Rust doc comments carry
    /// executable doctests, so they appear here and not in `code`.
    pub total: u64,
    /// Lines with something left after comments are stripped.
    pub code: u64,
    /// Number of `.rs` files counted.
    pub files: u64,
}

impl LineCount {
    /// Accumulate one file's contents.
    pub fn add_file(&mut self, contents: &str) {
        self.files += 1;
        self.total += contents.lines().count() as u64;
        self.code += strip_rust_comments(contents)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u64;
    }

    /// Sum of two counts.
    pub fn merge(mut self, other: Self) -> Self {
        self.total += other.total;
        self.code += other.code;
        self.files += other.files;
        self
    }
}

/// Whether `path` lies under any of `roots` (repository-relative, `/`-joined).
///
/// Used to exclude a subtree that is classified differently from the crate
/// containing it, so nothing is counted twice.
pub fn under_any(path: &str, roots: &BTreeSet<String>) -> bool {
    roots
        .iter()
        .any(|root| path == root || path.starts_with(&format!("{}/", root.trim_end_matches('/'))))
}

/// Convenience for callers holding a filesystem path rather than a git path.
pub fn is_rust_source_path(path: &Path) -> bool {
    path.to_str().is_some_and(is_rust_source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_lines(src: &str) -> u64 {
        let mut count = LineCount::default();
        count.add_file(src);
        count.code
    }

    #[test]
    fn line_comments_do_not_count_as_code() {
        assert_eq!(code_lines("// just a comment\n"), 0);
        assert_eq!(code_lines("let x = 1; // trailing\n"), 1);
        assert_eq!(code_lines("/// doc comment\nfn f() {}\n"), 1);
    }

    #[test]
    fn block_comments_nest() {
        let src = "/* outer /* inner */ still outer */ fn f() {}\n";
        assert_eq!(code_lines(src), 1);
        let spanning = "/*\n a\n b\n*/\nfn f() {}\n";
        assert_eq!(code_lines(spanning), 1);
    }

    /// A comment's newlines must survive, or every later line is misattributed.
    #[test]
    fn stripping_preserves_line_alignment() {
        let src = "fn a() {}\n/*\n\n\n*/\nfn b() {}\n";
        let stripped = strip_rust_comments(src);
        assert_eq!(
            stripped.lines().count(),
            src.lines().count(),
            "stripped text must stay line-aligned with the original"
        );
        assert_eq!(code_lines(src), 2);
    }

    /// A `//` inside a string is not a comment.
    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_comment() {
        assert_eq!(code_lines("let url = \"https://example.com\";\n"), 1);
        assert_eq!(
            code_lines("let s = \"/* not a comment */\";\nlet t = 1;\n"),
            2
        );
    }

    #[test]
    fn raw_strings_are_respected_including_hashes() {
        assert_eq!(code_lines("let s = r\"a // b\";\n"), 1);
        assert_eq!(code_lines("let s = r#\"a \" // b\"#;\n"), 1);
        assert_eq!(code_lines("let s = br#\"bytes /* x */\"#;\n"), 1);
    }

    /// The `r` in `for` must not open a raw string -- the token-boundary test.
    #[test]
    fn an_r_inside_an_identifier_does_not_open_a_raw_string() {
        let src = "for x in v { }\n// comment\n";
        assert_eq!(code_lines(src), 1);
        let verb = "let verb = 1;\n// comment\n";
        assert_eq!(code_lines(verb), 1);
    }

    /// **A lifetime is not a character literal.** Getting this wrong swallows
    /// the rest of the file looking for a closing quote, so a single `&'a str`
    /// would silently zero a file's count.
    #[test]
    fn a_lifetime_is_not_mistaken_for_a_character_literal() {
        let src = "fn f<'a>(x: &'a str) -> &'a str { x }\n// comment\nfn g() {}\n";
        assert_eq!(code_lines(src), 2);

        let stripped = strip_rust_comments(src);
        assert!(
            stripped.contains("&'a str"),
            "the lifetime must survive stripping, got: {stripped}"
        );
    }

    #[test]
    fn character_literals_are_recognised_in_all_their_shapes() {
        for literal in ["'x'", r"'\n'", r"'\\'", r"'\x41'", r"'\u{1F600}'", r"'\''"] {
            let src = format!("let c = {literal};\n// comment\n");
            assert_eq!(code_lines(&src), 1, "failed on {literal}");
        }
    }

    #[test]
    fn blank_lines_never_count() {
        assert_eq!(code_lines("\n\n   \n\t\n"), 0);
        assert_eq!(code_lines("fn f() {}\n\n\nfn g() {}\n"), 2);
    }

    #[test]
    fn totals_include_what_code_excludes() {
        let mut count = LineCount::default();
        count.add_file("// a\n// b\nfn f() {}\n\n");
        assert_eq!(count.total, 4, "every line, blank and comment included");
        assert_eq!(count.code, 1);
        assert_eq!(count.files, 1);
    }

    #[test]
    fn skipped_directories_are_excluded_by_path() {
        assert!(is_rust_source("src/lib.rs"));
        assert!(!is_rust_source("target/debug/build/x.rs"));
        assert!(!is_rust_source("vendor/foo/src/lib.rs"));
        assert!(!is_rust_source("src/lib.py"));
        assert!(!is_rust_source("third_party/a/b.rs"));
        // A directory merely CONTAINING a skip name is not skipped.
        assert!(is_rust_source("src/targeting/mod.rs"));
    }

    #[test]
    fn subtree_exclusion_matches_on_path_boundaries() {
        let roots: BTreeSet<String> = ["src/bin/njoy-tui".to_string()].into_iter().collect();
        assert!(under_any("src/bin/njoy-tui/main.rs", &roots));
        assert!(under_any("src/bin/njoy-tui", &roots));
        // A sibling with a shared prefix must NOT be excluded.
        assert!(!under_any("src/bin/njoy-tui-extra/main.rs", &roots));
        assert!(!under_any("src/lib.rs", &roots));
    }
}
