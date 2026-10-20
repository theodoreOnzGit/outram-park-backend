# Reference-LAPACK output for PETIR's V&V

Output of **Reference-LAPACK run**, the code-to-code oracle for
`petir::linalg::gebal` (LAPACK `DGEBAL` / `DGEBAK`, gh:#998). Lives outside
`crates/` like `../gsl/`, so it is git-tracked but never in a published
crate tarball.

## Provenance

- **Library run:** the system `liblapack.so.3` and `libblas.so.3` of Arch
  Linux packages `lapack 3.12.1-2` and `blas 3.12.1-2` (upstream URL
  <https://www.netlib.org/lapack>, i.e. the Netlib reference build, not
  OpenBLAS). GNU Octave 11.3.0 links exactly these two here (`ldd` on
  `liboctave.so`), so this is also the `balance` that Octave's `expm` calls.
- **Source ported and cross-checked:** Reference-LAPACK tag `v3.12.1`, commit
  `6ec7f2bc4ecf4c4a93496aa2fa519575bc0e39ca` (2024-12-03),
  <https://github.com/Reference-LAPACK/lapack>: `SRC/dgebal.f`,
  `SRC/dgebak.f`, `BLAS/SRC/dnrm2.f90`, `BLAS/SRC/idamax.f` (and
  `dscal.f`, `dswap.f`, `disnan.f`, `dlaisnan.f`, `INSTALL/dlamch.f`,
  `lsame.f`, `xerbla.f`, compiled but not ported). Licence: modified BSD,
  read from the repository's `LICENSE`, 2026-10-20.
- **Built:** 2026-10-20, `gcc -O2` / `gfortran -O2`, x86_64-unknown-linux-gnu.

## Files

- `gebal_reference_driver.c` — calls `DGEBAL` for every `JOB` and `DGEBAK`
  for both `SIDE`s on 68 cases (the Octave `expm` inputs, read from
  `../octave/expm-octave-11.3.0-reference.txt`, plus generated ones; see the
  driver header).
- `build_and_run.sh` — builds the driver against the system library (the
  committed output) AND against the v3.12.1 sources compiled with gfortran,
  and fails unless the two outputs are byte-identical.
- `gebal-lapack-3.12.1-reference.txt` — the output.

Regenerate from the repository root, with a checkout of the `v3.12.1` tag
at `<lapack>`:

```sh
reference-data/lapack/build_and_run.sh <lapack> \
    reference-data/lapack/gebal-lapack-3.12.1-reference.txt
```

## What the comparison found (2026-10-20)

The system library and the compiled v3.12.1 sources agree byte for byte.
PETIR's port agrees with them on **26 024/26 024 values**. Replayed by
`crates/petir/tests/lapack_gebal_code_to_code.rs`.
