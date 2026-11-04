# HF → LF rung 1: closures from the double-pipe CFD into the TUAS 1-D exchanger (gh:#1178)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1178 (rung 1 of epic gh:#1177). **Date:** 2026-11-03.

The design, the held-out split and the predictions below were written and
saved **before** any design-grid run, CFD or 1-D. Results were added after,
in their own section, without editing the predictions.

This rung has **no calibration**. The CFD-derived closures of step (b) are
read off the CFD fields (not fitted to any 1-D output), and the held-out
points are never used to build them. Rung 3 (gh:#1180) calibrates later.

## Question

1. How far is the TUAS 1-D exchanger, run **uncalibrated** with its own
   textbook correlations, from the conjugate CFD of the same exchanger, per
   design point, in UA, heat duty, effectiveness and pressure drop? And how
   far is each from ε-NTU?
2. With the Nusselt numbers and friction read off the CFD at the training
   points, interpolated to the held-out points, how far is the 1-D model
   from the CFD at the held-out points?

## Reuse: what was searched, and what was picked (before writing code)

- **High fidelity:** `outram_foam_appbuilder_lib::cases::double_pipe`
  (gh:#1156), unchanged in its physics. Its instruments are reused:
  `report` (mixing-cup outlet temperatures, the four heats, ε, the local UA
  `UA_local`, `UA_LMTD`), the steady criteria and the pseudo time step with
  the viscous cap (gh:#1162). **Added there** (it is a CFD instrument, so it
  belongs with the case): an independent annulus flow rate
  (`annulus_flow_rate`, `None` keeps the demo's `C_r = 1`), the per-layer
  profiles `profiles` (heat, area, bulk and interface temperatures, local
  Nu on both sides, the ΔT-weighted mean Nu), and `pressure_drop`.
- **ε-NTU:** `outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness::evaluate`
  (counter-current), as stage 2.
- **Low fidelity:** searched TUAS (`pre_built_components/*`,
  `heat_transfer_correlations/*`), tampines and dwsim-libs for a double-pipe
  or annulus exchanger and for annulus Nusselt correlations (grep for
  `annul`, `double pipe`, `concentric`, `Shah`, `Graetz`, `Hausen`). **No
  annulus Nusselt correlation exists anywhere in the workspace**, and no
  double-pipe component. The closest is TUAS
  `SimpleShellAndTubeHeatExchanger` with `number_of_tubes = 1`, which **is**
  a double pipe: tube fluid | tube wall (a `SolidColumn`, with axial
  conduction) | shell fluid | outer shell, counterflow when the two mass
  flows have opposite signs (its own `heat_transfer_verification` tests run
  it that way). Picked.
- **TUAS's own textbook correlations, as used here (the uncalibrated
  model):** tube side `NusseltCorrelation::PipeGnielinskiGeneric`, which in
  laminar flow is `laminar_nusselt_uniform_heat_flux_developing`
  (Gnielinski's simultaneously developing, constant-flux mean Nu,
  L/D of the tube). Shell side: the same correlation on the shell's
  hydraulic diameter `D_h = D_shell − D_tube,o`, which is what the TUAS
  shell-and-tube does for any shell with a hydraulic diameter (no annulus
  correlation exists to pick instead). Friction: TUAS's pipe loss
  (Churchill, laminar 64/Re) on each side's hydraulic diameter. The outer
  shell is adiabatic (parasitic Nu = 0, no insulation, zero ambient h), as
  the CFD's shell wall.
- **Twin-engine placement:** `outram-park-digital-twin-engine` already
  depended on `tuas_boussinesq_solver` and `outram-park-fork-dwsim-libs`
  (real) and on `outram-foam-appbuilder-lib` (dev). The step needs the CFD
  from its library, so `outram-foam-appbuilder-lib` and `outram-foam-mesh`
  become real dependencies. Neither depends on this crate (checked in their
  `Cargo.toml`s), so the graph stays acyclic. The module
  (`src/multifidelity/double_pipe/`) composes those crates and adds **no
  correlation or governing equation of its own**: it builds the CFD case and
  the TUAS component from one input, marches both to steady, reads their
  outputs, and tabulates closures.

## Design (fixed before any run)

**Geometry and fluids (stage 2's, fixed):** r = 5 / 6 / 10 mm (tube inner,
tube outer, shell); water-like fluid ρ = 1000 kg/m³, c_p = 4000 J/(kg K),
k = 0.6 W/(m K), ν = 1e-6 m²/s (Pr = 6.667); wall ρ = 8000 kg/m³,
c = 500 J/(kg K); hot (tube, +z) inlet 350 K, cold (annulus, −z) inlet
300 K. Indicative property values for a demonstration, not a material data
set.

**Inputs x = (Re_hot, Re_cold, k_wall, L).** Re_hot on the tube diameter
D = 10 mm, Re_cold on the annulus hydraulic diameter D_h = 8 mm, both with
the nominal circular areas: Q = Re ν A / D. The CFD and the 1-D model are
given the same Q.

**Training set (36 points, full factorial, so it is an interpolation
table):** Re_hot ∈ {10, 30, 90} × Re_cold ∈ {5, 15, 45} ×
k_wall ∈ {1, 100} W/(m K) × L ∈ {0.1, 0.3} m.

**Held-out set (8 points, never used to build a closure):**

| id | Re_hot | Re_cold | k_wall | L [m] | why |
|---|---|---|---|---|---|
| H1 | 17.32 | 8.660 | 10 | 0.20 | geometric mid-cell in every coordinate |
| H2 | 17.32 | 25.98 | 10 | 0.20 | |
| H3 | 51.96 | 8.660 | 10 | 0.20 | |
| H4 | 51.96 | 25.98 | 10 | 0.20 | |
| H5 | 17.32 | 25.98 | 1 | 0.15 | on a k_wall node, off-grid elsewhere |
| H6 | 51.96 | 8.660 | 100 | 0.25 | |
| H7 | 30 | 15 | 10 | 0.20 | grid-centre Re, off-grid k_wall and L |
| H8 | 90 | 45 | 10 | 0.15 | the highest-Re edge |

17.32 = √300, 51.96 = √2700, 8.660 = √75, 25.98 = √675: geometric
midpoints, because the closures are interpolated in log coordinates.

**Ranges.** Laminar on both sides (no turbulence model in the CFD); the
CFD's pseudo time step keeps the viscous number ν Δt / h² ≤ 0.5 (gh:#1162)
and Co ≤ 4, as stage 2. The k_wall range spans a wall resistance that is
≈ 15 % of the total (k = 1, polymer-like) to negligible (k = 100,
copper-alloy-like), with 10 the steel-like midpoint; the 1-D model's wall
material takes k_wall from a fixed set of levels {1, 10, 16, 100} W/(m K)
because TUAS custom materials are `fn` pointers (stated in
`one_d.rs`).

**Mesh.** Stage 2's 20k demo cross-section, n = 5, nr = (4, 2, 5) (20 cells
around; 105 + 40 + 100 cells per layer), with its axial cell held at
Δz = L/nz ≈ 2.67 mm: nz = round(375 L), i.e. 38 / 56 / 75 / 94 / 113 layers
at L = 0.1 / 0.15 / 0.2 / 0.25 / 0.3 m (9.3k to 27.7k cells). Why: it is
the default the demo runs (the 18,375-cell grid at L = 0.2), and holding
the cell size rather than the cell count fixed keeps the CFD's
discretisation error the same function of the local physics at every
design point, so it does not change the closures from point to point
through the mesh. Its known error at Re_hot = 30: ε below ε-NTU by
0.8 % from first-order upwind (stage 2). A mesh check is run at the most
demanding corner (Re_hot 90, Re_cold 45, k_wall 100, L 0.1: highest Péclet,
thinnest entrance layers) on n = 7, nr = (6, 3, 7), nz × 1.5; it is a
check, not a dataset point.

**1-D mesh.** TUAS nodes = the CFD's axial layer count (inner nodes
nz − 2, plus TUAS's front and back nodes: equal node lengths L/nz). Why:
both are first-order upwind in z, so equal layer counts give both models
the same axial discretisation error, and the comparison then measures the
closures. ε-NTU, which has no axial discretisation, is reported beside
both.

## Methodology

**CFD (per point):** `DoublePipeExchanger` with `flow_rate` and
`annulus_flow_rate` from x; flows marched to the `laminar_pipe` steady
criterion (< 1e-3), frozen; the three steady energy equations iterated to
a pass change of 1e-8 K. Recorded: the `report` quantities, `profiles`
(local Nu on both sides per layer, written to the profile CSV),
`pressure_drop` on both sides, cell counts, steps, passes, wall time.

**Closures read off the CFD (definitions fixed before the runs):**

- **Mean h per side:** `h_m = Σ_l q_l / Σ_l A_l ΔT_l`, the local heat over
  the local wall-to-bulk difference summed over layers: the uniform h that
  carries the same total heat across the same local differences. As a
  conductance per unit length, `G' = h_m P` (P the mesh interface
  perimeter). For TUAS's circular geometry it is converted to the Nusselt
  number TUAS needs so that the 1-D model gets the same `G'`:
  `Nu_1D,hot = G'_hot / (π k)`, `Nu_1D,cold = G'_cold D_h / (π D_tube,o k)`.
- **Friction per side:** `B = (2 Δp / (ρ U²)) Re`, i.e. `(f L/D + K) Re`
  of the apparent CFD drop, given to TUAS's `SimpleReynoldsPower(0, B, −1)`
  loss, which reproduces that Δp exactly at that Re.
- **Interpolation to an x not in the table:** multilinear in
  (ln Re_hot, ln Re_cold, ln k_wall, ln L) on ln Nu_1D and ln B, over the
  3 × 3 × 2 × 2 training table. Log coordinates because developing laminar
  Nu and f·Re are close to power laws in the Graetz number.

**1-D runs (per point):** TUAS `SimpleShellAndTubeHeatExchanger`, one tube,
counterflow, constant-temperature inlets, marched in time to steady (a
step changing no node by more than 1e-9 K). Two runs: (a) uncalibrated,
θ nominal; (b) the closures (interpolated at held-out points, the table
value at training points). The 1-D UA is `N × 1/(1/G_tube + 1/G_shell)`
from TUAS's own nodal conductances (they include the two wall halves).

**Disagreement measures:** relative errors in UA, heat duty Q,
effectiveness ε, and Δp on each side, 1-D against CFD; ε against ε-NTU at
each model's own UA. **Which numbers to quote:** Q and UA, per point and as
the range over the grid. ε is reported but saturates where NTU is large
(it then hides UA errors), and `UA_LMTD` stays a consistency check, as in
stage 2.

## Predictions (written before the runs)

| # | quantity | prediction | why |
|---|---|---|---|
| P1 | CFD annulus Nu_1D,cold vs TUAS's pipe correlation on D_h | CFD **higher at every point**, by +15 % to +45 % | the fully developed inner-wall-heated annulus at r* = 0.6 has Nu_ii ≈ 5.6–6.0 (Kays & Crawford / Shah & London tables, interpolated between r* = 0.5 and 1), against the pipe's 4.36 / 3.66 on the same D_h; the entrance raises both, so the ratio shrinks somewhat at high Graetz number |
| P2 | CFD tube Nu_1D,hot vs TUAS's Gnielinski (constant-flux, developing) | within −15 % to +5 %, mostly CFD lower | the conjugate counterflow wall is between constant flux (C_r ≈ 1) and constant temperature (C_r far from 1); constant T gives Nu ≈ 16 % below constant flux when developed |
| P3 | uncalibrated 1-D UA vs CFD UA_local | 1-D **lower at every point**, by 3 % to 20 % | P1 dominates (annulus resistance ≈ 30–40 % of the total); the wall is the same physics in both |
| P4 | uncalibrated 1-D Q vs CFD Q | 1-D lower at every point, \|ΔQ/Q\| ≤ \|ΔUA/UA\| | ε saturates in NTU, so the duty error is the UA error damped |
| P5 | tube Δp, 1-D (64/Re) vs CFD | CFD higher by +1 % to +20 %, growing with Re/L | the CFD's uniform inlet adds Shah's entrance increment K(∞) ≈ 1.25 dynamic heads: 1.25 Re / (64 L/D) is 0.7 % (Re 10, L 0.3) to 18 % (Re 90, L 0.1); plus the grid's f·Re error (~1–3 %) |
| P6 | annulus Δp, 1-D (pipe 64/Re on D_h) vs CFD | CFD higher by +45 % to +70 % | the annulus at r* = 0.6 has f·Re ≈ 95.6 (Shah & London) against 64, ratio 1.49, plus the entrance |
| P7 | CFD ε vs ε-NTU(UA_local) | CFD lower, by 0 to 3 % | stage 2's first-order axial error (−0.8 % at Δz = 2.67 mm, NTU 1.3), larger at larger NTU, plus axial wall conduction at k_wall = 100 |
| P8 | 1-D ε vs ε-NTU(1-D's own UA) | 1-D lower, by 0 to 3 % | the same first-order layer error (same layer count) |
| P9 | 1-D with the CFD closures, **training** points: UA vs CFD | \|ΔUA/UA\| < 3 %, \|ΔQ/Q\| < 2 % | the closures carry the CFD's mean h exactly; what remains is the uniform-h assumption against the CFD's varying h(z), and the circular vs polygonal wall (~0.4 %) |
| P10 | interpolated closures at the **held-out** points: Nu_1D on each side vs that point's CFD | \|ΔNu/Nu\| < 5 % per side | log-multilinear on a 3-level Re grid with ratio 3; Nu is smooth in Gz |
| P11 | 1-D with interpolated closures, held-out: Q vs CFD | \|ΔQ/Q\| < 3 %, an improvement on (a) at every held-out point | P9 + P10 |
| P12 | Δp closures, held-out | \|ΔΔp/Δp\| < 3 % per side | f·Re + K is smoother in Re than Nu |

## Results (2026-11-03 / 04)

Run with
`cargo run --release -p outram-park-digital-twin-engine --example hf_lf_double_pipe_dataset -- --threads 3`
on the shared 4-CPU host: the 44 CFD points took ≈ 5 min of wall time on 3
threads (1.8 s to 93 s per point; flow steady in 18–23 steps, energy in
69–1043 passes), the 88 TUAS runs ≈ 14 min on one thread (the example now
runs them on 3). Files, in `hf_lf_double_pipe_gh1178/`:

| file | contents |
|---|---|
| `cfd_dataset.csv` | 44 rows: x, split, the CFD outputs (UA_local, Q, ε, outlet T, Δp, the closures Nu_1D, the mesh-convention Nu, G′, ε-NTU at UA_local, cells, steps, passes, wall time), with the provenance header |
| `cfd_profiles.csv` | per point and axial layer: interface heats and areas, bulk and interface temperatures, local Nu(z) on both sides (3 344 rows) |
| `one_d_results.csv` | per point: the CFD's UA, Q, ε, ε-NTU; the 1-D model's (a) and (b) UA, Q, ε, ε-NTU, Δp, Nu; the closures used and the point's own |

**Reproducibility.** The CFD is deterministic: T33 re-run gives its
committed row character for character (test
`the_committed_dataset_regenerates_bit_for_bit`); the held-out 1-D runs
regenerate their committed numbers to ≤ 1e-12
(`the_held_out_one_d_runs_regenerate_the_record`). Energy closes in every
CFD run (Q_hot = Q_cold to ≤ 2.0e-8 relative).

**Mesh check** (T35's x: Re_hot 90, Re_cold 45, k_wall 100, L 0.1; default
grid 9 310 cells, 38 layers, against n = 7, nr = (6, 3, 7), 56 layers,
27 832 cells): UA −1.05 %, Q −0.66 %, Nu_1D,hot −1.26 %, Nu_1D,cold −0.76 %,
Δp_hot −0.70 %, **Δp_cold +2.28 %** on refinement. So at the most demanding
point the dataset's heat-transfer numbers carry about 1 % of mesh error, and
its annulus Δp is low by ≥ 2 %. Not refined further.

### (a) The uncalibrated 1-D model against the CFD and ε-NTU

Ranges over the 44 points (per point: `one_d_results.csv`, and the table
the example prints). Relative differences, 1-D against CFD unless stated.

| # | quantity | prediction | measured | |
|---|---|---|---|---|
| P1 | CFD Nu_cold / TUAS − 1 | +15 to +45 %, positive everywhere | **+25.7 % (T33) to +58.1 % (T12)**, positive at 44/44 | sign met; **range missed at the top** (4 points above +45 %, T06, T08, T10, T12: Re_hot 10, L = 0.3 m, where TUAS's correlation is near its 4.35 floor while the annulus's developed value is higher) |
| P2 | CFD Nu_hot / TUAS − 1 | −15 to +5 %, mostly CFD lower | −12.3 % (T33) to **+7.8 %** (T28); CFD lower at 31/44 | **missed at the top** (CFD above TUAS by > 5 % at 3 points, T12, T26, T28, all L = 0.3 m with NTU 3.6–5.4. Not investigated further) |
| P3 | UA, 1-D vs CFD UA_local | 1-D lower everywhere, by 3 to 20 % | **−1.1 % (T21) to −21.4 % (T28)**, lower at 44/44 | sign met; range missed at both ends (6 points under 3 %: T09, T11, T21, T23, T33, T35, all Re_cold 45 with L 0.1; 1 over 20 %, T28) |
| P4 | Q, 1-D vs CFD | 1-D lower everywhere, \|ΔQ\| ≤ \|ΔUA\| | **−0.9 % (T21) to −6.7 % (T27)**, lower at 44/44; \|ΔQ\| ≤ \|ΔUA\| at 43/44 | sign met; the inequality **missed at T11** (Q −3.3 %, UA −2.0 %): there 2.5 % of the CFD's Q enters by conduction through the hot inlet face (below), which the 1-D model has no counterpart for |
| P5 | tube Δp, CFD / TUAS − 1 | +1 to +20 %, growing with Re/L | **+3.8 % (T02) to +23.5 % (T25)**, growing with Re/L | sign and trend met; **missed at the top** (Re 90, L 0.1: +23.5 %; the K(∞) = 1.25 estimate gives +17.6 %) |
| P6 | annulus Δp, CFD / TUAS − 1 | +45 to +70 % | **+43.7 % (T01) to +47.2 % (T09)** | **missed at the bottom.** The CFD's developed annulus f·Re is ≈ 64 × 1.437 = 92 against Shah & London's ≈ 95.6 at r* = 0.6, i.e. ≈ 4 % low; the mesh check moved Δp_cold +2.3 % on refinement, so most of this is the annulus's 5 radial cells. P6's prediction assumed a converged CFD |
| P7 | CFD ε / ε-NTU(UA_local) − 1 | 0 to −3 %, CFD lower | **−3.9 % (T15) to +2.0 % (T12)**; lower at 36/44 | **missed both ways.** Below −3 %: T15, T03 (k_wall 100, L 0.1): axial wall conduction, λ = k_w A_w/(L C_min) = 0.069 / 0.110, which ε-NTU does not carry. Above 0: 8 points, six at Re_hot 10 (T06, T08–T12) and T26, T28 (+0.3, +0.2 %). At Re_hot 10 the CFD's Q includes heat conducted in through the fixed-temperature hot inlet (0.9 to 2.5 % of Q at Re_hot 10, Pe = 67), and Q / (C_min ΔT) can exceed the ideal exchanger's and even 1 (T08 1.0012, T12 1.0136: the Dirichlet inlet acts as a heat source by conduction). Stage 2's ε definition (Q_hot including conduction) is kept; an enthalpy-only ε would not exceed 1 |
| P8 | 1-D ε / ε-NTU(own UA) − 1 | 0 to −3 % | **−0.15 % (T10) to −4.8 % (T03)**, lower at 44/44 | sign met; **missed below −3 %** at T03, T15 for the same reason as P7: TUAS's `SolidColumn` conducts axially too |

**Which numbers to quote:** Q, −0.9 to −6.7 % (TUAS low everywhere), and
UA, −1.1 to −21.4 %. The single largest uncalibrated error is **TUAS's
annulus Nusselt number, 26–58 % low**, because no annulus correlation exists
in TUAS and the pipe correlation on D_h stands in for it; the tube side is
within −12 to +8 %. Pressure drop: tube 4–24 % low (no entrance term),
annulus 30–32 % low (pipe 64/Re on D_h for an annulus whose f·Re is ≈ 92–96).

### (b) The 1-D model with the CFD closures

| # | quantity | prediction | measured | |
|---|---|---|---|---|
| P9 | training: UA vs CFD | \|ΔUA\| < 3 % | **−6.3 % (T27) to +12.7 % (T12)** | **missed.** With each side's mean h carried exactly, the 1-D's series UA and the CFD's UA_local are no longer the same quantity when h(z) varies strongly (the worst points have NTU 4.5–5.4 or the shortest exchangers). Not separated further |
| P9 | training: Q vs CFD | \|ΔQ\| < 2 % | **−2.2 % (T27) to +1.9 % (T23)** | **missed by 0.2 %** at T27 (Re_hot 90, Re_cold 5, k_wall 100, L 0.1: the strongest axial wall conduction and the most developing tube) |
| — | training: Δp vs CFD | exact by construction | 0 (to round-off) | the `SimpleReynoldsPower(0, B, −1)` closure reproduces its own Δp, a check of the plumbing only |
| P10 | held-out: interpolated Nu vs the point's own | \|ΔNu\| < 5 % per side | tube **+1.7 to +4.9 %**, annulus **+0.1 to +3.6 %** | met, narrowly on the tube side (H3, +4.9 %); the interpolation is biased high on both sides at every held-out point |
| P11 | held-out: Q vs CFD | \|ΔQ\| < 3 %, better than (a) everywhere | **+0.4 % (H6) to +2.5 % (H4)**; better than (a) at 8/8 (by 0.4 to 4.8 points) | met |
| P12 | held-out: Δp vs CFD | \|ΔΔp\| < 3 % per side | tube +0.5 to +1.7 %, annulus +0.04 to +0.27 % | met |

**Which numbers to quote:** on the held-out points, the closures take the
1-D duty error from **−2.4 … −5.3 %** (uncalibrated) to **+0.4 … +2.5 %**,
and Δp from −5 … −14 % (tube) and −31 % (annulus) to within 1.7 %. The
residual is mostly the interpolation's high bias on the tube-side Nu
(+1.7 to +4.9 %). UA is not the number to quote for (b) (P9).

### What this does not show

- The CFD itself is not validated here: its V&V is stage 2's (ε-NTU at
  Re_hot 30 only, −0.8 % at this Δz) plus this rung's mesh check at one
  point. The dataset carries ≈ 1 % mesh error in the heat transfer and
  ≥ 2 % in the annulus Δp at the hardest point.
- No experimental data; the fluids are indicative. Laminar only.
- The closures are interpolated within the training box only (no
  extrapolation; `ClosureTable::at` refuses it).
- The geometry is stage 2's cross-section extruded to 0.1–0.3 m, so stage
  2's mesh images (`crates/outram-foam-appbuilder-lib/verification_and_validation/cht_double_pipe_gh1156/`)
  show it; no new geometry was built, and no new images were drawn.

## The rung-3 interface (gh:#1180; the `raffles::fidelity` traits are gh:#1194)

**Since 2026-11-04** the pair is also available through `raffles::fidelity`
(gh:#1194): `multifidelity::double_pipe::fidelity::{TuasOneD, committed_dataset}`;
record `crates/raffles/verification_and_validation/fidelity_interface_gh1194.md`.

- **x:** `multifidelity::double_pipe::DoublePipeInput { re_hot: Ratio,
  re_cold: Ratio, k_wall: ThermalConductivity, length: Length }`.
- **CFD outputs:** `DoublePipeOutput { ua: ThermalConductance, q: Power,
  effectiveness: Ratio, t_hot_out, t_cold_out: ThermodynamicTemperature,
  dp_hot, dp_cold: Pressure, nu_hot, nu_cold: Ratio }`.
- **The dataset:** `dataset::load_committed()` → 44 `DatasetRow { id,
  split, input, output, extra }`, compiled in (`COMMITTED_CSV`); the split
  (`design::Split::{Training, HeldOut}`) was fixed before any run and is in
  every row; the provenance is `dataset::PROVENANCE` (also the CSV's `#`
  header). The CSV is SI with the unit in every column header.
- **The 1-D model:** `one_d::evaluate(&x, &θ) -> DoublePipeOutput` (and
  `one_d::try_evaluate` returning `Result`), θ =
  `Theta { nu_mult_hot: Ratio, nu_mult_cold: Ratio, wall_resistance:
  AreaSpecificThermalResistance (m² K/W, on the tube's outer area),
  friction_mult_hot: Ratio, friction_mult_cold: Ratio }`, nominal
  `Theta::nominal()` = (1, 1, 0, 1, 1), which is the uncalibrated model
  above. ≈ 0.4–30 s per call (the explicit TUAS march at Courant 0.5).
- **Limits a calibration must respect:** `k_wall` must be one of
  `one_d::WALL_K_LEVELS` {1, 10, 16, 100} W/(m K) (TUAS materials are `fn`
  pointers); every design point satisfies this.

## Follow-ups

- An annulus Nusselt (and friction) correlation for TUAS, the largest
  uncalibrated error (P1, P6): gh:#1195.
- Read while choosing the 1-D model, not exercised here: TUAS's uninsulated
  shell-and-tube links the outer shell to ambient through the shell-fluid
  conductance (gh:#1196; no effect on this rung, whose parasitic Nu and
  ambient h are 0).
- The annulus Δp's mesh error (P6, mesh check).
