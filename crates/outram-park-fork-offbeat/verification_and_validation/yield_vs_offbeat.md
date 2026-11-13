# FRAPTRAN yield stress vs OFFBEAT 80e84450 (gh:#1393, round 2)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS_round2.md` D).
Route and caveats as in `conductivity_vs_offbeat.md` (standalone C++ transcription, `dump_yield.cpp`, cell part of
`yieldStressFRAPTRAN::correctYieldStress`; fixture `reference_yield.txt`, 11 286 rows; test
`tests/offbeat_differential_yield.rs`; **not a run of the compiled library**).  `check_verbatim.sh` lists 22
lines not found: all are renames of `m_[cellI]`/`n_[cellI]`/`Kcoeff_[cellI]` to locals or of
`Kcoeff_[cellI]`, `pow(..)` operands to the `Coef` struct fields; the arithmetic is unchanged.  Compared:
`sigmaY = max((K/E^n (rate/1e-3)^m)^(1/(1-n)), K eps_old^n (rate/1e-3)^m)`.  Not compared: `Hp_`.

Grid: 19 temperatures (300-2000 K incl. every branch edge), 11 fluences (0-15e25 n/m^2), cold work 0/0.5, `eps_old`
0/0.01/0.05, rate 1e-3/5e-3/0.1, E = 100 GPa; 3 762 points x 3 upstream variants.

## Results

| comparison | rows | result |
|---|---:|---|
| port vs `intended` chain (the `>=` reading the port implements) | 3 762 | **bit-identical on all 3 762**, worst relative 0 |
| port vs upstream as written, first call, fluence takes the same K branch | 1 026 | all agree within G1 |
| port vs upstream as written, first call, branches differ | 2 736 | all differ; relative difference 14.5 % to 184 % in sigma_y |
| port vs upstream, second call (`phiValue_` multiplied by 1e25 again) | 3 762 | 3 420 differ beyond G1 (second call is independent of the input fluence) |

Prediction scorecard: 1 (port == intended within the gates): held, bit for bit.  2 (differs from the first call exactly
where the chains differ): held (0 exceptions either way).  3, 4: held.

## Findings (upstream behaviour; port unchanged)

1. **The fluence factor of K is applied in the wrong branch by the upstream typo, and the port's own comment misdescribed
   it.**  Upstream's chain is `if (phi < 0.1e25) .. else if (phi <= 0.1e25 && phi < 2e25) .. else if (phi <= 2e25 &&
   phi < 12e25)`.  Evaluated as written, fluences in `(0.1e25, 2e25]` take the THIRD branch (`Kphi = 0.53236 +
   2.6618e-27 phi`, i.e. +0.53..+0.59 instead of the intended +0.003..+0.059), and `(2e25, 12e25)` and `>= 12e25`
   take no branch (no factor).  The port's comment said "for any fluence in (0.1e25, 12e25) no fluence factor is
   applied at all"; that is wrong for `(0.1e25, 2e25]`.  Comment corrected in place (strike-through) in
   `src/rheology/yield_stress.rs`.
2. **`phiValue_ *= 1e25;` runs on every call of `correctYieldStress`** (a member is mutated), so the fluence is
   multiplied by 1e25 on each step: from the second call on every cell sees a saturated fluence (n multiplier
   1.608953, no K factor).  Upstream results therefore depend on the number of calls.  The port is stateless and
   cannot reproduce this; it uses the single, intended fluence.
3. Above 12e25 n/m^2 the port holds the factor at its 12e25 value while upstream applies none.  Documented in the port.

Whether to add bug-compatible switches for 1 and 2 is **deferred for the maintainer** (gh:#1394); the port's default is
the physically intended correlation.  Not an input-tuned result: the comparison is against the transcribed code.

Provenance: OFFBEAT 80e84450 (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat), accessed 2026-11-13.
