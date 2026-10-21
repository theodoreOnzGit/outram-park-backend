// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "WHO.js" (translatorID
//   cd587058-6125-4b33-a876-8c6aae48b5e8, lastUpdated 2022-12-06 12:21:28).
// Copyright (c) 2018-2021 Mario Trojan and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: the search half only, `detectSearch` :154-156 and `doSearch`
//   :158-192 (with its RIS `itemDone` handler); the web half (`detectWeb`,
//   `doWeb`, `scrape` :39-152) is not ported (kovan has no web translation).

//! The WHO search translator (search half): World Health Organization
//! ISBNs (978-92-4) looked up in WHO IRIS as RIS, imported by RIS, with the
//! record page's `citation_pdf_url` added as a PDF attachment.

use crate::zotero::framework::html_dom::query_selector_all;
use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "cd587058-6125-4b33-a876-8c6aae48b5e8",
    label: "WHO",
    creator: "Mario Trojan, Philipp Zumstein, and Abe Jellinek",
    target: "^https?://apps\\.who\\.int/iris/",
    min_version: "3.0",
    priority: 96,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-12-06 12:21:28",
};

/// `detectSearch` (:154-156).
pub fn detect_search(search: &JsObject) -> bool {
    if !search.truthy("ISBN") {
        return false;
    }
    let isbn = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
    clean_isbn(&isbn, false).is_some_and(|i| i.starts_with("978924"))
}

/// `doSearch` (:158-192).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let isbn = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
    let isbn = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
    let url = format!("https://apps.who.int/iris/discover/export?format=refman&list=discover&rpp=10&etal=0&query={isbn}&group_by=none&page=1&filtertype_0=identifier&filter_relational_operator_0=equals&filter_0={isbn}");
    let ris_text = ctx.do_get(&url)?;
    if ris_text.is_empty() {
        return Ok(());
    }
    // risText.replace(/^SE(\s*-\s*[0-9]+).*$/m, 'SP$1'): the first match
    // only; JavaScript's multiline ^/$ and `.` treat \n, \r, U+2028 and
    // U+2029 as line terminators.
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| {
        Regex::new(&format!(
            "(?m)(?:^|[\\r\\x{{2028}}\\x{{2029}}])SE({ws}*-{ws}*[0-9]+)[^\\n\\r\\x{{2028}}\\x{{2029}}]*",
            ws = js::WS
        ))
        .expect("static regex")
    });
    let ris_text = replace_first_se(re, &ris_text);
    for mut item in ctx.child_import(Translator::Ris, &ris_text)? {
        item.set("libraryCatalog", "WHO IRIS");
        item.set("archive", "");
        if item.truthy("url") {
            let page = item.get_string("url").unwrap_or_default();
            let (doc, _) = ctx.process_document(&page)?;
            // attr(recordDoc, 'meta[name="citation_pdf_url"]', 'content')
            let pdf_url =
                query_selector_all(&doc, doc.document(), r#"meta[name="citation_pdf_url"]"#)
                    .first()
                    .and_then(|&m| doc.get_attribute(m, "content"))
                    .map(|s| js::trim(s).to_owned())
                    .unwrap_or_default();
            if !pdf_url.is_empty() {
                let mut a = JsObject::new();
                a.set("title", "Full Text PDF");
                a.set("mimeType", "application/pdf");
                a.set("url", pdf_url);
                item.attachments.push(a);
            }
        }
        ctx.complete(item)?;
    }
    Ok(())
}

/// The first match of the SE fix-up, replaced by `SP$1`; a match that
/// starts at a \r/U+2028/U+2029 terminator keeps that terminator.
fn replace_first_se(re: &Regex, s: &str) -> String {
    let Some(c) = re.captures(s) else {
        return s.to_owned();
    };
    let (Some(m), Some(g)) = (c.get(0), c.get(1)) else {
        return s.to_owned();
    };
    let se = m.as_str().find("SE").unwrap_or(0);
    format!(
        "{}{}SP{}{}",
        &s[..m.start()],
        &m.as_str()[..se],
        g.as_str(),
        &s[m.end()..]
    )
}
