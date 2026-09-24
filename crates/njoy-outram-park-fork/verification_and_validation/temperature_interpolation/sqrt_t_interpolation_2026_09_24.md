# How wrong is the `sqrt(T)` temperature-interpolation shortcut? — measured

**Date:** 2026-09-24
**Issue:** [gh:#269](https://github.com/theodoreOnzGit/outram-park-backend/issues/269)
scope item 4: *"A measurement of the interpolation error against a directly
NJOY-broadened library at the same temperature — which is the check that says
whether the shortcut is acceptable at all."*
**Test:** `crates/njoy-outram-park-fork/tests/wmp_arbitrary_temperature.rs`
(3 tests).

## Why this could be measured without building anything

#269 is **P3 and filed to be recorded, not scheduled** — it has no acceptance
section, only "scope, if picked up". Its open question is whether interpolating
between two pre-broadened tables is good enough to be worth building, and
answering that does **not** need multi-temperature support. It needs an *exact*
cross section at the intermediate temperature to compare an interpolation
against, and windowed multipole is exactly that — analytic Doppler broadening at
any temperature. So the measurement costs a few evaluations and it de-risks the
decision without implementing the feature.

## Method

Upstream's `temperature_method = 'interpolation'` is linear in `sqrt(T)`. For a
target `T` bracketed by tabulated `T1 < T < T2`:

```text
    f     = (sqrt(T) - sqrt(T1)) / (sqrt(T2) - sqrt(T1))
    sigma = (1 - f) sigma(T1) + f sigma(T2)
```

compared against `sigma(T)` evaluated analytically. The target is the **midpoint
in `sqrt(T)`**, which is the worst case for a linear interpolant.

Probe: **U-238's 6.67 eV capture resonance peak**, the hardest place for any
interpolation — broadening changes the peak by a factor of ~5 over 0–1200 K, and
a resonance peak is where the temperature dependence is most curved. The **wing
at 6.424 eV** is reported beside it because it moves the *opposite* way with
temperature (the peak falls, the wing rises), so no interpolation can be biased
to suit both. The test asserts that opposition rather than only printing it: if
the two ever moved together the wing would stop being an independent check.

## Results

| bracket `[T1, T2]` K | target T | peak interp | peak exact | **peak err** | **wing err** |
|---|---|---|---|---|---|
| [293.6, 600.0] | 433.4 | 6217.431 | 6091.006 | **+2.076 %** | +1.032 % |
| [600.0, 900.0] | 742.4 | 4904.206 | 4868.122 | **+0.741 %** | +2.131 % |
| [900.0, 1200.0] | 1044.6 | 4217.838 | 4201.330 | **+0.393 %** | +2.521 % |
| [293.6, 1200.0] | 670.2 | 5531.063 | 5084.137 | **+8.791 %** | **+19.426 %** |

Underlying analytic values (barn), for reference:

| T [K] | `sigma_a` peak | `sigma_a` wing |
|---|---|---|
| 0.0 | 22257.071 | 54.672 |
| 293.6 | 7108.719 | 58.834 |
| 600.0 | 5326.143 | 65.015 |
| 900.0 | 4482.269 | 77.029 |
| 1200.0 | 3953.408 | 101.279 |

## What it says

1. **With ~300 K brackets the shortcut costs 0.4–2.1 % on the resonance peak and
   1.0–2.5 % on the wing.** That is the regime a real multi-temperature library
   ships in, and it is a usable number: tolerable for a scoping calculation,
   probably not for a resonance-sensitive result quoted to better than a percent.
2. **Interpolating across the whole range is much worse — 8.8 % at the peak and
   19.4 % on the wing.** So the bracket width is the entire question, and
   "interpolation" is not one accuracy but a family of them.
3. **The error is always POSITIVE.** `sigma(T)` is convex in `sqrt(T)`, so a
   linear interpolant always sits above the true value. It is a *bias*, not a
   scatter — it will not average out over many materials, which matters for the
   spatially-varying-temperature coupling (#208) that #269 names as the case
   where this would start to bite.
4. **The peak error shrinks with temperature while the wing error grows.** The
   two cannot be traded off against each other by choosing brackets.

## The important caveat

**This measures interpolation against WMP, not against NJOY.** #269 asks for the
comparison against *"a directly NJOY-broadened library"*. WMP is an analytic
representation fitted to the evaluation, not a BROADR run, so a WMP-vs-BROADR
residual would add to what is measured here. That comparison is separate work and
is **not done**. What the numbers above bound is the **interpolation term alone**
— which is the term the shortcut introduces, and the one the decision turns on.

## Limitations

1. **One nuclide, one resonance, one energy pair.** U-238's 6.67 eV resonance is
   chosen as the hardest case, not as a representative one.
2. **Absorption only.** Elastic and fission are not measured.
3. **Midpoint targets only.** The error at other points in a bracket is smaller
   but is not mapped.
4. **Nothing is implemented.** No multi-temperature representation, no
   `temperature_method`, no per-material selection at lookup. #269 remains a
   record, and this makes it a record with a number in it.

## The gap itself, re-verified 2026-09-24

All three of #269's factual claims still hold:

* **Single temperature per material** — `outram-mc-libs/src/material/material.rs:68`,
  `pub temperature: f64`, "Temperature in Kelvin (passed straight to the WMP
  Doppler evaluator)", exactly as the issue quotes.
* **WMP gives analytic Doppler at any temperature** — verified by running:
  arbitrary targets 700 K, 723.5 K and 750 K all evaluate, and two temperatures
  coexist in one process (300 K → 7049.389, 1500 K → 3580.863).
* **BROADR can broaden to a requested temperature** —
  `njoy-outram-park-fork::broadr::broaden_result(result, temp_k)`.

So the two workarounds the issue relies on are both real, which is what makes P3
the right priority.
