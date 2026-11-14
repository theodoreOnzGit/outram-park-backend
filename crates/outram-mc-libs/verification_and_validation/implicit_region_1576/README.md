# Implicit intersection in a region (GitHub #1576)

**Defect.** `Cell::contains` evaluated a region of bare half-spaces by its last
token alone. OpenMC inserts `OP_INTERSECTION` between adjacent operands when it
parses a region (`src/cell.cpp:626`, OpenMC `8f79d9d0`). Fixed 2026-11-14:
operands left on the stack are ANDed. The plot-bounds region tree
(`plot::model_plot::bounds::region_tree`) folds them the same way.

**Test:** `tests/implicit_region_1576.rs` (+ `Cell::contains` unit test
`bare_halfspaces_are_an_implicit_intersection`). Pass criteria were fixed before
running: identical pixels to the explicit-operator region; fuel area within 1 %
of analytic.

**Results (2026-11-14, release, 360 x 360 px over 12 cm):**

| slice | fuel pixels | area cm2 | analytic cm2 | difference |
|---|---|---|---|---|
| xy z=0 (pi R^2, R = 3) | 25448 | 28.2756 | 28.2743 | +0.004 % |
| xz y=0 (2R x 2H, H = 4) | 43200 | 48.0000 | 48.0000 | +0.000 % |

Implicit and explicit regions differ in **0 pixels** on both slices. Images:
`implicit_xy_z0.png`, `implicit_xz_y0.png` (drawn from the assembled geometry).

**Audit of hand-built operator-less regions.** A temporary constructor probe
(removed) logged every `Cell::material`/`Cell::fill` whose region leaves more
than one operand, across the `outram-mc-libs --lib` suite (544 tests run):
one site, an inline test in `physics/transport_csg_distributed.rs`. A static
scan of every file mentioning `RegionToken` found one more,
`tests/collision_track_output.rs`. `nee_soon` builds regions with its `Rgn`
builder (explicit operators), the `dhoby-ghaut` examples have none, and the
presets / pincell / export struct literals carry explicit `Intersection`.
Both found sites are now explicit. Their meaning did not change: each is a
two-sphere shell listed after its inner ball, so `locate` took the ball first
and the shell's last-token rule agreed with the correct one everywhere it was
reached; no picture changes. Transport k is unchanged (tracking uses surface
distances; `collision_track_output` still passes bit for bit). Not audited:
`nee_soon --lib` ran only partly (killed during unrelated thermal-hydraulics
tests).
