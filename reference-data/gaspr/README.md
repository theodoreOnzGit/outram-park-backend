# Golden NJOY2016 GASPR tapes for V&V (repo-tracked, NOT crate-packaged)

Gas-production cross sections (ENDF MF=3, MT=203–207) written by the upstream
Fortran NJOY2016 `GASPR` module, used as a like-for-like oracle for
`crates/njoy-outram-park-fork/src/gaspr/`
(`tests/gaspr_vs_njoy2016.rs`). Like `../endf/`, `../reconr/`, `../errorr/`,
`../covr/` and `../leapr/`, they live at the repository root, outside
`crates/`, so they are git-tracked but never part of a published crate
tarball. Read them through
`njoy_outram_park_fork::reference_data::reference_file("gaspr", …)`.

Each tape is the deck's `tape24` **trimmed to MF=1 plus MF=3/MT=203–207** —
the reconstructed partials are already covered by `../reconr/` and
`../errorr/`, and keeping only the gas sections holds this directory to a few
hundred kilobytes. The `.njoy-input` beside each tape is the verbatim deck
that produced it, so the reference regenerates rather than being trusted.

## Provenance

NJOY2016 upstream `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
(2026-04-06), built from source and executed 2026-09-17. Inputs are the
committed ENDF/B-VIII.0 evaluations in `../endf/`:

| tape | evaluation | MAT |
|---|---|---|
| `li6-…` | `n-003_Li_006-ENDF8.0.endf` | 325 |
| `be9-…` | `n-004_Be_009-ENDF8.0.endf` | 425 |
| `b10-…` | `n-005_B_010-ENDF8.0.endf` | 525 |
| `h2-…`  | `n-001_H_002-ENDF8.0.endf`  | 128 |
| `c12-…` | `n-006_C_012-ENDF8.0.endf`  | 625 |

All five at `RECONR err = 0.001`, 0 K (no BROADR).

## Why these five

They are where gas production is not a lookup on the MT number: Li-6's
`(n,t)` leaves an alpha, Be-9's `(n,2n)` leaves particle-unbound ⁸Be, B-10
carries the discrete `(n,t)` level MT=700 but no lumped MT=105, H-2's tritium
comes entirely from radiative capture, and C-12 states MT=5 gas yields as
energy-dependent MF=6 multiplicities. The write-up, including the three
defects this comparison found, is
`../../crates/njoy-outram-park-fork/verification_and_validation/gaspr_light_nuclides_vs_njoy2016.md`.
