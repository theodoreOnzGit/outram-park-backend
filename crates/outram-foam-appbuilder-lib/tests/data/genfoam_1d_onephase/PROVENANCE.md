# Provenance: single-phase channel for the GeN-Foam `onePhase` comparison (gh:#1361 round 4)

- **Derived from:** GeN-Foam `Tutorials/featureCases/1D_boiling/` (mesh, `g`, `0/p`, `0/p_rgh`, `0/T.liquid` as `0/T`,
  `0/U.liquid` as `0/U`, `thermophysicalProperties.liquid` as `thermophysicalProperties`,
  `turbulenceProperties.liquid` as `turbulenceProperties`, `controlDict`) and `Tutorials/featureCases/1D_HX/onePhase/`
  (`system/fluidRegion/{fvSchemes,fvSolution}`), commit `652b3da`,
  <https://gitlab.com/foam-for-nuclear/GeN-Foam>. Copyright (C) 2015-2022 EPFL (GeN-Foam developers), GPL-3.0.
- **Accessed:** 2026-11-12, local build tree.
- **Written here:** `constant/fluidRegion/phaseProperties` (structure and closures of 1D_boiling in the flat form
  1D_HX/onePhase uses; heater `powerDensity` 4.5e7 W/m3, 1D_boiling has 4.5e8) and `regionSolvers fluidRegion onePhase` in
  `system/controlDict` (`maxCoTwoPhase` and `marginToPhaseChange` removed).
- **Not a shipped tutorial.** The real GeN-Foam binary reads these files unmodified (apart from the comparison protocol,
  `scripts/onephase_protocol.sh`), so the comparison is like for like; nothing here is a physical benchmark.
- **Use:** `tests/genfoam_one_phase_vs_upstream.rs`.
