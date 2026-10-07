// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: wicked-good-xpath 1.3.1-z002 (MIT; Copyright (c) 2007 Cybozu
//   Labs, Inc., Copyright (c) 2012 Google Inc.), dist/wgxpath.install-node.js:
//   evaluation (`T.a` steps, `hb.a` paths, `ab` predicates, `P`
//   comparisons, `D`/`La`/`Na` descendant search, the axes, the core
//   functions, `A` string values). See [`super`].
// Licence: AGPL-3.0 (this port; MIT permits it).

//! XPath evaluation over an [`XmlDocument`], as wicked-good-xpath does it.

use super::parse::{kind_test, Axis, BinOp, Expr, Func, Kind, Step, Test};
use super::{err, R};
use crate::zotero::framework::js;
use crate::zotero::framework::xml::{node_type, NodeId, NodeKind, XNode, XmlDocument};
use std::cmp::Ordering;

// ---------------------------------------------------------------- values

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Set(Vec<XNode>),
}

/// The evaluation context `n`: node, position, size.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ctx {
    node: XNode,
    pos: usize,
    size: usize,
}

impl Ctx {
    pub(super) fn of(node: XNode) -> Ctx {
        Ctx {
            node,
            pos: 1,
            size: 1,
        }
    }
}

pub(super) struct Eval {
    /// `ownerDocument.lookupNamespaceURI(null)`.
    pub(super) default_ns: Option<String>,
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
    for (p, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(h) = t.strip_prefix(p) {
            if !h.is_empty() && h.chars().all(|c| c.is_digit(radix)) {
                return h.chars().fold(0.0, |a, c| {
                    a * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64
                });
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
        return if x > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
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
pub(super) fn js_round(x: f64) -> f64 {
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

impl Eval {
    /// `A`: the string value of a node.
    pub(super) fn string_value(&self, doc: &XmlDocument, x: XNode) -> String {
        let d = doc;
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
                    self.walk_values(doc, s, &mut out);
                    sib = d.next_sibling(s);
                }
                out
            }
            _ => d.node_value(x).unwrap_or_else(|| "null".into()),
        }
    }

    pub(super) fn walk_values(&self, doc: &XmlDocument, n: NodeId, out: &mut String) {
        if !doc.is_element(n) {
            if let Some(v) = doc.node_value(XNode::Node(n)) {
                out.push_str(&v);
            } else {
                out.push_str("null");
            }
        }
        for &c in doc.children(n) {
            self.walk_values(doc, c, out);
        }
    }

    pub(super) fn first_string(&self, doc: &XmlDocument, set: &[XNode]) -> String {
        set.first()
            .map(|&x| self.string_value(doc, x))
            .unwrap_or_default()
    }

    /// `M`.
    pub(super) fn to_str(&self, doc: &XmlDocument, v: &Value) -> String {
        match v {
            Value::Set(s) => self.first_string(doc, s),
            Value::Str(s) => s.clone(),
            Value::Num(n) => js_number_to_string(*n),
            Value::Bool(b) => b.to_string(),
        }
    }

    /// `L`.
    pub(super) fn to_num(&self, doc: &XmlDocument, v: &Value) -> f64 {
        match v {
            Value::Set(s) => js_to_number(&self.first_string(doc, s)),
            Value::Str(s) => js_to_number(s),
            Value::Num(n) => *n,
            Value::Bool(b) => f64::from(u8::from(*b)),
        }
    }

    /// `N`.
    pub(super) fn to_bool(v: &Value) -> bool {
        match v {
            Value::Set(s) => !s.is_empty(),
            Value::Str(s) => !s.is_empty(),
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Bool(b) => *b,
        }
    }

    /// `F.a`: the name test.
    pub(super) fn name_matches(
        &self,
        doc: &XmlDocument,
        x: XNode,
        local: &str,
        ns: &Option<String>,
    ) -> bool {
        let t = doc.node_type(x);
        if t != node_type::ELEMENT && t != node_type::ATTRIBUTE {
            return false;
        }
        let name = doc.local_name(x).unwrap_or_default();
        if local != "*" && local != name {
            return false;
        }
        match ns.as_deref() {
            Some("*") => true,
            Some(u) => doc.namespace_uri(x) == Some(u),
            None => doc.namespace_uri(x) == self.default_ns.as_deref(),
        }
    }

    pub(super) fn test(&self, doc: &XmlDocument, t: &Test, x: XNode) -> bool {
        match t {
            Test::Name { local, ns } => self.name_matches(doc, x, local, ns),
            Test::Kind { kind, .. } => {
                let nt = doc.node_type(x);
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
    pub(super) fn has_attr(&self, doc: &XmlDocument, x: XNode, attr: Option<&str>) -> bool {
        let Some(name) = attr else {
            return true;
        };
        match x {
            XNode::Node(n) if doc.is_element(n) => {
                doc.get_attribute(n, name).is_some_and(|v| !v.is_empty())
            }
            _ => false,
        }
    }

    /// `D` (`La`, `Na`): descendants of `x` matching the test.
    pub(super) fn descendants(
        &self,
        doc: &XmlDocument,
        t: &Test,
        x: XNode,
        attr: Option<&str>,
        out: &mut Vec<XNode>,
    ) {
        let XNode::Node(n) = x else {
            return;
        };
        match t {
            Test::Kind { .. } => {
                for d in doc.descendants(n) {
                    let dx = XNode::Node(d);
                    if self.has_attr(doc, dx, attr) && self.test(doc, t, dx) {
                        out.push(dx);
                    }
                }
            }
            Test::Name { local, ns } => {
                // Only Document and Element have getElementsByTagName(NS).
                if !matches!(doc.kind(n), NodeKind::Document | NodeKind::Element(_)) {
                    return;
                }
                let found = match ns.as_deref() {
                    Some(u) if u != "*" => doc.get_elements_by_tag_name_ns(n, Some(u), local),
                    _ => doc.get_elements_by_tag_name(n, local),
                };
                for e in found {
                    let ex = XNode::Node(e);
                    if self.test(doc, t, ex) && self.has_attr(doc, ex, attr) {
                        out.push(ex);
                    }
                }
            }
        }
    }

    /// An axis applied to one node (`U(...)` functions).
    pub(super) fn axis(
        &self,
        doc: &XmlDocument,
        axis: Axis,
        t: &Test,
        x: XNode,
        attr: Option<&str>,
    ) -> Vec<XNode> {
        let d = doc;
        let mut out = Vec::new();
        match axis {
            Axis::Ancestor | Axis::AncestorOrSelf => {
                if axis == Axis::AncestorOrSelf && self.test(doc, t, x) {
                    out.push(x);
                }
                // An Attr has no parentNode in jsdom.
                let mut cur = x.as_node().and_then(|n| d.parent(n));
                while let Some(p) = cur {
                    if self.test(doc, t, XNode::Node(p)) {
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
                let all = matches!(
                    t,
                    Test::Kind {
                        kind: Kind::Node,
                        ..
                    }
                ) || name == "*";
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
                        if self.has_attr(doc, cx, attr) && self.test(doc, t, cx) {
                            out.push(cx);
                        }
                    }
                }
            }
            Axis::Descendant => self.descendants(doc, t, x, attr, &mut out),
            Axis::DescendantOrSelf => {
                if self.has_attr(doc, x, attr) && self.test(doc, t, x) {
                    out.push(x);
                }
                self.descendants(doc, t, x, attr, &mut out);
            }
            Axis::Following => {
                let mut cur = x.as_node();
                while let Some(b) = cur {
                    let mut f = d.next_sibling(b);
                    while let Some(s) = f {
                        let sx = XNode::Node(s);
                        if self.has_attr(doc, sx, attr) && self.test(doc, t, sx) {
                            out.push(sx);
                        }
                        self.descendants(doc, t, sx, attr, &mut out);
                        f = d.next_sibling(s);
                    }
                    cur = d.parent(b);
                }
                sort_dedup(d, &mut out);
            }
            Axis::FollowingSibling => {
                let mut f = x.as_node().and_then(|n| d.next_sibling(n));
                while let Some(s) = f {
                    if self.test(doc, t, XNode::Node(s)) {
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
                        if self.test(doc, t, XNode::Node(p)) {
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
                        if self.has_attr(doc, sx, attr) && self.test(doc, t, sx) {
                            out.push(sx);
                        }
                        self.descendants(doc, t, sx, attr, &mut out);
                    }
                }
                sort_dedup(d, &mut out);
            }
            Axis::PrecedingSibling => {
                let mut p = x.as_node().and_then(|n| d.previous_sibling(n));
                while let Some(s) = p {
                    if self.test(doc, t, XNode::Node(s)) {
                        out.push(XNode::Node(s));
                    }
                    p = d.previous_sibling(s);
                }
                out.reverse();
            }
            Axis::SelfAxis => {
                if self.test(doc, t, x) {
                    out.push(x);
                }
            }
        }
        out
    }

    /// `ab`: apply predicates from index `from`.
    pub(super) fn filter(
        &self,
        doc: &XmlDocument,
        preds: &[Expr],
        mut set: Vec<XNode>,
        from: usize,
        reverse: bool,
    ) -> R<Vec<XNode>> {
        for p in preds.iter().skip(from) {
            let size = set.len();
            let mut kept = Vec::with_capacity(size);
            for (k, &x) in set.iter().enumerate() {
                let pos = if reverse { size - k } else { k + 1 };
                let v = self.eval(doc, p, Ctx { node: x, pos, size })?;
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
    pub(super) fn step_from(&self, doc: &XmlDocument, s: &Step, x: XNode) -> R<Vec<XNode>> {
        let from = usize::from(s.shortcut.is_some());
        let set = self.axis(doc, s.axis, &s.test, x, s.shortcut.as_deref());
        self.filter(doc, &s.preds, set, from, s.axis.reverse())
    }

    /// `T.prototype.a`: a step.
    pub(super) fn step(&self, doc: &XmlDocument, s: &Step, ctx: Ctx) -> R<Vec<XNode>> {
        if !s.desc {
            return self.step_from(doc, s, ctx.node);
        }
        if s.positional || s.axis != Axis::Child {
            let all = self.axis(
                doc,
                Axis::DescendantOrSelf,
                &kind_test("node"),
                ctx.node,
                None,
            );
            let mut out = Vec::new();
            for x in all {
                out.extend(self.step_from(doc, s, x)?);
            }
            sort_dedup(doc, &mut out);
            Ok(out)
        } else {
            let mut set = Vec::new();
            self.descendants(doc, &s.test, ctx.node, s.shortcut.as_deref(), &mut set);
            let from = usize::from(s.shortcut.is_some());
            self.filter(doc, &s.preds, set, from, false)
        }
    }

    pub(super) fn eval(&self, doc: &XmlDocument, e: &Expr, ctx: Ctx) -> R<Value> {
        Ok(match e {
            Expr::Literal(s) => Value::Str(s.clone()),
            Expr::Number(n) => Value::Num(*n),
            Expr::Root => Value::Set(vec![XNode::Node(doc.document())]),
            Expr::Context => Value::Set(vec![ctx.node]),
            Expr::Neg(a) => Value::Num(-self.num_of(doc, a, ctx)?),
            Expr::Union(v) => {
                let mut out = Vec::new();
                for x in v {
                    match self.eval(doc, x, ctx)? {
                        Value::Set(s) => out.extend(s),
                        _ => return err("Path expression must evaluate to NodeSet."),
                    }
                }
                sort_dedup(doc, &mut out);
                Value::Set(out)
            }
            Expr::Filter(p, preds) => match self.eval(doc, p, ctx)? {
                Value::Set(s) => Value::Set(self.filter(doc, preds, s, 0, false)?),
                other => other,
            },
            Expr::Path(start, steps) => {
                let Value::Set(mut set) = self.eval(doc, start, ctx)? else {
                    return err("Filter expression must evaluate to nodeset.");
                };
                for s in steps {
                    if set.is_empty() {
                        break;
                    }
                    let mut next = Vec::new();
                    for &x in &set {
                        next.extend(self.step(doc, s, Ctx::of(x))?);
                    }
                    sort_dedup(doc, &mut next);
                    set = next;
                }
                Value::Set(set)
            }
            Expr::Binary(op, a, b) => self.binary(doc, *op, a, b, ctx)?,
            Expr::Call(f, args) => self.call(doc, *f, args, ctx)?,
        })
    }

    pub(super) fn num_of(&self, doc: &XmlDocument, e: &Expr, ctx: Ctx) -> R<f64> {
        Ok(self.to_num(doc, &self.eval(doc, e, ctx)?))
    }

    pub(super) fn str_of(&self, doc: &XmlDocument, e: &Expr, ctx: Ctx) -> R<String> {
        Ok(self.to_str(doc, &self.eval(doc, e, ctx)?))
    }

    pub(super) fn binary(
        &self,
        doc: &XmlDocument,
        op: BinOp,
        a: &Expr,
        b: &Expr,
        ctx: Ctx,
    ) -> R<Value> {
        Ok(match op {
            BinOp::Div => Value::Num(self.num_of(doc, a, ctx)? / self.num_of(doc, b, ctx)?),
            BinOp::Mod => Value::Num(self.num_of(doc, a, ctx)? % self.num_of(doc, b, ctx)?),
            BinOp::Mul => Value::Num(self.num_of(doc, a, ctx)? * self.num_of(doc, b, ctx)?),
            BinOp::Add => Value::Num(self.num_of(doc, a, ctx)? + self.num_of(doc, b, ctx)?),
            BinOp::Sub => Value::Num(self.num_of(doc, a, ctx)? - self.num_of(doc, b, ctx)?),
            BinOp::And => Value::Bool(
                Self::to_bool(&self.eval(doc, a, ctx)?) && Self::to_bool(&self.eval(doc, b, ctx)?),
            ),
            BinOp::Or => Value::Bool(
                Self::to_bool(&self.eval(doc, a, ctx)?) || Self::to_bool(&self.eval(doc, b, ctx)?),
            ),
            _ => {
                let l = self.eval(doc, a, ctx)?;
                let r = self.eval(doc, b, ctx)?;
                Value::Bool(self.compare(doc, op, &l, &r))
            }
        })
    }

    /// `P`.
    pub(super) fn compare(&self, doc: &XmlDocument, op: BinOp, b: &Value, c: &Value) -> bool {
        let is_eq = matches!(op, BinOp::Eq | BinOp::Ne);
        match (b, c) {
            (Value::Set(x), Value::Set(y)) => x.iter().any(|&d| {
                let ds = Prim::Str(self.string_value(doc, d));
                y.iter()
                    .any(|&f| prim_cmp(op, &ds, &Prim::Str(self.string_value(doc, f))))
            }),
            (Value::Set(set), other) | (other, Value::Set(set)) => {
                let set_left = matches!(b, Value::Set(_));
                let prim = to_prim(other);
                set.iter().any(|&k| {
                    let s = self.string_value(doc, k);
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
                        prim_cmp(
                            op,
                            &Prim::Bool(Self::to_bool(b)),
                            &Prim::Bool(Self::to_bool(c)),
                        )
                    } else if matches!(pb, Prim::Num(_)) || matches!(pc, Prim::Num(_)) {
                        prim_cmp(
                            op,
                            &Prim::Num(self.to_num(doc, b)),
                            &Prim::Num(self.to_num(doc, c)),
                        )
                    } else {
                        prim_cmp(op, &pb, &pc)
                    }
                } else {
                    prim_cmp(
                        op,
                        &Prim::Num(self.to_num(doc, b)),
                        &Prim::Num(self.to_num(doc, c)),
                    )
                }
            }
        }
    }

    pub(super) fn opt_node(&self, doc: &XmlDocument, args: &[Expr], ctx: Ctx) -> R<Option<XNode>> {
        Ok(match args.first() {
            None => Some(ctx.node),
            Some(a) => match self.eval(doc, a, ctx)? {
                Value::Set(s) => s.first().copied(),
                _ => None,
            },
        })
    }

    pub(super) fn opt_string(&self, doc: &XmlDocument, args: &[Expr], ctx: Ctx) -> R<String> {
        Ok(match args.first() {
            Some(a) => self.str_of(doc, a, ctx)?,
            None => self.string_value(doc, ctx.node),
        })
    }

    pub(super) fn call(&self, doc: &XmlDocument, f: Func, args: &[Expr], ctx: Ctx) -> R<Value> {
        let d = doc;
        Ok(match f {
            Func::Boolean => Value::Bool(Self::to_bool(&self.eval(doc, &args[0], ctx)?)),
            Func::Ceiling => Value::Num(self.num_of(doc, &args[0], ctx)?.ceil()),
            Func::Concat => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&self.str_of(doc, a, ctx)?);
                }
                Value::Str(s)
            }
            Func::Contains => {
                let (a, b) = (
                    self.str_of(doc, &args[0], ctx)?,
                    self.str_of(doc, &args[1], ctx)?,
                );
                Value::Bool(a.contains(&b))
            }
            Func::Count => match self.eval(doc, &args[0], ctx)? {
                Value::Set(s) => Value::Num(s.len() as f64),
                _ => Value::Num(0.0),
            },
            Func::False => Value::Bool(false),
            Func::Floor => Value::Num(self.num_of(doc, &args[0], ctx)?.floor()),
            Func::Id => {
                let ids = self.str_of(doc, &args[0], ctx)?;
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
            Func::LocalName => Value::Str(match self.opt_node(doc, args, ctx)? {
                Some(x) => d
                    .local_name(x)
                    .map(str::to_owned)
                    .unwrap_or_else(|| d.node_name(x)),
                None => String::new(),
            }),
            Func::Name => Value::Str(match self.opt_node(doc, args, ctx)? {
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
                let s = self.opt_string(doc, args, ctx)?;
                Value::Str(crate::zotero::framework::utilities::trim_internal(&s))
            }
            Func::Not => Value::Bool(!Self::to_bool(&self.eval(doc, &args[0], ctx)?)),
            Func::Number => Value::Num(match args.first() {
                Some(a) => self.num_of(doc, a, ctx)?,
                None => js_to_number(&self.string_value(doc, ctx.node)),
            }),
            Func::Position => Value::Num(ctx.pos as f64),
            Func::Round => Value::Num(js_round(self.num_of(doc, &args[0], ctx)?)),
            Func::StartsWith => {
                let (a, b) = (
                    self.str_of(doc, &args[0], ctx)?,
                    self.str_of(doc, &args[1], ctx)?,
                );
                Value::Bool(a.starts_with(&b))
            }
            Func::String => Value::Str(self.opt_string(doc, args, ctx)?),
            Func::StringLength => Value::Num(js::len(&self.opt_string(doc, args, ctx)?) as f64),
            Func::Substring => {
                let s = self.str_of(doc, &args[0], ctx)?;
                let c = self.num_of(doc, &args[1], ctx)?;
                if c.is_nan() || c.is_infinite() {
                    return Ok(Value::Str(String::new()));
                }
                let len = match args.get(2) {
                    Some(a) => self.num_of(doc, a, ctx)?,
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
                let (a, b) = (
                    self.str_of(doc, &args[0], ctx)?,
                    self.str_of(doc, &args[1], ctx)?,
                );
                Value::Str(match a.find(&b) {
                    Some(i) => a[i + b.len()..].to_owned(),
                    None => String::new(),
                })
            }
            Func::SubstringBefore => {
                let (a, b) = (
                    self.str_of(doc, &args[0], ctx)?,
                    self.str_of(doc, &args[1], ctx)?,
                );
                Value::Str(match a.find(&b) {
                    Some(i) => a[..i].to_owned(),
                    None => String::new(),
                })
            }
            Func::Sum => match self.eval(doc, &args[0], ctx)? {
                Value::Set(s) => Value::Num(
                    s.iter()
                        .map(|&x| js_to_number(&self.string_value(doc, x)))
                        .sum(),
                ),
                _ => Value::Num(0.0),
            },
            Func::Translate => {
                let s = self.str_of(doc, &args[0], ctx)?;
                let from: Vec<char> = self.str_of(doc, &args[1], ctx)?.chars().collect();
                let to: Vec<char> = self.str_of(doc, &args[2], ctx)?.chars().collect();
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
pub(super) enum Prim {
    Num(f64),
    Str(String),
    Bool(bool),
}

pub(super) fn to_prim(v: &Value) -> Prim {
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
pub(super) fn prim_cmp(op: BinOp, a: &Prim, b: &Prim) -> bool {
    let ord: Option<Ordering> = match (a, b) {
        (Prim::Num(x), Prim::Num(y)) => x.partial_cmp(y),
        (Prim::Str(x), Prim::Str(y)) => {
            if matches!(op, BinOp::Eq | BinOp::Ne) {
                Some(if x == y {
                    Ordering::Equal
                } else {
                    Ordering::Less
                })
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

pub(super) fn sort_dedup(doc: &XmlDocument, v: &mut Vec<XNode>) {
    v.sort_by(|a, b| doc.compare_order(*a, *b));
    v.dedup();
}
