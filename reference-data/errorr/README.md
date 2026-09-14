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
| `ar37-tendl2023-L1last-293.6K-ign3-iwt6-rel` (5355 lines) | Ar-37, MAT 1828 | `n-018_Ar_37-tendl2023-mf2-L1-last.endf` (TENDL-2023, see `../endf/README.md`) | 6 | 1 | `ar37-tendl2023-L1last-293.6K.pendf` (committed) |

None of the ENDF/B-VIII.0 evaluations carries **MF=32** (resonance-
parameter covariances), so those runs take ERRORR's pure MF=33 path
(`covcal` → `covout`, no `resprx`). **Ar-37 (TENDL-2023) does carry MF=32**
— a resolved MLBW range (`LRU=1/LRF=2`, `LCOMP=2`, 7 resonances × 3
parameters, `ISR=1`, `DAP=2.032674e-2`) and an unresolved range (`LRU=2`,
22 parameters with a relative covariance LIST) — and is the oracle for the
`resprx` chain (`tests/errorr_mf32_ar37_golden.rs`). NJOY2016 `ac5adf5`
aborts on the unmodified TENDL-2023 tape (`***error in rpxlc12***problem`
at the `L=2` resonance `-2100.016 eV`: `errorr.f90:4363-4380` never advances
its MF=2 pointer past the empty `L=1` block), so the deck ran on the
physics-identical variant with the empty block moved last in MF=2; NJOY's
RECONR+BROADR PENDF from the two tapes is byte-identical (checked
2026-09-10). Its listing prints the `c**`/`u**` diagonals ("resolved" /
"unresolve") for the total, elastic and capture blocks; those 4-figure
tables are pinned in the test.
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

MF=32 (2026-09-10, `tests/errorr_mf32_ar37_golden.rs`, Ar-37): tier 1 —
all 465 blocks / 14 048 non-zero elements within 4.97e-7 of `tape23`, the
four resonance pairs included; the unmodified tape through the crate
matches the variant-tape oracle to the same 4.97e-7; the listing's
resolved/unresolved diagonals match to 4.3e-4 (4 printed figures). Tier 2
exposed a RECONR defect (Ar-37 elastic +6.3 % at low energy, −8.2 % at
1 keV: `LRF=2` evaluated with the SLBW elastic formula), recorded on its
own bead.

## Cl-35 (ENDF/B-VII.1, `LRF=7` R-matrix limited, MF=32 `LCOMP=2`) — added 2026-09-11 (`op-cjw.1`/`op-cjw.4`)

| Tape | Material | Deck | Companion PENDF |
|---|---|---|---|
| `cl35-ENDF7.1-293.6K-ign4-iwt2-rel.errorr` (770 lines) | Cl-35, MAT 1725 | `cl35-ENDF7.1-293.6K-ign4-iwt2-rel.njoy-input`: `reconr 20 21 / 1725 0 0 / 0.001 /`, `broadr 20 21 22 / 1725 1 / 0.001 / 293.6 /`, `errorr 20 22 0 23 0 0 / 1725 4 2 1 1 / 1 293.6 / 0 33 1 1 -1 2e6 0 /` (`ign=4` 27 groups, `iwt=2`, `irelco=1`) | `cl35-ENDF7.1-293.6K.pendf` (5.0 MB, committed) |

`tape20` for this deck is **not** the pristine tape but NJOY's own `999`
output for it (the `tests/20` deck's `tape21`): the evaluation carries no
MF=33, and ERRORR needs the four dummy `MT=1/2/102/600` sections to have
blocks for the resonance covariance to land in. The crate reproduces that
step with `errorr::covadd` on the pristine `../endf/n-017_Cl_035-ENDF7.1.endf`.
NJOY's shipped `tests/20` deck takes the group cross sections from a
GROUPR GENDF (`ngout`); this deck takes them from the PENDF (`npend`),
the path the crate implements — the MF=32 arithmetic is the same.

Measured (2026-09-11, `tests/errorr_mf32_cl35_rml_golden.rs`): tier 1
(NJOY PENDF in) — all 10 blocks, 3174 non-zero elements within **4.83e-7**
of `tape23`: `(2,2)` 4.16e-7, `(2,102)` 4.41e-7, `(2,600)` 4.12e-7,
`(102,102)` 4.38e-7, `(102,600)` 4.20e-7, `(600,600)` 4.83e-7; the four
`MT=1` blocks are identically zero on both sides (a directly-evaluated
dummy total gets no sensitivity through `akxy`). `σ_g` worst 3.3e-7. This
is the first oracle for the SAMM resonance-parameter derivatives
(`samm.f90` `babb`/`abpart`/`setqri`/`settri`/`derres`) and for
`rpxsamm`'s panel integration and INTG covariance. Tier 2 (crate RECONR +
BROADR in): the diagonal relative blocks are unchanged (the `cflx·csig`
normalisation cancels exactly), the cross blocks move by ≤ 3.7e-3 and
`σ_g` by ≤ 3.6e-3 (`MT=600`, group 11) — the crate's PENDF, not ERRORR.

## U-238 (JENDL-3.3, `LRF=3` Reich-Moore, MF=32 `LCOMP=1`) — added 2026-09-11 (`op-gh96`)

| Tape | Material | Deck | Companion PENDF |
|---|---|---|---|
| `u238-JENDL3.3-300K-ign3-iwt6-rel.errorr` (5958 lines) | U-238, MAT 9237 | `u238-JENDL3.3-300K-ign3-iwt6-rel.njoy-input`: `reconr 20 21 / 9237 0 / 0.001 /`, `broadr 20 21 22 / 9237 1 / 0.001 / 300. /`, `errorr 20 22 0 23 0 0 / 9237 3 6 1 1 / 1 300. / 0 33 1 1 -1 2e6 0 /` (NJOY2016's own `tests/16` deck) | 25 MB, **not committed** — set `OUTRAM_PARK_NJOY_J33U238_PENDF` to `tape22` |

Input: `../endf/n-092_U_238-JENDL3.3.endf` (the NJOY2016 test-suite
`J33U238` resource, see `../endf/README.md`). MF=2: ten `LRU=1/LRF=3`
ranges of 1 keV from 1e-5 eV to 10 keV, then `LRU=2/LRF=2` (`LSSF=0`) to
150 keV; MF=32: one `LCOMP=1` short-range block per resolved range
(`NSRS=1`, `MPAR=3` — `ER`, `Γn`, `Γγ` — 26–37 resonances, full
covariance LIST) plus an `LRU=2` block; MF=33 for 37 reactions. NJOY's
listing warns `mf2 nls=2, but mf32 nls=0` for every range and continues.

Measured (2026-09-11, `tests/errorr_mf32_j33u238_lrf3_golden.rs`): tier 1
(NJOY PENDF in) — all 666 blocks / 14 069 non-zero elements within
**4.95e-7**; listing diagonals within 4.67e-4. Tier 2 (crate RECONR +
BROADR in), groups 1–12: `σ_g` 4.09e-3, resonance-pair covariances
8.17e-3, listing 8.28e-3; groups 13–18 reported only (the crate's RECONR
has no `LSSF=0` URR average, bead `op-t0wt`) and sub-threshold fission
(`σ_g < 1e-6 b`) excluded — see the test's module docs.

Data policy: derived products of open ENDF/B-VIII.0, ENDF/B-VII.1,
JENDL-3.3 and TENDL-2023 data processed with the BSD-licensed NJOY2016; no proprietary
content.
