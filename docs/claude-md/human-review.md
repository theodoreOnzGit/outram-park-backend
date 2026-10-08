# Human review is the bottleneck: make it cheap (index of practices)

**Maintainer direction, 2026-10-05.** AI output in this workspace is untrusted
draft material until a human reviews it (`RESPONSIBLE_USE.md`, `AI_USAGE.md`).
AI can produce work much faster than one maintainer can review it, so human
review time is the scarcest resource here. Every practice below exists to spend
less of it per finding, or to make sure a finding is not lost.

This file is an **index**. Each practice is owned by the file it links to,
which stays the authority. Add a row here when a new review practice is
adopted, and keep the rule itself in its owning file.

## 1. Before the human looks: prepare the material

| Practice | What it saves the reviewer | Authority |
|---|---|---|
| **Build the picture before the code is read.** An interactive view driven by the library's own calls, showing the whole population at once, with every modelling choice drawn, zeros included | Finding a wrong *model* from one look instead of reading every function; a faithful port's inherited simplification is invisible to tests (gh:#583) | [`visual-review-first.md`](visual-review-first.md) |
| **Draw reactor geometry as the solver sees it** (HARD RULE): slices from the assembled geometry, at every nesting level, coloured by material | Catching geometry defects that every run reports as green (the 2026-09-24/25 HTR-10 cases) | [`crates/dhoby-ghaut/CLAUDE.md`](../../crates/dhoby-ghaut/CLAUDE.md), and every geometry crate's `CLAUDE.md` |
| **V&V docs state methodology AND results**, with uncertainty, date and interpretation | Judging a check without re-running it | Workspace [`CLAUDE.md`](../../CLAUDE.md), "Verification & validation documentation" |
| **Predictions are written before the run**: sign and magnitude, committed first | Telling a fitted answer from one that could have failed | Workspace `CLAUDE.md`, "Debugging a port", step 4; [`process-and-porting-lessons.md`](../claude-md-rationale/process-and-porting-lessons.md) |
| **Report misses and say which numbers to quote** | Not having to dig out the number that counts, or the unflattering one | Workspace `CLAUDE.md`, "Get the PROCESS right first" |
| **Inputs carry their citation**, and borrowed values are named as borrowed | Checking provenance without hunting for it | Workspace `CLAUDE.md`, "Model hierarchy" and "Data provenance"; each crate's `docs/References.md` |

## 2. When the human looks: present it so it reads fast

| Practice | What it saves the reviewer | Authority |
|---|---|---|
| **Code walks show the code inline**, entry point to concept, generated and checked by a tool | Clicking through to GitHub, and trusting a hand-written chain | [`docs/lessons/lesson-philosophy.md`](../lessons/lesson-philosophy.md), section 4; `kovan-cli code-walk` |
| **Lesson pages carry a review stamp and a "doesn't tally" button** | Knowing what has been reviewed, and reporting a mismatch in one step | `lesson-philosophy.md`, section 7 and the checklist |
| **Demos work on a phone and never lag** | Reviewing anywhere, without waiting on the UI | [`mobile-first-tutorials-and-demos.md`](mobile-first-tutorials-and-demos.md) |
| **The Haiku API test** before calling an API usable | A cheap model fails where a human would stumble, before the human does | [`api-design-and-maturity.md`](api-design-and-maturity.md) |
| **Corrections are struck through, never silently replaced** (`~~old~~ **CORRECTED <date>**`) | Seeing what changed and why, and trusting the rest | Workspace `CLAUDE.md`, "A doc claim contradicted by the code is a DEFECT" |
| **Review manifests for agent-fleet changesets**: what changed, what was run, what is untrusted | Reviewing a large AI changeset from one page | Precedent only, not yet a written rule: `crates/outram-mc-libs/docs/ai-fleet-review/*/REVIEW_MANIFEST.md` |

## 3. After the human looks: keep what the review found

| Practice | Why | Authority |
|---|---|---|
| **Only the maintainer flips a crate's Bookkeeping status** (V&V, and human or user interface) | The axes record *human* review; an AI-flipped tick is worthless | [`bookkeeping-and-api-docs.md`](bookkeeping-and-api-docs.md) |
| **Log what only a human caught** in the corrections file, with the check that would have caught it | Turning a war story into a standing check; its "Recurring failure modes" list is a pre-review checklist | [`docs/human-corrections-to-ai-work.md`](../human-corrections-to-ai-work.md) |
| **Findings go to an issue**, and closures are proposed with evidence, never made by the AI | Nothing the review raised is lost; the human decides what is done | Workspace `CLAUDE.md`, "Issue tracking" |
| **Review stamps** in `review/stamps.toml`: one per function a human reviewed (rung 3) or V&V-checked (rung 4), hashed over its code and `///` docs. Rung 5 is derived, never stamped: ~~a second, independent, qualified reviewer's stamp~~ **CORRECTED 2026-10-08** (GitHub #809) independent V&V (IV&V) in NUREG/BR-0167's sense, a hand-written V&V case by a qualified reviewer whose registry organisation is "both technically and managerially separate" from the developing organisation, with a signed separation attestation whose audit record is a public GitHub issue (`kovan_common::review::ivv`); every reason a function misses it is shown. `kovan-cli stamps-check [--diff A..B]` lists exactly which stamps a change voids | A review is not silently carried over to code that has changed since; re-review is asked for function by function | [`crates/kovan/src/review_stamps/`](../../crates/kovan/src/review_stamps/mod.rs), GitHub #739 |

### Review stamps: AI never stamps (HARD RULE)

A stamp records that **a human** reviewed the code. AI agents never create,
edit or delete a stamp, never run `kovan-cli stamp` or
`review_stamps::stamp_function`, and never pass `--i-am-the-reviewer`,
whoever asks. Agents may run `kovan-cli stamps-check` and `stamps-levels`,
and must report a VOID stamp their change causes rather than work around it.
Stamping is done by the maintainer in desktop kovan (#740).

## What none of this replaces

These practices make human review cheaper; they do not substitute for it. No
picture, walk, test or manifest turns AI output into trusted output without a
human's review (`AI_USAGE.md`, "Required Human Review").
