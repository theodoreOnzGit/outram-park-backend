# Golden NJOY2016 DTFR run for V&V (repo-tracked, NOT crate-packaged)

A CLAW-format DTF table set written by the upstream Fortran NJOY2016 `DTFR`
module, with the GENDF it consumed, used as the like-for-like oracle for
the crate's DTFR GENDF reader and table assembly
(`tests/dtfr_u238_claw_njoy_golden.rs`). Like the other `reference-data/`
subdirectories these live at the repository root, outside `crates/`, so
they are git-tracked but never part of a published crate tarball. Read
them through `njoy_outram_park_fork::reference_data::reference_file("dtfr", …)`.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built 2026-09-10;
generated 2026-09-10 from `../endf/n-092_U_238.endf` and NJOY's own 293.6 K
PENDF of it (the RECONR/BROADR steps of `../gendf/*.njoy-input`), with the
deck `u238-29g-claw.njoy-input`:

```text
groupr   20 22 0 23 / 9237 1 0 3 0 1 6 1 / ... 29 groups, iwt=3, 6 sigma0,
         3 1, 3 2, 3 18, 3 102, 6 2 (the elastic matrix)
moder    23 -24 /                       ASCII GENDF -> binary (dtfr reads binary)
dtfr     -24 25 0 0 / 0 0 1 / 1 29 / 0 0 / 'u238' 9237 1 293.6 / /
```

| File | Content |
|---|---|
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf` (44 KB) | the GENDF (`tape23`): MF=1/451, MF=3 MT=1/2/18/102, **MF=6 MT=2** (the committed `../gendf/` tapes carry MF=3 only) |
| `u238-29g-claw.dtf` (207 lines) | `tape25`: the 48-column `edit xsec (29x 48)` block and the `l=0 n-n table (32x 29)` — per DTF group: absorption, nu*sigma_f, total, then the 29-wide scatter band, six `1PE12.5` fields per line, last line zero-filled (`dtfr.f90:838-890`) |

## Measured agreement (2026-09-10)

The crate's `build_neutron_table` (MF=3/MT=1 total → `iptotl`, absorption
= total − scatter at `iptotl-2`, MF=6/MT=2 packed into the band) on the
same GENDF reproduces the 32 x 29 n-n table: 115 non-zero entries, worst
relative deviation **4.5e-6** (group 23 total: 10.99815 vs NJOY's
`1.09982E+01`, i.e. the 6-figure printing). nu*sigma_f is zero on both
sides (no nubar on the tape). The 48 edit columns (elastic/inelastic/…
edits, fission, chi) are outside the crate's minimal reader and are not
compared.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
