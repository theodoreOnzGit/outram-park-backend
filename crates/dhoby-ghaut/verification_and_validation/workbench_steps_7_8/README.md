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
| pebble bed, fast Σ_t | 0.3604 (± 0.40 %) | 0.3611 (± 0.40 %) |
| core cavity, fast Σ_t | 4.448e-2 (± 1.2 %) | 4.514e-2 (± 1.2 %) |
| carbon bricks, thermal Σ_a | 3.045e-2 (± 4.1 %) | 3.126e-2 (± 3.5 %) |

The whole table is `mgxs.csv`.

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
