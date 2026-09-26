# CLAUDE.md


Guidance for Claude Code (and other AI assistants) working in the `nee_soon` crate.

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

## What this is

**NEE_SOON** — **N**eutron **E**nergy-dependent **S**imulation using
**O**pen-source **O**bject-**O**riented **N**umerics.

The **coupling / integration layer** of the OUTRAM PARK suite. It composes the
neutronics + kinetics crates behind a single object-oriented facade so users
assemble simulations without wiring the crates together by hand.

| Composes | Crate | Role |
|---|---|---|
| Nuclear data / cross sections | `njoy-outram-park-fork` | energy-dependent σ(E), ν̄, χ, WMP |
| Monte Carlo transport | `outram-mc-libs` | CSG geometry, k-eigenvalue, Woodcock tracking |
| Point reactor kinetics | `teh-o-prke` | PRKE precursor / reactivity time response |
| GeN-Foam SP3 multiphysics | `outram-foam-appbuilder-lib` | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang SP3 workflow) |

See the workspace `docs/architecture.md` for the responsibility split
(nuclear data ⟂ Monte Carlo ⟂ deterministic/TH ⟂ coupling).

## Status

**Mostly scaffold; the prompt-excursion path is real.**
`NeeSoon::new_prompt_excursion_model` is real, wired code — a thin pass-through
to `teh-o-prke`'s `NordheimFuchsExactTimestepper`, backed by an integration
test (bead op-fr2.1, closed). The `xin_wang_sp3_workflow` module now exists as a
documented four-stage scaffold: its stage `run()` methods return
`WorkflowError::NotYetImplemented` (each naming its tracking bead), while
carrying real Mk1 case data. The `njoy-outram-park-fork` and `outram-mc-libs`
coupling logic is still future work. **Do not add physics kernels here** — only
orchestration / facade / cross-crate glue belongs in this crate.

## Design rules

- **One big struct.** The public surface is reached through `NeeSoon` — the
  object-oriented entry point that creates the simulation pieces. Keep the
  crate navigable by `rust-analyzer` alone; every public item needs a `///`
  doc comment (what physical quantity, valid ranges, units).
- **Expose and integrate — do not reimplement.** New cross-section code goes to
  `njoy-outram-park-fork`; new transport to `outram-mc-libs`; new kinetics to
  `teh-o-prke`. Only *new coupled* functionality belongs here.
- **Dimensioned units.** All public physical quantities use `uom`, never bare
  `f64`.
- Follow the workspace-wide architecture rules (root `CLAUDE.md`): no
  `Box<dyn Trait>` / no trait-object dispatch (use enums), no `Box<T>`, no
  lifetime parameters, `Arc<RwLock<T>>` over channels for shared state.

## Build & test

Always `--release` (workspace rule). ~~System OpenBLAS required (pulled in via
`outram-mc-libs`).~~ **CORRECTED 2026-09-17** — false: `outram-mc-libs`'s own
`Cargo.toml` states it deliberately avoids BLAS/C dependencies to stay
Android-safe (`Cargo.toml:41`), and `nee_soon`'s own `Cargo.toml` carries no
BLAS dependency either. Per the workspace root `CLAUDE.md` "Build & test"
section, the only thing in this workspace that needs a system BLAS is
`outram-foam-basic-lib`'s `matrix_bench` dev-dependency test — nothing in
`nee_soon`'s dependency chain needs it. This was one of the "stale BLAS
claims" named in `docs/real-time-multifidelity-scoping.md` item 9.

```bash
cargo build --release -p nee_soon
cargo test  --release -p nee_soon
```
