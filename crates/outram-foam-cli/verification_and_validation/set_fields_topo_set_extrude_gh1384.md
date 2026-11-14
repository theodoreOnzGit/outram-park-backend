# setFields / topoSet / extrudeMesh vs ESI v2506 (gh:#1384)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

Upstream: ESI OpenFOAM-v2506 `setFields`, `topoSet`, `extrudeMesh`, built by
`scripts/upstream/build-esi-v2506-and-genfoam.sh utilities` (514 s, 2 jobs;
adds libfiniteArea and libgenericPatchFields, which `setFields` links).

## setFields: predictions (written 2026-11-14 BEFORE the ESI run or the port output was diffed)

Case: `tests/cases/esi_v2506/set_fields/case`, 12x8x2 box mesh (192 cells,
ESI blockMesh), fields `T` (fixedValue / zeroGradient / calculated / empty) and
`U` (fixedValue / zeroGradient / fixedValue / empty), `setFieldsDict` with
defaults, then boxToCell, sphereToCell, cylinderToCell, boxToCell.

- S1. Selected-cell counts agree between the port and ESI for all four regions
  (same bit-identical cell centres, same inequalities).
- S2. Internal values of `T` and `U` are identical numbers; the `-flavour esi`
  header and 192-entry internal list are byte-identical.
- S3. The `walls` patch of `T` is `calculated`, so after `pfld =
  patchInternalField()` it carries a NONUNIFORM `value`. ESI prints a
  long-form list there; the port's one-line form (flagged "NOT verified" in
  `field.rs`) will DIFFER. This is the first predicted divergence. The
  `left` fixedValue and `right` zeroGradient blocks are identical.
- S4. `U` (no nonuniform boundary values) is byte-identical: 1 of 1.
- S5. `T`: 0 of 1 byte-identical until the nonuniform boundary layout is fixed.
