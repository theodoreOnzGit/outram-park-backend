// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/formats.js (CSL.Mode and CSL.substituteOne, the decorator
//              lookup, are util_processor.rs; CSL.getSafeEscape and the
//              constants are load.rs)
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

//! `CSL.Output.Formats`: the output formats (html, text, rtf, asciidoc,
//! xsl-fo, latex), each with its text escaper, bibliography wrapper strings
//! and decorators (`@font-style/italic`, `@quotes/true`, `@display/block`,
//! `@bibliography/entry`, `@URL/true`, ...).
//!
//! # How it maps onto the JS
//!
//! * `CSL.Output.Formats[mode]` is [`Format`], an enum. `setOutputFormat(mode)`
//!   (`this.opt.mode = mode; this.fun.decorate = CSL.Mode(mode)`) is
//!   [`set_output_format`], which stores the enum in `state.fun.decorate`.
//! * `state.fun.decorate[name][value].call(blob, state, str, extra)` is
//!   [`decorate`]. A template entry (`"<i>%%STRING%%</i>"`) behaves like
//!   `CSL.substituteOne` (including JS's `$`-patterns in `String.replace`
//!   replacement text); a `false` entry is `passthrough`; a function entry
//!   is the Rust function of the same name.
//! * `CSL.getSafeEscape(state)` (load.js) is `load::get_safe_escape`; the
//!   format's `text_escape` is [`text_escape`].
//!
//! # Host callbacks
//!
//! Some decorators call `state.sys.variableWrapper`, `wrapCitationEntry` or
//! `embedBibliographyEntry`. The port's `Sys` is data only, so [`HostHooks`]
//! (in `state.fun.host_hooks`) records which exist. Only the
//! `variableWrapper` the citeproc-js test runner installs
//! (test_runner.js:223) is implemented; the other two return
//! `Err(NotYetPorted)` when their hook is enabled and `Err(Csl)` when it is
//! not (JS: a TypeError on calling `undefined`).
//!
//! # Terms
//!
//! `state.getTerm("open-quote")` and `state.getOpt("punctuation-in-quote")`
//! are `State::get_term_no_flag` / `State::get_opt` (build.rs); [`get_term`]
//! and [`get_opt_flag`] are thin readers over them.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js;
use super::obj_blob::{Blob, JS_WS_CLASS};
use super::state::State;
use super::{CslResult, EngineError, OutputFormat};

use super::load::{SUPERSCRIPTS, SWAPPING_PUNCTUATION};

/// The plain text `CSL.SUPERSCRIPTS[c]` for a superscript character.
pub fn superscript_for(c: char) -> Option<&'static str> {
    SUPERSCRIPTS.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// `str.replace(CSL.SUPERSCRIPTS_REGEXP, function (aChar) { return wrap(SUPERSCRIPTS[aChar]) })`.
fn replace_superscripts(s: &str, wrap: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match superscript_for(c) {
            Some(plain) => out.push_str(&wrap(plain)),
            None => out.push(c),
        }
    }
    out
}

/// Which optional `sys` callbacks the host supplies (JS tests the property
/// for existence). All `false` by default, as in the fixture harness except
/// where a test sets the `variableWrapper` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostHooks {
    /// `sys.variableWrapper` exists.
    pub variable_wrapper: bool,
    /// `sys.wrapCitationEntry` exists.
    pub wrap_citation_entry: bool,
    /// `sys.embedBibliographyEntry` exists.
    pub embed_bibliography_entry: bool,
}

/// `CSL.Output.Formats[mode]`: an output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// `html` (the engine default).
    #[default]
    Html,
    /// `text`.
    Text,
    /// `rtf`.
    Rtf,
    /// `asciidoc`.
    Asciidoc,
    /// `fo` (XSL-FO; the `xslfo` of the port's `OutputFormat`).
    Fo,
    /// `latex`.
    Latex,
}

impl Format {
    /// The mode string `setOutputFormat` takes (`"html"`, `"text"`, `"rtf"`,
    /// `"asciidoc"`, `"fo"`, `"latex"`).
    pub fn mode_name(self) -> &'static str {
        match self {
            Format::Html => "html",
            Format::Text => "text",
            Format::Rtf => "rtf",
            Format::Asciidoc => "asciidoc",
            Format::Fo => "fo",
            Format::Latex => "latex",
        }
    }

    /// `CSL.Output.Formats[mode]` lookup; `None` for an unknown mode.
    pub fn from_mode(mode: &str) -> Option<Format> {
        match mode {
            "html" => Some(Format::Html),
            "text" => Some(Format::Text),
            "rtf" => Some(Format::Rtf),
            "asciidoc" => Some(Format::Asciidoc),
            "fo" => Some(Format::Fo),
            "latex" => Some(Format::Latex),
            _ => None,
        }
    }

    /// `decorate.bibstart` (`CSL.Mode` copies the non-`@` entries across).
    pub fn bibstart(self) -> &'static str {
        match self {
            Format::Html => "<div class=\"csl-bib-body\">\n",
            Format::Text | Format::Asciidoc | Format::Fo => "",
            Format::Rtf => "{\\rtf ",
            Format::Latex => "\\begin{thebibliography}{4}",
        }
    }

    /// `decorate.bibend`.
    pub fn bibend(self) -> &'static str {
        match self {
            Format::Html => "</div>",
            Format::Text | Format::Asciidoc | Format::Fo => "",
            Format::Rtf => "}",
            Format::Latex => "\\end{thebibliography}",
        }
    }
}

impl From<OutputFormat> for Format {
    /// The port's public `OutputFormat` onto the JS mode: `Plain` is
    /// `"text"`, `Xslfo` is `"fo"`.
    fn from(f: OutputFormat) -> Format {
        match f {
            OutputFormat::Html => Format::Html,
            OutputFormat::Rtf => Format::Rtf,
            OutputFormat::Plain => Format::Text,
            OutputFormat::Asciidoc => Format::Asciidoc,
            OutputFormat::Xslfo => Format::Fo,
        }
    }
}

/// `CSL.Engine.prototype.setOutputFormat(mode)` (api_control.js:3):
/// `this.opt.mode = mode; this.fun.decorate = CSL.Mode(mode)`. (The
/// `this.output[mode] = {tmp: {}}` bag is never read and is not kept.)
/// An unknown `mode` is an `Err`.
pub fn set_output_format(state: &mut State, mode: &str) -> CslResult<()> {
    let f = Format::from_mode(mode)
        .ok_or_else(|| EngineError::Csl(format!("unknown output format: {mode}")))?;
    state
        .opt
        .insert("mode".into(), Value::String(mode.to_string()));
    state.fun.decorate = f;
    Ok(())
}

/// The format `CSL.Output.Formats[state.opt.mode]` denotes (falling back to
/// `state.fun.decorate` when `opt.mode` is unset, which `setOutputFormat`
/// never leaves).
pub fn current_format(state: &State) -> Format {
    match state.opt.get("mode").and_then(Value::as_str) {
        Some(m) => Format::from_mode(m).unwrap_or(state.fun.decorate),
        None => state.fun.decorate,
    }
}

// ---------------------------------------------------------------------------
// text_escape
// ---------------------------------------------------------------------------

static RE_TWO_WS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("[{ws}][{ws}]", ws = JS_WS_CLASS)).unwrap_or_else(|_| never())
});

fn never() -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(r"[^\s\S]").expect("constant regex")
}

/// `CSL.Output.Formats[mode].text_escape(text)`: escape `text` for output in
/// `format`. (JS: a falsy `text` becomes `""`.)
pub fn text_escape(format: Format, text: &str) -> String {
    match format {
        Format::Html => {
            // Numeric entities, in case the output is processed as xml.
            let t = text
                .replace('&', "&#38;")
                .replace('<', "&#60;")
                .replace('>', "&#62;");
            let t = RE_TWO_WS.replace_all(&t, "\u{00A0} ").into_owned();
            replace_superscripts(&t, |p| format!("<sup>{p}</sup>"))
        }
        Format::Text | Format::Latex => text.to_string(),
        Format::Rtf => {
            let mut t = String::with_capacity(text.len());
            for c in text.chars() {
                if matches!(c, '\\' | '{' | '}') {
                    t.push('\\');
                }
                t.push(c);
            }
            let t = replace_superscripts(&t, |p| format!("\\super {p}\\nosupersub{{}}"));
            let mut out = String::with_capacity(t.len());
            for c in t.chars() {
                let cp = c as u32;
                if (0x7F..=0xFFFF).contains(&cp) {
                    out.push_str(&format!("\\uc0\\u{cp}{{}}"));
                } else if cp > 0xFFFF {
                    // JS matches each UTF-16 code unit separately.
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf).iter() {
                        out.push_str(&format!("\\uc0\\u{u}{{}}"));
                    }
                } else {
                    out.push(c);
                }
            }
            out.split('\t').collect::<Vec<_>>().join("\\tab{}")
        }
        Format::Asciidoc => {
            // `String.prototype.replace(str, str, "g")`: V8 ignores the third
            // argument, so only the FIRST occurrence of each is replaced.
            let t = text
                .replacen('*', "pass:[*]", 1)
                .replacen('_', "pass:[_]", 1)
                .replacen('#', "pass:[#]", 1)
                .replacen('^', "pass:[^]", 1)
                .replacen('~', "pass:[~]", 1)
                .replacen("[[", "pass:[[[]", 1)
                .replacen("  ", "&#160; ", 1);
            replace_superscripts(&t, |p| format!("^{p}^"))
        }
        Format::Fo => {
            let t = text
                .replace('&', "&#38;")
                .replace('<', "&#60;")
                .replace('>', "&#62;")
                .replacen("  ", "&#160; ", 1); // first occurrence only, as above
            replace_superscripts(&t, |p| {
                format!("<fo:inline vertical-align=\"super\">{p}</fo:inline>")
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Readers of build.js (see the module docs)
// ---------------------------------------------------------------------------

/// `state.getTerm(name)` as a string (`""` for `undefined`, which JS would
/// splice in as the text "undefined"; every locale defines the terms read
/// here). Without `getTerm`'s `cite_renders_content` side effect, see
/// [`State::get_term_no_flag`].
pub fn get_term(state: &State, name: &str) -> String {
    state
        .get_term_no_flag(name, None, None, None, None, false)
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// `state.getOpt(name)` as a boolean flag: `state.locale[opt.lang].opts[name]`
/// (`false` when unset or when the locale is missing).
pub fn get_opt_flag(state: &State, name: &str) -> bool {
    state.get_opt(name).map(|v| js::truthy(&v)).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// decorate
// ---------------------------------------------------------------------------

/// `String.prototype.replace(pattern, replacement)` for a **string** pattern
/// (first occurrence only) with JS's replacement patterns: `$$`, `$&`, `` $` ``,
/// `$'` are expanded; `$n` stays literal because a string pattern has no
/// capture groups.
pub fn js_replace_first(subject: &str, pattern: &str, replacement: &str) -> String {
    let Some(pos) = subject.find(pattern) else {
        return subject.to_string();
    };
    let before = &subject[..pos];
    let after = &subject[pos + pattern.len()..];
    let mut out = String::with_capacity(subject.len() + replacement.len());
    out.push_str(before);
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            match chars.peek() {
                Some('$') => {
                    out.push('$');
                    chars.next();
                }
                Some('&') => {
                    out.push_str(pattern);
                    chars.next();
                }
                Some('`') => {
                    out.push_str(before);
                    chars.next();
                }
                Some('\'') => {
                    out.push_str(after);
                    chars.next();
                }
                _ => out.push('$'),
            }
        } else {
            out.push(c);
        }
    }
    out.push_str(after);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Func {
    QuotesTrue,
    QuotesInner,
    CiteEntry,
    BibEntry,
    DisplayBlock,
    DisplayLeftMargin,
    DisplayRightInline,
    DisplayIndent,
    ShowId,
    Url,
    Doi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// A `%%STRING%%` template.
    Tpl(&'static str),
    /// `false`, or `CSL.Output.Formatters.passthrough`.
    Passthrough,
    /// A function entry.
    Func(Func),
}

/// The template (`Some`) or `false` (`None`) for the 13 font-style-like
/// decorators, per format; `Err(())` when the key is not one of them.
fn font_entry(format: Format, key: &str) -> Option<Option<&'static str>> {
    let k = key;
    let r: Option<&'static str> = match format {
        Format::Html => match k {
            "@font-style/italic" => Some("<i>%%STRING%%</i>"),
            "@font-style/oblique" => Some("<em>%%STRING%%</em>"),
            "@font-style/normal" => Some("<span style=\"font-style:normal;\">%%STRING%%</span>"),
            "@font-variant/small-caps" => {
                Some("<span style=\"font-variant:small-caps;\">%%STRING%%</span>")
            }
            "@font-variant/normal" => {
                Some("<span style=\"font-variant:normal;\">%%STRING%%</span>")
            }
            "@font-weight/bold" => Some("<b>%%STRING%%</b>"),
            "@font-weight/normal" => Some("<span style=\"font-weight:normal;\">%%STRING%%</span>"),
            "@font-weight/light" => None,
            "@text-decoration/none" => {
                Some("<span style=\"text-decoration:none;\">%%STRING%%</span>")
            }
            "@text-decoration/underline" => {
                Some("<span style=\"text-decoration:underline;\">%%STRING%%</span>")
            }
            "@vertical-align/sup" => Some("<sup>%%STRING%%</sup>"),
            "@vertical-align/sub" => Some("<sub>%%STRING%%</sub>"),
            "@vertical-align/baseline" => Some("<span style=\"baseline\">%%STRING%%</span>"),
            _ => return None,
        },
        Format::Text => match k {
            "@font-style/italic"
            | "@font-style/oblique"
            | "@font-style/normal"
            | "@font-variant/small-caps"
            | "@font-variant/normal"
            | "@font-weight/bold"
            | "@font-weight/normal"
            | "@font-weight/light"
            | "@text-decoration/none"
            | "@text-decoration/underline"
            | "@vertical-align/baseline"
            | "@vertical-align/sup"
            | "@vertical-align/sub" => None,
            _ => return None,
        },
        Format::Rtf => match k {
            "@font-style/italic" => Some("{\\i{}%%STRING%%}"),
            "@font-style/normal" => Some("{\\i0{}%%STRING%%}"),
            "@font-style/oblique" => Some("{\\i{}%%STRING%%}"),
            "@font-variant/small-caps" => Some("{\\scaps %%STRING%%}"),
            "@font-variant/normal" => Some("{\\scaps0{}%%STRING%%}"),
            "@font-weight/bold" => Some("{\\b{}%%STRING%%}"),
            "@font-weight/normal" => Some("{\\b0{}%%STRING%%}"),
            "@font-weight/light" => None,
            "@text-decoration/none" => None,
            "@text-decoration/underline" => Some("{\\ul{}%%STRING%%}"),
            "@vertical-align/baseline" => None,
            "@vertical-align/sup" => Some("\\super %%STRING%%\\nosupersub{}"),
            "@vertical-align/sub" => Some("\\sub %%STRING%%\\nosupersub{}"),
            _ => return None,
        },
        Format::Asciidoc => match k {
            "@font-style/italic" => Some("__%%STRING%%__"),
            "@font-style/oblique" => Some("__%%STRING%%__"),
            "@font-style/normal" => None,
            "@font-variant/small-caps" => Some("[small-caps]#%%STRING%%#"),
            "@font-variant/normal" => None,
            "@font-weight/bold" => Some("**%%STRING%%**"),
            "@font-weight/normal" => None,
            "@font-weight/light" => None,
            "@text-decoration/none" => None,
            "@text-decoration/underline" => Some("[underline]##%%STRING%%##"),
            "@vertical-align/sup" => Some("^^%%STRING%%^^"),
            "@vertical-align/sub" => Some("~~%%STRING%%~~"),
            "@vertical-align/baseline" => None,
            _ => return None,
        },
        Format::Fo => match k {
            "@font-style/italic" => Some("<fo:inline font-style=\"italic\">%%STRING%%</fo:inline>"),
            "@font-style/oblique" => {
                Some("<fo:inline font-style=\"oblique\">%%STRING%%</fo:inline>")
            }
            "@font-style/normal" => Some("<fo:inline font-style=\"normal\">%%STRING%%</fo:inline>"),
            "@font-variant/small-caps" => {
                Some("<fo:inline font-variant=\"small-caps\">%%STRING%%</fo:inline>")
            }
            "@font-variant/normal" => {
                Some("<fo:inline font-variant=\"normal\">%%STRING%%</fo:inline>")
            }
            "@font-weight/bold" => Some("<fo:inline font-weight=\"bold\">%%STRING%%</fo:inline>"),
            "@font-weight/normal" => {
                Some("<fo:inline font-weight=\"normal\">%%STRING%%</fo:inline>")
            }
            "@font-weight/light" => {
                Some("<fo:inline font-weight=\"lighter\">%%STRING%%</fo:inline>")
            }
            "@text-decoration/none" => {
                Some("<fo:inline text-decoration=\"none\">%%STRING%%</fo:inline>")
            }
            "@text-decoration/underline" => {
                Some("<fo:inline text-decoration=\"underline\">%%STRING%%</fo:inline>")
            }
            "@vertical-align/sup" => {
                Some("<fo:inline vertical-align=\"super\">%%STRING%%</fo:inline>")
            }
            "@vertical-align/sub" => {
                Some("<fo:inline vertical-align=\"sub\">%%STRING%%</fo:inline>")
            }
            "@vertical-align/baseline" => {
                Some("<fo:inline vertical-align=\"baseline\">%%STRING%%</fo:inline>")
            }
            _ => return None,
        },
        Format::Latex => match k {
            "@font-style/italic" => Some("{\\em %%STRING%%}"),
            "@font-weight/bold" => Some("{\\bf %%STRING%%}"),
            "@font-style/oblique"
            | "@font-style/normal"
            | "@font-variant/small-caps"
            | "@font-variant/normal"
            | "@font-weight/normal"
            | "@font-weight/light"
            | "@text-decoration/none"
            | "@text-decoration/underline"
            | "@vertical-align/baseline"
            | "@vertical-align/sup"
            | "@vertical-align/sub" => None,
            _ => return None,
        },
    };
    Some(r)
}

/// `CSL.Mode(mode)`'s lookup: the entry for `decorate[name][value]`.
fn entry(format: Format, name: &str, value: &str) -> Option<Entry> {
    let key = format!("{name}/{value}");
    if let Some(t) = font_entry(format, &key) {
        return Some(match t {
            Some(tpl) => Entry::Tpl(tpl),
            None => Entry::Passthrough,
        });
    }
    match key.as_str() {
        "@passthrough/true" | "@strip-periods/true" | "@strip-periods/false" | "@quotes/false" => {
            return Some(Entry::Passthrough)
        }
        "@quotes/true" => return Some(Entry::Func(Func::QuotesTrue)),
        "@quotes/inner" => return Some(Entry::Func(Func::QuotesInner)),
        "@cite/entry" => return Some(Entry::Func(Func::CiteEntry)),
        "@bibliography/entry" => return Some(Entry::Func(Func::BibEntry)),
        "@display/block" => {
            return Some(if format == Format::Rtf {
                Entry::Tpl("\\line{}%%STRING%%\\line\r\n")
            } else {
                Entry::Func(Func::DisplayBlock)
            })
        }
        "@display/left-margin" => return Some(Entry::Func(Func::DisplayLeftMargin)),
        "@display/right-inline" => return Some(Entry::Func(Func::DisplayRightInline)),
        "@display/indent" => return Some(Entry::Func(Func::DisplayIndent)),
        "@showid/true" => return Some(Entry::Func(Func::ShowId)),
        "@URL/true" => return Some(Entry::Func(Func::Url)),
        "@DOI/true" => return Some(Entry::Func(Func::Doi)),
        _ => {}
    }
    None
}

/// `state.fun.decorate[name][value].call(blob, state, str, extra)`.
///
/// * `blob` is JS `this` (some decorators read `this.item_id`, `this.params`,
///   `this.system_id` from [`Blob::extra`]).
/// * `s` is `str`; `None` is JS `undefined` (only `@quotes/*` act on that).
/// * `extra` is the third decoration element (`@showid`'s node id).
///
/// An unknown `name`/`value` is a JS TypeError (`undefined` is not a
/// function); here an `Err`.
pub fn decorate(
    state: &State,
    blob: Option<&Blob>,
    name: &str,
    value: &str,
    s: Option<&str>,
    extra: Option<&str>,
) -> CslResult<String> {
    let format = state.fun.decorate;
    match entry(format, name, value) {
        None => Err(EngineError::Csl(format!(
            "no {} decorator for {name}/{value}",
            format.mode_name()
        ))),
        Some(Entry::Tpl(t)) => Ok(super::util_processor::substitute_one(t, s)),
        Some(Entry::Passthrough) => Ok(s.unwrap_or("").to_string()),
        Some(Entry::Func(f)) => run_func(format, f, state, blob, s, extra),
    }
}

fn blob_extra<'b>(blob: Option<&'b Blob>, key: &str) -> Option<&'b Value> {
    blob.and_then(|b| b.extra.get(key))
}

/// `"" + this.<key>` as JS would print it (`"undefined"` when absent).
fn js_prop_string(blob: Option<&Blob>, key: &str) -> String {
    match blob_extra(blob, key) {
        Some(v) => js::to_js_string(v),
        None => "undefined".to_string(),
    }
}

static RE_PREPUNCT: LazyLock<Regex> = LazyLock::new(|| {
    // CSL.VARIABLE_WRAPPER_PREPUNCT_REX = ^([ .!?:,]*)(.*)   (build.js:43)
    Regex::new("^([ .!?:,]*)([^\n\r\u{2028}\u{2029}]*)").unwrap_or_else(|_| never())
});

/// The shared body of the `@showid/true` decorators: split off leading and
/// trailing punctuation and hand the middle to `sys.variableWrapper`.
fn showid_wrap(state: &State, blob: Option<&Blob>, s: &str) -> CslResult<String> {
    let mut s = s.to_string();
    let mut pre_punct = String::new();
    if !s.is_empty() {
        // `m` always matches; (.*) stops at a line terminator.
        if let Some(m) = RE_PREPUNCT.captures(&s) {
            pre_punct = m.get(1).map(|x| x.as_str()).unwrap_or("").to_string();
            let rest = m.get(2).map(|x| x.as_str()).unwrap_or("").to_string();
            s = rest;
        }
    }
    let mut post_punct = String::new();
    if !s.is_empty() {
        let last = super::obj_blob::last_char(&s).to_string();
        if SWAPPING_PUNCTUATION.contains(&last.as_str()) {
            s = super::obj_blob::drop_last(&s).to_string();
            post_punct = last;
        }
    }
    variable_wrapper(
        state,
        blob_extra(blob, "params"),
        &pre_punct,
        &s,
        &post_punct,
    )
}

/// `state.sys.variableWrapper(params, prePunct, str, postPunct)` as the
/// citeproc-js test runner defines it (test_runner.js:223). `params` is
/// `{variableNames: [...], itemData: {...}, context, position}`.
fn variable_wrapper(
    state: &State,
    params: Option<&Value>,
    pre: &str,
    s: &str,
    post: &str,
) -> CslResult<String> {
    if !state.fun.host_hooks.variable_wrapper {
        return Err(EngineError::Csl(
            "state.sys.variableWrapper is not a function".into(),
        ));
    }
    let p =
        params.ok_or_else(|| EngineError::Csl("variableWrapper called without params".into()))?;
    let var0 = p
        .get("variableNames")
        .and_then(|v| v.get(0))
        .and_then(Value::as_str)
        .unwrap_or("");
    let context_citation = p.get("context").and_then(Value::as_str) == Some("citation");
    let position_first = p.get("position").and_then(Value::as_str) == Some("first");
    let url = p
        .get("itemData")
        .and_then(|d| d.get("URL"))
        .filter(|u| js::truthy(u));
    if var0 == "title" && url.is_some() && context_citation && position_first {
        let u = url.map(js::to_js_string).unwrap_or_default();
        Ok(format!("{pre}<a href=\"{u}\">{s}</a>{post}"))
    } else if var0 == "first-reference-note-number" && context_citation && !position_first {
        Ok(format!("{pre}<b>{s}</b>{post}"))
    } else {
        Ok(format!("{pre}{s}{post}"))
    }
}

fn sys_missing(what: &str) -> EngineError {
    EngineError::Csl(format!("state.sys.{what} is not a function"))
}

fn wrap_citation_entry(state: &State) -> CslResult<String> {
    if state.fun.host_hooks.wrap_citation_entry {
        // PORT-LATER(sys.wrapCitationEntry): host callback (formats.js @cite/entry), needs the host's closure; api_cite agent
        Err(EngineError::NotYetPorted {
            method: "sys.wrapCitationEntry",
        })
    } else {
        Err(sys_missing("wrapCitationEntry"))
    }
}

/// `state.sys.embedBibliographyEntry(id) + "\n"` when the host defines it,
/// else `""` (the html/fo `@bibliography/entry` test `if (state.sys.embedBibliographyEntry)`).
fn embed_bibliography_entry(state: &State) -> CslResult<String> {
    if state.fun.host_hooks.embed_bibliography_entry {
        // PORT-LATER(sys.embedBibliographyEntry): host callback (formats.js @bibliography/entry); api_bibliography agent
        Err(EngineError::NotYetPorted {
            method: "sys.embedBibliographyEntry",
        })
    } else {
        Ok(String::new())
    }
}

fn doi_url(s: &str) -> String {
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://doi.org/{s}")
    }
}

fn run_func(
    format: Format,
    f: Func,
    state: &State,
    blob: Option<&Blob>,
    s: Option<&str>,
    extra: Option<&str>,
) -> CslResult<String> {
    // The decorators that take `str` as a JS string; undefined behaves as ""
    // for concatenation only where JS would print "undefined".
    if f == Func::Doi
        && s.is_none()
        && matches!(format, Format::Html | Format::Asciidoc | Format::Fo)
    {
        // `str.match(...)` on undefined
        return Err(EngineError::Csl(
            "@DOI/true: str is undefined (TypeError)".into(),
        ));
    }
    let st = s.unwrap_or("undefined");
    // For decorators that return `str` itself: JS `undefined` stays undefined
    // (here the empty string).
    let pass = s.unwrap_or("");
    Ok(match (format, f) {
        // ---- @quotes ----
        (Format::Asciidoc, Func::QuotesTrue) => match s {
            None => "``".to_string(),
            Some(x) => format!("``{x}''"),
        },
        (Format::Asciidoc, Func::QuotesInner) => match s {
            None => "`".to_string(),
            Some(x) => format!("`{x}'"),
        },
        (Format::Rtf, Func::QuotesTrue) => {
            let o = text_escape(Format::Rtf, &get_term(state, "open-quote"));
            match s {
                None => o,
                Some(x) => format!("{o}{x}{}", text_escape(Format::Rtf, &get_term(state, "close-quote"))),
            }
        }
        (Format::Rtf, Func::QuotesInner) => match s {
            None => text_escape(Format::Rtf, "\u{2019}"),
            Some(x) => format!(
                "{}{x}{}",
                text_escape(Format::Rtf, &get_term(state, "open-inner-quote")),
                text_escape(Format::Rtf, &get_term(state, "close-inner-quote"))
            ),
        },
        (_, Func::QuotesTrue) => match s {
            None => get_term(state, "open-quote"),
            Some(x) => format!(
                "{}{x}{}",
                get_term(state, "open-quote"),
                get_term(state, "close-quote")
            ),
        },
        (_, Func::QuotesInner) => match s {
            // Mostly right by being wrong (for apostrophes)
            None => "\u{2019}".to_string(),
            Some(x) => format!(
                "{}{x}{}",
                get_term(state, "open-inner-quote"),
                get_term(state, "close-inner-quote")
            ),
        },
        // ---- @cite/entry ----
        (_, Func::CiteEntry) => return wrap_citation_entry(state),
        // ---- @bibliography/entry ----
        (Format::Html, Func::BibEntry) => {
            let insert = embed_bibliography_entry(state)?;
            format!("  <div class=\"csl-entry\">{st}</div>\n{insert}")
        }
        (Format::Text, Func::BibEntry) | (Format::Asciidoc, Func::BibEntry) => format!("{st}\n"),
        (Format::Rtf, Func::BibEntry) => pass.to_string(),
        (Format::Fo, Func::BibEntry) => {
            let mut indent = String::new();
            if let Some(hi) = state
                .bibliography
                .opt
                .get("hangingindent")
                .filter(|v| js::truthy(v))
            {
                let hi = js::to_js_string(hi);
                indent = format!(" start-indent=\"{hi}em\" text-indent=\"-{hi}em\"");
            }
            let insert = embed_bibliography_entry(state)?;
            format!(
                "<fo:block id=\"{}\"{indent}>{st}</fo:block>\n{insert}",
                js_prop_string(blob, "system_id")
            )
        }
        (Format::Latex, Func::BibEntry) => {
            // "\\bibitem{" + state.sys.embedBibliographyEntry(this.item_id) + "}\n"
            if state.fun.host_hooks.embed_bibliography_entry {
                // PORT-LATER(sys.embedBibliographyEntry): host callback (formats.js latex @bibliography/entry)
                return Err(EngineError::NotYetPorted { method: "sys.embedBibliographyEntry" });
            }
            return Err(sys_missing("embedBibliographyEntry"));
        }
        // ---- @display ----
        (Format::Html, Func::DisplayBlock) => format!("\n\n    <div class=\"csl-block\">{st}</div>\n"),
        (Format::Html, Func::DisplayLeftMargin) => {
            format!("\n    <div class=\"csl-left-margin\">{st}</div>")
        }
        (Format::Html, Func::DisplayRightInline) => {
            format!("<div class=\"csl-right-inline\">{st}</div>\n  ")
        }
        (Format::Html, Func::DisplayIndent) => format!("<div class=\"csl-indent\">{st}</div>\n  "),
        (Format::Text, Func::DisplayBlock) | (Format::Latex, Func::DisplayBlock) => {
            format!("\n{st}")
        }
        (Format::Text, Func::DisplayLeftMargin) => format!("{st} "),
        (Format::Text, Func::DisplayRightInline) => pass.to_string(),
        (Format::Text, Func::DisplayIndent) | (Format::Latex, Func::DisplayIndent) => {
            format!("\n    {st}")
        }
        (Format::Rtf, Func::DisplayBlock) => format!("\\line{{}}{st}\\line\r\n"),
        (Format::Rtf, Func::DisplayLeftMargin) => format!("{st}\\tab "),
        (Format::Rtf, Func::DisplayRightInline) => format!("{st}\r\n"),
        (Format::Rtf, Func::DisplayIndent) => format!("\n\\tab {st}\\line\r\n"),
        (Format::Asciidoc, Func::DisplayBlock) | (Format::Asciidoc, Func::DisplayLeftMargin) => {
            pass.to_string()
        }
        (Format::Asciidoc, Func::DisplayRightInline) | (Format::Asciidoc, Func::DisplayIndent) => {
            format!(" {st}")
        }
        (Format::Fo, Func::DisplayBlock) => format!("\n  <fo:block>{st}</fo:block>\n"),
        (Format::Fo, Func::DisplayLeftMargin) => format!(
            "\n  <fo:table table-layout=\"fixed\" width=\"100%\">\n    \
             <fo:table-column column-number=\"1\" column-width=\"$$$__COLUMN_WIDTH_1__$$$\"/>\n    \
             <fo:table-column column-number=\"2\" column-width=\"proportional-column-width(1)\"/>\n    \
             <fo:table-body>\n      \
             <fo:table-row>\n        \
             <fo:table-cell>\n          \
             <fo:block>{st}</fo:block>\n        \
             </fo:table-cell>\n        "
        ),
        (Format::Fo, Func::DisplayRightInline) => format!(
            "<fo:table-cell>\n          \
             <fo:block>{st}</fo:block>\n        \
             </fo:table-cell>\n      \
             </fo:table-row>\n    \
             </fo:table-body>\n  \
             </fo:table>\n"
        ),
        (Format::Fo, Func::DisplayIndent) => {
            format!("<fo:block margin-left=\"2em\">{st}</fo:block>\n")
        }
        (Format::Latex, Func::DisplayLeftMargin) | (Format::Latex, Func::DisplayRightInline) => {
            pass.to_string()
        }
        // Func::DisplayBlock for Rtf handled as a template in `entry`; keep
        // the arm above for completeness.
        // ---- @showid ----
        (Format::Html, Func::ShowId) => {
            if !state.tmp.just_looking && !state.tmp.suppress_decorations {
                if let Some(cslid) = extra.filter(|c| !c.is_empty()) {
                    let nodename = state
                        .opt
                        .get("nodenames")
                        .and_then(|n| match n {
                            Value::Array(a) => cslid.parse::<usize>().ok().and_then(|i| a.get(i)),
                            Value::Object(o) => o.get(cslid),
                            _ => None,
                        })
                        .map(js::to_js_string)
                        .unwrap_or_else(|| "undefined".to_string());
                    format!("<span class=\"{nodename}\" cslid=\"{cslid}\">{st}</span>")
                } else if blob_extra(blob, "params").map(js::truthy).unwrap_or(false) && s.is_some() {
                    showid_wrap(state, blob, st)?
                } else {
                    pass.to_string()
                }
            } else {
                pass.to_string()
            }
        }
        (Format::Rtf, Func::ShowId) => {
            if !state.tmp.just_looking && !state.tmp.suppress_decorations {
                showid_wrap(state, blob, st)?
            } else {
                pass.to_string()
            }
        }
        (Format::Asciidoc, Func::ShowId) | (Format::Fo, Func::ShowId) => {
            if !state.tmp.just_looking
                && !state.tmp.suppress_decorations
                && blob_extra(blob, "params").map(js::truthy).unwrap_or(false)
                && s.is_some()
            {
                showid_wrap(state, blob, st)?
            } else {
                pass.to_string()
            }
        }
        (Format::Text, Func::ShowId) | (Format::Latex, Func::ShowId) => pass.to_string(),
        // ---- @URL / @DOI ----
        (Format::Html, Func::Url) => format!("<a href=\"{st}\">{st}</a>"),
        (Format::Fo, Func::Url) => {
            format!("<fo:basic-link external-destination=\"url('{st}')\">{st}</fo:basic-link>")
        }
        (_, Func::Url) => pass.to_string(),
        (Format::Html, Func::Doi) => format!("<a href=\"{}\">{st}</a>", doi_url(st)),
        (Format::Asciidoc, Func::Doi) => format!("{}[{st}]", doi_url(st)),
        (Format::Fo, Func::Doi) => format!(
            "<fo:basic-link external-destination=\"url('{}')\">{st}</fo:basic-link>",
            doi_url(st)
        ),
        (_, Func::Doi) => pass.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Reference output of citeproc-js 2.4.63's `CSL.Output.Formats`
    /// (text escapers, bibstart/bibend, every decorator of every format on
    /// edge strings, `getSafeEscape`), generated by
    /// `scripts/csl-units/formats.cjs`.
    const DATA: &str = include_str!("../../tests/data/csl/units/formats.json");

    fn state_for(mode: &str, hook: bool) -> State {
        let mut st = State::default();
        crate::citeproc::test_support::install_output_locale(&mut st, false);
        set_output_format(&mut st, mode).unwrap();
        st.tmp.area = "citation".to_string();
        st.opt.insert(
            "nodenames".into(),
            serde_json::json!(["zero", "one", "two"]),
        );
        st.bibliography
            .opt
            .insert("hangingindent".into(), serde_json::json!(2));
        st.fun.host_hooks.variable_wrapper = hook;
        st
    }

    #[test]
    fn escapes_and_boundaries_match_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let mut bad = Vec::new();
        let mut n = 0;
        for (mode, m) in v["modes"].as_object().unwrap() {
            let f = Format::from_mode(mode).unwrap();
            for pair in m["escape"].as_array().unwrap() {
                n += 1;
                let (input, want) = (pair[0].as_str().unwrap(), &pair[1]);
                let got = text_escape(f, input);
                if want.as_str() != Some(got.as_str()) {
                    bad.push(format!("{mode} escape {input:?}: got {got:?}, want {want}"));
                }
            }
            if m["bibstart"].as_str() != Some(f.bibstart()) {
                bad.push(format!("{mode} bibstart"));
            }
            if m["bibend"].as_str() != Some(f.bibend()) {
                bad.push(format!("{mode} bibend"));
            }
        }
        assert!(n > 150);
        assert!(
            bad.is_empty(),
            "{} mismatches:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }

    #[test]
    fn decorators_match_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let mut bad = Vec::new();
        let mut n = 0;
        for (mode, m) in v["modes"].as_object().unwrap() {
            for c in m["deco"].as_array().unwrap() {
                n += 1;
                let hook = c["hook"].as_bool().unwrap();
                let st = state_for(mode, hook);
                let key = c["k"].as_str().unwrap();
                let (name, value) = key.split_once('/').unwrap();
                let mut blob = Blob::default();
                if let Some(th) = c["th"].as_object() {
                    for (k, val) in th {
                        blob.extra.insert(k.clone(), val.clone());
                    }
                }
                let got = decorate(
                    &st,
                    Some(&blob),
                    name,
                    value,
                    c["s"].as_str(),
                    c["x"].as_str(),
                );
                let want = &c["r"];
                let ok = match (&got, want) {
                    (Ok(g), w) if w.is_string() => w.as_str() == Some(g.as_str()),
                    (Ok(g), w) if w.get("undef").is_some() => g.is_empty(),
                    (Err(_), w) => w.get("error").is_some(),
                    _ => false,
                };
                if !ok {
                    bad.push(format!(
                        "{mode} {key} s={} x={} th={} hook={hook}: got {got:?}, want {want}",
                        c["s"], c["x"], c["th"]
                    ));
                }
            }
        }
        assert!(n > 6000);
        assert!(
            bad.is_empty(),
            "{} mismatches, first 20:\n{}",
            bad.len(),
            bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
        );
    }

    #[test]
    fn safe_escape_matches_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let mut bad = Vec::new();
        for c in v["safe"].as_array().unwrap() {
            let mut st = State::default();
            set_output_format(&mut st, c["mode"].as_str().unwrap()).unwrap();
            st.tmp.area = c["area"].as_str().unwrap().to_string();
            st.opt.insert(
                "development_extensions".into(),
                serde_json::json!({ "thin_non_breaking_space_html_hack": c["thin"] }),
            );
            let got = crate::citeproc::load::get_safe_escape(&st).escape(c["s"].as_str().unwrap());
            if c["r"].as_str() != Some(got.as_str()) {
                bad.push(format!("{c}: got {got:?}"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn replace_patterns_follow_js() {
        assert_eq!(
            js_replace_first("<i>%%STRING%%</i>", "%%STRING%%", "a$$b"),
            "<i>a$b</i>"
        );
        assert_eq!(
            js_replace_first("x%%STRING%%y", "%%STRING%%", "[$&]"),
            "x[%%STRING%%]y"
        );
        assert_eq!(js_replace_first("x%%STRING%%y", "%%STRING%%", "$1"), "x$1y");
        assert_eq!(
            js_replace_first("x%%STRING%%y", "%%STRING%%", "[$`|$']"),
            "x[x|y]y"
        );
    }

    #[test]
    fn html_escape_basics() {
        assert_eq!(text_escape(Format::Html, "a & b < c"), "a &#38; b &#60; c");
        assert_eq!(text_escape(Format::Html, "a  b"), "a\u{a0} b");
        assert_eq!(text_escape(Format::Html, "m\u{b2}"), "m<sup>2</sup>");
    }

    #[test]
    fn mode_names_round_trip() {
        for f in [
            Format::Html,
            Format::Text,
            Format::Rtf,
            Format::Asciidoc,
            Format::Fo,
            Format::Latex,
        ] {
            assert_eq!(Format::from_mode(f.mode_name()), Some(f));
        }
        assert_eq!(Format::from_mode("nope"), None);
    }
}
