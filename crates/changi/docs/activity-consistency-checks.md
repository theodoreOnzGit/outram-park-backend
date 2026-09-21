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

The plan's day-3 end-to-end *example* was not written (an automated safety
classifier stopped that response). The chain is demonstrated by the integration
tests instead.
