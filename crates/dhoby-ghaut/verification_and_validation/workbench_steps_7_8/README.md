# Workbench Steps 7 and 8 on the HTR-10 preset (TENTATIVE)

**Research, education and V&V only.** These are verification records of the
Dhoby Ghaut workbench's meshing (Step 7, gh:#572) and multigroup cross
sections (Step 8, gh:#573) on the TENTATIVE `nee_soon::htr10_rmc` HTR-10
preset. Nothing here is validated. Never use it for an operational purpose.

Taken 2026-10-05 on the worktree branch of the Step 7/8 agent, 16-thread
desktop, release build.

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-mgxs \
    --out <case> --temperatures 300.15,600 --groups 2 \
    --particles 2000 --inactive 10 --active 20 --threads 10
```

## Files

| file | what |
|---|---|
| `images/regions_rz.png` | the R-Z region map, drawn from `RegionMap::region_at` (what the cells are assigned from) |
| `images/mesh_<mesh>_xz.png`, `_xy.png` | each mesh drawn from the mesh itself (cells cut by the plane, faces in black, coloured by region): X-Z through the axis, X-Y at the bed mid-height |
| `images/gui_step7.png`, `gui_step8.png` | the workbench at Steps 7 and 8 (Xvfb, `--open-step 7 --auto-build`) |
| `meshes.csv` | per mesh: cells, volume against exact, quality, patches; per region: cells and volume against exact; per map: overlap and coverage |
| `mgxs.csv` | per state, region and group: Σ_t, Σ_a, νΣ_f, χ, Σ_s g→g, D, flux, with relative σ |
| `nuclearData` | the GeN-Foam `constant/neutroRegion/nuclearData` Step 8 wrote |
| `run.log` | timings and modelling notes |

The `polyMesh` folders are not committed (about 60 MB); `--headless-mesh`
writes them in 15 s.

## Step 7: meshes and maps

**Methodology.** The R-Z domain is the assembled preset's (bed, conus,
cavity from `AssembledCore`; reflector zones from TECDOC-1382 Fig. 4.10 via
`nee_soon::htr10_rmc::reflector_geometry::FIG_4_10_BOXES`). Each mesh is a
32-facet body of revolution of its R-Z profile meshed by the cfMesh port:
neutronics, the whole model, tet-dual at 30 cm; thermal-hydraulics, the core
cavity (bed + gas), tet-dual at 15 cm with 3 prism layers (1 cm, ratio 1.3)
on the cavity's fluid wall only; structural, the graphite around the cavity,
conus and tube, tetrahedra at 25 cm. Cells take the region of their centroid.
Maps: `outram_blender::unstructured::overlap`, the port of OpenFOAM's
`tetOverlapVolume` + `cellVolumeWeightMethod` that GeN-Foam's `meshHandler`
calls, with weights normalised as upstream.

**Pass criteria (test `the_headless_meshes_hold_what_the_hand_off_promises`,
at 40/25/40 cm).** Volumes within 3 % of exact; structural all `Tet4` and
convertible to farrer-park; TH patches `inlet`, `outlet`, `cavity_wall`,
`bed_wall`; every polyMesh and cellZones file reads back through the GeN-Foam
port's reader with every cell in one zone; N → TH and N → S cover ≥ 99 %; a
constant maps to itself; the power mapped N → TH equals what the neutronics
cells deposit in the TH domain to 1e-4.

**Results (default sizes).**

| mesh | cells | volume vs exact | non-star cells | max non-orth |
|---|---|---|---|---|
| neutronics | 14 041 polyhedra | −1.24 % | 354 | 73.5° |
| thermal-hydraulics | 20 952 (9 384 polyhedra + 11 568 layer hexes) | −1.29 % | 224 | 86.5° |
| structural | 94 392 Tet4 | −0.91 % | 0 | 88.0° |

The inscribed 32-gon alone accounts for −0.64 %; the rest is the carve and
snap. Max non-orthogonality is above OpenFOAM's 70° warning on every mesh
(the TH one at its prism layers; the FEM one does not use it).

| map | overlap [cm³] | target covered |
|---|---|---|
| N → TH | 5.577e6 | 99.9999 % |
| N → S | 6.192e7 | 99.70 % |
| S → N | 6.192e7 | 90.63 % (the rest is the core cavity, not structure) |
| TH → S, S → TH | 8.1e4 | 0.13 % / 1.46 %: the domains only touch at the wall |

**What the images show, and what they do not.** Checked by eye: the three
meshes are where the R-Z map puts them; the cavity, conus and tube are holes
in the structural mesh; the TH layers sit on the cavity wall only; the
region colours agree between meshes. Visible defects: the snapped structural
mesh rounds the cavity corners and narrows the 25 cm discharge tube (about two
cells across) on one side; region boundaries are stair-stepped at the cell
size, worst for thin bands: the 10 cm cold-gas plenum band is +90 % on the
neutronics mesh and the 8 cm coolant band is a row of whole cells. **What the
images cannot show:** the explicit borings (not in the meshes at all, gh:#594)
and anything off the two planes drawn.

## Step 8: MGXS at two state points

**Methodology.** At each temperature the nuclear data are processed with every
material at T (`Htr10DataConfig::default`, correct-physics default, bound
thermal laws on). Two k-eigenvalue passes at seed 1, 2000 × (10 + 20)
generations, tally on the neutronics mesh (unstructured mesh filter, track
lengths split across cells): `[Mesh, Energy]` with Flux, Total, Absorption,
NuFission, KappaFission, and `[Mesh, Energy, EnergyOut]` with ScatterN and
NuFission. `nee_soon::mgxs::condense` per cell, `homogenised_subset` per
region (flux-weighted), `rebalanced`, descending groups,
`nee_soon::genfoam_xs::to_nuclear_data_input`, states stacked with
`xsVariables { TFuel log; }`, written, then read back with
`read_nuclear_data` and built into `CrossSectionData`. Both passes of each
state gave the same k to 1e-12 (identical histories, so the scalar pass's
flux is the matrix pass's denominator).

**Results, 2 groups (0.625 eV), TENTATIVE.**

| T [K] | k (this short run) | data [s] | transport, both passes [s] |
|---|---|---|---|
| 300.15 | 0.99696 ± 0.00723 | 114.8 | 333.7 |
| 600 | 0.94228 ± 0.00635 | 120.1 | 316.5 |

| region, group | 300.15 K | 600 K |
|---|---|---|
| pebble bed, thermal νΣ_f [1/cm] | 8.830e-3 (± 0.81 %) | 6.725e-3 (± 0.82 %) |
| pebble bed, thermal Σ_a | 4.681e-3 (± 0.77 %) | 3.585e-3 (± 0.77 %) |
| pebble bed, fast Σ_t | ~~0.3604 (± 0.40 %)~~ | ~~0.3611 (± 0.40 %)~~ |
| core cavity, fast Σ_t | 4.448e-2 (± 1.2 %) | 4.514e-2 (± 1.2 %) |
| carbon bricks, thermal Σ_a | 3.045e-2 (± 4.1 %) | 3.126e-2 (± 3.5 %) |

The whole table is `mgxs.csv`.

**SUPERSEDED for the pebble bed (2026-10-06, gh:#598).** Every bed number
in this table, and the bed zones of `mgxs.csv` and `nuclearData` here, carry
the defect described in the gh:#598 section below: the bed flux misses its
helium, so the bed's Σ are about 1/0.65 too large (the νΣ_f and Σ_a rows
too). This 2-state, 12-layer run was **not re-run**; the corrected numbers
are those of the 19-layer, 4-state case in the gh:#598 section. Regions
outside the bed move by a few per cent at most (their stair-stepped cells
hold some bed).

**Interpretation, and what the numbers do not say.**

- k falls by 0.0547 ± 0.0096 from 300 to 600 K (isothermal: every material
  heated). No comparison is made: the k values come from 10 active
  generations each and are not the preset's V&V number (see the Step 5
  record).
- **The "core cavity" region is not helium.** Its Σ_t (0.045/cm) is over a thousand
  times helium's at 1 atm (about 2e-5/cm), because on the 30 cm neutronics mesh the cells whose centroid is
  in the cavity also hold bed and top-reflector graphite. The cross sections
  are right for those cells, and those are the cells the diffusion solver
  will give them to, so the homogenisation is self-consistent; but the region
  is not the cavity. A finer neutronics mesh, or body-fitted regions
  (gh:#594), is needed before a cavity streaming effect means anything.
  The same smearing gives a small νΣ_f to the side reflector and conus
  graphite.
- σ is a first-order estimate (cells of a region independent; ratio
  variances added, their positive correlation ignored), gh:#595.
- No delayed-neutron data (`precGroups 0`), so the file supports a steady
  eigenvalue, not a transient (gh:#595).

## gh:#598: the delta-tracked bed's flux (2026-10-06)

**Symptom** (found by the Step 10 V&V, gh:#591). The bed's Σ_t came out
about 0.40 /cm, the same as solid graphite, where a 61 %-packed bed should
carry about 0.25 /cm; diffusion `k` on these constants was +24 700 to
+27 000 pcm above Step 8's own Monte Carlo `k`.

**Upstream first.** OpenMC has no delta tracking. Its MGXS module scores
flux with the track-length estimator by default (`openmc/mgxs/mgxs.py`,
`estimator = 'tracklength'`); its collision estimator scores
`w/Σ_t` at real collisions only (`src/tallies/tally_scoring.cpp`,
`SCORE_FLUX`, `flux * p.wgt_last() / p.macro_xs().total`). A delta-tracked
flight has no track length, so the port's choice had no upstream
counterpart. Serpent, which does delta-track, scores its collision flux at
every tentative collision as `w/Σ_maj` (J. Leppänen, Ann. Nucl. Energy 37
(2010) 715-722).

**Root cause: the binning, not the estimator, not the denominator, not the
volumes.**

- In a delta-tracked region `outram-mc-libs` scored the real-collision
  estimator `w/Σ_t` by passing `1/Σ_t` to `score_track_length` as if it
  were a length.
- Since gh:#492 an unstructured mesh filter splits a track-length segment
  across the cells it crosses. It rebuilt a "segment" of length `1/Σ_t`
  centred on the collision and split it.
- For a graphite collision (`1/Σ_t ≈ 2.5 cm`) that only smears the score
  over neighbouring cells. For a helium collision (`1/Σ_t ≈ 5e4 cm` at
  1 atm, against a model about 4 m across) almost all of the score falls
  outside the mesh and is dropped.
- The real-collision estimator itself is unbiased in helium (rare
  collisions, each worth `1/Σ_He`), the denominator `Σ_t` was the true
  local one, and the region volumes do not enter `Σ = RR/φ`. What was lost
  was the helium's share of the flux, about 39 % of the bed volume.
- Reaction rates were unaffected (helium reacts with almost nothing), so
  the Monte Carlo `k` was right and only the flux denominator was low.

**Fix (outram-mc-libs, default ON).**

- `tally::scoring::score_collision_point`: a collision-estimator score is
  binned at its site by every filter, never split.
- `pebble_beds::delta_tracking::bounded_delta_flight_visiting`: visits every
  tentative collision site of a delta flight.
- `physics::transport_csg::DeltaTallyEstimator`, in
  `KeffSettings::delta_tally_estimator`: the default `TentativeCollision`
  scores `w/Σ_maj` (flux) and `w·Σ_x(r)/Σ_maj` (rates, in the material at
  the site) at every tentative site up to the nearest surface. Its sites
  have density `φ·Σ_maj` everywhere, helium included. `RealCollision`
  (`w/Σ_t` at real collisions, now point-binned) is the explicit ablation.
  Neither draws random numbers, so `k` is bit-identical between them.

**A defect of the fix, found by its own test and fixed before recording.**
The hybrid driver advances a delta-tracked particle at most to the nearest
surface (a pebble or TRISO surface) and re-samples from there; the flight's
tentative sites beyond that surface were first being scored and then
sampled again. On the BCC test below this read −1.9 % in both groups
(15 σ) against surface tracking. Scoring only the sites before the nearest
surface closed it.

**Verification** (`crates/outram-mc-libs/tests/delta_collision_estimator.rs`,
2026-10-06, release build).

| test | methodology | pass | result |
|---|---|---|---|
| `score_collision_point_bins_at_the_point` | two unit hexes, a `1/Σ = 5e4` score at x = 0.5 | all in cell 0, exactly | exact; the old route gives 1 + 1 (the 5e4 dropped) |
| `tentative_collision_estimator_recovers_the_volume_weighted_sigma` | analytic: pure scatterer at 1 MeV (H-1, embedded data) in an infinite BCC lattice at packing **0.61**, uniform isotropic starts, 20 000 histories cut at 40 cm, so the flux is flat and region Σ_t = f·Σ_p + (1 − f)·Σ_g | within 4 σ and 0.5 % | helium-like gas (Σ_g/Σ_p = 5e-5): 0.233231 ± 0.000393 vs 0.233190 (+0.017 %, 0.10 σ); true void: 0.233260 ± 0.000394 vs 0.233183 (+0.033 %, 0.19 σ). The real-collision estimator: +19 % ± 13 % on helium (rare huge scores), exactly Σ_p (+63.9 %) on a void |
| `delta_tally_matches_surface_track_length_on_a_bcc_cell` | the full hybrid driver on a reflective BCC cell (a = 6 cm, f = 0.61, graphite + 2e-5 U-235 spheres, helium at 2.4452e-5 /(b cm)), one-hex unstructured mesh × 2 groups; reference: surface tracking of the identical geometry (track length, the OpenMC MGXS estimator) | Σ_t within 4 σ per group; `k` bit-identical between estimators | at 3000 × (20 + 40): fast 0.263419 vs 0.263530 (−0.04 %, 0.09 σ), thermal 0.226186 vs 0.226054 (+0.06 %, 0.48 σ); real-collision ablation +2.2 % ± 2.9 % and −0.7 % ± 2.3 %. The committed test runs 2000 × (20 + 30) (169 s): fast 0.263307 vs 0.263476 (−0.06 %, 0.09 σ), thermal 0.226347 vs 0.226307 (+0.02 %, 0.12 σ); `k` 1.958613 in both delta arms, surface 1.956899 ± 0.003915; real-collision ablation −8.8 % ± 4.9 % and +2.1 % ± 2.6 % |

**Predictions, stated before the HTR-10 re-run.** Bed Σ falls by about
0.61; Step 8's `k` unchanged bit for bit; diffusion `k` near the gh:#591
diagnostic arm (+1600 to +3400 pcm).

**HTR-10, 19 layers, 4 states, same command, seed and histories as the
gh:#591 record** (`../htr10_multiphysics_step10/step8_full_core_case/`,
re-run 2026-10-06 with 6 threads; the pre-fix set is in
`../htr10_multiphysics_step10/superseded_pre_598/`).

| quantity, 300.15 K | before (pre-fix) | after |
|---|---|---|
| MC `k` | 1.14617 ± 0.00678 | 1.14617 ± 0.00678 (bit-identical; all four states) |
| bed Σ_t, fast / thermal [1/cm] | ~~0.3543 / 0.3954~~ | **0.2299 / 0.2588** (× 0.649 / 0.654) |
| bed Σ_a thermal [1/cm] | ~~4.518e-3~~ | 2.933e-3 |
| bed νΣ_f thermal [1/cm] | ~~8.526e-3~~ | 5.531e-3 |
| bed D fast / thermal [cm] | ~~0.941 / 0.843~~ | 1.450 / 1.288 |
| side reflector Σ_t fast / thermal | ~~0.3404 / 0.3973~~ | 0.3162 / 0.3910 (its stair-stepped cells hold bed) |
| diffusion `k` − MC `k`, 4 states [pcm] | ~~+24 726, +25 190, +24 906, +27 009~~ | **+4020, +4283, +3652, +5828** (5.8–8.2 σ) |
| coupled 10 MW: `k_eff`, peak/mean, max W/cm³, peak kernel | ~~1.31739, 1.743, 3.57, 1169.7 °C~~ | **1.10560, 1.505, 3.08, 1079.0 °C** |

The factor came out 0.65, not 0.61, and the diffusion gap +3650 to +5830
pcm, not +1600 to +3400: the prediction was the diagnostic's 0.61, and the
measured flux ratio is what it is. Transport time per state (both passes,
6 threads, machine shared with a test run): 409–498 s, against 348–398 s
on the pre-fix run (thread count of that run not recorded), so the cost of
the estimator is not separated from the change of host load here. Hardware:
16 logical cores, 62 GB, Linux, CPU only (CPU model not recorded).

**Not done.** The 2-state run above was not re-run; the mesh and group
studies of the Step 10 record were not re-run on the corrected data; the
remaining diffusion-MC gap is not attributed; the tentative estimator's
`Cell` filter still bins by the cell the flight started in (material and
spatial filters bin at the site); a region whose majorant is zero at some
energy scores no flux there.

