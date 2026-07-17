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
- `tally::scoring` — flux / reaction-rate scoring (implemented): both the
  collision estimator (`score_collision`) and a **track-length estimator**
  (`score_track_length` + per-batch `flush_batch`), the latter now driving the
  CSG loop and the energy-binned flux spectrum (op-6tz.9).
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

The **multigroup (MG) k-eigenvalue path is now live** too:
`physics::physics_mg` supplies the MGXS data types (`Mgxs` = `XSdata`/`Macroscopic`,
`MgxsLibrary` = `MGXSLibrary`) and `run_keff_mg`, the MG twin of `run_keff_csg`
(group-indexed collision physics — group total, absorption/fission split, χ birth
spectrum, scatter-matrix group transfer — over the same CSG geometry). It consumes
a supplied MGXS set; MGXS *generation* stays njoy-side.

Still genuinely gated (`#[ignore]` with a documented gap): the generic
history-based `physics::transport` variant, MG cross-section **plotting** and
**mesh tallies** (mg-mode-part-ii/iii, on top of the now-live MG kernel),
DAGMC / unstructured-mesh geometry,
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
| `flux-spectrum` | outram-mc | `examples.pwr_pin_cell`, `EnergyFilter`, `mgxs.GROUP_STRUCTURES`, `Tally`, `StatePoint` | **LIVE** — the CSG loop now drives a **track-length flux estimator** (`tally::scoring::score_track_length` + per-batch `flush_batch`) that scores an `EnergyFilter`-binned flux; live test asserts spectrum shape (fast tail + slowing-down side, finite/normalized, converged) | **yes** | op-6tz.9 (→.8,.7) | Fast HEU + free-gas-H infinite pin cell; energy-binned track-length flux over a 50-bin log grid. See `tests/openmc_notebooks/flux_spectrum.rs`. Volume normalization / derived-tally arithmetic is downstream op-6tz.22. |
| `gamma-detector` | outram-mc | `data.decay_photon_energy`, `Source`, `stats.Discrete`/`Isotropic`, photon transport, `EnergyFilter` | Neutron-only; no photon transport, no decay-photon source | no | op-6tz.19 | Photon transport is out of scope (`src/photon.cpp` not ported). |
| `post-processing` | outram-mc | `StatePoint`, `RegularMesh`, `MeshFilter`, `Tally`, mesh reshaping | No StatePoint persistence / mesh tally | no | op-6tz.22, .13 | Reads back tally results for plotting; needs scored mesh tallies + StatePoint. |
| `pandas-dataframes` | outram-mc | `Tally.get_pandas_dataframe`, `DistribcellFilter`, `MeshFilter`, `EnergyFilter`, `Trigger` | No DataFrame export, no distribcell/mesh scoring | no | op-6tz.22 (→.9) | Pandas export is an inspection layer atop scored tallies. |
| `mg-mode-part-i` | outram-mc | `XSdata`, `Macroscopic`, `MGXSLibrary`, `RectLattice`, multigroup `run` | **LIVE** — `physics::physics_mg`: `Mgxs` (`XSdata`/`Macroscopic`) + `MgxsLibrary` (`MGXSLibrary`) MGXS data types + `run_keff_mg` multigroup k-eigenvalue over CSG geometry | **yes** | op-6tz.15 | Multigroup transport execution is outram-mc's (MGXS *generation* is njoy). Live test asserts a **2-group infinite-medium k∞ = 1.10085 ± 0.00175 vs analytic 1.10000** (reflective cube), plus a leakage-monotonicity smoke. See `tests/openmc_notebooks/mg_mode_part_i.rs`. |
| `mg-mode-part-ii` | outram-mc | `mgxs.Library`, `plot_xs`, `MeshFilter`, multigroup `run` | MG transport now live (`run_keff_mg`); MG cross-section **plotting** (`plot_xs`) + mesh tally still absent | no | op-6tz.15 | Needs MGXS plotting + mesh tally on top of the now-live MG kernel. |
| `mg-mode-part-iii` | outram-mc | `mgxs.Library`, `RectLattice`, `MeshFilter`, multigroup `run` | MG transport now live; spatial **mesh tally** filter still absent | no | op-6tz.15 | Needs a mesh tally filter on top of the now-live MG kernel. |
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
(geometry correctness + k smoke), `triso` (doubly-het k∞ by delta tracking),
`depletion` (one-group burnup trends), `mg-mode-part-i` (multigroup k∞ vs an
analytic 2-group reference), and `flux-spectrum` (energy-binned track-length flux
spectrum). The benchmark-gated case is the Godiva one:

- **`flux-spectrum` — energy-binned track-length flux spectrum (op-6tz.9).**
  Methodology & results are in `tests/openmc_notebooks/flux_spectrum.rs`. A fast
  HEU + free-gas-H infinite reflective pin cell is run with a single `EnergyFilter`
  (50 log bins, 1e-3 eV – 20 MeV) scoring the **track-length flux**
  (`tally::scoring::score_track_length`, per-batch `flush_batch`). Pass criterion
  is physical spectrum shape (no reference k): all bins finite/non-negative,
  fractions normalize to 1, a substantial fast tail (E > 0.1 MeV) *and* substantial
  below-0.1-MeV slowing-down flux, and a converged peak bin. Measured
  (2026-07-17, deterministic): k_inf = 1.82844 ± 0.00917, fast fraction = 0.674,
  below-0.1-MeV fraction = 0.326, peak-bin (1.87–3.00 MeV) batch rel-sd = 0.027.
  This verifies the track-length energy-binned tally wiring, not spectral accuracy;
  volume normalization / derived-tally arithmetic is downstream op-6tz.22.

- **`mg-mode-part-i` — multigroup infinite-medium k∞.** Methodology & results are
  documented in `tests/openmc_notebooks/mg_mode_part_i.rs` and the unit test in
  `src/physics/physics_mg.rs`. A 2-group macroscopic MGXS set with a closed-form
  infinite-medium eigenvalue k∞ = 1.10 is run in a reflective cube (zero leakage);
  measured k∞ = 1.10085 ± 0.00175 (+0.5σ, 2026-07-17). A supplementary
  vacuum-cube leakage smoke asserts only physics-sanity monotonicity (no invented
  reference). This verifies the MG collision physics and CSG transport, not a
  benchmark accuracy gate.

- **`pincell` (partial) — Godiva bare-sphere k-eff.** Methodology & measured
  results are documented in the test module and in
  `src/physics/keff.rs` (LOW-tier k_eff ~ 1.010 +/- 0.002 vs ICSBEP
  HEU-MET-FAST-001 1.0000 +/- 0.0010, 2026-07). The harness test asserts a broad
  plausibility band, not a benchmark gate — it guards the transport chain, not
  accuracy. This is a *criticality-eigenvalue* stand-in for the pincell notebook,
  **not** the notebook's LWR thermal pin cell.

The remaining (still-gated) rows are `#[ignore]` with the gap bead recorded in the
ignore reason — no fabricated passing tests.
