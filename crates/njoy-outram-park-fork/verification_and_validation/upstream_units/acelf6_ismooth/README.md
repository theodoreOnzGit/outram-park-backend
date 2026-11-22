# ACER `acelf6` `ismooth` extensions against compiled NJOY2016 (unit level), gh:#1368

Research, education and V&V only; not for facility operation or safety decisions.

## Methodology

- Upstream: NJOY2016 (modified BSD-3, LANL), **2016.80 `ab27c6414a0f25b173f2b637e6dc7e428fdaff00`** and the port's previous
  baseline **2016.79 `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`**, accessed 2026-11-10. Provenance: `../../../upstream_source/README.md`.
- 2016.80 change (upstream commit `7d317f8`, "Fix bug in smoothing distributions when outgoing energy grid does not start
  at 0"): both `ismooth` branches of `acelf6` (LEP=1 log-scale sqrt(E) extension of histograms, LEP=2 lin-lin extension) also
  require `eout1 = scr(7)/emev = 0`, i.e. the first tabulated outgoing energy is zero.
- `extract.sh 80|79 <dir>` (SHA-256 of `acefc.f90` checked) extracts the block verbatim, inside `acelf6` only, wrapped in
  `subroutine smooth`; the only added line is `eout1=scr(7)/emev` before it (so both versions compile; 2016.79's block never
  reads it). `driver.f90` runs 12 LAW=1/LANG=2 incident-energy records (`scr(7..)`: rows `E', pdf, A..`), prints inputs
  (`I`/`D` lines) and outputs (`R`/`E`) as hex. `run.sh` rebuilds both (gfortran 13.3.0 `-O2`, same flags).
- Cases: 1 LEP=1, E'_1=0; 2 LEP=1, E'_1=1 keV; 3 LEP=1, E'_1=1e-3 eV; 4 LEP=1, E'_1=0 with merging; 5 LEP=2, E'_1=0;
  6 LEP=2, E'_1=500 eV; 7 LEP=2, E'_1=30 eV; 8 LEP=2, E'_1=1e-3 eV; 9 LEP=1, NA=1, E'_1=0; 10 LEP=2, NA=1, E'_1=700 eV;
  11 ND=1 (no extension in either); 12 LEP=1, E'_1=0, second E' below 40 eV (nothing to insert).
- Port: `acer::acelf6::ismooth_extend_low_energy` (the block lifted out of `incident_block` unchanged, plus the guard).
  Test: `tests/acelf6_ismooth_vs_njoy2016_units.rs`.
- **Survey of `reference-data/endf/` (issue's first step, 2026-11-10):** a throwaway parser of every MF=6 section (LAW=1, LANG=2,
  neutron product `ZAP=1`) over all `*.endf` tapes there found **28 tapes** with such records (B-11, C-nat/C-12, O-16, Al-27, Si-28/29/30,
  P-31, Cl-35, Ar-37, Ca-40, Cr-50/52/53/54, Cu-63/65, U-234/235/236/238, ...), **all `LEP=1`, none with `ND>0`, and none with a first outgoing
  energy above 0** in any incident-energy record. So the 2016.80 guard is inert on every evaluation held here and the five-route parity gate
  cannot move; `LEP=2` occurs in none. The cases here are therefore synthetic, said plainly: they check the algorithm against upstream on
  identical input and say nothing about any evaluation. (Parser not committed; it reads MF=6 TAB1/TAB2/LIST counts only.)

## Prediction (before the port change)

Port (no guard) differs from 2016.80 exactly on the records with E'_1 > 0 that the unguarded branch extends (2, 3, 6, 7, 8, 10) and
equals 2016.79 in all 12; after the guard it equals 2016.80 in all 12.

## Results (2026-11-10)

| comparison | result |
|---|---|
| port with guard vs 2016.80 | **12/12** bit-identical (row count, every word) |
| port without guard (temporarily switched off) vs 2016.79 | **12/12** (the old port was a faithful 2016.79 translation) |
| port without guard vs 2016.80 | 6/12 (differs in 2, 3, 6, 7, 8, 10) |
| 2016.80 vs 2016.79 | differ in exactly 2, 3, 6, 7, 8, 10 |
| out-of-order outgoing energies | 2016.79 case 2 (E'_1 = 1 keV: 34 rows, inserted energies below 1 keV after it); 2016.80 none |

A first reference run had the `E` line cut at 60 words by a Fortran format, which hid case 2's 68-word output; fixed and
regenerated (the harness defect was found by comparing lengths, not by a mismatch).
Other `ismooth` sites checked, unchanged upstream between 2016.79 and 2016.80 and independent of E'_1: `acelf5` (`mt=18`, `jnt=2`,
`acefc.f90:6816`) and the delayed-neutron histogram extension (`:6181`, port `delayed_blocks.rs:193`).
