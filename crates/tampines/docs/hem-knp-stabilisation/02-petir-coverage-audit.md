# petir coverage and migration audit for the HEM/KNP solver (epic #896, Phase 1)

**Date:** 2026-10-16. **Mode:** read-only audit. Nothing in the repo was edited.
**Scope:** `crates/petir`, `crates/tampines-steam-tables`, `crates/outram-park-fork-coolprop`, `crates/outram-foam-basic-lib`.
**Counting instrument:** `count.sh` (in the same scratchpad folder). It counts
regex occurrences of `\.name\(`, skips comment lines, and calls a site "non-test"
when its file path contains neither `tests/` nor `test`, and it sits before the
file's first `#[cfg(test)]`. This is a heuristic. It also matches `uom`
quantity methods and any user method with the same name, so treat the counts as
±5 %, not exact.

---

## 0. Headline findings

1. **`tampines-steam-tables` is already mostly migrated.** It has 193 non-test
   `r_exp`/`r_ln`/`r_powf` sites (`petir::mathf::RealMath`, imported in 46
   files) and **zero** raw `.powf(`. What still reaches the platform on the HEM
   hot path is almost entirely **`f64::powi`, about 258 sites in the IF97
   regions and backward equations**, plus one `.atan(` in thermal conductivity.
   **DONE 2026-10-16 (S2, gh:#903):** 259 `powi`, 1 `atan`, 8 `ln` and 1
   `exp` (the last nine in `rho_h_flash_eqm`, which this audit had not
   counted) moved to `r_*`; gated by
   `tampines-steam-tables/tests/no_platform_libm_on_hot_path.rs`. Moves are
   recorded in `tampines-steam-tables/verification_and_validation/determinism_golden_moves.md`.
2. ~~**`outram-park-fork-coolprop` has not been migrated at all.** It has 0 `r_*`
   sites.~~ **DONE 2026-10-16 (S3, gh:#903)** for the six files below: exactly
   71 exp, 15 ln, 131 powf, 83 powi, 1 sin and 1 atan now go through
   `RealMath`, gated by `outram-park-fork-coolprop/tests/no_platform_libm_on_hot_path.rs`.
   The rest of the crate (`humid_air`, `mixtures`, `incompressibles`,
   `openfoam_algorithms`, `transport_corresponding_states.rs`) is still
   unmigrated. The original finding: its HEM hot path (`eos.rs`, `transport.rs`, `flash.rs`, `vle.rs`,
   `ancillaries.rs`, `props.rs`) has **about 71 exp, 15 ln, 131 powf, 83 powi
   and 2 trig** calls.
3. **`outram-foam-basic-lib` is not on the transcendental hot path.** Tampines
   and coolprop take only `mesh::fv_mesh`, `interface::one_dimensional_meshing`
   and `primitives::*` from it, and those modules contain only `sqrt` (4 sites).
   Each solver uses its **own ~15k-line copy** of `openfoam_source`: 15,100
   lines in tampines and 14,776 in coolprop. That copy holds the LDU matrix, CG,
   Gauss-Seidel and fv_operators, and its hot-path parts contain only `sqrt` and
   one `powi`.
4. **The CFD/KNP layer itself is clean.** Neither `rhoPimpleFoam/central_upwind.rs`
   has a non-`sqrt` transcendental. Every determinism risk enters through the
   **property calls**: `hem_sound_speed_ph`, `t/v/s/cp/kappa_t/lambda/mu_ph_eqm`,
   `p_rho_h_eqm`, `w_tp_{1,2,3,5}`, `w_ps_eqm_region4_kieffer` in tampines, and
   `state_ph`, `state_pt`, `drho_dp_t`, `state_trho`, `viscosity`,
   `conductivity` in coolprop.
5. ~~**petir's `RealMath` covers only `r_exp`, `r_ln` and `r_powf`.**~~
   **CORRECTED 2026-10-16 (S1 done under D6):** `RealMath` now also has
   `r_powi` (→ `real::powi`) and the `libm`-delegated `r_log10`, `r_cbrt`,
   `r_exp_m1`, `r_ln_1p`, `r_sin`, `r_cos`, `r_tan`, `r_asin`, `r_acos`,
   `r_atan`, `r_atan2`, `r_hypot`, `r_sinh`, `r_cosh` and `r_tanh`, each
   pinned bit-identical to its target. The original finding read: The
   migration also needs method forms for `powi` (a free function
   `petir::real::powi` exists), `log10`, `cbrt`, `exp_m1`, `ln_1p`, `sin`,
   `cos`, `tan`, `atan`, `atan2`, `hypot` and `acos`. All of them are
   *delegations* to `libm` and need no GSL port.
6. **Hand-rolled root finders do not break determinism.** Pure arithmetic plus
   petir transcendentals is already bit-reproducible. Replacing them with
   petir is a provenance/deduplication task, not a determinism one. Two of them
   have **no petir/GSL equivalent**: the Illinois false position
   (`rho_h_flash_eqm`) and the safeguarded Newton-with-bisection (coolprop
   `flash.rs`).
7. ~~**The existing wasm-vs-native Sod mismatch (about 9e-16 relative) has not
   been traced, and the petir migration may not remove it.** The
   `RhoCentralFoam` flux kernel calls only `sqrt`, which is IEEE correctly
   rounded and so identical on both targets. Stage 0 below must find the
   source first, or the premise "petir ⇒ bitwise" is untested.~~
   **TRACED 2026-10-16 (S0, gh:#903).** It was not a platform difference.
   `outram_foam_mesh::block_mesh` ordered boundary faces by iterating a
   randomly seeded `HashMap`, so cell centres moved by 1–2 ulp **between two
   native runs of the same binary**. After the boundary faces were sorted,
   native and `wasm32-wasip1` agree bit for bit at all four Δt, as does every
   step of the Δt = 1e-6 trace. The `sqrt` reasoning above was right; the
   difference was never in the arithmetic. See the `wasm_worker_timing`
   README.

---

## 1. What petir offers

`#![no_std]` unconditionally, `alloc` required, `#![forbid(unsafe_code)]`, no
`std` feature (the `platform-libm` hatch was removed 2026-09-17). The only
dependencies are `libm`, plus optional no_std `uom` (`transfer-fn`, default on)
and optional `wgpu`. Maturity was declared 2026-09-15 on the bar "agreement with
compiled GSL 2.8". `roots`, `min`, `interp` and `ode` (RKF45) agree 100 %
bit-identically, every iterate (`crates/petir/CLAUDE.md`, "Maturity"). It is not
validated against any benchmark.

| Need | petir module | Notes |
|---|---|---|
| exp, ln, powf | `real::{exp, ln, powf}` and `mathf::RealMath::{r_exp, r_ln, r_powf}` → `fast_exp`/`fast_log`/`fast_pow` (ARM optimized-routines ports, bit-identical to upstream ARM C) | **Default** deterministic route |
| powi | `real::powi` (free function, libgcc `__powidf2` algorithm) | ~~No `RealMath` method~~ `RealMath::r_powi` since 2026-10-16 |
| log10, cbrt, sin, cos, tanh, atan2 | `real::*` free functions → `libm` (musl port) | ~~No `RealMath` method~~ `r_`-methods since 2026-10-16 (D6) |
| sqrt, exp2, exp_m1, ln_1p, log2, tan, asin/acos/atan, sinh/cosh/asinh/…, hypot, floor/ceil/round, mul_add | `real::Real` trait → `libm` | **Trap:** `Real::exp/ln/powf` go to **libm**, not ARM, so lifted code keeps its bits. In a `std` crate the inherent `f64` method shadows the trait, so `x.exp()` silently stays on the platform. You must write `Real::sinh(a)` (UFCS), see `docs/why-petir-last-ulp-evidence.md` |
| erf, erfc, tgamma, lgamma | `real::*`, `specfunc` | Already used by all three crates' `inc_gamma`/`inv_inc_gamma` |
| Root finding | `roots::BracketingSolver` (bisection, false position, Brent) and `roots::PolishingSolver` (Newton, secant, Steffenson), with GSL's three convergence tests | GSL 2.8 port, bit-identical every iterate. **No** safeguarded/bracketed Newton and **no** Illinois variant |
| Minimisation | `min::Minimizer` (golden section, Brent) | 1-D only |
| Integration | `integration` (QUADPACK QK rules, `qag`, `qags`), Gauss-Legendre to order 30, Newton-Cotes | No infinite-range/weighted variants |
| ODE | `ode::{rk4_step, rkf45_step, solve_rkf45}` | **No stiff/implicit methods** (no Rosenbrock) |
| Linear algebra | `linalg`: dense `SquareMatrix` Crout LU (lifted from foam-basic-lib), det/lndet/inverse, BLAS-1, symmetric tridiagonal, Householder QR plus least squares | **Dense only.** No sparse/LDU, no Krylov (CG, BiCGStab), no Gauss-Seidel, no eigen/SVD |
| Interpolation | `interp` (linear, natural cubic spline) and `cheb`/`cheb_slice` | No Akima/Steffen/periodic |
| Polynomials | `poly` (Horner, lifted OpenFOAM linear/quadratic/cubic roots, quartic, companion-matrix all-roots, dense algebra) | — |
| Differentiation | `deriv` (central/forward/backward with error estimate) | — |
| RNG | `rng::lcg` (OpenMC) | — |

**Documented determinism guarantee:** `libm` and ARM routines are one fixed
implementation, so the results are "bit-identical across platforms". Measured
evidence:

- `real.rs`: std vs libm differ on 1.68 % of `ln`, 9.64 % of `exp` and 3.22 %
  of `cos` over 200k points.
- `docs/why-petir-last-ulp-evidence.md`: `bedok` moved 14.33 pcm Linux vs
  Windows from 7 `sinh`/`cosh` calls.

**petir has no cross-target runtime test** of that claim. It is built for
thumbv7em and wasm32 and checked for Android, but its outputs are never compared
across targets.

## 2. What the three crates use today

### 2a. Platform-libm-dependent call sites (non-test / all)

| call | tampines | coolprop | foam-basic-lib |
|---|---|---|---|
| `.powf(` | 0 / 0 | **169** / 169 | 17 / 24 |
| `.powi(` | **269** / 272 | **180** / 182 | 9 / 25 |
| `.exp(` | 1 / 4 | **108** / 116 | 20 / 64 |
| `.ln(` | 2 / 23 | **66** / 68 | 41 / 49 |
| `.log10(` | 24 / 65 | 0 | 0 / 1 |
| `.sin(` `.cos(` `.tan(` | 3, 11, 1 | 2, 1, 0 | 1, 1, 0 (33/15 in tests) |
| `.cbrt(` | 4 | 6 | 4 / 6 |
| `.atan(` `.atan2(` `.acos(` | 1, 2, 0 | 1, 1, 0 | 0, 1, 1 |
| `.exp_m1(` `.ln_1p(` `.hypot(` | 3, 3, 2 | 0, 3, 1 | 0, 3, 2 |
| `.tanh(` | 0 | 0 | 0 / 5 |
| **subtotal (excl. sqrt)** | **326** | **538** | **100** |
| `.sqrt(`, deterministic (IEEE correctly rounded) | 80 / 98 | 76 / 78 | 52 / 81 |
| already on petir `r_exp`/`r_ln`/`r_powf` | 45 / 55 / 93 | 0 | 0 |
| `mul_add` (correctly rounded everywhere, so deterministic) | 2 | 2 | 2 (+1 test) |
| `uom` typenum `powi(P2)` etc. | 0 | 0 | 0 |

`uom` transcendental methods are negligible. About 15 `.sqrt()` calls in
tampines are on `uom` quantities, and they are deterministic in any case.

**Is `f64::powi` deterministic? Unverified, and petir contradicts itself on
it.**

- `crates/petir/src/real.rs:163-171` records that `f64::powi` "is lowered by
  each target's backend, so its last bit is not pinned". Its evidence: 27 ACE
  words differed on `x86_64-pc-windows-msvc` (njoy `sigfig`, 2026-10-15).
- `crates/petir/src/mathf.rs:91-95` says `powi` is "exact" and "already
  bit-identical on every platform".

**This is a doc defect to fix in Stage 1.** A plausible mechanism, not
verified: LLVM constant-folds `llvm.powi` with both arguments constant using the
*compiler host's* C `pow`, while runtime calls use square-and-multiply. If that
is right, the risk is concentrated in constant-argument sites, and IF97's
`x.powi(ii)` with a runtime base is probably already deterministic.

**MEASURED 2026-10-16 (S2), x86_64-linux:** the constant-folding mechanism
is real, and it differs *within one target*. `sat_pressure_4(453.15 K)`, with
a literal temperature in a lib test, gave `1002634.5688120957 Pa` through
`f64::powi` (folded at compile time) and `…0954 Pa` through `r_powi`; the
same expression with the temperature behind `black_box` gives the `r_powi`
bits through `f64::powi` too (scratch reproduction). The triple-point
`p_triple()` moved by 1 ulp the same way. At runtime-base sites, `f64::powi`
and `r_powi` agreed on 2 000 000 of 2 000 000 random samples and on every
Part A/B sub-hash.

### 2b. External numerics crates

| crate | Cargo deps | where used |
|---|---|---|
| tampines | `petir`, `ndarray`, `approx`, `uom`, `outram-foam-basic-lib`. `roots` is a **dev-dependency only** (non-android/wasm) | `ndarray`: none found in hot path. The `roots::` hits in `src/` are OpenFOAM's `polynomial::roots::{Roots, RootType}`, **not the `roots` crate** |
| coolprop | `petir`, `uom`, `outram-foam-basic-lib` | no external numerics |
| foam-basic-lib | `petir`, `ndarray`, `rayon` (optional `parallel`), `wgpu` (optional), `ndarray-linalg` (optional, **only** the `blas-bench` `matrix_bench` test) | `rayon` in `compute.rs`, `math/parallel.rs`, `math/minimise.rs`, `math/differentiate.rs`, `ldu_matrix/parallel.rs`; not on the HEM path; off by default |

No `peroxide`, `nalgebra` or `argmin` in any of the three crates.

### 2c. Hand-rolled solvers that duplicate (or lack) petir

| solver | location(s) | petir equivalent | hot path? |
|---|---|---|---|
| Illinois false position, plus bisection fallback | `tampines/src/interfaces/functional_programming/rho_h_flash_eqm/mod.rs:1193-1230` (`p_rho_h_eqm_si`) | **none** (GSL `falsepos` is a different safeguard) | **yes** (`p_rho_h_eqm` from `rhoPimpleFoam/mod.rs:1443,2882`) |
| bracket bisection (log scan) | same file, `region_boundaries` :298, `bracket_pressure` :382 | `roots` Bisection | yes |
| Newton with numerical slope | same file, `saturated_liquid_temperature_from_enthalpy` :1064 | `PolishingSolver` Newton/secant (unsafeguarded) | yes (Region-1 snap) |
| Newton, positivity damping, bisection fallback | `coolprop/src/flash.rs` `density_pt` :63, `solve_pt_outer` :171, `temperature_hrho_counted` :336 | **none** (no safeguarded Newton in GSL) | **yes** |
| golden section | `tampines/.../choked_flow/stagnation_point_outside_vle_ph_dome_multiphase.rs:168`; `foam-basic-lib/src/math/minimise.rs:1377,1403,1453` (batch) | `min::Minimizer` GoldenSection | break BC (critical flow) |
| Newton / Brent / bisection lanes (batch) | `foam-basic-lib/src/math/parallel.rs:1321,1444,1494,1603` | `roots` (scalar only) | no |
| HS-flash iteration, choked-flow, nozzle iterations | `tampines/.../hs_flash_eqm/mod.rs:961`, `steam_turbine_equations/**` | `roots` | choked-flow BC only |
| `newton_t` (JANAF/thermo) | three copies: `*/openfoam_source/thermophysics/thermo/traits.rs:115` and foam-basic-lib `thermophysics/thermo/traits.rs:116` | `roots` Newton | no |
| Rkf45 (OpenFOAM lineage), Rosenbrock23 | three copies of `ode/rkf45.rs`; tampines `ode/rosenbrock23.rs` | `ode` RKF45 (GSL coefficients differ); **no stiff method** | no |
| CG, Gauss-Seidel, BiCGStab, GAMG (LDU sparse) | three copies of `ldu_matrix/{fv_matrix.rs, solvers/gauss_seidel.rs}`; foam-basic-lib `krylov/bicgstab.rs`, `linear_operator.rs:490` | **none** (petir is dense only) | **yes** (`solve`, `solve_cg` in both PIMPLE loops). Sequential arithmetic, so already deterministic |

## 3. Hot-path map and migration priority

**Tampines KNP/HEM path:**

- `rhoPimpleFoam/{mod.rs, central_upwind.rs, lateral_coupling.rs}`. 0 non-sqrt
  transcendentals in non-test code.
- It calls the `ph_flash_eqm` functions (`ph_flash_region`, `t_ph_eqm`,
  `v_ph_eqm`, `s_ph_eqm`, `cp_ph_eqm`, `kappa_t_ph_eqm`, `lambda_ph_eqm`), plus
  `mu_ph_eqm`, `rho_h_flash_eqm::p_rho_h_eqm`, `w_tp_{1,2,3,5}`,
  `w_ps_eqm_region4_{kieffer,finite_diff_vol}` and `pt_flash_eqm` (IC).
- Underneath are the IF97 regions 1–5 and the backward ph/ps/hs equations.

Remaining platform calls, non-test:

| dir | platform calls |
|---|---|
| region_1 | 24 powi |
| region_2 | 64 powi |
| region_3 | 119 powi |
| region_4 | 10 powi |
| region_5 | 29 powi (plus 1 `log10` in `t_ph_5`/`t_ps_5`, Region 5 only) |
| backward ph/ps/hs | 5 powi |
| thermal_conductivity | 7 powi, 1 atan |

That is **≈258 powi and 1 atan**. ~~Everything else is already `r_*` or `sqrt`.~~
**CORRECTED 2026-10-16 (S2):** `interfaces/functional_programming/rho_h_flash_eqm/mod.rs`
(`p_rho_h_eqm`, on the solver path) also had 8 raw `ln`, 1 `exp` and 1 `powi`;
the S0 negative control caught it, and S2 migrated it.
The chebyshev `p_rho_h` (24 `log10`) is **not** on the solver path: only
`p_rho_h_eqm_explicit` uses it, and the solver calls the iterative
`p_rho_h_eqm`.

**Coolprop KNP/HEM path:**

- `rhoPimpleFoam` (1 sqrt).
- `flash::{state_ph, state_pt, drho_dp_t}`, `props::state_trho`,
  `vle::{phase_at_ph, saturation_*}`, `transport::{viscosity, conductivity}`.

Platform calls by file:

| file | exp | ln | powf | powi | trig |
|---|---|---|---|---|---|
| `eos.rs` | 15 | 8 | 48 | 4 | — |
| `transport.rs` | 55 | 6 | 81 | 78 | 1 atan, 1 sin |
| `ancillaries.rs` | 1 | — | 1 | — | — |
| `flash.rs` | — | — | 1 | — | — |
| `vle.rs` | — | 1 | — | — | — |
| `props.rs` | — | — | — | 1 | — |

**foam-basic-lib on the path:** `mesh::fv_mesh`, `one_dimensional_meshing`,
`primitives` (4 sqrt). **Nothing to migrate for determinism.**

**Priority:**

1. tampines IF97 `powi` and `atan`. This is small, and it completes the crate.
2. coolprop `eos.rs` + `transport.rs` + `flash`/`vle`/`ancillaries`/`props`.
   This is the largest piece.
3. The duplicated `openfoam_source` copies. tampines: 9 powi, 4 cbrt, 3 ln_1p,
   1 atan2/hypot/sin/cos. coolprop: 28 exp, 43 ln, 17 powf, 11 powi, 4 cbrt,
   and others. None of it is hot. Prefer deduplicating onto foam-basic-lib per
   the API-placement slice.
4. foam-basic-lib (100 sites) for workspace consistency. Coordinate with petir
   re-lifts (see Risk R4).
5. Off-path: tampines `steam_turbine_equations` trig, coolprop
   `humid_air`/`incompressibles`/`mixtures`.

## 4. Is cross-platform determinism testable? Yes, with a negative control

**Why the test is needed:**

- **Rust does not contract a*b+c into FMA** and does not reassociate float
  reductions.
- `sqrt` and `mul_add` are correctly rounded everywhere.
- The hot path has no `HashMap` iteration. Coolprop's `gamg.rs` uses one, but
  GAMG is not called by its PIMPLE loop.
- `rayon` is off.

So once every transcendental goes through petir, bitwise agreement across
x86_64 / aarch64 / wasm32 is *expected*. It is still unproven. ~~The Sod
mismatch shows that an untraced source exists.~~ **CORRECTED 2026-10-16:** the
Sod mismatch was a run-to-run `HashMap` ordering defect in `block_mesh`, now
fixed (item 7 of §0). It says nothing about the arithmetic.

**Test design**, proposed files `tampines-steam-tables/tests/cross_platform_determinism.rs`
and a twin in coolprop:

1. **Hash.** Inline FNV-1a-64 over `f64::to_bits()` of each output, in a fixed
   order. It is about 10 lines and adds no dependency (`sha2` is already a
   workspace dep if preferred). Keep **one sub-hash per function** so a failure
   names the routine. When `DETERMINISM_DUMP=1` is set, print the first
   differing index and value to help diagnosis.
2. **Part A, IF97 property sweep.** It takes milliseconds.
   - Grid: a fixed 48 × 48 grid. p is log-spaced 611.657 Pa – 100 MPa; h is
     linear 0 – 4200 kJ/kg, filtered by the crate's own validity predicates.
   - Calls: `t_ph_eqm`, `v_ph_eqm`, `s_ph_eqm`, `x_ph_flash`,
     `kappa_t_ph_eqm`, `mu_ph_eqm`, `lambda_ph_eqm`, and `p_rho_h_eqm` on the
     round-tripped ρ.
   - Also `hem_sound_speed_ph`. It is `pub(crate)`, so put this part in a lib
     `#[cfg(test)]` module, or expose it through the epic's API stage.
   - Include **constant-argument `powi`** probes (e.g. `10f64.powi(-23)`) to
     test the constant-folding hypothesis directly.
3. **Part B, short Edwards slice.** It takes about 3–5 s native and is
   extrapolated from 346 s per 20,000 steps.
   - Run the `wasm_smoke.rs` geometry with the benchmark's Hendrie IC and the
     choked BC: 24 cells, dt = 30 µs, **200 steps**.
   - Hash `p`, `he`, `rho`, `U` after steps 1, 10, 50 and 200. Hashing at
     several steps shows *when* a divergence starts, which matters for a chained
     solve (`mathf.rs`: a 1-ulp change flipped Edwards to a panic).
   - Run it once with `SolverMode::Pimple` and once with the hybrid KNP on.
4. **Part C, coolprop.**
   - `state_ph`/`state_pt` sweeps for Water and Nitrogen.
   - A 200-step coolprop `rhoPimpleFoam` array run.
5. **Targets, using runners that already exist:**

   | target | where it runs |
   |---|---|
   | `x86_64-unknown-linux-gnu` | ubuntu-latest |
   | `x86_64-pc-windows-msvc` | windows-latest, which also catches the MSVC `powi` evidence |
   | `aarch64-apple-darwin` | macos-latest, which **is Apple Silicon, so a real aarch64 runtime** |
   | `wasm32-wasip1` | existing Node runner in `.cargo/config.toml`, same mechanism as `tests/wasm_smoke.rs` |
   | `aarch64-unknown-linux-gnu` | optional, `ubuntu-24.04-arm` or qemu-user. Neither qemu nor wasmtime is in this container (node 22 is) |
   | Android | compile-only in CI (`fast-tests.yml`). Runtime is a manual Termux run |

   Note: `wasm32-wasip1` std maths goes to wasi-libc, while
   `wasm32-unknown-unknown` std goes to the Rust `libm` port. Pre-migration
   they may differ from each other; post-migration both use petir.
6. **Pass criterion.** Every target reproduces one committed `const GOLDEN_*:
   u64` per sub-hash. The test must be < 5 min, so no `long-tests` gate is
   needed.
7. **Negative control** (the test must be able to fail, per CLAUDE.md "Get
   the PROCESS right"):
   - **Before migrating**, run the same harness and record per-target hashes.
   - **Prediction:** Part A *differs* between linux-gnu and macOS/wasm for
     coolprop (raw `exp`/`powf`), and possibly for tampines `powi`
     constant-folded sites.
   - If the pre-migration hashes already agree everywhere, the harness has no
     discriminating power for that path, and the report must say so.
   - Also do a 1-ulp input-perturbation mutation, to show the hash moves.
8. **Golden maintenance.** When the bits legitimately move, regenerate with
   `DETERMINISM_PRINT=1`. Record the move (old hash, new hash, cause) in a
   tampines `verification_and_validation/` note, per "the bits move once".

## 5. Risks

**R1. Missing petir surface (all delegations; no GSL porting needed):**

- `RealMath::r_powi` (wrap `real::powi`), `r_log10`, `r_cbrt`, `r_exp_m1`,
  `r_ln_1p`, `r_sin`, `r_cos`, `r_tan`, `r_atan`, `r_atan2`, `r_hypot`,
  `r_acos` and `r_tanh` (`r_sinh`/`r_cosh` for other consumers).
- `mathf.rs` deliberately excluded the libm-only ones ("opt in knowingly").
  Extending it is therefore a **maintainer decision**, or consumers write
  `petir::real::log10(x)` or `Real::cbrt(x)` (UFCS) instead.

**R2. Missing solvers (would need porting, and the maintainer must approve
non-GSL sources per `petir/CLAUDE.md`):**

- (a) A safeguarded Newton (Newton plus bisection inside a bracket), used by
  coolprop `flash.rs`. GSL has none. Candidates: **CoolProp's own
  `src/Solvers.cpp`** (MIT; it is this port's upstream, so "read upstream
  first" applies) or Boost.Math `newton_raphson_iterate` (BSL-1.0). **Numerical
  Recipes `rtsafe` is not licence-compatible.**
- (b) The Illinois false position in `rho_h_flash_eqm`. GSL has none. Keeping
  it hand-rolled is acceptable: it is arithmetic-only, so already
  deterministic. Swapping it for petir Brent changes iterates and therefore
  moves results.
- (c) A stiff ODE solver (Rosenbrock), off-path.
- (d) Sparse/LDU Krylov solvers. These are outside petir's dense-only scope.
  Keep them in foam-basic-lib. They are already deterministic when sequential;
  with `parallel` on, rayon reductions are **not** reproducible across thread
  counts, so keep that feature off in the determinism test.

**R3. Performance.** Measured in `petir/src/real.rs`, 2M calls, ARM route vs
platform: `exp` **0.70–0.75×** (faster), `ln` 1.04–1.25×, `powf` **1.41–1.57×**
(slower).

- Coolprop's 131 hot `powf` and 83 `powi` dominate EOS/transport cost, so
  expect a measurable slowdown there. Mitigation: route integer exponents to
  `powi`, and check whether CoolProp upstream already special-cases integer
  exponents in its Helmholtz terms.
- The libm `log10`/`cbrt`/trig are "measurably slower" than the platform, but
  that is **not quantified** in petir's docs. Measure it.
- **Never route `sqrt` through `libm`** (3.73× slower). `Real::sqrt` does
  exactly that in a no_std context.
- Benchmark gate: Edwards 600 ms wall time before and after (346.17 s recorded
  2026-09-14 on the platform route).

**R4. Verbatim-lift coupling.** `petir/tests/verbatim_provenance.rs` diffs
nine foam-basic-lib files:

- `math/{erf_inv, inc_gamma, inv_inc_gamma}.rs`
- `matrix/square_matrix.rs`
- `polynomial/{cubic_eqn, linear_eqn, polynomial, quadratic_eqn, roots}.rs`

Rewriting `.exp()` → `.r_exp()` in those files breaks the gate unless petir
re-lifts in the same change. A re-lift also changes petir's own numerics: the
lifted text uses `Real::exp`, which is libm, and `r_exp` is ARM. Today the lifted
copy is **textually identical but numerically different** from its source
(platform vs libm).

**R5. Behaviour changes to recorded V&V.**

- **Edwards–O'Brien** has already shown that a ~1-ulp sweep can flip the run
  from pass to panic (`mathf.rs`, `golden/edwards_600ms_platform_libm.md`).
- ~~The current pins are plateau **359.0 psia** and GS-1 RMSE **42.8 psia**
  (linear scheme, gh:#406).~~ **CORRECTED 2026-10-16 (gh:#900):** with the
  Phase A defaults the pins are plateau **360.5 psia** and GS-1 RMSE
  **17.2 psia** (linear scheme); van Leer gives 358.3 / 19.9 psia. Every migration stage must re-run the full 600 ms
  case and record any movement, including unflattering movement.
- Also re-measure:
  - tampines `verification_and_validation/generated/*.md` (6 reports;
    predicted character-identical, as at the ARM adoption);
  - all coolprop reference tests (`water_reference`, `helium_reference`,
    `transport_vle`, `single_cv_and_flash`, …) and
    `water_critical_point_iapws95.md` (5.2e-14 rel);
  - the coolprop `rhoPimpleFoam` tests.
- **Prediction to write before running:** single-state properties move ≤ a
  few ulp. The `powi` → `real::powi` switch moves **0 bits** at runtime-base
  sites on x86_64 Linux, and moves only constant-folded sites. Chained
  transients can move by O(1) in marginal cells.

**R6. Doc defects found** (fix them in the change that touches each file;
this audit is read-only):

- (a) `crates/petir/src/mathf.rs:91-95` claims `powi` is already bit-identical
  everywhere. `crates/petir/src/real.rs:163-171` contradicts it with measured
  Windows evidence.
- (b) `crates/outram-foam-appbuilder-lib/verification_and_validation/wasm_worker_timing/README.md`
  attributes the 9e-16 mismatch to libm "`pow`/`sqrt`". `sqrt` is IEEE
  correctly rounded, so it cannot differ, and the `rho_central_foam` flux
  kernel (`mod.rs:295,303`) calls only `sqrt`. The README itself says the cause
  was not traced, so this is a hedge to correct, not a fabrication.

**R7. Triplication.** The KNP solver and the ~15k-line `openfoam_source` exist
in tampines, coolprop and (as a library) foam-basic-lib. Migrating before
deduplicating triples the work and the V&V re-measurement.

## 6. Staged migration plan (each stage is a candidate child issue of #896)

| # | Stage | Deliverable / acceptance |
|---|---|---|
| **S0** | **Determinism harness and negative control first** | Add `cross_platform_determinism.rs` (Parts A–C) to tampines and coolprop. Wire it into `fast-tests.yml` on ubuntu, windows and macos, plus a `wasm32-wasip1` job. Record **pre-migration** per-target hashes as the negative control. **Trace the Sod 9e-16 wasm/native mismatch** by per-step hashing. If the source is not libm, find it before claiming that petir fixes determinism |
| **S1** | **petir surface** (maintainer decision on R1) | Add `RealMath::r_powi` (→ `real::powi`) and the libm-delegated `r_log10`, `r_cbrt`, `r_exp_m1`, `r_ln_1p`, `r_sin`, `r_cos`, `r_tan`, `r_atan`, `r_atan2`, `r_hypot`, `r_acos`, `r_tanh`, each with a test pinning bit-identity to `real::`/`libm`. Fix the `mathf.rs` powi doc (R6a) with strike-through. Run petir's five no_std gates |
| **S2** | **tampines IF97 hot path** | About 258 `powi` → `r_powi` and 1 `atan` → `r_atan` in `region_1..5`, `backward_eqn_{ph,ps,hs}_*`, `thermal_conductivity`. Add a grep gate test, `no_platform_libm_on_hot_path`, in the style of petir's `no_panic_gate`. Re-run the full tampines suite and the Edwards 600 ms; record the movements; S0 hashes agree on all targets for Part A |
| **S3** | **coolprop HEM hot path** | `eos.rs`, `transport.rs`, `flash.rs`, `vle.rs`, `ancillaries.rs`, `props.rs` → `r_*` (about 71 exp, 15 ln, 131 powf, 83 powi, 2 trig). Add the same grep gate. Report a performance benchmark (state_ph sweep, coolprop array run) before and after. Re-measure all reference tests and V&V docs |
| **S4** | **Deduplicate `openfoam_source`** (with the API-placement slice) | Fold the tampines and coolprop copies onto `outram-foam-basic-lib` where they are identical. Otherwise migrate the remaining copies (tampines ~25 sites, coolprop ~120 sites) |
| **S5** | **outram-foam-basic-lib consistency** | 100 non-test sites → `r_*`. Re-lift the nine verbatim files into petir in the **same change** (R4), and record petir's moved bits. Re-run foam-basic-lib V&V (cavity, Sod) |
| **S6** | **Solver provenance** (optional, maintainer-gated) | Decide R2(a)/(b): either port CoolProp `Solvers.cpp` (or Boost) safeguarded Newton into petir as a maintainer-directed non-GSL port, or keep the hand-rolled solvers documented as such. Replace golden-section duplicates with `petir::min` where the batch shape is not needed. Every swap is a behaviour change, so re-measure |
| **S7** | **Off-path sweep** | tampines `steam_turbine_equations`, coolprop `humid_air`/`incompressibles`/`mixtures`. Low priority |

**Re-measurement clause for every stage (CLAUDE.md "Correct physics is the
DEFAULT", "re-measure every V&V number"):** run `cargo test --workspace --lib
--tests --release` and the full Edwards case. Record every moved number with
old and new values in the relevant V&V doc. Never re-tune a threshold to absorb
a move.
