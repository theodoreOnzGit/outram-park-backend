# gh:#261 -- tally filters against OpenMC 0.15.3, bin for bin

Date: 2026-11-18. Predictions (written and committed before each run, amendments dated and
labelled): [`prediction_before_measuring.md`](prediction_before_measuring.md).
Test: `tests/tally_filters_vs_openmc.rs`. New code beside the reviewed transport:
`src/tally/history_events.rs`. Research, education and V&V only (`RESPONSIBLE_USE.md`).

## Provenance

| item | value |
|---|---|
| OpenMC | 0.15.3, commit `27e38e894697bb32a1dac7848d2618818b6b8daf`, built from source, no MPI |
| data | ENDF/B-VIII.0 O-16, NJOY2016 (`IncidentNeutron.from_njoy`, 293.6 K), **one nuclide**, **CE mode** ([`make_o16_lib.py`](openmc_inputs/make_o16_lib.py)); ours: the same tape through `Nuclide::from_endf_file`, tolerance 1e-3 |
| scripts | [`exp_a_beams.py`](openmc_inputs/exp_a_beams.py), [`exp_b_collisions.py`](openmc_inputs/exp_b_collisions.py), verbatim as run |
| OpenMC output | plain text, committed: [`openmc_reference/`](openmc_reference/) (no `.h5`) |
| hardware | Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB RAM, Linux 6.18, CPU only; shared with two other agents; OpenMC with `OMP_NUM_THREADS=2` / `threads=2`, ours single-threaded |
| timings | Exp. A OpenMC: 18 one-particle runs 2.8 s wall (A2: 2.0 s). Exp. B OpenMC: 3.1-3.6 s per 1e6-history run; ours 2.9-7.1 s per 1e6 histories through `trace_csg_history` (load dependent) |

## Which of the 16 filters exist upstream in 0.15.3

`ls src/tallies/`: **no `filter_reaction.cpp` and no `filter_particle_production.cpp`** in 0.15.3.
`ParentNuclideFilter` exists but only matches a decay-chain parent index
(`filter_parent_nuclide.cpp:42`), which only a decay-photon source sets. These three cannot be
compared with this OpenMC.

## What the work found in the crate before any comparison

**`FilterEvent`'s event-history fields were set by nothing outside the filter files.**
`scoring.rs` builds its events with `..Default::default()`, so on a real run a `CellFromFilter`,
`CellBornFilter`, `MaterialFromFilter`, `MuSurfaceFilter`, `MeshBornFilter`, `MeshSurfaceFilter`,
`CollisionFilter` or `WeightFilter` scored nothing (or binned every event at the default). That
is the second time the "reachable" gate (`every_filter_kind_is_reachable`) was satisfied while
the filter could not work. The reviewed `transport_history_vr` is not edited; `history_events`
rebuilds the events from the `TraceEvent` stream it already emits.

## Experiment A -- deterministic beams (exact comparison)

Method: see `tests/tally_filters_vs_openmc.rs` header. 18 sites; 3x1x1 lattice, pitch 6 cm,
tile = sphere r = 2 cm (cell A, M1) in cell B (M2), O-16 at 1e-10 / 2e-10 atoms/b-cm (no
collision is sampled), vacuum box. One OpenMC run per site, compared per site per bin. Pass:
relative 2e-8 (the transport's 1e-9 cm nudge per crossing, amendment 1) for flux, exact for
currents. Geometry pictures (ported plotter, from the assembled geometry):
[`geometry/xy_z0.png`](geometry/xy_z0.png), [`geometry/xz_y05.png`](geometry/xz_y05.png),
[`geometry/xy_z0_mesh.png`](geometry/xy_z0_mesh.png) (registered 6x2x1 mesh: its planes at
x = -9..9 step 3 run through every tile edge and sphere centre),
[`geometry/xy_z0_mesh_a2.png`](geometry/xy_z0_mesh_a2.png) (A2 mesh 5x3x1).

| filter | path | bins | nonzero | worst scaled diff | misses |
|---|---|---|---|---|---|
| DISTRIBCELL | real runner | 54 | 9 | 1.4e-8 | 0 |
| CELL_INSTANCE (A and B pairs) | history driver | 108 | 31 | 1.4e-8 | 0 |
| ENERGY_FUNCTION | real runner | 18 | 18 | 9.5e-10 | 0 |
| CELLBORN | history driver | 36 | 18 | 9.5e-10 | 0 |
| CELLFROM | history driver | 252 | 29 | 0 (exact) | 0 |
| MATERIALFROM | history driver | 252 | 19 | 0 (exact) | 0 |
| MUSURFACE | history driver | 1008 | 27 | 0 (exact) | 0 |
| MESHBORN | history driver | 216 | 18 | 9.5e-10 | 0 |
| **MESH_MATERIAL, as shipped (midpoint rule, real runner)** | real runner | 432 | 52 | 3.2 (cm) | **25 -- predicted** |
| MESH_MATERIAL, with track-length splitting (new, beside) | history driver | 432 | 52 | 1.6e-8 | 0 |
| **MESH_SURFACE, registered 6x2x1 mesh** | history driver | 2592 | 56 | 1 (count) | **16 at 4 sites** |
| MESH_BORN / MESH_MATERIAL(split) / MESH_SURFACE on A2 (5x3x1) | history driver | 270 / 540 / 3240 | 18 / 51 / 60 | 9.5e-10 / 1.4e-8 / 1 | 0 / 0 / **1** |

The worst flux differences (1.4e-8 to 1.6e-8) are the 1e-9 cm nudge per crossing the
transport takes and OpenMC does not, as predicted in amendment 1; they sit inside the
registered 2e-8.

### Misses, reported

* **MESH_MATERIAL as shipped (25 bins).** Predicted. `MeshMaterialFilter::get_bin` bins the
  segment midpoint, so a flight crossing a mesh edge inside one material is credited to one
  element; OpenMC splits it (`filter_meshmaterial.cpp`). The new `regular_mesh_split` splits it:
  0 misses. ~~The shipped path is **not** fixed (the fix belongs in `scoring.rs`, outside "beside
  the port"); `mesh_material_vs_openmc` stays red.~~ **CORRECTED 2026-11-18 (maintainer-confirmed
  edit): `score_length_like` in `scoring.rs` now splits a MESH_MATERIAL track across the regular
  mesh with `regular_mesh_split`; `mesh_material_vs_openmc` passes (0 / 432 bins, worst 1.6e-8).** The same gap exists for the older `MeshFilter`
  on a regular mesh (`FilterKind::splits_track_length` is false for it): not measured here.
* **MESH_SURFACE, 16 bins (registered mesh) and 1 bin (A2).** At sites 1, 4, 16, 17 of the
  registered experiment every flight that crosses a tile edge ends or starts exactly on a mesh
  plane, so whether the crossing is counted depends on the last ulp of `r + d*u` in each code
  (OpenMC's `raytrace_mesh` offsets the start by `TINY_BIT` and returns when
  `traveled_distance >= total_distance`, `mesh.cpp:1028,1061`; ours counts crossings on the open
  interval). This tests tie-breaking, not bin assignment. I wrote amendment 2 *after* seeing it,
  with a mesh that avoids the coincidence; it predicted 0 misses and **was wrong by one**: the
  diagonal beam leaves through the box edge y = z = 3 cm (also a tie; I overlooked that when
  predicting). Both are reported as misses; neither was removed or loosened.
  `RegularMesh::surface_bins_crossed` is a re-derivation, not a port of `raytrace_mesh`; off ties it
  agrees with OpenMC in every bin tested.

### Upstream behaviours the comparison exposed (reproduced in the driver, not "fixed")

* `SurfaceFilter` weights a `current` score by **+-1** (`filter_surface.cpp:55-60`, `-1` when
  `p.surface() < 0`). This crate's `SurfaceFilter` carries no sign; the sign is applied in the
  scorer (`HistoryEventKind::Crossing { sign }`).
* The sign is that of `u.n` **in the frame of the level the surface lives in**
  (`geometry.cpp:426-434`). My first driver used the global frame and was wrong for every tile but
  the central one; the cell-from tally caught it (5 misses before, 0 after).
* `MuSurfaceFilter` takes the normal at the **global** `p.r()` (`filter_musurface.cpp:15`), so for
  a surface inside a lattice tile it is evaluated at the wrong point. Reproduced bit for bit
  (1008 bins, 0 misses), which is evidence the port matches upstream, not that upstream is right.
* `material_from` at a **vacuum leak** is the material of the cell *before* the one the particle
  is in (no cell search happens, `geometry.cpp:176`, `particle.cpp:392`): 27 misses before the
  driver emulated it, 0 after.
* OpenMC's `CellFromFilter` loops over **all coordinate levels** of the last state
  (`filter_cellfrom.cpp:11`); ours holds one cell index. Predicted divergence, **not tested** by
  the registered bins (leaf cells only); a bin naming the lattice-filled cell would differ.

## Experiment B -- statistical, CE O-16 gas

Sphere r = 10 cm, vacuum, O-16 0.03 atoms/b-cm, isotropic 1 MeV point source, analog; 20 batches
x 50 000 histories per code; z = (a-b)/sqrt(sa^2 + sb^2) with batch standard errors.

| filter | bins | first run worst \|z\| | 8-replicate pooled worst \|z\| |
|---|---|---|---|
| COLLISION (track-length flux, n = 0..5) | 6 | **2.87** (n = 4, ours 0.753318 +- 0.002031, OpenMC 0.744105 +- 0.002480) | 1.54 (n = 4: +1.18) |
| LEGENDRE order 3 x scatter (analog, bare P_l) | 4 | 1.49 | 1.19 |

The first run **missed** the registered |z| <= 2 on one bin of ten (chance ~4 %); the registered
replication plan (8 independent seed pairs, 1001..1008) was run and pooled to 1.54, so the miss
reads as noise, but the first-run gate stays red and is reported as such.

## Summary of the 16 filters

| # | filter | implemented | OpenMC comparison |
|---|---|---|---|
| 1 | DISTRIBCELL | yes | exact, 0 misses |
| 2 | CELL_INSTANCE | yes | exact, 0 misses |
| 3 | MESH_SURFACE | yes | **miss** on ties (16 / 1 bins); off ties exact |
| 4 | MESHBORN | yes | exact, 0 misses |
| 5 | MESH_MATERIAL | yes | **miss** as shipped (midpoint); 0 misses with splitting |
| 6 | ENERGY_FUNCTION | yes | exact, 0 misses |
| 7 | LEGENDRE | yes | statistical, \|z\| <= 1.49 (1.19 pooled) |
| 8 | REACTION | yes | **not comparable**: no such filter in OpenMC 0.15.3 |
| 9 | PARENT_NUCLIDE | yes | **not done**: needs a depletion chain and a decay source |
| 10 | PARTICLE_PRODUCTION | yes | **not comparable**: no such filter in OpenMC 0.15.3 |
| 11 | COLLISION | yes | first run \|z\| 2.87 (miss), pooled 1.54 |
| 12 | CELLFROM | yes | exact, 0 misses (leaf cells only) |
| 13 | CELLBORN | yes | exact, 0 misses |
| 14 | MATERIALFROM | yes | exact, 0 misses |
| 15 | MUSURFACE | yes | exact, 0 misses |
| 16 | WEIGHT | yes | ~~**not done**: needs survival biasing through a traced entry point~~ **CORRECTED 2026-11-18**: done through the real runner (`FixedSourceSettings::variance_reduction`), Experiment C below: worst \|z\| 1.06 |

~~**Superseded 2026-11-18, see the last section:** all but CELL_INSTANCE and LEGENDRE are now also
scored through the real transport.~~ Ten of the comparisons (CELL_INSTANCE, CELLFROM, CELLBORN, MATERIALFROM, MUSURFACE, MESHBORN,
MESH_SURFACE, MESH_MATERIAL with splitting, COLLISION, LEGENDRE) are scored through the new history
driver, not the real transport. Wiring those fields into the transport needs an edit to the
reviewed `transport_history_vr` and is left for the maintainer's confirmation (not touched).
DISTRIBCELL, ENERGY_FUNCTION and MESH_MATERIAL-as-shipped go through `run_fixed_source_traced`.

## Driver script, Experiment A (verbatim; Experiment B and the library builder are beside it)

```python
"""gh:#261 Experiment A -- deterministic single-neutron beams, OpenMC 0.15.3.

Provenance: OpenMC 0.15.3, commit 27e38e894697bb32a1dac7848d2618818b6b8daf (built from source,
no MPI); data: ENDF/B-VIII.0 O-16 via NJOY2016 at 293.6 K (make_o16_lib.py), ONE nuclide, CE mode;
1 particle x 1 batch per run, one run per source site (mono-directional, mono-energetic, so the
track is fully determined; both materials are 1e-10 / 2e-10 atoms/b-cm O-16, i.e. transparent).

Usage: exp_a_beams.py <openmc_exec> <cross_sections.xml> <out_dir> [nx,ny,nz prefix]   (A1: no extras; A2: "5,3,1" expA2)
Writes <out_dir>/sites.txt and <out_dir>/<prefix>_<tally>.txt  (columns: site bin value).
Geometry: 3x1x1 lattice, pitch 6 cm (x in [-9,9], y,z in [-3,3], vacuum on every face), tile =
sphere r=2 cm (cell A, material M1) inside cell B (material M2).
"""
import sys, os, shutil, tempfile, time
import numpy as np
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
# optional: mesh dimension "nx,ny,nz" and output prefix (A2: "5,3,1" "expA2")
DIM = tuple(int(v) for v in (sys.argv[4] if len(sys.argv) > 4 else "6,2,1").split(","))
PREFIX = sys.argv[5] if len(sys.argv) > 5 else "expA"
ONLY_MESH = len(sys.argv) > 4
os.makedirs(OUT, exist_ok=True)

# ---- sites (fixed seed; two hand-picked beams appended) ----------------------------------------
rs = np.random.RandomState(261)
sites = []
for _ in range(16):
    r = rs.uniform([-8.9, -2.9, -2.9], [8.9, 2.9, 2.9])
    mu = rs.uniform(-1, 1); ph = rs.uniform(0, 2 * np.pi); s = np.sqrt(1 - mu * mu)
    u = np.array([s * np.cos(ph), s * np.sin(ph), mu])
    e = 10 ** rs.uniform(4.0, 7.0)
    sites.append(list(r) + list(u) + [e])
sites.append([-8.5, 0.5, 0.25, 1.0, 0.0, 0.0, 1.0e6])
d = np.ones(3) / np.sqrt(3.0)
sites.append([-8.5, -2.5, -2.5, d[0], d[1], d[2], 3.0e5])
with open(os.path.join(OUT, "sites.txt"), "w") as f:
    for s in sites:
        f.write(" ".join("%.17g" % v for v in s) + "\n")

# ---- model --------------------------------------------------------------------------------------
openmc.reset_auto_ids()
m1 = openmc.Material(material_id=1); m1.add_nuclide("O16", 1.0); m1.set_density("atom/b-cm", 1.0e-10)
m2 = openmc.Material(material_id=2); m2.add_nuclide("O16", 1.0); m2.set_density("atom/b-cm", 2.0e-10)
mats = openmc.Materials([m1, m2]); mats.cross_sections = XS

px = [openmc.XPlane(-9.0, surface_id=1, boundary_type="vacuum"), openmc.XPlane(9.0, surface_id=2, boundary_type="vacuum")]
py = [openmc.YPlane(-3.0, surface_id=3, boundary_type="vacuum"), openmc.YPlane(3.0, surface_id=4, boundary_type="vacuum")]
pz = [openmc.ZPlane(-3.0, surface_id=5, boundary_type="vacuum"), openmc.ZPlane(3.0, surface_id=6, boundary_type="vacuum")]
sph = openmc.Sphere(r=2.0, surface_id=10)
cA = openmc.Cell(cell_id=1, fill=m1, region=-sph)
cB = openmc.Cell(cell_id=2, fill=m2, region=+sph)
tile = openmc.Universe(universe_id=1, cells=[cA, cB])
lat = openmc.RectLattice(lattice_id=5)
lat.lower_left = (-9.0, -3.0, -3.0); lat.pitch = (6.0, 6.0, 6.0)
lat.universes = [[[tile, tile, tile]]]  # [z][y][x]
root = openmc.Cell(cell_id=100, fill=lat, region=+px[0] & -px[1] & +py[0] & -py[1] & +pz[0] & -pz[1])
geom = openmc.Geometry([root]); geom.root_universe = openmc.Universe(universe_id=0, cells=[root])

mesh = openmc.RegularMesh(mesh_id=1)
mesh.lower_left = (-9.0, -3.0, -3.0); mesh.upper_right = (9.0, 3.0, 3.0); mesh.dimension = DIM

surf_all = [sph, px[0], px[1], py[0], py[1], pz[0], pz[1]]
mm_pairs = [(e, mid) for e in range(DIM[0] * DIM[1] * DIM[2]) for mid in (1, 2)]
ef_E = [1.0e3, 1.0e5, 1.0e6, 1.0e7, 2.0e7]; ef_y = [0.5, 2.0, 3.0, 1.0, 0.2]
mu_edges = list(np.linspace(-1.0, 1.0, 9))

def T(tid, name, filters, score):
    t = openmc.Tally(tally_id=tid, name=name); t.filters = filters; t.scores = [score]; return t
ALL = [
    T(1, "distribcell_A", [openmc.DistribcellFilter(cA)], "flux"),
    T(2, "cell_instance", [openmc.CellInstanceFilter([(cA, 0), (cA, 1), (cA, 2), (cB, 0), (cB, 1), (cB, 2)])], "flux"),
    T(3, "energy_function", [openmc.EnergyFunctionFilter(ef_E, ef_y)], "flux"),
    T(4, "mesh_material", [openmc.MeshMaterialFilter(mesh, mm_pairs)], "flux"),
    T(5, "cell_born", [openmc.CellBornFilter([cA, cB])], "flux"),
    T(6, "cell_from", [openmc.SurfaceFilter(surf_all), openmc.CellFromFilter([cA, cB])], "current"),
    T(7, "material_from", [openmc.SurfaceFilter(surf_all), openmc.MaterialFromFilter([m1, m2])], "current"),
    T(8, "mu_surface", [openmc.SurfaceFilter(surf_all), openmc.MuSurfaceFilter(mu_edges)], "current"),
    T(9, "mesh_surface", [openmc.MeshSurfaceFilter(mesh)], "current"),
    T(10, "mesh_born", [openmc.MeshBornFilter(mesh)], "flux"),
]
# A2 (post-hoc, non-degenerate mesh) runs only the three mesh-based tallies
tallies = openmc.Tallies([t for t in ALL if (not ONLY_MESH or t.name in ("mesh_material", "mesh_surface", "mesh_born"))])

results = {t.name: [] for t in tallies}
t0 = time.time()
for i, s in enumerate(sites):
    wd = tempfile.mkdtemp(prefix="expA_")
    st = openmc.Settings(); st.run_mode = "fixed source"; st.particles = 1; st.batches = 1
    st.seed = 1 + i
    st.source = openmc.IndependentSource(
        space=openmc.stats.Point(tuple(s[:3])),
        angle=openmc.stats.Monodirectional(tuple(s[3:6])),
        energy=openmc.stats.Discrete([s[6]], [1.0]), particle="neutron")
    mats.export_to_xml(wd + "/materials.xml"); geom.export_to_xml(wd + "/geometry.xml")
    st.export_to_xml(wd + "/settings.xml"); tallies.export_to_xml(wd + "/tallies.xml")
    openmc.run(cwd=wd, openmc_exec=OPENMC, output=False, threads=1)
    sp = openmc.StatePoint(wd + "/statepoint.1.h5")
    for t in tallies:
        results[t.name].append(sp.get_tally(id=t.id).sum.flatten().copy())
    sp.close(); shutil.rmtree(wd)
wall = time.time() - t0
for name, rows in results.items():
    with open(os.path.join(OUT, "%s_%s.txt" % (PREFIX, name)), "w") as f:
        for i, row in enumerate(rows):
            for b, v in enumerate(row):
                f.write("%d %d %.17g\n" % (i, b, v))
print("sites", len(sites), "runs", len(sites), "wall_s %.1f" % wall)
```

## Real-transport wiring pass (gh:#261, 2026-11-18, maintainer-confirmed) -- PREDICTIONS

Written and committed before any run of the wired code. Full text: amendment 3 of
[`prediction_before_measuring.md`](prediction_before_measuring.md). Summary: bit-identical k and
existing pins (no random number drawn; no re-pin); real-transport Experiment A: CELLBORN, MESHBORN,
CELLFROM, MATERIALFROM, MUSURFACE 0 misses (the three currents exact); MESH_MATERIAL 0 misses on
both meshes after the split fix; MESH_SURFACE the same ties as the driver (16 on the registered
mesh, 1 on A2); Experiment B COLLISION |z| <= 2 on a fresh stream; WEIGHT (new Experiment C,
survival biasing in both codes) |z| <= 2. (B-10 became Li-6 before Experiment C was run: NJOY2016
ACER refuses ENDF/B-VIII.0 B-10, amendment 4.)

### Results (2026-11-18)

**What was edited.** `transport_history_vr_urr` (`src/physics/transport_csg.rs`; the body behind the
kovan-reviewed `transport_history_vr`, fn id `fn:cef42e1a8f2fd45a`, which is a forwarding wrapper
left **unedited**; `_urr` is `fn:bf052a3604a90ca7` and carries no review stamp): a per-particle
`EventHistory` and four calls. `tally/scoring.rs`: `score_length_like` takes the history and sets
`weight`, `cell_born`, `position_born`, `n_collision` on the event; new `score_track_length_hist`;
MESH_MATERIAL on a regular mesh splits the track. `tally/history_events.rs`: `EventHistory`,
`score_crossing`, `score_mesh_surface_segment`, `flight_track_ends`. `tally/review.md` holds no
review stamp, so no other reviewed function was touched.

**k and pins: bit-identical, as predicted.** `stats_move_fingerprints` (9 tests, incl. the nested
lattice k and the FHR delta-tracking pebble), `distribcell_end_to_end`, `particle_clock`,
`every_filter_kind_is_reachable`, `filter_extra_vs_upstream` all pass unchanged; no re-pin.
`history_tallies_do_not_perturb_the_run`: a CellBorn-binned and an unbinned flux tally from one
seed agree bit for bit.

**Experiment A through the real runner** (18 sites, `run_fixed_source_traced`):

| filter | bins | misses | worst scaled diff | prediction |
|---|---|---|---|---|
| CELLBORN | 36 | 0 | 9.5e-10 | held |
| MESHBORN (registered mesh / A2) | 216 / 270 | 0 / 0 | 9.5e-10 | held |
| MESH_MATERIAL, shipped path now (registered / A2) | 432 / 540 | **0 / 0** (was 25 / 0 reported) | 1.6e-8 / 1.4e-8 | held |
| CELLFROM | 252 | 0 | 0 (exact) | held |
| MATERIALFROM | 252 | 0 | 0 (exact) | held |
| MUSURFACE | 1008 | 0 | 0 (exact) | held |
| MESH_SURFACE, registered mesh | 2592 | **16** (same 4 sites, same bins as the driver) | 1 | held (ties) |
| MESH_SURFACE, A2 mesh | 3240 | **1** (site 17, bin 142) | 1 | held (tie) |

CELL_INSTANCE is not reachable through the runner for both cells at once (one distribcell table per
run); DISTRIBCELL, ENERGY_FUNCTION went through the runner already.

**Experiment B through the real runner** (20 x 50 000, seed 261): COLLISION worst \|z\| **2.56**
(bins 2 and 4, +2.56 and +2.55): **a miss of the registered criterion, prediction \|z\| <= 2
not met**, in the direction of the driver's first run. Replication plan fixed beforehand
(amendment 5): 8 seed pairs pooled, worst \|z\| **1.39** (bin 4: ours 0.749141 +- 0.000783,
OpenMC 0.747317 +- 0.001057). Read as noise (OpenMC's own first-run bin 4, 0.744105, is 1.3 sigma
below its pooled value); the first-run gate `real_b_collision_vs_openmc` stays red.

**Experiment C, WEIGHT under survival biasing** (Li-6 0.05 atoms/b-cm, r = 10 cm, 300 keV isotropic
point source, cutoff 0.25 / survive 1.0 both codes, 20 x 50 000, `make_li6_lib.py`,
[`exp_c_weight.py`](openmc_inputs/exp_c_weight.py)): bins 0-1 empty in both codes (roulette),
bin 2 ours 1.968699 +- 0.001685 vs OpenMC 1.965959 +- 0.001969 (z +1.06), bin 3 1.529552 vs 1.529379
(+0.07), bin 4 5.663063 vs 5.662350 (+0.13). Worst \|z\| **1.06**, prediction held. Hardware as in
the provenance table (4-core Xeon 2.10 GHz, shared); ours 4.4 s per 1e6 histories (O-16 run).

**The red gates now.** Of the original four: `mesh_material_vs_openmc` is **green**.
`mesh_surface_vs_openmc` and `a2_mesh_surface_vs_openmc` (driver oracle) stay red on the tie
misses; their real-transport twins pass only because they pin the counts 16 and 1, not zero.
`b_collision_and_legendre_vs_openmc` (driver, first run, 2.87) stays red, and its real-transport
twin `real_b_collision_vs_openmc` (2.56) is red too. Four red gates remain, one of them new.

**Not wired (stated):** the collision-estimator (`score_collision`), scatter-matrix
(`score_scatter_matrix`), fission-birth and delta-tracking scoring paths still build their events
with the defaults, so a history-field filter there scores at the default value; LEGENDRE x scatter
has no real-transport route and stays on the driver; plain MESH on a regular mesh still bins the
segment midpoint (same gap as MESH_MATERIAL had, unmeasured); PARENT_NUCLIDE, REACTION and
PARTICLE_PRODUCTION as before; `CellFromFilter` still holds one (leaf) cell where upstream loops
over all levels.
