# Young/Poisson MATPRO Zircaloy, PARFUME, Snead SiC and constants vs OFFBEAT 80e84450 (gh:#1393, round 2)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS_round2.md` C).
Route and caveats as in `conductivity_vs_offbeat.md` (standalone C++ transcription, `dump_zy_parfume.cpp`;
fixture `reference_zy_parfume.txt`, 1 432 rows; test `tests/offbeat_differential_zy_parfume.rs`;
**not a run of the compiled library**).  `check_verbatim.sh`: 1 line not found (`return 1e9*nominalValue;`
is the driver's return form of upstream's `sf[cellI] = 1e9*nominalValue;`).

## Results (gates G1/G2 as in the thermal-expansion record)

| model | compared | skipped | bit-identical | worst relative | G1 / G2 failures |
|---|---:|---:|---:|---:|---|
| E MATPRO Zircaloy (oxC, cW, fluence) | 240 | 0 | 240 | 0 | 0 / 0 |
| E PARFUME buffer | 96 | 0 | 96 | 0 | 0 / 0 |
| E PARFUME PyC | 384 | 192 (density outside the port's 1800-2000 kg/m^3 window) | 315 | 3.8e-16 | 0 / 0 |
| E PARFUME SiC | 10 | 0 | 10 | 0 | 0 / 0 |
| E Snead SiC (porosity, C) | 30 | 0 | 30 | 0 | 0 / 0 |
| Nu MATPRO Zircaloy, unit-consistent | 240 | 0 | 240 | 0 | 0 / 0 |
| Nu MATPRO Zy, as written, zero fluence or beta phase | gated | | | | 0 / 0 |
| Poisson constants UO2 / UPuO2 / Zy / Mo | 4 | | 4 | 0 | (literal upstream defaults) |

No port defect.  The Young's-modulus Zircaloy model has the same operation order as upstream and agrees
bit for bit on all 240 rows, including non-zero fluence (`exp(-phi/1e25)`), cold work and oxygen content.

## Finding: upstream's Young/Poisson Zircaloy pair is unit-inconsistent, and it matters

`YoungModulusMatproZy.C` scales the fluence field by `1e4` before `exp(-phi/1e25)`;
`PoissonRatioMatproZy.C` does not.  The port uses n/m^2 in both (documented).  Against upstream *as
compiled* the alpha-phase Poisson ratio differs: `(nu_up+1)/(nu_port+1) = K3(phi_cm2/1e25) /
K3(phi_cm2*1e4/1e25)`, `K3(x) = 0.88 + 0.12 exp(-x)`.  Predicted before the run (8.2 % at 1e21 n/cm^2);
observed on 84 alpha-phase rows with fluence: closed form reproduced to 2.2e-16, largest ratio 1.1353
(5e21 n/cm^2); the blend band shows the same 13.5 % at that fluence; zero fluence and the beta phase show none.
The port's doc sentence "immaterial in the alpha phase" was true only inside the port's own convention and
is qualified in `poisson_ratio.rs` (strike-through).  Whether to add a bug-compatible switch is
**deferred for the maintainer** (gh:#1394).

## Prediction scorecard

1 held.  2 (Young MATPRO Zy bit-identical >= 80 % with fluence): held, 100 %.  3 (closed-form ratio): held to
2.2e-16.  4 (Buffer, SiC, Snead 100 %; PyC < 100 %): held (PyC 315/384; the PyC multiplication order differs
from upstream).  5 (no port defect): held.

Not compared: isotropic-cracking branches of the UO2/MOX Young/Poisson models (need the slice mapper and
`lhgr`).  Provenance as in the other records (OFFBEAT 80e84450, GPL-3.0, accessed 2026-11-13).
