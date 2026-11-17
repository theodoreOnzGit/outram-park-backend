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

- The search is the workspace's `search_for_keff` (bracketed bisection or false
  position), **not GRsecant**, which the workspace does not have. It needs a
  straddling bracket: `(x0, x1)` first, then the absolute bracket once, then an
  error. There is no `sigma_final` test and no adaptive batch count.
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
