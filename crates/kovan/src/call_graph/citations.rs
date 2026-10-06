//! **Docs that cite a function** (GitHub #746, item 7): which Markdown pages
//! reference it, read from the pages, plus `kovan-concept:` tags in source.
//!
//! # Sources
//!
//! - **`code-walk` blocks** in lessons, deep dives and tutorials
//!   (`<!-- code-walk: from=… to=… -->` … `<!-- /code-walk -->`, see
//!   `commands::code_walk::lesson`). **Every function on the walk** is
//!   recorded, not only the endpoints: the generated body lists each hop as
//!   `<!-- snippet-check: <file>:<line> fn <name> -->`, which names the
//!   function by file and declaration line; the header's `from=`, `to=` and
//!   each `hand:` hop name functions in code-walk path form. A block whose
//!   body has not been generated yet still contributes its header's ids.
//! - **`kovan-concept:` tags** in source: `//! kovan-concept: <path>` in a
//!   module's doc comment tags the module ([`super::Module::concepts`]);
//!   `/// kovan-concept: <path>` in a function's doc comment tags the
//!   function, recorded as a citation of kind `concept_tag`. **Count found
//!   2026-10-06: 0** (the tags are proposed in
//!   `crates/kovan-literature/docs/concept-proposals.md`, none added yet).
//! - **Reserved:** kovan Markdown artifacts' `[[relation]] to = "code:…"`
//!   links (#743), kind `relation`. Not read yet; the slot exists so the
//!   reader need not change when they are.
//!
//! # Anchor and site link
//!
//! `anchor` is the id mdBook gives the nearest heading above the block:
//! the heading's text with inline code, emphasis and link targets removed,
//! lower-cased, whitespace to `-`, keeping letters, digits, `_` and `-`
//! (mdBook's `normalize_id`); an explicit `{#id}` wins. mdBook's `-1`
//! suffix for a repeated heading is not reproduced. A page inside a book
//! listed in `docs/site/deep-dives.txt` or `docs/site/tutorials.txt` gets
//! `site`, its path on the Pages site (`deep-dives/<name>/<page>.html#<anchor>`,
//! `README.md` → `index.html`), relative to [`SITE_BASE`].

use serde::{Deserialize, Serialize};

/// The workspace's GitHub Pages site (`.github/workflows/pages.yml`).
pub const SITE_BASE: &str = "https://theodoreonzgit.github.io/outram-park-backend/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationKind {
    CodeWalk,
    ConceptTag,
    /// Reserved for #743's `[[relation]] to = "code:…"` links; not emitted.
    Relation,
}

/// One reference to a function.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Citation {
    pub kind: CitationKind,
    /// The citing page, workspace-relative; for `concept_tag`, the concept
    /// path the tag names.
    pub page: String,
    /// 1-based line of the block (or tag).
    pub line: u32,
    /// The heading id above the block; empty for a tag or a page without
    /// headings.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub anchor: String,
    /// Path on the Pages site, relative to [`SITE_BASE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
}

/// How a page names a function.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FnRef {
    /// `snippet-check`: workspace-relative file, 1-based line, name.
    At {
        file: String,
        line: u32,
        name: String,
    },
    /// Code-walk path form, `file.rs::name` or `file.rs::Type::name`.
    Path(String),
}

/// One code-walk block's functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRefs {
    /// 1-based line of the opening comment.
    pub line: u32,
    pub anchor: String,
    pub refs: Vec<FnRef>,
}

/// mdBook's heading id for `text`.
pub fn heading_id(text: &str) -> String {
    let t = text.trim();
    if let Some(open) = t.rfind("{#") {
        if t.ends_with('}') {
            return t[open + 2..t.len() - 1].to_string();
        }
    }
    // Drop link targets: `[a](b)` -> `a`.
    let mut plain = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ']' && chars.peek() == Some(&'(') {
            for d in chars.by_ref() {
                if d == ')' {
                    break;
                }
            }
            continue;
        }
        plain.push(c);
    }
    plain
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c.to_lowercase().next().unwrap_or(c))
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn comment_body<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
    let t = line.trim();
    let rest = t.strip_prefix("<!--")?.trim_start().strip_prefix(tag)?;
    Some(rest.trim().trim_end_matches("-->").trim())
}

/// The code-walk blocks of one Markdown page.
pub fn scan_page(text: &str) -> Vec<PageRefs> {
    let mut out: Vec<PageRefs> = Vec::new();
    let mut anchor = String::new();
    let mut in_fence = false;
    let mut cur: Option<PageRefs> = None;
    let mut in_header = false;
    for (k, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if in_header {
            // The opening comment's `hand:` lines, up to `-->`.
            if let Some(h) = t.strip_prefix("hand:") {
                let hop = h.split('|').next().unwrap_or("");
                if let Some(c) = cur.as_mut() {
                    for id in hop.split("->").map(str::trim).filter(|s| !s.is_empty()) {
                        c.refs.push(FnRef::Path(id.to_string()));
                    }
                }
            }
            if t.contains("-->") {
                in_header = false;
            }
            continue;
        }
        if t.starts_with('#') && cur.is_none() {
            let h = t.trim_start_matches('#');
            if h.starts_with(' ') || h.is_empty() {
                anchor = heading_id(h);
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("<!-- code-walk:") {
            if let Some(done) = cur.take() {
                out.push(done);
            }
            let first = rest.split("-->").next().unwrap_or("");
            let mut refs = Vec::new();
            for tok in first.split_whitespace() {
                if let Some(v) = tok
                    .strip_prefix("from=")
                    .or_else(|| tok.strip_prefix("to="))
                {
                    refs.push(FnRef::Path(v.to_string()));
                }
            }
            in_header = !rest.contains("-->");
            cur = Some(PageRefs {
                line: k as u32 + 1,
                anchor: anchor.clone(),
                refs,
            });
            continue;
        }
        if t.starts_with("<!-- /code-walk") {
            if let Some(done) = cur.take() {
                out.push(done);
            }
            continue;
        }
        if let (Some(c), Some(body)) = (cur.as_mut(), comment_body(t, "snippet-check:")) {
            // `<file>:<line> fn <name>`; other snippet checks name call sites.
            let mut parts = body.split_whitespace();
            let (Some(loc), Some("fn"), Some(name)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if let Some((file, l)) = loc.rsplit_once(':') {
                if let Ok(l) = l.parse() {
                    c.refs.push(FnRef::At {
                        file: file.to_string(),
                        line: l,
                        name: name.to_string(),
                    });
                }
            }
        }
    }
    if let Some(done) = cur.take() {
        out.push(done);
    }
    for b in &mut out {
        b.refs.sort();
        b.refs.dedup();
    }
    out
}

/// The `kovan-concept:` tags in a source file: `(0-based line, doc kind
/// '!' for `//!` or '/' for `///`, concept path)`.
pub fn concept_tags(lines: &[String]) -> Vec<(u32, char, String)> {
    let mut out = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        let (kind, rest) = if let Some(r) = t.strip_prefix("//!") {
            ('!', r)
        } else if let Some(r) = t.strip_prefix("///") {
            ('/', r)
        } else {
            continue;
        };
        if let Some(p) = rest.trim().strip_prefix("kovan-concept:") {
            let p = p.trim();
            if !p.is_empty() {
                out.push((k as u32, kind, p.to_string()));
            }
        }
    }
    out
}

/// The site path of a page, given the books as `(prefix, book dir)` pairs
/// (`("deep-dives/triso-atops", "crates/boon-lay/docs/lessons")`).
pub fn site_path(books: &[(String, String)], page: &str, anchor: &str) -> Option<String> {
    for (prefix, dir) in books {
        let Some(rel) = page.strip_prefix(&format!("{dir}/src/")) else {
            continue;
        };
        let rel = rel.strip_suffix(".md")?;
        let rel = match rel.strip_suffix("README") {
            Some(d) => format!("{d}index"),
            None => rel.to_string(),
        };
        let hash = if anchor.is_empty() {
            String::new()
        } else {
            format!("#{anchor}")
        };
        return Some(format!("{prefix}/{rel}.html{hash}"));
    }
    None
}

/// The books of a `docs/site/*.txt` list (`<url-name> <dir>` lines, `#`
/// comments), each under `prefix/`.
pub fn parse_books(text: &str, prefix: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((
                format!("{prefix}/{}", it.next()?),
                it.next()?.trim_end_matches('/').to_string(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "# Title\n\n## Running `step` (the [pools](x.md))\n\nText.\n\n<!-- code-walk: from=crates/a/src/x.rs::run to=crates/a/src/y.rs::P::step\nhand: crates/a/src/x.rs::run -> crates/a/src/z.rs::g | trait dispatch\n-->\n<!-- generated -->\n\n## Not a heading inside\n<!-- snippet-check: crates/a/src/x.rs:10 fn run -->\n<!-- snippet-check: crates/a/src/x.rs:12 call_site -->\n```rust\n# not a heading\n<!-- snippet-check: crates/a/src/q.rs:1 fn hidden -->\n```\n<!-- snippet-check: crates/a/src/y.rs:40 fn step -->\n<!-- /code-walk -->\n\n<!-- snippet-check: crates/a/src/w.rs:3 fn outside -->\n";

    /// Methodology: a page with one block: header ids (`from`, `to`, the
    /// hand hop's two ends), the hops' `fn` snippet checks, a call-site
    /// snippet check (ignored), one inside a code fence (ignored) and one
    /// after the block (ignored); the anchor is the heading above.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn code_walk_block_lists_every_hop() {
        let b = scan_page(PAGE);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].line, 7);
        assert_eq!(b[0].anchor, "running-step-the-pools");
        let at: Vec<&FnRef> = b[0]
            .refs
            .iter()
            .filter(|r| matches!(r, FnRef::At { .. }))
            .collect();
        assert_eq!(
            at,
            vec![
                &FnRef::At {
                    file: "crates/a/src/x.rs".into(),
                    line: 10,
                    name: "run".into()
                },
                &FnRef::At {
                    file: "crates/a/src/y.rs".into(),
                    line: 40,
                    name: "step".into()
                },
            ]
        );
        let paths: Vec<&FnRef> = b[0]
            .refs
            .iter()
            .filter(|r| matches!(r, FnRef::Path(_)))
            .collect();
        assert_eq!(paths.len(), 3, "{paths:?}");
    }

    /// Methodology: site paths for a deep-dive page, a README and a page
    /// outside every book; heading ids with an explicit `{#id}`.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn site_paths_and_ids() {
        let books = parse_books(
            "# c\ntriso-atops crates/boon-lay/docs/lessons\n",
            "deep-dives",
        );
        assert_eq!(
            site_path(&books, "crates/boon-lay/docs/lessons/src/sub/a.md", "h"),
            Some("deep-dives/triso-atops/sub/a.html#h".into())
        );
        assert_eq!(
            site_path(&books, "crates/boon-lay/docs/lessons/src/README.md", ""),
            Some("deep-dives/triso-atops/index.html".into())
        );
        assert_eq!(site_path(&books, "docs/lessons/x.md", ""), None);
        assert_eq!(heading_id(" Hello *World* {#custom}"), "custom");
        assert_eq!(heading_id(" What is k_eff?"), "what-is-k_eff");
    }

    /// Methodology: module and function concept tags.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn concept_tags_found() {
        let lines: Vec<String> = [
            "//! kovan-concept: a/b",
            "/// kovan-concept: c",
            "// kovan-concept: no",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            concept_tags(&lines),
            vec![(0, '!', "a/b".to_string()), (1, '/', "c".to_string())]
        );
    }
}
