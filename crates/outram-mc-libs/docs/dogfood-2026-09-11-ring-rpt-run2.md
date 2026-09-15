# Haiku dogfood, run 2 — after the `ExplicitTrisoPebble` fix

A/B against [run 1](dogfood-2026-09-11-ring-rpt.md). Protocol fixed **in writing
before any result** so the comparison could not be tuned to fit the outcome.

## What was held fixed, and what changed

Held fixed: model (Haiku), fresh agent with no prior context, an isolated
directory containing only `docs/outram-mc-libs-api.md`, the same five-part task
worded identically, and the same prohibitions — no crate source, no filesystem
search for it, **no compiler**. The agent was told nothing about run 1 or that an
API had been added in response to it.

Changed: `ExplicitTrisoPebble` exists, and the mirror describes it.

## The primary measure

Fixed in advance as a yes/no, because that is the question the API change was
made to answer: **does the agent complete the explicit-TRISO pebble?**

| task | run 1 | run 2 |
|---|---|---|
| 1. FHR materials | ✅ | ✅ |
| 2. ring-RPT pebble | ✅ | ✅ |
| **3. explicit-TRISO pebble** | ❌ **could not** | ✅ **completed** |
| 4. delta-tracked k-eff | ✅ | ✅ |
| 5. six-factor decomposition | ✅ | ✅ |

Verified in the file, not taken on the agent's word: `attempt.rs` imports and
calls `ExplicitTrisoPebble::new`, `TrisoMaterials` and `.material_at(p)`.

**Counts are secondary and weak** (run 1: 11 round-trips / 7 wrong guesses;
run 2: 6 / 3). One sample each, different agent instances, and run 1's own record
warns against reading much into count deltas. The yes/no is the result.

## What the run found that nobody was looking for

The agent is forbidden to compile. Compiling its output **afterwards** is
therefore a separate measurement — the gap between "correct from the
documentation" and "actually builds". Six errors, of which **two were real API
defects**:

```
error[E0603]: struct `TrisoMaterials` is private
error[E0603]: enum `ComputeType` is private
```

The first was **mine, landed hours earlier**. `ExplicitTrisoPebble::new` takes a
`TrisoMaterials` and sits beside it in `pebble_beds::fhr_pebble` — but only as a
private `use`. The agent wrote the obvious import and it did not compile. *An API
that cannot be called from the module it is documented in is not callable.*
`KeffSettings { compute: ComputeType }` had the identical defect in
`physics::keff`, pre-existing.

**Why the acceptance test could not have caught it.** Every in-repo caller lives
*inside* the crate, where module privacy does not bite. The bit-identical k-eff
(1.31813 ± 0.00210) and 316 green tests were both true and both blind to it. It
took an external consumer, and the dogfood is the only thing in this workspace
that acts like one.

Fixed in `ed8db6de`; the mirror now states the import line outright.

The remaining four errors were the agent's own slips — `ln` on an ambiguous
float (×3), and a closure returning `usize` where `Option<usize>` was wanted.
Not API defects, and kept distinct rather than folded in to inflate the finding.

## Two deviations, recorded rather than smoothed over

- **The agent did not write `FINDINGS.md`** as the task instructed; it put the
  log in its final reply instead. This document is transcribed from that reply.
- **A sonnet agent's worktree was edited while it was still running**, and its
  branch committed, by the coordinator. The agent correctly flagged both as
  suspicious and guessed a parallel agent was responsible. It was not — it was
  the coordinator. Authorship of the `TrisoMaterials` test helper is genuinely
  ambiguous as a result. The workspace rule against this already existed.

## The finding both runs agree on, still open

Run 1 asked for "a worked CSG construction example". Run 2 independently ranked
an "FHR Pebble Quickstart" workflow section HIGH, observing that building a
pebble means composing five sibling submodules — `crp_packing` →
`sphere_packing` → `fhr_pebble` → `delta_tracking` → `keff_delta` — with nothing
anywhere showing the pipeline end to end.

**Two independent agents converging on the same gap is the strongest signal this
exercise produces.** It is the next thing to fix.

Run 2's other open items, in its own ranking: undocumented closure trait bounds
on `run_keff_delta_in<F>`; `Estimate` used in `SixFactors` output but never
defined; no realistic TRISO radii example; no guidance on whether
`Material::temperature` and `KeffSettings::temperature_k` must agree.
