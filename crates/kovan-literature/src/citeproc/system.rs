// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/system.js
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

//! `CSL.setupXml` (src/system.js): turn a style or locale given to the
//! engine into an [`XmlJson`].
//!
//! Only the string input and the JSON-object input of upstream's four
//! branches exist here:
//!
//! * a string that starts (after a leading `\s*`) with `<` is serialized XML
//!   and goes through `CSL.parseXml` ([`super::xmljson::parse_xml`]);
//! * any other string is serialized JSON (`JSON.parse`) of the
//!   `{name, attrs, children}` tree ([`super::xmljson::XmlJson::from_value`]);
//! * an already-parsed JSON object ([`setup_xml_object`]) is used as is.
//!
//! The DOM branch (`getAttribute` present) and the E4X branch (`toXMLString`
//! present) of upstream cannot be reached from Rust, and `xmldom.js` is not
//! ported (see `xmldom.rs`).

use serde_json::Value;

use super::js;
use super::xmljson::{parse_xml, XmlJson};
use super::{CslResult, EngineError};

/// `CSL.setupXml(xmlObject)` for a string `xmlObject`.
///
/// `xmlObject.replace("^﻿", "")` is a *literal* replace of the five-
/// character string `^` + U+FEFF (not a regex), so it only matters for input
/// that really starts that way; it is kept. Leading white space is stripped.
pub fn setup_xml(xml: &str) -> CslResult<XmlJson> {
    let s = xml.replacen("^\u{feff}", "", 1);
    let s = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if js::slice(s, 0, Some(1)) == "<" {
        // Assume serialized XML
        parse_xml(s)
    } else {
        // Assume serialized JSON
        match serde_json::from_str::<Value>(s) {
            Ok(v) => Ok(XmlJson::from_value(&v)),
            Err(e) => Err(EngineError::Csl(format!("SyntaxError: {e}"))),
        }
    }
}

/// `CSL.setupXml(xmlObject)` for a JS object (`new CSL.XmlJSON(xmlObject)`).
pub fn setup_xml_object(obj: &Value) -> XmlJson {
    XmlJson::from_value(obj)
}

/// `CSL.setupXml(undefined)`: `CSL.error("unable to parse XML input")`.
pub fn setup_xml_undefined() -> CslResult<XmlJson> {
    Err(EngineError::Csl("unable to parse XML input".to_string()))
}

/// `CSL.setupXml(false)`: what `localeConfigure` makes of
/// `sys.retrieveLocale(lang)` returning `false` for a locale it does not
/// have. `false` is not a string, has no `getAttribute` or `toXMLString`, so
/// it becomes `new CSL.XmlJSON(false)`: a parser over nothing, in which every
/// search finds no node.
pub fn setup_xml_missing() -> XmlJson {
    XmlJson::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_and_json_strings_both_work() {
        let x = setup_xml("  <style a=\"b\"><info/></style>").unwrap();
        assert_eq!(x.nodename(x.data_obj.unwrap()), "style");
        let j = setup_xml(r#"{"name":"style","attrs":{"a":"b"},"children":[]}"#).unwrap();
        assert_eq!(j.get_attribute_string(j.data_obj.unwrap(), "a"), "b");
        assert!(setup_xml("{nope").is_err());
        assert!(setup_xml_missing().data_obj.is_none());
        assert!(setup_xml_undefined().is_err());
    }
}
