# `bwr_mark1`: perturbation ensemble (gh:#1044)

> Research, education and V&V only. AI draft, not reviewed. An instrument
> for reading the model's own numbers; no input is chosen from it (plan
> rule 1: calibration only as the last step).

## Why

The fourth input-sensitivity run (`input-sensitivity.md`) found the run
**chaotic at the 1e-9 level**. A relative perturbation of 1e-9 in one input
(the pool's water) moves the lower head's failure time by 6 %. The run is
deterministic and bit-reproducible, but a single run's head time, H2 or
drywell peak is one trajectory of a system with sensitive dependence on
its inputs, not a converged answer. The pinned baseline in
`compare::the_uncalibrated_baseline_is_pinned` stays as a regression check
on the code, not as the model's result.

## Methodology

- **What is computed.** `compare::ensemble_member(k, rel, seed)` runs
  member `k`:
  - every one of the 183 inputs in `Params::INPUTS` is scaled by
    `1 + rel u`, with `u` uniform in [-1, 1] from a SplitMix64 hash of
    (seed, member, input index);
  - every mechanism is on;
  - the metrics are `compare::measure_with`'s: lower-head failure,
    depressurisation, in-vessel H2 at 25 h, drywell peak over 0–25 h and
    the vent's opening.

  `compare::ensemble_report` gives each metric's minimum, nearest-rank
  quartiles, median and maximum.
- **Run.** `cargo run --release -p sembawang --example bwr_mark1_ensemble -- <n> <rel> <seed>`.
  The members are independent and deterministic, so the result does not
  depend on the thread count.
- **Checks.** `compare::tests::the_ensemble_reproduces_the_default_and_reports_quartiles`:
  - a member with `rel = 0` equals the default run exactly;
  - the quartiles match a hand case.
- **Why this instrument.** It samples the branches the model itself can
  take under perturbations far below any input's uncertainty. It needs no
  input ranges, so it can neither calibrate nor smuggle in a choice. It is
  **not** an uncertainty analysis over the inputs' real uncertainties,
  which would be wider.

## Predictions (written 2026-10-30, before the first ensemble)

1. The lower head fails between about 4.4 and 6.0 h, multimodal, and the
   baseline's 5.238 h falls inside the interquartile range.
2. In-vessel H2 spreads over 230–480 kg, with a median of 350–450 kg.
3. The drywell peak lies in 0.75–0.87 MPa.
4. The vent opens at 23.73 h (the order) in every member.

## Results (2026-10-30, physics of `442bac1f5`, `rel` = 1e-9)

| Ensemble | Metric | min | 25 % | median | 75 % | max |
|---|---|---|---|---|---|---|
| seed 1, 64 | lower head, h | 4.649 | 5.024 | 5.234 | 5.312 | 5.454 |
| seed 2, 64 | lower head, h | 4.502 | 4.653 | 5.233 | 5.237 | 5.565 |
| **seed 3, 256** | **lower head, h** | **4.500** | **4.914** | **5.234** | **5.242** | **5.559** |
| seed 1, 64 | H2 at 25 h, kg | 402.9 | 419.4 | 449.0 | 449.5 | 481.0 |
| seed 2, 64 | H2 at 25 h, kg | 307.4 | 405.9 | 448.5 | 449.3 | 480.6 |
| **seed 3, 256** | **H2 at 25 h, kg** | **304.7** | **417.0** | **448.7** | **449.4** | **480.9** |
| seed 1, 64 | drywell peak, MPa | 0.791 | 0.804 | 0.808 | 0.889 | 0.964 |
| seed 2, 64 | drywell peak, MPa | 0.782 | 0.804 | 0.808 | 0.955 | 0.962 |
| **seed 3, 256** | **drywell peak, MPa** | **0.782** | **0.804** | **0.809** | **0.918** | **0.965** |

In every member the vent opened at 23.7336 h, the lower head failed, and
the building did not explode. The three medians agree to 0.001 h, 0.5 kg
and 0.001 MPa. The 64-member quartiles do not agree between seeds: the
distribution is bimodal, and 64 members do not resolve the weight of its
early branch. The 256-member run is the one to quote.

**Predictions against results:**

| | Prediction | Result |
|---|---|---|
| 1 | head 4.4–6.0 h, baseline inside the IQR | held in part: range 4.50–5.56 h, narrower, and the baseline 5.238 h is inside the IQR (4.914–5.242 h) |
| 2 | H2 230–480 kg, median 350–450 kg | held: 305–481 kg, median 448.7 kg |
| 3 | peak 0.75–0.87 MPa | missed: the upper quarter reaches 0.92–0.97 MPa |
| 4 | vent at 23.73 h in every member | held |

## What to quote, and why

Quote **the 256-member distribution**, not the single run:

- **lower-head failure:** median 5.23 h, interquartile 4.91–5.24 h,
  range 4.50–5.56 h;
- **in-vessel H2 at 25 h:** median 449 kg, interquartile 417–449 kg,
  range 305–481 kg;
- **drywell peak:** median 0.809 MPa, interquartile 0.804–0.918 MPa.

The single-run numbers elsewhere (the baseline pin, the plan's table,
the BSAF-figure comparisons) are one member. Read their movements under
a mechanism as indicative only when the movement is larger than this
spread. A move smaller than the spread can be a branch change, not
physics.

Against BSAF: the nine codes give 398–1315 kg of H2, so the model's
interquartile range (417–449 kg) lies at the low end of their band.
