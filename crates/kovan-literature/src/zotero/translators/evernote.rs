// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Evernote.js, "Simple Evernote Export"
//   (translatorID 18dd188a-9afc-4cd6-8775-1980c3ce0fbf, lastUpdated
//   2019-10-11 07:30:00): header :1-15, `doExport` :35-104.
// Copyright (C) 2012 Volodymir Skipa.
// Licence: AGPL-3.0 (upstream: GPL-3.0-or-later, combined under section 13
//   of both licences).

//! The Simple Evernote Export translator: export, an Evernote `.enex`
//! document with one note per item (title, child notes, dates, tags, URL).
//!
//! The item is the legacy export format (minVersion 2.1.9 < 4.0.27):
//! `dateAdded` holds the SQL form of the input's `dateModified` (the
//! endpoint's quirk, ported in the framework) and `dateModified` stays ISO,
//! so `<updated>` ends in "ZZ", as upstream's does. An item without
//! `dateAdded` or `dateModified` (or with a non-string one) makes upstream
//! throw (`item.dateAdded.replace`): the port returns
//! [`TranslateError::Translator`], as the endpoint answers 500 and writes
//! nothing.
//!
//! Lengths (`title.length > 252`, `tag.length > 95`) and `substr` count
//! UTF-16 code units, as upstream does.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_translators.rs`, `evernote_*`).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorItem};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "18dd188a-9afc-4cd6-8775-1980c3ce0fbf",
    label: "Simple Evernote Export",
    creator: "Volodymir Skipa",
    target: "enex",
    min_version: "2.1.9",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportNotes", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2019-10-11 07:30:00",
};

/// `s.length` in UTF-16 code units.
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.substr(0, n)` in UTF-16 code units (a split surrogate pair, which a
/// Rust string cannot hold, becomes U+FFFD).
fn utf16_prefix(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().take(n).collect();
    String::from_utf16_lossy(&units)
}

fn type_error(what: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {what}"))
}

/// `date.replace(/[-:]/g, "").replace(" ", "T") + "Z"` (:71-74).
fn enex_date(item: &TranslatorItem, field: &str) -> Result<String, TranslateError> {
    match item.get(field) {
        Some(Value::String(s)) => {
            let s: String = s.chars().filter(|c| *c != '-' && *c != ':').collect();
            Ok(s.replacen(' ', "T", 1) + "Z")
        }
        _ => Err(type_error(&format!(
            "item.{field}.replace is not a function"
        ))),
    }
}

fn encode_brackets(s: &str) -> String {
    s.replace('<', "&lt;").replace('>', "&gt;")
}

/// `doExport` (:35-104).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static CLASS: OnceLock<Regex> = OnceLock::new();
    static ID: OnceLock<Regex> = OnceLock::new();
    static PAGE: OnceLock<Regex> = OnceLock::new();
    static COMMA: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS.get_or_init(|| Regex::new(r#"(<[^>]*) class="[^"]+"([^>]*>)"#).unwrap());
    let id_re = ID.get_or_init(|| Regex::new(r#"(<[^>]*) id="[^"]+"([^>]*>)"#).unwrap());
    let page_re = PAGE.get_or_init(|| Regex::new(r"</?(html|head|body)[^>]*>").unwrap());
    let comma_re = COMMA.get_or_init(|| Regex::new(&format!("{0}*,{0}*", js::WS)).unwrap());

    let mut out = format!(
        "<?xml version='1.0' encoding='UTF-8'?>\n<!DOCTYPE en-export SYSTEM 'http://xml.evernote.com/pub/evernote-export.dtd'>\n<en-export application='Zotero' version='{}'>\n",
        ctx.options.env.zotero_version
    );
    let export_notes = ctx.options.option_truthy("exportNotes");
    while let Some(item) = ctx.next_item() {
        let mut s = String::from("<note>\n");
        let title = match item.get("title") {
            Some(v) if js::truthy(Some(v)) => match v {
                Value::String(t) => t.clone(),
                _ => return Err(type_error("title.replace is not a function")),
            },
            _ => "[Untitled]".to_owned(),
        };
        let mut title = encode_brackets(&title);
        if utf16_len(&title) > 252 {
            title = utf16_prefix(&title, 252) + "...";
        }
        s += &format!("    <title>{title}</title>\n");
        s += "    <content>\n";
        s += "        <![CDATA[<?xml version='1.0' encoding='UTF-8'?><!DOCTYPE en-note SYSTEM 'http://xml.evernote.com/pub/enml2.dtd'>\n";
        s += "        <en-note>\n";
        if !item.notes.is_empty() && export_notes {
            for n in &item.notes {
                let c = class_re.replace_all(&n.note, "$1$2");
                let c = id_re.replace_all(&c, "$1$2");
                let c = page_re.replace_all(&c, "");
                s += &format!("            <div>{c}<br/></div>\n");
            }
        }
        s += "        </en-note>]]>\n";
        s += "    </content>\n";
        s += &format!(
            "    <created>{}</created>\n",
            enex_date(&item, "dateAdded")?
        );
        s += &format!(
            "    <updated>{}</updated>\n",
            enex_date(&item, "dateModified")?
        );
        for t in &item.tags {
            let tag = comma_re.replace_all(&t.tag, " / ");
            let mut tag = encode_brackets(&tag);
            if utf16_len(&tag) > 95 {
                tag = utf16_prefix(&tag, 95) + "...";
            }
            s += &format!("    <tag>{tag}</tag>\n");
        }
        s += "    <note-attributes>\n";
        s += "        <source>web.clip</source>\n";
        let url = item
            .get("url")
            .map_or("undefined".to_owned(), js::to_js_string);
        s += &format!("        <source-url>{url}</source-url>\n");
        s += "    </note-attributes>\n";
        s += "</note>\n";
        out += &s.replace('&', "&amp;");
    }
    ctx.write(&out);
    ctx.write("</en-export>\n");
    Ok(())
}
