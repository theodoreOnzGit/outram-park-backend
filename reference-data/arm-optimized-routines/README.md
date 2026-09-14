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
  on `x86_64-unknown-linux-gnu`, from `math/{exp,exp_data,math_err}.c`.
- **Driver:** `exp_reference_driver.c` in this directory, committed so the file
  can be regenerated rather than trusted.

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

gcc -O2 -I$A/include -c $A/exp.c -o /tmp/ax.o
objcopy --redefine-sym exp=arm_exp \
        --redefine-sym __ieee754_exp=arm_ieee754_exp \
        --redefine-sym __exp_finite=arm_exp_finite /tmp/ax.o /tmp/ax2.o
gcc -O2 -I$A/include -c $A/exp_data.c -o /tmp/axd.o
gcc -O2 -I$A/include -c $A/math_err.c  -o /tmp/axe.o
gcc -O2 $R/exp_reference_driver.c /tmp/ax2.o /tmp/axd.o /tmp/axe.o -o /tmp/expdriver -lm

/tmp/expdriver > $R/exp-arm-optimized-routines-reference.txt
```

## What is in it

8 345 probes, one per line, as `<x bits> <exp(x) bits>` in hex64 — raw IEEE-754
encodings so the check is bit-for-bit with no decimal round-trip in the way:

- every 5th point of `k * 0.035, k in [-20000, 20000]` — the ordinary
  table-driven path across the whole finite range;
- `±2^e` for `e in [-60, 9]` — crosses the tiny-`x` early return
  (`abstop - top12(0x1p-54) >= 0x80000000`);
- `709.0 ± d*0.02` and `-745.0 ± d*0.02`, `d in [-50, 50]` — the overflow and
  underflow boundaries, where `specialcase` runs and the subnormal
  pre-rounding step matters;
- `+0.0` and `-0.0`.

Lines beginning `#` are comments.
