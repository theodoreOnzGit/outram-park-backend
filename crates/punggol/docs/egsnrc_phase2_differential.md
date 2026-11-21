# EGSnrc phase 2: differential tests against compiled upstream (2026-11-21)

Status: **partial**. The PEGS4 program (`pegs4_mortran.rs`) has been
compared with a built upstream, and the data-file readers have been run on the
real EGSnrc data files. Everything else in `punggol::egsnrc` and all of
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

## Not differentially tested, and why

See the GitHub issues linked from `egsnrc_port_remaining.md`. In short:

- The EGSnrc transport kernel (`photon`, `electr`, `shower`, `moller`, `brems`,
  EII, triplet, Rayleigh, ...): needs a built user code on the full EGSnrc
  tree and a matching geometry on the Rust side.
- Everything that reads the data files above: now readable (see above), but
  no EGSnrc output exists to compare the derived tables with until the
  transport kernel is built.
- All of `punggol::openmc_photon`: OpenMC needs its vendored submodules and
  HDF5 photon data, which are not available.
