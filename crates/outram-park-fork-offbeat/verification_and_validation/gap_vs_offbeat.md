# FRAPCON fuel-rod gap conductance vs OFFBEAT 80e84450 (gh:#1393, round 2)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS_round2.md` E).
Route and caveats as in `conductivity_vs_offbeat.md` (standalone C++ transcription, `dump_gap.cpp`; fixture
`reference_gap.txt`, 6 184 rows; test `tests/offbeat_differential_gap.rs`; **not a run of the compiled library**).
`check_verbatim.sh` lists 17 lines not found; all are renames of field-indexed names (`pT[i]` -> `pT`, `M_` ->
`g.M`, `gapWidth_[i]` -> `gapWidth_`); the arithmetic is unchanged.  Compared: `gapFRAPCON::kappa` (mixture
conductivity), `::a` (accommodation coefficient), and the per-face gas, radiative and contact conductances of
`fuelRodGapFvPatchScalarField::hGap` (scaling `F = 1`, `delta = 0`, `relax = 1`).  Mixtures: seven dyadic
{He, Ar, Kr, Xe} compositions (sum exactly 1).  Not compared: gas-pressure / plenum bookkeeping, gap-width
geometry, TRISO gap, tabulated gap gas.

## Results (G1/G2 as in the other records; scales derived in the driver)

| quantity | compared | bit-identical | worst relative | G1 / G2 failures |
|---|---:|---:|---:|---|
| gas mixture conductivity `kappa(T)` | 49 | 40 | 1.9e-16 | 0 / 0 |
| accommodation coefficient `a(T)` | 49 | 41 | 1.7e-16 | 0 / 0 |
| `hGas` | 5 670 | 5 143 | 6.4e-16 | 0 / 0 |
| `hRad` with the CODATA 2018 sigma (the port's) | 144 | 144 | 0 | 0 / 0 |
| `hContact` | 128 | 128 | 0 | 0 / 0 |

## Finding: OpenFOAM's Stefan-Boltzmann constant differs from CODATA by +6.02e-6

`physicoChemical::sigma` in OpenFOAM-ESI v2506 is computed from the truncated controlDict constants
(`k = 1.38065e-23`, `h = 6.62607e-34`, `c = 2.99792e8`): 5.6704085580608236e-8 against the port's SI-2019
5.670374419e-8.  `hRad` is linear in sigma, so an OFFBEAT run has a radiative conductance larger by
a factor 1.000006021; the 144 transcribed `_Foam` rows reproduce that ratio to 4.4e-16 (prediction 2: held,
within the 4 eps gate).  Effect on the total gap conductance is below 6e-6 times the radiative fraction.  Kept at
SI-2019; deferred for the maintainer (gh:#1394).  Doc added at `STEFAN_BOLTZMANN` in `src/gap/conductance.rs`.

## Scorecard and notes

1 (gates): held.  3 (contact bit-identical): held, 128/128.  4 (no mixture-rule discrepancy): held.  5 (no port
defect): held.  Non-bit-identical gas rows come from `pow(.,0.5)`/`pow(.,2)` against `sqrt`/`r_powi` inside the
mixture rule and from `interfaceP/1e4/9.8` against `interfaceP/(1e4*9.8)`; not separately isolated.  The
species order of the sums is the port's (He, Ar, Kr, Xe); the upstream order is the dictionary's, which can move
a sum by an ulp.

Provenance: OFFBEAT 80e84450 (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat), accessed 2026-11-13; OpenFOAM-ESI
v2506 constants from `/home/user/upstream-build/OpenFOAM-v2506/etc/controlDict`.
