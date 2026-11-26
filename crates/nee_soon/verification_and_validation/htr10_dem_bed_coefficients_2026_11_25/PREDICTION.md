# Predictions: HTR-10 temperature coefficients and rod-bank worth on the poured bed (gh:#1739)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted (Claude Code), not yet reviewed by the
> maintainer. **Written and committed before any run of this record.**

## What will be run

- **Driver:** `crates/nee_soon/examples/htr10_graphite_zone_worth.rs`, the
  gh:#1174 driver, with two options added for this record:
  - `--dem-bed`: every case on the poured bed of the 2026-10-08 record (the
    gh:#216 pour, cut to 16 681 core balls, `paper_fuel_assignment`);
  - `--rods-in`: a RODS-IN case (base temperatures, all ten rods fully in).
- **Statistics, temperatures and seed:** gh:#1174's, unchanged, so the bed
  is the only difference:
  - base 600 K, +600 K, 3000 × [20 + 80], seed 20260917;
  - correlated sampling, uncorrelated σ quoted (an upper bound).
- **Cases on the poured bed:** BASE, FUEL-GRAPHITE, DUMMIES, KERNEL,
  RODS-IN.
- **On the lattice:** BASE and RODS-IN, with the same driver and settings.
  - BASE re-runs gh:#1174's base on today's code.
  - RODS-IN pairs the rod bank at 600 K. The gh:#1025 bank, 17.62 ± 0.48 %
    Δk/k, was taken at 300.15 K and 2000 × [20 + 100], so it is not a pair.
  - The lattice coefficients are gh:#1174's (`htr10_rz_transients/README.md`
    §11.1): fuel graphite −9.95 ± 0.66, dummies −6.78 ± 0.63, kernel
    −2.49 ± 0.64 pcm/K.
- **Coefficients:** `α = (1/k_base − 1/k_case)/ΔT`, the arithmetic of
  MSRE's `coefficient_from_pair` and the twin template's
  `reactivity_coefficient`, with σ from the two runs in quadrature.
  - The moderator coefficient is FUEL-GRAPHITE + DUMMIES.
  - The fuel coefficient is KERNEL.
- **Host:** this cloud container, 4 vCPU, 4 threads. The MSRE live split
  (gh:#1748) may share the CPU at the start. That changes wall time, not
  results.

## Predictions, with reasons

| Quantity | Lattice | Predicted, poured bed | Reason |
|---|---|---|---|
| BASE k at 600 K | 0.95389 ± 0.00229 (gh:#1174) | 0.9480 (range 0.944 to 0.952) | the poured bed sat −583 ± 143 pcm below the lattice at 300 K (2026-10-08 record); I expect the same offset at 600 K, since nothing in it depends on temperature |
| Lattice BASE re-run | — | equal to 0.95389 within 1σ; bit for bit only if the transport does not depend on the thread count (gh:#1174 ran 16 threads, this runs 4) | the code since gh:#1174 claims bit-identity (e.g. gh:#1732) |
| Moderator, FUEL-GRAPHITE + DUMMIES | −16.73 ± 0.91 pcm/K | −16 (range −13 to −20) pcm/K | the moderator coefficient comes from the thermal spectrum and leakage. The bed has the same fuel fraction and nearly the same packing, so the spectrum is nearly the same; the wall layering changes the leakage a little |
| Dummies' share of the moderator worth | 0.405 ± 0.04 | 0.35 to 0.50 | the same mass shares |
| Fuel, KERNEL | −2.49 ± 0.64 pcm/K | −2.5 (range −1.0 to −4.0) pcm/K | Doppler in U-238 depends on the self-shielding inside a pebble, and the pebbles are identical |
| Bed effect on each coefficient | — | not resolved: inside 2σ of the lattice | the statistics give σ ≈ 0.7 to 1.0 pcm/K per coefficient |
| Rod bank, RODS-IN at 600 K | (paired run) | 15 to 20 % Δk/k on both beds, poured minus lattice within ±1.5 % | the rods sit in the side reflector. The poured bed's wall layering moves fuel at the periphery, which could change the worth by a few percent of itself |

**What would surprise me:**
- a moderator coefficient outside −10 to −24 pcm/K on the poured bed;
- a bed effect on any coefficient resolved beyond 3σ;
- a rod-bank difference above 3 % Δk/k.

Any of these would be investigated before the number reaches the plant
(#1740).

## Known gaps, stated now

- **One pour.** The pour-to-pour scatter (#798) is not measured. Each poured
  number is one pour's, and the scatter is missing.
- **A secant, not a derivative.** The coefficients are 600 → 1200 K secants,
  as gh:#1174's. The plant uses them as its coefficients near operating
  temperature, and the record will say so.
- **Not validation.** No measured HTR-10 coefficient on a poured bed exists
  to compare against. This is code-to-code at most against [T1]'s values,
  which gh:#1174 quotes.
