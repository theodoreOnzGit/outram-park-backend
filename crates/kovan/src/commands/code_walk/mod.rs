//! `kovan-cli code-walk` and `code-walk-check` — call chains from an entry
//! point down to the function that implements a concept, and exhaustive call
//! trees, for the deep dives and tutorials (GitHub issue #523, part of #509).
//!
//! This is kovan's long-deferred `callees` (historical `op-l3uz`), built on
//! the same rust-analyzer plumbing as `def`/`sig`/`refs` (`super::semq`, the
//! warm `super::lsp_daemon`) and ported from kopitiam's `callees`
//! (`apps/cli/src/semq.rs` in the maintainer's `kopitiam` repository, commit
//! `dfdf1c4`, AGPL-3.0, the same licence as this crate). What changed in the
//! port, and why, is in `builder`'s and `source`'s module docs.
//!
//! # Modes
//!
//! - **Concept path** — `--from <file>::<fn> --to <file>::<fn>`: the shortest
//!   chain(s) of workspace calls between the two, breadth-first, std and
//!   dependencies filtered out. `--depth` caps the search (default 12).
//! - **Exhaustive tree** — `--from <file>::<fn>` alone: everything reachable
//!   within `--depth` hops (default 3), each function expanded once and every
//!   later call to it a back-reference.
//!
//! A function is named `path/to/file.rs::name`, or `...::Type::name` for a
//! method when the bare name is ambiguous in that file.
//!
//! # Output
//!
//! Markdown (a nested list for mdBook `{{#include}}`; in a lesson block, a
//! concept path is numbered steps with each hop's code inline, see
//! `render`'s module doc), Mermaid, or JSON. Each
//! hop carries a permalink with the `@@COMMIT@@` placeholder the Pages build
//! fills in, its signature, the first sentence of its doc comment and the
//! line it is called from. A call the tool cannot follow — a trait method, a closure or fn
//! pointer, a workspace macro, a token rust-analyzer cannot resolve — is
//! marked `UNRESOLVED(<kind>)`, never guessed and never dropped. Calls inside
//! closures (including `rayon` closures) are ordinary calls in the body and
//! are followed; a function passed by name (`.map(f)`) is resolved as a
//! function value.
//!
//! # Filling gaps by hand
//!
//! The tool generates; a lesson-writing agent fills the gaps. A hop it
//! filled goes on a `hand:` line (`--hand` on the command line, or in the
//! lesson block's opening comment — see `lesson`); it is added to the graph
//! before the search, labelled `filled by hand` in every format, and
//! re-verified on every check: if either endpoint stops existing, the check
//! fails. A hand hop the tool now resolves itself is reported as redundant.

pub(crate) mod builder;
pub(crate) mod graph;
pub(crate) mod lesson;
pub mod render;
pub(crate) mod source;

use std::path::{Path, PathBuf};

use builder::{HandHop, Workspace};
use render::Header;

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Format {
    /// Nested Markdown list (numbered steps with inline code for a concept
    /// path in a lesson block).
    #[default]
    Markdown,
    /// A Mermaid flowchart in a fenced block.
    Mermaid,
    /// Markdown followed by the Mermaid chart.
    Both,
    /// The explored graph, chains and gaps as JSON.
    Json,
}

/// One walk's parameters, from the command line or a lesson block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WalkSpec {
    pub(crate) from: String,
    pub(crate) to: Option<String>,
    pub(crate) depth: Option<usize>,
    pub(crate) format: Format,
    pub(crate) max_paths: usize,
    pub(crate) hand: Vec<HandHop>,
}

impl Default for WalkSpec {
    fn default() -> Self {
        WalkSpec {
            from: String::new(),
            to: None,
            depth: None,
            format: Format::Markdown,
            max_paths: DEFAULT_MAX_PATHS,
            hand: Vec::new(),
        }
    }
}

const DEFAULT_PATH_DEPTH: usize = 12;
const DEFAULT_TREE_DEPTH: usize = 3;
const DEFAULT_MAX_PATHS: usize = 8;
/// Function cap: a search that knows more functions than this stops after
/// the current layer and says it was truncated.
const MAX_NODES: usize = 4000;

/// The rendered walk plus what the check mode needs to judge it.
pub(crate) struct WalkOutput {
    pub(crate) text: String,
    /// Path mode: whether a chain was found (always true in tree mode).
    pub(crate) connected: bool,
    pub(crate) redundant_hand: Vec<String>,
}

/// `lesson` is the Markdown file the walk is written into: a concept path
/// for a lesson shows each hop's code inline (`render::Inline`).
pub(crate) fn generate(
    ws: &mut Workspace,
    spec: &WalkSpec,
    repo_url: &str,
    lesson: Option<&Path>,
) -> Result<WalkOutput, String> {
    let mut walk = ws.start(&spec.hand)?;
    let from = ws.locate(&spec.from)?;
    let from = walk.graph.intern(from);
    let to = match &spec.to {
        Some(t) => {
            let n = ws.locate(t)?;
            Some(walk.graph.intern(n))
        }
        None => None,
    };
    let depth = spec.depth.unwrap_or(if to.is_some() {
        DEFAULT_PATH_DEPTH
    } else {
        DEFAULT_TREE_DEPTH
    });
    // `depth` layers are expanded; functions first reached at the limit are
    // shown but not entered (tree mode says so on each of them).
    ws.bfs(&mut walk, from, to, depth, MAX_NODES)?;
    let inline = match (lesson, to) {
        (Some(md), Some(_)) => Some(render::Inline {
            to_root: path_to_root(&ws.root, md)?,
            spans: (0..walk.graph.nodes.len())
                .map(|i| {
                    let n = &walk.graph.nodes[i];
                    let (file, line) = (n.file.clone(), n.line);
                    ws.span(&file, line)
                })
                .collect(),
        }),
        _ => None,
    };
    let header = Header {
        from: spec.from.clone(),
        to: spec.to.clone(),
        depth,
        repo_url: repo_url.to_string(),
        inline,
    };
    let g = &walk.graph;
    let (lines, chains, connected) = match to {
        Some(t) => {
            let chains = graph::shortest_paths(g, from, t, spec.max_paths);
            let connected = !chains.is_empty();
            (graph::chains_tree(g, &chains), Some(chains), connected)
        }
        None => (graph::tree(g, from, depth), None, true),
    };
    let md = || match &chains {
        Some(c) => render::markdown_path(&header, g, c),
        None => render::markdown_tree(&header, g, &lines, walk.truncated),
    };
    let text = match spec.format {
        Format::Markdown => md(),
        Format::Mermaid => render::mermaid(g, &lines),
        Format::Both => format!("{}\n{}", md(), render::mermaid(g, &lines)),
        Format::Json => render::json(&header, g, chains.as_deref(), walk.truncated, walk.external_calls),
    };
    Ok(WalkOutput {
        text,
        connected,
        redundant_hand: walk.redundant_hand,
    })
}

/// `../../../` from the directory holding `md` up to the workspace `root`,
/// for mdBook `{{#include}}` paths (which are relative to the page).
fn path_to_root(root: &Path, md: &Path) -> Result<String, String> {
    let abs = std::fs::canonicalize(md).map_err(|e| format!("{}: {e}", md.display()))?;
    let dir = abs
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", md.display()))?;
    let rel = dir
        .strip_prefix(root)
        .map_err(|_| format!("{} is outside the workspace {}", md.display(), root.display()))?;
    Ok("../".repeat(rel.components().count()))
}

/// `kovan-cli code-walk`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    from: String,
    to: Option<String>,
    depth: Option<usize>,
    format: Format,
    max_paths: usize,
    hand: Vec<String>,
    root: PathBuf,
    repo_url: String,
) -> Result<(), String> {
    let hand = hand
        .iter()
        .map(|h| lesson::parse_hand(h))
        .collect::<Result<Vec<_>, _>>()?;
    let spec = WalkSpec {
        from,
        to,
        depth,
        format,
        max_paths,
        hand,
    };
    let mut ws = Workspace::open(&root)?;
    let out = generate(&mut ws, &spec, &repo_url, None);
    ws.close();
    let out = out?;
    println!("{}", out.text.trim_end());
    for r in &out.redundant_hand {
        eprintln!("kovan-cli code-walk: hand-filled hop {r} is now resolved by the tool; it can be removed");
    }
    if !out.connected {
        return Err("no call chain found (gaps listed above)".to_string());
    }
    Ok(())
}

/// `kovan-cli code-walk-check` — regenerate every code-walk block under
/// `paths` and fail if any is stale or broken (`update` writes instead of
/// failing on a mere difference).
pub fn run_check(paths: Vec<PathBuf>, update: bool, root: PathBuf, repo_url: String) -> Result<(), String> {
    let files = lesson::markdown_files(&paths);
    let mut work: Vec<(PathBuf, String, Vec<lesson::Block>)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        if !text.contains(lesson::OPEN) {
            continue;
        }
        match lesson::find_blocks(&text) {
            Ok(b) if !b.is_empty() => work.push((f, text, b)),
            Ok(_) => {}
            Err(e) => failures.push(format!("{}: {e}", f.display())),
        }
    }
    let total: usize = work.iter().map(|w| w.2.len()).sum();
    if total == 0 && failures.is_empty() {
        println!("code-walk-check: no code-walk blocks found");
        return Ok(());
    }
    let mut ws = Workspace::open(&root)?;
    let (mut ok, mut stale) = (0usize, 0usize);
    for (file, text, blocks) in &work {
        let mut new_text = String::new();
        let mut cursor = 0usize;
        let mut changed = false;
        for b in blocks {
            let place = format!("{}:{}", file.display(), b.line);
            let current = &text[b.body.0..b.body.1];
            let fresh = match generate(&mut ws, &b.spec, &repo_url, Some(file.as_path())) {
                Ok(out) => {
                    for r in &out.redundant_hand {
                        println!("note {place}: hand-filled hop {r} is now resolved by the tool");
                    }
                    if !out.connected {
                        failures.push(format!(
                            "{place}: chain broken — no path from {} to {}",
                            b.spec.from,
                            b.spec.to.as_deref().unwrap_or("")
                        ));
                    }
                    Some(lesson::body_text(&out.text))
                }
                Err(e) => {
                    failures.push(format!("{place}: {e}"));
                    None
                }
            };
            let Some(fresh) = fresh else { continue };
            if fresh.trim() == current.trim() && b.closed {
                ok += 1;
                continue;
            }
            stale += 1;
            if update {
                new_text.push_str(&text[cursor..b.header.1]);
                new_text.push_str(&fresh);
                if !b.closed {
                    new_text.push_str(lesson::CLOSE);
                    new_text.push('\n');
                }
                cursor = b.body.1;
                changed = true;
                println!("updated {place}");
            } else {
                failures.push(format!(
                    "{place}: stale — regenerated walk differs from the committed one\n{}",
                    lesson::line_diff(current, &fresh, 12)
                ));
            }
        }
        if changed {
            new_text.push_str(&text[cursor..]);
            std::fs::write(file, new_text).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    ws.close();
    println!(
        "code-walk-check: {total} block(s), {ok} current, {stale} {}",
        if update { "updated" } else { "stale" }
    );
    if failures.is_empty() {
        return Ok(());
    }
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    let hint = if update {
        ""
    } else {
        " (run `kovan-cli code-walk-check --update <paths>` to regenerate, after checking the change is intended)"
    };
    Err(format!("{} code-walk problem(s){hint}", failures.len()))
}
