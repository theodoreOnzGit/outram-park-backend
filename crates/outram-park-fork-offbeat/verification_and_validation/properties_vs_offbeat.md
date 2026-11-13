# Heat capacity, density, Young's modulus, Poisson ratio, emissivity vs OFFBEAT 80e84450 (gh:#1393, case 1)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS.md`).
Route, caveats and licence are those of `conductivity_vs_offbeat.md` (standalone
C++ transcription of the `correct()` loop bodies, not a run of the compiled
library).  Driver `dump_properties.cpp`, fixture `reference_properties.txt`
(4 234 rows), test `tests/offbeat_differential_properties.rs`.  `check_verbatim.sh`
lists 1 expected miss (`3.349E+11 - 5.101E+07*T[cellI]`, written `T`).

## The gas constant (the one real finding)

OFFBEAT's MATPRO oxide heat capacities (`heatCapacityMatproUO2`, `heatCapacityMatproUPuO2`) take
`R = Foam::constant::physicoChemical::R.value()`.  Read from the rebuilt OpenFOAM-ESI v2506
(`/home/user/upstream-build/OpenFOAM-v2506`): `R = NA * k`, with
`NA = 6.0221417930e+23` (`src/OpenFOAM/global/constants/fundamental/fundamentalConstants.C:121`)
and `k = 1.38065e-23` (`etc/controlDict:1116`, SICoeffs physicoChemical), i.e.
`R = 8.31447006650545 J/(mol K)`.  The port hard-codes CODATA 2018
`8.314462618153240`.  Relative difference `8.96e-7`.  The constant is a property of
the OpenFOAM build (ESI and Foundation differ), not of OFFBEAT.  Not changed in the
port (the CODATA value is the more accurate one); recorded as a **deferred decision**
on gh:#1394.  Effect on Cp: up to `2.27e-6` relative at 3000 K, O/M 2.03 (MATPRO UO2 and MOX),
bounded by the first-order sensitivity `|dR/R| (ED/(R T) + 1)`; observed / bound = 0.29.

A gate-bound note: my first gate for these rows was a flat 2e-6, a guess; it was exceeded
(2.27e-6) and replaced by the derived first-order bound before any other change.  The
observed numbers are unchanged.

## Results (port against the transcribed kernels)

Gate: relative difference < 1e-14 on every compared point except the `*_RFoam` rows.
Points outside a model's validity window skipped.

| model | compared | bit-identical | <= 1 ulp | <= 4 ulp | worst relative |
|---|---:|---:|---:|---:|---:|
| Hc MatproUo2 (R 2018) | 108 | 34 | 96 | 108 | 3.0e-16 |
| Hc MatproMox (R 2018) | 324 | 135 | 291 | 324 | 3.6e-16 |
| Hc MatproUo2 (R as built in OpenFOAM) | 108 | 8 | 12 | 16 | 2.27e-6 |
| Hc MatproMox (R as built in OpenFOAM) | 324 | 28 | 36 | 48 | 2.27e-6 |
| Hc FinkMox | 27 | 20 | 24 | 27 | 4.4e-16 |
| Hc MatproZircaloy | 25 | 25 | 25 | 25 | 0 |
| Hc IaeaZircaloy | 25 | 25 | 25 | 25 | 0 |
| Hc Banerjee1515Ti | 18 | 18 | 18 | 18 | 0 |
| Hc Molybdenum | 26 | 26 | 26 | 26 | 0 |
| Hc SneadSiC | 27 | 26 | 27 | 27 | 1.7e-16 |
| Rho IaeaZy | 24 | 24 | 24 | 24 | 0 |
| Rho Schumann1515Ti | 29 | 10 | 20 | 29 | 3.5e-16 |
| E MatproUo2 | 108 | 60 | 80 | 108 | 5.6e-16 |
| E MatproMox | 1296 | 726 | 966 | 1296 | 8.1e-16 |
| E SckCenMox | 1392 | 985 | 1374 | 1392 | 5.0e-16 |
| E HofmanD9 | 18 | 18 | 18 | 18 | 0 |
| E BisonMolybdenum | 26 | 26 | 26 | 26 | 0 |
| E Tobbe1515Ti | 18 | 18 | 18 | 18 | 0 |
| E WatrousHastelloyN | 18 | 16 | 18 | 18 | 1.6e-16 |
| Nu Tobbe1515Ti | 18 | 18 | 18 | 18 | 0 |
| Emissivity RelapUO2 | 29 | 29 | 29 | 29 | 0 |
| **total** | **3 988** | **2 275 (57.05 %)** | | | |

Excluding the 432 R-as-built rows: 3 556 compared, 2 239 bit-identical (62.97 %), worst
relative 8.1e-16 (E MatproMox, 3000 K, p 0.2, O/M 2.03, Pu 0.3).

## Interpretation

* **No port defect** in these families: every R-2018 and non-R row is within 8.1e-16.
* **Localised causes** of the non-bit-identical rows (`localise_ulp_output.txt`):
  * Young `MatproUo2` / `MatproMox`: upstream computes `(1 - densityFrac_)` from the dictionary
    density fraction; the port takes porosity directly. `1-(1-p) != p` for 2 of 4 grid
    porosities (an operand-representation difference of the interface, not an error).
  * Young `SckCenMox`: upstream applies the porosity factor as `nominal *= A/B`, the port as
    `((nominal*A)/B)`; `(X*A)/B != X*(A/B)` on 30 of 78 samples.
  * Density `Schumann1515Ti`: `pow(b,3)` against `b*b*b` (the port's `r_powi`), 8 of 26
    temperatures.
  * Density `IaeaZy`: the port writes the blend as `rho1 + (rho2-rho1)*w` with
    `w = (T-1083)/(1144-1083)`, upstream as `(rho2-rho1)*(T-1083)/(1144-1083)`
    (different association). Four grid temperatures fall in the 61 K blend band (1085.5,
    1090, 1100, 1120 K) and all four are bit-identical, so the grid is thin here
    (a weak match, not a proof of equivalence).
  * Heat capacities (MATPRO oxides, Fink), 1-2 ulp: **not localised to one operation**.
    The port groups the Einstein term differently from upstream's `pow(...)`/`exp`
    expression; this is the likely cause and has not been isolated.
* Prediction scorecard: (1) < 1e-14 **held** for all non-R-constant rows; (2) >= 70 %
  bit-identical **missed** (62.97 %; 4 ulp bound **held**); (3) no-transcendental models 100 %
  **held**.
* A prediction I did not write down and that was wrong: I expected the R constants to
  agree to better than 1e-9; they differ by 9e-7.

## Not compared in this pass (listed on gh:#1394)

Thermal expansion (15 models), Young MatproZircaloy / PARFUME / Snead, Poisson MatproZircaloy
and the constant models (they need `oxC_`, `cW_`, fluence and cold-work fields), the
isotropic-cracking branches of the Young/Poisson models (need the slice mapper and `lhgr`),
densification, relocation and swelling models, creep and yield-stress models, gap conductance,
corrosion.
