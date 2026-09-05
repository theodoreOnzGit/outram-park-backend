# KOVAN — Agent Decisions for Human Review

**Date:** 2026-07-15
**Scope:** best-effort implementation pass across all seven `crates/kovan-*`
crates, fleshing the `// TODO(kovan)` scaffolds into working functionality.
One agent per crate (per the `docs/kovan.md` "Agent Roles" A–K decomposition),
run in two waves (five library crates, then `kovan-cli` / `kovan-tui`).

This file is the **review hub**: it summarises the non-trivial decisions each
agent made and collects the items that need a human call. Every crate also has
its own full `crates/kovan-<crate>/DECISIONS.md` — linked per section — with the
complete rationale.

> All seven crates build, test, clippy (`-D warnings`), `fmt --check`, `doc`,
> and `cargo check --target aarch64-linux-android` **clean**. Combined new/updated
> test count: **198** (common 15 · discovery 12 · literature 25 · semantics 26 ·
> codegen 37 · cli 39 · tui 44). No commits/pushes were made by the agents; no
> real or proprietary data was used (all fixtures synthetic).

---

## ⚠️ Decisions that need a human call

1. **`kovan-common` type additions requested by three downstream crates** — the
   single most common theme. Common was treated as a stable contract and left
   un-edited by the others; they worked around gaps locally and reported what
   they need:
   - **`KovanSymbol` needs `file` / `line` / `language` fields** (from
     `kovan-semantics`, which had to keep a crate-local `ExtractedSymbol` because
     the shared type carries no location). Also proposed: a `KovanModule` type and
     an enum `kind`.
   - **`KovanDocument` additions** (from `kovan-literature`): `assets: Vec<String>`,
     `page_count`, a source path/hash, journal locator fields
     (`volume`/`pages`/`number`), and a builder.
   - **A `GeneratedArtifact` provenance record** (from `kovan-codegen`) to wire the
     Paper→Correlation→Implementation vision.
   - `kovan-common` itself deferred **`Display`/`FromStr` on the enums** and a
     **result/measurement type on `KovanValidationCase`** until a first consumer
     reveals the right shape.
   → **RESOLVED (v2 pass, 2026-07-15):** `KovanSymbol` gained `file`/`line`/`language`
   (+ a `Language` enum); `KovanDocument` gained `volume`/`pages`/`number`/`source_path`/
   `source_sha256`/`page_count`/`assets` + a `KovanDocumentBuilder`; `GeneratedArtifact`
   was added and wired into `kovan-codegen`. Downstream crates rewired; all 7 crates
   green. **Still deferred:** `KovanModule` and a typed `kind` enum (no consumer yet);
   and `source_sha256` *auto-population* (needs `sha2.workspace = true` in
   kovan-literature — `sha2` is already a workspace dep — the field itself exists and
   is builder-settable). See `crates/kovan-common/DECISIONS.md` §"v2 pass".

2. **`kovan-discovery` behavior change — second opinion wanted.** A real bug was
   fixed: `.gitignore` rules were inert outside a `.git` repo (the `ignore`
   crate's `require_git: true` default), contradicting the crate's own docs. Fixed
   with `.require_git(false)`. This changes behavior for callers pointing at a
   non-repo directory (they now honour a bare `.gitignore`). No effect on in-repo
   call sites. Flagged for your confirmation.

3. **New third-party workspace dependencies added** (root `[workspace.dependencies]`;
   all pure-Rust, offline, Android-friendly, GPLv3-compatible):
   - `regex` — `kovan-semantics` symbol scanner (already pulled transitively by
     `grep-regex`).
   - `pdf-extract` `0.12` + `lopdf` `0.42` — `kovan-literature` PDF text/metadata.
   - `tempfile` `3.14` (dev-only) — fixture trees for discovery/cli/tui tests.
   → Please sanity-check these are acceptable additions to the minimal KOVAN stack.

4. **`Method::Pde(PdeScheme)` added to the `kovan-codegen` catalogue enum** — the
   spec lists PDE under Numerical Methods but the enum lacked it. Additive; the
   enum stays closed; downstream `kovan-cli`/`kovan-tui` updated.

---

## Per-crate decisions

### kovan-common — Core Types (Agent B) · [DECISIONS](../crates/kovan-common/DECISIONS.md)
- The lone `// TODO(kovan)` was stale boilerplate module-doc text (not an
  unimplemented function) — replaced with an accurate "Maturity" note.
- Added `#[serde(default)]` to every `Vec<T>` field of `KovanDocument` so an older
  serialised document still deserialises after the struct grows a field
  (regression test locks this in).
- Added `Hash` to `Visibility`/`DocumentType`. Documented the organisational-author
  convention (`Author{given: ""}`).
- **Deferred:** enum `Display`/`FromStr`, `KovanValidationCase` result type (see call #1).

### kovan-discovery — file discovery + search · [DECISIONS](../crates/kovan-discovery/DECISIONS.md)
- Quality pass only (crate was already real). **Fixed the `require_git` bug** (call #2).
- Additive API: `SearchMatch.column`, `search_repository(root, kind, pattern)`;
  `discover` output now explicitly sorted for cross-platform determinism.
- **Open:** an unused `kovan-common` dep declared in its `Cargo.toml`; whether
  `kovan-semantics` should rewire onto `search_repository`.

### kovan-literature — PDF→MD→KovanDocument→BibTeX (Agents C/D) · [DECISIONS](../crates/kovan-literature/DECISIONS.md)
- **PDF extractor: `pdf-extract` 0.12, wired (not stubbed)** — pure-Rust, offline,
  Android-clean; extraction wrapped in `catch_unwind` (it can panic on malformed
  PDFs). `lopdf` for Info-dict metadata + raw image assets.
- Assets = honest partial (only DCTDecode→`.jpg` / JPXDecode→`.jp2`, other codecs
  reported-skipped, not fabricated). Metadata: Info-dict first, conservative text
  fallbacks, unknown fields left `None` (no body-text author guessing).
- `KovanDocument` stays authoritative; BibTeX generated from it
  (`Paper→article`, `Report→techreport`, `Manual→manual`, else `misc`).
- Storage-path `Visibility` inferred from a `proprietary/` path component —
  deterministic, so material can't be mislabelled "open".

### kovan-semantics — Rust/C++/Python/Fortran (Agents E/F/G/H) · [DECISIONS](../crates/kovan-semantics/DECISIONS.md)
- Real, offline, ripgrep-first per-language regex extractors with a light scope
  stack (qualified names like `module::Type::method`). **No Tree-sitter** (per spec),
  no compiler reimplementation.
- **Language-server adapters** (rust-analyzer/clangd/Pyright/fortls) **scaffolded and
  deferred** behind a `language-servers` feature + `cfg(not(target_os="android"))`;
  the LSP client itself returns `Unimplemented`. Default build stays Android-clean.
- Known limit (documented): C++ most-vexing-parse false positives — exactly what the
  clangd escalation path is meant to fix later.
- **Needs `KovanSymbol` location fields** (call #1).

### kovan-codegen — Code Generation (Agent I) · [DECISIONS](../crates/kovan-codegen/DECISIONS.md)
- **Deterministic string templating (`include_str!`), not proc-macros** — pure
  `core`/`std`, offline, Android-clean, every artifact diffable.
- **Verification link:** each template is *both* emitted by `generate()` and compiled
  into a tested `reference` module from the same file, so tests exercise the exact
  emitted bytes; a test asserts they can't drift.
- Fully generated + tested: root finders (bisection/regula-falsi/secant/Newton/Brent),
  dense LU, fixed-point + Newton-for-systems, ODE (Euler/RK2/RK4/backward-Euler),
  1-D FD Poisson, 3 engineering patterns, a `kovan_fixed_point!` declarative macro.
  Remaining catalogue entries return `Unimplemented`. Derive/attribute/proc-macros
  emit companion-crate skeletons (can't live in a lib crate).
- Added `Method::Pde` (call #4).

### kovan-cli — agent-facing `kovan` CLI · [DECISIONS](../crates/kovan/DECISIONS.md)
- New commands: `symbols`, `summary`, `gen` (nested root/linear/nonlinear/ode/pde),
  `lit import|bibtex|outline`, repo-wide `search --root`; `methods` extended for PDE.
- `clap::ValueEnum` mirrors of the library catalogue enums (rather than deriving
  `ValueEnum` on the lib enums) to keep the CLI's surface decoupled.
- Line-oriented deterministic output (the CLI's purpose — parseable by coding agents).
- **Friction:** `kovan_semantics::outputs` is a private module (only its two fns are
  re-exported). Left out: `lit assets`, a general `--json` mode.

### kovan-tui — human-facing `ratatui` TUI · [DECISIONS](../crates/kovan/DECISIONS.md)
- Grew from the static placeholder to a **five-tab TUI** (overview / repo browser /
  symbol catalogue / method catalogue+preview / literature browser) over the real
  lib functionality. Verified with a live `tmux` smoke test plus 44 headless tests.
- **Android stub preserved** — the entire `tui` module tree is behind
  `cfg(not(target_os="android"))`; on Android it still compiles to the CLI-redirect stub.
- State owned by value (single-threaded event loop) — documented why `Arc<RwLock>` was
  not used here. Left out (not gold-plated): async scanning for very large repos,
  PDF-asset preview.

---

## Beads

The KOVAN epic **`op-5v5` is JSONL-only — absent from the local Dolt store** (part
of the standing `.beads/issues.jsonl` ↔ Dolt sync mismatch). Per instruction, no
agent tried to fix the export or parent children under it; each recorded its
intended follow-up beads inside its own `DECISIONS.md`. **These intended beads
should be filed once the beads sync is reconciled.**

## Data provenance / compliance

No real or proprietary PDFs, papers, or datasets were introduced. Every literature
and CLI/TUI test uses synthetic `lopdf`-built PDFs or temp-dir fixtures. The
`proprietary/` storage tree stays gitignored. All added dependencies are
GPLv3-compatible and pure-Rust.
