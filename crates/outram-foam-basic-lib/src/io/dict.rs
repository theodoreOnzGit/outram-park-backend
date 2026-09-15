// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The `FoamFile` ASCII **dictionary** format: tokeniser, AST, parser, writer.
//!
//! An OpenFOAM dictionary file is a banner comment, an optional `FoamFile`
//! header block, and a body of `keyword value ;` / `keyword { subdict }`
//! entries. This module models that as:
//!
//! - [`FoamHeader`] — the `FoamFile { … }` block (an ordered flat
//!   keyword→raw-value map; values are kept verbatim so `version 2.0`
//!   round-trips as `2.0`, not `2`).
//! - [`FoamDict`] — an **ordered** keyword→[`FoamEntry`] map (ordered so
//!   writes preserve the input order and round-trip).
//! - [`FoamEntry`] — the value bound to one keyword: a scalar, word, quoted
//!   string, bare token sequence, parenthesised list, dimensioned value, or a
//!   nested sub-dictionary.
//! - [`FoamValue`] — a leaf inside a list / token sequence.
//! - [`Dimensioned`] — a `[0 2 -2 0 0 0 0] value` dimensioned quantity.
//!
//! ## Grammar notes
//!
//! The tokeniser records whether each token was **glued** to the previous one
//! (no intervening whitespace). This disambiguates a function-style word such
//! as `div(phi,U)` or `grad(U)` (parentheses glued to a word → part of the
//! word) from a genuine list `(0 0 1)` (parenthesis preceded by whitespace →
//! a [`FoamValue::List`]). It also lets `4(1 4 13 10)` count-prefixed lists
//! parse cleanly.

use std::fmt::Write as _;
use std::path::Path;

use super::IoError;

// ───────────────────────────────────────────────────────────────────────────
//  Tokeniser
// ───────────────────────────────────────────────────────────────────────────

/// One lexical token from an OpenFOAM dictionary.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    /// The token text. For a quoted string this is the inner content (quotes
    /// stripped) and `quoted` is set.
    pub text: String,
    /// True if no whitespace separated this token from the previous one.
    pub glued: bool,
    /// True if this token came from a `"…"` quoted string literal.
    pub quoted: bool,
}

/// Split dictionary `text` into [`Token`]s, stripping `//` line comments and
/// `/* … */` block comments. The delimiters `( ) { } ; [ ]` are always their
/// own single-character tokens.
pub(crate) fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut tokens: Vec<Token> = Vec::new();
    // `pending_ws` is true when whitespace (or a comment) has been seen since
    // the last emitted token — i.e. the next token is NOT glued.
    let mut pending_ws = true;
    let mut word = String::new();
    let mut word_glued = true;

    let is_delim = |c: u8| matches!(c, b'(' | b')' | b'{' | b'}' | b';' | b'[' | b']');

    macro_rules! flush_word {
        () => {
            if !word.is_empty() {
                tokens.push(Token {
                    text: std::mem::take(&mut word),
                    glued: word_glued,
                    quoted: false,
                });
            }
        };
    }

    while i < n {
        let c = bytes[i];
        if i + 1 < n && c == b'/' && bytes[i + 1] == b'/' {
            flush_word!();
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            pending_ws = true;
        } else if i + 1 < n && c == b'/' && bytes[i + 1] == b'*' {
            flush_word!();
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            pending_ws = true;
        } else if c == b'"' {
            flush_word!();
            i += 1;
            let mut s = String::new();
            while i < n && bytes[i] != b'"' {
                s.push(bytes[i] as char);
                i += 1;
            }
            i += 1; // closing quote
            tokens.push(Token {
                text: s,
                glued: !pending_ws,
                quoted: true,
            });
            pending_ws = false;
        } else if is_delim(c) {
            flush_word!();
            tokens.push(Token {
                text: (c as char).to_string(),
                glued: !pending_ws,
                quoted: false,
            });
            pending_ws = false;
            i += 1;
        } else if (c as char).is_whitespace() {
            flush_word!();
            pending_ws = true;
            i += 1;
        } else {
            if word.is_empty() {
                word_glued = !pending_ws;
            }
            word.push(c as char);
            pending_ws = false;
            i += 1;
        }
    }
    flush_word!();
    tokens
}

// ───────────────────────────────────────────────────────────────────────────
//  AST
// ───────────────────────────────────────────────────────────────────────────

/// A dimensioned quantity: seven SI dimension exponents plus zero or more
/// numeric components.
///
/// OpenFOAM writes these as `[mass length time temperature moles current
/// luminous]` optionally followed by a value, e.g. `dimensions [0 2 -2 0 0 0
/// 0];` (no value) or `nu [0 2 -1 0 0 0 0] 1e-05;` (scalar value). The value
/// components are stored as raw [`FoamValue`]s to support scalar and vector
/// forms.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimensioned {
    /// The seven SI dimension exponents, OpenFOAM order:
    /// `[kg, m, s, K, mol, A, cd]`.
    pub dims: [f64; 7],
    /// Trailing value component(s); empty for a bare `dimensions […]` entry,
    /// one element for a dimensioned scalar, three for a dimensioned vector.
    pub value: Vec<FoamValue>,
}

/// A leaf value inside a list or a bare token sequence.
#[derive(Debug, Clone, PartialEq)]
pub enum FoamValue {
    /// A numeric scalar.
    Scalar(f64),
    /// A bare identifier / keyword-like word (e.g. `ascii`, `Gauss`, `PCG`,
    /// or a function-style `grad(U)`).
    Word(String),
    /// A `"…"` quoted string (stored without the surrounding quotes).
    Str(String),
    /// A parenthesised `( … )` list of values (may nest).
    List(Vec<FoamValue>),
}

impl FoamValue {
    /// Interpret a `List` of exactly three scalars as a [`Vector3`]; returns
    /// `None` for any other shape.
    ///
    /// [`Vector3`]: crate::primitives::Vector3
    pub fn as_vector3(&self) -> Option<crate::primitives::Vector3> {
        if let FoamValue::List(v) = self {
            if let [FoamValue::Scalar(x), FoamValue::Scalar(y), FoamValue::Scalar(z)] = v.as_slice()
            {
                return Some(crate::primitives::Vector3::new(*x, *y, *z));
            }
        }
        None
    }

    /// The scalar value, if this is a [`FoamValue::Scalar`].
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            FoamValue::Scalar(x) => Some(*x),
            _ => None,
        }
    }

    /// The word text, if this is a [`FoamValue::Word`].
    pub fn as_word(&self) -> Option<&str> {
        match self {
            FoamValue::Word(w) => Some(w.as_str()),
            _ => None,
        }
    }
}

/// The value bound to one keyword in a [`FoamDict`].
#[derive(Debug, Clone, PartialEq)]
pub enum FoamEntry {
    /// A single numeric scalar: `startTime 0;`.
    Scalar(f64),
    /// A single word: `application icoFoam;` (or function-style `default
    /// Gauss;`).
    Word(String),
    /// A single quoted string.
    Str(String),
    /// A bare, space-separated multi-token value that is **not** parenthesised:
    /// `div(phi,U) Gauss linearUpwind grad(U);` → the keyword is `div(phi,U)`
    /// and the entry is `Tokens([Gauss, linearUpwind, grad(U)])`.
    Tokens(Vec<FoamValue>),
    /// A single parenthesised list: `( … )`.
    List(Vec<FoamValue>),
    /// A dimensioned value: `[0 2 -2 0 0 0 0] …`.
    Dimensioned(Dimensioned),
    /// A nested sub-dictionary: `keyword { … }`.
    SubDict(FoamDict),
}

/// An **ordered** keyword → [`FoamEntry`] map — the body of a dictionary or a
/// sub-dictionary.
///
/// Insertion order is preserved so that serialising then parsing round-trips
/// byte-order of the entries. Lookups are linear (dictionaries are small).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FoamDict {
    entries: Vec<(String, FoamEntry)>,
}

impl FoamDict {
    /// A new, empty dictionary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `keyword → entry`. If the keyword already exists, the new entry
    /// replaces the old one **in place** (order unchanged); otherwise it is
    /// pushed to the end.
    pub fn insert(&mut self, keyword: impl Into<String>, entry: FoamEntry) -> &mut Self {
        let keyword = keyword.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == keyword) {
            slot.1 = entry;
        } else {
            self.entries.push((keyword, entry));
        }
        self
    }

    /// Borrow the entry bound to `keyword`, if present.
    pub fn get(&self, keyword: &str) -> Option<&FoamEntry> {
        self.entries
            .iter()
            .find(|(k, _)| k == keyword)
            .map(|(_, v)| v)
    }

    /// Borrow the sub-dictionary bound to `keyword`, if the entry is one.
    pub fn get_dict(&self, keyword: &str) -> Option<&FoamDict> {
        match self.get(keyword) {
            Some(FoamEntry::SubDict(d)) => Some(d),
            _ => None,
        }
    }

    /// The scalar bound to `keyword`, if the entry is a scalar.
    pub fn get_scalar(&self, keyword: &str) -> Option<f64> {
        match self.get(keyword) {
            Some(FoamEntry::Scalar(x)) => Some(*x),
            _ => None,
        }
    }

    /// The word bound to `keyword`, if the entry is a word.
    pub fn get_word(&self, keyword: &str) -> Option<&str> {
        match self.get(keyword) {
            Some(FoamEntry::Word(w)) => Some(w.as_str()),
            _ => None,
        }
    }

    /// Iterate over `(keyword, entry)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &FoamEntry)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of top-level entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the dictionary has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Header
// ───────────────────────────────────────────────────────────────────────────

/// The `FoamFile { … }` header block — an ordered flat keyword → raw-value map.
///
/// Values are stored verbatim (quotes preserved on the values that had them)
/// so that e.g. `version 2.0;` round-trips as `2.0` rather than being reparsed
/// to `2`, and `location "constant/polyMesh";` keeps its quotes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FoamHeader {
    entries: Vec<(String, String)>,
}

impl FoamHeader {
    /// A new, empty header.
    pub fn new() -> Self {
        Self::default()
    }

    /// The standard header for a given `class` and `object`, with
    /// `version 2.0; format ascii;`.
    pub fn standard(class: &str, object: &str) -> Self {
        let mut h = Self::new();
        h.set("version", "2.0");
        h.set("format", "ascii");
        h.set("class", class);
        h.set("object", object);
        h
    }

    /// Like [`Self::standard`] but also records a `location "…"`.
    pub fn standard_with_location(class: &str, location: &str, object: &str) -> Self {
        let mut h = Self::new();
        h.set("version", "2.0");
        h.set("format", "ascii");
        h.set("class", class);
        h.set("location", &format!("\"{location}\""));
        h.set("object", object);
        h
    }

    /// Set `keyword → value` (replacing in place if present, else appending).
    pub fn set(&mut self, keyword: &str, value: &str) -> &mut Self {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| k == keyword) {
            slot.1 = value.to_string();
        } else {
            self.entries.push((keyword.to_string(), value.to_string()));
        }
        self
    }

    /// The raw value bound to `keyword` (quotes still present if it had them).
    pub fn get(&self, keyword: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == keyword)
            .map(|(_, v)| v.as_str())
    }

    /// The `class` field, if present.
    pub fn class(&self) -> Option<&str> {
        self.get("class")
    }

    /// The `object` field, if present.
    pub fn object(&self) -> Option<&str> {
        self.get("object")
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Parser
// ───────────────────────────────────────────────────────────────────────────

/// Forward-only token cursor with the dictionary-grammar parse routines.
///
/// Owns its tokens by value (no lifetime parameters, per the crate's design
/// rules).
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    context: String,
}

impl Parser {
    fn new(tokens: Vec<Token>, context: impl Into<String>) -> Self {
        Self {
            tokens,
            pos: 0,
            context: context.into(),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_text(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|t| t.text.as_str())
    }

    fn advance(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn err(&self, message: impl Into<String>) -> IoError {
        IoError::parse(self.context.clone(), message.into())
    }

    fn expect(&mut self, tok: &str) -> Result<(), IoError> {
        match self.advance() {
            Some(t) if t.text == tok => Ok(()),
            Some(t) => Err(self.err(format!("expected `{tok}`, found `{}`", t.text))),
            None => Err(self.err(format!("expected `{tok}`, found end of input"))),
        }
    }

    /// Read a keyword: a word, folding any glued trailing `(…)` group into it
    /// so `div(phi,U)` becomes a single keyword string.
    fn parse_keyword(&mut self) -> Result<String, IoError> {
        let first = self
            .advance()
            .ok_or_else(|| self.err("expected a keyword, found end of input"))?;
        if matches!(first.text.as_str(), "(" | ")" | "{" | "}" | ";" | "[" | "]") {
            return Err(self.err(format!("expected a keyword, found `{}`", first.text)));
        }
        let mut kw = first.text.clone();
        // Fold a glued `( … )` group (function-style keyword).
        while matches!(self.peek(), Some(t) if t.text == "(" && t.glued) {
            kw.push_str(&self.fold_paren_text());
        }
        Ok(kw)
    }

    /// Consume a `( … )` group and return its flattened text (no whitespace),
    /// used to reassemble a function-style word such as `grad(U)`.
    fn fold_paren_text(&mut self) -> String {
        let mut out = String::new();
        // consume "("
        self.advance();
        out.push('(');
        let mut depth = 1;
        while depth > 0 {
            let Some(t) = self.advance() else { break };
            match t.text.as_str() {
                "(" => {
                    depth += 1;
                    out.push('(');
                }
                ")" => {
                    depth -= 1;
                    out.push(')');
                }
                other => out.push_str(other),
            }
        }
        out
    }

    /// Parse the body of a dictionary up to (and consuming) a closing `}`, or
    /// to end-of-input at the top level.
    fn parse_dict_body(&mut self, top_level: bool) -> Result<FoamDict, IoError> {
        let mut dict = FoamDict::new();
        loop {
            match self.peek_text() {
                None => {
                    if top_level {
                        break;
                    }
                    return Err(self.err("unexpected end of input inside `{ … }`"));
                }
                Some("}") => {
                    if top_level {
                        return Err(self.err("unexpected `}` at top level"));
                    }
                    self.advance();
                    break;
                }
                Some(";") => {
                    self.advance(); // stray semicolon
                }
                _ => {
                    let (kw, entry) = self.parse_entry()?;
                    dict.insert(kw, entry);
                }
            }
        }
        Ok(dict)
    }

    /// Parse one `keyword … ;` (or `keyword { … }`) entry.
    fn parse_entry(&mut self) -> Result<(String, FoamEntry), IoError> {
        let keyword = self.parse_keyword()?;
        match self.peek_text() {
            Some("{") => {
                self.advance();
                let sub = self.parse_dict_body(false)?;
                // OpenFOAM allows an optional trailing ';' after a sub-dict.
                if self.peek_text() == Some(";") {
                    self.advance();
                }
                Ok((keyword, FoamEntry::SubDict(sub)))
            }
            Some("[") => {
                let dim = self.parse_dimensioned()?;
                self.expect(";")?;
                Ok((keyword, FoamEntry::Dimensioned(dim)))
            }
            _ => {
                let entry = self.parse_value_sequence()?;
                self.expect(";")?;
                Ok((keyword, entry))
            }
        }
    }

    /// Parse the seven-exponent `[ … ]` block plus any trailing value tokens.
    fn parse_dimensioned(&mut self) -> Result<Dimensioned, IoError> {
        self.expect("[")?;
        let mut dims = [0.0f64; 7];
        for (i, slot) in dims.iter_mut().enumerate() {
            let t = self
                .advance()
                .ok_or_else(|| self.err("unexpected end of input inside `[ … ]`"))?;
            *slot = t.text.parse::<f64>().map_err(|_| {
                self.err(format!(
                    "dimension exponent {i} is not a number: `{}`",
                    t.text
                ))
            })?;
        }
        self.expect("]")?;
        // Trailing values (scalar / vector) up to the terminating ';'.
        let mut value = Vec::new();
        while !matches!(self.peek_text(), Some(";") | None) {
            value.push(self.parse_leaf()?);
        }
        Ok(Dimensioned { dims, value })
    }

    /// Parse a bare value sequence terminated by `;` (not consumed) and reduce
    /// it to the most specific [`FoamEntry`] variant.
    fn parse_value_sequence(&mut self) -> Result<FoamEntry, IoError> {
        let mut values = Vec::new();
        while !matches!(self.peek_text(), Some(";") | None) {
            values.push(self.parse_leaf()?);
        }
        Ok(match values.len() {
            0 => return Err(self.err("empty entry value")),
            1 => match values.into_iter().next().unwrap() {
                FoamValue::Scalar(x) => FoamEntry::Scalar(x),
                FoamValue::Word(w) => FoamEntry::Word(w),
                FoamValue::Str(s) => FoamEntry::Str(s),
                FoamValue::List(l) => FoamEntry::List(l),
            },
            _ => FoamEntry::Tokens(values),
        })
    }

    /// Parse one leaf value: a list `( … )`, a quoted string, a number, or a
    /// (possibly function-style) word.
    fn parse_leaf(&mut self) -> Result<FoamValue, IoError> {
        let t = self
            .peek()
            .ok_or_else(|| self.err("expected a value, found end of input"))?;
        if t.text == "(" {
            return self.parse_list();
        }
        // consume the token
        let t = self.advance().unwrap();
        if t.quoted {
            return Ok(FoamValue::Str(t.text.clone()));
        }
        let text = t.text.clone();
        // Function-style word: fold glued trailing `( … )` groups.
        if self.peek().is_some_and(|p| p.text == "(" && p.glued) {
            let mut w = text;
            while matches!(self.peek(), Some(p) if p.text == "(" && p.glued) {
                w.push_str(&self.fold_paren_text());
            }
            return Ok(FoamValue::Word(w));
        }
        match text.parse::<f64>() {
            Ok(x) => Ok(FoamValue::Scalar(x)),
            Err(_) => Ok(FoamValue::Word(text)),
        }
    }

    /// Parse a `( … )` list, skipping any count prefix already handled by the
    /// caller.
    fn parse_list(&mut self) -> Result<FoamValue, IoError> {
        self.expect("(")?;
        let mut items = Vec::new();
        loop {
            match self.peek_text() {
                Some(")") => {
                    self.advance();
                    break;
                }
                None => return Err(self.err("unexpected end of input inside `( … )`")),
                _ => items.push(self.parse_leaf()?),
            }
        }
        Ok(FoamValue::List(items))
    }
}

/// Parse an optional `FoamFile { … }` header, returning it plus the remaining
/// tokens' start position.
fn parse_header(parser: &mut Parser) -> Result<Option<FoamHeader>, IoError> {
    if parser.peek_text() != Some("FoamFile") {
        return Ok(None);
    }
    parser.advance(); // FoamFile
    parser.expect("{")?;
    let mut header = FoamHeader::new();
    loop {
        match parser.peek_text() {
            Some("}") => {
                parser.advance();
                break;
            }
            None => return Err(parser.err("unexpected end of input inside FoamFile header")),
            _ => {
                let kw = parser.parse_keyword()?;
                // Header value = single raw token up to ';'.
                let vt = parser
                    .advance()
                    .ok_or_else(|| parser.err("expected a header value"))?;
                let raw = if vt.quoted {
                    format!("\"{}\"", vt.text)
                } else {
                    vt.text.clone()
                };
                parser.expect(";")?;
                header.set(&kw, &raw);
            }
        }
    }
    Ok(Some(header))
}

// ───────────────────────────────────────────────────────────────────────────
//  Public parse / write API
// ───────────────────────────────────────────────────────────────────────────

/// A parsed OpenFOAM dictionary file: its `FoamFile` header (if present) and
/// the body of entries.
#[derive(Debug, Clone, PartialEq)]
pub struct FoamFile {
    /// The `FoamFile { … }` header, or `None` if the file had none.
    pub header: Option<FoamHeader>,
    /// The dictionary body.
    pub dict: FoamDict,
}

impl FoamFile {
    /// Parse dictionary `text` into a header (if any) and body.
    pub fn parse(text: &str) -> Result<Self, IoError> {
        Self::parse_named(text, "dictionary")
    }

    /// Like [`Self::parse`] but labels parse errors with `context`.
    pub fn parse_named(text: &str, context: impl Into<String>) -> Result<Self, IoError> {
        let tokens = tokenize(text);
        let mut parser = Parser::new(tokens, context);
        let header = parse_header(&mut parser)?;
        let dict = parser.parse_dict_body(true)?;
        Ok(Self { header, dict })
    }

    /// Read and parse a dictionary file from `path`.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, IoError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| IoError::io(path.display().to_string(), e))?;
        Self::parse_named(&text, path.display().to_string())
    }

    /// Serialise to OpenFOAM ASCII text (banner + header + body).
    pub fn to_foam_string(&self) -> String {
        let mut out = String::new();
        out.push_str(BANNER);
        if let Some(h) = &self.header {
            write_header(&mut out, h);
        }
        out.push_str(SEPARATOR);
        out.push('\n');
        write_dict_body(&mut out, &self.dict, 0);
        out.push('\n');
        out.push_str(FOOTER);
        out
    }

    /// Write to `path` as OpenFOAM ASCII text.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), IoError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_foam_string())
            .map_err(|e| IoError::io(path.display().to_string(), e))
    }
}

// ── Serialisation helpers (also used by poly_mesh / field) ──────────────────

/// The OpenFOAM-style ASCII banner comment written at the top of every file.
pub(crate) const BANNER: &str = "\
/*--------------------------------*- C++ -*----------------------------------*\\
| =========                 |                                                 |
| \\\\      /  F ield         | OUTRAM PARK: independent OpenFOAM-format I/O    |
|  \\\\    /   O peration     | (not the official OpenFOAM software)            |
|   \\\\  /    A nd           |                                                 |
|    \\\\/     M anipulation  |                                                 |
\\*---------------------------------------------------------------------------*/
";

pub(crate) const SEPARATOR: &str =
    "// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //";

pub(crate) const FOOTER: &str =
    "// ************************************************************************* //\n";

/// Write the `FoamFile { … }` header block.
pub(crate) fn write_header(out: &mut String, header: &FoamHeader) {
    out.push_str("FoamFile\n{\n");
    for (k, v) in &header.entries {
        let _ = writeln!(out, "    {k:<12}{v};");
    }
    out.push_str("}\n");
}

/// Format a scalar compactly: integers without a decimal point.
pub(crate) fn fmt_scalar(x: f64) -> String {
    if x.is_finite() && x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

fn write_value(out: &mut String, v: &FoamValue) {
    match v {
        FoamValue::Scalar(x) => out.push_str(&fmt_scalar(*x)),
        FoamValue::Word(w) => out.push_str(w),
        FoamValue::Str(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        FoamValue::List(items) => {
            out.push('(');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                write_value(out, it);
            }
            out.push(')');
        }
    }
}

fn write_dimensioned(out: &mut String, d: &Dimensioned) {
    out.push('[');
    for (i, e) in d.dims.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&fmt_scalar(*e));
    }
    out.push(']');
    for v in &d.value {
        out.push(' ');
        write_value(out, v);
    }
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("    ");
    }
}

/// Write the entries of `dict` at nesting `level` (each level = 4 spaces).
pub(crate) fn write_dict_body(out: &mut String, dict: &FoamDict, level: usize) {
    for (k, entry) in dict.iter() {
        match entry {
            FoamEntry::SubDict(sub) => {
                indent(out, level);
                out.push_str(k);
                out.push('\n');
                indent(out, level);
                out.push_str("{\n");
                write_dict_body(out, sub, level + 1);
                indent(out, level);
                out.push_str("}\n");
            }
            FoamEntry::Scalar(x) => {
                indent(out, level);
                let _ = writeln!(out, "{k:<16}{};", fmt_scalar(*x));
            }
            FoamEntry::Word(w) => {
                indent(out, level);
                let _ = writeln!(out, "{k:<16}{w};");
            }
            FoamEntry::Str(s) => {
                indent(out, level);
                let _ = writeln!(out, "{k:<16}\"{s}\";");
            }
            FoamEntry::Tokens(vs) => {
                indent(out, level);
                let _ = write!(out, "{k:<16}");
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    write_value(out, v);
                }
                out.push_str(";\n");
            }
            FoamEntry::List(items) => {
                indent(out, level);
                let _ = write!(out, "{k:<16}");
                let mut v = String::new();
                write_value(&mut v, &FoamValue::List(items.clone()));
                out.push_str(&v);
                out.push_str(";\n");
            }
            FoamEntry::Dimensioned(d) => {
                indent(out, level);
                let _ = write!(out, "{k:<16}");
                write_dimensioned(out, d);
                out.push_str(";\n");
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Tests
// ───────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_glued_flag() {
        let toks = tokenize("div(phi,U) (0 0 1)");
        // div ( phi,U ) ( 0 0 1 )
        assert_eq!(toks[0].text, "div");
        assert_eq!(toks[1].text, "(");
        assert!(toks[1].glued, "`(` after div should be glued");
        // the list's opening paren is preceded by whitespace
        let list_open = toks.iter().rposition(|t| t.text == "(").unwrap();
        assert!(!toks[list_open].glued);
    }

    #[test]
    fn parse_function_keyword() {
        let f = FoamFile::parse("divSchemes { div(phi,U) Gauss linear; }").unwrap();
        let ds = f.dict.get_dict("divSchemes").unwrap();
        match ds.get("div(phi,U)").unwrap() {
            FoamEntry::Tokens(v) => {
                assert_eq!(v[0], FoamValue::Word("Gauss".into()));
                assert_eq!(v[1], FoamValue::Word("linear".into()));
            }
            other => panic!("expected Tokens, got {other:?}"),
        }
    }

    #[test]
    fn dimensioned_entry() {
        let f = FoamFile::parse("nu [0 2 -1 0 0 0 0] 1e-05;").unwrap();
        match f.dict.get("nu").unwrap() {
            FoamEntry::Dimensioned(d) => {
                assert_eq!(d.dims, [0.0, 2.0, -1.0, 0.0, 0.0, 0.0, 0.0]);
                assert_eq!(d.value, vec![FoamValue::Scalar(1e-5)]);
            }
            other => panic!("expected Dimensioned, got {other:?}"),
        }
    }
}
