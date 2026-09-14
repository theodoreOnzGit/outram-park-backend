# ARM optimized-routines reference output for PETIR's V&V

Numerical output produced by **compiling and running ARM's optimized-routines
itself**, used as the code-to-code oracle for `petir::fast_exp`. Like
`../gsl/`, `../endf/` and `../leapr/`, this lives at the repository root,
outside `crates/`, so it is git-tracked but never part of a published crate
tarball.

## Provenance

- **Library:** ARM optimized-routines, commit
  `f2e4faf58c6c671154f472a76eaaa977bb36c870` (2026-09-09), read 2026-09-14.
  Licence: MIT OR Apache-2.0 WITH LLVM-exception — see
  `crates/petir/upstream_source/README.md` for the verification record.
- **Built:** 2026-09-14, `gcc -O2`, **no `-mfma`, no `-march=native`**, glibc
  on `x86_64-unknown-linux-gnu`, from
  `math/{exp,exp_data,log,log_data,pow,pow_log_data,math_err}.c`.
- **Drivers:** `exp_reference_driver.c`, `log_reference_driver.c` and
  `pow_reference_driver.c` in this directory, committed so each file can be
  regenerated rather than trusted.

## The three oracles

| file | probes | covers |
|---|---|---|
| `exp-arm-optimized-routines-reference.txt` | 8 345 | `crates/petir/src/fast_exp.rs` |
| `log-arm-optimized-routines-reference.txt` | 16 751 | `crates/petir/src/fast_log.rs` |
| `pow-arm-optimized-routines-reference.txt` | 43 435 | `crates/petir/src/fast_pow.rs` |

Each line is raw IEEE-754 bit patterns in hex64 — `<x> <f(x)>`, or
`<x> <y> <pow(x,y)>` — so the check is bit-for-bit with no decimal round-trip
in the way. Lines beginning `#` are comments. All three ports pass at
**100.000 % bit-identical**; the per-test module docs in
`crates/petir/tests/fast_*_vs_arm_optimized_routines.rs` record the counts and
the probe rationale.

## Why the build must not enable FMA (measured, not assumed)

`exp_inline` evaluates its polynomial as

```c
tmp = tail + r + r2 * (C2 + r * C3) + r2 * r2 * (C4 + r * C5);
```

With `-mfma` in effect, GCC contracts each `a * b + c` into a single fused
multiply-add — one rounding instead of two. That changes the last bit of the
result on a small fraction of inputs. Measured on the 40 001-point grid
`k * 0.035, k in [-20000, 20000]`, comparing ARM's C against this host's
`glibc 2.x` `exp`:

| ARM build | differences vs glibc |
|---|---|
| `gcc -O2` (no FMA available) | **22 / 40001** (0.055 %), every one exactly 1 ULP |
| `gcc -O2 -mfma -mavx2 -ffp-contract=fast` | **0 / 40001** |
| `gcc -O2 -mfma -mavx2 -ffp-contract=off` | 22 / 40001 — identical to the no-FMA build |

So glibc on an FMA-capable x86-64 host dispatches (via ifunc) to an
**FMA-compiled** build of this same source, and the 22 differences are
contraction, not a defect in either implementation. The third row is the
control that proves it: with the hardware available but contraction disabled,
the result goes straight back to the no-FMA bit pattern.

`HAVE_FAST_FMA` does more than contraction, which is why the oracle builds for
`log` and `pow` matter even more than the one for `exp`. In `log.c` it selects
a different argument reduction (`fma(z, invc, -1)` against
`(z - chi - clo) * invc`, which needs the whole `tab2` table that the FMA build
does not even compile) *and* a different near-1 branch. In `pow.c` it selects a
different `log_inline` reduction and a different hi/lo split of `y * log(x)`.
These are different code, not different rounding.

**PETIR deliberately tracks the no-FMA row.** Rust never contracts
floating-point expressions (there is no `-ffp-contract=fast` equivalent and
`mul_add` must be written explicitly), so the portable form is the one a Rust
transcription can reproduce *on every target* — which is the whole point of
routing through PETIR. Chasing glibc's FMA bit pattern instead would mean
writing `mul_add` throughout, which lowers to a libcall (correctly rounded but
very slow) on any target without hardware FMA, trading the speed this module
exists for.

## Regenerating

The upstream symbol is `exp`, which collides with `<math.h>` in the driver, so
it is renamed after compilation rather than with `-Dexp=…` (which would break
`<math.h>` itself). Upstream's headers already namespace the internal symbols
(`__math_oflow` → `arm_math_oflow`, `__exp_data` → `arm_math_exp_data`), so
only the three public entry points need renaming.

```sh
A=crates/petir/upstream_source/ARM-optimized-routines/math    # the vendored clone
R=reference-data/arm-optimized-routines

gcc -O2 -I$A -I$A/include -c $A/exp_data.c     -o /tmp/axd.o
gcc -O2 -I$A -I$A/include -c $A/log_data.c     -o /tmp/ald.o
gcc -O2 -I$A -I$A/include -c $A/pow_log_data.c -o /tmp/apd.o
gcc -O2 -I$A -I$A/include -c $A/math_err.c     -o /tmp/axe.o

gcc -O2 -I$A -I$A/include -c $A/exp.c -o /tmp/ax.o
objcopy --redefine-sym exp=arm_exp \
        --redefine-sym __ieee754_exp=arm_ieee754_exp \
        --redefine-sym __exp_finite=arm_exp_finite /tmp/ax.o /tmp/ax2.o
gcc -O2 -I$A -I$A/include -c $A/log.c -o /tmp/al.o
objcopy --redefine-sym log=arm_log \
        --redefine-sym __ieee754_log=arm_ieee754_log \
        --redefine-sym __log_finite=arm_log_finite /tmp/al.o /tmp/al2.o
gcc -O2 -I$A -I$A/include -c $A/pow.c -o /tmp/ap.o
objcopy --redefine-sym pow=arm_pow \
        --redefine-sym __ieee754_pow=arm_ieee754_pow \
        --redefine-sym __pow_finite=arm_pow_finite /tmp/ap.o /tmp/ap2.o

gcc -O2 $R/exp_reference_driver.c /tmp/ax2.o /tmp/axd.o /tmp/axe.o -o /tmp/expdriver -lm
gcc -O2 $R/log_reference_driver.c /tmp/al2.o /tmp/ald.o /tmp/axe.o -o /tmp/logdriver -lm
gcc -O2 $R/pow_reference_driver.c /tmp/ap2.o /tmp/apd.o /tmp/axd.o /tmp/axe.o -o /tmp/powdriver -lm

/tmp/expdriver > $R/exp-arm-optimized-routines-reference.txt
/tmp/logdriver > $R/log-arm-optimized-routines-reference.txt
/tmp/powdriver > $R/pow-arm-optimized-routines-reference.txt
```

## What is in the `exp` file

8 345 probes, one per line, as `<x bits> <exp(x) bits>`:

- every 5th point of `k * 0.035, k in [-20000, 20000]` — the ordinary
  table-driven path across the whole finite range;
- `±2^e` for `e in [-60, 9]` — crosses the tiny-`x` early return
  (`abstop - top12(0x1p-54) >= 0x80000000`);
- `709.0 ± d*0.02` and `-745.0 ± d*0.02`, `d in [-50, 50]` — the overflow and
  underflow boundaries, where `specialcase` runs and the subnormal
  pre-rounding step matters;
- `+0.0` and `-0.0`.

## What is in the `log` file

16 751 probes, as `<x bits> <log(x) bits>`:

- every binade from the subnormal floor (`2^-1070`) to `2^1020`, at six
  mantissas each — the ordinary table path, plus the subnormal
  renormalisation;
- `1 + k*4e-5` for `k in [-2000, 2000]`, which straddles the near-1 window
  `[1 - 0x1p-4, 1 + 0x1.09p-4]` and both of its edges, where a different
  (`poly1`) polynomial runs;
- the subnormal floor itself, `d * 5e-324` for `d in [1, 200]`;
- `1.0`, `+0.0`, `-0.0`, `-1.0` — the exact-zero, divide-by-zero and
  invalid-operand returns.

## What is in the `pow` file

43 435 probes, as `<x bits> <y bits> <pow(x,y) bits>`:

- 400 positive bases (`i * 0.37`) against 81 exponents (`j * 0.83`, both
  signs) — the ordinary `log_inline` -> `exp_inline` path;
- 120 negative bases against every integer exponent in `[-25, 25]` plus three
  non-integer ones, exercising `checkint`'s odd/even/not-an-integer
  classification, the `SIGN_BIAS` path, and the invalid-operand return;
- bases within 3e-2 of 1.0 against exponents of `±1000`, `1e6` and `0.5` —
  the regime that makes `pow` hard, and the reason `log_inline` carries about
  15 bits beyond double precision;
- integer and half-integer exponents against `±2`, `±0.5`, `±10`, `3`;
- bases across the full binade range at step 7, crossing subnormal
  normalisation and both the overflow and underflow edges of the result;
- the full IEEE special-case cross product (`±0`, `±1`, `±2`, `±inf` against
  `±0`, `±1`, `±2`, `-3`, `±0.5`, `±inf`, `1e-20`, `1e20`) that `pow`'s
  prologue resolves before any polynomial runs.

Note the drivers never call `pow` to build their own probes — `pow` is one of
the things under test here, so every probe is built from repeated
multiplication instead.
