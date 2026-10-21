# #496 — source-convergence diagnostics: V&V protocol (stub)

**Issue:** [gh:#496](https://github.com/theodoreOnzGit/outram-park-backend/issues/496), epic #493.
**Code:** `src/stats/convergence.rs` (`SourceConvergence`, `TraceDiagnosis`);
`raffles::estimators::stationarity` (MSER-5, Geweke, change-point).

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## What is being checked

That the diagnostic (a) flags non-convergence while the fission source is
still relaxing, (b) passes once the entropy trace is flat, and (c) recommends
an inactive count that agrees with a long reference run.

## Methodology

- **Deliberately bad initial source.** A case whose initial source is
  concentrated in a corner of the fissile region (a `SourceBox` covering a
  small fraction of the core), with an entropy mesh, run with many inactive
  generations. Candidate: the HTR-10 core (`nee_soon`), where the reference's
  5 inactive cycles are flagged as a possible bias; a cheaper Godiva-sized
  case first.
- **Truncated replicas.** Feed `SourceConvergence::from_traces` the first
  `G` generations for increasing `G`: it must report "NOT converged" while the
  entropy still drifts, and "converged" once it is flat.
- **Reference.** A long run (≥ 10× the recommended count) whose entropy is
  judged flat by eye on the plotted trace **and** by the diagnostic; the
  recommended count from a short run must not be smaller than the point at
  which the long run's entropy enters its final band.

## Pass criteria (fixed in advance, 2026-10-03)

- On the drifting prefix: `recommended_inactive == None` and the entropy
  trace's MSER optimum in the second half or Geweke `|z| > 1.96`.
- On the settled run: `recommended_inactive == Some(r)`, and `r` is at least
  the long reference run's settling generation minus one MSER batch (5).
- Report-only: the transported `k` of every run is bit-identical with and
  without the diagnostic (it reads a finished `KeffResult`).

Fixed constants (not tunable): MSER batch 5; Geweke windows 10 % / 50 %;
`|z|` critical 1.96; change-point on batch means of 5.

## Results

| case | date | generations | recommended inactive | reference settling gen | pass |
|---|---|---|---|---|---|
| bad-source case | — | — | NOT YET MEASURED | | |
| HTR-10 | — | — | NOT YET MEASURED | | |
