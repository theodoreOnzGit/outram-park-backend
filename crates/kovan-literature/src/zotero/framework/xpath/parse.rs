// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: wicked-good-xpath 1.3.1-z002 (MIT; Copyright (c) 2007 Cybozu
//   Labs, Inc., Copyright (c) 2012 Google Inc.), dist/wgxpath.install-node.js:
//   the lexer (`Ca`, token regex `Da`), parser (`xb`, `yb`, `Cb`, `Db`, `Eb`,
//   `Bb`), node tests (`F`, `G`), the axis table (`U(...)`), the function
//   table (`R(...)`), the `[@attr]` shortcut (`o`). See [`super`].
// Licence: AGPL-3.0 (this port; MIT permits it).

//! XPath lexing and parsing into an expression tree, as wicked-good-xpath
//! does it.

use super::eval::js_to_number;
use super::{err, XPathError, R};
use crate::zotero::framework::js;

// ---------------------------------------------------------------- lexer

pub(super) fn is_w(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub(super) fn is_name_part(c: char) -> bool {
    is_w(c) || c == '-' || c == '.'
}

/// `Ca`: the token regex `Da` applied globally, whitespace tokens dropped.
pub(super) fn tokenize(s: &str) -> Vec<String> {
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
pub(super) fn has_word_start(s: &str) -> bool {
    s.chars().any(|c| is_w(c) && !c.is_ascii_digit())
}

// ---------------------------------------------------------------- AST

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BinOp {
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
    pub(super) fn from_token(t: &str) -> Option<BinOp> {
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
    pub(super) fn prec(self) -> u8 {
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
    pub(super) fn ty(self) -> u8 {
        if self.prec() >= 5 {
            1
        } else {
            2
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Func {
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
    pub(super) fn from_name(n: &str) -> Option<Func> {
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
    pub(super) fn spec(self) -> (u8, bool, usize, Option<usize>, bool) {
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
pub(super) enum Axis {
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
    pub(super) fn from_name(n: &str) -> Option<Axis> {
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
    pub(super) fn reverse(self) -> bool {
        matches!(
            self,
            Axis::Ancestor | Axis::AncestorOrSelf | Axis::Preceding | Axis::PrecedingSibling
        )
    }

    /// `I`: the axis may use the `[@attr]` shortcut.
    pub(super) fn shortcut(self) -> bool {
        matches!(
            self,
            Axis::Child
                | Axis::Descendant
                | Axis::DescendantOrSelf
                | Axis::Following
                | Axis::Preceding
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Comment,
    Text,
    Pi,
    Node,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Test {
    /// `F`: local name (or `*`) and namespace: an explicit (resolved)
    /// namespace, `*`, or `None` for "the document's default namespace".
    Name { local: String, ns: Option<String> },
    /// `G`: a kind test (its name kept: the attribute axis uses it).
    Kind { kind: Kind, name: String },
}

impl Test {
    /// `b()`: the local name or the kind's name.
    pub(super) fn name(&self) -> &str {
        match self {
            Test::Name { local, .. } => local,
            Test::Kind { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Step {
    pub(super) axis: Axis,
    pub(super) test: Test,
    pub(super) preds: Vec<Expr>,
    /// `w`: the step follows `//`.
    pub(super) desc: bool,
    /// `o`: the `[@name]` shortcut's attribute name.
    pub(super) shortcut: Option<String>,
    /// `g`: a predicate depends on position.
    pub(super) positional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Expr {
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
    pub(super) fn ty(&self) -> u8 {
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
    pub(super) fn positional(&self) -> bool {
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
    pub(super) fn attr_shortcut(&self) -> Option<String> {
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

pub(super) struct Parser {
    toks: Vec<String>,
    i: usize,
    /// The namespace resolver's map (prefix, URI).
    ns: Vec<(String, String)>,
}

impl Parser {
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
            axis =
                Axis::from_name(&t).ok_or_else(|| XPathError(format!("No axis with name: {t}")))?;
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

pub(super) fn is_kind_name(s: &str) -> bool {
    matches!(s, "comment" | "text" | "processing-instruction" | "node")
}

pub(super) fn kind_test(name: &str) -> Test {
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
pub(super) fn make_step(axis: Axis, test: Test, preds: Vec<Expr>, desc: bool) -> Step {
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
pub(super) fn make_call(f: Func, args: Vec<Expr>) -> R<Expr> {
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
                return err(format!(
                    "Argument {i} to function {f:?} is not of type Nodeset"
                ));
            }
        }
    }
    Ok(Expr::Call(f, args))
}

/// `new Ib(expr, resolver)`: parse.
pub(super) fn compile(expr: &str, ns: &[(&str, &str)]) -> R<Expr> {
    if expr.is_empty() {
        return err("Empty XPath expression.");
    }
    let toks = tokenize(expr);
    if toks.is_empty() {
        return err("Invalid XPath expression.");
    }
    let mut p = Parser {
        toks,
        i: 0,
        ns: ns
            .iter()
            .map(|(p, u)| ((*p).to_owned(), (*u).to_owned()))
            .collect(),
    };
    let e = p.expr()?;
    if !p.done() {
        return err(format!("Bad token: {}", p.next().unwrap_or_default()));
    }
    Ok(e)
}
