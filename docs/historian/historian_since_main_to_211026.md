# OUTRAM PARK — historian report (all of `origin/develop` not in `origin/main`)

> Pre-merge-to-`main` accounting of the API tokens spent and the lines / KLOC written across this window of `develop` history. **Auto-generated** by `kovan-cli historian`; regenerate with `kovan-cli historian --from DDMMYY --to DDMMYY`.

## Scope

- **Branch:** `origin/develop` (vs base `origin/main`)
- **Window:** all of `origin/develop` not in `origin/main`
- **Commits (non-merge):** 3384
- **Token coverage:** 1223/3384 commits carry an `API-Usage-Since-Last-Commit` trailer. Commits before the token-accounting hooks existed (or made outside a Claude session) contribute 0 and are counted here as *no token data* — that is correct, not missing data.

## Totals

### Lines written (git numstat, merges excluded)

| Metric | Lines | KLOC |
|---|--:|--:|
| Added (all files) | 17,315,095 | 17315.1 |
| Removed (all files) | 1,383,157 | 1383.2 |
| **Net (all files)** | **15,931,938** | **15931.9** |
| Added (Rust `.rs`) | 2,250,856 | 2250.9 |
| Net (Rust `.rs`) | 1,898,639 | 1898.6 |

### API tokens spent

| Component | Tokens |
|---|--:|
| input | 228,814 |
| output | 61,548,357 |
| cache_read | 27,355,359,455 |
| cache_write | 377,534,871 |
| **total** | **27,794,671,497** |

_`total` = input + output + cache_read + cache_write. Cache-read (prompt-cache re-reads of the growing context) usually dominates; the output figure is the closest proxy for net generated content._

## Lines added, by crate (top 20)

| Crate | Lines added |
|---|--:|
| `njoy-outram-park-fork` | 2,357,852 |
| `openfoam-appbuilder-lib` | 2,063,431 |
| `kovan-literature` | 1,066,923 |
| `outram-mc-libs` | 646,006 |
| `kovan` | 435,950 |
| `tampines-steam-tables` | 429,540 |
| `outram-park-digital-twin-engine` | 325,769 |
| `outram-park-fork-dwsim-libs` | 297,790 |
| `tuas_boussinesq_solver` | 271,565 |
| `kovan-common` | 260,545 |
| `petir` | 254,637 |
| `changi` | 241,290 |
| `outram-blender` | 236,139 |
| `outram-park-fork-coolprop` | 230,293 |
| `outram-foam-basic-lib` | 230,238 |
| `boon-lay` | 181,477 |
| `dhoby-ghaut` | 174,285 |
| `outram-foam-appbuilder-lib` | 164,880 |
| `bedok` | 142,040 |
| `outram-park-fork-offbeat` | 141,784 |

## Per-commit ledger

| Date | Commit | Subject | +lines | -lines | Tokens |
|---|---|---|--:|--:|--:|
| 2026-06-22 | `a2715456df` | removed started TH loop | 0 | 1 | — |
| 2026-06-23 | `f2a8dcbb61` | noted that outside dome test fails again. Working on it now | 69 | 19 | — |
| 2026-06-23 | `99c15e636b` | added notes on which datapoints need fixing | 47 | 55 | — |
| 2026-06-23 | `913de832f9` | added test results into comments: CLAUDE agent generated | 444 | 5 | — |
| 2026-06-23 | `6453b29906` | added notes for CLAUDE.md that HEM has physical limitations | 33 | 15 | — |
| 2026-06-23 | `d0afc4c623` | added zaloudek in deom and subcooled files. | 1,083 | 0 | — |
| 2026-06-23 | `b6b54c7b3f` | added a basic rundown of openfoam-basic-lib | 372 | 0 | — |
| 2026-06-23 | `18566c1e8b` | added more things for sonnet to port over | 259 | 8 | — |
| 2026-06-23 | `04402b5394` | added tensors and vector3 | 1,532 | 0 | — |
| 2026-06-23 | `975d1729eb` | added math and polynomial libraries, the solvers now use C ab... | 1,612 | 0 | — |
| 2026-06-23 | `177c01fd68` | updated extern C policy | 28 | 0 | — |
| 2026-06-23 | `570a82b21b` | added interpolation matrices and ode solvers | 1,247 | 3 | — |
| 2026-06-23 | `426505998d` | openfoam basic lib at v0.1.1 | 2 | 2 | — |
| 2026-06-23 | `59a694a765` | workspace is also now v0.1.1 | 1 | 1 | — |
| 2026-06-23 | `c266e6976a` | updated claude md for specie-level thermophysics | 260 | 2 | — |
| 2026-06-23 | `2508d8d502` | scaffolded CLAUDE.md for thermophysical model conversion | 121 | 22 | — |
| 2026-06-24 | `0f0d5d2368` | openfoam-basic-lib v0.1.2: Layer 1h specie-level thermophysics | 1,165 | 3 | — |
| 2026-06-24 | `272700ac19` | added HRM critical flow solver comment | 299 | 0 | — |
| 2026-06-24 | `5f6830b406` | added polynomial models for openfoam | 492 | 3 | — |
| 2026-06-24 | `0394008848` | added pengrobinson and tablulated h into the thermo libaries ... | 779 | 4 | — |
| 2026-06-24 | `3009aa4c07` | changed glob imports as a code refactor | 71 | 131 | — |
| 2026-06-24 | `b6fd5427e6` | added updated solver import targets | 5 | 4 | — |
| 2026-06-24 | `e578c2aeab` | added openfoam fields, boundary and ldu matrix solvers | 1,749 | 2 | — |
| 2026-06-24 | `f0bdabef92` | added note on ndarray-linalg | 13 | 0 | — |
| 2026-06-24 | `32d4dad6fb` | fixed doctests and imports | 22 | 3 | — |
| 2026-06-24 | `18e776ba01` | updated doctest claude md policy | 11 | 10 | — |
| 2026-06-24 | `2a5830536a` | adding fvc and fvm, pending token reset | 719 | 0 | — |
| 2026-06-24 | `02b94de6c2` | finished adding and testing fv operators | 280 | 4 | — |
| 2026-06-24 | `3e441a9667` | openfoam-basic-lib v0.1.3: add Layer 4 fluid thermodynamics | 574 | 4 | — |
| 2026-06-24 | `638ae657bb` | openfoam-basic-lib v0.1.4: icoFoam prerequisites + multi-regi... | 1,767 | 9 | — |
| 2026-06-24 | `d7e448e0c9` | no more auto commit for CLAUDE.md | 6 | 0 | — |
| 2026-06-24 | `551cd5a3ca` | added ddt and ddt_vec | 105 | 10 | — |
| 2026-06-24 | `d8e1860bb9` | addeda  matrix benchmark test for openfoam vs ndarray linalg | 270 | 75 | — |
| 2026-06-24 | `db2d7becd5` | removed ndarray-linalg from tuas dependencies, using openfoam... | 55 | 136 | — |
| 2026-06-24 | `c8678d3f37` | tuas boussinesq solver now bumped to v0.1.2, remoed the ndarr... | 30 | 28 | — |
| 2026-06-24 | `85ac0d8b27` | moved tampines-steam-tables ndarray-linalg to dev depdencies | 59 | 3 | — |
| 2026-06-24 | `3d03b577ca` | added test backlog for openfoam-basic-libs | 95 | 0 | — |
| 2026-06-24 | `0ac6fcb596` | added new tests, need to pass them | 360 | 24 | — |
| 2026-06-24 | `19493f7445` | openfoam-basic-lib v0.1.5: P2 tests, ignore known failures | 134 | 3 | — |
| 2026-06-24 | `715c521d1d` | added dwsim-libs and openmc-libs barebones, just a start of t... | 1,810 | 0 | — |
| 2026-06-25 | `f1e63ed779` | in process of adding boon lay | 414 | 6 | — |
| 2026-06-25 | `fb616d0f99` | added boon lay scaffold and openmc rng distributions | 108 | 34 | — |
| 2026-06-25 | `9b79c7b93f` | partially migrated boon lay decay simulator and triso simulat... | 13,167 | 24 | — |
| 2026-06-25 | `d53b1344ca` | added two phase thermosyspro modelica choked flow implementat... | 220 | 0 | — |
| 2026-06-25 | `d548fd2eca` | scaffolded openfoam-appbuilder-lib and openfoam-turbulence-lib | 543 | 1 | — |
| 2026-06-25 | `146714973a` | added first import of openfoam-appbuilder-lib and openfoam-tu... | 1,155 | 35 | — |
| 2026-06-25 | `1b1b5e7cf7` | added nnc261 matrix from matrix market | 1,657 | 22 | — |
| 2026-06-25 | `e772b001e0` | added hrmfoam rho central foam, rhopimplefoam and sonic foam ... | 898 | 110 | — |
| 2026-06-25 | `c9bcf20cf3` | added nnc261 solve for verification | 135 | 0 | — |
| 2026-06-25 | `533aa2e6de` | nnc261 now comes with an explanation | 20 | 5 | — |
| 2026-06-25 | `9ca8a7769b` | added license to openfoam-appbuilder-lib and openfoam-turbule... | 4,457 | 31 | — |
| 2026-06-25 | `c92f64dea5` | bumped teh o prke to v0.1.1 | 3 | 3 | — |
| 2026-06-25 | `d80576a221` | started scaffolding the openFoam comparison files | 2,030,410 | 0 | — |
| 2026-06-25 | `a4ebf4e57c` | noted that pimpleFoam cavity flow is from the icoFoam verific... | 44 | 14 | — |
| 2026-06-25 | `3bcaf2a73c` | polyMesh parser now works! | 762 | 24 | — |
| 2026-06-25 | `37c4727107` | pimplefoam cavity blockMesh runs okay | 0 | 1 | — |
| 2026-06-25 | `e3b4a61020` | complete boon-lay triso simulator example port; mandate relea... | 1,743 | 54 | — |
| 2026-06-25 | `0d65e0691a` | wire up pimpleFoam/rhoCentralFoam/rhoPimpleFoam tutorial test... | 685 | 107 | — |
| 2026-06-26 | `7bec91e264` | document pimpleFoam port: OpenFOAM source, change justificati... | 157 | 1 | — |
| 2026-06-26 | `86368b7afe` | sketch MC transmutation design in boon-lay CLAUDE.md | 40 | 4 | — |
| 2026-06-26 | `f3aed30442` | add fvcDdtPhiCoeff limiter + fix PISO corrector loop; cavity ... | 195 | 136 | — |
| 2026-06-26 | `13ab29e659` | clarify Zaloudek data is HEM-computed, not experimental measu... | 20 | 9 | — |
| 2026-06-26 | `831a688240` | add MUSCL reconstruction (2nd-order rhoCentralFoam) and imple... | 715 | 56 | — |
| 2026-06-26 | `b676ea9379` | add nuclear-data distribution design notes for the OpenMC port | 202 | 0 | — |
| 2026-06-26 | `3d11770916` | added some stuff on openfoam-appbuilder-lib | 1,227 | 682 | — |
| 2026-06-26 | `50c9567979` | rhopimplefoam undergoing fix | 232 | 96 | — |
| 2026-06-26 | `4d33b2ee8c` | added rhopimpelfoam fix | 81 | 34 | — |
| 2026-06-26 | `4f9092e3db` | added ghia benchmark | 134 | 11 | — |
| 2026-06-26 | `8a4395d5ad` | benchmarks now have csv data to plot and show that they work! | 71 | 6 | — |
| 2026-06-26 | `3dbbdd9b18` | Add HEM critical-flow solver for superheated vapour / supercr... | 404 | 18 | — |
| 2026-06-26 | `89b848fa5b` | Claude added possible causes. numerics being no.4 | 76 | 0 | — |
| 2026-06-26 | `b164d81b0a` | added opus conversation | 22 | 0 | — |
| 2026-06-26 | `2bf191343e` | added notes for the ai agent to consider formulating a soluti... | 64 | 21 | — |
| 2026-06-26 | `4f57b80332` | tampines: add bubble-point HEM artifact diagnostic sweep | 110 | 0 | — |
| 2026-06-26 | `06e33fb0db` | tampines: fix near-saturation (x≈0) HEM choked-flow artifact | 224 | 154 | — |
| 2026-06-27 | `3dc3634458` | Add mandatory human interface layer design principle to CLAUD... | 36 | 0 | — |
| 2026-06-27 | `fffa4b2eb8` | Add project motivation (OpenFOAM negative example) to all ope... | 80 | 0 | — |
| 2026-06-27 | `5e2f03389d` | Add mandatory Rust design rules: enums over trait objects, no... | 188 | 0 | — |
| 2026-06-27 | `003e402a53` | docs: add AI-assisted development cost vs. value note to README | 57 | 0 | — |
| 2026-06-28 | `559cdbeaa0` | claude md trimmed down | 2,655 | 2,458 | — |
| 2026-06-28 | `243c277744` | Scaffold njoy-outram-park-fork: Rust port of NJOY2016 | 598 | 0 | — |
| 2026-06-28 | `d44e477589` | njoy-outram-park-fork: Phase 1 — ENDF tape reader + physical ... | 1,007 | 34 | — |
| 2026-06-28 | `ac619bb9a2` | njoy-outram-park-fork: add U-235 integration tests (485K line... | 485,519 | 0 | — |
| 2026-06-28 | `4640bce194` | njoy-outram-park-fork: Phase 2a — RECONR linearisation + MF=1... | 2,617 | 0 | — |
| 2026-06-28 | `3494666869` | Phase 2b SLBW/MLBW resonance reconstruction + uom OOP API | 48,564 | 248 | — |
| 2026-06-28 | `54bf53405d` | Add MtReaction enum; replace raw i32 MT numbers in public API | 560 | 44 | — |
| 2026-06-28 | `827f2feeb6` | Prepare njoy-outram-park-fork for crates.io publish | 67 | 55 | — |
| 2026-06-28 | `a747d07a28` | Exclude integration tests and ENDF fixtures from published crate | 2 | 1 | — |
| 2026-06-29 | `b73a0f575a` | njoy debugging via opus complete for reconr and broadr, pendi... | 1,385 | 136 | — |
| 2026-06-29 | `46f19c8f9d` | merged from branch PR | 391 | 248 | — |
| 2026-06-29 | `50632f05d2` | HEM dispatcher for tampines steam tables now working - debugg... | 355 | 87 | — |
| 2026-06-29 | `edc234bbdd` | doc comments debugged | 2 | 2 | — |
| 2026-06-29 | `0ffbe91eec` | added mass and energy balance interface | 470 | 2 | — |
| 2026-06-29 | `3a6fadcac9` | in the midst of debugging moddy isobars... | 44 | 15 | — |
| 2026-06-29 | `ccd3e9ce92` | added debug notes, and removed ndarray-linalg from tampines-s... | 45 | 10 | — |
| 2026-06-29 | `f4f999081b` | added todo for ghia benchmark | 23 | 0 | — |
| 2026-06-29 | `aab9e7a87d` | added polymesh for cavityFine map | 20,586 | 0 | — |
| 2026-06-29 | `54692c1f1a` | pimpleFoam cavity test now has mesh refined case | 144 | 26 | — |
| 2026-06-29 | `7e759c6918` | added performance benchmarks | 44 | 0 | — |
| 2026-06-29 | `1585dacd6e` | moody tests passing, but the zaloudek tests failing. Also ope... | 435 | 65 | — |
| 2026-06-29 | `85ce745b0e` | added gamg solver, works well but doesn't speed things up | 910 | 468 | — |
| 2026-06-29 | `7173480c53` | moody tests pass! | 18 | 9 | — |
| 2026-06-29 | `c7723157ab` | added temporary fix for moody's datapoints | 48 | 14 | — |
| 2026-06-29 | `3be7a23e53` | added subcooling degree as discriminator | 44 | 4 | — |
| 2026-06-29 | `20e20d3ff9` | debugging moody test, tolerance TBD | 79 | 23 | — |
| 2026-06-29 | `561a6ffec5` | renamed mod to interface | 3 | 3 | — |
| 2026-06-29 | `7b6def03e5` | added notes about interface | 4 | 0 | — |
| 2026-06-29 | `aff73b84a9` | added some comments to openfoam input and output and stuff | 3 | 0 | — |
| 2026-06-30 | `e2e4dac8c1` | Complete Moody deeply-subcooled assertions; document isobar_0... | 225 | 29 | — |
| 2026-06-30 | `1ae3670a46` | v0.2.1 ready to push to cargo, moody tests are passing | 74 | 63 | — |
| 2026-06-30 | `56e1ce2098` | Moody: run isobar_pref_0_25 in-dome-only; comment out deep di... | 119 | 43 | — |
| 2026-06-30 | `59539f7e03` | added ace file writer and zaloudek/moody data | 1,639 | 12 | — |
| 2026-06-30 | `b495083e68` | added zaloudek data for all curves. | 388 | 0 | — |
| 2026-07-01 | `a562538b95` | njoy ACER Phase 4c: elastic angular distribution (MF=4 → LAND... | 587 | 25 | — |
| 2026-07-01 | `b56c1fd502` | added a one d mesh constructor | 258 | 23 | — |
| 2026-07-01 | `d47e249a71` | added one dimensional meshing to openfoam-basic-libs, plus ch... | 96 | 35 | — |
| 2026-07-01 | `c250679306` | fixed polymesh builder error | 2 | 2 | — |
| 2026-07-01 | `3def31133b` | njoy ACER 4d start (MF=5→Law 4) + Phase-4 scaffolds (S(α,β), ... | 480 | 5 | — |
| 2026-07-01 | `01bda7c11e` | Scaffold TampinesSteamArray: 1-D rhoPimpleFoam on FvMesh | 417 | 5 | — |
| 2026-07-01 | `1c332c1de6` | explained my dilemma with openfoam and tampines steam array. ... | 112 | 0 | — |
| 2026-07-01 | `0de3b57d2a` | docs(tampines): record appbuilder as steam-table consumption ... | 39 | 4 | — |
| 2026-07-01 | `ea4fdfcadb` | outram park ace 4d complete | 126 | 0 | — |
| 2026-07-01 | `026320d133` | added thermal writer mf7 | 27,213 | 6 | — |
| 2026-07-01 | `3af55170ba` | njoy ACER 4d: wire the DLW block — loadable table with energy... | 532 | 102 | — |
| 2026-07-01 | `e9b0fabec7` | njoy ACER 4d: discrete-level non-elastic angular (MF=4 → AND) | 136 | 49 | — |
| 2026-07-01 | `ed7367547a` | njoy THERMR: coherent-elastic (Bragg) thermal scattering | 131 | 6 | — |
| 2026-07-01 | `e213cbe2f0` | docs: record the "own Rust parser, no C++ interop" rule for O... | 22 | 0 | — |
| 2026-07-01 | `3c8e2c50d4` | njoy THERMR: incoherent-inelastic scattering from S(alpha,beta) | 281 | 5 | — |
| 2026-07-01 | `85ab3210b0` | in the middle of adding s(alpha,beta) ace writer. I ran out o... | 118 | 14 | — |
| 2026-07-01 | `a904010322` | in the midst of adding ace test for thermal s(alpha,beta) | 353 | 71 | — |
| 2026-07-01 | `b16eea2758` | added claudemd for openfoam algorithms | 116 | 0 | — |
| 2026-07-01 | `957ff48763` | unit tests passing for openfoam import, but doctests failing | 15,212 | 6 | — |
| 2026-07-01 | `99d7d67b3d` | fixed doctests... but this assumes my openfoam algos are pub ... | 6 | 7 | — |
| 2026-07-01 | `31ea509f09` | tampines steam table tests pass! | 3 | 3 | — |
| 2026-07-01 | `1b3f2ee0a4` | gamg doctest reset | 1 | 1 | — |
| 2026-07-01 | `6639d11a50` | added pub crate port debt | 26 | 0 | — |
| 2026-07-02 | `e87c7ff50a` | chore(tampines): make openfoam_source pub(crate) only | 38 | 38 | — |
| 2026-07-02 | `b7be46911f` | ensure memory leaks do not happen in tests | 124 | 15 | — |
| 2026-07-02 | `c9b5df2aec` | incoherent elastic complete! | 63 | 0 | — |
| 2026-07-02 | `47fa4ce688` | incoherent elastic complete | 250 | 8 | — |
| 2026-07-02 | `46e2ec7635` | njoy: H(ZrH) incoherent-elastic thermal ACE integration test | 16,444 | 0 | — |
| 2026-07-02 | `cd839a4365` | Neutronics scaffold: njoy owns all nuclear data; Keff + Doppl... | 682 | 75 | — |
| 2026-07-02 | `f8594518b6` | in midst of validating with u238 endf data | 430 | 42 | — |
| 2026-07-02 | `e8713d68f9` | njoy hdf5 u238 wmp done | 339 | 31 | — |
| 2026-07-02 | `a2f4dfb336` | wmp finished! ish | 5 | 3 | — |
| 2026-07-02 | `83a6c192e8` | added wmp nuclide manifest | 444 | 0 | — |
| 2026-07-02 | `c708671434` | wmp manifest: expand CORE to package D (LFTR salts) | 16 | 10 | — |
| 2026-07-02 | `bbbd5d4cb3` | keff roadmap: fast range via multigroup, not pointwise lean-ACE | 37 | 5 | — |
| 2026-07-02 | `934ca2676d` | docs: data acquisition tiers + parallel-run download safety | 126 | 0 | — |
| 2026-07-02 | `f5ead15332` | docs(data-acquisition): collapse to two tiers — LOW (WMP+10-g... | 24 | 11 | — |
| 2026-07-02 | `0470a0cfd1` | njoy WMP: implement WMPB v1 blob codec (to_blob / from_blob) | 343 | 12 | — |
| 2026-07-02 | `9dabaa6c15` | njoy WMP: add WmpLibrary multi-nuclide WMPL container | 206 | 0 | — |
| 2026-07-02 | `84dd677b4f` | njoy WMP: bake CORE blob (always embedded) + Watt-weighted fa... | 544 | 64 | — |
| 2026-07-02 | `299ef48bc8` | njoy: bake fast-range MGXS from ENDF/B-VIII.0 (RECONR MF=3 → ... | 638 | 8 | — |
| 2026-07-03 | `32f5d034ee` | openmc: first end-to-end Godiva Keff (WMP+MGXS → power iterat... | 862 | 95 | — |
| 2026-07-03 | `3ac34edcbf` | njoy: wire HIGH-fidelity ENDF download + selectable MGXS weig... | 1,151 | 27 | — |
| 2026-07-03 | `cb44e15d53` | openmc: HIGH-fidelity Godiva on device-reconstructed ENDF + L... | 488 | 48 | — |
| 2026-07-03 | `c7615c3152` | openmc: model inelastic scattering — Godiva HIGH −2510 pcm | 428 | 62 | — |
| 2026-07-03 | `dd2a78b104` | openmc: anisotropic MF=4 elastic scatter — Godiva HIGH reache... | 303 | 75 | — |
| 2026-07-03 | `0928103834` | LOW tier: inelastic + forward elastic scatter; add pebble_bed... | 1,090 | 81 | — |
| 2026-07-03 | `94b6262295` | openmc-libs: (n,2n) yield-2 multiplicity + canonical-source p... | 238 | 90 | — |
| 2026-07-03 | `c8815cfbac` | godiva keff tests ongoing | 462 | 38 | — |
| 2026-07-03 | `d84fa5c08b` | openmc/njoy: energy-dependent MF=5 fission spectrum χ (HIGH t... | 98 | 9 | — |
| 2026-07-03 | `dd20bcc9da` | njoy/openmc: finish MF=5 porting — LF=7/9/11 + NK>1 mixtures;... | 651 | 93 | — |
| 2026-07-03 | `0114a6bde1` | gaspr finished | 330 | 1 | — |
| 2026-07-03 | `3d7e3b0eda` | docs: GASPR V&V entry + porting-plan Phase 3 note | 66 | 1 | — |
| 2026-07-03 | `7512665aef` | docs: THERMR + thermal ACE writer were already done — fix sta... | 47 | 38 | — |
| 2026-07-03 | `50ed17ccc0` | docs(thermal): mark IFENG=1/2 and nmix>1 gaps inline, not jus... | 18 | 1 | — |
| 2026-07-03 | `dbd50ec0fc` | docs: HEATR phased porting scaffold (H1-H7) | 44 | 1 | — |
| 2026-07-03 | `679e3ac603` | njoy: HEATR H1 — elastic kinematic heating (MT=2) | 310 | 0 | — |
| 2026-07-04 | `19c43e7d00` | njoy: HEATR H2 — local-deposition reactions (MT=102, 103-117) | 87 | 15 | — |
| 2026-07-04 | `42aa8ccc0c` | njoy: HEATR H3 -- single-escaping-neutron reactions (discrete... | 124 | 10 | — |
| 2026-07-04 | `36a78e83fe` | njoy: HEATR H4 -- fission heating (MT=18, 19-21, 38) | 371 | 49 | — |
| 2026-07-04 | `eeedd7cabf` | njoy: HEATR H5 — multi-neutron-exit + continuum inelastic (MT... | 317 | 28 | — |
| 2026-07-04 | `5814a2b717` | njoy: HEATR H7 (elastic) — damage-energy production (MT=444),... | 285 | 6 | — |
| 2026-07-04 | `0a77311827` | njoy: HEATR H5 — MF=6 emission spectra via EmissionSpectrum enum | 171 | 25 | — |
| 2026-07-04 | `364e86112e` | njoy: ACE 4e — wire HEATR H1–H5 KERMA into the ESZ heating co... | 194 | 25 | — |
| 2026-07-04 | `1ab83314fb` | added some todos for tampines and tuas for porting to android | 113 | 0 | — |
| 2026-07-04 | `dc657f7d26` | njoy: HEATR H7 — discrete-level damage energy (MT=51–90) | 182 | 60 | — |
| 2026-07-04 | `5a72772b0c` | njoy: HEATR H6 (part 1) — photon-production parser (MF=12 LO=... | 434 | 0 | — |
| 2026-07-04 | `bfd6f4e095` | njoy: HEATR H6 (part 2) — energy-balance KERMA + wire into ACE | 61 | 18 | — |
| 2026-07-06 | `1cd53a571b` | tampines: fix Edwards-pipe test path in openfoam_algorithms C... | 78 | 59 | — |
| 2026-07-06 | `7314693240` | scaffolded nee soon and started readmes ports for njoy | 2,364 | 118 | — |
| 2026-07-06 | `d34478c06a` | added sod shock tube claude md | 255 | 0 | — |
| 2026-07-06 | `18d7a6fb6d` | cleaned up tampines-steam-tables dependencies to not use tuas... | 11 | 850 | — |
| 2026-07-06 | `33f44af6f1` | sod shock tube now validated! | 778 | 5 | — |
| 2026-07-06 | `b5b8845228` | added github readme to sod shock tube | 334 | 0 | — |
| 2026-07-06 | `e6059dcd8e` | changed github markdown, hopefully it renders equations cleanly | 57 | 39 | — |
| 2026-07-06 | `5435c5b2ae` | changed claude md to include readme markdown checks | 36 | 0 | — |
| 2026-07-06 | `86c8513db2` | njoy u238 test port in progress, updated openfoam sod shock t... | 196,888 | 278 | — |
| 2026-07-06 | `724653964f` | doppler test initial commit ok | 200,512 | 23 | — |
| 2026-07-06 | `4768abef7a` | njoy: UNRESR physics kernel port (ENDF LRU=2, Faddeeva W-func... | 1,566 | 43 | — |
| 2026-07-06 | `bafda7687f` | njoy: PURR scaffolding port (ENDF reuse, uw2, RNG, ladder gen... | 785 | 61 | — |
| 2026-07-06 | `367644e97d` | njoy: PURR unrest port -- Monte Carlo probability-table binni... | 881 | 74 | — |
| 2026-07-07 | `51335d73e7` | njoy: SAMM Phase 1 -- ENDF LRF=7/KRM=3 (R-matrix-limited) par... | 418 | 23 | — |
| 2026-07-07 | `959a4a4a8d` | njoy: SAMM Phase 2 -- spin/parity/penetrability setup (non-Co... | 758 | 38 | — |
| 2026-07-07 | `ced6c22138` | njoy: SAMM Phase 3 -- Coulomb wave functions, plus betset's n... | 1,433 | 46 | — |
| 2026-07-07 | `d6a6fe845d` | njoy: SAMM Phase 4 -- R-matrix inversion (LINPACK-style solver) | 939 | 13 | — |
| 2026-07-07 | `f465587c4d` | njoy: split samm::coulomb into a directory module by function... | 1,168 | 1,097 | — |
| 2026-07-07 | `9c6c9b5aca` | njoy: SAMM Phase 5 -- cross-section formula (first end-to-end... | 757 | 30 | — |
| 2026-07-07 | `2dfe17b406` | njoy: SAMM Phase 6 -- top-level orchestration (setup + cssammy) | 215 | 20 | — |
| 2026-07-07 | `7651ce3dcb` | njoy: wire samm (LRF=7 R-Matrix Limited) into RECONR's resona... | 173 | 20 | — |
| 2026-07-07 | `ffc0d00e45` | njoy: fix U-238 capture wing pedestal (RECONR grid density) +... | 726 | 47 | — |
| 2026-07-07 | `3685314080` | openmc: add Godiva HIGH-fidelity k_eff V&V doc (validation.md) | 58 | 0 | — |
| 2026-07-07 | `23fbd81382` | added trademarks and attributions to every crate | 1,130 | 0 | — |
| 2026-07-08 | `c87bbb8e29` | bd init: initialize beads issue tracking | 740 | 0 | — |
| 2026-07-08 | `62ae613153` | workspace: adopt beads (bd) for issue/roadmap tracking; seed ... | 73 | 1 | — |
| 2026-07-08 | `d1fe5a33df` | beads: defer njoy fissile-path speedup (research-worthy); mar... | 2 | 2 | — |
| 2026-07-08 | `b97198bd95` | beads: track tampines blowdown-test PR (Ethan) + trademark-co... | 10 | 0 | — |
| 2026-07-08 | `5ce3b023f6` | beads: prke/tuas inline matrix solvers (drop openfoam-basic-l... | 6 | 2 | — |
| 2026-07-08 | `54fe11797c` | beads: plan outram-park-fork-coolprop (CoolProp Rust translat... | 1 | 0 | — |
| 2026-07-08 | `9fba09c386` | beads: refresh issues.jsonl export (coolprop cross-links + note) | 7 | 2 | — |
| 2026-07-08 | `06db42b8f0` | android: gate ndarray-linalg matrix bench off Android + add A... | 40 | 0 | — |
| 2026-07-08 | `00163d3fc0` | beads: refresh export (Android epic links) | 6 | 2 | — |
| 2026-07-08 | `89dd390728` | prke/tuas: inline SquareMatrix LU solver; drop openfoam-basic... | 596 | 11 | — |
| 2026-07-08 | `66e4632a70` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `7fdda9f632` | coolprop: initiate outram-park-fork-coolprop; first verified ... | 766 | 3 | — |
| 2026-07-08 | `23474425dd` | beads: refresh export | 6 | 2 | — |
| 2026-07-08 | `3fa43ec07f` | coolprop: add Helium fluid + EnthalpyEntropyOffset ideal term | 105 | 1 | — |
| 2026-07-08 | `cf32270cf9` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `be3a19407d` | coolprop: full 137-fluid codegen coverage + term-type engine ... | 800 | 14 | — |
| 2026-07-08 | `853a851170` | coolprop: wire all 137 CoolProp fluids into the Fluid enum | 5,168 | 69 | — |
| 2026-07-08 | `b2a903ed2d` | coolprop: vendor openfoam_algorithms + add OPCPFluidSingleCV ... | 17,074 | 21 | — |
| 2026-07-08 | `d396febf84` | coolprop: rename TampinesSteamArray -> OPCPFluidArray + wire ... | 113 | 48 | — |
| 2026-07-08 | `1231c5e027` | coolprop: drop the unused vendored solvers, keep only rhoPimp... | 23 | 609 | — |
| 2026-07-08 | `0cff3fde72` | coolprop: add transport (mu/lambda), saturation ancillaries, ... | 2,795 | 48 | — |
| 2026-07-08 | `a90cb2b0f1` | coolprop: port Helium's hardcoded transport (mu + lambda) | 193 | 25 | — |
| 2026-07-08 | `25433b20c4` | coolprop: port Water (IAPWS) and CO2 (Laesecke/Huber) hardcod... | 189 | 25 | — |
| 2026-07-09 | `840ef1ae6a` | changed tuas to v0.1.3 | 7 | 3 | — |
| 2026-07-09 | `a69e0e76b5` | added teh-o-prke to next level, will yank v0.1.1 | 21 | 3 | — |
| 2026-07-10 | `48fb5de705` | coolprop: expand hardcoded transport + add Olchowy-Sengers cr... | 747 | 310 | — |
| 2026-07-10 | `67be164c6c` | coolprop: finish all per-fluid hardcoded transport formulas | 366 | 44 | — |
| 2026-07-10 | `1ba44705c8` | coolprop: scaffold HumidAir, incompressibles, mixtures, non-a... | 804 | 8 | — |
| 2026-07-10 | `fd4b8e8504` | coolprop: implement non-analytic terms, HumidAir, incompressi... | 1,661 | 299 | — |
| 2026-07-10 | `fc0b516374` | coolprop: document why the remaining scaffold gaps are scoped... | 41 | 4 | — |
| 2026-07-10 | `28f8a90f31` | coolprop: port all 126 CoolProp incompressible fluids via cod... | 3,802 | 114 | — |
| 2026-07-10 | `70066fe2d8` | coolprop: port 840 CoolProp mixture binary pairs via codegen | 2,539 | 141 | — |
| 2026-07-10 | `90cffff84d` | workspace: scaffold verification_and_validation/, upstream_so... | 3,217 | 177 | — |
| 2026-07-10 | `136d1a4e89` | coolprop: add auto-generated docs/api.md via rustdoc-md (nigh... | 14,283 | 1 | — |
| 2026-07-10 | `50ea849a97` | beads: file op-4wl.2 (replace RELAP/Zweibaum loss coeffs with... | 1 | 0 | — |
| 2026-07-10 | `130ecd4af6` | added debug to opcp fluid array | 1 | 1 | — |
| 2026-07-10 | `3017f6a7bb` | openfoam-*-lib: add docs/api.md; file TODO to rename to outra... | 15,131 | 10 | — |
| 2026-07-10 | `ecebd56569` | beads: refresh export (op-ahi epic note timestamp) | 3 | 3 | — |
| 2026-07-11 | `9034104b57` | CLAUDE.md: add mandatory working-hours guardrail (AI safety) | 38 | 0 | — |
| 2026-07-11 | `abe0e57f40` | beads: file op-fvc (cross-platform work-hours time-check scri... | 1 | 0 | — |
| 2026-07-13 | `b5c8bd8da6` | Rename OpenFOAM/OpenMC crates to outram-foam-*/outram-mc-libs | 8,726 | 586 | — |
| 2026-07-13 | `ec21a70048` | docs: add valuation.md (cost/value estimate); close op-cpe | 144 | 5 | — |
| 2026-07-13 | `194c75dc35` | coolprop: implement HumidAir wet-bulb + dew-point; add missin... | 498 | 36 | — |
| 2026-07-13 | `c4e69746f6` | coolprop: fix stale ha_props doc comment (T_wb/T_dp are now o... | 8 | 4 | — |
| 2026-07-13 | `cbb2460d43` | coolprop: HumidAir entropy S + T_wb/T_dp as input keys (op-kb... | 514 | 79 | — |
| 2026-07-13 | `386cba4bc3` | outram-foam/outram-mc crates: align version to 0.1.0 | 4 | 4 | — |
| 2026-07-13 | `c58aae9653` | added important provenance does | 25 | 11 | — |
| 2026-07-13 | `1c2293debd` | docs: finish provenance pass -- dwsim README, OpenFOAM matrix... | 60 | 6 | — |
| 2026-07-13 | `d144faeae4` | added new digital twin plan markdown and beads | 195 | 1 | — |
| 2026-07-13 | `913d591e1c` | coolprop: wire lateral coupling + heat source into step(), cl... | 377 | 2 | — |
| 2026-07-13 | `5bca4347fe` | tampines crate added | 141 | 25 | — |
| 2026-07-13 | `e778c4c115` | did some housekeeping and AI USAGE markdowns | 652 | 10 | — |
| 2026-07-13 | `c61d14513d` | added responsible use to outram park | 338 | 23 | — |
| 2026-07-13 | `bd1df92655` | docs: wire RESPONSIBLE_USE/DATA_POLICY/AI_USAGE/etc. into roo... | 246 | 46 | — |
| 2026-07-13 | `5b18f6c3bc` | dwsim-libs: port pipe correlations (Darcy-Weisbach + Beggs-Br... | 722 | 3 | — |
| 2026-07-13 | `da722cc0b2` | docs: fold in AI Agent Instructions (license/provenance prese... | 10 | 0 | — |
| 2026-07-13 | `5f53f5e84b` | dwsim-libs: port valve/heat_exchanger/expander/pump, wire int... | 1,351 | 20 | — |
| 2026-07-13 | `e64b29bc82` | dwsim-libs: close out the 4 deferred DWSIM ports (op-qo2.6-9) | 1,080 | 11 | — |
| 2026-07-13 | `e82c8f599b` | tampines: add components/ -- 8 BOP component structs (op-dt3.4) | 439 | 1 | — |
| 2026-07-13 | `f0e1869b30` | tampines: add balance_of_plant/ + cooling_tower/ grouping mod... | 82 | 1 | — |
| 2026-07-13 | `db818e0e68` | outram-park-digital-twin-gui: crate scaffold + color_maps + c... | 741 | 3 | — |
| 2026-07-13 | `ec1482dd29` | outram-park-digital-twin-gui: add animation/ tracer/travel-ti... | 126 | 1 | — |
| 2026-07-13 | `3b4fdfc201` | outram-park-digital-twin-gui: add app_scaffold/ threading/pan... | 189 | 1 | — |
| 2026-07-13 | `58ce36502c` | tampines-steam-tables: give TampinesSteamArray a FluidArray/O... | 445 | 14 | — |
| 2026-07-13 | `356daff560` | tampines-steam-tables: wire real IAPWS-IF97 (p,h) flash into ... | 123 | 24 | — |
| 2026-07-13 | `1c59327f3c` | Add DEVELOPER_HEALTH_WARNING.md: agentic development health/s... | 429 | 1 | — |
| 2026-07-14 | `f9f6e1ad45` | Add Nordheim-Fuchs exact timestepper, wire teh-o-prke -> nee_... | 654 | 27 | — |
| 2026-07-14 | `88ffcd91ca` | beads: sync issues.jsonl export | 3 | 1 | — |
| 2026-07-14 | `86ccb04b26` | Move fhr_sim_v2 to tampines, reconstruct kinetics on Nordheim... | 251 | 173 | — |
| 2026-07-14 | `540fd45414` | bug fixes in progress | 760 | 54 | — |
| 2026-07-14 | `b49686ef3f` | tampines-steam-tables: handle two-phase region in lambda_ph_e... | 172 | 23 | — |
| 2026-07-14 | `5311d46188` | Add OpenFOAM-style pressure bounding to TampinesSteamArray + ... | 779 | 0 | — |
| 2026-07-14 | `032e95f042` | Wire TampinesSteamArray into fhr_sim_v2 steam-generator tube | 425 | 29 | — |
| 2026-07-14 | `45a3ebc58e` | tampines-steam-tables README: log the (p,h)-flash coaching as... | 27 | 0 | — |
| 2026-07-15 | `494bc90829` | beads: log vendoring rule (op-264) and kovan crate scaffold (... | 2 | 0 | — |
| 2026-07-15 | `7298c73a1a` | docs: add blanket 'unverified until validated' notice to all ... | 332 | 0 | — |
| 2026-07-15 | `9274eb6c01` | njoy: fix SAMM mf2 eliminated-channel reorder + partial ERROR... | 4,348 | 99 | — |
| 2026-07-15 | `8fd3263605` | beads: sync export after NJOY fleet run (op-cjw.3 closed, op-... | 5 | 3 | — |
| 2026-07-15 | `27744b726b` | Port NJOY front-ends: GROUPR/GAMINR/LEAPR/COVR/MIXR/RESXSR/DT... | 10,875 | 203 | — |
| 2026-07-15 | `881b86e4db` | kovan: add KOVAN knowledge layer as 7 workspace member crates | 2,838 | 8 | — |
| 2026-07-15 | `f63cdb4204` | beads: sync export (op-145 kovan scaffold closed; NJOY-rest f... | 4 | 2 | — |
| 2026-07-15 | `0d21e93ec8` | beads: add KOVAN epic op-5v5 + implementation child beads | 5 | 0 | — |
| 2026-07-15 | `bd52224698` | docs/beads: openmc-notebooks become verification tests; outra... | 43 | 0 | — |
| 2026-07-15 | `66915b1943` | outram-mc: scaffold openmc-notebooks verification harness + m... | 877 | 0 | — |
| 2026-07-15 | `7a0d9ba87d` | beads: sync export (outram-mc notebook track op-6tz.7-.22) | 24 | 0 | — |
| 2026-07-15 | `d32e223de1` | njoy op-6tz.6: OpenMC data-notebooks as verification tests (m... | 915 | 0 | — |
| 2026-07-15 | `39de0cb4e2` | beads: THERMR H-in-H2O bead op-cjw.19 (blocks op-6tz.12 therm... | 13 | 7 | — |
| 2026-07-15 | `44e2ebf7d4` | njoy thermr op-cjw.19: complete H-in-H2O S(alpha,beta) incohe... | 764 | 32 | — |
| 2026-07-15 | `ad7ee71ffa` | beads: THERMR op-cjw.19 closed; follow-ups op-cjw.20/.22/.23 | 2 | 1 | — |
| 2026-07-15 | `38faf1f8d3` | outram-mc op-6tz.7/.8/.9/.10/.16: wire CSG geometry + surface... | 1,876 | 167 | — |
| 2026-07-15 | `853d4ba745` | beads: outram-mc pincell/triso track (op-6tz.7/.8/.9/.10 clos... | 9 | 2 | — |
| 2026-07-15 | `377a687100` | outram-mc: wire H-in-H2O S(a,b) thermal scattering into trans... | 920 | 37 | — |
| 2026-07-15 | `c3673cd436` | beads: thermal pincell op-6tz.12 closed (k_inf=1.398); follow... | 10 | 6 | — |
| 2026-07-15 | `cafc3d4502` | Session batch: engine rename, KOVAN implementation, TRISO-ATO... | 32,067 | 5,211 | — |
| 2026-07-15 | `6c3fcfa1f0` | outram-mc: compare pincell k_inf vs openmc pincell.ipynb; CSV... | 24 | 1 | — |
| 2026-07-15 | `10849ffa10` | vv: commit pincell k_inf comparison CSV (force-added past fol... | 2 | 0 | — |
| 2026-07-15 | `0db6f02117` | CLAUDE.md: add optional Singlish mode (chat prose only) | 25 | 0 | — |
| 2026-07-15 | `044f1f6ef7` | outram-mc op-6tz.11: port OpenMC HexLattice + make hexagonal-... | 1,136 | 21 | — |
| 2026-07-15 | `b5ece92a49` | beads: hex-lattice op-6tz.11 closed; follow-ups op-6tz.29/.31 | 2 | 1 | — |
| 2026-07-15 | `ece1872bbf` | boon-lay: TRISO-ATOPS first-principles derivation docs (Pytho... | 718 | 3 | — |
| 2026-07-15 | `ce8f82866b` | outram-mc notebooks: DAGMC won't-port, pandas on hold, unstru... | 18 | 14 | — |
| 2026-07-15 | `e161253d18` | outram-mc op-6tz.16/.25: random TRISO packing + delta-trackin... | 1,552 | 141 | — |
| 2026-07-15 | `2877d9c529` | beads: triso finish track (op-6tz.16/.25 random packing + del... | 16 | 10 | — |
| 2026-07-15 | `c7a749a681` | tampines (p,h)-flash R4/R5 hardening + TUAS CIET pipe-38 K=17... | 1,177 | 235 | — |
| 2026-07-15 | `6dd420e651` | njoy P2/P4 fleet pass: GROUPR vector engine, LEAPR MF=7, COVR... | 8,113 | 958 | — |
| 2026-07-15 | `89340ac19c` | tuas: SAM (NED-2021 Table 4) comparison columns + AI-generate... | 838 | 18 | — |
| 2026-07-15 | `d9c293e22a` | beads: RPT reproduction intake bead op-vfb; depletion/op-3ut ... | 7 | 2 | — |
| 2026-07-15 | `4a6d608223` | outram-foam-basic-lib: tensor/vector-field FV operators (op-y... | 4,927 | 1,855 | — |
| 2026-07-15 | `f4c10b6117` | outram-foam-appbuilder-lib: GeN-Foam multiphysics port (neutr... | 24,147 | 95 | — |
| 2026-07-15 | `a56a1974e1` | beads: fleet progress sync (op-3ut GROUPR matrix + depletion ... | 2 | 1 | — |
| 2026-07-15 | `ac56abdbd9` | outram-mc: depletion / transmutation driver + LIVE depletion ... | 2,421 | 12 | — |
| 2026-07-15 | `ef567a5db7` | beads: depletion op-6tz.18 closed (CRAM); follow-ups op-23s/o... | 2 | 1 | — |
| 2026-07-15 | `87d093f346` | groupr matrix path: skeleton modules (matrix, unresolved, gam... | 112 | 0 | — |
| 2026-07-15 | `91d0bc88a8` | groupr/gaminr matrix path (op-3ut): scatter matrix, CM->lab k... | 4,057 | 87 | — |
| 2026-07-15 | `df8a82b8e6` | beads: GROUPR matrix path (op-3ut) sync | 4 | 2 | — |
| 2026-07-15 | `fc38e49c9f` | fhr_sim_v2: fix reactor-power oscillation via delayed-neutron... | 1,422 | 261 | — |
| 2026-07-15 | `0ade90a50e` | outram-park-digital-twin-engine: scaffold htgr_sim_v1 on the ... | 1,538 | 0 | — |
| 2026-07-15 | `3e2652bbed` | beads: reconcile Dolt <-> JSONL export (union of session + re... | 60 | 4 | — |
| 2026-07-15 | `7035aae6f7` | htgr_sim_v1: wire kinetics slot to the real teh_o_prke::Delay... | 76 | 200 | — |
| 2026-07-15 | `8a59f7f39c` | GeN-Foam completion: SP3, SN, thermalHydraulics (one-phase+BC... | 7,807 | 107 | — |
| 2026-07-15 | `b7ec19f05c` | kovan-cli: 'kovan setup' bootstrap for curated CLI tools; git... | 417 | 10 | — |
| 2026-07-15 | `46194ba62a` | teh-o-prke: implicit backward-Euler delayed-neutron precursor... | 229 | 80 | — |
| 2026-07-15 | `0acc13f0b2` | nee_soon: Xin Wang thesis -> markdown + njoy->openmc->genfoam... | 1,600 | 4 | — |
| 2026-07-15 | `4d57e0b7de` | kovan-cli: add gitoxide (gix) to 'kovan setup' tool list — pu... | 9 | 0 | — |
| 2026-07-15 | `d3e48c245a` | njoy groupr: self-shielded (Bondarenko-dilution) MGXS + URR P... | 2,766 | 11 | — |
| 2026-07-15 | `7be2c4129c` | beads: self-shielded MGXS op-bsz closed; op-6tz.6.3 advanced;... | 1 | 0 | — |
| 2026-07-15 | `fc1cafc63f` | digital-twin-engine: thread-panic "please restart" modal for ... | 514 | 33 | — |
| 2026-07-15 | `9bdb507124` | kovan-discovery: git-awareness via gix (library-first, binary... | 1,974 | 0 | — |
| 2026-07-15 | `2267887ff4` | beads: sync export (op-4wv crash-modal, op-5v5.7 gix, op-wqk.... | 28 | 12 | — |
| 2026-07-16 | `509c5487df` | tuas 0.1.4 + tampines-steam-tables 0.2.2: version bump + chan... | 23 | 6 | — |
| 2026-07-16 | `9f8d2e22e0` | outram-park-fork-coolprop 0.1.0: GPL-3.0 relicense + honesty ... | 772 | 25 | — |
| 2026-07-16 | `5d481e52d6` | outram-park-fork-coolprop: exclude upstream_source/CoolProp f... | 5 | 1 | — |
| 2026-07-16 | `88fe52d458` | outram-foam basic/turbulence/appbuilder: flag limitations in ... | 364 | 54 | — |
| 2026-07-16 | `595d8e5e5f` | outram-park-fork-coolprop: wave-1 property additions (op-kbc.... | 6,675 | 547 | — |
| 2026-07-16 | `bbde28ab41` | workspace: gate Android-hostile deps/examples off aarch64-lin... | 366 | 259 | — |
| 2026-07-16 | `56cc5bf794` | bookkeeping: doc-comment pass + README status flags + api.md ... | 19,921 | 9,562 | — |
| 2026-07-16 | `5a0dcd9202` | bookkeeping: add "Bookkeeping pass" command to CLAUDE.md; not... | 99 | 4 | — |
| 2026-07-16 | `2aeeb09491` | beads: sync export (pflotran op-v6s + wave-1/pki bookkeeping) | 14 | 0 | — |
| 2026-07-16 | `44975542ee` | bookkeeping: fix workspace doc drift (root README/CLAUDE.md +... | 2,669 | 527 | — |
| 2026-07-16 | `6f2005a375` | beads: bookkeeping housekeeping — retitle stale-named epics, ... | 11 | 10 | — |
| 2026-07-16 | `074c2c8c83` | outram-foam-appbuilder-lib: exclude tutorials/ from the packa... | 5 | 1 | — |
| 2026-07-16 | `8380513931` | tampines-steam-tables 0.2.3: Edwards blowdown V&V + flashing-... | 1,916 | 10 | — |
| 2026-07-16 | `46b75a6f3c` | tampines-steam-tables 0.2.4: stabilise HybridAllMach over the... | 108 | 28 | — |
| 2026-07-16 | `03690a7a79` | Document TampinesSteamArray solver (derivation + debugging) +... | 1,956 | 19 | — |
| 2026-07-16 | `9b7c1dbff2` | beads sync (op-ek2 coolprop port, op-21g.15.7 hybrid stabilit... | 27 | 1 | — |
| 2026-07-16 | `733e888bbe` | beads: sync export tick | 1 | 1 | — |
| 2026-07-17 | `a2d2025156` | outram-mc-libs: tally scoring + multigroup transport — flux_s... | 1,330 | 78 | — |
| 2026-07-17 | `110b7c1a86` | outram-foam-mesh: new crate — blockMesh, ideasUnvToFoam, poly... | 5,865 | 3 | — |
| 2026-07-17 | `8fe53754a8` | beads: sync export tick (op-ax7 mesh) | 5 | 3 | — |
| 2026-07-17 | `36428ef4b5` | outram-foam-basic-lib: OpenFOAM ASCII dictionary + case I/O (... | 2,947 | 1 | — |
| 2026-07-17 | `f819cc0416` | outram-foam-cli: scaffold OpenFOAM-style CLI (per-tool binari... | 350 | 0 | — |
| 2026-07-17 | `59305f2d12` | outram-foam-cli: wire tools to the case interface + OpenFOAM ... | 1,532 | 64 | — |
| 2026-07-17 | `74a37d6cf5` | beads: sync export tick (op-x8x CLI + follow-ups) | 3 | 1 | — |
| 2026-07-17 | `9b4509b114` | Add outram-blender scaffold: Blender-inspired mesh-authoring ... | 2,059 | 0 | — |
| 2026-07-17 | `73fbfb5e91` | outram-mc: convert 6 tractable openmc-notebook tests to LIVE ... | 2,560 | 94 | — |
| 2026-07-17 | `9896326b6e` | outram-blender: keep name, clearly mark as a Blender fork in ... | 22 | 15 | — |
| 2026-07-17 | `e3b33c95a6` | beads: outram-blender naming decision op-hzs.8 closed | 1 | 0 | — |
| 2026-07-17 | `ed2ba86414` | outram-blender: add faer (pure-Rust dense+sparse LA) for futu... | 709 | 2 | — |
| 2026-07-17 | `e9daba369f` | workspace: declare wgpu 29.0.3 (matches eframe stack); GUI/GP... | 7 | 0 | — |
| 2026-07-17 | `148f48fbfb` | outram-blender: feature-gated headless GPU compute module (wg... | 72 | 0 | — |
| 2026-07-17 | `8bd4af0357` | outram-blender: real headless GPU compute path + WGSL affine ... | 599 | 22 | — |
| 2026-07-17 | `e334013285` | beads: sync tick (outram-blender gpu op-hzs.10 closed) | 1 | 0 | — |
| 2026-07-17 | `2f74f35bdd` | outram-mc-libs: optional wgpu GPU compute -- batched pointwis... | 993 | 0 | — |
| 2026-07-17 | `e37407b349` | njoy-outram-park-fork: optional wgpu GPU compute (target-gate... | 744 | 0 | — |
| 2026-07-17 | `894aec6060` | outram-blender: replace unsafe block_on with safe Wake-based ... | 22 | 30 | — |
| 2026-07-17 | `273ae7c19f` | beads: sync tick (njoy/outram-mc/blender wgpu fleets) | 1 | 0 | — |
| 2026-07-17 | `8e5d1e583c` | beads: reconcile Dolt<->JSONL (union of session + work-PC bea... | 36 | 10 | — |
| 2026-07-17 | `be4279d78f` | beads: auto-export working post-reconcile (benchmark fleet be... | 1 | 2 | — |
| 2026-07-17 | `dfe65ee39e` | outram-mc: GPU-vs-CPU benchmarks (Godiva k_eff + HIGH-fidelit... | 1,867 | 2 | — |
| 2026-07-17 | `2b0aed5b2c` | beads: GPU benchmark fleet (op-nx0/op-6tz.37 results, op-h23 ... | 3 | 3 | — |
| 2026-07-17 | `6b125f58b8` | outram-mc: add ComputeType backend selector (single/multi/GPU... | 1,082 | 16 | — |
| 2026-07-17 | `71b63e500d` | beads: ComputeType/GPU-in-transport (op-u6s.4 progress, op-u6... | 3 | 2 | — |
| 2026-07-17 | `33ed8fbeff` | beads: sync tick (GPU + sod fleets in flight) | 3 | 1 | — |
| 2026-07-17 | `73fa32b2ac` | outram-foam-appbuilder: auto-emit plottable Sod shock tube CS... | 417 | 0 | — |
| 2026-07-17 | `eacdbde14c` | beads: sod CSV agent (op-czx) | 5 | 1 | — |
| 2026-07-17 | `3fbf67b104` | outram-mc: deepen GPU penetration — native union grid + event... | 2,766 | 37 | — |
| 2026-07-17 | `a61c95d460` | beads: GPU deep-transport (op-u6s.7 honest no-crossover, op-u... | 1 | 1 | — |
| 2026-07-17 | `f6345c6ae5` | njoy gpu: full-fidelity WMP Faddeeva pole-sum on GPU (WGSL) +... | 1,532 | 19 | — |
| 2026-07-17 | `00662c5841` | beads: njoy Faddeeva GPU (op-0m5 closed, op-0nh f32-accuracy ... | 7 | 0 | — |
| 2026-07-17 | `3a136d0e27` | docs: document the GPU f32/CPU f64 precision-vs-performance t... | 56 | 0 | — |
| 2026-07-17 | `cf05bd7ceb` | beads: sync tick (gpu collision fleet in flight) | 6 | 6 | — |
| 2026-07-17 | `29248fa68d` | outram-mc: move MC collision physics onto the GPU (op-u6s.8) | 2,583 | 17 | — |
| 2026-07-17 | `13cfd9c149` | beads: GPU collision physics op-u6s.8 closed; op-u6s.9 crosso... | 2 | 1 | — |
| 2026-07-17 | `c4e441164c` | docs: split Singlish mode into SINGLISH_MODE.md with a mainta... | 91 | 23 | — |
| 2026-07-17 | `b326ff3170` | docs(singlish): log 'can can' (short ack) + 'ho sei bo' (Hokk... | 8 | 1 | — |
| 2026-07-17 | `6b02f88445` | docs(singlish): 'ho sei bo' is standalone greeting; 'ur side ... | 7 | 1 | — |
| 2026-07-17 | `caa0a95ee3` | outram-blender: implement mesh operators, subdivision, boolea... | 3,460 | 213 | — |
| 2026-07-17 | `fde1543f5d` | beads: blender mesh impl (op-hzs.1/.2/.4/.5 closed; .3/.6/.7 ... | 10 | 5 | — |
| 2026-07-17 | `7b293846bc` | outram-foam-mesh: implement snappyHexMesh snapping + layers +... | 3,242 | 148 | — |
| 2026-07-17 | `8c7d206966` | beads: outram-foam-mesh snappy (op-ax7.2.1 closed; .2.2 parti... | 8 | 4 | — |
| 2026-07-20 | `f0ff16d85c` | outram-blender: compile wgpu unconditionally on desktop, grac... | 172 | 41 | — |
| 2026-07-20 | `aca40421cf` | gitignore: exclude Claude Code agent worktrees (.claude/workt... | 5 | 0 | — |
| 2026-07-20 | `84fd281b45` | outram-blender: add robust predicates + point-in-mesh classif... | 1,459 | 0 | — |
| 2026-07-20 | `30ee7e021e` | Gitignore vendored NJOY2016 reference build tree | 5 | 0 | — |
| 2026-07-20 | `84ff991373` | njoy(op-6tz.6.4): delayed-group MGXS collapse + live mdgxs-pa... | 433 | 10 | — |
| 2026-07-20 | `2c34042b99` | outram-foam: implement all turbulence models + advance mesh p... | 3,603 | 454 | — |
| 2026-07-20 | `dbe27541e8` | outram-blender: general mesh boolean (union/difference/non-co... | 1,184 | 83 | — |
| 2026-07-20 | `6368be8dbd` | beads: close op-hzs.14 (uv_sphere inward-winding fixed in pri... | 2 | 1 | — |
| 2026-07-20 | `9906177015` | outram-foam: regenerate docs/api.md mirrors for turbulence + ... | 4,538 | 108 | — |
| 2026-07-20 | `e6731d6970` | njoy(op-cjw.16): LEAPR frequency-integral V&V vs NJOY2016 (H-... | 86 | 0 | — |
| 2026-07-20 | `ce41d4ee77` | vv-data: add repo-tracked reference-ENDF folder (outside crat... | 49 | 0 | — |
| 2026-07-20 | `b9df5cc2e4` | outram-blender: CSG export — cylinder fitting, convex-faceted... | 466 | 20 | — |
| 2026-07-20 | `c17e1bd2c1` | njoy(op-cjw.16): validate full LEAPR pipeline (T_eff + Debye-... | 102 | 1 | — |
| 2026-07-20 | `5c124ce679` | docs(beads): migrate agent guidance from Go beads/Dolt to bea... | 55 | 30 | — |
| 2026-07-20 | `f45cb03f13` | Scaffold outram-park-fork-pflotran crate (PFLOTRAN fork, epic... | 1,202 | 0 | — |
| 2026-07-20 | `88d0e18bfd` | outram-foam-mesh: fix public-doc intra-doc links to private i... | 17 | 17 | — |
| 2026-07-20 | `9ceb04b3ba` | outram-blender: rounded (multi-segment) vertex bevel | 283 | 47 | — |
| 2026-07-20 | `53b910a88b` | outram-foam-mesh: sync docs/api.md with intra-doc-link fixes | 10 | 10 | — |
| 2026-07-20 | `09ddef751e` | beads: reconcile after develop merge — close completed njoy b... | 3 | 3 | — |
| 2026-07-20 | `01a673f3fa` | njoy(op-cjw.1): ENDF MF=31/33 covariance-section reader (ERRO... | 493 | 13 | — |
| 2026-07-20 | `6442bb349f` | njoy(op-3ut): wire stounr URR tape-locate + getsig PENDF feed... | 343 | 46 | — |
| 2026-07-20 | `6f12c6407d` | outram-blender: real-type export bridges to outram-foam-basic... | 285 | 15 | — |
| 2026-07-20 | `c2f6bf01e7` | outram-blender: update Cargo.lock for optional export-bridge ... | 2 | 0 | — |
| 2026-07-21 | `4355620c21` | docs(CLAUDE): make native-Termux compilation a HARD RULE (and... | 32 | 11 | — |
| 2026-07-21 | `4bb949acf9` | fix(outram-mc): gate godiva_gpu_benchmark example off Android... | 46 | 0 | — |
| 2026-07-21 | `c4b39491c0` | docs(CLAUDE): harden Android/Termux rule — all-targets check ... | 27 | 11 | — |
| 2026-07-21 | `ae10b541c6` | chore(beads): untrack issues.jsonl compat export, ignore per-... | 10 | 336 | — |
| 2026-07-21 | `dc295db040` | docs(CLAUDE): terminal apps (CLI/ratatui TUI) are IN scope fo... | 10 | 3 | — |
| 2026-07-21 | `bd9e5bff99` | feat(outram-mc): scaffold stochastic-media research track (CL... | 1,251 | 3 | — |
| 2026-07-21 | `cdc19bad51` | refactor(outram-mc): move stochastic-media research track to ... | 85 | 41 | — |
| 2026-07-21 | `7acc79cec0` | refactor(outram-mc): rename pebble_beds::stochastic_media to ... | 53 | 37 | — |
| 2026-07-21 | `380d897a2e` | outram-mc(op-eby.2): implement classical CLS flight driver | 182 | 27 | — |
| 2026-07-21 | `9966d21847` | outram-mc(op-eby.3): implement SCLS transport driver with ret... | 335 | 23 | — |
| 2026-07-21 | `40187700bb` | outram-mc(op-eby.7): RSA vs CLS vs SCLS absorption benchmark ... | 306 | 0 | — |
| 2026-07-21 | `29345c3ade` | outram-mc(op-eby.5): dependency-free kd-tree spatial-index ba... | 246 | 35 | — |
| 2026-07-21 | `8ec0e0b0ef` | outram-mc(op-eby.6): adaptive variable-radius SCLS extension | 148 | 1 | — |
| 2026-07-21 | `590a5c41be` | docs: add rhoPimpleFoam common-misconceptions catalogue | 455 | 0 | — |
| 2026-07-21 | `4d295cef37` | docs: backport F5 (converged residuals do not imply correctness) | 34 | 0 | — |
| 2026-07-21 | `5d92be3be8` | outram-mc: TRISO stochastic-media tutorial example (CLS/SCLS ... | 158 | 0 | — |
| 2026-07-21 | `5b3fbc84b8` | outram-mc: GPU CLS/SCLS TRISO tutorial (wgpu, CPU-referenced) | 959 | 0 | — |
| 2026-07-21 | `2539f12ca2` | outram-mc: TRISO four-method discrepancy tutorial (surface/de... | 554 | 0 | — |
| 2026-07-21 | `28764b6274` | outram-mc: rework GPU CLS/SCLS tutorial to reuse the stochast... | 54 | 45 | — |
| 2026-07-21 | `dbeaa7a7fa` | docs(tuas): cite the peer-reviewed TUAS journal article (jand... | 28 | 0 | — |
| 2026-07-21 | `9d1e353043` | docs(tuas): maintainer clears the V&V bookkeeping axis (CIET-... | 2 | 2 | — |
| 2026-07-21 | `fd8797cf63` | added derivation | 639 | 0 | — |
| 2026-07-22 | `bd548a5b00` | pflotran: wire foam-basic-lib dep + scaffold module skeleton ... | 38 | 7 | — |
| 2026-07-22 | `c18b353895` | docs(pflotran): add AI-directed-decisions review log | 89 | 0 | — |
| 2026-07-22 | `eedda1f6f3` | pflotran v1: implement grid, properties, io, Newton-Krylov so... | 4,676 | 13 | — |
| 2026-07-22 | `c8f36dd583` | pflotran: implement RICHARDS flow mode end-to-end (op-v6s.8) | 920 | 111 | — |
| 2026-07-22 | `bcb6cf8ec0` | pflotran: full test pyramid (verification/integration/regress... | 478 | 20 | — |
| 2026-07-22 | `1e9971484a` | pflotran: hydrostatic gravity verification + worked infiltrat... | 149 | 0 | — |
| 2026-07-22 | `fab428f34d` | docs(pflotran): record v1 completion status + human-review ba... | 35 | 0 | — |
| 2026-07-22 | `c010db65ca` | pflotran: Celia-1990 validation beads + mass-conservation dia... | 157 | 0 | — |
| 2026-07-22 | `971b0497f3` | pflotran: Haverkamp (1977) constitutive model for Celia bench... | 326 | 3 | — |
| 2026-07-22 | `f50e16057a` | pflotran: scaffold transport module skeleton (op-v6s.11) | 8 | 0 | — |
| 2026-07-22 | `5108932140` | docs(pflotran): record transport translation decisions (D11) ... | 29 | 0 | — |
| 2026-07-22 | `0093832406` | pflotran: conservative solute transport + RICHARDS flux coupl... | 905 | 11 | — |
| 2026-07-22 | `df9deead5d` | pflotran: scaffold energy (TH) + geochemistry module skeleton... | 17 | 0 | — |
| 2026-07-22 | `70f939ca68` | pflotran: thermal water + rock properties for TH mode (op-v6s... | 527 | 0 | — |
| 2026-07-22 | `aa1875abc5` | pflotran: aqueous equilibrium speciation core (op-v6s.12) + m... | 703 | 5 | — |
| 2026-07-22 | `cbf051cf08` | pflotran: TH energy transport + coupling; geochem compile fix... | 894 | 16 | — |
| 2026-07-22 | `2e521c62a7` | pflotran: scaffold reactive_transport module (op-v6s.12 coupl... | 8 | 0 | — |
| 2026-07-22 | `bd8056128a` | pflotran: block multi-DOF Newton-Krylov solver (op-v6s.4.1) | 1,201 | 0 | — |
| 2026-07-22 | `b75903c8c8` | pflotran: scaffold multiphase module skeleton (op-v6s.13) | 8 | 0 | — |
| 2026-07-22 | `6381ddc349` | pflotran: two-phase (air-water) multiphase flow on the block ... | 1,017 | 6 | — |
| 2026-07-22 | `9c5c261a53` | pflotran: reactive-transport coupling (transport + geochemist... | 962 | 6 | — |
| 2026-07-22 | `6346cd3c55` | pflotran: kinetic mineral geochemistry on foam ODE solver (op... | 784 | 0 | — |
| 2026-07-22 | `fa94b421ff` | docs(pflotran): update lib.rs module map for all implemented ... | 18 | 14 | — |
| 2026-07-22 | `51d975563e` | foam-basic-lib: TVD flux limiters translated from OpenFOAM li... | 225 | 0 | — |
| 2026-07-22 | `3ad750d133` | pflotran: TVD advection in transport via foam-basic-lib FluxL... | 126 | 1 | — |
| 2026-07-22 | `d17e0b4a73` | docs(pflotran): README What-exists-today reflects all transla... | 23 | 18 | — |
| 2026-07-22 | `19365a3603` | pflotran: CPU parallelism via rayon (op-v6s.14) | 128 | 81 | — |
| 2026-07-22 | `f9b62e9ada` | pflotran: optional wgpu GPU addon, Android-gated + CPU fallba... | 319 | 0 | — |
| 2026-07-22 | `edf8f34a8e` | docs(pflotran): record parallelism decision (D2: rayon + Andr... | 19 | 13 | — |
| 2026-07-22 | `ceb3048076` | docs(valuation): add 2026-07-22 token-count update | 63 | 0 | — |
| 2026-07-22 | `e0d72a8965` | docs(valuation): add without-AI replacement cost section | 36 | 0 | — |
| 2026-07-22 | `f24a53f46c` | boon-lay: Phase 0 — buffer-CLT failure analysis + first-passa... | 509 | 0 | — |
| 2026-07-23 | `aa4101f60d` | njoy-tui: mobile-first touchscreen JANIS-like nuclear-data TU... | 2,071 | 0 | — |
| 2026-07-23 | `8c6d8575c6` | boon-lay: Phase 1 — CPU Walk-on-Spheres first-passage engine | 386 | 18 | — |
| 2026-07-23 | `db2d15f8b8` | boon-lay: Phase 2 — multilayer interface transmission/reflection | 403 | 45 | — |
| 2026-07-23 | `f46b51aa6f` | outram-mc-tui: mobile-first touchscreen geometry/run TUI (op-... | 2,138 | 0 | — |
| 2026-07-23 | `ae84a04750` | boon-lay: Phase 3 — decay + transmutation depletion coupled t... | 369 | 0 | — |
| 2026-07-23 | `b883b9ba47` | boon-lay: Phase 4 — CRP-6 kernel release + interface V&V records | 374 | 25 | — |
| 2026-07-23 | `5e82733f4e` | boon-lay: Phase 5 — rayon parallel ensemble + wgpu compute sc... | 642 | 2 | — |
| 2026-07-23 | `f47c55d25c` | boon-lay: Phase 6 — real-time Walk-on-Spheres TRISO diffusion... | 273 | 0 | — |
| 2026-07-23 | `0aee140f64` | outram-mc: wire ComputeType dispatch into CSG + delta drivers... | 678 | 43 | — |
| 2026-07-23 | `9224884a2d` | Move njoy-tui + outram-mc-tui into their library crates as fe... | 155 | 102 | — |
| 2026-07-23 | `933517b09f` | pflotran: add tampines-steam-tables dep for real-EOS module (... | 5 | 0 | — |
| 2026-07-23 | `b9007cb212` | pflotran: aqueous activity coefficient models (op-v6s.15.1) | 342 | 0 | — |
| 2026-07-23 | `4d4583f2b6` | pflotran: real IAPWS water EOS + radioactive decay chains (op... | 1,102 | 0 | — |
| 2026-07-23 | `fb5b3e3099` | pflotran: equilibrium sorption — isotherms + Gaines-Thomas io... | 732 | 0 | — |
| 2026-07-23 | `e6f19f180e` | docs(pflotran): add activity/sorption/decay/eos_real to modul... | 7 | 0 | — |
| 2026-07-23 | `7e20770f6b` | outram-blender: cotangent/uniform Laplacian + implicit Laplac... | 615 | 6 | — |
| 2026-07-23 | `9bf8a798f2` | pflotran: wire radioactive decay into solute transport (op-v6... | 59 | 1 | — |
| 2026-07-23 | `8c678c66a6` | pflotran: wire linear sorption retardation into transport (op... | 97 | 5 | — |
| 2026-07-23 | `8f1e9668ef` | pflotran: microbial (Monod) biodegradation reactions (op-v6s.... | 853 | 0 | — |
| 2026-07-23 | `3ffe7ecfb1` | pflotran: wells/advanced-BCs and real-deck parser modules (op... | 1,768 | 2 | — |
| 2026-07-23 | `7e3fa96faa` | pflotran: log D14 upstream-parity wave (op-v6s.15.*) in AI-de... | 53 | 0 | — |
| 2026-07-23 | `0e2336d918` | pflotran: Pitzer ion-interaction activity model for brines (o... | 499 | 0 | — |
| 2026-07-23 | `9475ecbc5b` | examples: graceful ENDF-fetch failure in the two HIGH-fidelit... | 55 | 20 | — |
| 2026-07-23 | `480048bda2` | pflotran: unstructured finite-volume grid (TPFA) module (op-v... | 728 | 0 | — |
| 2026-07-23 | `0a3cac0e2d` | pflotran: CO2 (Redlich-Kwong) + NaCl brine EOS module (op-1y6) | 750 | 0 | — |
| 2026-07-23 | `ed715c763d` | pflotran: surface-complexation sorption (NEM/CCM/diffuse-laye... | 936 | 0 | — |
| 2026-07-23 | `a589fcd124` | pflotran: update D14 log — second parity fleet landed (224 li... | 16 | 2 | — |
| 2026-07-23 | `9cce0b86d1` | outram-blender: Taubin (lambda\|mu) shrinkage-free smoothing | 172 | 2 | — |
| 2026-07-23 | `a89f3c6a90` | outram-park-fork-liggghts: new crate + Phase 1 particle frame... | 749 | 0 | — |
| 2026-07-23 | `9002e7a622` | outram-blender: harmonic/Tutte planar parameterization | 404 | 1 | — |
| 2026-07-23 | `e5697b89ae` | outram-foam-multiphase: new crate + Stage 1 drift-flux founda... | 1,051 | 0 | — |
| 2026-07-23 | `057d8fe6b2` | outram-blender: fix parameterization test determinism + ill-c... | 26 | 9 | — |
| 2026-07-23 | `cc8fe2745c` | pflotran: GENERAL air-water-energy + two-way buoyancy TH modu... | 2,833 | 0 | — |
| 2026-07-23 | `69671cfb60` | pflotran: update D14 log — coupled-physics wave (general_mode... | 32 | 6 | — |
| 2026-07-23 | `e08fecfe4a` | outram-park-fork-liggghts: correct licensing — LIGGGHTS-PUBLI... | 62 | 59 | — |
| 2026-07-23 | `27cd2101d8` | outram-mc: search_for_keff critical-search driver + activate ... | 694 | 0 | — |
| 2026-07-23 | `15324e84d2` | outram-blender: ARAP (as-rigid-as-possible) surface deformation | 533 | 0 | — |
| 2026-07-23 | `735c9e2b5e` | outram-blender: README — document the sparse-solve operator s... | 11 | 6 | — |
| 2026-07-23 | `e9e549b386` | pflotran: robust convecting-regime solve for thermal_convecti... | 201 | 29 | — |
| 2026-07-23 | `5340a9be22` | outram-blender: QEM mesh decimation (Garland-Heckbert simplif... | 638 | 0 | — |
| 2026-07-23 | `c9be655d83` | njoy: activate mgxs-part-ii scatter-matrix + Chi tests; fix m... | 390 | 52 | — |
| 2026-07-23 | `5f0cb62f79` | outram-blender: Loop subdivision (triangle subdivision surface) | 297 | 0 | — |
| 2026-07-23 | `95fce7833d` | outram-blender: refresh stale lib.rs top-doc (whole crate is ... | 16 | 10 | — |
| 2026-07-23 | `a12db7bd7f` | feat(tooling): per-commit API-token accounting hooks + docs l... | 462 | 0 | 4,240,534 |
| 2026-07-23 | `74377bf282` | outram-park-mpi: new crate — MPICH-subset shared-memory MPI, ... | 1,180 | 0 | — |
| 2026-07-23 | `e3ef6f5a46` | fix(tooling): token ledger report — use git %x1f/%x1e format ... | 27 | 6 | 6,222,248 |
| 2026-07-23 | `e7fb2d3e4b` | chore(docs): refresh token-usage ledger | 2 | 1 | 1,457,487 |
| 2026-07-23 | `605e5126d9` | outram-blender: 3D convex hull from a point set | 402 | 0 | — |
| 2026-07-23 | `50cc6f69fb` | outram-park-mpi: collectives + reduction ops, milestone 2 (op... | 553 | 19 | — |
| 2026-07-23 | `3dcc6bad15` | outram-park-mpi: add Cargo.lock entry for the new crate | 7 | 0 | — |
| 2026-07-23 | `349d87fecf` | outram-blender: exercise the sparse-solve operators in the me... | 88 | 1 | — |
| 2026-07-23 | `9cf651c856` | scripts: add kloc_accounting.py, reproducible line-count acco... | 1,436 | 0 | — |
| 2026-07-23 | `0b1e867b8e` | outram-blender: weld / remove-doubles operator (merge coincid... | 400 | 3 | — |
| 2026-07-23 | `43c259c208` | outram-blender: fill-holes operator (cap open boundary loops ... | 259 | 4 | — |
| 2026-07-23 | `a1ea9de1bb` | outram-blender: solidify operator (shell thickness -> closed ... | 243 | 5 | — |
| 2026-07-23 | `e273ef27e0` | outram-foam Phase II: multiphase Stages 2-5 + DEM Phases 2-5 ... | 6,714 | 20 | — |
| 2026-07-23 | `95fbeda8b6` | outram-blender: recalculate-normals operator (consistent wind... | 303 | 5 | — |
| 2026-07-23 | `4de61200ff` | scripts: net TUAS against what it imported, not its predecess... | 41 | 9 | — |
| 2026-07-23 | `ad95279c48` | outram-blender: triangulate operator (ngon/quad Mesh -> trian... | 149 | 3 | — |
| 2026-07-23 | `3e65b02fd6` | outram-blender: inset-faces operator (per-face inset ring) | 197 | 5 | — |
| 2026-07-23 | `ea2643ea07` | outram-blender: bisect operator (plane cut / half-space clip) | 236 | 5 | — |
| 2026-07-23 | `62b7f22e11` | outram-blender: demo the repair/modeling/cutting operators in... | 69 | 0 | — |
| 2026-07-23 | `79dc787097` | outram-blender: revolve / spin operator (surface of revolution) | 228 | 5 | — |
| 2026-07-23 | `d3b505c8ef` | outram-blender: read polyMesh (from_poly_mesh) — round-trip p... | 113 | 8 | — |
| 2026-07-23 | `db91d17538` | docs(singlish): add 'bang gang' — knock off work / finish for... | 1 | 0 | 48,295,948 |
| 2026-07-24 | `2f5969384e` | feat(historian): pre-merge-to-main release report generator (... | 482 | 0 | — |
| 2026-07-24 | `6c50cdd2fd` | refactor(tooling): consolidate token accounting into docs/his... | 450 | 397 | 7,215,668 |
| 2026-07-24 | `a75a694725` | outram-park-fork-dwsim-libs: port compressor, heater, cooler,... | 2,112 | 0 | — |
| 2026-07-24 | `05f449c72c` | outram-park-fork-liggghts: DEM engine + rolling/cohesion, mes... | 3,322 | 0 | — |
| 2026-07-24 | `db924dd322` | Add outram-park-fork-cfmesh scaffold: vendored cfMesh + voro+... | 962 | 0 | — |
| 2026-07-24 | `744b732686` | outram-blender: STL import/export (ASCII + binary) | 351 | 0 | — |
| 2026-07-24 | `93e784744d` | outram-park-fork-dwsim-libs: Tier-1 thermodynamics kernel (EO... | 3,554 | 0 | — |
| 2026-07-24 | `035bfaa7be` | outram-blender: edge bevel (chamfer every edge) | 248 | 0 | — |
| 2026-07-24 | `b195d34778` | docs(readme): add a (clearly-in-jest) Phua Chu Kang tagline | 5 | 0 | 6,734,604 |
| 2026-07-24 | `071e4ff361` | outram-park-fork-cfmesh: volume-mesh core + Cartesian block m... | 526 | 15 | — |
| 2026-07-24 | `7693155eba` | outram-mc: port remaining CSG quadric surfaces — Plane, X/YCy... | 507 | 4 | 51,484,074 |
| 2026-07-24 | `fc7c55ff02` | chore(docs): refresh token-usage ledger | 15 | 7 | 3,281,946 |
| 2026-07-24 | `3e60439b0d` | chore(docs): refresh token-usage ledger (hooks bypassed to se... | 2 | 1 | — |
| 2026-07-24 | `13040a88be` | outram-park-fork-cfmesh: castellated Cartesian surface carve ... | 375 | 39 | — |
| 2026-07-24 | `1222b1877b` | outram-park-fork-dwsim-libs: compose thermo kernel — PT/PH fl... | 1,898 | 0 | — |
| 2026-07-24 | `6e35ca51e3` | outram-park-fork-liggghts: bonded-particle model + pebble-bed... | 1,193 | 0 | — |
| 2026-07-24 | `9b8fdf0065` | outram-park-fork-cfmesh: boundary snapping (staircase -> body... | 255 | 7 | — |
| 2026-07-24 | `1f4dd31fca` | outram-foam-multiphase: drift-flux PIMPLE pressure-velocity c... | 722 | 0 | — |
| 2026-07-24 | `d3ac4381e7` | outram-park-fork-cfmesh: foam PolyMesh bridge -> solvable FvM... | 160 | 14 | — |
| 2026-07-24 | `dc627ed4ff` | outram-park-fork-cfmesh: mesh quality checks (polyMeshGenChec... | 228 | 0 | — |
| 2026-07-24 | `347af8afb3` | outram-park-fork-cfmesh: multi-region carve (region between s... | 169 | 93 | — |
| 2026-07-24 | `59d744f697` | outram-park-fork-cfmesh: triangle-soup shape generators for r... | 217 | 0 | — |
| 2026-07-24 | `f7c5f0d9e7` | outram-park-fork-cfmesh: coolant-around-pebble end-to-end exa... | 80 | 0 | — |
| 2026-07-24 | `839d7aaece` | outram-park-mpi: communicator dup + split (op-wor) | 333 | 16 | — |
| 2026-07-24 | `a143f0e2cf` | outram-mc: fix nested-lattice surface-tracking under-count (o... | 95 | 1 | 35,914,153 |
| 2026-07-24 | `c3ffa19210` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `9bd10015b0` | pflotran: MPI domain decomposition + halo exchange, first sli... | 321 | 0 | — |
| 2026-07-24 | `1b9c2849ee` | outram-park-fork-cfmesh: per-surface boundary patch separation | 83 | 19 | — |
| 2026-07-24 | `21adbfb0ce` | outram-park-fork-cfmesh: bbox-culled multi-hole carve + pebbl... | 100 | 7 | — |
| 2026-07-24 | `2c7c77385d` | outram-foam-multiphase: two-fluid Euler-Euler shared-pressure... | 1,007 | 0 | — |
| 2026-07-24 | `67eb5112eb` | outram-park-fork-cfmesh: reactor geometry generators + LWR pi... | 206 | 30 | — |
| 2026-07-24 | `46b4ccf92f` | outram-mc: port Torus{X,Y,Z} CSG surfaces — completes the Ope... | 689 | 4 | 40,092,252 |
| 2026-07-24 | `f10a000757` | outram-mc: HexLattice 3-D axial rings + X-orientation round-t... | 0 | 809 | 8,028,654 |
| 2026-07-24 | `6a97906c27` | outram-park-fork-cfmesh: OpenFOAM polyMesh disk writer + MSR ... | 84 | 0 | — |
| 2026-07-24 | `78f56b85e3` | outram-park-fork-dwsim-libs: saturation, transport, EOS varia... | 2,971 | 0 | — |
| 2026-07-24 | `9f67449fa6` | outram-mc: add the lattice/ dir files (complete f10a000) | 1,072 | 0 | 6,064,023 |
| 2026-07-24 | `c849d272a2` | chore(docs): refresh token-usage ledger | 4 | 1 | — |
| 2026-07-24 | `5d8b5fff5a` | pflotran: HDF5 snapshot I/O via pure-Rust hdf5-pure (op-v6s.1... | 328 | 0 | — |
| 2026-07-24 | `738b04d8f1` | outram-park-fork-cfmesh: octree near-wall refinement with pol... | 368 | 0 | — |
| 2026-07-24 | `0ab7d2b0c4` | chore: pin transitive kstring to 2.0.2 for rustc 1.94 MSRV | 2 | 2 | 2,508,572 |
| 2026-07-24 | `41052a62e1` | docs: regenerate api.md mirrors for dwsim-libs, multiphase, l... | 22,747 | 1 | 1,165,500 |
| 2026-07-24 | `fed0bf3415` | chore(docs): refresh token-usage ledger | 2 | 1 | 2,321,347 |
| 2026-07-24 | `f2a83c421a` | chore(docs): flush token-usage ledger lag | 2 | 1 | — |
| 2026-07-24 | `01eb0573c1` | outram-park-mpi: groups + Cartesian topologies (op-er2) | 450 | 3 | — |
| 2026-07-24 | `9edd1c4ac7` | pflotran: distributed conjugate-gradient solve (parallel Kryl... | 249 | 1 | — |
| 2026-07-24 | `b8cfd2ea7d` | outram-park-fork-cfmesh: multi-level octree refinement + 2:1 ... | 134 | 48 | — |
| 2026-07-24 | `7d321ed9b2` | pflotran: 2-D Cartesian distributed solve (op-gj5, first slice) | 439 | 0 | — |
| 2026-07-24 | `3dc85bc57a` | cfmesh: add prism boundary layers (add_boundary_layers) | 393 | 3 | 189,149 |
| 2026-07-24 | `784f727c04` | pflotran: generic distributed CG + real variable-coefficient ... | 318 | 0 | — |
| 2026-07-24 | `aac6a3c382` | pflotran: distributed Jacobi-preconditioned CG (op-gj5) | 148 | 1 | — |
| 2026-07-24 | `efb8acdf66` | cfmesh: add polyhedral (median) dual — one cell per vertex (p... | 326 | 2 | 5,250,656 |
| 2026-07-24 | `e579dd798f` | cfmesh: V&V — polyhedral dual bridges to a solvable foam FvMesh | 20 | 0 | 2,974,882 |
| 2026-07-24 | `3f5b3740d1` | docs: refresh generated token-usage ledger | 17 | 1 | 891,335 |
| 2026-07-24 | `56eba24339` | docs: refresh generated token-usage ledger (lag row) | 2 | 1 | 754,285 |
| 2026-07-24 | `bacb3bbdca` | docs: refresh generated token-usage ledger (final lag row) | 2 | 1 | — |
| 2026-07-24 | `d654f1533e` | pflotran: distributed solve of the REAL assembled LduMatrix (... | 297 | 0 | — |
| 2026-07-24 | `35a4de88ea` | outram-foam-appbuilder-lib: reactingTwoPhaseEulerFoam applica... | 26,646 | 1,045 | 26,210,679 |
| 2026-07-24 | `f4624b1abd` | chore(docs): flush token-usage ledger lag | 15 | 1 | — |
| 2026-07-24 | `bfbc356ae7` | pflotran: distributed BiCGStab for the non-symmetric transpor... | 217 | 1 | — |
| 2026-07-24 | `22ea3bc3be` | pflotran: truly-distributed per-rank LduMatrix assembly (op-gj5) | 106 | 0 | — |
| 2026-07-24 | `2e11d6142e` | docs: refresh generated token-usage ledger (merge lag row) | 9 | 1 | — |
| 2026-07-24 | `d3f0275140` | njoy + outram-mc: make ratatui unconditional — TUI bins alway... | 67 | 90 | 32,670,485 |
| 2026-07-24 | `af6a88b1a7` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `483fd6089b` | chore(tooling): untrack docs/token-usage.md — commit trailers... | 20 | 47 | 15,508,820 |
| 2026-07-24 | `7e3201bd94` | pflotran: distributed transport timestep matching the real se... | 213 | 0 | — |
| 2026-07-24 | `186714774d` | outram-foam-appbuilder-lib: reacting-Euler species transport ... | 914 | 13 | 20,596,688 |
| 2026-07-24 | `bac6da5c86` | chore(docs): flush token-usage ledger lag | 12 | 1 | — |
| 2026-07-24 | `e597fb8969` | outram-foam-multiphase: bookkeeping pass (doc sync, api.md re... | 48 | 10 | 6,293,774 |
| 2026-07-24 | `477c9ff404` | outram-park-fork-liggghts: bookkeeping pass (doc sync, api.md... | 389 | 147 | 3,990,081 |
| 2026-07-24 | `fddb1a9363` | outram-foam-appbuilder-lib: bookkeeping pass (doc gaps + stal... | 331 | 54 | 5,605,508 |
| 2026-07-24 | `f4c3aaba5b` | outram-park-fork-dwsim-libs: bookkeeping pass (doc gaps + hon... | 1,282 | 410 | 6,102,931 |
| 2026-07-24 | `e33b8e74f7` | chore(docs): flush token-usage ledger lag | 5 | 1 | — |
| 2026-07-24 | `590b0e0c8b` | fix(outram-mc): correct hex-lattice ring→tile fill (op-6tz.38) | 280 | 12 | 46,781,908 |
| 2026-07-24 | `69d88742ba` | fix(outram-mc): compose reflective-corner reflections in one ... | 296 | 3 | 7,850,956 |
| 2026-07-24 | `0a34f095b6` | kovan-literature: add open nuclear digital-twin/shadow litera... | 491 | 0 | 43,506,855 |
| 2026-07-24 | `6d60907435` | kovan-literature: lock down verified citations in DT/shadow r... | 61 | 33 | 3,345,634 |
| 2026-07-24 | `9a7b5819b1` | kovan-literature: add inline verification-status marker to ev... | 36 | 3 | 6,177,955 |
| 2026-07-24 | `e27d65ef9c` | kovan-literature: reframe DT/shadow review around where Outra... | 90 | 13 | 17,889,996 |
| 2026-07-24 | `9d86853f40` | cfmesh: add tetrahedralization (centroid subdivision) — op-hz... | 258 | 2 | 11,477,957 |
| 2026-07-24 | `b2cece56fc` | pflotran: distributed transport for non-uniform flow (op-gj5) | 187 | 33 | — |
| 2026-07-24 | `f063e119bc` | pflotran: distributed transport with Dirichlet boundary condi... | 162 | 3 | — |
| 2026-07-24 | `fa4449c345` | feat(outram-mc): GPU ray-surface distance kernel for all 15 C... | 1,712 | 0 | 21,646,800 |
| 2026-07-24 | `834b7399f0` | cfmesh: face-minimal merged dual + smart-Laplacian quality sm... | 512 | 11 | 11,490,536 |
| 2026-07-28 | `521641763e` | pflotran: BiCGStab restart-on-breakdown + distributed TVD tra... | 265 | 34 | — |
| 2026-07-28 | `cb391324c5` | cfmesh: flip-based Delaunay improvement (2-3/3-2 bistellar fl... | 482 | 3 | 27,713,840 |
| 2026-07-28 | `ce4bd82ea7` | outram-foam-appbuilder-lib: V&V cases mirroring upstream drif... | 595 | 0 | 82,163,381 |
| 2026-07-28 | `3eb6731379` | pflotran: distributed energy (heat) transport timestep (op-gj5) | 145 | 0 | — |
| 2026-07-28 | `c025bf5330` | pflotran: distributed Newton solver for nonlinear systems (op... | 278 | 0 | — |
| 2026-07-28 | `a942f8190b` | outram-blender: Monte Carlo simulation backend (materials + r... | 500 | 14 | 28,954,759 |
| 2026-07-28 | `0f8778d339` | outram-blender/sim: add cell flux/nu-fission tally helpers | 49 | 0 | 3,920,399 |
| 2026-07-28 | `c73aa1b15a` | docs(CLAUDE.md): hard rule — agent-fleet progress updates eve... | 27 | 0 | 24,913,510 |
| 2026-07-28 | `e3f5fa1e11` | outram-blender: MC Studio — egui GUI to author + run basic ou... | 444 | 0 | 13,126,085 |
| 2026-07-28 | `bb48f28176` | Remove unused distributed_dot import in decomposition::ldu | 1 | 1 | — |
| 2026-07-28 | `001cc2c714` | docs(CLAUDE.md): hard rule — dogfood KOPITIAM in this workspace | 53 | 0 | 7,736,570 |
| 2026-07-28 | `dc3ef82c9a` | chore: keep KOPITIAM out of the OUTRAM PARK tree | 24 | 0 | 4,567,509 |
| 2026-07-28 | `4098a82e69` | digital-twin-engine: flow tracers, per-cell pipes, coupled HT... | 2,297 | 249 | — |
| 2026-07-28 | `9d51ac2d3a` | docs(CLAUDE.md): hard rule — kopitiam is binary-only, issues ... | 23 | 13 | 1,820,695 |
| 2026-07-28 | `5465ff4879` | docs(CLAUDE.md): stop hook authorises push to feature/develop... | 12 | 1 | — |
| 2026-07-28 | `e67d8b0b8f` | outram-mc: fixed-source transport driver (run_fixed_source) —... | 349 | 6 | 28,233,775 |
| 2026-07-28 | `442def8f0c` | fix(pflotran): restore distributed_dot import at test scope | 3 | 0 | 9,363,777 |
| 2026-07-28 | `7ab5c3c6a0` | cfmesh: Mesh Studio — egui GUI for polyhedral meshes + layers... | 447 | 0 | 24,536,996 |
| 2026-07-28 | `a52636a8fd` | cfmesh: adaptive boundary layers for curved walls — op-zhh (M... | 293 | 4 | 24,949,768 |
| 2026-07-28 | `3f5b191e2a` | build: add async-opcua to workspace dependencies (Android-ver... | 23 | 0 | 31,544,530 |
| 2026-07-28 | `dfc66b0034` | docs: reword async-opcua note as scope, not a user restriction | 10 | 5 | 1,876,572 |
| 2026-07-28 | `6d41e60df4` | boon-lay: CPU/GPU ComputeType resource switcher + off-thread ... | 994 | 2 | — |
| 2026-07-28 | `57bab46f4d` | boon-lay: first_passage_realtime — off-thread compute + CPU/G... | 240 | 134 | — |
| 2026-07-28 | `efa78282fb` | boon-lay: triso_simulator retrofit — WoS diffusion + CPU/GPU ... | 147 | 8 | — |
| 2026-07-28 | `b90452ddaf` | ciet: CIET Educational Simulator v2 with an OPC-UA interface ... | 24,723 | 7 | 417,150 |
| 2026-07-28 | `174e837dc8` | release: outram-park-digital-twin-engine 0.1.0 -> 0.2.0 | 3 | 3 | 8,734,716 |
| 2026-07-29 | `4345e350a3` | docs: scope the transformation to a Type I digital twin | 291 | 0 | 166,955,527 |
| 2026-07-29 | `eaac02dd1f` | docs(dt-scoping): simulator binaries may live in this repo | 36 | 17 | 9,014,289 |
| 2026-07-29 | `e802770207` | docs: scope the OFFBEAT + SCIANTIX port in detail | 188 | 0 | 29,874,580 |
| 2026-07-29 | `0ca34c9e8d` | offbeat: new crate + P0 mechanics bridgehead (epic op-6sl, be... | 1,704 | 0 | 190,671,075 |
| 2026-07-29 | `23f4d02ece` | offbeat: module stubs for the remaining port phases + Cargo.lock | 214 | 0 | 2,272,071 |
| 2026-07-29 | `3723c06cc8` | offbeat P3/P4: material property correlations, burnup, fast f... | 12,503 | 18 | 11,883,986 |
| 2026-07-29 | `4bc06269a4` | offbeat P3: behavioural models -- swelling, densification, re... | 3,516 | 73 | 5,424,301 |
| 2026-07-29 | `b1c308eac9` | offbeat P1: rheology -- plasticity and creep constitutive law... | 3,683 | 2 | 16,420,596 |
| 2026-07-29 | `e3053e3e1e` | offbeat P2: fuel/cladding gap -- conductance, gas mixture, co... | 5,467 | 2 | 2,931,872 |
| 2026-07-29 | `4b2dfb3109` | offbeat P5: cladding corrosion, hydrogen pickup, Anderson mix... | 5,034 | 2 | 7,540,360 |
| 2026-07-29 | `a3342f86ff` | docs: list outram-park-fork-offbeat in the workspace member t... | 2 | 0 | 2,320,510 |
| 2026-07-29 | `60d8ac5410` | offbeat: add the missing LICENSE, NOTICE and README | 876 | 0 | 43,035,292 |
| 2026-07-30 | `4fe7307266` | kovan-literature: correct thesis metadata extraction, add The... | 582 | 20 | — |
| 2026-07-30 | `b92e997f93` | kovan-literature: archive three open-access UC Berkeley theses | 6,031 | 0 | — |
| 2026-08-03 | `6aa61def7f` | release: patch-bump 8 crates for the digital-twin publish chain | 31 | 28 | 0 |
| 2026-08-03 | `a0e797256d` | release: placeholder 0.0.1 versions, authorship, and shipped ... | 4,298 | 19 | 6,940,322 |
| 2026-08-03 | `ad61434825` | docs: rewrite the crates.io publishing procedure from the 202... | 61 | 25 | 15,060,617 |
| 2026-08-03 | `57991e414a` | dwsim-fork: complete chemistry-model survey from upstream source | 217 | 8 | 85,709,336 |
| 2026-08-03 | `0dbe9a1457` | dwsim-fork: note HTGR water-ingress relevance in chemistry su... | 26 | 0 | 10,094,049 |
| 2026-08-03 | `b7d5baf10a` | dwsim-fork: port SLE flash, Gibbs speciation, electrolyte tie... | 2,892 | 0 | 45,403,356 |
| 2026-08-03 | `6f192e3cd8` | dwsim-libs: port reaction + reactor models (op-tts) | 1,908 | 0 | 11,902,529 |
| 2026-08-03 | `6835f3d7fc` | dwsim-libs: port advanced EOS (op-b4t) + inside-out & 3-phase... | 2,998 | 0 | 1,604,329 |
| 2026-08-03 | `123968726c` | dwsim-libs: Gibbs reactor + Langmuir-Hinshelwood catalytic ki... | 792 | 7 | 12,187,552 |
| 2026-08-03 | `1ed9c1212c` | dwsim-libs: port 5 thermo tail modules (op-qo2.10/.13/.14/.19... | 4,482 | 0 | 3,384,663 |
| 2026-08-03 | `14140ac4ad` | dwsim-libs: port SVLLE + Modified-UNIFAC-Dortmund + UNIFAC-LL... | 1,851 | 0 | 15,669,333 |
| 2026-08-03 | `22fbd81a61` | cfmesh: high-level tet-dual meshing pipeline (op-0xu) | 588 | 0 | 3,503,482 |
| 2026-08-03 | `6185c17740` | dwsim-libs: port sour-water package (op-qo2.16) | 1,123 | 0 | 5,376,231 |
| 2026-08-03 | `cdece4f376` | outram-blender: tet-dual Mesh Studio GUI + cfmesh bridge (op-... | 767 | 0 | 5,337,056 |
| 2026-08-03 | `ef6ed0ba32` | dwsim-libs: port multi-phase (N-phase) Gibbs minimisation (op... | 1,442 | 0 | 2,458,474 |
| 2026-08-03 | `451bdf924a` | chore: update Cargo.lock for outram-park-fork-cfmesh workspac... | 1 | 0 | 1,859,355 |
| 2026-08-03 | `4b8acdc316` | chore: bump versions of crates changed this session (+0.0.1 e... | 12 | 12 | 8,012,763 |
| 2026-08-03 | `b1e7b77d7a` | cfmesh: bookkeeping pass — doc/README refresh for the tet-dua... | 57 | 3 | 12,878,348 |
| 2026-08-03 | `93f232c349` | outram-mc-libs: bookkeeping pass — document fixed-source MC d... | 42 | 4 | 0 |
| 2026-08-03 | `1150d2d04a` | outram-blender: bookkeeping pass — document MC Studio + Mesh ... | 94 | 29 | 2,235,684 |
| 2026-08-03 | `559cd2fa6f` | dwsim-libs: bookkeeping pass — module map, honest scope, surv... | 219 | 99 | 15,228,568 |
| 2026-08-03 | `dd1fc4a28b` | workspace: bookkeeping — refresh member tables + stale scaffo... | 39 | 7 | 0 |
| 2026-08-03 | `8622e24f2f` | docs: regenerate api.md rustdoc mirrors for the 4 changed crates | 50,075 | 6,137 | 6,048,735 |
| 2026-08-04 | `84938054ff` | msre: scaffold outram-park-fork-{onix,thermochimica,moltres} ... | 272 | 0 | 108,134,040 |
| 2026-08-04 | `601ad91b3b` | outram-park-fork-onix: port ONIX CRAM depletion solver (op-6w... | 1,699 | 9 | 25,605,061 |
| 2026-08-04 | `b33218c08f` | outram-park-fork-thermochimica: port Thermochimica GEM core (... | 1,470 | 0 | 10,783,765 |
| 2026-08-04 | `e082e5d88d` | outram-park-fork-moltres: circulating-fuel MSR multiphysics o... | 3,124 | 9 | 7,004,597 |
| 2026-08-04 | `023e0da3bf` | outram-foam-basic-lib: add OpenFOAM patch-field boundary cond... | 736 | 10 | 33,272,447 |
| 2026-08-04 | `325ba2957b` | outram-foam-basic-lib: functional cyclic (periodic) patches (... | 898 | 21 | 20,204,442 |
| 2026-08-04 | `5423b54a03` | outram-foam-basic-lib: cyclicAMI non-conformal periodic patch... | 1,057 | 25 | 15,489,304 |
| 2026-08-04 | `5c7ccbdfab` | outram-foam-basic-lib: add OpenFOAM flow boundary conditions ... | 674 | 23 | 14,040,264 |
| 2026-08-04 | `5a434fcc82` | outram-blender: ship the GPL licence text with the crate | 674 | 0 | 6,048,938 |
| 2026-08-04 | `58b56de63f` | outram-blender: vendor Blender provenance, ship upstream lice... | 493 | 2 | 14,249,477 |
| 2026-08-04 | `5de0123264` | release: outram-blender 0.0.1 -> 0.0.2 | 3 | 3 | 5,019,031 |
| 2026-08-04 | `9b42d09f55` | release: outram-blender 0.0.3 — correct the ported-code prove... | 85 | 32 | 3,999,157 |
| 2026-08-04 | `1edb3a92e3` | docs: scope a MELCOR-class severe-accident capability | 255 | 0 | 32,124,212 |
| 2026-08-04 | `f726eb0186` | outram-foam-appbuilder-lib: restore the build — exhaustive Bo... | 58 | 0 | 816,770 |
| 2026-08-04 | `b44f549160` | widget studio: gallery app, plus a turbine widget driven by r... | 773 | 25 | 1,229,721 |
| 2026-08-04 | `3f3dedbe17` | added scoping for aster and turbine/pipe | 684 | 0 | 28,557,592 |
| 2026-08-04 | `aa7078e092` | boon-lay: fix WGSL shader parse failure — `target` is a reser... | 7 | 3 | 6,023,992 |
| 2026-08-04 | `01e0deb508` | docs: scope the code_aster constitutive-law and fracture port | 73 | 34 | 27,393,538 |
| 2026-08-04 | `1de6660e05` | turbine widget: stator rows, casing, per-stage internals, sym... | 632 | 45 | 28,726,364 |
| 2026-08-04 | `2b3473823b` | turbine widget: blade lean adjustments, stator ring, unused c... | 16 | 8 | 11,976,418 |
| 2026-08-04 | `64a4394cd1` | widget studio: pipes tab over three flow backends, and a HEM ... | 412 | 23 | 22,714,599 |
| 2026-08-04 | `d2b1cebc1f` | basic-lib: port OpenFOAM 3x3 eigen decomposition; start code_... | 5,680 | 3 | 81,012,341 |
| 2026-08-04 | `cdc362f5dd` | style: rustfmt basic-lib files that landed unformatted | 366 | 132 | 504,280 |
| 2026-08-04 | `f2721679d2` | offbeat: code_aster P0 kinematics -- Mandel Voigt + finite st... | 1,472 | 107 | 17,390,337 |
| 2026-08-04 | `abbf86db91` | offbeat: code_aster P1 local integration algorithms (op-a7p.2) | 990 | 0 | 21,151,406 |
| 2026-08-04 | `12e3b05484` | offbeat: pin the secant observed-order oscillation (op-a7p.2) | 39 | 0 | 2,998,996 |
| 2026-08-05 | `13b67a3850` | offbeat: GDEF_LOG finite-strain wrapper + fix a real eigen bu... | 803 | 4 | 26,049,479 |
| 2026-08-05 | `7bd4eac0da` | offbeat: code_aster P2 isotropic viscoplastic creep -- NORTON... | 781 | 0 | 22,775,222 |
| 2026-08-05 | `05123c466d` | offbeat: code_aster LEMAITRE_IRRA irradiation creep (op-a7p.3) | 340 | 0 | 16,115,712 |
| 2026-08-05 | `3bca0018ff` | dwsim-libs: scope the remaining UNIFAC ports (NIST-Modified U... | 235 | 0 | 264,264,032 |
| 2026-08-05 | `03279cf68b` | outram-foam: port the fvOptions/fvModels mechanism + solidifi... | 1,538 | 0 | 42,669,145 |
| 2026-08-05 | `1aefdfc81b` | Port DWSIM seawater, hydrocarbon, immiscible & black-oil pack... | 3,567 | 4 | 7,958,648 |
| 2026-08-05 | `ae45c4e3ad` | code_aster P2(a): NORTON_HOFF + isotropic hardening radial re... | 1,219 | 0 | 19,039,444 |
| 2026-08-05 | `9d179094c9` | code_aster: mid-flight checkpoint of the chaboche/metallurgy/... | 8,787 | 0 | 7,124,139 |
| 2026-08-05 | `bf88c814f6` | code_aster: checkpoint 2 -- chaboche and fracture revisions | 512 | 286 | 3,661,705 |
| 2026-08-05 | `6463f52167` | code_aster: land the fracture and Chaboche ports, wire re-exp... | 223 | 96 | 8,550,871 |
| 2026-08-05 | `c70be99150` | code_aster: checkpoint the damage/rupture port (op-a7p.4) | 4,414 | 8 | 2,762,621 |
| 2026-08-05 | `57a024881a` | code_aster: land the damage/rupture port (op-a7p.4), wire re-... | 133 | 7 | 5,688,444 |
| 2026-08-05 | `30dce1aeaf` | code_aster: land the metallurgy port, complete the fleet, ref... | 41 | 3 | 4,275,610 |
| 2026-08-05 | `17c628d44d` | docs: bookkeeping pass over offbeat and basic-lib | 34,558 | 713 | 36,209,328 |
| 2026-08-05 | `830a3874b6` | docs: restore the astest V&V oracle, correct scoping section 7 | 45 | 25 | 9,591,507 |
| 2026-08-05 | `db9f16a3eb` | docs: correct section 7 -- comp0* is not the oracle I claimed | 33 | 10 | 11,143,584 |
| 2026-08-05 | `19ae058f02` | docs: identify ssnv101a as the entry point for astest verific... | 42 | 6 | 8,526,284 |
| 2026-08-05 | `f42b6c88df` | code_aster: first astest verification -- Chaboche reproduces ... | 416 | 0 | 16,008,252 |
| 2026-08-05 | `eedd404204` | docs: ssnv113a is unusable -- missing material include, and w... | 14 | 0 | 15,313,203 |
| 2026-08-05 | `f895dad5cd` | astest harness: mixed strain/stress control + DEFI_FONCTION/D... | 569 | 0 | 21,998,464 |
| 2026-08-05 | `00a38e4d2b` | code_aster: upstream's own tolerances corroborate the VENDOCH... | 0 | 0 | 15,246,459 |
| 2026-08-05 | `3c471b26f1` | astest: wire ssnv126a -- VENDOCHAB does NOT yet reproduce ups... | 457 | 0 | 13,409,118 |
| 2026-08-05 | `2e9a4e7b32` | astest ssnv126a: correct the diagnosis -- one root cause, not... | 23 | 11 | 10,731,988 |
| 2026-08-05 | `3a1cde97eb` | damage: fix a saturation test that fired on every step (VENDO... | 204 | 100 | 25,352,024 |
| 2026-08-05 | `efbf12301b` | astest ssnv126a: sub-step convergence study -- first order, a... | 74 | 14 | 16,508,302 |
| 2026-08-05 | `946b12e4c6` | PipeVisual: rectangles sized from real geometry, per-cell box... | 357 | 33 | 12,858,918 |
| 2026-08-05 | `da0cd23a76` | PipeVisual: white rectangle tracers crossing each run in exac... | 196 | 14 | 14,605,033 |
| 2026-08-05 | `85ea8d4da5` | pipes: single pulsed tracer, and a wall drawn from TUAS's pre... | 315 | 32 | 15,769,406 |
| 2026-08-05 | `ef18cf5b64` | aster: unified isotropic hardening curve (op-fxp, step 1 of 2) | 643 | 0 | 61,056,670 |
| 2026-08-05 | `8cd80dd48d` | CHECKPOINT: agent fleet in flight -- ODE enum wrapper and mec... | 1,631 | 28 | 9,056,674 |
| 2026-08-05 | `6e2e66f53b` | pipes: cell dividers as wall metal, and a less squat helium run | 110 | 4 | 6,854,328 |
| 2026-08-05 | `9c03a291ad` | pipes: helium run to 12 m, canvas scrolls so length stays tru... | 28 | 6 | 3,771,955 |
| 2026-08-05 | `f6e0091222` | offbeat: Zircaloy Poisson validation case (op-6sl.7) -- block... | 3,186 | 30 | 6,861,279 |
| 2026-08-05 | `d1ab6696b0` | CHECKPOINT: VISCOCHAB tests and README, agents still in flight | 101 | 59 | 6,959,251 |
| 2026-08-05 | `18712612b3` | CHECKPOINT: merge develop; viscochab.rs still under edit | 1 | 2 | 6,229,603 |
| 2026-08-05 | `8573dcb018` | offbeat/foam: VISCOCHAB, ODE enum dispatch, rheology wiring, ... | 3,681 | 1,961 | 14,965,941 |
| 2026-08-05 | `bb52a81470` | offbeat: fracture is not blocked on finite elements -- correc... | 125 | 51 | 2,308,589 |
| 2026-08-05 | `456ad77fb5` | offbeat: refuse the Zircaloy Poisson crossover (op-6sl.7); fi... | 181 | 11 | 8,835,761 |
| 2026-08-05 | `3b88deb076` | bedok: stage-1 Rust translation of Than Yan Ren's coupled nod... | 25,924 | 0 | 71,600,043 |
| 2026-08-05 | `a96e98cfc3` | tampines: 1-D drift-flux solver (op-dt3.12); six-equation lef... | 3,827 | 5 | 30,484,127 |
| 2026-08-05 | `d106ec667b` | bedok: add the crate README, with the defect register front a... | 134 | 0 | 2,076,020 |
| 2026-08-05 | `c55917b6e4` | CHECKPOINT: melt_foam solver loop in flight, agent still writing | 535 | 0 | 6,232,846 |
| 2026-08-05 | `4f0bf810d3` | bedok: record that benchmark sources are cited, not republished | 16 | 0 | 5,926,180 |
| 2026-08-05 | `08029e65ae` | CHECKPOINT: melting V&V probe scaffold, agent still writing | 368 | 1 | 3,943,751 |
| 2026-08-05 | `67a3105a0c` | multiphase: port OpenFOAM's interfacial heat-transfer closure... | 1,263 | 179 | 9,811,393 |
| 2026-08-05 | `39aaea0784` | CHECKPOINT: melting V&V cases filled in (334 -> 971 lines), a... | 72 | 2 | 5,643,822 |
| 2026-08-05 | `04618b1cce` | kovan-tui: interactive PDF ingestion, with mandatory metadata... | 3,004 | 49 | 10,407,067 |
| 2026-08-05 | `769517e5ae` | outram-foam-basic-lib: solidification/melting fvOptions (from... | 1,443 | 5 | 234,154,623 |
| 2026-08-05 | `7fa6a3de25` | outram-foam-multiphase: interfacial heat-transfer closures (f... | 500 | 0 | 0 |
| 2026-08-05 | `bec668b666` | outram-foam-appbuilder-lib: melt_foam solver and melting V&V ... | 1,556 | 0 | 0 |
| 2026-08-05 | `a9ae462ad7` | tampines: 1-D drift-flux multiphase solver (from claude/outra... | 2,383 | 0 | 0 |
| 2026-08-05 | `43d4246a18` | Cargo.lock: regenerate for the tampines multiphase dependencies | 1 | 0 | 2,285,010 |
| 2026-08-05 | `1199b8fb29` | CHECKPOINT: melt_foam tests + References.md land; merge devel... | 553 | 109 | 3,080,702 |
| 2026-08-05 | `96631482e9` | appbuilder: melt_foam solver loop + Stefan/gallium V&V cases | 8 | 6 | 5,381,722 |
| 2026-08-05 | `d686901d8d` | outram-foam-basic-lib: document TemperatureTable / Solidifica... | 24 | 17 | 16,225,640 |
| 2026-08-05 | `8f0d914f47` | outram-foam-appbuilder-lib: melt_foam solver loop + Stefan/ga... | 536 | 97 | 85,444 |
| 2026-08-05 | `2e085b2293` | outram-foam-basic-lib: fix README table split by a stray blan... | 0 | 1 | 2,737,143 |
| 2026-08-06 | `1d03089ee4` | color_maps: vendor Crameri's Scientific colour maps (MIT), fr... | 520 | 0 | 46,053,354 |
| 2026-08-06 | `9c31adc488` | components: grade temperature blue -> white -> red with Crame... | 92 | 30 | 14,950,932 |
| 2026-08-06 | `6773284c3c` | components: colour-to-temperature legend in the studio's righ... | 622 | 1 | 17,326,105 |
| 2026-08-06 | `ccad91504b` | pipes: hook the arrays up — Pipe::step implemented, PipeCompo... | 2,097 | 104 | 98,238,537 |
| 2026-08-06 | `bfd72cc091` | components: smooth pipe bends, with a live-angle demonstratio... | 1,294 | 7 | 28,326,815 |
| 2026-08-06 | `504b0b0956` | pipe bend: signed turn angle spanning -180 to +180 | 202 | 38 | 27,767,512 |
| 2026-08-06 | `ebbde1b69e` | outram-mc GPU: make the WGSL surface-distance kernel agree wi... | 546 | 76 | 11,886,857 |
| 2026-08-06 | `8ce09292c1` | docs: reactor scoping slate — six reactor types with coupled ... | 1,468 | 0 | 9,692,180 |
| 2026-08-06 | `a6f49cd633` | engine: promote the FHR reactor vessel widget into the shared... | 986 | 936 | 16,772,032 |
| 2026-08-06 | `fcf2e54114` | engine: promote the temperature buttons, delete the duplicate... | 369 | 1,190 | 19,407,929 |
| 2026-08-06 | `511cc9a440` | CLAUDE.md: complete the Members table, dogfood kopi-beans, pi... | 162 | 32 | — |
| 2026-08-06 | `ee9c7b1865` | CLAUDE.md: resolved kopitiam issues move to docs/kopitiam-iss... | 55 | 4 | 7,217,855 |
| 2026-08-06 | `d5b0b63885` | engine: reactor-vessel art for all six scoped reactors, and m... | 1,082 | 24 | 36,463,152 |
| 2026-08-06 | `500dac1ae2` | turbine: flip the stator lean so nozzle and bucket oppose eac... | 82 | 10 | 11,151,998 |
| 2026-08-06 | `be49edb9a5` | engine: the FHR archetype now renders the real fhr_sim_v2 ves... | 61 | 42 | 10,581,826 |
| 2026-08-06 | `a2f5414342` | engine: FHR vessel keeps its proportions at any size, plus pe... | 347 | 4 | 17,603,654 |
| 2026-08-06 | `9be87c63bd` | engine: HTR-10 vessel artwork from the IAEA cross-section, in... | 2,931 | 43 | 22,612,771 |
| 2026-08-06 | `880a9bd6c7` | htr-10 vessel: randomly packed pebble bed, following the cone... | 201 | 37 | 12,021,493 |
| 2026-08-06 | `10c13a642c` | pebble artwork: TRISO speckle instead of one smooth hot sphere | 371 | 13 | 25,773,647 |
| 2026-08-06 | `6f5380f4c1` | kovan: ingest the MSRE design and operations report (ORNL-TM-... | 29,522 | 0 | 3,718,293 |
| 2026-08-06 | `e25544e225` | reference-data: vendor the NRIC/INL Virtual Test Bed (CC-BY-4... | 562,796 | 0 | 3,830,761 |
| 2026-08-06 | `07a1952f58` | triso: many more kernel dots per pebble | 9 | 9 | 16,449,134 |
| 2026-08-06 | `4f0d1f741b` | triso: crank the kernel density up again, plus the RAVEN port... | 1,110 | 8 | 3,337,523 |
| 2026-08-06 | `86f85f83f6` | triso: derive the kernel count from an explicit 80% fill target | 42 | 9 | 6,794,297 |
| 2026-08-06 | `452c26b82e` | triso: dial the fill target back to 65% | 5 | 5 | 6,859,598 |
| 2026-08-06 | `77fada9a86` | triso: fill target down to 55% | 4 | 4 | 4,136,538 |
| 2026-08-06 | `8f48e587c4` | raffles: new crate — Risk Analysis Framework For Learning & E... | 1,480 | 0 | 5,546,737 |
| 2026-08-06 | `c3985f6a85` | docs: what the vendored Virtual Test Bed actually gives us | 269 | 0 | 19,966,715 |
| 2026-08-06 | `f136e9015f` | pebble bed: a real DEM-settled packing, baked once | 1,649 | 0 | 12,859,820 |
| 2026-08-06 | `43e4ec0e69` | raffles: samplers and sensitivity analysis, and a real bug fo... | 2,741 | 151 | 10,011,572 |
| 2026-08-06 | `3263ccac98` | raffles: probability distributions (op-vjw.1) | 2,792 | 57 | 13,309,646 |
| 2026-08-06 | `b42df8f1a9` | widget studio: DEM-packed beds, three steam generators, three... | 4,992 | 248 | 30,059,643 |
| 2026-08-06 | `9f4ff6d470` | rng: fix init_seed to match OpenMC (op-rbo) | 3,361 | 1,344 | 11,697,975 |
| 2026-08-06 | `131f5a93c5` | docs: fence the torus quartic derivation as text, not Rust (o... | 11 | 3 | 14,425,662 |
| 2026-08-06 | `b16067d42b` | outram-mc-libs: record the RNG goal — statistics, not particl... | 33 | 0 | 4,275,739 |
| 2026-08-06 | `dc28d98146` | pebble beds: 3-D depth instead of a flat saw-cut | 1,551 | 510 | 56,238,852 |
| 2026-08-06 | `e71f1f97fa` | rng: port OpenMC's PCG output permutation (op-jis) | 1,861 | 476 | 15,385,625 |
| 2026-08-06 | `99925ce1fe` | raffles: correct two docs the RNG changes made false | 31 | 13 | 10,999,756 |
| 2026-08-06 | `91153bb86f` | docs: fix tampines-steam-tables doctests — a stale API, not a... | 56 | 23 | 8,311,676 |
| 2026-08-06 | `097c0c7e3f` | hexlattice: record that the op-jis shift is not meaningful | 16 | 0 | 8,355,438 |
| 2026-08-07 | `8c9ddd05b7` | docs: migrate issue-tracker instructions from beads-rs to kop... | 373 | 235 | 1,037,154 |
| 2026-08-07 | `1dfcd0191c` | outram-foam-basic-lib: FV/primitive layer work, and a documen... | 4,172 | 522 | 47,249,266 |
| 2026-08-07 | `ee4ac849f3` | outram-foam-appbuilder-lib: solver-application work, and a re... | 4,244 | 296 | 0 |
| 2026-08-07 | `1f7f815afa` | outram-blender: mesh-authoring work, and a provenance contrad... | 1,980 | 117 | 0 |
| 2026-08-07 | `9bab7ad706` | outram-park-fork-cfmesh: meshing work, temp-dir handling, and... | 2,863 | 172 | 0 |
| 2026-08-07 | `3e9c3a5372` | outram-foam-mesh: mesh generation/conversion work, and a docu... | 3,941 | 260 | 0 |
| 2026-08-07 | `fa4cbd07d9` | docs: MELCOR-parity and real-time multi-fidelity scoping | 2,388 | 0 | 531,351 |
| 2026-08-07 | `07aa498aa7` | docs: complete the migration to kopi-beans; CLAUDE.md said th... | 317 | 111 | 0 |
| 2026-08-11 | `734a530759` | HTR-10 foundations: 23-group decay heat, KTA/ZBS tests, kovan... | 8,372 | 345 | 95,389,999 |
| 2026-08-11 | `89100a934a` | dwsim-libs: port the flowsheet, dynamics, column, petroleum a... | 58,347 | 7 | 51,701,732 |
| 2026-08-11 | `5a94616dff` | kovan-literature: graph digitiser (engine + CLI, TUI and egui... | 4,297 | 0 | 14,929,678 |
| 2026-08-11 | `a6b1950042` | CLAUDE.md: mandate dogfooding the kovan graph digitiser | 40 | 0 | 5,523,494 |
| 2026-08-11 | `4428148679` | kovan-literature: librarian pass over the HTR-10 archive | 498 | 2,452 | 49,931,356 |
| 2026-08-11 | `30d72d3902` | kovan-literature: visibility is closed by default (op-nv6g) | 90 | 11 | 14,496,734 |
| 2026-08-11 | `e87f4f9188` | dwsim-libs: port ShortcutColumn (Fenske-Underwood-Gilliland) | 1,847 | 0 | 3,205,638 |
| 2026-08-11 | `0857f5ea5c` | CLAUDE.md: any literature ingested OR USED goes into kovan | 31 | 1 | 11,700,922 |
| 2026-08-11 | `c5422e8d7b` | CLAUDE.md: TUAS natural-circulation tests must run in parallel | 33 | 0 | 4,632,718 |
| 2026-08-11 | `a9f71f01ef` | chem-eng: relicense to GPL-3.0, O(1) recurrences, and z-domai... | 8,695 | 2,705 | 10,082,614 |
| 2026-08-11 | `4feb6c847e` | HTR-10 neutronics spec, (p,h) taper guard, and O(1) transfer ... | 7,136 | 254 | 15,077,007 |
| 2026-08-11 | `4cdb09fe8f` | chem-eng: make the step-cost regression tests assert a ratio,... | 89 | 57 | 34,160,302 |
| 2026-08-11 | `d6edfd9f96` | tampines: correct the false Marviken claims and add a bounds-... | 6 | 0 | 7,297,727 |
| 2026-08-11 | `771cb22561` | kovan-literature: catalogue NUREG/CR-2671, the Marviken criti... | 10,131 | 0 | 42,684,750 |
| 2026-08-11 | `c86506e1b0` | tampines: pebble-bed conduction stack, helium gas layer, and ... | 9,505 | 59 | 5,490,374 |
| 2026-08-11 | `e11989a717` | tampines-steam-tables: fix the IF97 router panic and gate Mar... | 6,248 | 664 | 25,632,699 |
| 2026-08-11 | `b37e79a395` | tampines-steam-tables: make the (p,s) validity check accept t... | 195 | 53 | 41,464,956 |
| 2026-08-11 | `c615dcd66b` | kovan-literature: add the missing nrc1982marviken CATALOGUE.m... | 24 | 0 | 353,340 |
| 2026-08-11 | `e0f0568ade` | tuas: fix the transposed Reynolds/Prandtl exponents in the Wa... | 330 | 25 | 118,291 |
| 2026-08-11 | `085ceea8b7` | tampines: drift-flux plenum inflow BC, Marviken V&V, and six-... | 2,916 | 35 | 24,921,232 |
| 2026-08-11 | `67ebcd6ca5` | outram-mc: give bound nuclides their thermal-elastic channel ... | 1,187 | 48 | 7,339,144 |
| 2026-08-11 | `6ab77bb6ca` | tampines: document how OpenFOAM regularises the six-equation ... | 1,111 | 0 | 89,380,435 |
| 2026-08-11 | `b1bc988081` | outram-blender: HTR-10 pebble-bed core envelope generator + t... | 461 | 0 | 643,777,063 |
| 2026-08-11 | `e43c4c255a` | docs(agents): mark the kopi-beans store-format blocker resolv... | 25 | 11 | 42,821,454 |
| 2026-08-12 | `31471e0608` | tampines: interfacial exchange closures for the six-equation ... | 1,914 | 0 | 13,010,751 |
| 2026-08-12 | `d9e3d0316b` | feat(outram-mc): TRISO nested-shell geometry + Jodrey-Tory hi... | 907 | 0 | 30,999,088 |
| 2026-08-12 | `dfc9dcb063` | outram-foam-basic-lib: declare rayon and target-gated wgpu | 15 | 0 | 6,698,480 |
| 2026-08-12 | `2b2c7c3355` | fix(njoy): correct carbon MAT numbers + register C-nat for gr... | 48 | 8 | 12,190,260 |
| 2026-08-12 | `fbad15fd4e` | outram-foam-basic-lib: ComputeBackend dispatch layer (op-yvj.... | 3,800 | 14 | 16,966,568 |
| 2026-08-12 | `71c2d755d9` | outram-foam-basic-lib: untrack kernels committed prematurely ... | 2 | 3,149 | 3,131,077 |
| 2026-08-12 | `8141f91d53` | feat(njoy): thermal-scattering-law (tsl) acquire/cache path f... | 155 | 4 | 26,166,423 |
| 2026-08-12 | `75de417398` | Cargo.lock: pick up rayon and target-gated wgpu from the foam... | 2 | 0 | 371,274,370 |
| 2026-08-12 | `4682771149` | outram-foam-basic-lib: checkpoint the rayon LDU and field ker... | 4,636 | 0 | 10,808,157 |
| 2026-08-12 | `1ef08b8ad4` | outram-foam-basic-lib: checkpoint agent test refinements (op-... | 10 | 8 | 3,751,036 |
| 2026-08-12 | `4a22e652c3` | outram-foam-basic-lib: record the measured field-kernel cross... | 35 | 5 | 3,889,191 |
| 2026-08-12 | `121ec1124e` | outram-foam-basic-lib: bound operator-derived field names (na... | 326 | 12 | 14,220,764 |
| 2026-08-12 | `c601037602` | outram-foam-basic-lib: checkpoint LDU parallel test edits (op... | 2 | 2 | 1,994,540 |
| 2026-08-12 | `e40d6a5769` | dwsim: transient (dynamic) rigorous distillation column + V&V | 860 | 0 | 74,990,843 |
| 2026-08-12 | `57378f1749` | outram-foam-basic-lib: checkpoint LDU SpMV work in progress (... | 188 | 94 | 3,036,925 |
| 2026-08-12 | `403bd70753` | outram-foam-basic-lib: hybrid LDU SpMV and Krylov vecops (op-... | 68 | 25 | 4,864,701 |
| 2026-08-12 | `f5040fc1bc` | docs: make 19 unpublished beads durable, and file the kopi-be... | 1,152 | 0 | 17,811,843 |
| 2026-08-12 | `9da38d5df4` | outram-foam-basic-lib: batched root finding on the hybrid bac... | 3,125 | 2 | 5,850,470 |
| 2026-08-12 | `846636932b` | gitignore: aider's virtualenv | 1 | 0 | 58,046,223 |
| 2026-08-12 | `0b132ea216` | tampines: implement the 1-D six-equation two-fluid solver (op... | 4,435 | 104 | 8,878,416 |
| 2026-08-12 | `9c0a3b629c` | tampines: correct the multiphase_1d module docs now the six-e... | 15 | 5 | 2,910,845 |
| 2026-08-12 | `878c73eaa8` | opcua: extract the reactor-agnostic layer out of ciet_opcua (... | 3,229 | 1,993 | 156,958,507 |
| 2026-08-12 | `f7c8a19024` | htgr_sim_v1: real HTR-10 pebble-bed core, and a schematic bui... | 2,278 | 611 | 1,951,840 |
| 2026-08-12 | `2a6a43c733` | fhr_sim_v2: make a steam-generator temperature cross impossib... | 2,639 | 86 | 978,188 |
| 2026-08-12 | `43f8110440` | htgr: OPC-UA node map, HTR-10 plant data, and two more source... | 4,103 | 1 | 1,960,899 |
| 2026-08-12 | `df8ad9a8a5` | rustfmt.toml: pin workspace formatting | 38 | 0 | 3,934,222 |
| 2026-08-12 | `dd00f68046` | style: apply rustfmt workspace-wide (formatting only, no beha... | 214,067 | 142,186 | 8,903,673 |
| 2026-08-12 | `3cbbb5a88a` | blame: ignore the workspace rustfmt pass | 12 | 0 | 991,612 |
| 2026-08-12 | `0d272cf16f` | htgr_sim_v1: wire the real htr10 KTA/ZBS module into the plan... | 868 | 238 | 31,734,367 |
| 2026-08-12 | `cf975861e6` | fhr_sim_v2: measure GUI frame time, so lag reports can be att... | 316 | 0 | 696,152 |
| 2026-08-12 | `079bd1d475` | docs: record kopitiam#19 as resolved -- bn v0.1.3 does publis... | 115 | 0 | 0 |
| 2026-08-12 | `bf2c851a3a` | htgr_sim_v1: command control rods, not reactivity | 401 | 14 | 24,589,508 |
| 2026-08-12 | `3c9d547a9b` | docs: refresh the kopi-beans notes against v0.1.3 | 111 | 49 | 0 |
| 2026-08-12 | `1e02513568` | htgr_sim_v1: make a steam-side temperature cross unrepresentable | 209 | 10 | 20,022,517 |
| 2026-08-12 | `71d07632e7` | htgr_sim_v1: automatic scram on measurable trip signals | 487 | 8 | 29,694,384 |
| 2026-08-12 | `0bc2a948ff` | app_scaffold: name the failing PLANT COMPONENT in crash reports | 493 | 8 | 1,597,888 |
| 2026-08-12 | `e144e0d926` | scripts: make the beads-store push work from any working dire... | 16 | 2 | 9,779,205 |
| 2026-08-12 | `91853c3f0a` | htgr_sim_v1: make the protection system a toggle, disarmed by... | 77 | 2 | 5,850,719 |
| 2026-08-12 | `541a38ef33` | components: bake the pebble bed to a texture, and animate the... | 7,444 | 80 | 3,838,214 |
| 2026-08-12 | `4c36c029bc` | components: real artwork for the condenser, cooling tower and... | 78 | 36 | 5,705,402 |
| 2026-08-12 | `3543645b9e` | widget_studio: tabs for the condenser, cooling tower and excu... | 2,462 | 0 | 23,929,049 |
| 2026-08-12 | `5375e27244` | docs: htr10.md described the opposite of what the simulator n... | 36 | 7 | 24,055,398 |
| 2026-08-12 | `2e3185dd29` | components: an HTGR excursion is not an explosion — rework th... | 1,863 | 1,330 | 2,666,404 |
| 2026-08-12 | `162a08face` | coolprop: fix two boundary-condition defects in OPCPFluidArray | 604 | 11 | 3,248,926 |
| 2026-08-12 | `c0d6317979` | docs: record the human corrections to AI work, and the BC con... | 380 | 0 | 27,251,795 |
| 2026-08-12 | `d9abc55616` | tampines-steam-tables: the energy equation's inlet BC was era... | 647 | 2 | 2,972,773 |
| 2026-08-12 | `2cc65f9a22` | docs: re-verify the whole HTR-10 scoping audit against curren... | 279 | 84 | 29,015,574 |
| 2026-08-12 | `bf023a61a9` | components: real artwork for the heat exchanger, plus a studi... | 3,469 | 22 | 1,283,009 |
| 2026-08-12 | `00a776be12` | htgr_sim_v1: the turbine rotor now turns, from a real torque ... | 1,056 | 20 | 2,582,852 |
| 2026-08-12 | `621061583e` | coolprop: bounded scalar convection, upwind boundaries, and a... | 825 | 34 | 7,177,635 |
| 2026-08-12 | `6c7cee7fc7` | docs: narrow the sibling-document claim from "audited" to "sp... | 32 | 10 | 4,637,090 |
| 2026-08-12 | `b531315643` | tampines-steam-tables: TUAS upwind terminals, and the conduct... | 1,177 | 32 | 0 |
| 2026-08-12 | `9f36deb3a6` | CLAUDE.md: hard rule — search the workspace before building a... | 60 | 0 | 16,198,039 |
| 2026-08-12 | `05b20cbd17` | fhr_sim_v2: measured the GUI, found nothing to fix, kept the ... | 488 | 264 | 18,836,035 |
| 2026-08-12 | `e7e44b28b3` | docs: GUI stutter on the workstation is the graphics stack, n... | 73 | 0 | 15,884,436 |
| 2026-08-12 | `10216f03a0` | docs: record the measured GPU headroom, with its caveats | 20 | 4 | 4,404,582 |
| 2026-08-12 | `edbb2bd72e` | htgr_sim_v1: nodalised counter-flow steam generator replaces ... | 2,845 | 170 | 5,165,784 |
| 2026-08-12 | `1d4ac74735` | changed numbers and some of the hot and cold side | 14 | 14 | 2,979,170 |
| 2026-08-13 | `d55997e216` | outram-foam-basic-lib: snapshot in-flight golden-section work... | 1,387 | 0 | 19,709,337 |
| 2026-08-13 | `8a83224789` | outram-foam-basic-lib: snapshot in-flight ODE and minimisatio... | 5,315 | 41 | 5,983,182 |
| 2026-08-13 | `fb59c7ef7c` | kovan: ingest ANL-75-55 and Pichler 2020 stainless-steel prop... | 2,109 | 0 | 131,404,259 |
| 2026-08-13 | `f565d31b05` | tuas: add SolidMaterial::SteelSS304LHighTemp (Kim ANL-75-55, ... | 1,055 | 1 | 442,112 |
| 2026-08-13 | `1a38223011` | outram-foam-basic-lib: snapshot golden-section test additions... | 89 | 15 | 3,453,944 |
| 2026-08-13 | `f693939556` | tuas: give the temperature-range error a payload, and stop mi... | 353 | 90 | 443,636 |
| 2026-08-13 | `3acc95362e` | htgr_sim_v1: reach real time (0.492 -> 1.032), and fix the ho... | 3,163 | 321 | 445,294 |
| 2026-08-13 | `4320d05688` | workspace: clear all 97 float_literal_f32_fallback warnings (... | 103 | 104 | 896,609 |
| 2026-08-13 | `d54ffe72bb` | outram-foam-basic-lib: batched golden-section minimisation (o... | 16 | 0 | 12,375,581 |
| 2026-08-13 | `8e97330da2` | coolprop: add a selectable implicit energy-balance mode to OP... | 756 | 7 | 10,079,720 |
| 2026-08-13 | `ee15d32ac8` | outram-foam-basic-lib: checkpoint ODE ensembles, doctest now ... | 104 | 3 | 3,202,375 |
| 2026-08-13 | `68e35551c2` | htgr_sim_v1: put the steam generator's helium side on implici... | 26 | 0 | 8,059,634 |
| 2026-08-13 | `981e2ccf22` | outram-foam-basic-lib: checkpoint ODE ensemble rewrite (op-yv... | 247 | 130 | 5,576,906 |
| 2026-08-13 | `515777665f` | outram-foam-basic-lib: batched ODE ensembles and quadrature (... | 104 | 60 | 4,293,471 |
| 2026-08-13 | `0ba10cbe37` | outram-foam-basic-lib: snapshot in-flight numerical Jacobians... | 1,997 | 0 | 9,893,912 |
| 2026-08-13 | `c0e85a503e` | htgr_sim_v1: cut the steam-generator sub-steps 8 -> 2, reachi... | 285 | 36 | 25,575,685 |
| 2026-08-13 | `e49784bf5e` | outram-foam-basic-lib: checkpoint Jacobians and the hybrid pa... | 3,748 | 15 | 7,107,909 |
| 2026-08-13 | `f45a7d2a35` | outram-foam-basic-lib: cross-cutting hybrid parity gate (op-y... | 1 | 0 | 5,685,700 |
| 2026-08-13 | `45f79ac20d` | outram-foam-basic-lib: checkpoint Jacobians, failure-detectio... | 646 | 16 | 3,138,186 |
| 2026-08-13 | `ffebc10033` | outram-foam-basic-lib: checkpoint Jacobian test additions (op... | 118 | 56 | 4,752,711 |
| 2026-08-13 | `5e6b25d3a6` | outram-foam-basic-lib: finite differences and batched Jacobia... | 89 | 0 | 9,161,739 |
| 2026-08-13 | `b3cc5b66fb` | outram-foam: fix a solver that reported success with a NaN st... | 105 | 19 | 19,365,173 |
| 2026-08-13 | `fded5ea956` | docs: retire the beads-recovery copy -- all 19 are now published | 0 | 1,039 | 6,780,032 |
| 2026-08-13 | `3288145f97` | htgr_sim_v1: add selectable temperature-cross remedies (None ... | 5,940 | 0 | 21,455,774 |
| 2026-08-13 | `762a109405` | outram-foam-basic-lib: checkpoint Krylov-on-HybridLdu wiring ... | 1,999 | 79 | 14,434,163 |
| 2026-08-13 | `dbe4c6db90` | htgr_sim_v1: drop the eliminate-metal remedy, prefer Yan Ren'... | 108 | 1,385 | 9,353,446 |
| 2026-08-13 | `c5e5983959` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 130 | 13 | 7,065,216 |
| 2026-08-13 | `f12599484b` | outram-foam-basic-lib: salvage the Krylov agent's last edits ... | 24 | 26 | 5,310,052 |
| 2026-08-13 | `b500c1a0b8` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 107 | 38 | 3,571,022 |
| 2026-08-13 | `17872fe4e9` | outram-foam-basic-lib: checkpoint Krylov wiring (op-yvj.4.4) | 163 | 58 | 7,833,462 |
| 2026-08-13 | `c601307809` | htgr_sim_v1: secondary-loop operator controls, degC/K toggle,... | 2,427 | 155 | 32,028,698 |
| 2026-08-13 | `60fb179eff` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 482 | 106 | 12,831,072 |
| 2026-08-13 | `65345929a3` | outram-foam-basic-lib: checkpoint GMRES hybrid wiring (op-yvj... | 11 | 2 | 2,474,822 |
| 2026-08-13 | `947a904dbe` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 40 | 10 | 4,353,459 |
| 2026-08-13 | `790f686378` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 67 | 17 | 2,497,606 |
| 2026-08-13 | `f76ed92e49` | outram-foam-basic-lib: checkpoint BiCGStab hybrid wiring (op-... | 17 | 3 | 3,137,608 |
| 2026-08-13 | `d44d592b1f` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 47 | 24 | 2,519,530 |
| 2026-08-13 | `bd373cc685` | outram-foam-basic-lib: checkpoint preconditioner hybrid wirin... | 13 | 8 | 3,166,210 |
| 2026-08-13 | `d688f7d7f0` | outram-foam-basic-lib: Krylov solvers on the hybrid backend (... | 11 | 3 | 5,752,956 |
| 2026-08-13 | `2d68f5b5c4` | checkpoint: op-zwk0 NaN-defect audit of the two duplicated OD... | 226 | 2 | 18,296,809 |
| 2026-08-13 | `c1e0fd35aa` | checkpoint: op-zwk0 guard in coolprop's duplicated Rosenbrock... | 10 | 0 | 5,985,016 |
| 2026-08-13 | `ce4dd30d80` | checkpoint: op-zwk0 NaN guards in both duplicated ODE trees | 354 | 36 | 4,012,774 |
| 2026-08-13 | `9d2f1dbb05` | checkpoint: op-zwk0 NaN guards, both trees (+40/-8) | 40 | 8 | 4,718,730 |
| 2026-08-13 | `72fe7518fc` | docs: record kopi-beans losing beads AFTER acknowledging the ... | 69 | 0 | 19,245,199 |
| 2026-08-13 | `6aa58b2ee1` | checkpoint: op-uyi3 golden-section de-duplication in steam-ta... | 372 | 96 | 10,592,281 |
| 2026-08-13 | `58a3284a76` | checkpoint: op-uyi3 golden-section de-duplication, Zaloudek g... | 500 | 35 | 7,156,851 |
| 2026-08-13 | `31174d8c07` | steam-tables: de-duplicate golden section; correct an underst... | 28 | 9 | 7,962,361 |
| 2026-08-13 | `704830226d` | steam-tables: stop writing to stderr from production flash pa... | 21 | 11 | 13,148,964 |
| 2026-08-13 | `5e1c988fc1` | policy: make the working-hours guardrail opt-in per session | 87 | 27 | — |
| 2026-08-13 | `dad0581309` | docs(outram-foam-basic-lib): settle the hybrid CPU+GPU precis... | 165 | 0 | 23,362,738 |
| 2026-08-13 | `86c46ffb69` | docs(outram-foam-basic-lib): coarse-to-fine supersedes the hy... | 125 | 9 | 2,954,825 |
| 2026-08-13 | `85143f25be` | docs: upstream the three outstanding kopi-beans defects | 6 | 0 | 17,510,691 |
| 2026-08-13 | `b3c5835c3c` | kovan-metrics: port token accounting + historian from Python ... | 2,590 | 24 | 121,773,353 |
| 2026-08-13 | `859f9221fb` | docs: record the kopi-beans 0.1.6 daemon fix, verified on thi... | 95 | 6 | 6,622,357 |
| 2026-08-13 | `4ecce27a62` | docs/historian: delete the Python scripts, kovan-metrics is n... | 29 | 754 | 8,073,261 |
| 2026-08-13 | `cd0fef9d39` | kovan-literature: catalogue Terry 2005 + three UC Berkeley th... | 157 | 0 | 31,680,456 |
| 2026-08-13 | `268e44c2c1` | cfmesh: add face-pyramid-volume and cell-determinant sliver c... | 521 | 0 | 19,131,890 |
| 2026-08-13 | `89e0d47b9f` | kovan-literature: record the HTR-10 r-z zone geometry read fr... | 312 | 6 | 1,667,472 |
| 2026-08-13 | `ac271965ce` | kovan-literature: confirm z orientation, start the Fig. 2 zon... | 39 | 3 | 6,773,634 |
| 2026-08-13 | `91dd04d501` | kovan-literature: complete the Fig. 2 bottom row -- the radia... | 35 | 20 | 2,580,140 |
| 2026-08-13 | `cb3647d99b` | kovan-literature: Fig. 2 second layer -- the map stops being ... | 51 | 0 | 1,740,390 |
| 2026-08-13 | `9860ae82ee` | kovan-literature: confirm the one-band spans of zones 46, 55,... | 7 | 7 | 3,951,994 |
| 2026-08-13 | `97c4940815` | docs: reconcile upstream kopitiam issue state, and close #19 | 44 | 9 | 50,865,877 |
| 2026-08-13 | `ff0c4b372a` | njoy: LEAPR can regenerate graphite S(alpha,beta) from its 12... | 4,530 | 49 | 19,783,743 |
| 2026-08-13 | `53fa574eba` | njoy: LEAPR regeneration is now the default source of graphit... | 3,585 | 81 | 34,932,070 |
| 2026-08-13 | `239182a220` | docs(data-acquisition): the cache substrate is no longer feat... | 15 | 0 | 5,573,065 |
| 2026-08-13 | `8a8340cd7d` | docs: log requested htgr_sim_v1 follow-ups for the next activ... | 63 | 0 | — |
| 2026-08-13 | `47476d1308` | outram-foam-mesh: cyclic-aware snapping — periodic seams snap... | 900 | 3 | — |
| 2026-08-13 | `80e6e68f34` | docs(outram-foam-basic-lib): fix 24 broken intra-doc links, d... | 56 | 41 | — |
| 2026-08-13 | `0e5e14eaba` | htgr_sim_v1: decay heat, a fuel-feedback heat sink, evaluated... | 1,284 | 149 | — |
| 2026-08-13 | `e97850f320` | docs: drop the htgr_sim_v1 follow-up log, now that the work i... | 0 | 63 | — |
| 2026-08-13 | `6a15c02b57` | htgr_sim_v1: fix the core-outlet temperature inversion, and m... | 618 | 299 | — |
| 2026-08-13 | `b662380759` | coolprop: exact backward T(rho, h), and record where human re... | 304 | 0 | — |
| 2026-08-13 | `322a64041e` | coolprop: seed the (rho, h) backward solve from the fixed-pre... | 193 | 13 | — |
| 2026-08-14 | `52a21fbaa0` | dealt with degC button and the restart button | 928 | 167 | — |
| 2026-08-14 | `e91125815a` | added bacon toml | 8 | 0 | — |
| 2026-08-14 | `d4387b59a2` | legend unit is now Celsius, not kelvin | 10 | 5 | — |
| 2026-08-14 | `fdc047185f` | htgr_sim_v1: the corrector loop is not converged, and say so ... | 111 | 7 | 64,241,387 |
| 2026-08-14 | `b0fafebefc` | kovan agent-docs-gen: bundle the API docs for an external 200... | 1,631 | 0 | 28,107,942 |
| 2026-08-14 | `20e118b4ba` | agent-docs-gen: --regenerate-missing never worked; the tools ... | 1,926 | 29 | 11,922,811 |
| 2026-08-14 | `070d3f317e` | Retire scripts/gen_api_docs.py: port it to `kovan api-docs` | 327 | 181 | 9,562,759 |
| 2026-08-14 | `b9c72f50af` | No Python for docs or accounting (hard rule); retire the aste... | 78 | 472 | 11,101,988 |
| 2026-08-14 | `e6cee0ba57` | kloc parity baseline: freeze the Python output before porting it | 321 | 0 | 3,080,340 |
| 2026-08-14 | `364b8b00ff` | kloc parity baseline: recapture with every repository present | 94 | 31 | 14,795,866 |
| 2026-08-14 | `7f342453e7` | changed loop to manual | 9 | 2 | 1,920,766 |
| 2026-08-14 | `bfb645fe30` | added manual for secondary loop | 1 | 1 | 3,437,244 |
| 2026-08-14 | `5b02a9d4f9` | added feedwater manual control and edited slider | 4 | 3 | 9,297,662 |
| 2026-08-14 | `68e2e11f3e` | Retire scripts/kloc_accounting.py: port it to `kovan kloc` | 0 | 1,454 | 33,752,515 |
| 2026-08-14 | `8d996dcafa` | added kovan metrics | 3,892 | 2 | 5,563,359 |
| 2026-08-14 | `673399fac1` | kovan: regenerate the whole doc suite in one command, and fin... | 78,580 | 17,850 | 25,407,865 |
| 2026-08-14 | `7a9d2b56ee` | added new api docs for each | 142,212 | 0 | 4,171,800 |
| 2026-08-14 | `db242bb2f0` | added new api | 149,800 | 0 | 0 |
| 2026-08-14 | `dd2682d98e` | added anders pdf | 9 | 0 | 0 |
| 2026-08-14 | `b983f83870` | README: document the `kovan lit` ingestion workflow and the t... | 54 | 1 | 13,486,507 |
| 2026-08-14 | `b297fea9b5` | kovan-literature: record the choo2023criticality tier decisio... | 13 | 2 | 5,209,786 |
| 2026-08-14 | `ffef0fb76f` | added 4 endf files specific to triso | 439,743 | 0 | 1,309,908 |
| 2026-08-14 | `f90b23506e` | changed pathbuf for njoy, pending test | 10 | 3 | 0 |
| 2026-08-14 | `1ace0acd4d` | added leapr files from endf, from here: https://www.nndc.bnl.... | 10,584 | 0 | — |
| 2026-08-14 | `e4afd51779` | njoy-outram-park-fork: embed all 33 ENDF/B-VIII.0 LEAPR decks... | 894 | 132 | 0 |
| 2026-08-14 | `933ee9ca7a` | outram-mc: route thermal scattering into both k-eigenvalue pa... | 462 | 106 | — |
| 2026-08-14 | `2becc4c978` | outram-mc: TRISO shell S(alpha,beta) composition, and the SiC... | 447 | 18 | — |
| 2026-08-14 | `9591944fb9` | njoy+outram-mc: light-water thermal scattering, deck to trans... | 1,056 | 43 | 753,784,864 |
| 2026-08-14 | `a2802a350b` | pincell: the thermal LWR benchmark is no longer data-gated | 18 | 17 | 84,498,218 |
| 2026-08-15 | `5673aaa253` | Patch-bump the njoy→digital-twin-engine publish pipeline (16 ... | 1,396 | 48 | 16,094,533 |
| 2026-08-15 | `df6f62f1be` | scripts: update cargo-publish.sh to the real njoy->digital-tw... | 38 | 11 | 4,989,786 |
| 2026-08-15 | `e556dcacab` | docs(historian): add develop-vs-main accounting report (998 c... | 1,062 | 0 | 6,075,285 |
| 2026-08-15 | `796c6af11e` | docs(claude.md): trim duplicated kopitiam/kopi-beans history,... | 57 | 176 | 12,784,522 |
| 2026-08-15 | `9229031d62` | dwsim-libs: promote benzene()/toluene() to public Component c... | 65 | 43 | 52,367,515 |
| 2026-08-15 | `a621a4fcd2` | digital-twin-engine: add distillation_sim_v1, a transient dis... | 1,465 | 0 | 88,264,483 |
| 2026-08-16 | `d214fa1079` | htgr_sim_v1: add selectable pebble-bed reactor-model fidelity... | 592 | 12 | 72,021,385 |
| 2026-08-17 | `419ee643d5` | htgr_sim_v1: wire outram-foam-appbuilder-lib (genfoam) in as ... | 50 | 17 | 8,980,750 |
| 2026-08-17 | `ed6af73607` | added CinSiC endf file | 95,305 | 0 | — |
| 2026-08-17 | `cdbea87c9e` | added si in sic endf | 95,355 | 0 | — |
| 2026-08-17 | `9281c422cb` | njoy: move ENDF tapes out of crates/ so cargo package cannot ... | 443 | 97 | 39,831,920 |
| 2026-08-17 | `605d7a7383` | njoy: keep the two SiC tapes out of src/leapr/decks/, and rec... | 32 | 95,306 | 6,429,131 |
| 2026-08-17 | `e407ae124d` | kovan api-docs: rename the per-crate mirror to <crate>-api.md... | 1,496 | 56,644 | 44,691,925 |
| 2026-08-17 | `fbfa6fc3c0` | re-dimensioned one node (human manual commit) | 21 | 29 | — |
| 2026-08-17 | `8eb3f7281b` | added heat exchanger NTU theory | 129 | 0 | — |
| 2026-08-17 | `104a8ff574` | htgr_sim_v1: fix three test call sites broken by the one_node... | 18 | 4 | 35,274,176 |
| 2026-08-17 | `1d3148108e` | corrected unit bug in outram park digital twin engine for htg... | 35 | 14 | — |
| 2026-08-17 | `1abb02a492` | added one node fluid | 4 | 18 | — |
| 2026-08-17 | `44a2daca57` | htgr_sim_v1: reapply the NTU dimensional fix, reverted by the... | 18 | 4 | 28,506,899 |
| 2026-08-17 | `6f163f9c8d` | redone claude md | 123 | 78 | 6,520,781 |
| 2026-08-17 | `dfea946eb7` | bedok: rewrite the MATLAB port as one flat module per .m file | 14,486 | 25,422 | — |
| 2026-08-17 | `52075bd0c6` | docs: remove superseded bedok scoping docs | 0 | 475 | 2,910,015 |
| 2026-08-17 | `dd7de27bd2` | bedok is now excluseively Yan ren's project | 1 | 1 | 5,423,020 |
| 2026-08-17 | `a72517da72` | htgr_sim_v1: hardcode the HTR-10 R-Z benchmark zone geometry ... | 448 | 0 | 9,626,640 |
| 2026-08-17 | `efebbd527e` | htr10_rz_geometry: split volume 4 into 82/30 at z=114.7 (main... | 36 | 13 | 4,984,536 |
| 2026-08-17 | `0c5a49d043` | htgr_sim_v1: helium gas volumes from the R-Z geometry, wire b... | 256 | 7 | 13,326,320 |
| 2026-08-17 | `b8714b1b27` | htgr_sim_v1: fix tests broken by the feedwater default AUTO -... | 87 | 55 | 24,482,479 |
| 2026-08-17 | `6c3aa9ce08` | added scaffold of single node implict matrix | 269 | 0 | 4,032,848 |
| 2026-08-17 | `f504446309` | htgr_sim_v1: implement PebbleBedPorousMediaNode's implicit 2x... | 329 | 83 | 26,676,690 |
| 2026-08-17 | `1003251fe5` | htgr_sim_v1: wire PebbleBedPorousMediaNode in as a new Reacto... | 109 | 9 | 10,314,513 |
| 2026-08-17 | `5fc597f2c2` | htgr_sim_v1: OneNodePorousMedia is now default; delete old On... | 321 | 809 | 224,118,817 |
| 2026-08-17 | `c1ef05b10e` | htgr sim v1 fixed, and added fast forward | 1,121 | 11 | 0 |
| 2026-08-18 | `e8843121d4` | htgr_sim_v1: add an HTR-10 Geometry tab drawing the R-Z bench... | 399 | 18 | 43,648,082 |
| 2026-08-18 | `f7bae9e645` | app_scaffold: add csv_display -- one copyable CSV box, "Copy ... | 173 | 0 | 26,075,888 |
| 2026-08-18 | `f8cc108d18` | htr10_rz_geometry: move volumes 82/30 under 29, restore volum... | 47 | 37 | 19,286,052 |
| 2026-08-18 | `48f5ce43cb` | htr10_rz_geometry: 29 shrinks, not 31 -- correct the 82/30 pl... | 41 | 41 | 11,820,426 |
| 2026-08-18 | `9bd5b30eac` | htr10_rz_geometry/geometry_tab: TopReflector material + schem... | 103 | 5 | 36,512,974 |
| 2026-08-18 | `f0a9da662f` | geometry_tab: fixed pixel scale (not fit-to-viewport), port v... | 42 | 18 | 10,471,723 |
| 2026-08-18 | `0e863e39e2` | htgr_sim_v1: CSV side panel for Time-History Plots, toggled b... | 185 | 12 | 33,794,819 |
| 2026-08-18 | `70afac2d35` | app_scaffold: CsvSnapshotPanel -- snapshot-on-click, interval... | 350 | 49 | 36,745,149 |
| 2026-08-18 | `c8a955fe2d` | added csv logging too | 11 | 18 | 1,994,836 |
| 2026-08-18 | `92226a8f48` | csv_display: MAX_CSV_ROWS 1000 -> 4000 (maintainer), measure ... | 72 | 8 | 17,476,871 |
| 2026-08-18 | `c92a051d87` | geometry_tab: zoom in/out/reset buttons; schematic: note the ... | 185 | 24 | 57,106,865 |
| 2026-08-18 | `dd85f4008a` | htr10_rz_geometry: volumes 17/46/55/64/72 confirmed Boronated... | 12 | 4 | 21,672,515 |
| 2026-08-18 | `60878e8a4f` | geometry_tab: draw same-volume zones as one continuous region... | 156 | 79 | 40,800,824 |
| 2026-08-18 | `6328543fe8` | bedok: complete the file-by-file MATLAB translation (all 50 f... | 23,289 | 17,117 | — |
| 2026-08-18 | `11f74c4e02` | CLAUDE.md: add 170-char file-path-length hard rule (Windows M... | 33 | 0 | 49,296,940 |
| 2026-08-18 | `087836707d` | tampines-steam-tables, tuas_boussinesq_solver: fix 53 file-pa... | 228 | 162 | 87,041,892 |
| 2026-08-18 | `e8d2ff215b` | tuas_boussinesq_solver: migrate all downstream references to ... | 615 | 637 | 26,537,545 |
| 2026-08-19 | `cf342e3fd5` | Ingest Zhu (2014) SiC thermal-scattering thesis into kovan-li... | 2,853 | 5 | — |
| 2026-08-20 | `8933cdbfe5` | LEAPR: implement generalized coherent-elastic formulation, cl... | 1,225 | 32 | — |
| 2026-08-20 | `672077538c` | tampines-steam-tables: interactive T-p/p-h/T-s/h-s steam-tabl... | 10,782 | 1 | — |
| 2026-08-20 | `e11560dd9b` | tampines-steam-tables: fix x_ph_flash near-critical Region-3 ... | 30 | 2 | — |
| 2026-08-20 | `5e00e0e0d5` | tampines-steam-tables: rename to tampines-steam-tables-gui, t... | 2,597 | 548 | 0 |
| 2026-08-21 | `1a904fc59a` | njoy LEAPR: select the temperature block that belongs to the ... | 336 | 8 | — |
| 2026-08-21 | `5a91ecf728` | njoy LEAPR: opt-in CachePolicy for regeneration, per maintain... | 282 | 41 | — |
| 2026-08-21 | `7634ae96e7` | tampines-steam-tables: TabulatedData library type + custom qu... | 3,657 | 3,153 | — |
| 2026-08-21 | `33cc638a67` | tampines-steam-tables GUI: replace all-or-nothing isobar/isot... | 297 | 41 | — |
| 2026-08-21 | `832898c509` | bedok: verify the port against the running MATLAB, and fix de... | 3,170 | 49 | — |
| 2026-08-21 | `fa83715ef1` | njoy: root-cause the SiC coherent-elastic residual to the pub... | 892 | 122 | — |
| 2026-08-21 | `12616d8a67` | tampines-steam-tables-gui: top tab bar (Graph/Evaluation/Cita... | 1,248 | 203 | 128,339,892 |
| 2026-08-21 | `caefcd7d5a` | tampines-steam-tables-gui: fix tabulated single-phase filter ... | 257 | 24 | 42,997,357 |
| 2026-08-21 | `4779f16524` | tampines-steam-tables-gui: full tabulated isobar/isotherm val... | 467 | 230 | 68,328,627 |
| 2026-08-21 | `73f9d1b25c` | added compile examples for windows | 42 | 0 | 9,228,475 |
| 2026-08-21 | `cdf584901f` | WORK IN PROGRESS due to token limit: consolidate kovan gui,tu... | 10,043 | 750 | 0 |
| 2026-08-21 | `499922ea28` | kovan: finish crate consolidation, remove old crates, fix sta... | 36 | 12,495 | 0 |
| 2026-08-21 | `e51431a177` | njoy: implement Zhu's exact per-atom-type Debye-Waller option... | 715 | 23 | — |
| 2026-08-21 | `b083cea759` | kovan: relicense to AGPL-3.0-only, add kopitiam-pdf as a crat... | 814 | 2 | — |
| 2026-08-21 | `b38cd88b5c` | kovan: relocate the graph digitiser from kovan-literature, de... | 268 | 4,421 | — |
| 2026-08-21 | `e1802fd187` | kovan: restructure to 3 binaries (kovan/kovan-cli/kovan-tui) ... | 1,885 | 1,336 | — |
| 2026-08-21 | `fe240134d4` | kovan: PDF rendering engine decision, Gruvbox theming, file p... | 1,370 | 765 | — |
| 2026-08-22 | `454826d15e` | githooks: point kovan-bin.sh at kovan-cli, not the GUI binary... | 678 | 144 | 0 |
| 2026-08-22 | `f3c0fa017c` | kovan-cli: token savings — cost/outline/slice/skill-gen (GitH... | 515 | 4 | 0 |
| 2026-08-22 | `7eafb645c0` | kovan-cli: def/sig/refs -- rust-analyzer-backed semantic quer... | 537 | 10 | 0 |
| 2026-08-22 | `a9f1b6ea1a` | kovan-cli: keep-warm rust-analyzer daemon for def/sig/refs (o... | 583 | 142 | 0 |
| 2026-08-23 | `ff4bdb022b` | bedok: open the correction stage — every High-severity defect... | 3,470 | 320 | — |
| 2026-08-23 | `b970b2409e` | kovan-gui: fix silent Auto-trace failure, decide OCR engine, ... | 1,882 | 62 | — |
| 2026-08-23 | `a1aad7c4aa` | kovan-gui: table digitiser (OCR) + BibTeX parser closing the ... | 12,095 | 49 | — |
| 2026-08-23 | `68b5e4d473` | bedok: close the defect register — 79 of 79 entries addressed | 2,760 | 130 | — |
| 2026-08-24 | `ebf85ba32b` | kovan-gui: unified box interactions, annotation-to-markdown, ... | 1,735 | 323 | — |
| 2026-08-24 | `b2962e7ac2` | kovan-gui: parallelogram calibration as an enum sibling of re... | 1,490 | 194 | — |
| 2026-08-25 | `ba9598913f` | kovan-gui: upgrade kopitiam-pdf (Type1 glyph fix), PDF reader... | 299 | 18 | — |
| 2026-08-25 | `cf67ecdc42` | kovan: crates.io publish prep for batch 1 (common/discovery/c... | 4,071 | 15 | — |
| 2026-08-25 | `df966e4f9d` | kovan: bump kopitiam-pdf 0.2.6 -> 0.2.7 (mandatory hayro fall... | 254 | 9 | 0 |
| 2026-08-25 | `8975b176d3` | kovan-gui: PDF reader renders embedded /Annots (Okular highli... | 726 | 0 | 0 |
| 2026-08-25 | `0ea169c3b0` | kovan-gui: annotation-visibility diagnostics; root-caused to ... | 454 | 0 | — |
| 2026-08-31 | `b7a9f5671b` | tampines-steam-tables: experimental Chebyshev backward correl... | 4,066 | 2 | — |
| 2026-08-31 | `b565c17ea4` | tampines-steam-tables: close the Region 4 p(rho,h) test gap; ... | 287 | 24 | — |
| 2026-08-31 | `017f6ba19d` | tampines-steam-tables: diagnostics now emit markdown V&V reports | 626 | 18 | — |
| 2026-08-31 | `e7e4faaf80` | tampines-steam-tables: commit the generated V&V reports; keep... | 247 | 25 | — |
| 2026-08-31 | `5e504e3f97` | moved kovan to use v0.3.1 of kopitiam pdf | 733 | 93 | — |
| 2026-08-31 | `7e766a7dcd` | kovan: keep/adapt/replace map for the issue #35 wiki redesign | 117 | 0 | — |
| 2026-08-31 | `080bffad12` | kovan: correct the migration map -- the BibTeX parser already... | 30 | 9 | — |
| 2026-08-31 | `c09c1e904f` | kovan: fix the library's Android build -- drop kopitiam-pdf d... | 1 | 1 | 36,381,783 |
| 2026-08-31 | `8a73906651` | kovan: KovanRoot -- the Git-backed library root abstraction (... | 613 | 0 | 0 |
| 2026-08-31 | `a708a1732c` | kovan: create a Kovan library -- skeleton, .gitignore, git in... | 291 | 0 | 6,319,181 |
| 2026-08-31 | `884fc139dd` | kovan: papers, topics and projects as entities (issue #35 ste... | 874 | 0 | 3,764,895 |
| 2026-08-31 | `3c4ccdd992` | kovan: fenced-TOML artifact parser (issue #35 step 12) | 910 | 0 | 5,459,296 |
| 2026-08-31 | `c38e057a8b` | Cargo.lock: dedupe eframe/egui/wgpu/ab_glyph/bit-set to singl... | 74 | 709 | — |
| 2026-08-31 | `ba3bea65e2` | kovan redesign: root/wiki startup + repository index + PDF in... | 1,298 | 4 | — |
| 2026-09-01 | `a51219fafa` | kovan redesign: PaperSession, ResearchRecordIndex, fine-grain... | 2,116 | 43 | — |
| 2026-09-01 | `798f397403` | kovan redesign: citation/wiki autocomplete, PDF<->Markdown Sy... | 1,482 | 6 | — |
| 2026-09-01 | `a67650422a` | Bump egui/eframe/wgpu/naga to 0.36/30.0.1 workspace-wide (op-... | 361 | 302 | — |
| 2026-09-01 | `1b88834cc6` | kovan: integrate kopitiam-pdf's reusable reader (op-9vo6.11) | 312 | 1,698 | — |
| 2026-09-01 | `e7759d5a2b` | kovan: unify Kovan root + active-paper context (op-sr4n.1/.2/... | 297 | 16 | — |
| 2026-09-01 | `91cb7594a8` | kovan: fix kvim drag bug, Wiki right-click sort, retire Markd... | 370 | 37 | — |
| 2026-09-01 | `aa111fd9fb` | kovan: wire the Save Repository backend to a button (op-nswf) | 35 | 3 | — |
| 2026-09-01 | `8b9cffe5e2` | kovan: migrate bibliography.rs onto KovanRoot, retire markdow... | 125 | 459 | — |
| 2026-09-01 | `b790e02df1` | kovan: PDF Reader page-context panel as a real artifact list ... | 158 | 31 | — |
| 2026-09-01 | `944e73ab72` | kovan: fix disappearing-paper bug, PDF mode page-state, Annot... | 219 | 0 | — |
| 2026-09-01 | `f8f58d0993` | kovan: shared knowledge state, Bibliography row merge, Save R... | 355 | 164 | — |
| 2026-09-01 | `cbb8c7b59e` | kovan: wire egui_graphs into the Mindmap, fix its Android-por... | 285 | 107 | — |
| 2026-09-01 | `cebe79e0c9` | kovan: move app shell from digitiser::gui::desktop to crate::... | 52 | 15 | — |
| 2026-09-01 | `b64a250adc` | kovan-semantics: add a typed scientific-concept ontology laye... | 3,809 | 146 | — |
| 2026-09-02 | `2c7cfae86f` | kovan: decouple document access rights from git storage mode ... | 131 | 2 | — |
| 2026-09-02 | `283b5b5692` | kovan: configure a private literature submodule in kovan_root... | 196 | 20 | — |
| 2026-09-02 | `c28afc6c33` | kovan: submodule-aware Save Repository sequencing (op-3gxp) | 335 | 42 | — |
| 2026-09-02 | `1a5b557ec6` | kovan: report an unavailable source PDF instead of a silent f... | 164 | 5 | — |
| 2026-09-02 | `4a74851dcc` | kovan: digitiser UI dogfooding feedback (op-g864) | 559 | 88 | — |
| 2026-09-02 | `e55f53d8a9` | kovan: page-context panel dogfooding — read-only preview, dou... | 1,192 | 189 | — |
| 2026-09-02 | `d18a33978d` | kovan: PDF-reader annotation dogfooding fixes (op-9vo6) | 166 | 82 | — |
| 2026-09-02 | `cb9a2881fb` | docs: record kopitiam-pdf #106 / #107 in the kopitiam-issues ... | 2 | 0 | — |
| 2026-09-02 | `817019f622` | kovan: continuous multi-page canvas with always-visible, clic... | 484 | 186 | — |
| 2026-09-02 | `fe4237b778` | kovan: a focused kvim editor consumes its keystrokes (op-9vo6) | 20 | 6 | — |
| 2026-09-02 | `ecf8da7631` | kovan: drop the embedded reader, one continuous canvas with i... | 273 | 245 | — |
| 2026-09-02 | `e70987754b` | kovan: double-click a box to edit; keep the edit panel from v... | 22 | 14 | — |
| 2026-09-02 | `a58479e21a` | kovan: hold the page in view across a zoom change (op-9vo6) | 23 | 0 | — |
| 2026-09-02 | `f6acf420a7` | kovan: re-render pages at higher DPI on zoom; keep the viewpo... | 74 | 35 | — |
| 2026-09-02 | `6a1da4bf51` | kovan: pointer-anchored zoom, and stop the mouse re-pointing ... | 136 | 32 | — |
| 2026-09-02 | `14c771cda3` | kovan: restore the page-thumbnail strip; Ctrl+D/U page step; ... | 200 | 1 | — |
| 2026-09-02 | `3e073e5128` | kovan: page jumps set the scroll offset outright, so a view s... | 16 | 9 | — |
| 2026-09-02 | `cc4c7a9466` | kovan: Copy button on each bibliography entry (op-9vo6) | 16 | 0 | — |
| 2026-09-02 | `6327404144` | kovan: digitiser export UX, page-ordered notes, autocomplete ... | 334 | 71 | — |
| 2026-09-02 | `8145709aae` | kovan-gui: PDF reader — single-click block edit, Pan tool, dr... | 108 | 24 | — |
| 2026-09-02 | `96978974c0` | outram-blender: mesh selection + topology adjacency (op-hzs.5... | 1,929 | 0 | — |
| 2026-09-02 | `d21246ea63` | outram-blender: selection — similar/grow/shrink + by-trait (o... | 630 | 4 | — |
| 2026-09-02 | `87fa933a57` | outram-blender: loop cut + knife (op-hzs.54.5, .6) | 677 | 0 | — |
| 2026-09-02 | `71966b0667` | outram-blender: edge/vertex slide + N-cut subdivide (op-hzs.5... | 772 | 0 | — |
| 2026-09-02 | `15133949fc` | outram-blender: bevel parity + extrude family (op-hzs.54.9, .10) | 637 | 0 | — |
| 2026-09-03 | `682ac3377d` | outram-blender: merge modes + rip/split/separate (op-hzs.54.1... | 583 | 0 | — |
| 2026-09-03 | `106fabc746` | outram-blender: dissolve + delete matrix (op-hzs.54.13) | 364 | 0 | — |
| 2026-09-03 | `c0822922d6` | outram-blender: bridge edge loops + fill operators (op-hzs.54... | 548 | 0 | — |
| 2026-09-03 | `4974b087bb` | outram-blender: connect / poke+quads / edge tools (op-hzs.54.... | 782 | 0 | — |
| 2026-09-03 | `97bf315238` | outram-blender: transform toolset + proportional editing (op-... | 655 | 0 | — |
| 2026-09-03 | `00dac97b82` | outram-blender: mesh symmetry + spin/screw (op-hzs.54.21, .22) | 498 | 0 | — |
| 2026-09-03 | `cc190abfb9` | outram-blender: transform-input model + snapping engine (op-h... | 874 | 0 | — |
| 2026-09-03 | `7c6b992cf7` | outram-blender: 3D cursor/pivots + measurement & inspection (... | 916 | 0 | — |
| 2026-09-03 | `a31076dcac` | outram-blender: normals toolset + attribute layer (op-hzs.54.... | 689 | 19 | — |
| 2026-09-03 | `a0b70a999e` | outram-blender: generate modifiers pt.1 (op-hzs.54.29) | 186 | 3 | — |
| 2026-09-03 | `61d59bdace` | outram-blender: voxel remesh + generate modifiers pt.2 (op-hz... | 454 | 0 | — |
| 2026-09-03 | `b9a695a88a` | outram-blender: deform modifiers pt.1 (op-hzs.54.31) | 465 | 0 | — |
| 2026-09-03 | `3af0aeec18` | outram-blender: deform modifiers pt.2 + stack semantics (op-h... | 709 | 12 | — |
| 2026-09-03 | `35fbc1182d` | outram-blender: curve authoring + curve->surface (op-hzs.54.3... | 950 | 0 | — |
| 2026-09-03 | `eedbced766` | outram-blender: curve<->mesh + NURBS surfaces (op-hzs.54.36, ... | 626 | 0 | — |
| 2026-09-03 | `94a1562aaa` | outram-blender: text->geometry + extra mesh primitives (op-hz... | 897 | 0 | — |
| 2026-09-03 | `27282806fd` | outram-blender: extra-objects generators + interactive draw t... | 930 | 1 | — |
| 2026-09-03 | `b1b957f295` | outram-blender: LoopTools + Snap Utilities Line (op-hzs.54.42... | 891 | 0 | — |
| 2026-09-03 | `1bd630f866` | outram-blender: Precision Drawing Tools + Bool Tool (op-hzs.5... | 708 | 0 | — |
| 2026-09-03 | `8ad3852145` | outram-blender: fill/mirror helpers + object-mode CAD ops (op... | 758 | 0 | — |
| 2026-09-03 | `9c396a4908` | outram-blender: array patterns — radial / object-offset / alo... | 362 | 0 | — |
| 2026-09-03 | `da75c3d698` | outram-blender 0.0.3 -> 0.0.4, tampines-steam-tables 0.2.6 ->... | 11 | 11 | — |
| 2026-09-03 | `7da0533098` | docs: working-hours guardrail — drop the mandatory per-sessio... | 37 | 34 | — |
| 2026-09-03 | `4d7bdf9f79` | outram-foam: lift the CLI case-driving glue into a reusable l... | 733 | 295 | — |
| 2026-09-03 | `0270f65ba0` | outram-foam-basic-lib: real WGSL kernels for the LDU sparse p... | 1,738 | 8 | — |
| 2026-09-03 | `941bb07f9d` | outram-foam-basic-lib: WGSL kernels for closed-form polynomia... | 447 | 0 | — |
| 2026-09-03 | `2c3c1ac641` | outram-foam-basic-lib: data-level CPU/GPU co-execution planne... | 339 | 0 | — |
| 2026-09-04 | `56fba8fc00` | workspace: wasm32-unknown-unknown support — 20/36 -> 27/36 cr... | 522 | 56 | — |
| 2026-09-04 | `58d85e96fc` | workspace: wasm32-unknown-unknown gate — 30/30 in-scope crate... | 324 | 6 | — |
| 2026-09-04 | `5fb885571d` | workspace: wasm RUNTIME smoke tests — Edwards, helium SG, and... | 787 | 8 | — |
| 2026-09-04 | `4039464502` | dwsim: crude distillation from a black-oil characterisation (... | 997 | 0 | — |
| 2026-09-04 | `a54fcee57d` | dwsim: record the headless/GUI boundary for the crude sim (op... | 7 | 0 | — |
| 2026-09-04 | `8b6a12b935` | dwsim: the crude column now converges to a physical CDU profi... | 190 | 83 | — |
| 2026-09-04 | `1694384552` | dwsim: wire PR78 into CubicEos and PropertyPackageModel (op-1... | 157 | 6 | — |
| 2026-09-04 | `44a2001339` | crude epic op-190j: fix the cubic-package solve, add the tran... | 1,215 | 126 | — |
| 2026-09-04 | `5cab9ef750` | mc + njoy: the derives and the concrete seam the Python bindi... | 80 | 3 | — |
| 2026-09-04 | `aad7946ee3` | style: workspace-wide `cargo fmt --all` | 7,222 | 2,654 | — |
| 2026-09-04 | `fb94deefe7` | dwsim: re-measure the crude column on PR78 — my recorded valu... | 50 | 17 | — |
| 2026-09-04 | `1e8ddb7464` | CLAUDE.md: dogfood the Python API on a small model, as a hard... | 60 | 0 | — |
| 2026-09-04 | `6b49dcac0f` | CLAUDE.md: "if it's too complex for Haiku, it's a bad API", e... | 18 | 0 | — |
| 2026-09-04 | `5b7f9814e4` | CLAUDE.md: point the trait-diagnostics rule at its own bead | 10 | 1 | — |
| 2026-09-04 | `4e5273e026` | tampines: record real-time-via-surrogate as the design intent... | 44 | 0 | — |
| 2026-09-05 | `4e802927eb` | Expose the local-ENDF seam, and make the prelude the taught path | 608 | 5 | — |
| 2026-09-05 | `00dbf66c30` | Gate the API-dogfooding rule behind a per-crate maturity decl... | 67 | 2 | — |
| 2026-09-05 | `7d4a70533c` | Declare eight crates mature, and fix the preludes that declar... | 373 | 0 | — |
| 2026-09-05 | `e862428900` | Verify every declared crate's test count, and correct the tam... | 56 | 22 | — |
| 2026-09-05 | `50f0d10bdf` | tampines: mark the Peng-Robinson EOS gap out of scope | 5 | 3 | — |
| 2026-09-05 | `d1083280b5` | teh-o-prke: tutorial example for a reactivity insertion trans... | 283 | 0 | — |
| 2026-09-05 | `d065d19139` | outram-mc-libs: tally tutorial, and the API docs its friction... | 887 | 1 | — |
| 2026-09-05 | `cbb29b65c8` | njoy: resonance/Doppler tutorial, corrected physics, prelude ... | 273 | 116 | — |
| 2026-09-05 | `83b2b09aa8` | tampines: Rankine cycle tutorial, and a prelude that exports ... | 418 | 0 | — |
| 2026-09-05 | `670bab9e71` | outram-foam-basic-lib: FV verification tutorial, and how to g... | 254 | 0 | — |
| 2026-09-05 | `fdb89e5196` | outram-foam-appbuilder: solver tutorial, and stop advertising... | 501 | 1 | — |
| 2026-09-05 | `7ecbcacbd8` | Bump six declared crates by one patch version for release | 286 | 18 | — |
| 2026-09-05 | `ada31a985e` | teh-o-prke: decay heat tutorial, and short aliases for the st... | 60 | 1 | — |
| 2026-09-06 | `979ce0e523` | docs: HTGR Sim v1 execution architecture (native + browser pa... | 346 | 0 | — |
| 2026-09-06 | `c4ed475940` | docs: sequencing decided — architecture before the HTR-10 phy... | 45 | 11 | — |
| 2026-09-06 | `a1eca4b74f` | docs: the worker pool is unconditional — the runtime is the d... | 51 | 3 | — |
| 2026-09-06 | `8799682ed7` | Declare outram-park-fork-liggghts mature, with the physics ca... | 71 | 1 | — |
| 2026-09-06 | `2f2494c87f` | docs: three execution classes — the kernel contract does not ... | 64 | 0 | — |
| 2026-09-06 | `fb70a13cba` | HARD RULE: every egui simulator ships a headless mode, and ht... | 253 | 0 | — |
| 2026-09-06 | `fdf825767c` | docs+test: dataflow audit and the pre-refactor reference base... | 325 | 2 | — |
| 2026-09-06 | `7b26c9a19a` | Extract the kernel boundary: PlantRuntime, controls in, bound... | 343 | 7 | — |
| 2026-09-07 | `6f21379913` | engine: headless harness and ASCII schematic rendering in the... | 705 | 31 | — |
| 2026-09-08 | `e9a01f8417` | kovan: user relations, artifact overlays and a headless mindm... | 3,903 | 64 | — |
| 2026-09-08 | `8e3fc21eb1` | kovan update: 3 and 6 | 397 | 54 | — |
| 2026-09-08 | `900a30debe` | attempted kovan dogfooding | 561 | 34 | — |
| 2026-09-08 | `ef9cce9afd` | fixing the csv boxes | 150 | 5 | — |
| 2026-09-08 | `63945f6072` | kovan: one artifact schema for every block, relations in the ... | 1,007 | 279 | — |
| 2026-09-08 | `b75f5949b4` | kovan PDF reader: go-to-page and go-to-digitiser on every art... | 146 | 37 | — |
| 2026-09-08 | `7f9dd3abab` | kovan PDF reader: a second right-click dismisses the dropdown... | 63 | 17 | — |
| 2026-09-08 | `5692738be9` | kovan: drop the Copy CSV button from the PDF reader's artifac... | 34 | 11 | — |
| 2026-09-08 | `0979d82568` | kovan PDF reader: a left-click outside dismisses the dropdown... | 19 | 2 | — |
| 2026-09-08 | `7b47643f6e` | kovan: reinstate Copy CSV in the PDF reader, drop the CopyBut... | 20 | 27 | — |
| 2026-09-09 | `f4664e676b` | docs: scope Adolphus Lye's seven UQ repos into raffles (GH #1... | 133 | 0 | — |
| 2026-09-09 | `ecdea731c1` | changing the look for htgr_sim_v1 | 58 | 8 | — |
| 2026-09-09 | `bd58efdc11` | htgr_sim_v1: correct HTR-10 primary-loop and SG schematic (GH... | 1,003 | 402 | — |
| 2026-09-09 | `ba7eca5e34` | tampines-steam-tables: mean-flow stage-by-stage turbine model | 1,324 | 0 | — |
| 2026-09-09 | `a5c09db52c` | tampines-steam-tables: v0.2.9, document the mean-flow stage m... | 56 | 2 | — |
| 2026-09-09 | `c5180a1ce8` | tampines-steam-tables: transient mean-flow turbine on the 1-D... | 646 | 1 | — |
| 2026-09-09 | `ecc1357bc0` | Cargo.lock: pick up tampines-steam-tables 0.2.9 | 1 | 1 | — |
| 2026-09-09 | `740c82dc76` | tampines-steam-tables: close the shaft loop through the gener... | 363 | 30 | — |
| 2026-09-10 | `0f3b22c0a6` | outram-mc-libs: mandate committing the OpenMC input scripts f... | 50 | 0 | — |
| 2026-09-10 | `ba869d4afa` | outram-mc-libs: explicit per-energy leakage accounting in the... | 389 | 18 | — |
| 2026-09-10 | `ec940f56f0` | outram-mc-libs: physics::reactor_physics -- six-factor + leth... | 1,044 | 1 | — |
| 2026-09-10 | `4a67ed267d` | outram-mc-libs: real Sigma_a on MacroXs -- ScoreType::Absorpt... | 48 | 37 | — |
| 2026-09-10 | `6fa2a97d20` | reference-data: ENDF/B-VIII.0 neutron tapes for the FHR TRISO... | 782,923 | 3 | — |
| 2026-09-10 | `b68ce61437` | outram-mc-libs: FHR ring-RPT pebble builders + code-to-code e... | 3,117 | 1 | — |
| 2026-09-10 | `ee06b5dd40` | outram-mc-libs: nudge across a surface along its normal, not ... | 62 | 5 | — |
| 2026-09-10 | `6d1a7a5021` | outram-mc-libs: absorption is MT=27 (capture + charged-partic... | 63 | 2 | 717,192,170 |
| 2026-09-10 | `954b0ef96c` | outram-mc-libs: carry a signed surface token through crossing... | 938 | 167 | 2,611,709 |
| 2026-09-10 | `4dbe7164b2` | outram-mc-libs: wire the full FHR pebble (graphite S(a,b) + r... | 225 | 161 | 57,532,000 |
| 2026-09-10 | `23cd25497a` | outram-mc: match deck free-gas SiC carbon; ring-RPT vs OpenMC... | 74 | 42 | 20,095,318 |
| 2026-09-10 | `ef48184e08` | pflotran: verify upstream licence is LGPL-3.0, and vendor the... | 108 | 19 | — |
| 2026-09-10 | `ae92a3f2b6` | pflotran: correct the NOTICE to the verified LGPL-3.0 finding | 191 | 25 | — |
| 2026-09-10 | `526d0f7dcf` | outram-mc: regenerate API mirror (adds reactor_physics + fhr_... | 3,372 | 504 | 4,921,980 |
| 2026-09-10 | `e09239ee9a` | njoy: record the U-238 URR seam defect + add a read-upstream-... | 155 | 0 | — |
| 2026-09-10 | `5732c661d6` | dwsim-libs: expose the three unreachable ColumnType variants ... | 740 | 11 | — |
| 2026-09-10 | `ac21df4ebd` | dwsim-libs: finalise ColumnType constructors; RefluxedAbsorbe... | 712 | 217 | — |
| 2026-09-10 | `f7b1ace6f2` | dwsim-libs: add a `prelude` module and a worked crude-column ... | 668 | 8 | — |
| 2026-09-10 | `31fa3880a6` | njoy: port BROADR thnmax, fix two URR w(z) defects, verify PU... | 1,590 | 65 | — |
| 2026-09-10 | `95cf5c26b7` | htgr_sim_v1: compare the headless baseline numerically, not b... | 75 | 7 | — |
| 2026-09-10 | `f40b71f619` | njoy: fix WAVE_K (cwaven rounded up), unresx AMUN fold; verif... | 124 | 39 | — |
| 2026-09-10 | `e9548ceffa` | dwsim-libs: regenerate the API-doc mirror | 5,986 | 3,560 | — |
| 2026-09-10 | `543e322e3e` | dwsim-libs: add the upstream port-coverage matrix (#78) | 312 | 0 | — |
| 2026-09-10 | `4024e53d8c` | dwsim-libs: refuse non-physical pseudo-components instead of ... | 377 | 86 | — |
| 2026-09-10 | `fd833197d5` | njoy: port NRO=1 scattering radius for LRU=2; verify UNRESR/P... | 440 | 39 | — |
| 2026-09-10 | `9df9cc9c40` | njoy: THERMR temperature interpolation on evaluated quantitie... | 292 | 65 | — |
| 2026-09-10 | `32c1aadf4d` | njoy: split heatr/mod.rs and wmp.rs under the file-size cap (... | 2,901 | 2,792 | — |
| 2026-09-10 | `b1b7c80ac2` | njoy: first GROUPR golden-file validation vs an NJOY2016 GEND... | 1,056 | 5 | — |
| 2026-09-10 | `95664653d3` | njoy: GROUPR URR self-shielding validated vs NJOY GENDF; fix ... | 645 | 25 | — |
| 2026-09-10 | `660e977148` | njoy: GROUPR flux calculator validated vs NJOY GENDF; three g... | 694 | 20 | — |
| 2026-09-10 | `15986d0d0c` | njoy: make the U-238 Doppler RRR L1 a hard gate, re-baselined... | 77 | 47 | — |
| 2026-09-10 | `a131f4f0f1` | njoy: split purr/mod.rs at the ladder/unrest boundary (op-cjw.8) | 1,477 | 1,361 | — |
| 2026-09-10 | `142bd953b2` | njoy: ERRORR MF=33 path ported end to end, validated vs nine ... | 49,605 | 61 | — |
| 2026-09-10 | `21e251ef08` | njoy: BROADR adaptive grid (broadn) + ENDF E-exponent parser ... | 776 | 60 | — |
| 2026-09-10 | `582beeced2` | njoy: ERRORR covout writes sub-eps interior elements verbatim... | 10 | 4 | — |
| 2026-09-10 | `3062ad78f5` | njoy: COVR library option end to end, byte-identical to NJOY ... | 13,356 | 310 | — |
| 2026-09-10 | `fe9818f2ea` | njoy: LEAPR mixed-moderator (b7<=0) merge, verified vs NJOY20... | 35,179 | 130 | — |
| 2026-09-10 | `5421e92d93` | njoy: RECONR resonance-grid convergence ported from resxs; Si... | 16,954 | 103 | — |
| 2026-09-10 | `6513de628b` | njoy: BROADR erfc to double precision; H-2 capture no longer ... | 100 | 18 | — |
| 2026-09-10 | `e8be678686` | njoy: LEAPR skold (Sköld coherence) ported, verified vs NJOY2... | 53,873 | 27 | — |
| 2026-09-10 | `25a88efb32` | njoy: MIXR golden test vs NJOY2016 (H-2 + Be-9 mix identical ... | 990 | 0 | — |
| 2026-09-10 | `20f7b9e092` | njoy: DTFR golden test vs NJOY2016 CLAW n-n table (U-238, 32x... | 945 | 3 | — |
| 2026-09-10 | `5d2232594c` | njoy: RESXSR byte-identical to NJOY2016: gfortran record fram... | 238 | 18 | — |
| 2026-09-10 | `aceead0f6c` | njoy: DTFR edit accumulation ported; CLAW edit block matches ... | 138 | 7 | — |
| 2026-09-10 | `4d8f791ff1` | njoy: RESXSR multi-temperature loop ported, byte-identical to... | 1,992 | 28 | — |
| 2026-09-10 | `06dcec5da7` | njoy: GROUPR two-body elastic matrix ported (getfle/getco, ge... | 2,136 | 16 | — |
| 2026-09-10 | `4923e1fd62` | njoy: GROUPR P0-P3 elastic matrix and MT=257/258/259 derived ... | 1,565 | 99 | — |
| 2026-09-10 | `be04ddacb9` | njoy: GROUPR genflx heterogeneity / multi-moderator terms por... | 717 | 121 | — |
| 2026-09-10 | `c51ceee9ed` | njoy: GROUPR discrete-level inelastic (MT=51/52/60/89) vector... | 958 | 20 | — |
| 2026-09-10 | `8a231bdf8d` | njoy: GAMINR gtff (coherent/incoherent/pair feeds), photon gp... | 1,924 | 65 | — |
| 2026-09-10 | `3b5a6181f2` | njoy: DTFR full-channel assembly (nu*sigma_f, chi incl. const... | 3,062 | 0 | — |
| 2026-09-10 | `2803d617e5` | njoy: GROUPR MF=10 residual-production yield search ported (m... | 498 | 11 | — |
| 2026-09-10 | `ba700a8f6c` | njoy: ERRORR MF=32 resprx chain ported (rpxlc2/rpxlc12/rpendf... | 63,072 | 33 | — |
| 2026-09-10 | `ced7a0c7d2` | njoy: RECONR evaluates LRF=2 with csmlbw's multilevel elastic... | 8,082 | 11 | — |
| 2026-09-10 | `f7276573c3` | njoy: LEAPR run driver (whole deck -> MF=1/451 Hollerith head... | 3,814 | 87 | — |
| 2026-09-10 | `b9d44326bf` | njoy: ERRORR MF=32 LRF=7 rpxsamm + SAMM resonance-parameter d... | 151,564 | 235 | — |
| 2026-09-10 | `e71405d28d` | reference-data: add maintainer-supplied H-in-H2O tsl and U ph... | 245,905 | 0 | — |
| 2026-09-10 | `cb645045cf` | reference-data: replace stale "Still wanted" list with the fi... | 23 | 3 | — |
| 2026-09-10 | `d2f98d68e2` | njoy: ERRORR LRF=3 ggrmat + LCOMP=1 oracle (JENDL-3.3 U-238);... | 43,230 | 66 | — |
| 2026-09-10 | `0045ba59d3` | reference-data: six evaluations from the NJOY2016 upstream te... | 327,652 | 12 | — |
| 2026-09-10 | `ba8b615130` | reference-data: those two tapes are JENDL-3.3, not JEFF-3.3 | 6 | 6 | — |
| 2026-09-11 | `0cd9a22c9e` | outram-mc: reflective-sphere delta domain, so the FHR pebble ... | 448 | 46 | — |
| 2026-09-11 | `50033b8a7e` | tests: 19 thermal tests were passing without asserting anything | 48 | 10 | — |
| 2026-09-11 | `f5c26afdd9` | tests: OUTRAM_PARK_REQUIRE_REFERENCE_DATA turns a data-gated ... | 148 | 16 | — |
| 2026-09-11 | `a7280ecb34` | outram-mc: the naive-homogenised FHR pebble was a different r... | 402 | 62 | — |
| 2026-09-11 | `cd37bc3990` | V&V: rewrite the ring-RPT record on the corrected, matched-ge... | 92 | 20 | — |
| 2026-09-11 | `22f7dc0e99` | outram-mc: single-case mode, and the baseline shift is data n... | 34 | 14 | — |
| 2026-09-11 | `53f64f5b51` | docs: bring the outram-mc-libs API mirror up to the DeltaDoma... | 72 | 9 | — |
| 2026-09-11 | `e744fac3e3` | docs: Haiku dogfood of the ring-RPT / explicit-TRISO pebble A... | 327 | 0 | — |
| 2026-09-11 | `0472ef371b` | tests: fix the red graphite sigma_inel pins, and add the inva... | 101 | 6 | — |
| 2026-09-11 | `4b95153d83` | dwsim-libs: fix RefluxedAbsorber — the port took a dead upstr... | 752 | 108 | — |
| 2026-09-11 | `43e57bb91c` | outram-mc: ExplicitTrisoPebble, the builder the explicit path... | 273 | 26 | — |
| 2026-09-11 | `1314329edb` | dwsim-libs: let a material stream carry a flashed state | 396 | 0 | — |
| 2026-09-11 | `66ba367f6e` | docs: mirror ExplicitTrisoPebble, the precondition for the do... | 60 | 0 | — |
| 2026-09-11 | `5d9d005de8` | farrer-park: crate scaffold, verified licence provenance, des... | 364 | 0 | — |
| 2026-09-11 | `ed8db6de4c` | outram-mc: re-export TrisoMaterials and ComputeType, found by... | 13 | 2 | — |
| 2026-09-11 | `229784c52c` | docs: show the TrisoMaterials import line in the mirror | 6 | 0 | — |
| 2026-09-11 | `5c8bf90ea6` | farrer-park: WIP snapshot — tensor, element, quadrature, mesh... | 4,009 | 0 | — |
| 2026-09-11 | `26d540cbb7` | farrer-park: WIP snapshot — all 13 modules present, crate com... | 3,453 | 0 | — |
| 2026-09-11 | `9a74608920` | docs: Haiku dogfood run 2 — the A/B after ExplicitTrisoPebble | 91 | 0 | — |
| 2026-09-11 | `2d73a6b761` | farrer-park: WIP — patch-test harness, mesh/quadrature/solver... | 829 | 65 | — |
| 2026-09-11 | `c05290368c` | farrer-park: WIP — full verification suite present (MMS, anal... | 1,151 | 68 | — |
| 2026-09-11 | `30705a432d` | outram-mc: FHR pebble quickstart, the gap both dogfood runs c... | 239 | 0 | — |
| 2026-09-11 | `29e1119cad` | farrer-park: verification suite passing, results recorded; co... | 776 | 11 | — |
| 2026-09-11 | `1e1000ff2a` | farrer-park: README limitations; record verification, gates a... | 34 | 3 | — |
| 2026-09-11 | `57d65535e0` | farrer-park: doc-accuracy fixes in crate CLAUDE.md | 23 | 5 | — |
| 2026-09-11 | `7e2cb53389` | docs: add farrer-park to the workspace Members table and depe... | 8 | 0 | — |
| 2026-09-11 | `09c3f59b8d` | outram-mc: compare six factors in OpenMC's convention, and an... | 248 | 1 | — |
| 2026-09-11 | `a2714f5630` | outram-mc: graphite S(a,b) matches NJOY too -- the bias is in... | 191 | 5 | — |
| 2026-09-11 | `2577eb25c5` | outram-mc: the scattering kernel is clean too -- every named ... | 144 | 19 | — |
| 2026-09-11 | `2d8a8158e3` | farrer-park: vendor upstreams, record provenance, correct the... | 1,016 | 43 | — |
| 2026-09-11 | `8eaf6c1b71` | WIP farrer-park: locking scratch harness (agent mid-flight, o... | 176 | 1 | — |
| 2026-09-11 | `2c32b45ae5` | farrer-park: B-bar locking treatment and plane stress, both v... | 2,254 | 135 | — |
| 2026-09-11 | `05f1ebf955` | farrer-park: finish B-bar and plane stress; record the two ne... | 192 | 9 | — |
| 2026-09-11 | `c68c8f8ab9` | WIP farrer-park: crystal-plasticity core (agent mid-flight, s... | 2,400 | 4 | — |
| 2026-09-11 | `475d15a62b` | farrer-park: crystal-plasticity verification suite; tangent q... | 1,515 | 175 | — |
| 2026-09-11 | `2b81c46aff` | WIP farrer-park: Fatemi-Socie FIP module (agent mid-flight, s... | 856 | 7 | — |
| 2026-09-11 | `e95556e161` | outram-mc: generalise the NJOY PENDF oracle, and add a nu-bar... | 29 | 5 | — |
| 2026-09-11 | `d055dbbb47` | farrer-park: crystal plasticity done, fatigue partial; docs a... | 942 | 26 | — |
| 2026-09-11 | `9cc24a6e50` | dwsim-libs: converged cut compositions, and #62's PR tests ar... | 679 | 1 | — |
| 2026-09-11 | `61c6954670` | dwsim-libs: amend the port-coverage matrix, and correct two o... | 57 | 29 | — |
| 2026-09-11 | `04f5d1924b` | dwsim-libs: audit stream / property-package ownership (#127) | 538 | 0 | — |
| 2026-09-11 | `6909393450` | dwsim-libs: add a name -> Component lookup (no new compound d... | 1,223 | 2 | — |
| 2026-09-11 | `3f74b1d4fb` | dwsim-libs: regenerate the API-doc mirror | 1,083 | 13 | — |
| 2026-09-11 | `c356cf2552` | outram-mc: sample free-gas target motion (op-50vu), and three... | 1,744 | 261 | — |
| 2026-09-11 | `1572497ffc` | outram-mc: the explicit pebble was packed 2.6% over the deck ... | 455 | 141 | — |
| 2026-09-11 | `3f19593c35` | outram-mc: report the six factors in OpenMC's two-group conve... | 34 | 7 | — |
| 2026-09-11 | `a64af039e5` | outram-mc: test resonance SELF-SHIELDING, and match the deck'... | 325 | 6 | — |
| 2026-09-11 | `3cef3bb87c` | outram-mc: exclude the last two mechanisms — resonance shape,... | 220 | 33 | — |
| 2026-09-11 | `6927f9e541` | ring-rpt V&V: the thermal-benchmark blocker is the spec, not ... | 20 | 4 | — |
| 2026-09-11 | `86df672a82` | outram-mc: rebuild NJOY2016 and verify every remaining nuclid... | 61 | 10 | — |
| 2026-09-11 | `9b5180169f` | outram-mc: check the graphite S(a,b) KERNEL against NJOY, not... | 301 | 13 | — |
| 2026-09-11 | `646d4d248f` | ring-rpt V&V: localise the residual to one scalar, and check ... | 53 | 6 | — |
| 2026-09-11 | `97b2eacb30` | ring-rpt V&V: close the moderating-power branch, and record t... | 87 | 7 | — |
| 2026-09-11 | `ea635dd3f6` | outram-mc: a THERMAL criticality benchmark — HEU-SOL-THERM-00... | 37,495 | 29 | — |
| 2026-09-11 | `54cc84adf8` | outram-mc: a U-238-DOMINATED criticality benchmark — Jemima, ... | 335 | 11 | — |
| 2026-09-11 | `ec865ef347` | outram-mc: the +4004 pcm is OURS — LEU-COMP-THERM-008, +2950 ... | 13,451 | 23 | — |
| 2026-09-11 | `d5908bf638` | outram-mc: the hunt's lessons as regression tests — and the r... | 2,114 | 5 | — |
| 2026-09-11 | `d4b0da78ff` | outram-mc: the SPATIAL half is sound too — and a boundary con... | 847 | 2 | — |
| 2026-09-11 | `ecda597cfc` | outram-mc: the residual is NOT U-238 resonance escape — it tr... | 2,217 | 52 | — |
| 2026-09-11 | `00a095d8dc` | outram-mc: ROOT CAUSE — our H-in-H2O thermal KERNEL moves neu... | 346 | 5 | — |
| 2026-09-11 | `fe2458d746` | outram-mc: every NJOY/analytic oracle from the hunt, as golde... | 1,098 | 3 | — |
| 2026-09-11 | `2a4343afc2` | outram-mc: every oracle example asserts its comparison, not j... | 3,021 | 239 | — |
| 2026-09-11 | `667f335cb6` | outram-mc: gate the ring-RPT, fingerprint and remaining Godiv... | 426 | 4 | — |
| 2026-09-11 | `0d948a2099` | outram-mc: size the lump-scan gate from its own statistics, n... | 17 | 2 | — |
| 2026-09-11 | `0a49852f9d` | njoy-fork: the oracle examples assert too, and the gate helpe... | 1,355 | 421 | — |
| 2026-09-11 | `de3910fbf2` | njoy-fork: assert the GPU WMP path agrees with the trusted CP... | 55 | 0 | — |
| 2026-09-11 | `2714d4134e` | V&V examples: two that couldn't run here, could all along | 172 | 69 | — |
| 2026-09-11 | `8ae2a49759` | outram-mc: gate the HTR-10 double-heterogeneity difference, a... | 105 | 0 | — |
| 2026-09-11 | `a6ade05d42` | outram-mc: record the ring-RPT re-measurement -- the last gat... | 16 | 0 | — |
| 2026-09-11 | `585c1cd1d5` | outram-mc: gate the four-method stochastic-media comparison | 147 | 0 | — |
| 2026-09-12 | `86da3c198d` | outram-mc: move the maturity evidence off a run that cannot t... | 109 | 27 | — |
| 2026-09-12 | `1e8ff714bd` | naming: OUTRAM PARK is Multi-Physics, not Multi-Phase | 50 | 15 | — |
| 2026-09-12 | `e97642c5f4` | outram-mc: measure the two moments of the thermal kernel that... | 1,171 | 0 | — |
| 2026-09-12 | `a76cb84dd0` | docs: cite the Outram Park Part I arXiv preprint (2608.17504) | 71 | 0 | 9,416,063 |
| 2026-09-12 | `190c2e73e2` | naming: record how Multi-Physics reaches the literature, not ... | 35 | 6 | 1,275,153 |
| 2026-09-12 | `3697c0655a` | docs: extract paper datasets for three manuscripts (op-rpb6) | 331 | 0 | 10,322,907 |
| 2026-09-12 | `7df8e711e7` | docs: Part II carries a brief nuclear-data-preparation verifi... | 65 | 7 | 3,692,896 |
| 2026-09-12 | `e052afbace` | docs: split neutronics into Part II (data + transport) and Pa... | 252 | 138 | 4,182,591 |
| 2026-09-12 | `91589f1129` | docs: fix the target set at three manuscripts (op-rpb6) | 28 | 12 | 1,735,517 |
| 2026-09-12 | `ccd55fc683` | docs: add the OpenMC absolute and cross-code equivalence tabl... | 9 | 0 | 4,787,557 |
| 2026-09-12 | `2a287620de` | outram-mc: pin the ring-RPT model against the OpenMC deck it ... | 231 | 0 | — |
| 2026-09-12 | `93e183212e` | outram-mc: record the composition and provenance checks on th... | 37 | 4 | — |
| 2026-09-12 | `8905055694` | outram-mc: the thick lump now has an oracle, and the Monte Ca... | 2,185 | 49 | — |
| 2026-09-12 | `3cdc2b1f8f` | outram-mc: assert the FLiBe density correlation against the d... | 77 | 0 | — |
| 2026-09-12 | `5916b91774` | outram-mc: the thermal kernel was wrong in both dimensions of... | 1,500 | 203 | — |
| 2026-09-12 | `f540b6acf2` | outram-mc: re-baseline every thermal number the emission-tabl... | 223 | 78 | — |
| 2026-09-12 | `bb94ac17bf` | outram-mc: the bound thermal kernels do not have the right fi... | 596 | 0 | — |
| 2026-09-12 | `9efb44dce1` | outram-mc: the emission-table resize is worth -371 +/- 194 pc... | 125 | 1 | — |
| 2026-09-12 | `e2ef43cfad` | outram-mc: price the graphite thermal law by deleting it -- i... | 133 | 1 | — |
| 2026-09-12 | `882cb3e343` | outram-mc: check the elastic angular law, then price it -- al... | 443 | 5 | — |
| 2026-09-12 | `75615b6786` | outram-mc: give the pricing harness a positive control, and i... | 58 | 9 | — |
| 2026-09-12 | `45f8d72d86` | outram-mc/njoy: a threshold reaction cannot occur below its t... | 802 | 53 | — |
| 2026-09-13 | `5ff8026134` | docs: rebaseline the paper datasets after the F-19 threshold ... | 204 | 87 | 13,996,182 |
| 2026-09-13 | `549fc5c6eb` | docs: keep Part II -> Part III order, and caveat the gaps exp... | 109 | 21 | 3,125,614 |
| 2026-09-12 | `81c6a14c7c` | outram-mc: sample the thermal kernel continuously, and find t... | 274 | 49 | — |
| 2026-09-13 | `2dd912fb32` | outram-mc: the continuous thermal kernel takes the FHR pebble... | 22 | 3 | — |
| 2026-09-13 | `76cb011101` | outram-mc: correct a k I derived instead of read, and carry t... | 39 | 5 | — |
| 2026-09-13 | `86907bf353` | docs: rebaseline Part III again after the continuous thermal ... | 34 | 21 | 36,539,815 |
| 2026-09-13 | `11fb94a36e` | outram-mc: three V&V gates re-armed after the continuous ther... | 71 | 14 | — |
| 2026-09-13 | `9abfe85472` | outram-mc: LCT-008 measured against all three fixes, and the ... | 51 | 25 | — |
| 2026-09-13 | `626d995f28` | docs: rebaseline Part II datasets to the frozen tree (9abfe85... | 16 | 9 | 9,152,184 |
| 2026-09-13 | `e845dc2c04` | outram-mc: every bound thermal law this repo carries is narro... | 278 | 0 | — |
| 2026-09-13 | `42597707f8` | njoy: bring sig()'s S(a,b) evaluation onto NJOY2016's own, in... | 328 | 40 | — |
| 2026-09-13 | `46219dd97a` | outram-mc: Godiva's +57 pcm baseline was one seed's draw, not... | 82 | 15 | — |
| 2026-09-13 | `41abb39d8d` | outram-mc+njoy: read the evaluated MF=6 continuum law instead... | 1,164 | 64 | — |
| 2026-09-13 | `0be4d89b7b` | vv: record the FHR pebble's MF=6 re-measurement, and say what... | 73 | 4 | — |
| 2026-09-13 | `cecd8bd92f` | njoy: adaptively linearise THERMR's E' grid -- most of H2O's ... | 205 | 33 | — |
| 2026-09-13 | `8b8f5a7eb1` | vv: re-run LCT-008 on current code -- its residual is still g... | 28 | 8 | — |
| 2026-09-13 | `9058ee2167` | outram-mc: record that N_OUTGOING is most of gh:#188's width ... | 38 | 0 | — |
| 2026-09-13 | `035f961793` | farrer-park: declare the crate mature (maintainer decision, 2... | 62 | 9 | — |
| 2026-09-13 | `102527c1dc` | njoy: port THERMR calcem/sigl and ACER's MF=5/MF=6 laws, and ... | 3,510 | 820 | — |
| 2026-09-13 | `1010c0952a` | njoy: gate THERMR's iform=1 continuous law, and settle an ope... | 282 | 0 | — |
| 2026-09-13 | `f5086cd214` | njoy: gh:#188 was never blocked -- NJOY2016 is built here, so... | 257 | 0 | — |
| 2026-09-13 | `c163ab847b` | njoy: gh:#188 root-caused against NJOY's own emission matrix ... | 284 | 0 | — |
| 2026-09-13 | `7e850615d0` | groupr: port f6dis, ll2lab and f6lab; verify the pass the tra... | 1,448 | 457 | — |
| 2026-09-13 | `32a43a086f` | leapr: isym was computed wrongly, and two options were refuse... | 166 | 6 | — |
| 2026-09-13 | `1785f8271b` | dwsim-libs: declare mature (maintainer decision, 2026-09-13) | 57 | 8 | — |
| 2026-09-13 | `4d074ea8b9` | gh:#188 FIXED: replace the home-grown emission matrix with NJ... | 1,115 | 23 | — |
| 2026-09-13 | `9892347b41` | groupr: add emax_ev to the ZrH thermal-ACE test's options lit... | 2 | 0 | — |
| 2026-09-13 | `99800e7ece` | groupr: add emax_ev to the remaining thermal-ACE test options... | 2 | 0 | — |
| 2026-09-13 | `bd3c250722` | dwsim-libs: upstream DWSIM now runs headless — the maturity b... | 70 | 11 | — |
| 2026-09-13 | `83d60d507e` | dwsim-libs: CORRECTION — the PR EOS matches upstream exactly;... | 6 | 6 | — |
| 2026-09-13 | `3cf70124d8` | dwsim-libs: apply the correction to the crate file (83d60d50 ... | 32 | 16 | — |
| 2026-09-13 | `a1452324e1` | dwsim-libs: trace the upstream EOS/CalcProp gap to a Peneloux... | 31 | 8 | — |
| 2026-09-13 | `9d567a806c` | dwsim-libs: PT-flash parity vs upstream DWSIM — gap traced to... | 100 | 0 | — |
| 2026-09-13 | `721fcf9f56` | dwsim-libs: preserve the upstream headless harness; column so... | 248 | 0 | — |
| 2026-09-14 | `932819a517` | groupr: port getmf6 (File-6 continuum feed), and fix f6lab dr... | 1,723 | 42 | — |
| 2026-09-14 | `e945c03b47` | dwsim-libs: fix a library panic in the cubic root solver (fou... | 45 | 1 | — |
| 2026-09-14 | `622c65d717` | dwsim-libs: column-layer parity vs upstream DWSIM — all three... | 177 | 56 | — |
| 2026-09-14 | `5bbac8a824` | groupr: wire the File-6 continuum feed into the matrix driver... | 257 | 9 | — |
| 2026-09-14 | `8d612e33e0` | leapr: wire and VALIDATE cold H2/D2 -- coldh reproduces NJOY'... | 5,033 | 22 | — |
| 2026-09-14 | `74d6fb2c4f` | acer: survey the MF=5/MF=6 surface and make the one real gap ... | 22 | 3 | — |
| 2026-09-14 | `d801e59b24` | petir: vendor GSL with a VERIFIED licence, and the no_std ske... | 419 | 0 | — |
| 2026-09-14 | `7b669caa62` | petir: port GSL's Chebyshev module, verified code-to-code aga... | 2,976 | 0 | — |
| 2026-09-14 | `edb42e8e01` | petir: convention adapter, and code-to-code against tampines'... | 151 | 0 | — |
| 2026-09-14 | `5fe4978550` | tampines: WIRE the Chebyshev evaluation to PETIR (bn:op-chyp.2) | 423 | 73 | — |
| 2026-09-14 | `cce9bba0e8` | petir: port GSL's exponential integral E_1 -- the first half ... | 920 | 0 | — |
| 2026-09-14 | `6b230a234a` | ACER: wire MF=5 LF=12 (Madland-Nix) onto PETIR -- it no longe... | 4,042 | 24 | — |
| 2026-09-14 | `a6f8974b83` | slides: OUTRAM PARK capabilities and roadmap (beamer) | 700 | 0 | — |
| 2026-09-14 | `b34fb2575a` | Remove the last extern "C" calls into the system C libm (bn:o... | 2,668 | 96 | — |
| 2026-09-14 | `e64e99d1cd` | outram-mc-libs: route the transcendentals through PETIR behin... | 400 | 103 | — |
| 2026-09-14 | `fab07081b7` | petir: fast exp ported from ARM optimized-routines, 100 % bit... | 9,538 | 30 | — |
| 2026-09-14 | `8b33e0d998` | petir: fast log and pow ported from ARM optimized-routines, b... | 62,657 | 39 | — |
| 2026-09-14 | `663124c04a` | petir: make the ARM port the default route for exp, ln and powf | 512 | 170 | — |
| 2026-09-14 | `27ac459ef2` | outram-mc: price the four DH tracking treatments against each... | 639 | 0 | — |
| 2026-09-14 | `82346911c6` | outram-mc: one enum to pick how a doubly heterogeneous univer... | 862 | 0 | — |
| 2026-09-14 | `1bc5645158` | outram-mc: k-eff V&V across the three DH treatments | 186 | 0 | — |
| 2026-09-14 | `05f2f686db` | outram-mc: DH k-eff V&V measured — both "speedups" are slowdo... | 74 | 9 | — |
| 2026-09-14 | `166e4e8d7d` | outram-mc: move the DH V&V onto ENDF/B-VIII.0, not the LOW em... | 118 | 29 | — |
| 2026-09-14 | `a0fdd6eaa9` | outram-mc: record the HIGH-tier ENDF/B-VIII.0 DH V&V results | 42 | 49 | — |
| 2026-09-14 | `355ca6f0fc` | outram-mc: DhTreatment::RingRpt did naive homogenisation, not... | 1,182 | 311 | — |
| 2026-09-14 | `7742e18612` | outram-mc: bound the delta majorant over reachable materials ... | 164 | 2 | — |
| 2026-09-14 | `d1454a9423` | outram-mc: record the post-fix DH V&V, and retract the "appro... | 101 | 40 | — |
| 2026-09-14 | `6817e3018f` | outram-mc: DH V&V at 9x statistics — ring-RPT reproduces exac... | 124 | 71 | — |
| 2026-09-14 | `37e1279ae6` | docs: release notes and historian report for the 2026-09-14 m... | 1,828 | 0 | — |
| 2026-09-14 | `0e35efea70` | docs: reconcile the release-notes commit count with the histo... | 4 | 2 | — |
| 2026-09-14 | `07581a91cc` | docs: state the release-notes count as work-only, so it stops... | 6 | 5 | — |
| 2026-09-14 | `5f433b0b43` | outram-mc: record the machine behind every DH timing, and wid... | 77 | 13 | — |
| 2026-09-14 | `0978776176` | petir: lift the RealMath trait into the crate (bn:op-j57z) | 153 | 0 | — |
| 2026-09-14 | `baf9408b44` | tampines-steam-tables: route exp/ln/powf through PETIR's ARM ... | 264 | 173 | — |
| 2026-09-14 | `9fa1856e0b` | docs: LaTeX listing of delta tracking and SCLS for the Part I... | 135 | 0 | — |
| 2026-09-14 | `b913378cd9` | docs: file the kopi-beans 0.1.9 sync failure that strands wri... | 112 | 0 | — |
| 2026-09-14 | `eca4889619` | outram-mc: stop printing a misleading OpenMC comparison from ... | 26 | 12 | — |
| 2026-09-14 | `4281cc8fd5` | docs: correct the kopi-beans write-up — the daemon checkpoint... | 136 | 112 | — |
| 2026-09-14 | `040a011a0c` | outram-mc: the smeared arms carried 3.7 % more fuel than the ... | 393 | 48 | — |
| 2026-09-14 | `350b7c202b` | outram-mc: majorant under-bounded Sigma_t below its grid floo... | 252 | 16 | — |
| 2026-09-14 | `8b829eae00` | outram-mc: measure whether the CLS/SCLS lock costs anything (... | 162 | 0 | — |
| 2026-09-14 | `84bb23c719` | outram-mc: SCLS is the wrong method for a graphite pebble, an... | 76 | 7 | — |
| 2026-09-14 | `ee34c1e100` | outram-mc: measured GH #205 — the throughput claim is refuted... | 42 | 3 | — |
| 2026-09-14 | `9387f6b0de` | outram-mc: surface tracking computed nested-universe surface ... | 173 | 58 | — |
| 2026-09-14 | `f718e50d7a` | outram-mc: the nested-frame crossing fix also moves the hex l... | 24 | 5 | — |
| 2026-09-14 | `2655d83fa1` | outram-mc: LEU-COMP-THERM-008 goes from +2341 to +37 pcm agai... | 70 | 6 | — |
| 2026-09-14 | `1bc7c0fb4f` | tampines-steam-tables: (rho,h) flash by inverting the IF97 ba... | 4,512 | 12 | — |
| 2026-09-14 | `11107ad046` | tampines-steam-tables: measure (rho,h) conditioning on the an... | 280 | 20 | — |
| 2026-09-14 | `56b3a83c68` | tampines-steam-tables: measure what the (rho,h) inversion cos... | 117 | 6 | — |
| 2026-09-14 | `2ab91fefc3` | tampines-steam-tables: (p,h), (p,s) and (h,s) now cover Region 5 | 1,017 | 37 | — |
| 2026-09-14 | `2f13bf42cc` | vv: re-extract the ring-RPT pebble case with the MF=6 continu... | 95 | 21 | — |
| 2026-09-14 | `f2fb106225` | docs: say which Part III numbers want to be small and which w... | 31 | 5 | — |
| 2026-09-14 | `a7a136c32f` | tampines-steam-tables: two negative results on where (rho,h) ... | 403 | 43 | — |
| 2026-09-14 | `96d2740b34` | steam-tables GUI isochore now crosses the dome; Sod gains a t... | 363 | 126 | — |
| 2026-09-14 | `fc20443c96` | docs: drop the superseded kopi-beans queue entry, now filed u... | 0 | 136 | — |
| 2026-09-14 | `892c389828` | petir: a platform-libm escape hatch, and the Edwards golden r... | 1,978 | 2 | — |
| 2026-09-14 | `81a0ec15b5` | tampines-steam-tables: the golden reference reproduces, and A... | 305 | 0 | — |
| 2026-09-14 | `45b5981625` | outram-mc: LCT008 re-measured at the original statistics — +2... | 51 | 35 | — |
| 2026-09-14 | `24e568017e` | tampines-steam-tables: the Edwards over-drain, and the energy... | 186 | 2 | — |
| 2026-09-14 | `ddb3ed443d` | njoy: five doc comments that contradicted the code they document | 46 | 15 | — |
| 2026-09-14 | `5e18aafdf2` | njoy: LRF=7 discarded its background R-matrix, and that was t... | 46,039 | 25 | — |
| 2026-09-14 | `fccb64356a` | petir: the no_std numerics core -- GSL ports, verbatim lifts,... | 11,393 | 0 | — |
| 2026-09-14 | `d59323b405` | njoy: MT=1 was not the sum of its parts, and the obvious fix ... | 462 | 0 | — |
| 2026-09-14 | `15f0132f61` | njoy: a doc comment made a zero-cross-section hole look inten... | 39 | 3 | — |
| 2026-09-14 | `8174b6a5b9` | tampines-steam-tables: A3 log -- the energy hold works, and t... | 105 | 0 | — |
| 2026-09-14 | `c681adcd07` | petir: recover the GSL Chebyshev port and its code-to-code re... | 2,983 | 4 | — |
| 2026-09-14 | `e75ce9ea34` | tampines-steam-tables: A4 finds a ported operator that was ne... | 175 | 4 | — |
| 2026-09-14 | `c1e705b1bf` | tampines-steam-tables: a register of omitted upstream terms (... | 84 | 0 | — |
| 2026-09-14 | `38a30837cd` | petir: recover expint, gamma_inc, cheb_slice and the real.rs ... | 8,034 | 58 | — |
| 2026-09-14 | `e3f4d1b07b` | tampines-steam-tables: wire in the transient Rhie-Chow term, ... | 137 | 1 | — |
| 2026-09-14 | `93a0c6b084` | tampines-steam-tables: Edwards passes at 30 us again, and is ... | 147 | 3 | — |
| 2026-09-14 | `0d1f690405` | tampines-steam-tables: make the enthalpy-hold band-aid self-r... | 27 | 0 | — |
| 2026-09-14 | `bd6ce7ec1c` | tampines-steam-tables: keep both band-aids, make them impossi... | 60 | 69 | — |
| 2026-09-14 | `9c190f4ca1` | tampines-steam-tables: the hold never fires now -- over-drain... | 102 | 8 | — |
| 2026-09-14 | `4285fdf5bc` | petir: pull in the consumer chain, port GSL roots + min, cred... | 75,689 | 292 | — |
| 2026-09-14 | `38a1b9cab4` | petir: port GSL numerical differentiation, and fix a docs cla... | 515 | 2 | — |
| 2026-09-14 | `c116688246` | petir: port GSL interpolation and the tridiagonal solver (bn:... | 1,180 | 1 | — |
| 2026-09-14 | `521d83cae5` | petir: port GSL quadrature and ODE integration -- the epic's ... | 1,757 | 3 | — |
| 2026-09-14 | `a4068bbfcc` | njoy: reconstruct the unresolved range, and measure who is ri... | 254,576 | 0 | — |
| 2026-09-15 | `32fca16539` | njoy: figure and full-precision table for the URR interpolati... | 334 | 29 | — |
| 2026-09-15 | `9483d1d5b2` | petir: remove every panicking subscript from the ported modul... | 1,291 | 325 | — |
| 2026-09-15 | `50cf30aeba` | Remove every panicking subscript from petir's verbatim lifts,... | 1,340 | 434 | — |
| 2026-09-15 | `6c057d8129` | njoy: write MF=2/MT=152, and use NJOY's own table to settle t... | 707 | 24 | — |
| 2026-09-15 | `c802a59bd6` | responsible use: say what counts as the initial human review | 11 | 0 | — |
| 2026-09-15 | `c302284771` | njoy: reconr() emits MF=2/MT=152, and the phase ordering that... | 168 | 7 | — |
| 2026-09-15 | `f024f6a415` | njoy: build eunr the way rdf2u0/u1/u2 build it, and close MT=... | 1,011 | 94 | — |
| 2026-09-15 | `722abe9b9b` | njoy: close MT=152's last unresolved path with a synthetic Ca... | 381 | 28 | — |
| 2026-09-15 | `f619be3b5b` | outram-mc: elastic slowing-down kinematics against a closed-f... | 289 | 0 | — |
| 2026-09-15 | `537dc92d9c` | tampines-steam-tables GUI: isochores on a temperature sweep, ... | 453 | 129 | — |
| 2026-09-15 | `e141bbbb64` | tampines-steam-tables: p_rho_h_eqm_explicit is now actually e... | 215 | 13 | — |
| 2026-09-15 | `85330ef769` | tampines-steam-tables: Region 1 sub-boundary for explicit p(r... | 457 | 25 | — |
| 2026-09-15 | `f6d797f42c` | tampines-steam-tables: the (rho,h) classifier was the dominan... | 357 | 5 | — |
| 2026-09-15 | `5152d096f9` | tampines-steam-tables: measure the explicit route's cost, and... | 21 | 0 | — |
| 2026-09-15 | `81dda3904f` | tampines-steam-tables: h-anchored T refinement fixes the liqu... | 197 | 39 | — |
| 2026-09-15 | `6e0cee5989` | njoy: the unfac l>=3 defect reaches a SHIPPING library, not j... | 122,263 | 2 | — |
| 2026-09-15 | `e20aef9efa` | petir: Chebyshev least-squares fitting at arbitrary points, o... | 2,501 | 19 | — |
| 2026-09-15 | `491d36a253` | petir: port GSL's last two Chebyshev entry points, and pin th... | 906 | 18 | — |
| 2026-09-15 | `d4308bea7b` | boon-lay: exhaustive code-to-code verification against upstre... | 5,667 | 1 | — |
| 2026-09-15 | `0e2b55cdd6` | boon-lay: complete the TRISO-ATOPS code-to-code sweep (bn:op-... | 6,273 | 4,660 | — |
| 2026-09-15 | `84505ba898` | petir: declared mature; close the uneven-evidence gap the dec... | 1,513 | 11 | — |
| 2026-09-15 | `bf5a92ef43` | changi: new crate, FLEXPART port, verified code-to-code at tw... | 6,363 | 2 | — |
| 2026-09-15 | `ed9fd51521` | petir: bring the peroxide/roots acknowledgement up to date wi... | 84 | 0 | — |
| 2026-09-15 | `b123ee2a9e` | genfoam/offbeat: code-to-code verification through the blockM... | 862 | 0 | — |
| 2026-09-15 | `63eee188f2` | changi: record the CHANGI and REDHILL acronyms and CHANGI's f... | 107 | 44 | — |
| 2026-09-15 | `dbb50a7618` | changi: PSA and consequence work is future scope; keep the cr... | 79 | 73 | — |
| 2026-09-15 | `58b178a05c` | changi: close the decay code-to-code fixture gap (op-zq9r) | 223 | 30 | — |
| 2026-09-15 | `934ec2532d` | petir: port the closed-form quartic from the `roots` crate, w... | 838 | 22 | — |
| 2026-09-15 | `c11e2e8fa8` | genfoam: read upstream nuclearData + cellZones, and verify k_... | 1,668 | 20 | — |
| 2026-09-15 | `0be3573649` | genfoam: gFHR pebble-conduction consistency check, and the tu... | 520 | 0 | — |
| 2026-09-15 | `394d3b5d9a` | petir: port the companion-matrix root finder from `roots`, cr... | 1,521 | 0 | — |
| 2026-09-15 | `bff34bcb7e` | petir: port polynomial algebra and the orthogonal families fr... | 1,346 | 102 | — |
| 2026-09-15 | `dcf3ff28d4` | genfoam: port GeN-Foam's albedoSP3 boundary condition, and cl... | 934 | 1 | — |
| 2026-09-15 | `ac19c2a0d9` | docs: record the MSFR albedo result in the tutorial verificat... | 41 | 12 | — |
| 2026-09-15 | `b54cce8354` | petir: port Gauss-Legendre and Newton-Cotes quadrature from `... | 1,358 | 3 | — |
| 2026-09-15 | `e92a57a313` | petir: record the quadrature port in the crate CLAUDE.md | 12 | 2 | — |
| 2026-09-15 | `cc06471dfe` | genfoam: port nuclearSteadyStatePebble, the gFHR tutorial's p... | 606 | 0 | — |
| 2026-09-15 | `22479d1ede` | docs: record the gFHR pebble-model port in the tutorial verif... | 44 | 22 | — |
| 2026-09-15 | `22af2a44aa` | docs: ready-to-post upstream issue drafts for `roots` and `pe... | 251 | 0 | — |
| 2026-09-15 | `7c06c3e2df` | docs: record the verification routes checked and ruled out | 28 | 0 | — |
| 2026-09-15 | `cf429ff5d1` | genfoam: reproduce the gFHR tutorial's Tfmax_min from upstrea... | 178 | 0 | — |
| 2026-09-15 | `e429222b6a` | outram-mc: a vacuum delta-tracking boundary, so Godiva can be... | 392 | 18 | — |
| 2026-09-15 | `c809b91c33` | outram-mc: Godiva against OpenMC on identical data -- the +24... | 333 | 4 | — |
| 2026-09-15 | `86296e35da` | outram-mc: the Godiva +250 pcm is NOT nuclear data -- measure... | 346 | 2 | — |
| 2026-09-15 | `f92babfb0f` | outram-mc: localise the Godiva +250 pcm -- missing inelastic ... | 183 | 0 | — |
| 2026-09-15 | `544017b677` | outram-mc: ablate elastic anisotropy and URR, with the tests ... | 570 | 11 | — |
| 2026-09-15 | `97a739df50` | outram-mc: clear elastic scattering against OpenMC, and recor... | 241 | 4 | — |
| 2026-09-15 | `91e4cd8305` | outram-mc: clear the fission spectrum too -- the Godiva attri... | 145 | 4 | — |
| 2026-09-15 | `b1c1d83ac2` | genfoam: build and RUN upstream GeN-Foam, and compare MSFR ag... | 110 | 0 | — |
| 2026-09-15 | `32c6a47435` | genfoam: ESFR verified against upstream at 60.7 pcm; all four... | 121 | 0 | — |
| 2026-09-15 | `dd43a2e074` | genfoam: PSBT wall-friction closure verified against an upstr... | 1,410 | 0 | — |
| 2026-09-15 | `33d185cd83` | genfoam: gFHR pebble model verified against an upstream run, ... | 2,198 | 0 | — |
| 2026-09-15 | `bf153154f4` | genfoam: per-cell cross-section evaluation, closing MSFR to 7... | 8,825 | 5 | — |
| 2026-09-15 | `f26d966b15` | docs: rewrite the tutorial verification map now that upstream... | 97 | 174 | — |
| 2026-09-15 | `cb4f01f709` | liggghts: translate the LIGGGHTS contact pipeline and verify ... | 5,876 | 11 | — |
| 2026-09-15 | `a194a169ed` | outram-mc: measure Godiva's flux spectrum against OpenMC — it... | 613 | 4 | — |
| 2026-09-15 | `5bbe5fd7bf` | liggghts: bulk pebble-bed cross-check, V&V report, and maturi... | 1,336 | 42 | — |
| 2026-09-15 | `90f9e894af` | liggghts: translate the CDT rolling model, bit-identical to u... | 572 | 24 | — |
| 2026-09-15 | `69f3f242a4` | genfoam: close the MSFR case to +0.2 pcm — fix the Laplacian ... | 27,635 | 150 | — |
| 2026-09-15 | `caff1e0bbb` | liggghts: record the angle-of-repose case as attempted and un... | 35 | 1 | — |
| 2026-09-15 | `40af17de23` | outram-mc: sample the discrete inelastic angular distribution... | 1,068 | 183 | — |
| 2026-09-16 | `6d83cf5926` | outram-mc: Godiva reaches +16 +/- 11 pcm — record the result ... | 313 | 22 | — |
| 2026-09-16 | `418fff2185` | outram-mc: confirm the Godiva fix with an ablation and the k_... | 117 | 6 | — |
| 2026-09-16 | `06aaf196ea` | tampines-steam-tables: regenerate the (rho,h) flash V&V repor... | 6 | 6 | — |
| 2026-09-16 | `79c86486ca` | liggghts: fix the port divergences instead of documenting the... | 810 | 104 | — |
| 2026-09-16 | `772d47d741` | liggghts: moving walls in the verified pipeline, and an ASCII... | 9,545 | 0 | — |
| 2026-09-16 | `c0833d1320` | outram-mc: verify the Godiva physics independently of k, and ... | 995 | 13 | — |
| 2026-09-16 | `210afdbb4f` | outram-mc: HTR-10 fuel pebble from Li/Yu/Wei (2014) Table 2, ... | 1,187 | 220 | — |
| 2026-09-16 | `bc2efc173d` | outram-mc: give the ICSBEP benchmark examples seed ensembles ... | 138 | 3 | — |
| 2026-09-16 | `38466b8476` | liggghts: document the SI-only simplification, and mesh neare... | 1,537 | 15 | — |
| 2026-09-16 | `e31de690c8` | outram-mc: lift the HTR-10 Table 2 composition into the libra... | 542 | 175 | — |
| 2026-09-16 | `9c3639a5f1` | outram-mc: record the measured HTR-10 boron ablation — the gr... | 40 | 0 | — |
| 2026-09-16 | `80963b93b6` | nee_soon: HTR-10 code-to-code verification against the RMC paper | 676 | 1 | — |
| 2026-09-16 | `ad8e99cf4b` | liggghts: angle of repose, done upstream's way and verified -... | 341 | 40 | — |
| 2026-09-16 | `9ed8daf633` | outram-mc: fix two defects in the benchmark seed-ensemble patch | 16 | 4 | — |
| 2026-09-16 | `7891012195` | Gate tests over 5 minutes behind a default-on long-tests feature | 163 | 7 | — |
| 2026-09-16 | `98e47e58ca` | neutronics: read the MF=6 continuum angular law, and two gate... | 2,114 | 151 | — |
| 2026-09-16 | `a130844d4d` | neutronics: record the MF=6 angular work in both crates' CLAU... | 393 | 0 | — |
| 2026-09-16 | `b737cbb8e5` | njoy: the LRF=7 doc still said the reconstruction was defective | 30 | 13 | — |
| 2026-09-16 | `3250f4bd7a` | Gate the liggghts bulk-packing case too, on measured rather t... | 37 | 14 | — |
| 2026-09-16 | `f1ec77413d` | outram-mc: V&V record for the MF=6 continuum angular law | 201 | 0 | — |
| 2026-09-16 | `50b2d1d442` | outram-mc: price the continuum angular law on Godiva -- a bou... | 85 | 13 | — |
| 2026-09-16 | `2a8bf8a6a8` | neutronics: sample Kalbach-Mann (LANG=2), closing the MF=6 re... | 655 | 45 | — |
| 2026-09-16 | `96a1100d50` | outram-mc: measure whether the continuum angular law hardens ... | 384 | 0 | — |
| 2026-09-16 | `7eba5201b3` | neutronics: the 128-seed price, a failed prediction, and a br... | 509 | 104 | — |
| 2026-09-16 | `d3ba638938` | outram-mc: build_data.py could never have run as committed | 24 | 4 | — |
| 2026-09-16 | `f26f535a94` | outram-mc: op-os8x re-measured on HEAD and LOCALISED in energy | 113 | 0 | — |
| 2026-09-16 | `6d3d0e58a9` | neutronics: a systematic physics-coverage survey, and the con... | 445 | 0 | — |
| 2026-09-16 | `b774d3242c` | outram-mc: free-gas target motion is ablatable in-process (co... | 392 | 25 | — |
| 2026-09-16 | `5b6df9b070` | outram-mc: the fission source is ablatable -- nu-bar and chi ... | 621 | 12 | — |
| 2026-09-16 | `f09bc4d2e8` | docs: revise the heavy-run hand-off for the four new ablation... | 157 | 13 | — |
| 2026-09-16 | `fc2c234c6e` | outram-mc: (n,2n) yield is ablatable -- and the control caugh... | 384 | 38 | — |
| 2026-09-16 | `245778b84c` | docs: add the (n,2n) yield job to the heavy-run hand-off | 46 | 6 | — |
| 2026-09-16 | `5f1cf6f3c2` | outram-mc: op-os8x's leading suspect is EXCLUDED -- the MT=91... | 469 | 3 | — |
| 2026-09-16 | `d716ab58fd` | outram-mc: drivers for the fission-source and target-motion a... | 1,523 | 0 | — |
| 2026-09-16 | `64eda5fa6b` | outram-mc: exclude the continuum sampler from op-os8x -- and ... | 610 | 94 | — |
| 2026-09-16 | `4723d974ef` | njoy: PURR probability tables verified against NJOY2016 (U-238) | 2,236 | 0 | — |
| 2026-09-16 | `318223a7ff` | outram-mc: wire URR probability tables into transport | 795 | 11 | — |
| 2026-09-16 | `fc8c5031a9` | outram-mc: (n,3n) had no branch at all, and a source sampler ... | 438 | 30 | — |
| 2026-09-16 | `20f33c2765` | outram-mc: implement DBRC, and fix a control that was passing... | 604 | 62 | — |
| 2026-09-16 | `678937391f` | raffles: wire up burn as the no_std PyTorch replacement, behi... | 663 | 10 | — |
| 2026-09-16 | `9974caecd3` | Gate the remaining 38 tests over 5 minutes, each on its own m... | 174 | 1 | — |
| 2026-09-16 | `6cd9eb8457` | raffles: Bayesian model updating — TMCMC and TEMCMC, verified... | 2,302 | 5 | — |
| 2026-09-16 | `1fb68e6661` | raffles: distance metrics and Approximate Bayesian Computation | 1,829 | 1 | — |
| 2026-09-16 | `767dc4bc20` | raffles: imprecise probability — confidence boxes and coheren... | 1,125 | 1 | — |
| 2026-09-16 | `3546351b9c` | raffles: graph neural networks — message passing in burn, and... | 1,394 | 0 | — |
| 2026-09-16 | `0d1caef0ff` | raffles: TMCMC-II tempering criterion and evidence-based mode... | 771 | 24 | — |
| 2026-09-16 | `163bd84f90` | raffles: surrogate models — polynomial regression and a burn-... | 1,452 | 8 | — |
| 2026-09-16 | `7454f12df6` | raffles: bring the README and CLAUDE.md status sections in li... | 46 | 9 | — |
| 2026-09-16 | `57c622f774` | gnn: finish the MPNN port with training and rollout, and wire... | 1,412 | 14 | — |
| 2026-09-16 | `b448577d0e` | raffles: port the GPL-3.0 tutorial case studies, including a ... | 343 | 0 | — |
| 2026-09-16 | `c260aff15f` | gnn: read the upstream's own .pt datasets, and measure all fi... | 1,416 | 8 | — |
| 2026-09-16 | `c74f0b1eb9` | outram-mc: depletion collapses one-group data against a spect... | 287 | 11 | — |
| 2026-09-16 | `2a0fb215fe` | njoy: the MF=6 incident-energy interpolation flag, measured a... | 190 | 5 | — |
| 2026-09-16 | `46a9bf90be` | raffles: correct the licence record for Adolphus Lye's reposi... | 121 | 43 | — |
| 2026-09-16 | `6a8f42b668` | outram-mc: LCT-008 re-run -- the thermal residual is GONE | 87 | 0 | — |
| 2026-09-16 | `b56be59928` | outram-mc: price DBRC and URR on LCT-008 -- both bounded, nei... | 109 | 5 | — |
| 2026-09-16 | `82fdedb937` | njoy: MF=6 LAW=6 phase space, converted and verified against ... | 308 | 4 | — |
| 2026-09-16 | `0b38a09996` | njoy: LANG 11-15 and MF=5 LF measured -- unused, now enforced... | 146 | 2 | — |
| 2026-09-16 | `bfab39a2b8` | raffles: cross-check the Bayesian modules against Adolphus Ly... | 858 | 5 | — |
| 2026-09-16 | `8378b1f7bb` | Add outram-park-fork-cyclus: no_std Rust port of CYCLUS + CYC... | 12,861 | 0 | — |
| 2026-09-17 | `bdeb1ce44a` | Complete the cyclus kernel: exchange, decay, toolkit, agents,... | 17,398 | 89 | — |
| 2026-09-17 | `36ce18e37a` | liggghts: radial and axial porosity map of a settled bed, as ... | 861 | 0 | — |
| 2026-09-17 | `5cce18d303` | njoy: MF=6 LAW=7 -- fix two parser defects, then wire the sam... | 1,287 | 53 | — |
| 2026-09-17 | `e416382890` | Add cavity grid-convergence test: 20/40/80, observed order an... | 296 | 0 | 432,337,826 |
| 2026-09-17 | `9491abee5e` | outram-mc: V&V record for the fission-source and target-motio... | 192 | 0 | — |
| 2026-09-17 | `807e9e2310` | outram-mc: record the job 4 and job 5 results in the drivers'... | 79 | 0 | — |
| 2026-09-17 | `3f5e9f51db` | njoy+mc: wire MF=4/MF=5 emission laws, and fix a mu-bar quadr... | 1,808 | 164 | — |
| 2026-09-17 | `bc5516ad06` | mc+njoy: work the four open items -- price ANGLE_TOL, land 8 ... | 2,961 | 78 | — |
| 2026-09-17 | `47d6081583` | TUAS thermophysical properties: no production panics, and an ... | 265 | 121 | 328,382,515 |
| 2026-09-17 | `f6aabcea58` | htgr_sim_v1: published HTR-10 kinetics, a trippable circulato... | 628 | 137 | 0 |
| 2026-09-17 | `60f6bb2615` | outram-mc: correct the FHR ablation's verdict text -- the gap... | 45 | 3 | — |
| 2026-09-17 | `60dac13a0e` | Stale docs are defects: fix three, and make correcting them a... | 50 | 3 | 62,375,157 |
| 2026-09-17 | `9bc626e87b` | mc: wire MT=5 "(n,anything)", and exclude chi's SHAPE as an o... | 926 | 8 | — |
| 2026-09-17 | `8d2f1bd748` | htgr_sim_v1: derive the HTR-10 passive decay-heat chain from ... | 1,597 | 34 | 182,540,421 |
| 2026-09-17 | `46d41106a8` | mc: exclude the cross sections BAND BY BAND, and show the op-... | 561 | 0 | — |
| 2026-09-17 | `dd8b111385` | outram-mc: port OpenMC's Shannon entropy, and fix a mesh boun... | 256 | 9 | 15,258,361 |
| 2026-09-17 | `a4e07ce173` | outram-mc: repair a semantic merge conflict in two spectrum e... | 5 | 5 | 5,399,720 |
| 2026-09-17 | `4367708125` | outram-mc: exhaustive code-to-code verification of Shannon en... | 5,658 | 1 | 8,628,803 |
| 2026-09-17 | `e570e8654b` | petir: remove the `std` and `platform-libm` features, which n... | 44 | 112 | 60,309,827 |
| 2026-09-17 | `eeaf739b03` | dhoby-ghaut: new crate, and move both GUI studios into it | 203 | 14 | 2,334,605 |
| 2026-09-17 | `05e1fe2dda` | docs: correct nine claims the code contradicts | 180 | 50 | 1,406,769 |
| 2026-09-17 | `ef7de0bf26` | boon-lay: migrate three egui examples to the current panel API | 13 | 13 | 0 |
| 2026-09-17 | `a4ae098f3c` | mc: run op-os8x's discriminating experiment -- the residual i... | 176 | 18 | — |
| 2026-09-17 | `1eb1723eb0` | outram-mc: gate depth-3 nested-lattice navigation, which noth... | 481 | 27 | 69,769,628 |
| 2026-09-17 | `9e1241327b` | outram-mc: measure what a control rod costs a global majorant... | 274 | 0 | 18,537,553 |
| 2026-09-17 | `149ba4945f` | outram-mc: exercise cell translation, and fix the frame offse... | 1,078 | 6 | — |
| 2026-09-17 | `2622b91cf9` | njoy: enumerate V&V coverage module by module, and verify HEA... | 322 | 0 | — |
| 2026-09-17 | `f5289133a5` | outram-mc: per-region tracking method and region-local majorants | 413 | 1 | 0 |
| 2026-09-17 | `ede5c51e4d` | raffles: fix a misplaced Cell field from the tracking-method ... | 1 | 1 | 10,731,245 |
| 2026-09-17 | `d08b3fc7bd` | outram-mc: bounded delta flight -- the boundary handoff for h... | 439 | 1 | 20,475,491 |
| 2026-09-17 | `bd308e9cf5` | outram-mc: region extent -- which level declared delta, and h... | 158 | 0 | 26,224,862 |
| 2026-09-17 | `213c994f5b` | liggghts: merge develop, enable gnn by default, half-stencil ... | 202 | 35 | — |
| 2026-09-17 | `31a71051f9` | liggghts: HTR-10 full-core DEM case, contact-network API, GNN... | 55,782 | 0 | — |
| 2026-09-17 | `e91738f772` | njoy: V&V gaspr against NJOY2016 -- three real defects, all f... | 4,463 | 226 | — |
| 2026-09-17 | `c6d9b5717a` | outram-mc: dispatch the transport loop on the region's tracki... | 96 | 5 | 24,839,973 |
| 2026-09-17 | `4d65204fee` | liggghts: GNN surrogate vs DEM -- a negative result, measured... | 55,509 | 55,139 | — |
| 2026-09-17 | `9557686e99` | outram-mc: report the majorant's price, and a cylindrical del... | 275 | 22 | 54,598,139 |
| 2026-09-17 | `f21b0dbfb8` | liggghts: HTR-10 V&V write-up; raise stiffness to 5e8 after t... | 89 | 16 | — |
| 2026-09-17 | `cb99d06f79` | outram-mc: the hybrid tracking equivalence gate -- Phase 1 co... | 306 | 3 | 10,983,861 |
| 2026-09-17 | `4bb0201c7d` | liggghts: correct four stale reference-data/doc claims; persi... | 68 | 14 | — |
| 2026-09-17 | `6fc94aee55` | outram-mc: adjudicate the HTR-10 TRISO radii, and settle the ... | 262 | 22 | 24,118,065 |
| 2026-09-17 | `b5b17c2cc5` | liggghts: commit the dump->CSV converters the reference data ... | 67 | 3 | — |
| 2026-09-17 | `177b32bbc8` | liggghts: regenerate the HTR-10 LIGGGHTS reference at E = 5e8... | 55,117 | 55,122 | — |
| 2026-09-17 | `b192b40c55` | outram-mc: cubic TRISO array in the fuel zone -- and 8335 exa... | 229 | 0 | 13,981,882 |
| 2026-09-17 | `e090d230c5` | outram-mc: resolve the conus blocker with per-tile surface tr... | 205 | 0 | 10,033,028 |
| 2026-09-17 | `6d14612542` | outram-mc: wire Shannon entropy into the k-eigenvalue driver | 244 | 6 | 17,806,109 |
| 2026-09-17 | `4589165cd0` | nee_soon: hex-prism bed tile assignment with the paper's 57:4... | 172 | 0 | 8,600,980 |
| 2026-09-17 | `08fc0e9f8f` | kovan-literature: catalogue the HTR-10 RMC benchmark paper (p... | 19 | 0 | 15,632,453 |
| 2026-09-17 | `c2d0defed4` | nee_soon: HTR-10 reflector zone compositions from IAEA-TECDOC... | 211 | 0 | 9,652,611 |
| 2026-09-17 | `4085c7773e` | nee_soon: HTR-10 geometry-integrity gates in the default suite | 123 | 0 | 8,868,454 |
| 2026-09-17 | `edecbbaf52` | nee_soon: assemble the HTR-10 core, and measure that scale is... | 285 | 4 | 12,573,543 |
| 2026-09-17 | `82829a6fa8` | outram-mc: V&V record for the HTR-10 RMC comparison | 82 | 0 | 2,715,800 |
| 2026-09-17 | `8239c42d8f` | nee_soon: attempt the HTR-10 eigenvalue -- it fails, and here... | 459 | 9 | 12,838,565 |
| 2026-09-17 | `835502316d` | nee_soon: rule the geometry OUT of the HTR-10 k = 0 failure | 82 | 1 | 5,579,316 |
| 2026-09-17 | `a93cae3798` | nee_soon: clear the hybrid tracker of the HTR-10 k = 0 failure | 36 | 5 | 6,558,551 |
| 2026-09-17 | `5504ce8265` | nee_soon: clear the MATERIALS too -- three eliminations, k st... | 42 | 1 | 7,553,541 |
| 2026-09-17 | `bf64272dc0` | liggghts: consolidated cross-code summary; correct two self-c... | 204 | 7 | — |
| 2026-09-17 | `45fe3f2bea` | mc: subtract the charged-particle channels from MT=27 to get ... | 29 | 4 | — |
| 2026-09-17 | `1db4e166b5` | liggghts: HTR-10 at E = 5e8 passes; per-particle agreement is... | 27,696 | 17 | — |
| 2026-09-17 | `763fcb131c` | nee_soon: localise the HTR-10 k = 0 failure to a stuck crossi... | 54 | 0 | 23,873,722 |
| 2026-09-17 | `5ee6b41106` | nee_soon: CONFIRM the HTR-10 k = 0 defect -- the nested TRISO... | 18 | 3 | 5,786,454 |
| 2026-09-17 | `f3ab8a6825` | liggghts: key shear history by stable tag, as upstream does; ... | 244 | 12 | — |
| 2026-09-17 | `c3cea97a36` | nee_soon: fix the TRISO lattice construction, and CORRECT a w... | 49 | 3 | 25,313,845 |
| 2026-09-17 | `de8450e9a0` | nee_soon: find half the missing fuel exactly -- one ball per ... | 40 | 0 | 10,845,155 |
| 2026-09-17 | `b931c6da1b` | liggghts: HTR-10 slow pebble recirculation case -- geometry a... | 3,782 | 0 | — |
| 2026-09-17 | `04cec9a72d` | njoy: cross-check HEATR against NJOY2016 -- KERMA and the unt... | 59,410 | 7 | — |
| 2026-09-17 | `0c81e1b2d9` | njoy: run GAMINR against a REAL photoatomic evaluation, not j... | 483 | 24 | — |
| 2026-09-17 | `ac4ed77ec0` | Fix HexLattice axial frame; HTR-10 core goes from k=0 to 0.857 | 999 | 100 | 76,980,263 |
| 2026-09-17 | `a9f2bf9d3c` | mc: op-os8x's leading suspect is measured and REJECTED -- on ... | 257 | 42 | — |
| 2026-09-17 | `599d9d2bfa` | docs: record the cell-translation regression result | 15 | 0 | — |
| 2026-09-17 | `fbc6695058` | HTR-10 core reaches -909 +/- 108 pcm against the RMC benchmark | 531 | 133 | 47,366,130 |
| 2026-09-18 | `7fb86bf58f` | liggghts: HTR-10 finished -- 0.61 reached, and it was frictio... | 311,496 | 118 | 0 |
| 2026-09-18 | `aa56117036` | CLAUDE.md: HARD RULE — get the process right first, the answe... | 63 | 0 | 18,550,235 |
| 2026-09-17 | `a78bae37ff` | CLAUDE.md: type-check in release too, keeping one build tree | 26 | 2 | — |
| 2026-09-17 | `dc92a6e656` | CLAUDE.md: document dhoby-ghaut, correct two stale crate counts | 4 | 3 | — |
| 2026-09-17 | `87237300fb` | tampines-steam-tables: regenerate the (rho,h) flash V&V repor... | 6 | 6 | — |
| 2026-09-18 | `0ee30a153e` | tampines-steam-tables: export per-point Chebyshev backward re... | 202 | 0 | 172,039,796 |
| 2026-09-18 | `96a6323068` | Pool the ICSBEP benchmarks; correct six stale copies; HTR-10 ... | 825 | 103 | 150,890,886 |
| 2026-09-18 | `cd142f9e07` | CI: fast tests on five platforms; fix the Android gui gate it... | 542 | 16 | 59,496,309 |
| 2026-09-18 | `9c5d04355c` | outram-mc: fix duplicate FilterKind import breaking two Godiv... | 2 | 2 | 362,682,528 |
| 2026-09-18 | `be6b8c3f83` | outram-mc: chase CLS and SCLS to correct implementations, wit... | 15,053 | 3,534 | 757,936 |
| 2026-09-18 | `8083ec6f68` | Gate BLAS behind an opt-in feature; exclude docs/ from 17 mor... | 89 | 24 | 48,526,747 |
| 2026-09-18 | `8f73c831a5` | Add SEMBAWANG and REDHILL placeholders; stop CI cancelling it... | 230 | 2 | 15,525,830 |
| 2026-09-18 | `03ba4cc327` | liggghts: the repose gap halves -- 12.78 deg was a nondetermi... | 27,731 | 27,559 | 24,715,946 |
| 2026-09-18 | `3ad50bec16` | Fix the three test failures CI found on Linux, macOS and Windows | 150 | 25 | 143,453,282 |
| 2026-09-18 | `26b595cc00` | reference-data: add ENDF/B-VII.0, the library the HTR-10 refe... | 396,521 | 0 | 186,703,073 |
| 2026-09-18 | `2210027c77` | outram-mc/nee_soon: the HTR-10 residual was the DATA LIBRARY,... | 468 | 16 | 52,133,997 |
| 2026-09-18 | `c807233d8a` | liggghts: fill the HTR-10 friction response curve across the ... | 137,998 | 2 | 2,001,720 |
| 2026-09-19 | `d9cb5b161d` | outram-mc: outgoing-energy filter, so a scattering matrix is ... | 1,148 | 334 | — |
| 2026-09-19 | `367535155d` | outram-mc: scattering matrix is now measured, and it conserve... | 463 | 2 | — |
| 2026-09-19 | `f3512b65a2` | nee_soon: MGXS condensed from Monte Carlo, checked against ne... | 2,090 | 269 | — |
| 2026-09-19 | `8e0c7e91cd` | outram-mc/nee_soon: the fission spectrum is measured, not ass... | 189 | 12 | — |
| 2026-09-19 | `fc53cd3cfb` | nee_soon: GeN-Foam reproduces the Monte Carlo eigenvalue from... | 722 | 0 | — |
| 2026-09-19 | `b1d8974e26` | nee_soon: one coupling API, and the HTR-10 core carried throu... | 972 | 5 | — |
| 2026-09-19 | `cf7086296b` | nee_soon: direct Monte Carlo <-> GeN-Foam coupling, iterated ... | 706 | 0 | — |
| 2026-09-19 | `83011650f9` | nee_soon: Haiku dogfoods the coupling API; correct the stale ... | 216 | 8 | — |
| 2026-09-19 | `cbed624d32` | outram-mc: restore FilterKind -- two fixes for one bug cancel... | 2 | 2 | 125,492,211 |
| 2026-09-19 | `4b9a9a52d1` | petir: WGSL f32 kernels for poly, Chebyshev and Legendre, ver... | 1,738 | 6 | — |
| 2026-09-19 | `0362fdaeaa` | petir: add the `wgpu` feature properly, with the std gate op-... | 508 | 356 | — |
| 2026-09-19 | `7dd5a76a66` | petir: use try_into in the GPU readback, so the no-panic gate... | 7 | 1 | — |
| 2026-09-19 | `276f2ec8e7` | petir: GSL's error-function family in WGSL, generated from th... | 734 | 12 | — |
| 2026-09-19 | `6f50f68101` | petir: keep CI GPU-independent -- move shader validation out ... | 226 | 80 | — |
| 2026-09-19 | `e5dfb26e71` | outram-mc: cross-lineage GPU root-finder check, and a test th... | 476 | 1 | — |
| 2026-09-19 | `e16c2c72e8` | petir: GSL matrices and CBLAS in WGSL, plus a coverage ledger... | 1,308 | 7 | — |
| 2026-09-19 | `46577a2d9e` | outram-mc: verify the GPU scattering kinematics against close... | 498 | 0 | — |
| 2026-09-19 | `5a0143c443` | petir: GSL's gamma family in WGSL, and a prediction the measu... | 678 | 6 | — |
| 2026-09-19 | `82a6494c32` | petir: GSL's order-0/1 Bessel family, with every coefficient ... | 2,021 | 0 | — |
| 2026-09-19 | `a661a87d67` | petir: GSL's Bessel family in WGSL, and a second prediction t... | 1,835 | 9 | — |
| 2026-09-19 | `e0fd882d06` | petir: GSL's digamma and zeta families, and a 24th coefficien... | 2,503 | 137 | — |
| 2026-09-19 | `2ee1fb72bc` | petir: psi and zeta in WGSL, and a test that was quietly chec... | 1,402 | 39 | — |
| 2026-09-19 | `0bf1f6857e` | outram-mc: verify the GPU helpers directly, and close an unen... | 1,623 | 2 | — |
| 2026-09-19 | `84cd3c8572` | petir: the all-features sweep runs in release, per the single... | 5 | 2 | — |
| 2026-09-19 | `7b69158607` | everything builds in release: the cross-targets were the bigg... | 92 | 37 | — |
| 2026-09-19 | `bbd28e14e6` | kovan: historian and tokens live on kovan-cli, not on the GUI... | 1,598 | 17 | — |
| 2026-09-19 | `57ca030f54` | petir: GSL's Debye functions, and three of my own errors the ... | 766 | 4 | — |
| 2026-09-19 | `35bc657a33` | petir/wgsl: Debye D_1..D_6 kernel, f32 mirror, GPU verification | 1,210 | 9 | — |
| 2026-09-19 | `b8e39a6ff2` | petir: dilog Li_2(x) — f64 port, WGSL kernel, f32 mirror, GPU... | 1,299 | 8 | — |
| 2026-09-19 | `c5ffbd0a1b` | ICSBEP: the U-238 evaluation is NOT the residual -- predictio... | 143 | 3 | 99,055,481 |
| 2026-09-19 | `7e339f4c89` | ICSBEP: U-238 inelastic angular also eliminated -- and the sc... | 90 | 0 | 33,990,641 |
| 2026-09-19 | `9c9bb630a4` | outram-mc gpu: verify lcg_advance; correct both shaders' RNG ... | 598 | 8 | — |
| 2026-09-19 | `a15f969a3c` | outram-mc: refute the nu-bar ablation, adopt the 100 pcm crit... | 152 | 17 | 35,108,610 |
| 2026-09-19 | `160ac16b5c` | petir: Airy Ai/Bi and the scaled forms, ported from GSL specf... | 968 | 5 | — |
| 2026-09-19 | `c33e0012ee` | petir/wgsl: Airy Ai/Bi kernel, f32 mirror, GPU verification | 1,336 | 8 | — |
| 2026-09-19 | `060f0ab0fe` | petir: Lambert W, and the audit gap AUDITED_ELSEWHERE was hiding | 666 | 3 | — |
| 2026-09-19 | `dab9b27e3f` | petir: Clausen Cl_2, and the extended-precision angle reducti... | 697 | 8 | — |
| 2026-09-19 | `4dc5fdcc8a` | petir/wgsl: Lambert W — upstream's stopping rule is wrong twi... | 826 | 10 | — |
| 2026-09-19 | `fe00db575f` | petir/wgsl: Clausen Cl_2, with an f32-redesigned argument red... | 627 | 9 | — |
| 2026-09-19 | `d0b8e1a445` | petir: transport integrals J(2..5), the Bloch-Gruneisen family | 582 | 8 | — |
| 2026-09-19 | `b2d3058da6` | petir/wgsl: transport integrals J(2..5) — a sixth kind of con... | 752 | 10 | — |
| 2026-09-19 | `af00b494bd` | petir: inverse-tangent integral Ti_2, f64 and WGSL | 840 | 14 | — |
| 2026-09-19 | `9f9f98cf51` | petir: synchrotron S_1 and S_2; upstream's underflow guard is... | 541 | 4 | — |
| 2026-09-19 | `bfb037ba5b` | petir/wgsl: synchrotron S_1 and S_2; a guard whose CATEGORY d... | 1,354 | 61 | — |
| 2026-09-19 | `2c74ec3683` | petir: complete Fermi-Dirac integrals F_j, and four tests tha... | 1,404 | 8 | — |
| 2026-09-19 | `5b54357d38` | petir: fd_whiz is unreachable, and GSL's F_2 far-field cut is... | 260 | 1 | — |
| 2026-09-19 | `4598222c23` | petir/wgsl: the Fermi-Dirac integrals, and two kinds of const... | 2,124 | 10 | — |
| 2026-09-19 | `471297408b` | petir: Dawson, Ei_3, Si and Ci, and an audit that had been as... | 1,618 | 66 | — |
| 2026-09-20 | `cb424374e4` | nee_soon: SP3 verified against diffusion, and two HTR-10 boun... | 693 | 14 | 324,345,569 |
| 2026-09-19 | `25ddd87ca6` | petir/wgsl: Dawson, and GSL's single-precision order that eve... | 872 | 13 | — |
| 2026-09-19 | `e395771d48` | petir/wgsl: Ei_3 and Si/Ci, and a reduction that looked obvio... | 1,580 | 11 | — |
| 2026-09-19 | `0c2c2b96e9` | outram-mc: verify continuum_inelastic on the GPU against an i... | 363 | 4 | — |
| 2026-09-19 | `d9f6f943bb` | outram-mc: verify the five primitives under surface_distance'... | 473 | 0 | — |
| 2026-09-19 | `c3da4a0d4a` | petir/wgsl: ship GSL's single-precision Chebyshev order every... | 444 | 456 | — |
| 2026-09-19 | `9b87c2986a` | petir: elliptic integrals, and a third kind of single-precisi... | 998 | 1 | — |
| 2026-09-19 | `b8bc8ee5e5` | petir/wgsl: elliptic integrals, and four shaders nothing was ... | 1,600 | 14 | — |
| 2026-09-19 | `f83bd16e5a` | petir/wgsl: the inline-coefficient effect is constant folding... | 298 | 8 | — |
| 2026-09-20 | `f8dbb49512` | outram-mc: correct physics is the DEFAULT, not an opt-in | 464 | 3 | 81,179,178 |
| 2026-09-19 | `9d2738b772` | petir/docs: the ledger's own prose named four ported blocks a... | 15 | 6 | — |
| 2026-09-19 | `2457e2ffa2` | petir: eight assertion messages rendered with runs of blank s... | 28 | 12 | — |
| 2026-09-19 | `8a76e44e52` | petir: Ei, Shi and Chi -- and two more registries nothing was... | 1,811 | 39 | — |
| 2026-09-19 | `6bc996603f` | petir: Jacobi elliptic functions, the inverse of the integral... | 1,401 | 18 | — |
| 2026-09-19 | `28309e6de5` | petir: Gegenbauer polynomials, bit-identical on the GPU at ev... | 1,008 | 9 | — |
| 2026-09-19 | `279afd2913` | petir: associated Legendre P_l^m, and an upstream guard hole ... | 1,000 | 10 | — |
| 2026-09-19 | `a9cb4de02d` | petir/docs: Coulomb is measured NOT GPU-shaped, so no shader ... | 11 | 0 | — |
| 2026-09-19 | `f94227fb5b` | petir: Overflow and Underflow variants, and eleven sites that... | 499 | 33 | — |
| 2026-09-20 | `bb8b7482af` | outram-mc: complete the LCT-008 nuclide set, in a cheap and a... | 1,442,766 | 8 | 224,042,648 |
| 2026-09-20 | `2184da720e` | outram-mc + nee_soon: fix two P0 tally/condensation defects f... | 2,211 | 34 | 519,942 |
| 2026-09-19 | `37ae4ce808` | outram-blender: fan triangulation was wrong on concave faces ... | 1,116 | 17 | — |
| 2026-09-19 | `4ddfff21a0` | outram-blender: port BLI_polyfill_beautify, and make both Bea... | 1,029 | 16 | — |
| 2026-09-19 | `9f272310ab` | outram-blender: port the two face-planarity operators, and me... | 1,167 | 0 | — |
| 2026-09-19 | `a455efa3bf` | outram-blender: one definition of the quad diagonal rule, not... | 139 | 24 | — |
| 2026-09-19 | `38ae132abe` | outram-blender: the upstream coverage matrix, so the gaps are... | 204 | 0 | — |
| 2026-09-20 | `e51277588e` | outram-blender: port limited dissolve — the old one lost 23.7... | 830 | 28 | — |
| 2026-09-20 | `29543cf099` | outram-blender: port concave-face splitting, completing the f... | 512 | 10 | — |
| 2026-09-20 | `c370eb1bf7` | outram-blender: poke was using the wrong centre mode — 2.4 un... | 201 | 8 | — |
| 2026-09-20 | `e93058d304` | dhoby-ghaut: give Mesh Studio a headless mode — it immediatel... | 585 | 106 | — |
| 2026-09-20 | `cbc582e477` | dhoby-ghaut: MC Studio gets a headless mode too — both studio... | 411 | 77 | — |
| 2026-09-20 | `9a6a04f4ed` | outram-blender: recalc-normals inverted a translated open sur... | 372 | 11 | — |
| 2026-09-20 | `2b6fa8836f` | transport: collision partition uses the material's own temper... | 180 | 45 | 38,052,208 |
| 2026-09-20 | `5a1410e954` | outram-blender: a feature-gated test had been red on develop,... | 51 | 8 | — |
| 2026-09-20 | `45c21a9333` | njoy+mc: reference ACE for OpenMC, and this port measured aga... | 659 | 0 | — |
| 2026-09-20 | `1ae780cd4d` | docs: record the maintainer correction "a reactor core is not... | 78 | 0 | 45,349,487 |
| 2026-09-20 | `26cd692eed` | reference-data: track the NJOY2016 ACE set as a submodule | 46 | 3 | — |
| 2026-09-20 | `e51c6a6b51` | njoy: stamp generator, version, commit and date into every AC... | 157 | 7 | — |
| 2026-09-20 | `88ed5e8ebc` | njoy: gate ACE generation against NJOY2016 upstream in the te... | 2,745 | 0 | — |
| 2026-09-20 | `37ba019882` | mc: take ablation thread counts from the machine; correct fou... | 152 | 20 | — |
| 2026-09-20 | `478455296f` | docs: record that nohup does not survive an agent turn in the... | 12 | 0 | — |
| 2026-09-20 | `c60d71d448` | mc: job 1 at 400 seeds -- continuum angular law is -16 +/- 13... | 101 | 7 | — |
| 2026-09-20 | `5d54dba6ad` | njoy/acer: store the 37 charged-particle level reactions, mat... | 165 | 23 | — |
| 2026-09-20 | `7f425b5858` | njoy/acer: write the ACE NU block -- bit-identical to NJOY201... | 662 | 30 | — |
| 2026-09-20 | `d8da81881c` | njoy/acer: follow upstream's mt19 fission rule; extend parity... | 427 | 42 | — |
| 2026-09-20 | `3f9cdb7291` | CLAUDE.md: op-sdbk's thnmax guard is ported; record what that... | 18 | 2 | — |
| 2026-09-20 | `bdd86b6a94` | njoy/acer: write fission secondaries (chi) -- NXS(5) now matc... | 82 | 8 | — |
| 2026-09-20 | `5745cf2abe` | njoy/acer: scope photon production rather than half-build it | 30 | 1 | — |
| 2026-09-20 | `717f262423` | njoy/acer: verify Doppler broadening against NJOY; guard the ... | 289 | 7 | — |
| 2026-09-20 | `c5579db7bb` | njoy/acer: gate Doppler broadening against NJOY2016 in the or... | 462 | 47 | — |
| 2026-09-20 | `9aae7d3bac` | mc: re-baseline jobs 4 and 5 on HEAD -- URR/DBRC moved none o... | 62 | 1 | — |
| 2026-09-20 | `c670aaa86c` | njoy/acer: write the photon-production blocks -- verified 6/6... | 565 | 10 | — |
| 2026-09-20 | `759aa3ab12` | njoy/acer: close photon production -- NTRP and MTRP match NJO... | 885 | 43 | — |
| 2026-09-20 | `01751a1965` | njoy/acer: let the comparator take any tape, and emit one mac... | 49 | 8 | — |
| 2026-09-20 | `ac994b32f2` | docs(njoy): correct two stale claims in the ACER-vs-NJOY2016 ... | 16 | 3 | — |
| 2026-09-20 | `3f7bb50d96` | fix(acer): exclude MT=460 delayed photon data from the photon... | 24 | 0 | — |
| 2026-09-20 | `c7656653cb` | fix(acer): key the MT=4 keep/drop on MF=12/MT=4, as upstream ... | 403 | 37 | — |
| 2026-09-20 | `9e64be4ae1` | fix(acer): MT=4 keep rule must test MF=12 only, never MF=13 | 1,081 | 379 | — |
| 2026-09-20 | `9ae8a85da9` | feat(acer): thermal S(a,b) ACE comparator, and all 70 ENDF ta... | 614 | 7 | — |
| 2026-09-20 | `658baf2f8b` | docs(njoy): Fe-57 tolerance ladder, and the final 70-tape tally | 48 | 1 | — |
| 2026-09-20 | `937b804f0f` | test(acer): gate the thermal S(a,b) table against NJOY2016 on... | 626 | 0 | — |
| 2026-09-20 | `738460b0cd` | test(acer): make the thermal gate honour OUTRAM_PARK_REQUIRE_... | 17 | 3 | — |
| 2026-09-20 | `3e2974e755` | ci: short tests on develop, long tests on main; gate the refe... | 209 | 7 | — |
| 2026-09-20 | `b40b40698b` | docs(njoy): Mo-95 OOM confirmed at tol=0.1, closing the 70th ... | 19 | 7 | — |
| 2026-09-20 | `d8b1048c35` | ci: manual full-suite runner, dispatchable and runnable from ... | 168 | 0 | — |
| 2026-09-20 | `a5bfbac230` | CLAUDE.md: document manual-tests.yml in the CI table | 30 | 0 | — |
| 2026-09-21 | `907cf68a1a` | ci: fix the Ubuntu and macOS failures -- CI has never been green | 44 | 3 | 77,508,093 |
| 2026-09-21 | `2a20445f7f` | CLAUDE.md: cut to 117k, deprecate kopi-beans, retire kovan li... | 2,801 | 1,542 | 0 |
| 2026-09-21 | `3424d2d762` | claude: drop the vestigial kopi-beans hooks from settings.json | 14 | 39 | 4,225,456 |
| 2026-09-21 | `2b260ba123` | changi: fix now-stale SEMBAWANG/REDHILL "not yet created" ref... | 12 | 7 | — |
| 2026-09-21 | `a6580baf08` | coaxial annular pipe added | 805 | 6 | 75,152,176 |
| 2026-09-21 | `fbfacbd3dd` | have new htr-10 SG | 616 | 1 | 26,477,436 |
| 2026-09-21 | `9cb3488ca1` | steam generator now has tracers | 333 | 13 | 13,864,689 |
| 2026-09-21 | `fcc8042479` | htr-10 SG now has tracers in the feedwater side | 136 | 36 | 13,560,388 |
| 2026-09-21 | `5c64e8521f` | boon-lay: correct the TRISO-ATOPS module map and record five ... | 210 | 128 | — |
| 2026-09-21 | `0ab0e5c478` | more artwork for htr-10 | 1,044 | 0 | 22,922,724 |
| 2026-09-21 | `bf2f6d2568` | u shaped bend at bottom of htr-10 | 163 | 21 | 6,899,818 |
| 2026-09-21 | `f2a09a1b10` | boon-lay: close every TRISO-ATOPS porting gap except the GUI | 12,040 | 1,305 | — |
| 2026-09-21 | `ec446d6cd2` | htr-10 at least to scale now | 575 | 292 | 26,484,688 |
| 2026-09-21 | `04a4515793` | boon-lay: third code-to-code pass — end-to-end accident_case,... | 3,964 | 189 | — |
| 2026-09-21 | `1bad312fd5` | updated htr-10 schematic | 282 | 62 | 39,296,114 |
| 2026-09-21 | `43533f49c1` | more on the schematics, looks better now | 30 | 29 | 4,827,902 |
| 2026-09-21 | `8f9c69d1d4` | ci: unblock fast tests -- raise the ceiling, gate two Windows... | 40 | 1 | 34,126,702 |
| 2026-09-21 | `dee7eabd7a` | changi: port the Gaussian puff model from Hammerling-Research... | 13,868 | 22 | — |
| 2026-09-21 | `72011230a9` | changi: add the puff example, and correct an overclaim it exp... | 383 | 0 | — |
| 2026-09-21 | `98e7c0586c` | bedok: route the nodal sinh/cosh through petir -- the last ul... | 306 | 24 | 80,766,321 |
| 2026-09-21 | `8f5160cc39` | changi: unit-release chi/Q dilution factors, binned by travel... | 1,243 | 9 | 101,366,997 |
| 2026-09-21 | `3126ab5bcd` | changi: deposition grouping, source-term types, and the surve... | 1,275 | 1 | 9,777,018 |
| 2026-09-21 | `7d2d8d3eb1` | boon-lay + redhill: two doc defects and one piece of lost his... | 84 | 3 | 4,159,211 |
| 2026-09-21 | `103aac269c` | sembawang: crate shell, the units boundary, and VentingWindow... | 706 | 15 | 7,903,002 |
| 2026-09-21 | `d15a4150b9` | author writte: i need MQTT also | 4 | 0 | 14,605,455 |
| 2026-09-21 | `9035e1f230` | sembawang: the accident driver, inventory and transient input... | 1,486 | 0 | 5,802,777 |
| 2026-09-21 | `6f60ade8e6` | docs: record the source-term chain in CLAUDE.md, sembawang RE... | 90 | 2 | 6,243,359 |
| 2026-09-21 | `0b64120534` | docs: trim workspace CLAUDE.md — drop KOPITIAM dogfooding, me... | 314 | 295 | 5,924,103 |
| 2026-09-21 | `3d9b99af21` | changi + sembawang: release examples in Bq/Ci, prescribed cha... | 393 | 3 | 8,645,127 |
| 2026-09-21 | `2357b55cac` | docs: SEMBAWANG and BOON LAY backronyms, BEDOK citation, boon... | 139 | 9 | 20,361,411 |
| 2026-09-21 | `1228877a8f` | docs: split the workspace CLAUDE.md — physics rules in full, ... | 1,573 | 1,477 | 4,415,246 |
| 2026-09-21 | `8a84cb4238` | sembawang: one run from TRISO release through changi dispersi... | 618 | 0 | 10,682,440 |
| 2026-09-21 | `8255082271` | docs: sembawang is the orchestrator of the offsite chain (#235) | 66 | 17 | 8,418,597 |
| 2026-09-21 | `9f4ebc7b57` | added new tab in studio | 118 | 14 | 15,490,057 |
| 2026-09-21 | `d11dedb186` | added new widget tab | 97 | 8 | 10,082,058 |
| 2026-09-21 | `2f753516bc` | added refuelling chute | 447 | 57 | 72,282,587 |
| 2026-09-21 | `55980c3b97` | SG and RPV in same tab | 806 | 72 | 97,254,562 |
| 2026-09-21 | `46bdf6ccc8` | htr-10 sim looks not too bad for prototype | 57 | 24 | 9,061,075 |
| 2026-09-21 | `ee7df8cf82` | raffles: port SCRAM's fault-tree quantification, verified aga... | 1,815 | 13 | — |
| 2026-09-21 | `d24b8e0afb` | raffles: rustfmt the crate (formatting only, no behaviour cha... | 217 | 231 | — |
| 2026-09-21 | `ecf014c14c` | raffles: generate minimal cut sets, verified end to end again... | 11,920 | 1,381 | — |
| 2026-09-21 | `99d4ac4d40` | raffles: deepen the SCRAM fixture ~9x and rank generated cut ... | 785 | 110 | — |
| 2026-09-21 | `f64620c9b9` | raffles: correct a wrong claim about SCRAM's probability cut-off | 84 | 25 | — |
| 2026-09-21 | `c7dbb28545` | feat(acer): read ACE Type 1 and Type 2; exhaustive S(a,b) ver... | 1,175 | 76 | — |
| 2026-09-21 | `62801790e6` | raffles: handle non-coherent fault trees, verified against co... | 5,728 | 116 | — |
| 2026-09-21 | `90ae70d73a` | test(acer): read every ACE class obtainable from this repository | 84 | 4 | — |
| 2026-09-21 | `aec680a6ae` | fix(acer): the ACE reader must not turn a corrupt token into ... | 46 | 6 | — |
| 2026-09-21 | `6cd78c5b20` | refactor(acer): last three ad-hoc ACE parsers now go through ... | 37 | 54 | — |
| 2026-09-21 | `6d425b148f` | raffles: add a BDD, removing the exact-probability cut-set ce... | 1,854 | 34 | — |
| 2026-09-21 | `afae166c5d` | raffles: add a ZBDD, so cut sets scale past what MOCUS can reach | 801 | 28 | — |
| 2026-09-21 | `42c361576b` | feat(acer): write ACE Type 1 and Type 2 — container parity wi... | 331 | 8 | — |
| 2026-09-21 | `fe13aa1e2b` | raffles: add prime implicants, the last algorithmic gap in th... | 1,691 | 61 | — |
| 2026-09-21 | `36d2cbaca6` | raffles: house events, upstream's own test suite, and a unity... | 3,742 | 3,117 | — |
| 2026-09-21 | `3f5e8070ab` | raffles: re-verify every recorded number against the current ... | 119 | 86 | — |
| 2026-09-21 | `2adbe5eb3a` | raffles: importance from the BDD, and a sign divergence it fo... | 480 | 2 | — |
| 2026-09-21 | `bdf3f529f8` | raffles: record the memory-bound Aralia models as a closed de... | 19 | 11 | — |
| 2026-09-22 | `83d88c4ac3` | feat(acer): port the photo-atomic and dosimetry ACE classes —... | 39,750 | 113 | — |
| 2026-09-22 | `758d256f4d` | feat(acer): thermal IFENG=1/2, mcnpx format, and byte-exact A... | 2,232 | 90 | — |
| 2026-09-22 | `0c8b044b68` | refactor(acer): one ACE serialiser, one set of Fortran edit d... | 724 | 213 | — |
| 2026-09-22 | `ee0276daba` | raffles: port SCRAM's non-BDD ZBDD constructor -- a third rou... | 605 | 37 | — |
| 2026-09-22 | `7c4d1b0572` | raffles: port SCRAM's expression library, verified on HIPPS's... | 1,648 | 0 | — |
| 2026-09-22 | `3e2a275c06` | connection to SG is better than before | 254 | 24 | 23,702,838 |
| 2026-09-22 | `7a706d8bd1` | raffles: port SCRAM's MEF reader, and unlock ThreeMotor | 6,298 | 66 | — |
| 2026-09-22 | `fc26a0c88c` | feat(acer): rebuild NJOY, synthesise a photonuclear tape, clo... | 8,760 | 6 | — |
| 2026-09-22 | `2502d9c427` | rankine cycle is good for htr-10 | 3,189 | 258 | 183,762,400 |
| 2026-09-22 | `e197a280e8` | DEM pebbles now visible in chute | 95 | 18 | 5,846,535 |
| 2026-09-22 | `d2a2179fe5` | raffles: port SCRAM's seven random deviates, and the Interval... | 1,548 | 97 | — |
| 2026-09-22 | `c32570f8fe` | feat(acer): photonuclear (iopt=5) reaches byte parity with NJ... | 1,433 | 15 | — |
| 2026-09-22 | `94e56406a1` | chore(acer): let the photonuclear comparator write the table ... | 8 | 1 | — |
| 2026-09-22 | `fbcd05213b` | raffles: port SCRAM's common-cause failure groups, applied by... | 1,855 | 60 | — |
| 2026-09-22 | `ae9a5e239c` | did the labels for the simulator in the studio | 672 | 336 | 117,715,968 |
| 2026-09-22 | `bb3797f338` | data(acer): archive the photo-nuclear pair, and stamp provena... | 90 | 6 | — |
| 2026-09-22 | `5d597487a9` | htr10 schematic looks okay | 107 | 38 | 25,961,277 |
| 2026-09-22 | `b1dc96729b` | htgr_sim_v1 upgraded to v1.1 | 517 | 237 | 47,506,479 |
| 2026-09-22 | `7e47beb2c2` | raffles: port SCRAM's substitutions, and diagnose upstream's ... | 1,655 | 40 | — |
| 2026-09-22 | `48c6f716e9` | fixed label sizing | 486 | 86 | 27,168,078 |
| 2026-09-22 | `1d4edaac93` | raffles: port SCRAM's sampling and uncertainty analysis, and ... | 1,478 | 39 | — |
| 2026-09-22 | `ab35549886` | made changes to GUI, fast fwd and initial rod insertion frac | 204 | 36 | 15,687,376 |
| 2026-09-22 | `50f391d4ed` | raffles: port SCRAM's alignments -- one model, one answer per... | 895 | 21 | — |
| 2026-09-22 | `fb3a960c4e` | changed some initial settings for htr-10 sim, awaiting steady... | 8 | 8 | 0 |
| 2026-09-22 | `87f26d12b9` | added kovan wiki | 551 | 0 | 18,739,874 |
| 2026-09-22 | `ff5411fc87` | feat(htgr_sim_v1): resolve the pebble, and retire an invented... | 784 | 62 | — |
| 2026-09-22 | `1977708f76` | kovan looks better | 3,023 | 502 | 137,023,957 |
| 2026-09-22 | `d64e4cbe58` | added pan | 76 | 25 | 8,610,422 |
| 2026-09-22 | `ef6733295d` | docs(htgr_sim_v1): 0.50 is the intended rod insertion; correc... | 67 | 29 | — |
| 2026-09-22 | `a7c176e636` | kovan: corpus repositories + first-run setup; literature move... | 11,485 | 22,555 | 202,762,915 |
| 2026-09-22 | `f1cd7c0b4a` | kovan mindmap more user friendly now | 282 | 42 | 45,258,752 |
| 2026-09-22 | `88d97f5f34` | README: how it started, with the 2025 Natural FHR Simulator M... | 23 | 0 | 105,124,241 |
| 2026-09-22 | `96b60b7611` | added kovan edits | 508 | 78 | 55,896,669 |
| 2026-09-22 | `1fe1f55f2f` | changed some corpus stuff | 229 | 31 | 6,422,367 |
| 2026-09-22 | `6ccede5aea` | feat(htgr_sim_v1): Doppler onto the fuel kernel, TRISO-ATOPS ... | 2,692 | 26 | — |
| 2026-09-22 | `54a3c1715b` | corpus bug fixed | 405 | 40 | 13,134,633 |
| 2026-09-22 | `bf5d016d22` | feat(htgr_sim_v1): Gaussian puff dispersion, closing the rele... | 1,447 | 2 | — |
| 2026-09-22 | `050f971657` | debugging kovan still... | 305 | 357 | 24,882,850 |
| 2026-09-22 | `0b266bf533` | added kovan fuzzy finder | 605 | 14 | 9,053,027 |
| 2026-09-22 | `7227a013f9` | fix(htgr_sim_v1): the GUI opened the bisected rod position at... | 106 | 17 | — |
| 2026-09-22 | `042f285a40` | ingest pdf shld work? | 24 | 4 | 8,416,771 |
| 2026-09-22 | `ac645adcbd` | annotations and zooms now work correctly | 75 | 26 | 13,789,084 |
| 2026-09-22 | `598b8d7b95` | kovan thing works right now, deletes malformed annotations | 248 | 6 | 30,703,482 |
| 2026-09-22 | `cb61416b92` | RZ tab now works on htgr_sim_v1 | 653 | 1,283 | 19,290,657 |
| 2026-09-22 | `9706dffb6b` | decay heat removal attempted fix | 232 | 38 | 29,234,855 |
| 2026-09-22 | `6d5b6e8a1e` | test(htgr_sim_v1): ablation probe for the LOFC shutdown failure | 41 | 0 | — |
| 2026-09-22 | `3170fe826f` | test(htgr_sim_v1): the kernel Doppler channel breaks the inhe... | 172 | 36 | — |
| 2026-09-22 | `b81040c870` | updated outram-mc docs | 13 | 12 | 39,522,932 |
| 2026-09-22 | `4d10acf0cc` | outram-mc-libs: bring HDF5 I/O and track files into scope (gh... | 39 | 1 | 5,553,478 |
| 2026-09-22 | `e14d4d90af` | fix(mc): white boundary is its own condition, periodic is ref... | 1,282 | 21 | — |
| 2026-09-22 | `2848e9ef57` | feat(mc): variance-reduction kernel — roulette, cutoff, survi... | 342 | 0 | — |
| 2026-09-22 | `6d58a79c6c` | added a fix recursive clone | 138 | 5 | 11,032,533 |
| 2026-09-22 | `b18edb854d` | kovan: recursive corpus clone, zoom controls, and sorting fro... | 773 | 92 | 36,938,039 |
| 2026-09-22 | `a9fe8f619f` | fix(mc): correct a FABRICATED V&V number in the roulette test... | 23 | 3 | — |
| 2026-09-22 | `e698ea1844` | feat(mc): rectilinear, cylindrical and spherical tally meshes... | 661 | 2 | — |
| 2026-09-22 | `e1cf03ba29` | kovan: artifact connections file the paper too, and a fuzzy r... | 164 | 6 | 17,627,551 |
| 2026-09-22 | `ec8e06c574` | feat(mc): wire every mesh type through MeshFilter, with per-b... | 210 | 7 | — |
| 2026-09-22 | `b44d7938d2` | kovan: digitiser eraser, BibTeX close, and selective-only ann... | 72 | 4 | 6,384,187 |
| 2026-09-22 | `bd66f6b763` | feat(mc): geometry slice plotting by emitting a matplotlib sc... | 456 | 0 | — |
| 2026-09-22 | `49fdfd089f` | feat(njoy): HDF5 WRITE path — mgxs.h5, read back by OpenMC it... | 372 | 0 | — |
| 2026-09-22 | `2d71e82cff` | kovan: subtopic naming moves to a dialog, and the connection ... | 170 | 31 | 13,066,067 |
| 2026-09-22 | `37bffbd022` | kovan mindmap: say why "Add subtopic" is unavailable, and off... | 56 | 7 | 2,746,998 |
| 2026-09-22 | `86d8be3f63` | feat(mc): eight tally filters, with MT summation ported in fu... | 703 | 0 | — |
| 2026-09-22 | `f439510a1a` | docs(mc): delayed-neutron audit — DelayedGroupFilter can only... | 104 | 0 | — |
| 2026-09-22 | `481f70a1c9` | kovan mindmap: user subtopics draw under corpus nodes, and pr... | 175 | 39 | 9,709,748 |
| 2026-09-22 | `c66c702bdb` | fix(mc): DelayedGroupFilter refuses construction instead of t... | 55 | 0 | — |
| 2026-09-22 | `0a1ba07ab0` | test(njoy): verify BOTH premises #269 rests on, rather than a... | 210 | 0 | — |
| 2026-09-22 | `7761895bf8` | feat(mc): tally triggers — run to a target precision (#263, f... | 337 | 0 | — |
| 2026-09-22 | `2c6ce75230` | kovan mindmap: add literature, concept CRUD, and no more slug... | 467 | 8 | 10,206,826 |
| 2026-09-22 | `71e1cda12a` | feat(mc): polar-azimuthal source angular distribution (#264) | 401 | 0 | — |
| 2026-09-22 | `94b1ac9f0d` | kovan: artifact connections were parsed with the wrong reader... | 89 | 10 | 5,722,072 |
| 2026-09-22 | `c35d277753` | test(mc): depletion step-size convergence study (#266, scope ... | 294 | 0 | — |
| 2026-09-22 | `9b861a861c` | feat(mc): particle restart from three integers (#271, scope i... | 251 | 0 | — |
| 2026-09-22 | `2a5254f9b9` | kovan reader: nested artifact boxes resolve to the smallest, ... | 81 | 6 | 17,739,786 |
| 2026-09-22 | `1afb37ec4a` | kovan reader: rank artifact hits in normalised region space, ... | 97 | 6 | 7,072,541 |
| 2026-09-23 | `7d15f2b6c3` | kovan reader: defer an artifact jump until the canvas can sat... | 83 | 0 | 20,520,415 |
| 2026-09-23 | `fa7731e3a6` | kovan: full concept CRUD — rename, move and delete rewrite ev... | 883 | 219 | 14,962,971 |
| 2026-09-23 | `f3307b39e9` | kovan: sort the relation-kind list by key, not a hand-written... | 2 | 2 | 4,258,081 |
| 2026-09-22 | `419f152ff3` | feat(mc): anisotropic multigroup scattering, worth -4371 pcm ... | 1,922 | 13 | — |
| 2026-09-22 | `27d919bad4` | feat(mc): survival biasing, Russian roulette and mesh weight ... | 1,580 | 49 | — |
| 2026-09-22 | `26b7b38bc1` | fix(mc): eight tally filters existed but no Tally could hold ... | 1,543 | 17 | — |
| 2026-09-22 | `c8424f2930` | feat(mc): delayed-neutron data, kinetics scores, and beta_eff... | 1,097 | 71 | — |
| 2026-09-22 | `73376796e3` | feat(mc): particle tracks, state points and run summaries (#2... | 957 | 4 | — |
| 2026-09-22 | `05c0bc1cb9` | feat(mc): tally derivatives and a keff trigger (#263 scope 1-4) | 569 | 0 | — |
| 2026-09-22 | `01a5e87688` | docs(mc): Godiva beta_eff = 688 +/- 77 pcm and Lambda = 6.21 ... | 130 | 0 | — |
| 2026-09-22 | `03188cd9b6` | feat(mc): mesh, surface, file and constrained sources (#264 s... | 702 | 1 | — |
| 2026-09-22 | `60995672e0` | feat(mc): CE/CM and CF4 integrators, transfer rates, independ... | 697 | 10 | — |
| 2026-09-22 | `960ec15c7b` | feat(njoy): cross_sections.xml reader, statepoint/summary/sou... | 816 | 0 | — |
| 2026-09-22 | `9a75933f26` | feat(mc): uniform fission site weighting (#258 scope 5) | 275 | 0 | — |
| 2026-09-22 | `87d948b9b7` | feat(mc): iterated fission probability — adjoint-weighted bet... | 421 | 0 | — |
| 2026-09-22 | `02e667ecea` | feat(mc): distribcell offset tables — the last of #261's sixt... | 537 | 2 | — |
| 2026-09-22 | `bfd3996a1d` | feat(njoy): nuclide .h5 writer — OpenMC transports with a fil... | 667 | 0 | — |
| 2026-09-22 | `fedbcb2440` | feat(mc): surface-source capture wired into transport (#264 a... | 171 | 5 | — |
| 2026-09-22 | `d92e27fc29` | docs(mc): correct five claims this branch's own work made stale | 20 | 6 | — |
| 2026-09-22 | `5e90247309` | fix(mc): the shielded-room test had unbounded runtime (#258) | 21 | 6 | — |
| 2026-09-22 | `29d48e6c7d` | feat(mc): distribcell instances reach the tally — end to end ... | 253 | 6 | — |
| 2026-09-22 | `dd5352c460` | docs(mc): variance-reduction ablation results, with the bias ... | 105 | 0 | — |
| 2026-09-22 | `db11e88e72` | feat(mc): report a figure of merit for the shielding case (#2... | 97 | 2 | — |
| 2026-09-22 | `41a62fb6cc` | docs(mc): the N=32000 discriminator resolves the survival-bia... | 120 | 33 | — |
| 2026-09-22 | `f8996d4f8f` | test(mc): compute the PAIRED bias error, not just the unpaire... | 49 | 0 | — |
| 2026-09-22 | `80daf3bfd1` | fix(mc): max_split is integral, or a split creates weight (#258) | 78 | 9 | — |
| 2026-09-22 | `56831b5382` | fix(mc): bound the weight-window arm by the clock, not by a p... | 105 | 66 | — |
| 2026-09-23 | `54da922bb5` | feat(njoy): regression tests initialise the ace submodule bef... | 129 | 0 | — |
| 2026-09-23 | `7f80e5cf39` | feat(njoy): decode continuous-energy ACE into physics, and re... | 629 | 6 | — |
| 2026-09-23 | `737827a28b` | feat(njoy): decode the ACE secondary-distribution blocks (AND... | 669 | 0 | — |
| 2026-09-23 | `fc72674c09` | advanced git works, but i cannot paste git force pull works | 743 | 10 | 9,299,746 |
| 2026-09-23 | `5d588d3208` | paste fix works now | 397 | 0 | 13,033,864 |
| 2026-09-23 | `21403143f1` | kvim editor edited | 219 | 3 | 8,734,366 |
| 2026-09-23 | `bdccaa1ad2` | updated decisions | 68 | 0 | 737,053 |
| 2026-09-23 | `7c52ebab1e` | more mindmap and hyperlinks setup | 1,689 | 82 | 59,617,213 |
| 2026-09-23 | `b476058762` | hyperlinks now work | 182 | 105 | 5,867,234 |
| 2026-09-23 | `2c5cd5c0f2` | added digitiser | 1,195 | 132 | 85,428,042 |
| 2026-09-23 | `870e65f1b7` | added radionuclide inventory for TRISO ATOPS code to ingest a... | 106 | 0 | 3,817,758 |
| 2026-09-23 | `a88d12ead4` | fix(mc): split daughters inherit their parent's window state ... | 213 | 48 | — |
| 2026-09-23 | `796d25c726` | docs(mc): MAGIC does not penetrate the shield — recorded as a... | 110 | 4 | — |
| 2026-09-23 | `19f5e6426e` | fix(mc): seed MAGIC from the analog tally, and correct the V&... | 85 | 23 | — |
| 2026-09-23 | `27d0392c27` | htr-10 geometry refactored part 1 | 732 | 118 | 137,323,531 |
| 2026-09-23 | `da4486a8be` | feat(mc): report the shielding FOM by depth band, and gate on... | 126 | 77 | — |
| 2026-09-23 | `4cab7687b7` | docs(mc): the shielding figure of merit, measured — and it is... | 68 | 0 | — |
| 2026-09-23 | `d3c070f9c2` | fix(mc): the tally variance cancelled, and MAGIC read the wre... | 214 | 7 | — |
| 2026-09-23 | `f79367bbe2` | triso atops done | 479 | 55 | 113,445,019 |
| 2026-09-23 | `c92fb30de9` | test(mc): route the reported geometric mean through RealMath,... | 5 | 1 | — |
| 2026-09-23 | `b2673126d4` | test(mc): the window-mesh trade-off, measured on both meshes ... | 21 | 5 | — |
| 2026-09-23 | `55f61ca7b4` | test(mc): gate on unbiasedness, report FOM measurability — th... | 33 | 26 | — |
| 2026-09-23 | `dd8817079f` | docs(mc): withdraw the artifact FOM numbers, record the corre... | 79 | 5 | — |
| 2026-09-23 | `77f8ecb14c` | HTR-10: SiC and UO2 thermal laws, Si isotopics, and per-run d... | 662 | 46 | 83,502,755 |
| 2026-09-23 | `f654fdf70b` | test(mc): make the window mesh and MAGIC iteration count over... | 23 | 4 | — |
| 2026-09-23 | `2c77eda20a` | feat(mc): Nuclide::from_ace — transport on NJOY2016's own lib... | 413 | 2 | — |
| 2026-09-23 | `826d17a038` | docs(njoy): ACE vs ENDF — this port's RECONR+BROADR against N... | 106 | 0 | — |
| 2026-09-23 | `bba47bdbd8` | feat(mc): Godiva through this workspace's own ACE write/read ... | 262 | 0 | — |
| 2026-09-23 | `5c9c3cdc0d` | feat(mc): simplified LCT-008 ACE round trip, with parity agai... | 334 | 0 | — |
| 2026-09-23 | `a08e5a9627` | chore: gitignore generated ACE, but not the committed NJOY201... | 23 | 0 | — |
| 2026-09-23 | `784670a4e2` | fix(mc): the Godiva round trip deletes its ACE, and records i... | 43 | 3 | — |
| 2026-09-23 | `8c19cfd288` | docs(mc): LCT-008 parity measured — routes agree, and the ACE... | 60 | 0 | — |
| 2026-09-23 | `8ee2b406d7` | test(mc): a diagnostic for the ACE-vs-ENDF transport cost gap | 81 | 0 | — |
| 2026-09-23 | `931dfd5445` | docs(mc): weight windows DO penetrate the shield — corrects t... | 59 | 0 | — |
| 2026-09-23 | `4a68836ea3` | docs(mc): the ACE route's 4.6x transport gap is cross-section... | 73 | 5 | — |
| 2026-09-23 | `005f4bb3ad` | feat(njoy,mc): read depletion chain XML — 0 disagreements wit... | 2,903 | 133 | — |
| 2026-09-23 | `cbb3c44d45` | changi: run the examples at Singapore wind conditions, cited ... | 656 | 140 | — |
| 2026-09-24 | `f2e33b7112` | fix(mc): urr_ptable_control asserted the URR default that #27... | 71 | 13 | — |
| 2026-09-24 | `d9cac91da0` | docs(njoy): the chain V&V record said the OpenMC cross-check ... | 35 | 2 | — |
| 2026-09-24 | `a40ae1070d` | feat(njoy): WMP library HDF5 WRITE — OpenMC reads what we wro... | 1,093 | 0 | — |
| 2026-09-24 | `2f8f32896e` | docs(njoy): say where the WMP writer lives, and fix the links... | 26 | 13 | — |
| 2026-09-24 | `48f25f561f` | fix(nee_soon): six HTR-10 examples never updated for the SiC ... | 143 | 6 | — |
| 2026-09-24 | `082cd413d9` | fix(nee_soon): the HTR-10 cavity is fixed hardware -- delete ... | 32 | 33 | — |
| 2026-09-24 | `5618d72ed8` | fix(nee_soon): UO2 thermal laws belong in BOTH library arms, ... | 26 | 5 | — |
| 2026-09-24 | `ab7f8657ab` | feat(nee_soon): HTR-10 ENDF/B-VIII.0 loading sweep, one funct... | 569 | 0 | — |
| 2026-09-24 | `a4a0a773c9` | fix(outram-mc-libs): two more HTR-10 examples left behind by ... | 52 | 2 | 88,249,648 |
| 2026-09-24 | `0cfd3b9481` | docs: the HTR-10 twelve-height verification suite, methodolog... | 333 | 0 | 6,608,566 |
| 2026-09-24 | `15d83361d7` | fix(mc): the shielding FOM was unmeasurable because of a cons... | 194 | 81 | — |
| 2026-09-24 | `d640ef3e3a` | changi: a published core inventory, so a source term can be a... | 216 | 87 | 54,123,815 |
| 2026-09-24 | `611dacba1f` | fix(kovan): the PDF reader forgot the page on every view switch | 194 | 47 | 24,911,698 |
| 2026-09-24 | `b35faf0a07` | fix(mc): a bound outside the unit of work cannot bound it — t... | 77 | 5 | — |
| 2026-09-24 | `6ae2bcba5e` | fix(kovan): the page-context panel is unusable past a few ann... | 165 | 85 | 23,915,588 |
| 2026-09-24 | `1b70e9fe4f` | feat(kovan): "Recently opened", beside Unsorted in the map an... | 542 | 66 | 56,041,086 |
| 2026-09-24 | `a4b00ff27d` | fix(kovan): the page-context preview is uncapped -- it is the... | 13 | 4 | 3,383,265 |
| 2026-09-24 | `c2c4dca912` | fix(mc): rel_std_dev was sqrt(n) too large — my own Welford p... | 56 | 12 | — |
| 2026-09-24 | `c4d2bcc8aa` | htgr_sim_v1: two dispersion placeholders replaced by publishe... | 124 | 32 | 23,036,440 |
| 2026-09-24 | `c54e8c0b06` | docs(mc): #258's FOM criterion measured — and the answer is a... | 244 | 0 | — |
| 2026-09-24 | `f00d93be8b` | added new pdf reader in kovan | 15 | 3 | 220,175,224 |
| 2026-09-24 | `13855775d7` | added on fuel failure | 572 | 0 | 6,278,323 |
| 2026-09-24 | `8254487a8c` | editing kvim | 111 | 0 | 25,951,011 |
| 2026-09-24 | `9fe1486b41` | feat(kovan): Esc abandons a box in the PDF reader | 62 | 0 | 15,543,613 |
| 2026-09-24 | `bd437e8833` | feat(mc): a two-stage surface-source run reproduces the singl... | 485 | 15 | — |
| 2026-09-24 | `16ff3800c9` | feat(kovan): a three-stage setup form between cropping a figu... | 726 | 1 | 7,516,279 |
| 2026-09-24 | `9851afc9c0` | feat(mc): depletion step convergence and integrator order, me... | 385 | 0 | — |
| 2026-09-24 | `12569fcfb5` | fix(kovan): the setup form shows the figure it is asking about | 89 | 3 | 8,945,701 |
| 2026-09-24 | `7cee94453d` | test(mc): geometry slice plot is pixel-identical to OpenMC's ... | 473 | 0 | — |
| 2026-09-24 | `20a4ff9efd` | test(njoy): the sqrt(T) interpolation error is measured, not ... | 233 | 0 | — |
| 2026-09-24 | `5c8c70b07e` | feat(kovan): the annotation note is a modal kvim buffer, with... | 470 | 29 | 30,270,130 |
| 2026-09-24 | `1af0cd25a9` | feat(kovan): multiple series on one figure | 467 | 7 | 23,417,578 |
| 2026-09-24 | `2a71f9e75a` | docs(kovan): the canonical KOVAN expansion | 128 | 1 | 13,533,958 |
| 2026-09-24 | `c243b3a35e` | feat(kovan): multi-series digitised graphs in one artifact, #... | 195 | 14 | 3,237,517 |
| 2026-09-24 | `41225d8ab0` | fix(kovan): re-saving a digitised figure overwrites it instea... | 199 | 0 | 23,655,309 |
| 2026-09-24 | `aed0d1bdf3` | fix(kovan): take kopitiam-neovim 0.2.5, which fixes the note ... | 23 | 3 | 13,535,126 |
| 2026-09-24 | `a9d4c297f1` | feat(boon-lay): PANAMA Eqs (8a)/(9a) verified 16/16 against T... | 904 | 367 | 24,438,607 |
| 2026-09-24 | `d0fa1dde36` | fix(kovan): a pending box no longer swallows every right-clic... | 83 | 17 | 25,921,906 |
| 2026-09-24 | `785158ff90` | feat(boon-lay): the reduced diffusion coefficient D_S, verifi... | 327 | 0 | 7,607,973 |
| 2026-09-24 | `28286f019d` | fix(boon-lay): Eq (7) for SiC thinning, and a decade error in... | 431 | 42 | 22,544,498 |
| 2026-09-24 | `3fcc559e0f` | feat(boon-lay): OPF, Eqs (5a)-(5f) -- and t_B is SECONDS, set... | 388 | 22 | 23,056,692 |
| 2026-09-24 | `9f5c0c4e6f` | fix(kovan): blank CSV after banking a series; drop the stale ... | 202 | 62 | 53,452,943 |
| 2026-09-24 | `6e8696fb02` | feat(mc): the transport loop gets a particle clock (#262, #261) | 347 | 13 | — |
| 2026-09-24 | `7bcce1bb84` | feat(boon-lay): the rest of PANAMA-I -- Booth release, molar ... | 2,852 | 21 | 15,999,924 |
| 2026-09-24 | `8a060e14ad` | feat(kovan): straighten a sideways or skewed figure in the di... | 780 | 2 | 2,307,084 |
| 2026-09-24 | `6e97467391` | test(changi): analytical verification of the single Gaussian ... | 647 | 0 | 35,157,818 |
| 2026-09-24 | `d2e0b646f7` | feat(boon-lay): Fig. 8 -- the m-systematic is a DIGITISATION ... | 191 | 31 | 1,597,509 |
| 2026-09-24 | `57c55a53f7` | feat(changi): per-nuclide decay as a transfer function on the... | 365 | 0 | 20,238,719 |
| 2026-09-24 | `54749f2a8a` | feat(boon-lay): wire PANAMA into TRISO-ATOPS' f_inc -- and fi... | 917 | 21 | 22,283,713 |
| 2026-09-24 | `c4000a8d50` | gaussian puff models now render faster in map page | 809 | 39 | 101,276,207 |
| 2026-09-24 | `9a0045898a` | feat(kovan): a decade readout beside every log axis, after th... | 114 | 3 | 41,211,432 |
| 2026-09-24 | `39c82e332d` | feat(changi): a GPU path for the puff field, a wasm gate, and... | 494 | 21 | 9,222,673 |
| 2026-09-24 | `51b37182c3` | feat(boon-lay): German-lineage TRISO qualification data for H... | 574 | 6 | 691,966 |
| 2026-09-24 | `72497c6ba5` | feat(mc): PolarAzimuthalFilter on a track-length tally binned... | 83 | 12 | — |
| 2026-09-24 | `96fe329db4` | feat(mc): sample a delayed-neutron precursor group, verified ... | 249 | 0 | — |
| 2026-09-24 | `5512fd7658` | feat(htgr_sim_v1): refresh the map field at 10 Hz in the GUI,... | 300 | 32 | 3,080,701 |
| 2026-09-24 | `c1ac235c97` | feat(sembawang): HTR-10 DLOFC — PANAMA-I fuel failure wired i... | 1,860 | 6 | — |
| 2026-09-24 | `4ef5c16bee` | docs(boon-lay): the burnup-ordering test's doc recorded an EA... | 58 | 7 | — |
| 2026-09-24 | `edfd769f72` | feat(mc): carry a delayed-neutron precursor group on the fiss... | 112 | 6 | — |
| 2026-09-24 | `04ac8c9022` | docs(nee_soon): HTR-10 run log -- and a 2.9 sigma disagreemen... | 361 | 0 | — |
| 2026-09-24 | `25a731f88f` | kovan-cli affected: select the crates a change can reach, and... | 433 | 1 | 126,571,797 |
| 2026-09-24 | `5ff61217c2` | lct008: --seed on both examples, so a sweep can time each run... | 25 | 3 | 110,604,563 |
| 2026-09-24 | `2cff15ed31` | test(sembawang): measure WHICH nuclides carry floored negativ... | 83 | 0 | — |
| 2026-09-24 | `a25ce00b71` | docs(sembawang): say that the silver-only claim was measured ... | 15 | 2 | — |
| 2026-09-24 | `92b4d68618` | njoy: one ACE assembly path, and an --via-ace route for LCT-008 | 134 | 34 | 45,465,017 |
| 2026-09-24 | `37d2768e2f` | lct008: OUTRAM_KEEP_ACE, so another code can read the ACE we ... | 5 | 1 | 95,880,190 |
| 2026-09-24 | `1133565c99` | data(liggghts): regenerate htr10_rdf.csv — the committed data... | 134 | 134 | — |
| 2026-09-24 | `f06d89350d` | feat(njoy): write a fissile nuclide .h5 and read one back (#3... | 3,064 | 248 | — |
| 2026-09-24 | `16ef1d5191` | test(mc): re-derive the band-averaged xs oracle in Rust from ... | 263 | 5 | — |
| 2026-09-24 | `92628c4ff9` | docs(njoy): record the upstream OpenMC OOB-index defect with ... | 290 | 1 | — |
| 2026-09-24 | `77545b685b` | fix(docs): correct the #306 mechanism -- it is a NaN, not a s... | 120 | 97 | — |
| 2026-09-24 | `21e9e1c869` | docs(njoy): record that the converged k_inf was abandoned, an... | 43 | 4 | — |
| 2026-09-25 | `78e4ca71c7` | outram-mc: Sigma_t without the 45 lookups it throws away — tr... | 175 | 4 | 92,334,299 |
| 2026-09-25 | `ce10262528` | fix(nee_soon): nuclide_name labels the SiC slots and names ev... | 10 | 4 | 10,252,718 |
| 2026-09-25 | `cbf860950d` | feat(nee_soon): OUTRAM_HTR10_UO2_TAPE_DIR reads tabulated UO2... | 32 | 0 | 0 |
| 2026-09-25 | `857e492dac` | outram-mc: two review findings on the Sigma_t fast path | 31 | 6 | 17,135,811 |
| 2026-09-24 | `da4a382b5d` | docs(njoy): record HDF5-write non-parity as an ACCEPTED limit... | 51 | 0 | — |
| 2026-09-24 | `e96f13832f` | test(mc): paired URR+DBRC ablation to explain the ACE-vs-ENDF... | 269 | 0 | — |
| 2026-09-25 | `1a83fad7c2` | outram-mc: the cheapest CORRECT path is the default, not an o... | 33 | 0 | 15,374,491 |
| 2026-09-25 | `d619b2e77e` | fix(nee_soon): HTR-10 bottom reflector placed from the reacto... | 174 | 31 | 0 |
| 2026-09-25 | `0454c1ad1b` | fix(nee_soon): the ENDF/B-VII.0 arm takes Si-29 and Si-30 fro... | 42,188 | 8 | 0 |
| 2026-09-25 | `664644d977` | docs(nee_soon): HTR-10 RMC model audit — correct three doc cl... | 85 | 20 | 16,233,148 |
| 2026-09-25 | `5b6af9428d` | docs: the V&V record still said the HTR-10 bottom is mirrored... | 10 | 2 | 833,690 |
| 2026-09-25 | `d872ca4a57` | docs: the graphite clip is 4.76 % of ALL core carbon, not 11 ... | 39 | 16 | 4,769,994 |
| 2026-09-25 | `7743d75ad4` | test(mc): URR+DBRC worth on homogenised LCT-008 is a BOUND, n... | 154 | 3 | — |
| 2026-09-25 | `13a39399b3` | docs(nee_soon): the HTR-10 bed pebbles INTERPENETRATE — the l... | 57 | 5 | 6,861,358 |
| 2026-09-25 | `e2080b086b` | feat(njoy,mc): decode the ACE UNR block so both data routes c... | 853 | 7 | — |
| 2026-09-25 | `5f726b2007` | docs(boon-lay): call fuel_failure "boon-lay fuel failure", no... | 4,808 | 318 | — |
| 2026-09-25 | `202138f4be` | feat(dover): add DOVER as an empty skeleton crate — scope not... | 167 | 7 | — |
| 2026-09-25 | `a267569306` | docs: every crate now has a README, and every MRT crate state... | 686 | 123 | — |
| 2026-09-25 | `9e0e1a7fab` | docs(dover): DOVER is the low-fidelity counterpart of DHOBY G... | 126 | 24 | — |
| 2026-09-25 | `7f55749edb` | docs(nee_soon): HTR-10 verification status is TENTATIVE — lis... | 49 | 0 | 0 |
| 2026-09-25 | `e7d20d367e` | feat(htgr_sim_v1): real-time plume on the Map tab, one WGSL e... | 1,592 | 242 | 35,150,778 |
| 2026-09-25 | `444170a33c` | fix(htr10): place B-11 beside B-10 in every HTR-10 material (... | 8,800 | 55 | 1,531,296 |
| 2026-09-25 | `27e691cb24` | docs(nee_soon): list gh:#316 (built core ~1.4 % short on heav... | 5 | 0 | 8,898,898 |
| 2026-09-25 | `c31548767a` | fix(nee_soon): build the TRISO lattice on the grid the partic... | 76 | 17 | 0 |
| 2026-09-25 | `9b87d7a0a5` | feat(mc): DBRC on the ACE route -- attached when possible, ex... | 400 | 11 | — |
| 2026-09-25 | `81ec3c782c` | docs(nee_soon): record the measured worth of the #316 TRISO-c... | 2 | 2 | 0 |
| 2026-09-25 | `b6b7a9fc1c` | fix(htgr_sim_v1): the log-shading test asserted a floor colou... | 24 | 5 | 4,311,397 |
| 2026-09-25 | `6850cbc31d` | feat(njoy,mc): decode the ACE delayed-neutron blocks (#307 it... | 464 | 9 | — |
| 2026-09-25 | `1aab3be78d` | feat(njoy,mc): read a thermal S(a,b) ACE table back (#307 ite... | 837 | 0 | — |
| 2026-09-25 | `f34fabf3e3` | feat(njoy,mc): read every DLW law upstream reads (#307 item 3... | 1,554 | 110 | — |
| 2026-09-25 | `7b31ac07e9` | feat(njoy,mc): pair the 0 K DBRC companion automatically, rea... | 1,016 | 18 | — |
| 2026-09-25 | `dcca5f9177` | docs(claude-md): HARD RULE — reactor geometry is drawn for a ... | 136 | 0 | 1,351,119 |
| 2026-09-25 | `daef9d5a31` | docs(claude-md): the geometry-drawing HARD RULE lives in the ... | 831 | 126 | 0 |
| 2026-09-25 | `48317800ef` | feat(mc): port OpenMC's plotter to Rust with native PNG outpu... | 4,079 | 32 | 0 |
| 2026-09-25 | `8cecd12c5e` | feat(nee_soon): draw the assembled HTR-10 core as PNG (geomet... | 381 | 4 | — |
| 2026-09-25 | `074b7c0b57` | docs(claude-md): the OpenMC-parity plotter has landed -- upda... | 180 | 40 | — |
| 2026-09-25 | `6314a65061` | docs(api): regenerate the outram-mc-libs and nee_soon API mir... | 28,164 | 4,820 | — |
| 2026-09-25 | `fe975296a2` | docs(nee_soon): gh:#218 drift cause now evidenced by the shru... | 8 | 5 | 0 |
| 2026-09-25 | `1b8c95704f` | nee_soon: HTR-10 bed as the paper's two-ball prism cell (gh:#... | 1,585 | 391 | — |
| 2026-09-25 | `d36e37ec07` | feat(dover): steam-methane-reforming CSTR, headless, deck-driven | 2,260 | 71 | — |
| 2026-09-25 | `4adddb9495` | docs(dover): link the dwsim-libs Cstr solver finding to GitHu... | 4 | 2 | — |
| 2026-09-25 | `2193e474b0` | test(mc): add the ablated ENDF arm to the ACE round trip (#30... | 76 | 1 | — |
| 2026-09-25 | `8eeb74af6f` | test(mc): multi-seed the ACE round trip, and gate its verdict... | 139 | 19 | — |
| 2026-09-25 | `ca293bc3df` | test(dwsim-libs): CSTR parity against compiled upstream DWSIM | 622 | 4 | — |
| 2026-09-25 | `55e46d6f4c` | WIP nee_soon: HTR-10 reflector as explicit 3-D geometry (chec... | 1,927 | 300 | — |
| 2026-09-25 | `f9d994b91f` | docs(nee_soon): withdraw the claim that the shrunk-pebble abl... | 10 | 8 | 0 |
| 2026-09-25 | `6409eadc2a` | test(mc): the ACE-vs-ENDF gap was one seed -- +23.9 +/- 125.0... | 274 | 18 | — |
| 2026-09-25 | `f8eac45abb` | nee_soon: HTR-10 bed as the paper's two-ball prism cell (gh:#... | 1,585 | 391 | — |
| 2026-09-25 | `d6ee9b2dad` | HTR-10 two-ball cell: V&V record, eigenvalue results, doc cor... | 1,453 | 88 | — |
| 2026-09-25 | `464657fe33` | WIP nee_soon: HTR-10 reflector as explicit 3-D geometry (chec... | 1,927 | 300 | — |
| 2026-09-26 | `3f141992ed` | feat(njoy): write the ACE UNR block, and fix the PURR energy ... | 1,098 | 56 | — |
| 2026-09-26 | `b9da6368d7` | test(mc): lct008_ace_roundtrip --purr builds route B with URR... | 45 | 11 | — |
| 2026-09-26 | `87f9177e57` | feat(mc): port OpenMC's Python Model.plot as emitted matplotl... | 8,487 | 90 | — |
| 2026-09-26 | `cb6d83b78a` | feat(mc,nee_soon): port plot_xs/Tracks.plot; HTR-10 cross-sec... | 10,608 | 56 | — |
| 2026-09-26 | `29efcae29e` | feat(nee_soon): OUTRAM_HTR10_NI_AS_FE -- rod-steel Ni replace... | 61 | 1 | — |
| 2026-09-26 | `92b76cce73` | feat(nee_soon): OUTRAM_HTR10_FE57_AS_FE56; rod-metal slots de... | 86 | 31 | — |
| 2026-09-26 | `b039d06f30` | njoy: ACE blocks reproduce NJOY2016's word for word except th... | 3,720 | 252 | — |
| 2026-09-26 | `36294e003f` | outram-mc-libs: record the 400-seed continuum-anisotropy wort... | 33 | 1 | — |
| 2026-09-26 | `0e44eac0ca` | docs(mc): HTR-10 fast VII.0/VIII.0 ablation on the explicit-r... | 86 | 0 | — |
| 2026-09-26 | `79f9df022b` | njoy: RECONR/BROADR reproduce NJOY2016's PENDFs word for word... | 4,206 | 648 | — |
| 2026-09-26 | `485ffec74a` | njoy: all four NJOY2016 reference ACE tables reproduced in ev... | 1,216 | 64 | — |
| 2026-09-27 | `c6f5232489` | docs(mc): hard rule -- show geometry visuals to the maintaine... | 27 | 0 | — |
| 2026-09-27 | `17aeab31e6` | docs(api): regenerate the nee_soon and outram-mc-libs API mir... | 25,373 | 17,618 | — |
| 2026-09-27 | `4ddfcec954` | fix(nee_soon): match RMC on the paper's whole-ball height (gh... | 108 | 22 | — |
| 2026-09-27 | `18f5a9e99b` | feat(nee_soon): carry Li (2014)'s MCNP columns as a rough gauge | 151 | 3 | — |
| 2026-09-27 | `f2e087cdba` | docs(mc): document and pin which temperature takes precedence... | 266 | 4 | — |
| 2026-09-27 | `ed874efaef` | feat(nee_soon): side-wall rejection ablation for gh:#331, wit... | 4,842 | 0 | — |
| 2026-09-27 | `0887a9a003` | docs(nee_soon): record the gh:#331 decision -- balls crossing... | 13 | 3 | — |
| 2026-09-27 | `67c08776cf` | docs(nee_soon): record the gh:#330 and gh:#332 decisions | 26 | 8 | — |
| 2026-09-27 | `f8d71da271` | nee_soon: correct stale HTR-10 end-of-run message (full radiu... | 9 | 2 | — |
| 2026-09-27 | `bf53668bfa` | nee_soon: OUTRAM_HTR10_SEED knob for one-seed-per-process poo... | 8 | 3 | — |
| 2026-09-27 | `b4c0e4ce5b` | V&V htr10_rmc: graphite S(a,b) sensitivity, fast runs (10P +3... | 38 | 0 | — |
| 2026-09-27 | `08e13a01e8` | nee_soon HTR-10: 30P reactor graphite is the default S(a,b) o... | 440 | 36 | — |
| 2026-09-27 | `f3909c64f2` | outram-mc-libs: every timing carries its hardware (hard rule,... | 130 | 12 | — |
| 2026-09-27 | `d38bb2d5d1` | nee_soon: htr10_pooled_study example (Linux only) — resumable... | 261 | 0 | — |
| 2026-09-27 | `c9a9e33246` | outram-mc-libs: hardware detection (CPU model, RAM) is Linux-... | 32 | 11 | — |
| 2026-09-27 | `f94e2873d6` | changi: Gaussian puffs are Lagrangian by default — they remem... | 532 | 43 | 196,448,796 |
| 2026-09-27 | `7fac861165` | build: a release-codegen profiling profile, a GPU plume-timin... | 67 | 0 | 584,492 |
| 2026-09-27 | `73a625ae0b` | htgr_sim_v1: the map plume marches with history; rod 0.30 and... | 1,820 | 319 | 585,411 |
| 2026-09-27 | `2c59406017` | htgr_sim_v1 map: rings and a wind arrow, with a LIVE chi/Q sa... | 322 | 36 | 30,075,469 |
| 2026-09-27 | `df364771e7` | tampines: the helical-coil steam generator moves here, with a... | 3,322 | 52 | 5,022,692 |
| 2026-09-27 | `9cde161092` | Profile the four ICSBEP benchmarks; remove an O(N^2) step in ... | 5,102 | 22 | — |
| 2026-09-27 | `7aed403836` | htgr_sim_v1: the dispersion is 0.00 % of the plant step, not ... | 41 | 6 | 6,383,361 |
| 2026-09-27 | `35f60d3655` | htgr_sim_v1 map: 60 fps field, and the live ring column is fi... | 287 | 7 | 16,326,665 |
| 2026-09-27 | `db1ee907ff` | htgr_sim_v1: dispersion every 2 s, and the timing breakdown w... | 234 | 26 | 53,024,216 |
| 2026-09-27 | `b207aea83f` | htgr_sim_v1: map field back to 10 Hz | 40 | 15 | 6,470,608 |
| 2026-09-27 | `76a83ca98a` | starting at 0.45 | 2 | 2 | 6,512,599 |
| 2026-09-27 | `f220e52623` | htr10_pooled_study: quote a difference's sigma only when both... | 7 | 3 | — |
| 2026-09-27 | `596861f4a9` | added map dispersion | 5 | 2 | 0 |
| 2026-09-27 | `d2d5b1f88b` | changing atmospheric dispersion now... | 291 | 46 | 14,821,737 |
| 2026-09-27 | `6a13c513c5` | mc: SpeedTier selector -- Standard / Fast (default, exact) / ... | 58,570 | 24,296 | — |
| 2026-09-27 | `9f62235ad4` | docs(mc): VeryFast dk on all four ICSBEP cases -- thermal sys... | 48 | 11 | — |
| 2026-09-27 | `55b95562dd` | test(mc): re-record the #258 analog pin after bisecting it to... | 63 | 7 | — |
| 2026-09-27 | `52075ae403` | V&V htr10_rmc: pooled graphite S(a,b), 4 seeds: 10P - crystal... | 41 | 0 | — |
| 2026-09-27 | `b082d3302b` | V&V htr10_rmc: pooled graphite S(a,b) study complete — 30P de... | 56 | 0 | — |
| 2026-09-28 | `ec28c8a8a0` | nee_soon V&V: plot script for HTR-10 k vs height on the fast ... | 138 | 0 | — |
| 2026-09-28 | `582204b2aa` | docs(mc): record the maintainer's choice of Fast as the defau... | 5 | 2 | — |
| 2026-09-28 | `041bbecfba` | nee_soon V&V: HTR-10 fast-ENDF-path single-seed runs, VII.0 a... | 91 | 8 | — |
| 2026-09-28 | `9986f2a812` | depreacted kopi-beans | 5 | 4 | 1,124,509 |
| 2026-09-28 | `79fbe235c4` | agents md updated | 45 | 146 | 1,077,013 |
| 2026-09-28 | `c4a5ccce34` | docs(agents): point AGENTS.md at GitHub issues; delete the bn... | 0 | 101 | 163,614 |
| 2026-09-28 | `5aedd28065` | V&V htr10_fast_path: pre-merge rebuild gives identical VII.0 ... | 31 | 5 | — |
| 2026-09-28 | `90e8ea28aa` | push ctrl rod drives | 18 | 6 | 3,461,989 |
| 2026-09-28 | `e2927527de` | table does seem to work well | 34,339 | 5,013 | 68,619,688 |
| 2026-09-28 | `39589dde08` | updated kovan and some included some other digitised tables | 10,805 | 61 | 54,154,933 |
| 2026-09-28 | `88aba9cebf` | updated changi claude md | 31 | 27 | 3,044,544 |
| 2026-09-28 | `0533d43096` | added buangkok and bishan | 394 | 7 | 22,557,651 |
| 2026-09-28 | `7bfa32b2af` | fixing kovan | 1,126 | 36 | 20,074,454 |
| 2026-09-28 | `f3ae50e012` | added more on table digitiser | 70 | 9 | 5,152,019 |
| 2026-09-28 | `09b63c9755` | added more kovan work | 2,674 | 66 | 36,171,373 |
| 2026-09-28 | `3ee9b70295` | feat(kovan): banked-series picker; confirm before recomputing... | 994 | 55 | 35,522,063 |
| 2026-09-28 | `6a5941882b` | V&V(boon-lay): PANAMA-I Fig. 7 re-digitised; the x7/6 reading... | 632 | 105 | 0 |
| 2026-09-28 | `ebcfef8a0d` | added new models for the graphite and panama stuff | 767 | 50 | 33,257,612 |
| 2026-09-28 | `84efe00b54` | kovan ctrl c and ctrl v fixes | 1,291 | 60 | 94,798,992 |
| 2026-09-28 | `f5a7495401` | feat: htgr_sim_v1 energy chain + graphite to 3000 K; kovan pu... | 10,793 | 2,713 | 124,468,142 |
| 2026-09-28 | `38f93f5568` | feat(buangkok): complete pyDOSEIA port, code-to-code verified... | 21,952 | 1,074 | 22,843,736 |
| 2026-09-28 | `993c227e35` | feat(kovan): EPA FGR-11, FGR-13 and FGR-15 (2025 rev.) in the... | 181 | 15 | 12,714,032 |
| 2026-09-28 | `16f72f48aa` | feat(htgr_sim_v1): map absolute source term, chi/Q-by-distanc... | 1,540 | 260 | 0 |
| 2026-09-28 | `8c464a5ccc` | feat(petir,buangkok): port GSL QAGS into petir; plume shine r... | 2,300 | 85 | 725,608 |
| 2026-09-28 | `b7a5c91b8d` | chore(kovan): bump kopitiam-pdf to 0.4.2; refuse rotating a r... | 43 | 3 | 3,317,899 |
| 2026-09-28 | `be4206c2d3` | feat(outram-mc-libs,njoy): five-route ICSBEP k_eff campaign p... | 1,675 | 0 | 7,267,437 |
| 2026-09-29 | `d495008952` | feat(outram-mc-libs): five-route ICSBEP k_eff results; ACE-ro... | 1,664 | 18 | 11,210,472 |
| 2026-09-29 | `9b97e1f64c` | feat(outram-mc-libs): lattice LCT-008 case 1 in the five-rout... | 1,036 | 831 | 6,698,591 |
| 2026-09-29 | `a922711f53` | fix(outram-mc-libs,njoy): ACE LNW chains of correlated laws s... | 1,349 | 24 | 3,258,972 |
| 2026-09-29 | `52770a2cad` | refactor(outram-mc-libs): lct008_ace_roundtrip runs the share... | 184 | 74 | 0 |
| 2026-09-29 | `3d9526e5a1` | fix(five_route_keff): launcher honours CARGO_TARGET_DIR; scri... | 2 | 1 | 3,112,286 |
| 2026-09-29 | `a15958912c` | fix(outram-mc-libs,njoy): ACE route drops redundant MT=4 and ... | 879 | 20 | 2,389,944 |
| 2026-09-29 | `73db0f619e` | fix(njoy,outram-mc-libs): thermal ACE THERMR tol from the cal... | 272 | 287 | 3,831,412 |
| 2026-09-29 | `9cc8f0dc47` | fix(njoy): thermal ACE ITIX is calcem's xsi via THERMR terp, ... | 79 | 4 | 0 |
| 2026-09-29 | `b6cf182295` | test(njoy): thermal_ace_tolerance: ITIX follows tol now that ... | 13 | 5 | 0 |
| 2026-09-29 | `25270acb67` | data(five_route_keff): lattice LCT-008 routes 1/2/4; archive ... | 284 | 394 | 0 |
| 2026-09-29 | `0051dd005a` | docs(digital-twin-engine): correct stale CLAUDE.md claims; in... | 55 | 5 | — |
| 2026-09-29 | `7ee65476e0` | docs(digital-twin-engine): helium-outlet <= bed is a steady-s... | 7 | 2 | — |
| 2026-09-29 | `787a2d9efc` | test(boon-lay): TRISO-ATOPS Booth vs Crank at CRP-6 Case 1, a... | 357 | 0 | — |
| 2026-09-29 | `9ab0f62b89` | test(htgr_sim_v1): circulating activity vs Liu & Cao Table 3 ... | 184 | 0 | — |
| 2026-09-29 | `1db08e5e9a` | feat(njoy,outram-mc-libs): ACE reader audit (#365): URR as Op... | 4,583 | 2,063 | 7,481,699 |
| 2026-09-29 | `b03e5cc33b` | feat(buangkok,htgr_sim_v1): indicative dose-rate map basis + ... | 2,624 | 65 | 4,203,082 |
| 2026-09-29 | `991b483432` | test(buangkok): external dose from Liu & Cao releases vs thei... | 465 | 1 | — |
| 2026-09-29 | `a79755763b` | diag(five_route_keff): --no-sab A/B on both codes (#367) | 13 | 0 | 11,686,130 |
| 2026-09-29 | `947da17ced` | diag(five_route_keff): --drop-nuclide A/B on both codes (#367) | 15 | 1 | 8,503,241 |
| 2026-09-29 | `4633eb0ff9` | data(five_route_keff): route 2/3/5 rerun rows (post-#366, fix... | 399 | 0 | 0 |
| 2026-09-29 | `9a64790c56` | docs(five_route_keff): interim record around the lattice; lct... | 261 | 330 | 0 |
| 2026-09-29 | `a0923eff77` | diag(five_route_keff): HST-009 --variant solution-inf\|soluti... | 57 | 1 | 5,845,315 |
| 2026-09-29 | `10f43e152a` | data(five_route_keff): #367 leakage/medium and bare-no-O16 A/Bs | 192 | 0 | 11,721,170 |
| 2026-09-29 | `f1ac422b11` | feat(njoy,outram-mc-libs): delayed-neutron spectra on both ro... | 803 | 42 | 823,482 |
| 2026-09-29 | `2d2a1787fa` | data(five_route_keff): archive pre-delayed-spectra route 3/4/... | 336 | 335 | 826,677 |
| 2026-09-29 | `12ef37a646` | fix(htgr_sim_v1): primary helium circuit as enthalpy-balance ... | 1,986 | 655 | 414,178 |
| 2026-09-29 | `3fc52607ea` | diag(five_route_keff): --ablate no-delayed-spectra (#365, #367) | 2 | 0 | 1,677,648 |
| 2026-09-29 | `d3b4517082` | feat(njoy,outram-mc-libs): continuous (IFENG=2) S(a,b) ACE ta... | 569 | 77 | 1,265,608 |
| 2026-09-29 | `79748c3cdd` | feat(njoy,outram-mc-libs): skewed S(a,b) bins and incoherent-... | 539 | 70 | 19,585,003 |
| 2026-09-29 | `0df2175706` | feat(njoy,outram-mc-libs): polynomial nu-bar evaluated exactl... | 197 | 7 | 8,578,104 |
| 2026-09-29 | `18821e33ae` | feat(njoy,outram-mc-libs): mixed thermal elastic (IDPNC=5) an... | 596 | 18 | 8,767,536 |
| 2026-09-29 | `d46f484fc7` | fix(htgr_sim_v1): passive path in the bed's implicit solve; A... | 1,371 | 726 | 7,531,383 |
| 2026-09-29 | `bc87485d60` | feat(htgr_sim_v1): reactivity budget panel; NRC 2023 10 mSv/9... | 298 | 2 | 1,908,076 |
| 2026-09-29 | `9794f5562c` | feat(htgr_sim_v1): reflector -> riser-helium leg in the impli... | 493 | 210 | 4,341,389 |
| 2026-09-29 | `99d6a9904f` | fix(htgr_sim_v1): placeholder-tier test passes the riser inle... | 1 | 0 | 0 |
| 2026-09-29 | `6e7d6654de` | feat(outram-mc-libs): transport the other neutron-emitting re... | 1,205 | 10 | 3,416,136 |
| 2026-09-29 | `f464c76f33` | feat(source-term stage 1): HTR-10 fuel quality, live in-servi... | 1,079 | 172 | 0 |
| 2026-09-29 | `96e2588321` | Add derivation in LaTeX | 881 | 0 | — |
| 2026-09-29 | `804012c739` | feat(source-term stage 2): bishan building CV, emission-time ... | 1,109 | 239 | 26,942,282 |
| 2026-09-29 | `fd9d5c287c` | Add derivation in markdown | 598 | 0 | — |
| 2026-09-29 | `6fa7992c4b` | fix(tampines-steam-tables): bounded energy convection in Tamp... | 581 | 34 | — |
| 2026-09-29 | `a22240d292` | feat(njoy,outram-mc-libs): discrete lines in tabulated energy... | 287 | 29 | 7,938,258 |
| 2026-09-29 | `e9a853a074` | fix(htgr_sim_v1): primary helium inventory at parity with Yao... | 124 | 40 | 0 |
| 2026-09-29 | `d32bf81af8` | fix(merge u7qwe0): drop the false second-law-test strike-thro... | 194 | 167 | 537,850 |
| 2026-09-29 | `98ca86577f` | fix(buangkok): missing-coefficient test after the FGR-15 tabl... | 24 | 4 | 9,320,899 |
| 2026-09-29 | `2bfe00e257` | test(buangkok): record the #379 accident band re-measured wit... | 8 | 0 | 2,779,628 |
| 2026-09-29 | `c798909638` | docs(tampines-steam-tables): Edwards V&V is valid only under ... | 29 | 1 | 3,969,408 |
| 2026-09-29 | `4d5e3c60a8` | docs(digital-twin-engine): htgr_sim_v1 is not in validation y... | 33 | 0 | 8,646,400 |
| 2026-09-29 | `27143819f2` | test(boon-lay): withdraw the "decade below the 11 % compact" ... | 107 | 9 | — |
| 2026-09-29 | `4b86656e88` | fix(htgr_sim_v1): one beta_eff everywhere; precursors-start-e... | 497 | 65 | 37,997,243 |
| 2026-09-29 | `52c6626686` | docs(boon-lay): regenerate boon-lay-api.md with kovan-cli api... | 556 | 76 | 4,415,027 |
| 2026-09-29 | `4b227dee35` | chore(kovan): bump kopitiam-pdf to 0.4.3 (plain-CMYK JPEGs no... | 7 | 3 | 5,082,498 |
| 2026-09-29 | `1eb308acf1` | data(five_route_keff): interim summary and figure at f1ac422b... | 26 | 24 | 14,179,278 |
| 2026-09-29 | `8d74ff6781` | feat(source-term stage 3): water ingress -- steam moderation,... | 1,332 | 28 | 5,919,584 |
| 2026-09-29 | `9967ccaf32` | fix(htgr_sim_v1): the out-of-range kernel-hydrolysis burst is... | 89 | 1 | 1,990,029 |
| 2026-09-29 | `a22661666e` | test(ci): copy the smoke set into top crates — Godiva HIGH ti... | 3,294 | 0 | — |
| 2026-09-29 | `f02992d6ea` | feat(kovan-cli): `ci top-crates`, `ci smoke`, `ci known-failu... | 648 | 10 | — |
| 2026-09-29 | `2bd2deec3d` | ci: fast-tests = top-crate compile gate + smoke set on 3 OSes... | 138 | 141 | — |
| 2026-09-29 | `b3bbf5f689` | feat(source-term stage 4a): DLOFC + ATWS -- DN65 blowdown, de... | 738 | 18 | 12,782,053 |
| 2026-09-29 | `d412f98bd7` | test(boon-lay): MHTGR end-to-end workflow vs Stoyer et al. --... | 1,210 | 0 | 680,885 |
| 2026-09-29 | `a249e14202` | feat(kovan): pulling the Kovan folder pulls its corpora (#422... | 719 | 48 | 29,496,138 |
| 2026-09-29 | `af75e7e31a` | feat(kovan digitiser): Lock axes button; no tracing until the... | 318 | 51 | 586,020 |
| 2026-09-29 | `37807e7069` | docs(kovan): regenerate kovan-api.md (kovan-cli api-docs kovan) | 1,986 | 490 | 0 |
| 2026-09-29 | `9291a3c6dc` | test(boon-lay): MHTGR final (heat-up) releases vs Stoyer Tabl... | 733 | 21 | 28,474,403 |
| 2026-09-29 | `4a456a2fb3` | feat(njoy,outram-mc-libs): correlated angle laws and nu/yield... | 4,043 | 141 | — |
| 2026-09-29 | `81ec018c39` | feat(outram-mc-libs): URR band per energy, DBRC and equiproba... | 1,610 | 134 | — |
| 2026-09-29 | `f78b5180d5` | docs(outram-mc-libs,njoy): #365/#407 V&V records and k shifts... | 478 | 12 | — |
| 2026-09-29 | `73cd81723a` | data(five_route_keff): final routes 3/4/5 at f78b5180d5; HST-... | 1,055 | 650 | — |
| 2026-09-30 | `28e9534fdd` | docs(outram-mc-libs): lct008_ace_roundtrip results at f78b518... | 23 | 0 | — |
| 2026-09-30 | `e5940ab6e4` | fix(kovan): fetch missing corpora on Pull, and the workspace ... | 371 | 13 | 0 |
| 2026-09-30 | `a0472247ce` | docs(boon-lay): the TRISO-ATOPS accident case and run file AR... | 1,009 | 2 | 66,872,388 |
| 2026-09-30 | `d7bb48f7f0` | data(kovan-literature): derived datasets for the fission-prod... | 457 | 0 | 44,283,534 |
| 2026-09-30 | `0c6ecc3867` | feat(sembawang): HTR-10 air-ingress bounding case B example —... | 298 | 0 | 37,599,099 |
| 2026-09-30 | `b751b9bb09` | fix(sembawang): restore upstream's uniform-constant-temperatu... | 3,496 | 51 | 34,844,595 |
| 2026-09-30 | `748bfb8e1f` | feat(sembawang): buangkok as a dependency; 96 h maximum dose ... | 256 | 24 | 26,315,288 |
| 2026-09-30 | `ec5a39a417` | test(boon-lay): #413 final releases re-run on the maintainer'... | 438 | 221 | 55,280,722 |
| 2026-09-30 | `354c92ae2d` | data(buangkok): FGR-11 inhalation for I-132..135, Cs-134, Sr-... | 38 | 10 | 24,333,201 |
| 2026-09-30 | `af26f7940b` | chore: Cargo.lock for sembawang -> buangkok (missed in 748bfb... | 1 | 0 | 4,486,006 |
| 2026-09-30 | `95e4375ab8` | data(kovan-literature): record the maintainer's TECDOC-978 Fi... | 31 | 1 | 14,246,025 |
| 2026-09-30 | `e4ca01bb72` | data(kovan-literature): exclude Fig. 5-21's hard-to-read 1400... | 6 | 1 | 5,292,963 |
| 2026-09-30 | `79b64032ed` | data(kovan-literature): TECDOC-978 Table 5-7 human-checked; t... | 7 | 0 | 4,651,156 |
| 2026-09-30 | `f131c0cd88` | data(kovan-literature): Table 5-7 provenance: pasted from the... | 3 | 2 | 2,669,619 |
| 2026-09-30 | `bd2bfabf9e` | test(boon-lay): #413 check (a) -- temperature-sensitivity DIA... | 1,074 | 0 | 9,067,092 |
| 2026-09-30 | `d25f78e978` | fix(sembawang): accidents start from real normal-operation po... | 1,238 | 124 | 84,971,428 |
| 2026-09-30 | `c5a26f1318` | fix(boon-lay,sembawang): #449, code-to-code verified — upstre... | 3,121 | 65 | 64,808,538 |
| 2026-09-30 | `a7e6506945` | fix(outram-mc-libs): DBRC rejection loop unbounded, as OpenMC... | 627 | 25 | — |
| 2026-09-30 | `e6914cae0e` | docs(outram-mc-libs): hardware line on the lct008_ace_roundtr... | 3 | 1 | — |
| 2026-09-30 | `a339d44c9a` | test(boon-lay): #413 check (b) -- dense PCHIP resampling of F... | 6,043 | 88 | 106,580,261 |
| 2026-09-30 | `2a581c205e` | fix(kovan): #455 standard-corpus documents are ingested; dupl... | 3,596 | 610 | 0 |
| 2026-09-30 | `be8d04ccfe` | fix(sembawang): #447 item 1: a non-uniform isothermal field i... | 241 | 8 | 30,773,885 |
| 2026-09-30 | `95b4aea8a3` | feat(sembawang): #452 HTR-10 bounding air ingress vs equal-po... | 2,029 | 6 | 15,257,126 |
| 2026-09-30 | `9fbf86fa45` | docs(sembawang): #452 example points at FGR-coverage follow-u... | 1 | 1 | 0 |
| 2026-09-30 | `34aad41720` | feat(htgr_sim_v1,sembawang): #453 "Bounding air ingress" map ... | 1,274 | 8 | 6,855,919 |
| 2026-09-30 | `de0593e14b` | test(outram-mc-libs): temperature-precedence control made obs... | 52 | 1 | — |
| 2026-09-30 | `f34ab1520f` | feat(htgr_sim_v1): #420 air ingress in the live DLOFC via the... | 265 | 40 | 34,020,213 |
| 2026-09-30 | `7d1e1c5ea7` | feat(sembawang): #452 HTR-10 design-basis arm (Liu & Cao Tabl... | 350 | 3 | 16,060,735 |
| 2026-09-30 | `3236b50dc3` | feat(sembawang,htgr_sim_v1): #450/#452/#453 DBA-vs-DBA is the... | 1,218 | 381 | 3,601,025 |
| 2026-09-30 | `2ceb84072c` | feat(kovan): #458 several standard, open and proprietary repo... | 4,699 | 397 | 0 |
| 2026-09-30 | `ea29cd8c43` | test(boon-lay): #413 check (c) part 2 -- release vs time agai... | 1,246 | 5 | 60,013,581 |
| 2026-09-30 | `c5d649ea8a` | feat(sembawang,htgr_sim_v1): #464 LWR LOCA + core melt arm; t... | 1,824 | 307 | — |
| 2026-09-29 | `41f707ca87` | test(buangkok,changi): changi's puff-train chi/Q checked agai... | 614 | 4 | — |
| 2026-09-30 | `ad40dc8012` | docs: correct "empty skeleton" claims about dover after its f... | 13 | 8 | — |
| 2026-09-30 | `11b46e4f42` | docs(dover): correct "no crate reads a simulation deck" now t... | 10 | 4 | — |
| 2026-09-30 | `3cd7909a7e` | fix(kovan): Save Repository's change list scrolls in a capped... | 89 | 9 | — |
| 2026-09-30 | `0a48ad4a7a` | docs(sembawang): hand-typed audit copy of the HTR-10 air-ingr... | 647 | 0 | — |
| 2026-09-30 | `f9e0274729` | docs(sembawang): hand-typed HTR-10 air-ingress example, secti... | 95 | 11 | — |
| 2026-09-30 | `43d0d09f72` | added comments useful for PyC retention | 8 | 0 | — |
| 2026-09-30 | `2d62194a0a` | docs(sembawang): hand-typed HTR-10 air-ingress example, secti... | 48 | 10 | — |
| 2026-09-30 | `5efa8d91b5` | feat(sembawang): htr10_dlofc_dose example, DLOFC release thro... | 271 | 0 | — |
| 2026-09-30 | `84942acfc3` | fix(njoy,outram-mc-libs): ENDF-route S(a,b) as OpenMC's libra... | 354 | 113 | — |
| 2026-09-30 | `0c1698f01b` | test(outram-mc-libs,njoy): re-pin the thermal tests #459 move... | 79 | 17 | 0 |
| 2026-09-30 | `0414bc8277` | fix(outram-mc-libs,nee_soon): OpenMC-parity audit fixes #460 ... | 1,108 | 452 | — |
| 2026-09-30 | `058d51af68` | changed name to human audited example | 0 | 0 | — |
| 2026-09-30 | `4a38a57bfd` | data(outram-mc-libs): five-route k_eff re-run after the OpenM... | 1,132 | 418 | — |
| 2026-09-30 | `542ac20036` | diag(outram-mc-libs): per-nuclide DBRC split for the lattice ... | 16 | 0 | — |
| 2026-09-30 | `d5d43d71cd` | fix(htgr_sim_v1,sembawang): carry the #469 human-audit notes ... | 395 | 45 | 39,771,077 |
| 2026-10-01 | `3364c1a422` | docs(claude-md): the maintainer supplies the kovan corpus; ag... | 29 | 27 | 22,944,473 |
| 2026-10-01 | `5fa661f608` | docs(nee_soon): Li's Table 3/4 MCNP columns are Seker's vacuu... | 11 | 3 | 1,070,928 |
| 2026-10-01 | `974cdee44c` | docs(nee_soon,outram-mc-libs): cite Seker & Colak (2003) in t... | 177 | 28 | 4,992,075 |
| 2026-10-01 | `91cc5e1515` | docs(claude-md): migrate the old kovan rules out of the CLAUD... | 95 | 68 | 0 |
| 2026-10-01 | `348eec9305` | feat(nee_soon): HTR-10 bed is Seker & Colak (2003)'s 13-ball ... | 1,544 | 194 | 33,069,929 |
| 2026-10-01 | `69210190d0` | docs(outram-mc-libs): V&V README marks Seker's 13-ball cell a... | 32 | 1 | 8,942,044 |
| 2026-10-01 | `f51a581f5b` | fix(nee_soon): withdraw the two-ball cut-pebble bed as an abl... | 25 | 8 | 5,902,491 |
| 2026-10-01 | `ad728330c3` | fix(nee_soon): withdraw the one-ball homogenised core_model::... | 42 | 49 | 8,358,563 |
| 2026-10-01 | `57ea1af9b4` | data(outram-mc-libs): first k_eff on Seker's whole-ball HTR-1... | 37 | 0 | 3,492,942 |
| 2026-10-01 | `80a87476f8` | docs(sembawang): justify f_inc = 5e-4 from Liu & Cao 2002 §2.... | 12 | 1 | 1,673,711 |
| 2026-10-01 | `89fad7b86e` | feat(htgr_sim_v1): steady Gaussian-plume overlay on the map, ... | 584 | 2 | 2,651,263 |
| 2026-10-01 | `3a335634b9` | feat(htgr_sim_v1): plume-centreline accumulated-dose graph be... | 725 | 1 | 5,764,446 |
| 2026-10-01 | `bb1ac2fd14` | docs(htgr_sim_v1): the Map tab does have a Reset plant contro... | 5 | 4 | 1,849,046 |
| 2026-10-01 | `af7991ca2a` | fix(htgr_sim_v1): split the cold return into cold-duct + RPV-... | 1,183 | 276 | 2,721,880 |
| 2026-10-01 | `1c6d753474` | docs(htgr_sim_v1): mark the LOFC, settled-power and baseline ... | 6 | 0 | 0 |
| 2026-10-01 | `b033db71aa` | feat(htgr_sim_v1): open at the HTR-10 test's experimental 1.2... | 130 | 47 | 9,060,064 |
| 2026-10-01 | `74aef7fe28` | feat(htgr_sim_v1): linear TEDE graph with AP1000 + bounding-a... | 2,199 | 163 | 9,302,878 |
| 2026-10-01 | `a0d57e5c3f` | feat(kovan): drag an edited annotation's box corners to corre... | 972 | 68 | 7,271,257 |
| 2026-10-01 | `86c01a7f5c` | feat(htgr_sim_v1): map/graph zoom, side scroll, Liu & Cao Tab... | 1,824 | 253 | 9,429,127 |
| 2026-10-01 | `cbd9172838` | feat(htgr_sim_v1): "Normalise to thermal power" selector on t... | 596 | 161 | 2,083,902 |
| 2026-10-01 | `3860b0f2da` | feat(nee_soon): compare the whole-ball HTR-10 bed to RMC at e... | 153 | 10 | 13,358,494 |
| 2026-10-01 | `46700f22f1` | feat(htgr_sim_v1): NRC 10 mSv / 96 h line thick, solid, pure ... | 17 | 4 | 7,064,417 |
| 2026-10-01 | `23fb6f44da` | fix(nee_soon): geometry export's realised-lattice rows use Se... | 33 | 17 | 12,106,371 |
| 2026-10-01 | `d00ca68161` | fix(nee_soon): HTR-10 reflector channels follow Seker & Colak... | 129 | 14 | 10,431,348 |
| 2026-10-01 | `906252eb49` | fix(nee_soon): 8335 TRISO per fuel pebble, as stated (generic... | 96 | 30 | 20,279,396 |
| 2026-10-01 | `2064aaf001` | fix(nee_soon): geometry images find a kernel off the pebble m... | 9 | 2 | 3,989,373 |
| 2026-10-01 | `0f2fdd2de9` | docs(htr10): correct stale and source-contradicted HTR-10 doc... | 298 | 61 | 2,011,655 |
| 2026-10-01 | `a5d5f1e190` | docs(nee_soon): correct stale and source-contradicted HTR-10 ... | 436 | 147 | 3,571,009 |
| 2026-10-01 | `bb42dc4366` | fix(nee_soon): follow-ups from the #428 doc pass | 29 | 18 | 6,791,038 |
| 2026-10-01 | `4f02d6180b` | fix(digital-twin): IAEA-TECDOC-1382 access tier is Proprietar... | 14 | 5 | 6,397,139 |
| 2026-10-01 | `12f7f61c73` | feat(outram-mc-libs): HTR-10 boron impurity is atom ppm of na... | 168 | 45 | 18,341,900 |
| 2026-10-01 | `ebbcb62b96` | data: neutron He-3/He-4 ENDF/B-VIII.0 tapes; ACE submodule ca... | 3,146 | 2 | 19,266,239 |
| 2026-10-01 | `b554a3660c` | feat(nee_soon): HTR-10 natural carbon, helium coolant and rea... | 8,499 | 2,324 | 0 |
| 2026-10-01 | `4171473f48` | fix(njoy): LRF=7 four-channel inverse and RECONR guards; Fe-5... | 131,001 | 37 | 0 |
| 2026-10-01 | `8c80ab6773` | fix(nee_soon): enable the full Ni/Fe rod-metal case: Fe-57 re... | 14 | 10 | 5,683,742 |
| 2026-10-01 | `21c4b58c61` | nee_soon V&V: k-vs-height plot script for the Seker 13-ball H... | 145 | 0 | — |
| 2026-10-01 | `bf1e859a02` | nee_soon V&V: HTR-10 ENDF/B-VIII.0 k vs height on Seker's bed... | 318 | 0 | — |
| 2026-10-01 | `f888cfd98e` | nee_soon V&V: HTR-10 ENDF/B-VII.0 arm on Seker's bed (N = 10/... | 369 | 0 | — |
| 2026-10-02 | `229a70d770` | nee_soon V&V: HTR-10 k vs height at the paper's statistics (1... | 2,664 | 0 | 28,753,761 |
| 2026-10-02 | `63460359d8` | nee_soon: regenerate htr10_python_plots for the 13-ball bed; ... | 99 | 93 | 6,801,919 |
| 2026-10-02 | `bcac8b58a0` | outram-mc V&V: re-measure ring-RPT vs OpenMC and the DH treat... | 252 | 0 | 31,602,454 |
| 2026-10-02 | `74f5585b40` | dwsim-libs: conversion reactor code-to-code vs upstream; port... | 733 | 49 | — |
| 2026-10-02 | `96643feee1` | dwsim-libs: PFR code-to-code vs upstream; port heterogeneous-... | 1,207 | 18 | — |
| 2026-10-02 | `b2f235b991` | dwsim-libs: equilibrium reactor code-to-code vs upstream; fix... | 481 | 3 | — |
| 2026-10-02 | `81f2c517db` | dwsim-libs: Gibbs reactor code-to-code vs upstream (GibbsMin ... | 646 | 2 | — |
| 2026-10-02 | `6d2a1a8c07` | dwsim-libs docs: reactor code-to-code results, root-free upst... | 83 | 8 | — |
| 2026-10-02 | `93bce110dd` | dwsim-libs V&V: reactors vs upstream DWSIM record (methodolog... | 92 | 0 | — |
| 2026-10-02 | `e4ff527d29` | dwsim-libs: sour water and sour gas code-to-code vs upstream ... | 896 | 2 | — |
| 2026-10-02 | `ba3c3e848e` | LCG -> petir (Rust + WGSL), CSG graph adapter -> outram-blend... | 1,451 | 288 | — |
| 2026-10-02 | `3a6c1458a0` | outram-blender: wgpu behind a default-on `gpu` feature (issue... | 66 | 32 | — |
| 2026-10-02 | `785fcd1ce5` | outram-mc generic statistics -> raffles::estimators / distrib... | 1,100 | 264 | 0 |
| 2026-10-02 | `aee3a7371f` | docs: regenerate API mirrors for raffles, outram-mc-libs, out... | 26,074 | 8,744 | — |
| 2026-10-02 | `6a883a7e59` | outram-mc docs: GPU test SIGSEGV under parallel threads is a ... | 22 | 0 | 146,832,174 |
| 2026-10-02 | `c2b5000db7` | outram-mc: pin #486 geometry-move fingerprints (CSG+lattice, ... | 417 | 0 | — |
| 2026-10-02 | `185a67c635` | changi: port FLEXPART stage-1 physics, verified code-to-code ... | 14,558 | 45 | — |
| 2026-10-02 | `aa0bc2d263` | kovan-literature: bump reactor-literature to ceffde2 (Conlin ... | 10 | 1 | 106,523,666 |
| 2026-10-02 | `af84affcc7` | njoy: record that the NJOY2016 manual is open literature (LAN... | 30 | 1 | 18,673,731 |
| 2026-10-02 | `08247c50e0` | changi: FLEXPART stages 2-7 ported and verified code-to-code ... | 62,613 | 215 | — |
| 2026-10-02 | `bf5a36ea2a` | changi: stochastic comparison of convective redistribution (#... | 604 | 1 | — |
| 2026-10-02 | `a0077d6282` | deps: fission-yields-data 0.1.4 -> 0.1.5 (uom 0.38); removes ... | 34 | 44 | 50,308,779 |
| 2026-10-02 | `c02638e321` | changi: WIP snapshot of FLEXPART wave-2 ports in progress (#4... | 22,220 | 0 | — |
| 2026-10-02 | `ca0e50e236` | changi: integrate FLEXPART outgrid/fluxes/initial_cond ports,... | 2,330 | 875 | — |
| 2026-10-02 | `319a2ba814` | changi: WIP snapshot of FLEXPART wave-2 ports in progress (#4... | 10,655 | 2,327 | — |
| 2026-10-02 | `3b7207da0b` | changi: integrate FLEXPART concoutput/timemanager ports, veri... | 2,687 | 4 | — |
| 2026-10-02 | `4bf2ba9c4c` | changi: WIP snapshot of verttransform port in progress (#410)... | 107 | 37 | — |
| 2026-10-02 | `37addf9bd0` | kovan-literature: bump reactor-literature to 03afc72 (Kabach ... | 10 | 1 | 12,394,095 |
| 2026-10-02 | `e3c936369d` | changi: integrate FLEXPART verttransform/shift_field ports, v... | 680 | 2 | — |
| 2026-10-02 | `7689fcbd21` | changi: WIP snapshot of release/domainfill port in progress (... | 511 | 262 | — |
| 2026-10-02 | `cfd364bb6e` | changi: integrate FLEXPART releaseparticles/domainfill ports,... | 937 | 1 | — |
| 2026-10-02 | `5b4d733027` | changi docs: FLEXPART not-ported list now only the I/O reader... | 12 | 9 | — |
| 2026-10-02 | `7bd0197154` | changi docs: module table and not-ported statements updated f... | 18 | 6 | — |
| 2026-10-02 | `6b57374b8e` | outram-blender: wasm-clean faer, workspace entry default-feat... | 48 | 12 | 21,603,866 |
| 2026-10-02 | `9dc9107d4a` | Retire outram-blender's mc-export: sim, to_mc_geometry, mc_go... | 361 | 285 | 0 |
| 2026-10-02 | `cdfb6f49f4` | Move CSG position, surfaces and cells to outram_blender::csg;... | 3,165 | 2,747 | 0 |
| 2026-10-02 | `425f8fdd1a` | Move universe, lattice, geometry navigation and TRISO builder... | 3,550 | 3,208 | 2,120,319 |
| 2026-10-02 | `886071480c` | Move the tally-mesh description to outram_blender::spatial_me... | 557 | 431 | 0 |
| 2026-10-02 | `a7c6b4bb8f` | Move the OpenMC-port geometry plotter to outram_blender::csg:... | 837 | 597 | 0 |
| 2026-10-02 | `873d82ea73` | outram-blender: native CSG export (export::to_csg_geometry) (... | 183 | 94 | 0 |
| 2026-10-02 | `35b2167886` | Docs and roster for the #486 geometry move (#486 stage 7) | 20,604 | 17,116 | 0 |
| 2026-10-02 | `fdb6c9ddca` | bedok: gate slow tests behind long-tests (#503); add CLAUDE.md | 89 | 0 | — |
| 2026-10-02 | `6b52f85854` | outram-blender csg: an empty region is all space, as in OpenM... | 76 | 7 | 34,662,284 |
| 2026-10-02 | `eb5ee78b69` | nee_soon: HTR-10 ENDF/B-VIII.0 k-vs-height examples (quick + ... | 3,128 | 108 | 0 |
| 2026-10-03 | `fc88426145` | nee_soon: HTR-10 ENDF/B-VIII.0 k-vs-height heavy sweep record... | 874 | 5 | — |
| 2026-10-03 | `7d1032d655` | outram-blender: neutral unstructured mesh + converters and me... | 2,967 | 4 | 76,419,744 |
| 2026-10-03 | `ccbb04c538` | MeshKind::Unstructured + unstructured-mesh tally scoring in o... | 566 | 19 | 0 |
| 2026-10-03 | `5e921dc5a4` | Neutral mesh: winding-number cells (non-star cfMesh duals), m... | 407 | 111 | 0 |
| 2026-10-03 | `42010623c3` | Dedupe FvMesh: tampines-steam-tables and coolprop re-export o... | 114 | 5,928 | 0 |
| 2026-10-03 | `0c4a3f4cab` | Docs, V&V plan and API mirrors for the meshing nexus (#492) | 3,787 | 37 | 0 |
| 2026-10-03 | `dd0c30f22a` | outram-blender: state the cfMesh conversion test's pass crite... | 9 | 2 | — |
| 2026-10-03 | `334701ebdf` | outram-mc stats: seed-pooled ensembles as a library feature, ... | 898 | 47 | 0 |
| 2026-10-03 | `9ed437a3fa` | outram-mc stats: autocorrelation-corrected sigma for k_eff an... | 632 | 0 | 0 |
| 2026-10-03 | `7b8cf798bc` | outram-mc stats: source-convergence diagnostics and inactive-... | 710 | 0 | 0 |
| 2026-10-03 | `547134575a` | outram-mc stats: weight windows from learned importance (off ... | 751 | 3 | 0 |
| 2026-10-03 | `bb772cde43` | outram-mc stats: uncertainty quantification and sensitivity b... | 505 | 0 | 0 |
| 2026-10-03 | `429dc9bd85` | outram-mc stats: surrogate-accelerated parameter sweeps with ... | 472 | 0 | 0 |
| 2026-10-03 | `9f63db366d` | teaching site: backend GitHub Pages scaffold (gh:#518) | 301 | 6 | 256,853,309 |
| 2026-10-03 | `2c5523beb4` | teaching site: Monte Carlo, dispersion and TRISO-ATOPS deep d... | 2,802 | 47 | 0 |
| 2026-10-03 | `460ef585a8` | teaching site: state that the site tracks develop, never main... | 19 | 0 | 3,206,449 |
| 2026-10-04 | `88b681bf0f` | dhoby-ghaut: TRISO pebble demo, one neutron at a time on real... | 2,262 | 9 | — |
| 2026-10-04 | `49f5e50f0d` | pages: publish the TRISO pebble demo on the backend GitHub Pa... | 65 | 8 | — |
| 2026-10-04 | `43304617e1` | dhoby-ghaut: TRISO pebble demo processes ENDF in a Web Worker... | 816 | 269 | — |
| 2026-10-04 | `235bdfcc3a` | dhoby-ghaut triso_pebble_web: default 7 cm/s flight speed, fi... | 2 | 2 | 16,568,933 |
| 2026-10-04 | `06500e3c43` | Link the TRISO pebble demo and the Monte Carlo deep dive to e... | 16 | 0 | 911,811 |
| 2026-10-04 | `bfeb81a083` | Hard rule: tutorials and demos are mobile-first; TRISO demo g... | 118 | 7 | 2,425,606 |
| 2026-10-04 | `3c41d99f5b` | Lesson philosophy: question chain, rung ladder, code walks, h... | 199 | 0 | 35,455,039 |
| 2026-10-04 | `7e9b8bd637` | outram-mc-libs: LCT-008 pin-cell pitch sweep for tutorial run... | 675 | 2 | — |
| 2026-10-04 | `5e802df3a4` | Hard rule: demos never lag — computation runs in background w... | 40 | 3 | 15,768,881 |
| 2026-10-04 | `41cca8a99e` | kovan-cli code-walk: call chains and call trees over rust-ana... | 3,095 | 8 | 0 |
| 2026-10-04 | `d4428668be` | Tutorial rung 4 lesson draft; correct LCT-008's confounded #1... | 452 | 2 | — |
| 2026-10-04 | `c199b7dc1f` | outram-mc-libs: rung-2 uranium-in-graphite four-factor exampl... | 1,159 | 153 | — |
| 2026-10-04 | `9de3bd202b` | Tutorial rung 4 code walks for the LCT-008 lattice path (#526) | 621 | 0 | — |
| 2026-10-04 | `caad02d52a` | outram-mc-libs: PowerIteration steps run_keff one generation ... | 487 | 95 | — |
| 2026-10-04 | `ab6933100b` | dhoby-ghaut: one multi-rung Monte Carlo web demo (godiva, triso) | 3,005 | 851 | — |
| 2026-10-04 | `bd5ad1648e` | Pages: the Godiva tutorial lesson and the Monte Carlo demo on... | 980 | 14 | — |
| 2026-10-04 | `499c7b2871` | Monte Carlo demo: the UI never waits on the engine's mailbox;... | 7 | 3 | — |
| 2026-10-04 | `43832d173b` | outram-mc-libs: rung-3 lumped natural-U cell k_inf example, p... | 878 | 320 | — |
| 2026-10-04 | `ab686b7564` | Godiva lesson: display equations fit a phone; the browser run... | 17 | 6 | — |
| 2026-10-04 | `d22fd88e6b` | kovan-cli code-walk: rust-analyzer integration test, retries,... | 735 | 37 | — |
| 2026-10-04 | `38269756e1` | build-pages: run kovan-cli code-walk-check over the deep dive... | 22 | 0 | — |
| 2026-10-04 | `5f6a5157b0` | lesson-philosophy: code-walk has landed; strike "kopitiam cal... | 2 | 1 | — |
| 2026-10-04 | `6f5465729c` | Monte Carlo demo: speed note uses glyphs egui's font has; ent... | 7 | 4 | — |
| 2026-10-04 | `3af9a9b753` | dhoby-ghaut: the web-demo framework moves into the library; M... | 2,281 | 1,468 | — |
| 2026-10-04 | `569929fb93` | Godiva lesson: code walks generated by kovan-cli code-walk; t... | 146 | 52 | 0 |
| 2026-10-04 | `2871edd2b0` | outram-mc-libs: re-measure the rung-2/3 checks before the les... | 192 | 11 | — |
| 2026-10-04 | `3f68150cf2` | Dispersion lessons (#530): conservation checks, rung examples... | 26,028 | 555 | — |
| 2026-10-04 | `0b289061d7` | Dispersion deep dive (#530): seven-rung core ladder, extended... | 3,488 | 47 | — |
| 2026-10-04 | `3977e43769` | pages: publish sembawang's rustdoc, linked from the dispersio... | 1 | 0 | — |
| 2026-10-04 | `1e95aab3b8` | outram-mc-libs: code walks for tutorial rungs 2 and 3 (#524, ... | 758 | 0 | — |
| 2026-10-04 | `5827847982` | boon-lay: lesson examples (TRISO slice, layer D table); re-me... | 488 | 13 | 0 |
| 2026-10-04 | `b2c243d7f1` | boon-lay: core lessons as a tutorial book (rungs 1-8) and an ... | 9,442 | 108 | — |
| 2026-10-04 | `0b2e4210cd` | pages: publish the boon-lay tutorial book (tutorials/triso-at... | 2 | 0 | — |
| 2026-10-04 | `5fec1453c0` | njoy: fix stale doc claims, record re-measurements, add lesso... | 181 | 39 | — |
| 2026-10-04 | `fab052fcff` | Nuclear data deep dive: rungs 0-9, extended pages, coverage, ... | 3,517 | 0 | 0 |
| 2026-10-04 | `ed21a32b98` | pages: publish the nuclear data deep dive (deep-dives/nuclear... | 9 | 2 | — |
| 2026-10-04 | `d3ff1e8f27` | njoy: regenerate the API mirror for the corrected doc comment... | 34 | 20 | — |
| 2026-10-04 | `8bc3e11a54` | Tutorial rung 4: pitch-sweep data from outram-mc and OpenMC, ... | 124 | 4 | — |
| 2026-10-04 | `d71ccea489` | Tutorial rung 4: pitch-sweep results; borated LCT-008 is over... | 242 | 10 | — |
| 2026-10-04 | `e67e4df59f` | pages: let a running deploy finish instead of cancelling it | 5 | 1 | 23,565,903 |
| 2026-10-04 | `9ca1bea050` | pages: detect a working rust-analyzer, not rustup's stub, bef... | 5 | 2 | 3,172,899 |
| 2026-10-04 | `5fed44eb36` | boon-lay tutorial: the dispersion link points at deep-dives/d... | 2 | 2 | — |
| 2026-10-04 | `4fd79f11d3` | Dispersion lessons (#530): every rung links its demo rung; st... | 26 | 36 | — |
| 2026-10-04 | `af66994a27` | dispersion_web: the dispersion track's demo, one app with sev... | 2,240 | 1 | — |
| 2026-10-05 | `2e7f73ea05` | dispersion demo: make web/dispersion/build.sh executable (the... | 0 | 0 | — |
| 2026-10-05 | `7d0d63ab62` | kovan code-walk: rustdoc path links in doc sentences become p... | 44 | 1 | 10,986,739 |
| 2026-10-05 | `743461b5d2` | Godiva lesson: regenerate the code-walk tree with plain-text ... | 3 | 3 | 7,226,375 |
| 2026-10-05 | `2c4031f178` | nuclear_data_web: the nuclear data track's demo, seven rungs ... | 1,908 | 21 | — |
| 2026-10-05 | `08b16274de` | Nuclear data book: regenerate code walks with the fixed rende... | 6 | 6 | — |
| 2026-10-05 | `333e898803` | pages: publish the nuclear data demo (demos/nuclear-data/) (#... | 44 | 1 | — |
| 2026-10-05 | `6c3a228d42` | Monte Carlo demo rung `lct008` and the rung-4 lesson page (#526) | 1,257 | 60 | — |
| 2026-10-05 | `df82b02581` | nuclear data demo: make web/nuclear_data/build.sh executable ... | 0 | 0 | — |
| 2026-10-05 | `3cf81853b7` | pages: run demo build scripts through bash; nuclear_data buil... | 6 | 3 | 7,048,750 |
| 2026-10-05 | `1f7c967834` | outram-mc-libs: rung-2 main case recorded, OpenMC agrees; com... | 356 | 160 | — |
| 2026-10-05 | `2d9ca7588f` | outram-mc-libs: regenerate the rung 2/3 code walks after the ... | 136 | 145 | — |
| 2026-10-05 | `f39501b8bd` | outram-mc-libs: S(a,b) emission-tabulation k-worth re-measure... | 51 | 2 | — |
| 2026-10-05 | `de3ca762be` | CLAUDE.md: strike the stale "broadening not verified against ... | 9 | 2 | — |
| 2026-10-05 | `9dc2cc96cc` | njoy: WMP U-238 pole count corrected; MODER compared with NJO... | 12,408 | 28 | — |
| 2026-10-05 | `e889dd0f94` | njoy: mark the unsourced U-238 RI_inf 275.7 b / 277 +/- 3 b N... | 48 | 20 | — |
| 2026-10-05 | `1b9d06e609` | njoy GAMINR: the MF=26 disagreement on real uranium was NJOY'... | 507 | 32 | — |
| 2026-10-05 | `6faff1ed85` | njoy HEATR: MT=445 sub-threshold damage diagnosed as disbar's... | 127 | 19 | — |
| 2026-10-05 | `01c3181823` | triso_atops_web: the boon-lay track's demo, eight rungs, ever... | 3,344 | 37 | — |
| 2026-10-05 | `a6fa8400d1` | dispersion_web: the six step animations as ?step= modes of th... | 1,071 | 34 | — |
| 2026-10-05 | `7691c4d580` | outram-mc-libs: measured <ln E/E'> for H-1 and O-16 against a... | 284 | 7 | — |
| 2026-10-05 | `8b17079bc5` | outram-mc-libs: --full-nuclides arm on the five-route driver'... | 32 | 4 | — |
| 2026-10-05 | `bd0852b504` | njoy HEATR H6a: MT=442 matches NJOY2016 at print precision; d... | 75,649 | 1,269 | — |
| 2026-10-05 | `1dc7860d16` | outram-mc-libs: rung-2 step-4a at-rest ablation measured; gra... | 217 | 8 | — |
| 2026-10-05 | `00fa612f4e` | Monte Carlo tutorial: rung 2 (uranium in graphite) and rung 3... | 1,070 | 6 | — |
| 2026-10-05 | `148be19d3e` | Monte Carlo demo: rungs ugraphite and lumped (Watch), menu en... | 769 | 7 | — |
| 2026-10-05 | `5b27c6c927` | njoy: Tape::write is byte-faithful to NJOY's MODER and keeps ... | 1,062 | 139 | — |
| 2026-10-05 | `3e46c23a62` | njoy: GROUPR tier 2 re-measured; it now equals tier 1 and is ... | 39 | 12 | — |
| 2026-10-05 | `bd92475ddc` | docs: regenerate stale code walks; fix call-trees.md photon path | 295 | 241 | — |
| 2026-10-05 | `373a1358b3` | Monte Carlo tutorial: regenerate code walks for rungs 2-4 aft... | 743 | 329 | — |
| 2026-10-05 | `4a48daabfe` | njoy HEATR H6b part 1: two-body neutron side follows nheat's ... | 878 | 139 | — |
| 2026-10-05 | `8acb34f8ac` | Pages: Home breadcrumb and an Up link on every page | 144 | 2 | — |
| 2026-10-05 | `bd05a299c8` | outram-mc-libs: rung-3 follow-up prediction at N_C/N_U = 200,... | 18 | 0 | — |
| 2026-10-05 | `9a8543a59a` | outram-mc-libs: Godiva drift gate re-pointed to -6 +/- 5 pcm,... | 568 | 66 | — |
| 2026-10-05 | `31f8efceaf` | njoy HEATR: energy balance no longer clamped at 0, as in heat... | 114 | 30 | — |
| 2026-10-05 | `f1163948a1` | outram-mc-libs: rung-2 step-7 sweep recorded: homogeneous nat... | 129 | 5 | — |
| 2026-10-05 | `483473a059` | outram-mc-libs: rung-3 scan recorded: lumps lift k_inf 0.776 ... | 215 | 24 | — |
| 2026-10-05 | `28778c1fe4` | outram-mc-libs: graphite S(a,b) worth on the rung-2 main case... | 196 | 148 | — |
| 2026-10-05 | `848f83f126` | dhoby-ghaut web_demo: keep the canvas at device-pixel size; u... | 96 | 5 | — |
| 2026-10-05 | `692542573e` | njoy HEATR: translate the whole of heatr.f90 as a tape-to-tap... | 7,029 | 0 | — |
| 2026-10-05 | `08492f92e7` | njoy HEATR driver: hconvr sets the module za/awr as upstream ... | 123 | 0 | — |
| 2026-10-05 | `eb09b0ba08` | njoy Tape: read gzipped tapes; end each MAT occurrence with F... | 28 | 6 | — |
| 2026-10-05 | `827c38e333` | njoy HEATR driver: byte-identical to NJOY2016 on 62 evaluatio... | 603 | 9 | — |
| 2026-10-05 | `14229f5838` | njoy ACE heating from the HEATR translation, as acefc.f90 com... | 81 | 93 | — |
| 2026-10-05 | `d475c7543d` | njoy HEATR docs: V&V section 6, audit result, stale claims st... | 229 | 26 | — |
| 2026-10-05 | `c0444c981c` | njoy API mirror regenerated (heatr::heatr, HeatrInput, heatr_... | 596 | 7 | — |
| 2026-10-05 | `49791c7770` | outram-mc-libs: bare-sphere initial source aims each neutron ... | 132 | 51 | — |
| 2026-10-05 | `dc83e0d16e` | njoy docs: code walks regenerated after the HEATR port; full-... | 99 | 150 | — |
| 2026-10-05 | `41b4fd461c` | kovan code-walk: show each hop's code inline in lessons; walk... | 2,862 | 363 | — |
| 2026-10-05 | `a8c4f045aa` | dhoby-ghaut TRISO-ATOPS demo: living diagram of the primary-c... | 297 | 18 | — |
| 2026-10-05 | `e5619e28b6` | outram-mc-libs: LCT-008 lattice, worth of the 24 dropped nucl... | 115 | 9 | — |
| 2026-10-05 | `61adc0a4ae` | outram-mc-libs: rung-5 predictions before re-measuring HTR-10... | 76 | 0 | — |
| 2026-10-05 | `a01d37ae60` | TRISO-ATOPS demo: every inventory nuclide through the primary... | 679 | 343 | — |
| 2026-10-05 | `6a418b8301` | dhoby-ghaut: guided high-fidelity workbench, first runnable d... | 4,907 | 10 | 154,696,432 |
| 2026-10-05 | `ad7bc0b629` | dhoby-ghaut: the workbench is the crate's binary, `--bin dhob... | 43 | 38 | 8,173,874 |
| 2026-10-05 | `09c07b9f32` | TRISO-ATOPS demo: primary-circuit leak on by default, at HTR-... | 15 | 3 | — |
| 2026-10-05 | `70873b7733` | TRISO-ATOPS demo: stamp the build into every file URL, so a d... | 34 | 6 | — |
| 2026-10-05 | `f0d701bfcc` | outram-mc-libs: htr10_fuel_zone_kinf on ENDF + graphite S(a,b... | 378 | 106 | — |
| 2026-10-05 | `c5a6ca4ae3` | outram-mc-libs: record HTR-10 fuel-zone k_inf on ENDF + graph... | 175 | 29 | — |
| 2026-10-05 | `7d2a0f3d53` | Monte Carlo demo: lct008 pitch slider (streamed pin-cell k_in... | 1,531 | 115 | — |
| 2026-10-05 | `258d4a3a7a` | docs: human review is the bottleneck; index of review practic... | 175 | 0 | — |
| 2026-10-05 | `1db67becdc` | outram-mc-libs: htr10_fuel_zone_kinf records cite the commits... | 4 | 4 | — |
| 2026-10-05 | `a2e98340d1` | Merge origin/develop; regenerate code walks shifted by transp... | 566 | 395 | — |
| 2026-10-05 | `18c65c5e53` | dhoby-ghaut: 3D section viewport, live k_eff console, file pi... | 7,321 | 416 | 183,444,373 |
| 2026-10-05 | `169062090c` | Merge origin/develop into claude/neutronics-runs-handoff-8xk972 | 4 | 4 | — |
| 2026-10-05 | `09f40e3786` | outram-mc-libs tutorial: code walks regenerated after #549 mo... | 279 | 279 | — |
| 2026-10-05 | `1513b1df5d` | outram-mc-libs: tutorial rung 5, TRISO lumps inside lumps end... | 1,091 | 4 | — |
| 2026-10-05 | `aa58f8aa47` | boon-lay: triso_atops_extensions, element-dependent removal r... | 813 | 0 | — |
| 2026-10-05 | `beb9595b2c` | Pages: credits and upstream code on the home page | 1,036 | 2 | — |
| 2026-10-05 | `59ededa3aa` | TRADEMARKS: correct the NJOY crate name and the PFLOTRAN licence | 9 | 4 | — |
| 2026-10-05 | `73812beeb2` | outram-mc-libs tutorial: code walks regenerated after 18c65c5... | 296 | 296 | — |
| 2026-10-05 | `1ef88ff1df` | boon-lay deep dive: call-tree walks regenerated (11 stale; RA... | 287 | 116 | — |
| 2026-10-05 | `af0a76e910` | boon-lay: document triso_atops_extensions as an extension; de... | 2,084 | 297 | — |
| 2026-10-05 | `45ceef0554` | dhoby-ghaut: fresh DEM pour in Step 1; ENDF libraries with su... | 1,164 | 49 | 106,040,037 |
| 2026-10-05 | `a884e4a8cb` | Pages: links into a demo open in a new tab | 54 | 0 | — |
| 2026-10-05 | `a81ecaf9e8` | Step 1 DEM pour builds the Monte Carlo core; random pour seeding | 10,683 | 1,636 | 49,438,640 |
| 2026-10-05 | `4f266c9e5e` | TRISO-ATOPS demo: fuel-side pools drawn from the particle's l... | 138 | 67 | — |
| 2026-10-05 | `282d22d35d` | outram-mc-libs: Majorant::bounding tabulated on every nuclide... | 745 | 147 | — |
| 2026-10-05 | `9bd5a67b82` | outram-mc-libs tutorial: code walks regenerated after the #58... | 95 | 80 | — |
| 2026-10-05 | `149f7aff0b` | outram-mc-libs: WMP pole nodes in the majorant; caller audit ... | 232 | 56 | — |
| 2026-10-05 | `d680b9fd1e` | outram-mc-libs: DeltaPowerIteration (delta-tracked keff, one ... | 340 | 139 | — |
| 2026-10-05 | `9b658b14c0` | Monte Carlo demo: the htr10 rung - zoom ladder, recorded laye... | 1,396 | 56 | — |
| 2026-10-05 | `a0509dd0e0` | htr10 rung on #585's Majorant::bounding: bound audited, numbe... | 123 | 104 | — |
| 2026-10-05 | `2ee2058bbf` | outram-mc-libs: htr10_fuel_zone_kinf confirmatory re-run on t... | 24 | 2 | — |
| 2026-10-05 | `f6c6fcd6d5` | Monte Carlo demo htr10: dark strip behind the zoom ladder so ... | 3 | 0 | — |
| 2026-10-05 | `1e9ee8b252` | Pages: home page links rung 5 (TRISO & HTR-10) and the htr10 ... | 5 | 3 | — |
| 2026-10-05 | `b208cecc7e` | outram-mc-libs: from_materials / over_indices add every nucli... | 223 | 40 | — |
| 2026-10-05 | `e09030a3cf` | outram-mc-libs: stats_move_fingerprints' thermal cases are li... | 89 | 7 | — |
| 2026-10-05 | `28eb371b92` | nee_soon: HTR-10 k-vs-height re-measurement on the bounded ma... | 261 | 1 | — |
| 2026-10-05 | `977221339c` | nee_soon: HTR-10 bounded-majorant re-measurement, VIII.0 N = ... | 389 | 0 | — |
| 2026-10-05 | `55744e459d` | nee_soon: HTR-10 bounded-majorant re-measurement, VIII.0 N = ... | 209 | 0 | — |
| 2026-10-05 | `ddfcd2319f` | nee_soon: HTR-10 bounded-majorant re-measurement, VIII.0 N = ... | 97 | 0 | — |
| 2026-10-05 | `f4fd368ae9` | dhoby-ghaut workbench: Step 6 reactivity map (low-fidelity su... | 2,352 | 21 | — |
| 2026-10-05 | `23bdb60457` | nee_soon: HTR-10 bounded-majorant re-measurement, VIII.0 N = ... | 97 | 0 | — |
| 2026-10-05 | `31b20d5b2b` | dhoby-ghaut Step 6: warn when too few runs back the held-out ... | 24 | 0 | — |
| 2026-10-05 | `6c148f7bcd` | nee_soon: HTR-10 bounded-majorant re-measurement, VIII.0 tabl... | 111 | 16 | — |
| 2026-10-05 | `10237f9593` | dhoby-ghaut Steps 9-10: multiphysics case setup and a simplif... | 11,269 | 17 | — |
| 2026-10-05 | `399ab135e5` | Workbench: rod insertion, ENDF folder load, design edits rebu... | 1,892 | 314 | — |
| 2026-10-05 | `fdedfcd3f4` | dhoby-ghaut: Step 6 runs on the Step 0 tape folder and the re... | 16 | 6 | 3,016,460 |
| 2026-10-05 | `a24690aed8` | dhoby-ghaut workbench Steps 7 (meshes, regions, mesh-to-mesh ... | 4,480 | 13 | — |
| 2026-10-05 | `19896403a5` | nee_soon: HTR-10 bounded-majorant re-measurement, VII.0 N = 1... | 117 | 3 | — |
| 2026-10-05 | `29fc77aaee` | dhoby-ghaut: ray tracing on the GPU (gh:#587) | 3,930 | 95 | — |
| 2026-10-05 | `9bfafc498e` | dhoby-ghaut Steps 7-8: V&V record (meshes, maps, 2-state MGXS... | 716 | 0 | — |
| 2026-10-05 | `261a2f9e80` | dhoby-ghaut Step 10: solve the power by diffusion on Steps 7-... | 2,630 | 294 | — |
| 2026-10-05 | `b414ce4450` | dhoby-ghaut: htr10 layers view shows the #589 re-measurement ... | 27 | 7 | — |
| 2026-10-05 | `3c22c7e63f` | nee_soon: HTR-10 bounded-majorant record finalised, with cont... | 415 | 20 | — |
| 2026-10-05 | `d9ab6c2627` | GPU CSG tracer: grid index, closed root grid; DEM pour as sph... | 2,432 | 155 | — |
| 2026-10-06 | `93f3cf5ab8` | dhoby-ghaut: style system (#586) - one type scale, widths in ... | 346 | 80 | 15,803,829 |
| 2026-10-06 | `9a9e9fd60d` | dhoby-ghaut Step 10 V&V: solved power on HTR-10, bed-flux def... | 26,632 | 74 | — |
| 2026-10-05 | `51d1f57ee5` | outram-mc-libs: LCT-008 full-model seeds 17-19 and a 1-thread... | 4 | 0 | — |
| 2026-10-05 | `6f4e315f59` | outram-mc-libs: LCT-008 full-model seeds 21-23, partial (#533) | 3 | 0 | — |
| 2026-10-06 | `169bc85073` | outram-mc-libs: delta-tracked tallies use the tentative-colli... | 875 | 27 | — |
| 2026-10-05 | `7bda595e15` | outram-mc-libs: LCT-008 full-model seeds 20 and 24-27, partia... | 5 | 0 | — |
| 2026-10-06 | `ef94fb5e3a` | dhoby-ghaut: re-run Step 8 and Step 10 on the gh:#598-correct... | 38,306 | 13,181 | — |
| 2026-10-05 | `41790c136c` | outram-mc-libs: LCT-008 full-model seeds 28-31, partial (#533) | 4 | 0 | — |
| 2026-10-05 | `0438a1c0f9` | outram-mc-libs docs: code walks regenerated after #598 moved ... | 427 | 395 | — |
| 2026-10-05 | `d3f2ff966b` | outram-mc-libs: LCT-008 dropped-nuclide worth at 32 full-mode... | 117 | 24 | — |
| 2026-10-06 | `83025d8761` | Agent worktrees: branch from develop, read reference-data fro... | 82 | 3 | 35,627,605 |
| 2026-10-06 | `6029a5f3d9` | outram-mc-libs: bit-identity gate for every delta-tracking pa... | 521 | 0 | — |
| 2026-10-06 | `9d6dd65f96` | outram-mc-libs: delta tracking gathered into physics::delta_t... | 2,387 | 1,829 | — |
| 2026-10-06 | `f08adf6456` | outram-mc-libs: a void cell inside a delta region is crossed,... | 78 | 32 | — |
| 2026-10-06 | `e85025c440` | outram-mc-libs: keff_delta collides at the material's tempera... | 545 | 6 | — |
| 2026-10-06 | `73aedcc0d2` | outram-mc-libs: count majorant violations and delta-lost hist... | 153 | 0 | — |
| 2026-10-06 | `fc9f35e69f` | outram-mc-libs docs: code walks regenerated after the delta-t... | 495 | 476 | — |
| 2026-10-06 | `1054ad64da` | docs: correct two claims the 2026-10-06 physics meta-audit fo... | 13 | 6 | — |
| 2026-10-06 | `44da5124a3` | dhoby-ghaut: report delta-tracking majorant violations and be... | 17 | 0 | 27,452,409 |
| 2026-10-06 | `8a0be031fe` | kovan code-walk: tree blocks in lessons show each function's ... | 167 | 6 | — |
| 2026-10-06 | `8c8382dc0c` | Code walks: every tree walk on the site now shows each functi... | 26,298 | 2,130 | — |
| 2026-10-06 | `b56cb6358b` | outram-mc-libs tutorial: tree walks inline, regenerated on to... | 3,358 | 237 | — |
| 2026-10-06 | `1707b99cf3` | outram-mc-libs lessons: API links follow the #718 delta-track... | 2 | 2 | — |
| 2026-10-06 | `8242607000` | reactor-literature: point at 62ff9af (six NRC guides in the s... | 1 | 1 | 140,285,804 |
| 2026-10-06 | `e8f2bc94be` | reactor-literature: point at cb3cb47 (standard corpus ground ... | 1 | 1 | 3,680,877 |
| 2026-10-06 | `9059e0a5ea` | kovan-common: the concept-tree skeleton, sources linked to th... | 657 | 0 | 27,621,976 |
| 2026-10-06 | `0d6c3198c9` | Move the concept-tree skeleton to kovan-literature, beside th... | 6 | 4 | 4,513,741 |
| 2026-10-06 | `9dd236ce9a` | Concept skeleton: fuel depletion under the fuel cycle; source... | 18 | 3 | 3,241,109 |
| 2026-10-06 | `d94959f4a8` | kovan-literature: concept-tree proposals, L3 concepts (194) a... | 4,676 | 0 | 21,767,186 |
| 2026-10-06 | `45506fe321` | Concept proposals: maintainer's rulings on the judgement call... | 100 | 31 | 12,261,635 |
| 2026-10-06 | `d77dae6003` | Concept proposals: aspiration links with stated prerequisites... | 63 | 1 | 9,724,855 |
| 2026-10-06 | `090ba7ddad` | Concept review §3.1 National position: four NUREG-1555 Ch. 8 ... | 48 | 3 | 4,889,129 |
| 2026-10-06 | `68d7b99bd5` | Concept review §3.2: design of structures (8) and fuel system... | 24 | 24 | 15,551,190 |
| 2026-10-06 | `f9f9fd7e49` | Concept review §3.2 nuclear design: three L3 groups (core con... | 126 | 61 | 20,953,013 |
| 2026-10-06 | `b62bdc3cb1` | Concept review §3.2: moderator and reflector (2, cross-linked... | 127 | 116 | 4,398,692 |
| 2026-10-06 | `08f3553c88` | Concept review §3.2 thermal-hydraulic design: MSR topics from... | 21 | 1 | 8,128,119 |
| 2026-10-06 | `589f044608` | Concept review §3.2: control rods (3), reactor coolant system... | 26 | 16 | 2,229,295 |
| 2026-10-06 | `072e54559a` | Concept review §3.2: engineered safety features (5), containm... | 22 | 22 | 4,476,661 |
| 2026-10-06 | `7552a9c1b3` | Concept review §3.2: steam and power conversion, conduct of o... | 24 | 20 | 2,250,131 |
| 2026-10-06 | `d75669447f` | Concept review §3.2: alternate power cycles (direct helium Br... | 70 | 5 | 16,680,797 |
| 2026-10-06 | `a4a66cba5d` | Concept review §3.2 accident analysis (12) approved; composed... | 27 | 13 | 9,971,411 |
| 2026-10-06 | `ba16c9f8c2` | Concept review §3.7 regulatory framework: QA (7, Part 50 App.... | 82 | 11 | 14,745,836 |
| 2026-10-06 | `5af871454c` | Concept review §3.8 radiation protection and §3.9 electrical ... | 70 | 9 | 9,456,842 |
| 2026-10-06 | `369a938d16` | Concept review §3.10 HR development (2) and §3.12 site and su... | 6 | 6 | 8,766,110 |
| 2026-10-06 | `22ab012346` | Concept review §3.13 environmental protection (12) approved; ... | 16 | 12 | 1,600,572 |
| 2026-10-06 | `109edaddb0` | Concept review §3.14 emergency planning: accident assessment ... | 5 | 5 | 2,407,123 |
| 2026-10-06 | `54f20e20b3` | Concept review §3.16 nuclear fuel cycle (8) approved; critica... | 10 | 8 | 1,608,084 |
| 2026-10-06 | `58252c027f` | Concept review §3.17 radioactive waste (6) and §3.18 industri... | 13 | 9 | 4,849,931 |
| 2026-10-06 | `d1a20bea1e` | Concept tree: NUTEC Plastics topic under process heat and ind... | 22 | 3 | 4,883,765 |
| 2026-10-06 | `b8c9314aa3` | Concept tree: 10 CFR 50/52/53 additions (36 new topics, propo... | 502 | 104 | 15,580,072 |
| 2026-10-06 | `c708605f25` | Concept tree: NUTEC Plastics cites the 2026 'Scaling up solut... | 5 | 5 | 9,147,972 |
| 2026-10-06 | `726819422f` | Concept review, 10 CFR additions: 36 approved; cybersecurity ... | 102 | 49 | 9,210,934 |
| 2026-10-06 | `baf26c3c42` | Concept tree consolidation: three merges (ALARA under protect... | 58 | 33 | 14,471,685 |
| 2026-10-06 | `7d820d49d2` | Concept skeleton: 12 L2 nodes for legal framework, stakeholde... | 79 | 1 | 24,401,783 |
| 2026-10-06 | `f852d91b7a` | concept_skeleton test: a skeleton node may cross-link to an L... | 3 | 1 | 7,949,979 |
| 2026-10-06 | `0099cf6384` | kovan, kovan-literature READMEs: the four-level concept tree ... | 135 | 0 | 7,991,977 |
| 2026-10-06 | `4b2dfa4745` | kovan-literature: typed concept-tree module (levels 1-3, appr... | 600 | 2 | — |
| 2026-10-06 | `470c744192` | Concept tree: three public concepts the maintainer's corpus n... | 35 | 1 | 75,888,750 |
| 2026-10-06 | `668aca85ec` | kovan ingest: open and proprietary literature kept strictly a... | 35 | 5 | 28,292,954 |
| 2026-10-06 | `fd573f323c` | kovan: the standard mind map is the concept tree, levels 1-3,... | 1,229 | 343 | 0 |
| 2026-10-06 | `9485de2510` | pasir-ris: placeholder crate, PASIR RIS (Probabilistic Assess... | 182 | 5 | 9,354,396 |
| 2026-10-06 | `b3edfc257c` | pasir-ris: backronym is Probabilistic Assessment of Safety In... | 11 | 12 | 859,368 |
| 2026-10-06 | `5bb1b11a32` | pasir-ris: README and CLAUDE.md line wrapping (#732) | 3 | 3 | 0 |
| 2026-10-06 | `d487dbee29` | kovan: ask where to keep the standard corpus and refresh it a... | 621 | 55 | 22,709,264 |
| 2026-10-06 | `025fc31717` | kovan: standard-corpus PDFs from the mind map in one click, a... | 148 | 75 | 27,380,082 |
| 2026-10-06 | `f7d7c87b8d` | pasir-ris: depends on every PSA-related crate across the thre... | 76 | 29 | 4,064,327 |
| 2026-10-06 | `e11a000e97` | Code map tags: row, topic and fidelity in every crate's [pack... | 597 | 0 | 14,192,329 |
| 2026-10-06 | `b97fbc43cb` | Code map tags: maturity on every crate (0 concept, 1 AI draft... | 296 | 54 | 11,065,835 |
| 2026-10-06 | `e838e86c37` | Code map tags: fidelity is an absolute scale (0 lumped, 1 one... | 338 | 167 | 4,598,035 |
| 2026-10-06 | `57d5b35285` | Code map tags: teh-o-prke maturity 1 (maintainer, 2026-10-06)... | 1 | 1 | 1,076,599 |
| 2026-10-06 | `bef7188cb3` | kovan code_map: model, layout and SVG of the workspace from i... | 1,549 | 106 | 0 |
| 2026-10-06 | `af64327c35` | kovan-cli code-map: write the code map as JSON or SVG (#734) | 77 | 0 | — |
| 2026-10-06 | `26e8917dfa` | Pages: the code map page and a preview on the main menu (#734) | 292 | 1 | — |
| 2026-10-06 | `7dc3ac3162` | kovan GUI: Code Map view (#734) | 598 | 1 | — |
| 2026-10-06 | `775d19980d` | kovan docs: the code map in README and DECISIONS (#734) | 66 | 0 | — |
| 2026-10-06 | `6aabc8627d` | kovan: hash-based human review stamps and stamps-check (#739) | 2,431 | 0 | 0 |
| 2026-10-06 | `9c989e5ccf` | kovan code-walk: keep every resolved call site, not only the ... | 11 | 4 | 0 |
| 2026-10-06 | `69143978d5` | kovan: call_graph model, crate -> module -> function with sou... | 994 | 0 | — |
| 2026-10-06 | `fe8f5ae1e2` | kovan-cli call-graph: the workspace call graph as determinist... | 838 | 0 | — |
| 2026-10-06 | `d12d5f57b4` | kovan docs: call-graph in README and DECISIONS (#737) | 103 | 0 | — |
| 2026-10-06 | `38acf36e54` | kovan docs: review stamps in README, DECISIONS and human-revi... | 90 | 0 | — |
| 2026-10-06 | `08a17e74d6` | review stamps: a re-review replaces the function's stamp in p... | 83 | 89 | 8,900,406 |
| 2026-10-06 | `6a2ae2f38a` | site: the code map as a navbar strip on every deep dive, tuto... | 386 | 2 | — |
| 2026-10-06 | `53b5518e86` | kovan call-graph schema 2: tests that reach a function, upstr... | 2,153 | 29 | 0 |
| 2026-10-06 | `27b45ad9d3` | kovan-common: wasm-clean code_map, call_graph, mindmap_view, ... | 1,522 | 721 | 0 |
| 2026-10-06 | `232e9f1ed0` | kovan-web: web-kovan, the read-only Code Review UI in egui fo... | 4,406 | 0 | — |
| 2026-10-06 | `89514e3518` | kovan: #743 schema additions for dogfooding, and a published ... | 1,584 | 42 | 0 |
| 2026-10-06 | `273cfc743b` | kovan-web: schema-2 hooks: tests that reach a function, upstr... | 82 | 9 | — |
| 2026-10-06 | `b1f5d4bf39` | web-kovan on Pages: code-review/ in build-pages.sh, increment... | 144 | 12 | — |
| 2026-10-06 | `28aefdffed` | kovan-web: the code map opens fitted to its height (everythin... | 29 | 6 | 12,800,759 |
| 2026-10-06 | `3bbd74887c` | pages: index the call graph within a 40-min budget and save t... | 29 | 3 | 5,020,742 |
| 2026-10-06 | `eff86f98a7` | kovan-web: left click on a function card opens its source pan... | 70 | 22 | 2,606,827 |
| 2026-10-06 | `6ee1bc2c8f` | kovan-web: a left click on a crate on the code map offers Mod... | 151 | 12 | 4,267,800 |
| 2026-10-06 | `48c8fb15db` | Pinch zoom on touch screens for the tutorial demos (dhoby_gha... | 31 | 9 | 1,638,214 |
| 2026-10-07 | `b142a065d2` | Relicense the whole kovan family to AGPL-3.0-only: kovan-comm... | 642 | 507 | 8,079,045 |
| 2026-10-07 | `8869ee6eb3` | kovan-common: port Hypothesis's annotation anchoring (W3C quo... | 2,524 | 0 | — |
| 2026-10-07 | `0efb38c279` | anchoring tests: case (b) pins its measured score instead of ... | 12 | 6 | — |
| 2026-10-07 | `3b146c83f5` | kovan-common, kovan-codegen: port Zotero's data model (#748, ... | 27,489 | 24 | 0 |
| 2026-10-07 | `7ea9ab77d5` | kovan-metrics: Zotero library counts and import report (#751,... | 1,317 | 0 | — |
| 2026-10-07 | `7a6f437be3` | kovan-literature: read a Zotero data folder (zotero.sqlite + ... | 12,453 | 16 | 0 |
| 2026-10-07 | `028de9bcc2` | kovan-semantics: port Zotero duplicates, relations and item m... | 5,102 | 0 | — |
| 2026-10-07 | `80b22f2377` | kovan-discovery: port Zotero's search engine over an in-memor... | 6,554 | 0 | — |
| 2026-10-07 | `510fce4c0b` | kovan-discovery: evaluate kovan-common's stored ZoteroSearch;... | 164 | 15 | — |
| 2026-10-07 | `6956689a7d` | kovan relation docs: a relation's source may be artifact: or ... | 14 | 8 | 10,410,416 |
| 2026-10-07 | `c86a5cfbfa` | kovan-literature: Zotero translation framework, CSL JSON tran... | 61,096 | 2 | — |
| 2026-10-07 | `2ac46c1c15` | kovan-literature: NOTICE for the Zotero translator port; reco... | 100 | 2 | — |
| 2026-10-07 | `597575a68c` | kovan-literature: port Zotero's RIS translator, import and ex... | 2,436 | 20 | — |
| 2026-10-07 | `8e076341dd` | kovan-literature: port Zotero's BibTeX (import + export) and ... | 4,369 | 30 | — |
| 2026-10-07 | `0c116d577d` | kovan-literature: merge fixes (duplicate pub mod zotero, dupl... | 79 | 6 | 3,417,576 |
| 2026-10-07 | `7ddcd1fbe1` | Zotero port: verify against the same Zotero the port follows;... | 146 | 1,090 | 13,316,583 |
| 2026-10-07 | `b633e56ba2` | kovan-literature: scaffolding for the #749 tagged-text, JSON ... | 70,393 | 17 | — |
| 2026-10-07 | `158e605eea` | kovan-literature: pure-Rust Zotero reader on turso_core, wasm... | 2,255 | 363 | — |
| 2026-10-07 | `e64e39cb25` | nee_soon V&V: start the 2026-10-07 HTR-10 re-measurement (sam... | 145 | 0 | — |
| 2026-10-07 | `96f13474be` | kovan-literature: XML DOM, XPath (wicked-good-xpath port), XM... | 39,342 | 7 | — |
| 2026-10-07 | `84a42aec4e` | kovan-literature: port Zotero's tagged-text, JSON and simple ... | 7,362 | 244 | — |
| 2026-10-07 | `8ce46cf443` | nee_soon: htr10_rmc_keff prints the majorant-violation and de... | 5 | 0 | — |
| 2026-10-07 | `dbb9e26eb1` | kovan-literature: port Zotero's XML translators and the note ... | 13,033 | 2,122 | — |
| 2026-10-07 | `f4bbdcb7ed` | kovan-literature: port Zotero's RDF translators (Zotero RDF, ... | 29,869 | 7 | — |
| 2026-10-07 | `e3b77ae783` | kovan: kovan-cli zotero import/export/formats/duplicates/sear... | 16,240 | 4,214 | 0 |
| 2026-10-07 | `bcf7614506` | kovan-literature RDF: parse RDF/XML on the framework's XML la... | 72 | 269 | — |
| 2026-10-07 | `2b0a01b96b` | kovan zotero CLI test: the note exporters (landed after the t... | 16 | 5 | 24,817,293 |
| 2026-10-07 | `9b8edd0334` | kovan zotero CLI test: Bibliontology RDF fails on an every-it... | 21 | 2 | 2,847,797 |
| 2026-10-07 | `0499147de0` | kovan-literature: identifier lookup framework (extractIdentif... | 6,408 | 3 | — |
| 2026-10-07 | `7664e3167d` | kovan-literature: search test options carry a placeholder Cam... | 10 | 1 | — |
| 2026-10-07 | `e0ca57597a` | kovan: Zotero identifier lookup, all 19 search translators, C... | 7,112 | 233 | 0 |
| 2026-10-07 | `65392cd6dc` | kovan-literature: keep the Zotero verification data and its t... | 9 | 0 | 11,670,066 |
| 2026-10-07 | `5d716aa93a` | kovan: publish as knowledge-oriented-vv-analysis-for-nuclear-... | 145 | 63 | 19,527,598 |
| 2026-10-07 | `6e13e7cc54` | kovan family: versions for the first AGPL publish: kovan-comm... | 52 | 52 | 9,816,090 |
| 2026-10-07 | `b10c434446` | kovan corpus: software QA sources NUREG/BR-0167 and DOE-STD-1... | 20,196 | 756 | 151,068,965 |
| 2026-10-07 | `0cf166bbd2` | nee_soon V&V: HTR-10 re-measured after #589/#719-#721 (same s... | 717 | 1 | — |
| 2026-10-07 | `31c5241754` | kovan corpus: DOE G 414.1-4 (DOE Safety Software Guide) in th... | 37 | 7 | 11,085,531 |
| 2026-10-07 | `299b1081dd` | kovan corpus: cite Kendrick 2019 (Nuclear QA in University Re... | 32 | 6 | 7,022,631 |
| 2026-10-07 | `c73539fa75` | kovan concept tree: SQA work activities and safety software Q... | 304 | 33 | 23,667,006 |
| 2026-10-07 | `57899429a8` | nee_soon V&V: HTR-10 2026-10-07 counter re-runs of VIII.0 N =... | 203 | 5 | — |
| 2026-10-07 | `0d5aa453f6` | CLAUDE.md: every function an agent writes or changes is reach... | 17 | 0 | 52,909,894 |
| 2026-10-07 | `8951659153` | petir: fast_log/fast_pow subnormal normalisation wraps explic... | 8 | 2 | — |
| 2026-10-07 | `e1cf7aaf79` | Release profile: integer overflow checks ON, with measured co... | 105 | 0 | — |
| 2026-10-07 | `2cdc0048ef` | petir ellint: count duplication steps per thread, not globall... | 15 | 6 | — |
| 2026-10-07 | `d79b240d12` | Kovan standard corpus: the NJOY2016 manual (LA-UR-17-20093), ... | 62 | 10 | 24,488,619 |
| 2026-10-07 | `a9c7a46685` | kovan-common: code review data model (kovan_root.toml, kovan.... | 4,343 | 1,021 | 0 |
| 2026-10-07 | `c5788e79c2` | kovan call-graph: SCIP backend (--backend scip / --scip <file... | 2,311 | 35 | 0 |
| 2026-10-07 | `64f2787f0c` | kovan-common: staleness engine and state enum; per-area revie... | 1,873 | 4 | — |
| 2026-10-07 | `9204b138e8` | Code map pinch zoom; big buttons between the JS code map and ... | 225 | 13 | 0 |
| 2026-10-07 | `a406bbf2bb` | kovan-common: hybrid fn: ids, commit-only upstream pins with ... | 1,046 | 176 | — |
| 2026-10-07 | `cecc8ceff6` | kovan: a rejected push merges and retries instead of leading ... | 889 | 78 | 0 |
| 2026-10-07 | `3b848ef82a` | farrer-park: depend on outram-foam-basic-lib through the work... | 3 | 1 | — |
| 2026-10-07 | `88c6493564` | Ship the GPL-3.0 LICENSE text in five never-published crates | 3,370 | 0 | — |
| 2026-10-07 | `c270b86ca0` | kovan-common: in-app ed25519 signing of review stamps (#762) | 2,614 | 41 | — |
| 2026-10-07 | `0ea9517b52` | kovan design principles: Leak Before Break (tamper-evident, n... | 34 | 1 | 46,119,698 |
| 2026-10-07 | `8595f17dd9` | Kovan standard corpus: 34 documents for #760 (IAEA-issue bedr... | 985 | 26 | 0 |
| 2026-10-07 | `825829636c` | kovan-common signing: explicit founder; retired keys cannot s... | 200 | 33 | — |
| 2026-10-07 | `f61882f629` | kovan-common: review wizard question set and stamp gate (#769) | 1,822 | 7 | 0 |
| 2026-10-07 | `f480289c52` | kovan: read code-review entries as artifacts and keep unknown... | 238 | 3 | — |
| 2026-10-07 | `c9371aab03` | kovan-cli test: record full-suite test evidence (#766) | 2,240 | 1 | 0 |
| 2026-10-07 | `5c725de551` | kovan scip: merge a file indexed twice instead of dropping a ... | 114 | 18 | — |
| 2026-10-07 | `4330f955db` | kovan-common: engine on the merged #762 signing and #769 wiza... | 198 | 60 | — |
| 2026-10-07 | `481cef79f3` | raffles: record the first measured result of transient_is_fou... | 17 | 2 | — |
| 2026-10-07 | `8667f3401b` | kovan scip: a same-file collision prefers the definition nest... | 37 | 12 | — |
| 2026-10-07 | `2cfe529cee` | kovan-cli test: relative --root gives workspace-relative sour... | 5 | 2 | — |
| 2026-10-07 | `aef8e9f92f` | kovan-common signing: append-only key history in kovan_root.t... | 1,432 | 452 | — |
| 2026-10-07 | `d08cf4af66` | kovan scip: items nested in another function are out of the r... | 53 | 12 | — |
| 2026-10-07 | `21094f6726` | WIP #765: kovan-web on the engine's StateKind; wizard: indepe... | 234 | 48 | — |
| 2026-10-07 | `813ff97737` | kovan call-graph: V&V note SCIP vs LSP on the whole workspace... | 4,651 | 1,028 | — |
| 2026-10-07 | `1385194b2c` | kovan-common: derived rung, invalid state, registry-only revi... | 464 | 218 | — |
| 2026-10-07 | `ce37f2ef2c` | wip #767: reach::tests_reaching, code_index ids draft | 488 | 0 | — |
| 2026-10-07 | `00cfc3c8d7` | Kovan: 26 software-properties sources in the standard corpus ... | 966 | 7 | 0 |
| 2026-10-07 | `0eaff3bdbc` | kovan-common code_index: ids, folders, refresh, heal, deleted... | 2,108 | 299 | — |
| 2026-10-07 | `f8fcf52fa9` | kovan-common engine: maintainer answers on #765 (rung at revi... | 399 | 45 | — |
| 2026-10-07 | `1bbd9d7b14` | kovan test evidence lives in kovan.toml [test_run] (code_inde... | 254 | 31 | — |
| 2026-10-07 | `96489dd5e5` | petir: two GSL table audits skip, as documented, when the ven... | 15 | 3 | — |
| 2026-10-07 | `6ef39c8a45` | raffles: centre y_B in the Sobol first-order estimator (gh:#584) | 47 | 14 | — |
| 2026-10-07 | `1821764b94` | outram-mc-libs: exact_attenuation_is_filled_exactly fits with... | 13 | 2 | — |
| 2026-10-07 | `99c959ce65` | wip #767: kovan-cli index command, physical interface tokens | 946 | 21 | — |
| 2026-10-07 | `6f58f9cbe2` | kovan-cli index: full run, --refresh without rust-analyzer, -... | 310 | 6 | 0 |
| 2026-10-07 | `057b12e8fa` | index: recover a malformed kovan.toml's ids and [test_run] fr... | 437 | 27 | — |
| 2026-10-07 | `eb12f626ff` | Pages: web-kovan call graph from one rust-analyzer scip index... | 329 | 74 | 0 |
| 2026-10-07 | `4a55ecf702` | code_index: clippy, API mirror, #781 reference (#767) | 28,368 | 44 | — |
| 2026-10-07 | `86621a56d8` | Versions for the nee_soon publish chain (maintainer-approved ... | 47 | 45 | 68,253,686 |
| 2026-10-07 | `7d98f34399` | njoy-outram-park-fork: ship build.rs in the package (the publ... | 5 | 0 | 10,062,475 |
| 2026-10-07 | `ec470722b5` | outram-mc-libs lumped tutorial: regenerate its code walk; a c... | 3 | 2 | 5,025,228 |
| 2026-10-07 | `994142b592` | kovan: "Index fresh" for any Rust workspace or crate, from th... | 2,983 | 104 | 0 |
| 2026-10-07 | `065bd710f9` | nee_soon: HTR-10 k vs height, all 22 points re-measured on th... | 3,015 | 8 | — |
| 2026-10-07 | `c6c5b35f21` | kovan: record every rust-analyzer version an index run used, ... | 514 | 54 | 0 |
| 2026-10-07 | `1beff4e4f3` | kovan stamps: sign a full signed_at timestamp and flag implau... | 1,685 | 25 | 0 |
| 2026-10-07 | `8c23486c9a` | kovan stamps: sign signed_at in the reviewer's local time (ch... | 60 | 12 | 4,836,603 |
| 2026-10-07 | `8df5cde5fb` | kovan-common: regenerate docs/kovan-common-api.md (now_local,... | 195 | 5 | 548,356 |
| 2026-10-07 | `c4d33d87ba` | htr10_fill: a pour is settled only after its KE has risen and... | 52 | 7 | — |
| 2026-10-07 | `629300e05f` | Rung 5: DEM bed beside the lattice (htr10 beds view), live DE... | 2,951 | 10 | — |
| 2026-10-07 | `6bf8078b78` | Rung 5 demos for the step 5 and step 6 code walks: dhshort an... | 2,743 | 276 | — |
| 2026-10-07 | `f608124bc3` | dhshort: live low-statistics k per DH treatment, measured fea... | 579 | 90 | — |
| 2026-10-07 | `7ee55f283a` | outram-mc-libs: step-level tracking events for surface and de... | 1,122 | 42 | — |
| 2026-10-07 | `e16be4b274` | stats_move_fingerprints: add host A's post-#589 FHR pin (#784) | 9 | 3 | — |
| 2026-10-08 | `364e55c9ae` | dhoby-ghaut: delta_tracking_web, surface vs delta tracking st... | 4,191 | 0 | — |
| 2026-10-08 | `dbf0b4923f` | triso.md steps 3-4: the step-by-step delta vs surface demo, m... | 417 | 293 | — |
| 2026-10-08 | `ac0adb5084` | triso.md: regenerate code walks after rebasing onto develop (... | 108 | 96 | — |
| 2026-10-07 | `a51cda9bbd` | HTR-10 k-vs-height consumers: repoint to the 2026-10-07 bound... | 125 | 45 | — |
| 2026-10-07 | `7599c36b51` | outram-mc-libs: share nuclear-data processing and generations... | 1,979 | 2 | — |
| 2026-10-07 | `5e1ddd2dd4` | nee_soon: the HTR-10 nuclide set in shareable jobs (#786) | 455 | 1 | — |
| 2026-10-07 | `eda20145cf` | dhoby-ghaut: the whole HTR-10 core on a Web Worker pool (#786) | 3,111 | 7 | — |
| 2026-10-08 | `53ea300410` | Rung 5: whole-core HTR-10 view measured, lesson step 7, code ... | 1,304 | 496 | — |
| 2026-10-08 | `3810f3017f` | pages.yml: fetch reference-data/ace for the HTR-10 core demo ... | 4 | 0 | — |
| 2026-10-08 | `b293463be2` | outram-mc-libs: salvage the triso_delta_tracking framing from... | 23 | 0 | — |
| 2026-10-08 | `85010a947c` | htr10 core web: predictions for the DEM random bed in the liv... | 24 | 0 | — |
| 2026-10-08 | `d74cc652e7` | htr10 core view: the DEM random bed in the live k (#786, #787) | 334 | 26 | — |
| 2026-10-08 | `797db9b3ed` | kovan: CSL citations for the Pages site, and the citeproc-js ... | 15,631 | 2 | — |
| 2026-10-08 | `eed51a2f9c` | nee_soon: htr10_rmc_keff takes a DEM bed; prediction for its ... | 408 | 0 | — |
| 2026-10-08 | `7a4ea492b5` | htr10_dem_bed_keff: smoke run, 15.5 ms CPU per history, 0 los... | 117 | 0 | — |
| 2026-10-08 | `17c530a21d` | htr10_dem_bed_keff: k = 0.989293 +/- 0.000962 on the DEM bed,... | 395 | 2 | — |
| 2026-10-08 | `b10cfe8c02` | beds view, triso.md step 7, dem.md: the random bed's recorded... | 128 | 18 | — |
| 2026-10-08 | `096bcdc9e2` | htr10 core view: fill DEM_NATIVE_RECORD from the #787 record;... | 42 | 20 | — |
| 2026-10-08 | `4acbb32e95` | nee_soon: regenerate the API mirror (kovan-cli api-docs nee_s... | 1,389 | 185 | — |
| 2026-10-08 | `d6b77b3c2e` | Pages: cite the authors of the methods on the deep-dive and t... | 408 | 68 | — |
| 2026-10-08 | `15ef9d87f6` | htr10_dem_bed_keff: record the post-run rebase and how to reb... | 15 | 2 | — |
| 2026-10-08 | `920142d6c4` | kovan-literature: citeproc-js port harness, test-suite refere... | 19,331 | 42 | — |
| 2026-10-08 | `abd9470349` | kovan-literature: keep the citeproc-js verification output an... | 8 | 0 | — |
| 2026-10-08 | `693fc11fee` | kovan-literature: citeproc-js intermediate-state reference du... | 228,179 | 0 | — |
| 2026-10-07 | `7255a7628a` | kovan-literature: citeproc port foundation — porting rules, J... | 3,968 | 0 | — |
| 2026-10-07 | `261e071029` | kovan-literature: citeproc port — pre-declared build/configur... | 516 | 0 | — |
| 2026-10-07 | `e7ed3c6eb9` | kovan-literature: citeproc port — date parser (CSL.DateParser... | 1,110 | 4 | — |
| 2026-10-07 | `145f71cbaf` | kovan-literature: citeproc port — attributes and node builder... | 5,466 | 402 | — |
| 2026-10-07 | `5710e8073a` | kovan-literature: citeproc port — output side: queue, formats... | 6,165 | 29 | — |
| 2026-10-07 | `dbe2e04f5c` | kovan-literature: citeproc port — numbers, dates, particles, ... | 4,852 | 21 | — |
| 2026-10-07 | `42c4c4ae18` | kovan-literature: citeproc port — xmljson, load, state, local... | 8,118 | 63 | — |
| 2026-10-07 | `50b9460ddc` | kovan-literature: citeproc port — nodes differential test, te... | 41,186 | 153 | — |
| 2026-10-07 | `23581bbe14` | kovan-literature: citeproc port — number/date formatter tests... | 698 | 20 | — |
| 2026-10-07 | `e9b3aeeefc` | kovan-literature: citeproc port — output side differential te... | 1,611 | 12 | — |
| 2026-10-07 | `095f817f18` | kovan-literature: citeproc port — name particle and name-inpu... | 371 | 14 | — |
| 2026-10-07 | `4eadd78553` | kovan-literature: citeproc port — static locator and label te... | 403 | 7 | — |
| 2026-10-07 | `88c4f45f51` | kovan-literature: citeproc port — retrieveItem and the input ... | 631 | 51 | — |
| 2026-10-08 | `9ebaba7773` | kovan-literature: citeproc port — input normalisation complet... | 939 | 249 | — |
| 2026-10-08 | `b12a6c20e6` | kovan-literature: citeproc port — intermediate dump test, dif... | 7,053 | 269 | — |
| 2026-10-08 | `72d6a3da13` | kovan-literature: citeproc port — integrate wave 1: state fie... | 369 | 904 | — |
| 2026-10-08 | `8d86286fd5` | kovan-literature: citeproc port — integrate wave 1: real loca... | 417 | 128 | — |
| 2026-10-08 | `6a0e533754` | kovan-literature: citeproc port — integrate wave 1: all 850 c... | 193 | 1,812 | — |
| 2026-10-08 | `648b3cb9aa` | kovan-literature: citeproc port — integrate wave 1: fix repla... | 2 | 4 | — |
| 2026-10-08 | `92525a43e5` | kovan-literature: citeproc port — integrate wave 1: one Match... | 321 | 786 | — |
| 2026-10-08 | `3031aea8a0` | kovan-literature: citeproc port — integrate wave 1: load.js f... | 347 | 674 | — |
| 2026-10-08 | `4a9ead425d` | kovan-literature: citeproc port — follow citeproc-js, but fla... | 93 | 7 | — |
| 2026-10-08 | `85819f3990` | kovan-literature: citeproc port — link candidate deviations C... | 5 | 5 | — |
| 2026-10-08 | `9bdc9849d5` | kovan-literature: citeproc port — integrate wave 1: load.js c... | 202 | 911 | — |
| 2026-10-08 | `edc5d54da8` | kovan-literature: citeproc port — integrate wave 1: page mang... | 60 | 175 | — |
| 2026-10-08 | `4fae080492` | kovan-literature: citeproc port — integrate wave 1: @is-numer... | 285 | 72 | — |
| 2026-10-08 | `4a89f3c722` | kovan-literature: citeproc port — integrate wave 1: docs upda... | 182 | 125 | — |
| 2026-10-08 | `ea56cbc0e0` | kovan-literature: citeproc port — register candidate quirks C... | 14 | 0 | — |
| 2026-10-08 | `c91f9c669a` | kovan-literature: citeproc port — collation via ICU4X icu_col... | 333 | 42 | — |
| 2026-10-08 | `010538f340` | kovan-literature: citeproc port — cs:date and cs:date-part cl... | 863 | 29 | — |
| 2026-10-08 | `390843aba2` | kovan-literature: citeproc port — names subsystem (NameOutput... | 5,804 | 79 | — |
| 2026-10-08 | `4cab259924` | kovan-literature: citeproc port — CSL.Transform, cs:text/grou... | 3,137 | 206 | — |
| 2026-10-08 | `2017c12bc5` | kovan-literature: citeproc port — the engine: registry, disam... | 8,193 | 384 | — |
| 2026-10-08 | `43b3f6e359` | kovan-literature: citeproc port — AmbigConfig element accesso... | 87 | 6 | — |
| 2026-10-08 | `2d806b97fe` | kovan-literature: citeproc port — disambig_settings_ref (#796) | 6 | 0 | — |
| 2026-10-08 | `326d067768` | kovan-literature: citeproc port — attributes bodies (@variabl... | 1,463 | 24 | — |
| 2026-10-08 | `6ed837c7b9` | kovan-literature: citeproc port — dates: differential driver ... | 833 | 17 | — |
| 2026-10-08 | `525a0a91a1` | kovan-literature: citeproc port — names: differential tests (... | 1,399 | 0 | — |
| 2026-10-08 | `3358e27497` | kovan-literature: citeproc port — differential render cases (... | 743 | 31 | — |
| 2026-10-08 | `d781204a00` | kovan-literature: citeproc port — dates: group/value-text sta... | 145 | 21 | — |
| 2026-10-08 | `e506231f84` | kovan-literature: citeproc port — names: generated end-to-end... | 419 | 1 | — |
| 2026-10-08 | `85ac0a391b` | kovan-literature: citeproc port — engine differential test (9... | 592 | 37 | — |
| 2026-10-08 | `53c0257f5c` | kovan-literature: citeproc port — names: suppress-author/coll... | 153 | 26 | — |
| 2026-10-08 | `9c92110dfd` | kovan-literature: citeproc port — registry stand-in, variable... | 310 | 110 | — |
| 2026-10-08 | `80d180f77b` | kovan-literature: citeproc port — harness and mod.rs tests up... | 759 | 282 | — |
| 2026-10-08 | `a9be6318b5` | kovan-literature: citeproc port — names: rustfmt (#794) | 751 | 243 | — |
| 2026-10-08 | `a327772fba` | kovan-literature: citeproc port — names: e2e reference withou... | 17 | 5 | — |
| 2026-10-08 | `105c990740` | kovan-literature: citeproc port — names: sort-key and require... | 128 | 26 | — |
| 2026-10-08 | `a4649599f8` | kovan-literature: citeproc port — integrate wave 2: names lab... | 126 | 17 | — |
| 2026-10-08 | `96646c9ebd` | kovan-literature: citeproc port — link candidate quirks C16-C... | 20 | 20 | — |
| 2026-10-08 | `c6c2d85794` | kovan-literature: csl on the citeproc-js port, hayagriva remo... | 711 | 1,548 | — |
| 2026-10-08 | `2a4085334f` | kovan: references cites HTML pages and pinned style names; re... | 129 | 81 | — |
| 2026-10-08 | `fa8cc2a242` | build-pages: fail on stale citations in the deep dives and tu... | 9 | 0 | — |
| 2026-10-08 | `79ca7386ee` | kovan-literature: citeproc port — getLocaleNames returns what... | 69 | 40 | — |
| 2026-10-08 | `4c49c67aff` | kovan-literature: citeproc port — Greek and Lithuanian locale... | 469 | 64 | — |
| 2026-10-08 | `3b84deff12` | kovan-literature: citeproc port — split authority/committee s... | 352 | 17 | — |
| 2026-10-08 | `b717c676b5` | kovan code map: row 2 stacks crates of one fidelity level one... | 43 | 8 | 69,784,015 |
| 2026-10-08 | `9233199d31` | kovan rung 5: IV&V by a technically and managerially separate... | 3,112 | 91 | 0 |
| 2026-10-08 | `5cf1c5a09c` | docs: rung 5 is IV&V (#809); regenerate the kovan-common API ... | 3,826 | 951 | — |
| 2026-10-08 | `388be33c0b` | kovan-literature: citeproc port — the object-to-array name fi... | 44 | 3 | — |
| 2026-10-08 | `7c7ac568ce` | kovan code map: row 3 stacks too; knowledge management is one... | 45 | 18 | 8,977,427 |
| 2026-10-08 | `b121d6daa6` | references.bib: drop four duplicate entries, keep the cited o... | 0 | 39 | — |
| 2026-10-08 | `d5378d406e` | site: cite the methods and data on the landing page and the d... | 243 | 2 | — |
| 2026-10-08 | `a21a3cf691` | kovan-literature: citeproc port — register D13: fixtures veri... | 201 | 10 | — |
| 2026-10-08 | `46dcb7d3c7` | kovan-literature: citeproc port — register D3, D4, D5: obliqu... | 491 | 37 | — |
| 2026-10-08 | `cbcb0a3f2c` | kovan-literature: citeproc port — register D6-D12: whole tran... | 699 | 122 | — |
| 2026-10-08 | `6d9b4c1661` | kovan-literature: regenerate CSL references for the updated b... | 655 | 1,342 | — |
| 2026-10-08 | `deaaeb52ce` | kovan-literature: citeproc port — pin the names e2e test's ex... | 8 | 0 | — |
| 2026-10-08 | `77e979b348` | site: let the landing page's reference list wrap at phone wid... | 1 | 1 | — |
| 2026-10-08 | `b4a26493ff` | ICSBEP five-route: geometry top down for every case, on Pages... | 207 | 7 | 35,743,770 |
| 2026-10-09 | `4f169582fc` | kovan code map: show the crate backronym, sembawang first (#815) | 471 | 325 | — |
| 2026-10-09 | `a8b9ad7833` | htr10 core demo: fix a race that failed live k_eff runs on ph... | 82 | 20 | 30,512,186 |
| 2026-10-09 | `9f6d53a870` | htr10 core pool: restart a failed run in parallel or serially... | 1,200 | 269 | — |
| 2026-10-09 | `72d261e8aa` | Monte Carlo demos: one processed nuclear-data set, cached in ... | 2,077 | 120 | — |
| 2026-10-10 | `fbdd0e3e83` | kovan index on Windows: read SCIP paths with either separator... | 186 | 5 | 0 |
| 2026-10-10 | `abc77b81de` | nee_soon V&V: point the merged 2026-10-07 fast record at the ... | 8 | 0 | 19,328,438 |
| 2026-10-10 | `85fc80850a` | WIP, never compiled: Code Review tab in desktop kovan (kovan-... | 1,022 | 17 | 0 |
| 2026-10-10 | `4107bbf46a` | Make the Code Review tab build, and hand Stamp / Needs fix to... | 400 | 35 | 0 |
| 2026-10-10 | `13008d7987` | kovan-common review: draft an unsigned stamp, splice review.m... | 1,435 | 3 | 0 |
| 2026-10-10 | `755658d44b` | kovan stamping: draft, write, register keys and per-function ... | 1,354 | 0 | — |
| 2026-10-10 | `f80682f39b` | kovan: the stamp dialog in desktop kovan's Code Review tab (#... | 2,048 | 84 | 0 |
| 2026-10-10 | `1c2319ffda` | kovan call-graph --split-dir: stamp states from review.md, no... | 39 | 44 | 18,109,257 |
| 2026-10-10 | `01a293d629` | kovan review mode logic: walk, diff since review, sections, h... | 2,794 | 13 | — |
| 2026-10-10 | `a923b0b41a` | kovan desktop review mode: source/diff, review.md sidebar, hi... | 1,408 | 15 | 0 |
| 2026-10-10 | `dcbb3e7780` | kovan: need-you queue, recently reviewed, plain-English matur... | 3,932 | 21 | 0 |
| 2026-10-10 | `ff08bbdc1a` | kovan rung 5 (IV&V): views, signing panel, attestation picker... | 3,263 | 47 | 0 |
| 2026-10-10 | `468c55304e` | kovan review mode: reload the view when the stamp dialog clos... | 8 | 2 | — |
| 2026-10-10 | `23baeafd79` | kovan: drop the legacy stamping schemas (#825) | 4,396 | 3,992 | 0 |
| 2026-10-10 | `bb35f67162` | kovan review mode: re-confirm, concept links, formulas, file ... | 2,150 | 126 | 0 |
| 2026-10-10 | `898821cc54` | kovan docs: correct comments that still described review_stam... | 12 | 10 | — |
| 2026-10-10 | `31e98c5093` | kovan-common: regenerate docs/kovan-common-api.md (#770 conce... | 21 | 3 | 0 |
| 2026-10-10 | `ec873fd0dd` | kovan-literature: draft Sciences concept tree, principles and... | 2,685 | 0 | — |
| 2026-10-10 | `2b3cf6b2a9` | kovan-literature Sciences draft: descriptive method ids (kryl... | 14 | 14 | 40,631,339 |
| 2026-10-10 | `938fce39bd` | kovan-literature: DOE Material Science, Chemistry, Mathematic... | 475 | 93 | 2,845,034 |
| 2026-10-10 | `77d3fc7190` | kovan-literature Sciences: approve the nuclear-chemistry bran... | 31 | 20 | 4,183,379 |
| 2026-10-10 | `50bc272e8e` | kovan-literature Sciences: OpenStax CC BY-NC-SA accepted as a... | 7 | 7 | 856,623 |
| 2026-10-10 | `41eaccee93` | kovan-literature Sciences: kinetics branch; approve 39 physic... | 162 | 95 | 2,606,959 |
| 2026-10-10 | `648eedfb84` | kovan-literature Sciences: neutron population kinetics under ... | 30 | 10 | 2,068,896 |
| 2026-10-10 | `b71202222f` | kovan-literature Sciences: first and second law children; ran... | 62 | 3 | 1,954,822 |
| 2026-10-10 | `805b6f8484` | kovan-literature Sciences: Boltzmann diffusion and radiative-... | 53 | 16 | 4,285,769 |
| 2026-10-10 | `698d34edd7` | kovan-literature Sciences: approve Boltzmann limits and quant... | 75 | 21 | 2,879,944 |
| 2026-10-10 | `61cbdff32d` | kovan-literature Sciences: kinetic theory of gases under rand... | 16 | 5 | 1,435,798 |
| 2026-10-10 | `9d84331395` | kovan corpus: OpenStax University Physics Volumes 1-3 in the ... | 89 | 23 | 8,163,055 |
| 2026-10-10 | `0e60fc086c` | kovan-literature Sciences: approve neutronics, materials, the... | 129 | 65 | 4,770,894 |
| 2026-10-10 | `82df397cf3` | kovan corpus: College Physics 2e in 11 parts; Sciences cites ... | 220 | 19 | 8,607,518 |
| 2026-10-11 | `88b2ce861e` | kovan-literature Sciences: kinetics and rate equations; heat ... | 268 | 142 | 6,914,346 |
| 2026-10-11 | `cc0844073f` | kovan-literature Sciences: approve the kinetics branch; nucle... | 48 | 31 | 2,745,320 |
| 2026-10-11 | `e81d4349a5` | kovan-literature Sciences: charged-particle-induced (fusion) ... | 47 | 6 | 2,711,544 |
| 2026-10-11 | `f45b6f8422` | kovan-literature Sciences: approve thermodynamics and the cha... | 100 | 25 | 3,203,211 |
| 2026-10-11 | `b0cdebf1a7` | kovan corpus: OpenStax Chemistry 2e in 9 parts; Sciences disc... | 212 | 21 | 8,734,147 |
| 2026-10-11 | `05fd85a04f` | kovan-literature Sciences: approve the disciplinary views of ... | 17 | 11 | 1,063,921 |
| 2026-10-11 | `47d2d2ea14` | kovan-literature Sciences: approve the computation shape, pro... | 81 | 55 | 2,446,624 |
| 2026-10-11 | `99cc76cce2` | kovan-literature Sciences: approve verified governing theorem... | 44 | 33 | 4,947,149 |
| 2026-10-11 | `fcc686b775` | kovan-literature Sciences: approve the first three IAEA-to-Sc... | 24 | 19 | 7,101,156 |
| 2026-10-11 | `c07d91d27b` | kovan-literature Sciences: approve the IAEA-to-Sciences links... | 9 | 9 | 1,755,042 |
| 2026-10-11 | `0d2944d31b` | kovan-literature: cross-link the thermal-hydraulic L2 family;... | 13 | 9 | 2,099,318 |
| 2026-10-11 | `354af26ed5` | kovan-literature Sciences: approve the thermal-hydraulic desi... | 7 | 7 | 1,591,052 |
| 2026-10-11 | `c7c3bf9d69` | kovan corpus + Sciences: file the nuclear-chemistry and FEM s... | 327 | 32 | 14,045,590 |
| 2026-10-11 | `0ca064abb3` | kovan-web: a desktop drill-in no longer bounces back to the m... | 39 | 6 | 26,752,112 |
| 2026-10-11 | `abb59fd4c5` | kovan index: show what the SCIP index holds for a crate repor... | 32 | 0 | 11,846,268 |
| 2026-10-11 | `7af27f061c` | kovan on Windows: resolve paths with dunce, so rust-analyzer ... | 130 | 57 | 0 |
| 2026-10-11 | `fc672d83fa` | CI: kovan daily, the desktop build and tests on three OSes an... | 125 | 0 | 0 |
| 2026-10-11 | `a3a39f49b2` | kovan Code Review tab: show the call graph's progress and log... | 48 | 10 | 0 |
| 2026-10-11 | `25782bdef0` | kovan: SCIP time estimates for a laptop; kovan daily tests de... | 22 | 8 | 0 |
| 2026-10-11 | `e9bdd05240` | added new kovan toml files and review md | 1,621,036 | 2 | 0 |
| 2026-10-12 | `f622a8e99a` | kovan GUI: read the Save Repository tab's Git state off the G... | 115 | 31 | 7,514,050 |
| 2026-10-12 | `c8240e8102` | added myself as the maintainer | 19 | 0 | 2,851,004 |
| 2026-10-12 | `8f7e6990c7` | kovan GUI: the stamp dialog is a movable window docked right,... | 55 | 41 | 1,350,781 |
| 2026-10-12 | `aaf48f122d` | kovan review: 1 stamp (#771) | 55 | 0 | 966,754 |
| 2026-10-12 | `17117e3848` | kovan GUI: the connection and picker windows can be resized a... | 79 | 52 | 4,805,506 |
| 2026-10-12 | `655e728858` | kovan GUI: "Sync down" in Code Review pulls the code repo, th... | 485 | 46 | 5,455,275 |
| 2026-10-12 | `cc66d6032e` | kovan-web: module cards count reviewed (green) and to-review ... | 72 | 12 | 6,893,478 |
| 2026-10-12 | `21f4b9fdaf` | desktop kovan: use DX12 on Windows unless WGPU_BACKEND is set... | 32 | 1 | 6,936,941 |
| 2026-10-12 | `f5b73f9741` | Pages: gate kovan's picker helpers behind `gui`; fetch full h... | 8 | 0 | 0 |
| 2026-10-12 | `b2ef4d9102` | kovan review: re-anchor a module's stamps after a squash or r... | 1,098 | 9 | 13,092,656 |
| 2026-10-12 | `eb29c0cb62` | desktop kovan: three major tabs (Mindmap, Code Map, Code Revi... | 200 | 28 | 8,958,357 |
| 2026-10-12 | `134b8b417e` | kovan review: 1 stamp (#771) | 57 | 0 | 12,598,049 |
| 2026-10-12 | `326db8e37a` | kovan review wizard: "this is a test" answers, offered and au... | 179 | 2 | 0 |
| 2026-10-12 | `2a5fbcf8b6` | kovan: a code repository's connections file, docs/mindmap/con... | 477 | 0 | 0 |
| 2026-10-12 | `1baa40cc1d` | kovan Code Review: right-click a reviewed function to add a c... | 299 | 4 | 4,333,417 |
| 2026-10-12 | `63e0d7cb88` | desktop kovan: a "Back to map" button under the Code Review m... | 16 | 5 | 686,044 |
| 2026-10-12 | `c6395b9c1d` | kovan: Commit and push accepts the code connections file (#839) | 26 | 3 | 10,417,048 |
| 2026-10-12 | `e795cef6ed` | kovan: code corpus and the Repository management page (#840, ... | 1,122 | 10 | 5,860,290 |
| 2026-10-12 | `cb9256b6f3` | kovan repo_management: the test's doc said the corpus was add... | 5 | 3 | 4,529,179 |
| 2026-10-12 | `3824323433` | raffles: record that calibration is acceptable when it is the... | 61 | 2 | — |
| 2026-10-12 | `bd31b9395b` | CLAUDE.md: point the model-hierarchy rule at the raffles cali... | 2 | 0 | — |
| 2026-10-13 | `2275d46ed4` | nee_soon: HTR-10 MGXS case registry, runner and records (gh:#... | 2,347 | 14 | — |
| 2026-10-13 | `49b7feed2f` | nee_soon: full-core MGXS tally verification passes; unlock th... | 2,491 | 358 | — |
| 2026-10-13 | `885ec93ecb` | outram-mc-libs physics_mg: a non-fissile Mgxs may carry an al... | 66 | 4 | — |
| 2026-10-13 | `8b786d76b9` | kovan: types and constants on the Code Review map and in kova... | 942 | 54 | 19,277,882 |
| 2026-10-13 | `df442ef742` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 12 | 3,891,721 |
| 2026-10-13 | `8997401599` | kovan: stamping and Add connection no longer wait 14 s; a rev... | 109 | 23 | 4,405,152 |
| 2026-10-13 | `47e559b14a` | kovan: functions are linked to the types and constants they u... | 395 | 23 | 10,570,795 |
| 2026-10-13 | `3d76348c31` | nee_soon mgxs: per-group standard errors and MgxsLibrary::to_... | 636 | 0 | — |
| 2026-10-13 | `1a463c5aac` | kovan: the workspace is loaded once and only changed folders ... | 495 | 50 | 11,617,780 |
| 2026-10-13 | `f37cf8c24e` | kovan: review questions for types and constants; rung 4 check... | 1,058 | 52 | 26,200,133 |
| 2026-10-13 | `c0187fd224` | kovan: code reviews of a mounted code repository are listed o... | 479 | 1 | 23,348,313 |
| 2026-10-13 | `9310d33f56` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 12 | 1,968,140 |
| 2026-10-13 | `a545924dc8` | kovan review: 1 stamp (#771) | 60 | 0 | 0 |
| 2026-10-13 | `c4f5d60bf4` | kovan review: 1 stamp (#771) | 45 | 0 | 0 |
| 2026-10-13 | `c61a1a0a7a` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 9 | 8 | 0 |
| 2026-10-13 | `dcf76a7641` | Scattering multiplicity on by default, MG kernel to CE tally ... | 1,089 | 97 | — |
| 2026-10-13 | `a2ada9a3c1` | kovan review: 1 stamp (#771) | 57 | 0 | 0 |
| 2026-10-13 | `aaa0eda7cf` | added many many kovan links, very nice now | 1,224,399 | 339,824 | 492,984 |
| 2026-10-13 | `fc43d8970b` | nee_soon htr10_mgxs::cells: single pebble and Şeker clusters,... | 1,516 | 193 | — |
| 2026-10-13 | `79818b4973` | nee_soon: size the single-pebble DH check from a measured pro... | 10 | 5 | — |
| 2026-10-13 | `2bf7769d62` | nee_soon: record the single-pebble CSG vs DhUniverse result (... | 35 | 9 | — |
| 2026-10-13 | `a12eb37cda` | kovan code map: backronyms for 14 more named crates (#815) | 75 | 10 | 2,817,263 |
| 2026-10-13 | `2420b5a395` | kovan review: 1 stamp (#771) | 52 | 0 | 189,832 |
| 2026-10-13 | `516a27e1c6` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 8 | 9 | 0 |
| 2026-10-13 | `6252a0a8a8` | boon-lay: SiC layer refuses impossible radii; mean_radius ren... | 576 | 296 | 2,881,195 |
| 2026-10-13 | `6d236fd247` | nee_soon, farrer-park: backronym lettering spells the name; b... | 283 | 271 | 0 |
| 2026-10-13 | `00c35bca8d` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 13 | 11 | 262,393 |
| 2026-10-13 | `ace4d205a0` | kovan review: 1 stamp (#771) | 51 | 0 | 0 |
| 2026-10-13 | `30c0ddb708` | kovan review: 1 stamp (#771) | 57 | 0 | 0 |
| 2026-10-13 | `f5aae16042` | kovan review: 1 stamp (#771) | 56 | 0 | 0 |
| 2026-10-13 | `b33dfa179f` | kovan review: 1 stamp (#771) | 59 | 0 | 0 |
| 2026-10-13 | `f006756abc` | new json indexed | 1,058 | 702 | 0 |
| 2026-10-13 | `0b50958199` | kovan review: 1 stamp (#771) | 56 | 0 | 0 |
| 2026-10-13 | `a23162e19e` | kovan review: 1 stamp (#771) | 56 | 0 | 0 |
| 2026-10-13 | `bf4cfdc0aa` | kovan review: 1 stamp (#771) | 55 | 0 | 0 |
| 2026-10-13 | `be363c09ec` | kovan review: 1 stamp (#771) | 55 | 0 | 0 |
| 2026-10-13 | `4db1971284` | boon-lay: SiC thinning and both stress routes refuse impossib... | 351 | 55 | 1,694,022 |
| 2026-10-13 | `684e6acd99` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 13 | 10,321,394 |
| 2026-10-13 | `67b8e9e0ae` | kovan: syntax colours in review; kvim quick fix gets colours,... | 1,134 | 50 | 11,781,834 |
| 2026-10-13 | `912b2915a5` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 6 | 10 | 570,395 |
| 2026-10-13 | `fcbd81ccbe` | kovan review: 2 stamps, 2 entries moved or retired (#771) | 13 | 25 | 0 |
| 2026-10-13 | `30b976362a` | nee_soon: 1/7/19 cell pilots recorded; record README leakage ... | 70 | 6 | — |
| 2026-10-13 | `36c9e46d55` | kovan review: 3 stamps, 3 entries moved or retired (#771) | 20 | 38 | 0 |
| 2026-10-13 | `80c51d993b` | Regenerate the nee_soon and outram-mc-libs API mirrors (gh:#8... | 10,444 | 3,963 | — |
| 2026-10-13 | `15c5eab823` | kovan: completion opens only on Ctrl+N / Ctrl+P; right-click ... | 148 | 66 | 4,141,608 |
| 2026-10-13 | `b2c7f31a9d` | kovan: Quick stamp covers a changed type or constant, proven ... | 50 | 8 | 6,014,128 |
| 2026-10-13 | `55d541bb7d` | kovan: workspace cache drops only the least recently used roo... | 58 | 7 | 3,300,290 |
| 2026-10-13 | `dc9bd09689` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 9 | 6 | 335,391 |
| 2026-10-13 | `6bade696e2` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 6 | 11 | 0 |
| 2026-10-13 | `c8ddf4a75e` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 5 | 5 | 0 |
| 2026-10-13 | `e13e29d7f3` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 5 | 5 | 0 |
| 2026-10-13 | `b88c329a28` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 6 | 6 | 0 |
| 2026-10-13 | `3ff8af1ddb` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 6 | 14 | 0 |
| 2026-10-13 | `27c157e248` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 13 | 0 |
| 2026-10-13 | `100e8df6b1` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 12 | 0 |
| 2026-10-13 | `835c084657` | kovan review: 1 stamp (#771) | 51 | 0 | 0 |
| 2026-10-13 | `cc8ba965b2` | kovan review: 1 stamp (#771) | 55 | 0 | 0 |
| 2026-10-13 | `f3bcd907a7` | kovan review: 1 stamp (#771) | 61 | 0 | 0 |
| 2026-10-13 | `04e00535a9` | kovan review: 1 stamp (#771) | 62 | 0 | 0 |
| 2026-10-13 | `af92cf3e65` | kovan review: 1 stamp (#771) | 62 | 0 | 0 |
| 2026-10-13 | `a84bcd2e36` | kovan review: 1 stamp (#771) | 63 | 0 | 2,704,333 |
| 2026-10-13 | `e37775c96e` | kovan review: 1 stamp (#771) | 61 | 0 | 0 |
| 2026-10-13 | `a4a253183e` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 7 | 13 | 0 |
| 2026-10-13 | `260c08aa09` | kovan review: 1 stamp (#771) | 61 | 0 | 0 |
| 2026-10-13 | `c4c8644c71` | kovan review: 1 stamp (#771) | 60 | 0 | 0 |
| 2026-10-13 | `d63530d7e8` | refreshed new kovan toml and json | 40,368 | 36,982 | 0 |
| 2026-10-13 | `1fabc85a27` | kovan review: 1 stamp (#771) | 51 | 0 | 0 |
| 2026-10-13 | `55ef18de56` | kovan: major tabs return to their last page; an open paper ke... | 180 | 7 | 6,001,251 |
| 2026-10-13 | `f8eb8159fd` | kovan review: 1 stamp (#771) | 51 | 0 | 363,908 |
| 2026-10-13 | `46c92f0a88` | kovan review: 1 stamp (#771) | 58 | 0 | 0 |
| 2026-10-13 | `40c95c7a03` | kovan review: 1 stamp, 1 entry moved or retired (#771) | 6 | 9 | 0 |
| 2026-10-13 | `766a6663a4` | kovan review: 1 stamp (#771) | 58 | 0 | 0 |
| 2026-10-13 | `6e531bb555` | kovan review: 1 stamp (#771) | 51 | 0 | 0 |
| 2026-10-13 | `3a6ce9d9f6` | kovan review: 1 stamp (#771) | 51 | 0 | 0 |
| 2026-10-13 | `7e7ec8d43e` | kovan review: the wizard's comments box starts from the previ... | 60 | 1 | 4,446,718 |
| 2026-10-13 | `30db8da38a` | kovan review: 1 stamp (#771) | 51 | 0 | 375,353 |
| 2026-10-13 | `18e9ad4a50` | kovan: a Back to code review button beside Back to map | 33 | 3 | 755,030 |
| 2026-10-13 | `0c1b9742ca` | kovan review: 1 stamp (#771) | 60 | 0 | 378,022 |
| 2026-10-13 | `69c8a0d9c9` | kovan corpus: file McSpaden et al. 2025, the MUSIC critical b... | 52 | 5 | 12,180,458 |
| 2026-10-13 | `bc8032b3f9` | nee_soon: HTR-10 MGXS case records (core_n12_t300, core_n12_t... | 462 | 0 | — |
| 2026-10-13 | `c2613d5f28` | Acceptance tests for the NJOY port and Outram MC: CI on three... | 2,457 | 189 | 40,516,593 |
| 2026-10-13 | `c5ceac1ffa` | Lessons: regenerate four code walks left stale by the boon-la... | 103 | 69 | 7,490,546 |
| 2026-10-13 | `c6dc726070` | Acceptance tests: mark the scripts executable and call them t... | 1 | 1 | 3,632,247 |
| 2026-10-13 | `7709868faa` | outram-mc-libs: move CsgPowerIteration::source_positions to t... | 13 | 8 | 21,335,146 |
| 2026-10-13 | `0a39f220b7` | Acceptance tests: run LCT-008 at 5000 x [40 + 120] like the o... | 24 | 8 | — |
| 2026-10-13 | `f5c30d2a1b` | Acceptance record for #859; LCT-008 demo default cut to 5000 ... | 249 | 28 | 22,088,620 |
| 2026-10-14 | `5286ff93d5` | Demo: geometry and neutron tracks during Run k_eff, CSG sourc... | 535 | 140 | 30,420,195 |
| 2026-10-13 | `9d58fb9a88` | docs: scope CFD / thermal-hydraulics / GeN-Foam web demo trac... | 240 | 0 | — |
| 2026-10-13 | `8ac363e0a6` | kovan Code Review: the cards follow a light theme (Gruvbox li... | 208 | 40 | — |
| 2026-10-14 | `9cbf9ec2fc` | Acceptance record: run 38476700995 on 5286ff93d5 passes all f... | 5 | 0 | 4,869,614 |
| 2026-10-14 | `3877d8cc50` | Acceptance record: maintainer checked the browser demo on an ... | 15 | 2 | 2,287,341 |
| 2026-10-14 | `4dca014ae4` | kovan review wizard: a number guard can be waived with a reas... | 201 | 4 | 10,612,154 |
| 2026-10-14 | `f9ee6d6fbb` | Monte Carlo demo: date and commit beside every k, and the Ope... | 120 | 22 | 9,653,690 |
| 2026-10-14 | `03d5c9270c` | njoy port docs: upstream has 86 test cases, 46 run acer (not ... | 8 | 6 | 3,273,919 |
| 2026-10-14 | `6e3785e62b` | ace_blocks_vs_reference: the UNR verdict no longer calls the ... | 5 | 2 | 8,745,715 |
| 2026-10-14 | `e653cee413` | LEAPR graphite tests read the committed deck and tape instead... | 62 | 18 | 6,032,887 |
| 2026-10-14 | `10abfe7f1e` | CI: the NJOY port's ACE and LEAPR tests on Linux, Windows and... | 153 | 0 | 10,304,457 |
| 2026-10-14 | `be4267455e` | njoy: acer::compare, the ACE block-by-block comparator, moved... | 485 | 111 | 18,727,267 |
| 2026-10-14 | `f8e1b81ef0` | Nuclear data demo: ACE generator, thermal ACE generator and A... | 851 | 16 | 47,866,525 |
| 2026-10-14 | `4de22a3e63` | Reference data keep their bytes on Windows checkouts (.gitatt... | 5 | 0 | 3,351,068 |
| 2026-10-14 | `85fe98ebf0` | CI: the ACE and LEAPR job also runs when .gitattributes chang... | 2 | 0 | 0 |
| 2026-10-14 | `7071711d9f` | ACE comparator rung: the side panel keeps the verdict, the re... | 7 | 1 | 11,164,917 |
| 2026-10-14 | `5b75b05e21` | ACER lesson links the three live ACE rungs; parity record not... | 163 | 128 | 13,964,081 |
| 2026-10-14 | `21be8a0d97` | Thermal ACE tables are NJOY2016's word for word; demo thermal... | 1,696 | 245 | 174,356,477 |
| 2026-10-14 | `ab526668a3` | Five-route: Rust H(H2O) table built on H-1's PENDF (--thermal... | 941 | 6 | 29,231,164 |
| 2026-10-14 | `26715ee406` | Gate the large thermal tables (IFENG=2, 64-bin H2O) against N... | 99 | 4 | 23,308,785 |
| 2026-10-14 | `c9d16971b3` | HEATR: no std::time::Instant on wasm; the browser ACE generat... | 24 | 2 | 40,160,942 |
| 2026-10-14 | `5eb637f6ce` | Kovan mind map: the corpus root's 19 IAEA issues in the Miles... | 327 | 34 | — |
| 2026-10-14 | `ba7e9ce253` | Incoherent-elastic thermal ACE (H in ZrH, IDPNC=3) is NJOY201... | 459 | 33 | — |
| 2026-10-14 | `480a184f66` | Thermal ACE tables away from the base temperature are NJOY201... | 1,001 | 18 | — |
| 2026-10-14 | `372e9c76fd` | Lessons point at the Code Review map instead of code walks (m... | 551 | 34,899 | 49,993,950 |
| 2026-10-14 | `673b2aca25` | Thermal parity docs: six gate cases (ZrH added); the three la... | 6 | 4 | 28,146,430 |
| 2026-10-14 | `4e7973582b` | Nuclear data lessons: propose the physics functions per rung ... | 301 | 0 | — |
| 2026-10-14 | `71b2ed092a` | Docs: interp.rs table had INT=3/4 the wrong way round; the LE... | 8 | 4 | 16,397,570 |
| 2026-10-14 | `ef42a07dbb` | Code Review map: tests among a function's callers, and a test... | 223 | 29 | — |
| 2026-10-14 | `6e0c9cd668` | LEAPR docs: graphite tape parity is 1.004e-13 max with 6,645/... | 9 | 9 | 22,159,113 |
| 2026-10-14 | `59f8b91c70` | LEAPR test docs: H2O interpretation still quoted the supersed... | 10 | 4 | 7,806,953 |
| 2026-10-14 | `d0d4f7440d` | njoy: four high-level entry points, process::{cross_sections,... | 1,844 | 166 | — |
| 2026-10-14 | `6e35113e2f` | Five-route: routes 2, 4 and 5 take H in H2O from the LEAPR de... | 210 | 9 | 19,361,866 |
| 2026-10-14 | `a58a31e5b8` | acer: MF=6 LAW=2 photons and the empty LDLW/DLW locators (gh:... | 123 | 16 | — |
| 2026-10-14 | `88a01b694c` | acer: convr's corrected photon totals and yield interpolation... | 106 | 10 | — |
| 2026-10-14 | `bf2396e779` | acer: discrete levels given as MF=6 LAW=2 neutrons carry thei... | 64 | 4 | — |
| 2026-10-14 | `4ddd8d65be` | acer acelcp: capture recoil, MF=4 LTT=1/2 and MF=6 LAW=0/2/3/... | 375 | 31 | — |
| 2026-10-14 | `29702e32eb` | acer: anisotropic photons, MF=13 windows, unionx discontinuit... | 890 | 52 | — |
| 2026-10-14 | `5e04032d7d` | #872: V&V record (24/24 identical), photon decode in the gate... | 179 | 3 | — |
| 2026-10-14 | `771d0e5e79` | five_route_keff: routes 4/5 on HST-009 and LCT-008 with H in ... | 172 | 0 | — |
| 2026-10-14 | `c86e0a7ec5` | hst009_keff: correct "none of the three is in the fuel" | 5 | 2 | — |
| 2026-10-14 | `2babfb18ff` | Five-route: LCT-008 runs its full nuclide list by default; LE... | 240 | 66 | — |
| 2026-10-14 | `88f877116f` | LCT-008: retire the 11-nuclide tier to a smoke test | 47 | 35 | — |
| 2026-10-14 | `da354c70ba` | Benchmark demos: H in H2O from LEAPR, LCT-008 on all 35 nuclides | 212 | 61 | — |
| 2026-10-14 | `cc82b32845` | Five-route: full-model LCT-008 routes 4/5, per-seed rows so f... | 3 | 0 | — |
| 2026-10-14 | `b6b9bcffe0` | Five-route: full-model LCT-008 per-seed rows, 3 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `8b50a618f0` | Five-route: full-model LCT-008 per-seed rows, 4 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `8f5fa3fe26` | Five-route: full-model LCT-008 per-seed rows, 5 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `6c40d70088` | Five-route: full-model LCT-008 per-seed rows, 6 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `91e970f739` | Five-route: full-model LCT-008 per-seed rows, 7 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `220f379f9f` | Five-route: full-model LCT-008 per-seed rows, 8 so far (route... | 1 | 0 | — |
| 2026-10-14 | `93598f9360` | Five-route: full-model LCT-008 per-seed rows, 9 so far (run i... | 1 | 0 | — |
| 2026-10-14 | `03dcd61573` | Five-route: full-model LCT-008 per-seed rows, 10 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `1578aaf856` | Five-route: full-model LCT-008 per-seed rows, 11 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `8f53904017` | Five-route: full-model LCT-008 per-seed rows, 12 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `9cde31079d` | Five-route: full-model LCT-008 per-seed rows, 13 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `7f8169c7ea` | Five-route: full-model LCT-008 per-seed rows, 14 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `0931228fbe` | Five-route: full-model LCT-008 per-seed rows, 15 so far (run ... | 1 | 0 | — |
| 2026-10-14 | `cef1853462` | Five-route: LCT-008 full model on routes 4 and 5, 8 seeds eac... | 64 | 1 | — |
| 2026-10-14 | `c4b897873f` | Five-route: the OpenMC converter reads nuclides.tsv (it held ... | 9 | 2 | — |
| 2026-10-14 | `9b448828ad` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 1 so... | 1 | 0 | — |
| 2026-10-14 | `f25225b0cb` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 2 so... | 1 | 0 | — |
| 2026-10-14 | `05699faf46` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 3 so... | 1 | 0 | — |
| 2026-10-14 | `40472bfd05` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 4 so... | 1 | 0 | — |
| 2026-10-14 | `a63e5bdc6f` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 5 so... | 1 | 0 | — |
| 2026-10-14 | `7c6f4b1860` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 6 so... | 1 | 0 | — |
| 2026-10-14 | `bc058caa4c` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 7 so... | 1 | 0 | — |
| 2026-10-14 | `6df5481b80` | Five-route: full-model LCT-008 route 3 complete, 8 seeds (rou... | 1 | 0 | — |
| 2026-10-14 | `99eff48380` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 9 so... | 1 | 0 | — |
| 2026-10-14 | `ab5601e5a4` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 14 s... | 5 | 0 | — |
| 2026-10-14 | `835d5c3491` | Five-route: full-model LCT-008 routes 1-3 per-seed rows, 16 s... | 2 | 0 | — |
| 2026-10-14 | `98cb2c8501` | njoy ACER: continuum reactions given as MF=4 + MF=5 emit thei... | 77 | 9 | — |
| 2026-10-14 | `14fa9d49b5` | Five-route: mark the first full-model route-5 rows superseded... | 9 | 2 | — |
| 2026-10-14 | `7990bf2a2d` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 2 | 0 | — |
| 2026-10-14 | `d6b1d95548` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `0cf28de382` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `751458f33f` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `f90607d41c` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `7b29cd2a04` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `dfe77362b5` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `b31daa1151` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `cbdf4660ea` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 4 | 0 | — |
| 2026-10-14 | `fac5dba94e` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `cbba2afa0f` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `f3d3b75a9e` | Five-route: full-model LCT-008 routes 5 and 2 on the fixed Ru... | 1 | 0 | — |
| 2026-10-14 | `a908ef540a` | Five-route: LCT-008 on its full nuclide list, all five routes... | 173 | 11 | — |
| 2026-10-14 | `2e2e4498f2` | Benchmarks page: lattice row quotes the full 35-nuclide record | 13 | 5 | — |
| 2026-10-14 | `0a4505ed19` | LCT-008 and HST-009 docs: point the deferred items at gh:#875 | 23 | 6 | — |
| 2026-10-15 | `6e3cfd9e91` | five_route_keff record: lesson from the 96-seed lattice exten... | 80 | 0 | — |
| 2026-10-15 | `621d194dd0` | reference-data: ENDF/B-VIII.0 U-236 tape, and its NJOY2016 29... | 17,901 | 1 | — |
| 2026-10-15 | `725af1b245` | Acceptance: 32 seeds on every case; git hooks keep submodules... | 87 | 24 | — |
| 2026-10-15 | `6394b5405a` | Acceptance: revert the 32-seed change; CI keeps 16, 24, 16, 1... | 22 | 29 | — |
| 2026-10-15 | `c7d7470f41` | HST-009: U-236 in the model (worth -167 +/- 38 pcm); LCT-008 ... | 1,089 | 582 | — |
| 2026-10-15 | `be19eeb5a0` | Five-route: LCT-008 defaults to 10000 x [40 + 120]; run sizes... | 57 | 16 | — |
| 2026-10-15 | `50911476b7` | njoy ACER/RECONR/PURR: LCT-008's trace nuclides match NJOY201... | 455 | 83 | — |
| 2026-10-15 | `093d8590cf` | outram-mc-libs: list the port deviations from OpenMC, RNG see... | 61 | 4 | — |
| 2026-10-15 | `a084bdf628` | five_route_keff: re-run all four cases on all five routes (20... | 1,710 | 1,004 | — |
| 2026-10-15 | `4cd2c2df56` | acer: B-10 and every PURR table are NJOY2016's bit for bit, n... | 183 | 12 | — |
| 2026-10-15 | `3b44216d49` | V&V: all 76 five-route tables are NJOY2016's bit for bit (res... | 84 | 6 | — |
| 2026-10-15 | `d731680350` | process_api: integer words of a Type-2 table read back throug... | 19 | 5 | — |
| 2026-10-15 | `0548cc05ec` | acer/heatr/purr: the LCT-008 library with HEATR is NJOY2016's... | 340 | 53 | — |
| 2026-10-15 | `1b8a882917` | V&V: all 76 LCT-008 library tables are NJOY2016's bit for bit... | 6 | 5 | — |
| 2026-10-15 | `a83067e06c` | Acceptance: restore the retired LCT-008 tier's route-1 refere... | 22 | 0 | 39,292,933 |
| 2026-10-15 | `9483185c27` | Monte Carlo demos: up to 100 000 neutrons x 5000 generations,... | 573 | 66 | — |
| 2026-10-15 | `869150e76b` | Five-route V&V: campaign reproduced at a83067e06c; LCT-008 ro... | 710 | 2 | 41,881,403 |
| 2026-10-16 | `fa9ddc00d8` | outram-mc-libs, boon-lay: ship only code to crates.io; record... | 74 | 17 | 21,163,243 |
| 2026-10-16 | `c18c9c7ef1` | outram-mc-libs CLAUDE.md: photon-produced electrons stay with... | 14 | 5 | 1,340,384 |
| 2026-10-16 | `e9b5484608` | AGENTS.md: record the develop/main rulesets and that the bypa... | 22 | 0 | 3,353,461 |
| 2026-10-16 | `06ac969b8f` | foam: measure the Sod RhoCentralFoam solve in a Web Worker (w... | 256 | 17 | — |
| 2026-10-16 | `9f79b22ce1` | docs: BWR boiling literature review (#895); correct stale two... | 659 | 24 | — |
| 2026-10-16 | `10af85038f` | docs: HEM/KNP stabilisation plan (#896); fix misattached and ... | 1,473 | 46 | — |
| 2026-10-16 | `52a79c01da` | hem-knp plan: record maintainer decisions D1-D6 (#896); HEM m... | 23 | 1 | — |
| 2026-10-16 | `5d16d67e25` | petir: RealMath extension (D6) and PyClaw SSP-RK port (D3); S... | 2,559 | 30 | — |
| 2026-10-16 | `fb06878307` | feat(basic-lib): one EOS-free KNP flux kernel; re-point all t... | 1,511 | 875 | — |
| 2026-10-16 | `f56b637038` | Docs redesign iteration 2: sparse landing page, three pathway... | 2,733 | 1,209 | — |
| 2026-10-16 | `9176697318` | tampines-steam-tables: stabilise TampinesSteamArray (gh:#900,... | 1,435 | 121 | — |
| 2026-10-16 | `f54cd592c0` | steam-tables: handle step()/correct_thermo() Results; regener... | 86 | 47 | — |
| 2026-10-16 | `fc6d022578` | Lesson 1: every demo starts when it scrolls into view (gh:#906) | 33 | 11 | — |
| 2026-10-16 | `3037f20366` | feat(steam-tables, tampines): density-based HEM pipe solver H... | 5,313 | 16 | — |
| 2026-10-16 | `4192a9b58f` | edwards_blowdown: strike the stale 'Marviken ignored' claim (... | 4 | 2 | — |
| 2026-10-16 | `a249439ba9` | Monte Carlo demo: Jemima, HST-009 and LCT-008 Run k_eff on a ... | 705 | 51 | — |
| 2026-10-16 | `f1f6dbd2ca` | Neutronics lessons: one question per page, and the thermal-mo... | 889 | 544 | — |
| 2026-10-16 | `a39fcaaada` | Neutronics page 5: a neutron wave scattering from a nucleus, ... | 402 | 4 | — |
| 2026-10-16 | `69a3db362a` | petir migration S2+S3 (gh:#903): IF97 and coolprop HEM hot pa... | 1,007 | 447 | — |
| 2026-10-16 | `f55ae9fddf` | steam-tables hem: route the (rho,e)->p bracket grid through p... | 5 | 2 | — |
| 2026-10-16 | `7de13dbe41` | Neutronics page 6: how do we store a continuous shape? (gh:#906) | 321 | 5 | — |
| 2026-10-16 | `43d8d8118e` | Neutronics page 7: how nuclear data stores scattering angles,... | 380 | 5 | — |
| 2026-10-16 | `950a398364` | Neutronics page 7: the ACE tables are our ACER port's, stated... | 5 | 6 | — |
| 2026-10-16 | `42a05c5987` | HemPipe Edwards: correct the recorded numbers (they came from... | 34 | 9 | — |
| 2026-10-16 | `e96d647027` | nee_soon: HTTR start-up core, explicit TRISO, delta-tracked, ... | 31,502 | 19 | 136,091,733 |
| 2026-10-16 | `97d34ee01e` | boon-lay: verbatim copy of RustBCA v3.0.0 (a356280d) sources,... | 6,582 | 0 | — |
| 2026-10-16 | `0519782da1` | Web demos: thumb-sized Hide, "Sure anot?" before a switch can... | 386 | 184 | 35,543,249 |
| 2026-10-16 | `4e4c1b60ee` | Web demos: view tabs in every phase, user-set statistics with... | 871 | 129 | 53,845,079 |
| 2026-10-16 | `2750844cb1` | monte_carlo_web: Godiva and Jemima loading notes no longer sa... | 5 | 4 | 18,979,400 |
| 2026-10-16 | `8b781c0f47` | boon-lay: RustBCA copy made no_std + alloc, re-exported, plus... | 3,259 | 1,007 | — |
| 2026-10-16 | `b7c9f3b771` | Godiva's Run k_eff on the worker pool, with the lesson counts... | 193 | 15 | 44,038,362 |
| 2026-10-16 | `383b87e24e` | HTR-10 fuel-zone k-inf on the worker pool: a distributed delt... | 408 | 14 | 33,020,222 |
| 2026-10-16 | `b376002be8` | Retire the KNP hybrid (SolverMode::HybridAllMach) from both P... | 492 | 2,305 | — |
| 2026-10-16 | `03a5e3196c` | Regenerate API mirrors after the KNP hybrid retirement | 20,547 | 9,927 | — |
| 2026-10-16 | `8050f1bc39` | monte_carlo_web: record the pooled Jemima Run k_eff timed in ... | 14 | 0 | 22,137,845 |
| 2026-10-16 | `b3fd9a60cd` | Site search: full-text index of every built page (gh:#876 ite... | 496 | 5 | 67,179,438 |
| 2026-10-16 | `ece621c6b6` | Neutronics page 7: sampling is its own widget under "Sampling... | 111 | 41 | 31,122,578 |
| 2026-10-16 | `bfe2b1c45b` | Neutronics pages 8 and 9: why the neutron's energy matters, a... | 651 | 2 | 106,021,570 |
| 2026-10-16 | `0bf9ca4ca0` | Neutronics pages 10 and 11: what can happen in a nucleus, and... | 498 | 2 | 37,696,974 |
| 2026-10-16 | `be8d57f9bc` | Neutronics pages 12-16: the resonance formalisms, from one pe... | 1,054 | 6 | 73,783,605 |
| 2026-10-16 | `0546f80e7b` | Neutronics page 17: why cross sections are drawn on log scale... | 533 | 50 | 30,187,870 |
| 2026-10-16 | `e0b8162a3b` | Neutronics page 18: the double whammy, and smearing the unres... | 372 | 2 | 26,049,079 |
| 2026-10-16 | `53479b7449` | Neutronics page 19: PURR, make up resonances that obey the st... | 610 | 123 | 39,976,683 |
| 2026-10-16 | `c9923b4ce6` | njoy-outram-park-fork: regenerate the API mirror (kovan-cli a... | 10,724 | 7,317 | 0 |
| 2026-10-16 | `8f6e3edeac` | Monte Carlo demo: a collision strip under the σ(E) panel (gh:... | 306 | 50 | — |
| 2026-10-16 | `cded697bd3` | Neutronics page 20: but the atoms are bound!; page 7's forwar... | 284 | 2 | 26,806,072 |
| 2026-10-16 | `e1eccc8e8d` | Neutronics page 17: mention the ugraphite demo's new collisio... | 1 | 1 | 3,388,554 |
| 2026-10-16 | `5b096700d8` | Neutronics page 21: what lattice scattering depends on, with ... | 223 | 4 | 13,474,059 |
| 2026-10-16 | `e59e5365f6` | Neutronics page 22: Doppler broadening, lone atoms warm (gh:#... | 245 | 2 | 19,868,514 |
| 2026-10-16 | `12e1c34b28` | Neutronics page 23: what is a phonon? (gh:#906 designs and am... | 418 | 2 | 22,333,748 |
| 2026-10-16 | `5327ea2599` | Neutronics page 24: can we have more than one phonon? The den... | 189 | 3 | 30,048,868 |
| 2026-10-16 | `52e163b1a1` | Neutronics page 25: how structure shapes the phonons; misconc... | 239 | 2 | 20,430,794 |
| 2026-10-16 | `eb88da007b` | Neutronics maths track, rungs 1 to 5h: from the elastic colli... | 1,497 | 2 | 53,624,250 |
| 2026-10-16 | `4bf2ceef7b` | Neutronics maths track, rung 5i: why each hydrogen scatters d... | 161 | 15 | 32,105,498 |
| 2026-10-16 | `b7705f5914` | Neutronics rungs 3 and 5d: restore the Case A and big-kick de... | 2 | 2 | 5,301,087 |
| 2026-10-16 | `dc9590f510` | Neutronics pages 21 and 23: define Debye-Waller and reciproca... | 2 | 2 | 884,800 |
| 2026-10-16 | `23abe5ad7e` | Neutronics glossary: one plain-words line per term, linked to... | 75 | 0 | 24,089,092 |
| 2026-10-16 | `8dc65df41a` | Neutronics lessons: link each glossary term's first use on la... | 56 | 0 | 12,652,392 |
| 2026-10-16 | `659d1aa1a9` | Pages: content-hashed file cache in a service worker, shared ... | 366 | 1 | 15,038,140 |
| 2026-10-16 | `f5bfa6abff` | Web demos: one nuclide cache for every demo, the nuclear-data... | 640 | 40 | 37,667,264 |
| 2026-10-16 | `963a3fcaee` | outram-mc-libs: regenerate the API mirror for the process_eva... | 401 | 320 | 1,172,717 |
| 2026-10-17 | `c02bbf5108` | Web demos: PURR rung opens at the Monte Carlo demos' 16 ladde... | 48 | 11 | 3,456,784 |
| 2026-10-17 | `5140ea45b2` | CIET Educational Simulator v2 in the browser: JS page, three ... | 4,247 | 1,787 | 68,205,191 |
| 2026-10-17 | `56cf030c01` | CIET web demo record: runs in real time on a Pixel 10a (maint... | 5 | 1 | 9,211,439 |
| 2026-10-17 | `2314563c96` | outram-mc-libs: depend on outram-foam-basic-lib for the begin... | 12 | 1 | 123,757,354 |
| 2026-10-17 | `6b8607eac2` | reactor-literature: Brown's M&C 2009 criticality review paper... | 1 | 1 | 84,057,697 |
| 2026-10-17 | `f3677e3e2d` | raffles: sample skewness, excess kurtosis and D'Agostino-Pear... | 504 | 0 | — |
| 2026-10-17 | `357033acbe` | outram-mc-libs: pin depletion fingerprints before moving the ... | 217 | 0 | — |
| 2026-10-17 | `faa61b5977` | njoy process: 0 K elastic for DBRC, on by default; RECONR spl... | 395 | 25 | — |
| 2026-10-17 | `3570c755ef` | write_ace_library: fast tables via process (one RECONR, then ... | 49 | 21 | — |
| 2026-10-17 | `cf3ddd8849` | petir: port OpenMC's depletion integrators, CRAM and burnup m... | 3,072 | 0 | — |
| 2026-10-17 | `51b683de3f` | outram-mc-libs: beginner model API: materials, shapes, Model,... | 5,147 | 1 | — |
| 2026-10-17 | `c7ea509042` | njoy: NuclearDataLibrary rebuilt on process, thin #[inline] m... | 844 | 236 | — |
| 2026-10-17 | `ac48f6b64f` | outram-foam-basic-lib: steady conduction solver with energy b... | 2,018 | 5 | — |
| 2026-10-17 | `920995f94b` | njoy acquire: strike the stale 'RECONR returns NotPorted for ... | 7 | 2 | — |
| 2026-10-17 | `2b4f78cc94` | outram-mc-libs: tutorial 1, Godiva hello world on the beginne... | 265 | 249 | — |
| 2026-10-17 | `991b8ce527` | outram-mc: measure process's PENDF precision against route 4'... | 315 | 0 | — |
| 2026-10-17 | `c3e2d528bc` | outram-mc/nee_soon: #672 remainders (gh:#934) | 346 | 136 | — |
| 2026-10-17 | `c90ebe4d7c` | docs: regenerate njoy, outram-mc and nee_soon API mirrors (ko... | 4,266 | 302 | — |
| 2026-10-17 | `0872b2eebb` | njoy test: MF=4/5 section census 20 -> 32 (stale on develop) | 6 | 1 | — |
| 2026-10-17 | `31da8a2bb3` | outram-mc-libs: beginner thermal profile on any CSG shape, ba... | 1,467 | 0 | — |
| 2026-10-17 | `86e1d05455` | outram-mc-libs: depletion math now from PETIR; measured order... | 1,870 | 1,493 | 0 |
| 2026-10-17 | `793055937f` | outram-mc-libs: regenerate the API mirror after the depletion... | 240 | 413 | — |
| 2026-10-17 | `5d4811f35e` | sembawang: pin the inventory handoff's outputs before the typ... | 211 | 0 | — |
| 2026-10-17 | `2536f91374` | outram-mc-libs: port OpenMC's stochastic temperature interpol... | 1,039 | 3 | — |
| 2026-10-17 | `360aa0eb94` | njoy: whole ENDF library from a folder, zip or upload; zip ov... | 2,798 | 58 | — |
| 2026-10-17 | `b3b8948bc1` | outram-mc-libs: RadionuclideInventory, the depletion -> sourc... | 20,149 | 5,549 | — |
| 2026-10-17 | `6a0e803f42` | sembawang: take outram-mc's RadionuclideInventory instead of ... | 1,291 | 537 | — |
| 2026-10-17 | `8deca340e4` | outram-mc-libs: several tallies per run, IFP on the CSG power... | 735 | 86 | — |
| 2026-10-17 | `7184c015e8` | outram-mc-libs: beginner Diagnostics, the report zip, advance... | 5,524 | 480 | — |
| 2026-10-17 | `fb8be99bac` | outram-mc: NuclearData from a library folder, zip, upload or ... | 1,558 | 38 | — |
| 2026-10-18 | `27213c16e6` | outram-mc-libs: route 4 is a thin call into njoy's process (g... | 888 | 58 | — |
| 2026-10-18 | `4d11d7854f` | outram-mc-libs: #947 temperature-interpolation error measured... | 1,802 | 43 | — |
| 2026-10-18 | `7b6a33747e` | outram-mc-libs: #948 WMP-at-local-T study: upstream reading, ... | 218 | 0 | 0 |
| 2026-10-18 | `de4a1cf2cd` | outram-mc-libs: tutorial 3, HEU-SOL-THERM-009 through the beg... | 807 | 1 | — |
| 2026-10-18 | `d7298e7eb3` | dhoby-ghaut: the browser nuclide pool makes its data through ... | 521 | 160 | — |
| 2026-10-18 | `7b4607910a` | outram-mc-libs: beginner repeated structures (gh #926): Cube/... | 2,058 | 208 | — |
| 2026-10-18 | `71bd889386` | outram-mc-libs: #948 windowed multipole at the local temperat... | 1,851 | 16 | — |
| 2026-10-18 | `472cae7b5d` | dhoby-ghaut: gh:#932 V&V record; TRISO fixture re-blessed on ... | 147 | 32 | — |
| 2026-10-18 | `2cd257c6a3` | outram-mc-libs: tutorial 4, one HTR-10 TRISO particle in a re... | 710 | 38 | — |
| 2026-10-18 | `b09b411dba` | outram-mc-libs: route 4 re-measured on process's data (gh:#931) | 1,631 | 9 | 0 |
| 2026-10-18 | `b5669c6cd8` | outram-mc-libs: model k runs print OpenMC-style progress by d... | 159 | 5 | 21,948,507 |
| 2026-10-18 | `c4d97dc4e1` | outram-mc-libs: tutorial 4 --geometry-only and --seeds-only (... | 51 | 0 | — |
| 2026-10-18 | `f8a31d12a6` | outram-mc-libs: MCSettings::Smoke, 1 000 neutrons per cycle a... | 28 | 0 | 4,431,768 |
| 2026-10-18 | `1b6979cf1c` | outram-mc-libs: tutorial 4 defaults to MCSettings::Fast (smok... | 41 | 8 | — |
| 2026-10-18 | `5ce3cc993e` | outram-mc-libs: re-record the analog fingerprint on process's... | 29 | 4 | — |
| 2026-10-18 | `1b7bff923c` | boon-lay: fission-product transport theory book, ten steps fr... | 1,997 | 1 | 22,797,096 |
| 2026-10-18 | `03e3ad3a13` | outram-mc-libs: DhRegion, lattice in a sphere and random pack... | 3,337 | 101 | — |
| 2026-10-18 | `b588ffcc22` | lessons: DBRC and the two ACE files, pages 42-45 (gh:#906) | 1,155 | 1 | — |
| 2026-10-18 | `f0c35cfafc` | boon-lay theory book part 2: out of the particle, to dispersi... | 2,176 | 8 | 20,321,977 |
| 2026-10-18 | `4c2977113b` | outram-mc-libs tutorial book: tutorial 3 (HST-009) and tutori... | 582 | 1 | 6,782,520 |
| 2026-10-18 | `a1af2285fc` | appbuilder: add (not subtract) the positive fvm::laplacian in... | 364 | 8 | — |
| 2026-10-18 | `bbe990de74` | lessons: a transport track, eleven pages from the free flight... | 1,734 | 11 | — |
| 2026-10-18 | `4cfb195801` | outram-mc-libs: tallies score the sampled URR band transport ... | 150 | 4 | — |
| 2026-10-18 | `714ebc3aff` | outram-mc-libs: V&V record for URR tally scoring (gh:#953) | 85 | 0 | — |
| 2026-10-18 | `918540c68f` | outram-mc-libs: tutorial 5 results, honest backend provenance... | 199 | 5 | — |
| 2026-10-18 | `c99a96386b` | outram-mc-libs: multi-thread the beginner k run by default (g... | 740 | 151 | — |
| 2026-10-18 | `78b3db6eb9` | outram-mc-libs tutorial pages: the beginner run is multi-thre... | 11 | 4 | 27,700,556 |
| 2026-10-18 | `aee70d46b9` | outram-mc-libs: gate the Auto watcher window on k normality (... | 234 | 26 | — |
| 2026-10-18 | `85d0a4064b` | outram-mc-libs: one fixed temperature per region (gh #945, st... | 1,570 | 41 | — |
| 2026-10-18 | `d2340fd734` | outram-mc-libs: #945 V&V images go on the issue, not develop;... | 7 | 2 | 19,149,370 |
| 2026-10-18 | `50a75d1fe5` | outram-mc-libs: tutorial 6, LCT-008 through the beginner API ... | 2,493 | 60 | — |
| 2026-10-18 | `7abd397f5a` | outram-mc-libs: tutorial 6 assembly picture sized to the asse... | 2 | 2 | — |
| 2026-10-18 | `ce0d616714` | outram-mc-libs: tutorial 7 stage 1, HexPrism and Lattice::hex... | 3,077 | 39 | — |
| 2026-10-18 | `a980a0e34a` | outram-mc-libs: tutorial 7 page, delta zones cut by a boundar... | 352 | 5 | — |
| 2026-10-18 | `c9d05789f7` | outram-mc-libs: tutorial 8, the full Seker bed stacked in a h... | 4,935 | 48 | — |
| 2026-10-18 | `b56acb7f0d` | outram-mc-libs: drop an unused mut in the tutorial 8 test | 1 | 1 | 30,933,408 |
| 2026-10-19 | `5eb2de1a7e` | outram-mc-libs: end-to-end URR tally check, track length vs a... | 606 | 3 | — |
| 2026-10-19 | `db4465fab4` | outram-mc-libs: Auto watcher threshold experiments, instrumen... | 1,492 | 0 | — |
| 2026-10-19 | `817d5b1cb0` | Neutronics page 13 (R-matrix), documentation V3: why interfer... | 66 | 35 | 38,790,830 |
| 2026-10-19 | `fdb1566f9c` | Neutronics page 15 (MLBW): why summing capture and fission pe... | 3 | 1 | 11,382,054 |
| 2026-10-19 | `b24b8391bf` | outram-mc-libs: majorant over a temperature range, Chebyshev ... | 1,343 | 6 | — |
| 2026-10-19 | `79129de8ce` | outram-mc-libs: Haiku dogfood doc fixes (doctests, data prove... | 205 | 37 | — |
| 2026-10-19 | `012dec403b` | outram-mc-libs: CannotExpand message without the repeated clause | 1 | 2 | 14,063,466 |
| 2026-10-19 | `79f12f948f` | htgr_sim_v1: TEDE power selector 600 -> 250 MWth (default); p... | 624 | 60 | — |
| 2026-10-19 | `8a99a6bfd2` | digital-twin-engine CLAUDE.md: htgr_sim_v1's empty-precursor ... | 15 | 0 | — |
| 2026-10-19 | `4127ce5e23` | digital-twin-engine CLAUDE.md: empty precursors are fine, but... | 15 | 12 | — |
| 2026-10-19 | `cb3b27d7fc` | htgr_sim_v1: open the core and graphite at 1500 degC so the s... | 126 | 7 | — |
| 2026-10-19 | `0500d27f93` | htgr_sim_v1: open the core at 1000 degC, not 1500: power by 3... | 61 | 39 | — |
| 2026-10-20 | `815f3a06b3` | Remove live kopi-beans (bn) instructions; GitHub issues are t... | 41 | 782 | — |
| 2026-10-20 | `897c17c02b` | bedok: publishable on crates.io (approved by Than Yan Ren and... | 679 | 1 | — |
| 2026-10-20 | `155f50c5cb` | science page: merge Neutronics and Nuclear data into one section | 119 | 61 | — |
| 2026-10-20 | `ee666fbad1` | science page: TRISO lessons one link each, in collapsible cha... | 97 | 7 | — |
| 2026-10-20 | `1ae20f0c73` | build-pages: publish the TRISO tutorial's animation under its... | 6 | 0 | — |
| 2026-10-20 | `42191b0380` | site-nav: the theory book's Up link reads "Fission Product Tr... | 6 | 2 | — |
| 2026-10-20 | `d48adb5545` | Lockstep release 2026-10-20: versions, packaging, release scr... | 11,338 | 1,072 | — |
| 2026-10-20 | `884ce24c0d` | CLAUDE.md: AGPL-3.0 is an accepted licence, standing direction | 11 | 1 | — |
| 2026-10-20 | `f5e1c86068` | WIP: fission-products lesson 1 (one-fission demo), unfinished | 1,879 | 0 | — |
| 2026-10-20 | `87cecc357d` | boon-lay: correct the TRISO-ATOPS clamping claim in two docs | 23 | 8 | — |
| 2026-10-20 | `4f4fdb5381` | kovan code_map_tags: a backronym may spell the package name's... | 13 | 3 | — |
| 2026-10-20 | `9f364ceb9e` | relicense the workspace to AGPL-3.0-only, except what cannot ... | 16,348 | 2,899 | — |
| 2026-10-20 | `cd6da8881a` | PUNGGOL domain: neutrons in OutramMC, gammas and e-/e+ in pun... | 36 | 8 | — |
| 2026-10-20 | `ed13095dce` | kovan-literature: regenerate the citeproc-js references for t... | 5,446 | 1,970 | — |
| 2026-10-20 | `3301c650bc` | Fission products lessons 1-3: finish lesson 1, add split atom... | 1,754 | 136 | 0 |
| 2026-10-20 | `a6e1f324b4` | kovan-literature: regenerate the citeproc-js reference for th... | 1,542 | 1,056 | 22,786,794 |
| 2026-10-20 | `8fc6a61b32` | Add the missing CLAUDE.md for outram-park-fork-thermochimica ... | 121 | 0 | — |
| 2026-10-20 | `08b8de30cb` | outram-foam-multiphase: revert the 'multiphysics crate' wordi... | 1 | 5 | — |
| 2026-10-20 | `7982994224` | Fission products chapter 2, "A fragment's story" (lessons 4-9) | 2,795 | 1 | 92,725,569 |
| 2026-10-20 | `8bb7091150` | kovan-literature CATALOGUE: correct the Kugeler (EUR 28712) P... | 1 | 1 | — |
| 2026-10-20 | `29da6e8038` | Fission products chapter 3, "TRISO kernel" (lessons 10-28) | 2,564 | 2 | 45,093,459 |
| 2026-10-20 | `0cfb3c0360` | WIP (does not compile yet): route every transcendental throug... | 4,042 | 3,475 | — |
| 2026-10-20 | `414720aea9` | Relicense the last five crates to AGPL-3.0-only (gh:#973) | 2,463 | 430 | — |
| 2026-10-20 | `90d13ad016` | PETIR everywhere, step 0: r_log2/r_exp2/r_sin_cos, petir deps... | 341 | 213 | — |
| 2026-10-20 | `74ed282e19` | PETIR everywhere, step 0b: converted crates' lib and test tar... | 76 | 65 | — |
| 2026-10-20 | `3526f7c8a6` | PETIR everywhere: njoy-outram-park-fork transcendentals throu... | 775 | 622 | — |
| 2026-10-20 | `57080391f9` | PETIR everywhere (OutramFoam), checkpoint: all six crates con... | 773 | 588 | — |
| 2026-10-20 | `89d8ea4e56` | Make Rust 1.99.1 the workspace standard toolchain | 27 | 2 | — |
| 2026-10-20 | `f4965df8ab` | petir: RealMath for f32, generic trait signatures (gh:#985) | 301 | 33 | 0 |
| 2026-10-20 | `413b1cfda2` | raffles: correct stale SCRAM refusal claims (alignments are r... | 13 | 5 | 0 |
| 2026-10-20 | `3bcb7543c8` | raffles: port Computing-with-Confidence (c-box fault-tree pro... | 2,088 | 3 | — |
| 2026-10-20 | `c52e25f798` | Licence doc sweep (root files + docs/): GPL -> AGPL-3.0-only ... | 77 | 77 | — |
| 2026-10-20 | `50bbc84fb0` | Licence doc sweep (a-n crates): GPL -> AGPL-3.0-only statemen... | 2,510 | 1,488 | — |
| 2026-10-20 | `c2fc1d351b` | raffles: AGPL combined work, ship LICENSE.GPL-3.0; licence do... | 7,673 | 4,085 | 14,163,170 |
| 2026-10-20 | `79060c2cff` | README: licence column simply AGPLv3, add a short License sec... | 38 | 31 | 2,944,241 |
| 2026-10-20 | `8f0a5998cf` | Licence doc sweep (o-z crates): GPL -> AGPL-3.0-only statemen... | 22,322 | 9,832 | — |
| 2026-10-20 | `c8b0d9c906` | coolprop: AGPL combined work, ship LICENSE.GPL-3.0 (gh:#973) | 694 | 4 | 5,722,934 |
| 2026-10-20 | `11ddcfd157` | Site footers: GPL-3.0 -> AGPLv3 (gh:#973) | 87 | 87 | 5,385,323 |
| 2026-10-20 | `5afc4e9573` | PETIR everywhere (rest), checkpoint: changi, chem-eng, bedok,... | 736 | 544 | — |
| 2026-10-20 | `2caf4e9dc8` | PETIR everywhere (rest), checkpoint: test-only imports moved ... | 5 | 11 | — |
| 2026-10-20 | `13352773d1` | raffles: stationarity test checks the MSER residual, not a dr... | 37 | 3 | 16,193,146 |
| 2026-10-20 | `b91da0d395` | PETIR everywhere (rest), checkpoint: f32 sites and raffles Cw... | 146 | 95 | — |
| 2026-10-20 | `dd40523e97` | njoy-outram-park-fork: NJOY2016 parity unchanged on PETIR; st... | 21 | 5 | — |
| 2026-10-20 | `0c12f3c023` | petir: port GSL Cholesky (decomp1, solve, invert), verified a... | 10,901 | 0 | — |
| 2026-10-20 | `33bb3bdcc3` | petir: port GSL's gsl_interp_eval_integ as Interp::eval_integ... | 1,069 | 1 | — |
| 2026-10-20 | `35083aa173` | fission-yields-data 0.1.5 -> 0.1.6 (no_std + alloc) | 3 | 3 | — |
| 2026-10-20 | `6d7ccc0ecc` | petir: port GSL symmetric tridiagonal form and eigensolver (s... | 16,911 | 21 | — |
| 2026-10-20 | `674097f29f` | tuas: replace peroxide and roots with PETIR (gh:#989, gh:#991) | 983 | 402 | — |
| 2026-10-20 | `9051377f20` | petir: port GSL's LU with unscaled partial pivoting and its s... | 4,170 | 1 | — |
| 2026-10-20 | `4ae05f2c19` | dwsim: solve_dense goes through petir::linalg::lu_decomp (gh:... | 54 | 66 | — |
| 2026-10-20 | `96d4f4a216` | farrer-park, njoy: record why two inverses stay off petir::li... | 21 | 0 | — |
| 2026-10-20 | `cf41d10121` | uom built without std workspace-wide; dropped quantity maths ... | 989 | 158 | — |
| 2026-10-20 | `d0262c4c8a` | Examples: RealMath imported where test modules and net-fetch ... | 12 | 0 | 0 |
| 2026-10-20 | `2c41a2194a` | petir: port GSL Golub-Reinsch SVD (SV_decomp, SV_solve) (gh:#... | 5,792 | 2 | — |
| 2026-10-20 | `b46b271f14` | petir: port GSL nonsymmetric eigenvalues (balance, Hessenberg... | 7,996 | 2 | — |
| 2026-10-20 | `4dfa27e401` | petir: Octave-style vocabulary on linalg::Matrix (gh:#993) | 475 | 0 | — |
| 2026-10-20 | `a9cd7733d1` | petir: port GNU Octave expm, verified against Octave 11.3.0 (... | 1,447 | 10 | — |
| 2026-10-20 | `ed167ea303` | dwsim-libs: solve_intercell_mass_flow on petir::roots Brent, ... | 128 | 26 | — |
| 2026-10-20 | `290133bd0d` | petir: re-lift 12 verbatim lifts onto RealMath so they match ... | 160 | 112 | — |
| 2026-10-20 | `ae1151b1b6` | teh-o-prke: fhr_sim_v1 parallel-branch solve on petir::roots ... | 109 | 18 | — |
| 2026-10-20 | `db13355410` | changi FLEXPART V&V on the PETIR route: before/after, derived... | 228 | 15 | — |
| 2026-10-20 | `22dc250dea` | Workspace gate: no platform-libm transcendental in any crate'... | 329 | 6 | — |
| 2026-10-20 | `dace71d2dd` | petir::rng: port the rand pieces RustBCA uses, bit-identical ... | 2,137 | 2 | — |
| 2026-10-20 | `2c1bdd8b90` | petir: port faer 0.24.4's sparse LU (COLAMD) and Cholesky (AM... | 3,816 | 3 | — |
| 2026-10-20 | `152237d05a` | outram-blender: sparse Cholesky solves on petir::sparse inste... | 159 | 62 | — |
| 2026-10-20 | `9d9a9d3fa3` | petir: record the sparse port's measured timing against faer ... | 9 | 4 | — |
| 2026-10-20 | `241fcafb4c` | tampines-steam-tables: fhr_sim_v1 parallel-branch solve on pe... | 109 | 18 | — |
| 2026-10-20 | `80399036b8` | petir docs: uom is inherited and no_std; uom_math covers the ... | 37 | 7 | — |
| 2026-10-20 | `0483ccaab4` | raffles: record the post-#985 re-measure of a_smaller_toleran... | 19 | 2 | — |
| 2026-10-20 | `578edc65a2` | tampines: fhr_sim_v2 parallel-branch solve on petir::roots Br... | 109 | 18 | — |
| 2026-10-20 | `b080e87072` | digital-twin-engine: fhr_sim_v2 parallel-branch solve on peti... | 110 | 23 | — |
| 2026-10-20 | `ab0809812a` | outram-mc-libs: re-pin the fingerprint gates once for PETIR r... | 120 | 65 | — |
| 2026-10-20 | `11a678aed0` | changi FLEXPART: cbl, clustering, calcpar evaluation held to ... | 117 | 33 | — |
| 2026-10-20 | `24f4a58f84` | tuas: re-measure after the PETIR switch; record findings (gh:... | 228 | 0 | — |
| 2026-10-20 | `41566068cb` | Cargo.lock: drop roots, now used by no workspace member (gh:#... | 0 | 6 | — |
| 2026-10-20 | `cb27c8c4c4` | petir: port CORE-MATH's correctly rounded hypot; GSL linalg n... | 11,892 | 35 | — |
| 2026-10-20 | `ed6b47cd97` | boon-lay: RustBCA draws from petir::rng; drop rand and rand_d... | 281 | 54 | — |
| 2026-10-20 | `3f073e99ed` | Feature-gated targets: RealMath in scope where .r_* is called... | 6 | 4 | — |
| 2026-10-20 | `75ca33d450` | petir: port LAPACK DGEBAL/DGEBAK; expm now balances as Octave... | 33,537 | 27 | — |
| 2026-10-20 | `3d77f5d082` | petir: record the workspace hypot re-run (gh:#997) | 14 | 0 | — |
| 2026-10-20 | `b4d5797c01` | onix: pin CRAM16 output fingerprints before the move to PETIR... | 255 | 0 | — |
| 2026-10-20 | `0f446895ab` | petir: port ONIX's partial-fraction CRAM16 into depletion::on... | 482 | 5 | — |
| 2026-10-20 | `74fc5fd033` | onix: cram16 calls petir::depletion::cram16_pfd; drop num-com... | 127 | 171 | — |
| 2026-10-20 | `c0b0a1b73a` | tuas: apply the maintainer's #989 decisions; full suite green... | 198 | 73 | — |
| 2026-10-20 | `62f52df4f7` | Workspace: drop the now-unused peroxide and roots dependencie... | 0 | 2 | 14,263,285 |
| 2026-10-20 | `e1c107c27d` | kaki-bukit: decay defaults to PETIR CRAM48; Uniform Taylor ke... | 583 | 67 | — |
| 2026-10-20 | `ae4d28bc63` | pflotran: decay defaults to PETIR CRAM48; petir expm replaces... | 339 | 131 | — |
| 2026-10-20 | `350a6921f3` | changi: check bateman_two_step against PETIR CRAM48 and expm ... | 141 | 0 | — |
| 2026-10-20 | `5e28cd01a4` | petir: stiff-chain test, linalg::expm vs depletion::cram48 on... | 313 | 0 | — |
| 2026-10-20 | `6e97cfd7ce` | boon-lay: CRAM48 expected inventory as the Monte Carlo's dete... | 434 | 1 | — |
| 2026-10-20 | `35fd359c94` | pflotran: point the decay docs at the fidelity follow-up, gh:... | 1 | 1 | 0 |
| 2026-10-20 | `aa655bc3f8` | bedok: sparse LU on petir::sparse instead of faer; faer remov... | 146 | 38 | — |
| 2026-10-20 | `4aa5e34d95` | pflotran decay: port upstream UFD_DECAY 3-generation Bateman ... | 1,956 | 0 | — |
| 2026-10-20 | `9026a9ec45` | pflotran decay: port upstream UFD_DECAY IMPLICIT_SOLUTION as ... | 472 | 0 | — |
| 2026-10-20 | `235e208800` | pflotran decay: DecaySolver::BackwardEuler from RADIOACTIVE_D... | 865 | 270 | — |
| 2026-10-20 | `2770456084` | petir: call Real methods by UFCS so std cannot shadow them (g... | 406 | 400 | — |
| 2026-10-20 | `959e063738` | CLAUDE.md: round-off-level code-to-code differences acceptabl... | 16 | 0 | 20,246,598 |
| 2026-10-20 | `513bb60c72` | petir::sparse::dense: faer's f64 matmul dispatch, triangular ... | 1,902 | 0 | — |
| 2026-10-20 | `a3ba91d0c5` | CLAUDE.md: bit-identical still strongly preferred; any residu... | 9 | 2 | 4,807,806 |
| 2026-10-20 | `0f649364dc` | petir: sqrt by UFCS and one canonical NaN from Real::sqrt/mul... | 247 | 154 | — |
| 2026-10-20 | `6cfbdb56a0` | petir: guard against build-graph-dependent output (gh #1001) | 672 | 0 | — |
| 2026-10-20 | `c39b72485b` | petir::faer_dense: no_std port of faer's dense SVD; outram-bl... | 8,168 | 317 | — |
| 2026-10-20 | `3a54366fad` | bedok: record why solve_dense stays off petir::linalg::lu_dec... | 38 | 2 | — |
| 2026-10-20 | `06f72b26bf` | petir::faer_dense: clippy clean (Mat3 alias, faer's iteration... | 15 | 3 | — |
| 2026-10-20 | `3dfba91ba4` | petir: synchrotron cancellation test measures log error again... | 304 | 43 | — |
| 2026-10-20 | `bc1e0b2435` | petir::sparse: faer's supernodal LU and LLT, with faer's path... | 1,953 | 81 | — |
| 2026-10-20 | `4ae44983ec` | petir: document the supernodal sparse port, its bit-identity ... | 162 | 64 | — |
| 2026-10-20 | `4533294d4d` | petir: synchrotron f32 cost test measured in log error per br... | 700 | 126 | — |
| 2026-10-20 | `e2608c6611` | petir: Fermi-Dirac F_0, F_1/2, F_3/2 f32 cost in log error (g... | 435 | 19 | — |
| 2026-10-20 | `48db8cde79` | petir: Bessel I0/I1/K0/K1 f32 cost in log error (gh #1003) | 314 | 10 | — |
| 2026-10-20 | `8cdfa1cfb1` | outram-blender: Laplacian edge weights in edge-key order, not... | 132 | 22 | — |
| 2026-10-20 | `2a98d727b5` | outram-blender: no hash-map iteration order reaches an operat... | 9,833 | 5,789 | — |
| 2026-10-20 | `4bd16d1dcd` | outram-blender: Mesh::from_polygons looks edges up in a map, ... | 73 | 2 | — |
| 2026-10-20 | `73edacc595` | petir svd3.wgsl + mirror; outram-blender opt-in GPU ARAP rota... | 2,405 | 18 | — |
| 2026-10-20 | `a198db7f1c` | petir: opt-in faer backend (feature faer, no_std + alloc) and... | 358 | 22 | — |
| 2026-10-20 | `596200d519` | bedok: sparse solves on faer again through petir's faer+std f... | 125 | 9 | — |
| 2026-10-20 | `0d0f2b64ac` | petir::sparse::dense: dot's lane sums without subscripts (no_... | 3 | 3 | — |
| 2026-10-20 | `d493a2209e` | petir::sparse: clippy allowances for two verbatim faer forms ... | 7 | 1 | — |
| 2026-10-21 | `c04d16d019` | petir CLAUDE.md: describe the lift/#1001 resolution as merged... | 11 | 8 | — |
| 2026-10-21 | `eb982bb500` | outram-mc-libs: correlated_angle_vs_openmc reads ACE via refe... | 4 | 4 | 126,925,876 |
| 2026-10-21 | `5d659c9ea3` | outram-foam ODE: retry a non-finite TRIAL step as upstream do... | 123 | 22 | — |
| 2026-10-21 | `672dbbab4e` | buangkok pyDOSEIA c2c: name the out-of-tolerance rows; record... | 20 | 0 | — |
| 2026-10-21 | `78237d340f` | raffles abc: test the MEAN posterior sd over seeds 1..=30, no... | 91 | 27 | — |
| 2026-10-21 | `8b9e9bb61e` | Record lockstep release rule: tests green before each publish... | 24 | 0 | — |
| 2026-10-21 | `fe2b373f79` | Release versions for gh:#971 (2026-10-21): +0.0.1 on crates c... | 53 | 47 | — |
| 2026-10-21 | `c51af108f8` | Cargo.lock: workspace member versions for the gh:#971 release | 23 | 23 | — |
| 2026-10-21 | `f2205d867d` | scripts/release-lockstep.sh: test pool ahead of a serial, rat... | 229 | 0 | — |
| 2026-10-21 | `1ad2c214ae` | kovan-literature: regenerate citeproc-js reference for the 25... | 2,223 | 1,093 | 0 |
| 2026-10-21 | `7c54ab82d4` | release-lockstep: match known failures by test binary too; fi... | 26,478 | 4 | — |
| 2026-10-21 | `88b298aace` | release: lockstep run 2 logs (kovan-literature, kovan publish... | 2,924 | 0 | — |
| 2026-10-21 | `a6e34e5cd0` | buangkok pyDOSEIA c2c: the two quadpack sin50 rows are a name... | 112 | 8 | — |
| 2026-10-21 | `123ab98bc4` | outram-foam-basic-lib 0.1.6 -> 0.1.7 (ODE non-finite trial re... | 3 | 3 | 74,872,838 |
