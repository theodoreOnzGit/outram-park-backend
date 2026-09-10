# Golden ERRORR covariance tapes for V&V (repo-tracked, NOT crate-packaged)

Multigroup **covariance** output tapes produced by the upstream Fortran
NJOY2016 `ERRORR` module, used as golden references by the
`njoy-outram-park-fork` ERRORR tests (`tests/errorr_mf33_golden.rs`). Like
`../endf/` and `../gendf/`, they live at the repository root, outside
`crates/`, so they are git-tracked but never part of a published crate
tarball. Read them through
`njoy_outram_park_fork::reference_data::reference_file("errorr", …)`
(override the root with `OUTRAM_PARK_REFERENCE_DATA_DIR`).

Every tape is committed together with the exact NJOY input deck
(`*.njoy-input`). Regenerate with `njoy < <deck>` after copying the named
ENDF tape to `tape20`; `tape23` is the covariance tape (`*.errorr`), `tape22`
the PENDF (`*.pendf`) that fed it.

## Provenance

All tapes: NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built
2026-09-10; generated 2026-09-10. Input ENDF tapes are ENDF/B-VIII.0 from
`../endf/`. Every deck is

```text
reconr  20 21 / 'pendf …' / <mat> 0 / 0.001 / 0 /
broadr  20 21 22 / <mat> 1 / 0.001 / 293.6 / 0 /
errorr  20 22 0 23 0 0 /        nendf npend ngout nout nin nstan
        <mat> 3 <iwt> 1 <irelco> /   matd ign iwt iprint irelco  (ign=3: LANL 30-group)
        1 293.6 /               mprint tempin
        0 33 1 1 -1 2e6 0 /     iread mfcov irespr legord ifissp efmean dap
```

| Tape (`.errorr`) | Material | Input ENDF | `iwt` | `irelco` | Companion PENDF |
|---|---|---|---|---|---|
| `h2-ENDF8.0-293.6K-ign3-iwt6-rel` (786 lines) | H-2, MAT 128 | `n-001_H_002-ENDF8.0.endf` | 6 | 1 | `h2-ENDF8.0-293.6K.pendf` (committed) |
| `h2-ENDF8.0-293.6K-ign3-iwt3-abs` (786 lines) | H-2, MAT 128 | same | 3 | 0 | same |
| `be9-ENDF8.0-293.6K-ign3-iwt6-rel` (1476 lines) | Be-9, MAT 425 | `n-004_Be_009-ENDF8.0.endf` | 6 | 1 | `be9-ENDF8.0-293.6K.pendf` (committed) |
| `li6-ENDF8.0-293.6K-ign3-iwt6-rel` (282 lines) | Li-6, MAT 325 | `n-003_Li_006-ENDF8.0.endf` | 6 | 1 | `li6-ENDF8.0-293.6K.pendf` (committed) |
| `c12-ENDF8.0-293.6K-ign3-iwt6-rel` (914 lines) | C-12, MAT 625 | `n-006_C_012-ENDF8.0.endf` | 6 | 1 | `c12-ENDF8.0-293.6K.pendf` (committed) |
| `f19-ENDF8.0-293.6K-ign3-iwt6-rel` (1519 lines) | F-19, MAT 925 | `n-009_F_019-ENDF8.0.endf` | 6 | 1 | `f19-ENDF8.0-293.6K.pendf` (committed) |
| `si30-ENDF8.0-293.6K-ign3-iwt6-rel` (1635 lines) | Si-30, MAT 1431 | `n-014_Si_030-ENDF8.0.endf` | 6 | 1 | `si30-ENDF8.0-293.6K.pendf` (committed) |
| `u234-ENDF8.0-293.6K-ign3-iwt6-rel` (501 lines) | U-234, MAT 9225 | `n-092_U_234-ENDF8.0.endf` | 6 | 1 | 6 MB, **not committed** — set `OUTRAM_PARK_NJOY_U234_PENDF` |
| `u238-ENDF8.0-293.6K-ign3-iwt6-rel` (5999 lines) | U-238, MAT 9237 | `n-092_U_238.endf` | 6 | 1 | 40 MB, **not committed** — set `OUTRAM_PARK_NJOY_U238_PENDF` (the same PENDF the GROUPR golden test uses) |

None of these evaluations carries **MF=32** (resonance-parameter
covariances), so every run takes ERRORR's pure MF=33 path
(`covcal` → `covout`, no `resprx`); that is the path the crate ports.
U-238 exercises lumped reactions (`MT=851/852` with 39 + 2 components) and
the NC-type `LTY=0` derivation of `MT=1` and `MT=4`; H-2 derives `MT=16`
over a second energy range (`3.339 MeV – 150 MeV`). U-234 also carries MF=31
(ν̄ covariances), which `mfcov=33` ignores.

## What the tapes contain

`tape23` is an ENDF-format "covariance GENDF": MF=1/MT=451 (a LIST of the
`ngn+1` user group bounds), MF=3 (one LIST of the `ngn` coarse-group cross
sections per reaction, lumped reactions included, their components not),
and MF=33 (per reaction a HEAD, then per companion reaction a CONT
`(0,0,MAT1,MT1,0,ngn)` followed by one LIST `(0,0,n,ig2lo,n,ig)` per group
row that has a non-zero element; the last row is always written). Sections
for lumped components are placeholders whose HEAD carries `MTL` in `L2`.

## Measured agreement (2026-09-10, `tests/errorr_mf33_golden.rs`)

Tier 1 (NJOY's own PENDF in, so only ERRORR logic differs): every group
cross section and every non-zero covariance element of every block agrees
with the tape to the tape's own printing precision — worst relative
deviation 4.9e-7 across the nine runs for values with a one-digit exponent
(7 printed figures) and 3.6e-6 for the one value with a two-digit exponent
(U-238 `MT=852`, `1.013290-10`, 6 printed figures). Block counts, reaction
lists, union-group counts (H-2 35, U-238 135, C-12 675) and lumped
placeholders match exactly.

Tier 2 (crate RECONR + BROADR in): covariances of directly-evaluated pairs
within 3.3e-6 (H-2) and 9.0e-4 (Be-9); the thermal-group cross sections
deviate by 32 % (H-2) and 1.2 % (Be-9) because the crate's BROADR does not
insert grid points across the free-gas 1/v rise of a light nuclide — a
BROADR finding tracked on its own bead, not an ERRORR one.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
