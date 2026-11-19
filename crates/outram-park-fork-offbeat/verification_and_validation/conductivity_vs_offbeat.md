# Thermal conductivity vs OFFBEAT 80e84450 (gh:#1393, case 1)

Measured 2026-11-13.  Predictions were committed first:
`offbeat_differential_gh1393/PREDICTIONS.md` (commit 1d7f05d53).

## Route (what was and was not run)

* **Not run:** the compiled OFFBEAT library.  `GeN-Foam-src/src/classes/offbeat`
  in this environment is a 4-file stub, and a library build needs the 829-directory
  tree on OpenFOAM-ESI (OFFBEAT `80e84450` targets ESI, the line of the rebuilt
  v2506).  On a 4-core shared box with 8 GB free that was not justified: every
  correlation `.C` is a per-cell scalar kernel whose only OpenFOAM content is
  `pow/exp/tanh/sqrt/min/max` on `double`.
* **Run:** `offbeat_differential_gh1393/dump_conductivity.cpp`, a standalone C++
  program (`g++ -O2 -ffp-contract=off`, glibc libm) whose kernels copy the
  `forAll(addr, i)` loop-body statements of `conductivity*.C` character for
  character; dictionary defaults are copied from the constructors.
  `check_verbatim.sh` greps every `// UPSTREAM` line into the upstream tree
  (fetched from gitlab.com/foam-for-nuclear/offbeat at the pinned commit).
  Lines that differ by design (mesh-field subscripts `Bu[cellI]`, `OM_[cellI]`,
  `Foam::sqrt`, a wrapped line) are written without the marker or are listed by
  the checker as expected misses: 14 on the conductivity driver, all of those
  kinds.  The swelling / densification / density-fraction bookkeeping that is
  not a statement of the loop body is unmarked.
* **Not exercised:** OFFBEAT constructors, registry look-ups, `perturb` / `parN`
  dictionary overrides, run-time selection.
* Fixture: `reference_conductivity.txt` (16 256 rows, `%.17g`, round-trips
  exactly), grid T (26 values, 290-3113 K, including non-round values) x
  burnup (0-1.2e5 MWd/t) x porosity (0-0.2) x O/M (1.94-2.0) x Gd (0-0.08) x Pu
  x Am, plus 81+81 swelling/densification rows.
* Port inputs are built by the documented unit conventions (burnup /1000 /0.881
  for the MATPRO/NFIR fits, `*1e-3` for the others, `oxygen_deviation = OM - 2`,
  densification = 3 x linear strain).  Points outside the port's temperature
  window are skipped (the port clamps; upstream extrapolates): 1 334 of 16 256.
* Test: `tests/offbeat_differential_conductivity.rs`; gate: relative difference
  < 1e-14 on every compared point (the issue's prediction); bit-identical and
  ulp counts reported.

## Results

| model | compared | bit-identical | <= 1 ulp | <= 4 ulp | worst relative | worst ulp |
|---|---:|---:|---:|---:|---:|---:|
| MatproUo2 | 1800 | 1800 | 1800 | 1800 | 0 | 0 |
| MatproUo2 (swelling/densification) | 81 | 81 | 81 | 81 | 0 | 0 |
| NfirUo2 | 1800 | 1800 | 1800 | 1800 | 0 | 0 |
| NfirUo2 (swelling/densification) | 81 | 81 | 81 | 81 | 0 | 0 |
| Ifa601Uo2 | 150 | 150 | 150 | 150 | 0 | 0 |
| BrancheriaMox | 96 | 87 | 94 | 96 | 3.4e-16 | 2 |
| KatoMox | 576 | 576 | 576 | 576 | 0 | 0 |
| LanningBeyerMox | 1728 | 1575 | 1683 | 1728 | 4.5e-16 | 4 |
| MagniMamox | 6912 | 6912 | 6912 | 6912 | 0 | 0 |
| PhilipponneauMox | 1512 | 1310 | 1467 | 1512 | 4.0e-16 | 3 |
| RelapZircaloy | 17 | 17 | 17 | 17 | 0 | 0 |
| Molybdenum | 23 | 23 | 23 | 23 | 0 | 0 |
| SwindemanHastelloyN | 10 | 10 | 10 | 10 | 0 | 0 |
| Tobbe1515Ti | 11 | 11 | 11 | 11 | 0 | 0 |
| ParfumeBuffer | 104 | 104 | 104 | 104 | 0 | 0 |
| ParfumeSiC | 21 | 21 | 21 | 21 | 0 | 0 |
| **total** | **14 922** | **14 558 (97.56 %)** | | | **4.5e-16** | 4 |

Not compared (no upstream counterpart or no kernel): `Constant`,
`MatproFthconUo2` and `MatproCthconZircaloy` (RELAP-7 manual models, not
OFFBEAT models; `MatproCthconZircaloy` reuses the Relap cubic, so it is covered
through `RelapZircaloy` below 2098 K).

## Interpretation

* **No port defect found in the conductivity set.**  Every compared point is
  within 4.5e-16 relative; the gate (1e-14) passes with a margin of 22x.
  `MatproUo2`, `NfirUo2` (including the discontinuous, probably dead gadolinia
  branch, reproduced faithfully), `Ifa601Uo2`, `KatoMox`, `MagniMamox` agree
  bit for bit, so PETIR's `r_exp` / `r_powf` / `r_tanh` reproduce glibc here
  for all their arguments on this grid.
* The 364 non-bit-identical points are three families, each localised to an
  operation (`localise_ulp.cpp`, output in `localise_ulp_output.txt`):
  1. `BrancheriaMox`, `PhilipponneauMox`: upstream `pow(Ti, 3.0)` against the
     port's `t*t*t`; `pow(T,3.0) != T*T*T` on 1 of the 26 grid temperatures
     (3000 K, the worst row).  libm `pow`, not a transcription error.
  2. `LanningBeyerMox`: upstream `C*pow(Ti,-2.0)` against the port's
     `C/(t*t)`, differing on 3 of 26 temperatures; the order `C * (1/T^2)` vs
     `C / T^2` is a re-expression in the port.  Predicted 1 ulp, observed
     up to 4 ulp (relative 4.5e-16) because the term is added to another of
     comparable size.
  3. `PhilipponneauMox`: upstream `Bu*1e-3/9.5/100` against the port's
     `bu/950` where `bu = Bu*1e-3`: differs on 2 of 6 burnups.
* Prediction scorecard (PREDICTIONS.md): (1) < 1e-14 everywhere **held**;
  (2) >= 70 % bit-identical **held** (97.56 %), within 4 ulp **held**;
  (3) no-transcendental models 100 % identical **held**; (4) MAMOX 1-2 ulp
  **wrong** (6912 / 6912 bit-identical, the `exp(-Bu/128.75e3)` vs
  `exp(-bu/128.75)` operand rounding does not bite on this grid), Philipponneau
  1-2 ulp **wrong** (up to 3 ulp); (5) swelling `3*s/3` <= 1 ulp **held**
  (all 162 volume rows bit-identical); (6) Kato dead code **held**.
* The port's deliberate deviations (temperature clamping, 0.05 floor in the
  volume correction, no dictionary overrides) are invisible on this grid by
  construction (grid inside the windows, volume factor > 0.05).  They are not
  tested here and remain as the crate documents them.

## Reproduce

```sh
cd crates/outram-park-fork-offbeat/verification_and_validation/offbeat_differential_gh1393
g++ -O2 -ffp-contract=off -o dump_conductivity dump_conductivity.cpp
./dump_conductivity > reference_conductivity.txt
cargo test --release -p outram-park-fork-offbeat --test offbeat_differential_conductivity -- --nocapture
```

Data provenance: OFFBEAT sources (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat,
commit 80e84450a115b0c411e1bfa5d166379f6bf6c084), accessed 2026-11-13 via the
repository archive; no third-party data beyond that code.  Licence of the
driver: derived from OFFBEAT, GPL-3.0 header kept.
