//! # kovan-tui
//!
//! The **human-facing** entry point to KOVAN: a terminal UI for browsing
//! literature, repositories, and generated knowledge. Agents should use the
//! `kovan` CLI instead (see `kovan-cli`).
//!
//! Built on [`ratatui`]. TUI is desktop scope — on Android the binary compiles
//! to a stub that redirects to the CLI, keeping the workspace Android-buildable
//! (see the root `CLAUDE.md` "Android portability" rule). The entire [`tui`]
//! module tree lives behind `cfg(not(target_os = "android"))` on the single
//! `mod tui;` declaration below, so none of its submodules need to repeat the
//! gate.
//!
//! ## Screens
//!
//! Five tabs, switched with `1`-`5` or `Tab`/`Shift+Tab`:
//!
//! 1. **Overview** — static module map (the original placeholder screen).
//! 2. **Browser** (`kovan_discovery`) — walk a repository root, filter by
//!    [`kovan_discovery::FileKind`], and navigate the discovered files.
//! 3. **Symbols** (`kovan_semantics`) — catalogue a repository's symbols with
//!    the ripgrep-first extractor and preview either the raw list or the
//!    generated `symbols.md` Markdown artifact.
//! 4. **Methods** (`kovan_codegen`) — browse the numerical-method catalogue by
//!    family and preview a method's generated source.
//! 5. **Literature** (`kovan_literature`) — list PDFs / Markdown / BibTeX under
//!    a literature root and preview each one (metadata extraction, heading
//!    outline, or raw text).
//!
//! Every screen is a **viewer only** — it reads the filesystem and renders
//! deterministic output from the sibling `kovan-*` crates; it never writes to
//! the repositories it browses (KOVAN's "not a repository modification agent"
//! non-goal, `docs/kovan.md` § "Non-Goals").

#[cfg(not(target_os = "android"))]
fn main() -> std::io::Result<()> {
    tui::run()
}

/// On Android there is no TUI; point the user at the agent-facing CLI.
#[cfg(target_os = "android")]
fn main() {
    eprintln!("kovan-tui is desktop-only. Use the `kovan` CLI on Android/Termux.");
}

#[cfg(not(target_os = "android"))]
mod tui;
