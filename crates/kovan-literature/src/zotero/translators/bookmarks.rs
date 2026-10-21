// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Bookmarks.js (translatorID
//   4e7119e0-02be-4848-86ef-79a64185aad8, lastUpdated 2022-07-14 20:21:20):
//   header :1-12, regular expressions :50-55, `detectImport` :57-76,
//   `doImport` :78-202, `convertDate` :204-214, `doExport` :218-245.
// Copyright (C) 2011 Avram Lyon, ajlyon@gmail.com.
// Licence: AGPL-3.0 (upstream: GPL-3.0-or-later, combined under section 13
//   of both licences).

//! The Bookmarks translator (Netscape bookmark file format): import and
//! export.
//!
//! Upstream parses with JavaScript regular expressions that use
//! backreferences and lookahead, which the `regex` crate lacks. Each of the
//! five is ported as a hand-written matcher with the same backtracking
//! outcome (documented on each), over the text as `char`s (upstream's
//! positions are UTF-16 code units; they differ only on astral-plane
//! characters). The `i` flag compares ASCII letters without case, as
//! JavaScript does for these all-ASCII patterns.
//!
//! Collections are kept in [`crate::zotero::framework::ImportResult`]'s
//! `collections` (the translation-server discards them).
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `bookmarks_*`).

use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::{lpad, trim_internal};
use crate::zotero::framework::{
    js, CollectionChild, ExportContext, ImportContext, TranslateError, TranslatorCollection,
    TranslatorItem, TranslatorTag,
};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "4e7119e0-02be-4848-86ef-79a64185aad8",
    label: "Bookmarks",
    creator: "Avram Lyon",
    target: "html",
    min_version: "2.1b6",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-07-14 20:21:20",
};

/// `MAX_DETECT_LINES` (:50).
const MAX_DETECT_LINES: usize = 150;

/// A match: start, end, and capture groups (as strings).
#[derive(Debug, Clone)]
struct Match {
    start: usize,
    end: usize,
    groups: Vec<String>,
}

/// Case-insensitive ASCII literal at `at`.
fn lit(c: &[char], at: usize, s: &str) -> bool {
    let n = s.len();
    at + n <= c.len()
        && c[at..at + n].iter().zip(s.chars()).all(|(a, b)| {
            a.to_ascii_lowercase() == b.to_ascii_lowercase() && a.is_ascii() == b.is_ascii()
        })
}

fn skip_ws(c: &[char], mut i: usize) -> usize {
    while i < c.len() && js::is_space(c[i]) {
        i += 1;
    }
    i
}

fn s(c: &[char], a: usize, b: usize) -> String {
    c[a..b].iter().collect()
}

/// `bookmarkRE` (:51) at `start`:
/// `<DT>[\s\r\n]*<A[^>]+HREF[\s\r\n]*=[\s\r\n]*(['"])([^"]+)\1[^>]*>([^<\n]+?)<\/A>`.
///
/// `[^>]+` is greedy, so the `HREF` tried first is the last one before the
/// tag's first `>`. `([^"]+)` is greedy and backtracks to the last closing
/// quote that lets the rest match. `[^>]*>` and the lazy title are
/// deterministic: the first `>`, then the text up to the first `<` or
/// newline, which must be `</A>`.
fn bookmark_at(c: &[char], start: usize) -> Option<Match> {
    if !lit(c, start, "<dt>") {
        return None;
    }
    let i = skip_ws(c, start + 4);
    if !lit(c, i, "<a") {
        return None;
    }
    let p0 = i + 2;
    let tag_end = (p0..c.len()).find(|&k| c[k] == '>').unwrap_or(c.len());
    // HREF starts in [p0 + 1, tag_end - 4], tried from the right.
    let mut h = tag_end.saturating_sub(4);
    while h > p0 {
        if lit(c, h, "href") {
            if let Some(m) = after_href(c, start, h + 4) {
                return Some(m);
            }
        }
        h -= 1;
    }
    None
}

fn after_href(c: &[char], start: usize, i: usize) -> Option<Match> {
    let i = skip_ws(c, i);
    if c.get(i) != Some(&'=') {
        return None;
    }
    let i = skip_ws(c, i + 1);
    let q = *c.get(i).filter(|q| **q == '\'' || **q == '"')?;
    let g2_start = i + 1;
    // `[^"]+` runs to the first '"'; then back off to a closing quote.
    let g2_max = (g2_start..c.len())
        .find(|&k| c[k] == '"')
        .unwrap_or(c.len());
    let mut e = g2_max;
    while e > g2_start {
        if c.get(e) == Some(&q) {
            // `[^>]*>`
            if let Some(gt) = (e + 1..c.len()).find(|&k| c[k] == '>') {
                // `([^<\n]+?)<\/A>`
                let t_start = gt + 1;
                let t_end = (t_start..c.len())
                    .find(|&k| c[k] == '<' || c[k] == '\n')
                    .unwrap_or(c.len());
                if t_end > t_start && lit(c, t_end, "</a>") {
                    return Some(Match {
                        start,
                        end: t_end + 4,
                        groups: vec![
                            s(c, start, t_end + 4),
                            q.to_string(),
                            s(c, g2_start, e),
                            s(c, t_start, t_end),
                        ],
                    });
                }
            }
        }
        e -= 1;
    }
    None
}

/// `collectionRE` (:52) at `start`: `<DT>[\s\r\n]*<H3[^>]*>([^<]+?)<\/H3>`.
fn collection_at(c: &[char], start: usize) -> Option<Match> {
    if !lit(c, start, "<dt>") {
        return None;
    }
    let i = skip_ws(c, start + 4);
    if !lit(c, i, "<h3") {
        return None;
    }
    let gt = (i + 3..c.len()).find(|&k| c[k] == '>')?;
    let n_start = gt + 1;
    let n_end = (n_start..c.len()).find(|&k| c[k] == '<').unwrap_or(c.len());
    if n_end > n_start && lit(c, n_end, "</h3>") {
        return Some(Match {
            start,
            end: n_end + 5,
            groups: vec![s(c, start, n_end + 5), s(c, n_start, n_end)],
        });
    }
    None
}

/// `collectionEndRE` (:53) at `start`: `<\/DL>`.
fn collection_end_at(c: &[char], start: usize) -> Option<Match> {
    lit(c, start, "</dl>").then(|| Match {
        start,
        end: start + 5,
        groups: vec![s(c, start, start + 5)],
    })
}

/// `descriptionRE` (:54) at `start`: `<DD>([\s\S]*?)(?=<(?:DT|\/DL|HR)>)`:
/// the text up to the first `<DT>`, `</DL>` or `<HR>`.
fn description_at(c: &[char], start: usize) -> Option<Match> {
    if !lit(c, start, "<dd>") {
        return None;
    }
    let d_start = start + 4;
    let end = (d_start..=c.len())
        .find(|&k| lit(c, k, "<dt>") || lit(c, k, "</dl>") || lit(c, k, "<hr>"))?;
    Some(Match {
        start,
        end,
        groups: vec![s(c, start, end), s(c, d_start, end)],
    })
}

/// `re.exec(text)` from `last_index` for a global regex: the leftmost match.
fn exec(c: &[char], last_index: usize, at: fn(&[char], usize) -> Option<Match>) -> Option<Match> {
    (last_index..c.len()).find_map(|p| at(c, p))
}

/// `bookmarkDetailsRE` (:55) matches in `m0`:
/// `[\s\r\n](HREF|TAGS|ADD_DATE|SHORTCUTURL|DESCRIPTION)[s\r\n]*=[s\r\n]*(['"])([\s\S]*?)\2`
/// (`[s\r\n]` is upstream's: the letter s, either case, CR or LF).
fn details(m0: &str) -> Vec<(String, String)> {
    let c: Vec<char> = m0.chars().collect();
    let in_class = |x: char| matches!(x, 's' | 'S' | '\r' | '\n');
    let mut out = Vec::new();
    let mut p = 0;
    'scan: while p < c.len() {
        if js::is_space(c[p]) {
            for name in ["HREF", "TAGS", "ADD_DATE", "SHORTCUTURL", "DESCRIPTION"] {
                if !lit(&c, p + 1, name) {
                    continue;
                }
                let mut i = p + 1 + name.len();
                while i < c.len() && in_class(c[i]) {
                    i += 1;
                }
                if c.get(i) != Some(&'=') {
                    continue;
                }
                i += 1;
                while i < c.len() && in_class(c[i]) {
                    i += 1;
                }
                let Some(&q) = c.get(i).filter(|q| **q == '\'' || **q == '"') else {
                    continue;
                };
                let v_start = i + 1;
                let Some(v_end) = (v_start..c.len()).find(|&k| c[k] == q) else {
                    continue;
                };
                out.push((s(&c, p + 1, p + 1 + name.len()), s(&c, v_start, v_end)));
                p = v_end + 1;
                continue 'scan;
            }
        }
        p += 1;
    }
    out
}

/// `detectImport` (:57-76).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut text: Vec<char> = Vec::new();
    let mut last_index = 0;
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        let before = i;
        i += 1;
        if before >= MAX_DETECT_LINES {
            break;
        }
        text.extend(line.chars());
        let m = exec(&text, last_index, bookmark_at);
        if let Some(m) = &m {
            if last_index < m.end {
                last_index = m.end;
            }
            if !m.groups[2].to_uppercase().starts_with("PLACE:") {
                return true;
            }
        }
    }
    false
}

/// JavaScript `Number(string)` (StringToNumber) for `convertDate`.
fn js_number(s: &str) -> f64 {
    let t = js::trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let lower = t.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(d) = lower.strip_prefix(prefix) {
            return u128::from_str_radix(d, radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let ok = t
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-'));
    if !ok {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// Civil date of a day count since 1970-01-01 (Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `ZU.lpad(n, '0', len)` for a number (`n ? n + '' : ''`: 0 and NaN pad
/// from "").
fn lpad_num(n: Option<i64>, len: usize) -> String {
    let s = match n {
        Some(n) if n != 0 => n.to_string(),
        _ => String::new(),
    };
    lpad(&s, "0", len)
}

/// `convertDate(timestamp)` (:204-214): seconds since the epoch as a UTC
/// `YYYY-MM-DD hh:mm:ss`.
fn convert_date(timestamp: &str) -> String {
    let ms = js_number(timestamp) * 1000.0;
    // `new Date(ms)`: TimeClip.
    let parts = if ms.is_finite() && ms.abs() <= 8.64e15 {
        let ms = ms.trunc() as i64;
        let days = ms.div_euclid(86_400_000);
        let rem = ms.rem_euclid(86_400_000) / 1000;
        let (y, m, d) = civil_from_days(days);
        Some((y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60))
    } else {
        None
    };
    let f = |g: fn(&(i64, i64, i64, i64, i64, i64)) -> i64| parts.as_ref().map(g);
    format!(
        "{}-{}-{} {}:{}:{}",
        lpad_num(f(|p| p.0), 4),
        lpad_num(f(|p| p.1), 2),
        lpad_num(f(|p| p.2), 2),
        lpad_num(f(|p| p.3), 2),
        lpad_num(f(|p| p.4), 2),
        lpad_num(f(|p| p.5), 2),
    )
}

/// `doImport` (:78-202).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut item_id: i64 = 0;
    let mut line: Vec<char> = Vec::new();
    let mut last_index = 0;
    let mut open_item: Option<TranslatorItem> = None;
    let mut collection_stack: Vec<TranslatorCollection> = Vec::new();
    let mut collection: Option<TranslatorCollection> = None;

    while let Some(l) = ctx.read_line() {
        line.push('\n');
        line.extend(l.chars());
        loop {
            // `for (var re in allREs)`: b, c, ce, d (d only with an open
            // item); the earliest match wins, ties to the first.
            let mut first: Option<(usize, Match)> = None;
            let matchers: [fn(&[char], usize) -> Option<Match>; 4] = [
                bookmark_at,
                collection_at,
                collection_end_at,
                description_at,
            ];
            for (k, at) in matchers.into_iter().enumerate() {
                if k == 3 && open_item.is_none() {
                    continue;
                }
                if let Some(m) = exec(&line, last_index, at) {
                    if first.as_ref().is_none_or(|(_, f)| m.start < f.start) {
                        first = Some((k, m));
                    }
                }
            }
            let Some((k, m)) = first else { break };
            last_index = m.end;
            match k {
                0 => {
                    if let Some(i) = open_item.take() {
                        ctx.item_done(i);
                    }
                    let title = js::trim(&m.groups[3]).to_owned();
                    if title.is_empty() || m.groups[2].to_uppercase().starts_with("PLACE:") {
                        continue;
                    }
                    let mut item = TranslatorItem::new("webpage");
                    item.set("title", unescape_html(&title));
                    // `openItem.itemID = openItem.id = itemID++` (dropped by
                    // itemToAPIJSON).
                    item.set("id", item_id);
                    item.set("itemID", item_id);
                    if let Some(c) = collection.as_mut() {
                        c.children.push(CollectionChild::Item {
                            id: item_id.to_string(),
                        });
                    }
                    item_id += 1;
                    for (name, value) in details(&m.groups[0]) {
                        match name.to_uppercase().as_str() {
                            "HREF" => item.set("url", value),
                            "DESCRIPTION" => item.set("abstractNote", value),
                            "TAGS" | "SHORTCUTURL" => {
                                let sep = regex_split_comma(&value);
                                item.tags.extend(sep.into_iter().map(TranslatorTag::new));
                            }
                            "ADD_DATE" => item.set("accessDate", convert_date(&value)),
                            _ => {}
                        }
                    }
                    open_item = Some(item);
                }
                1 => {
                    if let Some(i) = open_item.take() {
                        ctx.item_done(i);
                    }
                    if let Some(c) = collection.take() {
                        collection_stack.push(c);
                    }
                    collection = Some(TranslatorCollection {
                        name: unescape_html(&m.groups[1]),
                        children: Vec::new(),
                    });
                }
                2 => {
                    if let Some(i) = open_item.take() {
                        ctx.item_done(i);
                    }
                    match collection_stack.pop() {
                        Some(mut parent) => {
                            // `collection.children.length` with a collection
                            // open (the stack is only pushed with one).
                            if let Some(c) = collection.take() {
                                if !c.children.is_empty() {
                                    parent.children.push(CollectionChild::Collection(c));
                                }
                            }
                            collection = Some(parent);
                        }
                        None => {
                            if collection.as_ref().is_some_and(|c| !c.children.is_empty()) {
                                ctx.collection_done(collection.take().unwrap());
                            }
                        }
                    }
                }
                _ => {
                    let mut i = open_item.take().expect("checked above");
                    i.set("abstractNote", trim_internal(&m.groups[1]));
                    ctx.item_done(i);
                }
            }
        }
        line = line[last_index.min(line.len())..].to_vec();
        last_index = 0;
    }

    if let Some(i) = open_item.take() {
        ctx.item_done(i);
    }
    if let Some(mut c) = collection {
        while let Some(mut parent) = collection_stack.pop() {
            if !c.children.is_empty() {
                parent.children.push(CollectionChild::Collection(c));
            }
            c = parent;
        }
        if !c.children.is_empty() {
            ctx.collection_done(c);
        }
    }
    Ok(())
}

/// `value.split(/[\s\r\n]*,[\s\r\n]*/)`.
fn regex_split_comma(v: &str) -> Vec<String> {
    let c: Vec<char> = v.chars().collect();
    let mut out = Vec::new();
    let mut piece = 0;
    let mut q = 0;
    while q < c.len() {
        let k = skip_ws(&c, q);
        if c.get(k) == Some(&',') {
            out.push(s(&c, piece, q));
            let e = skip_ws(&c, k + 1);
            piece = e;
            q = e;
            continue;
        }
        q += 1;
    }
    out.push(s(&c, piece, c.len()));
    out
}

/// `doExport` (:218-245).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    ctx.write(concat!(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n",
        "<!-- This is an automatically generated file.\n",
        "     It will be read and overwritten.\n",
        "     DO NOT EDIT! -->\n",
        "<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n",
        "<TITLE>Bookmarks</TITLE>\n",
        "<H1>Bookmarks Menu</H1>\n",
        "<DL>\n"
    ));
    while let Some(item) = ctx.next_item() {
        let tags: Vec<&str> = item.tags.iter().map(|t| t.tag.as_str()).collect();
        let tags = tags.join(",");
        if item.truthy("url") {
            let url = item.get_string("url").unwrap_or_default();
            ctx.write(&format!("    <DT><A HREF=\"{url}\""));
            if !tags.is_empty() {
                ctx.write(&format!(" TAGS=\"{tags}\""));
            }
            let title = item
                .get("title")
                .map_or_else(|| "undefined".to_owned(), js::to_js_string);
            ctx.write(&format!(">{title}</A>\n"));
        }
    }
    ctx.write("</DL>");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matchers() {
        let c: Vec<char> = "<DT><A HREF=\"http://x\" TAGS=\"a, b\">T</A>"
            .chars()
            .collect();
        let m = bookmark_at(&c, 0).unwrap();
        assert_eq!(m.groups[2], "http://x");
        assert_eq!(m.groups[3], "T");
        assert_eq!(
            details(&m.groups[0]),
            vec![
                ("HREF".to_owned(), "http://x".to_owned()),
                ("TAGS".to_owned(), "a, b".to_owned())
            ]
        );
        assert_eq!(regex_split_comma("a , b,c"), ["a", "b", "c"]);
        assert_eq!(convert_date("0"), "1970-01-01 00:00:00");
        assert_eq!(convert_date("x"), "0000-00-00 00:00:00");
        assert_eq!(convert_date("1396239625"), "2014-03-31 04:20:25");
    }
}
