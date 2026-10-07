# kovan-metrics

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
**S**ciences. This crate is part of KOVAN: its repository
accounting (token trailers and the historian report).

> **On crates.io (2026-10-07):** the KOVAN app is published as
> `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`, its backronym
> spelled out (`kovan` on crates.io is an unrelated crate). Install it with
> `cargo install knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`;
> the binaries are still `kovan`, `kovan-cli` and `kovan-tui`, and the library
> is still `kovan`.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

**KOVAN repository accounting** — per-commit API-token trailers and the
pre-merge-to-`main` historian report, for the OUTRAM PARK workspace.

This replaced `docs/historian/token_usage.py` and `docs/historian/historian.py`
on 2026-08-13; both were deleted the same day, so this crate is now the only
implementation. It exists so the workspace toolchain needs **no
Python interpreter**: on Windows, `python3` routinely resolves to a Microsoft
Store alias stub that prints an advert and exits, which silently turned the git
hooks into no-ops and let commits ship carrying no `API-Usage` trailer at all.

~~Driven through the `kovan` CLI (`crates/kovan`):~~ **CORRECTED 2026-09-25** —
driven through the **`kovan-cli`** binary (`crates/kovan`). Since the 3-binary
split of 2026-08-21, `kovan` is the GUI and hangs a headless session; the
`tokens` and `historian` subcommands live on `kovan-cli` (checked with
`kovan-cli --help` and `kovan-cli tokens --help`), and `.githooks/kovan-bin.sh`
resolves `kovan-cli`, not `kovan`. The block below previously read `kovan …`:

```bash
kovan-cli tokens trailer <msgfile>   # prepare-commit-msg: stamp the trailers
kovan-cli tokens record              # post-commit: advance baseline, refresh ledger
kovan-cli tokens report              # regenerate docs/token-usage.md
kovan-cli tokens init                # stamp the baseline (installer)
kovan-cli tokens show                # live cumulative + delta since last commit
kovan-cli tokens query --from 010826 --to 130826 --branch develop [--per-commit] [--json]

kovan-cli historian --from 010826 --to 130826           # explicit window
kovan-cli historian                                     # develop not yet on main
```

## What it measures, and from where

| Quantity | Source |
|---|---|
| Live token usage | Claude Code session transcripts, `~/.claude/projects/<slug>/*.jsonl` — the same data `ccusage` reads |
| Recorded token usage | The `API-Usage-Since-Last-Commit` commit trailers |
| Lines / KLOC | `git log --numstat --no-merges` |

`total` = `in` + `out` + `cache_read` + `cache_write`. Cache-read (prompt-cache
re-reads of the growing context) usually dominates and is always reported
separately — never collapse it into a single figure that hides the split.

**Attribution is temporal, not per-diff.** A commit is charged the tokens spent
between the previous commit and itself, regardless of which files those tokens
touched. The baseline that makes this meaningful lives at
`<git-dir>/claude-token-baseline.json` — per-clone, never committed.

## Two rules this crate is built around

**Never block a commit.** The write-side entry points run inside
`prepare-commit-msg` and `post-commit`. They swallow their own errors and
degrade to a zero/`source=none` trailer rather than failing. A missing ~~`kovan`~~
`kovan-cli` (**CORRECTED 2026-09-25**, see above) binary is a no-op, not an error.

**Never invent a number.** A commit made outside a Claude session honestly reads
`total=0 source=none`; a commit predating the hooks honestly carries no trailer
and renders as `—`. Neither is a gap to be filled with an estimate.

## Dependencies

`kovan-discovery` (repo-root discovery — the workspace's canonical gitoxide
layer; this crate does not open a second one), `serde_json`, `thiserror`. No
date crate: `DDMMYY` parsing and civil-date arithmetic are in
[`src/date.rs`](src/date.rs) via Hinnant's algorithms, since three date
operations do not justify a workspace-wide dependency. `Date::today` is **UTC**
— `std` exposes no local timezone — which can differ from the Python's local
`date.today()` for the hours either side of midnight, and only for the default
`--to` bound and the generated filename tag.

Arbitrary `git log --format=…` / `--numstat` queries shell out to the `git`
binary, which `kovan-discovery`'s typed `CommitInfo` cannot express (it carries
no message body and no diff statistics). That is safe here because this code
runs inside git hooks, where `git` is present by construction.

## Zotero library and import accounting (`zotero`, GitHub #751)

Kovan's own accounting over a Zotero library, built on `kovan-common`'s port
of the Zotero data model and its `ZoteroItem` -> `KovanDocument` conversion
(#748). Not a port: Zotero has no import report.

| API | What |
|---|---|
| `LibraryCounts::of(&lib)` | items per item type, per collection (direct and including subcollections), per tag (manual/automatic), per attachment link mode, per annotation type, per relation predicate; saved searches; top-level, child, trashed and unfiled totals |
| `import_library(&lib) -> (Vec<KovanDocument>, ImportReport)` | converts every top-level, non-trashed regular item with a key; skips children, standalone notes/attachments/annotations, trashed, keyless and duplicate-key items, each with its reason |
| `.to_markdown()` on both | deterministic Markdown reports (sorted; no time, locale or file system) |

**How "lossy" is measured.** Empirically, from the conversion itself: each
item goes Zotero -> kovan -> Zotero with the verbatim `zotero_item` copy
removed, and every property the round trip does not reproduce is reported as
*dropped*, *altered* (e.g. `date` kept only as a year) or *moved to Extra*.
Fields are compared through their base field. "Lossy" means **not carried by
kovan's own document fields**: the item itself is kept in `zotero_item`, so
the full round trip is lossless. Skipped child items are preserved nowhere.

**V&V** (`tests/zotero_accounting.rs`, methodology and results in its doc
comment). On Zotero's own `itemJSON` fixture (one item of each of the 37
regular item types, every field filled) the report gives 37 imported, 0
skipped, 839 loss entries over 105 properties; the journal article's 28
losses were predicted from kovan-common's documented field map before the
first run and matched exactly. Seven tests, all passing on the first run
(2026-10-07).

**Maturity: AI draft (1).**

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

> **No parity gate was ever run against the Python, and the Python is now
> gone.** Byte-for-byte comparison with `token_usage.py` / `historian.py` was
> waived by the maintainer on 2026-08-13 ("don't bother with the parity"), and
> the scripts were deleted the same day ("we will just dogfood rust ones in
> kovan from now on"). A direct comparison is therefore **no longer possible
> without recovering them from git history** — they were last present at commit
> `c12624a41e`. This crate's outputs are covered by its own ~~37 unit tests~~ unit tests (**CORRECTED 2026-09-25** — `grep` now counts 88 `#[test]` functions under `src/`, many added with the later `kloc` module, plus 4 in `tests/kloc_parity.rs`; counted, not run) and by
> hand-verification against real repository history, **not** by equivalence to
> the originals. Note also that equivalence would have been the wrong bar: the
> Python's transcript-directory slug never matched on Windows, so it read zero
> tokens where this crate reads the real figures.

## Licence

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)).
