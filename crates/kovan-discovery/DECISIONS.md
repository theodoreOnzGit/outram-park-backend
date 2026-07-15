# kovan-discovery — decisions, assumptions, open questions

This is the record of a completeness/quality pass (2026-07-15), not a
rearchitecture. The crate was already real (no `// TODO(kovan)` markers) and
building; this pass strengthened tests/docs/examples and fixed one real
behavioural bug found while writing the tests. Nothing in the public API
surface was removed or renamed; one field was added to an existing struct.

## What changed

- **`SearchMatch` gained a `column: usize` field** (1-based, `char`-counted,
  not byte-counted). Computed by re-locating the already-compiled pattern
  within the matched line via `grep_matcher::Matcher::find`. This is additive
  to the struct's fields but is a **breaking change for anyone constructing
  `SearchMatch` by struct literal** (nobody in this workspace does — checked
  `kovan-semantics`, `kovan-cli`, `kovan-tui`, all only read `.line`/`.text`
  or move the whole struct around).
- **Added `search_repository(root, kind, pattern)`** — the "discover this
  `FileKind`, then search every result" loop, extracted as a named primitive.
  `kovan-semantics::rough_definition_scan` implements the identical loop by
  hand today; it was **not** rewired to call this (out of scope for this
  agent — `kovan-semantics` belongs to a different work item). Flagged below
  as a follow-up.
- **Fixed a real bug: `.gitignore` was silently inert outside an actual `.git`
  repository.** `ignore::WalkBuilder`'s default `require_git` is `true`,
  meaning `.gitignore`/`.git/info/exclude`/global-git-exclude rules are only
  honoured when the walked root is inside a directory that itself contains (or
  is a descendant of) a `.git` folder. A tempdir fixture with a bare
  `.gitignore` and no `.git` therefore leaked `target/`-style ignored content
  — caught by the new `tempfile`-based tests, which failed against the
  original code. Fixed by calling `.require_git(false)` on the `WalkBuilder`
  in `discover`. This does not change behaviour for real invocations against
  this workspace (which does have `.git`); it only makes `.gitignore` honouring
  work in non-repo trees too, matching what the crate's own doc comment
  already promised ("honouring `.gitignore`") before this fix made that
  promise actually true everywhere.
- **`discover`/`discover_kind` output is now explicitly sorted** (`out.sort()`
  on the collected `Vec<PathBuf>`) before returning, and documented as a
  determinism guarantee. Previously the order was whatever `ignore`'s
  single-threaded walk produced, which depends on the OS's raw directory-entry
  (`readdir`) order — not guaranteed stable across filesystems/platforms even
  though it usually looks stable on ext4/Linux. This is the one behavioural
  change beyond the gitignore fix; it is additive (a stronger guarantee than
  the crate previously made, not a narrower one) and does not change which
  files are returned, only their order.
- Rewrote/expanded rustdoc on every public item: module (`//!`), `FileKind`,
  `FileKind::extensions`, `SearchMatch` (+ new field), `DiscoveryError` (+
  clarified that non-UTF-8 files surface as `Io`, not silently mis-decoded),
  `discover`, `discover_kind`, `search_file`, `search_repository`. Each
  function doc now has explicit "Determinism" and (where relevant) "Errors" /
  "Error handling" sections, per the workspace's "human interface layer"
  mandate (navigable by rust-analyzer alone).
- Added `examples/discover_and_search.rs` — runs `discover_kind` → `search_file`
  → `search_repository` top-to-bottom against the crate's own source tree
  (reproducible with no setup), printing `path:line:column: text` in the
  `ripgrep`-CLI convention.
- Added 9 new unit tests using a `tempfile::TempDir` fixture
  (`fixture_repo()`), on top of the original 3 smoke tests against the crate's
  own source (kept, still pass): `.gitignore` include/exclude (file rule and
  whole-directory rule), extension filtering, sorted/stable-order determinism,
  nonexistent-root handling, line/column correctness (including a
  leading-whitespace/indentation case), missing-file I/O-error mapping, and
  `search_repository`'s combined discover+search+gitignore behaviour
  end-to-end. 12/12 pass in `--release`.
- Added `tempfile` as a **new workspace dev-dependency** (root `Cargo.toml`
  `[workspace.dependencies]`, pinned `"3.14"`) and wired it into
  `kovan-discovery`'s `[dev-dependencies]` only. It is pure-Rust and
  Android-friendly; confirmed with `cargo check -p kovan-discovery --tests
  --target aarch64-linux-android` (clean — see Verification below). It never
  reaches the library's non-test build (`cargo check` without `--tests` does
  not compile dev-dependencies), so it cannot affect the Android *library*
  build even in principle.

## Assumptions

- **"Coherent, well-typed, ergonomic" did not require changing existing
  function signatures.** `discover`/`discover_kind` returning a bare `Vec`
  (not `Result`) was kept as-is even though it silently swallows I/O errors
  (nonexistent root, permission-denied subtree) — changing the return type
  would be a breaking API change rippling into `kovan-cli` and
  `kovan-semantics`, and the existing behaviour (best-effort traversal,
  matching `fd`'s own default) is defensible for a "just find the files"
  primitive. Documented the behaviour explicitly instead of changing it.
- **Column counts `char`s, not bytes or grapheme clusters.** This matches
  what most editors show for a cursor position in practice, and is cheap to
  compute from the UTF8 sink's already-decoded `&str` line. It will
  under/over-count relative to a terminal's visual column for combining
  characters or wide (CJK) glyphs — not fixed here; flagged as a known
  limitation in the field's doc comment ("the practically-unreachable case"
  paragraph covers the fallback-to-1 case, but not the visual-width case,
  which is a design choice, not a bug).
- **`search_repository`'s error strategy is fail-fast, not best-effort.** A
  bad pattern or a single non-UTF-8 file aborts the whole scan rather than
  skipping that file and continuing. This matches `search_file`'s existing
  error contract (propagate, never paper over) and the workspace's "Never
  paper over `NonConvergent`-style errors with a default value" guardrail
  spirit — but it does mean one binary/non-UTF-8 file in a large repository
  scan aborts the whole `search_repository` call. Callers that want
  best-effort behaviour are told in the doc comment to call `discover_kind`
  + `search_file` themselves and handle each file's `Result` individually.
- **Did not touch `kovan-common`**, per the task's explicit instruction, even
  though `kovan-discovery`'s `Cargo.toml` declares `kovan-common.workspace =
  true` as a dependency that the crate does not currently use anywhere in
  `src/lib.rs`. Left as-is; flagged below.

## Open questions for human review

1. **Unused `kovan-common` dependency.** `kovan-discovery/Cargo.toml` lists
   `kovan-common.workspace = true`, but no `kovan_common::` item is referenced
   anywhere in `src/lib.rs`. Either this crate is meant to grow a
   `kovan_common`-typed API (e.g. a `discover` variant that returns
   `KovanRepository`-tagged results) and the dependency is forward-looking
   scaffolding, or it is dead weight that should be dropped. Not removed here
   since the task explicitly said not to edit `kovan-common` and removing a
   *consumer's* dependency on it felt like a judgment call outside "quality
   pass, not rearchitecture" — flagging instead of deciding.
2. **Should `kovan-semantics::rough_definition_scan` be rewired onto the new
   `search_repository`?** It currently duplicates that exact loop by hand.
   Rewiring would remove ~10 duplicated lines and make the "ripgrep-first"
   primitive visibly shared, but `kovan-semantics` is a different crate and
   this agent's scope was `kovan-discovery` only — left untouched to avoid
   reaching into another crate's in-progress work without sign-off.
3. **`require_git(false)` — is this the intended default for KOVAN's use
   case?** This pass changed it from the `ignore` crate's default (`true`,
   i.e. `.gitignore` only applies inside a real git repo) to `false`
   (`.gitignore`-style rules apply everywhere, git repo or not). This seems
   right for KOVAN's stated mission (a general offline discovery/search layer
   over arbitrary trees, not exclusively git checkouts — e.g. a
   `kovan-literature/open/` staging tree with its own `.gitignore` well before
   `git init` is ever run there), but it is a judgment call worth a second
   opinion, since it is the one place this pass changed observable behaviour
   for real (non-fixture) callers of `discover`/`discover_kind` running
   against directories that happen not to be inside a `.git` repo. For every
   call site actually in this workspace today (`kovan-cli`, `kovan-semantics`
   scanning real crate directories, which are inside this repo's `.git`), the
   change is a no-op.
4. **No beads filed.** Per this agent's brief, KOVAN epic `op-5v5` is
   JSONL-only / not in local Dolt, and I was told not to create children
   under it or try to fix the sync. If any of the open questions above turn
   into follow-up work, they should become beads once that sync issue is
   resolved; recording them here in the interim per the brief's instruction.

## Verification performed (this pass, 2026-07-15, all commands run for real)

- `cargo build -p kovan-discovery --release` — clean.
- `cargo test -p kovan-discovery --release` — **12/12 unit tests pass**, 0
  doc-tests (none written — no public item currently has a runnable `/// ```
  example; the `examples/` directory covers the "show me how to use it"
  need instead).
- `cargo fmt -p kovan-discovery -- --check` — clean (after running `cargo fmt`
  once to apply the workspace's formatting).
- `cargo clippy -p kovan-discovery --release --all-targets -- -D warnings` —
  clean (covers `src/lib.rs`, the test module, and `examples/`).
- `RUSTDOCFLAGS="-D warnings" cargo doc -p kovan-discovery --no-deps --release`
  — clean, no broken-doc-link or missing-doc warnings.
- `cargo run -p kovan-discovery --release --example discover_and_search` —
  runs end-to-end, output inspected manually (discovers its own 2 source
  files, finds 5 `pub fn` sites via both `search_file` and `search_repository`,
  identical results from both paths as expected).
- `cargo check -p kovan-discovery --target aarch64-linux-android` — clean
  (library only, no `--tests`).
- `cargo check -p kovan-discovery --tests --target aarch64-linux-android` —
  clean (confirms the new `tempfile` dev-dependency is Android-buildable too,
  not just excluded from the library build).
- `cargo build`/`cargo test -p kovan-semantics --release` (the one real
  downstream consumer of `kovan-discovery`'s public API in this workspace) —
  clean, all 3 existing tests still pass unchanged, confirming the `column`
  field addition to `SearchMatch` did not break `kovan-semantics`.
- `cargo build --release -p kovan-cli -p kovan-tui` was **not** completed —
  both depend on `kovan-literature`, which had a concurrent in-progress edit
  from a different session at the time of this pass (missing
  `bibtex`/`markdown`/`metadata`/`pdf_import` modules referenced by
  `kovan-literature/src/lib.rs`) and would not compile regardless of anything
  in this crate. Not this pass's bug to fix; re-run
  `cargo build --release -p kovan-cli -p kovan-tui` once that other work
  lands to confirm end-to-end.
