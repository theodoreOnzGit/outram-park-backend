# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the `sembawang` crate.

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## The inventory is outram-mc's `RadionuclideInventory` (2026-10-17, GitHub #951)

**Maintainer direction:** "Change sembawang to use RadionuclideInventory
instead." There is one inventory type in the workspace,
`outram_mc_libs::inventory::RadionuclideInventory`, re-exported from
`sembawang::inventory`; this crate's own `CoreInventory` / `NuclideInventory`
are gone.

- **Build one** with `inventory::prescribed(entries, n_radial, reference)`
  (published tables; names parsed through upstream's normaliser) or
  `inventory::unit(names, n_radial)` (the 1 Ci shakedown). From depletion:
  `RadionuclideInventory::from_burnup_step` in outram-mc, then
  `inventory::restrict_to_triso_atops`, which **reports** the nuclides
  TRISO-ATOPS does not model.
- **Rings, not components.** The release needs radial rings
  (`Regions::RadialRings`, or `WholeCore` as one ring). An inventory over named
  regions is an error until the caller maps it with `with_regions`.
- **No axial count on the inventory.** The axial split uses the transient's
  `n_axial`, as it always did.
- **Bit-identity gate:** `tests/inventory_handoff_fingerprints.rs` (pins
  printed on the pre-change code). Any later change to the inventory path must
  keep it passing or record why it moved.
- **Dependency:** `sembawang -> outram-mc-libs`, never the reverse.

## `bwr_mark1`: the Fukushima Daiichi Unit 1 model (2026-10-29, GitHub #1033)

**Maintainer direction:** "Physics libraries go to sembawang." The Unit 1
plant model moved here from `dhoby-ghaut`'s `fukushima_web` example; the demo
now only draws it and steps it on its Web Workers. New physics for it (the
F-items in `docs/fukushima-melcor-gap-plan.md`, epic #1028) lands here, never
in the demo.

- **Regression fixture:** `verification_and_validation/bwr_mark1/headless_30h.csv`,
  compared byte for byte by `bwr_mark1::tests::the_headless_run_matches_the_committed_fixture`.
  After a deliberate physics change, re-bless with
  `FUKUSHIMA_BLESS=1 cargo test --release -p sembawang --lib bwr_mark1` and
  record the movement in the commit and on the issue.
- **One worker equals three.** Parts cross as exact `f64`s (`wire`);
  `tests::three_workers_are_one_worker` must keep passing.
- **The BSAF bands are the check, never an input** (root `CLAUDE.md`,
  "PROCESS right first"). `compare::the_uncalibrated_baseline_is_pinned`
  moves only with the old value, the new one and why.
- **New mechanisms default ON** with a `Mechanisms::without_*` ablation and a
  row in `compare::ablations`.
- **Clean room:** public manuals and literature only, never MELCOR, MAAP,
  ASTEC, RELAP5, SCDAP or FRAPCON source.
- **Transcendentals through PETIR** (`.r_exp()` etc., gh:#985), never the
  platform's `.exp()`; converted 2026-10-29 with the effect measured in
  `verification_and_validation/bwr_mark1/petir-conversion.md`.
