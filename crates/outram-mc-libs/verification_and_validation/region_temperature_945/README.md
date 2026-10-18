# One fixed temperature per region, transported correctly (GitHub #945, stage 1)

**Issue:** [#945](https://github.com/theodoreOnzGit/outram-park-backend/issues/945),
stage 1 (epic #925). Stages 2 (stochastic interpolation, #947) and 3
(temperature varying along a flight, the Chebyshev majorant cover) are not
part of this record.

**Status:** untrusted AI-generated draft (2026-10-18) per
`RESPONSIBLE_USE.md`; research, education and V&V only. Predictions were
posted on #945 before any run
([comment](https://github.com/theodoreOnzGit/outram-park-backend/issues/945#issuecomment-6246522409)).

## 1. What was built, and what was reused

**Reused, unchanged:**

- **The CSG transport is already per-material temperature.** Every lookup,
  the collision partition and the elastic kinematics kT read the
  collision material's own temperature (`physics/transport_csg.rs`, the
  `mat_temp` block; `Nuclide::free_gas_kt`, gh:#313). For pointwise data
  the temperature lives in the processed nuclide itself
  (`Material::temperature` doc). So stage 1 is entirely a question of
  **which processed nuclide each region's material points at**; the kernel
  needs no change.
- **The lowering** (`model/lower.rs`, `lower_packed.rs`) already threaded
  one temperature through every shape and keyed nuclides by `NuclideKey`.
- **Data processing:** `NuclearData::load_nuclide` (RECONR + BROADR, URR
  probability tables, DBRC, S(α,β) by LEAPR from the embedded deck), at the
  key's temperature.
- **Majorants:** `AssembledModel::majorants` (`Majorant::bounding` over the
  region's materials) and `Majorant::audit`.
- **The thermal profile** (#949): `ThermalProfile` voxels and their
  solution; `Geometry::locate` to map a voxel centre to a cell.

**Upstream (OpenMC `d7d3284a1`, read first):** temperature is a cell
property (`src/cell.cpp:90-102`, `Cell::temperature(instance)`), set on a
particle when it enters a cell (`src/particle.cpp:695`,
`sqrtkT() = cell->sqrtkT(cell_instance())`). A fill cell's temperature is
pushed down to every material cell it contains (`openmc/cell.py`,
`Cell.temperature` setter). `get_temperatures` (`src/geometry_aux.cpp:221`)
collects every distinct cell temperature per nuclide and per S(α,β) table,
and each is loaded. Stage 1 follows that model, with two differences:

- the data are processed **exactly** at each temperature, so there is no
  `temperature_method` (nearest / interpolation) choice to make; and
- an inner shape's own temperature always wins. OpenMC's Python setter
  overwrites the inner cells at the moment it is called, so the order of
  the two assignments decides; here it does not.

**New:**

- `Shape::at_temperature(Temperature::Uniform(T))`, also on `DhRegion`.
- `Model::set_component_temperature(component, T)` and
  `Model::take_temperatures_from(&ThermalProfile)` (per component, the
  volume-mean of the voxels whose centres lie in that component's cells;
  `ThermalProfile::mean_temperature_by`).
- Precedence, first match wins: the shape's own; its component's; its
  container's; the model's (still required, and the default).
- `NuclideKey` and the material entries carry the temperature, so each
  nuclide × law × temperature is one entry, processed once per load.
- `AssembledModel::temperatures_k`; `RunProvenance::temperatures_k`;
  `NuclidePhysics::temperature_k`; the run summary and report name the
  temperature of every nuclide; a slice legend names the temperature when
  there are several.
- **A guard:** `keff_with` refuses nuclides whose data temperature
  (`Nuclide::data_kt_ev`) differs from their key's by more than a relative
  1e-4 (`RunError::NuclideTemperature`). Since majorants are rebuilt on
  every run from the materials and the nuclides passed, this makes a
  majorant built at the wrong temperature unreachable through the beginner
  API.

## 2. Methodology

All in `tests/region_temperature.rs` (and the lowering checks in
`src/model/region_temperature_tests.rs`).

1. **Uniform by region = uniform model, bit for bit.** Two nested HEU
   spheres, every shape and the component set to the model's 293.6 K,
   against the model temperature alone. LOW-tier data, 300 × [3 + 6].
2. **Model = hand-built low-level path, bit for bit.** The LEU cell below,
   fuel at 1200 K, water at 293.6 K, through `Model`, against a hand-built
   CSG (surfaces and tokens in the lowering's order), hand-built materials,
   and nuclides from `Nuclide::from_endf_file_with_speed` at each
   temperature (`ThermalScattering::from_embedded_leapr_deck` for H in
   H2O), run by `run_keff_csg` (CpuMultiThread). Same seed, same global
   nuclide order (the URR stream is keyed by nuclide index).
3. **Doppler sign.** The same cell with the fuel at 293.6 K and at 1200 K.
   Pass: `k` falls by more than 3 combined sigma.
4. **Guard.** 293.6 K fuel nuclides handed to the 1200 K model.
5. **Majorant of a delta-tracked region at a non-default temperature.** A
   `DhRegion` at 1200 K in a 293.6 K model; audit its own majorant against
   its 1200 K materials. Control: the same region's majorant built at
   293.6 K, audited against the same 1200 K materials (must be under, or
   the audit could not fail).

**The LEU cell:** a UO2 sphere (r = 0.6 cm; 10.4 g/cm3, 4.5 wt% U-235,
giving U-235 1.0570e-3, U-238 2.2147e-2, O-16 4.6408e-2 /b-cm) in a cube of
light water (side 1.6 cm; 0.998 g/cm3, H-1 6.6735e-2, O-16 3.3368e-2 /b-cm,
H in H2O on), reflective outer surface. These are stated inputs chosen
before any run, not tuned; the H/U ratio (~10) makes it more moderated
than a PWR pin. Run 5000 × [20 + 100], seed 945.

**The DhRegion:** a 0.5 cm sphere of bound graphite (crystalline,
C-12 8.5e-2 /b-cm) with a lattice (pitch 0.15 cm, whole-particle rejection)
of particles: a 0.025 cm UO2 kernel (as above) in a 0.05 cm graphite
shell; delta tracked; inside a 0.6 cm graphite shell at 293.6 K with a
reflective outer surface. Audit: `Majorant::audit` over 1e-5 eV to 20 MeV,
200 000 log points plus every breakpoint of every nuclide in the region,
its one-ulp neighbours and the interval midpoints (857 176 energies),
`Σ_t` being the largest URR band total.

Data: ENDF/B-VIII.0 from `reference-data/endf/`
(`n-092_U_235-ENDF8.0`, `n-092_U_238`, `n-008_O_016-ENDF8.0`,
`n-001_H_001-ENDF8.0-Beta6`, `n-006_C_012-ENDF8.0`), embedded LEAPR decks
for H in H2O and crystalline graphite.

## 3. Pictures (drawn from the assembled geometry)

Written by `the_test_geometries_draw` with `OUTRAM_945_IMAGES` set.

- `leu_cell_xy.png`: the LEU cell, x-y through the centre. Checked: the
  sphere is centred and inside the cube; two materials, each labelled with
  its temperature (UO2 1200 K, water 293.6 K).
- `dh_region_xy.png`: the DhRegion at z = 0.075 cm (a layer of particle
  centres; z = 0 falls between layers). Checked: kernels on the 0.15 cm
  grid, corner positions rejected whole, the 293.6 K shell distinct from
  the 1200 K matrix in the legend. Not visible at this scale: the graphite
  coating (same material and temperature as the matrix, so the same
  colour).

## 4. Predictions (before running) and results

2026-10-18, i9-13900K, 6 of 32 logical cores (`taskset -c 4-9`, rayon 6
threads), 62 GB RAM, Linux 7.2.9, CPU only, machine shared with other
sessions.

| check | predicted | measured | verdict |
|---|---|---|---|
| 1. uniform by region | bit-identical | bit-identical (k by cycle, mean, std) | as predicted |
| 2. model vs low-level | bit-identical | bit-identical, `k = 1.49292 ± 0.00172` both ways | as predicted |
| 3. Doppler, Δk (1200 K − 293.6 K) | −1500 to −4000 pcm, lower half likely | **−1920 ± 258 pcm (7.5 σ)**; `k` 1.51212 ± 0.00192 → 1.49292 ± 0.00172 | inside, lower half |
| 3. in reactivity | (not predicted separately) | −851 pcm, average −0.94 pcm/K | — |
| 4. guard | refused | refused: U235 at 293.6 K for a 1200 K region | as predicted |
| 5. own majorant, worst Σ_t/Σ_maj | ≤ 1 | **0.769** (9.9e-5 eV, UO2) | as predicted |
| 5. control, 293.6 K majorant | under by 1.3× to 4× | **1.314 under at 6.48 eV** (UO2) | inside, at its low end |

**Interpretation.**

- 1 and 2 show stage 1 adds no physics of its own: a region temperature
  lowers to exactly the materials and nuclides a careful hand-written model
  has, and transports identically.
- 3 is a sign check, not a benchmark. The prediction anchor (an LWR fuel
  coefficient of −2 to −4 pcm/K) was from memory, with no corpus citation;
  the measured −0.94 pcm/K in reactivity is well below it, which fits the
  cell's over-moderation (k = 1.5, less resonance absorption) but was not
  separately predicted. The −1920 pcm Δk sits inside the stated range
  because k ≈ 1.5 multiplies Δρ by k².
- 5: the region's own majorant bounds it everywhere checked; the worst
  point is just below the majorant's 1e-4 eV floor, where `Majorant::at`
  extrapolates as 1/v. The control shows the instrument can fail: a
  majorant built at 293.6 K is 1.31× under the 1200 K fuel on the wing of
  the U-238 6.67 eV resonance, the same place and size as
  `tests/delta_tracking_audit.rs` (1.34× at 6.5 eV on CORE data). #948's
  3.28× was at 1500 K, a larger step.

**Cost.** Processing the hot LEU model's 5 nuclide × temperature entries
took 148 s, the cold model's 4 took 72 s, the hand path's 5 took 139 s
(each set in parallel, the two tests running at once). Transport 5 s per
LEU run. The DhRegion's 5 entries: 102 s; its audit 18 s. Each distinct
temperature costs a full processing of every nuclide present at it: a
`ThermalProfile` with N components of different temperature multiplies
processing by up to N. That is the price of "exact, not interpolated" and
is what stage 2 (#947's stochastic interpolation on a fixed grid) removes.

## 5. Limits

- One temperature per region. A gradient inside a region (a field, fixed
  points) is stage 3; `take_temperatures_from` averages it away, per
  component, and a component no voxel centre lands in keeps what it had.
- Temperatures are matched exactly (bit for bit) when materials and
  nuclides are shared, so 900.0 K and 900.0000001 K are processed twice.
  OpenMC's comment in `get_temperatures` says it rounds to the nearest
  integer, but its code (`d7d3284a1`) does not.
- S(α,β) at a region temperature runs the LEAPR deck's block for that
  temperature (`leapr::generate::temperature_block_index`): a deck with
  one spectrum (graphite) runs at any temperature; a deck that
  re-specifies its spectrum per temperature (H in H2O, D in D2O, O in D2O)
  refuses a temperature it does not declare, so a water region must sit
  at one of the deck's temperatures.
- The ACE route reads tables at their own temperatures; a region at a
  temperature no file has is refused (`DataError::Ace`), as for the model
  temperature before.
