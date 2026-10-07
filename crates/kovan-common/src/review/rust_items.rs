//! Finding a function in one Rust source file with a real parser (`syn`),
//! and the normalised text a stamp's hash is taken over.
//!
//! Moved here from `kovan::review_stamps::parse` on 2026-10-07 (GitHub #764;
//! placement decided on #743: stamp types in kovan-common, which builds for
//! wasm). `kovan::review_stamps::parse` re-exports this module unchanged.
//! The one addition is [`FnEntry::code_without_name`], the code text with the
//! function's own name left out, which the #764 function hash
//! ([`super::hash`]) is taken over; [`stamp_hash`] (the 2026-10-06 `kovan`
//! stamp hash, name and doc included) is kept for `kovan`'s
//! `review/stamps.toml` until that file is retired.
//!
//! # Which functions exist
//!
//! Every `fn` item the parser sees at item level: free functions, methods in
//! `impl` blocks, and trait methods (with or without a default body), at any
//! depth of inline `mod name { ... }`. A function nested inside another
//! function's body is part of that function's code, not an item of its own,
//! so it cannot be stamped by itself. (`code-walk`'s text scanner does see
//! nested `fn`s; that is the one place the two differ.)
//!
//! # How a path names one
//!
//! Exactly as `kovan-cli code-walk` names it (`commands/code_walk/source.rs`
//! `FileIndex::find`): `name` matches every function so named in the file;
//! `Owner::name` matches a method whose `impl` self type, or whose trait,
//! has the last path segment `Owner` (generics and references dropped), or a
//! method declared in `trait Owner`. More than one match is an ambiguity, and
//! the caller is told to qualify the path.
//!
//! # The normalisation the hash is taken over
//!
//! A function's stamp text has two parts, built from the parsed item:
//!
//! - **Doc**: the item's own outer doc attributes, in order — every `///`
//!   line and `/** */` block (the compiler turns both into
//!   `#[doc = "..."]`), and any `#[doc = "literal"]` written out. The text of
//!   all of them is split into lines; a line that is blank after trimming is
//!   a paragraph break; within a paragraph the words are joined by single
//!   spaces. So re-wrapping or re-indenting a doc paragraph keeps the text,
//!   while changing, adding, removing or reordering a word, or adding or
//!   removing a paragraph break, changes it.
//! - **Code**: every other token of the item — its other attributes
//!   (`#[inline]`, `#[cfg(...)]`, a non-literal `#[doc = ...]`), visibility,
//!   signature and body — as the parser's token trees, each written as its
//!   text followed by one space: identifiers and literals as written
//!   (`1.0` and `1.` differ, as do `"a"` and `r"a"`), punctuation one
//!   character at a time with its joint/alone spacing dropped, and groups as
//!   their delimiter, contents, closing delimiter. Ordinary comments (`//`,
//!   `/* */`) never reach the token stream, and neither does any whitespace
//!   or line break, so rustfmt-style reformatting and comment edits leave the
//!   code text unchanged; changing any token changes it. Doc comments on
//!   items *nested inside the body* stay as `#[doc = "..."]` tokens of the
//!   code.
//!
//! Neither part depends on where the function sits in the file, so moving a
//! function keeps its stamp; the line numbers are reported separately.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, ImplItem, Item, Lit, Meta, TraitItem};

/// The block a function is declared in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Container {
    /// A free function, at file level or inside an inline `mod`.
    Free,
    /// Inside `impl Type` or `impl Trait for Type`.
    Impl {
        self_ty: String,
        trait_name: Option<String>,
    },
    /// Declared in `trait Name { ... }`.
    Trait { name: String },
}

/// One function found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnEntry {
    pub name: String,
    pub container: Container,
    /// 1-based inclusive line range, from the first outer attribute (doc
    /// comments included) to the closing brace or `;`.
    pub lines: [u32; 2],
    /// The normalised doc text (module doc above).
    pub doc: String,
    /// The normalised code text (module doc above).
    pub code: String,
    /// [`Self::code`] with the function's own name left out: the `fn`
    /// keyword is followed directly by the generics or the parameter list
    /// (GitHub #764, maintainer 2026-10-07: renaming alone must not change
    /// the hash). Only the declaration's name is dropped; the same
    /// identifier anywhere else (a recursive call, a nested item) stays.
    pub code_without_name: String,
    /// Inside a `#[cfg(test)]` module, or itself `#[test]` / `#[cfg(test)]`.
    pub is_test: bool,
    /// Inline modules enclosing the function, outermost first.
    pub inline_parents: Vec<String>,
}

impl FnEntry {
    /// `Type::name` for a method, `Trait::name` for a trait declaration,
    /// `name` for a free function (`code-walk`'s qualified name).
    pub fn qualname(&self) -> String {
        match &self.container {
            Container::Free => self.name.clone(),
            Container::Impl { self_ty, .. } => format!("{self_ty}::{}", self.name),
            Container::Trait { name } => format!("{name}::{}", self.name),
        }
    }
}

/// A `mod name;` declaration (no inline body) found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDecl {
    /// Inline modules enclosing the declaration, outermost first.
    pub inline_parents: Vec<String>,
    pub name: String,
    /// Under `#[cfg(test)]` (directly or through an enclosing module).
    pub is_test: bool,
    /// Carries a `#[path = ...]` attribute, which this module does not follow.
    pub has_path_attr: bool,
}

/// Everything parsed out of one file.
#[derive(Debug, Clone, Default)]
pub struct ParsedFile {
    pub fns: Vec<FnEntry>,
    pub mods: Vec<ModDecl>,
    /// Every inline `mod name { ... }`, as its full inline path.
    pub inline_mods: Vec<Vec<String>>,
}

/// Why a path did not name exactly one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateError {
    /// The file is not valid Rust as far as `syn` can tell.
    Parse(String),
    NotFound,
    /// Several functions match; their qualified names and first lines.
    Ambiguous(Vec<String>),
}

impl std::fmt::Display for LocateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocateError::Parse(e) => write!(f, "the file does not parse: {e}"),
            LocateError::NotFound => write!(f, "function not found"),
            LocateError::Ambiguous(m) => {
                write!(
                    f,
                    "the path is ambiguous ({}); qualify it as Type::name",
                    m.join(", ")
                )
            }
        }
    }
}

/// Parse `text` and list its functions and `mod name;` declarations.
pub fn parse_file(text: &str) -> Result<ParsedFile, String> {
    let file = syn::parse_file(text).map_err(|e| {
        let at = e.span().start();
        format!("line {}: {e}", at.line)
    })?;
    let mut out = ParsedFile::default();
    walk_items(&file.items, &mut Vec::new(), false, &mut out);
    // proc-macro2 keeps every parsed source in a per-thread map so spans can
    // report lines; nothing above holds a span any more, so free it (a
    // long-running GUI would otherwise keep every file it ever parsed).
    proc_macro2::extra::invalidate_current_thread_spans();
    Ok(out)
}

/// The one function `qual` (`name` or `Owner::name`) names in `text`.
pub fn locate(text: &str, qual: &str) -> Result<FnEntry, LocateError> {
    let parsed = parse_file(text).map_err(LocateError::Parse)?;
    let mut found: Vec<FnEntry> = parsed
        .fns
        .into_iter()
        .filter(|f| matches(f, qual))
        .collect();
    match found.len() {
        0 => Err(LocateError::NotFound),
        1 => Ok(found.remove(0)),
        _ => Err(LocateError::Ambiguous(
            found
                .iter()
                .map(|f| format!("{} (line {})", f.qualname(), f.lines[0]))
                .collect(),
        )),
    }
}

/// `code-walk`'s matching rule (module doc).
pub fn matches(f: &FnEntry, qual: &str) -> bool {
    let mut parts = qual.rsplitn(2, "::");
    let name = parts.next().unwrap_or(qual);
    if f.name != name {
        return false;
    }
    match parts.next() {
        None => true,
        Some(owner) => match &f.container {
            Container::Free => false,
            Container::Impl {
                self_ty,
                trait_name,
            } => self_ty == owner || trait_name.as_deref() == Some(owner),
            Container::Trait { name } => name == owner,
        },
    }
}

/// The stamp hash of a function's normalised doc and code text:
/// `sha256:` and the lowercase hex SHA-256 of
/// `"kovan-review-stamp-v1\ndoc\n" + doc + "\ncode\n" + code`.
pub fn stamp_hash(doc: &str, code: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"kovan-review-stamp-v1\ndoc\n");
    h.update(doc.as_bytes());
    h.update(b"\ncode\n");
    h.update(code.as_bytes());
    let digest = h.finalize();
    let mut s = String::from("sha256:");
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn walk_items(items: &[Item], inline: &mut Vec<String>, in_test: bool, out: &mut ParsedFile) {
    for item in items {
        match item {
            Item::Fn(f) => {
                let is_test = in_test || is_test_attr(&f.attrs);
                let mut bare = f.clone();
                let doc = take_docs(&mut bare.attrs);
                out.fns.push(entry(
                    f.sig.ident.to_string(),
                    Container::Free,
                    f.span(),
                    doc,
                    bare.to_token_stream(),
                    is_test,
                    inline,
                ));
            }
            Item::Impl(imp) => {
                let in_test = in_test || is_cfg_test(&imp.attrs);
                let container = Container::Impl {
                    self_ty: type_head(&imp.self_ty),
                    trait_name: imp
                        .trait_
                        .as_ref()
                        .and_then(|(_, path, _)| path.segments.last())
                        .map(|s| s.ident.to_string()),
                };
                for ii in &imp.items {
                    if let ImplItem::Fn(f) = ii {
                        let is_test = in_test || is_test_attr(&f.attrs);
                        let mut bare = f.clone();
                        let doc = take_docs(&mut bare.attrs);
                        out.fns.push(entry(
                            f.sig.ident.to_string(),
                            container.clone(),
                            f.span(),
                            doc,
                            bare.to_token_stream(),
                            is_test,
                            inline,
                        ));
                    }
                }
            }
            Item::Trait(t) => {
                let in_test = in_test || is_cfg_test(&t.attrs);
                let container = Container::Trait {
                    name: t.ident.to_string(),
                };
                for ti in &t.items {
                    if let TraitItem::Fn(f) = ti {
                        let mut bare = f.clone();
                        let doc = take_docs(&mut bare.attrs);
                        out.fns.push(entry(
                            f.sig.ident.to_string(),
                            container.clone(),
                            f.span(),
                            doc,
                            bare.to_token_stream(),
                            in_test || is_test_attr(&f.attrs),
                            inline,
                        ));
                    }
                }
            }
            Item::Mod(m) => {
                let is_test = in_test || is_cfg_test(&m.attrs);
                match &m.content {
                    Some((_, items)) => {
                        inline.push(m.ident.to_string());
                        out.inline_mods.push(inline.clone());
                        walk_items(items, inline, is_test, out);
                        inline.pop();
                    }
                    None => out.mods.push(ModDecl {
                        inline_parents: inline.clone(),
                        name: m.ident.to_string(),
                        is_test,
                        has_path_attr: m.attrs.iter().any(|a| a.path().is_ident("path")),
                    }),
                }
            }
            _ => {}
        }
    }
}

fn entry(
    name: String,
    container: Container,
    span: proc_macro2::Span,
    doc: Vec<String>,
    code: TokenStream,
    is_test: bool,
    inline: &[String],
) -> FnEntry {
    let mut code_text = String::new();
    push_tokens(code.clone(), &mut code_text);
    let mut anon = String::new();
    push_tokens_without_name(code, &name, &mut anon);
    FnEntry {
        code_without_name: anon.trim_end().to_string(),
        name,
        container,
        lines: [span.start().line as u32, span.end().line as u32],
        doc: normalise_doc(&doc),
        code: code_text.trim_end().to_string(),
        is_test,
        inline_parents: inline.to_vec(),
    }
}

/// Removes the literal doc attributes from `attrs` and returns their text.
fn take_docs(attrs: &mut Vec<Attribute>) -> Vec<String> {
    let mut docs = Vec::new();
    attrs.retain(|a| {
        if let Meta::NameValue(nv) = &a.meta {
            if nv.path.is_ident("doc") {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &nv.value
                {
                    docs.push(s.value());
                    return false;
                }
            }
        }
        true
    });
    docs
}

/// Paragraphs of single-spaced words, separated by a blank line.
pub fn normalise_doc(docs: &[String]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for d in docs {
        for line in d.split('\n') {
            if line.trim().is_empty() {
                if !current.is_empty() {
                    paragraphs.push(current.join(" "));
                    current.clear();
                }
            } else {
                current.extend(line.split_whitespace());
            }
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }
    paragraphs.join("\n\n")
}

/// [`push_tokens`] at the item's top level, dropping the identifier that
/// directly follows the first top-level `fn` keyword when it is `name`.
/// The signature is top-level in an item's token stream; the body is a
/// brace group, so nothing inside it is touched.
fn push_tokens_without_name(ts: TokenStream, name: &str, out: &mut String) {
    let mut after_fn = false;
    let mut dropped = false;
    for tt in ts {
        if !dropped && after_fn {
            if let TokenTree::Ident(i) = &tt {
                if i == name {
                    dropped = true;
                    after_fn = false;
                    continue;
                }
            }
        }
        after_fn = matches!(&tt, TokenTree::Ident(i) if i == "fn");
        push_tokens(std::iter::once(tt).collect(), out);
    }
}

fn push_tokens(ts: TokenStream, out: &mut String) {
    for tt in ts {
        match tt {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                if !open.is_empty() {
                    out.push_str(open);
                    out.push(' ');
                }
                push_tokens(g.stream(), out);
                if !close.is_empty() {
                    out.push_str(close);
                    out.push(' ');
                }
            }
            TokenTree::Ident(i) => {
                out.push_str(&i.to_string());
                out.push(' ');
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                out.push(' ');
            }
            TokenTree::Literal(l) => {
                out.push_str(&l.to_string());
                out.push(' ');
            }
        }
    }
}

fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && match &a.meta {
                Meta::List(l) => l.tokens.to_string().trim() == "test",
                _ => false,
            }
    })
}

fn is_test_attr(attrs: &[Attribute]) -> bool {
    is_cfg_test(attrs) || attrs.iter().any(|a| a.path().is_ident("test"))
}

/// The last path segment of a type, without generics or references:
/// `&mut crate::geom::Sphere<T>` -> `Sphere` (as `code-walk`'s `type_head`).
fn type_head(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        syn::Type::Reference(r) => type_head(&r.elem),
        syn::Type::Paren(p) => type_head(&p.elem),
        syn::Type::Group(g) => type_head(&g.elem),
        other => {
            let mut s = String::new();
            push_tokens(other.to_token_stream(), &mut s);
            s.trim_end().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
//! crate doc
use std::fmt;

/// Adds one.
///
/// Second paragraph.
pub fn add_one(x: f64) -> f64 {
    x + 1.0 // trailing comment
}

pub struct Sphere<T> { r: T }

impl<T: Copy> Sphere<T> {
    /// Radius.
    #[inline]
    pub fn radius(&self) -> T { self.r }
}

impl fmt::Display for Sphere<f64> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.r) }
}

trait Shape {
    /// Area.
    fn area(&self) -> f64;
    fn name(&self) -> &str { "shape" }
}

mod inner {
    pub fn add_one() {}
    mod deeper;
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
"#;

    /// Methodology: every item kind the walker handles, the code-walk path
    /// rule (bare, `Type::`, `Trait::`), ambiguity, test detection, line
    /// numbers including the doc comment, and `mod name;` declarations.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn finds_functions_by_code_walk_path() {
        let p = parse_file(SRC).unwrap();
        let names: Vec<String> = p.fns.iter().map(|f| f.qualname()).collect();
        assert_eq!(
            names,
            [
                "add_one",
                "Sphere::radius",
                "Sphere::fmt",
                "Shape::area",
                "Shape::name",
                "add_one",
                "t"
            ]
        );
        let add = &p.fns[0];
        assert_eq!(add.lines, [5, 10]);
        assert_eq!(add.doc, "Adds one.\n\nSecond paragraph.");
        assert_eq!(add.code, "pub fn add_one ( x : f64 ) - > f64 { x + 1.0 }");
        assert!(p.fns[6].is_test && !p.fns[0].is_test);
        assert!(matches!(
            locate(SRC, "add_one"),
            Err(LocateError::Ambiguous(_))
        ));
        assert_eq!(
            locate(SRC, "Sphere::radius").unwrap().code,
            "# [ inline ] pub fn radius ( & self ) - > T { self . r }"
        );
        assert_eq!(locate(SRC, "Display::fmt").unwrap().name, "fmt");
        assert_eq!(locate(SRC, "Shape::area").unwrap().doc, "Area.");
        assert_eq!(locate(SRC, "missing"), Err(LocateError::NotFound));
        assert!(matches!(locate("fn (", "x"), Err(LocateError::Parse(_))));
        assert_eq!(
            p.mods,
            vec![ModDecl {
                inline_parents: vec!["inner".into()],
                name: "deeper".into(),
                is_test: false,
                has_path_attr: false
            },]
        );
    }

    /// Methodology: the normalisation's two promises, on one function —
    /// rustfmt-style reformatting, `//` and `/* */` comment edits, and doc
    /// re-wrapping keep the hash; a changed token, a changed doc word and a
    /// new doc paragraph break each change it.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn hash_ignores_layout_and_comments_but_not_tokens_or_docs() {
        let h = |src: &str| {
            let f = locate(src, "f").unwrap();
            stamp_hash(&f.doc, &f.code)
        };
        let base = h("/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }");
        let same = [
            "/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 {\n    let y = x * 2.0;\n    y.round()\n}\n",
            "/// Returns twice x,\n/// rounded.\npub fn f(x:f64)->f64{let y=x*2.0;y.round()}",
            "/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 {\n    // why: doubling\n    let y = x /* inline */ * 2.0;\n    y.round() // done\n}",
            "///   Returns twice x, rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
            "/** Returns twice x,\n rounded. */\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
            "// a plain comment above\n/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
        ];
        for s in same {
            assert_eq!(h(s), base, "should keep the hash:\n{s}");
        }
        let different = [
            "/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 3.0; y.round() }",
            "/// Returns twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.; y.round() }",
            "/// Returns twice x,\n/// rounded.\nfn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
            "/// Returns twice x,\n/// rounded down.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
            "/// Returns twice x,\n///\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
            "/// Returns twice x,\n/// rounded.\n#[inline]\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
        ];
        for s in different {
            assert_ne!(h(s), base, "should change the hash:\n{s}");
        }
        assert!(base.starts_with("sha256:") && base.len() == 7 + 64);
    }

    /// Methodology: the parser must read real workspace code, not only
    /// fixtures — every `.rs` file under this crate's `src/` must parse.
    /// (Since the move to kovan-common, 2026-10-07, "this crate" is
    /// kovan-common, so the floor is 40 files rather than kovan's 100.)
    ///
    /// Result (2026-10-06, in kovan): passes. Result (2026-10-07, in
    /// kovan-common): passes.
    #[test]
    fn every_source_file_of_this_crate_parses() {
        fn visit(dir: &std::path::Path, n: &mut usize) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    visit(&p, n);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    parse_file(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                    *n += 1;
                }
            }
        }
        let mut n = 0;
        visit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut n,
        );
        assert!(n > 40, "only {n} files");
    }
}
