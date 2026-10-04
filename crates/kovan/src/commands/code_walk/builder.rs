//! Builds a [`CallGraph`] by asking rust-analyzer where every call-shaped
//! token in a function body resolves, breadth-first from an entry point.
//!
//! Port of kopitiam's `collect_callees` (`apps/cli/src/semq.rs`, commit
//! `dfdf1c4`), reworked in three ways:
//!
//! - **breadth-first with a global node set**, not depth-first with a
//!   `(path, line)` visited set shared across branches — kopitiam's visited
//!   set silently drops a callee from every branch after the first, and its
//!   depth-first order cannot give shortest chains;
//! - **recursion re-finds the callee from source text** (`super::source`),
//!   because this workspace's rust-analyzer answers `documentSymbol` with the
//!   flat shape kopitiam's `symbol_at` cannot match — the reason its second
//!   hop came back sparse (#523);
//! - **every token that does not resolve to a workspace function body is
//!   classified**: external (std or a dependency, filtered and counted),
//!   a constructor (ignored — not code), or a [`Gap`] that the output marks
//!   `UNRESOLVED(...)`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kopitiam_semantic::RustAnalyzerSession;

use super::graph::{CallGraph, Edge, EdgeKind, FnNode, Gap, GapKind, NodeId};
use super::source::{is_known_macro, CandidateKind, Container, FileIndex, FnDecl};
use crate::commands::{lsp_daemon, semq};

/// Where definitions come from: the keep-warm daemon (`commands::lsp_daemon`,
/// preferred — its index survives across invocations) or a session spawned
/// for this one run when no daemon can be reached (non-Unix targets).
pub(crate) enum Resolver {
    Daemon(PathBuf),
    Session(RustAnalyzerSession),
}

/// A definition site: absolute path, 0-based line and char column.
type Site = (PathBuf, u32, u32);

impl Resolver {
    /// Connects to (or starts) the daemon for `root`, falling back to a
    /// one-shot session.
    pub(crate) fn connect(root: &Path) -> Result<Resolver, String> {
        let probe = lsp_daemon::Request::Definitions {
            file: root.join("Cargo.toml"),
            positions: Vec::new(),
        };
        match lsp_daemon::query(root, &probe) {
            Some(resp) if resp.ok => Ok(Resolver::Daemon(root.to_path_buf())),
            Some(resp) => Err(resp
                .error
                .unwrap_or_else(|| "lsp-daemon failed to become ready".to_string())),
            None => Ok(Resolver::Session(semq::connect(root)?)),
        }
    }

    fn definitions(&mut self, file: &Path, positions: &[[u32; 2]]) -> Result<Vec<Vec<Site>>, String> {
        match self {
            Resolver::Daemon(root) => {
                let req = lsp_daemon::Request::Definitions {
                    file: file.to_path_buf(),
                    positions: positions.to_vec(),
                };
                let resp = lsp_daemon::query(root, &req)
                    .ok_or_else(|| "lost connection to the lsp-daemon".to_string())?;
                if !resp.ok {
                    return Err(resp.error.unwrap_or_else(|| "definition query failed".into()));
                }
                Ok(resp
                    .definitions
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| {
                        v.into_iter()
                            .map(|c| (PathBuf::from(c.file), c.line, c.character))
                            .collect()
                    })
                    .collect())
            }
            Resolver::Session(session) => positions
                .iter()
                .map(|[l, c]| {
                    semq::retry_content_modified(|| {
                        session.definition(file, *l, *c).map_err(|e| e.to_string())
                    })
                    .map(|locs| {
                        locs.iter()
                            .map(|x| (x.path.clone(), x.range.start.line, x.range.start.character))
                            .collect()
                    })
                })
                .collect(),
        }
    }

    /// Shuts a one-shot session down; the daemon is left warm on purpose.
    pub(crate) fn finish(self) {
        if let Resolver::Session(s) = self {
            let _ = s.shutdown();
        }
    }
}

/// A hop filled in by hand: `from -> to`, both `file.rs::qualname`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HandHop {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) note: String,
}

/// Reads and indexes workspace files and resolves calls through a
/// [`Resolver`]. One per workspace root; shared by every walk in a run.
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    resolver: Resolver,
    files: HashMap<String, FileIndex>,
    /// Definition queries sent so far (progress reporting).
    pub(crate) queries: usize,
}

/// A graph under construction for one walk.
pub(crate) struct Walk {
    pub(crate) graph: CallGraph,
    hand: Vec<(NodeId, NodeId, String)>,
    /// Hand-filled hops the tool now resolves by itself.
    pub(crate) redundant_hand: Vec<String>,
    /// Calls into std or dependencies, filtered out.
    pub(crate) external_calls: usize,
    /// Set when the node cap stopped the search early.
    pub(crate) truncated: bool,
}

impl Workspace {
    pub(crate) fn open(root: &Path) -> Result<Workspace, String> {
        let root = std::fs::canonicalize(root)
            .map_err(|e| format!("resolving workspace root {}: {e}", root.display()))?;
        let resolver = Resolver::connect(&root)?;
        Ok(Workspace {
            root,
            resolver,
            files: HashMap::new(),
            queries: 0,
        })
    }

    pub(crate) fn close(self) {
        self.resolver.finish();
    }

    fn index(&mut self, rel: &str) -> Result<&FileIndex, String> {
        if !self.files.contains_key(rel) {
            let text = std::fs::read_to_string(self.root.join(rel))
                .map_err(|e| format!("cannot read {rel}: {e}"))?;
            self.files.insert(rel.to_string(), FileIndex::parse(&text));
        }
        Ok(&self.files[rel])
    }

    /// Workspace-relative `/` path for an absolute one, or `None` when it is
    /// outside the workspace (std, the cargo registry) or in a build tree.
    fn relative(&self, abs: &Path) -> Option<String> {
        let canon = std::fs::canonicalize(abs).unwrap_or_else(|_| abs.to_path_buf());
        let rel = canon.strip_prefix(&self.root).ok()?;
        let s = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if s.starts_with("target/") || s.contains("/target/") || s.starts_with("vendor/") {
            return None;
        }
        Some(s)
    }

    fn node(rel: &str, d: &FnDecl) -> FnNode {
        FnNode {
            file: rel.to_string(),
            qualname: d.qualname(),
            line: d.line + 1,
            signature: d.signature.clone(),
            doc: d.doc.clone(),
        }
    }

    /// Resolves `crates/x/src/a.rs::name` or `...::Type::name` to the
    /// declaration it names.
    pub(crate) fn locate(&mut self, spec: &str) -> Result<FnNode, String> {
        let (file, qual) = split_spec(spec)?;
        let idx = self.index(&file)?;
        let found = idx.find(&qual);
        match found.as_slice() {
            [one] => Ok(Self::node(&file, one)),
            [] => Err(format!("no `fn` matching `{qual}` in {file}")),
            many => Err(format!(
                "`{qual}` is ambiguous in {file}: {} — qualify it as Type::name",
                many.iter()
                    .map(|d| format!("{} (line {})", d.qualname(), d.line + 1))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// Starts a walk, resolving its hand-filled hops. A hop whose endpoint
    /// no longer exists is an error: the check mode must fail on it.
    pub(crate) fn start(&mut self, hand: &[HandHop]) -> Result<Walk, String> {
        let mut walk = Walk {
            graph: CallGraph::default(),
            hand: Vec::new(),
            redundant_hand: Vec::new(),
            external_calls: 0,
            truncated: false,
        };
        for h in hand {
            let a = self
                .locate(&h.from)
                .map_err(|e| format!("hand-filled hop `{} -> {}`: {e}", h.from, h.to))?;
            let b = self
                .locate(&h.to)
                .map_err(|e| format!("hand-filled hop `{} -> {}`: {e}", h.from, h.to))?;
            let (a, b) = (walk.graph.intern(a), walk.graph.intern(b));
            walk.hand.push((a, b, h.note.clone()));
        }
        Ok(walk)
    }

    /// Scans `id`'s body and records every edge and gap out of it.
    pub(crate) fn expand(&mut self, walk: &mut Walk, id: NodeId) -> Result<(), String> {
        if walk.graph.expanded[id.0] {
            return Ok(());
        }
        walk.graph.expanded[id.0] = true;
        let (file, line) = {
            let n = walk.graph.node(id);
            (n.file.clone(), n.line - 1)
        };
        let idx = self.index(&file)?;
        let Some(decl) = idx.fn_at_line(line).cloned() else {
            return Err(format!("{file}:{}: no fn declaration here any more", line + 1));
        };
        let cands: Vec<_> = idx
            .candidates(&decl)
            .into_iter()
            .filter(|c| !(c.kind == CandidateKind::Macro && is_known_macro(&c.name)))
            .collect();
        let ask: Vec<usize> = (0..cands.len())
            .filter(|&i| cands[i].kind != CandidateKind::IndirectCall)
            .collect();
        let positions: Vec<[u32; 2]> = ask.iter().map(|&i| [cands[i].line, cands[i].character]).collect();
        let abs = self.root.join(&file);
        self.queries += positions.len();
        let answers = if positions.is_empty() {
            Vec::new()
        } else {
            self.resolver.definitions(&abs, &positions)?
        };
        let mut defs: Vec<Vec<Site>> = vec![Vec::new(); cands.len()];
        for (k, &i) in ask.iter().enumerate() {
            defs[i] = answers.get(k).cloned().unwrap_or_default();
        }

        for (cand, sites) in cands.iter().zip(defs) {
            let call_line = cand.line + 1;
            let gap = |kind: GapKind, target: Option<(String, u32)>, detail: String| Gap {
                from: id,
                call_line,
                kind,
                callee: match cand.kind {
                    CandidateKind::Macro => format!("{}!", cand.name),
                    _ => cand.name.clone(),
                },
                target,
                detail,
            };
            if cand.kind == CandidateKind::IndirectCall {
                push_gap(walk, gap(
                    GapKind::Closure,
                    None,
                    "call through a parenthesised expression (a closure or fn pointer)".into(),
                ));
                continue;
            }
            if sites.is_empty() {
                match cand.kind {
                    CandidateKind::Call => push_gap(walk, gap(
                        GapKind::NoDefinition,
                        None,
                        "rust-analyzer returned no definition".into(),
                    )),
                    CandidateKind::Macro => push_gap(walk, gap(
                        GapKind::Macro,
                        None,
                        "macro not resolved by rust-analyzer; its expansion is not followed".into(),
                    )),
                    _ => {}
                }
                continue;
            }
            for (path, dline, dchar) in sites {
                let Some(rel) = self.relative(&path) else {
                    if cand.kind != CandidateKind::FnValue {
                        walk.external_calls += 1;
                    }
                    continue;
                };
                let tidx = self.index(&rel)?;
                let target_text = tidx.line_text(dline).to_string();
                if cand.kind == CandidateKind::Macro {
                    push_gap(walk, gap(
                        GapKind::Macro,
                        Some((rel.clone(), dline + 1)),
                        "workspace macro; its expansion is not followed".into(),
                    ));
                    continue;
                }
                if let Some(d) = tidx
                    .fns
                    .iter()
                    .find(|f| f.line == dline && f.character == dchar)
                    .or_else(|| tidx.fn_at_line(dline))
                    .cloned()
                {
                    if let Container::Trait { name } = &d.container {
                        push_gap(walk, gap(
                            GapKind::Trait,
                            Some((rel.clone(), dline + 1)),
                            format!(
                                "trait method `{name}::{}`; the implementor is chosen by type, not followed",
                                d.name
                            ),
                        ));
                        continue;
                    }
                    let to = walk.graph.intern(Self::node(&rel, &d));
                    walk.graph.add_edge(Edge {
                        from: id,
                        to,
                        call_line,
                        kind: match cand.kind {
                            CandidateKind::FnValue => EdgeKind::FnValue,
                            _ => EdgeKind::Call,
                        },
                    });
                    continue;
                }
                if cand.kind == CandidateKind::FnValue || is_constructor(&target_text, &cand.name) {
                    continue; // a variable, or a tuple-struct / variant constructor
                }
                let binding = target_text.starts_with("let ")
                    || target_text.contains('|')
                    || (rel == file && dline >= decl.line && dline <= decl.body.map_or(dline, |b| b.0));
                if binding {
                    push_gap(walk, gap(
                        GapKind::Closure,
                        Some((rel.clone(), dline + 1)),
                        format!("call through a closure or fn-typed binding `{}`", cand.name),
                    ));
                } else {
                    push_gap(walk, gap(
                        GapKind::Other,
                        Some((rel.clone(), dline + 1)),
                        format!("resolves to `{}`, not a function body", truncate(&target_text, 80)),
                    ));
                }
            }
        }

        let hand: Vec<(NodeId, NodeId, String)> =
            walk.hand.iter().filter(|h| h.0 == id).cloned().collect();
        for (a, b, note) in hand {
            if walk.graph.edge(a, b).is_some() {
                walk.redundant_hand.push(format!(
                    "{} -> {}",
                    walk.graph.node(a).key(),
                    walk.graph.node(b).key()
                ));
                continue;
            }
            let call_line = walk.graph.node(a).line;
            walk.graph.add_edge(Edge {
                from: a,
                to: b,
                call_line,
                kind: EdgeKind::Hand { note },
            });
        }
        Ok(())
    }

    /// Breadth-first expansion from `from`, one layer at a time, stopping
    /// after the layer in which `target` is first reached (so every shortest
    /// chain to it is present), at `max_depth` layers, or once more than
    /// `max_nodes` functions are known.
    pub(crate) fn bfs(
        &mut self,
        walk: &mut Walk,
        from: NodeId,
        target: Option<NodeId>,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<(), String> {
        let mut seen = vec![false; walk.graph.nodes.len()];
        seen[from.0] = true;
        let mut layer = vec![from];
        for depth in 0..max_depth {
            if layer.is_empty() || target.is_some_and(|t| t != from && seen[t.0]) {
                break;
            }
            let mut next = Vec::new();
            for &u in &layer {
                self.expand(walk, u)?;
                seen.resize(walk.graph.nodes.len(), false);
                for e in walk.graph.out_edges(u) {
                    if !seen[e.to.0] {
                        seen[e.to.0] = true;
                        next.push(e.to);
                    }
                }
            }
            eprintln!(
                "kovan-cli code-walk: depth {} done — {} functions, {} definition queries",
                depth + 1,
                walk.graph.nodes.len(),
                self.queries
            );
            if walk.graph.nodes.len() > max_nodes {
                walk.truncated = true;
                break;
            }
            layer = next;
        }
        Ok(())
    }
}

fn push_gap(walk: &mut Walk, g: Gap) {
    if !walk
        .graph
        .gaps
        .iter()
        .any(|x| x.from == g.from && x.call_line == g.call_line && x.callee == g.callee)
    {
        walk.graph.gaps.push(g);
    }
}

/// A tuple struct or enum variant: `struct Name(`, or a variant line
/// `Name(...)` / `Name {` / `Name,`.
fn is_constructor(target_text: &str, name: &str) -> bool {
    let t = target_text
        .trim_start_matches("pub(crate) ")
        .trim_start_matches("pub ");
    if t.starts_with("struct ") || t.starts_with("enum ") {
        return true;
    }
    t.strip_prefix(name)
        .and_then(|r| r.trim_start().chars().next())
        .is_some_and(|c| matches!(c, '(' | '{' | ','))
        && name.chars().next().is_some_and(char::is_uppercase)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// Splits `crates/x/src/a.rs::Type::name` into the file and the qualified
/// name.
pub(crate) fn split_spec(spec: &str) -> Result<(String, String), String> {
    let Some(i) = spec.find(".rs::") else {
        return Err(format!(
            "`{spec}` is not of the form <path/to/file.rs>::<fn> (or ::<Type>::<fn>)"
        ));
    };
    let file = spec[..i + 3].trim_start_matches("./").to_string();
    let qual = spec[i + 5..].to_string();
    if qual.is_empty() {
        return Err(format!("`{spec}` names no function after `::`"));
    }
    Ok((file, qual))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the spec form, with and without a type qualifier, and
    /// two malformed specs.
    ///
    /// Result (2026-10-04): passes.
    #[test]
    fn split_spec_parses_file_and_qualified_name() {
        assert_eq!(
            split_spec("crates/a/src/geom.rs::Sphere::distance").unwrap(),
            ("crates/a/src/geom.rs".into(), "Sphere::distance".into())
        );
        assert_eq!(
            split_spec("./x.rs::main").unwrap(),
            ("x.rs".into(), "main".into())
        );
        assert!(split_spec("transport_history").is_err());
        assert!(split_spec("a.rs::").is_err());
    }

    /// Methodology: constructor lines (tuple struct, variant) must be
    /// recognised so they are not reported as gaps; a `let` binding and a
    /// lowercase name must not be.
    ///
    /// Result (2026-10-04): passes.
    #[test]
    fn constructors_are_recognised() {
        assert!(is_constructor("pub struct Coord(u32, u32);", "Coord"));
        assert!(is_constructor("Sphere(SphereData),", "Sphere"));
        assert!(is_constructor("Absorb { weight: f64 },", "Absorb"));
        assert!(!is_constructor("let f = |x| x + 1;", "f"));
        assert!(!is_constructor("static F: fn() = g;", "F"));
    }
}
