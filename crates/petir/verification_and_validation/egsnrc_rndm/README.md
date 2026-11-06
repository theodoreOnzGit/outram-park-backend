# EGSnrc RANMAR and RANLUX: PETIR against upstream compiled and run

gh:#1289. Ports: `src/rng/ranmar.rs` (from `HEN_HOUSE/egs++/egs_rndm.{h,cpp}`)
and `src/rng/ranlux.rs` (from `HEN_HOUSE/src/ranlux.mortran` and
`ranlux.macros`). EGSnrc commit `bd49a47aae3938f49f4100b8e018091861408b19`
(2026-09-28), AGPL-3.0-or-later.

Research, education and V&V only (see `RESPONSIBLE_USE.md`).

## Prediction (written 2026-11-05, before the comparison was run)

**Every stream bit-identical, in every case.** Reasons:

- Both generators are integer recurrences (`int` in C++, `integer*4` in
  Fortran). Integer arithmetic has no rounding, so the state sequence either
  matches exactly or is wrong.
- RANMAR's outputs are `twom24*r` (exact: `r < 2^24`) and, in high
  resolution, `+ twom24*twom24*r`. `twom24*twom24` is the exact power 2^-48
  and the product is exact; the one rounding is the final `f64` addition,
  and the sum of two 24-bit pieces 24 binary places apart needs 48 bits, so
  that addition is exact too.
- RANLUX's outputs are `real*4` expressions (`twom24` is `real*4` upstream).
  `uni*twom24` is exact. The refinement `uni*twom24 + seeds(j24)*twom24*twom24`
  rounds once, in `f32`, round-to-nearest-even; the port does the same `f32`
  operations in the same order. `-ffast-math` (EGSnrc's shipped flags) could
  reassociate it, but every reassociation (factoring out `twom24`) differs only
  by an exact power-of-two scaling, so it rounds identically.
- `getAzimuth` is `+ - * /` on the same inputs: IEEE-exact operations, so
  identical.

**The one possible exception:** `getGaussian` calls `log`. Upstream links
glibc's `log`; the port calls `petir::real::ln`, which is PETIR's port of ARM
optimized-routines' `log`. glibc's `log` has been the ARM optimized-routines
code since glibc 2.28, so I expect identity there too, but if a difference
appears it would be 1 ulp in `r = sqrt(-2 log(1-u))` and would be explained by
a different `log`. (`sqrt` is IEEE-exact.)

> **CORRECTED 2026-11-05, after the run:** the prediction misstated the code
> it was about. When it was written, `get_gaussian` called `Real::ln`, which
> is the `libm` crate's `log`, not `petir::real::ln`. See Results.

## Methodology

**Upstream, built and run** (`harness/`, our code; `build.sh <EGSnrc checkout>
<work dir>` re-runs everything, nothing upstream is copied into git):

- RANMAR: `harness.cpp` `#include`s `egs++/egs_rndm.cpp` unmodified, so the
  file-local `EGS_Ranmar` can be built with any seeds, array size and
  resolution; `defaultRNG` is called as upstream ships it. `g++ -O3`
  (EGSnrc's `egspp_gcc64` setting). `include/egs_config1.h` stands in for the
  configure-generated header (`EGS_Float = double`, the template's default).
- RANLUX: egs++ has **no** RANLUX (`egs_rndm.cpp`: "should offer
  implementation of RANLUX in a future release"). EGSnrc's RANLUX is
  `src/ranlux.mortran` (+ `ranlux.macros`), the default RNG of its Mortran
  codes. The harness builds EGSnrc's Mortran3 from `mortran3/mortran3.f`,
  translates `ranlux.macros` + `ranlux.mortran` with it, compiles the
  Fortran with EGSnrc's shipped gfortran flags (`-fPIC -O3 -ffast-math`) and
  calls `ranlux` / `init_ranlux` / `get_ranlux_state` / `set_ranlux_state`
  from C++. The 24-number buffer of `$RANDOMSET` is reproduced in the
  harness (`LuxBuffer`).
- Toolchain: GCC / gfortran 15, glibc 2.44, x86-64 (i9-13900K), 2026-11-05.
- Cross-check of the harness itself: RANLUX luxury 1 and 4, seed 1, give
  exactly the four sums upstream's own `user_codes/ranlux_test` says they
  "should be" (e.g. 500181.82346285 and 500049.78976676 for 10^6 numbers).
- Control: the Fortran rebuilt at `-O0` gives a byte-identical
  `reference.txt`, so `-ffast-math` changes nothing.

**Cases** (44): RANLUX at luxury 0-4 x seeds {0 (= default 314159265), 1,
12345, 314159265, 2147483647}, the not-initialised path, luxury 7 (-> 1),
seed -5 (-> default), and a `get/set_ranlux_state` round trip; RANMAR with
seeds (1802, 9373), (1, 1), (31327, 30080) (the largest allowed), (0, 40000)
(clamped), (12345, 6789), standard and high resolution, array sizes 1, 97,
128 and 1000, `defaultRNG(0)` and `defaultRNG(100000)` (the carry into the
first seed); `getGaussian` (200 000) and `getAzimuth` (100 000 pairs) on the
default stream; `saveState` / `resetState` / `getCopy`. Every uniform stream
is 10^6 draws, which crosses 7 812 RANMAR refills (n = 128) and 41 666
RANLUX refills.

**Committed reference** (`reference.txt`, made by the harness): per case the
exact bits of draws 0-47, 120-136, 990-1010 and the last 10 (each crosses at
least one refill), FNV-1a-64 digests of all draws' bits after 10^3, 10^5
and all, and the final counters (RANMAR) or RANLUX state.

**Comparison:** `tests/egsnrc_rndm_vs_upstream.rs`.
`reference_streams_are_bit_identical` (runs in the normal suite) regenerates
every stream, requires every value and digest to match exactly, and requires
each uniform stream drawn through PETIR's `Rng` as `random::<f64>()` to equal
the `get_uniform` stream bit for bit. `full_dumps_are_bit_identical`
(ignored; `EGSNRC_RNDM_DUMP_DIR=<work dir>`) compares every draw of the binary
dumps and prints the table below. Pass criterion: bit identity.

## Results (2026-11-05)

Against `reference.txt` (glibc's FMA `log` variants masked, see below):

| case | draws compared | bit-identical | worst difference | cause |
|---|---|---|---|---|
| RANLUX, luxury 0-4 x 5 seeds (25 cases) | 25 x 10^6 | 25 x 10^6 | 0 | |
| RANLUX not-initialised, luxury 7, seed -5 (3) | 3 x 10^6 | 3 x 10^6 | 0 | |
| RANLUX state round trip (300 after restore, + state) | 300 + 26 integers | all | 0 | |
| RANMAR standard resolution, 7 seedings incl. `defaultRNG` and n = 97 | 7 x 10^6 | 7 x 10^6 | 0 | |
| RANMAR high resolution, n = 1, 128 (x3), 1000 (5) | 5 x 10^6 | 5 x 10^6 | 0 | |
| RANMAR counters after 10^6 draws (12 cases) | 24 integers | all | 0 | |
| RANMAR `saveState`/`resetState`/`getCopy` | 300 | 300 | 0 | |
| `getAzimuth` | 200 000 | 200 000 | 0 | |
| `getGaussian` | 200 000 | 200 000 | 0 | |
| uniform streams through `Rng::random::<f64>()` (40 cases) | 40 x 10^6 | 40 x 10^6 | 0 | |

**The prediction held for RANMAR, RANLUX and `getAzimuth`, and the stated
exception occurred for `getGaussian`, with a cause different from the one
first guessed.** The first run differed in 6 688 of 200 000 Gaussians (worst
4.0e-16 relative). The cause: `Real::ln` is the `libm` crate's (musl) `log`,
not the ARM optimized-routines port; I had assumed it was. Replaying with
glibc's `log` gave 200 000/200 000. Switched to `petir::real::ln` (the ARM
port, portable non-FMA form): 199 993/200 000 against the default run, worst
2.2e-16 relative. Of 100 000 `log` calls, 9 rounded differently from glibc.

Those 9 come from **upstream depending on the machine**: on x86-64, glibc
selects an FMA-contracted `log` when the CPU has FMA (`fast_log.rs` documents
this). Re-running the unchanged harness binary with
`GLIBC_TUNABLES=glibc.cpu.hwcaps=-FMA,-AVX2,-FMA4` changed upstream's own
Gaussian stream, and PETIR then matched 200 000/200 000. Every other stream
was byte-identical between the two runs. `build.sh` therefore runs both;
the masked run is the committed reference. Against the FMA run (`<work
dir>/fma_variant/`): 7 of 200 000 Gaussians differ, worst 2.2e-16 relative,
cause the FMA `log`. That is below the 1e-8 allowance and its cause is
explained, and the portable build is bit-identical. Flagged for the
maintainer.

## The `Rng` adapter (PETIR's, not upstream)

EGSnrc has no integer output. Every EGSnrc uniform is an exact multiple of
2^-48 in `[0, 1)`, so `next_u64` returns `x * 2^64` (exact) and `next_u32`
`floor(x * 2^32)`, one draw per word. Then `random::<f64>()` returns exactly
`getUniform()` (pinned above, 40 x 10^6 draws), which is what ported EGSnrc
code drawing through `Rng` needs. The cost: the low bits of every word are
zero (40 for a 24-bit draw, 16 for a 48-bit one). PETIR's `Normal` /
`standard_normal` read the low 8 bits for the ziggurat layer, so **they must
not be fed RANMAR or RANLUX**; use `Ranmar::get_gaussian`. `fill_bytes` gives
the top 24 bits of each draw as 3 bytes.

## Observation on upstream, not ported: text state round trip

`EGS_RandomGenerator::storeState` writes the buffered numbers with the
stream's default precision (6 significant digits). The harness stored a
RANMAR after 1000 draws and restored it with `setState(istream&)`: 24 of the
next 300 draws (the rest of the buffer) differed, worst 3.0e-6 relative.
PETIR ports the state's **content** as `RanmarState`, exactly; it does not port
the text format.

## Interpretation

PETIR's RANMAR and RANLUX reproduce EGSnrc's streams bit for bit, and so do
the Gaussian and azimuth samplers on a portable glibc build. This is
verification (code-to-code). It says nothing about the generators' statistical
quality beyond what EGSnrc's own documentation states (for example, RANLUX
luxury 0 "fails many tests").
