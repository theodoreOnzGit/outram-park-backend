# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Scope boundary: this file governs `outram-park-backend` ONLY — never a parent folder (HARD RULE)

**This file, and every crate-level `CLAUDE.md` under `crates/*/`, apply
exclusively to the `outram-park-backend` repository tree** — this directory
and everything under it, nothing else. This repo is sometimes checked out
*inside* another person's project — as a git submodule, a subtree, or a plain
nested clone. When that is the case, **none** of these rules — the
never-auto-commit/push policy and its carve-outs, release-mode-only
builds/tests, `bn`/kopi-beans as the mandated tracker, `kovan` dogfooding, the
working-hours guardrail, the token-accounting policy, the Rust design rules
(no trait objects/`Box`/lifetimes), the Android/Termux hard rule, the
responsible-use/data-policy/AI-usage framing, the README math-syntax rule, the
"no Python for docs/accounting" rule — apply to the **parent** project, its
other submodules, or its top-level files. Ever. This binds regardless of
which directory the session's working directory happens to be in.

**How to tell whether a given file or command is in scope.** Resolve its
repository root and compare it against *this* repo's own root:

```bash
git -C <path-in-question> rev-parse --show-toplevel
```

If that root is `outram-park-backend` itself (or, for a submodule mount, the
path where this repo is checked out), the rules in this file apply. If it
resolves to a **different** root — a parent project's own `.git`, or another
sibling repository entirely — **this file has no authority there**, full
stop, no matter how the session got there or what was discussed earlier in
the conversation.

**Concretely, when a parent project merely contains this repo as a
submodule/subrepo:**

- Only apply this file's rules to work that is actually inside this repo's
  own working tree. A commit, build, dependency change, or doc edit anywhere
  else in the parent project is governed by *that* project's own conventions
  (its own `CLAUDE.md` if it has one, or none at all) — not by anything
  written here.
- Do not assume the parent project uses `bn`/kopi-beans, `kovan`, this repo's
  git hooks, or any other tool mandated here. Do not install or run them
  against the parent's own tree on the strength of this file.
- If a task spans both the parent and this submodule (a cross-repo refactor, a
  parent-level commit that bumps this submodule's pinned revision), this file
  governs only the parts that touch this repo's own tree. If it's unclear
  which rule set a given change falls under, ask rather than guessing outward.
- This applies even mid-session: if the working directory moves from inside
  this repo out into the parent project (or a sibling), re-check scope before
  continuing to apply anything from this file. Nothing here "sticks" once
  you've left this repo's tree.

**Why this exists.** `CLAUDE.md` guidance is easy to over-apply once it's been
read into a session — the failure mode is carrying a rule outward onto
unrelated code that merely happens to live in the same checkout, or in a
project that embeds this one. This section exists so that never happens: the
boundary is this repository's own root, determined by its own `.git`, not the
process's current working directory and not how far back the rule was stated
in the conversation.

## Working-hours guardrail (OPT-IN — ask at session start)

**This guardrail is OFF by default.** It is no longer a standing hard rule.
It is enabled **per session, by the user**, in answer to a question you ask at
the start of the session. Changed 2026-08-13 at the maintainer's request,
relayed from a colleague; the history and rationale for the original rule are
preserved under "Why it exists" below.

### Asking (this part IS mandatory)

**Ask once per session, before the first substantive work of that session.**
Use the `AskUserQuestion` tool with one question — *"Enable the working-hours
guardrail for this session?"* — offering **Off (default)** and **On**.

- **Don't ask before purely conversational replies.** Ask at the point a turn
  is about to become real work (writing code, running suites, agent fleets).
- **Ask only once.** The session's answer holds until the session ends; a
  compaction does not reset it. Do not re-prompt, and do not nag if the answer
  was Off.
- **If the question is skipped, dismissed, or unanswered, the answer is Off.**
  Never treat silence as On.
- The user may switch it on or off at any point by saying so in plain words —
  honour that immediately, no confirmation question needed.

### If the answer is Off (the default)

No time check, no hour restriction, no rest-day rule. Work normally. Do not
volunteer reminders about the maintainer's hours or health, and do not
re-litigate the setting.

### If the answer is On

Everything below applies **for the rest of that session, as a hard rule**:

**Check the real local time and day of week** with a system tool before
substantive work — do not infer it from conversation content, a cached date, or
skip the check. Preferred: `date +'%Y-%m-%d %H:%M %A %Z'` via the Bash tool. Any
equivalent works if `date` isn't available (`fastfetch`, a one-line Python
`datetime.now()` / Rust `chrono::Local::now()` script).

**Active working hours** (local time to the repository owner, Asia/Singapore):

| Day | Hours |
|---|---|
| Monday – Friday | 07:30 – 20:00 |
| Sunday | 12:00 – 19:00 |
| Saturday | none — full rest day |

**Outside these hours, with the guardrail enabled:**

- Do **not** answer substantive questions or add context, analysis, or
  explanation beyond the minimum needed to log something for later.
- Do **not** agentically write code, run test suites, or open-endedly work a
  task.
- Ideas, plans, or scaffolding that come up may be recorded — as a `bn` issue
  (see "Issue tracking & roadmap" below) or a short markdown note — and
  nothing more.
- **Exception, still allowed outside hours:** compiling / running the
  existing test suite to confirm already-finished work is good, and pushing
  already-finished work to GitHub. Nothing beyond finishing and shipping
  work that already exists.

**While enabled, the hour limits do not bend in the moment.** Opting in is a
decision made at the start of a session; asking for a one-off exception at
23:00 is not. If the user asks to work past the limit *within an enabled
session*, say so plainly, log the request in beads for the next active window,
and stop there — do not negotiate or justify. Turning the guardrail off
outright is always the user's call and is honoured immediately (above); what
this clause blocks is piecemeal erosion while it is on.

### Why it exists

It protects the human maintainer's rest. Instituted 2026-07-11 after a month of
illness from overwork; the supporting analysis is in
[`DEVELOPER_HEALTH_WARNING.md`](./DEVELOPER_HEALTH_WARNING.md). Making it
opt-in does not retract that finding — it moves the decision to the human each
session rather than having the assistant enforce it unilaterally.

**To restore it as an always-on rule**, change the default in this section back
to On and drop the "Asking" subsection. That is a maintainer decision.

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
- **Exception — the kopi-beans store ref is pushed automatically.** Per explicit
  maintainer instruction on 2026-08-11, `refs/heads/beads/store` is published
  without asking, by a **`Stop` hook** in `.claude/settings.json` that runs
  **`./scripts/push-beads-store.sh`** at the end of each turn. Beads filed in a
  session are otherwise stranded on one machine, which defeats a
  distributed tracker.
  - The script pushes **exactly one refspec** —
    `refs/heads/beads/store:refs/heads/beads/store` — and nothing else. It is a
    no-op when the local ref is absent or already matches the remote, and it
    warns rather than failing the session if the remote is unreachable.
  - **This carve-out does not widen.** It authorises publishing the beads store
    ref only. It does **not** authorise pushing branches or tags, and **never**
    `main` — that still requires the maintainer to ask in so many words. If you
    find yourself adding a second refspec to that script, stop and ask.
  - You may still run the push by hand at any time; it is idempotent.
  - **The hook is now redundant, and it stays anyway.** As of kopi-beans 0.1.3
    the daemon publishes the store ref by itself (verified 2026-08-12 —
    `docs/kopitiam-issues/resolved/kopi-beans-cannot-push-store-ref.md`), so the
    script is no longer load-bearing. It remains harmless and is a correct
    fallback when the daemon is not running. **Do not remove the `Stop` hook
    from `.claude/settings.json`, do not delete
    `scripts/push-beads-store.sh`, and do not drop this carve-out.**
    Retiring the carve-out is a **maintainer policy decision that has not been
    made** — an agent noticing the redundancy is not authorisation to act on it.
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

## Search the workspace before building anything (HARD RULE)

**Before attempting a solution — and before briefing an agent on one — scan this
workspace for existing code that solves the problem, or comes close to solving
it. Always reuse.** Writing something this workspace already contains wastes
time and tokens, and worse, creates a second implementation that silently drifts
from the first.

This is a **hard rule, not a preference**, and it applies to *specifying* work as
much as to writing it. A brief that names an approach without first checking what
exists is the same defect one level up: the agent follows it competently and
produces a duplicate.

**Why it needs to be a rule.** This workspace is 40+ crates, many of them ports
of mature codes. The prior is **"this probably exists already"**, not "this needs
writing". On 2026-08-12 alone, five separate pieces of work were specified before
checking, and every one turned out to be already present:

| Specified | Already existed |
|---|---|
| Packed-bed friction + effective conductivity as "over-scoped" | `src/htr10/kta.rs`, `src/htr10/zbs.rs` — tested |
| A hand-rolled implicit heat-exchanger matrix solver | `TampinesSteamArray` / `OPCPFluidArray` on `Arc<FvMesh>` with PIMPLE correctors |
| "Build it directly on `outram-foam-basic-lib`'s `fvm::` operators" | Those arrays already wrap exactly that |
| An `inletOutlet` boundary condition, written from scratch | `tuas_boussinesq_solver`'s `advection_to_bcs.rs` upwind terminal |
| A limiter for bounded scalar convection | `fvc::Limiter` (Upwind/Linear/VanLeer/Minmod), vendored and tested |

A whole subsystem — `crates/tampines/src/pebble_bed/`, 5,656 lines with 34
passing tests — was found only by a documentation audit, having had no consumer
and gone unnoticed while related work was being written elsewhere.

**How to comply, concretely.** Before writing or briefing:

1. **Grep for the domain noun, not your intended API name.** `grep -rn "pub
   struct .*Array" crates/`, `grep -rni "laplacian\|upwind\|limiter" crates/
   --include=*.rs`. You are looking for someone else's vocabulary, not your own.
2. **Read the relevant crate's `CLAUDE.md` and `docs/`.** Several crates document
   capabilities that are not obvious from their names, and several document
   *deliberate* omissions you would otherwise "fix" wrongly.
3. **Check `src/` of the crate you are editing, not only the `examples/` you are
   working in.** The KTA/ZBS duplication happened exactly this way.
4. **Search sibling crates for the same lineage.** Ports often exist in pairs
   (`tampines-steam-tables` ⟷ `outram-park-fork-coolprop`); a defect or a feature
   in one usually has a counterpart in the other.
5. **State in the brief what you checked**, so the agent can correct you. "I
   searched X and Y and found nothing" is a claim someone can falsify; silence is
   not.

**Reuse in preference to porting, and porting in preference to writing.** If
direct reuse does not compose — different lineage, incompatible interface — port
the *logic* and **cite the reference implementation in the doc comment** so the
two cannot drift unnoticed. Only write something new when both fail, and say
plainly in your report why.

**A checked-and-rejected is a good answer.** "I looked at `tampines/src/gas_phase/`
and it does not cover this because …" is valuable and should be reported. What is
not acceptable is not looking.

Related: the recurring-failure-mode list in
[`docs/human-corrections-to-ai-work.md`](docs/human-corrections-to-ai-work.md).

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
  `cargo check --workspace --lib --tests` / `cargo test --release` before
  calling work done — do not let kopitiam's default profile substitute for it.
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
  guardrail above **when the session has opted into it** (with it on, do not
  run fleets outside active hours in the first place) and the
  never-auto-commit/push rule.

## Token accounting on every commit (opt-in, HARD RULE as stated below)

**Changed 2026-08-17 at the maintainer's request: token accounting is now
opt-in, not mandatory.** The previous rule — "every commit must carry an
API-token-usage trailer" — is retired. It is replaced by the policy below,
which *is* the hard rule from now on: do not enforce or chase the old
mandate, and do not treat a missing trailer as something to fix.

- **`crates/kovan-metrics`**, driven through the **`kovan`** binary, remains
  the single source for token accounting **when a maintainer has chosen to set
  it up** on a given clone — it does both the write side (git hooks stamping
  commit trailers) and the query side (`kovan tokens query --from DDMMYY --to
  DDMMYY [--branch develop] [--per-commit] [--json]`, summing whatever
  trailers exist). Nothing about its internals changed; what changed is
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
  any window with `kovan tokens query --from DDMMYY --to DDMMYY`).
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
`crates/kovan-metrics` (via the `kovan` binary); the reports live under
**`docs/historian/`** at the workspace root.

- **Generate it:**
  `kovan historian --from DDMMYY --to DDMMYY`
  (`DDMMYY` = day-month-year, 2-digit year). With no `--from`, it defaults to
  "everything on `develop` not yet on `main`, up to today". Output is written to
  `docs/historian/historian_<from>_to_<to>.md`.
  - **This replaced `docs/historian/historian.py` on 2026-08-13** (epic
    `op-yz7b`), and the Python was deleted the same day. No parity gate was run
    against it; see the token-accounting section above.
  - `kovan historian` dates from **UTC**, where the Python used local time.
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

## Issue tracking & roadmap — kopi-beans (mandatory)

This workspace tracks issues and per-crate roadmap progress with
**kopi-beans** (`bn`). It is a dependency-aware issue tracker whose canonical
data lives **in git refs** — `refs/heads/beads/store` (holding `state.jsonl`,
`deps.jsonl`, `tombstones.jsonl`, `meta.json`), with backups under
`refs/beads/backup/*`. `.beads/issues.jsonl` is a local compat-export
**symlink**, not the source of truth.

> **Migrated 2026-08-07** from beads-rs (`bd`) to kopi-beans (`bn`), per
> explicit maintainer instruction. **The migration is complete and the store
> is live**; `bd` is uninstalled. A pre-migration snapshot is preserved at
> **`refs/beads/premigration-v1-20260807`** — **do not delete it.** Full
> migration evidence (the format_version fix, issue counts at the time) is in
> `docs/kopitiam-issues/resolved/kopi-beans-store-format-version-1.md`; run
> `bn status` for the current issue count rather than trusting a number
> written here.

- **KOPI-BEANS ONLY.** Do **not** install or use `beads-rs` (binary `bd`) in
  this workspace — `bn` is the mandated tool and `bd` is gone.
- **Install:** `cargo install kopi-beans` (binary `bn`; check the current
  version with `bn --version` rather than a number written here). The store in
  this repo is already initialised — do **not** run `bn init` here.
  `.claude/settings.json` carries a hand-written, verified hook (`bn prime
  --mcp`) and is maintained **by hand** — never let a generator rewrite it.
  `bn setup claude --project` is verified safe (see "Dogfood KOPITIAM and
  KOPI-BEANS" above); the unflagged/global form is not.
- **Standing rule: you MUST use `bn`** for all task/roadmap tracking and
  progress bookkeeping — in preference to TodoWrite / TaskCreate / ad-hoc
  markdown TODO lists. Create/close/update issues as work happens; file one for
  any follow-up you discover.
- **Syncing is automated.** See "Workflow rules" above (the `Stop`-hook
  carve-out) for the mechanics and exact scope — not repeated here. The daemon
  publishes `refs/heads/beads/store` on its own; the manual push
  (`git push origin refs/heads/beads/store:refs/heads/beads/store`) remains a
  valid, idempotent fallback for when the daemon isn't running.
- **If `bn` is genuinely unavailable** — an OS/environment with no `bn` build
  (a locked-down sandbox, or Android before a Termux build is confirmed) — fall
  back to the harness task tools (TaskCreate / TodoWrite) and note in your
  hand-off that the tracker wasn't updated and why. This is now an
  environment-specific exception, not the normal case.
- **Roadmap / progress summaries come from kopi-beans.** When the user asks
  "where are we" / "summarise progress" / "what's the roadmap", read it out of
  `bn` (`bn list`, `bn ready`, `bn show <id>`, `bn status`, `bn dep`) rather
  than re-deriving from scattered docs. One epic per member crate; child issues
  are that crate's workstreams.
- **Relationship to the memory system.** The tracker and the per-project
  memory files (`~/.claude/projects/<slug>/memory/`) are complementary and
  **both stay in use**: the tracker holds *tasks / roadmap / open work*; the
  memory files track *durable facts, user preferences, and feedback*. When in
  doubt: a thing to *do or finish* → an issue; a thing to *remember about how
  the user works or a settled fact* → memory.
- **After a plan is approved (exiting plan mode), convert it into issues
  before writing any code.** One epic per new
  crate the plan introduces (or a child under the relevant crate's existing
  epic, for plans scoped to one crate); one child issue per part/module/
  deliverable the plan names, with `bn dep add` wiring the real ordering
  constraints between them. Do this even if the plan is also saved as a
  markdown file — the markdown is for human reading, `bn` is what `bn ready`/
  `bn show` make queryable across a session boundary. This is a standing rule,
  not a one-off.

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
   doc comments changed, so `docs/<crate>-api.md` stays in sync with the code:

   ```bash
   kovan api-docs <crate-dir-name>                    # e.g. outram-foam-basic-lib
   ```

   This runs `cargo +nightly doc --no-deps` → rustdoc JSON → the `rustdoc-md`
   binary → `crates/<crate>/docs/<crate>-api.md`. Both prerequisites are **mandatory —
   install them, do not skip the mirror** (see "API-doc toolchain" below).
   `docs/` is `exclude`d from the packaged crate, so this mirror is repo-only
   and never
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

## API-doc toolchain: install it, never route around it (HARD RULE)

**`rustdoc-md` and a nightly Rust toolchain are required tooling in this
workspace, not optional extras. If either is missing, INSTALL IT.**

```bash
rustup toolchain install nightly          # rustdoc's JSON output is nightly-only
cargo install rustdoc-md --locked         # rustdoc JSON -> markdown
```

Two things depend on them, and both are load-bearing:

- **`kovan api-docs <crate>`** — regenerates `crates/<crate>/docs/<crate>-api.md`, the
  committed markdown mirror of a crate's public API and the third leg of the
  per-crate `docs/` convention. Step 1 of the bookkeeping pass runs it. (It
  replaced `scripts/gen_api_docs.py`, retired 2026-08-14, so the doc toolchain
  needs no Python interpreter — same reasoning as epic `op-yz7b`.)
- **`kovan agent-docs-gen --regenerate-missing`** — generates a mirror for a
  crate that has none, so it can be bundled for an external agent.

**Never report a mirror as un-regenerable because a tool is missing.** Installing
`rustdoc-md` takes one command; skipping the mirror leaves `docs/<crate>-api.md`
silently contradicting the code, which is exactly the drift the bookkeeping pass
exists to prevent. "The toolchain isn't installed" is a task, not a finding.

### Check before you claim a tool is absent

**Run the check. Do not infer it from a failure, and do not assume.**

```bash
which rustdoc-md && cargo install --list | grep rustdoc-md
rustup toolchain list
```

This is a rule because an agent once shipped a bead/commit/hand-off claiming
`rustdoc-md`/nightly were missing on a host where both were installed —
running the check instead immediately exposed a real bug in
`--regenerate-missing` that the false "tool missing" assumption had been
hiding. The general form: an untested code path plus an assumed-missing
prerequisite produces a confident, false statement about both. Check the
prerequisite, then run the path.

## No Python for documentation or accounting — build it into `kovan` (HARD RULE)

**Documentation generation and repository accounting are `kovan`'s job. Do not
write, restore, or reach for a Python script to do either. If `kovan` cannot do
it yet, extend `kovan`.**

This is settled direction, not a preference, and it has been applied three times:

| Retired | Replaced by | When |
|---|---|---|
| `docs/historian/historian.py` | `kovan historian` (`kovan-metrics`) | 2026-08-13, epic `op-yz7b` |
| `docs/historian/token_usage.py` | `kovan tokens` (`kovan-metrics`) | 2026-08-13, epic `op-yz7b` |
| `scripts/gen_api_docs.py` | `kovan api-docs` (`kovan`) | 2026-08-14, `op-w44a.7` |
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
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library (real-time simulators) | GPL-3.0 (relicensed from Apache-2.0 on 2026-08-11; published versions <= 0.1.1 stay Apache-2.0 — see crate `NOTICE`) |
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
| `bedok` | Systems-level multiphysics coupling — 3-D nodal-diffusion neutronics coupled to channel TH, at the fidelity band **above 1-D neutronics and below CFD**. Rust translation of a MATLAB implementation by Than Yan Ren (SNRSI). **Owned by Than Yan Ren as of 2026-08-17** — he develops on the `bedok-yan-ren` branch; the maintainer merges it into `develop` periodically rather than merging directly himself. The 2026-08-17 merge replaced the crate's prior `src/reference/` module architecture with a flat one-module-per-`.m`-file layout and removed the earlier committed NEACRP/IAEA-3D benchmark cases and their `#[ignore]`d gates along with it — no parity claim is made pending rebuilt coverage, tracked in bead `op-swhd`. | GPL-3.0 |
| `outram-park-digital-twin-engine` | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only; formerly `outram-park-digital-twin-gui`) | GPL-3.0 |
| `kovan-common` | **KOVAN** knowledge layer — shared canonical types (`KovanDocument`, `KovanSymbol`, …). The Rust struct is the source of truth. | GPL-3.0 |
| `kovan-discovery` | KOVAN file discovery + text search — the `fd` (`ignore`) walker and ripgrep (`grep-*`) engine. Offline, deterministic. | GPL-3.0 |
| `kovan-literature` | KOVAN literature archive — PDF → Markdown (`pulldown-cmark`) → `KovanDocument` → BibTeX. `open/` committable, `proprietary/` gitignored. | GPL-3.0 |
| `kovan-semantics` | KOVAN repo-understanding — ripgrep-first, escalating to language servers (rust-analyzer / clangd / Pyright / fortls). Does not reimplement compilers. | GPL-3.0 |
| `kovan-codegen` | KOVAN deterministic code generation — templates for known numerical methods (root finders, linear/nonlinear/ODE solvers). Not an AI assistant. | GPL-3.0 |
| `kovan-metrics` | KOVAN repository accounting — per-commit API-token trailers (read from the Claude Code session transcripts) and the pre-merge historian report. Replaced `docs/historian/*.py` on 2026-08-13 so the toolchain needs no Python. | GPL-3.0 |
| `kovan` (bins `kovan`, `kovan-cli`, `kovan-tui`) | KOVAN's three front ends over the knowledge layer, per GitHub issue #30's final interface spec (2026-08-21): `kovan` is the **human-facing GUI** (egui, the graph digitiser window); `kovan-cli` is the **agent-facing** CLI (`clap`, line-oriented output for Claude Code and other coding agents, incl. `digitise`); `kovan-tui` is the **human-facing** TUI (`ratatui`; genuinely Android/Termux-usable, not just buildable — its Android module gate was lifted the same day). Consolidated 2026-08-21 from the former separate `kovan-cli`/`kovan-tui` crates, then restructured from five binaries down to these three later the same day. **Relicensed to AGPL-3.0-only 2026-08-21** — the one crate in this workspace that differs from the default, so it can depend on `kopitiam-pdf` (also AGPL-3.0-only, GitHub issue #30's PDF-reader work). See `crates/kovan/NOTICE` and `crates/kovan/DECISIONS.md`. | **AGPL-3.0** (workspace exception — see NOTICE) |
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
| `raffles` | **RAFFLES** (Risk Analysis Framework For Learning & Ensemble Simulation) — independent pure-Rust port of the UQ / risk-analysis core of **RAVEN** (Apache-2.0, Idaho National Laboratory): distributions, samplers, Sobol/correlation sensitivity, surrogates. **Owned by Adolphus Lye.** Apache-2.0 into GPL-3.0 is **one-way** — code cannot flow back to RAVEN (see the crate `NOTICE`). Scaffold only, nothing implemented, no human V&V. Independent fork, not affiliated with RAVEN/INL. Scoping: `docs/raven-port-scoping.md`. | GPL-3.0 |

> **KOVAN** is the deterministic *knowledge* layer (literature + semantics +
> codegen), interfaced three ways, all binaries of the single `kovan` crate:
> the `kovan-cli` **CLI** for agents, the `kovan-tui` **TUI** for humans, and
> the `kovan` **GUI** (the graph digitiser window) for humans. Offline /
> Android-first, no cloud, no Tree-sitter/SQLite/vector-store. Full design
> spec: **`docs/kovan.md`** (+ `docs/kovan-architecture.md`). Non-GUI kovan
> crates build for Android; `kovan-cli` and `kovan-tui` (including its
> Digitiser tab) are genuinely Android/Termux-usable — `ratatui` is an
> unconditional dependency, not target-gated off Android (see
> `crates/kovan/README.md` "Android"); only the `kovan` GUI's egui/eframe
> stack is Android-hostile and stays behind the `gui` feature, target-gated
> off Android on top of that.

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
| **GenFOAM** (deterministic + TH) | *ported inside* `outram-foam-appbuilder-lib` | Deterministic neutronics + thermal hydraulics. **No longer on hold — substantially ported**: `src/genfoam/` is ~32k lines / ~262 tests as of 2026-08-07 (AI-assisted draft, no human V&V). |

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
exception is `ndarray-linalg`, whose BLAS backend feature is chosen per-target
(`openblas-system` on unix, `intel-mkl-static` on windows/macos) — as of
2026-08-07 exactly one member still declares it, `outram-foam-basic-lib`, and
only as a target-gated **dev-dependency** for `tests/matrix_bench.rs`. TUAS's
`ndarray-linalg` removal is **done** (TUAS v0.1.2/0.1.3), not planned.

A second exception: **`kopitiam-pdf`** (AGPL-3.0-only, GitHub issue #30's
PDF-reader work) is declared only in `crates/kovan/Cargo.toml`'s own
`[dependencies]`, never in the workspace table — `kovan` is the sole
AGPL-3.0-only crate in this workspace, and keeping the dependency
crate-local is what stops another crate from picking it up (and the
AGPL question that comes with it) by accident. See `crates/kovan/NOTICE`.

See `docs/workspace-maintenance.md` for the rationale and history.

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
  build, so the lib still builds headless for Android. Concretely: the `kovan`
  crate's `kovan-cli` (CLI) and `kovan-tui` (`ratatui` TUI — genuinely
  Android/Termux-*usable*, not merely buildable; no Android stub as of
  2026-08-21, see `crates/kovan/README.md` "Android") binaries are **in
  scope and verified building and running** for `aarch64-linux-android` (its
  `kovan` binary — the GUI, renamed from `kovan-gui` the same day — is the
  crate's own GUI exemption, gated behind a `gui` feature that defaults on
  everywhere except Android, so it never affects the other two's Android
  build); only `outram-park-digital-twin-engine` (egui/eframe) is a genuine
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

## File path length: 170-character hard cap (HARD RULE, added 2026-08-18)

**No new file in this workspace may have a repo-relative path longer than 170
characters.** Windows' classic `MAX_PATH` is 260 characters and includes the
drive letter and every parent directory down to the repo root, not just the
part shown by `git ls-files` — a clone under a realistic path like
`C:\Users\<name>\Documents\GitHub\outram-park-backend\` already spends
45-70+ characters before the repo-relative part even starts. Long-path
support (`git config --global core.longpaths true` plus Windows'
`LongPathsEnabled`) fixes this for a user who knows to enable it, but this
workspace targets contributors who may not, so the rule is to not need it.

- **Check before adding a deeply nested file or directory**:
  `git ls-files | awk '{print length, $0}' | sort -rn | head` from the repo
  root, or scope it to one path with `git ls-files -- '<prefix>/**' | awk
  '{print length}' | sort -rn | head`.
- **170, not 260**, deliberately leaves headroom for a real clone-path prefix
  on top of the repo-relative path, and for the file to still have room to
  grow (a rename, an added test suffix) without immediately re-crossing the
  line.
- **This governs new files going forward.** It does not retroactively demand
  renaming everything already over the limit — see the history below.
- **Existing violations are grandfathered, not silently ignored** when found.
  A backlog of 53 files across `tampines-steam-tables` and
  `tuas_boussinesq_solver` was cleared on 2026-08-19 — see "Path-length
  refactor precedent" below. Re-run the scan (`git ls-files | awk '{print
  length, $0}' | sort -rn | awk '$1>=170'`) rather than trusting a count
  written here, since new violations can accumulate again over time.

### Path-length refactor precedent — this is a well-trodden, low-risk operation

**An agent session did this exact refactor workspace-wide on 2026-08-19** (the
170-char backlog above) and it is safe to ask for again: shortening directory
and file names to fix path-length violations, keeping the whole workspace
compiling (including test binaries) throughout, is a routine that has already
been proven out here, not a novel or risky undertaking. Concretely, that pass:

- Renamed the offending directories/files in `tampines-steam-tables` (2 files,
  one private nested test-module directory, no public API involved) and
  `tuas_boussinesq_solver` (51 files, 17 directory renames + 8 file renames).
- **Checked every renamed identifier for public-API exposure first** (a
  workspace-wide grep for real `use`/`pub use` references, not just a
  directory listing) before touching anything — four `tuas_boussinesq_solver`
  identifiers turned out to be genuinely public, cross-crate API: old names
  `ciet_steady_state_natural_circulation_test_components`,
  `array_control_vol_and_fluid_component_collections`,
  `one_d_fluid_array_with_lateral_coupling`,
  `one_d_solid_array_with_lateral_coupling`, renamed to
  `ciet_nat_circ_tests`, `array_fluid_collections`,
  `fluid_array_lateral_coupling`, `solid_array_lateral_coupling`.
- **First landed with a `pub use new_name as old_name;` compatibility
  re-export** at each original declaration site, so the first push broke
  nothing downstream without touching any consumer. **Immediately followed,
  same day, by migrating every downstream reference** (`tampines`,
  `tampines-steam-tables`, `teh-o-prke`, and
  `outram-park-digital-twin-engine`'s CIET v2 binary/examples — 178 files
  workspace-wide) to the new short names directly, then deleting the four
  aliases once a workspace-wide grep for each old identifier came back empty.
  The alias was a same-session bridge, not a standing policy — the maintainer
  wanted the short names used everywhere, not kept behind a compatibility
  shim. If a future pass repeats this on a crate with real crates.io
  consumers outside this workspace, keeping the alias for at least one
  published version is worth raising with the maintainer explicitly, since
  those consumers cannot be grepped for and fixed in the same session.
- Verified with `cargo check --workspace --lib --tests` and
  `cargo test --workspace --no-run --release` (all test binaries, not just
  `--lib`) after each stage, all clean.
- **kopitiam's `rename` command could not be used** (a real bug: rust-analyzer
  rejected every rename request with "Client does not support rename
  capability", LSP error -32803 — filed as
  [kopitiam#30](https://github.com/theodoreOnzGit/kopitiam/issues/30)). The
  fallback this file's own "Workflow rules" section already prescribes for
  when the LSP rename tooling is unavailable — enumerate every reference with
  a text search, apply the rename by hand, and let the compiler (`cargo
  check`) be the reference-checker that catches anything missed — carried the
  whole refactor without incident.

## Build & test

A system BLAS is **only** needed to run `outram-foam-basic-lib`'s
`matrix_bench` test (its sole remaining `ndarray-linalg` dev-dependency) — no
library in the workspace needs it. If you want that bench:

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

### TUAS natural-circulation tests are VERY long running — run them in parallel

**HARD RULE.** The CIET coupled-DRACS natural-circulation regression tests and
simulations in `crates/tuas_boussinesq_solver` (under
`pre_built_components/ciet_nat_circ_tests/`,
including `coupled_dracs_loop_tests/` and the
`para_heat_loss_regr_tests/`) take a **very** long time. They
integrate a coupled loop at a 0.1 s timestep for 2000–2500 s of simulated time
to reach steady state — see the crate `CLAUDE.md` "Testing Notes".

**They must be run in parallel, not serially.** Let cargo's test harness use
all cores rather than forcing a single thread:

```bash
# GOOD — the harness parallelises across tests by default
cargo test --release -p tuas_boussinesq_solver

# BAD — serialises every case; these tests are far too slow for this
cargo test --release -p tuas_boussinesq_solver -- --test-threads=1
```

So: **do not add `--test-threads=1`**, and do not wrap them in anything that
serialises execution. If a specific case needs isolation, isolate that case
rather than the whole suite.

Practical consequences for an agent or a CI step:
- **Budget real wall-clock time.** A default 120 s command timeout will kill
  them mid-run; give them a generous timeout or run them in the background.
- **Run the targeted subset** while iterating (`cargo test --release -p
  tuas_boussinesq_solver <substring>`) and the full suite only when finishing.
- **A timeout is not a failure.** Do not report a killed run as a failing test,
  and never loosen a tolerance because a long test was inconvenient.

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
mandatory rule (responsible-use / data policy, V&V docs, Rust design rules,
never-auto-commit/push — and the working-hours guardrail whenever the session
has opted into it) is relaxed — correctness and honesty come first. **When in Singlish mode, read `SINGLISH_MODE.md` and apply its logged
corrections.** Default is standard English; opt-in only.


<!-- BEGIN ISSUE TRACKER INTEGRATION (hand-maintained for kopi-beans since 2026-08-07; formerly BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61) -->
## Issue Tracker (kopi-beans)

This project uses **kopi-beans** (`bn`) for issue tracking — it replaced
beads-rs (`bd`) on 2026-08-07 and `bd` is now uninstalled. Run `bn prime` to
see workflow context. The store is **live and readable** (kopi-beans **0.1.3**
installed as of 2026-08-12, format_version 2); `bn status`, `bn list`,
`bn ready` all work.

### Quick Reference

```bash
bn ready              # Find available work
bn show <id>          # View issue details
bn claim <id>         # Claim work
bn close <id>         # Complete work
```

### Rules

- Use `bn` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists. The only exception is an environment with no working `bn` build at all, which must be stated in the hand-off.
- Run `bn prime` for workflow context.
- **Sync is automated.** See "Workflow rules" above for the `Stop`-hook mechanics and scope (not repeated here).
- Persistent durable facts / user preferences: keep using the per-project
  `memory/` + `MEMORY.md` workflow (see the "Issue tracking & roadmap" section
  above — this workspace keeps MEMORY.md; it is **not** dropped).

**Architecture in one line (kopi-beans):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`, plus a preserved pre-migration snapshot at `refs/beads/premigration-v1-20260807` that must not be deleted. The daemon debounces local writes and publishes them to the git remote itself; the manual push is a fallback for when the daemon is not running. `.beads/issues.jsonl` is a local compat-export symlink.

## Agent Context Profiles

The managed tracker block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bn` for task tracking — its store is live in this repo. Do not run git commits, git pushes, or a manual sync unless explicitly asked; that includes `git push origin refs/heads/beads/store:refs/heads/beads/store`, which on kopi-beans 0.1.3 is normally unnecessary anyway — the daemon publishes the ref itself (kopitiam#19 resolved, verified 2026-08-12). At handoff, report changed files, validation, suggested next commands, and whether the store ref still needs pushing.
- **Minimal**: Keep tool instruction files as pointers to `bn prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close issues, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a kopi-beans-tracked implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create tracker issues (or, while the blocker above is open, harness tasks) for anything that needs follow-up
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
- Explicit user or orchestrator instructions override this tracker block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END ISSUE TRACKER INTEGRATION -->
