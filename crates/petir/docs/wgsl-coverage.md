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
| `matrix` | 145 | **PORTED** (core) | element access, add/sub/mul/div elements, scale, add_constant, transpose |
| `vector` | 99 | **PORTED** (core) | covered by the Level-1 kernels and element access |
| `blas` | 46 | **PORTED** (real, row-major) | L1 `dot`/`nrm2`/`asum`/`iamax`; L2 `gemv` ±trans; L3 `gemm` ±trans |
| — Legendre `P_n` | — | **PORTED** | Bonnet recurrence; not a GSL module but `gsl_sf_legendre`'s subject |
| `specfunc` (rest) | ~319 | PORTABLE | the largest remaining win — almost all pointwise. Bessel `J`/`Y`/`I`/`K`, `psi`, `zeta`, `dilog`, `airy`, `debye` are the next blocks |
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

Measured so far, on `llvmpipe (LLVM 20.1.2, 256 bits)`:

| kernel | GPU vs `f32` mirror | mirror vs `f64` reference |
|---|---|---|
| Horner | **0** bit-identical | 9.468e-07 |
| Clenshaw | **0** bit-identical | 1.705e-07 |
| Legendre, 17 orders | **0** bit-identical | 2.719e-06 |
| `erf` / `erfc` | 5.960e-08 / 1.192e-07 | 8.368e-06 |
| `lngamma` / `gamma` | 3.418e-06 / 9.107e-06 | 2.923e-05 (reflection branch) |
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

The size of that gap scales with how many builtins are involved: `erf` calls
only `exp` and lands at one ulp, while `lngamma` calls `log`, `exp` **and
`sin`** and lands at about thirty. The extra comes from `sin(pi * x)` in the
reflection branch, where device and `libm` argument reduction differ and the
sine's zero amplifies it — the same mechanism that makes that branch the worst
on the CPU side.

**`lngamma` is the least accurate kernel here, and not where it was expected
to be.** Measured per branch: the Padé windows are **exact**, the Lanczos sum
costs 7.839e-06, and the **reflection branch costs 4.040e-05** — five times
worse than the sum it was predicted to be dominated by. The cause is `f32`
argument reduction in `sin(pi * x)`, not cancellation in the Lanczos series, so
improving the series would move the headline figure very little. A caller
needing `ln Gamma` at moderately negative arguments in `f32` should expect
~1e-4.

**The `gemm` reassociation.** GSL's `sgemm` NoTrans/NoTrans branch sweeps `k`
outermost and scatters partial products into `C`. A per-element GPU kernel
cannot: one invocation owns one output element and must sum over `k` itself.
Those are the same value in exact arithmetic and different in `f32`. The mirror
therefore uses the GPU's `k`-inner order — keeping GPU-vs-mirror exact — and
`mirror_matrix::gemm_gsl_order` implements GSL's own sweep so the difference
can be measured rather than waved at.

---

## What would change this ledger

- **`specfunc`'s remaining ~329 operations** are the largest genuinely
  portable block left, and almost all are pointwise. That is where "exhaustive"
  has the most room to move.
- **`cdf` (~200)** is the next, and is mostly compositions of `specfunc`.
- Anything marked **N/A** will not move without a *different algorithm*, which
  would no longer be a port. Replacing GSL's quicksort with a bitonic sort is a
  legitimate thing to want and an illegitimate thing to call a GSL port; if it
  is done, it belongs in its own module with its own provenance.
