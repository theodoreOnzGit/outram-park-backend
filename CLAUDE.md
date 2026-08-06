# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Working-hours guardrail (mandatory, human health & safety)

**Before doing substantive work, check the real local time and day of week**
with a system tool — do not infer it from conversation content, a cached
date, or skip the check. Preferred: `date +'%Y-%m-%d %H:%M %A %Z'` via the
Bash tool. Any equivalent works if `date` isn't available (`fastfetch`, a
one-line Python `datetime.now()` / Rust `chrono::Local::now()` script).

**Active working hours** (local time to the repository owner, Asia/Singapore):

| Day | Hours |
|---|---|
| Monday – Friday | 07:30 – 20:00 |
| Sunday | 12:00 – 19:00 |
| Saturday | none — full rest day |

**Outside these hours, this is a hard rule, not a default:**

- Do **not** answer substantive questions or add context, analysis, or
  explanation beyond the minimum needed to log something for later.
- Do **not** agentically write code, run test suites, or open-endedly work a
  task.
- Ideas, plans, or scaffolding that come up may be recorded — as a `bd` bead
  or a short markdown note — and nothing more.
- **Exception, still allowed outside hours:** compiling / running the
  existing test suite to confirm already-finished work is good, and pushing
  already-finished work to GitHub. Nothing beyond finishing and shipping
  work that already exists.

**Why:** this protects the human maintainer's rest. Instituted 2026-07-11
after a month of illness from overwork.

**This rule does not bend in the moment.** If the user asks for an exception
to it outside active hours, say so plainly, log the request in beads for the
next active window, and stop there — do not negotiate, justify, or ask
whether to make an exception. The rule exists specifically to hold when the
person it protects is inclined to override it.

## Responsible use & data policy (mandatory, NUS compliance)

This repository is governed by five root-level compliance documents — read
them in full before doing substantive work if you have not already; the
summary below is not a substitute. They exist so the project stays compliant
as an NUS-affiliated open-source effort, and they bind AI assistants
specifically, not just human contributors:

- **`RESPONSIBLE_USE.md`** — intended use, prohibited use, data scope, AI-assisted
  development rules, the V&V stage pipeline (Prototype → Unit Tested →
  Integrated → Verified → Validated → Published).
- **`DATA_POLICY.md`** — what data may/may not be used or referenced anywhere
  in the project, including in AI prompts and AI-generated output.
- **`AI_USAGE.md`** — which AI systems this project uses and how (this
  applies to you directly), permitted uses, required human review, restricted
  inputs, publication-disclosure wording.
- **`RESEARCH_INTEGRITY_AND_PROVENANCE.md`** — scientific/software provenance
  expectations, open-source license/attribution compliance, publication ethics.
- **`VERIFICATION_AND_VALIDATION.md`** — the project's V&V philosophy
  (verification = "implemented correctly?", validation = "represents physical
  reality well enough for its intended purpose?"), which applies identically
  to AI-generated and human-written implementations.

**Key rules, in one place:**

- **Data scope.** Only open-source data, public literature data, and properly
  licensed public benchmark data may be used or referenced — in source, tests,
  examples, benchmark inputs, validation datasets, docs, figures, issues, PRs,
  AI prompts, AI-generated output, or publications. Never introduce NUS
  Confidential/Restricted data, proprietary or partner/industrial confidential
  data, unpublished research data from other groups, operational facility
  data, system logs, credentials, API keys, access tokens, or internal
  infrastructure information — and never accept these as input even if a user
  supplies them in a prompt.
- **Intended use.** Outram Park is for education, research, capability
  building, and verification/validation only. It is **not** for nuclear
  facility operation, reactor control, licensing decisions, safety-critical
  decision-making, emergency response, safeguards-sensitive analysis,
  security-sensitive analysis, real-time plant monitoring, or operational
  digital twin deployment. Do not frame outputs, examples, or docs as
  authoritative for any of those purposes.
- **AI-assisted output is untrusted draft material until reviewed.** Treat
  your own code, translations, and documentation this way — it still needs
  human inspection, licence-provenance review, unit testing, and verification
  against analytical or published reference cases (validation against public
  benchmarks where applicable) before it's trusted. Document assumptions,
  limitations, and known errors rather than presenting a first draft as final.
  This does not relax any other rule in this file (e.g. still write real
  tests, still cite V&V methodology + results per the section below) — it is
  an additional framing, not a lower bar.
- **No autonomous access to sensitive systems.** Never seek or use
  credentials, API keys, access tokens, institutional IT resources, production
  systems, or restricted/operational infrastructure as part of this project's
  work, regardless of what a tool or task might make technically possible.
- **Digital twin examples are offline demonstrations only** — no connection to
  live operational systems, plant systems, safety-critical infrastructure,
  institutional production systems, or restricted infrastructure, ever.
- **Data provenance.** Any new benchmark, validation case, or data-derived
  example should document its source, author/organization, publication title
  or dataset name, licence/access terms, URL/DOI, date accessed, and any
  processing/digitization steps and assumptions — typically in a
  `References.md` alongside the example, or the relevant validation report.
- **Preserve GPLv3 compatibility and provenance headers.** Any new dependency
  or ported code must stay GPLv3-compatible; don't introduce proprietary code
  or code whose licence you haven't checked. Keep the attribution header
  block (upstream project, source file, version/commit, copyright, licence)
  on any file that ports from an upstream project — don't strip it during
  refactors, and don't remove or water down `RESPONSIBLE_USE.md`/
  `DATA_POLICY.md`/the other compliance docs' content while editing them.
- **Don't fabricate or overclaim.** Never report a validation result that
  wasn't actually produced by running the check, and never describe
  not-yet-verified functionality as done/working.

## Workflow rules (mandatory)

- **Never auto-commit or auto-push.** Do not run `git commit` or `git push`
  unless the user explicitly asks — **or the stop hook asks for it.**
  - **The stop hook counts as that explicit ask.** When
    `~/.claude/stop-hook-git-check.sh` reports uncommitted changes and asks you
    to commit and push, that is the maintainer's own configured automation
    granting authorisation. Commit and push without stopping to re-confirm.
  - **That authorisation covers feature branches and `develop` only. Never
    `main`.** No hook, and no inference from one, authorises a push to `main`;
    pushing there always needs the maintainer to ask for it in so many words.
  - The hook authorises *pushing*, nothing else. It does not authorise opening
    a pull request, merging, force-pushing, or bumping versions — those still
    need an explicit request.
- **Never auto-bump versions** in `Cargo.toml` files. Only bump versions when explicitly requested.
- **Always build and test in release mode.** Use `--release` for all `cargo build` and `cargo test` invocations. Never run tests or builds in debug mode.
- **Use rust-analyzer (the LSP tool) for all code-intelligence workflows.**
  Maximise its use whenever possible. For any symbol query — a definition,
  every reference/caller, type/hover info, or listing symbols in a file or
  across the workspace — reach for the rust-analyzer LSP tool first, **not** text
  search (`grep`). It resolves symbols semantically, so it does not confuse a
  module path with a like-named identifier the way a text match can.
  - **The LSP tool here is read-only** — `goToDefinition`, `findReferences`,
    `hover`, `documentSymbol`, `workspaceSymbol`, and call hierarchy. It does
    **not** expose rename / code-action / `applyEdit`. (Full rust-analyzer in an
    editor like Neovim/VS Code does; this harness surfaces only the query half.)
  - For a refactor an editor would drive with *rename* (e.g. renaming a module
    and rewriting every `crate::…` path to it), first use `findReferences` to
    enumerate the sites, then apply the edits yourself, and rely on the compiler
    (`cargo build`/`cargo check`) as the reference checker — every missed
    reference is a hard error pointing at the exact line. Prefer this over a
    blind `sed` rename, which can silently mangle a colliding name.

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
  **AGPL-3.0-only**. Because it is a *fork of beads*, it covers the same ground
  as `bd`; see "Which tracker" below before filing anything.

Using them here is deliberate: this workspace is their proving ground, so
**reach for them first** where they cover the task, and **report every rough
edge you hit** (see "Raising issues" below).

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

**Document management: `kovan` is preferred over `kopitiam` (HARD RULE).**
For ingesting, cataloguing and citing literature, use this workspace's own
`kovan` CLI (`crates/kovan-cli`, binary `kovan`) rather than kopitiam's
PDF tooling. `kovan lit import <pdf> --json-out <…> --markdown-out <…>` produces
a `KovanDocument` — the canonical on-disk form — alongside the Markdown body,
and `kovan lit bibtex` / `kovan lit outline` work from it. That keeps every
ingested document inside the project's own knowledge layer with its metadata
and provenance intact, instead of leaving a loose Markdown file with no record
of where it came from.

- Build it from the workspace (`cargo build --release -p kovan-cli`) — it is a
  member crate, not something to `cargo install` from crates.io.
- **Respect the open/proprietary split.** Public, openly published literature
  goes under `crates/kovan-literature/open/` and is committable; anything
  restricted goes under `proprietary/`, which is gitignored. Confirm which a
  document is *before* ingesting it — see `DATA_POLICY.md`. The root-level
  `collaboration/` directory is gitignored scratch space and its contents are
  **not** automatically open; ask if the provenance is not stated.
- `kopitiam pdf2md` remains fine for a quick one-off conversion where no
  catalogue entry is wanted, but it is the fallback, not the default.

**Known friction (as of kopitiam 0.2.4, verified 2026-07-28):**

- `kopitiam check` / `kopitiam test` expose **no `--release` or profile flag**
  and run the `dev` profile, which conflicts with this workspace's mandatory
  release-mode rule. Until that is fixed upstream, use `kopitiam check
  --compact` for fast iteration but **still run the mandated `cargo check
  --workspace --lib --tests` / `cargo test --release`** before calling work
  done. Do not let kopitiam's default profile silently replace the release-mode
  requirement.

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
beads-rs, so it overlaps `bd` rather than complementing it. The mandatory
"Issue tracking & roadmap" section below remains authoritative for which one
holds this workspace's issues; installing and exercising `bn` is a dogfooding
obligation and does **not** on its own move the issue store. Do not migrate the
tracker without an explicit instruction from the maintainer.

**This rule relaxes nothing.** The release-mode rule, the working-hours
guardrail, never-auto-commit/push, the Android/Termux portability rule, and the
data-policy rules all still bind when using either tool.

## Agent-fleet progress reporting (HARD RULE, container-timeout prevention)

**Whenever you spawn an agent fleet — any background subagent, parallel agent
wave, or `Workflow` orchestration — you MUST post a summarised progress update
in chat at least every 15 minutes until the fleet is done.** This is a hard
rule, not a courtesy: long silent stretches while agents work let the remote
execution container idle out, and a timed-out container loses the session's
in-flight work.

**What this requires in practice:**

- **Never go quiet waiting on a fleet.** If agents are still running and ~15
  minutes have passed since your last chat message, post an update even when
  there is nothing new to report ("3 of 7 agents still running, no results
  back yet" is a valid update).
- **Summarise, don't dump.** Report what has landed, what is still in flight,
  and anything that failed or needs a decision. Do not paste raw subagent
  transcripts.
- **Schedule the heartbeat, don't rely on remembering it.** Use `send_later`
  (or an equivalent wake-up) at 15-minute intervals when the fleet may outlast
  a single turn, so the update fires even if no agent has reported back.
- **Keep it up until the fleet is fully done**, then post a final summary.
  Stop the heartbeat once there is nothing left running.
- This does **not** relax any other rule — in particular the working-hours
  guardrail above (do not run fleets outside active hours in the first place)
  and the never-auto-commit/push rule.

## Token accounting on every commit (mandatory, this workspace + all repos here)

**Every commit in this workspace — and in every repository worked on here — must
carry an API-token-usage trailer, and a per-commit token ledger is kept at
`docs/token-usage.md`.** This gives the maintainer a clear, honest accounting of
the Claude/API tokens spent producing each commit. It is automated by two git
hooks so it cannot be forgotten:

- **`docs/historian/token_usage.py`** is the single source for token accounting —
  it does **both** the write side and the query side. On the write side it reads
  the Claude Code session transcripts (`~/.claude/projects/<slug>/*.jsonl` — the
  same data `ccusage` reads) and attributes the **token delta since the previous
  commit** to each new commit. On the query side,
  `python3 docs/historian/token_usage.py query --from DDMMYY --to DDMMYY
  [--branch develop] [--per-commit] [--json]` sums the token usage **recorded in
  the git commit trailers** over any time period (reads the durable git record,
  not the live transcript).
- **`.githooks/prepare-commit-msg`** stamps the commit message with an
  `API-Usage-Since-Last-Commit:` trailer (`total`, `in`, `out`, `cache_read`,
  `cache_write`, `source`) plus an `API-Usage-Session-Cumulative:` line. It is
  idempotent (amend/rebase safe).
- **`.githooks/post-commit`** advances the baseline and regenerates the local
  `docs/token-usage.md` summary from the commit-message trailers.

**Source of truth: the per-commit trailers, not the markdown.** The durable
record is the `API-Usage-*` trailer in each commit message (queryable across any
window with `python3 docs/historian/token_usage.py query --from DDMMYY --to
DDMMYY`). `docs/token-usage.md` is a **regenerable local summary and is
gitignored** — it is deliberately *not* tracked, because committing a generated
file on many branches caused recurring merge conflicts. Never re-track it.

**Rules:**

- **Token usage MUST always be documented as a summary under the commit
  message.** Every commit carries its usage summary in the message body — the
  `API-Usage-Since-Last-Commit:` trailer (with `total`, `in`, `out`,
  `cache_read`, `cache_write`, `source`) plus the
  `API-Usage-Session-Cumulative:` line, appended below the prose. This is not
  optional and not conditional on the kind of change. The `prepare-commit-msg`
  hook writes it automatically, so in practice the rule is: **let the hook run,
  and never remove or edit what it appended.** If you find a commit being made
  without it, the hooks are not installed for that clone — run
  `./scripts/install-token-hooks.sh` before committing rather than
  hand-writing a summary.
- **Do not strip or fake the trailer.** The numbers come straight from the
  transcripts; nothing is estimated or invented. A commit made outside a Claude
  session legitimately shows `total=0 source=none` — that is correct, not a bug,
  so never hand-write a nonzero number.
- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read (prompt-
  cache re-reads of the growing context) usually dominates and is shown
  separately — do not collapse it into a single figure that hides the split.
- **Install per clone:** `./scripts/install-token-hooks.sh` (sets the local
  `core.hooksPath` to `.githooks` and initialises the baseline). `core.hooksPath`
  is a local, uncommitted config, so every fresh clone must run it once. The
  hooks and script are version-controlled, so they travel with the repo.
- **`docs/token-usage.md` is a generated, gitignored local summary — never
  hand-edit it and never `git add` it.** Rebuild any time with `python3
  docs/historian/token_usage.py report`; query the tracked trailers directly with
  the `query` subcommand. Because it is gitignored, `bd`/hook regens no longer
  dirty the tree or conflict on merge.
- **New repositories added to a session here inherit this rule** — copy
  `docs/historian/token_usage.py` + `.githooks/` + `scripts/install-token-hooks.sh`
  in and run the installer as part of onboarding that repo.
- This does **not** relax the never-auto-commit/push rule above: the hooks only
  act *when a commit the user asked for is being made*; they never initiate one.

## Historian report before every merge to `main` (mandatory)

**Before merging `develop` into `main`, generate a "historian" report** — a
python-generated markdown file accounting for the **API tokens spent** and the
**lines / KLOC written** across the window of `develop` history being released,
listing the commits over a `DDMMYY..DDMMYY` date range. Both the generator and
the reports live under **`docs/historian/`** at the workspace root.

- **Generate it:**
  `python3 docs/historian/historian.py --from DDMMYY --to DDMMYY`
  (`DDMMYY` = day-month-year, 2-digit year). With no `--from`, it defaults to
  "everything on `develop` not yet on `main`, up to today". Output is written to
  `docs/historian/historian_<from>_to_<to>.md`.
- **What it contains:** total lines added/removed/net (all files + Rust-only),
  total tokens broken out (`in`/`out`/`cache_read`/`cache_write`/`total`), a
  per-crate lines-added breakdown, and a per-commit ledger.
- **Sources, not estimates.** Tokens come from the `API-Usage-Since-Last-Commit`
  commit trailers (§ token accounting above); lines come from
  `git log --numstat --no-merges` over the range. Commits predating the token
  hooks legitimately show *no token data* — that is correct, not a gap.
- **Commit the generated report alongside the `develop`→`main` merge**, so each
  release carries its own accounting. Do not hand-edit the generated markdown.

## Issue tracking & roadmap — beads (mandatory when available)

This workspace tracks issues and per-crate roadmap progress with **beads-rs**
(`bd`). It is a dependency-aware issue tracker whose canonical data lives **in
git refs** — `refs/heads/beads/store` (holding `state.jsonl`, `deps.jsonl`,
`tombstones.jsonl`, `meta.json`), with backups under `refs/beads/backup/*`. A
background **`bd daemon`** debounces and **auto-syncs** that ref to the git
remote (a private channel, separate from `refs/heads/*` code branches).
`.beads/issues.jsonl` is now only a local compat-export **symlink**, not the
source of truth.

> **Migrated 2026-07-20** from the Go beads/Dolt implementation to **beads-rs**.
> The old Go binary is parked at `~/.local/bin/bd-go.deprecated`; `bd` now
> resolves to beads-rs. There is **no `bd dolt`**, no `.beads/` Dolt DB, and no
> `refs/dolt/data` sync anymore — those references elsewhere in this file and in
> per-crate docs are stale and tracked for cleanup (see the doc-sweep bead). The
> daemon auto-syncing `beads/store` is beads-rs's designed, opted-in behavior
> and does **not** relax the rule against committing/pushing **code** without
> explicit approval.

- **RUST BEADS ONLY.** Beads here is the **Rust** implementation — the
  [`beads-rs`](https://crates.io/crates/beads-rs) crate
  (`github.com/delightful-ai/beads-rs`), binary `bd`. Do **not** install or
  use any Go build of beads. If only a Go `bd` is on this machine, treat beads
  as unavailable (see the "not available" bullet below) and fall back to a
  short markdown note rather than filing into it.
- **Install:** `cargo install beads-rs` (binary `bd`). First-time setup in a
  repo: `bd init`, then `bd setup claude`, then `bd onboard`. Migrate a Go
  export with `bd migrate from-go --input <issues.jsonl>` (comment ids must be
  integers, every record needs a `description`, and unknown issue types like
  `decision` must be remapped — the importer is strict).
- **Standing rule: if `bd` is available on this machine, you MUST use it** for
  all task/roadmap tracking and progress bookkeeping — in preference to
  TodoWrite / TaskCreate / ad-hoc markdown TODO lists. Create/close/update
  beads as work happens; file a bead for any follow-up you discover.
- **If `bd` is *not* available** — e.g. an OS or environment without bash or
  without a beads build (Android, a locked-down sandbox, etc.) — that is fine:
  beads is optional there. Do **not** block work on it; fall back to the
  harness task tools and note in your hand-off that beads wasn't updated.
- **Roadmap / progress summaries come from beads.** When the user asks "where
  are we" / "summarise progress" / "what's the roadmap", read it out of beads
  (`bd list`, `bd ready`, `bd show <id>`, `bd status`, `bd dep`) rather than
  re-deriving from scattered docs.
  One epic per member crate; child beads are that crate's workstreams.
- **Relationship to the memory system.** Beads and the per-project memory
  files (`~/.claude/projects/<slug>/memory/`) are complementary and **both
  stay in use**: beads tracks *tasks / roadmap / open work*; the memory files
  track *durable facts, user preferences, and feedback*. The auto-generated
  "Beads Issue Tracker" block lower in this file says to drop `MEMORY.md` —
  that does **not** apply here; the memory workflow is unchanged. When in
  doubt: a thing to *do or finish* → bead; a thing to *remember about how the
  user works or a settled fact* → memory.
- **After a plan is approved (exiting plan mode), convert it into beads
  before writing any code.** One epic per new crate the plan introduces (or
  a child under the relevant crate's existing epic, for plans scoped to one
  crate); one child bead per part/module/deliverable the plan names, with
  `bd dep add` wiring the real ordering constraints between them (e.g. "this
  module wraps a type from that other bead's work" → a dependency edge, not
  just prose). Do this even if the plan is also saved as a markdown file —
  the markdown is for human reading, beads is what `bd ready`/`bd show`
  make queryable across a session boundary. This is a standing rule, not a
  one-off — apply it to every future plan-mode session in this workspace.

## README / Markdown format (mandatory)

**Every `README.md` in this workspace must render correctly on GitHub
(GitHub-Flavored Markdown).** GitHub renders LaTeX math via MathJax (`$...$`
inline, `$$...$$` display), so math *is* allowed — but keep it to a conservative
subset that also survives editor previewers. **No exotic math.** Concretely:

- **No matrix/array environments** (`\begin{bmatrix}`, `pmatrix`, `array`) and
  **no `\begin{cases}`** — write a matrix system or a piecewise definition as
  separate `$$...$$` equations, one per line, labelled in prose or with a
  trailing `\quad (\text{...})`.
- **No** `\boxed`, `\underbrace`, `\displaystyle`, `\tfrac`/`\dfrac` (use
  `\frac`), or negative-space `\!`.
- **No Unicode Greek or operators inside math** — use `\gamma`, `\rho`, `\xi`,
  `-`, `\le`, `\pm`, etc. (Unicode is fine in ordinary prose and in inline
  code spans.)
- Write superscripts/subscripts with explicit braces (`(\hat{u}^*)^2`, not
  `\hat u^{*2}`).

**Check every README before finishing.** Prefer `pandoc` when available — it
validates both markdown structure *and* the LaTeX math (via its texmath engine):

```bash
pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null
```

Exit 0 with **no warnings** means all math converted (any malformed equation
prints a `[WARNING] Could not convert TeX math …`). Note: without `--mathml`,
pandoc emits harmless "rendering as TeX" warnings for every equation — those are
not errors, so always pass `--mathml` when validating.

If `pandoc` is not installed, fall back to `cmark-gfm` for a structure-only
check (`cmark-gfm -e table -e strikethrough -e tagfilter README.md > /dev/null`,
exit 0, no warnings) — but `cmark-gfm` does not render math, so also eyeball the
math against the subset above.

## Verification & validation documentation (mandatory)

**Whenever verification and validation (V&V) are concerned, the documentation
must contain both the methodology and the results of the test.** This is a hard
rule for anything that checks physics against a reference — benchmark comparisons,
cross-section reconstruction gates, convergence studies, fidelity comparisons.

Concretely, the doc comment (or `docs/` entry) for a V&V test must state:

- **Methodology** — what is being computed, the reference/benchmark it is judged
  against, the inputs (geometry, material, data source, tolerances), and the pass
  criterion.
- **Results** — the actual measured numbers *with uncertainty* (e.g. `k_eff =
  1.12451 ± 0.00202`, `+12451 pcm` from benchmark), the date/data-version they
  were taken on, and the interpretation (what the result implies about the model).

A V&V test whose documentation states only what it does, but not what it produced,
is incomplete. Record results where a reader meets the test: in the `///` doc
comment of the test/example itself, and — for iterative studies worth citing in a
paper — in the relevant `docs/` development-history entry.


## Human interface layer (mandatory design principle)

**Every public API in this workspace must be navigable by a Rust developer using
rust-analyzer alone — no AI assistant, no prior knowledge of the codebase.**

This is a hard rule, not a goal. The human mind cannot hold large amounts of context
simultaneously. If understanding a function requires recalling three other modules at
once, the interface is wrong regardless of how correct the physics is.

### What this requires in practice

**Every public function, type, trait, and module must have a `///` or `//!` doc comment that answers:**
- What physical quantity does this compute or represent?
- What are the valid input ranges and assumptions?
- What units do parameters represent — even when `uom` enforces them, spell it out for human readers.

**Complex `uom` types must have named type aliases.** A user hovering in their editor
should see `SpecificEnthalpy`, not a raw `Quantity<ISQ<...>, SI<f64>, f64>`.

**Each module's `lib.rs` / `mod.rs` must have a `//!` module-level comment** that
explains what belongs in the module and what does not. This is the map a new user
reads first.

**Examples are the primary entry point, not the API docs.** A user must be able to
find an example, read it top-to-bottom without jumping to other files, and understand
what crate they need and how to call it.

### What AI assistants must not do

- Do not add complexity (extra type parameters, trait indirection, macro magic) in
  the name of correctness or generality if it raises the mental context load for a
  human reader.
- Do not leave public items undocumented. If you add or modify a public item, add or
  update its `///` doc comment in the same change.
- Do not write examples that require reading internal modules to understand.

## Bookkeeping pass (maintainer command)

When the maintainer asks for a **"bookkeeping pass"** (or "bookkeeping", "book
keeping", "update the docs + flags") over one or more crates, run this fixed
routine. It keeps the docs, the completeness flags, and the issue tracker honest
and in sync with the code. It is a recurring command, not a one-off.

**The four steps:**

1. **Doc-comment pass — fill gaps + fix stale (NOT a rewrite).** For every
   public `fn` / `struct` / `enum` / `trait` / `mod` in scope: add an accurate
   `///` / `//!` where missing; fix any doc that contradicts the current code
   (stale "scaffold only" / `todo!()` claims, wrong counts, renamed items);
   and **leave already-accurate docs untouched** — do not reword good docs.
   Obey the "Human interface layer" rule above (what physical quantity, valid
   ranges/assumptions, units even when `uom`-typed). Never strip `uom`.

   **Then regenerate the rustdoc → markdown API mirror** for each crate whose
   doc comments changed, so `docs/api.md` stays in sync with the code:

   ```bash
   python3 scripts/gen_api_docs.py <crate-dir-name>   # e.g. outram-foam-basic-lib
   ```

   This runs `cargo +nightly doc --no-deps` → rustdoc JSON → the `rustdoc-md`
   binary → `crates/<crate>/docs/api.md`. It needs a nightly toolchain
   (`rustup toolchain install nightly`) and `rustdoc-md`
   (`cargo install rustdoc-md --locked`); if either is missing, install it or
   note in the hand-off that the mirror wasn't regenerated. `docs/` is
   `exclude`d from the packaged crate, so this mirror is repo-only and never
   ships to crates.io.

2. **Completeness flags in the README.** Every crate's `README.md` carries a
   **`## Bookkeeping status`** block with two axes the *human maintainer* must
   personally sign off:
   - **Verification & Validation (V&V) — human-reviewed**
   - **Human / user interface — human-reviewed**

   Both default to **❌ Not yet manually checked** and a crate is marked
   **INCOMPLETE** until the maintainer clears both. AI assistants must **not**
   flip either axis to checked/✅ on their own — only the human does, because
   these axes record *human* review (see `RESPONSIBLE_USE.md`: AI output is
   untrusted draft material until a human reviews it). A crate flagged
   INCOMPLETE on either axis is not ready to be described as validated or
   trusted. The canonical block:

   ```markdown
   ## Bookkeeping status

   > Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
   > pass" command). A crate is **complete** only once the maintainer has
   > personally signed off on BOTH axes below.

   | Axis | Status |
   |---|---|
   | Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
   | Human / user interface — human-reviewed | ❌ Not yet manually checked |

   **Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
   ```

3. **Staleness audit.** Sweep the READMEs, beads, and every markdown file
   (recursively) for drift versus the actual code/state: internal
   contradictions, references to renamed/removed crates or files, "planned/TODO"
   items that are actually done, wrong member lists or crate counts, beads that
   should be closed (or reopened). Fix in-crate drift; for cross-cutting or bead
   changes, report candidates rather than silently editing — beads are closed by
   the maintainer's decision, and the read-only auditor never mutates them.

4. **Codify / update** this command here if the routine itself changes.

**How to run it as a fleet:** partition strictly by crate (one agent per crate,
no shared files → `cargo fmt -p` is safe to avoid per the parallel-agent rule),
plus a separate **read-only** agent for the cross-cutting markdown + beads
staleness audit (it must skip the crates being actively edited to avoid read
races). Commit any pending verified work first so the tree is clean, and
**exclude from the pass any crate with a publish in flight** (an uncommitted
doc edit trips `cargo publish`'s dirty-tree guard).

## Rust design rules (mandatory)

### No trait objects — use enums for dispatch

Do not use `Box<dyn Trait>`, `&dyn Trait`, or `Arc<dyn Trait>` for dispatch.
Use enums instead. The set of physics models (EOS, turbulence models, numerical
schemes, boundary conditions) is closed and known at compile time — enums are
the right tool.

Benefits over trait objects:
- **Exhaustiveness** — adding a new variant forces every `match` site to handle it; a missing case is a compile error, not a runtime surprise
- **Zero heap allocation** — the enum lives inline in its containing struct
- **rust-analyzer navigability** — Go-to-definition works on enum variants; it often fails on `dyn Trait` implementations

Traits are still useful as a **compiler-enforced contract** on each concrete
struct — the compiler verifies every model implements the right methods. They
are just not used for runtime dispatch. The pattern:

```rust
// Trait enforces the interface — compiler checks every model satisfies it
pub trait TurbulenceKernel {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;
    fn correct(&mut self);
}

// Enum dispatches without Box or dyn
pub enum TurbulenceModel {
    Laminar(LaminarModel),
    KOmegaSST(KOmegaSSTModel),
    KEpsilon(KEpsilonModel),
}

impl TurbulenceModel {
    pub fn correct(&mut self) {
        match self {
            Self::Laminar(m)   => m.correct(),
            Self::KOmegaSST(m) => m.correct(),
            Self::KEpsilon(m)  => m.correct(),
        }
    }
}
```

### No `Box<T>`

Do not use `Box<T>`. Own data by value or share it with `Arc<T>`.
`Box<T>` is only justified for recursive data structures (trees, linked lists),
which do not appear in this codebase.

### No lifetime parameters

Do not add lifetime parameters (`'a`) to structs, trait definitions, or impl
blocks. Own data by value, or share it with `Arc<T>`.

| Instead of | Use |
|---|---|
| `&'a FvMesh` in a struct | `Arc<FvMesh>` |
| `&'a f64` / uom quantity in a struct | own by value — all uom types are `Copy` |
| `Box<dyn Fn(&'a T) -> U>` | newtype struct that owns its captured state |
| `&'a Cell` for graph/topology links | `CellId(usize)` — index into a `Vec` |

### Shared state: `Arc<RwLock<T>>` over channels

For shared mutable simulation state (fields, solver coefficients), use
`Arc<RwLock<T>>`. For data that is read-only after construction (mesh topology,
lookup tables, material constants), use `Arc<T>` with no lock.

Prefer `RwLock<T>` over `Mutex<T>` — `RwLock` allows concurrent reads from
multiple threads; `Mutex` serialises even read-only access, which defeats
parallelism during the compute phase of a timestep.

Do not use channels (`mpsc`, `crossbeam`) for simulation state. Channels suit
pipeline patterns where data is produced, consumed, and discarded. The simulation
timestep loop is a shared-state pattern — threads compute over non-overlapping
regions of the same fields, then synchronise.

## What this is

**OUTRAM PARK backend** — the Cargo **workspace** that houses the OUTRAM PARK
(Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit) Rust suite.
Several crates that used to live as independent GitHub repositories under
`github.com/theodoreOnzGit` are now consolidated here under `crates/` and are
built, tested, and published from this single repository.

## Members

| Crate (`crates/…`) | Role | License |
|---|---|---|
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library (real-time simulators) | **Apache-2.0** |
| `teh-o-prke` | Point Reactor Kinetics (PRKE) for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| `tuas_boussinesq_solver` | Thermal-hydraulics (Boussinesq single-phase) solver — TUAS | GPL-3.0 |
| `tampines-steam-tables` | IAPWS-IF97 steam/water properties + steam-turbine equations — TAMPINES | GPL-3.0 |
| `outram-foam-basic-lib` | Pure-Rust translation of the OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics kernels, fields, mesh, FV operators, fluid/solid thermo | GPL-3.0 |
| `njoy-outram-park-fork` | **All nuclear data** — NJOY2016 ENDF port (RECONR/BROADR/THERMR/ACER), the Faddeeva kernel, windowed-multipole evaluation, lean-ACE + WMP data blobs, ν̄/χ. Exposes the `XsProvider` surface other crates pull cross sections from. | GPL-3.0 |
| `outram-mc-libs` | **Monte Carlo transport** — CSG geometry, particle tracking, k-eigenvalue, delta (Woodcock) tracking for doubly heterogeneous media, depletion. **Data-free**: pulls cross sections from `njoy-outram-park-fork`. | GPL-3.0 |
| `tampines` | Central thermal-hydraulic framework — composes `tuas`, `outram-park-fork-coolprop`, `tampines-steam-tables`, `outram-foam-basic-lib`, `chem-eng…` | GPL-3.0 |
| `outram-park-fork-coolprop` | Pure-Rust fork of **CoolProp** — Helmholtz-EOS thermophysical properties (137 fluids, incompressibles, humid air, mixtures). Independent fork, not official CoolProp. | GPL-3.0 |
| `outram-park-fork-offbeat` | Pure-Rust fork of **OFFBEAT** (foam-for-nuclear) — nuclear fuel performance: solid mechanics with eigenstrain, rheology (plasticity/creep), fuel-cladding gap and contact, ~70 material property correlations, burnup/fast-flux/FGR, cladding corrosion. Independent fork, not official OFFBEAT. | GPL-3.0 |
| `outram-park-fork-dwsim-libs` | Pure-Rust fork of **DWSIM** process-simulation building blocks. Independent fork. | GPL-3.0 |
| `outram-foam-turbulence-lib` | OpenFOAM turbulence closures (k-ω SST implemented; k-ε / k-ω / Spalart-Allmaras / Smagorinsky scaffolded) on `outram-foam-basic-lib` | GPL-3.0 |
| `outram-foam-appbuilder-lib` | OpenFOAM solver-application layer (pimpleFoam / rhoCentralFoam / rhoPimpleFoam) + case I/O; host of the in-progress **GeN-Foam** deterministic-neutronics + TH port | GPL-3.0 |
| `boon-lay` | TRISO-particle / Lagrangian decay simulator (BOON-LAY); includes the TRISO-ATOPS fork | GPL-3.0 |
| `nee_soon` | Integration / coupling layer — composes MC + deterministic/TH + nuclear data + PRKE (mostly scaffold; the prompt-excursion path is wired to `teh-o-prke`) | GPL-3.0 |
| `bedok` | Systems-level multiphysics coupling — 3-D nodal-diffusion neutronics coupled to channel TH, at the fidelity band **above 1-D neutronics and below CFD**. Rust translation of a MATLAB implementation by Than Yan Ren (SNRSI), used with the author's permission. Carries a committed NEACRP BWR transient case; the benchmark gates are `#[ignore]`d and **have not been run**, so no parity claim is made. | GPL-3.0 |
| `outram-park-digital-twin-engine` | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only; formerly `outram-park-digital-twin-gui`) | GPL-3.0 |
| `kovan-common` | **KOVAN** knowledge layer — shared canonical types (`KovanDocument`, `KovanSymbol`, …). The Rust struct is the source of truth. | GPL-3.0 |
| `kovan-discovery` | KOVAN file discovery + text search — the `fd` (`ignore`) walker and ripgrep (`grep-*`) engine. Offline, deterministic. | GPL-3.0 |
| `kovan-literature` | KOVAN literature archive — PDF → Markdown (`pulldown-cmark`) → `KovanDocument` → BibTeX. `open/` committable, `proprietary/` gitignored. | GPL-3.0 |
| `kovan-semantics` | KOVAN repo-understanding — ripgrep-first, escalating to language servers (rust-analyzer / clangd / Pyright / fortls). Does not reimplement compilers. | GPL-3.0 |
| `kovan-codegen` | KOVAN deterministic code generation — templates for known numerical methods (root finders, linear/nonlinear/ODE solvers). Not an AI assistant. | GPL-3.0 |
| `kovan-cli` (bin `kovan`) | KOVAN **agent-facing** CLI (`clap`) — line-oriented output for Claude Code and other coding agents. | GPL-3.0 |
| `kovan-tui` (bin) | KOVAN **human-facing** TUI (`ratatui`). Desktop scope: on Android it compiles to a CLI-redirect stub. | GPL-3.0 |
| `outram-blender` | Mesh-authoring frontend (GPL fork of Blender's mesh architecture) — headless surface authoring with opt-in **Monte Carlo** (`mc-export` → `sim` → MC Studio) and **OpenFOAM volume-meshing** (`foam-mesh` → `foam_mesh` → tet-dual Mesh Studio) solver bridges. Not affiliated with the Blender Foundation. | GPL-3.0 |
| `outram-park-fork-cfmesh` | Pure-Rust fork of **cfMesh** — Cartesian/tetrahedral/polyhedral volume meshing with boundary layers; `pipeline::surface_to_tet_dual_mesh` consumes an `outram-blender` surface and emits an `outram-foam` polyMesh. Independent fork, not official cfMesh. | GPL-3.0 |
| `outram-foam-mesh` | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, ideasUnvToFoam, polyDualMesh). Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-foam-cli` | OpenFOAM-style command-line utilities (blockMesh, pimpleFoam, gen-foam, …) as terminal binaries. Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-foam-multiphase` | Phase-II multiphase CFD — drift-flux first (Euler-Euler two-fluid, wall boiling, CHF, dryout planned). Reference physics for TAMPINES reduced-order models. Scaffold, no human V&V. Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-park-fork-liggghts` | Pure-Rust granular-DEM library — particles, contact mechanics, thermal DEM, pebble/packed-bed physics (ports LIGGGHTS/LAMMPS-granular). LIGGGHTS-PUBLIC is GPL-2-or-later (GPL-3-compatible; see `NOTICE`). Scaffold. | GPL-3.0 |
| `outram-park-fork-pflotran` | Pure-Rust fork of **PFLOTRAN** — subsurface flow & reactive transport; enum-dispatched, `uom`-typed, no PETSc/FFI/MPI. Scaffold, no human V&V. Independent fork. | GPL-3.0 |
| `outram-park-mpi` | Pure-Rust **MPICH** subset — the MPI-3 API surface (communicators, datatypes, point-to-point, core collectives) over a shared-memory threads-as-ranks transport. No C/FFI, Android-buildable. Scaffold. Not affiliated with MPICH. | GPL-3.0 |
| `outram-park-fork-moltres` | **Circulating-fuel MSR** multiphysics on the `outram-foam-basic-lib` FV layer — multigroup neutron diffusion + delayed-neutron **precursor drift** + salt heat transfer, reimplemented from the LGPL-2.1 **Moltres** formulation on `FvMesh`/`fvm` rather than MOOSE/PETSc finite elements. Steady eigenvalue only (no coupled flux transient), and **no crate depends on it yet**. Untrusted AI-assisted draft, no human V&V. Independent fork, not affiliated with Moltres/ARFC. | GPL-3.0 |
| `outram-park-fork-onix` | Pure-Rust fork of **ONIX** (MIT upstream) — Bateman/CRAM depletion + fission-product inventory for the MSRE digital twin. Untrusted AI-assisted draft, no human V&V. Independent fork, not affiliated with ONIX. | GPL-3.0 |
| `outram-park-fork-thermochimica` | Pure-Rust fork of **ORNL Thermochimica** (BSD-3) — molten-salt Gibbs-energy-minimisation thermochemistry (fission-product speciation, redox, solubility) for the MSRE digital twin. Scaffold, no human V&V. Independent fork, not affiliated with ORNL. | GPL-3.0 |

> **KOVAN** is the deterministic *knowledge* layer (literature + semantics +
> codegen), interfaced two ways: the `kovan` **CLI** for agents and the
> `kovan-tui` **TUI** for humans. Offline / Android-first, no cloud, no
> Tree-sitter/SQLite/vector-store. Full design spec: **`docs/kovan.md`**
> (+ `docs/kovan-architecture.md`). Non-GUI kovan crates build for Android;
> `ratatui` is pulled only under `cfg(not(target_os = "android"))`.

> **MSRE digital-twin group:** `outram-park-fork-moltres` (circulating-fuel
> neutronics), `outram-park-fork-onix` (depletion) and
> `outram-park-fork-thermochimica` (salt thermochemistry) exist to serve the
> MSRE digital twin and are tracked under the **`op-6w0`** epic. All three are
> AI-assisted drafts with no human V&V, and none is wired into a simulator yet
> — do not describe any of them as validated. Scoping: `docs/reactor-scoping/msre.md`.

> **Neutronics architecture:** the responsibility split (nuclear data ⟂ Monte
> Carlo ⟂ deterministic/TH ⟂ coupling), the dependency graph, and phasing live in
> **`docs/architecture.md`**. Rule of thumb: *all* cross-section /
> nuclear-data code belongs in `njoy-outram-park-fork`; transport crates are
> data-free and pull from it.

**Planned future crates** (not yet in the workspace):

| Crate | Depends on | Targets |
|---|---|---|
| `openfoam-icof` | `outram-foam-basic-lib` | **icoFoam** (incompressible laminar PISO) |
| `openfoam-cht` | `outram-foam-basic-lib` | **chtMultiRegionFoam** (conjugate heat transfer, multi-region) |
| `openfoam-rho` | `outram-foam-basic-lib` | **rhoPimpleFoam** / **sonicFoam** (compressible) |
| **GenFOAM** (deterministic + TH) | *ported inside* `outram-foam-appbuilder-lib` | Deterministic neutronics + thermal hydraulics. On hold until the MC + nuclear-data path matures. |

> `nee-soon` is no longer "planned" — it exists as the `nee_soon` member crate
> (see the Members table above); it remains mostly scaffold.

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`outram-foam-basic-lib`.  `outram-foam-basic-lib` provides the mathematical building
blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling logic,
and turbulence model registries belong in solver-specific crates so that
`outram-foam-basic-lib` stays publishable independently and is reusable by other
projects.

Internal dependency edges (all by **path**, not crates.io):
`teh-o-prke → tuas` (dev); `teh-o-prke → chem-eng` (real, non-dev -- `nordheim_fuchs`'s
optional reactivity-input driver reuses `chem-eng`'s `TransferFnFirstOrder`);
`tuas` dev-deps → `chem-eng`, `teh-o-prke`;
`nee_soon → teh-o-prke` (real -- `NeeSoon::new_prompt_excursion_model` exposes
`teh-o-prke::nordheim_fuchs::NordheimFuchsExactTimestepper`);
`outram-park-digital-twin-engine → nee_soon` (real -- `components::ReactorVesselVisual`
wraps `NordheimFuchsExactTimestepper`);
`tampines` dev-deps → `{tuas, teh-o-prke, chem-eng}` (the FHR simulator examples use TUAS —
the `tampines` **library** itself is TUAS-free).
`outram-foam-basic-lib` has no internal deps (pure third-party: `uom`, `ndarray`, `thiserror`).
`njoy-outram-park-fork` is lean (`thiserror`, `uom`; no BLAS) so data consumers stay light.
Neutronics edges (target): `outram-mc-libs → njoy-outram-park-fork` (cross sections; declared in
root workspace deps, wiring deferred); `nee-soon → {outram-mc-libs, njoy-outram-park-fork, teh-o-prke, outram-foam-appbuilder-lib}`.

## Dependency policy — single source of truth

All third-party versions live in the root `[workspace.dependencies]`. Members
inherit them with `<dep>.workspace = true`, so versions **cannot drift**. **When
changing a shared dependency, edit the root `Cargo.toml` only.** The one
exception is `ndarray-linalg`, whose BLAS backend feature is chosen per-target by
each member (`openblas-system` on unix, `intel-mkl-static` on windows/macos).

See `docs/workspace-maintenance.md` for the rationale and the planned
`ndarray-linalg` removal from TUAS.

## Android / Termux portability (HARD RULE for non-GUI code)

**Hard rule (not a default): every crate's non-GUI library code MUST compile on
Termux — native, on-device Android — with Android-hostile pieces held off behind
Android feature gates.** "Compiles on Termux" is the acceptance bar: a build run
*inside Termux* (native `aarch64-linux-android`, no NDK cross-toolchain, no system
BLAS/LAPACK, no C/Fortran toolchain) must succeed for every non-GUI library. This
does not bend for convenience — if a change cannot build on Termux, it is not done
until the offending dependency/test/example is gated off Android in the *same*
change. Workspace-wide tracking lives in the **`op-zfr` "Android support" epic**.

Termux specifics to keep in mind:

- **Termux builds natively on the device**, so the target is `aarch64-linux-android`
  and **`target_os = "android"`** (not `"linux"`). Every gate below keys off that.
- Prefer an explicit **Cargo feature** (e.g. `android`, or an inverted
  `native-blas`/`gui` feature that is simply *not* enabled on Termux) plus the
  `cfg(target_os = "android")` target gate, so a Termux user gets a working build
  from the default feature set with no manual flag-twiddling.
- No system package manager for BLAS/LAPACK/GUI libs is assumed to exist on Termux.

**Every crate's non-GUI library code must also compile for Android**
(`aarch64-linux-android` and the armv7/x86_64 emulator targets) when cross-built
from a host. Android has no system BLAS/LAPACK and no easy C/Fortran toolchain, so
**Android-hostile dependencies must not compile on Android** — gate them off by
target rather than letting them break the build.

- **`ndarray-linalg`** (and anything needing system BLAS/LAPACK, or a C/Fortran
  toolchain, or `std`-GUI/windowing) is Android-hostile. Declare it only under
  target-conditional tables — e.g.
  `[target.'cfg(not(target_os = "android"))'.dev-dependencies]` — never as an
  unconditional dependency. (Android's `target_os` is **`"android"`, not
  `"linux"`**, so an existing `cfg(target_os = "linux")` gate already excludes
  it — but do not *rely* on a linux-only gate to mean "not Android" without
  saying so.)
- **Examples/tests/benches count — they are NOT exempt.** A native Termux
  `cargo build` / `cargo test` compiles **examples, integration tests, and
  benches**, so an Android-hostile dep or a desktop-only-API reference in *any*
  of those breaks the on-device build even when the library itself is clean.
  Gate every one that touches an Android-hostile path:
  - **Tests / benches** (no `main` required): put `#![cfg(not(target_os =
    "android"))]` at the top of the file — blanking the whole file on Android is
    fine. Precedent: `outram-foam-basic-lib`'s `tests/matrix_bench.rs`.
  - **Examples / bins** (a `main` *is* required — a blanked file gives "main
    function not found"): add an **Android stub `main`** under `#[cfg(target_os
    = "android")]` that prints a "desktop-only" line, and gate every desktop
    item (`use`/`const`/`fn`/`struct`/…) with `#[cfg(not(target_os =
    "android"))]`. Precedent: `njoy-outram-park-fork`'s
    `examples/gpu_wmp_bench.rs` and `outram-mc-libs`'s
    `examples/godiva_gpu_benchmark.rs`.
- **Only windowing GUI is out of scope — terminal apps are IN scope.** Termux
  *is* a terminal, so a **CLI or a `ratatui` TUI must compile and run on
  Android** like any other non-GUI crate — do not exempt it. What is out of
  scope is **`egui`/`eframe`/`wgpu`-surface/windowing** GUI: keep that behind
  examples/optional bins/target gates, never in a library's unconditional
  build, so the lib still builds headless for Android. Concretely: `kovan-cli`
  (CLI) and `kovan-tui` (`ratatui` TUI — target-gated to a CLI-redirect stub on
  Android) are **in scope and verified building** for `aarch64-linux-android`;
  only `outram-park-digital-twin-engine` (egui/eframe) is a genuine
  GUI exemption.
- **New code follows this by default.** If you add a dep or a test that can't
  build on Android, target-gate it in the same change and note it.
- **The check MUST cover all targets, not just `--lib`.** A `cargo check
  --lib --target aarch64-linux-android` checks *only the library* and silently
  misses broken examples/tests/benches — the exact gap that let the
  `godiva_gpu_benchmark` example ship un-gated (found only by an on-device
  Termux build). The proxy check is therefore **`cargo check -p <crate>
  --all-targets --target aarch64-linux-android`** (needs the Android target +
  NDK / `cargo-ndk`). The **authoritative** check is still a **native Termux
  build** (`cargo build` / `cargo test` run inside Termux on-device), which
  compiles all targets by construction. Never report Android/Termux support as
  verified from a `--lib`-only run. Workspace-wide Android/Termux build tracking
  lives in beads (the **`op-zfr` "Android support" epic**).

## Build & test

Requires a system BLAS (OpenBLAS on Linux/macOS):

```bash
# Arch / EndeavourOS
sudo pacman -S openblas
# Debian / Ubuntu / Mint
sudo apt install libopenblas-dev
```

```bash
cargo build --workspace --release                  # all libraries
cargo check --workspace --lib --tests              # type-check (mode-independent)
cargo test  --workspace --lib --tests --release    # run the test suites
```

Note: a bare `cargo test --workspace` also compiles the **examples**. Use
`--lib --tests` to skip them.

## Reference material (read on demand, not per turn)

These live in `docs/` so they don't load on every turn — consult them only when
doing the relevant task:

- **`docs/workspace-maintenance.md`** — dependency-upgrade rationale, the
  2026-06 consolidation/migration history and version-bump table, the
  crates.io **publishing order and procedure**, Wayland/display notes, and the
  AI model-selection guide.

Each member crate has its own `CLAUDE.md` (crate-specific architecture and
rules) and, where relevant, a crate-level `docs/` for its reference material.

## Singlish mode (optional, for fun)

Optional **chat-only** Singlish style toggle — the full rules, vocabulary, and
the **maintainer-curated corrections log** live in **[`SINGLISH_MODE.md`](./SINGLISH_MODE.md)**.
In short: when the user asks for "Singlish mode" (or "lah mode" etc.), reply in
Singlish for the *conversational prose only*; **code, comments, commit messages,
`README`/`docs`, V&V write-ups, and beads stay clear standard English**, and no
mandatory rule (working-hours guardrail, responsible-use / data policy, V&V docs,
Rust design rules, never-auto-commit/push) is relaxed — correctness and honesty
come first. **When in Singlish mode, read `SINGLISH_MODE.md` and apply its logged
corrections.** Default is standard English; opt-in only.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **beads-rs** (`bd`) for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd claim <id>         # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Persistent durable facts / user preferences: keep using the per-project
  `memory/` + `MEMORY.md` workflow (see the "Issue tracking & roadmap" section
  above — this workspace keeps MEMORY.md; it is **not** dropped).

**Architecture in one line (beads-rs):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`; a background `bd daemon` auto-syncs that ref to the git remote (separate from `refs/heads/*` code branches). No `bd dolt`, no `.beads/` Dolt DB; `.beads/issues.jsonl` is a local compat-export symlink. Migrated off Go beads on 2026-07-20.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or a manual beads sync (`bd sync`) unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
