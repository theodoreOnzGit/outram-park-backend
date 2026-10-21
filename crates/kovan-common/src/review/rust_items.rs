//! Finding a function in one Rust source file with a real parser (`syn`),
//! and the normalised text a stamp's hash is taken over.
//!
//! Moved here from `kovan::review_stamps::parse` on 2026-10-07 (GitHub #764;
//! placement decided on #743: stamp types in kovan-common, which builds for
//! wasm). The one addition is [`FnEntry::code_without_name`], the code text
//! with the function's own name left out, which the #764 function hash
//! ([`super::hash`]) is taken over. ~~`kovan::review_stamps::parse`
//! re-exports this module unchanged; `stamp_hash` (the 2026-10-06 `kovan`
//! stamp hash, name and doc included) is kept for `kovan`'s
//! `review/stamps.toml` until that file is retired.~~ **CORRECTED
//! 2026-10-10** (#825): that file, `kovan::review_stamps` and `stamp_hash`
//! were removed; the normalisation is tested through [`super::hash`].
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
//! # Which other items exist (GitHub #843, 2026-10-13)
//!
//! Besides functions the parser lists, in [`ParsedFile::items`], every
//! `struct`, `enum`, `union`, `trait`, `type` alias, `const` and `static` at
//! item level (any depth of inline `mod`), each as an [`FnEntry`] whose
//! [`FnEntry::shape`] says which it is. Left out: `const _` (an unnamed
//! compile-time check), and constants and types declared inside an `impl` or
//! a `trait` (they are part of that block). A trait is listed once as an
//! item and its methods are still listed as functions.
//!
//! An item's code text is its whole definition. For a struct, enum or union
//! the name-free text the hash is taken over also carries the name-free code
//! of **every method written for it in the same file** (inherent and trait
//! `impl` blocks whose self type has that name), sorted, so that changing,
//! adding or removing a method changes the type's hash (maintainer, #843,
//! 2026-10-13: "changes in definitions and methods make structs and enums
//! stamp stale"). An `impl` in another file is not folded in: the parser
//! reads one file at a time.
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

use super::index::{ItemKind, ItemShape};

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
    /// 1-based line the item's own name is written on.
    pub name_line: u32,
    /// `None` for a function; which kind of item otherwise (module doc,
    /// "Which other items exist").
    pub shape: Option<ItemShape>,
}

impl FnEntry {
    /// The index's item kind: a function, a constant, a static or a type.
    pub fn item(&self) -> ItemKind {
        self.shape.map_or(ItemKind::Fn, ItemShape::item)
    }

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
    /// Every other reviewable item (module doc, "Which other items exist").
    pub items: Vec<FnEntry>,
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
    fold_methods(&mut out);
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

fn walk_items(items: &[Item], inline: &mut Vec<String>, in_test: bool, out: &mut ParsedFile) {
    for item in items {
        match item {
            Item::Fn(f) => {
                let is_test = in_test || is_test_attr(&f.attrs);
                let mut bare = f.clone();
                let doc = take_docs(&mut bare.attrs);
                out.fns.push(entry(
                    &f.sig.ident,
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
                            &f.sig.ident,
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
            Item::Struct(i) => push_item(&i.ident, ItemShape::Struct, &i.attrs, item, in_test, inline, out),
            Item::Enum(i) => push_item(&i.ident, ItemShape::Enum, &i.attrs, item, in_test, inline, out),
            Item::Union(i) => push_item(&i.ident, ItemShape::Union, &i.attrs, item, in_test, inline, out),
            Item::Type(i) => push_item(&i.ident, ItemShape::Alias, &i.attrs, item, in_test, inline, out),
            Item::Const(i) => push_item(&i.ident, ItemShape::Const, &i.attrs, item, in_test, inline, out),
            Item::Static(i) => push_item(&i.ident, ItemShape::Static, &i.attrs, item, in_test, inline, out),
            Item::Trait(t) => {
                push_item(&t.ident, ItemShape::Trait, &t.attrs, item, in_test, inline, out);
                let in_test = in_test || is_cfg_test(&t.attrs);
                let container = Container::Trait {
                    name: t.ident.to_string(),
                };
                for ti in &t.items {
                    if let TraitItem::Fn(f) = ti {
                        let mut bare = f.clone();
                        let doc = take_docs(&mut bare.attrs);
                        out.fns.push(entry(
                            &f.sig.ident,
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

/// Lists one non-function item (module doc, "Which other items exist").
/// `const _` is skipped: it has no name to review it under.
fn push_item(
    ident: &syn::Ident,
    shape: ItemShape,
    attrs: &[Attribute],
    item: &Item,
    in_test: bool,
    inline: &[String],
    out: &mut ParsedFile,
) {
    let name = ident.to_string();
    if name == "_" {
        return;
    }
    let name_line = ident.span().start().line as u32;
    let is_test = in_test || is_cfg_test(attrs);
    let mut bare = item.clone();
    let doc = match &mut bare {
        Item::Struct(i) => take_docs(&mut i.attrs),
        Item::Enum(i) => take_docs(&mut i.attrs),
        Item::Union(i) => take_docs(&mut i.attrs),
        Item::Type(i) => take_docs(&mut i.attrs),
        Item::Const(i) => take_docs(&mut i.attrs),
        Item::Static(i) => take_docs(&mut i.attrs),
        Item::Trait(i) => take_docs(&mut i.attrs),
        _ => Vec::new(),
    };
    let code = bare.to_token_stream();
    let mut code_text = String::new();
    push_tokens(code.clone(), &mut code_text);
    let mut anon = String::new();
    let mut dropped = false;
    for tt in code {
        if !dropped && matches!(&tt, TokenTree::Ident(i) if *i == name) {
            dropped = true;
            continue;
        }
        push_tokens(std::iter::once(tt).collect(), &mut anon);
    }
    let span = item.span();
    out.items.push(FnEntry {
        code_without_name: anon.trim_end().to_string(),
        name,
        container: Container::Free,
        lines: [span.start().line as u32, span.end().line as u32],
        doc: normalise_doc(&doc),
        code: code_text.trim_end().to_string(),
        is_test,
        inline_parents: inline.to_vec(),
        name_line,
        shape: Some(shape),
    });
}

/// Appends to each struct, enum and union's name-free text the name-free
/// code of every method written for it in this file, sorted (module doc).
fn fold_methods(out: &mut ParsedFile) {
    for it in &mut out.items {
        if !matches!(it.shape, Some(ItemShape::Struct | ItemShape::Enum | ItemShape::Union)) {
            continue;
        }
        let mut methods: Vec<&str> = out
            .fns
            .iter()
            .filter(|f| matches!(&f.container, Container::Impl { self_ty, .. } if *self_ty == it.name))
            .map(|f| f.code_without_name.as_str())
            .collect();
        methods.sort_unstable();
        for m in methods {
            it.code_without_name.push_str("\nmethod: ");
            it.code_without_name.push_str(m);
        }
    }
}

fn entry(
    ident: &syn::Ident,
    container: Container,
    span: proc_macro2::Span,
    doc: Vec<String>,
    code: TokenStream,
    is_test: bool,
    inline: &[String],
) -> FnEntry {
    let name = ident.to_string();
    let mut code_text = String::new();
    push_tokens(code.clone(), &mut code_text);
    let mut anon = String::new();
    push_tokens_without_name(code, &name, &mut anon);
    FnEntry {
        name_line: ident.span().start().line as u32,
        code_without_name: anon.trim_end().to_string(),
        name,
        container,
        lines: [span.start().line as u32, span.end().line as u32],
        doc: normalise_doc(&doc),
        code: code_text.trim_end().to_string(),
        is_test,
        inline_parents: inline.to_vec(),
        shape: None,
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

    /// Methodology: the fixture's other items (#843). Every struct, trait,
    /// alias, constant and static is listed with its shape and line range;
    /// `const _` is not. A struct's hash text changes when a method written
    /// for it in the same file changes, is added or is removed, and when a
    /// field changes; renaming the struct alone, reformatting and comments
    /// keep it. A method on another type does not touch it.
    ///
    /// Result (2026-10-13): passes.
    #[test]
    fn lists_types_constants_and_folds_methods_into_a_type() {
        let p = parse_file(SRC).unwrap();
        let got: Vec<(&str, ItemShape)> = p.items.iter().map(|i| (i.name.as_str(), i.shape.unwrap())).collect();
        assert_eq!(got, [("Sphere", ItemShape::Struct), ("Shape", ItemShape::Trait)]);
        assert_eq!(p.items[0].lines, [12, 12]);
        assert_eq!(p.items[0].item(), ItemKind::Type);
        assert_eq!(p.fns[0].item(), ItemKind::Fn);

        let one = |src: &str, name: &str| -> FnEntry {
            parse_file(src).unwrap().items.into_iter().find(|i| i.name == name).unwrap()
        };
        let base_src = "/// A ball.\npub struct Ball { r: f64 }\nimpl Ball { fn area(&self) -> f64 { 4.0 * self.r } }\nstruct Other; impl Other { fn a(&self) {} }\npub const PI2: f64 = 6.28;\npub static N: u8 = 1;\ntype R = f64;\nenum E { A, B }\nunion U { a: u8 }\nconst _: () = ();";
        let base = one(base_src, "Ball");
        assert_eq!(base.doc, "A ball.");
        assert_eq!(base.code, "pub struct Ball { r : f64 }");
        let shapes: Vec<ItemShape> = parse_file(base_src).unwrap().items.iter().map(|i| i.shape.unwrap()).collect();
        assert_eq!(
            shapes,
            [ItemShape::Struct, ItemShape::Struct, ItemShape::Const, ItemShape::Static, ItemShape::Alias, ItemShape::Enum, ItemShape::Union]
        );
        let same = [
            base_src.replace("Ball", "Globe"),
            base_src.replace("{ r: f64 }", "{\n    r: f64, // radius\n}").replace("r: f64,", "r: f64"),
            base_src.replace("fn area", "fn surface"),
            base_src.replace("fn a(&self) {}", "fn a(&self) { let _x = 1; }"),
        ];
        for s in &same {
            let name = if s.contains("Globe") { "Globe" } else { "Ball" };
            assert_eq!(one(s, name).code_without_name, base.code_without_name, "{s}");
        }
        let differ = [
            base_src.replace("4.0 * self.r", "3.0 * self.r"),
            base_src.replace("impl Ball { fn area(&self) -> f64 { 4.0 * self.r } }", ""),
            base_src.replace("struct Other;", "impl Ball { fn two(&self) {} } struct Other;"),
            base_src.replace("{ r: f64 }", "{ r: f32 }"),
        ];
        for s in &differ {
            assert_ne!(one(s, "Ball").code_without_name, base.code_without_name, "{s}");
        }
        assert_eq!(one(base_src, "PI2").code_without_name, "pub const : f64 = 6.28 ;");
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
