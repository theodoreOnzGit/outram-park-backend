# Upstream source — GNU Scientific Library

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

- **Project:** GNU Scientific Library (GSL)
- **Upstream repository:** <https://git.savannah.gnu.org/git/gsl.git>
- **Cloned from (mirror):** <https://github.com/ampl/gsl> — see *Caveat* below
- **Version:** 2.8 (`AC_INIT([gsl],[2.8])` in `configure.ac`)
- **Commit at last sync:** `cf180cd7fbd06039a577f9c9ff0b428784765ac1`
- **Commit date:** 2026-05-19 (`Update changelog with 20260519 release`)
- **Accessed:** 2026-09-14
- **Clone command:** `git clone --depth 1 https://github.com/ampl/gsl.git upstream_source/GSL`

## Licence — VERIFIED, not asserted

The epic (`bn:op-chyp`) requires this be read from upstream rather than taken
from documentation, because that failure mode has already bitten this
workspace once: on 2026-09-10 `outram-park-fork-pflotran`'s NOTICE asserted
LGPL-2.1-or-later from docs and OSTI records, and reading the actual upstream
LICENSE showed LGPL-3.0.

**`COPYING` is the unmodified GNU GPL version 3 text**, dated 29 June 2007:

```
                    GNU GENERAL PUBLIC LICENSE
                       Version 3, 29 June 2007

 Copyright (C) 2007 Free Software Foundation, Inc. <http://fsf.org/>
```

- `sha256(COPYING)` = `8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903`
- size = 35147 bytes

**`COPYING` alone does NOT settle the version question.** It is the licence
*text*; the "either version 3 … or (at your option) any later version" wording
that appears at its line 639 belongs to the *"How to Apply These Terms to Your
New Programs"* appendix — boilerplate inside the licence, not GSL's own
statement of what it grants. Reading only that line is the same mistake as
reading the docs.

The grant is in the **per-file headers**. `cheb/eval.c`, verbatim:

```c
/* cheb/eval.c
 *
 * Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or (at
 * your option) any later version.
 ...
 */
```

Every file in `cheb/` carries the identical clause — `grep -h "either version"
cheb/*.c cheb/*.h | sort -u` yields exactly one line.

**Conclusion: GSL is GPL-3.0-or-later.** This workspace is GPL-3.0-only, which
is compatible: we exercise the "or later" option by choosing version 3.

### Copyright holders

GSL's copyright is held by its individual authors and the FSF, and **differs
per module** — do not copy one file's holder onto another. For the Chebyshev
module actually ported here:

| file | copyright |
|---|---|
| `cheb/eval.c`, `cheb/init.c`, `cheb/deriv.c`, `cheb/integ.c`, `cheb/gsl_chebyshev.h` | Gerard Jungman, 1996–2000 |

Re-read the header of any further file before porting it.

## Caveat — the mirror

`git.savannah.gnu.org` is **not reachable from this environment**: the outbound
proxy answers `CONNECT tunnel failed, response 403`. The clone is therefore
from the `ampl/gsl` GitHub mirror, which `bn:op-chyp.3` names as the
alternative.

What that means honestly: the licence facts above are read from the mirror's
files, and the mirror's fidelity to savannah has **not** been independently
verified here. The commit SHA and file digests are recorded precisely so that
check can be made later from a machine with savannah access. The
`sha256(COPYING)` value is the canonical unmodified GPL-3.0 text digest, which
is one independent corroboration that the licence file at least has not been
altered in the mirror.

## Files vendored for porting

Digests recorded at sync so a reviewer can diff the exact bytes that were read:

| sha256 (first 16) | file | lines |
|---|---|---|
| `3d778da026971258` | `cheb/eval.c` | 206 |
| `c68c311b2e172393` | `cheb/init.c` | 117 |
| `d5d78a00766b1b7f` | `cheb/gsl_chebyshev.h` | 134 |
| `d7c1faa167596866` | `cheb/deriv.c` | 60 |
| `b146117271b69bd9` | `cheb/integ.c` | 65 |
| `6dd4a5e9f5536b22` | `cheb/test.c` | 297 |

`cheb/test.c` is GSL's own test suite for this module. Per `bn:op-chyp`
decision 2 — *numerics are ported from a library with real V&V, never written
from scratch* — that suite is the V&V this crate inherits, and it is ported
alongside the code rather than replaced by tests written here.

## Not vendored

Octave is **not** cloned. `bn:op-chyp.3` says to decide per routine and not to
clone speculatively. Note also that Octave's special functions are largely
SLATEC-derived (`liboctave/external/slatec-fn/`); for those, port from SLATEC
directly (public domain) rather than inheriting Octave's chain.

## The clone is not committed

`/upstream_source/GSL/` is gitignored (see the crate `.gitignore`, written
*before* cloning per `bn:op-chyp.3` step 1). Only this README is tracked.

---

# Upstream source — ARM optimized-routines

- **Project:** ARM optimized-routines
- **Upstream repository:** <https://github.com/ARM-software/optimized-routines>
- **Commit at last sync:** `f2e4faf58c6c671154f472a76eaaa977bb36c870`
- **Commit date:** 2026-09-09 (`string: Add a macro for SVE load`)
- **Accessed:** 2026-09-14
- **Clone command:** `git clone --depth 1 https://github.com/ARM-software/optimized-routines.git upstream_source/ARM-optimized-routines`

## Why a second upstream at all

GSL does not provide `exp`/`log`/`pow` — it calls the platform libm for them,
and in `no_std` PETIR routes them through the `libm` crate (a port of musl's)
instead. That is correct and portable but **slower than the platform**: 1.58x
on `exp`, 1.41x on `log`, 3.51x on `powf` as measured for `bn:op-chyp.5`. That
cost is the whole reason `outram-mc-libs` gates its transcendentals behind an
opt-in feature rather than switching unconditionally.

The cost turns out to be avoidable, because **the fast implementation glibc
ships is itself open source and permissively licensed**: glibc's `exp`, `log`
and `pow` *are* ARM optimized-routines, contributed upstream by Arm. Built from
source and benchmarked side by side on this machine:

| | glibc | ARM optimized-routines | Rust `libm` |
|---|---|---|---|
| `exp` | 118 ms | 119 ms | 255 ms |
| `log` | 162 ms | 158 ms | 216 ms |
| `pow` | 507 ms | 507 ms | 1544 ms |

## Licence — VERIFIED, not asserted

Same standard as the GSL section above: read from the upstream tree, not from
documentation.

**`LICENSE` is a dual-licence file**, not a single licence text:

```
MIT OR Apache-2.0 WITH LLVM-exception
=====================================


MIT License
-----------

Copyright (c) 1999-2022, Arm Limited.
```

- `sha256(LICENSE)` = `650afbf29f214451e02241adc42534e82c9d6ae2b38e2444b92b5a1ffcaf9346`
- size = 13491 bytes
- it contains, in order: the MIT licence text, the full Apache License 2.0
  (from line 32), its "How to apply" appendix (line 209), and the **LLVM
  Exceptions** (line 235).

Unlike GSL, here the top-level file *does* state the grant rather than merely
containing boilerplate — but the per-file headers are checked anyway, because
a repository-level file does not bind a file that says something different.
Every file ported or linked here carries the identical SPDX line:

```c
/*
 * Double-precision e^x function.
 *
 * Copyright (c) 2018-2025, Arm Limited.
 * SPDX-License-Identifier: MIT OR Apache-2.0 WITH LLVM-exception
 */
```

**Conclusion: MIT OR Apache-2.0 WITH LLVM-exception.** Both options are
permissive and GPL-3.0-compatible, so the transcription in `src/fast_exp.rs` is
GPL-3.0-only like the rest of this workspace. The flow is **one-way**: code
here cannot go back upstream under the original licence.

### Copyright holders

Arm Limited throughout, but the year ranges differ per file — read the header
of any further file before porting it:

| file | copyright |
|---|---|
| `math/exp.c` | Arm Limited, 2018–2025 |
| `math/exp_data.c` | Arm Limited, 2018–2023 |
| `math/log.c` | Arm Limited, 2018–2025 |
| `math/log_data.c` | Arm Limited, 2018 |
| `math/pow.c` | Arm Limited, 2018–2026 |
| `math/pow_common.h` | Arm Limited, 2026 |
| `math/pow_log_data.c` | Arm Limited, 2018 |
| `math/math_config.h` | Arm Limited, 2017–2025 |
| `math/math_err.c` | Arm Limited, 2018 |

## Files vendored for porting

Digests recorded at sync so a reviewer can diff the exact bytes that were read:

| sha256 (first 16) | file | lines |
|---|---|---|
| `219b5f666c84864e` | `math/exp.c` | 176 |
| `a5e84677e7693c7d` | `math/exp_data.c` | 1141 |
| `10e3774a72279c75` | `math/log.c` | 170 |
| `327cf18198709304` | `math/log_data.c` | 511 |
| `4497df6d64e0043c` | `math/pow.c` | 373 |
| `d28818bd95355e68` | `math/pow_log_data.c` | 184 |
| `9a3ee029ba2b5844` | `math/math_config.h` | 770 |
| `367906e9face47b4` | `math/pow_common.h` | 45 |
| `202a5f0b4983da16` | `math/math_err.c` | 80 |

## The tables were extracted mechanically, not retyped

`exp_data.c` selects between `N == 64` and `N == 128` variants with
preprocessor conditionals; `log_data.c` compiles its second table (`tab2`)
only when `HAVE_FAST_FMA == 0`. Resolving conditionals like these by eye is
exactly how a transcription error gets in. So every table in
`src/fast_{exp,log,pow}.rs` — 256 `exp` entries, 256 + 256 `log` entries, 384
`pow` entries, and 27 polynomial coefficients between them — was produced by
**linking the compiled `*_data.o` into a dumper** that printed the resolved
`__exp_data` / `__log_data` / `__pow_log_data` fields as hex bit patterns, and
generating the Rust from that output. Same reasoning as the mechanical
extraction of `expint`'s 150 coefficients.

The dumper also printed the resolved configuration, which is how the ports
know which branches to take rather than guessing: `HAVE_FAST_FMA=0`,
`LOG_TABLE_BITS=7`, `LOG_POLY_ORDER=6`, `LOG_POLY1_ORDER=12`,
`POW_LOG_TABLE_BITS=7`, `POW_LOG_POLY_ORDER=8`, `EXP_TABLE_BITS=7`,
`EXP_POLY_ORDER=5`, `EXP_USE_TOINT_NARROW=0`, `TOINT_INTRINSICS=0`.

## V&V oracle

`reference-data/arm-optimized-routines/` (repository root, outside `crates/`)
holds ARM's own compiled output over fixed probe grids, plus the committed
drivers that regenerate them. Each port is checked against its oracle **bit for
bit**, and all three pass at **100.000 %**:

| port | test | probes compared | bit-identical |
|---|---|---|---|
| `fast_exp::exp` | `tests/fast_exp_vs_arm_optimized_routines.rs` | 8 290 | 8 290 |
| `fast_log::ln` | `tests/fast_log_vs_arm_optimized_routines.rs` | 16 748 | 16 748 |
| `fast_pow::powf` | `tests/fast_pow_vs_arm_optimized_routines.rs` | 42 176 | 42 176 |

The remaining probes in each file are refusals — overflow, underflow,
divide-by-zero or invalid-operand — and each is checked against upstream's
returned infinity, zero or NaN rather than skipped.

Speed, measured **from Rust** over 2 000 000 calls per route
(`crates/petir/tests/fast_math_speed.rs`, `--release`, this host), because a
figure measured in C says nothing about this crate:

| | platform (glibc) | `petir::real` (libm) | `petir::fast_*` |
|---|---|---|---|
| `exp` | 11.9 ms | 16.6 ms | 9.7 ms |
| `ln` | 9.7 ms | 13.7 ms | 12.2 ms |
| `powf` | 35.0 ms | 94.9 ms | 44.3 ms |

So against the `libm` route: 1.7x on `exp`, 2.1x on `powf`, and only ~1.05x on
`ln`. The `ln` figure is small and is reported as such — `HAVE_FAST_FMA == 0`
costs `log` a second table and a longer near-1 branch, and the `libm` crate's
`log` is already good.

That directory's README also records the FMA finding: glibc's ifunc dispatch
picks an FMA-compiled build of this source, which differs from the portable
build on 0.055 % of inputs by exactly 1 ulp. The oracle is deliberately built
**without** FMA, because Rust never contracts floating-point expressions and
the portable form is the one a Rust transcription can hold on every target.

## The clone is not committed

`/upstream_source/ARM-optimized-routines/` is gitignored (see the crate
`.gitignore`, written before cloning). Only this README is tracked.
