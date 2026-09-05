# OUTRAM PARK — historian report (all of `origin/develop` not in `origin/main`)

> Pre-merge-to-`main` accounting of the API tokens spent and the lines / KLOC written across this window of `develop` history. **Auto-generated** by `kovan historian`; regenerate with `kovan historian --from DDMMYY --to DDMMYY`.

## Scope

- **Branch:** `origin/develop` (vs base `origin/main`)
- **Window:** all of `origin/develop` not in `origin/main`
- **Commits (non-merge):** 998
- **Token coverage:** 391/998 commits carry an `API-Usage-Since-Last-Commit` trailer. Commits before the token-accounting hooks existed (or made outside a Claude session) contribute 0 and are counted here as *no token data* — that is correct, not missing data.

## Totals

### Lines written (git numstat, merges excluded)

| Metric | Lines | KLOC |
|---|--:|--:|
| Added (all files) | 5,793,097 | 5793.1 |
| Removed (all files) | 243,956 | 244.0 |
| **Net (all files)** | **5,549,141** | **5549.1** |
| Added (Rust `.rs`) | 1,013,014 | 1013.0 |
| Net (Rust `.rs`) | 820,235 | 820.2 |

### API tokens spent

| Component | Tokens |
|---|--:|
| input | 136,579 |
| output | 21,452,549 |
| cache_read | 8,354,067,985 |
| cache_write | 163,809,603 |
| **total** | **8,539,466,716** |

_`total` = input + output + cache_read + cache_write. Cache-read (prompt-cache re-reads of the growing context) usually dominates; the output figure is the closest proxy for net generated content._

## Lines added, by crate (top 20)

| Crate | Lines added |
|---|--:|
| `openfoam-appbuilder-lib` | 2,063,431 |
| `njoy-outram-park-fork` | 1,630,752 |
| `outram-park-fork-dwsim-libs` | 177,009 |
| `outram-park-digital-twin-engine` | 137,785 |
| `tampines-steam-tables` | 121,344 |
| `outram-mc-libs` | 107,569 |
| `tuas_boussinesq_solver` | 103,346 |
| `outram-park-fork-offbeat` | 96,122 |
| `outram-park-fork-coolprop` | 95,904 |
| `outram-foam-basic-lib` | 86,127 |
| `kovan-literature` | 78,844 |
| `outram-foam-appbuilder-lib` | 71,433 |
| `bedok` | 50,190 |
| `outram-park-fork-pflotran` | 47,933 |
| `tampines` | 45,717 |
| `boon-lay` | 39,530 |
| `outram-blender` | 34,257 |
| `openfoam-basic-lib` | 31,842 |
| `outram-foam-mesh` | 20,390 |
| `outram-park-fork-liggghts` | 15,778 |

## Per-commit ledger

| Date | Commit | Subject | +lines | -lines | Tokens |
|---|---|---|--:|--:|--:|
| 2026-06-22 | `a2715456` | removed started TH loop | 0 | 1 | — |
| 2026-06-23 | `f2a8dcbb` | noted that outside dome test fails again. Working on it now | 69 | 19 | — |
| 2026-06-23 | `99c15e63` | added notes on which datapoints need fixing | 47 | 55 | — |
| 2026-06-23 | `913de832` | added test results into comments: CLAUDE agent generated | 444 | 5 | — |
| 2026-06-23 | `6453b299` | added notes for CLAUDE.md that HEM has physical limitations | 33 | 15 | — |
| 2026-06-23 | `d0afc4c6` | added zaloudek in deom and subcooled files. | 1,083 | 0 | — |
| 2026-06-23 | `b6b54c7b` | added a basic rundown of openfoam-basic-lib | 372 | 0 | — |
| 2026-06-23 | `18566c1e` | added more things for sonnet to port over | 259 | 8 | — |
| 2026-06-23 | `04402b53` | added tensors and vector3 | 1,532 | 0 | — |
| 2026-06-23 | `975d1729` | added math and polynomial libraries, the solvers now use C ab... | 1,612 | 0 | — |
| 2026-06-23 | `177c01fd` | updated extern C policy | 28 | 0 | — |
| 2026-06-23 | `570a82b2` | added interpolation matrices and ode solvers | 1,247 | 3 | — |
| 2026-06-23 | `42650599` | openfoam basic lib at v0.1.1 | 2 | 2 | — |
| 2026-06-23 | `59a694a7` | workspace is also now v0.1.1 | 1 | 1 | — |
| 2026-06-23 | `c266e697` | updated claude md for specie-level thermophysics | 260 | 2 | — |
| 2026-06-23 | `2508d8d5` | scaffolded CLAUDE.md for thermophysical model conversion | 121 | 22 | — |
| 2026-06-24 | `0f0d5d23` | openfoam-basic-lib v0.1.2: Layer 1h specie-level thermophysics | 1,165 | 3 | — |
| 2026-06-24 | `272700ac` | added HRM critical flow solver comment | 299 | 0 | — |
| 2026-06-24 | `5f6830b4` | added polynomial models for openfoam | 492 | 3 | — |
| 2026-06-24 | `03940088` | added pengrobinson and tablulated h into the thermo libaries ... | 779 | 4 | — |
| 2026-06-24 | `3009aa4c` | changed glob imports as a code refactor | 71 | 131 | — |
| 2026-06-24 | `b6fd5427e` | added updated solver import targets | 5 | 4 | — |
| 2026-06-24 | `e578c2ae` | added openfoam fields, boundary and ldu matrix solvers | 1,749 | 2 | — |
| 2026-06-24 | `f0bdabef` | added note on ndarray-linalg | 13 | 0 | — |
| 2026-06-24 | `32d4dad6` | fixed doctests and imports | 22 | 3 | — |
| 2026-06-24 | `18e776ba` | updated doctest claude md policy | 11 | 10 | — |
| 2026-06-24 | `2a583053` | adding fvc and fvm, pending token reset | 719 | 0 | — |
| 2026-06-24 | `02b94de6` | finished adding and testing fv operators | 280 | 4 | — |
| 2026-06-24 | `3e441a96` | openfoam-basic-lib v0.1.3: add Layer 4 fluid thermodynamics | 574 | 4 | — |
| 2026-06-24 | `638ae657` | openfoam-basic-lib v0.1.4: icoFoam prerequisites + multi-regi... | 1,767 | 9 | — |
| 2026-06-24 | `d7e448e0` | no more auto commit for CLAUDE.md | 6 | 0 | — |
| 2026-06-24 | `551cd5a3` | added ddt and ddt_vec | 105 | 10 | — |
| 2026-06-24 | `d8e1860b` | addeda  matrix benchmark test for openfoam vs ndarray linalg | 270 | 75 | — |
| 2026-06-24 | `db2d7bec` | removed ndarray-linalg from tuas dependencies, using openfoam... | 55 | 136 | — |
| 2026-06-24 | `c8678d3f` | tuas boussinesq solver now bumped to v0.1.2, remoed the ndarr... | 30 | 28 | — |
| 2026-06-24 | `85ac0d8b` | moved tampines-steam-tables ndarray-linalg to dev depdencies | 59 | 3 | — |
| 2026-06-24 | `3d03b577` | added test backlog for openfoam-basic-libs | 95 | 0 | — |
| 2026-06-24 | `0ac6fcb5` | added new tests, need to pass them | 360 | 24 | — |
| 2026-06-24 | `19493f74` | openfoam-basic-lib v0.1.5: P2 tests, ignore known failures | 134 | 3 | — |
| 2026-06-24 | `715c521d` | added dwsim-libs and openmc-libs barebones, just a start of t... | 1,810 | 0 | — |
| 2026-06-25 | `f1e63ed7` | in process of adding boon lay | 414 | 6 | — |
| 2026-06-25 | `fb616d0f` | added boon lay scaffold and openmc rng distributions | 108 | 34 | — |
| 2026-06-25 | `9b79c7b9` | partially migrated boon lay decay simulator and triso simulat... | 13,167 | 24 | — |
| 2026-06-25 | `d53b1344` | added two phase thermosyspro modelica choked flow implementat... | 220 | 0 | — |
| 2026-06-25 | `d548fd2e` | scaffolded openfoam-appbuilder-lib and openfoam-turbulence-lib | 543 | 1 | — |
| 2026-06-25 | `14671497` | added first import of openfoam-appbuilder-lib and openfoam-tu... | 1,155 | 35 | — |
| 2026-06-25 | `1b1b5e7c` | added nnc261 matrix from matrix market | 1,657 | 22 | — |
| 2026-06-25 | `e772b001` | added hrmfoam rho central foam, rhopimplefoam and sonic foam ... | 898 | 110 | — |
| 2026-06-25 | `c9bcf20c` | added nnc261 solve for verification | 135 | 0 | — |
| 2026-06-25 | `533aa2e6` | nnc261 now comes with an explanation | 20 | 5 | — |
| 2026-06-25 | `9ca8a776` | added license to openfoam-appbuilder-lib and openfoam-turbule... | 4,457 | 31 | — |
| 2026-06-25 | `c92f64de` | bumped teh o prke to v0.1.1 | 3 | 3 | — |
| 2026-06-25 | `d80576a2` | started scaffolding the openFoam comparison files | 2,030,410 | 0 | — |
| 2026-06-25 | `a4ebf4e5` | noted that pimpleFoam cavity flow is from the icoFoam verific... | 44 | 14 | — |
| 2026-06-25 | `3bcaf2a7` | polyMesh parser now works! | 762 | 24 | — |
| 2026-06-25 | `37c47271` | pimplefoam cavity blockMesh runs okay | 0 | 1 | — |
| 2026-06-25 | `e3b4a610` | complete boon-lay triso simulator example port; mandate relea... | 1,743 | 54 | — |
| 2026-06-25 | `0d65e069` | wire up pimpleFoam/rhoCentralFoam/rhoPimpleFoam tutorial test... | 685 | 107 | — |
| 2026-06-26 | `7bec91e2` | document pimpleFoam port: OpenFOAM source, change justificati... | 157 | 1 | — |
| 2026-06-26 | `86368b7a` | sketch MC transmutation design in boon-lay CLAUDE.md | 40 | 4 | — |
| 2026-06-26 | `f3aed304` | add fvcDdtPhiCoeff limiter + fix PISO corrector loop; cavity ... | 195 | 136 | — |
| 2026-06-26 | `13ab29e6` | clarify Zaloudek data is HEM-computed, not experimental measu... | 20 | 9 | — |
| 2026-06-26 | `831a6882` | add MUSCL reconstruction (2nd-order rhoCentralFoam) and imple... | 715 | 56 | — |
| 2026-06-26 | `b676ea93` | add nuclear-data distribution design notes for the OpenMC port | 202 | 0 | — |
| 2026-06-26 | `3d117709` | added some stuff on openfoam-appbuilder-lib | 1,227 | 682 | — |
| 2026-06-26 | `50c95679` | rhopimplefoam undergoing fix | 232 | 96 | — |
| 2026-06-26 | `4d33b2ee` | added rhopimpelfoam fix | 81 | 34 | — |
| 2026-06-26 | `4f9092e3` | added ghia benchmark | 134 | 11 | — |
| 2026-06-26 | `8a4395d5` | benchmarks now have csv data to plot and show that they work! | 71 | 6 | — |
| 2026-06-26 | `3dbbdd9b` | Add HEM critical-flow solver for superheated vapour / supercr... | 404 | 18 | — |
| 2026-06-26 | `89b848fa` | Claude added possible causes. numerics being no.4 | 76 | 0 | — |
| 2026-06-26 | `b164d81b` | added opus conversation | 22 | 0 | — |
| 2026-06-26 | `2bf19134` | added notes for the ai agent to consider formulating a soluti... | 64 | 21 | — |
| 2026-06-27 | `3dc36344` | Add mandatory human interface layer design principle to CLAUD... | 36 | 0 | — |
| 2026-06-27 | `fffa4b2e` | Add project motivation (OpenFOAM negative example) to all ope... | 80 | 0 | — |
| 2026-06-27 | `5e2f0338` | Add mandatory Rust design rules: enums over trait objects, no... | 188 | 0 | — |
| 2026-06-28 | `559cdbea` | claude md trimmed down | 2,655 | 2,458 | — |
| 2026-06-28 | `243c2777` | Scaffold njoy-outram-park-fork: Rust port of NJOY2016 | 598 | 0 | — |
| 2026-06-28 | `d44e4775` | njoy-outram-park-fork: Phase 1 — ENDF tape reader + physical ... | 1,007 | 34 | — |
| 2026-06-28 | `ac619bb9` | njoy-outram-park-fork: add U-235 integration tests (485K line... | 485,519 | 0 | — |
| 2026-06-28 | `4640bce1` | njoy-outram-park-fork: Phase 2a — RECONR linearisation + MF=1... | 2,617 | 0 | — |
| 2026-06-28 | `34946668` | Phase 2b SLBW/MLBW resonance reconstruction + uom OOP API | 48,564 | 248 | — |
| 2026-06-28 | `54bf5340` | Add MtReaction enum; replace raw i32 MT numbers in public API | 560 | 44 | — |
| 2026-06-28 | `827f2fee` | Prepare njoy-outram-park-fork for crates.io publish | 67 | 55 | — |
| 2026-06-28 | `a747d07a` | Exclude integration tests and ENDF fixtures from published crate | 2 | 1 | — |
| 2026-06-29 | `b73a0f57` | njoy debugging via opus complete for reconr and broadr, pendi... | 1,385 | 136 | — |
| 2026-06-29 | `46f19c8f` | merged from branch PR | 391 | 248 | — |
| 2026-06-29 | `50632f05` | HEM dispatcher for tampines steam tables now working - debugg... | 355 | 87 | — |
| 2026-06-29 | `edc234bb` | doc comments debugged | 2 | 2 | — |
| 2026-06-29 | `0ffbe91e` | added mass and energy balance interface | 470 | 2 | — |
| 2026-06-29 | `3a6fadca` | in the midst of debugging moddy isobars... | 44 | 15 | — |
| 2026-06-29 | `ccd3e9ce` | added debug notes, and removed ndarray-linalg from tampines-s... | 45 | 10 | — |
| 2026-06-29 | `f4f99908` | added todo for ghia benchmark | 23 | 0 | — |
| 2026-06-29 | `aab9e7a8` | added polymesh for cavityFine map | 20,586 | 0 | — |
| 2026-06-29 | `54692c1f` | pimpleFoam cavity test now has mesh refined case | 144 | 26 | — |
| 2026-06-29 | `7e759c69` | added performance benchmarks | 44 | 0 | — |
| 2026-06-29 | `1585dacd` | moody tests passing, but the zaloudek tests failing. Also ope... | 435 | 65 | — |
| 2026-06-29 | `85ce745b` | added gamg solver, works well but doesn't speed things up | 910 | 468 | — |
| 2026-06-29 | `7173480c` | moody tests pass! | 18 | 9 | — |
| 2026-06-29 | `c7723157` | added temporary fix for moody's datapoints | 48 | 14 | — |
| 2026-06-29 | `3be7a23e` | added subcooling degree as discriminator | 44 | 4 | — |
| 2026-06-29 | `20e20d3f` | debugging moody test, tolerance TBD | 79 | 23 | — |
| 2026-06-29 | `561a6ffe` | renamed mod to interface | 3 | 3 | — |
| 2026-06-29 | `7b6def03` | added notes about interface | 4 | 0 | — |
| 2026-06-29 | `aff73b84` | added some comments to openfoam input and output and stuff | 3 | 0 | — |
| 2026-06-30 | `e2e4dac8` | Complete Moody deeply-subcooled assertions; document isobar_0... | 225 | 29 | — |
| 2026-06-30 | `1ae3670a` | v0.2.1 ready to push to cargo, moody tests are passing | 74 | 63 | — |
| 2026-06-30 | `56e1ce20` | Moody: run isobar_pref_0_25 in-dome-only; comment out deep di... | 119 | 43 | — |
| 2026-06-30 | `59539f7e` | added ace file writer and zaloudek/moody data | 1,639 | 12 | — |
| 2026-06-30 | `b495083e` | added zaloudek data for all curves. | 388 | 0 | — |
| 2026-07-01 | `a562538b` | njoy ACER Phase 4c: elastic angular distribution (MF=4 → LAND... | 587 | 25 | — |
| 2026-07-01 | `b56c1fd5` | added a one d mesh constructor | 258 | 23 | — |
| 2026-07-01 | `d47e249a` | added one dimensional meshing to openfoam-basic-libs, plus ch... | 96 | 35 | — |
| 2026-07-01 | `c2506793` | fixed polymesh builder error | 2 | 2 | — |
| 2026-07-01 | `3def3113` | njoy ACER 4d start (MF=5→Law 4) + Phase-4 scaffolds (S(α,β), ... | 480 | 5 | — |
| 2026-07-01 | `01bda7c1` | Scaffold TampinesSteamArray: 1-D rhoPimpleFoam on FvMesh | 417 | 5 | — |
| 2026-07-01 | `1c332c1d` | explained my dilemma with openfoam and tampines steam array. ... | 112 | 0 | — |
| 2026-07-01 | `0de3b57d` | docs(tampines): record appbuilder as steam-table consumption ... | 39 | 4 | — |
| 2026-07-01 | `ea4fdfca` | outram park ace 4d complete | 126 | 0 | — |
| 2026-07-01 | `026320d1` | added thermal writer mf7 | 27,213 | 6 | — |
| 2026-07-01 | `3af55170` | njoy ACER 4d: wire the DLW block — loadable table with energy... | 532 | 102 | — |
| 2026-07-01 | `e9b0fabe` | njoy ACER 4d: discrete-level non-elastic angular (MF=4 → AND) | 136 | 49 | — |
| 2026-07-01 | `ed736754` | njoy THERMR: coherent-elastic (Bragg) thermal scattering | 131 | 6 | — |
| 2026-07-01 | `e213cbe2` | docs: record the "own Rust parser, no C++ interop" rule for O... | 22 | 0 | — |
| 2026-07-01 | `3c8e2c50` | njoy THERMR: incoherent-inelastic scattering from S(alpha,beta) | 281 | 5 | — |
| 2026-07-01 | `85ab3210` | in the middle of adding s(alpha,beta) ace writer. I ran out o... | 118 | 14 | — |
| 2026-07-01 | `a9040103` | in the midst of adding ace test for thermal s(alpha,beta) | 353 | 71 | — |
| 2026-07-01 | `b16eea27` | added claudemd for openfoam algorithms | 116 | 0 | — |
| 2026-07-01 | `957ff487` | unit tests passing for openfoam import, but doctests failing | 15,212 | 6 | — |
| 2026-07-01 | `99d7d67b` | fixed doctests... but this assumes my openfoam algos are pub ... | 6 | 7 | — |
| 2026-07-01 | `31ea509f` | tampines steam table tests pass! | 3 | 3 | — |
| 2026-07-01 | `1b3f2ee0` | gamg doctest reset | 1 | 1 | — |
| 2026-07-01 | `6639d11a` | added pub crate port debt | 26 | 0 | — |
| 2026-07-02 | `e87c7ff5` | chore(tampines): make openfoam_source pub(crate) only | 38 | 38 | — |
| 2026-07-02 | `b7be4691` | ensure memory leaks do not happen in tests | 124 | 15 | — |
| 2026-07-02 | `c9b5df2a` | incoherent elastic complete! | 63 | 0 | — |
| 2026-07-02 | `47fa4ce6` | incoherent elastic complete | 250 | 8 | — |
| 2026-07-02 | `46e2ec76` | njoy: H(ZrH) incoherent-elastic thermal ACE integration test | 16,444 | 0 | — |
| 2026-07-02 | `cd839a43` | Neutronics scaffold: njoy owns all nuclear data; Keff + Doppl... | 682 | 75 | — |
| 2026-07-02 | `f8594518` | in midst of validating with u238 endf data | 430 | 42 | — |
| 2026-07-02 | `e8713d68` | njoy hdf5 u238 wmp done | 339 | 31 | — |
| 2026-07-02 | `a2f4dfb3` | wmp finished! ish | 5 | 3 | — |
| 2026-07-02 | `83a6c192` | added wmp nuclide manifest | 444 | 0 | — |
| 2026-07-02 | `c7086714` | wmp manifest: expand CORE to package D (LFTR salts) | 16 | 10 | — |
| 2026-07-02 | `bbbd5d4c` | keff roadmap: fast range via multigroup, not pointwise lean-ACE | 37 | 5 | — |
| 2026-07-02 | `934ca267` | docs: data acquisition tiers + parallel-run download safety | 126 | 0 | — |
| 2026-07-02 | `f5ead153` | docs(data-acquisition): collapse to two tiers — LOW (WMP+10-g... | 24 | 11 | — |
| 2026-07-02 | `0470a0cf` | njoy WMP: implement WMPB v1 blob codec (to_blob / from_blob) | 343 | 12 | — |
| 2026-07-02 | `9dabaa6c` | njoy WMP: add WmpLibrary multi-nuclide WMPL container | 206 | 0 | — |
| 2026-07-02 | `84dd677b` | njoy WMP: bake CORE blob (always embedded) + Watt-weighted fa... | 544 | 64 | — |
| 2026-07-02 | `299ef48b` | njoy: bake fast-range MGXS from ENDF/B-VIII.0 (RECONR MF=3 → ... | 638 | 8 | — |
| 2026-07-03 | `32f5d034` | openmc: first end-to-end Godiva Keff (WMP+MGXS → power iterat... | 862 | 95 | — |
| 2026-07-03 | `3ac34edc` | njoy: wire HIGH-fidelity ENDF download + selectable MGXS weig... | 1,151 | 27 | — |
| 2026-07-03 | `cb44e15d` | openmc: HIGH-fidelity Godiva on device-reconstructed ENDF + L... | 488 | 48 | — |
| 2026-07-03 | `c7615c31` | openmc: model inelastic scattering — Godiva HIGH −2510 pcm | 428 | 62 | — |
| 2026-07-03 | `dd2a78b1` | openmc: anisotropic MF=4 elastic scatter — Godiva HIGH reache... | 303 | 75 | — |
| 2026-07-03 | `09281038` | LOW tier: inelastic + forward elastic scatter; add pebble_bed... | 1,090 | 81 | — |
| 2026-07-03 | `94b62622` | openmc-libs: (n,2n) yield-2 multiplicity + canonical-source p... | 238 | 90 | — |
| 2026-07-03 | `c8815cfb` | godiva keff tests ongoing | 462 | 38 | — |
| 2026-07-03 | `d84fa5c0` | openmc/njoy: energy-dependent MF=5 fission spectrum χ (HIGH t... | 98 | 9 | — |
| 2026-07-03 | `dd20bcc9` | njoy/openmc: finish MF=5 porting — LF=7/9/11 + NK>1 mixtures;... | 651 | 93 | — |
| 2026-07-03 | `0114a6bd` | gaspr finished | 330 | 1 | — |
| 2026-07-03 | `3d7e3b0e` | docs: GASPR V&V entry + porting-plan Phase 3 note | 66 | 1 | — |
| 2026-07-03 | `7512665a` | docs: THERMR + thermal ACE writer were already done — fix sta... | 47 | 38 | — |
| 2026-07-03 | `50ed17cc` | docs(thermal): mark IFENG=1/2 and nmix>1 gaps inline, not jus... | 18 | 1 | — |
| 2026-07-03 | `dbd50ec0` | docs: HEATR phased porting scaffold (H1-H7) | 44 | 1 | — |
| 2026-07-03 | `679e3ac6` | njoy: HEATR H1 — elastic kinematic heating (MT=2) | 310 | 0 | — |
| 2026-07-04 | `19c43e7d` | njoy: HEATR H2 — local-deposition reactions (MT=102, 103-117) | 87 | 15 | — |
| 2026-07-04 | `42aa8ccc` | njoy: HEATR H3 -- single-escaping-neutron reactions (discrete... | 124 | 10 | — |
| 2026-07-04 | `36a78e83` | njoy: HEATR H4 -- fission heating (MT=18, 19-21, 38) | 371 | 49 | — |
| 2026-07-04 | `eeedd7ca` | njoy: HEATR H5 — multi-neutron-exit + continuum inelastic (MT... | 317 | 28 | — |
| 2026-07-04 | `5814a2b7` | njoy: HEATR H7 (elastic) — damage-energy production (MT=444),... | 285 | 6 | — |
| 2026-07-04 | `0a773118` | njoy: HEATR H5 — MF=6 emission spectra via EmissionSpectrum enum | 171 | 25 | — |
| 2026-07-04 | `364e8611` | njoy: ACE 4e — wire HEATR H1–H5 KERMA into the ESZ heating co... | 194 | 25 | — |
| 2026-07-04 | `1ab83314` | added some todos for tampines and tuas for porting to android | 113 | 0 | — |
| 2026-07-04 | `dc657f7d` | njoy: HEATR H7 — discrete-level damage energy (MT=51–90) | 182 | 60 | — |
| 2026-07-04 | `5a72772b` | njoy: HEATR H6 (part 1) — photon-production parser (MF=12 LO=... | 434 | 0 | — |
| 2026-07-04 | `bfd6f4e0` | njoy: HEATR H6 (part 2) — energy-balance KERMA + wire into ACE | 61 | 18 | — |
| 2026-07-06 | `1cd53a57` | tampines: fix Edwards-pipe test path in openfoam_algorithms C... | 78 | 59 | — |
| 2026-07-06 | `73146932` | scaffolded nee soon and started readmes ports for njoy | 2,364 | 118 | — |
| 2026-07-06 | `d34478c0` | added sod shock tube claude md | 255 | 0 | — |
| 2026-07-06 | `18d7a6fb` | cleaned up tampines-steam-tables dependencies to not use tuas... | 11 | 850 | — |
| 2026-07-06 | `33f44af6` | sod shock tube now validated! | 778 | 5 | — |
| 2026-07-06 | `b5b88452` | added github readme to sod shock tube | 334 | 0 | — |
| 2026-07-06 | `e6059dcd` | changed github markdown, hopefully it renders equations cleanly | 57 | 39 | — |
| 2026-07-06 | `5435c5b2` | changed claude md to include readme markdown checks | 36 | 0 | — |
| 2026-07-06 | `86c8513d` | njoy u238 test port in progress, updated openfoam sod shock t... | 196,888 | 278 | — |
| 2026-07-06 | `72465396` | doppler test initial commit ok | 200,512 | 23 | — |
| 2026-07-06 | `4768abef` | njoy: UNRESR physics kernel port (ENDF LRU=2, Faddeeva W-func... | 1,566 | 43 | — |
| 2026-07-06 | `bafda768` | njoy: PURR scaffolding port (ENDF reuse, uw2, RNG, ladder gen... | 785 | 61 | — |
| 2026-07-06 | `367644e9` | njoy: PURR unrest port -- Monte Carlo probability-table binni... | 881 | 74 | — |
| 2026-07-07 | `51335d73` | njoy: SAMM Phase 1 -- ENDF LRF=7/KRM=3 (R-matrix-limited) par... | 418 | 23 | — |
| 2026-07-07 | `959a4a4a` | njoy: SAMM Phase 2 -- spin/parity/penetrability setup (non-Co... | 758 | 38 | — |
| 2026-07-07 | `ced6c221` | njoy: SAMM Phase 3 -- Coulomb wave functions, plus betset's n... | 1,433 | 46 | — |
| 2026-07-07 | `d6a6fe84` | njoy: SAMM Phase 4 -- R-matrix inversion (LINPACK-style solver) | 939 | 13 | — |
| 2026-07-07 | `f465587c` | njoy: split samm::coulomb into a directory module by function... | 1,168 | 1,097 | — |
| 2026-07-07 | `9c6c9b5a` | njoy: SAMM Phase 5 -- cross-section formula (first end-to-end... | 757 | 30 | — |
| 2026-07-07 | `2dfe17b4` | njoy: SAMM Phase 6 -- top-level orchestration (setup + cssammy) | 215 | 20 | — |
| 2026-07-07 | `7651ce3d` | njoy: wire samm (LRF=7 R-Matrix Limited) into RECONR's resona... | 173 | 20 | — |
| 2026-07-07 | `ffc0d00e` | njoy: fix U-238 capture wing pedestal (RECONR grid density) +... | 726 | 47 | — |
| 2026-07-07 | `36853140` | openmc: add Godiva HIGH-fidelity k_eff V&V doc (validation.md) | 58 | 0 | — |
| 2026-07-07 | `23fbd813` | added trademarks and attributions to every crate | 1,130 | 0 | — |
| 2026-07-08 | `c87bbb8e` | bd init: initialize beads issue tracking | 740 | 0 | — |
| 2026-07-08 | `62ae6131` | workspace: adopt beads (bd) for issue/roadmap tracking; seed ... | 73 | 1 | — |
| 2026-07-08 | `d1fe5a33` | beads: defer njoy fissile-path speedup (research-worthy); mar... | 2 | 2 | — |
| 2026-07-08 | `b97198bd` | beads: track tampines blowdown-test PR (Ethan) + trademark-co... | 10 | 0 | — |
| 2026-07-08 | `5ce3b023` | beads: prke/tuas inline matrix solvers (drop openfoam-basic-l... | 6 | 2 | — |
| 2026-07-08 | `54fe1179` | beads: plan outram-park-fork-coolprop (CoolProp Rust translat... | 1 | 0 | — |
| 2026-07-08 | `9fba09c3` | beads: refresh issues.jsonl export (coolprop cross-links + note) | 7 | 2 | — |
| 2026-07-08 | `06db42b8` | android: gate ndarray-linalg matrix bench off Android + add A... | 40 | 0 | — |
| 2026-07-08 | `00163d3f` | beads: refresh export (Android epic links) | 6 | 2 | — |
| 2026-07-08 | `89dd3907` | prke/tuas: inline SquareMatrix LU solver; drop openfoam-basic... | 596 | 11 | — |
| 2026-07-08 | `66e4632a` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `7fdda9f6` | coolprop: initiate outram-park-fork-coolprop; first verified ... | 766 | 3 | — |
| 2026-07-08 | `23474425` | beads: refresh export | 6 | 2 | — |
| 2026-07-08 | `3fa43ec0` | coolprop: add Helium fluid + EnthalpyEntropyOffset ideal term | 105 | 1 | — |
| 2026-07-08 | `cf32270c` | beads: refresh export | 1 | 1 | — |
| 2026-07-08 | `be3a1940` | coolprop: full 137-fluid codegen coverage + term-type engine ... | 800 | 14 | — |
| 2026-07-08 | `853a8511` | coolprop: wire all 137 CoolProp fluids into the Fluid enum | 5,168 | 69 | — |
| 2026-07-08 | `b2a903ed` | coolprop: vendor openfoam_algorithms + add OPCPFluidSingleCV ... | 17,074 | 21 | — |
| 2026-07-08 | `d396febf` | coolprop: rename TampinesSteamArray -> OPCPFluidArray + wire ... | 113 | 48 | — |
| 2026-07-08 | `1231c5e0` | coolprop: drop the unused vendored solvers, keep only rhoPimp... | 23 | 609 | — |
| 2026-07-08 | `0cff3fde` | coolprop: add transport (mu/lambda), saturation ancillaries, ... | 2,795 | 48 | — |
| 2026-07-08 | `a90cb2b0` | coolprop: port Helium's hardcoded transport (mu + lambda) | 193 | 25 | — |
| 2026-07-08 | `25433b20` | coolprop: port Water (IAPWS) and CO2 (Laesecke/Huber) hardcod... | 189 | 25 | — |
| 2026-07-09 | `840ef1ae` | changed tuas to v0.1.3 | 7 | 3 | — |
| 2026-07-09 | `a69e0e76` | added teh-o-prke to next level, will yank v0.1.1 | 21 | 3 | — |
| 2026-07-10 | `48fb5de7` | coolprop: expand hardcoded transport + add Olchowy-Sengers cr... | 747 | 310 | — |
| 2026-07-10 | `67be164c` | coolprop: finish all per-fluid hardcoded transport formulas | 366 | 44 | — |
| 2026-07-10 | `1ba44705` | coolprop: scaffold HumidAir, incompressibles, mixtures, non-a... | 804 | 8 | — |
| 2026-07-10 | `fd4b8e85` | coolprop: implement non-analytic terms, HumidAir, incompressi... | 1,661 | 299 | — |
| 2026-07-10 | `fc0b5163` | coolprop: document why the remaining scaffold gaps are scoped... | 41 | 4 | — |
| 2026-07-10 | `28f8a90f` | coolprop: port all 126 CoolProp incompressible fluids via cod... | 3,802 | 114 | — |
| 2026-07-10 | `70066fe2` | coolprop: port 840 CoolProp mixture binary pairs via codegen | 2,539 | 141 | — |
| 2026-07-10 | `90cffff8` | workspace: scaffold verification_and_validation/, upstream_so... | 3,217 | 177 | — |
| 2026-07-10 | `136d1a4e` | coolprop: add auto-generated docs/api.md via rustdoc-md (nigh... | 14,283 | 1 | — |
| 2026-07-10 | `50ea849a` | beads: file op-4wl.2 (replace RELAP/Zweibaum loss coeffs with... | 1 | 0 | — |
| 2026-07-10 | `130ecd4a` | added debug to opcp fluid array | 1 | 1 | — |
| 2026-07-10 | `3017f6a7` | openfoam-*-lib: add docs/api.md; file TODO to rename to outra... | 15,131 | 10 | — |
| 2026-07-10 | `ecebd565` | beads: refresh export (op-ahi epic note timestamp) | 3 | 3 | — |
| 2026-07-11 | `9034104b` | CLAUDE.md: add mandatory working-hours guardrail (AI safety) | 38 | 0 | — |
| 2026-07-11 | `abe0e57f` | beads: file op-fvc (cross-platform work-hours time-check scri... | 1 | 0 | — |
| 2026-07-13 | `b5c8bd8d` | Rename OpenFOAM/OpenMC crates to outram-foam-*/outram-mc-libs | 8,726 | 586 | — |
| 2026-07-13 | `ec21a700` | docs: add valuation.md (cost/value estimate); close op-cpe | 144 | 5 | — |
| 2026-07-13 | `194c75dc` | coolprop: implement HumidAir wet-bulb + dew-point; add missin... | 498 | 36 | — |
| 2026-07-13 | `c4e69746` | coolprop: fix stale ha_props doc comment (T_wb/T_dp are now o... | 8 | 4 | — |
| 2026-07-13 | `cbb2460d` | coolprop: HumidAir entropy S + T_wb/T_dp as input keys (op-kb... | 514 | 79 | — |
| 2026-07-13 | `386cba4b` | outram-foam/outram-mc crates: align version to 0.1.0 | 4 | 4 | — |
| 2026-07-13 | `c58aae96` | added important provenance does | 25 | 11 | — |
| 2026-07-13 | `1c2293de` | docs: finish provenance pass -- dwsim README, OpenFOAM matrix... | 60 | 6 | — |
| 2026-07-13 | `d144faea` | added new digital twin plan markdown and beads | 195 | 1 | — |
| 2026-07-13 | `913d591e` | coolprop: wire lateral coupling + heat source into step(), cl... | 377 | 2 | — |
| 2026-07-13 | `5bca4347` | tampines crate added | 141 | 25 | — |
| 2026-07-13 | `e778c4c1` | did some housekeeping and AI USAGE markdowns | 652 | 10 | — |
| 2026-07-13 | `c61d1451` | added responsible use to outram park | 338 | 23 | — |
| 2026-07-13 | `bd1df926` | docs: wire RESPONSIBLE_USE/DATA_POLICY/AI_USAGE/etc. into roo... | 246 | 46 | — |
| 2026-07-13 | `5b18f6c3` | dwsim-libs: port pipe correlations (Darcy-Weisbach + Beggs-Br... | 722 | 3 | — |
| 2026-07-13 | `da722cc0` | docs: fold in AI Agent Instructions (license/provenance prese... | 10 | 0 | — |
| 2026-07-13 | `5f53f5e8` | dwsim-libs: port valve/heat_exchanger/expander/pump, wire int... | 1,351 | 20 | — |
| 2026-07-13 | `e64b29bc` | dwsim-libs: close out the 4 deferred DWSIM ports (op-qo2.6-9) | 1,080 | 11 | — |
| 2026-07-13 | `e82c8f59` | tampines: add components/ -- 8 BOP component structs (op-dt3.4) | 439 | 1 | — |
| 2026-07-13 | `f0e1869b` | tampines: add balance_of_plant/ + cooling_tower/ grouping mod... | 82 | 1 | — |
| 2026-07-13 | `db818e0e` | outram-park-digital-twin-gui: crate scaffold + color_maps + c... | 741 | 3 | — |
| 2026-07-13 | `ec1482dd` | outram-park-digital-twin-gui: add animation/ tracer/travel-ti... | 126 | 1 | — |
| 2026-07-13 | `3b4fdfc2` | outram-park-digital-twin-gui: add app_scaffold/ threading/pan... | 189 | 1 | — |
| 2026-07-13 | `58ce3650` | tampines-steam-tables: give TampinesSteamArray a FluidArray/O... | 445 | 14 | — |
| 2026-07-13 | `356daff5` | tampines-steam-tables: wire real IAPWS-IF97 (p,h) flash into ... | 123 | 24 | — |
| 2026-07-13 | `1c59327f` | Add DEVELOPER_HEALTH_WARNING.md: agentic development health/s... | 429 | 1 | — |
| 2026-07-14 | `f9f6e1ad` | Add Nordheim-Fuchs exact timestepper, wire teh-o-prke -> nee_... | 654 | 27 | — |
| 2026-07-14 | `88ffcd91` | beads: sync issues.jsonl export | 3 | 1 | — |
| 2026-07-14 | `86ccb04b` | Move fhr_sim_v2 to tampines, reconstruct kinetics on Nordheim... | 251 | 173 | — |
| 2026-07-14 | `540fd454` | bug fixes in progress | 760 | 54 | — |
| 2026-07-14 | `b49686ef` | tampines-steam-tables: handle two-phase region in lambda_ph_e... | 172 | 23 | — |
| 2026-07-14 | `5311d461` | Add OpenFOAM-style pressure bounding to TampinesSteamArray + ... | 779 | 0 | — |
| 2026-07-14 | `032e95f0` | Wire TampinesSteamArray into fhr_sim_v2 steam-generator tube | 425 | 29 | — |
| 2026-07-14 | `45a3ebc5` | tampines-steam-tables README: log the (p,h)-flash coaching as... | 27 | 0 | — |
| 2026-07-15 | `494bc908` | beads: log vendoring rule (op-264) and kovan crate scaffold (... | 2 | 0 | — |
| 2026-07-15 | `7298c73a` | docs: add blanket 'unverified until validated' notice to all ... | 332 | 0 | — |
| 2026-07-15 | `9274eb6c` | njoy: fix SAMM mf2 eliminated-channel reorder + partial ERROR... | 4,348 | 99 | — |
| 2026-07-15 | `8fd32636` | beads: sync export after NJOY fleet run (op-cjw.3 closed, op-... | 5 | 3 | — |
| 2026-07-15 | `27744b72` | Port NJOY front-ends: GROUPR/GAMINR/LEAPR/COVR/MIXR/RESXSR/DT... | 10,875 | 203 | — |
| 2026-07-15 | `881b86e4` | kovan: add KOVAN knowledge layer as 7 workspace member crates | 2,838 | 8 | — |
| 2026-07-15 | `f63cdb42` | beads: sync export (op-145 kovan scaffold closed; NJOY-rest f... | 4 | 2 | — |
| 2026-07-15 | `0d21e93e` | beads: add KOVAN epic op-5v5 + implementation child beads | 5 | 0 | — |
| 2026-07-15 | `bd522246` | docs/beads: openmc-notebooks become verification tests; outra... | 43 | 0 | — |
| 2026-07-15 | `66915b19` | outram-mc: scaffold openmc-notebooks verification harness + m... | 877 | 0 | — |
| 2026-07-15 | `7a0d9ba8` | beads: sync export (outram-mc notebook track op-6tz.7-.22) | 24 | 0 | — |
| 2026-07-15 | `d32e223d` | njoy op-6tz.6: OpenMC data-notebooks as verification tests (m... | 915 | 0 | — |
| 2026-07-15 | `39de0cb4` | beads: THERMR H-in-H2O bead op-cjw.19 (blocks op-6tz.12 therm... | 13 | 7 | — |
| 2026-07-15 | `44e2ebf7` | njoy thermr op-cjw.19: complete H-in-H2O S(alpha,beta) incohe... | 764 | 32 | — |
| 2026-07-15 | `ad7ee71f` | beads: THERMR op-cjw.19 closed; follow-ups op-cjw.20/.22/.23 | 2 | 1 | — |
| 2026-07-15 | `38faf1f8` | outram-mc op-6tz.7/.8/.9/.10/.16: wire CSG geometry + surface... | 1,876 | 167 | — |
| 2026-07-15 | `853d4ba7` | beads: outram-mc pincell/triso track (op-6tz.7/.8/.9/.10 clos... | 9 | 2 | — |
| 2026-07-15 | `377a6871` | outram-mc: wire H-in-H2O S(a,b) thermal scattering into trans... | 920 | 37 | — |
| 2026-07-15 | `c3673cd4` | beads: thermal pincell op-6tz.12 closed (k_inf=1.398); follow... | 10 | 6 | — |
| 2026-07-15 | `cafc3d45` | Session batch: engine rename, KOVAN implementation, TRISO-ATO... | 32,067 | 5,211 | — |
| 2026-07-15 | `6c3fcfa1` | outram-mc: compare pincell k_inf vs openmc pincell.ipynb; CSV... | 24 | 1 | — |
| 2026-07-15 | `10849ffa` | vv: commit pincell k_inf comparison CSV (force-added past fol... | 2 | 0 | — |
| 2026-07-15 | `0db6f021` | CLAUDE.md: add optional Singlish mode (chat prose only) | 25 | 0 | — |
| 2026-07-15 | `044f1f6e` | outram-mc op-6tz.11: port OpenMC HexLattice + make hexagonal-... | 1,136 | 21 | — |
| 2026-07-15 | `b5ece92a` | beads: hex-lattice op-6tz.11 closed; follow-ups op-6tz.29/.31 | 2 | 1 | — |
| 2026-07-15 | `ece1872b` | boon-lay: TRISO-ATOPS first-principles derivation docs (Pytho... | 718 | 3 | — |
| 2026-07-15 | `ce8f8286` | outram-mc notebooks: DAGMC won't-port, pandas on hold, unstru... | 18 | 14 | — |
| 2026-07-15 | `e161253d` | outram-mc op-6tz.16/.25: random TRISO packing + delta-trackin... | 1,552 | 141 | — |
| 2026-07-15 | `2877d9c5` | beads: triso finish track (op-6tz.16/.25 random packing + del... | 16 | 10 | — |
| 2026-07-15 | `c7a749a6` | tampines (p,h)-flash R4/R5 hardening + TUAS CIET pipe-38 K=17... | 1,177 | 235 | — |
| 2026-07-15 | `6dd420e6` | njoy P2/P4 fleet pass: GROUPR vector engine, LEAPR MF=7, COVR... | 8,113 | 958 | — |
| 2026-07-15 | `89340ac1` | tuas: SAM (NED-2021 Table 4) comparison columns + AI-generate... | 838 | 18 | — |
| 2026-07-15 | `d9c293e2` | beads: RPT reproduction intake bead op-vfb; depletion/op-3ut ... | 7 | 2 | — |
| 2026-07-15 | `4a6d6082` | outram-foam-basic-lib: tensor/vector-field FV operators (op-y... | 4,927 | 1,855 | — |
| 2026-07-15 | `f4c10b61` | outram-foam-appbuilder-lib: GeN-Foam multiphysics port (neutr... | 24,147 | 95 | — |
| 2026-07-15 | `a56a1974` | beads: fleet progress sync (op-3ut GROUPR matrix + depletion ... | 2 | 1 | — |
| 2026-07-15 | `ac56abdb` | outram-mc: depletion / transmutation driver + LIVE depletion ... | 2,421 | 12 | — |
| 2026-07-15 | `ef567a5d` | beads: depletion op-6tz.18 closed (CRAM); follow-ups op-23s/o... | 2 | 1 | — |
| 2026-07-15 | `87d093f3` | groupr matrix path: skeleton modules (matrix, unresolved, gam... | 112 | 0 | — |
| 2026-07-15 | `91d0bc88` | groupr/gaminr matrix path (op-3ut): scatter matrix, CM->lab k... | 4,057 | 87 | — |
| 2026-07-15 | `df8a82b8` | beads: GROUPR matrix path (op-3ut) sync | 4 | 2 | — |
| 2026-07-15 | `fc38e49c` | fhr_sim_v2: fix reactor-power oscillation via delayed-neutron... | 1,422 | 261 | — |
| 2026-07-15 | `0ade90a5` | outram-park-digital-twin-engine: scaffold htgr_sim_v1 on the ... | 1,538 | 0 | — |
| 2026-07-15 | `3e2652bb` | beads: reconcile Dolt <-> JSONL export (union of session + re... | 60 | 4 | — |
| 2026-07-15 | `7035aae6` | htgr_sim_v1: wire kinetics slot to the real teh_o_prke::Delay... | 76 | 200 | — |
| 2026-07-15 | `8a59f7f3` | GeN-Foam completion: SP3, SN, thermalHydraulics (one-phase+BC... | 7,807 | 107 | — |
| 2026-07-15 | `b7ec19f0` | kovan-cli: 'kovan setup' bootstrap for curated CLI tools; git... | 417 | 10 | — |
| 2026-07-15 | `46194ba6` | teh-o-prke: implicit backward-Euler delayed-neutron precursor... | 229 | 80 | — |
| 2026-07-15 | `0acc13f0` | nee_soon: Xin Wang thesis -> markdown + njoy->openmc->genfoam... | 1,600 | 4 | — |
| 2026-07-15 | `4d57e0b7` | kovan-cli: add gitoxide (gix) to 'kovan setup' tool list — pu... | 9 | 0 | — |
| 2026-07-15 | `d3e48c24` | njoy groupr: self-shielded (Bondarenko-dilution) MGXS + URR P... | 2,766 | 11 | — |
| 2026-07-15 | `7be2c412` | beads: self-shielded MGXS op-bsz closed; op-6tz.6.3 advanced;... | 1 | 0 | — |
| 2026-07-15 | `fc1cafc6` | digital-twin-engine: thread-panic "please restart" modal for ... | 514 | 33 | — |
| 2026-07-15 | `9bdb5071` | kovan-discovery: git-awareness via gix (library-first, binary... | 1,974 | 0 | — |
| 2026-07-15 | `2267887f` | beads: sync export (op-4wv crash-modal, op-5v5.7 gix, op-wqk.... | 28 | 12 | — |
| 2026-07-16 | `509c5487` | tuas 0.1.4 + tampines-steam-tables 0.2.2: version bump + chan... | 23 | 6 | — |
| 2026-07-16 | `9f8d2e22` | outram-park-fork-coolprop 0.1.0: GPL-3.0 relicense + honesty ... | 772 | 25 | — |
| 2026-07-16 | `5d481e52` | outram-park-fork-coolprop: exclude upstream_source/CoolProp f... | 5 | 1 | — |
| 2026-07-16 | `88fe52d4` | outram-foam basic/turbulence/appbuilder: flag limitations in ... | 364 | 54 | — |
| 2026-07-16 | `595d8e5e` | outram-park-fork-coolprop: wave-1 property additions (op-kbc.... | 6,675 | 547 | — |
| 2026-07-16 | `bbde28ab` | workspace: gate Android-hostile deps/examples off aarch64-lin... | 366 | 259 | — |
| 2026-07-16 | `56cc5bf7` | bookkeeping: doc-comment pass + README status flags + api.md ... | 19,921 | 9,562 | — |
| 2026-07-16 | `5a0dcd92` | bookkeeping: add "Bookkeeping pass" command to CLAUDE.md; not... | 99 | 4 | — |
| 2026-07-16 | `2aeeb094` | beads: sync export (pflotran op-v6s + wave-1/pki bookkeeping) | 14 | 0 | — |
| 2026-07-16 | `44975542` | bookkeeping: fix workspace doc drift (root README/CLAUDE.md +... | 2,669 | 527 | — |
| 2026-07-16 | `6f2005a3` | beads: bookkeeping housekeeping — retitle stale-named epics, ... | 11 | 10 | — |
| 2026-07-16 | `074c2c8c` | outram-foam-appbuilder-lib: exclude tutorials/ from the packa... | 5 | 1 | — |
| 2026-07-16 | `83805139` | tampines-steam-tables 0.2.3: Edwards blowdown V&V + flashing-... | 1,916 | 10 | — |
| 2026-07-16 | `46b75a6f` | tampines-steam-tables 0.2.4: stabilise HybridAllMach over the... | 108 | 28 | — |
| 2026-07-16 | `03690a7a` | Document TampinesSteamArray solver (derivation + debugging) +... | 1,956 | 19 | — |
| 2026-07-16 | `9b7c1dbf` | beads sync (op-ek2 coolprop port, op-21g.15.7 hybrid stabilit... | 27 | 1 | — |
| 2026-07-16 | `733e888b` | beads: sync export tick | 1 | 1 | — |
| 2026-07-17 | `a2d20251` | outram-mc-libs: tally scoring + multigroup transport — flux_s... | 1,330 | 78 | — |
| 2026-07-17 | `110b7c1a` | outram-foam-mesh: new crate — blockMesh, ideasUnvToFoam, poly... | 5,865 | 3 | — |
| 2026-07-17 | `8fe53754` | beads: sync export tick (op-ax7 mesh) | 5 | 3 | — |
| 2026-07-17 | `36428ef4` | outram-foam-basic-lib: OpenFOAM ASCII dictionary + case I/O (... | 2,947 | 1 | — |
| 2026-07-17 | `f819cc04` | outram-foam-cli: scaffold OpenFOAM-style CLI (per-tool binari... | 350 | 0 | — |
| 2026-07-17 | `59305f2d` | outram-foam-cli: wire tools to the case interface + OpenFOAM ... | 1,532 | 64 | — |
| 2026-07-17 | `74a37d6c` | beads: sync export tick (op-x8x CLI + follow-ups) | 3 | 1 | — |
| 2026-07-17 | `9b4509b1` | Add outram-blender scaffold: Blender-inspired mesh-authoring ... | 2,059 | 0 | — |
| 2026-07-17 | `73fbfb5e` | outram-mc: convert 6 tractable openmc-notebook tests to LIVE ... | 2,560 | 94 | — |
| 2026-07-17 | `9896326b` | outram-blender: keep name, clearly mark as a Blender fork in ... | 22 | 15 | — |
| 2026-07-17 | `e3b33c95` | beads: outram-blender naming decision op-hzs.8 closed | 1 | 0 | — |
| 2026-07-17 | `ed2ba864` | outram-blender: add faer (pure-Rust dense+sparse LA) for futu... | 709 | 2 | — |
| 2026-07-17 | `e9daba36` | workspace: declare wgpu 29.0.3 (matches eframe stack); GUI/GP... | 7 | 0 | — |
| 2026-07-17 | `148f48fb` | outram-blender: feature-gated headless GPU compute module (wg... | 72 | 0 | — |
| 2026-07-17 | `8bd4af03` | outram-blender: real headless GPU compute path + WGSL affine ... | 599 | 22 | — |
| 2026-07-17 | `e3340132` | beads: sync tick (outram-blender gpu op-hzs.10 closed) | 1 | 0 | — |
| 2026-07-17 | `2f74f35b` | outram-mc-libs: optional wgpu GPU compute -- batched pointwis... | 993 | 0 | — |
| 2026-07-17 | `e37407b3` | njoy-outram-park-fork: optional wgpu GPU compute (target-gate... | 744 | 0 | — |
| 2026-07-17 | `894aec60` | outram-blender: replace unsafe block_on with safe Wake-based ... | 22 | 30 | — |
| 2026-07-17 | `273ae7c1` | beads: sync tick (njoy/outram-mc/blender wgpu fleets) | 1 | 0 | — |
| 2026-07-17 | `8e5d1e58` | beads: reconcile Dolt<->JSONL (union of session + work-PC bea... | 36 | 10 | — |
| 2026-07-17 | `be4279d7` | beads: auto-export working post-reconcile (benchmark fleet be... | 1 | 2 | — |
| 2026-07-17 | `dfe65ee3` | outram-mc: GPU-vs-CPU benchmarks (Godiva k_eff + HIGH-fidelit... | 1,867 | 2 | — |
| 2026-07-17 | `2b0aed5b` | beads: GPU benchmark fleet (op-nx0/op-6tz.37 results, op-h23 ... | 3 | 3 | — |
| 2026-07-17 | `6b125f58` | outram-mc: add ComputeType backend selector (single/multi/GPU... | 1,082 | 16 | — |
| 2026-07-17 | `71b63e50` | beads: ComputeType/GPU-in-transport (op-u6s.4 progress, op-u6... | 3 | 2 | — |
| 2026-07-17 | `33ed8fbe` | beads: sync tick (GPU + sod fleets in flight) | 3 | 1 | — |
| 2026-07-17 | `73fa32b2` | outram-foam-appbuilder: auto-emit plottable Sod shock tube CS... | 417 | 0 | — |
| 2026-07-17 | `eacdbde1` | beads: sod CSV agent (op-czx) | 5 | 1 | — |
| 2026-07-17 | `3fbf67b1` | outram-mc: deepen GPU penetration — native union grid + event... | 2,766 | 37 | — |
| 2026-07-17 | `a61c95d4` | beads: GPU deep-transport (op-u6s.7 honest no-crossover, op-u... | 1 | 1 | — |
| 2026-07-17 | `f6345c6a` | njoy gpu: full-fidelity WMP Faddeeva pole-sum on GPU (WGSL) +... | 1,532 | 19 | — |
| 2026-07-17 | `00662c58` | beads: njoy Faddeeva GPU (op-0m5 closed, op-0nh f32-accuracy ... | 7 | 0 | — |
| 2026-07-17 | `3a136d0e` | docs: document the GPU f32/CPU f64 precision-vs-performance t... | 56 | 0 | — |
| 2026-07-17 | `cf05bd7c` | beads: sync tick (gpu collision fleet in flight) | 6 | 6 | — |
| 2026-07-17 | `29248fa6` | outram-mc: move MC collision physics onto the GPU (op-u6s.8) | 2,583 | 17 | — |
| 2026-07-17 | `13cfd9c1` | beads: GPU collision physics op-u6s.8 closed; op-u6s.9 crosso... | 2 | 1 | — |
| 2026-07-17 | `c4e44116` | docs: split Singlish mode into SINGLISH_MODE.md with a mainta... | 91 | 23 | — |
| 2026-07-17 | `b326ff31` | docs(singlish): log 'can can' (short ack) + 'ho sei bo' (Hokk... | 8 | 1 | — |
| 2026-07-17 | `6b02f884` | docs(singlish): 'ho sei bo' is standalone greeting; 'ur side ... | 7 | 1 | — |
| 2026-07-17 | `caa0a95e` | outram-blender: implement mesh operators, subdivision, boolea... | 3,460 | 213 | — |
| 2026-07-17 | `fde1543f` | beads: blender mesh impl (op-hzs.1/.2/.4/.5 closed; .3/.6/.7 ... | 10 | 5 | — |
| 2026-07-17 | `7b293846` | outram-foam-mesh: implement snappyHexMesh snapping + layers +... | 3,242 | 148 | — |
| 2026-07-17 | `8c7d2069` | beads: outram-foam-mesh snappy (op-ax7.2.1 closed; .2.2 parti... | 8 | 4 | — |
| 2026-07-20 | `f0ff16d8` | outram-blender: compile wgpu unconditionally on desktop, grac... | 172 | 41 | — |
| 2026-07-20 | `aca40421` | gitignore: exclude Claude Code agent worktrees (.claude/workt... | 5 | 0 | — |
| 2026-07-20 | `84fd281b` | outram-blender: add robust predicates + point-in-mesh classif... | 1,459 | 0 | — |
| 2026-07-20 | `30ee7e02` | Gitignore vendored NJOY2016 reference build tree | 5 | 0 | — |
| 2026-07-20 | `84ff9913` | njoy(op-6tz.6.4): delayed-group MGXS collapse + live mdgxs-pa... | 433 | 10 | — |
| 2026-07-20 | `2c34042b` | outram-foam: implement all turbulence models + advance mesh p... | 3,603 | 454 | — |
| 2026-07-20 | `dbe27541` | outram-blender: general mesh boolean (union/difference/non-co... | 1,184 | 83 | — |
| 2026-07-20 | `6368be8d` | beads: close op-hzs.14 (uv_sphere inward-winding fixed in pri... | 2 | 1 | — |
| 2026-07-20 | `99061770` | outram-foam: regenerate docs/api.md mirrors for turbulence + ... | 4,538 | 108 | — |
| 2026-07-20 | `e6731d69` | njoy(op-cjw.16): LEAPR frequency-integral V&V vs NJOY2016 (H-... | 86 | 0 | — |
| 2026-07-20 | `ce41d4ee` | vv-data: add repo-tracked reference-ENDF folder (outside crat... | 49 | 0 | — |
| 2026-07-20 | `b9df5cc2` | outram-blender: CSG export — cylinder fitting, convex-faceted... | 466 | 20 | — |
| 2026-07-20 | `c17e1bd2` | njoy(op-cjw.16): validate full LEAPR pipeline (T_eff + Debye-... | 102 | 1 | — |
| 2026-07-20 | `5c124ce6` | docs(beads): migrate agent guidance from Go beads/Dolt to bea... | 55 | 30 | — |
| 2026-07-20 | `f45cb03f` | Scaffold outram-park-fork-pflotran crate (PFLOTRAN fork, epic... | 1,202 | 0 | — |
| 2026-07-20 | `88d0e18b` | outram-foam-mesh: fix public-doc intra-doc links to private i... | 17 | 17 | — |
| 2026-07-20 | `9ceb04b3` | outram-blender: rounded (multi-segment) vertex bevel | 283 | 47 | — |
| 2026-07-20 | `53b910a8` | outram-foam-mesh: sync docs/api.md with intra-doc-link fixes | 10 | 10 | — |
| 2026-07-20 | `09ddef75` | beads: reconcile after develop merge — close completed njoy b... | 3 | 3 | — |
| 2026-07-20 | `01a673f3` | njoy(op-cjw.1): ENDF MF=31/33 covariance-section reader (ERRO... | 493 | 13 | — |
| 2026-07-20 | `6442bb34` | njoy(op-3ut): wire stounr URR tape-locate + getsig PENDF feed... | 343 | 46 | — |
| 2026-07-20 | `6f12c640` | outram-blender: real-type export bridges to outram-foam-basic... | 285 | 15 | — |
| 2026-07-20 | `c2f6bf01` | outram-blender: update Cargo.lock for optional export-bridge ... | 2 | 0 | — |
| 2026-07-21 | `4355620c` | docs(CLAUDE): make native-Termux compilation a HARD RULE (and... | 32 | 11 | — |
| 2026-07-21 | `4bb949ac` | fix(outram-mc): gate godiva_gpu_benchmark example off Android... | 46 | 0 | — |
| 2026-07-21 | `c4b39491` | docs(CLAUDE): harden Android/Termux rule — all-targets check ... | 27 | 11 | — |
| 2026-07-21 | `ae10b541` | chore(beads): untrack issues.jsonl compat export, ignore per-... | 10 | 336 | — |
| 2026-07-21 | `dc295db0` | docs(CLAUDE): terminal apps (CLI/ratatui TUI) are IN scope fo... | 10 | 3 | — |
| 2026-07-21 | `bd9e5bff` | feat(outram-mc): scaffold stochastic-media research track (CL... | 1,251 | 3 | — |
| 2026-07-21 | `cdc19bad` | refactor(outram-mc): move stochastic-media research track to ... | 85 | 41 | — |
| 2026-07-21 | `7acc79ce` | refactor(outram-mc): rename pebble_beds::stochastic_media to ... | 53 | 37 | — |
| 2026-07-21 | `380d897a` | outram-mc(op-eby.2): implement classical CLS flight driver | 182 | 27 | — |
| 2026-07-21 | `9966d218` | outram-mc(op-eby.3): implement SCLS transport driver with ret... | 335 | 23 | — |
| 2026-07-21 | `40187700` | outram-mc(op-eby.7): RSA vs CLS vs SCLS absorption benchmark ... | 306 | 0 | — |
| 2026-07-21 | `29345c3a` | outram-mc(op-eby.5): dependency-free kd-tree spatial-index ba... | 246 | 35 | — |
| 2026-07-21 | `8ec0e0b0` | outram-mc(op-eby.6): adaptive variable-radius SCLS extension | 148 | 1 | — |
| 2026-07-21 | `590a5c41` | docs: add rhoPimpleFoam common-misconceptions catalogue | 455 | 0 | — |
| 2026-07-21 | `4d295cef` | docs: backport F5 (converged residuals do not imply correctness) | 34 | 0 | — |
| 2026-07-21 | `5d92be3b` | outram-mc: TRISO stochastic-media tutorial example (CLS/SCLS ... | 158 | 0 | — |
| 2026-07-21 | `5b3fbc84` | outram-mc: GPU CLS/SCLS TRISO tutorial (wgpu, CPU-referenced) | 959 | 0 | — |
| 2026-07-21 | `2539f12c` | outram-mc: TRISO four-method discrepancy tutorial (surface/de... | 554 | 0 | — |
| 2026-07-21 | `28764b62` | outram-mc: rework GPU CLS/SCLS tutorial to reuse the stochast... | 54 | 45 | — |
| 2026-07-21 | `dbeaa7a7` | docs(tuas): cite the peer-reviewed TUAS journal article (jand... | 28 | 0 | — |
| 2026-07-21 | `9d1e3530` | docs(tuas): maintainer clears the V&V bookkeeping axis (CIET-... | 2 | 2 | — |
| 2026-07-21 | `fd8797cf` | added derivation | 639 | 0 | — |
| 2026-07-22 | `bd548a5b` | pflotran: wire foam-basic-lib dep + scaffold module skeleton ... | 38 | 7 | — |
| 2026-07-22 | `c18b3538` | docs(pflotran): add AI-directed-decisions review log | 89 | 0 | — |
| 2026-07-22 | `eedda1f6` | pflotran v1: implement grid, properties, io, Newton-Krylov so... | 4,676 | 13 | — |
| 2026-07-22 | `c8f36dd5` | pflotran: implement RICHARDS flow mode end-to-end (op-v6s.8) | 920 | 111 | — |
| 2026-07-22 | `bcb6cf8e` | pflotran: full test pyramid (verification/integration/regress... | 478 | 20 | — |
| 2026-07-22 | `1e997148` | pflotran: hydrostatic gravity verification + worked infiltrat... | 149 | 0 | — |
| 2026-07-22 | `fab428f3` | docs(pflotran): record v1 completion status + human-review ba... | 35 | 0 | — |
| 2026-07-22 | `c010db65` | pflotran: Celia-1990 validation beads + mass-conservation dia... | 157 | 0 | — |
| 2026-07-22 | `971b0497` | pflotran: Haverkamp (1977) constitutive model for Celia bench... | 326 | 3 | — |
| 2026-07-22 | `f50e1605` | pflotran: scaffold transport module skeleton (op-v6s.11) | 8 | 0 | — |
| 2026-07-22 | `51089321` | docs(pflotran): record transport translation decisions (D11) ... | 29 | 0 | — |
| 2026-07-22 | `00938324` | pflotran: conservative solute transport + RICHARDS flux coupl... | 905 | 11 | — |
| 2026-07-22 | `df9deead` | pflotran: scaffold energy (TH) + geochemistry module skeleton... | 17 | 0 | — |
| 2026-07-22 | `70f939ca` | pflotran: thermal water + rock properties for TH mode (op-v6s... | 527 | 0 | — |
| 2026-07-22 | `aa1875ab` | pflotran: aqueous equilibrium speciation core (op-v6s.12) + m... | 703 | 5 | — |
| 2026-07-22 | `cbf051cf` | pflotran: TH energy transport + coupling; geochem compile fix... | 894 | 16 | — |
| 2026-07-22 | `2e521c62` | pflotran: scaffold reactive_transport module (op-v6s.12 coupl... | 8 | 0 | — |
| 2026-07-22 | `bd805612` | pflotran: block multi-DOF Newton-Krylov solver (op-v6s.4.1) | 1,201 | 0 | — |
| 2026-07-22 | `b75903c8` | pflotran: scaffold multiphase module skeleton (op-v6s.13) | 8 | 0 | — |
| 2026-07-22 | `6381ddc3` | pflotran: two-phase (air-water) multiphase flow on the block ... | 1,017 | 6 | — |
| 2026-07-22 | `9c5c261a` | pflotran: reactive-transport coupling (transport + geochemist... | 962 | 6 | — |
| 2026-07-22 | `6346cd3c` | pflotran: kinetic mineral geochemistry on foam ODE solver (op... | 784 | 0 | — |
| 2026-07-22 | `fa94b421` | docs(pflotran): update lib.rs module map for all implemented ... | 18 | 14 | — |
| 2026-07-22 | `51d97556` | foam-basic-lib: TVD flux limiters translated from OpenFOAM li... | 225 | 0 | — |
| 2026-07-22 | `3ad750d1` | pflotran: TVD advection in transport via foam-basic-lib FluxL... | 126 | 1 | — |
| 2026-07-22 | `d17e0b4a` | docs(pflotran): README What-exists-today reflects all transla... | 23 | 18 | — |
| 2026-07-22 | `19365a36` | pflotran: CPU parallelism via rayon (op-v6s.14) | 128 | 81 | — |
| 2026-07-22 | `f9b62e9a` | pflotran: optional wgpu GPU addon, Android-gated + CPU fallba... | 319 | 0 | — |
| 2026-07-22 | `edf8f34a` | docs(pflotran): record parallelism decision (D2: rayon + Andr... | 19 | 13 | — |
| 2026-07-22 | `ceb30480` | docs(valuation): add 2026-07-22 token-count update | 63 | 0 | — |
| 2026-07-22 | `e0d72a89` | docs(valuation): add without-AI replacement cost section | 36 | 0 | — |
| 2026-07-22 | `f24a53f4` | boon-lay: Phase 0 — buffer-CLT failure analysis + first-passa... | 509 | 0 | — |
| 2026-07-23 | `aa4101f6` | njoy-tui: mobile-first touchscreen JANIS-like nuclear-data TU... | 2,071 | 0 | — |
| 2026-07-23 | `8c6d8575` | boon-lay: Phase 1 — CPU Walk-on-Spheres first-passage engine | 386 | 18 | — |
| 2026-07-23 | `db2d15f8` | boon-lay: Phase 2 — multilayer interface transmission/reflection | 403 | 45 | — |
| 2026-07-23 | `f46b51aa` | outram-mc-tui: mobile-first touchscreen geometry/run TUI (op-... | 2,138 | 0 | — |
| 2026-07-23 | `ae84a047` | boon-lay: Phase 3 — decay + transmutation depletion coupled t... | 369 | 0 | — |
| 2026-07-23 | `b883b9ba` | boon-lay: Phase 4 — CRP-6 kernel release + interface V&V records | 374 | 25 | — |
| 2026-07-23 | `5e82733f` | boon-lay: Phase 5 — rayon parallel ensemble + wgpu compute sc... | 642 | 2 | — |
| 2026-07-23 | `f47c55d2` | boon-lay: Phase 6 — real-time Walk-on-Spheres TRISO diffusion... | 273 | 0 | — |
| 2026-07-23 | `0aee140f` | outram-mc: wire ComputeType dispatch into CSG + delta drivers... | 678 | 43 | — |
| 2026-07-23 | `9224884a` | Move njoy-tui + outram-mc-tui into their library crates as fe... | 155 | 102 | — |
| 2026-07-23 | `933517b0` | pflotran: add tampines-steam-tables dep for real-EOS module (... | 5 | 0 | — |
| 2026-07-23 | `b9007cb2` | pflotran: aqueous activity coefficient models (op-v6s.15.1) | 342 | 0 | — |
| 2026-07-23 | `4d4583f2` | pflotran: real IAPWS water EOS + radioactive decay chains (op... | 1,102 | 0 | — |
| 2026-07-23 | `fb5b3e30` | pflotran: equilibrium sorption — isotherms + Gaines-Thomas io... | 732 | 0 | — |
| 2026-07-23 | `e6f19f18` | docs(pflotran): add activity/sorption/decay/eos_real to modul... | 7 | 0 | — |
| 2026-07-23 | `7e20770f` | outram-blender: cotangent/uniform Laplacian + implicit Laplac... | 615 | 6 | — |
| 2026-07-23 | `9bf8a798` | pflotran: wire radioactive decay into solute transport (op-v6... | 59 | 1 | — |
| 2026-07-23 | `8c678c66` | pflotran: wire linear sorption retardation into transport (op... | 97 | 5 | — |
| 2026-07-23 | `8f1e9668` | pflotran: microbial (Monod) biodegradation reactions (op-v6s.... | 853 | 0 | — |
| 2026-07-23 | `3ffe7ecf` | pflotran: wells/advanced-BCs and real-deck parser modules (op... | 1,768 | 2 | — |
| 2026-07-23 | `7e3fa96f` | pflotran: log D14 upstream-parity wave (op-v6s.15.*) in AI-de... | 53 | 0 | — |
| 2026-07-23 | `0e2336d9` | pflotran: Pitzer ion-interaction activity model for brines (o... | 499 | 0 | — |
| 2026-07-23 | `9475ecbc` | examples: graceful ENDF-fetch failure in the two HIGH-fidelit... | 55 | 20 | — |
| 2026-07-23 | `480048bd` | pflotran: unstructured finite-volume grid (TPFA) module (op-v... | 728 | 0 | — |
| 2026-07-23 | `0a3cac0e` | pflotran: CO2 (Redlich-Kwong) + NaCl brine EOS module (op-1y6) | 750 | 0 | — |
| 2026-07-23 | `ed715c76` | pflotran: surface-complexation sorption (NEM/CCM/diffuse-laye... | 936 | 0 | — |
| 2026-07-23 | `a589fcd1` | pflotran: update D14 log — second parity fleet landed (224 li... | 16 | 2 | — |
| 2026-07-23 | `9cce0b86` | outram-blender: Taubin (lambda\|mu) shrinkage-free smoothing | 172 | 2 | — |
| 2026-07-23 | `a89f3c6a` | outram-park-fork-liggghts: new crate + Phase 1 particle frame... | 749 | 0 | — |
| 2026-07-23 | `9002e7a6` | outram-blender: harmonic/Tutte planar parameterization | 404 | 1 | — |
| 2026-07-23 | `e5697b89` | outram-foam-multiphase: new crate + Stage 1 drift-flux founda... | 1,051 | 0 | — |
| 2026-07-23 | `057d8fe6` | outram-blender: fix parameterization test determinism + ill-c... | 26 | 9 | — |
| 2026-07-23 | `cc8fe274` | pflotran: GENERAL air-water-energy + two-way buoyancy TH modu... | 2,833 | 0 | — |
| 2026-07-23 | `69671cfb` | pflotran: update D14 log — coupled-physics wave (general_mode... | 32 | 6 | — |
| 2026-07-23 | `e08fecfe` | outram-park-fork-liggghts: correct licensing — LIGGGHTS-PUBLI... | 62 | 59 | — |
| 2026-07-23 | `27cd2101` | outram-mc: search_for_keff critical-search driver + activate ... | 694 | 0 | — |
| 2026-07-23 | `15324e84` | outram-blender: ARAP (as-rigid-as-possible) surface deformation | 533 | 0 | — |
| 2026-07-23 | `735c9e2b` | outram-blender: README — document the sparse-solve operator s... | 11 | 6 | — |
| 2026-07-23 | `e9e549b3` | pflotran: robust convecting-regime solve for thermal_convecti... | 201 | 29 | — |
| 2026-07-23 | `5340a9be` | outram-blender: QEM mesh decimation (Garland-Heckbert simplif... | 638 | 0 | — |
| 2026-07-23 | `c9be655d` | njoy: activate mgxs-part-ii scatter-matrix + Chi tests; fix m... | 390 | 52 | — |
| 2026-07-23 | `5f0cb62f` | outram-blender: Loop subdivision (triangle subdivision surface) | 297 | 0 | — |
| 2026-07-23 | `95fce783` | outram-blender: refresh stale lib.rs top-doc (whole crate is ... | 16 | 10 | — |
| 2026-07-23 | `a12db7bd` | feat(tooling): per-commit API-token accounting hooks + docs l... | 462 | 0 | 4,240,534 |
| 2026-07-23 | `74377bf2` | outram-park-mpi: new crate — MPICH-subset shared-memory MPI, ... | 1,180 | 0 | — |
| 2026-07-23 | `e3ef6f5a` | fix(tooling): token ledger report — use git %x1f/%x1e format ... | 27 | 6 | 6,222,248 |
| 2026-07-23 | `e7fb2d3e` | chore(docs): refresh token-usage ledger | 2 | 1 | 1,457,487 |
| 2026-07-23 | `605e5126` | outram-blender: 3D convex hull from a point set | 402 | 0 | — |
| 2026-07-23 | `50cc6f69` | outram-park-mpi: collectives + reduction ops, milestone 2 (op... | 553 | 19 | — |
| 2026-07-23 | `3dcc6bad` | outram-park-mpi: add Cargo.lock entry for the new crate | 7 | 0 | — |
| 2026-07-23 | `349d87fe` | outram-blender: exercise the sparse-solve operators in the me... | 88 | 1 | — |
| 2026-07-23 | `9cf651c8` | scripts: add kloc_accounting.py, reproducible line-count acco... | 1,436 | 0 | — |
| 2026-07-23 | `0b1e867b` | outram-blender: weld / remove-doubles operator (merge coincid... | 400 | 3 | — |
| 2026-07-23 | `43c259c2` | outram-blender: fill-holes operator (cap open boundary loops ... | 259 | 4 | — |
| 2026-07-23 | `a1ea9de1` | outram-blender: solidify operator (shell thickness -> closed ... | 243 | 5 | — |
| 2026-07-23 | `e273ef27` | outram-foam Phase II: multiphase Stages 2-5 + DEM Phases 2-5 ... | 6,714 | 20 | — |
| 2026-07-23 | `95fbeda8` | outram-blender: recalculate-normals operator (consistent wind... | 303 | 5 | — |
| 2026-07-23 | `4de61200` | scripts: net TUAS against what it imported, not its predecess... | 41 | 9 | — |
| 2026-07-23 | `ad95279c` | outram-blender: triangulate operator (ngon/quad Mesh -> trian... | 149 | 3 | — |
| 2026-07-23 | `3e65b02f` | outram-blender: inset-faces operator (per-face inset ring) | 197 | 5 | — |
| 2026-07-23 | `ea2643ea` | outram-blender: bisect operator (plane cut / half-space clip) | 236 | 5 | — |
| 2026-07-23 | `62b7f22e` | outram-blender: demo the repair/modeling/cutting operators in... | 69 | 0 | — |
| 2026-07-23 | `79dc7870` | outram-blender: revolve / spin operator (surface of revolution) | 228 | 5 | — |
| 2026-07-23 | `d3b505c8` | outram-blender: read polyMesh (from_poly_mesh) — round-trip p... | 113 | 8 | — |
| 2026-07-23 | `db91d175` | docs(singlish): add 'bang gang' — knock off work / finish for... | 1 | 0 | 48,295,948 |
| 2026-07-24 | `2f596938` | feat(historian): pre-merge-to-main release report generator (... | 482 | 0 | — |
| 2026-07-24 | `6c50cdd2` | refactor(tooling): consolidate token accounting into docs/his... | 450 | 397 | 7,215,668 |
| 2026-07-24 | `a75a6947` | outram-park-fork-dwsim-libs: port compressor, heater, cooler,... | 2,112 | 0 | — |
| 2026-07-24 | `05f449c7` | outram-park-fork-liggghts: DEM engine + rolling/cohesion, mes... | 3,322 | 0 | — |
| 2026-07-24 | `db924dd3` | Add outram-park-fork-cfmesh scaffold: vendored cfMesh + voro+... | 962 | 0 | — |
| 2026-07-24 | `744b7326` | outram-blender: STL import/export (ASCII + binary) | 351 | 0 | — |
| 2026-07-24 | `93e78474` | outram-park-fork-dwsim-libs: Tier-1 thermodynamics kernel (EO... | 3,554 | 0 | — |
| 2026-07-24 | `035bfaa7` | outram-blender: edge bevel (chamfer every edge) | 248 | 0 | — |
| 2026-07-24 | `b195d347` | docs(readme): add a (clearly-in-jest) Phua Chu Kang tagline | 5 | 0 | 6,734,604 |
| 2026-07-24 | `071e4ff3` | outram-park-fork-cfmesh: volume-mesh core + Cartesian block m... | 526 | 15 | — |
| 2026-07-24 | `7693155e` | outram-mc: port remaining CSG quadric surfaces — Plane, X/YCy... | 507 | 4 | 51,484,074 |
| 2026-07-24 | `fc7c55ff` | chore(docs): refresh token-usage ledger | 15 | 7 | 3,281,946 |
| 2026-07-24 | `3e60439b` | chore(docs): refresh token-usage ledger (hooks bypassed to se... | 2 | 1 | — |
| 2026-07-24 | `13040a88` | outram-park-fork-cfmesh: castellated Cartesian surface carve ... | 375 | 39 | — |
| 2026-07-24 | `1222b187` | outram-park-fork-dwsim-libs: compose thermo kernel — PT/PH fl... | 1,898 | 0 | — |
| 2026-07-24 | `6e35ca51` | outram-park-fork-liggghts: bonded-particle model + pebble-bed... | 1,193 | 0 | — |
| 2026-07-24 | `9b8fdf00` | outram-park-fork-cfmesh: boundary snapping (staircase -> body... | 255 | 7 | — |
| 2026-07-24 | `1f4dd31f` | outram-foam-multiphase: drift-flux PIMPLE pressure-velocity c... | 722 | 0 | — |
| 2026-07-24 | `d3ac4381` | outram-park-fork-cfmesh: foam PolyMesh bridge -> solvable FvM... | 160 | 14 | — |
| 2026-07-24 | `dc627ed4` | outram-park-fork-cfmesh: mesh quality checks (polyMeshGenChec... | 228 | 0 | — |
| 2026-07-24 | `347af8af` | outram-park-fork-cfmesh: multi-region carve (region between s... | 169 | 93 | — |
| 2026-07-24 | `59d744f6` | outram-park-fork-cfmesh: triangle-soup shape generators for r... | 217 | 0 | — |
| 2026-07-24 | `f7c5f0d9` | outram-park-fork-cfmesh: coolant-around-pebble end-to-end exa... | 80 | 0 | — |
| 2026-07-24 | `839d7aae` | outram-park-mpi: communicator dup + split (op-wor) | 333 | 16 | — |
| 2026-07-24 | `a143f0e2` | outram-mc: fix nested-lattice surface-tracking under-count (o... | 95 | 1 | 35,914,153 |
| 2026-07-24 | `c3ffa192` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `9bd10015` | pflotran: MPI domain decomposition + halo exchange, first sli... | 321 | 0 | — |
| 2026-07-24 | `1b9c2849` | outram-park-fork-cfmesh: per-surface boundary patch separation | 83 | 19 | — |
| 2026-07-24 | `21adbfb0` | outram-park-fork-cfmesh: bbox-culled multi-hole carve + pebbl... | 100 | 7 | — |
| 2026-07-24 | `2c7c7738` | outram-foam-multiphase: two-fluid Euler-Euler shared-pressure... | 1,007 | 0 | — |
| 2026-07-24 | `67eb5112` | outram-park-fork-cfmesh: reactor geometry generators + LWR pi... | 206 | 30 | — |
| 2026-07-24 | `46b4ccf9` | outram-mc: port Torus{X,Y,Z} CSG surfaces — completes the Ope... | 689 | 4 | 40,092,252 |
| 2026-07-24 | `f10a0007` | outram-mc: HexLattice 3-D axial rings + X-orientation round-t... | 0 | 809 | 8,028,654 |
| 2026-07-24 | `6a97906c` | outram-park-fork-cfmesh: OpenFOAM polyMesh disk writer + MSR ... | 84 | 0 | — |
| 2026-07-24 | `78f56b85` | outram-park-fork-dwsim-libs: saturation, transport, EOS varia... | 2,971 | 0 | — |
| 2026-07-24 | `9f67449f` | outram-mc: add the lattice/ dir files (complete f10a000) | 1,072 | 0 | 6,064,023 |
| 2026-07-24 | `c849d272` | chore(docs): refresh token-usage ledger | 4 | 1 | — |
| 2026-07-24 | `5d8b5fff` | pflotran: HDF5 snapshot I/O via pure-Rust hdf5-pure (op-v6s.1... | 328 | 0 | — |
| 2026-07-24 | `738b04d8` | outram-park-fork-cfmesh: octree near-wall refinement with pol... | 368 | 0 | — |
| 2026-07-24 | `0ab7d2b0` | chore: pin transitive kstring to 2.0.2 for rustc 1.94 MSRV | 2 | 2 | 2,508,572 |
| 2026-07-24 | `41052a62` | docs: regenerate api.md mirrors for dwsim-libs, multiphase, l... | 22,747 | 1 | 1,165,500 |
| 2026-07-24 | `fed0bf34` | chore(docs): refresh token-usage ledger | 2 | 1 | 2,321,347 |
| 2026-07-24 | `f2a83c42` | chore(docs): flush token-usage ledger lag | 2 | 1 | — |
| 2026-07-24 | `01eb0573` | outram-park-mpi: groups + Cartesian topologies (op-er2) | 450 | 3 | — |
| 2026-07-24 | `9edd1c4a` | pflotran: distributed conjugate-gradient solve (parallel Kryl... | 249 | 1 | — |
| 2026-07-24 | `b8cfd2ea` | outram-park-fork-cfmesh: multi-level octree refinement + 2:1 ... | 134 | 48 | — |
| 2026-07-24 | `7d321ed9` | pflotran: 2-D Cartesian distributed solve (op-gj5, first slice) | 439 | 0 | — |
| 2026-07-24 | `3dc85bc5` | cfmesh: add prism boundary layers (add_boundary_layers) | 393 | 3 | 189,149 |
| 2026-07-24 | `784f727c` | pflotran: generic distributed CG + real variable-coefficient ... | 318 | 0 | — |
| 2026-07-24 | `aac6a3c3` | pflotran: distributed Jacobi-preconditioned CG (op-gj5) | 148 | 1 | — |
| 2026-07-24 | `efb8acdf` | cfmesh: add polyhedral (median) dual — one cell per vertex (p... | 326 | 2 | 5,250,656 |
| 2026-07-24 | `e579dd79` | cfmesh: V&V — polyhedral dual bridges to a solvable foam FvMesh | 20 | 0 | 2,974,882 |
| 2026-07-24 | `3f5b3740` | docs: refresh generated token-usage ledger | 17 | 1 | 891,335 |
| 2026-07-24 | `56eba243` | docs: refresh generated token-usage ledger (lag row) | 2 | 1 | 754,285 |
| 2026-07-24 | `bacb3bbd` | docs: refresh generated token-usage ledger (final lag row) | 2 | 1 | — |
| 2026-07-24 | `d654f153` | pflotran: distributed solve of the REAL assembled LduMatrix (... | 297 | 0 | — |
| 2026-07-24 | `35a4de88` | outram-foam-appbuilder-lib: reactingTwoPhaseEulerFoam applica... | 26,646 | 1,045 | 26,210,679 |
| 2026-07-24 | `f4624b1a` | chore(docs): flush token-usage ledger lag | 15 | 1 | — |
| 2026-07-24 | `bfbc356a` | pflotran: distributed BiCGStab for the non-symmetric transpor... | 217 | 1 | — |
| 2026-07-24 | `22ea3bc3` | pflotran: truly-distributed per-rank LduMatrix assembly (op-gj5) | 106 | 0 | — |
| 2026-07-24 | `2e11d614` | docs: refresh generated token-usage ledger (merge lag row) | 9 | 1 | — |
| 2026-07-24 | `d3f02751` | njoy + outram-mc: make ratatui unconditional — TUI bins alway... | 67 | 90 | 32,670,485 |
| 2026-07-24 | `af6a88b1` | chore(docs): refresh token-usage ledger | 2 | 1 | — |
| 2026-07-24 | `483fd608` | chore(tooling): untrack docs/token-usage.md — commit trailers... | 20 | 47 | 15,508,820 |
| 2026-07-24 | `7e3201bd` | pflotran: distributed transport timestep matching the real se... | 213 | 0 | — |
| 2026-07-24 | `18671477` | outram-foam-appbuilder-lib: reacting-Euler species transport ... | 914 | 13 | 20,596,688 |
| 2026-07-24 | `bac6da5c` | chore(docs): flush token-usage ledger lag | 12 | 1 | — |
| 2026-07-24 | `e597fb89` | outram-foam-multiphase: bookkeeping pass (doc sync, api.md re... | 48 | 10 | 6,293,774 |
| 2026-07-24 | `477c9ff4` | outram-park-fork-liggghts: bookkeeping pass (doc sync, api.md... | 389 | 147 | 3,990,081 |
| 2026-07-24 | `fddb1a93` | outram-foam-appbuilder-lib: bookkeeping pass (doc gaps + stal... | 331 | 54 | 5,605,508 |
| 2026-07-24 | `f4c3aaba` | outram-park-fork-dwsim-libs: bookkeeping pass (doc gaps + hon... | 1,282 | 410 | 6,102,931 |
| 2026-07-24 | `e33b8e74` | chore(docs): flush token-usage ledger lag | 5 | 1 | — |
| 2026-07-24 | `590b0e0c` | fix(outram-mc): correct hex-lattice ring→tile fill (op-6tz.38) | 280 | 12 | 46,781,908 |
| 2026-07-24 | `69d88742` | fix(outram-mc): compose reflective-corner reflections in one ... | 296 | 3 | 7,850,956 |
| 2026-07-24 | `0a34f095` | kovan-literature: add open nuclear digital-twin/shadow litera... | 491 | 0 | 43,506,855 |
| 2026-07-24 | `6d609074` | kovan-literature: lock down verified citations in DT/shadow r... | 61 | 33 | 3,345,634 |
| 2026-07-24 | `9a7b5819` | kovan-literature: add inline verification-status marker to ev... | 36 | 3 | 6,177,955 |
| 2026-07-24 | `e27d65ef` | kovan-literature: reframe DT/shadow review around where Outra... | 90 | 13 | 17,889,996 |
| 2026-07-24 | `9d86853f` | cfmesh: add tetrahedralization (centroid subdivision) — op-hz... | 258 | 2 | 11,477,957 |
| 2026-07-24 | `b2cece56` | pflotran: distributed transport for non-uniform flow (op-gj5) | 187 | 33 | — |
| 2026-07-24 | `f063e119` | pflotran: distributed transport with Dirichlet boundary condi... | 162 | 3 | — |
| 2026-07-24 | `fa4449c3` | feat(outram-mc): GPU ray-surface distance kernel for all 15 C... | 1,712 | 0 | 21,646,800 |
| 2026-07-24 | `834b7399` | cfmesh: face-minimal merged dual + smart-Laplacian quality sm... | 512 | 11 | 11,490,536 |
| 2026-07-28 | `52164176` | pflotran: BiCGStab restart-on-breakdown + distributed TVD tra... | 265 | 34 | — |
| 2026-07-28 | `cb391324` | cfmesh: flip-based Delaunay improvement (2-3/3-2 bistellar fl... | 482 | 3 | 27,713,840 |
| 2026-07-28 | `ce4bd82e` | outram-foam-appbuilder-lib: V&V cases mirroring upstream drif... | 595 | 0 | 82,163,381 |
| 2026-07-28 | `3eb67313` | pflotran: distributed energy (heat) transport timestep (op-gj5) | 145 | 0 | — |
| 2026-07-28 | `c025bf53` | pflotran: distributed Newton solver for nonlinear systems (op... | 278 | 0 | — |
| 2026-07-28 | `a942f819` | outram-blender: Monte Carlo simulation backend (materials + r... | 500 | 14 | 28,954,759 |
| 2026-07-28 | `0f8778d3` | outram-blender/sim: add cell flux/nu-fission tally helpers | 49 | 0 | 3,920,399 |
| 2026-07-28 | `c73aa1b1` | docs(CLAUDE.md): hard rule — agent-fleet progress updates eve... | 27 | 0 | 24,913,510 |
| 2026-07-28 | `e3f5fa1e` | outram-blender: MC Studio — egui GUI to author + run basic ou... | 444 | 0 | 13,126,085 |
| 2026-07-28 | `bb48f281` | Remove unused distributed_dot import in decomposition::ldu | 1 | 1 | — |
| 2026-07-28 | `001cc2c7` | docs(CLAUDE.md): hard rule — dogfood KOPITIAM in this workspace | 53 | 0 | 7,736,570 |
| 2026-07-28 | `dc3ef82c` | chore: keep KOPITIAM out of the OUTRAM PARK tree | 24 | 0 | 4,567,509 |
| 2026-07-28 | `4098a82e` | digital-twin-engine: flow tracers, per-cell pipes, coupled HT... | 2,297 | 249 | — |
| 2026-07-28 | `9d51ac2d` | docs(CLAUDE.md): hard rule — kopitiam is binary-only, issues ... | 23 | 13 | 1,820,695 |
| 2026-07-28 | `5465ff48` | docs(CLAUDE.md): stop hook authorises push to feature/develop... | 12 | 1 | — |
| 2026-07-28 | `e67d8b0b` | outram-mc: fixed-source transport driver (run_fixed_source) —... | 349 | 6 | 28,233,775 |
| 2026-07-28 | `442def8f` | fix(pflotran): restore distributed_dot import at test scope | 3 | 0 | 9,363,777 |
| 2026-07-28 | `7ab5c3c6` | cfmesh: Mesh Studio — egui GUI for polyhedral meshes + layers... | 447 | 0 | 24,536,996 |
| 2026-07-28 | `a52636a8` | cfmesh: adaptive boundary layers for curved walls — op-zhh (M... | 293 | 4 | 24,949,768 |
| 2026-07-28 | `3f5b191e` | build: add async-opcua to workspace dependencies (Android-ver... | 23 | 0 | 31,544,530 |
| 2026-07-28 | `dfc66b00` | docs: reword async-opcua note as scope, not a user restriction | 10 | 5 | 1,876,572 |
| 2026-07-28 | `6d41e60d` | boon-lay: CPU/GPU ComputeType resource switcher + off-thread ... | 994 | 2 | — |
| 2026-07-28 | `57bab46f` | boon-lay: first_passage_realtime — off-thread compute + CPU/G... | 240 | 134 | — |
| 2026-07-28 | `efa78282` | boon-lay: triso_simulator retrofit — WoS diffusion + CPU/GPU ... | 147 | 8 | — |
| 2026-07-28 | `b90452dd` | ciet: CIET Educational Simulator v2 with an OPC-UA interface ... | 24,723 | 7 | 417,150 |
| 2026-07-28 | `174e837d` | release: outram-park-digital-twin-engine 0.1.0 -> 0.2.0 | 3 | 3 | 8,734,716 |
| 2026-07-29 | `4345e350` | docs: scope the transformation to a Type I digital twin | 291 | 0 | 166,955,527 |
| 2026-07-29 | `eaac02dd` | docs(dt-scoping): simulator binaries may live in this repo | 36 | 17 | 9,014,289 |
| 2026-07-29 | `e8027702` | docs: scope the OFFBEAT + SCIANTIX port in detail | 188 | 0 | 29,874,580 |
| 2026-07-29 | `0ca34c9e` | offbeat: new crate + P0 mechanics bridgehead (epic op-6sl, be... | 1,704 | 0 | 190,671,075 |
| 2026-07-29 | `23f4d02e` | offbeat: module stubs for the remaining port phases + Cargo.lock | 214 | 0 | 2,272,071 |
| 2026-07-29 | `3723c06c` | offbeat P3/P4: material property correlations, burnup, fast f... | 12,503 | 18 | 11,883,986 |
| 2026-07-29 | `4bc06269` | offbeat P3: behavioural models -- swelling, densification, re... | 3,516 | 73 | 5,424,301 |
| 2026-07-29 | `b1c308ea` | offbeat P1: rheology -- plasticity and creep constitutive law... | 3,683 | 2 | 16,420,596 |
| 2026-07-29 | `e3053e3e` | offbeat P2: fuel/cladding gap -- conductance, gas mixture, co... | 5,467 | 2 | 2,931,872 |
| 2026-07-29 | `4b2dfb31` | offbeat P5: cladding corrosion, hydrogen pickup, Anderson mix... | 5,034 | 2 | 7,540,360 |
| 2026-07-29 | `a3342f86` | docs: list outram-park-fork-offbeat in the workspace member t... | 2 | 0 | 2,320,510 |
| 2026-07-29 | `60d8ac54` | offbeat: add the missing LICENSE, NOTICE and README | 876 | 0 | 43,035,292 |
| 2026-07-30 | `4fe73072` | kovan-literature: correct thesis metadata extraction, add The... | 582 | 20 | — |
| 2026-07-30 | `b92e997f` | kovan-literature: archive three open-access UC Berkeley theses | 6,031 | 0 | — |
| 2026-08-03 | `6aa61def` | release: patch-bump 8 crates for the digital-twin publish chain | 31 | 28 | 0 |
| 2026-08-03 | `a0e79725` | release: placeholder 0.0.1 versions, authorship, and shipped ... | 4,298 | 19 | 6,940,322 |
| 2026-08-03 | `ad614348` | docs: rewrite the crates.io publishing procedure from the 202... | 61 | 25 | 15,060,617 |
| 2026-08-03 | `57991e41` | dwsim-fork: complete chemistry-model survey from upstream source | 217 | 8 | 85,709,336 |
| 2026-08-03 | `0dbe9a14` | dwsim-fork: note HTGR water-ingress relevance in chemistry su... | 26 | 0 | 10,094,049 |
| 2026-08-03 | `b7d5baf1` | dwsim-fork: port SLE flash, Gibbs speciation, electrolyte tie... | 2,892 | 0 | 45,403,356 |
| 2026-08-03 | `6f192e3c` | dwsim-libs: port reaction + reactor models (op-tts) | 1,908 | 0 | 11,902,529 |
| 2026-08-03 | `6835f3d7` | dwsim-libs: port advanced EOS (op-b4t) + inside-out & 3-phase... | 2,998 | 0 | 1,604,329 |
| 2026-08-03 | `12396872` | dwsim-libs: Gibbs reactor + Langmuir-Hinshelwood catalytic ki... | 792 | 7 | 12,187,552 |
| 2026-08-03 | `1ed9c121` | dwsim-libs: port 5 thermo tail modules (op-qo2.10/.13/.14/.19... | 4,482 | 0 | 3,384,663 |
| 2026-08-03 | `14140ac4` | dwsim-libs: port SVLLE + Modified-UNIFAC-Dortmund + UNIFAC-LL... | 1,851 | 0 | 15,669,333 |
| 2026-08-03 | `22fbd81a` | cfmesh: high-level tet-dual meshing pipeline (op-0xu) | 588 | 0 | 3,503,482 |
| 2026-08-03 | `6185c177` | dwsim-libs: port sour-water package (op-qo2.16) | 1,123 | 0 | 5,376,231 |
| 2026-08-03 | `cdece4f3` | outram-blender: tet-dual Mesh Studio GUI + cfmesh bridge (op-... | 767 | 0 | 5,337,056 |
| 2026-08-03 | `ef6ed0ba` | dwsim-libs: port multi-phase (N-phase) Gibbs minimisation (op... | 1,442 | 0 | 2,458,474 |
| 2026-08-03 | `451bdf92` | chore: update Cargo.lock for outram-park-fork-cfmesh workspac... | 1 | 0 | 1,859,355 |
| 2026-08-03 | `4b8acdc3` | chore: bump versions of crates changed this session (+0.0.1 e... | 12 | 12 | 8,012,763 |
| 2026-08-03 | `b1e7b77d` | cfmesh: bookkeeping pass — doc/README refresh for the tet-dua... | 57 | 3 | 12,878,348 |
| 2026-08-03 | `93f232c3` | outram-mc-libs: bookkeeping pass — document fixed-source MC d... | 42 | 4 | 0 |
| 2026-08-03 | `1150d2d0` | outram-blender: bookkeeping pass — document MC Studio + Mesh ... | 94 | 29 | 2,235,684 |
| 2026-08-03 | `559cd2fa` | dwsim-libs: bookkeeping pass — module map, honest scope, surv... | 219 | 99 | 15,228,568 |
| 2026-08-03 | `dd1fc4a2` | workspace: bookkeeping — refresh member tables + stale scaffo... | 39 | 7 | 0 |
| 2026-08-03 | `8622e24f` | docs: regenerate api.md rustdoc mirrors for the 4 changed crates | 50,075 | 6,137 | 6,048,735 |
| 2026-08-04 | `84938054` | msre: scaffold outram-park-fork-{onix,thermochimica,moltres} ... | 272 | 0 | 108,134,040 |
| 2026-08-04 | `601ad91b` | outram-park-fork-onix: port ONIX CRAM depletion solver (op-6w... | 1,699 | 9 | 25,605,061 |
| 2026-08-04 | `b33218c0` | outram-park-fork-thermochimica: port Thermochimica GEM core (... | 1,470 | 0 | 10,783,765 |
| 2026-08-04 | `e082e5d8` | outram-park-fork-moltres: circulating-fuel MSR multiphysics o... | 3,124 | 9 | 7,004,597 |
| 2026-08-04 | `023e0da3` | outram-foam-basic-lib: add OpenFOAM patch-field boundary cond... | 736 | 10 | 33,272,447 |
| 2026-08-04 | `325ba295` | outram-foam-basic-lib: functional cyclic (periodic) patches (... | 898 | 21 | 20,204,442 |
| 2026-08-04 | `5423b54a` | outram-foam-basic-lib: cyclicAMI non-conformal periodic patch... | 1,057 | 25 | 15,489,304 |
| 2026-08-04 | `5c7ccbdf` | outram-foam-basic-lib: add OpenFOAM flow boundary conditions ... | 674 | 23 | 14,040,264 |
| 2026-08-04 | `5a434fcc` | outram-blender: ship the GPL licence text with the crate | 674 | 0 | 6,048,938 |
| 2026-08-04 | `58b56de6` | outram-blender: vendor Blender provenance, ship upstream lice... | 493 | 2 | 14,249,477 |
| 2026-08-04 | `5de01232` | release: outram-blender 0.0.1 -> 0.0.2 | 3 | 3 | 5,019,031 |
| 2026-08-04 | `9b42d09f` | release: outram-blender 0.0.3 — correct the ported-code prove... | 85 | 32 | 3,999,157 |
| 2026-08-04 | `1edb3a92` | docs: scope a MELCOR-class severe-accident capability | 255 | 0 | 32,124,212 |
| 2026-08-04 | `f726eb01` | outram-foam-appbuilder-lib: restore the build — exhaustive Bo... | 58 | 0 | 816,770 |
| 2026-08-04 | `b44f5491` | widget studio: gallery app, plus a turbine widget driven by r... | 773 | 25 | 1,229,721 |
| 2026-08-04 | `3f3dedbe` | added scoping for aster and turbine/pipe | 684 | 0 | 28,557,592 |
| 2026-08-04 | `aa7078e0` | boon-lay: fix WGSL shader parse failure — `target` is a reser... | 7 | 3 | 6,023,992 |
| 2026-08-04 | `01e0deb5` | docs: scope the code_aster constitutive-law and fracture port | 73 | 34 | 27,393,538 |
| 2026-08-04 | `1de6660e` | turbine widget: stator rows, casing, per-stage internals, sym... | 632 | 45 | 28,726,364 |
| 2026-08-04 | `2b347382` | turbine widget: blade lean adjustments, stator ring, unused c... | 16 | 8 | 11,976,418 |
| 2026-08-04 | `64a4394c` | widget studio: pipes tab over three flow backends, and a HEM ... | 412 | 23 | 22,714,599 |
| 2026-08-04 | `d2b1cebc` | basic-lib: port OpenFOAM 3x3 eigen decomposition; start code_... | 5,680 | 3 | 81,012,341 |
| 2026-08-04 | `cdc362f5` | style: rustfmt basic-lib files that landed unformatted | 366 | 132 | 504,280 |
| 2026-08-04 | `f2721679` | offbeat: code_aster P0 kinematics -- Mandel Voigt + finite st... | 1,472 | 107 | 17,390,337 |
| 2026-08-04 | `abbf86db` | offbeat: code_aster P1 local integration algorithms (op-a7p.2) | 990 | 0 | 21,151,406 |
| 2026-08-04 | `12e3b054` | offbeat: pin the secant observed-order oscillation (op-a7p.2) | 39 | 0 | 2,998,996 |
| 2026-08-05 | `13b67a38` | offbeat: GDEF_LOG finite-strain wrapper + fix a real eigen bu... | 803 | 4 | 26,049,479 |
| 2026-08-05 | `7bd4eac0` | offbeat: code_aster P2 isotropic viscoplastic creep -- NORTON... | 781 | 0 | 22,775,222 |
| 2026-08-05 | `05123c46` | offbeat: code_aster LEMAITRE_IRRA irradiation creep (op-a7p.3) | 340 | 0 | 16,115,712 |
| 2026-08-05 | `3bca0018` | dwsim-libs: scope the remaining UNIFAC ports (NIST-Modified U... | 235 | 0 | 264,264,032 |
| 2026-08-05 | `03279cf6` | outram-foam: port the fvOptions/fvModels mechanism + solidifi... | 1,538 | 0 | 42,669,145 |
| 2026-08-05 | `1aefdfc8` | Port DWSIM seawater, hydrocarbon, immiscible & black-oil pack... | 3,567 | 4 | 7,958,648 |
| 2026-08-05 | `ae45c4e3` | code_aster P2(a): NORTON_HOFF + isotropic hardening radial re... | 1,219 | 0 | 19,039,444 |
| 2026-08-05 | `9d179094` | code_aster: mid-flight checkpoint of the chaboche/metallurgy/... | 8,787 | 0 | 7,124,139 |
| 2026-08-05 | `bf88c814` | code_aster: checkpoint 2 -- chaboche and fracture revisions | 512 | 286 | 3,661,705 |
| 2026-08-05 | `6463f521` | code_aster: land the fracture and Chaboche ports, wire re-exp... | 223 | 96 | 8,550,871 |
| 2026-08-05 | `c70be991` | code_aster: checkpoint the damage/rupture port (op-a7p.4) | 4,414 | 8 | 2,762,621 |
| 2026-08-05 | `57a02488` | code_aster: land the damage/rupture port (op-a7p.4), wire re-... | 133 | 7 | 5,688,444 |
| 2026-08-05 | `30dce1ae` | code_aster: land the metallurgy port, complete the fleet, ref... | 41 | 3 | 4,275,610 |
| 2026-08-05 | `17c628d4` | docs: bookkeeping pass over offbeat and basic-lib | 34,558 | 713 | 36,209,328 |
| 2026-08-05 | `830a3874` | docs: restore the astest V&V oracle, correct scoping section 7 | 45 | 25 | 9,591,507 |
| 2026-08-05 | `db9f16a3` | docs: correct section 7 -- comp0* is not the oracle I claimed | 33 | 10 | 11,143,584 |
| 2026-08-05 | `19ae058f` | docs: identify ssnv101a as the entry point for astest verific... | 42 | 6 | 8,526,284 |
| 2026-08-05 | `f42b6c88` | code_aster: first astest verification -- Chaboche reproduces ... | 416 | 0 | 16,008,252 |
| 2026-08-05 | `eedd4042` | docs: ssnv113a is unusable -- missing material include, and w... | 14 | 0 | 15,313,203 |
| 2026-08-05 | `f895dad5` | astest harness: mixed strain/stress control + DEFI_FONCTION/D... | 569 | 0 | 21,998,464 |
| 2026-08-05 | `00a38e4d` | code_aster: upstream's own tolerances corroborate the VENDOCH... | 0 | 0 | 15,246,459 |
| 2026-08-05 | `3c471b26` | astest: wire ssnv126a -- VENDOCHAB does NOT yet reproduce ups... | 457 | 0 | 13,409,118 |
| 2026-08-05 | `2e9a4e7b` | astest ssnv126a: correct the diagnosis -- one root cause, not... | 23 | 11 | 10,731,988 |
| 2026-08-05 | `3a1cde97` | damage: fix a saturation test that fired on every step (VENDO... | 204 | 100 | 25,352,024 |
| 2026-08-05 | `efbf1230` | astest ssnv126a: sub-step convergence study -- first order, a... | 74 | 14 | 16,508,302 |
| 2026-08-05 | `946b12e4` | PipeVisual: rectangles sized from real geometry, per-cell box... | 357 | 33 | 12,858,918 |
| 2026-08-05 | `da0cd23a` | PipeVisual: white rectangle tracers crossing each run in exac... | 196 | 14 | 14,605,033 |
| 2026-08-05 | `85ea8d4d` | pipes: single pulsed tracer, and a wall drawn from TUAS's pre... | 315 | 32 | 15,769,406 |
| 2026-08-05 | `ef18cf5b` | aster: unified isotropic hardening curve (op-fxp, step 1 of 2) | 643 | 0 | 61,056,670 |
| 2026-08-05 | `8cd80dd4` | CHECKPOINT: agent fleet in flight -- ODE enum wrapper and mec... | 1,631 | 28 | 9,056,674 |
| 2026-08-05 | `6e2e66f5` | pipes: cell dividers as wall metal, and a less squat helium run | 110 | 4 | 6,854,328 |
| 2026-08-05 | `9c03a291` | pipes: helium run to 12 m, canvas scrolls so length stays tru... | 28 | 6 | 3,771,955 |
| 2026-08-05 | `f6e00912` | offbeat: Zircaloy Poisson validation case (op-6sl.7) -- block... | 3,186 | 30 | 6,861,279 |
| 2026-08-05 | `d1ab6696` | CHECKPOINT: VISCOCHAB tests and README, agents still in flight | 101 | 59 | 6,959,251 |
| 2026-08-05 | `18712612` | CHECKPOINT: merge develop; viscochab.rs still under edit | 1 | 2 | 6,229,603 |
| 2026-08-05 | `8573dcb0` | offbeat/foam: VISCOCHAB, ODE enum dispatch, rheology wiring, ... | 3,681 | 1,961 | 14,965,941 |
| 2026-08-05 | `bb52a814` | offbeat: fracture is not blocked on finite elements -- correc... | 125 | 51 | 2,308,589 |
| 2026-08-05 | `456ad77f` | offbeat: refuse the Zircaloy Poisson crossover (op-6sl.7); fi... | 181 | 11 | 8,835,761 |
| 2026-08-05 | `3b88deb0` | bedok: stage-1 Rust translation of Than Yan Ren's coupled nod... | 25,924 | 0 | 71,600,043 |
| 2026-08-05 | `a96e98cf` | tampines: 1-D drift-flux solver (op-dt3.12); six-equation lef... | 3,827 | 5 | 30,484,127 |
| 2026-08-05 | `d106ec66` | bedok: add the crate README, with the defect register front a... | 134 | 0 | 2,076,020 |
| 2026-08-05 | `c55917b6` | CHECKPOINT: melt_foam solver loop in flight, agent still writing | 535 | 0 | 6,232,846 |
| 2026-08-05 | `4f0bf810` | bedok: record that benchmark sources are cited, not republished | 16 | 0 | 5,926,180 |
| 2026-08-05 | `08029e65` | CHECKPOINT: melting V&V probe scaffold, agent still writing | 368 | 1 | 3,943,751 |
| 2026-08-05 | `67a3105a` | multiphase: port OpenFOAM's interfacial heat-transfer closure... | 1,263 | 179 | 9,811,393 |
| 2026-08-05 | `39aaea07` | CHECKPOINT: melting V&V cases filled in (334 -> 971 lines), a... | 72 | 2 | 5,643,822 |
| 2026-08-05 | `04618b1c` | kovan-tui: interactive PDF ingestion, with mandatory metadata... | 3,004 | 49 | 10,407,067 |
| 2026-08-05 | `769517e5` | outram-foam-basic-lib: solidification/melting fvOptions (from... | 1,443 | 5 | 234,154,623 |
| 2026-08-05 | `7fa6a3de` | outram-foam-multiphase: interfacial heat-transfer closures (f... | 500 | 0 | 0 |
| 2026-08-05 | `bec668b6` | outram-foam-appbuilder-lib: melt_foam solver and melting V&V ... | 1,556 | 0 | 0 |
| 2026-08-05 | `a9ae462a` | tampines: 1-D drift-flux multiphase solver (from claude/outra... | 2,383 | 0 | 0 |
| 2026-08-05 | `43d4246a` | Cargo.lock: regenerate for the tampines multiphase dependencies | 1 | 0 | 2,285,010 |
| 2026-08-05 | `1199b8fb` | CHECKPOINT: melt_foam tests + References.md land; merge devel... | 553 | 109 | 3,080,702 |
| 2026-08-05 | `96631482` | appbuilder: melt_foam solver loop + Stefan/gallium V&V cases | 8 | 6 | 5,381,722 |
| 2026-08-05 | `d686901d` | outram-foam-basic-lib: document TemperatureTable / Solidifica... | 24 | 17 | 16,225,640 |
| 2026-08-05 | `8f0d914f` | outram-foam-appbuilder-lib: melt_foam solver loop + Stefan/ga... | 536 | 97 | 85,444 |
| 2026-08-05 | `2e085b22` | outram-foam-basic-lib: fix README table split by a stray blan... | 0 | 1 | 2,737,143 |
| 2026-08-06 | `1d03089e` | color_maps: vendor Crameri's Scientific colour maps (MIT), fr... | 520 | 0 | 46,053,354 |
| 2026-08-06 | `9c31adc4` | components: grade temperature blue -> white -> red with Crame... | 92 | 30 | 14,950,932 |
| 2026-08-06 | `6773284c` | components: colour-to-temperature legend in the studio's righ... | 622 | 1 | 17,326,105 |
| 2026-08-06 | `ccad9150` | pipes: hook the arrays up — Pipe::step implemented, PipeCompo... | 2,097 | 104 | 98,238,537 |
| 2026-08-06 | `bfd72cc0` | components: smooth pipe bends, with a live-angle demonstratio... | 1,294 | 7 | 28,326,815 |
| 2026-08-06 | `504b0b09` | pipe bend: signed turn angle spanning -180 to +180 | 202 | 38 | 27,767,512 |
| 2026-08-06 | `ebbde1b6` | outram-mc GPU: make the WGSL surface-distance kernel agree wi... | 546 | 76 | 11,886,857 |
| 2026-08-06 | `8ce09292` | docs: reactor scoping slate — six reactor types with coupled ... | 1,468 | 0 | 9,692,180 |
| 2026-08-06 | `a6f49cd6` | engine: promote the FHR reactor vessel widget into the shared... | 986 | 936 | 16,772,032 |
| 2026-08-06 | `fcf2e541` | engine: promote the temperature buttons, delete the duplicate... | 369 | 1,190 | 19,407,929 |
| 2026-08-06 | `511cc9a4` | CLAUDE.md: complete the Members table, dogfood kopi-beans, pi... | 162 | 32 | — |
| 2026-08-06 | `ee9c7b18` | CLAUDE.md: resolved kopitiam issues move to docs/kopitiam-iss... | 55 | 4 | 7,217,855 |
| 2026-08-06 | `d5b0b638` | engine: reactor-vessel art for all six scoped reactors, and m... | 1,082 | 24 | 36,463,152 |
| 2026-08-06 | `500dac1a` | turbine: flip the stator lean so nozzle and bucket oppose eac... | 82 | 10 | 11,151,998 |
| 2026-08-06 | `be49edb9` | engine: the FHR archetype now renders the real fhr_sim_v2 ves... | 61 | 42 | 10,581,826 |
| 2026-08-06 | `a2f54143` | engine: FHR vessel keeps its proportions at any size, plus pe... | 347 | 4 | 17,603,654 |
| 2026-08-06 | `9be87c63` | engine: HTR-10 vessel artwork from the IAEA cross-section, in... | 2,931 | 43 | 22,612,771 |
| 2026-08-06 | `880a9bd6` | htr-10 vessel: randomly packed pebble bed, following the cone... | 201 | 37 | 12,021,493 |
| 2026-08-06 | `10c13a64` | pebble artwork: TRISO speckle instead of one smooth hot sphere | 371 | 13 | 25,773,647 |
| 2026-08-06 | `6f5380f4` | kovan: ingest the MSRE design and operations report (ORNL-TM-... | 29,522 | 0 | 3,718,293 |
| 2026-08-06 | `e25544e2` | reference-data: vendor the NRIC/INL Virtual Test Bed (CC-BY-4... | 562,796 | 0 | 3,830,761 |
| 2026-08-06 | `07a1952f` | triso: many more kernel dots per pebble | 9 | 9 | 16,449,134 |
| 2026-08-06 | `4f0d1f74` | triso: crank the kernel density up again, plus the RAVEN port... | 1,110 | 8 | 3,337,523 |
| 2026-08-06 | `86f85f83` | triso: derive the kernel count from an explicit 80% fill target | 42 | 9 | 6,794,297 |
| 2026-08-06 | `452c26b8` | triso: dial the fill target back to 65% | 5 | 5 | 6,859,598 |
| 2026-08-06 | `77fada9a` | triso: fill target down to 55% | 4 | 4 | 4,136,538 |
| 2026-08-06 | `8f48e587` | raffles: new crate — Risk Analysis Framework For Learning & E... | 1,480 | 0 | 5,546,737 |
| 2026-08-06 | `c3985f6a` | docs: what the vendored Virtual Test Bed actually gives us | 269 | 0 | 19,966,715 |
| 2026-08-06 | `f136e901` | pebble bed: a real DEM-settled packing, baked once | 1,649 | 0 | 12,859,820 |
| 2026-08-06 | `43e4ec0e` | raffles: samplers and sensitivity analysis, and a real bug fo... | 2,741 | 151 | 10,011,572 |
| 2026-08-06 | `3263ccac` | raffles: probability distributions (op-vjw.1) | 2,792 | 57 | 13,309,646 |
| 2026-08-06 | `b42df8f1` | widget studio: DEM-packed beds, three steam generators, three... | 4,992 | 248 | 30,059,643 |
| 2026-08-06 | `9f4ff6d4` | rng: fix init_seed to match OpenMC (op-rbo) | 3,361 | 1,344 | 11,697,975 |
| 2026-08-06 | `131f5a93` | docs: fence the torus quartic derivation as text, not Rust (o... | 11 | 3 | 14,425,662 |
| 2026-08-06 | `b16067d4` | outram-mc-libs: record the RNG goal — statistics, not particl... | 33 | 0 | 4,275,739 |
| 2026-08-06 | `dc28d981` | pebble beds: 3-D depth instead of a flat saw-cut | 1,551 | 510 | 56,238,852 |
| 2026-08-06 | `e71f1f97` | rng: port OpenMC's PCG output permutation (op-jis) | 1,861 | 476 | 15,385,625 |
| 2026-08-06 | `99925ce1` | raffles: correct two docs the RNG changes made false | 31 | 13 | 10,999,756 |
| 2026-08-06 | `91153bb8` | docs: fix tampines-steam-tables doctests — a stale API, not a... | 56 | 23 | 8,311,676 |
| 2026-08-06 | `097c0c7e` | hexlattice: record that the op-jis shift is not meaningful | 16 | 0 | 8,355,438 |
| 2026-08-07 | `8c9ddd05` | docs: migrate issue-tracker instructions from beads-rs to kop... | 373 | 235 | 1,037,154 |
| 2026-08-07 | `1dfcd019` | outram-foam-basic-lib: FV/primitive layer work, and a documen... | 4,172 | 522 | 47,249,266 |
| 2026-08-07 | `ee4ac849` | outram-foam-appbuilder-lib: solver-application work, and a re... | 4,244 | 296 | 0 |
| 2026-08-07 | `1f7f815a` | outram-blender: mesh-authoring work, and a provenance contrad... | 1,980 | 117 | 0 |
| 2026-08-07 | `9bab7ad7` | outram-park-fork-cfmesh: meshing work, temp-dir handling, and... | 2,863 | 172 | 0 |
| 2026-08-07 | `3e9c3a53` | outram-foam-mesh: mesh generation/conversion work, and a docu... | 3,941 | 260 | 0 |
| 2026-08-07 | `fa4cbd07` | docs: MELCOR-parity and real-time multi-fidelity scoping | 2,388 | 0 | 531,351 |
| 2026-08-07 | `07aa498a` | docs: complete the migration to kopi-beans; CLAUDE.md said th... | 317 | 111 | 0 |
| 2026-08-11 | `734a5307` | HTR-10 foundations: 23-group decay heat, KTA/ZBS tests, kovan... | 8,372 | 345 | 95,389,999 |
| 2026-08-11 | `89100a93` | dwsim-libs: port the flowsheet, dynamics, column, petroleum a... | 58,347 | 7 | 51,701,732 |
| 2026-08-11 | `5a94616d` | kovan-literature: graph digitiser (engine + CLI, TUI and egui... | 4,297 | 0 | 14,929,678 |
| 2026-08-11 | `a6b19500` | CLAUDE.md: mandate dogfooding the kovan graph digitiser | 40 | 0 | 5,523,494 |
| 2026-08-11 | `44281486` | kovan-literature: librarian pass over the HTR-10 archive | 498 | 2,452 | 49,931,356 |
| 2026-08-11 | `30d72d39` | kovan-literature: visibility is closed by default (op-nv6g) | 90 | 11 | 14,496,734 |
| 2026-08-11 | `e87f4f91` | dwsim-libs: port ShortcutColumn (Fenske-Underwood-Gilliland) | 1,847 | 0 | 3,205,638 |
| 2026-08-11 | `0857f5ea` | CLAUDE.md: any literature ingested OR USED goes into kovan | 31 | 1 | 11,700,922 |
| 2026-08-11 | `c5422e8d` | CLAUDE.md: TUAS natural-circulation tests must run in parallel | 33 | 0 | 4,632,718 |
| 2026-08-11 | `a9f71f01` | chem-eng: relicense to GPL-3.0, O(1) recurrences, and z-domai... | 8,695 | 2,705 | 10,082,614 |
| 2026-08-11 | `4feb6c84` | HTR-10 neutronics spec, (p,h) taper guard, and O(1) transfer ... | 7,136 | 254 | 15,077,007 |
| 2026-08-11 | `4cdb09fe` | chem-eng: make the step-cost regression tests assert a ratio,... | 89 | 57 | 34,160,302 |
| 2026-08-11 | `d6edfd9f` | tampines: correct the false Marviken claims and add a bounds-... | 6 | 0 | 7,297,727 |
| 2026-08-11 | `771cb225` | kovan-literature: catalogue NUREG/CR-2671, the Marviken criti... | 10,131 | 0 | 42,684,750 |
| 2026-08-11 | `c86506e1` | tampines: pebble-bed conduction stack, helium gas layer, and ... | 9,505 | 59 | 5,490,374 |
| 2026-08-11 | `e11989a7` | tampines-steam-tables: fix the IF97 router panic and gate Mar... | 6,248 | 664 | 25,632,699 |
| 2026-08-11 | `b37e79a3` | tampines-steam-tables: make the (p,s) validity check accept t... | 195 | 53 | 41,464,956 |
| 2026-08-11 | `c615dcd6` | kovan-literature: add the missing nrc1982marviken CATALOGUE.m... | 24 | 0 | 353,340 |
| 2026-08-11 | `e0f0568a` | tuas: fix the transposed Reynolds/Prandtl exponents in the Wa... | 330 | 25 | 118,291 |
| 2026-08-11 | `085ceea8` | tampines: drift-flux plenum inflow BC, Marviken V&V, and six-... | 2,916 | 35 | 24,921,232 |
| 2026-08-11 | `67ebcd6c` | outram-mc: give bound nuclides their thermal-elastic channel ... | 1,187 | 48 | 7,339,144 |
| 2026-08-11 | `6ab77bb6` | tampines: document how OpenFOAM regularises the six-equation ... | 1,111 | 0 | 89,380,435 |
| 2026-08-11 | `b1bc9880` | outram-blender: HTR-10 pebble-bed core envelope generator + t... | 461 | 0 | 643,777,063 |
| 2026-08-11 | `e43c4c25` | docs(agents): mark the kopi-beans store-format blocker resolv... | 25 | 11 | 42,821,454 |
| 2026-08-12 | `31471e06` | tampines: interfacial exchange closures for the six-equation ... | 1,914 | 0 | 13,010,751 |
| 2026-08-12 | `d9e3d031` | feat(outram-mc): TRISO nested-shell geometry + Jodrey-Tory hi... | 907 | 0 | 30,999,088 |
| 2026-08-12 | `dfc9dcb0` | outram-foam-basic-lib: declare rayon and target-gated wgpu | 15 | 0 | 6,698,480 |
| 2026-08-12 | `2b2c7c33` | fix(njoy): correct carbon MAT numbers + register C-nat for gr... | 48 | 8 | 12,190,260 |
| 2026-08-12 | `fbad15fd` | outram-foam-basic-lib: ComputeBackend dispatch layer (op-yvj.... | 3,800 | 14 | 16,966,568 |
| 2026-08-12 | `71c2d755` | outram-foam-basic-lib: untrack kernels committed prematurely ... | 2 | 3,149 | 3,131,077 |
| 2026-08-12 | `8141f91d` | feat(njoy): thermal-scattering-law (tsl) acquire/cache path f... | 155 | 4 | 26,166,423 |
| 2026-08-12 | `75de4173` | Cargo.lock: pick up rayon and target-gated wgpu from the foam... | 2 | 0 | 371,274,370 |
| 2026-08-12 | `46827711` | outram-foam-basic-lib: checkpoint the rayon LDU and field ker... | 4,636 | 0 | 10,808,157 |
| 2026-08-12 | `1ef08b8a` | outram-foam-basic-lib: checkpoint agent test refinements (op-... | 10 | 8 | 3,751,036 |
| 2026-08-12 | `4a22e652` | outram-foam-basic-lib: record the measured field-kernel cross... | 35 | 5 | 3,889,191 |
| 2026-08-12 | `121ec112` | outram-foam-basic-lib: bound operator-derived field names (na... | 326 | 12 | 14,220,764 |
| 2026-08-12 | `c6010376` | outram-foam-basic-lib: checkpoint LDU parallel test edits (op... | 2 | 2 | 1,994,540 |
| 2026-08-12 | `e40d6a57` | dwsim: transient (dynamic) rigorous distillation column + V&V | 860 | 0 | 74,990,843 |
| 2026-08-12 | `57378f17` | outram-foam-basic-lib: checkpoint LDU SpMV work in progress (... | 188 | 94 | 3,036,925 |
| 2026-08-12 | `403bd707` | outram-foam-basic-lib: hybrid LDU SpMV and Krylov vecops (op-... | 68 | 25 | 4,864,701 |
| 2026-08-12 | `f5040fc1` | docs: make 19 unpublished beads durable, and file the kopi-be... | 1,152 | 0 | 17,811,843 |
| 2026-08-12 | `9da38d5d` | outram-foam-basic-lib: batched root finding on the hybrid bac... | 3,125 | 2 | 5,850,470 |
| 2026-08-12 | `84663693` | gitignore: aider's virtualenv | 1 | 0 | 58,046,223 |
| 2026-08-12 | `0b132ea2` | tampines: implement the 1-D six-equation two-fluid solver (op... | 4,435 | 104 | 8,878,416 |
| 2026-08-12 | `9c0a3b62` | tampines: correct the multiphase_1d module docs now the six-e... | 15 | 5 | 2,910,845 |
| 2026-08-12 | `878c73ea` | opcua: extract the reactor-agnostic layer out of ciet_opcua (... | 3,229 | 1,993 | 156,958,507 |
| 2026-08-12 | `f7c8a190` | htgr_sim_v1: real HTR-10 pebble-bed core, and a schematic bui... | 2,278 | 611 | 1,951,840 |
| 2026-08-12 | `2a6a43c7` | fhr_sim_v2: make a steam-generator temperature cross impossib... | 2,639 | 86 | 978,188 |
| 2026-08-12 | `43f81104` | htgr: OPC-UA node map, HTR-10 plant data, and two more source... | 4,103 | 1 | 1,960,899 |
| 2026-08-12 | `df8ad9a8` | rustfmt.toml: pin workspace formatting | 38 | 0 | 3,934,222 |
| 2026-08-12 | `dd00f680` | style: apply rustfmt workspace-wide (formatting only, no beha... | 214,067 | 142,186 | 8,903,673 |
| 2026-08-12 | `3cbbb5a8` | blame: ignore the workspace rustfmt pass | 12 | 0 | 991,612 |
| 2026-08-12 | `0d272cf1` | htgr_sim_v1: wire the real htr10 KTA/ZBS module into the plan... | 868 | 238 | 31,734,367 |
| 2026-08-12 | `cf975861` | fhr_sim_v2: measure GUI frame time, so lag reports can be att... | 316 | 0 | 696,152 |
| 2026-08-12 | `079bd1d4` | docs: record kopitiam#19 as resolved -- bn v0.1.3 does publis... | 115 | 0 | 0 |
| 2026-08-12 | `bf2c851a` | htgr_sim_v1: command control rods, not reactivity | 401 | 14 | 24,589,508 |
| 2026-08-12 | `3c9d547a` | docs: refresh the kopi-beans notes against v0.1.3 | 111 | 49 | 0 |
| 2026-08-12 | `1e025135` | htgr_sim_v1: make a steam-side temperature cross unrepresentable | 209 | 10 | 20,022,517 |
| 2026-08-12 | `71d07632` | htgr_sim_v1: automatic scram on measurable trip signals | 487 | 8 | 29,694,384 |
| 2026-08-12 | `0bc2a948` | app_scaffold: name the failing PLANT COMPONENT in crash reports | 493 | 8 | 1,597,888 |
| 2026-08-12 | `e144e0d9` | scripts: make the beads-store push work from any working dire... | 16 | 2 | 9,779,205 |
| 2026-08-12 | `91853c3f` | htgr_sim_v1: make the protection system a toggle, disarmed by... | 77 | 2 | 5,850,719 |
| 2026-08-12 | `541a38ef` | components: bake the pebble bed to a texture, and animate the... | 7,444 | 80 | 3,838,214 |
| 2026-08-12 | `4c36c029` | components: real artwork for the condenser, cooling tower and... | 78 | 36 | 5,705,402 |
| 2026-08-12 | `3543645b` | widget_studio: tabs for the condenser, cooling tower and excu... | 2,462 | 0 | 23,929,049 |
| 2026-08-12 | `5375e272` | docs: htr10.md described the opposite of what the simulator n... | 36 | 7 | 24,055,398 |
| 2026-08-12 | `2e3185dd` | components: an HTGR excursion is not an explosion — rework th... | 1,863 | 1,330 | 2,666,404 |
| 2026-08-12 | `162a08fa` | coolprop: fix two boundary-condition defects in OPCPFluidArray | 604 | 11 | 3,248,926 |
| 2026-08-12 | `c0d63179` | docs: record the human corrections to AI work, and the BC con... | 380 | 0 | 27,251,795 |
| 2026-08-12 | `d9abc556` | tampines-steam-tables: the energy equation's inlet BC was era... | 647 | 2 | 2,972,773 |
| 2026-08-12 | `2cc65f9a` | docs: re-verify the whole HTR-10 scoping audit against curren... | 279 | 84 | 29,015,574 |
| 2026-08-12 | `bf023a61` | components: real artwork for the heat exchanger, plus a studi... | 3,469 | 22 | 1,283,009 |
| 2026-08-12 | `00a776be` | htgr_sim_v1: the turbine rotor now turns, from a real torque ... | 1,056 | 20 | 2,582,852 |
| 2026-08-12 | `62106158` | coolprop: bounded scalar convection, upwind boundaries, and a... | 825 | 34 | 7,177,635 |
| 2026-08-12 | `6c7cee7f` | docs: narrow the sibling-document claim from "audited" to "sp... | 32 | 10 | 4,637,090 |
| 2026-08-12 | `b5313156` | tampines-steam-tables: TUAS upwind terminals, and the conduct... | 1,177 | 32 | 0 |
| 2026-08-12 | `9f36deb3` | CLAUDE.md: hard rule — search the workspace before building a... | 60 | 0 | 16,198,039 |
| 2026-08-12 | `05b20cbd` | fhr_sim_v2: measured the GUI, found nothing to fix, kept the ... | 488 | 264 | 18,836,035 |
| 2026-08-12 | `e7e44b28` | docs: GUI stutter on the workstation is the graphics stack, n... | 73 | 0 | 15,884,436 |
| 2026-08-12 | `10216f03` | docs: record the measured GPU headroom, with its caveats | 20 | 4 | 4,404,582 |
| 2026-08-12 | `edbb2bd7` | htgr_sim_v1: nodalised counter-flow steam generator replaces ... | 2,845 | 170 | 5,165,784 |
| 2026-08-12 | `1d4ac747` | changed numbers and some of the hot and cold side | 14 | 14 | 2,979,170 |
| 2026-08-13 | `d55997e2` | outram-foam-basic-lib: snapshot in-flight golden-section work... | 1,387 | 0 | 19,709,337 |
| 2026-08-13 | `8a832247` | outram-foam-basic-lib: snapshot in-flight ODE and minimisatio... | 5,315 | 41 | 5,983,182 |
| 2026-08-13 | `fb59c7ef` | kovan: ingest ANL-75-55 and Pichler 2020 stainless-steel prop... | 2,109 | 0 | 131,404,259 |
| 2026-08-13 | `f565d31b` | tuas: add SolidMaterial::SteelSS304LHighTemp (Kim ANL-75-55, ... | 1,055 | 1 | 442,112 |
| 2026-08-13 | `1a382230` | outram-foam-basic-lib: snapshot golden-section test additions... | 89 | 15 | 3,453,944 |
| 2026-08-13 | `f6939395` | tuas: give the temperature-range error a payload, and stop mi... | 353 | 90 | 443,636 |
| 2026-08-13 | `3acc9536` | htgr_sim_v1: reach real time (0.492 -> 1.032), and fix the ho... | 3,163 | 321 | 445,294 |
| 2026-08-13 | `4320d056` | workspace: clear all 97 float_literal_f32_fallback warnings (... | 103 | 104 | 896,609 |
| 2026-08-13 | `d54ffe72` | outram-foam-basic-lib: batched golden-section minimisation (o... | 16 | 0 | 12,375,581 |
| 2026-08-13 | `8e97330d` | coolprop: add a selectable implicit energy-balance mode to OP... | 756 | 7 | 10,079,720 |
| 2026-08-13 | `ee15d32a` | outram-foam-basic-lib: checkpoint ODE ensembles, doctest now ... | 104 | 3 | 3,202,375 |
| 2026-08-13 | `68e35551` | htgr_sim_v1: put the steam generator's helium side on implici... | 26 | 0 | 8,059,634 |
| 2026-08-13 | `981e2ccf` | outram-foam-basic-lib: checkpoint ODE ensemble rewrite (op-yv... | 247 | 130 | 5,576,906 |
| 2026-08-13 | `51577766` | outram-foam-basic-lib: batched ODE ensembles and quadrature (... | 104 | 60 | 4,293,471 |
| 2026-08-13 | `0ba10cbe` | outram-foam-basic-lib: snapshot in-flight numerical Jacobians... | 1,997 | 0 | 9,893,912 |
| 2026-08-13 | `c0e85a50` | htgr_sim_v1: cut the steam-generator sub-steps 8 -> 2, reachi... | 285 | 36 | 25,575,685 |
| 2026-08-13 | `e49784bf` | outram-foam-basic-lib: checkpoint Jacobians and the hybrid pa... | 3,748 | 15 | 7,107,909 |
| 2026-08-13 | `f45a7d2a` | outram-foam-basic-lib: cross-cutting hybrid parity gate (op-y... | 1 | 0 | 5,685,700 |
| 2026-08-13 | `45f79ac2` | outram-foam-basic-lib: checkpoint Jacobians, failure-detectio... | 646 | 16 | 3,138,186 |
| 2026-08-13 | `ffebc100` | outram-foam-basic-lib: checkpoint Jacobian test additions (op... | 118 | 56 | 4,752,711 |
| 2026-08-13 | `5e6b25d3` | outram-foam-basic-lib: finite differences and batched Jacobia... | 89 | 0 | 9,161,739 |
| 2026-08-13 | `b3cc5b66` | outram-foam: fix a solver that reported success with a NaN st... | 105 | 19 | 19,365,173 |
| 2026-08-13 | `fded5ea9` | docs: retire the beads-recovery copy -- all 19 are now published | 0 | 1,039 | 6,780,032 |
| 2026-08-13 | `3288145f` | htgr_sim_v1: add selectable temperature-cross remedies (None ... | 5,940 | 0 | 21,455,774 |
| 2026-08-13 | `762a1094` | outram-foam-basic-lib: checkpoint Krylov-on-HybridLdu wiring ... | 1,999 | 79 | 14,434,163 |
| 2026-08-13 | `dbe4c6db` | htgr_sim_v1: drop the eliminate-metal remedy, prefer Yan Ren'... | 108 | 1,385 | 9,353,446 |
| 2026-08-13 | `c5e59839` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 130 | 13 | 7,065,216 |
| 2026-08-13 | `f1259948` | outram-foam-basic-lib: salvage the Krylov agent's last edits ... | 24 | 26 | 5,310,052 |
| 2026-08-13 | `b500c1a0` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 107 | 38 | 3,571,022 |
| 2026-08-13 | `17872fe4` | outram-foam-basic-lib: checkpoint Krylov wiring (op-yvj.4.4) | 163 | 58 | 7,833,462 |
| 2026-08-13 | `c6013078` | htgr_sim_v1: secondary-loop operator controls, degC/K toggle,... | 2,427 | 155 | 32,028,698 |
| 2026-08-13 | `60fb179e` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 482 | 106 | 12,831,072 |
| 2026-08-13 | `65345929` | outram-foam-basic-lib: checkpoint GMRES hybrid wiring (op-yvj... | 11 | 2 | 2,474,822 |
| 2026-08-13 | `947a904d` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 40 | 10 | 4,353,459 |
| 2026-08-13 | `790f6863` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 67 | 17 | 2,497,606 |
| 2026-08-13 | `f76ed92e` | outram-foam-basic-lib: checkpoint BiCGStab hybrid wiring (op-... | 17 | 3 | 3,137,608 |
| 2026-08-13 | `d44d592b` | outram-foam-basic-lib: checkpoint Krylov hybrid tests (op-yvj... | 47 | 24 | 2,519,530 |
| 2026-08-13 | `bd373cc6` | outram-foam-basic-lib: checkpoint preconditioner hybrid wirin... | 13 | 8 | 3,166,210 |
| 2026-08-13 | `d688f7d7` | outram-foam-basic-lib: Krylov solvers on the hybrid backend (... | 11 | 3 | 5,752,956 |
| 2026-08-13 | `2d68f5b5` | checkpoint: op-zwk0 NaN-defect audit of the two duplicated OD... | 226 | 2 | 18,296,809 |
| 2026-08-13 | `c1e0fd35` | checkpoint: op-zwk0 guard in coolprop's duplicated Rosenbrock... | 10 | 0 | 5,985,016 |
| 2026-08-13 | `ce4dd30d` | checkpoint: op-zwk0 NaN guards in both duplicated ODE trees | 354 | 36 | 4,012,774 |
| 2026-08-13 | `9d2f1dbb` | checkpoint: op-zwk0 NaN guards, both trees (+40/-8) | 40 | 8 | 4,718,730 |
| 2026-08-13 | `72fe7518` | docs: record kopi-beans losing beads AFTER acknowledging the ... | 69 | 0 | 19,245,199 |
| 2026-08-13 | `6aa58b2e` | checkpoint: op-uyi3 golden-section de-duplication in steam-ta... | 372 | 96 | 10,592,281 |
| 2026-08-13 | `58a3284a` | checkpoint: op-uyi3 golden-section de-duplication, Zaloudek g... | 500 | 35 | 7,156,851 |
| 2026-08-13 | `31174d8c` | steam-tables: de-duplicate golden section; correct an underst... | 28 | 9 | 7,962,361 |
| 2026-08-13 | `70483022` | steam-tables: stop writing to stderr from production flash pa... | 21 | 11 | 13,148,964 |
| 2026-08-13 | `5e1c988f` | policy: make the working-hours guardrail opt-in per session | 87 | 27 | — |
| 2026-08-13 | `dad05813` | docs(outram-foam-basic-lib): settle the hybrid CPU+GPU precis... | 165 | 0 | 23,362,738 |
| 2026-08-13 | `86c46ffb` | docs(outram-foam-basic-lib): coarse-to-fine supersedes the hy... | 125 | 9 | 2,954,825 |
| 2026-08-13 | `85143f25` | docs: upstream the three outstanding kopi-beans defects | 6 | 0 | 17,510,691 |
| 2026-08-13 | `b3c5835c` | kovan-metrics: port token accounting + historian from Python ... | 2,590 | 24 | 121,773,353 |
| 2026-08-13 | `859f9221` | docs: record the kopi-beans 0.1.6 daemon fix, verified on thi... | 95 | 6 | 6,622,357 |
| 2026-08-13 | `4ecce27a` | docs/historian: delete the Python scripts, kovan-metrics is n... | 29 | 754 | 8,073,261 |
| 2026-08-13 | `cd0fef9d` | kovan-literature: catalogue Terry 2005 + three UC Berkeley th... | 157 | 0 | 31,680,456 |
| 2026-08-13 | `268e44c2` | cfmesh: add face-pyramid-volume and cell-determinant sliver c... | 521 | 0 | 19,131,890 |
| 2026-08-13 | `89e0d47b` | kovan-literature: record the HTR-10 r-z zone geometry read fr... | 312 | 6 | 1,667,472 |
| 2026-08-13 | `ac271965` | kovan-literature: confirm z orientation, start the Fig. 2 zon... | 39 | 3 | 6,773,634 |
| 2026-08-13 | `91dd04d5` | kovan-literature: complete the Fig. 2 bottom row -- the radia... | 35 | 20 | 2,580,140 |
| 2026-08-13 | `cb3647d9` | kovan-literature: Fig. 2 second layer -- the map stops being ... | 51 | 0 | 1,740,390 |
| 2026-08-13 | `9860ae82` | kovan-literature: confirm the one-band spans of zones 46, 55,... | 7 | 7 | 3,951,994 |
| 2026-08-13 | `97c49408` | docs: reconcile upstream kopitiam issue state, and close #19 | 44 | 9 | 50,865,877 |
| 2026-08-13 | `ff0c4b37` | njoy: LEAPR can regenerate graphite S(alpha,beta) from its 12... | 4,530 | 49 | 19,783,743 |
| 2026-08-13 | `53fa574e` | njoy: LEAPR regeneration is now the default source of graphit... | 3,585 | 81 | 34,932,070 |
| 2026-08-13 | `239182a2` | docs(data-acquisition): the cache substrate is no longer feat... | 15 | 0 | 5,573,065 |
| 2026-08-13 | `8a8340cd` | docs: log requested htgr_sim_v1 follow-ups for the next activ... | 63 | 0 | — |
| 2026-08-13 | `47476d13` | outram-foam-mesh: cyclic-aware snapping — periodic seams snap... | 900 | 3 | — |
| 2026-08-13 | `80e6e68f` | docs(outram-foam-basic-lib): fix 24 broken intra-doc links, d... | 56 | 41 | — |
| 2026-08-13 | `0e5e14ea` | htgr_sim_v1: decay heat, a fuel-feedback heat sink, evaluated... | 1,284 | 149 | — |
| 2026-08-13 | `e97850f3` | docs: drop the htgr_sim_v1 follow-up log, now that the work i... | 0 | 63 | — |
| 2026-08-13 | `6a15c02b` | htgr_sim_v1: fix the core-outlet temperature inversion, and m... | 618 | 299 | — |
| 2026-08-13 | `b6623807` | coolprop: exact backward T(rho, h), and record where human re... | 304 | 0 | — |
| 2026-08-13 | `322a6404` | coolprop: seed the (rho, h) backward solve from the fixed-pre... | 193 | 13 | — |
| 2026-08-14 | `52a21fba` | dealt with degC button and the restart button | 928 | 167 | — |
| 2026-08-14 | `e9112581` | added bacon toml | 8 | 0 | — |
| 2026-08-14 | `d4387b59` | legend unit is now Celsius, not kelvin | 10 | 5 | — |
| 2026-08-14 | `fdc04718` | htgr_sim_v1: the corrector loop is not converged, and say so ... | 111 | 7 | 64,241,387 |
| 2026-08-14 | `b0fafebe` | kovan agent-docs-gen: bundle the API docs for an external 200... | 1,631 | 0 | 28,107,942 |
| 2026-08-14 | `20e118b4` | agent-docs-gen: --regenerate-missing never worked; the tools ... | 1,926 | 29 | 11,922,811 |
| 2026-08-14 | `070d3f31` | Retire scripts/gen_api_docs.py: port it to `kovan api-docs` | 327 | 181 | 9,562,759 |
| 2026-08-14 | `b9c72f50` | No Python for docs or accounting (hard rule); retire the aste... | 78 | 472 | 11,101,988 |
| 2026-08-14 | `e6cee0ba` | kloc parity baseline: freeze the Python output before porting it | 321 | 0 | 3,080,340 |
| 2026-08-14 | `364b8b00` | kloc parity baseline: recapture with every repository present | 94 | 31 | 14,795,866 |
| 2026-08-14 | `7f342453` | changed loop to manual | 9 | 2 | 1,920,766 |
| 2026-08-14 | `bfb645fe` | added manual for secondary loop | 1 | 1 | 3,437,244 |
| 2026-08-14 | `5b02a9d4` | added feedwater manual control and edited slider | 4 | 3 | 9,297,662 |
| 2026-08-14 | `68e2e11f` | Retire scripts/kloc_accounting.py: port it to `kovan kloc` | 0 | 1,454 | 33,752,515 |
| 2026-08-14 | `8d996dca` | added kovan metrics | 3,892 | 2 | 5,563,359 |
| 2026-08-14 | `673399fa` | kovan: regenerate the whole doc suite in one command, and fin... | 78,580 | 17,850 | 25,407,865 |
| 2026-08-14 | `7a9d2b56` | added new api docs for each | 142,212 | 0 | 4,171,800 |
| 2026-08-14 | `db242bb2` | added new api | 149,800 | 0 | 0 |
| 2026-08-14 | `dd2682d9` | added anders pdf | 9 | 0 | 0 |
| 2026-08-14 | `b983f838` | README: document the `kovan lit` ingestion workflow and the t... | 54 | 1 | 13,486,507 |
| 2026-08-14 | `b297fea9` | kovan-literature: record the choo2023criticality tier decisio... | 13 | 2 | 5,209,786 |
| 2026-08-14 | `ffef0fb7` | added 4 endf files specific to triso | 439,743 | 0 | 1,309,908 |
| 2026-08-14 | `f90b2350` | changed pathbuf for njoy, pending test | 10 | 3 | 0 |
| 2026-08-14 | `1ace0acd` | added leapr files from endf, from here: https://www.nndc.bnl.... | 10,584 | 0 | — |
| 2026-08-14 | `e4afd517` | njoy-outram-park-fork: embed all 33 ENDF/B-VIII.0 LEAPR decks... | 894 | 132 | 0 |
| 2026-08-14 | `933ee9ca` | outram-mc: route thermal scattering into both k-eigenvalue pa... | 462 | 106 | — |
| 2026-08-14 | `2becc4c9` | outram-mc: TRISO shell S(alpha,beta) composition, and the SiC... | 447 | 18 | — |
| 2026-08-14 | `9591944f` | njoy+outram-mc: light-water thermal scattering, deck to trans... | 1,056 | 43 | 753,784,864 |
| 2026-08-14 | `a2802a35` | pincell: the thermal LWR benchmark is no longer data-gated | 18 | 17 | 84,498,218 |
| 2026-08-15 | `5673aaa2` | Patch-bump the njoy→digital-twin-engine publish pipeline (16 ... | 1,396 | 48 | 16,094,533 |
| 2026-08-15 | `df6f62f1` | scripts: update cargo-publish.sh to the real njoy->digital-tw... | 38 | 11 | 4,989,786 |
