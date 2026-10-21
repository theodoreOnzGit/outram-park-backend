// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js `RISReader` :661-820, `TagCleaner`
//   :825-861, `ProCiteCleaner` :869-1218, `EndNoteCleaner` :1223-1239,
//   `CitaviCleaner` :1244-1275.
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Reading RIS entries and cleaning up their tags.
//!
//! Upstream's entry is an array of `{tag, value, raw}` objects plus a
//! `tags` property mapping each tag to the same objects; cleaners mutate the
//! objects through either view. Here the objects live in an arena
//! ([`Entry::pairs`]) and both views hold indices into it, so object
//! identity (`indexOf`, shared mutation) carries over.

use super::mapper::TagMapper;
use super::tables::{PROCITE_AUTHOR_ROLE, PROCITE_MAP};
use crate::zotero::framework::{js, ImportContext, TranslateError};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

fn ws_inner() -> &'static str {
    &js::WS[1..js::WS.len() - 1]
}

/// A tag-value pair (`{tag, value, raw}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagValue {
    /// The RIS tag.
    pub tag: String,
    /// The value, continuation lines joined.
    pub value: String,
    /// The line(s) as read.
    pub raw: String,
}

/// One RIS entry.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    /// Every pair object ever created for this entry.
    pub pairs: Vec<TagValue>,
    /// The entry array: pair ids in order.
    pub order: Vec<usize>,
    /// `entry.tags`: tag -> pair ids.
    pub tags: BTreeMap<String, Vec<usize>>,
}

impl Entry {
    /// `entry.length`.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Whether the entry has no pairs.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// `entry[i]`.
    pub fn at(&self, i: usize) -> &TagValue {
        &self.pairs[self.order[i]]
    }

    /// `entry[i]`, mutably.
    pub fn at_mut(&mut self, i: usize) -> &mut TagValue {
        &mut self.pairs[self.order[i]]
    }

    /// `entry.indexOf(pair)` (`None` is -1).
    pub fn index_of(&self, id: usize) -> Option<usize> {
        self.order.iter().position(|&x| x == id)
    }

    /// The first pair for a tag (`entry.tags[tag][0]`).
    pub fn first(&self, tag: &str) -> Option<&TagValue> {
        self.tags
            .get(tag)
            .and_then(|l| l.first())
            .map(|&id| &self.pairs[id])
    }

    /// `entry.tags[tag]` is truthy (an array, even an empty one).
    pub fn has(&self, tag: &str) -> bool {
        self.tags.contains_key(tag)
    }

    fn push(&mut self, tv: TagValue) {
        let id = self.pairs.len();
        self.tags.entry(tv.tag.clone()).or_default().push(id);
        self.pairs.push(tv);
        self.order.push(id);
    }

    /// `ZU.deepCopy(pair)` as a new arena object.
    fn copy_of(&mut self, id: usize) -> usize {
        let c = self.pairs[id].clone();
        self.pairs.push(c);
        self.pairs.len() - 1
    }
}

/// `TagCleaner.changeTag(entry, at, toTags)` (:836-860): retag the pair at
/// `at` (or remove it when `to_tags` is empty), adding copies for further tags.
pub fn change_tag(entry: &mut Entry, at: usize, to_tags: &[&str]) -> Result<(), TranslateError> {
    let source = entry.order[at];
    let src_tag = entry.pairs[source].tag.clone();
    // `byTag.splice(byTag.indexOf(source), 1)`: -1 removes the last element;
    // a missing list throws (TypeError), which rejects the import.
    let by_tag = entry.tags.get_mut(&src_tag).ok_or_else(|| {
        TranslateError::Translator(format!(
            "Cannot read properties of undefined (entry.tags.{src_tag})"
        ))
    })?;
    match by_tag.iter().position(|&x| x == source) {
        Some(i) => {
            by_tag.remove(i);
        }
        None => {
            by_tag.pop();
        }
    }
    if by_tag.is_empty() {
        entry.tags.remove(&src_tag);
    }
    if to_tags.is_empty() {
        entry.order.remove(at);
        return Ok(());
    }
    entry.pairs[source].tag = to_tags[0].to_owned();
    entry
        .tags
        .entry(to_tags[0].to_owned())
        .or_default()
        .push(source);
    for (i, t) in to_tags.iter().enumerate().skip(1) {
        let id = entry.copy_of(source);
        entry.pairs[id].tag = (*t).to_owned();
        let pos = (at + i).min(entry.order.len());
        entry.order.insert(pos, id);
        entry.tags.entry((*t).to_owned()).or_default().push(id);
    }
    Ok(())
}

/// The RIS reader's state (`RISReader`, :661-820) and the ProCite cleaner's
/// `proCiteMode` flag (:870), both singletons that persist across entries.
#[derive(Debug, Clone, Default)]
pub struct RisReader {
    tag_value_buffer: Vec<TagValue>,
    line_buffer: Vec<String>,
    max_line_length: usize,
    /// `ProCiteCleaner.proCiteMode`.
    pub pro_cite_mode: bool,
}

/// `risFormat` (:703): a tag line. `.` excludes line terminators.
fn ris_format() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, || {
        r"^([A-Z][A-Z0-9]) {1,2}-(?: ([^\n\r\x{2028}\x{2029}]*))?$".to_owned()
    })
}

impl RisReader {
    /// `nextEntry` (:675-701).
    pub fn next_entry(&mut self, ctx: &mut ImportContext) -> Option<Entry> {
        let mut entry = Entry::default();
        loop {
            let tv = match self.tag_value_buffer.pop() {
                Some(tv) => tv,
                None => match self.get_tag_value(ctx) {
                    Some(tv) => tv,
                    None => break,
                },
            };
            if tv.tag == "TY" && !entry.is_empty() {
                self.tag_value_buffer.push(tv);
                return Some(entry);
            }
            if tv.tag == "ER" {
                if entry.is_empty() {
                    continue;
                }
                return Some(entry);
            }
            entry.push(tv);
        }
        if entry.is_empty() {
            None
        } else {
            Some(entry)
        }
    }

    /// `_getTagValue` (:720-792).
    fn get_tag_value(&mut self, ctx: &mut ImportContext) -> Option<TagValue> {
        let mut tag_value: Option<TagValue> = None;
        let mut last_line_length: usize = 0;
        while let Some(line) = self.next_line(ctx) {
            let temp = ris_format().captures(&line);
            if temp.is_none() && tag_value.is_none() {
                continue;
            }
            let line_len = js::len(&line);
            if line_len > self.max_line_length {
                self.max_line_length = line_len;
            }
            if temp.is_some() && tag_value.is_some() {
                self.line_buffer.push(line);
                return tag_value;
            }
            if let Some(t) = temp {
                tag_value = Some(TagValue {
                    tag: t[1].to_owned(),
                    value: t.get(2).map_or(String::new(), |m| m.as_str().to_owned()),
                    raw: line.clone(),
                });
            } else if let Some(tv) = tag_value.as_mut() {
                let mut new_line_added = false;
                let mut clean_line = js::trim(&line).to_owned();
                if ["AB", "N1", "N2", "RN"].contains(&tv.tag.as_str())
                    && (self.max_line_length > 85 || last_line_length < 65 || clean_line.is_empty())
                {
                    clean_line = format!("\n{clean_line}");
                    new_line_added = true;
                }
                if !new_line_added && ["KW", "L1", "L2", "L3"].contains(&tv.tag.as_str()) {
                    clean_line = format!("\n{clean_line}");
                    new_line_added = true;
                }
                if !new_line_added && !tv.value.ends_with(' ') {
                    clean_line = format!(" {clean_line}");
                }
                tv.raw.push('\n');
                tv.raw.push_str(&line);
                tv.value.push_str(&clean_line);
            }
            last_line_length = line_len;
        }
        tag_value
    }

    /// `_nextLine` (:802-819).
    fn next_line(&mut self, ctx: &mut ImportContext) -> Option<String> {
        if let Some(l) = self.line_buffer.pop() {
            return Some(l);
        }
        let line = ctx.read_line()?;
        if line.contains('\u{2028}') || line.contains('\u{2029}') {
            static R: OnceLock<Regex> = OnceLock::new();
            let r = re(&R, || {
                format!(
                    r"{ws}?[\x{{2028}}\x{{2029}}]|[\x{{2028}}\x{{2029}}]{ws}?",
                    ws = js::WS
                )
            });
            return Some(r.replace_all(&line, " ").into_owned());
        }
        Some(line)
    }

    /// `ProCiteCleaner.cleanTags(entry, item)` (:928-1119).
    pub fn pro_cite_clean(
        &mut self,
        entry: &mut Entry,
        item_type: &str,
        fields: &TagMapper,
    ) -> Result<(), TranslateError> {
        static SPLIT: OnceLock<Regex> = OnceLock::new();
        let tag_value_split = re(&SPLIT, || {
            format!(
                r"([A-Za-z,{w}]+){ws}*:{ws}*([\s\S]*)",
                w = ws_inner(),
                ws = js::WS
            )
        });
        if self.pro_cite_mode {
            let ty = entry.first("TY").map(|t| t.value.clone());
            if matches!(ty.as_deref(), Some("CHAP") | Some("BOOK")) && entry.has("VL") {
                change_all_tags(entry, "VL", "ET");
            }
        }

        let mut extent_of_work: Option<usize> = None;
        let mut packaging_method: Option<usize> = None;
        let mut i: isize = 0;
        while (i as usize) < entry.len() {
            let iu = i as usize;
            if entry.at(iu).tag != "N1" {
                i += 1;
                continue;
            }
            let trimmed = js::trim(&entry.at(iu).value).to_owned();
            let Some(m) = tag_value_split.captures(&trimmed) else {
                i += 1;
                continue;
            };
            let (m1, m2) = (m[1].to_owned(), m[2].to_owned());
            match m1.as_str() {
                "Author, Subsidiary" | "Author, Monographic" => {
                    let ris_tag = if entry.has("A1") {
                        if entry.has("A2") {
                            "A3"
                        } else {
                            "A2"
                        }
                    } else {
                        "A1"
                    };
                    static SEMI: OnceLock<Regex> = OnceLock::new();
                    let authors: Vec<String> = re(&SEMI, || format!(";{}*", js::WS))
                        .split(&m2)
                        .map(str::to_owned)
                        .collect();
                    self.change_tag(entry, iu, &[ris_tag])?;
                    entry.at_mut(iu).value = fix_author(&authors[0]);
                    let base = entry.order[iu];
                    for a in authors.iter().skip(1) {
                        let id = entry.copy_of(base);
                        entry.pairs[id].value = a.clone();
                        entry.order.insert(iu + 1, id);
                        entry.tags.entry(ris_tag.to_owned()).or_default().push(id);
                    }
                    i += authors.len() as isize - 1;
                }
                "Artist Role"
                | "Series Editor Role"
                | "Editor/Compiler Role"
                | "Cartographer Role"
                | "Composer Role"
                | "Producer Role"
                | "Director Role"
                | "Performer Role"
                | "Author Role" => {
                    let roles = normalize_author_role(&m2);
                    let mut ris_tags: Vec<&'static str> = Vec::new();
                    let mut fail = false;
                    for r in &roles {
                        let Some((_, role)) = PROCITE_AUTHOR_ROLE.iter().find(|(k, _)| k == r)
                        else {
                            continue;
                        };
                        let role = format!("creators/{role}");
                        match fields.reverse_lookup(item_type, &role) {
                            None => {
                                fail = true;
                                break;
                            }
                            Some(t) => {
                                if !ris_tags.contains(&t) {
                                    ris_tags.push(t);
                                }
                            }
                        }
                    }
                    if fail || ris_tags.is_empty() {
                        i += 1;
                        continue;
                    }
                    let added =
                        self.remap_preceding_tags(entry, iu, &["A1", "A2", "A3"], &ris_tags)?;
                    if let Some(added) = added {
                        // `this._changeTag(entry, i)` removes whatever is at i
                        // now (after insertions, not necessarily the note).
                        self.change_tag(entry, iu, &[])?;
                        i -= 1;
                        i += added as isize;
                    }
                }
                "Record ID" | "Record Number" => {
                    self.change_tag(entry, iu, &["ID"])?;
                    entry.at_mut(iu).value = m2;
                }
                "Notes" => entry.at_mut(iu).value = m2,
                "Connective Phrase" => {
                    if js::trim(&m2).to_lowercase() == "in" {
                        self.change_tag(entry, iu, &[])?;
                        i -= 1;
                    }
                }
                "Extent of Work" => extent_of_work = Some(entry.order[iu]),
                "Packaging Method" => packaging_method = Some(entry.order[iu]),
                other => {
                    if let Some((_, field)) = PROCITE_MAP.iter().find(|(k, _)| *k == other) {
                        match fields.reverse_lookup(item_type, field) {
                            None => {
                                i += 1;
                                continue;
                            }
                            Some(t) => {
                                self.change_tag(entry, iu, &[t])?;
                                entry.at_mut(iu).value = m2;
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        if let Some(eow) = extent_of_work {
            static EXT: OnceLock<Regex> = OnceLock::new();
            static NUM: OnceLock<Regex> = OnceLock::new();
            static PACK: OnceLock<Regex> = OnceLock::new();
            let eow_value = entry.pairs[eow].value.clone();
            let mut extent = tag_value_split
                .captures(&eow_value)
                .map(|c| c[2].to_owned())
                .unwrap_or_default();
            let ext_re = re(&EXT, || {
                format!(
                    r"(?i)^([0-9]+){ws}*(pages?|p(?:p|gs?)?|vols?|volumes?)\.?$",
                    ws = js::WS
                )
            });
            let mut units: Option<&str> = None;
            let mut delete_packaging = false;
            if let Some(m) = ext_re.captures(&extent) {
                units = Some(if m[2].to_lowercase().starts_with('p') {
                    "numPages"
                } else {
                    "numberOfVolumes"
                });
                extent = m[1].to_owned();
            } else if let Some(pm) = packaging_method {
                let numeric =
                    re(&NUM, || format!(r"^{ws}*[0-9]+{ws}*$", ws = js::WS)).is_match(&extent);
                let pack_re = re(&PACK, || {
                    format!(
                        r"(?i):{ws}*(pages?|p(?:p|gs?)?|vols?|volumes?)\.?{ws}*$",
                        ws = js::WS
                    )
                });
                let pv = entry.pairs[pm].value.clone();
                if numeric {
                    if let Some(m) = pack_re.captures(&pv) {
                        units = Some(if m[1].to_lowercase().starts_with('p') {
                            "numPages"
                        } else {
                            "numberOfVolumes"
                        });
                        extent = js::trim(&extent).to_owned();
                        delete_packaging = true;
                    }
                }
            }
            if let Some(units) = units {
                if let Some(t) = fields.reverse_lookup(item_type, units) {
                    entry.pairs[eow].value = extent;
                    let at = entry.index_of(eow).ok_or_else(|| {
                        TranslateError::Translator("ProCite extent of work: entry not found".into())
                    })?;
                    self.change_tag(entry, at, &[t])?;
                    if delete_packaging {
                        if let Some(pm) = packaging_method {
                            let at = entry.index_of(pm).ok_or_else(|| {
                                TranslateError::Translator(
                                    "ProCite packaging method: entry not found".into(),
                                )
                            })?;
                            self.change_tag(entry, at, &[])?;
                        }
                    }
                }
            }
        }

        if !self.pro_cite_mode {
            return Ok(());
        }

        let ty = entry.first("TY").map(|t| t.value.clone());
        let ty = ty.as_deref();

        if ty == Some("CHAP") {
            let mut title_tags = vec!["T3", "T2", "TI"];
            let mut i = 0;
            while i < entry.len() && !title_tags.is_empty() {
                if ["TI", "T1", "T2", "T3"].contains(&entry.at(i).tag.as_str()) {
                    let new_tag = title_tags.pop().unwrap();
                    if entry.at(i).tag != new_tag {
                        self.change_tag(entry, i, &[new_tag])?;
                    }
                }
                i += 1;
            }
        }
        if ty == Some("BOOK") && entry.tags.get("IS").is_some_and(|l| !l.is_empty()) {
            change_all_tags(entry, "IS", "VL");
        }
        if matches!(ty, Some("CHAP") | Some("BOOK"))
            && entry.tags.get("VL").is_some_and(|l| l.len() > 1)
        {
            let first = entry.tags["VL"][0];
            let at = entry
                .index_of(first)
                .ok_or_else(|| TranslateError::Translator("ProCite VL: entry not found".into()))?;
            self.change_tag(entry, at, &["ET"])?;
        }
        if ty == Some("COMP") && entry.has("IS") {
            change_all_tags(entry, "IS", "ET");
        }
        if ty == Some("BILL") {
            if entry.has("CY") {
                change_all_tags(entry, "CY", "T2");
            }
            if entry.has("VL") {
                change_all_tags(entry, "VL", "M1");
            }
            if entry.has("SP") {
                change_all_tags(entry, "SP", "SE");
            }
        }
        if ty == Some("ART") && entry.has("M1") {
            change_all_tags(entry, "M1", "M3");
        }
        Ok(())
    }

    /// `ProCiteCleaner._changeTag` (:1171-1178): `changeTag`, then ProCite
    /// mode is on (for this and every later entry).
    fn change_tag(
        &mut self,
        entry: &mut Entry,
        at: usize,
        to_tags: &[&str],
    ) -> Result<(), TranslateError> {
        change_tag(entry, at, to_tags)?;
        self.pro_cite_mode = true;
        Ok(())
    }

    /// `_remapPreceedingTags` (:1194-1217): `None` is `false`; `Some(0)` is
    /// `true`; `Some(n)` the number of pairs added.
    fn remap_preceding_tags(
        &mut self,
        entry: &mut Entry,
        start: usize,
        allowed: &[&str],
        ris_tags: &[&str],
    ) -> Result<Option<usize>, TranslateError> {
        let mut tag: Option<String> = None;
        let mut added = 0usize;
        let mut i = start as isize - 1;
        while i >= 0 {
            let iu = i as usize;
            let cur = entry.at(iu).tag.clone();
            if tag.as_ref().is_some_and(|t| *t != cur) {
                return Ok(Some(added));
            }
            if !allowed.contains(&cur.as_str()) {
                return Ok(None);
            }
            tag = Some(cur);
            self.change_tag(entry, iu, ris_tags)?;
            added += ris_tags.len() - 1;
            i -= 1;
        }
        Ok(tag.map(|_| added))
    }
}

/// `_changeAllTags(entry, from, to)` (:1156-1165): every pair of `from`
/// becomes `to`, and `entry.tags[to]` is replaced by `from`'s list.
fn change_all_tags(entry: &mut Entry, from: &str, to: &str) {
    let Some(list) = entry.tags.remove(from) else {
        return;
    };
    for &id in &list {
        entry.pairs[id].tag = to.to_owned();
    }
    entry.tags.insert(to.to_owned(), list);
}

/// `_normalizeAuthorRole` (:1128-1133).
fn normalize_author_role(role: &str) -> Vec<String> {
    static STRIP: OnceLock<Regex> = OnceLock::new();
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    let s = re(&STRIP, || {
        format!(r"s(?-u:\b)|\.|{ws}+by(?-u:\b)|with an{ws}*", ws = js::WS)
    })
    .replace_all(&role.to_lowercase(), "")
    .into_owned();
    re(&SPLIT, || format!(r"{ws}*(?:,|and){ws}*", ws = js::WS))
        .split(&s)
        .map(str::to_owned)
        .collect()
}

/// `_fixAuthor` (:1142-1146): "First Last" -> "Last, First".
fn fix_author(author: &str) -> String {
    if author.contains(',') || !js::trim(author).contains(' ') {
        return author.to_owned();
    }
    let a = js::trim(author);
    let idx = a.rfind(' ').unwrap();
    format!("{}, {}", &a[idx + 1..], &a[..idx])
}

/// `EndNoteCleaner.cleanTags` (:1231-1238): authors of an edited book are
/// editors (A3).
pub fn end_note_clean(entry: &mut Entry) -> Result<(), TranslateError> {
    if entry.first("TY").is_some_and(|t| t.value == "EDBOOK") && entry.has("AU") {
        let n = entry.tags["AU"].len();
        for i in (0..n).rev() {
            let id = entry.tags["AU"][i];
            let at = entry
                .index_of(id)
                .ok_or_else(|| TranslateError::Translator("EndNote AU: entry not found".into()))?;
            change_tag(entry, at, &["A3"])?;
        }
    }
    Ok(())
}

/// `CitaviCleaner.cleanTags` (:1245-1274): the first H1/H2 pair becomes
/// DP/CN.
pub fn citavi_clean(entry: &mut Entry) -> Result<(), TranslateError> {
    if entry.has("CN") || entry.has("DP") {
        return Ok(());
    }
    if !entry.has("H1") && !entry.has("H2") {
        return Ok(());
    }
    if !entry.has("H1") {
        let id = entry.tags["H2"][0];
        if let Some(at) = entry.index_of(id) {
            change_tag(entry, at, &["CN"])?;
        }
        return Ok(());
    }
    // (Upstream's second `!entry.tags.H1` branch, :1260-1264, cannot run.)
    for i in 0..entry.len().saturating_sub(1) {
        if entry.at(i).tag == "H1" && entry.at(i + 1).tag == "H2" {
            change_tag(entry, i, &["DP"])?;
            change_tag(entry, i + 1, &["CN"])?;
            return Ok(());
        }
    }
    Ok(())
}
