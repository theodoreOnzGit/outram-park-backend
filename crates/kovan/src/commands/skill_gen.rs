//! `kovan-cli skill-gen` — write a Claude Code Skill-format Markdown file
//! documenting `kovan-cli`'s commands for an AI agent (GitHub issue #32:
//! "we shld be able to have kovan-cli generate skill.md also for AI agents
//! to read").
//!
//! A native subcommand rather than a companion script, per this workspace's
//! "build it into kovan, don't reach for a script" direction (the same
//! reasoning `kovan agent-docs-gen`/`kovan api-docs` already follow) —
//! mirrors the *shape* of kopitiam's own `scripts/gen-kopitiam-skill.sh` ->
//! `kopitiam_skill.md` (frontmatter, a "CRITICAL agent guidance" section, then
//! recipes) without depending on that script or its output.
//!
//! The content below is a hand-maintained template, not introspected from the
//! `clap::Command` tree — keep it in sync by hand when a subcommand's shape
//! changes materially (a mechanical `--help`-driven generator is a
//! reasonable follow-on, not implemented here).

use std::path::{Path, PathBuf};

/// Default output path when `--out` is not given.
pub const DEFAULT_OUT: &str = "kovan_skill.md";

const SKILL_MD: &str = r#"---
name: kovan-cli
description: Use when you need token-frugal repository understanding, literature/PDF cataloguing, numerical-method code generation, or graph digitisation in the OUTRAM PARK workspace, driven non-interactively from a script or coding agent.
---

# kovan-cli skill

## What kovan-cli is

`kovan-cli` is the agent-facing front end to KOVAN, this workspace's own
deterministic knowledge layer: repository discovery/search/scan, a
ripgrep-first symbol/outline extractor, a PDF literature pipeline, numerical
method code generation, per-commit token accounting, and a fully automatic
graph digitiser. Every subcommand is non-interactive, offline, and
deterministic — plain flags in, line-oriented (or `--json`) output out.

## CRITICAL agent guidance (read first)

- **Only run `kovan-cli` from an agent.** The other two binaries in this
  crate are INTERACTIVE and will HANG a non-interactive session: `kovan` is
  the GUI (an `eframe` window — the process never returns without a display
  and a human closing it), and `kovan-tui` is a full-screen `ratatui`
  terminal UI (it owns the terminal and blocks on keyboard input). Neither
  has a script-friendly mode. If a task looks like it needs the digitiser or
  PDF reader, use `kovan-cli digitise` (fully automatic) instead of reaching
  for the GUI.
- **Never run `kovan-cli lsp-daemon-serve` directly.** It is the keep-warm
  daemon's own foreground process — it blocks until stopped and will HANG a
  non-interactive session exactly like the GUI/TUI would. `def`/`sig`/`refs`
  spawn and detach it for you automatically; the only daemon subcommand meant
  to be run by hand is `lsp-daemon-stop`.
- **Prefer the `cost -> outline -> slice` loop over reading a whole file
  blind.** Check what a file would cost first, read only its declarations if
  that's enough, and slice out just the lines you actually need otherwise.
- **`lit`/`digitise` are the literature and graph-digitisation paths** — see
  their recipes below rather than reading a PDF's raw bytes or eyeballing a
  figure.

## Recipes

Estimate a file's token cost before deciding whether to read it whole:

```bash
kovan-cli cost src/big_module.rs
kovan-cli cost src/big_module.rs --by-line   # which lines dominate the cost
```

Get a declarations-only skeleton of one file instead of reading it whole:

```bash
kovan-cli outline src/big_module.rs --lang rust
```

Read only a line range once the outline says where the part you need is:

```bash
kovan-cli slice src/big_module.rs 120 180
```

Rust-analyzer-backed semantic queries (need `rust-analyzer` on PATH). The
**first** call for a given workspace root indexes it and can take up to a
couple of minutes; a background daemon then keeps that index warm, so every
later call (from any `kovan-cli` invocation) answers in well under a second
until you stop it:

```bash
kovan-cli def foo --file src/lib.rs      # where it's defined, plus its signature
kovan-cli sig foo --file src/lib.rs      # just the signature
kovan-cli refs foo --file src/lib.rs     # every reference site, as coordinates
kovan-cli lsp-daemon-stop --root .       # stop the warm daemon for this root
```

Discover files under a root, honouring `.gitignore`:

```bash
kovan-cli discover --root . --kind source
```

Regex search a single file or a whole repository:

```bash
kovan-cli search --path src/lib.rs --pattern 'fn \w+'
kovan-cli search --root . --kind source --pattern 'fn \w+'
```

Catalogue a repository's symbols, or render the full Markdown artifact:

```bash
kovan-cli symbols . --lang rust
kovan-cli symbols . --lang rust --markdown --out symbols.md
kovan-cli summary . --lang rust --out repository-summary.md
```

Import a PDF into the literature archive and get its BibTeX entry:

```bash
kovan-cli lit import paper.pdf --json-out doc.json --markdown-out doc.md
kovan-cli lit bibtex doc.json
kovan-cli lit outline paper.pdf
```

Generate numerical-method source from the codegen catalogue:

```bash
kovan-cli methods
kovan-cli gen root newton-raphson
```

Digitise a plot fully automatically (never do this by eyeballing a figure):

```bash
kovan-cli digitise --image fig7.png --x-scale log --x-range 1,1e6 \
    --y-scale log --y-range 0.1,10 --figure "Fig. 7" --json fig7.json
```

The emitted dataset is always `UNREVIEWED` — a human marks it reviewed in
`kovan-tui`'s Digitiser tab, never from an agent session.

## Command reference

| Command | Agent-safe | Notes |
|---|---|---|
| `cost` | yes | Token-cost estimate (`kopitiam-tokenizer`-backed) |
| `outline` | yes | Declarations-only skeleton, ripgrep-first |
| `slice` | yes | Print one line range |
| `def` / `sig` / `refs` | yes, but needs `rust-analyzer` | Semantic queries (definition/signature/references); kept warm by a background daemon until `lsp-daemon-stop` |
| `lsp-daemon-stop` | yes | Stops the warm rust-analyzer daemon for a root |
| `discover` / `search` / `scan` | yes | Repository discovery/search |
| `symbols` / `summary` | yes | Symbol catalogue / Markdown artifact |
| `lit` | yes | PDF import / BibTeX / literature outline |
| `gen` / `methods` | yes | Numerical-method code generation |
| `digitise` | yes | Fully automatic graph digitiser |
| `tokens` / `historian` | yes | Per-commit API-token accounting |
| `agent-docs-gen` / `api-docs` / `kloc` | yes | Documentation/accounting generators |
| `setup` | yes, but online | Installs external tools via `cargo install` |
| `kovan` (GUI) | **NO — hangs** | Interactive `eframe` window |
| `kovan-tui` | **NO — hangs** | Interactive full-screen terminal UI |
"#;

/// Write [`SKILL_MD`] to `out` (or [`DEFAULT_OUT`] if `None`).
///
/// # Errors
///
/// A message if the file cannot be written.
pub fn run(out: Option<PathBuf>) -> Result<(), String> {
    let out = out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    write(&out)
}

fn write(out: &Path) -> Result<(), String> {
    std::fs::write(out, SKILL_MD)
        .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: write to a temp file and check the frontmatter + the
    /// GUI/TUI hang warning both round-trip -- the two things this command
    /// exists to guarantee an agent sees.
    ///
    /// Result (2026-08-22): both present.
    #[test]
    fn generated_skill_carries_frontmatter_and_the_hang_warning() {
        let dir = std::env::temp_dir();
        let out = dir.join(format!("kovan_skill_test_{}.md", std::process::id()));
        run(Some(out.clone())).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("---\nname: kovan-cli"));
        assert!(text.contains("will HANG a non-interactive session"));
        std::fs::remove_file(&out).ok();
    }
}
