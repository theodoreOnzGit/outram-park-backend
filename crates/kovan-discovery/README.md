# kovan-discovery

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
science and engineering. This crate is part of KOVAN: its file discovery, text
search and read-only git layer.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

Offline, deterministic **file discovery + text search** for KOVAN — the layer
beneath `kovan-semantics`. Before any language-native tooling runs, KOVAN needs
to find files and grep their contents. This crate does exactly that, built on
two mature Rust engines:

- [`ignore`](https://docs.rs/ignore) — the `.gitignore`-aware directory walker
  behind `fd` / ripgrep.
- [`grep-searcher`](https://docs.rs/grep-searcher) + `grep-regex` — the ripgrep
  search engine.

No index database, no network access, no hidden state. Given the same tree and
arguments, every function returns the same result on every call and every
platform (Linux, Windows, macOS, Android/Termux).

## What it provides

| Function | Purpose |
|---|---|
| `discover` / `discover_kind` | Enumerate files under a root, honouring `.gitignore`, optionally filtered to a `FileKind` (source, Markdown, PDF, metadata). |
| `search_file` | ripgrep-style regex search of a single file — line number, 1-based character column, and text per match. |
| `search_repository` | Discover + search in one deterministic pass. |
| `git` module (`GitProvider`, `GixBackend`, `GixCliBackend`) | Read-only git awareness on the pure-Rust [`gix`](https://docs.rs/gix) library: repository root, `HEAD`, tracked files, per-path history, last commit and per-line blame, worktree-dirty check. Local `.git` only, no network. |

Results are always **sorted by path**, so callers get a stable order regardless
of the host filesystem's raw directory-entry order.

## `.gitignore` behaviour

`.gitignore` rules are honoured **even when the target directory is not inside a
git repository** — a bare `.gitignore` in any directory (a downloaded tarball, a
vendored source tree, a literature staging directory) is respected, because that
is what `.gitignore` is meant to do.

Internally this is `WalkBuilder::require_git(false)`, a deliberate deviation from
the `ignore` crate's default (`require_git = true`), which would otherwise treat
`.gitignore` as **inert** outside a real `.git` repository — silently breaking
the "honours `.gitignore`" contract for non-repo trees. `.ignore` files and
global git excludes are honoured either way, and inside a git repository the
behaviour is unchanged.

## Example

See [`examples/discover_and_search.rs`](examples/discover_and_search.rs) for a
top-to-bottom discover + search walkthrough:

```bash
cargo run -p kovan-discovery --release --example discover_and_search
```

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
