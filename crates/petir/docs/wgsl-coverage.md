# GSL → WGSL coverage ledger

The state of porting GSL into WGSL (`f32`), module by module, so that
"exhaustive" is a number someone can check rather than an aspiration.

Every figure here was counted from the vendored source at
`crates/petir/upstream_source/GSL` on **2026-09-19**, not recalled.

---

## How big is GSL, really

```
4334   public gsl_* symbols across 60 directories
```

That number is misleading and should not be quoted, because GSL **generates
most of it per element type**. `gsl_matrix_get`, `gsl_matrix_float_get`,
`gsl_matrix_int_get`, `gsl_matrix_ulong_get` … are one operation, not four.
Collapsing the type infixes:

| module | raw symbols | distinct operations | replication |
|---|---|---|---|
| `matrix` | 1021 | **145** | ~7x |
| `vector` | 655 | **99** | ~7x |
| `blas` | 106 | **46** | ~2.3x (`s`/`d`/`c`/`z`) |

**WGSL has one float type.** So the replication does not merely shrink — it
disappears: a single `f32` kernel is the whole of what would be seven C
functions. The honest target is the distinct-operation count, which for the
whole library is on the order of **1200**, not 4334.

---

## What a GPU can and cannot take

Three structural limits decide most of this ledger, and none of them is about
effort:

1. **No `f64`.** Baseline WebGPU has `f32` and nothing wider. Every port here
   is a *different numerical object* from its `f64` original and must be judged
   on an `f32` budget. `petir::wgsl::mirror` exists so that budget can be
   separated from transcription error.
2. **No host control flow inside a kernel.** An adaptive integrator, a root
   finder, a nonlinear least-squares driver — these are loops that decide what
   to compute next based on what they just computed. The *step* is a kernel;
   the *loop* stays on the host. Such modules are marked **PARTIAL** and that
   is their finished state, not a shortfall.
3. **No allocation, no recursion, no pointers into storage as parameters.**
   This rules out the data-structure modules outright.

---

## Status

**PORTED** — in `petir::wgsl`, with an `f32` CPU mirror and GPU verification.
**PORTABLE** — maps cleanly, not yet done.
**PARTIAL** — the per-element kernel is portable; the driver loop is host-side
and stays there.
**N/A** — structurally not a GPU target, with the reason given.

| module | distinct ops | status | notes |
|---|---|---|---|
| `poly` | 18 | **PORTED** (eval) / PORTABLE | Horner and derivatives done; `solve_quadratic`/`cubic` portable |
| `cheb` | ~14 | **PORTED** | Clenshaw, truncated Clenshaw, argument scaling |
| `specfunc` (erf family) | ~12 | **PORTED** | `erf`, `erfc`, series, `erfc8`, 3 Chebyshev branches, at GSL's own `order_sp` |
| `specfunc` (gamma family) | ~10 | **PORTED** | `lngamma`, `gamma`, `lnbeta`, `beta`; Lanczos `g=7` plus both Padé branches at the zeros |
| `specfunc` (Bessel family) | ~20 | **PORTED** | `J_0`, `J_1`, `Y_0`, `Y_1`, `I_0`, `I_1`, `K_0`, `K_1` and the four exponentially scaled modified forms; 18 Chebyshev series and the `cos_pi4`/`sin_pi4` phase helpers |
| `specfunc` (psi/zeta family) | ~18 | **PORTED** (continuous) | `psi`, `psi_1`, `psi_1piy`, `hzeta`, `zeta`, `zetam1`, `eta`. The integer-argument lookups, `zeta(s)` below `s = -34` and `psi_n` for `n >= 2` are deliberately absent — see below |
| `specfunc` (Debye family) | ~6 | **PORTED** | `D_1` .. `D_6` behind one `petir_debye(n, x)`; six Chebyshev series and the falling-factorial polynomial. **Two machine constants are retargeted to `f32`** and the exponential-sum counter is recomputed rather than decremented — see below |
| `matrix` | 145 | **PORTED** (core) | element access, add/sub/mul/div elements, scale, add_constant, transpose |
| `vector` | 99 | **PORTED** (core) | covered by the Level-1 kernels and element access |
| `blas` | 46 | **PORTED** (real, row-major) | L1 `dot`/`nrm2`/`asum`/`iamax`; L2 `gemv` ±trans; L3 `gemm` ±trans |
| — Legendre `P_n` | — | **PORTED** | Bonnet recurrence; not a GSL module but `gsl_sf_legendre`'s subject |
| `specfunc` (rest) | ~275 | PORTABLE | the largest remaining win — almost all pointwise. `dilog`, `airy`, the Fermi-Dirac and Bose-Einstein integrals, and the Coulomb wave functions are the next blocks; the integer-order and arbitrary-order Bessel functions build on the order-0/1 kernels already here |
| `cdf` | ~200 | PORTABLE | pointwise distribution functions |
| `randist` | 102 | PORTABLE | samplers; needs the RNG below |
| `rng` / `qrng` | 28 | PORTABLE | `outram-mc-libs` already has an LCG in WGSL |
| `fft` | 76 | PORTABLE | a classic GPU workload; multi-dispatch |
| `complex` | 59 | PORTABLE | as `vec2<f32>`; WGSL has no complex type |
| `const` | — | PORTABLE | constants only |
| `deriv` / `diff` | 6 | PORTABLE | pointwise finite differences |
| `sum` | 10 | PORTABLE | Levin u-transform |
| `spmatrix` / `spblas` | 527 | PORTABLE | CSR SpMV is a standard GPU kernel; large surface |
| `statistics` | 452 | PORTABLE (care) | reductions; a parallel tree changes summation order |
| `bspline` / `dht` / `wavelet` | 76 | PORTABLE | basis evaluation and transforms |
| `interp` | ~30 | PARTIAL | evaluation is a kernel; spline construction needs a tridiagonal solve |
| `integration` | 48 | PARTIAL | fixed rules are kernels; adaptive QAG is a host loop |
| `linalg` | 105 | PARTIAL | LU/QR/Cholesky are multi-dispatch, not one kernel |
| `eigen` | 55 | PARTIAL | same; iterative and sequential |
| `roots` / `min` | 34 | PARTIAL | one iteration is a kernel; the loop is host-side |
| `ode-initval2` | 36 | PARTIAL | one step is a kernel; step control is host-side |
| `multimin` / `multiroots` / `multifit` | 126 | PARTIAL | residual and Jacobian are kernels; the driver is not |
| `monte` | 18 | PARTIAL | sampling is a kernel; accumulation needs atomics |
| `filter` / `movstat` / `rstat` | 52 | PARTIAL | streaming state; windowed forms are portable |
| `sort` | 156 | N/A as-is | GSL's quicksort/heapsort are serial; a GPU wants bitonic, which is a *different algorithm*, not a port |
| `permutation` / `combination` / `multiset` / `block` / `bst` | ~120 | N/A | data structures and allocation |
| `histogram` / `ntuple` | 94 | N/A | accumulation into shared bins; needs atomics and is I/O-shaped |
| `err` | 10 | N/A | global error handler state; WGSL has no such mechanism |
| `ieee-utils` / `sys` | 20 | N/A | platform and IEEE introspection |
| `siman` | 2 | N/A | simulated annealing is a host-side control loop by definition |
| `fit` | 6 | PORTABLE | small linear fits |
| `test` / `doc` / `cmake` / `support` / `utils` / `ampl` | — | N/A | not library surface |

---

## Verification standard

Nothing counts as **PORTED** without all three:

1. **An `f32` CPU mirror** performing the same operations in the same order,
   so GPU-vs-mirror isolates transcription error from `f32` rounding.
2. **naga validation against baseline WebGPU capabilities**, which runs with no
   device and on every platform.
3. **A GPU dispatch test** comparing against that mirror, which skips cleanly
   where no adapter exists.

A dispatch test asserts a *budget*, not bit-identity, unless the kernel both
avoids every transcendental builtin **and** takes its coefficients from a
buffer. Both conditions, not just the first — see the inline-coefficient
effect below.

Measured so far, on `llvmpipe (LLVM 20.1.2, 256 bits)`:

| kernel | GPU vs `f32` mirror | mirror vs `f64` reference |
|---|---|---|
| Horner | **0** bit-identical | 9.468e-07 |
| Clenshaw | **0** bit-identical | 1.705e-07 |
| Legendre, 17 orders | **0** bit-identical | 2.719e-06 |
| `erf` / `erfc` | 5.960e-08 / 1.192e-07 | 8.368e-06 |
| `lngamma` / `gamma` | 3.418e-06 / 9.107e-06 | 2.923e-05 (reflection branch) |
| `psi` / `psi_1` | 1.228e-07 / 2.350e-07 | 1.152e-07 / 2.559e-07 |
| `psi_1piy` / `hzeta` | 2.445e-07 / 5.674e-07 | 7.540e-08 / 2.763e-07 |
| `zeta` / `eta` | 1.079e-05 / 1.040e-05 | 9.391e-06 / 6.775e-06 (below zero) |
| `zetam1` | 3.725e-09 | 2.090e-07 |
| `D_1` .. `D_6` | 2.994e-07 .. 1.630e-06 | 2.784e-07 .. 1.940e-06 |
| `I_0` / `I_1` | 1.821e-06 / 1.761e-06 | 1.576e-07 / 1.748e-07 |
| `K_0` / `K_1` | 2.242e-07 / 2.812e-07 | 1.629e-07 / 1.736e-07 |
| `J_0` / `J_1` | 2.246e-07 / 5.695e-07 | 2.744e-06 / 7.354e-06 (at the zeros) |
| `Y_0` / `Y_1` | 3.980e-07 / 3.329e-07 | 1.288e-05 / 2.786e-05 (at the zeros) |
| BLAS L1 (`dot`, `nrm2`, `asum`, `iamax`) | **0** bit-identical | — |
| `gemv`, `gemm` | **0** bit-identical | see reassociation note |
| element-wise matrix ops | **0** bit-identical (exact equality) | — |

**Why `erf` is not bit-identical and everything else is.** Pure arithmetic is
pinned by IEEE-754 to a single correctly-rounded answer, so a faithful
transcription matches exactly. `erf`/`erfc` call `exp`, and WGSL specifies its
builtins to an **ULP bound rather than correct rounding** — the device's `exp`
and `libm::expf` are different functions agreeing to about an ulp. A kernel
built only from arithmetic may be held to bit-identity; one touching a
transcendental builtin may not, and asserting otherwise would fail on a
conforming device.

~~That is the whole story.~~ **CORRECTED 2026-09-19** — it is necessary but
not sufficient, and the Debye row above is the counterexample: its `x <= 4`
branch is Clenshaw over 17 constants, calls no builtin at all, and still
misses the mirror by up to 4 ulp. See **the inline-coefficient effect**
below. The original claim stands for every kernel whose coefficients reach it
through a buffer; it does not stand for one that embeds them as literals.

The size of that gap scales with how many builtins are involved: `erf` calls
only `exp` and lands at one ulp, while `lngamma` calls `log`, `exp` **and
`sin`** and lands at about thirty. The extra comes from `sin(pi * x)` in the
reflection branch, where device and `libm` argument reduction differ and the
sine's zero amplifies it — the same mechanism that makes that branch the worst
on the CPU side.

The Bessel rows show the same effect isolated cleanly. `I_0(x)` is
`exp(x) * I_0_scaled(x)`; the scaled form calls no `exp` and agrees with the
mirror to 2.364e-07, while the unscaled one picks up 1.821e-06 at `x = 38`.
WGSL allows `exp` `3 + 2|x|` ulp, which is 79 ulp there, so the measured 15 is
well inside spec. **Prefer a scaled entry point on the GPU wherever one
exists** — it is not only about range.

**The Bessel mirror-vs-`f64` column needs reading carefully.** The `J` and `Y`
figures are dominated by probes landing next to a zero, where relative error is
unbounded for any implementation in any precision — `J_1`'s worst is at
`x = 3.875` against a zero at 3.8317. Restricted to `|f| > 0.05` all four fall
to 1.8e-07 – 6.6e-07, one `f32` ulp, and the bounded quantity is the absolute
error: 9.927e-08 (`J_0`) to 7.542e-07 (`Y_1`).

A prediction was recorded before that measurement and refuted by it: the
asymptotic branch past `x = 4` evaluates `cos(x - pi/4 + theta/x)` with the
phase being `x` itself, and was expected to be visibly worse than the
Chebyshev branch below the cut. It is not — the two agree to within a factor of
four out to `x = 50`, and `J_1` is worse *below* the cut.

**`lngamma` is the least accurate kernel here, and not where it was expected
to be.** Measured per branch: the Padé windows are **exact**, the Lanczos sum
costs 7.839e-06, and the **reflection branch costs 4.040e-05** — five times
worse than the sum it was predicted to be dominated by. The cause is `f32`
argument reduction in `sin(pi * x)`, not cancellation in the Lanczos series, so
improving the series would move the headline figure very little. A caller
needing `ln Gamma` at moderately negative arguments in `f32` should expect
~1e-4.

**`zeta` and `eta` below zero are `Gamma` showing through.** Both are two
orders worse than everything else in the table, and only for `s < 0`, where
the functional equation multiplies by `Gamma(1 - s)`. The `gamma` row two
lines above measures that kernel at 9.107e-06 on its own; `zeta`'s 1.079e-05
is that figure carried through one multiplication. The positive branch, which
calls no `Gamma`, sits at 1e-07 with the rest. Improving it means improving
the `f32` gamma, not the zeta transcription.

**The inline-coefficient effect: pure arithmetic is not enough for
bit-identity.** Measured by a controlled A/B in
`the_inline_coefficient_array_is_what_costs_bit_identity` — the same 17
`adeb1_cs` coefficients, the same 64 arguments, the same Clenshaw recurrence,
differing only in how the coefficients reach the kernel:

| coefficients | bit-exact against the CPU mirror |
|---|---|
| read from the storage buffer (`petir_cheb_eval`) | **64 / 64** |
| inline `array<f32, 17>` literal (`petir_debye_cheb1`) | 34 / 64 |

The buffer-fed form is exactly right everywhere. The inline form is one ulp
low at 30 of 64 points, compounding to 4 ulp through `petir_debye`. The
transcription, the coefficients and the recurrence are all correct; what
differs is what the shader compiler may do once the operands are compile-time
constants. Measured on `llvmpipe (LLVM 20.1.2, 256 bits)`; a different driver
may fold differently, which is a further reason not to assert bit-identity on
such a kernel.

**A prediction this raises, recorded rather than asserted.** `bessel.wgsl`,
`gamma.wgsl` and `psi_zeta.wgsl` all embed their series as inline literals, so
they should show the same signature — and one figure already in the table fits
it. `I_0_scaled` calls no `exp` and still sits 2.364e-07 from its mirror, which
until now had no explanation; the paragraph above attributes the unscaled
`I_0`'s larger 1.821e-06 to `exp`, and that stands, but the scaled form's
residue was left unaccounted for. The experiment that would settle it is a
buffer-fed copy of `bi0_cs` compared the same way. Not done, so not claimed.

**Three psi/zeta entry points are deliberately not in WGSL**, and the shader
header gives each reason in full: the integer-argument lookups are 101-entry
tables WGSL would rebuild on the stack per invocation; `zeta(s)` for
`s <= -34` needs a `Gamma` past `f32::MAX` (the *answer* is usually
representable — `zeta(-35)` is about 8e14 — but the route is not, so the
kernel returns `NaN` rather than the 0 or `inf` that would fall out); and
`psi_n` for `n >= 2` needs an `n!` that leaves `f32` at `n = 34`. All three
are present in the `f64` modules.

**`psi_zeta.wgsl` is the first source with a dependency.** `petir_zeta` calls
`petir_gamma`, so `GAMMA` must be concatenated ahead of it. That is
deliberate: the alternative was a second copy of the nine Lanczos
coefficients, and a duplicated table is exactly the drift this ledger's
verification standard exists to prevent.

**The `gemm` reassociation.** GSL's `sgemm` NoTrans/NoTrans branch sweeps `k`
outermost and scatters partial products into `C`. A per-element GPU kernel
cannot: one invocation owns one output element and must sum over `k` itself.
Those are the same value in exact arithmetic and different in `f32`. The mirror
therefore uses the GPU's `k`-inner order — keeping GPU-vs-mirror exact — and
`mirror_matrix::gemm_gsl_order` implements GSL's own sweep so the difference
can be measured rather than waved at.

---

## What would change this ledger

- **`specfunc`'s remaining ~281 operations** are the largest genuinely
  portable block left, and almost all are pointwise. That is where "exhaustive"
  has the most room to move. `dilog`, `airy`, `debye`, the Fermi-Dirac and
  Bose-Einstein integrals and the Coulomb wave functions are the next blocks;
  integer-order and arbitrary-order Bessel build on the order-0/1 kernels now
  present, and much of `cdf` builds on `psi` and the incomplete gamma.
- **`cdf` (~200)** is the next, and is mostly compositions of `specfunc`.
- Anything marked **N/A** will not move without a *different algorithm*, which
  would no longer be a port. Replacing GSL's quicksort with a bitonic sort is a
  legitimate thing to want and an illegitimate thing to call a GSL port; if it
  is done, it belongs in its own module with its own provenance.
