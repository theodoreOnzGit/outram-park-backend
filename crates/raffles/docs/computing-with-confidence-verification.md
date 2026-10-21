# Computing with Confidence — verification record

**Status: AI-assisted draft. Not reviewed by a human or by the crate owner
(Adolphus Lye).** Tracking issue: gh #987. Date of the measurements: 2026-10-20.
Education, research and V&V only; see the workspace `RESPONSIBLE_USE.md`.

## What was ported

Adolphus Lye's "Coherent systems" scripts
(<https://github.com/Adolphus8/Computing-with-Confidence>, accessed
2026-10-20): the illustrative example and Case studies 1-4. They propagate
confidence boxes (c-boxes) through fault-tree AND/OR logic, under independence
and with unknown dependence (Fréchet), using Scott Ferson's `pba BETTER.r`
(upstream <https://github.com/ScottFerson/pba.r>, GPL-3.0).

| Rust | What | Provenance |
|---|---|---|
| `src/scram/pba.rs` | `kn`, `kn_exact`, `and_unknown`, `or_unknown`, `and_independent`, `or_independent`, `complement`, `envelope`, `frechet_convolve`, `independent_product`, `clamp_pbox`, `point` | port of pba.r `KN`, `and`, `or`, `andI`, `orI`, `complement.pbox`, `env.pbox`, `frechetconv.pbox`, `conv.pbox`; GPL-3.0 header |
| `src/scram/pbox_tree.rs` | `evaluate_pbox_tree` (gate by gate, dependence per gate), `evaluate_cbox_edges` (left/right edge construction) | original; follows the case-study scripts |
| `tests/computing_with_confidence.rs`, `tests/computing_with_confidence_mc.rs` | V&V | original |

**`sra.r`** (the same folder, "All rights reserved") was **not read, not
ported and not run**. The R driver removes it from the working directory.

**Reused, not duplicated:** `imprecise::Pbox` (the quantile ladder; no parallel
p-box type), `imprecise::cbox_binomial`, `imprecise::EventDependence`,
`imprecise::Interval`, `Pbox::cut`, `scram::fault_tree::{FaultTree,
FaultTreeBuilder, Connective}`. **Checked and rejected:**
`SystemStructure::reliability_pbox` combines boxes at matched confidence level
only (comonotone), which is not pba.r's Fréchet/independent convolution, so it
cannot reproduce the scripts; `imprecise.rs` was therefore left unchanged
(apart from a corrected doc paragraph) and the new operations live beside the
fault tree. `Bdd::probability` takes point probabilities, so it cannot carry a
c-box.

## Does pba.r's `KN` equal `cbox_binomial`?

In the interior, yes (asserted level by level in `pba::tests`). Two differences
found by reading `KN`:

1. pba.r uses the exact support ends `qbeta(0) = 0`, `qbeta(1) = 1`;
   `cbox_binomial` evaluates the quantile at `1e-9` / `1 - 1e-9`, which for
   `Beta(23, 2)` is about 0.3 instead of 0 at that single end level.
2. `KN` clamps to `[1e-6, 1 - 1e-6]` (pba.r `Bzero`); the Case study 4 edges
   `c_L`, `c_R` are built with `beta()` and are **not** clamped.

`pba::kn_exact` is (1); `pba::kn` is (1) plus (2). `cbox_binomial` is unchanged.
The Case study 4 `e1f` comparison failed by exactly `1e-6` until the unclamped
edges were fed to the edge construction, which is the script's behaviour.

## Primary reference: the original R scripts

**Tool.** R version 4.6.1 (2026-06-24), base R only, no CRAN packages.
**Driver.** `verification_and_validation/computing_with_confidence/r_inputs/run_case.R`
stubs only the interactive graphics (`getGraphicsEvent`, `keyPressed`), opens
`pdf()`, evaluates the script as written, and prints every `interval_*` object.
Regenerate with `r_inputs/regenerate.sh <workdir>` (command lines inside); the
small outputs are committed as `r_inputs/*.r_output.txt` and are read by the
tests with `include_str!`.

Two scripts were run in a modified form, both committed in `r_inputs/`:

- `case3_pba_part.r`: Case study 3 lines 1-38 (up to `source("sra.r")`) plus
  three added lines printing the 95 % interval of `R_S1_ps_Boolean`.
- `case4_cboxes.r`: Case study 4 with lines 34-46 deleted. **The published
  script defines the nine c-boxes with `KN`, then redefines `c1..c9` as point
  values** (`c1 = (1/100)` ...), so as published its independent events and
  all its non-top Fréchet events are point values (e.g. `interval_e1i` is
  `[6.58e-05, 6.58e-05]`). The comments and plots indicate the c-boxes were
  intended; the as-published outputs are kept in `case4.r_output.txt`. The
  top event `e1f` (built from separate Beta edges) is identical in both runs:
  `[0, 0.0682080449372]`.

**Method.** The Rust test builds the same c-boxes (`KN(k, n)`, 200 steps =
201 levels; 1000 steps for Case study 4), evaluates the same tree with
`evaluate_pbox_tree` (unknown dependence = pba.r `and`/`or`, independent =
`andI`/`orI`) and compares the 95 % cut `[lower at 2.5 %, upper at 97.5 %]` with
`interval_*`.

**Pass criterion (fixed from the method before the runs):**
`|rust - R| <= 1e-9 + 1e-6 |R|`. Both codes perform the same arithmetic on the
same quantile cells (sorted cell products; min/max over grid pairs), so only
the Beta quantile routine and rounding can differ.

**Results.** 39 intervals (78 end points), all passing; worst absolute
difference `4.57e-13`.

| Case | Compared | Reference | Result |
|---|---|---|---|
| Illustrative (series, parallel; indep and Fréchet) | 4 intervals | R 4.6.1 `interval_ser_i/f`, `par_i/f` | pass |
| Case study 1, pressure tank | E1-E5, indep and Fréchet (10) | R `interval_e?i/f` | pass |
| Case study 2, TRIGA | E1-E4, indep and Fréchet (8) | R | pass |
| Case study 3, bridge, `pba` Boolean route | 1 | R `interval_R_S1_indep` | pass |
| Case study 4, radiation release | E2a, E2b, E3a-d, E4 indep and Fréchet; E1 indep (15) | R `case4_cboxes` | pass |
| Case study 4 top event, edge construction | 1 | R `interval_e1f` | pass |

Example values (95 % interval): illustrative series, independent
`[0.4926209336, 0.9417456539]`, Fréchet `[0.2934656193, 0.9620149319]`;
Case study 1 E1, independent `[1.2400563e-3, 2.1875889e-2]`, Fréchet
`[1.2390575e-3, 5.1436833e-2]`; Case study 4 top event, Fréchet edges
`[0, 6.8208045e-2]`.

## Secondary reference: Octave Monte Carlo (independence)

`octave_inputs/indep_mc_crosscheck.m`, GNU Octave 11.3.0,
`octave --no-gui indep_mc_crosscheck.m`, output committed. The original
Supplementary `.m` files could not be run as written: `betarnd` is in
Octave's `statistics` package (not installed, none is installed system-wide)
and Octave needs functions defined before use. The script follows their
structure with `Beta(a,b) = X/(X+Y)` from gamma draws. The Pressure-vessel
`.m` uses `KN(2, 2000)` for the tank where the R script uses `KN(0, 2000)`; the
R value is used so all three codes compute the same quantity. Case 3 `.m`
(shared-sample structure function) was not run: it is a different quantity from
the `pba` Boolean route (shared draws versus independent copies).

The pba cut is an **outer** bound (cells round outward at every operation), so
the test checks enclosure of the Monte Carlo end points (4 replicate standard
deviations of slack, from 20 replicates of 10^5 draws) and convergence as the
ladder is refined. Measured upper 97.5 % ends:

| Case, ladder (steps) | pba upper | Monte Carlo (mean, sd) |
|---|---|---|
| Case 1, 200 / 400 / 800 / 1600 / 3200 | 2.1876e-2 / 1.9601e-2 / 1.8707e-2 / 1.8357e-2 / 1.8197e-2 | 1.8048e-2 (4.8e-5) |
| Case 2, 200 / 400 / 800 / 1600 / 3200 | 5.2068e-2 / 5.0588e-2 / 4.9881e-2 / 4.9539e-2 / 4.9370e-2 | 4.9188e-2 (1.9e-4) |

Lower ends: Case 1 `1.24006e-3 -> 1.24146e-3` (MC `1.24452e-3`, sd `9.6e-6`);
Case 2 `1.3648e-4 -> 1.9130e-4` (MC `1.9406e-4`, sd `5.0e-6`). All five ladders
enclose the Monte Carlo interval and the excess shrinks at every doubling.
**Interpretation:** the 200-step R/pba numbers are conservative by 21 % (Case 1
upper end) and 6 % (Case 2) over the independence Monte Carlo; this is a
property of pba's 200-step discretisation, reproduced faithfully, not a port
error, and it is visible only because the second reference exists.

## Analytic and structural checks (unit tests)

Precise inputs reduce to the plain formulas (independent AND 0.18 and OR 0.72
for 0.3 and 0.6; Fréchet `[0, 0.3]` and `[0.6, 0.9]`; a three-event tree gives
0.314) exactly to 1e-12; `complement` is an involution to 1e-14; Fréchet boxes
enclose the independence boxes at all 201 levels; repeated events under
independence are refused; mismatched ladders, negative support and `Xor` are
rejected; Nand/Nor are exact complements; 2-of-3 under Fréchet encloses 0.5.

## What is NOT verified

- Case study 3's second half (structure function versus Boolean Monte Carlo)
  sits behind `source("sra.r")`, which is all-rights-reserved and was not
  used. The bridge structure-function route is **not** implemented; only the
  `pba` Boolean route is, and that treats the repeated components as
  independent copies (`RepeatedEvents::IndependentCopies`). It is not the
  exact structure-function answer, and the default refuses it.
- Case study 4's outputs as published (point values after the overwrite) were
  not reproduced, deliberately; see above.
- The Fréchet results are checked against R, not against an independent
  derivation of the tightest possible bound (grid-pair Fréchet bounds are valid
  but not necessarily tight).
- Accuracy against the exact answer beyond independence Monte Carlo on Cases 1
  and 2; no reference exists for Fréchet in Octave.
- The edge construction is justified for c-boxes (Lye et al. 2024) and is not
  proved here for arbitrary p-boxes.
- Atleast gates under Fréchet are expanded to OR-of-ANDs and only sanity-checked.
- Numbers published in the papers (doi:10.1061/AJRUA6/RUENG-1380) were not
  compared: not read from an open source.
- PETIR routing (gh #985): the new code uses no transcendental functions
  (`+ - * min max`, sort); the only ones involved are the Beta quantiles inside
  the existing `cbox_binomial`.

## Not for use in

Facility operation, licensing or safety decisions. Draft research code.
