// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js — `zotero2bibtexTypeMap` :219-235,
//   `alwaysMap` :285-295, `writeField` :1074-1098, `mapHTMLmarkup`
//   :1100-1113, `vphantomRe`/`escapeSpecialCharacters` :1167-1186,
//   `mapAccent` :1188-1190, `encodeFilePathComponent` :1192-1204,
//   `doExport` :1340-1573.
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky and Emiliano
//   heyns.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! BibTeX export (`doExport`).

use super::common::{
    build_cite_key, clean_file_path, creator_string, extra_recs_to_string, parse_extra_fields,
    protect_and,
};
use super::mapping_table::TABLE as MAPPING;
use super::{FIELD_MAP, MONTHS, REV_EXTRA_IDS};
use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::js;
use crate::zotero::framework::utilities::get_creators_for_type;
use crate::zotero::framework::{ExportContext, TranslateError};
use kovan_common::zotero::date::str_to_date;
use regex::{Captures, Regex};
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// JavaScript's `.` without the `s` flag.
pub(crate) const DOT: &str = r"[^\n\r\x{2028}\x{2029}]";

/// `zotero2bibtexTypeMap` (:219-235).
const ZOTERO_TO_BIBTEX_TYPE: [(&str, &str); 15] = [
    ("book", "book"),
    ("bookSection", "incollection"),
    ("journalArticle", "article"),
    ("magazineArticle", "article"),
    ("newspaperArticle", "article"),
    ("thesis", "phdthesis"),
    ("letter", "misc"),
    ("manuscript", "unpublished"),
    ("patent", "patent"),
    ("interview", "misc"),
    ("film", "misc"),
    ("artwork", "misc"),
    ("webpage", "misc"),
    ("conferencePaper", "inproceedings"),
    ("report", "techreport"),
];

/// `caseProtectedFields` (:110-116).
const CASE_PROTECTED: [&str; 5] = ["title", "type", "shorttitle", "booktitle", "series"];

/// `alwaysMap` (:285-295).
fn always_map(c: char) -> &'static str {
    match c {
        '|' => "{\\textbar}",
        '<' => "{\\textless}",
        '>' => "{\\textgreater}",
        '~' => "{\\textasciitilde}",
        '^' => "{\\textasciicircum}",
        '\\' => "{\\textbackslash}",
        '{' => "\\{\\vphantom{\\}}",
        '}' => "\\vphantom{\\{}\\}",
        _ => unreachable!(),
    }
}

const VPH_OPEN: &str = "\\vphantom{\\}}";
const VPH_CLOSE: &str = "\\vphantom{\\{}";

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The first match of `vphantomRe` (:1167),
/// `/\\vphantom{\\}}((?:.(?!\\vphantom{\\}}))*)\\vphantom{\\{}/`, as
/// (start, end, group 1) in chars. Hand-written because of the look-ahead:
/// from each start where the opening token occurs, the group takes
/// characters while the opening token does not follow them (and they are
/// not line terminators), greedily; then the longest prefix of that run
/// followed by the closing token wins, as backtracking would find it.
fn vphantom_match(c: &[char]) -> Option<(usize, usize, String)> {
    let open: Vec<char> = VPH_OPEN.chars().collect();
    let close: Vec<char> = VPH_CLOSE.chars().collect();
    let at = |p: usize, tok: &[char]| c.len() >= p + tok.len() && c[p..p + tok.len()] == *tok;
    for p in 0..c.len() {
        if !at(p, &open) {
            continue;
        }
        let g0 = p + open.len();
        let mut e = g0;
        while e < c.len() && !is_line_terminator(c[e]) && !at(e + 1, &open) {
            e += 1;
        }
        for q in (g0..=e).rev() {
            if at(q, &close) {
                return Some((p, q + close.len(), c[g0..q].iter().collect()));
            }
        }
    }
    None
}

/// `escapeSpecialCharacters(str)` (:1168-1186).
pub(super) fn escape_special_characters(s: &str) -> String {
    static SPECIAL: OnceLock<Regex> = OnceLock::new();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "|<>~^\\{}".contains(c) {
            out.push_str(always_map(c));
        } else {
            out.push(c);
        }
    }
    let mut new_str = re(&SPECIAL, || r"([#$%&_])".into())
        .replace_all(&out, "\\${1}")
        .into_owned();
    if new_str.contains("\\vphantom") {
        loop {
            let c: Vec<char> = new_str.chars().collect();
            let Some((start, end, inner)) = vphantom_match(&c) else {
                break;
            };
            new_str = c[..start].iter().collect::<String>()
                + &inner
                + &c[end..].iter().collect::<String>();
        }
    }
    new_str
}

/// `mapHTMLmarkup(characters)` (:1100-1113).
fn map_html_markup(s: &str) -> String {
    static I: OnceLock<Regex> = OnceLock::new();
    static B: OnceLock<Regex> = OnceLock::new();
    static SUP: OnceLock<Regex> = OnceLock::new();
    static SUB: OnceLock<Regex> = OnceLock::new();
    static SPAN: OnceLock<Regex> = OnceLock::new();
    static SC: OnceLock<Regex> = OnceLock::new();
    let lt = r"\{\\textless\}";
    let gt = r"\{\\textgreater\}";
    let pair = |tag: &str| format!("{lt}{tag}{gt}({DOT}+?){lt}/{tag}{gt}");
    let s = re(&I, || pair("i")).replace_all(s, "\\textit{${1}}");
    let s = re(&B, || pair("b")).replace_all(&s, "\\textbf{${1}}");
    let s = re(&SUP, || pair("sup")).replace_all(&s, "$$^{\\textrm{${1}}}$$");
    let s = re(&SUB, || pair("sub")).replace_all(&s, "$$_{\\textrm{${1}}}$$");
    let s = re(&SPAN, || {
        format!(
            "{lt}span{ws}style=\"small-caps\"{gt}({DOT}+?){lt}/span{gt}",
            ws = js::WS
        )
    })
    .replace_all(&s, "\\textsc{${1}}");
    let s = re(&SC, || pair("sc")).replace_all(&s, "\\textsc{${1}}");
    s.into_owned()
}

/// `mapAccent(character)` (:1188-1190) over `/[\u0080-\uFFFF]/g`: an
/// astral character is two UTF-16 units upstream, neither in the table.
fn map_accents(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let cp = c as u32;
        if cp < 0x80 {
            out.push(c);
        } else if cp > 0xFFFF {
            out.push_str("??");
        } else {
            let k = c.to_string();
            match MAPPING.iter().find(|(m, _)| *m == k) {
                Some((_, v)) if !v.is_empty() => out.push_str(v),
                _ => out.push('?'),
            }
        }
    }
    out
}

/// Which `protectCapsRE` (:1340-1352) is in force.
#[derive(Clone, Copy)]
enum ProtectCaps {
    /// `/(.)\b([\p{L}\d]*\p{Lu}[\p{L}\d]*)|^([\p{L}\d]+\p{Lu}[\p{L}\d]*)/gu`.
    All,
    /// `/()()\b([\p{L}\d]+\p{Lu}[\p{L}\d]*)/gu` (hidden preference
    /// `BibTeX.export.dontProtectInitialCase`).
    NonInitial,
}

fn protect_caps(s: &str, mode: ProtectCaps) -> String {
    static ALL: OnceLock<Regex> = OnceLock::new();
    static NONINIT: OnceLock<Regex> = OnceLock::new();
    // In /u mode \b still uses the ASCII \w; \d is [0-9].
    let r = match mode {
        ProtectCaps::All => re(&ALL, || {
            format!(
                r"({DOT})(?-u:\b)([\p{{L}}0-9]*\p{{Lu}}[\p{{L}}0-9]*)|^([\p{{L}}0-9]+\p{{Lu}}[\p{{L}}0-9]*)"
            )
        }),
        ProtectCaps::NonInitial => re(&NONINIT, || {
            r"()()(?-u:\b)([\p{L}0-9]+\p{Lu}[\p{L}0-9]*)".into()
        }),
    };
    r.replace_all(s, |c: &Captures| {
        let g = |i| c.get(i).map_or("", |m| m.as_str());
        format!("{}{{{}{}}}", g(1), g(2), g(3))
    })
    .into_owned()
}

/// `encodeFilePathComponent(value)` (:1201-1204), escaping `\ : ; $`.
fn encode_file_path_component(v: &str) -> String {
    let mut out = String::new();
    for c in v.chars() {
        if "\\:;$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

struct Writer {
    protect: ProtectCaps,
}

impl Writer {
    /// `writeField(field, value, isMacro)` (:1074-1098); `value` is
    /// already known truthy (callers check `!value`).
    fn write_field(&self, ctx: &mut ExportContext, field: &str, value: &str, is_macro: bool) {
        if value.is_empty() {
            return;
        }
        let mut value = value.to_owned();
        ctx.write(&format!(",\n\t{field} = "));
        if !is_macro {
            ctx.write("{");
        }
        if !is_macro && !matches!(field, "url" | "doi" | "file" | "lccn") {
            value = escape_special_characters(&value);
            if CASE_PROTECTED.contains(&field) {
                value = protect_caps(&value, self.protect);
            }
        }
        let charset = ctx.get_option("exportCharset").map(js::to_js_string);
        if let Some(cs) = charset.filter(|c| !c.is_empty()) {
            if !cs.starts_with("UTF-8") {
                value = map_accents(&value);
            }
        }
        if !matches!(field, "url" | "doi" | "file") {
            value = map_html_markup(&value);
        }
        ctx.write(&value);
        if !is_macro {
            ctx.write("}");
        }
    }
}

/// `doExport` (:1341-1573).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static THESIS: OnceLock<Regex> = OnceLock::new();
    static ACCESS: OnceLock<Regex> = OnceLock::new();
    static PAGES: OnceLock<Regex> = OnceLock::new();
    let protect = if ctx
        .get_hidden_pref("BibTeX.export.dontProtectInitialCase")
        .is_some_and(|v| js::truthy(Some(&v)))
    {
        ProtectCaps::NonInitial
    } else {
        ProtectCaps::All
    };
    let w = Writer { protect };
    ctx.write("\n");
    let mut first = true;
    let mut citekeys: Vec<String> = Vec::new();
    while let Some(mut item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let mut btype = ZOTERO_TO_BIBTEX_TYPE
            .iter()
            .find(|(z, _)| *z == item.item_type)
            .map(|(_, b)| *b);
        if btype == Some("phdthesis") {
            let tt = item.get_str("type").filter(|t| !t.is_empty()).map(|t| {
                re(&THESIS, || {
                    format!("[{}.]+|thesis|unpublished", &js::WS[1..js::WS.len() - 1])
                })
                .replace_all(&t.to_lowercase(), "")
                .into_owned()
            });
            if tt
                .as_deref()
                .is_some_and(|t| ["master", "masters", "master's", "ms", "msc", "ma"].contains(&t))
            {
                btype = Some("mastersthesis");
                item.set("type", "");
            }
        }
        let btype = btype.unwrap_or("misc");

        let mut extra = item
            .get_str("extra")
            .filter(|e| !e.is_empty())
            .map(parse_extra_fields);
        let citekey = build_cite_key(
            &item,
            extra.as_mut(),
            &mut citekeys,
            "BibTeX.export.simpleCitekey",
            ctx,
        );
        ctx.write(&format!(
            "{}@{btype}{{{citekey}",
            if first { "" } else { "\n\n" }
        ));
        first = false;

        for (field, zfield) in FIELD_MAP.iter() {
            if let Some(v) = item.get(zfield).filter(|v| js::truthy(Some(v))) {
                w.write_field(ctx, field, &js::to_js_string(v), false);
            }
        }

        let number = ["reportNumber", "issue", "seriesNumber", "patentNumber"]
            .iter()
            .find_map(|f| item.get(f).filter(|v| js::truthy(Some(v))));
        if let Some(n) = number {
            w.write_field(ctx, "number", &js::to_js_string(n), false);
        }

        if let Some(ad) = item.get_str("accessDate").filter(|a| !a.is_empty()) {
            let ymd = re(&ACCESS, || format!(r"{}*[0-9]+:[0-9]+:[0-9]+", js::WS)).replace(ad, "");
            w.write_field(ctx, "urldate", &ymd, false);
        }

        if let Some(pt) = item
            .get_str("publicationTitle")
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
        {
            if item.item_type == "bookSection" || item.item_type == "conferencePaper" {
                w.write_field(ctx, "booktitle", &pt, false);
            } else if ctx.options.option_truthy("useJournalAbbreviation")
                && item.truthy("journalAbbreviation")
            {
                let ja = item.get_string("journalAbbreviation").unwrap();
                w.write_field(ctx, "journal", &ja, false);
            } else {
                w.write_field(ctx, "journal", &pt, false);
            }
        }

        if let Some(p) = item
            .get_str("publisher")
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
        {
            let f = match item.item_type.as_str() {
                "thesis" => "school",
                "report" => "institution",
                _ => "publisher",
            };
            w.write_field(ctx, f, &p, false);
        }

        if !item.creators.is_empty() {
            let (mut author, mut editor, mut translator, mut collaborator) =
                (String::new(), String::new(), String::new(), String::new());
            let primary = get_creators_for_type(&item.item_type).first().copied();
            for c in &item.creators {
                let Some(s) = creator_string(c.first_name.as_deref(), c.last_name.as_deref())
                else {
                    return Err(TranslateError::Translator(
                        "TypeError: Cannot read properties of undefined (reading 'replace')".into(),
                    ));
                };
                let mut s = escape_special_characters(&s);
                if c.field_mode == Some(1) {
                    s = format!("{{{s}}}");
                } else {
                    s = protect_and(&s);
                }
                let ct = c.creator_type.as_deref();
                let target = if ct == Some("editor") || ct == Some("seriesEditor") {
                    &mut editor
                } else if ct == Some("translator") {
                    &mut translator
                } else if ct.is_some() && ct == primary {
                    &mut author
                } else {
                    &mut collaborator
                };
                target.push_str(" and ");
                target.push_str(&s);
            }
            for (f, v) in [
                ("author", &author),
                ("editor", &editor),
                ("translator", &translator),
                ("collaborator", &collaborator),
            ] {
                if !v.is_empty() {
                    w.write_field(ctx, f, &format!("{{{}}}", &v[5..]), true);
                }
            }
        }

        if let Some(d) = item.get_str("date").filter(|d| !d.is_empty()) {
            let date = str_to_date(d, &ctx.options.env.dates);
            if let Some(m) = date.month {
                w.write_field(ctx, "month", MONTHS[m as usize], true);
            }
            if let Some(y) = date.year.filter(|y| !y.is_empty()) {
                w.write_field(ctx, "year", &y, false);
            }
        }

        if let Some(mut extra) = extra {
            let mut i = 0;
            while i < extra.len() {
                let rec = &extra[i];
                let id = rec
                    .field
                    .as_deref()
                    .filter(|f| !f.is_empty())
                    .and_then(|f| REV_EXTRA_IDS.iter().find(|(k, _)| *k == f).map(|(_, v)| *v));
                let Some(id) = id else {
                    i += 1;
                    continue;
                };
                let value = js::trim(rec.value.as_deref().unwrap_or("")).to_owned();
                if !value.is_empty() {
                    w.write_field(ctx, id, &format!("{{{value}}}"), true);
                    extra.remove(i);
                } else {
                    i += 1;
                }
            }
            let e = extra_recs_to_string(&extra);
            if !e.is_empty() {
                w.write_field(ctx, "note", &e, false);
            }
        }

        if !item.tags.is_empty() {
            let mut ts = String::new();
            for t in &item.tags {
                ts.push_str(", ");
                ts.push_str(&t.tag);
            }
            w.write_field(ctx, "keywords", &ts[2..], false);
        }

        if let Some(p) = item.get_str("pages").filter(|p| !p.is_empty()) {
            let p = re(&PAGES, || "[-\u{2012}-\u{2015}\u{2053}]+".into())
                .replace_all(p, "--")
                .into_owned();
            w.write_field(ctx, "pages", &p, false);
        }

        if ctx.options.option_truthy("exportNotes") {
            for n in item.notes.clone() {
                w.write_field(ctx, "annote", &unescape_html(&n.note), false);
            }
        }

        let mut attachment_string = String::new();
        for a in &item.attachments {
            let title = clean_file_path(a.get_str("title"));
            // exportFileData needs attachment.saveFile, which JSON items lack.
            let path = a
                .get_str("localPath")
                .filter(|p| !p.is_empty())
                .map(|p| clean_file_path(Some(p)));
            if let Some(path) = path.filter(|p| !p.is_empty()) {
                attachment_string.push(';');
                attachment_string.push_str(&encode_file_path_component(&title));
                attachment_string.push(':');
                attachment_string.push_str(&encode_file_path_component(&path));
                attachment_string.push(':');
                attachment_string.push_str(&encode_file_path_component(
                    a.get_str("mimeType").unwrap_or(""),
                ));
            }
        }
        if !attachment_string.is_empty() {
            w.write_field(ctx, "file", &attachment_string[1..], false);
        }

        ctx.write(",\n}");
    }
    ctx.write("\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braces_balanced_lose_vphantom() {
        assert_eq!(escape_special_characters("a {b} c"), "a \\{b\\} c");
        assert_eq!(escape_special_characters("a {b"), "a \\{\\vphantom{\\}}b");
        assert_eq!(escape_special_characters("50% & x_y"), "50\\% \\& x\\_y");
    }

    #[test]
    fn caps_protection() {
        assert_eq!(
            protect_caps("The DNA of Rust", ProtectCaps::All),
            "The {DNA} of {Rust}"
        );
    }
}
