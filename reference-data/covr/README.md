# Golden COVR BOXER libraries for V&V (repo-tracked, NOT crate-packaged)

Condensed **BOXER-format covariance libraries** produced by the upstream
Fortran NJOY2016 `COVR` module in its library option, used as golden
references by the `njoy-outram-park-fork` COVR tests
(`tests/covr_boxer_golden.rs`). Like `../endf/`, `../gendf/` and
`../errorr/`, they live at the repository root, outside `crates/`, so they
are git-tracked but never part of a published crate tarball. Read them
through `njoy_outram_park_fork::reference_data::reference_file("covr", …)`
(override the root with `OUTRAM_PARK_REFERENCE_DATA_DIR`).

Every library is committed together with the exact NJOY input deck
(`*.njoy-input`). Regenerate with `njoy < <deck>` after copying the named
ERRORR tape from `../errorr/` to `tape20`; `tape21` is the library
(`*.boxer`).

## Provenance

All libraries: NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0,
built 2026-09-10; generated 2026-09-10. Inputs are the nine ERRORR
covariance tapes of `../errorr/` (ENDF/B-VIII.0 evaluations, RECONR 0.001
→ BROADR 293.6 K → ERRORR `ign=3` LANL 30-group, `mfcov=33`). Every deck is

```text
covr
20 21 0/            nin nout nplot          (nout>0: library option)
<matype> 1/         3=covariances 4=correlations, ncase
'w4cov'/            hinpid (6 characters)
'oracle covr w4'/   hdescr (21 characters)
<mat> 0 0 0/        mat mt mat1 mt1         (mt=0: every MT-MT1 pair)
stop
```

| Library (`-matype3.boxer` / `-matype4.boxer`) | Material | Input ERRORR tape | pairs | written | lines (3 / 4) |
|---|---|---|---|---|---|
| `h2-ENDF8.0-293.6K-ign3-iwt6-rel` | H-2, MAT 128 | same name, `.errorr` (relative) | 10 | 7 | 101 / 117 |
| `h2-ENDF8.0-293.6K-ign3-iwt3-abs` | H-2, MAT 128 | same name (absolute) | 10 | 7 | 349 / 127 |
| `be9-ENDF8.0-293.6K-ign3-iwt6-rel` | Be-9, MAT 425 | same name | 91 | 29 | 395 / 336 |
| `li6-ENDF8.0-293.6K-ign3-iwt6-rel` | Li-6, MAT 325 | same name | 3 | 2 | 113 / 99 |
| `c12-ENDF8.0-293.6K-ign3-iwt6-rel` | C-12, MAT 625 | same name | 55 | 14 | 189 / 174 |
| `f19-ENDF8.0-293.6K-ign3-iwt6-rel` | F-19, MAT 925 | same name | 66 | 27 | 516 / 467 |
| `si30-ENDF8.0-293.6K-ign3-iwt6-rel` | Si-30, MAT 1431 | same name | 153 | 31 | 477 / 423 |
| `u234-ENDF8.0-293.6K-ign3-iwt6-rel` | U-234, MAT 9225 | same name | 15 | 5 | 149 / 133 |
| `u238-ENDF8.0-293.6K-ign3-iwt6-rel` | U-238, MAT 9237 | same name | 66 | 66 | 3818 / 3385 |

"pairs" is the number of `(mt, mt1 >= mt)` combinations `expndo` builds
from the MTs present; "written" the number COVR did not suppress as a null
covariance matrix (`covr.f90:433-434`). In the library option COVR forces
`irelco = 1` (`covr.f90:240-243`), so the absolute H-2 tape yields an
absolute library whose `itype = 2` vector is an absolute standard
deviation.

## What the libraries contain

BOXER records (`covr.f90:1991-2247`): per matrix a header
`(i1,1x,a12,1x,a21,2(i5,i4),2(i4,i3),3i4)` — `itype` (0 group bounds, 1
cross section, 2 standard deviation, 3 covariance, 4 correlation),
`hlibid` (`hinpid` + `-a-`/`-b-` + group count), `hdescr`, `mat mt mat1
mt1`, then `nval nvf ncon ncf nrowm nrow ncol` — followed by the `nval`
distinct values (`1p8e10.3`, or `11f7.4` for correlations) and the `ncon`
run-length codes (`26i3` for vectors, `20i4` for the 30-group matrices; a
negative code repeats the next value, a positive one copies the row
above). `ncol = 0` marks a symmetric matrix stored as its upper triangle;
`nrowm > 0` continues on a further page whose header carries 34 dashes.
The group bounds are written once, before the first written pair; an
auto pair also gets its cross-section and standard-deviation vectors.

## Measured agreement (2026-09-10, `tests/covr_boxer_golden.rs`)

Tier 1 (NJOY's ERRORR tape in, so only COVR logic differs): the crate's
library is **byte-identical** to every one of the 18 files.

Tier 2 (the crate's ERRORR on NJOY's PENDF, written and re-read as a tape,
then the crate's COVR): **byte-identical** for all 18 as well (U-234 and
U-238 run when `OUTRAM_PARK_NJOY_U234_PENDF` / `OUTRAM_PARK_NJOY_U238_PENDF`
point at NJOY's `tape22`). U-238 first agreed on only 805 of 3818 lines
because the crate's ERRORR writer zeroed output elements below `1e-20`
where `errorr.f90:7563-7568` writes them verbatim (NJOY's tape holds
`e-49`…`e-69` elements that `press` encodes as distinct runs); fixing the
writer made the tapes identical.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
