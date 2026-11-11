# EBR-II in the engine library (`outram_park_digital_twin_engine::ebr2`): V&V record

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety-critical decisions or any operational purpose (`RESPONSIBLE_USE.md`).
> EBR-II was shut down in 1994. The numbers used here are published benchmark
> specifications, not operational data.

**Status: NOT VALIDATED. AI draft (Claude Opus 5.5, Claude Code), not yet
reviewed by a human.** Issue gh:#1331, epic gh:#1312. Results taken on
2026-11-10, branch `feat/engine-ebr2-lib` (from `develop` fef1ebdea), Rust
1.99.1, release profile. The model is deterministic, so each number is exact
for its inputs and carries no statistical uncertainty. The uncertainty that
matters comes from the uncited modelling choices, and is given as the spread
over the ablations below.

## What the module is

`ebr2` is the data and the stepping model a web-simulation author needs to
build an EBR-II SHRT-17 / SHRT-45R simulator from this crate alone. It reaches
the `outram-park` metacrate at the next lockstep release.

| Module | Contents | Provenance |
|---|---|---|
| `design` | subassembly, fuel, core and plant numbers; NATDEMO pump coefficients; IHX geometry; U-5Fs properties | ANL-ARC-226 / ARC-228 via `reference-data/ebr2/*.csv`, and gh:#1313 / gh:#1315 |
| `shrt` | SHRT-17 / SHRT-45R test descriptions, initial conditions, tabulated boundary conditions, interpolated in time | ARC-226 via `reference-data/ebr2/*.csv` |
| `kinetics` | Argonne's beta-eff, generation time and DIF3D / VARIANT coefficients | IAEA-TECDOC-1819 TABLE 37 and Section 7.19, via gh:#1314 |
| `pump` | the NATDEMO homologous head curve | ARC-226 Eqs 3.1-3.7 |
| `model` | `Ebr2Model`: `shrt17(dz)`, `shrt45r(dz)`, `new(config)`, `step(dt)`, getters, setters | new; composes `teh-o-prke` and TUAS |

**Reused, not rewritten:**

- `teh_o_prke::zero_power_prke::n_group_prke::PointKinetics` (backward Euler);
- `teh_o_prke::feedback_mechanisms::fast_reactor` (gh:#1326: Doppler, axial,
  radial, sodium density, driveline);
- `teh_o_prke::decay_heat::DecayHeat`;
- TUAS `liquid_sodium()` (CoolProp `LiqNa`, gh:#1320);
- TUAS `liquid_metal` Nusselt correlations (gh:#1321): Mikityuk in the core
  and on the IHX shell side, Lyon-Martinelli in the IHX tubes;
- TUAS `SteelSS304LHighTemp` (Kim 1975, ANL-75-55).

**New:**

- the lumped four-node thermal model;
- the quasi-static loop momentum balance with the NATDEMO curve;
- the epsilon-NTU IHX;
- the coupling of all of these.

Every coefficient is derived from cited geometry and property data. Nothing
is fitted to a SHRT result.

**Checked and not used:**

- `tampines::components::pump_coastdown::InertialPumpCoastdown`. The SHRT
  pump speeds are tabulated boundary conditions, so a coast-down model is
  not needed to reproduce the tests. The pump inertia and torques are exposed
  (`design::pump_inertia_and_torques`) for a caller who wants to drive the
  pumps with it instead.
- `outram_mc_libs::model::HexSubassembly`. That is a Monte Carlo geometry
  builder, and a lumped model has no use for it. The fresh MARK-II k-inf
  (gh:#1317, `feat/1317-ebr2-fresh-kinf`) is not duplicated.

## Model summary

The full derivations are in the `model` module doc.

- **Power.** SHRT-17 follows the A226 Table 3.1.4.2.1 fission and decay-heat
  table. SHRT-45R uses six-group point kinetics:
  - beta-eff 705 pcm and Lambda 3.58238e-7 s, from TECDOC-1819;
  - the group shape is `teh-o-prke`'s U-235 one, because Argonne's six-group
    set is not printed;
  - Argonne's VARIANT feedback coefficients;
  - decay heat from the 1978 ANS draft U-235 set, driven through A226's
    assumed pre-test history.
- **Thermal nodes.**
  - Fuel: 439.6 kg of U-5Fs. The heat capacity is A226 Eq 4.2, which is
    flagged as doubtful (see `design::U5FS_HEAT_CAPACITY`): discontinuous
    at 833 K and 913 K as printed (section 6).
  - Core coolant: 39 kg sodium plus the in-core steel. The node holds the
    mean temperature, with outlet = 2 T_mean - T_in.
  - IHX primary side: 1407 kg.
  - Pool: 291 144 kg (340 m3 at 371 C, less the other nodes), well mixed.
- **Fuel to coolant.** Four series resistances per pin: fuel, sodium bond,
  cladding and film. The film uses Mikityuk at the wire-wrap P/D of 1.282,
  with the Péclet number for one driver.
- **Flow.** A quasi-static balance of:
  - the mean NATDEMO head of the two pumps at the tabulated speeds;
  - the auxiliary EM pump, linear in current (an assumption);
  - buoyancy `g dz (rho_pool - rho_outlet)`;
  - a quadratic loop resistance that puts the rated point on the pump
    curve.

  **The IHX thermal-centre height `dz` is a required input with no
  default**, because no transcribed source gives it.
- **Numerics.**
  - Sub-steps of at most 0.05 s, every node implicit in its own temperature,
    and fuel and core coolant solved together.
  - About 10 us per sub-step natively, measured in the SHRT-45R test, so a
    whole 900 s test takes 0.18 s.
  - Builds for `wasm32-unknown-unknown`.
  - `step` returns an error if any sodium node leaves 400-1000 K, which is
    below sodium's ~1156 K boiling point.

## V&V results

Every test below is in `src/ebr2/`. Each test's doc comment carries its
methodology, the prediction written before the first run, and the result.

### 1. The data equal their transcriptions (`csv_equality`)

**Method.** Re-read every `reference-data/ebr2` CSV at test time. Compare
every cell of the 8 boundary-condition tables exactly, and every scalar
constant that came from a CSV, matched by its row key.

**Result.** 6 of 6 tests pass. The values that came from gh:#1313, #1314 and
#1315 have no CSV, so no test pins them; their doc comments say so.

### 2. The model's steady state against the cited initial conditions

**Method.** Each test's model is built at its own steady state from the cited
boundary conditions and geometry, uncalibrated (`dz` = 2 m). Its flow, core
inlet temperature, fuel temperature rise and pump head are compared with the
cited values.

**Predictions** (written before the first run):

- SHRT-17 flow within ±5 % of 455 kg/s;
- SHRT-45R flow about −7 %;
- core inlet within ±30 K;
- fuel minus coolant 55-85 K against 94 K;
- pump head about 300 kPa.

**Results:**

| quantity | SHRT-17 model | SHRT-17 cited | SHRT-45R model | SHRT-45R cited |
|---|---|---|---|---|
| primary flow (kg/s) | 449.7 | 454.1 (8500 gpm, core) | 447.6 | 479.3 (8971.9 gpm, pumps); 468.3 (8766.6, core) |
| core inlet (K) | 618.13 | 624.71 (664.8 F) | 621.36 | 616.93 (650.8 F) |
| core outlet (K) | 717.35 | (none) | 725.87 | 716.15 (443 C, Run 138B design) |
| core mean coolant (K) | 667.7 | (none) | 673.6 | 666.15 (393 C) |
| fuel minus mean coolant (K) | 68.3 | (none) | 71.2 | 94 (487 - 393 C) |
| mean pump head (kPa) | 289.0 | 292.3 (42.9 / 41.9 psi) | 285.2 | 288.5 (41.84 psi) |
| fuel-to-coolant UA (W/K) | 8.388e5 | | 8.433e5 | |
| IHX UA (W/K), effectiveness | 4.024e6, 0.983 | | 4.012e6, 0.993 | |

The SHRT-17 core mean is the mean of its inlet and outlet. The cited outlet,
mean and fuel temperatures are A228 Table 3.3's Run 138B design values at
60 MW.

**Interpretation.**

- Uncalibrated, the flow, inlet temperature and pump head agree to within a
  few percent and about ±7 K.
- SHRT-45R's flow is 6.6 % below the flow at the pumps and 4.4 % below the
  core flow. The NATDEMO curve with a rated-point loop gives slightly less
  flow at SHRT-45R's lower speeds.
- The fuel temperature rise is under-predicted by 24 %, in the predicted
  direction. The model's fuel is fully dense: the porosity correction, Eq
  4.6, is garbled in the transcription.
- Per pin, the resistances (m K/W) are: fuel 1.39e-3, cladding 1.25e-3,
  bond 2.8e-4 and film 3.1e-4. At Pe = 122 the Nusselt number is 8.97.

### 3. SHRT-17 over 900 s (`shrt17_run_stays_bounded_and_follows_the_table`)

**Method.**

- Tabulated power, the pump-speed and intermediate-side tables, and 0.5 s
  steps.
- `dz` swept over 1, 2 and 4 m.
- At every step the state must be finite and every sodium node must stay
  below 1000 K. The power must equal the table.

**Measured value available as a number:** the normalised power-to-flow ratio
peaked at **2.4 at about 1 min** (TECDOC-1819 Section 3.3, read from the
text, transcribed in gh:#1316, unreviewed).

**Predictions:** bounded for every `dz`; peak P/F between 1 and 3, between 35
and 90 s, rising as `dz` falls; peak outlet 700-900 K at `dz` = 2 m.

**Results** (every prediction held):

| dz | peak P/F (time) | peak core outlet (time) | flow at 300 s (of initial) |
|---|---|---|---|
| 1 m | 2.47 (50.0 s) | 797.0 K (65.5 s) | 1.51 % |
| 2 m | 1.65 (49.5 s) | 753.7 K (59.0 s) | 2.05 % |
| 4 m | 1.16 (48.5 s) | 721.6 K (55.5 s) | 2.76 % |

**Interpretation.**

- **The peak P/F is set mostly by `dz`, which is not in the transcribed
  sources.** The model reaches the measured 2.4 near `dz` = 1 m. That does
  not identify `dz`, and choosing `dz` that way would turn the check into an
  input (root `CLAUDE.md`, "Get the PROCESS right first"). `dz` must come
  from the plant elevations in ARC-226 Chapter 5 (Figs 5.1.1-5.1.2, not yet
  transcribed).
- The model's peak comes about 10 s earlier than "about 1 min". The model
  has no fluid inertia in the loop, so the flow drops with the pump speed.
- No measured temperature trace is transcribed (gh:#1316, D3), so no
  temperature comparison is possible yet.

### 4. SHRT-45R over 900 s (`shrt45r_run_stays_bounded_and_self_limits`)

**Method.**

- Point kinetics with Argonne's VARIANT coefficients and radial expansion on
  the core inlet temperature (both chosen before any run).
- `dz` = 2 m, 0.5 s steps, no scram.
- Six ablations, each changing one choice.

**Measured behaviour available as numbers** (TECDOC-1819 Section 3.3 via
gh:#1316):

- P/F peaked at **2.8 at about 1 min**;
- fission was more than half of the total power throughout;
- natural-circulation flow was more than double SHRT-17's;
- flow rose about 20 % when the auxiliary pump current rose 50 % at 600 s.

**Predictions:**

- bounded;
- net reactivity negative after the first seconds and never above
  +0.1 beta;
- power 5-25 % of initial at 300 s;
- peak P/F between 1.5 and 4;
- natural-circulation flow above SHRT-17's;
- the flow rise across the current step under 20 %.

**Results:**

| case | peak P/F (time) | peak outlet K | P at 300 s, MW | net rho range, pcm | min fission share |
|---|---|---|---|---|---|
| **base** (VARIANT, radial on inlet, dz 2 m) | **3.18 (74.0 s)** | 956.1 | 5.76 (9.6 %) | −217.0 to +15.3 | 0.745 |
| radial on mean coolant | 2.56 (76.0 s) | 890.6 | 4.68 | −279.2 to +23.0 | 0.708 |
| radial on outlet | 2.26 (76.5 s) | 857.7 | 4.24 | −323.3 to +28.3 | 0.687 |
| DIF3D coefficients | 3.15 (74.0 s) | 953.2 | 5.34 | −219.9 to +13.9 | 0.719 |
| auxiliary EM pump off | 3.52 (81.5 s) | 990.5 | 3.99 | −250.8 to +1.3 | 0.593 |
| dz 1 m | 3.38 (78.5 s) | 975.1 | 5.13 | −235.6 to +19.6 | 0.720 |
| dz 4 m | 2.98 (67.0 s) | 935.8 | 6.62 | −197.6 to +8.6 | 0.774 |

Further base-case numbers:

- natural-circulation flow at 300 s is 6.05 % of initial, against SHRT-17's
  2.05 % (2.95 times);
- flow rises 19.5 % from 620 s to 900 s, across the current step;
- the initial decay heat from the history is 3.491 MW (5.8 %).

**Interpretation.**

- The model shuts itself down on its feedback, as EBR-II did.
  - The base peak P/F, 3.18, is 14 % above the measured 2.8 and about 14 s
    late. Across the ablations it spans 2.26-3.52.
  - Fission stays above half of the total power in every case, as measured.
  - The natural-circulation flow is more than double SHRT-17's, as measured.
- One prediction missed. **Net reactivity turns positive**, to at most
  +15.3 pcm (0.022 beta), at about 655 s. The auxiliary pump's current step
  cools the core below the temperature of the feedback references.
- **The radial-expansion temperature is the largest uncited choice.** It
  moves the peak outlet by 98 K. The auxiliary-pump scaling is the second
  largest: switching the pump off moves the peak outlet by +34 K.
- The flow's +19.5 % across the current step matches T1819's "about 20 %".
  **This is not an independent check:** that number was known when the
  linear current scaling was chosen (`model` module doc).

### 5. Numerics

- **Stationarity.** With every input held at its t = 0 value, the nodes
  drift by at most 8.3e-6 K over 20 s, and the reactivity stays at 0.0000
  pcm.
  - The first run drifted by 0.2 K. That exposed two initialisation
    inconsistencies, both since fixed: the heat capacity was evaluated at
    different points in the initialiser and the step, and the pre-test
    history's total powers were fed to the decay model as fission powers.
- **Time-step refinement.** SHRT-45R to 200 s with 0.05 s against 0.01 s
  sub-steps:
  - peak outlet differs by 0.008 % of the rise;
  - power at 100 s differs by 0.006 %.
- **Determinism.** One 0.5 s step equals ten 0.05 s steps bit for bit, and
  two runs are identical.
- **IHX side assignment** (not in the data): UA changes by +0.03 % and the
  core inlet by under 0.01 K. It does not matter.

### 6. U-5Fs property correlations: branch continuity (gh:#1331, 2026-11-10)

Both correlations were read on the rendered pages of A226 (Eq 4.1 p.100,
Eq 4.2 p.101) and IAEA-TECDOC-1819 (Eqs 1-2 p.35), not from the text layer.

**Thermal expansion (`design::U5FS_THERMAL_EXPANSION`,
`u5fs_linear_thermal_strain`).** The two reports print the same correlation
except in the third branch (`T > 913 K`): A226 `0.014 + 2.1e-5 (T - 913)`,
TECDOC-1819 `0.014 + 2.1e-5 (T - 273)`. The question is which is continuous
with the second branch at 913 K.

- *Method.* Evaluate each branch's printed form at the boundaries, 823 K
  and 913 K, and take the step (upper minus lower branch).
- *Tolerance, stated before running.* The printed offsets have two
  significant figures (`0.0089`, `0.014`), so continuity is expected to half
  a unit in the last printed digit: 5e-5 at 823 K, 5e-4 at 913 K.
- *Result* (`design::tests::u5fs_expansion_is_continuous_with_the_arc226_third_branch`,
  as predicted by hand before the run):

  | boundary | lower branch | upper branch | step | within tolerance |
  |---|---|---|---|---|
  | 823 K | `1.62e-5 x 550 = 0.00891` | `0.0089` | -1.0e-5 | yes |
  | 913 K, A226 `(T - 913)` | `0.0089 + 5.7e-5 x 90 = 0.01403` | `0.014` | -3.0e-5 (-0.21 %) | yes |
  | 913 K, TECDOC-1819 `(T - 273)` | `0.01403` | `0.014 + 2.1e-5 x 640 = 0.02744` | +1.341e-2 (+96 %) | no (27x) |

- *Choice.* **A226's `(T - 913)` is the default**: it is continuous to the
  printed precision; TECDOC-1819's form puts a step larger than the whole
  strain accumulated from 273 to 823 K at a point where neither report
  describes one, so its `273` is read as a typesetting error. TECDOC-1819's
  form stays callable (`U5fsExpansionThirdBranch::Tecdoc1819`) so the
  comparison can be re-run.
- *Effect on the model: none.* The correlation is new in this change, and
  `Ebr2Model` does not use it (its axial-expansion feedback is Argonne's
  coefficient), so no V&V number above moves.

**Heat capacity (`design::U5FS_HEAT_CAPACITY`): discontinuous as printed,
known and unchanged.** Both reports print the same nine coefficients and the
form `a + bT + cT^2`. The branches do not meet: at **833 K** the first
branch gives 342.2 J/(kg K) and the second 245.8; at 913 K the second gives
224.9 and the third 362.3 (printed by
`design::tests::u5fs_properties_evaluate_as_transcribed`). Both are the
solid-solid transition temperatures, whose latent heats (A226 Eqs 4.3-4.4,
11.715 and 8.368 kJ/kg) the function does not carry. The fuel node starts at
about 760 K (487 C); a fuel node that crosses 833 K sees the step in its heat
capacity. Whether the SHRT runs cross it was not checked in this change. Not
changed here; a maintainer question.

## What is not done, and what would improve it

- **Validation.** The measured traces are plots only, and the acceptance
  bands are the maintainer's call (gh:#1316; held in gh:#1317).
- **`dz` and the plant elevations.** These need ARC-226 Chapter 5's figures,
  read by eye.
- **Missing physics.**
  - The pool is not stratified, and there is no upper plenum or Z-pipe
    volume.
  - The loop has no fluid inertia.
  - Low-flow resistance is not laminar.
  - There is no flow reversal.
  - Gamma heating is not split out of the fuel's heat.
  - The fuel has no porosity correction.
  - The cladding uses 304L properties for 316.
- **Kinetics data.** Argonne's six-group set and the axial-mesh worths are
  not printed (gh:#1314, GAP).
- **Doubtful transcriptions.** ~~The U-5Fs heat-capacity transcription is
  doubtful (`design::U5FS_HEAT_CAPACITY`).~~ **CORRECTED 2026-11-10
  (gh:#1331):** the transcription matches the rendered A226 p.101 and
  TECDOC-1819 p.35; what stays doubtful is the source correlation itself,
  discontinuous at 833 K and 913 K (section 6). The SHRT-45R intermediate-flow
  boundary condition is 7.2 % below its own test description (`shrt`
  module doc).
