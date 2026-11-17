# Determinism goldens: record of every move

`tests/cross_platform_determinism.rs` pins FNV-1a-64 hashes of solver outputs
on x86_64-unknown-linux-gnu (release). Its rule is that a golden may move
**only with a named cause**. This file is where each move is recorded,
including a migration that was expected to move them and did not. It is the
twin of `crates/tampines-steam-tables/verification_and_validation/determinism_golden_moves.md`.

## 2026-11-16: Part C `n2arr_pimple_*`, all 16 MOVED (gh:#1607, the vendored FV copy replaced by `outram-foam-basic-lib`)

**Cause.** gh:#1607 replaced the crate's vendored copy of
`outram-foam-basic-lib` (`openfoam_algorithms/openfoam_source/`, 84 files
that had drifted from it in 78) with re-exports of the crate itself, so the
`rhoPimpleFoam` array now runs on today's operators. Those were changed on
2026-11-11 to follow OpenFOAM v2506's operation order (gh:#1489, 34
differential rows bit-identical to upstream; gh:#1362, 58 rows), which the
copy predated. In the pressure equation:
- `fvc::interpolate` computes upstream's `w*(P - N) + N`
  (`surfaceInterpolationScheme::interpolate`), not `w*P + (1 - w)*N`, which
  differs by an ulp;
- `fvm::laplacian` follows upstream's coefficient order.

The other pressure-equation operators the array uses, `fvm::ddt`,
`FvMatrix::solve` and `solve_cg`, are code-identical to the copy. `fvm::div`
only gained boundary branches (`InletOutlet`, ...) these cases do not use.
The two functions only the copy had, `fvc::div_limited` and
`fvm::ddt_coeff_old`, moved into `outram-foam-basic-lib` unchanged.

**Method.** The same 200-step run on the commit before the change
(`ecb3895e3`, with the copy) and after, every field dumped at full precision
at the sampled steps, and the worst relative difference per field taken.
Release, x86_64 Linux, 2026-11-16.

| step | p | he | rho | U_x |
|---|---|---|---|---|
| 1 | 7.3e-16 | 3.7e-16 | 7.9e-16 | 1.5e-7 (U ≈ 4e-7 m/s at step 1) |
| 10 | 7.3e-16 | 9.4e-16 | 1.2e-15 | 3.6e-12 |
| 50 | 1.2e-15 | 2.8e-14 | 2.7e-14 | 8.6e-13 |
| 200 | 4.1e-15 | 2.6e-14 | 2.3e-14 | 2.7e-12 |

**Interpretation.** Round-off, and it stays round-off: the nitrogen tube is
single-phase, so the ulp-level seed is not amplified (at most 4e-15 in `p` and
3e-14 in `he` after 200 steps), inside the workspace's 1e-8 allowance.

| golden | count | old hash | new hash |
|---|---|---|---|
| `n2arr_pimple_p_1` | 10 | `0xc593ad5453fadb2d` | `0x7c50d8200128cc80` |
| `n2arr_pimple_he_1` | 10 | `0xe02fbb6bcb14e9b9` | `0xc4eea40de0341f61` |
| `n2arr_pimple_rho_1` | 10 | `0x1e7a398d49c67b58` | `0x55b0f68131619c9e` |
| `n2arr_pimple_U_1` | 30 | `0xcbd400ad968b13d8` | `0x252ede2f3e3d5ebe` |
| `n2arr_pimple_p_10` | 10 | `0x60cac2e4deec8a3b` | `0x8c471e8fbb6501c0` |
| `n2arr_pimple_he_10` | 10 | `0xc3825424b8e38e5d` | `0x7b2a7abc59856343` |
| `n2arr_pimple_rho_10` | 10 | `0xb2842afa7d9c8ab2` | `0x2da3069e612f856e` |
| `n2arr_pimple_U_10` | 30 | `0x53e674be6debf8f2` | `0x32fc950f398eb971` |
| `n2arr_pimple_p_50` | 10 | `0xe9fee336758b806d` | `0xd48c276a7fd157ce` |
| `n2arr_pimple_he_50` | 10 | `0x7c9c31f29a443528` | `0x8a4cb0874094000b` |
| `n2arr_pimple_rho_50` | 10 | `0x8758c468709d5cb5` | `0x4ac234ddf9cab513` |
| `n2arr_pimple_U_50` | 30 | `0x34895150d19f50b4` | `0x44f9237942ec31ec` |
| `n2arr_pimple_p_200` | 10 | `0x07ef4d9edb494bfb` | `0xd3f55629983357d8` |
| `n2arr_pimple_he_200` | 10 | `0x3fea5c7f55a1d2e8` | `0x53e4b04c20b48016` |
| `n2arr_pimple_rho_200` | 10 | `0x526666a45dd9bc30` | `0x2d5f8d028bbc0075` |
| `n2arr_pimple_U_200` | 30 | `0xa301458654117bb8` | `0xd3e359a0e51d4223` |

## 2026-11-13: all 20 Water / Nitrogen sweep goldens MOVED (gh:#1349, #917)

- **Cause, two parts.** (1) `density_pt` now picks the stable root first, so compressed-liquid
  `(p, T)` points return the liquid density instead of the metastable vapour root (the CoolProp
  differential counted 3 398 of 3 680 liquid-phase points wrong). Every sweep that goes through
  `state_pt` or `state_ph` (`*_rho`, `*_h`, `*_s`, `*_cp`, `*_cv`, `*_w`, `*_ph_T`, `*_ph_rho`,
  `*_mu`, `*_lambda`) moved. (2) The gas-branch Newton tolerance lost its absolute 1e-9 Pa floor
  (`p * 1e-11` now), which can change the last bits of a converged gas density.
  The two causes were not separated: only the sum was measured.
- **Prediction, written before the run:** the 20 sweep hashes move (liquid points are in both
  grids), the 16 `n2arr_pimple_*` hashes do not (the PIMPLE runs sit in the gas phase).
- **Measured** (release, x86_64-linux): the 20 sweep hashes moved, the 16 PIMPLE hashes did not
  (`cargo test ... --test cross_platform_determinism`: the sweep test failed with the old table, passes
  with the new). Old -> new, `(576 values each)`:

  | sub-hash | old | new |
  |---|---|---|
  | `water_rho` | `0xd6e7bdd49b514bfa` | `0x2de937f8790d710f` |
  | `water_h` | `0x116b029de0d713fd` | `0xab533980a99a3fda` |
  | `water_s` | `0x29cbd4815016e24f` | `0x21c709fe15ebef7c` |
  | `water_cp` | `0xdee28b77521dd8d5` | `0xc5ffcdead2b06f53` |
  | `water_cv` | `0xfb82eadb6e71ac38` | `0x3045d60112ec1f19` |
  | `water_w` | `0x0077a62492027d9e` | `0x5ee2a302f6a1859c` |
  | `water_ph_T` | `0x750f30d3065e6b85` | `0x18c000295ee0ce8f` |
  | `water_ph_rho` | `0x55a7d705c887ce88` | `0x52ce3f4783b91511` |
  | `water_mu` | `0x821605191e8f6344` | `0x1d4a89dc69b3cd9f` |
  | `water_lambda` | `0x1d03bb4cc196ac86` | `0x22e98ce7194cc716` |
  | `n2_rho` | `0xcadfff609240c746` | `0x481d4fb0073282e7` |
  | `n2_h` | `0xbfb6f6ff6e930f05` | `0xe8d8c748b265d0b3` |
  | `n2_s` | `0x8711f1af5c0733d1` | `0xb5e50825b228b9f9` |
  | `n2_cp` | `0x096b5980e6508b43` | `0x1d8b220ebe7a5af2` |
  | `n2_cv` | `0x539aa2ef45c5bd15` | `0x20d219003ad16b82` |
  | `n2_w` | `0x86b82aac43ec6efb` | `0xc0076ee751bcaece` |
  | `n2_ph_T` | `0xadd27a211d530ff5` | `0xe0e905eaa751439a` |
  | `n2_ph_rho` | `0x605496e9059d6d4e` | `0x7f2368e5d71930d4` |
  | `n2_mu` | `0x9d96a0d570808b0c` | `0xed714122666e0ff9` |
  | `n2_lambda` | `0xb78d38d10a8db01c` | `0x04250425065a4e64` |

- **What this does not say:** a hash cannot tell how many of the 576 points moved or by how much; the
  size of the physical change is the one measured against CoolProp in
  `coolprop_differential/RESULTS.md`.

## 2026-10-16: Part C hybrid runs REMOVED (`n2arr_hybrid_*`, `n2arr_knp_*`); nothing moved

- **Cause.** Maintainer decision D7, "retire the KNP hybrid" (gh:#914), in
  step with the steam-tables twin. `SolverMode::HybridAllMach` was deleted
  from `OPCPFluidArray`, so `part_c_rho_pimple_200_steps_hybrid_knp`
  (`n2arr_hybrid`) and `part_c_rho_pimple_200_steps_knp_forced`
  (`n2arr_knp`, Mach window and density taper forced open) were deleted with
  their **32 goldens**. 36 remain: the 20 sweep sub-hashes and the 16
  `n2arr_pimple` ones.
- **Removed values.** In git history: the `GOLDEN-TABLE` of commit
  `42a05c598`.
- **What did not move, and why that is the check.** The removal had to
  leave the PIMPLE path bit-identical. Measured after the removal (release,
  x86_64-linux): all 36 remaining sub-hashes match their goldens unchanged
  (`cargo test --release -p outram-park-fork-coolprop --test
  cross_platform_determinism`: `3 passed; 0 failed`). The library suite
  went from 249 to 245 passing tests: six hybrid-only tests were deleted
  and the two HEM sound-speed checks moved to `flash.rs`.

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
