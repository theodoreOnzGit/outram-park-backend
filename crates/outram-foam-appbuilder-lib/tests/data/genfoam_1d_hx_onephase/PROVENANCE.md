# Provenance: `featureCases/1D_HX/onePhase` as shipped (gh:#1847, GeN-Foam port round 44)

- **Copied unmodified from:** GeN-Foam `Tutorials/featureCases/1D_HX/onePhase/` (`0/`, `constant/`, `system/`;
  `system/fluidRegion/blockMeshDict` kept; the mesh is the shipped `constant/fluidRegion/polyMesh`), commit `652b3da`,
  <https://gitlab.com/foam-for-nuclear/GeN-Foam>. Copyright (C) 2015-2022 EPFL (GeN-Foam developers), GPL-3.0.
  Accessed 2026-11-27 from the local build tree. `Allrun`, `Alltest`, `Allclean` and `residuals` (gnuplot) are not needed.
- **Not changed.** The comparison protocol (tight solvers, `writePrecision 17`, `p_rgh` GAMG -> PCG/DIC, endTime) is applied to
  a per-test copy by the test and to the upstream run by `scripts/fc_protocol.sh`.
- **Use:** `tests/genfoam_heat_exchanger_vs_upstream.rs` (heat exchangers, gh:#1847).
