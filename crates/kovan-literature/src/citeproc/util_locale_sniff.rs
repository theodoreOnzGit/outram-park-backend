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
//! **What upstream actually returns.** `sniffLocaleOnOneNodeName(nodeName)` is
//! declared with one parameter but called as
//! `sniffLocaleOnOneNodeName(stylexml, localeIDs, nodeNames[i])`, so its
//! `nodeName` is the `stylexml` object, `getNodesByName` matches no node, and
//! the loop body (with its `this.extendLocaleList`, which would throw) is never
//! reached. citeproc-js 2.4.63 therefore returns `en-US`, the preferred locale
//! and the style's `default-locale` only, whatever `locale` attributes the
//! `layout`/`if`/`else-if`/`condition` nodes carry (checked in node 22 with the
//! npm bundle, 2026-10-08, #802). The port does the same.
//!
//! ~~Earlier versions of this file returned a TypeError as soon as such a node
//! carried a `locale` attribute, describing it as an upstream bug kept.~~
//! **CORRECTED 2026-10-08** (#802): that was a divergence from citeproc-js,
//! which never throws here. Nothing in the engine calls this function; the
//! `locale` attributes themselves (a CSL-M extension, not CSL 1.0.2) are
//! handled by `@locale` / `@locale-internal` in attributes.rs and node_layout.rs.

use super::system;
use super::util_locale::locale_resolve;
use super::CslResult;

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
    // The node sniffing loop (`sniffLocaleOnOneNodeName` over layout, if,
    // else-if, condition) matches no node upstream (see the module docs), so
    // it contributes nothing and is not ported.
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
    fn locale_attributes_on_nodes_add_nothing_as_upstream() {
        // citeproc-js 2.4.63 in node 22: getLocaleNames on a style whose
        // layout and if carry locale="fr-FR", preferred locale en-GB,
        // default-locale de-DE, returns ["en-US","en-GB","de-DE"], no throw.
        let style = r#"<style class="note" version="1.0" default-locale="de-DE"><citation><layout locale="fr-FR"><choose><if locale="fr-FR"><text value="x"/></if></choose></layout></citation></style>"#;
        assert_eq!(
            get_locale_names(style, Some("en-GB")).unwrap(),
            vec!["en-US", "en-GB", "de-DE"]
        );
    }
}
