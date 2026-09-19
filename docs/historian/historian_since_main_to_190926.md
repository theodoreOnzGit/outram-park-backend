# OUTRAM PARK — historian report (all of `origin/develop` not in `origin/main`)

> Pre-merge-to-`main` accounting of the API tokens spent and the lines / KLOC written across this window of `develop` history. **Auto-generated** by `kovan-cli historian`; regenerate with `kovan-cli historian --from DDMMYY --to DDMMYY`.

## Scope

- **Branch:** `origin/develop` (vs base `origin/main`)
- **Window:** all of `origin/develop` not in `origin/main`
- **Commits (non-merge):** 1510
- **Token coverage:** 474/1510 commits carry an `API-Usage-Since-Last-Commit` trailer. Commits before the token-accounting hooks existed (or made outside a Claude session) contribute 0 and are counted here as *no token data* — that is correct, not missing data.

## Totals

### Lines written (git numstat, merges excluded)

| Metric | Lines | KLOC |
|---|--:|--:|
| Added (all files) | 30,531,512 | 30531.5 |
| Removed (all files) | 605,284 | 605.3 |
| **Net (all files)** | **29,926,228** | **29926.2** |
| Added (Rust `.rs`) | 4,859,475 | 4859.5 |
| Net (Rust `.rs`) | 4,628,100 | 4628.1 |

### API tokens spent

| Component | Tokens |
|---|--:|
| input | 153,106 |
| output | 28,287,670 |
| cache_read | 12,456,334,718 |
| cache_write | 229,763,573 |
| **total** | **12,714,539,067** |

_`total` = input + output + cache_read + cache_write. Cache-read (prompt-cache re-reads of the growing context) usually dominates; the output figure is the closest proxy for net generated content._

## Lines added, by crate (top 20)

| Crate | Lines added |
|---|--:|
| `outram-foam-appbuilder-lib` | 6,505,785 |
| `njoy-outram-park-fork` | 2,986,236 |
| `openfoam-appbuilder-lib` | 2,063,431 |
| `outram-park-fork-dwsim-libs` | 698,561 |
| `tuas_boussinesq_solver` | 686,076 |
| `tampines-steam-tables` | 678,867 |
| `outram-mc-libs` | 545,676 |
| `outram-park-digital-twin-engine` | 536,052 |
| `outram-foam-basic-lib` | 382,380 |
| `outram-park-fork-offbeat` | 375,952 |
| `outram-park-fork-coolprop` | 362,106 |
| `kovan-literature` | 293,656 |
| `bedok` | 214,261 |
| `outram-blender` | 195,146 |
| `tampines` | 190,773 |
| `outram-park-fork-pflotran` | 190,031 |
| `kovan` | 159,950 |
| `boon-lay` | 155,604 |
| `outram-foam-mesh` | 79,038 |
| `outram-park-fork-liggghts` | 76,860 |

## Per-commit ledger

| Date | Commit | Subject | +lines | -lines | Tokens |
|---|---|---|--:|--:|--:|
| 2026-06-22 | `a2715456d` | removed started TH loop | 0 | 1 | — |
| 2026-06-23 | `f2a8dcbb6` | noted that outside dome test fails again. Working on it now | 69 | 19 | — |
| 2026-06-23 | `99c15e636` | added notes on which datapoints need fixing | 47 | 55 | — |
| 2026-06-23 | `913de832f` | added test results into comments: CLAUDE agent generated | 444 | 5 | — |
| 2026-06-23 | `6453b2990` | added notes for CLAUDE.md that HEM has physical limitations | 33 | 15 | — |
| 2026-06-23 | `d0afc4c62` | added zaloudek in deom and subcooled files. | 1,083 | 0 | — |
| 2026-06-23 | `b6b54c7b3` | added a basic rundown of openfoam-basic-lib | 372 | 0 | — |
| 2026-06-23 | `18566c1e8` | added more things for sonnet to port over | 259 | 8 | — |
| 2026-06-23 | `04402b539` | added tensors and vector3 | 1,532 | 0 | — |
| 2026-06-23 | `975d1729e` | added math and polynomial libraries, the solvers now use C ab... | 1,612 | 0 | — |
| 2026-06-23 | `177c01fd6` | updated extern C policy | 28 | 0 | — |
| 2026-06-23 | `570a82b21` | added interpolation matrices and ode solvers | 1,247 | 3 | — |
| 2026-06-23 | `426505998` | openfoam basic lib at v0.1.1 | 2 | 2 | — |
| 2026-06-23 | `59a694a76` | workspace is also now v0.1.1 | 1 | 1 | — |
| 2026-06-23 | `c266e6976` | updated claude md for specie-level thermophysics | 260 | 2 | — |
| 2026-06-23 | `2508d8d50` | scaffolded CLAUDE.md for thermophysical model conversion | 121 | 22 | — |
| 2026-06-24 | `0f0d5d236` | openfoam-basic-lib v0.1.2: Layer 1h specie-level thermophysics | 1,165 | 3 | — |
| 2026-06-24 | `272700ac1` | added HRM critical flow solver comment | 299 | 0 | — |
| 2026-06-24 | `5f6830b40` | added polynomial models for openfoam | 492 | 3 | — |
| 2026-06-24 | `039400884` | added pengrobinson and tablulated h into the thermo libaries ... | 779 | 4 | — |
| 2026-06-24 | `3009aa4c0` | changed glob imports as a code refactor | 71 | 131 | — |
| 2026-06-24 | `b6fd5427e` | added updated solver import targets | 5 | 4 | — |
| 2026-06-24 | `e578c2aea` | added openfoam fields, boundary and ldu matrix solvers | 1,749 | 2 | — |
| 2026-06-24 | `f0bdabef9` | added note on ndarray-linalg | 13 | 0 | — |
| 2026-06-24 | `32d4dad6f` | fixed doctests and imports | 22 | 3 | — |
| 2026-06-24 | `18e776ba0` | updated doctest claude md policy | 11 | 10 | — |
| 2026-06-24 | `2a5830536` | adding fvc and fvm, pending token reset | 719 | 0 | — |
| 2026-06-24 | `02b94de6c` | finished adding and testing fv operators | 280 | 4 | — |
| 2026-06-24 | `3e441a966` | openfoam-basic-lib v0.1.3: add Layer 4 fluid thermodynamics | 574 | 4 | — |
| 2026-06-24 | `638ae657b` | openfoam-basic-lib v0.1.4: icoFoam prerequisites + multi-regi... | 1,767 | 9 | — |
| 2026-06-24 | `d7e448e0c` | no more auto commit for CLAUDE.md | 6 | 0 | — |
| 2026-06-24 | `551cd5a3c` | added ddt and ddt_vec | 105 | 10 | — |
| 2026-06-24 | `d8e1860bb` | addeda  matrix benchmark test for openfoam vs ndarray linalg | 270 | 75 | — |
| 2026-06-24 | `db2d7becd` | removed ndarray-linalg from tuas dependencies, using openfoam... | 55 | 136 | — |
| 2026-06-24 | `c8678d3f3` | tuas boussinesq solver now bumped to v0.1.2, remoed the ndarr... | 30 | 28 | — |
| 2026-06-24 | `85ac0d8b2` | moved tampines-steam-tables ndarray-linalg to dev depdencies | 59 | 3 | — |
| 2026-06-24 | `3d03b577c` | added test backlog for openfoam-basic-libs | 95 | 0 | — |
| 2026-06-24 | `0ac6fcb59` | added new tests, need to pass them | 360 | 24 | — |
| 2026-06-24 | `19493f744` | openfoam-basic-lib v0.1.5: P2 tests, ignore known failures | 134 | 3 | — |
| 2026-06-24 | `715c521d1` | added dwsim-libs and openmc-libs barebones, just a start of t... | 1,810 | 0 | — |
| 2026-06-25 | `f1e63ed77` | in process of adding boon lay | 414 | 6 | — |
| 2026-06-25 | `fb616d0f9` | added boon lay scaffold and openmc rng distributions | 108 | 34 | — |
| 2026-06-25 | `9b79c7b93` | partially migrated boon lay decay simulator and triso simulat... | 13,167 | 24 | — |
| 2026-06-25 | `d53b1344c` | added two phase thermosyspro modelica choked flow implementat... | 220 | 0 | — |
| 2026-06-25 | `d548fd2ec` | scaffolded openfoam-appbuilder-lib and openfoam-turbulence-lib | 543 | 1 | — |
| 2026-06-25 | `146714973` | added first import of openfoam-appbuilder-lib and openfoam-tu... | 1,155 | 35 | — |
| 2026-06-25 | `1b1b5e7cf` | added nnc261 matrix from matrix market | 1,657 | 22 | — |
| 2026-06-25 | `e772b001e` | added hrmfoam rho central foam, rhopimplefoam and sonic foam ... | 898 | 110 | — |
| 2026-06-25 | `c9bcf20cf` | added nnc261 solve for verification | 135 | 0 | — |
| 2026-06-25 | `533aa2e6d` | nnc261 now comes with an explanation | 20 | 5 | — |
| 2026-06-25 | `9ca8a7769` | added license to openfoam-appbuilder-lib and openfoam-turbule... | 4,457 | 31 | — |
| 2026-06-25 | `c92f64dea` | bumped teh o prke to v0.1.1 | 3 | 3 | — |
| 2026-06-25 | `d80576a22` | started scaffolding the openFoam comparison files | 2,030,410 | 0 | — |
| 2026-06-25 | `a4ebf4e57` | noted that pimpleFoam cavity flow is from the icoFoam verific... | 44 | 14 | — |
| 2026-06-25 | `3bcaf2a73` | polyMesh parser now works! | 762 | 24 | — |
| 2026-06-25 | `37c472710` | pimplefoam cavity blockMesh runs okay | 0 | 1 | — |
| 2026-06-25 | `e3b4a6102` | complete boon-lay triso simulator example port; mandate relea... | 1,743 | 54 | — |
| 2026-06-25 | `0d65e0691` | wire up pimpleFoam/rhoCentralFoam/rhoPimpleFoam tutorial test... | 685 | 107 | — |
| 2026-06-26 | `7bec91e26` | document pimpleFoam port: OpenFOAM source, change justificati... | 157 | 1 | — |
| 2026-06-26 | `86368b7af` | sketch MC transmutation design in boon-lay CLAUDE.md | 40 | 4 | — |
| 2026-06-26 | `f3aed3044` | add fvcDdtPhiCoeff limiter + fix PISO corrector loop; cavity ... | 195 | 136 | — |
| 2026-06-26 | `13ab29e65` | clarify Zaloudek data is HEM-computed, not experimental measu... | 20 | 9 | — |
| 2026-06-26 | `831a68824` | add MUSCL reconstruction (2nd-order rhoCentralFoam) and imple... | 715 | 56 | — |
| 2026-06-26 | `b676ea937` | add nuclear-data distribution design notes for the OpenMC port | 202 | 0 | — |
| 2026-06-26 | `3d1177091` | added some stuff on openfoam-appbuilder-lib | 1,227 | 682 | — |
| 2026-06-26 | `50c956797` | rhopimplefoam undergoing fix | 232 | 96 | — |
| 2026-06-26 | `4d33b2ee8` | added rhopimpelfoam fix | 81 | 34 | — |
| 2026-06-26 | `4f9092e3d` | added ghia benchmark | 134 | 11 | — |
| 2026-06-26 | `8a4395d5a` | benchmarks now have csv data to plot and show that they work! | 71 | 6 | — |
| 2026-06-26 | `3dbbdd9b1` | Add HEM critical-flow solver for superheated vapour / supercr... | 404 | 18 | — |
| 2026-06-26 | `89b848fa5` | Claude added possible causes. numerics being no.4 | 76 | 0 | — |
| 2026-06-26 | `b164d81b0` | added opus conversation | 22 | 0 | — |
| 2026-06-26 | `2bf191343` | added notes for the ai agent to consider formulating a soluti... | 64 | 21 | — |
| 2026-06-27 | `3dc363445` | Add mandatory human interface layer design principle to CLAUD... | 36 | 0 | — |
| 2026-06-27 | `fffa4b2eb` | Add project motivation (OpenFOAM negative example) to all ope... | 80 | 0 | — |
| 2026-06-27 | `5e2f03389` | Add mandatory Rust design rules: enums over trait objects, no... | 188 | 0 | — |
| 2026-06-28 | `559cdbeaa` | claude md trimmed down | 2,655 | 2,458 | — |
| 2026-06-28 | `243c27774` | Scaffold njoy-outram-park-fork: Rust port of NJOY2016 | 598 | 0 | — |
| 2026-06-28 | `d44e47758` | njoy-outram-park-fork: Phase 1 — ENDF tape reader + physical ... | 1,007 | 34 | — |
| 2026-06-28 | `ac619bb9a` | njoy-outram-park-fork: add U-235 integration tests (485K line... | 485,519 | 0 | — |
| 2026-06-28 | `4640bce19` | njoy-outram-park-fork: Phase 2a — RECONR linearisation + MF=1... | 2,617 | 0 | — |
| 2026-06-28 | `349466686` | Phase 2b SLBW/MLBW resonance reconstruction + uom OOP API | 48,564 | 248 | — |
| 2026-06-28 | `54bf53405` | Add MtReaction enum; replace raw i32 MT numbers in public API | 560 | 44 | — |
| 2026-06-28 | `827f2feeb` | Prepare njoy-outram-park-fork for crates.io publish | 67 | 55 | — |
| 2026-06-28 | `a747d07a2` | Exclude integration tests and ENDF fixtures from published crate | 2 | 1 | — |
| 2026-06-29 | `b73a0f575` | njoy debugging via opus complete for reconr and broadr, pendi... | 1,385 | 136 | — |
| 2026-06-29 | `46f19c8f9` | merged from branch PR | 391 | 248 | — |
| 2026-06-29 | `50632f05d` | HEM dispatcher for tampines steam tables now working - debugg... | 355 | 87 | — |
| 2026-06-29 | `edc234bbd` | doc comments debugged | 2 | 2 | — |
| 2026-06-29 | `0ffbe91ee` | added mass and energy balance interface | 470 | 2 | — |
| 2026-06-29 | `3a6fadcac` | in the midst of debugging moddy isobars... | 44 | 15 | — |
| 2026-06-29 | `ccd3e9ce9` | added debug notes, and removed ndarray-linalg from tampines-s... | 45 | 10 | — |
| 2026-06-29 | `f4f999081` | added todo for ghia benchmark | 23 | 0 | — |
| 2026-06-29 | `aab9e7a87` | added polymesh for cavityFine map | 20,586 | 0 | — |
| 2026-06-29 | `54692c1f1` | pimpleFoam cavity test now has mesh refined case | 144 | 26 | — |
| 2026-06-29 | `7e759c691` | added performance benchmarks | 44 | 0 | — |
| 2026-06-29 | `1585dacd6` | moody tests passing, but the zaloudek tests failing. Also ope... | 435 | 65 | — |
| 2026-06-29 | `85ce745b0` | added gamg solver, works well but doesn't speed things up | 910 | 468 | — |
| 2026-06-29 | `7173480c5` | moody tests pass! | 18 | 9 | — |
| 2026-06-29 | `c7723157a` | added temporary fix for moody's datapoints | 48 | 14 | — |
| 2026-06-29 | `3be7a23e5` | added subcooling degree as discriminator | 44 | 4 | — |
| 2026-06-29 | `20e20d3ff` | debugging moody test, tolerance TBD | 79 | 23 | — |
| 2026-06-29 | `561a6ffec` | renamed mod to interface | 3 | 3 | — |
| 2026-06-29 | `7b6def03e` | added notes about interface | 4 | 0 | — |
| 2026-06-29 | `aff73b84a` | added some comments to openfoam input and output and stuff | 3 | 0 | — |
| 2026-06-30 | `e2e4dac8c` | Complete Moody deeply-subcooled assertions; document isobar_0... | 225 | 29 | — |
| 2026-06-30 | `1ae3670a4` | v0.2.1 ready to push to cargo, moody tests are passing | 74 | 63 | — |
| 2026-06-30 | `56e1ce209` | Moody: run isobar_pref_0_25 in-dome-only; comment out deep di... | 119 | 43 | — |
| 2026-06-30 | `59539f7e0` | added ace file writer and zaloudek/moody data | 1,639 | 12 | — |
| 2026-06-30 | `b495083e6` | added zaloudek data for all curves. | 388 | 0 | — |
| 2026-07-01 | `a562538b9` | njoy ACER Phase 4c: elastic angular distribution (MF=4 → LAND... | 587 | 25 | — |
| 2026-07-01 | `b56c1fd50` | added a one d mesh constructor | 258 | 23 | — |
| 2026-07-01 | `d47e249a7` | added one dimensional meshing to openfoam-basic-libs, plus ch... | 96 | 35 | — |
| 2026-07-01 | `c25067930` | fixed polymesh builder error | 2 | 2 | — |
| 2026-07-01 | `3def31133` | njoy ACER 4d start (MF=5→Law 4) + Phase-4 scaffolds (S(α,β), ... | 480 | 5 | — |
| 2026-07-01 | `01bda7c11` | Scaffold TampinesSteamArray: 1-D rhoPimpleFoam on FvMesh | 417 | 5 | — |
| 2026-07-01 | `1c332c1de` | explained my dilemma with openfoam and tampines steam array. ... | 112 | 0 | — |
| 2026-07-01 | `0de3b57d2` | docs(tampines): record appbuilder as steam-table consumption ... | 39 | 4 | — |
| 2026-07-01 | `ea4fdfcad` | outram park ace 4d complete | 126 | 0 | — |
| 2026-07-01 | `026320d13` | added thermal writer mf7 | 27,213 | 6 | — |
| 2026-07-01 | `3af55170b` | njoy ACER 4d: wire the DLW block — loadable table with energy... | 532 | 102 | — |
| 2026-07-01 | `e9b0fabec` | njoy ACER 4d: discrete-level non-elastic angular (MF=4 → AND) | 136 | 49 | — |
| 2026-07-01 | `ed7367547` | njoy THERMR: coherent-elastic (Bragg) thermal scattering | 131 | 6 | — |
| 2026-07-01 | `e213cbe2f` | docs: record the "own Rust parser, no C++ interop" rule for O... | 22 | 0 | — |
| 2026-07-01 | `3c8e2c50d` | njoy THERMR: incoherent-inelastic scattering from S(alpha,beta) | 281 | 5 | — |
| 2026-07-01 | `85ab3210b` | in the middle of adding s(alpha,beta) ace writer. I ran out o... | 118 | 14 | — |
| 2026-07-01 | `a90401032` | in the midst of adding ace test for thermal s(alpha,beta) | 353 | 71 | — |
| 2026-07-01 | `b16eea275` | added claudemd for openfoam algorithms | 116 | 0 | — |
| 2026-07-01 | `957ff4876` | unit tests passing for openfoam import, but doctests failing | 15,212 | 6 | — |
| 2026-07-01 | `99d7d67b3` | fixed doctests... but this assumes my openfoam algos are pub ... | 6 | 7 | — |
| 2026-07-01 | `31ea509f0` | tampines steam table tests pass! | 3 | 3 | — |
| 2026-07-01 | `1b3f2ee0a` | gamg doctest reset | 1 | 1 | — |
| 2026-07-01 | `6639d11a5` | added pub crate port debt | 26 | 0 | — |
| 2026-07-02 | `e87c7ff50` | chore(tampines): make openfoam_source pub(crate) only | 38 | 38 | — |
| 2026-07-02 | `b7be46911` | ensure memory leaks do not happen in tests | 124 | 15 | — |
| 2026-07-02 | `c9b5df2ae` | incoherent elastic complete! | 63 | 0 | — |
| 2026-07-02 | `47fa4ce68` | incoherent elastic complete | 250 | 8 | — |
| 2026-07-02 | `46e2ec763` | njoy: H(ZrH) incoherent-elastic thermal ACE integration test | 16,444 | 0 | — |
| 2026-07-02 | `cd839a436` | Neutronics scaffold: njoy owns all nuclear data; Keff + Doppl... | 682 | 75 | — |
| 2026-07-02 | `f8594518b` | in midst of validating with u238 endf data | 430 | 42 | — |
| 2026-07-02 | `e8713d68f` | njoy hdf5 u238 wmp done | 339 | 31 | — |
| 2026-07-02 | `a2f4dfb33` | wmp finished! ish | 5 | 3 | — |
| 2026-07-02 | `83a6c192e` | added wmp nuclide manifest | 444 | 0 | — |
| 2026-07-02 | `c70867143` | wmp manifest: expand CORE to package D (LFTR salts) | 16 | 10 | — |
| 2026-07-02 | `bbbd5d4cb` | keff roadmap: fast range via multigroup, not pointwise lean-ACE | 37 | 5 | — |
| 2026-07-02 | `934ca2676` | docs: data acquisition tiers + parallel-run download safety | 126 | 0 | — |
| 2026-07-02 | `f5ead1533` | docs(data-acquisition): collapse to two tiers — LOW (WMP+10-g... | 24 | 11 | — |
| 2026-07-02 | `0470a0cfd` | njoy WMP: implement WMPB v1 blob codec (to_blob / from_blob) | 343 | 12 | — |
| 2026-07-02 | `9dabaa6c1` | njoy WMP: add WmpLibrary multi-nuclide WMPL container | 206 | 0 | — |
| 2026-07-02 | `84dd677b4` | njoy WMP: bake CORE blob (always embedded) + Watt-weighted fa... | 544 | 64 | — |
| 2026-07-02 | `299ef48bc` | njoy: bake fast-range MGXS from ENDF/B-VIII.0 (RECONR MF=3 → ... | 638 | 8 | — |
| 2026-07-03 | `32f5d034e` | openmc: first end-to-end Godiva Keff (WMP+MGXS → power iterat... | 862 | 95 | — |
| 2026-07-03 | `3ac34edcb` | njoy: wire HIGH-fidelity ENDF download + selectable MGXS weig... | 1,151 | 27 | — |
| 2026-07-03 | `cb44e15d5` | openmc: HIGH-fidelity Godiva on device-reconstructed ENDF + L... | 488 | 48 | — |
| 2026-07-03 | `c7615c315` | openmc: model inelastic scattering — Godiva HIGH −2510 pcm | 428 | 62 | — |
| 2026-07-03 | `dd2a78b10` | openmc: anisotropic MF=4 elastic scatter — Godiva HIGH reache... | 303 | 75 | — |
| 2026-07-03 | `092810383` | LOW tier: inelastic + forward elastic scatter; add pebble_bed... | 1,090 | 81 | — |
| 2026-07-03 | `94b626229` | openmc-libs: (n,2n) yield-2 multiplicity + canonical-source p... | 238 | 90 | — |
| 2026-07-03 | `c8815cfba` | godiva keff tests ongoing | 462 | 38 | — |
| 2026-07-03 | `d84fa5c08` | openmc/njoy: energy-dependent MF=5 fission spectrum χ (HIGH t... | 98 | 9 | — |
| 2026-07-03 | `dd20bcc9d` | njoy/openmc: finish MF=5 porting — LF=7/9/11 + NK>1 mixtures;... | 651 | 93 | — |
| 2026-07-03 | `0114a6bde` | gaspr finished | 330 | 1 | — |
| 2026-07-03 | `3d7e3b0ed` | docs: GASPR V&V entry + porting-plan Phase 3 note | 66 | 1 | — |
| 2026-07-03 | `7512665ae` | docs: THERMR + thermal ACE writer were already done — fix sta... | 47 | 38 | — |
| 2026-07-03 | `50ed17ccc` | docs(thermal): mark IFENG=1/2 and nmix>1 gaps inline, not jus... | 18 | 1 | — |
| 2026-07-03 | `dbd50ec0f` | docs: HEATR phased porting scaffold (H1-H7) | 44 | 1 | — |
| 2026-07-03 | `679e3ac60` | njoy: HEATR H1 — elastic kinematic heating (MT=2) | 310 | 0 | — |
| 2026-07-04 | `19c43e7d0` | njoy: HEATR H2 — local-deposition reactions (MT=102, 103-117) | 87 | 15 | — |
| 2026-07-04 | `42aa8ccc0` | njoy: HEATR H3 -- single-escaping-neutron reactions (discrete... | 124 | 10 | — |
| 2026-07-04 | `36a78e83f` | njoy: HEATR H4 -- fission heating (MT=18, 19-21, 38) | 371 | 49 | — |
| 2026-07-04 | `eeedd7cab` | njoy: HEATR H5 — multi-neutron-exit + continuum inelastic (MT... | 317 | 28 | — |
| 2026-07-04 | `5814a2b71` | njoy: HEATR H7 (elastic) — damage-energy production (MT=444),... | 285 | 6 | — |
| 2026-07-04 | `0a7731182` | njoy: HEATR H5 — MF=6 emission spectra via EmissionSpectrum enum | 171 | 25 | — |
| 2026-07-04 | `364e86112` | njoy: ACE 4e — wire HEATR H1–H5 KERMA into the ESZ heating co... | 194 | 25 | — |
| 2026-07-04 | `1ab83314f` | added some todos for tampines and tuas for porting to android | 113 | 0 | — |
| 2026-07-04 | `dc657f7d2` | njoy: HEATR H7 — discrete-level damage energy (MT=51–90) | 182 | 60 | — |
| 2026-07-04 | `5a72772b0` | njoy: HEATR H6 (part 1) — photon-production parser (MF=12 LO=... | 434 | 0 | — |
| 2026-07-04 | `bfd6f4e09` | njoy: HEATR H6 (part 2) — energy-balance KERMA + wire into ACE | 61 | 18 | — |
| 2026-07-06 | `1cd53a571` | tampines: fix Edwards-pipe test path in openfoam_algorithms C... | 78 | 59 | — |
| 2026-07-06 | `731469324` | scaffolded nee soon and started readmes ports for njoy | 2,364 | 118 | — |
| 2026-07-06 | `d34478c06` | added sod shock tube claude md | 255 | 0 | — |
| 2026-07-06 | `18d7a6fb6` | cleaned up tampines-steam-tables dependencies to not use tuas... | 11 | 850 | — |
| 2026-07-06 | `33f44af6f` | sod shock tube now validated! | 778 | 5 | — |
| 2026-07-06 | `b5b884522` | added github readme to sod shock tube | 334 | 0 | — |
| 2026-07-06 | `e6059dcd8` | changed github markdown, hopefully it renders equations cleanly | 57 | 39 | — |
| 2026-07-06 | `5435c5b2a` | changed claude md to include readme markdown checks | 36 | 0 | — |
| 2026-07-06 | `86c8513db` | njoy u238 test port in progress, updated openfoam sod shock t... | 196,888 | 278 | — |
| 2026-07-06 | `724653964` | doppler test initial commit ok | 200,512 | 23 | — |
| 2026-07-06 | `4768abef7` | njoy: UNRESR physics kernel port (ENDF LRU=2, Faddeeva W-func... | 1,566 | 43 | — |
| 2026-07-06 | `bafda7687` | njoy: PURR scaffolding port (ENDF reuse, uw2, RNG, ladder gen... | 785 | 61 | — |
| 2026-07-06 | `367644e97` | njoy: PURR unrest port -- Monte Carlo probability-table binni... | 881 | 74 | — |
| 2026-07-07 | `51335d73e` | njoy: SAMM Phase 1 -- ENDF LRF=7/KRM=3 (R-matrix-limited) par... | 418 | 23 | — |
| 2026-07-07 | `959a4a4a8` | njoy: SAMM Phase 2 -- spin/parity/penetrability setup (non-Co... | 758 | 38 | — |
| 2026-07-07 | `ced6c2213` | njoy: SAMM Phase 3 -- Coulomb wave functions, plus betset's n... | 1,433 | 46 | — |
| 2026-07-07 | `d6a6fe845` | njoy: SAMM Phase 4 -- R-matrix inversion (LINPACK-style solver) | 939 | 13 | — |
| 2026-07-07 | `f465587c4` | njoy: split samm::coulomb into a directory module by function... | 1,168 | 1,097 | — |
| 2026-07-07 | `9c6c9b5ac` | njoy: SAMM Phase 5 -- cross-section formula (first end-to-end... | 757 | 30 | — |
| 2026-07-07 | `2dfe17b40` | njoy: SAMM Phase 6 -- top-level orchestration (setup + cssammy) | 215 | 20 | — |
| 2026-07-07 | `7651ce3dc` | njoy: wire samm (LRF=7 R-Matrix Limited) into RECONR's resona... | 173 | 20 | — |
| 2026-07-07 | `ffc0d00e4` | njoy: fix U-238 capture wing pedestal (RECONR grid density) +... | 726 | 47 | — |
| 2026-07-07 | `368531408` | openmc: add Godiva HIGH-fidelity k_eff V&V doc (validation.md) | 58 | 0 | — |
| 2026-07-07 | `23fbd8138` | added trademarks and attributions to every crate | 1,130 | 0 | — |
| 2026-07-08 | `c87bbb8e2` | bd init: initialize beads issue tracking | 740 | 0 | — |
| 2026-07-08 | `62ae61315` | workspace: adopt beads (bd) for issue/roadmap tracking; seed ... | 73 | 1 | — |
| 2026-07-08 | `d1fe5a33d` | beads: defer njoy fissile-path speedup (research-worthy); mar... | 2 | 2 | — |
| 2026-07-08 | `b97198bd9` | beads: track tampines blowdown-test PR (Ethan) + trademark-co... | 10 | 0 | — |
| 2026-07-08 | `5ce3b023f` | beads: prke/tuas inline matrix solvers (drop openfoam-basic-l... | 6 | 2 | — |
| 2026-07-08 | `54fe11797` | beads: plan outram-park-fork-coolprop (CoolProp Rust translat... | 1 | 0 | — |
| 2026-07-08 | `9fba09c38` | beads: refresh issues.jsonl export (coolprop cross-links + note) | 7 | 2 | — |
| 2026-07-08 | `06db42b8f` | android: gate ndarray-linalg matrix bench off Android + add A... | 40 | 0 | — |
| 2026-07-08 | `00163d3fc` | beads: refresh export (Android epic links) | 6 | 2 | — |
| 2026-07-08 | `89dd39072` | prke/tuas: inline SquareMatrix LU solver; drop openfoam-basic... | 596 | 11 | — |
| 2026-07-08 | `66e4632a7` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `7fdda9f63` | coolprop: initiate outram-park-fork-coolprop; first verified ... | 766 | 3 | — |
| 2026-07-08 | `23474425d` | beads: refresh export | 6 | 2 | — |
| 2026-07-08 | `3fa43ec07` | coolprop: add Helium fluid + EnthalpyEntropyOffset ideal term | 105 | 1 | — |
| 2026-07-08 | `cf32270cf` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `be3a19407` | coolprop: full 137-fluid codegen coverage + term-type engine ... | 800 | 14 | — |
| 2026-07-08 | `853a85117` | coolprop: wire all 137 CoolProp fluids into the Fluid enum | 5,168 | 69 | — |
| 2026-07-08 | `b2a903ed2` | coolprop: vendor openfoam_algorithms + add OPCPFluidSingleCV ... | 17,074 | 21 | — |
| 2026-07-08 | `d396febf8` | coolprop: rename TampinesSteamArray -> OPCPFluidArray + wire ... | 113 | 48 | — |
| 2026-07-08 | `1231c5e02` | coolprop: drop the unused vendored solvers, keep only rhoPimp... | 23 | 609 | — |
| 2026-07-08 | `0cff3fde7` | coolprop: add transport (mu/lambda), saturation ancillaries, ... | 2,795 | 48 | — |
| 2026-07-08 | `a90cb2b0f` | coolprop: port Helium's hardcoded transport (mu + lambda) | 193 | 25 | — |
| 2026-07-08 | `25433b20c` | coolprop: port Water (IAPWS) and CO2 (Laesecke/Huber) hardcod... | 189 | 25 | — |
| 2026-07-09 | `840ef1ae6` | changed tuas to v0.1.3 | 7 | 3 | — |
| 2026-07-09 | `a69e0e76b` | added teh-o-prke to next level, will yank v0.1.1 | 21 | 3 | — |
| 2026-07-10 | `48fb5de70` | coolprop: expand hardcoded transport + add Olchowy-Sengers cr... | 747 | 310 | — |
| 2026-07-10 | `67be164c6` | coolprop: finish all per-fluid hardcoded transport formulas | 366 | 44 | — |
| 2026-07-10 | `1ba44705c` | coolprop: scaffold HumidAir, incompressibles, mixtures, non-a... | 804 | 8 | — |
| 2026-07-10 | `fd4b8e850` | coolprop: implement non-analytic terms, HumidAir, incompressi... | 1,661 | 299 | — |
| 2026-07-10 | `fc0b51637` | coolprop: document why the remaining scaffold gaps are scoped... | 41 | 4 | — |
| 2026-07-10 | `28f8a90f3` | coolprop: port all 126 CoolProp incompressible fluids via cod... | 3,802 | 114 | — |
| 2026-07-10 | `70066fe2d` | coolprop: port 840 CoolProp mixture binary pairs via codegen | 2,539 | 141 | — |
| 2026-07-10 | `90cffff84` | workspace: scaffold verification_and_validation/, upstream_so... | 3,217 | 177 | — |
| 2026-07-10 | `136d1a4e8` | coolprop: add auto-generated docs/api.md via rustdoc-md (nigh... | 14,283 | 1 | — |
| 2026-07-10 | `50ea849a9` | beads: file op-4wl.2 (replace RELAP/Zweibaum loss coeffs with... | 1 | 0 | — |
| 2026-07-10 | `130ecd4af` | added debug to opcp fluid array | 1 | 1 | — |
| 2026-07-10 | `3017f6a7b` | openfoam-*-lib: add docs/api.md; file TODO to rename to outra... | 15,131 | 10 | — |
| 2026-07-10 | `ecebd5656` | beads: refresh export (op-ahi epic note timestamp) | 3 | 3 | — |
| 2026-07-11 | `9034104b5` | CLAUDE.md: add mandatory working-hours guardrail (AI safety) | 38 | 0 | — |
| 2026-07-11 | `abe0e57f4` | beads: file op-fvc (cross-platform work-hours time-check scri... | 1 | 0 | — |
| 2026-07-13 | `b5c8bd8da` | Rename OpenFOAM/OpenMC crates to outram-foam-*/outram-mc-libs | 8,726 | 586 | — |
| 2026-07-13 | `ec21a7004` | docs: add valuation.md (cost/value estimate); close op-cpe | 144 | 5 | — |
| 2026-07-13 | `194c75dc3` | coolprop: implement HumidAir wet-bulb + dew-point; add missin... | 498 | 36 | — |
| 2026-07-13 | `c4e69746f` | coolprop: fix stale ha_props doc comment (T_wb/T_dp are now o... | 8 | 4 | — |
| 2026-07-13 | `cbb2460d4` | coolprop: HumidAir entropy S + T_wb/T_dp as input keys (op-kb... | 514 | 79 | — |
| 2026-07-13 | `386cba4bc` | outram-foam/outram-mc crates: align version to 0.1.0 | 4 | 4 | — |
| 2026-07-13 | `c58aae965` | added important provenance does | 25 | 11 | — |
| 2026-07-13 | `1c2293deb` | docs: finish provenance pass -- dwsim README, OpenFOAM matrix... | 60 | 6 | — |
| 2026-07-13 | `d144faeae` | added new digital twin plan markdown and beads | 195 | 1 | — |
| 2026-07-13 | `913d591e1` | coolprop: wire lateral coupling + heat source into step(), cl... | 377 | 2 | — |
| 2026-07-13 | `5bca4347f` | tampines crate added | 141 | 25 | — |
| 2026-07-13 | `e778c4c11` | did some housekeeping and AI USAGE markdowns | 652 | 10 | — |
| 2026-07-13 | `c61d14513` | added responsible use to outram park | 338 | 23 | — |
| 2026-07-13 | `bd1df9265` | docs: wire RESPONSIBLE_USE/DATA_POLICY/AI_USAGE/etc. into roo... | 246 | 46 | — |
| 2026-07-13 | `5b18f6c3b` | dwsim-libs: port pipe correlations (Darcy-Weisbach + Beggs-Br... | 722 | 3 | — |
| 2026-07-13 | `da722cc0b` | docs: fold in AI Agent Instructions (license/provenance prese... | 10 | 0 | — |
| 2026-07-13 | `5f53f5e84` | dwsim-libs: port valve/heat_exchanger/expander/pump, wire int... | 1,351 | 20 | — |
| 2026-07-13 | `e64b29bc8` | dwsim-libs: close out the 4 deferred DWSIM ports (op-qo2.6-9) | 1,080 | 11 | — |
| 2026-07-13 | `e82c8f599` | tampines: add components/ -- 8 BOP component structs (op-dt3.4) | 439 | 1 | — |
| 2026-07-13 | `f0e1869b3` | tampines: add balance_of_plant/ + cooling_tower/ grouping mod... | 82 | 1 | — |
| 2026-07-13 | `db818e0e6` | outram-park-digital-twin-gui: crate scaffold + color_maps + c... | 741 | 3 | — |
| 2026-07-13 | `ec1482dd2` | outram-park-digital-twin-gui: add animation/ tracer/travel-ti... | 126 | 1 | — |
| 2026-07-13 | `3b4fdfc20` | outram-park-digital-twin-gui: add app_scaffold/ threading/pan... | 189 | 1 | — |
| 2026-07-13 | `58ce36502` | tampines-steam-tables: give TampinesSteamArray a FluidArray/O... | 445 | 14 | — |
| 2026-07-13 | `356daff56` | tampines-steam-tables: wire real IAPWS-IF97 (p,h) flash into ... | 123 | 24 | — |
| 2026-07-13 | `1c59327f3` | Add DEVELOPER_HEALTH_WARNING.md: agentic development health/s... | 429 | 1 | — |
| 2026-07-14 | `f9f6e1ad4` | Add Nordheim-Fuchs exact timestepper, wire teh-o-prke -> nee_... | 654 | 27 | — |
| 2026-07-14 | `88ffcd91c` | beads: sync issues.jsonl export | 3 | 1 | — |
| 2026-07-14 | `86ccb04b2` | Move fhr_sim_v2 to tampines, reconstruct kinetics on Nordheim... | 251 | 173 | — |
| 2026-07-14 | `540fd4541` | bug fixes in progress | 760 | 54 | — |
| 2026-07-14 | `b49686ef3` | tampines-steam-tables: handle two-phase region in lambda_ph_e... | 172 | 23 | — |
| 2026-07-14 | `5311d4618` | Add OpenFOAM-style pressure bounding to TampinesSteamArray + ... | 779 | 0 | — |
| 2026-07-14 | `032e95f04` | Wire TampinesSteamArray into fhr_sim_v2 steam-generator tube | 425 | 29 | — |
| 2026-07-14 | `45a3ebc58` | tampines-steam-tables README: log the (p,h)-flash coaching as... | 27 | 0 | — |
| 2026-07-15 | `494bc9082` | beads: log vendoring rule (op-264) and kovan crate scaffold (... | 2 | 0 | — |
| 2026-07-15 | `7298c73a1` | docs: add blanket 'unverified until validated' notice to all ... | 332 | 0 | — |
| 2026-07-15 | `9274eb6c0` | njoy: fix SAMM mf2 eliminated-channel reorder + partial ERROR... | 4,348 | 99 | — |
| 2026-07-15 | `8fd326360` | beads: sync export after NJOY fleet run (op-cjw.3 closed, op-... | 5 | 3 | — |
| 2026-07-15 | `27744b726` | Port NJOY front-ends: GROUPR/GAMINR/LEAPR/COVR/MIXR/RESXSR/DT... | 10,875 | 203 | — |
| 2026-07-15 | `881b86e4d` | kovan: add KOVAN knowledge layer as 7 workspace member crates | 2,838 | 8 | — |
| 2026-07-15 | `f63cdb420` | beads: sync export (op-145 kovan scaffold closed; NJOY-rest f... | 4 | 2 | — |
| 2026-07-15 | `0d21e93ec` | beads: add KOVAN epic op-5v5 + implementation child beads | 5 | 0 | — |
| 2026-07-15 | `bd5222469` | docs/beads: openmc-notebooks become verification tests; outra... | 43 | 0 | — |
| 2026-07-15 | `66915b194` | outram-mc: scaffold openmc-notebooks verification harness + m... | 877 | 0 | — |
| 2026-07-15 | `7a0d9ba87` | beads: sync export (outram-mc notebook track op-6tz.7-.22) | 24 | 0 | — |
| 2026-07-15 | `d32e223de` | njoy op-6tz.6: OpenMC data-notebooks as verification tests (m... | 915 | 0 | — |
| 2026-07-15 | `39de0cb4e` | beads: THERMR H-in-H2O bead op-cjw.19 (blocks op-6tz.12 therm... | 13 | 7 | — |
| 2026-07-15 | `44e2ebf7d` | njoy thermr op-cjw.19: complete H-in-H2O S(alpha,beta) incohe... | 764 | 32 | — |
| 2026-07-15 | `ad7ee71ff` | beads: THERMR op-cjw.19 closed; follow-ups op-cjw.20/.22/.23 | 2 | 1 | — |
| 2026-07-15 | `38faf1f8d` | outram-mc op-6tz.7/.8/.9/.10/.16: wire CSG geometry + surface... | 1,876 | 167 | — |
| 2026-07-15 | `853d4ba74` | beads: outram-mc pincell/triso track (op-6tz.7/.8/.9/.10 clos... | 9 | 2 | — |
| 2026-07-15 | `377a68710` | outram-mc: wire H-in-H2O S(a,b) thermal scattering into trans... | 920 | 37 | — |
| 2026-07-15 | `c3673cd43` | beads: thermal pincell op-6tz.12 closed (k_inf=1.398); follow... | 10 | 6 | — |
| 2026-07-15 | `cafc3d450` | Session batch: engine rename, KOVAN implementation, TRISO-ATO... | 32,067 | 5,211 | — |
| 2026-07-15 | `6c3fcfa1f` | outram-mc: compare pincell k_inf vs openmc pincell.ipynb; CSV... | 24 | 1 | — |
| 2026-07-15 | `10849ffa1` | vv: commit pincell k_inf comparison CSV (force-added past fol... | 2 | 0 | — |
| 2026-07-15 | `0db6f0211` | CLAUDE.md: add optional Singlish mode (chat prose only) | 25 | 0 | — |
| 2026-07-15 | `044f1f6ef` | outram-mc op-6tz.11: port OpenMC HexLattice + make hexagonal-... | 1,136 | 21 | — |
| 2026-07-15 | `b5ece92a4` | beads: hex-lattice op-6tz.11 closed; follow-ups op-6tz.29/.31 | 2 | 1 | — |
| 2026-07-15 | `ece1872bb` | boon-lay: TRISO-ATOPS first-principles derivation docs (Pytho... | 718 | 3 | — |
| 2026-07-15 | `ce8f82866` | outram-mc notebooks: DAGMC won't-port, pandas on hold, unstru... | 18 | 14 | — |
| 2026-07-15 | `e161253d1` | outram-mc op-6tz.16/.25: random TRISO packing + delta-trackin... | 1,552 | 141 | — |
| 2026-07-15 | `2877d9c52` | beads: triso finish track (op-6tz.16/.25 random packing + del... | 16 | 10 | — |
| 2026-07-15 | `c7a749a68` | tampines (p,h)-flash R4/R5 hardening + TUAS CIET pipe-38 K=17... | 1,177 | 235 | — |
| 2026-07-15 | `6dd420e65` | njoy P2/P4 fleet pass: GROUPR vector engine, LEAPR MF=7, COVR... | 8,113 | 958 | — |
| 2026-07-15 | `89340ac19` | tuas: SAM (NED-2021 Table 4) comparison columns + AI-generate... | 838 | 18 | — |
| 2026-07-15 | `d9c293e22` | beads: RPT reproduction intake bead op-vfb; depletion/op-3ut ... | 7 | 2 | — |
| 2026-07-15 | `4a6d60822` | outram-foam-basic-lib: tensor/vector-field FV operators (op-y... | 4,927 | 1,855 | — |
| 2026-07-15 | `f4c10b611` | outram-foam-appbuilder-lib: GeN-Foam multiphysics port (neutr... | 24,147 | 95 | — |
| 2026-07-15 | `a56a1974e` | beads: fleet progress sync (op-3ut GROUPR matrix + depletion ... | 2 | 1 | — |
| 2026-07-15 | `ac56abdbd` | outram-mc: depletion / transmutation driver + LIVE depletion ... | 2,421 | 12 | — |
| 2026-07-15 | `ef567a5db` | beads: depletion op-6tz.18 closed (CRAM); follow-ups op-23s/o... | 2 | 1 | — |
| 2026-07-15 | `87d093f34` | groupr matrix path: skeleton modules (matrix, unresolved, gam... | 112 | 0 | — |
| 2026-07-15 | `91d0bc88a` | groupr/gaminr matrix path (op-3ut): scatter matrix, CM->lab k... | 4,057 | 87 | — |
| 2026-07-15 | `df8a82b8e` | beads: GROUPR matrix path (op-3ut) sync | 4 | 2 | — |
| 2026-07-15 | `fc38e49c9` | fhr_sim_v2: fix reactor-power oscillation via delayed-neutron... | 1,422 | 261 | — |
| 2026-07-15 | `0ade90a50` | outram-park-digital-twin-engine: scaffold htgr_sim_v1 on the ... | 1,538 | 0 | — |
| 2026-07-15 | `3e2652bbe` | beads: reconcile Dolt <-> JSONL export (union of session + re... | 60 | 4 | — |
| 2026-07-15 | `7035aae6f` | htgr_sim_v1: wire kinetics slot to the real teh_o_prke::Delay... | 76 | 200 | — |
| 2026-07-15 | `8a59f7f39` | GeN-Foam completion: SP3, SN, thermalHydraulics (one-phase+BC... | 7,807 | 107 | — |
| 2026-07-15 | `b7ec19f05` | kovan-cli: 'kovan setup' bootstrap for curated CLI tools; git... | 417 | 10 | — |
| 2026-07-15 | `46194ba62` | teh-o-prke: implicit backward-Euler delayed-neutron precursor... | 229 | 80 | — |
| 2026-07-15 | `0acc13f0b` | nee_soon: Xin Wang thesis -> markdown + njoy->openmc->genfoam... | 1,600 | 4 | — |
| 2026-07-15 | `4d57e0b7d` | kovan-cli: add gitoxide (gix) to 'kovan setup' tool list — pu... | 9 | 0 | — |
| 2026-07-15 | `d3e48c245` | njoy groupr: self-shielded (Bondarenko-dilution) MGXS + URR P... | 2,766 | 11 | — |
| 2026-07-15 | `7be2c4129` | beads: self-shielded MGXS op-bsz closed; op-6tz.6.3 advanced;... | 1 | 0 | — |
| 2026-07-15 | `fc1cafc63` | digital-twin-engine: thread-panic "please restart" modal for ... | 514 | 33 | — |
| 2026-07-15 | `9bdb50712` | kovan-discovery: git-awareness via gix (library-first, binary... | 1,974 | 0 | — |
| 2026-07-15 | `2267887ff` | beads: sync export (op-4wv crash-modal, op-5v5.7 gix, op-wqk.... | 28 | 12 | — |
| 2026-07-16 | `509c5487d` | tuas 0.1.4 + tampines-steam-tables 0.2.2: version bump + chan... | 23 | 6 | — |
| 2026-07-16 | `9f8d2e22e` | outram-park-fork-coolprop 0.1.0: GPL-3.0 relicense + honesty ... | 772 | 25 | — |
| 2026-07-16 | `5d481e52d` | outram-park-fork-coolprop: exclude upstream_source/CoolProp f... | 5 | 1 | — |
| 2026-07-16 | `88fe52d45` | outram-foam basic/turbulence/appbuilder: flag limitations in ... | 364 | 54 | — |
| 2026-07-16 | `595d8e5e5` | outram-park-fork-coolprop: wave-1 property additions (op-kbc.... | 6,675 | 547 | — |
| 2026-07-16 | `bbde28ab4` | workspace: gate Android-hostile deps/examples off aarch64-lin... | 366 | 259 | — |
| 2026-07-16 | `56cc5bf79` | bookkeeping: doc-comment pass + README status flags + api.md ... | 19,921 | 9,562 | — |
| 2026-07-16 | `5a0dcd920` | bookkeeping: add "Bookkeeping pass" command to CLAUDE.md; not... | 99 | 4 | — |
| 2026-07-16 | `2aeeb0949` | beads: sync export (pflotran op-v6s + wave-1/pki bookkeeping) | 14 | 0 | — |
| 2026-07-16 | `44975542e` | bookkeeping: fix workspace doc drift (root README/CLAUDE.md +... | 2,669 | 527 | — |
| 2026-07-16 | `6f2005a37` | beads: bookkeeping housekeeping — retitle stale-named epics, ... | 11 | 10 | — |
| 2026-07-16 | `074c2c8c8` | outram-foam-appbuilder-lib: exclude tutorials/ from the packa... | 5 | 1 | — |
| 2026-07-16 | `838051393` | tampines-steam-tables 0.2.3: Edwards blowdown V&V + flashing-... | 1,916 | 10 | — |
| 2026-07-16 | `46b75a6f3` | tampines-steam-tables 0.2.4: stabilise HybridAllMach over the... | 108 | 28 | — |
| 2026-07-16 | `03690a7a7` | Document TampinesSteamArray solver (derivation + debugging) +... | 1,956 | 19 | — |
| 2026-07-16 | `9b7c1dbff` | beads sync (op-ek2 coolprop port, op-21g.15.7 hybrid stabilit... | 27 | 1 | — |
| 2026-07-16 | `733e888bb` | beads: sync export tick | 1 | 1 | — |
| 2026-07-17 | `a2d202515` | outram-mc-libs: tally scoring + multigroup transport — flux_s... | 1,330 | 78 | — |
| 2026-07-17 | `110b7c1a8` | outram-foam-mesh: new crate — blockMesh, ideasUnvToFoam, poly... | 5,865 | 3 | — |
| 2026-07-17 | `8fe53754a` | beads: sync export tick (op-ax7 mesh) | 5 | 3 | — |
| 2026-07-17 | `36428ef4b` | outram-foam-basic-lib: OpenFOAM ASCII dictionary + case I/O (... | 2,947 | 1 | — |
| 2026-07-17 | `f819cc041` | outram-foam-cli: scaffold OpenFOAM-style CLI (per-tool binari... | 350 | 0 | — |
| 2026-07-17 | `59305f2d1` | outram-foam-cli: wire tools to the case interface + OpenFOAM ... | 1,532 | 64 | — |
| 2026-07-17 | `74a37d6cf` | beads: sync export tick (op-x8x CLI + follow-ups) | 3 | 1 | — |
| 2026-07-17 | `9b4509b11` | Add outram-blender scaffold: Blender-inspired mesh-authoring ... | 2,059 | 0 | — |
| 2026-07-17 | `73fbfb5e9` | outram-mc: convert 6 tractable openmc-notebook tests to LIVE ... | 2,560 | 94 | — |
| 2026-07-17 | `9896326b6` | outram-blender: keep name, clearly mark as a Blender fork in ... | 22 | 15 | — |
| 2026-07-17 | `e3b33c95a` | beads: outram-blender naming decision op-hzs.8 closed | 1 | 0 | — |
| 2026-07-17 | `ed2ba8641` | outram-blender: add faer (pure-Rust dense+sparse LA) for futu... | 709 | 2 | — |
| 2026-07-17 | `e9daba369` | workspace: declare wgpu 29.0.3 (matches eframe stack); GUI/GP... | 7 | 0 | — |
| 2026-07-17 | `148f48fbf` | outram-blender: feature-gated headless GPU compute module (wg... | 72 | 0 | — |
| 2026-07-17 | `8bd4af035` | outram-blender: real headless GPU compute path + WGSL affine ... | 599 | 22 | — |
| 2026-07-17 | `e33401328` | beads: sync tick (outram-blender gpu op-hzs.10 closed) | 1 | 0 | — |
| 2026-07-17 | `2f74f35bd` | outram-mc-libs: optional wgpu GPU compute -- batched pointwis... | 993 | 0 | — |
| 2026-07-17 | `e37407b34` | njoy-outram-park-fork: optional wgpu GPU compute (target-gate... | 744 | 0 | — |
| 2026-07-17 | `894aec606` | outram-blender: replace unsafe block_on with safe Wake-based ... | 22 | 30 | — |
| 2026-07-17 | `273ae7c19` | beads: sync tick (njoy/outram-mc/blender wgpu fleets) | 1 | 0 | — |
| 2026-07-17 | `8e5d1e583` | beads: reconcile Dolt<->JSONL (union of session + work-PC bea... | 36 | 10 | — |
| 2026-07-17 | `be4279d78` | beads: auto-export working post-reconcile (benchmark fleet be... | 1 | 2 | — |
| 2026-07-17 | `dfe65ee39` | outram-mc: GPU-vs-CPU benchmarks (Godiva k_eff + HIGH-fidelit... | 1,867 | 2 | — |
| 2026-07-17 | `2b0aed5b2` | beads: GPU benchmark fleet (op-nx0/op-6tz.37 results, op-h23 ... | 3 | 3 | — |
| 2026-07-17 | `6b125f58b` | outram-mc: add ComputeType backend selector (single/multi/GPU... | 1,082 | 16 | — |
| 2026-07-17 | `71b63e500` | beads: ComputeType/GPU-in-transport (op-u6s.4 progress, op-u6... | 3 | 2 | — |
| 2026-07-17 | `33ed8fbef` | beads: sync tick (GPU + sod fleets in flight) | 3 | 1 | — |
| 2026-07-17 | `73fa32b2a` | outram-foam-appbuilder: auto-emit plottable Sod shock tube CS... | 417 | 0 | — |
| 2026-07-17 | `eacdbde14` | beads: sod CSV agent (op-czx) | 5 | 1 | — |
| 2026-07-17 | `3fbf67b10` | outram-mc: deepen GPU penetration — native union grid + event... | 2,766 | 37 | — |
| 2026-07-17 | `a61c95d46` | beads: GPU deep-transport (op-u6s.7 honest no-crossover, op-u... | 1 | 1 | — |
| 2026-07-17 | `f6345c6ae` | njoy gpu: full-fidelity WMP Faddeeva pole-sum on GPU (WGSL) +... | 1,532 | 19 | — |
| 2026-07-17 | `00662c584` | beads: njoy Faddeeva GPU (op-0m5 closed, op-0nh f32-accuracy ... | 7 | 0 | — |
| 2026-07-17 | `3a136d0e2` | docs: document the GPU f32/CPU f64 precision-vs-performance t... | 56 | 0 | — |
| 2026-07-17 | `cf05bd7ce` | beads: sync tick (gpu collision fleet in flight) | 6 | 6 | — |
| 2026-07-17 | `29248fa68` | outram-mc: move MC collision physics onto the GPU (op-u6s.8) | 2,583 | 17 | — |
| 2026-07-17 | `13cfd9c14` | beads: GPU collision physics op-u6s.8 closed; op-u6s.9 crosso... | 2 | 1 | — |
| 2026-07-17 | `c4e441164` | docs: split Singlish mode into SINGLISH_MODE.md with a mainta... | 91 | 23 | — |
| 2026-07-17 | `b326ff317` | docs(singlish): log 'can can' (short ack) + 'ho sei bo' (Hokk... | 8 | 1 | — |
| 2026-07-17 | `6b02f8844` | docs(singlish): 'ho sei bo' is standalone greeting; 'ur side ... | 7 | 1 | — |
| 2026-07-17 | `caa0a95ee` | outram-blender: implement mesh operators, subdivision, boolea... | 3,460 | 213 | — |
| 2026-07-17 | `fde1543f5` | beads: blender mesh impl (op-hzs.1/.2/.4/.5 closed; .3/.6/.7 ... | 10 | 5 | — |
| 2026-07-17 | `7b293846b` | outram-foam-mesh: implement snappyHexMesh snapping + layers +... | 3,242 | 148 | — |
| 2026-07-17 | `8c7d20696` | beads: outram-foam-mesh snappy (op-ax7.2.1 closed; .2.2 parti... | 8 | 4 | — |
| 2026-07-20 | `f0ff16d85` | outram-blender: compile wgpu unconditionally on desktop, grac... | 172 | 41 | — |
| 2026-07-20 | `aca40421c` | gitignore: exclude Claude Code agent worktrees (.claude/workt... | 5 | 0 | — |
| 2026-07-20 | `84fd281b4` | outram-blender: add robust predicates + point-in-mesh classif... | 1,459 | 0 | — |
| 2026-07-20 | `30ee7e021` | Gitignore vendored NJOY2016 reference build tree | 5 | 0 | — |
| 2026-07-20 | `84ff99137` | njoy(op-6tz.6.4): delayed-group MGXS collapse + live mdgxs-pa... | 433 | 10 | — |
| 2026-07-20 | `2c34042b9` | outram-foam: implement all turbulence models + advance mesh p... | 3,603 | 454 | — |
| 2026-07-20 | `dbe27541e` | outram-blender: general mesh boolean (union/difference/non-co... | 1,184 | 83 | — |
| 2026-07-20 | `6368be8db` | beads: close op-hzs.14 (uv_sphere inward-winding fixed in pri... | 2 | 1 | — |
| 2026-07-20 | `990617701` | outram-foam: regenerate docs/api.md mirrors for turbulence + ... | 4,538 | 108 | — |
| 2026-07-20 | `e6731d697` | njoy(op-cjw.16): LEAPR frequency-integral V&V vs NJOY2016 (H-... | 86 | 0 | — |
| 2026-07-20 | `ce41d4ee7` | vv-data: add repo-tracked reference-ENDF folder (outside crat... | 49 | 0 | — |
| 2026-07-20 | `b9df5cc2e` | outram-blender: CSG export — cylinder fitting, convex-faceted... | 466 | 20 | — |
| 2026-07-20 | `c17e1bd2c` | njoy(op-cjw.16): validate full LEAPR pipeline (T_eff + Debye-... | 102 | 1 | — |
| 2026-07-20 | `5c124ce67` | docs(beads): migrate agent guidance from Go beads/Dolt to bea... | 55 | 30 | — |
| 2026-07-20 | `f45cb03f1` | Scaffold outram-park-fork-pflotran crate (PFLOTRAN fork, epic... | 1,202 | 0 | — |
| 2026-07-20 | `88d0e18bf` | outram-foam-mesh: fix public-doc intra-doc links to private i... | 17 | 17 | — |
| 2026-07-20 | `9ceb04b3b` | outram-blender: rounded (multi-segment) vertex bevel | 283 | 47 | — |
| 2026-07-20 | `53b910a88` | outram-foam-mesh: sync docs/api.md with intra-doc-link fixes | 10 | 10 | — |
| 2026-07-20 | `09ddef751` | beads: reconcile after develop merge — close completed njoy b... | 3 | 3 | — |
| 2026-07-20 | `01a673f3f` | njoy(op-cjw.1): ENDF MF=31/33 covariance-section reader (ERRO... | 493 | 13 | — |
| 2026-07-20 | `6442bb349` | njoy(op-3ut): wire stounr URR tape-locate + getsig PENDF feed... | 343 | 46 | — |
| 2026-07-20 | `6f12c6407` | outram-blender: real-type export bridges to outram-foam-basic... | 285 | 15 | — |
| 2026-07-20 | `c2f6bf01e` | outram-blender: update Cargo.lock for optional export-bridge ... | 2 | 0 | — |
| 2026-07-21 | `4355620c2` | docs(CLAUDE): make native-Termux compilation a HARD RULE (and... | 32 | 11 | — |
| 2026-07-21 | `4bb949acf` | fix(outram-mc): gate godiva_gpu_benchmark example off Android... | 46 | 0 | — |
| 2026-07-21 | `c4b39491c` | docs(CLAUDE): harden Android/Termux rule — all-targets check ... | 27 | 11 | — |
| 2026-07-21 | `ae10b541c` | chore(beads): untrack issues.jsonl compat export, ignore per-... | 10 | 336 | — |
| 2026-07-21 | `dc295db04` | docs(CLAUDE): terminal apps (CLI/ratatui TUI) are IN scope fo... | 10 | 3 | — |
| 2026-07-21 | `bd9e5bff9` | feat(outram-mc): scaffold stochastic-media research track (CL... | 1,251 | 3 | — |
| 2026-07-21 | `cdc19bad5` | refactor(outram-mc): move stochastic-media research track to ... | 85 | 41 | — |
| 2026-07-21 | `7acc79cec` | refactor(outram-mc): rename pebble_beds::stochastic_media to ... | 53 | 37 | — |
| 2026-07-21 | `380d897a2` | outram-mc(op-eby.2): implement classical CLS flight driver | 182 | 27 | — |
| 2026-07-21 | `9966d2184` | outram-mc(op-eby.3): implement SCLS transport driver with ret... | 335 | 23 | — |
| 2026-07-21 | `40187700b` | outram-mc(op-eby.7): RSA vs CLS vs SCLS absorption benchmark ... | 306 | 0 | — |
| 2026-07-21 | `29345c3ad` | outram-mc(op-eby.5): dependency-free kd-tree spatial-index ba... | 246 | 35 | — |
| 2026-07-21 | `8ec0e0b0e` | outram-mc(op-eby.6): adaptive variable-radius SCLS extension | 148 | 1 | — |
| 2026-07-21 | `590a5c41b` | docs: add rhoPimpleFoam common-misconceptions catalogue | 455 | 0 | — |
| 2026-07-21 | `4d295cef3` | docs: backport F5 (converged residuals do not imply correctness) | 34 | 0 | — |
| 2026-07-21 | `5d92be3be` | outram-mc: TRISO stochastic-media tutorial example (CLS/SCLS ... | 158 | 0 | — |
| 2026-07-21 | `5b3fbc84b` | outram-mc: GPU CLS/SCLS TRISO tutorial (wgpu, CPU-referenced) | 959 | 0 | — |
| 2026-07-21 | `2539f12ca` | outram-mc: TRISO four-method discrepancy tutorial (surface/de... | 554 | 0 | — |
| 2026-07-21 | `28764b627` | outram-mc: rework GPU CLS/SCLS tutorial to reuse the stochast... | 54 | 45 | — |
| 2026-07-21 | `dbeaa7a7f` | docs(tuas): cite the peer-reviewed TUAS journal article (jand... | 28 | 0 | — |
| 2026-07-21 | `9d1e35304` | docs(tuas): maintainer clears the V&V bookkeeping axis (CIET-... | 2 | 2 | — |
| 2026-07-21 | `fd8797cf6` | added derivation | 639 | 0 | — |
| 2026-07-22 | `bd548a5b0` | pflotran: wire foam-basic-lib dep + scaffold module skeleton ... | 38 | 7 | — |
| 2026-07-22 | `c18b35389` | docs(pflotran): add AI-directed-decisions review log | 89 | 0 | — |
| 2026-07-22 | `eedda1f6f` | pflotran v1: implement grid, properties, io, Newton-Krylov so... | 4,676 | 13 | — |
| 2026-07-22 | `c8f36dd58` | pflotran: implement RICHARDS flow mode end-to-end (op-v6s.8) | 920 | 111 | — |
| 2026-07-22 | `bcb6cf8ec` | pflotran: full test pyramid (verification/integration/regress... | 478 | 20 | — |
| 2026-07-22 | `1e9971484` | pflotran: hydrostatic gravity verification + worked infiltrat... | 149 | 0 | — |
| 2026-07-22 | `fab428f34` | docs(pflotran): record v1 completion status + human-review ba... | 35 | 0 | — |
| 2026-07-22 | `c010db65c` | pflotran: Celia-1990 validation beads + mass-conservation dia... | 157 | 0 | — |
| 2026-07-22 | `971b0497f` | pflotran: Haverkamp (1977) constitutive model for Celia bench... | 326 | 3 | — |
| 2026-07-22 | `f50e16057` | pflotran: scaffold transport module skeleton (op-v6s.11) | 8 | 0 | — |
| 2026-07-22 | `510893214` | docs(pflotran): record transport translation decisions (D11) ... | 29 | 0 | — |
| 2026-07-22 | `009383240` | pflotran: conservative solute transport + RICHARDS flux coupl... | 905 | 11 | — |
| 2026-07-22 | `df9deead5` | pflotran: scaffold energy (TH) + geochemistry module skeleton... | 17 | 0 | — |
| 2026-07-22 | `70f939ca6` | pflotran: thermal water + rock properties for TH mode (op-v6s... | 527 | 0 | — |
| 2026-07-22 | `aa1875abc` | pflotran: aqueous equilibrium speciation core (op-v6s.12) + m... | 703 | 5 | — |
| 2026-07-22 | `cbf051cf0` | pflotran: TH energy transport + coupling; geochem compile fix... | 894 | 16 | — |
| 2026-07-22 | `2e521c62a` | pflotran: scaffold reactive_transport module (op-v6s.12 coupl... | 8 | 0 | — |
| 2026-07-22 | `bd8056128` | pflotran: block multi-DOF Newton-Krylov solver (op-v6s.4.1) | 1,201 | 0 | — |
| 2026-07-22 | `b75903c8c` | pflotran: scaffold multiphase module skeleton (op-v6s.13) | 8 | 0 | — |
| 2026-07-22 | `6381ddc34` | pflotran: two-phase (air-water) multiphase flow on the block ... | 1,017 | 6 | — |
| 2026-07-22 | `9c5c261a5` | pflotran: reactive-transport coupling (transport + geochemist... | 962 | 6 | — |
| 2026-07-22 | `6346cd3c5` | pflotran: kinetic mineral geochemistry on foam ODE solver (op... | 784 | 0 | — |
| 2026-07-22 | `fa94b421f` | docs(pflotran): update lib.rs module map for all implemented ... | 18 | 14 | — |
| 2026-07-22 | `51d975563` | foam-basic-lib: TVD flux limiters translated from OpenFOAM li... | 225 | 0 | — |
| 2026-07-22 | `3ad750d13` | pflotran: TVD advection in transport via foam-basic-lib FluxL... | 126 | 1 | — |
| 2026-07-22 | `d17e0b4a7` | docs(pflotran): README What-exists-today reflects all transla... | 23 | 18 | — |
| 2026-07-22 | `19365a360` | pflotran: CPU parallelism via rayon (op-v6s.14) | 128 | 81 | — |
| 2026-07-22 | `f9b62e9ad` | pflotran: optional wgpu GPU addon, Android-gated + CPU fallba... | 319 | 0 | — |
| 2026-07-22 | `edf8f34a8` | docs(pflotran): record parallelism decision (D2: rayon + Andr... | 19 | 13 | — |
| 2026-07-22 | `ceb304807` | docs(valuation): add 2026-07-22 token-count update | 63 | 0 | — |
| 2026-07-22 | `e0d72a896` | docs(valuation): add without-AI replacement cost section | 36 | 0 | — |
| 2026-07-22 | `f24a53f46` | boon-lay: Phase 0 — buffer-CLT failure analysis + first-passa... | 509 | 0 | — |
| 2026-07-23 | `aa4101f60` | njoy-tui: mobile-first touchscreen JANIS-like nuclear-data TU... | 2,071 | 0 | — |
| 2026-07-23 | `8c6d8575c` | boon-lay: Phase 1 — CPU Walk-on-Spheres first-passage engine | 386 | 18 | — |
| 2026-07-23 | `db2d15f8b` | boon-lay: Phase 2 — multilayer interface transmission/reflection | 403 | 45 | — |
| 2026-07-23 | `f46b51aa6` | outram-mc-tui: mobile-first touchscreen geometry/run TUI (op-... | 2,138 | 0 | — |
| 2026-07-23 | `ae84a0475` | boon-lay: Phase 3 — decay + transmutation depletion coupled t... | 369 | 0 | — |
| 2026-07-23 | `b883b9ba4` | boon-lay: Phase 4 — CRP-6 kernel release + interface V&V records | 374 | 25 | — |
| 2026-07-23 | `5e82733f4` | boon-lay: Phase 5 — rayon parallel ensemble + wgpu compute sc... | 642 | 2 | — |
| 2026-07-23 | `f47c55d25` | boon-lay: Phase 6 — real-time Walk-on-Spheres TRISO diffusion... | 273 | 0 | — |
| 2026-07-23 | `0aee140f6` | outram-mc: wire ComputeType dispatch into CSG + delta drivers... | 678 | 43 | — |
| 2026-07-23 | `9224884a2` | Move njoy-tui + outram-mc-tui into their library crates as fe... | 155 | 102 | — |
| 2026-07-23 | `933517b09` | pflotran: add tampines-steam-tables dep for real-EOS module (... | 5 | 0 | — |
| 2026-07-23 | `b9007cb21` | pflotran: aqueous activity coefficient models (op-v6s.15.1) | 342 | 0 | — |
| 2026-07-23 | `4d4583f2b` | pflotran: real IAPWS water EOS + radioactive decay chains (op... | 1,102 | 0 | — |
| 2026-07-23 | `fb5b3e309` | pflotran: equilibrium sorption — isotherms + Gaines-Thomas io... | 732 | 0 | — |
| 2026-07-23 | `e6f19f180` | docs(pflotran): add activity/sorption/decay/eos_real to modul... | 7 | 0 | — |
| 2026-07-23 | `7e20770f6` | outram-blender: cotangent/uniform Laplacian + implicit Laplac... | 615 | 6 | — |
| 2026-07-23 | `9bf8a798f` | pflotran: wire radioactive decay into solute transport (op-v6... | 59 | 1 | — |
| 2026-07-23 | `8c678c66a` | pflotran: wire linear sorption retardation into transport (op... | 97 | 5 | — |
| 2026-07-23 | `8f1e9668e` | pflotran: microbial (Monod) biodegradation reactions (op-v6s.... | 853 | 0 | — |
| 2026-07-23 | `3ffe7ecfb` | pflotran: wells/advanced-BCs and real-deck parser modules (op... | 1,768 | 2 | — |
| 2026-07-23 | `7e3fa96fa` | pflotran: log D14 upstream-parity wave (op-v6s.15.*) in AI-de... | 53 | 0 | — |
| 2026-07-23 | `0e2336d91` | pflotran: Pitzer ion-interaction activity model for brines (o... | 499 | 0 | — |
| 2026-07-23 | `9475ecbc5` | examples: graceful ENDF-fetch failure in the two HIGH-fidelit... | 55 | 20 | — |
| 2026-07-23 | `480048bda` | pflotran: unstructured finite-volume grid (TPFA) module (op-v... | 728 | 0 | — |
| 2026-07-23 | `0a3cac0e2` | pflotran: CO2 (Redlich-Kwong) + NaCl brine EOS module (op-1y6) | 750 | 0 | — |
| 2026-07-23 | `ed715c763` | pflotran: surface-complexation sorption (NEM/CCM/diffuse-laye... | 936 | 0 | — |
| 2026-07-23 | `a589fcd12` | pflotran: update D14 log — second parity fleet landed (224 li... | 16 | 2 | — |
| 2026-07-23 | `9cce0b86d` | outram-blender: Taubin (lambda\|mu) shrinkage-free smoothing | 172 | 2 | — |
| 2026-07-23 | `a89f3c6a9` | outram-park-fork-liggghts: new crate + Phase 1 particle frame... | 749 | 0 | — |
| 2026-07-23 | `9002e7a62` | outram-blender: harmonic/Tutte planar parameterization | 404 | 1 | — |
| 2026-07-23 | `e5697b89a` | outram-foam-multiphase: new crate + Stage 1 drift-flux founda... | 1,051 | 0 | — |
| 2026-07-23 | `057d8fe6b` | outram-blender: fix parameterization test determinism + ill-c... | 26 | 9 | — |
| 2026-07-23 | `cc8fe2745` | pflotran: GENERAL air-water-energy + two-way buoyancy TH modu... | 2,833 | 0 | — |
| 2026-07-23 | `69671cfb6` | pflotran: update D14 log — coupled-physics wave (general_mode... | 32 | 6 | — |
| 2026-07-23 | `e08fecfe4` | outram-park-fork-liggghts: correct licensing — LIGGGHTS-PUBLI... | 62 | 59 | — |
| 2026-07-23 | `27cd2101d` | outram-mc: search_for_keff critical-search driver + activate ... | 694 | 0 | — |
| 2026-07-23 | `15324e84d` | outram-blender: ARAP (as-rigid-as-possible) surface deformation | 533 | 0 | — |
| 2026-07-23 | `735c9e2b5` | outram-blender: README — document the sparse-solve operator s... | 11 | 6 | — |
| 2026-07-23 | `e9e549b38` | pflotran: robust convecting-regime solve for thermal_convecti... | 201 | 29 | — |
| 2026-07-23 | `5340a9be2` | outram-blender: QEM mesh decimation (Garland-Heckbert simplif... | 638 | 0 | — |
| 2026-07-23 | `c9be655d8` | njoy: activate mgxs-part-ii scatter-matrix + Chi tests; fix m... | 390 | 52 | — |
| 2026-07-23 | `5f0cb62f7` | outram-blender: Loop subdivision (triangle subdivision surface) | 297 | 0 | — |
| 2026-07-23 | `95fce7833` | outram-blender: refresh stale lib.rs top-doc (whole crate is ... | 16 | 10 | — |
| 2026-07-23 | `a12db7bd7` | feat(tooling): per-commit API-token accounting hooks + docs l... | 462 | 0 | 4,240,534 |
| 2026-07-23 | `74377bf28` | outram-park-mpi: new crate — MPICH-subset shared-memory MPI, ... | 1,180 | 0 | — |
| 2026-07-23 | `e3ef6f5a4` | fix(tooling): token ledger report — use git %x1f/%x1e format ... | 27 | 6 | 6,222,248 |
| 2026-07-23 | `e7fb2d3e4` | chore(docs): refresh token-usage ledger | 2 | 1 | 1,457,487 |
| 2026-07-23 | `605e5126d` | outram-blender: 3D convex hull from a point set | 402 | 0 | — |
| 2026-07-23 | `50cc6f69f` | outram-park-mpi: collectives + reduction ops, milestone 2 (op... | 553 | 19 | — |
| 2026-07-23 | `3dcc6bad1` | outram-park-mpi: add Cargo.lock entry for the new crate | 7 | 0 | — |
| 2026-07-23 | `349d87fec` | outram-blender: exercise the sparse-solve operators in the me... | 88 | 1 | — |
| 2026-07-23 | `9cf651c85` | scripts: add kloc_accounting.py, reproducible line-count acco... | 1,436 | 0 | — |
| 2026-07-23 | `0b1e867b8` | outram-blender: weld / remove-doubles operator (merge coincid... | 400 | 3 | — |
| 2026-07-23 | `43c259c20` | outram-blender: fill-holes operator (cap open boundary loops ... | 259 | 4 | — |
| 2026-07-23 | `a1ea9de1b` | outram-blender: solidify operator (shell thickness -> closed ... | 243 | 5 | — |
| 2026-07-23 | `e273ef27e` | outram-foam Phase II: multiphase Stages 2-5 + DEM Phases 2-5 ... | 6,714 | 20 | — |
| 2026-07-23 | `95fbeda8b` | outram-blender: recalculate-normals operator (consistent wind... | 303 | 5 | — |
| 2026-07-23 | `4de61200f` | scripts: net TUAS against what it imported, not its predecess... | 41 | 9 | — |
| 2026-07-23 | `ad95279c4` | outram-blender: triangulate operator (ngon/quad Mesh -> trian... | 149 | 3 | — |
| 2026-07-23 | `3e65b02fd` | outram-blender: inset-faces operator (per-face inset ring) | 197 | 5 | — |
| 2026-07-23 | `ea2643ea0` | outram-blender: bisect operator (plane cut / half-space clip) | 236 | 5 | — |
| 2026-07-23 | `62b7f22e1` | outram-blender: demo the repair/modeling/cutting operators in... | 69 | 0 | — |
| 2026-07-23 | `79dc78709` | outram-blender: revolve / spin operator (surface of revolution) | 228 | 5 | — |
| 2026-07-23 | `d3b505c8e` | outram-blender: read polyMesh (from_poly_mesh) — round-trip p... | 113 | 8 | — |
| 2026-07-23 | `db91d1753` | docs(singlish): add 'bang gang' — knock off work / finish for... | 1 | 0 | 48,295,948 |
| 2026-07-24 | `2f5969384` | feat(historian): pre-merge-to-main release report generator (... | 482 | 0 | — |
| 2026-07-24 | `6c50cdd2f` | refactor(tooling): consolidate token accounting into docs/his... | 450 | 397 | 7,215,668 |
| 2026-07-24 | `a75a69472` | outram-park-fork-dwsim-libs: port compressor, heater, cooler,... | 2,112 | 0 | — |
| 2026-07-24 | `05f449c72` | outram-park-fork-liggghts: DEM engine + rolling/cohesion, mes... | 3,322 | 0 | — |
| 2026-07-24 | `db924dd32` | Add outram-park-fork-cfmesh scaffold: vendored cfMesh + voro+... | 962 | 0 | — |
| 2026-07-24 | `744b73268` | outram-blender: STL import/export (ASCII + binary) | 351 | 0 | — |
| 2026-07-24 | `93e784744` | outram-park-fork-dwsim-libs: Tier-1 thermodynamics kernel (EO... | 3,554 | 0 | — |
| 2026-07-24 | `035bfaa7b` | outram-blender: edge bevel (chamfer every edge) | 248 | 0 | — |
| 2026-07-24 | `b195d3477` | docs(readme): add a (clearly-in-jest) Phua Chu Kang tagline | 5 | 0 | 6,734,604 |
| 2026-07-24 | `071e4ff36` | outram-park-fork-cfmesh: volume-mesh core + Cartesian block m... | 526 | 15 | — |
| 2026-07-24 | `7693155eb` | outram-mc: port remaining CSG quadric surfaces — Plane, X/YCy... | 507 | 4 | 51,484,074 |
| 2026-07-24 | `fc7c55ff0` | chore(docs): refresh token-usage ledger | 15 | 7 | 3,281,946 |
| 2026-07-24 | `3e60439b0` | chore(docs): refresh token-usage ledger (hooks bypassed to se... | 2 | 1 | — |
| 2026-07-24 | `13040a88b` | outram-park-fork-cfmesh: castellated Cartesian surface carve ... | 375 | 39 | — |
| 2026-07-24 | `1222b1877` | outram-park-fork-dwsim-libs: compose thermo kernel — PT/PH fl... | 1,898 | 0 | — |
| 2026-07-24 | `6e35ca51e` | outram-park-fork-liggghts: bonded-particle model + pebble-bed... | 1,193 | 0 | — |
| 2026-07-24 | `9b8fdf006` | outram-park-fork-cfmesh: boundary snapping (staircase -> body... | 255 | 7 | — |
| 2026-07-24 | `1f4dd31fc` | outram-foam-multiphase: drift-flux PIMPLE pressure-velocity c... | 722 | 0 | — |
| 2026-07-24 | `d3ac4381e` | outram-park-fork-cfmesh: foam PolyMesh bridge -> solvable FvM... | 160 | 14 | — |
| 2026-07-24 | `dc627ed4f` | outram-park-fork-cfmesh: mesh quality checks (polyMeshGenChec... | 228 | 0 | — |
| 2026-07-24 | `347af8afb` | outram-park-fork-cfmesh: multi-region carve (region between s... | 169 | 93 | — |
| 2026-07-24 | `59d744f69` | outram-park-fork-cfmesh: triangle-soup shape generators for r... | 217 | 0 | — |
| 2026-07-24 | `f7c5f0d9e` | outram-park-fork-cfmesh: coolant-around-pebble end-to-end exa... | 80 | 0 | — |
| 2026-07-24 | `839d7aaec` | outram-park-mpi: communicator dup + split (op-wor) | 333 | 16 | — |
| 2026-07-24 | `a143f0e2c` | outram-mc: fix nested-lattice surface-tracking under-count (o... | 95 | 1 | 35,914,153 |
| 2026-07-24 | `c3ffa1921` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `9bd10015b` | pflotran: MPI domain decomposition + halo exchange, first sli... | 321 | 0 | — |
| 2026-07-24 | `1b9c2849e` | outram-park-fork-cfmesh: per-surface boundary patch separation | 83 | 19 | — |
| 2026-07-24 | `21adbfb0c` | outram-park-fork-cfmesh: bbox-culled multi-hole carve + pebbl... | 100 | 7 | — |
| 2026-07-24 | `2c7c77385` | outram-foam-multiphase: two-fluid Euler-Euler shared-pressure... | 1,007 | 0 | — |
| 2026-07-24 | `67eb5112e` | outram-park-fork-cfmesh: reactor geometry generators + LWR pi... | 206 | 30 | — |
| 2026-07-24 | `46b4ccf92` | outram-mc: port Torus{X,Y,Z} CSG surfaces — completes the Ope... | 689 | 4 | 40,092,252 |
| 2026-07-24 | `f10a00075` | outram-mc: HexLattice 3-D axial rings + X-orientation round-t... | 0 | 809 | 8,028,654 |
| 2026-07-24 | `6a97906c2` | outram-park-fork-cfmesh: OpenFOAM polyMesh disk writer + MSR ... | 84 | 0 | — |
| 2026-07-24 | `78f56b85e` | outram-park-fork-dwsim-libs: saturation, transport, EOS varia... | 2,971 | 0 | — |
| 2026-07-24 | `9f67449fa` | outram-mc: add the lattice/ dir files (complete f10a000) | 1,072 | 0 | 6,064,023 |
| 2026-07-24 | `c849d272a` | chore(docs): refresh token-usage ledger | 4 | 1 | — |
| 2026-07-24 | `5d8b5fff5` | pflotran: HDF5 snapshot I/O via pure-Rust hdf5-pure (op-v6s.1... | 328 | 0 | — |
| 2026-07-24 | `738b04d8f` | outram-park-fork-cfmesh: octree near-wall refinement with pol... | 368 | 0 | — |
| 2026-07-24 | `0ab7d2b0c` | chore: pin transitive kstring to 2.0.2 for rustc 1.94 MSRV | 2 | 2 | 2,508,572 |
| 2026-07-24 | `41052a62e` | docs: regenerate api.md mirrors for dwsim-libs, multiphase, l... | 22,747 | 1 | 1,165,500 |
| 2026-07-24 | `fed0bf341` | chore(docs): refresh token-usage ledger | 2 | 1 | 2,321,347 |
| 2026-07-24 | `f2a83c421` | chore(docs): flush token-usage ledger lag | 2 | 1 | — |
| 2026-07-24 | `01eb0573c` | outram-park-mpi: groups + Cartesian topologies (op-er2) | 450 | 3 | — |
| 2026-07-24 | `9edd1c4ac` | pflotran: distributed conjugate-gradient solve (parallel Kryl... | 249 | 1 | — |
| 2026-07-24 | `b8cfd2ea7` | outram-park-fork-cfmesh: multi-level octree refinement + 2:1 ... | 134 | 48 | — |
| 2026-07-24 | `7d321ed9b` | pflotran: 2-D Cartesian distributed solve (op-gj5, first slice) | 439 | 0 | — |
| 2026-07-24 | `3dc85bc57` | cfmesh: add prism boundary layers (add_boundary_layers) | 393 | 3 | 189,149 |
| 2026-07-24 | `784f727c0` | pflotran: generic distributed CG + real variable-coefficient ... | 318 | 0 | — |
| 2026-07-24 | `aac6a3c38` | pflotran: distributed Jacobi-preconditioned CG (op-gj5) | 148 | 1 | — |
| 2026-07-24 | `efb8acdf6` | cfmesh: add polyhedral (median) dual — one cell per vertex (p... | 326 | 2 | 5,250,656 |
| 2026-07-24 | `e579dd798` | cfmesh: V&V — polyhedral dual bridges to a solvable foam FvMesh | 20 | 0 | 2,974,882 |
| 2026-07-24 | `3f5b3740d` | docs: refresh generated token-usage ledger | 17 | 1 | 891,335 |
| 2026-07-24 | `56eba2433` | docs: refresh generated token-usage ledger (lag row) | 2 | 1 | 754,285 |
| 2026-07-24 | `bacb3bbdc` | docs: refresh generated token-usage ledger (final lag row) | 2 | 1 | — |
| 2026-07-24 | `d654f1533` | pflotran: distributed solve of the REAL assembled LduMatrix (... | 297 | 0 | — |
| 2026-07-24 | `35a4de88e` | outram-foam-appbuilder-lib: reactingTwoPhaseEulerFoam applica... | 26,646 | 1,045 | 26,210,679 |
| 2026-07-24 | `f4624b1ab` | chore(docs): flush token-usage ledger lag | 15 | 1 | — |
| 2026-07-24 | `bfbc356ae` | pflotran: distributed BiCGStab for the non-symmetric transpor... | 217 | 1 | — |
| 2026-07-24 | `22ea3bc3b` | pflotran: truly-distributed per-rank LduMatrix assembly (op-gj5) | 106 | 0 | — |
| 2026-07-24 | `2e11d6142` | docs: refresh generated token-usage ledger (merge lag row) | 9 | 1 | — |
| 2026-07-24 | `d3f027514` | njoy + outram-mc: make ratatui unconditional — TUI bins alway... | 67 | 90 | 32,670,485 |
| 2026-07-24 | `af6a88b1a` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `483fd6089` | chore(tooling): untrack docs/token-usage.md — commit trailers... | 20 | 47 | 15,508,820 |
| 2026-07-24 | `7e3201bd9` | pflotran: distributed transport timestep matching the real se... | 213 | 0 | — |
| 2026-07-24 | `186714774` | outram-foam-appbuilder-lib: reacting-Euler species transport ... | 914 | 13 | 20,596,688 |
| 2026-07-24 | `bac6da5c8` | chore(docs): flush token-usage ledger lag | 12 | 1 | — |
| 2026-07-24 | `e597fb896` | outram-foam-multiphase: bookkeeping pass (doc sync, api.md re... | 48 | 10 | 6,293,774 |
| 2026-07-24 | `477c9ff40` | outram-park-fork-liggghts: bookkeeping pass (doc sync, api.md... | 389 | 147 | 3,990,081 |
| 2026-07-24 | `fddb1a936` | outram-foam-appbuilder-lib: bookkeeping pass (doc gaps + stal... | 331 | 54 | 5,605,508 |
| 2026-07-24 | `f4c3aaba5` | outram-park-fork-dwsim-libs: bookkeeping pass (doc gaps + hon... | 1,282 | 410 | 6,102,931 |
| 2026-07-24 | `e33b8e74f` | chore(docs): flush token-usage ledger lag | 5 | 1 | — |
| 2026-07-24 | `590b0e0c8` | fix(outram-mc): correct hex-lattice ring→tile fill (op-6tz.38) | 280 | 12 | 46,781,908 |
| 2026-07-24 | `69d88742b` | fix(outram-mc): compose reflective-corner reflections in one ... | 296 | 3 | 7,850,956 |
| 2026-07-24 | `0a34f095b` | kovan-literature: add open nuclear digital-twin/shadow litera... | 491 | 0 | 43,506,855 |
| 2026-07-24 | `6d6090743` | kovan-literature: lock down verified citations in DT/shadow r... | 61 | 33 | 3,345,634 |
| 2026-07-24 | `9a7b5819b` | kovan-literature: add inline verification-status marker to ev... | 36 | 3 | 6,177,955 |
| 2026-07-24 | `e27d65ef9` | kovan-literature: reframe DT/shadow review around where Outra... | 90 | 13 | 17,889,996 |
| 2026-07-24 | `9d86853f4` | cfmesh: add tetrahedralization (centroid subdivision) — op-hz... | 258 | 2 | 11,477,957 |
| 2026-07-24 | `b2cece56f` | pflotran: distributed transport for non-uniform flow (op-gj5) | 187 | 33 | — |
| 2026-07-24 | `f063e119b` | pflotran: distributed transport with Dirichlet boundary condi... | 162 | 3 | — |
| 2026-07-24 | `fa4449c34` | feat(outram-mc): GPU ray-surface distance kernel for all 15 C... | 1,712 | 0 | 21,646,800 |
| 2026-07-24 | `834b7399f` | cfmesh: face-minimal merged dual + smart-Laplacian quality sm... | 512 | 11 | 11,490,536 |
| 2026-07-28 | `521641763` | pflotran: BiCGStab restart-on-breakdown + distributed TVD tra... | 265 | 34 | — |
| 2026-07-28 | `cb391324c` | cfmesh: flip-based Delaunay improvement (2-3/3-2 bistellar fl... | 482 | 3 | 27,713,840 |
| 2026-07-28 | `ce4bd82ea` | outram-foam-appbuilder-lib: V&V cases mirroring upstream drif... | 595 | 0 | 82,163,381 |
| 2026-07-28 | `3eb673137` | pflotran: distributed energy (heat) transport timestep (op-gj5) | 145 | 0 | — |
| 2026-07-28 | `c025bf533` | pflotran: distributed Newton solver for nonlinear systems (op... | 278 | 0 | — |
| 2026-07-28 | `a942f8190` | outram-blender: Monte Carlo simulation backend (materials + r... | 500 | 14 | 28,954,759 |
| 2026-07-28 | `0f8778d33` | outram-blender/sim: add cell flux/nu-fission tally helpers | 49 | 0 | 3,920,399 |
| 2026-07-28 | `c73aa1b15` | docs(CLAUDE.md): hard rule — agent-fleet progress updates eve... | 27 | 0 | 24,913,510 |
| 2026-07-28 | `e3f5fa1e1` | outram-blender: MC Studio — egui GUI to author + run basic ou... | 444 | 0 | 13,126,085 |
| 2026-07-28 | `bb48f2817` | Remove unused distributed_dot import in decomposition::ldu | 1 | 1 | — |
| 2026-07-28 | `001cc2c71` | docs(CLAUDE.md): hard rule — dogfood KOPITIAM in this workspace | 53 | 0 | 7,736,570 |
| 2026-07-28 | `dc3ef82c9` | chore: keep KOPITIAM out of the OUTRAM PARK tree | 24 | 0 | 4,567,509 |
| 2026-07-28 | `4098a82e6` | digital-twin-engine: flow tracers, per-cell pipes, coupled HT... | 2,297 | 249 | — |
| 2026-07-28 | `9d51ac2d3` | docs(CLAUDE.md): hard rule — kopitiam is binary-only, issues ... | 23 | 13 | 1,820,695 |
| 2026-07-28 | `5465ff487` | docs(CLAUDE.md): stop hook authorises push to feature/develop... | 12 | 1 | — |
| 2026-07-28 | `e67d8b0b8` | outram-mc: fixed-source transport driver (run_fixed_source) —... | 349 | 6 | 28,233,775 |
| 2026-07-28 | `442def8f0` | fix(pflotran): restore distributed_dot import at test scope | 3 | 0 | 9,363,777 |
| 2026-07-28 | `7ab5c3c6a` | cfmesh: Mesh Studio — egui GUI for polyhedral meshes + layers... | 447 | 0 | 24,536,996 |
| 2026-07-28 | `a52636a8f` | cfmesh: adaptive boundary layers for curved walls — op-zhh (M... | 293 | 4 | 24,949,768 |
| 2026-07-28 | `3f5b191e2` | build: add async-opcua to workspace dependencies (Android-ver... | 23 | 0 | 31,544,530 |
| 2026-07-28 | `dfc66b003` | docs: reword async-opcua note as scope, not a user restriction | 10 | 5 | 1,876,572 |
| 2026-07-28 | `6d41e60df` | boon-lay: CPU/GPU ComputeType resource switcher + off-thread ... | 994 | 2 | — |
| 2026-07-28 | `57bab46f4` | boon-lay: first_passage_realtime — off-thread compute + CPU/G... | 240 | 134 | — |
| 2026-07-28 | `efa78282f` | boon-lay: triso_simulator retrofit — WoS diffusion + CPU/GPU ... | 147 | 8 | — |
| 2026-07-28 | `b90452dda` | ciet: CIET Educational Simulator v2 with an OPC-UA interface ... | 24,723 | 7 | 417,150 |
| 2026-07-28 | `174e837dc` | release: outram-park-digital-twin-engine 0.1.0 -> 0.2.0 | 3 | 3 | 8,734,716 |
| 2026-07-29 | `4345e350a` | docs: scope the transformation to a Type I digital twin | 291 | 0 | 166,955,527 |
| 2026-07-29 | `eaac02dd1` | docs(dt-scoping): simulator binaries may live in this repo | 36 | 17 | 9,014,289 |
| 2026-07-29 | `e80277020` | docs: scope the OFFBEAT + SCIANTIX port in detail | 188 | 0 | 29,874,580 |
| 2026-07-29 | `0ca34c9e8` | offbeat: new crate + P0 mechanics bridgehead (epic op-6sl, be... | 1,704 | 0 | 190,671,075 |
| 2026-07-29 | `23f4d02ec` | offbeat: module stubs for the remaining port phases + Cargo.lock | 214 | 0 | 2,272,071 |
| 2026-07-29 | `3723c06cc` | offbeat P3/P4: material property correlations, burnup, fast f... | 12,503 | 18 | 11,883,986 |
| 2026-07-29 | `4bc06269a` | offbeat P3: behavioural models -- swelling, densification, re... | 3,516 | 73 | 5,424,301 |
| 2026-07-29 | `b1c308eac` | offbeat P1: rheology -- plasticity and creep constitutive law... | 3,683 | 2 | 16,420,596 |
| 2026-07-29 | `e3053e3e1` | offbeat P2: fuel/cladding gap -- conductance, gas mixture, co... | 5,467 | 2 | 2,931,872 |
| 2026-07-29 | `4b2dfb310` | offbeat P5: cladding corrosion, hydrogen pickup, Anderson mix... | 5,034 | 2 | 7,540,360 |
| 2026-07-29 | `a3342f86f` | docs: list outram-park-fork-offbeat in the workspace member t... | 2 | 0 | 2,320,510 |
| 2026-07-29 | `60d8ac541` | offbeat: add the missing LICENSE, NOTICE and README | 876 | 0 | 43,035,292 |
| 2026-07-30 | `4fe730726` | kovan-literature: correct thesis metadata extraction, add The... | 582 | 20 | — |
| 2026-07-30 | `b92e997f9` | kovan-literature: archive three open-access UC Berkeley theses | 6,031 | 0 | — |
| 2026-08-03 | `6aa61def7` | release: patch-bump 8 crates for the digital-twin publish chain | 31 | 28 | 0 |
| 2026-08-03 | `a0e797256` | release: placeholder 0.0.1 versions, authorship, and shipped ... | 4,298 | 19 | 6,940,322 |
| 2026-08-03 | `ad6143482` | docs: rewrite the crates.io publishing procedure from the 202... | 61 | 25 | 15,060,617 |
| 2026-08-03 | `57991e414` | dwsim-fork: complete chemistry-model survey from upstream source | 217 | 8 | 85,709,336 |
| 2026-08-03 | `0dbe9a145` | dwsim-fork: note HTGR water-ingress relevance in chemistry su... | 26 | 0 | 10,094,049 |
| 2026-08-03 | `b7d5baf10` | dwsim-fork: port SLE flash, Gibbs speciation, electrolyte tie... | 2,892 | 0 | 45,403,356 |
| 2026-08-03 | `6f192e3cd` | dwsim-libs: port reaction + reactor models (op-tts) | 1,908 | 0 | 11,902,529 |
| 2026-08-03 | `6835f3d7f` | dwsim-libs: port advanced EOS (op-b4t) + inside-out & 3-phase... | 2,998 | 0 | 1,604,329 |
| 2026-08-03 | `123968726` | dwsim-libs: Gibbs reactor + Langmuir-Hinshelwood catalytic ki... | 792 | 7 | 12,187,552 |
| 2026-08-03 | `1ed9c1212` | dwsim-libs: port 5 thermo tail modules (op-qo2.10/.13/.14/.19... | 4,482 | 0 | 3,384,663 |
| 2026-08-03 | `14140ac4a` | dwsim-libs: port SVLLE + Modified-UNIFAC-Dortmund + UNIFAC-LL... | 1,851 | 0 | 15,669,333 |
| 2026-08-03 | `22fbd81a6` | cfmesh: high-level tet-dual meshing pipeline (op-0xu) | 588 | 0 | 3,503,482 |
| 2026-08-03 | `6185c1774` | dwsim-libs: port sour-water package (op-qo2.16) | 1,123 | 0 | 5,376,231 |
| 2026-08-03 | `cdece4f37` | outram-blender: tet-dual Mesh Studio GUI + cfmesh bridge (op-... | 767 | 0 | 5,337,056 |
| 2026-08-03 | `ef6ed0ba3` | dwsim-libs: port multi-phase (N-phase) Gibbs minimisation (op... | 1,442 | 0 | 2,458,474 |
| 2026-08-03 | `451bdf924` | chore: update Cargo.lock for outram-park-fork-cfmesh workspac... | 1 | 0 | 1,859,355 |
| 2026-08-03 | `4b8acdc31` | chore: bump versions of crates changed this session (+0.0.1 e... | 12 | 12 | 8,012,763 |
| 2026-08-03 | `b1e7b77d7` | cfmesh: bookkeeping pass — doc/README refresh for the tet-dua... | 57 | 3 | 12,878,348 |
| 2026-08-03 | `93f232c34` | outram-mc-libs: bookkeeping pass — document fixed-source MC d... | 42 | 4 | 0 |
| 2026-08-03 | `1150d2d04` | outram-blender: bookkeeping pass — document MC Studio + Mesh ... | 94 | 29 | 2,235,684 |
| 2026-08-03 | `559cd2fa6` | dwsim-libs: bookkeeping pass — module map, honest scope, surv... | 219 | 99 | 15,228,568 |
| 2026-08-03 | `dd1fc4a28` | workspace: bookkeeping — refresh member tables + stale scaffo... | 39 | 7 | 0 |
| 2026-08-03 | `8622e24f2` | docs: regenerate api.md rustdoc mirrors for the 4 changed crates | 50,075 | 6,137 | 6,048,735 |
| 2026-08-04 | `84938054f` | msre: scaffold outram-park-fork-{onix,thermochimica,moltres} ... | 272 | 0 | 108,134,040 |
| 2026-08-04 | `601ad91b3` | outram-park-fork-onix: port ONIX CRAM depletion solver (op-6w... | 1,699 | 9 | 25,605,061 |
| 2026-08-04 | `b33218c08` | outram-park-fork-thermochimica: port Thermochimica GEM core (... | 1,470 | 0 | 10,783,765 |
| 2026-08-04 | `e082e5d88` | outram-park-fork-moltres: circulating-fuel MSR multiphysics o... | 3,124 | 9 | 7,004,597 |
| 2026-08-04 | `023e0da3b` | outram-foam-basic-lib: add OpenFOAM patch-field boundary cond... | 736 | 10 | 33,272,447 |
| 2026-08-04 | `325ba2957` | outram-foam-basic-lib: functional cyclic (periodic) patches (... | 898 | 21 | 20,204,442 |
| 2026-08-04 | `5423b54a0` | outram-foam-basic-lib: cyclicAMI non-conformal periodic patch... | 1,057 | 25 | 15,489,304 |
| 2026-08-04 | `5c7ccbdfa` | outram-foam-basic-lib: add OpenFOAM flow boundary conditions ... | 674 | 23 | 14,040,264 |
| 2026-08-04 | `5a434fcc8` | outram-blender: ship the GPL licence text with the crate | 674 | 0 | 6,048,938 |
| 2026-08-04 | `58b56de63` | outram-blender: vendor Blender provenance, ship upstream lice... | 493 | 2 | 14,249,477 |
| 2026-08-04 | `5de012326` | release: outram-blender 0.0.1 -> 0.0.2 | 3 | 3 | 5,019,031 |
| 2026-08-04 | `9b42d09f5` | release: outram-blender 0.0.3 — correct the ported-code prove... | 85 | 32 | 3,999,157 |
| 2026-08-04 | `1edb3a92e` | docs: scope a MELCOR-class severe-accident capability | 255 | 0 | 32,124,212 |
| 2026-08-04 | `f726eb018` | outram-foam-appbuilder-lib: restore the build — exhaustive Bo... | 58 | 0 | 816,770 |
| 2026-08-04 | `b44f54916` | widget studio: gallery app, plus a turbine widget driven by r... | 773 | 25 | 1,229,721 |
| 2026-08-04 | `3f3dedbe1` | added scoping for aster and turbine/pipe | 684 | 0 | 28,557,592 |
| 2026-08-04 | `aa7078e09` | boon-lay: fix WGSL shader parse failure — `target` is a reser... | 7 | 3 | 6,023,992 |
| 2026-08-04 | `01e0deb50` | docs: scope the code_aster constitutive-law and fracture port | 73 | 34 | 27,393,538 |
| 2026-08-04 | `1de6660e0` | turbine widget: stator rows, casing, per-stage internals, sym... | 632 | 45 | 28,726,364 |
| 2026-08-04 | `2b3473823` | turbine widget: blade lean adjustments, stator ring, unused c... | 16 | 8 | 11,976,418 |
| 2026-08-04 | `64a4394cd` | widget studio: pipes tab over three flow backends, and a HEM ... | 412 | 23 | 22,714,599 |
| 2026-08-04 | `d2b1cebc1` | basic-lib: port OpenFOAM 3x3 eigen decomposition; start code_... | 5,680 | 3 | 81,012,341 |
| 2026-08-04 | `cdc362f5d` | style: rustfmt basic-lib files that landed unformatted | 366 | 132 | 504,280 |
| 2026-08-04 | `f2721679d` | offbeat: code_aster P0 kinematics -- Mandel Voigt + finite st... | 1,472 | 107 | 17,390,337 |
| 2026-08-04 | `abbf86db9` | offbeat: code_aster P1 local integration algorithms (op-a7p.2) | 990 | 0 | 21,151,406 |
| 2026-08-04 | `12e3b0548` | offbeat: pin the secant observed-order oscillation (op-a7p.2) | 39 | 0 | 2,998,996 |
| 2026-08-05 | `13b67a385` | offbeat: GDEF_LOG finite-strain wrapper + fix a real eigen bu... | 803 | 4 | 26,049,479 |
| 2026-08-05 | `7bd4eac0d` | offbeat: code_aster P2 isotropic viscoplastic creep -- NORTON... | 781 | 0 | 22,775,222 |
| 2026-08-05 | `05123c466` | offbeat: code_aster LEMAITRE_IRRA irradiation creep (op-a7p.3) | 340 | 0 | 16,115,712 |
| 2026-08-05 | `3bca0018f` | dwsim-libs: scope the remaining UNIFAC ports (NIST-Modified U... | 235 | 0 | 264,264,032 |
| 2026-08-05 | `03279cf68` | outram-foam: port the fvOptions/fvModels mechanism + solidifi... | 1,538 | 0 | 42,669,145 |
| 2026-08-05 | `1aefdfc81` | Port DWSIM seawater, hydrocarbon, immiscible & black-oil pack... | 3,567 | 4 | 7,958,648 |
| 2026-08-05 | `ae45c4e3a` | code_aster P2(a): NORTON_HOFF + isotropic hardening radial re... | 1,219 | 0 | 19,039,444 |
| 2026-08-05 | `9d179094c` | code_aster: mid-flight checkpoint of the chaboche/metallurgy/... | 8,787 | 0 | 7,124,139 |
| 2026-08-05 | `bf88c814f` | code_aster: checkpoint 2 -- chaboche and fracture revisions | 512 | 286 | 3,661,705 |
| 2026-08-05 | `6463f5216` | code_aster: land the fracture and Chaboche ports, wire re-exp... | 223 | 96 | 8,550,871 |
| 2026-08-05 | `c70be9915` | code_aster: checkpoint the damage/rupture port (op-a7p.4) | 4,414 | 8 | 2,762,621 |
| 2026-08-05 | `57a024881` | code_aster: land the damage/rupture port (op-a7p.4), wire re-... | 133 | 7 | 5,688,444 |
| 2026-08-05 | `30dce1aea` | code_aster: land the metallurgy port, complete the fleet, ref... | 41 | 3 | 4,275,610 |
| 2026-08-05 | `17c628d44` | docs: bookkeeping pass over offbeat and basic-lib | 34,558 | 713 | 36,209,328 |
| 2026-08-05 | `830a3874b` | docs: restore the astest V&V oracle, correct scoping section 7 | 45 | 25 | 9,591,507 |
| 2026-08-05 | `db9f16a3e` | docs: correct section 7 -- comp0* is not the oracle I claimed | 33 | 10 | 11,143,584 |
| 2026-08-05 | `19ae058f0` | docs: identify ssnv101a as the entry point for astest verific... | 42 | 6 | 8,526,284 |
| 2026-08-05 | `f42b6c88d` | code_aster: first astest verification -- Chaboche reproduces ... | 416 | 0 | 16,008,252 |
| 2026-08-05 | `eedd40420` | docs: ssnv113a is unusable -- missing material include, and w... | 14 | 0 | 15,313,203 |
| 2026-08-05 | `f895dad5c` | astest harness: mixed strain/stress control + DEFI_FONCTION/D... | 569 | 0 | 21,998,464 |
| 2026-08-05 | `00a38e4d2` | code_aster: upstream's own tolerances corroborate the VENDOCH... | 0 | 0 | 15,246,459 |
| 2026-08-05 | `3c471b26f` | astest: wire ssnv126a -- VENDOCHAB does NOT yet reproduce ups... | 457 | 0 | 13,409,118 |
| 2026-08-05 | `2e9a4e7b3` | astest ssnv126a: correct the diagnosis -- one root cause, not... | 23 | 11 | 10,731,988 |
| 2026-08-05 | `3a1cde97e` | damage: fix a saturation test that fired on every step (VENDO... | 204 | 100 | 25,352,024 |
| 2026-08-05 | `efbf12301` | astest ssnv126a: sub-step convergence study -- first order, a... | 74 | 14 | 16,508,302 |
| 2026-08-05 | `946b12e4c` | PipeVisual: rectangles sized from real geometry, per-cell box... | 357 | 33 | 12,858,918 |
| 2026-08-05 | `da0cd23a7` | PipeVisual: white rectangle tracers crossing each run in exac... | 196 | 14 | 14,605,033 |
| 2026-08-05 | `85ea8d4da` | pipes: single pulsed tracer, and a wall drawn from TUAS's pre... | 315 | 32 | 15,769,406 |
| 2026-08-05 | `ef18cf5b6` | aster: unified isotropic hardening curve (op-fxp, step 1 of 2) | 643 | 0 | 61,056,670 |
| 2026-08-05 | `8cd80dd48` | CHECKPOINT: agent fleet in flight -- ODE enum wrapper and mec... | 1,631 | 28 | 9,056,674 |
| 2026-08-05 | `6e2e66f53` | pipes: cell dividers as wall metal, and a less squat helium run | 110 | 4 | 6,854,328 |
| 2026-08-05 | `9c03a291a` | pipes: helium run to 12 m, canvas scrolls so length stays tru... | 28 | 6 | 3,771,955 |
| 2026-08-05 | `f6e009122` | offbeat: Zircaloy Poisson validation case (op-6sl.7) -- block... | 3,186 | 30 | 6,861,279 |
| 2026-08-05 | `d1ab6696b` | CHECKPOINT: VISCOCHAB tests and README, agents still in flight | 101 | 59 | 6,959,251 |
| 2026-08-05 | `18712612b` | CHECKPOINT: merge develop; viscochab.rs still under edit | 1 | 2 | 6,229,603 |
| 2026-08-05 | `8573dcb01` | offbeat/foam: VISCOCHAB, ODE enum dispatch, rheology wiring, ... | 3,681 | 1,961 | 14,965,941 |
| 2026-08-05 | `bb52a8147` | offbeat: fracture is not blocked on finite elements -- correc... | 125 | 51 | 2,308,589 |
| 2026-08-05 | `456ad77fb` | offbeat: refuse the Zircaloy Poisson crossover (op-6sl.7); fi... | 181 | 11 | 8,835,761 |
| 2026-08-05 | `3b88deb07` | bedok: stage-1 Rust translation of Than Yan Ren's coupled nod... | 25,924 | 0 | 71,600,043 |
| 2026-08-05 | `a96e98cfc` | tampines: 1-D drift-flux solver (op-dt3.12); six-equation lef... | 3,827 | 5 | 30,484,127 |
| 2026-08-05 | `d106ec667` | bedok: add the crate README, with the defect register front a... | 134 | 0 | 2,076,020 |
| 2026-08-05 | `c55917b6e` | CHECKPOINT: melt_foam solver loop in flight, agent still writing | 535 | 0 | 6,232,846 |
| 2026-08-05 | `4f0bf810d` | bedok: record that benchmark sources are cited, not republished | 16 | 0 | 5,926,180 |
| 2026-08-05 | `08029e65a` | CHECKPOINT: melting V&V probe scaffold, agent still writing | 368 | 1 | 3,943,751 |
| 2026-08-05 | `67a3105a0` | multiphase: port OpenFOAM's interfacial heat-transfer closure... | 1,263 | 179 | 9,811,393 |
| 2026-08-05 | `39aaea078` | CHECKPOINT: melting V&V cases filled in (334 -> 971 lines), a... | 72 | 2 | 5,643,822 |
| 2026-08-05 | `04618b1cc` | kovan-tui: interactive PDF ingestion, with mandatory metadata... | 3,004 | 49 | 10,407,067 |
| 2026-08-05 | `769517e5a` | outram-foam-basic-lib: solidification/melting fvOptions (from... | 1,443 | 5 | 234,154,623 |
| 2026-08-05 | `7fa6a3de2` | outram-foam-multiphase: interfacial heat-transfer closures (f... | 500 | 0 | 0 |
| 2026-08-05 | `bec668b66` | outram-foam-appbuilder-lib: melt_foam solver and melting V&V ... | 1,556 | 0 | 0 |
| 2026-08-05 | `a9ae462ad` | tampines: 1-D drift-flux multiphase solver (from claude/outra... | 2,383 | 0 | 0 |
| 2026-08-05 | `43d4246a1` | Cargo.lock: regenerate for the tampines multiphase dependencies | 1 | 0 | 2,285,010 |
| 2026-08-05 | `1199b8fb2` | CHECKPOINT: melt_foam tests + References.md land; merge devel... | 553 | 109 | 3,080,702 |
| 2026-08-05 | `96631482e` | appbuilder: melt_foam solver loop + Stefan/gallium V&V cases | 8 | 6 | 5,381,722 |
| 2026-08-05 | `d686901d8` | outram-foam-basic-lib: document TemperatureTable / Solidifica... | 24 | 17 | 16,225,640 |
| 2026-08-05 | `8f0d914f4` | outram-foam-appbuilder-lib: melt_foam solver loop + Stefan/ga... | 536 | 97 | 85,444 |
| 2026-08-05 | `2e085b229` | outram-foam-basic-lib: fix README table split by a stray blan... | 0 | 1 | 2,737,143 |
| 2026-08-06 | `1d03089ee` | color_maps: vendor Crameri's Scientific colour maps (MIT), fr... | 520 | 0 | 46,053,354 |
| 2026-08-06 | `9c31adc48` | components: grade temperature blue -> white -> red with Crame... | 92 | 30 | 14,950,932 |
| 2026-08-06 | `6773284c3` | components: colour-to-temperature legend in the studio's righ... | 622 | 1 | 17,326,105 |
| 2026-08-06 | `ccad91504` | pipes: hook the arrays up — Pipe::step implemented, PipeCompo... | 2,097 | 104 | 98,238,537 |
| 2026-08-06 | `bfd72cc09` | components: smooth pipe bends, with a live-angle demonstratio... | 1,294 | 7 | 28,326,815 |
| 2026-08-06 | `504b0b095` | pipe bend: signed turn angle spanning -180 to +180 | 202 | 38 | 27,767,512 |
| 2026-08-06 | `ebbde1b69` | outram-mc GPU: make the WGSL surface-distance kernel agree wi... | 546 | 76 | 11,886,857 |
| 2026-08-06 | `8ce09292c` | docs: reactor scoping slate — six reactor types with coupled ... | 1,468 | 0 | 9,692,180 |
| 2026-08-06 | `a6f49cd63` | engine: promote the FHR reactor vessel widget into the shared... | 986 | 936 | 16,772,032 |
| 2026-08-06 | `fcf2e5411` | engine: promote the temperature buttons, delete the duplicate... | 369 | 1,190 | 19,407,929 |
| 2026-08-06 | `511cc9a44` | CLAUDE.md: complete the Members table, dogfood kopi-beans, pi... | 162 | 32 | — |
| 2026-08-06 | `ee9c7b186` | CLAUDE.md: resolved kopitiam issues move to docs/kopitiam-iss... | 55 | 4 | 7,217,855 |
| 2026-08-06 | `d5b0b6388` | engine: reactor-vessel art for all six scoped reactors, and m... | 1,082 | 24 | 36,463,152 |
| 2026-08-06 | `500dac1ae` | turbine: flip the stator lean so nozzle and bucket oppose eac... | 82 | 10 | 11,151,998 |
| 2026-08-06 | `be49edb9a` | engine: the FHR archetype now renders the real fhr_sim_v2 ves... | 61 | 42 | 10,581,826 |
| 2026-08-06 | `a2f541434` | engine: FHR vessel keeps its proportions at any size, plus pe... | 347 | 4 | 17,603,654 |
| 2026-08-06 | `9be87c63b` | engine: HTR-10 vessel artwork from the IAEA cross-section, in... | 2,931 | 43 | 22,612,771 |
| 2026-08-06 | `880a9bd6c` | htr-10 vessel: randomly packed pebble bed, following the cone... | 201 | 37 | 12,021,493 |
| 2026-08-06 | `10c13a642` | pebble artwork: TRISO speckle instead of one smooth hot sphere | 371 | 13 | 25,773,647 |
| 2026-08-06 | `6f5380f4c` | kovan: ingest the MSRE design and operations report (ORNL-TM-... | 29,522 | 0 | 3,718,293 |
| 2026-08-06 | `e25544e22` | reference-data: vendor the NRIC/INL Virtual Test Bed (CC-BY-4... | 562,796 | 0 | 3,830,761 |
| 2026-08-06 | `07a1952f5` | triso: many more kernel dots per pebble | 9 | 9 | 16,449,134 |
| 2026-08-06 | `4f0d1f741` | triso: crank the kernel density up again, plus the RAVEN port... | 1,110 | 8 | 3,337,523 |
| 2026-08-06 | `86f85f83f` | triso: derive the kernel count from an explicit 80% fill target | 42 | 9 | 6,794,297 |
| 2026-08-06 | `452c26b82` | triso: dial the fill target back to 65% | 5 | 5 | 6,859,598 |
| 2026-08-06 | `77fada9a8` | triso: fill target down to 55% | 4 | 4 | 4,136,538 |
| 2026-08-06 | `8f48e587c` | raffles: new crate — Risk Analysis Framework For Learning & E... | 1,480 | 0 | 5,546,737 |
| 2026-08-06 | `c3985f6a8` | docs: what the vendored Virtual Test Bed actually gives us | 269 | 0 | 19,966,715 |
| 2026-08-06 | `f136e9015` | pebble bed: a real DEM-settled packing, baked once | 1,649 | 0 | 12,859,820 |
| 2026-08-06 | `43e4ec0e6` | raffles: samplers and sensitivity analysis, and a real bug fo... | 2,741 | 151 | 10,011,572 |
| 2026-08-06 | `3263ccac9` | raffles: probability distributions (op-vjw.1) | 2,792 | 57 | 13,309,646 |
| 2026-08-06 | `b42df8f1a` | widget studio: DEM-packed beds, three steam generators, three... | 4,992 | 248 | 30,059,643 |
| 2026-08-06 | `9f4ff6d47` | rng: fix init_seed to match OpenMC (op-rbo) | 3,361 | 1,344 | 11,697,975 |
| 2026-08-06 | `131f5a93c` | docs: fence the torus quartic derivation as text, not Rust (o... | 11 | 3 | 14,425,662 |
| 2026-08-06 | `b16067d42` | outram-mc-libs: record the RNG goal — statistics, not particl... | 33 | 0 | 4,275,739 |
| 2026-08-06 | `dc28d9814` | pebble beds: 3-D depth instead of a flat saw-cut | 1,551 | 510 | 56,238,852 |
| 2026-08-06 | `e71f1f97f` | rng: port OpenMC's PCG output permutation (op-jis) | 1,861 | 476 | 15,385,625 |
| 2026-08-06 | `99925ce1f` | raffles: correct two docs the RNG changes made false | 31 | 13 | 10,999,756 |
| 2026-08-06 | `91153bb86` | docs: fix tampines-steam-tables doctests — a stale API, not a... | 56 | 23 | 8,311,676 |
| 2026-08-06 | `097c0c7e3` | hexlattice: record that the op-jis shift is not meaningful | 16 | 0 | 8,355,438 |
| 2026-08-07 | `8c9ddd05b` | docs: migrate issue-tracker instructions from beads-rs to kop... | 373 | 235 | 1,037,154 |
| 2026-08-07 | `1dfcd0191` | outram-foam-basic-lib: FV/primitive layer work, and a documen... | 4,172 | 522 | 47,249,266 |
| 2026-08-07 | `ee4ac849f` | outram-foam-appbuilder-lib: solver-application work, and a re... | 4,244 | 296 | 0 |
| 2026-08-07 | `1f7f815af` | outram-blender: mesh-authoring work, and a provenance contrad... | 1,980 | 117 | 0 |
| 2026-08-07 | `9bab7ad70` | outram-park-fork-cfmesh: meshing work, temp-dir handling, and... | 2,863 | 172 | 0 |
| 2026-08-07 | `3e9c3a537` | outram-foam-mesh: mesh generation/conversion work, and a docu... | 3,941 | 260 | 0 |
| 2026-08-07 | `fa4cbd07d` | docs: MELCOR-parity and real-time multi-fidelity scoping | 2,388 | 0 | 531,351 |
| 2026-08-07 | `07aa498aa` | docs: complete the migration to kopi-beans; CLAUDE.md said th... | 317 | 111 | 0 |
| 2026-08-11 | `734a53075` | HTR-10 foundations: 23-group decay heat, KTA/ZBS tests, kovan... | 8,372 | 345 | 95,389,999 |
| 2026-08-11 | `89100a934` | dwsim-libs: port the flowsheet, dynamics, column, petroleum a... | 58,347 | 7 | 51,701,732 |
| 2026-08-11 | `5a94616df` | kovan-literature: graph digitiser (engine + CLI, TUI and egui... | 4,297 | 0 | 14,929,678 |
| 2026-08-11 | `a6b195004` | CLAUDE.md: mandate dogfooding the kovan graph digitiser | 40 | 0 | 5,523,494 |
| 2026-08-11 | `442814867` | kovan-literature: librarian pass over the HTR-10 archive | 498 | 2,452 | 49,931,356 |
| 2026-08-11 | `30d72d390` | kovan-literature: visibility is closed by default (op-nv6g) | 90 | 11 | 14,496,734 |
| 2026-08-11 | `e87f4f918` | dwsim-libs: port ShortcutColumn (Fenske-Underwood-Gilliland) | 1,847 | 0 | 3,205,638 |
| 2026-08-11 | `0857f5ea5` | CLAUDE.md: any literature ingested OR USED goes into kovan | 31 | 1 | 11,700,922 |
| 2026-08-11 | `c5422e8d7` | CLAUDE.md: TUAS natural-circulation tests must run in parallel | 33 | 0 | 4,632,718 |
| 2026-08-11 | `a9f71f01e` | chem-eng: relicense to GPL-3.0, O(1) recurrences, and z-domai... | 8,695 | 2,705 | 10,082,614 |
| 2026-08-11 | `4feb6c847` | HTR-10 neutronics spec, (p,h) taper guard, and O(1) transfer ... | 7,136 | 254 | 15,077,007 |
| 2026-08-11 | `4cdb09fe8` | chem-eng: make the step-cost regression tests assert a ratio,... | 89 | 57 | 34,160,302 |
| 2026-08-11 | `d6edfd9f9` | tampines: correct the false Marviken claims and add a bounds-... | 6 | 0 | 7,297,727 |
| 2026-08-11 | `771cb2256` | kovan-literature: catalogue NUREG/CR-2671, the Marviken criti... | 10,131 | 0 | 42,684,750 |
| 2026-08-11 | `c86506e1b` | tampines: pebble-bed conduction stack, helium gas layer, and ... | 9,505 | 59 | 5,490,374 |
| 2026-08-11 | `e11989a71` | tampines-steam-tables: fix the IF97 router panic and gate Mar... | 6,248 | 664 | 25,632,699 |
| 2026-08-11 | `b37e79a39` | tampines-steam-tables: make the (p,s) validity check accept t... | 195 | 53 | 41,464,956 |
| 2026-08-11 | `c615dcd66` | kovan-literature: add the missing nrc1982marviken CATALOGUE.m... | 24 | 0 | 353,340 |
| 2026-08-11 | `e0f0568ad` | tuas: fix the transposed Reynolds/Prandtl exponents in the Wa... | 330 | 25 | 118,291 |
| 2026-08-11 | `085ceea8b` | tampines: drift-flux plenum inflow BC, Marviken V&V, and six-... | 2,916 | 35 | 24,921,232 |
| 2026-08-11 | `67ebcd6ca` | outram-mc: give bound nuclides their thermal-elastic channel ... | 1,187 | 48 | 7,339,144 |
| 2026-08-11 | `6ab77bb6c` | tampines: document how OpenFOAM regularises the six-equation ... | 1,111 | 0 | 89,380,435 |
| 2026-08-11 | `b1bc98808` | outram-blender: HTR-10 pebble-bed core envelope generator + t... | 461 | 0 | 643,777,063 |
| 2026-08-11 | `e43c4c255` | docs(agents): mark the kopi-beans store-format blocker resolv... | 25 | 11 | 42,821,454 |
| 2026-08-12 | `31471e060` | tampines: interfacial exchange closures for the six-equation ... | 1,914 | 0 | 13,010,751 |
| 2026-08-12 | `d9e3d0316` | feat(outram-mc): TRISO nested-shell geometry + Jodrey-Tory hi... | 907 | 0 | 30,999,088 |
| 2026-08-12 | `dfc9dcb06` | outram-foam-basic-lib: declare rayon and target-gated wgpu | 15 | 0 | 6,698,480 |
| 2026-08-12 | `2b2c7c335` | fix(njoy): correct carbon MAT numbers + register C-nat for gr... | 48 | 8 | 12,190,260 |
| 2026-08-12 | `fbad15fd4` | outram-foam-basic-lib: ComputeBackend dispatch layer (op-yvj.... | 3,800 | 14 | 16,966,568 |
| 2026-08-12 | `71c2d755d` | outram-foam-basic-lib: untrack kernels committed prematurely ... | 2 | 3,149 | 3,131,077 |
| 2026-08-12 | `8141f91d5` | feat(njoy): thermal-scattering-law (tsl) acquire/cache path f... | 155 | 4 | 26,166,423 |
| 2026-08-12 | `75de41739` | Cargo.lock: pick up rayon and target-gated wgpu from the foam... | 2 | 0 | 371,274,370 |
| 2026-08-12 | `468277114` | outram-foam-basic-lib: checkpoint the rayon LDU and field ker... | 4,636 | 0 | 10,808,157 |
| 2026-08-12 | `1ef08b8ad` | outram-foam-basic-lib: checkpoint agent test refinements (op-... | 10 | 8 | 3,751,036 |
| 2026-08-12 | `4a22e652c` | outram-foam-basic-lib: record the measured field-kernel cross... | 35 | 5 | 3,889,191 |
| 2026-08-12 | `121ec1124` | outram-foam-basic-lib: bound operator-derived field names (na... | 326 | 12 | 14,220,764 |
| 2026-08-12 | `c60103760` | outram-foam-basic-lib: checkpoint LDU parallel test edits (op... | 2 | 2 | 1,994,540 |
| 2026-08-12 | `e40d6a576` | dwsim: transient (dynamic) rigorous distillation column + V&V | 860 | 0 | 74,990,843 |
| 2026-08-12 | `57378f174` | outram-foam-basic-lib: checkpoint LDU SpMV work in progress (... | 188 | 94 | 3,036,925 |
| 2026-08-12 | `403bd7075` | outram-foam-basic-lib: hybrid LDU SpMV and Krylov vecops (op-... | 68 | 25 | 4,864,701 |
| 2026-08-12 | `f5040fc1b` | docs: make 19 unpublished beads durable, and file the kopi-be... | 1,152 | 0 | 17,811,843 |
| 2026-08-12 | `9da38d5df` | outram-foam-basic-lib: batched root finding on the hybrid bac... | 3,125 | 2 | 5,850,470 |
| 2026-08-12 | `846636932` | gitignore: aider's virtualenv | 1 | 0 | 58,046,223 |
| 2026-08-12 | `0b132ea21` | tampines: implement the 1-D six-equation two-fluid solver (op... | 4,435 | 104 | 8,878,416 |
| 2026-08-12 | `9c0a3b629` | tampines: correct the multiphase_1d module docs now the six-e... | 15 | 5 | 2,910,845 |
| 2026-08-12 | `878c73eaa` | opcua: extract the reactor-agnostic layer out of ciet_opcua (... | 3,229 | 1,993 | 156,958,507 |
| 2026-08-12 | `f7c8a1902` | htgr_sim_v1: real HTR-10 pebble-bed core, and a schematic bui... | 2,278 | 611 | 1,951,840 |
| 2026-08-12 | `2a6a43c73` | fhr_sim_v2: make a steam-generator temperature cross impossib... | 2,639 | 86 | 978,188 |
| 2026-08-12 | `43f811044` | htgr: OPC-UA node map, HTR-10 plant data, and two more source... | 4,103 | 1 | 1,960,899 |
| 2026-08-12 | `df8ad9a8a` | rustfmt.toml: pin workspace formatting | 38 | 0 | 3,934,222 |
| 2026-08-12 | `dd00f6804` | style: apply rustfmt workspace-wide (formatting only, no beha... | 214,067 | 142,186 | 8,903,673 |
| 2026-08-12 | `3cbbb5a88` | blame: ignore the workspace rustfmt pass | 12 | 0 | 991,612 |
| 2026-08-12 | `0d272cf16` | htgr_sim_v1: wire the real htr10 KTA/ZBS module into the plan... | 868 | 238 | 31,734,367 |
| 2026-08-12 | `cf975861e` | fhr_sim_v2: measure GUI frame time, so lag reports can be att... | 316 | 0 | 696,152 |
| 2026-08-12 | `079bd1d47` | docs: record kopitiam#19 as resolved -- bn v0.1.3 does publis... | 115 | 0 | 0 |
| 2026-08-12 | `bf2c851a3` | htgr_sim_v1: command control rods, not reactivity | 401 | 14 | 24,589,508 |
| 2026-08-12 | `3c9d547a9` | docs: refresh the kopi-beans notes against v0.1.3 | 111 | 49 | 0 |
| 2026-08-12 | `1e0251356` | htgr_sim_v1: make a steam-side temperature cross unrepresentable | 209 | 10 | 20,022,517 |
| 2026-08-12 | `71d07632e` | htgr_sim_v1: automatic scram on measurable trip signals | 487 | 8 | 29,694,384 |
| 2026-08-12 | `0bc2a948f` | app_scaffold: name the failing PLANT COMPONENT in crash reports | 493 | 8 | 1,597,888 |
| 2026-08-12 | `e144e0d92` | scripts: make the beads-store push work from any working dire... | 16 | 2 | 9,779,205 |
| 2026-08-12 | `91853c3f0` | htgr_sim_v1: make the protection system a toggle, disarmed by... | 77 | 2 | 5,850,719 |
| 2026-08-12 | `541a38ef3` | components: bake the pebble bed to a texture, and animate the... | 7,444 | 80 | 3,838,214 |
| 2026-08-12 | `4c36c029b` | components: real artwork for the condenser, cooling tower and... | 78 | 36 | 5,705,402 |
| 2026-08-12 | `3543645b9` | widget_studio: tabs for the condenser, cooling tower and excu... | 2,462 | 0 | 23,929,049 |
| 2026-08-12 | `5375e2724` | docs: htr10.md described the opposite of what the simulator n... | 36 | 7 | 24,055,398 |
| 2026-08-12 | `2e3185dd2` | components: an HTGR excursion is not an explosion — rework th... | 1,863 | 1,330 | 2,666,404 |
| 2026-08-12 | `162a08fac` | coolprop: fix two boundary-condition defects in OPCPFluidArray | 604 | 11 | 3,248,926 |
| 2026-08-12 | `c0d631797` | docs: record the human corrections to AI work, and the BC con... | 380 | 0 | 27,251,795 |
| 2026-08-12 | `d9abc5561` | tampines-steam-tables: the energy equation's inlet BC was era... | 647 | 2 | 2,972,773 |
| 2026-08-12 | `2cc65f9a2` | docs: re-verify the whole HTR-10 scoping audit against curren... | 279 | 84 | 29,015,574 |
| 2026-08-12 | `bf023a61a` | components: real artwork for the heat exchanger, plus a studi... | 3,469 | 22 | 1,283,009 |
| 2026-08-12 | `00a776be1` | htgr_sim_v1: the turbine rotor now turns, from a real torque ... | 1,056 | 20 | 2,582,852 |
| 2026-08-12 | `621061583` | coolprop: bounded scalar convection, upwind boundaries, and a... | 825 | 34 | 7,177,635 |
| 2026-08-12 | `6c7cee7fc` | docs: narrow the sibling-document claim from "audited" to "sp... | 32 | 10 | 4,637,090 |
| 2026-08-12 | `b53131564` | tampines-steam-tables: TUAS upwind terminals, and the conduct... | 1,177 | 32 | 0 |
| 2026-08-12 | `9f36deb3a` | CLAUDE.md: hard rule — search the workspace before building a... | 60 | 0 | 16,198,039 |
| 2026-08-12 | `05b20cbd1` | fhr_sim_v2: measured the GUI, found nothing to fix, kept the ... | 488 | 264 | 18,836,035 |
| 2026-08-12 | `e7e44b28b` | docs: GUI stutter on the workstation is the graphics stack, n... | 73 | 0 | 15,884,436 |
| 2026-08-12 | `10216f03a` | docs: record the measured GPU headroom, with its caveats | 20 | 4 | 4,404,582 |
| 2026-08-12 | `edbb2bd72` | htgr_sim_v1: nodalised counter-flow steam generator replaces ... | 2,845 | 170 | 5,165,784 |
| 2026-08-12 | `1d4ac7473` | changed numbers and some of the hot and cold side | 14 | 14 | 2,979,170 |
| 2026-08-13 | `d55997e21` | outram-foam-basic-lib: snapshot in-flight golden-section work... | 1,387 | 0 | 19,709,337 |
| 2026-08-13 | `8a8322478` | outram-foam-basic-lib: snapshot in-flight ODE and minimisatio... | 5,315 | 41 | 5,983,182 |
| 2026-08-13 | `fb59c7ef7` | kovan: ingest ANL-75-55 and Pichler 2020 stainless-steel prop... | 2,109 | 0 | 131,404,259 |
| 2026-08-13 | `f565d31b0` | tuas: add SolidMaterial::SteelSS304LHighTemp (Kim ANL-75-55, ... | 1,055 | 1 | 442,112 |
| 2026-08-13 | `1a3822301` | outram-foam-basic-lib: snapshot golden-section test additions... | 89 | 15 | 3,453,944 |
| 2026-08-13 | `f69393955` | tuas: give the temperature-range error a payload, and stop mi... | 353 | 90 | 443,636 |
| 2026-08-13 | `3acc95362` | htgr_sim_v1: reach real time (0.492 -> 1.032), and fix the ho... | 3,163 | 321 | 445,294 |
| 2026-08-13 | `4320d0568` | workspace: clear all 97 float_literal_f32_fallback warnings (... | 103 | 104 | 896,609 |
| 2026-08-13 | `d54ffe72b` | outram-foam-basic-lib: batched golden-section minimisation (o... | 16 | 0 | 12,375,581 |
| 2026-08-13 | `8e97330da` | coolprop: add a selectable implicit energy-balance mode to OP... | 756 | 7 | 10,079,720 |
| 2026-08-13 | `ee15d32ac` | outram-foam-basic-lib: checkpoint ODE ensembles, doctest now ... | 104 | 3 | 3,202,375 |
| 2026-08-13 | `68e35551c` | htgr_sim_v1: put the steam generator's helium side on implici... | 26 | 0 | 8,059,634 |
| 2026-08-13 | `981e2ccf2` | outram-foam-basic-lib: checkpoint ODE ensemble rewrite (op-yv... | 247 | 130 | 5,576,906 |
| 2026-08-13 | `515777665` | outram-foam-basic-lib: batched ODE ensembles and quadrature (... | 104 | 60 | 4,293,471 |
| 2026-08-13 | `0ba10cbe3` | outram-foam-basic-lib: snapshot in-flight numerical Jacobians... | 1,997 | 0 | 9,893,912 |
| 2026-08-13 | `c0e85a503` | htgr_sim_v1: cut the steam-generator sub-steps 8 -> 2, reachi... | 285 | 36 | 25,575,685 |
| 2026-08-13 | `e49784bf5` | outram-foam-basic-lib: checkpoint Jacobians and the hybrid pa... | 3,748 | 15 | 7,107,909 |
| 2026-08-13 | `f45a7d2a3` | outram-foam-basic-lib: cross-cutting hybrid parity gate (op-y... | 1 | 0 | 5,685,700 |
| 2026-08-13 | `45f79ac20` | outram-foam-basic-lib: checkpoint Jacobians, failure-detectio... | 646 | 16 | 3,138,186 |
| 2026-08-13 | `ffebc1003` | outram-foam-basic-lib: checkpoint Jacobian test additions (op... | 118 | 56 | 4,752,711 |
| 2026-08-13 | `5e6b25d3a` | outram-foam-basic-lib: finite differences and batched Jacobia... | 89 | 0 | 9,161,739 |
| 2026-08-13 | `b3cc5b66f` | outram-foam: fix a solver that reported success with a NaN st... | 105 | 19 | 19,365,173 |
| 2026-08-13 | `fded5ea95` | docs: retire the beads-recovery copy -- all 19 are now published | 0 | 1,039 | 6,780,032 |
| 2026-08-13 | `3288145f9` | htgr_sim_v1: add selectable temperature-cross remedies (None ... | 5,940 | 0 | 21,455,774 |
| 2026-08-13 | `762a10940` | outram-foam-basic-lib: checkpoint Krylov-on-HybridLdu wiring ... | 1,999 | 79 | 14,434,163 |
| 2026-08-13 | `dbe4c6db9` | htgr_sim_v1: drop the eliminate-metal remedy, prefer Yan Ren'... | 108 | 1,385 | 9,353,446 |
| 2026-08-13 | `c5e598395` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 130 | 13 | 7,065,216 |
| 2026-08-13 | `f12599484` | outram-foam-basic-lib: salvage the Krylov agent's last edits ... | 24 | 26 | 5,310,052 |
| 2026-08-13 | `b500c1a0b` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 107 | 38 | 3,571,022 |
| 2026-08-13 | `17872fe4e` | outram-foam-basic-lib: checkpoint Krylov wiring (op-yvj.4.4) | 163 | 58 | 7,833,462 |
| 2026-08-13 | `c60130780` | htgr_sim_v1: secondary-loop operator controls, degC/K toggle,... | 2,427 | 155 | 32,028,698 |
| 2026-08-13 | `60fb179ef` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 482 | 106 | 12,831,072 |
| 2026-08-13 | `65345929a` | outram-foam-basic-lib: checkpoint GMRES hybrid wiring (op-yvj... | 11 | 2 | 2,474,822 |
| 2026-08-13 | `947a904db` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 40 | 10 | 4,353,459 |
| 2026-08-13 | `790f68637` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 67 | 17 | 2,497,606 |
| 2026-08-13 | `f76ed92e4` | outram-foam-basic-lib: checkpoint BiCGStab hybrid wiring (op-... | 17 | 3 | 3,137,608 |
| 2026-08-13 | `d44d592b1` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 47 | 24 | 2,519,530 |
| 2026-08-13 | `bd373cc68` | outram-foam-basic-lib: checkpoint preconditioner hybrid wirin... | 13 | 8 | 3,166,210 |
| 2026-08-13 | `d688f7d7f` | outram-foam-basic-lib: Krylov solvers on the hybrid backend (... | 11 | 3 | 5,752,956 |
| 2026-08-13 | `2d68f5b5c` | checkpoint: op-zwk0 NaN-defect audit of the two duplicated OD... | 226 | 2 | 18,296,809 |
| 2026-08-13 | `c1e0fd35a` | checkpoint: op-zwk0 guard in coolprop's duplicated Rosenbrock... | 10 | 0 | 5,985,016 |
| 2026-08-13 | `ce4dd30d8` | checkpoint: op-zwk0 NaN guards in both duplicated ODE trees | 354 | 36 | 4,012,774 |
| 2026-08-13 | `9d2f1dbb0` | checkpoint: op-zwk0 NaN guards, both trees (+40/-8) | 40 | 8 | 4,718,730 |
| 2026-08-13 | `72fe7518f` | docs: record kopi-beans losing beads AFTER acknowledging the ... | 69 | 0 | 19,245,199 |
| 2026-08-13 | `6aa58b2ee` | checkpoint: op-uyi3 golden-section de-duplication in steam-ta... | 372 | 96 | 10,592,281 |
| 2026-08-13 | `58a3284a7` | checkpoint: op-uyi3 golden-section de-duplication, Zaloudek g... | 500 | 35 | 7,156,851 |
| 2026-08-13 | `31174d8c0` | steam-tables: de-duplicate golden section; correct an underst... | 28 | 9 | 7,962,361 |
| 2026-08-13 | `704830226` | steam-tables: stop writing to stderr from production flash pa... | 21 | 11 | 13,148,964 |
| 2026-08-13 | `5e1c988fc` | policy: make the working-hours guardrail opt-in per session | 87 | 27 | — |
| 2026-08-13 | `dad058130` | docs(outram-foam-basic-lib): settle the hybrid CPU+GPU precis... | 165 | 0 | 23,362,738 |
| 2026-08-13 | `86c46ffb6` | docs(outram-foam-basic-lib): coarse-to-fine supersedes the hy... | 125 | 9 | 2,954,825 |
| 2026-08-13 | `85143f25b` | docs: upstream the three outstanding kopi-beans defects | 6 | 0 | 17,510,691 |
| 2026-08-13 | `b3c5835c3` | kovan-metrics: port token accounting + historian from Python ... | 2,590 | 24 | 121,773,353 |
| 2026-08-13 | `859f9221f` | docs: record the kopi-beans 0.1.6 daemon fix, verified on thi... | 95 | 6 | 6,622,357 |
| 2026-08-13 | `4ecce27a6` | docs/historian: delete the Python scripts, kovan-metrics is n... | 29 | 754 | 8,073,261 |
| 2026-08-13 | `cd0fef9d3` | kovan-literature: catalogue Terry 2005 + three UC Berkeley th... | 157 | 0 | 31,680,456 |
| 2026-08-13 | `268e44c2c` | cfmesh: add face-pyramid-volume and cell-determinant sliver c... | 521 | 0 | 19,131,890 |
| 2026-08-13 | `89e0d47b9` | kovan-literature: record the HTR-10 r-z zone geometry read fr... | 312 | 6 | 1,667,472 |
| 2026-08-13 | `ac271965c` | kovan-literature: confirm z orientation, start the Fig. 2 zon... | 39 | 3 | 6,773,634 |
| 2026-08-13 | `91dd04d50` | kovan-literature: complete the Fig. 2 bottom row -- the radia... | 35 | 20 | 2,580,140 |
| 2026-08-13 | `cb3647d99` | kovan-literature: Fig. 2 second layer -- the map stops being ... | 51 | 0 | 1,740,390 |
| 2026-08-13 | `9860ae82e` | kovan-literature: confirm the one-band spans of zones 46, 55,... | 7 | 7 | 3,951,994 |
| 2026-08-13 | `97c494081` | docs: reconcile upstream kopitiam issue state, and close #19 | 44 | 9 | 50,865,877 |
| 2026-08-13 | `ff0c4b372` | njoy: LEAPR can regenerate graphite S(alpha,beta) from its 12... | 4,530 | 49 | 19,783,743 |
| 2026-08-13 | `53fa574eb` | njoy: LEAPR regeneration is now the default source of graphit... | 3,585 | 81 | 34,932,070 |
| 2026-08-13 | `239182a22` | docs(data-acquisition): the cache substrate is no longer feat... | 15 | 0 | 5,573,065 |
| 2026-08-13 | `8a8340cd7` | docs: log requested htgr_sim_v1 follow-ups for the next activ... | 63 | 0 | — |
| 2026-08-13 | `47476d130` | outram-foam-mesh: cyclic-aware snapping — periodic seams snap... | 900 | 3 | — |
| 2026-08-13 | `80e6e68f3` | docs(outram-foam-basic-lib): fix 24 broken intra-doc links, d... | 56 | 41 | — |
| 2026-08-13 | `0e5e14eab` | htgr_sim_v1: decay heat, a fuel-feedback heat sink, evaluated... | 1,284 | 149 | — |
| 2026-08-13 | `e97850f32` | docs: drop the htgr_sim_v1 follow-up log, now that the work i... | 0 | 63 | — |
| 2026-08-13 | `6a15c02b5` | htgr_sim_v1: fix the core-outlet temperature inversion, and m... | 618 | 299 | — |
| 2026-08-13 | `b66238075` | coolprop: exact backward T(rho, h), and record where human re... | 304 | 0 | — |
| 2026-08-13 | `322a64041` | coolprop: seed the (rho, h) backward solve from the fixed-pre... | 193 | 13 | — |
| 2026-08-14 | `52a21fbaa` | dealt with degC button and the restart button | 928 | 167 | — |
| 2026-08-14 | `e91125815` | added bacon toml | 8 | 0 | — |
| 2026-08-14 | `d4387b59a` | legend unit is now Celsius, not kelvin | 10 | 5 | — |
| 2026-08-14 | `fdc047185` | htgr_sim_v1: the corrector loop is not converged, and say so ... | 111 | 7 | 64,241,387 |
| 2026-08-14 | `b0fafebef` | kovan agent-docs-gen: bundle the API docs for an external 200... | 1,631 | 0 | 28,107,942 |
| 2026-08-14 | `20e118b4b` | agent-docs-gen: --regenerate-missing never worked; the tools ... | 1,926 | 29 | 11,922,811 |
| 2026-08-14 | `070d3f317` | Retire scripts/gen_api_docs.py: port it to `kovan api-docs` | 327 | 181 | 9,562,759 |
| 2026-08-14 | `b9c72f50a` | No Python for docs or accounting (hard rule); retire the aste... | 78 | 472 | 11,101,988 |
| 2026-08-14 | `e6cee0ba5` | kloc parity baseline: freeze the Python output before porting it | 321 | 0 | 3,080,340 |
| 2026-08-14 | `364b8b00f` | kloc parity baseline: recapture with every repository present | 94 | 31 | 14,795,866 |
| 2026-08-14 | `7f342453e` | changed loop to manual | 9 | 2 | 1,920,766 |
| 2026-08-14 | `bfb645fe3` | added manual for secondary loop | 1 | 1 | 3,437,244 |
| 2026-08-14 | `5b02a9d4f` | added feedwater manual control and edited slider | 4 | 3 | 9,297,662 |
| 2026-08-14 | `68e2e11f3` | Retire scripts/kloc_accounting.py: port it to `kovan kloc` | 0 | 1,454 | 33,752,515 |
| 2026-08-14 | `8d996dcaf` | added kovan metrics | 3,892 | 2 | 5,563,359 |
| 2026-08-14 | `673399fac` | kovan: regenerate the whole doc suite in one command, and fin... | 78,580 | 17,850 | 25,407,865 |
| 2026-08-14 | `7a9d2b56e` | added new api docs for each | 142,212 | 0 | 4,171,800 |
| 2026-08-14 | `db242bb2f` | added new api | 149,800 | 0 | 0 |
| 2026-08-14 | `dd2682d98` | added anders pdf | 9 | 0 | 0 |
| 2026-08-14 | `b983f8387` | README: document the `kovan lit` ingestion workflow and the t... | 54 | 1 | 13,486,507 |
| 2026-08-14 | `b297fea9b` | kovan-literature: record the choo2023criticality tier decisio... | 13 | 2 | 5,209,786 |
| 2026-08-14 | `ffef0fb76` | added 4 endf files specific to triso | 439,743 | 0 | 1,309,908 |
| 2026-08-14 | `f90b23506` | changed pathbuf for njoy, pending test | 10 | 3 | 0 |
| 2026-08-14 | `1ace0acd4` | added leapr files from endf, from here: https://www.nndc.bnl.... | 10,584 | 0 | — |
| 2026-08-14 | `e4afd5177` | njoy-outram-park-fork: embed all 33 ENDF/B-VIII.0 LEAPR decks... | 894 | 132 | 0 |
| 2026-08-14 | `933ee9ca7` | outram-mc: route thermal scattering into both k-eigenvalue pa... | 462 | 106 | — |
| 2026-08-14 | `2becc4c97` | outram-mc: TRISO shell S(alpha,beta) composition, and the SiC... | 447 | 18 | — |
| 2026-08-14 | `9591944fb` | njoy+outram-mc: light-water thermal scattering, deck to trans... | 1,056 | 43 | 753,784,864 |
| 2026-08-14 | `a2802a350` | pincell: the thermal LWR benchmark is no longer data-gated | 18 | 17 | 84,498,218 |
| 2026-08-15 | `5673aaa25` | Patch-bump the njoy→digital-twin-engine publish pipeline (16 ... | 1,396 | 48 | 16,094,533 |
| 2026-08-15 | `df6f62f1b` | scripts: update cargo-publish.sh to the real njoy->digital-tw... | 38 | 11 | 4,989,786 |
| 2026-08-15 | `e556dcaca` | docs(historian): add develop-vs-main accounting report (998 c... | 1,062 | 0 | 6,075,285 |
| 2026-08-15 | `796c6af11` | docs(claude.md): trim duplicated kopitiam/kopi-beans history,... | 57 | 176 | 12,784,522 |
| 2026-08-15 | `9229031d6` | dwsim-libs: promote benzene()/toluene() to public Component c... | 65 | 43 | 52,367,515 |
| 2026-08-15 | `a621a4fcd` | digital-twin-engine: add distillation_sim_v1, a transient dis... | 1,465 | 0 | 88,264,483 |
| 2026-08-16 | `d214fa107` | htgr_sim_v1: add selectable pebble-bed reactor-model fidelity... | 592 | 12 | 72,021,385 |
| 2026-08-17 | `419ee643d` | htgr_sim_v1: wire outram-foam-appbuilder-lib (genfoam) in as ... | 50 | 17 | 8,980,750 |
| 2026-08-17 | `ed6af7360` | added CinSiC endf file | 95,305 | 0 | — |
| 2026-08-17 | `cdbea87c9` | added si in sic endf | 95,355 | 0 | — |
| 2026-08-17 | `9281c422c` | njoy: move ENDF tapes out of crates/ so cargo package cannot ... | 443 | 97 | 39,831,920 |
| 2026-08-17 | `605d7a738` | njoy: keep the two SiC tapes out of src/leapr/decks/, and rec... | 32 | 95,306 | 6,429,131 |
| 2026-08-17 | `e407ae124` | kovan api-docs: rename the per-crate mirror to <crate>-api.md... | 1,496 | 56,644 | 44,691,925 |
| 2026-08-17 | `fbfa6fc3c` | re-dimensioned one node (human manual commit) | 21 | 29 | — |
| 2026-08-17 | `8eb3f7281` | added heat exchanger NTU theory | 129 | 0 | — |
| 2026-08-17 | `104a8ff57` | htgr_sim_v1: fix three test call sites broken by the one_node... | 18 | 4 | 35,274,176 |
| 2026-08-17 | `1d3148108` | corrected unit bug in outram park digital twin engine for htg... | 35 | 14 | — |
| 2026-08-17 | `1abb02a49` | added one node fluid | 4 | 18 | — |
| 2026-08-17 | `44a2daca5` | htgr_sim_v1: reapply the NTU dimensional fix, reverted by the... | 18 | 4 | 28,506,899 |
| 2026-08-17 | `6f163f9c8` | redone claude md | 123 | 78 | 6,520,781 |
| 2026-08-17 | `dfea946eb` | bedok: rewrite the MATLAB port as one flat module per .m file | 14,486 | 25,422 | — |
| 2026-08-18 | `6328543fe` | bedok: complete the file-by-file MATLAB translation (all 50 f... | 23,289 | 17,117 | — |
| 2026-08-21 | `832898c50` | bedok: verify the port against the running MATLAB, and fix de... | 3,170 | 49 | — |
| 2026-08-23 | `ff4bdb022` | bedok: open the correction stage — every High-severity defect... | 3,470 | 320 | — |
| 2026-08-23 | `68b5e4d47` | bedok: close the defect register — 79 of 79 entries addressed | 2,760 | 130 | — |
| 2026-09-05 | `cbb29b65c` | njoy: resonance/Doppler tutorial, corrected physics, prelude ... | 5,964,225 | 0 | — |
| 2026-09-05 | `83b2b09aa` | tampines: Rankine cycle tutorial, and a prelude that exports ... | 418 | 0 | — |
| 2026-09-05 | `670bab9e7` | outram-foam-basic-lib: FV verification tutorial, and how to g... | 254 | 0 | — |
| 2026-09-05 | `fdb89e519` | outram-foam-appbuilder: solver tutorial, and stop advertising... | 501 | 1 | — |
| 2026-09-05 | `7ecbcacbd` | Bump six declared crates by one patch version for release | 286 | 18 | — |
| 2026-09-05 | `ada31a985` | teh-o-prke: decay heat tutorial, and short aliases for the st... | 60 | 1 | — |
| 2026-09-06 | `979ce0e52` | docs: HTGR Sim v1 execution architecture (native + browser pa... | 346 | 0 | — |
| 2026-09-06 | `c4ed47594` | docs: sequencing decided — architecture before the HTR-10 phy... | 45 | 11 | — |
| 2026-09-06 | `a1eca4b74` | docs: the worker pool is unconditional — the runtime is the d... | 51 | 3 | — |
| 2026-09-06 | `8799682ed` | Declare outram-park-fork-liggghts mature, with the physics ca... | 71 | 1 | — |
| 2026-09-06 | `2f2494c87` | docs: three execution classes — the kernel contract does not ... | 64 | 0 | — |
| 2026-09-06 | `fb70a13cb` | HARD RULE: every egui simulator ships a headless mode, and ht... | 253 | 0 | — |
| 2026-09-06 | `fdf825767` | docs+test: dataflow audit and the pre-refactor reference base... | 325 | 2 | — |
| 2026-09-06 | `7b26c9a19` | Extract the kernel boundary: PlantRuntime, controls in, bound... | 343 | 7 | — |
| 2026-09-07 | `6f2137991` | engine: headless harness and ASCII schematic rendering in the... | 705 | 31 | — |
| 2026-09-08 | `e9a01f841` | kovan: user relations, artifact overlays and a headless mindm... | 3,903 | 64 | — |
| 2026-09-08 | `8e3fc21eb` | kovan update: 3 and 6 | 397 | 54 | — |
| 2026-09-08 | `900a30deb` | attempted kovan dogfooding | 561 | 34 | — |
| 2026-09-08 | `ef9cce9af` | fixing the csv boxes | 150 | 5 | — |
| 2026-09-08 | `63945f607` | kovan: one artifact schema for every block, relations in the ... | 1,007 | 279 | — |
| 2026-09-08 | `b75f5949b` | kovan PDF reader: go-to-page and go-to-digitiser on every art... | 146 | 37 | — |
| 2026-09-08 | `7f9dd3aba` | kovan PDF reader: a second right-click dismisses the dropdown... | 63 | 17 | — |
| 2026-09-08 | `5692738be` | kovan: drop the Copy CSV button from the PDF reader's artifac... | 34 | 11 | — |
| 2026-09-08 | `0979d8256` | kovan PDF reader: a left-click outside dismisses the dropdown... | 19 | 2 | — |
| 2026-09-08 | `7b47643f6` | kovan: reinstate Copy CSV in the PDF reader, drop the CopyBut... | 20 | 27 | — |
| 2026-09-09 | `f4664e676` | docs: scope Adolphus Lye's seven UQ repos into raffles (GH #1... | 133 | 0 | — |
| 2026-09-09 | `ecdea731c` | changing the look for htgr_sim_v1 | 58 | 8 | — |
| 2026-09-09 | `bd58efdc1` | htgr_sim_v1: correct HTR-10 primary-loop and SG schematic (GH... | 1,003 | 402 | — |
| 2026-09-09 | `ba7eca5e3` | tampines-steam-tables: mean-flow stage-by-stage turbine model | 1,324 | 0 | — |
| 2026-09-09 | `a5c09db52` | tampines-steam-tables: v0.2.9, document the mean-flow stage m... | 56 | 2 | — |
| 2026-09-09 | `c5180a1ce` | tampines-steam-tables: transient mean-flow turbine on the 1-D... | 646 | 1 | — |
| 2026-09-09 | `ecc1357bc` | Cargo.lock: pick up tampines-steam-tables 0.2.9 | 1 | 1 | — |
| 2026-09-09 | `740c82dc7` | tampines-steam-tables: close the shaft loop through the gener... | 363 | 30 | — |
| 2026-09-10 | `0f3b22c0a` | outram-mc-libs: mandate committing the OpenMC input scripts f... | 50 | 0 | — |
| 2026-09-10 | `ba869d4af` | outram-mc-libs: explicit per-energy leakage accounting in the... | 389 | 18 | — |
| 2026-09-10 | `ec940f56f` | outram-mc-libs: physics::reactor_physics -- six-factor + leth... | 1,044 | 1 | — |
| 2026-09-10 | `4a67ed267` | outram-mc-libs: real Sigma_a on MacroXs -- ScoreType::Absorpt... | 48 | 37 | — |
| 2026-09-10 | `6fa2a97d2` | reference-data: ENDF/B-VIII.0 neutron tapes for the FHR TRISO... | 782,923 | 3 | — |
| 2026-09-10 | `b68ce6143` | outram-mc-libs: FHR ring-RPT pebble builders + code-to-code e... | 3,117 | 1 | — |
| 2026-09-10 | `ee06b5dd4` | outram-mc-libs: nudge across a surface along its normal, not ... | 62 | 5 | — |
| 2026-09-10 | `6d1a7a502` | outram-mc-libs: absorption is MT=27 (capture + charged-partic... | 63 | 2 | 717,192,170 |
| 2026-09-10 | `954b0ef96` | outram-mc-libs: carry a signed surface token through crossing... | 938 | 167 | 2,611,709 |
| 2026-09-10 | `4dbe7164b` | outram-mc-libs: wire the full FHR pebble (graphite S(a,b) + r... | 225 | 161 | 57,532,000 |
| 2026-09-10 | `23cd25497` | outram-mc: match deck free-gas SiC carbon; ring-RPT vs OpenMC... | 74 | 42 | 20,095,318 |
| 2026-09-10 | `ef48184e0` | pflotran: verify upstream licence is LGPL-3.0, and vendor the... | 108 | 19 | — |
| 2026-09-10 | `ae92a3f2b` | pflotran: correct the NOTICE to the verified LGPL-3.0 finding | 191 | 25 | — |
| 2026-09-10 | `526d0f7dc` | outram-mc: regenerate API mirror (adds reactor_physics + fhr_... | 3,372 | 504 | 4,921,980 |
| 2026-09-10 | `e09239ee9` | njoy: record the U-238 URR seam defect + add a read-upstream-... | 155 | 0 | — |
| 2026-09-10 | `5732c661d` | dwsim-libs: expose the three unreachable ColumnType variants ... | 740 | 11 | — |
| 2026-09-10 | `ac21df4eb` | dwsim-libs: finalise ColumnType constructors; RefluxedAbsorbe... | 712 | 217 | — |
| 2026-09-10 | `f7b1ace6f` | dwsim-libs: add a `prelude` module and a worked crude-column ... | 668 | 8 | — |
| 2026-09-10 | `31fa3880a` | njoy: port BROADR thnmax, fix two URR w(z) defects, verify PU... | 1,590 | 65 | — |
| 2026-09-10 | `95cf5c26b` | htgr_sim_v1: compare the headless baseline numerically, not b... | 75 | 7 | — |
| 2026-09-10 | `f40b71f61` | njoy: fix WAVE_K (cwaven rounded up), unresx AMUN fold; verif... | 124 | 39 | — |
| 2026-09-10 | `e9548ceff` | dwsim-libs: regenerate the API-doc mirror | 5,986 | 3,560 | — |
| 2026-09-10 | `543e322e3` | dwsim-libs: add the upstream port-coverage matrix (#78) | 312 | 0 | — |
| 2026-09-10 | `4024e53d8` | dwsim-libs: refuse non-physical pseudo-components instead of ... | 377 | 86 | — |
| 2026-09-10 | `582beeced` | njoy: ERRORR covout writes sub-eps interior elements verbatim... | 6,828,543 | 0 | — |
| 2026-09-10 | `3062ad78f` | njoy: COVR library option end to end, byte-identical to NJOY ... | 13,356 | 310 | — |
| 2026-09-10 | `fe9818f2e` | njoy: LEAPR mixed-moderator (b7<=0) merge, verified vs NJOY20... | 35,179 | 130 | — |
| 2026-09-10 | `5421e92d9` | njoy: RECONR resonance-grid convergence ported from resxs; Si... | 16,954 | 103 | — |
| 2026-09-10 | `6513de628` | njoy: BROADR erfc to double precision; H-2 capture no longer ... | 100 | 18 | — |
| 2026-09-10 | `e8be67868` | njoy: LEAPR skold (Sköld coherence) ported, verified vs NJOY2... | 53,873 | 27 | — |
| 2026-09-10 | `25a88efb3` | njoy: MIXR golden test vs NJOY2016 (H-2 + Be-9 mix identical ... | 990 | 0 | — |
| 2026-09-10 | `20f7b9e09` | njoy: DTFR golden test vs NJOY2016 CLAW n-n table (U-238, 32x... | 945 | 3 | — |
| 2026-09-10 | `5d2232594` | njoy: RESXSR byte-identical to NJOY2016: gfortran record fram... | 238 | 18 | — |
| 2026-09-10 | `aceead0f6` | njoy: DTFR edit accumulation ported; CLAW edit block matches ... | 138 | 7 | — |
| 2026-09-10 | `4d8f791ff` | njoy: RESXSR multi-temperature loop ported, byte-identical to... | 1,992 | 28 | — |
| 2026-09-10 | `06dcec5da` | njoy: GROUPR two-body elastic matrix ported (getfle/getco, ge... | 2,136 | 16 | — |
| 2026-09-10 | `4923e1fd6` | njoy: GROUPR P0-P3 elastic matrix and MT=257/258/259 derived ... | 1,565 | 99 | — |
| 2026-09-10 | `be04ddacb` | njoy: GROUPR genflx heterogeneity / multi-moderator terms por... | 717 | 121 | — |
| 2026-09-10 | `c51ceee9e` | njoy: GROUPR discrete-level inelastic (MT=51/52/60/89) vector... | 958 | 20 | — |
| 2026-09-10 | `8a231bdf8` | njoy: GAMINR gtff (coherent/incoherent/pair feeds), photon gp... | 1,924 | 65 | — |
| 2026-09-10 | `3b5a6181f` | njoy: DTFR full-channel assembly (nu*sigma_f, chi incl. const... | 3,062 | 0 | — |
| 2026-09-10 | `2803d617e` | njoy: GROUPR MF=10 residual-production yield search ported (m... | 498 | 11 | — |
| 2026-09-10 | `ba700a8f6` | njoy: ERRORR MF=32 resprx chain ported (rpxlc2/rpxlc12/rpendf... | 63,072 | 33 | — |
| 2026-09-10 | `ced7a0c7d` | njoy: RECONR evaluates LRF=2 with csmlbw's multilevel elastic... | 8,082 | 11 | — |
| 2026-09-10 | `f7276573c` | njoy: LEAPR run driver (whole deck -> MF=1/451 Hollerith head... | 3,814 | 87 | — |
| 2026-09-10 | `b9d44326b` | njoy: ERRORR MF=32 LRF=7 rpxsamm + SAMM resonance-parameter d... | 151,564 | 235 | — |
| 2026-09-10 | `e71405d28` | reference-data: add maintainer-supplied H-in-H2O tsl and U ph... | 245,905 | 0 | — |
| 2026-09-10 | `cb645045c` | reference-data: replace stale "Still wanted" list with the fi... | 23 | 3 | — |
| 2026-09-10 | `d2f98d68e` | njoy: ERRORR LRF=3 ggrmat + LCOMP=1 oracle (JENDL-3.3 U-238);... | 43,230 | 66 | — |
| 2026-09-10 | `0045ba59d` | reference-data: six evaluations from the NJOY2016 upstream te... | 327,652 | 12 | — |
| 2026-09-10 | `ba8b61513` | reference-data: those two tapes are JENDL-3.3, not JEFF-3.3 | 6 | 6 | — |
| 2026-09-11 | `0cd9a22c9` | outram-mc: reflective-sphere delta domain, so the FHR pebble ... | 448 | 46 | — |
| 2026-09-11 | `50033b8a7` | tests: 19 thermal tests were passing without asserting anything | 48 | 10 | — |
| 2026-09-11 | `f5c26afdd` | tests: OUTRAM_PARK_REQUIRE_REFERENCE_DATA turns a data-gated ... | 148 | 16 | — |
| 2026-09-11 | `a7280ecb3` | outram-mc: the naive-homogenised FHR pebble was a different r... | 402 | 62 | — |
| 2026-09-11 | `cd37bc399` | V&V: rewrite the ring-RPT record on the corrected, matched-ge... | 92 | 20 | — |
| 2026-09-11 | `22f7dc0e9` | outram-mc: single-case mode, and the baseline shift is data n... | 34 | 14 | — |
| 2026-09-11 | `53f64f5b5` | docs: bring the outram-mc-libs API mirror up to the DeltaDoma... | 72 | 9 | — |
| 2026-09-11 | `e744fac3e` | docs: Haiku dogfood of the ring-RPT / explicit-TRISO pebble A... | 327 | 0 | — |
| 2026-09-11 | `0472ef371` | tests: fix the red graphite sigma_inel pins, and add the inva... | 101 | 6 | — |
| 2026-09-11 | `4b95153d8` | dwsim-libs: fix RefluxedAbsorber — the port took a dead upstr... | 752 | 108 | — |
| 2026-09-11 | `43e57bb91` | outram-mc: ExplicitTrisoPebble, the builder the explicit path... | 273 | 26 | — |
| 2026-09-11 | `1314329ed` | dwsim-libs: let a material stream carry a flashed state | 396 | 0 | — |
| 2026-09-11 | `66ba367f6` | docs: mirror ExplicitTrisoPebble, the precondition for the do... | 60 | 0 | — |
| 2026-09-11 | `5d9d005de` | farrer-park: crate scaffold, verified licence provenance, des... | 364 | 0 | — |
| 2026-09-11 | `ed8db6de4` | outram-mc: re-export TrisoMaterials and ComputeType, found by... | 13 | 2 | — |
| 2026-09-11 | `229784c52` | docs: show the TrisoMaterials import line in the mirror | 6 | 0 | — |
| 2026-09-11 | `5c8bf90ea` | farrer-park: WIP snapshot — tensor, element, quadrature, mesh... | 4,009 | 0 | — |
| 2026-09-11 | `26d540cbb` | farrer-park: WIP snapshot — all 13 modules present, crate com... | 3,453 | 0 | — |
| 2026-09-11 | `9a7460892` | docs: Haiku dogfood run 2 — the A/B after ExplicitTrisoPebble | 91 | 0 | — |
| 2026-09-11 | `2d73a6b76` | farrer-park: WIP — patch-test harness, mesh/quadrature/solver... | 829 | 65 | — |
| 2026-09-11 | `c05290368` | farrer-park: WIP — full verification suite present (MMS, anal... | 1,151 | 68 | — |
| 2026-09-11 | `30705a432` | outram-mc: FHR pebble quickstart, the gap both dogfood runs c... | 239 | 0 | — |
| 2026-09-11 | `29e1119ca` | farrer-park: verification suite passing, results recorded; co... | 776 | 11 | — |
| 2026-09-11 | `1e1000ff2` | farrer-park: README limitations; record verification, gates a... | 34 | 3 | — |
| 2026-09-11 | `57d65535e` | farrer-park: doc-accuracy fixes in crate CLAUDE.md | 23 | 5 | — |
| 2026-09-11 | `7e2cb5338` | docs: add farrer-park to the workspace Members table and depe... | 8 | 0 | — |
| 2026-09-11 | `09c3f59b8` | outram-mc: compare six factors in OpenMC's convention, and an... | 248 | 1 | — |
| 2026-09-11 | `a2714f563` | outram-mc: graphite S(a,b) matches NJOY too -- the bias is in... | 191 | 5 | — |
| 2026-09-11 | `2577eb25c` | outram-mc: the scattering kernel is clean too -- every named ... | 144 | 19 | — |
| 2026-09-11 | `9cc24a6e5` | dwsim-libs: converged cut compositions, and #62's PR tests ar... | 679 | 1 | — |
| 2026-09-11 | `61c695467` | dwsim-libs: amend the port-coverage matrix, and correct two o... | 57 | 29 | — |
| 2026-09-11 | `04f5d1924` | dwsim-libs: audit stream / property-package ownership (#127) | 538 | 0 | — |
| 2026-09-11 | `690939345` | dwsim-libs: add a name -> Component lookup (no new compound d... | 1,223 | 2 | — |
| 2026-09-11 | `3f74b1d4f` | dwsim-libs: regenerate the API-doc mirror | 1,083 | 13 | — |
| 2026-09-11 | `2714d4134` | V&V examples: two that couldn't run here, could all along | 7,863,145 | 0 | — |
| 2026-09-11 | `8ae2a4975` | outram-mc: gate the HTR-10 double-heterogeneity difference, a... | 105 | 0 | — |
| 2026-09-11 | `a6ade05d4` | outram-mc: record the ring-RPT re-measurement -- the last gat... | 16 | 0 | — |
| 2026-09-11 | `585c1cd1d` | outram-mc: gate the four-method stochastic-media comparison | 147 | 0 | — |
| 2026-09-12 | `86da3c198` | outram-mc: move the maturity evidence off a run that cannot t... | 109 | 27 | — |
| 2026-09-12 | `1e8ff714b` | naming: OUTRAM PARK is Multi-Physics, not Multi-Phase | 50 | 15 | — |
| 2026-09-12 | `e97642c5f` | outram-mc: measure the two moments of the thermal kernel that... | 1,171 | 0 | — |
| 2026-09-12 | `a76cb84dd` | docs: cite the Outram Park Part I arXiv preprint (2608.17504) | 71 | 0 | 9,416,063 |
| 2026-09-12 | `190c2e73e` | naming: record how Multi-Physics reaches the literature, not ... | 35 | 6 | 1,275,153 |
| 2026-09-12 | `3697c0655` | docs: extract paper datasets for three manuscripts (op-rpb6) | 331 | 0 | 10,322,907 |
| 2026-09-12 | `7df8e711e` | docs: Part II carries a brief nuclear-data-preparation verifi... | 65 | 7 | 3,692,896 |
| 2026-09-12 | `e052afbac` | docs: split neutronics into Part II (data + transport) and Pa... | 252 | 138 | 4,182,591 |
| 2026-09-12 | `91589f112` | docs: fix the target set at three manuscripts (op-rpb6) | 28 | 12 | 1,735,517 |
| 2026-09-12 | `ccd55fc68` | docs: add the OpenMC absolute and cross-code equivalence tabl... | 9 | 0 | 4,787,557 |
| 2026-09-12 | `2a287620d` | outram-mc: pin the ring-RPT model against the OpenMC deck it ... | 231 | 0 | — |
| 2026-09-12 | `93e183212` | outram-mc: record the composition and provenance checks on th... | 37 | 4 | — |
| 2026-09-12 | `890505569` | outram-mc: the thick lump now has an oracle, and the Monte Ca... | 2,185 | 49 | — |
| 2026-09-12 | `3cdc2b1f8` | outram-mc: assert the FLiBe density correlation against the d... | 77 | 0 | — |
| 2026-09-12 | `5916b9177` | outram-mc: the thermal kernel was wrong in both dimensions of... | 1,500 | 203 | — |
| 2026-09-12 | `f540b6acf` | outram-mc: re-baseline every thermal number the emission-tabl... | 223 | 78 | — |
| 2026-09-12 | `bb94ac17b` | outram-mc: the bound thermal kernels do not have the right fi... | 596 | 0 | — |
| 2026-09-12 | `9efb44dce` | outram-mc: the emission-table resize is worth -371 +/- 194 pc... | 125 | 1 | — |
| 2026-09-12 | `e2ef43cfa` | outram-mc: price the graphite thermal law by deleting it -- i... | 133 | 1 | — |
| 2026-09-12 | `882cb3e34` | outram-mc: check the elastic angular law, then price it -- al... | 443 | 5 | — |
| 2026-09-12 | `75615b678` | outram-mc: give the pricing harness a positive control, and i... | 58 | 9 | — |
| 2026-09-12 | `45f8d72d8` | outram-mc/njoy: a threshold reaction cannot occur below its t... | 802 | 53 | — |
| 2026-09-13 | `5ff802613` | docs: rebaseline the paper datasets after the F-19 threshold ... | 204 | 87 | 13,996,182 |
| 2026-09-13 | `549fc5c6e` | docs: keep Part II -> Part III order, and caveat the gaps exp... | 109 | 21 | 3,125,614 |
| 2026-09-12 | `81c6a14c7` | outram-mc: sample the thermal kernel continuously, and find t... | 274 | 49 | — |
| 2026-09-13 | `2dd912fb3` | outram-mc: the continuous thermal kernel takes the FHR pebble... | 22 | 3 | — |
| 2026-09-13 | `76cb01110` | outram-mc: correct a k I derived instead of read, and carry t... | 39 | 5 | — |
| 2026-09-13 | `86907bf35` | docs: rebaseline Part III again after the continuous thermal ... | 34 | 21 | 36,539,815 |
| 2026-09-13 | `11fb94a36` | outram-mc: three V&V gates re-armed after the continuous ther... | 71 | 14 | — |
| 2026-09-13 | `9abfe8547` | outram-mc: LCT-008 measured against all three fixes, and the ... | 51 | 25 | — |
| 2026-09-13 | `626d995f2` | docs: rebaseline Part II datasets to the frozen tree (9abfe85... | 16 | 9 | 9,152,184 |
| 2026-09-13 | `e845dc2c0` | outram-mc: every bound thermal law this repo carries is narro... | 278 | 0 | — |
| 2026-09-13 | `42597707f` | njoy: bring sig()'s S(a,b) evaluation onto NJOY2016's own, in... | 328 | 40 | — |
| 2026-09-13 | `46219dd97` | outram-mc: Godiva's +57 pcm baseline was one seed's draw, not... | 82 | 15 | — |
| 2026-09-13 | `41abb39d8` | outram-mc+njoy: read the evaluated MF=6 continuum law instead... | 1,164 | 64 | — |
| 2026-09-13 | `0be4d89b7` | vv: record the FHR pebble's MF=6 re-measurement, and say what... | 73 | 4 | — |
| 2026-09-13 | `cecd8bd92` | njoy: adaptively linearise THERMR's E' grid -- most of H2O's ... | 205 | 33 | — |
| 2026-09-13 | `8b8f5a7eb` | vv: re-run LCT-008 on current code -- its residual is still g... | 28 | 8 | — |
| 2026-09-13 | `9058ee216` | outram-mc: record that N_OUTGOING is most of gh:#188's width ... | 38 | 0 | — |
| 2026-09-13 | `035f96179` | farrer-park: declare the crate mature (maintainer decision, 2... | 62 | 9 | — |
| 2026-09-13 | `102527c1d` | njoy: port THERMR calcem/sigl and ACER's MF=5/MF=6 laws, and ... | 3,510 | 820 | — |
| 2026-09-13 | `1010c0952` | njoy: gate THERMR's iform=1 continuous law, and settle an ope... | 282 | 0 | — |
| 2026-09-13 | `f5086cd21` | njoy: gh:#188 was never blocked -- NJOY2016 is built here, so... | 257 | 0 | — |
| 2026-09-13 | `c163ab847` | njoy: gh:#188 root-caused against NJOY's own emission matrix ... | 284 | 0 | — |
| 2026-09-13 | `7e850615d` | groupr: port f6dis, ll2lab and f6lab; verify the pass the tra... | 1,448 | 457 | — |
| 2026-09-13 | `32a43a086` | leapr: isym was computed wrongly, and two options were refuse... | 166 | 6 | — |
| 2026-09-13 | `1785f8271` | dwsim-libs: declare mature (maintainer decision, 2026-09-13) | 57 | 8 | — |
| 2026-09-13 | `4d074ea8b` | gh:#188 FIXED: replace the home-grown emission matrix with NJ... | 1,115 | 23 | — |
| 2026-09-13 | `9892347b4` | groupr: add emax_ev to the ZrH thermal-ACE test's options lit... | 2 | 0 | — |
| 2026-09-13 | `99800e7ec` | groupr: add emax_ev to the remaining thermal-ACE test options... | 2 | 0 | — |
| 2026-09-13 | `bd3c25072` | dwsim-libs: upstream DWSIM now runs headless — the maturity b... | 70 | 11 | — |
| 2026-09-13 | `83d60d507` | dwsim-libs: CORRECTION — the PR EOS matches upstream exactly;... | 6 | 6 | — |
| 2026-09-13 | `3cf70124d` | dwsim-libs: apply the correction to the crate file (83d60d50 ... | 32 | 16 | — |
| 2026-09-13 | `a1452324e` | dwsim-libs: trace the upstream EOS/CalcProp gap to a Peneloux... | 31 | 8 | — |
| 2026-09-13 | `9d567a806` | dwsim-libs: PT-flash parity vs upstream DWSIM — gap traced to... | 100 | 0 | — |
| 2026-09-13 | `721fcf9f5` | dwsim-libs: preserve the upstream headless harness; column so... | 248 | 0 | — |
| 2026-09-14 | `932819a51` | groupr: port getmf6 (File-6 continuum feed), and fix f6lab dr... | 1,723 | 42 | — |
| 2026-09-14 | `e945c03b4` | dwsim-libs: fix a library panic in the cubic root solver (fou... | 45 | 1 | — |
| 2026-09-14 | `622c65d71` | dwsim-libs: column-layer parity vs upstream DWSIM — all three... | 177 | 56 | — |
| 2026-09-14 | `5bbac8a82` | groupr: wire the File-6 continuum feed into the matrix driver... | 257 | 9 | — |
| 2026-09-14 | `8d612e33e` | leapr: wire and VALIDATE cold H2/D2 -- coldh reproduces NJOY'... | 5,033 | 22 | — |
| 2026-09-14 | `74d6fb2c4` | acer: survey the MF=5/MF=6 surface and make the one real gap ... | 22 | 3 | — |
| 2026-09-14 | `d801e59b2` | petir: vendor GSL with a VERIFIED licence, and the no_std ske... | 419 | 0 | — |
| 2026-09-14 | `7b669caa6` | petir: port GSL's Chebyshev module, verified code-to-code aga... | 2,976 | 0 | — |
| 2026-09-14 | `edb42e8e0` | petir: convention adapter, and code-to-code against tampines'... | 151 | 0 | — |
| 2026-09-14 | `5fe497855` | tampines: WIRE the Chebyshev evaluation to PETIR (bn:op-chyp.2) | 423 | 73 | — |
| 2026-09-14 | `cce9bba0e` | petir: port GSL's exponential integral E_1 -- the first half ... | 920 | 0 | — |
| 2026-09-14 | `6b230a234` | ACER: wire MF=5 LF=12 (Madland-Nix) onto PETIR -- it no longe... | 4,042 | 24 | — |
| 2026-09-14 | `a6f8974b8` | slides: OUTRAM PARK capabilities and roadmap (beamer) | 700 | 0 | — |
| 2026-09-14 | `b34fb2575` | Remove the last extern "C" calls into the system C libm (bn:o... | 2,668 | 96 | — |
| 2026-09-14 | `e64e99d1c` | outram-mc-libs: route the transcendentals through PETIR behin... | 400 | 103 | — |
| 2026-09-14 | `fab07081b` | petir: fast exp ported from ARM optimized-routines, 100 % bit... | 9,538 | 30 | — |
| 2026-09-14 | `8b33e0d99` | petir: fast log and pow ported from ARM optimized-routines, b... | 62,657 | 39 | — |
| 2026-09-14 | `663124c04` | petir: make the ARM port the default route for exp, ln and powf | 512 | 170 | — |
| 2026-09-14 | `27ac459ef` | outram-mc: price the four DH tracking treatments against each... | 639 | 0 | — |
| 2026-09-14 | `82346911c` | outram-mc: one enum to pick how a doubly heterogeneous univer... | 862 | 0 | — |
| 2026-09-14 | `1bc564515` | outram-mc: k-eff V&V across the three DH treatments | 186 | 0 | — |
| 2026-09-14 | `05f2f686d` | outram-mc: DH k-eff V&V measured — both "speedups" are slowdo... | 74 | 9 | — |
| 2026-09-14 | `166e4e8d7` | outram-mc: move the DH V&V onto ENDF/B-VIII.0, not the LOW em... | 118 | 29 | — |
| 2026-09-14 | `a0fdd6eaa` | outram-mc: record the HIGH-tier ENDF/B-VIII.0 DH V&V results | 42 | 49 | — |
| 2026-09-14 | `355ca6f0f` | outram-mc: DhTreatment::RingRpt did naive homogenisation, not... | 1,182 | 311 | — |
| 2026-09-14 | `7742e1861` | outram-mc: bound the delta majorant over reachable materials ... | 164 | 2 | — |
| 2026-09-14 | `d1454a942` | outram-mc: record the post-fix DH V&V, and retract the "appro... | 101 | 40 | — |
| 2026-09-14 | `6817e3018` | outram-mc: DH V&V at 9x statistics — ring-RPT reproduces exac... | 124 | 71 | — |
| 2026-09-14 | `37e1279ae` | docs: release notes and historian report for the 2026-09-14 m... | 1,828 | 0 | — |
| 2026-09-14 | `0e35efea7` | docs: reconcile the release-notes commit count with the histo... | 4 | 2 | — |
| 2026-09-14 | `07581a91c` | docs: state the release-notes count as work-only, so it stops... | 6 | 5 | — |
| 2026-09-14 | `5f433b0b4` | outram-mc: record the machine behind every DH timing, and wid... | 77 | 13 | — |
| 2026-09-14 | `097877617` | petir: lift the RealMath trait into the crate (bn:op-j57z) | 153 | 0 | — |
| 2026-09-14 | `baf9408b4` | tampines-steam-tables: route exp/ln/powf through PETIR's ARM ... | 264 | 173 | — |
| 2026-09-14 | `9fa1856e0` | docs: LaTeX listing of delta tracking and SCLS for the Part I... | 135 | 0 | — |
| 2026-09-14 | `b913378cd` | docs: file the kopi-beans 0.1.9 sync failure that strands wri... | 112 | 0 | — |
| 2026-09-14 | `eca488961` | outram-mc: stop printing a misleading OpenMC comparison from ... | 26 | 12 | — |
| 2026-09-14 | `4281cc8fd` | docs: correct the kopi-beans write-up — the daemon checkpoint... | 136 | 112 | — |
| 2026-09-14 | `040a011a0` | outram-mc: the smeared arms carried 3.7 % more fuel than the ... | 393 | 48 | — |
| 2026-09-14 | `350b7c202` | outram-mc: majorant under-bounded Sigma_t below its grid floo... | 252 | 16 | — |
| 2026-09-14 | `8b829eae0` | outram-mc: measure whether the CLS/SCLS lock costs anything (... | 162 | 0 | — |
| 2026-09-14 | `84bb23c71` | outram-mc: SCLS is the wrong method for a graphite pebble, an... | 76 | 7 | — |
| 2026-09-14 | `ee34c1e10` | outram-mc: measured GH #205 — the throughput claim is refuted... | 42 | 3 | — |
| 2026-09-14 | `9387f6b0d` | outram-mc: surface tracking computed nested-universe surface ... | 173 | 58 | — |
| 2026-09-14 | `f718e50d7` | outram-mc: the nested-frame crossing fix also moves the hex l... | 24 | 5 | — |
| 2026-09-14 | `2655d83fa` | outram-mc: LEU-COMP-THERM-008 goes from +2341 to +37 pcm agai... | 70 | 6 | — |
| 2026-09-14 | `1bc7c0fb4` | tampines-steam-tables: (rho,h) flash by inverting the IF97 ba... | 4,512 | 12 | — |
| 2026-09-14 | `11107ad04` | tampines-steam-tables: measure (rho,h) conditioning on the an... | 280 | 20 | — |
| 2026-09-14 | `56b3a83c6` | tampines-steam-tables: measure what the (rho,h) inversion cos... | 117 | 6 | — |
| 2026-09-14 | `2ab91fefc` | tampines-steam-tables: (p,h), (p,s) and (h,s) now cover Region 5 | 1,017 | 37 | — |
| 2026-09-14 | `2f13bf42c` | vv: re-extract the ring-RPT pebble case with the MF=6 continu... | 95 | 21 | — |
| 2026-09-14 | `f2fb10622` | docs: say which Part III numbers want to be small and which w... | 31 | 5 | — |
| 2026-09-14 | `a7a136c32` | tampines-steam-tables: two negative results on where (rho,h) ... | 403 | 43 | — |
| 2026-09-14 | `96d2740b3` | steam-tables GUI isochore now crosses the dome; Sod gains a t... | 363 | 126 | — |
| 2026-09-14 | `fc20443c9` | docs: drop the superseded kopi-beans queue entry, now filed u... | 0 | 136 | — |
| 2026-09-14 | `892c38982` | petir: a platform-libm escape hatch, and the Edwards golden r... | 1,978 | 2 | — |
| 2026-09-14 | `81a0ec15b` | tampines-steam-tables: the golden reference reproduces, and A... | 305 | 0 | — |
| 2026-09-14 | `45b598162` | outram-mc: LCT008 re-measured at the original statistics — +2... | 51 | 35 | — |
| 2026-09-14 | `24e568017` | tampines-steam-tables: the Edwards over-drain, and the energy... | 186 | 2 | — |
| 2026-09-14 | `ddb3ed443` | njoy: five doc comments that contradicted the code they document | 46 | 15 | — |
| 2026-09-14 | `5e18aafdf` | njoy: LRF=7 discarded its background R-matrix, and that was t... | 46,039 | 25 | — |
| 2026-09-14 | `fccb64356` | petir: the no_std numerics core -- GSL ports, verbatim lifts,... | 11,393 | 0 | — |
| 2026-09-14 | `d59323b40` | njoy: MT=1 was not the sum of its parts, and the obvious fix ... | 462 | 0 | — |
| 2026-09-14 | `15f0132f6` | njoy: a doc comment made a zero-cross-section hole look inten... | 39 | 3 | — |
| 2026-09-14 | `8174b6a5b` | tampines-steam-tables: A3 log -- the energy hold works, and t... | 105 | 0 | — |
| 2026-09-14 | `c681adcd0` | petir: recover the GSL Chebyshev port and its code-to-code re... | 2,983 | 4 | — |
| 2026-09-14 | `e75ce9ea3` | tampines-steam-tables: A4 finds a ported operator that was ne... | 175 | 4 | — |
| 2026-09-14 | `c1e705b1b` | tampines-steam-tables: a register of omitted upstream terms (... | 84 | 0 | — |
| 2026-09-14 | `38a30837c` | petir: recover expint, gamma_inc, cheb_slice and the real.rs ... | 8,034 | 58 | — |
| 2026-09-14 | `e3f4d1b07` | tampines-steam-tables: wire in the transient Rhie-Chow term, ... | 137 | 1 | — |
| 2026-09-14 | `93a0c6b08` | tampines-steam-tables: Edwards passes at 30 us again, and is ... | 147 | 3 | — |
| 2026-09-14 | `0d1f69040` | tampines-steam-tables: make the enthalpy-hold band-aid self-r... | 27 | 0 | — |
| 2026-09-14 | `bd6ce7ec1` | tampines-steam-tables: keep both band-aids, make them impossi... | 60 | 69 | — |
| 2026-09-14 | `9c190f4ca` | tampines-steam-tables: the hold never fires now -- over-drain... | 102 | 8 | — |
| 2026-09-14 | `4285fdf5b` | petir: pull in the consumer chain, port GSL roots + min, cred... | 75,689 | 292 | — |
| 2026-09-14 | `38a1b9cab` | petir: port GSL numerical differentiation, and fix a docs cla... | 515 | 2 | — |
| 2026-09-14 | `c11668824` | petir: port GSL interpolation and the tridiagonal solver (bn:... | 1,180 | 1 | — |
| 2026-09-14 | `521d83cae` | petir: port GSL quadrature and ODE integration -- the epic's ... | 1,757 | 3 | — |
| 2026-09-14 | `a4068bbfc` | njoy: reconstruct the unresolved range, and measure who is ri... | 254,576 | 0 | — |
| 2026-09-15 | `32fca1653` | njoy: figure and full-precision table for the URR interpolati... | 334 | 29 | — |
| 2026-09-15 | `9483d1d5b` | petir: remove every panicking subscript from the ported modul... | 1,291 | 325 | — |
| 2026-09-15 | `50cf30aeb` | Remove every panicking subscript from petir's verbatim lifts,... | 1,340 | 434 | — |
| 2026-09-15 | `6c057d812` | njoy: write MF=2/MT=152, and use NJOY's own table to settle t... | 707 | 24 | — |
| 2026-09-15 | `c802a59bd` | responsible use: say what counts as the initial human review | 11 | 0 | — |
| 2026-09-15 | `c30228477` | njoy: reconr() emits MF=2/MT=152, and the phase ordering that... | 168 | 7 | — |
| 2026-09-15 | `f024f6a41` | njoy: build eunr the way rdf2u0/u1/u2 build it, and close MT=... | 1,011 | 94 | — |
| 2026-09-15 | `722abe9b9` | njoy: close MT=152's last unresolved path with a synthetic Ca... | 381 | 28 | — |
| 2026-09-15 | `f619be3b5` | outram-mc: elastic slowing-down kinematics against a closed-f... | 289 | 0 | — |
| 2026-09-15 | `537dc92d9` | tampines-steam-tables GUI: isochores on a temperature sweep, ... | 453 | 129 | — |
| 2026-09-15 | `e141bbbb6` | tampines-steam-tables: p_rho_h_eqm_explicit is now actually e... | 215 | 13 | — |
| 2026-09-15 | `85330ef76` | tampines-steam-tables: Region 1 sub-boundary for explicit p(r... | 457 | 25 | — |
| 2026-09-15 | `f6d797f42` | tampines-steam-tables: the (rho,h) classifier was the dominan... | 357 | 5 | — |
| 2026-09-15 | `5152d096f` | tampines-steam-tables: measure the explicit route's cost, and... | 21 | 0 | — |
| 2026-09-15 | `81dda3904` | tampines-steam-tables: h-anchored T refinement fixes the liqu... | 197 | 39 | — |
| 2026-09-15 | `6e0cee598` | njoy: the unfac l>=3 defect reaches a SHIPPING library, not j... | 122,263 | 2 | — |
| 2026-09-15 | `e20aef9ef` | petir: Chebyshev least-squares fitting at arbitrary points, o... | 2,501 | 19 | — |
| 2026-09-15 | `491d36a25` | petir: port GSL's last two Chebyshev entry points, and pin th... | 906 | 18 | — |
| 2026-09-15 | `d4308bea7` | boon-lay: exhaustive code-to-code verification against upstre... | 5,667 | 1 | — |
| 2026-09-15 | `0e2b55cdd` | boon-lay: complete the TRISO-ATOPS code-to-code sweep (bn:op-... | 6,273 | 4,660 | — |
| 2026-09-15 | `84505ba89` | petir: declared mature; close the uneven-evidence gap the dec... | 1,513 | 11 | — |
| 2026-09-15 | `bf5a92ef4` | changi: new crate, FLEXPART port, verified code-to-code at tw... | 6,363 | 2 | — |
| 2026-09-15 | `ed9fd5152` | petir: bring the peroxide/roots acknowledgement up to date wi... | 84 | 0 | — |
| 2026-09-15 | `b123ee2a9` | genfoam/offbeat: code-to-code verification through the blockM... | 862 | 0 | — |
| 2026-09-15 | `63eee188f` | changi: record the CHANGI and REDHILL acronyms and CHANGI's f... | 107 | 44 | — |
| 2026-09-15 | `dbb50a761` | changi: PSA and consequence work is future scope; keep the cr... | 79 | 73 | — |
| 2026-09-15 | `58b178a05` | changi: close the decay code-to-code fixture gap (op-zq9r) | 223 | 30 | — |
| 2026-09-15 | `934ec2532` | petir: port the closed-form quartic from the `roots` crate, w... | 838 | 22 | — |
| 2026-09-15 | `c11e2e8fa` | genfoam: read upstream nuclearData + cellZones, and verify k_... | 1,668 | 20 | — |
| 2026-09-15 | `0be357364` | genfoam: gFHR pebble-conduction consistency check, and the tu... | 520 | 0 | — |
| 2026-09-15 | `394d3b5d9` | petir: port the companion-matrix root finder from `roots`, cr... | 1,521 | 0 | — |
| 2026-09-15 | `bff34bcb7` | petir: port polynomial algebra and the orthogonal families fr... | 1,346 | 102 | — |
| 2026-09-15 | `dcf3ff28d` | genfoam: port GeN-Foam's albedoSP3 boundary condition, and cl... | 934 | 1 | — |
| 2026-09-15 | `ac19c2a0d` | docs: record the MSFR albedo result in the tutorial verificat... | 41 | 12 | — |
| 2026-09-15 | `b54cce835` | petir: port Gauss-Legendre and Newton-Cotes quadrature from `... | 1,358 | 3 | — |
| 2026-09-15 | `e92a57a31` | petir: record the quadrature port in the crate CLAUDE.md | 12 | 2 | — |
| 2026-09-15 | `cc06471df` | genfoam: port nuclearSteadyStatePebble, the gFHR tutorial's p... | 606 | 0 | — |
| 2026-09-15 | `22479d1ed` | docs: record the gFHR pebble-model port in the tutorial verif... | 44 | 22 | — |
| 2026-09-15 | `22af2a44a` | docs: ready-to-post upstream issue drafts for `roots` and `pe... | 251 | 0 | — |
| 2026-09-15 | `7c06c3e2d` | docs: record the verification routes checked and ruled out | 28 | 0 | — |
| 2026-09-15 | `cf429ff5d` | genfoam: reproduce the gFHR tutorial's Tfmax_min from upstrea... | 178 | 0 | — |
| 2026-09-15 | `e429222b6` | outram-mc: a vacuum delta-tracking boundary, so Godiva can be... | 392 | 18 | — |
| 2026-09-15 | `c809b91c3` | outram-mc: Godiva against OpenMC on identical data -- the +24... | 333 | 4 | — |
| 2026-09-15 | `86296e35d` | outram-mc: the Godiva +250 pcm is NOT nuclear data -- measure... | 346 | 2 | — |
| 2026-09-15 | `f92babfb0` | outram-mc: localise the Godiva +250 pcm -- missing inelastic ... | 183 | 0 | — |
| 2026-09-15 | `544017b67` | outram-mc: ablate elastic anisotropy and URR, with the tests ... | 570 | 11 | — |
| 2026-09-15 | `97a739df5` | outram-mc: clear elastic scattering against OpenMC, and recor... | 241 | 4 | — |
| 2026-09-15 | `91e4cd830` | outram-mc: clear the fission spectrum too -- the Godiva attri... | 145 | 4 | — |
| 2026-09-15 | `b1c1d83ac` | genfoam: build and RUN upstream GeN-Foam, and compare MSFR ag... | 110 | 0 | — |
| 2026-09-15 | `32c6a4743` | genfoam: ESFR verified against upstream at 60.7 pcm; all four... | 121 | 0 | — |
| 2026-09-15 | `dd43a2e07` | genfoam: PSBT wall-friction closure verified against an upstr... | 1,410 | 0 | — |
| 2026-09-15 | `33d185cd8` | genfoam: gFHR pebble model verified against an upstream run, ... | 2,198 | 0 | — |
| 2026-09-15 | `bf153154f` | genfoam: per-cell cross-section evaluation, closing MSFR to 7... | 8,825 | 5 | — |
| 2026-09-15 | `f26d966b1` | docs: rewrite the tutorial verification map now that upstream... | 97 | 174 | — |
| 2026-09-15 | `cb4f01f70` | liggghts: translate the LIGGGHTS contact pipeline and verify ... | 5,876 | 11 | — |
| 2026-09-15 | `a194a169e` | outram-mc: measure Godiva's flux spectrum against OpenMC — it... | 613 | 4 | — |
| 2026-09-15 | `5bbe5fd7b` | liggghts: bulk pebble-bed cross-check, V&V report, and maturi... | 1,336 | 42 | — |
| 2026-09-15 | `90f9e894a` | liggghts: translate the CDT rolling model, bit-identical to u... | 572 | 24 | — |
| 2026-09-15 | `69f3f242a` | genfoam: close the MSFR case to +0.2 pcm — fix the Laplacian ... | 27,635 | 150 | — |
| 2026-09-15 | `caff1e0bb` | liggghts: record the angle-of-repose case as attempted and un... | 35 | 1 | — |
| 2026-09-15 | `40af17de2` | outram-mc: sample the discrete inelastic angular distribution... | 1,068 | 183 | — |
| 2026-09-16 | `6d83cf592` | outram-mc: Godiva reaches +16 +/- 11 pcm — record the result ... | 313 | 22 | — |
| 2026-09-16 | `418fff218` | outram-mc: confirm the Godiva fix with an ablation and the k_... | 117 | 6 | — |
| 2026-09-16 | `06aaf196e` | tampines-steam-tables: regenerate the (rho,h) flash V&V repor... | 6 | 6 | — |
| 2026-09-16 | `79c86486c` | liggghts: fix the port divergences instead of documenting the... | 810 | 104 | — |
| 2026-09-16 | `772d47d74` | liggghts: moving walls in the verified pipeline, and an ASCII... | 9,545 | 0 | — |
| 2026-09-16 | `c0833d132` | outram-mc: verify the Godiva physics independently of k, and ... | 995 | 13 | — |
| 2026-09-16 | `210afdbb4` | outram-mc: HTR-10 fuel pebble from Li/Yu/Wei (2014) Table 2, ... | 1,187 | 220 | — |
| 2026-09-16 | `bc2efc173` | outram-mc: give the ICSBEP benchmark examples seed ensembles ... | 138 | 3 | — |
| 2026-09-16 | `38466b847` | liggghts: document the SI-only simplification, and mesh neare... | 1,537 | 15 | — |
| 2026-09-16 | `e31de690c` | outram-mc: lift the HTR-10 Table 2 composition into the libra... | 542 | 175 | — |
| 2026-09-16 | `9c3639a5f` | outram-mc: record the measured HTR-10 boron ablation — the gr... | 40 | 0 | — |
| 2026-09-16 | `80963b93b` | nee_soon: HTR-10 code-to-code verification against the RMC paper | 676 | 1 | — |
| 2026-09-16 | `ad8e99cf4` | liggghts: angle of repose, done upstream's way and verified -... | 341 | 40 | — |
| 2026-09-16 | `9ed8daf63` | outram-mc: fix two defects in the benchmark seed-ensemble patch | 16 | 4 | — |
| 2026-09-16 | `789101219` | Gate tests over 5 minutes behind a default-on long-tests feature | 163 | 7 | — |
| 2026-09-16 | `98e47e58c` | neutronics: read the MF=6 continuum angular law, and two gate... | 2,114 | 151 | — |
| 2026-09-16 | `a130844d4` | neutronics: record the MF=6 angular work in both crates' CLAU... | 393 | 0 | — |
| 2026-09-16 | `b737cbb8e` | njoy: the LRF=7 doc still said the reconstruction was defective | 30 | 13 | — |
| 2026-09-16 | `3250f4bd7` | Gate the liggghts bulk-packing case too, on measured rather t... | 37 | 14 | — |
| 2026-09-16 | `f1ec77413` | outram-mc: V&V record for the MF=6 continuum angular law | 201 | 0 | — |
| 2026-09-16 | `50b2d1d44` | outram-mc: price the continuum angular law on Godiva -- a bou... | 85 | 13 | — |
| 2026-09-16 | `2a8bf8a6a` | neutronics: sample Kalbach-Mann (LANG=2), closing the MF=6 re... | 655 | 45 | — |
| 2026-09-16 | `96a1100d5` | outram-mc: measure whether the continuum angular law hardens ... | 384 | 0 | — |
| 2026-09-16 | `7eba5201b` | neutronics: the 128-seed price, a failed prediction, and a br... | 509 | 104 | — |
| 2026-09-16 | `d3ba63893` | outram-mc: build_data.py could never have run as committed | 24 | 4 | — |
| 2026-09-16 | `f26f535a9` | outram-mc: op-os8x re-measured on HEAD and LOCALISED in energy | 113 | 0 | — |
| 2026-09-16 | `6d3d0e58a` | neutronics: a systematic physics-coverage survey, and the con... | 445 | 0 | — |
| 2026-09-16 | `b774d3242` | outram-mc: free-gas target motion is ablatable in-process (co... | 392 | 25 | — |
| 2026-09-16 | `5b6df9b07` | outram-mc: the fission source is ablatable -- nu-bar and chi ... | 621 | 12 | — |
| 2026-09-16 | `f09bc4d2e` | docs: revise the heavy-run hand-off for the four new ablation... | 157 | 13 | — |
| 2026-09-16 | `fc2c234c6` | outram-mc: (n,2n) yield is ablatable -- and the control caugh... | 384 | 38 | — |
| 2026-09-16 | `245778b84` | docs: add the (n,2n) yield job to the heavy-run hand-off | 46 | 6 | — |
| 2026-09-16 | `5f1cf6f3c` | outram-mc: op-os8x's leading suspect is EXCLUDED -- the MT=91... | 469 | 3 | — |
| 2026-09-16 | `d716ab58f` | outram-mc: drivers for the fission-source and target-motion a... | 1,523 | 0 | — |
| 2026-09-16 | `64eda5fa6` | outram-mc: exclude the continuum sampler from op-os8x -- and ... | 610 | 94 | — |
| 2026-09-16 | `4723d974e` | njoy: PURR probability tables verified against NJOY2016 (U-238) | 2,236 | 0 | — |
| 2026-09-16 | `318223a7f` | outram-mc: wire URR probability tables into transport | 795 | 11 | — |
| 2026-09-16 | `fc8c5031a` | outram-mc: (n,3n) had no branch at all, and a source sampler ... | 438 | 30 | — |
| 2026-09-16 | `20f33c276` | outram-mc: implement DBRC, and fix a control that was passing... | 604 | 62 | — |
| 2026-09-16 | `678937391` | raffles: wire up burn as the no_std PyTorch replacement, behi... | 663 | 10 | — |
| 2026-09-16 | `9974caecd` | Gate the remaining 38 tests over 5 minutes, each on its own m... | 174 | 1 | — |
| 2026-09-16 | `6cd9eb845` | raffles: Bayesian model updating — TMCMC and TEMCMC, verified... | 2,302 | 5 | — |
| 2026-09-16 | `1fb68e666` | raffles: distance metrics and Approximate Bayesian Computation | 1,829 | 1 | — |
| 2026-09-16 | `767dc4bc2` | raffles: imprecise probability — confidence boxes and coheren... | 1,125 | 1 | — |
| 2026-09-16 | `3546351b9` | raffles: graph neural networks — message passing in burn, and... | 1,394 | 0 | — |
| 2026-09-16 | `0d1caef0f` | raffles: TMCMC-II tempering criterion and evidence-based mode... | 771 | 24 | — |
| 2026-09-16 | `163bd84f9` | raffles: surrogate models — polynomial regression and a burn-... | 1,452 | 8 | — |
| 2026-09-16 | `7454f12df` | raffles: bring the README and CLAUDE.md status sections in li... | 46 | 9 | — |
| 2026-09-16 | `57c622f77` | gnn: finish the MPNN port with training and rollout, and wire... | 1,412 | 14 | — |
| 2026-09-16 | `b448577d0` | raffles: port the GPL-3.0 tutorial case studies, including a ... | 343 | 0 | — |
| 2026-09-16 | `c260aff15` | gnn: read the upstream's own .pt datasets, and measure all fi... | 1,416 | 8 | — |
| 2026-09-16 | `c74f0b1eb` | outram-mc: depletion collapses one-group data against a spect... | 287 | 11 | — |
| 2026-09-16 | `2a0fb215f` | njoy: the MF=6 incident-energy interpolation flag, measured a... | 190 | 5 | — |
| 2026-09-16 | `46a9bf90b` | raffles: correct the licence record for Adolphus Lye's reposi... | 121 | 43 | — |
| 2026-09-16 | `6a8f42b66` | outram-mc: LCT-008 re-run -- the thermal residual is GONE | 87 | 0 | — |
| 2026-09-16 | `b56be5992` | outram-mc: price DBRC and URR on LCT-008 -- both bounded, nei... | 109 | 5 | — |
| 2026-09-16 | `82fdedb93` | njoy: MF=6 LAW=6 phase space, converted and verified against ... | 308 | 4 | — |
| 2026-09-16 | `0b38a0999` | njoy: LANG 11-15 and MF=5 LF measured -- unused, now enforced... | 146 | 2 | — |
| 2026-09-16 | `bfab39a2b` | raffles: cross-check the Bayesian modules against Adolphus Ly... | 858 | 5 | — |
| 2026-09-16 | `8378b1f7b` | Add outram-park-fork-cyclus: no_std Rust port of CYCLUS + CYC... | 12,861 | 0 | — |
| 2026-09-17 | `bdeb1ce44` | Complete the cyclus kernel: exchange, decay, toolkit, agents,... | 17,398 | 89 | — |
| 2026-09-17 | `36ce18e37` | liggghts: radial and axial porosity map of a settled bed, as ... | 861 | 0 | — |
| 2026-09-17 | `5cce18d30` | njoy: MF=6 LAW=7 -- fix two parser defects, then wire the sam... | 1,287 | 53 | — |
| 2026-09-17 | `e41638289` | Add cavity grid-convergence test: 20/40/80, observed order an... | 296 | 0 | 432,337,826 |
| 2026-09-17 | `9491abee5` | outram-mc: V&V record for the fission-source and target-motio... | 192 | 0 | — |
| 2026-09-17 | `807e9e231` | outram-mc: record the job 4 and job 5 results in the drivers'... | 79 | 0 | — |
| 2026-09-17 | `3f5e9f51d` | njoy+mc: wire MF=4/MF=5 emission laws, and fix a mu-bar quadr... | 1,808 | 164 | — |
| 2026-09-17 | `bc5516ad0` | mc+njoy: work the four open items -- price ANGLE_TOL, land 8 ... | 2,961 | 78 | — |
| 2026-09-17 | `47d608158` | TUAS thermophysical properties: no production panics, and an ... | 265 | 121 | 328,382,515 |
| 2026-09-17 | `f6aabcea5` | htgr_sim_v1: published HTR-10 kinetics, a trippable circulato... | 628 | 137 | 0 |
| 2026-09-17 | `60f6bb261` | outram-mc: correct the FHR ablation's verdict text -- the gap... | 45 | 3 | — |
| 2026-09-17 | `60dac13a0` | Stale docs are defects: fix three, and make correcting them a... | 50 | 3 | 62,375,157 |
| 2026-09-17 | `9bc626e87` | mc: wire MT=5 "(n,anything)", and exclude chi's SHAPE as an o... | 926 | 8 | — |
| 2026-09-17 | `8d2f1bd74` | htgr_sim_v1: derive the HTR-10 passive decay-heat chain from ... | 1,597 | 34 | 182,540,421 |
| 2026-09-17 | `46d41106a` | mc: exclude the cross sections BAND BY BAND, and show the op-... | 561 | 0 | — |
| 2026-09-17 | `dd8b11138` | outram-mc: port OpenMC's Shannon entropy, and fix a mesh boun... | 256 | 9 | 15,258,361 |
| 2026-09-17 | `a4e07ce17` | outram-mc: repair a semantic merge conflict in two spectrum e... | 5 | 5 | 5,399,720 |
| 2026-09-17 | `436770812` | outram-mc: exhaustive code-to-code verification of Shannon en... | 5,658 | 1 | 8,628,803 |
| 2026-09-17 | `e570e8654` | petir: remove the `std` and `platform-libm` features, which n... | 44 | 112 | 60,309,827 |
| 2026-09-17 | `eeaf739b0` | dhoby-ghaut: new crate, and move both GUI studios into it | 203 | 14 | 2,334,605 |
| 2026-09-17 | `05e1fe2dd` | docs: correct nine claims the code contradicts | 180 | 50 | 1,406,769 |
| 2026-09-17 | `ef7de0bf2` | boon-lay: migrate three egui examples to the current panel API | 13 | 13 | 0 |
| 2026-09-17 | `a4ae098f3` | mc: run op-os8x's discriminating experiment -- the residual i... | 176 | 18 | — |
| 2026-09-17 | `1eb1723eb` | outram-mc: gate depth-3 nested-lattice navigation, which noth... | 481 | 27 | 69,769,628 |
| 2026-09-17 | `9e1241327` | outram-mc: measure what a control rod costs a global majorant... | 274 | 0 | 18,537,553 |
| 2026-09-17 | `149ba4945` | outram-mc: exercise cell translation, and fix the frame offse... | 1,078 | 6 | — |
| 2026-09-17 | `2622b91cf` | njoy: enumerate V&V coverage module by module, and verify HEA... | 322 | 0 | — |
| 2026-09-17 | `f5289133a` | outram-mc: per-region tracking method and region-local majorants | 413 | 1 | 0 |
| 2026-09-17 | `ede5c51e4` | raffles: fix a misplaced Cell field from the tracking-method ... | 1 | 1 | 10,731,245 |
| 2026-09-17 | `d08b3fc7b` | outram-mc: bounded delta flight -- the boundary handoff for h... | 439 | 1 | 20,475,491 |
| 2026-09-17 | `bd308e9cf` | outram-mc: region extent -- which level declared delta, and h... | 158 | 0 | 26,224,862 |
| 2026-09-17 | `213c994f5` | liggghts: merge develop, enable gnn by default, half-stencil ... | 202 | 35 | — |
| 2026-09-17 | `31a71051f` | liggghts: HTR-10 full-core DEM case, contact-network API, GNN... | 55,782 | 0 | — |
| 2026-09-17 | `e91738f77` | njoy: V&V gaspr against NJOY2016 -- three real defects, all f... | 4,463 | 226 | — |
| 2026-09-17 | `c6d9b5717` | outram-mc: dispatch the transport loop on the region's tracki... | 96 | 5 | 24,839,973 |
| 2026-09-17 | `4d65204fe` | liggghts: GNN surrogate vs DEM -- a negative result, measured... | 55,509 | 55,139 | — |
| 2026-09-17 | `9557686e9` | outram-mc: report the majorant's price, and a cylindrical del... | 275 | 22 | 54,598,139 |
| 2026-09-17 | `f21b0dbfb` | liggghts: HTR-10 V&V write-up; raise stiffness to 5e8 after t... | 89 | 16 | — |
| 2026-09-17 | `cb99d06f7` | outram-mc: the hybrid tracking equivalence gate -- Phase 1 co... | 306 | 3 | 10,983,861 |
| 2026-09-17 | `4bb0201c7` | liggghts: correct four stale reference-data/doc claims; persi... | 68 | 14 | — |
| 2026-09-17 | `6fc94aee5` | outram-mc: adjudicate the HTR-10 TRISO radii, and settle the ... | 262 | 22 | 24,118,065 |
| 2026-09-17 | `b5b17c2cc` | liggghts: commit the dump->CSV converters the reference data ... | 67 | 3 | — |
| 2026-09-17 | `177b32bbc` | liggghts: regenerate the HTR-10 LIGGGHTS reference at E = 5e8... | 55,117 | 55,122 | — |
| 2026-09-17 | `b192b40c5` | outram-mc: cubic TRISO array in the fuel zone -- and 8335 exa... | 229 | 0 | 13,981,882 |
| 2026-09-17 | `e090d230c` | outram-mc: resolve the conus blocker with per-tile surface tr... | 205 | 0 | 10,033,028 |
| 2026-09-17 | `6d1461254` | outram-mc: wire Shannon entropy into the k-eigenvalue driver | 244 | 6 | 17,806,109 |
| 2026-09-17 | `4589165cd` | nee_soon: hex-prism bed tile assignment with the paper's 57:4... | 172 | 0 | 8,600,980 |
| 2026-09-17 | `08fc0e9f8` | kovan-literature: catalogue the HTR-10 RMC benchmark paper (p... | 19 | 0 | 15,632,453 |
| 2026-09-17 | `c2d0defed` | nee_soon: HTR-10 reflector zone compositions from IAEA-TECDOC... | 211 | 0 | 9,652,611 |
| 2026-09-17 | `4085c7773` | nee_soon: HTR-10 geometry-integrity gates in the default suite | 123 | 0 | 8,868,454 |
| 2026-09-17 | `edecbbaf5` | nee_soon: assemble the HTR-10 core, and measure that scale is... | 285 | 4 | 12,573,543 |
| 2026-09-17 | `82829a6fa` | outram-mc: V&V record for the HTR-10 RMC comparison | 82 | 0 | 2,715,800 |
| 2026-09-17 | `8239c42d8` | nee_soon: attempt the HTR-10 eigenvalue -- it fails, and here... | 459 | 9 | 12,838,565 |
| 2026-09-17 | `835502316` | nee_soon: rule the geometry OUT of the HTR-10 k = 0 failure | 82 | 1 | 5,579,316 |
| 2026-09-17 | `a93cae379` | nee_soon: clear the hybrid tracker of the HTR-10 k = 0 failure | 36 | 5 | 6,558,551 |
| 2026-09-17 | `5504ce826` | nee_soon: clear the MATERIALS too -- three eliminations, k st... | 42 | 1 | 7,553,541 |
| 2026-09-17 | `bf64272dc` | liggghts: consolidated cross-code summary; correct two self-c... | 204 | 7 | — |
| 2026-09-17 | `45fe3f2be` | mc: subtract the charged-particle channels from MT=27 to get ... | 29 | 4 | — |
| 2026-09-17 | `1db4e166b` | liggghts: HTR-10 at E = 5e8 passes; per-particle agreement is... | 27,696 | 17 | — |
| 2026-09-17 | `763fcb131` | nee_soon: localise the HTR-10 k = 0 failure to a stuck crossi... | 54 | 0 | 23,873,722 |
| 2026-09-17 | `5ee6b4110` | nee_soon: CONFIRM the HTR-10 k = 0 defect -- the nested TRISO... | 18 | 3 | 5,786,454 |
| 2026-09-17 | `f3ab8a682` | liggghts: key shear history by stable tag, as upstream does; ... | 244 | 12 | — |
| 2026-09-17 | `c3cea97a3` | nee_soon: fix the TRISO lattice construction, and CORRECT a w... | 49 | 3 | 25,313,845 |
| 2026-09-17 | `de8450e9a` | nee_soon: find half the missing fuel exactly -- one ball per ... | 40 | 0 | 10,845,155 |
| 2026-09-17 | `b931c6da1` | liggghts: HTR-10 slow pebble recirculation case -- geometry a... | 3,782 | 0 | — |
| 2026-09-17 | `04cec9a72` | njoy: cross-check HEATR against NJOY2016 -- KERMA and the unt... | 59,410 | 7 | — |
| 2026-09-17 | `0c81e1b2d` | njoy: run GAMINR against a REAL photoatomic evaluation, not j... | 483 | 24 | — |
| 2026-09-17 | `ac4ed77ec` | Fix HexLattice axial frame; HTR-10 core goes from k=0 to 0.857 | 999 | 100 | 76,980,263 |
| 2026-09-17 | `a9f2bf9d3` | mc: op-os8x's leading suspect is measured and REJECTED -- on ... | 257 | 42 | — |
| 2026-09-17 | `599d9d2bf` | docs: record the cell-translation regression result | 15 | 0 | — |
| 2026-09-17 | `fbc669505` | HTR-10 core reaches -909 +/- 108 pcm against the RMC benchmark | 531 | 133 | 47,366,130 |
| 2026-09-18 | `7fb86bf58` | liggghts: HTR-10 finished -- 0.61 reached, and it was frictio... | 311,496 | 118 | 0 |
| 2026-09-18 | `aa5611703` | CLAUDE.md: HARD RULE — get the process right first, the answe... | 63 | 0 | 18,550,235 |
| 2026-09-17 | `a78bae37f` | CLAUDE.md: type-check in release too, keeping one build tree | 26 | 2 | — |
| 2026-09-17 | `dc92a6e65` | CLAUDE.md: document dhoby-ghaut, correct two stale crate counts | 4 | 3 | — |
| 2026-09-18 | `96a632306` | Pool the ICSBEP benchmarks; correct six stale copies; HTR-10 ... | 825 | 103 | 150,890,886 |
| 2026-09-18 | `cd142f9e0` | CI: fast tests on five platforms; fix the Android gui gate it... | 542 | 16 | 59,496,309 |
| 2026-09-18 | `9c5d04355` | outram-mc: fix duplicate FilterKind import breaking two Godiv... | 2 | 2 | 362,682,528 |
| 2026-09-18 | `be6b8c3f8` | outram-mc: chase CLS and SCLS to correct implementations, wit... | 15,053 | 3,534 | 757,936 |
| 2026-09-18 | `8083ec6f6` | Gate BLAS behind an opt-in feature; exclude docs/ from 17 mor... | 89 | 24 | 48,526,747 |
| 2026-09-18 | `8f73c831a` | Add SEMBAWANG and REDHILL placeholders; stop CI cancelling it... | 230 | 2 | 15,525,830 |
| 2026-09-18 | `03ba4cc32` | liggghts: the repose gap halves -- 12.78 deg was a nondetermi... | 27,731 | 27,559 | 24,715,946 |
| 2026-09-18 | `3ad50bec1` | Fix the three test failures CI found on Linux, macOS and Windows | 150 | 25 | 143,453,282 |
| 2026-09-18 | `26b595cc0` | reference-data: add ENDF/B-VII.0, the library the HTR-10 refe... | 396,521 | 0 | 186,703,073 |
| 2026-09-18 | `2210027c7` | outram-mc/nee_soon: the HTR-10 residual was the DATA LIBRARY,... | 468 | 16 | 52,133,997 |
| 2026-09-18 | `c807233d8` | liggghts: fill the HTR-10 friction response curve across the ... | 137,998 | 2 | 2,001,720 |
| 2026-09-19 | `d9cb5b161` | outram-mc: outgoing-energy filter, so a scattering matrix is ... | 1,148 | 334 | — |
| 2026-09-19 | `367535155` | outram-mc: scattering matrix is now measured, and it conserve... | 463 | 2 | — |
| 2026-09-19 | `f3512b65a` | nee_soon: MGXS condensed from Monte Carlo, checked against ne... | 2,090 | 269 | — |
| 2026-09-19 | `8e0c7e91c` | outram-mc/nee_soon: the fission spectrum is measured, not ass... | 189 | 12 | — |
| 2026-09-19 | `fc53cd3cf` | nee_soon: GeN-Foam reproduces the Monte Carlo eigenvalue from... | 722 | 0 | — |
| 2026-09-19 | `b1d8974e2` | nee_soon: one coupling API, and the HTR-10 core carried throu... | 972 | 5 | — |
| 2026-09-19 | `cf7086296` | nee_soon: direct Monte Carlo <-> GeN-Foam coupling, iterated ... | 706 | 0 | — |
| 2026-09-19 | `83011650f` | nee_soon: Haiku dogfoods the coupling API; correct the stale ... | 216 | 8 | — |
| 2026-09-19 | `cbed624d3` | outram-mc: restore FilterKind -- two fixes for one bug cancel... | 2 | 2 | 125,492,211 |
| 2026-09-19 | `4b9a9a52d` | petir: WGSL f32 kernels for poly, Chebyshev and Legendre, ver... | 1,738 | 6 | — |
| 2026-09-19 | `0362fdaea` | petir: add the `wgpu` feature properly, with the std gate op-... | 508 | 356 | — |
| 2026-09-19 | `7dd5a76a6` | petir: use try_into in the GPU readback, so the no-panic gate... | 7 | 1 | — |
| 2026-09-19 | `276f2ec8e` | petir: GSL's error-function family in WGSL, generated from th... | 734 | 12 | — |
| 2026-09-19 | `6f50f6810` | petir: keep CI GPU-independent -- move shader validation out ... | 226 | 80 | — |
| 2026-09-19 | `e5dfb26e7` | outram-mc: cross-lineage GPU root-finder check, and a test th... | 476 | 1 | — |
| 2026-09-19 | `e16c2c72e` | petir: GSL matrices and CBLAS in WGSL, plus a coverage ledger... | 1,308 | 7 | — |
| 2026-09-19 | `46577a2d9` | outram-mc: verify the GPU scattering kinematics against close... | 498 | 0 | — |
| 2026-09-19 | `5a0143c44` | petir: GSL's gamma family in WGSL, and a prediction the measu... | 678 | 6 | — |
| 2026-09-19 | `82a6494c3` | petir: GSL's order-0/1 Bessel family, with every coefficient ... | 2,021 | 0 | — |
| 2026-09-19 | `a661a87d6` | petir: GSL's Bessel family in WGSL, and a second prediction t... | 1,835 | 9 | — |
| 2026-09-19 | `e0fd882d0` | petir: GSL's digamma and zeta families, and a 24th coefficien... | 2,503 | 137 | — |
| 2026-09-19 | `2ee1fb72b` | petir: psi and zeta in WGSL, and a test that was quietly chec... | 1,402 | 39 | — |
| 2026-09-19 | `0bf1f6857` | outram-mc: verify the GPU helpers directly, and close an unen... | 1,623 | 2 | — |
| 2026-09-19 | `84cd3c857` | petir: the all-features sweep runs in release, per the single... | 5 | 2 | — |
| 2026-09-19 | `7b6915860` | everything builds in release: the cross-targets were the bigg... | 92 | 37 | — |
