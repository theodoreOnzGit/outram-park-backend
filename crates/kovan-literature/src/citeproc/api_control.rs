// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/api_control.js
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

//! The setters of the engine's public configuration API (src/api_control.js):
//! `setOutputFormat`, `setLangTagsForCslSort`,
//! `setLangTagsForCslTransliteration`, `setLangTagsForCslTranslation`,
//! `setLangPrefsForCites`, `setLangPrefsForCiteAffixes`,
//! `setAutoVietnameseNamesOption`, `setAbbreviations`,
//! `setSuppressTrailingPunctuation`, and the `getSortFunc` comparator.
//!
//! The `Engine` struct's public wrappers (`Engine::set_output_format`, ...)
//! call these on its [`State`].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::Value;

use super::js;
use super::state::State;
use super::util_processor;
use super::CslResult;

/// `getSortFunc()`'s comparator: more hyphen-separated segments first, then
/// a longer last segment first (so `en-US-x-sort` precedes `en-US` precedes
/// `en`).
pub fn lang_tag_compare(a: &str, b: &str) -> Ordering {
    let a: Vec<&str> = a.split('-').collect();
    let b: Vec<&str> = b.split('-').collect();
    if a.len() < b.len() {
        Ordering::Greater
    } else if a.len() > b.len() {
        Ordering::Less
    } else {
        let a = js::len(a[a.len() - 1]);
        let b = js::len(b[b.len() - 1]);
        if a < b {
            Ordering::Greater
        } else if a > b {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

fn string_array(opt: &mut super::js::Obj, key: &str) -> Vec<String> {
    match opt.get(key) {
        Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
        _ => Vec::new(),
    }
}

fn set_string_array(opt: &mut super::js::Obj, key: &str, v: Vec<String>) {
    opt.insert(
        key.to_string(),
        Value::Array(v.into_iter().map(Value::String).collect()),
    );
}

impl State {
    /// `setOutputFormat(mode)`: `opt.mode = mode` and `fun.decorate =
    /// CSL.Mode(mode)`.
    ///
    /// PORT-LATER(queue): upstream also creates `this.output[mode] = {tmp:
    /// {}}` on the output queue if it has none (wave1-output owns `Queue`).
    pub fn set_output_format(&mut self, mode: &str) -> CslResult<()> {
        self.opt
            .insert("mode".into(), Value::String(mode.to_string()));
        self.fun.decorate = util_processor::mode(mode)?;
        Ok(())
    }

    /// `setLangTagsForCslSort(tags)`: with tags, replace `opt['locale-sort']`;
    /// then sort it (longest tags first, see [`lang_tag_compare`]).
    pub fn set_lang_tags_for_csl_sort(&mut self, tags: Option<&[String]>) {
        if let Some(tags) = tags {
            set_string_array(&mut self.opt, "locale-sort", tags.to_vec());
        }
        let mut v = string_array(&mut self.opt, "locale-sort");
        v.sort_by(|a, b| lang_tag_compare(a, b));
        set_string_array(&mut self.opt, "locale-sort", v);
    }

    /// `setLangTagsForCslTransliteration(tags)`.
    pub fn set_lang_tags_for_csl_transliteration(&mut self, tags: Option<&[String]>) {
        let mut v: Vec<String> = tags.map(<[String]>::to_vec).unwrap_or_default();
        v.sort_by(|a, b| lang_tag_compare(a, b));
        set_string_array(&mut self.opt, "locale-translit", v);
    }

    /// `setLangTagsForCslTranslation(tags)`.
    pub fn set_lang_tags_for_csl_translation(&mut self, tags: Option<&[String]>) {
        let mut v: Vec<String> = tags.map(<[String]>::to_vec).unwrap_or_default();
        v.sort_by(|a, b| lang_tag_compare(a, b));
        set_string_array(&mut self.opt, "locale-translat", v);
    }

    /// `setLangPrefsForCites(obj, conv)`: for each of persons, institutions,
    /// titles, journals, publishers and places that `obj` gives (under the
    /// name `conv` maps it to; default lower case), normalise the order of
    /// its second and third entries (`translit` before `translat`) and
    /// copy it into `opt['cite-lang-prefs']`.
    pub fn set_lang_prefs_for_cites(
        &mut self,
        obj: &BTreeMap<String, Vec<String>>,
        conv: Option<fn(&str) -> String>,
    ) {
        let segments = [
            "Persons",
            "Institutions",
            "Titles",
            "Journals",
            "Publishers",
            "Places",
        ];
        for seg in segments {
            let client_segment = match conv {
                Some(f) => f(seg),
                None => seg.to_lowercase(),
            };
            let citeproc_segment = seg.to_lowercase();
            let Some(given) = obj.get(&client_segment) else {
                continue;
            };
            //
            // Normalize the sequence of secondary and tertiary
            // in the provided obj segment list.
            //
            let mut given = given.clone();
            let mut supplements: Vec<String> = Vec::new();
            while given.len() > 1 {
                if let Some(x) = given.pop() {
                    supplements.push(x);
                }
            }
            let sortval = |s: &str| match s {
                "orig" => Some(1),
                "translit" => Some(2),
                "translat" => Some(3),
                _ => None,
            };
            if supplements.len() == 2 {
                if let (Some(a), Some(b)) = (sortval(&supplements[0]), sortval(&supplements[1])) {
                    if a < b {
                        supplements.reverse();
                    }
                }
            }
            while let Some(s) = supplements.pop() {
                given.push(s);
            }
            //
            // normalization done.
            //
            if let Some(Value::Object(prefs)) = self.opt.get_mut("cite-lang-prefs") {
                prefs.insert(
                    citeproc_segment,
                    Value::Array(given.into_iter().map(Value::String).collect()),
                );
            }
        }
    }

    /// `setLangPrefsForCiteAffixes(affixList)`: a list of exactly 48 entries
    /// (prefix and suffix for each of six segments and four forms) into
    /// `opt.citeAffixes`. Any other length is ignored.
    ///
    /// Upstream quirks kept: the four forms are `translit, orig, translit,
    /// translat` (translit twice); the suffix is taken from `affixList[count +
    /// 1]` only if `affixList[count]` is truthy; the second `translit` slot
    /// of each segment (index 4 of 8) is written only if still empty.
    pub fn set_lang_prefs_for_cite_affixes(&mut self, affix_list: &[Value]) {
        if affix_list.len() != 48 {
            return;
        }
        let settings = [
            "persons",
            "institutions",
            "titles",
            "journals",
            "publishers",
            "places",
        ];
        let forms = ["translit", "orig", "translit", "translat"];
        let mut count = 0usize;
        let Some(Value::Object(affixes)) = self.opt.get_mut("citeAffixes") else {
            return;
        };
        let get = |i: usize| affix_list.get(i).cloned().unwrap_or(Value::Null);
        for setting in settings {
            for form in forms {
                let key = format!("locale-{form}");
                let slot = affixes
                    .get_mut(setting)
                    .and_then(|s| s.get_mut(&key))
                    .and_then(Value::as_object_mut);
                if let Some(slot) = slot {
                    let blank = |v: Value| {
                        if js::truthy(&v) {
                            v
                        } else {
                            Value::String(String::new())
                        }
                    };
                    if count % 8 == 4 {
                        if !js::truthy_opt(slot.get("prefix"))
                            && !js::truthy_opt(slot.get("suffix"))
                        {
                            slot.insert("prefix".into(), blank(get(count)));
                            let sfx = if js::truthy(&get(count)) {
                                get(count + 1)
                            } else {
                                Value::String(String::new())
                            };
                            slot.insert("suffix".into(), sfx);
                        }
                    } else {
                        slot.insert("prefix".into(), blank(get(count)));
                        let sfx = if js::truthy(&get(count)) {
                            get(count + 1)
                        } else {
                            Value::String(String::new())
                        };
                        slot.insert("suffix".into(), sfx);
                    }
                }
                count += 2;
            }
        }
    }

    /// `setAutoVietnameseNamesOption(arg)`.
    pub fn set_auto_vietnamese_names_option(&mut self, arg: bool) {
        self.opt
            .insert("auto-vietnamese-names".into(), Value::Bool(arg));
    }

    /// `setAbbreviations(arg)`: calls `sys.setAbbreviations` if the sys has
    /// one; this port's `Sys` has none, so it does nothing.
    pub fn set_abbreviations(&mut self, _arg: &Value) {}

    /// `setSuppressTrailingPunctuation(arg)`: `citation.opt.suppressTrailingPunctuation = !!arg`.
    pub fn set_suppress_trailing_punctuation(&mut self, arg: bool) {
        self.citation
            .opt
            .insert("suppressTrailingPunctuation".into(), Value::Bool(arg));
    }

    /// `setParseNames(val)` (defined inside the `CSL.Engine` constructor,
    /// build.js): `opt['parse-names'] = val`.
    pub fn set_parse_names(&mut self, val: bool) {
        self.opt.insert("parse-names".into(), Value::Bool(val));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_tags_sort_longest_first() {
        let mut st = State::default();
        st.set_lang_tags_for_csl_translation(Some(&[
            "de".to_string(),
            "en-US".to_string(),
            "fr".to_string(),
        ]));
        let got = string_array(&mut st.opt, "locale-translat");
        assert_eq!(got, vec!["en-US", "de", "fr"]);
    }

    #[test]
    fn cite_lang_prefs_normalise_the_supplement_order() {
        let mut st = State::default();
        st.opt = super::super::state::new_opt();
        let mut obj = BTreeMap::new();
        obj.insert(
            "titles".to_string(),
            vec![
                "orig".to_string(),
                "translat".to_string(),
                "translit".to_string(),
            ],
        );
        st.set_lang_prefs_for_cites(&obj, None);
        assert_eq!(
            st.opt["cite-lang-prefs"]["titles"],
            serde_json::json!(["orig", "translit", "translat"])
        );
    }
}
