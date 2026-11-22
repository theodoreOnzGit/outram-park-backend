# Octave control package differential (gh:#1433, epic #1336)

Compares `beta_testing::z_domain` against a **run** of GNU Octave's control package.
`PREDICTIONS.md` was committed (416fc84) before any run; `RESULTS.md` has numbers,
localisation, defects and gaps.

## Upstream and how it was run

- Octave 8.4.0, `octave-control` 4.0.1, installed with
  `apt-get install -y octave octave-control` (Ubuntu). The port cites control 4.2.2+
  commit `f39e2625` (GPL-3.0-or-later, Lukas F. Reichlin); `pkg install -forge control`
  failed here. **The reference version differs from the cited one**, and it matters for
  c2d `matched` (see RESULTS.md).
- Second reference run: `OCTAVE_OVERRIDE_DIR` shadows `@tf/__c2d__.m` / `__d2c__.m`
  with the pkg-control 4.3.0 files (cloned from
  `https://github.com/gnu-octave/pkg-control`, HEAD `7773b7b1`, accessed 2026-11-13),
  copied here as `pkg430_tf__c2d__.m` and `pkg430_tf__d2c__.m` with their upstream
  headers intact (the only edit: `tf (zpk (..))` -> `zpk (..)`, for 4.0.1 compatibility).
- `octave-cli --no-gui octave_driver.m > octave_reference.csv`, and with
  `OCTAVE_OVERRIDE_DIR=<dir containing @tf/>` -> `octave_reference_pkg430_matched.csv`.
  Inputs (random systems from `rand('twister', 1433)`, random input sequence) are printed
  in the CSV; the test never regenerates them. Values are `%.17g`.

## Cases

| # | case | Octave code | port code |
|---|---|---|---|
| 1 | 13 discrete tfs (the `filt([0 3],[1 4 2])` doc example, 10 random of orders 1-4 including 2 unstable, 2 with a0 != 1), random / step / impulse input, 40 samples | `filter`, `lsim`, `step`, `impulse`, plus Octave-side replicas of both operation orders | `DiscreteTransferFn::advance_one_sample` |
| 2 | 9 continuous tfs x T in {0.1, 0.5} x {zoh, tustin, prewarp (w0 = 1 rad/s), matched}; 4 discrete tfs x {tustin, prewarp, matched} d2c | `c2d`, `d2c`, `tfdata` | `to_discrete`, `to_continuous` |
| 3 | DC gain of the 9 continuous tfs | `dcgain` | `steady_state_gain` |

Not compared (no API): `pole`, `zero`, `bode`, `margin`, `nyquist`, `rlocus`.

Data provenance: all inputs are synthetic numbers invented for this test; the only
upstream material is the Octave binary output and the two GPL-3.0-or-later `.m` files
above. Research, education and V&V only (`RESPONSIBLE_USE.md`).

## Bookkeeping status

| axis | done |
|---|---|
| Documented | [ ] |
| Human-reviewed | [ ] |
