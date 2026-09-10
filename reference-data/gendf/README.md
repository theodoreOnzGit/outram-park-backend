# Golden GENDF tapes for V&V (repo-tracked, NOT crate-packaged)

Multigroup **GENDF** output tapes produced by the upstream Fortran NJOY2016 and
used as golden references by the `njoy-outram-park-fork` GROUPR tests. Like the
raw ENDF tapes in `../endf/`, they live at the repository root, outside
`crates/`, so they are git-tracked but never part of a published crate tarball.
Read them through `njoy_outram_park_fork::reference_data::reference_file("gendf", …)`
(override the root with `OUTRAM_PARK_REFERENCE_DATA_DIR`).

A golden tape is only as reproducible as the deck that made it, so **every tape
here is committed together with the exact NJOY input deck** (`*.njoy-input`)
and the oracle build it came from. Regenerate with
`njoy < <deck>` after copying the named ENDF tape to `tape20`.

## Provenance

| File | Material | Input tape | Oracle | Deck | Generated |
|---|---|---|---|---|---|
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | `../endf/n-092_U_238.endf` (ENDF/B-VIII.0) | NJOY2016 upstream `ac5adf5` (2016.79), gfortran, built 2026-09-10 | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | `../endf/n-092_U_238.endf` (ENDF/B-VIII.0) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.njoy-input` | 2026-09-10 |

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.gendf`

`RECONR` (`err = 0.001`) → `BROADR` (293.6 K, `errthn = 0.001`) → `GROUPR`
with `ign = 1` (29 user groups, breaks 1e-5 … 2e7 eV as listed in the deck),
`igg = 0`, `iwt = 3` (1/E), `lord = 0`, `ntemp = 1` (293.6 K), `nsigz = 6`
(`1e10 1e4 1e3 1e2 10 1` barn), `iprint = 1`; reactions `MF=3 MT=1, 2, 18, 102`.
No `UNRESR`/`PURR` step, so the tape carries no URR self-shielding
(`stounr`: "no unresolved sigma zero data") — self-shielding on this tape is
the Bondarenko flux alone. Because `lord = 0` with `nsigz > 1`, GROUPR promotes
to two Legendre orders, so each group record holds `NL*NZ*NG2 = 2*6*2 = 24`
words (`il` fastest, then `iz`, then `it`).

Consumed by `crates/njoy-outram-park-fork/tests/groupr_u238_gendf_golden.rs`,
whose doc comment records the measured agreement.

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.gendf`

Identical to the above except that `UNRESR` (`9237 1 6 1 / 293.6 / 1e10 1e4
1e3 1e2 10 1 /`) runs between `BROADR` and `GROUPR`, so the PENDF GROUPR read
carries an MF=2/MT=152 self-shielded URR table and the tape's unresolved-range
groups (20–149 keV) are Bondarenko-shielded through `stounr`/`getunr`.
Consumed by the same test (tier 3).

Data policy: GENDF is a *derived* product of open ENDF/B-VIII.0 data processed
with the BSD-licensed NJOY2016; it carries no proprietary content.
