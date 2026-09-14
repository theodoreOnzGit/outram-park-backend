# Upstream source

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
