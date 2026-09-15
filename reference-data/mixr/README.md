# Golden NJOY2016 MIXR tape for V&V (repo-tracked, NOT crate-packaged)

A mixed-material PENDF written by the upstream Fortran NJOY2016 `MIXR`
module, used as the like-for-like oracle for the crate's mixing engine
(`tests/mixr_h2_be9_njoy_golden.rs`). Like the other `reference-data/`
subdirectories it lives at the repository root, outside `crates/`, so it is
git-tracked but never part of a published crate tarball. Read it through
`njoy_outram_park_fork::reference_data::reference_file("mixr", …)`.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built 2026-09-10;
generated 2026-09-10 from the two committed 293.6 K PENDFs
`../errorr/h2-ENDF8.0-293.6K.pendf` (H-2, MAT 128) and
`../errorr/be9-ENDF8.0-293.6K.pendf` (Be-9, MAT 425) with the deck
`h2-be9-0.5-0.5-293.6K.njoy-input`:

```text
mixr
24 20 21/                          nout nin1 nin2
1 2 102/                           output MTs
128 0.5 425 0.5/                   (mat, weight) pairs
293.6/                             temperature
9999 9999. 5.0/                    matd za awr
'w4 mixr oracle: 0.5 h2 + 0.5 be9 at 293.6 K'/
```

`tape24` is `h2-be9-0.5-0.5-293.6K.pendf` (814 lines): MAT 9999 with MF=1/451
and MF=3 MT=1, 2, 102 on the 787-point union grid of the two inputs.

## Measured agreement (2026-09-10)

The crate's `run_mix` on the same two files and cards gives 787 points per
reaction (as NJOY) and every value identical after the tape's rounding
(worst relative deviation 0 at all 787 x 3 points; section sums 9.736429e3,
9.666179e3, 1.644874e1 b identical); `ZA`/`AWR` of MF=1/451 identical.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
