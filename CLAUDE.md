# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

**How this file is organised (2026-09-21).** Rules about getting the
**physics** right come first and are written out in full. Everything else is a
one-line pointer to `docs/claude-md/`. **A pointer does not make a rule
optional**: when the task touches that topic, read the file, because it binds
exactly as if it were here.

| When you are… | Read |
|---|---|
| using `kovan-cli`, handling literature, or digitising a figure | [`docs/claude-md/kovan-tooling.md`](docs/claude-md/kovan-tooling.md) |
| running an agent fleet or a `Workflow` (post progress every 15 min) | [`docs/claude-md/agent-fleet-reporting.md`](docs/claude-md/agent-fleet-reporting.md) |
| committing (token trailers) or merging `develop` → `main` (historian report) | [`docs/claude-md/accounting-and-historian.md`](docs/claude-md/accounting-and-historian.md) |
| writing or editing any `README.md` or markdown with math | [`docs/claude-md/markdown-format.md`](docs/claude-md/markdown-format.md) |
| adding or changing a public API; judging a crate's maturity; the Haiku API test | [`docs/claude-md/api-design-and-maturity.md`](docs/claude-md/api-design-and-maturity.md) |
| asked for a "bookkeeping pass"; regenerating `docs/<crate>-api.md`; tempted to write Python for docs/accounting | [`docs/claude-md/bookkeeping-and-api-docs.md`](docs/claude-md/bookkeeping-and-api-docs.md) |
| writing Rust (enums not `dyn`, no `Box`, no lifetimes, `Arc<RwLock>`) | [`docs/claude-md/rust-design-rules.md`](docs/claude-md/rust-design-rules.md) |
| looking up a crate, or changing a dependency (`burn`, BLAS, versions) | [`docs/claude-md/members-and-dependencies.md`](docs/claude-md/members-and-dependencies.md) |
| adding a dependency, test or example (Android/Termux, wasm), or a deep file path (170-char cap) | [`docs/claude-md/portability-android-wasm-paths.md`](docs/claude-md/portability-android-wasm-paths.md) |
| writing a slow test, or touching CI (`long-tests` tiers, TUAS parallel runs) | [`docs/claude-md/long-tests-and-ci.md`](docs/claude-md/long-tests-and-ci.md) |
| building an egui simulator (headless mode is mandatory) | [`crates/outram-park-digital-twin-engine/CLAUDE.md`](crates/outram-park-digital-twin-engine/CLAUDE.md) |
| asked to enable the working-hours guardrail (opt-in, off by default) | [`docs/claude-md/working-hours-guardrail.md`](docs/claude-md/working-hours-guardrail.md) |
| asked for Singlish mode | [`docs/claude-md/singlish-mode.md`](docs/claude-md/singlish-mode.md) |
| publishing, upgrading dependencies, or needing consolidation history | [`docs/workspace-maintenance.md`](docs/workspace-maintenance.md) |
| wanting the reasoning and worked examples behind a rule | [`docs/claude-md-rationale/`](docs/claude-md-rationale/) |

Each member crate has its own `CLAUDE.md` and, where relevant, a `docs/`
folder. **The crate's own file is the authority for that crate.**

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

# Part 1 — Physics and correctness (read every session)

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

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Applies to `outram-mc-libs`, `nee_soon`,
every `outram-foam-*` crate, and **every crate downstream of them**: today
`boon-lay`, `dhoby-ghaut`, `farrer-park`, `outram-blender`,
`outram-park-digital-twin-engine`, `outram-park-fork-cfmesh`,
`outram-park-fork-liggghts`, `outram-park-fork-moltres`,
`outram-park-fork-offbeat`, `outram-park-fork-pflotran`, `raffles`,
`sembawang`, `tampines` — and any crate that later takes one of them as a
dependency (check with `cargo metadata`, not this list).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial
  (x-y) slices at the heights that matter (bed, cavity, conus, reflector
  bands); and, for nested geometry, zoomed slices at every level down to the
  smallest (pebble, TRISO particle). Colour by material, with a legend and the
  key dimensions marked.
- **Commit the images with the change** (next to the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them**, and what you could not — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom
reflector mirrored from the top and up to 107 cm short; a core cavity that grew
with the bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon
clipped away (gh:#309, #310); and a TRISO lattice holding 8240 particles while
reporting 8340 (gh:#316, +353 pcm). **Every run completed with green
diagnostics.** Each was found by looking at the built geometry, not by the
eigenvalue.

**Tools.** `outram_mc_libs::geometry::plot` samples a slice of the assembled
geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once it
lands. For HTR-10, `nee_soon`'s `htr10_geometry_export` plus the manuscript's
`plot_htr10_geometry.py` draw from the assembly. Rationale and examples:
gh:#309, #310, #316 and `docs/htr10-rmc-verification-suite.md` s8.

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

**Why it needs to be a rule.** This workspace is 44 crates, many of them ports
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
primary interface to 44 crates that no one can hold in their head, and the
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

# Part 2 — Compliance and workflow

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

## Build & test

> Measured evidence, the CI-trigger investigation and the long-tests worked
> examples: [`docs/claude-md-rationale/build-test-and-ci.md`](docs/claude-md-rationale/build-test-and-ci.md).

A system BLAS is **only** needed to run `outram-foam-basic-lib`'s
`matrix_bench` test (its sole remaining `ndarray-linalg` dev-dependency) — no
library in the workspace needs it (`sudo pacman -S openblas` /
`sudo apt install libopenblas-dev`).

**This workspace has two submodules** — `reference-data/ace`
(`theodoreOnzGit/ace_and_other_data`, since 2026-09-20), holding the gzipped
NJOY2016 ACE tables that are too large to track here directly, and
`crates/kovan-literature/reactor-literature` (`theodoreOnzGit/reactor-literature`,
since 2026-09-22), holding the open literature PDFs (see "Literature and the
Kovan corpus" below). Clone with `git clone --recurse-submodules`, or run
`git submodule update --init` afterwards. A plain clone leaves each path an
**empty directory rather than an error**, so nothing complains until something
looks for a file and does not find one.

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

### Slow tests and CI (summary)

- A test over **5 minutes** (or one needing heavy reference data) is gated
  behind the crate's `long-tests` feature, which is **on by default**.
  Multi-hour tests use a plain `#[ignore]`.
- `cargo quick-test` skips them, for iteration only. **Work is not done on a
  quick-test run**: before reporting complete, run
  `cargo test --workspace --lib --tests --release`, and say which one you ran.
- TUAS natural-circulation tests are very long: run them in parallel, never
  `--test-threads=1`, and a timeout is not a failure.
- CI runs quick on `develop` and full on `main`.

Full rules, tiers and CI triggers: [`docs/claude-md/long-tests-and-ci.md`](docs/claude-md/long-tests-and-ci.md).

## Rust design rules (mandatory) — short form

- **No trait objects** (`Box<dyn …>`, `&dyn …`, `Arc<dyn …>`): dispatch with
  enums. Traits remain as compile-time contracts on each concrete type.
- **No `Box<T>`** (recursive structures excepted): own by value or `Arc<T>`.
- **No lifetime parameters** on structs, traits or impls: own, `Arc`, or index.
- **Shared mutable simulation state is `Arc<RwLock<T>>`**, read-only data is
  `Arc<T>`; no channels for simulation state.
- **`uom` everywhere** a physical quantity crosses an API, with named type
  aliases for complex quantities.

Full text, with the enum-dispatch pattern: [`docs/claude-md/rust-design-rules.md`](docs/claude-md/rust-design-rules.md).

## Portability — three hard build rules (summary)

- **Every non-GUI library must compile on Android/Termux**
  (`aarch64-linux-android`), checked with `--all-targets`. Android-hostile
  dependencies, tests and examples are gated off in the *same* change.
- **In-scope crates must compile for `wasm32-unknown-unknown`**
  (`scripts/check-wasm.sh`). That is compilation only; never call a crate
  wasm-ready on the strength of it.
- **No new file path over 170 characters** (repo-relative).

Full rules, gating patterns and exclusions: [`docs/claude-md/portability-android-wasm-paths.md`](docs/claude-md/portability-android-wasm-paths.md).

# Part 3 — The workspace

## What this is

**OUTRAM PARK backend** — the Cargo **workspace** that houses the OUTRAM PARK
(Open-source Unified TRAnsient Multi-Physics Advanced Reactor simulation Kit)
Rust suite.
Several crates that used to live as independent GitHub repositories under
`github.com/theodoreOnzGit` are now consolidated here under `crates/` and are
built, tested, and published from this single repository.

**44 member crates**, all GPL-3.0 except `kovan` (AGPL-3.0-only). The roster,
maturity marks, internal dependency edges and the dependency policy (all
versions in the root `[workspace.dependencies]`) are in
[`docs/claude-md/members-and-dependencies.md`](docs/claude-md/members-and-dependencies.md).
Two things to know without opening it: **all nuclear-data code belongs in
`njoy-outram-park-fork`** (transport crates are data-free), and **a crate's own
`CLAUDE.md` is the authority for its status**, never a roster row.

## Literature and the Kovan corpus (brief)

- **PDFs do not live in this repository.** Open literature is in the
  `reactor-literature` Git submodule at `crates/kovan-literature/reactor-literature/`
  (`git submodule update --init` it): `kovan-standard-open-corpus/` and
  `theodore-open-corpus/`, each README stating every document's licence basis.
  Proprietary literature is in the maintainer's private repository, never here.
- **Kovan's built-in corpus** is the nuclear-engineering map compiled into
  `crates/kovan/src/corpus.rs`: the topic tree plus metadata for exactly the
  documents in `kovan-standard-open-corpus/`, nothing else.
- Details and rules: [`crates/kovan-literature/CLAUDE.md`](crates/kovan-literature/CLAUDE.md)
  and its `CATALOGUE.md`.

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
