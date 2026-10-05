# Recipe: HTR-10 pebble bed

```toml
[kovan]
id = "recipe"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

A Dhoby Ghaut recipe: the input deck of a guided high-fidelity build. Load it in the workbench to reopen the build, or read it as a record of what was chosen and why. Research, education and V&V only.

```toml
format = "dhoby-ghaut-recipe"
version = 1
title = "HTR-10 pebble bed"
reactor = "htgr"
mode = "basic"
preset = "htr10"
edited = false
vv_status = "TENTATIVE. Code-to-code against RMC (Li, Yu & Wei 2014), not against experiment. Open residual: -2726 pcm on ENDF/B-VIII.0 at the critical loading (explicit reflector, fast single-seed statistics, gh:#333), with a height-dependent drift (gh:#218) still unexplained. AI-drafted model awaiting human review."
```

# Step 0: Nuclear data library

```toml
[kovan]
id = "step-0"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

The folder of evaluated tapes the run reads, the library and the data temperature.

```toml
endf_dir = "/home/teddy0/Documents/research/outram-park-backend/.claude/worktrees/agent-a207d63f3e500c021/reference-data/endf"
library = "ENDF/B-VIII.0"
temperature_k = 300.15

[[cite]]
field = "temperature_k"
citekey = "li2014htr"
what = "the benchmark is at 27 °C (300.15 K) throughout; Şeker & Çolak (2003) state the same"
```

# Step 1: DEM pebble bed construction

```toml
[kovan]
id = "step-1"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

How the pebbles pack, how high the bed is loaded and the pebble-type mix.

```toml
packing = "Şeker 13-ball hexagonal prism cell; whole balls, rejected at the wall, cone and tube"
rings = 14
layers = 19
filling_fraction = 0.61
source = "lattice"

[mix]
fuel = 0.57
moderator = 0.43
fertile = 0.0
poison = 0.0

[dem]
n_pebbles = 27000
friction = 0.1
rolling_friction = 0.0
youngs_modulus_pa = 500000000.0
seed = 1592590352

[[cite]]
field = "mix"
citekey = "li2014htr"
what = "Table 1: fuel-to-moderator ball ratio 0.57/0.43"

[[cite]]
field = "filling_fraction"
citekey = "li2014htr"
what = "body text: ball filling fraction 0.61 in the core"

[[cite]]
field = "packing"
citekey = "seker2003htr10"
what = "Table 3: 1346 N + 733 balls for N layers; the bed is built to this inventory"
```

# Step 2: Pebble design

```toml
[kovan]
id = "step-2"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

One design per pebble type in the mix. The voids between pebbles are the interstitial coolant.

```toml
interstitial = "helium"

[[pebble]]
kind = "fuel"
outer_radius_cm = 3.0
fuel_zone_radius_cm = 2.5
matrix = "graphite"

[pebble.triso]
kernel = "UO2"
enrichment = 0.17
kernel_radius_cm = 0.025
layers = [
    [
    "buffer PyC",
    0.034,
],
    [
    "IPyC",
    0.038,
],
    [
    "SiC",
    0.0415,
],
    [
    "OPyC",
    0.0455,
],
]
particles_per_pebble = 8335

[[pebble]]
kind = "moderator"
outer_radius_cm = 3.0
matrix = "graphite"

[[cite]]
field = "pebble.outer_radius_cm"
citekey = "li2014htr"
what = "Table 2: 6 cm ball diameter, fuel and moderator"

[[cite]]
field = "pebble.fuel.triso"
citekey = "li2014htr"
what = "Table 2: 8335 particles per ball, 250 µm kernel, UO2 at 10.4 g/cm³, 17 wt% U-235"

[[cite]]
field = "pebble.fuel.fuel_zone_radius_cm"
citekey = "li2014htr"
what = "Table 2: 2.5 cm fuelled zone"
```

# Step 3: Reflector and internals

```toml
[kovan]
id = "step-3"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

Reflector, boronated bricks, borings, chutes, plenums and risers, each with its status in the model.

```toml
core_radius_cm = 90.0
reflector_outer_cm = 190.0
model_height_cm = 610.0

[[element]]
name = "graphite reflector zones (TECDOC Fig. 4.10 zone map)"
status = "in_model"
note = "each zone with its own Table 4-3 composition"

[[element]]
name = "boronated carbon bricks"
status = "in_model"
note = "own material (mat::BORONATED)"

[[element]]
name = "control-rod channels (10)"
status = "in_model"
note = "explicit borings; 18° azimuth convention (gh:#330)"

[[element]]
name = "small-absorber-sphere (KLAK) channels (7)"
status = "in_model"
note = "explicit borings, including the slot section"

[[element]]
name = "irradiation channels (3)"
status = "in_model"
note = "explicit borings"

[[element]]
name = "cold gas risers (20 coolant channels)"
status = "in_model"
note = "explicit borings in the side reflector"

[[element]]
name = "hot gas duct"
status = "in_model"
note = "explicit"

[[element]]
name = "defuelling chute (discharge tube)"
status = "in_model"
note = "explicit whole graphite balls, Li (2014)'s rejection rule"

[[element]]
name = "cold helium chamber (above the core)"
status = "simplified"
note = "TECDOC zone 3 composition; not an explicit plenum"

[[element]]
name = "hot gas plenum (below the core)"
status = "simplified"
note = "represented by its TECDOC zone composition; check against the zone map"

[[element]]
name = "refuelling chute"
status = "not_in_model"
note = "not in the benchmark model the record follows; gh:#570"

[[cite]]
field = "elements"
citekey = "iaea-tecdoc-1382-part2"
what = "Fig. 4.10 zone map and Table 4-3 zone compositions; p. 242 (printed) for the corrections once borings are explicit"

[[cite]]
field = "core_radius_cm"
citekey = "li2014htr"
what = "Table 1: core diameter 180 cm"
```

# Step 4: Inserts

```toml
[kovan]
id = "step-4"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

What fills the borings, each with its status in the model.

```toml
control_rods = 10
b4c_inner_radius_cm = 3.0
b4c_outer_radius_cm = 5.25
b4c_density_g_per_cm3 = 1.7

[[element]]
name = "control rods (B4C, steel sleeves, iron joints)"
status = "in_model"
note = "explicit; withdrawn (the benchmark's state) unless Step 5 inserts them, all ten together (gh:#580)"

[[element]]
name = "small absorber spheres"
status = "not_in_model"
note = "KLAK channels are empty (maintainer, gh:#330); gh:#570"

[[element]]
name = "pebbles in the irradiation channels"
status = "not_in_model"
note = "irradiation channels are empty (gh:#330); gh:#570"

[[element]]
name = "pressure vessel and core barrel steel"
status = "not_in_model"
note = "the model ends at the reflector's outer radius, 190 cm"

[[cite]]
field = "control_rods"
citekey = "iaea-tecdoc-1382-part2"
what = "control-rod geometry and B4C composition"
```

# Review gate: geometry

```toml
[kovan]
id = "review"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

The assembled-geometry slices a human looked at before Monte Carlo ran.

```toml
viewed = []
```

# Step 5: Monte Carlo

```toml
[kovan]
id = "step-5"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

Run settings, the ablations (if any) and the runs made.

```toml
particles = 2000
inactive = 30
active = 70
seed = 20260917
threads = 14
rod_insertion = 0.0
spectrum_bins_per_decade = 10
ablations = []
run = []

[[cite]]
field = "particles"
citekey = "li2014htr"
what = "the paper ran 10 000 × [5 + 135]; the prefill is the workspace's QUICK record statistics, 2000 × [30 + 70]"
```

# Step 6: Parameter extraction: choose a branch

```toml
[kovan]
id = "step-6"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

The branch (multiphysics, the default, or a reactivity map) and the reactivity map's fit settings and state-point plan.

```toml
branch = "multiphysics"
plan_lo_k = 300.0
plan_hi_k = 1200.0
plan_points = 5

[map]
order = 2
temperature_basis = "sqrt"
```

# Step 9: Multiphysics case setup

```toml
[kovan]
id = "step-9"
kind = "note"
created = "2026-10-05T15:56:44Z"
modified = "2026-10-05T15:56:44Z"
```

Boundary conditions, models and solver settings of the coupled case: the OUTRAM-Foam porous-core side, neutronics, the coupling loop and the farrer-park structural side. Each part of the model is listed as in model, simplified or NOT in model.

```toml
[foam]
core_radius_cm = 90.0
core_height_cm = 192.16122045152153
filling_fraction = 0.61
pebble_diameter_cm = 6.0
fuel_pebble_fraction = 0.57
inlet_temperature_c = 250.0
total_mass_flow_kg_s = 4.32
core_mass_flow_kg_s = 3.77
outlet_pressure_mpa = 3.0
wall = "adiabatic"
flow_downward = true
fast_fluence_1e25_per_m2 = 0.0
friction_model = "KTA 3102.3 packed-bed friction"
heat_transfer_model = "Wakao-Funazkri (1978) particle-to-fluid Nusselt"
pebble_model = "two-zone conduction + hottest TRISO (tampines Pebble::htr10)"
helium_properties = "outram-park-fork-coolprop helium EOS, Arp-McCarty-Friend viscosity, Hands-Arp conductivity"
radial_rings = 40
axial_nodes = 200

[neutronics]
thermal_power_mw = 10.0
isothermal_coefficient_per_k = -0.00014
reference_temperature_k = 300.15
weighting = "power"

[neutronics.shape]
kind = "diffusion"

[coupling]
max_iterations = 150
pressure_tolerance = 0.0001
temperature_tolerance_k = 0.01
flow_relaxation = 0.8
power_relaxation = 0.5
power_tolerance = 0.0001
k_tolerance = 0.000001

[structural]
enabled = false
element = "Tet4 (never polyhedral for stress)"
material_model = "linear elastic, small strain"
load = "thermal expansion from the TH temperatures (needs the Step 7 mapping)"

[[element]]
name = "Helium energy balance"
status = "in_model"
note = "Exact enthalpy march per ring with the coolprop-port helium EOS ((p, h) flash)."

[[element]]
name = "Pebble-to-helium heat transfer"
status = "in_model"
note = "Wakao-Funazkri (1978) on local Re, Pr and k (tampines::pebble_bed::cht)."

[[element]]
name = "Pebble and TRISO conduction"
status = "in_model"
note = "Two-zone pebble + hottest TRISO at the pebble centre (tampines::pebble_bed::Pebble::htr10), fluence from Step 9."

[[element]]
name = "Bed friction and flow split"
status = "in_model"
note = "KTA 3102.3 per node; rings share one plenum-to-plenum pressure drop (the coupling loop)."

[[element]]
name = "Thermal-hydraulic mesh"
status = "simplified"
note = "Equal-area rings x axial nodes (r-z multi-channel). Step 7's tet-dual TH mesh carries power and temperature between the neutronics mesh and the rings but is not solved on (gh:#592)."

[[element]]
name = "OUTRAM-Foam porous solver"
status = "not_in_model"
note = "outram-foam-appbuilder-lib's OnePhaseSolver has constant properties and one-cell tests only; not used (gh:#592)."

[[element]]
name = "Radial conduction and radiation between rings"
status = "not_in_model"
note = "ZBS effective conductivity (tampines::pebble_bed::zbs) is not applied across rings (gh:#592)."

[[element]]
name = "Side wall"
status = "simplified"
note = "Adiabatic: no heat to the side reflector or the RCCS (gh:#592)."

[[element]]
name = "Reflector, plenums, bypass channels"
status = "simplified"
note = "Not modelled thermally; the bypass (total minus core flow) is mixed at the inlet temperature."

[[element]]
name = "Power shape and k"
status = "in_model"
note = "Solved (gh:#591): GeN-Foam port multigroup diffusion k-eigenvalue on Step 7's neutronics mesh, Step 8's constants at each cell's TFuel (ln T, extrapolated linearly outside the state points as upstream), Marshak vacuum boundary, Picard-coupled to the march through Step 7's maps with relaxed power. Ablation: --prescribed-power (J0 x cosine with lumped feedback)."

[[element]]
name = "Neutronics data"
status = "simplified"
note = "2 groups by default, P0 scattering, D = 1/(3 Sigma_t), no discontinuity factors, no delayed neutrons (steady state only), Monte Carlo statistics of a short run (gh:#595)."

[[element]]
name = "Neutronics regions"
status = "simplified"
note = "Cells take their region by centroid on the 30 cm neutronics mesh: region boundaries are stair-stepped, borings not explicit (gh:#594). Fission power landing outside the TH bed is reported and the bed power rescaled to the thermal power."

[[element]]
name = "Temperature feedback"
status = "simplified"
note = "One temperature per cell drives every material's constants: the fuel-pebble volume average in the bed (state points are isothermal, gh:#595); no separate moderator / coolant-density feedback."

[[element]]
name = "Reflector, conus, tube temperatures (for the cross sections)"
status = "simplified"
note = "Held at the inlet helium temperature: no reflector heat balance (gh:#592)."

[[element]]
name = "Neutronics -> ring grid transfer"
status = "simplified"
note = "Power: Step 7's volume-weighted map to the TH mesh, then each bed cell's power shared over the ring nodes its nearest-cell samples fall in (conservative). Temperature: TH cell takes its centroid's node, then Step 7's map to the neutronics mesh."

[[element]]
name = "Structural (farrer-park)"
status = "not_in_model"
note = "farrer-park is FEM mechanics only (no heat conduction) and has no mapping from the TH mesh yet (gh:#593); not run."

[[cite]]
field = "foam.core_radius_cm"
citekey = "li2014htr"
what = "Table 1: core diameter 180 cm"

[[cite]]
field = "foam.core_height_cm"
citekey = "li2014htr"
what = "Table 1: core height 197 cm"

[[cite]]
field = "foam.filling_fraction"
citekey = "li2014htr"
what = "body text: ball filling fraction 0.61"

[[cite]]
field = "foam.pebble_diameter_cm"
citekey = "iaea-tecdoc-1382-part2"
what = "Chapter 4: ball diameter 6.0 cm"

[[cite]]
field = "foam.inlet_temperature_c"
citekey = "gao2002htr10th"
what = "Table 2: helium at reactor inlet 250 °C at 100 % load"

[[cite]]
field = "foam.outlet_pressure_mpa"
citekey = "gao2002htr10th"
what = "Table 2: coolant pressure 3 MPa"

[[cite]]
field = "foam.total_mass_flow_kg_s"
citekey = "gao2002htr10th"
what = "Table 2: coolant mass flow 4.32 kg/s"

[[cite]]
field = "foam.core_mass_flow_kg_s"
citekey = "gao2002htr10th"
what = "Table 1: 3.77 kg/s through the pebble bed and bottom reflector (87.3 %); §1 asks at least 86 %"

[[cite]]
field = "foam.flow_downward"
citekey = "gao2002htr10th"
what = "§2: cold helium from the top plenum flows downward through the bed"

[[cite]]
field = "foam.fuel_pebble_fraction"
citekey = "gao2002htr10th"
what = "§4.1: equilibrium core, fuel element heat factor 1.0 (no graphite balls)"

[[cite]]
field = "neutronics.thermal_power_mw"
citekey = "li2014htr"
what = "Table 1: thermal power 10 MW"

[[cite]]
field = "neutronics.shape"
citekey = "gao2002htr10th"
what = "§4.2: max power density 2.57 W/cm³ (equilibrium core) against Table 2's 2 MW/m³ mean (the prescribed-shape ablation's input; the comparison for the solved shape)"

[[cite]]
field = "neutronics.isothermal_coefficient_per_k"
citekey = "chen2009htr10"
what = "Table 1: -1.4e-4 dk/k per °C (as transcribed in htgr_sim_v1 kinetics.rs; not in the kovan corpus)"
```

