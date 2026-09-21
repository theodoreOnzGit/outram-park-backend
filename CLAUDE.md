# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Scope boundary: this file governs `outram-park-backend` ONLY — never a parent folder (HARD RULE)

**This file, and every crate-level `CLAUDE.md` under `crates/*/`, apply
exclusively to the `outram-park-backend` repository tree** — this directory and
everything under it, nothing else. This repo is sometimes checked out *inside*
another person's project — as a git submodule, a subtree, or a plain nested
clone. When that is the case, **none** of these rules apply to the **parent**
project, its other submodules, or its top-level files. Ever. This binds
regardless of which directory the session's working directory happens to be in.

**How to tell whether a given file or command is in scope.** Resolve its
repository root and compare it against *this* repo's own root:

```bash
git -C <path-in-question> rev-parse --show-toplevel
```

If that root is `outram-park-backend` itself (or, for a submodule mount, the
path where this repo is checked out), the rules here apply. If it resolves to a
**different** root, **this file has no authority there**, full stop, no matter
how the session got there or what was discussed earlier in the conversation.

**Concretely, when a parent project merely contains this repo as a
submodule/subrepo:**

- Only apply this file's rules to work actually inside this repo's own working
  tree. A commit, build, dependency change, or doc edit anywhere else in the
  parent project is governed by *that* project's conventions — not by anything
  written here.
- Do not assume the parent project uses `kovan`, GitHub issues, this repo's git
  hooks, or any other tool mandated here. Do not install or run them against
  the parent's own tree on the strength of this file.
- If a task spans both, this file governs only the parts touching this repo's
  own tree. If it's unclear which rule set a change falls under, **ask rather
  than guessing outward.**
- This applies even mid-session: if the working directory moves out into the
  parent (or a sibling), re-check scope before continuing. Nothing here
  "sticks" once you've left this repo's tree.

**Why this exists.** `CLAUDE.md` guidance is easy to over-apply once read into
a session — the failure mode is carrying a rule outward onto unrelated code
that merely happens to live in the same checkout. The boundary is this
repository's own root, determined by its own `.git`, not the process's current
working directory and not how far back the rule was stated.

## Working-hours guardrail (OPT-IN — off unless the user turns it on)

**This guardrail is OFF by default, and nothing about it is mandatory** —
there is no required question to ask at session start and no required time
check to run. It is not a standing rule. Changed 2026-08-13, and further
relaxed 2026-09-03, at the maintainer's request.

**Turning it on.** The guardrail applies only when the user turns it on in
plain words ("enable the working-hours guardrail", "enforce my hours this
session"). You are **not** required to ask about it — offer it only if the
user seems to want it, and **treat silence as Off, always.** The user may
switch it on or off at any point; honour that immediately, with no
confirmation question.

**If it is off (the default):** no time check, no hour restriction, no
rest-day rule. Work normally. Do not volunteer reminders about the
maintainer's hours or health, and do not re-litigate the setting.

**If the user has turned it on**, everything below applies for the rest of
that session, as a hard rule.

**Check the real local time and day of week** with a system tool before
substantive work — do not infer it from conversation content, a cached date,
or skip the check. Preferred: `date +'%Y-%m-%d %H:%M %A %Z'`.

**Active working hours** (local to the repository owner, Asia/Singapore):

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
- Ideas, plans, or scaffolding may be recorded — as a GitHub issue or a short
  markdown note — and nothing more.
- **Exception, still allowed:** compiling / running the existing test suite to
  confirm already-finished work is good, and pushing already-finished work to
  GitHub. Nothing beyond finishing and shipping work that already exists.

**While enabled, the hour limits do not bend in the moment.** Turning the
guardrail on is a deliberate decision; asking for a one-off exception at 23:00
is not. If the user asks to work past the limit *within an enabled session*,
say so plainly, log the request for the next active window, and stop there —
do not negotiate or justify. Turning the guardrail off outright is always the
user's call and is honoured immediately; what this clause blocks is piecemeal
erosion while it is on.

**Why it exists.** It protects the human maintainer's rest. Instituted
2026-07-11 after a month of illness from overwork; the supporting analysis is
in [`DEVELOPER_HEALTH_WARNING.md`](./DEVELOPER_HEALTH_WARNING.md). Making it
opt-in does not retract that finding — it moves the decision to the human.
**To restore it as an always-on rule**, edit this section accordingly. That is
a maintainer decision.

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
- **Never auto-bump versions** in `Cargo.toml` files. Only bump versions when
  explicitly requested.
- **Always build and test in release mode.** Use `--release` for all `cargo`
  invocations — see "EVERYTHING is release" under "Build & test". Never run
  tests or builds in debug mode.
- **Use rust-analyzer (the LSP tool) for all code-intelligence workflows.**
  For any symbol query — a definition, every reference/caller, type/hover
  info, or listing symbols in a file or across the workspace — reach for the
  rust-analyzer LSP tool first, **not** text search (`grep`). It resolves
  symbols semantically, so it does not confuse a module path with a like-named
  identifier the way a text match can.
  - **The LSP tool here is read-only** — `goToDefinition`, `findReferences`,
    `hover`, `documentSymbol`, `workspaceSymbol`, and call hierarchy. It does
    **not** expose rename / code-action / `applyEdit`.
  - For a refactor an editor would drive with *rename*, first use
    `findReferences` to enumerate the sites, then apply the edits yourself,
    and rely on the compiler as the reference checker — every missed reference
    is a hard error pointing at the exact line. Prefer this over a blind `sed`
    rename, which can silently mangle a colliding name.

## Get the PROCESS right first; the answer comes second (HARD RULE)

**Maintainer direction, 2026-09-18.** This is the governing rule of how work is
done in this workspace, and every other rule below is downstream of it.

**Never reason backwards from a target number.** Fix the inputs, the physics,
the instrument and the assumptions on their own merits — from literature, from
upstream, from first principles — and then report whatever answer that
produces. A right answer obtained by a wrong process is worth **less than
nothing**, because it looks like evidence while carrying none: it cannot fail,
so it cannot inform, and the next person inherits a number with no provenance.

### What this forbids

- **Tuning an input until a comparison passes.** The benchmark is the check;
  the moment it becomes an input, the check is gone.
- **Moving a threshold to make a test pass.** If a gate fails, the first
  hypothesis is that the *thing being gated* is wrong, not the gate.
- **Choosing the instrument after seeing the result.** Pick the measure the
  physics calls for, state why, and keep it even when it is unflattering.
- **Quietly repairing a number you already published.** Correct it in place,
  say what it was, say what changed it.

### What it requires

1. **Justify every input from outside the comparison.** A material property
   comes from the literature or from the upstream code, with a citation —
   never from what makes the answer come out right.
2. **Fix the protocol, not the criterion.** When a bound is breached, change
   the procedure until it is genuinely satisfied.
3. **Match the instrument to the physics**, and say why it is the right one.
4. **Test the assumption the result rests on**, especially when it is
   load-bearing and convenient. An assumption that has never been able to fail
   is not evidence.
5. **Report the disagreement when there is one.** An honestly-derived miss is
   worth more than a fitted hit.
6. **Say which numbers to quote** when several exist, and why the others do
   not count.

**Worked example.** The HTR-10 filling fraction (GitHub issue #216) was missed
by −8.7 % and closed to **−0.9 %** — and the process is why the result is
worth anything: `µ` was ablated across the *literature* graphite range rather
than tuned, all four cells reported including the two that miss by 3–6 %; a
breached quasi-static bound was fixed by lengthening the settle window 4×
rather than relaxing the threshold; a rate-independence control was run and
**revised two of my own numbers**, halving one and exposing another as an
artefact. And the rule cuts the other way: questioning whether a run was
reproducible — an assumption no test had ever challenged — exposed a P0 defect
in which the HTR-10 bed gave a *different answer every run*, because the
neighbour grid iterated a randomly-seeded `HashMap`.

> **The point is not that 0.61 was reached. It is that the run which reached it
> was capable of missing.**

## Search the workspace before building anything (HARD RULE)

**Before attempting a solution — and before briefing an agent on one — scan
this workspace for existing code that solves the problem, or comes close to
solving it. Always reuse.** Writing something this workspace already contains
wastes time and tokens, and worse, creates a second implementation that
silently drifts from the first.

This is a **hard rule, not a preference**, and it applies to *specifying* work
as much as to writing it. A brief that names an approach without first
checking what exists is the same defect one level up: the agent follows it
competently and produces a duplicate.

**Why it needs to be a rule.** This workspace is 43 crates, many of them ports
of mature codes. The prior is **"this probably exists already"**, not "this
needs writing". On 2026-08-12 alone, five separate pieces of work were
specified before checking, and **every one turned out to be already present** —
packed-bed friction and effective conductivity, an implicit heat-exchanger
solver, the FV operators a brief proposed building on, an `inletOutlet`
boundary condition, and a bounded-scalar limiter. A whole subsystem
(`crates/tampines/src/pebble_bed/`, 5,656 lines, 34 passing tests) was found
only by a documentation audit, having had no consumer and gone unnoticed while
related work was being written elsewhere.

**How to comply, concretely.** Before writing or briefing:

1. **Grep for the domain noun, not your intended API name.** You are looking
   for someone else's vocabulary, not your own.
2. **Read the relevant crate's `CLAUDE.md` and `docs/`.** Several crates
   document capabilities that are not obvious from their names, and several
   document *deliberate* omissions you would otherwise "fix" wrongly.
3. **Check `src/` of the crate you are editing, not only the `examples/` you
   are working in.**
4. **Search sibling crates for the same lineage.** Ports often exist in pairs;
   a defect or a feature in one usually has a counterpart in the other.
5. **State in the brief what you checked**, so the agent can correct you. "I
   searched X and Y and found nothing" is falsifiable; silence is not.

**Reuse in preference to porting, and porting in preference to writing.** If
direct reuse does not compose, port the *logic* and **cite the reference
implementation in the doc comment** so the two cannot drift unnoticed. Only
write something new when both fail, and say plainly in your report why.

**A checked-and-rejected is a good answer.** "I looked at X and it does not
cover this because …" is valuable and should be reported. What is not
acceptable is not looking.

Related: [`docs/human-corrections-to-ai-work.md`](docs/human-corrections-to-ai-work.md).

## A doc claim contradicted by the code is a DEFECT — fix it in the same change (HARD RULE)

**When you find a statement in any `docs/`, `CLAUDE.md`, `README.md` or `///`
doc comment that the code contradicts, correct it in the change where you
found it. Do not file it, do not "note it for later", and do not report it as
an observation while leaving it in place.** A stale doc is not a tidiness
problem; it is a false statement that the next reader — human or agent — will
act on.

**Why this is a hard rule and not a courtesy.** This workspace's docs are the
primary interface to 43 crates that no one can hold in their head, and the
search-before-building rule above *depends on them being true*. A stale
"missing" claim is the worst kind, because it causes exactly the duplication
this file exists to prevent: an agent reads "no model exists", believes it,
and writes a second one. Three were found in a single session on 2026-09-17 —
including one claiming `reference-data/endf/` held only a README when it holds
**39 ENDF/B-VIII.0 tapes**. That claim carried an explicit **re-verification
stamp** (*"re-verified 2026-08-12 — still true"*) and was still wrong. A dated
"still true" marker is evidence of when someone last looked, not that the
claim holds now — **re-check the claim, never trust the stamp.**

**How to comply:**

- **Correct it where it lives**, including the generated
  `docs/<crate>-api.md` mirror if the source doc comment changed
  (`kovan-cli api-docs <crate>`).
- **Strike through rather than delete** when the claim shaped a decision —
  `~~old claim~~ **CORRECTED <date>** — new position`. The history is why a
  reader can trust the correction; a silent edit looks like the doc was always
  right.
- **Say what you verified**, not just what is now true. "39 tapes present" is
  checkable; "this is fixed" is not.
- **A claim you cannot check is not a claim you may leave standing unmarked.**
  Mark it `Not re-checked` with the reason.
- **This binds for docs you did not write and were not asked to touch.**
  Finding it makes it yours. The one exception is the scope boundary at the
  top of this file: a doc whose repository root is not this one is out of
  scope entirely.

**This does not license rewriting docs you merely disagree with.** The trigger
is a claim the *code contradicts* — a falsifiable mismatch you have checked,
not a wording preference. Fixing style while claiming to fix staleness is how
a review pass becomes an unreviewable diff.

## Debugging a port: read upstream first (HARD RULE)

**When a ported module misbehaves, find out how the upstream code handles that
exact situation BEFORE proposing, writing, or testing a fix.** Most of this
workspace is a translation — NJOY2016, PFLOTRAN, CoolProp, GeN-Foam, OFFBEAT,
`code_aster`, DWSIM, GSL, LIGGGHTS, FLEXPART — and in a translation the
overwhelmingly likely cause of a discrepancy is that upstream does something
the port does not. **Upstream is the specification.** Reasoning about the
physics from first principles, or from what the port's own comments claim, is
not a substitute for reading it.

1. **Find the upstream routine** that owns the behaviour and read it — the
   Fortran/C++/VB source, not just the manual. Vendored sources live in the
   gitignored `vendor/` folders; the manuals are in `kovan`'s literature store.
2. **Ask what upstream does that we do not.** Bounds and guards are the usual
   answer: an upper energy limit, a card default, a branch on a format flag, a
   range check, a special case for a boundary. **A missing *limit* is a far
   more common port defect than a wrong *formula***, because formulas get
   reviewed line-by-line during translation and control flow does not.
3. **Check the data's own format flags before blaming the code.** ENDF-6 (and
   equivalents elsewhere) change the meaning of a section based on flags —
   `LSSF`, `LRU`/`LRF`, `LI`, `INT`. A "missing physics" hypothesis that
   ignores the flag will send you porting a module you did not need.
4. **Only then form a hypothesis, and state its predicted sign and magnitude
   before you measure.** If the fix would move the answer the wrong way, you
   have the wrong hypothesis — stop and go back to step 1.

**Record what you find in the issue**, including when upstream turns out to
handle it the same way we do — that result is worth as much as a defect, and
saves the next session repeating the search.

**Why this exists.** The U-238 ring-RPT discrepancy was recorded with "URR
self-shielding not reconstructed" as the leading hypothesis. Reading the
evaluation first would have shown `LSSF=1` in U-238's `LRU=2` range — MF=3
already carries the infinitely-dilute unresolved cross sections, so the
reconstruction is *not* low, and adding PURR self-shielding would have moved
the case **further** from the reference. Hours went into a first-principles
argument and two speculative patches that reading `broadr.f90` and the ENDF
flag would have pre-empted.

**A second-order consequence, worth knowing.** Because neither code broadens
above `thnmax`, a shared-grid comparison against NJOY at 293.6 K that does
*not* split at the limit measures only the **unbroadened** table — on all
three uranium nuclides, **zero** shared grid points fall below it. This port's
Doppler broadening is therefore **not verified against NJOY at all**, despite
a 293.6 K comparison that reads as ~1e-6 agreement. See
`crates/njoy-outram-park-fork/verification_and_validation/acer_ce_vs_njoy2016_multi_nuclide.md`.

> Full original text of all four sections, with the complete worked-example
> tables and the `thnmax` correction history:
> [`docs/claude-md-rationale/process-and-porting-lessons.md`](docs/claude-md-rationale/process-and-porting-lessons.md).

## Use KOVAN for repository context

`kovan` is this workspace's own deterministic knowledge layer — reach for it
first for repo understanding, symbol/code queries, and literature scoped to
this codebase. **Use `kovan-cli`**, the agent front end.

- **Token-frugal reading:** `kovan-cli cost <path>` (real BPE-approximation
  estimate), `kovan-cli outline <file>` (declarations skeleton),
  `kovan-cli slice <file> <start> <end>`. Prefer this
  `cost → outline → refs → slice` loop over reading whole large files.
- **Symbol queries:** `kovan-cli def|sig|refs <symbol> --file <file>`,
  rust-analyzer-backed (needs `rust-analyzer` on PATH). The *first* query for
  a workspace root pays a real indexing wait (up to `KOVAN_RA_TIMEOUT_SECS`,
  default 180 s); that root then stays warm in a background `lsp-daemon`, so
  every later call answers in well under a second. There is deliberately **no**
  idle timeout. Stop it explicitly with
  `kovan-cli lsp-daemon-stop --root <root>`.
- **Never run `kovan`, `kovan-tui` or `kovan-cli lsp-daemon-serve` directly**
  in a non-interactive session — the first two are GUI/TUI front ends and the
  third is the daemon's own foreground process; all three will hang.

Run `kovan-cli skill-gen` to (re)generate `kovan_skill.md` for an agent
session that hasn't read this file.

**KOPITIAM is no longer dogfooded here** (maintainer direction, 2026-09-21).
It may still be used where it helps, but it is not mandated and its rough
edges no longer need filing from this workspace. The former rule, and the
`docs/kopitiam-issues/` queue it fed, are preserved in
[`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](docs/claude-md-rationale/tooling-kopitiam-kovan.md).

### Literature handling (the kovan ingestion mandate was RETIRED 2026-09-21)

**~~ANY literature ingested OR USED goes into kovan (HARD RULE)~~ and ~~READ
AND WRITE THE LITERATURE LIBRARY THROUGH `kovan` (HARD RULE)~~ — RETIRED
2026-09-21 at the maintainer's request. Neither applies any more.** Literature
does **not** have to be routed into `crates/kovan-literature`, and reads of it
do not have to go through the `kovan lit` CLI. Do not enforce either rule, and
do not treat a paper that is not in the archive as a defect to fix. The full
original text of both rules is preserved in
[`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](docs/claude-md-rationale/tooling-kopitiam-kovan.md).

`kovan lit import` / `outline` / `bibtex` and `kopitiam pdf2md` all remain
available and are still perfectly good tools — using them is now a choice, not
an obligation. `crates/kovan-literature/` and its `CATALOGUE.md` stay where
they are; existing citations into the archive remain valid.

**What survives this retirement, because it never came from `kovan` in the
first place** — these are `DATA_POLICY.md` and compliance obligations and they
still bind wherever a document lives:

- **The open/proprietary split.** Public, openly published literature is
  committable; anything restricted is not and must stay out of the repository.
  **Decide the access tier from the document's own copyright page**, not from
  where it was downloaded — public hosting (INIS, gen-4.org, a lab's website)
  grants no redistribution rights. **Unsure means proprietary**; that failure
  direction is recoverable and the other is a licence violation in a public
  repository. The root-level `collaboration/` directory is gitignored scratch
  and is **not** automatically open — ask if provenance is not stated.
- **Text-and-data-mining / AI-training reservations.** Where a copyright line
  carries one, record metadata and factual findings only and do **not** extract
  the full text — facts are not copyrightable, but the corpus is what that
  clause reserves.
- **Provenance for anything the code depends on.** If a document informs the
  code — a correlation taken from it, a benchmark value cited, a number in a
  doc comment — record its source, author, title, licence/access terms,
  URL/DOI, date accessed and any processing steps, per the "Responsible use &
  data policy" section. A citation that points at nothing a reader can reach
  is a dead reference. Where to put that record is now your judgement: a
  `References.md` beside the example, the relevant validation report, or the
  kovan archive.
### Graph digitisation (the `kovan-cli digitise` mandate was RETIRED 2026-09-21)

**~~Graph digitisation: dogfood `kovan-cli digitise` (HARD RULE)~~ — RETIRED
2026-09-21 at the maintainer's request, alongside the kovan literature
mandate above. It no longer applies.** Getting data off a published figure
does not have to go through `kovan-cli digitise`, the `kovan-tui` Digitiser
tab or the `kovan` GUI. Do not enforce it. The full original text is preserved
in [`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](docs/claude-md-rationale/tooling-kopitiam-kovan.md).

The digitiser still exists in `crates/kovan/src/digitiser/` and still works —
using it is now a choice. Two of its properties are worth knowing if you do:
its accuracy is verified **against synthetic ground truth only** (never
against real published figures), and **only a human can mark a dataset
reviewed** — an agent session cannot, and a CLI run can only ever emit
`Unreviewed`.

**What survives, because it was never a `kovan` rule:** if a number in this
codebase came off a plot, **say so and say how**. Record the figure it came
from, the axis calibration or reference points used, the scale (linear or
log), and whether the reading was automatic or by eye. That is the
"Responsible use & data policy" provenance obligation and the
"Verification & validation documentation" rule, not a tooling preference — a
digitised value with no record of how it was read is not a citable number,
whatever produced it.
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

> Full opt-in policy, the historian's replacement history and the reasoning:
> [`docs/claude-md-rationale/accounting-and-no-python.md`](docs/claude-md-rationale/accounting-and-no-python.md).

## Issue tracking — GitHub issues (mandatory)

Track all tasks and roadmap progress in GitHub issues on
`theodoreOnzGit/outram-park-backend`, via `gh`, not TodoWrite or markdown
TODO lists. (`bn` / kopi-beans was deprecated 2026-09-21; see
[`docs/kopi-beans-deprecation.md`](docs/kopi-beans-deprecation.md).)

```bash
gh issue list --state open                       # what is open
gh issue view <number>                           # details + comments
gh issue create --title "..." --body "..."       # file one
gh issue comment <number> --body "..."           # progress
```

- **Do not close issues on your own initiative.** Propose the closure with
  its evidence; the maintainer decides.
- **Labels:** `bug`, `enhancement`, `epic`, `P0`…`P3`. One epic per member
  crate; state blocking relationships in the body in words ("blocked by #123").
- **After a plan is approved, convert it into issues before writing code** —
  one child issue per deliverable, under the relevant crate's epic.
- **Roadmap / progress questions** are answered from `gh issue list/view`.
- **If `gh` is unavailable**, fall back to the harness task tools and say so
  in the hand-off.
- **`op-*` ids are historical** references into the old beads store; do not
  look them up in `gh` and do not mint new ones. The store refs
  (`refs/heads/beads/store`, `refs/beads/*`) are preserved — do not delete them.
- The tracker holds *work to do*; the per-project `memory/` files hold
  *durable facts and preferences*. Both stay in use.

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

## egui simulators: headless mode lives in the crate docs

Every egui/eframe simulator must ship a deterministic `--headless` mode with a
regression test. The full rule is in
[`crates/outram-park-digital-twin-engine/CLAUDE.md`](crates/outram-park-digital-twin-engine/CLAUDE.md);
the other crates with egui simulator examples point to it.

## Crate maturity gate (HARD RULE)

**The dogfooding rule below is a hard rule for every crate the maintainer has
declared mature, and is not enforced on any crate before that.** Both halves
bind: do not skip it on a mature crate, and do not demand it of one still
finding its shape.

**API polish comes after the internals are shown to be reasonably accurate,
never before.** An interface is a commitment to a shape, and shaping an
interface around physics that is still moving means paying for the same
interface twice. Worse, an API that is pleasant to call and quietly wrong is
more dangerous than one that is awkward, because the ergonomics invite trust
the numbers have not earned.

**What "reasonably accurate" means here.** A crate becomes eligible when its
internals are backed by at least one of:

- **Analytical or manufactured solution** — a closed-form result, or an MMS
  convergence study showing the expected order of accuracy. This proves the
  numerics.
- **Cross-code comparison** — agreement with an established code (OpenMC,
  Serpent, MOOSE, OpenFOAM, NJOY) on the same input, where no closed form
  exists.
- **Unit tests and internal consistency** — conservation, symmetry,
  reciprocity and limiting-case invariants holding under test. Necessary
  always, and sufficient on its own only for crates with no physics to get
  wrong (utility, I/O, tooling).

**Published-benchmark agreement is deliberately not on that list.** Matching
HTR-10, MSRE or ICSBEP is the *goal* of the validation pipeline, and that
pipeline is driven through the very API in question — so requiring it first
would deadlock the work. Benchmark agreement is the reward for a good API, not
its entry fee. (Rationale recorded 2026-09-05; revisit if it proves too
lenient.)

**The tolerance is per-crate, and lives in that crate's own `CLAUDE.md`.**
There is no workspace-wide number: 500 pcm means something in `outram-mc-libs`
and nothing in `outram-park-fork-dwsim-libs`. Each crate states its own bar,
the reference it is measured against, and the evidence class above that it
claims. **A crate with no such statement is by definition not yet mature.**

**Who declares it.** The maintainer, and only the maintainer. An agent that
believes a crate has cleared its bar may *propose* maturity — open an issue,
cite the specific runs and numbers, and stop there. **Do not flip the flag**,
and do not begin enforcing the dogfooding rule on the strength of your own
assessment.

**The bar moves, and that is expected.** Record every revision as a dated
entry in the crate's own `CLAUDE.md`, **keeping the superseded ones** — an
agent reading the crate later needs to know not just today's bar but that it
moved, or it will misread older results as failures against a standard that
did not exist when they were produced.

**Twelve crates were declared mature as of 2026-09-15**, plus `petir`; the
Members table marks them. **The bar and its evidence live in each crate's own
`CLAUDE.md`, which is the authority — this file's roster is a pointer.**

**Read the crate's own file before citing any of them as validated.** Several
carry caveats that matter and that a roster row cannot hold: `teh-o-prke`
lacks analytical transient validation; `farrer-park` has a **known uncured
shear-locking defect** and no cross-code or benchmark leg at all;
`outram-park-fork-dwsim-libs` has met its bar **at the EOS layer only**, with
0 of 107 port-coverage rows at `PORTED + VALIDATED`; `petir`'s bar is
cross-code agreement **alone**, against no published benchmark; and
`outram-park-fork-liggghts` reproduces upstream LIGGGHTS essentially exactly
while having **no experimental comparison** of the granular physics at all.
Agreeing on a number is not the same as agreeing on the physics —
`outram-mc-libs` sits well inside its 500 pcm bar on Godiva while a ~69 pcm
*spectral* residual against OpenMC is still open.

> The full roster with each crate's bar and evidence class, the six honest
> notes in full, and the measured API-guessing table:
> [`docs/claude-md-rationale/maturity-and-api-dogfooding.md`](docs/claude-md-rationale/maturity-and-api-dogfooding.md).

### Verifying it: dogfood the API on a small model (HARD RULE)

> **If it is too complex for Haiku, it is a bad API.**

That is the standing test for **every crate declared mature** (see the gate
directly above — it is not asked of a crate still finding its shape), and it
is a test, not a slogan: a small model has no budget to read the source, so it
can only use what the interface itself makes discoverable — exactly like the
human this file already requires the API to serve. When it cannot get there,
the interface is wrong, however correct the physics underneath.

**Before claiming a public API is usable from Python, run it on a Haiku agent
with a fresh context and the wheel only — no repository access, no `.pyi`
pasted in, no source.** Count the round trips to a working script and log
every exception. That number is the usability measure; the exception log is
the fix list.

**WHY THIS EXISTS: capability masks usability defects.** A strong model is the
*worst* instrument for this, for the same reason the author of a tool is the
worst person to usability-test it — when the API is unguessable it reads the
Rust source and compensates, and the compensation is invisible in the result.
The finished script looks clean and the defect ships. Measured 2026-09-04, an
Opus session **with** full source access and grep still guessed wrong ten
times across six scripts (`mc.Sphere(center, radius)` for
`Sphere(x0, y0, z0, r, bc)`, `.k_eff` for `.k_mean`, `FvSolution()` which is
not constructible at all, and seven more — the table is in the rationale doc).
Every one cost a round trip. A model that *cannot* read the source has no way
to recover from any of them, which is the point.

**What the test requires:**

- **Fixed context.** Wheel installed, nothing else. Handing the agent the
  stubs or the crate source tests something other than the API.
- **A fixed task list** spanning the tiers — a one-call case, a case assembled
  from parts, and one exotic enough to need the low-level types.
- **Every exception logged, not just the count.** Ranked by frequency across
  tasks, that log *is* the backlog, in priority order.

**What it does NOT measure: physical correctness.** A script can run cleanly
and be nonsense. This never substitutes for the V&V tests — it tests whether
the API can be *found and called*, nothing more.

**Reading the result.** A failure the Opus session also hit is an API defect.
A failure unique to the small model is more likely its own priors — the tell
is whether a different model trips on the same call. Fix the first kind.

**The same principle applies one layer down, to Rust callers.** A confusing
trait-bound error is the compiler's version of an undiscoverable API.
`#[diagnostic::on_unimplemented]` (stable since 1.78) replaces "the trait
bound `X: Y` is not satisfied" with a message, a label and a note of your
choosing; `#[diagnostic::do_not_recommend]` (stable since 1.85) suppresses a
blanket impl the compiler would otherwise suggest unhelpfully. This workspace
has 65 public traits and, as of 2026-09-04, not one carries either attribute.
Any trait whose bound a caller can plausibly fail should.

A good message does not restate the bound. It names the concrete types that
*do* implement the trait, or the constructor to call: "implemented by
`PengRobinson`, `Srk`, `PengRobinson1978`" turns a search through the source
into a choice from a list.

**Take the baseline *before* changing anything** — the fixes this suggests
should be measured against it, not asserted.
## Model hierarchy: correct physics first, surrogates uncalibrated before calibrated (HARD RULE)

**Maintainer direction, 2026-09-17.** This governs *what kind of model* to
reach for, and in what order. It applies to every crate in this workspace.

### High-fidelity crates: the correct physical model is the top priority

**Implement the governing physics. Nothing outranks it** — not ergonomics, not
runtime, not a number that matches a reference. Where a high-fidelity crate
cannot yet carry the real model, say so plainly rather than substituting
something cheaper that reads as if it does.

### Correct physics is the DEFAULT SETTING, not an opt-in (HARD RULE)

**Maintainer direction, 2026-09-20. Binds every high-fidelity crate**, i.e.
every crate whose job is to represent physics rather than to tabulate,
orchestrate or visualise it — `outram-mc-libs`, `njoy-outram-park-fork`,
`outram-foam-*`, `farrer-park`, `tampines*`, `bedok`, `boon-lay`,
`outram-park-fork-*`, `changi`, `nee_soon`.

**Physics the evaluation, the correlation or the governing equation SUPPLIES
must be applied unless a caller explicitly ablates it.** A physics term behind
an off-by-default flag is, in practice, physics the code does not have: nobody
passes the flag, the examples do not pass it, and the recorded V&V numbers are
measured without it.

**Concretely:**

- **Default ON.** If the data carries it and the model is meant to represent
  it, the constructor applies it. Do not put it behind a flag "for
  performance" and do not make the *caller* responsible for knowing it exists.
- **Ablation must be an explicit, visible act** — a `without_*` builder or a
  named environment knob, never the default state. An ablation that is the
  default is not an ablation, it is a missing term.
- **Runtime is not a reason to default it off.** Correctness outranks runtime
  (see the section above). Where the cost is large, state it in the doc
  comment with a measured number.
- **Pin the default with a test.** A default that nothing asserts will drift
  back off silently. `crates/outram-mc-libs/tests/correct_physics_is_default.rs`
  is the pattern: construct through the ordinary path and assert the physics is
  present.
- **When a default changes, RE-MEASURE every V&V number that depended on it**,
  and record the movement including when it is unflattering.

**Why this is a rule and not a preference — the case that produced it.** On
2026-09-20 a nine-hypothesis hunt for `outram-mc-libs`' ICSBEP residuals found
that **unresolved-resonance self-shielding (URR) and resonance elastic
scattering (DBRC) both defaulted OFF**, and that *no* ICSBEP benchmark example
enabled them. Every recorded residual for Godiva, Jemima, HST-009 and LCT-008
had been measured against a model missing both. The omission was invisible
because a missing physics term does not announce itself in `k_eff` — it just
shifts it.

Worse, the defaults were *mis-read* during the hunt itself: a grep for
`with_urr_probability_tables` returned hits in `lct008_keff.rs` and that was
taken as the feature being ACTIVE, when three lines away it read
`urr: args.iter().any(|a| a == "--urr")` — off unless asked for. **Counting a
symbol's presence is not checking a default.**

**And the fix made the benchmark numbers WORSE, which is the point.** Turning
URR on moved Jemima from `-253` to `-395 pcm`; DBRC+URR moved LCT-008 from
`+165` to `+139` (a shift not resolved at 0.7 sigma). Correct physics is not
selected by whether it flatters a comparison — the same lesson as gh:#192's
two-body cap, which moved Godiva **+85 pcm away** from its experiment and was
kept because it was right.

### Low-fidelity crates: hierarchical surrogates, in this order

**A hierarchical (physics-derived, reduced-order) surrogate is preferred to a
data-driven one, and preferred to a calibrated one.** A model whose structure
comes from the physics degrades gracefully outside the range it was built in;
a fit does not, and a fit *looks* right precisely where it was fitted.

The order is mandatory:

1. **Build the hierarchical surrogate and run it UNCALIBRATED.** Derive every
   coefficient from geometry, material properties and the governing equation.
   Then compare against data and **record the disagreement**. This is the step
   that is usually skipped, and it is the only one that produces evidence: an
   honestly-derived 14 % error is worth more than an exact match obtained by
   tuning, because only the first one was capable of failing.
2. **Only then calibrate**, against data, and validate. State which parameters
   were adjusted, over what range, against which dataset, and what the
   uncalibrated value was.
3. **Calibration must be accompanied by ABLATION TESTING.** Turn each
   calibrated parameter off — or back to its derived value — one at a time,
   and report how much of the agreement it was carrying. A calibration whose
   contribution has not been measured is indistinguishable from curve-fitting,
   and the ablation is what tells a reader which parts of the model are
   physics and which are fitting.

**Never calibrate a free parameter until a comparison passes.** That converts
the reference into an input and destroys the check. If a derived model
disagrees, the finding is the disagreement — investigate the geometry, the
material data, or whether the reference describes the same quantity at all.

**Worked example, 2026-09-17** — `htgr_sim_v1`'s HTR-10 passive decay-heat
path. The original two-leg chain had its conductance split *fitted* so the
series value reproduced Hu et al.'s published 206 kW, and carried **no
radiation term on either leg**. Rebuilt as five legs derived from the R-Z zone
map, published vessel dimensions, ZBS `k_eff(T)` and Stefan-Boltzmann on the
two radiative legs, it gives **234.5 kW — +13.9 % against the published
figure, derived and not fitted**. Calibrating an effective axial length until
it matched exactly was considered and rejected by the maintainer. The +13.9 %
is now a real gate that can fail; the previous exact agreement could not.
See `crates/outram-park-digital-twin-engine/CLAUDE.md`.

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
   kovan-cli api-docs <crate-dir-name>               # e.g. outram-foam-basic-lib
   ```

   Use **`kovan-cli`**, not `kovan` — `kovan` is the egui GUI binary and will
   hang a non-interactive session trying to open a display.

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

3. **Staleness audit.** Sweep the READMEs, the open GitHub issues, and every
   markdown file (recursively) for drift versus the actual code/state:
   internal contradictions, references to renamed/removed crates or files,
   "planned/TODO" items that are actually done, wrong member lists or crate
   counts, issues that should be closed (or reopened). Fix in-crate drift; for
   cross-cutting or issue-state changes, report candidates rather than
   silently editing — **issues are closed by the maintainer's decision**, and
   the read-only auditor never mutates them.

4. **Codify / update** this command here if the routine itself changes.

**How to run it as a fleet:** partition strictly by crate (one agent per crate,
no shared files → `cargo fmt -p` is safe to avoid per the parallel-agent rule),
plus a separate **read-only** agent for the cross-cutting markdown + issue
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

- **`kovan-cli api-docs <crate>`** — regenerates `crates/<crate>/docs/<crate>-api.md`, the
  committed markdown mirror of a crate's public API and the third leg of the
  per-crate `docs/` convention. Step 1 of the bookkeeping pass runs it. (It
  replaced `scripts/gen_api_docs.py`, retired 2026-08-14, so the doc toolchain
  needs no Python interpreter — same reasoning as epic `op-yz7b`.) Use
  `kovan-cli`, **not** `kovan` (the GUI binary — it hangs a headless session).
- **`kovan-cli agent-docs-gen --regenerate-missing`** — generates a mirror for a
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

This is a rule because an agent once shipped an issue/commit/hand-off claiming
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

This is settled direction, not a preference. Five Python scripts have been
retired under it — `historian.py`, `token_usage.py`, `gen_api_docs.py`,
`gen_aster_behaviour_registry.py` and `kloc_accounting.py` — replaced by
`kovan-cli historian` / `tokens` / `api-docs` and `kovan kloc`.
**`scripts/` now holds no tracked Python.**

**Why, concretely.** A script merely has to exist; an interpreter has to be
installed, on `PATH`, and not shadowed. On Windows `python3` routinely
resolves to a Microsoft Store alias stub that prints an advert and exits —
which silently turned the token-accounting git hooks into no-ops and let
commits ship with no `API-Usage` trailer at all. That is the failure mode this
rule exists to prevent: not an error, a **silent** no-op in the thing that
keeps the records honest.

**Scope.** Documentation generation, repository accounting, and the artifacts
either produces. It does **not** reach into `collaboration/` (gitignored
scratch owned by collaborators), `reference-data/` (vendored upstream trees),
or a third-party tool that happens to be written in Python.

**Seven first-party Python files remain and are NOT covered by this rule as
written:** `crates/outram-park-fork-coolprop/dev/*.py`. Six are **code
generation** (they read the gitignored upstream CoolProp JSON clone and emit
Rust), which is neither documentation nor accounting; `gen_latex_doc.py`
arguably is. Whether to bring them in is a maintainer decision that has not
been made. **Do not delete them under this rule without asking.** Everything
else matching `*.py` is vendored upstream source under `upstream_source/` or
gitignored `collaboration/` scratch — both explicitly out of scope.

**When porting, gate parity — do not waive it.** If the old output is not
committed anywhere, generate it with the Python *before* deleting the script,
commit it, then port and diff. `op-w44a.7` did this and it was a real check;
`op-yz7b` did not, and that gap is recorded as a known weakness.

**A Python script that is a published reproducibility artifact is a different
question — ask, do not delete.** Where a script exists so that a *journal
reader* can re-derive a table or figure, replacing it with a Rust binary
raises the reproduction bar from "run this script" to "build a 43-crate Rust
workspace", and may break a byte-identical copy held in a manuscript
repository. Raise it with the maintainer rather than applying this rule
mechanically. `kloc_accounting.py` was exactly that case: it was put to the
maintainer on 2026-08-14 with the consequence stated, and they chose the full
port.

> The retirement table, the full Windows-alias incident and the
> `kloc_accounting.py` decision:
> [`docs/claude-md-rationale/accounting-and-no-python.md`](docs/claude-md-rationale/accounting-and-no-python.md).
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
(Open-source Unified TRAnsient Multi-Physics Advanced Reactor simulation Kit)
Rust suite.
Several crates that used to live as independent GitHub repositories under
`github.com/theodoreOnzGit` are now consolidated here under `crates/` and are
built, tested, and published from this single repository.

## Members

**43 member crates.** The table below is a one-line index: what each crate is
for, and whether it is declared mature. **Each crate's own
`crates/<crate>/CLAUDE.md` and `README.md` are the authority** for its status,
its bar, its known gaps and its correction history — this table is a pointer,
not a status record, and must not be cited as evidence of anything.

> Full per-crate prose, with every status note, caveat and dated correction as
> it stood on 2026-09-21:
> [`docs/claude-md-rationale/crate-roster.md`](docs/claude-md-rationale/crate-roster.md).

All crates are **GPL-3.0** except `kovan` (**AGPL-3.0-only**, workspace
exception — see `crates/kovan/NOTICE`) and
`chem-eng-real-time-process-control-simulator` (GPL-3.0 since 2026-08-11;
published versions <= 0.1.1 stay Apache-2.0 — see its `NOTICE`). "Mature" in
the last column means the maintainer has declared it so — see "When this
applies: only to crates declared mature".

| Crate (`crates/…`) | Role | Mature |
|---|---|---|
| `petir` | **Core numerics** — polynomials, equations, transforms, integration, roots. `no_std`, dependency-lean, ported from GSL. Every other crate's numerics floor. | ✅ |
| `outram-foam-basic-lib` | OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, solvers, interpolation, thermophysics, fields, mesh, FV operators | ✅ |
| `outram-foam-turbulence-lib` | OpenFOAM turbulence closures (k-ω SST implemented; others scaffolded) | |
| `outram-foam-appbuilder-lib` | OpenFOAM solver-application layer + case I/O; host of the **GeN-Foam** deterministic-neutronics + TH port | ✅ |
| `outram-foam-mesh` | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, …) | |
| `outram-foam-cli` | OpenFOAM-style command-line utilities as terminal binaries | |
| `outram-foam-multiphase` | Phase-II multiphase CFD — drift-flux first. Scaffold, no human V&V | |
| `njoy-outram-park-fork` | **All nuclear data** — NJOY2016 ENDF port (RECONR/BROADR/THERMR/ACER), Faddeeva, windowed multipole, ν̄/χ. Exposes `XsProvider` | ✅ |
| `outram-mc-libs` | **Monte Carlo transport** — CSG geometry, tracking, k-eigenvalue, delta tracking, depletion. **Data-free**: pulls from `njoy-outram-park-fork` | ✅ |
| `teh-o-prke` | Point Reactor Kinetics (PRKE) | ✅ |
| `nee_soon` | Integration / coupling layer — MC ⟷ deterministic/TH ⟷ nuclear data ⟷ PRKE. Steady state only, no validation | |
| `bedok` | Systems-level multiphysics — 3-D nodal diffusion + channel TH, above 1-D neutronics and below CFD. IAEA-3D matches published `k_eff`; stage-2 corrections on by default | |
| `tuas_boussinesq_solver` | Thermal-hydraulics (Boussinesq single-phase) — TUAS | ✅ |
| `tampines` | Central thermal-hydraulic framework — composes TUAS, CoolProp, steam tables, outram-foam, chem-eng | |
| `tampines-steam-tables` | IAPWS-IF97 steam/water properties + steam-turbine equations | ✅ |
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library | ✅ |
| `outram-park-fork-coolprop` | Pure-Rust fork of **CoolProp** — Helmholtz-EOS properties (137 fluids, incompressibles, humid air, mixtures) | |
| `outram-park-fork-dwsim-libs` | Pure-Rust fork of **DWSIM** process-simulation building blocks. **EOS layer only** has met its bar; flash layer open | ✅ |
| `outram-park-fork-offbeat` | Pure-Rust fork of **OFFBEAT** — nuclear fuel performance (eigenstrain, rheology, gap/contact, burnup, FGR, corrosion) | |
| `farrer-park` | **FEM structural mechanics** — small-strain elasticity, J2 plasticity, crystal plasticity. **Shear locking measured and NOT cured** (`op-uqqg`) | ✅ |
| `outram-park-fork-liggghts` | Granular DEM — contact mechanics, thermal DEM, pebble-bed physics (ports LIGGGHTS). Cross-code verified; **no experimental validation** | ✅ |
| `outram-park-fork-pflotran` | Pure-Rust fork of **PFLOTRAN** — subsurface flow & reactive transport. Scaffold | |
| `outram-park-fork-cfmesh` | Pure-Rust fork of **cfMesh** — Cartesian/tet/polyhedral volume meshing with boundary layers | |
| `outram-park-fork-moltres` | **Circulating-fuel MSR** — multigroup diffusion + precursor drift + salt heat transfer on the FV layer. Steady eigenvalue only, no consumer yet | |
| `outram-park-fork-onix` | Pure-Rust fork of **ONIX** — Bateman/CRAM depletion + fission-product inventory | |
| `outram-park-fork-thermochimica` | Pure-Rust fork of **ORNL Thermochimica** — molten-salt Gibbs-energy minimisation. Scaffold | |
| `boon-lay` | TRISO-particle / Lagrangian decay simulator; includes the TRISO-ATOPS fork | |
| `kaki-bukit` | **KAKI BUKIT** — agent-based nuclear fuel-cycle kernel, `no_std` fork of CYCLUS/CYCAMORE, numerics from `petir`. Scaffold, no human V&V | |
| `changi` | **CHANGI** — atmospheric dispersion, plume transport, deposition, ground contamination. Two ports: FLEXPART v10.4 (scalar kernels only) and the Gaussian puff model of `Hammerling-Research-Group/puff` (physics complete, MIT→GPL one-way). Plus `changi::activity` (2026-09-21, **not a port, no upstream**): unit-release chi/Q, decay in transit, dry deposition; deposition velocities are uncited placeholders. **Research/education/V&V only** | |
| `sembawang` | **SEMBAWANG** — source term for CHANGI. ~~Placeholder: nothing implemented~~ **CORRECTED 2026-09-21** — the TRISO fission-product **release** path exists (on `boon-lay`'s TRISO-ATOPS fork, under a caller-**prescribed** temperature transient and inventory). **Severe-accident progression (melt, relocation, vessel failure, MCCI, hydrogen, aerosols) is NOT implemented.** No human V&V | |
| `redhill` | **REDHILL** — groundwater and geological transport of radionuclides after deposition. **Placeholder: nothing implemented** | |
| `raffles` | **RAFFLES** — UQ / risk analysis ported from RAVEN. **Owned by Adolphus Lye.** Apache-2.0 → GPL-3.0 is **one-way**. Implemented in part, no human V&V | |
| `outram-park-mpi` | Pure-Rust **MPICH** subset over a shared-memory threads-as-ranks transport. No C/FFI, Android-buildable. Scaffold | |
| `outram-blender` | Mesh-authoring frontend (GPL fork of Blender's mesh architecture) + the MC and OpenFOAM export bridges | |
| `dhoby-ghaut` | **DHOBY GHAUT** — intended GUI home for the meshing and MC studios. **Placeholder**; holds the `mc_studio` / `mesh_studio` examples | |
| `outram-park-digital-twin-engine` | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only) | |
| `kovan-common` | KOVAN shared canonical types (`KovanDocument`, `KovanSymbol`, …) | |
| `kovan-discovery` | KOVAN file discovery + text search (`ignore` walker, `grep-*` engine) | |
| `kovan-literature` | KOVAN literature archive — PDF → Markdown → `KovanDocument` → BibTeX. `open/` committable, `proprietary/` gitignored | |
| `kovan-semantics` | KOVAN repo understanding — ripgrep-first, escalating to language servers | |
| `kovan-codegen` | KOVAN deterministic code generation for known numerical methods. Not an AI assistant | |
| `kovan-metrics` | KOVAN repository accounting — token trailers and the historian report | |
| `kovan` (bins `kovan`, `kovan-cli`, `kovan-tui`) | KOVAN's three front ends: `kovan` = **human GUI** (egui, digitiser window); `kovan-cli` = **agent CLI**; `kovan-tui` = **human TUI** (ratatui, Android/Termux-usable) | |

> **KOVAN** is the deterministic *knowledge* layer (literature + semantics +
> codegen), interfaced three ways, all binaries of the single `kovan` crate.
> Offline / Android-first, no cloud, no Tree-sitter/SQLite/vector-store. Full
> design spec: **`docs/kovan.md`** (+ `docs/kovan-architecture.md`). Non-GUI
> kovan crates build for Android; only the `kovan` GUI's egui/eframe stack is
> Android-hostile and stays behind the `gui` feature.

> **MSRE digital-twin group:** `outram-park-fork-moltres`,
> `outram-park-fork-onix` and `outram-park-fork-thermochimica`, tracked under
> the **`op-6w0`** epic. All three are AI-assisted drafts with no human V&V and
> none is wired into a simulator yet — do not describe any as validated.
> Scoping: `docs/reactor-scoping/msre.md`.

> **Consequence chain:** SEMBAWANG → CHANGI → REDHILL (source term →
> atmospheric dispersion → groundwater). Two of the three are placeholders.

> **Neutronics architecture:** the responsibility split (nuclear data ⟂ Monte
> Carlo ⟂ deterministic/TH ⟂ coupling), the dependency graph and phasing live
> in **`docs/architecture.md`**. Rule of thumb: *all* cross-section /
> nuclear-data code belongs in `njoy-outram-park-fork`; transport crates are
> data-free and pull from it.

**Planned future crates** (not yet in the workspace): `openfoam-icof`
(**icoFoam**), `openfoam-cht` (**chtMultiRegionFoam**), `openfoam-rho`
(**rhoPimpleFoam** / **sonicFoam**) — all on `outram-foam-basic-lib`.
**GenFOAM** is *ported inside* `outram-foam-appbuilder-lib` (`src/genfoam/`,
~32k lines / ~262 tests as of 2026-08-07; AI-assisted draft, no human V&V).

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`outram-foam-basic-lib`. `outram-foam-basic-lib` provides the mathematical
building blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling
logic, and turbulence model registries belong in solver-specific crates so
that `outram-foam-basic-lib` stays publishable independently.

**Internal dependency edges** are all by **path**, not crates.io. The ones
worth knowing: `teh-o-prke → {tuas (dev), chem-eng (real)}`; `tuas` dev-deps →
`{chem-eng, teh-o-prke}`; `nee_soon → teh-o-prke`;
`outram-park-digital-twin-engine → nee_soon`; `tampines` dev-deps →
`{tuas, teh-o-prke, chem-eng}` (the **library** itself is TUAS-free);
`outram-mc-libs → njoy-outram-park-fork` (cross sections).
`outram-foam-basic-lib` has no internal deps, and `njoy-outram-park-fork` is
kept lean (`thiserror`, `uom`; no BLAS) so data consumers stay light.

**`farrer-park → outram-foam-basic-lib` is for the shared numerical backend
ONLY.** Its FEM `CsrMatrix` implements
`outram_foam_basic_lib::linear_operator::LinearOperator` and drives that
crate's `cg_op`/`gmres_op`/`bicgstab_op`. It does **not** take the FV
discretisation — `LduMatrix` stays FVM-optimised and FEM stays FEM
(GitHub issue #175, epic `op-vrtt`). `linear_operator` was added for this and
is purely additive. GAMG and Gauss-Seidel are *not* on the contract, since
coarsening needs face addressing, so `farrer-park` carries its own CSR ILU(0).

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

### `burn` is this workspace's PyTorch (maintainer direction, 2026-09-16)

Several of the codes this suite ports from reach for **PyTorch** for their
machine-learning parts — RAVEN's surrogates are the first case to land. The
Rust replacement is **`burn`** (tracel-ai, MIT OR Apache-2.0, so permissive
into GPL-3.0 and one-way like the rest): **reach for `burn` wherever the
upstream reaches for PyTorch, as far as `burn` will go.** Do not introduce a
second ML framework, and do not hand-roll a tensor/autodiff layer beside it.

- Declared once in the root `[workspace.dependencies]` as
  `burn = { version = "0.21.0", default-features = false }` — i.e. **`no_std`**,
  which is deliberate: `burn` runs on `core` + `alloc` (`alloc` is implicit,
  there is no feature to name), and that is what keeps it inside the Android
  and wasm rules below. `default-features = false` also drops `burn`'s
  std-only dataset/network/train/sqlite stack. A `std` binary links a `no_std`
  `burn` without complaint, so do not turn defaults back on for convenience.
- **The backend is the consumer's choice, and the safe one is `ndarray`** —
  pure Rust, libm-backed, no system BLAS. `accelerate`, `blas-netlib`,
  `openblas`, `openblas-system`, `tch`, `candle`, `cuda` and `rocm` all want a
  system BLAS/LAPACK or a C/C++ toolchain and must never appear in an
  unconditional library build.
- **GPU is out of scope for the first pass.** `burn`'s
  `wgpu`/`vulkan`/`metal`/`webgpu` backends inherit the `wgpu` rule: behind a
  feature, example or optional bin under `cfg(not(target_os = "android"))`.
- **MSRV:** `burn` 0.21 declares `rust-version = "1.92"` and `edition = "2024"`,
  so any crate that actually enables it raises its own toolchain floor to 1.92.
  Upstream has deprecated `burn-ndarray` in the 0.22 pre-releases in favour of
  `burn-flex`/`burn-cpu`; check that crate's `no_std` story before bumping.
- First consumer: `raffles`, optionally, behind its own `burn` feature (off by
  default) — see `crates/raffles/CLAUDE.md` "Machine learning".

## Android / Termux portability (HARD RULE for non-GUI code)

**Hard rule (not a default): every crate's non-GUI library code MUST compile on
Termux — native, on-device Android — with Android-hostile pieces held off behind
Android feature gates.** "Compiles on Termux" is the acceptance bar: a build run
*inside Termux* (native `aarch64-linux-android`, no NDK cross-toolchain, no system
BLAS/LAPACK, no C/Fortran toolchain) must succeed for every non-GUI library. This
does not bend for convenience — if a change cannot build on Termux, it is not done
until the offending dependency/test/example is gated off Android in the *same*
change. Tracking: the **`op-zfr` "Android support" epic**.

- **Termux builds natively on the device**, so the target is
  `aarch64-linux-android` and **`target_os = "android"`** (not `"linux"`).
  Every gate keys off that. No system package manager for BLAS/LAPACK/GUI libs
  is assumed to exist.
- Prefer an explicit **Cargo feature** (e.g. an inverted `native-blas`/`gui`
  feature that is simply *not* enabled on Termux) **plus** the
  `cfg(target_os = "android")` target gate, so a Termux user gets a working
  build from the default feature set with no manual flag-twiddling.
- **`ndarray-linalg`** — and anything needing system BLAS/LAPACK, a C/Fortran
  toolchain, or `std`-GUI/windowing — is Android-hostile. Declare it only
  under target-conditional tables, never unconditionally.
- **Examples/tests/benches count — they are NOT exempt.** A native Termux
  build compiles them, so an Android-hostile dep in *any* of them breaks the
  on-device build even when the library is clean. Tests/benches take
  `#![cfg(not(target_os = "android"))]` at the top of the file; examples/bins
  need an **Android stub `main`** (a blanked file gives "main function not
  found") with every desktop item gated. Precedents:
  `outram-foam-basic-lib`'s `tests/matrix_bench.rs`,
  `njoy-outram-park-fork`'s `examples/gpu_wmp_bench.rs`.
- **Only windowing GUI is out of scope — terminal apps are IN scope.** Termux
  *is* a terminal, so a CLI or a `ratatui` TUI must compile and run on Android
  like any other non-GUI crate. `egui`/`eframe`/`wgpu`-surface windowing stays
  behind examples/optional bins/target gates. `kovan-cli` and `kovan-tui` are
  in scope and verified; only `outram-park-digital-twin-engine` is a genuine
  GUI exemption.
- **New code follows this by default.** If you add a dep or a test that can't
  build on Android, target-gate it in the same change and note it.
- **The check MUST cover all targets, not just `--lib`.** A `--lib`-only check
  silently misses broken examples/tests/benches — the exact gap that let the
  `godiva_gpu_benchmark` example ship un-gated. The proxy check is
  **`cargo check --release -p <crate> --all-targets --target
  aarch64-linux-android`** (needs the Android target + NDK / `cargo-ndk`). The
  **authoritative** check is a **native Termux build** on-device. Never report
  Android/Termux support as verified from a `--lib`-only run.
## WebAssembly (`wasm32-unknown-unknown`) — supported target, with a hard caveat

**Every in-scope crate's library must compile for `wasm32-unknown-unknown`, and
a gate enforces it.** Added 2026-09-04. **37 of the 43 members are in scope; 6 are deliberately
excluded** — `kovan`, `kovan-discovery`, `kovan-metrics`, `kovan-semantics`,
`bedok` and `outram-blender`, each with its reason in the script.
**CORRECTED 2026-09-21** — this file had said "34 of 40", stale on both
numbers; verified against `scripts/check-wasm.sh` and the workspace member
list. Run `scripts/check-wasm.sh` (the gate;
`-v` shows first error lines); install the target once with
`rustup target add wasm32-unknown-unknown`.

**COMPILING IS NOT RUNNING.** The gate checks **compilation only**, and the
gap between that and working in a browser is large: `std::thread::spawn`,
`std::time::Instant` and `std::fs` all **compile** for wasm32 and fail only at
**run time**. `chem-eng-real-time-process-control-simulator` is the standing
proof — it passes the gate today while containing 5 `thread::spawn` sites and
10 files using `std::fs`. So "passes `check-wasm.sh`" means *the types line
up*, never *this works in a browser*. Making a crate genuinely run on wasm is
per-crate work tracked under epic `op-eeqw` (GH #39). **Do not describe a
crate as wasm-ready on the strength of this gate.**

**The gate is `--lib`, and that is a known limitation.** The Android rule uses
`--all-targets`; wasm cannot, because several crates carry GUI examples and
terminal binaries that legitimately cannot build for wasm, and a permanently
red gate is an ignored gate. A broken wasm-facing *example* will **not** be
caught. Stated here rather than papered over.

**Choose the right mechanism: a feature is for "the user may not want this", a
target gate is for "this cannot exist here".** And gate off the *right*
target — `rayon`, `ratatui`/`crossterm`, `async-opcua`/`tokio`/`mdns-sd`/
`directories` are all gated **off wasm only** and stay available on Android;
`wgpu` is gated **off Android** (no system Vulkan/Metal loader) and separately
off wasm in some crates for a different reason.

**Exclusions are deliberate and listed in one place** —
`scripts/check-wasm.sh` carries the list with a reason per crate. Note
`kovan-codegen`, `kovan-common` and `kovan-literature` are **not** excluded:
they already pass, so gating them is free.

**This does not relax the Android rule.** wasm is an additional target, not a
replacement, and nothing may break `aarch64-linux-android`.

> The gate-vs-feature reasoning table, the `wasm_par.rs` / `getrandom` /
> 32-bit-`usize` / `!Send`-wgpu patterns and the full exclusion list:
> [`docs/claude-md-rationale/portability-android-wasm.md`](docs/claude-md-rationale/portability-android-wasm.md).
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

**An agent session did this exact refactor workspace-wide on 2026-08-19** —
shortening directory and file names to fix path-length violations while
keeping the whole workspace compiling (including test binaries) throughout is
a routine that has already been proven out here, not a novel or risky
undertaking. It is safe to ask for again.

Two things that pass carried that are worth repeating:

- **Check every renamed identifier for public-API exposure first** — a
  workspace-wide grep for real `use`/`pub use` references, not just a
  directory listing. Four `tuas_boussinesq_solver` identifiers turned out to
  be genuinely public cross-crate API.
- **The compiler is the reference-checker.** kopitiam's `rename` could not be
  used (rust-analyzer rejected every request — filed as
  [kopitiam#30](https://github.com/theodoreOnzGit/kopitiam/issues/30)), so the
  fallback this file's "Workflow rules" already prescribes — enumerate
  references by text search, rename by hand, let `cargo check` catch the
  misses — carried the whole refactor without incident.

The compatibility re-exports used on the first push were a **same-session
bridge, not a standing policy**; they were deleted the same day once every
downstream reference had been migrated. If a future pass repeats this on a
crate with real crates.io consumers outside this workspace, keeping the alias
for at least one published version is worth raising with the maintainer
explicitly, since those consumers cannot be grepped for and fixed in-session.

> Full account — the exact renames, the four public identifiers, the
> verification commands run at each stage:
> [`docs/claude-md-rationale/path-length-refactor-precedent.md`](docs/claude-md-rationale/path-length-refactor-precedent.md).

## Build & test

> Measured evidence, the CI-trigger investigation and the long-tests worked
> examples: [`docs/claude-md-rationale/build-test-and-ci.md`](docs/claude-md-rationale/build-test-and-ci.md).

A system BLAS is **only** needed to run `outram-foam-basic-lib`'s
`matrix_bench` test (its sole remaining `ndarray-linalg` dev-dependency) — no
library in the workspace needs it (`sudo pacman -S openblas` /
`sudo apt install libopenblas-dev`).

**This workspace has a submodule as of 2026-09-20** — `reference-data/ace`
(`theodoreOnzGit/ace_and_other_data`), holding the gzipped NJOY2016 ACE tables
that are too large to track here directly. Clone with
`git clone --recurse-submodules`, or run `git submodule update --init
reference-data/ace` afterwards. A plain clone leaves that path an **empty
directory rather than an error**, so nothing complains until something looks
for a table and does not find one.

```bash
cargo build --workspace --release                   # all libraries
cargo check --release --workspace --lib --tests     # type-check
cargo test  --workspace --lib --tests --release     # run the test suites
```

Note: a bare `cargo test --workspace` also compiles the **examples**. Use
`--lib --tests` to skip them.

### EVERYTHING is release — every profile, every target, no exceptions

**Maintainer direction, 2026-09-19: "everything should be release, no debug".**
**Every `cargo build`, `check`, `test`, `run`, `clippy` and `bench` in this
workspace passes `--release`** — in a command you type, in a command a doc
tells someone to type, and in a script. This includes `cargo check`, which
otherwise builds a second complete `target/debug` tree beside the release one,
and it includes every `--target` cross-compilation invocation (the wasm and
Android gates), which default to the dev profile and put the result under a
directory nobody looks at.

The profile changes what a check *costs*, never what it *reports*. Measured
2026-09-19: moving the gates to release took the five build trees from ~15 GB
to 5.2 GB for identical results. `cargo install`, `cargo publish` and
`cargo fmt` need nothing — the first two build in release already, the third
builds nothing.

**Historical records stay as they were run**
(`verification_and_validation/generated/`, `debug_markdowns/`, the
`docs/<crate>-api.md` mirrors, the V&V logs). Fix the instruction, never the
receipt.

### TUAS natural-circulation tests are VERY long running — run them in parallel

**HARD RULE.** The CIET coupled-DRACS natural-circulation regression tests and
simulations in `crates/tuas_boussinesq_solver` (under
`pre_built_components/ciet_nat_circ_tests/`, including
`coupled_dracs_loop_tests/` and `para_heat_loss_regr_tests/`) integrate a
coupled loop at a 0.1 s timestep for 2000–2500 s of simulated time to reach
steady state — see the crate `CLAUDE.md` "Testing Notes".

**They must be run in parallel, not serially.** Let cargo's test harness use
all cores: `cargo test --release -p tuas_boussinesq_solver`. **Do not add
`--test-threads=1`**, and do not wrap them in anything that serialises
execution. If a specific case needs isolation, isolate that case rather than
the whole suite.

- **Budget real wall-clock time.** A default 120 s command timeout will kill
  them mid-run; give them a generous timeout or run them in the background.
- **Run the targeted subset** while iterating (`cargo test --release -p
  tuas_boussinesq_solver <substring>`) and the full suite only when finishing.
- **A timeout is not a failure.** Do not report a killed run as a failing test,
  and never loosen a tolerance because a long test was inconvenient.

### Any test over 5 minutes is gated behind `long-tests` (HARD RULE)

**Every individual test whose runtime exceeds 5 minutes MUST be gated behind
its crate's `long-tests` feature, and that feature MUST be in the crate's
`default` set.** Both halves bind: the gate is mandatory so a fast run is
possible at all, and the default-on is mandatory so the slow tests still run
unless someone deliberately turns them off.

**The threshold is per test, not per suite.** Three tiers — do not collapse
them together:

| runtime | treatment | in a default `cargo test`? |
|---|---|---|
| under 5 min | nothing — a plain `#[test]` | yes |
| 5 min to about an hour | `#[cfg_attr(not(feature = "long-tests"), ignore = "...")]` | **yes**, skipped only under `--no-default-features` |
| multiple hours | plain unconditional `#[ignore = "..."]` | no — opt in with `--ignored` |

**Measure, never inherit a number.** Runtimes in this workspace's own comments
have been wrong by a factor of four (see the rationale doc). Record the number
**with its date**, and re-measure rather than trusting a figure written by a
previous session on different hardware. When it is close to the threshold,
gate it. Do **not** promote an hours-long test into the middle tier, and do
**not** demote a mid-tier test into the bottom one — if a test is the evidence
for a claim quoted in this file, it belongs in the default run.

**A second, orthogonal criterion shares the same flag: a test built on heavy
reference data goes behind `long-tests` even when its runtime is well under 5
minutes** (maintainer direction, 2026-09-20). Cargo gives one lever, not two.
The point is that a *short* run must need no heavy reference data, which is
what makes the CI split below possible. **State which criterion applies in the
`ignore` message and the doc comment** — a reader who sees a 3.3 s test behind
`long-tests` will otherwise assume the runtime figure is wrong.

**This rule is about RUNTIME (and reference data) ONLY. It un-ignores
nothing.** An `#[ignore]` that exists for any other reason keeps it, at any
runtime: measurements and diagnostics that assert nothing, unimplemented or
won't-port work, and anything deliberately on hold by maintainer decision.
Before converting any `#[ignore]` to the feature gate, read its message. If it
does not say the test is slow, leave it alone.

**How to gate one:**

```rust
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "long test (~9 min); runs by default, skipped under --no-default-features"
)]
fn coupled_dracs_loop_reaches_steady_state() { /* ... */ }
```

```toml
[features]
default = ["long-tests"]
long-tests = []
```

**Use `#[cfg_attr(..., ignore)]`, never `#![cfg(feature = ...)]`.** The
`ignore` form keeps the test compiled and reports it as `ignored` when
switched off. Blanking it out with `cfg` makes it disappear silently, and a
test that can silently vanish is a test that rots.

**Turning them off, for iteration only:** `cargo quick-test` (whole workspace)
or `cargo quick-test -p <crate>`. The alias lives in `.cargo/config.toml` and
expands to `cargo test --release --lib --tests --no-default-features`.

**Work is NOT done on a `quick-test` run.** Before reporting work complete,
before committing, and before any hand-off, run
`cargo test --workspace --lib --tests --release`. **Say which of the two you
ran.** "Tests pass (quick-test; long tests not run)" is an honest report.
"Tests pass" after a `quick-test` is not.

**When you write a test that crosses the threshold, gate it in the same
change** — add the feature to the crate's `Cargo.toml` if it has none, and
state the measured runtime in the `ignore` message. Do not guess; measure.

### CI runs short on `develop` and long on `main`

| branch | workflow | command | `long-tests` |
|---|---|---|---|
| `develop` | `.github/workflows/fast-tests.yml` | `cargo quick-test` | **off** |
| `main` | `.github/workflows/full-tests.yml` | `cargo test --workspace --lib --tests --release` | **on** |
| *on request, any branch* | `.github/workflows/manual-tests.yml` | either, chosen by input | selectable |

`manual-tests.yml` fills the gap the split leaves — running the FULL suite
against `develop` on demand. It has **two** triggers because GitHub exposes
`workflow_dispatch` only for workflows present on the **default branch**, and
`main` has no `.github` directory at all. The second trigger is a push to a
`ci-run/**` branch, which a push event runs from the pushed ref itself:

```bash
git push -f origin develop:ci-run/develop     # full suite, develop's exact tree
```

Force-push is expected — `ci-run/**` branches are disposable triggers, never
merged from, safe to delete. The pattern excludes `develop` and `main` so it
cannot fire on ordinary work.

**Not yet enabled, deliberately:** `OUTRAM_PARK_REQUIRE_REFERENCE_DATA=1` on
the full job, which turns a data-skip into a hard failure. Whether the whole
workspace passes with it on has **not been measured**; switching it on
unmeasured would paint the job red for the wrong reason. Measure, then enable.
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
`README`/`docs`, V&V write-ups, and issues stay clear standard English**, and no
mandatory rule (responsible-use / data policy, V&V docs, Rust design rules,
never-auto-commit/push — and the working-hours guardrail whenever the session
has opted into it) is relaxed — correctness and honesty come first. **When in Singlish mode, read `SINGLISH_MODE.md` and apply its logged
corrections.** Default is standard English; opt-in only.


## Session completion

This protocol applies when ending an implementation workflow. It is
subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** — `gh issue create` for anything that
   needs follow-up.
2. **Run quality gates** (if code changed) — tests, linters, builds, in
   release mode, and say which suite you ran (see "Build & test").
3. **Update issue status** — comment progress on in-progress items; propose
   closures rather than making them.
4. **Handle git by the never-auto-commit/push rule** — report `git status` and
   the proposed commands and wait for approval, unless the user or the stop
   hook has asked for the commit.
5. **Hand off** — summarise changes, validation, issue status, and any blocked
   commit/push step with the exact command and error.
