# `bwr_mark1`: input sensitivity, to rank F13's checks (gh:#1044, #1033)

> **Two runs are recorded.** The first is on the 2026-10-29 morning model
> (MATPRO c_p, physics of `07f478da8`). The second, at the end, is on the
> model after F3–F7 and F10 (physics of `e082fc843`). The second supersedes
> the first's ranking; the first is kept because its method and its
> smoothness finding still hold.

> Research, education and V&V only. AI draft, not reviewed. This is a
> **ranking instrument**, not an uncertainty analysis and not a calibration.
> No value is chosen from it (plan rule 1: calibration only as the last step).

## Methodology

**What is computed.** For each of the 73 scalar inputs in
`bwr_mark1::params::Params::INPUTS`, `compare::elasticity` does the following:

- scales that one input by 0.99 and by 1.01 about its default, with every
  other input at its default and every mechanism on;
- runs the recorded Unit 1 sequence to 26 h each time;
- reports the elasticity `(dm/m) / (dp/p)`, by central difference, of three
  metrics: lower-head failure time, in-vessel H2 at 25 h, and peak drywell
  pressure over 0–25 h.

It also flags whether the wetwell vent or the building explosion appears in
either perturbed run (neither happens at the defaults).

**Why this instrument.** The question is which unverified inputs the answer
leans on hardest, so that F13 (#1044) checks those against the sources first.
A local, normalised derivative answers that without anyone choosing a range.

**What it cannot say.** It is local: a step of a few per cent could cross a
threshold that ±1 % does not. A relative step is also not an input's real
uncertainty: 1 % of an Arrhenius activation energy is a large change, while
1 % of a valve area is a small one. An event time is resolved to one 1 s step
(about 3e-4 h), which limits elasticities to about ±0.005 on the lower-head
time.

The tags come from each constant's doc comment, and
`params::tests::every_input_carries_its_documented_tag` checks them: \[P\]
published, \[U\] unverified, \[A\] the model's assumption.

**Run.** `cargo run --release -p sembawang --example bwr_mark1_sensitivity`

## Predictions (written 2026-10-29, before the study was run)

1. **Lower-head time** leans hardest on `lower_head_failure_k` (\[A\], 1500 K;
   it is the failure criterion itself) and `zr_melt_k` (\[U\]; it starts
   relocation). Next come `debris_quench_s` and the decay-heat scale
   (`thermal_power_w`). Elasticity of `lower_head_failure_k`: positive, order
   0.1–1.
2. **H2 at 25 h** leans on `zr_melt_k`, which ends oxidation when a node
   relocates, so the elasticity is positive and large. The Urbanic-Heidrick
   activation energies `uh_low_b_k` and `uh_high_b_k` come next, with large
   **negative** elasticities (`exp(-B/T)`: +1 % of B at 1500 K cuts the rate
   by about 10 %). `oxidation_min_k` is small.
3. **Drywell peak** leans on `drywell_volume_m3` (negative),
   `initial_containment_pa` (positive), and the leak and breach areas
   (`hot_leak_m2`, `breach_m2`).
4. **Zero effect:** `containment_design_pa`, which only the demo draws, and
   probably `h2_explosion` and `h2_flammable`, because no hydrogen reaches the
   building at the defaults.
5. **No vent or explosion appears** at ±1 % of any input.

## Results (2026-10-29)

Run: x86_64, Rust 1.99.1, `--release`, 4 threads, 1 min 45 s for 146 runs.
Baseline, at the physics of `07f478da8`: lower head 5.386 h, H2 122.31 kg,
drywell peak 0.4749 MPa, no vent, no explosion.

### The assumption the instrument rests on, tested: it FAILS for H2

A central difference means something only if the metric is smooth in the
input. Tested by repeating eight inputs at steps of 0.25, 0.5, 1, 2 and 4 %
(table below):

- **Smooth, the elasticity stable across steps:**
  - lower-head time against `lower_head_failure_k` (+0.25 up to 2 %,
    +0.19 at 4 %);
  - lower-head time against `rpv_radius_m` (+2.15 to +2.5 from 0.5 % up);
  - lower-head time against `thermal_power_w` (-1.34 to -1.36 from 0.5 to
    2 %);
  - drywell peak against `drywell_volume_m3` (+0.320 to +0.323 at every
    step);
  - H2 against `uh_low_b_k` (-3.0 to -4.9, always negative).
- **Not smooth:** H2 at 25 h against most inputs. Its elasticity changes
  sign with the step for `bypass_gain_kg_s_pa` (+0.18, -0.75, -1.92, -0.41,
  -0.53), `ic_ua_w_per_k`, `zr_melt_k`, `rpv_radius_m` and
  `thermal_power_w`. `bypass_gain_kg_s_pa` acts only in the first 60 s, yet
  moves H2 at 25 h by about 2 %.

**Interpretation.** H2 at 25 h is not a smooth function of the inputs at
this resolution. The likely cause is that relocation is node by node: 36
nodes, each stopping its oxidation abruptly when it reaches `zr_melt_k`. A
small early shift changes which step a node relocates in, and with it that
node's remaining oxidation. That cause is a hypothesis, **not checked**. So
the H2 column of the full table below is **not a ranking**: an |elasticity|
up to about 2.5 is within the noise. The lower-head and drywell columns are
usable, with the same caveat at steps under 0.5 % (an event time resolves to
1 s).

This is a finding about the model as well as the instrument. A metric that
jumps under a 0.25 % input change cannot be compared with a band to better
than its jump. It bears on F4 (candling and continued oxidation of
relocating Zr, #1035), which would replace the abrupt node-wise stop.

### Predictions against results

1. Lower head: `lower_head_failure_k` is positive and inside the predicted
   0.1–1, at **+0.25**. It is not the largest, though: `rpv_radius_m`
   (**+2.2**, the vessel's water inventory) and `thermal_power_w` (**-1.36**,
   the decay heat) dominate. `zr_melt_k` is small and of unstable sign.
   **Partly wrong.**
2. H2: `uh_low_b_k` is negative and the largest, as predicted. `zr_melt_k`
   is not reliably positive (noise). **Half right; the column is noise
   beyond the activation energy.**
3. Drywell peak: `drywell_volume_m3` is **positive (+0.32), not negative as
   predicted. Wrong sign.** Consistent with the other containment
   elasticities, the peak is governed by the wetwell:
   - `initial_pool_k` +2.05, the pool's vapour pressure;
   - `pool_water_m3` -0.66 and `wetwell_gas_m3` -0.47, the gas space;
   - `initial_containment_pa` +0.60.

   A bigger drywell holds more nitrogen at the same initial pressure, and
   pushing it into a fixed wetwell gas space raises the pressure. That
   explanation fits the pattern but is **not checked** in the code path.
   `breach_m2` and `hot_leak_m2` are near zero for the drywell, contrary to
   the prediction.
4. Zero effect: `containment_design_pa`, `h2_flammable` and `h2_explosion`
   are zero, as predicted. Also zero, not predicted:
   - `uo2_cp` and `zr_cp`, which only the constant-c_p ablation uses;
   - `relief_open_pa` and `relief_close_pa`. The relief path never acts, and
     the safety valves do (`safety_close_pa` is nonzero). ~~Whether that is
     intended is **not checked here**.~~ **Checked 2026-10-29: intended.**
     `Plant::begin` uses the relief set points only while `dc_power` holds,
     and the spring safety valves' set points after DC is lost
     (`mod.rs`, the `open_at`/`close_at` choice), as `params.rs` documents.
     Before the blackout, the isolation condensers keep the vessel below the
     relief set point;
   - `msiv_closure_s`, plausibly because ±0.6 s rounds to the same 1 s step;
   - the vent, flange, vacuum-breaker, building and downcomer inputs, whose
     paths never open.
5. No vent or explosion appears at ±1 % of any input. **As predicted.**

### What F13 should check first (from the usable columns only)

- **Lower-head time:**
  - `rpv_radius_m` \[U\] and `rpv_height_m` \[U\];
  - `thermal_power_w` \[P\], through the decay heat;
  - `initial_pressure_pa` \[P\];
  - `lower_plenum_free` \[A\] and `baf_m` \[A\], the vessel's water
    inventory below the core;
  - `lower_head_failure_k` \[A\], the criterion itself. Its 1500 K has no
    source, and it is what F3 replaces.
- **Drywell peak:** `initial_pool_k` \[A\], `pool_water_m3` \[P\],
  `initial_containment_pa` \[A\], `wetwell_gas_m3` \[P\] and
  `drywell_volume_m3` \[U\].
- **H2:** the Urbanic-Heidrick constants `uh_low_b_k` and `uh_low_a`
  \[U\], which are the only sign-stable entries.

### Step dependence (eight inputs)

```text
== rel 0.0025
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.278 | +0.184 | -0.130 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.322 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | +0.928 | -2.353 | -0.286 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.248 | +0.000 | +0.026 | no |
| `rpv_radius_m` | U | 2.400000e0 | +1.227 | +8.795 | +2.078 | no |
| `thermal_power_w` | P | 1.380000e9 | -2.393 | +5.681 | +1.261 | no |
| `uh_low_b_k` | U | 1.682000e4 | +0.134 | -3.821 | -0.624 | no |
| `zr_melt_k` | U | 2.100000e3 | +0.021 | +0.951 | +0.074 | no |
== rel 0.005
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.062 | -0.749 | -0.064 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.320 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | +0.186 | -0.707 | -0.012 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.248 | +0.000 | +0.028 | no |
| `rpv_radius_m` | U | 2.400000e0 | +2.506 | -0.402 | +0.875 | no |
| `thermal_power_w` | P | 1.380000e9 | -1.341 | -0.344 | +0.430 | no |
| `uh_low_b_k` | U | 1.682000e4 | +0.217 | -2.987 | -0.658 | no |
| `zr_melt_k` | U | 2.100000e3 | +0.098 | -1.795 | -0.162 | no |
== rel 0.01
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.173 | -1.919 | -0.244 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.323 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | -0.018 | +2.349 | +0.263 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.248 | +0.000 | +0.026 | no |
| `rpv_radius_m` | U | 2.400000e0 | +2.269 | +0.467 | +0.753 | no |
| `thermal_power_w` | P | 1.380000e9 | -1.364 | +0.420 | +0.565 | no |
| `uh_low_b_k` | U | 1.682000e4 | +0.449 | -4.943 | -0.686 | no |
| `zr_melt_k` | U | 2.100000e3 | -0.139 | +1.455 | +0.201 | no |
== rel 0.02
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.063 | -0.412 | -0.060 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.323 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | +0.171 | +0.274 | +0.017 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.248 | +0.000 | +0.026 | no |
| `rpv_radius_m` | U | 2.400000e0 | +2.183 | +1.295 | +0.789 | no |
| `thermal_power_w` | P | 1.380000e9 | -1.363 | +0.907 | +0.686 | no |
| `uh_low_b_k` | U | 1.682000e4 | +0.371 | -3.799 | -0.516 | no |
| `zr_melt_k` | U | 2.100000e3 | +0.218 | +0.545 | +0.035 | no |
== rel 0.04
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.041 | -0.527 | -0.062 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.323 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | +0.083 | +0.005 | -0.003 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.191 | +0.000 | +0.020 | no |
| `rpv_radius_m` | U | 2.400000e0 | +2.152 | +0.859 | +0.785 | no |
| `thermal_power_w` | P | 1.380000e9 | -1.115 | -0.476 | +0.514 | no |
| `uh_low_b_k` | U | 1.682000e4 | +0.239 | -3.020 | -0.478 | no |
| `zr_melt_k` | U | 2.100000e3 | -0.055 | +1.746 | +0.192 | no |
```

### Full table, step 1 %

Baseline: Metrics { lower_head_h: Some(5.386111111111111), depressurised_h: Some(5.386111111111111), h2_25h_kg: 122.31485529048929, p_dw_peak_mpa: 0.4749066862486361, vent_h: None, explosion_h: None }
Relative step: 0.01

| Input | Tag | Default | lower-head time | H2 at 25 h | drywell peak | vent or explosion appears |
|---|---|---|---|---|---|---|
| `uh_low_b_k` | U | 1.682000e4 | +0.449 | -4.943 | -0.686 | no |
| `active_length_m` | U | 3.660000e0 | +0.103 | +2.893 | +0.392 | no |
| `uo2_k` | A | 3.000000e0 | -0.217 | +2.665 | +0.321 | no |
| `upper_free` | A | 8.500000e-1 | +0.085 | +2.462 | +0.408 | no |
| `relief_throat_m2` | A | 8.100000e-3 | +0.281 | -2.389 | -0.196 | no |
| `relief_valves` | U | 4.000000e0 | +0.281 | -2.389 | -0.196 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | -0.018 | +2.349 | +0.263 | no |
| `rpv_radius_m` | U | 2.400000e0 | +2.269 | +0.467 | +0.753 | no |
| `uh_low_a` | U | 2.960000e1 | -0.206 | +2.238 | +0.235 | no |
| `rpv_height_m` | U | 2.000000e1 | +0.273 | -2.154 | -0.319 | no |
| `uo2_density` | A | 1.041200e4 | -0.191 | +2.095 | +0.151 | no |
| `channel_wall_m` | U | 2.032000e-3 | +0.206 | -2.056 | -0.222 | no |
| `core_other_area_m2` | A | 1.000000e0 | -0.281 | +2.053 | +0.219 | no |
| `initial_pool_k` | A | 3.031500e2 | -0.034 | -1.780 | +2.052 | no |
| `uh_low_max_k` | U | 1.853000e3 | -0.284 | +2.037 | +0.263 | no |
| `pellet_r_m` | U | 5.200000e-3 | +0.338 | -1.993 | -0.267 | no |
| `zr_density` | P | 6.550000e3 | +0.181 | -1.983 | -0.189 | no |
| `assemblies` | P | 4.000000e2 | -0.211 | +1.982 | +0.262 | no |
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.173 | -1.919 | -0.244 | no |
| `oxidation_min_k` | A | 1.000000e3 | -0.219 | +1.757 | +0.198 | no |
| `hot_leak_gas_k` | A | 1.000000e3 | -0.124 | +1.748 | +0.194 | no |
| `initial_pressure_pa` | P | 7.030000e6 | -0.464 | +1.742 | +0.181 | no |
| `channel_length_m` | U | 4.200000e0 | +0.178 | -1.582 | -0.171 | no |
| `channel_inner_width_m` | U | 1.340000e-1 | +0.235 | -1.555 | -0.144 | no |
| `hot_leak_m2` | A | 3.000000e-4 | +0.034 | -1.486 | -0.117 | no |
| `zr_melt_k` | U | 2.100000e3 | -0.139 | +1.455 | +0.201 | no |
| `thermal_power_w` | P | 1.380000e9 | -1.364 | +0.420 | +0.565 | no |
| `safety_close_pa` | U | 7.900000e6 | +0.018 | -1.219 | -0.082 | no |
| `ic_autostart_pa` | P | 7.231300e6 | +0.049 | -0.961 | -0.230 | no |
| `cosine_extrapolation` | A | 1.300000e0 | +0.036 | -0.678 | -0.109 | no |
| `pool_water_m3` | P | 1.750000e3 | +0.000 | +0.000 | -0.659 | no |
| `initial_containment_pa` | A | 1.050000e5 | +0.000 | +0.000 | +0.600 | no |
| `uh_high_b_k` | U | 1.661000e4 | -0.031 | +0.600 | +0.066 | no |
| `clad_wall_m` | U | 8.100000e-4 | +0.064 | -0.548 | -0.105 | no |
| `lower_plenum_free` | A | 7.500000e-1 | +0.485 | -0.313 | +0.004 | no |
| `wetwell_gas_m3` | P | 2.620000e3 | +0.000 | +0.000 | -0.468 | no |
| `baf_m` | A | 5.000000e0 | +0.428 | -0.379 | +0.123 | no |
| `normal_level_above_taf_m` | U | 4.400000e0 | +0.425 | -0.316 | +0.105 | no |
| `oxidation_heat_j_per_kg_zr` | P | 6.763001e6 | +0.003 | -0.415 | -0.044 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.323 | no |
| `boiling_h` | A | 2.000000e4 | +0.000 | +0.281 | -0.031 | no |
| `clad_outer_r_m` | U | 6.135000e-3 | -0.173 | +0.257 | -0.032 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.248 | +0.000 | +0.026 | no |
| `debris_quench_s` | A | 1.200000e2 | -0.013 | +0.233 | +0.014 | no |
| `rods_per_assembly` | U | 6.200000e1 | +0.103 | -0.228 | -0.179 | no |
| `gap_h` | A | 5.000000e3 | +0.080 | -0.213 | +0.003 | no |
| `injection_temperature_k` | A | 2.931500e2 | +0.000 | +0.000 | +0.203 | no |
| `uh_high_a` | U | 8.790000e1 | -0.018 | -0.187 | -0.007 | no |
| `initial_drywell_k` | A | 3.231500e2 | +0.000 | +0.000 | -0.172 | no |
| `uh_high_min_k` | U | 1.873000e3 | -0.039 | +0.151 | +0.035 | no |
| `ic_shell_water_kg` | A | 1.000000e5 | +0.010 | -0.109 | -0.058 | no |
| `fire_engine_max_kg_s` | A | 4.000000e0 | +0.000 | +0.000 | -0.090 | no |
| `safety_open_pa` | U | 8.300000e6 | -0.005 | -0.057 | +0.044 | no |
| `fire_engine_shutoff_pa` | A | 1.000000e6 | +0.000 | +0.000 | -0.026 | no |
| `vent_submergence_pa` | A | 1.180000e4 | +0.000 | +0.000 | +0.025 | no |
| `vacuum_breaker_pa` | A | 3.400000e3 | +0.000 | +0.000 | +0.003 | no |
| `breach_m2` | A | 1.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `containment_design_pa` | P | 5.313000e5 | +0.000 | +0.000 | +0.000 | no |
| `discharge_coefficient` | A | 6.000000e-1 | +0.000 | +0.000 | +0.000 | no |
| `downcomer_area_m2` | A | 2.000000e1 | +0.000 | +0.000 | +0.000 | no |
| `flange_leak_m2` | A | 5.000000e-4 | +0.000 | +0.000 | +0.000 | no |
| `flange_leak_pa` | A | 7.500000e5 | +0.000 | +0.000 | +0.000 | no |
| `h2_explosion` | A | 8.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `h2_flammable` | P | 4.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `msiv_closure_s` | U | 6.000000e1 | +0.000 | +0.000 | +0.000 | no |
| `reactor_building_m3` | A | 2.600000e4 | +0.000 | +0.000 | +0.000 | no |
| `relief_close_pa` | U | 7.101300e6 | +0.000 | +0.000 | +0.000 | no |
| `relief_open_pa` | U | 7.471300e6 | +0.000 | +0.000 | +0.000 | no |
| `uo2_cp` | A | 3.300000e2 | +0.000 | +0.000 | +0.000 | no |
| `vacuum_breaker_m2` | A | 5.000000e-1 | +0.000 | +0.000 | +0.000 | no |
| `vent_area_m2` | A | 7.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `vent_rupture_disk_pa` | A | 5.490000e5 | +0.000 | +0.000 | +0.000 | no |
| `zr_cp` | A | 3.500000e2 | +0.000 | +0.000 | +0.000 | no |


---

## Second run (2026-10-29, physics of `e082fc843`)

**What changed.** The model in between gained:

- the lower-head wall (F3);
- Zr fusion heat and rod collapse (F4);
- in-core radiation (F6);
- the swollen level (F5);
- ANS-1979 decay heat (F10).

There are now 106 inputs. The method is unchanged: ±1 %, central difference,
every mechanism on. Run time was 2 min 47 s on 4 threads. Baseline: lower
head 4.509 h, H2 286.63 kg, drywell peak 0.5622 MPa, vent 23.73 h.

### The finding: the baseline sits at a regime boundary

**Seven inputs flip the run's regime at ±1 %.** In each case the vent
disappears, and the elasticities jump to ±9–19, about 0.4 h of lower-head time
per percent. **All seven set the vessel's water inventory or its relief
behaviour**, and all seven are unverified or assumed:

| Input | Tag | Lower head | H2 |
|---|---|---|---|
| `rpv_radius_m` | U | +11.9 | −16.6 |
| `normal_level_above_taf_m` | U | +9.8 | −16.9 |
| `safety_close_pa` | U | +9.7 | −17.4 |
| `relief_valves` | U | −9.7 | +17.3 |
| `relief_throat_m2` | A | −9.4 | +18.0 |
| `hot_leak_gas_k` | A | +9.1 | −16.3 |
| `upper_free` | A | +8.9 | −18.7 |

**Interpretation.** These are not derivatives: each is a jump between the
early and late regimes described on gh:#1036. The water inventory and the
relief history decide whether the large debris slug meets a wet plenum. So
**these seven are the first inputs for F13 (#1044) to check against plant
sources**, and the current lower-head time, H2 and vent timing should be read
as one side of a boundary, not as a converged answer.

The bubble-rise scan (gh:#1036) found the regime robust to that input over
0.20–0.35 m/s. It is not robust to the vessel inputs above at 1 %.

Below the seven, the inputs that do not flip the regime:

- `rod_collapse_k` (\[A\], lower head −1.4);
- the decay-heat inputs (Np-239, Q, E; up to about ±0.8);
- `initial_pool_k`.

H2 elasticities below about ±2.5 remain noise, as in the first run.

### Full table, second run

Baseline: Metrics { lower_head_h: Some(4.509166666666666), depressurised_h: Some(4.509166666666666), h2_25h_kg: 286.6262851590136, p_dw_peak_mpa: 0.5622038232954651, vent_h: Some(23.73361111111111), explosion_h: None }
Relative step: 0.01

| Input | Tag | Default | lower-head time | H2 at 25 h | drywell peak | vent or explosion changes |
|---|---|---|---|---|---|---|
| `upper_free` | A | 8.500000e-1 | +8.923 | -18.651 | -2.953 | yes |
| `relief_throat_m2` | A | 8.100000e-3 | -9.425 | +17.986 | +2.867 | yes |
| `safety_close_pa` | U | 7.900000e6 | +9.669 | -17.411 | -2.551 | yes |
| `relief_valves` | U | 4.000000e0 | -9.662 | +17.315 | +2.758 | yes |
| `normal_level_above_taf_m` | U | 4.400000e0 | +9.816 | -16.872 | -2.471 | yes |
| `rpv_radius_m` | U | 2.400000e0 | +11.902 | -16.556 | -2.198 | yes |
| `hot_leak_gas_k` | A | 1.000000e3 | +9.062 | -16.300 | -2.544 | yes |
| `rod_collapse_k` | A | 2.500000e3 | -1.429 | +2.655 | +0.180 | no |
| `np239_decay_per_s` | P | 3.410000e-6 | +0.231 | -2.404 | -0.453 | no |
| `initial_pool_k` | A | 3.031500e2 | +1.124 | -2.292 | +0.869 | no |
| `assemblies` | P | 4.000000e2 | -0.690 | +1.883 | -0.000 | no |
| `uh_high_b_k` | P | 1.661000e4 | +1.669 | -0.530 | +0.004 | no |
| `clad_outer_r_m` | U | 6.135000e-3 | -0.148 | -1.608 | -0.324 | no |
| `e_u239_mev` | P | 4.740000e-1 | +0.129 | +1.525 | +0.467 | no |
| `q_pu239_mev` | A | 2.102000e2 | +0.810 | -1.382 | -0.408 | no |
| `pellet_r_m` | U | 5.200000e-3 | -0.108 | +1.259 | +0.065 | no |
| `active_length_m` | U | 3.660000e0 | -0.062 | +1.198 | +0.177 | no |
| `cosine_extrapolation` | A | 1.300000e0 | -0.336 | +1.189 | +0.123 | no |
| `rpv_height_m` | U | 2.000000e1 | +0.280 | -1.174 | -0.239 | no |
| `baf_m` | A | 5.000000e0 | +0.462 | -1.070 | -0.123 | no |
| `e_np239_mev` | P | 4.190000e-1 | -0.274 | +1.042 | +0.325 | no |
| `q_u238_mev` | A | 1.993000e2 | +0.185 | -1.034 | -0.375 | no |
| `q_u235_mev` | A | 1.990000e2 | +0.465 | -1.027 | -0.434 | no |
| `gap_h` | A | 5.000000e3 | -0.151 | -0.961 | -0.379 | no |
| `ic_shell_water_kg` | A | 1.000000e5 | +0.025 | -0.954 | -0.246 | no |
| `channel_length_m` | U | 4.200000e0 | +0.151 | -0.951 | -0.261 | no |
| `zr_density` | P | 6.550000e3 | -0.074 | -0.941 | -0.296 | no |
| `debris_quench_s` | A | 1.200000e2 | -0.317 | +0.935 | +0.137 | no |
| `radial_exchange` | A | 1.000000e-1 | -0.136 | -0.930 | -0.377 | no |
| `uh_low_max_k` | P | 1.853000e3 | +0.077 | -0.905 | -0.241 | no |
| `oxidation_min_k` | A | 1.000000e3 | -0.139 | +0.863 | +0.235 | no |
| `initial_pressure_pa` | P | 7.030000e6 | +0.249 | +0.849 | +0.283 | no |
| `safety_open_pa` | U | 8.300000e6 | -0.671 | +0.793 | -0.156 | no |
| `lower_head_thickness_m` | A | 2.200000e-1 | -0.034 | +0.773 | +0.306 | no |
| `thermal_power_w` | P | 1.380000e9 | -0.761 | +0.189 | +0.289 | no |
| `uh_low_a` | P | 2.960000e1 | +0.083 | -0.731 | -0.190 | no |
| `uh_high_min_k` | P | 1.873000e3 | -0.194 | +0.710 | +0.131 | no |
| `uo2_density` | A | 1.041200e4 | -0.169 | +0.707 | +0.043 | no |
| `lower_plenum_free` | A | 7.500000e-1 | +0.622 | -0.701 | +0.196 | no |
| `channel_wall_m` | U | 2.032000e-3 | +0.092 | -0.701 | -0.206 | no |
| `axial_exchange` | A | 1.000000e-1 | +0.031 | -0.670 | -0.182 | no |
| `shroud_thickness_m` | A | 5.000000e-2 | -0.006 | -0.633 | -0.195 | no |
| `uh_low_b_k` | P | 1.682000e4 | +0.216 | -0.622 | -0.301 | no |
| `rods_per_assembly` | U | 6.200000e1 | -0.305 | -0.576 | -0.238 | no |
| `pool_water_m3` | P | 1.750000e3 | +0.000 | +0.000 | -0.429 | no |
| `head_failure_wall_k` | A | 1.273150e3 | +0.419 | +0.000 | +0.080 | no |
| `ic_ua_w_per_k` | A | 2.300000e5 | -0.413 | +0.340 | +0.005 | no |
| `wetwell_gas_m3` | P | 2.620000e3 | +0.000 | +0.000 | -0.407 | no |
| `initial_containment_pa` | A | 1.050000e5 | +0.000 | +0.000 | +0.385 | no |
| `oxidation_heat_j_per_kg_zr` | P | 6.763001e6 | -0.314 | +0.384 | +0.073 | no |
| `vent_rupture_disk_pa` | A | 5.490000e5 | +0.000 | +0.000 | +0.363 | no |
| `channel_inner_width_m` | U | 1.340000e-1 | +0.336 | +0.010 | +0.124 | no |
| `fissions_per_fissile_atom` | A | 7.130000e-1 | +0.333 | +0.220 | +0.050 | no |
| `hot_leak_m2` | A | 3.000000e-4 | +0.330 | -0.130 | +0.026 | no |
| `fission_share_u235` | A | 6.472890e-1 | -0.206 | +0.319 | +0.052 | no |
| `uo2_k` | A | 3.000000e0 | +0.286 | +0.121 | +0.141 | no |
| `bubble_rise_m_s` | A | 3.000000e-1 | -0.111 | +0.282 | -0.025 | no |
| `drywell_volume_m3` | U | 3.410000e3 | +0.000 | +0.000 | +0.264 | no |
| `zro2_density` | P | 5.600000e3 | -0.243 | +0.183 | +0.000 | no |
| `max_pool_void` | A | 4.000000e-1 | -0.240 | +0.034 | -0.047 | no |
| `boiling_h` | A | 2.000000e4 | +0.237 | +0.003 | +0.054 | no |
| `bypass_gain_kg_s_pa` | A | 1.000000e-3 | +0.228 | +0.234 | +0.197 | no |
| `core_other_area_m2` | A | 1.000000e0 | -0.102 | +0.220 | +0.028 | no |
| `uh_high_a` | P | 8.790000e1 | +0.216 | +0.143 | +0.099 | no |
| `ic_autostart_pa` | P | 7.231300e6 | -0.129 | +0.197 | +0.107 | no |
| `u239_per_fission` | A | 5.260000e-1 | -0.111 | +0.170 | +0.113 | no |
| `initial_drywell_k` | A | 3.231500e2 | +0.000 | +0.000 | -0.168 | no |
| `injection_temperature_k` | A | 2.931500e2 | +0.000 | +0.000 | +0.164 | no |
| `clad_wall_m` | U | 8.100000e-4 | +0.034 | -0.092 | -0.139 | no |
| `debris_porosity` | A | 4.000000e-1 | +0.009 | -0.136 | -0.033 | no |
| `u239_decay_per_s` | P | 4.910000e-4 | +0.111 | -0.079 | +0.087 | no |
| `operating_time_s` | A | 5.050000e7 | -0.025 | -0.040 | -0.082 | no |
| `fire_engine_max_kg_s` | A | 4.000000e0 | +0.000 | +0.000 | -0.075 | no |
| `debris_to_head_w_m2_k` | A | 1.000000e3 | -0.037 | -0.000 | -0.007 | no |
| `fission_share_pu239` | A | 3.106205e-1 | -0.034 | -0.021 | -0.026 | no |
| `fire_engine_shutoff_pa` | A | 1.000000e6 | +0.000 | +0.000 | -0.033 | no |
| `vent_submergence_pa` | A | 1.180000e4 | +0.000 | +0.000 | +0.022 | no |
| `assembly_pitch_m` | U | 1.524000e-1 | -0.018 | +0.019 | +0.000 | no |
| `clad_breach_k` | A | 2.400000e3 | +0.000 | +0.000 | -0.006 | no |
| `damage_b` | P | 7.000000e-3 | +0.000 | +0.000 | +0.003 | no |
| `vacuum_breaker_pa` | A | 3.400000e3 | +0.000 | +0.000 | -0.002 | no |
| `discharge_coefficient` | A | 6.000000e-1 | +0.000 | +0.000 | -0.000 | no |
| `vacuum_breaker_m2` | A | 5.000000e-1 | +0.000 | +0.000 | -0.000 | no |
| `damage_a` | P | 2.160000e-11 | +0.000 | +0.000 | +0.000 | no |
| `breach_m2` | A | 1.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `containment_design_pa` | P | 5.313000e5 | +0.000 | +0.000 | +0.000 | no |
| `creep_fail_strain` | P | 1.800000e-1 | +0.000 | +0.000 | +0.000 | no |
| `downcomer_area_m2` | A | 2.000000e1 | +0.000 | +0.000 | +0.000 | no |
| `flange_leak_m2` | A | 5.000000e-4 | +0.000 | +0.000 | +0.000 | no |
| `flange_leak_pa` | A | 7.500000e5 | +0.000 | +0.000 | +0.000 | no |
| `h2_explosion` | A | 8.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `h2_flammable` | P | 4.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `lm_c` | P | 7.042000e0 | +0.000 | +0.000 | +0.000 | no |
| `lm_p0` | P | 4.812000e4 | +0.000 | +0.000 | +0.000 | no |
| `lm_slope` | P | 4.725000e3 | +0.000 | +0.000 | +0.000 | no |
| `lower_head_failure_k` | A | 1.500000e3 | +0.000 | +0.000 | +0.000 | no |
| `min_metal_m` | A | 1.000000e-4 | +0.000 | +0.000 | +0.000 | no |
| `msiv_closure_s` | U | 6.000000e1 | +0.000 | +0.000 | +0.000 | no |
| `reactor_building_m3` | A | 2.600000e4 | +0.000 | +0.000 | +0.000 | no |
| `relief_close_pa` | U | 7.101300e6 | +0.000 | +0.000 | +0.000 | no |
| `relief_open_pa` | U | 7.471300e6 | +0.000 | +0.000 | +0.000 | no |
| `rod_collapse_max_k` | A | 3.100000e3 | +0.000 | +0.000 | +0.000 | no |
| `uo2_cp` | A | 3.300000e2 | +0.000 | +0.000 | +0.000 | no |
| `vent_area_m2` | A | 7.000000e-2 | +0.000 | +0.000 | +0.000 | no |
| `zr_cp` | A | 3.500000e2 | +0.000 | +0.000 | +0.000 | no |
| `zr_melt_k` | U | 2.100000e3 | +0.000 | +0.000 | +0.000 | no |
