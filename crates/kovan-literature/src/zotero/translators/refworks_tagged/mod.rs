// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RefWorks Tagged.js (translatorID
//   1a3506da-a303-4b0a-a1cd-f216e6138d86, lastUpdated 2016-06-21 08:45:20):
//   header :1-18, `detectImport` :28-43, `TagMapper` :348-399; the rest in
//   this module's files.
// Copyright: no notice upstream; translator by Simon Kornblith, Aurimas
//   Vinckevicius and Sebastian Karcher (header `creator`).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! The RefWorks Tagged translator: import and export.
//!
//! | File | Upstream |
//! |---|---|
//! | [`tables`] | type maps, `fieldMap`, `degenerateImportFieldMap`, `exportOrder` (generated from the translator) |
//! | [`import`] | `processTag`, `applyValue`, `dateRWtoZotero`, `completeItem`, `getLine`, `doImport` |
//! | [`export`] | `addTag`, `doExport` |
//!
//! Not ported: `exportedOptions.itemType` (:47-49), which only a translator
//! calling this one through `loadTranslator` can set (it is `false` for a
//! direct import, and that path is ported); `doImport`'s `attachments`
//! argument, likewise only passed by a calling translator; and
//! `saveFile` for attachments on export, which the translation-server never
//! provides (its `ItemGetter.exportFiles` is a no-op, so the attachment URL
//! is exported, as here).
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `refworks_tagged_*`).

pub mod export;
pub mod import;
#[rustfmt::skip]
pub mod tables;

use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};
use crate::zotero::translators::ris::tables::{Sel, TagMap};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "1a3506da-a303-4b0a-a1cd-f216e6138d86",
    label: "RefWorks Tagged",
    creator: "Simon Kornblith, Aurimas Vinckevicius, and Sebastian Karcher",
    target: "txt",
    min_version: "3.0.4",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(true)),
    ],
    hidden_prefs: &[],
    last_updated: "2016-06-21 08:45:20",
};

/// A JavaScript line terminator (what `.` does not match).
pub(crate) fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `detectImport` (:28-43): a line matching `/^RT\s+./` before more than
/// 151 other non-blank lines. (Upstream returns `undefined`, falsy, at the
/// end of the input.)
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        let line = js::trim_start(&line);
        if !line.is_empty() {
            if rt_line(line) {
                return true;
            }
            let before = i;
            i += 1;
            if before > 150 {
                return false;
            }
        }
    }
    false
}

/// `line.search(/^RT\s+./) != -1`: "RT", then whitespace, then one more
/// character that is not a line terminator (`\s+` backtracks, so it may be
/// whitespace too).
fn rt_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("RT") else {
        return false;
    };
    let c: Vec<char> = rest.chars().collect();
    let mut k = 0;
    while k < c.len() && js::is_space(c[k]) {
        k += 1;
        if k < c.len() && !is_line_terminator(c[k]) {
            return true;
        }
    }
    false
}

/// `TagMapper.getFields(itemType, tag)` (:353-399): for each map, the tag's
/// field for the item type. Within a type-dependent map the LAST matching
/// field wins (the loop does not break), `__default` applies when nothing
/// matched and the type is not excluded. Ported with upstream's `var`
/// quirk: `field` and `def` are declared with `var` inside the loop without
/// initialisers, so they keep their values from the previous map; a field
/// found in one map is therefore pushed again for every later map. Only
/// `getFields(...)[0]` is read, which is the first map's field when it has
/// one.
pub fn get_fields(
    maps: &[&'static [(&'static str, TagMap)]],
    item_type: &str,
    tag: &str,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    let mut field: Option<&'static str> = None;
    let mut def: Option<&'static str> = None;
    for map in maps {
        match map.iter().find(|(t, _)| *t == tag).map(|(_, m)| m) {
            Some(TagMap::ByType(entries)) => {
                let mut exclude = false;
                for (f, sel) in entries.iter() {
                    if *f == "__default" {
                        if let Sel::Field(d) = sel {
                            def = Some(d);
                        }
                        continue;
                    }
                    if *f == "__exclude" {
                        if let Sel::Types(types) = sel {
                            if types.contains(&item_type) {
                                exclude = true;
                            }
                        }
                        continue;
                    }
                    let hit = match sel {
                        Sel::Types(types) => types.contains(&item_type),
                        Sel::Field(s) => s.contains(item_type),
                    };
                    if hit {
                        field = Some(f);
                    }
                }
                if field.is_none() && def.is_some() && !exclude {
                    field = def;
                }
            }
            Some(TagMap::Field(f)) => field = Some(f),
            None => {}
        }
        if let Some(f) = field.filter(|f| !f.is_empty()) {
            fields.push(f);
        }
    }
    fields
}

/// `doImport` (:775-807).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    import::do_import(ctx)
}

/// `doExport` (:843-971).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    export::do_export(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rt_line_like_upstream() {
        assert!(rt_line("RT Book, Whole"));
        assert!(rt_line("RT  "));
        assert!(!rt_line("RT "));
        assert!(!rt_line("RTX"));
    }
}
