# Determinism goldens: record of every move

`tests/cross_platform_determinism.rs` pins FNV-1a-64 hashes of solver outputs
on x86_64-unknown-linux-gnu (release). Its rule is that a golden may move
**only with a named cause**. This file is where each move is recorded.

## 2026-10-16: Part B (`edw_*`, 200-step Edwards slice), all three modes

- **Cause.** gh:#900 (epic #896 Phase A) changed `TampinesSteamArray`'s
  defaults to match upstream OpenFOAM-10 `rhoPimpleFoam`. The maintainer
  approved this change of defaults (D5, 2026-10-16). The three changes are:
  - implicit upwind energy convection with deferred correction
    (`EnergyBalanceMode::Implicit`);
  - `PsiRefresh::OncePerOuterCorrector`;
  - the kinetic-energy terms (`KineticEnergyTerms::Included`).
- **What moved.**
  - 45 of 48 Part B sub-hashes moved: Pimple, hybrid KNP and KNP-forced, each
    at steps 1, 10, 50 and 200.
  - The three `he` hashes at step 1 did not move.
  - Part A (the IF97 property sweep) did not move, so the property routines
    are untouched.
- **Old and new values.** The old hashes are in git history: the
  `GOLDEN-TABLE` in `5d16d67e2`. The new hashes are in the current file.
- **Interpretation.** This is a physics/numerics change by design, not a
  determinism regression. The Edwards 600 ms results it corresponds to are
  recorded in the `tests/edwards_blowdown.rs` header. On the Linear scheme,
  the plateau and GS-1 RMSE went from 359.0 / 42.8 psia to 360.5 / 17.2 psia,
  and pressure-bound events went from 150 to 0.

## 2026-10-16: Part A `lambda_ph` and `p_rho_h` (stage S2, petir migration); new `petir_libm` probe

- **Cause.** gh:#903 stage S2 (epic #896, maintainer decision D6) moved the
  remaining platform-libm calls on the HEM solver path onto
  `petir::mathf::RealMath`: 259 `.powi(` → `.r_powi(` in `region_1`..`region_5`,
  the backward `(p,h)`/`(p,s)`/`(h,s)` equations, `thermal_conductivity` and
  `rho_h_flash_eqm`; 1 `.atan(` → `.r_atan(` (`thermal_conductivity`,
  critical enhancement); 8 `.ln(` → `.r_ln(` and 1 `.exp(` → `.r_exp(`
  (`rho_h_flash_eqm`, the `p_rho_h_eqm` bracket scan and Illinois iteration).
- **Predictions, written before the run.** `powi` → `r_powi` moves 0 bits at
  runtime-base sites on x86_64 Linux (same `__powidf2` algorithm).
  `lambda_ph` may move by 1 ulp at a few points (`atan`, glibc → musl `libm`).
  `p_rho_h` moves 0 bits, because glibc's `exp`/`log` are ARM
  optimized-routines too. Part B unchanged.
- **What moved (old → new).**
  - `lambda_ph`: `0x2623fc41179960f4` → `0xf2fb2a32e2b829c9`. One value of
    2164 moved, grid index 1835, by −1 ulp
    (0.1255938678945361 → 0.12559386789453608 W/(m K), rel 2.2e-16). It is
    the same index that differed native vs wasm in the S0 negative control,
    so the `atan` was the cause there. **Prediction right.**
  - `p_rho_h`: `0xe8efa026bd513b32` → `0x8a482250694e65de`. **134 of 2164
    values moved**, by up to 3733 ulp (max relative change 5.0e-13).
    **Prediction wrong.** glibc 2.39 and petir's ARM ports are *not*
    bit-identical on this machine: measured over 2 000 000 random inputs,
    `exp` differs on 1388, `ln` on 26 and `powf` on 1358 (`powi` on 0).
    `p_rho_h_eqm` is an Illinois false-position inversion, so a 1-ulp change
    in one `ln` of `v(p)` moves the iterate path and the converged root
    moves inside the bracket tolerance (`P_RHO_H_REL_TOL = 1e-12`
    relative; the largest move, 5.0e-13, is inside it), not by 1 ulp.
  - **Not moved:** `t_ph`, `v_ph`, `s_ph`, `x_ph`, `kappa_t_ph`, `mu_ph`,
    `hem_sound`, the `powi_*` probes and all 48 Part B (`edw_*`) sub-hashes.
    So the 259 `powi` sites moved 0 bits, as predicted.
  - **New:** `petir_libm` (`0x6dfb4db67eae5e27`), the `std_libm` samples
    through `r_exp`/`r_ln`/`r_powf`/`r_sin`/`r_cbrt`, as a positive control.
    `std_libm` stays raw `std` on purpose: it is the canary that still shows
    the platform differs.
- **Downstream V&V, re-measured.**
  - Edwards 600 ms (`tests/edwards_blowdown.rs`, Linear): plateau 360.5 →
    360.5 psia, GS-1 RMSE 17.2 → 17.2 psia, pressure-bound events 0 → 0,
    drained-cell holds 0 → 0. The whole printed output of the target was
    diffed against a pre-migration build of the same tree and is identical.
  - `edwards_hybrid_damps_ringing_vs_pimple` (known failure, gh:#914) fails
    identically before and after: `sum_h = 2035.79` vs `sum_p = 1648.29`,
    plateau Pimple 360.5 / Hybrid 368.8 psia.
  - The six `verification_and_validation/generated/*.md` reports regenerate
    character-identical except the timestamp line.
  - Library tests: 1013 passed, 0 failed, 14 ignored, before and after.
    Their whole `--nocapture` output was diffed before vs after. Apart from
    timing lines, two printed values moved, and both expose the
    **compile-time folding of `f64::powi`**: `sat_pressure_4(453.15 K)`
    `1002634.5688120957 → 1002634.5688120954 Pa` (and the `v` values derived
    from it by 2–5 ulp), and `p_triple()` `611.212677444345 →
    611.2126774443449 Pa` (1 ulp). Both are called with literal arguments
    in tests, so LLVM folded the old `powi` with the compiler host's `pow`;
    `r_powi` folds to the same bits it computes at run time. A scratch
    reproduction confirmed it: the old expression gives `1.0026345688120957`
    (MPa, dimensionless) when folded and `1.0026345688120955` when the
    temperature is behind `black_box`; `r_powi` gives `…0955` both ways. **This is the
    within-one-target hazard the audit hypothesised (§2a), now measured; the
    migration removes it.**
- **Native vs wasm32-wasip1 after the migration** (Node 22 WASI, release):
  **60 of 61 sub-hashes agree** (59 of the original 60). `lambda_ph` and
  `p_rho_h`, which differed before, now agree. `petir_libm` agrees. The one
  that differs is `std_libm`, the deliberate raw-`std` canary in the test
  itself, not library code.
- **Performance** (interleaved A/B, pre- vs post-migration test binaries,
  5 repetitions, medians with range; load average under 1):
  - Part A IF97 sweep: 0.59 s [0.52–0.65] → 0.58 s [0.55–0.61], no
    measurable change.
  - Part B 200-step Edwards slice (`Pimple`): 2.12 s [2.05–2.39] → 1.84 s
    [1.75–1.85], 0.87×. That is faster, not slower; the cause is not traced.
  - The full Edwards target ran 397.9 s before and 302.9 s after, but the
    two runs were under different machine load, so that is not a
    benchmark.
