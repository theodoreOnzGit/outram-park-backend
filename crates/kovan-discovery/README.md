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
| `zotero` module (`Search`, `SearchLibrary`, `quick_search`) | Zotero's search engine (conditions, operators, groups, result levels, quick search), evaluated in memory over a `kovan_common::zotero::ZoteroLibrary`. See [Zotero search](#zotero-search). |
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

## Zotero search

`kovan_discovery::zotero` ports Zotero's search (GitHub #751, epic #747):
`chrome/content/zotero/xpcom/data/search.js` and `searchConditions.js`, the
full-text matching of `fulltext.js`, `normalizeForSearch`, and the items
pane's quick search (`collectionTreeRow.js`), from Zotero commit
9cbba8c4d281 (AGPL-3.0; see [`NOTICE`](NOTICE)). Zotero builds SQL and lets
SQLite run it; this port evaluates the same rules in memory, replacing each
piece of SQL by the set of rows it selects. Results are item keys, sorted;
the clock is an input, so a search is deterministic. **Maturity: AI draft.**

```rust
use kovan_discovery::zotero::{Search, SearchClock, SearchLibrary};

let lib = SearchLibrary::new(&zotero_library, SearchClock::utc(now));
let mut s = Search::new();
s.add_condition("joinMode", "any", "")?;
s.add_condition("title", "contains", "neutron")?;
s.add_condition("tag", "is", "HTGR")?;
let keys: Vec<String> = s.run(&lib)?;
```

What is equivalent to upstream and what is not (the full table, with the SQL
each condition generates, is in the module docs):

| Conditions | Status |
|---|---|
| `joinMode`, groups, `resultLevel` (cross-level mapping), `deleted`/`includeDeleted`, `noChildren`, `unfiled`, `publications`, `includeParents*`, `recursive`, `collection`, `savedSearch` | equivalent |
| every field (`title`, ..., with base-field mapping), `anyField`, `titleCreatorYear`, `year`, date fields, number fields, creators, `tag`, `itemType`, the `num*` counts, `key`, `dateAdded`/`dateModified`/`lastRead`, `attachmentStorageType`, `fileTypeID`, `annotationText`/`Comment`/`Type`/`Color` | equivalent (accent and case folding as `normalizeForSearch`; SQLite `LIKE` semantics, `%` and `_` included) |
| quick search: `titleCreatorYear`, `fields`, `everything`, `titleCreatorYearNote` | equivalent expansion |
| `note` | approximate: the note HTML's plain text comes from a simple tag stripper, not Mozilla's converter |
| `fulltextContent` (and `/regexp`) | only over text supplied with `SearchLibrary::set_full_text` (no index exists for a `ZoteroLibrary`); Rust regex syntax |
| `retracted`, `feed` | no data in a `ZoteroLibrary`: match nothing |
| `libraryID`, `itemID`, `itemTypeID`, `tagID`, `tempTable`, `annotationAuthor` | unsupported (database ids, temporary tables, group-user ids): an error |

**Verification.** Upstream Zotero desktop cannot run here, and the
translation server at 127.0.0.1:1969 does not expose search, so Zotero's own
`test/tests/searchTest.js` is the reference. `tests/zotero_search.rs` ports
it: of its 128 cases, 118 are ported with each test's database objects
rebuilt as a `ZoteroLibrary` fixture and the expected results unchanged, one
more is covered by an equivalent ported case, and 9 are not ported (database
persistence, SQL bind parameters, a schema migration). **All ported cases
pass (2026-10-07, `cargo test --release -p kovan-discovery`).**

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

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)). Part of the [OUTRAM PARK](../../README.md) workspace.
