# Thermal expansion vs OFFBEAT 80e84450 (gh:#1393, round 2, case 1)

Measured 2026-11-13.  Predictions: `offbeat_differential_gh1393/PREDICTIONS_round2.md`
section A (committed before the driver was compiled).  Route and caveats as in
`conductivity_vs_offbeat.md`: standalone C++ transcription of the `correct()` bodies
(`dump_thermal_expansion.cpp`), fixture `reference_thermal_expansion.txt` (4 008 rows),
test `tests/offbeat_differential_thermal_expansion.rs`.  **Not a run of the compiled
OFFBEAT library.**  Upstream archive sha256
`7b5cde47ceba60903850f02e66b4eb6591cebfc7d272539b8189d625934b1e93` (round 1 recorded none).

`check_verbatim.sh` reports 20 lines not found; all are shim renames, not arithmetic
changes: `T[cellI]` written `T`/`Ti` (`const scalar Ti = T - 273.15;`, `BuGWd_tHM = Bu / 1000`,
Gehr's `if( T <= 293 )`), `alpha_.value()` etc. renamed in Constant, and MAMOX's `a0..a3`
member functions inlined into one function with the same body (`1e-3*(b0 + ...)`) and the
`dL_L0` sums written with the precomputed `a0v..a3v`.

Count: the port enum has 15 variants; 14 are OFFBEAT classes.  `MatproFthconUO2`
(gh:#1070, RELAP-7 manual) has no OFFBEAT class and is not compared.

## Gates (derived before the run)

G1: `|port - upstream| <= 8 eps * scale`; G2: relative < 1e-14 where `|result| >= 0.2*scale`
(`scale` = sum of |additive terms| of both operands; derivation in PREDICTIONS_round2.md A).
Result: **G1 and G2 hold at all 3 376 compared points, every model.** The first
draft of the derivation said "4 eps"; it was revised to "8 eps" before any run because
the 4 could not be derived from the Horner/pow forward-error bounds.

## Results

| model | compared | bit-identical | <= 4 ulp | worst relative | worst relative where cond <= 5 (G2 points) |
|---|---:|---:|---:|---:|---|
| Constant | 180 | 180 | 180 | 0 | 0 |
| PARFUMESiC | 180 | 180 | 180 | 0 | 0 |
| RelapUO2 | 90 | 90 | 90 | 0 | 0 |
| Molybdenum | 30 | 30 | 30 | 0 | 0 |
| MatproZy | 78 | 78 | 78 | 0 | 0 |
| SwindemanHastelloyN | 90 | 90 | 90 | 0 | 0 |
| PARFUMEPyC radial / tangential | 270 + 270 | 270 + 270 | all | 0 | 0 |
| PARFUMEBuffer (vs mag()-fixed twin) | 90 | 87 | 90 | 2.3e-16 | < 1e-14 |
| Gehr1515Ti | 28 (2 skipped, <=273 K window) | 26 | 27 | 2.0e-16 | < 1e-14 |
| MatproUPuO2 | 90 | 41 | 87 | 2.2e-15 | < 1e-14 |
| MartinUPuO2 | 270 | 116 | 253 | 1.4e-14 (near T=Tref cancellation) | < 1e-14 |
| SneadSiC | 90 | 36 | 82 | 1.1e-14 (near cancellation) | < 1e-14 |
| LemehovUPuO2 | 1 080 | 401 | 922 | 2.3e-13 (near cancellation) | < 1e-14 |
| MAMOX | 540 | 169 | 487 | 3.1e-13 (near cancellation) | < 1e-14 |
| **total** | **3 376** | **2 064 (61.14 %)** | | 3.1e-13 | |

The large relative values (Lemehov, MAMOX 3e-13; Martin and Snead 1e-14) sit at points where the strain is the
difference of two nearly equal operands (e.g. T = 293.0 K, Tref = 293.15 K), where the
condition number is ~1e3: they are within G1 and are the expected cancellation
amplification of ulp-level operand differences, not an error.

Two Gehr1515Ti rows are `-0.0` upstream (`nominalValue *= 0` on a negative value) against
`+0.0` in the port: the same number, a signed zero (reported by the ulp counter as 2^63).

## Localisation of the non-bit-identical rows (`localise_te.cpp`, `localise_te_output.txt`)

* MatproUPuO2, Martin, Snead, (and the polynomial inside MAMOX and Lemehov): upstream writes
  `p1 + p2*T + p3*pow(T,2) + p4*pow(T,3)`, the port Horner.  On the 30-point grid the two
  differ at 18 (MatproUPuO2, Celsius), 14 / 12 (Martin low / high) and 26 (Snead) points,
  and `pow(Tc,3) != Tc*Tc*Tc` at 7.  This is the first divergent operation.
* Gehr1515Ti: `par3*pow(Ti,2)` against `(par3*tc)*tc`: 1 of 30 points.
* PARFUMEBuffer: upstream subtracts `(T-273.15) - (Tref-273.15)`, the port `T - Tref`: 4 of 90
  pairs differ, 3 rows are not bit-identical.
* MAMOX and Lemehov: compound (a polynomial in `T`, whose coefficients are themselves
  response surfaces / `pow(Ti/Tm,k)` of a burnup-dependent `Tm`); the first divergent
  operation is the polynomial evaluation as above, but I did not isolate each of the
  intermediate roundings separately.  Not localised further.

## Prediction scorecard

* (1) G1/G2 hold: **held**.  (2) Constant, SiC, Molybdenum, Swindeman 100 %: **held**.
* (3) RelapUO2 predicted 1-3 ulp from `exp(-ED/k/Ti)` vs `exp(-ED/(k*T))`: **wrong**, 90/90
  bit-identical.  Reason (not isolated): the exponential term is 1e-6..1e-3 of the sum
  `K1*T - K2 + K3 exp(..)` over most of the grid, so ulp differences of the exponential are
  below the rounding granularity of the sum.
* (5) MatproZy blend-band 1-2 ulp: **wrong**, 78/78 identical; the blend band holds only
  4 temperatures x 3 Tref, a thin grid, so this is a weak match.
* (6) PARFUME dead-zone divergence: **held qualitatively**; upstream-as-written is zero below
  Tref for 105 of 630 (T, Tref, ...) rows and the port is non-zero at all 105.  I computed the
  number 105 from the grid (15 (T, Tref) pairs with T < Tref, times 1 buffer + 3 anisotropy x
  2 components) after seeing it; it was not written down beforehand.

## Findings

* **No port defect.**
* PARFUMEBuffer / PARFUMEPyC: upstream's dead zone omits `mag()`, so a case cooling below Tref
  gets **zero** thermal strain upstream.  The port deliberately corrects it (documented in the
  variant docs).  Now pinned by the test.  Whether to add a bug-compatible switch is a
  maintainer decision (deferred, gh:#1394).
* MatproUPuO2 upstream constructor reads `par5..par8` from the dictionary keys `par1..par4`
  (copy-paste defect).  Defaults are used on the grid; the port has no dictionary input.
  Recorded, not observable.
* Lemehov burnup basis: upstream divides the registry `Bu` (MWd/t-oxide) by 1000 and calls it
  GWd/tHM, with no 0.881 factor.  The test maps `state.burnup = Bu/1000` accordingly.  If the
  crate's burnup module feeds this model `Bu/1000/0.881` (MWd/kgHM), the Tm argument differs
  by ~13 %.  Listed for the maintainer (gh:#1394); not changed.

Reproduce:

```sh
cd crates/outram-park-fork-offbeat/verification_and_validation/offbeat_differential_gh1393
g++ -O2 -ffp-contract=off -o dump_thermal_expansion dump_thermal_expansion.cpp
./dump_thermal_expansion > reference_thermal_expansion.txt
cargo test --release -p outram-park-fork-offbeat --test offbeat_differential_thermal_expansion -- --nocapture
```

Data provenance: OFFBEAT sources (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat, commit
80e84450a115b0c411e1bfa5d166379f6bf6c084), accessed 2026-11-13; no other data.
