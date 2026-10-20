# CORE-MATH `hypot` correct-rounding reference for PETIR's V&V

Output of GNU MPFR, used to verify that `petir::hypot` (a port of CORE-MATH's
correctly rounded `cr_hypot` / `cr_hypotf`, gh:#997) is correctly rounded.
Like `../gsl/`, this lives outside `crates/`, so it is git-tracked but never
part of a published crate tarball.

## Provenance

- **Ported upstream:** CORE-MATH, <https://gitlab.inria.fr/core-math/core-math>,
  commit `7ebab73913c5a33a25402ee51c96ec06b3a18095` (2026-10-20), files
  `src/binary64/hypot/hypot.c` (Copyright (c) 2022-2026 Alexei Sibidanov) and
  `src/binary32/hypot/hypotf.c` (Copyright (c) 2022-2025 Alexei Sibidanov).
  **Licence verified by reading** the repository's `LICENSE` and both file
  headers: MIT (<https://spdx.org/licenses/MIT>). Accessed 2026-10-20.
- **Reference library:** GNU MPFR 4.2.2 (system package), `mpfr_hypot`.
- **Platform compared:** glibc 2.44, x86_64-unknown-linux-gnu.
- **Worst-case inputs:** CORE-MATH's own `hypot.wc` (33 174 inputs after
  dropping comment lines) and `hypotf.wc` (7 244), read from the same commit.
  They are not copied here; every 16th one is, with its MPFR result, as an
  `E` line.

## Files

- `hypot_mpfr_driver.c` — the driver. For every input it compares PETIR's
  result, glibc's and the `libm` crate's (musl) against MPFR and counts the
  mismatches (`# SUMMARY` lines). Generators are splitmix64; the Rust test
  `crates/petir/tests/hypot_correct_rounding.rs` regenerates them bit for bit.
- `petir_hypot_shim.rs` — exports `petir::hypot::{hypot, hypotf}` and
  `libm::{hypot, hypotf}` under C names, so the driver calls the Rust code.
- `build_and_run.sh` — builds both and writes the reference.
- `hypot-mpfr-reference.txt` — the output: `H` lines (one per 10^6-input
  sweep, with an FNV-1a hash of MPFR's results), `E` lines (explicit inputs
  and MPFR results), `# SUMMARY` lines.

```sh
git clone https://gitlab.inria.fr/core-math/core-math.git /tmp/core-math
cargo build --release -p petir
reference-data/core-math/build_and_run.sh /tmp/core-math 1000000 \
    reference-data/core-math/hypot-mpfr-reference.txt
```

## What it found (2026-10-20)

PETIR: **0 misroundings in 8 044 838 inputs** (binary64 and binary32). glibc
2.44's binary64 `hypot` is not correctly rounded (23 per million uniform
inputs, 6884 of 33 174 worst cases); the `libm` crate's misrounds far more
often. Full table in the test's doc comment.
