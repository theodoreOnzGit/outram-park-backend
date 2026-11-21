# EGSnrc phase 2: differential tests against compiled upstream (2026-11-21)

Status: **partial**. The PEGS4 program, the HATCH tables (photon, electron,
spin, Rayleigh) and a 200-history `tutor1` transport run of `punggol::egsnrc`
have been compared with a built upstream, and the data-file readers have been
run on the real EGSnrc data files. `punggol::openmc_photon` has not (gh:#1690). Everything else in `punggol::egsnrc` and all of
`punggol::openmc_photon` is still "first port, not differentially tested"; the
reasons and the issues are at the end.

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
bit-identical to the `-O0/-O1` build and **no cause has been identified**.
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
(not yet compared). A copy, `difftab.mortran`, calls `HATCH` and dumps
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
`$MAXRELAX = 10000` and is refused as upstream would. The 5e-9 and 1e-8
residuals are above the 1e-9 bound and unexplained beyond round-off.

## Not differentially tested, and why

See the GitHub issues linked from `egsnrc_port_remaining.md`. In short:

- Optional kernel branches not in the `tutor1` run: EII, triplet, NIST/NRC
  bremsstrahlung, NRC pair, photonuclear,
  positron-dominated showers and electromagnetic fields (not ported). The
  built reference can run them with a user code that sets the switches.
- Everything that reads the data files above: now readable (see above), but
  no EGSnrc output exists to compare the derived tables with until the
  transport kernel is built.
- All of `punggol::openmc_photon`: OpenMC needs its vendored submodules and
  HDF5 photon data, which are not available.

## Issues filed (2026-11-21)

- gh:#1686 ~~histories comparison (`tutor1`)~~ **done 2026-11-21 (above)**; `init_compton` with
  `eadl_relax` and the edge tables are done too (EADL section).
- gh:#1687 single-precision constants in the upstream build: maintainer decision.
- gh:#1688 PEGS4 residual of 2e-4, cause not identified.
- gh:#1689 the four `eii.mortran` oddities, now checkable against the built EGSnrc.
- gh:#1690 OpenMC phase 2: needs vendored sources and HDF5 photon data.
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
possible, not tested. Residual above 1e-9: reported to the maintainer.
Spin, Rayleigh and EADL stock references were not built (the existing ones are
promoted builds).


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
Not ported: the mixture's I-value and `egs_estar`.
