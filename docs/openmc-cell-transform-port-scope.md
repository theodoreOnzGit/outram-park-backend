# OpenMC cell transforms in `outram-mc-libs`: what is ported, what is not

**Written 2026-09-17.** Covers `openmc::Cell`'s coordinate-transform members —
`translation_` and `rotation_` — plus the rest of `Cell`'s data, so the next
person can see the boundary of the port without re-deriving it.

**Class: verification scoping.** Nothing here is validated, and no comparison
against a running OpenMC was made (OpenMC is not installed in the environment
this was written in). Upstream was read from
`openmc-dev/openmc@develop`: `src/cell.cpp`, `include/openmc/cell.h`,
`src/geometry.cpp`, `include/openmc/particle_data.h`.

## Summary

| OpenMC member | Status in `outram-mc-libs` |
|---|---|
| `Cell::translation_` | **Ported, exercised and verified** — this change |
| `Cell::rotation_` | **Not ported.** No field, no code path. See below |
| `LocalCoord::rotated_` | Not ported (only meaningful with rotation) |
| `Cell::material_` (vector, distribcell) | Single material only — `CellFill::Material(usize)` |
| `Cell::sqrtkT_` (vector) | Single `temperature: f64` per cell |
| `Cell::density_mult_` | Not ported |
| `Cell::distribcell_index_`, `offset_` | Not ported — no distribcell tallies |
| `Cell::name_` | Not ported (ids only) |
| `Region` RPN, `contains`, `distance` | Ported, incl. the `on_surface` override |
| DAGMC / CAD cells (`DAGCell`) | Not ported, and out of scope for a CSG-only crate |

## Translation — what was actually wrong, and what it cost

The field `Cell::translation`, its two application sites in `Geometry::locate`
(mirroring `src/geometry.cpp:222` for a universe fill and `:241` for a lattice
fill) and its doc comment were **already present** before this change.

**It had never been set to a non-zero value anywhere in the workspace, and no
test exercised it.** A workspace-wide search for `translation:` outside
`Position::ZERO` returned only unrelated crates. That is the shape of defect
bead `op-50vu` records: a mechanism that is present in the source, absent from
every result, and indistinguishable from working because nothing ever asked it
a question it could get wrong. A field that is always zero is not a ported
feature; it is untested code that happens to compile.

Writing the first test that sets it non-zero immediately failed, and the cause
was **not** in the translation code.

### The defect the tests found: a reconstructed frame offset

`Geometry::cross_surface_in_frame` must apply a boundary condition in the frame
the surface is declared in. It obtained the global→local offset for the
crossing level by **subtracting two stored positions**:

```rust
let offset = path.levels[0].r - path.levels[coord_level].r;
```

That is catastrophic cancellation. With a probe at `y = -9` and a translation
of `0.2`, it returns `0.19999999999999929` — an error of `7.2e-16 cm`, measured
and recorded by `tests/cell_translation.rs`.

**That error is amplified by six orders of magnitude.** `nudge_across` branches
on `dot == 0.0` — exact floating-point equality against a tangency that is
generically impossible — to decide between a one-nudge step (along `u_out`
only) and a two-nudge step (along `u_out` *and* the surface normal). A `1e-16`
perturbation of the local coordinate decides that branch, and the two branches
differ by `NUDGE = 1e-9 cm` of displacement. Measured discrepancy between two
constructions of the *same physical geometry*: **8.0e-10 cm**.

The consequence is the one `Geometry::cross_surface_in_frame`'s own doc comment
already describes for a different cause: the particle can land on the side it
came from, the following `locate` returns the cell it was leaving, and the next
flight segment is scored in the wrong material.

**Fix:** `Coord` now carries an exact `offset`, accumulated on the way down
(`parent.offset + cell.translation`, plus `Lattice::tile_center(idx)` for a
lattice level) instead of being recovered afterwards by subtraction.
`Lattice::tile_center` was added to return exactly what `get_local_position`
subtracts, so the accumulation cannot drift from it.

**This was a pre-existing defect, not one introduced by translations.** The
same cancellation applies to every lattice tile centre, so it was reachable by
the existing TRISO and hexagonal-lattice cases whenever a tile centre was not
exactly representable relative to the probe position. Non-zero translations
merely made it easy to hit and easy to see.

### Verification: an exact identity, not a tolerance

`tests/cell_translation.rs` rests on an algebraic identity with no
approximation in it:

> Translating a fill cell by `t` is the same geometry as leaving the fill cell
> untranslated and moving every surface of the filled universe by `+t`.

Two geometries are built for every case — **TRANSLATED** (surfaces at the
origin, `cell.translation = t`) and **MOVED** (surfaces at `t`, no translation)
— and required to agree. **The agreement demanded is bit-equality.** That is
justified rather than hopeful: TRANSLATED forms `r - t` and evaluates a surface
centred at `0` (`r_local - 0.0`, exact), MOVED evaluates a surface centred at
`t` (`r - t`), so both compute the identical IEEE-754 expression. A tolerance
would hide exactly what the test is for.

Nine translations (zero, each axis, both signs, an oblique offset, and one
large enough to push the inner sphere against the root boundary) × 1331 probe
points × 6 directions.

| test | what it fixes on | result |
|---|---|---|
| membership | located material and leaf cell | 71 874 queries agree; 838 inside the translated sphere |
| distance | `distance_to_boundary` | 64 314 comparisons **bit-identical** |
| frame offset | `Coord::offset` exact; `r - offset` reproduces the stored local `r` | 64 314 paths exact; old reconstruction off by up to **7.216e-16 cm** |
| surface crossing | `cross_surface_in_frame` in the global frame | 64 314 crossings agree (**failed at 8.0e-10 before the fix**) |
| lattice fill | tile material **and tile index** | 71 874 lookups agree |
| k_eff | full transport, power iteration | `1.043880` **bit-identical** |
| negative control | translation is not a no-op | origin reads material 0 untranslated, material 1 at `t=(3,0,0)` |

**The negative controls are load-bearing.** Every equivalence test above would
pass trivially if `coord.r -= cell.translation` were deleted, because TRANSLATED
and MOVED would both collapse to "surfaces where they are declared". Two tests
therefore assert the opposite direction — a static one on cell membership and a
transport one requiring the untranslated eigenvalue (`1.038742`) to **differ**
from the translated one (`1.043880`).

### What the translation V&V does not establish

- **Not validation.** No experiment, and no comparison against OpenMC running
  the same input.
- **Not a rotation claim.** See below.
- **Not a claim about `nudge_across`.** The `dot == 0.0` branch is still an
  exact-equality test on a generically non-zero quantity, and it is still a
  10^6 amplifier of any upstream coordinate error. Carrying the offset removes
  the error that was reaching it here; it does not make the branch robust.
  **This is the most valuable remaining lead in the geometry kernel** and is
  worth its own investigation — a relative tolerance, or removing the special
  case entirely, both need their own measurement.

## Rotation — not ported, and what porting it needs

`Cell::rotation_` (`include/openmc/cell.h:407`) is a 9-element row-major
**inverse** rotation matrix, optionally followed by the three user-specified
angles. `Cell::set_rotation` (`src/cell.cpp:48`) builds it from x/y/z angles in
degrees, **negating each** before forming the matrix — so the stored matrix maps
global → local, which is what the transform needs.

It is applied at four sites, all immediately after the translation:

| site | context |
|---|---|
| `src/geometry.cpp:226` | `find_cell_inner`, `Fill::UNIVERSE` |
| `src/geometry.cpp:245` | `find_cell_inner`, `Fill::LATTICE` |
| `src/geometry.cpp:392` | `cross_lattice`, recomputing the local position |
| `src/geometry.cpp:463` | hex-lattice neighbour search |

**Porting it is materially harder than translation, for three reasons this
crate's current design makes explicit:**

1. **A rotation transforms the direction, not just the position.**
   `LocalCoord::rotate` rotates both `r_` and `u_`, and sets `rotated_ = true`.
   Every place in this crate that currently assumes "a direction needs no
   transformation at all" would become wrong. Those assumptions are written
   down, which is the good news — `Geometry::cross_surface_in_frame`,
   `Geometry::distance_to_boundary` and `Universe::find_cell` each state
   "nested frames here are pure translations (no rotation)" in their doc
   comments, and `tests/cell_translation.rs` asserts the direction is
   bit-unchanged at every level. Those three doc comments and that assertion are
   the checklist.

2. **The frame offset stops being a translation.** `Coord::offset` — the field
   this change added — is a `Position`, and under rotation the global↔local map
   is no longer `r - offset`. It would have to become an affine transform
   (matrix + vector) carried per level. `outram-blender`'s `Affine3`
   (`crates/outram-blender/src/transform.rs`) is the existing type of that
   shape in this workspace and should be looked at before a new one is written
   — see the workspace `CLAUDE.md` rule on searching before building.

3. **The `on_surface` token's validity argument breaks.**
   `Universe::find_cell` documents that "because every nested frame in this
   crate is a pure translation, the global token stays valid at every level".
   A surface index is global and a rotation does not change *which* surface a
   particle sits on, so the token itself survives — but the *sense* recorded
   with it is a geometric side, and the argument that it transfers unchanged
   across levels needs restating rather than assuming, since it currently rests
   on the pure-translation premise.

**Estimated scope:** the transform plumbing is the bulk of it, not the maths.
`set_rotation`'s matrix construction is 40 lines and directly portable; the work
is converting `Coord::offset` to an affine, rotating `u` at every descent,
un-rotating for boundary conditions, and re-establishing the three documented
invariants above. The V&V has an equally clean identity available — *rotating a
fill by `R` equals rotating the filled universe's surfaces by `R`* — but only
for surfaces this crate can rotate, which for the current `SurfaceKind` set
means spheres trivially and axis-aligned planes/cylinders **not at all** (an
`XPlane` has no rotated form; it would need a general `Plane`). So a rotation
port very likely needs a general plane surface first.

**Nothing in this workspace currently needs rotation.** Every geometry in
`crates/outram-mc-libs/examples/` and `tests/` is axis-aligned or spherical. It
should be ported when a case demands it, not speculatively.

## Other unported `Cell` members, in priority order

1. **`density_mult_`** — a per-instance density multiplier. Cheap to add,
   and the natural hook for a future perturbation/sensitivity capability.
2. **`material_` / `sqrtkT_` as vectors** — OpenMC allows a *distributed* cell
   to carry a different material and temperature per instance. This crate has
   one of each. Needed for distribcell tallies and for depletion of individual
   TRISO particles; not needed by anything today.
3. **`distribcell_index_` / `offset_`** — the distribcell offset table. Only
   meaningful once (2) exists.
4. **`name_`** — cosmetic.

DAGMC cells are deliberately out of scope: this crate is CSG-only and says so.

## Files

- `crates/outram-mc-libs/src/geometry/cell.rs` — `Cell::translation`
- `crates/outram-mc-libs/src/geometry/geometry.rs` — `Coord::offset`,
  `Geometry::locate`, `Geometry::cross_surface_in_frame`
- `crates/outram-mc-libs/src/geometry/lattice/mod.rs` — `Lattice::tile_center`
- `crates/outram-mc-libs/tests/cell_translation.rs` — the V&V suite
