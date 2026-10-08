// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_locale_sniff.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.getLocaleNames` (src/util_locale_sniff.js): which locales a style
//! uses, for a host that wants to preload them.
//!
//! **Upstream bug kept.** `sniffLocaleOnOneNodeName` calls
//! `this.extendLocaleList(...)`, but `extendLocaleList` is a plain nested
//! function and `this` is `undefined` in the strict-mode bundle, so as soon
//! as a `layout`, `if`, `else-if` or `condition` node carries a `locale`
//! attribute upstream throws a TypeError. This port returns the same error.

use std::sync::LazyLock;

use regex::Regex;

use super::js;
use super::system;
use super::util_locale::locale_resolve;
use super::{CslResult, EngineError};

/// `extendLocaleList(localeList, locale)`: append the `base` and `best` of
/// `locale` if not already present.
fn extend_locale_list(locale_list: &mut Vec<String>, locale: &str) {
    if locale.is_empty() {
        return;
    }
    let normalized = locale_resolve(locale, None);
    for form in [&normalized.base, &normalized.best] {
        if !form.is_empty() && !locale_list.contains(form) {
            locale_list.push(form.clone());
        }
    }
}

/// `CSL.getLocaleNames(myxml, preferredLocale)`: `en-US`, the preferred
/// locale, the style's `default-locale`, then (see the module docs) the
/// locales named by `layout`/`if`/`else-if`/`condition` nodes.
pub fn get_locale_names(myxml: &str, preferred_locale: Option<&str>) -> CslResult<Vec<String>> {
    let stylexml = system::setup_xml(myxml)?;
    let mut locale_ids = vec!["en-US".to_string()];
    extend_locale_list(&mut locale_ids, preferred_locale.unwrap_or(""));
    let style_node = stylexml
        .get_nodes_by_name(stylexml.data_obj, "style", "")
        .first()
        .copied();
    let default_locale = match style_node {
        Some(n) => stylexml.get_attribute_string(n, "default-locale"),
        None => String::new(),
    };
    extend_locale_list(&mut locale_ids, &default_locale);
    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(" +").expect("static"));
    for node_name in ["layout", "if", "else-if", "condition"] {
        for n in stylexml.get_nodes_by_name(stylexml.data_obj, node_name, "") {
            let node_locales = stylexml.get_attribute_string(n, "locale");
            if !node_locales.is_empty() && !js::split(&SPACES, &node_locales).is_empty() {
                return Err(EngineError::Csl(
                    "TypeError: Cannot read properties of undefined (reading 'extendLocaleList')"
                        .to_string(),
                ));
            }
        }
    }
    Ok(locale_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_preferred_and_default_locales() {
        let style = r#"<style class="note" version="1.0" default-locale="fr-CA"><citation><layout/></citation></style>"#;
        assert_eq!(
            get_locale_names(style, Some("de")).unwrap(),
            vec!["en-US", "de-DE", "fr-FR", "fr-CA"]
        );
    }

    #[test]
    fn a_locale_attribute_reaches_the_upstream_bug() {
        let style = r#"<style class="note" version="1.0"><citation><layout locale="fr"/></citation></style>"#;
        assert!(get_locale_names(style, None).is_err());
    }
}
