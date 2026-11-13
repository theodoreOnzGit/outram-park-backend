# AGENTS.md


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

**NEE_SOON** — **NE**utron **E**nergy-dependent **S**imulation using
**O**pen-source **O**bject-Oriented **N**umerics. The letters are N-E-E-S-O-O-N: ~~**N**eutron ... **O**bject-**O**riented~~ **CORRECTED 2026-10-13**, the earlier lettering spelled NESOOON.

The **coupling / integration layer** of the OUTRAM PARK suite. It composes the
neutronics + kinetics crates behind a single object-oriented facade so users
assemble simulations without wiring the crates together by hand.

| Composes | Crate | Role |
|---|---|---|
| Nuclear data / cross sections | `njoy-outram-park-fork` | energy-dependent σ(E), ν̄, χ, WMP |
| Monte Carlo transport | `outram-mc-libs` | CSG geometry, k-eigenvalue, Woodcock tracking |
| Point reactor kinetics | `teh-o-prke` | PRKE precursor / reactivity time response |
| GeN-Foam SP3 multiphysics | `outram-foam-appbuilder-lib` | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang SP3 workflow) |
| Reactor thermal-hydraulics (loop + core TH, heat source as input) | `tampines` | since 2026-11-03: the HTR-10 primary loop and its core models (`tampines::htr10_primary_loop`), coupled to the point kinetics in `htr10_th_transient` |
| Geometry authoring | `outram-blender` | since 2026-10-02 (GitHub #486): `blender_bridge::to_mc_geometry` (authored surface -> CSG, a wrapper over `outram_blender::export::to_csg_geometry`) and the `sim` run driver behind MC Studio, both moved from blender's retired `mc-export` feature. The edge runs nee_soon -> blender; blender never depends on outram-mc-libs |

See the workspace `docs/architecture.md` for the responsibility split
(nuclear data ⟂ Monte Carlo ⟂ deterministic/TH ⟂ coupling).

## Status

**Mostly scaffold; the prompt-excursion path is real.**
`NeeSoon::new_prompt_excursion_model` is real, wired code — a thin pass-through
to `teh-o-prke`'s `NordheimFuchsExactTimestepper`, backed by an integration
test (bead op-fr2.1, closed). The `xin_wang_sp3_workflow` module now exists as a
documented four-stage scaffold: its stage `run()` methods return
`WorkflowError::NotYetImplemented` (each naming its tracking bead), while
carrying real Mk1 case data. ~~The `njoy-outram-park-fork` and `outram-mc-libs`
coupling logic is still future work.~~ **CORRECTED 2026-10-02:** it is wired
for HTR-10 (`htr10_rmc::data` loads the tapes through the NJOY port;
`htr10_rmc::keff_vs_height` transports with `outram-mc-libs`), though not yet
through the `NeeSoon` facade.

**HTR-10 k against height (gh:#501):** `examples/htr10_endf8_kvsh_quick.rs`
and `examples/htr10_endf8_kvsh_heavy.rs` run the whole N = 10..20 sweep and
write the figure script, results table and parameters. The shared machinery
(majorant, source box, entropy mesh, reference interpolation, emitters) is
`htr10_rmc::keff_vs_height`; use it rather than copying it into a new example.

**HTR-10 MGXS cases (gh:#845, 2026-10-12):** `htr10_mgxs` holds a registry
of named, fixed MGXS runs on the full core (tally once on WIMS-69 + 1 groups,
collapse exactly to 2/4/8 with `mgxs::MgxsLibrary::collapse`), the runner,
and the record each writes to
`verification_and_validation/htr10_mgxs_runs/<case>/`. They are run on a
separate machine by `examples/htr10_mgxs_case.rs` following
`docs/htr10-mgxs-desktop-runbook.md`: **add or change runs in
`htr10_mgxs::cases`, never in the runbook or the example**, and give a
changed case a new name once it has a record. The scalar pass carries `Fission`
since the same change (`mgxs::SCALAR_SCORES`, appended last).

**MGXS into the multigroup kernel (gh:#848, #682, 2026-10-13).**
`MgxsLibrary::to_physics_mg()` converts to `outram_mc_libs::physics::physics_mg`:
it reverses to descending energy and attaches the (n,xn) scattering
multiplicity **by default** (`to_physics_mg_without_multiplicity` is the
ablation). It refuses a library whose scatter-only row disagrees with
`Sigma_t - Sigma_a` beyond 4σ of `condense`'s per-group standard errors
(`ZoneMgxs::sigma`). It also refuses a library with no standard errors or no
scatter-only matrix (`ZoneMgxs::scatter_only`, the matrix tally's third score,
`MATRIX_SCORE_TYPES`). Records written before 2026-10-13 carry neither, so they
convert only through explicit ablations
(`verification_and_validation/htr10_mgxs_runs/README.md`). `k_inf()` counts the
(n,xn) gain since the same change. End-to-end check:
`tests/mgxs_multiplicity_infinite_medium.rs`.

**HTR-10 cells (gh:#849, 2026-10-13).** `htr10_mgxs::cells::Htr10Cell` is
built from the core's own TRISO and pebble builders
(`core_shell::TrisoBuild`, `core_model::build_tile_universes`, extracted; the
core's geometry hash is unchanged). There are two kinds:
- `single_pebble()`: a reflective 6 cm cube;
- `seker_cluster(1..=3)`: reflective superlattice prisms of 1/7/19 tiles.

They are drawn and checked in `verification_and_validation/htr10_mgxs_cells/`.
The CSG pebble agrees with `DhUniverse::pebble` (+102 ± 347 pcm). The single
pebble is under-moderated (no dummy balls), so its constants do not describe
the core; use a Şeker cluster for that. The 1/7/19 convergence is not resolved
by the pilots and waits on the `cell_*_t300` records.

**HTR-10 bed options (2026-10-05).** Two builders share everything outside
the bed (`htr10_rmc::core_shell`, extracted from `assemble_explicit_triso`,
geometry verified byte-identical before/after by its `Debug` hash):
- `core_model::assemble_explicit_triso(n_rings, layers, majorant)` — Şeker &
  Çolak (2003)'s ordered 13-ball bed (`PebbleBed::Seker`), the default and the
  one every recorded `k` is on.
- `explicit_bed::assemble_explicit_triso_from_centres(centres, is_fuel,
  n_rings, majorant)` — an explicit ball list in the DEM frame (metres,
  `z = 0` the bed floor), e.g. a LIGGGHTS-port pour (`PebbleBed::Explicit`).
  Soft-sphere overlaps are split by the bisector plane (nothing shrunk); the
  tube below the DEM column is filled with Şeker's tube balls; see the module
  docs. Drawn in `verification_and_validation/htr10_dem_bed_images/`
  (`examples/htr10_dem_bed_images.rs`). ~~**No validated `k` on it yet.**~~
  **UPDATED 2026-10-08 (gh:#787):** one recorded `k`, still not validated:
  `htr10_rmc_keff` with `OUTRAM_HTR10_DEM_BED=<csv>` cut to the lattice's
  16 681 balls (N = 12) gives 0.989293 ± 0.000962, −583 ± 143 pcm from the
  lattice. One pour; scatter between pours unmeasured
  (`verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).

Both take a design since 2026-10-05 (`assemble_explicit_triso_with`,
`assemble_explicit_triso_from_centres_with`, `core_design::Htr10CoreDesign`;
gh:#566, gh:#580): fuel-ball fraction, fuel-zone radius, TRISO radii and
count, and the control-rod insertion (all ten explicit rods, 0 withdrawn to 1
fully inserted). The default design is the record's model and builds the
same geometry (pinned by `the_default_design_builds_the_geometry_unchanged`
and the workbench fixture). The 6 cm ball, materials and fertile/poison
balls are not part of a design; `core_design`'s module docs list them.
Tapes can be read from a chosen folder, flat or an extracted library, found
by MAT and NSUB (`data::TapeSource::Folder`, gh:#581).

**HTTR start-up core (gh epic #888, 2026-10-16).** `httr` builds the IRPhEP
HTTR-GCR-RESR-001 fully-loaded core (R1: INL/EXT-08-14767 Rev. 2) with every
TRISO particle, delta-tracked in the compacts as HTR-10's bed is
(`httr::assemble_httr`, `httr::compact_majorant`). Every dimension is in
`httr::spec`, every composition in `httr::materials`, each with its R1 table
or deck card; R1's text and deck disagree in three places and the deck is
followed (listed in `httr`'s module docs). It reads its 48 nuclides through
HTR-10's loader, generalised to any slot list
(`htr10_rmc::data::load_slots_with_progress`). Drawn by
`examples/httr_geometry_images.rs`; run by `examples/httr_keff.rs`; record in
`verification_and_validation/httr_startup_core/README.md`. First results
(2026-10-16, uncalibrated): critical k = 1.02400 ± 0.00049, i.e. +2150 pcm
vs the benchmark, which is R1's known Monte Carlo bias, and +110 pcm vs
MCNP5 VII.0. Rod worth is 3.4 % low after the graphite-law ablation (gh
#919, open). **AI first build; the maintainer called it "a good first
draft" (#888), not a line-by-line review.**

**HTR-10 per-zone temperatures and `ParametricStudy` (gh epic #1137,
2026-11-03).** `htr10_rmc::zones` splits the core into eight temperature
zones:
- fuel pebbles, moderator pebbles, coolant;
- top, side and bottom reflector;
- control rods, boronated bricks.

A design with `temperature_zones` places the extra slots
(`mat::MODERATOR_BALL` and a copy of each reflector composition per region);
without it the geometry is the record's, cell for cell.

`htr10_rmc::parametric::ParametricStudy` holds every setting (bed, rods,
zone temperatures, statistics, data):
- it processes each (nuclide, temperature) exactly, and every bound law by
  LEAPR at its zone's temperature (`ThermalLawSource::Leapr`);
- **use it for any run that changes temperatures**: the CLI is
  `examples/htr10_parametric.rs`, and the browser demo builds through it
  too;
- with every zone at one temperature it transports bit for bit as the
  single-temperature model (`tests/htr10_parametric_isothermal.rs`).

Record and images: `verification_and_validation/htr10_zone_temperatures/`.

**Random TRISO packing in DEM-bed pebbles (gh:#1243, 2026-11-05).**
`Htr10CoreDesign::triso_packing` is `Lattice` (the default, the record's
regular lattice) or `Random { realisations, seed }`:
- only the explicit (DEM) bed builder reads it; the Şeker bed always uses
  the lattice;
- each realisation is exactly the lattice's particle count, packed by RSA
  (`outram_mc_libs::dh_universe::pack_count_in_ball`) and wrapped in a
  tracking lattice (`create_triso_lattice`), as geometry lattices 2 and up;
  fuel ball j takes realisation j mod K;
- `ParametricStudy` uses `TrisoPacking::DEM_DEFAULT` (8 realisations) for an
  explicit bed, so the browser demo's DEM beds are random. Native runs
  outside the study keep the lattice, and so does the one DEM-bed k on
  record (0.989293).

Record and images: `verification_and_validation/htr10_random_triso/`.

**HTR-10 coupled TH transients (gh:#1151, moved here 2026-11-03).**
Maintainer: "any neutronics and TH shld live in nee soon"; "Tampines will
have fixed or user determined heat sources". `htr10_th_transient` holds the
point kinetics (PRKE + ANS decay heat + Xe-135 + fuel/moderator/reflector
feedback, on `teh-o-prke`), the coupled step with `tampines`'s primary loop
and core models, and the LOFC ATWS case with its named sensitivities.
`tampines` keeps the loop and core thermal-hydraulics with the power as an
input. Record: `verification_and_validation/htr10_loop_lofc/README.md`
(moved from tampines with git history; the moved tests reproduce its
numbers). **Part B (same day):** the R-Z porous core
(`tampines::htr10_primary_loop::RzPorousCore` over
`outram_foam_appbuilder_lib::htr10_rz::Htr10RzTransientCore`) in the loop,
the CRW case (Tab 2.60), the Am-Be start-up source **ON by default**
(`NeutronSource`, ablation `without_neutron_source`; pinned by a test), and
the C09 thermocouple probes. Record:
`verification_and_validation/htr10_rz_transients/README.md`. Status: the
short-term shutdown, the top-reflector heat-up and the side-reflector
thermocouples agree; **re-criticality ~3400 s late and long-term power
~40 % low in both tests** (model heat sink ~100 kW against ~180 kW); CRW
first peak 158 % against 216/241 %. Not validated. **UPDATED 2026-11-03
(gh:#1173-#1175):** the gas Picard converges on the 10 cm mesh, and the
20 cm base is **not mesh-converged** (10 cm: re-criticality +590 s, late
power +33 kW); RPV-cavity air convection is on by default and adds ~5 kW
(not the heat-sink gap); the moderator coefficient reads all pebble
graphite and the dummy pebbles have their own node (both default): LOFC
re-criticality 7426 s, P(100 s) 0.170 (measured 0.204), CRW first peak
154 % at 22 s. Still not validated.

**The two multiphysics chains (gh epic #1183, 2026-11-03).** Untrusted AI draft; not validated.

- **`power_shape` (gh:#1184)** puts a total power onto TH cells, conserved to round-off. It holds:
  - the shapes, as a closed enum: lumps, cosine/sine, `J0 x cos`, Legendre-z, Zernike(-Legendre), Fourier-Bessel x Legendre, spatial spherical harmonics, and an R-Z table;
  - `PowerShape::from_tally`, which reads the Monte Carlo `KappaFission` tallies built by `McPowerTally`;
  - `zernike_to_bessel` (Neumann/Dini by default);
  - the MC / TECDOC / GeN-Foam frames, tied at the bed floor (`TECDOC_BED_FLOOR_Z_CM`).

  Record: `verification_and_validation/power_shapes/`.
- **`htr10_coupling` (gh:#1186)** runs `Htr10DirectCoupling`: the `ParametricStudy` Monte Carlo with an R-Z power tally, Picard-iterated against either GeN-Foam's lumped node or `htr10_rz`'s R-Z core.
  - **Every temperature is reprocessed exactly** and cached: setting `Material.temperature` alone does nothing for HTR-10's NJOY nuclides.
  - The coupled run is `examples/htr10_direct_coupling.rs`, which is far too slow for a test.
  - Record: `verification_and_validation/htr10_direct_coupling/`.
- **`mgxs_genfoam` (gh:#1187)** stacks the four isothermal records with `TFuel` (log law).
  - The regions are read from the Monte Carlo geometry on the `htr10_rz` mesh lines.
  - Rebalance, descending order and SI units are on by default; each has a `without_*` ablation.
  - The diffusion `k` is compared against the records' Monte Carlo `k` in `examples/htr10_mgxs_genfoam_rz.rs`.
  - Record: `verification_and_validation/htr10_mgxs_genfoam_rz/`.
- **`direct_coupling::Convergence` and `McGenFoamDirect::new`'s volume are `uom`** since the same change.

**LEAPR is the default for graphite and SiC (maintainer, 2026-11-03: "just use
LEAPR by default").** `Htr10DataConfig::default().thermal_laws` is
`ThermalLawSource::Leapr`, for both the jobs path and `load_htr10_nuclides`;
`load_slots_with_progress` (HTTR too) follows the same default.

**Every HTR-10 and HTTR k recorded before 2026-11-03 used the published
tapes' graphite and SiC laws**, at the tape's nearest temperature (296 K for
a 300.15 K run). ~~To reproduce one, set `OUTRAM_HTR10_TAPE_LAWS=1`
(`ThermalLawSource::Tapes`).~~ **CORRECTED 2026-11-11 (gh:#1333):** the
variable is read only where `ThermalLawSource::from_env()` is called
(`load_slots_with_progress`, `examples/htr10_rmc_keff.rs`, HTTR's
materials note); `Htr10DataConfig::default()` ignores it, so a test or
caller building that config must set `thermal_laws:
ThermalLawSource::Tapes` itself (verified: with only the variable set,
`tests/htr10_rod_bank_insertion.rs`'s k was bit-identical to the LEAPR
run). **And every k recorded before 2026-11-10 also used the DBRC
resonance-scattering sampler**, now an option beside the default RVS
(gh:#1311): reproducing one needs `Nuclide::with_resonance_scattering_method(
ResonanceScatteringMethod::Dbrc)` as well. With both, the rod-bank test's
2026-10-23 k reproduced bit for bit on 2026-11-11. Re-measuring those
records under the new default is gh:#1145.

**Do not add physics kernels here** — only orchestration / facade /
cross-crate glue belongs in this crate.

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

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
