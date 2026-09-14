# GSL reference output for PETIR's V&V

Numerical output produced by **running the GNU Scientific Library itself**,
used as the code-to-code oracle for the `petir` port. Like `../endf/` and
`../leapr/`, this lives at the repository root, outside `crates/`, so it is
git-tracked but never part of a published crate tarball.

## Provenance

- **Library:** GSL 2.8, commit `cf180cd7fbd06039a577f9c9ff0b428784765ac1`
  (2026-05-19), read from the `ampl/gsl` mirror — see
  `crates/petir/upstream_source/README.md` for the licence verification and the
  mirror caveat.
- **Built:** 2026-09-14, `gcc -O2`, glibc on x86_64-unknown-linux-gnu, from
  `cheb/{init,eval,deriv,integ}.c` + `err/{error,stream}.c` against GSL's own
  headers.
- **Driver:** `cheb_reference_driver.c` in this directory, committed so the file
  can be regenerated rather than trusted.

## Regenerating

```sh
G=crates/petir/upstream_source/GSL          # the vendored clone
mkdir -p /tmp/gslcheb/gsl && cd /tmp/gslcheb
for h in $G/*.h $G/cheb/*.h $G/err/*.h $G/sys/*.h; do ln -sf "$h" gsl/$(basename $h); done
printf '#define HAVE_INLINE 1\n#define RETURN_IF_NULL(x) if (!x) { return ; }\n' > config.h
gcc -O2 -I. -I./gsl -o gsldriver <this dir>/cheb_reference_driver.c \
    $G/cheb/{init,eval,deriv,integ}.c $G/err/{error,stream}.c -lm
./gsldriver > cheb-gsl-2.8-reference.txt
```

## What is in it

Five cases — `sin` on `[-π, π]` at order 40, `exp` on `[-1, 2]` at order 12,
the Runge function on `[-1, 1]` at order 24, and `sin` on `[-5, 5]` at orders 1
and 2 (which exercise the `n == 1` / `n == 2` special branches of
`gsl_cheb_calc_integ`). For each: every coefficient, and at 201 points across
the interval, `gsl_cheb_eval`, `gsl_cheb_eval_err` (result and error estimate),
`gsl_cheb_eval_n`, and evaluation of the derivative and integral series.

## What the comparison found (2026-09-14)

Full pipeline — our coefficients, our evaluation — against GSL's:

```
  6114 values compared, 4707 bit-identical (77.0 %)

  coefficients     worst |delta| = 3.79e-17
  eval             worst |delta| = 8.88e-16   (exp12, where values reach e^2)
  eval_err result  worst |delta| = 8.88e-16
  eval_err abserr  worst |delta| = 5.42e-18
  eval_n(7)        worst |delta| = 2.22e-16
  integ eval       worst |delta| = 3.33e-16
  deriv eval       worst |delta| = 1.65e-14
```

The derivative's larger figure is not a defect: differentiating a Chebyshev
series amplifies coefficient error by `n^2`, and `1600 * 1e-17` is `1.6e-14`.

**Where the difference comes from — isolated, not guessed.** Feeding GSL's
*own* coefficients through *our* `eval` / `eval_err` / `deriv` / `integ`:

```
  5025 / 5025 bit-identical (100.00 %), worst |delta| = 0.000e0
```

So the evaluation arithmetic is a bit-exact transcription, and the entire
residual is in the coefficients — that is, in `cos()`. PETIR uses `libm` (a
pure-Rust port of musl's) because it is `no_std`; GSL as built here uses glibc.
The two differ in the last ulp. That is the deliberate trade recorded in the
workspace manifest: one fixed implementation buys bit-identical results across
platforms, at the cost of not matching any particular system libm exactly.
