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

## setFields: results (2026-11-14, release, `tests/set_fields_vs_esi_gh1384.rs`)

| prediction | outcome |
|---|---|
| S1 counts 32/48/32/32 | held: port and ESI select 32, 48, 32, 32 of 192 cells (and 1, 0, 2, 2 of 6 on the small case) |
| S2 header + 192-entry internal list identical | held |
| S3 nonuniform `calculated` boundary `value` differs | **held in kind, not measured as a miss**: reading ESI's output before the first port run showed a long-form boundary list (`value nonuniform List<scalar> \n48\n(\n...)\n;`) where `field.rs` wrote one line, so the ESI writer was changed first (`esi_nonuniform` in `outram-foam-basic-lib/src/io/field.rs`, ESI flavour only; default layout unchanged). I did NOT run the old writer against ESI, so no "before" byte count exists. |
| S4 `U` byte-identical | held: 1 of 1 |
| S5 `T` not identical until fixed | held after the fix: `T` 1 of 1 |

Also found by running ESI on a 6-cell mesh (not predicted): lists of up to 10
elements are written inline, `nonuniform List<scalar> 6(a b ...);`, for the
internal field and the boundary `value` alike; the ESI flavour of the writer
now follows `UList::writeList` (`inline_list`). Pinned by
`set_fields_esi_flavour_small_lists_are_written_inline_like_esi`.

Totals: 2 cases x {T, U} = **4 of 4 files byte-identical** to ESI v2506
`setFields`. The `Info` log text is not byte-compared (the port prints its own
short log).

Not covered: face sources, tensor fields, finite-area, field BC types the
reader does not support (it errors, it does not guess).

## topoSet: predictions (written 2026-11-14 BEFORE the port's topoSet output was compared)

ESI `topoSet` was run once on `tests/cases/esi_v2506/set_fields/case/system/topoSetDict`
(cellSets c0..c3 = new/add/subtract/invert/subset/clear; ESI log read: sizes
32, 80, 68, 32 then 160 (invert), 80 then 40 (subset), 2 then 0 (clear)). The
port's `LabelHashSet` was written from the ESI `HashTable.C` source, NOT
fitted to those files. Two actions were added after that first run: c4 (new,
then add, which forces a re-read of an 8-label sparse column: labels 0, 12,
24, ..., 84) and c5 (new only). Predictions:

- T1. Set sizes after every action equal ESI's.
- T2. c0..c3 files byte-identical (dense, labels below the capacity, so the
  write order is ascending).
- T3. c5 (written straight from the 16384-bucket new set) is ascending:
  `8(0 12 24 36 48 60 72 84)`.
- T4. c4 after the re-read has 16 buckets (reserve 8 gives 16), so labels
  collide modulo 16 and the write order is bucket order, NOT ascending. The
  model's order is the prediction; agreement with ESI on c4 is the test of the
  model. If it differs the model is wrong, not the file.
