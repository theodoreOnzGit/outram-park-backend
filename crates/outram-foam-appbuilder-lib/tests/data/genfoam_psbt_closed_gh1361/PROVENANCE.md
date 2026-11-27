# genfoam_psbt_closed_gh1361: provenance

AI-written, not human-reviewed. Research, education and V&V only (`RESPONSIBLE_USE.md`).

**Source.** GeN-Foam `652b3da0cc555ffcb4a5eb2dcbb2b0ab82c7c9d1`, GPL-3.0, EPFL,
<https://gitlab.com/foam-for-nuclear/GeN-Foam>, tutorial
`Tutorials/featureCases/1D_PSBT_SC/Phase_Ex1_12223` (a PSBT rod-bundle Phase I Exercise 1 case; the
mesh, `phaseProperties`, `thermophysicalProperties.*`, `turbulenceProperties.*`, `fvSchemes`,
`fvSolution`, `controlDict`, `g` and every other file are unmodified). Copied 2026-11-26 from the
local checkout `/home/user/upstream-build/GeN-Foam-src` (no download). No data beyond the tutorial.

**Why it is not the shipped case, and what was changed.** The shipped case is a through-flow case with
a velocity inlet. The port's `TwoPhaseCase::read` refuses, by name, `fixedValue` velocity patches,
`slip` walls, `pressureInletOutletVelocity`, and (silently, see gh:#1361 round 8) treats
`fixedFluxPressure` and `inletOutlet` as `calculated`. Only the boundary-condition files below were
rewritten, to the boundary conditions `1D_boiling` (the case the solver was built for) uses, so that
upstream and the port run the SAME modified case. The closures, models, mesh, power, initial fields
and solver settings are the shipped ones.

| file (`0/fluidRegion/`) | shipped | here |
|---|---|---|
| `U.liquid`, `U.vapour` | inlet `fixedValue (4.467 0 0)` / `zeroGradient`; outlet `pressureInletOutletVelocity`; walls `slip` | all patches `zeroGradient` |
| `p_rgh` | inlet `fixedFluxPressure`; outlet `fixedValue 14719781.65`; walls `fixedFluxPressure` | inlet `fixedValue 14739781.65` (a 2e4 Pa drive), outlet `fixedValue 14719781.65`, walls `zeroGradient` |
| `alpha.vapour` | inlet, outlet `inletOutlet`; walls `zeroGradient` | all patches `zeroGradient` |

`T.liquid`, `T.vapour` (inlet `fixedValue 592.75`, others `zeroGradient`) and `p` (`calculated`) are
the shipped files. The initial `U.liquid` is the shipped `(4.467 0 0)`.

This is therefore NOT a validation of the PSBT experiment and not a comparison against the shipped
tutorial's results: it is a code-to-code comparison on a pressure-driven variant of the PSBT channel.
