# MATPRO fuel creep and power-law creep vs OFFBEAT 80e84450 (gh:#1393, round 2)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS_round2.md` F).
Route and caveats as in `conductivity_vs_offbeat.md` (standalone C++ transcription, `dump_creep.cpp`; fixture
`reference_creep.txt`, 18 234 rows; test `tests/offbeat_differential_creep.rs`; **not a run of the compiled library**).
`check_verbatim.sh`: 4 lines not found (`F`, `G`, `D` and `fTrialDev`, differing only in the field-look-up renames
`heatSource[cellI]` etc. or in a line break).  Compared: `MatproCreepModel` rate `fTrial` and Newton derivative `fTrialDev`
(both SakaiCorrection settings) and `powerLaw` `creepRate`.

Not compared: Limback, Tobbe, Malygin, Routbort, PARFUME and Zhang creep (stateful strain history and many fields; listed
as remaining), the implicit loop and the relaxation.  The stress `sigmaEff` is an input (>= 1e5 Pa: the port deliberately
omits upstream's lower clamp; `D > 90.5 %TD`: the port refuses below, upstream would divide through a sign change).

## Results (gate G1 `8 eps x scale`, `scale = value x (3 + max Q/(R T))`)

| quantity | compared | bit-identical | worst relative | worst G1 ratio | failures |
|---|---:|---:|---:|---:|---|
| MATPRO rate | 9 072 | 5 384 (59 %) | 7.3e-15 | 0.102 | 0 |
| MATPRO Newton derivative | 9 072 | 6 055 (67 %) | 7.4e-15 | 0.105 | 0 |
| power law (rate per second) | 90 | 90 | 0 | 0 | 0 |

G2 is never evaluated for the MATPRO rows: with `Q/(R T)` of 10 to 60 the derived condition number exceeds 5 everywhere,
so only G1 applies.  Worst relative 7.4e-15 (33 eps) is the `exp(-Q/R/T)` against `exp(-q/(R t))` argument rounding
amplified by `Q/(R T)`; inside the derived bound by a factor 10.  Association differences not separately isolated.
Predictions: 1 held, 2 held (59 % / 67 % bit-identical), 3 held (100 %), 4 held.  No port defect.

Provenance: OFFBEAT 80e84450 (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat), accessed 2026-11-13.
