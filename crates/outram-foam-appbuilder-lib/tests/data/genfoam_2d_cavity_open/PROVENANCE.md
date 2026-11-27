# Provenance: `genfoam_2d_cavity_open`, the second face-centred case of gh:#1878 item 5 (gh:#1361 round 38)

- **Derived from:** `tests/data/genfoam_2d_cavity_boussinesq` (the shipped `2D_cavityBoussinesq`, GeN-Foam 652b3da,
  <https://gitlab.com/foam-for-nuclear/GeN-Foam>, Copyright (C) 2015-2022 EPFL, GPL-3.0) with four edits, made so that the case
  needs ONLY what the round-4 `one_phase_fc` already ported (no closed volume, no Boussinesq, no `noSlip`):
  1. `constant/fluidRegion/thermophysicalProperties`: the round-4 channel's sodium package (`icoPolynomial`, `hPolynomial`,
     `polynomial` transport; `tests/data/genfoam_1d_onephase`) in place of Boussinesq `hConst`;
  2. `0/fluidRegion/p_rgh`: `top` `fixedValue 1e5`, `bottom` `fixedValue 100500` (an inlet), `fixedWalls` `zeroGradient` (not `fixedFluxPressure`);
  3. `0/fluidRegion/T`: `bottom` `fixedValue 520` (not 1000: a first variant with the shipped 1000 K and an all-`zeroGradient` bottom aborted upstream at t = 0.0074 s with a negative temperature, and 520 K with a `zeroGradient` bottom at t = 0.256 s; recorded, not hidden);
  4. `0/fluidRegion/U`: every wall patch `zeroGradient` (not `noSlip`).
- **Not a shipped tutorial and not a physical benchmark:** a 40 x 40 two-dimensional (`empty` front and back) face-centred flow,
  heated from the bottom wall, open at the top, which exercises the multi-iteration `PCG`/`DIC` solve that the 1-D channel
  (one iteration per solve) cannot. The real binary reads the files unmodified apart from `scripts/fc_protocol.sh`.
- **Use:** `tests/genfoam_one_phase_fc_vs_upstream.rs`.
