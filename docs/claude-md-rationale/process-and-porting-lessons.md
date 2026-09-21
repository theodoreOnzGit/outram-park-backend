# Rationale: process, search-before-building, doc drift and port debugging

> Split out of the root `CLAUDE.md` on 2026-09-21 to keep that file under the
> 150k-character context limit. **This is the full original text, verbatim**,
> of four hard-rule sections whose rules `CLAUDE.md` still states in full.
> What is kept here is the evidence: the HTR-10 shortcut-vs-process table, the
> five already-existed duplications of 2026-08-12, the three stale doc claims
> of 2026-09-17, and the U-238 LSSF=1 / `thnmax` port-debugging case.

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

- **Tuning an input until a comparison passes.** The benchmark is the check; the
  moment it becomes an input, the check is gone. (This is the same rule the
  "Model hierarchy" section states for calibration — here it is general.)
- **Moving a threshold to make a test pass.** If a gate fails, the first
  hypothesis is that the *thing being gated* is wrong, not the gate.
- **Choosing the instrument after seeing the result.** Pick the measure the
  physics calls for, state why, and keep it even when it is unflattering.
- **Quietly repairing a number you already published.** Correct it in place, say
  what it was, say what changed it.

### What it requires

1. **Justify every input from outside the comparison.** A material property
   comes from the literature or from the upstream code, with a citation — never
   from what makes the answer come out right.
2. **Fix the protocol, not the criterion.** When a bound is breached, change the
   procedure until it is genuinely satisfied.
3. **Match the instrument to the physics**, and say why it is the right one.
4. **Test the assumption the result rests on**, especially when it is load-
   bearing and convenient. An assumption that has never been able to fail is
   not evidence.
5. **Report the disagreement when there is one.** An honestly-derived miss is
   worth more than a fitted hit.
6. **Say which numbers to quote** when several exist, and why the others do not
   count.

### Worked examples — all from the HTR-10 pebble-bed work (GitHub issue #216)

The published filling fraction of **0.61** had been missed by −8.7 % and the
gap was recorded as unexplained. It was closed to **−0.9 %** — and the process
is why the result is worth anything:

| decision | the shortcut | what was done instead |
|---|---|---|
| friction `µ` | tune `µ` down until `φ` hits 0.61 | ran a **2×2 ablation** over the *literature* graphite range (graphite is a solid lubricant, `µ ≈ 0.1–0.2`); reported all four cells, including the two that miss by 3–6 % |
| quasi-static bound breached at `1.57e-2` vs `1e-2` | relax the threshold | **fixed the protocol** — 4× the settle window, giving `~2e-4`; the bound is the only reason the case can claim to measure creep |
| does the result survive? | assume rate-independence, since the theory says so | **ran the control**; it revised two of my own numbers, halving one magnitude and exposing another as an artefact |
| bed height | keep `max z`, it was already written | `max z` is a single-pebble statistic that jumps a full diameter on one placement — switched to the **99th percentile** |
| per-particle agreement through a chaotic rearrangement | loosen the tolerance until it passes | recognised it as **Lyapunov divergence** — asserted tightly at early time where the contact path is verifiable, loosely at late time, and documented why |

And the rule cuts the other way too: **the process being right is what surfaces
defects nobody was looking for.** Questioning whether a run was reproducible —
an assumption no test had ever challenged — exposed a P0 defect in which the
HTR-10 bed gave a *different answer every run*, because the neighbour grid
iterated a randomly-seeded `HashMap`.

> **The point is not that 0.61 was reached. It is that the run which reached it
> was capable of missing.**

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

## A doc claim contradicted by the code is a DEFECT — fix it in the same change (HARD RULE)

**When you find a statement in any `docs/`, `CLAUDE.md`, `README.md` or `///`
doc comment that the code contradicts, correct it in the change where you found
it. Do not file it, do not "note it for later", and do not report it as an
observation while leaving it in place.** A stale doc is not a tidiness problem;
it is a false statement that the next reader — human or agent — will act on.

**Why this is a hard rule and not a courtesy.** This workspace's docs are the
primary interface to 40+ crates that no one can hold in their head, and the
search-before-building rule above *depends on them being true*. A stale
"missing" claim is the worst kind, because it causes exactly the duplication
this file exists to prevent: an agent reads "no model exists", believes it, and
writes a second one. Three found in a single session on 2026-09-17:

| Claim | Reality |
|---|---|
| `docs/reactor-scoping/htr10.md`: "`reference-data/endf/` holds only a README", *re-verified 2026-08-12 — still true* | **39 ENDF/B-VIII.0 tapes**, and `outram-mc-libs` reconstructs ~13 nuclides from them |
| same file: decay heat "is still not wired into `htgr_sim_v1`" | Wired — `kinetics.rs` holds `pub decay: DecayHeat`, seeds at equilibrium, applies per substep |
| `htgr_sim_v1/headless.rs`: "a baseline of the current **PRISMATIC** model" | Retargeted to pebble-bed on 2026-08-12 |

Note the first one carries an explicit **re-verification stamp** and was still
wrong. A dated "still true" marker is evidence of when someone last looked, not
that the claim holds now — so re-check the claim, never trust the stamp.

**How to comply:**

- **Correct it where it lives**, including the generated `docs/<crate>-api.md`
  mirror if the source doc comment changed (`kovan-cli api-docs <crate>`).
- **Strike through rather than delete** when the claim shaped a decision —
  `~~old claim~~ **CORRECTED <date>** — new position`. The history is why a
  reader can trust the correction; a silent edit looks like the doc was always
  right.
- **Say what you verified**, not just what is now true. "39 tapes present" is
  checkable; "this is fixed" is not.
- **A claim you cannot check is not a claim you may leave standing unmarked.**
  Mark it `Not re-checked` with the reason, as section 3 of
  `docs/reactor-scoping/htr10.md` already does.
- **This binds for docs you did not write and were not asked to touch.** Finding
  it makes it yours. The one exception is the scope boundary at the top of this
  file: a doc whose repository root is not this one is out of scope entirely.

**This does not license rewriting docs you merely disagree with.** The trigger
is a claim the *code contradicts* — a falsifiable mismatch you have checked, not
a wording preference, not a different opinion about emphasis. Fixing style while
claiming to fix staleness is how a review pass becomes an unreviewable diff.

## Debugging a port: read upstream first (HARD RULE)

**When a ported module misbehaves, find out how the upstream code handles that
exact situation BEFORE proposing, writing, or testing a fix.** Most of this
workspace is a translation — NJOY2016, PFLOTRAN, CoolProp, GeN-Foam, OFFBEAT,
`code_aster`, DWSIM — and in a translation the overwhelmingly likely cause of a
discrepancy is that upstream does something the port does not. Upstream is the
specification. Reasoning about the physics from first principles, or from what
the port's own comments claim, is not a substitute for reading it.

**What "read upstream first" means, concretely:**

1. **Find the upstream routine** that owns the behaviour and read it — the
   Fortran/C++/VB source, not just the manual. Vendored sources live in the
   gitignored `vendor/` folders (workspace rule); the manuals are in `kovan`'s
   literature store.
2. **Ask what upstream does that we do not.** Bounds and guards are the usual
   answer: an upper energy limit, a card default, a branch on a format flag, a
   range check, a special case for a boundary. A missing *limit* is a far more
   common port defect than a wrong *formula*, because formulas get reviewed
   line-by-line during translation and control flow does not.
3. **Check the data's own format flags before blaming the code.** ENDF-6 (and
   equivalents elsewhere) change the meaning of a section based on flags —
   `LSSF`, `LRU`/`LRF`, `LI`, `INT`. A file where `LSSF=1` means something
   categorically different from `LSSF=0`, and a "missing physics" hypothesis
   that ignores the flag will send you porting a module you did not need.
4. **Only then form a hypothesis, and state its predicted sign and magnitude
   before you measure.** If the fix would move the answer the wrong way, you
   have the wrong hypothesis — stop and go back to step 1.

**Record what you find in the bead**, including when upstream turns out to
handle it the same way we do (that result is worth as much as a defect, and
saves the next session repeating the search).

**Why this exists.** A worked example, 2026-09-10: the U-238 ring-RPT
discrepancy (`op-mzvp.2.12`) was recorded with "URR self-shielding not
reconstructed" as the leading hypothesis and "expect ours low + structureless"
as the predicted signature. Reading the evaluation first would have shown
`LSSF=1` in U-238's `LRU=2` range — MF=3 already carries the infinitely-dilute
unresolved cross sections, so the reconstruction is *not* low (it reproduces
MF=3 to 0.08 % from 24 keV up), and adding PURR self-shielding would have
*raised* k, moving the case further from the reference rather than closer. The
real defect was one upstream guard we had not ported: NJOY bounds BROADR at
`thnmax` and never runs SIGMA1 across the resolved/unresolved boundary, while
~~this port broadens the whole grid unconditionally~~ (`op-sdbk`). Hours went
into a first-principles argument and two speculative patches that reading
`broadr.f90` and the ENDF flag would have pre-empted.

**CORRECTED 2026-09-20 — the guard IS ported.** `broadr::broadening_limit`
implements `thnmax`, citing `broadr.f90:441` and `:522-524`, and
`doppler_broaden_below` copies everything above it through untouched. The
lesson above is unchanged and is why this section exists; only the present
tense was wrong. Verified by reading the function, and visible in
`examples/ace_vs_njoy2016`, which now prints the limit it derives (U-234
1.5 keV, U-235 2.25 keV, U-238 20 keV).

**And a second-order consequence of that guard, found the same day:** because
neither code broadens above `thnmax`, a shared-grid comparison against NJOY at
293.6 K that does *not* split at the limit measures only the **unbroadened**
table — on all three uranium nuclides, **zero** shared grid points fall below
it. This port's Doppler broadening is therefore **not verified against NJOY at
all**, despite a 293.6 K comparison that reads as ~1e-6 agreement. See
`crates/njoy-outram-park-fork/verification_and_validation/acer_ce_vs_njoy2016_multi_nuclide.md`.

