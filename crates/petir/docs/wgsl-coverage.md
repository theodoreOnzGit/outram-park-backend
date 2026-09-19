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
| `specfunc` (dilogarithm) | ~2 | **PORTED** (real) | `Li_2(x)` for all real `x`, seven branch identities and two convergent series; **no coefficient tables at all**. The complex entry points and the `clausen` dependency under them are deliberately absent — see below |
| `specfunc` (Airy family) | ~8 | **PORTED** | `Ai`, `Bi` and both exponentially scaled forms; 13 Chebyshev series, 281 coefficients. The derivatives (`airy_der.c`) and the zeros (`airy_zero.c`) are not ported |
| `specfunc` (Lambert `W`) | ~4 | **PORTED** | `W_0` and `W_{-1}`, both real branches. **Upstream's stopping rule is corrected in two places** for `f32` — see below |
| `specfunc` (Clausen `Cl_2`) | ~2 | **PORTED** | one Chebyshev series, plus an **`f32`-redesigned argument reduction** — the `f64` three-way split of `2 pi` does not carry over. See below |
| `matrix` | 145 | **PORTED** (core) | element access, add/sub/mul/div elements, scale, add_constant, transpose |
| `vector` | 99 | **PORTED** (core) | covered by the Level-1 kernels and element access |
| `blas` | 46 | **PORTED** (real, row-major) | L1 `dot`/`nrm2`/`asum`/`iamax`; L2 `gemv` ±trans; L3 `gemm` ±trans |
| — Legendre `P_n` | — | **PORTED** | Bonnet recurrence; not a GSL module but `gsl_sf_legendre`'s subject |
| `specfunc` (rest) | ~259 | PORTABLE | the largest remaining win — almost all pointwise. the Fermi-Dirac and Bose-Einstein integrals, and the Coulomb wave functions are the next blocks; the integer-order and arbitrary-order Bessel functions build on the order-0/1 kernels already here |
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
| `Li_2`, six of seven branches | 3.815e-06 abs (whole range) | 4.244e-08 .. 6.982e-07 |
| `Ai` / `Bi` (oscillatory) | 1.839e-06 / 1.963e-06 abs | 3.558e-06 abs over `[-30, -8)` |
| `Ai_scaled` / `Bi_scaled` | 3.375e-07 / 3.137e-07 | 1.548e-07 / 1.746e-07 |
| `W_0` / `W_{-1}` | 1.383e-07 / 5.792e-07 | 1.161e-07 (`W_0` vs `f64`) |
| `Cl_2` | 7.339e-07 / 9.947e-07 abs | 4.521e-07 abs over one period |
| `Li_2`, inversion branch (`x > 2`) | — | 5.710e-06, at `Li_2`'s zero |
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

**Clausen needed its ARGUMENT REDUCTION redesigned, not transcribed.** GSL
splits `2 pi` into three `f64` pieces so each `y * Pk` subtraction is exact;
`P1` holds about 30 significant bits, which leaves no room in a 24-bit
mantissa for the period count. The replacement head is **upstream's own** —
`clausen.c` already carries `p0 = 6.28125` (201/32, eight significant bits)
for its `pi - x` reflection, so the split reuses it and adds a remainder.

Measured against a naive single-`f32` `2 pi` reduction, through `sin`:

| `theta` | split | naive | ratio |
|---|---|---|---|
| 1e2 | 2.384e-07 | 4.682e-05 | 196 |
| 1e4 | 9.783e-07 | 5.350e-03 | **5468** |
| 2.6e5 .. 5.2e5 | 7.774e-06 | 2.893e-02 | ~3700 |

It holds near 8e-06 **all the way to the refusal**, with no collapse inside
the usable range — because an eight-bit head keeps `y * P0` exact to 65 536
periods, `4.12e+05` in `theta`, against a cut at `5.24e+05`. The head width
and the cut are matched to within a factor of 1.3. Contrast `f64`, where the
equivalent split collapses at `1e8` against a cut at `2.8e+14`, seven decades
apart.

Two instrument notes, both from tests that failed first. `|reduce(theta) -
(theta mod 2 pi)|` is the obvious measure and is **wrong**: at a period
boundary the reduced value jumps between `0` and `2 pi`, so a probe either
side reports an error of `2 pi` when nothing is amiss — the comparison goes
through `sin`, which is continuous there. And a first sweep ran to `6.7e+05`,
*past the refusal*, and reported a collapse at `1e5` that does not exist
inside the domain.

**Lambert `W` is the first ITERATING kernel here, and it needed upstream's
stopping rule corrected in two places.** The rule is
`|t| < 10 eps max(|w|, 1/(|p| e^w))`, and at `f32` width it fails twice over:

1. `eps` is a **precision constant** whose `f64` value is unreachable. Over
   200 `W_0` probes, 142 burn the full iteration budget and the other 58 stop
   only because their step became **exactly zero** — all 58, checked — which
   satisfies any positive tolerance. The rule never terminates the loop.
2. The `1/(|p| e^w)` term makes the tolerance `O(1)` once `w` is very
   negative, and is **dropped**. At `x = -2.1e-06` it gives 5.947e-01 against
   1.885e-05 without it, over a hundred times the 0.005 step it then accepts,
   stopping an iteration early with `w` wrong in the third decimal. Over the
   whole `W_{-1}` domain: 4.769e-03 with the term against **9.982e-07**
   without, for one extra iteration, with `W_0` untouched at 3.071e-07 either
   way.

**That makes four distinct kinds of constant decision across this ledger, and
they do not generalise to each other:**

| kernel | constant | what it is FOR | call |
|---|---|---|---|
| `debye` | `xcut` | `f64` **exponent range**, sets a loop length | retarget (50x) |
| `dilog` | Taylor cut | a **truncation order** against epsilon | keep, gain is 1.18x |
| `airy` | `Bi` overflow guard | a **range guard** on a representable quantity | keep — retargeting discards answers |
| `lambert` | stopping rule | a **precision constant** *and* an inadequate **formula** | retarget one, change the other |

The rule is *know what upstream's constant is FOR*. Knowing what it equals
tells you nothing about whether it survives the change of width.

**`Ai` and `Bi` are measured in absolute error for the same reason `dilog`
is** — both oscillate through infinitely many zeros below `x = -1`, so a
relative figure over that range measures the probe grid. `mirror_airy` records
the per-branch relative numbers where they mean something: one to two `f32`
ulp everywhere except the oscillatory branch.

**The oscillatory branch's error is irreducible in `f32`, and that is
measured rather than asserted.** Running the identical modulus/phase formula
in `f64` on the *same* `f32` coefficients isolates the arithmetic: the
modulus stays flat at one ulp from `x = -2` to `x = -50`, while the phase's
absolute error grows linearly with `theta` and stays at **0.21 to 0.52 of one
ulp of `theta` itself**. A phase good to a fraction of its own ulp cannot be
improved at that width, and `cos`/`sin` turn its absolute error into a
comparable relative error in the answer.

**A third threshold case, and it points the opposite way to `debye`'s.**
`Bi`'s overflow guard carries GSL's `f64` constant. Retargeting it to the
`f32` analogue of the same inequality was predicted necessary — and is
**wrong**: over `x` in (25.87, 26.07) it returns `+inf` for values between
3.6e+37 and 7.8e+37, all representable in an `f32` whose maximum is 3.4e+38.
Upstream's constant never fires before `exp` genuinely overflows.

So the three cases disagree, deliberately: `debye`'s `xcut` comes from `f64`'s
**exponent range** and controls a loop length, so retargeting was worth 50x;
`dilog`'s cut comes from a **truncation order**, so retargeting is neutral at
best; `airy`'s is a **range guard on a quantity still representable**, so
retargeting loses answers. The rule is *know what upstream's constant is FOR*,
which is a stronger requirement than knowing what it equals.

**`dilog` is measured in ABSOLUTE error, and that is not a softer standard.**
`Li_2` has a real zero at `x = 12.595170`, inside any useful probe range and
squarely inside the `x > 2` inversion branch, where `(1/2) ln^2 x` passes
through `pi^2/3`. Relative error is unbounded at a zero for any implementation
in any precision, so a relative figure over a range containing it measures
where the probe grid fell. The per-branch relative numbers, taken where they
mean something (`|Li_2| > 0.1`), are in `mirror_dilog`: six of the seven
branches sit at one `f32` ulp and the inversion branch at 5.710e-06.

**A prediction recorded and refuted, worth keeping beside `debye`'s.**
`dilog`'s accelerated series switches at `x = 0.01` between a logarithm and
its Taylor expansion. By analogy with `debye.wgsl` — where two `f64`
thresholds genuinely had to be retargeted — that cut was expected to be far
too small in `f32`. Measured across six values, the best available gain is
**1.18x**, and past 0.25 the error grows five-fold. Upstream's value is kept.

The rule the two cases together support is *"know which of upstream's
constants still mean something in `f32`"*, not "always retarget". Debye's
`xcut` is derived from `f64`'s **exponent range**, which `f32` does not share,
so it had to move; `dilog`'s cut is derived from a **truncation order against
epsilon**, and moving it buys nothing because the error lives elsewhere. A
first draft of `dilog`'s note claimed the sweep was flat, on two measured
points with the other four assumed between them; the assertion that pins the
claim failed on the third and is why the table above has six rows.

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
