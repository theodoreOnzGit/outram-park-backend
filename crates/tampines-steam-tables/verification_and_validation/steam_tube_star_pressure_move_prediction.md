# Prediction, registered BEFORE bisecting the IF97 steam shock-tube move (gh:#1346, gh:#1349)

Written 2026-11-13, before any build or run for this investigation.

Observation to explain: `crates/tampines/tests/hem_coolprop.rs`,
`coolprop_and_if97_agree_on_a_single_phase_water_shock_tube` (500 K,
0.2 | 0.1 MPa, 40 cells, 0.5 ms). IF97 star pressure (cell 20)
0.139940 MPa (2026-10-16, 3037f2036) -> 0.140066 MPa (2026-11-13, HEAD);
CoolProp side unchanged at 0.139938 MPa.

## Reasoning from reading the code, not from measuring

- The suspect commit 7be8a7d24 corrected T(p,s) 2b, T(h,s) 2b/2c, T(p,s) 3a
  and p(h,s) 3a. At 0.1-0.2 MPa, 500 K the state is Region 2a (p < 4 MPa), and
  IF97 T(p,s) selects 2b only for p > 4 MPa. The HEM `(rho, e)` closure
  (`hem/iapws97.rs`) is a Brent root over the explicit `(p,h)` flash
  (`v_ph_eqm`), with the sound speed from `w_ph_eqm_hem`; the backward
  equations enter only through `T(p,h)` (2a here, not touched by that commit).
  So I expect that commit to be bit-identical on this tube.
- A 126 Pa shift is 1.3e-3 of the 0.1 MPa jump: far above round-off (1e-12
  relative), so a libm/PETIR round-off change (gh:#985) is not expected to
  produce it either.

## Prediction

1. Probability that 7be8a7d24 (or its parent) is the culprit: **about 10 %**.
2. Most likely culprit (about 60 %): a commit in
   `crates/tampines-steam-tables/src/hem/` that changed the pipe's default
   path, i.e. the 2026-11-01 batch (variable area 30d01d0a2, friction/gravity
   1211cc9de, networks 2c9b7a02e), through a changed default (time step, area
   term, flux), not through the steam tables.
3. Remaining about 30 %: a change to the `(p,h)` flash / `w_ph_eqm_hem` /
   Region 2 code on the shared path (e.g. the 2026-11-10 `lambda_ph_eqm`
   or Kieffer sound-speed commits, or #1111 choke work).
4. Sign and size: whichever commit it is, the move is +126 Pa in the star
   pressure, a step change at one commit (not drift over many), and the IF97
   side should then sit 290 Pa from CoolProp, i.e. the agreement got WORSE,
   so unless the cause is a correction it should be examined as a possible
   regression.
