// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: the XML parser behind the translation-server's DOMParser:
//   jsdom 29.0.1 (MIT, https://github.com/jsdom/jsdom)
//   lib/jsdom/browser/parser/xml.js (the saxes event handlers that build the
//   tree: text outside the root dropped, internal `<!ENTITY n "v">`
//   declarations picked up by a regular expression, the doctype's
//   name/publicId/systemId by regular expressions), driving saxes 6.0.0
//   (ISC, https://github.com/lddubeau/saxes) saxes.js with `xmlns: true`,
//   XML 1.0 forced: end-of-line normalisation (`getCode10`), attribute-value
//   whitespace normalisation (`sAttribValueQuoted`), entities
//   (`parseEntity`), namespace resolution (`processAttribsNS`, `resolve`,
//   `nsPairCheck`) and the well-formedness checks that make the parse fail.
// Copyright (c) 2010 Elijah Insua and the jsdom contributors (MIT);
//   Copyright (c) Louis-Dominique Dubeau and saxes contributors (ISC).
//   Both licences permit this use; this file is AGPL-3.0 as part of kovan.
// Licence: AGPL-3.0 (this port).

//! Parsing XML the way the translation-server does (`DOMParser` with
//! "text/xml"): saxes' events turned into the tree jsdom builds.
//!
//! Modelled: a leading byte-order mark skipped; CR LF and CR read as LF
//! everywhere; tab and newline in attribute values read as spaces; the five
//! predefined entities, character references, and internal
//! `<!ENTITY name "value">` declarations (their value inserted verbatim, as
//! jsdom does); namespaces (`xmlns` attributes kept as attributes in the
//! XMLNS namespace; the default namespace not applied to attributes);
//! `xml:` bound to the XML namespace; the XML declaration (only at the very
//! start, never a PI node); doctype, comments, PIs and CDATA nodes; text
//! outside the root element dropped.
//!
//! Every well-formedness error saxes reports makes jsdom return a
//! `<parsererror>` document, so a failure here is an `Err` with saxes'
//! message (without its line:column prefix). Checked: unmatched, unclosed
//! and stray tags; a second root or no root; text or entities outside the
//! root; undefined entities and malformed character references; `<` in an
//! attribute value; duplicate attributes; unbound prefixes and the
//! `xml`/`xmlns` binding rules; `]]>` in text; a misplaced XML declaration
//! or doctype; malformed comments; characters outside the XML 1.0 `Char`
//! production; names that are not XML names.

use super::xml::{ElementData, NodeId, NodeKind, XmlAttr, XmlDocument, XMLNS_NS, XML_NS};
use super::xml::XmlParseError;

fn is_name_start(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}'
        | '\u{F8}'..='\u{2FF}' | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}')
}

fn is_name_char(c: char) -> bool {
    is_name_start(c)
        || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn is_s(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn is_name(s: &str) -> bool {
    let mut it = s.chars();
    it.next().is_some_and(is_name_start) && it.all(is_name_char)
}

struct OpenTag {
    node: NodeId,
    name: String,
    /// Prefix bindings this tag declares.
    ns: Vec<(String, String)>,
}

struct Parser {
    s: Vec<char>,
    i: usize,
    doc: XmlDocument,
    stack: Vec<OpenTag>,
    entities: Vec<(String, String)>,
    saw_root: bool,
    closed_root: bool,
    saw_doctype: bool,
}

type R<T> = Result<T, XmlParseError>;

fn fail<T>(msg: &str) -> R<T> {
    Err(XmlParseError(msg.to_owned()))
}

/// Parse a whole document (jsdom `parseIntoDocument`); `Err` is the first
/// saxes error.
pub fn parse_document(input: &str) -> Result<XmlDocument, XmlParseError> {
    // End-of-line normalisation (saxes getCode10: CR LF and CR become LF).
    let mut s: Vec<char> = Vec::with_capacity(input.len());
    let mut it = input.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\r' {
            if it.peek() == Some(&'\n') {
                it.next();
            }
            s.push('\n');
        } else {
            s.push(c);
        }
    }
    let mut p = Parser {
        s,
        i: 0,
        doc: XmlDocument::new(),
        stack: Vec::new(),
        entities: vec![
            ("amp".into(), "&".into()),
            ("gt".into(), ">".into()),
            ("lt".into(), "<".into()),
            ("quot".into(), "\"".into()),
            ("apos".into(), "'".into()),
        ],
        saw_root: false,
        closed_root: false,
        saw_doctype: false,
    };
    p.run()?;
    Ok(p.doc)
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn starts(&self, lit: &str) -> bool {
        let mut j = self.i;
        for c in lit.chars() {
            if self.s.get(j) != Some(&c) {
                return false;
            }
            j += 1;
        }
        true
    }

    fn bump(&mut self) -> R<char> {
        match self.s.get(self.i).copied() {
            None => fail("unexpected end."),
            Some(c) => {
                self.i += 1;
                if !is_xml_char(c) {
                    return fail("disallowed character.");
                }
                Ok(c)
            }
        }
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(is_s) {
            self.i += 1;
        }
    }

    fn current_parent(&self) -> NodeId {
        self.stack
            .last()
            .map(|t| t.node)
            .unwrap_or(self.doc.document())
    }

    fn append(&mut self, kind: NodeKind) -> NodeId {
        let n = self.doc.push(kind);
        let p = self.current_parent();
        self.doc.append_child(p, n);
        n
    }

    fn run(&mut self) -> R<()> {
        if self.peek() == Some('\u{FEFF}') {
            self.i += 1;
        }
        let begin = self.i;
        self.skip_spaces();
        let mut xml_decl_possible = self.i == begin;
        let mut text = String::new();
        loop {
            let Some(c) = self.peek() else {
                break;
            };
            if c == '<' {
                self.flush_text(&mut text)?;
                self.i += 1;
                let possible = xml_decl_possible;
                xml_decl_possible = false;
                self.markup(possible)?;
            } else if c == '&' {
                self.i += 1;
                if self.stack.is_empty() {
                    return fail("text data outside of root node.");
                }
                let e = self.entity()?;
                text.push_str(&e);
            } else {
                self.i += 1;
                if !is_xml_char(c) {
                    return fail("disallowed character.");
                }
                if self.stack.is_empty() {
                    if !is_s(c) {
                        return fail("text data outside of root node.");
                    }
                } else {
                    if c == '>' && text.ends_with("]]") {
                        return fail("the string \"]]>\" is disallowed in char data.");
                    }
                    text.push(c);
                }
            }
        }
        self.flush_text(&mut text)?;
        if !self.saw_root {
            return fail("document must contain a root element.");
        }
        if let Some(t) = self.stack.last() {
            return fail(&format!("unclosed tag: {}", t.name));
        }
        Ok(())
    }

    fn flush_text(&mut self, text: &mut String) -> R<()> {
        if !text.is_empty() {
            if !self.stack.is_empty() {
                let t = std::mem::take(text);
                self.append(NodeKind::Text(t));
            } else {
                text.clear();
            }
        }
        Ok(())
    }

    /// After `<`.
    fn markup(&mut self, xml_decl_possible: bool) -> R<()> {
        match self.peek() {
            Some('/') => {
                self.i += 1;
                self.close_tag()
            }
            Some('!') => {
                self.i += 1;
                if self.starts("[CDATA[") {
                    self.i += 7;
                    if self.stack.is_empty() {
                        return fail("text data outside of root node.");
                    }
                    let data = self.until("]]>")?;
                    self.append(NodeKind::CData(data));
                    Ok(())
                } else if self.starts("--") {
                    self.i += 2;
                    let data = self.until("--")?;
                    if self.peek() != Some('>') {
                        return fail("malformed comment.");
                    }
                    self.i += 1;
                    self.append(NodeKind::Comment(data));
                    Ok(())
                } else if self.starts("DOCTYPE") {
                    self.i += 7;
                    if self.saw_doctype || self.saw_root {
                        return fail("inappropriately located doctype declaration.");
                    }
                    self.doctype()
                } else {
                    fail("incorrect syntax.")
                }
            }
            Some('?') => {
                self.i += 1;
                self.pi(xml_decl_possible)
            }
            Some(c) if is_name_start(c) => self.open_tag(),
            _ => fail("disallowed character in tag name"),
        }
    }

    /// Characters up to `end` (consumed), checked against `Char`.
    fn until(&mut self, end: &str) -> R<String> {
        let mut out = String::new();
        loop {
            if self.peek().is_none() {
                return fail("unexpected end.");
            }
            if self.starts(end) {
                self.i += end.chars().count();
                return Ok(out);
            }
            out.push(self.bump()?);
        }
    }

    fn name(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(is_name_char) {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    fn doctype(&mut self) -> R<()> {
        // saxes collects the declaration's text (quoted strings and the
        // internal subset verbatim) up to the closing `>`.
        let mut dt = String::new();
        let mut in_subset = false;
        loop {
            let c = self.bump()?;
            match c {
                '"' | '\'' => {
                    dt.push(c);
                    loop {
                        let d = self.bump()?;
                        dt.push(d);
                        if d == c {
                            break;
                        }
                    }
                }
                '[' if !in_subset => {
                    in_subset = true;
                    dt.push(c);
                }
                ']' if in_subset => {
                    in_subset = false;
                    dt.push(c);
                }
                '<' if in_subset && self.starts("!--") => {
                    dt.push(c);
                    self.i += 3;
                    dt.push_str("!--");
                    let body = self.until("--")?;
                    dt.push_str(&body);
                    dt.push_str("--");
                    if self.peek() != Some('>') {
                        return fail("malformed comment.");
                    }
                }
                '>' if !in_subset => break,
                _ => dt.push(c),
            }
        }
        self.saw_doctype = true;
        // jsdom xml.js parseDocType on `<!doctype ${dt}>`.
        let full = format!("<!doctype {dt}>");
        let (name, public_id, system_id) = parse_doctype(&full);
        self.append(NodeKind::DocumentType {
            name,
            public_id,
            system_id,
        });
        // jsdom: /<!ENTITY ([^ ]+) "([^"]+)">/g, first definition wins.
        let mut rest = dt.as_str();
        while let Some(i) = rest.find("<!ENTITY ") {
            let after = &rest[i + 9..];
            let parsed = (|| {
                let sp = after.find(' ')?;
                let name = &after[..sp];
                let v = after[sp + 1..].strip_prefix('"')?;
                let q = v.find('"')?;
                if q == 0 || !v[q + 1..].starts_with('>') {
                    return None;
                }
                Some((name.to_owned(), v[..q].to_owned()))
            })();
            if let Some((n, v)) = parsed {
                if !self.entities.iter().any(|(k, _)| *k == n) {
                    self.entities.push((n, v));
                }
            }
            rest = after;
        }
        Ok(())
    }

    fn pi(&mut self, xml_decl_possible: bool) -> R<()> {
        let target = self.name();
        if target.is_empty() {
            return fail("processing instruction without a target.");
        }
        if !is_name(&target) {
            return fail("disallowed character in processing instruction name.");
        }
        if target == "xml" {
            if !xml_decl_possible {
                return fail("an XML declaration must be at the start of the document.");
            }
            return self.xml_decl();
        }
        match self.peek() {
            Some('?') => {}
            Some(c) if is_s(c) => {}
            _ => return fail("disallowed character in processing instruction name."),
        }
        self.skip_spaces();
        let body = self.until("?>")?;
        if target.eq_ignore_ascii_case("xml") {
            return fail("the XML declaration must appear at the start of the document.");
        }
        self.append(NodeKind::Pi { target, data: body });
        Ok(())
    }

    fn xml_decl(&mut self) -> R<()> {
        let body = self.until("?>")?;
        // Pseudo-attributes in order: version, encoding?, standalone?
        let mut expect = vec!["version", "encoding", "standalone"];
        let mut rest = body.as_str();
        let mut saw_version = false;
        loop {
            let t = rest.trim_start_matches(is_s);
            if t.is_empty() {
                break;
            }
            if t.len() == rest.len() && saw_version {
                return fail("whitespace required.");
            }
            let eq = t
                .find('=')
                .ok_or_else(|| XmlParseError("value required.".into()))?;
            let name = t[..eq].trim_end_matches(is_s);
            let pos = expect
                .iter()
                .position(|e| *e == name)
                .ok_or_else(|| XmlParseError(format!("expected one of {}", expect.join(", "))))?;
            if !saw_version && name != "version" {
                return fail("expected the name version.");
            }
            expect.drain(..=pos);
            let v = t[eq + 1..].trim_start_matches(is_s);
            let q = v
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| XmlParseError("value must be quoted.".into()))?;
            let close = v[1..]
                .find(q)
                .ok_or_else(|| XmlParseError("XML declaration is incomplete.".into()))?;
            let value = &v[1..1 + close];
            match name {
                "version" => {
                    saw_version = true;
                    let ok = value
                        .strip_prefix("1.")
                        .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
                    if !ok {
                        return fail("version number must match /^1\\.[0-9]+$/.");
                    }
                }
                "encoding" => {
                    let ok = value
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic())
                        && value
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
                    if !ok {
                        return fail("encoding value must match /^[A-Za-z0-9][A-Za-z0-9._-]*$/.");
                    }
                }
                _ => {
                    if value != "yes" && value != "no" {
                        return fail("standalone value must match \"yes\" or \"no\".");
                    }
                }
            }
            rest = &v[1 + close + 1..];
        }
        if !saw_version {
            return fail("XML declaration must contain a version.");
        }
        Ok(())
    }

    /// The text of an entity reference after `&` (consumed through `;`).
    fn entity(&mut self) -> R<String> {
        let mut name = String::new();
        loop {
            match self.peek() {
                None => return fail("unexpected end."),
                Some(';') => {
                    self.i += 1;
                    break;
                }
                Some(_) => name.push(self.bump()?),
            }
        }
        if name.is_empty() {
            return fail("empty entity name.");
        }
        if let Some(num) = name.strip_prefix('#') {
            let code = if let Some(h) = num.strip_prefix('x') {
                if !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    None
                }
            } else if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
                num.parse::<u32>().ok()
            } else {
                None
            };
            return match code.and_then(char::from_u32).filter(|c| is_xml_char(*c)) {
                Some(c) => Ok(c.to_string()),
                None => fail("malformed character entity."),
            };
        }
        match self.entities.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => Ok(v.clone()),
            None if is_name(&name) => fail("undefined entity."),
            None => fail("disallowed character in entity name."),
        }
    }

    fn resolve(&self, prefix: &str, own: &[(String, String)]) -> Option<String> {
        if let Some((_, u)) = own.iter().rev().find(|(p, _)| p == prefix) {
            return Some(u.clone());
        }
        for t in self.stack.iter().rev() {
            if let Some((_, u)) = t.ns.iter().rev().find(|(p, _)| p == prefix) {
                return Some(u.clone());
            }
        }
        match prefix {
            "xml" => Some(XML_NS.to_owned()),
            "xmlns" => Some(XMLNS_NS.to_owned()),
            _ => None,
        }
    }

    fn open_tag(&mut self) -> R<()> {
        let name = self.name();
        if self.closed_root {
            return fail("documents may contain only one root.");
        }
        self.saw_root = true;
        // Attributes.
        let mut raw: Vec<(String, String)> = Vec::new();
        let self_closing;
        loop {
            let before = self.i;
            self.skip_spaces();
            let spaced = self.i > before;
            match self.peek() {
                None => return fail("unexpected end."),
                Some('>') => {
                    self.i += 1;
                    self_closing = false;
                    break;
                }
                Some('/') => {
                    self.i += 1;
                    if self.peek() != Some('>') {
                        return fail("forward-slash in opening tag not followed by >.");
                    }
                    self.i += 1;
                    self_closing = true;
                    break;
                }
                Some(c) if is_name_start(c) => {
                    if !spaced {
                        return fail(if raw.is_empty() {
                            "disallowed character in tag name."
                        } else {
                            "no whitespace between attributes."
                        });
                    }
                    let an = self.name();
                    self.skip_spaces();
                    if self.peek() != Some('=') {
                        return fail("attribute without value.");
                    }
                    self.i += 1;
                    self.skip_spaces();
                    let q = match self.peek() {
                        Some(q @ ('"' | '\'')) => q,
                        _ => return fail("unquoted attribute value."),
                    };
                    self.i += 1;
                    let mut v = String::new();
                    loop {
                        match self.peek() {
                            None => return fail("unexpected end."),
                            Some(c) if c == q => {
                                self.i += 1;
                                break;
                            }
                            Some('&') => {
                                self.i += 1;
                                let e = self.entity()?;
                                v.push_str(&e);
                            }
                            Some('<') => return fail("disallowed character."),
                            Some('\n') | Some('\t') => {
                                self.i += 1;
                                v.push(' ');
                            }
                            Some(_) => v.push(self.bump()?),
                        }
                    }
                    raw.push((an, v));
                }
                _ => {
                    return fail(if raw.is_empty() && !spaced {
                        "disallowed character in tag name."
                    } else {
                        "disallowed character in attribute name."
                    })
                }
            }
        }
        // Namespace declarations (pushAttribNS) and resolution
        // (processAttribsNS).
        let mut own: Vec<(String, String)> = Vec::new();
        for (an, v) in &raw {
            if let Some(local) = an.strip_prefix("xmlns:") {
                let t = super::js::trim(v).to_owned();
                if t.is_empty() {
                    return fail("invalid attempt to undefine prefix in XML 1.0");
                }
                ns_pair_check(local, &t)?;
                own.push((local.to_owned(), t));
            } else if an == "xmlns" {
                let t = super::js::trim(v).to_owned();
                ns_pair_check("", &t)?;
                own.push((String::new(), t));
            }
        }
        let (prefix, local) = qname(&name)?;
        let uri = self.resolve(&prefix, &own).unwrap_or_default();
        if !prefix.is_empty() {
            if prefix == "xmlns" {
                return fail("tags may not have \"xmlns\" as prefix.");
            }
            if uri.is_empty() {
                return fail(&format!("unbound namespace prefix: \"{prefix}\"."));
            }
        }
        let mut attrs: Vec<XmlAttr> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (an, v) in raw {
            let (ap, al) = qname(&an)?;
            let (auri, eq) = if ap.is_empty() {
                let u = if an == "xmlns" {
                    XMLNS_NS.to_owned()
                } else {
                    String::new()
                };
                (u, an.clone())
            } else {
                let u = match self.resolve(&ap, &own) {
                    Some(u) => u,
                    None => return fail(&format!("unbound namespace prefix: \"{ap}\".")),
                };
                let eq = format!("{{{u}}}{al}");
                (u, eq)
            };
            if seen.contains(&eq) {
                return fail(&format!("duplicate attribute: {eq}."));
            }
            seen.push(eq);
            attrs.push(XmlAttr {
                namespace: (!auri.is_empty()).then_some(auri),
                prefix: (!ap.is_empty()).then_some(ap),
                local: al,
                value: v,
            });
        }
        let node = self.append(NodeKind::Element(ElementData {
            namespace: (!uri.is_empty()).then_some(uri),
            prefix: (!prefix.is_empty()).then_some(prefix),
            local,
            attrs,
        }));
        if self_closing {
            if self.stack.is_empty() {
                self.closed_root = true;
            }
        } else {
            self.stack.push(OpenTag {
                node,
                name,
                ns: own,
            });
        }
        Ok(())
    }

    fn close_tag(&mut self) -> R<()> {
        let name = self.name();
        self.skip_spaces();
        if self.peek() != Some('>') {
            return fail("disallowed character in closing tag.");
        }
        self.i += 1;
        if name.is_empty() {
            return fail("weird empty close tag.");
        }
        match self.stack.last() {
            None => fail("unexpected close tag."),
            Some(t) if t.name == name => {
                self.stack.pop();
                if self.stack.is_empty() {
                    self.closed_root = true;
                }
                Ok(())
            }
            Some(_) => fail(&format!("unmatched closing tag: {name}.")),
        }
    }
}

/// saxes `qname`: split at the first colon; an empty side or a second colon
/// is malformed.
fn qname(name: &str) -> R<(String, String)> {
    match name.split_once(':') {
        None => Ok((String::new(), name.to_owned())),
        Some((p, l)) => {
            if p.is_empty() || l.is_empty() || l.contains(':') {
                return fail(&format!("malformed name: {name}."));
            }
            Ok((p.to_owned(), l.to_owned()))
        }
    }
}

/// saxes `nsPairCheck`.
fn ns_pair_check(prefix: &str, uri: &str) -> R<()> {
    if prefix == "xml" && uri != XML_NS {
        return fail(&format!("xml prefix must be bound to {XML_NS}."));
    }
    if prefix == "xmlns" && uri != XMLNS_NS {
        return fail(&format!("xmlns prefix must be bound to {XMLNS_NS}."));
    }
    if uri == XMLNS_NS {
        return fail("may not assign a prefix (even \"xmlns\") to the xmlns URI.");
    }
    if uri == XML_NS && prefix != "xml" {
        return fail(if prefix.is_empty() {
            "the default namespace may not be set to the xml URI."
        } else {
            "may not assign the xml namespace to another prefix."
        });
    }
    Ok(())
}

/// jsdom xml.js `parseDocType` on `<!doctype ...>`: (name, publicId,
/// systemId) by its regular expressions (ASCII case-insensitive keywords).
fn parse_doctype(html: &str) -> (String, String, String) {
    let lower = html.to_ascii_lowercase();
    if lower.contains("<!doctype html>") {
        return ("html".into(), String::new(), String::new());
    }
    let after = &html["<!doctype".len()..];
    let ws_end = after.len() - after.trim_start().len();
    let rest = &after[ws_end..];
    let name: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '>')
        .collect();
    let tail = &rest[name.len()..];
    let tail_l = tail.to_ascii_lowercase();
    // PUBLIC "p" "s" (both double-quoted, as the regex requires).
    let quoted = |s: &str| -> Option<(String, usize)> {
        let s2 = s.strip_prefix('"')?;
        let e = s2.find('"')?;
        Some((s2[..e].to_owned(), e + 2))
    };
    let t = tail.trim_start();
    let tl = tail_l.trim_start();
    let name_ok = !name.is_empty();
    if name_ok && tl.starts_with("public") && t.len() > 6 && t[6..].starts_with(char::is_whitespace)
    {
        let r = t[6..].trim_start();
        if let Some((p, n)) = quoted(r) {
            let r2 = &r[n..];
            if r2.starts_with(char::is_whitespace) {
                if let Some((s, _)) = quoted(r2.trim_start()) {
                    if !p.is_empty() && !s.is_empty() {
                        return (name, p, s);
                    }
                }
            }
        }
    }
    if name_ok && tl.starts_with("system") && t.len() > 6 && t[6..].starts_with(char::is_whitespace)
    {
        if let Some((s, _)) = quoted(t[6..].trim_start()) {
            if !s.is_empty() {
                return (name, String::new(), s);
            }
        }
    }
    let name = if name.is_empty() {
        "html".to_owned()
    } else {
        name
    };
    (name, String::new(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::framework::xml::XNode;

    #[test]
    fn eol_attribute_and_entity_normalisation() {
        let d = parse_document(
            "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\r\n<!DOCTYPE x [<!ENTITY foo \"bar&amp;\">]><a k=\"1\t2\r\n3&#10;\">x\r\ny&foo;&#x41;</a>",
        )
        .unwrap();
        let a = d.document_element().unwrap();
        assert_eq!(d.get_attribute(a, "k"), Some("1 2 3\n"));
        assert_eq!(d.text(XNode::Node(a)), "x\nybar&amp;A");
    }

    #[test]
    fn errors_like_saxes() {
        assert!(parse_document(" <?xml version=\"1.0\"?><a/>").is_err());
        assert!(parse_document("<a/><b/>").is_err());
        assert!(parse_document("<a>&nbsp;</a>").is_err());
        assert!(parse_document("<p:a/>").is_err());
        assert!(parse_document("<a x='1' x='2'/>").is_err());
        assert!(parse_document("<a/>x").is_err());
        assert!(parse_document(" \n<a/>\n").is_ok());
        assert!(parse_document("\u{FEFF}<?xml version=\"1.0\"?><a/>").is_ok());
    }
}
