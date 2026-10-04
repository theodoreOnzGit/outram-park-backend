//! Pure text analysis of one Rust source file for `code-walk`: where each
//! `fn` is declared, what block it sits in (`impl Type`, `trait Name`), its
//! body's line range, its one-line signature and first doc line, and the
//! call-shaped tokens inside the body.
//!
//! Nothing here talks to rust-analyzer. The scanner finds *candidate*
//! positions; `super::builder` asks rust-analyzer what each one resolves to.
//! [`call_candidates`] is a port of kopitiam's `semq.rs::call_candidates_on_line`
//! (`apps/cli/src/semq.rs` in the maintainer's `kopitiam` repository, commit
//! `dfdf1c4`), extended in four ways kopitiam lacks:
//!
//! - comments and string/char literals are blanked first ([`blank_non_code`]),
//!   so `foo(` in a doc comment or a format string is not a call;
//! - every occurrence is kept, not the first per name, because
//!   `a.distance(` and `b.distance(` can resolve to different methods;
//! - turbofish calls (`collect::<Vec<_>>(`) are recognised;
//! - macro invocations (`name!(`), function values passed as arguments
//!   (`.map(transport_history)`) and calls through a parenthesised expression
//!   (`(self.f)(x)`) are reported separately, so the walk can resolve or mark
//!   them instead of silently dropping them.
//!
//! The body range comes from brace matching on the blanked text, not from
//! `textDocument/documentSymbol`: this workspace's rust-analyzer answers that
//! with the flat `SymbolInformation` shape whose range starts at the leading
//! doc comment (see `semq::locate_declaration`), and the keep-warm daemon's
//! async session has no `document_symbols` at all. kopitiam's recursion
//! re-finds a callee through `documentSymbol`'s `selectionRange`, which that
//! flat shape lacks; that is why its depth-2 output looked sparse (#523).

/// What kind of block a `fn` is declared inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Container {
    /// A free function (or one nested in a `mod`/another `fn`).
    Free,
    /// Inside `impl Type` or `impl Trait for Type`: the self type, and the
    /// trait if any.
    Impl {
        self_ty: String,
        trait_name: Option<String>,
    },
    /// Inside `trait Name { ... }`: a trait method declaration. Calls that
    /// resolve here dispatch to implementors the walk cannot see statically.
    Trait { name: String },
}

/// One `fn` declaration found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FnDecl {
    /// The bare identifier.
    pub(crate) name: String,
    /// 0-based line of the identifier.
    pub(crate) line: u32,
    /// 0-based char column of the identifier.
    pub(crate) character: u32,
    pub(crate) container: Container,
    /// 0-based inclusive line range of the body braces, `None` for a
    /// bodiless declaration (`fn f();` in a trait or `extern` block).
    pub(crate) body: Option<(u32, u32)>,
    /// 0-based char column of the body's opening `{` on its line.
    pub(crate) body_col: u32,
    /// The declaration up to its body, whitespace-collapsed, attributes and
    /// comments removed.
    pub(crate) signature: String,
    /// The first `///` line above the declaration, without the marker.
    pub(crate) doc: String,
}

impl FnDecl {
    /// `Type::name` for a method, `Trait::name` for a trait declaration,
    /// `name` for a free function.
    pub(crate) fn qualname(&self) -> String {
        match &self.container {
            Container::Free => self.name.clone(),
            Container::Impl { self_ty, .. } => format!("{self_ty}::{}", self.name),
            Container::Trait { name } => format!("{name}::{}", self.name),
        }
    }
}

/// A call-shaped token in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) name: String,
    pub(crate) line: u32,
    pub(crate) character: u32,
    pub(crate) kind: CandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateKind {
    /// `name(` / `.name(` / `Path::name(` / `name::<T>(`.
    Call,
    /// `name!(`, `name![`, `name!{` — a macro the walk cannot expand.
    Macro,
    /// A bare identifier in argument position, possibly a function passed by
    /// value (`.map(transport_history)`); rust-analyzer decides.
    FnValue,
    /// `)(` — a call through a parenthesised expression (a closure or `fn`
    /// pointer held in a field or returned by a call).
    IndirectCall,
}

/// Everything `code-walk` needs from one file, computed once and cached.
#[derive(Debug, Clone)]
pub(crate) struct FileIndex {
    /// Source lines, as read.
    pub(crate) lines: Vec<String>,
    /// The same lines with comments and literals blanked to spaces.
    pub(crate) code: Vec<String>,
    pub(crate) fns: Vec<FnDecl>,
}

/// Standard-library and ubiquitous third-party macros: their arguments are
/// already scanned as ordinary code, and their expansions call nothing in
/// the workspace, so they are not gaps.
const KNOWN_MACROS: [&str; 30] = [
    "println", "eprintln", "print", "eprint", "format", "write", "writeln", "panic", "assert",
    "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne", "vec",
    "matches", "unreachable", "todo", "unimplemented", "format_args", "concat", "stringify",
    "include_str", "include_bytes", "env", "cfg", "line", "file", "dbg", "compile_error",
];

const NOT_CALLS: [&str; 16] = [
    "if", "while", "for", "match", "return", "fn", "let", "in", "as", "where", "move", "dyn",
    "loop", "else", "impl", "mut",
];

impl FileIndex {
    pub(crate) fn parse(text: &str) -> FileIndex {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let blanked = blank_non_code(text);
        let code: Vec<String> = blanked.lines().map(str::to_string).collect();
        let blocks = blocks(&code);
        let mut fns = Vec::new();
        for (ln, line) in code.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i + 2 < chars.len() {
                let word_start = i == 0 || !is_ident(chars[i - 1]);
                if word_start && chars[i] == 'f' && chars[i + 1] == 'n' && chars[i + 2] == ' ' {
                    let mut j = i + 3;
                    while j < chars.len() && chars[j] == ' ' {
                        j += 1;
                    }
                    let start = j;
                    while j < chars.len() && is_ident(chars[j]) {
                        j += 1;
                    }
                    if j > start && !chars[start].is_ascii_digit() {
                        let name: String = chars[start..j].iter().collect();
                        fns.push(decl_at(&lines, &code, &blocks, ln as u32, start as u32, name));
                    }
                    i = j.max(i + 1);
                } else {
                    i += 1;
                }
            }
        }
        FileIndex { lines, code, fns }
    }

    /// The declaration whose identifier is at `line` (0-based), if any.
    pub(crate) fn fn_at_line(&self, line: u32) -> Option<&FnDecl> {
        self.fns.iter().find(|f| f.line == line)
    }

    /// Every declaration matching `qual` — a bare name, or `Type::name` /
    /// `Trait::name`.
    pub(crate) fn find(&self, qual: &str) -> Vec<&FnDecl> {
        let mut parts = qual.rsplitn(2, "::");
        let name = parts.next().unwrap_or(qual);
        let owner = parts.next();
        self.fns
            .iter()
            .filter(|f| f.name == name)
            .filter(|f| match owner {
                None => true,
                Some(o) => match &f.container {
                    Container::Free => false,
                    Container::Impl {
                        self_ty,
                        trait_name,
                    } => self_ty == o || trait_name.as_deref() == Some(o),
                    Container::Trait { name } => name == o,
                },
            })
            .collect()
    }

    /// The trimmed source text of line `line` (0-based).
    pub(crate) fn line_text(&self, line: u32) -> &str {
        self.lines.get(line as usize).map(|s| s.trim()).unwrap_or("")
    }

    /// The call-shaped tokens inside `decl`'s body, in source order.
    pub(crate) fn candidates(&self, decl: &FnDecl) -> Vec<Candidate> {
        let Some((start, end)) = decl.body else {
            return Vec::new();
        };
        let locals = local_names(&self.code, decl.line, end);
        let mut out = Vec::new();
        let mut in_use = false;
        for ln in start..=end {
            let Some(line) = self.code.get(ln as usize) else {
                continue;
            };
            // Only what follows the body's `{` on its first line: the
            // signature (`fn f(g: impl Fn(u32))`) holds no calls.
            let skip = if ln == start { decl.body_col as usize + 1 } else { 0 };
            let line = blank_use_statements(line, skip, &mut in_use);
            out.extend(call_candidates(&line, ln, &locals));
        }
        out
    }
}

/// Blanks the first `skip` chars of `line` and any `use ...;` statement on
/// it (`in_use` carries a statement split over several lines): the names in
/// `use a::{b, c};` sit in argument-like position but are imports, not
/// function values.
fn blank_use_statements(line: &str, skip: usize, in_use: &mut bool) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if i < skip {
            out.push(' ');
            i += 1;
            continue;
        }
        if !*in_use {
            let at_stmt_start = chars[..i]
                .iter()
                .rev()
                .find(|c| **c != ' ')
                .is_none_or(|c| matches!(c, '{' | '}' | ';'));
            let is_use = chars[i..].starts_with(&['u', 's', 'e', ' ']);
            if at_stmt_start && is_use {
                *in_use = true;
            }
        }
        if *in_use {
            if chars[i] == ';' {
                *in_use = false;
            }
            out.push(' ');
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out.into_iter().collect()
}

/// True for a macro name the walk need not ask about.
pub(crate) fn is_known_macro(name: &str) -> bool {
    KNOWN_MACROS.contains(&name)
}

fn is_ident(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// Replaces comments, string literals (plain, byte, raw) and char literals
/// with spaces, keeping every newline and every column where it was, so a
/// position in the blanked text is the same position in the source.
pub(crate) fn blank_non_code(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = c.clone();
    let blank = |out: &mut Vec<char>, from: usize, to: usize| {
        for ch in out.iter_mut().take(to).skip(from) {
            if *ch != '\n' {
                *ch = ' ';
            }
        }
    };
    let mut i = 0;
    while i < c.len() {
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            let mut j = i;
            while j < c.len() && c[j] != '\n' {
                j += 1;
            }
            blank(&mut out, i, j);
            i = j;
        } else if c[i] == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            let mut j = i + 2;
            while j < c.len() && depth > 0 {
                if c[j] == '/' && c.get(j + 1) == Some(&'*') {
                    depth += 1;
                    j += 2;
                } else if c[j] == '*' && c.get(j + 1) == Some(&'/') {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if c[i] == 'r'
            && (i == 0 || !is_ident(c[i - 1]))
            && matches!(c.get(i + 1), Some('"') | Some('#'))
        {
            let mut hashes = 0;
            let mut j = i + 1;
            while c.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if c.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            j += 1;
            loop {
                if j >= c.len() {
                    break;
                }
                if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            blank(&mut out, i, j);
            i = j;
        } else if c[i] == '#'
            && (c.get(i + 1) == Some(&'[')
                || (c.get(i + 1) == Some(&'!') && c.get(i + 2) == Some(&'[')))
        {
            // An attribute: `#[cfg(not(feature = "x"))]` holds no calls.
            let mut depth = 0i32;
            let mut j = i;
            while j < c.len() {
                match c[j] {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            blank(&mut out, i, j);
            i = j;
        } else if c[i] == '"' {
            let mut j = i + 1;
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            blank(&mut out, i, (j + 1).min(c.len()));
            i = j + 1;
        } else if c[i] == '\'' {
            // A char literal is 'x', '\n', '\'', '\u{..}'; anything else
            // (`'a`, `'static`, a label) is a lifetime and is code.
            let end = if c.get(i + 1) == Some(&'\\') {
                let mut j = i + 2;
                while j < c.len() && c[j] != '\'' && c[j] != '\n' {
                    j += 1;
                }
                (c.get(j) == Some(&'\'')).then_some(j)
            } else if c.get(i + 2) == Some(&'\'') && c.get(i + 1) != Some(&'\n') {
                Some(i + 2)
            } else {
                None
            };
            match end {
                Some(j) => {
                    blank(&mut out, i, j + 1);
                    i = j + 1;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// One brace-delimited block: where it opens and closes (0-based line and
/// char) and the item header text in front of its `{`.
#[derive(Debug, Clone)]
struct Block {
    open: (u32, u32),
    close: (u32, u32),
    header: String,
}

/// Brace-matches the blanked text. A block's header is the code between the
/// previous `;`, `{` or `}` and its own `{`, whitespace-collapsed.
fn blocks(code: &[String]) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut header = String::new();
    for (ln, line) in code.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            match ch {
                '{' => {
                    out.push(Block {
                        open: (ln as u32, col as u32),
                        close: (u32::MAX, 0),
                        header: collapse(&header),
                    });
                    stack.push(out.len() - 1);
                    header.clear();
                }
                '}' => {
                    if let Some(b) = stack.pop() {
                        out[b].close = (ln as u32, col as u32);
                    }
                    header.clear();
                }
                ';' => header.clear(),
                _ => header.push(ch),
            }
        }
        header.push(' ');
    }
    out
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Drops leading `#[...]` / `#![...]` attributes from a header.
fn strip_attributes(header: &str) -> &str {
    let mut h = header.trim_start();
    while h.starts_with("#[") || h.starts_with("#![") {
        let mut depth = 0i32;
        let mut cut = h.len();
        for (i, ch) in h.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        cut = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        h = h[cut..].trim_start();
    }
    h
}

/// `impl<T> Foo<T> for Bar<T> where ...` -> (`Bar`, Some(`Foo`)).
fn parse_impl_header(h: &str) -> Option<(String, Option<String>)> {
    let h = h.strip_prefix("unsafe ").unwrap_or(h);
    let rest = h.strip_prefix("impl")?;
    if !(rest.starts_with(' ') || rest.starts_with('<')) {
        return None;
    }
    let rest = skip_generics(rest.trim_start()).trim_start();
    let rest = rest.split(" where ").next().unwrap_or(rest);
    let (trait_part, ty_part) = match rest.split_once(" for ") {
        Some((t, ty)) => (Some(t), ty),
        None => (None, rest),
    };
    Some((type_head(ty_part), trait_part.map(type_head)))
}

fn skip_generics(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return &s[i + 1..];
                }
            }
            _ => {}
        }
    }
    s
}

/// The last path segment of a type, without generics or references:
/// `&mut crate::geom::Sphere<T>` -> `Sphere`.
fn type_head(s: &str) -> String {
    let s = s.trim().trim_start_matches('&').trim_start_matches("mut ").trim();
    let s = s.trim_start_matches("dyn ");
    let base = s.split('<').next().unwrap_or(s).trim();
    base.rsplit("::").next().unwrap_or(base).trim().to_string()
}

fn trait_name(h: &str) -> Option<String> {
    let mut words = h.split_whitespace().peekable();
    while let Some(w) = words.next() {
        if w == "trait" {
            return words.next().map(|n| {
                n.split(|c: char| c == '<' || c == ':')
                    .next()
                    .unwrap_or(n)
                    .to_string()
            });
        }
        if !matches!(w, "pub" | "unsafe" | "auto") && !w.starts_with("pub(") {
            return None;
        }
    }
    None
}

fn decl_at(
    lines: &[String],
    code: &[String],
    blocks: &[Block],
    line: u32,
    character: u32,
    name: String,
) -> FnDecl {
    let pos = (line, character);
    // Innermost enclosing impl/trait block.
    let mut container = Container::Free;
    let mut best_open = (0u32, 0u32);
    for b in blocks {
        if b.open < pos && pos < b.close && b.open >= best_open {
            let h = strip_attributes(&b.header);
            if let Some((self_ty, trait_name)) = parse_impl_header(h) {
                container = Container::Impl {
                    self_ty,
                    trait_name,
                };
                best_open = b.open;
            } else if let Some(name) = trait_name(h) {
                container = Container::Trait { name };
                best_open = b.open;
            } else if h.contains("fn ") || h.starts_with("mod ") || h.contains(" mod ") {
                // A nested fn or a module resets the container.
                container = Container::Free;
                best_open = b.open;
            }
        }
    }
    // The body: the first block opening after the identifier whose header
    // still belongs to this declaration (no `;` in between).
    let (body, body_col, signature) = body_and_signature(code, blocks, line, character);
    FnDecl {
        name,
        line,
        character,
        container,
        body,
        body_col,
        signature,
        doc: first_doc_line(lines, line),
    }
}

fn body_and_signature(
    code: &[String],
    blocks: &[Block],
    line: u32,
    character: u32,
) -> (Option<(u32, u32)>, u32, String) {
    // Signature text runs from the start of the declaring line's item (the
    // `pub`/`async`/... qualifiers before `fn`) to the first `{` or `;`.
    let decl_line = code.get(line as usize).map(String::as_str).unwrap_or("");
    let ident_byte = decl_line
        .char_indices()
        .nth(character as usize)
        .map(|(b, _)| b)
        .unwrap_or(decl_line.len());
    let prefix = &decl_line[..ident_byte];
    let item_start = prefix
        .rfind(['{', '}', ';'])
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut sig = String::from(&decl_line[item_start..]);
    let mut end: Option<(u32, u32, char)> = None;
    let mut first = true;
    let mut depth_paren = 0i32;
    'scan: for (ln, l) in code.iter().enumerate().skip(line as usize) {
        let start_col = if first { ident_byte } else { 0 };
        if !first {
            sig.push(' ');
            sig.push_str(l);
        }
        first = false;
        for (bi, ch) in l[start_col.min(l.len())..].char_indices() {
            match ch {
                '(' | '[' => depth_paren += 1,
                ')' | ']' => depth_paren -= 1,
                '{' | ';' if depth_paren <= 0 => {
                    let col = l[..start_col + bi].chars().count() as u32;
                    end = Some((ln as u32, col, ch));
                    break 'scan;
                }
                _ => {}
            }
        }
        if ln as u32 > line + 40 {
            break;
        }
    }
    let Some((eln, ecol, ech)) = end else {
        return (None, 0, collapse(&sig));
    };
    // Trim the signature at the terminator.
    let consumed_lines = (eln - line) as usize;
    let mut sig_text = String::new();
    for (k, l) in code
        .iter()
        .enumerate()
        .skip(line as usize)
        .take(consumed_lines + 1)
    {
        let from = if k == line as usize { item_start } else { 0 };
        let to = if k as u32 == eln {
            l.char_indices()
                .nth(ecol as usize)
                .map(|(b, _)| b)
                .unwrap_or(l.len())
        } else {
            l.len()
        };
        sig_text.push_str(&l[from.min(to)..to]);
        sig_text.push(' ');
    }
    // rustfmt's one-parameter-per-line layout collapses to `( a, b, )`;
    // print it the way it would be written on one line.
    let signature = collapse(strip_attributes(&collapse(&sig_text)))
        .replace(", )", ")")
        .replace(",)", ")")
        .replace("( ", "(")
        .replace(" )", ")");
    if ech == ';' {
        return (None, 0, signature);
    }
    let body = blocks
        .iter()
        .find(|b| b.open == (eln, ecol))
        .map(|b| (b.open.0, b.close.0));
    (body, ecol, signature)
}

/// The first sentence of the doc comment directly above `line` (attribute
/// lines skipped): its first paragraph, joined across `///` lines and cut
/// after the first `. `, so a sentence wrapped over two source lines is not
/// shown half-finished.
fn first_doc_line(lines: &[String], line: u32) -> String {
    let mut doc: Vec<String> = Vec::new();
    let mut k = line as usize;
    while k > 0 {
        k -= 1;
        let t = lines[k].trim();
        if let Some(rest) = t.strip_prefix("///") {
            doc.push(rest.trim().to_string());
        } else if t.starts_with("#[") || t.starts_with("#![") || t.ends_with(")]") {
            continue;
        } else {
            break;
        }
    }
    doc.reverse();
    let para: Vec<&str> = doc
        .iter()
        .map(String::as_str)
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty() && !l.starts_with("```") && !l.starts_with('#'))
        .collect();
    let joined = strip_intra_doc_links(&para.join(" "));
    match joined.find(". ") {
        Some(i) => joined[..=i].to_string(),
        None => joined,
    }
}

/// `` [`Name`] `` (a rustdoc intra-doc link, which Markdown outside rustdoc
/// shows with its brackets) becomes `` `Name` ``; a real link `[x](url)` is
/// left alone.
fn strip_intra_doc_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("[`") {
        let after = &rest[i + 1..];
        match after.find("`]") {
            Some(j) if !after[j + 2..].starts_with(['(', '[']) => {
                out.push_str(&rest[..i]);
                out.push_str(&after[..j + 1]);
                rest = &after[j + 2..];
            }
            _ => {
                out.push_str(&rest[..i + 2]);
                rest = &rest[i + 2..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Names bound locally in `[from, to]` (`let x`, `let mut x`, closure
/// parameters `|x|`, `for x in`, `name:` parameters on the declaration
/// line), so a bare identifier in argument position that is one of these is
/// not asked about as a possible function value.
fn local_names(code: &[String], from: u32, to: u32) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for ln in from..=to {
        let Some(l) = code.get(ln as usize) else {
            continue;
        };
        let words: Vec<&str> = l
            .split(|c: char| !is_ident(c))
            .filter(|w| !w.is_empty())
            .collect();
        for w in words.windows(2) {
            if matches!(w[0], "let" | "mut" | "for" | "ref") {
                out.insert(w[1].to_string());
            }
        }
        // `name:` (params, struct-literal fields) and `|a, b|` closure params.
        let chars: Vec<char> = l.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if is_ident(chars[i]) && (i == 0 || !is_ident(chars[i - 1])) {
                let s = i;
                while i < chars.len() && is_ident(chars[i]) {
                    i += 1;
                }
                let name: String = chars[s..i].iter().collect();
                if chars.get(i) == Some(&':') && chars.get(i + 1) != Some(&':') {
                    out.insert(name);
                }
            } else {
                i += 1;
            }
        }
        let mut in_pipe = false;
        let mut cur = String::new();
        for ch in l.chars() {
            if ch == '|' {
                if in_pipe && !cur.is_empty() {
                    out.insert(std::mem::take(&mut cur));
                }
                in_pipe = !in_pipe;
                cur.clear();
            } else if in_pipe {
                if is_ident(ch) {
                    cur.push(ch);
                } else if !cur.is_empty() {
                    out.insert(std::mem::take(&mut cur));
                }
            }
        }
    }
    out
}

/// Finds the call-shaped tokens on one blanked line. Port of kopitiam's
/// `call_candidates_on_line`, extended as described in the module doc.
pub(crate) fn call_candidates(
    line: &str,
    line_no: u32,
    locals: &std::collections::HashSet<String>,
) -> Vec<Candidate> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == ')' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            // `)(` is a call through an expression, unless the `)` closes a
            // control-flow condition like `if (a) (b)`, which rustfmt never
            // writes. A tuple `(a)(b)` is not valid Rust either.
            if j == i + 1 && chars.get(j) == Some(&'(') {
                out.push(Candidate {
                    name: String::from("(expression)"),
                    line: line_no,
                    character: i as u32,
                    kind: CandidateKind::IndirectCall,
                });
            }
            i += 1;
            continue;
        }
        let starts_ident = (chars[i] == '_' || chars[i].is_alphabetic())
            && (i == 0 || !is_ident(chars[i - 1]));
        if !starts_ident {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_ident(chars[i]) {
            i += 1;
        }
        let name: String = chars[start..i].iter().collect();
        let prev = chars[..start].iter().rev().find(|c| **c != ' ').copied();
        let mut j = i;
        // Turbofish: name::<...>(
        if chars.get(j) == Some(&':') && chars.get(j + 1) == Some(&':') && chars.get(j + 2) == Some(&'<') {
            let mut depth = 0i32;
            let mut k = j + 2;
            while k < chars.len() {
                match chars[k] {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            k += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            j = k;
        }
        let mut k = j;
        while k < chars.len() && chars[k] == ' ' {
            k += 1;
        }
        let next = chars.get(k).copied();
        if next == Some('(') && !NOT_CALLS.contains(&name.as_str()) {
            out.push(Candidate {
                name,
                line: line_no,
                character: start as u32,
                kind: CandidateKind::Call,
            });
        } else if chars.get(i) == Some(&'!')
            && matches!(chars.get(i + 1), Some('(') | Some('[') | Some('{'))
        {
            out.push(Candidate {
                name,
                line: line_no,
                character: start as u32,
                kind: CandidateKind::Macro,
            });
        } else if matches!(prev, Some('(') | Some(','))
            && matches!(next, Some(')') | Some(','))
            && j == i
            && prev != Some('.')
            && name.chars().next().is_some_and(|c| c.is_lowercase() || c == '_')
            && !locals.contains(&name)
            && !matches!(name.as_str(), "self" | "true" | "false" | "None" | "_")
        {
            out.push(Candidate {
                name,
                line: line_no,
                character: start as u32,
                kind: CandidateKind::FnValue,
            });
        }
        i = i.max(start + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"//! module doc mentioning foo(1)
use std::fmt;

/// Advance one history.
///
/// Long text.
#[inline]
pub fn transport_history(seed: &mut u64) -> f64 {
    let s = "not_a_call(";
    let c = '{';
    use crate::x::{alpha,
        beta};
    let x = helper(seed); // also_not(a call)
    let v = data.iter().map(scale).collect::<Vec<_>>();
    my_macro!(x);
    println!("{}", inner(x));
    (self.f)(x)
}

impl<T: Clone> Sphere<T> {
    /// Distance to the surface.
    pub(crate) fn distance(
        &self,
        r: f64,
    ) -> f64
    where
        T: Copy,
    {
        r
    }
}

impl Geometry for Sphere {
    fn volume(&self) -> f64 { 1.0 }
}

pub trait Geometry {
    fn volume(&self) -> f64;
}
"#;

    /// Methodology: a synthetic file with a free fn, a multi-line generic
    /// method with a `where` clause, a trait impl and a bodiless trait
    /// declaration. Checks containers, body ranges, signatures, doc lines.
    ///
    /// Result (2026-10-04): all four declarations classified as written.
    #[test]
    fn indexes_declarations_with_containers_bodies_and_signatures() {
        let idx = FileIndex::parse(SRC);
        let names: Vec<String> = idx.fns.iter().map(FnDecl::qualname).collect();
        assert_eq!(
            names,
            vec![
                "transport_history",
                "Sphere::distance",
                "Sphere::volume",
                "Geometry::volume"
            ]
        );
        let th = &idx.fns[0];
        assert_eq!(th.line, 7);
        assert_eq!(th.body, Some((7, 17)));
        assert_eq!(th.signature, "pub fn transport_history(seed: &mut u64) -> f64");
        assert_eq!(th.doc, "Advance one history.");
        let wrapped: Vec<String> = ["/// One sentence", "/// wrapped. Second.", "fn f() {}"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(first_doc_line(&wrapped, 2), "One sentence wrapped.");
        assert_eq!(
            strip_intra_doc_links("the form of [`two_body_scatter`] and [`x`](u) [`y`][z]"),
            "the form of `two_body_scatter` and [`x`](u) [`y`][z]"
        );
        let d = &idx.fns[1];
        assert_eq!(
            d.signature,
            "pub(crate) fn distance(&self, r: f64) -> f64 where T: Copy,"
        );
        assert_eq!(d.doc, "Distance to the surface.");
        assert_eq!(d.body.map(|b| b.1), Some(29));
        assert!(matches!(
            idx.fns[2].container,
            Container::Impl { ref trait_name, .. } if trait_name.as_deref() == Some("Geometry")
        ));
        assert_eq!(idx.fns[3].body, None);
        assert_eq!(idx.find("Geometry::volume").len(), 2);
        assert_eq!(idx.find("Sphere::volume").len(), 1);
        assert_eq!(idx.find("distance").len(), 1);
    }

    /// Methodology: the body of `transport_history` above holds a call in a
    /// string, a call in a comment, a brace char literal, a real call, a
    /// function passed by value, a turbofish call, a workspace macro, a std
    /// macro wrapping a call, and a call through `(self.f)`.
    ///
    /// Result (2026-10-04): only the real tokens are reported, each with its
    /// kind; the literal and the comment produce nothing.
    #[test]
    fn candidates_skip_literals_and_classify_kinds() {
        let idx = FileIndex::parse(SRC);
        let c = idx.candidates(&idx.fns[0]);
        let got: Vec<(String, CandidateKind)> =
            c.iter().map(|c| (c.name.clone(), c.kind)).collect();
        use CandidateKind::*;
        assert_eq!(
            got,
            vec![
                ("helper".to_string(), Call),
                ("iter".to_string(), Call),
                ("map".to_string(), Call),
                ("scale".to_string(), FnValue),
                ("collect".to_string(), Call),
                ("my_macro".to_string(), Macro),
                ("println".to_string(), Macro),
                ("inner".to_string(), Call),
                ("(expression)".to_string(), IndirectCall),
            ]
        );
        // `x` is a local: never a function-value candidate.
        assert!(!got.iter().any(|(n, _)| n == "x"));
    }

    /// Methodology: lifetimes must survive blanking (they are code), char
    /// literals and raw strings must not.
    ///
    /// Result (2026-10-04): passes.
    #[test]
    fn blanking_keeps_lifetimes_and_columns() {
        let src = "fn f<'a>(x: &'a str) { #[cfg(not(x))] let s = r#\"g(\"#; let c = '\\''; }";
        let b = blank_non_code(src);
        assert_eq!(b.chars().count(), src.chars().count());
        assert!(b.contains("<'a>"));
        assert!(!b.contains("g("));
        assert!(!b.contains("'\\''"));
        // Attributes hold no calls.
        assert!(!b.contains("cfg"));
    }
}
