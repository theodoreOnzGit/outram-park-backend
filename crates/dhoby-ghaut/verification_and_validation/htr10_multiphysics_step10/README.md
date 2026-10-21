# HTR-10 coupled run (workbench Steps 9 and 10): V&V record

**Status: TENTATIVE.** AI-drafted, awaiting human review. Research, education
and V&V only: not for facility operation, licensing or safety decisions.

This record has two parts.

- **Part 1 (2026-10-05/06, gh:#591): the power and k are SOLVED.** Step 10
  computes the power shape and `k` with multigroup diffusion on Step 7's
  neutronics mesh, using Step 8's cross sections. It is Picard-coupled to the
  porous-core thermal-hydraulics. This is the default.
- **Part 2 (2026-10-05, gh:#574): the first build.** It used a PRESCRIBED
  J0 × cosine power shape and lumped feedback. That is now the explicit
  ablation `--prescribed-power`. Its files moved to
  `prescribed_equilibrium_core/`.

## Headline (read this first)

**UPDATED 2026-10-06 (gh:#598): Step 8's bed flux is fixed, and every
result in Part 1 was re-run on the corrected constants.** The previous
headline is kept below, struck through.

1. **The cause was found and fixed in the Monte Carlo tally.** In the
   delta-tracked bed the flux was the real-collision estimator `w/Σ_t`,
   scored through the track-length routine with `1/Σ_t` as a "length". The
   unstructured mesh filter (gh:#492) split that pseudo-segment along the
   ray. A helium collision's `1/Σ_t` is about 5e4 cm, so almost all of its
   score fell outside the mesh and was dropped. The bed lost its helium
   flux. Now the bed is tallied with the tentative-collision estimator
   (`w/Σ_maj` at every virtual and real collision, binned at the site), the
   default in `outram-mc-libs`. Details: `../workbench_steps_7_8/README.md`,
   "gh:#598".
2. **Bed constants fell by a factor 0.649 (fast) and 0.654 (thermal) at
   300 K**, not by 0.61. Bed `Σ_t`: 0.3543 → **0.2299** /cm (fast), 0.3954 →
   **0.2588** /cm (thermal). Step 8's Monte Carlo `k` is **bit-identical**
   (1.14617 ± 0.00678 at 300 K, all four states unchanged): the estimator
   draws no random numbers.
3. **Diffusion `k` against Monte Carlo `k`: +3650 to +5830 pcm (5.8–8.2 σ)**,
   down from +24 700 to +27 000. This is above the 0.61 diagnostic's +1600 to
   +3400, consistent with the measured factor 0.65 being smaller than the
   0.61 correction. The remaining gap is **not attributed**. Candidates
   measured before on the pre-fix data: the 30 cm neutronics mesh (about
   −1300 pcm to 20 cm), 2 groups (−212 pcm to 8). Not re-measured on the
   new data.
4. **Coupled run, 10 MW, on the corrected data** (19 layers, 192 cm).
   Converged in 15 Picard iterations, 17.2 s:
   - `k_eff` 1.10560;
   - node peak/mean power **1.505**, maximum **3.08 W/cm³** (Gao & Shi
     initial core: 2.84);
   - peak kernel **1079.0 °C** (Gao & Shi: about 995 or 1049 °C);
   - vessel outlet 695.9 °C (design 700 °C);
   - the prescribed J0 × cosine ablation on the same core: peak/mean 1.280,
     peak kernel 1035.3 °C. The solved shape is still more peaked.
5. **The `--diagnostic-bed-sigma-scale` arm is superseded.** It was a test
   of the hypothesis on the pre-fix data, and the hypothesis held. Applying
   it to post-fix data would double-correct; the binary now warns.

**Previous headline (2026-10-05, pre-gh:#598 data), superseded:**

1. ~~**The diffusion `k` is 24 700–27 000 pcm above Step 8's own Monte Carlo
   `k`.** That is 35–40 σ, at all four state points.~~
2. ~~**The cause is mostly found, and it is upstream of Step 10.** Step 8's
   pebble-bed cross sections are about 1/0.61 too large. The bed is
   delta-tracked, and its flux is scored with a collision estimator. That
   estimator scores nothing in the helium voids, so the bed flux is low by
   about the void fraction and every bed `Σ = RR/φ` is high by the inverse.~~
   - ~~**A diagnostic test supports this.** Multiplying only the bed's
     constants by the recipe's filling fraction, 0.61, brings `k` to within
     +1600 to +3400 pcm of Monte Carlo (2.5–4.8 σ). It also brings the
     region absorption shares into line.~~
   - ~~**The test cannot fail on the fit.** 0.61 is a geometric input, not a
     fitted number.~~
   - ~~**It is not the default.** The fix belongs in Step 8 (filed as
     gh:#598).~~
3. ~~**Coupled results, as-is Step 8 data.** HTR-10 initial-core recipe, 19
   layers (192 cm), 10 MW. Converged in 15 Picard iterations, 21 s:~~
   - ~~`k_eff` 1.31739;~~
   - ~~node peak/mean power 1.74, maximum 3.57 W/cm³ (Gao & Shi initial core:
     2.84);~~
   - ~~peak kernel 1170 °C (Gao & Shi: 995 or 1049 °C);~~
   - ~~vessel outlet 695.9 °C (design 700 °C).~~
4. ~~**The same run with the diagnostic bed constants** gives:~~
   - ~~`k_eff` 1.08299;~~
   - ~~peak/mean 1.45, maximum 2.97 W/cm³ (+0.13 against 2.84);~~
   - ~~peak kernel 1082 °C.~~
5. ~~**The prescribed J0 × cosine ablation on the same core** gives peak/mean
   1.28 and peak kernel 1035 °C. The solved shape is more peaked than the
   prescribed one in both arms.~~

## Part 1. Solved power (gh:#591)

### What is computed

The code is `crates/dhoby-ghaut/src/bin/dhoby-ghaut/spatial.rs`. It is
driven by `mp_headless.rs` with no window, and by `coupled.rs` and
`coupled_ui.rs` in the workbench. Each Picard iteration:

1. **Neutronics.** It uses `DiffusionNeutronics::new_with_cell_parameters`
   from `outram-foam-appbuilder-lib`, the port of GeN-Foam's
   `diffusionNeutronics` (upstream commit 652b3da).
   - **Mesh and constants.** It runs on Step 7's neutronics `polyMesh`, read
     with the port's reader, with its `cellZones` as the regions. Step 8's
     `nuclearData` is evaluated at each cell's own `TFuel`, by the port's
     polyharmonic-spline interpolation in `log` (linear in ln T, extrapolated
     linearly outside the state points, as upstream does).
   - **Boundary.** The outer boundary is a Marshak vacuum: albedo γ = 1/2
     with the exact face closure.
   - **Warm start.** Each solve starts from the last flux and `k`, through
     the new `DiffusionNeutronics::set_initial_guess`.
   - **Power.** The power density `Σ_g φ_g sigmaPow_g` is scaled to 10 MW.
2. **Neutronics → TH mesh.** This uses Step 7's `MeshMapping` (upstream
   `mapTgtToSrc`, volume weights).
3. **TH mesh → ring grid.** Each of the 40 × 200 nodes takes its volume
   times the mean TH power density over its samples. There are 64 samples
   per node, uniform in volume. Each sample is placed by exact point location
   in the TH mesh, or by the nearest bed-cell centroid when it falls outside
   every bed cell (2.2 % of samples).
   - Power that the neutronics puts in TH cavity cells (3.1 %), or outside
     the TH mesh (7.5 %, stair-stepped regions, gh:#594), is not used.
   - The grid is then rescaled to 10 MW (factor 1.105 at convergence).
4. **Relaxation.** The node power is under-relaxed (ω = 0.5), and the march
   (`porous_core.rs`) does one outer iteration.
5. **TH → neutronics.** Each TH bed cell takes the fuel-pebble
   volume-average temperature of the nodes sampled in it. Cavity cells take
   the inlet helium temperature. Step 7's TH → neutronics map then gives
   `TFuel`. Neutronics cells outside the TH mesh (reflectors, conus, tube)
   are held at the inlet helium temperature, 523 K.
6. **Convergence.** It stops when the ring Δp spread is below 1e-4, the
   largest ΔT is below 0.01 K, the unrelaxed node-power change is below
   1e-4 of the maximum, and |Δk| is below 1e-6.

**The core is the reactor Steps 1–8 built** (`mp_preset::on_built_core`):

- The Step 9 prefill describes Gao & Shi's 197 cm, all-fuel equilibrium
  core. With the solved shape, the march takes the bed of Step 7's domain
  and the recipe's fuel-pebble share.
- This run used the preset recipe with 19 layers, a 192.2 cm bed and 0.57
  fuel pebbles. That is the HTR-10 initial-core composition at roughly full
  height. The file is `step8_full_core_case/recipe_19_layers.md`.
- The preset's 12-layer first-criticality loading (123.6 cm) cannot run at
  10 MW. Kernels pass the 2000 K range of the TRISO correlation, and the
  march stops with an error rather than extrapolate.

### Inputs and their sources

The inputs are those of Part 2, except for the rows below.

| Input | Value | Source |
|---|---|---|
| Bed | 19 lattice layers: 192.2 cm high, 90 cm radius, 0.57 fuel / 0.43 moderator pebbles, filling 0.61 | preset recipe (Li, Yu & Wei 2014 Table 1; Şeker 2003 Table 3 inventory), layers 12 → 19 for this record |
| Cross sections | Step 8, 2 groups (0.625 eV), states 300.15 / 600 / 900 / 1200 K, 2000 × (10 + 20) histories per pass, seed 20260917, 6 threads; delta-tracked bed tallied with the tentative-collision estimator (gh:#598) | `step8_full_core_case/` (`nuclearData`, `mgxs.csv`, `mgxs_set.toml`, `run.log`), re-run 2026-10-06; the pre-gh:#598 set is in `superseded_pre_598/step8_full_core_case/` |
| Meshes | Step 7 defaults: neutronics 30 cm tet-dual (14 041 cells), TH 15 cm (12 888 cells) | `step8_full_core_case/meshes.csv` |
| Thermal power | 10 MW | Step 9 (Li 2014 Table 1) |

Reproduce:

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- \
  --recipe crates/dhoby-ghaut/verification_and_validation/htr10_multiphysics_step10/step8_full_core_case/recipe_19_layers.md \
  --headless-multiphysics \
  --case crates/dhoby-ghaut/verification_and_validation/htr10_multiphysics_step10/step8_full_core_case \
  --out <dir> [--diagnostic-bed-sigma-scale 0.61   # pre-gh:#598 data only, superseded]
```

### Verification tests, with what they measured

Run them with `cargo test --release -p dhoby-ghaut --bin dhoby-ghaut spatial`.

- **`bare_cylinder_k_on_the_neutronics_mesh_matches_the_analytic_value`.**
  - *Setup.* One homogeneous 2-group medium on the Step 7 neutronics mesh
    (40 cm cells), with zero flux on the boundary. It goes through the whole
    hand-off: polyMesh, cellZones, and a `nuclearData` from Step 8's writer.
  - *Reference.* The analytic bare cylinder
    `k = [νΣf1 (Σa2 + D2 B²) + νΣf2 Σ12] / [(Σr1 + D1 B²)(Σa2 + D2 B²)]`.
    `R` is the radius of the circle with the inscribed 32-gon's area.
  - *Result.* **−219 pcm** (gate 1 %).
- **`the_coupled_loop_conserves_power_across_the_maps_and_converges`.**
  Coarse Step 7 meshes, synthetic constants, 3 MW.
  - The neutronics power equals the requested power to 1e-12.
  - The power the N → TH map puts on the TH mesh equals `Σ_s q_s ov_s` to
    1e-9. Here `ov_s` is each neutronics cell's overlap with the TH mesh,
    rebuilt from the weights, and no cell deposits more than its volume.
  - A uniform power density reaches every ring node unchanged (1e-6).
  - No node exceeds the largest TH cell value.
  - The march's energy balance holds to 1e-6. It measured 0.
  - It converged in 14 iterations, and the hot `k` is below the first, cold
    one.
- **`warm_start_reaches_the_same_k_in_fewer_iterations`**
  (`outram-foam-appbuilder-lib`). The same `k` to 1e-7, in 1 outer
  iteration instead of 11.

**A defect of this work, found and fixed before recording.** The first
transfer gave each TH cell's power to the nodes that sampled it, with
nearest-centroid sampling. A cell that no sample reached dumped all its power
into one node.

- **What it did.** It produced nodes at 7.3× the mean, and up to 6.1 W/cm³
  when no neutronics cell exceeded 3.45 W/cm³.
- **How it was found.** By comparing the node peak with the neutronics-cell
  peak. That diagnostic line is now printed on every run.
- **The fix.** The node-sampled overlap above can only average, never
  concentrate, and the test above now gates it.

### Results

All runs used the 30 cm neutronics mesh, 15 cm TH mesh and 40 × 200 ring
grid, unless stated. The files are in `solved/` (since 2026-10-06 the
gh:#598-corrected data; the pre-fix run is in `superseded_pre_598/solved/`)
and `solved_bed_sigma_diagnostic/` (pre-fix data).

#### 1. Diffusion `k` against Step 8's Monte Carlo `k`, every cell at the state temperature

**After gh:#598 (2026-10-06, the current result).** Same mesh, boundary
and command; Step 8 re-run on the fixed estimator. Source:
`solved/isothermal_k.csv`, `solved/console.txt`.

| T [K] | MC `k` (Step 8, bit-identical to before) | diffusion | diff [pcm] | system P/A, diffusion | system P/A, MC |
|---|---|---|---|---|---|
| 300.15 | 1.14617 ± 0.00678 | 1.18637 | **+4020 (5.9 σ)** | 1.19261 | 1.13785 |
| 600 | 1.10254 ± 0.00734 | 1.14537 | +4283 (5.8 σ) | 1.15331 | 1.10692 |
| 900 | 1.06975 ± 0.00627 | 1.10627 | +3652 (5.8 σ) | 1.11487 | 1.06708 |
| 1200 | 1.02742 ± 0.00714 | 1.08570 | +5828 (8.2 σ) | 1.09406 | 1.02682 |

Predicted before the run (2026-10-06): close to the diagnostic arm's +1600
to +3400 pcm. Measured: +3650 to +5830, about 2000 pcm more. The reason
is in the constants: the fix lowered the bed `Σ` by 0.65, not by the 0.61
the diagnostic assumed, so the diagnostic over-corrected. The remaining
+4000 pcm is **not attributed**; on the pre-fix data the 30 → 20 cm mesh
moved `k` by about −1300 pcm and 8 groups by −212 pcm, and neither has been
re-measured here. Region balance at 300 K
(`solved/isothermal_region_balance.csv`): bed absorption share 0.675
(diffusion) against 0.638 (MC), side reflector 0.138 against 0.170; the
diffusion side-reflector flux relative to the bed's is now about 0.78 of
the MC's in both groups (it was 1/2.4–1/2.6 before).

**Before gh:#598 (2026-10-05), superseded:**

| T [K] | MC `k` (Step 8) | diffusion, as-is | diff [pcm] | diffusion, bed Σ × 0.61 (diagnostic) | diff [pcm] |
|---|---|---|---|---|---|
| 300.15 | 1.14617 ± 0.00678 | ~~1.39343~~ | ~~**+24 726 (36.5 σ)**~~ | 1.16629 | +2012 (3.0 σ) |
| 600 | 1.10254 ± 0.00734 | ~~1.35445~~ | ~~+25 190~~ | 1.12289 | +2035 |
| 900 | 1.06975 ± 0.00627 | ~~1.31881~~ | ~~+24 906~~ | 1.08574 | +1599 |
| 1200 | 1.02742 ± 0.00714 | ~~1.29751~~ | ~~+27 009~~ | 1.06166 | +3424 |

Sources: `superseded_pre_598/solved/isothermal_k.csv` and
`solved_bed_sigma_diagnostic/isothermal_k.csv`. The earlier 12-layer core
(123.6 cm), with Step 7/8's committed 2-state data, gave +29 658 and
+30 218 pcm.

**Expected discrepancy sources, stated before measuring, and what each
turned out to be worth.**

| Source | Predicted | Measured |
|---|---|---|
| Neutronics mesh (stair-stepped regions, gh:#594) | hundreds of pcm | 30 → 20 cm cells: −971 pcm at 300 K, −1271 pcm hot |
| Outer boundary | small: 100 cm of reflector | Marshak face / Marshak cell / zero flux: **< 1 pcm** apart |
| Two groups, `D = 1/(3Σt)` (no transport correction) | thousands of pcm, too high | 8 groups at 300 K: **−212 pcm** only |

**On the group-structure prediction.** I predicted that a broad fast group
under-states the migration area, by roughly a factor of 3 against graphite's
Fermi age. I predicted that 8 groups would recover most of the gap. They did
not, so that hypothesis is **refuted**, and recorded as such.

**The source found: the bed constants themselves.**

- **What the constants show.** Step 8's pebble-bed total cross section is
  0.40 /cm in the epithermal groups. The graphite side reflector is 0.38 /cm,
  at 1.76 g/cm³. A bed at 0.61 packing of about 1.75 g/cm³ graphite should
  carry about 0.61 × 0.41 ≈ 0.25 /cm. The fastest group (1.35–20 MeV) shows
  the same: 0.156 /cm tallied, against about 0.09–0.11 expected.
- ~~**Why.**~~ **CORRECTED 2026-10-06 (gh:#598): the mechanism below was
  wrong in its detail, right in its effect.** The real-collision estimator
  is unbiased in helium in expectation; what dropped the helium flux was
  that its `1/Σ_t` score was split along the ray by the unstructured mesh
  filter as if it were a track, so a helium collision (`1/Σ_t ~ 5e4 cm`)
  put almost all of its score outside the mesh. See
  `../workbench_steps_7_8/README.md`, "gh:#598". The struck text:
  ~~The MC transports the bed by delta tracking. In delta-tracked
  regions it scores flux with the collision estimator,
  `crates/outram-mc-libs/src/physics/transport_csg.rs`, "Delta tracking:
  COLLISION estimator". That estimator scores `w/Σt` only at real
  collisions.~~
  - ~~In the helium voids (1 atm in this model) real collisions essentially
    never happen, so 39 % of the bed's volume contributes no flux.~~
  - Reaction rates are unaffected, which is why the Monte Carlo `k` and its
    production/absorption balance stay consistent.
  - The flux denominator of every bed constant is low by about the filling
    fraction. So `Σ` is high by about 1/0.61, `D` low by 0.61, and the
    diffusion area low by about 2.7×.
- **What it does downstream.** It confines neutrons to the bed. The diffusion
  reflector flux is 2.4–2.6× below the MC's in every group, and the bed's
  share of absorption is 0.80 against 0.64. The `k` comes out high. See
  `superseded_pre_598/solved/isothermal_region_balance.csv`.
- **The test** (`--diagnostic-bed-sigma-scale 0.61`, with predicted sign and
  size). Multiply the bed's `Σ` by 0.61 and divide its `D` by 0.61. The gap
  should close by most of its 25 000 pcm.
  - It closed to +1600 to +3400 pcm.
  - Absorption shares now agree: bed 0.658 against 0.640, side reflector
    0.145 against 0.169.
  - The refined-mesh and 8-group checks above leave this conclusion
    unchanged.
- **What remains.** +2000 pcm (3 σ) after the diagnostic is the size the
  remaining approximations (2 groups, P0, no transport correction, coarse
  mesh) are expected to leave. It is not attributed further.
- **The fix belongs in Step 8, not here** (gh:#598). ~~Options: score the
  MGXS flux by track length on a surface-tracked bed, or apply the void
  correction in the MC tally.~~ **DONE 2026-10-06:** the tally estimator
  was fixed in `outram-mc-libs` (no correction factor); results above.
  Step 10 uses Step 8's data as delivered.

#### 2. Coupled run, 10 MW

The first column is the current result (gh:#598-corrected Step 8 data,
`solved/`, 2026-10-06; wall clock on the 16-thread desktop, shared with
another test run). The next two are superseded: the pre-fix data and the
diagnostic arm on it (`superseded_pre_598/solved/`,
`solved_bed_sigma_diagnostic/`).

| Quantity | **gh:#598-corrected Step 8 (current)** | ~~As-is Step 8~~ (pre-fix) | ~~Diagnostic (bed Σ × 0.61)~~ (pre-fix) | Prescribed J0 × cos ablation, same core | Gao & Shi 2002, **initial** core |
|---|---|---|---|---|---|
| Iterations / wall clock | 15 / 17 s | ~~15 / 21 s~~ | ~~15 / 27 s~~ | 10 / ~5 s | — |
| `k_eff` (hot) | **1.10560** | ~~1.31739~~ | ~~1.08299~~ | (lumped) | — |
| Cold → hot reactivity (diffusion, isothermal 300 K → coupled) | −6158 pcm | ~~−4140 pcm~~ | ~~−6595 pcm~~ | −8150 pcm (lumped α) | — |
| Node peak / mean power | **1.505** | ~~**1.743**~~ | ~~**1.450**~~ | 1.280 (input 2.57/2.0) | 2.84 / 2.0 = 1.42 (§4.2) |
| Maximum node power density [W/cm³] | **3.08** | ~~3.57~~ | ~~2.97~~ | 2.62 | 2.84 at R = 0, Z = 90 cm |
| Position of the maximum | ring 0, 88–89 cm below the bed top (node 92) | ~~ring 0, 100 cm below the bed top~~ **CORRECTED 2026-10-06:** ring 0, 85 cm below (node 88 in `superseded_pre_598/solved/multiphysics_fields.csv`) | ~~ring 0, 109 cm below~~ (not re-checked) | mid-height | Z = 90 cm (origin not stated) |
| Peak kernel [°C] | **1079.0** | ~~1169.7~~ | ~~1081.6~~ | 1035.3 | "about 995" (§4.3) / 1049 (§5) |
| Peak fuel-pebble surface [°C] | 1010.2 | ~~1098.6~~ | ~~1005.5~~ | 968.3 | — |
| Peak helium [°C] | 936.3 | ~~1026.0~~ | ~~924.8~~ | 870.5 | 818 (equilibrium, Table 2) |
| Power-weighted mean fuel pebble [°C] | 655.6 | ~~672.1~~ | ~~653.1~~ | 643.0 | 605.7 mean fuel (§4.3) |
| Vessel outlet [°C] | 695.9 | 695.9 | 695.9 | 695.9 | 700 (design) |
| Bed Δp [kPa] | 0.503 | ~~0.501~~ | ~~0.499~~ | 0.497 | 1.3 (bed and bottom reflector) |
| Neutronics cells with `TFuel` above 1200 K (extrapolated) | 21 | ~~64~~ | ~~18~~ | — | — |

**Reading the table.**

- **Only the outlet temperature is an energy balance.** Everything else
  depends on the shape.
- **UPDATED 2026-10-06.** On the corrected data the solved shape gives
  peak/mean 1.505 and 3.08 W/cm³, +0.24 W/cm³ (+8.4 %) against Gao &
  Shi's initial-core 2.84; the peak kernel is 1079 °C. These sit between
  the pre-fix as-is and diagnostic arms, as the measured 0.65 factor
  (between 1 and 0.61) predicts. The paragraph below is the pre-fix
  reading.
- ~~**The as-is shape is too peaked, and the diagnostic arm explains why.**
  The as-is shape gives 3.57 W/cm³ against Gao & Shi's 2.84. With the bed
  constants corrected for the void flux, the solved peak/mean is 1.45 and
  the maximum 2.97 W/cm³, within 0.13 W/cm³ (+4.4 %) of the published
  initial-core value. That is consistent with the bed-flux defect above.~~
- **The temperatures are still high in every arm.** This one is not
  explained here. Gao & Shi's figures may include uncertainty factors; ours
  are nominal.
  - The march still has no conduction or radiation between rings, and an
    adiabatic wall (gh:#592).
  - Part 2's mesh study showed that these alone put 40–50 K on the peaks.
  - The comparison is not like for like: 0.57 fuel share against their 0.5,
    and 192 against 197 cm.
- **The ~~64~~ 21 cells above 1200 K** (corrected data) are extrapolated in ln T beyond the hottest
  state point, as upstream does. A 1500 K state point would remove that.

#### 3. Mesh and iteration convergence (as-is data)

**Pre-gh:#598 data; not re-run on the corrected constants.** The mesh
sensitivity is expected to carry over in size, not in value.

| Change | `k_eff` hot | Node peak/mean | Peak kernel [°C] |
|---|---|---|---|
| Default (neutronics 30 cm, rings 40 × 200) | 1.31739 | 1.743 | 1169.7 |
| Neutronics 20 cm (45 669 cells) | 1.30468 (−1271 pcm) | 1.770 | 1191.8 |
| Rings 20 × 100 | 1.31742 | 1.732 | 1157.1 |

- **The neutronics mesh is not converged.** It moves `k` by about 1300 pcm
  and the peak by 22 K. Part of that is the region map: at 20 cm the bed
  holds 92.7 % of the fission power instead of 90.2 %.
- **The Picard loop converges linearly.** Each iteration roughly halves
  every residual, as the relaxation of 0.5 implies. That took 15 iterations
  to 1e-4 node power and 3e-7 |Δk|. See `superseded_pre_598/solved/console.txt`
  (the corrected run, `solved/console.txt`, also took 15).
- **The warm-started eigenvalue** drops from 57 to 3 outer iterations.

### Images (the drawing rule)

Every image is drawn from the solver's own cells, not from constants.
**Since 2026-10-06 `solved/` holds the gh:#598-corrected run**; the
pre-fix images are in `superseded_pre_598/solved/`. Re-checked on the new
images: power only in the bed and its stair-stepped neighbours, peaking on
the axis near mid-height (2.6–2.8 W/cm³ on the neutronics cells); `TFuel`
250 °C at the bed top rising to about 930–970 °C at the bottom on the axis,
reflectors at 250 °C; the ring-grid power smooth, peaking on the axis about
88 cm below the bed top. The notes below were written on the pre-fix
images.

- **`solved/neutronics_power_xz.png`, `solved/neutronics_power_xy.png`:**
  the computed power on the neutronics mesh.
  - *Checked.* Fission power sits only in the bed and the stair-stepped
    cells around it.
  - The thin ring at 0–0.2 W/cm³ in the side reflector and control-rod band
    is those regions' small `νΣf`. Step 8 tallied it from bed material that
    the 30 cm cells smear in (gh:#594).
  - The x-y slice is not quite axisymmetric, because the tet-dual cells are
    not.
- **`solved/neutronics_power_isothermal_xz.png`:** the cold shape, with no
  feedback.
- **`solved/neutronics_tfuel_xz.png`:** `TFuel` as the cross sections see
  it. *Checked (pre-fix):* 250 °C at the bed top, rising downward to about 1090 °C at
  the bottom on the axis, with the reflectors at 250 °C. One column of
  side-reflector cells beyond r = 90 cm takes bed temperatures; that is
  upstream's normalised weights on partly covered cells.
- **`solved/multiphysics_rz_power_density.png`:** the node power on the
  ring grid. *Checked:* smooth, peaking on the axis slightly above
  mid-height. The first transfer's stripes and spikes are gone.
- **`solved/multiphysics_rz_kernel_temperature.png`** and
  **`multiphysics_rz_helium_temperature.png`.**
- **`solved/ablation_prescribed_rz_power_density.png`:** the prescribed
  ablation on the same core.
- **`solved_bed_sigma_diagnostic/`:** the same images for the diagnostic
  arm.

**Not checked.** No image of the TH-mesh field is drawn; only the ring grid
and the neutronics mesh are. The meshes themselves are as in the Step 7
record (`../workbench_steps_7_8/images/`). This run's 19-layer meshes were
regenerated by the run, and only their CSV is committed.

### What is not done

- ~~**The cause of the +25 000 pcm needs a fix in Step 8's bed flux
  estimator** (gh:#598).~~ **Fixed 2026-10-06.** The remaining +3650 to
  +5830 pcm between diffusion and Monte Carlo is not attributed; the mesh
  and group studies were not re-run on the corrected data. Step 10 runs on
  Step 8's data as delivered.
- **Steady state only.** There is no delayed-neutron data (gh:#595).
- **Feedback uses one temperature per cell** (gh:#595). There is no
  reflector heat balance (gh:#592).
- **SP3 is not used.** The port has `Sp3Neutronics`; with P0 data and no
  transport correction it would add little until gh:#595 is done.
- **The workbench GUI path** was run once, under Xvfb:
  `--recipe step8_full_core_case/recipe_19_layers.md --load-mgxs
  step8_full_core_case --open-step 10 --auto-build --auto-run`.
  - *Result.* Step 7 built its meshes in the window, and Step 10 converged
    in 14.7 s with `k_eff` 1.31736 (the headless run gave 1.31739, on its
    own meshes; pre-gh:#598 data, not re-run). The screenshot is
    `superseded_pre_598/solved/gui_step10.png`: the kernel field
    on the ring grid, the console, the residual / temperature / `k` plots,
    and the computed power on the neutronics mesh.
  - *Not checked.* Responsiveness during the run was not measured. The run
    is on the engine thread and the UI only drains events, as before.

---

## Part 2. The first build: prescribed power shape (2026-10-05, gh:#574)

**Superseded as the default on 2026-10-05 by Part 1.** It is kept as the
record of the `--prescribed-power` ablation. Its console, CSVs and images
are now in `prescribed_equilibrium_core/`. Its element list was the one
before gh:#591: "Power shape: PRESCRIBED", "Reactivity: Lumped". It ran Gao
& Shi's 197 cm, all-fuel equilibrium core from the Step 9 prefill, not the
recipe's core.

### What is computed

Step 10 of the `dhoby-ghaut` workbench runs the Step 9 case. This record uses
the HTR-10 prefill, `crates/dhoby-ghaut/src/bin/dhoby-ghaut/mp_preset.rs`.

- **Thermal-hydraulics.** An r-z multi-channel model of the pebble bed. The bed
  is split into 40 equal-area rings and 200 axial nodes. Each ring is marched
  in the flow direction, which is downward. The march tracks enthalpy exactly,
  using the helium (p, h) flash of `outram-park-fork-coolprop` via
  `tampines::gas_phase::properties`.
- **Pebble surface.** Wakao-Funazkri film coefficient on the local Re, Pr and k
  (`tampines::pebble_bed::cht`).
- **Pebble and fuel temperatures.** Two-zone pebble conduction plus the hottest
  TRISO particle at the pebble centre (`tampines::pebble_bed::Pebble::htr10`),
  on fresh fuel (fluence 0).
- **Friction.** KTA 3102.3 bed friction per node, with the static head
  included.
- **Coupling loop.** Picard iteration over three things:
  - the flow split among rings, until every ring sees the same
    plenum-to-plenum pressure drop;
  - every helium property and temperature;
  - the lumped reactivity feedback, `rho = alpha_iso (T_bed - T_ref)`.
- **Power shape.** **PRESCRIBED**, not solved. It is the bare-cylinder mode
  `J0(2.405 r / R_e) cos(pi z / H_e)`, with one extrapolation length on the
  radius and on both ends. That length is solved so that the peak/mean power
  density equals 2.57 / 2.0. The 2.57 is Gao & Shi's equilibrium-core maximum
  power density (section 4.2) and the 2.0 is their 100 % mean (Table 2). The
  result is 107.2 cm.

#### Inputs and their sources

| Input | Value | Source |
|---|---|---|
| Thermal power | 10 MW | Li, Yu & Wei 2014 Table 1 (`nee_soon::htr10_rmc::table1`) |
| Bed radius, height | 90 cm, 197 cm | same |
| Filling fraction, pebble diameter | 0.61, 6 cm | same; IAEA-TECDOC-1382 |
| Inlet helium, system pressure | 250 °C, 3.0 MPa | IAEA benchmark (`tampines` `htr10_design_point`); Gao & Shi 2002 Table 2 |
| Total flow, bed flow | 4.32 kg/s, 3.77 kg/s | Gao & Shi 2002 Table 2, Table 1 |
| Fuel-pebble fraction | 1.0 (equilibrium core) | Gao & Shi 2002 section 4.1 |
| Isothermal coefficient | -1.4e-4 /K | Chen et al. 2009 Table 1, as transcribed in `htgr_sim_v1` (not in the kovan corpus) |
| Cold reference temperature | 300.15 K | Step 5's data temperature |
| Tolerances | ring Δp spread < 1e-4, max ΔT < 0.01 K | Step 9 defaults |

Gao & Shi (2002) is proprietary. Its values were read from the transcription
in `docs/reactor-scoping/htr10-plant-data.md`, sections 7.4–7.6. That document
records that the column assignment of Table 2 was reconstructed by
monotonicity.

### Pass criteria (verification) and how they are tested

These tests run in `cargo test --release -p dhoby-ghaut --bin dhoby-ghaut`.

- **Energy.** One ring with uniform power. The bed-exit temperature from the
  (p, h) march must match an independent `T_in + P / (m c_p)`, with `c_p` from
  the (T, p) route, to within 0.5 K
  (`exit_temperature_matches_an_independent_cp_balance`). The heat carried by
  the helium equals the power to 1e-9, measured at 1.6e-15.
- **Coupling.** The HTR-10 case converges. At convergence the ring
  pressure-drop spread is below 1e-4, the centre ring carries the least flow,
  and the shape's peak/mean is 1.285 to 1e-9
  (`htr10_case_converges_with_equal_ring_pressure_drops`).
- **Shape.** The J0 and J1 series match Abramowitz & Stegun Table 9.1 to 1e-9,
  and the bare-cylinder peak/mean is 3.638 (`porous_core` tests).
- **Regression.** The summary row is pinned by
  `tests/fixtures/dhoby_ghaut_multiphysics.csv`.

### Results (2026-10-05, 40 × 200 mesh)

The run converged after 10 iterations in about 10 s. The full console is in
`prescribed_equilibrium_core/console.txt`, and the node fields are in `prescribed_equilibrium_core/multiphysics_fields.csv`.

| Quantity | Ours | Gao & Shi 2002, 100 % | Difference |
|---|---|---|---|
| Vessel outlet (bed exit mixed with the bypass) | 695.9 °C | 700 °C | −4.1 K |
| Bed exit, mixed | 760.9 °C | — | — |
| Maximum helium temperature | 869.3 °C | 818 °C | **+51.3 K** |
| Maximum fuel (kernel) temperature | 958.7 °C | 918.7 °C | **+40.0 K** |
| Maximum fuel-pebble surface temperature | 922.8 °C | 876.7 °C | **+46.1 K** |
| Bed pressure drop | 0.51 kPa | 1.3 kPa (bed and bottom reflector) | −0.79 kPa |
| Interstitial velocity at the inlet | 1.39 m/s | 1.5 m/s average at core inlet (section 4.4) | −0.11 m/s |
| Power-weighted mean fuel pebble temperature | 583.1 °C | — | — |
| Lumped reactivity, cold to hot | −7785 pcm | — | — |
| k relative to a cold-critical reference | 0.92777 | — | — |

#### Mesh study

The peaks are **mesh dependent**: no heat crosses between rings, so finer
rings resolve a hotter centreline. The default mesh was chosen by
convergence, not by agreement.

| Rings × axial nodes | Maximum fuel | Maximum surface | Maximum helium |
|---|---|---|---|
| 5 × 40 | 931.5 °C | 896.5 °C | 848.4 °C |
| 10 × 40 | 944.8 °C | 909.0 °C | 860.3 °C |
| 20 × 160 | 955.1 °C | 919.3 °C | 866.3 °C |
| **40 × 200 (default)** | **958.7 °C** | **922.8 °C** | **869.3 °C** |
| 80 × 400 | 960.9 °C | 924.9 °C | 870.8 °C |

The first draft defaulted to 5 × 40. It reads 13 K closer to the published
maximum fuel temperature only because the coarse mesh averages the peak away.
It was replaced.

#### Ablation: power shape

With a uniform power density (`--uniform-power`):

| Quantity | Uniform | Default shape |
|---|---|---|
| Maximum fuel | 849.3 °C | 958.7 °C |
| Maximum surface | 813.2 °C | 922.8 °C |
| Maximum helium | 760.9 °C | 869.3 °C |

The prescribed shape carries about 110 K of the peaks. The peaks therefore
depend mostly on an input that no neutronics solve has checked (gh:#591).
**UPDATED 2026-10-05:** Part 1 now solves the shape. On the recipe's core it
comes out MORE peaked than this prescribed one (1.74 as-is, 1.45 with the
bed-flux diagnostic, against 1.28), so the prescribed shape was not hiding
an over-estimate.

### Interpretation, and the disagreements

1. **The outlet temperature is an energy balance, not evidence.** 10 MW in
   4.32 kg/s of helium gives 695.9 °C. Gao & Shi's 700 °C is the design value.
2. **The maxima are 40–51 K hotter than Gao & Shi**, even though their
   temperatures *include* uncertainty factors and ours are nominal. Their
   factors (section 4.1) are a burnup peaking factor of 1.2, a hot-spot factor
   of 1.05 and a heat-transfer factor of 1.2. So the true nominal gap is larger
   than 40–51 K. Leading suspects, none of which has been tested:
   - **No radial heat transfer between rings.** ZBS effective conductivity
     and radiation would flatten the centreline (gh:#592).
   - **The adiabatic side wall.**
   - **The prescribed shape.** A J0 × cosine with one extrapolation length is
     not the HTR-10 equilibrium-core distribution, which peaks in the upper
     core (section 4.2 gives the initial-core maximum at Z = 90 cm).
   - **Fresh-fuel graphite conductivity** where the equilibrium core is
     irradiated. Irradiation lowers the conductivity, so this would move our
     peak *up*, not down.
3. **The pressure drop is about 40 % of the published value**, which also
   includes the bottom reflector, and we do not model that. Whether the bed
   share alone agrees is not known.
4. **k is not an eigenvalue.** It is `1 / (1 - rho)`, with rho from one
   isothermal coefficient extrapolated from 27 °C to 583 °C. Chen's value was
   evaluated for 212–650 °C, and the TECDOC-1382 values (−7.4e-5 to −9.2e-5 /K
   over 20–250 °C) differ from it by up to a factor of 2. Read the −7785 pcm
   as an order of magnitude, not a result.

### What was not modelled (at the first build)

See the element list on the Step 9 panel and in `mp_preset.rs::elements`:

- the OUTRAM-Foam porous solver (gh:#592);
- conduction and radiation between rings (gh:#592);
- the reflector, plenums and bypass, thermally;
- ~~spatial neutronics, which needs Step 8's cross sections (gh:#591);~~
  **DONE 2026-10-05, Part 1** (default; this record is now its ablation);
- the farrer-park structural side (gh:#593).

### Images (the crate's drawing rule)

These images are drawn from the solver's own node fields on its mesh, not
from the input constants. They are an axial section, mirrored about the axis,
with every 2nd ring line and every 3rd axial line drawn. Each comes with a
legend.

- `prescribed_equilibrium_core/multiphysics_rz_power_density.png`: the prescribed shape. Checked: it is
  symmetric about mid-height, peaks on the axis, and the node averages span 1.36–2.55 W/cm³ (the 2.57 peak is a point value).
- `prescribed_equilibrium_core/multiphysics_rz_helium_temperature.png`: the helium heats downward, and the
  centre runs hottest.
- `prescribed_equilibrium_core/multiphysics_rz_kernel_temperature.png`: the peak is at the bottom of the
  centre ring (z = 192–197 cm from the top of the bed).

Not checked: whether the HTR-10 bed's conus and discharge tube matter. They
are not in this mesh.
