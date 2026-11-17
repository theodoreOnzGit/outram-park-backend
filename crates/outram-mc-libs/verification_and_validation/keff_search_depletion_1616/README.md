# k search coupled to depletion (gh:#1616)

Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for design,
licensing or operation. The case is a cold, clad-free, CORE-tier pin cell: its
critical boron is **not** a PWR number and must not be quoted as one.

Code: `src/depletion/keff_search.rs`
(`deplete_independent_with_keff_search`, `KeffSearchControl`).
Gate: `tests/keff_search_depletion_1616.rs`. Unit tests: `keff_search::tests`.

## Upstream, as read

OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
(`crates/outram-mc-libs/upstream_source/OpenMC/`):

| file | lines | what |
|---|---|---|
| `openmc/deplete/keff_search_control.py` | 7-134 | `_KeffSearchControl`: validates `bracket` (2 values, `[0] < [1]`, `:32-35`); `run(x)` pushes `x` into the materials, searches, writes the found compositions back into `x` (`:44-65`, `_update_vec` `:106-134`); raises `ValueError` if the search did not converge, warns if the root is outside the bracket (`:85-99`) |
| `openmc/deplete/abc.py` | 1106-1199 | `Integrator.add_keff_search_control(function, x0, x1, bracket, **kw)` |
| `openmc/deplete/abc.py` | 883-900, 924-985 | the hook runs at the **beginning of each step**, before the operator is evaluated, when `source_rate != 0`, and once more before the final operator evaluation |
| `openmc/model/model.py` | 3248-3405 | `Model.keff_search`: GRsecant (Price and Roskoff), seeds `x0`, `x1`, weighted-linear-fit secant over 4 points, clamped to `[x_min, x_max]`, adaptive batches; stops when `|k - target| <= k_tol` (1e-4) and `sigma <= sigma_final` (3e-4); `maxiter` 50 |

What it adjusts: whatever the user's function does to `openmc.lib` (a cell
translation, a nuclide density, ...). Only **burnable** nuclides found in the
searched material are written back into the depletion vector, so a boron
search changes the transport solves but not the depleted inventory.

## Differences in this port (all in the module doc table)

- ~~The search is the workspace's `search_for_keff` (bracketed bisection or false
  position), **not GRsecant**, which the workspace does not have. It needs a
  straddling bracket: `(x0, x1)` first, then the absolute bracket once, then an
  error. There is no `sigma_final` test and no adaptive batch count.~~
  **CORRECTED 2026-11-17 (gh:#1618)**: the default search is now GRsecant
  (`physics::search_grsecant`, `model.py:3248-3405`), with `sigma_final` and an
  adaptive batch count; the text above describes the #1616 run, which is kept
  and is selected by name (`KeffSearchControl::with_search_settings`). See
  "2026-11-17, #1618 item 1" below.
- Refused for stochastic-implicit integrators (upstream's SI loop never calls
  the hook). No restart restore (no restart files here).
- It composes with the **independent** one-group operator. Frozen cross
  sections mean the parameter reaches depletion only through the caller's
  `adjust`. For boron the burnup is bit-identical to `deplete_independent`.
- `search_for_keff`, `deplete_independent` and the other drivers are not
  edited. No kovan-reviewed function was touched.

## Bit-identity without the hook

The existing drivers are untouched, so without the hook they are what they
were; `tests/depletion_move_fingerprints.rs` is their pin. In addition,
`keff_search::tests::a_parameter_that_changes_no_burnable_nuclide_leaves_the_burnup_unchanged`
asserts, for Predictor, CE/CM, CF4, CE/LI, EPC-RK4 and LE/QI, that the hooked
loop with a no-op `adjust` reproduces `deplete_independent` bit for bit, and
the V&V gate below repeats that on the real case.

## Case

Reflective pin cell, fuel radius 0.4096 cm, half-pitch 0.63 cm, no clad, water
1.0 g/cm3 (H-1 6.69e-2, O-16 3.345e-2 per b cm), 293.6 K, CORE data
(`Nuclide::from_core`: H-1 free gas, no S(a,b)). Fuel 4.25 wt% UO2, U-235
9.8e-4 and U-238 2.222e-2 per b cm, O-16 at twice the uranium. Burnable
inventory: `DepletionChain::simple()` (nuclides CORE lacks are skipped).
Depletion: independent operator, single-energy 0.0253 eV cross sections,
Predictor, CRAM16, 174 W in 0.5542 cm3, 4 steps x 50 d (about 7 GWd/tU).
Parameter: natural boron in the water, ppm by mass, B-10 only (19.9 %, B-11
not in CORE). Search: false position on [0, 6000] ppm, guesses (0, 6000),
`k_tol = 0.005`, parameter tolerance 25 ppm, at most 12 iterations. Transport
per solve: 2500 histories x (8 inactive + 25 active), fixed seed.

## Predictions (written before the first run, in the test's doc comment)

1. Every search converges in the bracket. 2. k within `k_tol` of 1. 3. Boron
non-increasing with burnup. 4. Drop 300 to 900 ppm, largest step 0 to 1 (Xe).

## Process note: the first run, kept

The first run used a **smaller** design (900 histories x (8 inactive + 20
active), 4 x 20 d, bracket [0, 4000], `k_tol` 0.01) and **failed gate 3**: boron
3945, 3735, 3793, 3458, 3629 ppm. Its sigma_k was 0.005 to 0.009, equal to
about 100 to 200 ppm, the size of the burnup signal in 80 d, so it could not
resolve the trend and was not evidence for or against it. The design was
changed to more histories, tighter `k_tol` and 50 d steps, and the gate was
rewritten to size its tolerance from the search's own noise rather than a
fixed 50 ppm. Both were decided from that noise, not from the second run's
output; the second run's sign was not known before it ran. The critical boron
there sat 50 ppm below the old bracket's upper end, which is why the bracket
was widened to 6000.

## Results (2026-11-17)

Hardware: Intel Xeon @ 2.10 GHz, 4 logical cores, 1 thread used (single-thread
CPU driver), 15 GB RAM, Linux 6.18.44, CPU only; **shared and heavily loaded**
(load average 3 to 9 on 4 cores, other agents running transport). Wall time
**176.9 s** for 5 searches and 30 transport solves; hardware as above, so only
a rough size.

| search before step | burnup time [d] | critical boron [ppm] | k at root | sigma_k | solves |
|---|---|---|---|---|---|
| 0 | 0 | 4014.8 | 0.99663 | 0.00327 | 5 |
| 1 | 50 | 3642.2 | 0.99745 | 0.00340 | 7 |
| 2 | 100 | 3454.0 | 0.99807 | 0.00426 | 6 |
| 3 | 150 | 3270.0 | 0.99849 | 0.00367 | 7 |
| final | 200 | 3029.2 | 1.00308 | 0.00331 | 7 |

Sensitivity read from the search endpoints: k(0 ppm) = 1.489 at BOL falling to
1.433 at 200 d; k(6000 ppm) = 0.862 falling to 0.783. Chord slope about
-1.0e-4 per ppm at BOL; local slope near the root about -5.4e-5 per ppm, so
sigma_k of 0.0033 is about 61 ppm locally.

Gates, all pass (`cargo test --release -p outram-mc-libs --no-default-features
--test keff_search_depletion_1616`):

- a search ran before each of the 4 steps and at the end (5);
- `|k - 1| <= k_tol + 3 sigma` at every search (largest `|k - 1|` is 0.0031,
  inside `k_tol` = 0.005 itself);
- no search needed the absolute-bracket fallback;
- the burnup (`k_inf` and U-235, every step) equals `deplete_independent`
  bit for bit;
- boron is monotone non-increasing with burnup (every step falls: -373, -188,
  -184, -241 ppm), and the total drop of **986 ppm** exceeds 3 sigma of the end
  searches.

## Against the predictions

1. Met. 2. Met (all five inside `k_tol`). 3. Met, every step falls.
4. **Partly missed:** the drop is 986 ppm, above the predicted 300 to 900; the
largest step is 0 to 1 (-373) as predicted. The prediction was an order of
magnitude from an assumed worth of 10 pcm/ppm. The local worth near the root
is about 5.4 pcm/ppm (step 0: k 0.9486 at 4684 ppm, 0.9728 at 4238 ppm), half
the assumed value, and the fixed-boron k loss over 200 d is 0.056 (1.489 to
1.433 at 0 ppm), so the drop is about 0.056 / 5.4e-5 = 1000 ppm. The error
was in my worth estimate, not in the search.

The chord slope over [0, 6000] (-1.0e-4 per ppm) overstates the local slope,
so the gate's noise estimate (sigma_k over the chord slope, about 33 ppm) is
about half the local value (about 61 ppm). The gate is tighter than the truth
there; with the local value the allowed single-step rise is about 260 ppm and
the 986 ppm total drop still clears 3 sigma (about 260 ppm).

## What this does and does not show

- It shows the hook is wired where upstream puts it, converges within
  `k_tol` at every step, and returns a monotone parameter trajectory on a case
  where the physics requires one.
- The absolute boron (about 4000 ppm) is **not physical**. k(0 ppm) = 1.49
  comes from this model: no clad, cold water, H-1 free gas without S(a,b),
  CORE-tier data, frozen single-energy depletion cross sections. Nothing here
  is validated against a measurement or against OpenMC's own coupled run.
  Not compared with OpenMC numerically: no OpenMC run was made.
- Boron does not feed back into depletion (frozen cross sections), so this
  is the trajectory of the parameter, not a coupled spectrum-shift study. The
  enrichment-type path (parameter changes a burnable nuclide) is exercised
  only with a synthetic k in the unit test, not with transport.
- sigma_k of 0.003 to 0.004 per solve is the resolving power. A 25 ppm bracket
  tolerance is below it, so the search stops on the residual criterion.
- One seed per solve; no ensemble over seeds.

## 2026-11-17, #1618 item 1: GRsecant is the default search

Code: `src/physics/search_grsecant.rs` (`grsecant_search_for_keff`), selected by
`KeffSearchControl::new` (the default) or `with_grsecant_settings`. The
bracketed search above is `with_search_settings`, unchanged. Upstream read at
`8f79d9d0`: `openmc/model/model.py:3248-3405` (`Model.keff_search`, GRsecant of
Price and Roskoff, doi:10.1016/j.pnucene.2023.104731), and
`openmc/deplete/keff_search_control.py:41-42, 79, 85-99` for how the hook sets
`x_min`/`x_max` from the bracket, calls it and raises on non-convergence. The
helper it calls, scipy `curve_fit`, is replaced by the closed-form weighted
least squares that has the same minimiser (weights `1/sigma^2`). Differences
from upstream are in the module doc table; the one that matters here is that
the model closure now receives the active-batch count, which the workspace's
k-eigenvalue driver supports through `KeffSettings::n_active`, so **adaptive
batches are ported**.

Unit tests: `physics::search_grsecant::tests` (10) and `keff_search::tests`
(the hook with GRsecant on a synthetic `k(p)` with `sigma ~ 1/sqrt(b)`).

### The case, same inputs

Seeds `(0, 6000)` ppm (the old bracket's ends), clamp `[0, 6000]`, `k_tol =
0.005` (as before), `sigma_final = 0.004` (the size of the old `sigma_k`), `b0 =
25` active, 8 inactive, `b_min = 20` (upstream's), `b_max = 100` (cap for a shared
box), `maxiter = 12`; everything else as the case above, fixed seed.

### Predictions, written before the run (also in the test's doc comment)

1. Every search converges, `sigma <= 0.004`, in at most 8 evaluations.
2. Boron within 250 ppm of the bracketed trajectory at every search, mostly
   within 100 ppm.
3. Monotone non-increasing within noise; total drop 986 +- 150 ppm.
4. 20 to 35 transport solves in total (bracketed: 32).

### Measured (2026-11-17; two runs agree to every digit, fixed seed)

Hardware: Intel Xeon @ 2.10 GHz, 4 logical cores, 1 thread used, 15 GB RAM,
Linux 6.18.44, CPU only; shared, load average 2 to 4.

| search before step | burnup [d] | GRsecant boron [ppm] | k at root | sigma_k | solves | bracketed boron [ppm] | difference [ppm] |
|---|---|---|---|---|---|---|---|
| 0 | 0 | 3991.1 | 1.00129 | 0.00315 | 7 | 4014.8 | -23.7 |
| 1 | 50 | 3622.4 | 1.00395 | 0.00370 | 9 | 3642.2 | -19.8 |
| 2 | 100 | 3424.5 | 1.00209 | 0.00268 | 8 | 3454.0 | -29.5 |
| 3 | 150 | 3244.1 | 1.00190 | 0.00388 | 9 | 3270.0 | -25.9 |
| final | 200 | 3019.7 | 1.00241 | 0.00253 | 8 | 3029.2 | -9.5 |

Total drop 971 ppm (bracketed 986). Wall time 232.8 s for 41 solves (bracketed,
re-run alone the same day: 207.8 s for 32 solves; the original 176.9 s was on a
different load; hardware as above, so only ratios on one host mean anything,
and these were not taken back to back under equal load).

### Against the predictions

1. **Partly missed.** Converged with `sigma <= 0.004` at all five, but the
   evaluation counts are 7, 9, 8, 9, 8: two searches needed 9, not at most 8.
2. Met. All five inside 30 ppm (limit 250, "mostly 100").
3. Met. Every step falls; total 971.
4. **Missed:** 41 solves against 20 to 35, and more than the bracketed run's
   32. A reading of the trial lists: the 4-point fit window holds the two far
   seeds (0 and 6000 ppm) for the first iterations, and `k(p)` is convex over
   that span, so the first proposals land high (4684, 4505, 4354, 4156, 3999 ppm,
   against roots near 3000 to 4000) and several steps are needed to walk in.
   This is a reading of the printed trajectories, not an ablation; a seed pair
   nearer the root, or `memory = 2`, would test it and was not run.

The new trajectory sits a consistent 10 to 30 ppm below the old one, all
five of one sign. Both runs use the same random seed per solve but different
batch counts (GRsecant's solves carry between `b_min` 20 and `b_max` 100 active
generations, against the old fixed 25; the counts were not printed), so the two are not the same noise realisation, and a 25 ppm shift
is 0.4 sigma of one search (65 ppm); one sign over five searches is not
significant at that size and is not explained further here.

The bracketed run re-run through the new hook reproduces the README table above
**to every printed digit** (4014.8, 3642.2, 3454.0, 3270.0, 3029.2 ppm), which
checks that the hook change did not move it. One trap, found on the way: the
bracketed search passes the model `GrSecantSettings::b0` active generations
(default 50), and the first re-run with that default gave 4004.6, 3692.1, 3456.5,
3283.1, 3051.6 ppm, a different design (twice the statistics), not a regression.
The test sets `b0 = 25` explicitly.

What this does not show: one seed per solve, so the two methods' 10 to 30 ppm
difference and the 41-versus-32 solve count are one realisation each. No
ensemble over seeds was run (#1618 item 5 is not done).
