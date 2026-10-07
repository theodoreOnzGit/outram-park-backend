// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/formatters.js, src/util_processor.js (CSL.Doppeler only,
//              which formatters.js instantiates), src/load.js
//              (CSL.toLocaleUpperCase, CSL.toLocaleLowerCase, CSL.SKIP_WORDS)
//              and src/build.js (the skip-words regexp, `makeRegExp`)
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

//! `CSL.Output.Formatters`: the text-case transforms (`lowercase`,
//! `uppercase`, `capitalize-first`, `capitalize-all`, `sentence`, `title`) and
//! `passthrough`, plus the `CSL.Doppeler` splitter they are built on.
//!
//! Entry point: [`apply`] (`CSL.Output.Formatters[name](state, string)`), or
//! the named functions ([`lowercase`], [`title`], ...).
//!
//! # Integration points (for the integrator)
//!
//! * **Skip words.** `title` and the last-word rule read
//!   `state.locale[state.opt.lang].opts["skip-words-regexp"]`, built in
//!   build.js by `makeRegExp(skip-words)`. That is [`make_skip_words_regex`]
//!   here; the build port stores the result in `state.fun.skip_words_rex`
//!   (field added by this file's owner). When it is `None`, the default
//!   English list `CSL.SKIP_WORDS` is used.
//! * **Locale-aware case.** `CSL.toLocaleUpperCase` reads
//!   `state.tmp.lang_array`; see [`to_locale_upper_case`] for what is and is
//!   not reproduced (Turkic dotted/dotless i yes, Lithuanian no).
//! * **`CSL.Doppeler`** is defined in util_processor.js; it is ported here
//!   because formatters.js instantiates it three times at load time
//!   (`// DUP-CHECK: util_processor.js CSL.Doppeler`).

use std::sync::LazyLock;

use regex::Regex;

use super::obj_blob::{js_trim, JS_WS_CLASS};
use super::state::State;
use super::{CslResult, EngineError};

// DUP-CHECK: load.js CSL.SKIP_WORDS
/// `CSL.SKIP_WORDS`: the default English stop-word list (regexp sources).
pub const SKIP_WORDS: &[&str] = &[
    "about",
    "above",
    "across",
    "afore",
    "after",
    "against",
    "al",
    "along",
    "alongside",
    "amid",
    "amidst",
    "among",
    "amongst",
    "anenst",
    "apropos",
    "apud",
    "around",
    "as",
    "aside",
    "astride",
    "at",
    "athwart",
    "atop",
    "barring",
    "before",
    "behind",
    "below",
    "beneath",
    "beside",
    "besides",
    "between",
    "beyond",
    "but",
    "by",
    "circa",
    "despite",
    "down",
    "during",
    "et",
    "except",
    "for",
    "forenenst",
    "from",
    "given",
    "in",
    "inside",
    "into",
    "lest",
    "like",
    "modulo",
    "near",
    "next",
    "notwithstanding",
    "of",
    "off",
    "on",
    "onto",
    "out",
    "over",
    "per",
    "plus",
    "pro",
    "qua",
    "sans",
    "since",
    "than",
    "through",
    " thru",
    "throughout",
    "thruout",
    "till",
    "to",
    "toward",
    "towards",
    "under",
    "underneath",
    "until",
    "unto",
    "up",
    "upon",
    "versus",
    "vs.",
    "v.",
    "vs",
    "v",
    "via",
    "vis-à-vis",
    "with",
    "within",
    "without",
    "according to",
    "ahead of",
    "apart from",
    "as for",
    "as of",
    "as per",
    "as regards",
    "aside from",
    "back to",
    "because of",
    "close to",
    "due to",
    "except for",
    "far from",
    "inside of",
    "instead of",
    "near to",
    "next to",
    "on to",
    "out from",
    "out of",
    "outside of",
    "prior to",
    "pursuant to",
    "rather than",
    "regardless of",
    "such as",
    "that of",
    "up to",
    "where as",
    "or",
    "yet",
    "so",
    "for",
    "and",
    "nor",
    "a",
    "an",
    "the",
    "de",
    "d'",
    "von",
    "van",
    "c",
    "ca",
];

/// The regexp build.js's `makeRegExp(lst)` builds from the `skip-words` list:
/// `(?:(?:[?!:]*\s+|-|^)(?:w1|w2|...)(?=[!?:]*\s+|-|$))` (flag `g`).
///
/// citeproc-js only ever uses it as `string.match(rex)` truthiness, so the
/// lookahead is rewritten as a consumed tail `(?:[!?:]*\s|-|$)`, which has
/// the same match/no-match answer. `lst` entries are regexp sources, exactly
/// as in the locale (`"vs."`, `"d'"`, `" thru"`).
pub fn make_skip_words_regex(lst: &[String]) -> CslResult<Regex> {
    let pat = format!(
        "(?:(?:[?!:]*[{ws}]+|-|^)(?:{words})(?:[!?:]*[{ws}]|-|$))",
        ws = JS_WS_CLASS,
        words = lst.join("|")
    );
    Regex::new(&pat).map_err(|e| EngineError::Csl(format!("bad skip-words regexp: {e}")))
}

static DEFAULT_SKIP_WORDS_REX: LazyLock<Regex> = LazyLock::new(|| {
    let lst: Vec<String> = SKIP_WORDS.iter().map(|s| s.to_string()).collect();
    // The default list is constant and valid.
    make_skip_words_regex(&lst).unwrap_or_else(|_| unreachable_regex())
});

/// A regex that never matches, used only as the `unwrap_or_else` of the
/// constant patterns in this file (which are verified valid by the tests).
fn unreachable_regex() -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(r"[^\s\S]").expect("constant regex")
}

/// `state.locale[state.opt.lang].opts["skip-words-regexp"]` (see the module
/// docs).
fn skip_words_rex(state: &State) -> Regex {
    match &state.fun.skip_words_rex {
        Some(r) => r.clone(),
        None => DEFAULT_SKIP_WORDS_REX.clone(),
    }
}

// ---------------------------------------------------------------------------
// CSL.Doppeler
// ---------------------------------------------------------------------------

/// The string mangler a [`Doppeler`] runs before splitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mangler {
    None,
    /// The `tagDoppel` mangler in formatters.js (normalises span markup).
    TagDoppel,
}

/// The result of `Doppeler.split`: `strings` has one more element than
/// `tags`. `orig_strings` is a copy of `strings` taken after the apostrophe
/// fix (JS: absent when there is no tag at all).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Doppel {
    /// The separators (`tags`).
    pub tags: Vec<String>,
    /// The pieces between them.
    pub strings: Vec<String>,
    /// `origStrings`.
    pub orig_strings: Vec<String>,
}

/// `CSL.Doppeler(rexStr, stringMangler)`: splits a string around the matches
/// of a regexp, keeping the matches (`tags`) and the pieces (`strings`).
#[derive(Debug, Clone)]
pub struct Doppeler {
    match_rex: Regex,
    mangler: Mangler,
}

static RE_NOCASE_SPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(<span)[{ws}]+(class="no(?:case|decor)")[^>]*(>)"#,
        ws = JS_WS_CLASS
    ))
    .unwrap_or_else(|_| unreachable_regex())
});
static RE_SMALLCAPS_SPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(<span)[{ws}]+(style="font-variant:)[{ws}]*(small-caps);?(")[^>]*(>)"#,
        ws = JS_WS_CLASS
    ))
    .unwrap_or_else(|_| unreachable_regex())
});

impl Doppeler {
    fn new(rex_str: &str, mangler: Mangler) -> Doppeler {
        Doppeler {
            match_rex: Regex::new(rex_str).unwrap_or_else(|_| unreachable_regex()),
            mangler,
        }
    }

    /// `Doppeler.prototype.split` (the `this.split` closure).
    pub fn split(&self, s: &str) -> Doppel {
        // Normalize markup
        let mangled;
        let s = match self.mangler {
            Mangler::None => s,
            Mangler::TagDoppel => {
                let a = RE_NOCASE_SPAN.replace_all(s, "${1} ${2}${3}").into_owned();
                mangled = RE_SMALLCAPS_SPAN
                    .replace_all(&a, "${1} ${2} ${3};${4}${5}")
                    .into_owned();
                mangled.as_str()
            }
        };
        let mut tags: Vec<String> = self
            .match_rex
            .find_iter(s)
            .map(|m| m.as_str().to_string())
            .collect();
        if tags.is_empty() {
            return Doppel {
                tags: Vec::new(),
                strings: vec![s.to_string()],
                orig_strings: Vec::new(),
            };
        }
        let mut strings: Vec<String> = self.match_rex.split(s).map(str::to_string).collect();
        for i in (0..tags.len()).rev() {
            if tags[i] == "'" && !strings[i + 1].is_empty() {
                // Fixes https://forums.zotero.org/discussion/comment/294317
                strings[i + 1] = format!("{}{}", tags[i], strings[i + 1]);
                tags[i] = String::new();
            }
        }
        Doppel {
            tags,
            orig_strings: strings.clone(),
            strings,
        }
    }

    /// `Doppeler.prototype.join`.
    pub fn join(&self, obj: &Doppel) -> String {
        // lst = obj.strings.slice(-1); then for i = last tag ... 0 push
        // tags[i], strings[i]; reverse; join("").
        let mut lst: Vec<&str> = Vec::new();
        if let Some(last) = obj.strings.last() {
            lst.push(last);
        }
        for i in (0..obj.tags.len()).rev() {
            lst.push(&obj.tags[i]);
            lst.push(obj.strings.get(i).map(String::as_str).unwrap_or(""));
        }
        lst.reverse();
        lst.concat()
    }
}

// The three instances formatters.js creates at load time.
const TAG_REX: &str = "(?:\u{2018}|\u{2019}|\u{201C}|\u{201D}| \"| '|\"|'|[-\u{2013}\u{2014}/.,;?!:]|\\[|\\]|\\(|\\)|<span style=\"font-variant: small-caps;\">|<span class=\"no(?:case|decor)\">|</span>|</?(?:i|sc|b|sub|sup)>)";

// Note: upstream's rexNameStr is a JS string literal in which `\s` (inside
// `span\s+class`) is just `s`, so that alternative reads `spans+class=...` and
// never matches a real `<span class="nocase">`. Reproduced as is.
const NAME_REX: &str =
    "(?:[-\\s]*</*(?:spans+class=\"no(?:case|decor)\"|i|sc|b|sub|sup)>[-\\s]*|[-\\s]+)";

static TAG_DOPPEL: LazyLock<Doppeler> =
    LazyLock::new(|| Doppeler::new(TAG_REX, Mangler::TagDoppel));
static NAME_DOPPEL: LazyLock<Doppeler> = LazyLock::new(|| {
    // `[-\s]` uses JS whitespace.
    let rex = NAME_REX.replace("\\s", JS_WS_CLASS);
    Doppeler::new(&rex, Mangler::None)
});
static WORD_DOPPEL: LazyLock<Doppeler> = LazyLock::new(|| {
    Doppeler::new(
        "(?:[\u{00A0}\u{0020}\u{00A0}\u{2000}-\u{200B}\u{205F}\u{3000}]+)",
        Mangler::None,
    )
});

/// `CSL.Output.Formatters.nameDoppel`.
pub fn name_doppel() -> &'static Doppeler {
    &NAME_DOPPEL
}

// ---------------------------------------------------------------------------
// Locale-aware case
// ---------------------------------------------------------------------------

/// Whether `tag` passes JS's `Intl` structural check for a language tag
/// (approximation of `CanonicalizeLocaleList`; a failing tag makes
/// `toLocale*Case` throw, which citeproc-js catches by falling back to
/// `toUpperCase`/`toLowerCase`).
fn is_valid_lang_tag(tag: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$").unwrap_or_else(|_| unreachable_regex())
    });
    RE.is_match(tag)
}

/// The special-casing language of `state.tmp.lang_array` (first element
/// decides, as `String.prototype.toLocaleUpperCase` does; any invalid tag
/// makes the whole call fall back to the plain method).
#[derive(PartialEq)]
enum CaseLang {
    Plain,
    Turkic,
}

fn case_lang(state: &State) -> CaseLang {
    let arr = &state.tmp.lang_array;
    if arr.iter().any(|t| !is_valid_lang_tag(t)) {
        return CaseLang::Plain;
    }
    match arr.first() {
        Some(first) => {
            let primary = first.split('-').next().unwrap_or("").to_ascii_lowercase();
            if primary == "tr" || primary == "az" {
                CaseLang::Turkic
            } else {
                CaseLang::Plain
            }
        }
        None => CaseLang::Plain,
    }
}

/// `CSL.toLocaleUpperCase.call(state, str)`: `str.toLocaleUpperCase(
/// state.tmp.lang_array)`, falling back to `toUpperCase()` when the tag list
/// is invalid. Reproduced: Turkic `i` to `\u{130}`. Not reproduced:
/// Lithuanian's retained-dot rules.
pub fn to_locale_upper_case(state: &State, s: &str) -> String {
    // DUP-CHECK: load.js CSL.toLocaleUpperCase
    if case_lang(state) == CaseLang::Turkic {
        let pre: String = s
            .chars()
            .map(|c| if c == 'i' { '\u{130}' } else { c })
            .collect();
        return pre.to_uppercase();
    }
    s.to_uppercase()
}

/// `CSL.toLocaleLowerCase.call(state, str)`; see [`to_locale_upper_case`].
pub fn to_locale_lower_case(state: &State, s: &str) -> String {
    // DUP-CHECK: load.js CSL.toLocaleLowerCase
    if case_lang(state) == CaseLang::Turkic {
        let pre = s.replace("I\u{307}", "i");
        let pre: String = pre
            .chars()
            .map(|c| match c {
                'I' => '\u{131}',
                '\u{130}' => 'i',
                other => other,
            })
            .collect();
        return pre.to_lowercase();
    }
    s.to_lowercase()
}

// ---------------------------------------------------------------------------
// The text-case engine
// ---------------------------------------------------------------------------

/// `_capitalise(word)`.
fn capitalise(state: &State, word: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        // JS: (^\s*)(<one non-terminator character>)(.*)  [`.` excludes
        // \n \r U+2028 U+2029]
        Regex::new(&format!(
            "^([{ws}]*)([^\n\r\\x{{2028}}\\x{{2029}}])([^\n\r\\x{{2028}}\\x{{2029}}]*)",
            ws = JS_WS_CLASS
        ))
        .unwrap_or_else(|_| unreachable_regex())
    });
    if let Some(m) = RE.captures(word) {
        let m1 = m.get(1).map(|x| x.as_str()).unwrap_or("");
        let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
        let m3 = m.get(3).map(|x| x.as_str()).unwrap_or("");
        // Do not uppercase lone Greek letters
        // (No case transforms in Greek citations, but chars used in titles to science papers)
        let lone_greek = {
            let mut cs = m2.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) => ('\u{0370}'..='\u{03FF}').contains(&c),
                _ => false,
            }
        };
        if !(lone_greek && m3.is_empty()) {
            return format!("{}{}{}", m1, to_locale_upper_case(state, m2), m3);
        }
    }
    word.to_string()
}

/// `config.quoteState` entry.
#[derive(Debug, Clone)]
struct QuoteEntry {
    opener: String,
    closer: String,
    /// `pos` (JS: undefined on entries `tryOpen` replaced, because of a
    /// `positions:` typo upstream).
    pos: Option<usize>,
}

/// Which `config.capitaliseWords` closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Lowercase,
    Uppercase,
    Sentence,
    Title,
    CapitalizeFirst,
    CapitalizeAll,
}

/// The `config` object built by each public formatter.
struct Config {
    case: Case,
    /// `quoteState`: `None` is JS `null`.
    quote_state: Option<Vec<QuoteEntry>>,
    tag_state: Vec<String>,
    /// `afterPunct`: `None` is `null`.
    after_punct: Option<bool>,
    /// `isFirst`: `None` is `null`.
    is_first: Option<bool>,
    last_word_pos: Option<(usize, usize)>,
    skip_words_rex: Option<Regex>,
    doppel: Doppel,
}

fn tag_params(tag: &str) -> Option<&'static str> {
    match tag {
        "<span style=\"font-variant: small-caps;\">" => Some("</span>"),
        "<span class=\"nocase\">" => Some("</span>"),
        "<span class=\"nodecor\">" => Some("</span>"),
        "<sc>" => Some("</sc>"),
        "<sub>" => Some("</sub>"),
        "<sup>" => Some("</sup>"),
        _ => None,
    }
}

fn quote_params(tag: &str) -> Option<(&'static str, &'static str)> {
    match tag {
        " \"" => Some((" '", "\"")),
        " '" => Some((" \"", "'")),
        "\u{2018}" => Some(("\u{2018}", "\u{2019}")),
        "\u{201C}" => Some(("\u{201C}", "\u{201D}")),
        _ => None,
    }
}

impl Config {
    /// `tryOpen(tag, pos)`: `None` for JS `false`/`undefined`.
    fn try_open(&mut self, tag: &str, pos: usize) -> Option<usize> {
        let qs = self.quote_state.get_or_insert_with(Vec::new);
        let (opener, closer) = quote_params(tag).unwrap_or(("", ""));
        let fits = match qs.last() {
            None => true,
            Some(top) => tag == top.opener,
        };
        if fits {
            qs.push(QuoteEntry {
                opener: opener.to_string(),
                closer: closer.to_string(),
                pos: Some(pos),
            });
            None
        } else {
            let prev = qs.last().and_then(|t| t.pos);
            qs.pop();
            qs.push(QuoteEntry {
                opener: opener.to_string(),
                closer: closer.to_string(),
                pos: None, // upstream typo: `positions: pos`
            });
            prev
        }
    }

    /// `tryClose(tag, pos)`.
    fn try_close(&mut self, tag: &str, pos: usize) -> Option<usize> {
        let qs = self.quote_state.get_or_insert_with(Vec::new);
        if let Some(top) = qs.last() {
            if tag == top.closer {
                qs.pop();
                return None;
            }
        }
        Some(pos)
    }

    /// `quoteFix(tag, positions)`.
    fn quote_fix(&mut self, tag: &str, positions: usize) -> Option<usize> {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new("(^(?:\u{2018}|\u{2019}|\u{201C}|\u{201D}|\"|')|(?: \"| ')$)")
                .unwrap_or_else(|_| unreachable_regex())
        });
        let m = RE.captures(tag)?;
        let m1 = m.get(1)?.as_str().to_string();
        // pushQuoteState
        let is_opener = ["\u{201C}", "\u{2018}", " \"", " '"].contains(&m1.as_str());
        if is_opener {
            self.try_open(&m1, positions)
        } else {
            self.try_close(&m1, positions)
        }
    }

    /// `config.capitaliseWords(str, i, followingTag)`.
    fn capitalise_words(
        &mut self,
        state: &State,
        s: &str,
        i: usize,
        following_tag: Option<&str>,
    ) -> String {
        match self.case {
            Case::Lowercase => {
                let mut words: Vec<String> = s.split(' ').map(str::to_string).collect();
                for w in words.iter_mut() {
                    if !w.is_empty() {
                        *w = to_locale_lower_case(state, w);
                    }
                }
                words.join(" ")
            }
            Case::Uppercase => {
                let mut words: Vec<String> = s.split(' ').map(str::to_string).collect();
                for w in words.iter_mut() {
                    if !w.is_empty() {
                        *w = to_locale_upper_case(state, w);
                    }
                }
                words.join(" ")
            }
            Case::Sentence => {
                let mut words: Vec<String> = s.split(' ').map(str::to_string).collect();
                for w in words.iter_mut() {
                    if !w.is_empty() {
                        if self.is_first == Some(true) {
                            *w = capitalise(state, w);
                            self.is_first = Some(false);
                        } else {
                            *w = to_locale_lower_case(state, w);
                        }
                    }
                }
                words.join(" ")
            }
            Case::Title => {
                if js_trim(s).is_empty() {
                    return s.to_string();
                }
                let mut wordle = WORD_DOPPEL.split(s);
                let n = wordle.strings.len();
                for j in 0..n {
                    let word = wordle.strings[j].clone();
                    if word.is_empty() {
                        continue;
                    }
                    let lcase = to_locale_lower_case(state, &word);
                    let is_skip = self
                        .skip_words_rex
                        .as_ref()
                        .map(|r| r.is_match(&lcase))
                        .unwrap_or(false);
                    let mut capitalize = false;
                    if crate::citeproc::js::len(&word) > 1 && !is_skip {
                        // Capitalize every word that is not a stop-word
                        capitalize = true;
                    } else if j == n - 1 && following_tag == Some("-") {
                        capitalize = true;
                    } else if self.is_first == Some(true) {
                        // Capitalize first word, even if a stop-word
                        capitalize = true;
                    } else if self.after_punct == Some(true) {
                        // Capitalize after punctuation
                        capitalize = true;
                    }
                    // Don't capitalize if word already contains capitalization
                    if capitalize && word == lcase {
                        wordle.strings[j] = capitalise(state, &word);
                    }
                    self.after_punct = Some(false);
                    self.is_first = Some(false);
                    self.last_word_pos = Some((i, j));
                }
                WORD_DOPPEL.join(&wordle)
            }
            Case::CapitalizeFirst => {
                let mut wordle = WORD_DOPPEL.split(s);
                for j in 0..wordle.strings.len() {
                    let word = wordle.strings[j].clone();
                    if !word.is_empty() && self.is_first == Some(true) {
                        // Don't capitalize if word already contains capitalization
                        if word == to_locale_lower_case(state, &word) {
                            wordle.strings[j] = capitalise(state, &word);
                        }
                        self.is_first = Some(false);
                        break;
                    }
                }
                WORD_DOPPEL.join(&wordle)
            }
            Case::CapitalizeAll => {
                let mut wordle = WORD_DOPPEL.split(s);
                for j in 0..wordle.strings.len() {
                    let word = wordle.strings[j].clone();
                    if !word.is_empty() {
                        // Don't capitalize if word already contains capitalization
                        if word == to_locale_lower_case(state, &word) {
                            wordle.strings[j] = capitalise(state, &word);
                        }
                    }
                }
                WORD_DOPPEL.join(&wordle)
            }
        }
    }
}

/// `_textcaseEngine(config, string)`.
fn textcase_engine(state: &State, config: &mut Config, string: &str) -> String {
    if string.is_empty() {
        return String::new();
    }
    config.doppel = TAG_DOPPEL.split(string);

    // Run state machine
    if !config.doppel.strings.is_empty() && !js_trim(&config.doppel.strings[0]).is_empty() {
        let s0 = config.doppel.strings[0].clone();
        let t0 = config.doppel.tags.first().cloned();
        config.doppel.strings[0] = config.capitalise_words(state, &s0, 0, t0.as_deref());
    }

    let ilen = config.doppel.tags.len();
    for i in 0..ilen {
        let tag = config.doppel.tags[i].clone();
        let s = config.doppel.strings[i + 1].clone();

        // (config.tagState !== null is always true: every config has an array)
        if let Some(closer) = tag_params(&tag) {
            config.tag_state.push(closer.to_string());
        } else if config.tag_state.last().map(|t| *t == tag).unwrap_or(false) {
            config.tag_state.pop();
        }

        if config.after_punct.is_some() {
            // Evaluate punctuation state of current string
            if matches!(tag.chars().last(), Some('!') | Some('?') | Some(':')) {
                config.after_punct = Some(true);
            }
        }

        // Process if outside tag scope, else noop for upper-casing
        if config.tag_state.is_empty() {
            // Upstream passes (str, i+1, config.doppel, tags[i+1]): the
            // fourth argument is the following tag, but `followingTag` is
            // the *third* parameter, so inside the loop it is the doppel
            // object and never equals "-".
            let r = config.capitalise_words(state, &s, i + 1, None);
            config.doppel.strings[i + 1] = r;
        } else if !js_trim(&config.doppel.strings[i + 1]).is_empty() {
            config.last_word_pos = None;
        }

        if config.quote_state.is_some() {
            // Evaluate quote state of current string and fix chars that have flown
            if let Some(quote_pos) = config.quote_fix(&tag, i) {
                let orig_char = config
                    .doppel
                    .orig_strings
                    .get(quote_pos + 1)
                    .map(|o| super::obj_blob::first_char(o).to_string())
                    .unwrap_or_default();
                let rest =
                    super::obj_blob::drop_first(&config.doppel.strings[quote_pos + 1]).to_string();
                config.doppel.strings[quote_pos + 1] = format!("{orig_char}{rest}");
                config.last_word_pos = None;
            }
        }

        // If there was a printable string, unset first-word and after-punctuation
        if config.is_first == Some(true) && !js_trim(&s).is_empty() {
            config.is_first = Some(false);
        }
        if config.after_punct == Some(true) && !js_trim(&s).is_empty() {
            config.after_punct = Some(false);
        }
    }
    if let Some(qs) = config.quote_state.clone() {
        for e in qs.iter() {
            // Test for quotePos avoids a crashing error (flipflop_OrphanQuote)
            if let Some(quote_pos) = e.pos {
                let orig_char = config
                    .doppel
                    .orig_strings
                    .get(quote_pos + 1)
                    .map(|o| super::obj_blob::first_char(o).to_string())
                    .unwrap_or_default();
                let rest =
                    super::obj_blob::drop_first(&config.doppel.strings[quote_pos + 1]).to_string();
                config.doppel.strings[quote_pos + 1] = format!("{orig_char}{rest}");
            }
        }
    }
    // Specially capitalize the last word if necessary (invert stop-word list)
    if let Some((si, wi)) = config.last_word_pos {
        let mut last_words = WORD_DOPPEL.split(&config.doppel.strings[si]);
        if let Some(last_word) = last_words.strings.get(wi).cloned() {
            let lower = to_locale_lower_case(state, &last_word);
            let is_skip = config
                .skip_words_rex
                .as_ref()
                .map(|r| r.is_match(&lower))
                .unwrap_or(false);
            if crate::citeproc::js::len(&last_word) > 1 && is_skip {
                last_words.strings[wi] = capitalise(state, &last_word);
            }
        }
        config.doppel.strings[si] = WORD_DOPPEL.join(&last_words);
    }

    // Recombine the string
    TAG_DOPPEL.join(&config.doppel)
}

fn new_config(
    case: Case,
    quote_state: Option<Vec<QuoteEntry>>,
    after_punct: Option<bool>,
    is_first: Option<bool>,
    skip: Option<Regex>,
) -> Config {
    Config {
        case,
        quote_state,
        tag_state: Vec::new(),
        after_punct,
        is_first,
        last_word_pos: None,
        skip_words_rex: skip,
        doppel: Doppel::default(),
    }
}

/// `CSL.Output.Formatters.passthrough`: a noop that delivers the string.
pub fn passthrough(_state: &State, s: &str) -> String {
    s.to_string()
}

/// `CSL.Output.Formatters.lowercase`: force all letters to lowercase,
/// skipping nocase spans.
pub fn lowercase(state: &State, string: &str) -> String {
    let mut c = new_config(Case::Lowercase, None, None, None, None);
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters.uppercase`: force all letters to uppercase.
pub fn uppercase(state: &State, string: &str) -> String {
    let mut c = new_config(Case::Uppercase, None, None, None, None);
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters.sentence`: capitalise the first word, lowercase
/// the rest.
pub fn sentence(state: &State, string: &str) -> String {
    let mut c = new_config(Case::Sentence, Some(Vec::new()), None, Some(true), None);
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters.title`: title case with the locale's stop words
/// (see the module docs for where the stop-word regexp comes from).
pub fn title(state: &State, string: &str) -> String {
    let mut c = new_config(
        Case::Title,
        Some(Vec::new()),
        Some(false),
        Some(true),
        Some(skip_words_rex(state)),
    );
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters["capitalize-first"]`: capitalise the first letter,
/// leave the rest untouched.
pub fn capitalize_first(state: &State, string: &str) -> String {
    let mut c = new_config(
        Case::CapitalizeFirst,
        Some(Vec::new()),
        None,
        Some(true),
        None,
    );
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters["capitalize-all"]`: capitalise the first letter of
/// each space-delimited word.
pub fn capitalize_all(state: &State, string: &str) -> String {
    let mut c = new_config(Case::CapitalizeAll, Some(Vec::new()), None, None, None);
    textcase_engine(state, &mut c, string)
}

/// `CSL.Output.Formatters[name](state, string)`. An unknown `name` is a
/// JS TypeError upstream, an `Err` here.
pub fn apply(state: &State, name: &str, string: &str) -> CslResult<String> {
    match name {
        "passthrough" => Ok(passthrough(state, string)),
        "lowercase" => Ok(lowercase(state, string)),
        "uppercase" => Ok(uppercase(state, string)),
        "sentence" => Ok(sentence(state, string)),
        "title" => Ok(title(state, string)),
        "capitalize-first" => Ok(capitalize_first(state, string)),
        "capitalize-all" => Ok(capitalize_all(state, string)),
        other => Err(EngineError::Csl(format!(
            "unknown text-case formatter: {other}"
        ))),
    }
}

/// Whether `name` names a text-case formatter (`CSL.Output.Formatters[name]`
/// is a function).
pub fn is_formatter(name: &str) -> bool {
    matches!(
        name,
        "passthrough"
            | "lowercase"
            | "uppercase"
            | "sentence"
            | "title"
            | "capitalize-first"
            | "capitalize-all"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Reference output of citeproc-js 2.4.63's `CSL.Output.Formatters`,
    /// generated by `scripts/csl-units/formatters.cjs` (an en-US engine;
    /// inputs: the CSL test suite's INPUT strings, hand-made edge cases and
    /// 1800 generated strings mixing markup, quotes, stop words and
    /// punctuation; plus a language-tag subset).
    const DATA: &str = include_str!("../../tests/data/csl/units/formatters.json");

    #[test]
    fn default_skip_words_regex_compiles() {
        let lst: Vec<String> = SKIP_WORDS.iter().map(|s| s.to_string()).collect();
        assert!(make_skip_words_regex(&lst).is_ok());
        assert!(NAME_DOPPEL.match_rex.is_match("a - b"));
    }

    #[test]
    fn name_doppel_matches_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let mut bad = Vec::new();
        let list = v["doppel"].as_array().unwrap();
        assert!(list.len() > 500);
        for c in list {
            let d = name_doppel().split(c["s"].as_str().unwrap());
            let strs = |x: &Value| -> Vec<String> {
                x.as_array()
                    .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
                    .unwrap_or_default()
            };
            let ok = d.tags == strs(&c["tags"])
                && d.strings == strs(&c["strings"])
                && (c["orig"].is_null() || d.orig_strings == strs(&c["orig"]))
                && name_doppel().join(&d) == c["joined"].as_str().unwrap();
            if !ok {
                bad.push(format!("{}: got {:?} / {:?}", c["s"], d.tags, d.strings));
            }
        }
        assert!(
            bad.is_empty(),
            "{} mismatches:\n{}",
            bad.len(),
            bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
        );
    }

    #[test]
    fn text_case_matches_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let names: Vec<String> = v["names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap().to_string())
            .collect();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() > 3000);
        let mut bad = Vec::new();
        for c in cases {
            let input = c["s"].as_str().unwrap();
            let mut st = State::default();
            if let Some(la) = c["la"].as_array() {
                st.tmp.lang_array = la.iter().map(|x| x.as_str().unwrap().to_string()).collect();
            }
            for (i, name) in names.iter().enumerate() {
                let want = &c["r"][i];
                let got = apply(&st, name, input).unwrap();
                match want.as_str() {
                    Some(w) => {
                        if got != w {
                            bad.push(format!("{name}({input:?}) = {got:?}, want {w:?}"));
                        }
                    }
                    None => bad.push(format!("{name}({input:?}): reference threw {want}")),
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} mismatches, first 25:\n{}",
            bad.len(),
            bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}
