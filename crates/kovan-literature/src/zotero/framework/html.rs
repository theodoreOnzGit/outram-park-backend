// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, identical in the translation-server's 1dd38e27edf8):
//   utilities.js `unescapeHTML` :699-722, whose Node branch returns
//   `new JSDOM(str).window.document.documentElement.textContent` with runs
//   of spaces collapsed. What that computes is defined by the WHATWG HTML
//   parsing algorithm (tokenizer §13.2.5, tree construction §13.2.6), which
//   this file follows for the parts that decide the text.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `Zotero.Utilities.unescapeHTML`: the plain text of an HTML fragment, as
//! the translation-server computes it (jsdom's HTML parser, then
//! `textContent`).
//!
//! Modelled, because they change the text: input newline normalisation
//! (CR LF and CR become LF); tags, comments, doctypes and bogus comments
//! (`<!...>`, `<?...>`, `</ ...>`) removed, including `>` inside quoted
//! attribute values; a `<` that does not start a tag kept as text; every
//! named character reference (the full WHATWG table, legacy forms without
//! `;` included, longest match) and numeric ones (with the C1 and invalid
//! code point replacements); RCDATA (`title`, `textarea`, entities decoded)
//! and raw text (`style`, `script`, `xmp`, `iframe`, `noembed`, `noframes`,
//! `plaintext`) elements, whose content is text; whitespace dropped at the
//! start of the document (the initial, "before html" and "before head"
//! insertion modes ignore it); the newline dropped after `<pre>`,
//! `<listing>` and `<textarea>`; NUL dropped in data and replaced in raw
//! text; the content of `<template>` (not part of `textContent`).
//!
//! Not modelled (documented divergences, none seen in the reference
//! fixtures): foster parenting, which moves text that sits directly inside
//! a `<table>` before the table; the script-data escape states
//! (`<!--` inside `<script>`); a byte-order mark at the very start.

use super::html_entities::{MAX_NAME_LEN, NAMED};

/// `Zotero.Utilities.unescapeHTML(str)`.
pub fn unescape_html(s: &str) -> String {
    // "If no tags, no need to unescape" (:704).
    if !s.contains('<') && !s.contains('&') {
        return s.to_owned();
    }
    collapse_spaces(&text_content(s))
}

/// `.replace(/ {2,}/g, " ")`.
fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// The tokenizer's content model after a start tag.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    /// RCDATA: text and character references until the end tag.
    RcData,
    /// RAWTEXT / script data: text until the end tag.
    RawText,
    /// PLAINTEXT: text to the end of input.
    PlainText,
}

struct Parser {
    c: Vec<char>,
    i: usize,
    out: String,
    /// In the initial / before-html / before-head insertion modes, where
    /// whitespace characters are ignored.
    before_head: bool,
    /// Inside `<template>` (depth): text there is not in `textContent`.
    template_depth: usize,
    /// Drop one LF if it comes next (after `<pre>`, `<listing>`,
    /// `<textarea>`).
    skip_lf: bool,
}

fn is_tag_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{0C}' | ' ')
}

/// The numeric character reference replacements for 0x80-0x9F (§13.2.5.80).
fn c1_replacement(n: u32) -> Option<char> {
    Some(match n {
        0x80 => '\u{20AC}',
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}',
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        _ => return None,
    })
}

fn lookup_named(name: &str) -> Option<&'static str> {
    NAMED
        .binary_search_by(|(k, _)| k.as_bytes().cmp(name.as_bytes()))
        .ok()
        .map(|i| NAMED[i].1)
}

impl Parser {
    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }

    /// A character token reaching the tree builder.
    fn emit(&mut self, ch: char) {
        if self.skip_lf {
            self.skip_lf = false;
            if ch == '\n' {
                return;
            }
        }
        if self.before_head {
            if matches!(ch, '\t' | '\n' | '\u{0C}' | '\r' | ' ') {
                return;
            }
            self.before_head = false;
        }
        if self.template_depth > 0 {
            return;
        }
        self.out.push(ch);
    }

    fn emit_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.emit(ch);
        }
    }

    /// After `&` (already consumed): a character reference (§13.2.5.72-80).
    fn char_ref(&mut self) {
        match self.peek(0) {
            Some(c) if c.is_ascii_alphanumeric() => {
                let max = MAX_NAME_LEN.min(self.c.len() - self.i);
                for k in (1..=max).rev() {
                    let cand: String = self.c[self.i..self.i + k].iter().collect();
                    if let Some(rep) = lookup_named(&cand) {
                        self.i += k;
                        self.emit_str(rep);
                        return;
                    }
                }
                // No match: the ampersand and what follows are text.
                self.emit('&');
            }
            Some('#') => {
                let hex = matches!(self.peek(1), Some('x') | Some('X'));
                let start = self.i + if hex { 2 } else { 1 };
                let mut j = start;
                let mut value: u32 = 0;
                while let Some(&d) = self.c.get(j) {
                    let dv = if hex { d.to_digit(16) } else { d.to_digit(10) };
                    match dv {
                        Some(v) if d.is_ascii() => {
                            value = value
                                .saturating_mul(if hex { 16 } else { 10 })
                                .saturating_add(v);
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if j == start {
                    // No digits: "&#" / "&#x" are text.
                    self.emit('&');
                    return;
                }
                self.i = j;
                if self.peek(0) == Some(';') {
                    self.i += 1;
                }
                let ch = if value == 0 || value > 0x10FFFF || (0xD800..=0xDFFF).contains(&value) {
                    '\u{FFFD}'
                } else if let Some(r) = c1_replacement(value) {
                    r
                } else {
                    char::from_u32(value).unwrap_or('\u{FFFD}')
                };
                self.emit(ch);
            }
            _ => self.emit('&'),
        }
    }

    /// Skip to just after the next `>` (bogus comment, doctype).
    fn skip_past_gt(&mut self) {
        while let Some(c) = self.peek(0) {
            self.i += 1;
            if c == '>' {
                return;
            }
        }
    }

    /// After `<!--`: a comment (§13.2.5.43-52), ending at `-->`, `--!>`, or
    /// `>` right at its start (`<!-->`, `<!--->`).
    fn comment(&mut self) {
        if self.peek(0) == Some('>') {
            self.i += 1;
            return;
        }
        if self.peek(0) == Some('-') && self.peek(1) == Some('>') {
            self.i += 2;
            return;
        }
        while self.i < self.c.len() {
            if self.peek(0) == Some('-') && self.peek(1) == Some('-') {
                if self.peek(2) == Some('>') {
                    self.i += 3;
                    return;
                }
                if self.peek(2) == Some('!') && self.peek(3) == Some('>') {
                    self.i += 4;
                    return;
                }
            }
            self.i += 1;
        }
    }

    /// Parse a tag's name and attributes from the current position (just
    /// after `<` or `</`). Returns the lower-cased name, or `None` when the
    /// input ends inside the tag (the tag is then dropped).
    fn tag(&mut self) -> Option<String> {
        let mut name = String::new();
        while let Some(c) = self.peek(0) {
            if is_tag_ws(c) || c == '/' || c == '>' {
                break;
            }
            name.push(if c == '\0' {
                '\u{FFFD}'
            } else {
                c.to_ascii_lowercase()
            });
            self.i += 1;
        }
        // Attributes.
        loop {
            let c = self.peek(0)?;
            if is_tag_ws(c) || c == '/' {
                self.i += 1;
                continue;
            }
            if c == '>' {
                self.i += 1;
                return Some(name);
            }
            // Attribute name (the first character may be '=').
            self.i += 1;
            while let Some(c) = self.peek(0) {
                if is_tag_ws(c) || c == '/' || c == '>' || c == '=' {
                    break;
                }
                self.i += 1;
            }
            while self.peek(0).is_some_and(is_tag_ws) {
                self.i += 1;
            }
            if self.peek(0) == Some('=') {
                self.i += 1;
                while self.peek(0).is_some_and(is_tag_ws) {
                    self.i += 1;
                }
                match self.peek(0)? {
                    q @ ('"' | '\'') => {
                        self.i += 1;
                        loop {
                            let c = self.peek(0)?;
                            self.i += 1;
                            if c == q {
                                break;
                            }
                        }
                    }
                    '>' => {
                        self.i += 1;
                        return Some(name);
                    }
                    _ => {
                        while let Some(c) = self.peek(0) {
                            if is_tag_ws(c) || c == '>' {
                                break;
                            }
                            self.i += 1;
                        }
                    }
                }
            }
        }
    }

    /// Text of an RCDATA / raw-text element up to its end tag, which is
    /// consumed.
    fn raw_until_end_tag(&mut self, name: &str, content: Content) {
        let n: Vec<char> = name.chars().collect();
        while let Some(c) = self.peek(0) {
            if c == '<' && self.peek(1) == Some('/') && content != Content::PlainText {
                let ok = n.iter().enumerate().all(|(k, &nc)| {
                    self.peek(2 + k)
                        .is_some_and(|x| x.to_ascii_lowercase() == nc)
                });
                let after = self.peek(2 + n.len());
                if ok && after.is_some_and(|a| is_tag_ws(a) || a == '/' || a == '>') {
                    self.i += 2;
                    let _ = self.tag();
                    return;
                }
            }
            self.i += 1;
            match c {
                '&' if content == Content::RcData => self.char_ref(),
                '\0' => self.emit('\u{FFFD}'),
                other => self.emit(other),
            }
        }
    }

    fn start_tag(&mut self, name: &str) {
        if self.before_head {
            // Any start tag but <html> ends the modes that drop whitespace
            // (<head> enters "in head", which keeps it; anything else
            // implies <head> and is processed from there).
            if name != "html" {
                self.before_head = false;
            }
        }
        match name {
            "template" => self.template_depth += 1,
            "pre" | "listing" => self.skip_lf = true,
            "textarea" => {
                self.skip_lf = true;
                self.raw_until_end_tag(name, Content::RcData);
            }
            "title" => self.raw_until_end_tag(name, Content::RcData),
            "style" | "xmp" | "iframe" | "noembed" | "noframes" | "script" => {
                self.raw_until_end_tag(name, Content::RawText)
            }
            "plaintext" => self.raw_until_end_tag(name, Content::PlainText),
            _ => {}
        }
    }

    fn end_tag(&mut self, name: &str) {
        if self.before_head && matches!(name, "head" | "body" | "html" | "br") {
            self.before_head = false;
        }
        if name == "template" && self.template_depth > 0 {
            self.template_depth -= 1;
        }
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            self.i += 1;
            match c {
                '&' => self.char_ref(),
                '\0' => {} // ignored in the "in body" insertion mode
                '<' => match self.peek(0) {
                    Some('!') => {
                        self.i += 1;
                        if self.peek(0) == Some('-') && self.peek(1) == Some('-') {
                            self.i += 2;
                            self.comment();
                        } else {
                            // DOCTYPE, CDATA outside foreign content, and
                            // anything else: up to the next '>'.
                            self.skip_past_gt();
                        }
                    }
                    Some('?') => self.skip_past_gt(),
                    Some('/') => match self.peek(1) {
                        Some(a) if a.is_ascii_alphabetic() => {
                            self.i += 1;
                            if let Some(name) = self.tag() {
                                self.end_tag(&name);
                            }
                        }
                        Some('>') => self.i += 2,
                        None => {
                            self.i += 1;
                            self.emit_str("</");
                        }
                        Some(_) => self.skip_past_gt(),
                    },
                    Some(a) if a.is_ascii_alphabetic() => {
                        if let Some(name) = self.tag() {
                            self.start_tag(&name);
                        }
                    }
                    _ => self.emit('<'),
                },
                other => self.emit(other),
            }
        }
    }
}

/// `documentElement.textContent` of `s` parsed as an HTML document.
pub fn text_content(s: &str) -> String {
    // Input stream preprocessing: CR LF -> LF, CR -> LF (§13.2.3.5).
    let mut c = Vec::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '\r' {
            if it.peek() == Some(&'\n') {
                it.next();
            }
            c.push('\n');
        } else {
            c.push(ch);
        }
    }
    let mut p = Parser {
        c,
        i: 0,
        out: String::new(),
        before_head: true,
        template_depth: 0,
        skip_lf: false,
    };
    p.run();
    p.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_tags_and_whitespace() {
        assert_eq!(unescape_html("a b"), "a b");
        assert_eq!(unescape_html("Smith &amp; Jones"), "Smith & Jones");
        assert_eq!(unescape_html("Smith & Jones"), "Smith & Jones");
        assert_eq!(
            unescape_html("&notit; &notin; &amp"),
            "\u{AC}it; \u{2209} &"
        );
        assert_eq!(unescape_html("<p>A <b>child</b>  note</p>"), "A child note");
        assert_eq!(unescape_html("  &lt;i&gt;x"), "<i>x");
        assert_eq!(unescape_html("<b>  x</b>"), "  x".replacen("  ", " ", 1));
        assert_eq!(unescape_html("a <3 b & c"), "a <3 b & c");
        assert_eq!(unescape_html("<a title='x>y'>t</a>"), "t");
        assert_eq!(unescape_html("&#x80;&#0;&#65"), "\u{20AC}\u{FFFD}A");
        assert_eq!(unescape_html("<title>a &amp; <b></title>"), "a & <b>");
        assert_eq!(unescape_html("x<!-- c -->y"), "xy");
        assert_eq!(unescape_html("a\r\n<br>b"), "a\nb");
    }
}
