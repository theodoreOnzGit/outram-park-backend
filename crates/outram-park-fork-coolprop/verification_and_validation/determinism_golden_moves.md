# Determinism goldens: record of every move

`tests/cross_platform_determinism.rs` pins FNV-1a-64 hashes of solver outputs
on x86_64-unknown-linux-gnu (release). Its rule is that a golden may move
**only with a named cause**. This file is where each move is recorded,
including a migration that was expected to move them and did not. It is the
twin of `crates/tampines-steam-tables/verification_and_validation/determinism_golden_moves.md`.

## 2026-10-16: stage S3 (petir migration of the HEM hot path): **no golden moved**

- **Change.** gh:#903 stage S3 (epic #896, maintainer decision D6). Every raw
  `f64` transcendental in `eos.rs`, `transport.rs`, `flash.rs`, `vle.rs`,
  `ancillaries.rs` and `props.rs` now calls `petir::mathf::RealMath`:
  71 `exp` → `r_exp`, 15 `ln` → `r_ln`, 131 `powf` → `r_powf`,
  83 `powi` → `r_powi`, 1 `sin` → `r_sin`, 1 `atan` → `r_atan`. It is a
  mechanical rename: no expression was restructured, and integer-exponent
  `powf` sites were left as `r_powf`. `tests/no_platform_libm_on_hot_path.rs`
  gates it.
- **Prediction, written before the run.** `powi` → `r_powi` moves 0 bits.
  `exp`/`ln`/`powf` → ARM ports: glibc's are ARM-derived, but they could
  still differ in the last bit, so a few-ulp move is possible at some
  points. `sin`/`atan` → `libm` may move `*_mu`/`*_lambda` by 1 ulp. All
  reference tests pass unchanged, and the critical-point V&V number is
  unchanged at its printed precision.
- **Result.**
  - **All 68 sub-hashes are bit-identical before and after:** the Water and
    Nitrogen `state_pt`/`state_ph`/`viscosity`/`conductivity` sweeps (20)
    and the three 200-step `OPCPFluidArray` runs (48). The Part C half of
    the prediction ("may move") was **wrong in the cautious direction**.
  - **Why nothing moved, and how far that goes.** Over 2 000 000 random
    samples on this machine, glibc 2.39 and petir's ARM ports differ on
    1388 `exp`, 26 `ln` and 1358 `powf` results (`powi`: 0). So the routes
    are *not* identical: they disagree on about 0.07 % of `exp`/`powf`
    inputs. Either no swept state hit such an input, or the 1-ulp difference
    was absorbed in the Helmholtz sums before it reached an output. The
    harness cannot tell which. The S2 twin did see a move (`p_rho_h`, 134
    values), so the routes are not interchangeable in general.
  - The binary really is on the new route. The migrated test binary no
    longer imports glibc `pow`, `log`, `sin` or `atan`. It still imports
    `exp`, used by the harness's own grid and by modules outside S3.
  - **Reference tests:** every coolprop test target passes before and after:
    the library (249 passed, 4 ignored) and the 18 integration targets
    (`water_reference`, `helium_reference`, `transport_vle`,
    `single_cv_and_flash`, `term_types_reference`, `chung_viscosity`,
    mixtures, humid air, incompressibles, smoke tests,
    `non_analytic_critical_region`, `cross_platform_determinism`). Their full
    `--nocapture` output, minus timing lines, is **byte-identical** before
    and after.
  - **`water_critical_point_iapws95.md`:** `p(T_c, ρ_c) = 22064000.00000 Pa`,
    rel. error **5.217e-14 → 5.217e-14**. Unchanged.
- **Native vs wasm32-wasip1 after the migration** (Node 22 WASI, release):
  **68 of 68 agree**, as before. On this pair that adds nothing, because
  glibc and wasi-libc already agreed on `exp`/`ln`/`pow` (see the test's
  module docs). The discriminating targets are Windows MSVC and macOS, and
  they are still not run.

### Performance (the cost of determinism; maintainer chose determinism, D6)

Interleaved A/B runs of the pre- and post-migration test binaries
(`--exact <test> --test-threads=1`, 5 repetitions each, the harness's own
`finished in` time). The machine was otherwise lightly loaded (load average
under 1). Medians, with the range in brackets:

| workload | before [s] | after [s] | ratio |
|---|---|---|---|
| Water + N2 `state_pt` → `state_ph` + μ, λ sweep (2 × 576 points) | 2.02 [1.94–2.12] | 3.58 [3.49–3.59] | **1.77×** |
| N2 `OPCPFluidArray`, 200 steps, `Pimple` | 0.36 [0.32–0.47] | 0.60 [0.55–0.65] | **1.67×** |
| N2 `OPCPFluidArray`, 200 steps, KNP forced | 1.12 [1.12–1.17] | 1.89 [1.69–1.90] | **1.69×** |

The slowdown is larger than the 1.41–1.57× that petir measured for `powf`
alone. These workloads are dominated by EOS and transport evaluations, and
those are `powf`-heavy (131 sites, many with integer exponents). The
mitigation the audit names (R3) has **not** been applied, because it would
change bits: route integer-exponent `powf` sites to `r_powi`, or check
whether CoolProp upstream special-cases integer exponents. That is a
candidate follow-up.
