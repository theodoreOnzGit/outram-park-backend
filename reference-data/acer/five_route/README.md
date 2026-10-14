# NJOY2016 ACE tables for the five-route parity gate (GitHub #872)

Six continuous-energy ACE tables made by **NJOY2016 2016.79** (commit
`ac5adf5`) from the ENDF/B-VIII.0 evaluations in `reference-data/endf/`, at
293.6 K, with the five-route campaign's deck:

```
reconr  .001
broadr  293.6 K, .001
purr    20 bins, 64 ladders
acer    iopt 1, newfor 1, iopp 1 (photons on), no HEATR
```

(`crates/outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/make_ace.sh`,
run by `.../icsbep/five_route_keff/scripts/make_njoy_library.sh`.) Each table
carries its provenance in its own 70-character ACE comment: name, library,
NJOY version and commit, date, and this workspace's revision.

| file | evaluation (MAT) | form |
|---|---|---|
| `H1_293.6K_njoy2016.ace.gz` | `n-001_H_001-ENDF8.0-Beta6.endf` (125) | Type 1 |
| `F19_293.6K_njoy2016.ace.gz` | `n-009_F_019-ENDF8.0.endf` (925) | Type 1 |
| `Al27_293.6K_njoy2016.ace.gz` | `n-013_Al_027-ENDF8.0.endf` (1325) | Type 1 |
| `B10_293.6K_njoy2016.ace.gz` | `n-005_B_010-ENDF8.0.endf` (525) | **Type 2**: NJOY2016's Type-1 formatter stops in `change` on B-10's law-0 DLWH entry; the same deck with `itype = 2` completes |
| `O16_293.6K_njoy2016.ace.gz` | `n-008_O_016-ENDF8.0.endf` (825) | Type 1 |
| `Mg24_293.6K_njoy2016.ace.gz` | `n-012_Mg_024-ENDF8.0.endf` (1225) | Type 1; added 2026-10-14 (MT=16/22/28/91 as MF=4 + MF=5, no MF=6) |

~~These tables are used by the gate below.~~ **CORRECTED 2026-10-14:** until
then none of them was committed. `.gitignore`'s `*.ace.gz` excluded this
folder (its exception covered only `reference-data/acer/*.ace.gz`), so the
gate skipped on every machine but the one that made them, while reporting
`ok`. The exception now covers this folder. The six tables committed on
2026-10-14 were regenerated in a cloud container by NJOY2016 2016.79 (build
stamp `18Mar25`; its source commit was not recorded) with the same deck,
through `make_njoy_library.sh`; their ACE comment reads `NJOY2016 unknown
unknown`, because the script's version variables were not set.

- **Source data:** ENDF/B-VIII.0 (public, NNDC/BNL), see `reference-data/endf/README.md`.
- **Generated:** 2026-10-14 in a cloud container; gzip -9.
- **Used by:** `crates/njoy-outram-park-fork/tests/ace_five_route_parity.rs`.
- **Licence:** derived from public nuclear data by NJOY2016 (BSD-3-style,
  LANL); no restrictions beyond those of the evaluations.
