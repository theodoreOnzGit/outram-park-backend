// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "METS.js" (translatorID
//   5c6895a1-b6a9-4939-9c65-43f8ae0ef096, lastUpdated 2021-07-13 20:44:34):
//   header :1-15, `detectImport` :41-55, `doImport` :57-76,
//   `createAttachments` :78-116, `extractData` :118-132, `processData`
//   :134-171, `callImport` :173-203. `atob` is the WHATWG HTML Standard's
//   "forgiving-base64 decode" as jsdom 29.0.1 implements `window.atob`.
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The METS translator: import. The descriptive metadata in each
//! `dmdSec` is handed to another translator (MODS, or for MARC the MARCXML
//! translator) as a child translation.
//!
//! Not ported: MAB2 (the third entry of the MARC translator list), which
//! never runs: an import given an array of translators runs its first one
//! only ([`crate::zotero::framework::child`]).
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_xml_translators.rs`, `mets_*`).

use super::{marcxml, mods};
use crate::zotero::framework::child::run_child_import;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::{get_xml, NodeId, XmlDocument};
use crate::zotero::framework::{ImportContext, JsObject, TranslateError, TranslatorItem};
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "5c6895a1-b6a9-4939-9c65-43f8ae0ef096",
    label: "METS",
    creator: "Abe Jellinek",
    target: "xml",
    min_version: "3.0",
    priority: 50,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-07-13 20:44:34",
};

/// `detectImport` (:41-55): `<mets` in one of the first nine non-empty
/// lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        if !line.is_empty() {
            if line.contains("<mets") {
                return true;
            }
            let before = i;
            i += 1;
            if before > 7 {
                return false;
            }
        }
    }
    false
}

/// `doImport` (:57-76).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let xml = get_xml(ctx)?;
    for mets in xml.query_selector_all(xml.document(), "mets") {
        let attachments = create_attachments(&xml, mets);
        for dmd in xml.query_selector_all(mets, "dmdSec") {
            let Some(md_wrap) = xml.query_selector(dmd, "mdWrap") else {
                // Upstream logs, then reads `mdWrap.getAttribute` of null.
                return Err(TranslateError::Translator(
                    "TypeError: Cannot read properties of null (reading 'getAttribute')".into(),
                ));
            };
            let md_type = xml.get_attribute(md_wrap, "MDTYPE").map(str::to_owned);
            let data = extract_data(&xml, md_wrap)?;
            process_data(ctx, md_type.as_deref(), &data, &attachments)?;
        }
    }
    Ok(())
}

/// `createAttachments` (:78-116).
fn create_attachments(xml: &XmlDocument, mets: NodeId) -> Vec<JsObject> {
    let mut attachments = Vec::new();
    for file in xml.query_selector_all(mets, "fileSec file") {
        if attachments.len() >= 5 {
            attachments.clear();
            break;
        }
        let mime = xml.get_attribute(file, "MIMETYPE").unwrap_or("");
        let url = xml
            .query_selector(file, "FLocat[LOCTYPE=\"URL\"]")
            .and_then(|l| xml.get_attribute(l, "xlink:href"))
            .unwrap_or("");
        if !mime.is_empty() && !url.is_empty() {
            let title = if mime == "application/pdf" {
                "PDF"
            } else if mime.starts_with("audio/") {
                "Audio"
            } else if mime.starts_with("video/") {
                "Video"
            } else if mime.starts_with("image/") {
                "Image"
            } else {
                "Attachment"
            };
            let mut a = JsObject::new();
            a.set("title", title);
            a.set("mimeType", mime);
            a.set("url", url);
            attachments.push(a);
        }
    }
    attachments
}

/// `extractData` (:118-132).
fn extract_data(xml: &XmlDocument, md_wrap: NodeId) -> Result<String, TranslateError> {
    let inner = |n| xml.inner_html(n).map_err(TranslateError::Translator);
    for child in xml.element_children(md_wrap) {
        let tag = xml.tag_name(child);
        if tag.ends_with("xmlData") {
            return inner(child);
        } else if tag.ends_with("binData") {
            return atob(&inner(child)?);
        }
    }
    inner(md_wrap)
}

/// `window.atob` (forgiving-base64 decode): the bytes as a Latin-1 string;
/// invalid input throws `InvalidCharacterError`.
pub fn atob(s: &str) -> Result<String, TranslateError> {
    let bad = || {
        TranslateError::Translator(
            "InvalidCharacterError: The string to be decoded contains invalid characters.".into(),
        )
    };
    let mut d: String = s
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' '))
        .collect();
    if d.len() % 4 == 0 {
        if d.ends_with("==") {
            d.truncate(d.len() - 2);
        } else if d.ends_with('=') {
            d.truncate(d.len() - 1);
        }
    }
    if d.len() % 4 == 1 {
        return Err(bad());
    }
    let val = |c: char| -> Option<u32> {
        Some(match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        })
    };
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in d.chars() {
        buf = (buf << 6) | val(c).ok_or_else(bad)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(char::from(((buf >> bits) & 0xFF) as u8));
        }
    }
    Ok(out)
}

/// The item-modifying callback of `processData` (MODS: `callNumber = ''`).
#[derive(Clone, Copy)]
enum Callback {
    None,
    ClearCallNumber,
}

/// `processData` (:134-171).
fn process_data(
    ctx: &mut ImportContext,
    md_type: Option<&str>,
    data: &str,
    attachments: &[JsObject],
) -> Result<(), TranslateError> {
    match md_type {
        // MARCXML, then MARC, then MAB2: only the first runs (child.rs).
        Some("MARC") => call_import(
            ctx,
            data,
            &marcxml::METADATA,
            marcxml::do_import,
            attachments,
            Callback::None,
        ),
        Some("MODS") => call_import(
            ctx,
            data,
            &mods::METADATA,
            mods::do_import,
            attachments,
            Callback::ClearCallNumber,
        ),
        Some("OTHER") => Ok(()),
        other => Err(TranslateError::Translator(format!(
            "Unsupported metadata type: {}",
            other.unwrap_or("null")
        ))),
    }
}

/// `callImport` (:173-203): the child translation and its `itemDone`
/// handler.
fn call_import(
    ctx: &mut ImportContext,
    data: &str,
    meta: &'static TranslatorMetadata,
    run: fn(&mut ImportContext) -> Result<(), TranslateError>,
    attachments: &[JsObject],
    callback: Callback,
) -> Result<(), TranslateError> {
    for mut item in run_child_import(ctx, data, meta, run)? {
        if !enough_fields(&item) {
            continue;
        }
        item.attachments.extend(attachments.iter().cloned());
        if item.get_str("language") == Some("zxx") {
            item.remove("language");
        }
        if let Callback::ClearCallNumber = callback {
            item.set("callNumber", "");
        }
        ctx.item_done(item);
    }
    Ok(())
}

/// More than one non-empty string property besides `itemType` (:176-190).
fn enough_fields(item: &TranslatorItem) -> bool {
    item.props
        .iter()
        .filter(|(_, v)| matches!(v, Value::String(s) if !s.is_empty()))
        .count()
        > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atob_forgiving_base64() {
        assert_eq!(atob("aGk=").unwrap(), "hi");
        assert_eq!(atob(" aG k ").unwrap(), "hi");
        assert_eq!(atob("/w==").unwrap(), "\u{ff}");
        assert!(atob("a").is_err());
        assert!(atob("a*bc").is_err());
    }
}
