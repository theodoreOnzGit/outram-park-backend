# NJOY2016 ACE tables for the five-route parity gate (GitHub #872)

Continuous-energy ACE tables made by **NJOY2016 2016.79** (commit
`ac5adf5`) from the ENDF/B-VIII.0 evaluations in `reference-data/endf/`, at
293.6 K (and 0 K, below), with the five-route campaign's deck:

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
| `Cu65_0K_njoy2016.ace.gz` | `n-029_Cu_065-ENDF8.0.endf` (2931) | Type 1, **0 K**; added 2026-10-15 (GitHub #875) |
| `Na23_0K_njoy2016.ace.gz` | `n-011_Na_023-ENDF8.0.endf` (1125) | Type 1, **0 K**; added 2026-10-15 (#875) |
| `Ti48_0K_njoy2016.ace.gz` | `n-022_Ti_048-ENDF8.0.endf` (2231) | Type 1, **0 K**; added 2026-10-15 (#875) |
| `Zn64_0K_njoy2016.ace.gz` | `n-030_Zn_064-ENDF8.0.endf` (3025) | Type 1, **0 K**; added 2026-10-15 (#875) |
| `Zn70_293.6K_njoy2016.ace.gz` | `n-030_Zn_070-ENDF8.0.endf` (3043) | ~~Type 1; added 2026-10-15 (#875)~~ **Type 2 since 2026-10-15** (the same deck with `itype = 2`; comment `Zn70 E8.0 NJOY2016 2016.79 ac5adf5 2026-10-15 opb 093d859`; SHA-256 `5b4c1400…ffc601b807`). Its LUNR block holds the MT=153 values NJOY2016 read back as text, which a Type-1 file prints too coarsely to check bit for bit |
| `Ti48_0K_heatr_njoy2016.ace.gz` | `n-022_Ti_048-ENDF8.0.endf` (2231) | **Type 2, 0 K, with HEATR**: `reconr -> heatr (20 21 26 / 2231 0 0 0 0 0 /) -> acer`; added 2026-10-15 |
| `Zn70_293.6K_heatr_njoy2016.ace.gz` | `n-030_Zn_070-ENDF8.0.endf` (3043) | **Type 2, 293.6 K, with HEATR**: `reconr -> broadr -> heatr -> purr -> acer`; added 2026-10-15 |

The two `_heatr_` tables are the deck the LCT-008 Rust library actually runs
(`write_ace_library`, HEATR on by default), with HEATR between BROADR and
PURR and every unit positive (text), as an NJOY2016 deck would carry it. The
five-route NJOY2016 library itself has no HEATR, so its heating column is
zero; these references hold the heating number, PURR's heating column and
HEATR's text hand-offs to NJOY2016's bits.

**Type 2 is the stronger reference.** It holds the doubles NJOY2016 had in
memory; a Type-1 file rounds them to 12 figures, which hides `sigfig`'s
`1.0000000000001` bias and MTRP's fractional words (B-10's `801001.2`). The
gate holds a Type-2 reference to every bit (2026-10-15). NJOY2016 cannot
write an unrounded **thermal** table: `aceth.f90:541` fixes `itype = 1`.

The **0 K** tables use the 0 K deck,
`.../openmc_godiva_cross_code/make_ace_0k.sh`: `reconr .001 -> acer`, no
BROADR, no PURR. The #875 tables were made on 2026-10-14 by the same
NJOY2016 build and script as the six above, for LCT-008's full nuclide list.

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
