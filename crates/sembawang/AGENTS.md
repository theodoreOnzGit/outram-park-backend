# AGENTS.md

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

- **The containment is the control-volume network (2026-11-02, gh:#1099,
  maintainer: "switch it").** `Plant::parts` builds
  `containment_network::ContainmentNetwork` behind `PlantContainment` (enum
  dispatch); the lumped `containment::Containment` is the explicit ablation
  `Mechanisms::without_network_containment()`, under which every result
  pinned before the switch is reproduced bit for bit. Pinned by
  `tests::the_containment_network_is_the_default`. The network costs about
  100 times the lumped model (a 30 h run takes about 20 min against 10 s),
  so a test that runs the network for more than 5 min goes behind the
  crate's `long-tests` feature (default on, added with the switch), and a
  multi-hour one is a plain `#[ignore]`. **Results on it stay tentative**
  until ~~its geometry is drawn~~ a human has checked its drawn geometry
  (gh:#1110: drawn 2026-11-02, `verification_and_validation/bwr_mark1/geometry/`),
  and the vent depends on the torus
  pool's layer count (gh:#1108).
- **Regression fixtures:** ~~`verification_and_validation/bwr_mark1/headless_30h.csv`,
  compared byte for byte by `bwr_mark1::tests::the_headless_run_matches_the_committed_fixture`.~~
  **CHANGED 2026-11-02 (gh:#1099):** `headless_30h.csv` is now the network
  plant's (`the_headless_run_matches_the_committed_fixture`, long-tests),
  and the lumped plant's, byte-identical to the fixture before the switch,
  is `headless_30h_lumped.csv` (`the_lumped_headless_run_matches_its_fixture`).
  After a deliberate physics change, re-bless with
  `FUKUSHIMA_BLESS=1 cargo test --release -p sembawang --lib bwr_mark1` and
  record the movement in the commit and on the issue.
  **STALE since 2026-11-02 (gh:#1116, #1117, #1114), on purpose:** those
  tampines changes move the network plant, and `headless_30h.csv` was not
  re-blessed. Maintainer: "until we get geometry no need to test
  regression", and "unnecessary to pin a regression and test if we are not
  even sure of model yet". So the two network regression pins,
  `the_headless_run_matches_the_committed_fixture` and
  `compare::tests::the_network_baseline_is_pinned`, are `#[ignore]`d with
  that reason until the network model is checked (the human check of the
  drawn geometry, gh:#1110); then re-bless and un-ignore them.
  One attempt was stopped at its 7000 s timeout, so a 30 h network run with
  #1116 takes over about 2 h. The lumped fixture is unaffected.
- **One worker equals three.** Parts cross as exact `f64`s (`wire`);
  `tests::three_workers_are_one_worker` must keep passing. Since gh:#1099
  the network's state has no wire form: the containment worker
  (`ContainmentWorker`) holds the containment and only the snapshot and the
  result cross; the core still crosses with its whole state.
- **The BSAF bands are the check, never an input** (root `CLAUDE.md`,
  "PROCESS right first"). `compare::the_uncalibrated_baseline_is_pinned`
  moves only with the old value, the new one and why.
- **New mechanisms default ON** with a `Mechanisms::without_*` ablation and a
  row in `compare::ablations`. (`compare`'s pinned-baseline, ablation-table,
  ensemble and elasticity tests run on the lumped containment since
  gh:#1099, for cost; `compare::tests::the_network_baseline_is_pinned` pins
  the network default, and `the_ablation_table_on_the_network` is ignored,
  multi-hour.)
- **Clean room:** public manuals and literature only, never MELCOR, MAAP,
  ASTEC, RELAP5, SCDAP or FRAPCON source.
- **Equations go to offbeat, the plant stays here (2026-11-11, gh:#1335;
  maintainer 2026-11-10: "the MELCOR equations clean room should live as an
  offbeat module rather than in sembawang").** `aerosol`, `vapour`,
  `rn_volume`, `iodine`, `wire` and the constant `params::R_GAS` moved
  unchanged to `outram_park_fork_offbeat::melcor`;
  `sembawang::bwr_mark1::{aerosol, iodine, rn_volume, vapour, wire}` are
  deprecated `pub use` aliases for one release. New generic MELCOR-manual
  equations go to `offbeat::melcor` when they need nothing beyond `petir`,
  `tampines-steam-tables` and `changi`; anything needing `tampines`,
  `teh-o-prke`, the ENDF libraries or `tuas_boussinesq_solver`, and all
  plant data, stays here. Why each remaining file stayed: gh:#1335.
- **Transcendentals through PETIR** (`.r_exp()` etc., gh:#985), never the
  platform's `.exp()`; converted 2026-10-29 with the effect measured in
  `verification_and_validation/bwr_mark1/petir-conversion.md`.
- **Units 2 and 3 (2026-11-02, gh:#1109).** `Plant::for_unit(Unit::Two |
  Unit::Three, ..)` runs `Params::unit2()`/`unit3()` on
  `NetworkConfig::unit2()`/`unit3()` with RCIC and HPCI (`turbine_pump`)
  and, under `Mode::Historical`, the unit's sequence (`units23`), every time
  cited to USNRC HRTD ch. 7.1 Table 7.1-2 with its page. The plant's own
  trips and the table's readings (`units23::OBSERVATIONS`) are the check,
  never inputs. Every older constructor is Unit 1, and Unit 1 must stay
  bit-identical (`the_lumped_headless_run_matches_its_fixture`). Results:
  `verification_and_validation/bwr_mark1/units23_sequence.md`.
- **Inputs are `params::Params`** (gh:#1033): every scalar input as one
  `Copy` value, defaulting to the documented constants (whose tags and
  sources stay the record). Run a corrected or perturbed plant with
  `Plant::with_params`; a new input goes in the `params!` list (which fixes
  its wire order) with its constant, tag and source. Model code reads
  `prm.field`, never the constant.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
