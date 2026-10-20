# GNU Octave reference output for PETIR's V&V

Output produced by **running GNU Octave itself**, the code-to-code oracle for
the parts of `petir` ported from Octave. Lives at the repository root like
`../gsl/`, so it is git-tracked but never in a published crate tarball.

## `expm-octave-11.3.0-reference.txt` — `expm` (gh:#993)

- **Program:** GNU Octave 11.3.0 (x86_64-pc-linux-gnu, Arch Linux package),
  LAPACK 3.12.0, reference BLAS 3.12.1 (`version("-blas")` reports "unknown
  or reference BLAS").
- **Source ported:** `scripts/linear-algebra/expm.m` as installed at
  `/usr/share/octave/11.3.0/m/linear-algebra/expm.m`, sha256
  `3f97a52a9c9ef91340d72f80438577fadc53438db7e6b6b914ddc6b65bef7345`.
  Copyright (C) 2008-2026 The Octave Project Developers, GPL-3.0-or-later
  (read from the file's header). Not vendored: PETIR does not vendor Octave
  (`crates/petir/upstream_source/README.md`), so the version, path and digest
  are recorded instead.
- **Built:** 2026-10-20.
- **Driver:** `expm_reference_driver.m` in this directory. It contains
  `expm_nobal`, a copy of `expm.m` whose only change is that the balancing
  step (LAPACK `dgebal` through Octave's `balance`) and its inverse are
  removed — the deviation PETIR's port makes. Regenerate with

  ```sh
  octave-cli --no-gui reference-data/octave/expm_reference_driver.m \
      > reference-data/octave/expm-octave-11.3.0-reference.txt
  ```

- **Format:** `CASE <name> <n>`, then `A`, `R` (stock `expm`) and `RNB`
  (`expm_nobal`) entries as `<tag> <i> <j> <value>`, zero-based, `%.17e`.

### What the comparison found (2026-10-20)

PETIR agrees with `expm_nobal` to at worst 3.2e-16 normwise over fourteen
cases (127/220 entries bit-identical); balancing is a no-op on thirteen of
them, and on a badly scaled 3x3 it moves Octave's answer by 1.1e-11 normwise,
which is then exactly PETIR's difference from stock `expm`. Replayed by
`crates/petir/tests/octave_expm_code_to_code.rs`.
