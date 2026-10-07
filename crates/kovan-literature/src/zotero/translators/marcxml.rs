// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARCXML.js" (translatorID
//   edd87d07-9194-42f8-b2ad-997c4c7deefd, lastUpdated 2025-03-28 15:11:06):
//   `detectImport` :37-51, `parseDocument` :57-95, `doImport` :98-106.
// Copyright (c) 2012 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MARCXML translator (import): MARC 21 slim XML records, mapped by
//! MARC's record model ([`super::marc`], upstream's `getTranslatorObject`).
//! METS runs it as a child translator.

use super::marc::{Record, SUBFIELD_DELIMITER};
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::xml::{get_xml, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{ImportContext, TranslateError, TranslatorItem};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "edd87d07-9194-42f8-b2ad-997c4c7deefd",
    label: "MARCXML",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-03-28 15:11:06",
};

/// `detectImport` (:37-51): the MARC slim namespace declared in one of the
/// first seven non-empty lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| {
        Regex::new(r#"xmlns(:marc)?="http://www\.loc\.gov/MARC21/slim""#).expect("regex")
    });
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        if !line.is_empty() {
            if re.is_match(&line) {
                return true;
            }
            // `i++ > 5`
            if i > 5 {
                return false;
            }
            i += 1;
        }
    }
    false
}

const NS: &[(&str, &str)] = &[("marc", "http://www.loc.gov/MARC21/slim")];

/// JavaScript `a + b` of two `ZU.xpathText` results (null is "null").
fn js_concat(a: Option<String>, b: Option<String>) -> String {
    format!(
        "{}{}",
        a.unwrap_or_else(|| "null".into()),
        b.unwrap_or_else(|| "null".into())
    )
}

/// `parseDocument(xml)` (:57-95): one MARC record per `marc:record` with at
/// least one `datafield`.
pub fn parse_document(doc: &XmlDocument) -> Result<Vec<Record>, TranslateError> {
    static CTRL: OnceLock<Regex> = OnceLock::new();
    let ctrl = CTRL.get_or_init(|| Regex::new(r"[\x{80}-\x{9F}]").expect("regex"));
    let mut records = Vec::new();
    for rn in xpath(doc, doc.document(), "//marc:record", NS)? {
        if xpath(doc, rn, "./marc:datafield", NS)?.is_empty() {
            continue;
        }
        let mut record = Record::new();
        record.leader = xpath_text(doc, rn, "./marc:leader", NS, None)?;
        for field in xpath(doc, rn, "./marc:datafield", NS)? {
            let mut tag = String::new();
            for sub in xpath(doc, field, "./marc:subfield", NS)? {
                let code = xpath_text(doc, sub, "./@code", NS, None)?;
                let sf = xpath_text(doc, sub, "./text()", NS, None)?;
                // `if (sf)`: a non-empty string.
                if let Some(sf) = sf.filter(|s| !s.is_empty()) {
                    let sf = ctrl.replace_all(&sf, "");
                    tag = format!(
                        "{tag}{SUBFIELD_DELIMITER}{}{sf}",
                        code.unwrap_or_else(|| "null".into())
                    );
                }
            }
            // :91: the second xpathText has no namespaces (unprefixed @ind2
            // needs none).
            let ind = js_concat(
                xpath_text(doc, field, "./@ind1", NS, None)?,
                xpath_text(doc, field, "./@ind2", &[], None)?,
            );
            let t = xpath_text(doc, field, "./@tag", NS, None)?.unwrap_or_else(|| "null".into());
            record.add_field(&t, &ind, &tag);
        }
        records.push(record);
    }
    Ok(records)
}

/// `doImport` (:98-106).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    for record in parse_document(&doc)? {
        let mut item = TranslatorItem::new("");
        record.translate(&mut item)?;
        ctx.item_done(item);
    }
    Ok(())
}
