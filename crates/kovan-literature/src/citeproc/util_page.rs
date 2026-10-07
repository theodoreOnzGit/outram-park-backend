// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_page.js
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

//! `CSL.Util.PageRangeMangler`: formats page and year ranges
//! (`expanded`, `minimal`, `minimal-two`, `chicago`, `chicago-15`,
//! `chicago-16`).
//!
//! `CSL.Util.PageRangeMangler.getFunction(state, rangeType)` returns a JS
//! closure over a handful of *shared* local variables (`pos`, `len`, `m`, `b`,
//! `e`, `ret`, `lst`, ...). The port keeps the one that matters: the match
//! array `m`, which `chicago15`/`chicago16` read even for list elements that
//! are not ranges (they see whatever `m` was last, so a non-range element can
//! make them throw upstream; here that is an `Err`).
//!
//! Integration: build.js:258-259 stores `getFunction(this, "page")` and
//! `getFunction(this, "year")` as `state.fun.page_mangler` / `year_mangler`;
//! those fields are [`PageRangeMangler`]s and are called as
//! `state.fun.page_mangler.mangle(str, false)` (JS `page_mangler(str)`) and
//! `state.fun.year_mangler.mangle(str, true)` (JS `year_mangler(str, true)`).

use std::sync::LazyLock;

use regex::Regex;

use super::formats::get_term;
use super::obj_blob::JS_WS_CLASS;
use super::state::State;
use super::{CslResult, EngineError};

/// `state.opt["<type>-range-format"]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RangeFormat {
    /// Option unset (falsy): behaves as `expanded`.
    #[default]
    Unset,
    /// `"expanded"`.
    Expanded,
    /// `"minimal"`.
    Minimal,
    /// `"minimal-two"`.
    MinimalTwo,
    /// `"chicago"` (same function as `chicago-15`).
    Chicago,
    /// `"chicago-15"`.
    Chicago15,
    /// `"chicago-16"`.
    Chicago16,
    /// Any other truthy value: upstream returns `undefined` and the call
    /// would throw.
    Unknown,
}

impl RangeFormat {
    /// Parse the option value (`None` is unset/falsy).
    pub fn from_option(v: Option<&str>) -> RangeFormat {
        match v {
            None | Some("") => RangeFormat::Unset,
            Some("expanded") => RangeFormat::Expanded,
            Some("minimal") => RangeFormat::Minimal,
            Some("minimal-two") => RangeFormat::MinimalTwo,
            Some("chicago") => RangeFormat::Chicago,
            Some("chicago-15") => RangeFormat::Chicago15,
            Some("chicago-16") => RangeFormat::Chicago16,
            Some(_) => RangeFormat::Unknown,
        }
    }
}

/// The function `CSL.Util.PageRangeMangler.getFunction(state, rangeType)`
/// returns (`state.fun.page_mangler` / `year_mangler`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageRangeMangler {
    /// `range_delimiter` = `state.getTerm(rangeType + "-range-delimiter")`.
    pub range_delimiter: String,
    /// `state.opt[rangeType + "-range-format"]`.
    pub format: RangeFormat,
}

/// One element of the list `listify`/`expand` build: a string, or (a range
/// that was expanded) the 4-element array `m.slice(1)`:
/// `[prefix1, begin, delimiter+prefix, end]` (`undefined` is `None`).
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Str(String),
    Range([Option<String>; 4]),
}

type Match5 = Vec<Option<String>>;

fn re(p: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(p).expect("constant regex")
}

static RANGEREX: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "([0-9]*[a-zA-Z]+0*)?([0-9]+[a-z]*)[{ws}]*(?:\\x{{2013}}|-)[{ws}]*([0-9]*[a-zA-Z]+0*)?([0-9]+[a-z]*)",
        ws = JS_WS_CLASS
    ))
});
static REXM: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "((?:[0-9]*[a-zA-Z]+0*)?[0-9]+[a-z]*[{ws}]+-[{ws}]+(?:[0-9]*[a-zA-Z]+0*)?[0-9]+[a-z]*)",
        ws = JS_WS_CLASS
    ))
});
static REXLST: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "(?:[0-9]*[a-zA-Z]+0*)?[0-9]+[a-z]*[{ws}]+-[{ws}]+(?:[0-9]*[a-zA-Z]+0*)?[0-9]+[a-z]*",
        ws = JS_WS_CLASS
    ))
});
static WS_HYPHEN_WS: LazyLock<Regex> =
    LazyLock::new(|| re(&format!("[{ws}]+-[{ws}]+", ws = JS_WS_CLASS)));
static WS_HYPHEN_WS_OPT: LazyLock<Regex> =
    LazyLock::new(|| re(&format!("[{ws}]*-[{ws}]*", ws = JS_WS_CLASS)));
static NOT_BACKSLASH_HYPHEN: LazyLock<Regex> = LazyLock::new(|| re(r"([^\\])-"));

fn opt_str(o: &Option<String>) -> &str {
    o.as_deref().unwrap_or("")
}

/// JS `parseInt(s, 10)` for the digit strings the range regexp produces.
fn parse_int(s: &str) -> Option<i64> {
    super::js::parse_int(s)
}

impl PageRangeMangler {
    /// `CSL.Util.PageRangeMangler.getFunction(state, rangeType)`: reads the
    /// delimiter term and `state.opt[rangeType + "-range-format"]`.
    pub fn get_function(state: &State, range_type: &str) -> PageRangeMangler {
        let range_delimiter = get_term(state, &format!("{range_type}-range-delimiter"));
        let fmt = state
            .opt
            .get(&format!("{range_type}-range-format"))
            .and_then(|v| v.as_str());
        PageRangeMangler {
            range_delimiter,
            format: RangeFormat::from_option(fmt),
        }
    }

    /// `stringify(lst)`.
    fn stringify(&self, lst: &[Item]) -> String {
        let mut joined = String::new();
        for it in lst {
            match it {
                Item::Str(s) => joined.push_str(s),
                Item::Range(a) => {
                    for x in a {
                        joined.push_str(opt_str(x));
                    }
                }
            }
        }
        let d = self.range_delimiter.clone();
        NOT_BACKSLASH_HYPHEN
            .replace_all(&joined, |c: &regex::Captures| format!("{}{}", &c[1], d))
            .into_owned()
    }

    /// `listify(str)`.
    fn listify(&self, s: &str) -> Vec<Item> {
        // Normalized delimiter form, for use in regexps
        let this_range_delimiter = if self.range_delimiter == "-" {
            String::new()
        } else {
            self.range_delimiter.clone()
        };
        let mut class = String::from("-");
        for ch in this_range_delimiter.chars() {
            class.push_str(&regex::escape(&ch.to_string()));
        }
        class.push_str("\u{2013}");
        let delim_rex = re(&format!("([^\\\\])[{class}]"));
        let s = delim_rex.replace_all(s, "${1} - ").into_owned();
        let s = WS_HYPHEN_WS.replace_all(&s, " - ").into_owned();
        let m: Vec<String> = REXM.find_iter(&s).map(|x| x.as_str().to_string()).collect();
        let lst: Vec<String> = REXLST.split(&s).map(str::to_string).collect();
        if lst.is_empty() {
            // ret = m  (JS: m may be null; never reached, split always yields >= 1)
            return m.into_iter().map(Item::Str).collect();
        }
        let mut ret = vec![Item::Str(lst[0].clone())];
        for pos in 1..lst.len() {
            let mm = m.get(pos - 1).cloned().unwrap_or_default();
            ret.push(Item::Str(
                WS_HYPHEN_WS_OPT.replace_all(&mm, "-").into_owned(),
            ));
            ret.push(Item::Str(lst[pos].clone()));
        }
        ret
    }

    /// `expand(str)`: the list, and the last `m` (the match array of the final
    /// odd element, as modified by this function; `None` is JS `null` or, when
    /// there is no odd element, `undefined`).
    fn expand(&self, s: &str) -> (Vec<Item>, Option<Match5>) {
        let mut lst = self.listify(s);
        let mut m_last: Option<Match5> = None;
        let mut pos = 1;
        while pos < lst.len() {
            let text = match &lst[pos] {
                Item::Str(t) => t.clone(),
                Item::Range(_) => String::new(),
            };
            let caps = RANGEREX.captures(&text);
            let mut m: Option<Match5> = caps.map(|c| {
                (0..5)
                    .map(|i| c.get(i).map(|x| x.as_str().to_string()))
                    .collect()
            });
            if let Some(mv) = m.as_mut() {
                let m3_falsy = mv[3].as_deref().map(str::is_empty).unwrap_or(true);
                if m3_falsy || mv[1] == mv[3] {
                    let l4 = super::js::len(opt_str(&mv[4]));
                    let l2 = super::js::len(opt_str(&mv[2]));
                    if l4 < l2 {
                        let head = super::js::slice(opt_str(&mv[2]), 0, Some((l2 - l4) as i64));
                        mv[4] = Some(format!("{}{}", head, opt_str(&mv[4])));
                    }
                    let b = parse_int(opt_str(&mv[2]));
                    let e = parse_int(opt_str(&mv[4]));
                    if let (Some(b), Some(e)) = (b, e) {
                        if b < e {
                            let pre = if mv[1].as_deref().map(|x| !x.is_empty()).unwrap_or(false) {
                                opt_str(&mv[1]).to_string()
                            } else {
                                String::new()
                            };
                            mv[3] = Some(format!("{}{}", self.range_delimiter, pre));
                            lst[pos] = Item::Range([
                                mv[1].clone(),
                                mv[2].clone(),
                                mv[3].clone(),
                                mv[4].clone(),
                            ]);
                        }
                    }
                }
            }
            m_last = m;
            if let Item::Str(t) = &lst[pos] {
                // lst[pos].replace(/\-/g, range_delimiter)
                let r = t.replace('-', &self.range_delimiter);
                lst[pos] = Item::Str(r);
            }
            pos += 2;
        }
        (lst, m_last)
    }

    /// `minimize_internal(begin, end, minchars, isyear)`.
    fn minimize_internal(begin: &str, end: &str, minchars: usize, isyear: bool) -> String {
        let b: Vec<char> = begin.chars().collect();
        let e: Vec<char> = end.chars().collect();
        let mut ret: Vec<char> = e.iter().rev().copied().collect();
        if b.len() == e.len() {
            for i in 0..b.len() {
                if b[i] == e[i] && ret.len() > minchars {
                    ret.pop();
                } else {
                    if minchars > 0 && isyear && ret.len() == 3 {
                        let front: Vec<char> = b[..i].iter().rev().copied().collect();
                        ret.extend(front);
                    }
                    break;
                }
            }
        }
        ret.reverse();
        ret.into_iter().collect()
    }

    /// `minimize(lst, minchars, isyear)`.
    fn minimize(&self, mut lst: Vec<Item>, minchars: usize, isyear: bool) -> String {
        let mut i = 1;
        while i < lst.len() {
            if let Item::Range(a) = &mut lst[i] {
                a[3] = Some(Self::minimize_internal(
                    opt_str(&a[1]),
                    opt_str(&a[3]),
                    minchars,
                    isyear,
                ));
                if Some(super::obj_blob::drop_first(opt_str(&a[2]))) == a[0].as_deref() {
                    a[2] = Some(self.range_delimiter.clone());
                }
            }
            i += 2;
        }
        self.stringify(&lst)
    }

    /// The `if (m[2].slice(1) === m[0]) m[2] = range_delimiter` tail of
    /// `chicago15`/`chicago16`, applied to whichever array `m` currently is.
    fn chicago_tail(
        &self,
        lst: &mut [Item],
        cur: &mut Cur,
        m_expand: &mut Option<Match5>,
    ) -> CslResult<()> {
        match cur {
            Cur::Item(k) => {
                if let Item::Range(a) = &mut lst[*k] {
                    if Some(super::obj_blob::drop_first(opt_str(&a[2]))) == a[0].as_deref() {
                        a[2] = Some(self.range_delimiter.clone());
                    }
                }
                Ok(())
            }
            Cur::Expand => match m_expand.as_mut() {
                None => Err(EngineError::Csl(
                    "page range: m is null (TypeError in citeproc-js)".into(),
                )),
                Some(mv) => {
                    if Some(super::obj_blob::drop_first(opt_str(&mv[2]))) == mv[0].as_deref() {
                        mv[2] = Some(self.range_delimiter.clone());
                    }
                    Ok(())
                }
            },
        }
    }

    /// `chicago15(lst)` (also `"chicago"`).
    fn chicago15(&self, mut lst: Vec<Item>, m_expand: Option<Match5>) -> CslResult<String> {
        let mut m_expand = m_expand;
        let mut cur = Cur::Expand;
        let mut pos = 1;
        while pos < lst.len() {
            if let Item::Range(a) = &mut lst[pos] {
                let begin = parse_int(opt_str(&a[1]));
                let end = parse_int(opt_str(&a[3]));
                if let (Some(begin), Some(end)) = (begin, end) {
                    if begin > 100 && begin % 100 != 0 && begin / 100 == end / 100 {
                        a[3] = Some((end % 100).to_string());
                    } else if begin >= 10000 {
                        a[3] = Some((end % 1000).to_string());
                    }
                }
                cur = Cur::Item(pos);
            }
            self.chicago_tail(&mut lst, &mut cur, &mut m_expand)?;
            pos += 2;
        }
        Ok(self.stringify(&lst))
    }

    /// `chicago16(lst)`.
    fn chicago16(&self, mut lst: Vec<Item>, m_expand: Option<Match5>) -> CslResult<String> {
        let mut m_expand = m_expand;
        let mut cur = Cur::Expand;
        let mut pos = 1;
        while pos < lst.len() {
            if let Item::Range(a) = &mut lst[pos] {
                let begin = parse_int(opt_str(&a[1]));
                let end = parse_int(opt_str(&a[3]));
                let e_len = match end {
                    Some(v) => v.to_string().len(),
                    None => 3, // "NaN"
                };
                if let Some(begin) = begin {
                    if begin > 100 && begin % 100 != 0 {
                        for i in 2..e_len {
                            let divisor = 10i128.pow(i as u32);
                            if let Some(end) = end {
                                if (begin as i128).div_euclid(divisor)
                                    == (end as i128).div_euclid(divisor)
                                {
                                    a[3] = Some(((end as i128) % divisor).to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                cur = Cur::Item(pos);
            }
            self.chicago_tail(&mut lst, &mut cur, &mut m_expand)?;
            pos += 2;
        }
        Ok(self.stringify(&lst))
    }

    /// The closure `getFunction` returns: `ret_func(str, isyear)`.
    ///
    /// `is_year` is the second argument (only `minimal-two` reads it).
    pub fn mangle(&self, s: &str, is_year: bool) -> CslResult<String> {
        // sniff(str, func, minchars, isyear): str = "" + str; lst = expand(str)
        let (lst, m) = self.expand(s);
        match self.format {
            RangeFormat::Unset | RangeFormat::Expanded => Ok(self.stringify(&lst)),
            RangeFormat::Minimal => Ok(self.minimize(lst, 0, false)),
            RangeFormat::MinimalTwo => Ok(self.minimize(lst, 2, is_year)),
            RangeFormat::Chicago | RangeFormat::Chicago15 => self.chicago15(lst, m),
            RangeFormat::Chicago16 => self.chicago16(lst, m),
            RangeFormat::Unknown => Err(EngineError::Csl(
                "range format function is undefined (unknown range-format value)".into(),
            )),
        }
    }
}

/// Which array the closure variable `m` currently aliases in
/// `chicago15`/`chicago16`.
enum Cur {
    /// The (modified) full-match array left over from `expand`.
    Expand,
    /// `lst[k]`, the array assigned by `m = lst[pos]`.
    Item(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(fmt: &str) -> PageRangeMangler {
        PageRangeMangler {
            range_delimiter: "\u{2013}".into(),
            format: RangeFormat::from_option(if fmt.is_empty() { None } else { Some(fmt) }),
        }
    }

    #[test]
    fn expanded_and_minimal() {
        assert_eq!(
            mk("expanded").mangle("42-5", false).unwrap(),
            "42\u{2013}45"
        );
        assert_eq!(
            mk("minimal").mangle("321-328", false).unwrap(),
            "321\u{2013}8"
        );
        assert_eq!(
            mk("minimal-two").mangle("321-328", false).unwrap(),
            "321\u{2013}28"
        );
        assert_eq!(
            mk("chicago").mangle("71-72", false).unwrap(),
            "71\u{2013}72"
        );
        assert_eq!(
            mk("chicago").mangle("321-328", false).unwrap(),
            "321\u{2013}28"
        );
    }
}
