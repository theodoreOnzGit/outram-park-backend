# OpenMC notebooks → outram-mc verification map

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
This mapping was drafted by an AI assistant and is untrusted until a human has
verified every "tractable-now" claim against the actual crate API. See
`docs/ai-fleet-review/op-6tz/REVIEW_MANIFEST.md`.

Tracks beads epic **op-6tz** ("OpenMC-API parity + openmc-notebooks as
verification tests"). This is the notebook → test → required-API mapping doc
(op-6tz.1); the test harness it drives lives in
`tests/openmc_notebooks/` (op-6tz.2).

## Provenance

- **Source:** <https://github.com/openmc-dev/openmc-notebooks>
- **Commit pinned:** `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (committed 2024-07-10)
- **License:** the openmc-notebooks repository is MIT-licensed (OpenMC project).
  Only notebook *structure and API usage* are referenced here; no notebook text
  is copied. Cross-section inputs used by any live test are the crate's own
  embedded/open data, not notebook data. Data policy (`DATA_POLICY.md`): all
  open-source, no confidential/operational data.
- **Notebook count:** the pinned commit contains **27** `.ipynb` notebooks.
  The `shielded_room_weight_window` notebook named in the outram-mc directive is
  **not present** at this commit (it appears to be a planned/renamed notebook);
  it is carried below and in the harness as an absent-upstream placeholder so the
  variance-reduction gap is still tracked.

## How to read this table

- **Owner** — `outram-mc` = this crate's responsibility (transport / geometry /
  tally / depletion / variance-reduction); `njoy` = `njoy-outram-park-fork`'s
  responsibility (nuclear-data generation / mgxs / search). This run only builds
  tests for the `outram-mc` subset; `njoy` rows are mapped for completeness but
  their harness lives in the parallel njoy track.
- **Tractable now?** — can a *faithful* (not fake) test run against the current
  `outram-mc-libs` API today. `partial` = a reduced but honest slice runs live;
  the notebook's full model is still gated.
- **Gap bead** — the op-6tz child bead tracking the missing API.

## Current outram-mc API reality (assessed 2026-07-16)

The **general CSG k-eigenvalue path is now live**:
`physics::transport_csg::run_keff_csg` navigates the full geometry
(surfaces → cells → universes → lattices), samples collisions, scatters, and
banks fission sites. The earlier `physics::keff::run_keff` homogeneous
bare-sphere path is still present (it powers the Godiva validation), but the
general machinery below is now implemented, not stubbed:

- `geometry::geometry::Geometry::locate` — nested lattice descent (implemented).
- `physics::transport_csg::run_keff_csg` — the live CSG k-eigenvalue loop.
- `tally::scoring` — flux / reaction-rate scoring (implemented).
- `geometry::universe::Universe::find_cell` — implemented (no `todo!()`).
- `geometry::cell::Cell::contains` — RPN region evaluation (implemented).
- `geometry::lattice`: both `RectLattice` and `HexLattice` (ring construction +
  indexing + lattice transport) are implemented.
- `material::{nuclide, reaction, thermal, material}` — implemented, incl.
  `Nuclide::from_core` (embedded WMP data) and S(α,β) thermal-scatter tables.
- `material::thermal::ThermalScattering` — implemented.
- `depletion` — one-group CRAM burnup (`chain.rs`, `cram.rs`, `matrix.rs`,
  `operator.rs`), live for inventory & k_inf trends.
- `pebble_beds` (`delta_tracking`, `stochastic_media` packing) — assembled into
  the live TRISO doubly-heterogeneous k∞ case.

Still genuinely gated (`#[ignore]` with a documented gap): the generic
history-based `physics::transport` variant, multigroup mode
(`physics::physics_mg`, still a stub), DAGMC / unstructured-mesh geometry,
functional-expansion / mesh / distribcell tally filters, photon transport, the
`StatePoint`/pandas inspection layer, and the C-API.

Consequence: notebooks resolvable with the CSG core (criticality eigenvalue,
hex-lattice geometry, TRISO packing, one-group depletion) are live; everything
requiring the still-gated features above is `#[ignore]` with a documented gap.

## Mapping — outram-mc subset (this crate)

| Notebook | Owner | OpenMC API exercised | outram-mc equivalent / GAP | Tractable now? | Gap bead | Notes |
|---|---|---|---|---|---|---|
| `pincell` | outram-mc | `Material`, `ZCylinder`/`XPlane`/`YPlane`, `Universe`, `Cell`, `Geometry`, `IndependentSource`, `Settings`, `run`, `Tally`+`CellFilter`, `StatePoint` | `run_keff` (bare-sphere k-eff) covers the criticality-eigenvalue core; full LWR pin-in-a-reflected-cell + thermal spectrum + flux tally is absent | **partial (LIVE)** | op-6tz.7, .10, .12 | Live test runs the **Godiva bare-sphere** k-eff (op-u6s.1) as the criticality-eigenvalue verification. The notebook's true LWR pin (square cell, reflective BC, S(a,b) water, cell flux tally) needs general CSG + lattice + thermal scatter + tally scoring — none live. |
| `hexagonal-lattice` | outram-mc | `HexLattice`, `Universe`, `Cell`, `ZCylinder`, `model.HexagonalPrism`, `Plot.from_geometry` | `HexLattice` ring construction + indexing + `Geometry::locate` nested-lattice descent are live; the notebook itself runs **no** k-eff, so LIVE assertions are geometry-correctness properties (+ a k smoke run, no reference) | **LIVE (geometry)** | op-6tz.11 | Geometry-construction + plotting notebook. Live geometry test + supplementary k smoke; see `tests/openmc_notebooks/hexagonal_lattice.rs`. |
| `triso` | outram-mc | `model.TRISO`, `model.pack_spheres`, `model.create_triso_lattice`, `Universe`, `Cell`, `Sphere` | `pebble_beds::stochastic_media` packing + `delta_tracking` (Woodcock) assembled into a live doubly-heterogeneous k∞ sim; notebook runs `plot` only (no reference k), so assertions are physics-correctness (packing fraction + unbiased delta tracking) | **LIVE** | op-6tz.16/.25 | Random-packed doubly-het k∞ by delta tracking; see `tests/openmc_notebooks/triso.rs`. |
| `candu` | outram-mc | `ZCylinder`, `Cell`, `Universe`, `DistribcellFilter`, `Tally` | No cluster geometry navigation, no distribcell filter | no | op-6tz.11, .7 | CANDU cluster in a pressure tube; needs CSG descent + distribcell tally. |
| `cad-based-geometry` | outram-mc | `DAGMCUniverse`, `dagmc`, `RegularMesh`, `MeshFilter`, `Model`, `Plot` | No DAGMC/CAD geometry backend | no | op-6tz.17 | Large; CAD geometry is long-horizon / partially out of scope for the pure-Rust CSG core. |
| `unstructured-mesh-part-i` | outram-mc | `UnstructuredMesh`, `MeshFilter`, `lib._libmesh_enabled`, `examples.pwr_assembly` | No unstructured mesh type or mesh tally | no | op-6tz.17 | Needs libMesh/MOAB-style unstructured mesh tallies. |
| `unstructured-mesh-part-ii` | outram-mc | `UnstructuredMesh`, `DAGMCUniverse`, `MeshFilter`, `EnergyFilter`, `lib._dagmc_enabled` | Same as above + DAGMC | no | op-6tz.17 | |
| `tally-arithmetic` | outram-mc | `Tally`, `EnergyFilter`, `CellFilter`, `MeshSurfaceFilter`, derived-tally algebra (`+ - * /`), `get_slice`, `summation` | Tally data structs exist; no transport-driven scoring and no derived-tally arithmetic | no | op-6tz.22, .9 | Needs scored tallies first, then the arithmetic layer. |
| `tally-power-normalization` | outram-mc | `Tally` (heating/fission), `CellFilter`, `StatePoint`, power normalization from `kappa-fission` | No heating tally, no StatePoint | no | op-6tz.22 (→.9) | Requires a scored fission-energy tally + normalization to a target power. |
| `expansion-filters` | outram-mc | `SpatialLegendreFilter`, `ZernikeFilter`, `ZernikeRadialFilter`, `Legendre`, `legendre_from_expcoef` | No functional-expansion filters | no | op-6tz.14 (→.9) | Legendre/Zernike moment tallies. |
| `flux-spectrum` | outram-mc | `examples.pwr_pin_cell`, `EnergyFilter`, `mgxs.GROUP_STRUCTURES`, `Tally`, `StatePoint` | `EnergyFilter` exists; no energy-binned track-length flux tally driven by transport | no | op-6tz.9 (→.8,.7) | Needs the transport loop to score flux into energy bins. |
| `gamma-detector` | outram-mc | `data.decay_photon_energy`, `Source`, `stats.Discrete`/`Isotropic`, photon transport, `EnergyFilter` | Neutron-only; no photon transport, no decay-photon source | no | op-6tz.19 | Photon transport is out of scope (`src/photon.cpp` not ported). |
| `post-processing` | outram-mc | `StatePoint`, `RegularMesh`, `MeshFilter`, `Tally`, mesh reshaping | No StatePoint persistence / mesh tally | no | op-6tz.22, .13 | Reads back tally results for plotting; needs scored mesh tallies + StatePoint. |
| `pandas-dataframes` | outram-mc | `Tally.get_pandas_dataframe`, `DistribcellFilter`, `MeshFilter`, `EnergyFilter`, `Trigger` | No DataFrame export, no distribcell/mesh scoring | no | op-6tz.22 (→.9) | Pandas export is an inspection layer atop scored tallies. |
| `mg-mode-part-i` | outram-mc | `XSdata`, `Macroscopic`, `MGXSLibrary`, `RectLattice`, multigroup `run` | `physics_mg` is a stub; no MGXS data types, no MG transport | no | op-6tz.15 | Multigroup transport execution is outram-mc's (MGXS *generation* is njoy). |
| `mg-mode-part-ii` | outram-mc | `mgxs.Library`, `plot_xs`, `MeshFilter`, multigroup `run` | Same MG gap | no | op-6tz.15 | |
| `mg-mode-part-iii` | outram-mc | `mgxs.Library`, `RectLattice`, `MeshFilter`, multigroup `run` | Same MG gap | no | op-6tz.15 | |
| `depletion` | outram-mc | `deplete.CoupledOperator`, `deplete.PredictorIntegrator`, `deplete.Results`, `deplete.Chain`, `model.pin` | **LIVE (partial)** — `depletion` module: CRAM `exp(A·dt)` solver + `DepletionChain` (chain_simple) + one-group burnup loop | **yes** (one-group trends) | op-6tz.18 | Inventory + k_inf trends match the notebook (sign/order); absolute k needs multigroup transport-coupled rates (follow-up). See `docs/ai-fleet-review/op-6tz-depletion/`. |
| `capi` | outram-mc | `openmc.lib` (`init`, `simulation_init`, `next_batch`, `tallies`, `cells`, `materials`, `finalize`) | No in-memory run/introspection interface | no | op-6tz.20 | Needs a batch-stepping + live-edit API analog. |
| `shielded_room_weight_window` | outram-mc | (absent at pinned commit) weight windows / variance reduction | No weight-window machinery | no | op-6tz.21 | **Notebook absent** from openmc-notebooks@cf1e5db; placeholder test tracks the variance-reduction gap. |

## Mapping — njoy subset (parallel track, not built here)

These are nuclear-data / cross-section-generation notebooks owned by
`njoy-outram-park-fork`. Listed for completeness; their harness is not part of
this run.

| Notebook | Owner | OpenMC API exercised | njoy equivalent / GAP | Notes |
|---|---|---|---|---|
| `nuclear-data` | njoy | `data.IncidentNeutron`, ACE/ENDF reconstruction, `data.Reaction` | njoy RECONR/BROADR port + `XsProvider` | Data-generation notebook. |
| `nuclear-data-resonance-covariance` | njoy | `data.ResonanceCovariance`, resonance parameters | Covariance handling (largely unported) | |
| `search` | njoy | `search_for_keff`, criticality search driver | Needs a k-eff search loop over a parametrised model | Depends on a working geometry+transport path too. |
| `mgxs-part-i` | njoy | `mgxs.Library`, `mgxs.EnergyGroups`, tally-to-MGXS | MGXS generation | |
| `mgxs-part-ii` | njoy | `mgxs.Library`, domain-by-domain MGXS | MGXS generation | |
| `mgxs-part-iii` | njoy | `mgxs.Library`, transport-corrected MGXS | MGXS generation | |
| `mdgxs-part-i` | njoy | `mgxs.MDGXS`, multi-delayed-group XS | Multi-delayed-group XS generation | |
| `mdgxs-part-ii` | njoy | `mgxs.MDGXS`, delayed-neutron data | | |

## V&V status of the live cases

The live cases are now `pincell` (Godiva bare-sphere k-eff), `hexagonal-lattice`
(geometry correctness + k smoke), `triso` (doubly-het k∞ by delta tracking), and
`depletion` (one-group burnup trends). The benchmark-gated case is the Godiva one:

- **`pincell` (partial) — Godiva bare-sphere k-eff.** Methodology & measured
  results are documented in the test module and in
  `src/physics/keff.rs` (LOW-tier k_eff ~ 1.010 +/- 0.002 vs ICSBEP
  HEU-MET-FAST-001 1.0000 +/- 0.0010, 2026-07). The harness test asserts a broad
  plausibility band, not a benchmark gate — it guards the transport chain, not
  accuracy. This is a *criticality-eigenvalue* stand-in for the pincell notebook,
  **not** the notebook's LWR thermal pin cell.

The remaining (still-gated) rows are `#[ignore]` with the gap bead recorded in the
ignore reason — no fabricated passing tests.
