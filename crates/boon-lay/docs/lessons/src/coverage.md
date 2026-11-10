# Coverage: every module, its lesson, its check

> **Research, education and V&V only** ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> Surveyed 2026-10-04 at `develop` `5e802df3a4`: 25 641 lines in
> `crates/boon-lay/src`, about 509 public items.

Every public module of the crate has a row. A module with no lesson would be
a visible row with "none"; at this survey there is none without a page, but
several have **no V&V**, and those rows say so. "Code Review map" names the
page section whose link into the [Code Review map](../../code-review/) reaches
the module (until 2026-10-14, a generated code walk). ~~"Core lesson" named a
rung and step of the TRISO-ATOPS tutorial book.~~ **CORRECTED 2026-11-04
(gh:#1207):** that book was folded into the Fission products science lessons,
and "Core lesson" now names the lesson page; "that lesson" in the Code Review
map column means the lesson's "Where it is in the code" section. The
[concept map](../../science/fission-products/atops-concepts.html) gives the
same mapping concept by concept for TRISO-ATOPS.

## The random-walk side (`lagrangian_decay_simulator`, `lagrangian_transmutation_and_fission_simulator`)

| Module | Core lesson | Extended | Code Review map | V&V record | Gaps |
|---|---|---|---|---|---|
| `…::single_particle_simulator::constructive_solid_geometry` (`TrisoCell`, `Region`, `Sphere`, `TrisoRegion`) | [Why layers?](../../science/fission-products/barriers-why.html) | [legacy](./legacy-gaussian.md) | that lesson | `triso_cell_slice` geometry check (2026-10-04) | — |
| `…::monte_carlo_single_radionuclide_decay_simulator` (`SingleNuclideSimulatorMC`, postprocessing) | [An atom's clock](../../science/fission-products/decay-clock.html) | — | that lesson | `stochastic_half_life_calculator` (re-measured 2026-10-04), `decay_chain_th232` | branching frequencies never sampled |
| `…::stochastic_decay_chain` (`StochasticDecayChain`, iterators) | [An atom's clock](../../science/fission-products/decay-clock.html) | — | that lesson | `decay_chain_th232` | gh:#538 |
| `…::central_limit_theorem` (variance formulas, Gaussian sampler, `OoRng64`) | [Walking on spheres](../../science/fission-products/walk-on-spheres.html) | [legacy](./legacy-gaussian.md) | that lesson | none (diagnosis doc only) | no tests of its own |
| `…::single_particle_simulator` (`SingleParticleDiffusionSimulatorMC`), `movement_within_triso_particle`, `interaction_with_decaying_nuclide_simulator`, `cached_normals` | [Walking on spheres](../../science/fission-products/walk-on-spheres.html) | [legacy](./legacy-gaussian.md) | that lesson | none | one unit test; no V&V |
| `…::isotropic_scattering` (`Vec3`) | — | [legacy](./legacy-gaussian.md) | — | none | — |
| `…::chatgpt_5_*` (four), `triso_particle_widget` | — | [legacy](./legacy-gaussian.md) | — | test-only code | not in a library build |
| `…::first_passage::sphere_fpt` | [Walking on spheres](../../science/fission-products/walk-on-spheres.html) | [Getting out](./release.md) | that lesson | four unit tests (pass 2026-10-04) | — |
| `…::first_passage::walk_on_spheres` (`WoSWalker`, `WalkParams`, `shell_bounds`) | [Walking on spheres](../../science/fission-products/walk-on-spheres.html), [Across the whole particle](../../science/fission-products/barriers-whole.html) | [Getting out](./release.md) | that lesson | [CRP-6 Case 1](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/crp6_case1_kernel_release_vs_crank.md) (re-measured) | no multilayer record; gh:#537 |
| `…::single_particle_simulator::release_fraction_crp_6_case_1a_1b`, `release_fraction_analytical_solution` | [Walking on spheres](../../science/fission-products/walk-on-spheres.html) | — | that lesson | CRP-6 Case 1 | gh:#537 (swallowed literature check) |
| `…::first_passage::interface` | [Migration through the layers](../../science/fission-products/barriers-migration.html), [Across the whole particle](../../science/fission-products/barriers-whole.html) | [Getting out](./release.md) | that lesson | [interface equilibrium](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/interface_uniform_equilibrium_density.md) (re-measured) | seed spread unmeasured |
| `…::temperature_dependent_collisions` (+ `diffusion_coeffs`: Ag, Cs, Sr, Kr) | [Migration through the layers](../../science/fission-products/barriers-migration.html) | [legacy](./legacy-gaussian.md) | that lesson | transcription tests vs Jiang 2023 curves; `layer_diffusion_table` | gh:#541 |
| `…::first_passage::depletion`; `lagrangian_transmutation_and_fission_simulator` | named in [An atom's clock](../../science/fission-products/decay-clock.html) | [transmutation](./transmutation.md) | transmutation page | unit tests only | gh:#539 |
| `…::first_passage::ensemble`, `…::first_passage::live` | — | [compute backends](./compute-backends.md) | compute page | unit tests (reproducibility, agreement with Crank) | — |

## Decay data

| Module | Core lesson | Extended | Code Review map | V&V record | Gaps |
|---|---|---|---|---|---|
| `nuclide_reaction_and_decay_data` (`DecayLibrary`, `NuclideReactionAndDecayData`, `DecayType`, `get_decay_info`, parsing) | [An atom's clock](../../science/fission-products/decay-clock.html) | [decay data](./decay-data.md) | that lesson | about 70 half-life parse tests | gh:#538 (~~`get_decay_branch_info` is `todo!()`~~ implemented 2026-11-10) |
| `decay_xml_info_serde` | — | [decay data](./decay-data.md) | decay-data page | re-export only | — |
| `prelude` | — | [architecture](./architecture.md) | — | re-export only | — |

## boon-lay fuel failure (`fuel_failure`)

| Module | Core lesson | Extended | Code Review map | V&V record | Gaps |
|---|---|---|---|---|---|
| `geometry` (`SicLayer`) | [Why layers?](../../science/fission-products/barriers-why.html), [A tiny pressure vessel](../../science/fission-products/failure-vessel.html) | [fuel failure](./fuel-failure.md) | that lesson | stress-route tests | — |
| `pressure`, `booth`, `oxygen`, `molar_volume`, `diffusion` | [A tiny pressure vessel](../../science/fission-products/failure-vessel.html) | [fuel failure](./fuel-failure.md) | that lesson | module docs + [units register](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md) (Figs 1, 2, 3) | UO₂ `D_S` disagrees with Fig. 2 (recorded) |
| `stress`, `corrosion` | [A tiny pressure vessel](../../science/fission-products/failure-vessel.html), [The wall gets thinner](../../science/fission-products/failure-thinning.html) | [fuel failure](./fuel-failure.md) | that lesson | Fig. 4 (483 points) | — |
| `weibull`, `strength`, `grain_boundary` | [How strong is SiC?](../../science/fission-products/failure-weibull.html) | [fuel failure](./fuel-failure.md) | that lesson | Table 1 16/16, Table 2 | — |
| `decomposition` | [An accident, step by step](../../science/fission-products/failure-accident.html), [What the model leaves out](../../science/fission-products/failure-leaves-out.html) | [fuel failure](./fuel-failure.md) | that lesson | **none in the report**; (14a) excluded by Fig. 6 | unverified |
| `history` (`AccidentHistory`), `mod` (`total_failure_fraction`) | [An accident, step by step](../../science/fission-products/failure-accident.html) | [fuel failure](./fuel-failure.md) | that lesson | step-size independence; Figs 6, 7 | gh:#295 |
| `htr10` | [An accident, step by step](../../science/fission-products/failure-accident.html) | [HTR-10](./htr10.md) | — | sweeps re-measured 2026-10-04 | extrapolation; gh:#296 |
| `htr10::qualification` | [What the model leaves out](../../science/fission-products/failure-leaves-out.html) | [HTR-10](./htr10.md) | — | burnup ordering (pass) | gh:#383 |

## Chemistry (`chemistry`)

| Module | Core lesson | Code Review map | V&V record | Gaps |
|---|---|---|---|---|
| `graphite_air` | [Air on hot graphite](../../science/fission-products/chem-air.html) | that lesson | transcription test | gh:#457, #441, #442 |
| `graphite_steam` | [Steam on graphite and bare kernels](../../science/fission-products/chem-steam.html) | that lesson | transcription test; loose band vs measured points | gh:#441 |
| `kernel_hydrolysis` | [Steam on graphite and bare kernels](../../science/fission-products/chem-steam.html) | that lesson | transcription test | gh:#418, #444 |

## The TRISO-ATOPS port (`triso_atops_fork`)

| Module | Core lesson | Extended | Code Review map | V&V record | Gaps |
|---|---|---|---|---|---|
| `nuclide_model` (+ `nuclide_database`) | [84 nuclides, five groups](../../science/fission-products/atops-groups.html) | [running](./running-triso-atops.md) | that lesson | code to code | — |
| `diffusion` | [One diffusion law per group](../../science/fission-products/atops-diffusion.html) | [Getting out](./release.md) | that lesson | code to code | — |
| `release_models` (`steady_state`, `transient`) | [One release model per group](../../science/fission-products/atops-release-models.html), [Booth on one grain](../../science/fission-products/kernel-booth-grain.html) | [Getting out](./release.md) | that lesson | code to code; Booth = Crank 2.2·10⁻¹⁶ | gh:#385 |
| `activities` (`source_terms`, `coolant_activity`, `live_pools`) | [Normal operation, node by node](../../science/fission-products/atops-normal-operation.html), [Into the coolant](../../science/fission-products/atops-coolant.html) | [source term](./source-term.md) | that lesson | code to code; `live_pools` vs closed forms | — |
| `normal_operation` | [Normal operation, node by node](../../science/fission-products/atops-normal-operation.html) | [source term](./source-term.md) | that lesson | code to code (3.1·10⁻¹¹); MHTGR/Stoyer | `k_plate` misprint |
| `accident` | [Accidents: Booth on the integral, and venting](../../science/fission-products/atops-accident.html) | [source term](./source-term.md) | that lesson | code to code; MHTGR/Stoyer heat-up (~0.90 factor) | gh:#446, #447 |
| `run_selection` | — | [running](./running-triso-atops.md) | running page | code to code (name normalisation) | gh:#219 |
| `variant` (`TrisoAtopsVariant`, `PaperReadings`) | — | [paper reproduction](./atops2026-paper.md) | paper reproduction page | `Atops2026Paper(UPSTREAM_CODE)` = `Upstream` bit for bit; the paper's printed releases (Table 19: upstream 8/8, `atops2026paper` 6/8) | not code to code (no upstream counterpart); gh:#1015 |
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
| `triso_cell_slice` | [Why layers?](../../science/fission-products/barriers-why.html) (optional section) | yes: asserts the recovered radii |
| `layer_diffusion_table` | [Migration through the layers](../../science/fission-products/barriers-migration.html) | prints a table |
| `first_passage_realtime`, `triso_simulator`, `boon_lay_decay_simulator` | [architecture](./architecture.md) | **no `--headless` mode** (workspace rule) |
