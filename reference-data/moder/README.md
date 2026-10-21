# Golden NJOY2016 MODER tape for V&V (repo-tracked, NOT crate-packaged)

A material-selection tape written by the upstream Fortran NJOY2016 `MODER`
module, the like-for-like oracle for the crate's `moder::select_materials` +
`Tape::write` (`crates/njoy-outram-park-fork/tests/moder_vs_njoy2016.rs`,
GitHub #536). Like the other `reference-data/` subdirectories it lives at the
repository root, outside `crates/`, so it is git-tracked but never part of a
published crate tarball. Read it through
`njoy_outram_park_fork::reference_data::reference_file("moder", …)`.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, `cmake
-DCMAKE_BUILD_TYPE=Release`, built and run 2026-10-05, with the deck
`h2-li6-select.njoy-input`:

```text
moder
 1 30/                                   nin=1: select from ENDF/PENDF; nout=30 coded
 'moder selection: H-2 (128) + Li-6 (325), ENDF/B-VIII.0'/
 20 128/                                 unit 20, MAT 128
 21 325/                                 unit 21, MAT 325
 0/
stop
```

| unit | file |
|---|---|
| `tape20` | `../endf/n-001_H_002-ENDF8.0.endf` (H-2, MAT 128, ENDF/B-VIII.0) |
| `tape21` | `../endf/n-003_Li_006-ENDF8.0.endf` (Li-6, MAT 325, ENDF/B-VIII.0) |
| `tape30` | `h2-li6-select.moder.tape` (this directory) |

`h2-li6-select.moder.tape`: 955 233 bytes, 11 793 lines, SHA-256
`0a15e866c842d565b7a4b49b01ab9103f756955a3902b3690e9cbd53ec91b1b6`.

The evaluations are ENDF/B-VIII.0 (Brown et al., Nucl. Data Sheets 148, 1
(2018)), openly distributed; their provenance is in `../endf/README.md`. The
tape is NJOY's re-encoding of those two evaluations and carries no other data.

Record: `crates/njoy-outram-park-fork/verification_and_validation/moder_vs_njoy2016.md`.
