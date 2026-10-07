// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: turndown (MIT, https://github.com/mixmark-io/turndown,
//   lib/turndown.browser.umd.js) and turndown-plugin-gfm (MIT,
//   https://github.com/mixmark-io/turndown-plugin-gfm), as bundled in Zotero
//   translators' "Note Markdown.js" (commit 3d1c78530f42) :233-1372;
//   `collapseWhitespace` adapted there from collapse-whitespace by Luc
//   Thevenard (MIT) :826-951.
// Copyright (c) 2017+ Dom Christie (turndown, turndown-plugin-gfm, MIT);
//   Copyright (c) 2014 Luc Thevenard (collapse-whitespace, MIT).
// Licence: AGPL-3.0 (this port; MIT permits it).

//! Turndown (HTML to Markdown) with the gfm plugin and Note Markdown's
//! options, escapes and list-item rule, over the HTML DOM of
//! [`crate::zotero::framework::html_dom`].
//!
//! Only the configuration Note Markdown uses is ported: `headingStyle: atx`,
//! `bulletListMarker: '-'`, `emDelimiter: '*'`, `codeBlockStyle: fenced`,
//! the other options at turndown's defaults (`hr: '* * *'`,
//! `strongDelimiter: '**'`, `linkStyle: inlined`, `br: '  '`,
//! `preformattedCode: false`). Rules are tried in the order the bundle
//! builds them (each `addRule` puts its rule first).

use crate::zotero::framework::html_dom::{html_node_name, is_html_element, outer_html};
use crate::zotero::framework::js;
use crate::zotero::framework::xml::{NodeId, NodeKind, XmlDocument};
use crate::zotero::framework::xpath::{js_number_to_string, js_to_number};
use crate::zotero::framework::TranslateError;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).expect("static regex"))
}

const BLOCK: [&str; 47] = [
    "ADDRESS",
    "ARTICLE",
    "ASIDE",
    "AUDIO",
    "BLOCKQUOTE",
    "BODY",
    "CANVAS",
    "CENTER",
    "DD",
    "DIR",
    "DIV",
    "DL",
    "DT",
    "FIELDSET",
    "FIGCAPTION",
    "FIGURE",
    "FOOTER",
    "FORM",
    "FRAMESET",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "HEADER",
    "HGROUP",
    "HR",
    "HTML",
    "ISINDEX",
    "LI",
    "MAIN",
    "MENU",
    "NAV",
    "NOFRAMES",
    "NOSCRIPT",
    "OL",
    "OUTPUT",
    "P",
    "PRE",
    "SECTION",
    "TABLE",
    "TBODY",
    "TD",
    "TFOOT",
    "TH",
    "THEAD",
];
// "TR", "UL" complete the list (split for the array length above).
const BLOCK_MORE: [&str; 2] = ["TR", "UL"];

const VOID: [&str; 16] = [
    "AREA", "BASE", "BR", "COL", "COMMAND", "EMBED", "HR", "IMG", "INPUT", "KEYGEN", "LINK",
    "META", "PARAM", "SOURCE", "TRACK", "WBR",
];

const MEANINGFUL_WHEN_BLANK: [&str; 11] = [
    "A", "TABLE", "THEAD", "TBODY", "TFOOT", "TH", "TD", "IFRAME", "SCRIPT", "AUDIO", "VIDEO",
];

type R<T> = Result<T, TranslateError>;

/// A turndown run over one document.
pub struct Turndown {
    /// The document (owned for the run: turndown works on a clone of the
    /// element, which lives in it).
    pub doc: XmlDocument,
    /// `node.isCode`, set as nodes are visited.
    is_code: HashMap<NodeId, bool>,
}

fn name(doc: &XmlDocument, n: NodeId) -> String {
    html_node_name(doc, n)
}

fn is_in(doc: &XmlDocument, n: NodeId, list: &[&str]) -> bool {
    let nm = name(doc, n);
    list.contains(&nm.as_str())
}

fn is_block(doc: &XmlDocument, n: NodeId) -> bool {
    is_in(doc, n, &BLOCK) || is_in(doc, n, &BLOCK_MORE)
}

fn is_void(doc: &XmlDocument, n: NodeId) -> bool {
    is_in(doc, n, &VOID)
}

/// `node.getElementsByTagName(tag).length` (HTML document: HTML elements by
/// lowercased name), for each tag: `has`.
fn has(doc: &XmlDocument, n: NodeId, list: &[&str]) -> bool {
    if !matches!(doc.kind(n), NodeKind::Element(_) | NodeKind::Document) {
        return false;
    }
    let descendants = doc.descendants(n);
    list.iter().any(|tag| {
        let lower = tag.to_ascii_lowercase();
        descendants.iter().any(|&d| {
            doc.element(d).is_some_and(|e| {
                if is_html_element(doc, d) {
                    e.qualified_name() == lower
                } else {
                    e.qualified_name() == *tag
                }
            })
        })
    })
}

fn text_content(doc: &XmlDocument, n: NodeId) -> String {
    doc.text(n)
}

fn data(doc: &XmlDocument, n: NodeId) -> String {
    match doc.kind(n) {
        NodeKind::Text(t) | NodeKind::CData(t) => t.clone(),
        _ => String::new(),
    }
}

/// `/^\s*$/i.test(s)` (JavaScript `\s`).
fn all_space(s: &str) -> bool {
    s.chars().all(js::is_space)
}

/// `edgeWhitespace` (:1071-1081).
struct Edges {
    leading: String,
    leading_ascii: String,
    leading_non_ascii: String,
    trailing: String,
    trailing_ascii: String,
    trailing_non_ascii: String,
}

fn is_ascii_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn edge_whitespace(s: &str) -> Edges {
    let c: Vec<char> = s.chars().collect();
    let la = c.iter().take_while(|&&x| is_ascii_ws(x)).count();
    let l = c.iter().take_while(|&&x| js::is_space(x)).count();
    let rest = &c[l..];
    let t = rest.iter().rev().take_while(|&&x| js::is_space(x)).count();
    let tail = &rest[rest.len() - t..];
    let ta = tail.iter().rev().take_while(|&&x| is_ascii_ws(x)).count();
    let s = |v: &[char]| v.iter().collect::<String>();
    Edges {
        leading: s(&c[..l]),
        leading_ascii: s(&c[..la]),
        leading_non_ascii: s(&c[la..l]),
        trailing: s(tail),
        trailing_non_ascii: s(&tail[..t - ta]),
        trailing_ascii: s(&tail[t - ta..]),
    }
}

/// Note Markdown's `TurndownService.prototype.escape` (:1414-1440).
pub fn escape(s: &str) -> String {
    static R1: OnceLock<Regex> = OnceLock::new();
    static R2: OnceLock<Regex> = OnceLock::new();
    static R3: OnceLock<Regex> = OnceLock::new();
    let mut out = s.replace('*', "\\*");
    if let Some(r) = out.strip_prefix('-') {
        out = format!("\\-{r}");
    }
    if let Some(r) = out.strip_prefix("+ ") {
        out = format!("\\+ {r}");
    }
    out = re(&R1, "^(=+)").replace(&out, "\\$1").into_owned();
    out = re(&R2, "^(#{1,6}) ").replace(&out, "\\$1 ").into_owned();
    out = out.replace('`', "\\`");
    if let Some(r) = out.strip_prefix("~~~") {
        out = format!("\\~~~{r}");
    }
    if let Some(r) = out.strip_prefix('>') {
        out = format!("\\>{r}");
    }
    out = re(&R3, r"^([0-9]+)\. ")
        .replace(&out, "$1\\. ")
        .into_owned();
    out.replace("\\`\\`\\`", "```")
}

/// `join` (:1338-1345).
fn join(output: &str, replacement: &str) -> String {
    let s1 = output.trim_end_matches('\n');
    let s2 = replacement.trim_start_matches('\n');
    let nls = (output.len() - s1.len())
        .max(replacement.len() - s2.len())
        .min(2);
    format!("{s1}{}{s2}", &"\n\n"[..nls])
}

/// `cleanAttribute`: runs of newlines (and following whitespace) as one
/// newline.
fn clean_attribute(a: Option<&str>) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    match a {
        Some(s) if !s.is_empty() => re(&RE, &format!(r"(\n+{}*)+", js::WS))
            .replace_all(s, "\n")
            .into_owned(),
        _ => String::new(),
    }
}

impl Turndown {
    /// A run over `doc`.
    pub fn new(doc: XmlDocument) -> Self {
        Turndown {
            doc,
            is_code: HashMap::new(),
        }
    }

    /// `turndownService.turndown(element)` (:1186-1197): a deep clone of
    /// the element, whitespace collapsed, converted, post-processed.
    pub fn turndown(&mut self, input: NodeId) -> R<String> {
        let root = self.doc.clone_subtree(input);
        self.collapse_whitespace(root);
        let out = self.process(root)?;
        // postProcess: referenceLink's `append` adds nothing (inline links).
        let out = join(&out, "");
        let out = out.trim_start_matches(['\t', '\r', '\n']);
        Ok(out
            .trim_end_matches(|c: char| c == '\t' || c == '\r' || c == '\n' || js::is_space(c))
            .to_owned())
    }

    fn is_pre(&self, n: NodeId) -> bool {
        name(&self.doc, n) == "PRE"
    }

    /// `next(prev, current, isPre)` (:937-943).
    fn next(&self, prev: Option<NodeId>, current: NodeId) -> Option<NodeId> {
        let d = &self.doc;
        if prev.is_some_and(|p| d.parent(p) == Some(current)) || self.is_pre(current) {
            return d.next_sibling(current).or_else(|| d.parent(current));
        }
        d.first_child(current)
            .or_else(|| d.next_sibling(current))
            .or_else(|| d.parent(current))
    }

    /// `remove(node)` (:921-927).
    fn remove(&mut self, n: NodeId) -> Option<NodeId> {
        let next = self.doc.next_sibling(n).or_else(|| self.doc.parent(n));
        self.doc.detach(n);
        next
    }

    /// `collapseWhitespace` (:851-913).
    fn collapse_whitespace(&mut self, element: NodeId) {
        if self.doc.first_child(element).is_none() || self.is_pre(element) {
            return;
        }
        static WSRUN: OnceLock<Regex> = OnceLock::new();
        let wsrun = re(&WSRUN, "[ \r\n\t]+");
        let mut prev_text: Option<NodeId> = None;
        let mut keep_leading_ws = false;
        let mut prev: Option<NodeId> = None;
        let mut node = self.next(prev, element);
        while let Some(n) = node {
            if n == element {
                break;
            }
            match self.doc.kind(n) {
                NodeKind::Text(_) | NodeKind::CData(_) => {
                    let mut text = wsrun.replace_all(&data(&self.doc, n), " ").into_owned();
                    let prev_ends_space =
                        prev_text.is_none_or(|p| data(&self.doc, p).ends_with(' '));
                    if prev_ends_space && !keep_leading_ws && text.starts_with(' ') {
                        text.remove(0);
                    }
                    if text.is_empty() {
                        node = self.remove(n);
                        continue;
                    }
                    self.doc.set_text_data(n, &text);
                    prev_text = Some(n);
                }
                NodeKind::Element(_) => {
                    if is_block(&self.doc, n) || name(&self.doc, n) == "BR" {
                        if let Some(p) = prev_text {
                            let d = data(&self.doc, p);
                            let t = d.strip_suffix(' ').unwrap_or(&d).to_owned();
                            self.doc.set_text_data(p, &t);
                        }
                        prev_text = None;
                        keep_leading_ws = false;
                    } else if is_void(&self.doc, n) || self.is_pre(n) {
                        prev_text = None;
                        keep_leading_ws = true;
                    } else if prev_text.is_some() {
                        keep_leading_ws = false;
                    }
                }
                _ => {
                    node = self.remove(n);
                    continue;
                }
            }
            let next_node = self.next(prev, n);
            prev = Some(n);
            node = next_node;
        }
        if let Some(p) = prev_text {
            let d = data(&self.doc, p);
            let t = d.strip_suffix(' ').unwrap_or(&d).to_owned();
            self.doc.set_text_data(p, &t);
            if t.is_empty() {
                self.remove(p);
            }
        }
    }

    /// `isBlank` (:1053-1061).
    fn is_blank(&self, n: NodeId) -> bool {
        let d = &self.doc;
        !is_void(d, n)
            && !is_in(d, n, &MEANINGFUL_WHEN_BLANK)
            && all_space(&text_content(d, n))
            && !has(d, n, &VOID)
            && !has(d, n, &MEANINGFUL_WHEN_BLANK)
    }

    /// `isFlankedByWhitespace` (:1083-1105).
    fn is_flanked(&self, left: bool, n: NodeId) -> bool {
        let d = &self.doc;
        let sibling = if left {
            d.previous_sibling(n)
        } else {
            d.next_sibling(n)
        };
        let test = |s: &str| {
            if left {
                s.ends_with(' ')
            } else {
                s.starts_with(' ')
            }
        };
        match sibling {
            Some(s) => match d.kind(s) {
                NodeKind::Text(t) => test(t),
                NodeKind::Element(_) if !is_block(d, s) => test(&text_content(d, s)),
                _ => false,
            },
            None => false,
        }
    }

    /// `flankingWhitespace` (:1063-1079): (leading, trailing).
    fn flanking(&self, n: NodeId) -> (String, String) {
        if is_block(&self.doc, n) {
            return (String::new(), String::new());
        }
        let e = edge_whitespace(&text_content(&self.doc, n));
        let mut leading = e.leading;
        let mut trailing = e.trailing;
        if !e.leading_ascii.is_empty() && self.is_flanked(true, n) {
            leading = e.leading_non_ascii;
        }
        if !e.trailing_ascii.is_empty() && self.is_flanked(false, n) {
            trailing = e.trailing_non_ascii;
        }
        (leading, trailing)
    }

    /// `process` (:1292-1307).
    fn process(&mut self, parent: NodeId) -> R<String> {
        let mut output = String::new();
        for c in self.doc.children(parent).to_vec() {
            // `new Node(node)`: isCode.
            let parent_code = self.is_code.get(&parent).copied().unwrap_or(false);
            let code = name(&self.doc, c) == "CODE" || parent_code;
            self.is_code.insert(c, code);
            let replacement = match self.doc.kind(c) {
                NodeKind::Text(t) => {
                    if code {
                        t.clone()
                    } else {
                        escape(t)
                    }
                }
                NodeKind::Element(_) => self.replacement_for_node(c)?,
                _ => String::new(),
            };
            output = join(&output, &replacement);
        }
        Ok(output)
    }

    /// `replacementForNode` (:1321-1330).
    fn replacement_for_node(&mut self, n: NodeId) -> R<String> {
        let blank = self.is_blank(n);
        let (leading, trailing) = self.flanking(n);
        let mut content = self.process(n)?;
        if !leading.is_empty() || !trailing.is_empty() {
            content = js::trim(&content).to_owned();
        }
        let r = if blank {
            if is_block(&self.doc, n) {
                "\n\n".to_owned()
            } else {
                String::new()
            }
        } else {
            self.rule(n, &content)?
        };
        Ok(format!("{leading}{r}{trailing}"))
    }

    /// The first rule whose filter accepts the node, then keep, then the
    /// default (`Rules.forNode`), applied.
    fn rule(&self, n: NodeId, content: &str) -> R<String> {
        let d = &self.doc;
        let nm = name(d, n);
        let lower = nm.to_ascii_lowercase();
        let block = is_block(d, n);
        // Note Markdown's listItem (:1443-1459).
        if lower == "li" {
            return Ok(self.list_item(n, content));
        }
        // gfm taskListItems.
        if nm == "INPUT"
            && d.get_attribute(n, "type")
                .is_some_and(|t| t.eq_ignore_ascii_case("checkbox"))
            && d.parent(n).is_some_and(|p| name(d, p) == "LI")
        {
            let checked = d.has_attribute(n, "checked");
            return Ok(format!("{} ", if checked { "[x]" } else { "[ ]" }));
        }
        // gfm tableSection.
        if matches!(lower.as_str(), "thead" | "tbody" | "tfoot") {
            return Ok(content.to_owned());
        }
        // gfm table (a table whose first row is a heading row).
        if nm == "TABLE" && self.is_heading_row(self.first_row(n)?)? {
            let c = content.replacen("\n\n", "\n", 1);
            return Ok(format!("\n\n{c}\n\n"));
        }
        // gfm tableRow.
        if lower == "tr" {
            let mut border = String::new();
            if self.is_heading_row(n)? {
                for c in d.children(n).iter().copied() {
                    let align = d.get_attribute(c, "align").unwrap_or("").to_lowercase();
                    let b = match align.as_str() {
                        "left" => ":--",
                        "right" => "--:",
                        "center" => ":-:",
                        _ => "---",
                    };
                    border.push_str(&self.cell(b, c));
                }
            }
            return Ok(if border.is_empty() {
                format!("\n{content}")
            } else {
                format!("\n{content}\n{border}")
            });
        }
        // gfm tableCell.
        if lower == "th" || lower == "td" {
            return Ok(self.cell(content, n));
        }
        // gfm strikethrough.
        if matches!(lower.as_str(), "del" | "s" | "strike") {
            return Ok(format!("~{content}~"));
        }
        // gfm highlightedCodeBlock.
        static HL: OnceLock<Regex> = OnceLock::new();
        let hl = re(&HL, "highlight-(?:text|source)-([a-z0-9]+)");
        if nm == "DIV" {
            let class = d.get_attribute(n, "class").unwrap_or("");
            if let (true, Some(fc)) = (hl.is_match(class), d.first_child(n)) {
                if name(d, fc) == "PRE" {
                    let lang = hl
                        .captures(class)
                        .and_then(|c| c.get(1))
                        .map_or("", |m| m.as_str());
                    return Ok(format!("\n\n```{lang}\n{}\n```\n\n", text_content(d, fc)));
                }
            }
        }
        match lower.as_str() {
            "p" => return Ok(format!("\n\n{content}\n\n")),
            "br" => return Ok("  \n".to_owned()),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level: usize = lower[1..].parse().unwrap_or(1);
                return Ok(format!("\n\n{} {content}\n\n", "#".repeat(level)));
            }
            "blockquote" => {
                let c = content.trim_matches('\n');
                // `replace(/^/gm, '> ')`: before every line (JavaScript line
                // terminators).
                let mut q = String::from("> ");
                for ch in c.chars() {
                    q.push(ch);
                    if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                        q.push_str("> ");
                    }
                }
                return Ok(format!("\n\n{q}\n\n"));
            }
            "ul" | "ol" => {
                let p = d.parent(n);
                let last_el = p.and_then(|p| d.element_children(p).last().copied());
                return Ok(
                    if p.is_some_and(|p| name(d, p) == "LI") && last_el == Some(n) {
                        format!("\n{content}")
                    } else {
                        format!("\n\n{content}\n\n")
                    },
                );
            }
            _ => {}
        }
        // fencedCodeBlock.
        if nm == "PRE" {
            if let Some(fc) = d.first_child(n).filter(|&f| name(d, f) == "CODE") {
                let class = d.get_attribute(fc, "class").unwrap_or("").to_owned();
                static LANG: OnceLock<Regex> = OnceLock::new();
                let lang = re(&LANG, r"language-(\S+)")
                    .captures(&class)
                    .and_then(|c| c.get(1))
                    .map_or(String::new(), |m| m.as_str().to_owned());
                let code = text_content(d, fc);
                let mut size = 3;
                static FENCE: OnceLock<Regex> = OnceLock::new();
                for m in re(&FENCE, "(?m)^`{3,}").find_iter(&code) {
                    if m.as_str().len() >= size {
                        size = m.as_str().len() + 1;
                    }
                }
                let fence = "`".repeat(size);
                let code = code.strip_suffix('\n').unwrap_or(&code);
                return Ok(format!("\n\n{fence}{lang}\n{code}\n{fence}\n\n"));
            }
        }
        if lower == "hr" {
            return Ok("\n\n* * *\n\n".to_owned());
        }
        // inlineLink.
        if nm == "A" {
            if let Some(href) = d.get_attribute(n, "href").filter(|h| !h.is_empty()) {
                let mut title = clean_attribute(d.get_attribute(n, "title"));
                if !title.is_empty() {
                    title = format!(" \"{title}\"");
                }
                return Ok(format!("[{content}]({href}{title})"));
            }
        }
        if lower == "em" || lower == "i" {
            return Ok(if js::trim(content).is_empty() {
                String::new()
            } else {
                format!("*{content}*")
            });
        }
        if lower == "strong" || lower == "b" {
            return Ok(if js::trim(content).is_empty() {
                String::new()
            } else {
                format!("**{content}**")
            });
        }
        if nm == "CODE" {
            let has_siblings = d.previous_sibling(n).is_some() || d.next_sibling(n).is_some();
            let is_code_block = d.parent(n).is_some_and(|p| name(d, p) == "PRE") && !has_siblings;
            if !is_code_block {
                return Ok(code_span(content));
            }
        }
        if lower == "img" {
            let alt = clean_attribute(d.get_attribute(n, "alt"));
            let src = d.get_attribute(n, "src").unwrap_or("");
            let title = clean_attribute(d.get_attribute(n, "title"));
            let title_part = if title.is_empty() {
                String::new()
            } else {
                format!(" \"{title}\"")
            };
            return Ok(if src.is_empty() {
                String::new()
            } else {
                format!("![{alt}]({src}{title_part})")
            });
        }
        // gfm keep: a table without a heading row, as HTML.
        if nm == "TABLE" && !self.is_heading_row(self.first_row(n)?)? {
            let h = outer_html(d, n);
            return Ok(if block { format!("\n\n{h}\n\n") } else { h });
        }
        // defaultReplacement.
        Ok(if block {
            format!("\n\n{content}\n\n")
        } else {
            content.to_owned()
        })
    }

    /// `table.rows[0]`: thead rows, then rows of tbodies and direct tr
    /// children in tree order, then tfoot rows. An error where upstream
    /// throws (`isHeadingRow(undefined)`).
    fn first_row(&self, table: NodeId) -> R<NodeId> {
        let d = &self.doc;
        let rows_of = |s: NodeId| -> Vec<NodeId> {
            d.element_children(s)
                .into_iter()
                .filter(|&r| name(d, r) == "TR")
                .collect()
        };
        let kids = d.element_children(table);
        let mut rows = Vec::new();
        for &k in &kids {
            if name(d, k) == "THEAD" {
                rows.extend(rows_of(k));
            }
        }
        for &k in &kids {
            match name(d, k).as_str() {
                "TBODY" => rows.extend(rows_of(k)),
                "TR" => rows.push(k),
                _ => {}
            }
        }
        for &k in &kids {
            if name(d, k) == "TFOOT" {
                rows.extend(rows_of(k));
            }
        }
        rows.first().copied().ok_or_else(|| {
            TranslateError::Translator(
                "Cannot read properties of undefined (reading 'parentNode')".into(),
            )
        })
    }

    /// gfm `isHeadingRow`.
    fn is_heading_row(&self, tr: NodeId) -> R<bool> {
        let d = &self.doc;
        let Some(parent) = d.parent(tr) else {
            return Ok(false);
        };
        let pn = name(d, parent);
        if pn == "THEAD" {
            return Ok(true);
        }
        Ok(d.first_child(parent) == Some(tr)
            && (pn == "TABLE" || self.is_first_tbody(parent))
            && d.children(tr).iter().all(|&c| name(d, c) == "TH"))
    }

    fn is_first_tbody(&self, el: NodeId) -> bool {
        let d = &self.doc;
        name(d, el) == "TBODY"
            && match d.previous_sibling(el) {
                None => true,
                Some(p) => name(d, p) == "THEAD" && all_space(&text_content(d, p)),
            }
    }

    /// gfm `cell`.
    fn cell(&self, content: &str, n: NodeId) -> String {
        let d = &self.doc;
        let index = d
            .parent(n)
            .and_then(|p| d.children(p).iter().position(|&c| c == n));
        let prefix = if index == Some(0) { "| " } else { " " };
        format!("{prefix}{content} |")
    }

    /// Note Markdown's `listItem` rule (:1443-1459).
    fn list_item(&self, n: NodeId, content: &str) -> String {
        let d = &self.doc;
        let c = content.trim_start_matches('\n');
        let trimmed = c.trim_end_matches('\n');
        let c = if trimmed.len() < c.len() {
            format!("{trimmed}\n")
        } else {
            c.to_owned()
        };
        let c = c.replace('\n', "\n    ");
        let mut prefix = "- ".to_owned();
        if let Some(parent) = d.parent(n) {
            if name(d, parent) == "OL" {
                let index = d
                    .element_children(parent)
                    .iter()
                    .position(|&x| x == n)
                    .map_or(-1.0, |i| i as f64);
                let start = d.get_attribute(parent, "start").filter(|s| !s.is_empty());
                let num = match start {
                    Some(s) => js_to_number(s) + index,
                    None => index + 1.0,
                };
                prefix = format!("{}. ", js_number_to_string(num));
            }
        }
        let nl = if d.next_sibling(n).is_some() && !c.ends_with('\n') {
            "\n"
        } else {
            ""
        };
        format!("{prefix}{c}{nl}")
    }
}

/// The `code` rule's replacement.
fn code_span(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }
    static NL: OnceLock<Regex> = OnceLock::new();
    let content = re(&NL, "\r?\n|\r").replace_all(content, " ").into_owned();
    static EXTRA: OnceLock<Regex> = OnceLock::new();
    let extra = if re(&EXTRA, r"^`|^ .*?[^ ].* $|`$").is_match(&content) {
        " "
    } else {
        ""
    };
    static TICKS: OnceLock<Regex> = OnceLock::new();
    let runs: Vec<&str> = re(&TICKS, "`+")
        .find_iter(&content)
        .map(|m| m.as_str())
        .collect();
    let mut delimiter = "`".to_owned();
    while runs.contains(&delimiter.as_str()) {
        delimiter.push('`');
    }
    format!("{delimiter}{extra}{content}{extra}{delimiter}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_joins_like_turndown() {
        assert_eq!(escape("- a * b `c`"), "\\- a \\* b \\`c\\`");
        assert_eq!(escape("12. x"), "12\\. x");
        assert_eq!(escape("```"), "```");
        assert_eq!(join("a\n", "\n\nb"), "a\n\nb");
        assert_eq!(join("a", "b"), "ab");
        assert_eq!(code_span("a`b"), "``a`b``");
        let e = edge_whitespace(" \u{A0}x \u{A0} ");
        assert_eq!(
            (e.leading.as_str(), e.trailing.as_str()),
            (" \u{A0}", " \u{A0} ")
        );
        assert_eq!(e.trailing_ascii, " ");
    }
}
