//! **Quick fixes in kvim** (GitHub #770; #740 correction of 2026-10-07):
//! "Fixes are made either in kvim inside kovan (quick fixes) or in the
//! maintainer's own Neovim, opened by the maintainer from the CLI. kovan
//! offers no Neovim launcher and no agent hand-off."
//!
//! The review panel opens the function's own lines (doc comment included)
//! in kovan's kvim editor; Save writes those lines back into the file and
//! nothing else. A save is refused when the file's lines are no longer the
//! ones opened (an edit, a pull or a checkout meanwhile): the editor is
//! reloaded rather than overwriting someone else's change (Leak Before
//! Break). The saved edit changes the function's hash, so the existing
//! rule applies: the stamp is refused until the change is committed and
//! the diff shown (#740 decision 7).
//!
//! "Document it now" (#740, the upstream-fidelity question's
//! `deviation_not_documented`) opens the same editor with a deviation
//! template inserted above the function's doc comment
//! ([`with_deviation_template`]): "what differs, why".

use std::path::Path;

use super::diff::find_fn;

/// The function's lines as opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickFix {
    /// Workspace-relative file.
    pub file: String,
    /// 1-based inclusive lines in the file when opened.
    pub lines: [u32; 2],
    /// Those lines, as opened (joined with `\n`).
    pub original: String,
}

/// Open function `qual` of `file` for a quick fix.
pub fn open(root: &Path, file: &str, qual: &str) -> Result<QuickFix, String> {
    let text = std::fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
    let f = find_fn(&text, qual).ok_or_else(|| format!("{file}::{qual}: not found"))?;
    Ok(QuickFix {
        file: file.to_string(),
        lines: f.entry.lines,
        original: super::diff::lines_of(&text, f.entry.lines),
    })
}

/// `text` with lines `[a, b]` replaced by `new` (line endings of the rest
/// kept), or `None` when those lines are not `expected`.
pub fn splice_lines(text: &str, [a, b]: [u32; 2], expected: &str, new: &str) -> Option<String> {
    let parts: Vec<&str> = text.split_inclusive('\n').collect();
    let (a, b) = (a as usize, b as usize);
    if a == 0 || b < a || b > parts.len() {
        return None;
    }
    let current: String = parts[a - 1..b].concat();
    // `str::lines` drops `\r\n` and `\n` alike, as `lines_of` did.
    if current.lines().collect::<Vec<_>>().join("\n") != expected {
        return None;
    }
    let ending = if parts[b - 1].ends_with("\r\n") {
        "\r\n"
    } else if parts[b - 1].ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let mut out: String = parts[..a - 1].concat();
    out.push_str(new.trim_end_matches(['\n', '\r']));
    out.push_str(ending);
    out.push_str(&parts[b..].concat());
    Some(out)
}

/// Save `new` over the opened lines (module doc). Refused when the file
/// changed under the editor.
pub fn save(root: &Path, q: &QuickFix, new: &str) -> Result<(), String> {
    let path = root.join(&q.file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", q.file))?;
    let out = splice_lines(&text, q.lines, &q.original, new).ok_or_else(|| {
        format!(
            "{} changed since the quick fix was opened: reload it (nothing was written)",
            q.file
        )
    })?;
    std::fs::write(&path, out).map_err(|e| format!("{}: {e}", q.file))
}

/// The function's lines with a deviation template inserted at the top of
/// its doc comment (or above the item when it has none), indented as the
/// first line.
pub fn with_deviation_template(original: &str) -> String {
    let first = original.lines().next().unwrap_or("");
    let indent: String = first.chars().take_while(|c| c.is_whitespace()).collect();
    let template = format!(
        "{indent}/// **Deviation from upstream:** what differs from the upstream routine: TODO.\n\
         {indent}/// Why: TODO.\n\
         {indent}///\n"
    );
    format!("{template}{original}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: open a method, save an edited body, then try to save
    /// over a file changed meanwhile. Pass: only the function's lines
    /// change (the rest byte for byte, CRLF endings kept); the stale save
    /// is refused and nothing is written.
    #[test]
    fn save_writes_only_the_function_and_refuses_a_stale_file() {
        let d = tempfile::tempdir().unwrap();
        let src = "// head\r\nstruct S;\r\nimpl S {\r\n    /// M.\r\n    fn m(&self) -> u8 {\r\n        1\r\n    }\r\n}\r\n";
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/a.rs"), src).unwrap();
        let q = open(d.path(), "src/a.rs", "S::m").unwrap();
        assert_eq!(q.lines, [4, 7]);
        assert!(q.original.starts_with("    /// M."));
        save(
            d.path(),
            &q,
            "    /// M.\n    fn m(&self) -> u8 {\n        2\n    }",
        )
        .unwrap();
        let after = std::fs::read_to_string(d.path().join("src/a.rs")).unwrap();
        assert!(after.starts_with("// head\r\nstruct S;\r\nimpl S {\r\n"));
        assert!(after.contains("        2\n    }\r\n}\r\n"));
        let e = save(d.path(), &q, "x").unwrap_err();
        assert!(e.contains("changed since"), "{e}");
        assert_eq!(
            std::fs::read_to_string(d.path().join("src/a.rs")).unwrap(),
            after
        );
        assert!(open(d.path(), "src/a.rs", "S::zz").is_err());
        assert_eq!(splice_lines("a\nb\n", [3, 3], "", "x"), None);
    }

    /// Methodology: the template goes above the doc comment at the item's
    /// indentation. Pass: as written.
    #[test]
    fn deviation_template_is_indented() {
        let t = with_deviation_template("    /// M.\n    fn m() {}");
        assert!(t.starts_with("    /// **Deviation from upstream:**"));
        assert!(t.ends_with("    ///\n    /// M.\n    fn m() {}"));
    }
}
