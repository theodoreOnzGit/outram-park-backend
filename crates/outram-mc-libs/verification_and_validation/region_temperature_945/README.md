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

Written by `the_test_geometries_draw` with `OUTRAM_945_IMAGES` set. The images are
not committed (gitignored here); they are posted on gh #945, by the maintainer's
rule that V&V images go on the issue.

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


---

# Stage 3: a majorant that covers a temperature range (2026-10-19)

**Status:** untrusted AI-generated draft per `RESPONSIBLE_USE.md`; research,
education and V&V only. Predictions were posted on #945 before any run
([comment](https://github.com/theodoreOnzGit/outram-park-backend/issues/945#issuecomment-6258439360)).
Code: `src/physics/delta_tracking/temperature_cover.rs`; tests:
`tests/majorant_temperature_cover.rs`, plus unit tests in the module and
`src/model/advanced.rs`. Raw logs are not committed (private scratch).

## 6. Construction

Maintainer direction (#945, 2026-10-17): "chebyshev polynomial to cover logE
and neutron T. Don't have to fit exactly but have to cover everything", then
"Or rather a few regions". A cover, not a fit: looseness costs virtual
collisions, an under-bound anywhere is a defect.

**Searched first.** The workspace's Chebyshev code is `material::
temperature_interpolation` (nodes and an interpolant of cross-section values
in T, #947) and `petir::ChebSeries`; neither produces a bound, so neither was
reused for the cover (the cover has its own Clenshaw evaluation, 15 lines).
Reused: `Majorant::bounding` (per-table bound by construction),
`Majorant::audit`, the stage-1 lowering and `AssembledModel::majorants`.

**Step 1, the T axis (`Majorant::over_temperature_tables`).** Process the
region's nuclides at tables spanning `[T_min, T_max]`; build
`Majorant::bounding` at each; take the pointwise maximum on the union of the
grids; multiply by `1 + t_margin` (`DEFAULT_T_MARGIN = 0.01`). Guaranteed at
each table. **Between tables it is not guaranteed by construction**: at a
fixed `E` near a resonance's half-width crossover `Σ_t(T)` is not monotone
(the peak falls, the wing rises), so its maximum over a temperature interval
can exceed both bracketing tables; and tables processed separately differ by
the processing tolerance. So `t_margin` is *measured* against data processed
exactly between the tables (section 7), not proven.

**Step 2, the E axis (`Majorant::chebyshev_cover`).** The step-1 node table
(~3e5 nodes) becomes `regions` bands equal in `ln E`, snapped to nodes; band
`b` is `exp(P_b(x) + s_b)` with `x = ln E` scaled to `[-1, 1]` and `P_b` the
Chebyshev interpolant, at the Chebyshev nodes, of the band's upper envelope of
`ln Σ_maj` (the best of degrees `0..=degree` by the band's mean `ln` cover).
**The lift `s_b` is proven, not sampled:** the table's value on a node interval
is the larger of its end nodes (what `Majorant::at` returns), a constant `y_i`;
`|T_j'| <= j^2` on `[-1, 1]`, so `|P'| <= L = Σ j^2 |c_j|`; on an interval of
width `w`, `min P >= min(P at its ends) - L w / 2`, and intervals wider than
`W_MAX = 2e-3` are subdivided first; `s_b = max_i (y_i - bound on min P over
interval i) + 1e-12`. Then the cover is `>=` the table on every interval, so
`>=` the bound it covers. Below the first node the 1/v extrapolation of
`Majorant::at` is kept. This is a guarantee against the step-1 table; the
data-level guarantee is step 1's. `tests/majorant_temperature_cover.rs` also
checks the proof on the data: the cover is not below its table at 1.2 million
probe energies (400 001 log points and their 1e-6 relative neighbours).

**Where it is exposed.** `model.advanced().with_majorant_cover(CoverSettings {
regions, degree })`, carried to `AssembledModel::majorants`, which then
covers each delta-tracked region's stage-1 table. It changes cost, never the
answer, so it is not an ablation and is not listed in `provenance.ablations`.
**Without the call nothing changes: the stage-1 per-temperature table stays
the default** (pinned by `advanced_cover_reaches_the_region_majorant_and_
bounds_the_table`, `cover_bands() == 0`). The model path today builds one
temperature per region, so what it covers is the energy axis of a region's
stage-1 bound; the `T`-range tables of step 1 are a library function
(`over_temperature_tables`) that a temperature field (#949) will feed. They
are not wired into `Model` yet.

## 7. Methodology

ENDF/B-VIII.0 (`reference-data/endf/`), RECONR + BROADR (tolerance 1e-3, speed
tier `Fast`), URR tables, and LEAPR crystalline graphite for the bound carbon,
each processed **exactly** at 293.6, 450, 600, 750, 900, 1050, 1200, 1350 and
1500 K.

- `U238`: U-238 metal, 0.04784 /b-cm (the worst Doppler case, and a URR
  nuclide).
- `MIX`: graphite (C-12 tape 0.0800 /b-cm, crystalline S(α,β)) with UO2 smeared
  at 5 % by volume (U-235 5.285e-5, U-238 1.10735e-3, O-16 2.3204e-3 /b-cm).
  The C-12 tape stands for carbon; natural carbon's C-13 is not included.

Tables: `{293.6}` (control), `{293.6, 1500}`, `{293.6, 900, 1500}`,
`{293.6, 600, 900, 1200, 1500}`. Each table's bound is `Majorant::bounding`
(4096 bins x 32, 1e-4 eV to 20 MeV), as stage 1, at per-table margin `m0 = 0`
(isolates the between-table excess) and `0.3` (stage 1's). **Audit:**
`Majorant::audit_over_temperatures` against every processed temperature that
is not a table (the between-table test) and the tables themselves: 50 000 log
energies over 1e-5 eV to 20 MeV plus every nuclide breakpoint, its ulp
neighbours and every interval midpoint, `Σ_t` the largest URR band. **Cost:**
mean `Σ_maj / Σ_t` (the expected virtual collisions per real one, plus 1) at
20 000 log energies over 1e-3 eV to 20 MeV for the material at **1050 K**, flat
in lethargy and in three windows (thermal < 1 eV, epithermal 1 eV to 10 keV,
fast > 10 keV). This is a lethargy-weighted proxy, **not** a mean along
histories (no run was made), so it over-weights regions neutrons rarely visit
and the absolute figures are not the transport slowdown; the ratios between
constructions are the finding. Build time is wall clock for the bounding(s) and
merge; the data processing is not in it (U-238: 628 s for the nine
temperatures; the mix about 1900 s).

Hardware: 13th Gen Intel Core i9-13900K, 4 of 32 logical cores
(`taskset -c 0-3`, rayon 4 threads), 62 GB RAM, Linux 7.2.9, CPU only (GPU
hidden), machine shared with other sessions.

## 8. Predictions (before running) and results

| check | predicted | measured | verdict |
|---|---|---|---|
| between-table excess, U-238, `m0 = 0`, `t_margin = 0` | worst `Σ_t/Σ_maj` 1.00 to 1.15 | **1.0000000** with two tables 1207 K apart (ties at 2e7 eV, where the data are not broadened); the single-table control is **2.256 under** at 6.48 eV, 1500 K | refuted (smaller than predicted: none measurable) |
| same, `m0 = 0.3` | <= 1, about 0.8 to 0.95 | **0.769** (= 1/1.3) at every table count | as predicted |
| between-table excess, graphite + UO2, `m0 = 0` | smaller than U-238 | **1.00127** at 1.69 MeV, 600 K (ends only); **1.00049** at 1.77 eV, 1200 K (3 tables); **1.00023** at 3.37 eV, 750 K (5 tables); single-table control 2.46 under at 1.4e-5 eV, 1500 K | refuted (larger than U-238's, which had none) |
| same, `m0 = 0.3` | <= 1 | **0.770** (ends), 0.770, 0.769 | as predicted |
| overhead of the T-max over a rebuild at the middle T | 1.1x to 1.6x | U-238 **1.37x** (4.33 against 3.16 flat; epithermal 8.68 against 5.86); mix **1.06x** (1.48 against 1.40) | inside for U-238, below for the mix |
| Chebyshev cover looseness, tens of bands, degree 4 | 5x to 100x the table | U-238 64 x 4 **10.1x**, 256 x 4 **2.9x**; mix 64 x 4 **1.9x**, 256 x 4 **1.2x** | inside for U-238, tighter for the mix |
| cover with 1 to 4 bands | over 100x | U-238 **99x to 782x**; mix **6x to 40x** | U-238 as predicted, mix far tighter |
| build: tables cost n x one bounding, the cover fit under 2 s | | U-238 0.9 s per bounding, 5 tables 4.8 s; mix 8.7 s per bounding, 5 tables 46 s; cover fit **0.00 to 0.09 s** | as predicted |

### 8.1 What the between-table numbers say

- U-238 alone shows **no excess** at 7 temperatures between two tables 1207 K
  apart, to 1e-7. Its broadened resonance region is smooth enough in `T` that
  the maximum over the two end tables bounds the intermediate ones. This
  refutes the prediction of a small excess; it does not prove the wing-crossover
  argument wrong for a narrower table spacing question, it only says it was not
  measurable here.
- The mix does show one: **0.127 %** (ends), **0.049 %** (3 tables), **0.023 %**
  (5 tables) at zero margin, falling with the spacing. The worst of the ends-only
  case is at 1.69 MeV, where the data are not Doppler-broadened at all: the
  difference there is the processing tolerance of separate RECONR/BROADR runs
  (1e-3), not Doppler physics. The 3- and 5-table worst points are at 1.77 eV
  and 3.37 eV, in the resolved resonances, where I did not separate the
  two causes. **Not diagnosed further.** The stage-1 per-table 30 % margin
  already covers all of it (worst 0.770).
- At margin 0 the mix is also 1.015 over at the tables themselves, at 1.4e-5 eV,
  *below* the 1e-4 eV floor: `Majorant::at`'s 1/v extrapolation under-bounds the
  S(α,β) graphite total there (pre-existing, not a temperature effect; the 30 %
  margin covers it, 0.78).
- **Margin chosen:** `DEFAULT_T_MARGIN = 1 %`, 8x the worst measured excess
  (0.127 %), as a floor for cases not measured (water, other nuclides), **not**
  a derived bound. The guarantee between tables is therefore empirical on these
  two contents; it is not a proof.

### 8.2 The Chebyshev cover, regions x degree (mean `Σ_maj/Σ_t`, flat in lethargy, 1050 K; 5-table bound)

Table (no cover): U-238 4.33, mix 1.48. Per-temperature rebuild at 1050 K
(stage 1): U-238 3.16, mix 1.40. Multiple of the 5-table bound:

| regions \ degree | U-238 0 | 2 | 4 | 8 | mix 0 | 2 | 4 | 8 |
|---|---|---|---|---|---|---|---|---|
| 1 | 367 | 783 | 218 | 287 | 40.6 | 37.4 | 16.5 | 13.0 |
| 4 | 148 | 213 | 319 | 99 | 14.1 | 10.5 | 8.8 | 6.2 |
| 16 | 57 | 186 | 191 | 115 | 5.8 | 6.3 | 6.7 | 4.6 |
| 64 | 22.5 | 24.7 | 10.1 | 5.9 | 3.1 | 2.5 | 1.9 | 1.5 |
| 256 | 7.5 | 4.0 | **2.9** | 2.4 | 1.7 | 1.3 | **1.2** | 1.1 |
| 1024 | 2.5 | 1.7 | 1.6 | 1.5 | 1.2 | 1.1 | 1.1 | 1.1 |

Fit time 0.00 to 0.09 s in every cell. The cover is very loose where there are
resonances: a band holding a resonance sits above its peak everywhere in the
band. It is **not monotone in degree** at few bands (U-238 1 region: 367, 783,
218, 287): the fit keeps the degree with the smallest mean *ln* cover, which can
raise the linear mean the transport pays for. Choosing by the linear mean
(`∫ exp(P+s)/table`) would fix that; not done, noted as open.

### 8.3 Defaults chosen, with the evidence

`CoverSettings::default() = { regions: 256, degree: 4 }`: 1280 coefficients in
place of ~3e5 nodes; 3.0x (U-238) and 1.2x (mix) the table's virtual-collision
proxy; 64 bands cost 10x and 1.9x; 1024 bands buy 1.6x and 1.1x for four times
the coefficients; degree 8 at 256 bands buys under 20 %. **A cost choice, not an
accuracy choice**: the cover holds at every setting tried, because the lift is
computed for each. Used only if `advanced()` asks; the table stays the default.

**The shipped default, audited** (`default_audit`: tables 293.6 and 1500 K only,
`m0 = 0.3`, `t_margin = 1 %`, cover 256 x 4; audited at 293.6, 750 and 1500 K):

| content | table worst | cover worst | table build | cover fit | mean `Σ_maj/Σ_t` at 750 K, flat/thermal/epi/fast: table -> cover |
|---|---|---|---|---|---|
| U-238 | 0.7616 (1e-4 eV, 1500 K) | 0.7616 (2e7 eV, 293.6 K) | 1.9 s | 0.02 s | 4.65/1.33/9.46/1.82 -> 13.70/1.33/32.61/2.02 |
| graphite + UO2 | 0.7729 (1.4e-5 eV, 1500 K) | 0.7729 | 19.2 s | 0.03 s | 1.53/1.51/1.68/1.37 -> 1.83/1.53/2.37/1.46 |

Every ratio is at most 1. (The sweep's own end-of-run audit used 64 x 4 and
all nine temperatures: U-238 0.7692, mix 0.7803, both at most 1; those cells
are in the 64-band rows above.)

### 8.4 Against the per-temperature rebuild (stage 1)

- **Build:** a rebuild is one `bounding` (U-238 0.9 s, mix 8.7 s), paid again
  whenever the region's temperature changes. The range bound is `n_tables`
  boundings (4.8 s and 46 s for five) paid **once** for the whole range, and the
  cover fit adds 0.02 to 0.03 s. After that a temperature change inside the
  range costs nothing: no reprocessing and no rebuild, the saving #949's Picard
  iteration wants. Data processing, the larger cost (about 70 s per U-238
  temperature, about 200 s for the mix), is not saved by this; stage 2's
  stochastic interpolation is what removes it.
- **Virtual collisions:** the range-wide table is 1.37x (U-238) and 1.06x (mix) the
  rebuild's proxy; the default cover is a further 3.0x and 1.2x on top of the
  table, so about 4.3x (U-238) and 1.3x (mix) a rebuild at the exact
  temperature. The U-238 cost sits in the epithermal window (8.7 and 32.6
  against 5.9 for a rebuild); thermal and fast are near 1.

## 9. Limits and open questions

- **The between-table guarantee is empirical.** Measured on two contents, nine
  temperatures, 293.6 to 1500 K. The mix's worst points (resolved resonances,
  and a processing-tolerance point at 1.69 MeV) were not separated further. A
  denser table set can only shrink the excess; the margin should be re-measured
  for water (H in H2O, which refuses undeclared temperatures) and a WMP nuclide,
  where the T dependence is continuous (#948): not measured here.
- **Not wired into `Model` beyond the E-axis cover.** Step 1 is a library
  function; the model still builds one temperature per region.
- **No transport run.** The cost figure is a lethargy-weighted proxy, not a mean
  along histories, and no k was computed with a cover; the bound is checked
  against the data, not by transport. A cover only changes the number of virtual
  collisions, but that is a statement of the method, not a measurement here.
- **Band edges are equal in `ln E`**, not at the data's own boundaries (S(α,β)
  cutoff, resolved/unresolved limits) the maintainer named. Whether
  data-driven edges tighten the cover at equal band count is untested.
- **Selection of the degree** by mean `ln` (not linear overhead), see 8.2.
- The U-238 file here is the repository's `n-092_U_238.endf`, as stage 1.
