# Provenance: `2D_cavityBoussinesq` as shipped (gh:#1878, gh:#1361 round 38)

- **Copied unmodified from:** GeN-Foam `Tutorials/featureCases/2D_cavityBoussinesq/` (`0/`, `constant/`, `system/`;
  `README.md` kept as `UPSTREAM_README.md`; `system/fluidRegion/blockMeshDict` omitted: the mesh is the shipped
  `constant/fluidRegion/polyMesh`), commit `652b3da`, <https://gitlab.com/foam-for-nuclear/GeN-Foam>.
  Copyright (C) 2015-2022 EPFL (GeN-Foam developers), GPL-3.0. Accessed 2026-11-27 from the local build tree.
- **Not changed.** The comparison protocol (tight solvers, `writePrecision 17`, `p_rgh` GAMG -> PCG/DIC, endTime) is applied to
  a per-test copy by the test and to the upstream run by `scripts/fc_protocol.sh`.
- **Use:** `tests/genfoam_one_phase_fc_vs_upstream.rs` (the face-centred 2-D case of gh:#1878).
