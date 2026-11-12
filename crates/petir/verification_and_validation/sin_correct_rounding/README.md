# `r_sin` vs glibc `sin` vs the correctly rounded sine (gh:#1455)

Research, education and V&V only (see `RESPONSIBLE_USE.md`).

## Methodology
`sinq.c` writes `sin_ref.txt`: 8000 deterministic arguments (xorshift64; a
quarter each in [0, 2 pi), [0, 100), [-500, 500), [0, 0.1)), with glibc 2.39
`sin` (what gfortran/NJOY calls) and the correctly rounded sine (libquadmath
binary128 `sinq`, rounded once to binary64; double rounding wrong only if the
sine lies within 2^-60 of a midpoint, none expected). Test:
`tests/sin_correct_rounding_vs_glibc.rs` evaluates `RealMath::r_sin` (the `libm`
crate, a musl/fdlibm port) on every argument.

Prediction (written before the run): both miss on a minority of arguments;
`libm` misses more often than glibc.

## Results (2026-11-11, glibc 2.39, gcc 13.3, x86-64)
| quantity | count of 8000 |
|---|---|
| glibc `sin` != correctly rounded | 8 (0.1 %) |
| `r_sin` != correctly rounded | 199 (2.5 %), max 1 ulp |
| `r_sin` != glibc | 201 |
| `r_sin` wrong where glibc is right | 196 |
| `r_sin` right where glibc is wrong | 5 |

## Verdict on #1455
glibc's `sin` is almost always correctly rounded (99.9 %); `libm`'s is 25x
worse (2.5 % one-ulp misses). Prediction confirmed. The one-ulp `csunr`
differences against NJOY are therefore due to `libm`'s `sin`, not to the
translation; to be bit-identical with NJOY (gfortran -> glibc) the port should
call std's `f64::sin` (glibc on this target) where NJOY calls `sin`, accepting
that results then depend on the platform libm (on wasm/musl targets the
difference reappears). Not a physics defect: <= 1 ulp.
