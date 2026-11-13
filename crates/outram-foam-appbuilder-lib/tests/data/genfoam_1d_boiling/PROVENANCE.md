# Provenance: GeN-Foam `1D_boiling` tutorial case

- **Source:** GeN-Foam, `Tutorials/featureCases/1D_boiling/`, commit `652b3da`
  ("docs: Add archive banner"), <https://gitlab.com/foam-for-nuclear/GeN-Foam>.
- **Authors / copyright:** (C) 2015-2022 EPFL (GeN-Foam developers).
- **Licence:** GPL-3.0 (GeN-Foam), compatible with this repository.
- **Accessed:** 2026-11-04, shallow sparse clone.
- **Files copied unchanged:** `0/`, `constant/`, `system/controlDict`,
  `system/fluidRegion/`, and the case `README.md` as `UPSTREAM_README.md`. Not
  copied: `Allrun`, `Alltest`, `Allclean`, `residuals` (scripts).
- **Use:** input to `tests/genfoam_two_phase_1d_boiling.rs` (gh:#1218). The case
  ships no reference results; see
  `verification_and_validation/genfoam_two_phase/README.md`.
- **Known quirk kept as shipped:** `0/fluidRegion/alpha.liquid` declares its
  inlet/outlet patches as `type calulated` (sic). The port's reader treats any
  type it does not know as `calculated`.
