# `changi::activity` — what is and is not verified

**There is no upstream and no code-to-code verification for this module.** It is
not a port. Nothing here has been compared against measured atmospheric
dispersion. Manufacturing an "expected" value would be worse than having none.

## Methodology

Consistency checks (`tests/activity_properties.rs`, plus unit tests in
`src/activity/`): linearity in released activity, conservation under
re-segmentation, exact decay weighting per travel-time bin, exact zero
deposition for noble gases, exact scaling with deposition velocity, and the
sign reversal of ground-level vs breathing-height concentration with distance.

One check is stronger: the binned unit response, and a full source term through
`survey`, agree with `puff::simulate::simulate_sensor_mode` (itself checked
against upstream R) to better than 1e-12 relative (measured 2026-09-21).

## Results worth knowing

- Class D, 500 m, 30 m release, 4 m/s: chi/Q = 3.04e-5 s/m3 against a
  continuous-plume hand estimate of 3.2e-5.
- Two property tests failed first and both were the test's fault (a fixed
  stability class against a wind-derived one, 28 % out; and a wrongly assumed
  sign for breathing height vs ground level).

## Limits

Dry deposition only (not an upper bound); deposition is diagnostic, not
depleting; no daughter ingrowth, plume rise, building wake or iodine
speciation; Pasquill-Gifford is fitted ~0.1-10 km; puffs are dropped at
`puff_duration`. **Deposition velocities are uncited order-of-magnitude
placeholders** (`order_of_magnitude_placeholder`); no source has been read.
NUREG/CR-4691, IAEA SRS-19 and Sehmel (1980) are candidates, unconsulted.
No dose quantity is computed.

## Not done

~~The plan's day-3 end-to-end *example* was not written (an automated safety
classifier stopped that response). The chain is demonstrated by the integration
tests instead.~~ **CORRECTED 2026-09-21** — written later the same day:
`examples/site_activity_survey.rs` carries a prescribed 1 Ci each of Kr-88 and
I-131, released over one hour from 30 m in Pasquill class D at 4 m/s, and
prints the release (Bq, Ci), time-integrated air concentration (Bq·s/m³) and
dry deposition (Bq/m²) at 100 m – 8 km. `sembawang`'s
`examples/npmhtgr_release.rs` is the source-term half, in Bq and Ci. Neither
computes a dose.

Hand check, 2026-09-21: at 1 km the survey gives I-131
`8.68e5 Bq·s/m³`. The textbook ground-level Gaussian-plume value with
Briggs-rural class-D sigmas (σy ≈ 68 m, σz ≈ 32 m at 1 km),
`Q/(π σy σz u)·exp(−H²/2σz²)` with Q = 3.7e10 Bq, u = 4 m/s, H = 30 m, is
≈ `8.7e5 Bq·s/m³`. That is an order-of-magnitude sanity check by hand, not a
verification: the sigmas were read from memory of the standard curves, not
from a cited table.
