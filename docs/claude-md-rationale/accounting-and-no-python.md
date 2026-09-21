# Rationale: token accounting, the historian report, and the no-Python rule

> Split out of the root `CLAUDE.md` on 2026-09-21. **Full original text,
> verbatim.** Includes the opt-in token-accounting policy in full, the
> historian generator's details, the retired-Python table and the Windows
> `python3` alias incident that produced the no-Python hard rule.

## Token accounting on every commit (opt-in, HARD RULE as stated below)

**Changed 2026-08-17 at the maintainer's request: token accounting is now
opt-in, not mandatory.** The previous rule — "every commit must carry an
API-token-usage trailer" — is retired. It is replaced by the policy below,
which *is* the hard rule from now on: do not enforce or chase the old
mandate, and do not treat a missing trailer as something to fix.

- **`crates/kovan-metrics`**, driven through the **`kovan-cli`** binary,
  remains the single source for token accounting **when a maintainer has chosen
  to set it up** on a given clone — it does both the write side (git hooks
  stamping commit trailers) and the query side (`kovan-cli tokens query --from
  DDMMYY --to DDMMYY [--branch develop] [--per-commit] [--json]`, summing
  whatever trailers exist).

  **`kovan-cli`, not `kovan`** — corrected 2026-09-19 against
  `crates/kovan/src/bin/kovan-cli.rs`, which is where `Command::Tokens` and
  `Command::Historian` are actually declared. `kovan` is the egui GUI binary
  and will hang a non-interactive session trying to open a display, exactly as
  the "API-doc toolchain" section already warns for `kovan-cli api-docs`. Nothing about its internals changed; what changed is
  whether using it is required.
- **`.githooks/prepare-commit-msg`** and **`.githooks/post-commit`**, where
  installed, still stamp the `API-Usage-Since-Last-Commit:` /
  `API-Usage-Session-Cumulative:` trailer and regenerate the local
  `docs/token-usage.md` summary exactly as before (idempotent, amend/rebase
  safe). Let them run undisturbed on a clone that has opted in.

**If a clone has not opted in (no `kovan` binary, hooks not installed):**

- **Do not prompt the user to install anything.** No suggesting
  `./scripts/install-token-hooks.sh`, no "want me to set up token accounting"
  — opting in is a maintainer decision made once, not a gap to flag on every
  commit.
- **Do not hand-write a trailer or invent numbers.** If the hooks aren't
  present, the commit has no `API-Usage-*` trailer, full stop.
- **Note the absence in the commit message body instead**, one short line
  (e.g. `No token accounting on this clone.`) rather than saying nothing.
  That line is the entire obligation — it carries no numbers and is not a
  substitute trailer.

**If a clone has opted in, the mechanics are unchanged:**

- **Source of truth: the per-commit trailers, not the markdown.** The durable
  record is the `API-Usage-*` trailer in each commit message (queryable across
  any window with `kovan-cli tokens query --from DDMMYY --to DDMMYY`).
  `docs/token-usage.md` is a regenerable, gitignored local summary — never
  hand-edit it, never `git add` it, never re-track it.
- **Do not strip or fake a trailer the hooks wrote.** The numbers come
  straight from the transcripts; nothing is estimated or invented. A commit
  made outside a Claude session legitimately shows `total=0 source=none` —
  that is correct, not a bug.
- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read
  (prompt-cache re-reads of the growing context) usually dominates and is
  shown separately — do not collapse it into a single figure that hides the
  split.
- **Opting in:** `./scripts/install-token-hooks.sh` (sets the local
  `core.hooksPath` to `.githooks` and initialises the baseline) is still there
  for whoever wants it, unchanged — it is simply no longer something to push
  on someone who hasn't asked for it.
- **New repositories worked on here do not inherit this as a requirement.**
  Copying `.githooks/` (incl. `kovan-bin.sh`) + the installer over is optional,
  at that repository's maintainer's discretion — same opt-in stance as here.
- This does not relax the never-auto-commit/push rule above: the hooks, where
  present, only act *when a commit the user asked for is being made*; they
  never initiate one.

**This section documents the reasoning history** — `docs/historian/*` and the
"No Python for documentation or accounting" section below still describe real
past incidents (e.g. the Windows `python3` alias silently zeroing out
trailers) accurately; they are history, not evidence that accounting is
mandatory today.

## Historian report before every merge to `main` (mandatory)

**Before merging `develop` into `main`, generate a "historian" report** — a
generated markdown file accounting for the **API tokens spent** and the
**lines / KLOC written** across the window of `develop` history being released,
listing the commits over a `DDMMYY..DDMMYY` date range. The generator is
`crates/kovan-metrics` (via the **`kovan-cli`** binary — *not* `kovan`, which
is the GUI; see the token-accounting section above); the reports live under
**`docs/historian/`** at the workspace root.

- **Generate it:**
  `kovan-cli historian --from DDMMYY --to DDMMYY`
  (`DDMMYY` = day-month-year, 2-digit year). With no `--from`, it defaults to
  "everything on `develop` not yet on `main`, up to today". Output is written to
  `docs/historian/historian_<from>_to_<to>.md`.
  - **This replaced `docs/historian/historian.py` on 2026-08-13** (epic
    `op-yz7b`), and the Python was deleted the same day. No parity gate was run
    against it; see the token-accounting section above.
  - `kovan-cli historian` dates from **UTC**, where the Python used local time.
    This only affects the default `--to` bound and the filename tag; pass
    `--to` explicitly if a midnight boundary matters.
- **What it contains:** total lines added/removed/net (all files + Rust-only),
  total tokens broken out (`in`/`out`/`cache_read`/`cache_write`/`total`), a
  per-crate lines-added breakdown, and a per-commit ledger.
- **Sources, not estimates.** Tokens come from the `API-Usage-Since-Last-Commit`
  commit trailers (§ token accounting above); lines come from
  `git log --numstat --no-merges` over the range. Commits predating the token
  hooks legitimately show *no token data* — that is correct, not a gap.
- **Commit the generated report alongside the `develop`→`main` merge**, so each
  release carries its own accounting. Do not hand-edit the generated markdown.


## No Python for documentation or accounting — build it into `kovan` (HARD RULE)

**Documentation generation and repository accounting are `kovan`'s job. Do not
write, restore, or reach for a Python script to do either. If `kovan` cannot do
it yet, extend `kovan`.**

This is settled direction, not a preference, and it has been applied three times:

| Retired | Replaced by | When |
|---|---|---|
| `docs/historian/historian.py` | `kovan-cli historian` (`kovan-metrics`) | 2026-08-13, epic `op-yz7b` |
| `docs/historian/token_usage.py` | `kovan-cli tokens` (`kovan-metrics`) | 2026-08-13, epic `op-yz7b` |
| `scripts/gen_api_docs.py` | `kovan-cli api-docs` | 2026-08-14, `op-w44a.7` |
| `scripts/gen_aster_behaviour_registry.py` | retired; procedure recorded in `catalogue.rs` | 2026-08-14 |
| `scripts/kloc_accounting.py` | `kovan kloc` (`kovan-metrics`) | 2026-08-14 |

**`scripts/` now holds no tracked Python** — only `.gitignore` and four shell
scripts. (`find` reports hits under `scripts/vendor/`; that directory is
gitignored and holds repository clones the retired `kloc_accounting.py` made.
It is now orphaned: `kovan kloc` vendors into its own output directory instead,
so `scripts/vendor/` can be deleted.)

**Seven first-party Python files remain, and are NOT covered by this rule as
written:** `crates/outram-park-fork-coolprop/dev/*.py` — `gen_fluid.py`,
`gen_incompressible.py`, `gen_mixture.py`, their three `regen_*_all.py`
drivers, and `gen_latex_doc.py`. Six are **code generation** (they read the
gitignored upstream CoolProp JSON clone and emit Rust), which is neither
documentation nor accounting; `gen_latex_doc.py` scaffolds a LaTeX doc series
and arguably is. Whether to bring them in is a maintainer decision that has not
been made — tracked as a bead. Do not delete them under this rule without
asking.

Everything else matching `*.py` is vendored upstream source under
`upstream_source/` (NJOY2016, Blender, TRISO-ATOPS) or gitignored
`collaboration/` scratch, both explicitly out of scope.

**Why, concretely.** A script merely has to exist; an interpreter has to be
installed, on `PATH`, and not shadowed. On Windows `python3` routinely resolves
to a Microsoft Store alias stub that prints an advert and exits — which silently
turned the token-accounting git hooks into no-ops and let commits ship with no
`API-Usage` trailer at all. That is the failure mode this rule exists to
prevent: not an error, a **silent** no-op in the thing that keeps the records
honest. The reasoning is recorded in `.githooks/kovan-bin.sh`.

**Scope.** Documentation generation, repository accounting, and the artifacts
either produces. It does **not** reach into `collaboration/` (gitignored scratch
owned by collaborators), `reference-data/` (vendored upstream trees), or a
third-party tool that happens to be written in Python.

**When porting, gate parity — do not waive it.** `op-yz7b` shipped without a
byte-for-byte comparison against the Python it replaced, and that gap is
recorded above as a known weakness. `op-w44a.7` did gate it: the Python-generated
`api.md` was already committed, so regenerating through the Rust path and
running `git diff --quiet` was a real check, and it passed. **Do that.** If the
old output is not committed anywhere, generate it with the Python *before*
deleting the script, commit it, then port.

**A Python script that is a published reproducibility artifact is a different
question — ask, do not delete.** Where a script exists so that a *journal
reader* can re-derive a table or figure, replacing it with a Rust binary raises
the reproduction bar from "run this script" to "build a 40-crate Rust
workspace", and may break a byte-identical copy held in a manuscript
repository. Raise it with the maintainer rather than applying this rule
mechanically.

`kloc_accounting.py` was exactly that case: it reproduces the Annals of Nuclear
Energy submission's tables and figure. It was put to the maintainer on
2026-08-14 with the consequence stated, and they chose the full port. **The
manuscript's own copy and any text telling a reader to run the script are now
stale and are the maintainer's to update** — that repository is not visible from
here.

