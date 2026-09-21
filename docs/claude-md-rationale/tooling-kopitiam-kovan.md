# Rationale: KOPITIAM / KOPI-BEANS / KOVAN tooling

> Split out of the root `CLAUDE.md` on 2026-09-21 to keep that file under the
> 150k-character context limit. **This is the full original text, verbatim.**
> The binding rules live in `CLAUDE.md` under "Dogfood KOPITIAM and
>
> **RETIRED RULES PRESERVED HERE.** Two hard rules reproduced below no longer
> apply: the **kopi-beans tracker mandate** (deprecated 2026-09-21, see
> [`../kopi-beans-deprecation.md`](../kopi-beans-deprecation.md)) and the
> **kovan literature ingestion / read-write mandate** (retired 2026-09-21 at
> the maintainer's request). They are kept verbatim as history. **Do not act on
> them** — `CLAUDE.md` is the live rule set.
> KOPI-BEANS"; what is kept here is the command inventory, the version and
> binary-rename history, and the measured accuracy figures.

## Dogfood KOPITIAM and KOPI-BEANS (HARD RULE)

**KOPITIAM (`kopitiam`) and KOPI-BEANS (`kopi-beans`, binary `bn`) are
first-party tools of this project's maintainer and MUST be dogfooded in this
workspace, by default.** Install both from crates.io:

```bash
cargo install kopitiam     # binary: kopitiam
cargo install kopi-beans   # binary: bn
```

Source for both: https://github.com/theodoreOnzGit/kopitiam.

- **`kopitiam`** is a local-first "Semantic Runtime" CLI over real `cargo` /
  rust-analyzer / rustdoc facts, plus a PDF-to-Markdown engine.
- **`kopi-beans`** is a distributed, git-backed work-item tracker — a
  Windows/Termux-capable fork of beads-rs (MIT upstream), relicensed
  **AGPL-3.0-only**, binary `bn`. It is this workspace's mandated issue
  tracker (superseding beads-rs's `bd` — see "Which tracker" below); its CLI
  mirrors `bd`'s 1:1 (`init`, `create`, `show`, `list`, `ready`, `claim`,
  `close`, `dep`, `status`, `prime`, …).

Using them here is deliberate: this workspace is their proving ground, so
**reach for them first** where they cover the task, and **report every rough
edge you hit** (see "Raising issues" below).

**For OUTRAM-PARK-specific context, prefer `kovan` over `kopitiam` (maintainer
direction, 2026-08-15).** `kovan` (`kovan-semantics`, `kovan-literature`, etc.)
is this workspace's *own* deterministic knowledge layer — reach for it first
for repo understanding, symbol/code queries, and literature scoped to this
codebase. **`kovan-cli` now has its own token-frugal reading loop** — `kovan-cli
cost <path>` (a real BPE-approximation estimate, `kopitiam-tokenizer`-backed —
see `crates/kovan/src/commands/cost.rs`), `kovan-cli outline <file>`
(ripgrep-first declarations skeleton, `kovan-semantics`-backed), and
`kovan-cli slice <file> <start> <end>` — landed 2026-08-22 per GitHub issue
#32 ("kovan token savings"). **`kovan-cli` also now has rust-analyzer-backed
`def <symbol> --file <file>` / `sig <symbol> --file <file>` / `refs <symbol>
--file <file>`** (wired directly to `kopitiam-semantic`'s
`RustAnalyzerSession` — needs `rust-analyzer` on PATH, `rustup component add
rust-analyzer`; the *first* query for a workspace root pays a real indexing
wait, up to `KOVAN_RA_TIMEOUT_SECS` (default 180s), but that root then stays
warm in a background `lsp-daemon` (op-fdph) so every later `def`/`sig`/`refs`
call — from any `kovan-cli` invocation — answers in well under a second,
until explicitly stopped with `kovan-cli lsp-daemon-stop --root <root>`; there
is deliberately **no** idle timeout (maintainer direction, 2026-08-22) — once
warm, it stays warm rather than risking a cold restart mid-work. **Never run
`kovan-cli lsp-daemon-serve` directly** — it's the daemon's own foreground
process (spawned detached automatically) and will hang a non-interactive
session exactly like `kovan`/`kovan-tui` would). **Prefer all six of these over
`kopitiam tokens`/`outline`/`slice`/`def`/`sig`/`refs` when the file in
question is inside this workspace** — same reasoning as the rest of this
section, kovan is scoped to this codebase and kopitiam is not. `kopitiam`
remains the right tool for what `kovan` doesn't cover yet:
`callers`/`callees`/`impls` (a deferred kovan-semantics call-hierarchy
composition, tracked as `op-l3uz`) and `rename`/`code-actions`, which `kovan`
has no equivalent for. Run `kovan-cli skill-gen` to (re)generate
`kovan_skill.md`, a Claude-Code-Skill-format Markdown file spelling this out
for an agent session that hasn't read this file.

> **Licence note.** `kopi-beans` is AGPL-3.0-only. That is fine here because it
> is **consumed as a standalone binary**, never linked or vendored — see the
> hard boundary below. Do not add it as a dependency of any workspace crate.

**Where `kopitiam` is the preferred tool:**

- **Token-frugal code reading.** `kopitiam tokens <path>` before deciding to
  read a file; `kopitiam outline <file>` for a declarations-only skeleton;
  `kopitiam slice <file> <range>` to read only the lines you need. Prefer this
  `tokens → outline → refs → slice` loop over reading whole large files.
- **Symbol queries.** `def`, `sig`, `refs`, `callers`, `callees`, `impls` —
  rust-analyzer-backed, so they resolve semantically. These complement the
  read-only LSP tool described under "Workflow rules".
- **Rename and code actions.** `kopitiam rename` (diff preview by default,
  `--apply` to write) and `kopitiam code-actions` **fill the exact gap** the
  Workflow-rules section flags — the harness LSP tool is query-only and exposes
  no rename/code-action/`applyEdit`. Prefer `kopitiam rename` over a hand-rolled
  or `sed`-based rename.
- **Compact diagnostics.** `kopitiam check --compact` and
  `kopitiam test --compact` collapse cargo output to one line per distinct
  problem — far cheaper to read than raw cargo output. **The dedup is opt-in:
  without `--compact` (or `--json`) the raw output streams through unchanged.**
- **PDF → Markdown.** `kopitiam pdf2md` / `translate`. **But for document and
  literature management, prefer `kovan` — see the rule immediately below.**

**ANY literature ingested OR USED goes into kovan (HARD RULE).** If a document
informs the code — a correlation taken from it, a benchmark value cited, a
number in a doc comment, a design decision justified by it — it belongs in
`crates/kovan-literature`, catalogued, with its access tier and provenance. Not
in `~/Downloads`, not loose in `reference-data/`, not read once and forgotten.

- **"Used" is the trigger, not "ingested".** Reading a paper and typing one of
  its numbers into a constant makes it a dependency of this codebase. A
  citation in a doc comment that points at nothing in the archive is a dead
  reference the next reader cannot check.
- **Decide the access tier BEFORE cataloguing**, from the document's own
  copyright page — not from where it was downloaded. Public hosting (INIS,
  gen-4.org, a lab's website) grants no redistribution rights. Unsure means
  **proprietary**; that failure direction is recoverable and the other is a
  licence violation in a public repository.
- **`kovan_import/` is the staging area** (gitignored) — drop the PDF there
  first, decide the tier, then `kovan lit import` into `open/` or
  `proprietary/`. Delete the staged copy once catalogued.
- **Check the extracted metadata; it is frequently wrong.** Observed failures
  include the article-type label, a journal running header, the PII string and
  the Word source filename all taken as titles, editors taken as authors, and
  a scan date taken as the publication year. `kovan lit bibtex <json>` must
  round-trip cleanly — that is the acceptance check.
- **Watch for a text-and-data-mining / AI-training reservation** in the
  copyright line. Where present, catalogue metadata and factual findings only
  and do **not** extract the full text — facts are not copyrightable, but the
  corpus is what that clause reserves. See `op-b7bx`.
- `crates/kovan-literature/CATALOGUE.md` is the human-readable index; keep it
  current when adding a document.

**Document management: `kovan` is preferred over `kopitiam` (HARD RULE).**
For ingesting, cataloguing and citing literature, use this workspace's own
`kovan` CLI (`crates/kovan`, binary `kovan`) rather than kopitiam's
PDF tooling. `kovan lit import <pdf> --json-out <…> --markdown-out <…>` produces
a `KovanDocument` — the canonical on-disk form — alongside the Markdown body,
and `kovan lit bibtex` / `kovan lit outline` work from it. That keeps every
ingested document inside the project's own knowledge layer with its metadata
and provenance intact, instead of leaving a loose Markdown file with no record
of where it came from.

- Build it from the workspace (`cargo build --release -p kovan --bin kovan`) — it is a
  member crate, not something to `cargo install` from crates.io.
- **Respect the open/proprietary split.** Public, openly published literature
  goes under `crates/kovan-literature/open/` and is committable; anything
  restricted goes under `proprietary/`, which is gitignored. Confirm which a
  document is *before* ingesting it — see `DATA_POLICY.md`. The root-level
  `collaboration/` directory is gitignored scratch space and its contents are
  **not** automatically open; ask if the provenance is not stated.
- `kopitiam pdf2md` remains fine for a quick one-off conversion where no
  catalogue entry is wanted, but it is the fallback, not the default.

**READ AND WRITE THE LITERATURE LIBRARY THROUGH `kovan` — HARD RULE.** The
previous rule covers *ingesting*. This one covers everything after: `kovan` is
the interface to `crates/kovan-literature`, in **both** directions, for humans
and agents alike. Reaching around it — `cat`-ing a PDF's converted markdown,
`grep`-ing the archive, hand-editing `CATALOGUE.md`, writing a JSON sidecar by
hand — is not a shortcut, it is how the catalogue and the documents drift apart.

- **Reading.** Query the archive with `kovan lit` (`outline`, `bibtex`, and the
  search/show subcommands `kovan lit --help` lists) rather than opening files
  under `open/`, `proprietary/`, `generated/` or `derived/` directly. It is the
  path that carries the access tier and the provenance with the text; a raw
  `Read` of a markdown body gives you the words with neither, which is exactly
  how a proprietary passage or a TDM-reserved full text gets quoted into a
  commit by someone who did not know.
- **Writing.** New documents arrive via `kovan lit import`. New *derived* data —
  a digitised figure, a hand-read table, an extracted parameter set — goes into
  `crates/kovan-literature/derived/` **beside the document it came from**, with
  its provenance block, not into `docs/` and not into a crate's `src/`. Where a
  scoping doc needs it, that doc holds a **pointer**, not a copy;
  `docs/reactor-scoping/htr10-rz-zone-geometry.md` is the shape to follow.
- **`CATALOGUE.md` is maintained through `kovan`, not by hand.** If a catalogue
  entry is wrong, fix it at the source and regenerate. Hand-editing it is what
  produced the duplicated, mutually-contradicting corroboration sections found
  in the Terry 2005 geometry record on 2026-08-14.
- **If `kovan` cannot do what you need, that is a bug in `kovan` — file it as a
  bead and say so in your hand-off.** KOVAN is this workspace's own crate, so
  the deliverable is an issue against it (like `op-szai` for the metadata
  extractor), never a hand-rolled workaround that bypasses the library.
- This does not relax the open/proprietary split, the TDM/AI-reservation rule,
  or `DATA_POLICY.md` — it is the mechanism by which they are actually enforced.

**Graph digitisation: dogfood `kovan-cli digitise` (HARD RULE).** Several
validation targets this project depends on exist **only as figures** — the
HTR-10 safety demonstration tests and the MSRE reactivity-insertion figures
are both recorded in `docs/reactor-scoping/` as arriving that way. When data
must come off a plot, use this workspace's own digitiser, in
`crates/kovan/src/digitiser/` (**moved here from `crates/kovan-literature/`
on 2026-08-21** — see `crates/kovan/NOTICE`: only the digitiser needs
`kopitiam-pdf`, which is why `kovan` alone is AGPL-3.0-only), reachable from
all three of that crate's binaries — **collapsed from five standalone
digitiser/TUI/CLI binaries to exactly three later the same day**, per
GitHub issue #30's final interface spec:

```bash
cargo build --release -p kovan --bin kovan-cli                # CLI: `kovan-cli digitise`
cargo build --release -p kovan --bin kovan-tui                # TUI: Digitiser tab
cargo build --release -p kovan --bin kovan --features gui     # GUI
```

- **`kovan-cli digitise` is the agent path** — fully automatic, scriptable,
  deterministic. **Use it rather than reading points off a figure by eye.** A
  hand-read point has no calibration record, no uncertainty and no audit
  trail, and is exactly the kind of silent processing step `DATA_POLICY.md`
  forbids. Same reasoning as the 138-number Tobias Table 16 transcription:
  prefer the machine-readable path and validate it, over eyeballing.
- **`kovan-tui`'s Digitiser tab and the `kovan` GUI are the human path** —
  automatic pass first, then the maintainer verifies. The CLI can only ever
  emit `Unreviewed`; **only a human marks a dataset reviewed**, and editing a
  point afterwards resets it to unreviewed. Do not attempt to mark anything
  reviewed from an agent session. (This engine has had three different
  binary layouts in one day, 2026-08-21: first as `kovan-literature`'s
  `kovan-digitise`/`kovan-digitise-tui`/`kovan-digitise-gui`; then moved into
  `kovan` unrenamed except `kovan-digitise-gui` → `kovan-gui`; then
  `kovan-gui` → plain `kovan`, the old `kovan` CLI → `kovan-cli` with a
  `digitise` subcommand replacing standalone `kovan-digitise`, and
  `kovan-digitise-tui`'s review screen absorbed into `kovan-tui` as a
  Digitiser tab. There is no longer any `kovan-digitise`, `kovan-gui`, or
  `kovan-digitise-tui`/`-gui` binary — see `crates/kovan/DECISIONS.md`.)
- **Provenance is structurally mandatory and must stay that way.** A
  `DigitisedDataset` cannot be constructed without a `FigureSource` and a
  `PlotCalibration`. Never add a path that exports points without them.
- **Known limit, by design:** there is no tick-label OCR (no ML, per KOVAN's
  offline-deterministic rule), so the axis reference values must be supplied —
  read them off the figure and pass `--x-range`/`--y-range`, or explicit
  `--x-ref`/`--y-ref` pixel=value pairs. Always state `--x-scale`/`--y-scale`;
  log axes interpolate in log space and getting this wrong is silent.
- **Report rough edges as beads, not workarounds.** Unlike kopitiam, KOVAN is
  *our own* crate, so a defect here is a bead in this workspace (e.g.
  `op-szai` for the metadata extractor), not an upstream issue.
- **Accuracy is verified against synthetic ground truth only** (lin-lin and
  log-lin 0.138% of span, log-log 0.002765 decades, measured 2026-08-11).
  There is **no** verification against real published figures until the
  maintainer supplies the hand-digitised Tobias oracle (`op-amfh`). Do not
  describe digitised data as validated before then.

**Known friction and resolution history — read `docs/kopitiam-issues/`, don't
trust a version number written here.** Both tools have moved fast enough
(kopi-beans alone: 0.1.3 -> 0.1.4 -> 0.1.6 -> 0.1.7 in a single day, once) that
any dated claim in this file goes stale almost immediately. The current open
queue, the upstream issue-number table, and full closing evidence for every
resolved issue live in **`docs/kopitiam-issues/README.md`** (open + upstream
state table) and **`docs/kopitiam-issues/resolved/`** (closed, each with the
re-run reproduction and new output). Check there, or run `bn --version` /
`cargo install --list`, before citing either tool's behaviour as current.

Two facts that stay true regardless of version churn:

- **`kopitiam check`/`kopitiam test` have no `--release` flag** and run the
  `dev` profile. Use them for fast iteration, but still run the mandated
  `cargo check --release --workspace --lib --tests` / `cargo test --release` before
  calling work done — do not let kopitiam's default profile substitute for it.
  Note that running them also materialises the `target/debug` tree that the
  release-check note under "Build & test" exists to avoid.
- **`bn setup claude --project` is verified safe** — it writes only the
  gitignored `.claude/settings.local.json` and leaves the hand-maintained
  `.claude/settings.json` untouched. The **unflagged/global form was never
  tested** — do not run it without testing first.

**CONSUME THE BINARIES ONLY — NEVER MODIFY KOPITIAM OR KOPI-BEANS FROM THIS
WORKSPACE.** This is the hard boundary, it covers **both** tools, and it does
not bend:

- **Use released binaries.** Install with `cargo install kopitiam` /
  `cargo install kopi-beans` (crates.io). Upgrade by installing a newer
  published version. That is the *only* supported way this workspace consumes
  them.
- **Never edit their source from here.** No local edits, no local patched
  builds, no `cargo install --path` off a working copy, no commits, no
  branches, and no pull requests to the kopitiam repo out of this workspace.
  If a bug or missing feature blocks you, **the deliverable is an issue, not a
  patch.**
- **Never make them part of this workspace.** Do not add either to
  `[workspace.dependencies]`, do not add them as workspace members, and do not
  vendor their source here. This matters doubly for `kopi-beans`, which is
  AGPL-3.0-only.
- **If you consult its source at all, treat it as strictly read-only**, and
  keep the clone in a **separate directory outside this repository** — e.g.
  `/workspace/kopitiam`, never anywhere under the OUTRAM PARK working tree.
  A nested clone would pollute `git status`, break `cargo` workspace
  discovery, and risk committing another project's history into this one.
  Reading it is for writing an *accurate issue*, nothing more.
- **Its per-project state stays local.** Running kopitiam here writes
  `.kopitiam/state.redb` (session memory) into the repo root; that path is
  gitignored and must never be committed or un-ignored.
- Keep the projects' trackers separate: OUTRAM PARK work goes in this
  workspace's tracker, kopitiam/kopi-beans bugs go upstream.

**Raising issues — two channels, in this order.** Every rough edge, bug, and
feature request in either tool gets written up. Never silently work around a
defect.

1. **Preferred: a GitHub issue, via `gh` if it is available.** The kopitiam
   repo is *not* in this workspace's default GitHub scope — add it to the
   session first (`add_repo` for `theodoreOnzGit/kopitiam`), then file with
   `gh issue create --repo theodoreOnzGit/kopitiam`. Both tools live in that
   one repo; say in the title which tool it concerns.
2. **Fallback, when `gh` is unavailable or unauthenticated: file locally under
   `docs/kopitiam-issues/`, one markdown file per issue.** Name it
   `<tool>-<short-kebab-slug>.md` (e.g. `kopitiam-check-has-no-release-flag.md`,
   `kopi-beans-bn-init-fails-on-termux.md`). These are a queue for later
   upstreaming, not a private bug tracker — do not let them accumulate silently;
   mention any new ones in your hand-off.

Whichever channel: report **what you actually ran, the observed output, and
the expected behaviour**, plus the tool version from `cargo install --list`.
Do not invent version numbers or fabricate reproductions. Filing the issue is
the end of your involvement in the fix — do not follow it up with code.

**HARD RULE — resolved issues move to `docs/kopitiam-issues/resolved/`.** Once
an issue is actually fixed upstream, **move its markdown file** from
`docs/kopitiam-issues/` into `docs/kopitiam-issues/resolved/`. Do not delete it
and do not leave it sitting in the top-level queue.

- **"Resolved" means verified, not announced.** Upgrade to the published
  version that claims the fix (`cargo install kopitiam` /
  `cargo install kopi-beans`), **re-run the exact reproduction recorded in the
  file**, and confirm the behaviour changed. Only then move it.
- **Record the closing evidence in the file as you move it:** the version that
  fixes it, the date, the command re-run, and its new output. A file in
  `resolved/` without that evidence is not a resolution, it is a claim.
- If the fix landed upstream as a GitHub issue rather than a local file, close
  the loop the same way — verify against a published binary before treating it
  as done.
- The top level of `docs/kopitiam-issues/` therefore always reads as **the live
  queue**, and `resolved/` as the history. Anything still at the top level is
  outstanding.
- This also applies when the workspace's own "known friction" notes (e.g. the
  kopitiam `--release` gap recorded above) are fixed: update or remove the note
  in this file in the same change, so `CLAUDE.md` never advertises friction
  that no longer exists.

**Which tracker for OUTRAM PARK's own work.** `bn` (kopi-beans) is a *fork of*
beads-rs, so it overlaps `bd` rather than complementing it — running both
against one repo defeats the purpose. **Per explicit maintainer instruction on
2026-08-07, kopi-beans (`bn`) replaces beads-rs (`bd`) as this workspace's
tracker.** This is *not* a dogfooding-only install anymore — see the mandatory
"Issue tracking & roadmap" section below, which is written for `bn`. **The
migration is complete**: kopi-beans 0.1.2 reads and has migrated the store
(format_version 2), and `bd` is uninstalled and its daemon stopped. There is
no reason to reach for `bd` here, and no supported way to.

**This rule relaxes nothing.** The release-mode rule, the working-hours
guardrail *when it has been enabled for the session*, never-auto-commit/push,
the Android/Termux portability rule, and the data-policy rules all still bind
when using either tool.
