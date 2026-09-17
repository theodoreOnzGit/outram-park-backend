# Golden NJOY2016 RESXSR file for V&V (repo-tracked, NOT crate-packaged)

A RESXS file written by the upstream Fortran NJOY2016 `RESXSR` module, the
like-for-like oracle for the crate's RESXSR driver and record writer
(`tests/resxsr_h2_njoy_golden.rs`). Like the other `reference-data/`
subdirectories it lives at the repository root, outside `crates/`. Read it
through `njoy_outram_park_fork::reference_data::reference_file("resxsr", …)`.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built 2026-09-10;
generated 2026-09-10 from `../errorr/h2-ENDF8.0-293.6K.pendf` (H-2, MAT 128,
293.6 K) with the deck `h2-293.6K-eps0.001.njoy-input`:

```text
resxsr
 25/                                     nout (see note)
 1 1 1 1e-5 1e3 0.001/                   nmat maxt nholl efirst elast eps
 'w4 test' 1/                            huse ivers
 'resxsr oracle: h2 293.6 K pendf, eps 0.001'/
 'h2' 128 20/                            hmat mat unit
```

NJOY's listing: `mt=2 ne=314`, `mt=102 ne=314`, `after thinning, ne=165`.
`tape25` is `h2-293.6K-eps0.001.resxs` (2,124 bytes): six gfortran
sequential-unformatted records (`[u32 nbytes][payload][u32 nbytes]`, 4-byte
words, `real(4)` values) — file identification, file control, blank
set-Hollerith, file data, the material control record and one 1,980-byte
cross-section block (165 points x (energy + 2 reactions)).

Note: the documented binary unit (`nout = -25`) aborts under this gfortran
("Unit number is negative and unit was not already opened with
OPEN(NEWUNIT=...)", `resxsr.f90:445`); the module writes the same
unformatted records to a positive unit, which is what is committed.

## Two temperatures (`h2-293.6K-600K-*`)

`h2-293.6K-600K.pendf` is NJOY's own RECONR + BROADR of the same H-2 ENDF at
293.6 K **and** 600 K (deck in `h2-293.6K-600K-eps0.001.njoy-input`), and
`h2-293.6K-600K-eps0.001.resxs` (3,508 bytes) the `resxsr` output with
`maxt = 2`: NJOY's listing reports 314 + 314 points at 293.6 K, 329 + 329 at
600 K, 168 after thinning; the material control record carries both
temperatures and each point holds `1 + 2 x 2` words (`resxsr.f90:267-352`,
columns temperature-major).

## Measured agreement (2026-09-10)

Two temperatures: `run_resxs` with `maxt = 2` is **byte-identical** to the
3,508-byte oracle (168 points, `nreac = 2`, temperatures 293.6 and 600 K)
on the first run of the ported temperature loop.

One temperature:

The crate's `run_resxs` output is **byte-identical** (no differing byte in
2,124) once its record framing matched gfortran's and `locm` matched
upstream's `irec` bookkeeping; decoded, the 165 energies and 330 values are
identical.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
