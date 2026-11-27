# TetGen predicates, bit for bit against upstream (gh:#1790)

Stage 1 of the TetGen 1.6.0 port (epic gh:#1789): `src/tetgen/predicates.rs`
is a line-for-line port of TetGen's `predicates.cxx` (Shewchuk's
public-domain adaptive-precision predicates, with Hang Si's static filter and
`-X` options). This record checks it against upstream to the bit.

> Research, education and V&V only; see `RESPONSIBLE_USE.md`. AI-written,
> not yet human-reviewed.

## Methodology

**Reference.** Upstream `predicates.cxx` from `libigl/tetgen` at
`e05aca7df74e` (TetGen 1.6.0, AGPL-3.0-or-later), compiled unmodified.
`gen_vectors.cxx` (our harness) `#include`s it into one translation unit, so
it can also read the file's `static` constants after `exactinit()`.

**Compiler.** g++ 13.3.0 on x86_64 (SSE2 doubles, no x87), with
`-std=c++17 -ffp-contract=off -fno-fast-math`, so no `a * b + c` is fused
into an FMA (Rust never contracts either). Three builds write the vector
file, and `run.sh` checks they agree byte for byte:

| build | flags | role |
|---|---|---|
| `gen_O2` | `-O2` | the committed vectors |
| `gen_O0` | `-O0` | upstream's own makefile compiles the predicates at `-O0` |
| `gen_count` | `-O2 -DCOUNT_STAGES`, on `instrument.awk`'s copy | stage counts |

All three agreed on 2026-11-26 (2 222 160 bytes, sha256 prefix
`3cf30f717c302f44`). The counted copy only wraps each `return` of the six
adaptive functions in `{ TG_HIT(fn, k); return ...; }`; upstream itself is
never edited.

**`exactinit` configurations.** 12 extent triples and option pairs: the
three evaluation modes (static filter on, TetGen's default; filter off,
`-X1`; inexact, `-X`; all with extents 2.7, 0.9, 1.3), the other five
orderings of those extents (the sort decides the rounding of the filter
products), and degenerate ones (0, 0, 0; 1e-300, 3e-7, 1e100). All 19
constants and flags are compared.

**Inputs.** A splitmix64 stream seeded `0x1790179017901790`; 600 cases per
category, ten categories per family.

| cat | four points (`orient3d` family) | five points + heights (`insphere`, `orient4d`) |
|---|---|---|
| 0 | uniform in [-1, 1) | uniform, random heights |
| 1 | random full mantissas, exponents -10..10 | full mantissas; heights random or the lifts |
| 2 | integer points on a plane (exact zero, exact differences) | integer points on spheres r = 3, 7, 9, 11, 15; integer lifts |
| 3 | on the plane x = y, full mantissas (exact zero, inexact differences) | five points on x = y (both determinants exactly zero) |
| 4 | `pd = pa + s(pb-pa) + t(pc-pa)` in floating point | cat 2 translated by a non-dyadic offset |
| 5 | cats 2-4 with one coordinate moved 1 ulp | cats 2-4 with one coordinate or height moved 1 ulp |
| 6 | large: x 2^200, x 1e60 | large: x 2^120, x 1e40 (heights x s^2) |
| 7 | tiny: x 2^-200, 1e-60, 2^-340 (subnormal products) | tiny: x 2^-120, 1e-40, 2^-210 |
| 8 | cat 2 translated by non-dyadic offsets (stage C) | unit-sphere points by normalisation |
| 9 | repeated points, collinear triples | heights on a hyperplane (orient4d zero) |

**Evaluated.** `orient3d`, `insphere` and `orient4d` in each of the three
modes; `orient3dfast`, `orient3dexact`, `orient2dexact` (on `pa, pb, pc`),
`insphereexact` and `orient4dexact` once (they read no option).

**File.** `test_data/tetgen_predicates/predicates_vectors.bin.gz`
(1 231 956 bytes; format in the header comment of `gen_vectors.cxx`).

**Check.** `tests/tetgen_predicates_vs_upstream.rs` reads only that file (no
compiler, so it runs on Android) and asserts `to_bits()` equality of every
constant and every result. Pass criterion: all identical, no tolerance.

**Can it fail?** Yes: deleting one term (`+ cdztail * (adx * bdy - ady * bdx)`)
from `orient3dadapt`'s stage C made 1354 of 84 000 results differ
(2026-11-26, reverted).

## How to regenerate

```bash
git clone https://github.com/libigl/tetgen && git -C tetgen checkout e05aca7df74e
crates/outram-park-fork-cfmesh/verification_and_validation/tetgen_predicates_gh1790/run.sh tetgen [build-dir]
cargo test --release -p outram-park-fork-cfmesh --test tetgen_predicates_vs_upstream -- --nocapture
```

## Results (2026-11-26)

**Every result bit-identical.** `exactinit`: 12 of 12 configurations, 228 of
228 constants. Predicates, identical / cases:

| predicate | filter on | filter off (`-X1`) | inexact (`-X`) |
|---|---|---|---|
| `orient3d` | 6000 / 6000 | 6000 / 6000 | 6000 / 6000 |
| `insphere` | 6000 / 6000 | 6000 / 6000 | 6000 / 6000 |
| `orient4d` | 6000 / 6000 | 6000 / 6000 | 6000 / 6000 |
| `orient3dfast` | 6000 / 6000 | | |
| `orient3dexact` | 6000 / 6000 | | |
| `orient2dexact` | 6000 / 6000 | | |
| `insphereexact` | 6000 / 6000 | | |
| `orient4dexact` | 6000 / 6000 | | |

Total 84 000 / 84 000. No difference was found, so there is no cause to
report.

### Every adaptive stage is reached

Return sites hit, from `gen_count` (calls of each function per mode):

| function | return site | filter on | filter off | inexact |
|---|---|---|---|---|
| `orient3d` | inexact (`-X`) | 0 | 0 | 6000 |
| | static filter, `det > f` | 1132 | 0 | 0 |
| | static filter, `det < -f` | 1037 | 0 | 0 |
| | stage A | 201 | 1604 | 0 |
| | to `orient3dadapt` | 3630 | 4396 | 0 |
| `orient3dadapt` | stage B | 255 | 281 | 0 |
| | all tails zero | 650 | 722 | 0 |
| | stage C | 1307 | 1969 | 0 |
| | exact tail | 1418 | 1424 | 0 |
| `insphere` | inexact (`-X`) | 0 | 0 | 6000 |
| | static filter, `fabs(det) > f` | 2285 | 0 | 0 |
| | stage A | 229 | 2251 | 0 |
| | to `insphereadapt` | 3486 | 3749 | 0 |
| `insphereadapt` | stage B | 165 | 168 | 0 |
| | all tails zero | 834 | 861 | 0 |
| | stage C | 1204 | 1431 | 0 |
| | to `insphereexact` | 1283 | 1289 | 0 |
| `orient4d` | stage A | 1875 | 1875 | 1875 |
| | to `orient4dadapt` | 4125 | 4125 | 4125 |
| `orient4dadapt` | stage B | 191 | 191 | 191 |
| | all tails zero | 1072 | 1072 | 1072 |
| | stage C | 1583 | 1583 | 1583 |
| | to `orient4dexact` | 1279 | 1279 | 1279 |

The zeros are the sites a mode cannot reach (the filter with the filter off,
anything past the `-X` return in inexact mode). `orient4d` reads neither
option in upstream, so its three columns are equal.

## Interpretation and limits

- The port reproduces upstream's values, not only its signs, on every path,
  which is what TetGen's later stages need (it uses `orient3d` as a volume).
- Checked on x86_64 only. The Rust side is IEEE binary64 with no
  contraction on every target, so the result should not depend on the
  platform, but the test has not been run on aarch64 or wasm32 here (they
  are compile-checked only).
- NaN and infinite inputs are not in the vectors. Overflowing products are
  avoided by the chosen scales; subnormal products are included (cat 7).
- Not ported, because nothing listed calls them: `Square`, `Two_Two_Sum`,
  the `Four_*` / `Eight_*` macros, `grow_expansion`, `compress`, the `*slow`
  and incircle routines, and the CGAL branch. `inspherefast` is not ported
  either (TetGen does not declare it).
