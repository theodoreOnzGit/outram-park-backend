# ACER plot file `aplots` and `consis`' table edit (gh:#1369)

Date: 2026-11-12. NJOY2016.80 (`ac5adf5` + the three 2016.80 files),
`gfortran` 13.3 `-O3`, glibc 2.39, toolchain per `rust-toolchain.toml`.
Research, education and V&V only.

## What was ported

`aplots` (`acefc.f90:15215-16833`) and `aplotr`, `aplopf`, `aplof4`, `aplonu`,
`aplodd`, `aplodn`, `aplopp`, `aploxp`, `getl7`, `ascle`, `ascll` and `mtname`
(`acecm.f90`) into `src/acer/aplots/` (one file per routine group, each under
the 1000-line cap; see its `mod.rs` table). `acefix_type1` writes the plot when
`AcefixRequest::plot` is set (`ngend != 0`). The output is the **viewr input
text**; drawing it is `viewr`'s job and is not done here.

Also ported, because the plot reads the table it edits: the one part of
`consis` that changes the table (`src/acer/consis.rs`, below).

Upstream quirks kept on purpose (each is in a module doc): `aplodd` law 4's
`xss(1+n+loci)` with the reaction index `n`, its compounding `test` limit, law
44/61 running `ascll` twice, `getl7`'s initialising call overwriting `ep`;
`aplopp`'s integer `nint(xss(loci))/10` against `aplodd`'s `nint(xss(loci)/10)`
and its `j < nd` / `j <= nd` mismatch; the total UR page's `ymax=s1` typo;
`aplof4`/`aploxp`'s `it` not being reset; `aplopf`'s label `(<m>e>v>)`;
single-precision literals (`1.e-6` in `aplots`, `0.1*amin` in `ascll`).

## 1. End to end: the check run of ACER decks (line diff against the NJOY2016 binary)

Method: the upstream deck cut before its first `viewr`; this crate and the
NJOY2016 binary run it; the `ngend` unit is compared line by line
(`tests/deck_driver_tapes_vs_njoy2016.rs`, `*_acer_check_unit_and_directory`).
**Prediction, written before the first run:** the charged-particle decks
(50-54, 62, 71) exercise only the principal/heating/threshold/angular/
particle-production pages and agree in every line; the neutron decks exercise
the rest (UR, nubar, delayed, laws 4/44/61, photon spectra) and agree except
where the ACE table itself differs (35, 37, 63: PURR words).

| deck | material | `ngend` unit | differing lines |
|---|---|---|---|
| 50 | He-4 + alpha | 33 (432 lines) | 0 |
| 51 | H-2 + p | 33 (3 918) | 0 |
| 52 | H-1 + p | 33 (6 042) | 0 |
| 53 | H-2 + d | 33 (17 236) | 0 |
| 54 | H-3 + p | 33 (11 340) | 0 |
| 62 | He-3 + d | 33 (5 460) | 0 |
| 71 | Pt-184 + d (TENDL-2021) | 33 (123 992) | 0 (after `consis`, below) |
| 24 | Pu-239 | 33 | 0 |
| 36, 38, 39, 40, 41, 42 | Sn-119, Kr-83, Ni-63, Mn-55, Ge-71, Zn-67 | 43 | 0 |
| 72 | Be-9 | 43 | 0 |
| 35 | Ag-109 | 43 | 5 (the ACE differs in 5 PURR words) |
| 37, 63 | Co-58, Ag-109 | 43 | not reached: the directory unit 42 differs first (PURR block length) |

Decks 35, 37 and 63 are `#[ignore]`d for that reason. To isolate `aplots` from
it, `test_{35,37,63,71}_plot_of_njoys_own_ace` feed **NJOY's own** `iopt = 1`
file to `acefix` and compare the plot (and the edited ACE) with NJOY's:
**0 differing lines in all four**.

## 2. A defect found: `consis` edits the table (deck 71)

`consis` was recorded as "checks that only print". It does one edit: for a law
4 or law 44 distribution whose outgoing energies exceed `epmax` it drops the
points above `epmax` but one, sets that one to `sigfig(epmax,7,-1)`, and
renormalises the pdf/cdf (`acefc.f90:14516-14560`, `14640-14700`). Deck 71 first
showed it: NJOY's unit 34 had `4.94590800000E+00` where its own unit 31 had
`5.00000000000E+00`; the first plot comparison differed on 48 lines for the same
reason. Before: unit 34 38 lines, unit 33 48 lines differing. After
`consis_shift_distributions`: 0 and 0. The deck-71 test is no longer ignored.
(Issue: the existing #1369 comment thread; the cause was a wrong doc claim in
`acefix.rs` and `acer_iopt7_2026_11_12.md`, both corrected in place.)

## 3. Function level: `ascle`, `ascll`, `mtname` against extracted Fortran

`extract.sh` copies the three routines unchanged (SHA-256 of `acefc.f90`
`66ccec86…` and `acecm.f90` `504c5ac4…` checked first), builds
`scale_driver.f90` and writes `reference.txt` (hex bit patterns).
`tests/aplots_code_to_code.rs` reads the inputs back and compares.
Prediction: `ascle` and `mtname` identical; `ascll` identical but for possible
last-bit PETIR-against-glibc differences in `10**x` (gh:#1455).

| routine | cases | bit-identical | worst difference |
|---|---|---|---|
| `ascle` | 3 040 (40 hand-picked covering every branch + 3 000 random) | 3 040 | 0 |
| `ascll` | 3 000 (24 decades, equal ends, near-equal ends, exact decades) | 3 000 | 0 (PETIR `powf`/`log10` agreed with glibc on all) |
| `mtname` | 21 600 (MT × 6 particles × `iverf` -1/4/5/6) | 21 600 | n/a (text) |

The prediction held in full. `mtname` for `iverf >= 6` and MT 501..599 indexes
`hndf` out of range upstream (undefined behaviour); those are not compared and
no real tape has such an MT.

## Not covered by an upstream comparison, and why

- **`getl7` and the law-67 page of `aplodd`**: no NJOY ACE file in this
  workspace carries a law 67 (ACER converts ENDF LAW=7 to law 61, as the Be-9
  deck shows), so no reference plot exercises it. It is a line-for-line
  translation, **unverified**; extracting it needs `terp1` from `endf.f90`.
- **UR pages, `LSSF = 0` against `LSSF = 1`**: which branch each neutron deck
  takes was not recorded; both are line-for-line translations and each is
  covered only if some deck above has that `LSSF`. Not confirmed.
- **`iverf` 4/5 `mtname` branch in a plot**: covered at function level only
  (the plot uses `iverf = -1`, which upstream treats like 6; a job that ran an
  ENDF-4/5 `iopt = 1` first would differ, not reproduced).
- `ngend` for itype 2 (binary) and MCNPX remain `NotPorted`.
