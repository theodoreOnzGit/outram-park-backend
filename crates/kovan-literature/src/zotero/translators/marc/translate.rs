// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARC.js" (translatorID
//   a6ee60df-1ddc-4aae-bb25-45e0537be973, lastUpdated 2025-03-28 15:43:42):
//   `record.prototype._associateDBField` :274-315, `_associateNotes`
//   :318-332, `_associateTags` :335-347, `translate` :350-836.
// Copyright (c) 2020 Simon Kornblith, Sylvain Machefert.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `record.translate(item)`: a MARC record (MARC 21 or UNIMARC) into an
//! item.

use super::record::{clean, glue_together, pull_isbn, pull_number, re, Record, SUBFIELD_DELIMITER};
use crate::zotero::framework::js;
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::{
    TranslateError, TranslatorCreator, TranslatorItem, TranslatorNote, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The `execMe` callbacks `_associateDBField` is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exec {
    None,
    PullIsbn,
    PullNumber,
    /// `author(value, type, useComma)` = `ZU.cleanAuthor`.
    Author(&'static str, bool),
}

fn creator(author: &str, ty: &str, use_comma: bool) -> TranslatorCreator {
    let a = clean_author(author, ty, use_comma);
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

fn institution(name: String, ty: &str) -> TranslatorCreator {
    TranslatorCreator {
        last_name: Some(name),
        creator_type: Some(ty.to_owned()),
        field_mode: Some(1),
        ..Default::default()
    }
}

fn type_error(what: &str) -> TranslateError {
    TranslateError::Translator(format!(
        "TypeError: Cannot read properties of undefined (reading '{what}')"
    ))
}

/// `item[key] = value` where `value` may be undefined (`None`): the
/// property exists either way; `_itemDone` drops an undefined one.
fn set_opt(item: &mut TranslatorItem, key: &str, value: Option<&str>) {
    item.set(
        key,
        value.map_or(Value::Null, |v| Value::String(v.to_owned())),
    );
}

impl Record {
    /// `_associateDBField(item, fieldNo, part, fieldName, execMe, ...)`
    /// (:274-315).
    fn associate_db_field(
        &self,
        item: &mut TranslatorItem,
        field_no: &str,
        part: &str,
        field_name: &str,
        exec: Exec,
    ) {
        for f in self.get_field_subfields(field_no) {
            let mut value: Option<String> = None;
            for p in part.chars() {
                let p = p.to_string();
                if let Some(v) = f.get(&p).filter(|v| !v.is_empty()) {
                    value = Some(match value {
                        Some(prev) if !prev.is_empty() => format!("{prev} {v}"),
                        _ => v.to_owned(),
                    });
                }
            }
            let Some(v) = value.filter(|v| !v.is_empty()) else {
                continue;
            };
            let v = clean(Some(&v)).unwrap_or_default();
            if let Exec::Author(ty, comma) = exec {
                // execMe returns a creator object; the only field name it is
                // used with is "creator".
                item.creators.push(creator(&v, ty, comma));
                continue;
            }
            let v = match exec {
                Exec::PullIsbn => pull_isbn(&v),
                Exec::PullNumber => pull_number(&v),
                _ => v,
            };
            if field_name == "ISBN" {
                if !item.truthy("ISBN") {
                    item.set("ISBN", v);
                } else {
                    let prev = item.get_string("ISBN").unwrap_or_default();
                    item.set("ISBN", format!("{prev} {v}"));
                }
            } else {
                item.set(field_name, v);
                return;
            }
        }
    }

    /// `_associateNotes(item, fieldNo, part)` (:318-332).
    fn associate_notes(&self, item: &mut TranslatorItem, field_no: &str, part: &str) {
        let mut texts = Vec::new();
        for f in self.get_field_subfields(field_no) {
            for p in part.chars() {
                if let Some(v) = f.get(&p.to_string()).filter(|v| !v.is_empty()) {
                    texts.push(clean(Some(v)).unwrap_or_default());
                }
            }
        }
        let text = texts.join(" ");
        if !js::trim(&text).is_empty() {
            item.notes.push(TranslatorNote::new(text));
        }
    }

    /// `_associateTags(item, fieldNo, part)` (:335-347).
    fn associate_tags(&self, item: &mut TranslatorItem, field_no: &str, part: &str) {
        for f in self.get_field_subfields(field_no) {
            for p in part.chars() {
                if let Some(v) = f.get(&p.to_string()).filter(|v| !v.is_empty()) {
                    item.tags
                        .push(TranslatorTag::new(clean(Some(v)).unwrap_or_default()));
                }
            }
        }
    }

    fn has(&self, tag: &str) -> bool {
        !self.get_field(tag).is_empty()
    }

    /// `translate(item)` (:350-836).
    pub fn translate(&self, item: &mut TranslatorItem) -> Result<(), TranslateError> {
        let leader = self.leader.as_deref().filter(|l| !l.is_empty());
        if item.item_type.is_empty() {
            if let Some(l) = leader {
                let t = super::record::substr(l, 6.0, Some(1.0));
                let ty = match t.as_str() {
                    "g" => "film",
                    "j" | "i" => "audioRecording",
                    "e" | "f" => "map",
                    "k" => "artwork",
                    "t" | "b" => "manuscript",
                    _ => "",
                };
                item.item_type = ty.to_owned();
            }
        }
        if item.item_type.is_empty() {
            item.item_type = "book".into();
        }

        if self.has("200") && !self.has("245") {
            self.translate_unimarc(item)?;
        } else {
            self.translate_marc21(item)?;
        }

        // :819-835
        if item.item_type == "book" {
            let has_author = item
                .creators
                .iter()
                .any(|c| c.creator_type.as_deref() == Some("author"));
            if !has_author {
                for c in &mut item.creators {
                    if c.creator_type.as_deref() == Some("contributor") {
                        c.creator_type = Some("editor".into());
                    }
                }
            }
        }
        Ok(())
    }

    /// The UNIMARC branch (:381-489).
    fn translate_unimarc(&self, item: &mut TranslatorItem) -> Result<(), TranslateError> {
        if self.has("328") {
            item.item_type = "thesis".into();
        }
        self.associate_db_field(item, "010", "a", "ISBN", Exec::PullIsbn);
        self.associate_db_field(item, "011", "a", "ISSN", Exec::PullIsbn);
        for tag in ["700", "701", "702"] {
            for aut in self.get_field_subfields(tag) {
                let text = match (
                    aut.get("b").filter(|s| !s.is_empty()),
                    aut.get("a").filter(|s| !s.is_empty()),
                ) {
                    (Some(b), Some(a)) => {
                        static R: OnceLock<Regex> = OnceLock::new();
                        let a = re(&R, || format!(r",{}*$", js::WS)).replace(a, "");
                        Some(format!("{a}, {b}"))
                    }
                    _ => aut.get("a").map(str::to_owned),
                };
                if let Some(t) = text.filter(|t| !t.is_empty()) {
                    item.creators.push(creator(&t, "author", true));
                }
            }
        }
        for tag in ["710", "711", "712"] {
            for aut in self.get_field_subfields(tag) {
                if let Some(a) = aut.get("a").filter(|s| !s.is_empty()) {
                    item.creators.push(institution(a.to_owned(), "contributor"));
                }
            }
        }
        self.associate_db_field(item, "101", "a", "language", Exec::None);
        self.associate_db_field(item, "328", "a", "abstractNote", Exec::None);
        self.associate_db_field(item, "330", "a", "abstractNote", Exec::None);
        self.associate_tags(item, "610", "a");
        self.associate_db_field(item, "206", "a", "scale", Exec::None);
        // :437-442: chop off translations at the first $d.
        let raw = self
            .get_field("200")
            .first()
            .map(|f| f.1.clone())
            .ok_or_else(|| type_error("1"))?;
        static D: OnceLock<Regex> = OnceLock::new();
        let raw = re(&D, || format!("{SUBFIELD_DELIMITER}d.+"))
            .replace(&raw, "")
            .into_owned();
        let title = self.extract_subfields(&raw);
        let t = glue_together(clean(title.get("a")), clean(title.get("e")), Some(": "));
        set_opt(item, "title", t.as_deref());
        self.associate_db_field(item, "205", "a", "edition", Exec::None);
        let film = item.item_type == "film";
        let pub_field = if film { "distributor" } else { "publisher" };
        let f = if self.has("214") { "214" } else { "210" };
        self.associate_db_field(item, f, "a", "place", Exec::None);
        self.associate_db_field(item, f, "c", pub_field, Exec::None);
        self.associate_db_field(item, f, "d", "date", Exec::PullNumber);
        self.associate_db_field(item, "225", "a", "series", Exec::None);
        self.associate_db_field(item, "225", "v", "seriesNumber", Exec::None);
        self.associate_db_field(item, "686", "ab", "callNumber", Exec::None);
        self.associate_db_field(item, "676", "a", "callNumber", Exec::None);
        self.associate_db_field(item, "675", "a", "callNumber", Exec::None);
        self.associate_db_field(item, "680", "ab", "callNumber", Exec::None);
        Ok(())
    }

    /// The MARC 21 branch (:490-817).
    fn translate_marc21(&self, item: &mut TranslatorItem) -> Result<(), TranslateError> {
        if self.has("502") && !self.has("020") {
            item.item_type = "thesis".into();
        }
        self.associate_db_field(item, "020", "a", "ISBN", Exec::PullIsbn);
        self.associate_db_field(item, "022", "a", "ISSN", Exec::PullIsbn);
        self.associate_db_field(item, "041", "a", "language", Exec::None);
        let relaterm = |k: &str| -> Option<&'static str> {
            Some(match k {
                "act" => "castMember",
                "asn" => "contributor",
                "aut" => "author",
                "cmp" => "composer",
                "ctb" => "contributor",
                "drt" => "director",
                "edt" => "editor",
                "pbl" | "pub" => "SKIP",
                "prf" => "performer",
                "pro" => "producer",
                "trl" => "translator",
                _ => return None,
            })
        };
        let has100 = self.has("100");
        for tag in ["100", "110", "700", "710", "720"] {
            for af in self.get_field_subfields(tag) {
                let Some(a) = af.get("a").filter(|s| !s.is_empty()) else {
                    continue;
                };
                let rel = af.get("4").filter(|s| !s.is_empty()).and_then(relaterm);
                if rel == Some("SKIP") {
                    continue;
                }
                let mut c = match tag {
                    "100" | "700" => creator(a, "author", true),
                    "720" => creator(a, "contributor", true),
                    _ => {
                        static L: OnceLock<Regex> = OnceLock::new();
                        static T: OnceLock<Regex> = OnceLock::new();
                        static S: OnceLock<Regex> = OnceLock::new();
                        let ws = &js::WS[1..js::WS.len() - 1];
                        let v = re(&L, || format!(r"^[{ws}\x{{A0}}.,/\[\]:]+")).replace(a, "");
                        let v = re(&T, || format!(r"[{ws}\x{{A0}}.,/\[\]:]+$")).replace(&v, "");
                        let v = re(&S, || format!(r"[{ws}\x{{A0}}]+")).replace(&v, " ");
                        institution(v.into_owned(), "contributor")
                    }
                };
                if tag == "700" && !has100 && item.item_type == "book" {
                    c.creator_type = Some("editor".into());
                }
                if let Some(r) = rel {
                    c.creator_type = Some(r.into());
                }
                item.creators.push(c);
            }
        }
        self.associate_db_field(item, "111", "a", "meetingName", Exec::None);
        self.associate_db_field(item, "711", "a", "meetingName", Exec::None);

        if item.item_type == "book" && item.creators.is_empty() {
            if let Some(f) = self.get_field_subfields("600").first() {
                let a = f.get("a").ok_or_else(|| type_error("replace"))?;
                item.creators.push(creator(a, "author", true));
            }
        }

        for (tag, parts) in [
            ("600", "aqtxyzv"),
            ("610", "abxyzv"),
            ("611", "abtxyzv"),
            ("630", "acetxyzv"),
            ("648", "atxyzv"),
            ("650", "axyzv"),
            ("651", "abcxyzv"),
            ("653", "axyzv"),
            ("654", "abcyzv"),
            ("655", "abcxyzv"),
            ("656", "axyzv"),
            ("657", "axyzv"),
            ("658", "ab"),
            ("662", "abcdfgh"),
        ] {
            self.associate_tags(item, tag, parts);
        }

        self.associate_notes(item, "500", "a");
        self.associate_notes(item, "502", "a");
        self.associate_notes(item, "505", "art");
        if !item.truthy("abstractNote") && self.get_field("520").len() == 1 {
            self.associate_db_field(item, "520", "ab", "abstractNote", Exec::None);
        } else {
            self.associate_notes(item, "520", "ab");
        }
        self.associate_notes(item, "545", "ab");

        // :624-629: `this.getFieldSubfields("245")[0]` must exist.
        let ts = self
            .get_field_subfields("245")
            .into_iter()
            .next()
            .ok_or_else(|| type_error("a"))?;
        let title = glue_together(
            glue_together(clean(ts.get("a")), clean(ts.get("b")), Some(": ")),
            glue_together(clean(ts.get("n")), clean(ts.get("p")), Some(": ")),
            Some(". "),
        );
        set_opt(item, "title", title.as_deref());

        self.associate_db_field(item, "250", "a", "edition", Exec::None);
        self.associate_db_field(item, "260", "a", "place", Exec::None);
        if item.item_type == "film" {
            self.associate_db_field(item, "260", "b", "distributor", Exec::None);
        } else {
            self.associate_db_field(item, "260", "b", "publisher", Exec::None);
        }
        self.associate_db_field(item, "260", "c", "date", Exec::PullNumber);
        self.associate_db_field(item, "300", "a", "numPages", Exec::PullNumber);
        self.associate_db_field(item, "490", "a", "series", Exec::None);
        self.associate_db_field(item, "490", "v", "seriesNumber", Exec::None);
        self.associate_db_field(item, "440", "a", "series", Exec::None);
        self.associate_db_field(item, "440", "v", "seriesNumber", Exec::None);
        for (tag, parts) in [
            ("084", "ab"),
            ("082", "a"),
            ("080", "ab"),
            ("070", "ab"),
            ("060", "ab"),
            ("050", "ab"),
            ("090", "ab"),
            ("099", "a"),
            ("852", "khim"),
        ] {
            self.associate_db_field(item, tag, parts, "callNumber", Exec::None);
        }
        // :674-678
        if let Some(cn) = self.get_field_subfields("035").first() {
            if let Some(a) = cn.get("a").filter(|a| !a.is_empty()) {
                if a.starts_with("(OCoLC)") {
                    item.set(
                        "extra",
                        format!("OCLC: {}", super::record::substr(a, 7.0, None)),
                    );
                }
            }
        }
        self.associate_db_field(item, "245", "h", "medium", Exec::None);
        let medium = item.get_str("medium");
        if medium == Some("electronic resource") || medium == Some("Elektronische Ressource") {
            self.associate_db_field(item, "856", "u", "url", Exec::None);
        }
        if !item.truthy("place") {
            self.associate_db_field(item, "264", "a", "place", Exec::None);
        }
        if !item.truthy("publisher") {
            self.associate_db_field(item, "264", "b", "publisher", Exec::None);
        }
        if !item.truthy("date") {
            self.associate_db_field(item, "264", "c", "date", Exec::PullNumber);
        }
        // German :691-699
        if !item.truthy("place") {
            self.associate_db_field(item, "410", "a", "place", Exec::None);
        }
        if !item.truthy("publisher") {
            self.associate_db_field(item, "412", "a", "publisher", Exec::None);
        }
        if !item.truthy("title") {
            self.associate_db_field(item, "331", "a", "title", Exec::None);
        }
        if !item.truthy("title") {
            self.associate_db_field(item, "1300", "a", "title", Exec::None);
        }
        if !item.truthy("date") {
            self.associate_db_field(item, "425", "a", "date", Exec::PullNumber);
        }
        if !item.truthy("date") {
            self.associate_db_field(item, "595", "a", "date", Exec::PullNumber);
        }
        if self.has("104") {
            self.associate_db_field(item, "104", "a", "creator", Exec::Author("author", true));
        }
        if self.has("800") {
            self.associate_db_field(item, "800", "a", "creator", Exec::Author("author", true));
        }
        // Spanish :702-724 (`!item.creators` is never true: creators is an
        // array, so the 700-702 fallback never runs).
        if !item.truthy("title") {
            self.associate_db_field(item, "200", "a", "title", Exec::None);
        }
        if !item.truthy("place") {
            self.associate_db_field(item, "210", "a", "place", Exec::None);
        }
        if !item.truthy("publisher") {
            self.associate_db_field(item, "210", "c", "publisher", Exec::None);
        }
        if !item.truthy("date") {
            self.associate_db_field(item, "210", "d", "date", Exec::None);
        }
        if item.truthy("title") {
            let t = item.get_string("title").unwrap_or_default();
            item.set("title", capitalize_title(&t, false));
        }
        if let Some(f) = self.get_field_subfields("335").first() {
            let t = item
                .get_string("title")
                .unwrap_or_else(|| "undefined".into());
            let a = f.get("a").unwrap_or("undefined");
            item.set("title", format!("{t}: {a}"));
        }
        for id in self.get_field_subfields("024") {
            if id.get("2") == Some("doi") {
                set_opt(item, "DOI", id.get("a"));
            }
        }
        if let Some(container) = self.get_field_subfields("773").into_iter().next() {
            self.translate_container(item, &container);
        }
        Ok(())
    }

    /// The host item (773) :739-816.
    fn translate_container(&self, item: &mut TranslatorItem, c: &super::record::Subfields) {
        match c.get("7").unwrap_or("") {
            "nnam" => item.item_type = "bookSection".into(),
            "nnas" => item.item_type = "journalArticle".into(),
            "m2am" => item.item_type = "conferencePaper".into(),
            _ => {
                if c.truthy("t") && c.truthy("z") {
                    item.item_type = "bookSection".into();
                } else if c.truthy("t") {
                    item.item_type = "journalArticle".into();
                }
            }
        }
        let d = r"[0-9]";
        let ws = js::WS;
        let publication = c.get("t");
        if item.item_type == "bookSection" || item.item_type == "conferencePaper" {
            if let Some(pubinfo) = c.get("d").filter(|s| !s.is_empty()) {
                static P1: OnceLock<Regex> = OnceLock::new();
                static P2: OnceLock<Regex> = OnceLock::new();
                static P3: OnceLock<Regex> = OnceLock::new();
                item.set(
                    "place",
                    re(&P1, || ":.+".into()).replace(pubinfo, "").into_owned(),
                );
                if let Some(m) = re(&P2, || format!(r":{ws}*(.+),{ws}*{d}{{4}}")).captures(pubinfo)
                {
                    item.set("publisher", m[1].to_owned());
                }
                if let Some(m) = re(&P3, || format!(r",{ws}*({d}{{4}})")).captures(pubinfo) {
                    item.set("date", m[1].to_owned());
                }
            }
            if let Some(publication) = publication.filter(|s| !s.is_empty()) {
                static DOT: OnceLock<Regex> = OnceLock::new();
                let pt = re(&DOT, || r"\..*".into())
                    .replace(publication, "")
                    .into_owned();
                if item.item_type == "bookSection" {
                    item.set("bookTitle", pt);
                } else {
                    item.set("proceedingsTitle", pt);
                }
                if publication.contains("Edited by") {
                    static ED: OnceLock<Regex> = OnceLock::new();
                    static SPLIT: OnceLock<Regex> = OnceLock::new();
                    if let Some(m) =
                        re(&ED, || format!(r"Edited by{ws}+(.+)\.?")).captures(publication)
                    {
                        let eds = m[1].to_owned();
                        let sp = re(&SPLIT, || format!(r"{ws}+and{ws}+|{ws}*,{ws}*|{ws}*;{ws}*"));
                        for e in sp.split(&eds) {
                            item.creators.push(creator(e, "editor", false));
                        }
                    }
                }
            }
            if let Some(pages) = c.get("g").filter(|s| !s.is_empty()) {
                static G1: OnceLock<Regex> = OnceLock::new();
                static G2: OnceLock<Regex> = OnceLock::new();
                let m = re(&G1, || format!(r"[ps]\.{ws}*({d}+(-{d}+)?)"))
                    .captures(pages)
                    .or_else(|| re(&G2, || format!(r"({d}+-{d}+)")).captures(pages));
                if let Some(m) = m {
                    item.set("pages", m[1].to_owned());
                }
            }
            if let Some(event) = c.get("a").filter(|s| !s.is_empty()) {
                item.set("conferenceName", event.replace(['{', '}'], ""));
            }
            set_opt(item, "ISBN", c.get("z"));
        } else {
            if let Some(publication) = publication.filter(|s| !s.is_empty()) {
                static T: OnceLock<Regex> = OnceLock::new();
                let p =
                    re(&T, || format!(r"[.,{}]+$", &ws[1..ws.len() - 1])).replace(publication, "");
                item.set("publicationTitle", p.into_owned());
            }
            set_opt(item, "journalAbbreviation", c.get("p"));
            if let Some(loc) = c.get("g").filter(|s| !s.is_empty()) {
                static G1: OnceLock<Regex> = OnceLock::new();
                static G2: OnceLock<Regex> = OnceLock::new();
                static DATE: OnceLock<Regex> = OnceLock::new();
                static VOL: OnceLock<Regex> = OnceLock::new();
                static ISS: OnceLock<Regex> = OnceLock::new();
                static VI: OnceLock<Regex> = OnceLock::new();
                let m = re(&G1, || format!(r"[ps]\.{ws}*({d}+(-{d}+)?)"))
                    .captures(loc)
                    .or_else(|| re(&G2, || format!(r"({d}{d}+-{d}+)")).captures(loc));
                if let Some(m) = m {
                    item.set("pages", m[1].to_owned());
                }
                let date = re(&DATE, || {
                    format!(
                        r"((Jan(uary)?|Feb(ruary)?|Mar(ch)?|Apr(il)?|May|Jun(e)?|Jul(y)?|Aug(ust)?|Sep(tember)?|Oct(ober)?|Nov(ember)?|Dec(ember)?)\.?{ws}*)?{d}{{4}}"
                    )
                });
                if let Some(m) = date.find(loc) {
                    item.set("date", m.as_str().to_owned());
                }
                // JS /i is ASCII-and-simple folding here: the patterns are ASCII.
                if let Some(m) =
                    re(&VOL, || format!(r"(?i)(?:vol\.|bd\.){ws}*({d}+)")).captures(loc)
                {
                    item.set("volume", m[1].to_owned());
                }
                if let Some(m) = re(&ISS, || {
                    format!(r"(?i)(?:vol\.|bd\.){ws}*{d}+{ws}*,{ws}*(?:no\.|nr\.){ws}*({d}[0-9/]*)")
                })
                .captures(loc)
                {
                    item.set("issue", m[1].to_owned());
                }
                if !item.truthy("volume") {
                    if let Some(m) = re(&VI, || format!(r"({d}+):({d}+)")).captures(loc) {
                        item.set("volume", m[1].to_owned());
                        item.set("issue", m[2].to_owned());
                    }
                }
                set_opt(item, "ISSN", c.get("x"));
            }
        }
    }
}
