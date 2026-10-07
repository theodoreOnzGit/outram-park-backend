# Prediction, written 2026-10-07T02:00:52Z before any k_eff of this sweep was read

From `../htr10_seker_2026_10_05_majorant_fix/summary.md` (bounded majorant,
10 000 x [5 + 20], 4 VIII.0 points + 1 VII.0 point):

- **Shift new − old (vs `../htr10_seker_2026_10_01_10k/`, same statistics,
  old under-bound majorant):** about **−260 pcm** at every N on VIII.0
  (measured −263 ± 147), and about −300 pcm on VII.0 (one point, −332 ± 316).
  No height dependence expected: χ²/dof of the shifts about their mean ≈ 1.
- **Per-point σ:** about 105 pcm (the 10-01 record's at 135 active cycles), so
  each shift carries a combined σ of about 150 pcm.
- **Residual vs RMC:** the 10-01 record's VIII.0 residuals moved down by
  ~260 pcm; whatever slope with height that record had (gh:#218, +6.7 pcm/cm
  on the older bed) should survive unchanged.

If the shift is height-dependent beyond its σ, or its mean is outside
−260 ± 300 pcm, that is a finding to chase, not to explain away.
