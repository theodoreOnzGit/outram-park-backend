<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Token accounting on every commit (opt-in)

**Changed 2026-08-17 at the maintainer's request: token accounting is opt-in,
not mandatory.** The previous rule — "every commit must carry an
API-token-usage trailer" — is retired. Do not enforce or chase it, and do not
treat a missing trailer as something to fix.

**If a clone has not opted in** (no `kovan` binary, hooks not installed):

- **Do not prompt the user to install anything.** Opting in is a maintainer
  decision made once, not a gap to flag on every commit.
- **Do not hand-write a trailer or invent numbers.** No hooks means no
  `API-Usage-*` trailer, full stop.
- **Note the absence in the commit message body instead**, one short line
  (e.g. `No token accounting on this clone.`). That line is the entire
  obligation — it carries no numbers and is not a substitute trailer.

**If a clone has opted in**, `.githooks/prepare-commit-msg` and
`.githooks/post-commit` stamp the `API-Usage-Since-Last-Commit:` /
`API-Usage-Session-Cumulative:` trailers and regenerate the gitignored local
`docs/token-usage.md`. Let them run undisturbed, and:

- **Source of truth is the per-commit trailers, not the markdown.** Query any
  window with `kovan-cli tokens query --from DDMMYY --to DDMMYY`.
  `docs/token-usage.md` is a regenerable local summary — never hand-edit it,
  never `git add` it, never re-track it.
- **Do not strip or fake a trailer the hooks wrote.** The numbers come from
  the transcripts; nothing is estimated. A commit made outside a Claude
  session legitimately shows `total=0 source=none` — correct, not a bug.
- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read
  usually dominates and is shown separately — do not collapse the split.
- Opting in is `./scripts/install-token-hooks.sh`. **New repositories worked
  on here do not inherit this as a requirement.**

**Use `kovan-cli`, not `kovan`** — `kovan` is the egui GUI binary and will
hang a non-interactive session trying to open a display. This does not relax
the never-auto-commit/push rule: the hooks only act when a commit the user
asked for is being made; they never initiate one.

## Historian report before every merge to `main` (mandatory)

**Before merging `develop` into `main`, generate a "historian" report** — a
generated markdown file accounting for the **API tokens spent** and the
**lines / KLOC written** across the window of `develop` history being
released. The generator is `crates/kovan-metrics` (via the **`kovan-cli`**
binary — *not* `kovan`, the GUI); reports live under **`docs/historian/`**.

```bash
kovan-cli historian --from DDMMYY --to DDMMYY     # DDMMYY = day-month-year, 2-digit year
```

With no `--from`, it defaults to "everything on `develop` not yet on `main`,
up to today". Output goes to `docs/historian/historian_<from>_to_<to>.md`.
It dates from **UTC**, which affects the default `--to` bound and the filename
tag; pass `--to` explicitly if a midnight boundary matters.

**What it contains:** total lines added/removed/net (all files + Rust-only),
tokens broken out (`in`/`out`/`cache_read`/`cache_write`/`total`), a per-crate
lines-added breakdown, and a per-commit ledger.

**Sources, not estimates.** Tokens come from the `API-Usage-*` commit
trailers; lines come from `git log --numstat --no-merges`. Commits predating
the token hooks legitimately show *no token data* — that is correct, not a gap.

**Commit the generated report alongside the `develop`→`main` merge**, so each
release carries its own accounting. Do not hand-edit the generated markdown.

## A full `kovan-cli test` run before every merge to `main` (mandatory)

**Maintainer direction, 2026-10-29: "Full runs only when merging to main".**
Day to day, stamps pending a workspace test are cleared by the targeted run
of their reaching tests: `kovan-cli test --pending-stamps` (GitHub #1052),
nightly on `develop` (`.github/workflows/kovan-nightly-tests.yml`) or the
"Run pending stamps' tests" button in kovan's Code Review. **The full
suite is run when `develop` is merged into `main`:** run `kovan-cli test`
(or Code Review's "Run workspace test") on the `develop` commit being
merged, commit the `[test_run]` it writes into the `kovan.toml` files to
`develop`, and merge that commit. **No fail-fast** (maintainer,
2026-10-29): every merge-to-`main` test run goes on past a failure, so one
failing binary cannot hide the rest. `kovan-cli test` always passes
`--no-fail-fast`, as do `full-tests.yml` and
`scripts/njoy-ace-leapr-tests.sh`; the k-eff acceptance script runs every
case whatever an earlier one gave. A full run also clears what a targeted
run cannot: stamps on functions no test reaches.

> Full opt-in policy, the historian's replacement history and the reasoning:
> [`docs/claude-md-rationale/accounting-and-no-python.md`](../claude-md-rationale/accounting-and-no-python.md).

