# chem-eng-real-time-process-control-simulator

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

A real-time process control simulator library for chemical engineering 
and other engineering.

## Per-step cost (changed in version 0.2.0)

Up to version 0.1.1 every transfer-function block kept a `Vec` of past step
responses — one entry per input change — and re-summed the whole vector on
every call. Entries were retired once 20 time constants had elapsed, so the
vector saturated at `20 * tau / dt` entries, but only if the run lasted that
long. A block with `tau = 80.8 s` stepped at 1 ms never gets there.

Version 0.2.0 replaces that with the **exact zero-order-hold (step-invariant)
discrete equivalent** of each block: constant time, constant memory, and
*exact* — not an approximation — for input held constant between calls. The
first-order case is the familiar one-pole update

```text
y[n] = exp(-T/tau) y[n-1] + (1 - exp(-T/tau)) K_p u[n]
```

Second-order, decaying-sinusoid and decaying-exponential blocks use the same
idea with two or three state variables. Irregular timesteps and dead times are
still supported.

Measured on an AMD Ryzen 5 5600 (rustc 1.97.0, release build, 200-step windows,
two 5,000-step warm-ups discarded), driving a filtered PID
(`K_c = 1.75`, `tau_I = 1.75 s`, `alpha = 1.0`) with an always-changing input:

| case | 0.1.1 per step | 0.2.0 per step | 0.1.1 total | 0.2.0 total |
|---|---|---|---|---|
| PID, `dt = 0.04 s`, 40k steps | 28.35 us (plateau) | 0.17 us | 1.188 s | 0.007 s |
| PID, `dt = 0.001 s`, 80k steps | 1176 us (plateau) | 0.17 us | 83.280 s | 0.015 s |
| P only, `dt = 0.04 s`, 40k steps | 9.42 us | 0.06 us | 0.379 s | 0.002 s |
| PID `tau_d = 1000 s`, `dt = 0.04 s`, 40k | 736.86 us, still rising | 0.17 us | 15.167 s | 0.007 s |
| PID `tau_d = 80.8 s`, `dt = 0.001 s`, 80k | 2329.52 us, still rising | 0.17 us | 127.599 s | 0.014 s |

The sum of every output over each of those runs is unchanged to the three
decimal places printed by the benchmark, so this is a cost change and not a
physics change. The remaining difference is that 0.1.1 discarded
`exp(-20) = 2.06e-9` of each step response when it retired one, and 0.2.0
carries that residual instead — so the new answer is very slightly the more
accurate of the two.

Verification of the recurrences against the closed-form analytic
superposition, and the O(1) regression tests, live in
`src/lib/*/stable_transfer_functions/recurrence_tests.rs`, with methodology
and measured results in each test's doc comment.

## z-domain (discrete-time) controllers — Octave control-package port

`beta_testing::z_domain` (new in 0.2.0) carries a SISO port of the
continuous/discrete conversion core of the
[GNU Octave control package](https://github.com/gnu-octave/pkg-control):

- `ContinuousTransferFn` — SISO `tf` surface (polynomial coefficients in `s`).
- `DiscreteTransferFn` — SISO `filt` surface (coefficients in `z^-1` plus a
  `uom` sample time), stepped by an O(1) fixed-state
  direct-form-II-transposed recurrence — the same no-growing-history
  discipline as the transfer-function blocks above.
- `c2d` (`ContinuousTransferFn::to_discrete`) with zero-order hold, Tustin,
  Tustin-with-prewarping and matched pole/zero methods; `d2c`
  (`DiscreteTransferFn::to_continuous`) with Tustin, prewarped Tustin and
  matched pole/zero. ZOH and matched are implemented in closed form for
  system order <= 2 (all of this crate's blocks); the Tustin variants work
  for any order. `d2c` by ZOH (matrix logarithm) and `c2d` by first-order
  hold / impulse invariance are deliberately not ported.

The ported files come from the **GPLv3-or-later side** of the mixed-licence
upstream; the BSD-3 SLICOT kernels were **not** ported — the ZOH matrix
exponential and bilinear substitution are independent closed-form
implementations. Zero-order hold `c2d` of a first-order lag reproduces the
0.2.0 recurrence block sample-for-sample (verified to 1.2e-14 over 200
samples); methodology and measured results live in
`src/lib/beta_testing/z_domain/verification_tests.rs` doc comments.

## to run 

```bash
cargo run 
```
To watch:
```bash
cargo watch -x run --ignore "*.csv"
```

## Documentation

TBD. Theory of basic items is in my PhD thesis (to be published later).

Citations appreciated.

## Licenses 

This crate is released under the **GNU General Public License v3.0 only
(GPL-3.0-only)** — see the `LICENSE` file.

It was **relicensed from Apache-2.0 to GPL-3.0-only on 2026-08-11** at the
direction of the maintainer, who is the sole copyright holder (verified
from the crate's full git history — see the `NOTICE` file). **Versions
published to crates.io before the relicense (0.0.1 through 0.1.1) were
published under Apache-2.0 and remain available under Apache-2.0
forever**; the relicense affects future versions only, starting with
0.2.0.

Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
University of California, Berkeley
Thermal Hydraulics Lab, Repository Contributors and 
Singapore Nuclear Research and Safety Institute (SNRSI) and
National University of Singapore (NUS)]

This program is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the
Free Software Foundation, version 3 of the License.

This program is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General
Public License for more details.

Parts of the z-domain (discrete-time) functionality are ported from the
GNU Octave [control package](https://github.com/gnu-octave/pkg-control)
(GPLv3-or-later, with SLICOT files under BSD 3-Clause) — see `NOTICE` for
the exact commit and the per-file licence provenance rules.

I use crates such as approx, csv, thiserror and uom. The licenses 
are located in the licenses_of_dependencies folder.


## References 

I used some of these references to develop this library:

Seborg, Dale E., Thomas F. Edgar, Duncan A. Mellichamp, and 
Francis J. Doyle III. Process dynamics and control. John Wiley & Sons, 2016.

Green, D. W., & Perry, R. H. (2008). Perry's chemical engineers' 
handbook. McGraw-Hill Education.
