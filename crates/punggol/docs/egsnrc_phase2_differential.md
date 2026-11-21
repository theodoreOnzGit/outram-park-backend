# EGSnrc phase 2: differential tests against compiled upstream (2026-11-21)

~~Status: **partial**.~~ **CORRECTED 2026-11-21: every ported `egsnrc` file the compiled upstream can exercise is now compared; the sections below record the numbers, later ones supersede earlier ones where struck through. What is not compared is listed in "Not differentially tested, and why" (corrected).** (Original status text:) The PEGS4 program, the HATCH tables (photon, electron,
spin, Rayleigh) and a 200-history `tutor1` transport run of `punggol::egsnrc`
have been compared with a built upstream, and the data-file readers have been
run on the real EGSnrc data files. ~~`punggol::openmc_photon` has not (gh:#1690). Everything else in `punggol::egsnrc` and all of `punggol::openmc_photon` is still "first port, not differentially tested"~~ **CORRECTED 2026-11-21**: `punggol::openmc_photon` is compared for U and H, see `openmc_photon_phase2.md`; the further elements need data (gh:#1695).

## How the reference was built (scratch, nothing committed)

EGSnrc `bd49a47a` read from a sparse clone (`HEN_HOUSE/{src,pegs4,mortran3,
pieces,scripts,specs,makefiles,cutils,data}`), never committed here.

1. `gfortran -std=legacy -O1 HEN_HOUSE/mortran3/mortran3.f stub.f -o mortran3.exe`
   (`stub.f` supplies `egs_fdate`, which `machine.F` normally does).
2. `mortran3.exe -s -d mornew77.raw -o7 mortran3.dat` (the macro data base).
3. A hand-written `machine.macros` with what `scripts/configure` writes for
   gfortran on Linux (`$BYTE_ORDER '1234'`, `$RECL-FACTOR 1`, `$CALL_EXIT` as
   `call exit`, `$FLUSH_UNIT` as `call flush`, `$LONG_INT integer*8`,
   `$SHORT_INT integer*2`).
4. `mortran3.exe -d mortran3.dat -f machine.macros pegs4.mortran -o7 pegs4.f`
   then `gfortran -std=legacy -O1 pegs4.f stub.f` (default 72-column fixed
   form; `-ffixed-line-length-132` corrupts the path literals). `stub.f` also
   has `egs_isdir`.
5. `EGS_HOME=... HEN_HOUSE=... pegs4.exe -i <name>` for `pb700`, `h2o700`,
   `tis521` (`HEN_HOUSE/pegs4/inputs/*.pegs4inp`).

The reference is deterministic across compiler settings that do not
reassociate: `-O0` and `-O1` give identical files. `-O3 -ffast-math` changes
the output (see below).

## Comparison

`pegs4_mortran::tests::differential_against_compiled_pegs4`, run with
`PEGS4_REFERENCE_DIR` (the three `.pegs4dat` files) and `PEGS4_DATA_DIR`
(`pgs4pepr.dat`, `pgs4form.dat`, `aprime.data`), compares every number of every
line of the DECK output with the reference. Without the variables it is
skipped.

| case | lines (port / ref) | identical lines | differing | worst `abs(d) / max(abs(line))` |
|---|---|---|---|---|
| `pb700` (ELEM Pb, IRAYL=1) | 3388 / 3388 | 2782 | 606 | 1.7e-4 |
| `h2o700` (COMP H2O, IRAYL=1) | 3741 / 3741 | 3165 | 576 | 3.8e-4 |
| `tis521` (MIXT ICRU tissue, IRAYL=1) | 674 / 674 | 543 | 131 | 2.4e-4 |

What agrees exactly: the line count of all three, the token count of every
line, every header, medium and element record, and the number of fit intervals
(so the fits chose the same breakpoints). What differs: the digits of
single-precision fit coefficients at the 1e-5 to 1e-4 level. The worst
*relative* differences (1.5e-2 on `pb700` line 2919, and entries that are 0 in
the reference and about 1e-5 in the port) sit on near-cancelling coefficients;
the line-normalised column is the meaningful one.

Scale of the effect: rebuilding the reference with `-O3 -ffast-math` moves
`pb700` by 3.8e-4 in the same measure on 2633 lines, so the port's residual is
inside the single-precision sensitivity of the program, but it is **not**
bit-identical to the `-O0/-O1` build and ~~**no cause has been identified**~~ **CORRECTED 2026-11-21: found, f32 `ln`/`exp`/`powf` vs glibc; PEGS4 is now bit-identical, see "PEGS4 residual: `f32` library functions" below**.
Candidates, not tested: the order of single-precision operations in
`mix_pegs4`/`spinit_pegs4`/`pwlf1`, and `f32` `exp`/`ln` in
`petir::mathf::RealMath` against glibc's `expf`/`logf`. This is flagged for the
maintainer: it is above the 1e-9 code-to-code bound and is propagated error of
the `real*4` program, not a physics disagreement.

## The data files ARE in the EGSnrc repository (correction)

~~"The files are not available"~~ **CORRECTED 2026-11-21:** the maintainer does
not have them to hand, but `HEN_HOUSE/data/` of the pinned EGSnrc commit holds
`spinms.data`, `pair_nrc1.data`, `eii_ik.data`, `nist_brems.data`,
`nrc_brems.data`, `triplet.data`, `epdl_{photo,pair,rayleigh,triplet}.data`,
`compton_sigma.data`, `iaea_photonuc.data`, `photo_relax.data`, `photo_cs.data`
(31 MB; a sparse checkout of that directory fetched them). They are AGPL data
and stay in the scratch clone, never committed. Test
`real_hen_house_data_files_when_available` (env `EGSNRC_HEN_HOUSE_DATA`) runs
the readers on them; it checks structure and sanity, not agreement with EGSnrc
output:

- `spinms.data` is exactly `1 + 100*4*16` records of 1104 bytes
  (7,066,704 bytes), header 1 to 100 keV, b2 0.3005 to 1; decodes.
  ~~"not re-checked against a real `spinms.data`"~~: layout confirmed by size
  and header.
- `pair_nrc1.data` is exactly `1 + 100*84` records of 260 bytes (2,184,260
  bytes), `emin 2.01`, `emax 168.6`, `ne 84`, `nb 65`; builds the water tables.
- `triplet.data`, `epdl_*`, `compton_sigma.data`, `iaea_photonuc.data`,
  `eii_ik.data`, `nist_brems.data`, `nrc_brems.data`, `photo_relax.data`,
  `photo_cs.data` parse; Pb binding energies read K 88.0045, L1 15.86, L2 15.20,
  L3 13.04 keV (the K edge matches `EKEDGE(82)` of the PEGS4 run).
- The user-photon tables for water give a no-Rayleigh photon mean free path at
  1 MeV of 14.167 cm against 14.144 cm from 1/(0.0707 cm^2/g) (0.0707 recalled
  from NIST XCOM, not read from the repo): +0.16 %.

## The EGSnrc kernel build and the first kernel-level comparison

The full kernel builds from the same sparse clone: Mortran3 over
`egsnrc.macros machine.macros ranmar.macros <user code> ranmar.mortran
nrcaux.mortran machine.mortran egs_utilities.mortran egsnrc.mortran`
(`machine.mortran` empty; `machine.macros` as above plus `$copy_file`,
`$move_file`, `$remove_directory`, `$COMPILE_TIME`, `$GIT_HASH`, and
`$RECL-FACTOR 4`, which **confirms the 1104-byte `spinms.data` record the port
assumed**: with 1 the real code fails reading element 73), then `gfortran -x
f77-cpp-input -DGIT_HASH=... -DCOMPILE_TIME=...` with a small stub file for the
`egs_*` system routines. `HEN_HOUSE/user_codes/tutor1` (20 MeV electrons on a
1 mm Ta plate, `tutor_data.pegs4dat`) runs and prints per-history exit lists
(~~not yet compared~~ **compared, see "Histories" below**). A copy, `difftab.mortran`, calls `HATCH` and dumps
`GMFP0/1`, `GBR10/11`, `GBR20/21`, `COHE0/1` of the Ta medium.

Test `differential_user_photon_tables_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_PHOTON_TABLES`): `init_user_photon` with the default
`xcom` series, bound Compton and EADL relaxation against that dump
(2000 grid points, `ge0`, `ge1` equal to the last digit):

| build of the reference | gmfp | gbr1 | gbr2 | cohe (fitted function values, worst) | coefficients (worst) |
|---|---|---|---|---|---|
| default flags (`-O1`, no `-fdefault-real-8`) | 2.8e-8 | 2.8e-6 (floor 1e-6) | 1.9e-8 | 9.9e-10 | 3.3e-4 on slopes |
| `-fdefault-real-8 -fdefault-double-8` | 1.1e-15 | 3.0e-16 | 5.9e-16 | 2.4e-16 | 1.7e-12 |

**Cause, verified by the second row:** Mortran3 emits real constants without a
`D` exponent (`1.6605655`, `0.1274783851`, ...), which are *single precision* in
gfortran unless `-fdefault-real-8` is given, so the default upstream build uses
`f32`-rounded constants inside `real*8` arithmetic. The port uses `f64`
literals and agrees with the promoted build to round-off. EGSnrc's own
`gfortran` spec (`specs/gfortran-suse11.4.conf`) sets `FCFLAGS = -fPIC`,
`FOPT = -O3 -ffast-math` and no promotion, so a stock build has the 1e-8
effect. ~~**Maintainer decision needed:** keep `f64` literals or round such constants through `f32`.~~ **DECIDED 2026-11-21 (maintainer):** compare like with like: first a single-precision-constants mode of the port against the stock build, and once that agrees, `f64` constants become the workspace standard, validated against the `-fdefault-real-8` build (gh:#1687). **IMPLEMENTED 2026-11-21:** cargo feature `egsnrc-f32-constants` (`punggol::egsnrc::lit`, 204 Mortran decimal constants in `egsnrc_mortran.rs` plus the tutor1 driver's `0.1`, `20.5109989461`, `0.5109989461`); off by default. Results against the STOCK build with the feature on are in "Single-precision-constants mode" below. This explained differences of the
same order in other ported routines and is the first thing to check there.

### Electron tables (`hatch_media`: PEGS4 intake, `DUNIT`, `fix_brems`, `mscati`)

Test `differential_electron_tables_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_ELECTRON_TABLES`, the `tables_electron.txt` of the same
`difftab` run: Ta, `tutor_data.pegs4dat`, default options, 14 tables over the
electron grid):

| reference build | result |
|---|---|
| `-fdefault-real-8 -fdefault-double-8` | **bit-identical** (relative difference exactly 0) for `eke0/1`, `blcc`, `xcc`, `esig_e`, `psig_e`, `te`, `thmoll`, `ededx`, `pdedx`, `esig`, `psig`, `ebr1`, `pbr1`, `pbr2`, both ranges and `e_array`; `tmxs` 1.6e-14 |
| stock (`-O1`, single-precision literals) | the same tables exact except `te` 9.6e-8, `thmoll` 2.0e-8, `blcc` 3.7e-8, `tmxs` 3.3e-8 (the `prm`/`rm` constant carried as a single-precision literal) |

### Spin fits (`init_spin` on the real `spinms.data`)

Test `differential_spin_fits_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_SPIN_TABLES`): `ETAE_MS`, `ETAP_MS`, `Q1CE_MS`, `Q1CP_MS`,
`Q2CE_MS`, `Q2CP_MS`, `BLCCE` (both coefficients, every interval) against the
`-fdefault-real-8` reference: **bit-identical** (relative difference exactly 0
on all 14 arrays). This also exercises `SpinFile::from_bytes` on the real file
and the single-precision (`real*4`) storage of the spin data.

`init_compton`, the Rayleigh tables and the photoelectric edge data are not in
these comparisons (`hatch_media` does not assemble them).

### Rayleigh tables

Test `differential_rayleigh_tables_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_RAYLEIGH_TABLES`): `xgrid` exact, `fcum` 2.6e-16, `b_array`
3.1e-15, `c_array` 3.1e-15, `pmax0/1` 4.6e-13 / 6.0e-13 (cancelling
coefficients), `i_array` identical, against the promoted build.

### Transport histories (`tutor1`)

A copy of `tutor1.mortran` (`difftut`) seeds RANMAR at 1802/9373, runs 200
histories of a 20 MeV electron on the 1 mm Ta plate and logs every discarded
particle (charge, kinetic energy, `w`) at full precision, with `eadl_relax`
set false (simple relaxation; the EADL default is compared in its own section). Test
`differential_tutor1_histories_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_HISTORIES`) runs `punggol::egsnrc::shower` on the same seeds
(`petir` RANMAR) with the full `HATCH` assembly (PEGS4 intake, `hatch_media`,
`init_user_photon` xcom series, `init_compton`, `edgset_from_text`,
`init_spin` on the real `spinms.data`, `MsData` from the real `msnew.data`),
default options (`ibcmp = 3` bound Compton, Rayleigh on, photoelectron
angular sampling on, spin effects on, PRESTA-II, exact BCA) and a Rust copy of
`tutor1`'s `HOWFAR`/`HOWNEAR`:

| reference build | histories | same sequence of discarded particles | worst relative kinetic energy difference | worst abs difference of `w` |
|---|---|---|---|---|
| `-fdefault-real-8` | 200 | **200 of 200** | 1.1e-8 | 8.0e-8 |
| stock (single-precision constants) | 200 | **200 of 200** | 5.8e-7 | 2.0e-6 |

History 1 (the first discarded electron): port 15.145539980145696 MeV,
`w = 0.973402260762049`; promoted build 15.145539980154389,
0.973402260772736 (relative 5.7e-13); stock 15.145540869713436,
0.9734022872287683. Every random decision of all 200 showers (every
interaction type, every secondary) is the same as upstream's, which is a much
stronger statement than the table agreement; the residual 1e-8 is round-off
amplified through the shower, and the stock-build 6e-7 is the
single-precision-constant effect of gh:#1687 propagated. It is above the 1e-9
code-to-code bound and is flagged for the maintainer; the sample (one
geometry, one material, one source) is small. Not covered: EADL relaxation,
photonuclear, positrons, EII, triplet, NIST/NRC bremsstrahlung and the NRC pair
tables in a full transport run (the readers and samplers of the last four are
exercised only by unit tests).

### EADL relaxation (the stock default configuration)

Ported 2026-11-21: `EadlFile::from_bytes` (`relax.data`, 4-byte records),
`init_relax_eadl` (`egs_init_relax`, `egsnrc.mortran:5793-6008`), `eadl_relax`
(`egs_eadl_relax`, `:6019-6208`), the EADL branches of `relax`, `photo`,
`init_compton` (`init_compton_eadl`) and `EDGSET` (`edgset_eadl_from_text`).
The MCDF shellwise photoelectric option (off by default) is not ported.
Against a reference built with `eadl_relax` at its stock default (`difftutE`,
`difftabE`, promoted build; tests `..._histories_against_compiled_egsnrc_eadl`
with `EGSNRC_REFERENCE_HISTORIES_EADL`, and
`differential_eadl_tables_against_compiled_egsnrc` with
`EGSNRC_REFERENCE_EADL_TABLES`):

- 200 `tutor1` histories: **200 of 200** same charge sequence, worst relative
  kinetic energy difference 5.3e-9, worst |dw| 2.4e-8 (the simple-relaxation
  case is unchanged at 1.07e-8).
- Tables: all 22 EADL shells and 1270 relaxation channels of Ta, all 1538
  Compton shells, the medium alias table and the Ta binding energies agree;
  integers exactly, reals bit-identical except `be_array` at 1.7e-16.

Upstream oddities reproduced (doc comments): `egsnrc.mortran:6145` tests
`new_state < 0` on an alias bin number that is always >= 1, so the leftover bin
is treated as a radiative transition to a global shell index that can be 0 or
outside the array (the port uses 0.0 and ends the cascade); `:6108` assigns
`edep = Evac` where `:6125` uses `+=` (equivalent only because `COMPT` zeroes
`edep`); loading all 100 elements into one medium needs 10055 channels against
`$MAXRELAX = 10000` and is refused as upstream would. ~~The 5e-9 and 1e-8 residuals are above the 1e-9 bound and unexplained beyond round-off.~~ **CORRECTED 2026-11-21: explained and removed, see "Histories to round-off"; EADL histories are now 1.0e-15.**

## Not differentially tested, and why (CORRECTED 2026-11-21)

**Current state:** compared and agreeing (see the sections below): PEGS4, HATCH photon/electron/spin/Rayleigh/Compton/EADL/EII/NIST/NRC-pair/triplet tables, 200-history transport in every option combination (~1e-15), ESTAR mixtures and `egs_estar`. Not compared, with the reason: `xsec_out` (diagnostic file output, not ported on purpose), `outputDensityFile` (same), ~~photonuclear~~ **CORRECTED 2026-11-21: photonuclear IS ported (the `PHOTONUC` placeholder, the HATCH `photonuc0/1` table and the `$PHOTONUC-*` hooks in `photon`) and is now compared, see "Photonuclear" below**; electromagnetic fields (`EEMF`) and `egs_parallel` (not ported), the OpenMC elements other than U and H (no data, gh:#1695). The text of this section as first written follows, struck through where later work superseded it.

See the GitHub issues linked from `egsnrc_port_remaining.md`. In short:

- ~~Optional kernel branches not in the `tutor1` run: EII, triplet, NIST/NRC
  bremsstrahlung, NRC pair,~~ **now compared (see "Optional branches", "Option combinations")**; photonuclear,
  positron-dominated showers and electromagnetic fields (not ported). The
  built reference can run them with a user code that sets the switches.
- Everything that reads the data files above: now readable (see above), but
  no EGSnrc output exists to compare the derived tables with until the
  transport kernel is built.
- ~~All of `punggol::openmc_photon`: OpenMC needs its vendored submodules and HDF5 photon data, which are not available.~~ **CORRECTED 2026-11-21**: OpenMC was built and U and H compared (`openmc_photon_phase2.md`).

## Issues filed (2026-11-21)

- gh:#1686 ~~histories comparison (`tutor1`)~~ **done 2026-11-21 (above)**; `init_compton` with
  `eadl_relax` and the edge tables are done too (EADL section).
- gh:#1687 single-precision constants in the upstream build: maintainer decision.
- gh:#1688 ~~PEGS4 residual of 2e-4, cause not identified~~ **resolved 2026-11-21, bit-identical**.
- ~~gh:#1689~~ **SETTLED 2026-11-21, see "Electron-impact ionisation"** the four `eii.mortran` oddities, now checkable against the built EGSnrc.
- gh:#1690 ~~OpenMC phase 2: needs vendored sources and HDF5 photon data~~ **done for U and H, including a two-element material**; other elements: gh:#1695.
- gh:#1692 ~~EADL relaxation not ported~~ **done 2026-11-21 (above)**; triplet, NRC pair, NIST/NRC bremsstrahlung and EII still not wired into a full run.
- gh:#1691 ~~`egs_compoundstoelements` source missing~~ **FOUND AND PORTED 2026-11-21** (see "ESTAR mixtures" below); `rad_compton1` (not in scope) and the duplicate `ztbl` remain.


## Single-precision-constants mode against the stock build (gh:#1687, 2026-11-21)

`cargo test --release -p punggol --features egsnrc-f32-constants --lib differential_`
with the stock-build references (`difftab`, `difftut`; `-O1`, no promotion).
Default build (feature off) is unchanged: histories against the promoted build
still 1.1e-8 (re-run, same day).

| quantity | f64 constants vs stock (before) | f32 constants vs stock (now) |
|---|---|---|
| photon tables `gmfp`, `gbr1`, `gbr2` (worst, 2000 points) | 2.8e-8 | 1.5e-13, 3.1e-13, 7.1e-12 |
| electron tables | up to 9.6e-8 (`te`) | exact except `tmxs` 4.6e-14 |
| tutor1 histories, same discard sequence | 200 of 200 | 200 of 200 |
| tutor1 histories, worst relative kinetic energy | 5.8e-7 | 4.5e-8 |
| tutor1 histories, worst `|dw|` | 2.0e-6 | 1.3e-7 |
| history 1 (first), kinetic energy | 15.1455401944 vs ref 15.1455408697 (4.5e-8, wrapped port constants only) | 15.145540867215 vs 15.145540869713 (1.6e-10) |

The history result needed the tutor1 **driver's own** single-precision
literals too (`PCUT = 0.1`, `ZBOUND = 0.1`, `EIN = 20.5109989461`,
`EKINE = E - 0.5109989461`): the Rust copy of the driver used exact `f64`, and
that alone left the first history at 4.5e-8. The remaining worst-case 4.5e-8
over 200 histories is not explained; a constant not yet wrapped (Mortran
literals in files other than `egsnrc_mortran.rs`, or in the expansion of a
macro) or amplification of a ~1e-10 start through the shower are both
~~possible, not tested. Residual above 1e-9: reported to the maintainer.~~ **CORRECTED 2026-11-21: the cause was the single-precision `init_ms_SR` arrays and the driver's own literals; the stock-build histories are now ~1e-15 ("Histories to round-off").**
Spin, Rayleigh and EADL stock references were built afterwards (below).

### Stock-build spin, Rayleigh and EADL (2026-11-21)

Stock references rebuilt from the same sources (`gfortran -x f77-cpp-input
-std=legacy -O1`, no promotion) as `difftab_stock`, `difftabE_stock`,
`difftutE_stock`; port with `egsnrc-f32-constants` (feature) vs without:

| quantity | f64 constants | f32 constants |
|---|---|---|
| Rayleigh `xgrid` | 2.5e-8 | exact |
| Rayleigh `fcum`, `b`, `c` | 4.2e-12, 8.8e-13, 5.1e-8 | 2.9e-16, 3.1e-15, 3.0e-15 |
| Rayleigh `pmax0`, `pmax1` | not recorded | 5.9e-7, 4.7e-4 (cancelling coefficients) |
| spin fits (worst of 14 tables) | 1.3e-5 | 4.2e-6 (`etap1`; `q1ce1` 6.5e-9) |
| EADL `shell_be`, Ta binding energies | 2.5e-9 | exact |
| EADL Compton `be` | 3.8e-8 | 1.7e-16 |
| EADL Compton `eno` | 1.9e-8 | 1.9e-8 (unchanged) |
| EADL Compton `erfJo` | 1.0e-3 | 1.0e-3 (unchanged; relative metric on near-zero values) |
| EADL tutor1 histories (200, same sequence) worst relative kinetic energy | 5.6e-7 | 4.1e-8 |
| EADL history 1 | 15.145539980 vs ref 15.145540870 | 15.145540867 vs 15.145540870 |

The feature removes most of the stock-build difference everywhere but does not
reach round-off for the spin fits, `pmax`, `eno`, `erfJo` and the 4e-8 history
worst case: some single-precision effect is still not reproduced (not located;
tracked in gh:#1694). The EADL table test asserts `< 1e-9` and therefore fails
against the stock references in both modes, as it should (its gate is the
promoted build).


## ESTAR mixtures: `egs_compoundstoelements` (gh:#1691, 2026-11-21)

The routine was missing because the clone held only `HEN_HOUSE/src` and
`pegs4`; it is `egsCompoundsToElements` in `HEN_HOUSE/egs++/estar/estar.cpp`
(with `estar_formulaCalcs.cpp`, `estar_dataParser.cpp`, `estar_dataTables.cpp`;
`git sparse-checkout add HEN_HOUSE/egs++/estar`). Ported as
`punggol::egsnrc::estar_cpp` and wired into `get_media_inputs`, so a `mixture
compounds` medium is expanded to elements (ascending `Z`, normalised mass
fractions). Differential test `differential_estar_mixture_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_ESTAR`) against upstream's own `mixtureCalculation`,
built by `verification_and_validation/egsnrc_estar/harness.cpp` with
`egsFatal`/`egsInformation` stubbed (`g++ -std=c++17 -Iinc -I<estar>
harness.cpp estar_formulaCalcs.cpp estar_dataParser.cpp estar_dataTables.cpp`):
5 mixtures (water; water + a hydrocarbon; salt, water and oxygen; a
four-oxide rock; uranium dioxide + water), **0 difference** in `Z` and in every
mass fraction (bit-identical). Upstream oddity ported as it is: `getElemArray`
capitalises the first and lower-cases the second character of each token, so
`CH4` becomes `Ch4` (an unknown symbol, fatal) and must be written `C1H4`.
Mixture I-value and `egs_estar` are ported later (see the `egs_estar` section).


## Electron-impact ionisation (gh:#1689, 2026-11-21)

Reference: `difftutI`/`difftabI`, i.e. `difftutE`/`difftabE` (EADL relaxation,
the stock default) with `eii_flag=1; eii_xfile='ik';` before `CALL HATCH` and
`EII-DATA` in the user code's `COMIN` list (the 200-history `tutor1` run and
the table dump); promoted and stock builds. Tests: tables via
`EGSNRC_REFERENCE_EII_TABLES` and histories via
`EGSNRC_REFERENCE_HISTORIES_EII`
(`differential_tutor1_histories_against_compiled_egsnrc_eii`).

**Finding: the four oddities of `eii.mortran` were a red herring.**
`HEN_HOUSE/src/eii.mortran` is not compiled into EGSnrc; `egsnrc.mortran`
contains its own, different `eii_init` (`:7433`) and `eii_sample` (`:7759`).
The first comparison of the port (made from `eii.mortran`) failed: `ededx0`
1.3 %, `esig1` 12x, `ebr1` (rewritten by the live version, unchanged by the
old) 0.74, whatever combination of the three `EiiReadings` switches was used,
and the histories diverged in the first history (17.74 vs 18.05 MeV).
Ported the live routines as `eii_init_egsnrc` and `eii_sample_egsnrc`
(`moller_eii_select` now calls the latter). Differences from the standalone
file: `binding_energies(ish, iZ)` and `ap(imed)` are read as intended;
`sh_0`/`sh_1`, `sigo` and `de` (with `sum_occn`) are new; `esig` is rebuilt
from `sigm + wbrem*sig`, `ebr1` is rewritten, `esig_e` is the maximum of
`sigma/dedx`; `eii_a(jjj)` is still indexed by shell type; `eii_sample` has
`c1 = (Wmax/peie)^2`, `fm_h = 2 + c1 - c2`, `0.66666667`, the other soft and
hard sampling, acceptance on `r3`, and `pese1 = peie - wx` (so no refusal
below `T = prm`). Not ported: `eii_L_factor` (default 1), `xsec_out`.

| quantity | result |
|---|---|
| electron tables after `eii_init` (12 columns, 40 energies), promoted build | **bit-identical** (all 12 worst differences 0) |
| same, stock build with `egsnrc-f32-constants` | bit-identical |
| same, stock build, f64 constants | up to 3.3e-7 (`pbr` slope) |
| 200 histories, promoted build | 200 of 200 same discard sequence, worst relative kinetic energy 1.7e-9, worst `|dw|` 1.0e-8 |
| 200 histories, stock build with `egsnrc-f32-constants` | 200 of 200, 3.6e-9, 2.6e-8 |
| 200 histories, stock build, f64 constants | 200 of 200, 1.5e-7, 6.5e-7 |

~~Residual 1.7e-9 to 3.6e-9 is above the 1e-9 allowance and is reported to the maintainer; its cause is not isolated~~ **CORRECTED 2026-11-21: cause found (single-precision `init_ms_SR` arrays), histories now at ~1e-15, see "Histories to round-off"**. The three `EiiReadings` switches are not
used by the live path and remain for the standalone file.


## Optional branches: NIST/NRC bremsstrahlung, NRC pair, triplet (gh:#1692, 2026-11-21)

References: `difftutE` (EADL) plus one line before `CALL HATCH`
(`ibr_nist=1;`, `ibr_nist=2;`, `pair_nrc=1;`, `itriplet=1;`) and `BREMPR` in
the user code's `COMIN` list (`difftutB`, `difftutR`, `difftutP`, `difftutT`;
promoted build, 200 `tutor1` histories). For `pair_nrc` and `itriplet` the port
needed wiring: `PhotonTables::{nrc_pair, triplet, up_max}` and
`Stack::triplet` (the `save`d `sample_triplet` state, which Fortran keeps
across histories, so the test reuses one `Stack`), and `photon` now calls
`pair_core` with them. Real `pair_nrc1.data`, `triplet.data`, `nist_brems.data`
and `nrc_brems.data` are read for the first time here.

| option | tables | 200 histories, same discard sequence | worst relative kinetic energy | worst `|dw|` |
|---|---|---|---|---|
| `ibr_nist = 1` (NIST brems) | bit-identical (12 columns) | 200 of 200 | 3.2e-8 | 2.8e-7 |
| `ibr_nist = 1`, stock build with `egsnrc-f32-constants` | bit-identical | 200 of 200 | 8.2e-8 | 7.6e-7 |
| `ibr_nist = 2` (NRC brems) | bit-identical | 200 of 200 | 3.8e-9 | 1.9e-8 |
| `pair_nrc = 1` | not dumped | 200 of 200 | 2.1e-8 | 1.6e-7 |
| `itriplet = 1` | not dumped | 200 of 200 | 4.4e-9 | 4.7e-9 |

Control: the port WITHOUT `pair_nrc` or `itriplet` against those references
diverges at history 19 (58 and 48 of 200 histories agree in sequence), so the
options do change the run and the agreement above is not an artefact of rare
events. ~~All residuals are above the 1e-9 allowance; causes not isolated. Not compared: `pair_nrc1.data` and `triplet.data` tables themselves, the options in combination, `eii_flag` with brems options.~~ **CORRECTED 2026-11-21: all of these are now compared and the histories are ~1e-15, see "Option combinations, pair/triplet tables, `eii_L_factor`" and "Histories to round-off"**.


## PEGS4 residual: `f32` library functions (gh:#1688, 2026-11-21)

The PEGS4 port's single-precision fit coefficients differed from a built PEGS4
at 1.7e-4 to 3.8e-4. **Cause found:** `petir`'s `f32` `r_exp`, `r_ln` and
`r_powf` are one ulp off glibc's `expf`, `logf`, `powf` (which gfortran calls
for `real*4` `exp`, `log`, `**`) in 7 to 9 % of arguments
(`tests/f32_math_vs_gfortran.rs` with `verification_and_validation/f32_math/`:
319, 282 and 379 of 4000 arguments differ; `r_powi` agrees on 48000 of 48000).
Computing the three in `f64` and rounding (glibc's results are within 0.502
ulp, so this agrees with them almost always) in `pegs4_routines_mortran.rs`
and `pegs4_mortran.rs`:

| medium | differing lines before | after | worst `|d|/max|line|` before | after |
|---|---|---|---|---|
| `pb700` | 606 of 3388 | 9 | 1.7e-4 | 8.9e-6 |
| `h2o700` | 576 of 3741 | 8 | 3.8e-4 | 3.2e-6 |
| `tis521` | 131 of 674 | 3 | 2.4e-4 | 2.8e-6 |

The 20 lines that still differ (three clusters of a few coefficients each)
~~are above the 1e-9 bound; cause not isolated~~ **CORRECTED 2026-11-21: after the `f32` fix below all three media are bit-identical**. The `f64` functions of `petir`
also differ from glibc in the last bit for `sin` (571 of 20000), `cos` (618),
`tan` (110), `atan` (917), `log10` (2410), `exp` (22), `pow` (9), `ln` (1);
substituting `f64::sin/cos/tan/exp/ln/powf` (glibc through `std`) for them in
`egsnrc_mortran.rs` changed **no** history number (tutor1, promoted build:
still 1.07e-8 / 5.3e-9 with EADL), so those last-bit differences are not what
limits the `shower` comparison. `petir` is not changed here. `ln32` (the
spin-table setup's `real*4` `alog`) now uses the `f64` route too, with no
visible effect.


## Stock-build residuals resolved (gh:#1694, 2026-11-21)

Method: dump `eta_array`, `c_array`, `g_array` and `earray` from the stock and
promoted Fortran builds (a line inserted into the generated `.f`) and from the
port, and bisect. The stock build differs from the promoted one by more than
"constants rounded through f32": **expressions that mix a `real*4` constant
with `real*4` variables are evaluated in single precision**, and `Log(c)` of a
constant is a single-precision `logf`. Found and fixed (each only in
`egsnrc-f32-constants` mode; the default is unchanged):

- `0.26112447*aux_o` (`init_spin`): real*4 times real*4, `lit_mul`;
- `Log(1e5)` (`elarray`, `init_spin`): `lit_ln`;
- `atav = 1./nsh` (`init_compton`): `lit_div`;
- constants that are `d0` in the Mortran (`20.607544d0`, `1.005d0`,
  `1.8215416D4`, `0.001D0`) had been rounded by mistake: now exact;
- `erf1`'s `DATA` coefficients and `CONST` are real*4 literals in a stock
  build: now `lit`.

Results against the stock build with the feature on (all were above 1e-9
before):

| quantity | before | after |
|---|---|---|
| spin fits (14 tables) | up to 4.2e-6 | **bit-identical** |
| Rayleigh `pmax0/1` | 5.9e-7, 4.7e-4 | 5.0e-13, 6.5e-13 |
| Compton `eno`, `erfJo` | 1.9e-8, 1.0e-3 | **0, 0** |
| tutor1 histories (200, same sequence) | 4.5e-8 | 1.07e-8 |
| EADL histories | 4.1e-8 | 5.3e-9 |

~~Remaining: the histories differ from both the stock and the promoted build by
5e-9 to 1e-8 although every table is bit-identical~~ **CORRECTED 2026-11-21, see "Histories to round-off" below: found and fixed** (original text: the histories differ from both builds by 5e-9 to 1e-8 although every table is bit-identical, so the cause is
arithmetic order inside the transport routines; the first history already
differs by 5.7e-13 and the shower amplifies it. This is a propagated
round-off of the kind the maintainer's rule (root `CLAUDE.md`: "1e-4 for
propagated error") allows, and is recorded here rather than chased. The
20 `pegs4` lines of gh:#1688 are not resolved.


## Histories to round-off, PEGS4 bit-identical (gh:#1694, gh:#1688, 2026-11-21)

**Histories.** Traces from both programs (Fortran `WRITE`s inserted into the
generated `.f`; the port's `eprintln`s, since removed) were compared event by
event: `mscat` inputs, `msdist_pII` outputs, `UPHI` angles, `BREMS`, `MOLLER`,
the energy loss per step. The first difference was in the very first
`msdist_pII`: `sint1` differed by 1e-10 although `cost1` differed by one ulp,
i.e. the multiple-scattering table `fms_array` was one single-precision ulp
off. Cause: `init_ms_SR` computes `fms_array(k) = fms_array(k+1)/fms_array(k)
- 1` and `dllamb`, `dllambi`, `dqms`, `dqmsi` with **all-`real*4` operands**,
i.e. in single precision, in both the stock and the promoted build (the
arrays are declared `real*4` explicitly); the port did them in `f64` and
rounded. Fixed (single-precision arithmetic). Two smaller order-of-operations
slips found on the way are also fixed (`temp2` in `msdist_pII`, now
`... * (q*q)` as the source has it).

Results (200 `tutor1` histories each, same discard sequence in all):

| reference | before | after (worst relative kinetic energy) |
|---|---|---|
| promoted, plain | 1.1e-8 | **1.5e-15** |
| promoted, EADL | 5.3e-9 | 1.0e-15 |
| promoted, EII | 1.7e-9 | 4.4e-16 |
| promoted, NIST brems / NRC brems | 3.2e-8 / 3.8e-9 | 1.0e-15 / 4.8e-16 |
| promoted, `pair_nrc` / `itriplet` | 2.1e-8 / 4.4e-9 | 1.7e-15 / 1.1e-15 |
| stock + `egsnrc-f32-constants`, plain / EADL / EII / NIST | 1.1e-8 / 5.3e-9 / 3.6e-9 / 8.2e-8 | 1.2e-15 / 1.3e-15 / 7.4e-16 / 1.2e-15 |

All are floating-point round-off (a few ulp). The earlier statement that the
remainder was "propagated round-off" was wrong: it was a real defect.

**PEGS4 (gh:#1688).** glibc's `expf`, `logf`, `powf` are not correctly rounded
(`logf` differs from the correctly rounded result in 0.35 % of arguments, 14
of 4000), so computing the `f32` functions in `f64` and rounding left 20 lines
differing. With the platform functions (`f32::ln`, `exp`, `powf`, `log10`,
which call glibc) `differential_against_compiled_pegs4` is **bit-identical on
all three media** (`pb700` 3388, `h2o700` 3741, `tis521` 674 lines, 0
differing). The `no_std` build keeps the `f64` route (compiles; not bit-identical,
as before). `ln32` in `init_spin` uses the same rule.


## Option combinations, pair/triplet tables, `eii_L_factor` (2026-11-21)

- **All options at once** (EADL, `eii_flag = 1`, `ibr_nist` 1 or 2, `pair_nrc = 1`,
  `itriplet = 1`; reference `difftutX1`/`difftutX2`, test
  `differential_tutor1_histories_against_compiled_egsnrc_all_options`,
  `EGSNRC_REFERENCE_HISTORIES_ALL`): 200 of 200 same sequence, worst relative
  kinetic energy **4.5e-16** (NIST) and **1.1e-15** (NRC).
- **NRC pair and triplet tables themselves** (`difftutN` dumps `nrcp_*` and
  `a_triplet`/`b_triplet` after `HATCH`; `EGSNRC_REFERENCE_PAIRTABLES`): triplet
  `a`, `b`, `dl`, `dli`, `bli`, `log_4rm` **bit-identical**; NRC pair x grid,
  `fdata`, `wdata`, `idata` bit-identical (3 layout offsets documented in the
  test: the port keeps `fdata` 0-based and `wdata`, `idata` with slot 0 unused).
  The header (`emin`, `emax`, `dle`, `dlei`) first differed by 4.2e-8 because
  `log((emax-2)/(emin-2))/(ne-1)` is single precision in the source (`emin`,
  `emax` are `real*4`); fixed, now bit-identical.
- **`eii_L_factor`** (L-shell EII cross sections scaled, default 1; new
  `eii_init_egsnrc_l`): reference with `eii_L_factor = 0.5`: tables bit-identical,
  200 histories 1.0e-15; the control with the factor left at 1 against the same
  reference diverges at history 1 (47 of 200 in sequence), so the factor does
  act.
- **`xsec_out`** only writes a diagnostic cross-section file: not ported on
  purpose (no physics, `no_std` library).
- **`egs_estar`** (density-effect correction): see the next section.


## `egs_estar` density-effect calculation (2026-11-21, gh:#1696)

Ported: `estar.cpp:estarCalculation` with `egsEstar`'s argument checks
(`estar_calculation` in `estar_cpp.rs`), `fcalc` / `mixtureCalculation` /
`compRes` / `getDataFromFormulae` (I-value, `Z/A`, mass fractions), `parseData`
(the oscillator layout of `elementData`), `fscof`, `fbspol`,
`objective_function`, `bisec`. The upstream tables `energy_grid`, `poth`,
`potgas`, `potcon` and `elementData` (14530 numbers in a `[14532]` array,
zero-filled like C++) are **read from `estar_dataTables.cpp` at run time**
(`EstarTables::from_upstream_source`) and never committed. Not ported:
`outputDensityFile` (writes a diagnostic text file), like `xsec_out`.

**The `q` grid is x87 `long double`.** Upstream builds `vector<long double> q`
(1101 points, ratio `exp(ln 10 / 50)`) and mixes it with `double` in
`eps + q`, `q / denom` and `q / (yq + 1)`; on x86-64 Linux those are 64-bit
mantissa operations. The port emulates them (`Ext`: round-to-nearest-even
multiply, divide, add, subtract, conversion to and from `f64`).

Reference: `verification_and_validation/egsnrc_estar/harness_calc.cpp` includes
the upstream `estar.cpp`, `estar_formulaCalcs.cpp`, `estar_dataParser.cpp` and
`estar_dataTables.cpp` (`g++ -O2 -std=c++17`, `egsFatal`/`egsInformation`
stubbed) and calls `estarCalculation` for 17 media: elements (H gas and
liquid, Al conductor, elemental C special case, Cu, Pb, Au, N gas), compounds
(water, water with a user I-value of 75 eV, NaI, SiO2, methane gas, a
repeated-element merge), mixtures (air, water plus methane, a three-oxide
mixture). Test `differential_estar_calculation_against_compiled_egsnrc`
(`EGSNRC_REFERENCE_ESTAR_CALC`, `EGSNRC_ESTAR_TABLES`).

| quantity | result |
|---|---|
| density correction, 17 media x 113 energies | **1921 of 1921 bit-identical**, worst 0 |
| mean I-value (`float`) | equal in all 17 |

**Ablation** (what the `long double` emulation carries): with every `Ext`
operation replaced by plain `f64` arithmetic, only **1150 of 1921** are
bit-identical and the worst relative difference is **1.8e-14** (H gas, grid
point 72). That is round-off below the 1e-9 allowance, but the emulation is
what reaches bit identity, so it stays.

Upstream observation (not a port defect): hydrogen at 1 g/cm3 makes
`estarCalculation` abort ("Newton's method converged to negative root"), so
the H case uses the physical densities 8.375e-5 and 0.0708 g/cm3; the port
returns `Err` in the same situation.


## Full re-run of every differential comparison (2026-11-21, one session, final sweep)

All run in release mode against the references in scratch, in the same session
as the last code change (`cargo test --release -p punggol --lib differential
-- --nocapture` with the `EGSNRC_REFERENCE_*` variables set; promoted-build
references, `-fdefault-real-8`):

| comparison | result |
|---|---|
| PEGS4 `pb700` / `h2o700` / `tis521` | 3388 / 3741 / 674 lines, **0 differing** |
| photon tables (`gmfp`, `gbr1`, `gbr2`, `cohe`) | worst 1.7e-12 at table-interval level; function values 2.9e-16 to 1.1e-15 |
| electron tables | exact except `tmxs` 1.6e-14, 1.1e-14 |
| Rayleigh / spin tables | worst 6.0e-13 / **0** |
| EADL tables (shells, channels, alias) | integers 0 mismatches; worst 1.7e-16 |
| EII tables (12 arrays) | all **0** |
| tutor1 200 histories, plain | same sequence 200 of 200, kinetic energy 1.5e-15 |
| ... EADL | 200 of 200, same order of size |
| ... EII | 200 of 200, 4.4e-16 |
| ... NIST brems (`ibr_nist = 1`, `difftutB_8`) | 200 of 200, 1.0e-15 |
| ... NRC brems (`ibr_nist = 2`, `difftutR_8`) | 200 of 200, 4.8e-16 |
| ... `pair_nrc = 1` | 200 of 200, 1.7e-15 |
| ... `itriplet = 1` | 200 of 200, 1.1e-15 |
| ... all options, NIST / NRC | 200 of 200, 4.5e-16 / 1.1e-15 |
| ESTAR mixtures (5) / `egs_estar` (17 media x 113) | 0 / 1921 of 1921 bit-identical |
| OpenMC U: `calculate_xs`, samplers, relaxation, reactions, TTB tables | 0 mismatches; worst 1.0e-13 (round-off) |
| OpenMC H: same | 0 mismatches; worst 2.6e-14 |
| OpenMC U + H material, weight-fraction TTB, `sample_element` | 0 mismatches; worst 4.3e-15 |

A mix-up worth recording: the first NRC-brems run used `difftutB2_8`, which is
**not** the NRC-brems reference (it is a trace build of another case) and
diverged at history 1; with the right reference (`difftutR_8`) the run agrees.
The reference directories are named by the user code that wrote them, so a
wrong pairing shows up as a divergence, not as a silent pass.

## Open items (every one filed)

- OpenMC elements other than U and H: **data missing**, gh:#1695.
- ~~Photonuclear, EEMF, `egs_parallel`~~ **CORRECTED: photonuclear is compared (below)**; EEMF and `egs_parallel` are not ported (nothing to compare), gh:#1697.
- `xsec_out`, `outputDensityFile`: diagnostic file output, deliberately not
  ported.


## Photonuclear (2026-11-21, gh:#1697 corrected)

`egsnrc_port_remaining.md` already said `PHOTONUC` was ported; the phase 2
list above wrongly counted photonuclear as unported. It is ported (the
`PHOTONUC` placeholder, `iphotonucr`, the `photonuc0/1` table that
`egs_init_user_photon` builds from `iaea_photonuc.data`, and the
`$PHOTONUC-CORRECTION` and `$PHOTONUCLEAR` hooks in `photon`) but had never
been compared. Now it is.

Reference: two copies of the `tutor1` user code with one line changed,
`iphotonucr(2) = 1;` before `CALL HATCH` (the Ta plate is region 2): `difftabF`
(the table-dump copy plus a dump of `PHOTONUC0/1`) and `difftutF` (the
history-listing copy), both built `-fdefault-real-8 -fdefault-double-8`. The
run log confirms "Photonuclear flag: 1 ... REQUESTED FOR MEDIUM NUMBER 1".

| comparison | result |
|---|---|
| `photonuc0`, `photonuc1`, 2000 grid points (`EGSNRC_REFERENCE_PHOTONUC_TABLES`, in `differential_user_photon_tables_against_compiled_egsnrc`) | **bit-identical** (0, 0) |
| 200 `tutor1` histories with `iphotonucr = 1` (`EGSNRC_PHOTONUC=1`, EADL relaxation, `differential_tutor1_histories_against_compiled_egsnrc_eadl`) | same sequence **200 of 200**, kinetic energy 7.4e-16, `|dw|` 1.8e-15 |
| control: the same reference against the port with photonuclear OFF | diverges at history 3 (37 of 200 in sequence), so the option acts |

A pairing mistake worth recording: `difftutF` was derived from `difftutB`,
which does not set `eadl_relax = .false.`, so its reference uses EADL
relaxation (the stock default). The first comparison used the non-EADL test and
diverged at history 4. Tracing `RNNO36`, `RNNO37` and `RNNO39` in both programs
(`WRITE`s inserted into the generated `.f`, the port's `eprintln`s, since
removed) showed the first three photon interactions agree and the divergence
followed a relaxation draw, which pointed at the option mismatch; with the EADL
test the run agrees to round-off. No port defect.

Still not ported: `EEMF`, `EEMF_macros_beamnrc` and `egs_parallel` (gh:#1697);
there is no Rust to compare.
