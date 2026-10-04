# Coverage: every module, its lesson, its check

> **Research, education and V&V only** ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> Surveyed 2026-10-04 at `develop` `5e802df3a4`: 25 641 lines in
> `crates/boon-lay/src`, about 509 public items.

Every public module of the crate has a row. A module with no lesson would be
a visible row with "none"; at this survey there is none without a page, but
several have **no V&V**, and those rows say so. "Code walk" names the page
section whose `code-walk` block reaches the module. "Rung" is the demo's
`?rung=` name.

## The random-walk side (`lagrangian_decay_simulator`, `lagrangian_transmutation_and_fission_simulator`)

| Module | Core lesson | Extended | Code walk | V&V record | Gaps |
|---|---|---|---|---|---|
| `…::single_particle_simulator::constructive_solid_geometry` (`TrisoCell`, `Region`, `Sphere`, `TrisoRegion`) | [1 `triso`](../../tutorials/triso-atops/triso.html) | [legacy](./legacy-gaussian.md) | rung 1, steps 1–2 | `triso_cell_slice` geometry check (2026-10-04) | — |
| `…::monte_carlo_single_radionuclide_decay_simulator` (`SingleNuclideSimulatorMC`, postprocessing) | [2 `decay`](../../tutorials/triso-atops/decay.html) | — | rung 2, steps 1 and 4 | `stochastic_half_life_calculator` (re-measured 2026-10-04), `decay_chain_th232` | branching frequencies never sampled |
| `…::stochastic_decay_chain` (`StochasticDecayChain`, iterators) | [2 `decay`](../../tutorials/triso-atops/decay.html) | — | rung 2, step 2 | `decay_chain_th232` | gh:#538 |
| `…::central_limit_theorem` (variance formulas, Gaussian sampler, `OoRng64`) | [3 `walk`](../../tutorials/triso-atops/walk.html) | [legacy](./legacy-gaussian.md) | rung 3, step 1 | none (diagnosis doc only) | no tests of its own |
| `…::single_particle_simulator` (`SingleParticleDiffusionSimulatorMC`), `movement_within_triso_particle`, `interaction_with_decaying_nuclide_simulator`, `cached_normals` | [3 `walk`](../../tutorials/triso-atops/walk.html) (the failure case) | [legacy](./legacy-gaussian.md) | rung 3, step 1 | none | one unit test; no V&V |
| `…::isotropic_scattering` (`Vec3`) | — | [legacy](./legacy-gaussian.md) | — | none | — |
| `…::chatgpt_5_*` (four), `triso_particle_widget` | — | [legacy](./legacy-gaussian.md) | — | test-only code | not in a library build |
| `…::first_passage::sphere_fpt` | [3 `walk`](../../tutorials/triso-atops/walk.html) | [Getting out](./release.md) | rung 3, step 3 | four unit tests (pass 2026-10-04) | — |
| `…::first_passage::walk_on_spheres` (`WoSWalker`, `WalkParams`, `shell_bounds`) | [3 `walk`](../../tutorials/triso-atops/walk.html), [4 `layers`](../../tutorials/triso-atops/layers.html) | [Getting out](./release.md) | rung 3 step 3; rung 4 step 2 | [CRP-6 Case 1](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/crp6_case1_kernel_release_vs_crank.md) (re-measured) | no multilayer record; gh:#537 |
| `…::single_particle_simulator::release_fraction_crp_6_case_1a_1b`, `release_fraction_analytical_solution` | [3 `walk`](../../tutorials/triso-atops/walk.html) | — | rung 3, steps 3–4 | CRP-6 Case 1 | gh:#537 (swallowed literature check) |
| `…::first_passage::interface` | [4 `layers`](../../tutorials/triso-atops/layers.html) | [Getting out](./release.md) | rung 4, step 2 | [interface equilibrium](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/interface_uniform_equilibrium_density.md) (re-measured) | seed spread unmeasured |
| `…::temperature_dependent_collisions` (+ `diffusion_coeffs`: Ag, Cs, Sr, Kr) | [4 `layers`](../../tutorials/triso-atops/layers.html) | [legacy](./legacy-gaussian.md) | rung 4, step 1 | transcription tests vs Jiang 2023 curves; `layer_diffusion_table` | gh:#541 |
| `…::first_passage::depletion`; `lagrangian_transmutation_and_fission_simulator` | named in [2](../../tutorials/triso-atops/decay.html) | [transmutation](./transmutation.md) | transmutation page | unit tests only | gh:#539 |
| `…::first_passage::ensemble`, `…::first_passage::live` | — | [compute backends](./compute-backends.md) | compute page | unit tests (reproducibility, agreement with Crank) | — |

## Decay data

| Module | Core lesson | Extended | Code walk | V&V record | Gaps |
|---|---|---|---|---|---|
| `nuclide_reaction_and_decay_data` (`DecayLibrary`, `NuclideReactionAndDecayData`, `DecayType`, `get_decay_info`, parsing) | [2 `decay`](../../tutorials/triso-atops/decay.html) | [decay data](./decay-data.md) | rung 2, step 3; decay-data page | about 70 half-life parse tests | gh:#538 (`get_decay_branch_info` is `todo!()`) |
| `decay_xml_info_serde` | — | [decay data](./decay-data.md) | decay-data page | re-export only | — |
| `prelude` | — | [architecture](./architecture.md) | — | re-export only | — |

## boon-lay fuel failure (`fuel_failure`)

| Module | Core lesson | Extended | Code walk | V&V record | Gaps |
|---|---|---|---|---|---|
| `geometry` (`SicLayer`) | [1](../../tutorials/triso-atops/triso.html), [5](../../tutorials/triso-atops/failure.html) | [fuel failure](./fuel-failure.md) | rung 1 step 3; rung 5 step 2 | stress-route tests | — |
| `pressure`, `booth`, `oxygen`, `molar_volume`, `diffusion` | [5 `failure`](../../tutorials/triso-atops/failure.html) step 1 | [fuel failure](./fuel-failure.md) | rung 5, step 1 | module docs + [units register](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md) (Figs 1, 2, 3) | UO₂ `D_S` disagrees with Fig. 2 (recorded) |
| `stress`, `corrosion` | 5, step 2 | [fuel failure](./fuel-failure.md) | rung 5, step 2 | Fig. 4 (483 points) | — |
| `weibull`, `strength`, `grain_boundary` | 5, step 3 | [fuel failure](./fuel-failure.md) | rung 5, step 3 | Table 1 16/16, Table 2 | — |
| `decomposition` | 5, step 4 | [fuel failure](./fuel-failure.md) | rung 5, step 4 | **none in the report**; (14a) excluded by Fig. 6 | unverified |
| `history` (`AccidentHistory`), `mod` (`total_failure_fraction`) | 5, step 5 | [fuel failure](./fuel-failure.md) | rung 5, step 5 | step-size independence; Figs 6, 7 | gh:#295 |
| `htr10` | 5, step 6 | [HTR-10](./htr10.md) | — | sweeps re-measured 2026-10-04 | extrapolation; gh:#296 |
| `htr10::qualification` | 5, step 6 | [HTR-10](./htr10.md) | — | burnup ordering (pass) | gh:#383 |

## Chemistry (`chemistry`)

| Module | Core lesson | Code walk | V&V record | Gaps |
|---|---|---|---|---|
| `graphite_air` | [6 `chemistry`](../../tutorials/triso-atops/chemistry.html) step 1 | rung 6, step 1 | transcription test | gh:#457, #441, #442 |
| `graphite_steam` | 6, step 2 | rung 6, step 2 | transcription test; loose band vs measured points | gh:#441 |
| `kernel_hydrolysis` | 6, step 3 | rung 6, step 3 | transcription test | gh:#418, #444 |

## The TRISO-ATOPS port (`triso_atops_fork`)

| Module | Core lesson | Extended | Code walk | V&V record | Gaps |
|---|---|---|---|---|---|
| `nuclide_model` (+ `nuclide_database`) | [4](../../tutorials/triso-atops/layers.html) step 3 | [running](./running-triso-atops.md) | rung 4 step 3; running page | code to code | — |
| `diffusion` | [4](../../tutorials/triso-atops/layers.html), [7](../../tutorials/triso-atops/coolant.html) | [Getting out](./release.md) | rung 7 steps 1, 3 | code to code | — |
| `release_models` (`steady_state`, `transient`) | [4](../../tutorials/triso-atops/layers.html) step 3 | [Getting out](./release.md) | rung 4, step 3 | code to code; Booth = Crank 2.2·10⁻¹⁶ | gh:#385 |
| `activities` (`source_terms`, `coolant_activity`, `live_pools`) | [5](../../tutorials/triso-atops/failure.html) step 7, [7](../../tutorials/triso-atops/coolant.html) steps 1–2 | [source term](./source-term.md) | rung 7, steps 1–2 | code to code; `live_pools` vs closed forms | — |
| `normal_operation` | [7](../../tutorials/triso-atops/coolant.html) | [source term](./source-term.md) | rung 7, steps 1–2 | code to code (3.1·10⁻¹¹); MHTGR/Stoyer | `k_plate` misprint |
| `accident` | [7](../../tutorials/triso-atops/coolant.html) step 3 | [source term](./source-term.md) | rung 7, step 3 | code to code; MHTGR/Stoyer heat-up (~0.90 factor) | gh:#446, #447 |
| `run_selection` | — | [running](./running-triso-atops.md) | running page | code to code (name normalisation) | gh:#219 |
| `run_file`, `run_file::upstream` | — | [running](./running-triso-atops.md) | — | code to code vs `process_run_file` | — |

## Compute (`compute`, `gpu`, `wasm_par`)

| Module | Extended | V&V record | Gaps |
|---|---|---|---|
| `compute` (`ComputeType`, `ThreadCount`) | [compute backends](./compute-backends.md) | unit tests | — |
| `gpu` (off Android and wasm) | [compute backends](./compute-backends.md) | **unverified**: only the CPU fallback has run | stated in the module |
| `wasm_par` (wasm only) | [compute backends](./compute-backends.md) | compiles; serial path is the reference | — |

## Examples

| Example | Page | Headless check |
|---|---|---|
| `triso_cell_slice` | [rung 1](../../tutorials/triso-atops/triso.html) | yes: asserts the recovered radii |
| `layer_diffusion_table` | [rung 4](../../tutorials/triso-atops/layers.html) | prints a table |
| `first_passage_realtime`, `triso_simulator`, `boon_lay_decay_simulator` | [architecture](./architecture.md) | **no `--headless` mode** (workspace rule) |
