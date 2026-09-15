//! # kovan-tui
//!
//! The **human-facing** entry point to KOVAN: a terminal UI for browsing
//! literature, repositories, and generated knowledge. Agents should use the
//! `kovan-cli` CLI instead; the GUI is `kovan` (desktop-only).
//!
//! Built on [`ratatui`]. **Genuinely Android/Termux-usable, not just
//! buildable** — this binary has no Android stub. Until 2026-08-21 the whole
//! [`kovan::tui`] module tree lived behind `cfg(not(target_os =
//! "android"))`, because at the time `ratatui` was itself an Android-gated
//! dependency of this crate. That gate was lifted the same day the digitiser
//! moved in (`ratatui` became an unconditional dependency to keep the former
//! `kovan-digitise-tui` binary's Android behaviour — see this crate's
//! `NOTICE`/`Cargo.toml`), which also made the *whole* TUI, not just the
//! digitiser, buildable and runnable on Android. Confirmed 2026-08-21:
//! `cargo check -p kovan --all-targets --target aarch64-linux-android` is
//! clean with `pub mod tui;` unconditional in `src/lib.rs`. This matters
//! directly for GitHub issue #30's final interface spec, which asked for
//! exactly these three binaries specifically so `kovan` stays usable on
//! Android — a stubbed `kovan-tui` would have quietly broken that on the
//! Digitiser tab (the former standalone `kovan-digitise-tui` binary it
//! absorbed *was* Android-functional).
//!
//! ## Screens
//!
//! Seven tabs, switched with `1`-`7` or `Tab`/`Shift+Tab`:
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
//! 6. **Ingest** (`kovan_literature`) — interactive PDF import: extract,
//!    review the metadata, and write Markdown/JSON/BibTeX.
//! 7. **Digitiser** (`kovan::digitiser`) — interactive graph digitisation:
//!    an automatic trace pass, then a terminal review screen (nudge/delete/
//!    duplicate points, mark reviewed, save). Absorbed the standalone
//!    `kovan-digitise-tui` binary on 2026-08-21 (GitHub issue #30's
//!    3-binary consolidation).
//!
//! Every screen but Ingest and Digitiser is a **viewer only** — it reads the
//! filesystem and renders deterministic output from the sibling `kovan-*`
//! crates; it never writes to the repositories it browses (KOVAN's "not a
//! repository modification agent" non-goal, `docs/kovan.md` § "Non-Goals").
//! Ingest and Digitiser are the two screens that write files, and only when
//! the user explicitly presses their save key.

fn main() -> std::io::Result<()> {
    kovan::tui::run()
}
