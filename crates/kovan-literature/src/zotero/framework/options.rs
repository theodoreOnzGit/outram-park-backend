// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate
//   (commit e0fe482b8a07): src/translation/translate.js `getOption` :245-252,
//   `getHiddenPref` :260-273, display-option defaults :1322; the translator
//   header format (translator.js); translation-server
//   (https://github.com/zotero/translation-server, commit 3a9d17614896)
//   src/zotero.js (`Zotero.version` :35) and exportEndpoint.js (legacy mode,
//   :59). Zotero utilities (commit 4051881d59c6; the same in the server's
//   1dd38e27edf8) utilities.js `semverCompare` :1719-1727.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Translator metadata (the JSON header of each translator file) and the
//! options a translation runs with.

use super::item::JsObject;
use kovan_common::zotero::date::DateOptions;
use serde_json::Value;

/// A default value in a translator header (`displayOptions`,
/// `hiddenPrefs`, `configOptions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderValue {
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(&'static str),
}

impl HeaderValue {
    /// As JSON.
    pub fn to_value(self) -> Value {
        match self {
            HeaderValue::Bool(b) => Value::Bool(b),
            HeaderValue::Str(s) => Value::String(s.to_owned()),
        }
    }
}

/// `translatorType` bits.
pub mod translator_type {
    /// Import.
    pub const IMPORT: u8 = 1;
    /// Export.
    pub const EXPORT: u8 = 2;
    /// Web.
    pub const WEB: u8 = 4;
    /// Search.
    pub const SEARCH: u8 = 8;
}

/// A translator's header, verbatim from its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslatorMetadata {
    /// `translatorID`.
    pub id: &'static str,
    /// `label`.
    pub label: &'static str,
    /// `creator`.
    pub creator: &'static str,
    /// `target` (for import/export translators, the file extension).
    pub target: &'static str,
    /// `minVersion`.
    pub min_version: &'static str,
    /// `priority` (lower is tried first).
    pub priority: u32,
    /// `translatorType` ([`translator_type`] bits).
    pub translator_type: u8,
    /// `configOptions`.
    pub config_options: &'static [(&'static str, HeaderValue)],
    /// `displayOptions`: the options and their defaults.
    pub display_options: &'static [(&'static str, HeaderValue)],
    /// `hiddenPrefs`: defaults for `Zotero.getHiddenPref`.
    pub hidden_prefs: &'static [(&'static str, HeaderValue)],
    /// `lastUpdated`.
    pub last_updated: &'static str,
}

impl TranslatorMetadata {
    /// Whether the translator imports.
    pub fn can_import(&self) -> bool {
        self.translator_type & translator_type::IMPORT != 0
    }

    /// Whether the translator exports.
    pub fn can_export(&self) -> bool {
        self.translator_type & translator_type::EXPORT != 0
    }

    /// Whether the framework gives this translator the legacy (pre-4.0.27)
    /// export item format: `semverCompare('4.0.27', minVersion) > 0`
    /// (translate.js:2568, exportEndpoint.js:59).
    pub fn legacy_export(&self) -> bool {
        semver_compare("4.0.27", self.min_version) == std::cmp::Ordering::Greater
    }

    /// The display options with their defaults (what `getOption` returns when
    /// the caller sets none: translate.js:1322).
    pub fn default_display_options(&self) -> JsObject {
        self.display_options
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.to_value()))
            .collect()
    }
}

/// `Zotero.Utilities.semverCompare(a, b)` (utilities.js:1719-1727): split
/// on ".", compare components pairwise (as integers when `parseInt` reads
/// one, else as strings; a number against a string compares false both ways
/// in JavaScript, so they count as equal), and when every shared component
/// is equal the version with more components is greater.
pub fn semver_compare(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    #[derive(PartialEq)]
    enum Part {
        Num(i64),
        Str(String),
    }
    // parseInt: optional leading whitespace and sign, then digits.
    let parse = |s: &str| -> Vec<Part> {
        s.split('.')
            .map(|v| {
                let t = super::js::trim_start(v);
                let (neg, digits) = match t.strip_prefix('-') {
                    Some(r) => (true, r),
                    None => (false, t.strip_prefix('+').unwrap_or(t)),
                };
                let d: String = digits.chars().take_while(char::is_ascii_digit).collect();
                match d.parse::<i64>() {
                    Ok(n) => Part::Num(if neg { -n } else { n }),
                    Err(_) => Part::Str(v.to_owned()),
                }
            })
            .collect()
    };
    let (pa, pb) = (parse(a), parse(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let o = match (x, y) {
            (Part::Num(x), Part::Num(y)) => x.cmp(y),
            (Part::Str(x), Part::Str(y)) => x.cmp(y),
            _ => Ordering::Equal,
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len())
}

/// What the translation environment supplies besides the translator's own
/// options. The defaults are the Zotero translation-server's: version
/// "5.0.97" (`Zotero.version`, translation-server zotero.js:35), en-US dates
/// without the client's day suffixes, UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationEnv {
    /// `Zotero.Utilities.getVersion()`.
    pub zotero_version: String,
    /// Date parsing and the "local" time zone (kovan-common's
    /// [`DateOptions`]).
    pub dates: DateOptions,
    /// `Zotero.parentTranslator` is set: the translation runs inside another
    /// translator. Some translators change behaviour (BibTeX splits keywords
    /// on spaces, unescapes HTML entities). `None` at top level.
    pub parent_translator: Option<String>,
}

impl Default for TranslationEnv {
    fn default() -> Self {
        TranslationEnv {
            zotero_version: "5.0.97".to_owned(),
            dates: DateOptions {
                // The translation-server is not the Zotero client, so
                // strToDate accepts no day suffixes (date.js:434).
                day_suffixes: Vec::new(),
                ..DateOptions::default()
            },
            parent_translator: None,
        }
    }
}

/// The options a translation runs with.
#[derive(Debug, Clone, PartialEq)]
pub struct TranslateOptions {
    /// Display options (`Zotero.getOption`). Start from
    /// [`TranslatorMetadata::default_display_options`].
    pub display: JsObject,
    /// Hidden preferences (`Zotero.getHiddenPref`), overriding the header's
    /// `hiddenPrefs` defaults (upstream reads `translators.<name>` prefs).
    pub hidden_prefs: JsObject,
    /// The environment.
    pub env: TranslationEnv,
    /// The first key `itemToAPIJSON` assigns (import); keys count up from
    /// here. See [`super::api_json::KeyGenerator`].
    pub first_key_index: u64,
}

impl TranslateOptions {
    /// A translator's default options in the default environment.
    pub fn for_translator(meta: &TranslatorMetadata) -> Self {
        TranslateOptions {
            display: meta.default_display_options(),
            hidden_prefs: JsObject::new(),
            env: TranslationEnv::default(),
            first_key_index: 0,
        }
    }

    /// `Zotero.getOption(name)`.
    pub fn get_option(&self, name: &str) -> Option<&Value> {
        self.display.get(name)
    }

    /// JavaScript truthiness of `Zotero.getOption(name)`.
    pub fn option_truthy(&self, name: &str) -> bool {
        self.display.truthy(name)
    }

    /// `Zotero.getHiddenPref(name)`: the caller's value, else the header
    /// default, else `None` (`undefined`).
    pub fn get_hidden_pref(&self, meta: &TranslatorMetadata, name: &str) -> Option<Value> {
        self.hidden_prefs.get(name).cloned().or_else(|| {
            meta.hidden_prefs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_value())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn semver_compare_orders_numerically() {
        assert_eq!(semver_compare("4.0.27", "2.1.9"), Ordering::Greater);
        assert_eq!(semver_compare("4.0.27", "3.0.4"), Ordering::Greater);
        assert_eq!(semver_compare("4.0.27", "4.0.27"), Ordering::Equal);
        assert_eq!(semver_compare("4.0.27", "4.0.100"), Ordering::Less);
        assert_eq!(semver_compare("5.0.0", "5.0.0.1"), Ordering::Less);
    }
}
