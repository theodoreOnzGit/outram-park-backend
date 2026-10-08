// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_citationlabel.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.Engine.prototype.getCitationLabel` and `.getTrigraphParams`: the
//! `citation-label` variable ("Smi00", "ABC99") built from the first
//! author's family name, or the title, and the year.
//!
//! `this.nameOutput.getName(name, "locale-translit", true)` (the names port)
//! is stood in for by the name itself (`PORT-LATER(w2-names)`): it only
//! matters for names with transliterated variants.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js;
use super::load::{NAME_VARIABLES, ROMANESQUE_NOT_REGEXP};
use super::state::State;
use super::{CslResult, EngineError};

/// One trigraph configuration: `{authors: [letters per author], year: digits}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrigraphConfig {
    /// `authors`: how many letters of each author's name to use.
    pub authors: Vec<usize>,
    /// `year`: how many digits of the year to use.
    pub year: usize,
}

/// `/^([ \'’a-z]+\s+)/`.
static LEADING_PARTICLE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(&format!("^[ '\u{2019}a-z]+[{}]+", js::WS)).expect("static regex")
});
/// `/^(a\s+|the\s+|an\s+)/`.
static LEADING_ARTICLE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(&format!("^(?:a[{ws}]+|the[{ws}]+|an[{ws}]+)", ws = js::WS)).expect("static regex")
});
static WS_RUN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(&format!("[{}]+", js::WS)).expect("static regex")
});

fn upper_first(s: &str) -> String {
    format!("{}{}", js::slice(s, 0, Some(1)).to_uppercase(), js::slice(s, 1, None))
}

/// `myname.slice(0,1).toUpperCase() + myname.slice(1).toLowerCase()` when the
/// name has more than one character, `toUpperCase()` when exactly one.
fn capitalise(s: &str) -> String {
    let n = js::len(s);
    if n > 1 {
        format!(
            "{}{}",
            js::slice(s, 0, Some(1)).to_uppercase(),
            js::slice(s, 1, None).to_lowercase()
        )
    } else if n == 1 {
        s.to_uppercase()
    } else {
        s.to_string()
    }
}

impl State {
    /// `getTrigraphParams()`: parse `opt.trigraph` ("Aaaa00:AaAa00:...").
    pub fn get_trigraph_params(&self) -> CslResult<Vec<TrigraphConfig>> {
        let trigraph = js::get_str(&self.opt, "trigraph").unwrap_or("");
        let mut params = Vec::new();
        if trigraph.is_empty() || js::slice(trigraph, 0, Some(1)) != "A" {
            return Err(EngineError::Csl(format!(
                "Bad trigraph definition: {trigraph}"
            )));
        }
        for part in trigraph.split(':') {
            let mut config = TrigraphConfig::default();
            for c in part.chars() {
                match c {
                    'A' => config.authors.push(1),
                    'a' => match config.authors.last_mut() {
                        Some(n) => *n += 1,
                        None => {
                            return Err(EngineError::Csl(
                                "TypeError: Cannot read properties of NaN in trigraph".to_string(),
                            ))
                        }
                    },
                    '0' => config.year += 1,
                    _ => {
                        return Err(EngineError::Csl(format!(
                            "Invalid character in trigraph definition: {trigraph}"
                        )))
                    }
                }
            }
            params.push(config);
        }
        Ok(params)
    }

    /// `getCitationLabel(Item)`.
    pub fn get_citation_label(&mut self, item: &Value) -> CslResult<String> {
        let mut label = String::new();
        let params = self.get_trigraph_params()?;
        let mut config: Option<TrigraphConfig> = params.first().cloned();
        let mut myname = self
            .get_term("reference", Some("short"), Some(0), None, None, false)?
            .unwrap_or_else(|| "reference".to_string());
        myname = myname.replacen('.', "", 1);
        myname = upper_first(&myname);
        for n in NAME_VARIABLES {
            let Some(names) = item.get(*n).filter(|v| js::truthy(v)) else {
                continue;
            };
            let names: Vec<Value> = match names {
                Value::Array(a) => a.clone(),
                _ => Vec::new(),
            };
            config = if names.len() > params.len() {
                params.last().cloned()
            } else {
                names.len().checked_sub(1).and_then(|i| params.get(i).cloned())
            };
            let cfg = config.clone().ok_or_else(|| {
                EngineError::Csl("TypeError: Cannot read properties of undefined (reading 'authors')".into())
            })?;
            for (j, name_in) in names.iter().enumerate() {
                if j == cfg.authors.len() {
                    break;
                }
                // `this.nameOutput.getName(names[j], "locale-translit", true).name`
                // PORT-LATER(w2-names)
                let name = name_in;
                if let Some(family) = name.get("family").filter(|v| js::truthy(v)) {
                    myname = js::to_js_string(family);
                    myname = LEADING_PARTICLE.replace(&myname, "").into_owned();
                } else if let Some(literal) = name.get("literal").filter(|v| js::truthy(v)) {
                    myname = js::to_js_string(literal);
                }
                if let Some(m) = LEADING_ARTICLE.find(&myname.to_lowercase()) {
                    myname = js::slice(&myname, js::len(m.as_str()) as i64, None);
                }
                myname = ROMANESQUE_NOT_REGEXP.replace_all(&myname, "").into_owned();
                if myname.is_empty() {
                    break;
                }
                myname = js::slice(&myname, 0, Some(cfg.authors[j] as i64));
                myname = capitalise(&myname);
                label.push_str(&myname);
            }
            break;
        }
        if label.is_empty() {
            // Try for something using title
            if let Some(title) = item.get("title").filter(|v| js::truthy(v)) {
                let lang = js::get_string(&self.opt, "lang").unwrap_or_default();
                let skip_words: Vec<String> = self
                    .locale
                    .get(&lang)
                    .and_then(|l| l.opts.get("skip-words"))
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(js::to_js_string).collect())
                    .ok_or_else(|| {
                        EngineError::Csl(
                            "TypeError: Cannot read properties of undefined (reading 'skip-words')"
                                .to_string(),
                        )
                    })?;
                let mut lst = js::split(&WS_RUN, &js::to_js_string(title));
                let mut i = lst.len();
                while i > 0 {
                    i -= 1;
                    if skip_words.contains(&lst[i]) {
                        lst.remove(i);
                    }
                }
                let joined = lst.concat();
                let first = params.first().ok_or_else(|| {
                    EngineError::Csl("TypeError: Cannot read properties of undefined (reading 'authors')".into())
                })?;
                let n = first.authors.first().copied().unwrap_or(0);
                let s = js::slice(&joined, 0, Some(n as i64));
                label = capitalise(&s);
            }
        }
        let mut year = "0000".to_string();
        if let Some(y) = item
            .get("issued")
            .and_then(|i| i.get("year"))
            .filter(|v| js::truthy(v))
        {
            year = js::to_js_string(y);
        }
        let digits = config.as_ref().map(|c| c.year).unwrap_or(0);
        // `year.slice(config.year * -1)`
        year = js::slice(&year, -(digits as i64), None);
        label.push_str(&year);
        Ok(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigraph_params_parse_the_default_definition() {
        let mut s = State::default();
        s.opt
            .insert("trigraph".into(), Value::String("Aaaa00:AaAa00:AaAA00:AAAA00".into()));
        let p = s.get_trigraph_params().unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], TrigraphConfig { authors: vec![4], year: 2 });
        assert_eq!(p[1], TrigraphConfig { authors: vec![2, 2], year: 2 });
        assert_eq!(p[2].authors, vec![2, 1, 1]);
        assert_eq!(p[3].authors, vec![1, 1, 1, 1]);
        s.opt.insert("trigraph".into(), Value::String("xA".into()));
        assert!(s.get_trigraph_params().is_err());
        s.opt.insert("trigraph".into(), Value::String("A1".into()));
        assert!(s.get_trigraph_params().is_err());
    }

    #[test]
    fn capitalise_follows_the_slice_rules() {
        assert_eq!(capitalise("sMITH"), "Smith");
        assert_eq!(capitalise("x"), "X");
        assert_eq!(capitalise(""), "");
    }
}
