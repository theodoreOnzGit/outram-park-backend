// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: wicked-good-xpath 1.3.1-z002 (Zotero's build; MIT,
//   https://github.com/google/wicked-good-xpath; Copyright (c) 2007 Cybozu
//   Labs, Inc., Copyright (c) 2012 Google Inc.), the XPath engine the
//   translation-server installs on jsdom (src/translation/translate.js :86-88
//   `wgxpath.install(dom.window, true)`). Ported from its compiled
//   dist/wgxpath.install-node.js: the lexer (`Ca`, the token regex `Da`),
//   parser (`xb`, `yb`, `Cb`, `Db`, `Eb`, `Bb`), node tests (`F` name test,
//   `G` kind test), axes (`U(...)`), the descendant search (`D`/`La`/`Na`,
//   via getElementsByTagName(NS)), the attribute-predicate shortcut (`o`,
//   `C`), predicates (`ab`), comparisons (`P`), the core functions (`R(...)`)
//   and string values (`A`). Zotero utilities (commit 4051881d59c6)
//   utilities.js `xpath` :1347-1420 and `xpathText` :1434-1452.
// Copyright (c) 2007 Cybozu Labs, Inc.; (c) 2012 Google Inc. (MIT);
//   Corporation for Digital Scholarship (utilities, AGPL-3.0-or-later).
// Licence: AGPL-3.0 (this port; MIT permits it).

//! `ZU.xpath` and `ZU.xpathText`: XPath 1.0 as wicked-good-xpath evaluates
//! it over jsdom, quirks included, because translators were written (and
//! their test cases recorded) against exactly this engine.
//!
//! Quirks that differ from the XPath 1.0 recommendation and are kept:
//!
//! * An **unprefixed name test** matches elements in the document's default
//!   namespace (`document.lookupNamespaceURI(null)`, i.e. the root element's
//!   default namespace), not elements in no namespace. So `crossref/journal`
//!   finds elements in the Crossref namespace when that is the default.
//! * `//name` (child axis, no positional predicate) searches with
//!   `getElementsByTagName(name)`, which matches the **qualified** name: an
//!   element written `x:name` is not found by `//name` even when `x` is
//!   bound to the default namespace (the child axis would find it).
//! * The attribute axis matches by `getNamedItem(qualifiedName)` (or
//!   `getNamedItemNS` for a prefixed test) and ignores the default
//!   namespace; `@*` includes `xmlns` attributes.
//! * `text()` matches Text nodes only, not CDATA sections.
//! * A step whose first predicate is a bare `[@name]` (child-like axes) tests
//!   it as `!!getAttribute(localName)`: an empty attribute does not count.
//! * `number()` of an empty node-set is 0; comparing a node-set with a
//!   boolean converts each node's string value to a boolean.
//! * `namespace-uri()` is always "" and `lang()` always false.
//! * The string value of the document node concatenates the text of every
//!   non-element node under it, comments and PIs included.
//!
//! Strings index `char`s where JavaScript indexes UTF-16 units (see
//! [`super::js`]); the two differ only on astral-plane characters.

use super::context::TranslateError;
use super::js;
use super::xml::{node_type, NodeId, NodeKind, XNode, XmlDocument};
use std::cmp::Ordering;

/// An XPath error (upstream throws; `ZU.xpath` rethrows it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPathError(pub String);

impl From<XPathError> for TranslateError {
    fn from(e: XPathError) -> Self {
        TranslateError::Translator(e.0)
    }
}

type R<T> = Result<T, XPathError>;

fn err<T>(m: impl Into<String>) -> R<T> {
    Err(XPathError(m.into()))
}

// ---------------------------------------------------------------- lexer

fn is_w(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_name_part(c: char) -> bool {
    is_w(c) || c == '-' || c == '.'
}

/// `Ca`: the token regex `Da` applied globally, whitespace tokens dropped.
fn tokenize(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    // One name part: `*` or `[\w-.]+`, not starting with [0-9-.].
    let part = |i: usize| -> Option<usize> {
        let ch = *c.get(i)?;
        if ch.is_ascii_digit() || ch == '-' || ch == '.' {
            return None;
        }
        if ch == '*' {
            return Some(i + 1);
        }
        let mut j = i;
        while c.get(j).is_some_and(|&x| is_name_part(x)) {
            j += 1;
        }
        (j > i).then_some(j)
    };
    while i < c.len() {
        let start = i;
        // \$?(?:prefix:)?local
        let mut j = i;
        if c[j] == '$' {
            j += 1;
        }
        let name_end = part(j).map(|e| {
            if c.get(e) == Some(&':') {
                if let Some(e2) = part(e + 1) {
                    return e2;
                }
            }
            e
        });
        if let Some(e) = name_end {
            out.push(c[start..e].iter().collect());
            i = e;
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        if two == "//" || two == ".." || two == "::" {
            out.push(two);
            i += 2;
            continue;
        }
        if c[i].is_ascii_digit() {
            let mut j = i;
            while c.get(j).is_some_and(|x| x.is_ascii_digit()) {
                j += 1;
            }
            if c.get(j) == Some(&'.') {
                j += 1;
                while c.get(j).is_some_and(|x| x.is_ascii_digit()) {
                    j += 1;
                }
            }
            out.push(c[i..j].iter().collect());
            i = j;
            continue;
        }
        if c[i] == '.' && c.get(i + 1).is_some_and(|x| x.is_ascii_digit()) {
            let mut j = i + 1;
            while c.get(j).is_some_and(|x| x.is_ascii_digit()) {
                j += 1;
            }
            out.push(c[i..j].iter().collect());
            i = j;
            continue;
        }
        if c[i] == '"' || c[i] == '\'' {
            if let Some(k) = c[i + 1..].iter().position(|&x| x == c[i]) {
                out.push(c[i..i + 2 + k].iter().collect());
                i += 2 + k;
                continue;
            }
        }
        if matches!(c[i], '!' | '<' | '>') && c.get(i + 1) == Some(&'=') {
            out.push(two);
            i += 2;
            continue;
        }
        if js::is_space(c[i]) {
            while c.get(i).is_some_and(|&x| js::is_space(x)) {
                i += 1;
            }
            continue;
        }
        // `.` (any character but a line terminator; those are spaces).
        out.push(c[i].to_string());
        i += 1;
    }
    out
}

/// `/(?![0-9])[\w]/.test(s)`: s contains a word character that is not a
/// digit.
fn has_word_start(s: &str) -> bool {
    s.chars().any(|c| is_w(c) && !c.is_ascii_digit())
}

// ---------------------------------------------------------------- AST

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Div,
    Mod,
    Mul,
    Add,
    Sub,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinOp {
    fn from_token(t: &str) -> Option<BinOp> {
        Some(match t {
            "div" => BinOp::Div,
            "mod" => BinOp::Mod,
            "*" => BinOp::Mul,
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "<" => BinOp::Lt,
            ">" => BinOp::Gt,
            "<=" => BinOp::Le,
            ">=" => BinOp::Ge,
            "=" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "and" => BinOp::And,
            "or" => BinOp::Or,
            _ => return None,
        })
    }

    /// Precedence (`Ya.C`).
    fn prec(self) -> u8 {
        match self {
            BinOp::Div | BinOp::Mod | BinOp::Mul => 6,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
            BinOp::Eq | BinOp::Ne => 3,
            BinOp::And => 2,
            BinOp::Or => 1,
        }
    }

    /// Result type (`Ya.j`): 1 number, 2 boolean.
    fn ty(self) -> u8 {
        if self.prec() >= 5 {
            1
        } else {
            2
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Func {
    Boolean,
    Ceiling,
    Concat,
    Contains,
    Count,
    False,
    Floor,
    Id,
    Lang,
    Last,
    LocalName,
    Name,
    NamespaceUri,
    NormalizeSpace,
    Not,
    Number,
    Position,
    Round,
    StartsWith,
    String,
    StringLength,
    Substring,
    SubstringAfter,
    SubstringBefore,
    Sum,
    Translate,
    True,
}

impl Func {
    fn from_name(n: &str) -> Option<Func> {
        Some(match n {
            "boolean" => Func::Boolean,
            "ceiling" => Func::Ceiling,
            "concat" => Func::Concat,
            "contains" => Func::Contains,
            "count" => Func::Count,
            "false" => Func::False,
            "floor" => Func::Floor,
            "id" => Func::Id,
            "lang" => Func::Lang,
            "last" => Func::Last,
            "local-name" => Func::LocalName,
            "name" => Func::Name,
            "namespace-uri" => Func::NamespaceUri,
            "normalize-space" => Func::NormalizeSpace,
            "not" => Func::Not,
            "number" => Func::Number,
            "position" => Func::Position,
            "round" => Func::Round,
            "starts-with" => Func::StartsWith,
            "string" => Func::String,
            "string-length" => Func::StringLength,
            "substring" => Func::Substring,
            "substring-after" => Func::SubstringAfter,
            "substring-before" => Func::SubstringBefore,
            "sum" => Func::Sum,
            "translate" => Func::Translate,
            "true" => Func::True,
            _ => return None,
        })
    }

    /// (result type, position-dependent, min args, max args (None:
    /// unbounded), args must be node-sets), from the `R(...)` table.
    fn spec(self) -> (u8, bool, usize, Option<usize>, bool) {
        match self {
            Func::Boolean => (2, false, 1, Some(1), false),
            Func::Ceiling => (1, false, 1, Some(1), false),
            Func::Concat => (3, false, 2, None, false),
            Func::Contains => (2, false, 2, Some(2), false),
            Func::Count => (1, false, 1, Some(1), true),
            Func::False => (2, false, 0, Some(0), false),
            Func::Floor => (1, false, 1, Some(1), false),
            Func::Id => (4, false, 1, Some(1), false),
            Func::Lang => (2, false, 1, Some(1), false),
            Func::Last => (1, true, 0, Some(0), false),
            Func::LocalName => (3, false, 0, Some(1), true),
            Func::Name => (3, false, 0, Some(1), true),
            Func::NamespaceUri => (3, true, 0, Some(1), true),
            Func::NormalizeSpace => (3, false, 0, Some(1), false),
            Func::Not => (2, false, 1, Some(1), false),
            Func::Number => (1, false, 0, Some(1), false),
            Func::Position => (1, true, 0, Some(0), false),
            Func::Round => (1, false, 1, Some(1), false),
            Func::StartsWith => (2, false, 2, Some(2), false),
            Func::String => (3, false, 0, Some(1), false),
            Func::StringLength => (1, false, 0, Some(1), false),
            Func::Substring => (3, false, 2, Some(3), false),
            Func::SubstringAfter => (3, false, 2, Some(2), false),
            Func::SubstringBefore => (3, false, 2, Some(2), false),
            Func::Sum => (1, false, 1, Some(1), true),
            Func::Translate => (3, false, 3, Some(3), false),
            Func::True => (2, false, 0, Some(0), false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Ancestor,
    AncestorOrSelf,
    Attribute,
    Child,
    Descendant,
    DescendantOrSelf,
    Following,
    FollowingSibling,
    Namespace,
    Parent,
    Preceding,
    PrecedingSibling,
    SelfAxis,
}

impl Axis {
    fn from_name(n: &str) -> Option<Axis> {
        Some(match n {
            "ancestor" => Axis::Ancestor,
            "ancestor-or-self" => Axis::AncestorOrSelf,
            "attribute" => Axis::Attribute,
            "child" => Axis::Child,
            "descendant" => Axis::Descendant,
            "descendant-or-self" => Axis::DescendantOrSelf,
            "following" => Axis::Following,
            "following-sibling" => Axis::FollowingSibling,
            "namespace" => Axis::Namespace,
            "parent" => Axis::Parent,
            "preceding" => Axis::Preceding,
            "preceding-sibling" => Axis::PrecedingSibling,
            "self" => Axis::SelfAxis,
            _ => return None,
        })
    }

    /// `s`: a reverse axis (positions count from the end).
    fn reverse(self) -> bool {
        matches!(
            self,
            Axis::Ancestor | Axis::AncestorOrSelf | Axis::Preceding | Axis::PrecedingSibling
        )
    }

    /// `I`: the axis may use the `[@attr]` shortcut.
    fn shortcut(self) -> bool {
        matches!(
            self,
            Axis::Child | Axis::Descendant | Axis::DescendantOrSelf | Axis::Following | Axis::Preceding
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Comment,
    Text,
    Pi,
    Node,
}

#[derive(Debug, Clone, PartialEq)]
enum Test {
    /// `F`: local name (or `*`) and namespace: an explicit (resolved)
    /// namespace, `*`, or `None` for "the document's default namespace".
    Name { local: String, ns: Option<String> },
    /// `G`: a kind test (its name kept: the attribute axis uses it).
    Kind { kind: Kind, name: String },
}

impl Test {
    /// `b()`: the local name or the kind's name.
    fn name(&self) -> &str {
        match self {
            Test::Name { local, .. } => local,
            Test::Kind { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Step {
    axis: Axis,
    test: Test,
    preds: Vec<Expr>,
    /// `w`: the step follows `//`.
    desc: bool,
    /// `o`: the `[@name]` shortcut's attribute name.
    shortcut: Option<String>,
    /// `g`: a predicate depends on position.
    positional: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Union(Vec<Expr>),
    Path(Box<Expr>, Vec<Step>),
    Root,
    Context,
    Filter(Box<Expr>, Vec<Expr>),
    Literal(String),
    Number(f64),
    Call(Func, Vec<Expr>),
}

impl Expr {
    /// The result type `j`: 1 number, 2 boolean, 3 string, 4 node-set.
    fn ty(&self) -> u8 {
        match self {
            Expr::Binary(op, ..) => op.ty(),
            Expr::Neg(_) => 1,
            Expr::Union(_) | Expr::Root | Expr::Context => 4,
            Expr::Path(start, _) => start.ty(),
            Expr::Filter(p, _) => p.ty(),
            Expr::Literal(_) => 3,
            Expr::Number(_) => 1,
            Expr::Call(f, _) => f.spec().0,
        }
    }

    /// `g`: depends on the context position or size.
    fn positional(&self) -> bool {
        match self {
            Expr::Binary(_, a, b) => a.positional() || b.positional(),
            Expr::Neg(a) => a.positional(),
            Expr::Union(v) => v.iter().any(Expr::positional),
            Expr::Path(start, _) => start.positional(),
            Expr::Filter(p, _) => p.positional(),
            Expr::Call(f, args) => f.spec().1 || args.iter().any(Expr::positional),
            _ => false,
        }
    }

    /// `o` of a path expression: a single non-`//` attribute step whose test
    /// is not `*`.
    fn attr_shortcut(&self) -> Option<String> {
        match self {
            Expr::Path(_, steps) if steps.len() == 1 => {
                let s = &steps[0];
                (!s.desc && s.axis == Axis::Attribute && s.test.name() != "*")
                    .then(|| s.test.name().to_owned())
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- parser

struct Parser<'a> {
    toks: Vec<String>,
    i: usize,
    ns: &'a [(&'a str, &'a str)],
}

impl Parser<'_> {
    fn peek(&self, off: usize) -> Option<&str> {
        self.toks.get(self.i + off).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn done(&self) -> bool {
        self.i >= self.toks.len()
    }

    /// `V`.
    fn need(&self, msg: &str) -> R<()> {
        if self.done() {
            return err(msg);
        }
        Ok(())
    }

    /// `zb`.
    fn expect(&mut self, t: &str) -> R<()> {
        let got = self.next();
        if got.as_deref() != Some(t) {
            return err(format!(
                "Bad token, expected: {t} got: {}",
                got.unwrap_or_else(|| "undefined".into())
            ));
        }
        Ok(())
    }

    /// `xb`: operators by precedence, left-associative.
    fn expr(&mut self) -> R<Expr> {
        let mut stack: Vec<(Expr, BinOp)> = Vec::new();
        let mut b;
        loop {
            self.need("Missing right hand side of binary expression.")?;
            b = self.unary()?;
            let Some(t) = self.next() else {
                break;
            };
            let Some(op) = BinOp::from_token(&t) else {
                self.i -= 1;
                break;
            };
            while let Some((_, top)) = stack.last() {
                if op.prec() <= top.prec() {
                    let (l, o) = stack.pop().expect("non-empty");
                    b = Expr::Binary(o, Box::new(l), Box::new(b));
                } else {
                    break;
                }
            }
            stack.push((b, op));
        }
        while let Some((l, o)) = stack.pop() {
            b = Expr::Binary(o, Box::new(l), Box::new(b));
        }
        Ok(b)
    }

    /// `yb`: unary minus, then a path or a union of paths.
    fn unary(&mut self) -> R<Expr> {
        if self.peek(0) == Some("-") {
            self.next();
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        let b = self.path()?;
        if self.peek(0) != Some("|") {
            return Ok(b);
        }
        let mut v = vec![b];
        while self.next().as_deref() == Some("|") {
            self.need("Missing next union location path.")?;
            v.push(self.path()?);
        }
        self.i -= 1;
        Ok(Expr::Union(v))
    }

    /// `Bb`.
    fn literal(&mut self) -> R<Expr> {
        let t = self.next().unwrap_or_default();
        let n = t.chars().count();
        if n < 2 {
            return err("Unclosed literal string");
        }
        Ok(Expr::Literal(t.chars().skip(1).take(n - 2).collect()))
    }

    /// `Eb`.
    fn predicates(&mut self) -> R<Vec<Expr>> {
        let mut v = Vec::new();
        while self.peek(0) == Some("[") {
            self.next();
            self.need("Missing predicate expression.")?;
            v.push(self.expr()?);
            self.need("Unclosed predicate expression.")?;
            self.expect("]")?;
        }
        Ok(v)
    }

    /// `Cb`: a path expression.
    fn path(&mut self) -> R<Expr> {
        let mut steps = Vec::new();
        let start;
        let is_slash = |t: Option<&str>| matches!(t, Some("/") | Some("//"));
        if is_slash(self.peek(0)) {
            let op = self.next().expect("peeked");
            let d = self.peek(0).map(str::to_owned);
            if op == "/"
                && (self.done()
                    || d.as_deref().is_some_and(|d| {
                        d != "." && d != ".." && d != "@" && d != "*" && !has_word_start(d)
                    }))
            {
                return Ok(Expr::Root);
            }
            start = Expr::Root;
            self.need("Missing next location step.")?;
            steps.push(self.step(&op)?);
        } else {
            let c = self.peek(0).unwrap_or_default().to_owned();
            let first = c.chars().next().unwrap_or('\0');
            let mut primary: Option<Expr> = match first {
                '$' => return err("Variable reference not allowed in HTML XPath"),
                '(' => {
                    self.next();
                    let e = self.expr()?;
                    self.need("unclosed \"(\"")?;
                    self.expect(")")?;
                    Some(e)
                }
                '"' | '\'' => Some(self.literal()?),
                _ => match js_to_number(&c) {
                    n if !n.is_nan() => {
                        self.next();
                        Some(Expr::Number(n))
                    }
                    _ => {
                        if !is_kind_name(&c)
                            && has_word_start(&first.to_string())
                            && self.peek(1) == Some("(")
                        {
                            self.next();
                            let f = Func::from_name(&c);
                            self.next();
                            let mut args = Vec::new();
                            while self.peek(0) != Some(")") {
                                self.need("Missing function argument list.")?;
                                args.push(self.expr()?);
                                if self.peek(0) != Some(",") {
                                    break;
                                }
                                self.next();
                            }
                            self.need("Unclosed function argument list.")?;
                            let t = self.next();
                            if t.as_deref() != Some(")") {
                                return err(format!("Bad token: {}", t.unwrap_or_default()));
                            }
                            let Some(f) = f else {
                                return err(format!(
                                    "Cannot read properties of null (reading 'B'): unknown function {c}"
                                ));
                            };
                            Some(make_call(f, args)?)
                        } else {
                            None
                        }
                    }
                },
            };
            if let Some(p) = primary.take() {
                let p = if self.peek(0) == Some("[") {
                    let preds = self.predicates()?;
                    if !preds.is_empty() && p.ty() != 4 {
                        return err(
                            "Primary expression must evaluate to nodeset if filter has predicate(s).",
                        );
                    }
                    Expr::Filter(Box::new(p), preds)
                } else {
                    p
                };
                if is_slash(self.peek(0)) {
                    start = p;
                } else {
                    return Ok(p);
                }
            } else {
                steps.push(self.step("/")?);
                start = Expr::Context;
            }
        }
        while is_slash(self.peek(0)) {
            let op = self.next().expect("peeked");
            self.need("Missing next location step.")?;
            steps.push(self.step(&op)?);
        }
        Ok(Expr::Path(Box::new(start), steps))
    }

    /// `Db`: one location step after `op` ("/" or "//").
    fn step(&mut self, op: &str) -> R<Step> {
        let mk = |axis, test, preds: Vec<Expr>, desc| make_step(axis, test, preds, desc);
        if self.peek(0) == Some(".") {
            self.next();
            return Ok(mk(Axis::SelfAxis, kind_test("node"), Vec::new(), false));
        }
        if self.peek(0) == Some("..") {
            self.next();
            return Ok(mk(Axis::Parent, kind_test("node"), Vec::new(), false));
        }
        let axis;
        if self.peek(0) == Some("@") {
            axis = Axis::Attribute;
            self.next();
            self.need("Missing attribute name")?;
        } else if self.peek(1) == Some("::") {
            let t = self.peek(0).unwrap_or_default().to_owned();
            if !has_word_start(&t.chars().next().map(String::from).unwrap_or_default()) {
                return err(format!("Bad token: {t}"));
            }
            self.next();
            axis = Axis::from_name(&t).ok_or_else(|| XPathError(format!("No axis with name: {t}")))?;
            self.next();
            self.need("Missing node name")?;
        } else {
            axis = Axis::Child;
        }
        let e = self.peek(0).unwrap_or_default().to_owned();
        let c0 = e.chars().next().unwrap_or('\0');
        let test = if (is_w(c0) && !c0.is_ascii_digit()) || c0 == '*' {
            if self.peek(1) == Some("(") {
                if !is_kind_name(&e) {
                    return err(format!("Invalid node type: {e}"));
                }
                self.next();
                self.expect("(")?;
                self.need("Bad nodetype")?;
                let f = self.peek(0).and_then(|t| t.chars().next());
                if f == Some('"') || f == Some('\'') {
                    self.literal()?;
                }
                self.need("Bad nodetype")?;
                let t = self.next();
                if t.as_deref() != Some(")") {
                    return err(format!("Bad token: {}", t.unwrap_or_default()));
                }
                kind_test(&e)
            } else {
                self.next();
                match e.find(':') {
                    None => {
                        let ns = (e == "*").then(|| "*".to_owned());
                        Test::Name { local: e, ns }
                    }
                    Some(i) => {
                        let prefix = &e[..i];
                        let ns = if prefix == "*" {
                            "*".to_owned()
                        } else {
                            self.ns
                                .iter()
                                .find(|(p, _)| *p == prefix)
                                .map(|(_, u)| (*u).to_owned())
                                .filter(|u| !u.is_empty())
                                .ok_or_else(|| {
                                    XPathError(format!("Namespace prefix not declared: {prefix}"))
                                })?
                        };
                        Test::Name {
                            local: e[i + 1..].to_owned(),
                            ns: Some(ns),
                        }
                    }
                }
            }
        } else {
            let t = self.next().unwrap_or_default();
            return err(format!("Bad token: {t}"));
        };
        let preds = self.predicates()?;
        Ok(mk(axis, test, preds, op == "//"))
    }
}

fn is_kind_name(s: &str) -> bool {
    matches!(s, "comment" | "text" | "processing-instruction" | "node")
}

fn kind_test(name: &str) -> Test {
    let kind = match name {
        "comment" => Kind::Comment,
        "text" => Kind::Text,
        "processing-instruction" => Kind::Pi,
        _ => Kind::Node,
    };
    Test::Kind {
        kind,
        name: name.to_owned(),
    }
}

/// `T`'s constructor: the `[@name]` shortcut and the positional flag.
fn make_step(axis: Axis, test: Test, preds: Vec<Expr>, desc: bool) -> Step {
    let shortcut = if axis.shortcut() {
        preds.first().and_then(Expr::attr_shortcut)
    } else {
        None
    };
    let positional = preds
        .iter()
        .any(|p| p.positional() || p.ty() == 1 || p.ty() == 0);
    Step {
        axis,
        test,
        preds,
        desc,
        shortcut,
        positional,
    }
}

/// `bb`'s constructor checks.
fn make_call(f: Func, args: Vec<Expr>) -> R<Expr> {
    let (_, _, min, max, nodesets) = f.spec();
    if args.len() < min {
        return err(format!(
            "Function {f:?} expects at least{min} arguments, {} given",
            args.len()
        ));
    }
    if let Some(max) = max {
        if args.len() > max {
            return err(format!(
                "Function {f:?} expects at most {max} arguments, {} given",
                args.len()
            ));
        }
    }
    if nodesets {
        for (i, a) in args.iter().enumerate() {
            if a.ty() != 4 {
                return err(format!("Argument {i} to function {f:?} is not of type Nodeset"));
            }
        }
    }
    Ok(Expr::Call(f, args))
}

/// `new Ib(expr, resolver)`: parse.
fn compile(expr: &str, ns: &[(&str, &str)]) -> R<Expr> {
    if expr.is_empty() {
        return err("Empty XPath expression.");
    }
    let toks = tokenize(expr);
    if toks.is_empty() {
        return err("Invalid XPath expression.");
    }
    let mut p = Parser { toks, i: 0, ns };
    let e = p.expr()?;
    if !p.done() {
        return err(format!("Bad token: {}", p.next().unwrap_or_default()));
    }
    Ok(e)
}

// ---------------------------------------------------------------- values

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Set(Vec<XNode>),
}

/// The evaluation context `n`: node, position, size.
#[derive(Debug, Clone, Copy)]
struct Ctx {
    node: XNode,
    pos: usize,
    size: usize,
}

impl Ctx {
    fn of(node: XNode) -> Ctx {
        Ctx {
            node,
            pos: 1,
            size: 1,
        }
    }
}

struct Eval<'a> {
    doc: &'a XmlDocument,
    /// `ownerDocument.lookupNamespaceURI(null)`.
    default_ns: Option<String>,
}

/// JavaScript `ToNumber` of a string.
pub fn js_to_number(s: &str) -> f64 {
    let t = js::trim(s);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (p, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(h) = t.strip_prefix(p) {
            if !h.is_empty() && h.chars().all(|c| c.is_digit(radix)) {
                return h
                    .chars()
                    .fold(0.0, |a, c| a * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64);
            }
            return f64::NAN;
        }
    }
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let st = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == st {
            return f64::NAN;
        }
        i = j;
    }
    if i != b.len() {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// JavaScript `Number.prototype.toString()` (radix 10).
pub fn js_number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let neg = x < 0.0;
    let e = format!("{:e}", x.abs());
    let (mant, exp) = e.split_once('e').expect("{:e} has an exponent");
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().expect("exponent") + 1;
    let mut s = String::new();
    if k <= n && n <= 21 {
        s.push_str(&digits);
        s.push_str(&"0".repeat((n - k) as usize));
    } else if 0 < n && n <= 21 {
        s.push_str(&digits[..n as usize]);
        s.push('.');
        s.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        s.push_str("0.");
        s.push_str(&"0".repeat((-n) as usize));
        s.push_str(&digits);
    } else {
        s.push_str(&digits[..1]);
        if k > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        s.push('e');
        s.push(if n - 1 >= 0 { '+' } else { '-' });
        s.push_str(&(n - 1).abs().to_string());
    }
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

/// `Math.round`.
fn js_round(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return x;
    }
    let r = (x + 0.5).floor();
    if r == 0.0 && x < 0.0 {
        -0.0
    } else {
        r
    }
}

impl Eval<'_> {
    /// `A`: the string value of a node.
    fn string_value(&self, x: XNode) -> String {
        let d = self.doc;
        if let Some(a) = d.attr(x) {
            return a.value.clone();
        }
        let n = x.node();
        match d.kind(n) {
            NodeKind::Element(_) => d.text(n),
            NodeKind::Document => {
                // Walk from the document element through its subtree and
                // following siblings, adding every non-element node's value.
                let mut out = String::new();
                let Some(start) = d.document_element() else {
                    return out;
                };
                let mut sib = Some(start);
                while let Some(s) = sib {
                    self.walk_values(s, &mut out);
                    sib = d.next_sibling(s);
                }
                out
            }
            _ => d.node_value(x).unwrap_or_else(|| "null".into()),
        }
    }

    fn walk_values(&self, n: NodeId, out: &mut String) {
        if !self.doc.is_element(n) {
            if let Some(v) = self.doc.node_value(XNode::Node(n)) {
                out.push_str(&v);
            } else {
                out.push_str("null");
            }
        }
        for &c in self.doc.children(n) {
            self.walk_values(c, out);
        }
    }

    fn first_string(&self, set: &[XNode]) -> String {
        set.first().map(|&x| self.string_value(x)).unwrap_or_default()
    }

    /// `M`.
    fn to_str(&self, v: &Value) -> String {
        match v {
            Value::Set(s) => self.first_string(s),
            Value::Str(s) => s.clone(),
            Value::Num(n) => js_number_to_string(*n),
            Value::Bool(b) => b.to_string(),
        }
    }

    /// `L`.
    fn to_num(&self, v: &Value) -> f64 {
        match v {
            Value::Set(s) => js_to_number(&self.first_string(s)),
            Value::Str(s) => js_to_number(s),
            Value::Num(n) => *n,
            Value::Bool(b) => f64::from(u8::from(*b)),
        }
    }

    /// `N`.
    fn to_bool(v: &Value) -> bool {
        match v {
            Value::Set(s) => !s.is_empty(),
            Value::Str(s) => !s.is_empty(),
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Bool(b) => *b,
        }
    }

    /// `F.a`: the name test.
    fn name_matches(&self, x: XNode, local: &str, ns: &Option<String>) -> bool {
        let t = self.doc.node_type(x);
        if t != node_type::ELEMENT && t != node_type::ATTRIBUTE {
            return false;
        }
        let name = self.doc.local_name(x).unwrap_or_default();
        if local != "*" && local != name {
            return false;
        }
        match ns.as_deref() {
            Some("*") => true,
            Some(u) => self.doc.namespace_uri(x) == Some(u),
            None => self.doc.namespace_uri(x) == self.default_ns.as_deref(),
        }
    }

    fn test(&self, t: &Test, x: XNode) -> bool {
        match t {
            Test::Name { local, ns } => self.name_matches(x, local, ns),
            Test::Kind { kind, .. } => {
                let nt = self.doc.node_type(x);
                match kind {
                    Kind::Node => true,
                    Kind::Text => nt == node_type::TEXT,
                    Kind::Comment => nt == node_type::COMMENT,
                    Kind::Pi => nt == node_type::PI,
                }
            }
        }
    }

    /// `C`: the `[@name]` shortcut (truthy `getAttribute(name)`).
    fn has_attr(&self, x: XNode, attr: Option<&str>) -> bool {
        let Some(name) = attr else {
            return true;
        };
        match x {
            XNode::Node(n) if self.doc.is_element(n) => {
                self.doc.get_attribute(n, name).is_some_and(|v| !v.is_empty())
            }
            _ => false,
        }
    }

    /// `D` (`La`, `Na`): descendants of `x` matching the test.
    fn descendants(&self, t: &Test, x: XNode, attr: Option<&str>, out: &mut Vec<XNode>) {
        let XNode::Node(n) = x else {
            return;
        };
        match t {
            Test::Kind { .. } => {
                for d in self.doc.descendants(n) {
                    let dx = XNode::Node(d);
                    if self.has_attr(dx, attr) && self.test(t, dx) {
                        out.push(dx);
                    }
                }
            }
            Test::Name { local, ns } => {
                // Only Document and Element have getElementsByTagName(NS).
                if !matches!(self.doc.kind(n), NodeKind::Document | NodeKind::Element(_)) {
                    return;
                }
                let found = match ns.as_deref() {
                    Some(u) if u != "*" => self.doc.get_elements_by_tag_name_ns(n, Some(u), local),
                    _ => self.doc.get_elements_by_tag_name(n, local),
                };
                for e in found {
                    let ex = XNode::Node(e);
                    if self.test(t, ex) && self.has_attr(ex, attr) {
                        out.push(ex);
                    }
                }
            }
        }
    }

    /// An axis applied to one node (`U(...)` functions).
    fn axis(&self, axis: Axis, t: &Test, x: XNode, attr: Option<&str>) -> Vec<XNode> {
        let d = self.doc;
        let mut out = Vec::new();
        match axis {
            Axis::Ancestor | Axis::AncestorOrSelf => {
                if axis == Axis::AncestorOrSelf && self.test(t, x) {
                    out.push(x);
                }
                // An Attr has no parentNode in jsdom.
                let mut cur = x.as_node().and_then(|n| d.parent(n));
                while let Some(p) = cur {
                    if self.test(t, XNode::Node(p)) {
                        out.push(XNode::Node(p));
                    }
                    cur = d.parent(p);
                }
                out.reverse();
            }
            Axis::Attribute => {
                let XNode::Node(n) = x else {
                    return out;
                };
                let Some(el) = d.element(n) else {
                    return out;
                };
                let name = t.name();
                let all = matches!(t, Test::Kind { kind: Kind::Node, .. }) || name == "*";
                if all {
                    out.extend((0..el.attrs.len()).map(|i| XNode::Attr(n, i)));
                } else {
                    let ns = match t {
                        Test::Name { ns: Some(u), .. } if u != "*" => Some(u.as_str()),
                        _ => None,
                    };
                    let hit = match ns {
                        Some(u) => el
                            .attrs
                            .iter()
                            .position(|a| a.namespace.as_deref() == Some(u) && a.local == name),
                        None => el.attrs.iter().position(|a| a.qualified_name() == name),
                    };
                    if let Some(i) = hit {
                        out.push(XNode::Attr(n, i));
                    }
                }
            }
            Axis::Child => {
                if let XNode::Node(n) = x {
                    for &c in d.children(n) {
                        let cx = XNode::Node(c);
                        if self.has_attr(cx, attr) && self.test(t, cx) {
                            out.push(cx);
                        }
                    }
                }
            }
            Axis::Descendant => self.descendants(t, x, attr, &mut out),
            Axis::DescendantOrSelf => {
                if self.has_attr(x, attr) && self.test(t, x) {
                    out.push(x);
                }
                self.descendants(t, x, attr, &mut out);
            }
            Axis::Following => {
                let mut cur = x.as_node();
                while let Some(b) = cur {
                    let mut f = d.next_sibling(b);
                    while let Some(s) = f {
                        let sx = XNode::Node(s);
                        if self.has_attr(sx, attr) && self.test(t, sx) {
                            out.push(sx);
                        }
                        self.descendants(t, sx, attr, &mut out);
                        f = d.next_sibling(s);
                    }
                    cur = d.parent(b);
                }
                sort_dedup(d, &mut out);
            }
            Axis::FollowingSibling => {
                let mut f = x.as_node().and_then(|n| d.next_sibling(n));
                while let Some(s) = f {
                    if self.test(t, XNode::Node(s)) {
                        out.push(XNode::Node(s));
                    }
                    f = d.next_sibling(s);
                }
            }
            Axis::Namespace => {}
            Axis::Parent => match x {
                XNode::Attr(el, _) => out.push(XNode::Node(el)),
                XNode::Node(n) => {
                    if let Some(p) = d.parent(n) {
                        if self.test(t, XNode::Node(p)) {
                            out.push(XNode::Node(p));
                        }
                    }
                }
            },
            Axis::Preceding => {
                let mut chain = Vec::new();
                let mut cur = x.as_node();
                while let Some(b) = cur {
                    chain.push(b);
                    cur = d.parent(b);
                }
                chain.reverse();
                for &b in chain.iter().skip(1) {
                    let mut sibs = Vec::new();
                    let mut p = d.previous_sibling(b);
                    while let Some(s) = p {
                        sibs.push(s);
                        p = d.previous_sibling(s);
                    }
                    sibs.reverse();
                    for s in sibs {
                        let sx = XNode::Node(s);
                        if self.has_attr(sx, attr) && self.test(t, sx) {
                            out.push(sx);
                        }
                        self.descendants(t, sx, attr, &mut out);
                    }
                }
                sort_dedup(d, &mut out);
            }
            Axis::PrecedingSibling => {
                let mut p = x.as_node().and_then(|n| d.previous_sibling(n));
                while let Some(s) = p {
                    if self.test(t, XNode::Node(s)) {
                        out.push(XNode::Node(s));
                    }
                    p = d.previous_sibling(s);
                }
                out.reverse();
            }
            Axis::SelfAxis => {
                if self.test(t, x) {
                    out.push(x);
                }
            }
        }
        out
    }

    /// `ab`: apply predicates from index `from`.
    fn filter(&self, preds: &[Expr], mut set: Vec<XNode>, from: usize, reverse: bool) -> R<Vec<XNode>> {
        for p in preds.iter().skip(from) {
            let size = set.len();
            let mut kept = Vec::with_capacity(size);
            for (k, &x) in set.iter().enumerate() {
                let pos = if reverse { size - k } else { k + 1 };
                let v = self.eval(p, Ctx { node: x, pos, size })?;
                let keep = match v {
                    Value::Num(n) => pos as f64 == n,
                    Value::Str(s) => !s.is_empty(),
                    Value::Bool(b) => b,
                    Value::Set(s) => !s.is_empty(),
                };
                if keep {
                    kept.push(x);
                }
            }
            set = kept;
        }
        Ok(set)
    }

    /// `T.prototype.m`: the axis from one node, then the predicates.
    fn step_from(&self, s: &Step, x: XNode) -> R<Vec<XNode>> {
        let from = usize::from(s.shortcut.is_some());
        let set = self.axis(s.axis, &s.test, x, s.shortcut.as_deref());
        self.filter(&s.preds, set, from, s.axis.reverse())
    }

    /// `T.prototype.a`: a step.
    fn step(&self, s: &Step, ctx: Ctx) -> R<Vec<XNode>> {
        if !s.desc {
            return self.step_from(s, ctx.node);
        }
        if s.positional || s.axis != Axis::Child {
            let all = self.axis(Axis::DescendantOrSelf, &kind_test("node"), ctx.node, None);
            let mut out = Vec::new();
            for x in all {
                out.extend(self.step_from(s, x)?);
            }
            sort_dedup(self.doc, &mut out);
            Ok(out)
        } else {
            let mut set = Vec::new();
            self.descendants(&s.test, ctx.node, s.shortcut.as_deref(), &mut set);
            let from = usize::from(s.shortcut.is_some());
            self.filter(&s.preds, set, from, false)
        }
    }

    fn eval(&self, e: &Expr, ctx: Ctx) -> R<Value> {
        Ok(match e {
            Expr::Literal(s) => Value::Str(s.clone()),
            Expr::Number(n) => Value::Num(*n),
            Expr::Root => Value::Set(vec![XNode::Node(self.doc.document())]),
            Expr::Context => Value::Set(vec![ctx.node]),
            Expr::Neg(a) => Value::Num(-self.num_of(a, ctx)?),
            Expr::Union(v) => {
                let mut out = Vec::new();
                for x in v {
                    match self.eval(x, ctx)? {
                        Value::Set(s) => out.extend(s),
                        _ => return err("Path expression must evaluate to NodeSet."),
                    }
                }
                sort_dedup(self.doc, &mut out);
                Value::Set(out)
            }
            Expr::Filter(p, preds) => match self.eval(p, ctx)? {
                Value::Set(s) => Value::Set(self.filter(preds, s, 0, false)?),
                other => other,
            },
            Expr::Path(start, steps) => {
                let Value::Set(mut set) = self.eval(start, ctx)? else {
                    return err("Filter expression must evaluate to nodeset.");
                };
                for s in steps {
                    if set.is_empty() {
                        break;
                    }
                    let mut next = Vec::new();
                    for &x in &set {
                        next.extend(self.step(s, Ctx::of(x))?);
                    }
                    sort_dedup(self.doc, &mut next);
                    set = next;
                }
                Value::Set(set)
            }
            Expr::Binary(op, a, b) => self.binary(*op, a, b, ctx)?,
            Expr::Call(f, args) => self.call(*f, args, ctx)?,
        })
    }

    fn num_of(&self, e: &Expr, ctx: Ctx) -> R<f64> {
        Ok(self.to_num(&self.eval(e, ctx)?))
    }

    fn str_of(&self, e: &Expr, ctx: Ctx) -> R<String> {
        Ok(self.to_str(&self.eval(e, ctx)?))
    }

    fn binary(&self, op: BinOp, a: &Expr, b: &Expr, ctx: Ctx) -> R<Value> {
        Ok(match op {
            BinOp::Div => Value::Num(self.num_of(a, ctx)? / self.num_of(b, ctx)?),
            BinOp::Mod => Value::Num(self.num_of(a, ctx)? % self.num_of(b, ctx)?),
            BinOp::Mul => Value::Num(self.num_of(a, ctx)? * self.num_of(b, ctx)?),
            BinOp::Add => Value::Num(self.num_of(a, ctx)? + self.num_of(b, ctx)?),
            BinOp::Sub => Value::Num(self.num_of(a, ctx)? - self.num_of(b, ctx)?),
            BinOp::And => {
                Value::Bool(Self::to_bool(&self.eval(a, ctx)?) && Self::to_bool(&self.eval(b, ctx)?))
            }
            BinOp::Or => {
                Value::Bool(Self::to_bool(&self.eval(a, ctx)?) || Self::to_bool(&self.eval(b, ctx)?))
            }
            _ => {
                let l = self.eval(a, ctx)?;
                let r = self.eval(b, ctx)?;
                Value::Bool(self.compare(op, &l, &r))
            }
        })
    }

    /// `P`.
    fn compare(&self, op: BinOp, b: &Value, c: &Value) -> bool {
        let is_eq = matches!(op, BinOp::Eq | BinOp::Ne);
        match (b, c) {
            (Value::Set(x), Value::Set(y)) => x.iter().any(|&d| {
                let ds = Prim::Str(self.string_value(d));
                y.iter()
                    .any(|&f| prim_cmp(op, &ds, &Prim::Str(self.string_value(f))))
            }),
            (Value::Set(set), other) | (other, Value::Set(set)) => {
                let set_left = matches!(b, Value::Set(_));
                let prim = to_prim(other);
                set.iter().any(|&k| {
                    let s = self.string_value(k);
                    let kv = match prim {
                        Prim::Num(_) => Prim::Num(js_to_number(&s)),
                        Prim::Bool(_) => Prim::Bool(!s.is_empty()),
                        Prim::Str(_) => Prim::Str(s),
                    };
                    if set_left {
                        prim_cmp(op, &kv, &prim)
                    } else {
                        prim_cmp(op, &prim, &kv)
                    }
                })
            }
            _ => {
                let (pb, pc) = (to_prim(b), to_prim(c));
                if is_eq {
                    if matches!(pb, Prim::Bool(_)) || matches!(pc, Prim::Bool(_)) {
                        prim_cmp(op, &Prim::Bool(Self::to_bool(b)), &Prim::Bool(Self::to_bool(c)))
                    } else if matches!(pb, Prim::Num(_)) || matches!(pc, Prim::Num(_)) {
                        prim_cmp(op, &Prim::Num(self.to_num(b)), &Prim::Num(self.to_num(c)))
                    } else {
                        prim_cmp(op, &pb, &pc)
                    }
                } else {
                    prim_cmp(op, &Prim::Num(self.to_num(b)), &Prim::Num(self.to_num(c)))
                }
            }
        }
    }

    fn opt_node(&self, args: &[Expr], ctx: Ctx) -> R<Option<XNode>> {
        Ok(match args.first() {
            None => Some(ctx.node),
            Some(a) => match self.eval(a, ctx)? {
                Value::Set(s) => s.first().copied(),
                _ => None,
            },
        })
    }

    fn opt_string(&self, args: &[Expr], ctx: Ctx) -> R<String> {
        Ok(match args.first() {
            Some(a) => self.str_of(a, ctx)?,
            None => self.string_value(ctx.node),
        })
    }

    fn call(&self, f: Func, args: &[Expr], ctx: Ctx) -> R<Value> {
        let d = self.doc;
        Ok(match f {
            Func::Boolean => Value::Bool(Self::to_bool(&self.eval(&args[0], ctx)?)),
            Func::Ceiling => Value::Num(self.num_of(&args[0], ctx)?.ceil()),
            Func::Concat => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&self.str_of(a, ctx)?);
                }
                Value::Str(s)
            }
            Func::Contains => {
                let (a, b) = (self.str_of(&args[0], ctx)?, self.str_of(&args[1], ctx)?);
                Value::Bool(a.contains(&b))
            }
            Func::Count => match self.eval(&args[0], ctx)? {
                Value::Set(s) => Value::Num(s.len() as f64),
                _ => Value::Num(0.0),
            },
            Func::False => Value::Bool(false),
            Func::Floor => Value::Num(self.num_of(&args[0], ctx)?.floor()),
            Func::Id => {
                let ids = self.str_of(&args[0], ctx)?;
                let mut out = Vec::new();
                for id in ids.split(js::is_space).filter(|s| !s.is_empty()) {
                    let hit = d.descendants(d.document()).into_iter().find(|&n| {
                        d.element(n).is_some_and(|e| {
                            e.attrs
                                .iter()
                                .any(|a| a.namespace.is_none() && a.local == "id" && a.value == id)
                        })
                    });
                    if let Some(h) = hit {
                        if !out.contains(&XNode::Node(h)) {
                            out.push(XNode::Node(h));
                        }
                    }
                }
                sort_dedup(d, &mut out);
                Value::Set(out)
            }
            Func::Lang => Value::Bool(false),
            Func::Last => Value::Num(ctx.size as f64),
            Func::LocalName => Value::Str(match self.opt_node(args, ctx)? {
                Some(x) => d
                    .local_name(x)
                    .map(str::to_owned)
                    .unwrap_or_else(|| d.node_name(x)),
                None => String::new(),
            }),
            Func::Name => Value::Str(match self.opt_node(args, ctx)? {
                Some(x) => match d.local_name(x) {
                    Some(l) => match d.prefix(x) {
                        Some(p) => format!("{p}:{l}"),
                        None => l.to_owned(),
                    },
                    None => d.node_name(x).to_lowercase(),
                },
                None => String::new(),
            }),
            Func::NamespaceUri => Value::Str(String::new()),
            Func::NormalizeSpace => {
                let s = self.opt_string(args, ctx)?;
                Value::Str(super::utilities::trim_internal(&s))
            }
            Func::Not => Value::Bool(!Self::to_bool(&self.eval(&args[0], ctx)?)),
            Func::Number => Value::Num(match args.first() {
                Some(a) => self.num_of(a, ctx)?,
                None => js_to_number(&self.string_value(ctx.node)),
            }),
            Func::Position => Value::Num(ctx.pos as f64),
            Func::Round => Value::Num(js_round(self.num_of(&args[0], ctx)?)),
            Func::StartsWith => {
                let (a, b) = (self.str_of(&args[0], ctx)?, self.str_of(&args[1], ctx)?);
                Value::Bool(a.starts_with(&b))
            }
            Func::String => Value::Str(self.opt_string(args, ctx)?),
            Func::StringLength => Value::Num(js::len(&self.opt_string(args, ctx)?) as f64),
            Func::Substring => {
                let s = self.str_of(&args[0], ctx)?;
                let c = self.num_of(&args[1], ctx)?;
                if c.is_nan() || c.is_infinite() {
                    return Ok(Value::Str(String::new()));
                }
                let len = match args.get(2) {
                    Some(a) => self.num_of(a, ctx)?,
                    None => f64::INFINITY,
                };
                if len.is_nan() || len == f64::NEG_INFINITY {
                    return Ok(Value::Str(String::new()));
                }
                let c = js_round(c) - 1.0;
                let start = c.max(0.0);
                let chars: Vec<char> = s.chars().collect();
                let clamp = |v: f64| -> usize { v.max(0.0).min(chars.len() as f64) as usize };
                // JS substring(a, b) swaps when a > b.
                let (a, b) = if len == f64::INFINITY {
                    (clamp(start), chars.len())
                } else {
                    let (x, y) = (clamp(start), clamp(c + js_round(len)));
                    (x.min(y), x.max(y))
                };
                Value::Str(chars[a..b].iter().collect())
            }
            Func::SubstringAfter => {
                let (a, b) = (self.str_of(&args[0], ctx)?, self.str_of(&args[1], ctx)?);
                Value::Str(match a.find(&b) {
                    Some(i) => a[i + b.len()..].to_owned(),
                    None => String::new(),
                })
            }
            Func::SubstringBefore => {
                let (a, b) = (self.str_of(&args[0], ctx)?, self.str_of(&args[1], ctx)?);
                Value::Str(match a.find(&b) {
                    Some(i) => a[..i].to_owned(),
                    None => String::new(),
                })
            }
            Func::Sum => match self.eval(&args[0], ctx)? {
                Value::Set(s) => {
                    Value::Num(s.iter().map(|&x| js_to_number(&self.string_value(x))).sum())
                }
                _ => Value::Num(0.0),
            },
            Func::Translate => {
                let s = self.str_of(&args[0], ctx)?;
                let from: Vec<char> = self.str_of(&args[1], ctx)?.chars().collect();
                let to: Vec<char> = self.str_of(&args[2], ctx)?.chars().collect();
                let mut map: Vec<(char, Option<char>)> = Vec::new();
                for (i, &c) in from.iter().enumerate() {
                    if !map.iter().any(|(k, _)| *k == c) {
                        map.push((c, to.get(i).copied()));
                    }
                }
                Value::Str(
                    s.chars()
                        .filter_map(|c| match map.iter().find(|(k, _)| *k == c) {
                            Some((_, r)) => *r,
                            None => Some(c),
                        })
                        .collect(),
                )
            }
            Func::True => Value::Bool(true),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Prim {
    Num(f64),
    Str(String),
    Bool(bool),
}

fn to_prim(v: &Value) -> Prim {
    match v {
        Value::Num(n) => Prim::Num(*n),
        Value::Str(s) => Prim::Str(s.clone()),
        Value::Bool(b) => Prim::Bool(*b),
        Value::Set(_) => Prim::Str(String::new()),
    }
}

/// JavaScript `==`, `!=`, `<`, ... on two values of the same primitive
/// type (relational operators on booleans compare them as numbers, on
/// strings by code units).
fn prim_cmp(op: BinOp, a: &Prim, b: &Prim) -> bool {
    let ord: Option<Ordering> = match (a, b) {
        (Prim::Num(x), Prim::Num(y)) => x.partial_cmp(y),
        (Prim::Str(x), Prim::Str(y)) => {
            if matches!(op, BinOp::Eq | BinOp::Ne) {
                Some(if x == y { Ordering::Equal } else { Ordering::Less })
            } else {
                Some(x.encode_utf16().cmp(y.encode_utf16()))
            }
        }
        (Prim::Bool(x), Prim::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    };
    match op {
        BinOp::Eq => ord == Some(Ordering::Equal),
        BinOp::Ne => ord != Some(Ordering::Equal),
        BinOp::Lt => ord == Some(Ordering::Less),
        BinOp::Gt => ord == Some(Ordering::Greater),
        BinOp::Le => matches!(ord, Some(Ordering::Less | Ordering::Equal)),
        BinOp::Ge => matches!(ord, Some(Ordering::Greater | Ordering::Equal)),
        _ => false,
    }
}

fn sort_dedup(doc: &XmlDocument, v: &mut Vec<XNode>) {
    v.sort_by(|a, b| doc.compare_order(*a, *b));
    v.dedup();
}

// ---------------------------------------------------------------- ZU API

/// `document.evaluate(expr, context, resolver, ORDERED_NODE_ITERATOR_TYPE)`
/// on one context node: the nodes, in document order.
pub fn evaluate(
    doc: &XmlDocument,
    context: XNode,
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, XPathError> {
    let e = compile(expr, ns)?;
    let ev = Eval {
        doc,
        default_ns: doc.lookup_namespace_uri(doc.document(), None),
    };
    match ev.eval(&e, Ctx::of(context))? {
        Value::Set(s) => Ok(s),
        _ => err("value could not be converted to the specified type"),
    }
}

/// `ZU.xpath(element, xpath, namespaces)` on one node (`namespaces`: prefix
/// and URI pairs; an empty slice is no resolver).
pub fn xpath(
    doc: &XmlDocument,
    node: impl Into<XNode>,
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, TranslateError> {
    Ok(evaluate(doc, node.into(), expr, ns)?)
}

/// `ZU.xpath(elements, xpath, namespaces)` on several nodes: each node's
/// results in turn (not merged).
pub fn xpath_all(
    doc: &XmlDocument,
    nodes: &[XNode],
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, TranslateError> {
    let mut out = Vec::new();
    for &n in nodes {
        out.extend(evaluate(doc, n, expr, ns)?);
    }
    Ok(out)
}

/// `ZU.xpathText(node, xpath, namespaces, delimiter)`: the matches' values
/// (an attribute's value, else `textContent`) joined by `delimiter`
/// (default ", "), `None` (null) when nothing matches.
pub fn xpath_text(
    doc: &XmlDocument,
    node: impl Into<XNode>,
    expr: &str,
    ns: &[(&str, &str)],
    delimiter: Option<&str>,
) -> Result<Option<String>, TranslateError> {
    let found = xpath(doc, node, expr, ns)?;
    Ok(join_text(doc, &found, delimiter))
}

/// [`xpath_text`] over several context nodes.
pub fn xpath_text_all(
    doc: &XmlDocument,
    nodes: &[XNode],
    expr: &str,
    ns: &[(&str, &str)],
    delimiter: Option<&str>,
) -> Result<Option<String>, TranslateError> {
    let found = xpath_all(doc, nodes, expr, ns)?;
    Ok(join_text(doc, &found, delimiter))
}

fn join_text(doc: &XmlDocument, found: &[XNode], delimiter: Option<&str>) -> Option<String> {
    if found.is_empty() {
        return None;
    }
    let parts: Vec<String> = found
        .iter()
        .map(|&x| doc.text_content(x).unwrap_or_default())
        .collect();
    Some(parts.join(delimiter.unwrap_or(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(d: &XmlDocument, v: &[XNode]) -> Vec<String> {
        v.iter()
            .map(|&x| format!("{}={}", d.node_name(x), d.text(x)))
            .collect()
    }

    /// Results recorded from wicked-good-xpath 1.3.1-z002 on jsdom 29.0.1
    /// (node, 2026-10-07).
    #[test]
    fn matches_wgxpath_on_namespaces() {
        let d = XmlDocument::parse(
            r#"<a xmlns="urn:x" xmlns:p="urn:p" id="1" p:q="2"><b id="3">1</b><p:b>2</p:b><c xmlns="">3</c><B>4</B><!--c--><?pi x?><![CDATA[cd]]>t</a>"#,
        )
        .unwrap();
        let root = XNode::Node(d.document());
        let x = |e: &str, ns: &[(&str, &str)]| names(&d, &evaluate(&d, root, e, ns).unwrap());
        assert_eq!(x("//b", &[]), ["b=1"]);
        assert!(x("//c", &[]).is_empty());
        assert_eq!(x("//p:b", &[("p", "urn:p")]), ["p:b=2"]);
        assert_eq!(x("//*", &[]), ["a=1234cdt", "b=1", "p:b=2", "c=3", "B=4"]);
        assert_eq!(x("//q:b", &[("q", "urn:x")]), ["b=1"]);
        assert_eq!(x("/a/b", &[]), ["b=1"]);
        assert_eq!(x("//@id", &[]), ["id=1", "id=3"]);
        assert_eq!(
            x("/a/@*", &[]),
            ["xmlns=urn:x", "xmlns:p=urn:p", "id=1", "p:q=2"]
        );
        assert_eq!(x("/a/text()", &[]), ["#text=t"]);
        assert_eq!(x("/a/node()", &[]).len(), 8);
    }

    #[test]
    fn predicates_unions_and_functions() {
        let d = XmlDocument::parse(
            r#"<r><x t="a">1</x><x t="">2</x><x>3</x><y><x t="b">4</x></y></r>"#,
        )
        .unwrap();
        let root = XNode::Node(d.document());
        let x = |e: &str| names(&d, &evaluate(&d, root, e, &[]).unwrap());
        assert_eq!(x("//x[1]"), ["x=1", "x=4"]);
        assert_eq!(x("(//x)[1]"), ["x=1"]);
        // The [@name] shortcut: an empty attribute does not count.
        assert_eq!(x("//x[@t]"), ["x=1", "x=4"]);
        assert_eq!(x("//x[not(@t)]"), ["x=3"]);
        assert_eq!(x("//x[@t='b'] | /r/x[last()]"), ["x=3", "x=4"]);
        assert_eq!(x("//x[starts-with(text(), '2') or contains(@t, 'a')]"), ["x=1", "x=2"]);
        assert_eq!(x("//y/x/.."), ["y=4"]);
        assert_eq!(x("/r/x[position() > 1]"), ["x=2", "x=3"]);
        assert!(evaluate(&d, root, "//p:x", &[]).is_err());
    }

    #[test]
    fn js_numbers() {
        assert_eq!(js_number_to_string(1.0), "1");
        assert_eq!(js_number_to_string(0.5), "0.5");
        assert_eq!(js_number_to_string(1e21), "1e+21");
        assert_eq!(js_number_to_string(1e-7), "1e-7");
        assert_eq!(js_number_to_string(123.456), "123.456");
        assert!(js_to_number("abc").is_nan());
        assert_eq!(js_to_number(" 12 "), 12.0);
        assert_eq!(js_to_number(""), 0.0);
        assert_eq!(js_to_number("0x1f"), 31.0);
    }
}
