# HTR-10 neutronics — scoping for a reproducible multifidelity pipeline

Scoping document for HTR-10 **core physics**: the IAEA-TECDOC-1382 benchmark
problems B1-B4, and the four-rung fidelity ladder the maintainer has directed
this work to be built as.

Companion to [htr10.md](htr10.md), which scopes the *thermal-hydraulic* side of
the same reactor. That document's capability audit is not repeated here.

> **Intended use.** Education, research, capability building, and V&V only.
> Offline; no connection to any operational system. See `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings are from a codebase audit
> performed **2026-08-11** and every one carries a `file:line`. Literature
> values are quoted from documents catalogued in `crates/kovan-literature/`,
> with their access tier stated. **Every literature k_eff, critical height and
> rod worth quoted here was computed by someone else, not by this project.**
> The single exception is section 11, which reports a k_inf this project *did*
> compute — and which is a code-exercise result, not a physics result, for the
> reasons given there. Where a number is ours, it says so and names the test or
> example that produced it.

---

## 1. Framing — read this first

### 1.1 There was no HTR-10 neutronics in this workspace

Before 2026-08-11 there was none, and there is still no *transport model* of
the reactor. What landed today is:

- `crates/outram-park-digital-twin-engine/src/htr10/` — design constants
  (`design.rs`), KTA packed-bed pressure drop (`kta.rs`), Zehner-Bauer-Schlunder
  bed conductivity (`zbs.rs`). **Thermal-hydraulics only.**
- `crates/outram-park-digital-twin-engine/src/htr10/neutronics.rs` — added by
  this work: the B1-B4 benchmark specification, the fuel/dummy pebble and TRISO
  layer stack, the core geometry the sources state in text, the measured first
  criticality, and 45 published reference values from four sources. **Data and
  tests only — it computes no transport.**

The IAEA benchmarks B1-B4 are *neutronics* benchmarks, so nothing in the
thermal-hydraulic module can meet them.

### 1.2 The deliverable is a pipeline, not a number

Per maintainer direction (2026-08-11), HTR-10 neutronics is to be built as a
**multifidelity pipeline** in which each rung is validated against the rung
above it:

```
  meshing  ->  [1] rigorous Monte Carlo  ->  [2] MGXS  ->  [3] deterministic
               (outram-mc-libs)              generation      multiphysics
                                                             (GeN-Foam)
                                                                 |
                                                                 v
                                                    [4] PRKE + decay heat
                                                        (teh-o-prke)
```

Retrofitting the validation chain afterwards does not work, so the interfaces
are specified in section 5 before any rung is built.

### 1.3 Two hard constraints on the architecture

1. **Reproducibility** (section 6). Every hand-off is an on-disk artefact with
   provenance, every run writes a manifest, MC is bit-identical for a fixed
   seed, and the whole chain is drivable headlessly.
2. **Frontend-drivable** (section 6.4). `crates/outram-blender` is to gain this
   as a third solver bridge (`op-hzs.53`) alongside its existing `mc-export`
   and `foam-mesh` bridges. The entry point must therefore take *a geometry
   description plus a run configuration*, not be callable only from a test.

Both constraints point the same way, which is a good sign the shape is right.

---

## 2. The benchmark problems, and the trap in B1

All four are defined in IAEA-TECDOC-1382 Chapter 4 (Open tier;
`crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`).

> **Source deduplication (resolved 2026-08-11).** An earlier ingest of the
> *same Chapter 4* without TECDOC provenance (`htr-10-iaea.md` / `.json`) has
> been removed from the archive; `iaea-tecdoc-1382-part2` is the sole record.
> Cite IAEA-TECDOC-1382 part 2.

| Problem | What it asks | Answer type |
|---|---|---|
| **B1** | Loading height (from the upper surface of the conus) at which k_eff = 1, helium, core temperature 20 C, no rods inserted | a **height**, not a k_eff |
| **B21/B22/B23** | k_eff of the full 5 m^3 core under helium at 20 / 120 / 250 C, no rods | k_eff |
| **B31/B32** | Reactivity worth of ten fully inserted rods / of one rod (others withdrawn), full core, helium, 20 C | percent delta-k/k |
| **B41/B42** | Same for the initial core at a 126 cm loading; B42 is the differential worth of one rod at seven stated axial positions | percent delta-k/k |

Verbatim definitions: B2 at part 2 line 2287, B3 at line 2291, B4 at lines
199-201. **The formal B1 definition subsection is missing from the markdown
conversion** — section 4.1.2.1's body was absorbed into the Table 4-3 dump
(the gap is lines 170-198). The definition survives as INET's restatement at
line 265 and CEA's at line 1383. Anyone relying on the exact B1 wording should
read the PDF, not the markdown.

### 2.1 "As defined" and "as measured" are different problems

After the benchmark was specified and before the core was loaded, three things
changed (part 2, section 4.2.1.3, lines 321-331). The literature calls the
result the **deviated** benchmark, versus the **original**:

| | Original (as defined) | Deviated (as built) |
|---|---|---|
| Dummy-ball graphite density | 1.73 g/cm^3 | **1.84 g/cm^3** |
| Dummy-ball boron equivalent | 1.3 ppm | **0.125 ppm** |
| Core atmosphere | helium | **humid air, 0.1013 MPa** |
| Core temperature | 20 C (definition) | **15 C** in the experiment |

Humid air fills the upper cavity *and* the inter-pebble spaces; water-vapour
density 2.57e-5 g/cm^3, air density 1.149e-3 g/cm^3, oxygen 23.14% and nitrogen
75.53% (part 2, line 335).

**This is worth about +1000 pcm.** Two independent open sources agree:
IAEA VSOP at a 126 cm loading gives 1.000448 original versus 1.010562 deviated
(+1011 pcm), and Choo and Xiao (2024) Serpent 2 gives 1.01474 versus 1.02415
(+941 pcm). That is *larger than the entire published code-to-code spread* on
this problem (683 +/- 22 pcm between MCNP5 and SCALE6 on identical geometry and
library, Wang et al. 2014). Comparing an as-defined calculation against the
as-measured experiment would therefore be a bigger error than any code
difference in the literature. Both shifts are computed by
`htr10::neutronics::tests::original_to_deviated_shift_agrees_between_two_independent_sources`.

Three further traps, all recorded in code:

- **The oxygen and nitrogen percentages sum to 98.67%.** The source does not
  name the remaining 1.33%. `HumidAirComposition::unaccounted_fraction` exposes
  the gap so a model must decide explicitly rather than silently renormalise.
- **1.84 versus 1.86 g/cm^3.** Tantillo et al. (2020) and the prose of Choo and
  Xiao (2024) both state 1.86; Choo and Xiao's own Table 1 states 1.84, as does
  IAEA-TECDOC-1382 twice. This module follows the primary source (1.84).
- **20 C versus 27 C.** The benchmark text defines B1/B2's lowest temperature
  as 20 C; INET's own tables and most later papers report 27 C (300.15 K, a
  standard library temperature). IAEA Table 4-4 gives both columns; they differ
  by about 15 pcm at the critical loading. Say which you used.

### 2.2 The measured target

First criticality, December 2000: **16,890 balls (9,627 fuel + 7,263 dummy,
57:43), loading height 123.06 cm, under 15 C air** (part 2, line 411).
Approach by inverse-count-rate extrapolation from three side-reflector
counters, with a 20 Ci Am-Be start-up source.

**The document states no uncertainty on any of it** — no ball-count tolerance,
no height tolerance, no temperature tolerance (searched: the only `+/-` figures
in the whole document are Monte Carlo statistical sigmas on *calculated*
k_eff). A comparison against this measurement therefore cannot quote an
experimental error bar and must say so instead of inventing one. The CSIs'
own list of uncertainty sources is at part 2 lines 2159-2163: dummy-block
impurity level, water and air content of graphite pores, Monte Carlo modelling
of coated particles, cross-section library choice, neutron streaming in
diffusion methods, and harmonics in thin annular cores.

A useful consequence, verified in
`tests::measured_ball_count_follows_from_loading_height`: 123.06 cm and 16,890
balls are **not independent measurements**. Filling a plain cylinder of 180 cm
diameter to 123.06 cm at f = 0.61 with 6 cm pebbles gives **16,890.0** balls
(residual -0.0001%, measured by that test on 2026-08-11) — the height was
derived from the count. A model that packs to a different fraction
must reconcile with the *count*, not the height.

### 2.3 Reference values now captured in code

`crates/outram-park-digital-twin-engine/src/htr10/neutronics.rs` holds 45
published values, each carrying its source and that source's access tier:

| Set | Count | Source | Tier |
|---|---|---|---|
| INET VSOP/MCNP B1 loading curves (4 tables, original + deviated) | 31 points | IAEA-TECDOC-1382 | Open |
| INET B2 full-core k_eff, both variants | 7 | IAEA-TECDOC-1382 | Open |
| INET B3/B4 rod worths + the B42 differential curve | 14 + 7 | IAEA-TECDOC-1382 | Open |
| Choo and Xiao (2024) Serpent 2 and HCP, B1 + B2, both variants | 16 | Choo and Xiao 2024 | Open |
| Wang et al. (2014) continuous-energy MCNP5 and SCALE6 | 6 | Wang 2014 | **Proprietary** |
| Wang et al. (2014) unit-cell homogenisation biases | 6 | Wang 2014 | **Proprietary** |
| Tantillo et al. (2020) infinite-pebble-bed k_inf | 2 | Tantillo 2020 | **Proprietary** |

The multi-country collation (part 2, Table 5-14, line 2273) is **not** yet in
code and should be: it spans ten countries and is the honest picture of how
hard this benchmark is.

| Country | Original, diffusion/transport | Original, Monte Carlo | Deviated, D/T | Deviated, MC |
|---|---|---|---|---|
| China (INET) | 125.8 | 126.1 | 122.558 | 122.874 |
| France (CEA) | - | - | - | 115.36 / 117.37 |
| Germany (FZJ) | 124.2 / 126.8 | - | 121.0 / 123.3 | - |
| Indonesia | 107 / 120 | - | - | - |
| Japan | 113 | - | - | - |
| Netherlands | 125.3 | - | 122.1 | - |
| Russia | 136 | 137.3 | - | - |
| South Africa | - | - | 122.537 | - |
| Turkey | 119.27 | 129.7 / 135.3 | - | - |
| USA (MIT) | - | 127.5 / 128 | - | - |
| **Experiment** | | | | **123.06 cm** |

All critical loading heights in cm. Spread across participants: 107 to 137 cm
against a measured 123.06 cm. **Being within a few centimetres of the
measurement is a real achievement on this problem, not a given.**

---

## 3. Capability audit

Audited 2026-08-11. Every claim carries a `file:line`.

### 3.1 HAVE — real, tested, and wired to real cross sections

| Capability | Where | Notes |
|---|---|---|
| **Woodcock delta tracking** | `crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:270` | `track_to_collision` takes geometry as a closure `Fn(Position) -> Option<f64>` returning local total macroscopic cross section. Generic, no trait object |
| **Resonance-safe majorant** | `.../delta_tracking.rs:125` | `Majorant::bounding` sub-samples each bin for the max. Use this, not `from_materials` (`:75`), which under-bounds across U-238 resonances — its own doc says so at `:99-103` |
| **Delta-tracked k-eigenvalue with sigma** | `.../pebble_beds/keff_delta.rs:239` | `run_keff_delta` returns `KeffResult { k_mean, k_std, k_by_generation }` (`crates/outram-mc-libs/src/physics/keff.rs:183`). Sequential (`:285`) and rayon (`:382`) backends, thread-count-invariant by seed jump-ahead |
| **RSA sphere packing** | `.../pebble_beds/sphere_packing.rs:205` | `pack_spheres(radius, half_width, packing_fraction, seed)`, plus O(1) membership `PackedSpheres::is_inside_kernel` (`:376`). Line-by-line port of OpenMC's `model/triso.py` |
| **Full CSG geometry** | `crates/outram-mc-libs/src/geometry/surface.rs:1021` | `SurfaceKind` enum covers sphere, X/Y/Z cylinder, X/Y/Z cone, planes, quadric — enough to express an HTR-10 core, conus and reflector |
| **Surface-tracked CSG k-eff with tallies** | `crates/outram-mc-libs/src/physics/transport_csg.rs:150` | `run_keff_csg(geom, materials, nuclides, source_box, settings, tally)`. This is the path that can express leakage; `run_keff_delta` cannot |
| **Materials and nuclides at two fidelity tiers** | `crates/outram-mc-libs/src/material/nuclide.rs:187`, `:253` | `from_core` (embedded WMP, LOW) and `from_endf` (download + RECONR/BROADR, HIGH, behind `net-fetch`) |
| **Carbon nuclear data** | `crates/njoy-outram-park-fork/src/acquire.rs:180-181`; `crates/njoy-outram-park-fork/docs/wmp-nuclide-manifest.md:26` | **Bead `op-h23` is stale.** C-12 (MAT 625) and C-13 (MAT 628) *are* in `well_known_mat`, and `C0` (C-nat) is in the embedded CORE WMP blob. See section 7.1 |
| **Incoherent-inelastic S(alpha,beta) reaching transport** | `crates/outram-mc-libs/src/material/thermal.rs:122` -> `nuclide.rs:221` -> `nuclide.rs:333-341` | Works; but it is the wrong channel for graphite. See section 7.2 |
| **Benchmark specification as typed data** | `crates/outram-park-digital-twin-engine/src/htr10/neutronics.rs` | Added by this work |
| **PRKE with a properly documented decay-heat model** | `crates/teh-o-prke/src/decay_heat.rs` | Replaced 2026-08-11 with the 23-group 1978 draft ANS standard fit; this is the doc/test standard rung 4 should match |

### 3.2 SCAFFOLD — do not count as working

- **`run_keff_delta` is reflective-cube-only.** `advance_reflective`
  (`crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:97-151`) reflects off six
  walls of a cube of half-width `half_width`. It therefore computes **k_inf of
  an infinite medium with zero leakage** — there is no cylinder, no reflector
  and no vacuum boundary. An HTR-10 *core* k_eff needs either a boundary
  treatment here or the CSG driver.
- **`ComputeType::Gpu` on delta tracking silently degrades to CPU**
  (`keff_delta.rs:257-272`) — logs a debug line, never errors.
- **LOW-tier nu-bar is flat and chi is a Watt stand-in**
  (`crates/outram-mc-libs/src/material/nuclide.rs:196-199`, `:1110-1124`) — the
  crate's own note puts this at about +500 pcm on Godiva.
- **`Mgxs` fast fallback is unshielded and temperature-independent**
  (`crates/njoy-outram-park-fork/src/nuclear_data/mod.rs:117-124`): "no Boltzmann
  self-shielding solve, no Bondarenko dilution, no URR treatment".
- **`k_std` is a generation-batch standard error** (`keff_delta.rs:614-625`),
  with no inter-generation correlation correction — so it under-states the true
  uncertainty. Report it as what it is.
- **`XsProvider` is not actually consumed by `outram-mc-libs`.** `lib.rs:6`
  claims it is; the real dependency edge is direct imports of
  `WindowedMultipole` / `Mgxs` / `NuBar` / `FissionSpectrum` / `ReconrResult` at
  `crates/outram-mc-libs/src/material/nuclide.rs:23-30`. Do not go looking for an
  `XsProvider` seam here — it is not the seam.

### 3.3 MISSING — with the line that proves it

| Gap | Evidence | Consequence for HTR-10 |
|---|---|---|
| **Cylindrical packing bounds** | `sphere_packing.rs:205`, `:98-104` — one cube `half_width` | Cannot pack the 180 cm cylindrical bed |
| **Two pebble species** | `sphere_packing.rs:99`, `:195` — one scalar `radius`, "all particles equal-radius" | Cannot represent the 57:43 fuel/dummy mixture, which *is* the initial core |
| **Packing fraction above 0.38** | `MAX_PF_RSA = 0.38` at `sphere_packing.rs:63`, rejection at `:211-216`; `RsaDem`/`OdrDem` return `Err(NotImplemented)` at `:160`, proved by the test at `:546` | HTR-10's bed is **0.61**. Plain RSA cannot reach it — this is the single hardest missing piece for an explicit-bed model |
| **Graphite S(alpha,beta) reaching transport** | `crates/outram-mc-libs/src/material/thermal.rs:24-26`: "Coherent / incoherent-elastic bound scattering (graphite, ZrH) is deliberately not wired here yet"; `crates/njoy-outram-park-fork/src/acquire.rs:172-177`: the `tsl-crystalline-graphite` MAT 30 tape is "**not** reachable through this table" | First-order physics error on a graphite-moderated thermal system. See section 7.2 |
| **B-10 / B-11 / Si-28-30 in `well_known_mat`** | `crates/njoy-outram-park-fork/src/acquire.rs:179-195` | HIGH-tier fetch of the boron poison and the SiC coating is blocked; LOW tier via `from_core` is unaffected |
| **Any MGXS generation path** | see rung 2, section 4.2 | No rung-2 output exists |
| **The Figure 4.10 zone geometry** | IAEA part 2 line 189 is a bare figure caption; Table 4-3 (lines 179-187) gives only zone number, carbon and boron atom density, and a remark | The R-Z model's zone boundaries are **not recoverable from the text**. See section 7.3 |

### 3.4 Reference data that is present, and reference data that is not

- **Present and immediately usable**: IAEA part 2 Table 4-3 gives homogenised
  carbon and natural-boron atom densities for all 83 R-Z zones (lines 179-187);
  Table 4-37 gives the pebble-bed cell geometry — fuel pebble R 3.0 cm, fuelled
  zone R 2.5 cm, packing fraction 0.61, moderator-pebble R 2.7310 cm, BCC cell
  6.8773 cm (line 1089); Table 4-38 gives explicit atom densities — graphite C
  8.674169E-02, B-10 2.244010E-08, B-11 9.032424E-08; kernel U-235 3.992067E-03,
  U-238 1.924449E-02, O 4.647329E-02 (line 1101). **These are the numbers a
  transport model needs and they are already in the open archive.**
- **Absent**: the Virtual Test Bed HTR-10 case
  (`reference-data/virtual_test_bed/htgr/htr10/steady/`) ships only the two
  Griffin input files, a `tests` file and an exodiff spec. The mesh
  (`../data/mesh/htr-10-critical-a-rev6.e`), the ten-group cross-section library
  (`../data/xs/htr-10-XS.xml`), the SPH library and **all gold Exodus files are
  not present on disk** — and are not git-LFS pointers, since the repository has
  no `.gitattributes`. Other VTB models do ship their artefacts, so this is
  specific to HTR-10.
  - The one gold eigenvalue recoverable from the checkout is
    **k = 1.1234735**, recorded as a comment in
    `custom_cmp_no_connected_region_id:32` for the *full core, all-rods-out,
    SPH-corrected 10-group diffusion* case. This corrects
    [vtb-findings.md](vtb-findings.md), which reports two eigenvalues including
    an "initial critical 1.0009032234669475" — **no gold for the critical
    configuration is recoverable from these four files**; the same exodiff spec
    is reused for both tests (`tests:21`).
  - The VTB authors' own caveat, `# Incomplete XS library`, appears at
    `tests:22` and `tests:35`. There is **no** CI truncation (`num_steps`,
    `fixed_point_max_its`, `max_its` appear nowhere) — only wall-clock guards.
  - So VTB is a useful *modelling reference* (block structure, group count,
    which states the library carries: `htr-10-full-1RI/-393K/-523K/-ARI/-ARO`,
    `htr-10-full.i:39-40`) but **not** a runnable benchmark here.

---

## 4. The four rungs

### 4.1 Rung 1 — rigorous Monte Carlo (`outram-mc-libs`)

**This is the reference the other three rungs are judged against**, so its
credibility is the pipeline's credibility.

Target sequence, cheapest first, each with a published number to hit:

| Step | Problem | Published target | Why first |
|---|---|---|---|
| 1a | TRISO grain unit cell, infinite medium | none directly | Exercises the doubly heterogeneous packing at the level RSA *can* reach (about 5% packing) |
| 1b | Infinite pebble-bed lattice k_inf | Serpent 1.6321 / HCP 1.6416 (Tantillo 2020) | No core geometry, no reflector, no leakage. The cheapest real comparison |
| 1c | BCC fuel + graphite ball lattice k_inf | MCNP5 1.77078 +/- 0.00008 (Wang 2014, config b) | Adds the 57:43 mixture without a core |
| 1d | B1 detailed core k_eff at the measured loading | MCNP5 1.01620 +/- 0.00014 (Wang 2014, config c) | The real thing |

Steps 1a-1c are within reach of the *existing* reflective-cube driver. Step 1d
needs the CSG path plus a bed model.

**Prerequisites that are not negotiable** (section 7).

### 4.2 Rung 2 — MGXS generation

**Architectural rule**: cross-section code lives in `njoy-outram-park-fork`;
transport crates are data-free and pull from it. So the MGXS *generator* belongs
in the nuclear-data crate, and the MC *tallies* that feed it belong in
`outram-mc-libs`.

Existing beads to wire to, **not** duplicate. Both are further along than the
bead titles suggest:

- **`op-6tz.15`** (in progress) — multigroup transport is **live**:
  `crates/outram-mc-libs/src/physics/physics_mg.rs` carries `Mgxs`,
  `MgxsLibrary` and `run_keff_mg` (group-indexed collision physics over CSG
  geometry), verified on a 2-group infinite medium at k_inf = 1.10085 +/- 0.00175
  against an analytic 1.10000 (+0.5 sigma). What remains is MG cross-section
  plotting and spatial mesh tallies — tally features, not transport.
- **`op-6tz.6.3`** (in progress) — both halves are live on real U-235
  ENDF/B-VIII.0 (MAT 9228), 2-group, 1/E weighting: a GROUPR elastic scatter
  matrix (fast-row sum 9.74251 b, matching the vector to better than 1e-6),
  Bondarenko-dilution self-shielded total cross sections (infinite dilution
  [1071.15, 35.19] b; sigma_0 = 1 gives [429.52, 14.44] b, monotone), and a
  group chi summing to 1. **Explicitly not validated against a real NJOY GENDF
  golden tape** — self-consistency only. Open gaps: incident-energy-dependent
  chi(E'|E) from MF=5, and the URR MF=2/MT=152 tape from UNRESR/PURR.

So rung 2's *machinery* substantially exists; what does not exist is an HTR-10
group structure, an HTR-10 tally set, and the bridge into GeN-Foam's format.

**Group structure.** Recommendation to the maintainer, for decision: start with
the **VTB HTR-10 ten-group structure** (`htr-10-full.i:196`, `G = 10`), because
it is what the only available HTGR reference deck uses and it makes the rung
2-to-3 comparison against a published Griffin model meaningful. Fall back to a
broader 6-group structure only if 10 proves unstable — Tantillo et al. note
that 6 broad groups already give "acceptable results compared to Monte Carlo"
for this reactor type. **Whichever is chosen must be recorded in the manifest.**

### 4.3 Rung 3 — deterministic multiphysics: **GeN-Foam** (decided)

Maintainer decision, 2026-08-11: **use the GeN-Foam port**
(`crates/outram-foam-appbuilder-lib/src/genfoam/`), not `crates/bedok`.

`bedok` remains in the workspace as a *different* fidelity band (3-D nodal
diffusion coupled to channel TH) and is noted here as an alternative, not as
this rung. `op-d3e` (P1) records 10 `todo!()` panics in
`crates/bedok/src/reference/coupling/seam.rs` (lines 471, 782, 804, 826, 849,
890, 925, 955, 984, 1011), all reachable from `steady.rs`, `transient.rs` and
`critical_boron.rs`. Worth noting for the record: `reference/nodal/` (16 files)
and `reference/th/` (11 files) contain **zero** `todo!()` — the physics is
there, only the seam was never rewired, so `op-d3e` is routing work rather than
new physics. That makes bedok a cheaper second opinion later than it looks.

GeN-Foam is an **AI-assisted draft with no human V&V**, so nothing in it may be
called validated. Its *implementation* state, however, is better than the bead
titles imply and was checked directly on 2026-08-11: **32,334 lines, 262
`#[test]`, and not a single `todo!()` anywhere in the `genfoam` subtree.**

| Module | Lines / tests | State |
|---|---|---|
| `neutronics/xs/` | 2167 / 13 | `CrossSectionData` (`xs/mod.rs:134`), `NuclearDataOneEnergy` (`xs/nuclear_data_one_energy.rs:63`), **`GroupConstants` (`xs/group_constants.rs:59`)** |
| `neutronics/diffusion/` | 1342 / 4 | `DiffusionNeutronics` (`diffusion/mod.rs:163`), real `solve_eigenvalue` (`diffusion/eigenvalue.rs:63`) and `step` (`diffusion/transient.rs:69`) |
| `neutronics/sp3/` | 1464 / 5 | `Sp3Neutronics` (`sp3/mod.rs:162`), solvers at `sp3/eigenvalue.rs:66`, `sp3/transient.rs:68` |
| `neutronics/sn/` | 1577 / 6 | `SnNeutronics` (`sn/mod.rs:197`), sweep in `sn/sweep.rs` |
| `neutronics/point_kinetics/` | 616 / 5 | `PointKineticsState::step` (`point_kinetics/mod.rs:433`) |
| `multi_region/` | 3953 / 24 | meshToMesh, RBF non-conformal mapping, Picard outer loop, `ReactivityFeedback` |
| `thermal_hydraulics/` | 17247 / 168 | the bulk |
| `thermo_mechanics/` | 2152 / 16 | |

**`GroupConstants` is the concrete MGXS landing struct** and it is already
`uom`-typed: `diffusion_coefficient` (m), `nu_sigma_f` (1/m), `sigma_pow`
(J/m), `sigma_removal` (1/m), `chi_prompt`, `chi_delayed`, `inverse_velocity`
(s/m), `disc_factor`; with `PrecursorConstants` (`xs/group_constants.rs:85`)
carrying `beta`, `beta_tot` and `lambda`.

**Two real caveats.**

1. **The `nuclearData` format is not an njoy/OpenMC MGXS library.** It is
   GeN-Foam's dict, *parameterised by feedback variables* via polyharmonic-spline
   RBF interpolation across perturbed reactor "states". So the rung 2 -> 3
   bridge is not a file-format conversion; it is "run rung 1 at N perturbed
   states and fit". That is unbuilt work and should be beaded as such.
2. **The `gen-foam` CLI is an honest stub.**
   `crates/outram-foam-cli/src/bin/gen_foam.rs:34-41` returns an error; its two
   stated prerequisites (`:18-21`) are a case-constructible top-level solver and
   a `constant/nuclearData` reader. That is bead **`op-p6p.16`**, and it is the
   single thing standing between the implemented physics and a runnable
   deterministic rung. (Note `gen_foam.rs:7-10`'s doc comment is stale — it
   claims only point kinetics is translated, which the table above contradicts.)

The choice is good precisely because it makes the seams concrete:

- **Rung 2 -> 3**: `op-p6p.4`, genfoam `nuclearData` cross-section data
  structures — *in progress*. That bead **is** the MGXS hand-off. Wire the MGXS
  plan to it rather than inventing a parallel format.
- **Rung 3 -> 4**: `op-p6p.10`, genfoam pointKinetics reactivity feedback and
  coupling layer. That is the collapse to PRKE parameters.
- Also relevant: `op-p6p.8` (multiRegion cross-mesh coupling) and `op-p6p.3`
  (timeProfile / InterpolateTable time-tabulated inputs).

### 4.4 Rung 4 — PRKE and decay heat (`teh-o-prke`)

The real-time-capable rung. Six-group point kinetics with delayed-neutron
precursors, Doppler and rod-worth feedback, iodine/xenon; decay heat replaced
2026-08-11 with the 23-group 1978 draft ANS Standard fit (Tobias Table 16) for
U-235 thermal, U-238 fast and Pu-239 thermal, integrated analytically.

For HTR-10 specifically, `op-jyyp.6` records the gap that matters: **a separate
graphite/moderator temperature-feedback channel**. Only lumped fuel feedback
exists, and the graphite channel is central to HTR-10 loss-of-flow behaviour.
It must come down rung 3 as its own reactivity coefficient, not be folded into
Doppler.

---

## 5. What each hand-off has to carry

This is where pipelines rot, so it is specified before anything is built.

### 5.1 Meshing -> rung 3

- A polyMesh consumed by GeN-Foam, with its generation parameters recorded.
- Quality metrics **measured and reported**: non-orthogonality and skewness at
  minimum. See section 8.

### 5.2 Rung 1 (MC) -> rung 2 (MGXS)

- The **group structure**, stated explicitly and by name.
- **Flux-weighted, self-shielded** cross sections per group per material zone.
- The **scatter matrix** (group-to-group, with the Legendre order stated).
- **Group chi** (prompt, and delayed by precursor group if rung 4 needs it).
- **Diffusion coefficients**, with the definition used (transport-corrected or
  otherwise) stated — this is a frequent silent inconsistency.
- The **statistical uncertainty on every tallied quantity**, propagated, not
  dropped.

### 5.3 Rung 2 (MGXS) -> rung 3 (deterministic)

- The same data on the deterministic mesh, in the `op-p6p.4` `nuclearData`
  format.
- **The homogenisation choice, named.** Wang et al. (2014) quantify the cost of
  getting the double heterogeneity wrong, against continuous-energy MCNP5 on the
  detailed HTR-10 model:

  | Treatment | Bias vs CE MCNP5 |
  |---|---|
  | INFHOMMEDIUM | **+2820 +/- 19 pcm** |
  | LATTICECELL | +681 +/- 24 pcm |
  | MULTIREGION | +661 +/- 21 pcm |
  | LATTICECELL (CELLMIX) | +653 +/- 21 pcm |
  | MULTIREGION (CELLMIX) | +470 +/- 22 pcm |
  | DOUBLEHET | **+276 +/- 20 pcm** |

  The worst treatment is 10.2 times the best. **A homogenised pebble-bed model
  cannot reach the ~200 pcm accuracy this benchmark comparison needs** — which
  is exactly why rung 1 must be an explicit doubly heterogeneous Monte Carlo,
  and why the rung 2 -> 3 homogenisation must be measured against it rather
  than assumed. Captured in code as
  `htr10::neutronics::wang_2014_unit_cell_bias`.

### 5.4 Rung 3 (deterministic) -> rung 4 (PRKE)

- **beta_eff** and **Lambda**, collapsed from the flux solution with the
  adjoint weighting used stated.
- **Reactivity coefficients**: fuel Doppler **and** the separate
  graphite/moderator channel (`op-jyyp.6`).
- Rod worth as a function of insertion, for comparison against B42.

---

## 6. Reproducibility as an architectural constraint

### 6.1 What it must mean here

1. **Every stage re-runnable from committed inputs**, without re-running the
   stage above. Each hand-off is an on-disk artefact, not an in-memory value.
   The MGXS library in particular must be a file a human can inspect and diff.
2. **A manifest per run** recording: workspace git commit; cross-section library
   and version (e.g. ENDF/B-VIII.0 and the exact tape); the fidelity tier
   (`from_core` LOW versus `from_endf` HIGH); MC RNG seed, particles per
   generation, total and inactive generations; the group structure; the
   homogenisation choice; the mesh and its generation parameters; and the code
   path taken at each rung. **A result whose data version is unknown is not
   reproducible.**
3. **Deterministic given a seed.** A fixed seed must give bit-identical results
   on the same build — `run_keff_delta_seq` is already documented as the
   bit-reproducible reference (`keff_delta.rs:285-295`) and the parallel backend
   is thread-count-invariant by seed jump-ahead (`:382-393`). Report sigma with
   every MC number; a k_eff without sigma cannot be compared to anything.
4. **Scriptable, not interactive** — which is also what makes rung-to-rung
   validation automatable in CI.
5. **Intermediate artefacts are the validation evidence.** A pcm comparison
   between rungs means nothing if the artefacts it came from are gone.

### 6.2 The obstacle that must not be designed around silently

**Rung 1's reproducibility is currently compromised by two open, verified P1
RNG defects in `outram-mc-libs`** — see section 7.4. They are upstream of the
entire pipeline's credibility, because rung 1 is the reference everything else
is validated against. Do not assume the MC reference is trustworthy until they
are closed.

### 6.3 Validation chain — state every comparison in pcm

| Comparison | Quantity | Must carry |
|---|---|---|
| Rung 2/3 versus rung 1 | k_eff, same problem | discrepancy in pcm, **plus the MC sigma** |
| Rung 4 versus rung 3 | beta_eff, Lambda, reactivity coefficients | relative difference, and the flux solution they were collapsed from |
| Whole chain versus B1 | critical loading height | cm and pcm, and **which variant** (original or deviated) |
| Whole chain versus B2 | k_eff at 20/120/250 C | pcm per temperature, and the isothermal coefficient in pcm/C |
| Whole chain versus B3/B4 | rod worths | percent delta-k/k |

The published isothermal temperature coefficients to compare against
(IAEA part 2, Table 4-33, line 965; the only ITC table in the document —
the document itself prints delta-k/k per C, the pcm/C conversion is ours):
NRG -7.37e-5 per C over 20-120 C and -8.05e-5 over 200-250 C; INET/VSOP
-7.49e-5 over 20-120 C and -9.15e-5 over 120-250 C.

### 6.4 The intended consumer: a headless, scriptable entry point

`crates/outram-blender` is to gain this pipeline as a **third solver bridge**
(`op-hzs.53`), alongside its existing `mc-export -> sim -> MC Studio` and
`foam-mesh -> foam_mesh -> tet-dual Mesh Studio` bridges. Do not build that now,
and do not edit that crate — but design the entry point as *a function taking a
geometry description plus a run configuration*, so a frontend can drive it.

---

## 7. Blockers — none of these may be papered over

### 7.1 `op-h23` (carbon in `well_known_mat`) — the capability exists

The bead text says C-12/C-nat is missing so a graphite moderator cannot be
reconstructed from ENDF. That is no longer true, and `op-hc2o`'s own ordered
blocker list already records `op-h23` as **done**, though the bead's own status
still reads Todo. Evidence:

- `crates/njoy-outram-park-fork/src/acquire.rs:180-181` lists **C-12 (MAT 625)**
  and **C-13 (MAT 628)**; the function's own doc at `:171` says the table covers
  "the Th/U/Pu actinides, plus carbon (C-12/C-13) for a graphite moderator", and
  the test at `:573-574` asserts `well_known_mat(6, 12) == Some(625)`.
- `C0` (C-nat) is in the embedded CORE WMP blob
  (`crates/njoy-outram-park-fork/docs/wmp-nuclide-manifest.md:26`), so
  `Nuclide::from_core("C0")` should resolve.

**Recommendation: close `op-h23` formally, and open a narrower successor** for
the nuclides that really are missing from `well_known_mat` — B-10, B-11 and
Si-28/29/30, which block the **HIGH-tier** ENDF fetch of the boron poison and
the SiC coating layer (`acquire.rs:179-195`). The **LOW tier is unaffected**:
`B10`, `B11`, `O16`, `Si28/29/30` and `C0` are all in the embedded CORE WMP
blob, which is how the rung-1 example in section 11 builds its materials.
Beads are the maintainer's to close; this is a recommendation, not an action.

### 7.2 Graphite S(alpha,beta) — the physics blocker, actively being worked

A k_eff computed with free-gas scattering on a graphite-moderated thermal
system is **not a criticality result** and must never be presented as one.

Current state, precisely:

- The bound-atom machinery exists in the nuclear-data crate: coherent elastic
  (`crates/njoy-outram-park-fork/src/thermr/coherent.rs:29`, `:53`), incoherent
  elastic (`.../thermr/incoherent_elastic.rs`), incoherent inelastic
  (`.../thermr/scattering.rs:81`), a full LEAPR port (`.../src/leapr/`), and a
  thermal ACE writer (`.../acer/thermal.rs:119`).
- **But only the incoherent-inelastic channel reaches transport.**
  `crates/outram-mc-libs/src/material/thermal.rs:24-26` states verbatim:
  "Coherent / incoherent-elastic bound scattering (graphite, ZrH) is
  deliberately not wired here yet."
- **And the graphite thermal tape is not downloadable.**
  `crates/njoy-outram-park-fork/src/acquire.rs:172-177`: the
  `tsl-crystalline-graphite` (MAT 30), `tsl-reactor-graphite-10P` (MAT 31) and
  `-30P` (MAT 32) materials are "**not** reachable through this table or
  `EndfLibrary::neutron_url`".
- `XsProvider` has no S(alpha,beta) variant at all — thermal scattering attaches
  directly to `Nuclide`, bypassing it.

`op-hc2o` (P1) is the umbrella bead for this critical path and states the gap
bluntly: `crates/outram-mc-libs/src/pebble_beds/` has **zero references to
thermal scattering** — the pebble-bed graphite matrix is pure free gas. What
already works on the njoy side, per that bead: MF=7 parsing of all three
graphite evaluations (MAT 30/31/32), coherent-elastic sigma(E) with the Bragg
cutoff at 1.83e-3 eV and sigma = 4.55 b at 0.0253 eV, and incoherent-inelastic
reaching the free-atom limit. THERMR is real (~1.4k lines); only its card-input
driver is unported.

Its ordered blockers are `op-h23` (done, see 7.1), then **`op-1y4y`** —
coherent-elastic is retained **only at 296 K**, which is a hard blocker for
**B2 at 120 C and 250 C** and for B3/B4 — then `op-u5ju` and `op-nhoa`.
`op-hc2o` also flags that 393 K and 523 K snap to 400 K and 500 K, a 23 K
error at the B22/B23 states.

**Another agent is actively working exactly the `op-1y4y` temperature problem**
in `njoy-outram-park-fork`. **Stay out of that crate.** See also `op-6tz.35`
for the full TRISO shell stack plus graphite S(alpha,beta).

One further data-version caution, relevant to any comparison: Tantillo et al.
(2020) record that large differences have been reported between ENDF/B-VII.0
and ENDF/B-VII.1 results for this reactor, "almost exclusively caused by an
update in the carbon capture cross section" (citing Bostelmann and Strydom,
2015). Every published value in section 2.3 is ENDF/B-VII.0 (INET's MCNP-4A
work is ENDF/B-V). **A calculation on ENDF/B-VIII.0 is not comparing
like-for-like and must say so.**

> Note on this citation: it was decoded from a passage of
> `tantillo2020hcpneutronics.md` corrupted by an OCR substitution artefact.
> The paper's B1/B2 result tables in that markdown are unreadable for the same
> reason and are deliberately **not** transcribed into code. The file needs
> re-ingesting — filed as a bead (section 10).

### 7.3 The R-Z zone geometry is not in the text

IAEA part 2 line 189 is a bare caption, `FIG. 4.10. HTR-10 core physics
calculation model (see Table 4-3)`, and Table 4-3 gives only zone number ->
carbon density -> boron density -> remark. **Core radius, conus angle,
discharge-tube radius, cavity height, and the axial coordinates of every zone
boundary are absent as text.**

What is textually available, and can partially reconstruct it:

- Core radius 90 cm (stated at part 2 line 1453; core diameter 180 cm at line 43).
- Russia's Figure 4.20 axis labels survive as bare OCR'd numbers (part 2 lines
  775-801): radial 90, 95.6, 108.6, 167.793, 190; axial 105, 114.7, 130,
  171.698, 351.818, 388.764, 402, 430 — with the key statement at line 799 that
  **Z = 351.818 cm corresponds to zero core height**, i.e. the top of the conus.
  The pairing of these numbers to specific zone boundaries is **not** recoverable
  and must not be treated as authoritative.
- Control-rod channel 13 cm diameter at r = 102.1 cm; rod lower end 119.2 cm
  withdrawn, 394.2 cm inserted (part 2 lines 109, 111).
- Twenty helium channels, 80 mm diameter at r = 1446 mm, z = 1050-6100 mm
  (line 197).
- VTB's Griffin deck gives a usable outer bound: vacuum applied at
  `sqrt(x^2+y^2) > 165.0`, bottom `z < 41`, top `z > 490`
  (`htr-10-critical.i:74`, `:80-81`, `:88-89`).

**Routes to closing this, for maintainer decision** (section 9, question 3):
read the dimensions off the figure in the PDF; obtain INL's evaluation of the
initial critical configuration (Terry, 2005, `INL/CON-05-00852`, cited by Choo
and Xiao as their geometry source); or obtain the VTB mesh from upstream, which
encodes the geometry directly.

### 7.4 The two RNG defects — prerequisites for rung 1

Both are **open P1 defects in `outram-mc-libs`**. Their actual blast radius
differs, and the difference matters — do not overstate either.

- **`op-rbo`** — `init_seed` derives streams one LCG *step* apart rather than
  `id * DEFAULT_STRIDE` apart, so streams that should be independent are a
  one-draw shift of each other. Upstream is
  `/home/teddy0/Documents/research/openmc/src/random_lcg.cpp:60`; the port at
  `crates/outram-mc-libs/src/rng/lcg.rs:90` drops the multiplication.
  **Scope, from the bead's own 2026-08-06 comment: `init_seed` has zero library
  call sites** (only five, in `examples/triso_stochastic_gpu.rs`), so no k_eff or
  transport statistic was ever produced through the bug. The fix is written and
  passing (238 tests) but **uncommitted, awaiting maintainer review**. What it
  invalidated was a quoted binomial sigma, not a central value.
- **`op-jis`** — `rng::lcg::prn` omits OpenMC's PCG-RXS-M-XS output permutation
  (`lcg.rs:26` versus `random_lcg.cpp:32-44`), returning the raw top 52 bits of
  the LCG state. The *state* recurrence is identical, so `future_seed`,
  jump-ahead and the GPU mirror tests are unaffected — only the state-to-double
  mapping diverges. Measured evidence is in the bead: from seed 1 the two
  generators' first five draws differ entirely while the state after five draws
  is `0xCBA276B4B881A9F0` either way.

**Framing, per the crate's own maintainer decision (`outram-mc-libs/CLAUDE.md`,
2026-08-06): bit-for-bit parity with OpenMC is explicitly NOT required — what
is required is that the statistics are right, and that stream separation
holds.** On that standard `op-jis` is a fidelity and reproducibility issue
(this crate can never reproduce an OpenMC sequence bit-for-bit, and the PCG
permutation exists *for* statistical quality so should be ported), while
`op-rbo` was the one that actually threatened a reported sigma. Fixing `op-jis`
means re-running every recorded statistic in the crate plus the WGSL shaders
and `crates/raffles`, so it needs a planned change, not a drive-by.

**Do not fix either from here.** Report any MC number with these caveats
attached, and do not assume the reference is trustworthy until they close.

### 7.5 Mesh quality — `op-79c`

**`op-79c` (P1, open)** records that the tet **primal** mesh quality, not the
dualisation step, is the real source of non-orthogonality in
`crates/outram-park-fork-cfmesh`. Non-orthogonality degrades finite-volume
accuracy directly, so it propagates into any GeN-Foam result computed on such a
mesh.

Three details from that bead that must not be lost when it is quoted:

1. **The "the dual is exact" result belongs to a different dual.** The 1e-12
   orthogonality proof
   (`crates/outram-foam-mesh/tests/poly_dual_mesh.rs:212`) is for
   `outram-foam-mesh`'s **cell-centre** polyDualMesh. cfmesh uses the
   **median/Donald vertex-centred** dual (`cfmesh/src/dual.rs:48-70`), which
   inherits none of those properties, and `dual.rs:86-88` says outright that
   there is no orthogonality or skewness test for cfmesh's dual. The diagnosis
   is therefore one step short of proven, and closing that gap is the bead's
   first sub-task.
2. **The 85-degree figures are doc-recorded, not test-asserted.**
   `cfmesh/src/pipeline.rs:774-775` records 85.14 and 85.38 degrees maximum
   non-orthogonality for un-dualised runs, but the test at `:812` runs two of
   eight rows and asserts only `< 90.0` (`:842`). Do not cite those figures as
   measured by CI.
3. **There is no non-orthogonality or skewness assertion on the tet primal
   anywhere in the tree** (`cfmesh/src/tet.rs`), which is the second sub-task.

`op-38z` (Delaunay-quality tet refinement) is the related improvement:
smart-Laplacian smoothing (`cfmesh/src/smooth.rs`) and flip-based Delaunay
(`cfmesh/src/delaunay.rs`, 2-to-3 / 3-to-2 bistellar flips with Shewchuk
predicates and an improve-or-noop guard) have landed; exact adaptive predicates
and size-driven Bowyer-Watson insertion have not.

**Deterministic results computed before `op-79c` is resolved carry an
unquantified mesh-quality error.** Say so wherever such a result is reported.
Do not attempt to fix cfmesh from here.

---

## 8. Meshing — an explicit stage upstream of rung 3

GeN-Foam cannot run without a mesh, so this is part of the pipeline, not beside
it.

**Decisions already made by the maintainer:**

1. **CLI tools first**, not a GUI. `crates/outram-foam-cli` ships OpenFOAM-style
   utilities as terminal binaries; `crates/outram-foam-mesh` holds mesh
   generation and conversion (blockMesh, snappyHexMesh, ideasUnvToFoam,
   polyDualMesh).
2. **One tet-dual mesh, used for both neutronics and thermal hydraulics** — not
   two.
3. **GUI later**, via the `outram-blender` bridge (`op-hzs.53`).

**Start from what exists**, verified 2026-08-11:

- **`crates/outram-park-fork-cfmesh/src/pipeline.rs:312`** —
  `surface_to_tet_dual_mesh(points: &[Vec3], tris: &[[usize; 3]], opts: &TetDualOptions)
  -> Result<(VolumeMesh, TetDualReport), String>`. **Implemented**, not stubbed;
  the body is `run_pipeline` at `pipeline.rs:392`. Seven stages — carve, snap,
  tetrahedralize, Delaunay flip, dual, Laplacian smooth, boundary layers — each
  optional stage gated by `acceptable` (`:243`, requires closed *and* zero
  negative-volume cells) with graceful fallback via `keep_if_ok` (`:253`). The
  returned `TetDualReport` (`:217`) already carries
  `max_non_orthogonality_deg`, `max_skewness` and `n_negative_volume_cells`.
  A multipatch variant is at `:379`, with a documented limitation (`:346-353`):
  boundary layers grow on the *whole* boundary including inlet and outlet, not
  per patch.
- **Quality metrics are implemented** in `crates/outram-park-fork-cfmesh/src/checks.rs:78`
  (`check_quality`): non-orthogonality as `acos(d.Sf/|d||Sf|)` (`:95-97`),
  skewness (`:99-106`), aspect ratio (`:130`), with OpenFOAM's default
  thresholds — non-orthogonality below 70 degrees, skewness below 4 — in
  `QualityReport::is_good` (`:70`).
- **CLI tools that generate or convert a mesh today**: `blockMesh`
  (`crates/outram-foam-cli/src/bin/block_mesh.rs`, reads
  `system/blockMeshDict`, writes `constant/polyMesh`), `ideasUnvToFoam`
  (`.../ideas_unv_to_foam.rs`), and `polyDualMesh` (`.../poly_dual_mesh.rs`,
  the **cell-centre** dual). `outram-foam-mesh` itself has **no binaries** — it
  is a pure library (10,719 lines, zero `todo!()`) exposing `block_mesh`
  (`src/block_mesh.rs:490`), `snappy_hex_mesh::generate`
  (`src/snappy_hex_mesh.rs:186`), `poly_dual_mesh` (`src/poly_dual_mesh.rs:779`),
  a high-level `driver::mesh_from_surface` (`src/driver.rs:345`) with
  `GeneratedMesh::write_case` (`:306`), and `mesh_quality::assess_quality`
  (`src/mesh_quality.rs:274`).
- **Stubs, for the record**: `rhoPimpleFoam`, `sonicFoam` and `gen-foam` in
  `outram-foam-cli` all return errors today.

`outram-blender` already carries the frontend half as its `foam-mesh` bridge,
which is why the eventual GUI ambition is realistic rather than speculative.

**Inputs**: the HTR-10 core geometry — 180 cm diameter, 197 cm average height,
100 cm side reflector (including carbon bricks), the conus and discharge tube,
plus the boring pattern (10 control rod at 13 cm on r = 102.1 cm, 7 absorber
ball, 3 irradiation, 20 helium at 8 cm on r = 144.6 cm). Note section 7.3: the
axial build is not fully specified in the open text.

**Outputs**: a polyMesh consumed by GeN-Foam, plus its quality report.

**Quality metrics that must be measured and reported, not hoped for**:
non-orthogonality and skewness at minimum, since those are what `op-79c` is
about. They belong in the run manifest alongside the mesh itself.

### 8.1 Why one mesh for both physics

Using a single tet-dual mesh for neutronics and TH **avoids cross-mesh mapping
error at the coupling seam**. If the two physics live on different meshes, every
exchanged field is interpolated, and that interpolation is an error source that
is easy to introduce and hard to see. `op-p6p.8` (genfoam multiRegion cross-mesh
coupling, in progress) exists precisely for the case where meshes *do* differ —
so the shared-mesh choice is a deliberate simplification that sidesteps it, and
should be recorded as such rather than assumed to be free.

**The honest tension**: the resolution neutronics wants and the resolution TH
wants are not usually the same. Neutronics wants resolution where the flux
gradient is steep — the core/reflector interface, the rod channels — while TH
wants it where the temperature and velocity gradients are steep, which in a
pebble bed is the near-wall porosity rise and the axial thermal gradient. On a
shared mesh the union of both refinement regions must be resolved, which costs
cells the coarser physics does not need. **Flagged as an open question**
(section 9, question 4).

---

## 9. Open questions for the maintainer

1. **Where should the benchmark specification finally live?** It is currently
   `crates/outram-park-digital-twin-engine/src/htr10/neutronics.rs`, co-located
   with the existing HTR-10 data under that module's maintainer-granted
   exception to the crate's no-physics rule. That is right for *today*, when it
   is data only. Once a transport model consumes it, a GUI crate is the wrong
   place. Candidates: `outram-mc-libs` (but it is meant to be reactor-agnostic),
   `nee_soon` (the designated coupling layer, `op-fr2`), or a small dedicated
   `htr10-benchmark` crate. **My recommendation: `nee_soon`**, since it is
   already the intended home for a pipeline above the individual crates.
2. **Which group structure for rung 2?** Recommendation in section 4.2 is the
   VTB ten-group structure, for comparability against the only available HTGR
   reference deck.
3. **How should the R-Z zone geometry be obtained** (section 7.3)? Reading the
   figure, obtaining Terry (2005) `INL/CON-05-00852`, or obtaining the VTB mesh
   from upstream. This is on the critical path for rung 1 step 1d and for all of
   rung 3.
4. **How is the neutronics/TH mesh-resolution tension resolved** (section 8.1)?
   Refine to the union and accept the cell count, or accept cross-mesh mapping
   and use `op-p6p.8`?
5. **`op-h23` should be closed as stale** (section 7.1) and replaced with a
   narrower bead for B-10/B-11/Si. Closing beads is the maintainer's call.
6. **`vtb-findings.md` needs a correction** (section 3.4): the "initial critical
   1.0009032234669475" eigenvalue it reports is not recoverable from the files
   present in this checkout, and the VTB HTR-10 mesh and cross-section library
   are absent, so "a complete, self-contained neutronics benchmark, extractable
   today" overstates what is actually here.

---

## 10. Ordered plan

Strictly ordered; each step is gated on the one above it.

| # | Work | Gated on |
|---|---|---|
| 0 | Close the two RNG defects `op-rbo` and `op-jis` | — |
| 1 | Wire graphite coherent/incoherent-elastic S(alpha,beta) through to `Nuclide`, at all tabulated temperatures | the njoy-side work in flight (`op-hc2o`) |
| 2 | Add a `tsl-*` acquisition path so the graphite thermal tape is fetchable | step 1 |
| 3 | Rung 1 step 1b: infinite pebble-bed k_inf against Tantillo's 1.6321 | steps 0-2 |
| 4 | Cylindrical bounds + two species + packing fraction 0.61 in `sphere_packing` (needs RSA-DEM or ODR-DEM, both currently `NotImplemented`) | step 3 |
| 5 | Obtain the R-Z zone geometry (question 3) | maintainer decision |
| 6 | Rung 1 step 1d: B1 detailed core k_eff, as-measured variant | steps 4, 5 |
| 7 | Meshing: tet-dual polyMesh of the HTR-10 core, with quality report | `op-79c`, step 5 |
| 8 | Rung 2: MGXS tallies and library, in the `op-p6p.4` format | steps 3, 6; beads `op-6tz.15`, `op-6tz.6.3` |
| 9 | Rung 3: GeN-Foam on the tet-dual mesh; k_eff versus rung 1 in pcm | steps 7, 8 |
| 10 | Rung 4: collapse beta_eff, Lambda and both reactivity channels; compare against rung 3 | step 9; `op-p6p.10`, `op-jyyp.6` |
| 11 | Whole chain against B1/B2/B3/B4, and the measured 123.06 cm | step 10 |
| 12 | `outram-blender` third solver bridge | `op-hzs.53`, step 11 |

---

## 11. Rung 1 step 1a — a computed result, and exactly what it is not

`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs` runs the rung-1
transport stack end to end on real HTR-10 material data and produces a k_inf
with a statistical uncertainty. **It is a code-exercise result, not a physics
result, and it is not an HTR-10 criticality result.**

### 11.1 The problem it solves

An infinite medium of the *fuelled zone* of an HTR-10 fuel pebble, run twice:
once with the UO2 kernels resolved explicitly as randomly packed spheres, and
once with exactly the same nuclide inventory homogenised into one medium. Atom
densities come straight from IAEA-TECDOC-1382 Table 4-38 (Open tier, part 2
line 1101) — kernel U-235 3.992067E-03, U-238 1.924449E-02, O 4.647329E-02,
B-10 1.849637E-08, B-11 7.445022E-08; matrix graphite C 8.674169E-02 (which is
exactly 1.73 g/cm^3 of carbon), B-10 2.244010E-08, B-11 9.032424E-08. The
kernel volume fraction of the fuelled zone follows from the specification as
8335 * (0.025/2.5)^3 = 8.335e-3.

### 11.2 Measured results (2026-08-11, this workspace)

| Run | Particles / inactive / active | Heterogeneous k_inf | Homogenised k_inf | delta k (pcm) |
|---|---|---|---|---|
| smoke | 200 / 8 / 25 | 1.60870 +/- 0.02031 | 1.47273 +/- 0.01489 | -13597 +/- 2519 |
| main | 400 / 10 / 40 | **1.57269 +/- 0.00877** | **1.45189 +/- 0.00820** | **-12081 +/- 1201** |

Main run, in full: 1018 explicitly packed kernels of radius 0.025 cm in a
reflective cube of half-width 1 cm, realized packing fraction 0.008328 from
packing seed 20260811; transport RNG seed 1; temperature 293.15 K; delta
(Woodcock) tracking with a bin-maximum bounding majorant. The reactivity
difference is -5291 +/- 526 pcm, a 10.1 sigma separation. The two runs agree
with each other to 1.6 sigma on the heterogeneous case.

**The sign is a physics check and it passes.** Resolving the kernels depresses
the flux inside each lump at the U-238 resonance energies, so fewer resonance
absorptions occur per U-238 atom and k rises; homogenising removes that
depression, so the homogenised case must come out lower. It does.

### 11.3 Everything that is wrong with it — read before quoting it

- **Thermal scattering is FREE GAS.** Graphite bound-atom S(alpha,beta) does
  not reach the transport path (section 7.2). On a graphite-moderated thermal
  system this is a first-order error in the thermal spectrum. **This alone
  disqualifies the absolute number as physics.**
- **It is a fuel-zone infinite medium** — no graphite shell, no dummy ball, no
  bed, no reflector, no leakage. **No published HTR-10 value corresponds to
  this problem**, so the absolute k_inf cannot be compared to the literature at
  all. Only the heterogeneous-versus-homogeneous difference is meaningful, and
  only as a self-comparison.
- **The TRISO coatings are not resolved** — buffer, inner PyC, SiC and outer
  PyC are smeared into the matrix graphite; only the fissile kernel is an
  explicit sphere (`op-6tz.35`).
- **LOW fidelity tier** — embedded windowed-multipole CORE library with the
  10-group fast fallback, a flat nu-bar and a Watt fission-spectrum stand-in
  (worth about +500 pcm on Godiva). Published HTR-10 values are
  continuous-energy ENDF/B-VII.0.
- **`k_std` is a generation-batch standard error** with no inter-generation
  correlation correction, so it under-states the true uncertainty.
- **The two RNG defects are inherited** (section 7.4).
- **The delta-k figure is NOT comparable to Wang et al.'s unit-cell biases.**
  Theirs are multigroup cross-section *processing* biases against a
  continuous-energy reference on the full HTR-10 model, and they run the other
  way (+2820 pcm for INFHOMMEDIUM). This is a continuous-energy *geometric*
  effect on a different problem.

### 11.4 What it does establish

That the rung-1 chain — IAEA atom densities, `Nuclide::from_core` for U-235,
U-238, O-16, C-nat, B-10 and B-11, `Majorant::bounding`, RSA packing, and
`run_keff_delta` — runs end to end on this reactor's real materials, is
reproducible from a stated seed, and gives a physically correctly signed
self-shielding effect. That is a real step, and it is all it is.

**Cost note.** Thermal neutrons in graphite scatter hundreds of times per
history, so this is far more expensive per particle than the fast systems the
crate's other examples use: the 400/10/40 run took roughly 20 minutes. Rung 1
step 1d will need serious compute budgeting.

---

## 12. What this work actually produced

- **This document.**
- `crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs` — the rung 1 step 1a
  calculation described in section 11.
- `crates/outram-park-digital-twin-engine/src/htr10/neutronics.rs` — the
  benchmark specification, the TRISO layer stack, the core geometry available in
  text, the measured first criticality, and 45 cited published values, with
  tests that reproduce published relations rather than assert ranges.
- Corrections to two existing documents, recorded above: `op-h23` is stale
  (section 7.1) and `vtb-findings.md` overstates the VTB HTR-10 case
  (section 3.4).

**Nothing here is validated.** No transport calculation of HTR-10 exists in this
workspace, and per the workspace V&V rule none may be described as validated
until the maintainer has personally reviewed it.

---

## Benchmark-model dimensions from Terry et al. (2005)

**Added 2026-08-13.** These are the geometry values `op-tvmf` was opened to
obtain. They close most — **not all** — of the gap that bead describes.

### Provenance

| Field | Value |
|---|---|
| Source | Terry, W. K.; Kim, S. S.; Montierth, L. M.; Cogliati, J. J.; Ougouag, A. M. |
| Title | *Evaluation of the HTR-10 Reactor as a Benchmark for Physics Code QA* |
| Report | INL/CON-05-00852 **(PREPRINT)** |
| Venue | International Reactor Physics Experiment Program Working Group Meeting |
| Organisation | Idaho National Laboratory |
| Date | November 2005 |
| Obtained from | <https://www.osti.gov/servlets/purl/911178> |
| Date accessed | 2026-08-13 |
| Access tier | **Proprietary** — `crates/kovan-literature/proprietary/reports/terry2005-htr10-benchmark-evaluation.pdf` (gitignored) |
| Table | Table 2, "Individual and total uncertainties" |
| Processing | Text extracted with `kovan lit import`; values transcribed by hand from the extracted table. No digitisation, no figure reading. |

**Why proprietary despite public OSTI hosting.** The preprint's own first page
states it "should not be cited or reproduced without permission of the author".
Per `DATA_POLICY.md`, the tier comes from the document's copyright page, not
from where it was downloaded — public hosting grants no redistribution rights.

**Why the numbers may nonetheless be recorded here.** Facts are not
copyrightable; the restriction attaches to reproducing the document, not to
measurements of a reactor. Terry §1 further states that all descriptive data
"were obtained from published documents, mainly two IAEA TECDOC reports" — both
of which this workspace already holds in the **open** tier. So these values have
an open provenance path.

**Citation caution.** The no-citation-without-permission clause is a
publication-ethics constraint. For any publication, cite the underlying IRPhEP
evaluation report or the IAEA TECDOCs, **not** this preprint, unless permission
has been obtained.

### Values (nominal first, then the bounding value used in the uncertainty study)

These are the **as-defined benchmark model** dimensions, not as-built plant
values. The second column is a perturbation used to compute an uncertainty — it
is *not* an alternative measurement.

| Item | Nominal | Bounding | k_eff uncertainty |
|---|---|---|--:|
| Core radius | 90 cm | +17 pebbles | 1.9e-4 |
| Core height (loading) | 123.06 cm | +17 pebbles | 3.7e-4 |
| **Height of core cavity** | **221.818 cm** | 222.818 cm | 2.4e-4 |
| **Height of conus** | **36.946 cm** | 39.6815 cm | 6.1e-4 |
| Outer diameter of graphite reflector | 380 cm | 382 cm | 1.0e-4 |
| Height of graphite reflector | 610 cm | 616.1 cm | 1e-5 |
| Cold coolant channel diameter | 8.0 cm | 8.5 cm | 1e-5 |
| Cold coolant channel radial location | 144.6 cm | 144.85 cm | 0 |
| Cold coolant channel height | 405 cm | 415 cm | 0 |
| Control-rod / irradiation channel diameter | 13 cm | 12.5 cm | 3.5e-4 |
| Control-rod / irradiation channel height | 450 cm | 452 cm | 0 |
| Control-rod / irradiation channel radial location | 102.1 cm | 102.35 cm | 9e-5 |
| KLAK channel diameter (upper) | 6 cm | 6.2929 cm | 0 |
| KLAK channel area (middle) | 88.2743 cm² | 97.1017 cm² | 2.8e-4 |
| KLAK channel diameter (lower) | 6 cm | 6.2929 cm | 0 |
| Hot gas duct | D = 30 cm, L = 100 cm | D = 31 cm, L = 119.25 cm | 0 |
| Fuel discharge tube radius | 25 cm | 25.25 cm | 0 |
| Fuel discharge tube height | 610 cm | 616.1 cm | 0 |
| Fuel pebble diameter | 6.0 cm | 5.98 cm | 5.0e-4 |
| **Pebble packing fraction** | **0.61** | 0.62 | 1.9e-3 |
| **Upper-surface cone angle from horizontal** | **19.5°** | 17°, 22° | 2.14e-3 |
| Pressure vessel + core barrel thickness | 0 | 10 cm | 1.2e-4 |
| Graphite block gaps | none | 1 cm at reflector outside | 1.6e-4 |
| **Total (RMS)** | | | **6.24e-3** |

Materials from the same table, for completeness: buffer 0.009 cm / 1.1 g·cm⁻³,
IPyC 0.004 cm / 1.9 g·cm⁻³, SiC 0.0035 cm / 3.18 g·cm⁻³, OPyC 0.004 cm /
1.9 g·cm⁻³; uranium loading 5 g/pebble; fuel-pebble matrix 1.73 g·cm⁻³;
reflector matrix 1.76 g·cm⁻³; boron in reflector graphite 4.8366 ppm; boron in
fuel element 1.3 ppm; boron in dummy pebbles 0.125 ppm; boronated carbon bricks
3.46349e-3 atoms·b⁻¹·cm⁻¹; O/U ratio 2.0; air at 0.1013 MPa, 15 °C.

### What this does and does not resolve

**Resolves** the axial build `op-tvmf` flagged as absent from the open text —
core cavity height and conus height in particular, which could not be recovered
from IAEA-TECDOC-1382 part 2 (its FIG. 4.10 survives only as a bare caption).
It also independently confirms **packing fraction 0.61**, which `op-5c5r`
requires for `sphere_packing`, and gives the **19.5° cone angle**.

**Does NOT resolve** the 83-zone R-Z boundaries of Table 4-3. Terry §1 says this
paper is a *summary* and directs the reader to the IRPhEP evaluation report
itself (its ref. [3]) for the detailed benchmark model. Obtaining that report is
the remaining work on `op-tvmf`.

**Caveat on the cone angle:** Terry states 19.5° was *calculated* by an INL
discrete-element code, and explicitly that "the cone angle was not measured in
the experiment". Treat it as a modelling assumption, not data.
